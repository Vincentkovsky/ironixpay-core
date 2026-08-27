//! Migration: Add indexes and CHECK constraints to Xero tables
//!
//! - session_id single-column index for idempotent enqueue lookups
//! - CHECK constraints on status enums for data integrity

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
                -- Single-column index for enqueue_sync_if_enabled lookups
                CREATE INDEX IF NOT EXISTS idx_xero_sync_session_id
                    ON xero_sync_logs(session_id);

                -- Status enum constraints
                ALTER TABLE xero_connections
                    ADD CONSTRAINT check_xero_conn_status
                    CHECK (status IN ('active', 'pending_selection', 'disconnected', 'error'));

                ALTER TABLE xero_sync_logs
                    ADD CONSTRAINT check_xero_sync_status
                    CHECK (status IN ('pending', 'synced', 'failed', 'skipped'));
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
                ALTER TABLE xero_sync_logs DROP CONSTRAINT IF EXISTS check_xero_sync_status;
                ALTER TABLE xero_connections DROP CONSTRAINT IF EXISTS check_xero_conn_status;
                DROP INDEX IF EXISTS idx_xero_sync_session_id;
                "#,
            )
            .await?;
        Ok(())
    }
}
