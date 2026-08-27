//! Payment Exceptions entity
//!
//! Tracks abnormal payments that require manual intervention:
//! - Late payments (session expired)
//! - Payments to idle addresses (no active session)
//! - Underpayments below threshold
//! - Payments after session completed

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Type of payment exception
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ExceptionType {
    /// Payment received after session expired
    #[sea_orm(string_value = "session_expired")]
    SessionExpired,
    /// Payment to idle address with no active session
    #[sea_orm(string_value = "no_active_session")]
    NoActiveSession,
    /// Payment after session already completed/paid
    #[sea_orm(string_value = "session_already_completed")]
    SessionAlreadyCompleted,
    /// Payment amount below dust threshold
    #[sea_orm(string_value = "dust_payment")]
    DustPayment,
    /// Session expired with partial payment (Underpaid → Expired)
    #[sea_orm(string_value = "underpaid_expired")]
    UnderpaidExpired,
    /// AML risk detected - payment from blacklisted or high-risk address
    #[sea_orm(string_value = "risk_blocked")]
    RiskBlocked,
    /// Wrong token: payment currency doesn't match session's expected currency
    #[sea_orm(string_value = "wrong_token")]
    WrongToken,
    /// Other unknown exception
    #[sea_orm(string_value = "unknown")]
    Unknown,
}

/// Status of the exception lifecycle
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ExceptionStatus {
    /// Awaiting manual review
    #[sea_orm(string_value = "Pending")]
    Pending,
    /// In progress (e.g. broadcasting transaction)
    #[sea_orm(string_value = "Processing")]
    Processing,
    /// Resolved (operation completed or auto-ignored)
    #[sea_orm(string_value = "Resolved")]
    Resolved,
    /// Resolution failed (e.g. broadcast failed)
    #[sea_orm(string_value = "Failed")]
    Failed,
}

/// Resolution action taken
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum Resolution {
    /// Accepted expired session (accept_expired_session)
    #[sea_orm(string_value = "Accepted")]
    Accepted,
    /// Bound to a specific session (attach_session)
    #[sea_orm(string_value = "Attached")]
    Attached,
    /// Funds transferred/refunded (manual_transfer)
    #[sea_orm(string_value = "Transferred")]
    Transferred,
    /// Automatically or manually ignored (e.g. dust)
    #[sea_orm(string_value = "Ignored")]
    Ignored,
    /// Manually swept to merchant balance (manual_sweep)
    #[sea_orm(string_value = "Swept")]
    Swept,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_exceptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub network: String,
    pub tx_hash: String,
    pub log_index: i32,
    pub exception_type: ExceptionType,
    pub to_address: String,
    pub from_address: String,
    pub amount: i64,
    pub currency_symbol: String,
    /// Merchant who owns this address (if known)
    pub merchant_id: Option<String>,
    /// Original session ID (if applicable)
    pub session_id: Option<String>,
    pub block_number: i64,
    pub block_timestamp: DateTimeWithTimeZone,
    pub status: ExceptionStatus,
    /// Resolution action taken
    pub resolution: Option<Resolution>,
    /// Optional reference ID for resolution (e.g. Refund TX Hash, Target Session ID)
    pub resolution_ref_id: Option<String>,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    /// Operator who resolved the exception
    pub resolved_by: Option<String>,
    /// Additional notes for audit trail
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
