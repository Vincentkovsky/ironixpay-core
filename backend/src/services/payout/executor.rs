//! Payout Executor Trait and Implementations
//!
//! Abstracts chain-specific payout (withdrawal) operations behind a common trait.
//! - `TronPayoutExecutor`: Uses EnergyManager + TronBroadcaster + SHA-256 signing
//! - `EvmPayoutExecutor`: Gas funding (BNB) + ERC20 transfer (USDT)
//!
//! Follows the same pattern as `SweepExecutor` in `services::sweeper::executor`.

use crate::entity::transactions::ChainTxState;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::services::outbound::{
    BroadcastDisposition, OutboundTransactionStore, RecoveryDisposition, StoredSignedTransaction,
};

/// Result of a payout execution
#[derive(Debug)]
pub struct PayoutResult {
    /// Transaction hash of the USDT transfer
    pub tx_hash: String,
    /// Optional funding transaction hash (TRX energy or BNB gas)
    pub funding_tx_hash: Option<String>,
    pub broadcast_disposition: BroadcastDisposition,
}

/// Chain-agnostic payout executor interface.
///
/// Each chain implements this trait to handle the full payout lifecycle:
/// gas/energy preparation → build token transfer → sign → broadcast → confirm.
#[async_trait]
pub trait PayoutExecutor: Send + Sync {
    /// Execute a full payout: prepare resources → build tx → sign → broadcast.
    ///
    /// `token_contract` specifies which TRC-20/ERC-20 contract to transfer.
    /// `token_decimals` specifies the on-chain decimals for amount scaling (e.g., 18 for BSC USDT, 6 for ETH USDT).
    ///
    /// Returns the payout result with tx hash on success.
    async fn execute_payout(
        &self,
        from_address: &str,   // treasury address
        to_address: &str,     // merchant collection address
        amount: u64,          // net_amount in 6-decimal system standard
        account_index: i32,   // HD derivation (treasury = 0)
        path_index: u32,      // HD derivation (treasury = 0)
        token_contract: &str, // TRC-20/ERC-20 contract address
        token_decimals: u8,   // on-chain decimals for amount scaling
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<PayoutResult>;

    /// Check transaction confirmation status.
    ///
    /// Returns `ChainTxState` for the given tx hash.
    async fn check_tx_status(&self, tx_hash: &str, min_confirmations: u64) -> Result<ChainTxState>;

    async fn recover_broadcast(
        &self,
        payload: &StoredSignedTransaction,
    ) -> Result<RecoveryDisposition> {
        let _ = payload;
        Ok(RecoveryDisposition::BroadcastUnknown(
            "Executor does not support durable rebroadcast".into(),
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRON Payout Executor
// ═══════════════════════════════════════════════════════════════════════════════

use crate::services::address::key_provider::TransactionSigner;
use crate::services::energy::EnergyManager;
use crate::services::tron::interface::TronBroadcaster;

pub struct TronPayoutExecutor {
    tron_client: Arc<dyn TronBroadcaster + Send + Sync>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
    energy_manager: Arc<EnergyManager>,
}

impl TronPayoutExecutor {
    pub fn new(
        tron_client: Arc<dyn TronBroadcaster + Send + Sync>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
        energy_manager: Arc<EnergyManager>,
    ) -> Self {
        Self {
            tron_client,
            signer,
            energy_manager,
        }
    }
}

#[async_trait]
impl PayoutExecutor for TronPayoutExecutor {
    async fn execute_payout(
        &self,
        from_address: &str,
        to_address: &str,
        amount: u64,
        account_index: i32,
        path_index: u32,
        token_contract: &str,
        _token_decimals: u8,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<PayoutResult> {
        // 1. Verify treasury liquidity before spending resources on the transfer.
        let treasury_balance = self
            .tron_client
            .get_trc20_balance(from_address, token_contract)
            .await?;
        let required_balance = i64::try_from(amount)
            .map_err(|_| anyhow::anyhow!("Payout amount exceeds supported token range"))?;
        if treasury_balance < required_balance {
            return Err(anyhow::anyhow!(
                "Treasury token balance insufficient: have {}, need {}",
                treasury_balance,
                amount
            ));
        }

        // 2. Prepare resources before making the root token transaction replayable.
        let resource_cost = self
            .energy_manager
            .ensure_resources(
                from_address,
                amount as i64,
                to_address,
                outbound_id,
                outbound_store,
            )
            .await?;

        if let Some(ref fund_hash) = resource_cost.funding_tx_hash {
            debug!(funding_tx = %fund_hash, "TRX energy funding completed");
        }

        // 3. Build, sign and durably record the exact token transaction.
        let unsigned = self
            .tron_client
            .build_trc20_transfer(from_address, to_address, amount, token_contract)
            .await?;
        let signature = self
            .signer
            .sign_transaction(&unsigned.raw_data, account_index, path_index)
            .await?;
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&unsigned.raw_data);
        let tx_hash = hex::encode(hasher.finalize());
        let signed = crate::services::tron::interface::SignedTransaction {
            tx_id: tx_hash.clone(),
            raw_data: unsigned.raw_data,
            signature,
            raw_data_json: unsigned.raw_data_json,
            expiration: unsigned.expiration,
        };
        outbound_store
            .record_signed(
                outbound_id,
                &StoredSignedTransaction::Tron {
                    tx_hash: tx_hash.clone(),
                    raw_data_hex: hex::encode(&signed.raw_data),
                    signature_hex: hex::encode(&signed.signature),
                    raw_data_json: signed.raw_data_json.clone(),
                    expiration_ms: signed.expiration,
                },
            )
            .await?;

        // 4. Broadcast
        let (broadcast_disposition, broadcast_error) =
            match self.tron_client.broadcast(&signed).await {
                Ok(result)
                    if result.success
                        && (result.tx_hash.is_empty() || result.tx_hash == tx_hash) =>
                {
                    (BroadcastDisposition::Accepted, None)
                }
                Ok(result) => {
                    (
                        BroadcastDisposition::Unknown,
                        Some(result.message.unwrap_or_else(|| {
                            format!("Unexpected TRON tx hash {}", result.tx_hash)
                        })),
                    )
                }
                Err(error) => (
                    BroadcastDisposition::Unknown,
                    Some(format!("TRON broadcast response was ambiguous: {error}")),
                ),
            };
        if let Err(error) = outbound_store
            .mark_broadcast(outbound_id, broadcast_disposition.clone(), broadcast_error)
            .await
        {
            tracing::error!(outbound_id, error = %error, "Failed to persist TRON payout broadcast outcome");
        }

        info!(tx_hash = %tx_hash, to = %to_address, amount, "TRON payout handed off to confirmation");

        Ok(PayoutResult {
            tx_hash,
            funding_tx_hash: resource_cost.funding_tx_hash,
            broadcast_disposition,
        })
    }

    async fn recover_broadcast(
        &self,
        payload: &StoredSignedTransaction,
    ) -> Result<RecoveryDisposition> {
        let StoredSignedTransaction::Tron {
            tx_hash,
            raw_data_hex,
            signature_hex,
            raw_data_json,
            expiration_ms,
        } = payload
        else {
            return Err(anyhow::anyhow!("Expected stored TRON transaction"));
        };
        if self
            .tron_client
            .transaction_known_on_any_endpoint(tx_hash)
            .await?
        {
            return Ok(RecoveryDisposition::Pending);
        }
        if expiration_ms
            .is_some_and(|expiration| chrono::Utc::now().timestamp_millis() > expiration + 60_000)
        {
            if self
                .tron_client
                .transaction_known_on_any_endpoint(tx_hash)
                .await?
            {
                return Ok(RecoveryDisposition::Pending);
            }
            return Ok(RecoveryDisposition::Expired);
        }
        let signed = crate::services::tron::interface::SignedTransaction {
            tx_id: tx_hash.clone(),
            raw_data: hex::decode(raw_data_hex)?,
            signature: hex::decode(signature_hex)?,
            raw_data_json: raw_data_json.clone(),
            expiration: *expiration_ms,
        };
        match self.tron_client.broadcast(&signed).await {
            Ok(result)
                if result.success && (result.tx_hash.is_empty() || result.tx_hash == *tx_hash) =>
            {
                Ok(RecoveryDisposition::Pending)
            }
            Ok(result) => Ok(RecoveryDisposition::BroadcastUnknown(
                result
                    .message
                    .unwrap_or_else(|| "TRON rebroadcast rejected".into()),
            )),
            Err(error) => Ok(RecoveryDisposition::BroadcastUnknown(error.to_string())),
        }
    }

    async fn check_tx_status(&self, tx_hash: &str, min_confirmations: u64) -> Result<ChainTxState> {
        // Check execution receipt
        let info_opt = self.tron_client.get_transaction_info(tx_hash).await?;

        match info_opt {
            Some(info) => {
                if !info.success {
                    return Ok(ChainTxState::Failed);
                }

                if min_confirmations > 0 {
                    let current_block = self.tron_client.get_current_block().await?.number;
                    let block_num = info.block_number.max(0) as u64;
                    let confirmations = current_block.saturating_sub(block_num);
                    if confirmations < min_confirmations {
                        return Ok(ChainTxState::Unconfirmed);
                    }
                }

                Ok(ChainTxState::Confirmed)
            }
            None => {
                // Check if tx exists at all (mempool/pending)
                let tx_opt = self.tron_client.get_transaction_by_id(tx_hash).await?;
                match tx_opt {
                    Some(tx) => {
                        // Check TRON expiration (zombie trap)
                        if let Some(exp) = tx.expiration {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)?
                                .as_millis() as i64;
                            const EXPIRATION_BUFFER_MS: i64 = 60_000;
                            if now_ms > (exp + EXPIRATION_BUFFER_MS) {
                                warn!(
                                    tx_hash,
                                    "Payout TX expired without being mined (Zombie Trap)"
                                );
                                return Ok(ChainTxState::NotFound);
                            }
                        }
                        Ok(ChainTxState::Pending)
                    }
                    None => Ok(ChainTxState::NotFound),
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EVM Payout Executor (BSC, Ethereum, etc.)
// ═══════════════════════════════════════════════════════════════════════════════

use crate::services::chain::traits::ChainClient;
use crate::services::chain::types::{ChainSignedTx, ChainUnsignedTx};
use crate::services::evm::gas_funder::EvmGasFunder;
use crate::services::evm::signing::{assemble_signed_tx, rlp_encode_for_signing};
use crate::services::evm::EvmClient;

pub struct EvmPayoutExecutor {
    evm_client: Arc<EvmClient>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
    /// Shared gas funder (provides nonce-safe native gas funding)
    gas_funder: Arc<EvmGasFunder>,
}

impl EvmPayoutExecutor {
    pub fn new(
        evm_client: Arc<EvmClient>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
        gas_funder: Arc<EvmGasFunder>,
    ) -> Self {
        Self {
            evm_client,
            signer,
            gas_funder,
        }
    }

    /// Scale a 6-decimal system amount to chain-native decimals.
    ///
    /// For BSC USDT/USDC (18 decimals): multiplies by 10^12
    /// For ETH USDT/USDC (6 decimals): no-op
    fn scale_to_chain_decimals(amount_6dec: u64, token_decimals: u8) -> alloy_primitives::U256 {
        if token_decimals <= 6 {
            alloy_primitives::U256::from(amount_6dec)
        } else {
            let shift = (token_decimals - 6) as u32;
            let multiplier =
                alloy_primitives::U256::from(10u64).pow(alloy_primitives::U256::from(shift));
            alloy_primitives::U256::from(amount_6dec) * multiplier
        }
    }
}

#[async_trait]
impl PayoutExecutor for EvmPayoutExecutor {
    async fn execute_payout(
        &self,
        from_address: &str,
        to_address: &str,
        amount: u64,
        account_index: i32,
        path_index: u32,
        token_contract: &str,
        token_decimals: u8,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<PayoutResult> {
        // Step 1: Verify treasury liquidity before spending native gas.
        let chain_amount = Self::scale_to_chain_decimals(amount, token_decimals);
        let treasury_balance =
            ChainClient::get_token_balance(self.evm_client.as_ref(), from_address, token_contract)
                .await?;
        if treasury_balance < chain_amount {
            return Err(anyhow::anyhow!(
                "Treasury token balance insufficient: have {}, need {}",
                treasury_balance,
                chain_amount
            ));
        }

        // Step 2: Estimate gas + idempotent gas funding
        let gas_limit = self
            .evm_client
            .estimate_token_transfer_gas(from_address, to_address, token_contract, chain_amount)
            .await;
        let funding = self
            .gas_funder
            .ensure_gas(from_address, gas_limit, outbound_id, outbound_store)
            .await?;

        info!(
            from = %from_address,
            to = %to_address,
            amount_6dec = amount,
            chain_amount = %chain_amount,
            "Building ERC-20 payout transaction"
        );

        let unsigned_tx = match ChainClient::build_token_transfer(
            self.evm_client.as_ref(),
            from_address,
            to_address,
            token_contract,
            chain_amount,
        )
        .await?
        {
            ChainUnsignedTx::Evm(tx) => tx,
            _ => return Err(anyhow::anyhow!("Expected EVM unsigned tx")),
        };

        let rlp_bytes = rlp_encode_for_signing(&unsigned_tx);
        let signature = self
            .signer
            .sign_transaction_for_coin(&rlp_bytes, account_index, path_index, 60)
            .await?;
        let signed = assemble_signed_tx(&unsigned_tx, &signature)?;
        let local_tx_hash = signed.tx_hash.clone();

        outbound_store
            .record_signed(
                outbound_id,
                &StoredSignedTransaction::Evm {
                    tx_hash: local_tx_hash.clone(),
                    raw_tx_hex: signed.raw_tx_hex.clone(),
                    from_address: unsigned_tx.from.clone(),
                    nonce: unsigned_tx.nonce,
                },
            )
            .await?;

        debug!(tx_hash = %signed.tx_hash, "Broadcasting EVM payout transaction");

        let broadcast =
            ChainClient::broadcast(self.evm_client.as_ref(), &ChainSignedTx::Evm(signed)).await;
        let (broadcast_disposition, broadcast_error) = match broadcast {
            Ok(result) if result.success && result.tx_hash.eq_ignore_ascii_case(&local_tx_hash) => {
                (BroadcastDisposition::Accepted, None)
            }
            Ok(result) => (
                BroadcastDisposition::Unknown,
                Some(format!(
                    "EVM broadcast acknowledgement mismatch: {:?}",
                    result.message
                )),
            ),
            Err(error) => (
                BroadcastDisposition::Unknown,
                Some(format!("EVM broadcast response was ambiguous: {error}")),
            ),
        };
        if let Err(error) = outbound_store
            .mark_broadcast(outbound_id, broadcast_disposition.clone(), broadcast_error)
            .await
        {
            tracing::error!(outbound_id, error = %error, "Failed to persist EVM payout broadcast outcome");
        }

        info!(tx_hash = %local_tx_hash, to = %to_address, amount, "EVM payout handed off to confirmation");

        Ok(PayoutResult {
            tx_hash: local_tx_hash,
            funding_tx_hash: funding.funding_tx_hash,
            broadcast_disposition,
        })
    }

    async fn recover_broadcast(
        &self,
        payload: &StoredSignedTransaction,
    ) -> Result<RecoveryDisposition> {
        let StoredSignedTransaction::Evm {
            tx_hash,
            raw_tx_hex,
            from_address,
            nonce,
        } = payload
        else {
            return Err(anyhow::anyhow!("Expected stored EVM transaction"));
        };
        if self
            .evm_client
            .transaction_known_on_any_endpoint(tx_hash)
            .await?
        {
            return Ok(RecoveryDisposition::Pending);
        }
        if self
            .evm_client
            .get_latest_nonce_across_endpoints(from_address)
            .await?
            > *nonce
        {
            if self
                .evm_client
                .transaction_known_on_any_endpoint(tx_hash)
                .await?
            {
                return Ok(RecoveryDisposition::Pending);
            }
            return Ok(RecoveryDisposition::Replaced);
        }
        let signed = crate::services::chain::types::EvmSignedTx {
            tx_hash: tx_hash.clone(),
            raw_tx_hex: raw_tx_hex.clone(),
        };
        match ChainClient::broadcast(self.evm_client.as_ref(), &ChainSignedTx::Evm(signed)).await {
            Ok(result) if result.success && result.tx_hash.eq_ignore_ascii_case(tx_hash) => {
                Ok(RecoveryDisposition::Pending)
            }
            Ok(result) => Ok(RecoveryDisposition::BroadcastUnknown(format!(
                "EVM rebroadcast acknowledgement mismatch: {:?}",
                result.message
            ))),
            Err(error) => Ok(RecoveryDisposition::BroadcastUnknown(error.to_string())),
        }
    }

    async fn check_tx_status(&self, tx_hash: &str, min_confirmations: u64) -> Result<ChainTxState> {
        match ChainClient::get_transaction_info(self.evm_client.as_ref(), tx_hash).await {
            Ok(Some(info)) => {
                if !info.success {
                    return Ok(ChainTxState::Failed);
                }
                if min_confirmations > 0 {
                    let current_block = ChainClient::get_current_block(self.evm_client.as_ref())
                        .await?
                        .number;
                    let block_num = info.block_number.max(0) as u64;
                    let confirmations = current_block.saturating_sub(block_num);
                    if confirmations < min_confirmations {
                        return Ok(ChainTxState::Unconfirmed);
                    }
                }
                Ok(ChainTxState::Confirmed)
            }
            Ok(None) => Ok(ChainTxState::NotFound),
            Err(e) => Err(e),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Solana Payout Executor
// ═══════════════════════════════════════════════════════════════════════════════

use crate::services::solana::SolanaClient;

/// Solana SPL Token payout executor (single-signer).
///
/// Unlike `SolanaSweepExecutor` (dual-signer: fee_payer ≠ authority),
/// payouts go from treasury → merchant, so treasury is both fee payer
/// AND token authority — only **one Ed25519 signature** needed.
pub struct SolanaPayoutExecutor {
    solana_client: Arc<SolanaClient>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
}

impl SolanaPayoutExecutor {
    pub fn new(
        solana_client: Arc<SolanaClient>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
    ) -> Self {
        Self {
            solana_client,
            signer,
        }
    }

    /// Scale a 6-decimal system amount to on-chain token decimals.
    ///
    /// For Solana USDT/USDC (6 decimals): pass-through.
    /// For tokens with different decimals: scale accordingly.
    fn scale_to_chain_decimals(amount_6dec: u64, token_decimals: u8) -> u64 {
        match token_decimals.cmp(&6) {
            std::cmp::Ordering::Equal => amount_6dec,
            std::cmp::Ordering::Greater => {
                let multiplier = 10u64.pow((token_decimals - 6) as u32);
                amount_6dec.saturating_mul(multiplier)
            }
            std::cmp::Ordering::Less => {
                let divisor = 10u64.pow((6 - token_decimals) as u32);
                amount_6dec / divisor
            }
        }
    }
}

/// Solana coin type for SLIP-0010 / BIP44 derivation (Ed25519).
const SOLANA_COIN_TYPE: u32 = 501;

#[async_trait]
impl PayoutExecutor for SolanaPayoutExecutor {
    async fn execute_payout(
        &self,
        from_address: &str,
        to_address: &str,
        amount: u64,
        account_index: i32,
        path_index: u32,
        token_contract: &str, // SPL token mint address (Base58)
        token_decimals: u8,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<PayoutResult> {
        // 1. Scale amount from system 6-decimal to on-chain decimals
        let chain_amount = Self::scale_to_chain_decimals(amount, token_decimals);

        // 2. Verify treasury liquidity before constructing and signing the transfer.
        let treasury_balance = self
            .solana_client
            .get_spl_token_balance(from_address, token_contract)
            .await?;
        let chain_amount_i64 = i64::try_from(chain_amount)
            .map_err(|_| anyhow::anyhow!("Payout amount exceeds supported token range"))?;
        if treasury_balance < chain_amount_i64 {
            return Err(anyhow::anyhow!(
                "Treasury token balance insufficient: have {}, need {}",
                treasury_balance,
                chain_amount
            ));
        }

        // Determine token program ID from mint address format
        // Default to standard SPL Token program (covers USDT, USDC)
        let token_program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

        info!(
            from = %from_address,
            to = %to_address,
            amount_6dec = amount,
            chain_amount,
            mint = %token_contract,
            "Building Solana SPL Token payout (single-signer)"
        );

        // 3. Build unsigned transfer (single signer: from = fee_payer = authority)
        let unsigned_tx = self
            .solana_client
            .build_spl_transfer(
                from_address,
                to_address,
                token_contract,
                chain_amount,
                token_decimals,
                token_program_id,
            )
            .await?;

        debug!(
            num_signers = unsigned_tx.num_required_signatures,
            "Unsigned payout transaction built"
        );

        // 4. Sign with treasury key (single signature)
        let signature = self
            .signer
            .sign_transaction_for_coin(
                &unsigned_tx.message_bytes,
                account_index,
                path_index,
                SOLANA_COIN_TYPE,
            )
            .await?;

        // 5. Assemble signed transaction
        let signed_tx = crate::services::solana::signing::assemble_signed_solana_tx(
            &unsigned_tx,
            &[signature],
        )?;

        outbound_store
            .record_signed(
                outbound_id,
                &StoredSignedTransaction::Solana {
                    tx_hash: signed_tx.signature.clone(),
                    serialized_tx: signed_tx.serialized_tx.clone(),
                    recent_blockhash: unsigned_tx.recent_blockhash.clone(),
                    last_valid_block_height: unsigned_tx.last_valid_block_height,
                },
            )
            .await?;

        debug!(tx_hash = %signed_tx.signature, "Broadcasting Solana payout transaction");

        // 5. Broadcast with retry loop
        let broadcast = self
            .solana_client
            .broadcast_solana(&signed_tx, &unsigned_tx.recent_blockhash)
            .await;
        let (broadcast_disposition, broadcast_error) = match broadcast {
            Ok(result) if result.success && result.tx_hash == signed_tx.signature => {
                (BroadcastDisposition::Accepted, None)
            }
            Ok(result) => (
                BroadcastDisposition::Unknown,
                Some(format!(
                    "Solana broadcast acknowledgement mismatch: {:?}",
                    result.message
                )),
            ),
            Err(error) => (
                BroadcastDisposition::Unknown,
                Some(format!("Solana broadcast response was ambiguous: {error}")),
            ),
        };
        if let Err(error) = outbound_store
            .mark_broadcast(outbound_id, broadcast_disposition.clone(), broadcast_error)
            .await
        {
            tracing::error!(outbound_id, error = %error, "Failed to persist Solana payout broadcast outcome");
        }

        info!(
            tx_hash = %signed_tx.signature,
            to = %to_address,
            amount_6dec = amount,
            "Solana payout handed off to confirmation"
        );

        Ok(PayoutResult {
            tx_hash: signed_tx.signature,
            funding_tx_hash: None, // No separate funding — treasury pays fees directly
            broadcast_disposition,
        })
    }

    async fn recover_broadcast(
        &self,
        payload: &StoredSignedTransaction,
    ) -> Result<RecoveryDisposition> {
        let StoredSignedTransaction::Solana {
            tx_hash,
            serialized_tx,
            last_valid_block_height,
            ..
        } = payload
        else {
            return Err(anyhow::anyhow!("Expected stored Solana transaction"));
        };
        if self
            .solana_client
            .signature_known_on_any_endpoint(tx_hash)
            .await?
        {
            return Ok(RecoveryDisposition::Pending);
        }
        if self
            .solana_client
            .get_block_height_across_endpoints()
            .await?
            > *last_valid_block_height
        {
            if self
                .solana_client
                .signature_known_on_any_endpoint(tx_hash)
                .await?
            {
                return Ok(RecoveryDisposition::Pending);
            }
            return Ok(RecoveryDisposition::Expired);
        }
        match self.solana_client.send_transaction(serialized_tx).await {
            Ok(signature) if signature == *tx_hash => Ok(RecoveryDisposition::Pending),
            Ok(signature) => Ok(RecoveryDisposition::BroadcastUnknown(format!(
                "Solana rebroadcast returned signature {signature} for stored signature {tx_hash}"
            ))),
            Err(error) => Ok(RecoveryDisposition::BroadcastUnknown(error.to_string())),
        }
    }

    async fn check_tx_status(
        &self,
        tx_hash: &str,
        _min_confirmations: u64,
    ) -> Result<ChainTxState> {
        let statuses = self
            .solana_client
            .get_signature_statuses(&[tx_hash])
            .await?;

        match statuses.first() {
            Some(Some(status)) => {
                // Check for on-chain error
                if status.err.is_some() {
                    return Ok(ChainTxState::Failed);
                }

                // Check confirmation level
                let confirmed = status
                    .confirmation_status
                    .as_deref()
                    .is_some_and(|s| s == "confirmed" || s == "finalized");

                if confirmed {
                    Ok(ChainTxState::Confirmed)
                } else {
                    Ok(ChainTxState::Unconfirmed)
                }
            }
            Some(None) | None => Ok(ChainTxState::NotFound),
        }
    }
}
