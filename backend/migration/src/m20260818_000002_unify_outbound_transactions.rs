//! Evolve the sweep ledger into the canonical outbound chain-execution journal.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE sweep_transactions RENAME TO outbound_transactions;
                ALTER TABLE outbound_transactions RENAME COLUMN sweep_type TO operation_type;
                ALTER TABLE outbound_transactions RENAME COLUMN status TO state;
                ALTER TABLE outbound_transactions RENAME CONSTRAINT sweep_transactions_pkey
                    TO outbound_transactions_pkey;
                ALTER INDEX IF EXISTS idx_sweep_transactions_session
                    RENAME TO idx_outbound_session;
                ALTER INDEX IF EXISTS idx_sweep_transactions_exception
                    RENAME TO idx_outbound_exception;

                ALTER TABLE outbound_transactions
                    DROP CONSTRAINT IF EXISTS check_source_exists,
                    DROP CONSTRAINT IF EXISTS fk_sweep_transactions_address,
                    DROP CONSTRAINT IF EXISTS fk_sweep_transactions_exception,
                    ADD COLUMN environment VARCHAR(32),
                    ADD COLUMN payout_id VARCHAR,
                    ADD COLUMN withdrawal_id VARCHAR,
                    ADD COLUMN parent_transaction_id VARCHAR,
                    ADD COLUMN purpose VARCHAR NOT NULL DEFAULT 'token_transfer',
                    ADD COLUMN provider_reference VARCHAR,
                    ADD COLUMN signed_payload_encrypted TEXT,
                    ADD COLUMN nonce BIGINT,
                    ADD COLUMN expires_at TIMESTAMPTZ,
                    ADD COLUMN last_valid_block_height BIGINT,
                    ADD COLUMN broadcast_attempts INTEGER NOT NULL DEFAULT 0,
                    ADD COLUMN last_broadcast_at TIMESTAMPTZ,
                    ADD COLUMN observed_at TIMESTAMPTZ,
                    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                ALTER TABLE withdrawals
                    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                UPDATE withdrawals SET updated_at = created_at;

                UPDATE outbound_transactions
                SET environment = CASE
                    WHEN current_database() ILIKE '%sandbox%' THEN 'sandbox'
                    ELSE 'production'
                END;

                ALTER TABLE outbound_transactions
                    ALTER COLUMN environment SET NOT NULL,
                    ADD CONSTRAINT check_outbound_source_exists CHECK (
                        num_nonnulls(
                            session_id,
                            exception_id,
                            payout_id,
                            withdrawal_id,
                            parent_transaction_id
                        ) = 1
                    ),
                    ADD CONSTRAINT fk_outbound_session
                        FOREIGN KEY (session_id) REFERENCES checkout_sessions(id)
                        ON DELETE RESTRICT,
                    ADD CONSTRAINT fk_outbound_exception
                        FOREIGN KEY (exception_id) REFERENCES payment_exceptions(id)
                        ON DELETE RESTRICT,
                    ADD CONSTRAINT fk_outbound_payout
                        FOREIGN KEY (payout_id) REFERENCES payouts(id) ON DELETE RESTRICT,
                    ADD CONSTRAINT fk_outbound_withdrawal
                        FOREIGN KEY (withdrawal_id) REFERENCES withdrawals(id) ON DELETE RESTRICT,
                    ADD CONSTRAINT fk_outbound_parent
                        FOREIGN KEY (parent_transaction_id) REFERENCES outbound_transactions(id)
                        ON DELETE RESTRICT;

                UPDATE outbound_transactions
                SET state = CASE
                    WHEN state = 'Pending' AND tx_hash IS NULL THEN 'Preparing'
                    ELSE state
                END;

                CREATE INDEX idx_outbound_payout ON outbound_transactions(payout_id);
                CREATE INDEX idx_outbound_withdrawal ON outbound_transactions(withdrawal_id);
                CREATE INDEX idx_outbound_recovery
                    ON outbound_transactions(network, state, updated_at)
                    WHERE state IN ('Signed', 'BroadcastUnknown', 'Pending');
                CREATE INDEX idx_outbound_from_address
                    ON outbound_transactions(network, from_address);
                CREATE UNIQUE INDEX idx_outbound_network_tx_hash
                    ON outbound_transactions(network, tx_hash)
                    WHERE tx_hash IS NOT NULL;
                CREATE UNIQUE INDEX idx_outbound_active_session
                    ON outbound_transactions(session_id)
                    WHERE session_id IS NOT NULL
                      AND parent_transaction_id IS NULL
                      AND purpose = 'token_transfer'
                      AND state IN ('Preparing', 'Signed', 'BroadcastUnknown', 'Pending');
                CREATE UNIQUE INDEX idx_outbound_active_exception
                    ON outbound_transactions(exception_id)
                    WHERE exception_id IS NOT NULL
                      AND parent_transaction_id IS NULL
                      AND purpose = 'token_transfer'
                      AND state IN ('Preparing', 'Signed', 'BroadcastUnknown', 'Pending');
                CREATE UNIQUE INDEX idx_outbound_active_payout
                    ON outbound_transactions(payout_id)
                    WHERE payout_id IS NOT NULL
                      AND parent_transaction_id IS NULL
                      AND purpose = 'token_transfer'
                      AND state IN ('Preparing', 'Signed', 'BroadcastUnknown', 'Pending');
                CREATE UNIQUE INDEX idx_outbound_active_withdrawal
                    ON outbound_transactions(withdrawal_id)
                    WHERE withdrawal_id IS NOT NULL
                      AND parent_transaction_id IS NULL
                      AND purpose = 'token_transfer'
                      AND state IN ('Preparing', 'Signed', 'BroadcastUnknown', 'Pending');
                CREATE UNIQUE INDEX idx_outbound_active_child
                    ON outbound_transactions(parent_transaction_id, purpose, to_address)
                    WHERE parent_transaction_id IS NOT NULL
                      AND state IN ('Preparing', 'Signed', 'BroadcastUnknown', 'Pending');
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DO $$
                BEGIN
                    IF EXISTS (
                        SELECT 1
                        FROM outbound_transactions
                        WHERE payout_id IS NOT NULL
                           OR withdrawal_id IS NOT NULL
                           OR parent_transaction_id IS NOT NULL
                           OR operation_type IN ('payout', 'withdrawal')
                    ) THEN
                        RAISE EXCEPTION
                            'Cannot roll back unified outbound journal after payout, withdrawal, or auxiliary transactions exist';
                    END IF;
                END $$;

                DROP INDEX IF EXISTS idx_outbound_active_child;
                DROP INDEX IF EXISTS idx_outbound_active_withdrawal;
                DROP INDEX IF EXISTS idx_outbound_active_payout;
                DROP INDEX IF EXISTS idx_outbound_active_exception;
                DROP INDEX IF EXISTS idx_outbound_active_session;
                DROP INDEX IF EXISTS idx_outbound_network_tx_hash;
                DROP INDEX IF EXISTS idx_outbound_from_address;
                DROP INDEX IF EXISTS idx_outbound_recovery;
                DROP INDEX IF EXISTS idx_outbound_withdrawal;
                DROP INDEX IF EXISTS idx_outbound_payout;

                ALTER TABLE outbound_transactions
                    DROP CONSTRAINT IF EXISTS check_outbound_source_exists,
                    DROP CONSTRAINT IF EXISTS fk_outbound_session,
                    DROP CONSTRAINT IF EXISTS fk_outbound_exception,
                    DROP CONSTRAINT IF EXISTS fk_outbound_payout,
                    DROP CONSTRAINT IF EXISTS fk_outbound_withdrawal,
                    DROP CONSTRAINT IF EXISTS fk_outbound_parent,
                    DROP COLUMN IF EXISTS environment,
                    DROP COLUMN IF EXISTS payout_id,
                    DROP COLUMN IF EXISTS withdrawal_id,
                    DROP COLUMN IF EXISTS parent_transaction_id,
                    DROP COLUMN IF EXISTS purpose,
                    DROP COLUMN IF EXISTS provider_reference,
                    DROP COLUMN IF EXISTS signed_payload_encrypted,
                    DROP COLUMN IF EXISTS nonce,
                    DROP COLUMN IF EXISTS expires_at,
                    DROP COLUMN IF EXISTS last_valid_block_height,
                    DROP COLUMN IF EXISTS broadcast_attempts,
                    DROP COLUMN IF EXISTS last_broadcast_at,
                    DROP COLUMN IF EXISTS observed_at,
                    DROP COLUMN IF EXISTS updated_at;

                UPDATE outbound_transactions
                SET state = 'Pending'
                WHERE state IN ('Preparing', 'Signed', 'BroadcastUnknown');

                UPDATE outbound_transactions
                SET state = 'Failed'
                WHERE state IN ('Reverted', 'Expired', 'Replaced');

                ALTER TABLE outbound_transactions RENAME COLUMN operation_type TO sweep_type;
                ALTER TABLE outbound_transactions RENAME COLUMN state TO status;
                ALTER TABLE outbound_transactions RENAME TO sweep_transactions;
                ALTER TABLE sweep_transactions RENAME CONSTRAINT outbound_transactions_pkey
                    TO sweep_transactions_pkey;
                ALTER INDEX IF EXISTS idx_outbound_session
                    RENAME TO idx_sweep_transactions_session;
                ALTER INDEX IF EXISTS idx_outbound_exception
                    RENAME TO idx_sweep_transactions_exception;

                ALTER TABLE sweep_transactions
                    ADD CONSTRAINT check_source_exists
                    CHECK (session_id IS NOT NULL OR exception_id IS NOT NULL),
                    ADD CONSTRAINT fk_sweep_transactions_exception
                        FOREIGN KEY (exception_id) REFERENCES payment_exceptions(id)
                        ON DELETE SET NULL;

                ALTER TABLE withdrawals DROP COLUMN IF EXISTS updated_at;
                "#,
            )
            .await?;

        Ok(())
    }
}
