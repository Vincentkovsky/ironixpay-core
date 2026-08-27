//! Address Cache Sync Manager
//!
//! Provides real-time address cache synchronization using two mechanisms:
//! 1. Primary: PostgreSQL LISTEN/NOTIFY for instant updates (< 10ms)
//! 2. Fallback: Periodic created_at rollback query (every 60s)
//!
//! This eliminates the "stale cache" vulnerability where payments to
//! newly created addresses were missed during the 5-minute refresh window.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, error, info, warn};

use crate::entity::{addresses, Addresses, Environment, Network};

use super::MonitoredAddressInfo;

/// Fallback sync interval (60 seconds)
const FALLBACK_SYNC_INTERVAL: Duration = Duration::from_secs(60);

/// Rollback window for created_at query (5 minutes)
/// This ensures we don't miss any addresses even if there's clock skew
const ROLLBACK_WINDOW_MINUTES: i32 = 5;

/// Manages real-time address cache synchronization
pub struct AddressSyncManager {
    /// Database URL for dedicated LISTEN connection
    db_url: String,
    /// SeaORM connection for fallback queries
    db: DatabaseConnection,
    /// Network this sync manager monitors
    network: Network,
    /// Environment for chain name resolution
    _environment: Environment,
    /// Shared address cache (DashMap for concurrent access)
    cache: Arc<DashMap<String, MonitoredAddressInfo>>,
}

// ... (struct definition)

#[derive(serde::Deserialize)]
struct NotifyPayload {
    network: String,
    address: String,
}

impl AddressSyncManager {
    // ... (new, start_notification_listener same)

    pub fn new(
        db_url: String,
        db: DatabaseConnection,
        network: Network,
        environment: Environment,
        cache: Arc<DashMap<String, MonitoredAddressInfo>>,
    ) -> Self {
        Self {
            db_url,
            db,
            network,
            _environment: environment,
            cache,
        }
    }

    /// Start the LISTEN/NOTIFY subscriber
    ///
    /// This creates a dedicated connection to PostgreSQL and listens for
    /// 'address_created' notifications. When a new address is inserted,
    /// the trigger fires and this listener updates the cache immediately.
    ///
    /// Auto-reconnects on connection failure.
    pub async fn start_notification_listener(
        self: Arc<Self>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        info!(
            network = %self.network.as_str(),
            "Starting LISTEN/NOTIFY address sync"
        );

        // We need to use sqlx directly for PgListener
        // SeaORM's underlying sqlx pool can be accessed, but PgListener needs its own connection
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("LISTEN/NOTIFY listener received shutdown signal");
                    break;
                }
                result = self.run_listener(token.clone()) => {
                    match result {
                        Ok(()) => {
                            // Clean exit (shouldn't happen normally unless shutdown)
                            info!("LISTEN/NOTIFY listener exited cleanly");
                            break;
                        }
                        Err(e) => {
                            error!(error = %e, "LISTEN/NOTIFY listener error, reconnecting in 5s...");
                            // Wait before reconnecting, but respect cancellation
                            tokio::select! {
                                _ = token.cancelled() => {
                                    info!("LISTEN/NOTIFY listener received shutdown signal during backoff");
                                    break;
                                }
                                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Internal: Run the listener loop
    async fn run_listener(&self, token: tokio_util::sync::CancellationToken) -> Result<()> {
        use sqlx::postgres::PgListener;

        let mut listener: sqlx::postgres::PgListener = PgListener::connect(&self.db_url).await?;
        listener.listen("address_created").await?;

        info!(
            network = %self.network.as_str(),
            "LISTEN/NOTIFY connected, waiting for notifications..."
        );

        loop {
            tokio::select! {
                    _ = token.cancelled() => {
                        return Ok(());
                    }
                    notification_res = listener.recv() => {
                        let notification: sqlx::postgres::PgNotification = notification_res?;

                        // Parse the JSON payload with strong typing
                        match serde_json::from_str::<NotifyPayload>(notification.payload()) {
                    Ok(payload) => {
                        // Only process notifications for our network
                        if payload.network != self.network.as_str() {
                            continue;
                        }
                        if payload.address.is_empty() {
                            continue;
                        }

                        // Unified Cache Strategy: Query DB to get full metadata
                        // The notification only contains the address, but cache needs merchant_id/status
                        let address_info = match Addresses::find()
                            .filter(addresses::Column::Network.eq(self.network.as_str()))
                            .filter(addresses::Column::Address.eq(&payload.address))
                            .one(&self.db)
                            .await
                        {
                            Ok(Some(addr)) => MonitoredAddressInfo {
                                merchant_id: addr.merchant_id,
                            },
                            Ok(None) => {
                                warn!(address = %payload.address, "Received notification for address not found in DB");
                                continue;
                            }
                            Err(e) => {
                                error!(address = %payload.address, error = %e, "Failed to query address details for cache update");
                                continue;
                            }
                        };

                        // Update cache (DashMap insert is atomic and doesn't block readers)
                        self.cache.insert(payload.address.clone(), address_info);

                        debug!(
                            network = %payload.network,
                            address = %payload.address,
                            cache_size = self.cache.len(),
                            "Hot-updated cache with new address"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, payload = %notification.payload(), "Failed to parse notification payload");
                        // Defensive: Sleep briefly to prevent CPU spin if we get flooded with bad notifications
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                    }
                }
            }
        }
    }

    /// Start the fallback sync loop
    ///
    /// Every 60 seconds, queries addresses created in the last 5 minutes.
    /// This serves as a safety net in case LISTEN/NOTIFY misses something
    /// (e.g., during reconnection).

    pub async fn start_fallback_sync(
        self: Arc<Self>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        info!(
            network = %self.network.as_str(),
            interval_secs = FALLBACK_SYNC_INTERVAL.as_secs(),
            "Starting fallback address sync"
        );

        let mut interval = tokio::time::interval(FALLBACK_SYNC_INTERVAL);

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Fallback sync received shutdown signal");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.sync_recent_addresses().await {
                        error!(error = %e, "Fallback sync failed");
                    }
                }
            }
        }
        Ok(())
    }

    /// Query addresses created in the last ROLLBACK_WINDOW_MINUTES
    async fn sync_recent_addresses(&self) -> Result<()> {
        // Calculate time threshold in Rust to ensure consistency with constants
        let time_threshold = Utc::now() - ChronoDuration::minutes(ROLLBACK_WINDOW_MINUTES as i64);

        // Unified Cache: Use safe Entity query to fetch metadata (merchant_id, status)
        let recent_addresses = Addresses::find()
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .filter(addresses::Column::CreatedAt.gt(time_threshold))
            .all(&self.db)
            .await?;

        let row_count = recent_addresses.len();
        let mut new_count = 0;

        for addr in recent_addresses {
            if self
                .cache
                .insert(
                    addr.address,
                    MonitoredAddressInfo {
                        merchant_id: addr.merchant_id,
                    },
                )
                .is_none()
            {
                new_count += 1;
            }
        }

        if new_count > 0 {
            info!(
                new_addresses = new_count,
                cache_size = self.cache.len(),
                "Fallback sync added new addresses"
            );
        } else {
            debug!(
                checked = row_count,
                cache_size = self.cache.len(),
                "Fallback sync: no new addresses"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(FALLBACK_SYNC_INTERVAL, Duration::from_secs(60));
        assert_eq!(ROLLBACK_WINDOW_MINUTES, 5);
    }
}
