//! Billing Logs entity
//! Aligned with docs/system_design.md

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum BillingType {
    #[sea_orm(string_value = "payment_credit")]
    PaymentCredit,
    #[sea_orm(string_value = "withdrawal")]
    Withdrawal,
    #[sea_orm(string_value = "refund")]
    Refund,
    #[sea_orm(string_value = "payout")]
    Payout,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Deserialize)]
#[sea_orm(table_name = "billing_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Environment (Production or Sandbox)
    pub environment: super::Environment,
    /// Blockchain network for this billing entry (e.g. "TRON", "BSC")
    pub network: String,
    pub merchant_id: String,
    pub session_id: Option<String>,
    /// External reference ID.
    /// - For `PaymentCredit`: Incoming transaction hash or session ID.
    /// - For `Withdrawal`: Withdrawal record ID (wd_xxx).
    /// - For `Refund`: Exception ID or refund transaction hash.
    pub external_ref_id: Option<String>,
    #[sea_orm(column_name = "type")]
    pub billing_type: BillingType,
    pub previous_balance: i64,
    pub amount_change: i64,
    pub balance_after: i64,
    pub description: Option<String>,
    /// Token symbol for this billing entry ("USDT" or "USDC")
    pub token: String,
    /// Gross amount before fee deduction (microunits). Used for agent commission calculation.
    pub gross_amount: Option<i64>,
    /// Actual fee charged (microunits). Used for agent commission calculation.
    pub fee_amount: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::MerchantId",
        to = "super::merchants::Column::Id"
    )]
    Merchant,
    #[sea_orm(
        belongs_to = "super::checkout_sessions::Entity",
        from = "Column::SessionId",
        to = "super::checkout_sessions::Column::Id"
    )]
    CheckoutSession,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl Related<super::checkout_sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CheckoutSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
