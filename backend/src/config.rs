use secrecy::{ExposeSecret, Secret};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

// ── Chain TOML Config (deserialized from chains.toml) ──

/// Top-level TOML file structure:
/// ```toml
/// [chains.BSC]
/// rpc_urls = [...]
///
/// [solana]
/// rpc_urls = [...]
/// treasury_address = "..."
/// ```
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ChainsFile {
    #[serde(default)]
    pub chains: HashMap<String, ChainTomlConfig>,
    /// Solana configuration (independent from EVM chains — different field set)
    #[serde(default)]
    pub solana: Option<SolanaChainConfig>,
}

/// Solana chain configuration (separate from EVM ChainTomlConfig)
///
/// Solana has fundamentally different requirements:
/// - No gas_sponsor (uses Fee Payer Delegation instead)
/// - Token configs derived from hardcoded ChainConfig (same as TRON/EVM)
/// - Treasury is HD-derived at m/44'/501'/0'/0' (hardcoded, matching TRON/EVM pattern)
#[derive(serde::Deserialize, Debug, Clone)]
pub struct SolanaChainConfig {
    /// RPC endpoint URLs (primary + fallbacks)
    #[serde(alias = "rpc_url", deserialize_with = "deserialize_rpc_urls")]
    pub rpc_urls: Vec<String>,
    /// Per-chain outbound fee for Withdrawal + Payout (USDT microunits).
    /// If not set, falls back to FeeConfig::flat_payout_fee (1.5 USDT).
    pub outbound_fee: Option<i64>,
    /// Per-chain deposit fee floor (USDT microunits).
    /// If not set, falls back to FeeConfig::floor_deposit (1 USDT).
    pub floor_deposit: Option<i64>,
}

/// Per-chain configuration entry in chains.toml
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ChainTomlConfig {
    /// RPC endpoint URLs. Accepts either:
    /// - `rpc_url = "https://..."` (single string, backward compatible)
    /// - `rpc_urls = ["https://primary", "https://fallback"]` (array with failover)
    /// First URL is primary, rest are fallbacks in order.
    #[serde(alias = "rpc_url", deserialize_with = "deserialize_rpc_urls")]
    pub rpc_urls: Vec<String>,
    /// Treasury / collection address — safety assertion (optional).
    /// When present, verified against HD-derived address at startup.
    /// Omit for chains where TRON assertion already validates seed correctness.
    pub treasury_address: Option<String>,
    /// Optional USDT contract override (defaults from Network::chain_config())
    pub usdt_contract: Option<String>,
    /// Optional USDC contract override (defaults from Network::chain_config())
    pub usdc_contract: Option<String>,
    /// EVM gas sponsor configuration (only needed for EVM chains)
    pub gas_sponsor: Option<GasSponsorConfig>,
    /// Per-chain outbound fee for Withdrawal + Payout (USDT microunits).
    /// If not set, falls back to FeeConfig::flat_payout_fee (1.5 USDT).
    /// Example: 300_000 = 0.3 USDT, 3_000_000 = 3.0 USDT
    pub outbound_fee: Option<i64>,
    /// Per-chain deposit fee floor (USDT microunits).
    /// If not set, falls back to FeeConfig::floor_deposit (1 USDT).
    /// Example: 100_000 = 0.1 USDT
    pub floor_deposit: Option<i64>,
}

/// Deserialize a TOML value that is either a single string or an array of strings.
/// This enables backward compat: `rpc_url = "..."` and `rpc_urls = ["...", "..."]`
///
/// Used by both `ChainTomlConfig` and `SolanaChainConfig`.
fn deserialize_rpc_urls<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct StringOrVec;
    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string or array of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![v.to_string()])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut urls = Vec::new();
            while let Some(url) = seq.next_element::<String>()? {
                urls.push(url);
            }
            if urls.is_empty() {
                return Err(de::Error::custom("rpc_urls array must not be empty"));
            }
            Ok(urls)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// Gas sponsor configuration for EVM chains (derives key from HD wallet)
#[derive(serde::Deserialize, Debug, Clone)]
pub struct GasSponsorConfig {
    /// On-chain address — safety assertion (optional).
    /// When present, verified against HD-derived address at startup.
    pub address: Option<String>,
    /// HD wallet account index for the gas sponsor key
    pub account_index: i32,
    /// HD wallet path index for the gas sponsor key
    pub path_index: u32,
}

