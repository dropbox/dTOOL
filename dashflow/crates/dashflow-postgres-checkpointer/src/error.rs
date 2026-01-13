//! Error types for PostgreSQL checkpointer

use thiserror::Error;

/// Errors that can occur when using the PostgreSQL checkpointer
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// PostgreSQL connection or query error
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] tokio_postgres::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Checkpoint not found
    #[error("Checkpoint not found: {0}")]
    NotFound(String),

    /// Generic error
    #[error("{0}")]
    Generic(String),
}

/// Result type for PostgreSQL checkpointer operations
///
/// This type alias is provided for convenience when working with the error types
/// defined in this module. Used in tests and available for external consumers.
#[allow(dead_code)] // Public API: Used in tests; available for external use
pub type Result<T> = std::result::Result<T, Error>;

/// Convert PostgreSQL checkpointer errors to DashFlow errors
impl From<Error> for dashflow::Error {
    fn from(err: Error) -> Self {
        use dashflow::error::CheckpointError;
        let checkpoint_err = match err {
            Error::Postgres(e) => CheckpointError::ConnectionLost {
                backend: "postgres".to_string(),
                reason: e.to_string(),
            },
            Error::Serialization(e) => CheckpointError::SerializationFailed {
                reason: e.to_string(),
            },
            Error::Json(e) => CheckpointError::SerializationFailed {
                reason: format!("JSON: {}", e),
            },
            Error::NotFound(id) => CheckpointError::NotFound {
                checkpoint_id: id,
            },
            Error::Generic(msg) => CheckpointError::Other(msg),
        };
        dashflow::Error::Checkpoint(checkpoint_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Error Display Tests ==========

    #[test]
    fn test_error_not_found_display() {
        let err = Error::NotFound("checkpoint-123".to_string());
        assert_eq!(err.to_string(), "Checkpoint not found: checkpoint-123");
    }

    #[test]
    fn test_error_generic_display() {
        let err = Error::Generic("Something went wrong".to_string());
        assert_eq!(err.to_string(), "Something went wrong");
    }

    #[test]
    fn test_error_serialization_display() {
        let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        let bincode_err: bincode::Error =
            bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);
        let msg = err.to_string();
        assert!(msg.contains("Serialization error"));
    }

    #[test]
    fn test_error_json_display() {
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let err = Error::Json(json_err);
        let msg = err.to_string();
        assert!(msg.contains("JSON error"));
    }

    // ========== Error Debug Tests ==========

    #[test]
    fn test_error_not_found_debug() {
        let err = Error::NotFound("cp-abc".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
        assert!(debug.contains("cp-abc"));
    }

    #[test]
    fn test_error_generic_debug() {
        let err = Error::Generic("test error".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Generic"));
        assert!(debug.contains("test error"));
    }

    #[test]
    fn test_error_serialization_debug() {
        let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        let bincode_err: bincode::Error =
            bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);
        let debug = format!("{:?}", err);
        assert!(debug.contains("Serialization"));
    }

    #[test]
    fn test_error_json_debug() {
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err = Error::Json(json_err);
        let debug = format!("{:?}", err);
        assert!(debug.contains("Json"));
    }

    // ========== From<Error> for dashflow::Error Tests ==========

    #[test]
    fn test_conversion_not_found() {
        let err = Error::NotFound("checkpoint-456".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains("checkpoint-456"),
            "Expected message to contain checkpoint ID, got: {}",
            msg
        );
    }

    #[test]
    fn test_conversion_generic() {
        let err = Error::Generic("custom error message".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains("custom error message"),
            "Expected message to contain generic error, got: {}",
            msg
        );
    }

    #[test]
    fn test_conversion_serialization() {
        // Create a bincode error by trying to deserialize invalid data
        let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        let bincode_err: bincode::Error =
            bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);

        // Convert to dashflow error
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();

        // The error should be converted to a serialization failed checkpoint error
        assert!(
            msg.to_lowercase().contains("serial"),
            "Expected serialization error, got: {}",
            msg
        );
    }

    #[test]
    fn test_conversion_json() {
        // Create a serde_json error by parsing invalid JSON
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();
        let err = Error::Json(json_err);

        // Convert to dashflow error
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();

        // The error should contain "JSON" prefix from our conversion
        assert!(
            msg.contains("JSON") || msg.to_lowercase().contains("serial"),
            "Expected JSON serialization error, got: {}",
            msg
        );
    }

    #[test]
    fn test_conversion_preserves_checkpoint_id() {
        let checkpoint_id = "unique-id-xyz-789";
        let err = Error::NotFound(checkpoint_id.to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains(checkpoint_id),
            "Checkpoint ID should be preserved, got: {}",
            msg
        );
    }

    // ========== Result Type Alias Tests ==========

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let result: Result<i32> = Err(Error::NotFound("test".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_result_ok_with_string() {
        let result: Result<String> = Ok("hello".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_result_ok_with_unit() {
        let result: Result<()> = Ok(());
        assert!(result.is_ok());
    }

    #[test]
    fn test_result_err_with_generic() {
        let result: Result<()> = Err(Error::Generic("failure".to_string()));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failure"));
    }

    #[test]
    fn test_result_map_ok() {
        let result: Result<i32> = Ok(10);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped.unwrap(), 20);
    }

    #[test]
    fn test_result_map_err_passes_through() {
        let result: Result<i32> = Err(Error::NotFound("x".to_string()));
        let mapped = result.map(|x| x * 2);
        assert!(mapped.is_err());
    }

    // ========== Error non_exhaustive Tests ==========

    #[test]
    fn test_error_is_non_exhaustive() {
        // This test documents that the enum is non_exhaustive
        // by demonstrating we can construct all known variants
        let errors: Vec<Error> = vec![
            Error::NotFound("id".to_string()),
            Error::Generic("msg".to_string()),
        ];
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_all_error_variants_constructible() {
        // Test that all public variants can be constructed
        let _not_found = Error::NotFound("id".to_string());
        let _generic = Error::Generic("msg".to_string());

        // Serialization and Json require actual errors from their crates
        let invalid_data: &[u8] = &[0xFF];
        let bincode_err = bincode::deserialize::<u64>(invalid_data).unwrap_err();
        let _serialization = Error::Serialization(bincode_err);

        let json_err = serde_json::from_str::<()>("invalid").unwrap_err();
        let _json = Error::Json(json_err);
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_error_not_found_empty_id() {
        let err = Error::NotFound(String::new());
        assert_eq!(err.to_string(), "Checkpoint not found: ");
    }

    #[test]
    fn test_error_generic_empty_message() {
        let err = Error::Generic(String::new());
        assert_eq!(err.to_string(), "");
    }

    #[test]
    fn test_error_not_found_special_chars() {
        let err = Error::NotFound("checkpoint/with:special@chars".to_string());
        assert!(err.to_string().contains("checkpoint/with:special@chars"));
    }

    #[test]
    fn test_error_generic_multiline() {
        let err = Error::Generic("line1\nline2\nline3".to_string());
        let msg = err.to_string();
        assert!(msg.contains("line1"));
        assert!(msg.contains("line2"));
        assert!(msg.contains("line3"));
    }

    #[test]
    fn test_error_not_found_uuid_format() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let err = Error::NotFound(uuid.to_string());
        assert!(err.to_string().contains(uuid));
    }

    #[test]
    fn test_error_not_found_very_long_id() {
        let long_id = "x".repeat(1000);
        let err = Error::NotFound(long_id.clone());
        assert!(err.to_string().contains(&long_id));
    }

    #[test]
    fn test_error_generic_with_unicode() {
        let err = Error::Generic("Error: データベース接続失敗".to_string());
        assert!(err.to_string().contains("データベース"));
    }

    #[test]
    fn test_error_not_found_with_whitespace() {
        let err = Error::NotFound("  spaced-id  ".to_string());
        assert!(err.to_string().contains("  spaced-id  "));
    }

    // ========== From Trait Tests ==========

    #[test]
    fn test_from_bincode_error() {
        let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let bincode_err: bincode::Error =
            bincode::deserialize::<Vec<String>>(invalid_data).unwrap_err();

        // Test From trait
        let err: Error = bincode_err.into();
        assert!(matches!(err, Error::Serialization(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err: serde_json::Error =
            serde_json::from_str::<Vec<i32>>("[1, 2, three]").unwrap_err();

        // Test From trait
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }

    // ========== Error Chain Tests ==========

    #[test]
    fn test_error_source_not_found() {
        use std::error::Error as StdError;
        let err = Error::NotFound("id".to_string());
        // NotFound has no source
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_generic() {
        use std::error::Error as StdError;
        let err = Error::Generic("msg".to_string());
        // Generic has no source
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_serialization() {
        use std::error::Error as StdError;
        let invalid_data: &[u8] = &[0xFF];
        let bincode_err = bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);
        // Serialization has a source (the underlying bincode error)
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_json() {
        use std::error::Error as StdError;
        let json_err = serde_json::from_str::<i32>("not-a-number").unwrap_err();
        let err = Error::Json(json_err);
        // Json has a source (the underlying serde_json error)
        assert!(err.source().is_some());
    }

    // ========== Conversion Round-trip Tests ==========

    #[test]
    fn test_conversion_roundtrip_not_found() {
        let original_id = "test-checkpoint-id";
        let err = Error::NotFound(original_id.to_string());
        let dashflow_err: dashflow::Error = err.into();

        // The original ID should still be accessible in some form
        let msg = dashflow_err.to_string();
        assert!(msg.contains(original_id));
    }

    #[test]
    fn test_conversion_roundtrip_generic() {
        let original_msg = "original error message";
        let err = Error::Generic(original_msg.to_string());
        let dashflow_err: dashflow::Error = err.into();

        let msg = dashflow_err.to_string();
        assert!(msg.contains(original_msg));
    }

    // ========== Error Message Quality Tests ==========

    #[test]
    fn test_error_messages_are_informative() {
        // NotFound should clearly indicate what wasn't found
        let not_found = Error::NotFound("cp-123".to_string());
        assert!(not_found.to_string().contains("Checkpoint not found"));
        assert!(not_found.to_string().contains("cp-123"));

        // Generic should pass through the message
        let generic = Error::Generic("Connection timed out after 30s".to_string());
        assert!(generic.to_string().contains("timed out"));
        assert!(generic.to_string().contains("30s"));
    }

    #[test]
    fn test_serialization_error_is_descriptive() {
        let invalid_data: &[u8] = &[0x00, 0x01, 0x02];
        let bincode_err = bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);

        let msg = err.to_string();
        // Should mention it's a serialization error
        assert!(msg.contains("Serialization"));
    }

    #[test]
    fn test_json_error_is_descriptive() {
        let json_err = serde_json::from_str::<i32>("\"not a number\"").unwrap_err();
        let err = Error::Json(json_err);

        let msg = err.to_string();
        // Should mention it's a JSON error
        assert!(msg.contains("JSON"));
    }

    // ========== Pattern Matching Tests ==========

    #[test]
    fn test_pattern_matching_not_found() {
        let err = Error::NotFound("test-id".to_string());
        match err {
            Error::NotFound(id) => assert_eq!(id, "test-id"),
            _ => panic!("Expected NotFound variant"),
        }
    }

    #[test]
    fn test_pattern_matching_generic() {
        let err = Error::Generic("test message".to_string());
        match err {
            Error::Generic(msg) => assert_eq!(msg, "test message"),
            _ => panic!("Expected Generic variant"),
        }
    }

    #[test]
    fn test_pattern_matching_serialization() {
        let invalid_data: &[u8] = &[0xFF];
        let bincode_err = bincode::deserialize::<String>(invalid_data).unwrap_err();
        let err = Error::Serialization(bincode_err);
        match err {
            Error::Serialization(_) => {} // Success
            _ => panic!("Expected Serialization variant"),
        }
    }

    #[test]
    fn test_pattern_matching_json() {
        let json_err = serde_json::from_str::<()>("invalid").unwrap_err();
        let err = Error::Json(json_err);
        match err {
            Error::Json(_) => {} // Success
            _ => panic!("Expected Json variant"),
        }
    }

    // ========== Realistic Error Scenarios ==========

    #[test]
    fn test_scenario_checkpoint_not_found() {
        // Simulate loading a checkpoint that doesn't exist
        fn load_checkpoint(id: &str) -> Result<String> {
            if id == "exists" {
                Ok("checkpoint data".to_string())
            } else {
                Err(Error::NotFound(id.to_string()))
            }
        }

        assert!(load_checkpoint("exists").is_ok());
        assert!(load_checkpoint("missing").is_err());
    }

    #[test]
    fn test_scenario_serialization_failure() {
        // Simulate serializing invalid data
        fn deserialize_state(data: &[u8]) -> Result<u64> {
            bincode::deserialize(data).map_err(Error::from)
        }

        let valid_data = bincode::serialize(&42u64).unwrap();
        assert!(deserialize_state(&valid_data).is_ok());

        let invalid_data = &[0xFF, 0xFF];
        assert!(deserialize_state(invalid_data).is_err());
    }

    #[test]
    fn test_scenario_json_metadata_parsing() {
        // Simulate parsing checkpoint metadata
        fn parse_metadata(json: &str) -> Result<serde_json::Value> {
            serde_json::from_str(json).map_err(Error::from)
        }

        assert!(parse_metadata(r#"{"key": "value"}"#).is_ok());
        assert!(parse_metadata("not json").is_err());
    }

    #[test]
    fn test_scenario_generic_error_wrapping() {
        // Simulate wrapping an unknown error
        fn operation_that_fails() -> Result<()> {
            Err(Error::Generic("Unknown database error occurred".to_string()))
        }

        let result = operation_that_fails();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown database"));
    }

    // ========== Different JSON Error Types ==========

    #[test]
    fn test_json_syntax_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad syntax").unwrap_err();
        let err = Error::Json(json_err);
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn test_json_type_mismatch() {
        let json_err = serde_json::from_str::<i32>("\"not an int\"").unwrap_err();
        let err = Error::Json(json_err);
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn test_json_eof_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{\"key\":").unwrap_err();
        let err = Error::Json(json_err);
        assert!(err.to_string().contains("JSON"));
    }

    // ========== Different Bincode Error Types ==========

    #[test]
    fn test_bincode_invalid_bool() {
        // Bools can only be 0 or 1
        let invalid: &[u8] = &[2];
        let bincode_err = bincode::deserialize::<bool>(invalid).unwrap_err();
        let err = Error::Serialization(bincode_err);
        assert!(err.to_string().contains("Serialization"));
    }

    #[test]
    fn test_bincode_string_too_long() {
        // String length prefix that exceeds actual data
        let invalid: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x41];
        let bincode_err = bincode::deserialize::<String>(invalid).unwrap_err();
        let err = Error::Serialization(bincode_err);
        assert!(err.to_string().contains("Serialization"));
    }
}
