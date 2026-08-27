//! Merchant Service Implementation
//!
//! Core business logic for merchant management.
//! Aligned with docs/system_design.md §1.1

use super::error::MerchantError;
use super::login_2fa_limiter::Login2faRateLimiter;
use super::registration_limiter::RegistrationRateLimiter;
use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use dashmap::DashMap;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, Statement,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use crate::entity::{
    api_keys, checkout_sessions, merchants, org_members, transactions, users, ApiKeys, Merchants,
    Transactions,
};

use super::types::{
    ApiKeyResponse, ApiKeySummary, AuthInfo, Claims, EmailVerificationClaims, LoginResponse,
    PasswordRequirements, PasswordResetClaims, RegisterRequest, StatsResult, TempAuthClaims,
};
use crate::services::email::EmailSender;
use secrecy::{ExposeSecret, Secret};
use std::net::IpAddr;
use std::sync::Arc;

/// Merchant Service
///
/// Manages merchant lifecycle: registration, authentication, API keys, and billing.
/// Security-critical operations require 2FA when enabled.
pub struct MerchantService {
    db: DatabaseConnection,
    jwt_secret: Secret<String>,
    jwt_expiry_hours: i64,
    password_requirements: PasswordRequirements,
    email_sender: Option<Arc<dyn EmailSender>>,
    address_manager: Option<Arc<crate::services::address::AddressManager>>,
    /// Networks enabled for this instance (e.g., [Tron] or [Tron, Bsc])
    enabled_networks: Vec<crate::entity::Network>,
    /// Runtime environment — determines JIT shadow account behavior
    environment: crate::config::Environment,
    /// Production database connection (Sandbox only).
    /// Used to sync TOTP fields from production users during JIT shadow creation.
    prod_db: Option<DatabaseConnection>,
    login_2fa_rate_limiter: Arc<Login2faRateLimiter>,
    registration_rate_limiter: Arc<RegistrationRateLimiter>,
    blocked_email_domains: HashSet<String>,
}

impl MerchantService {
    pub fn new(
        db: DatabaseConnection,
        jwt_secret: Secret<String>,
        jwt_expiry_hours: i64,
        environment: crate::config::Environment,
    ) -> Self {
        Self {
            db,
            jwt_secret,
            jwt_expiry_hours,
            password_requirements: PasswordRequirements::default(),
            email_sender: None,
            address_manager: None,
            enabled_networks: vec![crate::entity::Network::Tron], // Default to TRON
            environment,
            prod_db: None,
            login_2fa_rate_limiter: Login2faRateLimiter::new(),
            registration_rate_limiter: RegistrationRateLimiter::new(),
            blocked_email_domains: HashSet::from(["emalupe.com".to_string()]),
        }
    }

    /// Whether this instance is running in sandbox mode
    fn is_sandbox(&self) -> bool {
        matches!(self.environment, crate::config::Environment::Sandbox)
    }

    /// Set the email service for sending verification emails
    pub fn with_email_service(mut self, email_sender: Arc<dyn EmailSender>) -> Self {
        self.email_sender = Some(email_sender);
        self
    }

    /// Set the address manager for lazy initialization of crypto accounts
    pub fn with_address_manager(
        mut self,
        address_manager: Arc<crate::services::address::AddressManager>,
    ) -> Self {
        self.address_manager = Some(address_manager);
        self
    }

    /// Set the enabled networks for multi-chain support
    pub fn with_enabled_networks(mut self, networks: Vec<crate::entity::Network>) -> Self {
        self.enabled_networks = networks;
        self
    }

    /// Set the production database connection for TOTP sync (Sandbox only)
    pub fn with_production_db(mut self, prod_db: DatabaseConnection) -> Self {
        self.prod_db = Some(prod_db);
        self
    }

    /// Extend the disposable/abusive email domain denylist.
    pub fn with_blocked_email_domains(mut self, domains: Vec<String>) -> Self {
        self.blocked_email_domains.extend(
            domains
                .into_iter()
                .map(|domain| domain.trim().trim_start_matches('@').to_lowercase())
                .filter(|domain| !domain.is_empty()),
        );
        self
    }

    /// Record a public registration attempt before expensive verification work.
    pub fn check_registration_rate_limit(&self, client_ip: IpAddr) -> Result<(), MerchantError> {
        self.registration_rate_limiter
            .check_and_record(client_ip)
            .map_err(|dimension| {
                warn!(
                    client_ip = %client_ip,
                    dimension = dimension.as_str(),
                    "Registration rate limit exceeded"
                );
                MerchantError::RegistrationRateLimited
            })
    }

    // ============================================================
    // Registration & Authentication
    // ============================================================