/// Application environment - determines network and operational mode
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Environment {
    /// Production: TRON mainnet with energy delegation
    Production,
    /// Sandbox: TRON Nile testnet with TRX transfers for energy
    Sandbox,
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "production" | "prod" => Ok(Environment::Production),
            "sandbox" | "dev" | "development" | "local" => Ok(Environment::Sandbox),
            _ => Err(format!(
                "Invalid ENVIRONMENT '{}': must be 'Production' or 'Sandbox'",
                s
            )),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Production => write!(f, "Production"),
            Environment::Sandbox => write!(f, "Sandbox"),
        }
    }
}

impl Environment {
    /// Network identifier for database records (pure chain name)
    pub fn network(&self) -> &'static str {
        match self {
            Environment::Production => "TRON",
            Environment::Sandbox => "TRON",
        }
    }

    /// Default TRON RPC URL
    pub fn default_rpc_url(&self) -> &'static str {
        match self {
            Environment::Production => "https://api.trongrid.io",
            Environment::Sandbox => "https://nile.trongrid.io",
        }
    }

    /// Convert to entity::Environment for database operations
    pub fn to_entity_environment(&self) -> crate::entity::Environment {
        match self {
            Environment::Production => crate::entity::Environment::Production,
            Environment::Sandbox => crate::entity::Environment::Sandbox,
        }
    }

    /// Whether to use Netts.io energy delegation (Production only)
    /// Sandbox uses TRX transfers instead
    pub fn use_energy_delegation(&self) -> bool {
        matches!(self, Environment::Production)
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    // Environment (determines network, RPC, energy mode)
    pub environment: Environment,

    // Server
    pub host: String,
    pub port: u16,

    // Database
    pub database_url: Secret<String>,

    // Tron (derived from environment, can be overridden by chains.toml TRON entry)
    pub tron_full_node: String,
    pub tron_solidity_node: String,

    /// Network identifier derived from environment (pure chain name, e.g., "TRON")
    pub network: String,

    // Address Pool
    pub address_pool_size: usize,
    pub address_ttl_seconds: u64,
    pub recycle_check_interval_seconds: u64,

    // Session
    pub session_expiry_minutes: u64,

    // Webhook
    pub webhook_retry_max: u8,
    pub webhook_timeout_seconds: u64,

    // JWT Auth
    pub jwt_secret: Secret<String>,
    pub jwt_expiry_hours: i64,

    // Energy Provider
    pub netts_api_key: Option<Secret<String>>,
    pub sweep_threshold_ratio: f64,
    pub rate_limit_per_minute: u64,

    // Registration abuse protection
    pub turnstile_required: bool,
    pub turnstile_secret_key: Option<Secret<String>>,
    pub turnstile_expected_hostname: String,
    pub blocked_email_domains: Vec<String>,

    // Master Mnemonic (for Local/Sandbox Key Provider)
    pub master_mnemonic: Option<Secret<String>>,

    // AWS KMS (for Production envelope encryption)
    pub aws_kms_key_id: Option<String>,
    pub encrypted_seed: Option<Secret<String>>,

    // Encryption (Data Encryption Key for xpub, TOTP secrets, etc.)
    pub encryption_key: Secret<String>,

    // Sweeper Configuration
    pub sweep_confirmation_blocks: u64,
    pub sweep_energy_estimate: u64,

    // Checkout Frontend URL
    pub checkout_base_url: String,

    // Dashboard Frontend URL (for invite links, password reset, etc.)
    pub dashboard_base_url: String,

    // CORS Allowed Origins (comma-separated, e.g. "https://pay.ironixpay.com,https://app.ironixpay.com")
    pub cors_allowed_origins: Vec<String>,

    // Database Pool Configuration
    pub database_max_connections: u32,

    // Admin Portal Auth (Simple Token)
    pub admin_token: Option<Secret<String>>,

    // TronGrid API Key (improves rate limits and reliability)
    pub trongrid_api_key: Option<String>,

    // Treasury low-balance alert threshold (USDT microunits, default 100 USDT)
    pub treasury_low_balance_threshold: i64,

    /// Production database URL (Sandbox only).
    /// When set, JIT shadow accounts sync TOTP fields from production users.
    pub production_database_url: Option<Secret<String>>,

    // ── Exchange Rate Configuration ──
    /// CoinGecko Demo API Key (optional, improves rate limit)
    pub coingecko_api_key: Option<String>,
    /// Rate sync interval in seconds (default: 300 = 5 minutes)
    pub rate_sync_interval: u64,

    // ── Chain Configuration (loaded from chains.toml) ──
    /// Per-chain config keyed by network name (e.g. "TRON", "BSC")
    pub chains: HashMap<String, ChainTomlConfig>,
    /// Solana chain configuration (independent section in chains.toml)
    pub solana: Option<SolanaChainConfig>,

    // ── Helius Webhook (Solana event-driven indexing) ──
    /// Helius API key for webhook management (create/edit/delete)
    pub helius_api_key: Option<Secret<String>>,
    /// Shared secret for verifying incoming Helius webhook requests (Authorization header)
    pub helius_webhook_secret: Option<String>,
    /// Public URL where Helius should POST webhook events (e.g. https://api.ironixpay.com/webhooks/helius/solana)
    pub helius_webhook_url: Option<String>,

    // ── R2 Object Storage (Cloudflare R2, S3-compatible) ──
    /// R2 endpoint URL (e.g. https://<account_id>.r2.cloudflarestorage.com)
    pub r2_endpoint: Option<String>,
    /// R2 access key ID
    pub r2_access_key_id: Option<String>,
    /// R2 secret access key
    pub r2_secret_access_key: Option<Secret<String>>,
    /// R2 bucket name (e.g. ironixpay-assets)
    pub r2_bucket_name: Option<String>,
    /// R2 public URL for serving assets (e.g. https://assets.ironixpay.com)
    pub r2_public_url: Option<String>,

    // ── Xero Accounting Integration ──
    /// Xero OAuth 2.0 Client ID (OPTIONAL — Xero integration disabled if not set)
    pub xero_client_id: Option<String>,
    /// Xero OAuth 2.0 Client Secret
    pub xero_client_secret: Option<Secret<String>>,
    /// Xero OAuth redirect URI (must match Xero developer portal config)
    pub xero_redirect_uri: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        // dotenvy::dotenv() should be called in main.rs

        // Parse environment first - this determines defaults for other fields
        let env_str = env::var("ENVIRONMENT").unwrap_or_else(|_| "local".into());
        let environment = Environment::from_str(&env_str).expect("Failed to parse ENVIRONMENT");

        // Load chain config once (used for both TRON RPC resolution and the chains field)
        let chains_file = Self::load_chains_file();
        let chains = chains_file.chains;
        let solana = chains_file.solana;

        let turnstile_required = env::var("TURNSTILE_REQUIRED")
            .ok()
            .is_some_and(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes"));
        let turnstile_secret_key = env::var("TURNSTILE_SECRET_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(Secret::new);
        if turnstile_required && turnstile_secret_key.is_none() {
            panic!("TURNSTILE_SECRET_KEY must be set when TURNSTILE_REQUIRED=true");
        }

        let mut blocked_email_domains = vec!["emalupe.com".to_string()];
        if let Ok(domains) = env::var("BLOCKED_EMAIL_DOMAINS") {
            blocked_email_domains.extend(
                domains
                    .split(',')
                    .map(|domain| domain.trim().trim_start_matches('@').to_lowercase())
                    .filter(|domain| !domain.is_empty()),
            );
        }
        blocked_email_domains.sort_unstable();
        blocked_email_domains.dedup();

        let config = Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),

            database_url: Secret::new(env::var("DATABASE_URL").expect("DATABASE_URL must be set")),

            // Tron RPC: chains.toml TRON entry → TRON_FULL_NODE env var → environment default
            tron_full_node: chains
                .get("TRON")
                .and_then(|t| t.rpc_urls.first().cloned())
                .or_else(|| env::var("TRON_FULL_NODE").ok())
                .unwrap_or_else(|| environment.default_rpc_url().to_string()),
            tron_solidity_node: env::var("TRON_SOLIDITY_NODE")
                .unwrap_or_else(|_| environment.default_rpc_url().to_string()),

            // Network derived from environment
            network: environment.network().to_string(),

            address_pool_size: env::var("ADDRESS_POOL_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("ADDRESS_POOL_SIZE must be a number"),
            address_ttl_seconds: env::var("ADDRESS_TTL_SECONDS")
                .unwrap_or_else(|_| "1800".to_string())
                .parse()
                .expect("ADDRESS_TTL_SECONDS must be a number"),
            recycle_check_interval_seconds: env::var("RECYCLE_CHECK_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .expect("RECYCLE_CHECK_INTERVAL_SECONDS must be a number"),

            session_expiry_minutes: env::var("SESSION_EXPIRY_MINUTES")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("SESSION_EXPIRY_MINUTES must be a number"),

            webhook_retry_max: env::var("WEBHOOK_RETRY_MAX")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .expect("WEBHOOK_RETRY_MAX must be a number"),
            webhook_timeout_seconds: env::var("WEBHOOK_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("WEBHOOK_TIMEOUT_SECONDS must be a number"),

            jwt_secret: Secret::new(
                env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "change-this-secret-in-production".to_string()),
            ),
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .expect("JWT_EXPIRY_HOURS must be a number"),

            netts_api_key: env::var("NETTS_API_KEY").ok().map(Secret::new),
            sweep_threshold_ratio: env::var("SWEEP_THRESHOLD_RATIO")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2.0),

            rate_limit_per_minute: env::var("RATE_LIMIT_PER_MINUTE")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("RATE_LIMIT_PER_MINUTE must be a number"),

            turnstile_required,
            turnstile_secret_key,
            turnstile_expected_hostname: env::var("TURNSTILE_EXPECTED_HOSTNAME")
                .unwrap_or_else(|_| "app.ironixpay.com".to_string())
                .trim()
                .to_lowercase(),
            blocked_email_domains,

            master_mnemonic: env::var("MASTER_MNEMONIC").ok().map(Secret::new),

            // AWS KMS envelope encryption (Production)
            aws_kms_key_id: env::var("AWS_KMS_KEY_ID").ok(),
            encrypted_seed: env::var("ENCRYPTED_SEED").ok().map(Secret::new),

            encryption_key: Self::load_encryption_key(),

            sweep_confirmation_blocks: env::var("SWEEP_CONFIRMATION_BLOCKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(19),
            sweep_energy_estimate: env::var("SWEEP_ENERGY_ESTIMATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(65_000),

            checkout_base_url: env::var("CHECKOUT_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string()),

            dashboard_base_url: env::var("DASHBOARD_BASE_URL").unwrap_or_else(|_| {
                env::var("CHECKOUT_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:3001".to_string())
            }),

            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .map(|s| {
                    s.split(',')
                        .map(|o| o.trim().to_string())
                        .filter(|o| !o.is_empty())
                        .collect()
                })
                .unwrap_or_default(),

            environment: environment.clone(),

            database_max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| match environment {
                    Environment::Production => 70, // 8 chains × (indexer+sweeper) + API headroom (PG max=120)
                    Environment::Sandbox => 30,    // Same chain count, lower traffic
                }),

            admin_token: env::var("ADMIN_TOKEN").ok().map(Secret::new),

            trongrid_api_key: env::var("TRONGRID_API_KEY").ok().filter(|s| !s.is_empty()),

            treasury_low_balance_threshold: env::var("TREASURY_LOW_BALANCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000_000), // 100 USDT

            production_database_url: env::var("PRODUCTION_DATABASE_URL").ok().map(Secret::new),

            coingecko_api_key: env::var("COINGECKO_API_KEY").ok().filter(|s| !s.is_empty()),
            rate_sync_interval: env::var("RATE_SYNC_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300), // 5 minutes

            chains,
            solana,

            helius_api_key: env::var("HELIUS_API_KEY").ok().map(Secret::new),
            helius_webhook_secret: env::var("HELIUS_WEBHOOK_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
            helius_webhook_url: env::var("HELIUS_WEBHOOK_URL")
                .ok()
                .filter(|s| !s.is_empty()),

            r2_endpoint: env::var("R2_ENDPOINT").ok().filter(|s| !s.is_empty()),
            r2_access_key_id: env::var("R2_ACCESS_KEY_ID").ok().filter(|s| !s.is_empty()),
            r2_secret_access_key: env::var("R2_SECRET_ACCESS_KEY").ok().map(Secret::new),
            r2_bucket_name: env::var("R2_BUCKET_NAME").ok().filter(|s| !s.is_empty()),
            r2_public_url: env::var("R2_PUBLIC_URL").ok().filter(|s| !s.is_empty()),

            xero_client_id: env::var("XERO_CLIENT_ID").ok().filter(|s| !s.is_empty()),
            xero_client_secret: env::var("XERO_CLIENT_SECRET").ok().map(Secret::new),
            xero_redirect_uri: env::var("XERO_REDIRECT_URI").ok().filter(|s| !s.is_empty()),
        };

        config.validate();
        config
    }

    /// Load chain configuration from chains.toml file.
    ///
    /// Search order:
    /// 1. `CHAINS_CONFIG_PATH` env var (explicit path)
    /// 2. `./chains.toml` (working directory)
    ///
    /// Returns default ChainsFile if no config file found (backward-compatible: TRON-only mode).
    fn load_chains_file() -> ChainsFile {
        let path = env::var("CHAINS_CONFIG_PATH").unwrap_or_else(|_| "chains.toml".to_string());

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(path = %path, "No chains.toml found — running in TRON-only mode");
                return ChainsFile {
                    chains: HashMap::new(),
                    solana: None,
                };
            }
            Err(e) => {
                panic!(
                    "CRITICAL: Failed to read chains config at '{}': {}",
                    path, e
                );
            }
        };

        let chains_file: ChainsFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("CRITICAL: Failed to parse '{}': {}", path, e));

        tracing::info!(
            path = %path,
            chains = ?chains_file.chains.keys().collect::<Vec<_>>(),
            has_solana = chains_file.solana.is_some(),
            "Loaded chain configuration"
        );

        if let Some(ref sol) = chains_file.solana {
            tracing::info!(
                rpc_count = sol.rpc_urls.len(),
                "Solana configuration loaded (token config in ChainConfig)"
            );
        }

        chains_file
    }

    /// Validate configuration for safety
    fn validate(&self) {
        let has_kms = self.aws_kms_key_id.is_some();
        // Master key: must have mnemonic OR encrypted seed
        if self.master_mnemonic.is_none() && !(has_kms && self.encrypted_seed.is_some()) {
            panic!("CRITICAL: MASTER_MNEMONIC or (AWS_KMS_KEY_ID + ENCRYPTED_SEED) must be set");
        }

        if self.environment == Environment::Production {
            if self.netts_api_key.is_none() {
                panic!("CRITICAL: NETTS_API_KEY must be set in Production for energy delegation");
            }

            if self.jwt_secret.expose_secret() == "change-this-secret-in-production" {
                panic!("CRITICAL: Default JWT_SECRET is not allowed in Production!");
            }

            // check if encryption key is the default one
            let default_key = "00".repeat(32);
            if self.encryption_key.expose_secret() == &default_key {
                panic!("CRITICAL: Default ENCRYPTION_KEY is not allowed in Production!");
            }

            // In production, strongly recommend KMS
            if !has_kms {
                tracing::warn!("⚠️ Production without KMS: master mnemonic is in plaintext .env");
            }
        }
    }

    /// Whether to use energy delegation (Production) or TRX transfers (Sandbox)
    pub fn use_energy_delegation(&self) -> bool {
        self.environment.use_energy_delegation()
    }

    /// Load 32-byte encryption key from environment variable
    fn load_encryption_key() -> Secret<String> {
        let hex_key = env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
            // Default key for development only - CHANGE IN PRODUCTION
            "00".repeat(32)
        });

        // 校验一下长度，确保它是有效的 Hex 格式，但保持它是 String
        if hex_key.len() != 64 {
            panic!(
                "ENCRYPTION_KEY must be exactly 64 hex characters (32 bytes). Found {} chars.",
                hex_key.len()
            );
        }

        // 尝试 decode 一下验证内容合法性，但最后还是返回 Secret<String>
        if let Err(e) = hex::decode(&hex_key) {
            panic!("ENCRYPTION_KEY contains invalid hex characters: {}", e);
        }

        // 包装进 Secret
        Secret::new(hex_key)
    }
}
