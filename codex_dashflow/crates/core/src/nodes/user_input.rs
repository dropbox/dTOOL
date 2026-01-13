//! User input node
//!
//! This node receives user messages and prepares them for the reasoning step.
//! In the full implementation, this would integrate with the TUI or exec mode.

use std::future::Future;
use std::pin::Pin;

use crate::state::AgentState;

/// User input node - receives and processes user input
///
/// This node is the entry point for each agent turn. It:
/// 1. Validates the current state
/// 2. Prepares messages for the reasoning step
/// 3. Increments the turn counter
///
/// In the TUI, user input is added to state before invoking the graph.
/// This node handles any preprocessing needed.
///
/// For checkpoint resume (audit #28): If `turn_count > 0` and there are
/// pending tool calls or tool results, we allow resuming without requiring
/// a fresh user message - the agent is mid-turn and should continue.
pub fn user_input_node(
    mut state: AgentState,
) -> Pin<Box<dyn Future<Output = Result<AgentState, dashflow::Error>> + Send>> {
    Box::pin(async move {
        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            "Processing user input"
        );

        // Validate we have at least one user message
        let has_user_message = state
            .messages
            .iter()
            .any(|m| matches!(m.role, crate::state::MessageRole::User));

        // Allow checkpoint resume mid-turn (audit #28):
        // If turn_count > 0 and we have pending work, skip the user message requirement
        let is_mid_turn_resume = state.turn_count > 0
            && (!state.pending_tool_calls.is_empty() || !state.tool_results.is_empty());

        if !has_user_message && !is_mid_turn_resume {
            return Err(dashflow::Error::Generic(
                "No user message in state".to_string(),
            ));
        }

        // Clear any stale tool results from previous turns
        // (but only if this is a new turn, not a mid-turn resume)
        if !is_mid_turn_resume {
            state.tool_results.clear();
        }

        // Increment turn counter only on fresh turns, not on resume
        if !is_mid_turn_resume {
            state.turn_count += 1;
        }

        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            message_count = state.messages.len(),
            is_mid_turn_resume = is_mid_turn_resume,
            "User input processed"
        );

        Ok(state)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Message, ToolResult};

    #[tokio::test]
    async fn test_user_input_node_with_message() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));

        let result = user_input_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.turn_count, 1);
    }

    #[tokio::test]
    async fn test_user_input_node_no_message() {
        let state = AgentState::new();
        let result = user_input_node(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_input_node_clears_tool_results() {
        // Verify that tool_results from previous turns are cleared
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));
        state.tool_results.push(ToolResult {
            tool_call_id: "old_result".to_string(),
            tool: "shell".to_string(),
            output: "stale output".to_string(),
            success: true,
            duration_ms: 100,
        });

        let result = user_input_node(state).await.unwrap();
        assert!(
            result.tool_results.is_empty(),
            "Tool results should be cleared"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_increments_turn_count() {
        // Verify turn counter is incremented correctly across multiple turns
        let mut state = AgentState::new();
        state.messages.push(Message::user("First message"));
        state.turn_count = 5; // Simulate some turns already happened

        let result = user_input_node(state).await.unwrap();
        assert_eq!(
            result.turn_count, 6,
            "Turn count should be incremented by 1"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_preserves_messages() {
        // Verify that existing messages are preserved
        let mut state = AgentState::new();
        state.messages.push(Message::user("User message 1"));
        state
            .messages
            .push(Message::assistant("Assistant response"));
        state.messages.push(Message::user("User message 2"));

        let result = user_input_node(state).await.unwrap();
        assert_eq!(result.messages.len(), 3, "All messages should be preserved");
    }

    #[tokio::test]
    async fn test_user_input_node_with_only_assistant_message() {
        // Verify that having only assistant messages is rejected
        let mut state = AgentState::new();
        state
            .messages
            .push(Message::assistant("I am the assistant"));

        let result = user_input_node(state).await;
        assert!(
            result.is_err(),
            "Should fail when only assistant messages present"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_error_message_content() {
        // Verify the error message is descriptive
        let state = AgentState::new();
        let result = user_input_node(state).await;
        match result {
            Err(dashflow::Error::Generic(msg)) => {
                assert!(
                    msg.contains("No user message"),
                    "Error should mention missing user message"
                );
            }
            _ => panic!("Expected Generic error"),
        }
    }

    #[tokio::test]
    async fn test_user_input_node_preserves_session_id() {
        // Verify session_id is preserved through the node
        let mut state = AgentState::new();
        state.session_id = "test-session-123".to_string();
        state.messages.push(Message::user("Hello"));

        let result = user_input_node(state).await.unwrap();
        assert_eq!(
            result.session_id, "test-session-123",
            "Session ID should be preserved"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_mid_turn_resume_with_pending_tools() {
        // Audit #28: Allow checkpoint resume mid-turn when there are pending tool calls
        use crate::state::ToolCall;

        let mut state = AgentState::new();
        state.turn_count = 3; // Simulating mid-session
                              // No user message, but we have pending tool calls
        state.pending_tool_calls.push(ToolCall {
            id: "call_123".to_string(),
            tool: "shell".to_string(),
            args: serde_json::json!({"command": "ls"}),
        });

        let result = user_input_node(state).await;
        assert!(
            result.is_ok(),
            "Should allow resume with pending tool calls even without user message"
        );
        let state = result.unwrap();
        // Turn count should NOT increment on mid-turn resume
        assert_eq!(
            state.turn_count, 3,
            "Turn count should not increment on mid-turn resume"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_mid_turn_resume_with_tool_results() {
        // Audit #28: Allow checkpoint resume mid-turn when there are tool results
        let mut state = AgentState::new();
        state.turn_count = 2;
        // No user message, but we have tool results pending
        state.tool_results.push(ToolResult {
            tool_call_id: "call_456".to_string(),
            tool: "read_file".to_string(),
            output: "file contents".to_string(),
            success: true,
            duration_ms: 50,
        });

        let result = user_input_node(state).await;
        assert!(
            result.is_ok(),
            "Should allow resume with tool results even without user message"
        );
        let state = result.unwrap();
        // Tool results should be preserved on mid-turn resume
        assert_eq!(
            state.tool_results.len(),
            1,
            "Tool results should be preserved on mid-turn resume"
        );
    }

    #[tokio::test]
    async fn test_user_input_node_fresh_turn_requires_user_message() {
        // Turn count 0 with no user message should fail
        let state = AgentState::new();
        assert_eq!(state.turn_count, 0);

        let result = user_input_node(state).await;
        assert!(
            result.is_err(),
            "Fresh turn (turn_count=0) should require user message"
        );
    }
}
