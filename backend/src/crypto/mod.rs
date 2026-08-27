//! Encryption Utilities
//!
//! AES-256-GCM symmetric encryption for sensitive data at rest.
//! Aligned with docs/system_design.md § 6.3

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;

/// Encrypt sensitive data using AES-256-GCM
///
/// # Arguments
/// * `plaintext` - Data to encrypt (e.g., xpub, TOTP secret)
/// * `key` - 32-byte encryption key (DEK)
///
/// # Returns
/// Base64-encoded string: `nonce(12 bytes) || ciphertext || tag(16 bytes)`
pub fn encrypt_aes_gcm(plaintext: &str, key: &[u8; 32]) -> Result<String> {
    if plaintext.is_empty() {
        return Err(anyhow!("Cannot encrypt empty plaintext"));
    }

    let cipher = Aes256Gcm::new(key.into());

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    let mut encrypted = nonce_bytes.to_vec();
    encrypted.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&encrypted))
}

/// Decrypt AES-256-GCM encrypted data
pub fn decrypt_aes_gcm(encrypted_base64: &str, key: &[u8; 32]) -> Result<String> {
    if encrypted_base64.is_empty() {
        return Err(anyhow!("Cannot decrypt empty ciphertext"));
    }

    let encrypted = BASE64
        .decode(encrypted_base64)
        .map_err(|e| anyhow!("Invalid base64: {}", e))?;

    if encrypted.len() < 12 {
        return Err(anyhow!("Encrypted data too short"));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(key.into());
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext_bytes).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = b"ThisIsA32ByteTestKeyForAES256GCM";
        let plaintext = "xpub6D4BDPcP2GT577Vvch3R8wDkScZWzQzMMUm3PWbmWvVJrZwQY4VUNgqFJPMM3No2dFDFGTsxxpG5uJh7n7epu4trkrX7x7DogT5Uv6fcLW5";

        let encrypted = encrypt_aes_gcm(plaintext, key).unwrap();
        let decrypted = decrypt_aes_gcm(&encrypted, key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_nonces() {
        let key = b"ThisIsA32ByteTestKeyForAES256GCM";
        let plaintext = "secret_data";

        let enc1 = encrypt_aes_gcm(plaintext, key).unwrap();
        let enc2 = encrypt_aes_gcm(plaintext, key).unwrap();

        assert_ne!(enc1, enc2); // Random nonces
        assert_eq!(decrypt_aes_gcm(&enc1, key).unwrap(), plaintext);
        assert_eq!(decrypt_aes_gcm(&enc2, key).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = b"ThisIsA32ByteTestKeyForAES256GCM";
        let key2 = b"AnotherWrongKey32BytesLongHere!!";
        let plaintext = "secret";

        let encrypted = encrypt_aes_gcm(plaintext, key1).unwrap();
        let result = decrypt_aes_gcm(&encrypted, key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_empty_plaintext_fails() {
        let key = b"ThisIsA32ByteTestKeyForAES256GCM";
        let result = encrypt_aes_gcm("", key);
        assert!(result.is_err());
    }
}
