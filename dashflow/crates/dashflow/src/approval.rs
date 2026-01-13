// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Built-in approval flow for StateGraph
//!
//! This module provides mechanisms for implementing human-in-the-loop approval
//! patterns in graph execution. Use this when dangerous or sensitive operations
//! require explicit user confirmation before proceeding.
//!
//! # Overview
//!
//! The approval system consists of:
//! - [`ApprovalRequest`]: Describes what needs approval (message, risk level, timeout)
//! - [`ApprovalResponse`]: User's decision (approve/deny with optional reason)
//! - [`ApprovalNode`]: A node that pauses execution until approval is received
//! - [`ApprovalChannel`]: Communication channel for sending approval requests and receiving responses
//!
//! # Example: Basic Approval Flow
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, ApprovalNode, RiskLevel};
//! use dashflow::approval::{ApprovalChannel, ApprovalRequest};
//!
//! // Create approval channel for communication
//! let (tx, rx) = ApprovalChannel::new();
//!
//! // Define state with pending command
//! #[derive(Clone, Serialize, Deserialize)]
//! struct AgentState {
//!     pending_command: String,
//!     approved: bool,
//! }
//!
//! // Create approval node
//! let approval_node = ApprovalNode::new("approve_command", |state: &AgentState| {
//!     ApprovalRequest {
//!         message: format!("Execute command: {}", state.pending_command),
//!         risk_level: RiskLevel::High,
//!         timeout: Duration::from_secs(30),
//!         context: serde_json::json!({ "command": state.pending_command }),
//!     }
//! });
//!
//! // Build graph with approval node
//! let mut graph = StateGraph::new();
//! graph.add_node("prepare", prepare_node);
//! graph.add_node("approve", approval_node);
//! graph.add_node("execute", execute_node);
//! graph.add_edge("prepare", "approve");
//! graph.add_edge("approve", "execute");
//! graph.set_entry_point("prepare");
//!
//! // Execute with approval channel
//! let app = graph.compile()?;
//! let result = app.invoke_with_approvals(state, rx).await?;
//! ```
//!
//! # Risk Levels
//!
//! - [`RiskLevel::Low`]: Informational, auto-approve recommended
//! - [`RiskLevel::Medium`]: Standard operations, user confirmation suggested
//! - [`RiskLevel::High`]: Dangerous operations, always require explicit approval
//! - [`RiskLevel::Critical`]: Irreversible or system-impacting, require additional verification

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use crate::constants::{DEFAULT_HTTP_REQUEST_TIMEOUT, DEFAULT_MPSC_CHANNEL_CAPACITY};
use crate::error::{Error, Result};
use crate::node::Node;

/// Risk level of an operation requiring approval
///
/// Used to communicate urgency and danger level to the approval UI.
/// Higher risk levels typically require more explicit confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RiskLevel {
    /// Informational or safe operations
    /// Recommendation: Auto-approve or single-click confirmation
    Low,
    /// Standard operations with some impact
    /// Recommendation: Simple confirmation dialog
    #[default]
    Medium,
    /// Dangerous operations that could cause data loss or security issues
    /// Recommendation: Explicit "I understand" confirmation
    High,
    /// Irreversible or system-impacting operations
    /// Recommendation: Multi-step verification, additional auth
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// A request for user approval
///
/// Contains all information needed for a user to make an informed decision
/// about whether to approve an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Human-readable description of what needs approval
    pub message: String,
    /// Risk level of the operation
    pub risk_level: RiskLevel,
    /// How long to wait for approval before timing out
    pub timeout: Duration,
    /// Additional context for the approval UI (JSON-serializable)
    #[serde(default)]
    pub context: serde_json::Value,
    /// Node name that generated this request
    #[serde(default)]
    pub node_name: String,
    /// Unique request ID for tracking
    #[serde(default)]
    pub request_id: String,
}

