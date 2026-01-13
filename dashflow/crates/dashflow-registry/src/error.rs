//! Error types for the package registry.

use thiserror::Error;

/// Registry result type.
pub type Result<T> = std::result::Result<T, RegistryError>;

/// Errors that can occur in registry operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// Package not found in registry.
    #[error("Package not found: {0}")]
    PackageNotFound(String),

    /// Version not found for package.
    #[error("Version {version} not found for package {package}")]
    VersionNotFound { package: String, version: String },

    /// Hash mismatch during verification.
    #[error("Content hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Invalid signature.
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    /// Signature verification failed.
    #[error("Signature verification failed for key {key_id}: {reason}")]
    SignatureVerificationFailed { key_id: String, reason: String },

    /// Unknown public key.
    #[error("Unknown public key: {0}")]
    UnknownPublicKey(String),

    /// Package manifest is invalid.
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Network/API error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Transfer denied by peer.
    #[error("Transfer denied: {0}")]
    TransferDenied(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded, retry after {retry_after_secs} seconds")]
    RateLimitExceeded { retry_after_secs: u64 },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid content hash format.
    #[error("Invalid content hash: {0}")]
    InvalidContentHash(String),

    /// Package already exists.
    #[error("Package already exists: {0}")]
    PackageAlreadyExists(String),

    /// Unauthorized operation.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Invalid version string.
    #[error("Invalid version: {0}")]
    InvalidVersion(String),

    /// Invalid input to an operation.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Operation timed out.
    #[error("Operation timed out")]
    Timeout,

    /// Access denied by peer or registry.
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// Invalid data received.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Feature not implemented.
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Multi-model review failed.
    #[error("Review failed: {0}")]
    ReviewFailed(String),

    /// Not found (generic).
    #[error("Not found: {0}")]
    NotFound(String),

    /// Network error (for client operations).
    #[error("Network error: {0}")]
    Network(String),

    /// Rate limited by server.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// API error response.
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    /// Invalid response from server.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// IO error (for client file operations).
    #[error("IO error: {0}")]
    Io(String),

    /// Search operation failed.
    #[error("Search error: {0}")]
    Search(String),

    /// Cache operation failed.
    #[error("Cache error: {0}")]
    Cache(String),

    /// M-225: Client-side signature verification failed
    #[error("Client signature verification failed: {0}")]
    ClientSignatureVerificationFailed(String),

    /// M-225: Trust level insufficient for installation
    #[error("Insufficient trust level: required {required:?}, actual {actual:?}")]
    InsufficientTrustLevel {
        required: crate::TrustLevel,
        actual: crate::TrustLevel,
    },
}

impl From<serde_json::Error> for RegistryError {
    fn from(err: serde_json::Error) -> Self {
        RegistryError::SerializationError(err.to_string())
    }
}

impl From<reqwest::Error> for RegistryError {
    fn from(err: reqwest::Error) -> Self {
        RegistryError::NetworkError(err.to_string())
    }
}

impl From<semver::Error> for RegistryError {
    fn from(err: semver::Error) -> Self {
        RegistryError::InvalidVersion(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RegistryError::PackageNotFound("my-package".to_string());
        assert_eq!(err.to_string(), "Package not found: my-package");

        let err = RegistryError::HashMismatch {
            expected: "sha256:abc".to_string(),
            actual: "sha256:def".to_string(),
        };
        assert!(err.to_string().contains("mismatch"));
    }
}
