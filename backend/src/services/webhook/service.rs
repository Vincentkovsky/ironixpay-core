//! Webhook Service
//!
//! Handles webhook event queueing and delivery with signature and retry logic.
//! Aligned with docs/system_design.md - uses webhook_endpoints table for configuration.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Utc};
use hmac::{Hmac, Mac};
use reqwest::{redirect::Policy, Client};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect, Set, TransactionTrait,
};
use serde::Serialize;
use sha2::Sha256;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::crypto::decrypt_aes_gcm;
use crate::entity::{
    webhook_endpoints, webhook_events, Environment, Network, WebhookEndpoints, WebhookEvents,
};
use crate::services::alerting::{AlertLevel, AlertingService};
use rand::{rngs::OsRng, RngCore};
use tokio_util::sync::CancellationToken;
use url::Url;

type HmacSha256 = Hmac<Sha256>;

/// Retry delays aligned with docs/system_design.md §1.6
/// Schedule: 15s, 1m, 5m, 1h, 6h, 24h
const RETRY_DELAYS_SECS: [i64; 7] = [
    0,     // Attempt 0: immediate (first try)
    15,    // Attempt 1: 15 seconds
    60,    // Attempt 2: 1 minute
    300,   // Attempt 3: 5 minutes
    3600,  // Attempt 4: 1 hour
    21600, // Attempt 5: 6 hours
    86400, // Attempt 6: 24 hours
];

/// Maximum concurrent webhook deliveries to prevent overwhelming merchant servers
/// 50 balances throughput (100 webhooks/s at 500ms latency) vs merchant pressure
const MAX_CONCURRENT_DELIVERIES: usize = 50;

/// Batch size for recovery loop to prevent memory exhaustion
const RECOVERY_BATCH_SIZE: u64 = 50;

/// Bound DNS lookups initiated by merchant-controlled webhook URLs.
const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for webhook delivery
#[derive(Clone)]
pub struct WebhookConfig {
    pub timeout_seconds: u64,
    pub max_retries: u8,
}

#[derive(Debug)]
struct ResolvedWebhookTarget {
    url: Url,
    dns_host: Option<String>,
    addresses: Vec<SocketAddr>,
}

#[derive(Debug, thiserror::Error)]
enum WebhookRequestError {
    #[error("webhook target rejected by security policy: {0}")]
    UnsafeTarget(String),
    #[error(transparent)]
    Transient(#[from] anyhow::Error),
}

fn validate_public_addresses(addresses: &[SocketAddr]) -> Result<(), WebhookRequestError> {
    if addresses
        .iter()
        .map(SocketAddr::ip)
        .any(|ip| !is_publicly_routable(ip))
    {
        return Err(WebhookRequestError::UnsafeTarget(
            "host resolves to a non-public address".to_string(),
        ));
    }
    Ok(())
}

fn is_publicly_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();

    !matches!(
        (a, b, c),
        (0, _, _)
            | (10, _, _)
            | (100, 64..=127, _)
            | (127, _, _)
            | (169, 254, _)
            | (172, 16..=31, _)
            | (192, 0, 0)
            | (192, 0, 2)
            | (192, 88, 99)
            | (192, 168, _)
            | (198, 18..=19, _)
            | (198, 51, 100)
            | (203, 0, 113)
            | (224..=255, _, _)
    )
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let octets = ip.octets();

    // IPv4-compatible and IPv4-mapped forms can otherwise disguise an IPv4
    // loopback/private destination from the IPv6 checks below.
    if octets[..10] == [0; 10] && octets[10..12] == [0xff, 0xff] {
        let embedded = Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]);
        return is_public_ipv4(embedded);
    }
    if octets[..12] == [0; 12] {
        return false;
    }

    // Permit only globally routed unicast space. Explicit exclusions cover
    // special-use ranges inside 2000::/3 that must never be webhook targets.
    if !(0x20..=0x3f).contains(&octets[0]) {
        return false;
    }

    let segments = ip.segments();
    let is_special_2001 = segments[0] == 0x2001 && segments[1] <= 0x01ff;
    let is_documentation =
        (segments[0] == 0x2001 && segments[1] == 0x0db8) || ((segments[0] & 0xfff0) == 0x3ff0);
    let is_6to4 = segments[0] == 0x2002;

    !is_special_2001 && !is_documentation && !is_6to4
}

fn parse_webhook_url(url: &str, allow_private_targets: bool) -> Result<Url, WebhookRequestError> {
    let parsed = Url::parse(url)
        .map_err(|_| WebhookRequestError::UnsafeTarget("invalid URL format".to_string()))?;

    let allowed_scheme =
        parsed.scheme() == "https" || (allow_private_targets && parsed.scheme() == "http");
    if !allowed_scheme {
        return Err(WebhookRequestError::UnsafeTarget(
            "HTTPS is required".to_string(),
        ));
    }
    if parsed.host().is_none() {
        return Err(WebhookRequestError::UnsafeTarget(
            "URL must include a host".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(WebhookRequestError::UnsafeTarget(
            "URL credentials are not allowed".to_string(),
        ));
    }

    Ok(parsed)
}

/// Webhook service for queueing and delivering webhook events.
use secrecy::{ExposeSecret, Secret};

/// Webhook service for queueing and delivering webhook events.
pub struct WebhookService {
    db: DatabaseConnection,
    config: WebhookConfig,
    allow_private_targets: bool,
    encryption_key: Secret<String>,
    /// Semaphore to limit concurrent HTTP requests
    delivery_semaphore: Arc<Semaphore>,
    /// Semaphore to limit concurrent background tasks (prevent OOM)
    task_semaphore: Arc<Semaphore>,
    alerting_service: Arc<AlertingService>,
    /// Optional heartbeat reporter for /ready and admin health monitoring.
    service_health: Option<(
        crate::services::service_health::ServiceHealthRegistry,
        String,
    )>,
}

// Manual Clone implementation since Semaphore is shared via Arc
impl Clone for WebhookService {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            config: self.config.clone(),
            allow_private_targets: self.allow_private_targets,
            encryption_key: self.encryption_key.clone(),
            delivery_semaphore: self.delivery_semaphore.clone(),
            task_semaphore: self.task_semaphore.clone(),
            alerting_service: self.alerting_service.clone(),
            service_health: self.service_health.clone(),
        }
    }
}

