use anyhow::{anyhow, Result};
use async_trait::async_trait;
use secrecy::{ExposeSecret, Secret}; // Needed for verify_signature if we eventually add verify

/// Type alias for a thread-safe MasterKeyProvider trait object
pub type MasterKeyProviderBox = Box<dyn MasterKeyProvider + Send + Sync>;

/// Type alias for a thread-safe TransactionSigner trait object
pub type TransactionSignerBox = Box<dyn TransactionSigner + Send + Sync>;

/// Abstraction for retrieving the Account Extended Public Key (xpub).
/// This allows switching between Local Mnemonic (Dev) and KMS (Prod).
///
/// Implementors MUST override `get_account_xpub_for_coin` (the primary method).
/// `get_account_xpub` is a convenience wrapper that defaults to TRON (coin_type=195).
#[async_trait]
pub trait MasterKeyProvider: Send + Sync {
    /// Get the Account Extended Public Key (xpub) for a specific coin type.
    ///
    /// This is the **primary** method that implementations must provide.
    ///
    /// # Arguments
    /// * `account_index` - Merchant's account index
    /// * `coin_type` - BIP44 coin type (195 for TRON, 60 for EVM)
    async fn get_account_xpub_for_coin(&self, account_index: u32, coin_type: u32)
        -> Result<String>;

    /// Convenience: Get xpub for TRON (coin_type=195).
    ///
    /// Default delegates to `get_account_xpub_for_coin(account_index, 195)`.
    async fn get_account_xpub(&self, account_index: u32) -> Result<String> {
        self.get_account_xpub_for_coin(account_index, 195).await
    }

    /// Batch-derive addresses for coin types that require seed access (Solana).
    ///
    /// Solana uses Ed25519/SLIP-0010 which does NOT support xpub cold derivation.
    /// Each address derivation requires the master seed, so this method provides
    /// a batch interface that minimizes seed exposure.
    ///
    /// # Returns
    /// `Vec<(path_index, address)>` — matches `batch_derive_solana_addresses` output.
    async fn batch_derive_addresses(
        &self,
        _account_index: u32,
        coin_type: u32,
        _start_index: u32,
        _count: u32,
    ) -> Result<Vec<(u32, String)>> {
        Err(anyhow!(
            "batch_derive_addresses not supported for coin_type={}",
            coin_type
        ))
    }
}

/// Abstraction for signing transactions.
/// This allows switching between Local Mnemonic (Dev) and KMS (Prod).
///
/// Implementors MUST override `sign_transaction_for_coin` (the primary method).
/// `sign_transaction` is a convenience wrapper that defaults to TRON (coin_type=195).
#[async_trait]
pub trait TransactionSigner: Send + Sync {
    /// Sign a transaction with a specific coin type for key derivation.
    ///
    /// This is the **primary** method that implementations must provide.
    ///
    /// # Arguments
    /// * `transaction_bytes` - Raw transaction bytes (Protobuf for TRON, RLP for EVM).
    ///   **IMPORTANT**: Do NOT pass a hash. This function performs hashing internally.
    /// * `account_index` - Merchant's account index (hardened derivation).
    /// * `path_index` - Address index within merchant.
    /// * `coin_type` - BIP44 coin type (195 for TRON, 60 for EVM).
    ///
    /// # Returns
    /// * `Vec<u8>` - The 65-byte signature (R || S || V).
    async fn sign_transaction_for_coin(
        &self,
        transaction_bytes: &[u8],
        account_index: i32,
        path_index: u32,
        coin_type: u32,
    ) -> Result<Vec<u8>>;

    /// Convenience: Sign with TRON key derivation (coin_type=195).
    ///
    /// Default delegates to `sign_transaction_for_coin(bytes, idx, path, 195)`.
    async fn sign_transaction(
        &self,
        transaction_bytes: &[u8],
        account_index: i32,
        path_index: u32,
    ) -> Result<Vec<u8>> {
        self.sign_transaction_for_coin(transaction_bytes, account_index, path_index, 195)
            .await
    }
}

/// Local implementation using a fixed mnemonic string (Environment Variable).
pub struct LocalMnemonicProvider {
    mnemonic: Secret<String>,
}

impl LocalMnemonicProvider {
    pub fn new(mnemonic: Secret<String>) -> Self {
        Self { mnemonic }
    }
}

