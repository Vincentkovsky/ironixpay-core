//! Sub-Merchant Scope Middleware
//!
//! When a PSP sends `X-Sub-Merchant-Code` header, this middleware:
//! 1. Resolves the code to a child org ID via `sub_merchants` table
//! 2. Replaces `AuthenticatedMerchant.id` with the child org ID
//! 3. Stores original PSP context in `SubMerchantContext` extension
//!
//! Must be applied AFTER `auth_middleware` and ONLY to auth routes.

use axum::{extract::Extension, http::Request, middleware::Next, response::Response};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::{debug, warn};

use crate::api::error::AppError;
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::entity::sub_merchants::{self, SubMerchantStatus};
use crate::AppState;

/// Header name for sub-merchant code
const SUB_MERCHANT_CODE_HEADER: &str = "x-sub-merchant-code";

/// Context stored in request extensions when operating as a sub-merchant.
/// Allows downstream services (e.g., webhook) to find the original PSP org.
#[derive(Clone, Debug)]
pub struct SubMerchantContext {
    pub parent_org_id: String,
    pub sub_merchant_code: String,
    pub child_org_id: String,
}

/// Middleware that resolves `X-Sub-Merchant-Code` header to a child org.
///
/// If the header is absent, the request passes through unchanged.
/// If present, the `AuthenticatedMerchant.id` is replaced with the child org's ID.
pub async fn sub_merchant_scope(
    Extension(state): Extension<AppState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Extract header value (if present)
    let sub_merchant_code = request
        .headers()
        .get(SUB_MERCHANT_CODE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let code = match sub_merchant_code {
        Some(c) if !c.is_empty() => c,
        _ => {
            // No sub-merchant header — pass through (PSP's own transaction)
            return Ok(next.run(request).await);
        }
    };

    // AuthenticatedMerchant must exist (auth_middleware ran before us)
    let merchant = request
        .extensions()
        .get::<AuthenticatedMerchant>()
        .cloned()
        .ok_or_else(|| {
            warn!("SubMerchantScope: AuthenticatedMerchant not found in extensions");
            AppError::InternalServerError(anyhow::anyhow!("Authentication context missing"))
        })?;

    // Resolve the sub-merchant code
    let sub_merchant = sub_merchants::Entity::find()
        .filter(sub_merchants::Column::ParentOrgId.eq(&merchant.id))
        .filter(sub_merchants::Column::SubMerchantCode.eq(&code))
        .filter(sub_merchants::Column::Status.eq(SubMerchantStatus::Active))
        .one(&state.db)
        .await
        .map_err(|e| {
            warn!(error = %e, "Failed to query sub-merchant");
            AppError::InternalServerError(anyhow::anyhow!("Failed to resolve sub-merchant"))
        })?
        .ok_or_else(|| {
            debug!(
                parent_org_id = %merchant.id,
                sub_merchant_code = %code,
                "Sub-merchant code not found or inactive"
            );
            AppError::ValidationError {
                code: "invalid_sub_merchant_code",
                message: format!("Sub-merchant code '{}' not found or inactive", code),
                param: Some("X-Sub-Merchant-Code".into()),
            }
        })?;

    // Store sub-merchant context for downstream use (e.g., webhook routing)
    request.extensions_mut().insert(SubMerchantContext {
        parent_org_id: merchant.id.clone(),
        sub_merchant_code: code.clone(),
        child_org_id: sub_merchant.child_org_id.clone(),
    });

    // Replace AuthenticatedMerchant.id with the child org ID
    let replaced_merchant = AuthenticatedMerchant {
        id: sub_merchant.child_org_id.clone(),
        ..merchant
    };
    request.extensions_mut().insert(replaced_merchant);

    debug!(
        parent_org_id = %merchant.id,
        sub_merchant_code = %code,
        child_org_id = %sub_merchant.child_org_id,
        "Sub-merchant scope activated"
    );

    Ok(next.run(request).await)
}
