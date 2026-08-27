//! Rename webhook_events.session_id → resource_id and drop FK to checkout_sessions.
//!
//! Enables webhook events for non-checkout resources (payouts, withdrawals).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Drop FK constraint
        db.execute_unprepared(
            "ALTER TABLE webhook_events DROP CONSTRAINT IF EXISTS webhook_events_session_id_fkey",
        )
        .await?;

        // 2. Rename column
        db.execute_unprepared("ALTER TABLE webhook_events RENAME COLUMN session_id TO resource_id")
            .await?;

        // 3. Drop old index, create new one
        db.execute_unprepared("DROP INDEX IF EXISTS idx_webhook_events_session")
            .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_webhook_events_resource ON webhook_events(resource_id, created_at)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP INDEX IF EXISTS idx_webhook_events_resource")
            .await?;
        db.execute_unprepared("ALTER TABLE webhook_events RENAME COLUMN resource_id TO session_id")
            .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_webhook_events_session ON webhook_events(session_id, created_at)",
        )
        .await?;
        // Note: FK re-creation omitted — payout resource_ids would violate it

        Ok(())
    }
}
