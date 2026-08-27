//! Payout Settings entity — Organization-level risk control configuration
//!
//! 1:1 with merchants table. Lazy-initialized on first update.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payout_settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub merchant_id: String,

    /// Whether first-time addresses require approval
    pub require_new_address_approval: bool,
    /// Amount threshold (USDT microunits) above which approval is required. 0 = no limit.
    pub approval_threshold: i64,
    /// JSON array of role names that can approve (e.g., ["owner","admin"])
    pub approver_roles: serde_json::Value,

    /// Auto-withdraw master switch
    pub auto_withdraw_enabled: bool,
    /// Auto-withdraw balance threshold (USDT microunits)
    pub auto_withdraw_threshold: Option<i64>,
    /// Target network for auto-withdraw
    pub auto_withdraw_network: Option<String>,
    /// Currency for auto-withdraw (default: USDT)
    pub auto_withdraw_currency: String,

    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    /// Check if a given role string is in the approver_roles list.
    pub fn is_approver_role(&self, role: &str) -> bool {
        self.approver_roles
            .as_array()
            .map(|arr| arr.iter().any(|v| v.as_str() == Some(role)))
            .unwrap_or(false)
    }
}

/// Default settings returned when no payout_settings row exists for a merchant.
impl Default for Model {
    fn default() -> Self {
        Self {
            id: String::new(),
            merchant_id: String::new(),
            require_new_address_approval: true,
            approval_threshold: 5_000_000_000, // 5000 USDT
            approver_roles: serde_json::json!(["owner", "admin"]),
            auto_withdraw_enabled: false,
            auto_withdraw_threshold: None,
            auto_withdraw_network: None,
            auto_withdraw_currency: "USDT".to_string(),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }
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
