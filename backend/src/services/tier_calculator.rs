//! Tier Calculator Service
//!
//! Background task that runs daily to recalculate merchant fee tiers
//! based on last month's transaction volume.
//!
//! Priority: agent/manual fee_source > auto_tier > default
//! TierCalculator only manages merchants with fee_source = default | auto_tier.

use anyhow::Result;
use chrono::{Datelike, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryFilter, Set, Statement,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::entity::merchants::{self, FeeSource, FeeTier};

/// Configuration for tier thresholds
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// Minimum volume (microunits) to qualify for Enterprise tier
    /// Default: 2_000_000_000_000 ($2M)
    pub enterprise_min_volume: i64,
    /// Enterprise tier fee percentage (decimal fraction)
    pub enterprise_pct: Decimal,
    /// Business tier fee percentage (decimal fraction)
    pub business_pct: Decimal,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            enterprise_min_volume: 2_000_000_000_000, // $2M in microunits
            enterprise_pct: Decimal::new(1, 3),       // 0.001 = 0.1%
            business_pct: Decimal::new(3, 3),         // 0.003 = 0.3%
        }
    }
}

pub struct TierCalculatorService {
    db: DatabaseConnection,
    config: TierConfig,
}

impl TierCalculatorService {
    pub fn new(db: DatabaseConnection, config: TierConfig) -> Self {
        Self { db, config }
    }

    /// Start the tier calculator as a background loop.
    /// Runs once immediately on startup, then every 24 hours.
    pub async fn start(self: Arc<Self>, token: CancellationToken) -> Result<()> {
        info!("TierCalculator started");
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("TierCalculator shutting down");
                    break;
                }
                result = self.run_check() => {
                    if let Err(e) = result {
                        error!(error = %e, "TierCalculator run_check failed");
                    }
                }
            }
            // Sleep 24 hours before next check
            tokio::select! {
                _ = token.cancelled() => {
                    info!("TierCalculator shutting down during sleep");
                    break;
                }
                _ = sleep(Duration::from_secs(86400)) => {}
            }
        }
        Ok(())
    }

    /// Run a single tier check across all eligible merchants
    async fn run_check(&self) -> Result<()> {
        let now = Utc::now();
        let current_month = now.month();
        let current_year = now.year();

        // Fetch all merchants that are NOT agent/manual controlled
        // Also exclude sub-merchant orgs — they inherit fees from parent PSP
        let merchants = merchants::Entity::find()
            .filter(merchants::Column::FeeSource.is_in(["default", "auto_tier"]))
            .filter(merchants::Column::MerchantType.ne(merchants::MerchantType::SubMerchant))
            .all(&self.db)
            .await?;

        let mut updated_count = 0;
        let mut checked_count = 0;

        for merchant in merchants {
            checked_count += 1;

            // Skip if still in first-month promo
            let in_first_month = merchant
                .first_month_ends_at
                .map(|ends| now < ends.with_timezone(&Utc))
                .unwrap_or(false);

            if in_first_month {
                debug!(merchant_id = %merchant.id, "Still in first-month promo, skipping");
                continue;
            }

            // Check if already updated this month
            let tier_month = merchant.tier_updated_at.month();
            let tier_year = merchant.tier_updated_at.year();
            if tier_month == current_month && tier_year == current_year {
                debug!(merchant_id = %merchant.id, "Already updated this month, skipping");
                continue;
            }

            // Query last month's volume
            let volume = match self.query_last_month_volume(&merchant.id).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(merchant_id = %merchant.id, error = %e, "Failed to query volume, skipping");
                    continue;
                }
            };

            // Determine new tier
            let (new_tier, new_pct) = if volume >= self.config.enterprise_min_volume {
                (FeeTier::Enterprise, self.config.enterprise_pct)
            } else {
                (FeeTier::Business, self.config.business_pct)
            };

            let tier_changed = new_tier != merchant.fee_tier;
            if tier_changed {
                info!(
                    merchant_id = %merchant.id,
                    old_tier = ?merchant.fee_tier,
                    new_tier = ?new_tier,
                    last_month_volume = volume,
                    "Merchant tier changed"
                );
            }

            // Update merchant
            let mut active: merchants::ActiveModel = merchant.into();
            active.fee_tier = Set(new_tier);
            active.fee_source = Set(FeeSource::AutoTier);
            active.custom_fee_percentage = Set(Some(new_pct));
            active.last_month_volume = Set(volume);
            active.tier_updated_at = Set(Utc::now().into());
            active.updated_at = Set(Utc::now().into());
            if let Err(e) = active.update(&self.db).await {
                warn!(error = %e, "Failed to update merchant tier, skipping");
                continue;
            }

            updated_count += 1;
        }

        info!(
            checked = checked_count,
            updated = updated_count,
            "TierCalculator run complete"
        );
        Ok(())
    }

    /// Query last month's gross volume from billing_logs
    async fn query_last_month_volume(&self, merchant_id: &str) -> Result<i64> {
        let now = Utc::now();

        // Calculate first day of this month and first day of last month
        let first_of_this_month = now
            .with_day(1)
            .unwrap()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let last_month = if now.month() == 1 {
            chrono::NaiveDate::from_ymd_opt(now.year() - 1, 12, 1).unwrap()
        } else {
            chrono::NaiveDate::from_ymd_opt(now.year(), now.month() - 1, 1).unwrap()
        };
        let first_of_last_month = last_month.and_hms_opt(0, 0, 0).unwrap().and_utc();

        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                SELECT COALESCE(SUM(gross_amount), 0)::bigint as volume
                FROM billing_logs
                WHERE merchant_id = $1
                  AND type = 'payment_credit'
                  AND created_at >= $2::timestamptz
                  AND created_at < $3::timestamptz
                "#,
                vec![
                    merchant_id.into(),
                    first_of_last_month.to_rfc3339().into(),
                    first_of_this_month.to_rfc3339().into(),
                ],
            ))
            .await?;

        let volume: i64 = row
            .map(|r| r.try_get("", "volume").unwrap_or(0))
            .unwrap_or(0);

        Ok(volume)
    }
}
