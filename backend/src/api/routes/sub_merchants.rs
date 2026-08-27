//! Sub-Merchant routes — public (API Key auth) and internal (JWT auth).
//!
//! Public API: `/v1/sub-merchants` — PSP self-service CRUD.
//!   Auth: auth_middleware only, NO sub_merchant_scope.
//!
//! Internal API: `/api/internal/sub-merchants` — Dashboard queries.
//!   Auth: jwt_auth (applied at group level in api/mod.rs).

use axum::{
    extract::{Extension, Path, Query},
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

use crate::api::error::AppError;
use crate::api::middleware::auth::{auth_middleware, AuthenticatedMerchant};
use crate::services::sub_merchant::{CreateSubMerchantInput, Pagination, UpdateSubMerchantInput};
use crate::AppState;

// ─── DTOs ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateBody {
    /// Unique code for this sub-merchant (e.g. `shop_tokyo`). Must not start with `_`.
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "shop_tokyo")]
    pub sub_merchant_code: String,
    /// Human-readable display name.
    #[validate(length(min = 1, max = 200))]
    #[schema(example = "Tokyo Branch")]
    pub display_name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateBody {
    /// Updated display name (optional).
    #[validate(length(min = 1, max = 200))]
    #[schema(example = "Tokyo Main Branch")]
    pub display_name: Option<String>,
    /// Updated status (optional). `active` or `suspended`.
    pub status: Option<crate::entity::sub_merchants::SubMerchantStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub(crate) struct ListParams {
    #[serde(default = "default_page")]
    #[validate(range(min = 1, max = 1000))]
    page: u64,
    #[serde(default = "default_page_size")]
    #[validate(range(min = 1, max = 100))]
    page_size: u64,
}

fn default_page() -> u64 {
    1
}
fn default_page_size() -> u64 {
    20
}

// ─── Public Router (API Key auth, no sub_merchant_scope) ────

/// Public API router: `/v1/sub-merchants`
///
/// PSP merchants use their API Key to manage sub-merchants.
/// NO sub_merchant_scope — the PSP's own org_id is used as parent.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/:code", get(get_by_code).patch(update))
        .route_layer(middleware::from_fn(auth_middleware))
}

/// Internal API router: `/api/internal/sub-merchants`
///
/// Dashboard access for PSP merchants via JWT auth.
/// JWT auth is applied at group level in api/mod.rs, not here.
pub fn internal_router() -> Router<AppState> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/stats", get(stats))
        .route("/:code", get(get_by_code).patch(update))
}

// ─── Handlers (shared between public and internal) ──────────

/// Create Sub-Merchant
#[utoipa::path(
    post,
    path = "/v1/sub-merchants",
    operation_id = "create_sub_merchant",
    tag = "Sub-Merchants",
    security(("bearer_auth" = [])),
    request_body = CreateBody,
    responses(
        (status = 201, description = "Sub-merchant created", body = crate::services::sub_merchant::SubMerchantResponse),
        (status = 400, description = "Validation error (e.g. reserved code, duplicate)", body = crate::api::error::ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "reserved_code", "message": "Sub-merchant code '_self' is reserved and cannot be used", "param": "sub_merchant_code", "doc_url": null}})),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse),
    )
)]
pub async fn create(
    merchant: AuthenticatedMerchant,
    Extension(state): Extension<AppState>,
    Json(body): Json<CreateBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    body.validate()?;

    let result = state
        .sub_merchant_service
        .create(CreateSubMerchantInput {
            parent_org_id: merchant.id,
            sub_merchant_code: body.sub_merchant_code,
            display_name: body.display_name,
        })
        .await?;

    Ok(Json(serde_json::json!(result)))
}

/// List Sub-Merchants
#[utoipa::path(
    get,
    path = "/v1/sub-merchants",
    operation_id = "list_sub_merchants",
    tag = "Sub-Merchants",
    security(("bearer_auth" = [])),
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<u64>, Query, description = "Items per page (default: 20, max: 100)"),
    ),
    responses(
        (status = 200, description = "Paginated sub-merchant list", body = Vec<crate::services::sub_merchant::SubMerchantResponse>),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse),
    )
)]
pub(crate) async fn list(
    merchant: AuthenticatedMerchant,
    Extension(state): Extension<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    params.validate()?;

    let result = state
        .sub_merchant_service
        .list(
            Some(&merchant.id),
            Pagination {
                page: params.page,
                page_size: params.page_size,
            },
        )
        .await?;

    Ok(Json(serde_json::json!({
        "items": result.items,
        "total": result.total,
        "page": result.page,
        "page_size": result.page_size,
    })))
}

/// Get Sub-Merchant
#[utoipa::path(
    get,
    path = "/v1/sub-merchants/{code}",
    operation_id = "get_sub_merchant",
    tag = "Sub-Merchants",
    security(("bearer_auth" = [])),
    params(
        ("code" = String, Path, description = "Sub-merchant code (e.g. `shop_tokyo`)"),
    ),
    responses(
        (status = 200, description = "Sub-merchant details", body = crate::services::sub_merchant::SubMerchantResponse),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse),
        (status = 404, description = "Sub-merchant not found", body = crate::api::error::ApiErrorResponse),
    )
)]
pub async fn get_by_code(
    merchant: AuthenticatedMerchant,
    Extension(state): Extension<AppState>,
    Path(code): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = state
        .sub_merchant_service
        .get_by_code(&merchant.id, &code)
        .await?;

    Ok(Json(serde_json::json!(result)))
}

/// Update Sub-Merchant
#[utoipa::path(
    patch,
    path = "/v1/sub-merchants/{code}",
    operation_id = "update_sub_merchant",
    tag = "Sub-Merchants",
    security(("bearer_auth" = [])),
    params(
        ("code" = String, Path, description = "Sub-merchant code (e.g. `shop_tokyo`)"),
    ),
    request_body = UpdateBody,
    responses(
        (status = 200, description = "Sub-merchant updated", body = crate::services::sub_merchant::SubMerchantResponse),
        (status = 400, description = "Validation error", body = crate::api::error::ApiErrorResponse),
        (status = 401, description = "Invalid API key", body = crate::api::error::ApiErrorResponse),
        (status = 404, description = "Sub-merchant not found", body = crate::api::error::ApiErrorResponse),
    )
)]
pub async fn update(
    merchant: AuthenticatedMerchant,
    Extension(state): Extension<AppState>,
    Path(code): Path<String>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    body.validate()?;

    let result = state
        .sub_merchant_service
        .update_by_code(
            &merchant.id,
            &code,
            UpdateSubMerchantInput {
                display_name: body.display_name,
                status: body.status,
            },
        )
        .await?;

    Ok(Json(serde_json::json!(result)))
}

/// GET /stats — Aggregated transaction stats across all sub-merchants.
async fn stats(
    merchant: AuthenticatedMerchant,
    Extension(state): Extension<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = state.sub_merchant_service.get_stats(&merchant.id).await?;

    Ok(Json(serde_json::json!(result)))
}
