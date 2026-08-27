//! Withdrawal DTOs for merchant payout API
//!
//! All amounts are human-readable decimal strings (e.g., "10.5" = 10.5 USDT).

use crate::api::dtos::checkout::from_micro;
use crate::entity::withdrawals;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// POST /v1/merchants/withdrawals
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalRequest {
    /// Gross withdrawal amount in standard units as a decimal string (e.g., "10.50").
    #[validate(length(min = 1, message = "amount is required"))]
    pub amount: String,

    /// 2FA TOTP code. **Required** — withdrawals are blocked unless the merchant
    /// has 2FA enabled and provides a valid code.
    pub totp_code: Option<String>,

    /// Target chain for withdrawal. Defaults to "TRON" if not provided (backward compat).
    pub network: Option<String>,

    /// Token currency: "USDT" or "USDC". Defaults to "USDT" if not provided.
    pub currency: Option<String>,
}

/// Response DTO for a single withdrawal
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalResponse {
    pub id: String,
    pub merchant_id: String,
    pub environment: String,
    pub network: String,
    /// Gross amount in standard units (e.g., "10.5")
    pub amount: String,
    /// Fee deducted (e.g., "1.5")
    pub fee: String,
    /// Actual payout = amount - fee (e.g., "9")
    pub net_amount: String,
    pub to_address: String,
    pub status: withdrawals::WithdrawalStatus,
    pub tx_hash: Option<String>,
    pub error_reason: Option<String>,
    pub currency: String,
    pub created_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
    /// Who initiated this withdrawal (user_id)
    pub requested_by: Option<String>,
    /// Who approved/cancelled (user_id)
    pub reviewed_by: Option<String>,
    /// When approval/cancellation happened
    pub reviewed_at: Option<DateTime<FixedOffset>>,
}

impl From<withdrawals::Model> for WithdrawalResponse {
    fn from(m: withdrawals::Model) -> Self {
        let currency = &m.currency;
        Self {
            id: m.id,
            merchant_id: m.merchant_id,
            environment: m.environment.to_string(),
            network: m.network,
            amount: from_micro(m.amount, currency),
            fee: from_micro(m.network_fee, currency),
            net_amount: from_micro(m.net_amount, currency),
            to_address: m.to_address,
            status: m.status,
            tx_hash: m.tx_hash,
            error_reason: m.error_reason,
            currency: currency.clone(),
            created_at: m.created_at,
            completed_at: m.completed_at,
            requested_by: m.requested_by,
            reviewed_by: m.reviewed_by,
            reviewed_at: m.reviewed_at,
        }
    }
}
