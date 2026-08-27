//! Sub-Merchants entity — PSP → child org mapping
//!
//! Maps a PSP's sub-merchant code to a hidden merchant org (child_org_id).
//! The child org reuses all existing infrastructure (HD addresses, billing, etc.).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[serde(rename_all = "lowercase")]
pub enum SubMerchantStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "suspended")]
    Suspended,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sub_merchants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String, // sm_{uuid}
    pub parent_org_id: String,
    pub sub_merchant_code: String,
    pub display_name: String,
    pub child_org_id: String,
    pub status: SubMerchantStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::ParentOrgId",
        to = "super::merchants::Column::Id"
    )]
    ParentOrg,
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::ChildOrgId",
        to = "super::merchants::Column::Id"
    )]
    ChildOrg,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentOrg.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
