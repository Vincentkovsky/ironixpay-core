//! Session Expiry Worker
//!
//! Background worker that periodically expires sessions and triggers webhooks.
//! Uses per-session transactions to ensure atomicity between status update and webhook insertion.
//!
//! **UnderpaidExpired Handling**: When a session expires with partial payment (amount_received > 0),
//! creates a PaymentException for Resolution Center processing.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::entity::{payment_exceptions, webhook_events, Network};
use crate::services::checkout::{
    CheckoutService, ExpiredSessionInfo, SessionEventPayload, TransactionInfo, WebhookPricingInfo,
};
use crate::services::webhook::WebhookService;

/// Interval between expiry checks (seconds)
const EXPIRY_CHECK_INTERVAL_SECS: u64 = 60;

/// Maximum consecutive failures before circuit breaker trips
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Minimum amount (in micro-units) to create PaymentException for UnderpaidExpired
/// Below this threshold, the dust is ignored (gas fees > value)
/// 1 USDT = 1_000_000 micro-units (assuming 6 decimals)
const UNDERPAID_DUST_THRESHOLD: i64 = 1_000_000;

/// Background worker for session expiration with atomic webhook outbox
pub struct SessionExpiryWorker {
    db: DatabaseConnection,
    checkout_service: Arc<CheckoutService>,
    webhook_service: Arc<WebhookService>,
    environment: crate::entity::Environment,
}

impl SessionExpiryWorker {
    pub fn new(
        db: DatabaseConnection,
        checkout_service: Arc<CheckoutService>,
        webhook_service: Arc<WebhookService>,
        environment: crate::entity::Environment,
    ) -> Self {
        Self {
            db,
            checkout_service,
            webhook_service,
            environment,
        }
    }