impl WebhookService {
    pub fn new(
        db: DatabaseConnection,
        encryption_key: Secret<String>,
        timeout_seconds: u64,
        max_retries: u8,
        alerting_service: Arc<AlertingService>,
    ) -> Self {
        Self::new_with_target_policy(
            db,
            encryption_key,
            timeout_seconds,
            max_retries,
            alerting_service,
            false,
        )
    }

    /// Construct a service that can deliver to local mock servers.
    ///
    /// Production wiring must use [`WebhookService::new`]. This constructor is
    /// intentionally explicit so private-network access cannot be enabled by an
    /// ambiguous deployment environment value.
    #[doc(hidden)]
    pub fn new_allowing_private_targets_for_tests(
        db: DatabaseConnection,
        encryption_key: Secret<String>,
        timeout_seconds: u64,
        max_retries: u8,
        alerting_service: Arc<AlertingService>,
    ) -> Self {
        Self::new_with_target_policy(
            db,
            encryption_key,
            timeout_seconds,
            max_retries,
            alerting_service,
            true,
        )
    }

    fn new_with_target_policy(
        db: DatabaseConnection,
        encryption_key: Secret<String>,
        timeout_seconds: u64,
        max_retries: u8,
        alerting_service: Arc<AlertingService>,
        allow_private_targets: bool,
    ) -> Self {
        Self {
            db: db.clone(),
            config: WebhookConfig {
                timeout_seconds,
                max_retries,
            },
            allow_private_targets,
            encryption_key,
            delivery_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_DELIVERIES)),
            // Limit concurrent background tasks to 1000 to prevent OOM
            task_semaphore: Arc::new(Semaphore::new(1000)),
            alerting_service,
            service_health: None,
        }
    }

    /// Attach heartbeat reporting for service health monitoring.
    pub fn with_health(
        mut self,
        registry: crate::services::service_health::ServiceHealthRegistry,
        service_name: String,
    ) -> Self {
        self.service_health = Some((registry, service_name));
        self
    }

    fn validate_webhook_url_shape(&self, url: &str) -> Result<Url, WebhookRequestError> {
        parse_webhook_url(url, self.allow_private_targets)
    }

    /// Resolve and validate every destination address, then return addresses
    /// that can be pinned into reqwest for this one delivery attempt.
    async fn resolve_webhook_target(
        &self,
        url: &str,
    ) -> Result<ResolvedWebhookTarget, WebhookRequestError> {
        let parsed = self.validate_webhook_url_shape(url)?;
        let port = parsed.port_or_known_default().ok_or_else(|| {
            WebhookRequestError::UnsafeTarget("URL must include a valid port".to_string())
        })?;

        let (dns_host, addresses) = match parsed.host().expect("host checked above") {
            url::Host::Ipv4(ip) => (None, vec![SocketAddr::new(IpAddr::V4(ip), port)]),
            url::Host::Ipv6(ip) => (None, vec![SocketAddr::new(IpAddr::V6(ip), port)]),
            url::Host::Domain(domain) => {
                let domain = domain.to_string();
                let resolved = tokio::time::timeout(
                    DNS_RESOLUTION_TIMEOUT,
                    tokio::net::lookup_host((domain.as_str(), port)),
                )
                .await
                .map_err(|_| {
                    WebhookRequestError::Transient(anyhow!("webhook DNS lookup timed out"))
                })?
                .map_err(|e| {
                    WebhookRequestError::Transient(anyhow!("webhook DNS lookup failed: {e}"))
                })?;

                let mut addresses: Vec<SocketAddr> = resolved.collect();
                addresses.sort_unstable();
                addresses.dedup();
                (Some(domain), addresses)
            }
        };

        if addresses.is_empty() {
            return Err(WebhookRequestError::Transient(anyhow!(
                "webhook host did not resolve to any address"
            )));
        }

        if !self.allow_private_targets {
            validate_public_addresses(&addresses)?;
        }

        Ok(ResolvedWebhookTarget {
            url: parsed,
            dns_host,
            addresses,
        })
    }

    /// Generate a secure webhook secret
    /// Format: whsec_<32-bytes-hex>
    fn generate_secret() -> String {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        format!("whsec_{}", hex::encode(key))
    }

    /// Helper to get raw key bytes on demand
    fn get_key_bytes(&self) -> [u8; 32] {
        let hex_str = self.encryption_key.expose_secret();
        let vec = hex::decode(hex_str).expect("Key format validated in config");
        let mut key = [0u8; 32];
        key.copy_from_slice(&vec);
        key
    }

    /// Decrypt webhook endpoint secret
    fn decrypt_secret(&self, encrypted_secret: &str) -> Result<String> {
        let key = self.get_key_bytes();
        decrypt_aes_gcm(encrypted_secret, &key)
            .map_err(|e| anyhow!("Failed to decrypt webhook secret: {}", e))
    }

    /// Get retry delay for a given attempt count
    /// Returns delay in seconds according to spec: 15s, 1m, 5m, 1h, 6h, 24h
    fn get_retry_delay(attempt: i32) -> i64 {
        let idx = (attempt as usize).min(RETRY_DELAYS_SECS.len() - 1);
        RETRY_DELAYS_SECS[idx]
    }

    /// Queue a webhook event for delivery (standalone, uses internal db).
    /// For transactional use, see `queue_event_with_txn`.
    ///
    /// The `network` parameter is used to:
    /// 1. Filter endpoints by environment (network → environment mapping)
    /// 2. Set the `network` field on the created webhook events
    pub async fn queue_event<T: Serialize>(
        &self,
        source_id: &str,
        merchant_id: &str,
        network: Network,
        environment: Environment,
        event_type: &str,
        payload: &T,
    ) -> Result<Vec<String>> {
        let ids = self
            .queue_event_with_txn(
                &self.db,
                source_id,
                merchant_id,
                network,
                environment,
                event_type,
                payload,
            )
            .await?;

        self.trigger_delivery(&ids).await;

        Ok(ids)
    }

    /// Queue webhook events within an existing transaction.
    ///
    /// This variant accepts a transaction/connection reference, enabling atomic
    /// operations (e.g., session update + webhook queue in same transaction).
    ///
    /// **Important**: This method only inserts DB records. Call `spawn_delivery`
    /// for each returned event_id **after** the transaction commits.
    ///
    /// # Arguments
    /// * `network` - The blockchain network (used to derive environment for endpoint filtering)
    ///
    /// # Returns
    /// A list of event IDs that were created (one per endpoint).
    pub async fn queue_event_with_txn<C, T>(
        &self,
        txn: &C,
        source_id: &str,
        merchant_id: &str,
        network: Network,
        environment: Environment,
        event_type: &str,
        payload: &T,
    ) -> Result<Vec<String>>
    where
        C: ConnectionTrait,
        T: Serialize,
    {
        // Environment is now passed explicitly
        let endpoint_env = environment;

        // Sub-merchant support: if merchant_id is a child org, route webhooks to PSP's endpoints
        let (endpoint_org_id, sub_merchant_code) = {
            use crate::entity::sub_merchants;
            match sub_merchants::Entity::find()
                .filter(sub_merchants::Column::ChildOrgId.eq(merchant_id))
                .filter(sub_merchants::Column::Status.eq(sub_merchants::SubMerchantStatus::Active))
                .one(txn)
                .await
            {
                Ok(Some(sm)) => {
                    debug!(
                        child_org_id = %merchant_id,
                        parent_org_id = %sm.parent_org_id,
                        sub_merchant_code = %sm.sub_merchant_code,
                        "Routing webhook to PSP org (sub-merchant detected)"
                    );
                    (sm.parent_org_id, Some(sm.sub_merchant_code))
                }
                Ok(None) => (merchant_id.to_string(), None),
                Err(e) => {
                    warn!(
                        merchant_id = %merchant_id,
                        error = %e,
                        "Failed to check sub-merchant status, falling back to direct routing"
                    );
                    (merchant_id.to_string(), None)
                }
            }
        };

        // Find active webhook endpoints for this merchant AND environment
        let endpoints = WebhookEndpoints::find()
            .filter(webhook_endpoints::Column::MerchantId.eq(&endpoint_org_id))
            .filter(webhook_endpoints::Column::Environment.eq(endpoint_env.clone()))
            .filter(
                webhook_endpoints::Column::Status.eq(webhook_endpoints::EndpointStatus::Enabled),
            )
            .all(txn)
            .await?;

        debug!(
            merchant_id = %merchant_id,
            endpoint_org_id = %endpoint_org_id,
            environment = ?endpoint_env,
            found = endpoints.len(),
            "Searching webhook endpoints"
        );

        if endpoints.is_empty() {
            debug!(merchant_id, environment = ?endpoint_env, "No webhook endpoints configured for environment, skipping");
            return Ok(Vec::new());
        }

        let mut payload_json = serde_json::to_value(payload)?;

        // Inject sub_merchant_code into payload if this is a sub-merchant transaction
        if let Some(ref code) = sub_merchant_code {
            if let Some(data) = payload_json.get_mut("data") {
                data["sub_merchant_code"] = serde_json::Value::String(code.clone());
            } else {
                // If payload has no "data" wrapper, inject at top level
                payload_json["sub_merchant_code"] = serde_json::Value::String(code.clone());
            }
        }

        // Batch insert events for all endpoints
        let mut events_to_insert = Vec::new();
        let mut event_ids = Vec::new();

        for endpoint in endpoints {
            // Use v7 for time-ordered UUIDs to prevent database index fragmentation
            let event_id = format!("evt_{}", Uuid::now_v7().to_string().replace("-", ""));

            let event = webhook_events::ActiveModel {
                id: Set(event_id.clone()),
                network: Set(network.as_str().to_string()),
                endpoint_id: Set(endpoint.id.clone()),
                target_url: Set(endpoint.url.clone()),
                source_id: Set(source_id.to_string()),
                merchant_id: Set(merchant_id.to_string()),
                event_type: Set(event_type.to_string()),
                payload: Set(payload_json.clone()),
                status: Set(webhook_events::WebhookEventStatus::Pending),
                http_status_code: Set(None),
                attempt_count: Set(0),
                last_attempt_at: Set(None),
                next_retry_at: Set(None),
                ..Default::default()
            };

            events_to_insert.push(event);
            event_ids.push(event_id.clone());

            info!(
                event_id = %event_id,
                source_id = %source_id,
                network = ?network,
                event_type = %event_type,
                endpoint_id = %endpoint.id,
                "Webhook event prepared for queueing"
            );
        }

        // Batch insert all events in a single database operation
        if !events_to_insert.is_empty() {
            WebhookEvents::insert_many(events_to_insert)
                .exec(txn)
                .await?;
        }

        Ok(event_ids)
    }

    /// Trigger immediate delivery for events after transaction commits.
    ///
    /// Call this method with event IDs returned from `queue_event_with_txn`
    /// **after** the transaction has been committed.
    ///
    /// # Concurrency Model
    /// - Current: Spawns one Tokio task per event (limited by Semaphore during execution)
    /// - Suitable for: Medium traffic (<1000 events/sec)
    ///
    /// # High-Volume Optimization
    /// For extreme traffic (>10k events/sec), consider implementing a Worker Channel pattern:
    /// ```ignore
    /// // Create bounded channel
    /// let (tx, rx) = mpsc::channel(1000);
    ///
    /// // Fixed worker pool (e.g., 10 workers)
    /// for _ in 0..10 {
    ///     let rx = rx.clone();
    ///     tokio::spawn(async move {
    ///         while let Some(event_id) = rx.recv().await {
    ///             deliver_event_once(&event_id).await;
    ///         }
    ///     });
    /// }
    ///
    /// // trigger_delivery just sends to channel
    /// tx.send(event_id).await;
    /// ```
    /// This prevents unbounded task allocation and memory exhaustion during traffic spikes.
    pub async fn trigger_delivery(&self, event_ids: &[String]) {
        for event_id in event_ids {
            // Acquire permit BEFORE spawning to prevent unbound task creation (OOM protection)
            // This provides backpressure to the caller
            let permit = self.task_semaphore.clone().acquire_owned().await;

            let service = self.clone();
            let event_id = event_id.to_string();

            tokio::spawn(async move {
                // Permit is held for the duration of the task and dropped automatically
                // If acquire failed (Result err), we wouldn't be here, but acquire_owned returns Result
                // However, since we wait, we unwrap or handle logic.
                // Actually acquire_owned returns Result<OwnedSemaphorePermit, AcquireError>.
                // It only fails if semaphore is closed. We expect it open.
                if let Ok(_permit) = permit {
                    if let Err(e) = service.deliver_event_once(&event_id).await {
                        error!(event_id = %event_id, error = %e, "Webhook delivery task failed");
                    }
                }
            });
        }
    }

    /// Spawn a background task to deliver the webhook event.
    /// Uses try_acquire on task_semaphore — if full, skips (recovery loop will pick it up).
    pub fn spawn_delivery(&self, event_id: &str) {
        // Non-blocking: if semaphore is full, skip — recovery loop will handle it
        let permit = match self.task_semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                warn!(event_id = %event_id, "Task semaphore full, deferring to recovery loop");
                return;
            }
        };

        let service = self.clone();
        let event_id = event_id.to_string();
        tokio::spawn(async move {
            let _permit = permit; // held for task duration
            if let Err(e) = service.deliver_event_once(&event_id).await {
                error!(event_id = %event_id, error = %e, "Webhook delivery task failed");
            }
        });
    }

    /// Deliver a webhook event by ID (single attempt only).
    /// This function performs ONE delivery attempt and updates the database state.
    /// Retries are handled by the recovery loop based on next_retry_at.
    async fn deliver_event_once(&self, event_id: &str) -> Result<()> {
        let delivery_start = std::time::Instant::now();
        // Acquire semaphore permit to limit concurrent deliveries
        let _permit = self
            .delivery_semaphore
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire delivery permit: {}", e))?;

        // Start a transaction for atomic state updates
        let txn = self.db.begin().await?;

        // Load and lock the event with SELECT FOR UPDATE to prevent concurrent processing
        let event = WebhookEvents::find_by_id(event_id)
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or_else(|| anyhow!("Event {} not found", event_id))?;

        // Skip if already delivered or currently processing
        if event.status == webhook_events::WebhookEventStatus::Success {
            debug!(event_id = %event_id, "Event already delivered, skipping");
            txn.commit().await?;
            return Ok(());
        }

        // Check if another task is processing this (with timeout protection)
        if event.status == webhook_events::WebhookEventStatus::Processing {
            if let Some(last_attempt) = event.last_attempt_at {
                // Convert both to UTC for comparison
                let last_attempt_utc = last_attempt.with_timezone(&Utc);
                let elapsed = Utc::now().signed_duration_since(last_attempt_utc);
                // If processing for more than 5 minutes, consider it stale and retry
                // Dynamic stale threshold: config.timeout + buffer (60s)
                // This ensures we don't pick up tasks that are still legitimately running
                let stale_threshold = self.config.timeout_seconds as i64 + 60;

                if elapsed.num_seconds() < stale_threshold {
                    debug!(event_id = %event_id, "Event is being processed by another task (not stale)");
                    txn.commit().await?;
                    return Ok(());
                }
                warn!(event_id = %event_id, "Event stuck in processing state, retrying");
            }
        }

        // Mark as processing to prevent concurrent attempts
        let mut active: webhook_events::ActiveModel = event.clone().into();
        active.status = Set(webhook_events::WebhookEventStatus::Processing);
        active.last_attempt_at = Set(Some(Utc::now().into()));
        active.update(&txn).await?;
        // COMMIT TRANSACTION TO RELEASE LOCK BEFORE HTTP REQUEST
        // This ensures checking the lock and releasing it happens before checking external services
        txn.commit().await?;

        // Load the endpoint
        let endpoint = WebhookEndpoints::find_by_id(&event.endpoint_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Endpoint {} not found", event.endpoint_id))?;

        // Check if endpoint is still enabled
        if endpoint.status != webhook_endpoints::EndpointStatus::Enabled {
            warn!(event_id = %event_id, "Endpoint disabled, skipping delivery");

            // Treat as a failure to trigger retry logic (giving user chance to re-enable)
            let new_attempt_count = event.attempt_count + 1;
            let (final_status, next_retry_at) =
                if new_attempt_count >= self.config.max_retries as i32 {
                    (webhook_events::WebhookEventStatus::GivingUp, None)
                } else {
                    let delay = Self::get_retry_delay(new_attempt_count);
                    (
                        webhook_events::WebhookEventStatus::Failed,
                        Some((Utc::now() + ChronoDuration::seconds(delay)).into()),
                    )
                };

            let mut active: webhook_events::ActiveModel = event.into();
            active.status = Set(final_status);
            active.attempt_count = Set(new_attempt_count);
            active.last_attempt_at = Set(Some(Utc::now().into()));
            active.next_retry_at = Set(next_retry_at);
            active.update(&self.db).await?;

            return Ok(());
        }

        // Decrypt the webhook secret - handle permanent errors separately
        let decrypted_secret = match self.decrypt_secret(&endpoint.secret_encrypted) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    event_id = %event_id,
                    error = %e,
                    "Critical configuration error: secret decryption failed. Marking as GivingUp."
                );
                // Permanent error - mark as GivingUp to prevent infinite retries
                let mut active: webhook_events::ActiveModel = event.clone().into();
                active.status = Set(webhook_events::WebhookEventStatus::GivingUp);
                // Safe: use the original model's value instead of unwrapping ActiveValue
                active.attempt_count = Set(event.attempt_count + 1);
                active.last_attempt_at = Set(Some(Utc::now().into()));
                active.update(&self.db).await?;
                return Ok(());
            }
        };

        // Build the webhook payload
        let webhook_payload = WebhookPayload {
            id: event.id.clone(),
            event_type: event.event_type.clone(),
            created: event.created_at.timestamp(),
            data: event.payload.clone(),
        };

        // Perform single HTTP delivery attempt
        let (delivery_success, http_status, permanent_failure) = match self
            .send_http(&endpoint.url, &webhook_payload, &decrypted_secret)
            .await
        {
            Ok(status) => {
                let success = (200..300).contains(&status);
                if success {
                    info!(
                        event_id = %event_id,
                        url = %endpoint.url,
                        status = status,
                        "Webhook delivered successfully"
                    );
                } else {
                    warn!(
                        event_id = %event_id,
                        status = status,
                        "Webhook returned non-2xx status"
                    );
                }
                (success, Some(status as i32), false)
            }
            Err(WebhookRequestError::UnsafeTarget(reason)) => {
                warn!(
                    event_id = %event_id,
                    endpoint_id = %endpoint.id,
                    reason = %reason,
                    "Webhook target rejected; disabling endpoint"
                );

                let mut endpoint_active: webhook_endpoints::ActiveModel = endpoint.into();
                endpoint_active.status = Set(webhook_endpoints::EndpointStatus::Disabled);
                endpoint_active.update(&self.db).await?;

                (false, None, true)
            }
            Err(WebhookRequestError::Transient(e)) => {
                warn!(
                    event_id = %event_id,
                    error = %e,
                    "Webhook delivery attempt failed"
                );
                (false, None, false)
            }
        };

        // Calculate new state based on delivery result
        let new_attempt_count = event.attempt_count + 1;
        let final_status;
        let next_retry_at;

        if delivery_success {
            final_status = webhook_events::WebhookEventStatus::Success;
            next_retry_at = None;
        } else if permanent_failure || new_attempt_count >= self.config.max_retries as i32 {
            final_status = webhook_events::WebhookEventStatus::GivingUp;
            next_retry_at = None;
            crate::services::metrics::inc_webhook_delivery("giving_up");
            if !permanent_failure {
                warn!(
                    event_id = %event_id,
                    attempts = new_attempt_count,
                    "Webhook delivery failed after all retries"
                );
                self.alerting_service.send_alert(
                    "webhook_delivery_exhausted",
                    AlertLevel::Warning,
                    &format!(
                        "⚠️ Webhook permanently failed: event={} session={} endpoint={} after {} attempts",
                        event_id, event.source_id, event.endpoint_id, new_attempt_count
                    ),
                );
            }
        } else {
            final_status = webhook_events::WebhookEventStatus::Failed;
            // Calculate next retry time using exponential backoff
            let delay_secs = Self::get_retry_delay(new_attempt_count);
            next_retry_at = Some((Utc::now() + ChronoDuration::seconds(delay_secs)).into());
            debug!(
                event_id = %event_id,
                next_retry_in_secs = delay_secs,
                "Scheduled for retry"
            );
        }

        // Prometheus: track delivery outcome and duration
        if delivery_success {
            crate::services::metrics::inc_webhook_delivery("success");
        } else if final_status == webhook_events::WebhookEventStatus::Failed {
            crate::services::metrics::inc_webhook_delivery("retry");
        }
        crate::services::metrics::record_webhook_duration(delivery_start.elapsed().as_secs_f64());

        // Update event record with final state
        let mut active: webhook_events::ActiveModel = event.into();
        active.status = Set(final_status);
        active.http_status_code = Set(http_status);
        active.attempt_count = Set(new_attempt_count);
        active.last_attempt_at = Set(Some(Utc::now().into()));
        active.next_retry_at = Set(next_retry_at);
        active.update(&self.db).await?;

        Ok(())
    }

    /// Send HTTP POST request with HMAC signature
    async fn send_http(
        &self,
        url: &str,
        payload: &WebhookPayload,
        secret: &str,
    ) -> Result<u16, WebhookRequestError> {
        let target = self.resolve_webhook_target(url).await?;
        let body =
            serde_json::to_string(payload).map_err(|e| WebhookRequestError::Transient(e.into()))?;

        // Generate HMAC-SHA256 signature
        let timestamp = Utc::now().timestamp();
        let timestamp_str = timestamp.to_string();

        // Critical Fix: Signature must bind timestamp to prevent replay attacks
        // Spec: HMAC-SHA256(Secret, Timestamp + "." + Payload_JSON)
        let signature = self
            .generate_signature(&body, &timestamp_str, secret)
            .map_err(WebhookRequestError::Transient)?;

        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(self.config.timeout_seconds))
            .redirect(Policy::none())
            .no_proxy();
        if let Some(host) = &target.dns_host {
            client_builder = client_builder.resolve_to_addrs(host, &target.addresses);
        }
        let client = client_builder.build().map_err(|e| {
            WebhookRequestError::Transient(anyhow!("failed to build webhook client: {e}"))
        })?;

        let resp = client
            .post(target.url)
            .header("Content-Type", "application/json")
            // Aligned with docs/system_design.md §7.2
            .header("X-Signature", &signature)
            .header("X-Timestamp", &timestamp_str)
            .body(body)
            .send()
            .await
            .map_err(|e| WebhookRequestError::Transient(e.into()))?;

        let status = resp.status().as_u16();
        Ok(status)
    }

    fn generate_signature(&self, payload: &str, timestamp: &str, secret: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| anyhow!("HMAC invalid key length"))?;

        // Bind timestamp and payload
        let message = format!("{}.{}", timestamp, payload);
        mac.update(message.as_bytes());

        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Recovery loop: scans for pending/retryable events and delivers them.
    pub async fn start_recovery_loop(self: Arc<Self>, token: CancellationToken) -> Result<()> {
        info!("Starting webhook recovery loop");

        // Initial heartbeat to immediately transition from Starting -> Healthy
        if let Some((ref reg, ref name)) = self.service_health {
            reg.heartbeat(name);
        }

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Webhook recovery loop received shutdown signal");
                    break;
                }
                _ = async {
                     // Dynamic sleep: if we processed a full batch, check again immediately
                    match self.recover_pending_events().await {
                        Ok(count) => {
                            if count < RECOVERY_BATCH_SIZE {
                                // Relax if load is low
                                tokio::time::sleep(Duration::from_secs(60)).await;
                            } else {
                                // Yield to let other tasks run, then continue immediately
                                info!("Webhook recovery batch full, continuing immediately");
                                tokio::task::yield_now().await;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Webhook recovery loop iteration failed");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                } => {}
            }

            // Heartbeat after each recovery cycle
            if let Some((ref reg, ref name)) = self.service_health {
                reg.heartbeat(name);
            }
        }
        info!("Webhook recovery loop stopped");
        Ok(())
    }

    /// Scan for pending events that need delivery or retry
    /// Returns the number of events triggered for delivery
    pub async fn recover_pending_events(&self) -> Result<u64> {
        let now = Utc::now();
        // Dynamic stale threshold consistent with deliver_event_once
        let stale_threshold =
            now - ChronoDuration::seconds(self.config.timeout_seconds as i64 + 60);

        // Split complex OR query into simple queries for better index usage
        let mut events_to_process = Vec::new();

        // 1. Pending events (never attempted)
        let pending = WebhookEvents::find()
            .filter(webhook_events::Column::Status.eq(webhook_events::WebhookEventStatus::Pending))
            .limit(RECOVERY_BATCH_SIZE)
            .all(&self.db)
            .await?;
        events_to_process.extend(pending);

        // 2. Failed events ready for retry (only if we have capacity)
        if (events_to_process.len() as u64) < RECOVERY_BATCH_SIZE {
            let space_left = RECOVERY_BATCH_SIZE - events_to_process.len() as u64;
            let failed_retry = WebhookEvents::find()
                .filter(
                    webhook_events::Column::Status.eq(webhook_events::WebhookEventStatus::Failed),
                )
                .filter(webhook_events::Column::NextRetryAt.lte(now))
                .filter(webhook_events::Column::AttemptCount.lt(self.config.max_retries as i32))
                .limit(space_left)
                .all(&self.db)
                .await?;
            events_to_process.extend(failed_retry);
        }

        // 3. Stale processing events (only if we have capacity)
        if (events_to_process.len() as u64) < RECOVERY_BATCH_SIZE {
            let space_left = RECOVERY_BATCH_SIZE - events_to_process.len() as u64;
            let stale_processing = WebhookEvents::find()
                .filter(
                    webhook_events::Column::Status
                        .eq(webhook_events::WebhookEventStatus::Processing),
                )
                .filter(webhook_events::Column::LastAttemptAt.lte(stale_threshold))
                .limit(space_left)
                .all(&self.db)
                .await?;
            events_to_process.extend(stale_processing);
        }

        let mut recovered = 0;
        // Acquire semaphore permits for recovery tasks to respect global limit
        // We use spawn_delivery but bypassing the public trigger_delivery's semaphore check
        // because we want consistent throttling.
        // Actually, trigger_delivery adds the semaphore check.
        // spawn_delivery does NOT have the check.
        // We really should use the same throttling mechanism.
        // Let's modify usage here to use the throttled approach manually or via a helper.
        // But since we are in a loop and we don't want to block the recovery loop indefinitely
        // properly, we'll use a loop here.

        for event in events_to_process {
            debug!(event_id = %event.id, status = ?event.status, "Recovering webhook event");

            // We need to respect the task semaphore here too, otherwise recovery could spike OOM
            let permit = self.task_semaphore.clone().acquire_owned().await;

            if let Ok(_permit) = permit {
                let service = self.clone();
                let event_id = event.id.clone();
                tokio::spawn(async move {
                    // _permit held
                    if let Err(e) = service.deliver_event_once(&event_id).await {
                        error!(event_id = %event_id, error = %e, "Webhook recovery task failed");
                    }
                });
                recovered += 1;
            }
        }

        if recovered > 0 {
            info!(count = recovered, "Recovered pending webhook events");
        }

        Ok(recovered)
    }

    // === Configuration Management ===

    /// Get webhook configuration for a merchant in a specific environment
    pub async fn get_config(
        &self,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<Option<webhook_endpoints::Model>> {
        WebhookEndpoints::find()
            .filter(webhook_endpoints::Column::MerchantId.eq(merchant_id))
            .filter(webhook_endpoints::Column::Environment.eq(environment))
            .one(&self.db)
            .await
            .map_err(|e| anyhow!("Database error: {}", e))
    }

    /// Update or Create webhook configuration.
    ///
    /// - `url`: Required for creation, optional for updates (None = keep existing).
    /// - `rotate_secret`: When true, generates a new secret and returns plaintext.
    ///
    /// Returns the (Model, plain_text_secret_if_rotated_or_created).
    pub async fn update_config(
        &self,
        merchant_id: &str,
        environment: Environment,
        url: Option<String>,
        status_opt: Option<webhook_endpoints::EndpointStatus>,
        rotate_secret: bool,
    ) -> Result<(webhook_endpoints::Model, Option<String>)> {
        let existing = WebhookEndpoints::find()
            .filter(webhook_endpoints::Column::MerchantId.eq(merchant_id))
            .filter(webhook_endpoints::Column::Environment.eq(environment))
            .one(&self.db)
            .await?;

        // Resolve the final URL
        let final_url = match (&existing, &url) {
            (None, None) => {
                return Err(anyhow!(
                    "URL is required when creating a new webhook endpoint"
                ));
            }
            (None, Some(u)) | (Some(_), Some(u)) => u.clone(),
            (Some(e), None) => e.url.clone(), // Keep existing URL
        };

        // Determine final status: Use provided, or keep existing, or default to Enabled for new
        let final_status = status_opt.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.status.clone())
                .unwrap_or(webhook_endpoints::EndpointStatus::Enabled)
        });

        let was_enabled = existing
            .as_ref()
            .map(|endpoint| endpoint.status == webhook_endpoints::EndpointStatus::Enabled)
            .unwrap_or(false);
        if url.is_some()
            || (!was_enabled && final_status == webhook_endpoints::EndpointStatus::Enabled)
        {
            self.resolve_webhook_target(&final_url)
                .await
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        let (secret_encrypted, plain_secret) = match (existing.as_ref(), rotate_secret) {
            (Some(current), false) => (current.secret_encrypted.clone(), None),
            _ => {
                let new_secret = Self::generate_secret();
                let key = self.get_key_bytes();
                let encrypted = crate::crypto::encrypt_aes_gcm(&new_secret, &key)
                    .map_err(|e| anyhow!("Encryption failed: {}", e))?;
                (encrypted, Some(new_secret))
            }
        };

        let txn = self.db.begin().await?;

        let model = if let Some(current) = existing {
            let mut active: webhook_endpoints::ActiveModel = current.into();
            active.url = Set(final_url);
            active.status = Set(final_status);
            if rotate_secret {
                active.secret_encrypted = Set(secret_encrypted);
            }
            active.update(&txn).await?
        } else {
            // Create new
            use crate::entity::webhook_endpoints;
            let id = format!("we_{}", Uuid::now_v7().to_string().replace("-", ""));
            let active = webhook_endpoints::ActiveModel {
                id: Set(id),
                merchant_id: Set(merchant_id.to_string()),
                environment: Set(environment),
                url: Set(final_url),
                description: Set(Some("Default Webhook".to_string())),
                secret_encrypted: Set(secret_encrypted),
                status: Set(final_status),
                created_at: Set(Utc::now().into()),
            };
            active.insert(&txn).await?
        };

        txn.commit().await?;

        Ok((model, plain_secret))
    }

    /// Delete webhook configuration for a merchant environment.
    pub async fn delete_config(&self, merchant_id: &str, environment: Environment) -> Result<()> {
        let result = WebhookEndpoints::delete_many()
            .filter(webhook_endpoints::Column::MerchantId.eq(merchant_id))
            .filter(webhook_endpoints::Column::Environment.eq(environment))
            .exec(&self.db)
            .await?;

        info!(
            merchant_id = %merchant_id,
            rows_deleted = result.rows_affected,
            "Deleted webhook endpoint(s)"
        );

        Ok(())
    }

    /// List webhook logs
    pub async fn list_logs(
        &self,
        merchant_id: &str,
        filter: &crate::api::dtos::webhooks::WebhookLogFilter,
    ) -> Result<(Vec<webhook_events::Model>, u64)> {
        use sea_orm::{PaginatorTrait, QueryOrder};

        let mut query = webhook_events::Entity::find()
            .filter(webhook_events::Column::MerchantId.eq(merchant_id));

        if let Some(ref rid) = filter.source_id {
            query = query.filter(webhook_events::Column::SourceId.eq(rid));
        }

        let paginator = query
            .order_by_desc(webhook_events::Column::CreatedAt)
            .paginate(&self.db, filter.pagination.page_size);

        let total = paginator.num_items().await?;
        let data = paginator.fetch_page(filter.pagination.page - 1).await?;

        Ok((data, total))
    }
    /// Resend a webhook event
    /// Resets status to Pending and next_retry_at to Now
    pub async fn resend_event(
        &self,
        event_id: &str,
        merchant_id: &str,
    ) -> std::result::Result<(), crate::api::error::AppError> {
        let event = WebhookEvents::find_by_id(event_id)
            .one(&self.db)
            .await
            .map_err(|e| crate::api::error::AppError::InternalServerError(e.into()))?
            .ok_or_else(|| {
                crate::api::error::AppError::NotFound(format!(
                    "Webhook event '{}' not found",
                    event_id
                ))
            })?;

        if event.merchant_id != merchant_id {
            return Err(crate::api::error::AppError::PermissionDenied(
                "This webhook event belongs to a different merchant".into(),
            ));
        }

        // Idempotency check: If already pending and scheduled for immediate execution, skip DB update
        // This prevents double-clicks from resetting the state unnecessarily or causing race conditions
        let already_pending = event.status == webhook_events::WebhookEventStatus::Pending;
        let scheduled_soon = event
            .next_retry_at
            .map(|t| t.with_timezone(&Utc) <= Utc::now())
            .unwrap_or(true); // If None, it implies immediate (or was just created). Pending + None usually means immediate.

        if already_pending && scheduled_soon {
            info!(
                event_id,
                "Resend request idempotent - event already pending"
            );
            // Still spawn delivery to be sure it gets picked up (in case the previous spawn failed or worker died)
            // It uses a semaphore so it's safe to call multiple times (will just queue up or skip if locked)
            self.spawn_delivery(event_id);
            return Ok(());
        }

        let mut active: webhook_events::ActiveModel = event.into();
        active.status = Set(webhook_events::WebhookEventStatus::Pending);
        active.attempt_count = Set(0); // Reset attempts? Or keep incrementing?
                                       // Usually easier to reset if we want extensive retries.
                                       // But let's keep it clean: reset to Pending basically restarts the lifecycle.
        active.next_retry_at = Set(Some(Utc::now().into())); // Ready immediately
        active.last_attempt_at = Set(None); // Clear last attempt so it doesn't look like it just failed

        active
            .update(&self.db)
            .await
            .map_err(|e| crate::api::error::AppError::InternalServerError(e.into()))?;

        // Optionally trigger immediate delivery spawn?
        // The recovery loop will pick it up, but latency might be up to 60s.
        // Let's spawn it immediately for better UX.
        self.spawn_delivery(event_id);

        Ok(())
    }
}

