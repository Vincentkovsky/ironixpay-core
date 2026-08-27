//! Add performance indexes for webhook_events table
//!
//! These indexes optimize the recovery loop queries for webhook delivery.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Index for finding failed events ready for retry
        // Covers: WHERE status = 'failed' AND next_retry_at <= NOW()
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_events_retry")
                    .if_not_exists()
                    .table(WebhookEvents::Table)
                    .col(WebhookEvents::Status)
                    .col(WebhookEvents::NextRetryAt)
                    .to_owned(),
            )
            .await?;

        // Index for finding stale processing events
        // Covers: WHERE status = 'processing' AND last_attempt_at < threshold
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_events_stale")
                    .if_not_exists()
                    .table(WebhookEvents::Table)
                    .col(WebhookEvents::Status)
                    .col(WebhookEvents::LastAttemptAt)
                    .to_owned(),
            )
            .await?;

        // Index for merchant webhook history queries
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_events_merchant")
                    .if_not_exists()
                    .table(WebhookEvents::Table)
                    .col(WebhookEvents::MerchantId)
                    .col(WebhookEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Index for session webhook history queries
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_events_session")
                    .if_not_exists()
                    .table(WebhookEvents::Table)
                    .col(WebhookEvents::SessionId)
                    .col(WebhookEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_webhook_events_session")
                    .table(WebhookEvents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_webhook_events_merchant")
                    .table(WebhookEvents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_webhook_events_stale")
                    .table(WebhookEvents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_webhook_events_retry")
                    .table(WebhookEvents::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum WebhookEvents {
    Table,
    Status,
    NextRetryAt,
    LastAttemptAt,
    MerchantId,
    SessionId,
    CreatedAt,
}
