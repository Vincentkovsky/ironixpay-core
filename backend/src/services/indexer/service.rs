//! Transaction Indexer
//!
//! Monitors the Tron blockchain for incoming USDT payments to ALL system addresses.
//! Uses block-based event scanning for O(1) efficiency per block.
//! Writes to transactions and payment_events tables (Transactional Outbox pattern).
//!
//! Payment Classification:
//! - Normal payments: To addresses with active sessions (Pending/Underpaid)
//! - Exceptions: Late payments, payments to idle addresses, etc.
//!
//! IMPORTANT: This service does NOT modify checkout_sessions.status directly.
//! Session status updates are handled by PaymentEventProcessor.
//!
//! Aligned with docs/system_design.md

use super::MonitoredAddressInfo;

use secrecy::{ExposeSecret, Secret};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::chain_health::ChainHealthRegistry;

use crate::entity::{
    addresses, checkout_sessions, indexer_state, payment_events, payment_exceptions, transactions,
    Addresses, CheckoutSessions, Environment, IndexerState, Network, PaymentExceptions,
    Transactions,
};

use super::scanner::{BlockScanner, IndexerTransferEvent, TxVerificationResult};
use super::sync::AddressSyncManager;

/// Dust filter threshold for active sessions (0.000001 USDT = 1 in raw units)
const DUST_THRESHOLD: i64 = 1;

/// Higher dust threshold for idle addresses (1 USDT = 1000000 in raw units)
/// Prevents spam transactions from creating too many exception records
const IDLE_DUST_THRESHOLD: i64 = 1_000_000;

/// Timeout for individual RPC calls (prevents hung connections from blocking the indexer loop)
const RPC_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Check if indexer should reset to current block (dev mode)
/// Set INDEXER_RESET_START=true to start from current block instead of last_processed_block
fn should_reset_start() -> bool {
    std::env::var("INDEXER_RESET_START")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false)
}

/// Maximum age for pending transactions in memory (1 hour)
/// Transactions older than this are considered stale and removed
const MAX_PENDING_AGE: Duration = Duration::from_secs(3600);

/// Re-scan depth: how many blocks behind `last_processed` to re-scan periodically.
/// TRON=50 blocks (~2.5min), BSC=50 blocks (~2.5min).
/// Kept moderate to control RPC cost (TRON scans per-block, not batched: 50 calls per re-scan).
const RESCAN_DEPTH: i64 = 50;

/// Re-scan interval: how often to trigger periodic re-scan (5 minutes).
/// Low frequency safety net — primary protection is L1 Safety Lag.
const RESCAN_INTERVAL: Duration = Duration::from_secs(5 * 60);

// =============================================================================
// Block Event Fetching: Exponential Backoff "Persistent" Mode
// =============================================================================
// For core block fetching, we use a "never give up" strategy with exponential
// backoff. Transient errors (network, RPC unavailable) are retried indefinitely.
// Only fatal errors (data format errors, invalid responses) cause immediate failure.

/// Initial delay for exponential backoff (1 second)
const BLOCK_FETCH_INITIAL_DELAY: Duration = Duration::from_secs(1);

/// Maximum delay cap for exponential backoff (30 seconds)
const BLOCK_FETCH_MAX_DELAY: Duration = Duration::from_secs(30);

/// Number of consecutive failures before logging at ERROR level (alert threshold)
/// After this many failures, we escalate logging to help with monitoring/alerting
const BLOCK_FETCH_ALERT_THRESHOLD: u32 = 10;

/// Jitter factor for exponential backoff (±10%)
const BLOCK_FETCH_JITTER_FACTOR: f64 = 0.1;

/// Indexer state for tracking processed transactions
#[derive(Default)]
pub struct IndexerStateCache {
    /// Last processed block number
    pub last_block: i64,
    /// Pending transactions awaiting confirmations
    pub pending_confirmations: HashMap<String, PendingTx>,
}

#[derive(Clone, Debug)]
pub struct PendingTx {
    pub network: String,
    pub tx_hash: String,
    pub log_index: i32,
    pub session_id: Option<String>,
    pub amount: i64,
    pub block_number: i64,
    pub confirmations: i32,
    /// Whether this is an exception (no active session)
    pub is_exception: bool,
    /// Merchant ID (for tracking)
    pub merchant_id: Option<String>,
    /// Timestamp when this transaction was added to pending pool
    pub created_at: Instant,
}

/// Transaction Indexer Service
///
/// Responsibilities (per system_design.md):
/// - Scan blockchain for incoming payments (Block-based scanning O(1))
/// - Record transactions in `transactions` table (normal payments)
/// - Record exceptions in `payment_exceptions` table (abnormal payments)
/// - Emit events to `payment_events` table (Transactional Outbox)
/// - Update address status to `Detected`
///
/// NOT responsible for:
/// - Updating checkout_sessions.status (delegated to PaymentEventProcessor)
/// - Triggering sweeps directly (handled by PaymentEventProcessor)
pub struct TransactionIndexer {
    db: DatabaseConnection,
    /// Database URL for AddressSyncManager's LISTEN connection
    db_url: Secret<String>,
    scanner: Arc<dyn BlockScanner>,
    /// Contract→Symbol mapping (e.g., {"TR7NHqj..." => "USDT", "0xA0b86..." => "USDC"})
    watchlist: HashMap<String, String>,
    /// Network identifier this indexer monitors (e.g., "TRON", "BSC")
    network: Network,
    /// All system addresses (Base58 format) for O(1) lookup
    /// Uses DashMap for concurrent read/write without blocking
    /// Unified Cache: Stores existence AND metadata to avoid split-brain issues.
    all_addresses: Arc<DashMap<String, MonitoredAddressInfo>>,
    /// Indexer state (last block, pending txs)
    state: Arc<RwLock<IndexerStateCache>>,
    /// Environment context (Production/Sandbox)
    environment: Environment,
    /// Alerting service for MVP monitoring
    alerting_service: Arc<AlertingService>,
    /// Chain health registry for self-reporting health to the circuit breaker.
    /// None in tests or when registry is not wired.
    chain_health: Option<ChainHealthRegistry>,
    /// Timestamp of last periodic re-scan (L2 safety net)
    last_rescan: RwLock<Instant>,
}

/// Information about an active session (Pending/Underpaid)
#[derive(Clone, Debug)]
pub struct ActiveSessionInfo {
    pub session_id: String,
    pub merchant_id: String,
    pub status: checkout_sessions::SessionStatus,
    pub expires_at: DateTime<Utc>,
    /// The currency this session expects (e.g., "USDT", "USDC")
    pub currency: String,
}

impl TransactionIndexer {
    pub fn new(
        db: DatabaseConnection,
        db_url: Secret<String>,
        scanner: Arc<dyn BlockScanner>,
        watchlist: HashMap<String, String>,
        network: Network,
        environment: Environment,
        alerting_service: Arc<AlertingService>,
        chain_health: Option<ChainHealthRegistry>,
    ) -> Self {
        Self {
            db,
            db_url,
            scanner,
            watchlist,
            network,
            all_addresses: Arc::new(DashMap::new()),
            state: Arc::new(RwLock::new(IndexerStateCache::default())),
            environment,
            alerting_service,
            chain_health,
            last_rescan: RwLock::new(Instant::now()),
        }
    }

