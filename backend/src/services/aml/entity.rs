//! SeaORM entities for AML tables

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// AML Blacklist - OFAC/sanctions addresses
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "aml_blacklist")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String,
    pub source: String,
    pub risk_level: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// AML API Cache - GoPlus query results
pub mod api_cache {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "aml_api_cache")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub address: String,
        pub is_risky: bool,
        pub risk_reason: Option<String>,
        pub checked_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