impl ApprovalRequest {
    /// Create a new approval request with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            risk_level: RiskLevel::Medium,
            timeout: DEFAULT_HTTP_REQUEST_TIMEOUT, // 30 seconds from centralized constants
            context: serde_json::Value::Null,
            node_name: String::new(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Set the risk level
    #[must_use]
    pub fn with_risk_level(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    /// Set the timeout duration
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set additional context
    #[must_use]
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    /// Set the node name (usually set automatically)
    #[must_use]
    pub fn with_node_name(mut self, name: impl Into<String>) -> Self {
        self.node_name = name.into();
        self
    }
}

/// User's response to an approval request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    /// Whether the operation was approved
    pub approved: bool,
    /// Optional reason for approval/denial
    pub reason: Option<String>,
    /// Request ID this response is for
    pub request_id: String,
}

impl ApprovalResponse {
    /// Create an approval response
    pub fn approve(request_id: impl Into<String>) -> Self {
        Self {
            approved: true,
            reason: None,
            request_id: request_id.into(),
        }
    }

    /// Create a denial response
    pub fn deny(request_id: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: None,
            request_id: request_id.into(),
        }
    }

    /// Add a reason to the response
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// A pending approval request waiting for a response
pub struct PendingApproval {
    /// The approval request
    pub request: ApprovalRequest,
    /// Channel to send the response
    response_tx: oneshot::Sender<ApprovalResponse>,
}

impl PendingApproval {
    /// Approve this request
    ///
    /// Returns `true` if the approval was delivered, `false` if the receiver was dropped.
    pub fn approve(self) -> bool {
        let request_id = self.request.request_id.clone();
        match self
            .response_tx
            .send(ApprovalResponse::approve(&request_id))
        {
            Ok(()) => true,
            Err(_) => {
                // M-191: Never silently ignore oneshot send failures
                tracing::error!(
                    request_id = %request_id,
                    "Failed to deliver approval response: receiver dropped"
                );
                false
            }
        }
    }

    /// Approve with a reason
    ///
    /// Returns `true` if the approval was delivered, `false` if the receiver was dropped.
    pub fn approve_with_reason(self, reason: impl Into<String>) -> bool {
        let request_id = self.request.request_id.clone();
        match self
            .response_tx
            .send(ApprovalResponse::approve(&request_id).with_reason(reason))
        {
            Ok(()) => true,
            Err(_) => {
                // M-191: Never silently ignore oneshot send failures
                tracing::error!(
                    request_id = %request_id,
                    "Failed to deliver approval response: receiver dropped"
                );
                false
            }
        }
    }

    /// Deny this request
    ///
    /// Returns `true` if the denial was delivered, `false` if the receiver was dropped.
    pub fn deny(self) -> bool {
        let request_id = self.request.request_id.clone();
        match self.response_tx.send(ApprovalResponse::deny(&request_id)) {
            Ok(()) => true,
            Err(_) => {
                // M-191: Never silently ignore oneshot send failures
                tracing::error!(
                    request_id = %request_id,
                    "Failed to deliver denial response: receiver dropped"
                );
                false
            }
        }
    }

    /// Deny with a reason
    ///
    /// Returns `true` if the denial was delivered, `false` if the receiver was dropped.
    pub fn deny_with_reason(self, reason: impl Into<String>) -> bool {
        let request_id = self.request.request_id.clone();
        match self
            .response_tx
            .send(ApprovalResponse::deny(&request_id).with_reason(reason))
        {
            Ok(()) => true,
            Err(_) => {
                // M-191: Never silently ignore oneshot send failures
                tracing::error!(
                    request_id = %request_id,
                    "Failed to deliver denial response: receiver dropped"
                );
                false
            }
        }
    }

