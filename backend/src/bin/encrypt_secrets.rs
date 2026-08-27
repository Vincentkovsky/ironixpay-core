//! CLI tool to encrypt secrets using AWS KMS.
//!
//! Usage:
//!   # Encrypt a mnemonic (derives seed first, then encrypts the 64-byte seed)
//!   cargo run --bin encrypt_secrets -- --key-id alias/ironixpay-master --mnemonic "word1 word2 ..."
//!
//!   # Encrypt a hex-encoded secret (e.g. gas sponsor private key)
//!   cargo run --bin encrypt_secrets -- --key-id alias/ironixpay-master --hex-secret "deadbeef..."

use anyhow::{anyhow, Result};
use aws_sdk_kms::primitives::Blob;
use base64::Engine;
use clap::Parser;

/// Encryption context — must match the runtime decrypt context in KmsEnvelopeProvider.
const KMS_ENCRYPTION_CONTEXT_KEY: &str = "AppName";
const KMS_ENCRYPTION_CONTEXT_VALUE: &str = "IronixPay";

#[derive(Parser)]
#[command(name = "encrypt_secrets", about = "Encrypt secrets using AWS KMS")]
struct Args {
    /// AWS KMS key ID or alias (e.g. "alias/ironixpay-master")
    #[arg(long)]
    key_id: String,

    /// AWS region (e.g. "ap-southeast-1")
    #[arg(long, default_value = "ap-southeast-1")]
    region: String,

    /// BIP39 mnemonic to encrypt.
    /// The tool derives the 64-byte seed (via PBKDF2) and encrypts the seed, not the mnemonic.
    #[arg(long, conflicts_with = "hex_secret")]
    mnemonic: Option<String>,

    /// Hex-encoded secret to encrypt (e.g. gas sponsor private key)
    #[arg(long, conflicts_with = "mnemonic")]
    hex_secret: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Determine plaintext to encrypt
    let (plaintext_bytes, label) = if let Some(ref mnemonic) = args.mnemonic {
        // Parse mnemonic → derive seed (64 bytes)
        let mnemonic_parsed: bip39::Mnemonic = mnemonic
            .parse()
            .map_err(|e| anyhow!("Invalid BIP39 mnemonic: {}", e))?;

        let seed = mnemonic_parsed.to_seed("");
        assert_eq!(seed.len(), 64, "BIP39 seed must be 64 bytes");

        eprintln!("✅ Parsed mnemonic → derived 64-byte seed");
        (seed.to_vec(), "ENCRYPTED_SEED")
    } else if let Some(ref hex) = args.hex_secret {
        let bytes = hex::decode(hex).map_err(|e| anyhow!("Invalid hex: {}", e))?;
        eprintln!("✅ Parsed hex secret ({} bytes)", bytes.len());
        (bytes, "ENCRYPTED_SECRET")
    } else {
        return Err(anyhow!("Must specify --mnemonic or --hex-secret"));
    };

    // Initialize AWS SDK
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_sdk_kms::config::Region::new(args.region))
        .load()
        .await;
    let client = aws_sdk_kms::Client::new(&aws_config);

    // Encrypt via KMS
    eprintln!("🔐 Encrypting with KMS key: {}", args.key_id);
    let resp = client
        .encrypt()
        .key_id(&args.key_id)
        .plaintext(Blob::new(plaintext_bytes))
        .encryption_context(KMS_ENCRYPTION_CONTEXT_KEY, KMS_ENCRYPTION_CONTEXT_VALUE)
        .send()
        .await
        .map_err(|e| anyhow!("KMS encrypt failed: {}", e))?;

    let ciphertext = resp
        .ciphertext_blob()
        .ok_or_else(|| anyhow!("KMS returned empty ciphertext"))?;

    let b64_ciphertext = base64::engine::general_purpose::STANDARD.encode(ciphertext.as_ref());

    // Output for .env file
    eprintln!("\n✅ Encryption successful! Add this to your production .env:\n");
    println!("{}=\"{}\"", label, b64_ciphertext);

    // Verify: decrypt and check roundtrip
    eprintln!("\n🔄 Verifying roundtrip decrypt...");
    let verify_resp = client
        .decrypt()
        .key_id(&args.key_id)
        .ciphertext_blob(Blob::new(
            base64::engine::general_purpose::STANDARD
                .decode(&b64_ciphertext)
                .unwrap(),
        ))
        .encryption_context(KMS_ENCRYPTION_CONTEXT_KEY, KMS_ENCRYPTION_CONTEXT_VALUE)
        .send()
        .await
        .map_err(|e| anyhow!("Roundtrip decrypt failed: {}", e))?;

    let decrypted = verify_resp
        .plaintext()
        .ok_or_else(|| anyhow!("Roundtrip returned empty plaintext"))?;

    if args.mnemonic.is_some() {
        assert_eq!(
            decrypted.as_ref().len(),
            64,
            "Roundtrip seed length mismatch"
        );
    }

    eprintln!(
        "✅ Roundtrip verification successful ({} bytes)",
        decrypted.as_ref().len()
    );
    Ok(())
}
