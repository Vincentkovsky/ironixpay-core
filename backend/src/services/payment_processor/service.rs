//! Payment Event Processor Service
//!
//! Consumes payment events from the outbox table and updates session status.
//! Implements exponential backoff retry with jitter and dead letter handling.
//!
//! **Architecture**: This is the "hands" - it fetches events and delivers them.
//! The "brain" (business logic) lives in `CheckoutService::apply_payment`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rand::Rng;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Set, Statement,
    TransactionTrait,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::entity::{payment_events, webhook_events, CheckoutSessions, Network, PaymentEvents};
use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::aml::{AmlService, RiskResult};
use crate::services::checkout::CheckoutService;
use crate::services::sse::SseBroadcaster;
use crate::services::webhook::WebhookService;

/// Default underpayment tolerance (0.01 USDT = 10000 in raw units)
const DEFAULT_UNDERPAYMENT_THRESHOLD: i64 = 10000;

/// Maximum number of retry attempts
const MAX_RETRY_ATTEMPTS: i32 = 7;

/// Batch size for polling
const BATCH_SIZE: i32 = 100;

/// Stale event timeout in seconds (events stuck in processing)
/// Reduced from 5 minutes to 60 seconds for faster recovery on pod crashes.
/// Since handle_event executes in milliseconds, 60s provides sufficient margin
/// while minimizing payment processing delays during failures.
const STALE_TIMEOUT_SECS: i64 = 60; // 1 minute

/// Payment Event Processor
///
/// Responsible for consuming payment events from the outbox table.
/// Delegates actual session updates to `CheckoutService::apply_payment`.
///
/// Note: Sweep triggering is now handled by the Sweeper service's polling cycle,
/// not by this processor. The Sweeper checks session status and sweeps when appropriate.
pub struct PaymentEventProcessor {
    db: DatabaseConnection,
    checkout_service: Arc<CheckoutService>,
    webhook_service: Arc<WebhookService>,
    aml_service: Arc<AmlService>,
    billing_service: Arc<crate::services::billing::BillingService>,
    fee_config: Arc<crate::services::billing::fee_config::FeeConfig>,
    sse_broadcaster: Option<Arc<SseBroadcaster>>,
    cancel_token: CancellationToken,
    underpayment_threshold: i64,
    environment: crate::entity::Environment,
    alerting_service: Arc<AlertingService>,
    /// Per-chain deposit fee floor overrides from chains.toml.
    /// Key: Network, Value: floor in USDT microunits.
    /// Missing networks fall back to FeeConfig::floor_deposit.
    chain_deposit_floors: HashMap<crate::entity::Network, i64>,
    /// Optional heartbeat reporter for /ready and admin health monitoring.
    service_health: Option<(
        crate::services::service_health::ServiceHealthRegistry,
        String,
    )>,
    /// Optional Xero accounting integration — enqueues sync on completed sessions.
    xero_service: Option<Arc<crate::services::xero::XeroService>>,
}

impl PaymentEventProcessor {
    pub fn new(
        db: DatabaseConnection,
        checkout_service: Arc<CheckoutService>,
        webhook_service: Arc<WebhookService>,
        aml_service: Arc<AmlService>,
        billing_service: Arc<crate::services::billing::BillingService>,
        fee_config: Arc<crate::services::billing::fee_config::FeeConfig>,
        environment: crate::entity::Environment,
        alerting_service: Arc<AlertingService>,
    ) -> Self {
        Self::with_sse(
            db,
            checkout_service,
            webhook_service,
            aml_service,
            billing_service,
            fee_config,
            None,
            environment,
            alerting_service,
            HashMap::new(),
        )
    }

    /// Create processor with SSE broadcaster for real-time updates
    pub fn with_sse(
        db: DatabaseConnection,
        checkout_service: Arc<CheckoutService>,
        webhook_service: Arc<WebhookService>,
        aml_service: Arc<AmlService>,
        billing_service: Arc<crate::services::billing::BillingService>,
        fee_config: Arc<crate::services::billing::fee_config::FeeConfig>,
        sse_broadcaster: Option<Arc<SseBroadcaster>>,
        environment: crate::entity::Environment,
        alerting_service: Arc<AlertingService>,
        chain_deposit_floors: HashMap<crate::entity::Network, i64>,
    ) -> Self {
        Self::with_config(
            db,
            checkout_service,
            webhook_service,
            aml_service,
            billing_service,
            fee_config,
            sse_broadcaster,
            DEFAULT_UNDERPAYMENT_THRESHOLD,
            environment,
            alerting_service,
            chain_deposit_floors,
        )
    }

    /// Create processor with custom configuration
    pub fn with_config(
        db: DatabaseConnection,
        checkout_service: Arc<CheckoutService>,
        webhook_service: Arc<WebhookService>,
        aml_service: Arc<AmlService>,
        billing_service: Arc<crate::services::billing::BillingService>,
        fee_config: Arc<crate::services::billing::fee_config::FeeConfig>,
        sse_broadcaster: Option<Arc<SseBroadcaster>>,
        underpayment_threshold: i64,
        environment: crate::entity::Environment,
        alerting_service: Arc<AlertingService>,
        chain_deposit_floors: HashMap<crate::entity::Network, i64>,
    ) -> Self {
        Self {
            db,
            checkout_service,
            webhook_service,
            aml_service,
            billing_service,
            fee_config,
            sse_broadcaster,
            cancel_token: CancellationToken::new(),
            underpayment_threshold,
            environment,
            alerting_service,
            chain_deposit_floors,
            service_health: None,
            xero_service: None,
        }
    }

