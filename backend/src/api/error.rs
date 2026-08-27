use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

// Re-export for route handlers
// TODO: Migrate all remaining Json<Body> routes (merchants.rs, resolution.rs, webhooks.rs)
//       to use AppJson so JSON parse errors return Stripe-style responses everywhere.
pub use self::json_extractor::AppJson;

/// Custom JSON extractor that returns Stripe-style errors on parse failure.
///
/// Drop-in replacement for `axum::Json<T>`. Usage:
/// ```ignore
/// async fn handler(AppJson(body): AppJson<MyBody>) -> Result<Json<Response>, AppError> { ... }
/// ```
mod json_extractor {
    use super::*;
    use axum::{
        async_trait,
        extract::{FromRequest, Request},
    };

    pub struct AppJson<T>(pub T);

    #[async_trait]
    impl<S, T> FromRequest<S> for AppJson<T>
    where
        axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
        S: Send + Sync,
    {
        type Rejection = AppError;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            match axum::Json::<T>::from_request(req, state).await {
                Ok(Json(value)) => Ok(AppJson(value)),
                Err(rejection) => Err(AppError::from(rejection)),
            }
        }
    }
}

/// Stripe-style error response body.
///
/// All API errors are returned in this format:
/// ```json
/// {
///   "error": {
///     "type": "invalid_request_error",
///     "code": "resource_missing",
///     "message": "No such session: 'cs_123xyz'",
///     "param": "id",
///     "doc_url": "https://ironixpay.com/guide/errors#resource_missing"
///   }
/// }
/// ```
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct ApiErrorBody {
    /// The type of error. One of `api_error`, `invalid_request_error`, `idempotency_error`.
    #[serde(rename = "type")]
    #[schema(example = "invalid_request_error")]
    pub error_type: ApiErrorType,

    /// A short machine-readable code indicating the specific error.
    #[schema(example = "authentication_failed")]
    pub code: String,

    /// A human-readable message providing more details about the error.
    #[schema(example = "Invalid API key provided")]
    pub message: String,

    /// If the error is parameter-specific, the parameter related to the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub param: Option<String>,

    /// A URL to more information about the error code reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://ironixpay.com/guide/errors#authentication_failed")]
    pub doc_url: Option<String>,
}

/// Error type classification (Stripe-style).
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub enum ApiErrorType {
    /// Errors caused by invalid requests from the client.
    #[serde(rename = "invalid_request_error")]
    InvalidRequestError,
    /// Internal server errors or external dependency failures.
    #[serde(rename = "api_error")]
    ApiError,
    /// Errors related to idempotency key misuse.
    #[serde(rename = "idempotency_error")]
    IdempotencyError,
}

/// The top-level error response envelope.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

/// Base URL for error documentation links.
const DOC_BASE_URL: &str = "https://ironixpay.com/guide/errors";

// ── Error Code Constants ──
// Generic
pub const E_PARAMETER_INVALID: &str = "parameter_invalid";
// Auth & 2FA
pub const E_2FA_REQUIRED: &str = "2fa_required";
pub const E_INVALID_2FA_CODE: &str = "invalid_2fa_code";
// Payout / Withdrawal
pub const E_INSUFFICIENT_BALANCE: &str = "insufficient_balance";
pub const E_NO_COLLECTION_ADDRESS: &str = "no_collection_address";
pub const E_INVALID_ADDRESS: &str = "invalid_address";
pub const E_SELF_TRANSFER: &str = "self_transfer";
pub const E_INVALID_AMOUNT: &str = "invalid_amount";
pub const E_NO_CHAIN_ACCOUNT: &str = "no_chain_account";
// Resolution
pub const E_ACTION_NOT_ALLOWED: &str = "action_not_allowed";
pub const E_AMOUNT_TOO_SMALL: &str = "amount_too_small";
pub const E_AML_BLOCKED: &str = "aml_blocked";
pub const E_ADDRESS_SWEEPING: &str = "address_sweeping";
// Merchant
pub const E_EMAIL_NOT_VERIFIED: &str = "email_not_verified";
pub const E_ACCOUNT_SUSPENDED: &str = "account_suspended";
pub const E_WEAK_PASSWORD: &str = "weak_password";
pub const E_WRONG_PASSWORD: &str = "wrong_password";
pub const E_RATE_LIMITED: &str = "rate_limited";
pub const E_TOKEN_EXPIRED: &str = "token_expired";
pub const E_2FA_ALREADY_ENABLED: &str = "2fa_already_enabled";
pub const E_2FA_NOT_SETUP: &str = "2fa_not_setup";
pub const E_DISPOSABLE_EMAIL_NOT_ALLOWED: &str = "disposable_email_not_allowed";
pub const E_HUMAN_VERIFICATION_FAILED: &str = "human_verification_failed";

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Validation error: {message}")]
    ValidationError {
        code: &'static str,
        message: String,
        param: Option<String>,
    },

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal server error: {0}")]
    InternalServerError(anyhow::Error),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Gone: {0}")]
    Gone(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    #[error("Environment mismatch: {got} does not match server environment {expected}")]
    EnvironmentMismatch { expected: String, got: String },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = self.to_status_and_body();
        (status, Json(body)).into_response()
    }
}

