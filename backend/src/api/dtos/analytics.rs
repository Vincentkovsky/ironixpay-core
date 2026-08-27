//! Analytics API DTOs
//!
//! Request/response types for the merchant analytics dashboard.
//! All currency values are i64 microunits (1 USDT = 1_000_000).
//! Frontend must use `from_micro()` for display.

use chrono::{DateTime, Utc};
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};

/// GET /api/internal/analytics query parameters
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Start date (inclusive), ISO 8601 UTC. e.g. "2026-01-01T00:00:00Z"
    pub start_date: Option<DateTime<Utc>>,
    /// End date (inclusive), ISO 8601 UTC. e.g. "2026-03-31T23:59:59Z"
    pub end_date: Option<DateTime<Utc>>,
    /// Optional currency filter: "USDT" or "USDC". If omitted, returns both.
    pub currency: Option<String>,
}

/// Full analytics response
#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    /// KPI summaries, one entry per currency (USDT, USDC)
    pub kpis: Vec<KpiEntry>,
    /// Daily volume time series (successful payments only)
    pub time_series: Vec<TimeSeriesPoint>,
    /// Payment distribution by network (successful payments only)
    pub network_distribution: Vec<DistributionEntry>,
    /// Session count by terminal status
    pub status_breakdown: Vec<DistributionEntry>,
    /// Overall conversion rate: (paid + overpaid) / all terminal sessions
    /// Expressed as a float between 0.0 and 1.0
    pub conversion_rate: f64,
}

/// Per-currency KPI summary
#[derive(Debug, Serialize)]
pub struct KpiEntry {
    /// "USDT" or "USDC"
    pub currency: String,
    /// Total gross volume (amount_received) in microunits
    pub gross_volume: i64,
    /// Total net revenue (net_amount) in microunits
    pub net_revenue: i64,
    /// Total fees collected in microunits
    pub fee_total: i64,
    /// Number of successful transactions
    pub tx_count: i64,
}

/// Daily time series data point
#[derive(Debug, Serialize)]
pub struct TimeSeriesPoint {
    /// Date string, e.g. "2026-03-25"
    pub date: String,
    /// Currency: "USDT" or "USDC"
    pub currency: String,
    /// Daily gross volume in microunits
    pub volume: i64,
    /// Daily transaction count
    pub count: i64,
}

/// Generic distribution entry (for network/status breakdowns)
#[derive(Debug, Serialize)]
pub struct DistributionEntry {
    /// Label: network name (e.g. "TRON", "BSC") or status (e.g. "Paid", "Expired")
    pub label: String,
    /// Count or volume value
    pub value: i64,
}

// ── SQL result structs for FromQueryResult ──

#[derive(Debug, FromQueryResult)]
pub struct KpiRow {
    pub currency: String,
    pub gross_volume: Option<i64>,
    pub net_revenue: Option<i64>,
    pub fee_total: Option<i64>,
    pub tx_count: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
pub struct TimeSeriesRow {
    pub date: Option<String>,
    pub currency: String,
    pub volume: Option<i64>,
    pub count: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
pub struct DistributionRow {
    pub label: String,
    pub value: Option<i64>,
}
