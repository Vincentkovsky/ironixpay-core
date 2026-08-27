//! Payment Exceptions table for tracking abnormal payments
//!
//! Captures payments that cannot be processed normally:
//! - Late payments (session expired)
//! - Payments to idle addresses (no active session)
//! - Underpayments below threshold
//! - Payments after session completed

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create payment_exceptions table
        manager
            .create_table(
                Table::create()
                    .table(PaymentExceptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PaymentExceptions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::Network)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::TxHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::LogIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::ExceptionType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::ToAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::FromAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::CurrencySymbol)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::MerchantId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::SessionId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::BlockNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::BlockTimestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(PaymentExceptions::Resolution).string().null())
                    .col(
                        ColumnDef::new(PaymentExceptions::ResolvedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(PaymentExceptions::ResolvedBy).string().null())
                    .col(ColumnDef::new(PaymentExceptions::Notes).text().null())
                    .col(
                        ColumnDef::new(PaymentExceptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaymentExceptions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (network, tx_hash, log_index) for idempotency
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_exceptions_tx_unique")
                    .table(PaymentExceptions::Table)
                    .col(PaymentExceptions::Network)
                    .col(PaymentExceptions::TxHash)
                    .col(PaymentExceptions::LogIndex)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for querying by merchant
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_exceptions_merchant")
                    .table(PaymentExceptions::Table)
                    .col(PaymentExceptions::MerchantId)
                    .to_owned(),
            )
            .await?;

        // Index for querying pending exceptions
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_exceptions_status")
                    .table(PaymentExceptions::Table)
                    .col(PaymentExceptions::Status)
                    .to_owned(),
            )
            .await?;

        // Index for querying by address
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_exceptions_address")
                    .table(PaymentExceptions::Table)
                    .col(PaymentExceptions::Network)
                    .col(PaymentExceptions::ToAddress)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PaymentExceptions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PaymentExceptions {
    Table,
    Id,
    Network,
    TxHash,
    LogIndex,
    ExceptionType,
    ToAddress,
    FromAddress,
    Amount,
    CurrencySymbol,
    MerchantId,
    SessionId,
    BlockNumber,
    BlockTimestamp,
    Status,
    Resolution,
    ResolvedAt,
    ResolvedBy,
    Notes,
    CreatedAt,
    UpdatedAt,
}
