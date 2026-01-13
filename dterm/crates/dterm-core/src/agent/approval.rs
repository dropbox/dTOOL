//! Agent approval workflow.
//!
//! Implements the approval state machine from `tla/AgentApproval.tla`.
//!
//! ## TLA+ Safety Invariants
//!
//! - **INV-APPROVAL-1**: No request is both approved AND rejected
//! - **INV-APPROVAL-2**: All completed requests have audit entries
//! - **INV-APPROVAL-3**: Pending requests have no completion time
//! - **INV-APPROVAL-4**: Completed requests have valid completion time
//! - **INV-APPROVAL-5**: Request IDs are unique and sequential
//! - **INV-APPROVAL-6**: Timeout only possible if request exceeded timeout
//!
//! ## State Machine
//!
//! ```text
//! ┌─────────┐  Approve   ┌──────────┐
//! │         │───────────▶│ Approved │
//! │         │            └──────────┘
//! │ Pending │
//! │         │  Reject    ┌──────────┐
//! │         │───────────▶│ Rejected │
//! │         │            └──────────┘
//! │         │
//! │         │  Timeout   ┌──────────┐
//! │         │───────────▶│ TimedOut │
//! │         │            └──────────┘
//! │         │
//! │         │  Cancel    ┌───────────┐
//! └─────────┴───────────▶│ Cancelled │
//!                        └───────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::agent::approval::{ApprovalManager, ApprovalConfig, Action};
//!
//! let config = ApprovalConfig::default();
//! let mut manager = ApprovalManager::new(config);
//!
//! // Agent requests approval for a dangerous action
//! let request_id = manager.submit_request(agent_id, Action::Shell, "rm -rf /")?;
//!
//! // User reviews and approves
//! manager.approve(request_id)?;
//!
//! // Check result
//! assert!(manager.is_approved(request_id));
//! ```

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::{Duration, Instant};

use super::AgentId;

/// Unique identifier for an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApprovalRequestId(pub u64);

impl fmt::Display for ApprovalRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApprovalReq({})", self.0)
    }
}

/// State of an approval request.
///
/// Maps to `RequestStates` in TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    /// Waiting for user decision
    Pending,
    /// User approved the action
    Approved,
    /// User rejected the action
    Rejected,
    /// Request timed out (auto-reject)
    TimedOut,
    /// Agent cancelled the request
    Cancelled,
}

impl ApprovalState {
    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ApprovalState::Approved
                | ApprovalState::Rejected
                | ApprovalState::TimedOut
                | ApprovalState::Cancelled
        )
    }
}

impl fmt::Display for ApprovalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ApprovalState::Pending => "Pending",
            ApprovalState::Approved => "Approved",
            ApprovalState::Rejected => "Rejected",
            ApprovalState::TimedOut => "TimedOut",
            ApprovalState::Cancelled => "Cancelled",
        };
        write!(f, "{}", name)
    }
}

/// Types of actions that may require approval.
///
/// Maps to `Actions` constant in TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Shell command execution
    Shell,
    /// File write/delete operation
    FileWrite,
    /// Network access
    Network,
    /// Git push/force operations
    GitPush,
    /// Package installation
    PackageInstall,
    /// Container operations
    Container,
    /// Database modification
    DatabaseWrite,
    /// System administration
    Admin,
}

impl Action {
    /// Get the risk level of this action (0-3).
    ///
    /// Higher levels require more scrutiny.
    pub fn risk_level(&self) -> u8 {
        match self {
            Action::Shell => 2,          // Medium-high: arbitrary commands
            Action::FileWrite => 2,      // Medium-high: data modification
            Action::Network => 1,        // Low-medium: external access
            Action::GitPush => 2,        // Medium-high: code changes
            Action::PackageInstall => 2, // Medium-high: system changes
            Action::Container => 3,      // High: isolation escape risk
            Action::DatabaseWrite => 2,  // Medium-high: data modification
            Action::Admin => 3,          // High: system control
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Action::Shell => "shell",
            Action::FileWrite => "file_write",
            Action::Network => "network",
            Action::GitPush => "git_push",
            Action::PackageInstall => "package_install",
            Action::Container => "container",
            Action::DatabaseWrite => "database_write",
            Action::Admin => "admin",
        };
        write!(f, "{}", name)
    }
}

