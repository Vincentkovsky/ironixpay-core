//! Add performance index for checkout_sessions lookup by address
//!
//! This index optimizes the Sweeper's session query which needs to find
//! the LATEST session for a given address (addresses are recycled).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create composite index for efficient session lookup by address
        // The Sweeper queries: SELECT * FROM checkout_sessions
        //   WHERE network = ? AND pay_address = ?
        //   ORDER BY created_at DESC LIMIT 1
        //
        // This index covers:
        // 1. Filter by network + pay_address
        // 2. Sort by created_at DESC (to get latest session for recycled addresses)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_sessions_network_address_created
                ON checkout_sessions (network, pay_address, created_at DESC);
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
                DROP INDEX IF EXISTS idx_sessions_network_address_created;
                "#,
            )
            .await?;

        Ok(())
    }
}
