//! Runner for exec mode execution

use std::io;
use std::sync::Arc;

use codex_dashflow_core::state::{CompletionStatus, Message};
use codex_dashflow_core::{run_agent, AgentState, RunnerConfig};

use crate::config::{ExecConfig, OutputMode};
use crate::error::{ExecError, Result};
use crate::output::{
    ExecOutput, ExecStreamCallback, HumanOutputHandler, JsonOutputHandler, OutputHandler,
};

/// Result of exec mode execution
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Session ID
    pub session_id: String,
    /// Final response from the agent
    pub final_response: String,
    /// Number of turns executed
    pub turns: u32,
    /// Completion status
    pub status: CompletionStatus,
}

impl ExecResult {
    /// Check if execution completed successfully
    pub fn is_success(&self) -> bool {
        matches!(self.status, CompletionStatus::Complete)
    }
}

/// Run the agent in non-interactive exec mode
///
/// # Arguments
/// * `config` - Exec mode configuration
///
/// # Returns
/// The result of the execution
pub async fn run_exec(config: ExecConfig) -> Result<ExecResult> {
    // Create output handler based on mode
    let handler: Arc<dyn OutputHandler> = match config.output_mode {
        OutputMode::Json => Arc::new(JsonOutputHandler::new(io::stdout())),
        OutputMode::Human => Arc::new(HumanOutputHandler::new(io::stderr(), config.verbose)),
    };

    run_exec_with_handler(config, handler).await
}

/// Run exec mode with a custom output handler
pub async fn run_exec_with_handler(
    config: ExecConfig,
    handler: Arc<dyn OutputHandler>,
) -> Result<ExecResult> {
    // Create streaming callback
    let stream_callback = Arc::new(ExecStreamCallback::new(handler.clone()));

    // Build agent state from config
    let state = build_agent_state(&config);

    // Build runner config
    let runner_config = build_runner_config(&config, stream_callback.clone());

    // Run the agent
    let result = run_agent(state, &runner_config)
        .await
        .map_err(|e| ExecError::ExecutionFailed(e.to_string()))?;

    // Extract final response
    let final_response = result.state.last_response.clone().unwrap_or_default();

    // Build output for handler
    let output = ExecOutput {
        session_id: result.thread_id.clone(),
        final_response: final_response.clone(),
        turns: result.turns,
        status: format!("{:?}", result.state.status),
        tool_calls: stream_callback.tool_calls(),
    };

    // Print final output
    handler.print_final(&output);

    Ok(ExecResult {
        session_id: result.thread_id,
        final_response,
        turns: result.turns,
        status: result.state.status,
    })
}

/// Build agent state from exec configuration
fn build_agent_state(config: &ExecConfig) -> AgentState {
    let mut state = AgentState::new();
    state.session_id = config
        .session_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    if let Some(ref model) = config.model {
        state.llm_config.model = model.clone();
    }

    if config.max_turns > 0 {
        state.max_turns = config.max_turns;
    }

    if config.use_mock_llm {
        state = state.with_mock_llm();
    }

    // Add the user message
    state.messages.push(Message::user(&config.prompt));

    state
}

/// Build runner configuration from exec configuration
fn build_runner_config(
    config: &ExecConfig,
    stream_callback: Arc<ExecStreamCallback>,
) -> RunnerConfig {
    let mut runner_config = if config.enable_checkpointing {
        if let Some(ref path) = config.checkpoint_path {
            RunnerConfig::with_file_checkpointing(path)
                .with_stream_callback(stream_callback)
                .with_collect_training(config.collect_training)
                .with_load_optimized_prompts(config.load_optimized_prompts)
        } else {
            RunnerConfig::with_memory_checkpointing()
                .with_stream_callback(stream_callback)
                .with_collect_training(config.collect_training)
                .with_load_optimized_prompts(config.load_optimized_prompts)
        }
    } else {
        RunnerConfig::default()
            .with_stream_callback(stream_callback)
            .with_collect_training(config.collect_training)
            .with_load_optimized_prompts(config.load_optimized_prompts)
    };

    if let Some(ref prompt) = config.system_prompt {
        runner_config = runner_config.with_system_prompt(prompt);
    }

    runner_config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_exec_with_mock() {
        let config = ExecConfig::new("Hello, agent!")
            .with_mock_llm(true)
            .with_output_mode(OutputMode::Human);

        let result = run_exec(config).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(!result.session_id.is_empty());
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_run_exec_json_mode() {
        let config = ExecConfig::new("Hello")
            .with_mock_llm(true)
            .with_output_mode(OutputMode::Json);

        let result = run_exec(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_exec_with_session_id() {
        let config = ExecConfig::new("Hello")
            .with_mock_llm(true)
            .with_session_id("custom-session-123");

        let result = run_exec(config).await.unwrap();
        assert_eq!(result.session_id, "custom-session-123");
    }

    #[tokio::test]
    async fn test_run_exec_with_max_turns() {
        let config = ExecConfig::new("Hello")
            .with_mock_llm(true)
            .with_max_turns(5);

        let result = run_exec(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_exec_result_is_success() {
        let result = ExecResult {
            session_id: "test".to_string(),
            final_response: "response".to_string(),
            turns: 1,
            status: CompletionStatus::Complete,
        };
        assert!(result.is_success());

        let result = ExecResult {
            session_id: "test".to_string(),
            final_response: "response".to_string(),
            turns: 10,
            status: CompletionStatus::TurnLimitReached,
        };
        assert!(!result.is_success());
    }

    // Test with custom handler
    struct TestHandler {
        events: std::sync::Mutex<Vec<String>>,
    }

    impl TestHandler {
        fn new() -> Self {
            Self {
                events: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl OutputHandler for TestHandler {
        fn handle_event(&self, event: &codex_dashflow_core::streaming::AgentEvent) {
            if let Ok(mut events) = self.events.lock() {
                events.push(event.event_type().to_string());
            }
        }

        fn print_final(&self, _output: &ExecOutput) {
            if let Ok(mut events) = self.events.lock() {
                events.push("final".to_string());
            }
        }
    }

    #[tokio::test]
    async fn test_run_exec_with_custom_handler() {
        let handler = Arc::new(TestHandler::new());
        let config = ExecConfig::new("Hello").with_mock_llm(true);

        let result = run_exec_with_handler(config, handler.clone()).await;
        assert!(result.is_ok());

        // Check that events were captured
        let events = handler.events.lock().unwrap();
        assert!(events.contains(&"final".to_string()));
    }
}
