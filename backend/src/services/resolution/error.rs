//! Resolution Service Error Types
//!
//! Provides typed errors so the route layer can map to correct HTTP status codes
//! without fragile string matching.

use thiserror::Error;

/// Resolution service specific errors
#[derive(Debug, Error)]
pub enum ResolutionError {
    /// Exception not found — HTTP 404
    #[error("Exception not found: {0}")]
    NotFound(String),

    /// Session referenced by exception not found — HTTP 404
    #[error("Target session not found: {0}")]
    SessionNotFound(String),

    /// Merchant does not own this exception — HTTP 403
    #[error("This exception belongs to a different merchant")]
    Unauthorized,

    /// Action not allowed for current exception state — HTTP 400
    #[error("Action '{action}' not allowed: {reason}")]
    ActionNotAllowed { action: String, reason: String },

    /// Validation error (bad address, bad amount, etc.) — HTTP 400
    #[error("{0}")]
    ValidationError(String),

    /// 2FA verification failed — HTTP 401
    #[error("2FA verification failed: {0}")]
    TwoFactorFailed(String),

    /// Address is currently being swept — HTTP 409
    #[error("System is currently auto-sweeping this address, please wait and retry")]
    Sweeping,

    /// AML compliance block — HTTP 403
    #[error("{0}")]
    AmlBlocked(String),

    /// Amount too small after fee deduction — HTTP 400
    #[error("Amount ({amount} sun) is too small to cover the fee ({fee} sun)")]
    AmountTooSmall { amount: i64, fee: i64 },

    /// Insufficient on-chain balance for refund — HTTP 400
    #[error(
        "Insufficient balance on payment address: available {available} sun, required {required} sun"
    )]
    InsufficientBalance { available: i64, required: i64 },

    /// Exception is no longer in a valid state for this operation — HTTP 409
    #[error("Exception is not in a valid state for this operation (expected: pending)")]
    InvalidState,

    /// Database error — HTTP 500
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    /// Internal / catch-all — HTTP 500
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}
