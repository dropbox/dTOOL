//! Agent configuration types and validation errors.
//!
//! This module provides error types for validating agent configurations,
//! ensuring agents are properly set up before execution.

use thiserror::Error as ThisError;

/// Error type for agent configuration validation.
///
/// These errors occur during agent construction when configuration
/// parameters don't meet the requirements of specific agent types.
#[derive(Debug, Clone, PartialEq, ThisError)]
#[non_exhaustive]
pub enum AgentConfigError {
    /// SelfAskWithSearchAgent requires exactly one tool.
    #[error("SelfAskWithSearchAgent requires exactly one tool, got {count}")]
    InvalidToolCount {
        /// The actual number of tools provided.
        count: usize,
    },
    /// SelfAskWithSearchAgent tool must be named "Intermediate Answer".
    #[error("SelfAskWithSearchAgent tool must be named 'Intermediate Answer', got '{name}'")]
    InvalidToolName {
        /// The invalid tool name that was provided.
        name: String,
    },
    /// Window size must be greater than 0.
    #[error("Window size must be greater than 0, got {size}")]
    InvalidWindowSize {
        /// The invalid window size that was provided.
        size: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_tool_count_display() {
        let err = AgentConfigError::InvalidToolCount { count: 3 };
        assert_eq!(
            err.to_string(),
            "SelfAskWithSearchAgent requires exactly one tool, got 3"
        );
    }

    #[test]
    fn test_invalid_tool_name_display() {
        let err = AgentConfigError::InvalidToolName {
            name: "wrong_name".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "SelfAskWithSearchAgent tool must be named 'Intermediate Answer', got 'wrong_name'"
        );
    }

    #[test]
    fn test_invalid_window_size_display() {
        let err = AgentConfigError::InvalidWindowSize { size: 0 };
        assert_eq!(err.to_string(), "Window size must be greater than 0, got 0");
    }

    #[test]
    fn test_error_equality() {
        let err1 = AgentConfigError::InvalidToolCount { count: 2 };
        let err2 = AgentConfigError::InvalidToolCount { count: 2 };
        let err3 = AgentConfigError::InvalidToolCount { count: 3 };

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_error_clone() {
        let err = AgentConfigError::InvalidToolName {
            name: "test".to_string(),
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_error_debug() {
        let err = AgentConfigError::InvalidToolCount { count: 5 };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidToolCount"));
        assert!(debug_str.contains("5"));
    }
}
