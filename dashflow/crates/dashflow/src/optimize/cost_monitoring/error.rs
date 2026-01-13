// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Error types for cost monitoring

// Allow internal use of deprecated types within this deprecated module
#![allow(deprecated)]

use thiserror::Error;

/// Errors that can occur during cost monitoring
#[deprecated(
    since = "1.11.3",
    note = "Use errors from `dashflow_observability::cost` instead"
)]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CostMonitorError {
    /// Model not found in pricing database
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Budget exceeded.
    #[error("Budget exceeded: spent ${spent:.2}, limit ${limit:.2}")]
    BudgetExceeded {
        /// Amount spent so far.
        spent: f64,
        /// Budget limit.
        limit: f64,
    },

    /// Invalid budget configuration
    #[error("Invalid budget configuration: {0}")]
    InvalidConfig(String),

    /// Lock poisoned (thread panic)
    #[error("Internal lock poisoned: {0}")]
    LockPoisoned(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Convenience type alias for results from cost monitoring operations.
///
/// Wraps [`std::result::Result`] with [`CostMonitorError`] as the error type.
pub type Result<T> = std::result::Result<T, CostMonitorError>;

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_model_not_found_display() {
        let err = CostMonitorError::ModelNotFound("gpt-5".to_string());
        assert_eq!(err.to_string(), "Model not found: gpt-5");
    }

    #[test]
    fn test_budget_exceeded_display() {
        let err = CostMonitorError::BudgetExceeded {
            spent: 10.50,
            limit: 5.00,
        };
        assert_eq!(err.to_string(), "Budget exceeded: spent $10.50, limit $5.00");
    }

    #[test]
    fn test_invalid_config_display() {
        let err = CostMonitorError::InvalidConfig("missing api key".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid budget configuration: missing api key"
        );
    }

    #[test]
    fn test_lock_poisoned_display() {
        let err = CostMonitorError::LockPoisoned("budget tracker".to_string());
        assert_eq!(err.to_string(), "Internal lock poisoned: budget tracker");
    }

    #[test]
    fn test_serialization_error_from() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let err: CostMonitorError = json_err.into();
        assert!(err.to_string().contains("Serialization error:"));
    }

    #[test]
    fn test_error_debug() {
        let err = CostMonitorError::BudgetExceeded {
            spent: 100.0,
            limit: 50.0,
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("BudgetExceeded"));
        assert!(debug_str.contains("100"));
        assert!(debug_str.contains("50"));
    }
}
