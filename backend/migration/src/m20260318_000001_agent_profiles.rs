//! Migration: Agent Profiles & Referral System
//!
//! Creates `agent_profiles` table, adds `referred_by_agent_id` to merchants,
//! and adds `gross_amount` + `fee_amount` to billing_logs for commission tracking.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // 1. Create agent_profiles table (VARCHAR id — matches codebase convention, Rust generates via Uuid::new_v4)
        conn.execute_unprepared(
            r#"
            CREATE TABLE agent_profiles (
                id                    VARCHAR PRIMARY KEY,
                merchant_id           VARCHAR NOT NULL UNIQUE REFERENCES merchants(id) ON DELETE CASCADE,
                referral_code         VARCHAR(50) NOT NULL UNIQUE,
                base_rate             DECIMAL(5,4) NOT NULL DEFAULT 0.0010,
                max_markup            DECIMAL(5,4) NOT NULL DEFAULT 0.0040,
                default_merchant_rate DECIMAL(5,4) NOT NULL DEFAULT 0.0040,
                status                VARCHAR(20) NOT NULL DEFAULT 'active',
                created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX idx_agent_referral_code ON agent_profiles(referral_code);
            CREATE INDEX idx_agent_merchant_id ON agent_profiles(merchant_id);
            "#,
        )
        .await?;

        // 2. Add referred_by_agent_id to merchants (VARCHAR to match agent_profiles.id String mapping)
        conn.execute_unprepared(
            r#"
            ALTER TABLE merchants
                ADD COLUMN referred_by_agent_id VARCHAR REFERENCES agent_profiles(id);

            CREATE INDEX idx_merchants_referred_by ON merchants(referred_by_agent_id)
                WHERE referred_by_agent_id IS NOT NULL;
            "#,
        )
        .await?;

        // 3. Add gross_amount + fee_amount to billing_logs (for commission calculation)
        conn.execute_unprepared(
            r#"
            ALTER TABLE billing_logs
                ADD COLUMN gross_amount BIGINT,
                ADD COLUMN fee_amount   BIGINT;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        conn.execute_unprepared(
            r#"
            ALTER TABLE billing_logs
                DROP COLUMN IF EXISTS gross_amount,
                DROP COLUMN IF EXISTS fee_amount;

            DROP INDEX IF EXISTS idx_merchants_referred_by;
            ALTER TABLE merchants
                DROP COLUMN IF EXISTS referred_by_agent_id;

            DROP TABLE IF EXISTS agent_profiles;
            "#,
        )
        .await?;

        Ok(())
    }
}
