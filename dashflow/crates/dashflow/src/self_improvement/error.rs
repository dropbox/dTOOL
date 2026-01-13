// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Unified error type for the self-improvement system.
//!
//! This module provides a single error type that wraps all possible errors
//! in the self-improvement subsystem, enabling consistent error handling
//! and propagation throughout the codebase.

use std::path::PathBuf;
use thiserror::Error;

/// Unified error type for self-improvement operations.
///
/// This enum consolidates all error types used across the self-improvement
/// module into a single, well-typed error that can be easily matched and
/// converted from various sources.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SelfImprovementError {
    /// Storage I/O operation failed.
    #[error("Storage error: {0}")]
    Storage(#[from] std::io::Error),

    /// JSON serialization/deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Alert handling failed.
    #[error("Alert error: {message}")]
    Alert {
        /// Error message
        message: String,
        /// Handler that failed (if applicable)
        handler: Option<String>,
    },

    /// Prometheus/metrics query failed.
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Plan not found.
    #[error("Plan not found: {0}")]
    PlanNotFound(uuid::Uuid),

    /// Hypothesis not found.
    #[error("Hypothesis not found: {0}")]
    HypothesisNotFound(uuid::Uuid),

    /// Report not found.
    #[error("Report not found: {0}")]
    ReportNotFound(uuid::Uuid),

    /// File not found.
    #[error("File not found: {}", .0.display())]
    FileNotFound(PathBuf),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Analysis failed.
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    /// Validation failed.
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Network/HTTP error.
    #[error("Network error: {0}")]
    Network(String),

    /// Timeout error.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Consensus error.
    #[error("Consensus error: {0}")]
    Consensus(String),

    /// Generic error for other cases.
    #[error("{0}")]
    Other(String),
}

impl SelfImprovementError {
    /// Create an alert error.
    #[must_use]
    pub fn alert(message: impl Into<String>) -> Self {
        Self::Alert {
            message: message.into(),
            handler: None,
        }
    }

    /// Create an alert error with handler context.
    #[must_use]
    pub fn alert_from_handler(handler: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Alert {
            message: message.into(),
            handler: Some(handler.into()),
        }
    }

    /// Create a metrics error.
    #[must_use]
    pub fn metrics(message: impl Into<String>) -> Self {
        Self::Metrics(message.into())
    }

    /// Create an analysis error.
    #[must_use]
    pub fn analysis(message: impl Into<String>) -> Self {
        Self::AnalysisFailed(message.into())
    }

    /// Create a validation error.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::ValidationFailed(message.into())
    }

    /// Create a network error.
    #[must_use]
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network(message.into())
    }

    /// Create a timeout error.
    #[must_use]
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    /// Create a consensus error.
    #[must_use]
    pub fn consensus(message: impl Into<String>) -> Self {
        Self::Consensus(message.into())
    }

    /// Create an error for invalid configuration.
    #[must_use]
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig(message.into())
    }

    /// Create a generic "other" error.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Check if this is a storage error.
    #[must_use]
    pub fn is_storage(&self) -> bool {
        matches!(self, Self::Storage(_))
    }

    /// Check if this is a serialization error.
    #[must_use]
    pub fn is_serialization(&self) -> bool {
        matches!(self, Self::Serialization(_))
    }

    /// Check if this is a not-found error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::PlanNotFound(_)
                | Self::HypothesisNotFound(_)
                | Self::ReportNotFound(_)
                | Self::FileNotFound(_)
        )
    }

    /// Check if this is a timeout error.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_))
    }

    /// Check if this is a network error.
    #[must_use]
    pub fn is_network(&self) -> bool {
        matches!(self, Self::Network(_))
    }
}

/// Convenience type alias for Results with SelfImprovementError.
pub type Result<T> = std::result::Result<T, SelfImprovementError>;

// Conversion from String for backward compatibility with Result<T, String>
impl From<String> for SelfImprovementError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for SelfImprovementError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: SelfImprovementError = io_err.into();
        assert!(err.is_storage());
        assert!(err.to_string().contains("Storage error"));
    }

    #[test]
    fn test_alert_error() {
        let err = SelfImprovementError::alert("test alert");
        assert!(err.to_string().contains("Alert error: test alert"));
    }

    #[test]
    fn test_alert_with_handler() {
        let err = SelfImprovementError::alert_from_handler("WebhookHandler", "connection failed");
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_metrics_error() {
        let err = SelfImprovementError::metrics("prometheus unreachable");
        assert!(err.to_string().contains("Metrics error"));
    }

    #[test]
    fn test_plan_not_found() {
        let id = uuid::Uuid::new_v4();
        let err = SelfImprovementError::PlanNotFound(id);
        assert!(err.is_not_found());
        assert!(err.to_string().contains("Plan not found"));
    }

    #[test]
    fn test_from_string() {
        let err: SelfImprovementError = "generic error".into();
        assert!(matches!(err, SelfImprovementError::Other(_)));
        assert!(err.to_string().contains("generic error"));
    }

    #[test]
    fn test_serialization_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: SelfImprovementError = json_err.into();
        assert!(err.is_serialization());
    }

    #[test]
    fn test_error_predicates() {
        assert!(SelfImprovementError::timeout("test").is_timeout());
        assert!(SelfImprovementError::network("test").is_network());
        assert!(!SelfImprovementError::analysis("test").is_timeout());
    }
}
