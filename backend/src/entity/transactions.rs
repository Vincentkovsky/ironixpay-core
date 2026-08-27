//! Transactions entity
//! Aligned with docs/system_design.md
//! Composite PK: (network, tx_hash, log_index)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 链上交易状态机
/// 统一了 EVM/Tron/Solana 的状态流转
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ChainTxState {
    /// **查无此人**
    /// 含义：节点查不到该 Hash。可能未广播成功、被丢弃或 Expired。
    #[sea_orm(string_value = "not_found")]
    NotFound,

    /// **处理中/待打包** (Mempool)
    /// 含义：节点已通过广播，在内存池中等待打包。
    #[sea_orm(string_value = "pending")]
    Pending,

    /// **已进块/不安全** (Unconfirmed)
    /// 含义：已上链，但确认数不足 (Confirmations < N)。
    #[sea_orm(string_value = "unconfirmed")]
    Unconfirmed,

    /// **已固化/安全** (Finalized)
    /// 含义：确认数足够，不可逆。
    #[sea_orm(string_value = "confirmed")]
    Confirmed,

    /// **链上失败** (Reverted)
    /// 含义：交易已上链但执行失败（如 Gas 不足、合约 Revert）。
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub network: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tx_hash: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub log_index: i32,
    pub session_id: Option<String>,
    pub merchant_id: String,
    pub currency_symbol: String,
    pub currency_contract: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: i64,
    pub status: ChainTxState,
    pub confirmations_count: i32,
    pub block_number: i64,
    pub block_timestamp: DateTimeWithTimeZone,
    /// Whether this transaction's amount has been credited to the session.
    /// Used for idempotent payment processing - ensures each tx is only counted once.
    pub is_credited: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::checkout_sessions::Entity",
        from = "Column::SessionId",
        to = "super::checkout_sessions::Column::Id"
    )]
    CheckoutSession,
}

impl Related<super::checkout_sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CheckoutSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
