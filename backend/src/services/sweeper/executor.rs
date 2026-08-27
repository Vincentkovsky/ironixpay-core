//! Sweep Executor Trait and Implementations
//!
//! Abstracts chain-specific sweep operations behind a common trait.
//! - `TronSweepExecutor`: Uses EnergyManager + TronBroadcaster + SHA-256 signing
//! - `EvmSweepExecutor`: 2-step sweep (fund BNB → sweep USDT) with gas leak prevention

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::services::outbound::{
    BroadcastDisposition, OutboundTransactionStore, RecoveryDisposition, StoredSignedTransaction,
};

/// Result of a sweep execution
#[derive(Debug)]
pub struct SweepResult {
    /// Transaction hash of the USDT sweep
    pub tx_hash: String,
    /// Optional funding transaction hash (EVM gas funding, TRON TRX bandwidth)
    pub funding_tx_hash: Option<String>,
    /// Amount swept (in token's native precision)
    pub amount_swept: i64,
    /// Estimated gas cost in native token's smallest unit (SUN for TRON, Wei for EVM).
    /// Used by service layer to convert to USDT via PriceOracle.
    pub gas_cost_native: u64,
    /// Whether the RPC acknowledged the broadcast or its outcome is ambiguous.
    pub broadcast_disposition: BroadcastDisposition,
}

/// Transaction confirmation status (chain-agnostic)
#[derive(Debug, Clone, PartialEq)]
pub enum SweepTxStatus {
    /// Transaction confirmed on-chain successfully
    Confirmed,
    /// Transaction confirmed but reverted/failed
    Failed,
    /// Transaction is pending (in mempool or unconfirmed)
    Pending,
    /// Transaction not found on-chain
    NotFound,
}

/// Chain-agnostic sweep executor interface.
///
/// Each chain implements this trait to handle the full sweep lifecycle:
/// build + sign + resource prep + broadcast.
///
/// CRITICAL: All implementations must be IDEMPOTENT for retry safety.
/// The sweeper service may call execute_sweep multiple times for the same address.
#[async_trait]
pub trait SweepExecutor: Send + Sync {
    /// Get token balance at the given address (in 6-decimal i64).
    /// `token_contract` specifies which TRC-20/ERC-20 contract to query.
    async fn get_balance(&self, address: &str, token_contract: &str) -> Result<i64>;

    /// Execute a full sweep: build tx, sign, prepare resources, broadcast.
    ///
    /// For TRON: build_trc20_transfer → SHA-256 → sign → ensure_resources → broadcast
    /// For EVM: check BNB → fund gas (idempotent) → build ERC20 transfer → sign → broadcast
    ///
    /// Returns the sweep result with tx hash and amount.
    async fn execute_sweep(
        &self,
        from_address: &str,
        to_address: &str,
        account_index: i32,
        path_index: u32,
        token_contract: &str,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<SweepResult>;

    /// Re-submit the exact same signed bytes, or prove they can no longer land.
    async fn recover_broadcast(
        &self,
        payload: &StoredSignedTransaction,
    ) -> Result<RecoveryDisposition> {
        let _ = payload;
        Ok(RecoveryDisposition::BroadcastUnknown(
            "Executor does not support durable rebroadcast".into(),
        ))
    }

    /// Check transaction confirmation status.
    async fn check_tx_status(
        &self,
        tx_hash: &str,
        required_confirmations: i32,
    ) -> Result<SweepTxStatus>;

    /// Get current block number (for stuck-tx age detection).
    async fn get_current_block(&self) -> Result<i64>;
}

// ─── TRON Sweep Executor ───────────────────────────────────────────────────

use crate::services::address::key_provider::TransactionSigner;
use crate::services::energy::EnergyManager;
use crate::services::transaction_monitor::service::TransactionMonitor;
use crate::services::tron::interface::TronBroadcaster;
use crate::services::tron::TronClient;

/// TRON sweep executor.
///
/// Wraps the existing TRON sweep logic:
/// 1. build_trc20_transfer
/// 2. SHA-256 hash → sign via TransactionSigner
/// 3. EnergyManager.ensure_resources (energy delegation + TRX funding)
/// 4. Broadcast via TronBroadcaster
pub struct TronSweepExecutor {
    tron_client: Arc<TronClient>,
    broadcaster: Arc<dyn TronBroadcaster + Send + Sync>,
    energy_manager: Arc<EnergyManager>,
    transaction_monitor: Arc<TransactionMonitor>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
}

impl TronSweepExecutor {
    pub fn new(
        tron_client: Arc<TronClient>,
        broadcaster: Arc<dyn TronBroadcaster + Send + Sync>,
        energy_manager: Arc<EnergyManager>,
        transaction_monitor: Arc<TransactionMonitor>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
    ) -> Self {
        Self {
            tron_client,
            broadcaster,
            energy_manager,
            transaction_monitor,
            signer,
        }
    }
}

#[async_trait]
impl SweepExecutor for TronSweepExecutor {
    async fn get_balance(&self, address: &str, token_contract: &str) -> Result<i64> {
        self.tron_client
            .get_trc20_balance(address, token_contract)
            .await
    }