    /// Start the indexer loop (runs in background)
    pub async fn start(self: Arc<Self>, token: tokio_util::sync::CancellationToken) -> Result<()> {
        info!(
            safety_lag_blocks = self.scanner.safety_lag_blocks(),
            rescan_depth = RESCAN_DEPTH,
            rescan_interval_secs = RESCAN_INTERVAL.as_secs(),
            "Starting block-based transaction indexer (with real-time address sync)"
        );

        // Check for reset mode (dev mode: start from current block)
        let reset_start = should_reset_start();
        if reset_start {
            warn!("INDEXER_RESET_START is enabled - will start from current block instead of persisted state");
        }

        // Load last processed block from database (persistence across restarts)
        let last_block = if reset_start {
            0 // Force start from current block
        } else {
            self.load_last_processed_block().await?
        };
        {
            let mut state = self.state.write().await;
            state.last_block = last_block;
        }

        // If no persisted state OR reset mode, start from current block
        if last_block == 0 {
            let current_block = self.scanner.get_current_block().await?;
            let mut state = self.state.write().await;
            state.last_block = current_block;
            self.save_last_processed_block(current_block, current_block)
                .await?;
            if reset_start {
                info!(
                    block = current_block,
                    "Indexer starting from current block (RESET MODE)"
                );
            } else {
                info!(
                    block = current_block,
                    "Indexer starting from current block (no persisted state)"
                );
            }
        } else {
            info!(block = last_block, "Indexer resuming from persisted block");
        }

        // Recovery: Rebuild pending_confirmations from unconfirmed transactions in DB
        self.recover_unconfirmed_transactions().await?;

        // CRITICAL: Initial hydration of ALL addresses before starting block scan
        // This must complete before we start processing blocks to avoid missing payments
        self.hydrate_address_cache().await?;

        // Start AddressSyncManager for real-time cache updates
        // This replaces the old 5-minute periodic refresh with:
        // 1. LISTEN/NOTIFY: instant updates when new addresses are created
        // 2. Fallback sync: every 60s query for addresses created in last 5 minutes
        let sync_manager = Arc::new(AddressSyncManager::new(
            self.db_url.expose_secret().clone(),
            self.db.clone(),
            self.network.clone(),
            self.environment.clone(),
            self.all_addresses.clone(),
        ));

        // Spawn LISTEN/NOTIFY subscriber (primary real-time sync)
        let listener_manager = sync_manager.clone();
        let listener_token = token.clone();
        let listener_network = self.network.clone();
        tokio::spawn(async move {
            if let Err(e) = listener_manager
                .start_notification_listener(listener_token)
                .await
            {
                error!(network = %listener_network, error = %e, "LISTEN/NOTIFY sync failed");
            }
        });

        // Spawn fallback sync loop (safety net)
        let fallback_manager = sync_manager.clone();
        let fallback_token = token.clone();
        let fallback_network = self.network.clone();
        tokio::spawn(async move {
            if let Err(e) = fallback_manager.start_fallback_sync(fallback_token).await {
                error!(network = %fallback_network, error = %e, "Fallback sync failed");
            }
        });

        info!("Address sync manager started (LISTEN/NOTIFY + fallback)");

        // Main block scanning loop
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!(network = %self.network, "Indexer received shutdown signal, saving state...");
                    // Try to save state one last time
                    let state = self.state.read().await;
                    if let Err(e) = self.save_last_processed_block(state.last_block, state.last_block).await {
                         error!(network = %self.network, error = %e, "Failed to save final indexer state");
                    }
                    info!(network = %self.network, "Indexer shutdown complete.");
                    break;
                }
                result = async {
                    // Fetch current block ONCE per cycle with timeout protection
                    // Timeout prevents hung RPC connections from blocking the loop
                    let current_block = match tokio::time::timeout(
                        RPC_CALL_TIMEOUT,
                        self.scanner.get_current_block(),
                    ).await {
                        Ok(Ok(block_num)) => {
                            // Successful RPC = indexer is alive and processing
                            if let Some(ref health) = self.chain_health {
                                health.mark_healthy(&self.network);
                            }
                            block_num
                        }
                        Ok(Err(e)) => {
                            warn!(network = %self.network, error = %e, "Failed to fetch current block (transient, will retry)");
                            return Ok::<(), anyhow::Error>(());
                        }
                        Err(_) => {
                            error!(
                                network = %self.network,
                                timeout_secs = RPC_CALL_TIMEOUT.as_secs(),
                                "RPC call timed out (possible hung connection), failing over..."
                            );
                            return Ok::<(), anyhow::Error>(());
                        }
                    };

                    // L1 Safety Lag: only scan blocks old enough for RPC event indexing.
                    // Gives TronGrid/providers time to index events before we scan.
                    let safety_lag = self.scanner.safety_lag_blocks();
                    let safe_head = current_block.saturating_sub(safety_lag);

                    // Each iteration processes up to max_block_range blocks,
                    // which follows the active RPC provider's limit
                    // (Alchemy=10, Ankr=1000). Adjusts dynamically on failover.
                    let max_iterations = 200; // safety cap to prevent infinite loops
                    for _ in 0..max_iterations {
                        if token.is_cancelled() {
                            break; // respect shutdown during catch-up
                        }
                        let last = self.state.read().await.last_block;
                        if last >= safe_head {
                            break; // caught up (with safety lag)
                        }
                        if let Err(e) = self.scan_new_blocks(safe_head, current_block).await {
                            error!(network = %self.network, error = %e, "Error scanning blocks");
                            break; // stop on error, retry next cycle
                        }
                    }

                    // Check pending transactions for confirmations
                    if let Err(e) = self.check_confirmations(current_block).await {
                        error!(network = %self.network, error = %e, "Error checking confirmations");
                    }

                    // L2 Re-scan Window: periodically re-scan recent blocks
                    // to catch events missed due to extreme RPC indexing delays.
                    if self.last_rescan.read().await.elapsed() >= RESCAN_INTERVAL {
                        let last = self.state.read().await.last_block;
                        let rescan_from = last.saturating_sub(RESCAN_DEPTH).max(1);
                        info!(network = %self.network, from = rescan_from, to = last, "Starting periodic re-scan");
                        if let Err(e) = self.rescan_blocks(rescan_from, last, current_block).await {
                            warn!(network = %self.network, error = %e, "Re-scan failed (will retry next interval)");
                        }
                        *self.last_rescan.write().await = Instant::now();
                    }

                    Ok::<(), anyhow::Error>(())
                } => {
                     if let Err(e) = result {
                        // Log error but continue loop (unless fatal?)
                         error!(network = %self.network, error = %e, "Block processing cycle error");
                    }
                    // Wait for next poll interval
                    // Respect cancellation during wait
                    tokio::select! {
                        _ = token.cancelled() => {
                             info!(network = %self.network, "Indexer received shutdown signal during poll wait");
                             break;
                        }
                        _ = tokio::time::sleep(self.scanner.poll_interval()) => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Load last processed block from database
    async fn load_last_processed_block(&self) -> Result<i64> {
        let state = IndexerState::find_by_id(self.network.as_str())
            .one(&self.db)
            .await?;

        Ok(state.map(|s| s.last_processed_block).unwrap_or(0))
    }

    /// Save last processed block (and chain head) to database
    async fn save_last_processed_block(&self, block_number: i64, chain_head: i64) -> Result<()> {
        let model = indexer_state::ActiveModel {
            network: Set(self.network.as_str().to_string()),
            last_processed_block: Set(block_number),
            chain_head_block: Set(Some(chain_head)),
            updated_at: Set(Utc::now().into()),
        };

        // Upsert: insert or update on conflict
        let result = IndexerState::insert(model.clone())
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(indexer_state::Column::Network)
                    .update_columns([
                        indexer_state::Column::LastProcessedBlock,
                        indexer_state::Column::ChainHeadBlock,
                        indexer_state::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await;

        match result {
            Ok(_) => {
                debug!(
                    block = block_number,
                    chain_head, "Saved last processed block"
                );
                Ok(())
            }
            Err(e) => {
                error!(error = %e, block = block_number, "Failed to save last processed block");
                Err(e.into())
            }
        }
    }

    /// Hydrate cache: Load ALL addresses from database into DashMap cache
    /// Called once at startup before block scanning begins
    async fn hydrate_address_cache(&self) -> Result<()> {
        info!("Performing initial address cache hydration");

        // Load all addresses from addresses table
        let all_addrs = Addresses::find()
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .all(&self.db)
            .await?;

        for addr in all_addrs {
            // Insert into DashMap (concurrent-safe)
            self.all_addresses.insert(
                addr.address.clone(),
                MonitoredAddressInfo {
                    merchant_id: addr.merchant_id.clone(),
                },
            );
        }

        info!(
            total_addresses = self.all_addresses.len(),
            "Initial address load complete"
        );
        Ok(())
    }

    /// Scan new blocks for Transfer events to system addresses
    ///
    /// `scan_ceiling`: highest block to scan (= chain_head - safety_lag).
    /// `chain_head`:   actual chain head (for indexer_state monitoring only).
    ///
    /// Uses `scan_block_range` for batch fetching: EVM chains get a single
    /// `eth_getLogs(from, to)` call; TRON falls back to per-block scanning.
    async fn scan_new_blocks(&self, scan_ceiling: i64, chain_head: i64) -> Result<()> {
        let last_processed = {
            let state = self.state.read().await;
            state.last_block
        };

        if last_processed >= scan_ceiling {
            return Ok(());
        }

        // Limit blocks per cycle to the active RPC provider's max range.
        // Alchemy free = 10, Ankr = 1000. Changes dynamically on failover.
        let max_blocks_per_cycle = self.scanner.max_block_range();
        let from_block = last_processed + 1;
        let end_block = std::cmp::min(scan_ceiling, last_processed + max_blocks_per_cycle);

        // Fetch all events for the range in a single call (EVM) or per-block (TRON)
        // Returns (events, actual_to_block): actual_to_block may be < end_block if
        // a failover re-clamped the range to fit a provider's smaller limit.
        let (events, actual_end_block) = self
            .fetch_range_events_with_retry(from_block, end_block)
            .await?;

        // Process all events (address filter + payment detection)
        if !events.is_empty() && !self.all_addresses.is_empty() {
            for event in &events {
                if let Err(e) = self.process_event(event, chain_head).await {
                    error!(
                        tx_hash = %event.tx_hash,
                        event_index = event.event_index,
                        error = %e,
                        "Error processing event"
                    );
                }
            }
        }

        // Checkpoint: save last processed block after entire batch
        // Uses actual_end_block (not original end_block) to avoid skipping blocks
        // when failover re-clamped the range to a smaller provider limit.
        // Remaining blocks (actual_end_block+1 .. end_block) will be scanned next cycle.
        {
            let mut state = self.state.write().await;
            state.last_block = actual_end_block;
        }
        // chain_head still tracks the real head for /ready and admin monitoring
        self.save_last_processed_block(actual_end_block, chain_head)
            .await?;

        // Prometheus: record block lag and blocks scanned
        let blocks_scanned = (actual_end_block - from_block + 1).max(0);
        crate::services::metrics::record_block_lag(
            self.network.as_str(),
            chain_head - actual_end_block,
        );
        crate::services::metrics::inc_blocks_scanned_by(
            self.network.as_str(),
            blocks_scanned as u64,
        );

        Ok(())
    }

    /// L2 Re-scan: periodic re-scan of recent blocks to catch missed events.
    ///
    /// Uses pre-SELECT to distinguish genuinely recovered events from
    /// already-known ones, avoiding log noise ("日志风暴").
    /// Idempotency: already-processed events are detected via DB lookup
    /// and silently skipped. Only truly missing events are processed.
    async fn rescan_blocks(&self, from: i64, to: i64, current_block: i64) -> Result<()> {
        // Chunk the range by max_block_range to respect RPC provider limits
        // (e.g. Alchemy free tier = 10 blocks per eth_getLogs call)
        let chunk_size = self.scanner.max_block_range();
        let mut all_events = Vec::new();
        let mut chunk_start = from;
        while chunk_start <= to {
            let chunk_end = std::cmp::min(chunk_start + chunk_size - 1, to);
            let (events, actual_chunk_end) = self
                .fetch_range_events_with_retry(chunk_start, chunk_end)
                .await?;
            all_events.extend(events);
            chunk_start = actual_chunk_end + 1;
        }
        let events = all_events;
        if events.is_empty() {
            return Ok(());
        }

        let mut recovered = 0u32;
        let mut skipped = 0u32;
        for event in &events {
            // Skip events not targeting our addresses (same filter as main loop)
            if self.all_addresses.get(&event.to).is_none() {
                continue;
            }

            // Pre-SELECT: check if this event was already processed by main loop
            let exists = Transactions::find()
                .filter(transactions::Column::Network.eq(self.network.as_str()))
                .filter(transactions::Column::TxHash.eq(&event.tx_hash))
                .filter(transactions::Column::LogIndex.eq(event.event_index))
                .one(&self.db)
                .await?
                .is_some();
            if exists {
                skipped += 1;
                continue; // Already known, silently skip
            }

            // Also check payment_exceptions for exception events
            let exception_exists = PaymentExceptions::find()
                .filter(payment_exceptions::Column::Network.eq(self.network.as_str()))
                .filter(payment_exceptions::Column::TxHash.eq(&event.tx_hash))
                .filter(payment_exceptions::Column::LogIndex.eq(event.event_index))
                .one(&self.db)
                .await?
                .is_some();
            if exception_exists {
                skipped += 1;
                continue;
            }

            // Genuinely missed event — recover it
            if let Err(e) = self.process_event(event, current_block).await {
                warn!(tx = %event.tx_hash, error = %e, "Re-scan event processing error");
            } else {
                recovered += 1;
                warn!(
                    tx = %event.tx_hash,
                    block = event.block_number,
                    "🚨 Recovered missing event during re-scan!"
                );
            }
        }

        debug!(scanned_events = events.len(), skipped, "Re-scan completed");
        if recovered > 0 {
            info!(recovered, from, to, "Re-scan recovered missing events!");
        }
        Ok(())
    }

    /// Fetch events for a block range with exponential backoff retry.
    ///
    /// Same retry strategy as `fetch_block_events_with_retry` but calls
    /// `scan_block_range` for batch fetching (1 RPC call on EVM chains).
    /// Returns (events, actual_to_block): actual_to_block may be smaller than
    /// original_to_block if a failover re-clamped the range to fit a provider's
    /// smaller limit (e.g., public RPC 1000 → Alchemy 10).
    async fn fetch_range_events_with_retry(
        &self,
        from_block: i64,
        original_to_block: i64,
    ) -> Result<(Vec<IndexerTransferEvent>, i64)> {
        let mut attempt: u32 = 0;
        let mut current_delay = BLOCK_FETCH_INITIAL_DELAY;

        loop {
            attempt += 1;

            // Re-clamp to_block on every attempt: after failover, max_block_range
            // may shrink (e.g., public RPC 1000 → Alchemy 10). Without this,
            // retries send the original wide range which the new provider rejects.
            let current_max = self.scanner.max_block_range();
            let to_block = std::cmp::min(original_to_block, from_block + current_max - 1);

            match self.scanner.scan_block_range(from_block, to_block).await {
                Ok(events) => {
                    if attempt > 1 {
                        info!(
                            from_block,
                            to_block,
                            attempts = attempt,
                            "Successfully fetched range events after retries"
                        );
                    }
                    return Ok((events, to_block));
                }
                Err(e) => {
                    if Self::is_fatal_error(&e) {
                        error!(
                            network = %self.network,
                            from_block,
                            to_block,
                            error = %e,
                            "Fatal error fetching range events (non-retryable)"
                        );
                        return Err(e);
                    }

                    if attempt >= BLOCK_FETCH_ALERT_THRESHOLD {
                        error!(
                            network = %self.network,
                            from_block,
                            to_block,
                            attempt,
                            delay_secs = current_delay.as_secs_f32(),
                            error = %e,
                            "Range event fetch failing repeatedly ({}+ attempts), retrying...",
                            BLOCK_FETCH_ALERT_THRESHOLD
                        );
                        self.alerting_service.send_alert(
                            "indexer_sync_failing",
                            AlertLevel::Warning,
                            &format!(
                                "Block range {}-{} fetch failing ({} attempts): {}",
                                from_block, to_block, attempt, e
                            ),
                        );
                    } else {
                        warn!(
                            network = %self.network,
                            from_block,
                            to_block,
                            attempt,
                            delay_secs = current_delay.as_secs_f32(),
                            error = %e,
                            "Failed to fetch range events, retrying with backoff..."
                        );
                    }

                    let jittered_delay = Self::apply_jitter(current_delay);
                    tokio::time::sleep(jittered_delay).await;

                    current_delay =
                        std::cmp::min(current_delay.saturating_mul(2), BLOCK_FETCH_MAX_DELAY);
                }
            }
        }
    }

    /// Determine if an error is fatal (non-retryable)
    ///
    /// Fatal errors indicate data corruption or API contract violations.
    /// Transient errors (network issues) should be retried.
    fn is_fatal_error(error: &anyhow::Error) -> bool {
        let error_str = error.to_string().to_lowercase();

        // Fatal: Data format/parsing errors
        if error_str.contains("parse")
            || error_str.contains("deserialize")
            || error_str.contains("invalid format")
            || error_str.contains("unexpected type")
            || error_str.contains("missing field")
        {
            return true;
        }

        // Fatal: Authentication/authorization errors
        if error_str.contains("unauthorized")
            || error_str.contains("forbidden")
            || error_str.contains("invalid api key")
        {
            return true;
        }

        // NOTE: "block not found" is intentionally NOT treated as fatal.
        // In distributed TronGrid clusters, load balancing may route requests to nodes
        // with slightly different sync states. A node may return "block not found" for
        // a block that just became available from get_current_block() due to propagation delay.
        // This is a transient condition and should be retried with exponential backoff.

        // All other errors are considered transient (retryable)
        false
    }

    /// Apply jitter to a duration (±10%)
    fn apply_jitter(duration: Duration) -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Use current time as a simple pseudo-random source
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();

        let mut hasher = DefaultHasher::new();
        now.as_nanos().hash(&mut hasher);
        let hash = hasher.finish();

        // Convert hash to a value in range [-1.0, 1.0]
        let normalized = (hash as f64 / u64::MAX as f64) * 2.0 - 1.0;
        let jitter_multiplier = 1.0 + (normalized * BLOCK_FETCH_JITTER_FACTOR);

        Duration::from_secs_f64(duration.as_secs_f64() * jitter_multiplier)
    }

    /// Process a single Transfer event (already parsed by BlockScanner)
    ///
    /// Classifies payments as:
    /// - Normal: Active session exists for the address (queried from DB)
    /// - Exception: No active session (idle address, expired session, etc.)
    async fn process_event(&self, event: &IndexerTransferEvent, current_block: i64) -> Result<()> {
        // Normalize amount from chain-specific precision to 6-decimal i64
        let amount = self.normalize_amount(&event.amount, &event.token)?;
        let to_addr = &event.to;
        let from_addr = &event.from;
        let tx_hash = &event.tx_hash;

        // Check if this address belongs to our system (O(1) DashMap lookup)
        // CRITICAL: This is the main filter - absolutely NO DB fallback here
        // to protect against mainnet spam overwhelming the database
        // Also retrieve address info directly from the Unified Cache without cloning the whole map
        let address_info_entry = match self.all_addresses.get(to_addr) {
            Some(entry) => entry,
            None => return Ok(()),
        };
        // Clone only the metadata value, not the map
        let address_info_val = address_info_entry.value().clone();
        // Drop guard immediately to unblock specific key
        drop(address_info_entry);

        // Wrap in Option for consistency with previous logic (Checkout addresses only below)
        let address_info = Some(&address_info_val);

        // Query active session from DB (fresh data, no cache staleness issues)
        let session_info = self.fetch_active_session(to_addr).await?;

        // Determine payment type and apply appropriate dust filter
        match session_info {
            Some(ref session) => {
                // ── Dust Filter (applies to ALL tokens) ───────────────────
                // Must run BEFORE WrongToken check to avoid creating noisy
                // exception records for dust-amount wrong-token transfers.
                if amount < DUST_THRESHOLD {
                    debug!(tx_hash = %tx_hash, amount, "Filtered dust transaction (active session)");
                    return Ok(());
                }

                // ── Wrong Token Check ──────────────────────────────────────
                // If the incoming token doesn't match the session's expected currency,
                // create a WrongToken exception instead of crediting the payment.
                if event.token != session.currency {
                    warn!(
                        tx_hash = %tx_hash,
                        session_id = %session.session_id,
                        expected = %session.currency,
                        received = %event.token,
                        amount,
                        "Wrong token payment detected"
                    );

                    // Prometheus counter for WrongToken monitoring
                    // (per-type counter emitted inside process_exception_payment)

                    // Sentry alert for ops visibility
                    self.alerting_service.send_alert(
                        "wrong_token_payment",
                        AlertLevel::Warning,
                        &format!(
                            "⚠️ Wrong token payment: session={} expected={} received={} amount={} tx={}",
                            session.session_id, session.currency, event.token, amount, tx_hash
                        ),
                    );

                    return self
                        .process_exception_payment(
                            event,
                            to_addr,
                            from_addr,
                            amount,
                            address_info,
                            Some(payment_exceptions::ExceptionType::WrongToken),
                        )
                        .await;
                }

                self.process_normal_payment(
                    event,
                    to_addr,
                    from_addr,
                    amount,
                    session,
                    current_block,
                )
                .await
            }
            None => {
                // Exception path: no active session
                // Issue C Fix: Record dust payments as exceptions instead of silently discarding
                // For financial systems, all non-zero payments should be tracked
                let exception_type = if amount < IDLE_DUST_THRESHOLD {
                    // Mark as dust payment but still record it
                    Some(payment_exceptions::ExceptionType::DustPayment)
                } else {
                    None // Will be determined by process_exception_payment
                };

                self.process_exception_payment(
                    event,
                    to_addr,
                    from_addr,
                    amount,
                    address_info,
                    exception_type,
                )
                .await
            }
        }
    }

    /// Process a normal payment (active session exists)
    async fn process_normal_payment(
        &self,
        event: &IndexerTransferEvent,
        to_addr: &str,
        from_addr: &str,
        amount: i64,
        session: &ActiveSessionInfo,
        current_block: i64,
    ) -> Result<()> {
        let log_index = event.event_index;
        let tx_hash = &event.tx_hash;

        // Check if we've already processed this transaction
        let existing = Transactions::find()
            .filter(transactions::Column::Network.eq(self.network.as_str()))
            .filter(transactions::Column::TxHash.eq(tx_hash))
            .filter(transactions::Column::LogIndex.eq(log_index))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Ok(());
        }

        info!(
            session_id = %session.session_id,
            tx_hash = %tx_hash,
            log_index,
            amount,
            from = %from_addr,
            to = %to_addr,
            block = event.block_number,
            "Normal payment detected"
        );

        // Prometheus: normal payment detected
        crate::services::metrics::inc_payment_detected(self.network.as_str(), "normal");

        // Begin transaction for atomic write
        let db_tx = self.db.begin().await?;

        // 1. Record the transaction
        let confirmations = (current_block - event.block_number) as i32;
        let tx_record = transactions::ActiveModel {
            network: Set(self.network.as_str().to_string()),
            tx_hash: Set(tx_hash.clone()),
            log_index: Set(log_index),
            session_id: Set(Some(session.session_id.clone())),
            merchant_id: Set(session.merchant_id.clone()),
            currency_symbol: Set(event.token.clone()),
            currency_contract: Set(self
                .watchlist
                .iter()
                .find(|(_, symbol)| symbol.eq_ignore_ascii_case(&event.token))
                .map(|(contract, _)| contract.clone())
                .unwrap_or_default()),
            from_address: Set(from_addr.to_string()),
            to_address: Set(to_addr.to_string()),
            amount: Set(amount),
            status: Set(transactions::ChainTxState::Unconfirmed),
            confirmations_count: Set(confirmations),
            block_number: Set(event.block_number),
            block_timestamp: Set({
                let ts = event.block_timestamp;
                if ts == 0 {
                    Utc::now().into()
                } else {
                    chrono::DateTime::from_timestamp_millis(ts)
                        .unwrap_or_else(|| Utc::now().into())
                        .into()
                }
            }),
            is_credited: Set(false),
            ..Default::default()
        };
        tx_record.insert(&db_tx).await?;

        // 2. Write payment_detected event to outbox
        let event_id = format!("pe_{}", Uuid::new_v4().simple());
        let payment_event = payment_events::ActiveModel {
            id: Set(event_id),
            event_type: Set(payment_events::PaymentEventType::PaymentDetected),
            session_id: Set(session.session_id.clone()),
            tx_network: Set(self.network.as_str().to_string()),
            tx_hash: Set(tx_hash.clone()),
            tx_log_index: Set(log_index),
            amount: Set(amount),
            status: Set(payment_events::PaymentEventStatus::Pending),
            attempt_count: Set(0),
            next_retry_at: Set(Utc::now().into()),
            ..Default::default()
        };
        let _ = payment_event.insert(&db_tx).await;

        // 3. Update address status and balance
        // Dynamic column based on token type
        let balance_sql = if event.token == "USDC" {
            r#"
            UPDATE addresses
            SET status = 'Detected',
                usdc_balance = usdc_balance + $1,
                updated_at = NOW()
            WHERE network = $2 AND address = $3
              AND status IN ('Idle', 'Assigned', 'Detected')
            "#
        } else {
            r#"
            UPDATE addresses
            SET status = 'Detected',
                usdt_balance = usdt_balance + $1,
                updated_at = NOW()
            WHERE network = $2 AND address = $3
              AND status IN ('Idle', 'Assigned', 'Detected')
            "#
        };
        db_tx
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                balance_sql,
                [amount.into(), self.network.as_str().into(), to_addr.into()],
            ))
            .await?;

        db_tx.commit().await?;

        // Add to pending confirmations
        let mut state = self.state.write().await;
        let key = format!("{}:{}:{}", self.network, tx_hash, log_index);
        state.pending_confirmations.insert(
            key,
            PendingTx {
                network: self.network.as_str().to_string(),
                tx_hash: tx_hash.clone(),
                log_index,
                session_id: Some(session.session_id.clone()),
                amount,
                block_number: event.block_number,
                confirmations,
                is_exception: false,
                merchant_id: Some(session.merchant_id.clone()),
                created_at: Instant::now(),
            },
        );

        Ok(())
    }

