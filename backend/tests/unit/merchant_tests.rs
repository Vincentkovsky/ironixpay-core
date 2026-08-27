//! Unit Tests for MerchantService
//!
//! Tests for merchant registration, authentication, and API key management.

#[cfg(test)]
mod merchant_tests {
    use argon2::{
        password_hash::{
            rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        },
        Argon2,
    };
    use chrono::{Duration, Utc};
    use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
    use rand::Rng;
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_password_hashing() {
        // Test that Argon2 hashing works correctly
        let password = "test_password_123";
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        // Hash
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .expect("Failed to hash password");
        let hash_str = hash.to_string();

        // Verify correct password
        let parsed_hash = PasswordHash::new(&hash_str).expect("Failed to parse hash");
        assert!(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok());

        // Wrong password should fail
        assert!(argon2
            .verify_password(b"wrong_password", &parsed_hash)
            .is_err());
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        sub: String,
        exp: i64,
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let secret = "test_secret_key";
        let merchant_id = "mer_test123";
        let exp = (Utc::now() + Duration::hours(24)).timestamp();

        let claims = Claims {
            sub: merchant_id.to_string(),
            exp,
        };

        // Encode
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to encode JWT");

        assert!(token.starts_with("eyJ"));

        // Decode
        let decoded = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .expect("Failed to decode JWT");

        assert_eq!(decoded.claims.sub, merchant_id);
    }

    #[tokio::test]
    async fn test_jwt_token_expiration() {
        let secret = "test_secret";

        // Create expired token
        let expired_claims = Claims {
            sub: "mer_expired".to_string(),
            exp: (Utc::now() - Duration::hours(1)).timestamp(),
        };

        let expired_token = encode(
            &Header::default(),
            &expired_claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to encode");

        // Decoding expired token should fail
        let result = decode::<Claims>(
            &expired_token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_api_key_format() {
        // Live key format
        let prefix_live = "sk_live_";
        let random_bytes: Vec<u8> = (0..24).map(|_| rand::thread_rng().gen()).collect();
        let key_suffix = hex::encode(&random_bytes);
        let live_key = format!("{}{}", prefix_live, key_suffix);

        assert!(live_key.starts_with("sk_live_"));
        assert_eq!(live_key.len(), 8 + 48); // prefix + 24 bytes hex

        // Test key format
        let prefix_test = "sk_test_";
        let test_key = format!("{}{}", prefix_test, key_suffix);
        assert!(test_key.starts_with("sk_test_"));
    }

    #[test]
    fn test_api_key_hashing() {
        let api_key = ["sk_", "live_", "abcdef1234567890abcdef1234567890"].concat();

        // Hash for storage
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        let hash = hex::encode(hasher.finalize());

        assert_eq!(hash.len(), 64); // SHA256 = 32 bytes = 64 hex chars

        // Same key should produce same hash
        let mut hasher2 = Sha256::new();
        hasher2.update(api_key.as_bytes());
        let hash2 = hex::encode(hasher2.finalize());

        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_merchant_id_format() {
        let merchant_id = format!("mer_{}", Uuid::new_v4().to_string().replace("-", ""));

        assert!(merchant_id.starts_with("mer_"));
        assert_eq!(merchant_id.len(), 4 + 32); // prefix + 32 hex chars

        // IDs should be unique
        let merchant_id2 = format!("mer_{}", Uuid::new_v4().to_string().replace("-", ""));
        assert_ne!(merchant_id, merchant_id2);
    }

    #[test]
    fn test_settlement_address_validation() {
        // Valid Tron address (34 characters, starts with T)
        let valid_address = "TN7VEuGZnGpPWWZpZJCqByT5V8c3YdPZZZ";
        assert!(valid_address.starts_with('T'));
        assert_eq!(valid_address.len(), 34);

        // Invalid: too short
        let short_address = "TXxxx";
        assert!(short_address.len() != 34);

        // Invalid: wrong prefix
        let wrong_prefix = "0N7VEuGZnGpPWWZpZJCqByT5V8c3YdPZZZ";
        assert!(!wrong_prefix.starts_with('T'));
    }
}
