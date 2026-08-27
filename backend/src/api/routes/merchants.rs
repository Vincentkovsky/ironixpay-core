//! Merchant API Routes
//!
//! Handles merchant registration, authentication, and API key management.
//! Aligned with docs/system_design.md

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use std::net::SocketAddr;
use validator::Validate;

use crate::api::dtos::auth::{
    ApiKeyListResponse, ApiKeyResponse, ChangePasswordRequest, CreateApiKeyRequest,
    Enable2FARequest, ForgotPasswordRequest, LoginRequest, LoginResponse, MerchantBalanceResponse,
    MerchantResponse, MerchantStatsResponse, RegisterRequest, ResendVerificationRequest,
    ResetPasswordRequest, SuccessResponse, SwitchOrgRequest, SwitchOrgResponse, TotpSetupResponse,
    UpdateProfileRequest, UpdateUserRequest, UpdateWalletAddressRequest, Verify2FARequest,
    VerifyEmailRequest,
};
use crate::api::error::{
    AppError, E_2FA_REQUIRED, E_HUMAN_VERIFICATION_FAILED, E_INVALID_AMOUNT, E_PARAMETER_INVALID,
};
use crate::api::middleware::auth::{require_role, AuthenticatedMerchant};
use crate::api::middleware::rate_limit::extract_client_ip;
use crate::entity::org_members::MemberRole;
use crate::AppState;

/// Public auth routes: register, login, verify-email, resend-verification, verify-2fa.
/// No authentication required. Mounted at `/api/auth`.
pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/verify-email", post(verify_email))
        .route("/resend-verification", post(resend_verification))
        .route("/verify-2fa", post(verify_2fa))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
}

/// Protected merchant routes for the dashboard.
/// JWT auth is applied by the parent router (`/api/internal`).
/// Mounted at `/api/internal/merchants`.
pub fn internal_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_profile).put(update_profile))
        .route("/stats", get(get_stats))
        .route("/wallets/config", post(update_wallet_config))
        .route("/password", put(change_password))
        .route("/api-keys", post(create_api_key).get(get_api_keys))
        .route("/api-keys/:key_id", delete(revoke_api_key))
        .route("/balance", get(get_merchant_balance))
        .route(
            "/withdrawals",
            post(request_withdrawal).get(list_withdrawals),
        )
        .route("/withdrawals/:id", get(get_withdrawal))
        .route("/payouts", get(list_merchant_payouts))
        .route("/payouts/:id", get(get_merchant_payout))
        .route("/payouts/:id/approve", post(approve_payout))
        .route("/payouts/:id/reject", post(reject_payout))
        .route("/withdrawals/:id/approve", post(approve_withdrawal))
        .route("/withdrawals/:id/reject", post(reject_withdrawal))
        .route(
            "/settings/payout",
            get(get_payout_settings).put(update_payout_settings),
        )
        .route("/2fa/setup", post(setup_2fa))
        .route("/2fa/enable", post(enable_2fa))
        .route("/2fa/disable", post(disable_2fa))
        .route("/switch-org", post(switch_org))
        .route("/accept-invite", post(accept_invite))
        .route("/user/me", put(update_user))
        .route(
            "/notifications/pending-count",
            get(get_pending_approval_count),
        )
}

// === Handlers ===

