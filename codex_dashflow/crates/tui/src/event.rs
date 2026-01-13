//! TUI event handling
//!
//! Manages terminal events and agent streaming events.

use async_trait::async_trait;
use codex_dashflow_core::codex::ApprovalDecision as CoreApprovalDecision;
use codex_dashflow_core::state::ApprovalCallback;
use codex_dashflow_core::streaming::AgentEvent;
use crossterm::event::Event as CrosstermEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};

/// User's approval decision
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalDecision {
    /// Approve this one request
    Approve,
    /// Approve and don't ask again for this tool in this session
    ApproveSession,
    /// Reject this request
    Reject,
}

/// An approval request from the agent runner
#[derive(Debug)]
pub struct ApprovalRequestEvent {
    /// Unique ID for tracking this approval request
    pub request_id: String,
    /// Tool call ID being approved
    pub tool_call_id: String,
    /// Tool name
    pub tool: String,
    /// Tool arguments as JSON
    pub args: serde_json::Value,
    /// Reason why approval is required
    pub reason: Option<String>,
    /// Channel to send the decision back
    pub response_tx: oneshot::Sender<ApprovalDecision>,
}

/// Events that the TUI can handle
#[derive(Debug)]
pub enum TuiEvent {
    /// Terminal input event (key press, resize, etc.)
    Terminal(CrosstermEvent),
    /// Agent streaming event
    Agent(AgentEvent),
    /// Tick event for periodic updates
    Tick,
    /// Signal to quit the application
    Quit,
    /// Approval request from agent runner
    ApprovalRequest(ApprovalRequestEvent),
}

