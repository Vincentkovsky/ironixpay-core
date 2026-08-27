//! Email Service Module
//!
//! Provides a unified interface for sending emails, with pluggable backends.
//!
//! - `EmailSender` trait for loose coupling
//! - `ResendEmailSender` for production
//! - `DummyEmailSender` for testing/dev

use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

pub mod dummy;
pub mod resend;
pub mod templates;

#[derive(Debug, Clone)]
pub struct EnterpriseLeadNotification {
    pub lead_id: String,
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
    pub submitted_at: String,
}

/// Abstract interface for sending emails
#[async_trait]
pub trait EmailSender: Send + Sync + Debug {
    /// Send email verification link
    async fn send_verification_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        verification_token: &str,
    ) -> Result<()>;

    /// Send password reset email
    async fn send_password_reset_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        reset_token: &str,
    ) -> Result<()>;

    /// Send team invitation email
    async fn send_invitation_email(
        &self,
        to_email: &str,
        inviter_name: &str,
        org_name: &str,
        role: &str,
        invite_link: &str,
    ) -> Result<()>;

    /// Notify the internal sales inbox about a persisted enterprise inquiry.
    async fn send_enterprise_lead_notification(
        &self,
        to_email: &str,
        lead: &EnterpriseLeadNotification,
    ) -> Result<()>;
}
