//! Error types for WASM executor
//!
//! Provides comprehensive error handling with security-conscious error messages
//! that don't leak sensitive information.

use thiserror::Error;

/// Result type for WASM executor operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during WASM execution
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Authorization failed (user doesn't have permission)
    #[error("Authorization failed: {0}")]
    Authorization(String),

    /// Invalid WASM module
    #[error("Invalid WASM module: {0}")]
    InvalidWasm(String),

    /// WASM module too large
    #[error("WASM module exceeds size limit: {0} bytes")]
    WasmTooLarge(usize),

    /// WASM execution failed
    #[error("WASM execution failed: {0}")]
    ExecutionFailed(String),

    /// WASM execution timeout
    #[error("WASM execution timeout after {0} seconds")]
    Timeout(u64),

    /// Out of fuel (computational limit exceeded)
    #[error("Out of fuel: execution exceeded {0} operations")]
    OutOfFuel(u64),

    /// Out of memory
    #[error("Out of memory: execution exceeded {0} bytes")]
    OutOfMemory(usize),

    /// Audit logging failed
    #[error("Audit logging failed: {0}")]
    AuditFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// JWT error
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Wasmtime error
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),

    /// Internal error (should never happen in production)
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Sanitize error message for external exposure
    ///
    /// Removes potentially sensitive information from error messages
    /// before returning them to clients.
    #[must_use]
    pub fn sanitize(&self) -> String {
        match self {
            // These errors are safe to expose
            Error::Authentication(_) => "Authentication failed".to_string(),
            Error::Authorization(_) => "Access denied".to_string(),
            Error::InvalidWasm(_) => "Invalid WASM module".to_string(),
            Error::WasmTooLarge(size) => format!("WASM module too large: {size} bytes"),
            Error::Timeout(seconds) => format!("Execution timeout after {seconds} seconds"),
            Error::OutOfFuel(ops) => format!("Execution limit exceeded: {ops} operations"),
            Error::OutOfMemory(bytes) => format!("Memory limit exceeded: {bytes} bytes"),

            // These errors should be logged but not exposed
            Error::ExecutionFailed(_) => "Execution failed".to_string(),
            Error::AuditFailed(_) => "System error".to_string(),
            Error::Configuration(_) => "System error".to_string(),
            Error::Io(_) => "System error".to_string(),
            Error::Serialization(_) => "System error".to_string(),
            Error::Jwt(_) => "Authentication error".to_string(),
            Error::Wasmtime(_) => "Execution error".to_string(),
            Error::Internal(_) => "Internal system error".to_string(),
        }
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_sanitization() {
        let err = Error::ExecutionFailed("secret data: /etc/passwd".to_string());
        assert_eq!(err.sanitize(), "Execution failed");
        assert!(!err.sanitize().contains("secret"));
        assert!(!err.sanitize().contains("passwd"));
    }

    #[test]
    fn test_safe_error_exposure() {
        let err = Error::Timeout(30);
        assert_eq!(err.sanitize(), "Execution timeout after 30 seconds");
    }

    #[test]
    fn test_authentication_error_sanitization() {
        let err = Error::Authentication("Invalid JWT token: user=admin".to_string());
        assert_eq!(err.sanitize(), "Authentication failed");
        assert!(!err.sanitize().contains("JWT"));
        assert!(!err.sanitize().contains("admin"));
    }
}
