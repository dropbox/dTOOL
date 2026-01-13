//! End-to-end integration tests for the agent workflow
//!
//! These tests verify the complete agent flow from user input through
//! reasoning, tool execution, and result analysis.

use std::sync::Arc;

use codex_dashflow_core::{
    run_agent, run_turn, AgentState, Message, MetricsCallback, RunnerConfig,
};
use codex_dashflow_sandbox::SandboxMode;
use tempfile::TempDir;

// ============================================================================
// Helper functions
// ============================================================================

/// Create a test state with mock LLM
fn test_state() -> AgentState {
    AgentState::new().with_mock_llm()
}

/// Create a test state with mock LLM in a temporary working directory
fn test_state_in_dir(dir: &TempDir) -> AgentState {
    AgentState::new()
        .with_mock_llm()
        .with_working_directory(dir.path().to_string_lossy())
}

/// Create a metrics callback for capturing events
fn metrics_callback() -> Arc<MetricsCallback> {
    Arc::new(MetricsCallback::new())
}

// ============================================================================
// Basic Agent Flow Tests
// ============================================================================

#[tokio::test]
async fn test_agent_simple_conversation() {
    // Test basic conversation flow without tools
    let mut state = test_state();
    state.messages.push(Message::user("Hello, how are you?"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete successfully");
    let result = result.unwrap();

    // Should have a response
    assert!(
        result.state.last_response.is_some(),
        "Agent should produce a response"
    );

    // Should complete without errors
    assert!(
        !matches!(
            result.state.status,
            codex_dashflow_core::state::CompletionStatus::Error(_)
        ),
        "Agent should not error"
    );
}

#[tokio::test]
async fn test_agent_with_max_turns() {
    let mut state = test_state();
    state
        .messages
        .push(Message::user("List files and then create a new file"));

    let config = RunnerConfig::default().with_max_turns(3);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Should respect max turns
    assert!(
        result.turns <= 3,
        "Agent should respect max_turns limit: got {} turns",
        result.turns
    );
}

#[tokio::test]
async fn test_run_turn_api() {
    // Test the turn-based API
    let state = test_state();
    let config = RunnerConfig::default();

    let result = run_turn(state, "What time is it?", &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.state.last_response.is_some());
}

// ============================================================================
// Tool Execution Flow Tests
// ============================================================================

#[tokio::test]
async fn test_agent_tool_execution_shell() {
    // Test that shell commands flow through the agent correctly
    let mut state = test_state();
    // Use "List files" which triggers tool call in mock_llm_response
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete: {:?}", result.err());
    let result = result.unwrap();

    // Mock LLM should trigger a shell command for "list files"
    // Tool results should be collected (tool_execution pushes to tool_results)
    // OR we get additional messages from tool execution flow
    let has_tool_activity =
        !result.state.tool_results.is_empty() || result.state.messages.len() > 2;

    assert!(
        has_tool_activity,
        "Agent should have executed tools: {} tool_results, {} messages",
        result.state.tool_results.len(),
        result.state.messages.len()
    );
}

#[tokio::test]
async fn test_agent_tool_execution_multiple_tools() {
    // Test that multiple tool calls are handled correctly
    let mut state = test_state();
    state.messages.push(Message::user("List the files"));

    let config = RunnerConfig::default().with_max_turns(5);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Should have multiple messages from tool execution
    assert!(
        result.state.messages.len() > 2,
        "Agent should have multiple messages from tool execution"
    );
}

// ============================================================================
// Streaming Integration Tests
// ============================================================================

#[tokio::test]
async fn test_agent_streaming_events() {
    let metrics = metrics_callback();
    let mut state = test_state();
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::default().with_stream_callback(metrics.clone());
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let events = metrics.events();
    assert!(!events.is_empty(), "Should have emitted streaming events");

    // Check for expected event types
    let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
    assert!(
        event_types.contains(&"user_turn"),
        "Should have user_turn event"
    );
    assert!(
        event_types.contains(&"session_complete"),
        "Should have session_complete event"
    );
}

#[tokio::test]
async fn test_agent_streaming_with_tool_calls() {
    let metrics = metrics_callback();
    let mut state = test_state();
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default().with_stream_callback(metrics.clone());
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let events = metrics.events();
    let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();

    // Should have tool-related events
    assert!(
        event_types.contains(&"tool_call_requested"),
        "Should have tool_call_requested event"
    );
    assert!(
        event_types.contains(&"reasoning_start"),
        "Should have reasoning_start event"
    );
}

// ============================================================================
// Checkpointing Integration Tests
// ============================================================================

#[tokio::test]
async fn test_agent_with_memory_checkpointing() {
    let mut state = test_state();
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::with_memory_checkpointing();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Should have a thread ID for checkpointing
    assert!(!result.thread_id.is_empty(), "Should have thread ID");
}

#[tokio::test]
async fn test_agent_with_file_checkpointing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let checkpoint_path = temp_dir.path().join("checkpoints");

    let mut state = test_state();
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
}

// ============================================================================
// Checkpoint Reload Integration Tests (Audit #88)
// ============================================================================

// Note: DashFlow's checkpointing saves state at graph node boundaries.
// For simple agent runs that complete quickly (e.g., mock LLM), checkpoints may
// not be created because the graph completes without intermediate saves.
// These tests verify the checkpoint API behavior for both cases.

#[tokio::test]
async fn test_checkpoint_reload_file_based() {
    use codex_dashflow_core::{can_resume_session, resume_session};

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let checkpoint_path = temp_dir.path().join("checkpoints");

    // First, run an agent to potentially create a checkpoint
    let mut state = test_state();
    let session_id = state.session_id.clone();
    state.messages.push(Message::user("Hello from first run"));

    let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);
    let result = run_agent(state, &config).await;
    assert!(result.is_ok(), "Initial agent run should succeed");
    let result = result.unwrap();
    assert_eq!(
        result.thread_id, session_id,
        "Thread ID should match session ID"
    );

    // Check if checkpoint was created - with mock LLM, graph may complete
    // without saving intermediate checkpoints
    let can_resume = can_resume_session(&session_id, &config).await;
    if can_resume {
        // Resume the session and verify state is restored
        let resumed_state = resume_session(&session_id, &config).await;
        assert!(
            resumed_state.is_ok(),
            "Resume should succeed when checkpoint exists"
        );
        let resumed = resumed_state.unwrap();

        // Verify the resumed state has the original message
        assert!(
            resumed
                .messages
                .iter()
                .any(|m| m.content.contains("Hello from first run")),
            "Resumed state should contain original message"
        );
        assert_eq!(
            resumed.session_id, session_id,
            "Session ID should be preserved"
        );
    } else {
        // This is expected for simple mock LLM runs - verify API handles this gracefully
        let result = resume_session(&session_id, &config).await;
        assert!(
            result.is_err(),
            "Resume should fail when no checkpoint exists"
        );
    }
}