// register handler: remove collection_address
/// POST /v1/merchants/register
async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<MerchantResponse>), AppError> {
    body.validate()?;

    let client_ip =
        extract_client_ip(&headers, connect_info.map(|info| info.0)).ok_or_else(|| {
            AppError::InternalServerError(anyhow::anyhow!("Client IP unavailable for registration"))
        })?;
    state
        .merchant_service
        .check_registration_rate_limit(client_ip)?;

    if let Some(turnstile) = &state.turnstile_service {
        let token = body
            .turnstile_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| AppError::ValidationError {
                code: E_HUMAN_VERIFICATION_FAILED,
                message: "Please complete the human verification challenge.".to_string(),
                param: Some("turnstile_token".to_string()),
            })?;

        match turnstile.verify_registration(token, client_ip).await {
            Ok(()) => {}
            Err(crate::services::turnstile::TurnstileError::Rejected) => {
                return Err(AppError::ValidationError {
                    code: E_HUMAN_VERIFICATION_FAILED,
                    message: "Human verification failed. Please try again.".to_string(),
                    param: Some("turnstile_token".to_string()),
                });
            }
            Err(crate::services::turnstile::TurnstileError::Unavailable) => {
                return Err(AppError::ServiceUnavailable(
                    "Human verification is temporarily unavailable. Please try again later."
                        .to_string(),
                ));
            }
        }
    }

    let req = crate::services::merchant::RegisterRequest {
        name: body.name,
        email: body.email,
        password: body.password,
        invite_token: body.invite_token,
        referral_code: body.referral_code,
    };

    match state.merchant_service.register(req).await {
        Ok(merchant) => {
            let resp = state
                .merchant_service
                .build_merchant_response(merchant)
                .await
                .map_err(AppError::InternalServerError)?;
            Ok((StatusCode::CREATED, Json(resp)))
        }
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/login
async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    body.validate()?;

    match state
        .merchant_service
        .login(&body.email, &body.password)
        .await
    {
        Ok(crate::services::merchant::LoginResponse::Success {
            token,
            merchant_id,
            user_id,
            role,
            org_name,
            expires_at,
        }) => {
            let merchant = state
                .merchant_service
                .get_merchant(&merchant_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Merchant not found after login".to_string()))?;

            let mut resp = state
                .merchant_service
                .build_merchant_response(merchant)
                .await
                .map_err(AppError::InternalServerError)?;

            // Inject logged-in user's personal name
            if let Ok(Some(user)) = crate::entity::users::Entity::find_by_id(&user_id)
                .one(&state.db)
                .await
            {
                resp.user_name = Some(user.name);
                resp.email = user.email;
            }

            Ok(Json(LoginResponse::Success {
                token,
                merchant_id,
                user_id,
                role,
                org_name,
                expires_at,
                merchant: resp,
            }))
        }
        Ok(crate::services::merchant::LoginResponse::Requires2FA {
            temp_token,
            merchant_id,
        }) => Ok(Json(LoginResponse::Requires2FA {
            temp_token,
            merchant_id,
        })),
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/verify-email
///
/// Verifies merchant email and activates account.
/// Also triggers async address initialization for both networks.
async fn verify_email(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    match state.merchant_service.verify_email(&body.token).await {
        Ok(merchant_id) => {
            // Spawn async address initialization for ALL enabled networks
            // This runs in background without blocking the response
            let address_manager = state.address_manager.clone();
            let mid = merchant_id.clone();
            let environment = state.config.environment.to_entity_environment();
            let networks = state.enabled_networks.clone();
            tokio::spawn(async move {
                for network in networks {
                    if let Err(e) = address_manager
                        .initialize_merchant_addresses(&mid, network.clone(), environment.clone())
                        .await
                    {
                        tracing::warn!(
                            merchant_id = %mid,
                            network = ?network,
                            error = %e,
                            "Failed to initialize addresses (will retry on first session)"
                        );
                    }
                }

                tracing::info!(merchant_id = %mid, "Address initialization completed for all networks");
            });

            Ok(Json(SuccessResponse {
                success: true,
                message: "Email verified successfully. You can now log in.".to_string(),
            }))
        }
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/resend-verification
async fn resend_verification(
    State(state): State<AppState>,
    Json(body): Json<ResendVerificationRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    match state
        .merchant_service
        .resend_verification_email(&body.email)
        .await
    {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "Verification email sent. Please check your inbox.".to_string(),
        })),
        Err(e) => {
            tracing::warn!(email = %body.email, error = %e, "Resend verification failed");
            // For security reasons (preventing email enumeration), we still return a success message
            // even if the email wasn't found or sending failed.
            Ok(Json(SuccessResponse {
                success: true,
                message: "If this email is registered, a verification link will be sent."
                    .to_string(),
            }))
        }
    }
}

/// POST /v1/merchants/verify-2fa
async fn verify_2fa(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<Verify2FARequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let client_ip =
        extract_client_ip(&headers, connect_info.map(|info| info.0)).ok_or_else(|| {
            AppError::InternalServerError(anyhow::anyhow!(
                "Client IP unavailable for 2FA verification"
            ))
        })?;

    match state
        .merchant_service
        .verify_totp_login(&body.temp_token, &body.code, client_ip)
        .await
    {
        Ok(crate::services::merchant::LoginResponse::Success {
            token,
            merchant_id,
            user_id,
            role,
            org_name,
            expires_at,
        }) => {
            let merchant = state
                .merchant_service
                .get_merchant(&merchant_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Merchant not found after login".to_string()))?;

            let mut resp = state
                .merchant_service
                .build_merchant_response(merchant)
                .await
                .map_err(AppError::InternalServerError)?;

            // Inject logged-in user's personal name
            if let Ok(Some(user)) = crate::entity::users::Entity::find_by_id(&user_id)
                .one(&state.db)
                .await
            {
                resp.user_name = Some(user.name);
                resp.email = user.email;
            }

            Ok(Json(LoginResponse::Success {
                token,
                merchant_id,
                user_id,
                role,
                org_name,
                expires_at,
                merchant: resp,
            }))
        }
        Ok(crate::services::merchant::LoginResponse::Requires2FA { .. }) => Err(
            AppError::InternalServerError(anyhow::anyhow!("Verify 2FA returned Requires2FA")),
        ),
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/forgot-password
///
/// Sends a password reset email. Always returns success to prevent email enumeration.
async fn forgot_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<ForgotPasswordRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    if let Some(turnstile) = &state.turnstile_service {
        let client_ip =
            extract_client_ip(&headers, connect_info.map(|info| info.0)).ok_or_else(|| {
                AppError::InternalServerError(anyhow::anyhow!(
                    "Client IP unavailable for password reset"
                ))
            })?;
        let token = body
            .turnstile_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| AppError::ValidationError {
                code: E_HUMAN_VERIFICATION_FAILED,
                message: "Please complete the human verification challenge.".to_string(),
                param: Some("turnstile_token".to_string()),
            })?;

        match turnstile.verify_forgot_password(token, client_ip).await {
            Ok(()) => {}
            Err(crate::services::turnstile::TurnstileError::Rejected) => {
                return Err(AppError::ValidationError {
                    code: E_HUMAN_VERIFICATION_FAILED,
                    message: "Human verification failed. Please try again.".to_string(),
                    param: Some("turnstile_token".to_string()),
                });
            }
            Err(crate::services::turnstile::TurnstileError::Unavailable) => {
                return Err(AppError::ServiceUnavailable(
                    "Human verification is temporarily unavailable. Please try again later."
                        .to_string(),
                ));
            }
        }
    }

    match state
        .merchant_service
        .send_password_reset_email(&body.email)
        .await
    {
        Ok(()) => {}
        Err(e) => {
            // Log but don't expose to user (prevent email enumeration)
            tracing::warn!(email = %body.email, error = %e, "Password reset request failed");
        }
    }

    // Always return success to prevent email enumeration
    Ok(Json(SuccessResponse {
        success: true,
        message: "If this email is registered, a password reset link will be sent.".to_string(),
    }))
}

/// POST /v1/merchants/reset-password
///
/// Resets password using a valid reset token from email.
async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    match state
        .merchant_service
        .reset_password_with_token(&body.token, &body.new_password)
        .await
    {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "Password has been reset successfully. You can now log in.".to_string(),
        })),
        Err(e) => Err(e.into()),
    }
}