    /// Attach heartbeat reporting for service health monitoring.
    pub fn with_health(
        mut self,
        registry: crate::services::service_health::ServiceHealthRegistry,
        service_name: String,
    ) -> Self {
        self.service_health = Some((registry, service_name));
        self
    }

    /// Attach optional Xero accounting integration.
    pub fn with_xero(
        mut self,
        xero_service: Option<Arc<crate::services::xero::XeroService>>,
    ) -> Self {
        self.xero_service = xero_service;
        self
    }

    /// Get a clone of the cancellation token for graceful shutdown
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Start the event processor loop (runs in background)
    ///
    /// The loop continues until the cancellation token is triggered.
    pub async fn start(self: Arc<Self>, token: CancellationToken) -> Result<()> {
        info!("Starting payment event processor");

        loop {
            // Check for shutdown signal
            if token.is_cancelled() {
                info!("Payment event processor received shutdown signal");
                break;
            }

            // Reclaim stale events first
            if let Err(e) = self.reclaim_stale_events().await {
                error!(error = %e, "Failed to reclaim stale events");
            }

            // Process pending events
            match self.process_pending_events().await {
                Ok(count) if count > 0 => {
                    debug!(count, "Processed payment events");
                    if let Some((ref reg, ref name)) = self.service_health {
                        reg.heartbeat(name);
                    }
                    continue; // Immediately process next batch
                }
                Ok(_) => {
                    // No events to process — still alive, heartbeat
                    if let Some((ref reg, ref name)) = self.service_health {
                        reg.heartbeat(name);
                    }
                    // Sleep longer with cancellation support
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                        _ = token.cancelled() => {
                            info!("Payment event processor shutdown during idle");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error processing payment events");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }

            // Removed mandatory 200ms sleep here to allow burst processing
            // The loop will now spin as long as there is work (via continue)
            // or sleep for 500ms when idle.
        }

        info!("Payment event processor stopped gracefully");
        Ok(())
    }

    /// Fetch and lock pending events atomically
    async fn fetch_pending_events(&self) -> Result<Vec<payment_events::Model>> {
        // Use raw SQL for atomic UPDATE ... RETURNING with SKIP LOCKED
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE payment_events
                SET status = 'processing', updated_at = NOW()
                WHERE id IN (
                    SELECT id FROM payment_events
                    WHERE status = 'pending' AND next_retry_at <= NOW()
                    ORDER BY created_at ASC
                    LIMIT $1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING
                    id, event_type, session_id, tx_network, tx_hash, tx_log_index,
                    amount, status, attempt_count, next_retry_at, error_message,
                    created_at, updated_at, processed_at
                "#,
                [BATCH_SIZE.into()],
            ))
            .await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_type_str: String = row.try_get("", "event_type")?;
            let event_type = match event_type_str.as_str() {
                "payment_detected" => payment_events::PaymentEventType::PaymentDetected,
                "payment_confirmed" => payment_events::PaymentEventType::PaymentConfirmed,
                unknown => {
                    // Skip unknown event types (fault tolerance for Tron dirty data)
                    error!(
                        "Skipping event id={:?} with unknown type: {}",
                        row.try_get::<String>("", "id")
                            .unwrap_or_else(|_| "unknown".to_string()),
                        unknown
                    );
                    continue;
                }
            };

            let event = payment_events::Model {
                id: row.try_get("", "id")?,
                event_type,
                session_id: row.try_get("", "session_id")?,
                tx_network: row.try_get("", "tx_network")?,
                tx_hash: row.try_get("", "tx_hash")?,
                tx_log_index: row.try_get("", "tx_log_index")?,
                amount: row.try_get("", "amount")?,
                status: payment_events::PaymentEventStatus::Processing,
                attempt_count: row.try_get("", "attempt_count")?,
                next_retry_at: row.try_get("", "next_retry_at")?,
                error_message: row.try_get("", "error_message")?,
                created_at: row.try_get("", "created_at")?,
                updated_at: row.try_get("", "updated_at")?,
                processed_at: row.try_get("", "processed_at")?,
            };
            events.push(event);
        }

        Ok(events)
    }

    /// Process a batch of pending events
    async fn process_pending_events(&self) -> Result<usize> {
        let events = self.fetch_pending_events().await?;

        if events.is_empty() {
            return Ok(0);
        }

        let count = events.len();

        for event in events {
            match self.handle_event(&event).await {
                Ok(Some(session)) => {
                    // Payment successful and fully processed.
                    // Webhooks and Sweeps are triggered inside handle_event (post-commit)
                    crate::services::metrics::inc_events_processed("success");
                    debug!(session_id = %session.id, "Payment processed successfully");
                }
                Ok(None) => {
                    // Event was skipped (non-confirmed) or already processed
                }
                Err(e) => {
                    crate::services::metrics::inc_events_processed("failed");
                    error!(
                        event_id = %event.id,
                        session_id = %event.session_id,
                        error = %e,
                        "Failed to process payment event"
                    );
                    self.handle_event_failure(&event, &e.to_string()).await?;
                }
            }
        }

        Ok(count)
    }

    /// Enqueue a sweep task for durable processing
    ///
    /// Instead of fire-and-forget tokio::spawn, we update the address status
    /// to trigger the Sweeper's regular polling cycle. This ensures sweep
    /// requests survive process restarts.
    async fn enqueue_sweep_task(&self, network: &str, address: &str) {
        // The address should already be in 'Detected' state from the indexer.
        // The Sweeper service polls for Detected addresses with successful sessions.
        // We just log here - the Sweeper will pick it up on next cycle.
        info!(
            network = %network,
            address = %address,
            "Sweep eligible - will be processed by Sweeper service"
        );

        // Note: The Sweeper service (sweeper/mod.rs) already implements:
        // 1. broadcast_cycle() - finds Detected addresses and initiates sweeps
        // 2. confirmation_cycle() - monitors pending sweep transactions
        // 3. recycle_cycle() - returns Cooling addresses to Idle
        //
        // The address is already marked as Detected by the indexer, and the
        // session status is now Paid/Overpaid. The Sweeper will find it.
    }

    /// Handle a single payment event
    ///
    /// **Idempotency Guarantee**: This method is safe to call multiple times
    /// for the same event. Idempotency is ensured by:
    /// 1. Checking if event was already processed
    /// 2. `CheckoutService::apply_payment` checks terminal state
    /// 3. Checking transaction table for already credited amounts
    ///
    /// **Atomicity**: Session update and event status update are performed
    /// within a single database transaction to prevent inconsistent states.
    ///
    /// Returns `Some(session)` if payment was successful and sweep should be triggered.
    async fn handle_event(
        &self,
        event: &payment_events::Model,
    ) -> Result<Option<crate::entity::checkout_sessions::Model>> {
        info!(
            event_id = %event.id,
            event_type = ?event.event_type,
            session_id = %event.session_id,
            tx_hash = %event.tx_hash,
            amount = event.amount,
            "Processing payment event"
        );

        // Handle PaymentDetected events (0-confirmation)
        // These are informational and don't modify session balance.
        if event.event_type == payment_events::PaymentEventType::PaymentDetected {
            debug!(event_id = %event.id, "Processing PaymentDetected event");

            let detected_amount = match CheckoutSessions::find_by_id(&event.session_id)
                .one(&self.db)
                .await
            {
                Ok(Some(session)) => {
                    crate::api::dtos::checkout::from_micro(event.amount, &session.currency)
                }
                Ok(None) => {
                    warn!(
                        session_id = %event.session_id,
                        "Session not found while formatting PaymentDetected amount, falling back to USDT decimals"
                    );
                    crate::api::dtos::checkout::from_micro(event.amount, "USDT")
                }
                Err(e) => {
                    warn!(
                        session_id = %event.session_id,
                        error = %e,
                        "Failed to load session currency for PaymentDetected amount, falling back to USDT decimals"
                    );
                    crate::api::dtos::checkout::from_micro(event.amount, "USDT")
                }
            };

            // Broadcast to SSE clients for real-time checkout feedback
            if let Some(ref broadcaster) = self.sse_broadcaster {
                use crate::services::sse::SseEvent;
                broadcaster.broadcast(
                    &event.session_id,
                    SseEvent::PaymentDetected {
                        tx_hash: event.tx_hash.clone(),
                        amount: detected_amount,
                    },
                );
                debug!(
                    session_id = %event.session_id,
                    tx_hash = %event.tx_hash,
                    "SSE PaymentDetected broadcast sent"
                );
            }

            self.mark_event_processed(&event.id).await?;
            return Ok(None);
        }

        // Only PaymentConfirmed events modify session balance
        if event.event_type != payment_events::PaymentEventType::PaymentConfirmed {
            // Unknown event type - log and skip
            warn!(event_id = %event.id, event_type = ?event.event_type, "Unknown event type, skipping");
            self.mark_event_processed(&event.id).await?;
            return Ok(None);
        }

        // ============================================================
        // Begin atomic transaction for idempotency checks + state updates
        // ============================================================
        let txn = self.db.begin().await?;

        // ============================================================
        // IDEMPOTENCY CHECK 1: Verify this event hasn't been processed
        // ============================================================
        let already_processed = txn
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 1 FROM payment_events
                WHERE tx_network = $1 AND tx_hash = $2 AND tx_log_index = $3
                  AND event_type = 'payment_confirmed'
                  AND status = 'processed'
                "#,
                [
                    event.tx_network.clone().into(),
                    event.tx_hash.clone().into(),
                    event.tx_log_index.into(),
                ],
            ))
            .await?;

