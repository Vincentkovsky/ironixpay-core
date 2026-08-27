//! Webhook Endpoints entity
//! Aligned with docs/system_design.md
//!
//! Uses `environment` (sandbox/production) instead of `network` because
//! merchants typically have only 2 URLs - one for dev, one for prod.
//! They don't configure different URLs per blockchain.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export Environment from network module for use in this entity
pub use super::network::Environment;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum EndpointStatus {
    #[sea_orm(string_value = "enabled")]
    Enabled,
    #[sea_orm(string_value = "disabled")]
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_endpoints")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    /// Environment this endpoint receives events for (sandbox or production)
    pub environment: Environment,
    pub url: String,
    pub description: Option<String>,
    pub secret_encrypted: String,
    pub status: EndpointStatus,
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
    #[sea_orm(has_many = "super::webhook_events::Entity")]
    WebhookEvents,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl Related<super::webhook_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WebhookEvents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
