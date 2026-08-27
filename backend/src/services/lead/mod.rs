//! Enterprise lead intake service.

use std::sync::Arc;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

use crate::entity::enterprise_leads::{self, LeadNotificationStatus, LeadStatus};
use crate::services::email::{EmailSender, EnterpriseLeadNotification};

#[derive(Debug, Clone)]
pub struct LeadSubmission {
    pub company_name: String,
    pub company_website: Option<String>,
    pub contact_email: String,
    pub telegram: Option<String>,
    pub business_type: String,
    pub monthly_volume: String,
    pub networks: Vec<String>,
    pub integration_needs: Vec<String>,
    pub message: Option<String>,
    pub locale: String,
}

#[derive(Debug)]
pub struct LeadService {
    db: DatabaseConnection,
    email_sender: Arc<dyn EmailSender>,
    notification_email: String,
}

impl LeadService {
    pub fn new(
        db: DatabaseConnection,
        email_sender: Arc<dyn EmailSender>,
        notification_email: String,
    ) -> Self {
        Self {
            db,
            email_sender,
            notification_email,
        }
    }

    pub async fn create(
        &self,
        submission: LeadSubmission,
    ) -> Result<enterprise_leads::Model, sea_orm::DbErr> {
        let now = Utc::now().fixed_offset();
        let id = format!("lead_{}", Uuid::new_v4().simple());

        enterprise_leads::ActiveModel {
            id: Set(id),
            company_name: Set(submission.company_name),
            company_website: Set(submission.company_website),
            contact_email: Set(submission.contact_email),
            telegram: Set(submission.telegram),
            business_type: Set(submission.business_type),
            monthly_volume: Set(submission.monthly_volume),
            networks: Set(json!(submission.networks)),
            integration_needs: Set(json!(submission.integration_needs)),
            message: Set(submission.message),
            locale: Set(submission.locale),
            source: Set("website_enterprise".to_string()),
            status: Set(LeadStatus::New),
            notification_status: Set(LeadNotificationStatus::Pending),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await
    }

    pub async fn notify(&self, lead: enterprise_leads::Model) {
        let notification = EnterpriseLeadNotification {
            lead_id: lead.id.clone(),
            company_name: lead.company_name.clone(),
            company_website: lead.company_website.clone(),
            contact_email: lead.contact_email.clone(),
            telegram: lead.telegram.clone(),
            business_type: lead.business_type.clone(),
            monthly_volume: lead.monthly_volume.clone(),
            networks: json_array_to_strings(&lead.networks),
            integration_needs: json_array_to_strings(&lead.integration_needs),
            message: lead.message.clone(),
            locale: lead.locale.clone(),
            submitted_at: lead.created_at.to_rfc3339(),
        };

        let status = match self
            .email_sender
            .send_enterprise_lead_notification(&self.notification_email, &notification)
            .await
        {
            Ok(()) => {
                info!(lead_id = %lead.id, "Enterprise lead notification sent");
                LeadNotificationStatus::Sent
            }
            Err(error) => {
                warn!(lead_id = %lead.id, error = %error, "Enterprise lead notification failed");
                LeadNotificationStatus::Failed
            }
        };

        if let Ok(Some(model)) = enterprise_leads::Entity::find_by_id(&lead.id)
            .one(&self.db)
            .await
        {
            let mut active: enterprise_leads::ActiveModel = model.into();
            active.notification_status = Set(status);
            active.updated_at = Set(Utc::now().fixed_offset());
            if let Err(error) = active.update(&self.db).await {
                warn!(lead_id = %lead.id, error = %error, "Failed to update lead notification status");
            }
        }
    }
}

fn json_array_to_strings(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
