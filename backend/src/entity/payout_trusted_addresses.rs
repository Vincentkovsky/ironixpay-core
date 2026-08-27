//! Payout Trusted Addresses entity
//!
//! Tracks addresses that have been successfully used for payouts/withdrawals.
//! Used by the risk control system to skip approval for known addresses.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payout_trusted_addresses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    /// Blockchain network (e.g., "TRON", "BSC")
    pub network: String,
    /// Destination address (case-normalized: EVM lowercase, TRON as-is)
    pub address: String,
    /// When this address was first successfully used
    pub first_used_at: DateTimeWithTimeZone,
    /// When this address was most recently used
    pub last_used_at: DateTimeWithTimeZone,
    /// Total number of successful payouts/withdrawals to this address
    pub total_payouts: i32,
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