impl AppError {
    /// Helper to convert error into (StatusCode, serde_json::Value)
    /// Useful when manual response construction is needed (e.g. for idempotency caching)
    pub fn to_status_and_body(&self) -> (StatusCode, serde_json::Value) {
        let (status, error_body) = self.to_api_error();

        // Log internal server errors
        if let AppError::InternalServerError(ref e) = self {
            tracing::error!("Internal Server Error: {:?}", e);
        }

        let response = ApiErrorResponse { error: error_body };
        (status, serde_json::to_value(response).unwrap())
    }

    /// Convert AppError into (StatusCode, ApiErrorBody)
    fn to_api_error(&self) -> (StatusCode, ApiErrorBody) {
        match self {
            AppError::AuthError(msg) => (
                StatusCode::UNAUTHORIZED,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "authentication_failed".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#authentication_failed")),
                },
            ),

            AppError::ValidationError {
                code,
                message,
                param,
            } => (
                StatusCode::BAD_REQUEST,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: code.to_string(),
                    message: message.clone(),
                    param: param.clone(),
                    doc_url: Some(format!("{DOC_BASE_URL}#{code}")),
                },
            ),

            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "resource_missing".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#resource_missing")),
                },
            ),

            AppError::PermissionDenied(msg) => (
                StatusCode::FORBIDDEN,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "permission_denied".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#permission_denied")),
                },
            ),

            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "conflict".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#conflict")),
                },
            ),

            AppError::Gone(msg) => (
                StatusCode::GONE,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "session_expired".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#session_expired")),
                },
            ),

            AppError::TooManyRequests(msg) => (
                StatusCode::TOO_MANY_REQUESTS,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: E_RATE_LIMITED.to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#{E_RATE_LIMITED}")),
                },
            ),

            AppError::EnvironmentMismatch { expected, got } => (
                StatusCode::FORBIDDEN,
                ApiErrorBody {
                    error_type: ApiErrorType::InvalidRequestError,
                    code: "environment_mismatch".to_string(),
                    message: format!(
                        "Environment mismatch. You are targeting '{}' but this instance is '{}'.",
                        got, expected
                    ),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#environment_mismatch")),
                },
            ),

            AppError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorBody {
                    error_type: ApiErrorType::ApiError,
                    code: "service_unavailable".to_string(),
                    message: msg.clone(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#service_unavailable")),
                },
            ),

            AppError::InternalServerError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorBody {
                    error_type: ApiErrorType::ApiError,
                    code: "api_error".to_string(),
                    message: "Internal server error. Please try again later.".to_string(),
                    param: None,
                    doc_url: Some(format!("{DOC_BASE_URL}#api_error")),
                },
            ),
        }
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        let message = match &rejection {
            JsonRejection::JsonDataError(e) => {
                // Serde gives us e.g. "network: unknown variant `tron`, expected `TRON` at line 6 column 19"
                // Clean up: strip line/column suffix for cleaner error message
                let body = e.body_text();
                let msg = body
                    .rsplit_once(" at line ")
                    .map(|(prefix, _)| prefix.to_string())
                    .unwrap_or(body);

                // Special case: i64 overflow on amount field (e.g., user passed 18-decimal BSC value)
                if msg.contains("expected i64") && msg.contains("amount") {
                    format!(
                        "Invalid amount: value overflows i64. All amounts should be \
                         provided as human-readable decimal strings \
                         (e.g., \"10.5\" for 10.5 USDT)."
                    )
                } else {
                    format!("Invalid request body: {}", msg)
                }
            }
            JsonRejection::JsonSyntaxError(_) => {
                "Invalid JSON: request body contains malformed JSON".to_string()
            }
            JsonRejection::MissingJsonContentType(_) => {
                "Missing Content-Type: expected Content-Type: application/json".to_string()
            }
            _ => format!("Invalid request body: {}", rejection),
        };
        AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message,
            param: None,
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalServerError(err)
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        // Extract the first field name from validation errors for the `param` field
        let param = err.field_errors().keys().next().map(|k| k.to_string());
        AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: err.to_string(),
            param,
        }
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        match err {
            sea_orm::DbErr::RecordNotFound(e) => AppError::NotFound(e),
            _ => AppError::InternalServerError(anyhow::anyhow!(err)),
        }
    }
}

