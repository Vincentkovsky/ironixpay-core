//! Resend Email Sender
//!
//! Uses the Resend API to send transactional emails.

use super::{templates, EmailSender, EnterpriseLeadNotification};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Resend API request payload
#[derive(Serialize)]
struct ResendEmailRequest {
    from: String,
    to: Vec<String>,
    subject: String,
    html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
}

// Manual Debug to avoid log spam from HTML content
impl fmt::Debug for ResendEmailRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResendEmailRequest")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("subject", &self.subject)
            .field("reply_to", &"<Reply-To Omitted>")
            .field("html", &"<HTML Content Omitted>")
            .finish()
    }
}

/// Resend API response
#[derive(Debug, Deserialize)]
struct ResendEmailResponse {
    id: Option<String>,
}

/// Email service using Resend API
#[derive(Clone)]
pub struct ResendEmailService {
    client: Client,
    from_address: String,
    /// Base URL for the application (e.g. https://checkout.example.com)
    /// Used for verification links, not for the API calls.
    app_base_url: String,
}

// Manual Debug implementation to avoid leaking sensitive data (Client internals, etc)
impl fmt::Debug for ResendEmailService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResendEmailService")
            .field("from_address", &self.from_address)
            .field("app_base_url", &self.app_base_url)
            .field("client", &"<Reqwest Client>")
            .finish()
    }
}

impl ResendEmailService {
    pub fn try_new(api_key: String, from_address: String, app_base_url: String) -> Result<Self> {
        // Configure default headers with Authorization
        let mut headers = header::HeaderMap::new();
        let mut auth_val = header::HeaderValue::from_str(&format!("Bearer {}", api_key))
            .map_err(|e| anyhow!("Invalid API key caused invalid header value: {}", e))?;
        auth_val.set_sensitive(true); // Don't log this header value
        headers.insert(header::AUTHORIZATION, auth_val);

        let client = Client::builder()
            .timeout(Duration::from_secs(10)) // 10s timeout
            .default_headers(headers)
            .build()
            .map_err(|e| anyhow!("Failed to build Resend HTTP client: {}", e))?;

        Ok(Self {
            client,
            from_address,
            app_base_url,
        })
    }

    /// Helper to send generic HTML email via Resend
    async fn send_raw_email(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        let request = ResendEmailRequest {
            from: self.from_address.clone(),
            to: vec![to.to_string()],
            subject: subject.to_string(),
            html: html.to_string(),
            reply_to: reply_to.map(str::to_owned),
        };

        // Idempotency key to prevent duplicate emails on retry
        let idempotency_key = Uuid::new_v4().to_string();

        let max_retries = 3;
        let mut retry_delay = Duration::from_millis(500);

        let mut last_error = None;

        for attempt in 1..=max_retries {
            // Authorization header is already in default_headers
            let result = self
                .client
                .post("https://api.resend.com/emails")
                .header("Idempotency-Key", &idempotency_key)
                .json(&request)
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        let result: ResendEmailResponse = response.json().await?;
                        info!(
                            email_id = ?result.id,
                            to = %to,
                            subject = %subject,
                            attempt = %attempt,
                            "Email sent successfully via Resend"
                        );
                        return Ok(());
                    } else if response.status().is_server_error() {
                        // 5xx errors -> Retry
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        warn!(
                            status = %status,
                            attempt = %attempt,
                            to = %to,
                            "Resend API server error, retrying..."
                        );
                        last_error = Some(anyhow!("Resend API error: {} - {}", status, error_text));
                    } else if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // 429: Rate limited → Retry with longer backoff
                        warn!(
                            attempt = %attempt,
                            to = %to,
                            "Resend API rate limited (429), retrying with longer backoff..."
                        );
                        last_error = Some(anyhow!("Resend API rate limited (429)"));
                        retry_delay = Duration::from_secs(2); // Longer backoff for rate limits
                    } else {
                        // Other 4xx errors -> Fatal (bad request, auth failure, etc.)
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        error!(
                            status = %status,
                            error = %error_text,
                            to = %to,
                            "Failed to send email via Resend (Fatal)"
                        );
                        return Err(anyhow!("Resend API error: {} - {}", status, error_text));
                    }
                }
                Err(e) => {
                    // Network errors -> Retry
                    warn!(
                        error = %e,
                        attempt = %attempt,
                        to = %to,
                        "Network error sending email, retrying..."
                    );
                    last_error = Some(anyhow!("Network error: {}", e));
                }
            }

            if attempt < max_retries {
                tokio::time::sleep(retry_delay).await;
                retry_delay *= 2; // Exponential backoff
            }
        }

        Err(anyhow!(
            "Failed to send email to {} after {} retries",
            to,
            max_retries
        )
        .context(last_error.unwrap_or_else(|| anyhow!("Unknown error"))))
    }
}

#[async_trait]
impl EmailSender for ResendEmailService {
    async fn send_verification_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        verification_token: &str,
    ) -> Result<()> {
        let html = templates::verification_email_html(
            merchant_name,
            &self.app_base_url,
            verification_token,
        );

        self.send_raw_email(to_email, "Verify your IronixPay account", &html, None)
            .await
    }

    async fn send_password_reset_email(
        &self,
        to_email: &str,
        merchant_name: &str,
        reset_token: &str,
    ) -> Result<()> {
        let html =
            templates::password_reset_email_html(merchant_name, &self.app_base_url, reset_token);

        self.send_raw_email(to_email, "Reset your IronixPay password", &html, None)
            .await
    }

    async fn send_invitation_email(
        &self,
        to_email: &str,
        inviter_name: &str,
        org_name: &str,
        role: &str,
        invite_link: &str,
    ) -> Result<()> {
        let html = templates::invitation_email_html(inviter_name, org_name, role, invite_link);

        self.send_raw_email(
            to_email,
            &format!("You've been invited to join {} on IronixPay", org_name),
            &html,
            None,
        )
        .await
    }

    async fn send_enterprise_lead_notification(
        &self,
        to_email: &str,
        lead: &EnterpriseLeadNotification,
    ) -> Result<()> {
        let html = templates::enterprise_lead_notification_html(lead);
        let company_name = lead.company_name.replace('\r', " ").replace('\n', " ");
        let subject = format!("[IronixPay] Enterprise inquiry: {}", company_name);

        self.send_raw_email(to_email, &subject, &html, Some(&lead.contact_email))
            .await
    }
}
