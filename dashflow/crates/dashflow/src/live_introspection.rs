// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Live Execution Introspection - Runtime State for AI Agents
//!
//! This module provides live execution introspection for DashFlow graphs.
//! Unlike platform or app introspection which describe static capabilities,
//! live introspection tracks runtime execution state in real-time.
//!
//! ## Three-Level Introspection Model
//!
//! DashFlow provides three levels of introspection:
//!
//! 1. **Platform Introspection** - DashFlow framework capabilities (shared by all apps)
//! 2. **App Introspection** - Application-specific configuration (per compiled graph)
//! 3. **Live Introspection** (this module) - Runtime execution state (per execution instance)
//!
//! ## Key Concepts
//!
//! - **Execution**: A single run of a compiled graph with a unique ID
//! - **ExecutionTracker**: Manages all active executions for a graph
//! - **ExecutionSummary**: Brief overview of an execution's status
//! - **ExecutionState**: Detailed state including current node, state values, history
//! - **ExecutionStep**: A record of a single node execution
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::live_introspection::{ExecutionTracker, ExecutionStatus};
//!
//! // Create a tracker for managing executions
//! let tracker = ExecutionTracker::new();
//!
//! // Start tracking an execution
//! let exec_id = tracker.start_execution("my_graph");
//!
//! // Record node execution
//! tracker.enter_node(&exec_id, "process_input");
//! // ... node executes ...
//! tracker.exit_node(&exec_id, "process_input", serde_json::json!({"result": "ok"}));
//!
//! // Query execution state
//! if let Some(state) = tracker.get_execution(&exec_id) {
//!     println!("Current node: {}", state.current_node);
//!     println!("Status: {:?}", state.status);
//! }
//!
//! // List all active executions
//! for summary in tracker.active_executions() {
//!     println!("{}: {} at {}", summary.execution_id, summary.status, summary.current_node);
//! }
//! ```
//!
//! ## Resource Safety
//!
//! The tracker implements resource bounds to prevent memory exhaustion:
//! - Maximum concurrent executions (default: 1000)
//! - Maximum history steps per execution (default: 100)
//! - Auto-cleanup of completed executions after TTL (configurable)

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

// Import centralized constants (M-147)
use crate::constants::{
    DEFAULT_COMPLETED_TTL_SECS as COMPLETED_TTL_SECS,
    DEFAULT_MAX_HISTORY_STEPS as MAX_HISTORY_STEPS,
    DEFAULT_QUEUE_CAPACITY,
    DEFAULT_WS_CHANNEL_CAPACITY,
};

// ============================================================================
// Configuration
// ============================================================================

/// Default maximum number of concurrent executions to track.
///
/// Uses `DEFAULT_QUEUE_CAPACITY` (1000) from centralized constants.
pub const DEFAULT_MAX_CONCURRENT_EXECUTIONS: usize = DEFAULT_QUEUE_CAPACITY;

/// Default maximum number of history steps per execution.
///
/// Uses `DEFAULT_MAX_HISTORY_STEPS` (100) from centralized constants.
pub const DEFAULT_MAX_HISTORY_STEPS: usize = MAX_HISTORY_STEPS;

/// Default TTL for completed executions before cleanup (5 minutes).
///
/// Uses `DEFAULT_COMPLETED_TTL_SECS` (300) from centralized constants.
pub const DEFAULT_COMPLETED_TTL_SECS: u64 = COMPLETED_TTL_SECS;

/// Configuration for the execution tracker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrackerConfig {
    /// Maximum number of concurrent executions to track.
    pub max_concurrent_executions: usize,
    /// Maximum number of history steps per execution.
    pub max_history_steps: usize,
    /// TTL for completed executions in seconds (0 = no auto-cleanup).
    pub completed_ttl_secs: u64,
}

impl Default for ExecutionTrackerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: DEFAULT_MAX_CONCURRENT_EXECUTIONS,
            max_history_steps: DEFAULT_MAX_HISTORY_STEPS,
            completed_ttl_secs: DEFAULT_COMPLETED_TTL_SECS,
        }
    }
}

impl ExecutionTrackerConfig {
    /// Create a new configuration with custom values.
    #[must_use]
    pub fn new(
        max_concurrent_executions: usize,
        max_history_steps: usize,
        completed_ttl_secs: u64,
    ) -> Self {
        Self {
            max_concurrent_executions,
            max_history_steps,
            completed_ttl_secs,
        }
    }

    /// Create configuration for high-throughput scenarios.
    #[must_use]
    pub fn high_throughput() -> Self {
        Self {
            max_concurrent_executions: 10000,
            max_history_steps: 50,
            completed_ttl_secs: 60,
        }
    }

    /// Create configuration for debugging with longer history retention.
    #[must_use]
    pub fn debug() -> Self {
        Self {
            max_concurrent_executions: 100,
            max_history_steps: 1000,
            completed_ttl_secs: 3600,
        }
    }
}

// ============================================================================
// Execution Status
// ============================================================================

/// Status of a live execution.
///
/// This is distinct from `graph_registry::ExecutionStatus` which tracks
/// historical execution status. `LiveExecutionStatus` includes additional
/// states for paused/waiting executions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LiveExecutionStatus {
    /// Execution is actively running.
    Running,
    /// Execution is paused (e.g., waiting for human approval).
    Paused,
    /// Execution is waiting for external input.
    WaitingForInput,
    /// Execution completed successfully.
    Completed,
    /// Execution failed with an error.
    Failed,
    /// Execution was cancelled.
    Cancelled,
}

impl LiveExecutionStatus {
    /// Check if the execution is still active (running or paused).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused | Self::WaitingForInput)
    }

    /// Check if the execution is in a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for LiveExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::WaitingForInput => write!(f, "waiting_for_input"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

// ============================================================================
// Step Outcome
// ============================================================================

/// Outcome of a single execution step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step failed with an error.
    Error(String),
    /// Step was skipped (e.g., conditional routing bypassed it).
    Skipped,
    /// Step is currently in progress.
    InProgress,
}

impl StepOutcome {
    /// Check if the step was successful.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if the step failed.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

// ============================================================================
// Execution Step
// ============================================================================

/// A single step in execution history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Step number (1-indexed).
    pub step_number: u32,
    /// Name of the node executed.
    pub node_name: String,
    /// When the step started (ISO 8601).
    pub started_at: String,
    /// When the step completed (ISO 8601), if completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Duration in milliseconds, if completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// State before this step (if tracking enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_before: Option<serde_json::Value>,
    /// State after this step (if tracking enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_after: Option<serde_json::Value>,
    /// Outcome of this step.
    pub outcome: StepOutcome,
}

impl ExecutionStep {
    /// Create a new execution step that is in progress.
    #[must_use]
    pub fn new(step_number: u32, node_name: impl Into<String>) -> Self {
        Self {
            step_number,
            node_name: node_name.into(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            duration_ms: None,
            state_before: None,
            state_after: None,
            outcome: StepOutcome::InProgress,
        }
    }

    /// Create with state snapshot before execution.
    #[must_use]
    pub fn with_state_before(mut self, state: serde_json::Value) -> Self {
        self.state_before = Some(state);
        self
    }

    /// Mark the step as completed with success.
    pub fn complete_success(&mut self, state_after: Option<serde_json::Value>) {
        self.completed_at = Some(Utc::now().to_rfc3339());
        self.outcome = StepOutcome::Success;
        self.state_after = state_after;
        self.calculate_duration();
    }

    /// Mark the step as failed with an error.
    pub fn complete_error(&mut self, error: impl Into<String>) {
        self.completed_at = Some(Utc::now().to_rfc3339());
        self.outcome = StepOutcome::Error(error.into());
        self.calculate_duration();
    }

    /// Calculate duration from started_at to completed_at.
    fn calculate_duration(&mut self) {
        if let (Ok(start), Some(Ok(end))) = (
            DateTime::parse_from_rfc3339(&self.started_at),
            self.completed_at
                .as_ref()
                .map(|s| DateTime::parse_from_rfc3339(s)),
        ) {
            let duration = end.signed_duration_since(start);
            self.duration_ms = Some(duration.num_milliseconds().max(0) as u64);
        }
    }
}

// ============================================================================
// Execution Metrics
// ============================================================================

/// Performance metrics for an execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LiveExecutionMetrics {
    /// Total execution time so far in milliseconds.
    pub total_duration_ms: u64,
    /// Number of nodes executed.
    pub nodes_executed: u32,
    /// Number of successful node executions.
    pub nodes_succeeded: u32,
    /// Number of failed node executions.
    pub nodes_failed: u32,
    /// Average node execution time in milliseconds.
    pub avg_node_duration_ms: Option<f64>,
    /// Slowest node name and duration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slowest_node: Option<(String, u64)>,
    /// Current iteration count.
    pub iteration: u32,
}

