//! Merchant Service Types
//!
//! DTOs, request/response types, and domain models for the Merchant Service.
//! Aligned with docs/system_design.md

use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

// ============================================================
// JWT Authentication Types
// ============================================================

/// Return type from verify_token — contains org, user, and role info
#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub org_id: String,
    pub user_id: String,
    pub role: String,
}

/// JWT Claims for merchant authentication
///
/// sub = org_id (backward compat with 45+ routes)
/// uid/role are Option for backward compat with old JWTs (pre Role & Org)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject: org_id (= merchant_id, backward compat)
    pub sub: String,
    /// User ID (None for old JWTs → fallback to sub)
    pub uid: Option<String>,
    /// Role (None for old JWTs → fallback to "owner")
    pub role: Option<String>,
    /// Expiration timestamp (Unix epoch)
    pub exp: i64,
    /// Issued at timestamp (Unix epoch)
    pub iat: i64,
    /// JWT ID (unique identifier for this token)
    pub jti: String,
    /// Token version - must match users.token_version to be valid
    pub tv: i32,
    /// User display name (for sandbox JIT shadow accounts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// User email (for sandbox JIT shadow accounts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

// ============================================================
// Registration & Login Types
// ============================================================

/// Merchant registration request
#[derive(Debug, Clone)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    /// If set, user joins existing org via invitation instead of creating new merchant
    pub invite_token: Option<String>,
    /// Agent referral code — if valid, links new merchant to agent and sets custom fee
    pub referral_code: Option<String>,
}

/// Login response - handles both direct login and 2FA flow
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum LoginResponse {
    /// Login successful, JWT token issued
    #[serde(rename = "success")]
    Success {
        token: String,
        /// org_id (backward compat field name)
        merchant_id: String,
        /// user_id (new)
        user_id: String,
        /// role (new)
        role: String,
        /// org name (new, for frontend UserState)
        org_name: String,
        expires_at: i64,
    },
    /// 2FA verification required
    #[serde(rename = "requires_2fa")]
    Requires2FA {
        /// Temporary token for 2FA verification (short-lived, ~5 minutes)
        temp_token: String,
        /// org_id (backward compat)
        merchant_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct MerchantResponse {
    pub id: String,
    pub name: String,
    pub email: String,
    /// The authenticated user's personal name (from users table)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    /// Alias for `name` — the organization/merchant name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    /// TRON collection address (Production) — backward compat
    pub collection_address: Option<String>,
    /// TRON collection address (Sandbox) — backward compat
    pub collection_address_sandbox: Option<String>,
    /// Per-chain collection addresses: { "TRON": { "production": addr, "sandbox": addr }, "BSC": {...} }
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub collection_addresses: HashMap<String, HashMap<String, Option<String>>>,
    pub is_2fa_enabled: bool,
    pub status: String,
    /// Production USDT balance — SUM of all chain balances (microunits string, backward compat)
    pub balance_prod: String,
    /// Sandbox USDT balance — SUM of all chain balances (microunits string, backward compat)
    pub balance_sandbox: String,
    /// Production USDC balance — SUM of all chain USDC balances (microunits string)
    pub usdc_balance_prod: String,
    /// Sandbox USDC balance — SUM of all chain USDC balances (microunits string)
    pub usdc_balance_sandbox: String,
    /// Per-chain USDT balances: { "TRON": { "production": "100000000" }, "BSC": { "sandbox": "50000000" } }
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub chain_balances: HashMap<String, HashMap<String, String>>,
    /// Per-chain USDC balances: same structure as chain_balances
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub chain_usdc_balances: HashMap<String, HashMap<String, String>>,
    pub gas_unit: String,
}

/// Claims for temporary 2FA token (short-lived)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempAuthClaims {
    /// Subject: user_id (changed from merchant_id for Role & Org)
    pub sub: String,
    /// Expiration timestamp (Unix epoch) - typically 5 minutes
    pub exp: i64,
    /// Marker to distinguish from regular JWT
    pub purpose: String,
    /// org_id needed to issue final JWT after 2FA verification
    pub org_id: Option<String>,
    /// role needed to issue final JWT after 2FA verification
    pub org_role: Option<String>,
}

// ============================================================
// API Key Types
// ============================================================

/// API Key creation response
/// Note: The full key is only shown once at creation time
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct ApiKeyResponse {
    pub id: String,
    /// Full API key - shown only once!
    pub key: String,
    /// Key prefix (sk_test_ or sk_live_)
    pub prefix: String,
    /// Optional name for the API key
    pub name: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// API Key summary for listing (without sensitive data)
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeySummary {
    pub id: String,
    /// Key prefix (sk_test_ or sk_live_) - masked for security
    pub prefix: String,
    /// Optional name for the API key
    pub name: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

// ============================================================
// Password Validation Types
// ============================================================

/// Password validation requirements
#[derive(Debug, Clone)]
pub struct PasswordRequirements {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl Default for PasswordRequirements {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: false, // Optional for better UX
        }
    }
}

// ============================================================
// Email Verification Types
// ============================================================

/// JWT Claims for email verification token
///
/// Uses stateless JWT approach - no database storage needed.
/// Token expires after 24 hours.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerificationClaims {
    /// Subject: user_id (queries users table for email_verified)
    pub sub: String,
    /// Expiration timestamp (Unix epoch)
    pub purpose: String,
    pub exp: i64,
}

/// JWT Claims for password reset token
///
/// Uses stateless JWT approach with short expiry for security.
/// Token expires after 1 hour and can only be used once (checked via token_version).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetClaims {
    /// Subject: user_id (queries users table)
    pub sub: String,
    /// Expiration timestamp (Unix epoch)
    pub exp: i64,
    /// Purpose marker - must be "password_reset"
    pub purpose: String,
    /// Token version - must match current users.token_version
    pub tv: i32,
}