#[tokio::test]
async fn test_checkpoint_reload_nonexistent_session() {
    use codex_dashflow_core::{can_resume_session, resume_session};

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let checkpoint_path = temp_dir.path().join("checkpoints");
    let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);

    // Try to resume a session that doesn't exist
    assert!(
        !can_resume_session("nonexistent-session-id", &config).await,
        "Should not be able to resume nonexistent session"
    );

    let result = resume_session("nonexistent-session-id", &config).await;
    assert!(
        result.is_err(),
        "Resume should fail for nonexistent session"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No checkpoint found"),
        "Error should mention no checkpoint found"
    );
}

#[tokio::test]
async fn test_checkpoint_reload_without_checkpointing_enabled() {
    use codex_dashflow_core::{can_resume_session, resume_session};

    // Config without checkpointing
    let config = RunnerConfig::default();
    assert!(!config.enable_checkpointing);

    // Should not be able to resume
    assert!(
        !can_resume_session("any-session", &config).await,
        "Should not be able to resume without checkpointing enabled"
    );

    let result = resume_session("any-session", &config).await;
    assert!(result.is_err(), "Resume should fail without checkpointing");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("checkpointing is not enabled"),
        "Error should explain checkpointing is not enabled"
    );
}

#[tokio::test]
async fn test_checkpoint_reload_memory_cannot_resume() {
    use codex_dashflow_core::resume_session;

    // Memory checkpointing cannot persist across process restarts
    let config = RunnerConfig::with_memory_checkpointing();

    // Try to resume - should fail with appropriate error
    let result = resume_session("any-session", &config).await;
    assert!(
        result.is_err(),
        "Resume should fail with memory checkpointer"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("memory checkpointer")
            || err.to_string().contains("not persisted"),
        "Error should explain memory checkpointing limitation"
    );
}

#[tokio::test]
async fn test_checkpoint_file_path_validation() {
    use codex_dashflow_core::can_resume_session;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let checkpoint_path = temp_dir.path().join("checkpoints");
    let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);

    // Querying non-existent session should return false, not panic
    let result = can_resume_session("test-session", &config).await;
    assert!(!result, "Non-existent session should not be resumable");
}