    async fn execute_sweep(
        &self,
        from_address: &str,
        to_address: &str,
        account_index: i32,
        path_index: u32,
        token_contract: &str,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<SweepResult> {
        use sha2::{Digest, Sha256};
        use tokio::time::{sleep, Duration};

        // 1. Get balance (full amount sweep)
        let balance = self
            .tron_client
            .get_trc20_balance(from_address, token_contract)
            .await?;
        if balance == 0 {
            return Err(anyhow::anyhow!("No funds to sweep"));
        }

        // 2. Prepare resources while the root journal remains Preparing. A crash
        // here must never make an unfunded token transaction replayable.
        let resource_cost = self
            .energy_manager
            .ensure_resources(
                from_address,
                balance,
                to_address,
                outbound_id,
                outbound_store,
            )
            .await?;

        if resource_cost.funding_tx_hash.is_some() {
            // Wait for TRX funding to confirm
            sleep(Duration::from_secs(6)).await;
        }

        // 3. Build and sign only after resources are ready.
        let unsigned_tx = self
            .tron_client
            .build_trc20_transfer(from_address, to_address, balance as u64, token_contract)
            .await?;
        let mut hasher = Sha256::new();
        hasher.update(&unsigned_tx.raw_data);
        let tx_id_hex = hex::encode(hasher.finalize());
        let signature = self
            .signer
            .sign_transaction(&unsigned_tx.raw_data, account_index, path_index)
            .await?;
        let signed_tx = crate::services::tron::interface::SignedTransaction {
            tx_id: tx_id_hex.clone(),
            raw_data: unsigned_tx.raw_data,
            raw_data_json: unsigned_tx.raw_data_json,
            signature,
            expiration: unsigned_tx.expiration,
        };
        outbound_store
            .record_signed(
                outbound_id,
                &StoredSignedTransaction::Tron {
                    tx_hash: tx_id_hex.clone(),
                    raw_data_hex: hex::encode(&signed_tx.raw_data),
                    signature_hex: hex::encode(&signed_tx.signature),
                    raw_data_json: signed_tx.raw_data_json.clone(),
                    expiration_ms: signed_tx.expiration,
                },
            )
            .await?;

        // 4. Broadcast
        let (broadcast_disposition, broadcast_error) =
            match self.broadcaster.broadcast(&signed_tx).await {
                Ok(res) if res.success && (res.tx_hash.is_empty() || res.tx_hash == tx_id_hex) => {
                    (BroadcastDisposition::Accepted, None)
                }
                Ok(res) => (
                    BroadcastDisposition::Unknown,
                    Some(format!(
                        "TRON broadcast was not acknowledged for {}: {}",
                        tx_id_hex,
                        res.message
                            .unwrap_or_else(|| format!("unexpected tx hash {}", res.tx_hash))
                    )),
                ),
                Err(error) => (
                    BroadcastDisposition::Unknown,
                    Some(format!("TRON broadcast response was ambiguous: {error}")),
                ),
            };
        if let Err(error) = outbound_store
            .mark_broadcast(outbound_id, broadcast_disposition.clone(), broadcast_error)
            .await
        {
            tracing::error!(outbound_id, error = %error, "Failed to persist TRON broadcast outcome");
        }

        Ok(SweepResult {
            tx_hash: tx_id_hex,
            funding_tx_hash: resource_cost.funding_tx_hash,
            amount_swept: balance,
            gas_cost_native: resource_cost.total_cost_sun,
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

        let now_ms = chrono::Utc::now().timestamp_millis();
        if self
            .broadcaster
            .transaction_known_on_any_endpoint(tx_hash)
            .await?
        {
            return Ok(RecoveryDisposition::Pending);
        }
        if expiration_ms.is_some_and(|expiration| now_ms > expiration + 60_000) {
            if self
                .broadcaster
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
        match self.broadcaster.broadcast(&signed).await {
            Ok(result) if result.success => Ok(RecoveryDisposition::Pending),
            Ok(result) => Ok(RecoveryDisposition::BroadcastUnknown(
                result
                    .message
                    .unwrap_or_else(|| "TRON rebroadcast rejected".into()),
            )),
            Err(error) => Ok(RecoveryDisposition::BroadcastUnknown(error.to_string())),
        }
    }

    async fn check_tx_status(
        &self,
        tx_hash: &str,
        required_confirmations: i32,
    ) -> Result<SweepTxStatus> {
        let latest_block = self
            .tron_client
            .get_current_block()
            .await
            .ok()
            .map(|b| b.number);

        use crate::entity::transactions::ChainTxState;
        match self
            .transaction_monitor
            .check_tx_status(tx_hash, required_confirmations as u64, latest_block)
            .await?
        {
            ChainTxState::Confirmed => Ok(SweepTxStatus::Confirmed),
            ChainTxState::Failed => Ok(SweepTxStatus::Failed),
            ChainTxState::Pending | ChainTxState::Unconfirmed => Ok(SweepTxStatus::Pending),
            ChainTxState::NotFound => Ok(SweepTxStatus::NotFound),
        }
    }

    async fn get_current_block(&self) -> Result<i64> {
        let info = self.tron_client.get_current_block().await?;
        Ok(info.number as i64)
    }
}

// ─── EVM Sweep Executor ────────────────────────────────────────────────────

use crate::services::chain::traits::ChainClient;
use crate::services::evm::gas_funder::EvmGasFunder;
use crate::services::evm::EvmClient;

/// EVM sweep executor with 2-step gas funding and idempotency guard.
///
/// Sweep flow:
/// 1. Check BNB balance → fund gas if needed (idempotent)
/// 2. Query full USDT balance → build ERC-20 transfer → sign → broadcast
///
/// Gas leak prevention: Before funding, check if child address already has
/// enough BNB for the sweep. This handles crash recovery gracefully.
pub struct EvmSweepExecutor {
    evm_client: Arc<EvmClient>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
    /// Shared gas funder (provides nonce-safe native gas funding)
    gas_funder: Arc<EvmGasFunder>,
    /// Token decimals on this chain (18 for BSC, 6 for Ethereum)
    token_decimals: u8,
}

impl EvmSweepExecutor {
    pub fn new(
        evm_client: Arc<EvmClient>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
        gas_funder: Arc<EvmGasFunder>,
        token_decimals: u8,
    ) -> Self {
        Self {
            evm_client,
            signer,
            gas_funder,
            token_decimals,
        }
    }

    /// Normalize token balance from chain-native decimals to 6-decimal i64.
    ///
    /// For BSC USDT (18 decimals): divides by 10^12
    /// For ETH USDT (6 decimals): no-op
    fn normalize_to_6_decimals(&self, balance: alloy_primitives::U256) -> i64 {
        if self.token_decimals <= 6 {
            // Already ≤ 6 decimals, no normalization needed
            balance.to_string().parse::<i64>().unwrap_or(0)
        } else {
            let shift = (self.token_decimals - 6) as u32;
            let divisor =
                alloy_primitives::U256::from(10u64).pow(alloy_primitives::U256::from(shift));
            let normalized = balance / divisor;
            normalized.to_string().parse::<i64>().unwrap_or(0)
        }
    }
}

#[async_trait]
impl SweepExecutor for EvmSweepExecutor {
    async fn get_balance(&self, address: &str, token_contract: &str) -> Result<i64> {
        let balance = self
            .evm_client
            .get_token_balance(address, token_contract)
            .await?;
        // Normalize to 6-decimal i64 (system standard)
        Ok(self.normalize_to_6_decimals(balance))
    }

    async fn execute_sweep(
        &self,
        from_address: &str,
        to_address: &str,
        account_index: i32,
        path_index: u32,
        token_contract: &str,
        outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<SweepResult> {
        use crate::services::chain::types::ChainUnsignedTx;
        use crate::services::evm::signing::{assemble_signed_tx, rlp_encode_for_signing};
        use tracing::info;

        // Step 1: Get full USDT balance (before funding, to estimate gas accurately)
        let usdt_balance = self
            .evm_client
            .get_token_balance(from_address, token_contract)
            .await?;

        if usdt_balance.is_zero() {
            return Err(anyhow::anyhow!("No token funds to sweep"));
        }

        // Step 2: Estimate gas for the ERC-20 transfer, then fund accordingly
        let gas_limit = self
            .evm_client
            .estimate_token_transfer_gas(from_address, to_address, token_contract, usdt_balance)
            .await;
        let funding = self
            .gas_funder
            .ensure_gas(from_address, gas_limit, outbound_id, outbound_store)
            .await?;

        info!(
            from = %from_address,
            to = %to_address,
            balance = %usdt_balance,
            "Building ERC-20 token sweep transaction"
        );

        let unsigned_tx = match ChainClient::build_token_transfer(
            self.evm_client.as_ref(),
            from_address,
            to_address,
            token_contract,
            usdt_balance,
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

        tracing::debug!(tx_hash = %signed.tx_hash, "Broadcasting token sweep transaction");

        let broadcast = ChainClient::broadcast(
            self.evm_client.as_ref(),
            &crate::services::chain::types::ChainSignedTx::Evm(signed),
        )
        .await;

        let (broadcast_disposition, broadcast_error) = match broadcast {
            Ok(result) if result.success && result.tx_hash.eq_ignore_ascii_case(&local_tx_hash) => {
                (BroadcastDisposition::Accepted, None)
            }
            Ok(result) => (
                BroadcastDisposition::Unknown,
                Some(format!(
                    "EVM broadcast acknowledgement mismatch for {}: {:?}",
                    local_tx_hash, result.message
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
            tracing::error!(outbound_id, error = %error, "Failed to persist EVM broadcast outcome");
        }

        let amount_swept = self.normalize_to_6_decimals(usdt_balance);

        Ok(SweepResult {
            tx_hash: local_tx_hash,
            funding_tx_hash: funding.funding_tx_hash,
            amount_swept,
            gas_cost_native: funding.gas_cost_native,
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
        match ChainClient::broadcast(
            self.evm_client.as_ref(),
            &crate::services::chain::types::ChainSignedTx::Evm(signed),
        )
        .await
        {
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

    async fn check_tx_status(
        &self,
        tx_hash: &str,
        required_confirmations: i32,
    ) -> Result<SweepTxStatus> {
        match ChainClient::get_transaction_info(self.evm_client.as_ref(), tx_hash).await {
            Ok(Some(info)) => {
                if !info.success {
                    return Ok(SweepTxStatus::Failed);
                }
                // Check confirmation depth
                if required_confirmations > 0 {
                    let current_block = self.get_current_block().await.unwrap_or(0);
                    let confirmations = current_block - info.block_number;
                    if confirmations < required_confirmations as i64 {
                        return Ok(SweepTxStatus::Pending);
                    }
                }
                Ok(SweepTxStatus::Confirmed)
            }
            Ok(None) => Ok(SweepTxStatus::NotFound),
            Err(e) => Err(e),
        }
    }

    async fn get_current_block(&self) -> Result<i64> {
        let info = ChainClient::get_current_block(self.evm_client.as_ref()).await?;
        Ok(info.number as i64)
    }
}