impl LiveExecutionMetrics {
    /// Create new empty metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update metrics after a step completes.
    pub fn record_step(&mut self, step: &ExecutionStep) {
        self.nodes_executed += 1;

        match &step.outcome {
            StepOutcome::Success => self.nodes_succeeded += 1,
            StepOutcome::Error(_) => self.nodes_failed += 1,
            _ => {}
        }

        if let Some(duration) = step.duration_ms {
            // Update slowest node
            if self
                .slowest_node
                .as_ref()
                .map_or(true, |(_, d)| duration > *d)
            {
                self.slowest_node = Some((step.node_name.clone(), duration));
            }

            // Update average
            let total = self.avg_node_duration_ms.unwrap_or(0.0) * (self.nodes_executed - 1) as f64;
            self.avg_node_duration_ms =
                Some((total + duration as f64) / self.nodes_executed as f64);
        }
    }

    /// Increment iteration counter.
    pub fn increment_iteration(&mut self) {
        self.iteration += 1;
    }
}

// ============================================================================
// Checkpoint Status
// ============================================================================

/// Checkpoint status for an execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointStatusInfo {
    /// Whether checkpointing is enabled.
    pub enabled: bool,
    /// Thread ID if checkpointing is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Last checkpoint timestamp (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_at: Option<String>,
    /// Number of checkpoints created.
    pub checkpoint_count: u32,
    /// Total checkpoint size in bytes.
    pub total_size_bytes: u64,
}

impl CheckpointStatusInfo {
    /// Create checkpoint status for an execution with checkpointing disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create checkpoint status for an execution with checkpointing enabled.
    #[must_use]
    pub fn enabled(thread_id: impl Into<String>) -> Self {
        Self {
            enabled: true,
            thread_id: Some(thread_id.into()),
            last_checkpoint_at: None,
            checkpoint_count: 0,
            total_size_bytes: 0,
        }
    }

    /// Record a checkpoint being created.
    pub fn record_checkpoint(&mut self, size_bytes: u64) {
        self.checkpoint_count += 1;
        self.total_size_bytes += size_bytes;
        self.last_checkpoint_at = Some(Utc::now().to_rfc3339());
    }
}

// ============================================================================
// Execution Events (Real-time Streaming)
// ============================================================================

/// Default broadcast channel capacity for execution events.
///
/// Uses `DEFAULT_WS_CHANNEL_CAPACITY` (256) from centralized constants.
/// This matches WebSocket channel capacity since both are used for event streaming.
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = DEFAULT_WS_CHANNEL_CAPACITY;

/// Events emitted during graph execution for real-time streaming.
///
/// These events can be subscribed to via WebSocket/SSE endpoints for live
/// monitoring of execution progress.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    /// Execution has started.
    ExecutionStarted {
        /// Execution ID.
        execution_id: String,
        /// Graph name.
        graph_name: String,
        /// Entry point node.
        entry_point: String,
        /// When the execution started (ISO 8601).
        timestamp: String,
    },

    /// Entered a node.
    NodeEntered {
        /// Execution ID.
        execution_id: String,
        /// Name of the node entered.
        node: String,
        /// Previous node (if any).
        previous_node: Option<String>,
        /// When the node was entered (ISO 8601).
        timestamp: String,
    },

    /// Exited a node.
    NodeExited {
        /// Execution ID.
        execution_id: String,
        /// Name of the node exited.
        node: String,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Outcome of the step.
        outcome: StepOutcome,
        /// When the node was exited (ISO 8601).
        timestamp: String,
    },

    /// State has changed.
    StateChanged {
        /// Execution ID.
        execution_id: String,
        /// State diff or new state snapshot.
        state: serde_json::Value,
        /// When the state changed (ISO 8601).
        timestamp: String,
    },

    /// Checkpoint was created.
    CheckpointCreated {
        /// Execution ID.
        execution_id: String,
        /// Checkpoint number.
        checkpoint_number: u32,
        /// Size in bytes.
        size_bytes: u64,
        /// When the checkpoint was created (ISO 8601).
        timestamp: String,
    },

    /// Iteration completed (for graphs with loops).
    IterationCompleted {
        /// Execution ID.
        execution_id: String,
        /// Iteration number.
        iteration: u32,
        /// When the iteration completed (ISO 8601).
        timestamp: String,
    },

    /// Execution status changed.
    StatusChanged {
        /// Execution ID.
        execution_id: String,
        /// Previous status.
        previous_status: LiveExecutionStatus,
        /// New status.
        new_status: LiveExecutionStatus,
        /// When the status changed (ISO 8601).
        timestamp: String,
    },

    /// Execution completed successfully.
    ExecutionCompleted {
        /// Execution ID.
        execution_id: String,
        /// Final state.
        final_state: serde_json::Value,
        /// Total duration in milliseconds.
        duration_ms: u64,
        /// When the execution completed (ISO 8601).
        timestamp: String,
    },

    /// Execution failed with an error.
    ExecutionFailed {
        /// Execution ID.
        execution_id: String,
        /// Error message.
        error: String,
        /// Node where failure occurred (if known).
        failed_node: Option<String>,
        /// When the execution failed (ISO 8601).
        timestamp: String,
    },

    /// Execution was cancelled.
    ExecutionCancelled {
        /// Execution ID.
        execution_id: String,
        /// When the execution was cancelled (ISO 8601).
        timestamp: String,
    },
}

impl ExecutionEvent {
    /// Get the execution ID for this event.
    #[must_use]
    pub fn execution_id(&self) -> &str {
        match self {
            Self::ExecutionStarted { execution_id, .. } => execution_id,
            Self::NodeEntered { execution_id, .. } => execution_id,
            Self::NodeExited { execution_id, .. } => execution_id,
            Self::StateChanged { execution_id, .. } => execution_id,
            Self::CheckpointCreated { execution_id, .. } => execution_id,
            Self::IterationCompleted { execution_id, .. } => execution_id,
            Self::StatusChanged { execution_id, .. } => execution_id,
            Self::ExecutionCompleted { execution_id, .. } => execution_id,
            Self::ExecutionFailed { execution_id, .. } => execution_id,
            Self::ExecutionCancelled { execution_id, .. } => execution_id,
        }
    }

    /// Get the timestamp for this event.
    #[must_use]
    pub fn timestamp(&self) -> &str {
        match self {
            Self::ExecutionStarted { timestamp, .. } => timestamp,
            Self::NodeEntered { timestamp, .. } => timestamp,
            Self::NodeExited { timestamp, .. } => timestamp,
            Self::StateChanged { timestamp, .. } => timestamp,
            Self::CheckpointCreated { timestamp, .. } => timestamp,
            Self::IterationCompleted { timestamp, .. } => timestamp,
            Self::StatusChanged { timestamp, .. } => timestamp,
            Self::ExecutionCompleted { timestamp, .. } => timestamp,
            Self::ExecutionFailed { timestamp, .. } => timestamp,
            Self::ExecutionCancelled { timestamp, .. } => timestamp,
        }
    }

    /// Get the event type name.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ExecutionStarted { .. } => "execution_started",
            Self::NodeEntered { .. } => "node_entered",
            Self::NodeExited { .. } => "node_exited",
            Self::StateChanged { .. } => "state_changed",
            Self::CheckpointCreated { .. } => "checkpoint_created",
            Self::IterationCompleted { .. } => "iteration_completed",
            Self::StatusChanged { .. } => "status_changed",
            Self::ExecutionCompleted { .. } => "execution_completed",
            Self::ExecutionFailed { .. } => "execution_failed",
            Self::ExecutionCancelled { .. } => "execution_cancelled",
        }
    }
}

/// A stream of execution events for real-time monitoring.
///
/// Wraps a broadcast receiver that yields [`ExecutionEvent`]s.
/// The stream can be used with WebSocket/SSE handlers for live updates.
#[derive(Debug)]
pub struct ExecutionEventStream {
    /// Receiver for execution events.
    /// Public for use with `tokio_stream::wrappers::BroadcastStream`.
    pub receiver: broadcast::Receiver<ExecutionEvent>,
    /// Optional filter for specific execution ID.
    execution_filter: Option<String>,
}

