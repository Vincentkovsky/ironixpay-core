//! Add custom_fee_percentage to merchants table.
//!
//! Allows per-merchant fee override. NULL = use global default (1%).
//! Stored as decimal fraction (0.01 = 1%, 0.005 = 0.5%).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE merchants ADD COLUMN custom_fee_percentage DECIMAL(5,4) NULL",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE merchants DROP COLUMN IF EXISTS custom_fee_percentage")
            .await?;
        Ok(())
    }
}
