//! Solana Sweep Executor
//!
//! Implements the `SweepExecutor` trait for Solana SPL Token sweeps.
//!
//! # Key Differences from TRON/EVM
//! - **Fee Payer Delegation**: Treasury pays gas fees, so deposit addresses
//!   don't need SOL. This requires TWO Ed25519 signatures per sweep:
//!   1. Treasury (fee payer) — hardcoded at account=0, path=0
//!   2. Source address (authority) — `account_index` / `path_index`
//! - **No gas funding step**: Unlike EVM's 2-step (fund BNB → sweep USDT),
//!   Solana sweeps are single-transaction.
//! - **ATA derivation**: Balance queries require computing the Associated Token
//!   Account (ATA) from the owner address + mint.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{debug, info};

use crate::services::address::key_provider::TransactionSigner;
use crate::services::outbound::{
    BroadcastDisposition, OutboundTransactionStore, RecoveryDisposition, StoredSignedTransaction,
};
use crate::services::solana::signing::assemble_signed_solana_tx;
use crate::services::solana::{derive_ata_address, SolanaClient, SPL_TOKEN_PROGRAM_ID};
use crate::services::sweeper::executor::{SweepExecutor, SweepResult, SweepTxStatus};

/// Solana sweep executor with Fee Payer Delegation (dual-signer).
///
/// Sweep flow:
/// 1. Derive ATA for source address → get SPL Token balance
/// 2. `build_spl_sweep` (from → treasury, fee_payer = treasury)
/// 3. Sign with treasury key (fee payer, first signature)
/// 4. Sign with source key (authority, second signature)
/// 5. Assemble + broadcast via `send_with_retry_loop`
///
/// Treasury is always HD-derived at `m/44'/501'/0'/0'` (hardcoded,
/// matching TRON/EVM pattern of account_index=0, path_index=0).
pub struct SolanaSweepExecutor {
    solana_client: Arc<SolanaClient>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
    /// Treasury address (Base58) — receives swept funds AND pays fees
    treasury_address: String,
    /// Token decimals keyed by mint address (e.g., USDT mint → 6)
    token_decimals: HashMap<String, u8>,
}

/// Treasury HD derivation: account_index=0, path_index=0
/// (same convention as TRON m/44'/195'/0'/0/0 and EVM m/44'/60'/0'/0/0)
const TREASURY_ACCOUNT_INDEX: i32 = 0;
const TREASURY_PATH_INDEX: u32 = 0;

impl SolanaSweepExecutor {
    pub fn new(
        solana_client: Arc<SolanaClient>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
        treasury_address: String,
        token_decimals: HashMap<String, u8>,
    ) -> Self {
        Self {
            solana_client,
            signer,
            treasury_address,
            token_decimals,
        }
    }

    /// Get token decimals by mint address.
    fn decimals_for_mint(&self, mint_address: &str) -> Result<u8> {
        self.token_decimals
            .get(mint_address)
            .copied()
            .ok_or_else(|| {
                anyhow!(
                    "Unknown Solana token mint: {}. Configured mints: {:?}",
                    mint_address,
                    self.token_decimals.keys().collect::<Vec<_>>()
                )
            })
    }
}

/// Solana coin type for SLIP-0010 / BIP44 derivation (Ed25519).
const SOLANA_COIN_TYPE: u32 = 501;

