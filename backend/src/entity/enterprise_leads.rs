//! Enterprise lead entity
//!
//! Public website inquiries are stored before Resend notification so delivery
//! failures cannot discard a lead.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum LeadStatus {
    #[sea_orm(string_value = "new")]
    New,
    #[sea_orm(string_value = "contacted")]
    Contacted,
    #[sea_orm(string_value = "qualified")]
    Qualified,
    #[sea_orm(string_value = "closed")]
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum LeadNotificationStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "sent")]
    Sent,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "enterprise_leads")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub company_name: String,
    pub company_website: Option<String>,
    pub contact_email: String,
    pub telegram: Option<String>,
    pub business_type: String,
    pub monthly_volume: String,
    pub networks: Json,
    pub integration_needs: Json,
    #[sea_orm(column_type = "Text")]
    pub message: Option<String>,
    pub locale: String,
    pub source: String,
    pub status: LeadStatus,
    pub notification_status: LeadNotificationStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
