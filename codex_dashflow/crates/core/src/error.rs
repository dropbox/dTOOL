//! Error types for Codex DashFlow Core

use thiserror::Error;

/// Errors that can occur during function/tool call processing
#[derive(Debug, Error, PartialEq, Clone)]
pub enum FunctionCallError {
    /// Error that should be reported back to the model for retry/correction
    #[error("{0}")]
    RespondToModel(String),

    /// The function call was denied (e.g., by approval policy)
    #[error("{0}")]
    Denied(String),

    /// A shell call is missing its required call_id
    #[error("LocalShellCall without call_id or id")]
    MissingLocalShellCallId,

    /// Fatal error that should abort the agent turn
    #[error("Fatal error: {0}")]
    Fatal(String),
}

/// Core errors for the Codex DashFlow agent
#[derive(Error, Debug)]
pub enum Error {
    #[error("Graph compilation error: {0}")]
    GraphCompilation(String),

    #[error("Graph execution error: {0}")]
    GraphExecution(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("DashFlow error: {0}")]
    DashFlow(#[from] dashflow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Agent shutdown")]
    AgentShutdown,
}

/// Result type alias for Codex DashFlow Core
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_graph_compilation() {
        let err = Error::GraphCompilation("invalid node".to_string());
        assert_eq!(format!("{}", err), "Graph compilation error: invalid node");
    }

    #[test]
    fn test_error_display_graph_execution() {
        let err = Error::GraphExecution("node failed".to_string());
        assert_eq!(format!("{}", err), "Graph execution error: node failed");
    }

    #[test]
    fn test_error_display_tool_execution() {
        let err = Error::ToolExecution("command failed".to_string());
        assert_eq!(format!("{}", err), "Tool execution error: command failed");
    }

    #[test]
    fn test_error_display_unknown_tool() {
        let err = Error::UnknownTool("foo_tool".to_string());
        assert_eq!(format!("{}", err), "Unknown tool: foo_tool");
    }

    #[test]
    fn test_error_display_llm_api() {
        let err = Error::LlmApi("rate limited".to_string());
        assert_eq!(format!("{}", err), "LLM API error: rate limited");
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Serialization(_)));
        assert!(format!("{}", err).starts_with("Serialization error:"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(format!("{}", err).starts_with("IO error:"));
    }

    #[test]
    fn test_error_debug() {
        let err = Error::UnknownTool("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("UnknownTool"));
    }

    #[test]
    fn test_function_call_error_respond_to_model() {
        let err = FunctionCallError::RespondToModel("invalid input".to_string());
        assert_eq!(format!("{}", err), "invalid input");
    }

    #[test]
    fn test_function_call_error_denied() {
        let err = FunctionCallError::Denied("command not approved".to_string());
        assert_eq!(format!("{}", err), "command not approved");
    }

    #[test]
    fn test_function_call_error_missing_id() {
        let err = FunctionCallError::MissingLocalShellCallId;
        assert_eq!(format!("{}", err), "LocalShellCall without call_id or id");
    }

    #[test]
    fn test_function_call_error_fatal() {
        let err = FunctionCallError::Fatal("system crash".to_string());
        assert_eq!(format!("{}", err), "Fatal error: system crash");
    }

    #[test]
    fn test_function_call_error_clone() {
        let err = FunctionCallError::Denied("test".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_function_call_error_eq() {
        let err1 = FunctionCallError::RespondToModel("test".to_string());
        let err2 = FunctionCallError::RespondToModel("test".to_string());
        let err3 = FunctionCallError::RespondToModel("other".to_string());
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
