//! Error types for `LangServe`

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for `LangServe` operations
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LangServeError {
    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Runnable execution failed
    #[error("Runnable execution failed: {0}")]
    ExecutionError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Schema generation error
    #[error("Schema generation error: {0}")]
    SchemaError(String),

    /// Streaming error
    #[error("Streaming error: {0}")]
    StreamingError(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Not found error
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Error response structure for API
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: u16,
}

impl IntoResponse for LangServeError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            LangServeError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            LangServeError::ExecutionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            LangServeError::SerializationError(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            LangServeError::SchemaError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            LangServeError::StreamingError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            LangServeError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            LangServeError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = Json(ErrorResponse {
            error: error_message,
            status: status.as_u16(),
        });

        (status, body).into_response()
    }
}

/// Result type alias for `LangServe` operations
pub type Result<T> = std::result::Result<T, LangServeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_request_display() {
        let err = LangServeError::InvalidRequest("missing field".to_string());
        assert_eq!(err.to_string(), "Invalid request: missing field");
    }

    #[test]
    fn test_execution_error_display() {
        let err = LangServeError::ExecutionError("timeout".to_string());
        assert_eq!(err.to_string(), "Runnable execution failed: timeout");
    }

    #[test]
    fn test_serialization_error_from() {
        let json_err: serde_json::Error = serde_json::from_str::<String>("invalid").unwrap_err();
        let err = LangServeError::from(json_err);
        assert!(matches!(err, LangServeError::SerializationError(_)));
        assert!(err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_schema_error_display() {
        let err = LangServeError::SchemaError("invalid schema".to_string());
        assert_eq!(err.to_string(), "Schema generation error: invalid schema");
    }

    #[test]
    fn test_streaming_error_display() {
        let err = LangServeError::StreamingError("connection lost".to_string());
        assert_eq!(err.to_string(), "Streaming error: connection lost");
    }

    #[test]
    fn test_internal_error_display() {
        let err = LangServeError::InternalError("database failure".to_string());
        assert_eq!(err.to_string(), "Internal error: database failure");
    }

    #[test]
    fn test_not_found_display() {
        let err = LangServeError::NotFound("resource".to_string());
        assert_eq!(err.to_string(), "Not found: resource");
    }

    #[test]
    fn test_error_debug() {
        let err = LangServeError::InvalidRequest("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidRequest"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_error_response_serialize() {
        let resp = ErrorResponse {
            error: "test error".to_string(),
            status: 400,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("test error"));
        assert!(json.contains("400"));
    }

    #[test]
    fn test_error_response_deserialize() {
        let json = r#"{"error": "test error", "status": 400}"#;
        let resp: ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.error, "test error");
        assert_eq!(resp.status, 400);
    }

    // ==================== Additional Error Display Tests ====================

    #[test]
    fn test_error_display_with_empty_message() {
        let err = LangServeError::InvalidRequest(String::new());
        assert_eq!(err.to_string(), "Invalid request: ");
    }

    #[test]
    fn test_error_display_with_unicode() {
        let err = LangServeError::InvalidRequest("日本語エラー".to_string());
        assert!(err.to_string().contains("日本語エラー"));
    }

    #[test]
    fn test_error_display_with_long_message() {
        let long_msg = "x".repeat(1000);
        let err = LangServeError::ExecutionError(long_msg.clone());
        assert!(err.to_string().contains(&long_msg));
    }

    #[test]
    fn test_error_display_with_special_chars() {
        let err = LangServeError::InvalidRequest("error with\nnewline\tand\ttabs".to_string());
        assert!(err.to_string().contains("newline"));
    }

    // ==================== Error Response Tests ====================

    #[test]
    fn test_error_response_debug() {
        let resp = ErrorResponse {
            error: "debug test".to_string(),
            status: 500,
        };
        let debug = format!("{:?}", resp);
        assert!(debug.contains("ErrorResponse"));
        assert!(debug.contains("debug test"));
        assert!(debug.contains("500"));
    }

    #[test]
    fn test_error_response_roundtrip() {
        let original = ErrorResponse {
            error: "roundtrip test".to_string(),
            status: 422,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(original.error, deserialized.error);
        assert_eq!(original.status, deserialized.status);
    }

    #[test]
    fn test_error_response_empty_error() {
        let resp = ErrorResponse {
            error: String::new(),
            status: 400,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""error":"""#));
    }

    #[test]
    fn test_error_response_various_status_codes() {
        for status in [200, 400, 401, 403, 404, 500, 502, 503] {
            let resp = ErrorResponse {
                error: "test".to_string(),
                status,
            };
            let json = serde_json::to_string(&resp).unwrap();
            let deser: ErrorResponse = serde_json::from_str(&json).unwrap();
            assert_eq!(deser.status, status);
        }
    }

    // ==================== Error Type-Specific Tests ====================

    #[test]
    fn test_schema_error_variants() {
        let messages = vec![
            "missing required field",
            "type mismatch",
            "invalid format",
            "",
        ];
        for msg in messages {
            let err = LangServeError::SchemaError(msg.to_string());
            assert!(err.to_string().contains("Schema generation error"));
        }
    }

    #[test]
    fn test_streaming_error_variants() {
        let messages = vec![
            "connection reset",
            "timeout",
            "EOF",
            "parse error",
        ];
        for msg in messages {
            let err = LangServeError::StreamingError(msg.to_string());
            assert!(err.to_string().contains("Streaming error"));
        }
    }

    #[test]
    fn test_internal_error_variants() {
        let messages = vec![
            "null pointer",
            "stack overflow",
            "out of memory",
            "assertion failed",
        ];
        for msg in messages {
            let err = LangServeError::InternalError(msg.to_string());
            assert!(err.to_string().contains("Internal error"));
        }
    }

    #[test]
    fn test_not_found_error_variants() {
        let messages = vec![
            "/api/v1/users",
            "resource-123",
            "endpoint",
        ];
        for msg in messages {
            let err = LangServeError::NotFound(msg.to_string());
            assert!(err.to_string().contains("Not found"));
            assert!(err.to_string().contains(msg));
        }
    }

    // ==================== Error Pattern Matching Tests ====================

    #[test]
    fn test_error_matches_invalid_request() {
        let err = LangServeError::InvalidRequest("test".to_string());
        assert!(matches!(err, LangServeError::InvalidRequest(_)));
    }

    #[test]
    fn test_error_matches_execution_error() {
        let err = LangServeError::ExecutionError("test".to_string());
        assert!(matches!(err, LangServeError::ExecutionError(_)));
    }

    #[test]
    fn test_error_matches_schema_error() {
        let err = LangServeError::SchemaError("test".to_string());
        assert!(matches!(err, LangServeError::SchemaError(_)));
    }

    #[test]
    fn test_error_matches_streaming_error() {
        let err = LangServeError::StreamingError("test".to_string());
        assert!(matches!(err, LangServeError::StreamingError(_)));
    }

    #[test]
    fn test_error_matches_internal_error() {
        let err = LangServeError::InternalError("test".to_string());
        assert!(matches!(err, LangServeError::InternalError(_)));
    }

    #[test]
    fn test_error_matches_not_found() {
        let err = LangServeError::NotFound("test".to_string());
        assert!(matches!(err, LangServeError::NotFound(_)));
    }

    // ==================== Serialization Error Tests ====================

    #[test]
    fn test_serialization_error_invalid_json() {
        let json_err: serde_json::Error = serde_json::from_str::<String>("{invalid}").unwrap_err();
        let err = LangServeError::from(json_err);
        assert!(matches!(err, LangServeError::SerializationError(_)));
    }

    #[test]
    fn test_serialization_error_type_mismatch() {
        let json_err: serde_json::Error = serde_json::from_str::<u32>(r#""string""#).unwrap_err();
        let err = LangServeError::from(json_err);
        assert!(matches!(err, LangServeError::SerializationError(_)));
    }

    #[test]
    fn test_serialization_error_eof() {
        let json_err: serde_json::Error = serde_json::from_str::<String>("{").unwrap_err();
        let err = LangServeError::from(json_err);
        assert!(matches!(err, LangServeError::SerializationError(_)));
    }
}
