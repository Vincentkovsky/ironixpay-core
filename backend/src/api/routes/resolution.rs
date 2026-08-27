//! Resolution API Routes
//!
//! JWT auth is applied by the parent router (`/api/internal`).

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use validator::Validate;

use crate::api::dtos::pagination::{PaginatedResponse, PaginationRequest};
use crate::api::dtos::resolution::{
    AttachRequest, ExceptionResponse, ResolutionStatsResponse, TransferRequest,
};
use crate::api::error::{AppError, E_PARAMETER_INVALID};
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::AppState;
use serde::Deserialize;
use std::collections::HashMap;

/// Shared sub-merchant filter params for mixed display.
#[derive(Deserialize, Default)]
pub struct SubMerchantFilter {
    #[serde(default)]
    pub include_sub_merchants: bool,
    #[serde(default)]
    pub sub_merchant_code: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/exceptions", get(list_exceptions))
        .route("/exceptions/:id/accept", post(accept_exception))
        .route("/exceptions/:id/transfer", post(transfer_exception))
        .route("/exceptions/:id/attach", post(manual_attach))
}

/// GET /api/internal/resolution/stats
async fn get_stats(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(sm_filter): Query<SubMerchantFilter>,
) -> Result<Json<ResolutionStatsResponse>, AppError> {
    // Resolve merchant_ids based on sub-merchant filter
    let (merchant_ids, _code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    // Pass None for network to show exceptions across all enabled networks
    let stats = state
        .resolution_service
        .get_stats(&merchant_ids, None)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(stats))
}

#[derive(Deserialize)]
struct ExceptionFilter {
    #[serde(default)]
    status: Option<String>, // "pending", "processing", "resolved", "failed"
    #[serde(default)]
    exception_type: Option<String>, // "SessionExpired", "NoActiveSession", "RiskBlocked", etc.
}

/// GET /api/internal/resolution/exceptions
async fn list_exceptions(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<ExceptionFilter>,
    Query(sm_filter): Query<SubMerchantFilter>,
) -> Result<Json<PaginatedResponse<ExceptionResponse>>, AppError> {
    pagination.validate()?;

    // Resolve merchant_ids based on sub-merchant filter
    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    tracing::debug!(
        merchant_id = %merchant.id,
        merchant_ids_count = merchant_ids.len(),
        status = ?filter.status,
        exception_type = ?filter.exception_type,
        "list_exceptions query params"
    );

    let (data, total) = state
        .resolution_service
        .list_exceptions(
            &merchant_ids,
            None, // Show all networks
            filter.status,
            filter.exception_type,
            pagination.search_text,
            pagination.page,
            pagination.page_size,
        )
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    // Batch-lookup sweep info for resolved exceptions (tx_hash + to_address)
    let exception_ids: Vec<String> = data.iter().map(|(ex, _)| ex.id.clone()).collect();
    let sweep_map = if !exception_ids.is_empty() {
        use crate::entity::outbound_transactions;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
        let sweeps = outbound_transactions::Entity::find()
            .filter(outbound_transactions::Column::ExceptionId.is_in(&exception_ids))
            .filter(
                outbound_transactions::Column::Purpose
                    .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
            )
            .order_by_desc(outbound_transactions::Column::CreatedAt)
            .all(&state.db)
            .await
            .unwrap_or_default();
        let mut map = HashMap::new();
        for sweep in sweeps {
            if let Some(exception_id) = sweep.exception_id {
                map.entry(exception_id)
                    .or_insert((sweep.tx_hash, sweep.to_address));
            }
        }
        map
    } else {
        HashMap::new()
    };

    let dtos: Vec<ExceptionResponse> = data
        .into_iter()
        .map(|(exception, session)| {
            let sweep_info = sweep_map.get(&exception.id).cloned();
            // Reverse lookup: exception.merchant_id → sub_merchant_code
            let sm_code = exception
                .merchant_id
                .as_deref()
                .and_then(|mid| code_map.get(mid).cloned());
            ExceptionResponse::from_model(exception, session, sweep_info, sm_code)
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        dtos,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// POST /api/internal/resolution/exceptions/:id/accept
async fn accept_exception(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Resolve allowed merchant_ids (parent + all children) for IDOR check
    let (merchant_ids, _) = state
        .sub_merchant_service
        .resolve_merchant_ids(&merchant.id, true, None)
        .await?;

    state
        .resolution_service
        .accept_expired_session(&id, &merchant.id, &merchant_ids)
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/internal/resolution/exceptions/:id/transfer
async fn transfer_exception(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
    Json(body): Json<TransferRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    body.validate().map_err(|e| AppError::ValidationError {
        code: E_PARAMETER_INVALID,
        message: e.to_string(),
        param: None,
    })?;

    // Resolve allowed merchant_ids (parent + all children) for IDOR check
    let (merchant_ids, _) = state
        .sub_merchant_service
        .resolve_merchant_ids(&merchant.id, true, None)
        .await?;

    state
        .resolution_service
        .manual_transfer(&id, &merchant.id, &merchant_ids, body)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "status": "processing",
        "message": "Transfer initiated. Check exception status for result."
    })))
}

/// POST /api/internal/resolution/exceptions/:id/attach
async fn manual_attach(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
    Json(body): Json<AttachRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Resolve allowed merchant_ids (parent + all children) for IDOR check
    let (merchant_ids, _) = state
        .sub_merchant_service
        .resolve_merchant_ids(&merchant.id, true, None)
        .await?;

    state
        .resolution_service
        .attach_session(&id, &merchant.id, &merchant_ids, &body.session_id)
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}