#[async_trait]
impl MasterKeyProvider for LocalMnemonicProvider {
    async fn get_account_xpub_for_coin(
        &self,
        account_index: u32,
        coin_type: u32,
    ) -> Result<String> {
        use anyhow::Context;

        // Solana uses Ed25519 (SLIP-0010) which does NOT support xpub cold derivation.
        // Each address derivation requires the master seed.
        if coin_type == 501 {
            return Err(anyhow!(
                "Solana (coin_type=501) does not support xpub derivation. \
                 Use batch_derive_solana_addresses() with seed access instead."
            ));
        }

        let index_i32: i32 = account_index
            .try_into()
            .context("Account index exceeds i32::MAX limit")?;

        let mnemonic = self.mnemonic.clone();

        tokio::task::spawn_blocking(move || {
            crate::services::address::hd_wallet::derive_account_xpub_from_mnemonic_for_coin(
                mnemonic.expose_secret(),
                index_i32,
                coin_type,
            )
        })
        .await
        .context("Key derivation task panicked or was cancelled")?
    }

    async fn batch_derive_addresses(
        &self,
        account_index: u32,
        coin_type: u32,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<(u32, String)>> {
        use anyhow::Context;

        if coin_type != 501 {
            return Err(anyhow!(
                "batch_derive_addresses only supports Solana (coin_type=501), got {}",
                coin_type
            ));
        }

        let acct_i32: i32 = account_index
            .try_into()
            .context("Account index exceeds i32::MAX")?;
        let mnemonic = self.mnemonic.clone();

        tokio::task::spawn_blocking(move || {
            use bip39::Mnemonic;
            use zeroize::Zeroize;

            let mnemonic_parsed: Mnemonic = mnemonic
                .expose_secret()
                .parse()
                .map_err(|e| anyhow!("Invalid mnemonic: {}", e))?;
            let mut seed = mnemonic_parsed.to_seed("");

            let result = crate::services::address::hd_wallet::batch_derive_solana_addresses(
                &seed,
                acct_i32,
                start_index,
                count,
            );
            seed.zeroize();
            result
        })
        .await
        .context("Solana address derivation task panicked")?
    }
}

#[async_trait]
impl TransactionSigner for LocalMnemonicProvider {
    async fn sign_transaction_for_coin(
        &self,
        transaction_bytes: &[u8],
        account_index: i32,
        path_index: u32,
        coin_type: u32,
    ) -> Result<Vec<u8>> {
        use anyhow::Context;

        let mnemonic = self.mnemonic.clone();
        let raw_bytes = transaction_bytes.to_vec();

        tokio::task::spawn_blocking(move || {
            // Solana: Ed25519 signing (SLIP-0010 key derivation, NO pre-hashing)
            if coin_type == 501 {
                use bip39::Mnemonic;
                use ed25519_dalek::{Signer, SigningKey};
                use zeroize::Zeroize;

                let mnemonic_parsed: Mnemonic = mnemonic
                    .expose_secret()
                    .parse()
                    .map_err(|e| anyhow!("Invalid mnemonic: {}", e))?;
                let mut seed = mnemonic_parsed.to_seed("");

                let result =
                    crate::services::address::hd_wallet::derive_solana_private_key_from_seed(
                        &seed,
                        account_index,
                        path_index,
                    );
                seed.zeroize(); // Zeroize seed immediately after derivation
                let priv_key = result?;

                let signing_key = SigningKey::from_bytes(&*priv_key);
                let signature = signing_key.sign(&raw_bytes);
                return Ok(signature.to_bytes().to_vec()); // 64 bytes
            }

            // TRON/EVM: secp256k1 signing
            let priv_key_bytes =
                crate::services::address::hd_wallet::derive_private_key_from_mnemonic_for_coin(
                    mnemonic.expose_secret(),
                    account_index,
                    path_index,
                    coin_type,
                )?;

            use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};

            let secp = Secp256k1::new();
            let secret_key = SecretKey::from_slice(&*priv_key_bytes)
                .map_err(|e| anyhow!("Invalid derived private key: {}", e))?;

            // Hash transaction bytes with chain-appropriate algorithm
            let tx_hash: Vec<u8> = match coin_type {
                195 => {
                    use sha2::{Digest, Sha256};
                    Sha256::digest(&raw_bytes).to_vec()
                }
                60 => {
                    use sha3::{Digest, Keccak256};
                    Keccak256::digest(&raw_bytes).to_vec()
                }
                _ => return Err(anyhow!("Unsupported coin_type for signing: {}", coin_type)),
            };

            let message = Message::from_digest_slice(&tx_hash)
                .map_err(|e| anyhow!("Invalid message hash: {}", e))?;

            let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
            let (rec_id, sig_bytes) = sig.serialize_compact();

            let mut full_sig = Vec::new();
            full_sig.extend_from_slice(&sig_bytes);
            full_sig.push(rec_id.to_i32() as u8 + 27);

            Ok(full_sig)
        })
        .await
        .context("Signing task panicked")?
    }
}

