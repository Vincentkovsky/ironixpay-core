//! Solana transaction assembly (post-signing).
//!
//! Provides `assemble_signed_solana_tx()` to combine an unsigned message
//! with Ed25519 signatures into a broadcast-ready Base64 transaction.
//!
//! # Signing flow
//! 1. Build `SolanaUnsignedTx` via `build_spl_transfer()` / `build_spl_sweep()`
//! 2. Sign `message_bytes` via `TransactionSigner::sign_transaction_for_coin(..., coin_type=501)`
//!    (handles SLIP-0010 key derivation + Ed25519 signing internally)
//! 3. Call `assemble_signed_solana_tx()` to produce broadcast-ready `SolanaSignedTx`
//!
//! Note: Unlike EVM (RLP + keccak256), Solana tx format is simple:
//!   `[num_signatures(compact-u16)] [sig(64)]... [message_bytes]`
//! The tx_hash is the fee payer's signature (first one), Base58-encoded.

use anyhow::{anyhow, Result};

use crate::services::chain::types::{SolanaSignedTx, SolanaUnsignedTx};

// ─── Public API ─────────────────────────────────────────────────────────────

/// Assemble a signed Solana transaction from an unsigned tx and ordered signatures.
///
/// # Arguments
/// * `tx` - The unsigned transaction (contains `message_bytes` and signer metadata)
/// * `signatures` - Ordered Ed25519 signatures (64 bytes each), matching `signer_pubkeys` order.
///   Fee payer's signature MUST be first (Solana wire format requirement).
///
/// # Wire Format
/// ```text
/// [num_signatures: compact-u16]  (usually 1 byte for <=127 signers)
/// [signature_0: 64 bytes]        (fee payer — FIRST, also serves as tx_hash)
/// [signature_1: 64 bytes]        (authority, if dual-signer sweep)
/// ...
/// [message_bytes: N bytes]       (serialized Solana Message)
/// ```
///
/// # Returns
/// `SolanaSignedTx` with:
/// - `signature`: Base58-encoded fee payer signature (= tx_hash on Solana)
/// - `serialized_tx`: Base64-encoded complete transaction (for `sendTransaction`)
pub fn assemble_signed_solana_tx(
    tx: &SolanaUnsignedTx,
    signatures: &[Vec<u8>],
) -> Result<SolanaSignedTx> {
    // Validate signature count matches expected signers
    if signatures.len() != tx.num_required_signatures as usize {
        return Err(anyhow!(
            "Signature count mismatch: expected {}, got {}",
            tx.num_required_signatures,
            signatures.len()
        ));
    }

    // Validate each signature is exactly 64 bytes (Ed25519)
    for (i, sig) in signatures.iter().enumerate() {
        if sig.len() != 64 {
            return Err(anyhow!(
                "Signature {} has invalid length: expected 64, got {}",
                i,
                sig.len()
            ));
        }
    }

    // Build wire-format transaction
    let num_sigs = signatures.len();
    let mut wire = Vec::with_capacity(3 + num_sigs * 64 + tx.message_bytes.len());

    // 1. Number of signatures (compact-u16 encoding)
    encode_compact_u16(&mut wire, num_sigs as u16);

    // 2. Signatures (fee payer first, then other signers)
    for sig in signatures {
        wire.extend_from_slice(sig);
    }

    // 3. Message bytes
    wire.extend_from_slice(&tx.message_bytes);

    // tx_hash = fee payer's signature, Base58-encoded
    let tx_hash = bs58::encode(&signatures[0]).into_string();

    // sendTransaction expects Base64 encoding
    let serialized_tx = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &wire);

    Ok(SolanaSignedTx {
        signature: tx_hash,
        serialized_tx,
    })
}

/// Low-level Ed25519 signing helper (convenience wrapper).
///
/// Signs raw message bytes with a 32-byte Ed25519 private key.
/// No pre-hashing — Ed25519 handles hashing internally (SHA-512).
///
/// Used primarily for testing. Production code uses
/// `TransactionSigner::sign_transaction_for_coin(..., 501)` which
/// handles key derivation + signing + zeroization.
pub fn sign_solana_message(message_bytes: &[u8], private_key: &[u8; 32]) -> [u8; 64] {
    use ed25519_dalek::Signer;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(message_bytes);
    signature.to_bytes()
}

// ─── Compact-u16 Encoding ───────────────────────────────────────────────────