/// GET /api/internal/merchants/me
///
/// Returns merchant profile with the authenticated user's personal name.
async fn get_profile(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<MerchantResponse>, AppError> {
    match state.merchant_service.get_merchant(&merchant.id).await {
        Ok(Some(m)) => {
            let mut resp = state
                .merchant_service
                .build_merchant_response(m)
                .await
                .map_err(AppError::InternalServerError)?;

            // Inject authenticated user's info (name + email)
            if let Ok(Some(user)) = crate::entity::users::Entity::find_by_id(&merchant.user_id)
                .one(&state.db)
                .await
            {
                resp.user_name = Some(user.name);
                resp.email = user.email;
            }

            Ok(Json(resp))
        }
        Ok(None) => Err(AppError::NotFound("Merchant not found".to_string())),
        Err(e) => Err(AppError::InternalServerError(e)),
    }
}

/// PUT /api/internal/merchants/user/me
///
/// Update the current user's personal name.
async fn update_user(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    let db = &state.db;
    let user = crate::entity::users::Entity::find_by_id(&merchant.user_id)
        .one(db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    use sea_orm::{ActiveModelTrait, Set};
    let mut user_am: crate::entity::users::ActiveModel = user.into();
    user_am.name = Set(body.name.trim().to_string());
    // NOTE: NOT bumping token_version — name is not in the JWT,
    // so no need to force re-login for a display-name change.
    user_am
        .update(db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    tracing::info!(user_id = %merchant.user_id, "User personal name updated");

    Ok(Json(SuccessResponse {
        success: true,
        message: "Profile updated.".to_string(),
    }))
}

/// PUT /api/internal/merchants/me
///
/// Update the organization profile (Owner/Admin only).
async fn update_profile(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<MerchantResponse>, AppError> {
    body.validate()?;

    // Role guard: only Owner and Admin can update org profile
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    // Update identity (name)
    match state
        .merchant_service
        .update_merchant(&merchant.id, Some(body.name))
        .await
    {
        Ok(m) => {
            let resp = state
                .merchant_service
                .build_merchant_response(m)
                .await
                .map_err(AppError::InternalServerError)?;
            Ok(Json(resp))
        }
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/wallets/config
///
/// Updates the collection address for a specific environment and network.
/// Requires 2FA verification.
async fn update_wallet_config(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<UpdateWalletAddressRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    // Role guard: only Owner and Admin can update wallet config
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let code = body.code.ok_or_else(|| AppError::ValidationError {
        code: E_2FA_REQUIRED,
        message: "2FA code is required to update wallet configuration".into(),
        param: Some("code".into()),
    })?;

    state
        .merchant_service
        .update_collection_address_with_2fa(
            &merchant.id,
            body.network,
            merchant.environment,
            &body.collection_address,
            &code,
        )
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Wallet configuration updated successfully".to_string(),
    }))
}

/// PUT /v1/merchants/password
async fn change_password(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    match state
        .merchant_service
        .change_password(
            &merchant.user_id,
            &body.old_password,
            &body.new_password,
            body.code.as_deref(),
        )
        .await
    {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "Password changed successfully".to_string(),
        })),
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/api-keys
async fn create_api_key(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyResponse>), AppError> {
    // Role guard: Owner, Admin, Developer can manage API keys
    require_role(
        &merchant,
        &[MemberRole::Owner, MemberRole::Admin, MemberRole::Developer],
    )?;

    let is_test = matches!(
        state.config.environment,
        crate::config::Environment::Sandbox
    );
    match state
        .merchant_service
        .create_api_key(&merchant.id, body.name.clone(), is_test)
        .await
    {
        Ok(key) => Ok((
            StatusCode::CREATED,
            Json(ApiKeyResponse {
                id: key.id,
                key: key.key,
                prefix: key.prefix,
                name: key.name,
                created_at: key.created_at,
                last_used_at: key.last_used_at,
            }),
        )),
        Err(e) => Err(e.into()),
    }
}

/// DELETE /v1/merchants/api-keys/:key_id
async fn revoke_api_key(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(key_id): Path<String>,
) -> Result<StatusCode, AppError> {
    // Role guard: Owner, Admin, Developer can manage API keys
    require_role(
        &merchant,
        &[MemberRole::Owner, MemberRole::Admin, MemberRole::Developer],
    )?;

    match state
        .merchant_service
        .revoke_api_key(&key_id, &merchant.id)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(e.into()),
    }
}

/// GET /v1/merchants/balance
async fn get_merchant_balance(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<MerchantBalanceResponse>, AppError> {
    // Role guard: Owner, Admin, Finance, Viewer can view balance (no Developer)
    require_role(
        &merchant,
        &[
            MemberRole::Owner,
            MemberRole::Admin,
            MemberRole::Finance,
            MemberRole::Viewer,
        ],
    )?;

    match state
        .merchant_service
        .get_merchant_balance(&merchant.id)
        .await
    {
        Ok(balance) => {
            let balance_str = crate::api::dtos::checkout::from_micro(balance, "USDT");
            Ok(Json(MerchantBalanceResponse {
                balance: balance_str,
                unit: "USDT+USDC".to_string(),
            }))
        }
        Err(e) => Err(AppError::InternalServerError(e)),
    }
}

/// GET /v1/merchants/stats
async fn get_stats(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<MerchantStatsResponse>, AppError> {
    // Dashboard stats always aggregate across parent + all sub-merchants
    let (merchant_ids, _) = state
        .sub_merchant_service
        .resolve_merchant_ids(&merchant.id, true, None)
        .await?;

    state
        .merchant_service
        // Pass None to aggregate across all enabled networks
        .get_stats(&merchant_ids, None, merchant.environment)
        .await
        .map(Json)
        .map_err(AppError::InternalServerError)
}

/// GET /v1/merchants/api-keys
async fn get_api_keys(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<ApiKeyListResponse>, AppError> {
    // Role guard: Owner, Admin, Developer can view API keys
    require_role(
        &merchant,
        &[MemberRole::Owner, MemberRole::Admin, MemberRole::Developer],
    )?;

    match state.merchant_service.get_api_keys(&merchant.id).await {
        Ok(keys) => {
            let key_responses = keys
                .into_iter()
                .map(|k| ApiKeyResponse {
                    id: k.id,
                    key: format!("{}...", k.key_prefix), // Don't return full key/hash
                    prefix: k.key_prefix.clone(),
                    name: k.name,
                    created_at: k.created_at.to_rfc3339(),
                    last_used_at: k.last_used_at.map(|dt| dt.to_rfc3339()),
                })
                .collect();

            Ok(Json(ApiKeyListResponse {
                keys: key_responses,
            }))
        }
        Err(e) => Err(AppError::InternalServerError(e)),
    }
}

/// POST /v1/merchants/2fa/setup
async fn setup_2fa(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<TotpSetupResponse>, AppError> {
    tracing::info!("Received setup_2fa request for user: {}", merchant.user_id);
    match state.merchant_service.setup_totp(&merchant.user_id).await {
        Ok(res) => Ok(Json(TotpSetupResponse {
            secret: res.secret,
            qr_code_uri: res.qr_code_uri,
            backup_codes: res.backup_codes,
        })),
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/2fa/enable
async fn enable_2fa(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<Enable2FARequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    match state
        .merchant_service
        .enable_totp(&merchant.user_id, &body.code)
        .await
    {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "2FA enabled successfully".to_string(),
        })),
        Err(e) => Err(e.into()),
    }
}

/// POST /v1/merchants/2fa/disable
async fn disable_2fa(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<Enable2FARequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    match state
        .merchant_service
        .disable_totp(&merchant.user_id, &body.code)
        .await
    {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "2FA disabled successfully".to_string(),
        })),
        Err(e) => Err(e.into()),
    }
}

// === Withdrawal Handlers ===

/// POST /v1/merchants/withdrawals
///
/// Request a withdrawal from merchant balance.
/// Amount is a human-readable decimal string (e.g., "10.50"). Currency: USDT or USDC.
async fn request_withdrawal(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<crate::api::dtos::withdrawals::WithdrawalRequest>,
) -> Result<
    (
        StatusCode,
        Json<crate::api::dtos::withdrawals::WithdrawalResponse>,
    ),
    AppError,
> {
    body.validate()?;

    let amount_decimal: rust_decimal::Decimal =
        body.amount.parse().map_err(|_| AppError::ValidationError {
            code: E_INVALID_AMOUNT,
            message: "Invalid amount: must be a numeric decimal string (e.g., \"10.50\")".into(),
            param: Some("amount".into()),
        })?;

    if amount_decimal <= rust_decimal::Decimal::ZERO {
        return Err(AppError::ValidationError {
            code: E_INVALID_AMOUNT,
            message: "Amount must be positive".into(),
            param: Some("amount".into()),
        });
    }

    // Determine currency early for to_micro conversion
    let currency = body.currency.as_deref().unwrap_or("USDT");
    let amount: i64 =
        crate::api::dtos::checkout::to_micro(amount_decimal, currency).ok_or_else(|| {
            AppError::ValidationError {
                code: E_INVALID_AMOUNT,
                message: "Amount is too large or has too many decimal places".into(),
                param: Some("amount".into()),
            }
        })?;

    // Role guard: only Owner, Admin, Finance can request withdrawals
    require_role(
        &merchant,
        &[MemberRole::Owner, MemberRole::Admin, MemberRole::Finance],
    )?;

    // ── M12: Mandatory 2FA for withdrawals ──
    // Withdrawals are the sole fund-exit path. Require 2FA unconditionally.
    // 1) User must have 2FA enabled (auth fields now in users table)
    // 2) A valid TOTP code must be provided
    let user = crate::entity::users::Entity::find_by_id(&merchant.user_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    if !user.is_totp_enabled {
        return Err(AppError::ValidationError {
            code: E_2FA_REQUIRED,
            message:
                "Two-Factor Authentication (2FA) must be enabled before you can withdraw funds. \
             Please enable 2FA in your account settings."
                    .into(),
            param: Some("totp_code".into()),
        });
    }

    let totp_code = body
        .totp_code
        .as_deref()
        .ok_or_else(|| AppError::ValidationError {
            code: E_2FA_REQUIRED,
            message: "TOTP code is required for withdrawals".into(),
            param: Some("totp_code".into()),
        })?;

    state
        .merchant_service
        .verify_totp_action(&merchant.id, totp_code)
        .await?;
    // ── End M12 ──

    let network_enum = {
        let network_str = body.network.as_deref().unwrap_or("TRON");
        crate::entity::Network::from_str_lenient(network_str).ok_or_else(|| {
            AppError::ValidationError {
                code: "INVALID_NETWORK",
                message: format!("Unsupported network: '{network_str}'. Supported: TRON, BSC"),
                param: Some("network".into()),
            }
        })?
    };

    // Validate and default currency
    let currency = body.currency.as_deref().unwrap_or("USDT");
    if currency != "USDT" && currency != "USDC" {
        return Err(AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: format!(
                "Unsupported currency: '{}'. Supported: USDT, USDC",
                currency
            ),
            param: Some("currency".into()),
        });
    }

    // USDC not available on TRON
    if currency == "USDC" {
        let chain_config = network_enum.chain_config(&merchant.environment);
        if chain_config.usdc_contract.is_none() {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: format!("USDC is not supported on {} network", network_enum.as_str()),
                param: Some("currency".into()),
            });
        }
    }

    let withdrawal = state
        .payout_service
        .request_withdrawal(
            &merchant.id,
            amount,
            merchant.environment,
            network_enum,
            currency,
            Some(&merchant.user_id),
            false, // Dashboard withdrawals are subject to risk control
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(crate::api::dtos::withdrawals::WithdrawalResponse::from(
            withdrawal,
        )),
    ))
}