    /// Register a new merchant
    ///
    /// Creates a new merchant account with:
    /// - Unique account_index for HD wallet derivation (Identity)
    /// - Argon2 password hashing
    /// - Initial status: PendingVerification
    /// - Default Profile for the current environment (the other is JIT-created on first access)
    pub async fn register(&self, req: RegisterRequest) -> Result<merchants::Model, MerchantError> {
        use super::types::InviteClaims;
        use sea_orm::TransactionTrait;

        // Validate password strength first
        self.validate_password(&req.password)?;

        // Normalize email (case-insensitive, trim whitespace)
        let email = req.email.trim().to_lowercase();

        if self.is_blocked_email(&email) {
            warn!(email_domain = ?email_domain(&email), "Blocked disposable email registration");
            return Err(MerchantError::DisposableEmailNotAllowed);
        }

        // Check if email already exists (in users table now)
        let existing = users::Entity::find()
            .filter(users::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(MerchantError::EmailAlreadyRegistered);
        }

        // Hash password using Argon2 in a blocking thread
        let password = req.password.clone();
        let password_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|h| h.to_string())
        })
        .await
        .map_err(|e| MerchantError::Internal(anyhow!("Task join error: {}", e)))?
        .map_err(|e| MerchantError::Internal(anyhow!("Failed to hash password: {}", e)))?;

        let user_id = format!("usr_{}", Uuid::new_v4().to_string().replace("-", ""));
        let now = Utc::now().fixed_offset();

        // Start Transaction
        let txn = self.db.begin().await?;

        // 1. Create user (login identity) — always needed
        let user = users::ActiveModel {
            id: Set(user_id.clone()),
            email: Set(email.clone()),
            password_hash: Set(password_hash),
            name: Set(req.name.clone()),
            totp_secret: Set(None),
            is_totp_enabled: Set(false),
            token_version: Set(0),
            backup_codes: Set(None),
            email_verified: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };
        user.insert(&txn).await?;

        // 2. Branch: invited registration vs self-registration
        let result = if let Some(invite_token) = &req.invite_token {
            // === Invited Registration: join existing org, do NOT create merchant ===
            let invite_claims = decode::<InviteClaims>(
                invite_token,
                &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
                &Validation::default(),
            )
            .map_err(|e| {
                MerchantError::Internal(anyhow!("Invalid or expired invite token: {}", e))
            })?;

            let claims = invite_claims.claims;

            // Validate purpose
            if claims.purpose != "team_invite" {
                return Err(MerchantError::Internal(anyhow!(
                    "Token is not a team invitation"
                )));
            }

            // Verify email matches the invitation
            if claims.email != email {
                return Err(MerchantError::Internal(anyhow!(
                    "Register email does not match invitation email"
                )));
            }

            let invite_id = &claims.sub; // org_member row ID
            let org_id = &claims.org_id;

            // Find the pending invitation
            let invitation = org_members::Entity::find_by_id(invite_id)
                .filter(org_members::Column::Status.eq(org_members::MemberStatus::Pending))
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    MerchantError::Internal(anyhow!("Invitation not found or already accepted"))
                })?;

            // Accept the invitation: set user_id + status=active
            let mut active: org_members::ActiveModel = invitation.into();
            active.user_id = Set(Some(user_id.clone()));
            active.status = Set(org_members::MemberStatus::Active);
            active.accepted_at = Set(Some(now));
            active.updated_at = Set(now);
            active.update(&txn).await?;

            // Auto-verify email for invited users (they were invited by a trusted org member)
            let user_active = users::ActiveModel {
                id: Set(user_id.clone()),
                email_verified: Set(true),
                updated_at: Set(now),
                ..Default::default()
            };
            user_active.update(&txn).await?;

            info!(user_id = %user_id, org_id = %org_id, "User registered via invitation, joined existing org");

            // Return the merchant (org) they joined
            Merchants::find_by_id(org_id)
                .one(&txn)
                .await?
                .ok_or_else(|| MerchantError::Internal(anyhow!("Org not found")))?
        } else {
            // === Self-Registration: create new merchant org ===
            let merchant_id = format!("mer_{}", Uuid::new_v4().to_string().replace("-", ""));
            let member_id = format!("om_{}", Uuid::new_v4());

            // Look up agent by referral code (if provided)
            let (agent_id, custom_fee_pct) = if let Some(ref code) = req.referral_code {
                let code = code.trim().to_uppercase();
                if !code.is_empty() {
                    // Direct query — AgentService may not be injected into MerchantService
                    use crate::entity::{agent_profiles, AgentProfiles};
                    let agent = AgentProfiles::find()
                        .filter(agent_profiles::Column::ReferralCode.eq(&code))
                        .filter(agent_profiles::Column::Status.eq("active"))
                        .one(&txn)
                        .await?;
                    match agent {
                        Some(a) => {
                            info!(
                                referral_code = %code,
                                agent_id = %a.id,
                                default_merchant_rate = %a.default_merchant_rate,
                                "Referral code matched — setting custom fee for new merchant"
                            );
                            (Some(a.id), Some(a.default_merchant_rate))
                        }
                        None => {
                            warn!(referral_code = %code, "Invalid referral code, ignoring");
                            (None, None)
                        }
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let merchant = merchants::ActiveModel {
                id: Set(merchant_id.clone()),
                name: Set(req.name.clone()),
                status: Set(merchants::MerchantStatus::PendingVerification),
                merchant_type: Set(merchants::MerchantType::Direct),
                custom_fee_percentage: Set(custom_fee_pct),
                fee_tier: Set(merchants::FeeTier::Enterprise),
                fee_source: Set(if agent_id.is_some() {
                    merchants::FeeSource::Agent
                } else {
                    merchants::FeeSource::AutoTier
                }),
                first_month_ends_at: Set(Some((now + chrono::Duration::days(30)).into())),
                referred_by_agent_id: Set(agent_id),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            let merchant_result = merchant.insert(&txn).await?;

            // Create owner membership
            let member = org_members::ActiveModel {
                id: Set(member_id),
                org_id: Set(merchant_id.clone()),
                user_id: Set(Some(user_id.clone())),
                invited_email: Set(None),
                role: Set(org_members::MemberRole::Owner),
                invited_by: Set(None),
                invited_at: Set(Some(now)),
                accepted_at: Set(Some(now)),
                status: Set(org_members::MemberStatus::Active),
                created_at: Set(now),
                updated_at: Set(now),
            };
            member.insert(&txn).await?;

            info!(merchant_id = %merchant_id, user_id = %user_id, account_index = merchant_result.account_index, "Self-registration: new merchant created");
            merchant_result
        };

        // Commit
        txn.commit().await?;

        // Send verification email only for self-registration (invited users are auto-verified)
        if req.invite_token.is_none() {
            if let Some(email_sender) = &self.email_sender {
                let token = self.generate_email_verification_token(&user_id)?;
                let email_clone = email.clone();
                let name = result.name.clone();
                let merchant_id_for_log = result.id.clone();
                let email_sender = email_sender.clone();

                tokio::spawn(async move {
                    if let Err(e) = email_sender
                        .send_verification_email(&email_clone, &name, &token)
                        .await
                    {
                        tracing::error!(error = %e, email = %email_clone, "Failed to send verification email");
                    } else {
                        tracing::info!(merchant_id = %merchant_id_for_log, "Verification email sent");
                    }
                });
            } else {
                tracing::error!(
                    merchant_id = %result.id,
                    "EMAIL SERVICE NOT CONFIGURED! Verification email SKIPPED."
                );
                #[cfg(debug_assertions)]
                {
                    let token = self.generate_email_verification_token(&user_id)?;
                    tracing::warn!(
                        merchant_id = %result.id,
                        token = %token,
                        "[DEV MODE] Use this token to verify: /verify-email?token={}", token
                    );
                }
            }
        }

        Ok(result)
    }

    fn is_blocked_email(&self, email: &str) -> bool {
        is_blocked_email_domain(email, &self.blocked_email_domains)
    }

    /// Resend verification email
    ///
    /// Security: Always returns Ok(()) to prevent user enumeration attacks.
    /// Rate limited: max 3 per hour per email (in-memory, per-instance).
    pub async fn resend_verification_email(&self, email: &str) -> Result<()> {
        let email = email.trim().to_lowercase();

        static RESEND_RATE_LIMIT: LazyLock<DashMap<String, Vec<Instant>>> =
            LazyLock::new(DashMap::new);

        const MAX_RESENDS: usize = 3;
        const WINDOW: std::time::Duration = std::time::Duration::from_secs(3600);

        {
            let mut entry = RESEND_RATE_LIMIT.entry(email.clone()).or_default();
            let now = Instant::now();
            entry.retain(|t| now.duration_since(*t) < WINDOW);
            if entry.len() >= MAX_RESENDS {
                tracing::warn!(email = %email, "Resend verification rate limited (3/hour)");
                return Ok(());
            }
            entry.push(now);
        }

        // Query users table instead of merchants
        let user_opt = users::Entity::find()
            .filter(users::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        let user = match user_opt {
            Some(u) => u,
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                tracing::debug!(email = %email, "Resend verification: email not found (hidden from user)");
                return Ok(());
            }
        };

        if user.email_verified {
            tracing::debug!(user_id = %user.id, "Resend verification: already verified (hidden from user)");
            return Ok(());
        }

        let Some(email_sender) = &self.email_sender else {
            tracing::error!(user_id = %user.id, "Email service not configured! Cannot resend verification.");
            return Ok(());
        };

        let token = self.generate_email_verification_token(&user.id)?;

        if let Err(e) = email_sender
            .send_verification_email(&user.email, &user.name, &token)
            .await
        {
            tracing::error!(user_id = %user.id, error = %e, "Failed to send verification email");
        } else {
            info!(user_id = %user.id, "Verification email resent");
        }

        Ok(())
    }

    /// Verify email and activate account
    ///
    /// Validates the JWT token and activates the merchant account.
    /// Idempotent: returns Ok with merchant_id if already verified.
    ///
    /// Returns the merchant_id for downstream processing (e.g., address initialization).
    pub async fn verify_email(&self, token: &str) -> Result<String, MerchantError> {
        let token_data = decode::<EmailVerificationClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| MerchantError::InvalidToken)?;

        if token_data.claims.purpose != "email_verification" {
            return Err(MerchantError::InvalidToken);
        }

        // sub is now user_id
        let user_id = token_data.claims.sub;

        // Find user
        let user = users::Entity::find_by_id(&user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if user.email_verified {
            // Find org membership to return org_id for address initialization
            let membership = org_members::Entity::find()
                .filter(org_members::Column::UserId.eq(&user_id))
                .one(&self.db)
                .await?
                .ok_or(MerchantError::Internal(anyhow!("No org membership")))?;
            return Ok(membership.org_id);
        }

        // Update user email_verified
        let mut active: users::ActiveModel = user.into();
        active.email_verified = Set(true);
        active.update(&self.db).await?;

        // Find org membership to activate merchant and return org_id
        let membership = org_members::Entity::find()
            .filter(org_members::Column::UserId.eq(&user_id))
            .one(&self.db)
            .await?
            .ok_or(MerchantError::Internal(anyhow!("No org membership")))?;

        // Activate merchant org if PendingVerification
        let merchant = Merchants::find_by_id(&membership.org_id)
            .one(&self.db)
            .await?;
        if let Some(m) = merchant {
            if m.status == merchants::MerchantStatus::PendingVerification {
                let mut active: merchants::ActiveModel = m.into();
                active.status = Set(merchants::MerchantStatus::Active);
                active.update(&self.db).await?;
            }
        }

        info!(user_id = %user_id, org_id = %membership.org_id, "Email verified, account activated");
        Ok(membership.org_id)
    }

    /// Generate email verification token (JWT, 24h expiry)
    fn generate_email_verification_token(&self, merchant_id: &str) -> Result<String> {
        let claims = EmailVerificationClaims {
            sub: merchant_id.to_string(),
            exp: (Utc::now() + Duration::hours(24)).timestamp(),
            purpose: "email_verification".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
        )
        .map_err(|e| anyhow!("Failed to generate verification token: {}", e))
    }

    /// Generate a team invitation JWT token (24h expiry).
    ///
    /// Contains `purpose: "team_invite"` to prevent confusion with other token types.
    pub fn generate_invite_token(
        &self,
        member_id: &str,
        org_id: &str,
        email: &str,
    ) -> Result<String> {
        use super::types::InviteClaims;

        let now = Utc::now();
        let claims = InviteClaims {
            sub: member_id.to_string(),
            org_id: org_id.to_string(),
            email: email.to_string(),
            purpose: "team_invite".to_string(),
            exp: (now + Duration::hours(24)).timestamp(),
            iat: now.timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
        )
        .map_err(|e| anyhow!("Failed to generate invite token: {}", e))
    }

    /// Expose the email sender for use by team routes (invitation emails).
    pub fn get_email_sender(&self) -> &Option<Arc<dyn crate::services::email::EmailSender>> {
        &self.email_sender
    }

    /// Verify and decode a team invitation JWT token.
    ///
    /// Returns the `InviteClaims` if the token is valid and its purpose is `"team_invite"`.
    pub fn verify_invite_token(&self, token: &str) -> Result<super::types::InviteClaims> {
        let token_data = decode::<super::types::InviteClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| anyhow!("Invalid invite token: {}", e))?;

        if token_data.claims.purpose != "team_invite" {
            return Err(anyhow!("Token is not a team invitation"));
        }

        Ok(token_data.claims)
    }

    /// Login - Step 1: Verify password
    ///
    /// Returns either:
    /// - `LoginResponse::Success` with JWT if 2FA is NOT enabled
    /// - `LoginResponse::Requires2FA` with temp_token if 2FA IS enabled
    ///
    /// Security considerations:
    /// - Argon2 verification runs in blocking thread to not block async runtime
    /// - Timing attack prevention: even if user not found, we still perform
    ///   a dummy hash verification to maintain consistent response times
    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResponse, MerchantError> {
        // Normalize email (case-insensitive, trim whitespace)
        let email = email.trim().to_lowercase();

        // Find user by email (was: merchants table)
        let user_opt = users::Entity::find()
            .filter(users::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        // Verify password in blocking thread (Argon2 is CPU-intensive)
        let password = password.to_string();
        let (is_valid, user) = match user_opt {
            Some(u) => {
                let hash = u.password_hash.clone();
                let valid = tokio::task::spawn_blocking(move || {
                    PasswordHash::new(&hash)
                        .ok()
                        .and_then(|parsed| {
                            Argon2::default()
                                .verify_password(password.as_bytes(), &parsed)
                                .ok()
                        })
                        .is_some()
                })
                .await
                .unwrap_or(false);
                (valid, Some(u))
            }
            None => {
                // Dummy verification to prevent timing-based user enumeration
                let dummy_hash =
                    "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc";
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(parsed) = PasswordHash::new(dummy_hash) {
                        let _ = Argon2::default().verify_password(password.as_bytes(), &parsed);
                    }
                })
                .await;
                (false, None)
            }
        };

        // Return generic error for both invalid user and invalid password
        if !is_valid {
            return Err(MerchantError::InvalidCredentials);
        }

        let user = user.unwrap(); // Safe: is_valid implies user exists

        // Require email verification before login
        if !user.email_verified {
            return Err(MerchantError::EmailNotVerified);
        }

        // Find user's org membership (active only, most recently joined first)
        let membership = org_members::Entity::find()
            .filter(org_members::Column::UserId.eq(&user.id))
            .filter(org_members::Column::Status.eq("active"))
            .order_by_desc(org_members::Column::CreatedAt)
            .one(&self.db)
            .await?
            .ok_or(MerchantError::Internal(anyhow!(
                "No active organization membership"
            )))?;

        // Find the org (merchant)
        let merchant = Merchants::find_by_id(&membership.org_id)
            .one(&self.db)
            .await?
            .ok_or(MerchantError::Internal(anyhow!("Organization not found")))?;

        // Check account status
        if merchant.status == merchants::MerchantStatus::Suspended {
            return Err(MerchantError::AccountSuspended);
        }

        let role_str = serde_json::to_value(&membership.role)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "owner".to_string());

        // Check if 2FA is enabled
        if user.is_totp_enabled {
            // Issue a short-lived temp token for 2FA verification
            let now = Utc::now();
            let exp = now + Duration::minutes(5); // 5-minute window for 2FA

            let temp_claims = TempAuthClaims {
                sub: user.id.clone(),
                exp: exp.timestamp(),
                purpose: "2fa_pending".to_string(),
                org_id: Some(membership.org_id.clone()),
                org_role: Some(role_str),
            };

            let temp_token = encode(
                &Header::default(),
                &temp_claims,
                &EncodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            )
            .map_err(|e| MerchantError::Internal(e.into()))?;

            info!(user_id = %user.id, org_id = %membership.org_id, "2FA required for login");

            return Ok(LoginResponse::Requires2FA {
                temp_token,
                merchant_id: membership.org_id,
            });
        }

        // No 2FA - issue full JWT token
        self.issue_jwt_token(&user, &membership.org_id, &merchant.name, &role_str)
    }

    /// Login - Step 2: Verify TOTP code (only if 2FA is enabled)
    ///
    /// Takes the temp_token from Step 1 and the TOTP code from the authenticator app.
    /// Returns the full JWT token on success.
    pub async fn verify_totp_login(
        &self,
        temp_token: &str,
        totp_code: &str,
        client_ip: IpAddr,
    ) -> Result<LoginResponse, MerchantError> {
        // Decode first so valid tokens can also be limited by user. Invalid tokens
        // are still counted against their token fingerprint and source IP.
        let token_data = decode::<TempAuthClaims>(
            temp_token,
            &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            &Validation::default(),
        );

        let limited_user_id = token_data
            .as_ref()
            .ok()
            .filter(|data| data.claims.purpose == "2fa_pending")
            .map(|data| data.claims.sub.as_str());
        let attempt = self
            .login_2fa_rate_limiter
            .begin(limited_user_id, temp_token, client_ip)
            .map_err(|dimension| {
                warn!(
                    dimension = dimension.as_str(),
                    client_ip = %client_ip,
                    "Login 2FA rate limit exceeded"
                );
                MerchantError::RateLimited
            })?;

        let token_data = match token_data {
            Ok(token_data) => token_data,
            Err(_) => {
                attempt.failure();
                return Err(MerchantError::InvalidToken);
            }
        };

        // Verify it's a 2FA pending token
        if token_data.claims.purpose != "2fa_pending" {
            attempt.failure();
            return Err(MerchantError::InvalidToken);
        }

        let user_id = &token_data.claims.sub;

        // Get user and verify TOTP
        let user = match users::Entity::find_by_id(user_id).one(&self.db).await? {
            Some(user) => user,
            None => {
                attempt.failure();
                return Err(MerchantError::InvalidToken);
            }
        };

        // Verify the TOTP code or backup code
        let is_valid = verify_2fa_code_for_user(&self.db, &user, totp_code).await?;

        if !is_valid {
            attempt.failure();
            return Err(MerchantError::Invalid2FACode);
        }

        attempt.success();

        // Get org context from temp claims or look up membership
        let (org_id, role_str) = match (token_data.claims.org_id, token_data.claims.org_role) {
            (Some(oid), Some(role)) => (oid, role),
            _ => {
                // Fallback: look up from org_members (backward compat for old temp tokens)
                let membership = org_members::Entity::find()
                    .filter(org_members::Column::UserId.eq(user_id))
                    .filter(org_members::Column::Status.eq("active"))
                    .one(&self.db)
                    .await?
                    .ok_or(MerchantError::Internal(anyhow!("No active org membership")))?;
                let role = serde_json::to_value(&membership.role)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "owner".to_string());
                (membership.org_id, role)
            }
        };

        let merchant = Merchants::find_by_id(&org_id)
            .one(&self.db)
            .await?
            .ok_or(MerchantError::Internal(anyhow!("Organization not found")))?;

        info!(user_id = %user.id, org_id = %org_id, "2FA verification successful");

        // Issue full JWT token
        self.issue_jwt_token(&user, &org_id, &merchant.name, &role_str)
    }

    /// Verify TOTP code for sensitive actions (non-login)
    ///
    /// # Arguments
    /// * `merchant_id` - The authenticated merchant ID
    /// * `totp_code` - The 6-digit TOTP code or backup code
    ///
    /// Returns Ok(()) if valid, Err if invalid.
    ///
    /// # Security (HIGH-3 Fix)
    /// Rate limited: max 5 failed attempts per 5 minutes per merchant.
    /// This prevents brute-force attacks on TOTP codes.
    pub async fn verify_totp_action(
        &self,
        merchant_id: &str,
        totp_code: &str,
    ) -> Result<(), MerchantError> {
        // HIGH-3 FIX: Rate limiting for TOTP brute-force prevention
        if is_totp_rate_limited(merchant_id) {
            warn!(merchant_id = %merchant_id, "TOTP rate limit exceeded");
            return Err(MerchantError::RateLimited);
        }

        // For verify_totp_action, merchant_id is actually org_id from AuthenticatedMerchant.
        // We need to find the user. For now, look up via org_members.
        // TODO: Phase 3 — pass user_id directly from AuthenticatedMerchant
        let membership = org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(merchant_id))
            .filter(org_members::Column::Status.eq("active"))
            .one(&self.db)
            .await?
            .ok_or(MerchantError::NotFound("Membership not found".into()))?;

        let user_id = membership.user_id.ok_or(MerchantError::NotFound(
            "User not found in membership".into(),
        ))?;

        let user = users::Entity::find_by_id(&user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if !user.is_totp_enabled {
            return Ok(());
        }

        let is_valid = verify_2fa_code_for_user(&self.db, &user, totp_code).await?;
        if !is_valid {
            record_totp_failure(merchant_id);
            return Err(MerchantError::Invalid2FACode);
        }

        clear_totp_failures(merchant_id);
        Ok(())
    }

    /// Issue a full JWT token for authenticated user
    pub(crate) fn issue_jwt_token(
        &self,
        user: &users::Model,
        org_id: &str,
        org_name: &str,
        role: &str,
    ) -> Result<LoginResponse, MerchantError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expiry_hours);

        let claims = Claims {
            sub: org_id.to_string(),
            uid: Some(user.id.clone()),
            role: Some(role.to_string()),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            tv: user.token_version,
            name: Some(user.name.clone()),
            email: Some(user.email.clone()),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
        )
        .map_err(|e| MerchantError::Internal(e.into()))?;

        info!(user_id = %user.id, org_id = %org_id, role = %role, "JWT token issued");

        Ok(LoginResponse::Success {
            token,
            merchant_id: org_id.to_string(),
            user_id: user.id.clone(),
            role: role.to_string(),
            org_name: org_name.to_string(),
            expires_at: exp.timestamp(),
        })
    }

    /// Verify JWT token and return AuthInfo { org_id, user_id, role }
    ///
    /// Checks:
    /// 1. JWT signature and expiration (handled by jsonwebtoken)
    /// 2. Backward compat: old JWTs without uid/role → fallback to sub/owner
    /// 3. Token version matches users.token_version (production only)
    /// 4. Merchant org must exist and not be suspended
    ///
    /// Sandbox behavior:
    /// - Skips token_version check (prod-signed JWT is trusted)
    /// - JIT creates a shadow merchant if not found in local DB
    pub async fn verify_token(&self, token: &str) -> Result<AuthInfo> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            &Validation::default(),
        )?;

        let claims = token_data.claims;

        // Backward compat: old JWTs don't have uid/role
        let user_id = claims.uid.unwrap_or_else(|| claims.sub.clone());
        let role = claims.role.unwrap_or_else(|| "owner".to_string());

        // Find merchant (org) in local DB, with JIT fallback for sandbox
        let merchant = match Merchants::find_by_id(&claims.sub).one(&self.db).await? {
            Some(m) => m,
            None if self.is_sandbox() => {
                info!(merchant_id = %claims.sub, user_id = %user_id, "JIT: Creating shadow merchant in sandbox");
                self.ensure_merchant_shadow(
                    &claims.sub,
                    &user_id,
                    &role,
                    claims.name.as_deref(),
                    claims.email.as_deref(),
                )
                .await?
            }
            None => return Err(anyhow!("Merchant not found")),
        };

        // Token version check against users table (skip in sandbox)
        if !self.is_sandbox() {
            let user = users::Entity::find_by_id(&user_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| anyhow!("User not found"))?;

            if claims.tv != user.token_version {
                return Err(anyhow!("Token has been revoked"));
            }
        } else {
            // Sandbox: ensure user + org_member exist (team member JIT)
            // Also syncs name/email from production JWT on every access.
            self.ensure_user_shadow(
                &user_id,
                &claims.sub,
                &role,
                claims.name.as_deref(),
                claims.email.as_deref(),
            )
            .await;

            // Sandbox: sync org-level fields (name, logo_url) from production
            // This keeps shadow merchant branding in sync with production changes.
            if let Some(ref prod_db) = self.prod_db {
                if let Ok(Some(prod_merchant)) =
                    Merchants::find_by_id(&claims.sub).one(prod_db).await
                {
                    let needs_update = merchant.name != prod_merchant.name
                        || merchant.logo_url != prod_merchant.logo_url;
                    if needs_update {
                        let mut active: merchants::ActiveModel = merchant.clone().into();
                        active.name = Set(prod_merchant.name);
                        active.logo_url = Set(prod_merchant.logo_url);
                        active.updated_at = Set(Utc::now().fixed_offset());
                        if let Err(e) = active.update(&self.db).await {
                            tracing::warn!(
                                merchant_id = %claims.sub,
                                error = %e,
                                "Failed to sync org fields from production"
                            );
                        }
                    }
                }
            }
        }

        // Verify account is not suspended
        if merchant.status == merchants::MerchantStatus::Suspended {
            return Err(anyhow!("Account suspended"));
        }

        Ok(AuthInfo {
            org_id: claims.sub,
            user_id,
            role,
        })
    }

    /// JIT: Create a shadow merchant record in the sandbox database.
    ///
    /// Called when a valid JWT references a merchant_id that doesn't exist locally.
    /// Uses `ON CONFLICT DO NOTHING` to handle concurrent requests safely
    /// (e.g., Dashboard fires 5 parallel API calls on first sandbox switch).
    ///
    /// Creates:
    /// - `merchants` row (minimal: no password, no login capability)
    /// - `merchant_chain_accounts` + pre-generated address pool (async, non-blocking)
    async fn ensure_merchant_shadow(
        &self,
        merchant_id: &str,
        user_id: &str,
        role: &str,
        jwt_name: Option<&str>,
        jwt_email: Option<&str>,
    ) -> Result<merchants::Model> {
        use sea_orm::sea_query::OnConflict;

        let now = Utc::now().fixed_offset();
        let entity_env = self.environment.to_entity_environment();

        // 1. Upsert merchant identity (ON CONFLICT UPDATE name to sync from production)
        // Shadow merchant: only org-level fields (no auth fields — those are on users table)

        // Sync org name + logo_url from production merchant (single query)
        let (display_name, prod_logo_url) = if let Some(ref prod_db) = self.prod_db {
            match Merchants::find_by_id(merchant_id).one(prod_db).await {
                Ok(Some(prod_merchant)) => (prod_merchant.name, prod_merchant.logo_url),
                _ => (jwt_name.unwrap_or("Shadow Account").to_string(), None),
            }
        } else {
            (jwt_name.unwrap_or("Shadow Account").to_string(), None)
        };

        let shadow = merchants::ActiveModel {
            id: Set(merchant_id.to_string()),
            name: Set(display_name.to_string()),
            logo_url: Set(prod_logo_url),
            status: Set(merchants::MerchantStatus::Active),
            merchant_type: Set(merchants::MerchantType::Direct),
            // account_index: let PostgreSQL SEQUENCE assign it
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        merchants::Entity::insert(shadow)
            .on_conflict(
                OnConflict::column(merchants::Column::Id)
                    .update_columns([
                        merchants::Column::Name,
                        merchants::Column::LogoUrl,
                        merchants::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .ok();

        // 2. Upsert shadow user + org_member
        self.ensure_user_shadow(user_id, merchant_id, role, jwt_name, jwt_email)
            .await;

        // 3. Initialize chain account + address pool (fire-and-forget)
        //    This ensures sandbox is fully functional from first access.
        //    Uses ON CONFLICT DO NOTHING internally — safe for concurrent JIT calls.
        if let Some(am) = &self.address_manager {
            let am = Arc::clone(am);
            let merchant_id_owned = merchant_id.to_string();
            let env = entity_env;
            let networks = self.enabled_networks.clone();
            tokio::spawn(async move {
                for network in networks {
                    match am
                        .initialize_merchant_addresses(
                            &merchant_id_owned,
                            network.clone(),
                            env.clone(),
                        )
                        .await
                    {
                        Ok(result) => {
                            info!(
                                merchant_id = %merchant_id_owned,
                                network = ?network,
                                addresses_created = result.addresses_created,
                                "JIT: Chain account + address pool initialized"
                            );
                        }
                        Err(e) => {
                            warn!(
                                merchant_id = %merchant_id_owned,
                                network = ?network,
                                error = %e,
                                "JIT: Failed to initialize chain account (will retry on next access)"
                            );
                        }
                    }
                }
            });
        }

        // 4. Fetch and return the merchant (guaranteed to exist now)
        Merchants::find_by_id(merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Failed to create shadow merchant"))
    }

    /// JIT: Upsert a shadow user + org_member in the sandbox database.
    ///
    /// Called in two scenarios:
    /// 1. From `ensure_merchant_shadow` during full JIT creation (new merchant)
    /// 2. From `verify_token` when merchant exists but the current user doesn't
    ///    (e.g., a team member accessing sandbox for the first time)
    ///
    /// Uses `ON CONFLICT DO UPDATE` for user name/email to keep sandbox
    /// in sync when production data changes (e.g., user renamed).
    /// Uses `ON CONFLICT DO NOTHING` for org_member role (role shouldn't auto-change).
    async fn ensure_user_shadow(
        &self,
        user_id: &str,
        org_id: &str,
        role: &str,
        jwt_name: Option<&str>,
        jwt_email: Option<&str>,
    ) {
        use sea_orm::sea_query::OnConflict;

        let now = Utc::now().fixed_offset();
        let shadow_email = jwt_email
            .map(|e| e.to_string())
            .unwrap_or_else(|| format!("{}@shadow.local", user_id));
        let shadow_user_name = jwt_name.unwrap_or("Shadow User");

        // Sync TOTP fields from production database (if available)
        let (totp_enabled, totp_secret, backup_codes) = if let Some(ref prod_db) = self.prod_db {
            match users::Entity::find_by_id(user_id).one(prod_db).await {
                Ok(Some(prod_user)) => {
                    tracing::debug!(
                        user_id = %user_id,
                        totp_enabled = prod_user.is_totp_enabled,
                        "JIT: Syncing TOTP fields from production user"
                    );
                    (
                        prod_user.is_totp_enabled,
                        prod_user.totp_secret,
                        prod_user.backup_codes,
                    )
                }
                Ok(None) => {
                    tracing::debug!(user_id = %user_id, "JIT: Production user not found, using defaults");
                    (false, None, None)
                }
                Err(e) => {
                    tracing::warn!(user_id = %user_id, error = %e, "JIT: Failed to query production DB for TOTP sync");
                    (false, None, None)
                }
            }
        } else {
            (false, None, None)
        };

        let shadow_user = users::ActiveModel {
            id: Set(user_id.to_string()),
            email: Set(shadow_email),
            password_hash: Set("shadow-no-login".to_string()),
            name: Set(shadow_user_name.to_string()),
            is_totp_enabled: Set(totp_enabled),
            totp_secret: Set(totp_secret),
            backup_codes: Set(backup_codes),
            token_version: Set(0),
            email_verified: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        users::Entity::insert(shadow_user)
            .on_conflict(
                OnConflict::column(users::Column::Id)
                    .update_columns([
                        users::Column::Name,
                        users::Column::Email,
                        users::Column::IsTotpEnabled,
                        users::Column::TotpSecret,
                        users::Column::BackupCodes,
                        users::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .ok();

        // Upsert org_member (links user → org with role)
        // DO NOTHING: role shouldn't auto-change if member already exists
        let member_id = format!("om_{}", uuid::Uuid::new_v4());
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"INSERT INTO org_members (id, org_id, user_id, role, status, accepted_at, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, 'active', NOW(), NOW(), NOW())
                   ON CONFLICT (org_id, user_id) DO NOTHING"#,
                [member_id.into(), org_id.into(), user_id.into(), role.into()],
            ))
            .await
            .ok();
    }

    /// Invalidate all existing tokens for a merchant
    ///
    /// Increments the token_version, which immediately invalidates all
    /// previously issued JWTs. Use this for:
    /// - Logout from all devices
    /// - Password change
    /// - Account compromise
    /// - Admin action
    pub async fn invalidate_all_tokens(&self, user_id: &str) -> Result<()> {
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                UPDATE users
                SET token_version = token_version + 1, updated_at = NOW()
                WHERE id = $1
                "#,
                [user_id.into()],
            ))
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("User not found"));
        }

        info!(user_id = %user_id, "All tokens invalidated");
        Ok(())
    }

    // ============================================================
    // Password Management
    // ============================================================

    /// Change password for authenticated user
    ///
    /// Security requirements:
    /// - Must verify old password
    /// - New password must meet strength requirements
    /// - If 2FA is enabled, TOTP code is required
    /// - All existing tokens are invalidated after password change
    ///
    /// # Arguments
    /// * `user_id` - The user changing their password
    /// * `old_password` - Current password for verification
    /// * `new_password` - New password (will be validated)
    /// * `totp_code` - Required if 2FA is enabled
    pub async fn change_password(
        &self,
        user_id: &str,
        old_password: &str,
        new_password: &str,
        totp_code: Option<&str>,
    ) -> Result<(), MerchantError> {
        // Directly look up user by ID (post Role & Org refactor)
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        // Verify old password
        let old_password = old_password.to_string();
        let hash = user.password_hash.clone();
        let is_valid = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&hash)
                .ok()
                .and_then(|parsed| {
                    Argon2::default()
                        .verify_password(old_password.as_bytes(), &parsed)
                        .ok()
                })
                .is_some()
        })
        .await
        .unwrap_or(false);

        if !is_valid {
            return Err(MerchantError::WrongPassword);
        }

        // If 2FA is enabled, verify TOTP
        if user.is_totp_enabled {
            let code = totp_code.ok_or(MerchantError::TwoFARequired)?;
            let is_valid = verify_2fa_code_for_user(&self.db, &user, code).await?;
            if !is_valid {
                return Err(MerchantError::Invalid2FACode);
            }
            info!(user_id = %user_id, "2FA verified for password change");
        }

        self.validate_password(new_password)?;

        let new_password = new_password.to_string();
        let new_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(new_password.as_bytes(), &salt)
                .map(|h| h.to_string())
        })
        .await
        .map_err(|e| MerchantError::Internal(anyhow!("Task join error: {}", e)))?
        .map_err(|e| MerchantError::Internal(anyhow!("Failed to hash password: {}", e)))?;

        // Update password in users table
        let mut active: users::ActiveModel = user.into();
        active.password_hash = Set(new_hash);
        active.update(&self.db).await?;

        self.invalidate_all_tokens(&user_id).await?;

        info!(user_id = %user_id, "Password changed successfully");
        Ok(())
    }

    /// Send password reset email
    ///
    /// Security: Always returns Ok(()) to prevent user enumeration attacks.
    /// Rate limited: max 3 per hour per email (implement at API layer)
    ///
    /// # Arguments
    /// * `email` - Email address to send reset link to
    pub async fn send_password_reset_email(&self, email: &str) -> Result<()> {
        let email = email.trim().to_lowercase();

        let user_opt = users::Entity::find()
            .filter(users::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        let user = match user_opt {
            Some(u) => u,
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                tracing::debug!(email = %email, "Password reset: email not found (hidden from user)");
                return Ok(());
            }
        };

        // Find org to check suspension
        let membership = org_members::Entity::find()
            .filter(org_members::Column::UserId.eq(&user.id))
            .filter(org_members::Column::Status.eq("active"))
            .one(&self.db)
            .await?;

        if let Some(m) = &membership {
            let merchant = Merchants::find_by_id(&m.org_id).one(&self.db).await?;
            if let Some(merchant) = merchant {
                if merchant.status == merchants::MerchantStatus::Suspended {
                    tracing::debug!(user_id = %user.id, "Password reset: account suspended (hidden from user)");
                    return Ok(());
                }
            }
        }

        let Some(email_sender) = &self.email_sender else {
            tracing::error!(user_id = %user.id, "Email service not configured! Cannot send password reset.");
            return Ok(());
        };

        let token = self.generate_password_reset_token(&user.id, user.token_version)?;

        if let Err(e) = email_sender
            .send_password_reset_email(&user.email, &user.name, &token)
            .await
        {
            tracing::error!(user_id = %user.id, error = %e, "Failed to send password reset email");
        } else {
            info!(user_id = %user.id, "Password reset email sent");
        }

        Ok(())
    }

    /// Reset password using a reset token from email
    ///
    /// Validates the JWT token and sets a new password.
    /// Token can only be used once (validated against token_version).
    ///
    /// # Arguments
    /// * `token` - Reset token from email
    /// * `new_password` - New password (will be validated)
    pub async fn reset_password_with_token(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<(), MerchantError> {
        let token_data = decode::<PasswordResetClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| MerchantError::InvalidToken)?;

        if token_data.claims.purpose != "password_reset" {
            return Err(MerchantError::InvalidToken);
        }

        // sub is now user_id
        let user_id = &token_data.claims.sub;

        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if token_data.claims.tv != user.token_version {
            return Err(MerchantError::TokenAlreadyUsed);
        }

        self.validate_password(new_password)?;

        let new_password = new_password.to_string();
        let new_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(new_password.as_bytes(), &salt)
                .map(|h| h.to_string())
        })
        .await
        .map_err(|e| MerchantError::Internal(anyhow!("Task join error: {}", e)))?
        .map_err(|e| MerchantError::Internal(anyhow!("Failed to hash password: {}", e)))?;

        let mut active: users::ActiveModel = user.into();
        active.password_hash = Set(new_hash);
        active.update(&self.db).await?;

        self.invalidate_all_tokens(user_id).await?;

        info!(user_id = %user_id, "Password reset successfully");
        Ok(())
    }

    // ============================================================
    // Statistics
    // ============================================================

    /// Get merchant statistics for dashboard
    ///
    /// Returns:
    /// - Total Volume (USDT)
    /// - Today's Volume (USDT) - UTC based
    /// - Total Transactions Count
    /// - Today's Transactions Count
    /// - Success Rate (Paid/Overpaid sessions / Total sessions)
    pub async fn get_stats(
        &self,
        merchant_ids: &[String],
        network: Option<crate::entity::Network>,
        _environment: crate::entity::Environment,
    ) -> Result<crate::api::dtos::auth::MerchantStatsResponse> {
        // Optional network isolation
        let network_name = network.map(|n| n.as_str().to_string());

        // 1. Transactions Logic
        // We use `transactions` table for volume as it records actual confirmed money movement.
        // `total_volume` = Sum of all transaction amounts
        // `today_volume` = Sum of transaction amounts where created_at >= Today 00:00 UTC

        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        // Query 1: Total Stats (Volume & Count)

        let mut total_query = Transactions::find()
            .join(
                sea_orm::JoinType::InnerJoin,
                transactions::Relation::CheckoutSession.def(),
            )
            .filter(checkout_sessions::Column::MerchantId.is_in(merchant_ids));
        if let Some(ref net_name) = network_name {
            total_query = total_query.filter(checkout_sessions::Column::Network.eq(net_name));
        }
        let total_stats = total_query
            .select_only()
            .column_as(transactions::Column::Amount.sum(), "volume")
            .column_as(transactions::Column::TxHash.count(), "count")
            .into_model::<StatsResult>()
            .one(&self.db)
            .await?;

        let (total_volume, total_tx_count) = match total_stats {
            Some(stats) => (
                stats.volume.unwrap_or(sea_orm::prelude::Decimal::ZERO),
                stats.count as u64,
            ),
            None => (sea_orm::prelude::Decimal::ZERO, 0),
        };

        // Query 2: Today Stats (Volume & Count)

        let mut today_query = Transactions::find()
            .join(
                sea_orm::JoinType::InnerJoin,
                transactions::Relation::CheckoutSession.def(),
            )
            .filter(checkout_sessions::Column::MerchantId.is_in(merchant_ids));
        if let Some(ref net_name) = network_name {
            today_query = today_query.filter(checkout_sessions::Column::Network.eq(net_name));
        }
        let today_stats = today_query
            .filter(transactions::Column::CreatedAt.gte(today_start))
            .select_only()
            .column_as(transactions::Column::Amount.sum(), "volume")
            .column_as(transactions::Column::TxHash.count(), "count")
            .into_model::<StatsResult>()
            .one(&self.db)
            .await?;

        let (today_volume, today_tx_count) = match today_stats {
            Some(stats) => (
                stats.volume.unwrap_or(sea_orm::prelude::Decimal::ZERO),
                stats.count as u64,
            ),
            None => (sea_orm::prelude::Decimal::ZERO, 0),
        };

        Ok(crate::api::dtos::auth::MerchantStatsResponse {
            total_volume_usdt: {
                let divisor = rust_decimal::Decimal::from(1_000_000_i64);
                (total_volume / divisor).normalize().to_string()
            },
            today_volume_usdt: {
                let divisor = rust_decimal::Decimal::from(1_000_000_i64);
                (today_volume / divisor).normalize().to_string()
            },
            total_transactions: total_tx_count,
            total_transactions_today: today_tx_count,
        })
    }
    /// Generate password reset token (JWT, 1h expiry)
    fn generate_password_reset_token(
        &self,
        merchant_id: &str,
        token_version: i32,
    ) -> Result<String> {
        let claims = PasswordResetClaims {
            sub: merchant_id.to_string(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
            purpose: "password_reset".to_string(),
            tv: token_version,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.expose_secret().as_bytes()),
        )
        .map_err(|e| anyhow!("Failed to generate password reset token: {}", e))
    }

    // ============================================================
    // API Key Management
    // ============================================================

    /// Create a new API key for a merchant
    ///
    /// Generates a cryptographically secure API key with:
    /// - Prefix: sk_test_ (sandbox) or sk_live_ (production)
    /// - Key stored as SHA-256 hash
    /// - Full key returned only once
    ///
    /// Note: Environment is derived from prefix, not stored as a field.
    pub async fn create_api_key(
        &self,
        merchant_id: &str,
        name: Option<String>,
        is_test: bool,
    ) -> Result<ApiKeyResponse, MerchantError> {
        // Verify merchant exists
        Merchants::find_by_id(merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("Merchant not found".into()))?;

        // Generate API key with environment-specific prefix
        let prefix = if is_test { "sk_test_" } else { "sk_live_" };
        let key_suffix = generate_random_key(24);
        let full_key = format!("{}{}", prefix, key_suffix);

        // Hash the key for storage
        let key_hash = hash_api_key(&full_key);

        let key_id = format!(
            "key_{}",
            Uuid::new_v4().to_string().replace("-", "")[..16].to_string()
        );

        let api_key = api_keys::ActiveModel {
            id: Set(key_id.clone()),
            merchant_id: Set(merchant_id.to_string()),
            name: Set(name.clone()),
            key_prefix: Set(prefix.to_string()),
            key_hash: Set(key_hash),
            is_active: Set(true),
            ..Default::default()
        };

        let result = api_key.insert(&self.db).await?;

        let env_str = if is_test { "sandbox" } else { "production" };
        info!(key_id = %key_id, merchant_id = %merchant_id, environment = %env_str, "API key created");

        Ok(ApiKeyResponse {
            id: key_id,
            key: full_key, // Only shown once!
            prefix: prefix.to_string(),
            name,
            created_at: result.created_at.to_rfc3339(),
            last_used_at: None, // New key, never used
        })
    }

    /// List all API keys for a merchant
    pub async fn get_api_keys(&self, merchant_id: &str) -> Result<Vec<api_keys::Model>> {
        let keys = ApiKeys::find()
            .filter(api_keys::Column::MerchantId.eq(merchant_id))
            .filter(api_keys::Column::IsActive.eq(true))
            .order_by_desc(api_keys::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(keys)
    }

    /// Verify API key and return (merchant_id, environment)
    ///
    /// Uses direct hash lookup - O(1) via database index.
    /// API keys only determine environment (sandbox/production), not network.
    /// Network is specified per-request (e.g., body.network in checkout).
    pub async fn verify_api_key(
        &self,
        api_key: &str,
    ) -> Result<(String, crate::entity::Environment)> {
        use crate::entity::Environment;

        // Validate format and extract env from prefix
        let environment = if api_key.starts_with("sk_test_") {
            Environment::Sandbox
        } else if api_key.starts_with("sk_live_") {
            Environment::Production
        } else {
            return Err(anyhow!("Invalid API key format"));
        };

        let key_hash = hash_api_key(api_key);

        // Direct O(1) lookup via hash index
        let key = ApiKeys::find()
            .filter(api_keys::Column::KeyHash.eq(&key_hash))
            .filter(api_keys::Column::IsActive.eq(true))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Invalid API key"))?;

        let merchant_id = key.merchant_id.clone();

        // Fire-and-forget: update last_used_at without blocking auth
        let db = self.db.clone();
        let key_id = key.id.clone();
        tokio::spawn(async move {
            let mut active: api_keys::ActiveModel = key.into();
            active.last_used_at = Set(Some(chrono::Utc::now().into()));
            if let Err(e) = active.update(&db).await {
                tracing::warn!(key_id = %key_id, "Failed to update last_used_at: {}", e);
            }
        });

        Ok((merchant_id, environment))
    }

    /// Revoke an API key
    pub async fn revoke_api_key(
        &self,
        key_id: &str,
        merchant_id: &str,
    ) -> Result<(), MerchantError> {
        let key = ApiKeys::find_by_id(key_id)
            .filter(api_keys::Column::MerchantId.eq(merchant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MerchantError::NotFound(format!(
                    "API key '{}' not found or does not belong to this merchant",
                    key_id
                ))
            })?;

        let mut active: api_keys::ActiveModel = key.into();
        active.is_active = Set(false);
        active.update(&self.db).await?;

        info!(key_id = %key_id, "API key revoked");
        Ok(())
    }

    /// List all API keys for a merchant
    ///
    /// Returns ApiKeySummary (without key_hash) to avoid leaking sensitive data.
    pub async fn list_api_keys(&self, merchant_id: &str) -> Result<Vec<ApiKeySummary>> {
        let keys = ApiKeys::find()
            .filter(api_keys::Column::MerchantId.eq(merchant_id))
            .order_by_desc(api_keys::Column::CreatedAt)
            .all(&self.db)
            .await?;

        // Convert to DTO (without key_hash)
        let summaries = keys
            .into_iter()
            .map(|k| ApiKeySummary {
                id: k.id,
                prefix: k.key_prefix,
                name: k.name,
                is_active: k.is_active,
                created_at: k.created_at.to_rfc3339(),
                last_used_at: k.last_used_at.map(|dt| dt.to_rfc3339()),
            })
            .collect();

        Ok(summaries)
    }

    // ============================================================
    // Merchant Profile Management
    // ============================================================

    /// Get merchant by ID
    pub async fn get_merchant(&self, merchant_id: &str) -> Result<Option<merchants::Model>> {
        Ok(Merchants::find_by_id(merchant_id).one(&self.db).await?)
    }

    /// Update merchant profile (non-sensitive fields)
    ///
    /// For updating collection_address, use `update_collection_address_with_2fa` instead.
    pub async fn update_merchant(
        &self,
        merchant_id: &str,
        name: Option<String>,
    ) -> Result<merchants::Model, MerchantError> {
        let merchant = self
            .get_merchant(merchant_id)
            .await?
            .ok_or(MerchantError::NotFound("Merchant not found".into()))?;

        let mut active: merchants::ActiveModel = merchant.into();

        if let Some(name) = name {
            // Validate name length
            let name = name.trim();
            if name.is_empty() || name.len() > 100 {
                return Err(MerchantError::InvalidInput(
                    "Name must be between 1 and 100 characters".into(),
                ));
            }
            active.name = Set(name.to_string());
        }

        let updated = active.update(&self.db).await?;
        info!(merchant_id = %merchant_id, "Merchant profile updated");

        Ok(updated)
    }

    /// Update collection address with 2FA verification (Per Chain)
    ///
    /// Security: This is a critical operation that redirects all collected funds.
    /// Requirements:
    /// - 2FA MUST be enabled first
    /// - Valid TOTP code required
    ///
    /// # Arguments
    /// * `merchant_id` - The merchant to update
    /// * `network` - The Network enum (e.g. Tron)
    /// * `environment` - The Environment enum (Production/Sandbox)
    /// * `collection_address` - New Tron address for fund collection
    /// * `totp_code` - 6-digit TOTP code for verification
    pub async fn update_collection_address_with_2fa(
        &self,
        merchant_id: &str,
        network: crate::entity::Network,
        environment: crate::entity::Environment,
        collection_address: &str,
        totp_code: &str,
    ) -> Result<crate::entity::merchant_chain_accounts::Model, MerchantError> {
        use crate::entity::merchant_chain_accounts;

        let _merchant = self
            .get_merchant(merchant_id)
            .await?
            .ok_or_else(|| MerchantError::NotFound("Merchant not found".into()))?;
        let net_enum = network;
        let env = environment;

        // Validate address format based on network (TRON Base58 vs EVM 0x)
        net_enum
            .validate_collection_address(collection_address)
            .map_err(|e| {
                MerchantError::InvalidInput(format!("Invalid collection address: {}", e))
            })?;

        // Security: 2FA MUST be enabled before setting collection_address
        // Look up user for 2FA check
        let membership = org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(merchant_id))
            .filter(org_members::Column::Status.eq("active"))
            .one(&self.db)
            .await?
            .ok_or(MerchantError::NotFound("Membership not found".into()))?;

        let uid = membership
            .user_id
            .ok_or(MerchantError::NotFound("User not found".into()))?;

        let user = users::Entity::find_by_id(&uid)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if !user.is_totp_enabled {
            return Err(MerchantError::TwoFARequired);
        }

        // Verify 2FA code or backup code
        let is_valid = verify_2fa_code_for_user(&self.db, &user, totp_code).await?;
        if !is_valid {
            return Err(MerchantError::Invalid2FACode);
        }

        info!(merchant_id = %merchant_id, network = %network, "2FA verified for collection_address change");

        // Find or Create Chain Account
        let chain_account_opt = merchant_chain_accounts::Entity::find_by_id((
            merchant_id.to_string(),
            env.clone(),
            net_enum.clone(),
        ))
        .one(&self.db)
        .await?;

        let account = match chain_account_opt {
            Some(acc) => acc,
            None => {
                // Lazy Initialization
                // If the account doesn't exist, we try to initialize it now.
                // This improves UX by auto-fixing the "not initialized" state.
                let am = self.address_manager.as_ref().ok_or_else(|| {
                    MerchantError::Internal(anyhow!(
                        "AddressManager not configured in MerchantService"
                    ))
                })?;

                info!(
                    merchant_id = %merchant_id,
                    network = %net_enum,
                    "Lazy initializing chain account during collection address update"
                );

                // Call initialization logic
                // This handles xpub encryption and address generation internally.
                am.initialize_merchant_addresses(merchant_id, net_enum.clone(), env.clone())
                    .await
                    .map_err(|e| {
                        MerchantError::Internal(anyhow!(
                            "Failed to lazy initialize addresses: {}",
                            e
                        ))
                    })?;

                // Fetch the newly created account
                merchant_chain_accounts::Entity::find_by_id((
                    merchant_id.to_string(),
                    env.clone(),
                    net_enum.clone(),
                ))
                .one(&self.db)
                .await?
                .ok_or_else(|| {
                    MerchantError::Internal(anyhow!(
                        "Failed to retrieve account after lazy initialization"
                    ))
                })?
            }
        };

        let mut active: merchant_chain_accounts::ActiveModel = account.into();
        active.collection_address = Set(Some(collection_address.to_string()));
        let updated_account = active.update(&self.db).await?;

        info!(merchant_id = %merchant_id, network = %net_enum, new_address = %collection_address, "Collection address updated");

        Ok(updated_account)
    }

    // ============================================================
    // Balance / Billing Management
    // ============================================================

    /// Get merchant balance in micro USDT (atomic units) for the current environment.
    ///
    /// Returns SUM of all chain account balances. Frontend divides by 1_000_000 for display.
    pub async fn get_merchant_balance(&self, merchant_id: &str) -> Result<i64> {
        use crate::entity::merchant_chain_accounts;
        use sea_orm::{ColumnTrait, QueryFilter};

        let entity_env = self.environment.to_entity_environment();

        let accounts = merchant_chain_accounts::Entity::find()
            .filter(merchant_chain_accounts::Column::MerchantId.eq(merchant_id))
            .filter(merchant_chain_accounts::Column::Environment.eq(entity_env))
            .all(&self.db)
            .await?;

        Ok(accounts.iter().map(|a| a.usdt_balance).sum())
    }

    // ============================================================
    // 2FA (TOTP) Management
    // ============================================================

    /// Setup TOTP for a merchant (generates secret, not yet enabled)
    ///
    /// Returns the secret and QR code URI for the authenticator app.
    /// The merchant must call `enable_totp` with a valid code to activate.
    pub async fn setup_totp(
        &self,
        user_id: &str,
    ) -> Result<super::types::TotpSetupResponse, MerchantError> {
        use totp_rs::{Algorithm, Secret, TOTP};

        // Directly look up user by ID (post Role & Org refactor)
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if user.is_totp_enabled {
            return Err(MerchantError::TwoFAAlreadyEnabled);
        }

        let secret = Secret::generate_secret();
        let secret_base32 = secret.to_encoded().to_string();

        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret
                .to_bytes()
                .map_err(|e| MerchantError::Internal(anyhow!("Secret error: {}", e)))?,
            Some("IronixPay".to_string()),
            user.email.clone(),
        )
        .map_err(|e| MerchantError::Internal(anyhow!("TOTP error: {}", e)))?;

        let backup_codes: Vec<String> = (0..8)
            .map(|_| generate_random_key(4).to_uppercase())
            .collect();

        use super::types::BackupCodeEntry;
        let backup_code_entries: Vec<BackupCodeEntry> = backup_codes
            .iter()
            .map(|code| BackupCodeEntry {
                hash: hash_backup_code(code),
                used: false,
            })
            .collect();

        let backup_codes_json = serde_json::to_string(&backup_code_entries).map_err(|e| {
            MerchantError::Internal(anyhow!("Failed to serialize backup codes: {}", e))
        })?;

        // Update users table
        let mut active: users::ActiveModel = user.clone().into();
        active.totp_secret = Set(Some(secret_base32.clone()));
        active.backup_codes = Set(Some(backup_codes_json));
        active.update(&self.db).await?;

        let qr_code_uri = totp.get_url();

        info!(user_id = %user_id, "TOTP setup initiated with backup codes");

        Ok(super::types::TotpSetupResponse {
            secret: secret_base32,
            qr_code_uri,
            backup_codes,
        })
    }

    /// Enable TOTP after verifying a code from the authenticator app
    ///
    /// This confirms the user has successfully set up their authenticator.
    pub async fn enable_totp(&self, user_id: &str, code: &str) -> Result<(), MerchantError> {
        // Directly look up user by ID (post Role & Org refactor)
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if user.is_totp_enabled {
            return Err(MerchantError::TwoFAAlreadyEnabled);
        }

        if user.totp_secret.is_none() {
            return Err(MerchantError::NoPendingSetup);
        }

        let is_valid = verify_2fa_code_for_user(&self.db, &user, code).await?;
        if !is_valid {
            return Err(MerchantError::Invalid2FACode);
        }

        let mut active: users::ActiveModel = user.into();
        active.is_totp_enabled = Set(true);
        active.update(&self.db).await?;

        info!(user_id = %user_id, "TOTP enabled");
        Ok(())
    }

    /// Disable TOTP (requires valid code for security)
    pub async fn disable_totp(&self, user_id: &str, code: &str) -> Result<(), MerchantError> {
        // Directly look up user by ID (post Role & Org refactor)
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| MerchantError::NotFound("User not found".into()))?;

        if !user.is_totp_enabled {
            return Err(MerchantError::TwoFANotEnabled);
        }

        if user.totp_secret.is_none() {
            return Err(MerchantError::Internal(anyhow!("TOTP secret not found")));
        }

        let is_valid = verify_2fa_code_for_user(&self.db, &user, code).await?;
        if !is_valid {
            return Err(MerchantError::Invalid2FACode);
        }

        let mut active: users::ActiveModel = user.into();
        active.is_totp_enabled = Set(false);
        active.totp_secret = Set(None);
        active.backup_codes = Set(None);
        active.update(&self.db).await?;

        self.invalidate_all_tokens(&user_id).await?;

        info!(user_id = %user_id, "TOTP disabled");
        Ok(())
    }

    // ============================================================
    // Internal Helpers
    // ============================================================

    /// Validate password strength
    fn validate_password(&self, password: &str) -> Result<(), MerchantError> {
        let req = &self.password_requirements;

        if password.len() < req.min_length {
            return Err(MerchantError::WeakPassword(format!(
                "Password must be at least {} characters",
                req.min_length
            )));
        }

        if req.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(MerchantError::WeakPassword(
                "Password must contain at least one uppercase letter".into(),
            ));
        }

        if req.require_lowercase && !password.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(MerchantError::WeakPassword(
                "Password must contain at least one lowercase letter".into(),
            ));
        }

        if req.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(MerchantError::WeakPassword(
                "Password must contain at least one digit".into(),
            ));
        }

        if req.require_special
            && !password
                .chars()
                .any(|c| "!@#$%^&*()_+-=[]{}|;':\",./<>?".contains(c))
        {
            return Err(MerchantError::WeakPassword(
                "Password must contain at least one special character".into(),
            ));
        }

        Ok(())
    }

    // ============================================================
    // Response Builders
    // ============================================================

    /// Build MerchantResponse from Model by aggregating additional data
    pub async fn build_merchant_response(
        &self,
        merchant: merchants::Model,
    ) -> Result<super::types::MerchantResponse> {
        use super::types::MerchantResponse;
        use crate::entity::{merchant_chain_accounts, Environment, Network};

        // Fetch Chain Accounts (addresses + balances)
        let chain_accounts = merchant_chain_accounts::Entity::find()
            .filter(merchant_chain_accounts::Column::MerchantId.eq(&merchant.id))
            .all(&self.db)
            .await?;

        let mut collection_address = None; // TRON prod (backward compat)
        let mut collection_address_sandbox = None; // TRON sandbox (backward compat)
        let mut collection_addresses: std::collections::HashMap<
            String,
            std::collections::HashMap<String, Option<String>>,
        > = std::collections::HashMap::new();

        let mut chain_balances: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = std::collections::HashMap::new();
        let mut chain_usdc_balances: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = std::collections::HashMap::new();
        let mut balance_prod: i64 = 0;
        let mut balance_sandbox: i64 = 0;
        let mut usdc_balance_prod: i64 = 0;
        let mut usdc_balance_sandbox: i64 = 0;

        for ca in chain_accounts {
            let network_key = ca.network.as_str().to_string(); // "TRON", "BSC", etc.
            let env_key = match ca.environment {
                Environment::Production => "production",
                Environment::Sandbox => "sandbox",
            };

            // Build per-chain collection address map
            collection_addresses
                .entry(network_key.clone())
                .or_default()
                .insert(env_key.to_string(), ca.collection_address.clone());

            // Build per-chain USDT balance map
            chain_balances
                .entry(network_key.clone())
                .or_default()
                .insert(
                    env_key.to_string(),
                    crate::api::dtos::checkout::from_micro(ca.usdt_balance, "USDT"),
                );

            // Build per-chain USDC balance map
            chain_usdc_balances.entry(network_key).or_default().insert(
                env_key.to_string(),
                crate::api::dtos::checkout::from_micro(ca.usdc_balance, "USDC"),
            );

            // Aggregate USDT balances per environment (backward compat)
            match ca.environment {
                Environment::Production => balance_prod += ca.usdt_balance,
                Environment::Sandbox => balance_sandbox += ca.usdt_balance,
            }

            // Aggregate USDC balances per environment
            match ca.environment {
                Environment::Production => usdc_balance_prod += ca.usdc_balance,
                Environment::Sandbox => usdc_balance_sandbox += ca.usdc_balance,
            }

            // Backward-compat: populate flat TRON fields
            if ca.network == Network::Tron {
                match ca.environment {
                    Environment::Production => collection_address = ca.collection_address,
                    Environment::Sandbox => collection_address_sandbox = ca.collection_address,
                }
            }
        }

        // Fetch owner user for auth fields that were moved from merchants to users table
        let owner_user = {
            let owner_uid = org_members::Entity::find()
                .filter(org_members::Column::OrgId.eq(&merchant.id))
                .filter(org_members::Column::Role.eq("owner"))
                .filter(org_members::Column::Status.eq("active"))
                .one(&self.db)
                .await?
                .and_then(|m| m.user_id);
            if let Some(uid) = owner_uid {
                users::Entity::find_by_id(&uid).one(&self.db).await?
            } else {
                None
            }
        };

        Ok(MerchantResponse {
            id: merchant.id,
            name: merchant.name.clone(),
            email: owner_user
                .as_ref()
                .map(|u| u.email.clone())
                .unwrap_or_default(),
            user_name: None, // Populated by route handler with authenticated user's name
            org_name: Some(merchant.name),
            collection_address,
            collection_address_sandbox,
            collection_addresses,
            is_2fa_enabled: owner_user
                .as_ref()
                .map(|u| u.is_totp_enabled)
                .unwrap_or(false),
            status: format!("{:?}", merchant.status),
            balance_prod: crate::api::dtos::checkout::from_micro(balance_prod, "USDT"),
            balance_sandbox: crate::api::dtos::checkout::from_micro(balance_sandbox, "USDT"),
            usdc_balance_prod: crate::api::dtos::checkout::from_micro(usdc_balance_prod, "USDC"),
            usdc_balance_sandbox: crate::api::dtos::checkout::from_micro(
                usdc_balance_sandbox,
                "USDC",
            ),
            chain_balances,
            chain_usdc_balances,
            gas_unit: "USDT".to_string(),
        })
    }
}