/// Encode a u16 using Solana's compact-u16 format.
///
/// - 0..0x7F → 1 byte
/// - 0x80..0x3FFF → 2 bytes
/// - 0x4000..0xFFFF → 3 bytes
///
/// Each byte uses 7 data bits + 1 continuation bit (MSB).
fn encode_compact_u16(buf: &mut Vec<u8>, mut value: u16) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value > 0 {
            byte |= 0x80; // Set continuation bit
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_u16_encoding() {
        let mut buf = Vec::new();

        // Single byte: 0
        encode_compact_u16(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);
        buf.clear();

        // Single byte: 1
        encode_compact_u16(&mut buf, 1);
        assert_eq!(buf, vec![0x01]);
        buf.clear();

        // Single byte: 127 (max single byte)
        encode_compact_u16(&mut buf, 127);
        assert_eq!(buf, vec![0x7F]);
        buf.clear();

        // Two bytes: 128
        encode_compact_u16(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);
        buf.clear();

        // Two bytes: 255
        encode_compact_u16(&mut buf, 255);
        assert_eq!(buf, vec![0xFF, 0x01]);
        buf.clear();

        // Two bytes: 16383 (max two bytes)
        encode_compact_u16(&mut buf, 16383);
        assert_eq!(buf, vec![0xFF, 0x7F]);
        buf.clear();

        // Three bytes: 16384
        encode_compact_u16(&mut buf, 16384);
        assert_eq!(buf, vec![0x80, 0x80, 0x01]);
        buf.clear();
    }

    #[test]
    fn test_assemble_rejects_signature_count_mismatch() {
        let tx = SolanaUnsignedTx {
            message_bytes: vec![1, 2, 3],
            recent_blockhash: "test".to_string(),
            last_valid_block_height: 100,
            num_required_signatures: 1,
            signer_pubkeys: vec!["11111111111111111111111111111111".to_string()],
        };

        // No signatures
        let result = assemble_signed_solana_tx(&tx, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mismatch"));

        // Too many signatures
        let result = assemble_signed_solana_tx(&tx, &[vec![0u8; 64], vec![0u8; 64]]);
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_rejects_bad_signature_length() {
        let tx = SolanaUnsignedTx {
            message_bytes: vec![1, 2, 3],
            recent_blockhash: "test".to_string(),
            last_valid_block_height: 100,
            num_required_signatures: 1,
            signer_pubkeys: vec!["11111111111111111111111111111111".to_string()],
        };

        // 65-byte signature (EVM-style, wrong for Solana)
        let result = assemble_signed_solana_tx(&tx, &[vec![0u8; 65]]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("64"));
    }

    #[test]
    fn test_assemble_single_signer_produces_valid_output() {
        let message = vec![42u8; 100]; // Fake message bytes
        let sig = vec![1u8; 64]; // Fake signature

        let tx = SolanaUnsignedTx {
            message_bytes: message.clone(),
            recent_blockhash: "test".to_string(),
            last_valid_block_height: 100,
            num_required_signatures: 1,
            signer_pubkeys: vec!["11111111111111111111111111111111".to_string()],
        };

        let result = assemble_signed_solana_tx(&tx, &[sig.clone()]).unwrap();

        // tx_hash = Base58 of first signature
        assert_eq!(result.signature, bs58::encode(&sig).into_string());

        // Decode and verify wire format
        let wire = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &result.serialized_tx,
        )
        .unwrap();

        // Wire format: [compact-u16(1)] [sig(64)] [message(100)]
        assert_eq!(wire[0], 1); // compact-u16 for 1
        assert_eq!(&wire[1..65], &sig[..]); // Signature
        assert_eq!(&wire[65..], &message[..]); // Message
    }

    #[test]
    fn test_assemble_dual_signer_produces_valid_output() {
        let message = vec![55u8; 200];
        let sig_fee_payer = vec![0xAAu8; 64];
        let sig_authority = vec![0xBBu8; 64];

        let tx = SolanaUnsignedTx {
            message_bytes: message.clone(),
            recent_blockhash: "test".to_string(),
            last_valid_block_height: 100,
            num_required_signatures: 2,
            signer_pubkeys: vec![
                "treasury_pubkey".to_string(),
                "from_address_pubkey".to_string(),
            ],
        };

        let result =
            assemble_signed_solana_tx(&tx, &[sig_fee_payer.clone(), sig_authority.clone()])
                .unwrap();

        // tx_hash = fee payer's signature (first)
        assert_eq!(result.signature, bs58::encode(&sig_fee_payer).into_string());

        // Decode wire format
        let wire = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &result.serialized_tx,
        )
        .unwrap();

        // [compact-u16(2)] [sig_fee_payer(64)] [sig_authority(64)] [message(200)]
        assert_eq!(wire[0], 2); // compact-u16 for 2
        assert_eq!(&wire[1..65], &sig_fee_payer[..]);
        assert_eq!(&wire[65..129], &sig_authority[..]);
        assert_eq!(&wire[129..], &message[..]);
    }

    #[test]
    fn test_sign_solana_message_produces_valid_signature() {
        // Use a known test key
        let private_key = [42u8; 32];
        let message = b"test message for signing";

        let sig = sign_solana_message(message, &private_key);
        assert_eq!(sig.len(), 64);

        // Verify the signature is valid
        use ed25519_dalek::{Signature, SigningKey, Verifier};
        let signing_key = SigningKey::from_bytes(&private_key);
        let verifying_key = signing_key.verifying_key();
        let signature = Signature::from_bytes(&sig);
        assert!(verifying_key.verify(message, &signature).is_ok());
    }

    #[test]
    fn test_sign_solana_message_is_deterministic() {
        let private_key = [7u8; 32];
        let message = b"deterministic signing test";

        let sig1 = sign_solana_message(message, &private_key);
        let sig2 = sign_solana_message(message, &private_key);
        assert_eq!(sig1, sig2);
    }
}
