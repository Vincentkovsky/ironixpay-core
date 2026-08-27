use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create merchant_profiles table
        manager
            .create_table(
                Table::create()
                    .table(MerchantProfiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MerchantProfiles::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::Environment)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::Balance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(MerchantProfiles::MerchantId)
                            .col(MerchantProfiles::Environment),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MerchantProfiles::Table, MerchantProfiles::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Create merchant_chain_accounts table
        manager
            .create_table(
                Table::create()
                    .table(MerchantChainAccounts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MerchantChainAccounts::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::Environment)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::Network)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::XpubEncrypted)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::LastPathIndex)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::CollectionAddress)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MerchantChainAccounts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(MerchantChainAccounts::MerchantId)
                            .col(MerchantChainAccounts::Environment)
                            .col(MerchantChainAccounts::Network),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MerchantChainAccounts::Table,
                                MerchantChainAccounts::MerchantId,
                            )
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Migrate Data

        // Migrate Profiles (Production)
        let insert_profiles_prod = r#"
            INSERT INTO merchant_profiles (merchant_id, environment, balance, created_at, updated_at)
            SELECT id, 'production', balance_prod, created_at, updated_at FROM merchants
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                insert_profiles_prod.to_owned(),
            ))
            .await?;

        // Migrate Profiles (Sandbox)
        let insert_profiles_sandbox = r#"
            INSERT INTO merchant_profiles (merchant_id, environment, balance, created_at, updated_at)
            SELECT id, 'sandbox', balance_sandbox, created_at, updated_at FROM merchants
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                insert_profiles_sandbox.to_owned(),
            ))
            .await?;

        // Migrate Chain Accounts (Sandbox, TRON) - Only if xpub exists
        // Note: Legacy data is considered Sandbox data.
        let insert_chain_sandbox = r#"
            INSERT INTO merchant_chain_accounts (merchant_id, environment, network, xpub_encrypted, last_path_index, collection_address, created_at, updated_at)
            SELECT id, 'sandbox', 'TRON', xpub_encrypted, last_path_index, collection_address, created_at, updated_at
            FROM merchants
            WHERE xpub_encrypted IS NOT NULL
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                insert_chain_sandbox.to_owned(),
            ))
            .await?;

        // 4. Drop columns from merchants
        // Note: We do this one by one or in batch depending on DB backend, SeaORM helper usually one by one.

        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .drop_column(Merchants::BalanceProd)
                    .drop_column(Merchants::BalanceSandbox)
                    .drop_column(Merchants::XpubEncrypted)
                    .drop_column(Merchants::LastPathIndex)
                    .drop_column(Merchants::CollectionAddress)
                    .drop_column(Merchants::FlatFee)
                    .drop_column(Merchants::MinSweepThreshold)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse is tricky because we lost data structure, but for structure:

        // 1. Add columns back
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .add_column(
                        ColumnDef::new(Merchants::BalanceProd)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(
                        ColumnDef::new(Merchants::BalanceSandbox)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(ColumnDef::new(Merchants::XpubEncrypted).text().null())
                    .add_column(
                        ColumnDef::new(Merchants::LastPathIndex)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(ColumnDef::new(Merchants::CollectionAddress).string().null())
                    .add_column(
                        ColumnDef::new(Merchants::FlatFee)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(
                        ColumnDef::new(Merchants::MinSweepThreshold)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Restore data (Best effort)
        // Update Prod Balance
        let restore_prod = r#"
            UPDATE merchants m
            SET balance_prod = mp.balance
            FROM merchant_profiles mp
            WHERE m.id = mp.merchant_id AND mp.environment = 'production'
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                restore_prod.to_owned(),
            ))
            .await?;

        // Restore Sandbox Balance
        let restore_sandbox = r#"
            UPDATE merchants m
            SET balance_sandbox = mp.balance
            FROM merchant_profiles mp
            WHERE m.id = mp.merchant_id AND mp.environment = 'sandbox'
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                restore_sandbox.to_owned(),
            ))
            .await?;

        // Restore Xpub from sandbox/TRON account
        let restore_xpub = r#"
            UPDATE merchants m
            SET xpub_encrypted = mca.xpub_encrypted,
                last_path_index = mca.last_path_index,
                collection_address = mca.collection_address
            FROM merchant_chain_accounts mca
            WHERE m.id = mca.merchant_id AND mca.environment = 'sandbox' AND mca.network = 'TRON'
        "#;
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                restore_xpub.to_owned(),
            ))
            .await?;

        // 3. Drop tables
        manager
            .drop_table(Table::drop().table(MerchantChainAccounts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MerchantProfiles::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Merchants {
    Table,
    Id,
    BalanceProd,
    BalanceSandbox,
    XpubEncrypted,
    LastPathIndex,
    CollectionAddress,
    FlatFee,
    MinSweepThreshold,
}

#[derive(Iden)]
enum MerchantProfiles {
    Table,
    MerchantId,
    Environment,
    Balance,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MerchantChainAccounts {
    Table,
    MerchantId,
    Environment,
    Network,
    XpubEncrypted,
    LastPathIndex,
    CollectionAddress,
    CreatedAt,
    UpdatedAt,
}
