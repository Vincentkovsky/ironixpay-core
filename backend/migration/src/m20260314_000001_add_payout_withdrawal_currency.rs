use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add currency column to payouts table
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("payouts"))
                    .add_column(
                        ColumnDef::new(Alias::new("currency"))
                            .string()
                            .not_null()
                            .default("USDT"),
                    )
                    .to_owned(),
            )
            .await?;

        // Add currency column to withdrawals table
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("withdrawals"))
                    .add_column(
                        ColumnDef::new(Alias::new("currency"))
                            .string()
                            .not_null()
                            .default("USDT"),
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
                    .table(Alias::new("payouts"))
                    .drop_column(Alias::new("currency"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("withdrawals"))
                    .drop_column(Alias::new("currency"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