// ============================================================
// Crypto Utilities
// ============================================================

/// Generate a cryptographically secure random hex key
fn generate_random_key(bytes: usize) -> String {
    let mut random_bytes = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut random_bytes);
    hex::encode(random_bytes)
}

/// Hash API key for storage using SHA-256
fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash backup code for storage using SHA-256
fn hash_backup_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify and mark a backup code as used
///
/// Returns Ok(true) if code is valid and unused, Ok(false) if invalid or already used.
/// Updates the database to mark the code as used.
async fn verify_and_mark_backup_code_for_user(
    db: &DatabaseConnection,
    user: &users::Model,
    code: &str,
) -> Result<bool> {
    use super::types::BackupCodeEntry;

    let backup_codes_json = match &user.backup_codes {
        Some(json) => json,
        None => return Ok(false),
    };

    let mut backup_codes: Vec<BackupCodeEntry> = serde_json::from_str(backup_codes_json)
        .map_err(|e| anyhow!("Failed to parse backup codes: {}", e))?;

    let code_hash = hash_backup_code(code);

    let mut found = false;
    for entry in &mut backup_codes {
        if entry.hash == code_hash {
            if entry.used {
                return Ok(false);
            }
            entry.used = true;
            found = true;
            break;
        }
    }

    if !found {
        return Ok(false);
    }

    let updated_json = serde_json::to_string(&backup_codes)
        .map_err(|e| anyhow!("Failed to serialize backup codes: {}", e))?;

    let mut active: users::ActiveModel = user.clone().into();
    active.backup_codes = Set(Some(updated_json));
    active.update(db).await?;

    Ok(true)
}

