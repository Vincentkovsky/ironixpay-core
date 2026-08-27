//! Indexer State table for persisting last processed block
//!
//! Ensures block scanning resumes correctly after service restart.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create indexer_state table
        manager
            .create_table(
                Table::create()
                    .table(IndexerState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IndexerState::Network)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(IndexerState::LastProcessedBlock)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexerState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IndexerState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum IndexerState {
    Table,
    Network,
    LastProcessedBlock,
    UpdatedAt,
}
