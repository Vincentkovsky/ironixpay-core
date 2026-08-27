//! Xero OAuth states entity
//!
//! Stores one-time OAuth nonces with expiry/consumed timestamps so callback
//! validation remains safe across restarts and multiple backend instances.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use super::network::Environment;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "xero_oauth_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub nonce: String,
    pub merchant_id: String,
    pub environment: Environment,
    pub expires_at: DateTimeWithTimeZone,
    pub consumed_at: Option<DateTimeWithTimeZone>,
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
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
