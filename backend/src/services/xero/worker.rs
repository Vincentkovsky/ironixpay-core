//! Xero Sync Worker
//!
//! Background task that polls for pending sync logs and processes them.
//! Implements per-tenant throttling and rate limit handling.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::client::XeroError;
use super::XeroService;
use anyhow::Result;
use tokio::time::{sleep, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Minimum interval between Xero API calls for the same tenant (rate limit: 60/min).
const MIN_TENANT_INTERVAL: Duration = Duration::from_millis(1100);
/// Poll interval when no pending logs found.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Batch size per poll cycle.
const BATCH_SIZE: u64 = 20;

pub struct XeroSyncWorker {
    xero_service: Arc<XeroService>,
    cancel_token: CancellationToken,
}

impl XeroSyncWorker {
    pub fn new(xero_service: Arc<XeroService>, cancel_token: CancellationToken) -> Self {
        Self {
            xero_service,
            cancel_token,
        }
    }

    /// Main loop: poll for pending sync logs and process them.
    pub async fn run(&self) -> Result<()> {
        info!("Xero sync worker started");

        // Track last API call time per tenant for throttling
        let mut tenant_last_call: HashMap<String, Instant> = HashMap::new();
        // Exponential backoff for DB query failures
        let mut error_backoff = Duration::from_secs(1);
        const MAX_BACKOFF: Duration = Duration::from_secs(300);

        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    info!("Xero sync worker shutting down");
                    return Ok(());
                }
                _ = sleep(IDLE_POLL_INTERVAL) => {}
            }

            let logs = match self.xero_service.get_pending_sync_logs(BATCH_SIZE).await {
                Ok(logs) => {
                    error_backoff = Duration::from_secs(1);
                    logs
                }
                Err(e) => {
                    error!(error = %e, backoff_secs = error_backoff.as_secs(), "Failed to fetch pending Xero sync logs");
                    sleep(error_backoff).await;
                    error_backoff = (error_backoff * 2).min(MAX_BACKOFF);
                    continue;
                }
            };

            if logs.is_empty() {
                continue;
            }

            debug!(count = logs.len(), "Processing Xero sync batch");

            for log in &logs {
                // Check cancellation between items
                if self.cancel_token.is_cancelled() {
                    return Ok(());
                }

                // Load connection
                let connection = match self
                    .xero_service
                    .get_connection_by_id(log.connection_id)
                    .await
                {
                    Ok(Some(conn)) => conn,
                    Ok(None) => {
                        warn!(sync_id = %log.id, "Connection not found, marking skipped");
                        let _ = self
                            .xero_service
                            .mark_sync_skipped(log, "Connection not found")
                            .await;
                        continue;
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to load Xero connection");
                        continue;
                    }
                };

                // Defensive guard: query already filters active+auto_sync, but re-check in case
                // connection changed after logs were selected.
                if connection.status
                    != crate::entity::xero_connections::XeroConnectionStatus::Active
                    || !connection.auto_sync_enabled
                {
                    debug!(
                        sync_id = %log.id,
                        status = %connection.status,
                        auto_sync_enabled = connection.auto_sync_enabled,
                        "Skipping sync because connection is not processable"
                    );
                    continue;
                }

                // Per-tenant throttling
                let tenant_id = &connection.xero_tenant_id;
                if let Some(last) = tenant_last_call.get(tenant_id) {
                    let elapsed = last.elapsed();
                    if elapsed < MIN_TENANT_INTERVAL {
                        sleep(MIN_TENANT_INTERVAL - elapsed).await;
                    }
                }

                // Execute sync
                match self.xero_service.sync_session(log, &connection).await {
                    Ok(()) => {
                        tenant_last_call.insert(tenant_id.clone(), Instant::now());
                        debug!(session_id = %log.session_id, "Xero sync succeeded");
                    }
                    Err(e) => {
                        tenant_last_call.insert(tenant_id.clone(), Instant::now());

                        // Handle rate limiting (429)
                        if let Some(rate_err) = e.downcast_ref::<XeroError>() {
                            match rate_err {
                                XeroError::RateLimited { retry_after } => {
                                    warn!(
                                        tenant_id = tenant_id,
                                        retry_after = retry_after,
                                        "Rate limited by Xero, postponing all syncs for tenant"
                                    );

                                    let _ = self
                                        .xero_service
                                        .postpone_connection_syncs(
                                            connection.id,
                                            *retry_after as i64,
                                        )
                                        .await;

                                    // Skip rest of batch for this tenant
                                    continue;
                                }
                            }
                        }

                        // Normal failure → schedule retry
                        let err_str = e.to_string();
                        error!(
                            session_id = %log.session_id,
                            error = %err_str,
                            "Xero sync failed"
                        );
                        let _ = self.xero_service.mark_sync_failed(log, &err_str).await;
                    }
                }
            }
        }
    }
}
