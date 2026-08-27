//! Add tiered pricing fields to merchants table.
//!
//! - fee_tier: current tier (business/enterprise/standard)
//! - fee_source: who set the fee (default/auto_tier/manual/agent)
//! - first_month_ends_at: when the 30-day promo expires
//! - last_month_volume: cached last month's volume (microunits)
//! - tier_updated_at: when tier was last recalculated

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Add new columns
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants
              ADD COLUMN fee_tier VARCHAR(20) NOT NULL DEFAULT 'business',
              ADD COLUMN fee_source VARCHAR(20) NOT NULL DEFAULT 'default',
              ADD COLUMN first_month_ends_at TIMESTAMPTZ,
              ADD COLUMN last_month_volume BIGINT NOT NULL DEFAULT 0,
              ADD COLUMN tier_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            "#,
        )
        .await?;

        // Backfill: set first_month_ends_at for all existing merchants
        db.execute_unprepared(
            "UPDATE merchants SET first_month_ends_at = created_at + INTERVAL '30 days'",
        )
        .await?;

        // Backfill: mark agent-set fees
        db.execute_unprepared(
            r#"
            UPDATE merchants SET fee_source = 'agent'
            WHERE custom_fee_percentage IS NOT NULL
              AND referred_by_agent_id IS NOT NULL
            "#,
        )
        .await?;

        // Backfill: mark admin-set fees (non-agent)
        db.execute_unprepared(
            r#"
            UPDATE merchants SET fee_source = 'manual'
            WHERE custom_fee_percentage IS NOT NULL
              AND referred_by_agent_id IS NULL
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants
              DROP COLUMN IF EXISTS fee_tier,
              DROP COLUMN IF EXISTS fee_source,
              DROP COLUMN IF EXISTS first_month_ends_at,
              DROP COLUMN IF EXISTS last_month_volume,
              DROP COLUMN IF EXISTS tier_updated_at
            "#,
        )
        .await?;
        Ok(())
    }
}
