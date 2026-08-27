//! Network and Environment Enums
//!
//! Defines the supported blockchain networks for environment isolation.
//!
//! Key Insight: Network ⊇ Environment
//! - TRON_MAINNET → Production
//! - TRON_NILE → Sandbox
//! - BSC_MAINNET → Production
//! - BSC_TESTNET → Sandbox
//! - ETHEREUM_MAINNET → Production
//! - ETHEREUM_SEPOLIA → Sandbox
//! - POLYGON_MAINNET → Production
//! - POLYGON_AMOY → Sandbox
//! - ARBITRUM_MAINNET → Production
//! - ARBITRUM_SEPOLIA → Sandbox
//! - BASE_MAINNET → Production
//! - BASE_SEPOLIA → Sandbox
//! - OPTIMISM_MAINNET → Production
//! - OPTIMISM_SEPOLIA → Sandbox
//! - SOLANA_MAINNET → Production
//! - SOLANA_DEVNET → Sandbox
//!
//! For chain-related tables (transactions, addresses), use Network.
//! For configuration tables (webhook_endpoints), use Environment.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// Logical environment (not chain-specific)
///
/// Used for configuration tables like `webhook_endpoints` where merchants
/// typically have only 2 URLs (dev/prod), not per-chain URLs.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub enum Environment {
    #[sea_orm(string_value = "production")]
    #[serde(rename = "production")]
    Production,
    #[sea_orm(string_value = "sandbox")]
    #[serde(rename = "sandbox")]
    Sandbox,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Production => "production",
            Environment::Sandbox => "sandbox",
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Chain family grouping (determines shared crypto primitives).
///
/// Networks within the same family share:
/// - HD derivation coin_type (BIP44 for Tron/EVM, SLIP-0010 for Solana)
/// - Signing algorithm (secp256k1 for Tron/Evm, Ed25519 for Solana)
/// - Address format
///
/// EVM addresses are **universal** across all EVM-compatible chains.
/// A single xpub at m/44'/60'/N' can derive addresses used on
/// Ethereum, BSC, Polygon, Arbitrum, Optimism, Base, etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChainFamily {
    /// TRON: Base58 addresses (T-prefix), coin_type=195, SHA-256 tx hashing
    Tron,
    /// EVM: Hex addresses (0x-prefix), coin_type=60, Keccak-256 tx hashing
    Evm,
    /// Solana: Base58 addresses, coin_type=501, Ed25519, SLIP-0010 derivation
    Solana,
}

/// Supported blockchain networks.
///
/// **Production** (`sk_live_`): All networks are supported.
/// **Sandbox** (`sk_test_`): Currently only `TRON` is supported (maps to TRON Nile testnet).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub enum Network {
    /// TRON Protocol
    #[sea_orm(string_value = "TRON")]
    #[serde(rename = "TRON", alias = "Tron", alias = "tron")]
    Tron,

    /// Binance Smart Chain (EVM-compatible)
    #[sea_orm(string_value = "BSC")]
    #[serde(rename = "BSC", alias = "Bsc", alias = "bsc")]
    Bsc,

    /// Ethereum Mainnet
    #[sea_orm(string_value = "ETHEREUM")]
    #[serde(rename = "ETHEREUM", alias = "Ethereum", alias = "ethereum")]
    Ethereum,

    /// Polygon PoS
    #[sea_orm(string_value = "POLYGON")]
    #[serde(rename = "POLYGON", alias = "Polygon", alias = "polygon")]
    Polygon,

    /// Arbitrum One (L2)
    #[sea_orm(string_value = "ARBITRUM")]
    #[serde(rename = "ARBITRUM", alias = "Arbitrum", alias = "arbitrum")]
    Arbitrum,

    /// Base (L2, Coinbase)
    #[sea_orm(string_value = "BASE")]
    #[serde(rename = "BASE", alias = "Base", alias = "base")]
    Base,

    /// Optimism (L2)
    #[sea_orm(string_value = "OPTIMISM")]
    #[serde(rename = "OPTIMISM", alias = "Optimism", alias = "optimism")]
    Optimism,

    /// Solana
    #[sea_orm(string_value = "SOLANA")]
    #[serde(rename = "SOLANA", alias = "Solana", alias = "solana")]
    Solana,
}