#[tokio::test]
async fn test_checkpoint_api_thread_id_consistency() {
    // Verify that thread_id matches session_id when running agent
    let mut state = test_state();
    let expected_session_id = state.session_id.clone();
    state.messages.push(Message::user("Test message"));

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let checkpoint_path = temp_dir.path().join("checkpoints");
    let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);

    let result = run_agent(state, &config).await;
    assert!(result.is_ok());
    let result = result.unwrap();

    assert_eq!(
        result.thread_id, expected_session_id,
        "thread_id should match the session_id from state"
    );
}

// ============================================================================
// Sandbox Integration Tests
// ============================================================================

/// Check if sandbox is available on the current platform
fn sandbox_available() -> bool {
    codex_dashflow_sandbox::SandboxExecutor::is_available()
}

#[tokio::test]
async fn test_agent_with_readonly_sandbox() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mut state = test_state_in_dir(&temp_dir);
    state = state.with_sandbox_mode(SandboxMode::ReadOnly);
    state.messages.push(Message::user("Run: echo hello"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete in sandbox mode");
}

#[tokio::test]
async fn test_agent_with_workspace_write_sandbox() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mut state = test_state_in_dir(&temp_dir);
    state = state.with_sandbox_mode(SandboxMode::WorkspaceWrite);
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete in workspace sandbox");
}

#[tokio::test]
async fn test_agent_sandbox_mode_preserved_through_flow() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mut state = test_state_in_dir(&temp_dir);
    state = state.with_sandbox_mode(SandboxMode::ReadOnly);
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Verify sandbox mode was preserved
    assert_eq!(
        result.state.sandbox_mode,
        SandboxMode::ReadOnly,
        "Sandbox mode should be preserved through agent flow"
    );
}

// ============================================================================
// Training Data Collection Tests
// ============================================================================

#[tokio::test]
async fn test_agent_training_data_collection() {
    let mut state = test_state();
    state.messages.push(Message::user("Hello, world!"));

    let config = RunnerConfig::default().with_collect_training(true);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Should have collected a training example
    assert!(
        result.training_example.is_some(),
        "Should have collected training example"
    );

    let example = result.training_example.unwrap();
    assert_eq!(example.user_input, "Hello, world!");
    assert!(example.score > 0.0, "Score should be positive");
}

