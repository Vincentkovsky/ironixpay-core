//! HD Wallet Module for Multi-Chain Address Derivation
//!
//! Implements BIP32 hierarchical deterministic wallet for TRON and EVM addresses.
//! Security model: Account-level xpub is stored per merchant, allowing
//! derivation of payment addresses without exposing master private key.
//!
//! # Coin Types (BIP44)
//! - TRON: `m/44'/195'/...` → Base58 address (T-prefix)
//! - EVM:  `m/44'/60'/...`  → Hex address (0x-prefix, EIP-55 checksum)
//!
//! # Key Insight: EVM Address Universality
//! All EVM chains (Ethereum, BSC, Polygon, Arbitrum, ...) share coin_type=60.
//! A single xpub at `m/44'/60'/N'` derives identical addresses across all EVM chains.

use crate::entity::network::{ChainFamily, Network};
use anyhow::{anyhow, Result};
use bip32::{ChildNumber, XPub};
use sha3::{Digest, Keccak256};
use std::str::FromStr;

// ─── Public API: Network-Aware Dispatch ─────────────────────────────────────

/// Derive an address from Account-level xpub for the given network.
///
/// Dispatches to `derive_tron_address` or `derive_evm_address` based on
/// the network's chain family.
pub fn derive_address(account_xpub: &str, path_index: u32, network: Network) -> Result<String> {
    match network.chain_family() {
        ChainFamily::Tron => derive_tron_address(account_xpub, path_index),
        ChainFamily::Evm => derive_evm_address(account_xpub, path_index),
        ChainFamily::Solana => Err(anyhow::anyhow!(
            "Solana uses Ed25519/SLIP-0010 key derivation — not compatible with BIP32 xpub. \
             Use SolanaKeyManager instead."
        )),
    }
}

// ─── TRON Address Derivation ────────────────────────────────────────────────

/// Derive a TRON address from Account-level extended public key
///
/// # Path Structure
/// Given Account xpub at: m/44'/195'/{account_index}'
/// Derives child at: /0/{path_index}
///
/// # Returns
/// TRON mainnet address (starts with 'T', 34 characters)
pub fn derive_tron_address(account_xpub: &str, path_index: u32) -> Result<String> {
    let pubkey_bytes = derive_child_pubkey_uncompressed(account_xpub, path_index)?;

    // Hash the 64 bytes (excluding 0x04 prefix) with Keccak256
    let mut hasher = Keccak256::new();
    hasher.update(&pubkey_bytes[1..65]);
    let hash = hasher.finalize();

    // Take last 20 bytes
    let address_bytes = &hash[12..32];

    // Prepend TRON mainnet prefix (0x41)
    let mut addr_with_prefix = vec![0x41];
    addr_with_prefix.extend_from_slice(address_bytes);

    // Base58Check encode
    let tron_address = bs58::encode(&addr_with_prefix).with_check().into_string();

    Ok(tron_address)
}

// ─── EVM Address Derivation ─────────────────────────────────────────────────

/// Derive an EVM address from Account-level extended public key.
///
/// # Path Structure
/// Given Account xpub at: m/44'/60'/{account_index}'
/// Derives child at: /0/{path_index}
///
/// # Returns
/// EVM address with EIP-55 mixed-case checksum (e.g., "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B")
///
/// # Address Derivation Steps (per Ethereum Yellow Paper)
/// 1. Derive child public key (uncompressed, 65 bytes)
/// 2. Drop the 0x04 prefix → 64 bytes
/// 3. Keccak-256 hash → 32 bytes
/// 4. Take last 20 bytes → raw address
/// 5. Apply EIP-55 checksum (via alloy_primitives::Address)
pub fn derive_evm_address(account_xpub: &str, path_index: u32) -> Result<String> {
    let pubkey_bytes = derive_child_pubkey_uncompressed(account_xpub, path_index)?;

    // Hash the 64 bytes (excluding 0x04 prefix) with Keccak256
    let mut hasher = Keccak256::new();
    hasher.update(&pubkey_bytes[1..65]);
    let hash = hasher.finalize();

    // Take last 20 bytes
    let mut address_bytes = [0u8; 20];
    address_bytes.copy_from_slice(&hash[12..32]);

    // Use alloy_primitives for EIP-55 checksum encoding
    let address = alloy_primitives::Address::from(address_bytes);
    Ok(address.to_checksum(None))
}

