//! Payment Events entity (Transactional Outbox)
//!
//! Implements the Transactional Outbox pattern for decoupling
//! TransactionIndexer from CheckoutService.
//!
//! Indexer writes to this table atomically with transactions,
//! and a background processor consumes events to update session status.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Payment event status for outbox processing
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PaymentEventStatus {
    /// Event is waiting to be processed
    #[sea_orm(string_value = "pending")]
    Pending,
    /// Event is currently being processed by a worker
    #[sea_orm(string_value = "processing")]
    Processing,
    /// Event has been successfully processed
    #[sea_orm(string_value = "processed")]
    Processed,
    /// Event failed after max retries (dead letter)
    #[sea_orm(string_value = "failed")]
    Failed,
}

/// Payment event types
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PaymentEventType {
    /// Transaction detected but not yet confirmed
    #[sea_orm(string_value = "payment_detected")]
    PaymentDetected,
    /// Transaction has reached required confirmations
    #[sea_orm(string_value = "payment_confirmed")]
    PaymentConfirmed,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Event type (payment_detected, payment_confirmed)
    pub event_type: PaymentEventType,

    /// Associated checkout session ID
    pub session_id: String,

    /// Network identifier (e.g. TRON, BSC)
    pub tx_network: String,

    /// Transaction hash
    pub tx_hash: String,

    /// Log index within the transaction (for batch transfers)
    pub tx_log_index: i32,

    /// Payment amount in minimal units (USDT: 10^6)
    pub amount: i64,

    /// Processing status
    pub status: PaymentEventStatus,

    /// Number of processing attempts
    pub attempt_count: i32,

    /// Next retry time (for exponential backoff)
    pub next_retry_at: DateTimeWithTimeZone,

    /// Error message if processing failed
    #[sea_orm(column_type = "Text")]
    pub error_message: Option<String>,

    /// Event creation time
    pub created_at: DateTimeWithTimeZone,

    /// Last status update time
    pub updated_at: DateTimeWithTimeZone,

    /// Time when event was successfully processed
    pub processed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::checkout_sessions::Entity",
        from = "Column::SessionId",
        to = "super::checkout_sessions::Column::Id"
    )]
    CheckoutSession,
}

impl Related<super::checkout_sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CheckoutSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Maximum number of retry attempts before moving to dead letter
pub const MAX_RETRY_ATTEMPTS: i32 = 7;

/// Retry delays in seconds (exponential backoff with jitter)
pub const RETRY_DELAYS_SECS: [u64; 7] = [2, 4, 8, 16, 32, 64, 128];