    /// Run the expiry loop until cancellation or fatal error
    ///
    /// Circuit breaker: stops after MAX_CONSECUTIVE_FAILURES consecutive errors.
    pub async fn run(&self, cancel_token: CancellationToken) -> Result<()> {
        info!("Starting session expiry worker...");
        let mut failure_count = 0u32;

        loop {
            // Wait for interval or cancellation
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Session expiry worker shutdown");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(EXPIRY_CHECK_INTERVAL_SECS)) => {}
            }

            match self.expire_and_notify().await {
                Ok(_) => {
                    failure_count = 0;
                }
                Err(e) => {
                    failure_count += 1;
                    error!("Session expiry error (count: {}): {}", failure_count, e);
                    if failure_count >= MAX_CONSECUTIVE_FAILURES {
                        return Err(anyhow::anyhow!("Session expiry failed too many times"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Expire sessions and queue webhooks atomically (per-session transactions)
    ///
    /// Each session is processed in its own transaction to:
    /// 1. Isolate failures (one bad session doesn't block others)
    /// 2. Ensure atomicity (status update + webhook insert commit together)
    async fn expire_and_notify(&self) -> Result<()> {
        // Step 1: Read-only query to get candidates (no mutation yet)
        let candidates = self.checkout_service.get_expiry_candidates().await?;

        if candidates.is_empty() {
            return Ok(());
        }

        info!("Found {} session(s) to expire", candidates.len());

        let mut success_count = 0;
        let mut skipped_count = 0;

        // Step 2: Process each session in its own transaction
        for session in &candidates {
            match self.process_single_expiry(session).await {
                Ok(did_expire) => {
                    if did_expire {
                        success_count += 1;
                    } else {
                        skipped_count += 1;
                    }
                }
                Err(e) => {
                    // Log error but continue processing other sessions
                    error!(
                        session_id = %session.session_id,
                        error = %e,
                        "Failed to process session expiry, continuing with others"
                    );
                }
            }
        }

        if success_count > 0 || skipped_count > 0 {
            info!(
                success = success_count,
                skipped = skipped_count,
                total = candidates.len(),
                "Session expiry batch completed"
            );
        }

        Ok(())
    }

    /// Process a single session expiry with atomic transaction
    ///
    /// Returns:
    /// - `Ok(true)` - Session was expired and webhook queued
    /// - `Ok(false)` - Session was skipped (status already changed, e.g., paid)
    /// - `Err(_)` - Processing failed, transaction rolled back
    async fn process_single_expiry(&self, session: &ExpiredSessionInfo) -> Result<bool> {
        // Begin transaction for this session
        let txn = self.db.begin().await?;

        // Step A: Try to mark session as expired (CAS check)
        let did_expire = self
            .checkout_service
            .mark_session_expired_with_txn(&txn, session)
            .await?;

        if !did_expire {
            // CAS miss: session status changed (probably got paid) - just rollback and skip
            txn.rollback().await?;
            return Ok(false);
        }

        // Step B: Build webhook payload
        let transactions: Vec<TransactionInfo> = self
            .checkout_service
            .get_session_transactions(&session.session_id, &session.currency)
            .await
            .unwrap_or_default();

        // Pre-extract sender info for UnderpaidExpired exception (before transactions is moved)
        let underpaid_sender = if transactions.is_empty() {
            "Unknown".to_string()
        } else if transactions.len() == 1 {
            transactions[0].from_address.clone()
        } else {
            // Check if all senders are the same
            let first_sender = &transactions[0].from_address;
            if transactions.iter().all(|t| &t.from_address == first_sender) {
                first_sender.clone()
            } else {
                "Multiple".to_string()
            }
        };

        // Pre-extract tx_hash info for UnderpaidExpired exception
        // Use the last transaction's tx_hash as the primary reference
        let (underpaid_tx_hash, all_tx_hashes): (String, Vec<String>) = if transactions.is_empty() {
            (
                format!("no_transactions:{}", session.session_id),
                Vec::new(),
            )
        } else {
            let last_tx = transactions.last().unwrap();
            let all_hashes: Vec<String> = transactions.iter().map(|t| t.tx_hash.clone()).collect();
            (last_tx.tx_hash.clone(), all_hashes)
        };

        let payload = SessionEventPayload {
            object: "checkout_session",
            id: session.session_id.clone(),
            merchant_id: session.merchant_id.clone(),
            amount: crate::api::dtos::checkout::from_micro(
                session.amount_expected,
                &session.currency,
            ),
            amount_received: crate::api::dtos::checkout::from_micro(
                session.amount_received,
                &session.currency,
            ),
            fee_amount: None,
            net_amount: None,
            currency: session.currency.clone(),
            token_contract: session.currency_contract.clone(),
            network: session.network.clone(),
            livemode: Network::is_livemode_env(&self.environment),
            status: "Expired".to_string(),
            pay_address: session.pay_address.clone(),
            client_reference_id: session.client_reference_id.clone(),
            created_at: session.created_at,
            paid_at: None, // Expired sessions have no paid_at
            tx_count: transactions.len() as i32,
            transactions,
            pricing: WebhookPricingInfo {
                currency: session.pricing_currency.clone(),
                amount: session.pricing_amount.normalize().to_string(),
                exchange_rate: session.exchange_rate.to_string(),
            },
        };

        // Step C: Queue webhook in same transaction
        let event_ids = if let Some(network_enum) = Network::from_str_lenient(&session.network) {
            let environment = self.environment.clone();
            self.webhook_service
                .queue_event_with_txn(
                    &txn,
                    &session.session_id,
                    &session.merchant_id,
                    network_enum,
                    environment,
                    webhook_events::EVENT_SESSION_EXPIRED,
                    &payload,
                )
                .await?
        } else {
            // Invalid network string - this is a data issue, log and skip webhook
            warn!(
                session_id = %session.session_id,
                network = %session.network,
                "Invalid network string, skipping webhook but still expiring session"
            );
            vec![]
        };

        // Step C.5: Create UnderpaidExpired PaymentException if session had partial payment
        // This ensures funds don't get stuck - they'll be routed to Resolution Center
        if session.amount_received >= UNDERPAID_DUST_THRESHOLD {
            let exception_id = format!("pex_{}", uuid::Uuid::new_v4().simple());

            // Build notes with all transaction hashes for audit trail
            let notes = if all_tx_hashes.is_empty() {
                format!(
                    "Session expired with partial payment. Expected: {}, Received: {}. No transactions found.",
                    session.amount_expected, session.amount_received
                )
            } else {
                format!(
                    "Session expired with partial payment. Expected: {}, Received: {}. Transactions: [{}]",
                    session.amount_expected, session.amount_received, all_tx_hashes.join(", ")
                )
            };

            let exception = payment_exceptions::ActiveModel {
                id: Set(exception_id.clone()),
                network: Set(session.network.clone()),
                // Use real tx_hash from the last transaction (or synthetic if no transactions)
                tx_hash: Set(underpaid_tx_hash),
                log_index: Set(0), // Not available from TransactionInfo
                exception_type: Set(payment_exceptions::ExceptionType::UnderpaidExpired),
                to_address: Set(session.pay_address.clone()),
                from_address: Set(underpaid_sender),
                amount: Set(session.amount_received),
                currency_symbol: Set(session.currency.clone()),
                merchant_id: Set(Some(session.merchant_id.clone())),
                session_id: Set(Some(session.session_id.clone())),
                block_number: Set(0), // Not available from TransactionInfo
                block_timestamp: Set(Utc::now().into()),
                status: Set(payment_exceptions::ExceptionStatus::Pending),
                resolution: Set(None),
                resolution_ref_id: Set(None),
                resolved_at: Set(None),
                resolved_by: Set(None),
                notes: Set(Some(notes)),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
            };

            exception.insert(&txn).await?;

            info!(
                session_id = %session.session_id,
                amount_received = session.amount_received,
                exception_id = %exception_id,
                tx_count = all_tx_hashes.len(),
                "Created UnderpaidExpired PaymentException for Resolution Center"
            );
        } else if session.amount_received > 0 {
            // Below dust threshold - just log, don't create exception
            debug!(
                session_id = %session.session_id,
                amount_received = session.amount_received,
                threshold = UNDERPAID_DUST_THRESHOLD,
                "Underpaid session expired but below dust threshold, ignoring"
            );
        }

        // Step D: Commit transaction (atomic: both expire + webhook + exception succeed or fail together)
        txn.commit().await?;

        // Step E: Trigger webhook delivery AFTER commit (Outbox pattern)
        if !event_ids.is_empty() {
            self.webhook_service.trigger_delivery(&event_ids).await;
        }

        Ok(true)
    }
}
