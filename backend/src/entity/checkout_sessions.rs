//! Checkout Sessions entity
//! Aligned with docs/system_design.md

use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use ts_rs::TS;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub enum SessionStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Underpaid")]
    Underpaid,
    #[sea_orm(string_value = "Paid")]
    Paid,
    #[sea_orm(string_value = "Overpaid")]
    Overpaid,
    #[sea_orm(string_value = "Expired")]
    Expired,
    /// AML risk detected - funds blocked from settlement
    #[sea_orm(string_value = "Blocked")]
    Blocked,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub enum SettlementStatus {
    #[sea_orm(string_value = "Unsettled")]
    Unsettled,
    #[sea_orm(string_value = "Settling")]
    Settling,
    #[sea_orm(string_value = "Settled")]
    Settled,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

impl SessionStatus {
    /// Determine session status based on expected vs received amounts
    ///
    /// This is the centralized logic for payment status determination.
    /// All services should use this method to ensure consistency.
    ///
    /// # Arguments
    /// * `amount_expected` - Expected payment amount in minimal units
    /// * `amount_received` - Actual received amount in minimal units
    /// * `underpayment_threshold` - Tolerance for minor underpayments (e.g., 0.1 USDT = 100000)
    ///
    /// # Returns
    /// The appropriate `SessionStatus` based on the payment amount
    pub fn determine_by_amount(
        amount_expected: i64,
        amount_received: i64,
        underpayment_threshold: i64,
    ) -> Self {
        let diff = amount_expected - amount_received;

        if diff <= 0 {
            if amount_received > amount_expected {
                Self::Overpaid
            } else {
                Self::Paid
            }
        } else if diff <= underpayment_threshold {
            // Within tolerance - treat as paid
            Self::Paid
        } else {
            Self::Underpaid
        }
    }

    /// Check if this status is a terminal (final) state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Paid | Self::Overpaid | Self::Expired | Self::Blocked
        )
    }

    /// Check if this status indicates successful payment
    pub fn is_successful(&self) -> bool {
        matches!(self, Self::Paid | Self::Overpaid)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    pub network: String,
    pub pay_address: String,
    pub client_reference_id: Option<String>,
    pub currency: String,
    pub currency_contract: String,
    pub amount_expected: i64,
    pub amount_received: i64,
    pub status: SessionStatus,
    pub settlement_status: SettlementStatus,
    pub settlement_tx_hash: Option<String>,
    /// Redirect URL for successful payment (optional — API-only integrations may omit)
    pub success_url: Option<String>,
    /// Redirect URL for cancelled/expired session (optional)
    pub cancel_url: Option<String>,
    pub expires_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    /// Fee charged by platform (USDT microunits), set on payment confirmation
    pub fee_amount: Option<i64>,
    /// Net amount credited to merchant (USDT microunits) = amount_received - fee_amount
    pub net_amount: Option<i64>,
    /// Pricing currency code (crypto: "USDT"/"USDC", fiat: "USD"/"CNY" etc.)
    pub pricing_currency: String,
    /// Original amount in pricing currency (e.g., 10.50 for $10.50 or 10.50 USDT)
    #[sea_orm(column_type = "Decimal(Some((18, 8)))")]
    pub pricing_amount: Decimal,
    /// Exchange rate at session creation (1 crypto = N fiat; 1.0 for crypto→crypto)
    #[sea_orm(column_type = "Decimal(Some((18, 8)))")]
    pub exchange_rate: Decimal,
    /// Sub-merchant code (only set for sessions created via PSP context switch)
    pub sub_merchant_code: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::MerchantId",
        to = "super::merchants::Column::Id"
    )]
    Merchant,
    #[sea_orm(has_many = "super::transactions::Entity")]
    Transactions,
    #[sea_orm(has_many = "super::billing_logs::Entity")]
    BillingLogs,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl Related<super::transactions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transactions.def()
    }
}

impl Related<super::billing_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillingLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
