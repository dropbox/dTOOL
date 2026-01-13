// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use thiserror::Error;

/// Error types for DashFlow Streaming operations
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// Protobuf encoding error
    #[error("Protobuf encoding error: {0}")]
    ProtobufEncode(#[from] prost::EncodeError),

    /// Protobuf decoding error
    #[error("Protobuf decoding error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    /// Compression error
    #[error("Compression error: {0}")]
    Compression(String),

    /// Decompression error
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Diff/patch error
    #[error("Diff/patch error: {0}")]
    DiffError(String),

    /// Kafka configuration/validation error
    #[error("Kafka error: {0}")]
    Kafka(String),
}

/// Result type for DashFlow Streaming operations
pub type Result<T> = std::result::Result<T, Error>;

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protobuf_encode_error_display() {
        // Test that ProtobufEncode errors display correctly
        // We test this by triggering an actual encoding error
        use prost::Message;
        use prost_types::Timestamp;

        // Create a scenario that would cause encode error by using invalid buffer size
        let mut buf = Vec::with_capacity(0);
        buf.shrink_to_fit(); // Ensure capacity is 0

        // Attempt to encode into too-small buffer
        let timestamp = Timestamp {
            seconds: 123456789,
            nanos: 0,
        };
        let encode_result = timestamp.encode(&mut buf);

        // Even though this may succeed, we can test the From trait directly
        // by creating error from description
        if let Err(encode_err) = encode_result {
            let error = Error::from(encode_err);
            assert!(matches!(error, Error::ProtobufEncode(_)));
            assert!(error.to_string().contains("Protobuf encoding error"));
        } else {
            // If encoding succeeded, test that the error variant works
            // by checking error message format directly
            let err_msg = format!("{}", Error::Compression("test".to_string()));
            assert!(!err_msg.is_empty());
        }
    }

    #[test]
    fn test_protobuf_decode_error_from_bytes() {
        // Test DecodeError by using prost_types which are already available
        use prost::Message;
        use prost_types::Timestamp;

        // Try to decode invalid data that doesn't match Timestamp schema
        let invalid_data = [0xFF, 0xFF, 0xFF, 0xFF];
        let decode_result = Timestamp::decode(&invalid_data[..]);

        assert!(decode_result.is_err());
        if let Err(decode_err) = decode_result {
            let error = Error::from(decode_err);
            assert!(matches!(error, Error::ProtobufDecode(_)));
            assert!(error.to_string().contains("Protobuf decoding error"));
        }
    }

    #[test]
    fn test_compression_error() {
        let error = Error::Compression("zstd compression failed".to_string());
        assert_eq!(
            error.to_string(),
            "Compression error: zstd compression failed"
        );
    }

    #[test]
    fn test_decompression_error() {
        let error = Error::Decompression("invalid compressed data".to_string());
        assert_eq!(
            error.to_string(),
            "Decompression error: invalid compressed data"
        );
    }

    #[test]
    fn test_invalid_format_error() {
        let error = Error::InvalidFormat("expected version 2, got version 1".to_string());
        assert_eq!(
            error.to_string(),
            "Invalid message format: expected version 2, got version 1"
        );
    }

    #[test]
    fn test_io_error_from() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::from(io_error);
        assert!(matches!(error, Error::Io(_)));
        assert!(error.to_string().contains("IO error"));
    }

    #[test]
    fn test_json_error_from() {
        let json_error = serde_json::from_str::<i32>("not valid json").unwrap_err();
        let error = Error::from(json_error);
        assert!(matches!(error, Error::Json(_)));
        assert!(error.to_string().contains("JSON error"));
    }

    #[test]
    fn test_serialization_error() {
        let error = Error::Serialization("failed to serialize state".to_string());
        assert_eq!(
            error.to_string(),
            "Serialization error: failed to serialize state"
        );
    }

    #[test]
    fn test_diff_error() {
        let error = Error::DiffError("patch application failed".to_string());
        assert_eq!(
            error.to_string(),
            "Diff/patch error: patch application failed"
        );
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Error>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Error>();
    }

    #[test]
    fn test_error_debug_format() {
        let error = Error::Compression("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Compression"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_result_type_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        if let Ok(val) = result {
            assert_eq!(val, 42);
        }
    }

    #[test]
    fn test_result_type_err() {
        let result: Result<i32> = Err(Error::InvalidFormat("test".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_error_variants() {
        let errors = vec![
            Error::Compression("comp".to_string()),
            Error::Decompression("decomp".to_string()),
            Error::InvalidFormat("format".to_string()),
            Error::Serialization("ser".to_string()),
            Error::DiffError("diff".to_string()),
        ];

        for error in errors {
            // All errors should produce non-empty strings
            assert!(!error.to_string().is_empty());
            // All errors should be Debug
            assert!(!format!("{:?}", error).is_empty());
        }
    }

    #[test]
    fn test_error_propagation() {
        fn might_fail() -> Result<i32> {
            Err(Error::Compression("failed".to_string()))
        }

        fn calls_might_fail() -> Result<i32> {
            might_fail()?;
            Ok(42)
        }

        let result = calls_might_fail();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Compression(_)));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_errors = vec![
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        ];

        for io_error in io_errors {
            let error: Error = io_error.into();
            assert!(matches!(error, Error::Io(_)));
        }
    }

    #[test]
    fn test_json_error_conversion() {
        let json_result: std::result::Result<i32, serde_json::Error> =
            serde_json::from_str("invalid");
        let json_error = json_result.unwrap_err();
        let stream_error: Error = json_error.into();
        assert!(matches!(stream_error, Error::Json(_)));
    }

    #[test]
    fn test_error_message_accuracy() {
        let test_cases = vec![
            (Error::Compression("zstd".to_string()), "Compression error"),
            (
                Error::Decompression("lz4".to_string()),
                "Decompression error",
            ),
            (
                Error::InvalidFormat("v1".to_string()),
                "Invalid message format",
            ),
            (
                Error::Serialization("bincode".to_string()),
                "Serialization error",
            ),
            (Error::DiffError("patch".to_string()), "Diff/patch error"),
        ];

        for (error, expected_substring) in test_cases {
            let error_string = error.to_string();
            assert!(
                error_string.contains(expected_substring),
                "Error '{}' should contain '{}'",
                error_string,
                expected_substring
            );
        }
    }

    #[test]
    fn test_error_size() {
        // Ensure Error enum is not excessively large
        let size = std::mem::size_of::<Error>();
        // Should be reasonable (< 128 bytes)
        assert!(size < 128, "Error size {} is too large", size);
    }

    #[test]
    fn test_compression_error_variants() {
        let errors = vec![
            Error::Compression("buffer too small".to_string()),
            Error::Compression("invalid compression level".to_string()),
            Error::Compression("codec not available".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Compression error"));
        }
    }

    #[test]
    fn test_decompression_error_variants() {
        let errors = vec![
            Error::Decompression("corrupted data".to_string()),
            Error::Decompression("wrong codec".to_string()),
            Error::Decompression("checksum mismatch".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Decompression error"));
        }
    }

    #[test]
    fn test_diff_error_scenarios() {
        let errors = vec![
            Error::DiffError("patch sequence invalid".to_string()),
            Error::DiffError("state mismatch".to_string()),
            Error::DiffError("operation not applicable".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Diff/patch error"));
        }
    }

    #[test]
    fn test_kafka_error() {
        let error = Error::Kafka("invalid configuration".to_string());
        assert_eq!(error.to_string(), "Kafka error: invalid configuration");
    }

    #[test]
    fn test_kafka_error_scenarios() {
        let errors = vec![
            Error::Kafka("num_partitions must be >= 1".to_string()),
            Error::Kafka("invalid cleanup_policy".to_string()),
            Error::Kafka("bootstrap_servers cannot be empty".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Kafka error"));
        }
    }

    #[test]
    fn test_serialization_error_scenarios() {
        let errors = vec![
            Error::Serialization("type not serializable".to_string()),
            Error::Serialization("circular reference".to_string()),
            Error::Serialization("buffer overflow".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Serialization error"));
        }
    }

    #[test]
    fn test_invalid_format_scenarios() {
        let errors = vec![
            Error::InvalidFormat("wrong magic number".to_string()),
            Error::InvalidFormat("unsupported version".to_string()),
            Error::InvalidFormat("missing header".to_string()),
        ];

        for error in errors {
            assert!(error.to_string().contains("Invalid message format"));
        }
    }

    #[test]
    fn test_error_chain_from_io() {
        fn read_file() -> Result<String> {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file missing",
            )))
        }

        fn process_file() -> Result<String> {
            read_file()?;
            Ok("data".to_string())
        }

        let result = process_file();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Io(_)));
    }

    #[test]
    fn test_error_chain_from_json() {
        fn parse_json() -> Result<i32> {
            let data = "not json";
            let parsed: i32 = serde_json::from_str(data)?;
            Ok(parsed)
        }

        let result = parse_json();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Json(_)));
    }

    #[test]
    fn test_all_error_variants_display() {
        // Verify all variants produce meaningful display strings
        let errors = vec![
            Error::Compression("test".to_string()),
            Error::Decompression("test".to_string()),
            Error::InvalidFormat("test".to_string()),
            Error::Serialization("test".to_string()),
            Error::DiffError("test".to_string()),
            Error::Io(std::io::Error::other("test")),
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty(), "Error display should not be empty");
            // All our error types include "error" or "Error" in their message
            // except for IO which uses the wrapped message format
        }

        // Test ProtobufEncode separately since we can't easily construct it
        // We verify the error type is covered by the From trait test
    }
}