// ─── Shared Helper ──────────────────────────────────────────────────────────

/// Derive child public key in uncompressed format (65 bytes: 0x04 || x || y).
///
/// Used by both `derive_tron_address` and `derive_evm_address`.
fn derive_child_pubkey_uncompressed(account_xpub: &str, path_index: u32) -> Result<Vec<u8>> {
    let xpub = XPub::from_str(account_xpub).map_err(|e| anyhow!("Invalid xpub format: {}", e))?;

    // Derive path /0/{path_index} (external chain)
    let external_chain = ChildNumber::new(0, false)?;
    let address_index = ChildNumber::new(path_index, false)?;

    let child_xpub = xpub
        .derive_child(external_chain)?
        .derive_child(address_index)?;

    // Get uncompressed public key (65 bytes)
    let verifying_key = child_xpub.public_key();
    let uncompressed = verifying_key.to_encoded_point(false);
    let bytes = uncompressed.as_bytes();

    if bytes.len() != 65 || bytes[0] != 0x04 {
        return Err(anyhow!("Invalid uncompressed public key format"));
    }

    Ok(bytes.to_vec())
}

// ─── Key Derivation (with coin_type) ────────────────────────────────────────

/// Derive a private key from Mnemonic for a specific coin type.
///
/// # Path Structure
/// `m/44'/{coin_type}'/{account_index}'/0/{path_index}`
pub fn derive_private_key_from_mnemonic_for_coin(
    mnemonic: &str,
    account_index: i32,
    path_index: u32,
    coin_type: u32,
) -> Result<zeroize::Zeroizing<[u8; 32]>> {
    use bip32::XPrv;
    use bip39::Mnemonic;

    let mnemonic: Mnemonic = mnemonic
        .parse()
        .map_err(|e| anyhow!("Invalid mnemonic: {}", e))?;

    let seed = mnemonic.to_seed("");

    let path = format!("m/44'/{}'/{}'/0/{}", coin_type, account_index, path_index);
    let xprv = XPrv::derive_from_path(&seed, &path.parse()?)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    let key_bytes: [u8; 32] = xprv.to_bytes().into();
    Ok(zeroize::Zeroizing::new(key_bytes))
}

/// Derive Account-level xpub from Mnemonic for a specific coin type.
///
/// # Path Structure
/// `m/44'/{coin_type}'/{account_index}'` (hardened Account xpub)
pub fn derive_account_xpub_from_mnemonic_for_coin(
    mnemonic: &str,
    account_index: i32,
    coin_type: u32,
) -> Result<String> {
    use bip32::XPrv;
    use bip39::Mnemonic;

    let mnemonic: Mnemonic = mnemonic
        .parse()
        .map_err(|e| anyhow!("Invalid mnemonic: {}", e))?;

    let seed = mnemonic.to_seed("");

    let path = format!("m/44'/{}'/{}'", coin_type, account_index);
    let xprv = XPrv::derive_from_path(&seed, &path.parse()?)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    use bip32::Prefix;
    Ok(xprv.public_key().to_string(Prefix::XPUB))
}

/// Derive a private key from a raw BIP39 seed for a specific coin type.
///
/// # Path Structure
/// `m/44'/{coin_type}'/{account_index}'/0/{path_index}`
pub fn derive_private_key_from_seed_for_coin(
    seed: &[u8; 64],
    account_index: i32,
    path_index: u32,
    coin_type: u32,
) -> Result<zeroize::Zeroizing<[u8; 32]>> {
    use bip32::XPrv;

    let path = format!("m/44'/{}'/{}'/0/{}", coin_type, account_index, path_index);
    let xprv = XPrv::derive_from_path(seed, &path.parse()?)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    let key_bytes: [u8; 32] = xprv.to_bytes().into();
    Ok(zeroize::Zeroizing::new(key_bytes))
}

