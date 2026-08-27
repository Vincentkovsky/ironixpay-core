//! Xero Accounting Integration Service
//!
//! Handles OAuth token management, Xero API calls, and session sync orchestration.

pub mod client;
pub mod worker;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, InsertResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use secrecy::{ExposeSecret, Secret};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use url::form_urlencoded;
use uuid::Uuid;

use crate::crypto::{decrypt_aes_gcm, encrypt_aes_gcm};
use crate::entity::{
    xero_connections, xero_oauth_states, xero_sync_logs, Environment, XeroConnections, XeroSyncLogs,
};
use crate::services::exchange_rate::ExchangeRateService;

use self::client::{XeroApiClient, XeroTokens};

/// Retry delays in seconds (aligned with webhook retry pattern).
const RETRY_DELAYS_SECS: [i64; 5] = [
    0,     // Attempt 0: immediate
    60,    // Attempt 1: 1 minute
    300,   // Attempt 2: 5 minutes
    1800,  // Attempt 3: 30 minutes
    21600, // Attempt 4: 6 hours
];

const MAX_ATTEMPTS: i32 = 5;

#[derive(Debug, Clone)]
struct SyncAmountSnapshot {
    fx_rate: Decimal,
    source_currency: String,
    target_currency: String,
    converted_gross: Decimal,
    converted_fee: Decimal,
    converted_net: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum XeroConfigError {
    #[error("Cannot enable auto sync without {field}")]
    MissingField { field: &'static str },
    #[error("{message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
}

/// Xero OAuth scopes required for integration.
const XERO_SCOPES: &str = "openid profile email offline_access accounting.invoices accounting.payments accounting.contacts accounting.settings.read";

pub struct XeroService {
    db: DatabaseConnection,
    pub(crate) client: XeroApiClient,
    exchange_rate_service: Option<Arc<ExchangeRateService>>,
    /// Cached key bytes derived from hex-encoded encryption_key.
    key_bytes: [u8; 32],
    /// Per-connection mutex to serialize token refresh (prevents race on refresh_token rotation).
    /// Key: connection_id. DashMap::get() Ref must NOT be held across .await.
    token_locks: DashMap<Uuid, Arc<Mutex<()>>>,
    /// Per-connection mutex to serialize contact creation (prevents duplicate contacts).
    contact_locks: DashMap<Uuid, Arc<Mutex<()>>>,
    /// Xero OAuth config
    pub client_id: String,
    pub client_secret: Secret<String>,
    pub redirect_uri: String,
}

impl XeroService {
    pub fn new(
        db: DatabaseConnection,
        encryption_key: Secret<String>,
        client_id: String,
        client_secret: Secret<String>,
        redirect_uri: String,
        exchange_rate_service: Option<Arc<ExchangeRateService>>,
    ) -> Self {
        let key_bytes = {
            let hex_str = encryption_key.expose_secret();
            let vec = hex::decode(hex_str).expect("ENCRYPTION_KEY must be valid hex");
            let mut key = [0u8; 32];
            key.copy_from_slice(&vec);
            key
        };
        Self {
            db: db.clone(),
            client: XeroApiClient::new(),
            exchange_rate_service,
            key_bytes,
            token_locks: DashMap::new(),
            contact_locks: DashMap::new(),
            client_id,
            client_secret,
            redirect_uri,
        }
    }

    // ─── OAuth Flow ───

    /// Get cached key bytes for encryption/decryption.
    fn get_key_bytes(&self) -> [u8; 32] {
        self.key_bytes
    }

    /// Encrypt a state payload (used for OAuth state parameter).
    pub fn encrypt_state(&self, payload: &str) -> Result<String> {
        encrypt_aes_gcm(payload, &self.key_bytes)
    }

    /// Decrypt a state payload (used for OAuth callback verification).
    pub fn decrypt_state(&self, encrypted: &str) -> Result<String> {
        decrypt_aes_gcm(encrypted, &self.key_bytes)
    }

    /// Remove expired/consumed OAuth nonces from DB.
    async fn prune_expired_oauth_state_nonces(&self) -> Result<()> {
        use sea_orm::QueryFilter;

        let now = Utc::now();
        xero_oauth_states::Entity::delete_many()
            .filter(
                xero_oauth_states::Column::ExpiresAt
                    .lt(now)
                    .or(xero_oauth_states::Column::ConsumedAt.lt(now - ChronoDuration::hours(1))),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// Issue encrypted OAuth state with one-time nonce.
    /// Format: merchant_id:environment:timestamp:nonce
    pub async fn issue_oauth_state(
        &self,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<String> {
        // Best effort cleanup to keep table compact.
        if let Err(e) = self.prune_expired_oauth_state_nonces().await {
            warn!(error = %e, "Failed to prune expired Xero OAuth states");
        }

        let now = Utc::now();
        let env_str = format!("{:?}", environment).to_lowercase();
        let timestamp = now.timestamp();
        let nonce = Uuid::new_v4().to_string();
        let state_payload = format!("{}:{}:{}:{}", merchant_id, env_str, timestamp, nonce);

        // Persist nonce for one-time callback verification.
        let model = xero_oauth_states::ActiveModel {
            nonce: Set(nonce),
            merchant_id: Set(merchant_id.to_string()),
            environment: Set(environment),
            expires_at: Set((now + ChronoDuration::minutes(10)).into()),
            consumed_at: Set(None),
            created_at: Set(now.into()),
        };
        model.insert(&self.db).await?;

        self.encrypt_state(&state_payload)
    }

    /// Verify and consume OAuth state nonce (one-time).
    pub async fn verify_and_consume_oauth_state(
        &self,
        encrypted_state: &str,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<()> {
        // Best effort cleanup to keep table compact.
        if let Err(e) = self.prune_expired_oauth_state_nonces().await {
            warn!(error = %e, "Failed to prune expired Xero OAuth states");
        }

        let state_payload = self.decrypt_state(encrypted_state)?;
        let parts: Vec<&str> = state_payload.splitn(4, ':').collect();
        if parts.len() != 4 {
            return Err(anyhow!("Invalid OAuth state format"));
        }

        let expected_env = format!("{:?}", environment).to_lowercase();
        if parts[0] != merchant_id || parts[1] != expected_env {
            return Err(anyhow!("OAuth state mismatch"));
        }

        let ts = parts[2]
            .parse::<i64>()
            .map_err(|_| anyhow!("Invalid OAuth state timestamp"))?;
        let age = Utc::now().timestamp() - ts;
        if age > 600 || age < -60 {
            return Err(anyhow!("OAuth state expired"));
        }

        let nonce = parts[3];
        if nonce.is_empty() {
            return Err(anyhow!("Missing OAuth state nonce"));
        }

        // Atomically consume nonce: only one callback can succeed.
        let now = Utc::now();
        let res = xero_oauth_states::Entity::update_many()
            .col_expr(
                xero_oauth_states::Column::ConsumedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(xero_oauth_states::Column::Nonce.eq(nonce))
            .filter(xero_oauth_states::Column::MerchantId.eq(merchant_id))
            .filter(xero_oauth_states::Column::Environment.eq(environment))
            .filter(xero_oauth_states::Column::ConsumedAt.is_null())
            .filter(xero_oauth_states::Column::ExpiresAt.gte(now))
            .exec(&self.db)
            .await?;

        if res.rows_affected == 1 {
            Ok(())
        } else {
            Err(anyhow!(
                "OAuth state nonce not found, expired, or already used"
            ))
        }
    }

    /// Encrypt a token value (used for storing tokens in pending connections).
    pub fn encrypt_token(&self, token: &str) -> Result<String> {
        encrypt_aes_gcm(token, &self.key_bytes)
    }

    /// Delete a connection by ID (cleanup helper).
    pub async fn delete_connection(&self, connection_id: Uuid) -> Result<()> {
        self.token_locks.remove(&connection_id);
        self.contact_locks.remove(&connection_id);
        xero_connections::Entity::delete_by_id(connection_id)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn skip_pending_sync_logs_for_connection<C: ConnectionTrait>(
        &self,
        db: &C,
        connection_id: Uuid,
        reason: &str,
    ) -> Result<u64> {
        let result = db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"UPDATE xero_sync_logs
                   SET status = 'skipped',
                       last_error = $1,
                       next_retry_at = NULL,
                       updated_at = NOW()
                   WHERE connection_id = $2 AND status IN ('pending', 'failed')"#,
                [reason.into(), connection_id.into()],
            ))
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn skip_pending_sync_logs_for_connection_id(
        &self,
        connection_id: Uuid,
        reason: &str,
    ) -> Result<u64> {
        self.skip_pending_sync_logs_for_connection(&self.db, connection_id, reason)
            .await
    }

    /// Generate Xero authorization URL for OAuth 2.0 code flow.
    /// `state` should be an encrypted token containing merchant_id + environment + nonce.
    pub fn authorize_url(&self, state: &str) -> String {
        let query: String = form_urlencoded::Serializer::new(String::new())
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", XERO_SCOPES)
            .append_pair("state", state)
            .finish();
        format!(
            "https://login.xero.com/identity/connect/authorize?{}",
            query
        )
    }

    /// Exchange authorization code for tokens and fetch tenant info.
    pub async fn exchange_code(&self, code: &str) -> Result<(XeroTokens, Vec<client::XeroTenant>)> {
        let tokens = self
            .client
            .exchange_code(
                code,
                &self.client_id,
                self.client_secret.expose_secret(),
                &self.redirect_uri,
            )
            .await?;

        let tenants = self.client.get_connections(&tokens.access_token).await?;

        Ok((tokens, tenants))
    }

    fn lock_key(merchant_id: &str, environment: Environment) -> String {
        format!("xero_connection:{}:{}", merchant_id, environment)
    }

    async fn acquire_connection_lock<C: ConnectionTrait>(
        &self,
        db: &C,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<()> {
        let key = Self::lock_key(merchant_id, environment);
        db.query_one(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtext($1));",
            [key.into()],
        ))
        .await?;
        Ok(())
    }

    async fn get_connection_for_update<C: ConnectionTrait>(
        &self,
        db: &C,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<Option<xero_connections::Model>> {
        let conn = XeroConnections::find()
            .filter(xero_connections::Column::MerchantId.eq(merchant_id))
            .filter(xero_connections::Column::Environment.eq(environment))
            .one(db)
            .await?;
        Ok(conn)
    }

    /// Save or refresh a Xero connection after OAuth completion.
    /// If an existing connection exists for this merchant+environment, update it in-place
    /// to preserve historical sync logs linked by connection_id.
    pub async fn save_connection(
        &self,
        merchant_id: &str,
        environment: Environment,
        tokens: &XeroTokens,
        tenant: &client::XeroTenant,
        default_currency: &str,
    ) -> Result<xero_connections::Model> {
        let now = Utc::now().into();
        let access_encrypted = encrypt_aes_gcm(&tokens.access_token, &self.get_key_bytes())?;
        let refresh_encrypted = encrypt_aes_gcm(&tokens.refresh_token, &self.get_key_bytes())?;
        let expires_at = Utc::now() + ChronoDuration::seconds(tokens.expires_in as i64);
        let txn = self.db.begin().await?;
        self.acquire_connection_lock(&txn, merchant_id, environment)
            .await?;

        // Update existing connection in-place to avoid deleting sync logs via ON DELETE CASCADE.
        if let Some(existing) = self
            .get_connection_for_update(&txn, merchant_id, environment)
            .await?
        {
            let tenant_changed = existing.xero_tenant_id != tenant.tenant_id;

            let mut active: xero_connections::ActiveModel = existing.into();
            active.access_token_encrypted = Set(access_encrypted);
            active.refresh_token_encrypted = Set(refresh_encrypted);
            active.token_expires_at = Set(expires_at.into());
            active.xero_tenant_id = Set(tenant.tenant_id.clone());
            active.xero_tenant_name = Set(Some(tenant.tenant_name.clone()));
            active.default_currency = Set(default_currency.to_string());
            active.status = Set(xero_connections::XeroConnectionStatus::Active);
            active.updated_at = Set(now);

            // Tenant switched: old chart mappings/contact may not exist in new org.
            if tenant_changed {
                active.xero_account_code = Set(None);
                active.xero_fee_account_code = Set(None);
                active.xero_payment_account_code = Set(None);
                active.xero_tax_type = Set("NONE".to_string());
                active.xero_contact_id = Set(None);
                active.auto_sync_enabled = Set(false);
            }

            let result = active.update(&txn).await?;
            if tenant_changed {
                let skipped = self
                    .skip_pending_sync_logs_for_connection(
                        &txn,
                        result.id,
                        "Xero tenant changed; pending sync logs skipped for safety",
                    )
                    .await?;
                if skipped > 0 {
                    warn!(
                        connection_id = %result.id,
                        skipped,
                        "Skipped pending Xero sync logs after tenant switch"
                    );
                }
            }
            txn.commit().await?;
            info!(merchant_id = %merchant_id, tenant = %tenant.tenant_name, "Xero connection refreshed");
            return Ok(result);
        }

        let model = xero_connections::ActiveModel {
            id: Set(Uuid::new_v4()),
            merchant_id: Set(merchant_id.to_string()),
            environment: Set(environment),
            access_token_encrypted: Set(access_encrypted),
            refresh_token_encrypted: Set(refresh_encrypted),
            token_expires_at: Set(expires_at.into()),
            xero_tenant_id: Set(tenant.tenant_id.clone()),
            xero_tenant_name: Set(Some(tenant.tenant_name.clone())),
            xero_account_code: Set(None),
            xero_fee_account_code: Set(None),
            xero_payment_account_code: Set(None),
            xero_tax_type: Set("NONE".to_string()),
            xero_contact_id: Set(None),
            default_currency: Set(default_currency.to_string()),
            auto_sync_enabled: Set(false),
            status: Set(xero_connections::XeroConnectionStatus::Active),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = model.insert(&txn).await?;
        txn.commit().await?;
        info!(merchant_id = %merchant_id, tenant = %tenant.tenant_name, "Xero connection saved");
        Ok(result)
    }

    /// Save/update connection as pending_selection for multi-tenant OAuth callback.
    /// Uses DB transaction + advisory lock to avoid concurrent callback races.
    pub async fn save_pending_selection_connection(
        &self,
        merchant_id: &str,
        environment: Environment,
        tokens: &XeroTokens,
        first_tenant: &client::XeroTenant,
    ) -> Result<xero_connections::Model> {
        let now = Utc::now().into();
        let access_encrypted = self.encrypt_token(&tokens.access_token)?;
        let refresh_encrypted = self.encrypt_token(&tokens.refresh_token)?;
        let expires_at = Utc::now() + ChronoDuration::seconds(tokens.expires_in as i64);

        let txn = self.db.begin().await?;
        self.acquire_connection_lock(&txn, merchant_id, environment)
            .await?;

        if let Some(existing) = self
            .get_connection_for_update(&txn, merchant_id, environment)
            .await?
        {
            let tenant_changed = existing.xero_tenant_id != first_tenant.tenant_id;
            let mut active: xero_connections::ActiveModel = existing.into();
            active.access_token_encrypted = Set(access_encrypted);
            active.refresh_token_encrypted = Set(refresh_encrypted);
            active.token_expires_at = Set(expires_at.into());
            active.xero_tenant_id = Set(first_tenant.tenant_id.clone());
            active.xero_tenant_name = Set(Some(first_tenant.tenant_name.clone()));
            active.default_currency = Set("USD".to_string());
            active.auto_sync_enabled = Set(false);
            active.status = Set(xero_connections::XeroConnectionStatus::PendingSelection);
            active.updated_at = Set(now);

            if tenant_changed {
                active.xero_account_code = Set(None);
                active.xero_fee_account_code = Set(None);
                active.xero_payment_account_code = Set(None);
                active.xero_tax_type = Set("NONE".to_string());
                active.xero_contact_id = Set(None);
            }

            let result = active.update(&txn).await?;
            if tenant_changed {
                let skipped = self
                    .skip_pending_sync_logs_for_connection(
                        &txn,
                        result.id,
                        "Xero tenant changed; pending sync logs skipped for safety",
                    )
                    .await?;
                if skipped > 0 {
                    warn!(
                        connection_id = %result.id,
                        skipped,
                        "Skipped pending Xero sync logs after pending tenant switch"
                    );
                }
            }
            txn.commit().await?;
            return Ok(result);
        }

        let model = xero_connections::ActiveModel {
            id: Set(Uuid::new_v4()),
            merchant_id: Set(merchant_id.to_string()),
            environment: Set(environment),
            access_token_encrypted: Set(access_encrypted),
            refresh_token_encrypted: Set(refresh_encrypted),
            token_expires_at: Set(expires_at.into()),
            xero_tenant_id: Set(first_tenant.tenant_id.clone()),
            xero_tenant_name: Set(Some(first_tenant.tenant_name.clone())),
            xero_account_code: Set(None),
            xero_fee_account_code: Set(None),
            xero_payment_account_code: Set(None),
            xero_tax_type: Set("NONE".to_string()),
            xero_contact_id: Set(None),
            default_currency: Set("USD".to_string()),
            auto_sync_enabled: Set(false),
            status: Set(xero_connections::XeroConnectionStatus::PendingSelection),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = model.insert(&txn).await?;
        txn.commit().await?;
        Ok(result)
    }

    /// Activate a selected tenant after pending-selection OAuth flow.
    /// If tenant changed, mappings/contact are reset and pending logs are skipped.
    pub async fn activate_selected_tenant(
        &self,
        connection: xero_connections::Model,
        selected: &client::XeroTenant,
        default_currency: &str,
    ) -> Result<xero_connections::Model> {
        let txn = self.db.begin().await?;
        self.acquire_connection_lock(&txn, &connection.merchant_id, connection.environment)
            .await?;
        let tenant_changed = connection.xero_tenant_id != selected.tenant_id;

        let mut active: xero_connections::ActiveModel = connection.into();
        active.xero_tenant_id = Set(selected.tenant_id.clone());
        active.xero_tenant_name = Set(Some(selected.tenant_name.clone()));
        active.default_currency = Set(default_currency.to_string());
        active.status = Set(xero_connections::XeroConnectionStatus::Active);
        // Require explicit merchant confirmation after mapping account/tax settings.
        active.auto_sync_enabled = Set(false);
        active.updated_at = Set(Utc::now().into());

        if tenant_changed {
            active.xero_account_code = Set(None);
            active.xero_fee_account_code = Set(None);
            active.xero_payment_account_code = Set(None);
            active.xero_tax_type = Set("NONE".to_string());
            active.xero_contact_id = Set(None);
        }

        let updated = active.update(&txn).await?;
        if tenant_changed {
            let skipped = self
                .skip_pending_sync_logs_for_connection(
                    &txn,
                    updated.id,
                    "Xero tenant changed; pending sync logs skipped for safety",
                )
                .await?;
            if skipped > 0 {
                warn!(
                    connection_id = %updated.id,
                    skipped,
                    "Skipped pending Xero sync logs after tenant selection switch"
                );
            }
        }
        txn.commit().await?;
        Ok(updated)
    }

    // ─── Token Management ───

    /// Get a valid access token for a connection, refreshing if expired.
    /// Uses per-connection mutex to prevent concurrent refresh_token races.
    pub async fn get_access_token(&self, connection: &xero_connections::Model) -> Result<String> {
        // Clone the mutex Arc BEFORE awaiting (DashMap Ref must not cross .await)
        let mutex = {
            self.token_locks
                .entry(connection.id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _guard = mutex.lock().await;

        // Re-fetch from DB to check if another task already refreshed
        let conn = XeroConnections::find_by_id(connection.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Xero connection not found"))?;

        if conn.token_expires_at > chrono::DateTime::<chrono::FixedOffset>::from(Utc::now()) {
            // Token still valid
            return decrypt_aes_gcm(&conn.access_token_encrypted, &self.get_key_bytes());
        }

        // Token expired, refresh
        let refresh_token = decrypt_aes_gcm(&conn.refresh_token_encrypted, &self.get_key_bytes())?;

        match self
            .client
            .refresh_tokens(
                &refresh_token,
                &self.client_id,
                self.client_secret.expose_secret(),
            )
            .await
        {
            Ok(new_tokens) => {
                let access_encrypted =
                    encrypt_aes_gcm(&new_tokens.access_token, &self.get_key_bytes())?;
                let refresh_encrypted =
                    encrypt_aes_gcm(&new_tokens.refresh_token, &self.get_key_bytes())?;
                let expires_at = Utc::now() + ChronoDuration::seconds(new_tokens.expires_in as i64);

                let mut active: xero_connections::ActiveModel = conn.into();
                active.access_token_encrypted = Set(access_encrypted);
                active.refresh_token_encrypted = Set(refresh_encrypted);
                active.token_expires_at = Set(expires_at.into());
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await?;

                debug!(connection_id = %connection.id, "Xero token refreshed");
                Ok(new_tokens.access_token)
            }
            Err(e) => {
                warn!(connection_id = %connection.id, error = %e, "Xero token refresh failed");

                // Mark connection as error
                let mut active: xero_connections::ActiveModel =
                    XeroConnections::find_by_id(connection.id)
                        .one(&self.db)
                        .await?
                        .ok_or_else(|| anyhow!("Connection not found"))?
                        .into();
                active.status = Set(xero_connections::XeroConnectionStatus::Error);
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await?;

                Err(anyhow!(
                    "Token refresh failed: {}. Merchant must reconnect Xero.",
                    e
                ))
            }
        }
    }

    // ─── Connection Queries ───

    /// Get active Xero connection for a merchant + environment.
    pub async fn get_connection(
        &self,
        merchant_id: &str,
        environment: Environment,
    ) -> Result<Option<xero_connections::Model>> {
        let conn = XeroConnections::find()
            .filter(xero_connections::Column::MerchantId.eq(merchant_id))
            .filter(xero_connections::Column::Environment.eq(environment))
            .one(&self.db)
            .await?;
        Ok(conn)
    }

    /// Get active connection by ID.
    pub async fn get_connection_by_id(&self, id: Uuid) -> Result<Option<xero_connections::Model>> {
        Ok(XeroConnections::find_by_id(id).one(&self.db).await?)
    }

    /// Update connection configuration.
    pub async fn update_connection_config(
        &self,
        connection_id: Uuid,
        account_code: Option<Option<String>>,
        fee_account_code: Option<Option<String>>,
        payment_account_code: Option<Option<String>>,
        tax_type: Option<Option<String>>,
        auto_sync: Option<bool>,
    ) -> Result<xero_connections::Model> {
        let conn = XeroConnections::find_by_id(connection_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Connection not found"))?;

        let should_validate_accounts = account_code.is_some()
            || fee_account_code.is_some()
            || payment_account_code.is_some()
            || auto_sync == Some(true);

        let mut next_account_code = conn.xero_account_code.clone();
        if let Some(input) = account_code.as_ref() {
            next_account_code = match input {
                Some(v) => Some(
                    Self::normalize_account_code("xero_account_code", &v).map_err(|e| {
                        XeroConfigError::InvalidField {
                            field: "xero_account_code",
                            message: e.to_string(),
                        }
                    })?,
                ),
                None => None,
            };
        }

        let mut next_fee_account_code = conn.xero_fee_account_code.clone();
        if let Some(input) = fee_account_code.as_ref() {
            next_fee_account_code = match input {
                Some(v) => Some(
                    Self::normalize_account_code("xero_fee_account_code", &v).map_err(|e| {
                        XeroConfigError::InvalidField {
                            field: "xero_fee_account_code",
                            message: e.to_string(),
                        }
                    })?,
                ),
                None => None,
            };
        }

        let mut next_payment_account_code = conn.xero_payment_account_code.clone();
        if let Some(input) = payment_account_code.as_ref() {
            next_payment_account_code = match input {
                Some(v) => Some(
                    Self::normalize_account_code("xero_payment_account_code", &v).map_err(|e| {
                        XeroConfigError::InvalidField {
                            field: "xero_payment_account_code",
                            message: e.to_string(),
                        }
                    })?,
                ),
                None => None,
            };
        }

        let mut next_tax_type = conn.xero_tax_type.clone();
        if let Some(input) = tax_type.as_ref() {
            next_tax_type = match input {
                Some(v) => {
                    Self::normalize_tax_type(&v).map_err(|e| XeroConfigError::InvalidField {
                        field: "xero_tax_type",
                        message: e.to_string(),
                    })?
                }
                None => "NONE".to_string(),
            };
        }

        let should_validate_tax = (tax_type.is_some() || auto_sync == Some(true))
            && !next_tax_type.eq_ignore_ascii_case("NONE");

        let next_auto_sync = auto_sync.unwrap_or(conn.auto_sync_enabled);
        if next_auto_sync {
            if next_account_code.is_none() {
                return Err(XeroConfigError::MissingField {
                    field: "xero_account_code",
                }
                .into());
            }
            if next_fee_account_code.is_none() {
                return Err(XeroConfigError::MissingField {
                    field: "xero_fee_account_code",
                }
                .into());
            }
            if next_payment_account_code.is_none() {
                return Err(XeroConfigError::MissingField {
                    field: "xero_payment_account_code",
                }
                .into());
            }
        }

        if should_validate_accounts || should_validate_tax {
            let access_token = self.get_access_token(&conn).await?;

            if should_validate_accounts {
                let accounts = self
                    .client
                    .get_accounts(&access_token, &conn.xero_tenant_id)
                    .await
                    .context("Failed to validate Xero account codes")?;
                let known_codes: HashSet<String> = accounts
                    .iter()
                    .filter_map(|a| a["Code"].as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let known_accounts: HashMap<String, &serde_json::Value> = accounts
                    .iter()
                    .filter_map(|a| {
                        let code = a["Code"].as_str()?.trim();
                        if code.is_empty() {
                            None
                        } else {
                            Some((code.to_string(), a))
                        }
                    })
                    .collect();

                for (field, code) in [
                    ("xero_account_code", next_account_code.as_deref()),
                    ("xero_fee_account_code", next_fee_account_code.as_deref()),
                    (
                        "xero_payment_account_code",
                        next_payment_account_code.as_deref(),
                    ),
                ] {
                    if let Some(code) = code {
                        if !known_codes.contains(code) {
                            return Err(XeroConfigError::InvalidField {
                                field,
                                message: format!(
                                    "{} '{}' does not exist in Xero chart of accounts",
                                    field, code
                                ),
                            }
                            .into());
                        }
                    }
                }

                if let Some(payment_code) = next_payment_account_code.as_deref() {
                    if let Some(account) = known_accounts.get(payment_code) {
                        let is_bank = account["Type"]
                            .as_str()
                            .map(|v| v.eq_ignore_ascii_case("BANK"))
                            .unwrap_or(false);
                        let can_receive_payment = account["EnablePaymentsToAccount"]
                            .as_bool()
                            .unwrap_or(false);
                        if !is_bank && !can_receive_payment {
                            return Err(XeroConfigError::InvalidField {
                                field: "xero_payment_account_code",
                                message: format!(
                                    "xero_payment_account_code '{}' is not a receivable payment account (expected BANK or EnablePaymentsToAccount=true)",
                                    payment_code
                                ),
                            }
                            .into());
                        }
                    }
                }
            }

            if should_validate_tax {
                let tax_rates = self
                    .client
                    .get_tax_rates(&access_token, &conn.xero_tenant_id)
                    .await
                    .context("Failed to validate Xero tax type")?;

                let is_valid = tax_rates.iter().any(|t| {
                    t["TaxType"]
                        .as_str()
                        .map(|s| s.eq_ignore_ascii_case(&next_tax_type))
                        .unwrap_or(false)
                        && t["Status"]
                            .as_str()
                            .map(|s| s.eq_ignore_ascii_case("ACTIVE"))
                            .unwrap_or(false)
                        && t["CanApplyToRevenue"].as_bool().unwrap_or(false)
                });

                if !is_valid {
                    return Err(XeroConfigError::InvalidField {
                        field: "xero_tax_type",
                        message: format!(
                            "xero_tax_type '{}' is not an active revenue tax rate in Xero",
                            next_tax_type
                        ),
                    }
                    .into());
                }
            }
        }

        let mut active: xero_connections::ActiveModel = conn.into();
        active.xero_account_code = Set(next_account_code);
        active.xero_fee_account_code = Set(next_fee_account_code);
        active.xero_payment_account_code = Set(next_payment_account_code);
        active.xero_tax_type = Set(next_tax_type);
        active.auto_sync_enabled = Set(next_auto_sync);
        active.updated_at = Set(Utc::now().into());
        Ok(active.update(&self.db).await?)
    }

    fn normalize_account_code(field: &str, raw: &str) -> Result<String> {
        let code = raw.trim();
        if code.is_empty() {
            return Err(anyhow!("{} cannot be empty", field));
        }
        if code.starts_with("__bank_no_code__") {
            return Err(anyhow!("{} is invalid", field));
        }
        if code.len() > 20 {
            return Err(anyhow!("{} is too long (max 20)", field));
        }
        if !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow!(
                "{} contains invalid characters (allowed: A-Z, a-z, 0-9, -, _)",
                field
            ));
        }
        Ok(code.to_string())
    }

    fn normalize_tax_type(raw: &str) -> Result<String> {
        let normalized = raw.trim().to_uppercase();
        if normalized.is_empty() {
            return Ok("NONE".to_string());
        }
        if normalized.len() > 50 {
            return Err(anyhow!("xero_tax_type is too long (max 50)"));
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(anyhow!(
                "xero_tax_type contains invalid characters (allowed: A-Z, 0-9, _, -)"
            ));
        }
        Ok(normalized)
    }

    /// Disconnect Xero (revoke token + mark disconnected).
    pub async fn disconnect(&self, connection_id: Uuid) -> Result<()> {
        let conn = XeroConnections::find_by_id(connection_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Connection not found"))?;

        // Revoke token (best effort — log result for audit)
        if let Ok(refresh_token) =
            decrypt_aes_gcm(&conn.refresh_token_encrypted, &self.get_key_bytes())
        {
            match self
                .client
                .revoke_token(
                    &refresh_token,
                    &self.client_id,
                    self.client_secret.expose_secret(),
                )
                .await
            {
                Ok(true) => {
                    info!(connection_id = %connection_id, "Xero token revoked successfully");
                }
                Ok(false) => {
                    warn!(connection_id = %connection_id, "Xero token revocation returned non-success — proceeding with local disconnect");
                }
                Err(e) => {
                    warn!(connection_id = %connection_id, error = %e, "Failed to revoke Xero token — proceeding with local disconnect");
                }
            }
        }

        let mut active: xero_connections::ActiveModel = conn.into();
        active.status = Set(xero_connections::XeroConnectionStatus::Disconnected);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        // Clean up locks
        self.token_locks.remove(&connection_id);
        self.contact_locks.remove(&connection_id);

        info!(connection_id = %connection_id, "Xero connection disconnected");
        Ok(())
    }

    // ─── Sync Enqueueing ───

    /// Enqueue a session for Xero sync (called from PaymentEventProcessor).
    /// Only inserts if the merchant has an active Xero connection with auto_sync enabled.
    pub async fn enqueue_sync_if_enabled(
        &self,
        merchant_id: &str,
        environment: Environment,
        session_id: &str,
    ) -> Result<()> {
        let conn = match self.get_connection(merchant_id, environment).await? {
            Some(c)
                if c.status == xero_connections::XeroConnectionStatus::Active
                    && c.auto_sync_enabled =>
            {
                c
            }
            _ => return Ok(()), // No active connection or sync disabled
        };

        let now = Utc::now().into();
        let log = xero_sync_logs::ActiveModel {
            id: Set(Uuid::new_v4()),
            connection_id: Set(conn.id),
            session_id: Set(session_id.to_string()),
            xero_invoice_id: Set(None),
            xero_payment_id: Set(None),
            status: Set(xero_sync_logs::XeroSyncStatus::Pending),
            attempt_count: Set(0),
            last_error: Set(None),
            next_retry_at: Set(None), // Immediate pickup
            fx_rate: Set(None),
            fx_source_currency: Set(None),
            fx_target_currency: Set(None),
            converted_gross: Set(None),
            converted_fee: Set(None),
            converted_net: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        let _result: InsertResult<xero_sync_logs::ActiveModel> =
            xero_sync_logs::Entity::insert(log)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::columns([
                        xero_sync_logs::Column::ConnectionId,
                        xero_sync_logs::Column::SessionId,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(&self.db)
                .await?;

        debug!(session_id = session_id, "Xero sync enqueued");
        Ok(())
    }

    fn microunits_to_decimal(amount: i64) -> Decimal {
        Decimal::from(amount) / Decimal::from(1_000_000_i64)
    }

    fn normalize_currency(code: &str) -> String {
        code.trim().to_uppercase()
    }

    fn resolve_fx_rate(&self, source_currency: &str, target_currency: &str) -> Result<Decimal> {
        if source_currency.eq_ignore_ascii_case(target_currency) {
            return Ok(Decimal::ONE);
        }

        let svc = self
            .exchange_rate_service
            .as_ref()
            .ok_or_else(|| anyhow!("Exchange rate service is not configured"))?;

        if !ExchangeRateService::is_supported_fiat(target_currency) {
            return Err(anyhow!(
                "Xero target currency '{}' is not supported by exchange rate service",
                target_currency
            ));
        }

        if !ExchangeRateService::is_crypto(source_currency) {
            return Err(anyhow!(
                "Unsupported source currency '{}' for Xero sync conversion",
                source_currency
            ));
        }

        svc.get_rate(source_currency, target_currency)
    }

    fn calculate_snapshot(
        &self,
        session: &crate::entity::checkout_sessions::Model,
        connection: &xero_connections::Model,
    ) -> Result<SyncAmountSnapshot> {
        let source_currency = Self::normalize_currency(&session.currency);
        let target_currency = Self::normalize_currency(&connection.default_currency);

        let gross = Self::microunits_to_decimal(session.amount_received);
        let fee = Self::microunits_to_decimal(session.fee_amount.unwrap_or(0));

        let fx_rate = self.resolve_fx_rate(&source_currency, &target_currency)?;
        let converted_gross = (gross * fx_rate).round_dp(2);
        let converted_fee = (fee * fx_rate).round_dp(2);
        // Keep payment amount deterministic and equal to invoice lines.
        let converted_net = (converted_gross - converted_fee).round_dp(2);

        Ok(SyncAmountSnapshot {
            fx_rate,
            source_currency,
            target_currency,
            converted_gross,
            converted_fee,
            converted_net,
        })
    }

    async fn load_or_create_snapshot(
        &self,
        sync_log: &xero_sync_logs::Model,
        session: &crate::entity::checkout_sessions::Model,
        connection: &xero_connections::Model,
    ) -> Result<SyncAmountSnapshot> {
        if let (
            Some(fx_rate),
            Some(source_currency),
            Some(target_currency),
            Some(converted_gross),
            Some(converted_fee),
            Some(converted_net),
        ) = (
            sync_log.fx_rate,
            sync_log.fx_source_currency.clone(),
            sync_log.fx_target_currency.clone(),
            sync_log.converted_gross,
            sync_log.converted_fee,
            sync_log.converted_net,
        ) {
            return Ok(SyncAmountSnapshot {
                fx_rate,
                source_currency,
                target_currency,
                converted_gross,
                converted_fee,
                converted_net,
            });
        }

        let snapshot = self.calculate_snapshot(session, connection)?;
        let mut active: xero_sync_logs::ActiveModel = sync_log.clone().into();
        active.fx_rate = Set(Some(snapshot.fx_rate));
        active.fx_source_currency = Set(Some(snapshot.source_currency.clone()));
        active.fx_target_currency = Set(Some(snapshot.target_currency.clone()));
        active.converted_gross = Set(Some(snapshot.converted_gross));
        active.converted_fee = Set(Some(snapshot.converted_fee));
        active.converted_net = Set(Some(snapshot.converted_net));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(snapshot)
    }

    // ─── Sync Execution ───

    /// Execute sync for a single session. Called by XeroSyncWorker.
    /// Implements checkpoint recovery: checks existing xero_invoice_id/xero_payment_id
    /// before making Xero API calls to avoid duplicate creation on retry.
    pub async fn sync_session(
        &self,
        sync_log: &xero_sync_logs::Model,
        connection: &xero_connections::Model,
    ) -> Result<()> {
        let access_token = self.get_access_token(connection).await?;
        let tenant_id = &connection.xero_tenant_id;

        // Load session data
        let session = crate::entity::checkout_sessions::Entity::find_by_id(&sync_log.session_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Session {} not found", sync_log.session_id))?;

        let snapshot = self
            .load_or_create_snapshot(sync_log, &session, connection)
            .await
            .context("Failed to resolve Xero sync FX snapshot")?;
        let converted_gross = snapshot.converted_gross;
        let converted_fee = snapshot.converted_fee;
        let converted_net = snapshot.converted_net;
        let mut current_log = XeroSyncLogs::find_by_id(sync_log.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Xero sync log {} not found", sync_log.id))?;

        // Step 1: Ensure Contact exists (cached)
        let contact_id = self
            .ensure_contact(&access_token, tenant_id, connection)
            .await?;

        // Step 2: Create Invoice (skip if already created — checkpoint recovery)
        let invoice_id = if let Some(ref existing) = current_log.xero_invoice_id {
            existing.clone()
        } else {
            let account_code = connection.xero_account_code.as_deref().unwrap_or("200");
            let fee_account_code = connection.xero_fee_account_code.as_deref().unwrap_or("404");
            let revenue_tax_type = if connection.xero_tax_type.trim().is_empty() {
                "NONE".to_string()
            } else {
                connection.xero_tax_type.trim().to_uppercase()
            };
            let line_amount_type = if revenue_tax_type == "NONE" {
                "NoTax"
            } else {
                "Exclusive"
            };

            let description = format!(
                "Crypto payment - {} {} on {}",
                Self::microunits_to_decimal(session.amount_received),
                session.currency,
                session.network
            );
            let fee_description = format!("IronixPay gateway fee");

            let mut line_items = vec![serde_json::json!({
                "Description": description,
                "Quantity": 1,
                "UnitAmount": converted_gross.to_string(),
                "AccountCode": account_code,
                "TaxType": revenue_tax_type,
            })];

            if converted_fee > rust_decimal::Decimal::ZERO {
                line_items.push(serde_json::json!({
                    "Description": fee_description,
                    "Quantity": 1,
                    "UnitAmount": format!("-{}", converted_fee),
                    "AccountCode": fee_account_code,
                    "TaxType": "NONE",
                }));
            }

            let completed_at = session.updated_at.to_rfc3339();
            let invoice_body = serde_json::json!({
                "Type": "ACCREC",
                "Contact": { "ContactID": contact_id },
                "Date": &completed_at[..10],
                "DueDate": &completed_at[..10],
                "InvoiceNumber": session.id,
                "Reference": session.client_reference_id,
                "CurrencyCode": connection.default_currency,
                // Default to NoTax; allow tax-inclusive behavior when merchant configures
                // a revenue tax type other than NONE.
                "LineAmountTypes": line_amount_type,
                "Status": "AUTHORISED",
                "LineItems": line_items,
            });

            let inv_id = self
                .client
                .create_invoice(
                    &access_token,
                    tenant_id,
                    &invoice_body,
                    &format!("{}-inv", sync_log.id),
                )
                .await
                .context("Failed to create Xero invoice")?;

            // Checkpoint: save invoice ID immediately
            let mut active: xero_sync_logs::ActiveModel = current_log.clone().into();
            active.xero_invoice_id = Set(Some(inv_id.clone()));
            active.updated_at = Set(Utc::now().into());
            current_log = active.update(&self.db).await?;

            inv_id
        };

        // Step 3: Create Payment (skip if already created)
        if current_log.xero_payment_id.is_some() {
            // Already fully synced
            let mut active: xero_sync_logs::ActiveModel = current_log.into();
            active.status = Set(xero_sync_logs::XeroSyncStatus::Synced);
            active.updated_at = Set(Utc::now().into());
            active.update(&self.db).await?;
            return Ok(());
        }

        let payment_account_code = connection
            .xero_payment_account_code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("xero_payment_account_code is not configured"))?;

        let completed_at = session.updated_at.to_rfc3339();
        let payment_body = serde_json::json!({
            "Invoice": { "InvoiceID": invoice_id },
            "Account": { "Code": payment_account_code },
            "Date": &completed_at[..10],
            "Amount": converted_net.to_string(),
            "Reference": format!("IronixPay {}", session.id),
        });

        let payment_id = self
            .client
            .create_payment(
                &access_token,
                tenant_id,
                &payment_body,
                &format!("{}-pay", sync_log.id),
            )
            .await
            .context("Failed to create Xero payment")?;

        // Final: Mark synced
        let mut active: xero_sync_logs::ActiveModel = current_log.into();
        active.xero_payment_id = Set(Some(payment_id));
        active.status = Set(xero_sync_logs::XeroSyncStatus::Synced);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        info!(session_id = %sync_log.session_id, "Xero sync completed");
        Ok(())
    }

    /// Ensure the IronixPay Contact exists in Xero, using cached ID when available.
    /// Uses per-connection mutex to prevent concurrent duplicate contact creation.
    async fn ensure_contact(
        &self,
        access_token: &str,
        tenant_id: &str,
        connection: &xero_connections::Model,
    ) -> Result<String> {
        // Fast path: cached contact ID
        if let Some(ref cached) = connection.xero_contact_id {
            return Ok(cached.clone());
        }

        // Serialize contact creation per connection to avoid duplicates
        let mutex = {
            self.contact_locks
                .entry(connection.id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _guard = mutex.lock().await;

        // Re-check DB after acquiring lock (another task may have created it)
        let fresh_conn = XeroConnections::find_by_id(connection.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Connection not found"))?;
        if let Some(ref cached) = fresh_conn.xero_contact_id {
            return Ok(cached.clone());
        }

        let contact_id = self
            .client
            .ensure_contact(access_token, tenant_id, "IronixPay Payments")
            .await?;

        // Cache in DB
        let mut active: xero_connections::ActiveModel = fresh_conn.into();
        active.xero_contact_id = Set(Some(contact_id.clone()));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(contact_id)
    }

    // ─── Sync Log Queries ───

    /// Get pending/failed sync logs ready for processing.
    pub async fn get_pending_sync_logs(&self, limit: u64) -> Result<Vec<xero_sync_logs::Model>> {
        let now = Utc::now();
        let logs = XeroSyncLogs::find()
            .inner_join(XeroConnections)
            .filter(xero_sync_logs::Column::Status.is_in([
                xero_sync_logs::XeroSyncStatus::Pending,
                xero_sync_logs::XeroSyncStatus::Failed,
            ]))
            .filter(
                xero_connections::Column::Status.eq(xero_connections::XeroConnectionStatus::Active),
            )
            .filter(xero_connections::Column::AutoSyncEnabled.eq(true))
            .filter(xero_connections::Column::XeroPaymentAccountCode.is_not_null())
            .filter(
                xero_sync_logs::Column::NextRetryAt
                    .is_null()
                    .or(xero_sync_logs::Column::NextRetryAt.lte(now)),
            )
            .filter(xero_sync_logs::Column::AttemptCount.lt(MAX_ATTEMPTS))
            .order_by_asc(xero_sync_logs::Column::NextRetryAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(logs)
    }

    /// Mark a sync log as failed with retry scheduling.
    pub async fn mark_sync_failed(
        &self,
        sync_log: &xero_sync_logs::Model,
        error_msg: &str,
    ) -> Result<()> {
        let current = XeroSyncLogs::find_by_id(sync_log.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Xero sync log {} not found", sync_log.id))?;

        let new_attempt = current.attempt_count + 1;
        let next_retry = if (new_attempt as usize) < RETRY_DELAYS_SECS.len() {
            Some(Utc::now() + ChronoDuration::seconds(RETRY_DELAYS_SECS[new_attempt as usize]))
        } else {
            None // Max retries reached
        };

        let mut active: xero_sync_logs::ActiveModel = current.into();
        active.status = Set(xero_sync_logs::XeroSyncStatus::Failed);
        active.attempt_count = Set(new_attempt);
        active.last_error = Set(Some(error_msg.to_string()));
        active.next_retry_at = Set(next_retry.map(|t| t.into()));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(())
    }

    /// Mark a sync log as skipped without consuming retries.
    pub async fn mark_sync_skipped(
        &self,
        sync_log: &xero_sync_logs::Model,
        reason: &str,
    ) -> Result<()> {
        let current = XeroSyncLogs::find_by_id(sync_log.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Xero sync log {} not found", sync_log.id))?;

        let mut active: xero_sync_logs::ActiveModel = current.into();
        active.status = Set(xero_sync_logs::XeroSyncStatus::Skipped);
        active.last_error = Set(Some(reason.to_string()));
        active.next_retry_at = Set(None);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;
        Ok(())
    }

    /// List sync logs for a connection (paginated).
    pub async fn list_sync_logs(
        &self,
        connection_id: Uuid,
        page: u64,
        per_page: u64,
        session_id: Option<&str>,
    ) -> Result<(Vec<xero_sync_logs::Model>, u64)> {
        let mut total_query =
            XeroSyncLogs::find().filter(xero_sync_logs::Column::ConnectionId.eq(connection_id));
        if let Some(sid) = session_id {
            total_query = total_query.filter(xero_sync_logs::Column::SessionId.eq(sid));
        }
        let total = total_query.count(&self.db).await?;

        let mut logs_query = XeroSyncLogs::find()
            .filter(xero_sync_logs::Column::ConnectionId.eq(connection_id))
            .order_by_desc(xero_sync_logs::Column::CreatedAt)
            .offset((page.saturating_sub(1)) * per_page)
            .limit(per_page);
        if let Some(sid) = session_id {
            logs_query = logs_query.filter(xero_sync_logs::Column::SessionId.eq(sid));
        }
        let logs = logs_query.all(&self.db).await?;

        Ok((logs, total))
    }

    /// Retry a specific failed sync log. Verifies it belongs to the given connection.
    pub async fn retry_sync_log(&self, sync_log_id: Uuid, connection_id: Uuid) -> Result<()> {
        let log = XeroSyncLogs::find_by_id(sync_log_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Sync log not found"))?;

        if log.connection_id != connection_id {
            return Err(anyhow!("Sync log does not belong to this connection"));
        }

        if log.status != xero_sync_logs::XeroSyncStatus::Failed {
            return Err(anyhow!("Only failed sync logs can be retried"));
        }

        let mut active: xero_sync_logs::ActiveModel = log.into();
        active.status = Set(xero_sync_logs::XeroSyncStatus::Pending);
        active.next_retry_at = Set(None);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(())
    }

    /// Postpone all pending sync logs for a connection (used on 429 rate limit).
    pub async fn postpone_connection_syncs(
        &self,
        connection_id: Uuid,
        retry_after_secs: i64,
    ) -> Result<()> {
        let retry_at = Utc::now() + ChronoDuration::seconds(retry_after_secs);

        // Use raw SQL for batch update
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"UPDATE xero_sync_logs SET next_retry_at = $1, updated_at = NOW()
                   WHERE connection_id = $2 AND status IN ('pending', 'failed')"#,
                [retry_at.into(), connection_id.into()],
            ))
            .await?;

        warn!(connection_id = %connection_id, retry_after = retry_after_secs, "Xero rate limited, postponing syncs");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::XeroService;

    #[test]
    fn normalize_account_code_rejects_placeholder_value() {
        let result =
            XeroService::normalize_account_code("xero_payment_account_code", "__bank_no_code__123");
        assert!(result.is_err());
    }

    #[test]
    fn normalize_account_code_accepts_valid_code() {
        let result = XeroService::normalize_account_code("xero_account_code", " 091_MAIN ");
        assert_eq!(result.unwrap(), "091_MAIN");
    }

    #[test]
    fn normalize_tax_type_defaults_to_none_on_blank() {
        let result = XeroService::normalize_tax_type("   ");
        assert_eq!(result.unwrap(), "NONE");
    }

    #[test]
    fn normalize_tax_type_rejects_invalid_chars() {
        let result = XeroService::normalize_tax_type("GST 9%");
        assert!(result.is_err());
    }
}
