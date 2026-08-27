//! Xero Connections entity
//!
//! Stores OAuth credentials and sync configuration for Xero accounting integration.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use super::network::Environment;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum XeroConnectionStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "pending_selection")]
    PendingSelection,
    #[sea_orm(string_value = "disconnected")]
    Disconnected,
    #[sea_orm(string_value = "error")]
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "xero_connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub merchant_id: String,
    pub environment: Environment,

    pub access_token_encrypted: String,
    pub refresh_token_encrypted: String,
    pub token_expires_at: DateTimeWithTimeZone,

    pub xero_tenant_id: String,
    pub xero_tenant_name: Option<String>,

    pub xero_account_code: Option<String>,
    pub xero_fee_account_code: Option<String>,
    pub xero_payment_account_code: Option<String>,
    pub xero_tax_type: String,
    pub xero_contact_id: Option<String>,
    pub default_currency: String,
    pub auto_sync_enabled: bool,

    pub status: XeroConnectionStatus,
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
    #[sea_orm(has_many = "super::xero_sync_logs::Entity")]
    XeroSyncLogs,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl Related<super::xero_sync_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::XeroSyncLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Display for XeroConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::PendingSelection => write!(f, "pending_selection"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Error => write!(f, "error"),
        }
    }
}
