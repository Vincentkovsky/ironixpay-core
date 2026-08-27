//! Add session_id to sweep_transactions table
//!
//! This migration adds an optional session_id field to link sweeps directly to sessions.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add session_id column (nullable - orphan funds may not have a session)
        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .add_column(ColumnDef::new(SweepTransactions::SessionId).string().null())
                    .to_owned(),
            )
            .await?;

        // Add index for session_id lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_sweep_transactions_session")
                    .table(SweepTransactions::Table)
                    .col(SweepTransactions::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sweep_transactions_session")
                    .table(SweepTransactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .drop_column(SweepTransactions::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum SweepTransactions {
    Table,
    SessionId,
}
