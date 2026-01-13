//! Tool selection node
//!
//! This node validates and filters tool calls based on execpolicy
//! and user approval settings.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::execpolicy::{ApprovalRequirement, ExecPolicy};
use crate::state::{AgentState, ToolCall};
use crate::streaming::AgentEvent;

/// Tool selection node - validates and approves tool calls
///
/// This node:
/// 1. Validates tool calls against execpolicy
/// 2. Checks user approval requirements (suggest/auto-approve modes)
/// 3. Filters approved tools for execution
///
/// The policy can be configured to:
/// - Auto-approve all tools (permissive mode)
/// - Require approval for dangerous tools (default)
/// - Require approval for all tools (strict mode)
/// - Follow explicit rules for specific tool patterns
///
/// Uses the policy from `state.exec_policy()`, which returns the default
/// dangerous patterns policy if not explicitly configured.
pub fn tool_selection_node(
    state: AgentState,
) -> Pin<Box<dyn Future<Output = Result<AgentState, dashflow::Error>> + Send>> {
    // Use policy from state (defaults to dangerous patterns if not configured)
    let policy = state.exec_policy();
    tool_selection_with_policy(state, policy)
}

/// Tool selection with a specific policy
pub fn tool_selection_with_policy(
    mut state: AgentState,
    policy: Arc<ExecPolicy>,
) -> Pin<Box<dyn Future<Output = Result<AgentState, dashflow::Error>> + Send>> {
    Box::pin(async move {
        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            pending_tools = state.pending_tool_calls.len(),
            "Selecting tools for execution"
        );

        let mut approved_calls = Vec::new();
        let mut needs_approval = Vec::new();
        let mut forbidden = Vec::new();

        // Evaluate each pending tool call against the policy
        for tool_call in &state.pending_tool_calls {
            let requirement = policy.evaluate(tool_call);

            match requirement {
                ApprovalRequirement::Approved => {
                    tracing::info!(
                        tool = %tool_call.tool,
                        id = %tool_call.id,
                        "Tool call approved"
                    );
                    // Audit #74: Emit streaming event for approved tool
                    state.emit_event(AgentEvent::ToolCallApproved {
                        session_id: state.session_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        tool: tool_call.tool.clone(),
                    });
                    approved_calls.push(tool_call.clone());
                }
                ApprovalRequirement::NeedsApproval { reason } => {
                    tracing::info!(
                        tool = %tool_call.tool,
                        id = %tool_call.id,
                        reason = ?reason,
                        "Tool call needs approval"
                    );
                    // In interactive mode, this would pause for user approval
                    // For now, we auto-approve if reason indicates it's just cautionary
                    // In production, this would integrate with TUI for approval
                    needs_approval.push((tool_call.clone(), reason));
                }
                ApprovalRequirement::Forbidden { reason } => {
                    tracing::warn!(
                        tool = %tool_call.tool,
                        id = %tool_call.id,
                        reason = %reason,
                        "Tool call forbidden"
                    );
                    // Audit #74: Emit streaming event for forbidden tool
                    state.emit_event(AgentEvent::ToolCallRejected {
                        session_id: state.session_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        tool: tool_call.tool.clone(),
                        reason: reason.clone(),
                    });
                    forbidden.push((tool_call.clone(), reason));
                }
            }
        }

        // Log summary
        let approved_count = approved_calls.len();
        let needs_approval_count = needs_approval.len();
        let forbidden_count = forbidden.len();

        tracing::debug!(
            session_id = %state.session_id,
            approved = approved_count,
            needs_approval = needs_approval_count,
            forbidden = forbidden_count,
            "Tool selection complete"
        );

        // Audit #53: DO NOT auto-approve NeedsApproval tools here.
        // The tool_execution_node handles approval via check_tool_approval() which:
        // - Checks session approval state
        // - Emits ApprovalRequired events for TUI
        // - Calls the approval callback for interactive approval
        // By passing NeedsApproval tools through, we let tool_execution handle them properly.
        for (tool_call, _reason) in needs_approval {
            approved_calls.push(tool_call);
        }

        // Audit #54: Add feedback to the LLM about forbidden tools
        // This helps the model understand why certain tool calls were rejected
        // and avoid requesting them again.
        if !forbidden.is_empty() {
            let forbidden_msg = forbidden
                .iter()
                .map(|(tc, reason)| format!("- {} ({}): {}", tc.tool, tc.id, reason))
                .collect::<Vec<_>>()
                .join("\n");

            tracing::warn!("Forbidden tool calls:\n{}", forbidden_msg);

            // Add a system message informing the LLM about forbidden tools
            // This is added as a tool result message so the LLM understands the rejection
            use crate::state::ToolResult;
            for (tc, reason) in &forbidden {
                // Create a tool result indicating the rejection
                let rejection_result = ToolResult {
                    tool_call_id: tc.id.clone(),
                    tool: tc.tool.clone(),
                    output: format!(
                        "Error: This tool call was forbidden by the security policy. Reason: {}. \
                         Please use a different approach or ask the user for permission.",
                        reason
                    ),
                    success: false,
                    duration_ms: 0,
                };
                state.tool_results.push(rejection_result);
            }
        }

        // Update state with approved tool calls (and NeedsApproval tools for execution node)
        state.pending_tool_calls = approved_calls;

        Ok(state)
    })
}

