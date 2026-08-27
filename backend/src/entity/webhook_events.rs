//! Webhook Events entity
//! Aligned with docs/system_design.md

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum WebhookEventStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processing")]
    Processing,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "giving_up")]
    GivingUp,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Network identifier (e.g., TRON, BSC)
    pub network: String,
    pub endpoint_id: String,
    /// Snapshot of the endpoint URL at the time of event creation
    pub target_url: String,
    /// Source resource ID (e.g., cs_xxx for checkout, po_xxx for payout, wd_xxx for withdrawal)
    pub source_id: String,
    pub merchant_id: String,
    pub event_type: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub payload: serde_json::Value,
    pub status: WebhookEventStatus,
    pub http_status_code: Option<i32>,
    pub attempt_count: i32,
    pub last_attempt_at: Option<DateTimeWithTimeZone>,
    pub next_retry_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::webhook_endpoints::Entity",
        from = "Column::EndpointId",
        to = "super::webhook_endpoints::Column::Id"
    )]
    WebhookEndpoint,
}

impl Related<super::webhook_endpoints::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WebhookEndpoint.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Session event type constants
pub const EVENT_SESSION_COMPLETED: &str = "session.completed";
/// Session expired, may or may not have payment
pub const EVENT_SESSION_EXPIRED: &str = "session.expired";
/// Triggered when a session is blocked due to AML/compliance risk
pub const EVENT_SESSION_BLOCKED: &str = "session.blocked";
/// Triggered when a Resolution operation (Accept/Attach) modifies a session
pub const EVENT_SESSION_RESOLVED: &str = "session.resolved";

// Payout event type constants
pub const EVENT_PAYOUT_COMPLETED: &str = "payout.completed";
pub const EVENT_PAYOUT_FAILED: &str = "payout.failed";

// Withdrawal event type constants
pub const EVENT_WITHDRAWAL_COMPLETED: &str = "withdrawal.completed";
pub const EVENT_WITHDRAWAL_FAILED: &str = "withdrawal.failed";
