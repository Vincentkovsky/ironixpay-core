//! Withdrawals entity for merchant payouts

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub enum WithdrawalStatus {
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

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "withdrawals")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    pub environment: super::Environment,
    /// Blockchain network for this withdrawal (e.g. "TRON", "BSC")
    pub network: String,
    /// Requested withdrawal amount (USDT microunits)
    pub amount: i64,
    /// Network fee deducted (USDT microunits), configurable, default 1 USDT
    pub network_fee: i64,
    /// Actual payout = amount - network_fee
    pub net_amount: i64,
    /// Merchant's collection address
    pub to_address: String,
    pub status: WithdrawalStatus,
    pub tx_hash: Option<String>,
    pub error_reason: Option<String>,
    pub currency: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
    /// User ID of who initiated the withdrawal (for self-approval prevention)
    pub requested_by: Option<String>,
    /// User ID of the person who approved/cancelled this withdrawal
    pub reviewed_by: Option<String>,
    /// When the withdrawal was approved/cancelled
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
