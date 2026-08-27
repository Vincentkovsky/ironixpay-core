use crate::entity::transactions::ChainTxState;
use crate::services::tron::interface::TronBroadcaster;
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::warn;

/// Buffer time (in milliseconds) to account for clock drift between local system and TRON nodes.
/// This prevents premature expiration detection due to minor time discrepancies.
const EXPIRATION_BUFFER_MS: i64 = 60_000;

pub struct TransactionMonitor {
    tron_client: Arc<dyn TronBroadcaster + Send + Sync>,
}

impl TransactionMonitor {
    pub fn new(tron_client: Arc<dyn TronBroadcaster + Send + Sync>) -> Self {
        Self { tron_client }
    }

    /// Check the status of a transaction on-chain.
    ///
    /// This abstracts the complexities of:
    /// 1. Checking if the transaction exists (get_transaction_by_id)
    /// 2. Checking if it's confirmed/solidified (get_transaction_info_by_id)
    /// 3. Handling API errors gracefully (Propagates errors for retry)
    pub async fn check_tx_status(
        &self,
        tx_hash: &str,
        min_confirmations: u64,
        latest_block_number: Option<u64>,
    ) -> Result<ChainTxState> {
        // 1. Try to get execution info (Receipt)
        // This is only available after the transaction is included in a block
        let info_opt = self
            .tron_client
            .get_transaction_info(tx_hash)
            .await
            .with_context(|| format!("Failed to get transaction info for tx_hash={}", tx_hash))?;

        match info_opt {
            Some(info) => {
                // Transaction is mined and has a receipt

                // Check execution success
                if !info.success {
                    let error_code = info.result.unwrap_or_else(|| "UNKNOWN".to_string());
                    let reason = info
                        .revert_message
                        .unwrap_or_else(|| "No revert message".to_string());
                    warn!(
                        tx_hash,
                        error_code, reason, "Transaction execution failed on-chain"
                    );
                    return Ok(ChainTxState::Failed);
                }

                // Check confirmations if required
                if min_confirmations > 0 {
                    let current_height = match latest_block_number {
                        Some(h) => h,
                        None => {
                            self.tron_client
                                .get_current_block()
                                .await
                                .context("Failed to get current block height")?
                                .number
                        }
                    };

                    // Defensive: ensure block_number is non-negative before casting
                    let block_num = info.block_number.max(0) as u64;
                    let confirmations = current_height.saturating_sub(block_num);

                    if confirmations < min_confirmations {
                        // Included but not enough confirmations
                        return Ok(ChainTxState::Unconfirmed);
                    }
                }

                // Enough confirmations
                Ok(ChainTxState::Confirmed)
            }
            None => {
                // No receipt found. This means either:
                // A. Transaction is Pending (Mempool or just mined but no receipt yet)
                // B. Transaction is Not Found (Invalid hash, expired, or dropped)

                // 2. Fallback: Check for raw transaction existence
                let tx_opt = self
                    .tron_client
                    .get_transaction_by_id(tx_hash)
                    .await
                    .with_context(|| {
                        format!("Failed to get transaction by id for tx_hash={}", tx_hash)
                    })?;

                match tx_opt {
                    Some(tx) => {
                        // 3. Zombie Transaction Trap: Check for expiration
                        // TRON expiration is a timestamp in milliseconds.
                        if let Some(exp) = tx.expiration {
                            // Use local system time as final authority to avoid node lag issues.
                            // Return error if system clock is unavailable - this is critical for correctness.
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .context("System clock error: time went backwards")?
                                .as_millis() as i64;

                            // Buffer to allow for clock drift between local and node
                            if now_ms > (exp + EXPIRATION_BUFFER_MS) {
                                warn!(
                                    tx_hash,
                                    expiration = exp,
                                    now_ms,
                                    "Transaction expired without being mined (Zombie Trap)"
                                );
                                // Return NotFound because an expired transaction is equivalent to never existing.
                                // This allows safe retries without side effects.
                                return Ok(ChainTxState::NotFound);
                            }
                        }

                        // Exists in mempool/chain but no receipt yet -> Pending
                        Ok(ChainTxState::Pending)
                    }
                    None => {
                        // Does not exist at all -> NotFound
                        Ok(ChainTxState::NotFound)
                    }
                }
            }
        }
    }
}
