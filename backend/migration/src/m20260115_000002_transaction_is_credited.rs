//! Add is_credited column to transactions table
//!
//! This field tracks whether the transaction amount has been credited
//! to the session's amount_received. Used for idempotent payment processing.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add is_credited column with default false
        manager
            .alter_table(
                Table::alter()
                    .table(Transactions::Table)
                    .add_column(
                        ColumnDef::new(Transactions::IsCredited)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for querying uncredited transactions
        manager
            .create_index(
                Index::create()
                    .name("idx_transactions_is_credited")
                    .table(Transactions::Table)
                    .col(Transactions::IsCredited)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_transactions_is_credited")
                    .table(Transactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Transactions::Table)
                    .drop_column(Transactions::IsCredited)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Transactions {
    Table,
    IsCredited,
}
