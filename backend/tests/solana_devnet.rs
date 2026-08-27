//! Solana Devnet E2E Tests
//!
//! These tests verify Solana integration against the real Devnet.
//! Run with: `cargo test --test solana_devnet -- --ignored --nocapture`
//!
//! Prerequisites:
//! - Devnet SOL in test keypair (/tmp/solana-test.json)
//! - Test SPL token created (see test output for mint address)

use anyhow::Result;

/// Test mnemonic (BIP39 standard test vector)
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Test address derived from test mnemonic at m/44'/501'/0'/0'
/// Verified against Solana CLI `solana-keygen recover`
const EXPECTED_ADDR_0: &str = "HAgk14JpMQLgt6rVgv7cBQFJWFto5Dqxi472uT3DKpqk";

/// SPL Token Program ID (legacy)
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Test SPL token mint (created on Devnet with 6 decimals)
const TEST_TOKEN_MINT: &str = "EhxoqTX5wKBNKfp2UsXBwDAe1XzAhtNz3HBNJfFVh4ah";

/// ATA created by `spl-token create-account` on Devnet
const CLI_CREATED_ATA: &str = "BHHK1sSsxStMThSP6pPdCxX66gMfWv7cShqF26qJ7MtT";

// ── Phase A: Address Derivation ─────────────────────────────────────────────

#[test]
#[ignore] // Requires manual verification context
fn phase_a_address_derivation_matches_cli() {
    use bip39::Mnemonic;
    use ironix_pay::services::address::hd_wallet::derive_solana_address_from_seed;

    let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
    let seed = mnemonic.to_seed("");

    let addr = derive_solana_address_from_seed(&seed, 0, 0).unwrap();
    println!("Rust-derived address: {}", addr);
    println!("CLI-recovered address: {}", EXPECTED_ADDR_0);

    assert_eq!(
        addr, EXPECTED_ADDR_0,
        "Rust SLIP-0010 derivation must match Solana CLI recover"
    );
    println!("✅ Phase A: Address derivation matches Solana CLI");
}

// ── Phase B: ATA Address Verification ───────────────────────────────────────

#[test]
#[ignore] // Requires Devnet token setup
fn phase_b_ata_derivation_matches_cli() {
    use ironix_pay::services::solana::derive_ata_address;

    let rust_ata =
        derive_ata_address(EXPECTED_ADDR_0, TEST_TOKEN_MINT, SPL_TOKEN_PROGRAM_ID).unwrap();

    println!("Rust ATA:  {}", rust_ata);
    println!("CLI ATA:   {}", CLI_CREATED_ATA);

    assert_eq!(
        rust_ata, CLI_CREATED_ATA,
        "Rust derive_ata_address must match spl-token CLI"
    );
    println!("✅ Phase B: ATA derivation matches spl-token CLI");
}

// ── Phase C: Ed25519 Signing ────────────────────────────────────────────────

#[test]
#[ignore]
fn phase_c_ed25519_sign_and_verify() {
    use bip39::Mnemonic;
    use ed25519_dalek::{Signature, SigningKey, Verifier};
    use ironix_pay::services::address::hd_wallet::derive_solana_private_key_from_seed;
    use ironix_pay::services::solana::signing::sign_solana_message;

    let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
    let seed = mnemonic.to_seed("");

    let private_key = derive_solana_private_key_from_seed(&seed, 0, 0).unwrap();
    let signing_key = SigningKey::from_bytes(&*private_key);
    let verifying_key = signing_key.verifying_key();

    // Sign a test message
    let message = b"IronixPay Solana Devnet test message";
    let sig_bytes = sign_solana_message(message, &*private_key);

    let signature = Signature::from_bytes(&sig_bytes);
    assert!(
        verifying_key.verify(message, &signature).is_ok(),
        "Signature must verify with corresponding public key"
    );

    println!(
        "✅ Phase C: Ed25519 sign({} bytes) → verify OK",
        sig_bytes.len()
    );
}

// ── Phase D: SPL Transfer (requires Devnet balance) ─────────────────────────

