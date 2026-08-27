//! Migration: Add backup_codes field to merchants table
//!
//! Adds support for 2FA backup codes (recovery codes).
//! Backup codes are stored as JSON array with SHA-256 hashes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add backup_codes column (nullable TEXT for JSON storage)
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .add_column(
                        ColumnDef::new(Merchants::BackupCodes).text().null(), // NULL when 2FA not enabled or no backup codes
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove backup_codes column
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .drop_column(Merchants::BackupCodes)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Merchants {
    Table,
    BackupCodes,
}
