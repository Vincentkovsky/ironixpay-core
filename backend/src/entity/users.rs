//! Users entity — login identity, extracted from merchants
//! Part of Role & Organization feature

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: String,
    pub name: String,
    pub totp_secret: Option<String>,
    pub is_totp_enabled: bool,
    /// Token version for JWT revocation. Increment to invalidate all existing tokens.
    pub token_version: i32,
    /// JSON array of backup codes: [{"hash": "sha256", "used": bool}]
    pub backup_codes: Option<String>,
    pub email_verified: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "super::org_members::Entity",
        from = "Column::Id",
        to = "super::org_members::Column::UserId"
    )]
    OrgMembers,
}

impl Related<super::org_members::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrgMembers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
