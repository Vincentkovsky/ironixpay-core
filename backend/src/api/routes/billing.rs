//! Billing API Routes
//!
//! Handles billing logs and sweep history.
//! Enforces strict environment isolation via `X-Environment` header.
//! JWT auth is applied by the parent router (`/api/internal`).

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::NaiveTime;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use validator::Validate;

use crate::api::dtos::billing::{
    BillingExportRequest, BillingLogResponse, BillingLogsFilter, PaymentsExportRequest,
};
use crate::api::dtos::pagination::{PaginatedResponse, PaginationRequest};
use crate::api::error::AppError;
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::entity::{billing_logs, checkout_sessions, transactions};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/logs", get(get_billing_logs))
        .route("/logs/export", get(export_billing_csv))
        .route("/logs/:id", get(get_billing_log_detail))
        .route("/payments/export", get(export_payments_csv))
}

/// GET /api/internal/billing/logs
///
/// Returns paginated billing logs (credits, withdrawals, refunds).
/// Filtered by merchant and environment. Optionally filtered by network.
async fn get_billing_logs(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(pagination): Query<PaginationRequest>,
    Query(params): Query<BillingLogsFilter>,
    Query(sm_filter): Query<crate::api::routes::resolution::SubMerchantFilter>,
) -> Result<Json<PaginatedResponse<BillingLogResponse>>, AppError> {
    pagination.validate()?;

    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    let mut query = billing_logs::Entity::find()
        .filter(billing_logs::Column::MerchantId.is_in(&merchant_ids))
        .filter(billing_logs::Column::Environment.eq(merchant.environment.clone()));

    // Optional network filter (e.g. "BSC", "POLYGON")
    if let Some(ref network) = params.network {
        if !network.is_empty() {
            query = query.filter(billing_logs::Column::Network.eq(network.as_str()));
        }
    }

    let paginator = query
        .order_by_desc(billing_logs::Column::CreatedAt)
        .paginate(&state.db, pagination.page_size);

    let total = paginator
        .num_items()
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;
    let data = paginator
        .fetch_page(pagination.page - 1)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    let data: Vec<BillingLogResponse> = data
        .into_iter()
        .map(|log| {
            let sm_code = code_map.get(&log.merchant_id).cloned();
            BillingLogResponse::from_model(log, sm_code)
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/internal/billing/logs/export
///
/// Exports billing logs as CSV file.
/// Supports optional date range and type filtering.
async fn export_billing_csv(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(params): Query<BillingExportRequest>,
    Query(sm_filter): Query<crate::api::routes::resolution::SubMerchantFilter>,
) -> Result<impl IntoResponse, AppError> {
    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    let mut query = billing_logs::Entity::find()
        .filter(billing_logs::Column::MerchantId.is_in(&merchant_ids))
        .filter(billing_logs::Column::Environment.eq(merchant.environment.clone()));

    // Date range filter
    if let Some(start) = params.start_date {
        let start_dt = start.and_time(NaiveTime::MIN).and_utc();
        query = query.filter(billing_logs::Column::CreatedAt.gte(start_dt));
    }
    if let Some(end) = params.end_date {
        // End date is inclusive — use end of day
        let end_dt = end
            .and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
            .and_utc();
        query = query.filter(billing_logs::Column::CreatedAt.lte(end_dt));
    }

    // Type filter
    if let Some(ref billing_type) = params.billing_type {
        if billing_type != "all" {
            query = query.filter(billing_logs::Column::BillingType.eq(billing_type.as_str()));
        }
    }

    let rows = query
        .order_by_asc(billing_logs::Column::CreatedAt)
        .all(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    // Build CSV
    let mut wtr = csv::Writer::from_writer(Vec::new());

    // Header row
    wtr.write_record([
        "Date",
        "Type",
        "Network",
        "Reference",
        "Sub-Merchant",
        "Currency",
        "Amount",
        "Balance After",
        "Description",
    ])
    .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV write error: {}", e)))?;

    for row in &rows {
        let amount = crate::api::dtos::checkout::from_micro(row.amount_change, &row.token);
        let balance = crate::api::dtos::checkout::from_micro(row.balance_after, &row.token);
        let type_label = match row.billing_type {
            billing_logs::BillingType::PaymentCredit => "Payment Credit",
            billing_logs::BillingType::Withdrawal => "Withdrawal",
            billing_logs::BillingType::Refund => "Refund",
            billing_logs::BillingType::Payout => "Payout",
        };

        let sm_code = code_map.get(&row.merchant_id).cloned().unwrap_or_default();
        wtr.write_record([
            row.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            type_label.to_string(),
            row.network.clone(),
            row.external_ref_id.clone().unwrap_or_default(),
            sm_code,
            row.token.clone(),
            amount,
            balance,
            row.description.clone().unwrap_or_default(),
        ])
        .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV write error: {}", e)))?;
    }

    let csv_bytes = wtr
        .into_inner()
        .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV flush error: {}", e)))?;

    // Prepend UTF-8 BOM for Excel CJK compatibility
    let mut body = Vec::with_capacity(3 + csv_bytes.len());
    body.extend_from_slice(b"\xEF\xBB\xBF");
    body.extend_from_slice(&csv_bytes);

    // Build filename with date range
    let filename = match (params.start_date, params.end_date) {
        (Some(s), Some(e)) => format!("billing_{}_{}.csv", s, e),
        (Some(s), None) => format!("billing_{}_all.csv", s),
        (None, Some(e)) => format!("billing_all_{}.csv", e),
        (None, None) => "billing_all.csv".to_string(),
    };

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        body,
    ))
}

/// GET /api/internal/billing/logs/:id
///
/// Returns a single billing log by ID.
/// Enforces merchant ownership check.
async fn get_billing_log_detail(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
) -> Result<Json<BillingLogResponse>, AppError> {
    let log = billing_logs::Entity::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Billing log '{}' not found", id)))?;

    // Get allowed merchant_ids for IDOR check
    let allowed_ids = state
        .sub_merchant_service
        .get_all_child_org_ids(&merchant.id)
        .await
        .unwrap_or_default();
    let is_own = log.merchant_id == merchant.id;
    let is_child = allowed_ids.contains(&log.merchant_id);

    // IDOR + Environment Protection
    if (!is_own && !is_child) || log.environment != merchant.environment {
        return Err(AppError::NotFound(format!(
            "Billing log '{}' not found",
            id
        )));
    }

    let code_map = state
        .sub_merchant_service
        .get_code_map(&merchant.id)
        .await
        .unwrap_or_default();
    let sm_code = code_map.get(&log.merchant_id).cloned();

    Ok(Json(BillingLogResponse::from_model(log, sm_code)))
}

/// GET /api/internal/billing/payments/export
///
/// Exports checkout sessions as CSV (Stripe-style Payments report).
/// Includes session-level details and associated transaction hashes.
async fn export_payments_csv(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(params): Query<PaymentsExportRequest>,
    Query(sm_filter): Query<crate::api::routes::resolution::SubMerchantFilter>,
) -> Result<impl IntoResponse, AppError> {
    let (merchant_ids, _code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    // Export sessions across all networks for this merchant
    let mut query = checkout_sessions::Entity::find()
        .filter(checkout_sessions::Column::MerchantId.is_in(&merchant_ids));

    // Date range filter
    if let Some(start) = params.start_date {
        let start_dt = start.and_time(NaiveTime::MIN).and_utc();
        query = query.filter(checkout_sessions::Column::CreatedAt.gte(start_dt));
    }
    if let Some(end) = params.end_date {
        let end_dt = end
            .and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
            .and_utc();
        query = query.filter(checkout_sessions::Column::CreatedAt.lte(end_dt));
    }

    // Status filter
    if let Some(ref status) = params.status {
        if status != "all" {
            query = query.filter(checkout_sessions::Column::Status.eq(status.as_str()));
        }
    }

    // Fetch sessions with related transactions in one query
    let sessions_with_txs = query
        .order_by_asc(checkout_sessions::Column::CreatedAt)
        .find_with_related(transactions::Entity)
        .all(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    // Build CSV
    let mut wtr = csv::Writer::from_writer(Vec::new());

    // Stripe-style header row
    wtr.write_record([
        "id",
        "Created (UTC)",
        "Amount",
        "Amount Received",
        "Fee",
        "Net",
        "Currency",
        "Status",
        "Client Reference ID",
        "Sub-Merchant",
        "Network",
        "Payment Method",
        "Tx Hash",
    ])
    .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV write error: {}", e)))?;

    for (session, txs) in &sessions_with_txs {
        // Skip Expired sessions with no payment — they add no value for reconciliation
        if matches!(session.status, checkout_sessions::SessionStatus::Expired)
            && session.amount_received == 0
        {
            continue;
        }

        let amount =
            crate::api::dtos::checkout::from_micro(session.amount_expected, &session.currency);
        let received =
            crate::api::dtos::checkout::from_micro(session.amount_received, &session.currency);
        // NULL fee/net → empty string (not 0) to distinguish "no data" from "zero fee"
        let fee_str = session
            .fee_amount
            .map(|v| crate::api::dtos::checkout::from_micro(v, &session.currency))
            .unwrap_or_default();
        let net_str = session
            .net_amount
            .map(|v| crate::api::dtos::checkout::from_micro(v, &session.currency))
            .unwrap_or_default();

        // Collect confirmed tx hashes, comma-separated
        let tx_hashes: Vec<&str> = txs.iter().map(|t| t.tx_hash.as_str()).collect();
        let tx_id = tx_hashes.join(", ");

        let status_str = format!("{:?}", session.status);

        let payment_method = format!("{} ({})", &session.currency, &session.network);

        wtr.write_record([
            session.id.as_str(),
            &session.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            &amount,
            &received,
            &fee_str,
            &net_str,
            &session.currency,
            &status_str,
            session.client_reference_id.as_deref().unwrap_or(""),
            session.sub_merchant_code.as_deref().unwrap_or(""),
            &session.network,
            &payment_method,
            &tx_id,
        ])
        .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV write error: {}", e)))?;
    }

    let csv_bytes = wtr
        .into_inner()
        .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("CSV flush error: {}", e)))?;

    // Prepend UTF-8 BOM for Excel CJK compatibility
    let mut body = Vec::with_capacity(3 + csv_bytes.len());
    body.extend_from_slice(b"\xEF\xBB\xBF");
    body.extend_from_slice(&csv_bytes);

    // Build filename with date range
    let filename = match (params.start_date, params.end_date) {
        (Some(s), Some(e)) => format!("payments_{}_{}.csv", s, e),
        (Some(s), None) => format!("payments_{}_all.csv", s),
        (None, Some(e)) => format!("payments_all_{}.csv", e),
        (None, None) => "payments_all.csv".to_string(),
    };

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        body,
    ))
}
