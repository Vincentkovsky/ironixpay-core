//! Helius Webhook Receiver for Solana
//!
//! Receives Enhanced Transaction webhooks from Helius, extracts incoming
//! SPL token transfers to monitored ATAs, and feeds them into the existing
//! `TransactionIndexer.ingest_external_event` pipeline.
//!
//! # Auth
//! Validates the `Authorization` header against `HELIUS_WEBHOOK_SECRET`.
//! No JWT/API Key auth — this endpoint is called by Helius servers.
//!
//! # Design Decisions
//! - Uses `accountData[].tokenBalanceChanges[].rawTokenAmount` (string) for
//!   precision, NOT `tokenTransfers[].tokenAmount` (float).
//! - Filters: rawTokenAmount > 0 (incoming only), mint in watchlist, ATA in cache.
//! - Returns 200 immediately, processes events async via `tokio::spawn`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::services::indexer::scanner::IndexerTransferEvent;
use crate::AppState;

// ─── Helius Enhanced Webhook DTOs ───────────────────────────────────────────

/// Top-level enhanced transaction from Helius webhook payload.
/// We only parse the fields we need; Helius sends much more.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeliusEnhancedTx {
    /// Transaction signature (base58)
    signature: String,
    /// Slot number
    slot: u64,
    /// Unix timestamp (seconds)
    timestamp: i64,
    /// Per-account data including token balance changes
    #[serde(default)]
    account_data: Vec<HeliusAccountData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeliusAccountData {
    /// The account's public key
    account: String,
    /// Token balance changes for this account
    #[serde(default)]
    token_balance_changes: Vec<HeliusTokenBalanceChange>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeliusTokenBalanceChange {
    /// Token mint address
    mint: String,
    /// Raw token amount (integer string, no precision loss)
    raw_token_amount: HeliusRawAmount,
    /// The token account (ATA address)
    token_account: String,
    /// The owner of the token account (main wallet address)
    user_account: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeliusRawAmount {
    /// Number of decimals for this token
    #[allow(dead_code)]
    decimals: u8,
    /// Raw integer amount as string (positive = received, negative = sent)
    token_amount: String,
}

// ─── Shared State for Webhook Handler ───────────────────────────────────────

/// ATA → main address mapping shared between webhook handler and indexer.
/// Populated by `SolanaIndexer.sync_ata_cache()` logic at startup.
#[derive(Debug, Clone)]
pub struct AtaLookupEntry {
    /// The owner's main Solana address
    pub main_address: String,
    /// Merchant ID for this address
    pub merchant_id: String,
    /// Token symbol (e.g., "USDT")
    pub token_symbol: String,
    /// Token mint address
    pub mint_address: String,
}

/// Shared state injected into the webhook handler via Axum Extension.
/// Kept separate from AppState to avoid coupling.
#[derive(Clone)]
pub struct HeliusWebhookState {
    /// Event channel to feed into TransactionIndexer.ingest_external_event
    pub event_tx: tokio::sync::mpsc::Sender<IndexerTransferEvent>,
    /// ATA address → lookup info (main_address, merchant_id, token)
    pub ata_cache: Arc<RwLock<HashMap<String, AtaLookupEntry>>>,
    /// Mint address → token symbol watchlist
    pub watchlist: HashMap<String, String>,
    /// Expected Authorization header value
    pub webhook_secret: String,
}

// ─── Router ─────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new().route("/solana", post(handle_helius_webhook))
}

// ─── Handler ────────────────────────────────────────────────────────────────

/// Handle incoming Helius Enhanced Webhook events.
///
/// 1. Validate Authorization header
/// 2. Return 200 immediately (Helius requires fast response)
/// 3. Spawn async task to process token balance changes → IndexerTransferEvents
async fn handle_helius_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(transactions): Json<Vec<HeliusEnhancedTx>>,
) -> StatusCode {
    // 1. Extract webhook state (if Solana/Helius is configured)
    let webhook_state = match &state.helius_webhook_state {
        Some(ws) => ws.clone(),
        None => {
            warn!("Helius webhook received but no webhook state configured");
            return StatusCode::NOT_FOUND;
        }
    };

    // 2. Validate Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if auth_header != webhook_state.webhook_secret {
        warn!("Helius webhook: invalid Authorization header");
        return StatusCode::UNAUTHORIZED;
    }

    let tx_count = transactions.len();
    debug!(tx_count, "Helius webhook received");

    // 3. Return 200 immediately, process async
    let event_tx = webhook_state.event_tx.clone();
    let ata_cache = webhook_state.ata_cache.clone();
    let watchlist = webhook_state.watchlist.clone();

    tokio::spawn(async move {
        let cache = ata_cache.read().await;
        let mut events_emitted = 0u32;

        for tx in &transactions {
            for account in &tx.account_data {
                for change in &account.token_balance_changes {
                    // Filter: only mints in our watchlist
                    let token_symbol = match watchlist.get(&change.mint) {
                        Some(symbol) => symbol.clone(),
                        None => continue,
                    };

                    // Filter: only incoming transfers (positive raw amount)
                    let raw_amount: i128 =
                        change.raw_token_amount.token_amount.parse().unwrap_or(0);
                    if raw_amount <= 0 {
                        continue; // Outgoing (sweep) or zero — skip
                    }

                    // Filter: ATA must be in our cache
                    let entry = match cache.get(&change.token_account) {
                        Some(e) => e,
                        None => continue, // Not one of our monitored ATAs
                    };

                    // Verify mint matches the cached entry
                    if change.mint != entry.mint_address {
                        continue;
                    }

                    // Determine sender address
                    let from_address =
                        find_sender(&tx.account_data, &change.mint, &change.token_account);

                    let event = IndexerTransferEvent {
                        tx_hash: tx.signature.clone(),
                        from: from_address,
                        to: entry.main_address.clone(), // ATA → Main translation
                        amount: raw_amount.to_string(), // Raw integer string, no precision loss
                        event_index: 0, // Standardized for Solana (dedup consistency)
                        block_number: tx.slot as i64,
                        block_timestamp: tx.timestamp,
                        token: token_symbol,
                    };

                    if event_tx.send(event).await.is_err() {
                        tracing::error!("Helius webhook: event channel closed");
                        return;
                    }
                    events_emitted += 1;
                }
            }
        }

        if events_emitted > 0 {
            info!(
                events = events_emitted,
                txs = tx_count,
                "Helius webhook processed"
            );
        }
    });

    StatusCode::OK
}

/// Find the sender of a token transfer by looking for the account whose
/// balance DECREASED for the same mint (negative rawTokenAmount).
fn find_sender(account_data: &[HeliusAccountData], mint: &str, our_ata: &str) -> String {
    for account in account_data {
        // Skip our own ATA
        if account.account == our_ata {
            continue;
        }
        for change in &account.token_balance_changes {
            if change.mint != mint {
                continue;
            }
            let amount: i128 = change.raw_token_amount.token_amount.parse().unwrap_or(0);
            if amount < 0 {
                // This account's balance decreased — it's the sender
                // Use user_account (owner) if available, else the token account
                return change
                    .user_account
                    .clone()
                    .unwrap_or_else(|| account.account.clone());
            }
        }
    }
    "unknown".to_string()
}