/// Chain-specific configuration resolved from (Network, Environment).
///
/// This is the **single source of truth** for contract addresses and
/// infrastructure URLs per network/environment combination.
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// USDT contract address for this network/environment
    pub usdt_contract: String,
    /// USDC contract address (None = not supported on this chain)
    pub usdc_contract: Option<String>,
    /// Energy provider URL (TRON mainnet only, testnet burns TRX directly)
    pub energy_provider_url: Option<String>,
    /// EVM chain ID (None for non-EVM chains like TRON)
    pub chain_id: Option<u64>,
    /// Native token symbol (TRX, BNB, ETH)
    pub native_symbol: &'static str,
    /// Token decimals for USDT on this chain
    pub usdt_decimals: u8,
    /// USDC decimals (None = not supported). All USDC = 6 decimals except BSC = 18.
    pub usdc_decimals: Option<u8>,
    /// Required confirmations before considering a transaction final
    pub confirmation_blocks: u32,
    /// Default indexer poll interval in seconds.
    pub poll_interval_secs: u64,
    /// Public RPC URL for frontend payment detection polling.
    pub detection_rpc_url: &'static str,
}

impl ChainConfig {
    /// Get contract address for the given token, or None if unsupported.
    pub fn token_contract(&self, token: &str) -> Option<&str> {
        match token {
            "USDT" => Some(&self.usdt_contract),
            "USDC" => self.usdc_contract.as_deref(),
            _ => None,
        }
    }

    /// Get decimals for the given token, or None if unsupported.
    pub fn token_decimals(&self, token: &str) -> Option<u8> {
        match token {
            "USDT" => Some(self.usdt_decimals),
            "USDC" => self.usdc_decimals,
            _ => None,
        }
    }

    /// Returns (contract_address, symbol) pairs for all supported tokens on this chain.
    pub fn supported_tokens(&self) -> Vec<(&str, &str)> {
        let mut tokens = vec![(&*self.usdt_contract, "USDT")];
        if let Some(ref usdc) = self.usdc_contract {
            tokens.push((usdc, "USDC"));
        }
        tokens
    }
}

impl Network {
    /// Get the chain family for this network.
    ///
    /// Networks in the same family share crypto primitives (key derivation,
    /// signing algorithm, address format). EVM chains share addresses.
    pub fn chain_family(&self) -> ChainFamily {
        match self {
            Network::Tron => ChainFamily::Tron,
            Network::Bsc
            | Network::Ethereum
            | Network::Polygon
            | Network::Arbitrum
            | Network::Base
            | Network::Optimism => ChainFamily::Evm,
            Network::Solana => ChainFamily::Solana,
        }
    }

    /// Validate a collection address for this network.
    ///
    /// Dispatches to the appropriate validator based on chain family:
    /// - TRON: Base58 with T-prefix, checksum verification
    /// - EVM (BSC/ETH): 0x-prefix, 40 hex characters
    pub fn validate_collection_address(&self, addr: &str) -> Result<(), String> {
        match self.chain_family() {
            ChainFamily::Tron => {
                crate::services::tron::address::validate_address(addr).map_err(|e| e.to_string())
            }
            ChainFamily::Evm => validate_evm_address(addr),
            ChainFamily::Solana => validate_solana_address(addr),
        }
    }

