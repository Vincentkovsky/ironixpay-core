use sea_orm_migration::prelude::*;

/// Phase 1: Solana network enum support.
///
/// The `network` column in `checkout_sessions`, `transactions`, `chain_accounts`,
/// `addresses`, `sweep_transactions`, `indexer_state`, `payment_events`,
/// `billing_logs`, and `withdrawals` already uses VARCHAR — no schema change
/// needed for the "SOLANA" string value.
///
/// This migration serves as a checkpoint marker. Phase 2 will add:
/// - `solana_address_pool` table (pre-derived Ed25519 addresses)
/// - Solana-specific `indexer_state` entries
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Phase 1: No schema changes needed.
        // Network enum is stored as VARCHAR, so "SOLANA" value works automatically.
        // Phase 2 will add solana_address_pool table and related indexes.
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op: no schema changes to revert.
        Ok(())
    }
}
