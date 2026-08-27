//! Supervisor Loop for Background Task Fault Isolation
//!
//! Generic wrapper that restarts any async task on failure with exponential backoff.
//! Chain-specific tasks report health to ChainHealthRegistry; non-chain tasks
//! (webhook recovery, payout worker, etc.) just restart + alert.
//!
//! Exit conditions:
//! - CancellationToken fired (graceful shutdown)
//! - Task returns Ok(()) (graceful exit, e.g. cancelled internally)
//!
//! Errors are NEVER propagated — the supervisor loops forever until cancelled.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::entity::Network;
use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::chain_health::ChainHealthRegistry;

/// Maximum backoff duration cap (60 seconds)
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Initial backoff duration after first failure (5 seconds)
const INITIAL_BACKOFF: Duration = Duration::from_secs(5);

/// Restarts a background task on failure with exponential backoff.
///
/// # Arguments
/// - `task_name`: Human-readable name for logging/alerting (e.g. "TRON Indexer")
/// - `health`: If `Some`, updates ChainHealthRegistry lifecycle (Starting/Unhealthy).
///   The task itself is responsible for calling `mark_healthy()`.
///   Pass `None` for non-chain tasks like Webhook Recovery.
/// - `cancel_token`: Shared cancellation signal for graceful shutdown
/// - `alerting`: Alert service for critical crash notifications
/// - `task_factory`: Closure that creates a new Future for each restart attempt.
///   Must be `Fn` (not `FnOnce`) because it's called on every restart.
///
/// # Ownership Pattern
/// The `task_factory` closure typically captures `Arc<Service>` and `CancellationToken`,
/// cloning them for each invocation:
/// ```ignore
/// move || {
///     let service = service_arc.clone();
///     let token = token.clone();
///     async move { service.start(token).await }
/// }
/// ```
pub async fn supervisor_loop<F, Fut>(
    task_name: &str,
    health: Option<(ChainHealthRegistry, Network)>,
    cancel_token: CancellationToken,
    alerting: Arc<AlertingService>,
    task_factory: F,
) -> Result<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let mut restart_count = 0u32;
    let mut backoff = INITIAL_BACKOFF;
    let alert_key = format!("supervisor_{}", task_name.to_lowercase().replace(' ', "_"));

    loop {
        // Mark Starting (if chain-specific)
        if let Some((ref registry, ref net)) = health {
            registry.mark_starting(net);
        }

        if restart_count > 0 {
            info!(
                task = task_name,
                restart = restart_count,
                "Supervisor restarting task"
            );
        } else {
            info!(task = task_name, "Supervisor starting task");
        }

        match task_factory().await {
            Ok(()) => {
                // Graceful exit — task was cancelled internally or completed
                info!(task = task_name, "Task exited gracefully");
                break;
            }
            Err(e) => {
                restart_count += 1;
                error!(
                    task = task_name,
                    error = %e,
                    restart = restart_count,
                    "Task crashed, will restart after backoff"
                );

                // Mark Unhealthy (if chain-specific)
                if let Some((ref registry, ref net)) = health {
                    registry.mark_unhealthy(net, e.to_string());
                }

                // Fire critical alert (with dedup via alert_key)
                alerting.send_alert(
                    &alert_key,
                    AlertLevel::Critical,
                    &format!("{} crashed (restart #{}): {}", task_name, restart_count, e),
                );

                // Exponential backoff with cancellation support
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!(task = task_name, "Supervisor cancelled during backoff");
                        break;
                    }
                    _ = tokio::time::sleep(backoff) => {
                        // Double backoff, cap at MAX_BACKOFF
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                    }
                }
            }
        }
    }

    Ok(())
}
