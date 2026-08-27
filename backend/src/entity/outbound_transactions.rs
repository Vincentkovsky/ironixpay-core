//! Canonical journal for every signed outbound blockchain transaction.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum OutboundState {
    #[sea_orm(string_value = "Preparing")]
    Preparing,
    #[sea_orm(string_value = "Signed")]
    Signed,
    #[sea_orm(string_value = "BroadcastUnknown")]
    BroadcastUnknown,
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Confirmed")]
    Confirmed,
    #[sea_orm(string_value = "Reverted")]
    Reverted,
    #[sea_orm(string_value = "Expired")]
    Expired,
    #[sea_orm(string_value = "Replaced")]
    Replaced,
    #[sea_orm(string_value = "Failed")]
    Failed,
    #[sea_orm(string_value = "Stuck")]
    Stuck,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum OutboundOperationType {
    #[sea_orm(string_value = "auto_sweep")]
    AutoSweep,
    #[sea_orm(string_value = "manual_sweep")]
    ManualSweep,
    #[sea_orm(string_value = "manual_transfer")]
    ManualTransfer,
    #[sea_orm(string_value = "payout")]
    Payout,
    #[sea_orm(string_value = "withdrawal")]
    Withdrawal,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum OutboundPurpose {
    #[sea_orm(string_value = "token_transfer")]
    TokenTransfer,
    #[sea_orm(string_value = "gas_funding")]
    GasFunding,
    #[sea_orm(string_value = "energy_funding")]
    EnergyFunding,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "outbound_transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,
    pub environment: super::Environment,
    pub session_id: Option<String>,
    pub exception_id: Option<String>,
    pub payout_id: Option<String>,
    pub withdrawal_id: Option<String>,
    pub parent_transaction_id: Option<String>,
    pub operation_type: OutboundOperationType,
    pub purpose: OutboundPurpose,
    pub network: String,
    pub from_address: String,
    pub to_address: String,
    pub energy_delegate_tx_hash: Option<String>,
    pub funding_tx_hash: Option<String>,
    pub provider_reference: Option<String>,
    pub tx_hash: Option<String>,
    pub amount: i64,
    pub state: OutboundState,
    pub token: String,
    pub cost_in_usdt: Option<i64>,
    pub signed_payload_encrypted: Option<String>,
    pub nonce: Option<i64>,
    pub expires_at: Option<DateTimeWithTimeZone>,
    pub last_valid_block_height: Option<i64>,
    pub broadcast_attempts: i32,
    pub last_broadcast_at: Option<DateTimeWithTimeZone>,
    pub observed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
    pub error_message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
