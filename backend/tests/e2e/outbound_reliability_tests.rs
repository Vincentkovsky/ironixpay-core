use ironix_pay::entity::{merchants, outbound_transactions, payouts, Environment, Merchants};
use ironix_pay::migration::Migrator;
use ironix_pay::services::outbound::{
    create_attempt, new_id, preparing_model, BroadcastDisposition, OutboundTransactionStore,
    StoredSignedTransaction, TerminalEvidence,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::MigratorTrait;

use crate::common;

async fn seed_payout(db: &sea_orm::DatabaseConnection, payout_id: &str) {
    if Merchants::find_by_id("mer_outbound_test")
        .one(db)
        .await
        .unwrap()
        .is_none()
    {
        merchants::ActiveModel {
            id: Set("mer_outbound_test".into()),
            name: Set("Outbound Test".into()),
            status: Set(merchants::MerchantStatus::Active),
            account_index: Set(Some(9_999)),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    payouts::ActiveModel {
        id: Set(payout_id.into()),
        merchant_id: Set("mer_outbound_test".into()),
        environment: Set(Environment::Production),
        network: Set("BSC".into()),
        to_address: Set("0x2222222222222222222222222222222222222222".into()),
        amount: Set(10_000_000),
        fee: Set(50_000),
        net_amount: Set(9_950_000),
        status: Set(payouts::PayoutStatus::Processing),
        tx_hash: Set(None),
        error_reason: Set(None),
        idempotency_key: Set(format!("idem_{payout_id}")),
        description: Set(None),
        metadata: Set(None),
        currency: Set("USDT".into()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        completed_at: Set(None),
        reviewed_by: Set(None),
        reviewed_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
}

async fn create_signed_root(
    db: &sea_orm::DatabaseConnection,
    store: &OutboundTransactionStore,
    payout_id: &str,
    tx_hash: &str,
) -> outbound_transactions::Model {
    seed_payout(db, payout_id).await;
    let mut root = preparing_model(
        new_id(),
        "mer_outbound_test".into(),
        Environment::Production,
        outbound_transactions::OutboundOperationType::Payout,
        "BSC".into(),
        "0x1111111111111111111111111111111111111111".into(),
        "0x2222222222222222222222222222222222222222".into(),
        9_950_000,
        "USDT".into(),
    );
    root.payout_id = Set(Some(payout_id.into()));
    let root = create_attempt(db, root).await.unwrap();
    store
        .record_signed(
            &root.id,
            &StoredSignedTransaction::Evm {
                tx_hash: tx_hash.into(),
                raw_tx_hex: "0xdeadbeef".into(),
                from_address: "0x1111111111111111111111111111111111111111".into(),
                nonce: 7,
            },
        )
        .await
        .unwrap();
    root
}

#[tokio::test]
async fn auxiliary_rows_cannot_impersonate_business_roots() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    let root = create_signed_root(db, &store, "po_outbound_child", "0xroot").await;

    let child = store
        .create_child_attempt(
            &root.id,
            outbound_transactions::OutboundPurpose::GasFunding,
            "0x3333333333333333333333333333333333333333".into(),
            "0x1111111111111111111111111111111111111111".into(),
            100,
            "NATIVE".into(),
        )
        .await
        .unwrap();

    assert_eq!(
        child.parent_transaction_id.as_deref(),
        Some(root.id.as_str())
    );
    assert!(child.payout_id.is_none());
    assert!(child.withdrawal_id.is_none());
    assert!(child.session_id.is_none());
    assert!(child.exception_id.is_none());
    assert_eq!(
        store
            .find_for_payout_tx("po_outbound_child", "0xroot")
            .await
            .unwrap()
            .unwrap()
            .id,
        root.id
    );
}

#[tokio::test]
async fn only_one_competing_terminal_transition_wins() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    let root = create_signed_root(db, &store, "po_outbound_cas", "0xcas").await;

    let (confirmed, reverted) = tokio::join!(
        store.mark_state(
            &root.id,
            outbound_transactions::OutboundState::Confirmed,
            None,
        ),
        store.mark_state(
            &root.id,
            outbound_transactions::OutboundState::Reverted,
            Some("reverted".into()),
        )
    );
    assert_ne!(confirmed.unwrap(), reverted.unwrap());

    let final_state = outbound_transactions::Entity::find_by_id(&root.id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .state;
    assert!(matches!(
        final_state,
        outbound_transactions::OutboundState::Confirmed
            | outbound_transactions::OutboundState::Reverted
    ));
}

#[tokio::test]
async fn signed_executor_handoff_is_idempotent() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    let root = create_signed_root(db, &store, "po_outbound_handoff", "0xhandoff").await;

    assert!(store
        .adopt_executor_result(&root.id, "0xhandoff", BroadcastDisposition::Accepted)
        .await
        .unwrap());
    assert!(!store
        .adopt_executor_result(&root.id, "0xdifferent", BroadcastDisposition::Accepted)
        .await
        .unwrap());
}

#[tokio::test]
async fn provider_transactions_cannot_reuse_a_network_hash() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    let first_root = create_signed_root(db, &store, "po_outbound_provider_1", "0xroot1").await;
    let second_root = create_signed_root(db, &store, "po_outbound_provider_2", "0xroot2").await;

    let first_child = store
        .create_child_attempt(
            &first_root.id,
            outbound_transactions::OutboundPurpose::EnergyFunding,
            "external_energy_provider".into(),
            "TProviderTarget1".into(),
            64_000,
            "ENERGY".into(),
        )
        .await
        .unwrap();
    assert!(store
        .adopt_executor_result(
            &first_child.id,
            "provider_tx_hash",
            BroadcastDisposition::Accepted,
        )
        .await
        .unwrap());

    let second_child = store
        .create_child_attempt(
            &second_root.id,
            outbound_transactions::OutboundPurpose::EnergyFunding,
            "external_energy_provider".into(),
            "TProviderTarget2".into(),
            64_000,
            "ENERGY".into(),
        )
        .await
        .unwrap();
    assert!(store
        .adopt_executor_result(
            &second_child.id,
            "provider_tx_hash",
            BroadcastDisposition::Accepted,
        )
        .await
        .is_err());
}

#[tokio::test]
async fn terminal_evidence_requires_a_second_observation_after_grace() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    let root = create_signed_root(db, &store, "po_outbound_grace", "0xgrace").await;

    assert_eq!(
        store
            .stage_terminal_evidence(
                &root.id,
                outbound_transactions::OutboundState::Replaced,
                "nonce consumed",
            )
            .await
            .unwrap(),
        TerminalEvidence::Staged
    );
    db.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        format!(
            "UPDATE outbound_transactions SET observed_at = NOW() - INTERVAL '31 seconds' WHERE id = '{}'",
            root.id
        ),
    ))
    .await
    .unwrap();
    assert_eq!(
        store
            .stage_terminal_evidence(
                &root.id,
                outbound_transactions::OutboundState::Replaced,
                "nonce consumed",
            )
            .await
            .unwrap(),
        TerminalEvidence::Ready
    );
}

