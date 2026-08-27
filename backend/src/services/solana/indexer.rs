//! Solana Indexer — Independent Signature-Cursor Scanning
//!
//! Unlike EVM/TRON indexers which use block-range scanning (`BlockScanner` trait),
//! Solana uses a fundamentally different approach:
//! - `getSignaturesForAddress(ATA)` with cursor-based pagination
//! - Per-ATA signature cursors (not global block pointers)
//! - ATA→Main address translation at the scanner layer
//!
//! This module provides a standalone `SolanaIndexer` that produces
//! `IndexerTransferEvent`s compatible with the existing `TransactionIndexer`.
//!
//! # Architecture
//! ```text
//! SolanaIndexer
//! ├── ata_cache: HashMap<ATA, (MainAddress, MerchantId)>
//! │   ↑ Built at startup from DB addresses × token configs
//! ├── last_seen_sigs: HashMap<ATA, String>   // per-ATA cursor
//! ├── solana_client: Arc<SolanaClient>
//! └── watchlist: Vec<(mint, symbol, program_id)>
//! ```
//!
//! # Scan Loop (not via BlockScanner trait)
//! ```text
//! loop {
//!     for each ata in ata_cache {
//!         sigs = getSignaturesForAddress(ata, until=last_seen_sig)
//!         for sig in sigs {
//!             tx = getTransaction(sig)
//!             event = parse_spl_transfer(tx, ata → main_address)
//!             → emit IndexerTransferEvent
//!         }
//!     }
//!     sleep(poll_interval)
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::services::indexer::scanner::IndexerTransferEvent;
use crate::services::indexer::MonitoredAddressInfo;
use crate::services::solana::{derive_ata_address, SolanaClient, SPL_TOKEN_PROGRAM_ID};

// ─── Configuration ──────────────────────────────────────────────────────────

/// Default polling interval between scan cycles.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Maximum signatures to fetch per `getSignaturesForAddress` call.
/// Solana RPC returns at most 1000 per call.
const SIGNATURES_PER_CALL: usize = 100;

/// Maximum pages to fetch during backfill (prevents runaway RPC usage).
const MAX_BACKFILL_PAGES: usize = 50;

// ─── ATA Cache Entry ────────────────────────────────────────────────────────

/// Cached mapping from ATA address to its owner's main address and merchant ID.
#[derive(Clone, Debug)]
pub struct AtaCacheEntry {
    /// The owner's main Solana address (what downstream sees)
    pub main_address: String,
    /// Merchant ID for this address
    pub merchant_id: String,
    /// Token symbol (e.g., "USDT")
    pub token_symbol: String,
    /// Token mint address
    pub mint_address: String,
}

// ─── SolanaIndexer ──────────────────────────────────────────────────────────

/// Independent Solana indexer using signature-cursor scanning.
///
/// Unlike `TransactionIndexer` which delegates to `BlockScanner` implementations,
/// this indexer runs its own loop because Solana's address-based signature API
/// is fundamentally incompatible with block-range scanning.
///
/// # Integration with existing system
/// - Produces `IndexerTransferEvent`s (same struct used by TRON/EVM indexers)
/// - Events are sent via a channel to the caller, which feeds them into
///   the same `process_event` pipeline
/// - ATA→Main address translation happens HERE, so downstream services
///   never see ATA addresses
pub struct SolanaIndexer {
    solana_client: Arc<SolanaClient>,
    /// ATA → (main_address, merchant_id, token_symbol, mint)
    ata_cache: HashMap<String, AtaCacheEntry>,
    /// Per-ATA signature cursor (last seen signature for incremental scanning)
    last_seen_sigs: HashMap<String, String>,
    /// Token watchlist: (mint_address, symbol)
    watchlist: Vec<(String, String)>,
    /// Polling interval between scan cycles
    poll_interval: Duration,
    /// Last processed slot (for backfill reference on restart)
    last_processed_slot: u64,
    /// Shared address cache from TransactionIndexer (updated via AddressSyncManager).
    /// SolanaIndexer reads this to discover new addresses without maintaining
    /// its own DB connection or LISTEN/NOTIFY subscription.
    shared_addresses: Arc<DashMap<String, MonitoredAddressInfo>>,
    /// Tracks last synced count for incremental ATA derivation.
    last_synced_count: usize,
    /// Optional shared ATA cache for Helius webhook handler.
    /// When set, sync_ata_cache() also updates this cache so the webhook
    /// handler can resolve ATA→main address for newly added addresses.
    helius_ata_cache:
        Option<Arc<RwLock<HashMap<String, crate::api::routes::helius_webhook::AtaLookupEntry>>>>,
}