    /// Get the request details
    pub fn request(&self) -> &ApprovalRequest {
        &self.request
    }
}

/// Channel for sending approval requests and receiving responses
///
/// The sender side is used by approval nodes to request approval.
/// The receiver side is used by the approval UI/handler to process requests.
pub struct ApprovalChannel {
    /// Sender for approval requests (used by nodes)
    request_tx: mpsc::Sender<PendingApproval>,
}

impl ApprovalChannel {
    /// Create a new approval channel pair
    ///
    /// Returns the channel (for nodes) and a receiver (for the approval handler).
    pub fn new() -> (Self, ApprovalReceiver) {
        let (request_tx, request_rx) = mpsc::channel(DEFAULT_MPSC_CHANNEL_CAPACITY);
        (Self { request_tx }, ApprovalReceiver { request_rx })
    }

    /// Request approval and wait for response
    ///
    /// Returns the approval response, or an error if the channel is closed
    /// or the request times out.
    pub async fn request_approval(&self, request: ApprovalRequest) -> Result<ApprovalResponse> {
        let timeout = request.timeout;
        let (response_tx, response_rx) = oneshot::channel();

        let pending = PendingApproval {
            request,
            response_tx,
        };

        self.request_tx
            .send(pending)
            .await
            .map_err(|e| Error::InternalExecutionError(format!("Approval channel closed: {e}")))?;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(Error::InternalExecutionError(
                "Approval response channel dropped".to_string(),
            )),
            Err(_) => Err(Error::Timeout(timeout)),
        }
    }
}

impl Clone for ApprovalChannel {
    fn clone(&self) -> Self {
        Self {
            request_tx: self.request_tx.clone(),
        }
    }
}

impl Default for ApprovalChannel {
    fn default() -> Self {
        Self::new().0
    }
}

/// Receiver for approval requests
///
/// Used by the approval UI/handler to receive and process approval requests.
pub struct ApprovalReceiver {
    request_rx: mpsc::Receiver<PendingApproval>,
}

impl ApprovalReceiver {
    /// Receive the next pending approval request
    ///
    /// Returns `None` if the channel is closed.
    pub async fn recv(&mut self) -> Option<PendingApproval> {
        self.request_rx.recv().await
    }

    /// Try to receive a pending approval request without waiting
    ///
    /// Returns `None` if no request is available.
    pub fn try_recv(&mut self) -> Option<PendingApproval> {
        self.request_rx.try_recv().ok()
    }
}

