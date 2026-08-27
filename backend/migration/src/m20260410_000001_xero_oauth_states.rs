//! Migration: Add persistent storage for Xero OAuth state nonces
//!
//! This enables one-time state verification across process restarts and
//! multi-instance deployments.

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
                CREATE TABLE xero_oauth_states (
                    nonce       VARCHAR(64) PRIMARY KEY,
                    merchant_id VARCHAR NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
                    environment VARCHAR(10) NOT NULL,
                    expires_at  TIMESTAMPTZ NOT NULL,
                    consumed_at TIMESTAMPTZ,
                    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
                );

                CREATE INDEX idx_xero_oauth_states_expires_at
                    ON xero_oauth_states(expires_at);

                CREATE INDEX idx_xero_oauth_states_merchant_env
                    ON xero_oauth_states(merchant_id, environment);
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
                DROP INDEX IF EXISTS idx_xero_oauth_states_merchant_env;
                DROP INDEX IF EXISTS idx_xero_oauth_states_expires_at;
                DROP TABLE IF EXISTS xero_oauth_states;
                "#,
            )
            .await?;
        Ok(())
    }
}
