//! Helius Webhook Manager for Solana
//!
//! Manages the lifecycle of a Helius Enhanced Webhook:
//! 1. On startup: create or update the webhook with all known ATAs
//! 2. Continuously: monitor for new addresses, update webhook immediately
//! 3. Reconciliation: poll `getSignaturesForAddress` for Assigned addresses
//!    every 30 minutes as a fault-tolerance fallback

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::entity::{addresses, Addresses, Network};
use crate::services::indexer::scanner::IndexerTransferEvent;
use crate::services::solana::indexer::SolanaIndexer;
use crate::services::solana::SolanaClient;

/// Helius API base URL
const HELIUS_API_BASE: &str = "https://api-mainnet.helius-rpc.com";

/// Reconciliation interval (30 minutes)
const RECONCILIATION_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Address sync check interval (how often we check for new addresses)
const ADDRESS_SYNC_INTERVAL: Duration = Duration::from_secs(30);

/// Max signatures to fetch per reconciliation call
const RECONCILIATION_LIMIT: u32 = 10;

/// Manages the Helius webhook lifecycle and address synchronization.
pub struct HeliusWebhookManager {
    /// Helius API key
    api_key: Secret<String>,
    /// Helius webhook ID (populated after first GET/POST)
    webhook_id: Option<String>,
    /// Public URL for Helius to POST events to
    webhook_url: String,
    /// Authorization secret sent in webhook requests
    webhook_secret: String,
    /// HTTP client for Helius API calls
    http_client: reqwest::Client,
    /// Database connection for querying active addresses
    db: DatabaseConnection,
    /// Network (always Solana)
    network: Network,
    /// Token mints to derive ATAs from (e.g., USDT mint address)
    token_mints: Vec<String>,
    /// Solana RPC client (for reconciliation getSignaturesForAddress)
    solana_client: Arc<SolanaClient>,
    /// SolanaIndexer instance (for process_signature during reconciliation)
    solana_indexer: Arc<Mutex<SolanaIndexer>>,
    /// Event channel sender (for reconciliation events → TransactionIndexer)
    event_tx: tokio::sync::mpsc::Sender<IndexerTransferEvent>,
    /// Tracks registered ATA count (for detecting new addresses)
    last_registered_count: usize,
    /// Helius webhook type: "enhancedDevnet" for devnet, "enhanced" for mainnet
    webhook_type: String,
}

// ─── Helius API DTOs ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct CreateWebhookRequest {
    #[serde(rename = "webhookURL")]
    webhook_url: String,
    #[serde(rename = "transactionTypes")]
    transaction_types: Vec<String>,
    #[serde(rename = "accountAddresses")]
    account_addresses: Vec<String>,
    #[serde(rename = "webhookType")]
    webhook_type: String,
    #[serde(rename = "authHeader")]
    auth_header: String,
}

#[derive(Debug, Serialize)]
struct UpdateWebhookRequest {
    #[serde(rename = "webhookURL")]
    webhook_url: String,
    #[serde(rename = "transactionTypes")]
    transaction_types: Vec<String>,
    #[serde(rename = "accountAddresses")]
    account_addresses: Vec<String>,
    #[serde(rename = "webhookType")]
    webhook_type: String,
    #[serde(rename = "authHeader")]
    auth_header: String,
}

#[derive(Debug, Deserialize)]
struct HeliusWebhookResponse {
    #[serde(alias = "webhookID", alias = "webhookId")]
    webhook_id: String,
    #[allow(dead_code)]
    #[serde(alias = "webhookURL", alias = "webhookUrl", default)]
    webhook_url: String,
    #[serde(alias = "accountAddresses", default)]
    account_addresses: Vec<String>,
}

impl HeliusWebhookManager {
    pub fn new(
        api_key: Secret<String>,
        webhook_url: String,
        webhook_secret: String,
        db: DatabaseConnection,
        network: Network,
        token_mints: Vec<String>,
        solana_client: Arc<SolanaClient>,
        solana_indexer: Arc<Mutex<SolanaIndexer>>,
        event_tx: tokio::sync::mpsc::Sender<IndexerTransferEvent>,
        rpc_url: &str,
    ) -> Self {
        // Determine webhook type from RPC URL: devnet → enhancedDevnet, mainnet → enhanced
        let webhook_type = if rpc_url.contains("devnet") {
            "enhancedDevnet".to_string()
        } else {
            "enhanced".to_string()
        };
        info!(webhook_type = %webhook_type, rpc_url, "Helius webhook type auto-detected");

        Self {
            api_key,
            webhook_id: None,
            webhook_url,
            webhook_secret,
            http_client: reqwest::Client::new(),
            db,
            network,
            token_mints,
            solana_client,
            solana_indexer,
            event_tx,
            last_registered_count: 0,
            webhook_type,
        }
    }