/// An approval request from an agent.
///
/// Maps to `Request` record in TLA+ spec.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// Unique identifier
    pub id: ApprovalRequestId,
    /// Agent that submitted the request
    pub agent_id: AgentId,
    /// Type of action requiring approval
    pub action: Action,
    /// Current state
    pub state: ApprovalState,
    /// When the request was created
    pub created_at: Instant,
    /// When the request was completed (None if pending)
    pub completed_at: Option<Instant>,
    /// Human-readable description
    pub description: String,
}

impl ApprovalRequest {
    /// Create a new pending approval request.
    fn new(id: ApprovalRequestId, agent_id: AgentId, action: Action, description: String) -> Self {
        Self {
            id,
            agent_id,
            action,
            state: ApprovalState::Pending,
            created_at: Instant::now(),
            completed_at: None,
            description,
        }
    }

    /// Check if the request has exceeded the timeout duration.
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.state == ApprovalState::Pending && self.created_at.elapsed() >= timeout
    }

    /// Get the age of this request.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Entry in the audit log.
///
/// Maps to `AuditEntry` record in TLA+ spec.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// ID of the request
    pub request_id: ApprovalRequestId,
    /// Agent that made the request
    pub agent_id: AgentId,
    /// Action that was requested
    pub action: Action,
    /// Decision made
    pub decision: ApprovalState,
    /// When the decision was made
    pub timestamp: Instant,
    /// Description of the request
    pub description: String,
}

/// Configuration for the approval manager.
#[derive(Debug, Clone)]
pub struct ApprovalConfig {
    /// Maximum concurrent approval requests
    pub max_requests: usize,
    /// Maximum pending requests per agent
    pub max_per_agent: usize,
    /// Timeout duration before auto-reject
    pub timeout: Duration,
    /// Maximum audit log entries to retain
    pub max_audit_entries: usize,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            max_per_agent: 10,
            timeout: Duration::from_secs(300), // 5 minutes
            max_audit_entries: 1000,
        }
    }
}

/// Errors from approval operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// Maximum requests reached
    MaxRequestsReached,
    /// Maximum requests per agent reached
    MaxPerAgentReached,
    /// Request not found
    RequestNotFound(ApprovalRequestId),
    /// Request not in pending state
    NotPending(ApprovalRequestId),
    /// Agent mismatch (can only cancel own requests)
    AgentMismatch,
    /// Invalid state transition
    InvalidStateTransition(String),
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalError::MaxRequestsReached => {
                write!(f, "Maximum approval requests reached")
            }
            ApprovalError::MaxPerAgentReached => {
                write!(f, "Maximum requests per agent reached")
            }
            ApprovalError::RequestNotFound(id) => {
                write!(f, "Approval request not found: {}", id)
            }
            ApprovalError::NotPending(id) => {
                write!(f, "Request {} is not pending", id)
            }
            ApprovalError::AgentMismatch => {
                write!(f, "Agent can only cancel own requests")
            }
            ApprovalError::InvalidStateTransition(msg) => {
                write!(f, "Invalid state transition: {}", msg)
            }
        }
    }
}

impl std::error::Error for ApprovalError {}

/// Result type for approval operations.
pub type ApprovalResult<T> = Result<T, ApprovalError>;

/// Callback trait for approval UI integration.
///
/// Implement this trait to receive notifications about approval requests.
pub trait ApprovalCallback: Send + Sync {
    /// Called when a new approval request is submitted.
    fn on_request_submitted(&self, request: &ApprovalRequest);

    /// Called when a request is approved.
    fn on_request_approved(&self, request: &ApprovalRequest);

    /// Called when a request is rejected.
    fn on_request_rejected(&self, request: &ApprovalRequest);

    /// Called when a request times out.
    fn on_request_timeout(&self, request: &ApprovalRequest);

    /// Called when a request is cancelled.
    fn on_request_cancelled(&self, request: &ApprovalRequest);
}

