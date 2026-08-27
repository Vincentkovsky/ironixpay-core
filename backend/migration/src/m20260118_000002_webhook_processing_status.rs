//! Add 'processing' status to webhook_events
//!
//! This migration adds the 'processing' status to prevent concurrent webhook delivery
//! and improve reliability of the webhook system.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Note: The 'processing' status is added to the enum in the application code.
        // This migration serves as a marker for the schema change.
        // PostgreSQL/MySQL will accept the new enum value automatically when inserted.

        // For databases that strictly validate enum values, you would need to:
        // ALTER TYPE webhook_event_status ADD VALUE 'processing';
        // However, SeaORM uses string-based enums, so no DB change is needed.

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Removing an enum value is complex and risky in production.
        // This is a no-op migration for down.
        Ok(())
    }
}