/// GET /v1/merchants/withdrawals
///
/// Returns paginated withdrawal history for the authenticated merchant.
async fn list_withdrawals(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(pagination): Query<crate::api::dtos::pagination::PaginationRequest>,
) -> Result<
    Json<
        crate::api::dtos::pagination::PaginatedResponse<
            crate::api::dtos::withdrawals::WithdrawalResponse,
        >,
    >,
    AppError,
> {
    pagination.validate()?;

    // Role guard: Owner, Admin, Finance, Viewer can list withdrawals (no Developer)
    require_role(
        &merchant,
        &[
            MemberRole::Owner,
            MemberRole::Admin,
            MemberRole::Finance,
            MemberRole::Viewer,
        ],
    )?;

    let paginator = crate::entity::withdrawals::Entity::find()
        .filter(crate::entity::withdrawals::Column::MerchantId.eq(&merchant.id))
        .filter(crate::entity::withdrawals::Column::Environment.eq(merchant.environment))
        .order_by_desc(crate::entity::withdrawals::Column::CreatedAt)
        .paginate(&state.db, pagination.page_size);

    let total = paginator
        .num_items()
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;
    let data = paginator
        .fetch_page(pagination.page - 1)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    let data: Vec<crate::api::dtos::withdrawals::WithdrawalResponse> =
        data.into_iter().map(Into::into).collect();

    Ok(Json(crate::api::dtos::pagination::PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /v1/merchants/withdrawals/:id
///
/// Returns a single withdrawal by ID (must belong to the authenticated merchant).
async fn get_withdrawal(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
) -> Result<Json<crate::api::dtos::withdrawals::WithdrawalResponse>, AppError> {
    let withdrawal = crate::entity::withdrawals::Entity::find_by_id(&id)
        .filter(crate::entity::withdrawals::Column::MerchantId.eq(&merchant.id))
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Withdrawal not found".into()))?;

    Ok(Json(
        crate::api::dtos::withdrawals::WithdrawalResponse::from(withdrawal),
    ))
}

// === Payout Handlers (read-only, payouts are created via Public API) ===

/// Optional filter for merchant payout list
#[derive(Debug, serde::Deserialize, Default)]
struct PayoutListFilter {
    #[serde(default)]
    status: Option<String>,
}

/// GET /api/internal/merchants/payouts
///
/// Returns paginated payout history for the authenticated merchant.
/// Payouts are created via the Public API (API Key auth).
/// This endpoint is read-only for Dashboard display.
async fn list_merchant_payouts(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(pagination): Query<crate::api::dtos::pagination::PaginationRequest>,
    Query(filter): Query<PayoutListFilter>,
    Query(sm_filter): Query<crate::api::routes::resolution::SubMerchantFilter>,
) -> Result<
    Json<
        crate::api::dtos::pagination::PaginatedResponse<crate::api::dtos::payouts::PayoutResponse>,
    >,
    AppError,
> {
    pagination.validate()?;

    // Role guard: Owner, Admin, Finance, Viewer can list payouts (no Developer)
    require_role(
        &merchant,
        &[
            MemberRole::Owner,
            MemberRole::Admin,
            MemberRole::Finance,
            MemberRole::Viewer,
        ],
    )?;

    // Resolve merchant IDs (parent + sub-merchants based on filter)
    let (merchant_ids, code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            sm_filter.include_sub_merchants,
            sm_filter.sub_merchant_code.as_deref(),
        )
        .await?;

    let mut query = crate::entity::payouts::Entity::find()
        .filter(crate::entity::payouts::Column::MerchantId.is_in(&merchant_ids))
        .filter(crate::entity::payouts::Column::Environment.eq(merchant.environment));

    // Status filter
    if let Some(ref status) = filter.status {
        query = query.filter(crate::entity::payouts::Column::Status.eq(status.as_str()));
    }

    // Search filter
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            sea_orm::Condition::any()
                .add(crate::entity::payouts::Column::Id.contains(search))
                .add(crate::entity::payouts::Column::ToAddress.contains(search))
                .add(crate::entity::payouts::Column::TxHash.contains(search)),
        );
    }

    let paginator = query
        .order_by_desc(crate::entity::payouts::Column::CreatedAt)
        .paginate(&state.db, pagination.page_size);

    let total = paginator
        .num_items()
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;
    let data = paginator
        .fetch_page(pagination.page - 1)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    let livemode =
        crate::entity::Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let items: Vec<crate::api::dtos::payouts::PayoutResponse> = data
        .into_iter()
        .map(|p| {
            let sm_code = code_map.get(&p.merchant_id).cloned();
            crate::api::dtos::payouts::PayoutResponse::from_model(p, livemode, sm_code)
        })
        .collect();

    Ok(Json(crate::api::dtos::pagination::PaginatedResponse::new(
        items,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/internal/merchants/payouts/:id
///
/// Returns a single payout by ID (must belong to the authenticated merchant).
async fn get_merchant_payout(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<String>,
) -> Result<Json<crate::api::dtos::payouts::PayoutResponse>, AppError> {
    let payout = crate::entity::payouts::Entity::find_by_id(&id)
        .filter(crate::entity::payouts::Column::MerchantId.eq(&merchant.id))
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Payout not found".into()))?;

    let livemode =
        crate::entity::Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let mut resp = crate::api::dtos::payouts::PayoutResponse::from_model(payout, livemode, None);

    // Resolve reviewed_by user_id → display name
    if let Some(ref uid) = resp.reviewed_by {
        if let Ok(Some(user)) = crate::entity::users::Entity::find_by_id(uid)
            .one(&state.db)
            .await
        {
            resp.reviewed_by = Some(user.name);
        }
    }

    Ok(Json(resp))
}

/// POST /api/internal/merchants/switch-org
///
/// Switch the current user's active organization context.
/// Verifies the user has an active membership in the target org,
/// then issues a new JWT scoped to that org.
async fn switch_org(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<SwitchOrgRequest>,
) -> Result<Json<SwitchOrgResponse>, AppError> {
    body.validate()?;

    let target_org_id = &body.org_id;

    // Verify user has an active membership in the target org
    let membership = crate::entity::org_members::Entity::find()
        .filter(crate::entity::org_members::Column::OrgId.eq(target_org_id))
        .filter(crate::entity::org_members::Column::UserId.eq(&merchant.user_id))
        .filter(crate::entity::org_members::Column::Status.eq("active"))
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| {
            AppError::PermissionDenied("You are not an active member of this organization".into())
        })?;

    // Fetch the target org
    let org = crate::entity::merchants::Entity::find_by_id(target_org_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Organization not found".into()))?;

    // Fetch the current user
    let user = crate::entity::users::Entity::find_by_id(&merchant.user_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Issue new JWT for the target org
    // Use SeaORM's string_value representation (lowercase) for role
    let role_str = match membership.role {
        MemberRole::Owner => "owner",
        MemberRole::Admin => "admin",
        MemberRole::Developer => "developer",
        MemberRole::Finance => "finance",
        MemberRole::Viewer => "viewer",
    };
    let login_response = state
        .merchant_service
        .issue_jwt_token(&user, &org.id, &org.name, role_str)
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    // Extract fields from LoginResponse::Success
    match login_response {
        crate::services::merchant::LoginResponse::Success {
            token,
            merchant_id,
            user_id,
            role,
            org_name,
            expires_at,
        } => Ok(Json(SwitchOrgResponse {
            token,
            merchant_id,
            user_id,
            role,
            org_name,
            expires_at,
        })),
        _ => Err(AppError::InternalServerError(anyhow::anyhow!(
            "Unexpected login response from issue_jwt_token"
        ))),
    }
}

/// POST /api/internal/merchants/accept-invite
///
/// Accept a team invitation. The user must be logged in and their email
/// must match the invitation's invited_email (anti-hijack check).
async fn accept_invite(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<crate::api::dtos::team::AcceptInviteRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    body.validate()?;

    // Decode and verify the invite JWT
    let claims = state
        .merchant_service
        .verify_invite_token(&body.invite_token)
        .map_err(|e| AppError::AuthError(format!("Invalid or expired invite token: {}", e)))?;

    // Fetch the current user's email
    let user = crate::entity::users::Entity::find_by_id(&merchant.user_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Anti-hijack: user's email must match the invitation
    if user.email.to_lowercase() != claims.email.to_lowercase() {
        return Err(AppError::PermissionDenied(
            "Your email does not match this invitation.".into(),
        ));
    }

    // Find the org_member record
    let member = crate::entity::org_members::Entity::find_by_id(&claims.sub)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Invitation not found or already revoked.".into()))?;

    // Must be still pending
    if member.status != crate::entity::org_members::MemberStatus::Pending {
        return Err(AppError::ValidationError {
            code: "ALREADY_ACCEPTED",
            message: "This invitation has already been accepted.".into(),
            param: None,
        });
    }

    // UNIQUE(org_id, user_id) constraint: check user isn't already a member
    let already_member = crate::entity::org_members::Entity::find()
        .filter(crate::entity::org_members::Column::OrgId.eq(&claims.org_id))
        .filter(crate::entity::org_members::Column::UserId.eq(Some(merchant.user_id.clone())))
        .filter(
            crate::entity::org_members::Column::Status
                .eq(crate::entity::org_members::MemberStatus::Active),
        )
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    if already_member.is_some() {
        return Err(AppError::ValidationError {
            code: "ALREADY_MEMBER",
            message: "You are already an active member of this organization.".into(),
            param: None,
        });
    }

    // Activate the membership
    use sea_orm::{ActiveModelTrait, Set};
    let mut active_model: crate::entity::org_members::ActiveModel = member.into();
    active_model.user_id = Set(Some(merchant.user_id.clone()));
    active_model.status = Set(crate::entity::org_members::MemberStatus::Active);
    active_model.accepted_at = Set(Some(chrono::Utc::now().into()));
    active_model
        .update(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    // Invalidate current JWT so next login binds to the newly joined org
    if let Some(user) = crate::entity::users::Entity::find_by_id(&merchant.user_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
    {
        let mut user_am: crate::entity::users::ActiveModel = user.into();
        user_am.token_version = Set(user_am.token_version.unwrap() + 1);
        user_am
            .update(&state.db)
            .await
            .map_err(|e| AppError::InternalServerError(e.into()))?;
    }

    tracing::info!(
        user_id = %merchant.user_id,
        org_id = %claims.org_id,
        member_id = %claims.sub,
        "Team invitation accepted (token invalidated)"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: "Invitation accepted. Please log in again to access the new organization."
            .to_string(),
    }))
}

// =========================================================================
// Helpers
// =========================================================================

/// Convert MemberRole to lowercase string for approver_roles matching.
/// Explicit match avoids fragile Debug formatting.
fn member_role_as_str(role: &MemberRole) -> &'static str {
    match role {
        MemberRole::Owner => "owner",
        MemberRole::Admin => "admin",
        MemberRole::Developer => "developer",
        MemberRole::Finance => "finance",
        MemberRole::Viewer => "viewer",
    }
}

// =========================================================================
// Payout Settings & Approval Endpoints
// =========================================================================

/// GET /api/internal/merchants/settings/payout
async fn get_payout_settings(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<crate::api::dtos::payout_settings::PayoutSettingsResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let settings = state.payout_service.get_payout_settings(&merchant.id).await;

    Ok(Json(settings.into()))
}

/// PUT /api/internal/merchants/settings/payout
async fn update_payout_settings(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<crate::api::dtos::payout_settings::UpdatePayoutSettingsRequest>,
) -> Result<Json<crate::api::dtos::payout_settings::PayoutSettingsResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner])?;

    // Validate approver_roles if provided
    if let Some(ref roles) = body.approver_roles {
        let valid_roles = ["owner", "admin", "finance", "developer", "viewer"];
        for role in roles {
            if !valid_roles.contains(&role.as_str()) {
                return Err(AppError::ValidationError {
                    code: E_PARAMETER_INVALID,
                    message: format!("Invalid approver role: '{}'", role),
                    param: Some("approver_roles".into()),
                });
            }
        }
        // Owner must always be in the list
        if !roles.iter().any(|r| r == "owner") {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: "'owner' must always be included in approver_roles".into(),
                param: Some("approver_roles".into()),
            });
        }
    }

    // Parse threshold from decimal string to microunits
    let threshold_micro = if let Some(ref threshold_str) = body.approval_threshold {
        let decimal: rust_decimal::Decimal =
            threshold_str
                .parse()
                .map_err(|_| AppError::ValidationError {
                    code: E_PARAMETER_INVALID,
                    message: "Invalid approval_threshold: must be a decimal string".into(),
                    param: Some("approval_threshold".into()),
                })?;
        if decimal < rust_decimal::Decimal::NEGATIVE_ONE {
            return Err(AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message:
                    "approval_threshold must be >= -1 (-1 = disabled, 0 = all, >0 = threshold)"
                        .into(),
                param: Some("approval_threshold".into()),
            });
        }
        Some(
            crate::api::dtos::checkout::to_micro(decimal, "USDT").ok_or_else(|| {
                AppError::ValidationError {
                    code: E_PARAMETER_INVALID,
                    message: "approval_threshold value is too large".into(),
                    param: Some("approval_threshold".into()),
                }
            })?,
        )
    } else {
        None
    };

    let auto_withdraw_threshold_micro =
        if let Some(ref threshold_str) = body.auto_withdraw_threshold {
            let decimal: rust_decimal::Decimal =
                threshold_str
                    .parse()
                    .map_err(|_| AppError::ValidationError {
                        code: E_PARAMETER_INVALID,
                        message: "Invalid auto_withdraw_threshold".into(),
                        param: Some("auto_withdraw_threshold".into()),
                    })?;
            if decimal < rust_decimal::Decimal::ZERO {
                return Err(AppError::ValidationError {
                    code: E_PARAMETER_INVALID,
                    message: "auto_withdraw_threshold must be >= 0".into(),
                    param: Some("auto_withdraw_threshold".into()),
                });
            }
            Some(
                crate::api::dtos::checkout::to_micro(decimal, "USDT").ok_or_else(|| {
                    AppError::ValidationError {
                        code: E_PARAMETER_INVALID,
                        message: "auto_withdraw_threshold too large".into(),
                        param: Some("auto_withdraw_threshold".into()),
                    }
                })?,
            )
        } else {
            None
        };

    let approver_roles_json = body.approver_roles.map(|roles| {
        serde_json::Value::Array(roles.into_iter().map(serde_json::Value::String).collect())
    });

    let settings = state
        .payout_service
        .update_payout_settings(
            &merchant.id,
            body.require_new_address_approval,
            threshold_micro,
            approver_roles_json,
            body.auto_withdraw_enabled,
            auto_withdraw_threshold_micro,
            body.auto_withdraw_network,
            body.auto_withdraw_currency,
        )
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(settings.into()))
}

