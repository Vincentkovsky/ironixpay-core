//! Migration: Add FX snapshot fields to xero_sync_logs
//!
//! Stores deterministic conversion snapshot for Xero sync retries.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE xero_sync_logs
                    ADD COLUMN IF NOT EXISTS fx_rate NUMERIC(28, 10),
                    ADD COLUMN IF NOT EXISTS fx_source_currency VARCHAR(10),
                    ADD COLUMN IF NOT EXISTS fx_target_currency VARCHAR(10),
                    ADD COLUMN IF NOT EXISTS converted_gross NUMERIC(20, 6),
                    ADD COLUMN IF NOT EXISTS converted_fee NUMERIC(20, 6),
                    ADD COLUMN IF NOT EXISTS converted_net NUMERIC(20, 6);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE xero_sync_logs
                    DROP COLUMN IF EXISTS converted_net,
                    DROP COLUMN IF EXISTS converted_fee,
                    DROP COLUMN IF EXISTS converted_gross,
                    DROP COLUMN IF EXISTS fx_target_currency,
                    DROP COLUMN IF EXISTS fx_source_currency,
                    DROP COLUMN IF EXISTS fx_rate;
                "#,
            )
            .await?;
        Ok(())
    }
}