        if already_processed.is_some() {
            info!(
                event_id = %event.id,
                tx_hash = %event.tx_hash,
                "Event already processed (idempotency check), skipping"
            );
            txn.rollback().await?;
            return Ok(None);
        }

        // ============================================================
        // IDEMPOTENCY CHECK 2: Check if THIS SPECIFIC transaction is already credited
        // Using unique fingerprint (network, tx_hash, log_index) + is_credited flag
        // ============================================================
        let tx_is_credited = txn
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT is_credited, from_address FROM transactions
                WHERE network = $1 AND tx_hash = $2 AND log_index = $3
                FOR UPDATE
                "#,
                [
                    event.tx_network.clone().into(),
                    event.tx_hash.clone().into(),
                    event.tx_log_index.into(),
                ],
            ))
            .await?;

        if let Some(row) = tx_is_credited {
            let is_credited: bool = row.try_get("", "is_credited").unwrap_or(false);
            let from_address: String = row.try_get("", "from_address").unwrap_or_default();

            if is_credited {
                // This specific transaction has already been credited to the session.
                // Mark event as processed and skip.
                info!(
                    event_id = %event.id,
                    tx_hash = %event.tx_hash,
                    log_index = event.tx_log_index,
                    "Transaction already credited (is_credited=true), skipping"
                );
                self.mark_event_processed_with_txn(&txn, &event.id).await?;
                txn.commit().await?;
                return Ok(None);
            }

            // ============================================================
            // AML GATEKEEPER: Check sender address before applying payment
            // ============================================================
            if !from_address.is_empty() {
                match self
                    .aml_service
                    .check_address(&from_address, &event.tx_network)
                    .await
                {
                    Ok(RiskResult::Blocked { reason }) => {
                        warn!(
                            event_id = %event.id,
                            session_id = %event.session_id,
                            from_address = %from_address,
                            reason = %reason,
                            "AML BLOCKED: Risky sender address detected"
                        );
                        self.alerting_service.send_alert(
                            "aml_payment_blocked",
                            AlertLevel::Info,
                            &format!(
                                "ℹ️ AML blocked payment: session={} from={} reason={}",
                                event.session_id, from_address, reason
                            ),
                        );

                        // Check if session is already blocked (avoid duplicate processing)
                        let session_status: Option<String> = txn
                            .query_one(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                r#"SELECT status FROM checkout_sessions WHERE id = $1"#,
                                [event.session_id.clone().into()],
                            ))
                            .await?
                            .and_then(|row| row.try_get("", "status").ok());

                        if session_status.as_deref() == Some("Blocked") {
                            // Session already blocked - just mark event as processed and skip
                            info!(
                                event_id = %event.id,
                                session_id = %event.session_id,
                                "Session already blocked, skipping duplicate AML block"
                            );
                            self.mark_event_processed_with_txn(&txn, &event.id).await?;
                            txn.commit().await?;
                            return Ok(None);
                        }

                        // 1. Update session to Blocked status (正确大小写)
                        txn.execute(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"UPDATE checkout_sessions SET status = 'Blocked', updated_at = NOW() WHERE id = $1"#,
                            [event.session_id.clone().into()],
                        ))
                        .await?;

                        // 2. Lock the payment address (正确大小写)
                        txn.execute(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"UPDATE addresses SET status = 'Locked', updated_at = NOW() WHERE address = (SELECT pay_address FROM checkout_sessions WHERE id = $1)"#,
                            [event.session_id.clone().into()],
                        ))
                        .await?;

                        // 3. Create RiskBlocked exception (包含所有必要字段, 从 transactions 表取 block_number/timestamp)
                        let exception_id = format!("pex_{}", uuid::Uuid::new_v4().simple());
                        txn.execute(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            INSERT INTO payment_exceptions
                            (id, network, tx_hash, log_index, exception_type, to_address, from_address,
                             amount, currency_symbol, merchant_id, session_id, block_number, block_timestamp,
                             status, notes, created_at, updated_at)
                            SELECT $1, cs.network, $2, $3, 'risk_blocked', cs.pay_address, $4,
                                   $5, cs.currency, cs.merchant_id, $6,
                                   COALESCE(tx.block_number, 0),
                                   COALESCE(tx.block_timestamp, NOW()),
                                   'Pending', $7, NOW(), NOW()
                            FROM checkout_sessions cs
                            LEFT JOIN transactions tx ON tx.network = $8 AND tx.tx_hash = $2 AND tx.log_index = $3
                            WHERE cs.id = $6
                            "#,
                            [
                                exception_id.clone().into(),
                                event.tx_hash.clone().into(),
                                event.tx_log_index.into(),
                                from_address.clone().into(),
                                event.amount.into(),
                                event.session_id.clone().into(),
                                format!("AML blocked: {}", reason).into(),
                                event.tx_network.clone().into(),
                            ],
                        ))
                        .await?;

                        // 4. Queue session.blocked webhook (atomic with above changes)
                        use crate::services::checkout::{SessionEventPayload, WebhookPricingInfo};

                        // Fetch session info for webhook payload
                        let blocked_session_row = txn
                            .query_one(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                r#"
                            SELECT merchant_id, network, amount_expected, amount_received, currency,
                                   currency_contract, pay_address, client_reference_id,
                                   pricing_currency, pricing_amount, exchange_rate,
                                   EXTRACT(EPOCH FROM created_at)::bigint as created_at_epoch
                            FROM checkout_sessions WHERE id = $1
                            "#,
                                [event.session_id.clone().into()],
                            ))
                            .await?
                            .ok_or_else(|| anyhow!("Session not found"))?;

                        let blocked_currency: String =
                            blocked_session_row.try_get("", "currency")?;
                        let blocked_amount_expected: i64 =
                            blocked_session_row.try_get("", "amount_expected")?;
                        let blocked_payload = SessionEventPayload {
                            object: "checkout_session",
                            id: event.session_id.clone(),
                            merchant_id: blocked_session_row.try_get("", "merchant_id")?,
                            amount: crate::api::dtos::checkout::from_micro(
                                blocked_amount_expected,
                                &blocked_currency,
                            ),
                            amount_received: crate::api::dtos::checkout::from_micro(
                                event.amount,
                                &blocked_currency,
                            ),
                            fee_amount: None,
                            net_amount: None,
                            currency: blocked_currency.clone(),
                            token_contract: blocked_session_row.try_get("", "currency_contract")?,
                            network: blocked_session_row.try_get("", "network")?,
                            livemode: Network::is_livemode_env(&self.environment),
                            status: "Blocked".to_string(),
                            pay_address: blocked_session_row.try_get("", "pay_address")?,
                            client_reference_id: blocked_session_row
                                .try_get("", "client_reference_id")
                                .ok(),
                            created_at: blocked_session_row.try_get("", "created_at_epoch")?,
                            paid_at: None, // Blocked sessions don't have a paid_at
                            tx_count: 1,
                            transactions: vec![], // AML-blocked tx not included for security
                            pricing: {
                                let pricing_cur: String =
                                    blocked_session_row.try_get("", "pricing_currency")?;
                                let pricing_amt: rust_decimal::Decimal =
                                    blocked_session_row.try_get("", "pricing_amount")?;
                                let ex_rate: rust_decimal::Decimal =
                                    blocked_session_row.try_get("", "exchange_rate")?;
                                WebhookPricingInfo {
                                    currency: pricing_cur,
                                    amount: pricing_amt.normalize().to_string(),
                                    exchange_rate: ex_rate.to_string(),
                                }
                            },
                        };

                        let network_str: String = blocked_session_row.try_get("", "network")?;
                        let merchant_id_str: String =
                            blocked_session_row.try_get("", "merchant_id")?;

                        let blocked_webhook_ids =
                            if let Some(network_enum) = Network::from_str_lenient(&network_str) {
                                let environment = self.environment.clone();
                                self.webhook_service
                                .queue_event_with_txn(
                                    &txn,
                                    &event.session_id,
                                    &merchant_id_str,
                                    network_enum,
                                    environment,
                                    webhook_events::EVENT_SESSION_BLOCKED,
                                    &blocked_payload,
                                )
                                .await
                                .unwrap_or_else(|e| {
                                    warn!(error = %e, "Failed to queue session.blocked webhook");
                                    vec![]
                                })
                            } else {
                                vec![]
                            };

                        // 5. Mark event as processed
                        self.mark_event_processed_with_txn(&txn, &event.id).await?;

                        // 6. Commit transaction (atomic)
                        txn.commit().await?;

                        // 7. POST-COMMIT: Trigger webhook delivery
                        if !blocked_webhook_ids.is_empty() {
                            self.webhook_service
                                .trigger_delivery(&blocked_webhook_ids)
                                .await;
                        }

                        crate::services::metrics::inc_session("blocked", &event.tx_network);
                        info!(
                            session_id = %event.session_id,
                            exception_id = %exception_id,
                            webhook_count = blocked_webhook_ids.len(),
                            "Session blocked due to AML risk, exception created, webhook triggered"
                        );

                        return Ok(None);
                    }
                    Ok(RiskResult::Safe) => {
                        debug!(from_address = %from_address, "AML check passed");
                    }
                    Err(e) => {
                        // Fail-open: Log error but proceed (L1 blacklist already checked)
                        warn!(from_address = %from_address, error = %e, "AML check error, proceeding (fail-open)");
                    }
                }
            }

            // Transaction exists but not yet credited - proceed to credit it
            debug!(
                event_id = %event.id,
                tx_hash = %event.tx_hash,
                "Transaction exists and AML passed, proceeding"
            );
        }
        // If transaction doesn't exist in DB yet, that's unexpected but we proceed anyway
        // (The indexer should have inserted it before emitting the payment_confirmed event)

        // ============================================================
        // ATOMIC: Apply payment + Mark transaction credited + Mark event processed
        // ============================================================

        // Capture pre-payment status to detect whether THIS payment caused the
        // terminal transition. We must guard against ALL terminal states, not just
        // Paid/Overpaid, because a race between expiry worker / AML block and the
        // payment processor can leave the session in Expired/Blocked by the time
        // the payment event is processed.
        let (pre_session_status, session_currency) = {
            use crate::entity::checkout_sessions;
            let session = checkout_sessions::Entity::find_by_id(&event.session_id)
                .one(&txn)
                .await
                .ok()
                .flatten();
            (
                session.as_ref().map(|s| s.status.clone()),
                session
                    .as_ref()
                    .map(|s| s.currency.clone())
                    .unwrap_or_else(|| "USDT".to_string()),
            )
        };

        let already_terminal = pre_session_status
            .as_ref()
            .map(|s| s.is_terminal())
            .unwrap_or(false);

        // ============================================================
        // TERMINAL GUARD: Session already in final state
        // If session is terminal (Paid/Overpaid/Expired/Blocked), this is a
        // late/extra payment. Create an exception for Resolution Center.
        // ============================================================
        if already_terminal {
            // Determine exception type based on why the session is terminal
            let (exception_type_str, note_prefix) = match &pre_session_status {
                Some(crate::entity::checkout_sessions::SessionStatus::Paid)
                | Some(crate::entity::checkout_sessions::SessionStatus::Overpaid) => (
                    "session_already_completed",
                    "Late payment after session completed",
                ),
                Some(crate::entity::checkout_sessions::SessionStatus::Expired) => (
                    "session_expired",
                    "Payment arrived after session expired (race)",
                ),
                Some(crate::entity::checkout_sessions::SessionStatus::Blocked) => (
                    "session_already_completed",
                    "Payment arrived after session blocked",
                ),
                _ => (
                    "session_already_completed",
                    "Payment arrived for terminal session",
                ),
            };

            let exception_id = format!("pex_{}", uuid::Uuid::new_v4().simple());
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO payment_exceptions
                (id, network, tx_hash, log_index, exception_type, to_address, from_address,
                 amount, currency_symbol, merchant_id, session_id, block_number, block_timestamp,
                 status, notes, created_at, updated_at)
                SELECT $1, cs.network, $2, $3, $8, cs.pay_address,
                       COALESCE(tx.from_address, ''),
                       $4, cs.currency, cs.merchant_id, $5,
                       COALESCE(tx.block_number, 0),
                       COALESCE(tx.block_timestamp, NOW()),
                       'Pending',
                       $6,
                       NOW(), NOW()
                FROM checkout_sessions cs
                LEFT JOIN transactions tx ON tx.network = $7 AND tx.tx_hash = $2 AND tx.log_index = $3
                WHERE cs.id = $5
                "#,
                [
                    exception_id.clone().into(),
                    event.tx_hash.clone().into(),
                    event.tx_log_index.into(),
                    event.amount.into(),
                    event.session_id.clone().into(),
                    format!(
                        "{}: {} {} (tx: {})",
                        note_prefix,
                        event.amount as f64 / 1_000_000.0,
                        session_currency,
                        event.tx_hash
                    )
                    .into(),
                    event.tx_network.clone().into(),
                    exception_type_str.into(),
                ],
            ))
            .await?;

            // Mark event as processed (so it doesn't retry)
            // NOTE: Do NOT mark tx is_credited=true — it hasn't been credited yet
            self.mark_event_processed_with_txn(&txn, &event.id).await?;

            txn.commit().await?;

            warn!(
                event_id = %event.id,
                session_id = %event.session_id,
                exception_id = %exception_id,
                exception_type = exception_type_str,
                amount = event.amount,
                tx_hash = %event.tx_hash,
                "TERMINAL GUARD: Session already terminal, exception created for Resolution Center"
            );

            return Ok(None);
        }

        let updated_session = self
            .checkout_service
            .apply_payment_with_txn(
                &txn,
                &event.session_id,
                event.amount,
                self.underpayment_threshold,
            )
            .await
            .map_err(|e| anyhow!("Failed to apply payment: {}", e))?;

        // Mark THIS transaction as credited (unique fingerprint guarantee)
        txn.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE transactions
            SET is_credited = true, updated_at = NOW()
            WHERE network = $1 AND tx_hash = $2 AND log_index = $3
            "#,
            [
                event.tx_network.clone().into(),
                event.tx_hash.clone().into(),
                event.tx_log_index.into(),
            ],
        ))
        .await?;

        // NOTE: Balance credit has been moved to the is_successful() block below.
        // This is the "Lazy Credit" model: merchant balance is only credited when
        // the session reaches a terminal success state (Paid/Overpaid), not per-payment.

        // Mark event as processed within the same transaction
        self.mark_event_processed_with_txn(&txn, &event.id).await?;

        // ============================================================
        // ATOMIC WEBHOOK QUEUEING + LAZY CREDIT
        // ============================================================
        let mut webhook_event_ids = Vec::new();
        if updated_session.status.is_successful() {
            // ============================================================
            // LAZY CREDIT: Credit merchant balance on terminal success
            // Fee is calculated ONCE on total amount_received (not per-payment).
            // This is atomic with webhook queueing — both succeed or both roll back.
            // ============================================================
            let total_received = updated_session.amount_received;

            // Look up merchant's custom fee percentage (if any)
            let merchant_custom_pct = {
                use crate::entity::merchants;
                merchants::Entity::find_by_id(&updated_session.merchant_id)
                    .one(&txn)
                    .await?
                    .and_then(|m| m.custom_fee_percentage)
            };

            let (total_fee, total_net) = {
                let network_enum = Network::from_str_lenient(&updated_session.network)
                    .ok_or_else(|| anyhow!("Invalid network '{}'", updated_session.network))?;
                let chain_floor = self.chain_deposit_floors.get(&network_enum).copied();
                self.fee_config.net_after_fee_for_chain(
                    total_received,
                    chain_floor,
                    merchant_custom_pct,
                )
            };

            let network_enum = Network::from_str_lenient(&updated_session.network)
                .ok_or_else(|| anyhow!("Invalid network '{}'", updated_session.network))?;
            let environment = self.environment.clone();

            // Always record billing log (even for net=0 dust payments — audit trail)
            self.billing_service
                .process_deposit(
                    &txn,
                    &updated_session.merchant_id,
                    total_net,
                    Some(format!("session_{}", updated_session.id)),
                    Some(format!(
                        "Session completed: {} {} received, {} {} fee, {} {} net",
                        total_received as f64 / 1_000_000.0,
                        updated_session.currency,
                        total_fee as f64 / 1_000_000.0,
                        updated_session.currency,
                        total_net as f64 / 1_000_000.0,
                        updated_session.currency,
                    )),
                    network_enum,
                    environment,
                    &updated_session.currency,
                    Some(total_received),
                    Some(total_fee),
                )
                .await
                .map_err(|e| anyhow!("Failed to credit merchant balance: {}", e))?;

            // Always set fee/net on session (not accumulate — this is the final value)
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE checkout_sessions
                SET fee_amount = $1,
                    net_amount = $2,
                    updated_at = NOW()
                WHERE id = $3
                "#,
                [
                    total_fee.into(),
                    total_net.into(),
                    updated_session.id.clone().into(),
                ],
            ))
            .await?;

            crate::services::metrics::inc_session("completed", &updated_session.network);
            info!(
                session_id = %updated_session.id,
                merchant_id = %updated_session.merchant_id,
                total_received = total_received,
                fee = total_fee,
                net = total_net,
                session_status = ?updated_session.status,
                "LAZY CREDIT: Session completed, merchant balance credited"
            );

            use crate::services::checkout::{SessionEventPayload, WebhookPricingInfo};

            // Fetch all credited transactions for the session (ordered by block_timestamp ASC)
            // IMPORTANT: Use txn to see uncommitted is_credited changes from within this transaction
            let transactions = self
                .checkout_service
                .get_session_transactions_with_conn(
                    &txn,
                    &updated_session.id,
                    &updated_session.currency,
                )
                .await
                .unwrap_or_default();

            let payload = SessionEventPayload {
                object: "checkout_session",
                id: updated_session.id.clone(),
                merchant_id: updated_session.merchant_id.clone(),
                amount: crate::api::dtos::checkout::from_micro(
                    updated_session.amount_expected,
                    &updated_session.currency,
                ),
                amount_received: crate::api::dtos::checkout::from_micro(
                    updated_session.amount_received,
                    &updated_session.currency,
                ),
                fee_amount: Some(crate::api::dtos::checkout::from_micro(
                    total_fee,
                    &updated_session.currency,
                )),
                net_amount: Some(crate::api::dtos::checkout::from_micro(
                    total_net,
                    &updated_session.currency,
                )),
                currency: updated_session.currency.clone(),
                token_contract: updated_session.currency_contract.clone(),
                network: updated_session.network.clone(),
                livemode: Network::is_livemode_env(&self.environment),
                status: format!("{:?}", updated_session.status),
                pay_address: updated_session.pay_address.clone(),
                client_reference_id: updated_session.client_reference_id.clone(),
                created_at: updated_session.created_at.timestamp(),
                paid_at: Some(Utc::now().timestamp()),
                tx_count: transactions.len() as i32,
                transactions,
                pricing: WebhookPricingInfo {
                    currency: updated_session.pricing_currency.clone(),
                    amount: updated_session.pricing_amount.normalize().to_string(),
                    exchange_rate: updated_session.exchange_rate.to_string(),
                },
            };

            // Queue events within the SAME transaction
            // If the transaction fails/rolls back, these events are never created.
            // If it commits, they are guaranteed to exist.
            // Perform Network Resolution
            let network_enum = Network::from_str_lenient(&updated_session.network)
                .ok_or_else(|| anyhow!("Invalid network '{}'", updated_session.network))?;
            let environment = self.environment.clone();

            match self
                .webhook_service
                .queue_event_with_txn(
                    &txn,
                    &updated_session.id,
                    &updated_session.merchant_id,
                    network_enum,
                    environment,
                    webhook_events::EVENT_SESSION_COMPLETED,
                    &payload,
                )
                .await
            {
                Ok(ids) => webhook_event_ids = ids,
                Err(e) => {
                    // Log error but don't fail the payment transaction for a webhook queueing error?
                    // actually, if we want strict consistency, we SHOULD fail.
                    // But practically, payment success is more important than webhook success?
                    // No, "Atomic" means all or nothing. If webhook queue fails (DB error), we should probably fail/retry the whole thing.
                    // Because otherwise we lose the notification permanently (Double-Write risk).
                    return Err(anyhow!("Failed to atomically queue webhook: {}", e));
                }
            }
        }

        // Commit the atomic transaction
        txn.commit().await?;

        // ============================================================
        // POST-COMMIT SIDE EFFECTS
        // ============================================================

        // 1. Trigger Webhook Delivery (Fire-and-forget, data is safe in DB)
        if !webhook_event_ids.is_empty() {
            info!(
                session_id = %updated_session.id,
                event_count = webhook_event_ids.len(),
                "Triggering webhook delivery (atomic)"
            );
            self.webhook_service
                .trigger_delivery(&webhook_event_ids)
                .await;
        }

        // 2. SSE Broadcast for real-time checkout updates
        if let Some(ref broadcaster) = self.sse_broadcaster {
            use crate::services::sse::SseEvent;
            broadcaster.broadcast(
                &updated_session.id,
                SseEvent::SessionUpdated {
                    status: format!("{:?}", updated_session.status),
                    amount_received: crate::api::dtos::checkout::from_micro(
                        updated_session.amount_received,
                        &updated_session.currency,
                    ),
                    expires_at: Some(updated_session.expires_at.to_rfc3339()),
                },
            );
            debug!(
                session_id = %updated_session.id,
                status = ?updated_session.status,
                "SSE broadcast sent"
            );
        }

        // 3. Trigger Sweep (Data safety: enqueue_sweep_task only logs, sweeper polls DB)
        if updated_session.status.is_successful() {
            self.enqueue_sweep_task(&updated_session.network, &updated_session.pay_address)
                .await;
        }

        // 4. Enqueue Xero sync (fire-and-forget, just inserts pending row if enabled)
        if updated_session.status.is_successful() {
            if let Some(ref xero_svc) = self.xero_service {
                if let Err(e) = xero_svc
                    .enqueue_sync_if_enabled(
                        &updated_session.merchant_id,
                        self.environment.clone(),
                        &updated_session.id,
                    )
                    .await
                {
                    warn!(
                        session_id = %updated_session.id,
                        error = %e,
                        "Failed to enqueue Xero sync (non-critical)"
                    );
                }
            }
        }

        info!(
            event_id = %event.id,
            session_id = %event.session_id,
            new_status = ?updated_session.status,
            "Payment event processed atomically"
        );

        // Return session if successful (for sweep triggering)
        if updated_session.status.is_successful() {
            Ok(Some(updated_session))
        } else {
            Ok(None)
        }
    }

    /// Mark event as processed within an existing transaction
    async fn mark_event_processed_with_txn<C>(&self, txn: &C, event_id: &str) -> Result<()>
    where
        C: ConnectionTrait,
    {
        txn.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE payment_events
            SET status = 'processed',
                processed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            "#,
            [event_id.into()],
        ))
        .await?;

        debug!(event_id = %event_id, "Event marked as processed (in transaction)");
        Ok(())
    }

    /// Mark event as successfully processed (standalone, for non-transactional use)
    async fn mark_event_processed(&self, event_id: &str) -> Result<()> {
        self.db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE payment_events
                SET status = 'processed',
                    processed_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                "#,
                [event_id.into()],
            ))
            .await?;

        debug!(event_id = %event_id, "Event marked as processed");
        Ok(())
    }

    /// Handle event processing failure with exponential backoff
    async fn handle_event_failure(&self, event: &payment_events::Model, error: &str) -> Result<()> {
        let new_attempt_count = event.attempt_count + 1;

        if new_attempt_count > MAX_RETRY_ATTEMPTS {
            // Move to dead letter queue
            self.mark_event_failed(&event.id, error).await?;
            error!(
                event_id = %event.id,
                session_id = %event.session_id,
                attempts = new_attempt_count,
                last_error = %error,
                "CRITICAL: Payment event moved to dead letter queue after max retries"
            );
            self.alerting_service.send_alert(
                "payment_event_dead_letter",
                AlertLevel::Critical,
                &format!(
                    "🚨 Payment event {} (session={}) moved to dead letter after {} retries: {}",
                    event.id, event.session_id, new_attempt_count, error
                ),
            );
        } else {
            // Schedule retry with exponential backoff + jitter
            let next_retry = Self::calculate_next_retry(new_attempt_count);
            self.schedule_retry(&event.id, new_attempt_count, next_retry, error)
                .await?;
            info!(
                event_id = %event.id,
                attempt = new_attempt_count,
                next_retry = %next_retry,
                "Event scheduled for retry"
            );
        }

        Ok(())
    }

    /// Mark event as failed (dead letter)
    async fn mark_event_failed(&self, event_id: &str, error: &str) -> Result<()> {
        let event = PaymentEvents::find_by_id(event_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Event not found: {}", event_id))?;

        let mut active: payment_events::ActiveModel = event.into();
        active.status = Set(payment_events::PaymentEventStatus::Failed);
        active.error_message = Set(Some(error.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(())
    }

    /// Schedule retry with new attempt count and next retry time
    async fn schedule_retry(
        &self,
        event_id: &str,
        attempt_count: i32,
        next_retry: DateTime<Utc>,
        error: &str,
    ) -> Result<()> {
        let event = PaymentEvents::find_by_id(event_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Event not found: {}", event_id))?;

        let mut active: payment_events::ActiveModel = event.into();
        active.status = Set(payment_events::PaymentEventStatus::Pending);
        active.attempt_count = Set(attempt_count);
        active.next_retry_at = Set(next_retry.into());
        active.error_message = Set(Some(error.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(())
    }

    /// Calculate next retry time with exponential backoff and jitter
    fn calculate_next_retry(attempt_count: i32) -> DateTime<Utc> {
        // Ensure minimum of 1 for first retry: delays are 2, 4, 8, 16, 32, 64, 128 seconds
        let clamped_attempt = attempt_count.max(1).min(7) as u32;
        let base_delay_secs = 2u64.pow(clamped_attempt);

        // Cap at 1 hour
        let delay_secs = base_delay_secs.min(3600);

        // Add ±20% jitter to avoid thundering herd
        let jitter = rand::thread_rng().gen_range(0.8..1.2);
        let final_delay = Duration::from_secs_f64(delay_secs as f64 * jitter);

        Utc::now()
            + chrono::Duration::from_std(final_delay).unwrap_or_else(|e| {
                warn!(error = %e, "Duration conversion failed, using 2s fallback");
                chrono::Duration::seconds(2)
            })
    }

    /// Reclaim events stuck in processing state (zombie cleanup)
    ///
    /// Uses randomized jitter (0-60 seconds) to prevent thundering herd when
    /// multiple stale events are reclaimed simultaneously.
    async fn reclaim_stale_events(&self) -> Result<u64> {
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE payment_events
                SET status = 'pending',
                    attempt_count = attempt_count + 1,
                    next_retry_at = NOW() + (random() * INTERVAL '60 seconds'),
                    updated_at = NOW()
                WHERE status = 'processing'
                  AND updated_at < NOW() - INTERVAL '1 second' * $1
                "#,
                [STALE_TIMEOUT_SECS.into()],
            ))
            .await?;

        let affected = result.rows_affected();
        if affected > 0 {
            warn!(
                count = affected,
                "Reclaimed stale payment events (with jitter)"
            );
        }

        Ok(affected)
    }

    /// Get processor statistics
    pub async fn get_stats(&self) -> Result<ProcessorStats> {
        // Single query with GROUP BY for efficiency
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT status, COUNT(*) as count
                FROM payment_events
                WHERE status IN ('pending', 'processing', 'failed')
                GROUP BY status
                "#,
                [],
            ))
            .await?;

        let mut counts: HashMap<String, i64> = HashMap::new();
        for row in rows {
            let status: String = row.try_get("", "status").unwrap_or_default();
            let count: i64 = row.try_get("", "count").unwrap_or_else(|e| {
                warn!(error = %e, status = %status, "Failed to parse count");
                0
            });
            counts.insert(status, count);
        }

        Ok(ProcessorStats {
            pending_count: *counts.get("pending").unwrap_or(&0) as usize,
            processing_count: *counts.get("processing").unwrap_or(&0) as usize,
            failed_count: *counts.get("failed").unwrap_or(&0) as usize,
        })
    }
}

#[derive(Debug)]
pub struct ProcessorStats {
    pub pending_count: usize,
    pub processing_count: usize,
    pub failed_count: usize,
}
