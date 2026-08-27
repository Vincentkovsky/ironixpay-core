use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SweepTransactions::Table)
                    .add_column(
                        ColumnDef::new(SweepTransactions::ErrorMessage)
                            .string()
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
                    .table(SweepTransactions::Table)
                    .drop_column(SweepTransactions::ErrorMessage)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SweepTransactions {
    Table,
    ErrorMessage,
}