/// Unified 2FA verification for users table (supports both TOTP and backup codes)
async fn verify_2fa_code_for_user(
    db: &DatabaseConnection,
    user: &users::Model,
    code: &str,
) -> Result<bool> {
    let totp_secret = user
        .totp_secret
        .as_ref()
        .ok_or_else(|| anyhow!("2FA not configured"))?;

    if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
        let totp = build_totp(totp_secret, &user.email)?;
        totp.check_current(code)
            .map_err(|e| anyhow!("TOTP error: {}", e))
    } else {
        verify_and_mark_backup_code_for_user(db, user, code).await
    }
}

/// Build a TOTP instance from a Base32 secret
///
/// Uses totp-rs for RFC 6238 compliant TOTP verification.
fn build_totp(secret_base32: &str, account_name: &str) -> Result<totp_rs::TOTP> {
    use totp_rs::{Algorithm, Secret, TOTP};

    let secret = Secret::Encoded(secret_base32.to_string());
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| anyhow!("Invalid TOTP secret: {}", e))?;

    TOTP::new(
        Algorithm::SHA1,
        6,  // digits
        1,  // skew (allow ±1 time step for clock drift)
        30, // step (seconds)
        secret_bytes,
        Some("TronCheckout".to_string()),
        account_name.to_string(),
    )
    .map_err(|e| anyhow!("TOTP error: {}", e))
}

