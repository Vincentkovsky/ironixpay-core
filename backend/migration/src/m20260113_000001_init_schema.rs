//! Initial schema migration (aligned with docs/system_design.md)
//!
//! Creates all tables with proper relationships, composite keys, and indexes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ============================================================
        // 1. MERCHANTS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(Merchants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Merchants::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Merchants::Name).string().not_null())
                    .col(
                        ColumnDef::new(Merchants::Email)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Merchants::PasswordHash).string().not_null())
                    .col(
                        ColumnDef::new(Merchants::Status)
                            .string()
                            .not_null()
                            .default("pending_verification"),
                    )
                    .col(
                        ColumnDef::new(Merchants::AccountIndex)
                            .integer()
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Merchants::LastPathIndex)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Merchants::CollectionAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Merchants::FlatFee)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Merchants::MinSweepThreshold)
                            .big_integer()
                            .not_null()
                            .default(1_000_000),
                    ) // 1 USDT
                    .col(
                        ColumnDef::new(Merchants::GasCreditBalance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Merchants::TotpSecret).string().null())
                    .col(
                        ColumnDef::new(Merchants::IsTotpEnabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Merchants::EmailVerified)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Merchants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Merchants::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 2. API_KEYS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(ApiKeys::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApiKeys::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ApiKeys::MerchantId).string().not_null())
                    .col(ColumnDef::new(ApiKeys::KeyPrefix).string().not_null())
                    .col(ColumnDef::new(ApiKeys::KeyHash).string().not_null())
                    .col(
                        ColumnDef::new(ApiKeys::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ApiKeys::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ApiKeys::Table, ApiKeys::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 3. WEBHOOK_ENDPOINTS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(WebhookEndpoints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebhookEndpoints::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WebhookEndpoints::Url).string().not_null())
                    .col(
                        ColumnDef::new(WebhookEndpoints::Description)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::SecretEncrypted)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::Status)
                            .string()
                            .not_null()
                            .default("enabled"),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WebhookEndpoints::Table, WebhookEndpoints::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 4. ADDRESSES (Composite PK: network + address)
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(Addresses::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Addresses::Network).string().not_null())
                    .col(ColumnDef::new(Addresses::Address).string().not_null())
                    .col(ColumnDef::new(Addresses::MerchantId).string().not_null())
                    .col(ColumnDef::new(Addresses::PathIndex).integer().not_null())
                    .col(
                        ColumnDef::new(Addresses::NativeBalance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Addresses::UsdtBalance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Addresses::Status)
                            .string()
                            .not_null()
                            .default("Idle"),
                    )
                    .col(ColumnDef::new(Addresses::ErrorReason).text().null())
                    .col(
                        ColumnDef::new(Addresses::SweepAttempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Addresses::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Addresses::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(Addresses::Network)
                            .col(Addresses::Address),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Addresses::Table, Addresses::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index: addresses(merchant_id, status) for pool queries
        manager
            .create_index(
                Index::create()
                    .name("idx_addresses_merchant_status")
                    .table(Addresses::Table)
                    .col(Addresses::MerchantId)
                    .col(Addresses::Status)
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 5. CHECKOUT_SESSIONS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(CheckoutSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CheckoutSessions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::Network)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::PayAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::ClientReferenceId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::Currency)
                            .string()
                            .not_null()
                            .default("USDT"),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::CurrencyContract)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::AmountExpected)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::AmountReceived)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::Status)
                            .string()
                            .not_null()
                            .default("Pending"),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutSessions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CheckoutSessions::Table, CheckoutSessions::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Composite FK to addresses (network, pay_address)
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checkout_sessions_address")
                            .from(CheckoutSessions::Table, CheckoutSessions::Network)
                            .from_col(CheckoutSessions::PayAddress)
                            .to(Addresses::Table, Addresses::Network)
                            .to_col(Addresses::Address)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 6. TRANSACTIONS (Composite PK: network + tx_hash + log_index)
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(Transactions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Transactions::Network).string().not_null())
                    .col(ColumnDef::new(Transactions::TxHash).string().not_null())
                    .col(ColumnDef::new(Transactions::LogIndex).integer().not_null())
                    .col(ColumnDef::new(Transactions::SessionId).string().not_null())
                    .col(ColumnDef::new(Transactions::MerchantId).string().not_null())
                    .col(
                        ColumnDef::new(Transactions::CurrencySymbol)
                            .string()
                            .not_null()
                            .default("USDT"),
                    )
                    .col(
                        ColumnDef::new(Transactions::CurrencyContract)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transactions::FromAddress)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Transactions::ToAddress).string().not_null())
                    .col(
                        ColumnDef::new(Transactions::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transactions::Status)
                            .string()
                            .not_null()
                            .default("Unconfirmed"),
                    )
                    .col(
                        ColumnDef::new(Transactions::ConfirmationsCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Transactions::BlockNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transactions::BlockTimestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Transactions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(Transactions::Network)
                            .col(Transactions::TxHash)
                            .col(Transactions::LogIndex),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Transactions::Table, Transactions::SessionId)
                            .to(CheckoutSessions::Table, CheckoutSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index: transactions(merchant_id, created_at DESC)
        manager
            .create_index(
                Index::create()
                    .name("idx_transactions_merchant")
                    .table(Transactions::Table)
                    .col(Transactions::MerchantId)
                    .col((Transactions::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 7. SWEEP_TRANSACTIONS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(SweepTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SweepTransactions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::Network)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::FromAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::ToAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::EnergyDelegateTxHash)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::FundingTxHash)
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(SweepTransactions::TxHash).string().null())
                    .col(
                        ColumnDef::new(SweepTransactions::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::Status)
                            .string()
                            .not_null()
                            .default("Pending"),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::EnergyCost)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::BandwidthCost)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(SweepTransactions::ConfirmedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sweep_transactions_address")
                            .from(SweepTransactions::Table, SweepTransactions::Network)
                            .from_col(SweepTransactions::FromAddress)
                            .to(Addresses::Table, Addresses::Network)
                            .to_col(Addresses::Address)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 8. BILLING_LOGS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(BillingLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BillingLogs::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BillingLogs::MerchantId).string().not_null())
                    .col(ColumnDef::new(BillingLogs::SessionId).string().null())
                    .col(ColumnDef::new(BillingLogs::SweepTxId).string().null())
                    .col(ColumnDef::new(BillingLogs::Type).string().not_null())
                    .col(
                        ColumnDef::new(BillingLogs::PreviousBalance)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BillingLogs::AmountChange)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BillingLogs::BalanceAfter)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BillingLogs::Description).string().null())
                    .col(
                        ColumnDef::new(BillingLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(BillingLogs::Table, BillingLogs::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ============================================================
        // 9. WEBHOOK_EVENTS
        // ============================================================
        manager
            .create_table(
                Table::create()
                    .table(WebhookEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebhookEvents::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::EndpointId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WebhookEvents::SessionId).string().not_null())
                    .col(
                        ColumnDef::new(WebhookEvents::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WebhookEvents::EventType).string().not_null())
                    .col(
                        ColumnDef::new(WebhookEvents::Payload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::HttpStatusCode)
                            .integer()
                            .null(),
                    )
                    .col(ColumnDef::new(WebhookEvents::ResponseBody).text().null())
                    .col(
                        ColumnDef::new(WebhookEvents::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::LastAttemptAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::NextRetryAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WebhookEvents::Table, WebhookEvents::EndpointId)
                            .to(WebhookEndpoints::Table, WebhookEndpoints::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WebhookEvents::Table, WebhookEvents::SessionId)
                            .to(CheckoutSessions::Table, CheckoutSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // IdempotencyKeys table creation removed (managed by m20260115_000003_idempotency_keys)

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse order of creation (respect FK dependencies)

        manager
            .drop_table(Table::drop().table(WebhookEvents::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BillingLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SweepTransactions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Transactions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CheckoutSessions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Addresses::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WebhookEndpoints::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Merchants::Table).to_owned())
            .await?;
        Ok(())
    }
}

// ============================================================
// Table/Column Identifiers
// ============================================================

#[derive(Iden)]
enum Merchants {
    Table,
    Id,
    Name,
    Email,
    PasswordHash,
    Status,
    AccountIndex,
    LastPathIndex,
    CollectionAddress,
    FlatFee,
    MinSweepThreshold,
    GasCreditBalance,
    TotpSecret,
    IsTotpEnabled,
    EmailVerified,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ApiKeys {
    Table,
    Id,
    MerchantId,
    KeyPrefix,
    KeyHash,
    IsActive,
    CreatedAt,
}

#[derive(Iden)]
enum WebhookEndpoints {
    Table,
    Id,
    MerchantId,
    Url,
    Description,
    SecretEncrypted,
    Status,
    CreatedAt,
}

#[derive(Iden)]
enum Addresses {
    Table,
    Network,
    Address,
    MerchantId,
    PathIndex,
    NativeBalance,
    UsdtBalance,
    Status,
    ErrorReason,
    SweepAttempts,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CheckoutSessions {
    Table,
    Id,
    MerchantId,
    Network,
    PayAddress,
    ClientReferenceId,
    Currency,
    CurrencyContract,
    AmountExpected,
    AmountReceived,
    Status,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Transactions {
    Table,
    Network,
    TxHash,
    LogIndex,
    SessionId,
    MerchantId,
    CurrencySymbol,
    CurrencyContract,
    FromAddress,
    ToAddress,
    Amount,
    Status,
    ConfirmationsCount,
    BlockNumber,
    BlockTimestamp,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum SweepTransactions {
    Table,
    Id,
    MerchantId,
    Network,
    FromAddress,
    ToAddress,
    EnergyDelegateTxHash,
    FundingTxHash,
    TxHash,
    Amount,
    Status,
    EnergyCost,
    BandwidthCost,
    CreatedAt,
    ConfirmedAt,
}

#[derive(Iden)]
enum BillingLogs {
    Table,
    Id,
    MerchantId,
    SessionId,
    SweepTxId,
    Type,
    PreviousBalance,
    AmountChange,
    BalanceAfter,
    Description,
    CreatedAt,
}

#[derive(Iden)]
enum WebhookEvents {
    Table,
    Id,
    EndpointId,
    SessionId,
    MerchantId,
    EventType,
    Payload,
    Status,
    HttpStatusCode,
    ResponseBody,
    AttemptCount,
    LastAttemptAt,
    NextRetryAt,
    CreatedAt,
}
