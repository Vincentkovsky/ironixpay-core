use reqwest::Client;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;
use thiserror::Error;
use tracing::warn;

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const REGISTRATION_ACTION: &str = "register";
const FORGOT_PASSWORD_ACTION: &str = "forgot_password";
const MAX_TOKEN_LENGTH: usize = 2_048;

#[derive(Debug, Error)]
pub enum TurnstileError {
    #[error("Human verification failed")]
    Rejected,
    #[error("Human verification service is temporarily unavailable")]
    Unavailable,
}

#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

pub struct TurnstileService {
    client: Client,
    secret_key: Secret<String>,
    expected_hostname: String,
    endpoint: String,
}

impl TurnstileService {
    pub fn new(secret_key: Secret<String>, expected_hostname: String) -> Self {
        Self::with_endpoint(secret_key, expected_hostname, SITEVERIFY_URL.to_string())
    }

    fn with_endpoint(
        secret_key: Secret<String>,
        expected_hostname: String,
        endpoint: String,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .no_proxy()
            .build()
            .expect("Failed to build Turnstile HTTP client");

        Self {
            client,
            secret_key,
            expected_hostname: expected_hostname.trim().to_lowercase(),
            endpoint,
        }
    }

    pub async fn verify_registration(
        &self,
        token: &str,
        client_ip: IpAddr,
    ) -> Result<(), TurnstileError> {
        self.verify(token, client_ip, REGISTRATION_ACTION).await
    }

    pub async fn verify_forgot_password(
        &self,
        token: &str,
        client_ip: IpAddr,
    ) -> Result<(), TurnstileError> {
        self.verify(token, client_ip, FORGOT_PASSWORD_ACTION).await
    }

    async fn verify(
        &self,
        token: &str,
        client_ip: IpAddr,
        expected_action: &'static str,
    ) -> Result<(), TurnstileError> {
        let token = token.trim();
        if token.is_empty() || token.len() > MAX_TOKEN_LENGTH {
            return Err(TurnstileError::Rejected);
        }

        let remote_ip = client_ip.to_string();
        let response = self
            .client
            .post(&self.endpoint)
            .form(&[
                ("secret", self.secret_key.expose_secret().as_str()),
                ("response", token),
                ("remoteip", remote_ip.as_str()),
            ])
            .send()
            .await
            .map_err(|error| {
                warn!(error = %error, "Turnstile Siteverify request failed");
                TurnstileError::Unavailable
            })?;

        if !response.status().is_success() {
            warn!(status = %response.status(), "Turnstile Siteverify returned an HTTP error");
            return Err(TurnstileError::Unavailable);
        }

        let result: SiteverifyResponse = response.json().await.map_err(|error| {
            warn!(error = %error, "Turnstile Siteverify returned an invalid response");
            TurnstileError::Unavailable
        })?;

        if !result.success {
            warn!(
                expected_action,
                error_codes = ?result.error_codes,
                "Turnstile rejected challenge"
            );
            return Err(TurnstileError::Rejected);
        }

        if result.action.as_deref() != Some(expected_action) {
            warn!(expected_action, action = ?result.action, "Turnstile action mismatch");
            return Err(TurnstileError::Rejected);
        }

        let hostname_matches = result
            .hostname
            .as_deref()
            .is_some_and(|hostname| hostname.eq_ignore_ascii_case(&self.expected_hostname));
        if !hostname_matches {
            warn!(hostname = ?result.hostname, "Turnstile hostname mismatch");
            return Err(TurnstileError::Rejected);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

    fn verifier(server: &MockServer) -> TurnstileService {
        TurnstileService::with_endpoint(
            Secret::new("test-secret".to_string()),
            "app.ironixpay.com".to_string(),
            format!("{}/siteverify", server.uri()),
        )
    }

    #[tokio::test]
    async fn accepts_matching_registration_challenge() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "hostname": "app.ironixpay.com",
                "action": "register"
            })))
            .mount(&server)
            .await;

        assert!(verifier(&server)
            .verify_registration("valid-token", "203.0.113.1".parse().unwrap())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn accepts_matching_forgot_password_challenge() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "hostname": "app.ironixpay.com",
                "action": "forgot_password"
            })))
            .mount(&server)
            .await;

        assert!(verifier(&server)
            .verify_forgot_password("valid-token", "203.0.113.1".parse().unwrap())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn rejects_registration_token_for_forgot_password() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "hostname": "app.ironixpay.com",
                "action": "register"
            })))
            .mount(&server)
            .await;

        assert!(matches!(
            verifier(&server)
                .verify_forgot_password("registration-token", "203.0.113.1".parse().unwrap())
                .await,
            Err(TurnstileError::Rejected)
        ));
    }

    #[tokio::test]
    async fn rejects_action_or_hostname_mismatch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "hostname": "attacker.example",
                "action": "login"
            })))
            .mount(&server)
            .await;

        assert!(matches!(
            verifier(&server)
                .verify_registration("valid-token", "203.0.113.1".parse().unwrap())
                .await,
            Err(TurnstileError::Rejected)
        ));
    }

    #[tokio::test]
    async fn rejects_failed_challenge_without_exposing_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": false,
                "error-codes": ["timeout-or-duplicate"]
            })))
            .mount(&server)
            .await;

        assert!(matches!(
            verifier(&server)
                .verify_registration("replayed-token", "203.0.113.1".parse().unwrap())
                .await,
            Err(TurnstileError::Rejected)
        ));
    }
}
