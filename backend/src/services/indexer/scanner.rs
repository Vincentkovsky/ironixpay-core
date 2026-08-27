//! Block Scanner Trait and Implementations
//!
//! Abstracts chain-specific block scanning behind a common trait.
//! - `TronBlockScanner`: Uses TronGrid event API (polling block events)
//! - `EvmBlockScanner`: Uses `eth_getLogs` with Transfer topic (log-based)

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Chain-agnostic transfer event detected by the block scanner.
///
/// The `amount` field is a String to preserve full precision across chains:
/// - TRON USDT: 6 decimals (e.g., "1000000" = 1 USDT)
/// - BSC USDT: 18 decimals (e.g., "1000000000000000000" = 1 USDT)
///
/// Normalization to the DB's i64 (6-decimal) representation happens
/// in the indexer service, NOT here.
#[derive(Debug, Clone)]
pub struct IndexerTransferEvent {
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    /// Raw amount string from the chain (preserves full precision)
    pub amount: String,
    pub event_index: i32,
    pub block_number: i64,
    pub block_timestamp: i64,
    /// Token symbol (e.g., "USDT", "USDC") — set by scanner from watchlist
    pub token: String,
}

/// Result of verifying a transaction's on-chain status.
#[derive(Debug, Clone)]
pub enum TxVerificationResult {
    /// Transaction confirmed successfully
    Success,
    /// Transaction confirmed but failed (e.g., OUT_OF_ENERGY, REVERT)
    Failed(String),
    /// Transaction not found on chain (ghost/dropped)
    NotFound,
}

/// Chain-agnostic block scanner interface.
///
/// Each chain implements this trait to provide transfer event detection.
/// The indexer service calls these methods without knowing chain specifics.
#[async_trait]
pub trait BlockScanner: Send + Sync {
    /// Safety lag: how many blocks behind chain head to scan.
    /// Gives the RPC provider (e.g. TronGrid) time to complete event indexing
    /// before the indexer scans those blocks. Without this, the indexer can
    /// scan a block before events are indexed, causing missed payments.
    /// Default: same as required_confirmations (TRON=20 ~60s, BSC=15 ~45s).
    fn safety_lag_blocks(&self) -> i64 {
        self.required_confirmations() as i64
    }

    /// Get the current (latest) block number on this chain.
    async fn get_current_block(&self) -> Result<i64>;

    /// Scan a single block for USDT Transfer events.
    ///
    /// Returns ALL Transfer events for the configured USDT contract.
    /// The caller (indexer service) is responsible for filtering by address.
    async fn scan_block(&self, block_number: i64) -> Result<Vec<IndexerTransferEvent>>;

    /// Verify a transaction's on-chain status (for confirmation checking).
    async fn verify_transaction(&self, tx_hash: &str) -> Result<TxVerificationResult>;

    /// Number of confirmations required for finality on this chain.
    fn required_confirmations(&self) -> i32;

    /// Polling interval between block scans.
    fn poll_interval(&self) -> Duration;

    /// USDT decimal places for this chain (6 for TRON, 18 for BSC).
    /// Used by the indexer to normalize amounts to i64 (6-decimal) for DB storage.
    fn usdt_decimals(&self) -> u8;

    /// Max block range per `eth_getLogs` / scan_block_range call.
    /// Defaults to 10 (TRON per-block scanning). EVM chains override dynamically
    /// based on the active RPC provider (Alchemy=10, Ankr=1000).
    fn max_block_range(&self) -> i64 {
        10
    }

    /// Scan a range of blocks for USDT Transfer events in a single call.
    ///
    /// Default implementation falls back to per-block `scan_block()` calls.
    /// EVM chains override this with a single `eth_getLogs(fromBlock, toBlock)`
    /// call, reducing RPC usage from N to 1 per cycle.
    async fn scan_block_range(
        &self,
        from_block: i64,
        to_block: i64,
    ) -> Result<Vec<IndexerTransferEvent>> {
        let mut all_events = Vec::new();
        for block in from_block..=to_block {
            all_events.extend(self.scan_block(block).await?);
        }
        Ok(all_events)
    }
}

// ─── TRON Block Scanner ────────────────────────────────────────────────────