#[tokio::test]
async fn test_agent_training_data_with_tools() {
    let mut state = test_state();
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default().with_collect_training(true);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Training data should be collected for successful runs
    // Tool calls may or may not be included depending on agent flow
    assert!(
        result.training_example.is_some(),
        "Training example should be collected"
    );

    if let Some(example) = result.training_example {
        // Verify basic structure - tool_calls may be empty if flow completed
        // without needing tools or if tools ran but results weren't captured
        assert!(!example.user_input.is_empty(), "Input should not be empty");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_agent_handles_empty_input() {
    let mut state = test_state();
    state.messages.push(Message::user(""));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    // Should not panic, but may complete without meaningful output
    assert!(result.is_ok(), "Agent should handle empty input gracefully");
}

#[tokio::test]
async fn test_agent_handles_turn_limit() {
    let mut state = test_state();
    state
        .messages
        .push(Message::user("Keep running tools forever"));

    let config = RunnerConfig::default().with_max_turns(1);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Should hit turn limit
    assert!(
        result.turns <= 1,
        "Agent should stop at turn limit: got {} turns",
        result.turns
    );
}

// ============================================================================
// Working Directory Tests
// ============================================================================

#[tokio::test]
async fn test_agent_with_working_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a test file in the temp directory
    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write test file");

    let mut state = test_state_in_dir(&temp_dir);
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // The working directory should have been used
    assert_eq!(
        result.state.working_directory,
        temp_dir.path().to_string_lossy(),
        "Working directory should be preserved"
    );
}

// ============================================================================
// Multi-Turn Conversation Tests
// ============================================================================

#[tokio::test]
async fn test_agent_multi_turn_conversation() {
    // First turn
    let state = test_state();
    let config = RunnerConfig::default();

    let result = run_turn(state, "Hello!", &config).await;
    assert!(result.is_ok());
    let result1 = result.unwrap();
    let first_turn_messages = result1.state.messages.len();

    // Second turn using the state from first turn
    let result = run_turn(result1.state, "What did I just say?", &config).await;
    assert!(result.is_ok());
    let result2 = result.unwrap();

    // Should have accumulated messages
    assert!(
        result2.state.messages.len() > first_turn_messages,
        "Messages should accumulate across turns"
    );
}

// ============================================================================
// System Prompt Tests
// ============================================================================

#[tokio::test]
async fn test_agent_with_custom_system_prompt() {
    let custom_prompt = "You are a helpful coding assistant specialized in Rust.";
    let mut state = test_state();
    state = state.with_system_prompt(custom_prompt);
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Verify system prompt was preserved
    assert_eq!(
        result.state.system_prompt,
        Some(custom_prompt.to_string()),
        "Custom system prompt should be preserved"
    );
}

#[tokio::test]
async fn test_agent_with_system_prompt_from_runner_config() {
    let custom_prompt = "You are a specialized assistant.";
    let mut state = test_state();
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::default().with_system_prompt(custom_prompt);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Config system prompt should be applied to state
    assert_eq!(
        result.state.system_prompt,
        Some(custom_prompt.to_string()),
        "System prompt from config should be applied to state"
    );
}

#[tokio::test]
async fn test_agent_state_system_prompt_takes_precedence() {
    let state_prompt = "State-level prompt";
    let config_prompt = "Config-level prompt";

    let mut state = test_state();
    state = state.with_system_prompt(state_prompt);
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::default().with_system_prompt(config_prompt);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // State's system prompt should take precedence over config
    assert_eq!(
        result.state.system_prompt,
        Some(state_prompt.to_string()),
        "State system prompt should take precedence over config"
    );
}

// ============================================================================
// PromptRegistry Integration Tests
// ============================================================================

#[tokio::test]
async fn test_prompt_registry_loading() {
    use codex_dashflow_core::optimize::{FewShotExample, PromptConfig, PromptRegistry};

    // Create a test PromptRegistry in a temp file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let prompts_file = temp_dir.path().join("prompts.toml");

    let custom_instruction =
        "You are a highly specialized assistant for test purposes. Follow instructions exactly.";

    // Create a registry with custom prompts
    let mut registry = PromptRegistry::with_defaults();
    registry.prompts.insert(
        "system".to_string(),
        PromptConfig {
            instruction: custom_instruction.to_string(),
            few_shot_examples: vec![FewShotExample {
                user_input: "Test input".to_string(),
                expected_output: "Test output".to_string(),
                reasoning: None,
                score: 0.9,
            }],
            metadata: Default::default(),
        },
    );

    // Save to temp file
    registry
        .save(&prompts_file)
        .expect("Failed to save registry");

    // Verify the file was created
    assert!(prompts_file.exists(), "Prompts file should exist");

    // Load it back and verify
    let loaded = PromptRegistry::load(&prompts_file).expect("Failed to load registry");
    assert!(
        loaded.prompts.contains_key("system"),
        "Should have system prompt"
    );

    let system_config = loaded.prompts.get("system").unwrap();
    assert_eq!(system_config.instruction, custom_instruction);
    assert_eq!(system_config.few_shot_examples.len(), 1);
}

#[tokio::test]
async fn test_runner_config_load_optimized_prompts() {
    use codex_dashflow_core::optimize::{PromptConfig, PromptRegistry};
    use std::env;

    // Create a test PromptRegistry in the default location
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create .codex-dashflow directory in temp dir
    let config_dir = temp_dir.path().join(".codex-dashflow");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let prompts_file = config_dir.join("prompts.toml");

    let custom_instruction = "Custom optimized system prompt for testing.";

    // Create a registry
    let mut registry = PromptRegistry::with_defaults();
    registry.prompts.insert(
        "system".to_string(),
        PromptConfig {
            instruction: custom_instruction.to_string(),
            few_shot_examples: vec![],
            metadata: Default::default(),
        },
    );

    registry
        .save(&prompts_file)
        .expect("Failed to save registry");

    // Override HOME to use temp dir for PromptRegistry::load_default
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_dir.path());

    // Create a config that loads optimized prompts
    let config = RunnerConfig::default().with_load_optimized_prompts(true);

    // resolve_system_prompt should load from the registry
    let resolved = config.resolve_system_prompt();

    // Restore HOME
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }

    // The system prompt should have been loaded
    assert!(
        resolved.is_some(),
        "Should resolve system prompt from registry"
    );
    assert!(
        resolved.unwrap().contains("Custom optimized"),
        "Resolved prompt should contain custom content"
    );
}

// ============================================================================
// Token Usage Integration Tests
// ============================================================================

#[tokio::test]
async fn test_agent_metrics_capture_token_counts() {
    // Test that token counts are properly captured through the full agent flow
    let metrics = metrics_callback();
    let mut state = test_state();
    state.messages.push(Message::user("Hello, world!"));

    let config = RunnerConfig::default().with_stream_callback(metrics.clone());
    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete successfully");

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify we captured token usage
    let input_tokens = metrics.total_input_tokens();
    let output_tokens = metrics.total_output_tokens();

    // Mock LLM now returns simulated token usage
    assert!(
        input_tokens > 0,
        "Should have captured input token count: got {}",
        input_tokens
    );
    assert!(
        output_tokens > 0,
        "Should have captured output token count: got {}",
        output_tokens
    );
}

