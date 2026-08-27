//! EVM Gas Funder — shared service for gas-sponsoring ERC-20 operations.
//!
//! Extracts the common "check native balance → fund deficit from gas sponsor →
//! wait for confirmation" flow used by both EvmSweepExecutor and EvmPayoutExecutor.
//!
//! Includes a per-instance `tokio::sync::Mutex` to serialize gas sponsor nonce
//! usage, preventing nonce conflicts when sweep and payout happen concurrently
//! on the same chain.

use crate::services::address::key_provider::TransactionSigner;
use crate::services::chain::traits::ChainClient;
use crate::services::chain::types::{ChainSignedTx, ChainUnsignedTx};
use crate::services::evm::signing::{assemble_signed_tx, rlp_encode_for_signing};
use crate::services::evm::EvmClient;
use crate::services::outbound::{
    BroadcastDisposition, OutboundTransactionStore, StoredSignedTransaction,
};
use alloy_primitives::U256;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Result of a gas funding attempt.
#[derive(Debug)]
pub struct GasFundingResult {
    /// Transaction hash of the native-token funding tx (None if funding was skipped)
    pub funding_tx_hash: Option<String>,
    /// Gas cost in native token's smallest unit (Wei).
    /// If funding occurred: the deficit we sent.
    /// If skipped: estimated gas cost (gasLimit × gasPrice × 1.5).
    pub gas_cost_native: u64,
}

/// Shared EVM gas funder with per-instance nonce serialization.
///
/// One instance per EVM chain. Both sweep and payout executors share the same
/// `EvmGasFunder`, which ensures gas sponsor transactions are never concurrent.
pub struct EvmGasFunder {
    evm_client: Arc<EvmClient>,
    signer: Arc<dyn TransactionSigner + Send + Sync>,
    gas_sponsor_address: String,
    gas_sponsor_account_index: i32,
    gas_sponsor_path_index: u32,
    /// Serializes gas sponsor nonce: get_nonce → sign → broadcast.
    /// Confirmation polling happens OUTSIDE this lock.
    sponsor_lock: Mutex<()>,
}

impl EvmGasFunder {
    pub fn new(
        evm_client: Arc<EvmClient>,
        signer: Arc<dyn TransactionSigner + Send + Sync>,
        gas_sponsor_address: String,
        gas_sponsor_account_index: i32,
        gas_sponsor_path_index: u32,
    ) -> Self {
        Self {
            evm_client,
            signer,
            gas_sponsor_address,
            gas_sponsor_account_index,
            gas_sponsor_path_index,
            sponsor_lock: Mutex::new(()),
        }
    }

