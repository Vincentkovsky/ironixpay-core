use crate::api::dtos::checkout::from_micro;
use crate::entity::{billing_logs, outbound_transactions};
use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

/// Optional filter params for GET /api/internal/billing/logs (beyond pagination)
#[derive(Debug, Deserialize, Default)]
pub struct BillingLogsFilter {
    /// Filter by network (e.g. "BSC", "POLYGON", "TRON")
    pub network: Option<String>,
}

/// Query params for GET /api/internal/billing/logs/export
#[derive(Debug, Deserialize)]
pub struct BillingExportRequest {
    /// Inclusive start date (YYYY-MM-DD)
    pub start_date: Option<NaiveDate>,
    /// Inclusive end date (YYYY-MM-DD)
    pub end_date: Option<NaiveDate>,
    /// Filter by billing type
    #[serde(rename = "type")]
    pub billing_type: Option<String>,
}

/// Query params for GET /api/internal/billing/payments/export
#[derive(Debug, Deserialize)]
pub struct PaymentsExportRequest {
    /// Inclusive start date (YYYY-MM-DD)
    pub start_date: Option<NaiveDate>,
    /// Inclusive end date (YYYY-MM-DD)
    pub end_date: Option<NaiveDate>,
    /// Filter by session status (e.g. "Paid", "Expired", "all")
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingLogResponse {
    pub id: String,
    pub environment: String,
    pub network: String,
    pub merchant_id: String,
    pub session_id: Option<String>,
    pub external_ref_id: Option<String>,
    #[serde(rename = "type")]
    pub billing_type: billing_logs::BillingType,
    /// Serialized as String to prevent JS precision loss
    pub previous_balance: String,
    /// Serialized as String to prevent JS precision loss
    pub amount_change: String,
    /// Serialized as String to prevent JS precision loss
    pub balance_after: String,
    pub description: Option<String>,
    pub currency: String,
    pub created_at: DateTime<FixedOffset>,
    /// Sub-merchant code (None if belongs to parent merchant directly)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_merchant_code: Option<String>,
}

impl BillingLogResponse {
    pub fn from_model(model: billing_logs::Model, sub_merchant_code: Option<String>) -> Self {
        let currency = &model.token;
        Self {
            id: model.id,
            environment: model.environment.to_string(),
            network: model.network,
            merchant_id: model.merchant_id,
            session_id: model.session_id,
            external_ref_id: model.external_ref_id,
            billing_type: model.billing_type,
            previous_balance: from_micro(model.previous_balance, currency),
            amount_change: from_micro(model.amount_change, currency),
            balance_after: from_micro(model.balance_after, currency),
            description: model.description,
            currency: model.token,
            created_at: model.created_at,
            sub_merchant_code,
        }
    }
}

impl From<billing_logs::Model> for BillingLogResponse {
    fn from(model: billing_logs::Model) -> Self {
        Self::from_model(model, None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepTransactionResponse {
    pub id: String,
    pub network: String,
    pub merchant_id: String,
    pub session_id: Option<String>,
    pub tx_hash: Option<String>,
    /// Serialized as String to prevent JS precision loss
    pub amount: String,
    pub from_address: String,
    pub to_address: String,
    pub status: outbound_transactions::OutboundState,
    pub error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub confirmed_at: Option<DateTime<FixedOffset>>,
}

impl From<outbound_transactions::Model> for SweepTransactionResponse {
    fn from(model: outbound_transactions::Model) -> Self {
        Self {
            id: model.id,
            network: model.network,
            merchant_id: model.merchant_id,
            session_id: model.session_id,
            tx_hash: model.tx_hash,
            amount: from_micro(model.amount, "USDT"),
            from_address: model.from_address,
            to_address: model.to_address,
            status: model.state,
            error_message: model.error_message,
            created_at: model.created_at,
            confirmed_at: model.confirmed_at,
        }
    }
}
