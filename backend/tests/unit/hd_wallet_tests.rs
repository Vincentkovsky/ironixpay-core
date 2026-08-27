use ironix_pay::services::address::hd_wallet;

// Standard Test Vectors from TronWeb / BIP32
// Mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

// Expected Private Keys & Addresses for path m/44'/195'/{account}'/0/{index}
// We will test Account 0, Path 0
// Reference: Derived using TronWeb or debug_hd tool
const ACCOUNT_0_PATH_0_ADDR: &str = "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH";
// Private key for m/44'/195'/0'/0/0 from BIP39 tool for this mnemonic + Tron path
// Verified externally
const ACCOUNT_0_PATH_0_PRIV_HEX: &str =
    "b5a4cea271ff424d7c31dc12a3e43e401df7a40d7412a15750f3f0b6b5449a28";

#[test]
fn test_derive_private_key_vector() {
    let priv_key = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 0).unwrap();
    let priv_hex = hex::encode(priv_key);

    assert_eq!(
        priv_hex, ACCOUNT_0_PATH_0_PRIV_HEX,
        "Private key mismatch against standard vector"
    );
}

#[test]
fn test_derive_private_key_account_isolation() {
    // Generate key for Account 0
    let key_acc0 = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 0).unwrap();

    // Generate key for Account 1
    let key_acc1 = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 1, 0).unwrap();

    // They must be different
    assert_ne!(
        key_acc0, key_acc1,
        "Different accounts must have different keys"
    );
}

#[test]
fn test_derive_private_key_path_isolation() {
    // Generate key for Path 0
    let key_path0 = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 0).unwrap();

    // Generate key for Path 1
    let key_path1 = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 1).unwrap();

    // They must be different
    assert_ne!(
        key_path0, key_path1,
        "Different paths must have different keys"
    );
}

#[test]
fn test_invalid_mnemonic() {
    let res = hd_wallet::derive_private_key_from_mnemonic("invalid mnemonic phrase", 0, 0);
    assert!(res.is_err(), "Should fail with invalid mnemonic");
}

#[test]
fn test_derive_address_consistency() {
    // 1. Derive Private Key
    let priv_key = hd_wallet::derive_private_key_from_mnemonic(TEST_MNEMONIC, 0, 0).unwrap();

    // 2. Derive Address from Private Key (Manual Calculation to Verify Consistency)
    // This effectively tests that the Private Key we derived corresponds to the Address
    // we expect from the public key derivation path used in `derive_tron_address`.
    // Since `derive_tron_address` uses xpub, this connects the two worlds.

    use k256::ecdsa::SigningKey;
    let signing_key = SigningKey::from_bytes((&*priv_key).into()).unwrap();
    let verifying_key = signing_key.verifying_key();
    let uncompressed = verifying_key.to_encoded_point(false);
    let public_key_bytes = uncompressed.as_bytes();

    // Keccak256
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(&public_key_bytes[1..]); // Skip 0x04 prefix
    let hash = hasher.finalize();

    let mut addr_with_prefix = vec![0x41];
    addr_with_prefix.extend_from_slice(&hash[12..32]);
    let address = bs58::encode(&addr_with_prefix).with_check().into_string();

    assert_eq!(
        address, ACCOUNT_0_PATH_0_ADDR,
        "Derived address from private key must match expected standard vector address"
    );
}