    /// Ensure the target address has enough native gas for an ERC-20 transfer.
    ///
    /// Idempotent: if the target already has sufficient native balance, this is a no-op.
    /// Nonce-safe: gas sponsor transactions are serialized via internal Mutex.
    ///
    /// # Arguments
    /// * `target_address` — The address that needs gas (child/treasury)
    /// * `gas_limit` — Gas limit for the upcoming token transfer (default: 65,000)
    pub async fn ensure_gas(
        &self,
        target_address: &str,
        gas_limit: u64,
        parent_outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<GasFundingResult> {
        // 1. Check target's native balance
        let native_balance =
            ChainClient::get_native_balance(self.evm_client.as_ref(), target_address).await?;
        let gas_price = self.evm_client.get_gas_price().await?;
        // +20% buffer to match the buffered gas_price used in build_native_transfer/build_token_transfer
        let gas_price = gas_price + gas_price / 5;

        // Required gas = gasLimit × gasPrice × 1.5 (safety buffer)
        let base_cost = U256::from(gas_limit) * U256::from(gas_price);
        let required_gas = base_cost + base_cost / U256::from(2u64);

        // 2. If sufficient, skip funding
        if native_balance >= required_gas {
            debug!(
                target = %target_address,
                balance = %native_balance,
                "Native gas sufficient, skipping funding"
            );
            return Ok(GasFundingResult {
                funding_tx_hash: None,
                gas_cost_native: required_gas.to::<u64>(),
            });
        }

        // 3. Fund the deficit — lock gas sponsor nonce for atomicity
        let deficit = required_gas - native_balance;
        let deficit_u64 = deficit.to::<u64>();
        let funding_outbound = outbound_store
            .create_child_attempt(
                parent_outbound_id,
                crate::entity::outbound_transactions::OutboundPurpose::GasFunding,
                self.gas_sponsor_address.clone(),
                target_address.to_string(),
                i64::try_from(deficit_u64)
                    .map_err(|_| anyhow::anyhow!("Gas funding amount exceeds i64"))?,
                "NATIVE".to_string(),
            )
            .await?;

        info!(
            target = %target_address,
            sponsor = %self.gas_sponsor_address,
            deficit = %deficit,
            gas_price,
            "Funding native gas for EVM operation"
        );

        let (fund_tx_hash, gas_cost) = {
            // Lock scope: get_nonce → sign → broadcast (~1s)
            let _guard = self.sponsor_lock.lock().await;

            let unsigned_tx = match ChainClient::build_native_transfer(
                self.evm_client.as_ref(),
                &self.gas_sponsor_address,
                target_address,
                deficit,
            )
            .await?
            {
                ChainUnsignedTx::Evm(tx) => tx,
                _ => return Err(anyhow::anyhow!("Expected EVM unsigned tx")),
            };

            let rlp_bytes = rlp_encode_for_signing(&unsigned_tx);
            let signature = self
                .signer
                .sign_transaction_for_coin(
                    &rlp_bytes,
                    self.gas_sponsor_account_index,
                    self.gas_sponsor_path_index,
                    60, // EVM coin type
                )
                .await?;
            let signed = assemble_signed_tx(&unsigned_tx, &signature)?;
            let local_tx_hash = signed.tx_hash.clone();

            outbound_store
                .record_signed(
                    &funding_outbound.id,
                    &StoredSignedTransaction::Evm {
                        tx_hash: local_tx_hash.clone(),
                        raw_tx_hex: signed.raw_tx_hex.clone(),
                        from_address: unsigned_tx.from.clone(),
                        nonce: unsigned_tx.nonce,
                    },
                )
                .await?;

            debug!(tx_hash = %signed.tx_hash, "Broadcasting native gas funding tx");

            let broadcast =
                ChainClient::broadcast(self.evm_client.as_ref(), &ChainSignedTx::Evm(signed)).await;
            let (disposition, error) = match broadcast {
                Ok(result)
                    if result.success && result.tx_hash.eq_ignore_ascii_case(&local_tx_hash) =>
                {
                    (BroadcastDisposition::Accepted, None)
                }
                Ok(result) => (
                    BroadcastDisposition::Unknown,
                    Some(format!(
                        "Gas funding acknowledgement mismatch: {:?}",
                        result.message
                    )),
                ),
                Err(error) => (
                    BroadcastDisposition::Unknown,
                    Some(format!("Gas funding broadcast was ambiguous: {error}")),
                ),
            };
            if let Err(error) = outbound_store
                .mark_broadcast(&funding_outbound.id, disposition, error)
                .await
            {
                tracing::error!(outbound_id = %funding_outbound.id, error = %error, "Failed to persist gas funding broadcast outcome");
            }

            // Total cost = deficit (ETH sent to target) + funding tx gas (21000 × gasPrice)
            // The gas_sponsor pays 21000 gas for the native ETH transfer itself.
            let funding_tx_gas_cost = U256::from(21_000u64) * U256::from(gas_price);
            (local_tx_hash, (deficit + funding_tx_gas_cost).to::<u64>())
            // _guard dropped here — lock released after broadcast
        };

        // 4. Poll for confirmation OUTSIDE the lock (max 60s, covers Ethereum ~12s blocks)
        let mut confirmed = false;
        for attempt in 0..12 {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            match ChainClient::get_transaction_info(self.evm_client.as_ref(), &fund_tx_hash).await {
                Ok(Some(info)) if info.success => {
                    confirmed = true;
                    let _ = outbound_store
                        .mark_state(
                            &funding_outbound.id,
                            crate::entity::outbound_transactions::OutboundState::Confirmed,
                            None,
                        )
                        .await;
                    info!(tx_hash = %fund_tx_hash, "Gas funding confirmed on-chain");
                    break;
                }
                Ok(Some(info)) if !info.success => {
                    let _ = outbound_store
                        .mark_state(
                            &funding_outbound.id,
                            crate::entity::outbound_transactions::OutboundState::Reverted,
                            Some("Gas funding transaction reverted on-chain".into()),
                        )
                        .await;
                    return Err(anyhow::anyhow!("Gas funding tx failed on-chain"));
                }
                _ => {
                    debug!(attempt, "Waiting for gas funding tx confirmation...");
                }
            }
        }
        if !confirmed {
            return Err(anyhow::anyhow!(
                "Gas funding tx {} not confirmed after 60s — will retry next cycle",
                fund_tx_hash
            ));
        }

        Ok(GasFundingResult {
            funding_tx_hash: Some(fund_tx_hash),
            gas_cost_native: gas_cost,
        })
    }
}