    /// BIP44 coin type for HD derivation.
    ///
    /// All EVM chains share coin_type=60, meaning a single xpub
    /// derives the same addresses across ETH/BSC/Polygon/Arbitrum/etc.
    pub fn coin_type(&self) -> u32 {
        match self.chain_family() {
            ChainFamily::Tron => 195,
            ChainFamily::Evm => 60,
            ChainFamily::Solana => 501,
        }
    }
    /// Human-readable network name for display, logging, and config.
    ///
    /// Returns environment-specific names like `"TRON_MAINNET"` or `"TRON_NILE"`.
    ///
    /// **⚠️ NOT for database queries** — use [`as_str()`](Self::as_str) for DB operations.
    /// The database stores environment-agnostic values (`"TRON"`, `"BSC"`) because
    /// environment isolation is handled at the database level (separate databases).
    pub fn display_name(&self, env: &Environment) -> &'static str {
        match (self, env) {
            (Network::Tron, Environment::Production) => "TRON_MAINNET",
            (Network::Tron, Environment::Sandbox) => "TRON_NILE",
            (Network::Bsc, Environment::Production) => "BSC_MAINNET",
            (Network::Bsc, Environment::Sandbox) => "BSC_TESTNET",
            (Network::Ethereum, Environment::Production) => "ETHEREUM_MAINNET",
            (Network::Ethereum, Environment::Sandbox) => "ETHEREUM_SEPOLIA",
            (Network::Polygon, Environment::Production) => "POLYGON_MAINNET",
            (Network::Polygon, Environment::Sandbox) => "POLYGON_AMOY",
            (Network::Arbitrum, Environment::Production) => "ARBITRUM_MAINNET",
            (Network::Arbitrum, Environment::Sandbox) => "ARBITRUM_SEPOLIA",
            (Network::Base, Environment::Production) => "BASE_MAINNET",
            (Network::Base, Environment::Sandbox) => "BASE_SEPOLIA",
            (Network::Optimism, Environment::Production) => "OPTIMISM_MAINNET",
            (Network::Optimism, Environment::Sandbox) => "OPTIMISM_SEPOLIA",
            (Network::Solana, Environment::Production) => "SOLANA_MAINNET",
            (Network::Solana, Environment::Sandbox) => "SOLANA_DEVNET",
        }
    }

    /// Database storage format — use this for all DB queries and inserts.
    ///
    /// Returns environment-agnostic values: `"TRON"`, `"BSC"`, `"ETHEREUM"`, etc.
    /// Environment context comes from database-level isolation (separate databases),
    /// not from the network string.
    ///
    /// For human-readable display (e.g. API responses, logs), use [`display_name()`](Self::display_name).
    pub fn as_str(&self) -> &'static str {
        match self {
            Network::Tron => "TRON",
            Network::Bsc => "BSC",
            Network::Ethereum => "ETHEREUM",
            Network::Polygon => "POLYGON",
            Network::Arbitrum => "ARBITRUM",
            Network::Base => "BASE",
            Network::Optimism => "OPTIMISM",
            Network::Solana => "SOLANA",
        }
    }

    /// Parse from string (lenient: case-insensitive, accepts legacy values)
    pub fn from_str_lenient(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TRON" | "TRON_MAINNET" | "TRON_NILE" | "TRON_SHASTA" => Some(Network::Tron),
            "BSC" | "BSC_MAINNET" | "BSC_TESTNET" | "BNB" => Some(Network::Bsc),
            "ETHEREUM" | "ETHEREUM_MAINNET" | "ETHEREUM_SEPOLIA" | "ETH" => Some(Network::Ethereum),
            "POLYGON" | "POLYGON_MAINNET" | "POLYGON_AMOY" | "MATIC" => Some(Network::Polygon),
            "ARBITRUM" | "ARBITRUM_MAINNET" | "ARBITRUM_SEPOLIA" | "ARB" => Some(Network::Arbitrum),
            "BASE" | "BASE_MAINNET" | "BASE_SEPOLIA" => Some(Network::Base),
            "OPTIMISM" | "OPTIMISM_MAINNET" | "OPTIMISM_SEPOLIA" | "OP" => Some(Network::Optimism),
            "SOLANA" | "SOLANA_MAINNET" | "SOLANA_DEVNET" | "SOL" => Some(Network::Solana),
            _ => None,
        }
    }

    /// Infer default network from API key prefix.
    ///
    /// **TRON-only legacy fallback.** Do not use for multi-chain routing.
    /// Future: default network should come from API Key table, not prefix.
    #[deprecated(note = "TRON-only assumption; use merchant's configured network instead")]
    pub fn default_from_prefix(key_prefix: &str) -> Option<Self> {
        if key_prefix.starts_with("sk_live_") || key_prefix.starts_with("sk_test_") {
            Some(Network::Tron)
        } else {
            None
        }
    }

    /// Parse network string into (Network, Environment) tuple
    pub fn parse_string(s: &str) -> Option<(Self, Environment)> {
        match s.to_uppercase().as_str() {
            "TRON_MAINNET" => Some((Network::Tron, Environment::Production)),
            "TRON_NILE" => Some((Network::Tron, Environment::Sandbox)),
            "TRON" => Some((Network::Tron, Environment::Production)),
            "BSC_MAINNET" => Some((Network::Bsc, Environment::Production)),
            "BSC_TESTNET" => Some((Network::Bsc, Environment::Sandbox)),
            "BSC" => Some((Network::Bsc, Environment::Production)),
            "ETHEREUM_MAINNET" => Some((Network::Ethereum, Environment::Production)),
            "ETHEREUM_SEPOLIA" => Some((Network::Ethereum, Environment::Sandbox)),
            "ETHEREUM" => Some((Network::Ethereum, Environment::Production)),
            "POLYGON_MAINNET" => Some((Network::Polygon, Environment::Production)),
            "POLYGON_AMOY" => Some((Network::Polygon, Environment::Sandbox)),
            "POLYGON" => Some((Network::Polygon, Environment::Production)),
            "ARBITRUM_MAINNET" => Some((Network::Arbitrum, Environment::Production)),
            "ARBITRUM_SEPOLIA" => Some((Network::Arbitrum, Environment::Sandbox)),
            "ARBITRUM" => Some((Network::Arbitrum, Environment::Production)),
            "BASE_MAINNET" => Some((Network::Base, Environment::Production)),
            "BASE_SEPOLIA" => Some((Network::Base, Environment::Sandbox)),
            "BASE" => Some((Network::Base, Environment::Production)),
            "OPTIMISM_MAINNET" => Some((Network::Optimism, Environment::Production)),
            "OPTIMISM_SEPOLIA" => Some((Network::Optimism, Environment::Sandbox)),
            "OPTIMISM" => Some((Network::Optimism, Environment::Production)),
            "SOLANA_MAINNET" => Some((Network::Solana, Environment::Production)),
            "SOLANA_DEVNET" => Some((Network::Solana, Environment::Sandbox)),
            "SOLANA" => Some((Network::Solana, Environment::Production)),
            _ => None,
        }
    }

    /// Determine if a stored network string represents a production (live) environment.
    ///
    /// **DEPRECATED** after network normalization: DB stores pure chain names ("TRON"),
    /// so this always returns true. Use `is_livemode_env` instead.
    #[deprecated(note = "Use is_livemode_env() after network normalization")]
    pub fn is_livemode(network_str: &str) -> bool {
        matches!(
            network_str.to_uppercase().as_str(),
            "TRON_MAINNET"
                | "TRON"
                | "BSC_MAINNET"
                | "BSC"
                | "ETHEREUM_MAINNET"
                | "ETHEREUM"
                | "POLYGON_MAINNET"
                | "POLYGON"
                | "ARBITRUM_MAINNET"
                | "ARBITRUM"
                | "BASE_MAINNET"
                | "BASE"
                | "OPTIMISM_MAINNET"
                | "OPTIMISM"
                | "SOLANA_MAINNET"
                | "SOLANA"
        )
    }

    /// Determine if the current process environment is production (live).
    ///
    /// Used to populate the `livemode` field in API responses and webhook payloads.
    /// Since each backend process serves exactly one environment, this is derived
    /// from the process's `Environment`, not the network string.
    pub fn is_livemode_env(env: &Environment) -> bool {
        matches!(env, Environment::Production)
    }

    /// Canonical chain configuration for this network + environment.
    ///
    /// **SINGLE SOURCE OF TRUTH** for contract addresses.
    /// All services derive their USDT contract address from here at startup.
    pub fn chain_config(&self, env: &Environment) -> ChainConfig {
        match self {
            Network::Tron => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string(),
                    usdc_contract: None, // TRON USDC discontinued by Circle
                    energy_provider_url: Some("https://api.feee.io".into()),
                    chain_id: None,
                    native_symbol: "TRX",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 19,
                    poll_interval_secs: 3,
                    detection_rpc_url: "https://api.trongrid.io",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: None,
                    native_symbol: "TRX",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 19,
                    poll_interval_secs: 3,
                    detection_rpc_url: "https://nile.trongrid.io",
                },
            },
            Network::Bsc => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0x55d398326f99059fF775485246999027B3197955".to_string(),
                    usdc_contract: Some("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(56),
                    native_symbol: "BNB",
                    usdt_decimals: 18,
                    usdc_decimals: Some(18),
                    confirmation_blocks: 15,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc.ankr.com/bsc",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(97),
                    native_symbol: "tBNB",
                    usdt_decimals: 18,
                    usdc_decimals: None,
                    confirmation_blocks: 15,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://data-seed-prebsc-1-s1.binance.org:8545",
                },
            },
            Network::Ethereum => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
                    usdc_contract: Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(1),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 12,
                    poll_interval_secs: 30,
                    detection_rpc_url: "https://rpc.ankr.com/eth",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(11155111),
                    native_symbol: "SepoliaETH",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 12,
                    poll_interval_secs: 30,
                    detection_rpc_url: "https://rpc.sepolia.org",
                },
            },
            Network::Polygon => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string(),
                    usdc_contract: Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(137),
                    native_symbol: "POL",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 128,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc.ankr.com/polygon",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(80002),
                    native_symbol: "POL",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 128,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc-amoy.polygon.technology",
                },
            },
            Network::Arbitrum => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string(),
                    usdc_contract: Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(42161),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 40,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc.ankr.com/arbitrum",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(421614),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 40,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://sepolia-rollup.arbitrum.io/rpc",
                },
            },
            Network::Base => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2".to_string(),
                    usdc_contract: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(8453),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 20,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc.ankr.com/base",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(84532),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 20,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://sepolia.base.org",
                },
            },
            Network::Optimism => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string(),
                    usdc_contract: Some("0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85".to_string()),
                    energy_provider_url: None,
                    chain_id: Some(10),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 20,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://rpc.ankr.com/optimism",
                },
                Environment::Sandbox => ChainConfig {
                    usdt_contract: "0x0000000000000000000000000000000000000000".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: Some(11155420),
                    native_symbol: "ETH",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 20,
                    poll_interval_secs: 15,
                    detection_rpc_url: "https://sepolia.optimism.io",
                },
            },
            Network::Solana => match env {
                Environment::Production => ChainConfig {
                    usdt_contract: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
                    usdc_contract: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
                    energy_provider_url: None,
                    chain_id: None, // Solana doesn't use chain IDs
                    native_symbol: "SOL",
                    usdt_decimals: 6,
                    usdc_decimals: Some(6),
                    confirmation_blocks: 31, // ~confirmed commitment
                    poll_interval_secs: 5,
                    detection_rpc_url: "https://api.mainnet-beta.solana.com",
                },
                Environment::Sandbox => ChainConfig {
                    // Custom test USDT token on Solana Devnet (deployed for E2E testing)
                    usdt_contract: "EhxoqTX5wKBNKfp2UsXBwDAe1XzAhtNz3HBNJfFVh4ah".to_string(),
                    usdc_contract: None,
                    energy_provider_url: None,
                    chain_id: None,
                    native_symbol: "SOL",
                    usdt_decimals: 6,
                    usdc_decimals: None,
                    confirmation_blocks: 31,
                    poll_interval_secs: 5,
                    detection_rpc_url: "https://api.devnet.solana.com",
                },
            },
        }
    }

    /// Static task name for indexer supervisor loop.
    ///
    /// Returns a `&'static str` to avoid `Box::leak` for dynamic string allocation.
    pub fn indexer_task_name(&self) -> &'static str {
        match self {
            Network::Tron => "TRON Indexer",
            Network::Bsc => "BSC Indexer",
            Network::Ethereum => "ETHEREUM Indexer",
            Network::Polygon => "POLYGON Indexer",
            Network::Arbitrum => "ARBITRUM Indexer",
            Network::Base => "BASE Indexer",
            Network::Optimism => "OPTIMISM Indexer",
            Network::Solana => "SOLANA Indexer",
        }
    }

    /// Static task name for sweeper supervisor loop.
    ///
    /// Returns a `&'static str` to avoid `Box::leak` for dynamic string allocation.
    pub fn sweeper_task_name(&self) -> &'static str {
        match self {
            Network::Tron => "TRON Sweeper",
            Network::Bsc => "BSC Sweeper",
            Network::Ethereum => "ETHEREUM Sweeper",
            Network::Polygon => "POLYGON Sweeper",
            Network::Arbitrum => "ARBITRUM Sweeper",
            Network::Base => "BASE Sweeper",
            Network::Optimism => "OPTIMISM Sweeper",
            Network::Solana => "SOLANA Sweeper",
        }
    }
}