fn email_domain(email: &str) -> Option<&str> {
    email.rsplit_once('@').map(|(_, domain)| domain)
}

fn is_blocked_email_domain(email: &str, blocked_domains: &HashSet<String>) -> bool {
    let Some(domain) = email_domain(email) else {
        return false;
    };

    blocked_domains
        .iter()
        .any(|blocked| domain == blocked || domain.ends_with(&format!(".{blocked}")))
}

// ============================================================
// TOTP Rate Limiting (HIGH-3 Fix)
// ============================================================

/// In-memory store for TOTP failed attempts
/// Key: merchant_id, Value: (attempt_count, first_attempt_time)
static TOTP_RATE_LIMITER: LazyLock<DashMap<String, (u32, Instant)>> = LazyLock::new(DashMap::new);

/// Rate limit configuration
const TOTP_MAX_ATTEMPTS: u32 = 5;
const TOTP_WINDOW_SECONDS: u64 = 300; // 5 minutes
const TOTP_MAX_ENTRIES: usize = 10_000; // Max entries to prevent OOM

/// Check if merchant is rate-limited for TOTP verification
/// Also performs lazy deletion of expired entries
fn is_totp_rate_limited(merchant_id: &str) -> bool {
    if let Some(entry) = TOTP_RATE_LIMITER.get(merchant_id) {
        let value: &(u32, Instant) = entry.value();
        let elapsed = value.1.elapsed().as_secs();

        // If window has expired, remove entry (lazy deletion) and not rate limited
        if elapsed > TOTP_WINDOW_SECONDS {
            drop(entry); // Release read lock before remove
            TOTP_RATE_LIMITER.remove(merchant_id);
            return false;
        }

        // Still within window, check attempt count
        return value.0 >= TOTP_MAX_ATTEMPTS;
    }
    false
}