// ─── KMS Envelope Provider (Production) ────────────────────────────────────

/// AWS KMS envelope encryption provider.
///
/// Stores the **encrypted** seed as ciphertext. On each signing/derivation
/// request, calls KMS `Decrypt`, uses the seed for ~1ms, then immediately
/// zeroizes it. The mnemonic never enters the production server.
///
/// # Security properties
/// - Seed in memory for ~1ms per operation (vs hours in startup-decrypt)
/// - Encryption context prevents cross-environment decryption attacks
/// - KMS client reuses TLS sessions for low-latency decrypt (~50-100ms)
/// - 3x retry with exponential backoff for network resilience
#[derive(Clone)]
pub struct KmsEnvelopeProvider {
    kms_client: aws_sdk_kms::Client,
    kms_key_id: String,
    encrypted_seed: Vec<u8>,
}

/// Encryption context key-value pairs for KMS.
/// Both encrypt and decrypt must use the same context.
const KMS_ENCRYPTION_CONTEXT_KEY: &str = "AppName";
const KMS_ENCRYPTION_CONTEXT_VALUE: &str = "IronixPay";

impl KmsEnvelopeProvider {
    /// Create a new KMS provider from pre-encrypted seed (base64-encoded ciphertext).
    ///
    /// # Arguments
    /// * `kms_key_id` - AWS KMS key ARN or alias (e.g. "alias/ironixpay-master")
    /// * `encrypted_seed_b64` - Base64-encoded ciphertext of the 64-byte BIP39 seed
    pub async fn new(kms_key_id: String, encrypted_seed_b64: &str) -> Result<Self> {
        use base64::Engine;
        use zeroize::Zeroize;

        let encrypted_seed = base64::engine::general_purpose::STANDARD
            .decode(encrypted_seed_b64)
            .map_err(|e| anyhow!("Invalid base64 in ENCRYPTED_SEED: {}", e))?;

        // Initialize AWS SDK from environment (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION)
        let aws_config = aws_config::load_from_env().await;
        let kms_client = aws_sdk_kms::Client::new(&aws_config);

        // Validate: do a test decrypt to fail fast at startup
        let provider = Self {
            kms_client,
            kms_key_id,
            encrypted_seed,
        };

        // Verify we can actually decrypt (fail fast if credentials/permissions are wrong)
        let mut test_seed = provider.kms_decrypt().await?;
        if test_seed.len() != 64 {
            test_seed.zeroize();
            return Err(anyhow!(
                "KMS decrypted seed has wrong length: expected 64 bytes, got {}",
                test_seed.len()
            ));
        }
        test_seed.zeroize();
        tracing::info!("✅ KMS Envelope Provider initialized (test decrypt successful)");

        Ok(provider)
    }

    /// Decrypt the seed from KMS with 3x retry and exponential backoff.
    async fn kms_decrypt(&self) -> Result<Vec<u8>> {
        use aws_sdk_kms::primitives::Blob;
        use std::time::Duration;

        let max_retries = 3u32;

        for attempt in 0..max_retries {
            let result = self
                .kms_client
                .decrypt()
                .key_id(&self.kms_key_id)
                .ciphertext_blob(Blob::new(self.encrypted_seed.clone()))
                .encryption_context(KMS_ENCRYPTION_CONTEXT_KEY, KMS_ENCRYPTION_CONTEXT_VALUE)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let plaintext = resp
                        .plaintext()
                        .ok_or_else(|| anyhow!("KMS returned empty plaintext"))?;
                    return Ok(plaintext.as_ref().to_vec());
                }
                Err(e) if attempt < max_retries - 1 => {
                    let backoff = Duration::from_millis(100 << attempt); // 100ms, 200ms, 400ms
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries,
                        backoff_ms = backoff.as_millis() as u64,
                        error = %e,
                        "KMS decrypt failed, retrying..."
                    );
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => {
                    return Err(anyhow!(
                        "KMS decrypt failed after {} attempts: {}",
                        max_retries,
                        e
                    ));
                }
            }
        }

        unreachable!()
    }

    /// Decrypt seed → run closure → zeroize seed. Guarantees cleanup via Drop.
    ///
    /// Uses `Zeroizing<Vec<u8>>` which zeroizes on drop, ensuring cleanup even
    /// if the closure panics or returns an error.
    async fn with_seed<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&[u8; 64]) -> Result<R> + Send,
    {
        use zeroize::Zeroizing;

        let seed_vec = Zeroizing::new(self.kms_decrypt().await?);
        let seed: &[u8; 64] = seed_vec
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("Decrypted seed is not 64 bytes"))?;

        f(seed)
        // seed_vec dropped here → automatically zeroized via Zeroizing<T> Drop impl
    }

    /// Derive a raw private key at a specific HD path, using the KMS-encrypted seed.
    ///
    /// The seed is decrypted in memory, used for derivation, then immediately zeroized.
    /// Only the derived child key (32 bytes) is returned — the master seed never leaves
    /// this method.
    ///
    /// # Arguments
    /// * `account_index` - BIP44 account index (0 for platform-reserved addresses)
    /// * `path_index` - BIP44 address index (e.g. 0=treasury, 1=gas_sponsor)
    /// * `coin_type` - BIP44 coin type (195=TRON, 60=EVM, 501=Solana)
    pub async fn derive_raw_private_key(
        &self,
        account_index: i32,
        path_index: u32,
        coin_type: u32,
    ) -> Result<Vec<u8>> {
        self.with_seed(|seed| {
            if coin_type == 501 {
                // Solana: SLIP-0010 Ed25519 derivation
                let key = crate::services::address::hd_wallet::derive_solana_private_key_from_seed(
                    seed,
                    account_index,
                    path_index,
                )?;
                return Ok(key.to_vec());
            }
            // TRON/EVM: BIP32 secp256k1 derivation
            let key = crate::services::address::hd_wallet::derive_private_key_from_seed_for_coin(
                seed,
                account_index,
                path_index,
                coin_type,
            )?;
            Ok(key.to_vec())
        })
        .await
    }
}

