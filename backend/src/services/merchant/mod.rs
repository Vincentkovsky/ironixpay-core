//! Merchant Service Module
//!
//! Handles merchant registration, authentication, API key management,
//! and billing operations.
//!
//! Aligned with docs/system_design.md §1.1
//!
//! ## Module Structure
//!
//! - `types` - DTOs, request/response types, claims
//! - `service` - Core business logic (MerchantService)
//!
//! ## Security Features
//!
//! - Argon2 password hashing
//! - JWT authentication
//! - API key with SHA-256 hashing + constant-time verification
//! - Password strength validation
//! - Atomic account_index allocation (race condition prevention)
//!
//! ## Usage
//!
//! ```ignore
//! use crate::services::merchant::{MerchantService, RegisterRequest, LoginResponse};
//!
//! let service = MerchantService::new(db, jwt_secret, jwt_expiry_hours, environment);
//! let merchant = service.register(RegisterRequest { ... }).await?;
//! let login = service.login(&email, &password).await?;
//! ```

pub mod error;
mod login_2fa_limiter;
mod registration_limiter;
mod service;
pub mod types;

// Re-export main service and error type
pub use error::MerchantError;
pub use service::MerchantService;

// Re-export types for external use
pub use types::{
    // API Keys
    ApiKeyResponse,
    ApiKeySummary,
    // Billing
    BillingLogEntry,
    // Authentication
    Claims,
    // Webhooks
    CreateWebhookEndpointRequest,
    LoginResponse,
    // Profile
    MerchantProfileResponse,
    MerchantResponse,
    // Password
    PasswordRequirements,
    RegisterRequest,
    // 2FA (reserved)
    TotpSetupResponse,
    TotpVerifyRequest,
    UpdateMerchantRequest,
    UpdateWebhookEndpointRequest,
    WebhookEndpointResponse,
};
