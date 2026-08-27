//! Webhooks API Routes
//!
//! Handles webhook configuration and logs for merchants.
//! JWT auth is applied by the parent router (`/api/internal`).

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use validator::Validate;

use crate::api::dtos::pagination::PaginatedResponse;
use crate::api::dtos::webhooks::{
    RotateSecretResponse, UpdateWebhookConfigRequest, WebhookConfigResponse, WebhookLogResponse,
};
use crate::api::error::{AppError, E_PARAMETER_INVALID};
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::entity::webhook_endpoints;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/config",
            get(get_config).put(update_config).delete(delete_config),
        )
        .route("/config/rotate-secret", post(rotate_secret))
        .route("/logs", get(list_logs))
        .route("/logs/:id/resend", post(resend_log))
}

/// GET /api/internal/webhooks/config
async fn get_config(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<Option<WebhookConfigResponse>>, AppError> {
    let config = state
        .webhook_service
        .get_config(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?;

    match config {
        Some(model) => Ok(Json(Some(model.into()))),
        None => Ok(Json(None)),
    }
}

/// PUT /api/internal/webhooks/config
///
/// Creates or updates webhook config. On create, returns the initial plaintext
/// secret in the response (one-time reveal). On update, secret stays masked.
async fn update_config(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<UpdateWebhookConfigRequest>,
) -> Result<Json<WebhookConfigResponse>, AppError> {
    body.validate()?;

    let status = match body.status.as_deref() {
        Some("enabled") => Some(webhook_endpoints::EndpointStatus::Enabled),
        Some("disabled") => Some(webhook_endpoints::EndpointStatus::Disabled),
        Some(invalid) => {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: format!(
                    "Invalid status '{}'. Must be 'enabled' or 'disabled'.",
                    invalid
                ),
                param: Some("status".into()),
            })
        }
        None => None,
    };

    let (model, plain_secret) = state
        .webhook_service
        .update_config(&merchant.id, merchant.environment, body.url, status, false)
        .await
        .map_err(|e| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: e.to_string(),
            param: Some("url".into()),
        })?;

    let mut resp: WebhookConfigResponse = model.into();
    // On initial creation, plain_secret is returned — reveal it once.
    if let Some(s) = plain_secret {
        resp.secret = s;
    }

    Ok(Json(resp))
}

/// POST /api/internal/webhooks/config/rotate-secret
///
/// Rotates the webhook signing secret. Old secret is invalidated immediately.
/// Returns the new plaintext secret (one-time reveal).
async fn rotate_secret(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<RotateSecretResponse>, AppError> {
    let (_model, plain_secret) = state
        .webhook_service
        .update_config(&merchant.id, merchant.environment, None, None, true)
        .await
        .map_err(|e| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: e.to_string(),
            param: None,
        })?;

    // rotate=true always produces a plain_secret
    let secret = plain_secret.expect("rotate_secret must return a new secret");

    Ok(Json(RotateSecretResponse { secret }))
}

/// DELETE /api/internal/webhooks/config
///
/// Removes the webhook configuration for the current environment.
async fn delete_config(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .webhook_service
        .delete_config(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// GET /api/internal/webhooks/logs
async fn list_logs(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(filter): Query<crate::api::dtos::webhooks::WebhookLogFilter>,
) -> Result<Json<PaginatedResponse<WebhookLogResponse>>, AppError> {
    filter.pagination.validate()?;

    let (data, total) = state
        .webhook_service
        .list_logs(&merchant.id, &filter)
        .await
        .map_err(AppError::InternalServerError)?;

    let dtos: Vec<WebhookLogResponse> = data.into_iter().map(|m| m.into()).collect();

    Ok(Json(PaginatedResponse::new(
        dtos,
        total,
        filter.pagination.page,
        filter.pagination.page_size,
    )))
}

/// POST /api/internal/webhooks/logs/:id/resend
async fn resend_log(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .webhook_service
        .resend_event(&id, &merchant.id)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}
