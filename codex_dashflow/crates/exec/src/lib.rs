//! Codex DashFlow Exec
//!
//! Non-interactive execution mode for automation and scripting.
//! This crate provides a way to run the agent without a TUI.
//!
//! # Features
//!
//! - Non-interactive prompt execution
//! - JSON output mode for programmatic consumption
//! - Human-readable output mode for terminal usage
//! - Streaming events during execution
//!
//! # Example
//!
//! ```no_run
//! use codex_dashflow_exec::{ExecConfig, OutputMode, run_exec};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ExecConfig::new("List the files in the current directory")
//!         .with_output_mode(OutputMode::Human);
//!
//!     let result = run_exec(config).await?;
//!     println!("Final response: {}", result.final_response);
//!     Ok(())
//! }
//! ```

mod config;
mod error;
mod output;
mod runner;

pub use config::{ExecConfig, OutputMode};
pub use error::{ExecError, Result};
pub use output::{
    ExecOutput, ExecStreamCallback, HumanOutputHandler, JsonOutputHandler, OutputHandler,
};
pub use runner::{run_exec, run_exec_with_handler, ExecResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_config_basic() {
        let config = ExecConfig::new("test prompt");
        assert_eq!(config.prompt, "test prompt");
        assert_eq!(config.output_mode, OutputMode::Human);
        assert!(!config.use_mock_llm);
    }

    #[test]
    fn test_exec_config_json_mode() {
        let config = ExecConfig::new("test").with_output_mode(OutputMode::Json);
        assert_eq!(config.output_mode, OutputMode::Json);
    }

    #[test]
    fn test_exec_config_with_mock() {
        let config = ExecConfig::new("test").with_mock_llm(true);
        assert!(config.use_mock_llm);
    }

    #[test]
    fn test_exec_config_with_max_turns() {
        let config = ExecConfig::new("test").with_max_turns(15);
        assert_eq!(config.max_turns, 15);
    }

    #[test]
    fn test_output_mode_display() {
        assert_eq!(format!("{}", OutputMode::Human), "human");
        assert_eq!(format!("{}", OutputMode::Json), "json");
    }

    #[test]
    fn test_output_mode_from_str() {
        use std::str::FromStr;
        assert_eq!(OutputMode::from_str("human").unwrap(), OutputMode::Human);
        assert_eq!(OutputMode::from_str("json").unwrap(), OutputMode::Json);
        assert_eq!(OutputMode::from_str("JSON").unwrap(), OutputMode::Json);
        assert!(OutputMode::from_str("invalid").is_err());
    }
}
