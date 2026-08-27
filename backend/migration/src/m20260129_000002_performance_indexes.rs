use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Checkout Sessions: (merchant_id, network, created_at DESC)
        // Optimized for list_sessions pagination and filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_sessions_lookup")
                    .table(CheckoutSessions::Table)
                    .col(CheckoutSessions::MerchantId)
                    .col(CheckoutSessions::Network)
                    .col((CheckoutSessions::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // 2. Checkout Sessions: (client_reference_id)
        // Optimized for merchant order lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_sessions_client_ref")
                    .table(CheckoutSessions::Table)
                    .col(CheckoutSessions::ClientReferenceId)
                    .to_owned(),
            )
            .await?;

        // 3. Transactions: (tx_hash, merchant_id, network)
        // Optimized for isolated transaction hash lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_transactions_hash_lookup")
                    .table(Transactions::Table)
                    .col(Transactions::TxHash)
                    .col(Transactions::MerchantId)
                    .col(Transactions::Network)
                    .to_owned(),
            )
            .await?;

        // 4. Addresses: Partial index for Idle addresses
        // Optimized for fast address allocation for new sessions
        // Using raw SQL because index_condition might have version-specific issues in fluent API
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_addresses_idle_allocation ON addresses (merchant_id, network, status) WHERE status = 'Idle'"
            )
            .await?;

        // 5. Transactions: created_at
        // Optimized for statistics aggregation
        manager
            .create_index(
                Index::create()
                    .name("idx_transactions_created_at")
                    .table(Transactions::Table)
                    .col(Transactions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_transactions_created_at")
                    .table(Transactions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX idx_addresses_idle_allocation")
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_transactions_hash_lookup")
                    .table(Transactions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_checkout_sessions_client_ref")
                    .table(CheckoutSessions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_checkout_sessions_lookup")
                    .table(CheckoutSessions::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum CheckoutSessions {
    Table,
    MerchantId,
    Network,
    CreatedAt,
    ClientReferenceId,
}

#[derive(Iden)]
enum Transactions {
    Table,
    MerchantId,
    Network,
    TxHash,
    CreatedAt,
}
