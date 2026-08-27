//! Merchant Chain Accounts
//!
//! Stores network-specific assets and keys.
//!
//! Key responsibility:
//! - Extended Public Keys (xPubs) per chain
//! - Address derivation counters
//! - Collection addresses per chain

use super::network::{Environment, Network};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "merchant_chain_accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub merchant_id: String,

    #[sea_orm(primary_key, auto_increment = false)]
    pub environment: Environment,

    #[sea_orm(primary_key, auto_increment = false)]
    pub network: Network,

    /// Extended Public Key (xPub) for this specific chain/network.
    /// Encrypted with KMS.
    #[sea_orm(column_type = "Text")]
    pub xpub_encrypted: String,

    /// HD Wallet path index counter for this chain.
    #[sea_orm(default_value = 0)]
    pub last_path_index: i32,

    /// Merchant's withdrawal/payout destination address for this chain.
    /// Named "collection" historically — this is where the merchant COLLECTS payouts,
    /// NOT the HD-derived addresses where customers pay.
    pub collection_address: Option<String>,

    /// Per-chain available balance in USDT microunits (6 decimals).
    /// Credited on payment confirmation, debited on withdrawal.
    #[sea_orm(default_value = 0)]
    pub usdt_balance: i64,

    /// Per-chain available balance in USDC microunits (6 decimals).
    #[sea_orm(default_value = 0)]
    pub usdc_balance: i64,

    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::MerchantId",
        to = "super::merchants::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Merchant,
    // Note: No direct relation to merchants table needed here — queries use
    // the parent is Merchant. (Profile is a sibling).
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
