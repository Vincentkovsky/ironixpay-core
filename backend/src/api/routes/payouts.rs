//! Payout API Routes
//!
//! Public API for merchant-initiated payouts to arbitrary addresses.
//! Uses API Key authentication (same as Checkout API).
//!
//! Requires `Idempotency-Key` header on POST requests.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use validator::Validate;

use crate::api::dtos::pagination::{PaginatedResponse, PaginationRequest};
use crate::api::dtos::payouts::{CreatePayoutBody, PayoutResponse};
use crate::api::error::{AppError, AppJson, E_PARAMETER_INVALID};
use crate::api::middleware::auth::{auth_middleware, AuthenticatedMerchant};
use crate::entity::Network;
use crate::AppState;

use crate::api::middleware::sub_merchant::sub_merchant_scope;

/// Payout API router. All routes require API Key auth.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_payout).get(list_payouts))
        .route("/:id", get(get_payout))
        .route_layer(middleware::from_fn(sub_merchant_scope))
        .route_layer(middleware::from_fn(auth_middleware))
}

/// POST /v1/payouts
///
/// Creates a new payout to the specified address.
///
/// The payout is created in `Pending` status and processed asynchronously.
/// The Payout Worker will broadcast the on-chain transaction and update status.
///
/// **Requires `Idempotency-Key` header** (recommended: UUID v4).
#[utoipa::path(
    post,
    path = "/v1/payouts",
    tag = "Payouts",
    security(("bearer_auth" = [])),
    params(
        ("Idempotency-Key" = String, Header, description = "Unique key for idempotent requests (recommended: UUID v4)"),
    ),
    request_body = CreatePayoutBody,
    responses(
        (status = 201, description = "Payout created", body = PayoutResponse),
        (status = 400, description = "Validation error", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "insufficient_balance", "message": "Insufficient balance: have 5.00 USDT, need 10.00 USDT", "param": "amount", "doc_url": "https://ironixpay.com/guide/errors#insufficient_balance"}})),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "authentication_failed", "message": "Invalid API key provided", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})),
        (status = 409, description = "Idempotency conflict", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"type": "idempotency_error", "code": "idempotency_conflict", "message": "Idempotency key already used for a different payout", "param": null, "doc_url": "https://ironixpay.com/guide/errors#idempotency_conflict"}})),
    )
)]
pub async fn create_payout(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    headers: HeaderMap,
    AppJson(body): AppJson<CreatePayoutBody>,
) -> Result<impl IntoResponse, AppError> {
    // Validate request body
    body.validate()?;

    // Require Idempotency-Key header
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Missing required header: Idempotency-Key".into(),
            param: Some("Idempotency-Key".into()),
        })?;

    // Parse amount: accept human-readable decimal string (e.g., "10.50")
    // Convert to microunits internally via to_micro()
    let amount_decimal: rust_decimal::Decimal =
        body.amount.parse().map_err(|_| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Invalid amount: must be a numeric decimal string (e.g., \"10.50\")".into(),
            param: Some("amount".into()),
        })?;

    if amount_decimal <= rust_decimal::Decimal::ZERO {
        return Err(AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Amount must be positive".into(),
            param: Some("amount".into()),
        });
    };

    let amount: i64 = crate::api::dtos::checkout::to_micro(amount_decimal, &body.currency)
        .ok_or_else(|| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Amount is too large or has too many decimal places".into(),
            param: Some("amount".into()),
        })?;

    // Validate network is enabled
    if !state.enabled_networks.contains(&body.network) {
        return Err(AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: format!(
                "The requested network '{}' is not supported. Supported networks: {:?}",
                body.network.as_str(),
                state
                    .enabled_networks
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
            ),
            param: Some("network".into()),
        });
    }

    // Validate metadata size (max 4KB to prevent DB bloat)
    if let Some(ref meta) = body.metadata {
        let serialized = serde_json::to_string(meta).unwrap_or_default();
        if serialized.len() > 4096 {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: "Metadata exceeds maximum size of 4KB".into(),
                param: Some("metadata".into()),
            });
        }
    }

    // Block USDC on networks without USDC contract (e.g. TRON)
    if body.currency == "USDC" {
        let env = state.config.environment.to_entity_environment();
        let cc = body.network.chain_config(&env);
        if cc.usdc_contract.is_none() {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: format!(
                    "USDC payouts are not supported on {}. Use USDT or choose a different network.",
                    body.network.as_str()
                ),
                param: Some("currency".into()),
            });
        }
    }

    // Create payout via service
    let payout = state
        .payout_service
        .create_payout(
            &merchant.id,
            amount,
            merchant.environment,
            body.network,
            body.to_address,
            idempotency_key,
            body.description,
            body.metadata,
            &body.currency,
        )
        .await?;

    let livemode = Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let response = PayoutResponse::from_model(payout, livemode, None);

    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /v1/payouts/:id
