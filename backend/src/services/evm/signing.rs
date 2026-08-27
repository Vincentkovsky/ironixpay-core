//! EVM transaction RLP encoding for EIP-155 replay protection.
//!
//! Provides two functions for the two-step EVM signing flow:
//! 1. `rlp_encode_for_signing()` — Build the EIP-155 pre-image (for TransactionSigner)
//! 2. `assemble_signed_tx()` — Combine unsigned tx + signature into broadcast-ready format
//!
//! Note: Actual signing (key derivation + secp256k1) is handled by the `TransactionSigner`
//! trait, which uses BIP-44 coin_type=60 for EVM chains.

use anyhow::{anyhow, Result};
use sha3::{Digest, Keccak256};

use crate::services::chain::types::{EvmSignedTx, EvmUnsignedTx};

// ─── RLP Encoding ───────────────────────────────────────────────────────────
// Minimal RLP encoder for EIP-155 legacy transactions.
// Only supports the types needed: byte arrays, integers, and lists.

/// RLP-encode a single unsigned integer (big-endian, no leading zeros).
fn rlp_encode_uint(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80]; // Empty string = zero
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    let trimmed = &bytes[start..];
    rlp_encode_bytes(trimmed)
}

/// RLP-encode a byte slice.
fn rlp_encode_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] < 0x80 {
        vec![bytes[0]]
    } else if bytes.len() <= 55 {
        let mut out = vec![0x80 + bytes.len() as u8];
        out.extend_from_slice(bytes);
        out
    } else {
        let len_bytes = encode_length(bytes.len());
        let mut out = vec![0xb7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(bytes);
        out
    }
}