impl ExecutionEventStream {
    /// Create a new event stream from a broadcast receiver.
    #[must_use]
    pub fn new(receiver: broadcast::Receiver<ExecutionEvent>) -> Self {
        Self {
            receiver,
            execution_filter: None,
        }
    }

    /// Create an event stream filtered to a specific execution.
    #[must_use]
    pub fn for_execution(
        receiver: broadcast::Receiver<ExecutionEvent>,
        execution_id: impl Into<String>,
    ) -> Self {
        Self {
            receiver,
            execution_filter: Some(execution_id.into()),
        }
    }

    /// Receive the next event (blocking).
    ///
    /// Returns `None` if the channel is closed or lagged.
    pub async fn recv(&mut self) -> Option<ExecutionEvent> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    // Apply filter if set
                    if let Some(ref filter) = self.execution_filter {
                        if event.execution_id() != filter {
                            continue;
                        }
                    }
                    return Some(event);
                }
                Err(broadcast::error::RecvError::Closed) => return None,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Skip lagged messages and continue
                    continue;
                }
            }
        }
    }

    /// Try to receive the next event without blocking.
    ///
    /// Returns `None` if no event is available.
    pub fn try_recv(&mut self) -> Option<ExecutionEvent> {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    // Apply filter if set
                    if let Some(ref filter) = self.execution_filter {
                        if event.execution_id() != filter {
                            continue;
                        }
                    }
                    return Some(event);
                }
                Err(_) => return None,
            }
        }
    }

    /// Check if there's a filter set.
    #[must_use]
    pub fn has_filter(&self) -> bool {
        self.execution_filter.is_some()
    }

    /// Get the execution filter if set.
    #[must_use]
    pub fn execution_filter(&self) -> Option<&str> {
        self.execution_filter.as_deref()
    }
}

// ============================================================================
// Execution Summary
// ============================================================================

/// Summary of an active execution (lightweight for listing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Unique execution identifier.
    pub execution_id: String,
    /// Name of the graph being executed.
    pub graph_name: String,
    /// When execution started (ISO 8601).
    pub started_at: String,
    /// Current node being executed.
    pub current_node: String,
    /// Current iteration count.
    pub iteration: u32,
    /// Current status.
    pub status: LiveExecutionStatus,
}

// ============================================================================
// Execution State
// ============================================================================

/// Detailed execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Unique execution identifier.
    pub execution_id: String,
    /// Name of the graph being executed.
    pub graph_name: String,
    /// When execution started (ISO 8601).
    pub started_at: String,
    /// Current node being executed.
    pub current_node: String,
    /// Previous node (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_node: Option<String>,
    /// Current iteration count.
    pub iteration: u32,
    /// Total number of nodes visited.
    pub total_nodes_visited: u32,
    /// Current state values (JSON snapshot).
    pub state: serde_json::Value,
    /// Performance metrics.
    pub metrics: LiveExecutionMetrics,
    /// Checkpoint status.
    pub checkpoint: CheckpointStatusInfo,
    /// Current status.
    pub status: LiveExecutionStatus,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ExecutionState {
    /// Create a new execution state.
    #[must_use]
    pub fn new(
        execution_id: impl Into<String>,
        graph_name: impl Into<String>,
        entry_point: impl Into<String>,
    ) -> Self {
        Self {
            execution_id: execution_id.into(),
            graph_name: graph_name.into(),
            started_at: Utc::now().to_rfc3339(),
            current_node: entry_point.into(),
            previous_node: None,
            iteration: 0,
            total_nodes_visited: 0,
            state: serde_json::Value::Null,
            metrics: LiveExecutionMetrics::new(),
            checkpoint: CheckpointStatusInfo::disabled(),
            status: LiveExecutionStatus::Running,
            error: None,
        }
    }

    /// Set the current state snapshot.
    pub fn set_state(&mut self, state: serde_json::Value) {
        self.state = state;
    }

    /// Move to a new node.
    pub fn enter_node(&mut self, node_name: impl Into<String>) {
        self.previous_node = Some(std::mem::take(&mut self.current_node));
        self.current_node = node_name.into();
        self.total_nodes_visited += 1;
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.status = LiveExecutionStatus::Completed;
    }

    /// Mark as failed with error.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = LiveExecutionStatus::Failed;
        self.error = Some(error.into());
    }

    /// Mark as cancelled.
    pub fn cancel(&mut self) {
        self.status = LiveExecutionStatus::Cancelled;
    }

    /// Pause the execution.
    pub fn pause(&mut self) {
        self.status = LiveExecutionStatus::Paused;
    }

    /// Resume the execution.
    pub fn resume(&mut self) {
        self.status = LiveExecutionStatus::Running;
    }

    /// Set to waiting for input.
    pub fn wait_for_input(&mut self) {
        self.status = LiveExecutionStatus::WaitingForInput;
    }

    /// Create a summary view.
    #[must_use]
    pub fn to_summary(&self) -> ExecutionSummary {
        ExecutionSummary {
            execution_id: self.execution_id.clone(),
            graph_name: self.graph_name.clone(),
            started_at: self.started_at.clone(),
            current_node: self.current_node.clone(),
            iteration: self.iteration,
            status: self.status,
        }
    }

    /// Enable checkpointing for this execution.
    pub fn enable_checkpointing(&mut self, thread_id: impl Into<String>) {
        self.checkpoint = CheckpointStatusInfo::enabled(thread_id);
    }
}

// ============================================================================
// Internal Execution Record
// ============================================================================

/// Internal record for tracking an execution (not serialized directly).
#[derive(Debug)]
struct ExecutionRecord {
    /// The execution state.
    state: ExecutionState,
    /// Execution history (bounded by config).
    history: Vec<ExecutionStep>,
    /// When this record was created (for TTL).
    created_at: Instant,
    /// Current step being executed (if any).
    current_step: Option<ExecutionStep>,
    /// Config for this execution.
    max_history_steps: usize,
}

impl ExecutionRecord {
    fn new(state: ExecutionState, max_history_steps: usize) -> Self {
        Self {
            state,
            history: Vec::new(),
            created_at: Instant::now(),
            current_step: None,
            max_history_steps,
        }
    }

    fn add_to_history(&mut self, step: ExecutionStep) {
        if self.history.len() >= self.max_history_steps {
            // Remove oldest step to make room
            self.history.remove(0);
        }
        self.state.metrics.record_step(&step);
        self.history.push(step);
    }
}

// ============================================================================
// Execution Tracker
// ============================================================================

/// Tracks active and recent executions for a graph.
///
/// The tracker is thread-safe and can be shared across async tasks.
/// It implements resource bounds to prevent memory exhaustion.
///
/// ## Event Streaming
///
/// The tracker emits events via a broadcast channel for real-time monitoring.
/// Subscribe to events using [`subscribe()`](Self::subscribe) or
/// [`subscribe_to_execution()`](Self::subscribe_to_execution).
#[derive(Debug)]
pub struct ExecutionTracker {
    /// Configuration.
    config: ExecutionTrackerConfig,
    /// Active and recent executions.
    executions: Arc<RwLock<HashMap<String, ExecutionRecord>>>,
    /// Event broadcast sender.
    event_sender: broadcast::Sender<ExecutionEvent>,
}

impl Clone for ExecutionTracker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            executions: Arc::clone(&self.executions),
            event_sender: self.event_sender.clone(),
        }
    }
}

