//! Durable journal for signed outbound blockchain transactions.

use crate::crypto::{decrypt_aes_gcm, encrypt_aes_gcm};
use crate::entity::outbound_transactions::{self, OutboundState};
use anyhow::{anyhow, Context, Result};
use chrono::{TimeZone, Utc};
use sea_orm::{
    sea_query::Expr, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter,
};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OutboundTransactionStore {
    db: DatabaseConnection,
    encryption_key: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "chain", rename_all = "snake_case")]
pub enum StoredSignedTransaction {
    Evm {
        tx_hash: String,
        raw_tx_hex: String,
        from_address: String,
        nonce: u64,
    },
    Tron {
        tx_hash: String,
        raw_data_hex: String,
        signature_hex: String,
        raw_data_json: Option<serde_json::Value>,
        expiration_ms: Option<i64>,
    },
    Solana {
        tx_hash: String,
        serialized_tx: String,
        recent_blockhash: String,
        last_valid_block_height: u64,
    },
}

impl StoredSignedTransaction {
    pub fn tx_hash(&self) -> &str {
        match self {
            Self::Evm { tx_hash, .. }
            | Self::Tron { tx_hash, .. }
            | Self::Solana { tx_hash, .. } => tx_hash,
        }
    }

    fn nonce(&self) -> Option<i64> {
        match self {
            Self::Evm { nonce, .. } => i64::try_from(*nonce).ok(),
            _ => None,
        }
    }

    fn expires_at(&self) -> Option<chrono::DateTime<chrono::FixedOffset>> {
        let Self::Tron {
            expiration_ms: Some(expiration_ms),
            ..
        } = self
        else {
            return None;
        };

        Utc.timestamp_millis_opt(*expiration_ms)
            .single()
            .map(Into::into)
    }

