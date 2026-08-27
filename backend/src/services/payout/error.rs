//! Payout Service Error Types
//!
//! Provides typed errors so the route layer can map to correct HTTP status codes
//! without fragile string matching.

use thiserror::Error;

/// Payout service errors (shared by Withdrawal and Payout API paths).
///
/// Variants that differ between paths carry a `param` field so the API layer
/// can return the correct field name in Stripe-style error responses.
#[derive(Debug, Error)]
pub enum PayoutError {
    /// Amount validation failed — HTTP 400
    #[error("{0}")]
    InvalidAmount(String),

    /// Insufficient merchant balance — HTTP 400
    #[error("Insufficient balance: have {have} {currency}, need {need} {currency}")]
    InsufficientBalance {
        have: i64,
        need: i64,
        currency: String,
    },

    /// No chain account for this merchant/environment — HTTP 400
    #[error("No chain account found for merchant '{merchant_id}' in {environment}")]
    NoChainAccount {
        merchant_id: String,
        environment: String,
    },

    /// Merchant has no collection address configured — HTTP 400
    #[error("Merchant has no collection address configured. Please set one in account settings.")]
    NoCollectionAddress,

    /// Invalid destination address format — HTTP 400
    ///
    /// `param` = "to_address" (Payout API) or "collection_address" (Withdrawal).
    #[error("{message}")]
    InvalidAddress { message: String, param: String },

    /// Cannot send to treasury address — HTTP 400
    ///
    /// `param` = "to_address" (Payout API) or "collection_address" (Withdrawal).
    #[error("{message}")]
    SelfTransfer { message: String, param: String },

    /// Idempotency key already used for a different payout — HTTP 409
    #[error("Idempotency key already used")]
    IdempotencyConflict,

    /// Database / transaction error — HTTP 500
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    /// Internal / catch-all — HTTP 500
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}