/// POST /api/internal/merchants/payouts/:id/approve
async fn approve_payout(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(payout_id): Path<String>,
    Json(body): Json<crate::api::dtos::payout_settings::ApproveRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    // 1. Check approver role
    let settings = state.payout_service.get_payout_settings(&merchant.id).await;

    let role_str = member_role_as_str(&merchant.role);
    if !settings.is_approver_role(role_str) {
        return Err(AppError::PermissionDenied(
            "Your role is not authorized to approve payouts".into(),
        ));
    }

    // NOTE: No self-approval prevention for API payouts — they are initiated via
    // API Key (no user identity), so there is no "self" to check against.

    // 2. Verify TOTP
    state
        .merchant_service
        .verify_totp_action(&merchant.id, &body.totp_code)
        .await?;

    // 3. Approve
    state
        .payout_service
        .approve_payout(&payout_id, &merchant.id, &merchant.user_id)
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Payout approved".to_string(),
    }))
}

/// POST /api/internal/merchants/payouts/:id/reject
async fn reject_payout(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(payout_id): Path<String>,
    Json(body): Json<crate::api::dtos::payout_settings::RejectRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    // Verify TOTP
    state
        .merchant_service
        .verify_totp_action(&merchant.id, &body.totp_code)
        .await?;

    state
        .payout_service
        .reject_payout(
            &payout_id,
            &merchant.id,
            &merchant.user_id,
            body.reason.as_deref(),
        )
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Payout rejected and refunded".to_string(),
    }))
}