/// No-op implementation for testing.
#[derive(Debug, Default)]
pub struct NullApprovalCallback;

impl ApprovalCallback for NullApprovalCallback {
    fn on_request_submitted(&self, _request: &ApprovalRequest) {}
    fn on_request_approved(&self, _request: &ApprovalRequest) {}
    fn on_request_rejected(&self, _request: &ApprovalRequest) {}
    fn on_request_timeout(&self, _request: &ApprovalRequest) {}
    fn on_request_cancelled(&self, _request: &ApprovalRequest) {}
}

/// Manager for approval requests.
///
/// Implements the state machine from `AgentApproval.tla`.
pub struct ApprovalManager {
    /// Configuration
    config: ApprovalConfig,
    /// All requests by ID
    requests: HashMap<ApprovalRequestId, ApprovalRequest>,
    /// Next request ID
    next_id: u64,
    /// Audit log (bounded)
    audit_log: VecDeque<AuditEntry>,
    /// Pending requests per agent (for limit enforcement)
    pending_per_agent: HashMap<AgentId, usize>,
    /// Callback for UI notifications
    callback: Box<dyn ApprovalCallback>,
}

impl ApprovalManager {
    /// Create a new approval manager with the given configuration.
    pub fn new(config: ApprovalConfig) -> Self {
        Self {
            config,
            requests: HashMap::new(),
            next_id: 0,
            audit_log: VecDeque::new(),
            pending_per_agent: HashMap::new(),
            callback: Box::new(NullApprovalCallback),
        }
    }

