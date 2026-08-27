//! Addresses entity
//! Aligned with docs/system_design.md
//! Composite PK: (network, address)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum AddressStatus {
    #[sea_orm(string_value = "Idle")]
    Idle,
    #[sea_orm(string_value = "Assigned")]
    Assigned,
    #[sea_orm(string_value = "Detected")]
    Detected,
    #[sea_orm(string_value = "Sweeping")]
    Sweeping,
    #[sea_orm(string_value = "Cooling")]
    Cooling,
    #[sea_orm(string_value = "Locked")]
    Locked,
    #[sea_orm(string_value = "Error")]
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "addresses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub network: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String,
    pub merchant_id: String,
    pub path_index: i32,
    pub native_balance: i64,
    pub usdt_balance: i64,
    pub usdc_balance: i64,
    pub status: AddressStatus,
    pub error_reason: Option<String>,
    pub sweep_attempts: i32,
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
