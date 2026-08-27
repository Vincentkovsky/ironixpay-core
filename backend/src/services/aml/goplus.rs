//! GoPlus Security API Client
//!
//! https://docs.gopluslabs.io/reference/address_security

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

/// GoPlus API response structure
#[derive(Debug, Deserialize)]
pub struct GoPlusResponse {
    pub code: i32,
    pub message: String,
    pub result: Option<AddressSecurityResult>,
}

/// Address security result from GoPlus
#[derive(Debug, Clone, Deserialize)]
pub struct AddressSecurityResult {
    /// 1 = sanctioned by OFAC
    #[serde(default)]
    pub sanctioned: String,
    /// 1 = known malicious address
    #[serde(default)]
    pub malicious_address: String,
    /// 1 = phishing activities detected
    #[serde(default)]
    pub phishing_activities: String,
    /// 1 = stealing attack history
    #[serde(default)]
    pub stealing_attack: String,
    /// 1 = blackmail activities
    #[serde(default)]
    pub blackmail_activities: String,
    /// 1 = money laundering activities
    #[serde(default)]
    pub money_laundering: String,
    /// 1 = cybercrime activities
    #[serde(default)]
    pub cybercrime: String,
    /// 1 = darkweb transactions
    #[serde(default)]
    pub darkweb_transactions: String,
    /// 1 = coin mixer address (high money laundering risk)
    #[serde(default)]
    pub mixer: String,
    /// 1 = related to honeypot tokens (scam creator)
    #[serde(default)]
    pub honeypot_related_address: String,
    /// Contract interaction risk
    #[serde(default)]
    pub contract_address: String,
}

impl AddressSecurityResult {
    /// Check if any high-risk flag is set
    pub fn is_high_risk(&self) -> bool {
        self.sanctioned == "1"
            || self.malicious_address == "1"
            || self.phishing_activities == "1"
            || self.stealing_attack == "1"
            || self.blackmail_activities == "1"
            || self.money_laundering == "1"
            || self.cybercrime == "1"
            || self.darkweb_transactions == "1"
            || self.mixer == "1"
            || self.honeypot_related_address == "1"
    }

    /// Get the primary risk reason (priority order: most severe first)
    pub fn risk_reason(&self) -> Option<String> {
        if self.sanctioned == "1" {
            Some("sanctioned".to_string())
        } else if self.money_laundering == "1" {
            Some("money_laundering".to_string())
        } else if self.mixer == "1" {
            Some("mixer".to_string())
        } else if self.malicious_address == "1" {
            Some("malicious_address".to_string())
        } else if self.darkweb_transactions == "1" {
            Some("darkweb_transactions".to_string())
        } else if self.cybercrime == "1" {
            Some("cybercrime".to_string())
        } else if self.phishing_activities == "1" {
            Some("phishing_activities".to_string())
        } else if self.stealing_attack == "1" {
            Some("stealing_attack".to_string())
        } else if self.blackmail_activities == "1" {
            Some("blackmail_activities".to_string())
        } else if self.honeypot_related_address == "1" {
            Some("honeypot_related_address".to_string())
        } else {
            None
        }
    }
}

/// GoPlus API client
pub struct GoPlusClient {
    client: Client,
    base_url: String,
    timeout: Duration,
}

impl GoPlusClient {
    /// Create new client with configurable timeout
    pub fn new(timeout_seconds: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: "https://api.gopluslabs.io/api/v1".to_string(),
            timeout: Duration::from_secs(timeout_seconds),
        }
    }

    /// Check address security via GoPlus API
    ///
    /// Returns Ok(Some(result)) if API call succeeds
    /// Returns Ok(None) if address not found or API returns empty result
    /// Returns Err on network/timeout errors
    pub async fn check_address(
        &self,
        address: &str,
        chain_id: &str,
    ) -> Result<Option<AddressSecurityResult>> {
        let url = format!(
            "{}/address_security/{}?chain_id={}",
            self.base_url, address, chain_id
        );

        debug!("Calling GoPlus API: {}", url);

        let response = self
            .client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| anyhow!("GoPlus API request failed: {}", e))?;

        if !response.status().is_success() {
            warn!("GoPlus API returned status: {}", response.status());
            return Err(anyhow!("GoPlus API returned status: {}", response.status()));
        }

        let body: GoPlusResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse GoPlus response: {}", e))?;

        if body.code != 1 {
            warn!(
                "GoPlus API returned error code {}: {}",
                body.code, body.message
            );
            return Ok(None);
        }

        Ok(body.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_high_risk() {
        let safe = AddressSecurityResult {
            sanctioned: "0".to_string(),
            malicious_address: "0".to_string(),
            phishing_activities: "0".to_string(),
            stealing_attack: "0".to_string(),
            blackmail_activities: "0".to_string(),
            money_laundering: "0".to_string(),
            cybercrime: "0".to_string(),
            darkweb_transactions: "0".to_string(),
            mixer: "0".to_string(),
            honeypot_related_address: "0".to_string(),
            contract_address: "0".to_string(),
        };
        assert!(!safe.is_high_risk());

        let risky = AddressSecurityResult {
            sanctioned: "1".to_string(),
            ..safe.clone()
        };
        assert!(risky.is_high_risk());
        assert_eq!(risky.risk_reason(), Some("sanctioned".to_string()));

        // Test money_laundering priority
        let ml_risky = AddressSecurityResult {
            money_laundering: "1".to_string(),
            ..safe.clone()
        };
        assert!(ml_risky.is_high_risk());
        assert_eq!(ml_risky.risk_reason(), Some("money_laundering".to_string()));

        // Test mixer priority
        let mixer_risky = AddressSecurityResult {
            mixer: "1".to_string(),
            ..safe.clone()
        };
        assert!(mixer_risky.is_high_risk());
        assert_eq!(mixer_risky.risk_reason(), Some("mixer".to_string()));
    }
}
