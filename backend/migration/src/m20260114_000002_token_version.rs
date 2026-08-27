//! Migration: Add token_version for JWT revocation
//!
//! Enables immediate token invalidation by incrementing token_version.
//! All JWTs issued before the increment become invalid.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add token_version column with default 0
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .add_column(
                        ColumnDef::new(Merchants::TokenVersion)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .drop_column(Merchants::TokenVersion)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Merchants {
    Table,
    TokenVersion,
}