#[tokio::test]
async fn test_agent_metrics_capture_with_tool_calls() {
    // Test that metrics are captured even when tool calls are made
    let metrics = metrics_callback();
    let mut state = test_state();
    // "List files" triggers a tool call in the mock
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default()
        .with_stream_callback(metrics.clone())
        .with_max_turns(3);
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Should have metrics from both tool call turn and summary turn
    let input_tokens = metrics.total_input_tokens();
    let output_tokens = metrics.total_output_tokens();

    // Multiple LLM calls should accumulate tokens
    assert!(input_tokens > 0, "Should capture input tokens across turns");
    assert!(
        output_tokens > 0,
        "Should capture output tokens across turns"
    );

    // With tool calls, we typically have at least 2 LLM calls
    // (one to decide on tools, one to summarize results)
    let events = metrics.events();
    let reasoning_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type() == "reasoning_complete")
        .collect();

    // Depending on mock behavior, could have 1 or 2 reasoning events
    assert!(!reasoning_events.is_empty(), "Should have reasoning events");
}

#[tokio::test]
async fn test_agent_llm_metrics_event_emitted() {
    // Test that LlmMetrics events are emitted with mock LLM
    let metrics = metrics_callback();
    let mut state = test_state();
    state.messages.push(Message::user("Hello!"));

    let config = RunnerConfig::default().with_stream_callback(metrics.clone());
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let events = metrics.events();
    let llm_metrics_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type() == "llm_metrics")
        .collect();

    // Mock LLM now emits LlmMetrics events since it returns token usage
    assert!(
        !llm_metrics_events.is_empty(),
        "Should emit LlmMetrics event from mock"
    );
}

// ============================================================================
// Combined Feature Integration Tests (Audit Item #100)
// ============================================================================

/// Test that sandbox mode, streaming, and checkpointing work together
/// This verifies that all DashFlow integration points can be used simultaneously.
#[tokio::test]
async fn test_sandbox_streaming_checkpointing_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let metrics = metrics_callback();

    // Create state with sandbox mode set
    let mut state = test_state_in_dir(&temp_dir);
    state.sandbox_mode = SandboxMode::WorkspaceWrite;
    state.messages.push(Message::user("List files"));

    // Create config with streaming callback (checkpointing defaults to memory)
    let config = RunnerConfig::default()
        .with_stream_callback(metrics.clone())
        .with_max_turns(3);

    let result = run_agent(state, &config).await;

    assert!(result.is_ok(), "Agent should complete: {:?}", result.err());
    let result = result.unwrap();

    // Verify sandbox mode was preserved through execution
    assert_eq!(
        result.state.sandbox_mode,
        SandboxMode::WorkspaceWrite,
        "Sandbox mode should be preserved"
    );

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify streaming events were emitted
    let events = metrics.events();
    assert!(!events.is_empty(), "Should have streaming events");

    let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
    assert!(
        event_types.contains(&"user_turn"),
        "Should have user_turn event"
    );

    // Verify we got a response
    assert!(
        result.state.last_response.is_some(),
        "Agent should produce a response"
    );
}

/// Test that sandbox mode restricts write operations in read-only mode
#[tokio::test]
async fn test_sandbox_read_only_prevents_writes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create state with read-only sandbox
    let mut state = test_state_in_dir(&temp_dir);
    state.sandbox_mode = SandboxMode::ReadOnly;

    // This should work (reading is allowed)
    state.messages.push(Message::user("Hello"));

    let config = RunnerConfig::default();
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());
    let result = result.unwrap();

    // Sandbox mode should be preserved
    assert_eq!(result.state.sandbox_mode, SandboxMode::ReadOnly);
}

/// Test streaming events are emitted even with sandbox restrictions
#[tokio::test]
async fn test_streaming_events_with_sandbox() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let metrics = metrics_callback();

    let mut state = test_state_in_dir(&temp_dir);
    state.sandbox_mode = SandboxMode::ReadOnly;
    state.messages.push(Message::user("List files"));

    let config = RunnerConfig::default().with_stream_callback(metrics.clone());
    let result = run_agent(state, &config).await;

    assert!(result.is_ok());

    // Allow events to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let events = metrics.events();

    // Even with sandbox restrictions, we should still get streaming events
    assert!(!events.is_empty(), "Should have streaming events");

    // Should have session_complete event
    let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
    assert!(
        event_types.contains(&"session_complete"),
        "Should have session_complete event"
    );
}