/// RLP-encode a list (already-encoded items concatenated).
fn rlp_encode_list(items: &[u8]) -> Vec<u8> {
    if items.len() <= 55 {
        let mut out = vec![0xc0 + items.len() as u8];
        out.extend_from_slice(items);
        out
    } else {
        let len_bytes = encode_length(items.len());
        let mut out = vec![0xf7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(items);
        out
    }
}

/// Encode a length as big-endian bytes (no leading zeros).
fn encode_length(len: usize) -> Vec<u8> {
    let bytes = (len as u64).to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    bytes[start..].to_vec()
}

/// RLP-encode a hex string value (0x-prefixed). "0x0" → empty bytes (zero).
fn rlp_encode_hex_value(hex: &str) -> Vec<u8> {
    let stripped = hex.strip_prefix("0x").unwrap_or(hex);
    if stripped.is_empty() || stripped.chars().all(|c| c == '0') {
        return vec![0x80];
    }
    let padded = if stripped.len() % 2 != 0 {
        format!("0{}", stripped)
    } else {
        stripped.to_string()
    };
    let bytes = hex::decode(&padded).unwrap_or_default();
    rlp_encode_bytes(&bytes)
}

/// RLP-encode hex calldata (0x-prefixed). Empty "0x" → empty bytes.
fn rlp_encode_hex_data(hex: &str) -> Vec<u8> {
    let stripped = hex.strip_prefix("0x").unwrap_or(hex);
    if stripped.is_empty() {
        return vec![0x80];
    }
    let bytes = hex::decode(stripped).unwrap_or_default();
    rlp_encode_bytes(&bytes)
}

/// RLP-encode a 20-byte EVM address from 0x-prefixed hex string.
fn rlp_encode_address(addr: &str) -> Vec<u8> {
    let stripped = addr.strip_prefix("0x").unwrap_or(addr);
    let bytes = hex::decode(stripped).unwrap_or_default();
    rlp_encode_bytes(&bytes)
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Build the EIP-155 RLP pre-image for signing.
///
/// Returns the raw bytes that should be passed to `TransactionSigner::sign_transaction_for_coin()`.
/// The signer will internally hash (Keccak-256) and sign with secp256k1.
///
/// EIP-155 pre-image: `rlp([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0])`
pub fn rlp_encode_for_signing(tx: &EvmUnsignedTx) -> Vec<u8> {
    let mut items = Vec::new();
    items.extend(rlp_encode_uint(tx.nonce));
    items.extend(rlp_encode_uint(tx.gas_price));
    items.extend(rlp_encode_uint(tx.gas_limit));
    items.extend(rlp_encode_address(&tx.to));
    items.extend(rlp_encode_hex_value(&tx.value));
    items.extend(rlp_encode_hex_data(&tx.data));
    items.extend(rlp_encode_uint(tx.chain_id));
    items.extend(rlp_encode_uint(0)); // EIP-155 placeholder
    items.extend(rlp_encode_uint(0)); // EIP-155 placeholder

    rlp_encode_list(&items)
}

/// Assemble a signed EVM transaction from an unsigned tx and a 65-byte signature.
///
/// Takes the `(R || S || V)` signature from `TransactionSigner` and produces
/// a broadcast-ready RLP-encoded signed transaction.
///
/// # Arguments
/// * `tx` - The unsigned transaction (same one passed to `rlp_encode_for_signing`)
/// * `signature` - 65-byte signature: `R (32 bytes) || S (32 bytes) || V (1 byte recovery_id)`
///
/// # Returns
/// `EvmSignedTx` with `raw_tx_hex` (0x-prefixed RLP) and `tx_hash` (keccak256).
pub fn assemble_signed_tx(tx: &EvmUnsignedTx, signature: &[u8]) -> Result<EvmSignedTx> {
    if signature.len() != 65 {
        return Err(anyhow!(
            "Invalid signature length: expected 65, got {}",
            signature.len()
        ));
    }

    let r = &signature[0..32];
    let s = &signature[32..64];
    // TransactionSigner returns V = recovery_id + 27 (standard Ethereum convention).
    // EIP-155 needs raw recovery_id (0 or 1).
    let raw_v = signature[64];
    let recovery_id = if raw_v >= 27 { raw_v - 27 } else { raw_v };

    // EIP-155: v = chainId * 2 + 35 + recovery_id
    let v = tx.chain_id * 2 + 35 + recovery_id as u64;

    // Build signed transaction RLP
    let mut signed_items = Vec::new();
    signed_items.extend(rlp_encode_uint(tx.nonce));
    signed_items.extend(rlp_encode_uint(tx.gas_price));
    signed_items.extend(rlp_encode_uint(tx.gas_limit));
    signed_items.extend(rlp_encode_address(&tx.to));
    signed_items.extend(rlp_encode_hex_value(&tx.value));
    signed_items.extend(rlp_encode_hex_data(&tx.data));
    signed_items.extend(rlp_encode_uint(v));
    // Strip leading zeros from r and s for proper RLP encoding
    let r_trimmed = trim_leading_zeros(r);
    let s_trimmed = trim_leading_zeros(s);
    signed_items.extend(rlp_encode_bytes(r_trimmed));
    signed_items.extend(rlp_encode_bytes(s_trimmed));

    let raw_tx = rlp_encode_list(&signed_items);
    let raw_tx_hex = format!("0x{}", hex::encode(&raw_tx));

    // tx_hash = keccak256(signed_rlp)
    let tx_hash = format!("0x{}", hex::encode(Keccak256::digest(&raw_tx)));

    Ok(EvmSignedTx {
        tx_hash,
        raw_tx_hex,
    })
}

/// Trim leading zero bytes (but keep at least one byte).
fn trim_leading_zeros(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(bytes.len().saturating_sub(1));
    &bytes[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlp_encode_uint() {
        assert_eq!(rlp_encode_uint(0), vec![0x80]);
        assert_eq!(rlp_encode_uint(1), vec![0x01]);
        assert_eq!(rlp_encode_uint(127), vec![0x7f]);
        assert_eq!(rlp_encode_uint(128), vec![0x81, 0x80]);
        assert_eq!(rlp_encode_uint(256), vec![0x82, 0x01, 0x00]);
    }

    #[test]
    fn test_rlp_encode_address() {
        let addr = "0x0000000000000000000000000000000000000001";
        let encoded = rlp_encode_address(addr);
        assert_eq!(encoded.len(), 21); // 0x94 prefix + 20 bytes
        assert_eq!(encoded[0], 0x94); // 0x80 + 20
    }

    #[test]
    fn test_rlp_encode_for_signing_produces_valid_list() {
        let tx = EvmUnsignedTx {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: "0x0000000000000000000000000000000000000002".to_string(),
            data: "0x".to_string(),
            value: "0x0".to_string(),
            nonce: 0,
            gas_price: 3_000_000_000, // 3 gwei
            gas_limit: 21_000,
            chain_id: 56,
        };

        let rlp = rlp_encode_for_signing(&tx);
        // Should start with 0xc0+ (short list) or 0xf7+ (long list)
        assert!(rlp[0] >= 0xc0, "RLP should be a list, got 0x{:02x}", rlp[0]);
    }

    #[test]
    fn test_assemble_signed_tx_rejects_bad_signature() {
        let tx = EvmUnsignedTx {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: "0x0000000000000000000000000000000000000002".to_string(),
            data: "0x".to_string(),
            value: "0x0".to_string(),
            nonce: 0,
            gas_price: 3_000_000_000,
            gas_limit: 21_000,
            chain_id: 56,
        };

        let bad_sig = vec![0u8; 64]; // Too short
        assert!(assemble_signed_tx(&tx, &bad_sig).is_err());
    }

    #[test]
    fn test_assemble_signed_tx_produces_valid_output() {
        let tx = EvmUnsignedTx {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: "0x0000000000000000000000000000000000000002".to_string(),
            data: "0x".to_string(),
            value: "0x0".to_string(),
            nonce: 0,
            gas_price: 3_000_000_000,
            gas_limit: 21_000,
            chain_id: 56,
        };

        // Fake 65-byte signature (not cryptographically valid, but structurally correct)
        let mut sig = vec![1u8; 64];
        sig.push(0); // recovery_id = 0
        let result = assemble_signed_tx(&tx, &sig).unwrap();
        assert!(result.raw_tx_hex.starts_with("0x"));
        assert!(result.tx_hash.starts_with("0x"));
        assert_eq!(result.tx_hash.len(), 66); // 0x + 64 hex chars
    }
}
