//! Admin Portal authentication middleware.
//!
//! Simple Bearer token auth using ADMIN_TOKEN environment variable.
//! No merchant context, no environment header — admin sees everything.

use axum::{
    extract::Extension,
    http::{header::AUTHORIZATION, Request},
    middleware::Next,
    response::Response,
};
use secrecy::ExposeSecret;

use crate::api::error::AppError;
use crate::AppState;

/// Admin-only auth middleware.
///
/// Validates `Authorization: Bearer {token}` against `config.admin_token`.
/// Returns 403 if ADMIN_TOKEN is not configured (feature disabled).
/// Returns 401 if token is missing or invalid.
pub async fn admin_auth(
    Extension(state): Extension<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Check if admin token is configured
    let expected_token = state.config.admin_token.as_ref().ok_or_else(|| {
        AppError::PermissionDenied(
            "Admin portal is not configured. Set ADMIN_TOKEN env var.".into(),
        )
    })?;

    // Extract Bearer token from Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::AuthError("Missing Authorization header".into()))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        AppError::AuthError("Invalid Authorization format. Use: Bearer <token>".into())
    })?;

    // Constant-time comparison to prevent timing attacks
    if !constant_time_eq(token.as_bytes(), expected_token.expose_secret().as_bytes()) {
        return Err(AppError::AuthError("Invalid admin token".into()));
    }

    Ok(next.run(req).await)
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
