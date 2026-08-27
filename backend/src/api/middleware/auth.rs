use crate::api::error::AppError;
use axum::{
    extract::{Extension, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts, Request},
    middleware::Next,
    response::Response,
};
use tracing::warn; // 引入日志

use crate::config::Environment;
use crate::entity::org_members::MemberRole;
use crate::entity::Environment as EntityEnvironment;
use crate::AppState;

/// Authenticated merchant info extracted from JWT or API Key
///
/// Contains the org ID, user ID, role, and the environment context.
/// - API Key: `sk_live_*` → `Production`, `sk_test_*` → `Sandbox`
/// - JWT (Dashboard): Determined by `X-Environment` header, defaults to `Production`
///
/// # Financial Isolation
/// Environment is **always required** - never optional. This ensures:
/// - No mixing of production (real money) and sandbox (test money) data
/// - All queries are scoped appropriately
/// - Compile-time enforcement of environment context
#[derive(Clone, Debug)]
pub struct AuthenticatedMerchant {
    /// org_id (= merchant_id, backward compat — all 45+ routes use this)
    pub id: String,
    /// user_id (new for Role & Org)
    pub user_id: String,
    /// Role in this org (new for Role & Org, defaults to Owner for API Key auth)
    pub role: MemberRole,
    /// Environment context - always required for financial isolation.
    /// Dashboard users switch via X-Environment header.
    pub environment: EntityEnvironment,
}

/// Check if the authenticated user has one of the required roles.
///
/// Returns `Ok(())` if the role is allowed, or `AppError::PermissionDenied` (403) if not.
///
/// # Usage
/// ```ignore
/// require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;
/// ```
pub fn require_role(
    merchant: &AuthenticatedMerchant,
    allowed: &[MemberRole],
) -> Result<(), AppError> {
    if allowed.contains(&merchant.role) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Insufficient permissions. Required role: {:?}, your role: {:?}",
            allowed, merchant.role
        )))
    }
}

/// Extract from request extensions
/// This expects that `auth_middleware` has already run and inserted the struct into extensions.
impl<S> FromRequestParts<S> for AuthenticatedMerchant
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            parts
                .extensions
                .get::<AuthenticatedMerchant>()
                .cloned()
                .ok_or(AppError::AuthError("Not authenticated".into()))
        })
    }
}

pub async fn auth_middleware(
    Extension(state): Extension<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Get Authorization Header and immediately Clone to String
    // Purpose: Release borrow on req, avoiding lifetime conflicts during async
    let auth_header_value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .map(|v| v.to_string()); // Clone here!

    let Some(auth_value) = auth_header_value else {
        return Err(AppError::AuthError("Missing Authorization header".into()));
    };

    // 2. Unified Auth Logic
    if let Some(token_ref) = auth_value.strip_prefix("Bearer ") {
        let token = token_ref.to_string();

        // Case A: API Key (as Bearer)
        if token.starts_with("sk_") {
            if let Ok((mid, env)) = state.merchant_service.verify_api_key(&token).await {
                // Enforce Environment Match
                validate_environment_match(&state.config.environment, &env)?;

                req.extensions_mut().insert(AuthenticatedMerchant {
                    id: mid.clone(),
                    user_id: mid, // API key: user_id = org_id (no user context)
                    role: MemberRole::Owner, // API key: assume owner
                    environment: env,
                });
                return Ok(next.run(req).await);
            }
        }

        // Case B: JWT (Dashboard)
        if let Ok(auth_info) = state.merchant_service.verify_token(&token).await {
            // Parse environment Header (strict mode: required)
            let env = parse_environment_header(&req)?;

            // Enforce Environment Match (e.g. Dashboard switching to Prod on Sandbox instance)
            validate_environment_match(&state.config.environment, &env)?;

            let role = auth_info.role.parse().unwrap_or(MemberRole::Owner);
            req.extensions_mut().insert(AuthenticatedMerchant {
                id: auth_info.org_id,
                user_id: auth_info.user_id,
                role,
                environment: env,
            });
            return Ok(next.run(req).await);
        } else {
            // If JWT verification failed, check if it was an expired token vs invalid
            warn!("Invalid JWT token provided");
        }
    }
    // Case C: Raw API Key
    else if auth_value.starts_with("sk_") {
        if let Ok((mid, env)) = state.merchant_service.verify_api_key(&auth_value).await {
            // Enforce Environment Match
            validate_environment_match(&state.config.environment, &env)?;

            req.extensions_mut().insert(AuthenticatedMerchant {
                id: mid.clone(),
                user_id: mid,            // API key: user_id = org_id
                role: MemberRole::Owner, // API key: assume owner
                environment: env,
            });
            return Ok(next.run(req).await);
        }
    }

    // Auth failed
    Err(AppError::AuthError("Invalid API Key or Token".into()))
}