use crate::services::tron::TronClient;

/// TRON block scanner using TronGrid's event API.
///
/// Wraps the existing `TronClient.get_block_events()` method.
pub struct TronBlockScanner {
    tron_client: Arc<TronClient>,
    /// Contract→Symbol mapping (e.g., [("TR7NHqj...", "USDT")])
    watchlist: Vec<(String, String)>,
}

impl TronBlockScanner {
    pub fn new(tron_client: Arc<TronClient>, watchlist: Vec<(String, String)>) -> Self {
        Self {
            tron_client,
            watchlist,
        }
    }
}

#[async_trait]
impl BlockScanner for TronBlockScanner {
    async fn get_current_block(&self) -> Result<i64> {
        let block_info = self.tron_client.get_current_block().await?;
        Ok(block_info.number as i64)
    }

    async fn scan_block(&self, block_number: i64) -> Result<Vec<IndexerTransferEvent>> {
        let events = self
            .tron_client
            .get_block_events(block_number, false)
            .await?;

        let mut transfers = Vec::new();
        for event in events {
            // Only Transfer events
            if event.event_name != "Transfer" {
                continue;
            }

            // Match against watchlist (all monitored token contracts)
            let token_symbol = match self
                .watchlist
                .iter()
                .find(|(contract, _)| *contract == event.contract_address)
            {
                Some((_, symbol)) => symbol.clone(),
                None => continue, // Not a monitored token
            };

            let to = match event.result.get("to") {
                Some(addr) => addr.clone(),
                None => continue,
            };
            let from = event.result.get("from").cloned().unwrap_or_default();
            let amount = event.result.get("value").cloned().unwrap_or_default();

            transfers.push(IndexerTransferEvent {
                tx_hash: event.transaction_id,
                from,
                to,
                amount,
                event_index: event.event_index,
                block_number: event.block_number,
                block_timestamp: event.block_timestamp,
                token: token_symbol,
            });
        }

        Ok(transfers)
    }

    fn required_confirmations(&self) -> i32 {
        19 // TRON: 19 blocks (~57 seconds)
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(6) // TRON: ~1 block/3s, 6s interval covers ~2 blocks
    }

    async fn verify_transaction(&self, tx_hash: &str) -> Result<TxVerificationResult> {
        match self.tron_client.get_transaction_info(tx_hash).await {
            Ok(Some(info)) => {
                if info.success {
                    Ok(TxVerificationResult::Success)
                } else {
                    Ok(TxVerificationResult::Failed(
                        info.result.unwrap_or_else(|| "Unknown failure".to_string()),
                    ))
                }
            }
            Ok(None) => Ok(TxVerificationResult::NotFound),
            Err(e) => Err(e),
        }
    }

    fn usdt_decimals(&self) -> u8 {
        6
    }
}

// ─── EVM Block Scanner ─────────────────────────────────────────────────────

use crate::services::chain::traits::ChainClient;
use crate::services::evm::EvmClient;

/// EVM block scanner using `eth_getLogs` with Transfer topic.
///
/// Parses ERC-20 Transfer(address,address,uint256) events from logs.
/// All chain-specific parameters (decimals, confirmations, poll interval)
/// are injected at construction time from `ChainConfig`.
pub struct EvmBlockScanner {
    evm_client: Arc<EvmClient>,
    /// Contract→Symbol mapping (e.g., [("0xdAC17F...", "USDT"), ("0xA0b869...", "USDC")])
    watchlist: Vec<(String, String)>,
    /// Default USDT decimals on this chain (backward compat for usdt_decimals())
    default_decimals: u8,
    /// Required confirmations for finality
    confirmations: i32,
    /// Polling interval between block scans
    interval: Duration,
}

impl EvmBlockScanner {
    pub fn new(
        evm_client: Arc<EvmClient>,
        watchlist: Vec<(String, String)>,
        default_decimals: u8,
        confirmations: i32,
        interval: Duration,
    ) -> Self {
        Self {
            evm_client,
            watchlist,
            default_decimals,
            confirmations,
            interval,
        }
    }

