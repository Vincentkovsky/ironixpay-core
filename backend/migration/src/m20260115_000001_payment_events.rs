//! Migration: Add payment_events table for Transactional Outbox pattern
//!
//! This table decouples TransactionIndexer from CheckoutService,
//! ensuring Indexer only writes transactions and events, while
//! CheckoutService is the sole owner of session state updates.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create payment_events table
        manager
            .create_table(
                Table::create()
                    .table(PaymentEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PaymentEvents::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PaymentEvents::EventType).string().not_null())
                    .col(ColumnDef::new(PaymentEvents::SessionId).string().not_null())
                    .col(ColumnDef::new(PaymentEvents::TxNetwork).string().not_null())
                    .col(ColumnDef::new(PaymentEvents::TxHash).string().not_null())
                    .col(
                        ColumnDef::new(PaymentEvents::TxLogIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::NextRetryAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(PaymentEvents::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(PaymentEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaymentEvents::ProcessedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_payment_events_session")
                            .from(PaymentEvents::Table, PaymentEvents::SessionId)
                            .to(CheckoutSessions::Table, CheckoutSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique constraint for deduplication
        // Prevents duplicate events for the same transaction
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_events_unique_tx")
                    .table(PaymentEvents::Table)
                    .col(PaymentEvents::TxNetwork)
                    .col(PaymentEvents::TxHash)
                    .col(PaymentEvents::TxLogIndex)
                    .col(PaymentEvents::EventType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fetching pending events (outbox consumer)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_payment_events_pending
                ON payment_events(next_retry_at)
                WHERE status = 'pending'
                "#,
            )
            .await?;

        // Index for detecting stale processing events
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_payment_events_processing
                ON payment_events(updated_at)
                WHERE status = 'processing'
                "#,
            )
            .await?;

        // Index for session lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_events_session")
                    .table(PaymentEvents::Table)
                    .col(PaymentEvents::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PaymentEvents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PaymentEvents {
    Table,
    Id,
    EventType,
    SessionId,
    TxNetwork,
    TxHash,
    TxLogIndex,
    Amount,
    Status,
    AttemptCount,
    NextRetryAt,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
    ProcessedAt,
}

#[derive(Iden)]
enum CheckoutSessions {
    Table,
    Id,
}
