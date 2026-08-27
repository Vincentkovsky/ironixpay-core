//! Exchange Rates entity — latest rate cache from CoinGecko (UPSERT pattern)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "exchange_rates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub crypto: String,
    pub fiat: String,
    #[sea_orm(column_type = "Decimal(Some((18, 8)))")]
    pub rate: Decimal,
    pub source: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