    /// Process an exception payment with optional pre-determined exception type
    ///
    /// If `known_exception_type` is Some, use that type directly.
    /// Otherwise, determine the type based on session state.
    async fn process_exception_payment(
        &self,
        event: &IndexerTransferEvent,
        to_addr: &str,
        from_addr: &str,
        amount: i64,
        address_info: Option<&MonitoredAddressInfo>,
        known_exception_type: Option<payment_exceptions::ExceptionType>,
    ) -> Result<()> {
        let log_index = event.event_index;
        let tx_hash = &event.tx_hash;

        // Check if we've already recorded this exception
        let existing = PaymentExceptions::find()
            .filter(payment_exceptions::Column::Network.eq(self.network.as_str()))
            .filter(payment_exceptions::Column::TxHash.eq(tx_hash))
            .filter(payment_exceptions::Column::LogIndex.eq(log_index))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Ok(());
        }

        // Use override type if provided, otherwise determine from session state
        let exception_type = match known_exception_type {
            Some(t) => t,
            None => self.determine_exception_type(to_addr).await?,
        };

        // Get merchant_id from address info if available
        let merchant_id = address_info.map(|info| info.merchant_id.clone());

        // Prometheus: exception payment detected (aggregate + per-type)
        crate::services::metrics::inc_payment_detected(self.network.as_str(), "exception");
        crate::services::metrics::inc_exception(
            self.network.as_str(),
            &format!("{:?}", exception_type),
        );