/// JWT-only authentication middleware for `/api/internal/*` routes.
///
/// Only accepts JWT Bearer tokens (no API Keys). Requires X-Environment header.
/// Used for dashboard-only endpoints that should never be accessed via API Key.
pub async fn jwt_auth(
    Extension(state): Extension<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header_value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .map(|v| v.to_string());

    let Some(auth_value) = auth_header_value else {
        return Err(AppError::AuthError("Missing Authorization header".into()));
    };

    let Some(token_ref) = auth_value.strip_prefix("Bearer ") else {
        return Err(AppError::AuthError(
            "Authorization header must be Bearer token".into(),
        ));
    };

    let token = token_ref.to_string();

    // Reject API Keys — this middleware is JWT-only
    if token.starts_with("sk_") {
        return Err(AppError::AuthError(
            "API Keys are not accepted on internal endpoints. Use JWT token.".into(),
        ));
    }

    // Verify JWT
    let auth_info = state
        .merchant_service
        .verify_token(&token)
        .await
        .map_err(|_| AppError::AuthError("Invalid or expired JWT token".into()))?;

    // Parse X-Environment header (strict mode)
    let env = parse_environment_header(&req)?;

    // Enforce environment match
    validate_environment_match(&state.config.environment, &env)?;

    let role = auth_info.role.parse().unwrap_or(MemberRole::Owner);
    req.extensions_mut().insert(AuthenticatedMerchant {
        id: auth_info.org_id,
        user_id: auth_info.user_id,
        role,
        environment: env,
    });
    Ok(next.run(req).await)
}

/// Parse the X-Environment header to determine environment context for dashboard
///
/// Header values:
/// - "sandbox" or "test" → `EntityEnvironment::Sandbox`
/// - "production" or "live" → `EntityEnvironment::Production`
/// - Not present or invalid → `EntityEnvironment::Production` (production default for safety)
///
/// # Financial Safety
/// Always returns a concrete Environment - never optional.
/// Defaulting to Production ensures dashboard users see real data by default.
/// Parse the X-Environment header to determine environment context for dashboard
///
/// Header values:
/// - "sandbox" or "test" → `EntityEnvironment::Sandbox`
/// - "production" or "live" → `EntityEnvironment::Production`
///
/// # Strict Mode
/// Returns error if header is missing or invalid.
fn parse_environment_header(
    req: &Request<axum::body::Body>,
) -> Result<EntityEnvironment, AppError> {
    let raw_header = req
        .headers()
        .get("X-Environment")
        .and_then(|h| h.to_str().ok());

    match raw_header {
        Some(v) => match v.to_lowercase().as_str() {
            "sandbox" | "test" => Ok(EntityEnvironment::Sandbox),
            "production" | "live" => Ok(EntityEnvironment::Production),
            unknown => {
                warn!("Invalid X-Environment: {}", unknown);
                Err(AppError::AuthError(format!(
                    "Invalid X-Environment header: {}",
                    unknown
                )))
            }
        },
        None => {
            warn!("AuthMiddleware: Missing X-Environment header");
            Err(AppError::AuthError("Missing X-Environment header".into()))
        }
    }
}

/// Helper to ensure the request's intended environment matches the server's environment
fn validate_environment_match(
    server_env: &Environment,
    request_env: &EntityEnvironment,
) -> Result<(), AppError> {
    // Map entity::Environment to config::Environment for comparison
    let request_config_env = match request_env {
        EntityEnvironment::Production => Environment::Production,
        EntityEnvironment::Sandbox => Environment::Sandbox,
    };

    if server_env != &request_config_env {
        warn!(
            server_env = ?server_env,
            request_env = ?request_env,
            "Environment mismatch: Request targets {:?} but server is {:?}",
            request_env,
            server_env
        );

        return Err(AppError::EnvironmentMismatch {
            expected: server_env.to_string(),
            got: request_env.to_string(),
        });
    }

    Ok(())
}
