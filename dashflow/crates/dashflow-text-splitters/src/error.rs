//! Error types for text splitters

use thiserror::Error;

/// Errors that can occur when using text splitters
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Invalid configuration for text splitter
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Error during text splitting
    #[error("Text splitting failed: {0}")]
    SplittingError(String),

    /// Error from dashflow::core
    #[error("Core error: {0}")]
    CoreError(#[from] dashflow::core::Error),
}

/// Result type for text splitters
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_configuration_display() {
        let err = Error::InvalidConfiguration("chunk size too small".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid configuration: chunk size too small"
        );
    }

    #[test]
    fn test_splitting_error_display() {
        let err = Error::SplittingError("empty document".to_string());
        assert_eq!(err.to_string(), "Text splitting failed: empty document");
    }

    #[test]
    fn test_error_debug() {
        let err = Error::InvalidConfiguration("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidConfiguration"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_error_variants_exhaustive() {
        // Ensure all variants are testable
        let _ = Error::InvalidConfiguration("config".to_string());
        let _ = Error::SplittingError("split".to_string());
        // CoreError requires dashflow::core::Error which we can't easily construct here
    }
}