/// Event handler for the TUI
pub struct EventHandler {
    /// Receiver for TUI events
    rx: mpsc::UnboundedReceiver<TuiEvent>,
    /// Sender for TUI events (cloneable for agent callbacks)
    tx: mpsc::UnboundedSender<TuiEvent>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { rx, tx }
    }

    /// Get a sender that can be used to send events to the TUI
    pub fn sender(&self) -> mpsc::UnboundedSender<TuiEvent> {
        self.tx.clone()
    }

    /// Start the event polling loop
    ///
    /// This spawns a background task that polls for terminal events
    /// and sends them through the channel.
    pub fn start(&self, tick_rate: Duration) {
        let tx = self.tx.clone();

        // Terminal event polling task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                // Poll for crossterm events with a small timeout
                if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    if let Ok(event) = crossterm::event::read() {
                        if tx.send(TuiEvent::Terminal(event)).is_err() {
                            break;
                        }
                    }
                }

                // Send tick events at the tick rate
                interval.tick().await;
                if tx.send(TuiEvent::Tick).is_err() {
                    break;
                }
            }
        });
    }

    /// Receive the next event
    pub async fn next(&mut self) -> Option<TuiEvent> {
        self.rx.recv().await
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// A stream callback that sends events to the TUI
pub struct TuiStreamCallback {
    tx: mpsc::UnboundedSender<TuiEvent>,
}

impl TuiStreamCallback {
    /// Create a new TUI stream callback
    pub fn new(tx: mpsc::UnboundedSender<TuiEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait::async_trait]
impl codex_dashflow_core::streaming::StreamCallback for TuiStreamCallback {
    async fn on_event(&self, event: AgentEvent) {
        let _ = self.tx.send(TuiEvent::Agent(event));
    }
}

/// Channel for approval communication between agent runner and TUI
///
/// The agent runner uses this to request approval for tool calls,
/// and the TUI sends decisions back through the oneshot channel.
///
/// Implements `ApprovalCallback` trait from core for use with `AgentState`.
#[derive(Clone)]
pub struct ApprovalChannel {
    /// Sender for TUI events (used by runner to send approval requests)
    event_tx: mpsc::UnboundedSender<TuiEvent>,
    /// Session-approved tools that don't need re-approval
    session_approved: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl ApprovalChannel {
    /// Create a new approval channel
    pub fn new(event_tx: mpsc::UnboundedSender<TuiEvent>) -> Self {
        Self {
            event_tx,
            session_approved: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Request approval for a tool call
    ///
    /// This method sends an approval request to the TUI and waits for a decision.
    /// Returns the decision, or `ApprovalDecision::Reject` if the channel is closed.
    pub async fn request_approval(
        &self,
        request_id: String,
        tool_call_id: String,
        tool: String,
        args: serde_json::Value,
        reason: Option<String>,
    ) -> ApprovalDecision {
        // Check if tool is session-approved
        {
            let approved = self.session_approved.lock().await;
            if approved.contains(&tool) {
                return ApprovalDecision::Approve;
            }
        }

        // Create oneshot channel for response
        let (response_tx, response_rx) = oneshot::channel();

        // Send approval request to TUI
        let request = ApprovalRequestEvent {
            request_id,
            tool_call_id,
            tool: tool.clone(),
            args,
            reason,
            response_tx,
        };

        if self
            .event_tx
            .send(TuiEvent::ApprovalRequest(request))
            .is_err()
        {
            // TUI is not listening, reject
            return ApprovalDecision::Reject;
        }

        // Wait for response
        match response_rx.await {
            Ok(decision) => {
                // Track session-approved tools
                if decision == ApprovalDecision::ApproveSession {
                    let mut approved = self.session_approved.lock().await;
                    approved.insert(tool);
                }
                decision
            }
            Err(_) => {
                // Channel closed, reject
                ApprovalDecision::Reject
            }
        }
    }

    /// Check if a tool is session-approved
    pub async fn is_session_approved(&self, tool: &str) -> bool {
        let approved = self.session_approved.lock().await;
        approved.contains(tool)
    }

    /// Clear session approvals (for future use when clearing per-turn approvals)
    #[allow(dead_code)]
    pub async fn clear_session_approvals(&self) {
        let mut approved = self.session_approved.lock().await;
        approved.clear();
    }
}

/// Implementation of the core ApprovalCallback trait for TUI
///
/// This wraps the ApprovalChannel and converts between TUI and core approval types.
/// Use this to provide the TUI's approval mechanism to the agent runner.
#[async_trait]
impl ApprovalCallback for ApprovalChannel {
    async fn request_approval(
        &self,
        request_id: &str,
        tool_call_id: &str,
        tool: &str,
        args: &serde_json::Value,
        reason: Option<&str>,
    ) -> CoreApprovalDecision {
        // Delegate to the existing request_approval method
        let decision = ApprovalChannel::request_approval(
            self,
            request_id.to_string(),
            tool_call_id.to_string(),
            tool.to_string(),
            args.clone(),
            reason.map(String::from),
        )
        .await;

        // Convert TUI ApprovalDecision to Core ApprovalDecision
        match decision {
            ApprovalDecision::Approve => CoreApprovalDecision::Approve,
            ApprovalDecision::ApproveSession => CoreApprovalDecision::ApproveAndRemember,
            ApprovalDecision::Reject => CoreApprovalDecision::Deny,
        }
    }

    async fn is_session_approved(&self, tool: &str) -> bool {
        ApprovalChannel::is_session_approved(self, tool).await
    }

    async fn mark_session_approved(&self, tool: &str) {
        let mut approved = self.session_approved.lock().await;
        approved.insert(tool.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_dashflow_core::streaming::StreamCallback;

    #[test]
    fn test_event_handler_new() {
        let handler = EventHandler::new();
        // Verify we can get a sender
        let _sender = handler.sender();
    }

    #[test]
    fn test_event_handler_default() {
        let handler = EventHandler::default();
        let _sender = handler.sender();
    }

    #[tokio::test]
    async fn test_event_handler_send_receive() {
        let mut handler = EventHandler::new();
        let sender = handler.sender();

        // Send a quit event
        sender.send(TuiEvent::Quit).unwrap();

        // Receive it
        let event = handler.next().await;
        assert!(matches!(event, Some(TuiEvent::Quit)));
    }

    #[tokio::test]
    async fn test_event_handler_multiple_events() {
        let mut handler = EventHandler::new();
        let sender = handler.sender();

        // Send multiple events
        sender.send(TuiEvent::Tick).unwrap();
        sender.send(TuiEvent::Quit).unwrap();

        // Receive them in order
        assert!(matches!(handler.next().await, Some(TuiEvent::Tick)));
        assert!(matches!(handler.next().await, Some(TuiEvent::Quit)));
    }

    #[test]
    fn test_tui_stream_callback_new() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let _callback = TuiStreamCallback::new(tx);
    }

    #[tokio::test]
    async fn test_tui_stream_callback_sends_events() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let callback = TuiStreamCallback::new(tx);

        // Send an agent event through the callback
        let agent_event = AgentEvent::UserTurn {
            session_id: "test".to_string(),
            content: "hello".to_string(),
        };
        callback.on_event(agent_event).await;

        // Verify it was received as a TuiEvent::Agent
        let tui_event = rx.recv().await.unwrap();
        match tui_event {
            TuiEvent::Agent(AgentEvent::UserTurn {
                session_id,
                content,
            }) => {
                assert_eq!(session_id, "test");
                assert_eq!(content, "hello");
            }
            _ => panic!("Expected TuiEvent::Agent(UserTurn)"),
        }
    }

    #[test]
    fn test_tui_event_debug() {
        let event = TuiEvent::Tick;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Tick"));

        let event = TuiEvent::Quit;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Quit"));
    }

    // ApprovalDecision tests

    #[test]
    fn test_approval_decision_equality() {
        assert_eq!(ApprovalDecision::Approve, ApprovalDecision::Approve);
        assert_eq!(
            ApprovalDecision::ApproveSession,
            ApprovalDecision::ApproveSession
        );
        assert_eq!(ApprovalDecision::Reject, ApprovalDecision::Reject);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Reject);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::ApproveSession);
    }

    #[test]
    fn test_approval_decision_clone() {
        let decision = ApprovalDecision::ApproveSession;
        let cloned = decision.clone();
        assert_eq!(decision, cloned);
    }

    #[test]
    fn test_approval_decision_debug() {
        let decision = ApprovalDecision::Approve;
        let debug_str = format!("{:?}", decision);
        assert!(debug_str.contains("Approve"));
    }

    // ApprovalChannel tests

    #[test]
    fn test_approval_channel_new() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let _channel = ApprovalChannel::new(tx);
    }

    #[test]
    fn test_approval_channel_clone() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let channel1 = ApprovalChannel::new(tx);
        let _channel2 = channel1.clone();
    }

    #[tokio::test]
    async fn test_approval_channel_request_approval_approve() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let channel = ApprovalChannel::new(tx);

        // Spawn a task to handle the approval request
        let handle = tokio::spawn(async move {
            match rx.recv().await {
                Some(TuiEvent::ApprovalRequest(req)) => {
                    assert_eq!(req.tool, "shell");
                    let _ = req.response_tx.send(ApprovalDecision::Approve);
                }
                _ => panic!("Expected ApprovalRequest"),
            }
        });

        let decision = channel
            .request_approval(
                "req-1".to_string(),
                "call-1".to_string(),
                "shell".to_string(),
                serde_json::json!({"command": "ls"}),
                None,
            )
            .await;

        assert_eq!(decision, ApprovalDecision::Approve);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_approval_channel_request_approval_session() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let channel = ApprovalChannel::new(tx);

        // Handle first request with ApproveSession
        let handle = tokio::spawn(async move {
            if let Some(TuiEvent::ApprovalRequest(req)) = rx.recv().await {
                let _ = req.response_tx.send(ApprovalDecision::ApproveSession);
            }
        });

        let decision = channel
            .request_approval(
                "req-1".to_string(),
                "call-1".to_string(),
                "shell".to_string(),
                serde_json::json!({}),
                None,
            )
            .await;

        assert_eq!(decision, ApprovalDecision::ApproveSession);
        handle.await.unwrap();

        // Second request for same tool should be auto-approved
        assert!(channel.is_session_approved("shell").await);
    }

    #[tokio::test]
    async fn test_approval_channel_session_approved_auto_approve() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let channel = ApprovalChannel::new(tx);

        // Manually add session-approved tool
        {
            let mut approved = channel.session_approved.lock().await;
            approved.insert("read_file".to_string());
        }

        // Request should be auto-approved without going to TUI
        let decision = channel
            .request_approval(
                "req-1".to_string(),
                "call-1".to_string(),
                "read_file".to_string(),
                serde_json::json!({}),
                None,
            )
            .await;

        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_approval_channel_clear_session_approvals() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let channel = ApprovalChannel::new(tx);

        // Add session-approved tool
        {
            let mut approved = channel.session_approved.lock().await;
            approved.insert("shell".to_string());
        }

        assert!(channel.is_session_approved("shell").await);

        // Clear approvals
        channel.clear_session_approvals().await;

        assert!(!channel.is_session_approved("shell").await);
    }

    #[tokio::test]
    async fn test_approval_channel_channel_closed_rejects() {
        let (tx, rx) = mpsc::unbounded_channel();
        let channel = ApprovalChannel::new(tx);

        // Drop receiver to close channel
        drop(rx);

        let decision = channel
            .request_approval(
                "req-1".to_string(),
                "call-1".to_string(),
                "shell".to_string(),
                serde_json::json!({}),
                None,
            )
            .await;

        assert_eq!(decision, ApprovalDecision::Reject);
    }

    #[tokio::test]
    async fn test_approval_request_event_fields() {
        let (response_tx, _response_rx) = oneshot::channel();

        let request = ApprovalRequestEvent {
            request_id: "req-123".to_string(),
            tool_call_id: "call-456".to_string(),
            tool: "write_file".to_string(),
            args: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
            reason: Some("File write operation".to_string()),
            response_tx,
        };

        assert_eq!(request.request_id, "req-123");
        assert_eq!(request.tool_call_id, "call-456");
        assert_eq!(request.tool, "write_file");
        assert_eq!(request.reason, Some("File write operation".to_string()));
    }
}
