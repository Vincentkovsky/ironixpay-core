//! Add chain_head_block column to indexer_state table.
//!
//! Stores the real-time chain head fetched during each indexer poll cycle,
//! so the admin console can display blocks_behind without extra RPC calls.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("indexer_state"))
                    .add_column(
                        ColumnDef::new(Alias::new("chain_head_block"))
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("indexer_state"))
                    .drop_column(Alias::new("chain_head_block"))
                    .to_owned(),
            )
            .await
    }
}