#[tokio::test]
async fn database_rejects_two_active_roots_for_one_payout() {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    let store = OutboundTransactionStore::for_tests(db.clone());
    create_signed_root(db, &store, "po_outbound_unique", "0xunique1").await;

    let mut duplicate = preparing_model(
        new_id(),
        "mer_outbound_test".into(),
        Environment::Production,
        outbound_transactions::OutboundOperationType::Payout,
        "BSC".into(),
        "0x1111111111111111111111111111111111111111".into(),
        "0x2222222222222222222222222222222222222222".into(),
        9_950_000,
        "USDT".into(),
    );
    duplicate.payout_id = Set(Some("po_outbound_unique".into()));
    assert!(create_attempt(db, duplicate).await.is_err());

    assert_eq!(
        outbound_transactions::Entity::find()
            .filter(outbound_transactions::Column::PayoutId.eq("po_outbound_unique"))
            .all(db)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn unified_outbound_migration_round_trips_before_new_rows_exist() {
    let test_db = common::setup_test_db().await;
    Migrator::down(&test_db.conn, Some(1)).await.unwrap();
    Migrator::up(&test_db.conn, None).await.unwrap();

    let table: Option<String> = test_db
        .conn
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT to_regclass('public.outbound_transactions')::text AS table_name",
        ))
        .await
        .unwrap()
        .and_then(|row| row.try_get("", "table_name").ok());
    assert_eq!(table.as_deref(), Some("outbound_transactions"));
}

#[tokio::test]
async fn unified_outbound_migration_refuses_destructive_rollback() {
    let test_db = common::setup_test_db().await;
    let store = OutboundTransactionStore::for_tests(test_db.conn.clone());
    create_signed_root(&test_db.conn, &store, "po_outbound_no_down", "0xnodown").await;

    let error = Migrator::down(&test_db.conn, Some(1)).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("Cannot roll back unified outbound journal"));
}