    /// Main run loop. Handles:
    /// 1. Initial webhook setup
    /// 2. Address sync (check for new ATAs every 30s)
    /// 3. Reconciliation (poll Assigned addresses every 30min)
    pub async fn run(&mut self, cancel: CancellationToken) {
        // Phase 1: Initialize webhook
        match self.ensure_webhook().await {
            Ok(()) => info!(
                webhook_id = ?self.webhook_id,
                "Helius webhook initialized"
            ),
            Err(e) => {
                error!(error = %e, "Failed to initialize Helius webhook — continuing without webhook");
                // Don't return — reconciliation still works without webhook
            }
        }

        // Phase 2: Register all known ATAs
        if let Err(e) = self.sync_all_atas().await {
            error!(error = %e, "Failed to register ATAs with Helius webhook");
        }

        // Phase 3: Main loop — address sync + reconciliation
        let mut reconciliation_interval = tokio::time::interval(RECONCILIATION_INTERVAL);
        let mut address_sync_interval = tokio::time::interval(ADDRESS_SYNC_INTERVAL);
        // Skip first tick (we just synced)
        reconciliation_interval.tick().await;
        address_sync_interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("HeliusWebhookManager: shutdown");
                    break;
                }
                _ = address_sync_interval.tick() => {
                    // Keep webhook handler's ATA cache in sync with new addresses
                    // (when SolanaIndexer polling is disabled, no one else calls this)
                    {
                        let mut guard = self.solana_indexer.lock().await;
                        guard.sync_ata_cache();
                    }
                    if let Err(e) = self.check_new_addresses().await {
                        warn!(error = %e, "Address sync check failed");
                    }
                }
                _ = reconciliation_interval.tick() => {
                    if let Err(e) = self.reconciliation_loop().await {
                        warn!(error = %e, "Reconciliation loop failed");
                    }
                }
            }
        }
    }

    // ─── Webhook CRUD ───────────────────────────────────────────────────

    /// Ensure a webhook exists. GET all webhooks, find ours by URL, or create new.
    async fn ensure_webhook(&mut self) -> Result<()> {
        let url = format!(
            "{}/v0/webhooks?api-key={}",
            HELIUS_API_BASE,
            self.api_key.expose_secret()
        );

        let resp = self.http_client.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Helius GET webhooks failed: {} {}", status, body));
        }

        let webhooks: Vec<HeliusWebhookResponse> = resp.json().await?;

        // Find existing webhook by URL
        if let Some(existing) = webhooks.iter().find(|w| w.webhook_url == self.webhook_url) {
            self.webhook_id = Some(existing.webhook_id.clone());
            info!(
                webhook_id = %existing.webhook_id,
                addresses = existing.account_addresses.len(),
                "Found existing Helius webhook"
            );
            return Ok(());
        }

        // Don't create here — sync_all_atas() will create with addresses
        // (Helius requires at least 1 accountAddress)
        info!("No existing Helius webhook found — will create during ATA sync");
        Ok(())
    }

    /// Create a new Helius webhook with the given addresses.
    async fn create_webhook(&mut self, addresses: &[String]) -> Result<()> {
        let url = format!(
            "{}/v0/webhooks?api-key={}",
            HELIUS_API_BASE,
            self.api_key.expose_secret()
        );

        let body = CreateWebhookRequest {
            webhook_url: self.webhook_url.clone(),
            transaction_types: vec!["ANY".to_string()],
            account_addresses: addresses.to_vec(),
            webhook_type: self.webhook_type.clone(),
            auth_header: self.webhook_secret.clone(),
        };

        let resp = self.http_client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Helius CREATE webhook failed: {} {}",
                status,
                body_text
            ));
        }

        let created: HeliusWebhookResponse = resp.json().await?;
        info!(
            webhook_id = %created.webhook_id,
            addresses = addresses.len(),
            "Created Helius webhook"
        );
        self.webhook_id = Some(created.webhook_id);
        Ok(())
    }

    /// Update the webhook with a new set of addresses.
    async fn update_webhook_addresses(&self, addresses: Vec<String>) -> Result<()> {
        let webhook_id = self
            .webhook_id
            .as_ref()
            .ok_or_else(|| anyhow!("No webhook ID — cannot update"))?;

        let url = format!(
            "{}/v0/webhooks/{}?api-key={}",
            HELIUS_API_BASE,
            webhook_id,
            self.api_key.expose_secret()
        );

        let body = UpdateWebhookRequest {
            webhook_url: self.webhook_url.clone(),
            transaction_types: vec!["ANY".to_string()],
            account_addresses: addresses.clone(),
            webhook_type: self.webhook_type.clone(),
            auth_header: self.webhook_secret.clone(),
        };

        let resp = self.http_client.put(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Helius UPDATE webhook failed: {} {}",
                status,
                body_text
            ));
        }

        info!(
            webhook_id = %webhook_id,
            address_count = addresses.len(),
            "Updated Helius webhook addresses"
        );
        Ok(())
    }

    // ─── Address Sync ───────────────────────────────────────────────────

    /// Derive all ATAs from DB addresses + token mints (no lock contention).
    fn derive_all_atas_from_addresses(&self, db_addresses: &[addresses::Model]) -> Vec<String> {
        let mut atas = Vec::new();
        for addr in db_addresses {
            for mint in &self.token_mints {
                match crate::services::solana::derive_ata_address(
                    &addr.address,
                    mint,
                    crate::services::solana::SPL_TOKEN_PROGRAM_ID,
                ) {
                    Ok(ata) => atas.push(ata),
                    Err(e) => {
                        warn!(address = %addr.address, mint = %mint, error = %e, "Failed to derive ATA");
                    }
                }
            }
        }
        atas
    }

    /// Sync all ATAs to the Helius webhook by querying the DB directly.
    /// No lock contention — bypasses SolanaIndexer entirely.
    async fn sync_all_atas(&mut self) -> Result<()> {
        let solana_addresses = Addresses::find()
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .all(&self.db)
            .await?;

        if solana_addresses.is_empty() {
            debug!("No Solana addresses in database — skipping ATA sync");
            return Ok(());
        }

        let all_atas = self.derive_all_atas_from_addresses(&solana_addresses);
        if all_atas.is_empty() {
            debug!("No ATAs derived — skipping webhook sync");
            return Ok(());
        }

        self.last_registered_count = all_atas.len();
        info!(
            ata_count = all_atas.len(),
            address_count = solana_addresses.len(),
            "Registering ATAs with Helius webhook"
        );

        if self.webhook_id.is_some() {
            self.update_webhook_addresses(all_atas).await
        } else {
            self.create_webhook(&all_atas).await
        }
    }

    /// Check if new addresses were added and update the webhook if needed.
    async fn check_new_addresses(&mut self) -> Result<()> {
        let solana_addresses = Addresses::find()
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .all(&self.db)
            .await?;

        let all_atas = self.derive_all_atas_from_addresses(&solana_addresses);
        let current_count = all_atas.len();

        if current_count == self.last_registered_count {
            return Ok(()); // No changes
        }

        let new_count = current_count.saturating_sub(self.last_registered_count);
        info!(
            new_atas = new_count,
            total = current_count,
            "New ATAs detected — updating Helius webhook"
        );

        self.last_registered_count = current_count;
        if self.webhook_id.is_some() {
            self.update_webhook_addresses(all_atas).await
        } else {
            self.create_webhook(&all_atas).await
        }
    }

    // ─── Reconciliation ─────────────────────────────────────────────────

    /// Low-frequency reconciliation: poll `getSignaturesForAddress` for
    /// addresses with active checkout sessions (status = Assigned).
    ///
    /// This catches any events that Helius webhook may have missed due to
    /// temporary outages or delivery failures.
    async fn reconciliation_loop(&self) -> Result<()> {
        // 1. Query database for Assigned addresses (active sessions)
        let assigned_addresses = Addresses::find()
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .filter(addresses::Column::Status.eq("Assigned"))
            .all(&self.db)
            .await?;

        if assigned_addresses.is_empty() {
            debug!("Reconciliation: no Assigned addresses to check");
            return Ok(());
        }

        info!(
            count = assigned_addresses.len(),
            "Reconciliation: checking Assigned addresses"
        );

        // 2. Lock briefly to clone ATA entries, then release
        let ata_snapshot: Vec<(String, crate::services::solana::indexer::AtaCacheEntry)> = {
            let guard = self.solana_indexer.lock().await;
            let mut entries = Vec::new();
            for addr in &assigned_addresses {
                entries.extend(
                    guard
                        .ata_cache()
                        .iter()
                        .filter(|(_, entry)| entry.main_address == addr.address)
                        .map(|(ata, entry)| (ata.clone(), entry.clone())),
                );
            }
            entries
            // guard dropped here — SolanaIndexer is free to run
        };

        // 3. Do RPC calls lock-free
        let mut events_found = 0u32;

        for (ata, entry) in &ata_snapshot {
            // getSignaturesForAddress with limit=10 (10 credits per call)
            match self
                .solana_client
                .get_signatures_for_address(ata, None, None, RECONCILIATION_LIMIT as usize)
                .await
            {
                Ok(sigs) => {
                    for sig in &sigs {
                        // Lock briefly per-signature for process_signature
                        let result = {
                            let guard = self.solana_indexer.lock().await;
                            guard.process_signature(&sig.signature, ata, entry).await
                        };
                        match result {
                            Ok(Some(event)) => {
                                if self.event_tx.send(event).await.is_err() {
                                    error!("Reconciliation: event channel closed");
                                    return Ok(());
                                }
                                events_found += 1;
                            }
                            Ok(None) => {} // Not relevant or already processed
                            Err(e) => {
                                debug!(
                                    sig = %sig.signature,
                                    error = %e,
                                    "Reconciliation: failed to process signature"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        ata = %ata,
                        error = %e,
                        "Reconciliation: getSignaturesForAddress failed"
                    );
                }
            }
        }

        if events_found > 0 {
            info!(
                events = events_found,
                addresses = assigned_addresses.len(),
                "Reconciliation completed with new events"
            );
        } else {
            debug!(
                addresses = assigned_addresses.len(),
                "Reconciliation: no new events"
            );
        }

        Ok(())
    }
}
