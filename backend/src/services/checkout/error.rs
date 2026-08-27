//! Checkout Service Error Types
//!
//! Provides typed errors for better HTTP status code mapping.

use thiserror::Error;

/// Checkout service specific errors
#[derive(Debug, Error)]
pub enum CheckoutError {
    /// No available addresses in pool - return HTTP 503
    #[error("No payment addresses available. Please try again shortly or contact support.")]
    AddressPoolExhausted,

    /// Session not found - return HTTP 404
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already expired - return HTTP 410
    #[error("Session expired: {0}")]
    SessionExpired(String),

    /// Invalid request parameters - return HTTP 400
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Database error - return HTTP 500
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

impl CheckoutError {
    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            Self::AddressPoolExhausted => 503, // Service Unavailable
            Self::SessionNotFound(_) => 404,   // Not Found
            Self::SessionExpired(_) => 410,    // Gone
            Self::InvalidRequest(_) => 400,    // Bad Request
            Self::Database(_) => 500,          // Internal Server Error
        }
    }

    /// Check if this is a retriable error
    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::AddressPoolExhausted | Self::Database(_))
    }
}
