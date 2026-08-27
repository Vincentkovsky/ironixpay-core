//! Payouts entity
//!
//! Merchant-to-end-user payouts via Public API.
//! Separate from withdrawals (merchant self-withdrawal via Dashboard).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PayoutStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
    #[sea_orm(string_value = "PendingApproval")]
    PendingApproval,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
    #[sea_orm(string_value = "ApprovalExpired")]
    ApprovalExpired,
}

impl std::fmt::Display for PayoutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayoutStatus::Pending => write!(f, "Pending"),
            PayoutStatus::Processing => write!(f, "Processing"),
            PayoutStatus::Completed => write!(f, "Completed"),
            PayoutStatus::Failed => write!(f, "Failed"),
            PayoutStatus::PendingApproval => write!(f, "PendingApproval"),
            PayoutStatus::Cancelled => write!(f, "Cancelled"),
            PayoutStatus::ApprovalExpired => write!(f, "ApprovalExpired"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payouts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    pub environment: super::Environment,
    pub network: String,
    pub to_address: String,
    /// Gross amount requested (USDT microunits)
    pub amount: i64,
    /// Platform fee (USDT microunits)
    pub fee: i64,
    /// Net amount sent on-chain = amount - fee (USDT microunits)
    pub net_amount: i64,
    pub status: PayoutStatus,
    pub tx_hash: Option<String>,
    pub error_reason: Option<String>,
    pub idempotency_key: String,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub currency: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
    /// User ID of the person who approved/cancelled this payout
    pub reviewed_by: Option<String>,
    /// When the payout was approved/cancelled
    pub reviewed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::MerchantId",
        to = "super::merchants::Column::Id"
    )]
    Merchant,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