/// JWT Claims for team invitation token
///
/// Uses stateless JWT approach with 24h expiry.
/// On accept, the user's email must match the claims email (anti-hijack).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteClaims {
    /// Subject: org_member row ID (for lookup on accept)
    pub sub: String,
    /// Target organization ID
    pub org_id: String,
    /// Invited email address (verified against user.email on accept)
    pub email: String,
    /// Purpose marker - must be "team_invite"
    pub purpose: String,
    /// Expiration timestamp (Unix epoch, 24h)
    pub exp: i64,
    /// Issued at timestamp
    pub iat: i64,
}

// ============================================================
// 2FA Backup Codes (Internal)
// ============================================================

/// Internal structure for backup code storage
///
/// Stored as JSON array in merchants.backup_codes field.
/// Each code is hashed with SHA-256 for security.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupCodeEntry {
    /// SHA-256 hash of the backup code
    pub hash: String,
    /// Whether this code has been used
    pub used: bool,
}

// ============================================================
// 2FA (TOTP) Types
// ============================================================

/// TOTP setup response (for Google Authenticator)
#[derive(Debug, Clone, Serialize)]
pub struct TotpSetupResponse {
    /// Base32 encoded secret for manual entry
    pub secret: String,
    /// QR code URI for authenticator apps
    pub qr_code_uri: String,
    /// Backup codes for recovery
    pub backup_codes: Vec<String>,
}

/// TOTP verification request
#[derive(Debug, Clone, Deserialize)]
pub struct TotpVerifyRequest {
    /// 6-digit OTP code from authenticator
    pub code: String,
}

// ============================================================
// Webhook Endpoint Types - Aligned with system_design.md §1.6
// ============================================================

/// Webhook endpoint creation request
#[derive(Debug, Clone, Deserialize)]
pub struct CreateWebhookEndpointRequest {
    pub url: String,
    pub description: Option<String>,
}

/// Webhook endpoint response
#[derive(Debug, Clone, Serialize)]
pub struct WebhookEndpointResponse {
    pub id: String,
    pub url: String,
    pub description: Option<String>,
    /// The secret is only shown once at creation
    pub secret: Option<String>,
    pub status: String,
    pub created_at: String,
}

/// Webhook endpoint update request
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWebhookEndpointRequest {
    pub url: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

// ============================================================
// Billing Types - Aligned with system_design.md §1.7
// ============================================================

/// Billing log entry for audit trail
#[derive(Debug, Clone, Serialize)]
pub struct BillingLogEntry {
    pub id: String,
    pub log_type: String,
    pub previous_balance: i64,
    pub amount_change: i64,
    pub balance_after: i64,
    pub description: Option<String>,
    pub created_at: String,
}

// ============================================================
// Merchant Profile Types
// ============================================================

/// Merchant profile update request
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMerchantRequest {
    pub name: Option<String>,
}

/// Full merchant profile response (combines user + org data)
#[derive(Debug, Clone, Serialize)]
pub struct MerchantProfileResponse {
    /// org_id
    pub id: String,
    /// org name
    pub name: String,
    /// user email (from users table)
    pub email: String,
    pub status: String,
    /// from users table
    pub is_totp_enabled: bool,
    /// from users table
    pub email_verified: bool,
    pub account_index: i32,
    pub created_at: String,
    /// user_id (new)
    pub user_id: String,
    /// role in this org (new)
    pub role: String,
}

// ============================================================
// Dashboard Statistics Types
// ============================================================

use sea_orm::prelude::Decimal;

/// Helper struct for dashboard statistics aggregation
#[derive(Debug, FromQueryResult)]
pub struct StatsResult {
    pub volume: Option<Decimal>,
    pub count: i64,
}
