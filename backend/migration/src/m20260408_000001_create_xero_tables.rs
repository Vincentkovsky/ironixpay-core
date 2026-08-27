//! Migration: Create xero_connections and xero_sync_logs tables
//!
//! Supports Xero accounting integration: OAuth connections + per-session sync tracking.

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
                CREATE TABLE xero_connections (
                    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                    merchant_id               VARCHAR NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
                    environment               VARCHAR(10) NOT NULL,

                    access_token_encrypted    TEXT NOT NULL,
                    refresh_token_encrypted   TEXT NOT NULL,
                    token_expires_at          TIMESTAMPTZ NOT NULL,

                    xero_tenant_id            VARCHAR(64) NOT NULL,
                    xero_tenant_name          VARCHAR(255),

                    xero_account_code         VARCHAR(20),
                    xero_fee_account_code     VARCHAR(20),
                    xero_payment_account_code VARCHAR(20),
                    xero_contact_id           VARCHAR(64),
                    default_currency          VARCHAR(3) NOT NULL DEFAULT 'USD',
                    auto_sync_enabled         BOOLEAN NOT NULL DEFAULT TRUE,

                    status                    VARCHAR(20) NOT NULL DEFAULT 'active',
                    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at                TIMESTAMPTZ NOT NULL DEFAULT now(),

                    UNIQUE (merchant_id, environment)
                );

                CREATE TABLE xero_sync_logs (
                    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                    connection_id     UUID NOT NULL REFERENCES xero_connections(id) ON DELETE CASCADE,
                    session_id        VARCHAR(64) NOT NULL,

                    xero_invoice_id   VARCHAR(64),
                    xero_payment_id   VARCHAR(64),

                    status            VARCHAR(20) NOT NULL DEFAULT 'pending',
                    attempt_count     INT NOT NULL DEFAULT 0,
                    last_error        TEXT,
                    next_retry_at     TIMESTAMPTZ,

                    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

                    UNIQUE (connection_id, session_id)
                );

                CREATE INDEX idx_xero_sync_pending
                    ON xero_sync_logs(status, next_retry_at)
                    WHERE status IN ('pending', 'failed');
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
                DROP TABLE IF EXISTS xero_sync_logs;
                DROP TABLE IF EXISTS xero_connections;
                "#,
            )
            .await?;
        Ok(())
    }
}
