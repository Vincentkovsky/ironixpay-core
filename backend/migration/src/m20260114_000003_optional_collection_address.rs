//! Migration: Make collection_address optional
//!
//! Allows merchants to register without a collection address.
//! Security: Setting collection_address requires 2FA to be enabled first.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Make collection_address nullable
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .modify_column(
                        ColumnDef::new(Merchants::CollectionAddress).string().null(), // Change from NOT NULL to NULL
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Revert to NOT NULL (will fail if any NULL values exist)
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .modify_column(
                        ColumnDef::new(Merchants::CollectionAddress)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Merchants {
    Table,
    CollectionAddress,
}