impl From<crate::services::checkout::CheckoutError> for AppError {
    fn from(err: crate::services::checkout::CheckoutError) -> Self {
        use crate::services::checkout::CheckoutError;
        match err {
            CheckoutError::AddressPoolExhausted => AppError::ServiceUnavailable(err.to_string()),
            CheckoutError::SessionNotFound(msg) => AppError::NotFound(msg),
            CheckoutError::SessionExpired(msg) => AppError::Gone(msg),
            CheckoutError::InvalidRequest(msg) => AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: msg,
                param: None,
            },
            CheckoutError::Database(e) => e.into(),
        }
    }
}

impl From<crate::services::resolution::ResolutionError> for AppError {
    fn from(err: crate::services::resolution::ResolutionError) -> Self {
        use crate::services::resolution::ResolutionError;
        match err {
            ResolutionError::NotFound(msg) => AppError::NotFound(msg),
            ResolutionError::SessionNotFound(msg) => AppError::NotFound(msg),
            ResolutionError::Unauthorized => {
                AppError::PermissionDenied("This exception belongs to a different merchant".into())
            }
            ResolutionError::ActionNotAllowed { action, reason } => AppError::ValidationError {
                code: E_ACTION_NOT_ALLOWED,
                message: format!("Action '{}' not allowed: {}", action, reason),
                param: None,
            },
            ResolutionError::ValidationError(msg) => AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: msg,
                param: None,
            },
            ResolutionError::TwoFactorFailed(msg) => AppError::ValidationError {
                code: E_INVALID_2FA_CODE,
                message: msg,
                param: Some("totp_code".into()),
            },
            ResolutionError::Sweeping => AppError::ValidationError {
                code: E_ADDRESS_SWEEPING,
                message: "System is currently auto-sweeping this address, please wait and retry"
                    .into(),
                param: None,
            },
            ResolutionError::AmlBlocked(msg) => AppError::ValidationError {
                code: E_AML_BLOCKED,
                message: msg,
                param: None,
            },
            ResolutionError::AmountTooSmall { amount, fee } => AppError::ValidationError {
                code: E_AMOUNT_TOO_SMALL,
                message: format!(
                    "Amount ({} sun) is too small to cover the fee ({} sun)",
                    amount, fee
                ),
                param: Some("amount".into()),
            },
            ResolutionError::InsufficientBalance {
                available,
                required,
            } => AppError::ValidationError {
                code: E_INSUFFICIENT_BALANCE,
                message: format!(
                    "Insufficient balance on payment address: available {} sun, required {} sun. \
                         Funds may have been swept before this fix was deployed.",
                    available, required
                ),
                param: None,
            },
            ResolutionError::InvalidState => AppError::Conflict(
                "Exception is not in a valid state for this operation (expected: pending)".into(),
            ),
            ResolutionError::Database(e) => e.into(),
            ResolutionError::Internal(e) => AppError::InternalServerError(e),
        }
    }
}

