//! Dummy Email Sender
//!
//! Logs emails to stdout/logs instead of sending them.
//! Useful for development and testing.

use super::{EmailSender, EnterpriseLeadNotification};
use anyhow::Result;
use async_trait::async_trait;
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct DummyEmailService;

#[async_trait]
impl EmailSender for DummyEmailService {
    async fn send_verification_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        verification_token: &str,
    ) -> Result<()> {
        info!(
            to = %to_email,
            name = %merchant_name,
            token = %verification_token,
            "[DUMMY] Verification email SENT (simulated)"
        );
        Ok(())
    }

    async fn send_password_reset_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        reset_token: &str,
    ) -> Result<()> {
        info!(
            to = %to_email,
            name = %merchant_name,
            token = %reset_token,
            "[DUMMY] Password reset email SENT (simulated)"
        );
        Ok(())
    }

    async fn send_invitation_email(
        &self,
        to_email: &str,
        inviter_name: &str,
        org_name: &str,
        role: &str,
        invite_link: &str,
    ) -> Result<()> {
        info!(
            to = %to_email,
            inviter = %inviter_name,
            org = %org_name,
            role = %role,
            link = %invite_link,
            "[DUMMY] Team invitation email SENT (simulated)"
        );
        Ok(())
    }

    async fn send_enterprise_lead_notification(
        &self,
        to_email: &str,
        lead: &EnterpriseLeadNotification,
    ) -> Result<()> {
        info!(
            to = %to_email,
            lead_id = %lead.lead_id,
            company = %lead.company_name,
            "[DUMMY] Enterprise lead notification SENT (simulated)"
        );
        Ok(())
    }
}
