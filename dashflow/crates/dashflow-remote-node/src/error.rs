//! Error types for remote node execution

use thiserror::Error;

/// Result type for remote node operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during remote node execution
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// gRPC transport error
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// gRPC status error
    #[error("gRPC status error: {0}")]
    Status(Box<tonic::Status>),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Node not found on remote server
    #[error("Node '{0}' not found on remote server")]
    NodeNotFound(String),

    /// Execution error from remote node
    #[error("Remote node execution failed: {0}")]
    RemoteExecution(String),

    /// Timeout error
    #[error("Remote node execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Health check failed
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    /// Retry exhausted
    #[error("Retry exhausted after {0} attempts")]
    RetryExhausted(usize),

    /// Invalid response from server
    #[error("Invalid response from server: {0}")]
    InvalidResponse(String),

    /// `DashFlow` error
    #[error("DashFlow error: {0}")]
    DashFlow(#[from] dashflow::error::Error),
}

impl Error {
    /// Check if this error is retryable
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            // Network and temporary failures are retryable
            Error::Transport(_) => true,
            Error::Status(status) => {
                matches!(
                    status.code(),
                    tonic::Code::Unavailable
                        | tonic::Code::DeadlineExceeded
                        | tonic::Code::ResourceExhausted
                        | tonic::Code::Aborted
                )
            }
            Error::Timeout(_) => true,
            Error::RemoteExecution(_) => false, // Remote execution errors are not retryable by default
            Error::RetryExhausted(_) => false,
            _ => false,
        }
    }
}

impl From<tonic::Status> for Error {
    fn from(status: tonic::Status) -> Self {
        Error::Status(Box::new(status))
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_serialization_error_display() {
        let err = Error::Serialization("invalid format".to_string());
        assert_eq!(err.to_string(), "Serialization error: invalid format");
    }

    #[test]
    fn test_deserialization_error_display() {
        let err = Error::Deserialization("missing field".to_string());
        assert_eq!(err.to_string(), "Deserialization error: missing field");
    }

    #[test]
    fn test_node_not_found_display() {
        let err = Error::NodeNotFound("my_node".to_string());
        assert_eq!(
            err.to_string(),
            "Node 'my_node' not found on remote server"
        );
    }

    #[test]
    fn test_remote_execution_display() {
        let err = Error::RemoteExecution("timeout reached".to_string());
        assert_eq!(
            err.to_string(),
            "Remote node execution failed: timeout reached"
        );
    }

    #[test]
    fn test_timeout_display() {
        let err = Error::Timeout(Duration::from_secs(30));
        assert_eq!(err.to_string(), "Remote node execution timed out after 30s");
    }

    #[test]
    fn test_configuration_error_display() {
        let err = Error::Configuration("missing endpoint".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing endpoint");
    }

    #[test]
    fn test_health_check_failed_display() {
        let err = Error::HealthCheckFailed("server unreachable".to_string());
        assert_eq!(err.to_string(), "Health check failed: server unreachable");
    }

    #[test]
    fn test_retry_exhausted_display() {
        let err = Error::RetryExhausted(5);
        assert_eq!(err.to_string(), "Retry exhausted after 5 attempts");
    }

    #[test]
    fn test_invalid_response_display() {
        let err = Error::InvalidResponse("malformed JSON".to_string());
        assert_eq!(err.to_string(), "Invalid response from server: malformed JSON");
    }

    #[test]
    fn test_is_retryable_timeout() {
        let err = Error::Timeout(Duration::from_secs(10));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_remote_execution() {
        let err = Error::RemoteExecution("failed".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_is_retryable_retry_exhausted() {
        let err = Error::RetryExhausted(3);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_is_retryable_configuration() {
        let err = Error::Configuration("bad config".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_status_retryable_unavailable() {
        let status = tonic::Status::unavailable("server down");
        let err = Error::from(status);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_status_not_retryable_invalid_argument() {
        let status = tonic::Status::invalid_argument("bad input");
        let err = Error::from(status);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::NodeNotFound("test_node".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("NodeNotFound"));
        assert!(debug.contains("test_node"));
    }
}