/// A node that requests approval before continuing execution
///
/// When executed, this node:
/// 1. Calls the `request_fn` to generate an `ApprovalRequest` from current state
/// 2. Sends the request through the approval channel
/// 3. Waits for approval (or timeout)
/// 4. If approved, continues with state unchanged (or calls optional `on_approved`)
/// 5. If denied, returns an error
///
/// # Type Parameters
///
/// - `S`: The graph state type
/// - `F`: Function type `Fn(&S) -> ApprovalRequest`
pub struct ApprovalNode<S, F>
where
    S: crate::state::MergeableState,
    F: Fn(&S) -> ApprovalRequest + Send + Sync,
{
    /// Node name
    name: String,
    /// Function to generate approval request from state
    request_fn: F,
    /// Optional approval channel (can be set later via with_channel)
    channel: Option<ApprovalChannel>,
    /// Phantom for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S, F> ApprovalNode<S, F>
where
    S: crate::state::MergeableState,
    F: Fn(&S) -> ApprovalRequest + Send + Sync,
{
    /// Create a new approval node
    ///
    /// # Arguments
    ///
    /// * `name` - Node name (used in approval request)
    /// * `request_fn` - Function that generates an approval request from state
    pub fn new(name: impl Into<String>, request_fn: F) -> Self {
        Self {
            name: name.into(),
            request_fn,
            channel: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set the approval channel for this node
    #[must_use]
    pub fn with_channel(mut self, channel: ApprovalChannel) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Get the node name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl<S, F> Node<S> for ApprovalNode<S, F>
where
    S: crate::state::MergeableState,
    F: Fn(&S) -> ApprovalRequest + Send + Sync + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        // Generate approval request from state
        let mut request = (self.request_fn)(&state);
        request.node_name = self.name.clone();

        // Get the channel, or return error if not configured
        let channel = self.channel.as_ref().ok_or_else(|| {
            Error::Validation(format!(
                "ApprovalNode '{}' executed without approval channel. \
                 Use `with_channel()` or `invoke_with_approvals()`",
                self.name
            ))
        })?;

        // Request approval
        let response = channel.request_approval(request).await?;

        if response.approved {
            Ok(state)
        } else {
            let reason = response
                .reason
                .unwrap_or_else(|| "No reason provided".to_string());
            Err(Error::Validation(format!(
                "Approval denied for node '{}': {}",
                self.name, reason
            )))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Cannot derive Clone due to PhantomData, so implement manually
impl<S, F> Clone for ApprovalNode<S, F>
where
    S: crate::state::MergeableState,
    F: Fn(&S) -> ApprovalRequest + Send + Sync + Clone,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            request_fn: self.request_fn.clone(),
            channel: self.channel.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Auto-approval policy for development/testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoApprovalPolicy {
    /// Never auto-approve (production default)
    #[default]
    Never,
    /// Auto-approve low risk operations only
    LowRiskOnly,
    /// Auto-approve low and medium risk operations
    MediumAndBelow,
    /// Auto-approve everything (testing only!)
    Always,
}

impl AutoApprovalPolicy {
    /// Check if a risk level should be auto-approved
    pub fn should_auto_approve(&self, level: RiskLevel) -> bool {
        match self {
            AutoApprovalPolicy::Never => false,
            AutoApprovalPolicy::LowRiskOnly => level == RiskLevel::Low,
            AutoApprovalPolicy::MediumAndBelow => {
                matches!(level, RiskLevel::Low | RiskLevel::Medium)
            }
            AutoApprovalPolicy::Always => true,
        }
    }
}

/// Helper to create an auto-approving handler for testing
///
/// Returns an async task that processes approval requests according to the policy.
pub async fn auto_approval_handler(mut receiver: ApprovalReceiver, policy: AutoApprovalPolicy) {
    while let Some(pending) = receiver.recv().await {
        if policy.should_auto_approve(pending.request.risk_level) {
            pending.approve_with_reason("Auto-approved by policy");
        } else {
            pending.deny_with_reason("Denied by auto-approval policy");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentState;

    #[test]
    fn test_risk_level_display() {
        assert_eq!(format!("{}", RiskLevel::Low), "Low");
        assert_eq!(format!("{}", RiskLevel::Medium), "Medium");
        assert_eq!(format!("{}", RiskLevel::High), "High");
        assert_eq!(format!("{}", RiskLevel::Critical), "Critical");
    }

    #[test]
    fn test_risk_level_default() {
        assert_eq!(RiskLevel::default(), RiskLevel::Medium);
    }

    #[test]
    fn test_approval_request_builder() {
        let request = ApprovalRequest::new("Test operation")
            .with_risk_level(RiskLevel::High)
            .with_timeout(Duration::from_secs(60))
            .with_context(serde_json::json!({"key": "value"}))
            .with_node_name("test_node");

        assert_eq!(request.message, "Test operation");
        assert_eq!(request.risk_level, RiskLevel::High);
        assert_eq!(request.timeout, Duration::from_secs(60));
        assert_eq!(request.context, serde_json::json!({"key": "value"}));
        assert_eq!(request.node_name, "test_node");
        assert!(!request.request_id.is_empty());
    }

    #[test]
    fn test_approval_response_approve() {
        let response = ApprovalResponse::approve("req-123");
        assert!(response.approved);
        assert!(response.reason.is_none());
        assert_eq!(response.request_id, "req-123");
    }

    #[test]
    fn test_approval_response_deny() {
        let response = ApprovalResponse::deny("req-456").with_reason("Too risky");
        assert!(!response.approved);
        assert_eq!(response.reason, Some("Too risky".to_string()));
        assert_eq!(response.request_id, "req-456");
    }

    #[test]
    fn test_auto_approval_policy() {
        let policy = AutoApprovalPolicy::LowRiskOnly;
        assert!(policy.should_auto_approve(RiskLevel::Low));
        assert!(!policy.should_auto_approve(RiskLevel::Medium));
        assert!(!policy.should_auto_approve(RiskLevel::High));
        assert!(!policy.should_auto_approve(RiskLevel::Critical));

        let policy = AutoApprovalPolicy::MediumAndBelow;
        assert!(policy.should_auto_approve(RiskLevel::Low));
        assert!(policy.should_auto_approve(RiskLevel::Medium));
        assert!(!policy.should_auto_approve(RiskLevel::High));
        assert!(!policy.should_auto_approve(RiskLevel::Critical));

        let policy = AutoApprovalPolicy::Always;
        assert!(policy.should_auto_approve(RiskLevel::Low));
        assert!(policy.should_auto_approve(RiskLevel::Medium));
        assert!(policy.should_auto_approve(RiskLevel::High));
        assert!(policy.should_auto_approve(RiskLevel::Critical));

        let policy = AutoApprovalPolicy::Never;
        assert!(!policy.should_auto_approve(RiskLevel::Low));
        assert!(!policy.should_auto_approve(RiskLevel::Medium));
        assert!(!policy.should_auto_approve(RiskLevel::High));
        assert!(!policy.should_auto_approve(RiskLevel::Critical));
    }

    #[tokio::test]
    async fn test_approval_channel_approve() {
        let (channel, mut receiver) = ApprovalChannel::new();

        // Spawn handler that approves
        let handle = tokio::spawn(async move {
            if let Some(pending) = receiver.recv().await {
                pending.approve();
            }
        });

        let request = ApprovalRequest::new("Test").with_timeout(Duration::from_secs(1));
        let response = channel.request_approval(request).await.unwrap();

        assert!(response.approved);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_approval_channel_deny() {
        let (channel, mut receiver) = ApprovalChannel::new();

        // Spawn handler that denies
        let handle = tokio::spawn(async move {
            if let Some(pending) = receiver.recv().await {
                pending.deny_with_reason("Not allowed");
            }
        });

        let request = ApprovalRequest::new("Test").with_timeout(Duration::from_secs(1));
        let response = channel.request_approval(request).await.unwrap();

        assert!(!response.approved);
        assert_eq!(response.reason, Some("Not allowed".to_string()));
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_approval_channel_timeout() {
        let (channel, _receiver) = ApprovalChannel::new();

        // No handler, so request will timeout
        let request = ApprovalRequest::new("Test").with_timeout(Duration::from_millis(10));
        let result = channel.request_approval(request).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    #[tokio::test]
    async fn test_approval_node_approved() {
        let (channel, mut receiver) = ApprovalChannel::new();

        // Create approval node
        let node = ApprovalNode::new("test_approval", |state: &AgentState| {
            ApprovalRequest::new(format!("Approve iteration {}", state.iteration))
                .with_risk_level(RiskLevel::Medium)
                .with_timeout(Duration::from_secs(1))
        })
        .with_channel(channel);

        // Spawn handler that approves
        let handle = tokio::spawn(async move {
            if let Some(pending) = receiver.recv().await {
                assert!(pending.request.message.contains("iteration"));
                pending.approve();
            }
        });

        let state = AgentState {
            messages: vec![],
            iteration: 5,
            next: None,
            metadata: serde_json::Value::Null,
        };

        let result = node.execute(state).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().iteration, 5);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_approval_node_denied() {
        let (channel, mut receiver) = ApprovalChannel::new();

        let node = ApprovalNode::new("test_denial", |_state: &AgentState| {
            ApprovalRequest::new("Dangerous operation")
                .with_risk_level(RiskLevel::High)
                .with_timeout(Duration::from_secs(1))
        })
        .with_channel(channel);

        // Spawn handler that denies
        let handle = tokio::spawn(async move {
            if let Some(pending) = receiver.recv().await {
                pending.deny_with_reason("Operation rejected by user");
            }
        });

        let state = AgentState::default();

        let result = node.execute(state).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Approval denied"));
        assert!(err.contains("Operation rejected by user"));

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_approval_node_no_channel() {
        let node = ApprovalNode::new("no_channel", |_state: &AgentState| {
            ApprovalRequest::new("Test")
        });

        let state = AgentState::default();
        let result = node.execute(state).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("without approval channel"));
    }

    #[tokio::test]
    async fn test_auto_approval_handler() {
        let (channel, receiver) = ApprovalChannel::new();

        // Start auto-approval handler with MediumAndBelow policy
        tokio::spawn(auto_approval_handler(
            receiver,
            AutoApprovalPolicy::MediumAndBelow,
        ));

        // Low risk should be approved
        let low_request = ApprovalRequest::new("Low risk")
            .with_risk_level(RiskLevel::Low)
            .with_timeout(Duration::from_secs(1));
        let response = channel.request_approval(low_request).await.unwrap();
        assert!(response.approved);

        // Medium risk should be approved
        let medium_request = ApprovalRequest::new("Medium risk")
            .with_risk_level(RiskLevel::Medium)
            .with_timeout(Duration::from_secs(1));
        let response = channel.request_approval(medium_request).await.unwrap();
        assert!(response.approved);

        // High risk should be denied
        let high_request = ApprovalRequest::new("High risk")
            .with_risk_level(RiskLevel::High)
            .with_timeout(Duration::from_secs(1));
        let response = channel.request_approval(high_request).await.unwrap();
        assert!(!response.approved);
    }

    #[test]
    fn test_pending_approval_request_accessor() {
        let request = ApprovalRequest::new("Test operation");
        let (response_tx, _response_rx) = oneshot::channel();
        let pending = PendingApproval {
            request: request.clone(),
            response_tx,
        };

        assert_eq!(pending.request().message, "Test operation");
    }

    #[test]
    fn test_approval_channel_clone() {
        let (channel1, _receiver) = ApprovalChannel::new();
        let channel2 = channel1.clone();

        // Both should have functional senders
        assert!(channel1.request_tx.is_closed() == channel2.request_tx.is_closed());
    }

    #[test]
    fn test_approval_request_serialization() {
        let request = ApprovalRequest::new("Test")
            .with_risk_level(RiskLevel::High)
            .with_context(serde_json::json!({"key": "value"}));

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ApprovalRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.message, "Test");
        assert_eq!(deserialized.risk_level, RiskLevel::High);
        assert_eq!(deserialized.context, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_approval_response_serialization() {
        let response = ApprovalResponse::deny("req-123").with_reason("Not authorized");

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ApprovalResponse = serde_json::from_str(&json).unwrap();

        assert!(!deserialized.approved);
        assert_eq!(deserialized.reason, Some("Not authorized".to_string()));
        assert_eq!(deserialized.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_approval_receiver_try_recv() {
        let (channel, mut receiver) = ApprovalChannel::new();

        // No pending request
        assert!(receiver.try_recv().is_none());

        // Send a request
        let handle = tokio::spawn({
            let channel = channel.clone();
            async move {
                let request = ApprovalRequest::new("Test").with_timeout(Duration::from_secs(5));
                let _ = channel.request_approval(request).await;
            }
        });

        // Wait a bit for the request to be sent
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now try_recv should return the pending request
        let pending = receiver.try_recv();
        assert!(pending.is_some());

        // Approve it so the spawn doesn't hang
        pending.unwrap().approve();
        let _ = handle.await;
    }
}
