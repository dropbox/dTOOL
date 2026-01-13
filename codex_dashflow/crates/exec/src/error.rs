//! Error types for exec mode

use thiserror::Error;

/// Errors that can occur during exec mode execution
#[derive(Error, Debug)]
pub enum ExecError {
    /// Failed to initialize the agent
    #[error("Agent initialization failed: {0}")]
    InitializationFailed(String),

    /// Agent execution failed
    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    /// Turn limit reached
    #[error("Agent reached turn limit ({0} turns)")]
    TurnLimitReached(u32),

    /// Agent was interrupted
    #[error("Agent execution was interrupted")]
    Interrupted,

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Core error
    #[error("Core error: {0}")]
    CoreError(String),
}

impl From<codex_dashflow_core::Error> for ExecError {
    fn from(err: codex_dashflow_core::Error) -> Self {
        ExecError::CoreError(err.to_string())
    }
}

/// Result type for exec operations
pub type Result<T> = std::result::Result<T, ExecError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ExecError::InitializationFailed("test error".to_string());
        assert_eq!(err.to_string(), "Agent initialization failed: test error");
    }

    #[test]
    fn test_turn_limit_error() {
        let err = ExecError::TurnLimitReached(10);
        assert_eq!(err.to_string(), "Agent reached turn limit (10 turns)");
    }

    #[test]
    fn test_interrupted_error() {
        let err = ExecError::Interrupted;
        assert_eq!(err.to_string(), "Agent execution was interrupted");
    }
}
