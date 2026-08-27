//! Rename webhook_events.resource_id → source_id.
//!
//! Better semantics: "the source entity that triggered this webhook event".

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Rename column
        db.execute_unprepared("ALTER TABLE webhook_events RENAME COLUMN resource_id TO source_id")
            .await?;

        // Recreate index with new name
        db.execute_unprepared("DROP INDEX IF EXISTS idx_webhook_events_resource")
            .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_webhook_events_source ON webhook_events(source_id, created_at)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP INDEX IF EXISTS idx_webhook_events_source")
            .await?;
        db.execute_unprepared("ALTER TABLE webhook_events RENAME COLUMN source_id TO resource_id")
            .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_webhook_events_resource ON webhook_events(resource_id, created_at)",
        )
        .await?;

        Ok(())
    }
}
