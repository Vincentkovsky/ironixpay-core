//! Xero API HTTP Client
//!
//! Handles direct communication with Xero's OAuth and Accounting API endpoints.

use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Xero-specific errors that need special handling by the caller.
#[derive(Debug, thiserror::Error)]
pub enum XeroError {
    #[error("Xero rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
}

const XERO_TOKEN_URL: &str = "https://identity.xero.com/connect/token";
const XERO_CONNECTIONS_URL: &str = "https://api.xero.com/connections";
const XERO_API_BASE: &str = "https://api.xero.com/api.xro/2.0";
const XERO_REVOKE_URL: &str = "https://identity.xero.com/connect/revocation";
const XERO_ACCEPT_JSON: &str = "application/json";

/// Token response from Xero OAuth.
#[derive(Debug, Deserialize)]
pub struct XeroTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// A connected Xero tenant (organization).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XeroTenant {
    pub id: String,
    pub auth_event_id: String,
    pub tenant_id: String,
    pub tenant_type: String,
    pub tenant_name: String,
}

pub struct XeroApiClient {
    http: Client,
}

impl XeroApiClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    // ─── OAuth Endpoints ───

    /// Exchange authorization code for access + refresh tokens.
    pub async fn exchange_code(
        &self,
        code: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Result<XeroTokens> {
        let resp = self
            .http
            .post(XERO_TOKEN_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await
            .context("Failed to reach Xero token endpoint")?;
        self.check_rate_limit(&resp)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Xero token exchange failed ({}): {}", status, body));
        }

        resp.json::<XeroTokens>()
            .await
            .context("Failed to parse Xero token response")
    }

    /// Refresh an expired access token.
    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<XeroTokens> {
        let resp = self
            .http
            .post(XERO_TOKEN_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .context("Failed to reach Xero token endpoint for refresh")?;
        self.check_rate_limit(&resp)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Xero token refresh failed ({}): {}", status, body));
        }

        resp.json::<XeroTokens>()
            .await
            .context("Failed to parse Xero refresh response")
    }

    /// Revoke a refresh token (disconnect).
    /// Returns Ok(true) if revoked successfully, Ok(false) if Xero returned non-200.
    pub async fn revoke_token(
        &self,
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<bool> {
        let resp = self
            .http
            .post(XERO_REVOKE_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&[("token", refresh_token)])
            .send()
            .await
            .context("Failed to revoke Xero token")?;
        self.check_rate_limit(&resp)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(
                status = %status,
                body = %body,
                "Xero token revocation failed — token may remain valid on Xero's side"
            );
            return Ok(false);
        }
        Ok(true)
    }

    /// Get list of connected tenants (organizations).
    pub async fn get_connections(&self, access_token: &str) -> Result<Vec<XeroTenant>> {
        let resp = self
            .http
            .get(XERO_CONNECTIONS_URL)
            .bearer_auth(access_token)
            .header("Accept", XERO_ACCEPT_JSON)
            .send()
            .await
            .context("Failed to get Xero connections")?;
        self.check_rate_limit(&resp)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Xero connections API failed: {}", body));
        }

        resp.json::<Vec<XeroTenant>>()
            .await
            .context("Failed to parse Xero connections")
    }

    // ─── Accounting API ───

    /// Create an invoice in Xero. Returns the InvoiceID.
    pub async fn create_invoice(
        &self,
        access_token: &str,
        tenant_id: &str,
        invoice_body: &serde_json::Value,
        idempotency_key: &str,
    ) -> Result<String> {
        let url = format!("{}/Invoices", XERO_API_BASE);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", idempotency_key)
            .json(invoice_body)
            .send()
            .await
            .context("Failed to create Xero invoice")?;

        self.check_rate_limit(&resp)?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Xero invoice response")?;

        if !status.is_success() {
            return Err(self.extract_error(status, &body, "create invoice"));
        }

        body["Invoices"][0]["InvoiceID"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Missing InvoiceID in Xero response"))
    }

    /// Create a payment in Xero. Returns the PaymentID.
    pub async fn create_payment(
        &self,
        access_token: &str,
        tenant_id: &str,
        payment_body: &serde_json::Value,
        idempotency_key: &str,
    ) -> Result<String> {
        let url = format!("{}/Payments", XERO_API_BASE);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", idempotency_key)
            .json(payment_body)
            .send()
            .await
            .context("Failed to create Xero payment")?;

        self.check_rate_limit(&resp)?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Xero payment response")?;

        if !status.is_success() {
            return Err(self.extract_error(status, &body, "create payment"));
        }

        body["Payments"][0]["PaymentID"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Missing PaymentID in Xero response"))
    }

    /// Find or create a contact by name. Returns ContactID.
    pub async fn ensure_contact(
        &self,
        access_token: &str,
        tenant_id: &str,
        contact_name: &str,
    ) -> Result<String> {
        // Search existing (URL-encode the where clause)
        let where_clause = format!("Name==\"{}\"", contact_name);
        let url = format!("{}/Contacts", XERO_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[("where", &where_clause)])
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .send()
            .await
            .context("Failed to search Xero contacts")?;
        self.check_rate_limit(&resp)?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            return Err(self.extract_error(status, &body, "search contacts"));
        }

        if let Some(id) = body["Contacts"][0]["ContactID"].as_str() {
            return Ok(id.to_string());
        }

        // Create new
        let create_body = serde_json::json!({
            "Name": contact_name,
            "EmailAddress": "payments@ironixpay.com",
        });

        let resp = self
            .http
            .post(&format!("{}/Contacts", XERO_API_BASE))
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .header("Content-Type", "application/json")
            .json(&create_body)
            .send()
            .await
            .context("Failed to create Xero contact")?;
        self.check_rate_limit(&resp)?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if !status.is_success() {
            return Err(self.extract_error(status, &body, "create contact"));
        }

        body["Contacts"][0]["ContactID"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Missing ContactID in Xero response"))
    }

    /// Get chart of accounts (for account code selection).
    pub async fn get_accounts(
        &self,
        access_token: &str,
        tenant_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let where_clause =
            "Type==\"BANK\"||EnablePaymentsToAccount==true||Class==\"REVENUE\"||Class==\"EXPENSE\"||Class==\"OVERHEADS\"";
        let url = format!("{}/Accounts", XERO_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[("where", where_clause)])
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .send()
            .await
            .context("Failed to get Xero accounts")?;
        self.check_rate_limit(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Xero accounts API failed ({}): {}", status, body));
        }

        let raw = resp
            .text()
            .await
            .context("Failed to read Xero accounts response body")?;
        let body: serde_json::Value = Self::parse_json_body("Xero accounts response", &raw)?;
        let accounts = body["Accounts"].as_array().cloned().unwrap_or_default();
        Ok(accounts)
    }

    /// Get organisation info (for currency/name).
    pub async fn get_organisation(
        &self,
        access_token: &str,
        tenant_id: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/Organisation", XERO_API_BASE);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .send()
            .await
            .context("Failed to get Xero organisation")?;
        self.check_rate_limit(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Xero organisation API failed ({}): {}",
                status,
                body
            ));
        }

        let raw = resp
            .text()
            .await
            .context("Failed to read Xero organisation response body")?;
        let body: serde_json::Value = Self::parse_json_body("Xero organisation response", &raw)?;
        body["Organisations"]
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .ok_or_else(|| anyhow!("No organisations found in Xero response"))
    }

    /// Get tax rates (for tax code selection in invoice sync config).
    pub async fn get_tax_rates(
        &self,
        access_token: &str,
        tenant_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/TaxRates", XERO_API_BASE);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(access_token)
            .header("Xero-Tenant-Id", tenant_id)
            .header("Accept", XERO_ACCEPT_JSON)
            .send()
            .await
            .context("Failed to get Xero tax rates")?;
        self.check_rate_limit(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Xero tax rates API failed ({}): {}", status, body));
        }

        let raw = resp
            .text()
            .await
            .context("Failed to read Xero tax rates response body")?;
        let body: serde_json::Value = Self::parse_json_body("Xero tax rates response", &raw)?;
        let tax_rates = body["TaxRates"].as_array().cloned().unwrap_or_default();
        Ok(tax_rates)
    }

    // ─── Helpers ───

    /// Check if the response is rate-limited (429). Returns typed error with Retry-After info.
    fn check_rate_limit(&self, resp: &reqwest::Response) -> Result<()> {
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(XeroError::RateLimited { retry_after }.into());
        }
        Ok(())
    }

    /// Extract meaningful error from Xero API response.
    fn extract_error(
        &self,
        status: StatusCode,
        body: &serde_json::Value,
        operation: &str,
    ) -> anyhow::Error {
        let msg = body["Message"]
            .as_str()
            .or_else(|| body["Detail"].as_str())
            .or_else(|| body["Elements"][0]["ValidationErrors"][0]["Message"].as_str())
            .unwrap_or("Unknown error");
        anyhow!("Xero {} failed ({}): {}", operation, status, msg)
    }

    fn parse_json_body(context: &str, raw: &str) -> Result<serde_json::Value> {
        serde_json::from_str(raw).map_err(|e| {
            let preview = raw.replace('\n', " ").chars().take(240).collect::<String>();
            anyhow!(
                "Failed to parse {} as JSON: {}. Raw body preview: {}",
                context,
                e,
                preview
            )
        })
    }
}
