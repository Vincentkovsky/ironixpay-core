//! Merchant Service Error Types
//!
//! Provides typed errors so the route layer can map each variant to the
//! correct HTTP status code and machine-readable error code via
//! `From<MerchantError> for AppError`.

use thiserror::Error;

/// Merchant service specific errors (for API-facing methods only).
///
/// Internal-only helpers (e.g. `verify_token`, `ensure_merchant_shadow`)
/// continue to use `anyhow::Result`.
#[derive(Debug, Error)]
pub enum MerchantError {
    // ── Auth ──
    /// Generic "invalid email or password" — HTTP 401
    #[error("Invalid email or password")]
    InvalidCredentials,

    /// Account has been suspended by admin — HTTP 403
    #[error("Account suspended")]
    AccountSuspended,

    /// Must verify email before login — HTTP 400
    #[error("Please verify your email address first")]
    EmailNotVerified,

    /// Duplicate registration — HTTP 409
    #[error("Email already registered")]
    EmailAlreadyRegistered,

    /// Disposable or explicitly denied email domain — HTTP 400
    #[error("Disposable email addresses are not allowed")]
    DisposableEmailNotAllowed,

    /// Public registration velocity limit exceeded — HTTP 429
    #[error("Too many registration attempts, try again later")]
    RegistrationRateLimited,

    // ── Token ──
    /// JWT decode failure or wrong purpose — HTTP 401
    #[error("Invalid or expired token")]
    InvalidToken,

    /// Reset token already consumed (token_version mismatch) — HTTP 401
    #[error("Token has already been used or expired")]
    TokenAlreadyUsed,

    // ── 2FA ──
    /// TOTP code or backup code is wrong — HTTP 400
    #[error("Invalid 2FA code")]
    Invalid2FACode,

    /// 2FA must be enabled before this action (e.g. set collection address) — HTTP 400
    #[error("2FA must be enabled before this action")]
    TwoFARequired,

    /// Caller tried to use 2FA but it's not turned on — HTTP 400
    #[error("2FA is not enabled")]
    TwoFANotEnabled,

    /// Caller tried to enable 2FA but it's already on — HTTP 400
    #[error("2FA is already enabled")]
    TwoFAAlreadyEnabled,

    /// `enable_totp` called before `setup_totp` — HTTP 400
    #[error("TOTP not set up. Call setup_totp first.")]
    NoPendingSetup,

    /// TOTP brute-force rate limit exceeded — HTTP 429
    #[error("Too many failed attempts, try again later")]
    RateLimited,

    // ── Password ──
    /// Password does not meet strength requirements — HTTP 400
    #[error("{0}")]
    WeakPassword(String),

    /// Old password verification failed during change_password — HTTP 400
    #[error("Current password is incorrect")]
    WrongPassword,

    // ── Validation ──
    /// Generic input validation failure — HTTP 400
    #[error("{0}")]
    InvalidInput(String),

    // ── Resource ──
    /// Entity not found — HTTP 404
    #[error("{0}")]
    NotFound(String),

    // ── Infrastructure ──
    /// Database / ORM error — HTTP 500
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),

    /// Catch-all for unexpected internal errors — HTTP 500
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
