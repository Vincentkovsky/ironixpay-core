//! Agent Profiles entity
//!
//! Tracks merchants promoted to agent status. Agents earn commission
//! (fee spread) from merchants they refer.

use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// The merchant who is an agent (1:1)
    pub merchant_id: String,
    /// Unique referral code for sharing
    pub referral_code: String,
    /// IronixPay base rate (agent's wholesale price), e.g. 0.0010 = 0.1%
    #[sea_orm(column_type = "Decimal(Some((5, 4)))")]
    pub base_rate: Decimal,
    /// Maximum markup the agent can add, e.g. 0.0100 = 1.0%
    #[sea_orm(column_type = "Decimal(Some((5, 4)))")]
    pub max_markup: Decimal,
    /// Default fee rate set for newly referred merchants, e.g. 0.0080 = 0.8%
    #[sea_orm(column_type = "Decimal(Some((5, 4)))")]
    pub default_merchant_rate: Decimal,
    /// active | suspended
    pub status: String,
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
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
