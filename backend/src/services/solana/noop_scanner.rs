//! Solana Bridge Scanner for TransactionIndexer.
//!
//! Solana scanning is handled by `SolanaIndexer` (signature-cursor model),
//! NOT by the block-range scanning that `TransactionIndexer` normally uses.
//!
//! This scanner satisfies `TransactionIndexer`'s generic `BlockScanner`
//! requirement so we can reuse its address cache, `process_event` pipeline,
//! and `AddressSyncManager` for Solana — without running a real block scan.
//!
//! The block scan loop inside `TransactionIndexer::start()` becomes
//! effectively idle (polls every 3600s, finds no new blocks).  However,
//! `get_current_block` and `verify_transaction` are real — they hit the
//! Solana RPC so that `check_confirmations` can correctly promote
//! `payment_detected` → `payment_confirmed`.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::services::indexer::scanner::{BlockScanner, IndexerTransferEvent, TxVerificationResult};
use crate::services::solana::SolanaClient;

/// A bridge scanner that delegates slot queries and TX verification to
/// the real Solana RPC client, but performs no block-range event scanning.
///
/// Used exclusively for the Solana `TransactionIndexer` instance.
/// All event discovery is done by `SolanaIndexer` — this scanner only
/// provides the confirmation infrastructure.
pub struct SolanaBridgeScanner {
    client: Arc<SolanaClient>,
}

impl SolanaBridgeScanner {
    pub fn new(client: Arc<SolanaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl BlockScanner for SolanaBridgeScanner {
    /// Return the current Solana slot so `check_confirmations` can calculate
    /// how many slots have elapsed since a transaction was included.
    async fn get_current_block(&self) -> Result<i64> {
        let slot = self.client.get_slot().await?;
        Ok(slot as i64)
    }

    /// No block-range scanning — all events come via `ingest_external_event`.
    async fn scan_block(&self, _block_number: i64) -> Result<Vec<IndexerTransferEvent>> {
        Ok(vec![])
    }

    /// Verify a Solana transaction via RPC to guard against ghost transactions.
    async fn verify_transaction(&self, tx_hash: &str) -> Result<TxVerificationResult> {
        match self.client.get_transaction(tx_hash).await? {
            Some(tx) => {
                let success = tx.meta.as_ref().is_some_and(|m| m.err.is_none());
                if success {
                    Ok(TxVerificationResult::Success)
                } else {
                    let reason = tx
                        .meta
                        .as_ref()
                        .and_then(|m| m.err.as_ref())
                        .map(|e| format!("{}", e))
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Ok(TxVerificationResult::Failed(reason))
                }
            }
            None => Ok(TxVerificationResult::NotFound),
        }
    }

    fn required_confirmations(&self) -> i32 {
        31 // Solana finality (~confirmed commitment, ~12s)
    }

    /// No safety lag needed — Solana event detection is done by `SolanaIndexer`,
    /// not by block scanning. This prevents the Bridge from endlessly trying
    /// to "catch up" slots (each a no-op `scan_block`) every cycle.
    fn safety_lag_blocks(&self) -> i64 {
        0
    }

    /// Large range so the Bridge catches up in a single iteration per cycle
    /// instead of looping 200 times at max_block_range=10 (the default).
    /// Since `scan_block` is a no-op, this just fast-forwards the cursor.
    fn max_block_range(&self) -> i64 {
        1_000_000_000 // Large enough to catch up in one iteration, safe from overflow
    }

    /// Block scan loop interval. Controls how often `check_confirmations` runs.
    /// 60s balances confirmation latency with Helius RPC credit cost (~1,440 getSlot/day
    /// per instance vs 8,640/day at 10s). Acceptable because:
    /// - Payment *detection* runs at 3s via SolanaIndexer (independent of this)
    /// - Confirmation promotion (detected→confirmed) can tolerate 60s latency
    /// - Most merchants see "Payment Detected" in dashboard before confirmation anyway
    fn poll_interval(&self) -> Duration {
        Duration::from_secs(60)
    }

    fn usdt_decimals(&self) -> u8 {
        6 // USDT on Solana has 6 decimals
    }
}
