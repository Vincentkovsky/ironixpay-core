//! Add cost_in_usdt column to sweep_transactions.
//!
//! Replaces the never-populated energy_cost + bandwidth_cost fields
//! with a single USDT-denominated cost field (6-decimal i64).
//! Old columns are kept for backward compatibility.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("sweep_transactions"))
                    .add_column(
                        ColumnDef::new(Alias::new("cost_in_usdt"))
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
                    .table(Alias::new("sweep_transactions"))
                    .drop_column(Alias::new("cost_in_usdt"))
                    .to_owned(),
            )
            .await
    }
}
