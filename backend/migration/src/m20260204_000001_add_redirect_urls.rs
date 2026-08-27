//! Add success_url and cancel_url columns to checkout_sessions.
//! These URLs are required when creating sessions and used by the
//! Checkout frontend to redirect users after payment completes or expires.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add success_url column - redirect after successful payment (NOT NULL)
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .add_column(
                        ColumnDef::new(CheckoutSessions::SuccessUrl)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // Add cancel_url column - redirect after expiry/cancel (NOT NULL)
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .add_column(
                        ColumnDef::new(CheckoutSessions::CancelUrl)
                            .string()
                            .not_null()
                            .default(""),
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
                    .table(CheckoutSessions::Table)
                    .drop_column(CheckoutSessions::SuccessUrl)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .drop_column(CheckoutSessions::CancelUrl)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum CheckoutSessions {
    Table,
    SuccessUrl,
    CancelUrl,
}