/// POST /api/internal/merchants/withdrawals/:id/approve
async fn approve_withdrawal(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(withdrawal_id): Path<String>,
    Json(body): Json<crate::api::dtos::payout_settings::ApproveRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    // 1. Check approver role
    let settings = state.payout_service.get_payout_settings(&merchant.id).await;

    let role_str = member_role_as_str(&merchant.role);
    if !settings.is_approver_role(role_str) {
        return Err(AppError::PermissionDenied(
            "Your role is not authorized to approve withdrawals".into(),
        ));
    }

    // 2. Self-approval prevention: check if the approver is the initiator
    let wd = crate::entity::withdrawals::Entity::find_by_id(&withdrawal_id)
        .filter(crate::entity::withdrawals::Column::MerchantId.eq(&merchant.id))
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Withdrawal not found".into()))?;
    if wd.requested_by.as_deref() == Some(&merchant.user_id) {
        return Err(AppError::PermissionDenied(
            "You cannot approve your own withdrawal request".into(),
        ));
    }

    // 3. Verify TOTP
    state
        .merchant_service
        .verify_totp_action(&merchant.id, &body.totp_code)
        .await?;

    // 4. Approve
    state
        .payout_service
        .approve_withdrawal(&withdrawal_id, &merchant.id, &merchant.user_id)
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Withdrawal approved".to_string(),
    }))
}