#[async_trait]
impl MasterKeyProvider for KmsEnvelopeProvider {
    async fn get_account_xpub_for_coin(
        &self,
        account_index: u32,
        coin_type: u32,
    ) -> Result<String> {
        // Solana uses Ed25519 (SLIP-0010) — no xpub support
        if coin_type == 501 {
            return Err(anyhow!(
                "Solana (coin_type=501) does not support xpub derivation. \
                 Use batch_derive_solana_addresses() with seed access instead."
            ));
        }

        let index_i32: i32 = account_index
            .try_into()
            .map_err(|_| anyhow!("Account index exceeds i32::MAX"))?;

        self.with_seed(|seed| {
            crate::services::address::hd_wallet::derive_account_xpub_from_seed_for_coin(
                seed, index_i32, coin_type,
            )
        })
        .await
    }

    async fn batch_derive_addresses(
        &self,
        account_index: u32,
        coin_type: u32,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<(u32, String)>> {
        if coin_type != 501 {
            return Err(anyhow!(
                "batch_derive_addresses only supports Solana (coin_type=501), got {}",
                coin_type
            ));
        }

        let acct_i32: i32 = account_index
            .try_into()
            .map_err(|_| anyhow!("Account index exceeds i32::MAX"))?;

        self.with_seed(|seed| {
            crate::services::address::hd_wallet::batch_derive_solana_addresses(
                seed,
                acct_i32,
                start_index,
                count,
            )
        })
        .await
    }
}

#[async_trait]
impl TransactionSigner for KmsEnvelopeProvider {
    async fn sign_transaction_for_coin(
        &self,
        transaction_bytes: &[u8],
        account_index: i32,
        path_index: u32,
        coin_type: u32,
    ) -> Result<Vec<u8>> {
        let raw_bytes = transaction_bytes.to_vec();

        self.with_seed(|seed| {
            use zeroize::Zeroize;

            // Solana: Ed25519 signing via SLIP-0010 derived key
            if coin_type == 501 {
                use ed25519_dalek::{Signer, SigningKey};

                let priv_key =
                    crate::services::address::hd_wallet::derive_solana_private_key_from_seed(
                        seed,
                        account_index,
                        path_index,
                    )?;
                let signing_key = SigningKey::from_bytes(&*priv_key);
                let signature = signing_key.sign(&raw_bytes);
                return Ok(signature.to_bytes().to_vec()); // 64 bytes
            }

            // TRON/EVM: secp256k1 signing
            let mut priv_key_bytes =
                crate::services::address::hd_wallet::derive_private_key_from_seed_for_coin(
                    seed,
                    account_index,
                    path_index,
                    coin_type,
                )?;

            use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};

            let secp = Secp256k1::new();
            let secret_key = SecretKey::from_slice(&*priv_key_bytes)
                .map_err(|e| anyhow!("Invalid derived private key: {}", e))?;

            let tx_hash: Vec<u8> = match coin_type {
                195 => {
                    use sha2::{Digest, Sha256};
                    Sha256::digest(&raw_bytes).to_vec()
                }
                60 => {
                    use sha3::{Digest, Keccak256};
                    Keccak256::digest(&raw_bytes).to_vec()
                }
                _ => return Err(anyhow!("Unsupported coin_type for signing: {}", coin_type)),
            };

            let message = Message::from_digest_slice(&tx_hash)
                .map_err(|e| anyhow!("Invalid message hash: {}", e))?;

            let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
            let (rec_id, sig_bytes) = sig.serialize_compact();

            let mut full_sig = Vec::with_capacity(65);
            full_sig.extend_from_slice(&sig_bytes);
            full_sig.push(rec_id.to_i32() as u8 + 27);

            priv_key_bytes.zeroize();

            Ok(full_sig)
        })
        .await
    }
}

