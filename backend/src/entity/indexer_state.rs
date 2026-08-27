//! Indexer State entity
//!
//! Persists the last processed block number for each network,
//! ensuring block scanning resumes correctly after service restart.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "indexer_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub network: String,
    pub last_processed_block: i64,
    pub chain_head_block: Option<i64>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