///
/// Retrieves a payout by ID.
#[utoipa::path(
    get,
    path = "/v1/payouts/{id}",
    tag = "Payouts",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Payout ID (e.g. `po_abc123`)"),
    ),
    responses(
        (status = 200, description = "Payout found", body = PayoutResponse),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"code": "authentication_failed", "message": "Invalid API key provided", "type": "invalid_request_error", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})
        ),
        (status = 404, description = "Payout not found", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"code": "resource_missing", "message": "Payout 'po_abc123' not found", "type": "invalid_request_error", "param": null, "doc_url": "https://ironixpay.com/guide/errors#resource_missing"}})
        ),
    )
)]
pub async fn get_payout(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(payout_id): Path<String>,
) -> Result<Json<PayoutResponse>, AppError> {
    // Allow access to sub-merchant payouts too
    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(&merchant.id, true, None)
        .await?;

    let payout = state
        .payout_service
        .get_payout(&payout_id, &merchant_ids)
        .await
        .map_err(|e| AppError::InternalServerError(e))?
        .ok_or_else(|| AppError::NotFound(format!("Payout '{}' not found", payout_id)))?;

    let livemode = Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let sm_code = code_map.get(&payout.merchant_id).cloned();
    Ok(Json(PayoutResponse::from_model(payout, livemode, sm_code)))
}

/// GET /v1/payouts
///
/// Lists payouts for the authenticated merchant.
#[utoipa::path(
    get,
    path = "/v1/payouts",
    tag = "Payouts",
    security(("bearer_auth" = [])),
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<u64>, Query, description = "Items per page (default: 20, max: 100)"),
        ("include_sub_merchants" = Option<bool>, Query, description = "Include sub-merchant payouts"),
        ("sub_merchant_code" = Option<String>, Query, description = "Filter by sub-merchant code"),
    ),
    responses(
        (status = 200, description = "Paginated payouts", body = Vec<PayoutResponse>),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"code": "authentication_failed", "message": "Invalid API key provided", "type": "invalid_request_error", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})
        ),
    )
)]
pub async fn list_payouts(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(pagination): Query<PaginationRequest>,
    Query(sm_filter): Query<crate::api::routes::resolution::SubMerchantFilter>,
) -> Result<Json<PaginatedResponse<PayoutResponse>>, AppError> {
    pagination.validate()?;

    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    let (payouts, total) = state
        .payout_service
        .list_payouts(
            &merchant_ids,
            merchant.environment,
            pagination.page,
            pagination.page_size,
        )
        .await
        .map_err(|e| AppError::InternalServerError(e))?;

    let livemode = Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let items: Vec<PayoutResponse> = payouts
        .into_iter()
        .map(|p| {
            let sm_code = code_map.get(&p.merchant_id).cloned();
            PayoutResponse::from_model(p, livemode, sm_code)
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        items,
        total,
        pagination.page,
        pagination.page_size,
    )))
}