/// Derive Account-level xpub from a raw BIP39 seed for a specific coin type.
///
/// # Path Structure
/// `m/44'/{coin_type}'/{account_index}'` (hardened Account xpub)
pub fn derive_account_xpub_from_seed_for_coin(
    seed: &[u8; 64],
    account_index: i32,
    coin_type: u32,
) -> Result<String> {
    use bip32::{Prefix, XPrv};

    let path = format!("m/44'/{}'/{}'", coin_type, account_index);
    let xprv = XPrv::derive_from_path(seed, &path.parse()?)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    Ok(xprv.public_key().to_string(Prefix::XPUB))
}

// ─── Backward-Compatible Wrappers (TRON coin_type=195) ──────────────────────
// These preserve the original API for existing callers.

/// Derive a private key from Mnemonic (TRON: coin_type=195)
///
/// # Path Structure
/// `m/44'/195'/{account_index}'/0/{path_index}`
pub fn derive_private_key_from_mnemonic(
    mnemonic: &str,
    account_index: i32,
    path_index: u32,
) -> Result<zeroize::Zeroizing<[u8; 32]>> {
    derive_private_key_from_mnemonic_for_coin(mnemonic, account_index, path_index, 195)
}

/// Derive Account-level xpub from Mnemonic (TRON: coin_type=195)
///
/// # Path Structure
/// `m/44'/195'/{account_index}'`
pub fn derive_account_xpub_from_mnemonic(mnemonic: &str, account_index: i32) -> Result<String> {
    derive_account_xpub_from_mnemonic_for_coin(mnemonic, account_index, 195)
}

/// Derive a private key from seed (TRON: coin_type=195)
///
/// # Path Structure
/// `m/44'/195'/{account_index}'/0/{path_index}`
pub fn derive_private_key_from_seed(
    seed: &[u8; 64],
    account_index: i32,
    path_index: u32,
) -> Result<zeroize::Zeroizing<[u8; 32]>> {
    derive_private_key_from_seed_for_coin(seed, account_index, path_index, 195)
}

/// Derive Account-level xpub from seed (TRON: coin_type=195)
///
/// # Path Structure
/// `m/44'/195'/{account_index}'`
pub fn derive_account_xpub_from_seed(seed: &[u8; 64], account_index: i32) -> Result<String> {
    derive_account_xpub_from_seed_for_coin(seed, account_index, 195)
}

// ─── Solana: SLIP-0010 Ed25519 Derivation ───────────────────────────────────
//
// CRITICAL: Solana uses Ed25519, NOT secp256k1.
// - BIP32 `XPrv::derive_from_path()` uses secp256k1 → WRONG for Solana
// - SLIP-0010 uses HMAC-SHA512 iterative derivation → Ed25519 compatible
// - ALL path components are hardened (Ed25519 requirement per SLIP-0010)
// - No xpub cold derivation possible — must have seed for every derivation

/// Derive a Solana address from seed using SLIP-0010 (Ed25519).
///
/// # Path Structure
/// `m/44'/501'/{account_index}'/{path_index}'`
/// - 4 levels, ALL hardened (Ed25519 SLIP-0010 requirement)
/// - Compatible with Phantom wallet multi-account convention
///
/// # Returns
/// Base58-encoded 32-byte Ed25519 public key (= Solana address)
pub fn derive_solana_address_from_seed(
    seed: &[u8; 64],
    account_index: i32,
    path_index: u32,
) -> Result<String> {
    use ed25519_dalek::SigningKey;
    use zeroize::Zeroize;

    let acct_u32: u32 = account_index
        .try_into()
        .map_err(|_| anyhow!("account_index must be non-negative, got {}", account_index))?;

    // SLIP-0010: crate automatically adds hardened bit to each index
    let indexes = [44u32, 501, acct_u32, path_index];
    let mut private_key = slip10_ed25519::derive_ed25519_private_key(seed.as_ref(), &indexes);

    // Ed25519: public key = 32 bytes derived from private key
    let signing_key = SigningKey::from_bytes(&private_key);
    let verifying_key = signing_key.verifying_key();
    let address = bs58::encode(verifying_key.as_bytes()).into_string();

    // Zeroize private key from stack even though we only needed the public key
    private_key.zeroize();

    Ok(address)
}

