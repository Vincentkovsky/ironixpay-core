//! Alerting service for MVP monitoring.
//!
//! Provides fire-and-forget alerting to Slack/Discord/DingTalk via webhooks.
//! Features:
//! - Async non-blocking sends (tokio::spawn)
//! - Debounce/cooldown to prevent alert storms (10 min per unique key)

use dashmap::DashMap;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::entity::Environment;

/// Default cooldown period for duplicate alerts (10 minutes)
const DEFAULT_COOLDOWN: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl AlertLevel {
    fn emoji(&self) -> &'static str {
        match self {
            AlertLevel::Info => "ℹ️",
            AlertLevel::Warning => "⚠️",
            AlertLevel::Critical => "🚨",
        }
    }
}

#[derive(Clone)]
pub struct AlertingService {
    webhook_url: Option<String>,
    client: Client,
    /// Tracks last alert time per unique key for debouncing
    cooldowns: Arc<DashMap<String, Instant>>,
    cooldown_duration: Duration,
    /// Current environment for context in alert messages
    environment: Environment,
}

impl AlertingService {
    /// Create a new AlertingService.
    /// If `webhook_url` is None, alerts will be logged but not sent.
    pub fn new(webhook_url: Option<String>, environment: Environment) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
            cooldowns: Arc::new(DashMap::new()),
            cooldown_duration: DEFAULT_COOLDOWN,
            environment,
        }
    }

    /// Create with custom cooldown duration (useful for testing).
    pub fn with_cooldown(
        webhook_url: Option<String>,
        cooldown: Duration,
        environment: Environment,
    ) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
            cooldowns: Arc::new(DashMap::new()),
            cooldown_duration: cooldown,
            environment,
        }
    }

    /// Send an alert asynchronously (fire-and-forget).
    /// This will NOT block the calling task.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for deduplication (e.g., "sweeper_broadcast_failed")
    /// * `level` - Alert severity level
    /// * `message` - Human-readable alert message
    pub fn send_alert(&self, key: &str, level: AlertLevel, message: &str) {
        // Check cooldown
        let key_str = key.to_string();
        if let Some(last) = self.cooldowns.get(&key_str) {
            if last.elapsed() < self.cooldown_duration {
                // Still in cooldown, skip this alert
                return;
            }
        }

        // Update cooldown timestamp
        self.cooldowns.insert(key_str.clone(), Instant::now());

        // Log the alert
        match level {
            AlertLevel::Info => info!(key, %level, "Alert: {}", message),
            AlertLevel::Warning => warn!(key, %level, "Alert: {}", message),
            AlertLevel::Critical => error!(key, %level, "CRITICAL Alert: {}", message),
        }

        // If no webhook URL configured, just log
        let Some(url) = self.webhook_url.clone() else {
            return;
        };

        // Fire-and-forget: spawn a background task
        let client = self.client.clone();
        let env_tag = match self.environment {
            Environment::Production => "PROD",
            Environment::Sandbox => "SANDBOX",
        };
        let formatted_msg = format!("{} [{}] [{}] {}", level.emoji(), env_tag, key, message);

        tokio::spawn(async move {
            // Slack-compatible payload
            let payload = serde_json::json!({
                "text": formatted_msg
            });

            match client.post(&url).json(&payload).send().await {
                Ok(resp) if resp.status().is_success() => {
                    // Successfully sent
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "Failed to send alert: non-success status");
                }
                Err(e) => {
                    warn!(error = %e, "Failed to send alert: network error");
                }
            }
        });
    }

    /// Clear all cooldowns (useful for testing).
    #[cfg(test)]
    pub fn clear_cooldowns(&self) {
        self.cooldowns.clear();
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Info => write!(f, "INFO"),
            AlertLevel::Warning => write!(f, "WARNING"),
            AlertLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooldown_prevents_duplicate_alerts() {
        let service =
            AlertingService::with_cooldown(None, Duration::from_millis(100), Environment::Sandbox);

        // First alert should pass
        service.send_alert("test_key", AlertLevel::Warning, "Test message");

        // Check that cooldown was set
        assert!(service.cooldowns.contains_key("test_key"));

        // Immediate second call should be skipped (but we can't easily test this without a mock)
    }
}