impl Default for ExecutionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionTracker {
    /// Create a new execution tracker with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ExecutionTrackerConfig::default())
    }

    /// Create a new execution tracker with custom configuration.
    #[must_use]
    pub fn with_config(config: ExecutionTrackerConfig) -> Self {
        Self::with_config_and_capacity(config, DEFAULT_EVENT_CHANNEL_CAPACITY)
    }

    /// Create a new execution tracker with custom configuration and event channel capacity.
    #[must_use]
    pub fn with_config_and_capacity(
        config: ExecutionTrackerConfig,
        channel_capacity: usize,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(channel_capacity);
        Self {
            config,
            executions: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
        }
    }

    /// Subscribe to all execution events.
    ///
    /// Returns an [`ExecutionEventStream`] that yields all events from all executions.
    #[must_use]
    pub fn subscribe(&self) -> ExecutionEventStream {
        ExecutionEventStream::new(self.event_sender.subscribe())
    }

    /// Subscribe to events for a specific execution.
    ///
    /// Returns an [`ExecutionEventStream`] filtered to the given execution ID.
    #[must_use]
    pub fn subscribe_to_execution(&self, execution_id: impl Into<String>) -> ExecutionEventStream {
        ExecutionEventStream::for_execution(self.event_sender.subscribe(), execution_id)
    }

    /// Get the number of active subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.event_sender.receiver_count()
    }

    /// Emit an event to all subscribers.
    ///
    /// Returns the number of receivers that received the event.
    fn emit_event(&self, event: ExecutionEvent) -> usize {
        // send() returns Err if there are no receivers, which is fine
        self.event_sender.send(event).unwrap_or(0)
    }

    /// Start tracking a new execution.
    ///
    /// Returns the execution ID, or None if at capacity.
    /// Emits an [`ExecutionEvent::ExecutionStarted`] event.
    pub fn start_execution(&self, graph_name: impl Into<String>) -> Option<String> {
        self.start_execution_with_entry(graph_name, "__start__")
    }

    /// Start tracking a new execution with a custom entry point.
    ///
    /// Returns the execution ID, or None if at capacity.
    /// Emits an [`ExecutionEvent::ExecutionStarted`] event.
    pub fn start_execution_with_entry(
        &self,
        graph_name: impl Into<String>,
        entry_point: impl Into<String>,
    ) -> Option<String> {
        let graph_name = graph_name.into();
        let entry_point = entry_point.into();
        let mut executions = self.executions.write();

        // Cleanup expired entries first
        self.cleanup_expired_internal(&mut executions);

        // Check capacity
        let active_count = executions
            .values()
            .filter(|r| r.state.status.is_active())
            .count();

        if active_count >= self.config.max_concurrent_executions {
            return None;
        }

        let execution_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();
        let state = ExecutionState::new(&execution_id, &graph_name, &entry_point);
        let record = ExecutionRecord::new(state, self.config.max_history_steps);

        executions.insert(execution_id.clone(), record);

        // Emit event after releasing the lock
        drop(executions);
        self.emit_event(ExecutionEvent::ExecutionStarted {
            execution_id: execution_id.clone(),
            graph_name,
            entry_point,
            timestamp,
        });

        Some(execution_id)
    }

    /// Start tracking an execution with a known ID.
    ///
    /// Returns true if started, false if at capacity or ID already exists.
    /// Emits an [`ExecutionEvent::ExecutionStarted`] event.
    pub fn start_execution_with_id(
        &self,
        execution_id: impl Into<String>,
        graph_name: impl Into<String>,
        entry_point: impl Into<String>,
    ) -> bool {
        let execution_id = execution_id.into();
        let graph_name = graph_name.into();
        let entry_point = entry_point.into();
        let mut executions = self.executions.write();

        // Cleanup expired entries first
        self.cleanup_expired_internal(&mut executions);

        // Check if ID already exists
        if executions.contains_key(&execution_id) {
            return false;
        }

        // Check capacity
        let active_count = executions
            .values()
            .filter(|r| r.state.status.is_active())
            .count();

        if active_count >= self.config.max_concurrent_executions {
            return false;
        }

        let timestamp = Utc::now().to_rfc3339();
        let state = ExecutionState::new(&execution_id, &graph_name, &entry_point);
        let record = ExecutionRecord::new(state, self.config.max_history_steps);

        executions.insert(execution_id.clone(), record);

        // Emit event after releasing the lock
        drop(executions);
        self.emit_event(ExecutionEvent::ExecutionStarted {
            execution_id,
            graph_name,
            entry_point,
            timestamp,
        });

        true
    }

    /// Record entering a node.
    /// Emits an [`ExecutionEvent::NodeEntered`] event.
    pub fn enter_node(&self, execution_id: &str, node_name: impl Into<String>) {
        self.enter_node_with_state(execution_id, node_name, None);
    }

    /// Record entering a node with state snapshot.
    /// Emits an [`ExecutionEvent::NodeEntered`] event.
    pub fn enter_node_with_state(
        &self,
        execution_id: &str,
        node_name: impl Into<String>,
        state_before: Option<serde_json::Value>,
    ) {
        let node_name = node_name.into();
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_node = record.state.previous_node.clone();
            record.state.enter_node(&node_name);

            let step_number = record.state.total_nodes_visited;
            let mut step = ExecutionStep::new(step_number, &node_name);
            if let Some(state) = state_before {
                step = step.with_state_before(state);
            }
            record.current_step = Some(step);

            Some(ExecutionEvent::NodeEntered {
                execution_id: execution_id.to_string(),
                node: node_name,
                previous_node,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Record exiting a node successfully.
    /// Emits an [`ExecutionEvent::NodeExited`] event.
    pub fn exit_node_success(&self, execution_id: &str, state_after: Option<serde_json::Value>) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            if let Some(mut step) = record.current_step.take() {
                step.complete_success(state_after);
                let node = step.node_name.clone();
                let duration_ms = step.duration_ms.unwrap_or(0);
                let outcome = step.outcome.clone();
                record.add_to_history(step);

                Some(ExecutionEvent::NodeExited {
                    execution_id: execution_id.to_string(),
                    node,
                    duration_ms,
                    outcome,
                    timestamp: Utc::now().to_rfc3339(),
                })
            } else {
                None
            }
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Record exiting a node with an error.
    /// Emits an [`ExecutionEvent::NodeExited`] event.
    pub fn exit_node_error(&self, execution_id: &str, error: impl Into<String>) {
        let error = error.into();
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            if let Some(mut step) = record.current_step.take() {
                step.complete_error(&error);
                let node = step.node_name.clone();
                let duration_ms = step.duration_ms.unwrap_or(0);
                let outcome = step.outcome.clone();
                record.add_to_history(step);

                Some(ExecutionEvent::NodeExited {
                    execution_id: execution_id.to_string(),
                    node,
                    duration_ms,
                    outcome,
                    timestamp: Utc::now().to_rfc3339(),
                })
            } else {
                None
            }
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Update the current state snapshot for an execution.
    /// Emits an [`ExecutionEvent::StateChanged`] event.
    pub fn update_state(&self, execution_id: &str, state: serde_json::Value) {
        let mut executions = self.executions.write();

        let emit = if let Some(record) = executions.get_mut(execution_id) {
            record.state.set_state(state.clone());
            true
        } else {
            false
        };

        drop(executions);
        if emit {
            self.emit_event(ExecutionEvent::StateChanged {
                execution_id: execution_id.to_string(),
                state,
                timestamp: Utc::now().to_rfc3339(),
            });
        }
    }

    /// Increment the iteration counter.
    /// Emits an [`ExecutionEvent::IterationCompleted`] event.
    pub fn increment_iteration(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            record.state.iteration += 1;
            record.state.metrics.increment_iteration();
            Some(ExecutionEvent::IterationCompleted {
                execution_id: execution_id.to_string(),
                iteration: record.state.iteration,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Mark an execution as completed.
    /// Emits an [`ExecutionEvent::ExecutionCompleted`] event.
    pub fn complete_execution(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            record.state.complete();
            // Calculate total duration
            record.state.metrics.total_duration_ms = record.created_at.elapsed().as_millis() as u64;

            Some((
                ExecutionEvent::StatusChanged {
                    execution_id: execution_id.to_string(),
                    previous_status,
                    new_status: LiveExecutionStatus::Completed,
                    timestamp: Utc::now().to_rfc3339(),
                },
                ExecutionEvent::ExecutionCompleted {
                    execution_id: execution_id.to_string(),
                    final_state: record.state.state.clone(),
                    duration_ms: record.state.metrics.total_duration_ms,
                    timestamp: Utc::now().to_rfc3339(),
                },
            ))
        } else {
            None
        };

        drop(executions);
        if let Some((status_event, completed_event)) = event {
            self.emit_event(status_event);
            self.emit_event(completed_event);
        }
    }

    /// Mark an execution as failed.
    /// Emits an [`ExecutionEvent::ExecutionFailed`] event.
    pub fn fail_execution(&self, execution_id: &str, error: impl Into<String>) {
        let error = error.into();
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            let failed_node = Some(record.state.current_node.clone());
            record.state.fail(&error);
            record.state.metrics.total_duration_ms = record.created_at.elapsed().as_millis() as u64;

            Some((
                ExecutionEvent::StatusChanged {
                    execution_id: execution_id.to_string(),
                    previous_status,
                    new_status: LiveExecutionStatus::Failed,
                    timestamp: Utc::now().to_rfc3339(),
                },
                ExecutionEvent::ExecutionFailed {
                    execution_id: execution_id.to_string(),
                    error,
                    failed_node,
                    timestamp: Utc::now().to_rfc3339(),
                },
            ))
        } else {
            None
        };

        drop(executions);
        if let Some((status_event, failed_event)) = event {
            self.emit_event(status_event);
            self.emit_event(failed_event);
        }
    }

    /// Mark an execution as cancelled.
    /// Emits an [`ExecutionEvent::ExecutionCancelled`] event.
    pub fn cancel_execution(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            record.state.cancel();
            record.state.metrics.total_duration_ms = record.created_at.elapsed().as_millis() as u64;

            Some((
                ExecutionEvent::StatusChanged {
                    execution_id: execution_id.to_string(),
                    previous_status,
                    new_status: LiveExecutionStatus::Cancelled,
                    timestamp: Utc::now().to_rfc3339(),
                },
                ExecutionEvent::ExecutionCancelled {
                    execution_id: execution_id.to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                },
            ))
        } else {
            None
        };

        drop(executions);
        if let Some((status_event, cancelled_event)) = event {
            self.emit_event(status_event);
            self.emit_event(cancelled_event);
        }
    }

    /// Pause an execution.
    /// Emits an [`ExecutionEvent::StatusChanged`] event.
    pub fn pause_execution(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            record.state.pause();
            Some(ExecutionEvent::StatusChanged {
                execution_id: execution_id.to_string(),
                previous_status,
                new_status: LiveExecutionStatus::Paused,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Resume a paused execution.
    /// Emits an [`ExecutionEvent::StatusChanged`] event.
    pub fn resume_execution(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            record.state.resume();
            Some(ExecutionEvent::StatusChanged {
                execution_id: execution_id.to_string(),
                previous_status,
                new_status: LiveExecutionStatus::Running,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Set execution to waiting for input.
    /// Emits an [`ExecutionEvent::StatusChanged`] event.
    pub fn wait_for_input(&self, execution_id: &str) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            let previous_status = record.state.status;
            record.state.wait_for_input();
            Some(ExecutionEvent::StatusChanged {
                execution_id: execution_id.to_string(),
                previous_status,
                new_status: LiveExecutionStatus::WaitingForInput,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Record a checkpoint being created.
    /// Emits an [`ExecutionEvent::CheckpointCreated`] event.
    pub fn record_checkpoint(&self, execution_id: &str, size_bytes: u64) {
        let mut executions = self.executions.write();

        let event = if let Some(record) = executions.get_mut(execution_id) {
            record.state.checkpoint.record_checkpoint(size_bytes);
            Some(ExecutionEvent::CheckpointCreated {
                execution_id: execution_id.to_string(),
                checkpoint_number: record.state.checkpoint.checkpoint_count,
                size_bytes,
                timestamp: Utc::now().to_rfc3339(),
            })
        } else {
            None
        };

        drop(executions);
        if let Some(event) = event {
            self.emit_event(event);
        }
    }

    /// Enable checkpointing for an execution.
    pub fn enable_checkpointing(&self, execution_id: &str, thread_id: impl Into<String>) {
        let mut executions = self.executions.write();

        if let Some(record) = executions.get_mut(execution_id) {
            record.state.enable_checkpointing(thread_id);
        }
    }

    /// Get execution state by ID.
    #[must_use]
    pub fn get_execution(&self, execution_id: &str) -> Option<ExecutionState> {
        let executions = self.executions.read();
        executions.get(execution_id).map(|r| r.state.clone())
    }

    /// Get current node for an execution.
    #[must_use]
    pub fn current_node(&self, execution_id: &str) -> Option<String> {
        let executions = self.executions.read();
        executions
            .get(execution_id)
            .map(|r| r.state.current_node.clone())
    }

    /// Get current state for an execution.
    #[must_use]
    pub fn current_state(&self, execution_id: &str) -> Option<serde_json::Value> {
        let executions = self.executions.read();
        executions.get(execution_id).map(|r| r.state.state.clone())
    }

    /// Get execution history.
    #[must_use]
    pub fn execution_history(&self, execution_id: &str) -> Vec<ExecutionStep> {
        let executions = self.executions.read();
        executions
            .get(execution_id)
            .map(|r| r.history.clone())
            .unwrap_or_default()
    }

    /// Get execution metrics.
    #[must_use]
    pub fn execution_metrics(&self, execution_id: &str) -> Option<LiveExecutionMetrics> {
        let executions = self.executions.read();
        executions
            .get(execution_id)
            .map(|r| r.state.metrics.clone())
    }

    /// Get checkpoint status.
    #[must_use]
    pub fn checkpoint_status(&self, execution_id: &str) -> Option<CheckpointStatusInfo> {
        let executions = self.executions.read();
        executions
            .get(execution_id)
            .map(|r| r.state.checkpoint.clone())
    }

    /// List all active executions.
    #[must_use]
    pub fn active_executions(&self) -> Vec<ExecutionSummary> {
        let executions = self.executions.read();
        executions
            .values()
            .filter(|r| r.state.status.is_active())
            .map(|r| r.state.to_summary())
            .collect()
    }

    /// List all executions (including completed).
    #[must_use]
    pub fn all_executions(&self) -> Vec<ExecutionSummary> {
        let executions = self.executions.read();
        executions.values().map(|r| r.state.to_summary()).collect()
    }

    /// Get count of active executions.
    #[must_use]
    pub fn active_count(&self) -> usize {
        let executions = self.executions.read();
        executions
            .values()
            .filter(|r| r.state.status.is_active())
            .count()
    }

    /// Get total count of tracked executions.
    #[must_use]
    pub fn total_count(&self) -> usize {
        let executions = self.executions.read();
        executions.len()
    }

    /// Remove an execution from tracking.
    pub fn remove_execution(&self, execution_id: &str) -> Option<ExecutionState> {
        let mut executions = self.executions.write();
        executions.remove(execution_id).map(|r| r.state)
    }

    /// Cleanup expired completed executions.
    pub fn cleanup_expired(&self) {
        let mut executions = self.executions.write();
        self.cleanup_expired_internal(&mut executions);
    }

    /// Internal cleanup method (assumes lock is held).
    fn cleanup_expired_internal(&self, executions: &mut HashMap<String, ExecutionRecord>) {
        if self.config.completed_ttl_secs == 0 {
            return;
        }

        let ttl = Duration::from_secs(self.config.completed_ttl_secs);
        let now = Instant::now();

        executions.retain(|_, record| {
            // Keep active executions
            if record.state.status.is_active() {
                return true;
            }

            // Remove expired completed executions
            now.duration_since(record.created_at) < ttl
        });
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &ExecutionTrackerConfig {
        &self.config
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_status_is_active() {
        assert!(LiveExecutionStatus::Running.is_active());
        assert!(LiveExecutionStatus::Paused.is_active());
        assert!(LiveExecutionStatus::WaitingForInput.is_active());
        assert!(!LiveExecutionStatus::Completed.is_active());
        assert!(!LiveExecutionStatus::Failed.is_active());
        assert!(!LiveExecutionStatus::Cancelled.is_active());
    }

    #[test]
    fn test_execution_status_is_terminal() {
        assert!(!LiveExecutionStatus::Running.is_terminal());
        assert!(!LiveExecutionStatus::Paused.is_terminal());
        assert!(LiveExecutionStatus::Completed.is_terminal());
        assert!(LiveExecutionStatus::Failed.is_terminal());
        assert!(LiveExecutionStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_step_outcome_is_success() {
        assert!(StepOutcome::Success.is_success());
        assert!(!StepOutcome::Error("err".into()).is_success());
        assert!(!StepOutcome::Skipped.is_success());
        assert!(!StepOutcome::InProgress.is_success());
    }

    #[test]
    fn test_execution_step_new() {
        let step = ExecutionStep::new(1, "test_node");
        assert_eq!(step.step_number, 1);
        assert_eq!(step.node_name, "test_node");
        assert!(step.completed_at.is_none());
        assert!(matches!(step.outcome, StepOutcome::InProgress));
    }

    #[test]
    fn test_execution_step_complete_success() {
        let mut step = ExecutionStep::new(1, "test_node");
        step.complete_success(Some(serde_json::json!({"result": "ok"})));

        assert!(step.completed_at.is_some());
        assert!(matches!(step.outcome, StepOutcome::Success));
        assert!(step.state_after.is_some());
    }

    #[test]
    fn test_execution_step_complete_error() {
        let mut step = ExecutionStep::new(1, "test_node");
        step.complete_error("Something went wrong");

        assert!(step.completed_at.is_some());
        assert!(matches!(step.outcome, StepOutcome::Error(_)));
    }

    #[test]
    fn test_live_execution_metrics_new() {
        let metrics = LiveExecutionMetrics::new();
        assert_eq!(metrics.nodes_executed, 0);
        assert_eq!(metrics.nodes_succeeded, 0);
        assert_eq!(metrics.nodes_failed, 0);
    }

    #[test]
    fn test_live_execution_metrics_record_step() {
        let mut metrics = LiveExecutionMetrics::new();
        let mut step = ExecutionStep::new(1, "node1");
        step.duration_ms = Some(100);
        step.outcome = StepOutcome::Success;

        metrics.record_step(&step);

        assert_eq!(metrics.nodes_executed, 1);
        assert_eq!(metrics.nodes_succeeded, 1);
        assert_eq!(metrics.slowest_node, Some(("node1".into(), 100)));
    }

    #[test]
    fn test_checkpoint_status_disabled() {
        let status = CheckpointStatusInfo::disabled();
        assert!(!status.enabled);
        assert!(status.thread_id.is_none());
    }

    #[test]
    fn test_checkpoint_status_enabled() {
        let status = CheckpointStatusInfo::enabled("thread-123");
        assert!(status.enabled);
        assert_eq!(status.thread_id, Some("thread-123".into()));
    }

    #[test]
    fn test_checkpoint_status_record() {
        let mut status = CheckpointStatusInfo::enabled("thread-123");
        status.record_checkpoint(1024);

        assert_eq!(status.checkpoint_count, 1);
        assert_eq!(status.total_size_bytes, 1024);
        assert!(status.last_checkpoint_at.is_some());
    }

    #[test]
    fn test_execution_state_new() {
        let state = ExecutionState::new("exec-1", "my_graph", "__start__");

        assert_eq!(state.execution_id, "exec-1");
        assert_eq!(state.graph_name, "my_graph");
        assert_eq!(state.current_node, "__start__");
        assert!(matches!(state.status, LiveExecutionStatus::Running));
    }

    #[test]
    fn test_execution_state_enter_node() {
        let mut state = ExecutionState::new("exec-1", "my_graph", "__start__");
        state.enter_node("process");

        assert_eq!(state.current_node, "process");
        assert_eq!(state.previous_node, Some("__start__".into()));
        assert_eq!(state.total_nodes_visited, 1);
    }

    #[test]
    fn test_execution_state_status_transitions() {
        let mut state = ExecutionState::new("exec-1", "my_graph", "__start__");

        state.pause();
        assert!(matches!(state.status, LiveExecutionStatus::Paused));

        state.resume();
        assert!(matches!(state.status, LiveExecutionStatus::Running));

        state.wait_for_input();
        assert!(matches!(state.status, LiveExecutionStatus::WaitingForInput));

        state.complete();
        assert!(matches!(state.status, LiveExecutionStatus::Completed));
    }

    #[test]
    fn test_execution_state_fail() {
        let mut state = ExecutionState::new("exec-1", "my_graph", "__start__");
        state.fail("Something went wrong");

        assert!(matches!(state.status, LiveExecutionStatus::Failed));
        assert_eq!(state.error, Some("Something went wrong".into()));
    }

    #[test]
    fn test_execution_state_to_summary() {
        let state = ExecutionState::new("exec-1", "my_graph", "__start__");
        let summary = state.to_summary();

        assert_eq!(summary.execution_id, "exec-1");
        assert_eq!(summary.graph_name, "my_graph");
        assert_eq!(summary.current_node, "__start__");
    }

    #[test]
    fn test_tracker_start_execution() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph");

        assert!(exec_id.is_some());
        let exec_id = exec_id.unwrap();
        assert!(!exec_id.is_empty());

        let state = tracker.get_execution(&exec_id);
        assert!(state.is_some());
        assert_eq!(state.unwrap().graph_name, "my_graph");
    }

    #[test]
    fn test_tracker_start_execution_with_id() {
        let tracker = ExecutionTracker::new();
        let success = tracker.start_execution_with_id("my-exec-123", "my_graph", "__start__");

        assert!(success);

        let state = tracker.get_execution("my-exec-123");
        assert!(state.is_some());
        assert_eq!(state.unwrap().execution_id, "my-exec-123");
    }

    #[test]
    fn test_tracker_start_execution_duplicate_id() {
        let tracker = ExecutionTracker::new();
        tracker.start_execution_with_id("my-exec-123", "graph1", "__start__");
        let duplicate = tracker.start_execution_with_id("my-exec-123", "graph2", "__start__");

        assert!(!duplicate);
    }

    #[test]
    fn test_tracker_node_lifecycle() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node(&exec_id, "process");
        assert_eq!(tracker.current_node(&exec_id), Some("process".into()));

        tracker.exit_node_success(&exec_id, Some(serde_json::json!({"done": true})));

        let history = tracker.execution_history(&exec_id);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].node_name, "process");
        assert!(matches!(history[0].outcome, StepOutcome::Success));
    }

    #[test]
    fn test_tracker_node_error() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node(&exec_id, "failing_node");
        tracker.exit_node_error(&exec_id, "Node failed");

        let history = tracker.execution_history(&exec_id);
        assert_eq!(history.len(), 1);
        assert!(matches!(history[0].outcome, StepOutcome::Error(_)));
    }

    #[test]
    fn test_tracker_update_state() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.update_state(&exec_id, serde_json::json!({"counter": 42}));

        let state = tracker.current_state(&exec_id);
        assert!(state.is_some());
        assert_eq!(state.unwrap()["counter"], 42);
    }

    #[test]
    fn test_tracker_complete_execution() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.complete_execution(&exec_id);

        let state = tracker.get_execution(&exec_id);
        assert!(matches!(
            state.unwrap().status,
            LiveExecutionStatus::Completed
        ));
    }

    #[test]
    fn test_tracker_fail_execution() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.fail_execution(&exec_id, "Critical error");

        let state = tracker.get_execution(&exec_id).unwrap();
        assert!(matches!(state.status, LiveExecutionStatus::Failed));
        assert_eq!(state.error, Some("Critical error".into()));
    }

    #[test]
    fn test_tracker_active_executions() {
        let tracker = ExecutionTracker::new();
        let exec1 = tracker.start_execution("graph1").unwrap();
        let exec2 = tracker.start_execution("graph2").unwrap();

        tracker.complete_execution(&exec1);

        let active = tracker.active_executions();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].execution_id, exec2);
    }

    #[test]
    fn test_tracker_all_executions() {
        let tracker = ExecutionTracker::new();
        tracker.start_execution("graph1").unwrap();
        tracker.start_execution("graph2").unwrap();

        let all = tracker.all_executions();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_tracker_active_count() {
        let tracker = ExecutionTracker::new();
        let exec1 = tracker.start_execution("graph1").unwrap();
        tracker.start_execution("graph2").unwrap();

        assert_eq!(tracker.active_count(), 2);

        tracker.complete_execution(&exec1);
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn test_tracker_remove_execution() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let removed = tracker.remove_execution(&exec_id);
        assert!(removed.is_some());
        assert!(tracker.get_execution(&exec_id).is_none());
    }

    #[test]
    fn test_tracker_capacity_limit() {
        let config = ExecutionTrackerConfig::new(2, 100, 0);
        let tracker = ExecutionTracker::with_config(config);

        let exec1 = tracker.start_execution("graph1");
        let exec2 = tracker.start_execution("graph2");
        let exec3 = tracker.start_execution("graph3");

        assert!(exec1.is_some());
        assert!(exec2.is_some());
        assert!(exec3.is_none()); // At capacity
    }

    #[test]
    fn test_tracker_capacity_allows_after_completion() {
        let config = ExecutionTrackerConfig::new(2, 100, 0);
        let tracker = ExecutionTracker::with_config(config);

        let exec1 = tracker.start_execution("graph1").unwrap();
        tracker.start_execution("graph2").unwrap();

        // At capacity
        assert!(tracker.start_execution("graph3").is_none());

        // Complete one execution
        tracker.complete_execution(&exec1);

        // Now we should be able to start another
        let exec3 = tracker.start_execution("graph3");
        assert!(exec3.is_some());
    }

    #[test]
    fn test_tracker_history_limit() {
        let config = ExecutionTrackerConfig::new(1000, 3, 0);
        let tracker = ExecutionTracker::with_config(config);
        let exec_id = tracker.start_execution("my_graph").unwrap();

        // Add 5 steps, but limit is 3
        for i in 1..=5 {
            tracker.enter_node(&exec_id, format!("node{i}"));
            tracker.exit_node_success(&exec_id, None);
        }

        let history = tracker.execution_history(&exec_id);
        assert_eq!(history.len(), 3);
        // Should have the most recent 3
        assert_eq!(history[0].node_name, "node3");
        assert_eq!(history[1].node_name, "node4");
        assert_eq!(history[2].node_name, "node5");
    }

    #[test]
    fn test_tracker_checkpoint_operations() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enable_checkpointing(&exec_id, "thread-abc");
        tracker.record_checkpoint(&exec_id, 2048);
        tracker.record_checkpoint(&exec_id, 1024);

        let status = tracker.checkpoint_status(&exec_id).unwrap();
        assert!(status.enabled);
        assert_eq!(status.thread_id, Some("thread-abc".into()));
        assert_eq!(status.checkpoint_count, 2);
        assert_eq!(status.total_size_bytes, 3072);
    }

    #[test]
    fn test_tracker_increment_iteration() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.increment_iteration(&exec_id);
        tracker.increment_iteration(&exec_id);

        let state = tracker.get_execution(&exec_id).unwrap();
        assert_eq!(state.iteration, 2);
        assert_eq!(state.metrics.iteration, 2);
    }

    #[test]
    fn test_tracker_pause_resume() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.pause_execution(&exec_id);
        let state = tracker.get_execution(&exec_id).unwrap();
        assert!(matches!(state.status, LiveExecutionStatus::Paused));

        tracker.resume_execution(&exec_id);
        let state = tracker.get_execution(&exec_id).unwrap();
        assert!(matches!(state.status, LiveExecutionStatus::Running));
    }

    #[test]
    fn test_config_defaults() {
        let config = ExecutionTrackerConfig::default();
        assert_eq!(
            config.max_concurrent_executions,
            DEFAULT_MAX_CONCURRENT_EXECUTIONS
        );
        assert_eq!(config.max_history_steps, DEFAULT_MAX_HISTORY_STEPS);
        assert_eq!(config.completed_ttl_secs, DEFAULT_COMPLETED_TTL_SECS);
    }

    #[test]
    fn test_config_high_throughput() {
        let config = ExecutionTrackerConfig::high_throughput();
        assert_eq!(config.max_concurrent_executions, 10000);
        assert_eq!(config.max_history_steps, 50);
        assert_eq!(config.completed_ttl_secs, 60);
    }

    #[test]
    fn test_config_debug() {
        let config = ExecutionTrackerConfig::debug();
        assert_eq!(config.max_concurrent_executions, 100);
        assert_eq!(config.max_history_steps, 1000);
        assert_eq!(config.completed_ttl_secs, 3600);
    }

    #[test]
    fn test_execution_status_display() {
        assert_eq!(LiveExecutionStatus::Running.to_string(), "running");
        assert_eq!(LiveExecutionStatus::Paused.to_string(), "paused");
        assert_eq!(
            LiveExecutionStatus::WaitingForInput.to_string(),
            "waiting_for_input"
        );
        assert_eq!(LiveExecutionStatus::Completed.to_string(), "completed");
        assert_eq!(LiveExecutionStatus::Failed.to_string(), "failed");
        assert_eq!(LiveExecutionStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_execution_state_serialization() {
        let state = ExecutionState::new("exec-1", "my_graph", "__start__");
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ExecutionState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.execution_id, deserialized.execution_id);
        assert_eq!(state.graph_name, deserialized.graph_name);
    }

    #[test]
    fn test_execution_summary_serialization() {
        let state = ExecutionState::new("exec-1", "my_graph", "__start__");
        let summary = state.to_summary();
        let json = serde_json::to_string(&summary).unwrap();

        assert!(json.contains("exec-1"));
        assert!(json.contains("my_graph"));
    }

    #[test]
    fn test_execution_step_with_state_before() {
        let step =
            ExecutionStep::new(1, "node1").with_state_before(serde_json::json!({"input": "value"}));

        assert!(step.state_before.is_some());
        assert_eq!(step.state_before.unwrap()["input"], "value");
    }

    #[test]
    fn test_tracker_metrics_update() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node(&exec_id, "slow_node");
        // Simulate some delay
        std::thread::sleep(std::time::Duration::from_millis(10));
        tracker.exit_node_success(&exec_id, None);

        let metrics = tracker.execution_metrics(&exec_id).unwrap();
        assert_eq!(metrics.nodes_executed, 1);
        assert_eq!(metrics.nodes_succeeded, 1);
        assert!(metrics.slowest_node.is_some());
    }

    #[test]
    fn test_enter_node_with_state() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node_with_state(
            &exec_id,
            "process",
            Some(serde_json::json!({"before": true})),
        );
        tracker.exit_node_success(&exec_id, Some(serde_json::json!({"after": true})));

        let history = tracker.execution_history(&exec_id);
        assert_eq!(history.len(), 1);
        assert!(history[0].state_before.is_some());
        assert!(history[0].state_after.is_some());
    }

    // ========================================================================
    // Event Streaming Tests
    // ========================================================================

    #[test]
    fn test_execution_event_execution_id() {
        let event = ExecutionEvent::ExecutionStarted {
            execution_id: "exec-1".into(),
            graph_name: "test".into(),
            entry_point: "__start__".into(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        assert_eq!(event.execution_id(), "exec-1");
    }

    #[test]
    fn test_execution_event_timestamp() {
        let event = ExecutionEvent::NodeEntered {
            execution_id: "exec-1".into(),
            node: "process".into(),
            previous_node: None,
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        assert_eq!(event.timestamp(), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_execution_event_type() {
        let events = vec![
            (
                ExecutionEvent::ExecutionStarted {
                    execution_id: "e".into(),
                    graph_name: "g".into(),
                    entry_point: "s".into(),
                    timestamp: "t".into(),
                },
                "execution_started",
            ),
            (
                ExecutionEvent::NodeEntered {
                    execution_id: "e".into(),
                    node: "n".into(),
                    previous_node: None,
                    timestamp: "t".into(),
                },
                "node_entered",
            ),
            (
                ExecutionEvent::NodeExited {
                    execution_id: "e".into(),
                    node: "n".into(),
                    duration_ms: 0,
                    outcome: StepOutcome::Success,
                    timestamp: "t".into(),
                },
                "node_exited",
            ),
            (
                ExecutionEvent::StateChanged {
                    execution_id: "e".into(),
                    state: serde_json::Value::Null,
                    timestamp: "t".into(),
                },
                "state_changed",
            ),
            (
                ExecutionEvent::CheckpointCreated {
                    execution_id: "e".into(),
                    checkpoint_number: 1,
                    size_bytes: 0,
                    timestamp: "t".into(),
                },
                "checkpoint_created",
            ),
            (
                ExecutionEvent::IterationCompleted {
                    execution_id: "e".into(),
                    iteration: 1,
                    timestamp: "t".into(),
                },
                "iteration_completed",
            ),
            (
                ExecutionEvent::StatusChanged {
                    execution_id: "e".into(),
                    previous_status: LiveExecutionStatus::Running,
                    new_status: LiveExecutionStatus::Paused,
                    timestamp: "t".into(),
                },
                "status_changed",
            ),
            (
                ExecutionEvent::ExecutionCompleted {
                    execution_id: "e".into(),
                    final_state: serde_json::Value::Null,
                    duration_ms: 0,
                    timestamp: "t".into(),
                },
                "execution_completed",
            ),
            (
                ExecutionEvent::ExecutionFailed {
                    execution_id: "e".into(),
                    error: "err".into(),
                    failed_node: None,
                    timestamp: "t".into(),
                },
                "execution_failed",
            ),
            (
                ExecutionEvent::ExecutionCancelled {
                    execution_id: "e".into(),
                    timestamp: "t".into(),
                },
                "execution_cancelled",
            ),
        ];

        for (event, expected_type) in events {
            assert_eq!(event.event_type(), expected_type);
        }
    }

    #[test]
    fn test_execution_event_serialization() {
        let event = ExecutionEvent::NodeEntered {
            execution_id: "exec-1".into(),
            node: "process".into(),
            previous_node: Some("__start__".into()),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"node_entered\""));
        assert!(json.contains("\"execution_id\":\"exec-1\""));
        assert!(json.contains("\"node\":\"process\""));

        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_tracker_subscriber_count() {
        let tracker = ExecutionTracker::new();
        assert_eq!(tracker.subscriber_count(), 0);

        let _stream1 = tracker.subscribe();
        assert_eq!(tracker.subscriber_count(), 1);

        let _stream2 = tracker.subscribe();
        assert_eq!(tracker.subscriber_count(), 2);
    }

    #[test]
    fn test_execution_event_stream_filter() {
        let (tx, rx) = broadcast::channel::<ExecutionEvent>(16);

        let stream = ExecutionEventStream::new(rx);
        assert!(!stream.has_filter());
        assert!(stream.execution_filter().is_none());

        let filtered = ExecutionEventStream::for_execution(tx.subscribe(), "exec-1");
        assert!(filtered.has_filter());
        assert_eq!(filtered.execution_filter(), Some("exec-1"));
    }

    #[tokio::test]
    async fn test_event_stream_recv() {
        let tracker = ExecutionTracker::new();
        let mut stream = tracker.subscribe();

        // Start execution (emits ExecutionStarted event)
        let exec_id = tracker.start_execution("my_graph").unwrap();

        // Receive the event
        let event = stream.try_recv();
        assert!(event.is_some());
        let event = event.unwrap();

        match event {
            ExecutionEvent::ExecutionStarted {
                execution_id,
                graph_name,
                entry_point,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(graph_name, "my_graph");
                assert_eq!(entry_point, "__start__");
            }
            _ => panic!("Expected ExecutionStarted event"),
        }
    }

    #[tokio::test]
    async fn test_event_stream_filtered() {
        let tracker = ExecutionTracker::new();

        // Start two executions
        let exec1 = tracker.start_execution("graph1").unwrap();
        let exec2 = tracker.start_execution("graph2").unwrap();

        // Subscribe filtered to exec2
        let mut stream = tracker.subscribe_to_execution(&exec2);

        // Emit events for both executions
        tracker.enter_node(&exec1, "node1");
        tracker.enter_node(&exec2, "node2");

        // Should only receive exec2 events (skipping exec1)
        let event = stream.try_recv();
        assert!(event.is_some());
        assert_eq!(event.unwrap().execution_id(), exec2);
    }

    #[tokio::test]
    async fn test_event_node_entered() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.enter_node(&exec_id, "process");

        let event = stream.try_recv();
        assert!(event.is_some());
        match event.unwrap() {
            ExecutionEvent::NodeEntered {
                execution_id,
                node,
                previous_node,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(node, "process");
                // First enter_node call - previous_node is captured BEFORE enter_node updates state
                // When ExecutionState::new is called, current_node = __start__, previous_node = None
                // When enter_node is called, it captures previous_node (None) before updating
                assert_eq!(previous_node, None);
            }
            _ => panic!("Expected NodeEntered event"),
        }
    }

    #[tokio::test]
    async fn test_event_node_exited() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node(&exec_id, "process");

        let mut stream = tracker.subscribe();
        tracker.exit_node_success(&exec_id, None);

        let event = stream.try_recv();
        assert!(event.is_some());
        match event.unwrap() {
            ExecutionEvent::NodeExited {
                execution_id,
                node,
                outcome,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(node, "process");
                assert!(matches!(outcome, StepOutcome::Success));
            }
            _ => panic!("Expected NodeExited event"),
        }
    }

    #[tokio::test]
    async fn test_event_state_changed() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.update_state(&exec_id, serde_json::json!({"count": 42}));

        let event = stream.try_recv();
        assert!(event.is_some());
        match event.unwrap() {
            ExecutionEvent::StateChanged {
                execution_id,
                state,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(state["count"], 42);
            }
            _ => panic!("Expected StateChanged event"),
        }
    }

    #[tokio::test]
    async fn test_event_iteration_completed() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.increment_iteration(&exec_id);

        let event = stream.try_recv();
        assert!(event.is_some());
        match event.unwrap() {
            ExecutionEvent::IterationCompleted {
                execution_id,
                iteration,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(iteration, 1);
            }
            _ => panic!("Expected IterationCompleted event"),
        }
    }

    #[tokio::test]
    async fn test_event_checkpoint_created() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.record_checkpoint(&exec_id, 1024);

        let event = stream.try_recv();
        assert!(event.is_some());
        match event.unwrap() {
            ExecutionEvent::CheckpointCreated {
                execution_id,
                checkpoint_number,
                size_bytes,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(checkpoint_number, 1);
                assert_eq!(size_bytes, 1024);
            }
            _ => panic!("Expected CheckpointCreated event"),
        }
    }

    #[tokio::test]
    async fn test_event_execution_completed() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.complete_execution(&exec_id);

        // Should emit StatusChanged followed by ExecutionCompleted
        let event1 = stream.try_recv();
        assert!(event1.is_some());
        assert!(matches!(
            event1.unwrap(),
            ExecutionEvent::StatusChanged { .. }
        ));

        let event2 = stream.try_recv();
        assert!(event2.is_some());
        match event2.unwrap() {
            ExecutionEvent::ExecutionCompleted { execution_id, .. } => {
                assert_eq!(execution_id, exec_id);
            }
            _ => panic!("Expected ExecutionCompleted event"),
        }
    }

    #[tokio::test]
    async fn test_event_execution_failed() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        tracker.enter_node(&exec_id, "failing_node");

        let mut stream = tracker.subscribe();
        tracker.fail_execution(&exec_id, "Something went wrong");

        // Should emit StatusChanged followed by ExecutionFailed
        let event1 = stream.try_recv();
        assert!(event1.is_some());
        assert!(matches!(
            event1.unwrap(),
            ExecutionEvent::StatusChanged { .. }
        ));

        let event2 = stream.try_recv();
        assert!(event2.is_some());
        match event2.unwrap() {
            ExecutionEvent::ExecutionFailed {
                execution_id,
                error,
                failed_node,
                ..
            } => {
                assert_eq!(execution_id, exec_id);
                assert_eq!(error, "Something went wrong");
                assert_eq!(failed_node, Some("failing_node".into()));
            }
            _ => panic!("Expected ExecutionFailed event"),
        }
    }

    #[tokio::test]
    async fn test_event_execution_cancelled() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();
        tracker.cancel_execution(&exec_id);

        // Should emit StatusChanged followed by ExecutionCancelled
        let event1 = stream.try_recv();
        assert!(event1.is_some());
        assert!(matches!(
            event1.unwrap(),
            ExecutionEvent::StatusChanged { .. }
        ));

        let event2 = stream.try_recv();
        assert!(event2.is_some());
        match event2.unwrap() {
            ExecutionEvent::ExecutionCancelled { execution_id, .. } => {
                assert_eq!(execution_id, exec_id);
            }
            _ => panic!("Expected ExecutionCancelled event"),
        }
    }

    #[tokio::test]
    async fn test_event_status_changes() {
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("my_graph").unwrap();

        let mut stream = tracker.subscribe();

        // Test pause
        tracker.pause_execution(&exec_id);
        let event = stream.try_recv().unwrap();
        match event {
            ExecutionEvent::StatusChanged {
                previous_status,
                new_status,
                ..
            } => {
                assert_eq!(previous_status, LiveExecutionStatus::Running);
                assert_eq!(new_status, LiveExecutionStatus::Paused);
            }
            _ => panic!("Expected StatusChanged event"),
        }

        // Test resume
        tracker.resume_execution(&exec_id);
        let event = stream.try_recv().unwrap();
        match event {
            ExecutionEvent::StatusChanged {
                previous_status,
                new_status,
                ..
            } => {
                assert_eq!(previous_status, LiveExecutionStatus::Paused);
                assert_eq!(new_status, LiveExecutionStatus::Running);
            }
            _ => panic!("Expected StatusChanged event"),
        }

        // Test wait for input
        tracker.wait_for_input(&exec_id);
        let event = stream.try_recv().unwrap();
        match event {
            ExecutionEvent::StatusChanged {
                previous_status,
                new_status,
                ..
            } => {
                assert_eq!(previous_status, LiveExecutionStatus::Running);
                assert_eq!(new_status, LiveExecutionStatus::WaitingForInput);
            }
            _ => panic!("Expected StatusChanged event"),
        }
    }

    #[test]
    fn test_tracker_with_custom_channel_capacity() {
        let config = ExecutionTrackerConfig::default();
        let tracker = ExecutionTracker::with_config_and_capacity(config, 16);

        // Should work normally
        let exec_id = tracker.start_execution("my_graph");
        assert!(exec_id.is_some());
    }
}