/// Batch derive Solana addresses for a merchant.
///
/// Called during merchant registration to pre-generate an address pool.
/// ONLY addresses + path_index are stored in the database.
/// Private keys are NEVER persisted — deterministically re-derived at signing time.
///
/// # Returns
/// Vec of (path_index, address) tuples
pub fn batch_derive_solana_addresses(
    seed: &[u8; 64],
    account_index: i32,
    start_index: u32,
    count: u32,
) -> Result<Vec<(u32, String)>> {
    (start_index..(start_index + count))
        .map(|path_index| {
            let address = derive_solana_address_from_seed(seed, account_index, path_index)?;
            Ok((path_index, address))
        })
        .collect()
}

/// Derive a Solana Ed25519 private key from seed (for signing).
///
/// # Security
/// - The returned key is wrapped in `Zeroizing<>` for automatic cleanup
/// - Caller should use it immediately for signing and let it drop
/// - Same path always produces the same key (deterministic)
pub fn derive_solana_private_key_from_seed(
    seed: &[u8; 64],
    account_index: i32,
    path_index: u32,
) -> Result<zeroize::Zeroizing<[u8; 32]>> {
    let acct_u32: u32 = account_index
        .try_into()
        .map_err(|_| anyhow!("account_index must be non-negative, got {}", account_index))?;

    let indexes = [44u32, 501, acct_u32, path_index];
    let private_key = slip10_ed25519::derive_ed25519_private_key(seed.as_ref(), &indexes);
    Ok(zeroize::Zeroizing::new(private_key))
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test xpub from arbitrary key (for basic tests)
    const TEST_XPUB: &str = "xpub6D4BDPcP2GT577Vvch3R8wDkScZWzQzMMUm3PWbmWvVJrZwQY4VUNgqFJPMM3No2dFDFGTsxxpG5uJh7n7epu4trkrX7x7DogT5Uv6fcLW5";

    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // Reference vectors from TronWeb (JS) using standard test mnemonic
    const TRONWEB_XPUB: &str = "xpub6D1AabNHCupeiLM65ZR9UStMhJ1vCpyV4XbZdyhMZBiJXALQtmn9p42VTQckoHVn8WNqS7dqnJokZHAHcHGoaQgmv8D45oNUKx6DZMNZBCd";
    const TRONWEB_ADDRESSES: &[&str] = &[
        "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH",
        "TSeJkUh4Qv67VNFwY8LaAxERygNdy6NQZK",
        "TYJPRrdB5APNeRs4R7fYZSwW3TcrTKw2gx",
        "TRhVWK5XEDkQBDevcdCWW7RW51aRncty4W",
        "TT2X2yyubp7qpAWYYNE5JQWBtoZ7ikQFsY",
        "TBdYXtwq18cAhi1BA574TrP6tw2G86anu1",
        "TRGVdeYnLm92LodhRyCyTyxYvLqocfffRs",
        "TFj86XAQYBPvwUknt9yxmbc4k3zwUC7iVW",
        "TXAvZZ5aWvSDHdDJcfBZrJoX4ovLFMVeFA",
        "TCukLhQtEGXSSq3b43TQJp3QuhTieUqhfb",
    ];

    // ── TRON Tests (unchanged behavior) ──

    #[test]
    fn test_derive_deterministic() {
        let addr1 = derive_tron_address(TEST_XPUB, 0).unwrap();
        let addr2 = derive_tron_address(TEST_XPUB, 0).unwrap();

        assert_eq!(addr1, addr2);
        assert!(addr1.starts_with('T'));
        assert_eq!(addr1.len(), 34);

        let addr3 = derive_tron_address(TEST_XPUB, 1).unwrap();
        assert_ne!(addr1, addr3);
    }

    #[test]
    fn test_address_format() {
        let addr = derive_tron_address(TEST_XPUB, 0).unwrap();

        let decoded = bs58::decode(&addr).with_check(None).into_vec();
        assert!(decoded.is_ok());

        let bytes = decoded.unwrap();
        assert_eq!(bytes[0], 0x41); // TRON mainnet prefix
        assert_eq!(bytes.len(), 21); // 1 prefix + 20 address
    }

    #[test]
    fn test_boundary_conditions() {
        let addr0 = derive_tron_address(TEST_XPUB, 0);
        assert!(addr0.is_ok());

        let addr1 = derive_tron_address(TEST_XPUB, 1);
        assert!(addr1.is_ok());
        assert_ne!(addr0.unwrap(), addr1.unwrap());

        let addr_large = derive_tron_address(TEST_XPUB, 1000000);
        assert!(addr_large.is_ok());
        assert!(addr_large.unwrap().starts_with('T'));
    }

    #[test]
    fn test_invalid_xpub() {
        let result = derive_tron_address("invalid_xpub_string", 0);
        assert!(result.is_err());

        let result = derive_tron_address("", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequential_addresses_unique() {
        let addresses: Vec<String> = (0..10)
            .map(|i| derive_tron_address(TEST_XPUB, i).unwrap())
            .collect();

        let unique_count = addresses
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(unique_count, 10);

        for addr in &addresses {
            assert!(addr.starts_with('T'));
            assert_eq!(addr.len(), 34);
        }
    }

    #[test]
    fn test_tronweb_cross_validation() {
        for (index, expected_address) in TRONWEB_ADDRESSES.iter().enumerate() {
            let derived = derive_tron_address(TRONWEB_XPUB, index as u32).unwrap();
            assert_eq!(
                &derived, *expected_address,
                "Address mismatch at index {}: got {}, expected {}",
                index, derived, expected_address
            );
        }
    }

    #[test]
    fn test_seed_matches_mnemonic() {
        use bip39::Mnemonic;

        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        // Private key: multiple account/path combinations (TRON coin_type)
        for account_index in [0, 1, 5] {
            for path_index in [0, 1, 10, 100] {
                let from_mnemonic =
                    derive_private_key_from_mnemonic(TEST_MNEMONIC, account_index, path_index)
                        .unwrap();
                let from_seed =
                    derive_private_key_from_seed(&seed, account_index, path_index).unwrap();
                assert_eq!(
                    *from_mnemonic, *from_seed,
                    "Private key mismatch at account={}, path={}",
                    account_index, path_index
                );
            }
        }

        // Xpub: multiple account indices
        for account_index in [0, 1, 5] {
            let xpub_mnemonic =
                derive_account_xpub_from_mnemonic(TEST_MNEMONIC, account_index).unwrap();
            let xpub_seed = derive_account_xpub_from_seed(&seed, account_index).unwrap();
            assert_eq!(
                xpub_mnemonic, xpub_seed,
                "Xpub mismatch at account={}",
                account_index
            );
        }
    }

    // ── EVM Tests (new) ──

    #[test]
    fn test_derive_evm_address_format() {
        // Derive EVM xpub from test mnemonic (coin_type=60)
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();
        let addr = derive_evm_address(&evm_xpub, 0).unwrap();

        // Must start with 0x
        assert!(addr.starts_with("0x"), "EVM address must start with 0x");
        // Must be 42 chars (0x + 40 hex)
        assert_eq!(addr.len(), 42, "EVM address must be 42 characters");
        // Must be valid hex (after removing 0x prefix)
        assert!(
            hex::decode(&addr[2..]).is_ok(),
            "EVM address must be valid hex"
        );
    }

    #[test]
    fn test_derive_evm_address_deterministic() {
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();

        let addr1 = derive_evm_address(&evm_xpub, 0).unwrap();
        let addr2 = derive_evm_address(&evm_xpub, 0).unwrap();
        assert_eq!(addr1, addr2, "EVM derivation must be deterministic");

        let addr3 = derive_evm_address(&evm_xpub, 1).unwrap();
        assert_ne!(
            addr1, addr3,
            "Different indices must produce different addresses"
        );
    }

    #[test]
    fn test_derive_evm_address_cross_validation() {
        // Standard test mnemonic "abandon ... about" at m/44'/60'/0'/0/0
        // should produce a well-known Ethereum address.
        // Reference: https://iancoleman.io/bip39/ (BIP44 + ETH coin_type=60)
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();
        let addr = derive_evm_address(&evm_xpub, 0).unwrap();

        // The standard "abandon...about" mnemonic at m/44'/60'/0'/0/0
        // produces address: 0x9858EfFD232B4033E47d90003D41EC34EcaEda94
        assert_eq!(
            addr.to_lowercase(),
            "0x9858effd232b4033e47d90003d41ec34ecaeda94",
            "EVM address must match known test vector"
        );
    }

    #[test]
    fn test_evm_sequential_addresses_unique() {
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();

        let addresses: Vec<String> = (0..10)
            .map(|i| derive_evm_address(&evm_xpub, i).unwrap())
            .collect();

        let unique_count = addresses
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(unique_count, 10, "All 10 EVM addresses must be unique");
    }

    #[test]
    fn test_evm_tron_xpub_different() {
        // TRON and EVM xpubs must be different (different coin_type → different derivation path)
        let tron_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 195).unwrap();
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();

        assert_ne!(
            tron_xpub, evm_xpub,
            "TRON and EVM xpubs must differ (different coin_type)"
        );
    }

    #[test]
    fn test_coin_type_seed_mnemonic_consistency() {
        use bip39::Mnemonic;
        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        // EVM coin_type: mnemonic and seed must produce same results
        for coin_type in [60u32, 195] {
            let xpub_m =
                derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, coin_type).unwrap();
            let xpub_s = derive_account_xpub_from_seed_for_coin(&seed, 0, coin_type).unwrap();
            assert_eq!(xpub_m, xpub_s, "Xpub mismatch for coin_type={}", coin_type);

            let key_m =
                derive_private_key_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 0, coin_type).unwrap();
            let key_s = derive_private_key_from_seed_for_coin(&seed, 0, 0, coin_type).unwrap();
            assert_eq!(
                *key_m, *key_s,
                "Private key mismatch for coin_type={}",
                coin_type
            );
        }
    }

    #[test]
    fn test_derive_address_dispatch() {
        let tron_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 195).unwrap();
        let evm_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 60).unwrap();

        // TRON dispatch
        let tron_addr = derive_address(&tron_xpub, 0, Network::Tron).unwrap();
        assert!(tron_addr.starts_with('T'), "TRON address must start with T");

        // BSC dispatch (uses EVM derivation)
        let bsc_addr = derive_address(&evm_xpub, 0, Network::Bsc).unwrap();
        assert!(bsc_addr.starts_with("0x"), "BSC address must start with 0x");

        // Ethereum dispatch (uses same EVM derivation)
        let eth_addr = derive_address(&evm_xpub, 0, Network::Ethereum).unwrap();
        assert!(eth_addr.starts_with("0x"), "ETH address must start with 0x");

        // BSC and ETH should produce IDENTICAL addresses (same xpub, same derivation)
        assert_eq!(
            bsc_addr, eth_addr,
            "BSC and ETH addresses must be identical for same xpub"
        );
    }

    #[test]
    fn test_backward_compat_wrappers() {
        // Ensure the wrapper functions (no coin_type param) produce the same results
        // as the for_coin() variants with coin_type=195
        let xpub_wrapper = derive_account_xpub_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let xpub_explicit =
            derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 195).unwrap();
        assert_eq!(xpub_wrapper, xpub_explicit);

        let key_wrapper = derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 0).unwrap();
        let key_explicit =
            derive_private_key_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 0, 195).unwrap();
        assert_eq!(key_wrapper, key_explicit);
    }

    // ── Solana SLIP-0010 Ed25519 Tests ──

    #[test]
    fn test_solana_derive_deterministic() {
        use bip39::Mnemonic;
        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        // Same input → same output
        let addr1 = derive_solana_address_from_seed(&seed, 0, 0).unwrap();
        let addr2 = derive_solana_address_from_seed(&seed, 0, 0).unwrap();
        assert_eq!(addr1, addr2, "Solana derivation must be deterministic");

        // Different path_index → different address
        let addr3 = derive_solana_address_from_seed(&seed, 0, 1).unwrap();
        assert_ne!(
            addr1, addr3,
            "Different path indices must produce different addresses"
        );

        // Different account_index → different address
        let addr4 = derive_solana_address_from_seed(&seed, 1, 0).unwrap();
        assert_ne!(
            addr1, addr4,
            "Different account indices must produce different addresses"
        );
    }

    #[test]
    fn test_solana_address_format() {
        use crate::entity::network::validate_solana_address;
        use bip39::Mnemonic;

        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        let addr = derive_solana_address_from_seed(&seed, 0, 0).unwrap();

        // Solana addresses are Base58-encoded 32-byte Ed25519 public keys
        // Typically 32-44 characters
        assert!(
            addr.len() >= 32 && addr.len() <= 44,
            "Solana address length {} outside expected range 32-44",
            addr.len()
        );

        // Must decode to exactly 32 bytes
        let decoded = bs58::decode(&addr).into_vec().unwrap();
        assert_eq!(decoded.len(), 32, "Solana address must decode to 32 bytes");

        // Must not be all zeros
        assert!(
            !decoded.iter().all(|b| *b == 0),
            "Address must not be zero pubkey"
        );

        // Must pass the existing Solana address validator
        assert!(
            validate_solana_address(&addr).is_ok(),
            "Generated Solana address must pass validate_solana_address"
        );
    }

    #[test]
    fn test_solana_batch_derive() {
        use bip39::Mnemonic;

        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        let batch = batch_derive_solana_addresses(&seed, 0, 0, 10).unwrap();
        assert_eq!(batch.len(), 10);

        // All path indices should be contiguous 0..9
        let indices: Vec<u32> = batch.iter().map(|(idx, _)| *idx).collect();
        assert_eq!(indices, (0..10).collect::<Vec<_>>());

        // All addresses must be unique
        let unique_count = batch
            .iter()
            .map(|(_, a)| a)
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(unique_count, 10, "All batch addresses must be unique");

        // Each address should individually match derive_solana_address_from_seed
        for (i, addr) in &batch {
            let expected = derive_solana_address_from_seed(&seed, 0, *i).unwrap();
            assert_eq!(
                addr, &expected,
                "Batch address at index {} must match single derivation",
                i
            );
        }
    }

    #[test]
    fn test_solana_private_key_consistency() {
        use bip39::Mnemonic;
        use ed25519_dalek::SigningKey;

        let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
        let seed = mnemonic.to_seed("");

        // Private key → public key → address should match derive_solana_address_from_seed
        let private_key = derive_solana_private_key_from_seed(&seed, 0, 0).unwrap();
        let signing_key = SigningKey::from_bytes(&*private_key);
        let pubkey = signing_key.verifying_key();
        let addr_from_key = bs58::encode(pubkey.as_bytes()).into_string();

        let addr_from_derive = derive_solana_address_from_seed(&seed, 0, 0).unwrap();
        assert_eq!(
            addr_from_key, addr_from_derive,
            "Private key derivation must produce same address as address derivation"
        );
    }

    #[test]
    fn test_solana_derive_address_dispatch_error() {
        // derive_address() should return an error for Solana (requires seed, not xpub)
        let tron_xpub = derive_account_xpub_from_mnemonic_for_coin(TEST_MNEMONIC, 0, 195).unwrap();
        let result = derive_address(&tron_xpub, 0, Network::Solana);
        assert!(result.is_err(), "derive_address should error for Solana");
        assert!(
            result.unwrap_err().to_string().contains("SLIP-0010"),
            "Error should mention SLIP-0010"
        );
    }
}