/// Record a failed TOTP verification attempt
/// Includes protection against memory exhaustion attacks
fn record_totp_failure(merchant_id: &str) {
    let now = Instant::now();

    // Protection: If too many entries, do a cleanup sweep first
    if TOTP_RATE_LIMITER.len() >= TOTP_MAX_ENTRIES {
        cleanup_expired_entries();
    }

    // If still at capacity after cleanup, skip recording (fail open but log)
    if TOTP_RATE_LIMITER.len() >= TOTP_MAX_ENTRIES {
        warn!(
            merchant_id = %merchant_id,
            "TOTP rate limiter at capacity, skipping record (potential attack)"
        );
        return;
    }

    TOTP_RATE_LIMITER
        .entry(merchant_id.to_string())
        .and_modify(|entry: &mut (u32, Instant)| {
            // If window expired, reset counter
            if entry.1.elapsed().as_secs() > TOTP_WINDOW_SECONDS {
                entry.0 = 1;
                entry.1 = now;
            } else {
                entry.0 += 1;
            }
        })
        .or_insert((1, now));
}

/// Clear TOTP failure counter on successful verification
fn clear_totp_failures(merchant_id: &str) {
    TOTP_RATE_LIMITER.remove(merchant_id);
}

/// Cleanup all expired entries from the rate limiter
/// Called when approaching capacity to prevent OOM
fn cleanup_expired_entries() {
    let keys_to_remove: Vec<String> = TOTP_RATE_LIMITER
        .iter()
        .filter(|entry| entry.value().1.elapsed().as_secs() > TOTP_WINDOW_SECONDS)
        .map(|entry| entry.key().clone())
        .collect();

    for key in keys_to_remove {
        TOTP_RATE_LIMITER.remove(&key);
    }

    info!(
        remaining = TOTP_RATE_LIMITER.len(),
        "TOTP rate limiter cleanup completed"
    );
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_format() {
        let key = generate_random_key(24);
        assert_eq!(key.len(), 48); // 24 bytes = 48 hex chars
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let key = "sk_test_abc123";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_api_key_different_keys() {
        let hash1 = hash_api_key("sk_test_key1");
        let hash2 = hash_api_key("sk_test_key2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_password_requirements_default() {
        let req = PasswordRequirements::default();
        assert_eq!(req.min_length, 8);
        assert!(req.require_uppercase);
        assert!(req.require_lowercase);
        assert!(req.require_digit);
        assert!(!req.require_special);
    }

    #[test]
    fn email_domain_uses_last_separator() {
        assert_eq!(email_domain("name@example.com"), Some("example.com"));
        assert_eq!(email_domain("invalid"), None);
    }

    #[test]
    fn blocks_exact_domain_and_subdomains_only() {
        let blocked = HashSet::from(["emalupe.com".to_string()]);

        assert!(is_blocked_email_domain("user@emalupe.com", &blocked));
        assert!(is_blocked_email_domain("user@mail.emalupe.com", &blocked));
        assert!(!is_blocked_email_domain("user@notemalupe.com", &blocked));
        assert!(!is_blocked_email_domain("user@example.com", &blocked));
    }
}
