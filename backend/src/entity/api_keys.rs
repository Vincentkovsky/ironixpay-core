//! API Keys entity
//! Aligned with docs/system_design.md
//!
//! Note: Environment is derived from the key_prefix (sk_test_* vs sk_live_*),
//! NOT stored as a separate field. This supports future cross-chain expansion.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    pub name: Option<String>,
    /// Prefix determines environment: sk_test_* = Sandbox, sk_live_* = Production
    pub key_prefix: String,
    pub key_hash: String,
    pub is_active: bool,
    /// Which user created this key (NULL for keys created before multi-user support)
    pub created_by_user_id: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub last_used_at: Option<DateTimeWithTimeZone>,
}

impl Model {
    /// Check if this is a test/sandbox API key
    pub fn is_test_key(&self) -> bool {
        self.key_prefix.starts_with("sk_test_")
    }

    /// Check if this is a live/production API key
    pub fn is_live_key(&self) -> bool {
        self.key_prefix.starts_with("sk_live_")
    }
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
        belongs_to = "super::users::Entity",
        from = "Column::CreatedByUserId",
        to = "super::users::Column::Id"
    )]
    CreatedByUser,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
