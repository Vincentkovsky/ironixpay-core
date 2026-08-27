//! Payout API DTOs
//!
//! Request/Response types for the Public Payout API.
//! All amounts are human-readable decimal strings (e.g., "10.5" = 10.5 USDT).

use crate::api::dtos::checkout::from_micro;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::entity::payouts::PayoutStatus;
use crate::entity::Network;

/// POST /v1/payouts — Create a new payout
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreatePayoutBody {
    /// Amount in standard units as a decimal string (e.g., "10.50" = 10.50 USDT).
    /// Min: 1, Max: 10,000,000, Precision: 0.01.
    #[schema(example = "10.50")]
    pub amount: String,

    /// Currency: "USDT" or "USDC".
    #[schema(example = "USDT")]
    #[validate(custom(function = "validate_currency"))]
    pub currency: String,

    /// Blockchain network for the payout.
    #[schema(example = "TRON")]
    pub network: Network,

    /// Destination address on the specified network.
    #[schema(example = "TJn9bXhJMVn1Do3PfFHg5J3YNYN9hPQBqA")]
    pub to_address: String,

    /// Optional human-readable description.
    #[schema(example = "Affiliate commission for January")]
    pub description: Option<String>,

    /// Optional metadata (arbitrary JSON, max 50 keys).
    pub metadata: Option<serde_json::Value>,
}

fn validate_currency(currency: &str) -> Result<(), validator::ValidationError> {
    match currency {
        "USDT" | "USDC" => Ok(()),
        _ => Err(validator::ValidationError::new("unsupported_currency")),
    }
}

/// Payout response object
#[derive(Debug, Serialize, ToSchema)]
pub struct PayoutResponse {
    /// Payout ID (e.g. "po_abc123")
    #[schema(example = "po_a1b2c3d4e5f6")]
    pub id: String,
    /// Whether this is a live or sandbox payout
    #[schema(example = false)]
    pub livemode: bool,
    /// Current status: Pending, Processing, Completed, Failed
    pub status: PayoutStatus,
    /// Gross amount requested (e.g., "10" = 10 USDT)
    #[schema(example = "10")]
    pub amount: String,
    /// Platform fee deducted (e.g., "1.5" = 1.5 USDT)
    #[schema(example = "1.5")]
    pub fee: String,
    /// Net amount sent on-chain (e.g., "8.5" = 8.5 USDT)
    #[schema(example = "8.5")]
    pub net_amount: String,
    /// Currency ("USDT" or "USDC")
    #[schema(example = "USDT")]
    pub currency: String,
    /// Blockchain network
    #[schema(example = "TRON")]
    pub network: String,
    /// Destination address
    #[schema(example = "TJn9bXhJMVn1Do3PfFHg5J3YNYN9hPQBqA")]
    pub to_address: String,
    /// On-chain transaction hash (once broadcast)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub tx_hash: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Affiliate commission for January")]
    pub description: Option<String>,
    /// Error reason (if status=Failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub error_reason: Option<String>,
    /// Who approved/cancelled (user_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub reviewed_by: Option<String>,
    /// When approval/cancellation happened
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub reviewed_at: Option<String>,
    /// ISO 8601 creation timestamp
    #[schema(example = "2026-02-27T08:30:00+00:00")]
    pub created_at: String,
    /// ISO 8601 completion timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(null), nullable)]
    pub completed_at: Option<String>,
    /// Sub-merchant code (None if belongs to parent merchant directly)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "shop_tokyo")]
    pub sub_merchant_code: Option<String>,
}

impl PayoutResponse {
    pub fn from_model(
        model: crate::entity::payouts::Model,
        livemode: bool,
        sub_merchant_code: Option<String>,
    ) -> Self {
        let currency = &model.currency;
        Self {
            id: model.id,
            livemode,
            status: model.status,
            amount: from_micro(model.amount, currency),
            fee: from_micro(model.fee, currency),
            net_amount: from_micro(model.net_amount, currency),
            currency: currency.clone(),
            network: model.network,
            to_address: model.to_address,
            tx_hash: model.tx_hash,
            description: model.description,
            error_reason: model.error_reason,
            reviewed_by: model.reviewed_by,
            reviewed_at: model.reviewed_at.map(|t| t.to_rfc3339()),
            created_at: model.created_at.to_rfc3339(),
            completed_at: model.completed_at.map(|t| t.to_rfc3339()),
            sub_merchant_code,
        }
    }
}