/// POST /api/internal/merchants/withdrawals/:id/reject
async fn reject_withdrawal(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(withdrawal_id): Path<String>,
    Json(body): Json<crate::api::dtos::payout_settings::RejectRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    // Verify TOTP
    state
        .merchant_service
        .verify_totp_action(&merchant.id, &body.totp_code)
        .await?;

    state
        .payout_service
        .reject_withdrawal(
            &withdrawal_id,
            &merchant.id,
            &merchant.user_id,
            body.reason.as_deref(),
        )
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Withdrawal rejected and refunded".to_string(),
    }))
}

/// GET /api/internal/merchants/notifications/pending-count
///
/// Returns the count of PendingApproval withdrawals + payouts for the
/// authenticated merchant in the current environment.
async fn get_pending_approval_count(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::entity::{payouts, withdrawals};

    let env = state.config.environment.to_entity_environment();

    let withdrawal_count = withdrawals::Entity::find()
        .filter(withdrawals::Column::MerchantId.eq(&merchant.id))
        .filter(withdrawals::Column::Environment.eq(env.clone()))
        .filter(withdrawals::Column::Status.eq(withdrawals::WithdrawalStatus::PendingApproval))
        .count(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))? as i64;

    let payout_count = payouts::Entity::find()
        .filter(payouts::Column::MerchantId.eq(&merchant.id))
        .filter(payouts::Column::Environment.eq(env))
        .filter(payouts::Column::Status.eq(payouts::PayoutStatus::PendingApproval))
        .count(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))? as i64;

    Ok(Json(serde_json::json!({
        "pending_approvals": withdrawal_count + payout_count
    })))
}