        // Try to find a related session (might be expired)
        let related_session = CheckoutSessions::find()
            .filter(checkout_sessions::Column::PayAddress.eq(to_addr))
            .filter(checkout_sessions::Column::Network.eq(self.network.as_str()))
            .order_by_desc(checkout_sessions::Column::CreatedAt)
            .one(&self.db)
            .await?;

        let session_id = related_session.map(|s| s.id);

        // Use appropriate log level based on exception type
        // Dust payments are expected/normal, other exceptions warrant warnings
        if matches!(
            exception_type,
            payment_exceptions::ExceptionType::DustPayment
        ) {
            debug!(
                tx_hash = %tx_hash,
                log_index,
                amount,
                from = %from_addr,
                to = %to_addr,
                exception_type = ?exception_type,
                merchant_id = ?merchant_id,
                block = event.block_number,
                "Dust payment recorded as exception"
            );
        } else {
            warn!(
                tx_hash = %tx_hash,
                log_index,
                amount,
                from = %from_addr,
                to = %to_addr,
                exception_type = ?exception_type,
                merchant_id = ?merchant_id,
                session_id = ?session_id,
                block = event.block_number,
                "Payment exception detected"
            );
        }

        // Begin transaction
        let db_tx = self.db.begin().await?;

        // 1. Record the exception
        let (status, resolution) = if matches!(
            exception_type,
            payment_exceptions::ExceptionType::DustPayment
        ) {
            (
                payment_exceptions::ExceptionStatus::Resolved,
                Some(payment_exceptions::Resolution::Ignored),
            )
        } else {
            (payment_exceptions::ExceptionStatus::Pending, None)
        };

