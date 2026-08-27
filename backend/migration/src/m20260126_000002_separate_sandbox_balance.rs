use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Rename gas_credit_balance -> balance_prod
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .rename_column(Merchants::GasCreditBalance, Merchants::BalanceProd)
                    .to_owned(),
            )
            .await?;

        // 2. Add balance_sandbox column (default 0)
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .add_column(
                        ColumnDef::new(Merchants::BalanceSandbox)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop balance_sandbox
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .drop_column(Merchants::BalanceSandbox)
                    .to_owned(),
            )
            .await?;

        // 2. Rename balance_prod -> gas_credit_balance
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .rename_column(Merchants::BalanceProd, Merchants::GasCreditBalance)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Merchants {
    Table,
    GasCreditBalance,
    BalanceProd,
    BalanceSandbox,
}
