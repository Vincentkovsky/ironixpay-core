//! Migration: Payout Risk Control
//!
//! Creates payout_settings and payout_trusted_addresses tables,
//! adds approval-related columns to payouts and withdrawals tables.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // 1. Create payout_settings table (1:1 with merchants)
        conn.execute_unprepared(
            r#"
            CREATE TABLE payout_settings (
                id              VARCHAR NOT NULL PRIMARY KEY,
                merchant_id     VARCHAR NOT NULL UNIQUE REFERENCES merchants(id) ON DELETE CASCADE,
                risk_control_enabled       BOOLEAN NOT NULL DEFAULT false,
                require_new_address_approval BOOLEAN NOT NULL DEFAULT true,
                approval_threshold         BIGINT NOT NULL DEFAULT 5000000000,
                approver_roles             JSONB NOT NULL DEFAULT '["owner","admin"]'::jsonb,
                auto_withdraw_enabled      BOOLEAN NOT NULL DEFAULT false,
                auto_withdraw_threshold    BIGINT,
                auto_withdraw_network      VARCHAR,
                auto_withdraw_currency     VARCHAR NOT NULL DEFAULT 'USDT',
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .await?;

        // 2. Create payout_trusted_addresses table
        conn.execute_unprepared(
            r#"
            CREATE TABLE payout_trusted_addresses (
                id              VARCHAR NOT NULL PRIMARY KEY,
                merchant_id     VARCHAR NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
                network         VARCHAR NOT NULL,
                address         VARCHAR NOT NULL,
                first_used_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_used_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                total_payouts   INTEGER NOT NULL DEFAULT 1,
                CONSTRAINT uq_trusted_addr UNIQUE (merchant_id, network, address)
            );

            CREATE INDEX idx_trusted_addr_merchant ON payout_trusted_addresses (merchant_id, network);
            "#,
        )
        .await?;

        // 3. Add approval columns to payouts table
        conn.execute_unprepared(
            r#"
            ALTER TABLE payouts
                ADD COLUMN reviewed_by VARCHAR,
                ADD COLUMN reviewed_at TIMESTAMPTZ;
            "#,
        )
        .await?;

        // 4. Add approval columns to withdrawals table
        conn.execute_unprepared(
            r#"
            ALTER TABLE withdrawals
                ADD COLUMN requested_by VARCHAR,
                ADD COLUMN reviewed_by  VARCHAR,
                ADD COLUMN reviewed_at  TIMESTAMPTZ;
            "#,
        )
        .await?;

        // 5. Index for auto-expire worker: find PendingApproval records efficiently
        conn.execute_unprepared(
            r#"
            CREATE INDEX idx_payouts_pending_approval
                ON payouts (created_at)
                WHERE status = 'PendingApproval';

            CREATE INDEX idx_withdrawals_pending_approval
                ON withdrawals (created_at)
                WHERE status = 'PendingApproval';
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        conn.execute_unprepared(
            r#"
            DROP INDEX IF EXISTS idx_withdrawals_pending_approval;
            DROP INDEX IF EXISTS idx_payouts_pending_approval;

            ALTER TABLE withdrawals
                DROP COLUMN IF EXISTS requested_by,
                DROP COLUMN IF EXISTS reviewed_by,
                DROP COLUMN IF EXISTS reviewed_at;

            ALTER TABLE payouts
                DROP COLUMN IF EXISTS reviewed_by,
                DROP COLUMN IF EXISTS reviewed_at;

            DROP TABLE IF EXISTS payout_trusted_addresses;
            DROP TABLE IF EXISTS payout_settings;
            "#,
        )
        .await?;

        Ok(())
    }
}