        let exception_id = format!("pex_{}", Uuid::new_v4().simple());
        let exception = payment_exceptions::ActiveModel {
            id: Set(exception_id.clone()),
            network: Set(self.network.as_str().to_string()),
            tx_hash: Set(tx_hash.clone()),
            log_index: Set(log_index),
            exception_type: Set(exception_type.clone()),
            to_address: Set(to_addr.to_string()),
            from_address: Set(from_addr.to_string()),
            amount: Set(amount),
            currency_symbol: Set(event.token.clone()),
            merchant_id: Set(merchant_id.clone()),
            session_id: Set(session_id.clone()),
            block_number: Set(event.block_number),
            block_timestamp: Set({
                let ts = event.block_timestamp;
                if ts == 0 {
                    Utc::now().into()
                } else {
                    chrono::DateTime::from_timestamp_millis(ts)
                        .unwrap_or_else(|| Utc::now().into())
                        .into()
                }
            }),
            status: Set(status),
            resolution: Set(resolution.clone()),
            resolution_ref_id: Set(None),
            resolved_at: Set(if resolution.is_some() {
                Some(Utc::now().into())
            } else {
                None
            }),
            resolved_by: Set(None),
            notes: Set(if resolution.is_some() {
                Some("Automatically ignored dust payment".to_string())
            } else {
                None
            }),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };
        exception.insert(&db_tx).await?;

