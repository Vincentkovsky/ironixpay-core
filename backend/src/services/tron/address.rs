//! TRON address utilities
//!
//! Local validation and conversion between TRON Base58 and EVM 20-byte address formats.

use anyhow::{anyhow, Result};

/// TRON mainnet address prefix (byte)
const TRON_MAINNET_PREFIX: u8 = 0x41;

/// TRON mainnet address prefix character
const TRON_ADDRESS_PREFIX_CHAR: char = 'T';

/// Valid TRON Base58 address length
const TRON_ADDRESS_LENGTH: usize = 34;

/// Valid TRON address decoded length (prefix + 20 bytes)
const TRON_ADDRESS_BYTES_LENGTH: usize = 21;

// ============================================================================
// Address Validation
// ============================================================================

/// Address validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressValidationError {
    /// Address is empty
    Empty,
    /// Address doesn't start with 'T'
    InvalidPrefix(char),
    /// Address length is incorrect
    InvalidLength { expected: usize, actual: usize },
    /// Invalid Base58 encoding
    InvalidBase58(String),
    /// Invalid checksum
    InvalidChecksum,
    /// Invalid network prefix byte
    InvalidNetworkPrefix { expected: u8, actual: u8 },
}

impl std::fmt::Display for AddressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Address is empty"),
            Self::InvalidPrefix(c) => {
                write!(f, "Address must start with 'T', got '{}'", c)
            }
            Self::InvalidLength { expected, actual } => {
                write!(f, "Address length must be {}, got {}", expected, actual)
            }
            Self::InvalidBase58(msg) => write!(f, "Invalid Base58 encoding: {}", msg),
            Self::InvalidChecksum => write!(f, "Invalid checksum"),
            Self::InvalidNetworkPrefix { expected, actual } => {
                write!(
                    f,
                    "Invalid network prefix: expected 0x{:02x}, got 0x{:02x}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for AddressValidationError {}

/// Validate a TRON address locally (no network call)
///
/// Performs comprehensive validation:
/// - Checks address is non-empty
/// - Checks address starts with 'T' (mainnet)
/// - Checks address length is 34 characters
/// - Validates Base58 encoding and checksum
/// - Validates network prefix byte (0x41)
///
/// # Returns
/// - `Ok(())` if the address is valid
/// - `Err(AddressValidationError)` with specific error details
pub fn validate_address(addr: &str) -> std::result::Result<(), AddressValidationError> {
    // Check empty
    if addr.is_empty() {
        return Err(AddressValidationError::Empty);
    }

    // Check prefix character
    if let Some(first_char) = addr.chars().next() {
        if first_char != TRON_ADDRESS_PREFIX_CHAR {
            return Err(AddressValidationError::InvalidPrefix(first_char));
        }
    }

    // Check length
    if addr.len() != TRON_ADDRESS_LENGTH {
        return Err(AddressValidationError::InvalidLength {
            expected: TRON_ADDRESS_LENGTH,
            actual: addr.len(),
        });
    }

    // Decode Base58 with checksum verification
    let bytes = bs58::decode(addr)
        .with_check(None)
        .into_vec()
        .map_err(|e| match e {
            bs58::decode::Error::InvalidChecksum { .. } => AddressValidationError::InvalidChecksum,
            _ => AddressValidationError::InvalidBase58(e.to_string()),
        })?;

    // Check decoded length
    if bytes.len() != TRON_ADDRESS_BYTES_LENGTH {
        return Err(AddressValidationError::InvalidLength {
            expected: TRON_ADDRESS_BYTES_LENGTH,
            actual: bytes.len(),
        });
    }

    // Check network prefix byte
    if bytes[0] != TRON_MAINNET_PREFIX {
        return Err(AddressValidationError::InvalidNetworkPrefix {
            expected: TRON_MAINNET_PREFIX,
            actual: bytes[0],
        });
    }

    Ok(())
}

/// Check if a TRON address is valid (returns boolean)
///
/// This is a convenience wrapper around `validate_address` for simple checks.
pub fn is_valid_address(addr: &str) -> bool {
    validate_address(addr).is_ok()
}

/// Convert TRON Base58 address to 20-byte EVM address (without prefix)
pub fn tron_to_evm(addr: &str) -> Result<[u8; 20]> {
    let bytes = bs58::decode(addr)
        .with_check(None)
        .into_vec()
        .map_err(|e| anyhow!("Invalid Base58 address: {}", e))?;

    if bytes.len() != 21 {
        return Err(anyhow!("Invalid TRON address length: {}", bytes.len()));
    }

    if bytes[0] != TRON_MAINNET_PREFIX {
        return Err(anyhow!("Invalid TRON address prefix: 0x{:02x}", bytes[0]));
    }

    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes[1..21]);
    Ok(result)
}

/// Convert 20-byte EVM address to TRON Base58 address
pub fn evm_to_tron(addr: &[u8; 20]) -> String {
    let mut bytes = vec![TRON_MAINNET_PREFIX];
    bytes.extend_from_slice(addr);
    bs58::encode(bytes).with_check().into_string()
}

/// Convert TRON Base58 address to full hex (including 41 prefix)
pub fn to_hex(addr: &str) -> Result<String> {
    let bytes = bs58::decode(addr)
        .with_check(None)
        .into_vec()
        .map_err(|e| anyhow!("Invalid Base58 address: {}", e))?;
    Ok(hex::encode(bytes))
}

/// Convert hex address (with 41 prefix) to TRON Base58
pub fn from_hex(hex_addr: &str) -> Result<String> {
    let bytes = hex::decode(hex_addr).map_err(|e| anyhow!("Invalid hex: {}", e))?;
    if bytes.is_empty() || bytes[0] != TRON_MAINNET_PREFIX {
        return Err(anyhow!("Invalid TRON hex address prefix"));
    }
    Ok(bs58::encode(bytes).with_check().into_string())
}

/// Normalize any Tron address format (Hex, EVM-Hex, Base58) to Canonical Base58
pub fn normalize_to_base58(addr: &str) -> Option<String> {
    if addr.starts_with('T') && addr.len() == 34 {
        return Some(addr.to_string());
    }

    // Handle Hex (0x..., 41..., or raw 40 chars)
    let clean_hex = if addr.starts_with("0x") || addr.starts_with("0X") {
        &addr[2..]
    } else {
        addr
    };

    let hex_with_prefix = if clean_hex.len() == 40 {
        format!("41{}", clean_hex) // Add 41 prefix if missing
    } else if clean_hex.len() == 42 && clean_hex.starts_with("41") {
        clean_hex.to_string() // Already has 41 prefix
    } else {
        return None; // Unknown format
    };

    // Convert to Base58
    from_hex(&hex_with_prefix).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TRON_ADDR: &str = "TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9";
    const USDT_CONTRACT: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    // ========================================================================
    // Address Validation Tests
    // ========================================================================

    #[test]
    fn test_validate_valid_address() {
        assert!(validate_address(TEST_TRON_ADDR).is_ok());
        assert!(validate_address(USDT_CONTRACT).is_ok());
        assert!(is_valid_address(TEST_TRON_ADDR));
        assert!(is_valid_address(USDT_CONTRACT));
    }

    #[test]
    fn test_validate_empty_address() {
        let err = validate_address("").unwrap_err();
        assert_eq!(err, AddressValidationError::Empty);
        assert!(!is_valid_address(""));
    }

    #[test]
    fn test_validate_invalid_prefix() {
        // Ethereum address (starts with 0x)
        let err = validate_address("0x1234567890123456789012345678901234567890").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidPrefix('0')));

        // Random string starting with different letter
        let err = validate_address("AmuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidPrefix('A')));
    }

    #[test]
    fn test_validate_invalid_length() {
        // Too short
        let err = validate_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJire").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidLength { .. }));

        // Too long
        let err = validate_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9X").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidLength { .. }));
    }

    #[test]
    fn test_validate_invalid_base58() {
        // Contains invalid Base58 character (0, O, I, l are not in Base58)
        let err = validate_address("T0uA6YqfCeX8EhbfYEg5y7S4DqzSJireY9").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidBase58(_)));

        let err = validate_address("TluA6YqfCeX8EhbfYEg5y7S4DqzSJireY9").unwrap_err();
        assert!(matches!(err, AddressValidationError::InvalidBase58(_)));
    }

    #[test]
    fn test_validate_invalid_checksum() {
        // Valid format but wrong checksum (changed last character)
        let err = validate_address("TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY8").unwrap_err();
        assert!(matches!(
            err,
            AddressValidationError::InvalidChecksum | AddressValidationError::InvalidBase58(_)
        ));
    }

    #[test]
    fn test_is_valid_address_convenience() {
        assert!(is_valid_address(TEST_TRON_ADDR));
        assert!(!is_valid_address(""));
        assert!(!is_valid_address("invalid"));
        assert!(!is_valid_address("0x1234567890123456789012345678901234"));
    }

    #[test]
    fn test_validation_error_display() {
        assert_eq!(
            AddressValidationError::Empty.to_string(),
            "Address is empty"
        );
        assert_eq!(
            AddressValidationError::InvalidPrefix('X').to_string(),
            "Address must start with 'T', got 'X'"
        );
        assert!(AddressValidationError::InvalidLength {
            expected: 34,
            actual: 30
        }
        .to_string()
        .contains("34"));
    }

    // ========================================================================
    // Conversion Tests
    // ========================================================================

    #[test]
    fn test_tron_to_evm() {
        let evm = tron_to_evm(TEST_TRON_ADDR).unwrap();
        // Just verify it returns 20 bytes
        assert_eq!(evm.len(), 20);
    }

    #[test]
    fn test_to_hex() {
        let hex_str = to_hex(TEST_TRON_ADDR).unwrap();
        // TRON hex should start with 41 (mainnet prefix)
        assert!(hex_str.starts_with("41"));
        assert_eq!(hex_str.len(), 42); // 21 bytes = 42 hex chars
    }

    #[test]
    fn test_roundtrip() {
        let evm = tron_to_evm(TEST_TRON_ADDR).unwrap();
        let back = evm_to_tron(&evm);
        assert_eq!(back, TEST_TRON_ADDR);
    }

    #[test]
    fn test_hex_roundtrip() {
        let hex_str = to_hex(TEST_TRON_ADDR).unwrap();
        let back = from_hex(&hex_str).unwrap();
        assert_eq!(back, TEST_TRON_ADDR);
    }
}