    /// Create an approval manager with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ApprovalConfig::default())
    }

    /// Set the callback for UI notifications.
    pub fn set_callback(&mut self, callback: Box<dyn ApprovalCallback>) {
        self.callback = callback;
    }

    // =========================================================================
    // Request Operations (TLA+ Actions)
    // =========================================================================

    /// Submit a new approval request.
    ///
    /// Implements `SubmitRequest` from TLA+ spec.
    pub fn submit_request(
        &mut self,
        agent_id: AgentId,
        action: Action,
        description: impl Into<String>,
    ) -> ApprovalResult<ApprovalRequestId> {
        // Check max requests
        if self.requests.len() >= self.config.max_requests {
            return Err(ApprovalError::MaxRequestsReached);
        }

        // Check per-agent limit (TLA+: PendingForAgent(agent) < 10)
        let pending = self.pending_per_agent.get(&agent_id).copied().unwrap_or(0);
        if pending >= self.config.max_per_agent {
            return Err(ApprovalError::MaxPerAgentReached);
        }

        // Create request
        let id = ApprovalRequestId(self.next_id);
        self.next_id += 1;

        let request = ApprovalRequest::new(id, agent_id, action, description.into());

        // Notify callback
        self.callback.on_request_submitted(&request);

        // Update tracking
        *self.pending_per_agent.entry(agent_id).or_insert(0) += 1;
        self.requests.insert(id, request);

        Ok(id)
    }

    /// Approve a pending request.
    ///
    /// Implements `ApproveRequest` from TLA+ spec.
    pub fn approve(&mut self, id: ApprovalRequestId) -> ApprovalResult<()> {
        // First check state and extract needed info
        {
            let request = self
                .requests
                .get(&id)
                .ok_or(ApprovalError::RequestNotFound(id))?;

            if request.state != ApprovalState::Pending {
                return Err(ApprovalError::NotPending(id));
            }
        }

        // Now mutate
        let request = self.requests.get_mut(&id).unwrap();
        request.state = ApprovalState::Approved;
        request.completed_at = Some(Instant::now());
        let agent_id = request.agent_id;

        // Clone data needed for audit and callback
        let audit_data = (
            request.id,
            request.agent_id,
            request.action,
            request.description.clone(),
        );

        // Update pending count
        self.decrement_pending(agent_id);

        // Add audit entry
        self.add_audit_entry_from_data(audit_data, ApprovalState::Approved);

        // Notify callback
        if let Some(request) = self.requests.get(&id) {
            self.callback.on_request_approved(request);
        }

        Ok(())
    }

    /// Reject a pending request.
    ///
    /// Implements `RejectRequest` from TLA+ spec.
    pub fn reject(&mut self, id: ApprovalRequestId) -> ApprovalResult<()> {
        // First check state
        {
            let request = self
                .requests
                .get(&id)
                .ok_or(ApprovalError::RequestNotFound(id))?;

            if request.state != ApprovalState::Pending {
                return Err(ApprovalError::NotPending(id));
            }
        }

        // Now mutate
        let request = self.requests.get_mut(&id).unwrap();
        request.state = ApprovalState::Rejected;
        request.completed_at = Some(Instant::now());
        let agent_id = request.agent_id;

        // Clone data needed for audit and callback
        let audit_data = (
            request.id,
            request.agent_id,
            request.action,
            request.description.clone(),
        );

        // Update pending count
        self.decrement_pending(agent_id);

        // Add audit entry
        self.add_audit_entry_from_data(audit_data, ApprovalState::Rejected);

        // Notify callback
        if let Some(request) = self.requests.get(&id) {
            self.callback.on_request_rejected(request);
        }

        Ok(())
    }

    /// Cancel a pending request (agent cancellation).
    ///
    /// Implements `CancelRequest` from TLA+ spec.
    /// Only the requesting agent can cancel its own requests.
    pub fn cancel(&mut self, agent_id: AgentId, id: ApprovalRequestId) -> ApprovalResult<()> {
        // First check state and ownership
        {
            let request = self
                .requests
                .get(&id)
                .ok_or(ApprovalError::RequestNotFound(id))?;

            if request.state != ApprovalState::Pending {
                return Err(ApprovalError::NotPending(id));
            }

            // Only allow cancelling own requests (TLA+: requests[id].agent = agent)
            if request.agent_id != agent_id {
                return Err(ApprovalError::AgentMismatch);
            }
        }

        // Now mutate
        let request = self.requests.get_mut(&id).unwrap();
        request.state = ApprovalState::Cancelled;
        request.completed_at = Some(Instant::now());
        let req_agent_id = request.agent_id;

        // Clone data needed for audit and callback
        let audit_data = (
            request.id,
            request.agent_id,
            request.action,
            request.description.clone(),
        );

        // Update pending count
        self.decrement_pending(req_agent_id);

        // Add audit entry
        self.add_audit_entry_from_data(audit_data, ApprovalState::Cancelled);

        // Notify callback
        if let Some(request) = self.requests.get(&id) {
            self.callback.on_request_cancelled(request);
        }

        Ok(())
    }

    /// Process timeouts for all pending requests.
    ///
    /// Implements `TimeoutRequest` from TLA+ spec.
    /// Returns the number of requests that timed out.
    pub fn process_timeouts(&mut self) -> usize {
        let timeout = self.config.timeout;
        let expired: Vec<_> = self
            .requests
            .values()
            .filter(|r| r.is_expired(timeout))
            .map(|r| r.id)
            .collect();

        let count = expired.len();
        for id in expired {
            self.timeout_request(id);
        }
        count
    }

    /// Timeout a specific request.
    fn timeout_request(&mut self, id: ApprovalRequestId) {
        // Check if request exists and is pending
        let (agent_id, audit_data) = {
            let request = match self.requests.get(&id) {
                Some(r) if r.state == ApprovalState::Pending => r,
                _ => return,
            };
            (
                request.agent_id,
                (
                    request.id,
                    request.agent_id,
                    request.action,
                    request.description.clone(),
                ),
            )
        };

        // Now mutate
        if let Some(request) = self.requests.get_mut(&id) {
            request.state = ApprovalState::TimedOut;
            request.completed_at = Some(Instant::now());
        }

        // Update pending count
        self.decrement_pending(agent_id);

        // Add audit entry
        self.add_audit_entry_from_data(audit_data, ApprovalState::TimedOut);

        // Notify callback
        if let Some(request) = self.requests.get(&id) {
            self.callback.on_request_timeout(request);
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Decrement pending count for an agent.
    fn decrement_pending(&mut self, agent_id: AgentId) {
        if let Some(count) = self.pending_per_agent.get_mut(&agent_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.pending_per_agent.remove(&agent_id);
            }
        }
    }

    /// Add an entry to the audit log from extracted data.
    ///
    /// Used to avoid borrow conflicts - data is extracted before mutation,
    /// then passed here after state changes.
    fn add_audit_entry_from_data(
        &mut self,
        data: (ApprovalRequestId, AgentId, Action, String),
        decision: ApprovalState,
    ) {
        let (request_id, agent_id, action, description) = data;
        let entry = AuditEntry {
            request_id,
            agent_id,
            action,
            decision,
            timestamp: Instant::now(),
            description,
        };

        self.audit_log.push_back(entry);

        // Enforce max audit entries
        while self.audit_log.len() > self.config.max_audit_entries {
            self.audit_log.pop_front();
        }
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Get a request by ID.
    pub fn get(&self, id: ApprovalRequestId) -> Option<&ApprovalRequest> {
        self.requests.get(&id)
    }

    /// Check if a request is approved.
    pub fn is_approved(&self, id: ApprovalRequestId) -> bool {
        self.requests
            .get(&id)
            .map(|r| r.state == ApprovalState::Approved)
            .unwrap_or(false)
    }

    /// Check if a request is pending.
    pub fn is_pending(&self, id: ApprovalRequestId) -> bool {
        self.requests
            .get(&id)
            .map(|r| r.state == ApprovalState::Pending)
            .unwrap_or(false)
    }

    /// Get all pending requests.
    pub fn pending_requests(&self) -> impl Iterator<Item = &ApprovalRequest> {
        self.requests
            .values()
            .filter(|r| r.state == ApprovalState::Pending)
    }

    /// Get all pending requests for a specific agent.
    pub fn pending_for_agent(&self, agent_id: AgentId) -> impl Iterator<Item = &ApprovalRequest> {
        self.requests
            .values()
            .filter(move |r| r.state == ApprovalState::Pending && r.agent_id == agent_id)
    }

    /// Get the number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.requests
            .values()
            .filter(|r| r.state == ApprovalState::Pending)
            .count()
    }

    /// Get the number of pending requests for an agent.
    pub fn pending_count_for_agent(&self, agent_id: AgentId) -> usize {
        self.pending_per_agent.get(&agent_id).copied().unwrap_or(0)
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> impl Iterator<Item = &AuditEntry> {
        self.audit_log.iter()
    }

    /// Get recent audit entries (up to n).
    pub fn recent_audit_entries(&self, n: usize) -> impl Iterator<Item = &AuditEntry> {
        self.audit_log.iter().rev().take(n)
    }

    /// Get total request count (including completed).
    pub fn total_request_count(&self) -> usize {
        self.requests.len()
    }

    /// Clean up old completed requests.
    ///
    /// Removes requests that completed more than `max_age` ago.
    pub fn cleanup_old_requests(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.requests.retain(|_, request| {
            if let Some(completed_at) = request.completed_at {
                now.duration_since(completed_at) < max_age
            } else {
                true // Keep pending requests
            }
        });
    }

    // =========================================================================
    // Invariant Verification (for testing)
    // =========================================================================

    /// Verify INV-APPROVAL-1: No double decisions (structural guarantee).
    #[cfg(test)]
    fn verify_no_double_decision(&self) -> bool {
        // This is structurally guaranteed by the enum - a request
        // cannot be both Approved and Rejected.
        true
    }

    /// Verify INV-APPROVAL-2: All completed requests have audit entries.
    #[cfg(test)]
    fn verify_completed_have_audit(&self) -> bool {
        for request in self.requests.values() {
            if request.state.is_terminal() {
                let has_audit = self.audit_log.iter().any(|e| e.request_id == request.id);
                if !has_audit {
                    return false;
                }
            }
        }
        true
    }

    /// Verify INV-APPROVAL-3: Pending requests have no completion time.
    #[cfg(test)]
    fn verify_pending_not_completed(&self) -> bool {
        self.requests
            .values()
            .filter(|r| r.state == ApprovalState::Pending)
            .all(|r| r.completed_at.is_none())
    }

    /// Verify INV-APPROVAL-4: Completed requests have valid completion time.
    #[cfg(test)]
    fn verify_completed_have_time(&self) -> bool {
        self.requests
            .values()
            .filter(|r| r.state.is_terminal())
            .all(|r| r.completed_at.is_some())
    }

    /// Verify INV-APPROVAL-5: Request IDs are unique and sequential.
    #[cfg(test)]
    fn verify_ids_sequential(&self) -> bool {
        // IDs are unique by HashMap key
        // Sequential: all IDs < next_id
        self.requests.keys().all(|id| id.0 < self.next_id)
    }

    /// Verify all safety invariants.
    #[cfg(test)]
    pub fn verify_invariants(&self) -> bool {
        self.verify_no_double_decision()
            && self.verify_completed_have_audit()
            && self.verify_pending_not_completed()
            && self.verify_completed_have_time()
            && self.verify_ids_sequential()
    }
}

impl fmt::Debug for ApprovalManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalManager")
            .field("total_requests", &self.requests.len())
            .field("pending", &self.pending_count())
            .field("audit_entries", &self.audit_log.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_manager() -> ApprovalManager {
        ApprovalManager::new(ApprovalConfig {
            max_requests: 10,
            max_per_agent: 3,
            timeout: Duration::from_millis(100),
            max_audit_entries: 50,
        })
    }

    #[test]
    fn test_submit_request() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "echo hello")
            .unwrap();

        let request = manager.get(id).unwrap();
        assert_eq!(request.state, ApprovalState::Pending);
        assert_eq!(request.agent_id, agent_id);
        assert_eq!(request.action, Action::Shell);
        assert!(manager.verify_invariants());
    }

    #[test]
    fn test_approve_request() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "echo hello")
            .unwrap();

        manager.approve(id).unwrap();

        let request = manager.get(id).unwrap();
        assert_eq!(request.state, ApprovalState::Approved);
        assert!(request.completed_at.is_some());
        assert!(manager.is_approved(id));
        assert!(!manager.is_pending(id));
        assert!(manager.verify_invariants());

        // Audit log should have entry
        assert_eq!(manager.audit_log.len(), 1);
        assert_eq!(manager.audit_log[0].decision, ApprovalState::Approved);
    }

    #[test]
    fn test_reject_request() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "rm -rf /")
            .unwrap();

        manager.reject(id).unwrap();

        let request = manager.get(id).unwrap();
        assert_eq!(request.state, ApprovalState::Rejected);
        assert!(!manager.is_approved(id));
        assert!(manager.verify_invariants());
    }

    #[test]
    fn test_cancel_request() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);
        let other_agent = AgentId(2);

        let id = manager
            .submit_request(agent_id, Action::Shell, "echo hello")
            .unwrap();

        // Other agent cannot cancel
        assert!(matches!(
            manager.cancel(other_agent, id),
            Err(ApprovalError::AgentMismatch)
        ));

        // Owner can cancel
        manager.cancel(agent_id, id).unwrap();

        let request = manager.get(id).unwrap();
        assert_eq!(request.state, ApprovalState::Cancelled);
        assert!(manager.verify_invariants());
    }

    #[test]
    fn test_timeout() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "slow command")
            .unwrap();

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        let timed_out = manager.process_timeouts();
        assert_eq!(timed_out, 1);

        let request = manager.get(id).unwrap();
        assert_eq!(request.state, ApprovalState::TimedOut);
        assert!(manager.verify_invariants());
    }

    #[test]
    fn test_max_requests() {
        let mut manager = create_manager();

        // Fill up requests
        for i in 0..10_u64 {
            manager
                .submit_request(AgentId(i), Action::Shell, format!("cmd {}", i))
                .unwrap();
        }

        // Next should fail
        assert!(matches!(
            manager.submit_request(AgentId(99), Action::Shell, "overflow"),
            Err(ApprovalError::MaxRequestsReached)
        ));
    }

    #[test]
    fn test_max_per_agent() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        // Fill up agent's quota (max 3)
        for i in 0..3 {
            manager
                .submit_request(agent_id, Action::Shell, format!("cmd {}", i))
                .unwrap();
        }

        // Next should fail
        assert!(matches!(
            manager.submit_request(agent_id, Action::Shell, "overflow"),
            Err(ApprovalError::MaxPerAgentReached)
        ));

        // Different agent can still submit
        manager
            .submit_request(AgentId(2), Action::Shell, "other agent")
            .unwrap();
    }

    #[test]
    fn test_double_decision_prevented() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "echo hello")
            .unwrap();

        manager.approve(id).unwrap();

        // Cannot reject after approve
        assert!(matches!(
            manager.reject(id),
            Err(ApprovalError::NotPending(_))
        ));

        // Cannot approve again
        assert!(matches!(
            manager.approve(id),
            Err(ApprovalError::NotPending(_))
        ));
    }

    #[test]
    fn test_pending_count() {
        let mut manager = create_manager();
        let agent1 = AgentId(1);
        let agent2 = AgentId(2);

        manager
            .submit_request(agent1, Action::Shell, "cmd1")
            .unwrap();
        manager
            .submit_request(agent1, Action::Shell, "cmd2")
            .unwrap();
        manager
            .submit_request(agent2, Action::Shell, "cmd3")
            .unwrap();

        assert_eq!(manager.pending_count(), 3);
        assert_eq!(manager.pending_count_for_agent(agent1), 2);
        assert_eq!(manager.pending_count_for_agent(agent2), 1);
    }

    #[test]
    fn test_pending_count_decrements() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id1 = manager
            .submit_request(agent_id, Action::Shell, "cmd1")
            .unwrap();
        let id2 = manager
            .submit_request(agent_id, Action::Shell, "cmd2")
            .unwrap();

        assert_eq!(manager.pending_count_for_agent(agent_id), 2);

        manager.approve(id1).unwrap();
        assert_eq!(manager.pending_count_for_agent(agent_id), 1);

        manager.reject(id2).unwrap();
        assert_eq!(manager.pending_count_for_agent(agent_id), 0);
    }

    #[test]
    fn test_audit_log_bounded() {
        let mut manager = ApprovalManager::new(ApprovalConfig {
            max_requests: 100,
            max_per_agent: 100,
            timeout: Duration::from_secs(300),
            max_audit_entries: 5,
        });

        // Create and approve more than max_audit_entries
        for i in 0..10_u64 {
            let id = manager
                .submit_request(AgentId(i), Action::Shell, format!("cmd {}", i))
                .unwrap();
            manager.approve(id).unwrap();
        }

        // Audit log should be bounded
        assert_eq!(manager.audit_log.len(), 5);
    }

    #[test]
    fn test_action_risk_levels() {
        assert!(Action::Admin.risk_level() > Action::Network.risk_level());
        assert!(Action::Container.risk_level() > Action::Shell.risk_level());
    }

    #[test]
    fn test_cleanup_old_requests() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        let id = manager
            .submit_request(agent_id, Action::Shell, "old command")
            .unwrap();
        manager.approve(id).unwrap();

        // Wait a bit
        std::thread::sleep(Duration::from_millis(50));

        // Cleanup with short max_age
        manager.cleanup_old_requests(Duration::from_millis(10));

        // Request should be removed
        assert!(manager.get(id).is_none());
    }

    #[test]
    fn test_invariants_maintained_through_lifecycle() {
        let mut manager = create_manager();
        let agent_id = AgentId(1);

        // Submit
        let id1 = manager
            .submit_request(agent_id, Action::Shell, "cmd1")
            .unwrap();
        let id2 = manager
            .submit_request(agent_id, Action::FileWrite, "cmd2")
            .unwrap();
        let id3 = manager
            .submit_request(agent_id, Action::Network, "cmd3")
            .unwrap();
        assert!(manager.verify_invariants());

        // Approve one
        manager.approve(id1).unwrap();
        assert!(manager.verify_invariants());

        // Reject one
        manager.reject(id2).unwrap();
        assert!(manager.verify_invariants());

        // Cancel one
        manager.cancel(agent_id, id3).unwrap();
        assert!(manager.verify_invariants());
    }
}
