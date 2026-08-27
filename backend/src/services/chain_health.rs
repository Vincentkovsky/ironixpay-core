//! Chain Health Registry
//!
//! Thread-safe, lock-free registry tracking per-chain indexer health.
//! Used by:
//! - **Supervisor**: mark_starting(), mark_unhealthy() on task lifecycle events
//! - **Indexer tasks**: mark_healthy() after successful RPC calls
//! - **Checkout API**: is_healthy() as circuit breaker to reject orders for unhealthy chains
//! - **Health endpoint**: all_statuses() for /ready probe

use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::entity::Network;

/// Staleness threshold: if mark_healthy() hasn't been called within this duration,
/// is_healthy() returns false even if the recorded status is Healthy.
/// This catches "alive but hung" scenarios (e.g., RPC half-open connection).
const HEALTH_STALENESS_THRESHOLD: Duration = Duration::from_secs(120);

/// Chain lifecycle: Starting → Healthy ↔ Unhealthy
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Supervisor spawned the task, awaiting first successful RPC call
    Starting,
    /// Task's main loop is actively processing blocks
    Healthy,
    /// Task exited with error, supervisor is retrying with backoff
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Starting => write!(f, "starting"),
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Snapshot of a single chain's health state.
#[derive(Clone, Debug)]
pub struct ChainStatus {
    pub status: HealthStatus,
    pub last_updated: Instant,
    pub error_message: Option<String>,
    pub restart_count: u32,
}

/// Thread-safe registry tracking per-chain health.
///
/// Cheap to clone — inner state is `Arc<DashMap>`.
#[derive(Clone)]
pub struct ChainHealthRegistry {
    statuses: Arc<DashMap<Network, ChainStatus>>,
}

impl ChainHealthRegistry {
    /// Create a new registry for the given enabled networks.
    /// Only these networks will appear in the registry.
    /// All start as `Starting` (awaiting first indexer heartbeat).
    pub fn new(enabled_networks: &[Network]) -> Self {
        let statuses = DashMap::new();
        for network in enabled_networks {
            statuses.insert(
                network.clone(),
                ChainStatus {
                    status: HealthStatus::Starting,
                    last_updated: Instant::now(),
                    error_message: None,
                    restart_count: 0,
                },
            );
        }
        Self {
            statuses: Arc::new(statuses),
        }
    }

    /// Called by the **task itself** after a successful RPC call
    /// (e.g., `get_current_block()` returns Ok).
    ///
    /// This is the ONLY way to transition to Healthy, ensuring no false positives.
    pub fn mark_healthy(&self, network: &Network) {
        if let Some(mut entry) = self.statuses.get_mut(network) {
            // Only log transition, not every heartbeat
            if entry.status != HealthStatus::Healthy {
                tracing::info!(
                    network = %network.as_str(),
                    prev_status = %entry.status,
                    "Chain health → healthy"
                );
            }
            entry.status = HealthStatus::Healthy;
            entry.last_updated = Instant::now();
            entry.error_message = None;
        }
    }

    /// Called by the **supervisor** when a task exits with an error.
    pub fn mark_unhealthy(&self, network: &Network, error: String) {
        if let Some(mut entry) = self.statuses.get_mut(network) {
            tracing::warn!(
                network = %network.as_str(),
                error = %error,
                restart_count = entry.restart_count,
                "Chain health → unhealthy"
            );
            entry.status = HealthStatus::Unhealthy;
            entry.last_updated = Instant::now();
            entry.error_message = Some(error);
            entry.restart_count += 1;
        }
    }

    /// Called by the **supervisor** before spawning a new task attempt.
    pub fn mark_starting(&self, network: &Network) {
        if let Some(mut entry) = self.statuses.get_mut(network) {
            tracing::info!(
                network = %network.as_str(),
                restart_count = entry.restart_count,
                "Chain health → starting"
            );
            entry.status = HealthStatus::Starting;
            entry.last_updated = Instant::now();
        }
    }

    /// Circuit breaker check: returns `true` ONLY if:
    /// 1. Status is `Healthy`, AND
    /// 2. `mark_healthy()` was called within the staleness threshold (2 min)
    ///
    /// `Starting` and `Unhealthy` both return `false`.
    /// A stale `Healthy` (indexer hung without crashing) also returns `false`.
    pub fn is_healthy(&self, network: &Network) -> bool {
        self.statuses.get(network).map_or(false, |s| {
            s.status == HealthStatus::Healthy
                && s.last_updated.elapsed() < HEALTH_STALENESS_THRESHOLD
        })
    }

    /// Snapshot of all chain statuses. Used by `/ready` endpoint.
    pub fn all_statuses(&self) -> Vec<(Network, ChainStatus)> {
        self.statuses
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Returns true if ALL registered chains are healthy (non-stale).
    /// Used by `/ready` for strict readiness semantics.
    pub fn all_healthy(&self) -> bool {
        self.statuses.iter().all(|entry| {
            entry.status == HealthStatus::Healthy
                && entry.last_updated.elapsed() < HEALTH_STALENESS_THRESHOLD
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_starts_as_starting() {
        let registry = ChainHealthRegistry::new(&[Network::Tron, Network::Bsc]);
        assert!(!registry.is_healthy(&Network::Tron));
        assert!(!registry.is_healthy(&Network::Bsc));
        let statuses = registry.all_statuses();
        assert_eq!(statuses.len(), 2);
        for (_, s) in &statuses {
            assert_eq!(s.status, HealthStatus::Starting);
        }
    }

    #[test]
    fn test_mark_healthy_enables_circuit_breaker() {
        let registry = ChainHealthRegistry::new(&[Network::Tron]);
        assert!(!registry.is_healthy(&Network::Tron));

        registry.mark_healthy(&Network::Tron);
        assert!(registry.is_healthy(&Network::Tron));
    }

    #[test]
    fn test_mark_unhealthy_disables_circuit_breaker() {
        let registry = ChainHealthRegistry::new(&[Network::Tron]);
        registry.mark_healthy(&Network::Tron);
        assert!(registry.is_healthy(&Network::Tron));

        registry.mark_unhealthy(&Network::Tron, "RPC timeout".into());
        assert!(!registry.is_healthy(&Network::Tron));
    }

    #[test]
    fn test_restart_count_increments() {
        let registry = ChainHealthRegistry::new(&[Network::Bsc]);
        registry.mark_unhealthy(&Network::Bsc, "err1".into());
        registry.mark_unhealthy(&Network::Bsc, "err2".into());
        let statuses = registry.all_statuses();
        let bsc = statuses.iter().find(|(n, _)| *n == Network::Bsc).unwrap();
        assert_eq!(bsc.1.restart_count, 2);
    }

    #[test]
    fn test_all_healthy() {
        let registry = ChainHealthRegistry::new(&[Network::Tron, Network::Bsc]);
        assert!(!registry.all_healthy());

        registry.mark_healthy(&Network::Tron);
        assert!(!registry.all_healthy());

        registry.mark_healthy(&Network::Bsc);
        assert!(registry.all_healthy());
    }

    #[test]
    fn test_unknown_network_is_not_healthy() {
        let registry = ChainHealthRegistry::new(&[Network::Tron]);
        assert!(!registry.is_healthy(&Network::Bsc)); // Not registered
    }
}
