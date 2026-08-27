//! Migration: Reserve account_index=0 for platform (treasury/gas_sponsor)
//!
//! Adds a CHECK constraint to ensure no merchant is ever assigned account_index=0.
//! account_index=0 is used by the platform for HD-derived treasury (path_index=0)
//! and gas sponsor (path_index=1) addresses.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Safety check: ensure no merchant already has account_index=0
        // If any does, this migration will fail — fix data first.
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants
            ADD CONSTRAINT account_index_reserved_for_platform
            CHECK (account_index >= 1);
            "#,
        )
        .await?;

        // Also ensure the sequence never produces 0
        // (it starts from MAX+1 which should already be >= 1)
        db.execute_unprepared(
            r#"
            ALTER SEQUENCE merchant_account_index_seq MINVALUE 1;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            ALTER TABLE merchants DROP CONSTRAINT IF EXISTS account_index_reserved_for_platform;
            ALTER SEQUENCE merchant_account_index_seq MINVALUE 0;
            "#,
        )
        .await?;

        Ok(())
    }
}