/// Validate an EVM address (0x-prefix + 40 hex characters).
///
/// Checks:
/// 1. Non-empty
/// 2. Starts with "0x" or "0X"
/// 3. Total length is 42 characters
/// 4. Remaining 40 characters are valid hex
fn validate_evm_address(addr: &str) -> Result<(), String> {
    if addr.is_empty() {
        return Err("Address is empty".to_string());
    }

    if !addr.starts_with("0x") && !addr.starts_with("0X") {
        return Err(format!(
            "EVM address must start with '0x', got '{}'",
            &addr[..2.min(addr.len())]
        ));
    }

    if addr.len() != 42 {
        return Err(format!(
            "EVM address must be 42 characters (0x + 40 hex), got {}",
            addr.len()
        ));
    }

    // Validate hex characters (after 0x prefix)
    let hex_part = &addr[2..];
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("EVM address contains invalid hex characters".to_string());
    }

    // Reject zero address
    if hex_part.chars().all(|c| c == '0') {
        return Err("Zero address (0x000...000) is not allowed".to_string());
    }

    Ok(())
}

/// Validate a Solana address (Base58-encoded Ed25519 public key, 32 bytes).
///
/// Checks:
/// 1. Non-empty
/// 2. Valid Base58 encoding
/// 3. Decodes to exactly 32 bytes
/// 4. Not the all-zero public key
pub fn validate_solana_address(addr: &str) -> Result<(), String> {
    if addr.is_empty() {
        return Err("Address is empty".to_string());
    }

    // Solana Base58 addresses are between 32 and 44 characters.
    // This rejects EVM (42 chars, starts with 0x) and helps disambiguate from
    // TRON (34 chars, starts with 'T', decodes to 25 bytes).
    if addr.len() < 32 || addr.len() > 44 {
        return Err(format!(
            "Solana address must be 32-44 characters, got {}",
            addr.len()
        ));
    }

    let decoded = bs58::decode(addr)
        .into_vec()
        .map_err(|e| format!("Invalid Base58 encoding: {}", e))?;

    // Solana pubkeys are 32 bytes. bs58 into_vec() strips leading zero bytes,
    // so short addresses (with leading 0x00 bytes) may decode to < 32 bytes.
    // We accept ≤ 32 and left-pad; reject > 32.
    if decoded.len() > 32 {
        return Err(format!(
            "Solana address must decode to at most 32 bytes, got {}",
            decoded.len()
        ));
    }

    // TRON addresses decode to 25 bytes (1 prefix + 20 addr + 4 checksum).
    // Reject anything that's clearly not a 32-byte pubkey.
    if decoded.len() < 30 {
        return Err(format!(
            "Decoded address too short for Solana pubkey: {} bytes (expected ~32)",
            decoded.len()
        ));
    }

    // Left-pad to 32 bytes for zero-check
    let mut pubkey = [0u8; 32];
    let offset = 32 - decoded.len();
    pubkey[offset..].copy_from_slice(&decoded);

    // Reject all-zero pubkey
    if pubkey.iter().all(|&b| b == 0) {
        return Err("Zero public key is not allowed".to_string());
    }

    Ok(())
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for Network {
    fn default() -> Self {
        Network::Tron
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_name_mapping() {
        assert_eq!(
            Network::Tron.display_name(&Environment::Production),
            "TRON_MAINNET"
        );
        assert_eq!(
            Network::Tron.display_name(&Environment::Sandbox),
            "TRON_NILE"
        );
        assert_eq!(
            Network::Bsc.display_name(&Environment::Production),
            "BSC_MAINNET"
        );
        assert_eq!(
            Network::Bsc.display_name(&Environment::Sandbox),
            "BSC_TESTNET"
        );
        assert_eq!(
            Network::Ethereum.display_name(&Environment::Production),
            "ETHEREUM_MAINNET"
        );
        assert_eq!(
            Network::Ethereum.display_name(&Environment::Sandbox),
            "ETHEREUM_SEPOLIA"
        );
    }

    #[test]
    fn test_chain_family() {
        assert_eq!(Network::Tron.chain_family(), ChainFamily::Tron);
        assert_eq!(Network::Bsc.chain_family(), ChainFamily::Evm);
        assert_eq!(Network::Ethereum.chain_family(), ChainFamily::Evm);
        assert_eq!(Network::Solana.chain_family(), ChainFamily::Solana);
    }

    #[test]
    fn test_coin_type() {
        assert_eq!(Network::Tron.coin_type(), 195);
        assert_eq!(Network::Bsc.coin_type(), 60);
        assert_eq!(Network::Ethereum.coin_type(), 60);
        assert_eq!(Network::Solana.coin_type(), 501);
        // All EVM chains share coin_type — critical for address reuse
        assert_eq!(Network::Bsc.coin_type(), Network::Ethereum.coin_type());
        // Solana uses a distinct coin_type
        assert_ne!(Network::Solana.coin_type(), Network::Ethereum.coin_type());
    }

    #[test]
    #[allow(deprecated)]
    fn test_default_from_prefix() {
        assert_eq!(
            Network::default_from_prefix("sk_live_abc123"),
            Some(Network::Tron)
        );
        assert_eq!(
            Network::default_from_prefix("sk_test_xyz789"),
            Some(Network::Tron)
        );
        assert_eq!(Network::default_from_prefix("invalid_key"), None);
    }

    #[test]
    fn test_from_str_lenient_case_insensitive() {
        assert_eq!(Network::from_str_lenient("TRON"), Some(Network::Tron));
        assert_eq!(Network::from_str_lenient("tron"), Some(Network::Tron));
        assert_eq!(
            Network::from_str_lenient("TRON_MAINNET"),
            Some(Network::Tron)
        );
        assert_eq!(Network::from_str_lenient("Tron_Nile"), Some(Network::Tron));
        // New chains
        assert_eq!(Network::from_str_lenient("BSC"), Some(Network::Bsc));
        assert_eq!(Network::from_str_lenient("BSC_MAINNET"), Some(Network::Bsc));
        assert_eq!(Network::from_str_lenient("BNB"), Some(Network::Bsc));
        assert_eq!(
            Network::from_str_lenient("ETHEREUM"),
            Some(Network::Ethereum)
        );
        assert_eq!(Network::from_str_lenient("ETH"), Some(Network::Ethereum));
        assert_eq!(
            Network::from_str_lenient("ETHEREUM_SEPOLIA"),
            Some(Network::Ethereum)
        );
        // Solana aliases
        assert_eq!(Network::from_str_lenient("SOLANA"), Some(Network::Solana));
        assert_eq!(Network::from_str_lenient("solana"), Some(Network::Solana));
        assert_eq!(Network::from_str_lenient("SOL"), Some(Network::Solana));
        assert_eq!(
            Network::from_str_lenient("SOLANA_MAINNET"),
            Some(Network::Solana)
        );
        assert_eq!(
            Network::from_str_lenient("SOLANA_DEVNET"),
            Some(Network::Solana)
        );
    }

    #[test]
    fn test_parse_string_bsc() {
        assert_eq!(
            Network::parse_string("BSC_MAINNET"),
            Some((Network::Bsc, Environment::Production))
        );
        assert_eq!(
            Network::parse_string("BSC_TESTNET"),
            Some((Network::Bsc, Environment::Sandbox))
        );
    }

    #[test]
    #[allow(deprecated)]
    fn test_is_livemode() {
        assert!(Network::is_livemode("TRON_MAINNET"));
        assert!(Network::is_livemode("BSC_MAINNET"));
        assert!(Network::is_livemode("ETHEREUM_MAINNET"));
        assert!(Network::is_livemode("SOLANA_MAINNET"));
        assert!(Network::is_livemode("SOLANA"));
        assert!(!Network::is_livemode("TRON_NILE"));
        assert!(!Network::is_livemode("BSC_TESTNET"));
        assert!(!Network::is_livemode("ETHEREUM_SEPOLIA"));
        assert!(!Network::is_livemode("SOLANA_DEVNET"));
    }

    #[test]
    fn test_chain_config_usdt_decimals() {
        // TRON USDT: 6 decimals
        let tron = Network::Tron.chain_config(&Environment::Production);
        assert_eq!(tron.usdt_decimals, 6);
        assert!(tron.chain_id.is_none());

        // BSC USDT: 18 decimals (!)
        let bsc = Network::Bsc.chain_config(&Environment::Production);
        assert_eq!(bsc.usdt_decimals, 18);
        assert_eq!(bsc.chain_id, Some(56));
        assert_eq!(bsc.native_symbol, "BNB");

        // Ethereum USDT: 6 decimals
        let eth = Network::Ethereum.chain_config(&Environment::Production);
        assert_eq!(eth.usdt_decimals, 6);
        assert_eq!(eth.chain_id, Some(1));

        // Solana USDT: 6 decimals, no chain_id
        let sol = Network::Solana.chain_config(&Environment::Production);
        assert_eq!(sol.usdt_decimals, 6);
        assert!(sol.chain_id.is_none());
        assert_eq!(sol.native_symbol, "SOL");
        assert_eq!(
            sol.usdt_contract,
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
        );
        assert!(sol.usdc_contract.is_some());
    }

    #[test]
    fn test_validate_evm_address() {
        // Valid EVM address
        assert!(validate_evm_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B").is_ok());
        // Lowercase valid
        assert!(validate_evm_address("0xab5801a7d398351b8be11c439e05c5b3259aec9b").is_ok());
        // Mixed case valid
        assert!(validate_evm_address("0xdAC17F958D2ee523a2206206994597C13D831ec7").is_ok());

        // Empty
        assert!(validate_evm_address("").is_err());
        // Missing 0x prefix
        assert!(validate_evm_address("Ab5801a7D398351b8bE11C439e05C5B3259aeC9B").is_err());
        // Too short
        assert!(validate_evm_address("0xAb5801a7D398351b8bE11C4").is_err());
        // Too long
        assert!(validate_evm_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9Bff").is_err());
        // Invalid hex chars
        assert!(validate_evm_address("0xGb5801a7D398351b8bE11C439e05C5B3259aeC9B").is_err());
        // Zero address
        assert!(validate_evm_address("0x0000000000000000000000000000000000000000").is_err());
        // TRON address (wrong format)
        assert!(validate_evm_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9").is_err());
    }

    #[test]
    fn test_validate_collection_address_dispatch() {
        // TRON network accepts T-prefix Base58 address
        assert!(Network::Tron
            .validate_collection_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9")
            .is_ok());
        // TRON network rejects 0x address
        assert!(Network::Tron
            .validate_collection_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B")
            .is_err());

        // BSC network accepts 0x address
        assert!(Network::Bsc
            .validate_collection_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B")
            .is_ok());
        // BSC network rejects TRON address
        assert!(Network::Bsc
            .validate_collection_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9")
            .is_err());

        // Ethereum also uses EVM validation
        assert!(Network::Ethereum
            .validate_collection_address("0xdAC17F958D2ee523a2206206994597C13D831ec7")
            .is_ok());

        // Solana network accepts Base58 Ed25519 pubkey (USDT mint address)
        assert!(Network::Solana
            .validate_collection_address("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")
            .is_ok());
        // Solana rejects EVM address
        assert!(Network::Solana
            .validate_collection_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B")
            .is_err());
        // Solana rejects TRON address (different Base58 format, wrong length)
        assert!(Network::Solana
            .validate_collection_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9")
            .is_err());
    }

    #[test]
    fn test_validate_solana_address() {
        // Valid Solana addresses (32-byte Ed25519 pubkeys)
        // USDT mint address
        assert!(validate_solana_address("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").is_ok());
        // USDC mint address
        assert!(validate_solana_address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").is_ok());
        // A typical Solana wallet address
        assert!(validate_solana_address("7EcDhSYGxXyscszYEp35KHN8vvw3svAuLKTzXwCFLtV").is_ok());

        // Empty
        assert!(validate_solana_address("").is_err());
        // Invalid Base58 (contains '0', 'O', 'I', 'l' — not valid Base58 chars)
        assert!(validate_solana_address("0x123InvalidBase58").is_err());
        // Too short (valid Base58 but decodes to < 32 bytes)
        assert!(validate_solana_address("1").is_err());
        // All-zero public key (system program)
        assert!(validate_solana_address("11111111111111111111111111111111").is_err());
    }

    #[test]
    fn test_parse_string_solana() {
        assert_eq!(
            Network::parse_string("SOLANA_MAINNET"),
            Some((Network::Solana, Environment::Production))
        );
        assert_eq!(
            Network::parse_string("SOLANA_DEVNET"),
            Some((Network::Solana, Environment::Sandbox))
        );
        assert_eq!(
            Network::parse_string("SOLANA"),
            Some((Network::Solana, Environment::Production))
        );
    }

    #[test]
    fn test_display_name_solana() {
        assert_eq!(
            Network::Solana.display_name(&Environment::Production),
            "SOLANA_MAINNET"
        );
        assert_eq!(
            Network::Solana.display_name(&Environment::Sandbox),
            "SOLANA_DEVNET"
        );
    }
}
