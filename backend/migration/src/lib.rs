//! Database migrations for Tron Checkout
//!
//! Schema aligned with docs/system_design.md v2

pub use sea_orm_migration::prelude::*;

mod m20260113_000001_init_schema;
mod m20260114_000001_account_index_sequence;
mod m20260114_000002_token_version;
mod m20260114_000003_optional_collection_address;
mod m20260114_000004_backup_codes;
mod m20260114_000005_hd_account_xpub;
mod m20260115_000001_payment_events;
mod m20260115_000002_transaction_is_credited;
mod m20260115_000003_idempotency_keys;
mod m20260115_000004_indexer_state;
mod m20260115_000005_payment_exceptions;
mod m20260116_000001_session_address_index;
mod m20260117_000001_gas_credit_to_usdt;
mod m20260118_000001_sweep_session_id;
mod m20260118_000002_webhook_processing_status;
mod m20260118_000003_webhook_indexes;
mod m20260120_000001_rename_xpub_column;
mod m20260120_000002_api_key_name_last_used;
mod m20260121_000001_add_sweep_error;
mod m20260121_000002_add_network_isolation;
mod m20260122_000001_address_notify_trigger;
mod m20260126_000001_rename_billing_ref;

mod m20260126_000002_separate_sandbox_balance;
mod m20260127_000001_split_merchant_tables;
mod m20260128_000001_rename_billing_network;
mod m20260129_000001_add_webhook_target_url;
mod m20260129_000002_performance_indexes;

mod m20260129_000003_add_settlement_status;

mod m20260130_000001_refactor_payment_exceptions;

mod m20260202_000001_add_address_type;

mod m20260204_000001_add_redirect_urls;

mod m20260205_000001_add_sweep_type;

mod m20260206_000001_aml_tables;

mod m20260208_000001_ledger_mode;

mod m20260223_000001_add_withdrawal_network;

mod m20260223_000002_chain_account_balance;

mod m20260223_000003_drop_profiles_add_billing_network;

mod m20260224_000001_normalize_network_values;

mod m20260224_000002_reserve_account_index_zero;

mod m20260225_000001_indexer_chain_head;

mod m20260226_000001_add_sweep_cost_in_usdt;

mod m20260226_000002_drop_legacy_cost_columns;

mod m20260227_000001_create_payouts;

mod m20260227_000002_webhook_rename_session_id;

mod m20260227_000003_webhook_rename_source_id;

mod m20260302_000001_add_custom_fee_percentage;

mod m20260303_000001_drop_address_type;

// Role & Organization Phase 1
mod m20260305_000001_create_users_table;
mod m20260305_000002_create_org_members;
mod m20260305_000003_api_keys_created_by;
mod m20260305_000004_migrate_auth_data;
mod m20260305_000005_drop_merchant_auth_fields;

// USDC Native Support
mod m20260311_000001_usdc_support;

// Fiat Pricing Layer
mod m20260312_000001_fiat_pricing;
mod m20260313_000001_pricing_not_null;

// Payout/Withdrawal USDC Support
mod m20260314_000001_add_payout_withdrawal_currency;

// Enforce single owner per org
mod m20260316_000001_enforce_single_owner;

// Payout Risk Control (approval flow + trusted addresses)
mod m20260316_000002_payout_risk_control;

// Agent Referral System
mod m20260318_000001_agent_profiles;

// Tiered Pricing
mod m20260319_000001_tiered_pricing;

// Sub-Merchant / PSP Support
mod m20260319_000002_sub_merchants;
mod m20260319_000003_checkout_sub_merchant_code;

// Optional Redirect URLs
mod m20260321_000001_optional_redirect_urls;

// Solana Network Support (Phase 1 — enum only, no schema changes)
mod m20260321_000002_solana_network_support;

// Drop risk_control_enabled master switch (individual rules now apply directly)
mod m20260325_000001_drop_risk_control_enabled;

// White-label checkout branding
mod m20260326_000001_add_merchant_logo;

// Xero accounting integration
mod m20260408_000001_create_xero_tables;
mod m20260409_000001_xero_indexes_constraints;
mod m20260409_000002_xero_tax_type_config;
mod m20260410_000001_xero_oauth_states;
mod m20260410_000002_xero_fx_snapshot;

// Public enterprise lead intake
mod m20260810_000001_enterprise_leads;