#[derive(Serialize, Clone)]
struct WebhookPayload {
    id: String,
    event_type: String,
    created: i64,
    data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn strict_urls_require_https_without_credentials() {
        assert!(parse_webhook_url("https://hooks.example.com/events", false).is_ok());
        assert!(parse_webhook_url("http://hooks.example.com/events", false).is_err());
        assert!(parse_webhook_url("https://user:pass@hooks.example.com", false).is_err());
        assert!(parse_webhook_url("http://127.0.0.1:3000/events", true).is_ok());
    }

    #[test]
    fn blocks_non_public_ipv4_ranges() {
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.1.1",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
        ] {
            assert!(!is_publicly_routable(ip.parse().unwrap()), "{ip}");
        }

        assert!(is_publicly_routable("1.1.1.1".parse().unwrap()));
        assert!(is_publicly_routable("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn blocks_non_public_and_ipv4_mapped_ipv6_ranges() {
        for ip in [
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "3fff::1",
        ] {
            assert!(!is_publicly_routable(ip.parse().unwrap()), "{ip}");
        }

        assert!(is_publicly_routable(
            "2606:4700:4700::1111".parse().unwrap()
        ));
    }

    #[test]
    fn rejects_a_hostname_if_any_resolved_address_is_non_public() {
        let addresses = [
            "1.1.1.1:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap(),
        ];
        assert!(validate_public_addresses(&addresses).is_err());
    }

    #[tokio::test]
    async fn strict_service_rejects_private_literal_targets() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let service = WebhookService::new(
            db,
            Secret::new(hex::encode([1u8; 32])),
            5,
            3,
            Arc::new(AlertingService::new(None, Environment::Sandbox)),
        );

        for url in [
            "https://127.0.0.1/webhook",
            "https://127.1/webhook",
            "https://2130706433/webhook",
            "https://[::ffff:127.0.0.1]/webhook",
            "https://localhost/webhook",
        ] {
            let result = service.resolve_webhook_target(url).await;
            assert!(
                matches!(result, Err(WebhookRequestError::UnsafeTarget(_))),
                "{url}"
            );
        }
    }

    #[tokio::test]
    async fn webhook_client_does_not_follow_redirects() {
        let server = MockServer::start().await;
        let redirect_target = format!("{}/internal", server.uri());

        Mock::given(method("POST"))
            .and(path("/redirect"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("Location", redirect_target.as_str()),
            )
            .mount(&server)
            .await;
        Mock::given(path("/internal"))
            .respond_with(ResponseTemplate::new(200).set_body_string("sensitive"))
            .mount(&server)
            .await;

        let db = Database::connect("sqlite::memory:").await.unwrap();
        let service = WebhookService::new_allowing_private_targets_for_tests(
            db,
            Secret::new(hex::encode([1u8; 32])),
            5,
            3,
            Arc::new(AlertingService::new(None, Environment::Sandbox)),
        );
        let payload = WebhookPayload {
            id: "evt_test".to_string(),
            event_type: "test.event".to_string(),
            created: Utc::now().timestamp(),
            data: serde_json::json!({}),
        };

        let status = service
            .send_http(&format!("{}/redirect", server.uri()), &payload, "secret")
            .await
            .unwrap();
        assert_eq!(status, 302);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/redirect");
    }
}
