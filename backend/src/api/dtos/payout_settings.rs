//! Payout Settings DTOs for risk control configuration and approval endpoints

use serde::{Deserialize, Serialize};

/// GET /api/internal/settings/payout response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutSettingsResponse {
    pub require_new_address_approval: bool,
    /// Approval threshold in human-readable decimal (e.g., "5000")
    pub approval_threshold: String,
    pub approver_roles: Vec<String>,
    pub auto_withdraw_enabled: bool,
    pub auto_withdraw_threshold: Option<String>,
    pub auto_withdraw_network: Option<String>,
    pub auto_withdraw_currency: String,
}

impl From<crate::entity::payout_settings::Model> for PayoutSettingsResponse {
    fn from(m: crate::entity::payout_settings::Model) -> Self {
        use crate::api::dtos::checkout::from_micro;
        let currency = &m.auto_withdraw_currency;
        Self {
            require_new_address_approval: m.require_new_address_approval,
            approval_threshold: from_micro(m.approval_threshold, "USDT"),
            approver_roles: m
                .approver_roles
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            auto_withdraw_enabled: m.auto_withdraw_enabled,
            auto_withdraw_threshold: m.auto_withdraw_threshold.map(|v| from_micro(v, currency)),
            auto_withdraw_network: m.auto_withdraw_network,
            auto_withdraw_currency: m.auto_withdraw_currency,
        }
    }
}

/// PUT /api/internal/settings/payout request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePayoutSettingsRequest {
    pub require_new_address_approval: Option<bool>,
    /// Approval threshold as decimal string. "-1" = disabled, "0" = all amounts, ">0" = exceeding.
    pub approval_threshold: Option<String>,
    /// List of role names allowed to approve. "owner" must always be included.
    pub approver_roles: Option<Vec<String>>,
    pub auto_withdraw_enabled: Option<bool>,
    pub auto_withdraw_threshold: Option<String>,
    pub auto_withdraw_network: Option<String>,
    pub auto_withdraw_currency: Option<String>,
}

/// POST /api/internal/payouts/:id/approve or withdrawals/:id/approve
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveRequest {
    /// TOTP 2FA code (required for approve actions)
    pub totp_code: String,
}

/// POST /api/internal/payouts/:id/reject or withdrawals/:id/reject
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectRequest {
    /// TOTP 2FA code (required for reject actions)
    pub totp_code: String,
    /// Optional reason for rejection (stored in error_reason for audit)
    pub reason: Option<String>,
}
