//! Service Health Registry
//!
//! Heartbeat-based health tracking for background services (non-chain tasks).
//! Mirrors `ChainHealthRegistry` but keyed by service name instead of `Network`.
//!
//! Used by:
//! - **Background tasks**: `heartbeat("service_name")` each poll cycle
//! - **`/ready` endpoint**: `all_statuses()` for informational display (fail-open)
//! - **Admin `/system/health`**: detailed service health for operator dashboards
//!
//! Design: fail-open for readiness — stale services make `/ready` return `degraded`
//! in the response body but do NOT cause HTTP 503. Only DB + chain indexer health
//! is strict (503). Rationale: a hung sweeper shouldn't block new checkouts.

use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Staleness threshold: if heartbeat() hasn't been called within this duration,
/// is_healthy() returns false. 5 minutes covers sweeper (60s cycle) and
/// payment processor (0.5s cycle) with generous margin.
const SERVICE_STALENESS_THRESHOLD: Duration = Duration::from_secs(300);

/// Service lifecycle status
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    /// Service registered but no heartbeat received yet
    Starting,
    /// Service actively sending heartbeats
    Healthy,
    /// No heartbeat within staleness threshold (possibly hung)
    Stale,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Starting => write!(f, "starting"),
            ServiceStatus::Healthy => write!(f, "healthy"),
            ServiceStatus::Stale => write!(f, "stale"),
        }
    }
}

/// Snapshot of a single service's health state.
#[derive(Clone, Debug)]
pub struct ServiceSnapshot {
    pub last_heartbeat: Instant,
    pub heartbeat_count: u64,
    /// When this service was registered (for detecting "never started" → stale).
    pub registered_at: Instant,
}

impl ServiceSnapshot {
    /// Derive status from snapshot state.
    fn status(&self) -> ServiceStatus {
        if self.heartbeat_count == 0 {
            // Never heartbeated — check if registration was too long ago
            if self.registered_at.elapsed() < SERVICE_STALENESS_THRESHOLD {
                ServiceStatus::Starting
            } else {
                ServiceStatus::Stale // registered but never started within threshold
            }
        } else if self.last_heartbeat.elapsed() < SERVICE_STALENESS_THRESHOLD {
            ServiceStatus::Healthy
        } else {
            ServiceStatus::Stale
        }
    }
}

/// Thread-safe registry tracking background service heartbeats.
///
/// Cheap to clone — inner state is `Arc<DashMap>`.
#[derive(Clone)]
pub struct ServiceHealthRegistry {
    services: Arc<DashMap<String, ServiceSnapshot>>,
}

impl ServiceHealthRegistry {
    /// Create a new registry with the given service names.
    /// All start as `Starting` (awaiting first heartbeat).
    pub fn new(service_names: &[&str]) -> Self {
        let services = DashMap::new();
        let now = Instant::now();
        for name in service_names {
            services.insert(
                name.to_string(),
                ServiceSnapshot {
                    last_heartbeat: now,
                    heartbeat_count: 0,
                    registered_at: now,
                },
            );
        }
        Self {
            services: Arc::new(services),
        }
    }

    /// Called by background tasks after each successful poll cycle.
    /// This is the ONLY way to prove the task is alive and making progress.
    pub fn heartbeat(&self, name: &str) {
        if let Some(mut entry) = self.services.get_mut(name) {
            entry.heartbeat_count += 1;
            entry.last_heartbeat = Instant::now();
        }
    }

    /// Dynamically register a new service (e.g., EVM sweepers discovered at runtime).
    /// If already registered, this is a no-op.
    pub fn register_service(&self, name: &str) {
        let now = Instant::now();
        self.services
            .entry(name.to_string())
            .or_insert(ServiceSnapshot {
                last_heartbeat: now,
                heartbeat_count: 0,
                registered_at: now,
            });
    }

    /// Returns the current status of a service.
    pub fn status(&self, name: &str) -> ServiceStatus {
        match self.services.get(name) {
            Some(entry) => entry.status(),
            None => ServiceStatus::Starting,
        }
    }

    /// Returns true if the service has sent at least one heartbeat
    /// and the last heartbeat is within the staleness threshold.
    pub fn is_healthy(&self, name: &str) -> bool {
        self.status(name) == ServiceStatus::Healthy
    }

    /// Snapshot of all service statuses. Used by `/ready` and admin endpoints.
    pub fn all_statuses(&self) -> Vec<(String, ServiceStatus)> {
        self.services
            .iter()
            .map(|entry| (entry.key().clone(), entry.status()))
            .collect()
    }

    /// Returns true if ALL registered services are healthy.
    /// Used for informational purposes only (not for HTTP status code).
    pub fn all_healthy(&self) -> bool {
        self.services
            .iter()
            .all(|entry| entry.status() == ServiceStatus::Healthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_starts_as_starting() {
        let registry = ServiceHealthRegistry::new(&["sweeper", "processor"]);
        assert_eq!(registry.status("sweeper"), ServiceStatus::Starting);
        assert_eq!(registry.status("processor"), ServiceStatus::Starting);
        assert!(!registry.is_healthy("sweeper"));
        assert!(!registry.all_healthy());
    }

    #[test]
    fn test_heartbeat_transitions_to_healthy() {
        let registry = ServiceHealthRegistry::new(&["sweeper"]);
        assert!(!registry.is_healthy("sweeper"));

        registry.heartbeat("sweeper");
        assert!(registry.is_healthy("sweeper"));
        assert_eq!(registry.status("sweeper"), ServiceStatus::Healthy);
    }

    #[test]
    fn test_all_healthy_requires_all() {
        let registry = ServiceHealthRegistry::new(&["a", "b"]);
        registry.heartbeat("a");
        assert!(!registry.all_healthy());

        registry.heartbeat("b");
        assert!(registry.all_healthy());
    }

    #[test]
    fn test_unknown_service_returns_starting() {
        let registry = ServiceHealthRegistry::new(&["sweeper"]);
        assert_eq!(registry.status("unknown"), ServiceStatus::Starting);
    }

    #[test]
    fn test_heartbeat_count_increments() {
        let registry = ServiceHealthRegistry::new(&["sweeper"]);
        registry.heartbeat("sweeper");
        registry.heartbeat("sweeper");
        registry.heartbeat("sweeper");
        let statuses = registry.all_statuses();
        assert_eq!(statuses.len(), 1);
        // Verify it's healthy (count > 0 and not stale)
        assert!(registry.is_healthy("sweeper"));
    }
}