impl SolanaIndexer {
    /// Create a new SolanaIndexer.
    ///
    /// # Arguments
    /// * `solana_client` - RPC client with failover
    /// * `watchlist` - Token watchlist: (mint_address, symbol) from ChainConfig
    /// * `poll_interval` - Override default 3s polling (None = use default)
    /// * `shared_addresses` - Shared address cache from TransactionIndexer
    /// * `helius_ata_cache` - Optional shared cache for webhook handler integration
    pub fn new(
        solana_client: Arc<SolanaClient>,
        watchlist: Vec<(String, String)>,
        poll_interval: Option<Duration>,
        shared_addresses: Arc<DashMap<String, MonitoredAddressInfo>>,
        helius_ata_cache: Option<
            Arc<RwLock<HashMap<String, crate::api::routes::helius_webhook::AtaLookupEntry>>>,
        >,
    ) -> Self {
        info!(
            tokens = ?watchlist.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            "SolanaIndexer initialized with token watchlist"
        );

        Self {
            solana_client,
            ata_cache: HashMap::new(),
            last_seen_sigs: HashMap::new(),
            watchlist,
            poll_interval: poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL),
            last_processed_slot: 0,
            shared_addresses,
            last_synced_count: 0,
            helius_ata_cache,
        }
    }

    /// Hydrate the ATA cache from a list of monitored addresses.
    ///
    /// For each address × each token in watchlist, compute the ATA and
    /// store the ATA → (main_address, merchant_id) mapping.
    ///
    /// Called at startup and when new addresses are added via LISTEN/NOTIFY.
    pub fn add_addresses(&mut self, addresses: &[(String, String)]) {
        for (main_address, merchant_id) in addresses {
            for (mint, symbol) in &self.watchlist {
                match derive_ata_address(main_address, mint, SPL_TOKEN_PROGRAM_ID) {
                    Ok(ata) => {
                        self.ata_cache.insert(
                            ata,
                            AtaCacheEntry {
                                main_address: main_address.clone(),
                                merchant_id: merchant_id.clone(),
                                token_symbol: symbol.clone(),
                                mint_address: mint.clone(),
                            },
                        );
                    }
                    Err(e) => {
                        warn!(
                            address = %main_address,
                            mint = %mint,
                            error = %e,
                            "Failed to derive ATA — skipping"
                        );
                    }
                }
            }
        }

        info!(
            ata_count = self.ata_cache.len(),
            address_count = addresses.len(),
            tokens = self.watchlist.len(),
            "ATA cache hydrated"
        );
    }

    /// Remove addresses from the ATA cache (e.g., when addresses are recycled).
    pub fn remove_addresses(&mut self, main_addresses: &[String]) {
        for main_address in main_addresses {
            self.ata_cache
                .retain(|_, entry| entry.main_address != *main_address);
            // Also clean up cursors for removed ATAs
            self.last_seen_sigs
                .retain(|ata, _| self.ata_cache.contains_key(ata));
        }
    }

    /// Set the last processed slot (loaded from `indexer_state` table on restart).
    pub fn set_last_processed_slot(&mut self, slot: u64) {
        self.last_processed_slot = slot;
    }

    /// Get the last processed slot (for persisting to `indexer_state` table).
    pub fn last_processed_slot(&self) -> u64 {
        self.last_processed_slot
    }

    /// Get a reference to the ATA cache (for Helius webhook handler integration).
    pub fn ata_cache(&self) -> &HashMap<String, AtaCacheEntry> {
        &self.ata_cache
    }

    /// Number of ATAs being monitored.
    pub fn monitored_ata_count(&self) -> usize {
        self.ata_cache.len()
    }

    /// Run the main scanning loop.
    ///
    /// Scans all monitored ATAs for new signatures, parses SPL transfers,
    /// and sends `IndexerTransferEvent`s via the provided channel.
    ///
    /// # Cancellation
    /// Respects `CancellationToken` for graceful shutdown.
    pub async fn run(
        &mut self,
        event_tx: tokio::sync::mpsc::Sender<IndexerTransferEvent>,
        cancel: CancellationToken,
    ) {
        info!(
            ata_count = self.ata_cache.len(),
            poll_interval_ms = self.poll_interval.as_millis(),
            "Starting Solana indexer loop"
        );

        loop {
            if cancel.is_cancelled() {
                info!("Solana indexer: shutdown requested");
                break;
            }

            match self.scan_cycle(&event_tx).await {
                Ok(events_found) => {
                    if events_found > 0 {
                        debug!(events_found, "Solana scan cycle completed");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Solana scan cycle failed");
                    // Back off on error to avoid hammering a failing RPC
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(self.poll_interval) => {}
                _ = cancel.cancelled() => {
                    info!("Solana indexer: shutdown during sleep");
                    break;
                }
            }
        }
    }

    /// Sync ATA cache from the shared address DashMap.
    ///
    /// Called at the start of each scan cycle to pick up new addresses
    /// added via LISTEN/NOTIFY → AddressSyncManager → DashMap.
    /// Only derives ATAs for addresses not already in the cache.
    pub fn sync_ata_cache(&mut self) {
        let current_count = self.shared_addresses.len();
        debug!(
            current_count,
            last_synced = self.last_synced_count,
            ata_cache_len = self.ata_cache.len(),
            "sync_ata_cache check"
        );
        if current_count == self.last_synced_count {
            return; // No changes
        }

        // Build set of main addresses already known
        let known_mains: HashSet<String> = self
            .ata_cache
            .values()
            .map(|e| e.main_address.clone())
            .collect();

        let mut new_count = 0usize;
        for entry in self.shared_addresses.iter() {
            let main_addr = entry.key();
            if known_mains.contains(main_addr) {
                continue;
            }
            for (mint, symbol) in &self.watchlist {
                match derive_ata_address(main_addr, mint, SPL_TOKEN_PROGRAM_ID) {
                    Ok(ata) => {
                        self.ata_cache.insert(
                            ata,
                            AtaCacheEntry {
                                main_address: main_addr.clone(),
                                merchant_id: entry.value().merchant_id.clone(),
                                token_symbol: symbol.clone(),
                                mint_address: mint.clone(),
                            },
                        );
                        new_count += 1;
                    }
                    Err(e) => {
                        warn!(
                            address = %main_addr,
                            mint = %mint,
                            error = %e,
                            "Failed to derive ATA during sync — skipping"
                        );
                    }
                }
            }
        }

        if new_count > 0 {
            info!(
                new_atas = new_count,
                total_atas = self.ata_cache.len(),
                "ATA cache synced from shared addresses"
            );

            // Sync new entries to the shared Helius webhook cache
            if let Some(ref helius_cache) = self.helius_ata_cache {
                let cache_clone = helius_cache.clone();
                let ata_snapshot: Vec<(String, AtaCacheEntry)> = self
                    .ata_cache
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                // Spawn to avoid holding SolanaIndexer lock during async write
                tokio::spawn(async move {
                    let mut cache = cache_clone.write().await;
                    for (ata_addr, entry) in ata_snapshot {
                        cache.entry(ata_addr).or_insert_with(|| {
                            crate::api::routes::helius_webhook::AtaLookupEntry {
                                main_address: entry.main_address,
                                merchant_id: entry.merchant_id,
                                token_symbol: entry.token_symbol,
                                mint_address: entry.mint_address,
                            }
                        });
                    }
                    debug!(cache_size = cache.len(), "Helius webhook ATA cache synced");
                });
            }
        }
        self.last_synced_count = current_count;
    }

    /// Execute one scan cycle across all monitored ATAs.
    ///
    /// Returns the number of events found.
    ///
    /// Uses RPC `until` parameter to let the server filter out already-seen
    /// signatures. This is critical — without it, if >SIGNATURES_PER_CALL
    /// new sigs arrive between polls, we'd miss the ones beyond page 1.
    async fn scan_cycle(
        &mut self,
        event_tx: &tokio::sync::mpsc::Sender<IndexerTransferEvent>,
    ) -> Result<usize> {
        // Sync ATA cache from shared DashMap (picks up new addresses)
        self.sync_ata_cache();

        let mut total_events = 0;

        // Collect ATAs to scan (avoid borrowing self.ata_cache during iteration)
        let atas: Vec<(String, AtaCacheEntry)> = self
            .ata_cache
            .iter()
            .map(|(ata, entry)| (ata.clone(), entry.clone()))
            .collect();

        for (ata, entry) in &atas {
            let until_sig = self.last_seen_sigs.get(ata).cloned();

            // First run ever (no cursor, no checkpoint) — fall through to
            // fetch_new_signatures, which uses slot-based filtering when no cursor.
            // We deliberately DO NOT skip processing here: payments that arrive
            // between backend start and the first scan cycle must be detected.
            // The slot filter in fetch_new_signatures prevents re-processing old history.

            // Fetch new signatures with RPC-side `until` filtering.
            // If we have a cursor, the RPC only returns sigs NEWER than `until`.
            // If no cursor but have a checkpoint, fetch all and filter locally by slot.
            let all_new_sigs = match self.fetch_new_signatures(ata, until_sig.as_deref()).await {
                Ok(sigs) => sigs,
                Err(e) => {
                    warn!(ata = %ata, error = %e, "Failed to fetch signatures — will retry next cycle");
                    continue;
                }
            };

            if all_new_sigs.is_empty() {
                continue;
            }

            // Update cursor to the newest signature BEFORE processing
            if let Some(newest) = all_new_sigs.first() {
                self.last_seen_sigs
                    .insert(ata.clone(), newest.signature.clone());
                self.last_processed_slot = self.last_processed_slot.max(newest.slot);
            }

            // Process signatures in chronological order (oldest first)
            for sig_info in all_new_sigs.iter().rev() {
                if sig_info.err.is_some() {
                    debug!(signature = %sig_info.signature, "Skipping failed transaction");
                    continue;
                }

                match self
                    .process_signature(&sig_info.signature, ata, entry)
                    .await
                {
                    Ok(Some(event)) => {
                        if event_tx.send(event).await.is_err() {
                            warn!("Event channel closed — stopping indexer");
                            return Err(anyhow!("Event channel closed"));
                        }
                        total_events += 1;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!(
                            signature = %sig_info.signature,
                            error = %e,
                            "Failed to process signature — skipping"
                        );
                    }
                }
            }
        }

        Ok(total_events)
    }

    /// Fetch all new signatures for an ATA, paginating if necessary.
    ///
    /// When `until_sig` is set, uses RPC `until` parameter for server-side
    /// filtering. If the result set is exactly SIGNATURES_PER_CALL, paginates
    /// backward using `before` to collect remaining new signatures.
    ///
    /// When `until_sig` is None (first run with checkpoint), fetches and
    /// filters locally by slot.
    async fn fetch_new_signatures(
        &self,
        ata: &str,
        until_sig: Option<&str>,
    ) -> Result<Vec<crate::services::solana::types::SignatureInfo>> {
        use crate::services::solana::types::SignatureInfo;

        let mut all_sigs: Vec<SignatureInfo> = Vec::new();
        let mut before: Option<String> = None;
        let mut pages = 0;

        loop {
            if pages >= MAX_BACKFILL_PAGES {
                warn!(ata = %ata, pages, "scan_cycle pagination limit reached");
                break;
            }

            let sigs = self
                .solana_client
                .get_signatures_for_address(ata, before.as_deref(), until_sig, SIGNATURES_PER_CALL)
                .await?;

            let batch_len = sigs.len();
            if batch_len == 0 {
                break;
            }

            // If no cursor, filter by slot checkpoint
            if until_sig.is_none() {
                let filtered: Vec<_> = sigs
                    .into_iter()
                    .filter(|s| s.slot > self.last_processed_slot)
                    .collect();
                if filtered.is_empty() {
                    break;
                }
                let need_next_page = filtered.len() == batch_len;
                if let Some(oldest) = filtered.last() {
                    before = Some(oldest.signature.clone());
                }
                all_sigs.extend(filtered);
                if !need_next_page {
                    break;
                }
            } else {
                // RPC already filtered by `until` — just collect
                if let Some(oldest) = sigs.last() {
                    before = Some(oldest.signature.clone());
                }
                let need_next_page = batch_len == SIGNATURES_PER_CALL;
                all_sigs.extend(sigs);
                if !need_next_page {
                    break; // Got all new sigs
                }
            }

            pages += 1;
        }

        Ok(all_sigs)
    }

    /// Process a single transaction signature.
    ///
    /// Fetches the full transaction, parses pre/post token balances
    /// to detect incoming SPL transfers, and constructs an IndexerTransferEvent.
    ///
    /// Returns `None` if the transaction is not a relevant incoming transfer.
    pub async fn process_signature(
        &self,
        signature: &str,
        ata: &str,
        entry: &AtaCacheEntry,
    ) -> Result<Option<IndexerTransferEvent>> {
        let tx = self
            .solana_client
            .get_transaction(signature)
            .await?
            .ok_or_else(|| anyhow!("Transaction not found: {}", signature))?;

        let meta = tx
            .meta
            .as_ref()
            .ok_or_else(|| anyhow!("Transaction has no metadata: {}", signature))?;

        // Skip failed transactions (double-check, should already be filtered)
        if meta.err.is_some() {
            return Ok(None);
        }

        // Find the account index for our ATA in the transaction's account keys
        let ata_account_index = tx
            .transaction
            .message
            .account_keys
            .iter()
            .position(|k| k.pubkey == ata);

        let ata_idx = match ata_account_index {
            Some(idx) => idx as u8,
            None => return Ok(None), // Our ATA not involved in this tx
        };

        // Compare pre/post token balances for our ATA to detect incoming transfers
        let pre_balances = meta.pre_token_balances.as_deref().unwrap_or(&[]);
        let post_balances = meta.post_token_balances.as_deref().unwrap_or(&[]);

        // Find pre-balance for our ATA + mint
        let pre_amount: u64 = pre_balances
            .iter()
            .find(|b| b.account_index == ata_idx && b.mint == entry.mint_address)
            .map(|b| b.ui_token_amount.amount.parse::<u64>().unwrap_or(0))
            .unwrap_or(0);

        // Find post-balance for our ATA + mint
        let post_amount: u64 = post_balances
            .iter()
            .find(|b| b.account_index == ata_idx && b.mint == entry.mint_address)
            .map(|b| b.ui_token_amount.amount.parse::<u64>().unwrap_or(0))
            .unwrap_or(0);

        // Only process INCOMING transfers (post > pre)
        if post_amount <= pre_amount {
            return Ok(None);
        }

        let transfer_amount = post_amount - pre_amount;

        // Identify the sender from pre_token_balances:
        // Find the token account whose balance DECREASED for the same mint.
        // Use its `owner` field (the real token owner), not the tx signer
        // (which could be a relay, DEX router, or multi-sig).
        let from_address = pre_balances
            .iter()
            .zip(post_balances.iter())
            .filter(|(pre, post)| {
                pre.mint == entry.mint_address
                    && post.mint == entry.mint_address
                    && pre.account_index == post.account_index
                    && pre.account_index != ata_idx // Not our own ATA
            })
            .find_map(|(pre, post)| {
                let pre_amt = pre.ui_token_amount.amount.parse::<u64>().unwrap_or(0);
                let post_amt = post.ui_token_amount.amount.parse::<u64>().unwrap_or(0);
                if pre_amt > post_amt {
                    // Balance decreased — this is the source
                    pre.owner.clone()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                // Fallback: first signer that isn't our address
                tx.transaction
                    .message
                    .account_keys
                    .iter()
                    .find(|k| k.signer && k.pubkey != entry.main_address)
                    .map(|k| k.pubkey.clone())
                    .unwrap_or_else(|| "unknown".to_string())
            });

        let block_timestamp = tx.block_time.unwrap_or(0);

        debug!(
            signature,
            from = %from_address,
            to = %entry.main_address,
            amount = transfer_amount,
            token = %entry.token_symbol,
            slot = tx.slot,
            "Detected incoming SPL transfer"
        );

        // Construct IndexerTransferEvent with ATA→Main address translation
        // Downstream services see the MAIN address, never the ATA
        Ok(Some(IndexerTransferEvent {
            tx_hash: signature.to_string(),
            from: from_address,
            to: entry.main_address.clone(), // ← ATA→Main translation happens HERE
            amount: transfer_amount.to_string(),
            event_index: 0, // Standardized for Solana (dedup consistency with webhook handler)
            block_number: tx.slot as i64,
            block_timestamp,
            token: entry.token_symbol.clone(),
        }))
    }

    /// Backfill missed transactions on restart.
    ///
    /// Uses `last_processed_slot` as the cutoff — fetches all signatures
    /// with slot > last_processed_slot, paginating if necessary.
    ///
    /// # Important
    /// `getSignaturesForAddress` returns at most 1000 per call. If the service
    /// was down long enough for >1000 txs per ATA, this method paginates
    /// using the `before` parameter (up to MAX_BACKFILL_PAGES pages).
    pub async fn backfill(
        &mut self,
        event_tx: &tokio::sync::mpsc::Sender<IndexerTransferEvent>,
    ) -> Result<usize> {
        if self.last_processed_slot == 0 {
            info!("No checkpoint — skipping backfill, will start from current tip");
            return Ok(0);
        }

        info!(
            last_slot = self.last_processed_slot,
            ata_count = self.ata_cache.len(),
            "Starting Solana backfill"
        );

        let mut total_events = 0;

        let atas: Vec<(String, AtaCacheEntry)> = self
            .ata_cache
            .iter()
            .map(|(ata, entry)| (ata.clone(), entry.clone()))
            .collect();

        for (ata, entry) in &atas {
            let mut before_sig: Option<String> = None;
            let mut page = 0;

            loop {
                if page >= MAX_BACKFILL_PAGES {
                    warn!(
                        ata = %ata,
                        pages = page,
                        "Backfill pagination limit reached"
                    );
                    break;
                }

                let signatures = self
                    .solana_client
                    .get_signatures_for_address(
                        ata,
                        before_sig.as_deref(),
                        None, // No `until` — we filter by slot locally
                        1000, // Max per call for backfill
                    )
                    .await?;

                if signatures.is_empty() {
                    break;
                }

                // Filter to signatures newer than our checkpoint
                let relevant: Vec<_> = signatures
                    .iter()
                    .filter(|s| s.slot > self.last_processed_slot)
                    .collect();

                // If no relevant sigs in this page, we've gone past our checkpoint
                if relevant.is_empty() {
                    break;
                }

                // Process relevant signatures (oldest first)
                for sig_info in relevant.iter().rev() {
                    if sig_info.err.is_some() {
                        continue;
                    }

                    match self
                        .process_signature(&sig_info.signature, ata, entry)
                        .await
                    {
                        Ok(Some(event)) => {
                            if event_tx.send(event).await.is_err() {
                                return Err(anyhow!("Event channel closed during backfill"));
                            }
                            total_events += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!(
                                signature = %sig_info.signature,
                                error = %e,
                                "Backfill: failed to process signature"
                            );
                        }
                    }
                }

                // Update cursor to newest signature
                if page == 0 {
                    if let Some(newest) = signatures.first() {
                        self.last_seen_sigs
                            .insert(ata.clone(), newest.signature.clone());
                        if newest.slot > self.last_processed_slot {
                            self.last_processed_slot = newest.slot;
                        }
                    }
                }

                // Check if we need another page
                // (all sigs in this batch are still newer than checkpoint)
                let oldest_sig_slot = signatures.last().map(|s| s.slot).unwrap_or(0);
                if oldest_sig_slot <= self.last_processed_slot {
                    break; // We've reached the checkpoint
                }

                // Set up next page cursor
                before_sig = signatures.last().map(|s| s.signature.clone());
                page += 1;
            }
        }

        info!(total_events, "Solana backfill completed");

        Ok(total_events)
    }
}