    /// Parse an address from a 32-byte hex topic (zero-padded).
    /// Input: "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b"
    /// Output: "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B" (EIP-55 checksum)
    fn parse_topic_address(topic: &str) -> Result<String> {
        let hex = topic.strip_prefix("0x").unwrap_or(topic);
        if hex.len() != 64 {
            return Err(anyhow::anyhow!("Invalid topic length: {}", hex.len()));
        }
        // Last 20 bytes = last 40 hex chars
        let addr_hex = &hex[24..64];
        let addr_bytes: [u8; 20] = hex::decode(addr_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid address bytes"))?;
        let address = alloy_primitives::Address::from(addr_bytes);
        Ok(address.to_checksum(None))
    }

    /// Parse a uint256 value from hex data.
    /// Input: "0x00000000000000000000000000000000000000000000000000000000000f4240"
    /// Output: "1000000"
    fn parse_data_amount(data: &str) -> Result<String> {
        let hex = data.strip_prefix("0x").unwrap_or(data);
        if hex.is_empty() {
            return Ok("0".to_string());
        }
        let value = alloy_primitives::U256::from_str_radix(hex, 16)
            .map_err(|e| anyhow::anyhow!("Invalid amount hex: {}", e))?;
        Ok(value.to_string())
    }
}

#[async_trait]
impl BlockScanner for EvmBlockScanner {
    async fn get_current_block(&self) -> Result<i64> {
        let block_info = ChainClient::get_current_block(self.evm_client.as_ref()).await?;
        Ok(block_info.number as i64)
    }

    async fn scan_block(&self, block_number: i64) -> Result<Vec<IndexerTransferEvent>> {
        self.scan_block_range(block_number, block_number).await
    }

    /// Override: single `eth_getLogs(fromBlock, toBlock)` for the entire range.
    /// Reduces N per-block RPC calls to 1, critical for multi-chain scaling.
    /// Queries ALL monitored token contracts in a single RPC call.
    async fn scan_block_range(
        &self,
        from_block: i64,
        to_block: i64,
    ) -> Result<Vec<IndexerTransferEvent>> {
        let contract_addrs: Vec<&str> = self.watchlist.iter().map(|(c, _)| c.as_str()).collect();
        let logs = self
            .evm_client
            .get_logs(from_block as u64, to_block as u64, &contract_addrs)
            .await?;

        self.parse_transfer_logs_with_token(logs)
    }

    fn required_confirmations(&self) -> i32 {
        self.confirmations
    }

    fn poll_interval(&self) -> Duration {
        self.interval
    }

    fn max_block_range(&self) -> i64 {
        self.evm_client.max_block_range() as i64
    }

    async fn verify_transaction(&self, tx_hash: &str) -> Result<TxVerificationResult> {
        match ChainClient::get_transaction_info(self.evm_client.as_ref(), tx_hash).await {
            Ok(Some(info)) => {
                if info.success {
                    Ok(TxVerificationResult::Success)
                } else {
                    let reason = info
                        .revert_message
                        .or(info.result)
                        .unwrap_or_else(|| "Unknown failure".to_string());
                    Ok(TxVerificationResult::Failed(reason))
                }
            }
            Ok(None) => Ok(TxVerificationResult::NotFound),
            Err(e) => Err(e),
        }
    }