/// Normalize SPL Token raw amount to 6-decimal i64 (system standard).
///
/// Handles all cases:
/// - `decimals == 6`: pass-through (USDT, USDC)
/// - `decimals > 6`: divide by `10^(decimals - 6)`
/// - `decimals < 6`: multiply by `10^(6 - decimals)`
fn normalize_spl_amount(raw_amount: u64, decimals: u8) -> i64 {
    let result = match decimals.cmp(&6) {
        std::cmp::Ordering::Equal => raw_amount as i128,
        std::cmp::Ordering::Greater => {
            let divisor = 10u64.pow((decimals - 6) as u32);
            (raw_amount / divisor) as i128
        }
        std::cmp::Ordering::Less => {
            let multiplier = 10u64.pow((6 - decimals) as u32);
            raw_amount as i128 * multiplier as i128
        }
    };
    // Clamp to i64 range (defensive — real amounts won't hit this)
    result.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

#[async_trait]
impl SweepExecutor for SolanaSweepExecutor {
    /// Get SPL Token balance at the given address (normalized to 6-decimal i64).
    ///
    /// `token_contract` is the SPL token mint address (Base58).
    async fn get_balance(&self, address: &str, token_contract: &str) -> Result<i64> {
        let decimals = self.decimals_for_mint(token_contract)?;

        // Derive ATA for the owner + mint
        let ata = derive_ata_address(address, token_contract, SPL_TOKEN_PROGRAM_ID)?;

        match self.solana_client.get_token_account_balance(&ata).await? {
            Some(raw_amount) => Ok(normalize_spl_amount(raw_amount, decimals)),
            None => Ok(0), // ATA doesn't exist = zero balance
        }
    }

    /// Execute a full SPL Token sweep with Fee Payer Delegation.
    ///
    /// Dual-sign flow:
    /// 1. Treasury signs as fee payer (first signature in wire format)
    /// 2. Source address signs as token authority (second signature)
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
        let decimals = self.decimals_for_mint(token_contract)?;

        // 1. Get full token balance (via ATA)
        let ata = derive_ata_address(from_address, token_contract, SPL_TOKEN_PROGRAM_ID)?;
        let balance = self
            .solana_client
            .get_token_account_balance(&ata)
            .await?
            .unwrap_or(0);

        if balance == 0 {
            return Err(anyhow!("No SPL token funds to sweep"));
        }

        info!(
            from = %from_address,
            to = %to_address,
            balance,
            mint = %token_contract,
            "Building Solana SPL Token sweep (Fee Payer Delegation)"
        );

        // 2. Build unsigned sweep transaction
        //    fee_payer = treasury, authority = from_address
        //    close_ata = false (reusable deposit addresses)
        let unsigned_tx = self
            .solana_client
            .build_spl_sweep(
                from_address,
                to_address,
                token_contract,
                balance,
                decimals,
                &self.treasury_address,
                SPL_TOKEN_PROGRAM_ID,
                false, // Don't close ATA — addresses are reusable
            )
            .await?;

        debug!(
            num_signers = unsigned_tx.num_required_signatures,
            signer_pubkeys = ?unsigned_tx.signer_pubkeys,
            "Unsigned sweep transaction built"
        );

        // 3. Sign with BOTH keys (order matches signer_pubkeys: treasury first, then source)
        //    This is the Fee Payer Delegation pattern:
        //    - Signature 0 = fee payer (treasury) — MUST be first per Solana wire format
        //    - Signature 1 = authority (source deposit address)
        let sig_treasury = self
            .signer
            .sign_transaction_for_coin(
                &unsigned_tx.message_bytes,
                TREASURY_ACCOUNT_INDEX,
                TREASURY_PATH_INDEX,
                SOLANA_COIN_TYPE,
            )
            .await?;

        let sig_source = self
            .signer
            .sign_transaction_for_coin(
                &unsigned_tx.message_bytes,
                account_index,
                path_index,
                SOLANA_COIN_TYPE,
            )
            .await?;

        // 4. Assemble signed transaction (signatures in signer_pubkeys order)
        let signed_tx = assemble_signed_solana_tx(&unsigned_tx, &[sig_treasury, sig_source])?;

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

        debug!(
            tx_hash = %signed_tx.signature,
            "Broadcasting Solana sweep transaction"
        );

        // 5. Broadcast with retry loop (Solana has no mempool — needs aggressive retry)
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
                    "Solana broadcast acknowledgement mismatch for {}: {:?}",
                    signed_tx.signature, result.message
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
            tracing::error!(outbound_id, error = %error, "Failed to persist Solana broadcast outcome");
        }

        let amount_swept = normalize_spl_amount(balance, decimals);

        Ok(SweepResult {
            tx_hash: signed_tx.signature,
            funding_tx_hash: None, // No separate funding tx — fee payer delegation
            amount_swept,
            gas_cost_native: 0, // Solana fees are negligible (~5000 lamports ≈ $0.001)
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
            return Err(anyhow!("Expected stored Solana transaction"));
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

    /// Check transaction confirmation status via `getSignatureStatuses`.
    async fn check_tx_status(
        &self,
        tx_hash: &str,
        _required_confirmations: i32,
    ) -> Result<SweepTxStatus> {
        let statuses = self
            .solana_client
            .get_signature_statuses(&[tx_hash])
            .await?;

        match statuses.first() {
            Some(Some(status)) => {
                // Check for on-chain error
                if status.err.is_some() {
                    return Ok(SweepTxStatus::Failed);
                }

                // Check confirmation level
                let confirmed = status
                    .confirmation_status
                    .as_deref()
                    .is_some_and(|s| s == "confirmed" || s == "finalized");

                if confirmed {
                    Ok(SweepTxStatus::Confirmed)
                } else {
                    Ok(SweepTxStatus::Pending)
                }
            }
            Some(None) | None => Ok(SweepTxStatus::NotFound),
        }
    }

    /// Get current slot (≈ block number for age detection).
    async fn get_current_block(&self) -> Result<i64> {
        let slot = self.solana_client.get_slot().await?;
        Ok(slot as i64)
    }
}