// ─── Standalone KMS decrypt helper ─────────────────────────────────────────

/// Decrypt an arbitrary secret (e.g. Gas Sponsor Key) using AWS KMS.
/// Reuses the provided KMS client for TLS session reuse. Includes 3x retry.
/// Returns the plaintext bytes. Caller is responsible for zeroizing.
pub async fn kms_decrypt_secret(kms_key_id: &str, encrypted_b64: &str) -> Result<Vec<u8>> {
    use aws_sdk_kms::primitives::Blob;
    use base64::Engine;
    use std::time::Duration;

    let encrypted = base64::engine::general_purpose::STANDARD
        .decode(encrypted_b64)
        .map_err(|e| anyhow!("Invalid base64: {}", e))?;

    let aws_config = aws_config::load_from_env().await;
    let client = aws_sdk_kms::Client::new(&aws_config);

    let max_retries = 3u32;
    for attempt in 0..max_retries {
        let result = client
            .decrypt()
            .key_id(kms_key_id)
            .ciphertext_blob(Blob::new(encrypted.clone()))
            .encryption_context(KMS_ENCRYPTION_CONTEXT_KEY, KMS_ENCRYPTION_CONTEXT_VALUE)
            .send()
            .await;

        match result {
            Ok(resp) => {
                let plaintext = resp
                    .plaintext()
                    .ok_or_else(|| anyhow!("KMS returned empty plaintext for secret"))?;
                return Ok(plaintext.as_ref().to_vec());
            }
            Err(e) if attempt < max_retries - 1 => {
                let backoff = Duration::from_millis(100 << attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries,
                    error = %e,
                    "KMS decrypt (secret) failed, retrying..."
                );
                tokio::time::sleep(backoff).await;
            }
            Err(e) => {
                return Err(anyhow!(
                    "KMS decrypt (secret) failed after {} attempts: {}",
                    max_retries,
                    e
                ));
            }
        }
    }
    unreachable!()
}

// ─── Mock Provider (Testing) ───────────────────────────────────────────────

/// Mock implementation for testing.
/// Returns a fixed or predictably generated xpub without needing a real mnemonic.
pub struct MockMasterKeyProvider {
    /// Optional fixed return value. If None, generates a dummy string based on index.
    fixed_xpub: Option<String>,
    /// Optional fixed signature for testing signing
    fixed_signature: Option<Vec<u8>>,
}

impl MockMasterKeyProvider {
    pub fn new(fixed_xpub: Option<String>) -> Self {
        Self {
            fixed_xpub,
            fixed_signature: None,
        }
    }

    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.fixed_signature = Some(signature);
        self
    }
}

#[async_trait]
impl TransactionSigner for MockMasterKeyProvider {
    async fn sign_transaction_for_coin(
        &self,
        _transaction_bytes: &[u8],
        _account_index: i32,
        _path_index: u32,
        _coin_type: u32,
    ) -> Result<Vec<u8>> {
        if let Some(ref sig) = self.fixed_signature {
            return Ok(sig.clone());
        }
        // Ed25519 (Solana) = 64 bytes; secp256k1 (TRON/EVM) = 65 bytes
        let sig_len = if _coin_type == 501 { 64 } else { 65 };
        Ok(vec![0u8; sig_len])
    }
}

#[async_trait]
impl MasterKeyProvider for MockMasterKeyProvider {
    async fn get_account_xpub_for_coin(
        &self,
        account_index: u32,
        coin_type: u32,
    ) -> Result<String> {
        // Solana does not support xpub derivation
        if coin_type == 501 {
            return Err(anyhow!(
                "Solana (coin_type=501) does not support xpub derivation"
            ));
        }
        if let Some(ref xpub) = self.fixed_xpub {
            return Ok(xpub.clone());
        }
        Ok(format!(
            "mock_xpub_account_{}_coin_{}",
            account_index, coin_type
        ))
    }
}
