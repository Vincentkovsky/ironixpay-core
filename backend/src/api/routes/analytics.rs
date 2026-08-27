//! Analytics API Routes
//!
//! Provides aggregated analytics data for the merchant dashboard.
//! JWT auth is applied by the parent router (`/api/internal`).

use axum::{extract::Query, extract::State, routing::get, Json, Router};
use sea_orm::{DatabaseBackend, FromQueryResult, Statement};

use crate::api::dtos::analytics::{
    AnalyticsQuery, AnalyticsResponse, DistributionEntry, DistributionRow, KpiEntry, KpiRow,
    TimeSeriesPoint, TimeSeriesRow,
};
use crate::api::error::AppError;
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::api::routes::resolution::SubMerchantFilter;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_analytics))
}

/// GET /api/internal/analytics
///
/// Returns merchant analytics: KPIs, time series, network distribution,
/// status breakdown, and conversion rate.
///
/// Query params:
/// - `start_date` / `end_date`: ISO 8601 UTC, optional
/// - `currency`: "USDT" or "USDC", optional (returns both if omitted)
/// - `sub_merchant_code`: optional sub-merchant filter
/// - `include_sub_merchants`: bool, defaults false
async fn get_analytics(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(params): Query<AnalyticsQuery>,
    Query(sm_filter): Query<SubMerchantFilter>,
) -> Result<Json<AnalyticsResponse>, AppError> {
    // Resolve merchant IDs (parent + optional sub-merchants)
    let (merchant_ids, _code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    // Build WHERE clause fragments
    let (where_clauses, bind_values) = build_where_clause(&merchant_ids, &params);

    // === Query 1: KPIs (per currency, successful payments only) ===
    let success_filter = " AND status IN ('Paid', 'Overpaid')";
    let kpi_sql = format!(
        r#"SELECT
            currency,
            COALESCE(SUM(amount_received), 0)::bigint AS gross_volume,
            COALESCE(SUM(COALESCE(net_amount, 0)), 0)::bigint AS net_revenue,
            COALESCE(SUM(COALESCE(fee_amount, 0)), 0)::bigint AS fee_total,
            COUNT(*)::bigint AS tx_count
        FROM checkout_sessions
        WHERE {}{}
        GROUP BY currency
        ORDER BY currency"#,
        where_clauses, success_filter
    );

    let kpi_rows = KpiRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        &kpi_sql,
        bind_values.clone(),
    ))
    .all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let kpis: Vec<KpiEntry> = kpi_rows
        .into_iter()
        .map(|r| KpiEntry {
            currency: r.currency,
            gross_volume: r.gross_volume.unwrap_or(0),
            net_revenue: r.net_revenue.unwrap_or(0),
            fee_total: r.fee_total.unwrap_or(0),
            tx_count: r.tx_count.unwrap_or(0),
        })
        .collect();

    // === Query 2: Time Series (daily, successful payments) ===
    let ts_sql = format!(
        r#"SELECT
            TO_CHAR(DATE_TRUNC('day', created_at), 'YYYY-MM-DD') AS date,
            currency,
            COALESCE(SUM(amount_received), 0)::bigint AS volume,
            COUNT(*)::bigint AS count
        FROM checkout_sessions
        WHERE {}{}
        GROUP BY DATE_TRUNC('day', created_at), currency
        ORDER BY DATE_TRUNC('day', created_at)"#,
        where_clauses, success_filter
    );

    let ts_rows = TimeSeriesRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        &ts_sql,
        bind_values.clone(),
    ))
    .all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let time_series: Vec<TimeSeriesPoint> = ts_rows
        .into_iter()
        .filter_map(|r| {
            r.date.map(|d| TimeSeriesPoint {
                date: d,
                currency: r.currency,
                volume: r.volume.unwrap_or(0),
                count: r.count.unwrap_or(0),
            })
        })
        .collect();

    // === Query 3: Network Distribution (successful payments) ===
    let net_sql = format!(
        r#"SELECT
            network AS label,
            COUNT(*)::bigint AS value
        FROM checkout_sessions
        WHERE {}{}
        GROUP BY network
        ORDER BY value DESC"#,
        where_clauses, success_filter
    );

    let net_rows = DistributionRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        &net_sql,
        bind_values.clone(),
    ))
    .all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let network_distribution: Vec<DistributionEntry> = net_rows
        .into_iter()
        .map(|r| DistributionEntry {
            label: r.label,
            value: r.value.unwrap_or(0),
        })
        .collect();

    // === Query 4: Status Breakdown (all terminal statuses, excluding Pending) ===
    let status_sql = format!(
        r#"SELECT
            status AS label,
            COUNT(*)::bigint AS value
        FROM checkout_sessions
        WHERE {} AND status != 'Pending'
        GROUP BY status
        ORDER BY value DESC"#,
        where_clauses
    );

    let status_rows = DistributionRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        &status_sql,
        bind_values.clone(),
    ))
    .all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let status_breakdown: Vec<DistributionEntry> = status_rows
        .into_iter()
        .map(|r| DistributionEntry {
            label: r.label,
            value: r.value.unwrap_or(0),
        })
        .collect();

    // === Compute conversion rate from status breakdown ===
    let conversion_rate = compute_conversion_rate(&status_breakdown);

    Ok(Json(AnalyticsResponse {
        kpis,
        time_series,
        network_distribution,
        status_breakdown,
        conversion_rate,
    }))
}

/// Build parameterized WHERE clause for analytics queries.
///
/// Returns (where_clause_string, bind_values).
/// Uses $1, $2, ... placeholders for Postgres.
fn build_where_clause(
    merchant_ids: &[String],
    params: &AnalyticsQuery,
) -> (String, Vec<sea_orm::Value>) {
    let mut clauses = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    let mut idx = 1;

    // merchant_id IN (...)
    if merchant_ids.len() == 1 {
        clauses.push(format!("merchant_id = ${}", idx));
        values.push(merchant_ids[0].clone().into());
        idx += 1;
    } else {
        let placeholders: Vec<String> = merchant_ids
            .iter()
            .map(|id| {
                let p = format!("${}", idx);
                values.push(id.clone().into());
                idx += 1;
                p
            })
            .collect();
        clauses.push(format!("merchant_id IN ({})", placeholders.join(", ")));
    }

    // Date range
    if let Some(ref start) = params.start_date {
        clauses.push(format!("created_at >= ${}", idx));
        values.push((*start).into());
        idx += 1;
    }
    if let Some(ref end) = params.end_date {
        clauses.push(format!("created_at <= ${}", idx));
        values.push((*end).into());
        idx += 1;
    }

    // Currency filter
    if let Some(ref currency) = params.currency {
        clauses.push(format!("currency = ${}", idx));
        values.push(currency.clone().into());
        // idx += 1; // last param
    }

    (clauses.join(" AND "), values)
}

/// Compute conversion rate from status breakdown entries.
///
/// Formula: (Paid + Overpaid) / (Paid + Overpaid + Expired + Underpaid + Blocked)
/// Pending sessions are excluded (they haven't reached terminal state).
fn compute_conversion_rate(statuses: &[DistributionEntry]) -> f64 {
    let get = |label: &str| -> i64 {
        statuses
            .iter()
            .find(|s| s.label == label)
            .map(|s| s.value)
            .unwrap_or(0)
    };

    let paid = get("Paid");
    let overpaid = get("Overpaid");
    let expired = get("Expired");
    let underpaid = get("Underpaid");
    let blocked = get("Blocked");

    let success = paid + overpaid;
    let total = success + expired + underpaid + blocked;

    if total == 0 {
        0.0
    } else {
        success as f64 / total as f64
    }
}