    fn usdt_decimals(&self) -> u8 {
        self.default_decimals
    }
}

impl EvmBlockScanner {
    /// Parse raw EVM logs into IndexerTransferEvent structs with token awareness.
    /// Uses the log's contract address to look up the token symbol from the watchlist.
    fn parse_transfer_logs_with_token(
        &self,
        logs: Vec<crate::services::evm::EvmLog>,
    ) -> Result<Vec<IndexerTransferEvent>> {
        let mut transfers = Vec::new();
        for log in logs {
            // Transfer event has 3 topics: [event_sig, from, to]
            if log.topics.len() < 3 {
                continue;
            }

            // Skip removed logs (reorged)
            if log.removed.unwrap_or(false) {
                continue;
            }

            // Look up token from log.address (contract that emitted the event)
            let token_symbol = match self
                .watchlist
                .iter()
                .find(|(contract, _)| contract.eq_ignore_ascii_case(&log.address))
            {
                Some((_, symbol)) => symbol.clone(),
                None => continue, // Shouldn't happen since we filtered in get_logs
            };

            // Skip logs with null metadata (pending blocks or reorg windows)
            let tx_hash = match log.transaction_hash {
                Some(h) => h,
                None => continue,
            };
            let block_number_hex = match &log.block_number {
                Some(b) => b.as_str(),
                None => continue,
            };
            let log_index_hex = match &log.log_index {
                Some(l) => l.as_str(),
                None => continue,
            };

            let from = match Self::parse_topic_address(&log.topics[1]) {
                Ok(addr) => addr,
                Err(_) => continue,
            };
            let to = match Self::parse_topic_address(&log.topics[2]) {
                Ok(addr) => addr,
                Err(_) => continue,
            };
            let amount = match Self::parse_data_amount(&log.data) {
                Ok(amt) => amt,
                Err(_) => continue,
            };

            let block_num = EvmClient::parse_hex_i64(block_number_hex).unwrap_or(0);
            let log_index = EvmClient::parse_hex_i64(log_index_hex).unwrap_or(0) as i32;

            transfers.push(IndexerTransferEvent {
                tx_hash,
                from,
                to,
                amount,
                event_index: log_index,
                block_number: block_num,
                // EVM logs don't include block timestamp; set 0 and resolve in indexer
                block_timestamp: 0,
                token: token_symbol,
            });
        }
        Ok(transfers)
    }

    /// Static helper for backward compatibility (used by tests).
    /// Parses Transfer logs without token awareness — sets token from log.address.
    pub fn parse_transfer_logs(
        logs: Vec<crate::services::evm::EvmLog>,
    ) -> Result<Vec<IndexerTransferEvent>> {
        let mut transfers = Vec::new();
        for log in logs {
            if log.topics.len() < 3 {
                continue;
            }
            if log.removed.unwrap_or(false) {
                continue;
            }

            let tx_hash = match log.transaction_hash {
                Some(h) => h,
                None => continue,
            };
            let block_number_hex = match &log.block_number {
                Some(b) => b.as_str(),
                None => continue,
            };
            let log_index_hex = match &log.log_index {
                Some(l) => l.as_str(),
                None => continue,
            };

            let from = match Self::parse_topic_address(&log.topics[1]) {
                Ok(addr) => addr,
                Err(_) => continue,
            };
            let to = match Self::parse_topic_address(&log.topics[2]) {
                Ok(addr) => addr,
                Err(_) => continue,
            };
            let amount = match Self::parse_data_amount(&log.data) {
                Ok(amt) => amt,
                Err(_) => continue,
            };

            let block_num = EvmClient::parse_hex_i64(block_number_hex).unwrap_or(0);
            let log_idx = EvmClient::parse_hex_i64(log_index_hex).unwrap_or(0) as i32;

            transfers.push(IndexerTransferEvent {
                tx_hash,
                from,
                to,
                amount,
                event_index: log_idx,
                block_number: block_num,
                block_timestamp: 0,
                token: log.address.clone(),
            });
        }
        Ok(transfers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_topic_address() {
        let topic = "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b";
        let addr = EvmBlockScanner::parse_topic_address(topic).unwrap();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
        assert_eq!(
            addr.to_lowercase(),
            "0xab5801a7d398351b8be11c439e05c5b3259aec9b"
        );
    }

    #[test]
    fn test_parse_data_amount() {
        // 1 USDT with 6 decimals = 1000000
        let data = "0x00000000000000000000000000000000000000000000000000000000000f4240";
        let amount = EvmBlockScanner::parse_data_amount(data).unwrap();
        assert_eq!(amount, "1000000");
    }

    #[test]
    fn test_parse_data_amount_large() {
        // 1 USDT with 18 decimals = 1000000000000000000
        let data = "0x0000000000000000000000000000000000000000000000000de0b6b3a7640000";
        let amount = EvmBlockScanner::parse_data_amount(data).unwrap();
        assert_eq!(amount, "1000000000000000000");
    }

    #[test]
    fn test_parse_data_amount_empty() {
        let amount = EvmBlockScanner::parse_data_amount("0x").unwrap();
        assert_eq!(amount, "0");
    }
}
