//! Business metrics for Prometheus instrumentation.
//!
//! Naming convention: ironixpay_{domain}_{metric_name}_{unit}
//! Labels: network, environment, status
//!
//! Usage: `use crate::services::metrics;` then call helper functions.
//! The global recorder is installed by `axum_prometheus::PrometheusMetricLayerBuilder`
//! in main.rs — these functions rely on that being set up first.

use axum_prometheus::metrics::{counter, gauge, histogram};

// ── Indexer Metrics ──────────────────────────────────────────────────────────

/// Record the block lag (chain_head - last_processed_block) for a given network.
pub fn record_block_lag(network: &str, lag: i64) {
    gauge!("ironixpay_indexer_block_lag", "network" => network.to_string()).set(lag as f64);
}

/// Increment the total number of blocks scanned by the indexer.
pub fn inc_blocks_scanned(network: &str) {
    counter!("ironixpay_indexer_blocks_scanned_total", "network" => network.to_string())
        .increment(1);
}

/// Increment blocks scanned by N (batch).
pub fn inc_blocks_scanned_by(network: &str, n: u64) {
    counter!("ironixpay_indexer_blocks_scanned_total", "network" => network.to_string())
        .increment(n);
}

/// Increment the total number of payments detected.
/// `kind`: "normal" (active session) or "exception" (no active session / expired / dust).
pub fn inc_payment_detected(network: &str, kind: &str) {
    counter!(
        "ironixpay_indexer_payments_detected_total",
        "network" => network.to_string(),
        "kind" => kind.to_string()
    )
    .increment(1);
}

// ── Sweeper Metrics ──────────────────────────────────────────────────────────

/// Increment sweep attempts counter.
/// `status`: "success" | "failed" | "permanent_failure"
/// `token`: "USDT" | "USDC"
pub fn inc_sweep(network: &str, status: &str, token: &str) {
    counter!(
        "ironixpay_sweeper_total",
        "network" => network.to_string(),
        "status" => status.to_string(),
        "token" => token.to_string()
    )
    .increment(1);
}

/// Record sweep execution duration in seconds.
pub fn record_sweep_duration(network: &str, seconds: f64) {
    histogram!("ironixpay_sweeper_duration_seconds", "network" => network.to_string())
        .record(seconds);
}

// ── Payment Processor Metrics ────────────────────────────────────────────────

/// Increment the total number of payment events processed.
/// `status`: "success" | "failed"
pub fn inc_events_processed(status: &str) {
    counter!("ironixpay_payment_events_processed_total", "status" => status.to_string())
        .increment(1);
}

// ── Checkout Metrics ─────────────────────────────────────────────────────────

/// Increment checkout session lifecycle counter.
/// `status`: "created" | "completed" | "expired" | "blocked"
pub fn inc_session(status: &str, network: &str) {
    counter!(
        "ironixpay_checkout_sessions_total",
        "status" => status.to_string(),
        "network" => network.to_string()
    )
    .increment(1);
}

// ── Webhook Metrics ──────────────────────────────────────────────────────────

/// Increment webhook delivery attempt counter.
/// `status`: "success" | "failed" | "retry" | "giving_up"
pub fn inc_webhook_delivery(status: &str) {
    counter!("ironixpay_webhook_deliveries_total", "status" => status.to_string()).increment(1);
}

/// Record webhook delivery HTTP round-trip duration in seconds.
pub fn record_webhook_duration(seconds: f64) {
    histogram!("ironixpay_webhook_delivery_duration_seconds").record(seconds);
}

// ── Address Pool Metrics ─────────────────────────────────────────────────────

/// Set the current count of idle (available) addresses per network.
pub fn set_idle_addresses(network: &str, count: i64) {
    gauge!("ironixpay_address_pool_idle", "network" => network.to_string()).set(count as f64);
}

// ── Payout Metrics ──────────────────────────────────────────────────────────

/// Increment payout/withdrawal broadcast counter.
/// `status`: "success" | "failed"
/// `kind`: "payout" | "withdrawal"
pub fn inc_payout_broadcast(network: &str, status: &str, kind: &str) {
    counter!(
        "ironixpay_payout_broadcast_total",
        "network" => network.to_string(),
        "status" => status.to_string(),
        "kind" => kind.to_string()
    )
    .increment(1);
}

/// Increment payout/withdrawal confirmation counter.
/// `status`: "confirmed" | "failed" | "stale"
/// `kind`: "payout" | "withdrawal"
pub fn inc_payout_confirmed(network: &str, status: &str, kind: &str) {
    counter!(
        "ironixpay_payout_confirmed_total",
        "network" => network.to_string(),
        "status" => status.to_string(),
        "kind" => kind.to_string()
    )
    .increment(1);
}

// ── Exception Metrics ───────────────────────────────────────────────────────

/// Increment exception counter by type.
/// `exception_type`: "WrongToken" | "SessionExpired" | "NoActiveSession" | etc.
pub fn inc_exception(network: &str, exception_type: &str) {
    counter!(
        "ironixpay_exceptions_total",
        "network" => network.to_string(),
        "exception_type" => exception_type.to_string()
    )
    .increment(1);
}

// ── Resolution Metrics ──────────────────────────────────────────────────────

/// Increment resolution action counter.
/// `action`: "accept_expired" | "attach_session" | "manual_transfer"
/// `status`: "success" | "failed"
pub fn inc_resolution(action: &str, status: &str) {
    counter!(
        "ironixpay_resolution_actions_total",
        "action" => action.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

// ── AML Metrics ─────────────────────────────────────────────────────────────

/// Increment AML check counter.
/// `result`: "safe" | "blocked" | "error"
pub fn inc_aml_check(network: &str, result: &str) {
    counter!(
        "ironixpay_aml_checks_total",
        "network" => network.to_string(),
        "result" => result.to_string()
    )
    .increment(1);
}