    fn last_valid_block_height(&self) -> Option<i64> {
        match self {
            Self::Solana {
                last_valid_block_height,
                ..
            } => i64::try_from(*last_valid_block_height).ok(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BroadcastDisposition {
    Accepted,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryDisposition {
    Pending,
    BroadcastUnknown(String),
    Expired,
    Replaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvidence {
    Staged,
    Ready,
    Conflict,
}

const TERMINAL_EVIDENCE_GRACE_SECONDS: i64 = 30;

impl OutboundTransactionStore {
    pub fn try_new(db: DatabaseConnection, encryption_key: Secret<String>) -> Result<Self> {
        let decoded = hex::decode(encryption_key.expose_secret())
            .context("ENCRYPTION_KEY must be valid hex for outbound transaction storage")?;
        let encryption_key: [u8; 32] = decoded.try_into().map_err(|bytes: Vec<u8>| {
            anyhow!(
                "ENCRYPTION_KEY must decode to exactly 32 bytes, got {}",
                bytes.len()
            )
        })?;
        Ok(Self { db, encryption_key })
    }

    pub fn for_tests(db: DatabaseConnection) -> Self {
        Self {
            db,
            encryption_key: *b"0123456789abcdef0123456789abcdef",
        }
    }

    /// Complete the executor-to-service handoff.
    ///
    /// In-process/mock executors can still return a hash without persisting a
    /// signed payload, so they transition directly from Preparing. Production
    /// executors persist Signed before broadcasting; for those, an exact hash
    /// match in any post-signing state makes this operation idempotent.
    pub async fn adopt_executor_result(
        &self,
        outbound_id: &str,
        tx_hash: &str,
        disposition: BroadcastDisposition,
    ) -> Result<bool> {
        let state = match disposition {
            BroadcastDisposition::Accepted => OutboundState::Pending,
            BroadcastDisposition::Unknown => OutboundState::BroadcastUnknown,
        };
        let result = outbound_transactions::Entity::update_many()
            .col_expr(outbound_transactions::Column::State, Expr::value(state))
            .col_expr(
                outbound_transactions::Column::TxHash,
                Expr::value(Some(tx_hash.to_string())),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
            .exec(&self.db)
            .await?;
        if result.rows_affected == 1 {
            return Ok(true);
        }

        let Some(outbound) = outbound_transactions::Entity::find_by_id(outbound_id)
            .one(&self.db)
            .await?
        else {
            return Ok(false);
        };
        let hash_matches = outbound
            .tx_hash
            .as_deref()
            .map(|stored| stored.eq_ignore_ascii_case(tx_hash))
            .unwrap_or(false);
        let handed_off = matches!(
            outbound.state,
            OutboundState::Signed
                | OutboundState::BroadcastUnknown
                | OutboundState::Pending
                | OutboundState::Confirmed
                | OutboundState::Reverted
                | OutboundState::Expired
                | OutboundState::Replaced
        );
        Ok(hash_matches && handed_off)
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Persist the exact signed bytes before the first network call.
    pub async fn record_signed(
        &self,
        outbound_id: &str,
        payload: &StoredSignedTransaction,
    ) -> Result<()> {
        let plaintext = serde_json::to_string(payload)?;
        let encrypted = encrypt_aes_gcm(&plaintext, &self.encryption_key)?;

        let result = outbound_transactions::Entity::update_many()
            .col_expr(
                outbound_transactions::Column::State,
                Expr::value(OutboundState::Signed),
            )
            .col_expr(
                outbound_transactions::Column::TxHash,
                Expr::value(Some(payload.tx_hash().to_string())),
            )
            .col_expr(
                outbound_transactions::Column::SignedPayloadEncrypted,
                Expr::value(Some(encrypted)),
            )
            .col_expr(
                outbound_transactions::Column::Nonce,
                Expr::value(payload.nonce()),
            )
            .col_expr(
                outbound_transactions::Column::ExpiresAt,
                Expr::value(payload.expires_at()),
            )
            .col_expr(
                outbound_transactions::Column::LastValidBlockHeight,
                Expr::value(payload.last_valid_block_height()),
            )
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
            .exec(&self.db)
            .await?;

        if result.rows_affected != 1 {
            return Err(anyhow!(
                "Outbound transaction {} was not in Preparing state",
                outbound_id
            ));
        }

        Ok(())
    }

    pub async fn mark_broadcast(
        &self,
        outbound_id: &str,
        disposition: BroadcastDisposition,
        error: Option<String>,
    ) -> Result<bool> {
        let state = match disposition {
            BroadcastDisposition::Accepted => OutboundState::Pending,
            BroadcastDisposition::Unknown => OutboundState::BroadcastUnknown,
        };

        let result = outbound_transactions::Entity::update_many()
            .col_expr(outbound_transactions::Column::State, Expr::value(state))
            .col_expr(
                outbound_transactions::Column::BroadcastAttempts,
                Expr::cust("broadcast_attempts + 1"),
            )
            .col_expr(
                outbound_transactions::Column::LastBroadcastAt,
                Expr::cust("NOW()"),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(error),
            )
            .col_expr(
                outbound_transactions::Column::ObservedAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.is_in([
                OutboundState::Signed,
                OutboundState::BroadcastUnknown,
                OutboundState::Pending,
            ]))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected == 1)
    }

    pub async fn mark_state(
        &self,
        outbound_id: &str,
        state: OutboundState,
        error: Option<String>,
    ) -> Result<bool> {
        self.mark_state_on(&self.db, outbound_id, state, error)
            .await
    }

    pub async fn mark_state_on<C: ConnectionTrait>(
        &self,
        db: &C,
        outbound_id: &str,
        state: OutboundState,
        error: Option<String>,
    ) -> Result<bool> {
        let terminal = matches!(state, OutboundState::Confirmed);
        let mut update = outbound_transactions::Entity::update_many()
            .col_expr(outbound_transactions::Column::State, Expr::value(state))
            .col_expr(
                outbound_transactions::Column::ObservedAt,
                Expr::cust("NOW()"),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(error),
            );
        if terminal {
            update = update.col_expr(
                outbound_transactions::Column::ConfirmedAt,
                Expr::cust("NOW()"),
            );
        }
        let result = update
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.is_in([
                OutboundState::Signed,
                OutboundState::BroadcastUnknown,
                OutboundState::Pending,
            ]))
            .exec(db)
            .await?;
        Ok(result.rows_affected == 1)
    }

    pub async fn mark_preparing_failed(&self, outbound_id: &str, error: String) -> Result<bool> {
        let result = outbound_transactions::Entity::update_many()
            .col_expr(
                outbound_transactions::Column::State,
                Expr::value(OutboundState::Failed),
            )
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(Some(error)),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected == 1)
    }

    /// Require the same evidence-backed terminal verdict to survive a grace period.
    /// Every caller re-runs its multi-provider chain checks before invoking this method,
    /// so `Ready` represents two independent observations separated in time.
    pub async fn stage_terminal_evidence(
        &self,
        outbound_id: &str,
        state: OutboundState,
        reason: &str,
    ) -> Result<TerminalEvidence> {
        if !matches!(state, OutboundState::Expired | OutboundState::Replaced) {
            return Err(anyhow!("Unsupported staged terminal state: {state:?}"));
        }

        let marker = format!("terminal-candidate:{state:?}:{reason}");
        let outbound = match outbound_transactions::Entity::find_by_id(outbound_id)
            .one(&self.db)
            .await?
        {
            Some(outbound) => outbound,
            None => return Ok(TerminalEvidence::Conflict),
        };
        if !matches!(
            outbound.state,
            OutboundState::Signed | OutboundState::BroadcastUnknown | OutboundState::Pending
        ) {
            return Ok(TerminalEvidence::Conflict);
        }

        let cutoff = Utc::now() - chrono::Duration::seconds(TERMINAL_EVIDENCE_GRACE_SECONDS);
        if outbound.error_message.as_deref() == Some(marker.as_str())
            && outbound
                .observed_at
                .map(|observed| observed.with_timezone(&Utc) <= cutoff)
                .unwrap_or(false)
        {
            return Ok(TerminalEvidence::Ready);
        }

        let result = outbound_transactions::Entity::update_many()
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(Some(marker)),
            )
            .col_expr(
                outbound_transactions::Column::ObservedAt,
                Expr::cust("NOW()"),
            )
            .col_expr(
                outbound_transactions::Column::UpdatedAt,
                Expr::cust("NOW()"),
            )
            .filter(outbound_transactions::Column::Id.eq(outbound_id))
            .filter(outbound_transactions::Column::State.is_in([
                OutboundState::Signed,
                OutboundState::BroadcastUnknown,
                OutboundState::Pending,
            ]))
            .exec(&self.db)
            .await?;

        Ok(if result.rows_affected == 1 {
            TerminalEvidence::Staged
        } else {
            TerminalEvidence::Conflict
        })
    }

    pub fn decrypt_payload(
        &self,
        outbound: &outbound_transactions::Model,
    ) -> Result<StoredSignedTransaction> {
        let encrypted = outbound
            .signed_payload_encrypted
            .as_deref()
            .ok_or_else(|| anyhow!("Outbound transaction {} has no signed payload", outbound.id))?;
        let plaintext = decrypt_aes_gcm(encrypted, &self.encryption_key)?;
        serde_json::from_str(&plaintext).context("Invalid stored signed transaction payload")
    }

    pub async fn find_for_payout_tx(
        &self,
        payout_id: &str,
        tx_hash: &str,
    ) -> Result<Option<outbound_transactions::Model>> {
        Ok(outbound_transactions::Entity::find()
            .filter(outbound_transactions::Column::PayoutId.eq(payout_id))
            .filter(outbound_transactions::Column::TxHash.eq(tx_hash))
            .filter(
                outbound_transactions::Column::Purpose
                    .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
            )
            .filter(outbound_transactions::Column::ParentTransactionId.is_null())
            .one(&self.db)
            .await?)
    }

    pub async fn find_for_withdrawal_tx(
        &self,
        withdrawal_id: &str,
        tx_hash: &str,
    ) -> Result<Option<outbound_transactions::Model>> {
        Ok(outbound_transactions::Entity::find()
            .filter(outbound_transactions::Column::WithdrawalId.eq(withdrawal_id))
            .filter(outbound_transactions::Column::TxHash.eq(tx_hash))
            .filter(
                outbound_transactions::Column::Purpose
                    .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
            )
            .filter(outbound_transactions::Column::ParentTransactionId.is_null())
            .one(&self.db)
            .await?)
    }

    pub async fn create_child_attempt(
        &self,
        parent_id: &str,
        purpose: outbound_transactions::OutboundPurpose,
        from_address: String,
        to_address: String,
        amount: i64,
        token: String,
    ) -> Result<outbound_transactions::Model> {
        let parent = outbound_transactions::Entity::find_by_id(parent_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Parent outbound transaction {} not found", parent_id))?;
        let mut child = preparing_model(
            new_id(),
            parent.merchant_id,
            parent.environment,
            parent.operation_type,
            parent.network,
            from_address,
            to_address,
            amount,
            token,
        );
        child.parent_transaction_id = Set(Some(parent.id));
        child.purpose = Set(purpose);
        create_attempt(&self.db, child).await
    }
}

pub async fn create_attempt(
    db: &DatabaseConnection,
    model: outbound_transactions::ActiveModel,
) -> Result<outbound_transactions::Model> {
    use sea_orm::ActiveModelTrait;
    Ok(model.insert(db).await?)
}

pub fn new_id() -> String {
    format!("otx_{}", uuid::Uuid::new_v4().simple())
}

pub fn preparing_model(
    id: String,
    merchant_id: String,
    environment: crate::entity::Environment,
    operation_type: outbound_transactions::OutboundOperationType,
    network: String,
    from_address: String,
    to_address: String,
    amount: i64,
    token: String,
) -> outbound_transactions::ActiveModel {
    outbound_transactions::ActiveModel {
        id: Set(id),
        merchant_id: Set(merchant_id),
        environment: Set(environment),
        operation_type: Set(operation_type),
        purpose: Set(outbound_transactions::OutboundPurpose::TokenTransfer),
        network: Set(network),
        from_address: Set(from_address),
        to_address: Set(to_address),
        amount: Set(amount),
        state: Set(OutboundState::Preparing),
        token: Set(token),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_payload_round_trips_through_json() {
        let payload = StoredSignedTransaction::Evm {
            tx_hash: "0xabc".into(),
            raw_tx_hex: "0xdeadbeef".into(),
            from_address: "0x123".into(),
            nonce: 42,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(
            serde_json::from_str::<StoredSignedTransaction>(&json).unwrap(),
            payload
        );
    }
}
