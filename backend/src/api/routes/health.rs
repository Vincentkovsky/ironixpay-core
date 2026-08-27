//! Health check endpoints for monitoring.
//!
//! - `/health` (Liveness): Is the process alive?
//! - `/ready` (Readiness): Can the service handle requests?
//!   Checks: DB, chain indexer health (via ChainHealthRegistry), disk, pool.
//!   Strict semantics: ALL enabled chains must be Healthy for 200.

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use sea_orm::ConnectionTrait;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::warn;

use crate::AppState;

/// Timeout for dependency health checks (DB)
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Disk usage threshold (percentage). Above this → degraded.
const DISK_USAGE_THRESHOLD: u8 = 90;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<HealthDetails>,
}

#[derive(Serialize)]
struct HealthDetails {
    database: bool,
    /// Per-chain indexer health from ChainHealthRegistry.
    /// e.g. {"TRON": "healthy", "BSC": "starting"}
    chains: HashMap<String, String>,
    disk_ok: bool,
    disk_usage_percent: u8,
    db_pool_ok: bool,
    /// Active connections used by this application's database
    db_pool_active: u32,
    /// Configured max connections for the pool
    db_pool_max: u32,
    /// Background service heartbeat status (informational, does NOT affect HTTP status).
    /// e.g. {"tron_sweeper": "healthy", "payment_processor": "starting"}
    services: HashMap<String, String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(liveness))
        .route("/ready", get(readiness))
}

/// Liveness probe: Returns 200 if the process is running.
/// Used by orchestrators to know if the process needs to be restarted.
async fn liveness() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        details: None,
    })
}

/// Readiness probe: Returns 200 if all dependencies are healthy.
///
/// Checks: Database connectivity, chain indexer health, disk usage, DB pool.
/// Strict semantics: ALL enabled chains must be `Healthy` (not `Starting` or
/// `Unhealthy`) for the endpoint to return 200. This ensures the HTTP layer
/// and the checkout circuit breaker have consistent behavior.
async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    // Run DB and infra checks concurrently
    let (db_ok, (disk_ok, disk_pct), (pool_ok, pool_active, pool_max)) = tokio::join!(
        async {
            timeout(HEALTH_CHECK_TIMEOUT, check_database(&state))
                .await
                .unwrap_or(false)
        },
        async { check_disk() },
        async {
            timeout(HEALTH_CHECK_TIMEOUT, check_db_pool(&state))
                .await
                .unwrap_or((false, 0, 0))
        },
    );

    // Chain health from registry (no RPC calls — pure in-memory check)
    // Use is_healthy() to reflect staleness in the display, so /ready output
    // is consistent with the checkout circuit breaker behavior.
    let chain_statuses = state.chain_health.all_statuses();
    let chains: HashMap<String, String> = chain_statuses
        .iter()
        .map(|(net, s)| {
            let display = if state.chain_health.is_healthy(net) {
                "healthy".to_string()
            } else if matches!(
                s.status,
                crate::services::chain_health::HealthStatus::Healthy
            ) {
                "healthy (stale)".to_string()
            } else {
                s.status.to_string()
            };
            (net.as_str().to_string(), display)
        })
        .collect();
    let chains_ok = state.chain_health.all_healthy();

    let all_ok = db_ok && chains_ok && disk_ok && pool_ok;
    let status = if all_ok { "ok" } else { "degraded" };

    // Log degraded state for Sentry/tracing visibility
    if !all_ok {
        warn!(
            db_ok,
            chains_ok,
            disk_ok,
            disk_pct,
            pool_ok,
            pool_active,
            pool_max,
            "Readiness check DEGRADED"
        );
    }

    // Service heartbeats (informational, fail-open)
    let service_statuses: HashMap<String, String> = state
        .service_health
        .all_statuses()
        .into_iter()
        .map(|(name, status)| (name, status.to_string()))
        .collect();

    let details = Some(HealthDetails {
        database: db_ok,
        chains,
        disk_ok,
        disk_usage_percent: disk_pct,
        db_pool_ok: pool_ok,
        db_pool_active: pool_active,
        db_pool_max: pool_max,
        services: service_statuses,
    });

    let response = HealthResponse { status, details };

    if all_ok {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}

/// Check database connectivity by executing a simple query.
async fn check_database(state: &AppState) -> bool {
    state.db.execute_unprepared("SELECT 1").await.is_ok()
}

/// Check disk usage on the root partition.
///
/// Uses `nix::sys::statvfs` which works on both Linux and macOS.
/// Returns (is_ok, usage_percent). Fail-open: if we can't read → assume ok.
fn check_disk() -> (bool, u8) {
    match nix::sys::statvfs::statvfs("/") {
        Ok(stat) => {
            let frag = stat.fragment_size() as u64;
            let total = (stat.blocks() as u64) * frag;
            let avail = (stat.blocks_available() as u64) * frag;
            if total == 0 {
                return (true, 0);
            }
            let used_pct = (((total - avail) * 100) / total) as u8;
            (used_pct < DISK_USAGE_THRESHOLD, used_pct)
        }
        Err(_) => (true, 0), // Fail-open: can't read disk → assume ok
    }
}

/// Check database connection pool utilization via `pg_stat_activity`.
///
/// Returns (is_ok, active_connections, max_connections).
/// Pool is considered unhealthy when active connections reach max (no headroom).
async fn check_db_pool(state: &AppState) -> (bool, u32, u32) {
    let max_conn = state.config.database_max_connections;

    let sql = "SELECT count(*)::int4 FROM pg_stat_activity WHERE datname = current_database()";
    let stmt = sea_orm::Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

    match state.db.query_one(stmt).await {
        Ok(Some(row)) => {
            let active: i32 = row.try_get_by_index::<i32>(0).unwrap_or(0);
            let active = active.max(0) as u32;
            // Healthy if at least 1 connection of headroom
            let is_ok = active < max_conn;
            (is_ok, active, max_conn)
        }
        _ => (false, 0, max_conn),
    }
}
