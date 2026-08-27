//! Xero Sync Logs entity
//!
//! Tracks per-session sync status to Xero. Supports idempotent retry with
//! checkpoint recovery (xero_invoice_id / xero_payment_id saved immediately).

use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum XeroSyncStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "synced")]
    Synced,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "skipped")]
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "xero_sync_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub connection_id: Uuid,
    pub session_id: String,

    pub xero_invoice_id: Option<String>,
    pub xero_payment_id: Option<String>,

    pub status: XeroSyncStatus,
    pub attempt_count: i32,
    #[sea_orm(column_type = "Text")]
    pub last_error: Option<String>,
    pub next_retry_at: Option<DateTimeWithTimeZone>,
    pub fx_rate: Option<Decimal>,
    pub fx_source_currency: Option<String>,
    pub fx_target_currency: Option<String>,
    pub converted_gross: Option<Decimal>,
    pub converted_fee: Option<Decimal>,
    pub converted_net: Option<Decimal>,

    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::xero_connections::Entity",
        from = "Column::ConnectionId",
        to = "super::xero_connections::Column::Id"
    )]
    XeroConnection,
}

impl Related<super::xero_connections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::XeroConnection.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Display for XeroSyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Synced => write!(f, "synced"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}