impl From<crate::services::payout::PayoutError> for AppError {
    fn from(err: crate::services::payout::PayoutError) -> Self {
        use crate::services::payout::PayoutError;
        match err {
            PayoutError::InvalidAmount(msg) => AppError::ValidationError {
                code: E_INVALID_AMOUNT,
                message: msg,
                param: Some("amount".into()),
            },
            PayoutError::InsufficientBalance { have, need, currency } => AppError::ValidationError {
                code: E_INSUFFICIENT_BALANCE,
                message: format!(
                    "Insufficient balance: have {} {}, need {} {}",
                    crate::api::dtos::checkout::from_micro(have, &currency), currency,
                    crate::api::dtos::checkout::from_micro(need, &currency), currency
                ),
                param: Some("amount".into()),
            },
            PayoutError::NoChainAccount {
                merchant_id,
                environment,
            } => AppError::ValidationError {
                code: E_NO_CHAIN_ACCOUNT,
                message: format!(
                    "No chain account found for merchant '{}' in {}. Please complete account setup.",
                    merchant_id, environment
                ),
                param: None,
            },
            PayoutError::NoCollectionAddress => AppError::ValidationError {
                code: E_NO_COLLECTION_ADDRESS,
                message: "Merchant has no collection address configured. Please set one in account settings.".into(),
                param: Some("collection_address".into()),
            },
            PayoutError::InvalidAddress { message, param } => AppError::ValidationError {
                code: E_INVALID_ADDRESS,
                message,
                param: Some(param),
            },
            PayoutError::SelfTransfer { message, param } => AppError::ValidationError {
                code: E_SELF_TRANSFER,
                message,
                param: Some(param),
            },
            PayoutError::IdempotencyConflict => AppError::Conflict(
                "Idempotency key already used for a different payout".into(),
            ),
            PayoutError::Database(e) => e.into(),
            PayoutError::Internal(e) => AppError::InternalServerError(e),
        }
    }
}

impl From<crate::services::merchant::MerchantError> for AppError {
    fn from(err: crate::services::merchant::MerchantError) -> Self {
        use crate::services::merchant::MerchantError;
        match err {
            // Auth — 401
            MerchantError::InvalidCredentials => {
                AppError::AuthError(err.to_string())
            }
            MerchantError::InvalidToken | MerchantError::TokenAlreadyUsed => {
                AppError::AuthError(err.to_string())
            }

            // Forbidden — 403
            MerchantError::AccountSuspended => {
                AppError::PermissionDenied(err.to_string())
            }

            // Conflict — 409
            MerchantError::EmailAlreadyRegistered => {
                AppError::Conflict(
                    "Email already registered. Please use a different email or log in to your existing account.".into(),
                )
            }

            // Too Many Requests — 429
            MerchantError::RateLimited | MerchantError::RegistrationRateLimited => {
                AppError::TooManyRequests(err.to_string())
            }

            // Not Found — 404
            MerchantError::NotFound(msg) => AppError::NotFound(msg),

            // Validation — 400 (with specific codes)
            MerchantError::EmailNotVerified => AppError::ValidationError {
                code: E_EMAIL_NOT_VERIFIED,
                message: err.to_string(),
                param: Some("email".into()),
            },
            MerchantError::DisposableEmailNotAllowed => AppError::ValidationError {
                code: E_DISPOSABLE_EMAIL_NOT_ALLOWED,
                message: err.to_string(),
                param: Some("email".into()),
            },
            MerchantError::Invalid2FACode => AppError::ValidationError {
                code: E_INVALID_2FA_CODE,
                message: err.to_string(),
                param: Some("totp_code".into()),
            },
            MerchantError::TwoFARequired | MerchantError::TwoFANotEnabled => {
                AppError::ValidationError {
                    code: E_2FA_REQUIRED,
                    message: err.to_string(),
                    param: Some("totp_code".into()),
                }
            }
            MerchantError::TwoFAAlreadyEnabled => AppError::ValidationError {
                code: E_2FA_ALREADY_ENABLED,
                message: err.to_string(),
                param: None,
            },
            MerchantError::NoPendingSetup => AppError::ValidationError {
                code: E_2FA_NOT_SETUP,
                message: err.to_string(),
                param: None,
            },
            MerchantError::WeakPassword(msg) => AppError::ValidationError {
                code: E_WEAK_PASSWORD,
                message: msg,
                param: Some("password".into()),
            },
            MerchantError::WrongPassword => AppError::ValidationError {
                code: E_WRONG_PASSWORD,
                message: err.to_string(),
                param: Some("old_password".into()),
            },
            MerchantError::InvalidInput(msg) => AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: msg,
                param: None,
            },

            // Infrastructure — 500
            MerchantError::Database(e) => e.into(),
            MerchantError::Internal(e) => AppError::InternalServerError(e),
        }
    }
}
