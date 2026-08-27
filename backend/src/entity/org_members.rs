//! Organization Members entity — links users to organizations with roles
//! Part of Role & Organization feature

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    #[sea_orm(string_value = "owner")]
    Owner,
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "developer")]
    Developer,
    #[sea_orm(string_value = "finance")]
    Finance,
    #[sea_orm(string_value = "viewer")]
    Viewer,
}

impl std::str::FromStr for MemberRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "developer" => Ok(Self::Developer),
            "finance" => Ok(Self::Finance),
            "viewer" => Ok(Self::Viewer),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[serde(rename_all = "lowercase")]
pub enum MemberStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "disabled")]
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "org_members")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub org_id: String,
    /// NULLABLE: pending invitations have no user yet
    pub user_id: Option<String>,
    /// Email of the invited person (for pending invitations)
    pub invited_email: Option<String>,
    pub role: MemberRole,
    pub invited_by: Option<String>,
    pub invited_at: Option<DateTimeWithTimeZone>,
    pub accepted_at: Option<DateTimeWithTimeZone>,
    pub status: MemberStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::OrgId",
        to = "super::merchants::Column::Id"
    )]
    Organization,

    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,

    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::InvitedBy",
        to = "super::users::Column::Id"
    )]
    InvitedByUser,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

// Related<users::Entity> using the User relation (user_id column).
// Note: org_members has two FK columns to users (user_id and invited_by).
// This impl uses user_id as the canonical relation for SeaORM's has_many/Related.
// For invited_by queries, use Relation::InvitedByUser.def() explicitly.
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