// Requeue residual balances attached to successful sessions
mod m20260818_000001_requeue_residual_balances;
mod m20260818_000002_unify_outbound_transactions;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260113_000001_init_schema::Migration),
            Box::new(m20260114_000001_account_index_sequence::Migration),
            Box::new(m20260114_000002_token_version::Migration),
            Box::new(m20260114_000003_optional_collection_address::Migration),
            Box::new(m20260114_000004_backup_codes::Migration),
            Box::new(m20260114_000005_hd_account_xpub::Migration),
            Box::new(m20260115_000001_payment_events::Migration),
            Box::new(m20260115_000002_transaction_is_credited::Migration),
            Box::new(m20260115_000003_idempotency_keys::Migration),
            Box::new(m20260115_000004_indexer_state::Migration),
            Box::new(m20260115_000005_payment_exceptions::Migration),
            Box::new(m20260116_000001_session_address_index::Migration),
            Box::new(m20260117_000001_gas_credit_to_usdt::Migration),
            Box::new(m20260118_000001_sweep_session_id::Migration),
            Box::new(m20260118_000002_webhook_processing_status::Migration),
            Box::new(m20260118_000003_webhook_indexes::Migration),
            Box::new(m20260120_000001_rename_xpub_column::Migration),
            Box::new(m20260120_000002_api_key_name_last_used::Migration),
            Box::new(m20260121_000001_add_sweep_error::Migration),
            Box::new(m20260121_000002_add_network_isolation::Migration),
            Box::new(m20260122_000001_address_notify_trigger::Migration),
            Box::new(m20260126_000001_rename_billing_ref::Migration),
            Box::new(m20260126_000002_separate_sandbox_balance::Migration),
            Box::new(m20260127_000001_split_merchant_tables::Migration),
            Box::new(m20260128_000001_rename_billing_network::Migration),
            Box::new(m20260129_000001_add_webhook_target_url::Migration),
            Box::new(m20260129_000002_performance_indexes::Migration),
            Box::new(m20260129_000003_add_settlement_status::Migration),
            Box::new(m20260130_000001_refactor_payment_exceptions::Migration),
            Box::new(m20260202_000001_add_address_type::Migration),
            Box::new(m20260204_000001_add_redirect_urls::Migration),
            Box::new(m20260205_000001_add_sweep_type::Migration),
            Box::new(m20260206_000001_aml_tables::Migration),
            Box::new(m20260208_000001_ledger_mode::Migration),
            Box::new(m20260223_000001_add_withdrawal_network::Migration),
            Box::new(m20260223_000002_chain_account_balance::Migration),
            Box::new(m20260223_000003_drop_profiles_add_billing_network::Migration),
            Box::new(m20260224_000001_normalize_network_values::Migration),
            Box::new(m20260224_000002_reserve_account_index_zero::Migration),
            Box::new(m20260225_000001_indexer_chain_head::Migration),
            Box::new(m20260226_000001_add_sweep_cost_in_usdt::Migration),
            Box::new(m20260226_000002_drop_legacy_cost_columns::Migration),
            Box::new(m20260227_000001_create_payouts::Migration),
            Box::new(m20260227_000002_webhook_rename_session_id::Migration),
            Box::new(m20260227_000003_webhook_rename_source_id::Migration),
            Box::new(m20260302_000001_add_custom_fee_percentage::Migration),
            Box::new(m20260303_000001_drop_address_type::Migration),
            // Role & Organization Phase 1
            Box::new(m20260305_000001_create_users_table::Migration),
            Box::new(m20260305_000002_create_org_members::Migration),
            Box::new(m20260305_000003_api_keys_created_by::Migration),
            Box::new(m20260305_000004_migrate_auth_data::Migration),
            Box::new(m20260305_000005_drop_merchant_auth_fields::Migration),
            // USDC Native Support
            Box::new(m20260311_000001_usdc_support::Migration),
            // Fiat Pricing Layer
            Box::new(m20260312_000001_fiat_pricing::Migration),
            Box::new(m20260313_000001_pricing_not_null::Migration),
            // Payout/Withdrawal USDC Support
            Box::new(m20260314_000001_add_payout_withdrawal_currency::Migration),
            // Enforce single owner per org
            Box::new(m20260316_000001_enforce_single_owner::Migration),
            // Payout Risk Control
            Box::new(m20260316_000002_payout_risk_control::Migration),
            // Agent Referral System
            Box::new(m20260318_000001_agent_profiles::Migration),
            // Tiered Pricing
            Box::new(m20260319_000001_tiered_pricing::Migration),
            // Sub-Merchant / PSP Support
            Box::new(m20260319_000002_sub_merchants::Migration),
            Box::new(m20260319_000003_checkout_sub_merchant_code::Migration),
            // Optional Redirect URLs
            Box::new(m20260321_000001_optional_redirect_urls::Migration),
            // Solana Network Support (Phase 1)
            Box::new(m20260321_000002_solana_network_support::Migration),
            // Drop risk_control_enabled master switch
            Box::new(m20260325_000001_drop_risk_control_enabled::Migration),
            // White-label checkout branding
            Box::new(m20260326_000001_add_merchant_logo::Migration),
            // Xero accounting integration
            Box::new(m20260408_000001_create_xero_tables::Migration),
            Box::new(m20260409_000001_xero_indexes_constraints::Migration),
            Box::new(m20260409_000002_xero_tax_type_config::Migration),
            Box::new(m20260410_000001_xero_oauth_states::Migration),
            Box::new(m20260410_000002_xero_fx_snapshot::Migration),
            // Public enterprise lead intake
            Box::new(m20260810_000001_enterprise_leads::Migration),
            // Requeue residual balances attached to successful sessions
            Box::new(m20260818_000001_requeue_residual_balances::Migration),
            // Canonical journal for sweeps, payouts, and withdrawals
            Box::new(m20260818_000002_unify_outbound_transactions::Migration),
        ]
    }
}