/// Result of tool selection evaluation
#[derive(Clone, Debug)]
pub struct SelectionResult {
    /// Tool calls approved for execution
    pub approved: Vec<ToolCall>,
    /// Tool calls needing user approval (with reasons)
    pub needs_approval: Vec<(ToolCall, Option<String>)>,
    /// Tool calls that are forbidden (with reasons)
    pub forbidden: Vec<(ToolCall, String)>,
}

impl SelectionResult {
    /// Check if all tools were approved
    pub fn all_approved(&self) -> bool {
        self.needs_approval.is_empty() && self.forbidden.is_empty()
    }

    /// Check if any tools were forbidden
    pub fn has_forbidden(&self) -> bool {
        !self.forbidden.is_empty()
    }

    /// Get count of approved tools
    pub fn approved_count(&self) -> usize {
        self.approved.len()
    }
}

/// Evaluate tool calls against a policy without modifying state
/// Useful for pre-checking before submission
pub fn evaluate_tool_calls(tool_calls: &[ToolCall], policy: &ExecPolicy) -> SelectionResult {
    let mut approved = Vec::new();
    let mut needs_approval = Vec::new();
    let mut forbidden = Vec::new();

    for tool_call in tool_calls {
        match policy.evaluate(tool_call) {
            ApprovalRequirement::Approved => {
                approved.push(tool_call.clone());
            }
            ApprovalRequirement::NeedsApproval { reason } => {
                needs_approval.push((tool_call.clone(), reason));
            }
            ApprovalRequirement::Forbidden { reason } => {
                forbidden.push((tool_call.clone(), reason));
            }
        }
    }

    SelectionResult {
        approved,
        needs_approval,
        forbidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execpolicy::{Decision, PolicyRule};

    #[tokio::test]
    async fn test_tool_selection_approves_safe_tools() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
    }

    #[tokio::test]
    async fn test_tool_selection_filters_forbidden() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "rm -rf /*"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        // Forbidden tool should be filtered out
        assert_eq!(state.pending_tool_calls.len(), 0);
    }

    #[tokio::test]
    async fn test_tool_selection_permissive_mode() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "ls -la"}),
        ));
        state.pending_tool_calls.push(ToolCall::new(
            "write_file",
            serde_json::json!({"path": "test.txt", "content": "hello"}),
        ));

        let policy = ExecPolicy::permissive();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 2);
    }

    #[tokio::test]
    async fn test_tool_selection_with_custom_rules() {
        let mut state = AgentState::new();
        state
            .pending_tool_calls
            .push(ToolCall::new("custom_tool", serde_json::json!({})));

        let mut policy = ExecPolicy::new();
        policy.add_rule(
            PolicyRule::new("custom_tool", Decision::Forbidden)
                .with_reason("Custom tool not allowed"),
        );

        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 0);
    }

    #[test]
    fn test_evaluate_tool_calls() {
        let tool_calls = vec![
            ToolCall::new("read_file", serde_json::json!({"path": "test.txt"})),
            ToolCall::new("shell", serde_json::json!({"command": "rm -rf /*"})),
            ToolCall::new("write_file", serde_json::json!({"path": "out.txt"})),
        ];

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = evaluate_tool_calls(&tool_calls, &policy);

        // read_file is allowed
        assert_eq!(result.approved.len(), 1);
        // write_file needs approval (dangerous tool in default mode)
        assert_eq!(result.needs_approval.len(), 1);
        // shell rm -rf is forbidden
        assert_eq!(result.forbidden.len(), 1);
    }

    #[test]
    fn test_selection_result_helpers() {
        let result = SelectionResult {
            approved: vec![ToolCall::new("read_file", serde_json::json!({}))],
            needs_approval: vec![],
            forbidden: vec![],
        };

        assert!(result.all_approved());
        assert!(!result.has_forbidden());
        assert_eq!(result.approved_count(), 1);

        let result_with_forbidden = SelectionResult {
            approved: vec![],
            needs_approval: vec![],
            forbidden: vec![(
                ToolCall::new("shell", serde_json::json!({})),
                "forbidden".to_string(),
            )],
        };

        assert!(!result_with_forbidden.all_approved());
        assert!(result_with_forbidden.has_forbidden());
    }

    #[tokio::test]
    async fn test_tool_selection_empty_pending_calls() {
        let state = AgentState::new();
        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.pending_tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_tool_selection_preserves_session_id() {
        let mut state = AgentState::new();
        let original_session_id = state.session_id.clone();
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.session_id, original_session_id);
    }

    #[tokio::test]
    async fn test_tool_selection_preserves_turn_count() {
        let mut state = AgentState::new();
        state.turn_count = 5;
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.turn_count, 5);
    }

    #[tokio::test]
    async fn test_tool_selection_preserves_messages() {
        use crate::state::Message;

        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));
        state.messages.push(Message::assistant("Hi there"));
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_tool_selection_multiple_safe_tools() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "file1.txt"}),
        ));
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "file2.txt"}),
        ));
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "file3.txt"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 3);
    }

    #[tokio::test]
    async fn test_tool_selection_mixed_safe_and_forbidden() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "safe.txt"}),
        ));
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "rm -rf /*"}), // forbidden
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        // Only safe tool should remain
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "read_file");
    }

    #[tokio::test]
    async fn test_tool_selection_needs_approval_auto_approved() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "write_file",
            serde_json::json!({"path": "test.txt", "content": "hello"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        // In non-interactive mode, needs_approval tools are auto-approved
        assert_eq!(state.pending_tool_calls.len(), 1);
    }

    #[test]
    fn test_evaluate_tool_calls_all_safe() {
        let tool_calls = vec![
            ToolCall::new("read_file", serde_json::json!({"path": "a.txt"})),
            ToolCall::new("read_file", serde_json::json!({"path": "b.txt"})),
        ];

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = evaluate_tool_calls(&tool_calls, &policy);

        assert_eq!(result.approved.len(), 2);
        assert!(result.needs_approval.is_empty());
        assert!(result.forbidden.is_empty());
        assert!(result.all_approved());
    }

    #[test]
    fn test_evaluate_tool_calls_all_forbidden() {
        // Both commands match the destructive rm -rf pattern which is forbidden
        let tool_calls = vec![
            ToolCall::new("shell", serde_json::json!({"command": "rm -rf /*"})),
            ToolCall::new("shell", serde_json::json!({"command": "rm -rf /var/*"})),
        ];

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = evaluate_tool_calls(&tool_calls, &policy);

        assert!(result.approved.is_empty());
        assert!(result.needs_approval.is_empty());
        assert_eq!(result.forbidden.len(), 2);
        assert!(result.has_forbidden());
    }

    #[test]
    fn test_evaluate_tool_calls_empty() {
        let tool_calls: Vec<ToolCall> = vec![];

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = evaluate_tool_calls(&tool_calls, &policy);

        assert!(result.approved.is_empty());
        assert!(result.needs_approval.is_empty());
        assert!(result.forbidden.is_empty());
        assert!(result.all_approved());
    }

    #[test]
    fn test_selection_result_needs_approval_not_all_approved() {
        let result = SelectionResult {
            approved: vec![ToolCall::new("read_file", serde_json::json!({}))],
            needs_approval: vec![(ToolCall::new("write_file", serde_json::json!({})), None)],
            forbidden: vec![],
        };

        assert!(!result.all_approved()); // needs_approval present
        assert!(!result.has_forbidden());
    }

    #[tokio::test]
    async fn test_tool_selection_default_node() {
        // Test the main tool_selection_node function (uses state's exec_policy)
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));

        let result = tool_selection_node(state).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
    }

    #[tokio::test]
    async fn test_tool_selection_with_state_policy() {
        use crate::execpolicy::ExecPolicy;

        let mut state = AgentState::new();
        // Set a permissive policy on the state
        state = state.with_exec_policy(Arc::new(ExecPolicy::permissive()));
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "dangerous command"}),
        ));

        let result = tool_selection_node(state).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        // Permissive policy allows everything
        assert_eq!(state.pending_tool_calls.len(), 1);
    }

    #[tokio::test]
    async fn test_tool_selection_preserves_tool_call_ids() {
        let mut state = AgentState::new();
        let tool_call = ToolCall::new("read_file", serde_json::json!({"path": "test.txt"}));
        let original_id = tool_call.id.clone();
        state.pending_tool_calls.push(tool_call);

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls[0].id, original_id);
    }

    #[tokio::test]
    async fn test_tool_selection_preserves_tool_call_args() {
        let mut state = AgentState::new();
        let args = serde_json::json!({"path": "specific/path.txt", "encoding": "utf-8"});
        state
            .pending_tool_calls
            .push(ToolCall::new("read_file", args.clone()));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls[0].args, args);
    }

    // Audit #54: Test that forbidden tools add feedback to tool_results
    #[tokio::test]
    async fn test_forbidden_tools_add_rejection_to_tool_results() {
        let mut state = AgentState::new();
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "rm -rf /*"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();

        // Forbidden tool should be filtered from pending
        assert_eq!(state.pending_tool_calls.len(), 0);

        // But a rejection result should be added to tool_results
        assert_eq!(
            state.tool_results.len(),
            1,
            "Forbidden tool should generate a rejection result"
        );
        assert!(!state.tool_results[0].success);
        assert!(state.tool_results[0].output.contains("forbidden"));
        assert!(state.tool_results[0].output.contains("security policy"));
    }

    #[tokio::test]
    async fn test_forbidden_tools_rejection_contains_tool_call_id() {
        let mut state = AgentState::new();
        let tool_call = ToolCall::new("shell", serde_json::json!({"command": "rm -rf /*"}));
        let tool_call_id = tool_call.id.clone();
        state.pending_tool_calls.push(tool_call);

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();

        assert_eq!(state.tool_results.len(), 1);
        // The rejection should have the same tool_call_id so the LLM can correlate it
        assert_eq!(state.tool_results[0].tool_call_id, tool_call_id);
        assert_eq!(state.tool_results[0].tool, "shell");
    }

    #[tokio::test]
    async fn test_mixed_approved_and_forbidden_tools() {
        let mut state = AgentState::new();
        // Safe tool
        state.pending_tool_calls.push(ToolCall::new(
            "read_file",
            serde_json::json!({"path": "safe.txt"}),
        ));
        // Forbidden tool
        state.pending_tool_calls.push(ToolCall::new(
            "shell",
            serde_json::json!({"command": "rm -rf /*"}),
        ));

        let policy = ExecPolicy::with_dangerous_patterns();
        let result = tool_selection_with_policy(state, Arc::new(policy)).await;

        assert!(result.is_ok());
        let state = result.unwrap();

        // Only safe tool should remain in pending
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "read_file");

        // Forbidden tool should have a rejection result
        assert_eq!(state.tool_results.len(), 1);
        assert_eq!(state.tool_results[0].tool, "shell");
        assert!(!state.tool_results[0].success);
    }
}
