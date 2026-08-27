use ironix_pay::services::tron::address::{is_valid_address, validate_address};
use proptest::prelude::*;

proptest! {
    /// detailed test for valid addresses is hard without a generator that produces checksums,
    /// so we focus on ensuring invalid random strings are rejected and don't panic.
    #[test]
    fn test_random_string_validation_does_not_panic(s in "\\PC*") {
        // \PC* matches any unicode character string
        let result = validate_address(&s);
        // It shouldn't panic
        assert!(result.is_ok() || result.is_err());

        let bool_result = is_valid_address(&s);
        // Consistency check
        assert_eq!(result.is_ok(), bool_result);
    }

    #[test]
    fn test_length_check(s in "[a-zA-Z0-9]{0, 100}") {
        if s.len() != 34 {
            assert!(!is_valid_address(&s));
        }
    }

    #[test]
    fn test_prefix_check(s in "[^T].*") {
        // If it start with not T, it must be invalid
        assert!(!is_valid_address(&s));
    }
}
