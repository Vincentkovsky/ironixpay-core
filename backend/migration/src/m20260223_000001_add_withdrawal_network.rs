//! Migration: Add `network` column to `withdrawals` table.
//!
//! Defaults to 'TRON' for backward compatibility.
//! Enables PayoutService to dispatch broadcast/confirmation per-chain.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Withdrawals::Table)
                    .add_column(
                        ColumnDef::new(Withdrawals::Network)
                            .string_len(32)
                            .not_null()
                            .default("TRON"),
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
                    .table(Withdrawals::Table)
                    .drop_column(Withdrawals::Network)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Withdrawals {
    Table,
    Network,
}