        // 2. Update address balance AND status (even for exceptions, track the funds)
        // SEMANTIC FIX: Set status to Detected to indicate funds are present.
        // This is consistent with process_normal_payment and makes queries cleaner.
        let balance_sql = if event.token == "USDC" {
            r#"
            UPDATE addresses
            SET status = 'Detected',
                usdc_balance = usdc_balance + $1,
                updated_at = NOW()
            WHERE network = $2 AND address = $3
              AND status IN ('Idle', 'Assigned', 'Detected')
            "#
        } else {
            r#"
            UPDATE addresses
            SET status = 'Detected',
                usdt_balance = usdt_balance + $1,
                updated_at = NOW()
            WHERE network = $2 AND address = $3
              AND status IN ('Idle', 'Assigned', 'Detected')
            "#
        };
        let update_result = db_tx
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                balance_sql,
                [amount.into(), self.network.as_str().into(), to_addr.into()],
            ))
            .await?;

        // Defensive: log when address balance update didn't match any rows.
        // This means the address status was not in (Idle, Assigned, Detected) —
        // likely already Cooling/Sweeping from a prior operation.
        if update_result.rows_affected() == 0 {
            warn!(
                tx_hash = %tx_hash,
                address = %to_addr,
                exception_type = ?exception_type,
                amount,
                "Address balance update skipped: status not in (Idle, Assigned, Detected). \
                 Exception recorded but address balance NOT updated."
            );
        }

        db_tx.commit().await?;

        info!(
            exception_id = %exception_id,
            exception_type = ?exception_type,
            merchant_id = ?merchant_id,
            amount,
            "Payment exception recorded for manual review"
        );

        Ok(())
    }

    /// Determine the type of payment exception
    async fn determine_exception_type(
        &self,
        to_addr: &str,
    ) -> Result<payment_exceptions::ExceptionType> {
        // Check if there's any session (past or present) for this address
        let any_session = CheckoutSessions::find()
            .filter(checkout_sessions::Column::PayAddress.eq(to_addr))
            .filter(checkout_sessions::Column::Network.eq(self.network.as_str()))
            .order_by_desc(checkout_sessions::Column::CreatedAt)
            .one(&self.db)
            .await?;

        match any_session {
            Some(session) => {
                // Session exists, determine why it's not active
                match session.status {
                    checkout_sessions::SessionStatus::Expired => {
                        Ok(payment_exceptions::ExceptionType::SessionExpired)
                    }
                    checkout_sessions::SessionStatus::Paid
                    | checkout_sessions::SessionStatus::Overpaid => {
                        Ok(payment_exceptions::ExceptionType::SessionAlreadyCompleted)
                    }
                    _ => Ok(payment_exceptions::ExceptionType::Unknown),
                }
            }
            None => {
                // No session ever existed for this address
                Ok(payment_exceptions::ExceptionType::NoActiveSession)
            }
        }
    }

    /// Check pending transactions for sufficient confirmations
    ///
    /// CRITICAL: Before confirming a transaction, we verify it still exists on-chain.
    /// This prevents "ghost transaction" attacks where a reorg causes a transaction
    /// to exist in our DB but not on the current main chain.
    ///
    /// `current_block` is fetched once per cycle in the main loop and shared
    /// with `scan_new_blocks()` to avoid a redundant RPC call.
    async fn check_confirmations(&self, current_block: i64) -> Result<()> {
        // Phase 1: Collect candidates and update confirmation counts
        // We need to release the lock before making RPC calls
        let (candidates_for_verification, _to_remove_immediately) = {
            let mut state = self.state.write().await;
            let mut candidates = Vec::new();
            let mut to_remove = Vec::new();

            for (key, pending) in state.pending_confirmations.iter_mut() {
                // Check if pending transaction is too old (stale)
                if pending.created_at.elapsed() > MAX_PENDING_AGE {
                    warn!(
                        tx_hash = %pending.tx_hash,
                        log_index = pending.log_index,
                        age_secs = pending.created_at.elapsed().as_secs(),
                        "Removing stale pending transaction (exceeded max age)"
                    );
                    to_remove.push(key.clone());
                    continue;
                }

                let confirmations = (current_block - pending.block_number) as i32;
                pending.confirmations = confirmations;

                // Exceptions don't need confirmation tracking or RPC verification
                if pending.is_exception {
                    if confirmations >= self.scanner.required_confirmations() {
                        to_remove.push(key.clone());
                    }
                    continue;
                }

                // Collect candidates that have reached confirmation threshold
                // We'll verify them via RPC before actually confirming
                if confirmations >= self.scanner.required_confirmations() {
                    candidates.push((key.clone(), pending.clone()));
                }
            }

            // Remove stale/exception entries immediately
            for key in &to_remove {
                state.pending_confirmations.remove(key);
            }

            (candidates, to_remove)
        };
        // Lock released here

        if candidates_for_verification.is_empty() {
            return Ok(());
        }

        debug!(
            count = candidates_for_verification.len(),
            "Verifying {} candidates for confirmation via RPC",
            candidates_for_verification.len()
        );

        // Phase 2: Verify each candidate via RPC (outside of lock)
        let mut verified_txs = Vec::new();
        let mut ghost_txs = Vec::new(); // Transactions that no longer exist on chain
        let mut failed_txs = Vec::new(); // Transactions that exist but failed

        for (key, pending) in &candidates_for_verification {
            match self.scanner.verify_transaction(&pending.tx_hash).await {
                Ok(TxVerificationResult::Success) => {
                    // Transaction exists and succeeded - safe to confirm
                    verified_txs.push((key.clone(), pending.clone()));
                }
                Ok(TxVerificationResult::Failed(reason)) => {
                    // Transaction exists but failed (e.g., OUT_OF_ENERGY after reorg)
                    warn!(
                        tx_hash = %pending.tx_hash,
                        reason = %reason,
                        "Transaction exists but failed on chain"
                    );
                    failed_txs.push((key.clone(), pending.clone()));
                }
                Ok(TxVerificationResult::NotFound) => {
                    // CRITICAL: Transaction not found on chain!
                    // This is the "ghost transaction" scenario - likely caused by reorg
                    error!(
                        tx_hash = %pending.tx_hash,
                        session_id = ?pending.session_id,
                        block_number = pending.block_number,
                        "🚨 GHOST TRANSACTION DETECTED: Transaction not found on chain (possible reorg)"
                    );
                    self.alerting_service.send_alert(
                        "indexer_ghost_transaction",
                        AlertLevel::Critical,
                        &format!(
                            "🚨 Ghost transaction detected (possible reorg): tx={} session={:?} block={}",
                            pending.tx_hash, pending.session_id, pending.block_number
                        ),
                    );
                    ghost_txs.push((key.clone(), pending.clone()));
                }
                Err(e) => {
                    // RPC error - don't remove, will retry next cycle
                    warn!(
                        tx_hash = %pending.tx_hash,
                        error = %e,
                        "Failed to verify transaction on chain, will retry"
                    );
                }
            }
        }

        // Phase 3: Update state and DB based on verification results
        let mut confirmed_txs = Vec::new();

        // Process verified transactions
        for (key, pending) in verified_txs {
            // Update DB status to Confirmed
            match Transactions::find()
                .filter(transactions::Column::Network.eq(&pending.network))
                .filter(transactions::Column::TxHash.eq(&pending.tx_hash))
                .filter(transactions::Column::LogIndex.eq(pending.log_index))
                .one(&self.db)
                .await
            {
                Ok(Some(tx)) => {
                    let mut active: transactions::ActiveModel = tx.into();
                    active.confirmations_count = Set(pending.confirmations);
                    active.status = Set(transactions::ChainTxState::Confirmed);
                    if let Err(e) = active.update(&self.db).await {
                        error!(tx_hash = %pending.tx_hash, error = %e, "Failed to update transaction status");
                        continue;
                    }
                    confirmed_txs.push(pending);
                }
                Ok(None) => {
                    warn!(tx_hash = %pending.tx_hash, "Transaction not found in DB during confirmation");
                }
                Err(e) => {
                    error!(tx_hash = %pending.tx_hash, error = %e, "DB error during confirmation");
                    continue;
                }
            }

            // Remove from pending
            let mut state = self.state.write().await;
            state.pending_confirmations.remove(&key);
        }

        // Handle ghost transactions - mark as failed in DB
        for (key, pending) in ghost_txs {
            if let Ok(Some(tx)) = Transactions::find()
                .filter(transactions::Column::Network.eq(&pending.network))
                .filter(transactions::Column::TxHash.eq(&pending.tx_hash))
                .filter(transactions::Column::LogIndex.eq(pending.log_index))
                .one(&self.db)
                .await
            {
                let mut active: transactions::ActiveModel = tx.into();
                active.status = Set(transactions::ChainTxState::NotFound);
                if let Err(e) = active.update(&self.db).await {
                    error!(tx_hash = %pending.tx_hash, error = %e, "Failed to mark ghost transaction as failed");
                }
            }

            // Remove from pending
            let mut state = self.state.write().await;
            state.pending_confirmations.remove(&key);
        }

        // Handle failed transactions - mark as failed in DB
        for (key, pending) in failed_txs {
            if let Ok(Some(tx)) = Transactions::find()
                .filter(transactions::Column::Network.eq(&pending.network))
                .filter(transactions::Column::TxHash.eq(&pending.tx_hash))
                .filter(transactions::Column::LogIndex.eq(pending.log_index))
                .one(&self.db)
                .await
            {
                let mut active: transactions::ActiveModel = tx.into();
                active.status = Set(transactions::ChainTxState::Failed);
                if let Err(e) = active.update(&self.db).await {
                    error!(tx_hash = %pending.tx_hash, error = %e, "Failed to mark failed transaction");
                }
            }

            // Remove from pending
            let mut state = self.state.write().await;
            state.pending_confirmations.remove(&key);
        }

        // Emit confirmation events for verified transactions
        for tx in confirmed_txs {
            self.on_transaction_confirmed(&tx).await?;
        }

        Ok(())
    }

    /// Handle a confirmed transaction - emit event to outbox
    async fn on_transaction_confirmed(&self, tx: &PendingTx) -> Result<()> {
        let session_id = match &tx.session_id {
            Some(id) => id,
            None => return Ok(()), // Exceptions don't emit confirmation events
        };

        info!(
            tx_hash = %tx.tx_hash,
            session_id = %session_id,
            amount = tx.amount,
            confirmations = tx.confirmations,
            "Transaction confirmed - emitting payment event"
        );

        // Write payment_confirmed event to outbox
        let event_id = format!("pe_{}", Uuid::new_v4().simple());
        let event = payment_events::ActiveModel {
            id: Set(event_id.clone()),
            event_type: Set(payment_events::PaymentEventType::PaymentConfirmed),
            session_id: Set(session_id.clone()),
            tx_network: Set(tx.network.clone()),
            tx_hash: Set(tx.tx_hash.clone()),
            tx_log_index: Set(tx.log_index),
            amount: Set(tx.amount),
            status: Set(payment_events::PaymentEventStatus::Pending),
            attempt_count: Set(0),
            next_retry_at: Set(Utc::now().into()),
            ..Default::default()
        };

        match event.insert(&self.db).await {
            Ok(_) => {
                info!(
                    event_id = %event_id,
                    session_id = %session_id,
                    "Payment confirmed event emitted to outbox"
                );
            }
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    debug!(
                        tx_hash = %tx.tx_hash,
                        "Payment confirmed event already exists (idempotent)"
                    );
                } else {
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    /// Recover unconfirmed transactions from database after service restart
    async fn recover_unconfirmed_transactions(&self) -> Result<()> {
        let unconfirmed_txs = Transactions::find()
            .filter(transactions::Column::Network.eq(self.network.as_str()))
            .filter(transactions::Column::Status.eq(transactions::ChainTxState::Unconfirmed))
            .all(&self.db)
            .await?;

        if unconfirmed_txs.is_empty() {
            info!("No unconfirmed transactions to recover");
            return Ok(());
        }

        let mut state = self.state.write().await;
        for tx in unconfirmed_txs {
            let key = format!("{}:{}:{}", tx.network, tx.tx_hash, tx.log_index);
            state.pending_confirmations.insert(
                key.clone(),
                PendingTx {
                    network: tx.network.clone(),
                    tx_hash: tx.tx_hash.clone(),
                    log_index: tx.log_index,
                    session_id: tx.session_id.clone(),
                    amount: tx.amount,
                    block_number: tx.block_number,
                    confirmations: tx.confirmations_count,
                    is_exception: false,
                    merchant_id: Some(tx.merchant_id.clone()),
                    created_at: Instant::now(), // Note: restarted, so we reset the age
                },
            );
            debug!(
                key = %key,
                session_id = ?tx.session_id,
                "Recovered unconfirmed transaction"
            );
        }

        info!(
            count = state.pending_confirmations.len(),
            "Recovered unconfirmed transactions from database"
        );
        Ok(())
    }

    /// Get indexer statistics
    pub async fn get_stats(&self) -> IndexerStats {
        let state = self.state.read().await;

        IndexerStats {
            last_block: state.last_block,
            pending_count: state.pending_confirmations.len(),
            total_addresses: self.all_addresses.len(),
        }
    }

    /// Fetch active session for an address from database
    ///
    /// This queries the DB directly for the freshest data, avoiding cache staleness.
    /// Only returns sessions in Pending or Underpaid status.
    /// Expired sessions are NOT included — late payments route to exception/Resolution Center.
    /// Orders by CreatedAt DESC to get the latest session.
    async fn fetch_active_session(&self, pay_address: &str) -> Result<Option<ActiveSessionInfo>> {
        let session = CheckoutSessions::find()
            .filter(checkout_sessions::Column::PayAddress.eq(pay_address))
            .filter(checkout_sessions::Column::Network.eq(self.network.as_str()))
            .filter(checkout_sessions::Column::Status.is_in([
                checkout_sessions::SessionStatus::Pending,
                checkout_sessions::SessionStatus::Underpaid,
            ]))
            .order_by_desc(checkout_sessions::Column::CreatedAt)
            .one(&self.db)
            .await?;

        Ok(session.map(|s| ActiveSessionInfo {
            session_id: s.id,
            merchant_id: s.merchant_id,
            status: s.status,
            expires_at: s.expires_at.into(),
            currency: s.currency.clone(),
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ParsedTransfer {
    pub from: String,
    pub to: String,
    pub amount: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub block_number: i64,
    pub block_timestamp: i64,
}

impl TransactionIndexer {
    /// Normalize a chain-specific amount string to 6-decimal i64.
    ///
    /// Decimals are resolved dynamically from ChainConfig based on network + token.
    /// Falls back to scanner's default usdt_decimals if token not found.
    ///
    /// For TRON (6 dec): "1000000" → 1000000
    /// For BSC USDT (18 dec): "1000000000000000000" → 1000000 (divide by 10^12)
    /// For BSC USDC (18 dec): same as USDT on BSC
    /// For ETH USDC (6 dec): "1000000" → 1000000
    fn normalize_amount(&self, amount_str: &str, token: &str) -> Result<i64> {
        // Resolve decimals from ChainConfig
        let chain_config = self.network.chain_config(&self.environment);
        let decimals = chain_config
            .token_decimals(token)
            .unwrap_or(self.scanner.usdt_decimals());

        if decimals <= 6 {
            // 6 dec or less: parse directly
            Ok(amount_str.parse::<i64>().unwrap_or(0))
        } else {
            // >6 dec: divide by 10^(decimals-6)
            let divisor_exp = (decimals - 6) as u32;
            // Use U256 for safe large number division
            let value = alloy_primitives::U256::from_str_radix(amount_str, 10)
                .unwrap_or(alloy_primitives::U256::ZERO);
            let divisor =
                alloy_primitives::U256::from(10u64).pow(alloy_primitives::U256::from(divisor_exp));
            let normalized = value / divisor;
            // Safe: normalized is at most ~10^13 for 10M USDT, well within i64
            Ok(normalized.to_string().parse::<i64>().unwrap_or(0))
        }
    }
}

impl TransactionIndexer {
    /// Accept an externally-produced event (from SolanaIndexer → channel → here).
    ///
    /// Delegates to the private `process_event` pipeline which handles:
    /// - Address filter (DashMap lookup)
    /// - Payment classification (normal vs exception)
    /// - DB writes (transactions, payment_events, balance updates)
    pub async fn ingest_external_event(
        &self,
        event: &IndexerTransferEvent,
        current_block: i64,
    ) -> Result<()> {
        self.process_event(event, current_block).await
    }

    /// Get a reference to the shared address cache (DashMap).
    ///
    /// Used by `SolanaIndexer` to sync its ATA cache with addresses
    /// managed by `AddressSyncManager` (LISTEN/NOTIFY + fallback).
    pub fn shared_address_cache(&self) -> Arc<DashMap<String, MonitoredAddressInfo>> {
        self.all_addresses.clone()
    }
}

pub struct IndexerStats {
    pub last_block: i64,
    pub pending_count: usize,
    pub total_addresses: usize,
}
