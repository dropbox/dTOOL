//! Result analysis node
//!
//! This node processes tool results and updates the conversation
//! with the outcomes.

use std::future::Future;
use std::pin::Pin;

use crate::state::{AgentState, CompletionStatus, Message};

/// Result analysis node - processes tool results
///
/// This node:
/// 1. Appends tool results to the conversation as tool messages
/// 2. Checks for completion conditions (turn limits, errors)
/// 3. Prepares state for the next reasoning iteration
pub fn result_analysis_node(
    mut state: AgentState,
) -> Pin<Box<dyn Future<Output = Result<AgentState, dashflow::Error>> + Send>> {
    Box::pin(async move {
        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            results = state.tool_results.len(),
            "Analyzing tool results"
        );

        // Convert tool results to messages
        let results = std::mem::take(&mut state.tool_results);

        // Audit #55: Max tool output size to prevent unbounded message growth
        // Tool outputs larger than this will be truncated with a note
        const MAX_TOOL_OUTPUT_CHARS: usize = 32_000;

        for result in results {
            let raw_content = if result.success {
                result.output
            } else {
                format!("Error: {}", result.output)
            };

            // Truncate if output exceeds limit
            let content = if raw_content.len() > MAX_TOOL_OUTPUT_CHARS {
                let truncated = &raw_content[..MAX_TOOL_OUTPUT_CHARS];
                // Try to truncate at a newline for cleaner output
                let truncate_at = truncated.rfind('\n').unwrap_or(MAX_TOOL_OUTPUT_CHARS - 1);
                let truncated_clean = &raw_content[..=truncate_at];
                tracing::warn!(
                    tool = %result.tool,
                    original_len = raw_content.len(),
                    truncated_len = truncated_clean.len(),
                    "Tool output truncated due to size"
                );
                format!(
                    "{}\n\n[Output truncated: {} chars shown of {} total]",
                    truncated_clean,
                    truncated_clean.len(),
                    raw_content.len()
                )
            } else {
                raw_content
            };

            state
                .messages
                .push(Message::tool(content, &result.tool_call_id));

            tracing::debug!(
                tool = %result.tool,
                success = result.success,
                duration_ms = result.duration_ms,
                "Tool result added to conversation"
            );
        }

        // Check turn limits
        if state.max_turns > 0 && state.turn_count >= state.max_turns {
            tracing::warn!(
                session_id = %state.session_id,
                turn = state.turn_count,
                max_turns = state.max_turns,
                "Turn limit reached"
            );
            state.status = CompletionStatus::TurnLimitReached;
        }

        // Emit TurnComplete event for telemetry/profiling (audit #27)
        let status_str = match &state.status {
            CompletionStatus::InProgress => "in_progress",
            CompletionStatus::Complete => "complete",
            CompletionStatus::TurnLimitReached => "turn_limit_reached",
            CompletionStatus::Interrupted => "interrupted",
            CompletionStatus::Error(_) => "error",
        };
        state.emit_event(crate::streaming::AgentEvent::TurnComplete {
            session_id: state.session_id.clone(),
            turn: state.turn_count,
            status: status_str.to_string(),
        });

        tracing::debug!(
            session_id = %state.session_id,
            status = ?state.status,
            "Result analysis complete"
        );

        Ok(state)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MessageRole, ToolResult};

    #[tokio::test]
    async fn test_result_analysis_adds_messages() {
        let mut state = AgentState::new();
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "file1.txt\nfile2.txt\n".to_string(),
            success: true,
            duration_ms: 50,
        });

        let initial_msg_count = state.messages.len();
        let result = result_analysis_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.messages.len(), initial_msg_count + 1);
        assert!(state.tool_results.is_empty());
    }

    #[tokio::test]
    async fn test_result_analysis_turn_limit() {
        let mut state = AgentState::new();
        state.max_turns = 5;
        state.turn_count = 5;
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "output".to_string(),
            success: true,
            duration_ms: 10,
        });

        let result = result_analysis_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.status, CompletionStatus::TurnLimitReached);
    }

    #[tokio::test]
    async fn test_result_analysis_failed_tool_adds_error_prefix() {
        // Failed tool results should have "Error: " prefix in message
        let mut state = AgentState::new();
        state.tool_results.push(ToolResult {
            tool_call_id: "fail-1".to_string(),
            tool: "shell".to_string(),
            output: "command not found".to_string(),
            success: false,
            duration_ms: 10,
        });

        let result = result_analysis_node(state).await.unwrap();
        let last_msg = result.messages.last().unwrap();
        assert!(
            last_msg.content.starts_with("Error: "),
            "Failed tool should have Error prefix, got: {}",
            last_msg.content
        );
    }

    #[tokio::test]
    async fn test_result_analysis_multiple_results() {
        // Multiple tool results should each create a message
        let mut state = AgentState::new();
        state.tool_results.push(ToolResult {
            tool_call_id: "call-1".to_string(),
            tool: "read_file".to_string(),
            output: "content1".to_string(),
            success: true,
            duration_ms: 5,
        });
        state.tool_results.push(ToolResult {
            tool_call_id: "call-2".to_string(),
            tool: "shell".to_string(),
            output: "content2".to_string(),
            success: true,
            duration_ms: 10,
        });

        let result = result_analysis_node(state).await.unwrap();
        assert_eq!(
            result.messages.len(),
            2,
            "Each result should create a message"
        );
        assert!(result.tool_results.is_empty());
    }

    #[tokio::test]
    async fn test_result_analysis_preserves_existing_messages() {
        // Existing messages should be preserved
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));
        state.messages.push(Message::assistant("Hi there"));
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "result".to_string(),
            success: true,
            duration_ms: 5,
        });

        let result = result_analysis_node(state).await.unwrap();
        assert_eq!(result.messages.len(), 3);
        assert!(matches!(result.messages[0].role, MessageRole::User));
        assert!(matches!(result.messages[1].role, MessageRole::Assistant));
        assert!(matches!(result.messages[2].role, MessageRole::Tool));
    }

    #[tokio::test]
    async fn test_result_analysis_tool_message_has_tool_call_id() {
        // Tool messages should include the tool_call_id
        let mut state = AgentState::new();
        state.tool_results.push(ToolResult {
            tool_call_id: "unique-call-id-123".to_string(),
            tool: "shell".to_string(),
            output: "output".to_string(),
            success: true,
            duration_ms: 5,
        });

        let result = result_analysis_node(state).await.unwrap();
        let tool_msg = result.messages.last().unwrap();
        assert_eq!(
            tool_msg.tool_call_id.as_deref(),
            Some("unique-call-id-123"),
            "Tool message should have tool_call_id"
        );
    }

    #[tokio::test]
    async fn test_result_analysis_no_turn_limit_when_zero() {
        // When max_turns is 0 (unlimited), should not trigger turn limit
        let mut state = AgentState::new();
        state.max_turns = 0; // No limit
        state.turn_count = 100;
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "output".to_string(),
            success: true,
            duration_ms: 10,
        });

        let result = result_analysis_node(state).await.unwrap();
        assert_eq!(
            result.status,
            CompletionStatus::InProgress,
            "Status should remain InProgress when max_turns is 0"
        );
    }

    #[tokio::test]
    async fn test_result_analysis_below_turn_limit() {
        // Below turn limit should keep InProgress status
        let mut state = AgentState::new();
        state.max_turns = 10;
        state.turn_count = 5;
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "output".to_string(),
            success: true,
            duration_ms: 10,
        });

        let result = result_analysis_node(state).await.unwrap();
        assert_eq!(result.status, CompletionStatus::InProgress);
    }

    #[tokio::test]
    async fn test_result_analysis_empty_results() {
        // No tool results should still succeed
        let state = AgentState::new();
        let result = result_analysis_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.messages.is_empty());
    }

    #[tokio::test]
    async fn test_result_analysis_preserves_session_id() {
        let mut state = AgentState::new();
        state.session_id = "my-session-xyz".to_string();
        state.tool_results.push(ToolResult {
            tool_call_id: "test-1".to_string(),
            tool: "shell".to_string(),
            output: "output".to_string(),
            success: true,
            duration_ms: 5,
        });

        let result = result_analysis_node(state).await.unwrap();
        assert_eq!(result.session_id, "my-session-xyz");
    }

    #[tokio::test]
    async fn test_result_analysis_truncates_large_output() {
        // Audit #55: Verify that large tool outputs are truncated
        let mut state = AgentState::new();

        // Create output larger than MAX_TOOL_OUTPUT_CHARS (32,000)
        let large_output = "x".repeat(40_000);
        state.tool_results.push(ToolResult {
            tool_call_id: "large-1".to_string(),
            tool: "shell".to_string(),
            output: large_output.clone(),
            success: true,
            duration_ms: 100,
        });

        let result = result_analysis_node(state).await.unwrap();
        let msg = result.messages.last().unwrap();

        // Should be truncated with a note
        assert!(
            msg.content.len() < large_output.len(),
            "Content should be smaller than original"
        );
        assert!(
            msg.content.contains("[Output truncated:"),
            "Should contain truncation note"
        );
        assert!(
            msg.content.contains("40000 total"),
            "Should mention original size"
        );
    }

    #[tokio::test]
    async fn test_result_analysis_small_output_not_truncated() {
        // Small outputs should pass through unchanged
        let mut state = AgentState::new();

        let small_output = "Hello, world!";
        state.tool_results.push(ToolResult {
            tool_call_id: "small-1".to_string(),
            tool: "shell".to_string(),
            output: small_output.to_string(),
            success: true,
            duration_ms: 5,
        });

        let result = result_analysis_node(state).await.unwrap();
        let msg = result.messages.last().unwrap();

        // Should be unchanged
        assert_eq!(msg.content, small_output);
        assert!(
            !msg.content.contains("[Output truncated:"),
            "Should not contain truncation note"
        );
    }
}
