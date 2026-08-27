//! Auto-Withdraw Background Task
//!
//! Periodically checks all merchants with auto_withdraw_enabled = true.
//! For each merchant, iterates ALL chain accounts and checks USDT + USDC
//! balances independently. When any (chain, currency) balance exceeds the
//! global threshold, the full balance is withdrawn (skipping risk control).

use crate::entity::{merchant_chain_accounts, payout_settings, withdrawals};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::PayoutService;

/// Interval between auto-withdraw checks (60 seconds)
const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Run the auto-withdraw background loop.
pub async fn run(
    db: DatabaseConnection,
    payout_service: Arc<PayoutService>,
    environment: crate::entity::Environment,
    cancel_token: CancellationToken,
) -> anyhow::Result<()> {
    info!("Auto-withdraw task started");
    let mut interval = tokio::time::interval(CHECK_INTERVAL);

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Auto-withdraw task shutting down");
                return Ok(());
            }
            _ = interval.tick() => {
                if let Err(e) = check_and_withdraw(&db, &payout_service, &environment).await {
                    error!(error = %e, "Auto-withdraw check cycle failed");
                }
            }
        }
    }
}

async fn check_and_withdraw(
    db: &DatabaseConnection,
    payout_service: &PayoutService,
    environment: &crate::entity::Environment,
) -> anyhow::Result<()> {
    // 1. Find all merchants with auto-withdraw enabled
    let settings = payout_settings::Entity::find()
        .filter(payout_settings::Column::AutoWithdrawEnabled.eq(true))
        .all(db)
        .await?;

    if settings.is_empty() {
        return Ok(());
    }

    debug!(
        count = settings.len(),
        "Checking auto-withdraw eligible merchants"
    );

    for setting in settings {
        let merchant_id = &setting.merchant_id;
        let threshold = match setting.auto_withdraw_threshold {
            Some(t) if t > 0 => t,
            _ => {
                debug!(
                    merchant_id,
                    "Auto-withdraw enabled but no valid threshold set, skipping"
                );
                continue;
            }
        };

        // 2. Get ALL chain accounts for this merchant + environment
        let chain_accounts = merchant_chain_accounts::Entity::find()
            .filter(merchant_chain_accounts::Column::MerchantId.eq(merchant_id))
            .filter(merchant_chain_accounts::Column::Environment.eq(environment.clone()))
            .all(db)
            .await?;

        if chain_accounts.is_empty() {
            continue;
        }

        // 3. For each chain account, check USDT and USDC independently
        for account in &chain_accounts {
            let network = &account.network;

            // Check USDT balance
            if account.usdt_balance > threshold {
                try_auto_withdraw(
                    db,
                    payout_service,
                    merchant_id,
                    network,
                    "USDT",
                    account.usdt_balance,
                    threshold,
                    environment,
                )
                .await;
            }

            // Check USDC balance (skip if zero — chain may not support USDC)
            if account.usdc_balance > 0 && account.usdc_balance > threshold {
                try_auto_withdraw(
                    db,
                    payout_service,
                    merchant_id,
                    network,
                    "USDC",
                    account.usdc_balance,
                    threshold,
                    environment,
                )
                .await;
            }
        }
    }

    Ok(())
}

/// Attempt an auto-withdrawal for a single (merchant, network, currency) bucket.
/// Checks for in-flight withdrawals to avoid duplicates, then triggers full-balance withdrawal.
async fn try_auto_withdraw(
    db: &DatabaseConnection,
    payout_service: &PayoutService,
    merchant_id: &str,
    network: &crate::entity::Network,
    currency: &str,
    balance: i64,
    threshold: i64,
    environment: &crate::entity::Environment,
) {
    let network_str = network.to_string();

    // In-flight dedup: check per (merchant, network, currency)
    let in_flight = withdrawals::Entity::find()
        .filter(withdrawals::Column::MerchantId.eq(merchant_id))
        .filter(withdrawals::Column::Network.eq(network_str.clone()))
        .filter(withdrawals::Column::Currency.eq(currency))
        .filter(
            sea_orm::Condition::any()
                .add(withdrawals::Column::Status.eq(withdrawals::WithdrawalStatus::Pending))
                .add(withdrawals::Column::Status.eq(withdrawals::WithdrawalStatus::Processing))
                .add(
                    withdrawals::Column::Status.eq(withdrawals::WithdrawalStatus::PendingApproval),
                ),
        )
        .one(db)
        .await;

    match in_flight {
        Ok(Some(_)) => {
            debug!(
                merchant_id,
                network = %network_str,
                currency,
                "In-flight withdrawal exists, skipping auto-withdraw"
            );
            return;
        }
        Err(e) => {
            warn!(
                merchant_id,
                network = %network_str,
                error = %e,
                "Failed to check in-flight withdrawals"
            );
            return;
        }
        Ok(None) => {} // No in-flight, proceed
    }

    info!(
        merchant_id,
        network = %network_str,
        currency,
        balance,
        threshold,
        "Auto-withdraw triggered: balance exceeds threshold"
    );

    match payout_service
        .request_withdrawal(
            merchant_id,
            balance,
            environment.clone(),
            network.clone(),
            currency,
            Some("system:auto-withdraw"),
            true, // skip_risk_control
        )
        .await
    {
        Ok(wd) => {
            info!(
                merchant_id,
                withdrawal_id = %wd.id,
                network = %network_str,
                currency,
                amount = balance,
                "Auto-withdrawal created successfully"
            );
        }
        Err(e) => {
            warn!(
                merchant_id,
                network = %network_str,
                currency,
                error = %e,
                "Auto-withdrawal failed"
            );
        }
    }
}