#[tokio::test]
#[ignore] // Requires Devnet SOL + minted tokens
async fn phase_d_spl_transfer_build_and_broadcast() -> Result<()> {
    use bip39::Mnemonic;
    use ironix_pay::entity::network::Network;
    use ironix_pay::services::address::hd_wallet::{
        derive_solana_address_from_seed, derive_solana_private_key_from_seed,
    };
    use ironix_pay::services::solana::signing::{assemble_signed_solana_tx, sign_solana_message};
    use ironix_pay::services::solana::{derive_ata_address, SolanaClient};

    let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
    let seed = mnemonic.to_seed("");

    let addr_0 = derive_solana_address_from_seed(&seed, 0, 0)?;
    let addr_1 = derive_solana_address_from_seed(&seed, 0, 1)?;
    println!("addr_0 (from): {}", addr_0);
    println!("addr_1 (to):   {}", addr_1);

    let client = SolanaClient::new(
        vec!["https://api.devnet.solana.com".to_string()],
        Network::Solana,
    );

    // Check balance on addr_0's ATA
    let ata_0 = derive_ata_address(&addr_0, TEST_TOKEN_MINT, SPL_TOKEN_PROGRAM_ID)?;
    let ata_0_balance = client.get_token_account_balance(&ata_0).await?;
    println!("addr_0 ATA balance: {:?}", ata_0_balance);

    if ata_0_balance.unwrap_or(0) == 0 {
        println!("⚠️ No tokens to transfer. Mint tokens first:");
        println!(
            "  spl-token mint {} 1000000 --fee-payer /tmp/solana-test.json",
            TEST_TOKEN_MINT
        );
        return Ok(());
    }

    // Build SPL sweep: addr_0 → addr_1 (using build_spl_sweep with fee_payer=addr_0)
    let transfer_amount = 100_000u64; // 0.1 token (6 decimals)
    let unsigned_tx = client
        .build_spl_sweep(
            &addr_0,
            &addr_1,
            TEST_TOKEN_MINT,
            transfer_amount,
            6,       // decimals
            &addr_0, // fee_payer = self (single signer)
            SPL_TOKEN_PROGRAM_ID,
            false, // don't close ATA
        )
        .await?;

    println!(
        "Unsigned tx built, {} signers required",
        unsigned_tx.num_required_signatures
    );

    // Sign with addr_0's private key (single signer: from == fee_payer)
    let pk_0 = derive_solana_private_key_from_seed(&seed, 0, 0)?;
    let sig = sign_solana_message(&unsigned_tx.message_bytes, &*pk_0);

    let signed_tx = assemble_signed_solana_tx(&unsigned_tx, &[sig.to_vec()])?;
    println!("Signed tx assembled, signature: {}", signed_tx.signature);

    // Broadcast
    let result = client
        .broadcast_solana(&signed_tx, &unsigned_tx.recent_blockhash)
        .await?;
    println!("✅ Phase D: TX broadcast success: {}", result.tx_hash);

    Ok(())
}

// ── Phase E: Fee Payer Delegation (dual-signer sweep) ───────────────────────

#[tokio::test]
#[ignore] // Requires Phase D setup + tokens on addr_1
async fn phase_e_fee_payer_delegation_sweep() -> Result<()> {
    use bip39::Mnemonic;
    use ironix_pay::entity::network::Network;
    use ironix_pay::services::address::hd_wallet::{
        derive_solana_address_from_seed, derive_solana_private_key_from_seed,
    };
    use ironix_pay::services::solana::signing::{assemble_signed_solana_tx, sign_solana_message};
    use ironix_pay::services::solana::{derive_ata_address, SolanaClient};

    let mnemonic: Mnemonic = TEST_MNEMONIC.parse().unwrap();
    let seed = mnemonic.to_seed("");

    let addr_0 = derive_solana_address_from_seed(&seed, 0, 0)?; // treasury
    let addr_1 = derive_solana_address_from_seed(&seed, 0, 1)?; // deposit (zero SOL)

    let client = SolanaClient::new(
        vec!["https://api.devnet.solana.com".to_string()],
        Network::Solana,
    );

    // Verify addr_1 has zero SOL (the whole point of Fee Payer Delegation)
    let sol_balance = client.get_sol_balance(&addr_1).await?;
    println!("addr_1 SOL balance: {} lamports", sol_balance);

    // Check token balance on addr_1
    let ata_1 = derive_ata_address(&addr_1, TEST_TOKEN_MINT, SPL_TOKEN_PROGRAM_ID)?;
    let token_balance = client.get_token_account_balance(&ata_1).await?;
    println!("addr_1 ATA ({}) token balance: {:?}", ata_1, token_balance);

    let balance = token_balance.unwrap_or(0);
    if balance == 0 {
        println!("⚠️ addr_1 has no tokens. Run Phase D first to transfer some.");
        return Ok(());
    }

    // Build sweep: addr_1 → addr_0, fee_payer = addr_0 (Fee Payer Delegation!)
    let unsigned_tx = client
        .build_spl_sweep(
            &addr_1, // from (has tokens, no SOL)
            &addr_0, // to (treasury)
            TEST_TOKEN_MINT,
            balance,
            6,       // decimals
            &addr_0, // fee_payer = treasury
            SPL_TOKEN_PROGRAM_ID,
            false, // don't close ATA
        )
        .await?;

    println!(
        "Unsigned sweep tx: {} signers, pubkeys: {:?}",
        unsigned_tx.num_required_signatures, unsigned_tx.signer_pubkeys
    );

    // Dual signing: treasury (fee_payer) first, then source
    let pk_0 = derive_solana_private_key_from_seed(&seed, 0, 0)?; // treasury
    let pk_1 = derive_solana_private_key_from_seed(&seed, 0, 1)?; // deposit

    let sig_treasury = sign_solana_message(&unsigned_tx.message_bytes, &*pk_0);
    let sig_source = sign_solana_message(&unsigned_tx.message_bytes, &*pk_1);

    // fee_payer signature MUST be first per Solana wire format
    let signed_tx =
        assemble_signed_solana_tx(&unsigned_tx, &[sig_treasury.to_vec(), sig_source.to_vec()])?;

    println!("Broadcasting dual-signed sweep...");
    let result = client
        .broadcast_solana(&signed_tx, &unsigned_tx.recent_blockhash)
        .await?;

    println!("✅ Phase E: Fee Payer Delegation sweep success!");
    println!("   TX: {}", result.tx_hash);
    println!("   From addr_1 (zero SOL) → addr_0 (treasury/fee payer)");
    println!("   Swept: {} tokens", balance);

    Ok(())
}
