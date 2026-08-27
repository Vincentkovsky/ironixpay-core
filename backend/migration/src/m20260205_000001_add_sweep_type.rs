//! Add sweep_type and exception_id to sweep_transactions
//!
//! This migration unifies all outbound transactions (auto sweep, manual sweep, manual transfer)
//! into a single table for unified confirmation tracking and audit.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add sweep_type column (default: auto_sweep for existing records)
        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .add_column(
                        ColumnDef::new(SweepTransactions::SweepType)
                            .string()
                            .not_null()
                            .default("auto_sweep"),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Add exception_id column (nullable, for manual sweep/transfer)
        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .add_column(
                        ColumnDef::new(SweepTransactions::ExceptionId)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Add foreign key constraint to payment_exceptions
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_sweep_transactions_exception")
                    .from(SweepTransactions::Table, SweepTransactions::ExceptionId)
                    .to(PaymentExceptions::Table, PaymentExceptions::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        // 4. Add check constraint: at least one source must exist
        // Note: SeaORM doesn't have built-in check constraint support,
        // so we use raw SQL
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE sweep_transactions
                ADD CONSTRAINT check_source_exists
                CHECK (session_id IS NOT NULL OR exception_id IS NOT NULL)
                "#,
            )
            .await?;

        // 5. Add index on exception_id for efficient lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_sweep_transactions_exception")
                    .table(SweepTransactions::Table)
                    .col(SweepTransactions::ExceptionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove check constraint first
        manager
            .get_connection()
            .execute_unprepared(
                r#"ALTER TABLE sweep_transactions DROP CONSTRAINT IF EXISTS check_source_exists"#,
            )
            .await?;

        // Remove index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sweep_transactions_exception")
                    .table(SweepTransactions::Table)
                    .to_owned(),
            )
            .await?;

        // Remove foreign key
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_sweep_transactions_exception")
                    .table(SweepTransactions::Table)
                    .to_owned(),
            )
            .await?;

        // Remove columns
        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .drop_column(SweepTransactions::ExceptionId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .drop_column(SweepTransactions::SweepType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum SweepTransactions {
    Table,
    SweepType,
    ExceptionId,
}

#[derive(Iden)]
enum PaymentExceptions {
    Table,
    Id,
}
