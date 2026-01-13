//! Error types for `LangSmith` client

use thiserror::Error;

/// Result type alias for `LangSmith` operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when using the `LangSmith` client
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// HTTP request error
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// API error with status code and message
    #[error("API error (status {status}): {message}")]
    Api {
        /// HTTP status code
        status: u16,
        /// Error message from API
        message: String,
    },

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create an API error from status and message
    pub fn api_error(status: u16, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Create an authentication error
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_authentication_error_display() {
        let err = Error::Authentication("invalid token".to_string());
        assert_eq!(err.to_string(), "Authentication failed: invalid token");
    }

    #[test]
    fn test_api_error_display() {
        let err = Error::Api {
            status: 404,
            message: "not found".to_string(),
        };
        assert_eq!(err.to_string(), "API error (status 404): not found");
    }

    #[test]
    fn test_config_error_display() {
        let err = Error::Config("missing api key".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing api key");
    }

    #[test]
    fn test_invalid_input_display() {
        let err = Error::InvalidInput("empty name".to_string());
        assert_eq!(err.to_string(), "Invalid input: empty name");
    }

    #[test]
    fn test_other_error_display() {
        let err = Error::Other("unknown error".to_string());
        assert_eq!(err.to_string(), "unknown error");
    }

    #[test]
    fn test_json_error_from() {
        let json_err: serde_json::Error = serde_json::from_str::<String>("invalid").unwrap_err();
        let err = Error::from(json_err);
        assert!(matches!(err, Error::Json(_)));
        assert!(err.to_string().contains("JSON error"));
    }

    #[test]
    fn test_api_error_constructor() {
        let err = Error::api_error(500, "internal server error");
        match err {
            Error::Api { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "internal server error");
            }
            _ => panic!("Expected Api variant"),
        }
    }

    #[test]
    fn test_config_constructor() {
        let err = Error::config("missing endpoint");
        assert!(matches!(err, Error::Config(_)));
        assert_eq!(err.to_string(), "Configuration error: missing endpoint");
    }

    #[test]
    fn test_auth_constructor() {
        let err = Error::auth("token expired");
        assert!(matches!(err, Error::Authentication(_)));
        assert_eq!(err.to_string(), "Authentication failed: token expired");
    }

    #[test]
    fn test_invalid_input_constructor() {
        let err = Error::invalid_input("empty project name");
        assert!(matches!(err, Error::InvalidInput(_)));
        assert_eq!(err.to_string(), "Invalid input: empty project name");
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Api {
            status: 400,
            message: "bad request".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Api"));
        assert!(debug.contains("400"));
        assert!(debug.contains("bad request"));
    }
}
