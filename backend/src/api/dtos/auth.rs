use crate::entity::network::Network;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct RegisterRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,

    /// Optional invite token. If present, the user joins an existing org
    /// instead of creating a new merchant.
    #[serde(default)]
    pub invite_token: Option<String>,

    /// Agent referral code. If valid, links new merchant to agent.
    #[serde(default)]
    pub referral_code: Option<String>,

    /// Cloudflare Turnstile token. Required whenever Turnstile is enabled.
    #[serde(default)]
    #[validate(length(max = 2048, message = "Invalid verification token"))]
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
#[serde(tag = "status")]
pub enum LoginResponse {
    #[serde(rename = "success")]
    Success {
        token: String,
        merchant_id: String,
        user_id: String,
        role: String,
        org_name: String,
        expires_at: i64,
        merchant: MerchantResponse,
    },
    #[serde(rename = "requires_2fa")]
    Requires2FA {
        temp_token: String,
        merchant_id: String,
    },
}

pub use crate::services::merchant::{ApiKeyResponse, MerchantResponse};

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct CreateApiKeyRequest {
    /// Optional name for the API key (e.g., "Production Server")
    pub name: Option<String>,
    #[serde(default)]
    pub is_test: bool,
    /// 2FA code - required if 2FA is enabled
    pub code: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct MerchantBalanceResponse {
    /// Total balance in standard units (e.g., "10.5" = 10.5 USDT)
    pub balance: String,
    pub unit: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct MerchantStatsResponse {
    pub total_volume_usdt: String,
    pub today_volume_usdt: String,
    pub total_transactions: u64,
    pub total_transactions_today: u64,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ResendVerificationRequest {
    pub email: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct Verify2FARequest {
    pub temp_token: String,
    pub code: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code_uri: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct Enable2FARequest {
    pub code: String,
}
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../frontend/packages/api-client/src/bindings/ApiKeyListResponse.ts"
)]
pub struct ApiKeyListResponse {
    pub keys: Vec<ApiKeyResponse>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,

    /// Optional: Required if 2FA is enabled
    pub code: Option<String>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 100, message = "Name must be 1-100 characters"))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct UpdateWalletAddressRequest {
    pub network: Network, // "TRON"
    pub collection_address: String,

    /// 2FA code is required for changing money destination
    pub code: Option<String>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "Old password is required"))]
    pub old_password: String,

    #[validate(length(min = 8, message = "New password must be at least 8 characters"))]
    pub new_password: String,

    /// Optional: Required if 2FA is enabled
    pub code: Option<String>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    /// Cloudflare Turnstile token. Required whenever Turnstile is enabled.
    #[serde(default)]
    #[validate(length(max = 2048, message = "Invalid verification token"))]
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ResetPasswordRequest {
    pub token: String,

    #[validate(length(min = 8, message = "New password must be at least 8 characters"))]
    pub new_password: String,
}

/// Request to switch the current user's active organization context.
/// Issues a new JWT scoped to the target org.
#[derive(Debug, Deserialize, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct SwitchOrgRequest {
    /// Target organization ID to switch to
    #[validate(length(min = 1, message = "org_id is required"))]
    pub org_id: String,
}

/// Response from switch-org containing the new JWT and org context.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct SwitchOrgResponse {
    pub token: String,
    /// org_id (backward compat field name)
    pub merchant_id: String,
    pub user_id: String,
    pub role: String,
    pub org_name: String,
    pub expires_at: i64,
}
