//! Branding API — merchant logo upload/delete via R2 storage
//!
//! Routes:
//! - GET  /api/internal/branding     → current branding settings
//! - POST /api/internal/branding/logo → upload logo (multipart)
//! - DELETE /api/internal/branding/logo → delete logo

use axum::{
    extract::State,
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::Multipart;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::Serialize;

use crate::api::error::AppError;
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::entity::merchants;
use crate::services::AppState;

#[derive(Serialize)]
pub struct BrandingResponse {
    pub logo_url: Option<String>,
}

/// GET /api/internal/branding
async fn get_branding(
    merchant: AuthenticatedMerchant,
    State(state): State<AppState>,
) -> Result<Json<BrandingResponse>, AppError> {
    let merchant_model = merchants::Entity::find_by_id(&merchant.id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Merchant not found".into()))?;

    Ok(Json(BrandingResponse {
        logo_url: merchant_model.logo_url,
    }))
}

/// POST /api/internal/branding/logo
///
/// Accepts multipart form data with a single file field named "logo".
/// Max 2MB, PNG/JPEG/WebP only.
async fn upload_logo(
    merchant: AuthenticatedMerchant,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<BrandingResponse>, AppError> {
    let r2 = state.r2_storage.as_ref().ok_or_else(|| {
        AppError::InternalServerError(anyhow::anyhow!("R2 storage not configured"))
    })?;

    // Extract file from multipart
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Err(AppError::ValidationError {
                code: "missing_field",
                message: "No file field found in request".into(),
                param: Some("logo".into()),
            });
        }
        Err(e) => {
            return Err(AppError::ValidationError {
                code: "invalid_request",
                message: format!("Invalid multipart data: {}", e),
                param: Some("logo".into()),
            });
        }
    };

    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    let data = match field.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            return Err(AppError::ValidationError {
                code: "invalid_request",
                message: format!("Failed to read file data: {}", e),
                param: Some("logo".into()),
            });
        }
    };

    // Upload to R2 (validates size + MIME internally)
    let logo_url = r2
        .upload_logo(&merchant.id, data, &content_type)
        .await
        .map_err(|e| AppError::ValidationError {
            code: "invalid_file",
            message: e.to_string(),
            param: Some("logo".into()),
        })?;

    // Delete old logo from R2 if replacing (prevent orphaned objects)
    let merchant_model = merchants::Entity::find_by_id(&merchant.id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Merchant not found".into()))?;

    if let Some(ref old_url) = merchant_model.logo_url {
        if let Err(e) = r2.delete_logo(old_url).await {
            tracing::warn!(
                merchant_id = %merchant.id,
                error = %e,
                "Failed to delete old logo from R2 (continuing with upload)"
            );
        }
    }

    // Update merchant record
    let mut active: merchants::ActiveModel = merchant_model.into();

    active.logo_url = Set(Some(logo_url.clone()));
    active.updated_at = Set(chrono::Utc::now().into());
    active
        .update(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    tracing::info!(
        merchant_id = %merchant.id,
        logo_url = %logo_url,
        "Merchant logo updated"
    );

    Ok(Json(BrandingResponse {
        logo_url: Some(logo_url),
    }))
}

/// DELETE /api/internal/branding/logo
async fn delete_logo(
    merchant: AuthenticatedMerchant,
    State(state): State<AppState>,
) -> Result<Json<BrandingResponse>, AppError> {
    let merchant_model = merchants::Entity::find_by_id(&merchant.id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Merchant not found".into()))?;

    // Delete from R2 if exists
    if let Some(ref logo_url) = merchant_model.logo_url {
        if let Some(r2) = state.r2_storage.as_ref() {
            if let Err(e) = r2.delete_logo(logo_url).await {
                tracing::warn!(
                    merchant_id = %merchant.id,
                    error = %e,
                    "Failed to delete logo from R2 (continuing with DB cleanup)"
                );
            }
        }
    }

    // Clear DB field
    let mut active: merchants::ActiveModel = merchant_model.into();
    active.logo_url = Set(None);
    active.updated_at = Set(chrono::Utc::now().into());
    active
        .update(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    tracing::info!(
        merchant_id = %merchant.id,
        "Merchant logo deleted"
    );

    Ok(Json(BrandingResponse { logo_url: None }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_branding))
        .route(
            "/logo",
            post(upload_logo).layer(
                tower_http::limit::RequestBodyLimitLayer::new(3 * 1024 * 1024), // 3MB framework limit
            ),
        )
        .route("/logo", delete(delete_logo))
}
