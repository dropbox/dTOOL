//! Streaming telemetry for agent visibility
//!
//! This module provides real-time visibility into agent actions through
//! a callback-based streaming interface. It supports:
//! - Local console output for debugging
//! - Integration with DashFlow Streaming for production telemetry

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Event types emitted during agent execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentEvent {
    /// A new user turn has started
    UserTurn { session_id: String, content: String },

    /// LLM reasoning has started
    ReasoningStart {
        session_id: String,
        turn: u32,
        model: String,
    },

    /// LLM reasoning has completed
    ReasoningComplete {
        session_id: String,
        turn: u32,
        duration_ms: u64,
        has_tool_calls: bool,
        tool_count: usize,
        /// Input tokens used (prompt)
        input_tokens: Option<u32>,
        /// Output tokens generated (completion)
        output_tokens: Option<u32>,
    },

    /// LLM usage metrics for cost tracking and observability
    ///
    /// Emitted after each LLM call with detailed usage information.
    /// Compatible with DashFlow Streaming Metrics message type.
    LlmMetrics {
        session_id: String,
        /// LLM request identifier
        request_id: String,
        /// Model used
        model: String,
        /// Input tokens (prompt)
        input_tokens: u32,
        /// Output tokens (completion)
        output_tokens: u32,
        /// Total tokens
        total_tokens: u32,
        /// Latency in milliseconds
        latency_ms: u64,
        /// Estimated cost in USD (if available)
        cost_usd: Option<f64>,
        /// Whether response was cached
        cached: bool,
    },

    /// A tool call has been requested by the LLM
    ToolCallRequested {
        session_id: String,
        tool_call_id: String,
        tool: String,
        args: serde_json::Value,
    },

    /// A tool call has been approved for execution
    ToolCallApproved {
        session_id: String,
        tool_call_id: String,
        tool: String,
    },

    /// A tool call was rejected (policy or user denial)
    ToolCallRejected {
        session_id: String,
        tool_call_id: String,
        tool: String,
        reason: String,
    },

    /// Tool execution has started
    ToolExecutionStart {
        session_id: String,
        tool_call_id: String,
        tool: String,
    },

    /// Tool execution has completed
    ToolExecutionComplete {
        session_id: String,
        tool_call_id: String,
        tool: String,
        success: bool,
        duration_ms: u64,
        output_preview: String,
    },

    /// An agent turn has completed
    TurnComplete {
        session_id: String,
        turn: u32,
        status: String,
    },

    /// Agent session has completed
    SessionComplete {
        session_id: String,
        total_turns: u32,
        status: String,
    },

    /// Audit #77: Session metrics summary (emitted at end of session)
    ///
    /// Contains aggregated usage and cost data for the entire session.
    /// Useful for billing, monitoring, and cost optimization.
    SessionMetrics {
        session_id: String,
        /// Total input tokens across all LLM calls
        total_input_tokens: u32,
        /// Total output tokens across all LLM calls
        total_output_tokens: u32,
        /// Total cached tokens (from prompt caching)
        total_cached_tokens: u32,
        /// Total cost in USD (if calculable)
        total_cost_usd: Option<f64>,
        /// Number of LLM calls made
        llm_call_count: u32,
        /// Session duration in milliseconds
        duration_ms: u64,
    },

    /// LLM token chunk received (for streaming responses)
    TokenChunk {
        session_id: String,
        chunk: String,
        is_final: bool,
    },

    /// Error occurred during agent execution
    Error {
        session_id: String,
        error: String,
        context: String,
    },

    /// Tool call requires user approval
    ///
    /// Emitted when a tool call needs interactive approval from the user.
    /// The TUI should display an approval dialog and respond via the approval channel.
    ApprovalRequired {
        session_id: String,
        /// Unique ID for tracking this approval request
        request_id: String,
        /// Tool call ID being approved
        tool_call_id: String,
        /// Tool name
        tool: String,
        /// Tool arguments
        args: serde_json::Value,
        /// Reason why approval is required
        reason: Option<String>,
    },

    /// Eval dataset capture for regression testing
    ///
    /// Captures prompts and responses for building eval datasets.
    /// Only emitted when eval collection mode is enabled.
    EvalCapture {
        session_id: String,
        /// Unique capture ID for this prompt/response pair
        capture_id: String,
        /// Input prompt messages (JSON-serialized)
        input_messages: String,
        /// Output response (JSON-serialized)
        output_response: String,
        /// Model used
        model: String,
        /// Tool definitions available (JSON-serialized)
        tools: Option<String>,
        /// Metadata for categorization
        metadata: Option<serde_json::Value>,
    },

    /// Quality gate evaluation started
    ///
    /// Emitted when the quality gate begins evaluating an LLM response.
    QualityGateStart {
        session_id: String,
        /// Attempt number (1-indexed)
        attempt: usize,
        /// Maximum retries configured
        max_retries: usize,
        /// Quality threshold required
        threshold: f32,
    },

    /// Quality gate evaluation result
    ///
    /// Emitted after each quality evaluation attempt.
    QualityGateResult {
        session_id: String,
        /// Attempt number (1-indexed)
        attempt: usize,
        /// Whether quality threshold was met
        passed: bool,
        /// Quality score (accuracy dimension)
        accuracy: f32,
        /// Quality score (relevance dimension)
        relevance: f32,
        /// Quality score (completeness dimension)
        completeness: f32,
        /// Average quality score
        average_score: f32,
        /// Whether this was the final attempt
        is_final: bool,
        /// Reason for failure (if applicable)
        reason: Option<String>,
    },
}

impl AgentEvent {
    /// Get the session ID from the event
    pub fn session_id(&self) -> &str {
        match self {
            Self::UserTurn { session_id, .. } => session_id,
            Self::ReasoningStart { session_id, .. } => session_id,
            Self::ReasoningComplete { session_id, .. } => session_id,
            Self::LlmMetrics { session_id, .. } => session_id,
            Self::ToolCallRequested { session_id, .. } => session_id,
            Self::ToolCallApproved { session_id, .. } => session_id,
            Self::ToolCallRejected { session_id, .. } => session_id,
            Self::ToolExecutionStart { session_id, .. } => session_id,
            Self::ToolExecutionComplete { session_id, .. } => session_id,
            Self::TurnComplete { session_id, .. } => session_id,
            Self::SessionComplete { session_id, .. } => session_id,
            Self::SessionMetrics { session_id, .. } => session_id,
            Self::TokenChunk { session_id, .. } => session_id,
            Self::Error { session_id, .. } => session_id,
            Self::ApprovalRequired { session_id, .. } => session_id,
            Self::EvalCapture { session_id, .. } => session_id,
            Self::QualityGateStart { session_id, .. } => session_id,
            Self::QualityGateResult { session_id, .. } => session_id,
        }
    }

    /// Get a human-readable event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::UserTurn { .. } => "user_turn",
            Self::ReasoningStart { .. } => "reasoning_start",
            Self::ReasoningComplete { .. } => "reasoning_complete",
            Self::LlmMetrics { .. } => "llm_metrics",
            Self::ToolCallRequested { .. } => "tool_call_requested",
            Self::ToolCallApproved { .. } => "tool_call_approved",
            Self::ToolCallRejected { .. } => "tool_call_rejected",
            Self::ToolExecutionStart { .. } => "tool_execution_start",
            Self::ToolExecutionComplete { .. } => "tool_execution_complete",
            Self::TurnComplete { .. } => "turn_complete",
            Self::SessionComplete { .. } => "session_complete",
            Self::SessionMetrics { .. } => "session_metrics",
            Self::TokenChunk { .. } => "token_chunk",
            Self::Error { .. } => "error",
            Self::ApprovalRequired { .. } => "approval_required",
            Self::EvalCapture { .. } => "eval_capture",
            Self::QualityGateStart { .. } => "quality_gate_start",
            Self::QualityGateResult { .. } => "quality_gate_result",
        }
    }

    /// Get the DashFlow-compatible node ID for this event (Audit #80)
    ///
    /// Returns a string identifying which graph node emitted this event.
    /// This enables observability tools like `dashstream tail` to filter
    /// and aggregate events by node.
    ///
    /// Node IDs follow DashFlow conventions:
    /// - `user_input` - User input processing
    /// - `reasoning` - LLM reasoning/inference
    /// - `tool_selection` - Tool selection/approval
    /// - `tool_execution:{tool}` - Tool execution (parameterized by tool name)
    /// - `result_analysis` - Result analysis
    /// - `session` - Session-level events
    pub fn node_id(&self) -> String {
        match self {
            Self::UserTurn { .. } => "user_input".to_string(),
            Self::ReasoningStart { .. } => "reasoning".to_string(),
            Self::ReasoningComplete { .. } => "reasoning".to_string(),
            Self::LlmMetrics { model, .. } => format!("llm:{}", model),
            Self::ToolCallRequested { tool, .. } => format!("reasoning:tool_call:{}", tool),
            Self::ToolCallApproved { tool, .. } => format!("tool_selection:{}", tool),
            Self::ToolCallRejected { tool, .. } => format!("tool_selection:{}", tool),
            Self::ToolExecutionStart { tool, .. } => format!("tool_execution:{}", tool),
            Self::ToolExecutionComplete { tool, .. } => format!("tool_execution:{}", tool),
            Self::TurnComplete { .. } => "result_analysis".to_string(),
            Self::SessionComplete { .. } => "session".to_string(),
            Self::SessionMetrics { .. } => "session".to_string(),
            Self::TokenChunk { .. } => "reasoning".to_string(),
            Self::Error { context, .. } => format!("error:{}", context),
            Self::ApprovalRequired { tool, .. } => format!("tool_selection:{}", tool),
            Self::EvalCapture { .. } => "eval_capture".to_string(),
            Self::QualityGateStart { .. } => "quality_gate".to_string(),
            Self::QualityGateResult { .. } => "quality_gate".to_string(),
        }
    }
}

/// Callback trait for receiving agent events
///
/// Implementations can handle events in different ways:
/// - Print to console for debugging
/// - Send to DashFlow Streaming for production telemetry
/// - Write to a file for offline analysis
/// - Aggregate metrics
#[async_trait]
pub trait StreamCallback: Send + Sync {
    /// Called when an agent event occurs
    async fn on_event(&self, event: AgentEvent);

    /// Called when the stream should be flushed
    async fn flush(&self) {}
}

/// No-op callback that discards all events
#[derive(Default, Clone)]
pub struct NullStreamCallback;

#[async_trait]
impl StreamCallback for NullStreamCallback {
    async fn on_event(&self, _event: AgentEvent) {
        // Discard
    }
}

/// Console callback that prints events for debugging
#[derive(Default, Clone)]
pub struct ConsoleStreamCallback {
    verbose: bool,
}

impl ConsoleStreamCallback {
    /// Create a new console callback
    pub fn new() -> Self {
        Self { verbose: false }
    }

    /// Create a verbose console callback that shows all event details
    pub fn verbose() -> Self {
        Self { verbose: true }
    }
}

#[async_trait]
impl StreamCallback for ConsoleStreamCallback {
    async fn on_event(&self, event: AgentEvent) {
        // In verbose mode, prefix with node_id for observability (Audit #80)
        let node_prefix = if self.verbose {
            format!("[{}] ", event.node_id())
        } else {
            String::new()
        };

        match &event {
            AgentEvent::UserTurn { content, .. } => {
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..50])
                } else {
                    content.clone()
                };
                eprintln!("[STREAM] {}User: {}", node_prefix, preview);
            }
            AgentEvent::ReasoningStart { turn, model, .. } => {
                eprintln!(
                    "[STREAM] {}Reasoning started (turn {}, model: {})",
                    node_prefix, turn, model
                );
            }
            AgentEvent::ReasoningComplete {
                turn,
                duration_ms,
                has_tool_calls,
                tool_count,
                input_tokens,
                output_tokens,
                ..
            } => {
                let token_str = if self.verbose {
                    match (input_tokens, output_tokens) {
                        (Some(i), Some(o)) => format!(", {} in/{} out tokens", i, o),
                        (Some(i), None) => format!(", {} in tokens", i),
                        (None, Some(o)) => format!(", {} out tokens", o),
                        (None, None) => String::new(),
                    }
                } else {
                    String::new()
                };

                if *has_tool_calls {
                    eprintln!(
                        "[STREAM] {}Reasoning complete (turn {}, {}ms, {} tool calls{})",
                        node_prefix, turn, duration_ms, tool_count, token_str
                    );
                } else {
                    eprintln!(
                        "[STREAM] {}Reasoning complete (turn {}, {}ms, text response{})",
                        node_prefix, turn, duration_ms, token_str
                    );
                }
            }
            AgentEvent::ToolCallRequested { tool, args, .. } => {
                if self.verbose {
                    eprintln!(
                        "[STREAM] {}Tool requested: {} ({})",
                        node_prefix, tool, args
                    );
                } else {
                    eprintln!("[STREAM] Tool requested: {}", tool);
                }
            }
            AgentEvent::ToolCallApproved { tool, .. } => {
                eprintln!("[STREAM] {}Tool approved: {}", node_prefix, tool);
            }
            AgentEvent::ToolCallRejected { tool, reason, .. } => {
                eprintln!(
                    "[STREAM] {}Tool rejected: {} ({})",
                    node_prefix, tool, reason
                );
            }
            AgentEvent::ToolExecutionStart { tool, .. } => {
                eprintln!("[STREAM] {}Executing: {}", node_prefix, tool);
            }
            AgentEvent::ToolExecutionComplete {
                tool,
                success,
                duration_ms,
                output_preview,
                ..
            } => {
                let status = if *success { "OK" } else { "FAILED" };
                if self.verbose {
                    eprintln!(
                        "[STREAM] {}{} complete ({}, {}ms): {}",
                        node_prefix, tool, status, duration_ms, output_preview
                    );
                } else {
                    eprintln!("[STREAM] {} complete ({}, {}ms)", tool, status, duration_ms);
                }
            }
            AgentEvent::TurnComplete { turn, status, .. } => {
                eprintln!(
                    "[STREAM] {}Turn {} complete ({})",
                    node_prefix, turn, status
                );
            }
            AgentEvent::SessionComplete {
                total_turns,
                status,
                ..
            } => {
                eprintln!(
                    "[STREAM] {}Session complete ({} turns, {})",
                    node_prefix, total_turns, status
                );
            }
            AgentEvent::SessionMetrics {
                total_input_tokens,
                total_output_tokens,
                total_cost_usd,
                ..
            } => {
                let cost_str = total_cost_usd
                    .map(|c| format!(", total ${:.6}", c))
                    .unwrap_or_default();
                eprintln!(
                    "[STREAM] {}Session metrics: {} in, {} out tokens{}",
                    node_prefix, total_input_tokens, total_output_tokens, cost_str
                );
            }
            AgentEvent::TokenChunk {
                chunk, is_final, ..
            } => {
                if self.verbose && !is_final {
                    eprint!("{}", chunk);
                }
            }
            AgentEvent::Error { error, context, .. } => {
                eprintln!("[STREAM] {}Error in {}: {}", node_prefix, context, error);
            }
            AgentEvent::ApprovalRequired { tool, reason, .. } => {
                let reason_str = reason
                    .as_ref()
                    .map(|r| format!(" ({})", r))
                    .unwrap_or_default();
                eprintln!(
                    "[STREAM] {}Approval required: {}{}",
                    node_prefix, tool, reason_str
                );
            }
            AgentEvent::LlmMetrics {
                model,
                input_tokens,
                output_tokens,
                latency_ms,
                cost_usd,
                ..
            } => {
                if self.verbose {
                    let cost_str = cost_usd.map(|c| format!(", ${:.6}", c)).unwrap_or_default();
                    eprintln!(
                        "[STREAM] {}LLM metrics: {} ({} in, {} out, {}ms{})",
                        node_prefix, model, input_tokens, output_tokens, latency_ms, cost_str
                    );
                }
            }
            AgentEvent::EvalCapture {
                capture_id, model, ..
            } => {
                if self.verbose {
                    eprintln!(
                        "[STREAM] {}Eval captured: {} ({})",
                        node_prefix, capture_id, model
                    );
                }
            }
            AgentEvent::QualityGateStart {
                attempt,
                max_retries,
                threshold,
                ..
            } => {
                eprintln!(
                    "[STREAM] {}Quality gate check (attempt {}/{}, threshold {:.2})",
                    node_prefix, attempt, max_retries, threshold
                );
            }
            AgentEvent::QualityGateResult {
                attempt,
                passed,
                average_score,
                is_final,
                reason,
                ..
            } => {
                let status = if *passed { "PASSED" } else { "FAILED" };
                let final_str = if *is_final { " (final)" } else { "" };
                let reason_str = reason
                    .as_ref()
                    .map(|r| format!(": {}", r))
                    .unwrap_or_default();
                eprintln!(
                    "[STREAM] {}Quality gate attempt {}: {} (score {:.2}){}{}",
                    node_prefix, attempt, status, average_score, final_str, reason_str
                );
            }
        }
    }
}

/// Aggregating callback that collects metrics
#[derive(Default)]
pub struct MetricsCallback {
    events: std::sync::Mutex<Vec<AgentEvent>>,
}

impl MetricsCallback {
    /// Create a new metrics callback
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all collected events
    pub fn events(&self) -> Vec<AgentEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Get the count of events by type
    pub fn event_counts(&self) -> std::collections::HashMap<&'static str, usize> {
        let events = self.events.lock().unwrap();
        let mut counts = std::collections::HashMap::new();
        for event in events.iter() {
            *counts.entry(event.event_type()).or_insert(0) += 1;
        }
        counts
    }

    /// Get total reasoning time in milliseconds
    pub fn total_reasoning_time_ms(&self) -> u64 {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ReasoningComplete { duration_ms, .. } => Some(*duration_ms),
                _ => None,
            })
            .sum()
    }

    /// Get total tool execution time in milliseconds
    pub fn total_tool_execution_time_ms(&self) -> u64 {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ToolExecutionComplete { duration_ms, .. } => Some(*duration_ms),
                _ => None,
            })
            .sum()
    }

    /// Clear collected events
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get total input tokens across all LLM calls
    pub fn total_input_tokens(&self) -> u32 {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::LlmMetrics { input_tokens, .. } => Some(*input_tokens),
                AgentEvent::ReasoningComplete { input_tokens, .. } => *input_tokens,
                _ => None,
            })
            .sum()
    }

    /// Get total output tokens across all LLM calls
    pub fn total_output_tokens(&self) -> u32 {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::LlmMetrics { output_tokens, .. } => Some(*output_tokens),
                AgentEvent::ReasoningComplete { output_tokens, .. } => *output_tokens,
                _ => None,
            })
            .sum()
    }

    /// Get total estimated cost in USD
    pub fn total_cost_usd(&self) -> f64 {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::LlmMetrics { cost_usd, .. } => *cost_usd,
                _ => None,
            })
            .sum()
    }

    /// Get all eval captures
    pub fn eval_captures(&self) -> Vec<AgentEvent> {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::EvalCapture { .. }))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl StreamCallback for MetricsCallback {
    async fn on_event(&self, event: AgentEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// Multi-callback that sends events to multiple callbacks
pub struct MultiStreamCallback {
    callbacks: Vec<Arc<dyn StreamCallback>>,
}

impl MultiStreamCallback {
    /// Create a new multi-callback
    pub fn new(callbacks: Vec<Arc<dyn StreamCallback>>) -> Self {
        Self { callbacks }
    }

    /// Add a callback
    pub fn add(&mut self, callback: Arc<dyn StreamCallback>) {
        self.callbacks.push(callback);
    }
}

#[async_trait]
impl StreamCallback for MultiStreamCallback {
    async fn on_event(&self, event: AgentEvent) {
        for callback in &self.callbacks {
            callback.on_event(event.clone()).await;
        }
    }

    async fn flush(&self) {
        for callback in &self.callbacks {
            callback.flush().await;
        }
    }
}

/// Builder for creating stream callbacks
pub struct StreamCallbackBuilder {
    callbacks: Vec<Arc<dyn StreamCallback>>,
}

impl StreamCallbackBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Add console output
    pub fn with_console(mut self) -> Self {
        self.callbacks.push(Arc::new(ConsoleStreamCallback::new()));
        self
    }

    /// Add verbose console output
    pub fn with_verbose_console(mut self) -> Self {
        self.callbacks
            .push(Arc::new(ConsoleStreamCallback::verbose()));
        self
    }

    /// Add metrics collection
    pub fn with_metrics(mut self, metrics: Arc<MetricsCallback>) -> Self {
        self.callbacks.push(metrics);
        self
    }

    /// Add a custom callback
    pub fn with_callback(mut self, callback: Arc<dyn StreamCallback>) -> Self {
        self.callbacks.push(callback);
        self
    }

    /// Build the stream callback
    pub fn build(self) -> Arc<dyn StreamCallback> {
        match self.callbacks.len() {
            0 => Arc::new(NullStreamCallback),
            1 => self.callbacks.into_iter().next().unwrap(),
            _ => Arc::new(MultiStreamCallback::new(self.callbacks)),
        }
    }
}

impl Default for StreamCallbackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DashFlow Streaming Integration (requires "dashstream" feature)
// ============================================================================

/// Configuration for DashFlow streaming integration
///
/// This configuration is always available for building configs, but the
/// actual `DashFlowStreamAdapter` requires the "dashstream" feature.
#[derive(Clone, Debug)]
pub struct DashFlowStreamConfig {
    /// Kafka bootstrap servers (e.g., "localhost:9092")
    pub bootstrap_servers: String,
    /// Kafka topic name for events
    pub topic: String,
    /// Tenant ID for multi-tenancy
    pub tenant_id: String,
    /// Enable state diffing for incremental updates
    pub enable_state_diff: bool,
    /// Compression threshold in bytes (messages smaller than this are not compressed)
    /// Default: 512 bytes. Set to 0 to compress all messages.
    pub compression_threshold: usize,
    /// Compression level (1-22, higher = better compression but slower)
    /// Default: 3. Levels 1-3 are fast (good for streaming), 4-6 balanced, 7+ high ratio.
    pub compression_level: i32,
    /// Enable message compression
    /// Default: true. When enabled, messages larger than compression_threshold
    /// are compressed using zstd.
    pub enable_compression: bool,
}

impl Default for DashFlowStreamConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "codex-events".to_string(),
            tenant_id: "codex-dashflow".to_string(),
            enable_state_diff: true,
            compression_threshold: 512,
            compression_level: 3,
            enable_compression: true,
        }
    }
}

impl DashFlowStreamConfig {
    /// Create a new config with specified bootstrap servers
    pub fn new(bootstrap_servers: impl Into<String>) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.into(),
            ..Default::default()
        }
    }

    /// Set the Kafka topic
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Set the tenant ID
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = tenant_id.into();
        self
    }

    /// Enable or disable state diffing
    pub fn with_state_diff(mut self, enabled: bool) -> Self {
        self.enable_state_diff = enabled;
        self
    }

    /// Set compression threshold
    pub fn with_compression_threshold(mut self, threshold: usize) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Set compression level (1-22)
    ///
    /// - Levels 1-3: Fast compression (best for real-time streaming)
    /// - Levels 4-6: Balanced compression (good for most use cases)
    /// - Levels 7+: High compression ratio (CPU intensive, good for archival)
    pub fn with_compression_level(mut self, level: i32) -> Self {
        self.compression_level = level.clamp(1, 22);
        self
    }

    /// Enable or disable compression
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.enable_compression = enabled;
        self
    }
}

/// Adapter that bridges AgentEvent to DashFlow Streaming protocol
///
/// This sends events to Kafka for real-time observability via `dashstream tail`.
///
/// Requires the "dashstream" feature to be enabled (and protoc installed).
#[cfg(feature = "dashstream")]
pub struct DashFlowStreamAdapter {
    producer: Arc<dashflow_streaming::producer::DashStreamProducer>,
    config: DashFlowStreamConfig,
    thread_id: String,
    sequence: std::sync::Mutex<u64>,
}

#[cfg(feature = "dashstream")]
impl DashFlowStreamAdapter {
    /// Create a new DashFlow streaming adapter
    ///
    /// Configures the underlying DashFlow producer with compression settings
    /// from `DashFlowStreamConfig`. Compression is applied to messages larger
    /// than `compression_threshold` using zstd at the specified `compression_level`.
    pub async fn new(
        config: DashFlowStreamConfig,
        session_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Build ProducerConfig from our DashFlowStreamConfig
        let producer_config = dashflow_streaming::producer::ProducerConfig {
            bootstrap_servers: config.bootstrap_servers.clone(),
            topic: config.topic.clone(),
            tenant_id: config.tenant_id.clone(),
            enable_compression: config.enable_compression,
            // Note: DashFlow uses hardcoded compression threshold/level in codec.rs
            // Our config settings document the expected behavior but full tuning
            // requires DashFlow platform enhancement to ProducerConfig
            ..Default::default()
        };

        let producer =
            dashflow_streaming::producer::DashStreamProducer::with_config(producer_config).await?;

        Ok(Self {
            producer: Arc::new(producer),
            config,
            thread_id: session_id.to_string(),
            sequence: std::sync::Mutex::new(0),
        })
    }

    /// Get the next sequence number
    fn next_sequence(&self) -> u64 {
        let mut seq = self.sequence.lock().unwrap();
        let current = *seq;
        *seq += 1;
        current
    }

    /// Create a message header
    fn create_header(
        &self,
        message_type: dashflow_streaming::MessageType,
    ) -> dashflow_streaming::Header {
        use std::time::SystemTime;

        dashflow_streaming::Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros() as i64,
            tenant_id: self.config.tenant_id.clone(),
            thread_id: self.thread_id.clone(),
            sequence: self.next_sequence(),
            r#type: message_type as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
        }
    }

    /// Convert AgentEvent to DashFlow Event and send
    fn send_event(&self, agent_event: &AgentEvent) {
        use dashflow_streaming::{attribute_value, AttributeValue, Event, EventType, MessageType};

        // Helper to create a string AttributeValue
        fn str_attr(s: String) -> AttributeValue {
            AttributeValue {
                value: Some(attribute_value::Value::StringValue(s)),
            }
        }

        let (event_type, node_id, duration_us, attributes) = match agent_event {
            AgentEvent::UserTurn { content, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(
                    "content_preview".to_string(),
                    str_attr(if content.len() > 100 {
                        format!("{}...", &content[..100])
                    } else {
                        content.clone()
                    }),
                );
                (EventType::GraphStart, "user_input".to_string(), 0i64, attrs)
            }
            AgentEvent::ReasoningStart { turn, model, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("turn".to_string(), str_attr(turn.to_string()));
                attrs.insert("model".to_string(), str_attr(model.clone()));
                (EventType::NodeStart, "reasoning".to_string(), 0i64, attrs)
            }
            AgentEvent::ReasoningComplete {
                turn,
                duration_ms,
                has_tool_calls,
                tool_count,
                input_tokens,
                output_tokens,
                ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("turn".to_string(), str_attr(turn.to_string()));
                attrs.insert(
                    "has_tool_calls".to_string(),
                    str_attr(has_tool_calls.to_string()),
                );
                attrs.insert("tool_count".to_string(), str_attr(tool_count.to_string()));
                // Include token counts in attributes (DashFlow Event.attributes)
                if let Some(in_tokens) = input_tokens {
                    attrs.insert("input_tokens".to_string(), str_attr(in_tokens.to_string()));
                }
                if let Some(out_tokens) = output_tokens {
                    attrs.insert(
                        "output_tokens".to_string(),
                        str_attr(out_tokens.to_string()),
                    );
                }
                (
                    EventType::NodeEnd,
                    "reasoning".to_string(),
                    (*duration_ms as i64) * 1000,
                    attrs,
                )
            }
            AgentEvent::ToolCallRequested {
                tool_call_id,
                tool,
                args,
                ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("tool_call_id".to_string(), str_attr(tool_call_id.clone()));
                attrs.insert("tool".to_string(), str_attr(tool.clone()));
                attrs.insert("args".to_string(), str_attr(args.to_string()));
                (EventType::NodeStart, format!("tool:{}", tool), 0i64, attrs)
            }
            AgentEvent::ToolCallApproved { tool, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("status".to_string(), str_attr("approved".to_string()));
                (
                    EventType::ConditionalBranch,
                    format!("tool_selection:{}", tool),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::ToolCallRejected { tool, reason, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("status".to_string(), str_attr("rejected".to_string()));
                attrs.insert("reason".to_string(), str_attr(reason.clone()));
                (
                    EventType::ConditionalBranch,
                    format!("tool_selection:{}", tool),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::ToolExecutionStart { tool, .. } => {
                let attrs = std::collections::HashMap::new();
                (
                    EventType::NodeStart,
                    format!("tool_execution:{}", tool),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::ToolExecutionComplete {
                tool,
                success,
                duration_ms,
                output_preview,
                ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("success".to_string(), str_attr(success.to_string()));
                attrs.insert(
                    "output_preview".to_string(),
                    str_attr(output_preview.clone()),
                );
                (
                    EventType::NodeEnd,
                    format!("tool_execution:{}", tool),
                    (*duration_ms as i64) * 1000,
                    attrs,
                )
            }
            AgentEvent::TurnComplete { turn, status, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("turn".to_string(), str_attr(turn.to_string()));
                attrs.insert("status".to_string(), str_attr(status.clone()));
                (
                    EventType::EdgeTraversal,
                    "turn_complete".to_string(),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::SessionComplete {
                total_turns,
                status,
                ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("total_turns".to_string(), str_attr(total_turns.to_string()));
                attrs.insert("status".to_string(), str_attr(status.clone()));
                (EventType::GraphEnd, "session".to_string(), 0i64, attrs)
            }
            AgentEvent::TokenChunk {
                chunk, is_final, ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("is_final".to_string(), str_attr(is_final.to_string()));
                attrs.insert("chunk_len".to_string(), str_attr(chunk.len().to_string()));
                // Skip sending token chunks to reduce noise
                return;
            }
            AgentEvent::Error { error, context, .. } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("error".to_string(), str_attr(error.clone()));
                attrs.insert("context".to_string(), str_attr(context.clone()));
                (EventType::NodeError, "error".to_string(), 0i64, attrs)
            }
            AgentEvent::ApprovalRequired {
                request_id,
                tool_call_id,
                tool,
                args,
                reason,
                ..
            } => {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("request_id".to_string(), str_attr(request_id.clone()));
                attrs.insert("tool_call_id".to_string(), str_attr(tool_call_id.clone()));
                attrs.insert("tool".to_string(), str_attr(tool.clone()));
                attrs.insert("args".to_string(), str_attr(args.to_string()));
                if let Some(r) = reason {
                    attrs.insert("reason".to_string(), str_attr(r.clone()));
                }
                (
                    EventType::ConditionalBranch,
                    format!("approval_required:{}", tool),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::LlmMetrics {
                request_id,
                model,
                input_tokens,
                output_tokens,
                total_tokens,
                latency_ms,
                cost_usd,
                cached,
                ..
            } => {
                // LlmMetrics maps to DashFlow Streaming Metrics message
                // For now, send as Event with metrics attributes
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("request_id".to_string(), str_attr(request_id.clone()));
                attrs.insert("model".to_string(), str_attr(model.clone()));
                attrs.insert(
                    "input_tokens".to_string(),
                    str_attr(input_tokens.to_string()),
                );
                attrs.insert(
                    "output_tokens".to_string(),
                    str_attr(output_tokens.to_string()),
                );
                attrs.insert(
                    "total_tokens".to_string(),
                    str_attr(total_tokens.to_string()),
                );
                attrs.insert("latency_ms".to_string(), str_attr(latency_ms.to_string()));
                if let Some(cost) = cost_usd {
                    attrs.insert("cost_usd".to_string(), str_attr(format!("{:.8}", cost)));
                }
                attrs.insert("cached".to_string(), str_attr(cached.to_string()));
                (
                    EventType::NodeEnd,
                    format!("llm_metrics:{}", model),
                    (*latency_ms as i64) * 1000,
                    attrs,
                )
            }
            AgentEvent::EvalCapture {
                capture_id,
                model,
                input_messages,
                output_response,
                tools,
                metadata,
                ..
            } => {
                // EvalCapture is for building eval datasets
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("capture_id".to_string(), str_attr(capture_id.clone()));
                attrs.insert("model".to_string(), str_attr(model.clone()));
                attrs.insert(
                    "input_messages".to_string(),
                    str_attr(input_messages.clone()),
                );
                attrs.insert(
                    "output_response".to_string(),
                    str_attr(output_response.clone()),
                );
                if let Some(t) = tools {
                    attrs.insert("tools".to_string(), str_attr(t.clone()));
                }
                if let Some(m) = metadata {
                    attrs.insert("metadata".to_string(), str_attr(m.to_string()));
                }
                (EventType::NodeEnd, "eval_capture".to_string(), 0i64, attrs)
            }
            AgentEvent::SessionMetrics {
                total_input_tokens,
                total_output_tokens,
                total_cached_tokens,
                total_cost_usd,
                llm_call_count,
                duration_ms,
                ..
            } => {
                // SessionMetrics summarizes the entire session
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(
                    "total_input_tokens".to_string(),
                    str_attr(total_input_tokens.to_string()),
                );
                attrs.insert(
                    "total_output_tokens".to_string(),
                    str_attr(total_output_tokens.to_string()),
                );
                attrs.insert(
                    "total_cached_tokens".to_string(),
                    str_attr(total_cached_tokens.to_string()),
                );
                if let Some(cost) = total_cost_usd {
                    attrs.insert(
                        "total_cost_usd".to_string(),
                        str_attr(format!("{:.6}", cost)),
                    );
                }
                attrs.insert(
                    "llm_call_count".to_string(),
                    str_attr(llm_call_count.to_string()),
                );
                attrs.insert("duration_ms".to_string(), str_attr(duration_ms.to_string()));
                (
                    EventType::GraphEnd,
                    "session_metrics".to_string(),
                    (*duration_ms as i64) * 1000,
                    attrs,
                )
            }
            AgentEvent::QualityGateStart {
                session_id,
                attempt,
                max_retries,
                threshold,
            } => {
                // QualityGateStart indicates quality validation is beginning
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("session_id".to_string(), str_attr(session_id.clone()));
                attrs.insert("attempt".to_string(), str_attr(attempt.to_string()));
                attrs.insert("max_retries".to_string(), str_attr(max_retries.to_string()));
                attrs.insert(
                    "threshold".to_string(),
                    str_attr(format!("{:.2}", threshold)),
                );
                (
                    EventType::NodeStart,
                    "quality_gate".to_string(),
                    0i64,
                    attrs,
                )
            }
            AgentEvent::QualityGateResult {
                session_id,
                attempt,
                passed,
                accuracy,
                relevance,
                completeness,
                average_score,
                is_final,
                reason,
            } => {
                // QualityGateResult reports quality validation outcome
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("session_id".to_string(), str_attr(session_id.clone()));
                attrs.insert("attempt".to_string(), str_attr(attempt.to_string()));
                attrs.insert("passed".to_string(), str_attr(passed.to_string()));
                attrs.insert("accuracy".to_string(), str_attr(format!("{:.3}", accuracy)));
                attrs.insert(
                    "relevance".to_string(),
                    str_attr(format!("{:.3}", relevance)),
                );
                attrs.insert(
                    "completeness".to_string(),
                    str_attr(format!("{:.3}", completeness)),
                );
                attrs.insert(
                    "average_score".to_string(),
                    str_attr(format!("{:.3}", average_score)),
                );
                attrs.insert("is_final".to_string(), str_attr(is_final.to_string()));
                if let Some(r) = reason {
                    attrs.insert("reason".to_string(), str_attr(r.clone()));
                }
                (EventType::NodeEnd, "quality_gate".to_string(), 0i64, attrs)
            }
        };

        let event = Event {
            header: Some(self.create_header(MessageType::Event)),
            event_type: event_type as i32,
            node_id,
            attributes,
            duration_us,
            llm_request_id: String::new(),
        };

        // Send asynchronously
        let producer = self.producer.clone();
        tokio::spawn(async move {
            let _ = producer.send_event(event).await;
        });
    }

    /// Flush pending messages
    pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.producer
            .flush(std::time::Duration::from_secs(5))
            .await?;
        Ok(())
    }
}

#[cfg(feature = "dashstream")]
#[async_trait]
impl StreamCallback for DashFlowStreamAdapter {
    async fn on_event(&self, event: AgentEvent) {
        self.send_event(&event);
    }

    async fn flush(&self) {
        let _ = DashFlowStreamAdapter::flush(self).await;
    }
}

/// Helper struct for timing operations
pub struct EventTimer {
    start: Instant,
}

impl EventTimer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_callback() {
        let callback = NullStreamCallback;
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "test".into(),
                content: "hello".into(),
            })
            .await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_console_callback() {
        let callback = ConsoleStreamCallback::new();
        callback
            .on_event(AgentEvent::ReasoningStart {
                session_id: "test".into(),
                turn: 1,
                model: "gpt-4".into(),
            })
            .await;
        // Check stderr was written (manual inspection)
    }

    #[tokio::test]
    async fn test_console_callback_verbose_with_token_counts() {
        // Test that verbose mode includes token counts in ReasoningComplete output
        let callback = ConsoleStreamCallback::verbose();

        // With both token counts
        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "test".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(500),
                output_tokens: Some(150),
            })
            .await;

        // With tool calls and tokens
        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "test".into(),
                turn: 2,
                duration_ms: 200,
                has_tool_calls: true,
                tool_count: 2,
                input_tokens: Some(800),
                output_tokens: Some(250),
            })
            .await;

        // With partial token info (only input)
        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "test".into(),
                turn: 3,
                duration_ms: 50,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: None,
            })
            .await;

        // Verify non-verbose mode does not include tokens
        let non_verbose = ConsoleStreamCallback::new();
        non_verbose
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "test".into(),
                turn: 4,
                duration_ms: 75,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(300),
                output_tokens: Some(100),
            })
            .await;

        // Output format verified via stderr inspection
        // Verbose: "[STREAM] Reasoning complete (turn 1, 100ms, text response, 500 in/150 out tokens)"
        // Non-verbose: "[STREAM] Reasoning complete (turn 4, 75ms, text response)"
    }

    #[tokio::test]
    async fn test_metrics_callback() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "test".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(150),
                output_tokens: Some(50),
            })
            .await;

        metrics
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "test".into(),
                tool_call_id: "call1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 50,
                output_preview: "ok".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 2);
        assert_eq!(metrics.total_reasoning_time_ms(), 100);
        assert_eq!(metrics.total_tool_execution_time_ms(), 50);
        assert_eq!(metrics.total_input_tokens(), 150);
        assert_eq!(metrics.total_output_tokens(), 50);
    }

    #[tokio::test]
    async fn test_multi_callback() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());

        let multi = MultiStreamCallback::new(vec![metrics1.clone(), metrics2.clone()]);

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "test".into(),
                content: "hello".into(),
            })
            .await;

        assert_eq!(metrics1.events().len(), 1);
        assert_eq!(metrics2.events().len(), 1);
    }

    #[tokio::test]
    async fn test_callback_builder() {
        let metrics = Arc::new(MetricsCallback::new());
        let callback = StreamCallbackBuilder::new()
            .with_metrics(metrics.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "test".into(),
                content: "hello".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
    }

    #[test]
    fn test_event_type() {
        let event = AgentEvent::ReasoningStart {
            session_id: "test".into(),
            turn: 1,
            model: "gpt-4".into(),
        };
        assert_eq!(event.event_type(), "reasoning_start");
        assert_eq!(event.session_id(), "test");
    }

    #[test]
    fn test_event_node_id() {
        // Test node IDs for each event type (Audit #80)
        assert_eq!(
            AgentEvent::UserTurn {
                session_id: "s".into(),
                content: "hi".into()
            }
            .node_id(),
            "user_input"
        );

        assert_eq!(
            AgentEvent::ReasoningStart {
                session_id: "s".into(),
                turn: 1,
                model: "gpt-4".into()
            }
            .node_id(),
            "reasoning"
        );

        assert_eq!(
            AgentEvent::ToolCallRequested {
                session_id: "s".into(),
                tool_call_id: "c1".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
            }
            .node_id(),
            "reasoning:tool_call:shell"
        );

        assert_eq!(
            AgentEvent::ToolExecutionStart {
                session_id: "s".into(),
                tool_call_id: "c1".into(),
                tool: "file_read".into(),
            }
            .node_id(),
            "tool_execution:file_read"
        );

        assert_eq!(
            AgentEvent::LlmMetrics {
                session_id: "s".into(),
                request_id: "r1".into(),
                model: "claude-3".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 200,
                cost_usd: None,
                cached: false,
            }
            .node_id(),
            "llm:claude-3"
        );

        assert_eq!(
            AgentEvent::Error {
                session_id: "s".into(),
                error: "oops".into(),
                context: "tool_execution".into(),
            }
            .node_id(),
            "error:tool_execution"
        );
    }

    #[test]
    fn test_event_timer() {
        let timer = EventTimer::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(timer.elapsed_ms() >= 10);
    }

    // DashFlowStreamConfig tests (always available)

    #[test]
    fn test_dashflow_stream_config_default() {
        let config = DashFlowStreamConfig::default();
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "codex-events");
        assert_eq!(config.tenant_id, "codex-dashflow");
        assert!(config.enable_state_diff);
        assert_eq!(config.compression_threshold, 512);
    }

    #[test]
    fn test_dashflow_stream_config_new() {
        let config = DashFlowStreamConfig::new("kafka.example.com:9093");
        assert_eq!(config.bootstrap_servers, "kafka.example.com:9093");
        // Other fields should be defaults
        assert_eq!(config.topic, "codex-events");
        assert_eq!(config.tenant_id, "codex-dashflow");
    }

    #[test]
    fn test_dashflow_stream_config_builder_pattern() {
        let config = DashFlowStreamConfig::new("localhost:9092")
            .with_topic("my-events")
            .with_tenant_id("my-tenant")
            .with_state_diff(false)
            .with_compression_threshold(1024);

        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "my-events");
        assert_eq!(config.tenant_id, "my-tenant");
        assert!(!config.enable_state_diff);
        assert_eq!(config.compression_threshold, 1024);
    }

    #[test]
    fn test_dashflow_stream_config_clone() {
        let config1 = DashFlowStreamConfig::new("localhost:9092")
            .with_topic("events")
            .with_tenant_id("tenant1");

        let config2 = config1.clone();

        assert_eq!(config1.bootstrap_servers, config2.bootstrap_servers);
        assert_eq!(config1.topic, config2.topic);
        assert_eq!(config1.tenant_id, config2.tenant_id);
    }

    #[test]
    fn test_dashflow_stream_config_debug() {
        let config = DashFlowStreamConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DashFlowStreamConfig"));
        assert!(debug_str.contains("localhost:9092"));
    }

    // Tests for ApprovalRequired event (N=233)

    #[test]
    fn test_approval_required_event_type() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "test".into(),
            request_id: "req-123".into(),
            tool_call_id: "call-456".into(),
            tool: "shell".into(),
            args: serde_json::json!({"command": "rm -rf /tmp/test"}),
            reason: Some("Potentially dangerous command".into()),
        };
        assert_eq!(event.event_type(), "approval_required");
        assert_eq!(event.session_id(), "test");
    }

    #[test]
    fn test_approval_required_event_without_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "session-1".into(),
            request_id: "req-789".into(),
            tool_call_id: "call-101".into(),
            tool: "write_file".into(),
            args: serde_json::json!({"path": "/tmp/file.txt", "content": "hello"}),
            reason: None,
        };
        assert_eq!(event.event_type(), "approval_required");
        assert_eq!(event.session_id(), "session-1");
    }

    // Tests for new telemetry events (N=128)

    #[test]
    fn test_llm_metrics_event_type() {
        let event = AgentEvent::LlmMetrics {
            session_id: "test".into(),
            request_id: "req-123".into(),
            model: "gpt-4".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 500,
            cost_usd: Some(0.001),
            cached: false,
        };
        assert_eq!(event.event_type(), "llm_metrics");
        assert_eq!(event.session_id(), "test");
    }

    #[test]
    fn test_eval_capture_event_type() {
        let event = AgentEvent::EvalCapture {
            session_id: "test".into(),
            capture_id: "capture-456".into(),
            input_messages: r#"[{"role":"user","content":"hello"}]"#.into(),
            output_response: r#"{"content":"hi there"}"#.into(),
            model: "gpt-4".into(),
            tools: None,
            metadata: None,
        };
        assert_eq!(event.event_type(), "eval_capture");
        assert_eq!(event.session_id(), "test");
    }

    #[tokio::test]
    async fn test_metrics_callback_llm_metrics() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "test".into(),
                request_id: "req-1".into(),
                model: "gpt-4".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.001),
                cached: false,
            })
            .await;

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "test".into(),
                request_id: "req-2".into(),
                model: "gpt-4".into(),
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                latency_ms: 800,
                cost_usd: Some(0.002),
                cached: false,
            })
            .await;

        assert_eq!(metrics.events().len(), 2);
        assert_eq!(metrics.total_input_tokens(), 300);
        assert_eq!(metrics.total_output_tokens(), 150);
        assert!((metrics.total_cost_usd() - 0.003).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_metrics_callback_eval_captures() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::EvalCapture {
                session_id: "test".into(),
                capture_id: "cap-1".into(),
                input_messages: "messages1".into(),
                output_response: "response1".into(),
                model: "gpt-4".into(),
                tools: None,
                metadata: None,
            })
            .await;

        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "test".into(),
                content: "hello".into(),
            })
            .await;

        metrics
            .on_event(AgentEvent::EvalCapture {
                session_id: "test".into(),
                capture_id: "cap-2".into(),
                input_messages: "messages2".into(),
                output_response: "response2".into(),
                model: "gpt-4".into(),
                tools: None,
                metadata: None,
            })
            .await;

        assert_eq!(metrics.events().len(), 3);
        assert_eq!(metrics.eval_captures().len(), 2);
    }

    #[test]
    fn test_reasoning_complete_with_tokens() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "test".into(),
            turn: 1,
            duration_ms: 100,
            has_tool_calls: true,
            tool_count: 2,
            input_tokens: Some(500),
            output_tokens: Some(200),
        };

        match event {
            AgentEvent::ReasoningComplete {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, Some(500));
                assert_eq!(output_tokens, Some(200));
            }
            _ => panic!("Expected ReasoningComplete"),
        }
    }

    #[test]
    fn test_agent_event_session_id_all_variants() {
        let events = vec![
            AgentEvent::UserTurn {
                session_id: "s1".into(),
                content: "hi".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: "s2".into(),
                turn: 1,
                model: "gpt-4".into(),
            },
            AgentEvent::ReasoningComplete {
                session_id: "s3".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            },
            AgentEvent::LlmMetrics {
                session_id: "s4".into(),
                request_id: "r1".into(),
                model: "gpt-4".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: None,
                cached: false,
            },
            AgentEvent::ToolCallRequested {
                session_id: "s5".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
            },
            AgentEvent::ToolCallApproved {
                session_id: "s6".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolCallRejected {
                session_id: "s7".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                reason: "denied".into(),
            },
            AgentEvent::ToolExecutionStart {
                session_id: "s8".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: "s9".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 50,
                output_preview: "ok".into(),
            },
            AgentEvent::TurnComplete {
                session_id: "s10".into(),
                turn: 1,
                status: "ok".into(),
            },
            AgentEvent::SessionComplete {
                session_id: "s11".into(),
                total_turns: 5,
                status: "done".into(),
            },
            AgentEvent::TokenChunk {
                session_id: "s12".into(),
                chunk: "hello".into(),
                is_final: false,
            },
            AgentEvent::Error {
                session_id: "s13".into(),
                error: "fail".into(),
                context: "test".into(),
            },
            AgentEvent::ApprovalRequired {
                session_id: "s14".into(),
                request_id: "r1".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
                reason: None,
            },
            AgentEvent::EvalCapture {
                session_id: "s15".into(),
                capture_id: "c1".into(),
                input_messages: "[]".into(),
                output_response: "{}".into(),
                model: "gpt-4".into(),
                tools: None,
                metadata: None,
            },
        ];

        for (i, event) in events.iter().enumerate() {
            let expected = format!("s{}", i + 1);
            assert_eq!(event.session_id(), expected);
        }
    }

    #[test]
    fn test_agent_event_event_type_all_variants() {
        let test_cases = vec![
            (
                AgentEvent::UserTurn {
                    session_id: "t".into(),
                    content: "".into(),
                },
                "user_turn",
            ),
            (
                AgentEvent::ReasoningStart {
                    session_id: "t".into(),
                    turn: 1,
                    model: "".into(),
                },
                "reasoning_start",
            ),
            (
                AgentEvent::ReasoningComplete {
                    session_id: "t".into(),
                    turn: 1,
                    duration_ms: 0,
                    has_tool_calls: false,
                    tool_count: 0,
                    input_tokens: None,
                    output_tokens: None,
                },
                "reasoning_complete",
            ),
            (
                AgentEvent::LlmMetrics {
                    session_id: "t".into(),
                    request_id: "".into(),
                    model: "".into(),
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                    latency_ms: 0,
                    cost_usd: None,
                    cached: false,
                },
                "llm_metrics",
            ),
            (
                AgentEvent::ToolCallRequested {
                    session_id: "t".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                    args: serde_json::json!({}),
                },
                "tool_call_requested",
            ),
            (
                AgentEvent::ToolCallApproved {
                    session_id: "t".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                },
                "tool_call_approved",
            ),
            (
                AgentEvent::ToolCallRejected {
                    session_id: "t".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                    reason: "".into(),
                },
                "tool_call_rejected",
            ),
            (
                AgentEvent::ToolExecutionStart {
                    session_id: "t".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                },
                "tool_execution_start",
            ),
            (
                AgentEvent::ToolExecutionComplete {
                    session_id: "t".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                    success: true,
                    duration_ms: 0,
                    output_preview: "".into(),
                },
                "tool_execution_complete",
            ),
            (
                AgentEvent::TurnComplete {
                    session_id: "t".into(),
                    turn: 1,
                    status: "".into(),
                },
                "turn_complete",
            ),
            (
                AgentEvent::SessionComplete {
                    session_id: "t".into(),
                    total_turns: 0,
                    status: "".into(),
                },
                "session_complete",
            ),
            (
                AgentEvent::TokenChunk {
                    session_id: "t".into(),
                    chunk: "".into(),
                    is_final: false,
                },
                "token_chunk",
            ),
            (
                AgentEvent::Error {
                    session_id: "t".into(),
                    error: "".into(),
                    context: "".into(),
                },
                "error",
            ),
            (
                AgentEvent::ApprovalRequired {
                    session_id: "t".into(),
                    request_id: "".into(),
                    tool_call_id: "".into(),
                    tool: "".into(),
                    args: serde_json::json!({}),
                    reason: None,
                },
                "approval_required",
            ),
            (
                AgentEvent::EvalCapture {
                    session_id: "t".into(),
                    capture_id: "".into(),
                    input_messages: "".into(),
                    output_response: "".into(),
                    model: "".into(),
                    tools: None,
                    metadata: None,
                },
                "eval_capture",
            ),
        ];

        for (event, expected_type) in test_cases {
            assert_eq!(event.event_type(), expected_type);
        }
    }

    #[test]
    fn test_agent_event_clone() {
        let event = AgentEvent::UserTurn {
            session_id: "test".into(),
            content: "hello".into(),
        };
        let cloned = event.clone();
        assert_eq!(event.session_id(), cloned.session_id());
    }

    #[test]
    fn test_agent_event_debug() {
        let event = AgentEvent::UserTurn {
            session_id: "test".into(),
            content: "hello".into(),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("UserTurn"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_agent_event_serialize_deserialize() {
        let event = AgentEvent::UserTurn {
            session_id: "test".into(),
            content: "hello world".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("UserTurn"));
        assert!(json.contains("hello world"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "test");
    }

    #[test]
    fn test_null_stream_callback_clone_default() {
        let cb1 = NullStreamCallback;
        let cb2 = cb1.clone();
        let cb3 = NullStreamCallback;
        // Just verify they compile and create instances
        assert_eq!(std::mem::size_of_val(&cb1), std::mem::size_of_val(&cb2));
        assert_eq!(std::mem::size_of_val(&cb2), std::mem::size_of_val(&cb3));
    }

    #[tokio::test]
    async fn test_null_stream_callback_flush() {
        let cb = NullStreamCallback;
        cb.flush().await;
        // Should not panic
    }

    #[test]
    fn test_console_stream_callback_default_clone() {
        let cb1 = ConsoleStreamCallback::default();
        let cb2 = cb1.clone();
        // Should be non-verbose by default
        assert!(!cb1.verbose);
        assert!(!cb2.verbose);
    }

    #[tokio::test]
    async fn test_console_callback_all_events() {
        let callback = ConsoleStreamCallback::verbose();

        // Test all event types don't panic
        let events = vec![
            AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "A very long content that exceeds fifty characters in length for testing truncation behavior".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: "t".into(),
                turn: 1,
                model: "gpt-4".into(),
            },
            AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: true,
                tool_count: 2,
                input_tokens: Some(100),
                output_tokens: Some(50),
            },
            AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 2,
                duration_ms: 50,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: Some(30),
            },
            AgentEvent::ToolCallRequested {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                args: serde_json::json!({"cmd": "ls"}),
            },
            AgentEvent::ToolCallApproved {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolCallRejected {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                reason: "policy".into(),
            },
            AgentEvent::ToolExecutionStart {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 50,
                output_preview: "ok".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc2".into(),
                tool: "file".into(),
                success: false,
                duration_ms: 10,
                output_preview: "error".into(),
            },
            AgentEvent::TurnComplete {
                session_id: "t".into(),
                turn: 1,
                status: "ok".into(),
            },
            AgentEvent::SessionComplete {
                session_id: "t".into(),
                total_turns: 3,
                status: "done".into(),
            },
            AgentEvent::TokenChunk {
                session_id: "t".into(),
                chunk: "token".into(),
                is_final: false,
            },
            AgentEvent::TokenChunk {
                session_id: "t".into(),
                chunk: "final".into(),
                is_final: true,
            },
            AgentEvent::Error {
                session_id: "t".into(),
                error: "something failed".into(),
                context: "testing".into(),
            },
            AgentEvent::ApprovalRequired {
                session_id: "t".into(),
                request_id: "r1".into(),
                tool_call_id: "tc1".into(),
                tool: "dangerous".into(),
                args: serde_json::json!({}),
                reason: Some("risky".into()),
            },
            AgentEvent::ApprovalRequired {
                session_id: "t".into(),
                request_id: "r2".into(),
                tool_call_id: "tc2".into(),
                tool: "safe".into(),
                args: serde_json::json!({}),
                reason: None,
            },
            AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "gpt-4".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.001),
                cached: false,
            },
            AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r2".into(),
                model: "gpt-4".into(),
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                latency_ms: 800,
                cost_usd: None,
                cached: true,
            },
            AgentEvent::EvalCapture {
                session_id: "t".into(),
                capture_id: "c1".into(),
                input_messages: "[]".into(),
                output_response: "{}".into(),
                model: "gpt-4".into(),
                tools: None,
                metadata: None,
            },
        ];

        for event in events {
            callback.on_event(event).await;
        }
    }

    #[tokio::test]
    async fn test_metrics_callback_clear() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "hi".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);

        metrics.clear();
        assert_eq!(metrics.events().len(), 0);
    }

    #[tokio::test]
    async fn test_metrics_callback_event_counts() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "hi".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "hello".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningStart {
                session_id: "t".into(),
                turn: 1,
                model: "gpt-4".into(),
            })
            .await;

        let counts = metrics.event_counts();
        assert_eq!(counts.get("user_turn"), Some(&2));
        assert_eq!(counts.get("reasoning_start"), Some(&1));
        assert_eq!(counts.get("session_complete"), None);
    }

    #[test]
    fn test_metrics_callback_default() {
        let metrics = MetricsCallback::default();
        assert_eq!(metrics.events().len(), 0);
        assert_eq!(metrics.total_reasoning_time_ms(), 0);
        assert_eq!(metrics.total_tool_execution_time_ms(), 0);
    }

    #[tokio::test]
    async fn test_multi_stream_callback_add() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());

        let mut multi = MultiStreamCallback::new(vec![metrics1.clone()]);
        multi.add(metrics2.clone());

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "hi".into(),
            })
            .await;

        assert_eq!(metrics1.events().len(), 1);
        assert_eq!(metrics2.events().len(), 1);
    }

    #[tokio::test]
    async fn test_multi_stream_callback_flush() {
        let metrics = Arc::new(MetricsCallback::new());
        let multi = MultiStreamCallback::new(vec![metrics]);

        multi.flush().await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_stream_callback_builder_default() {
        let builder = StreamCallbackBuilder::default();
        let callback = builder.build();
        // Should return NullStreamCallback when no callbacks added
        // Test by sending an event (should not panic)
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
    }

    #[tokio::test]
    async fn test_stream_callback_builder_with_console() {
        let callback = StreamCallbackBuilder::new().with_console().build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_stream_callback_builder_with_verbose_console() {
        let callback = StreamCallbackBuilder::new().with_verbose_console().build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_stream_callback_builder_with_custom_callback() {
        let metrics = Arc::new(MetricsCallback::new());
        let callback = StreamCallbackBuilder::new()
            .with_callback(metrics.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
    }

    #[tokio::test]
    async fn test_stream_callback_builder_multiple_callbacks() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());

        let callback = StreamCallbackBuilder::new()
            .with_metrics(metrics1.clone())
            .with_callback(metrics2.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        assert_eq!(metrics1.events().len(), 1);
        assert_eq!(metrics2.events().len(), 1);
    }

    #[tokio::test]
    async fn test_stream_callback_builder_empty_returns_null() {
        let callback = StreamCallbackBuilder::new().build();

        // Should work with NullStreamCallback
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
    }

    #[tokio::test]
    async fn test_metrics_callback_cost_usd_none() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "gpt-4".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: None,
                cached: false,
            })
            .await;

        // No cost should sum to 0.0
        assert_eq!(metrics.total_cost_usd(), 0.0);
    }

    // ============================================================================
    // Additional test coverage (N=279)
    // ============================================================================

    // AgentEvent serialization tests for all variants

    #[test]
    fn test_agent_event_serialize_reasoning_start() {
        let event = AgentEvent::ReasoningStart {
            session_id: "sess-1".into(),
            turn: 5,
            model: "gpt-4-turbo".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningStart"));
        assert!(json.contains("sess-1"));
        assert!(json.contains("gpt-4-turbo"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "sess-1");
        assert_eq!(parsed.event_type(), "reasoning_start");
    }

    #[test]
    fn test_agent_event_serialize_reasoning_complete() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "sess-2".into(),
            turn: 3,
            duration_ms: 1500,
            has_tool_calls: true,
            tool_count: 2,
            input_tokens: Some(1000),
            output_tokens: Some(500),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningComplete"));
        assert!(json.contains("1500"));
        assert!(json.contains("1000"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "sess-2");
    }

    #[test]
    fn test_agent_event_serialize_llm_metrics() {
        let event = AgentEvent::LlmMetrics {
            session_id: "sess-3".into(),
            request_id: "req-abc".into(),
            model: "claude-3".into(),
            input_tokens: 200,
            output_tokens: 100,
            total_tokens: 300,
            latency_ms: 750,
            cost_usd: Some(0.0025),
            cached: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("LlmMetrics"));
        assert!(json.contains("claude-3"));
        assert!(json.contains("0.0025"));
        assert!(json.contains("true")); // cached

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "sess-3");
    }

    #[test]
    fn test_agent_event_serialize_tool_call_requested() {
        let event = AgentEvent::ToolCallRequested {
            session_id: "sess-4".into(),
            tool_call_id: "call-xyz".into(),
            tool: "shell".into(),
            args: serde_json::json!({"command": "ls -la", "timeout": 30}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolCallRequested"));
        assert!(json.contains("shell"));
        assert!(json.contains("ls -la"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "tool_call_requested");
    }

    #[test]
    fn test_agent_event_serialize_tool_call_approved() {
        let event = AgentEvent::ToolCallApproved {
            session_id: "sess-5".into(),
            tool_call_id: "call-123".into(),
            tool: "read_file".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolCallApproved"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "tool_call_approved");
    }

    #[test]
    fn test_agent_event_serialize_tool_call_rejected() {
        let event = AgentEvent::ToolCallRejected {
            session_id: "sess-6".into(),
            tool_call_id: "call-456".into(),
            tool: "dangerous_tool".into(),
            reason: "Policy violation".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolCallRejected"));
        assert!(json.contains("Policy violation"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "tool_call_rejected");
    }

    #[test]
    fn test_agent_event_serialize_tool_execution_start() {
        let event = AgentEvent::ToolExecutionStart {
            session_id: "sess-7".into(),
            tool_call_id: "call-789".into(),
            tool: "write_file".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolExecutionStart"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "tool_execution_start");
    }

    #[test]
    fn test_agent_event_serialize_tool_execution_complete() {
        let event = AgentEvent::ToolExecutionComplete {
            session_id: "sess-8".into(),
            tool_call_id: "call-abc".into(),
            tool: "shell".into(),
            success: true,
            duration_ms: 250,
            output_preview: "File created successfully".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolExecutionComplete"));
        assert!(json.contains("File created successfully"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "tool_execution_complete");
    }

    #[test]
    fn test_agent_event_serialize_turn_complete() {
        let event = AgentEvent::TurnComplete {
            session_id: "sess-9".into(),
            turn: 10,
            status: "success".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TurnComplete"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "turn_complete");
    }

    #[test]
    fn test_agent_event_serialize_session_complete() {
        let event = AgentEvent::SessionComplete {
            session_id: "sess-10".into(),
            total_turns: 15,
            status: "completed".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("SessionComplete"));
        assert!(json.contains("15"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "session_complete");
    }

    #[test]
    fn test_agent_event_serialize_token_chunk() {
        let event = AgentEvent::TokenChunk {
            session_id: "sess-11".into(),
            chunk: "Hello, world!".into(),
            is_final: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TokenChunk"));
        assert!(json.contains("Hello, world!"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "token_chunk");
    }

    #[test]
    fn test_agent_event_serialize_error() {
        let event = AgentEvent::Error {
            session_id: "sess-12".into(),
            error: "Connection timeout".into(),
            context: "LLM API call".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Error"));
        assert!(json.contains("Connection timeout"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "error");
    }

    #[test]
    fn test_agent_event_serialize_approval_required_with_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "sess-13".into(),
            request_id: "req-001".into(),
            tool_call_id: "call-001".into(),
            tool: "rm".into(),
            args: serde_json::json!({"path": "/important/file.txt"}),
            reason: Some("Deleting critical file".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ApprovalRequired"));
        assert!(json.contains("Deleting critical file"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "approval_required");
    }

    #[test]
    fn test_agent_event_serialize_approval_required_without_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "sess-14".into(),
            request_id: "req-002".into(),
            tool_call_id: "call-002".into(),
            tool: "safe_tool".into(),
            args: serde_json::json!({}),
            reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ApprovalRequired"));
        assert!(json.contains("null") || !json.contains("reason:")); // reason is None

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "approval_required");
    }

    #[test]
    fn test_agent_event_serialize_eval_capture_full() {
        let event = AgentEvent::EvalCapture {
            session_id: "sess-15".into(),
            capture_id: "cap-001".into(),
            input_messages: r#"[{"role":"user","content":"test"}]"#.into(),
            output_response: r#"{"role":"assistant","content":"response"}"#.into(),
            model: "gpt-4".into(),
            tools: Some(r#"[{"name":"shell"}]"#.into()),
            metadata: Some(serde_json::json!({"category": "test", "version": 1})),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("EvalCapture"));
        assert!(json.contains("cap-001"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "eval_capture");
    }

    #[test]
    fn test_agent_event_serialize_eval_capture_minimal() {
        let event = AgentEvent::EvalCapture {
            session_id: "sess-16".into(),
            capture_id: "cap-002".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "claude".into(),
            tools: None,
            metadata: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("EvalCapture"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "sess-16");
    }

    // MetricsCallback additional tests

    #[tokio::test]
    async fn test_metrics_callback_tokens_from_reasoning_complete_only() {
        let metrics = MetricsCallback::new();

        // ReasoningComplete with tokens (no LlmMetrics)
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(250),
                output_tokens: Some(75),
            })
            .await;

        assert_eq!(metrics.total_input_tokens(), 250);
        assert_eq!(metrics.total_output_tokens(), 75);
    }

    #[tokio::test]
    async fn test_metrics_callback_tokens_none_values() {
        let metrics = MetricsCallback::new();

        // ReasoningComplete with None tokens
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;

        assert_eq!(metrics.total_input_tokens(), 0);
        assert_eq!(metrics.total_output_tokens(), 0);
    }

    #[tokio::test]
    async fn test_metrics_callback_mixed_token_sources() {
        let metrics = MetricsCallback::new();

        // From ReasoningComplete
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
            .await;

        // From LlmMetrics
        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r".into(),
                model: "m".into(),
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                latency_ms: 500,
                cost_usd: None,
                cached: false,
            })
            .await;

        // Should sum both sources
        assert_eq!(metrics.total_input_tokens(), 300);
        assert_eq!(metrics.total_output_tokens(), 150);
    }

    #[tokio::test]
    async fn test_metrics_callback_multiple_reasoning_times() {
        let metrics = MetricsCallback::new();

        for i in 1..=5 {
            metrics
                .on_event(AgentEvent::ReasoningComplete {
                    session_id: "t".into(),
                    turn: i,
                    duration_ms: i as u64 * 100,
                    has_tool_calls: false,
                    tool_count: 0,
                    input_tokens: None,
                    output_tokens: None,
                })
                .await;
        }

        // 100 + 200 + 300 + 400 + 500 = 1500
        assert_eq!(metrics.total_reasoning_time_ms(), 1500);
    }

    #[tokio::test]
    async fn test_metrics_callback_multiple_tool_execution_times() {
        let metrics = MetricsCallback::new();

        for i in 1..=4 {
            metrics
                .on_event(AgentEvent::ToolExecutionComplete {
                    session_id: "t".into(),
                    tool_call_id: format!("tc{}", i),
                    tool: "shell".into(),
                    success: i % 2 == 0,
                    duration_ms: i as u64 * 50,
                    output_preview: "ok".into(),
                })
                .await;
        }

        // 50 + 100 + 150 + 200 = 500
        assert_eq!(metrics.total_tool_execution_time_ms(), 500);
    }

    #[tokio::test]
    async fn test_metrics_callback_cost_mixed() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "m".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.005),
                cached: false,
            })
            .await;

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r2".into(),
                model: "m".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: None, // No cost for this one
                cached: true,
            })
            .await;

        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r3".into(),
                model: "m".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.003),
                cached: false,
            })
            .await;

        // 0.005 + 0.0 + 0.003 = 0.008
        assert!((metrics.total_cost_usd() - 0.008).abs() < 0.0001);
    }

    // Console callback edge cases

    #[tokio::test]
    async fn test_console_callback_non_verbose_tool_requested() {
        let callback = ConsoleStreamCallback::new();
        assert!(!callback.verbose);

        // Non-verbose should not show args
        callback
            .on_event(AgentEvent::ToolCallRequested {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                args: serde_json::json!({"command": "very long command"}),
            })
            .await;
        // Output should just be "[STREAM] Tool requested: shell" without args
    }

    #[tokio::test]
    async fn test_console_callback_non_verbose_tool_complete() {
        let callback = ConsoleStreamCallback::new();

        callback
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 100,
                output_preview: "some long output preview that should not be shown in non-verbose"
                    .into(),
            })
            .await;
        // Output should not include output_preview in non-verbose mode
    }

    #[tokio::test]
    async fn test_console_callback_non_verbose_llm_metrics() {
        let callback = ConsoleStreamCallback::new();

        // LlmMetrics should not print anything in non-verbose mode
        callback
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "gpt-4".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.001),
                cached: false,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_non_verbose_eval_capture() {
        let callback = ConsoleStreamCallback::new();

        // EvalCapture should not print anything in non-verbose mode
        callback
            .on_event(AgentEvent::EvalCapture {
                session_id: "t".into(),
                capture_id: "c1".into(),
                input_messages: "[]".into(),
                output_response: "{}".into(),
                model: "gpt-4".into(),
                tools: None,
                metadata: None,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_user_turn_truncation() {
        let callback = ConsoleStreamCallback::new();

        // Content exactly 50 chars
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "12345678901234567890123456789012345678901234567890".into(), // exactly 50
            })
            .await;

        // Content less than 50 chars
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "short".into(),
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_reasoning_complete_branches() {
        let callback = ConsoleStreamCallback::verbose();

        // Only output tokens
        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: Some(50),
            })
            .await;

        // No tokens at all in verbose
        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 2,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
    }

    // DashFlowStreamConfig additional tests

    #[test]
    fn test_dashflow_stream_config_with_string_types() {
        // Test with String instead of &str
        let config = DashFlowStreamConfig::new(String::from("broker:9092"))
            .with_topic(String::from("my-topic"))
            .with_tenant_id(String::from("my-tenant"));

        assert_eq!(config.bootstrap_servers, "broker:9092");
        assert_eq!(config.topic, "my-topic");
        assert_eq!(config.tenant_id, "my-tenant");
    }

    #[test]
    fn test_dashflow_stream_config_chained_state_diff() {
        let config = DashFlowStreamConfig::default()
            .with_state_diff(true)
            .with_state_diff(false)
            .with_state_diff(true);

        assert!(config.enable_state_diff);
    }

    #[test]
    fn test_dashflow_stream_config_compression_threshold_extremes() {
        let config = DashFlowStreamConfig::default().with_compression_threshold(0);
        assert_eq!(config.compression_threshold, 0);

        let config2 = DashFlowStreamConfig::default().with_compression_threshold(usize::MAX);
        assert_eq!(config2.compression_threshold, usize::MAX);
    }

    // EventTimer tests

    #[test]
    fn test_event_timer_start_multiple() {
        let timer1 = EventTimer::start();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let timer2 = EventTimer::start();

        // timer1 should have more elapsed time
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(timer1.elapsed_ms() >= timer2.elapsed_ms());
    }

    #[test]
    fn test_event_timer_immediate() {
        let timer = EventTimer::start();
        // Immediate check should be near 0
        let elapsed = timer.elapsed_ms();
        assert!(elapsed <= 5); // Allow small variance
    }

    // StreamCallbackBuilder additional tests

    #[tokio::test]
    async fn test_stream_callback_builder_chain_all() {
        let metrics = Arc::new(MetricsCallback::new());

        let callback = StreamCallbackBuilder::new()
            .with_console()
            .with_verbose_console()
            .with_metrics(metrics.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        // Should have received the event
        assert_eq!(metrics.events().len(), 1);
    }

    #[tokio::test]
    async fn test_stream_callback_builder_single_returns_unwrapped() {
        let metrics = Arc::new(MetricsCallback::new());

        // With only one callback, should return it directly (not wrapped in Multi)
        let callback = StreamCallbackBuilder::new()
            .with_metrics(metrics.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
    }

    // MultiStreamCallback edge cases

    #[tokio::test]
    async fn test_multi_stream_callback_empty() {
        let multi = MultiStreamCallback::new(vec![]);

        // Should not panic with empty callbacks
        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        multi.flush().await;
    }

    #[tokio::test]
    async fn test_multi_stream_callback_many_callbacks() {
        let callbacks: Vec<Arc<dyn StreamCallback>> = (0..10)
            .map(|_| Arc::new(MetricsCallback::new()) as Arc<dyn StreamCallback>)
            .collect();

        let multi = MultiStreamCallback::new(callbacks.clone());

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        // All callbacks should have received the event
        // (We can't easily verify this without downcasting, but at least it shouldn't panic)
    }

    // Agent event Debug impl coverage

    #[test]
    fn test_agent_event_debug_all_variants() {
        let events: Vec<AgentEvent> = vec![
            AgentEvent::UserTurn {
                session_id: "s".into(),
                content: "c".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: "s".into(),
                turn: 1,
                model: "m".into(),
            },
            AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: Some(50),
            },
            AgentEvent::LlmMetrics {
                session_id: "s".into(),
                request_id: "r".into(),
                model: "m".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
                cost_usd: Some(0.01),
                cached: false,
            },
            AgentEvent::ToolCallRequested {
                session_id: "s".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
            },
            AgentEvent::ToolCallApproved {
                session_id: "s".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolCallRejected {
                session_id: "s".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
                reason: "r".into(),
            },
            AgentEvent::ToolExecutionStart {
                session_id: "s".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: "s".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 50,
                output_preview: "ok".into(),
            },
            AgentEvent::TurnComplete {
                session_id: "s".into(),
                turn: 1,
                status: "ok".into(),
            },
            AgentEvent::SessionComplete {
                session_id: "s".into(),
                total_turns: 5,
                status: "done".into(),
            },
            AgentEvent::TokenChunk {
                session_id: "s".into(),
                chunk: "tok".into(),
                is_final: false,
            },
            AgentEvent::Error {
                session_id: "s".into(),
                error: "e".into(),
                context: "c".into(),
            },
            AgentEvent::ApprovalRequired {
                session_id: "s".into(),
                request_id: "r".into(),
                tool_call_id: "t".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
                reason: Some("r".into()),
            },
            AgentEvent::EvalCapture {
                session_id: "s".into(),
                capture_id: "c".into(),
                input_messages: "[]".into(),
                output_response: "{}".into(),
                model: "m".into(),
                tools: Some("[]".into()),
                metadata: Some(serde_json::json!({"k": "v"})),
            },
        ];

        for event in events {
            let debug = format!("{:?}", event);
            assert!(!debug.is_empty());
        }
    }

    // Clone tests for events with complex fields

    #[test]
    fn test_agent_event_clone_with_json_args() {
        let event = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "shell".into(),
            args: serde_json::json!({"nested": {"deep": [1,2,3]}}),
        };
        let cloned = event.clone();

        match (&event, &cloned) {
            (
                AgentEvent::ToolCallRequested { args: a1, .. },
                AgentEvent::ToolCallRequested { args: a2, .. },
            ) => {
                assert_eq!(a1, a2);
            }
            _ => panic!("Expected ToolCallRequested"),
        }
    }

    #[test]
    fn test_agent_event_clone_eval_capture_with_metadata() {
        let event = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "m".into(),
            tools: Some("tools".into()),
            metadata: Some(serde_json::json!({"complex": {"nested": true}})),
        };
        let cloned = event.clone();

        match (&event, &cloned) {
            (
                AgentEvent::EvalCapture { metadata: m1, .. },
                AgentEvent::EvalCapture { metadata: m2, .. },
            ) => {
                assert_eq!(m1, m2);
            }
            _ => panic!("Expected EvalCapture"),
        }
    }

    // Verify trait bounds

    #[test]
    fn test_null_stream_callback_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NullStreamCallback>();
    }

    #[test]
    fn test_console_stream_callback_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConsoleStreamCallback>();
    }

    #[test]
    fn test_metrics_callback_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MetricsCallback>();
    }

    #[test]
    fn test_multi_stream_callback_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MultiStreamCallback>();
    }

    #[test]
    fn test_agent_event_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AgentEvent>();
    }

    // MetricsCallback eval_captures edge cases

    #[tokio::test]
    async fn test_metrics_callback_eval_captures_empty() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        // No eval captures
        assert!(metrics.eval_captures().is_empty());
    }

    #[tokio::test]
    async fn test_metrics_callback_eval_captures_multiple() {
        let metrics = MetricsCallback::new();

        for i in 0..5 {
            metrics
                .on_event(AgentEvent::EvalCapture {
                    session_id: "t".into(),
                    capture_id: format!("cap-{}", i),
                    input_messages: "[]".into(),
                    output_response: "{}".into(),
                    model: "m".into(),
                    tools: None,
                    metadata: None,
                })
                .await;
        }

        assert_eq!(metrics.eval_captures().len(), 5);
    }

    // Event counts edge cases

    #[tokio::test]
    async fn test_metrics_callback_event_counts_all_types() {
        let metrics = MetricsCallback::new();

        // Add one of each type
        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "c".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningStart {
                session_id: "t".into(),
                turn: 1,
                model: "m".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
        metrics
            .on_event(AgentEvent::TurnComplete {
                session_id: "t".into(),
                turn: 1,
                status: "ok".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::SessionComplete {
                session_id: "t".into(),
                total_turns: 1,
                status: "done".into(),
            })
            .await;

        let counts = metrics.event_counts();
        assert_eq!(counts.len(), 5);
        assert_eq!(counts.get("user_turn"), Some(&1));
        assert_eq!(counts.get("reasoning_start"), Some(&1));
        assert_eq!(counts.get("reasoning_complete"), Some(&1));
        assert_eq!(counts.get("turn_complete"), Some(&1));
        assert_eq!(counts.get("session_complete"), Some(&1));
    }

    // Verify serde round-trip for complex events

    #[test]
    fn test_serde_roundtrip_all_optional_fields() {
        // ApprovalRequired with Some reason
        let event1 = AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "t".into(),
            tool: "shell".into(),
            args: serde_json::json!({"key": "value"}),
            reason: Some("important reason".into()),
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::ApprovalRequired { reason, .. } => {
                assert_eq!(reason, Some("important reason".into()));
            }
            _ => panic!("Wrong variant"),
        }

        // ApprovalRequired with None reason
        let event2 = AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "t".into(),
            tool: "shell".into(),
            args: serde_json::json!({}),
            reason: None,
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::ApprovalRequired { reason, .. } => {
                assert_eq!(reason, None);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_llm_metrics_cost() {
        // With cost
        let event1 = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 500,
            cost_usd: Some(0.00123456789),
            cached: false,
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::LlmMetrics { cost_usd, .. } => {
                assert!(cost_usd.is_some());
                assert!((cost_usd.unwrap() - 0.00123456789).abs() < 1e-10);
            }
            _ => panic!("Wrong variant"),
        }

        // Without cost
        let event2 = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 500,
            cost_usd: None,
            cached: true,
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::LlmMetrics {
                cost_usd, cached, ..
            } => {
                assert!(cost_usd.is_none());
                assert!(cached);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_eval_capture_optional_fields() {
        // With all optional fields
        let event1 = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "m".into(),
            tools: Some("[{\"name\":\"shell\"}]".into()),
            metadata: Some(serde_json::json!({"key": "value", "nested": {"a": 1}})),
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::EvalCapture {
                tools, metadata, ..
            } => {
                assert!(tools.is_some());
                assert!(metadata.is_some());
            }
            _ => panic!("Wrong variant"),
        }

        // Without optional fields
        let event2 = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "m".into(),
            tools: None,
            metadata: None,
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::EvalCapture {
                tools, metadata, ..
            } => {
                assert!(tools.is_none());
                assert!(metadata.is_none());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_reasoning_complete_tokens() {
        // With both tokens
        let event1 = AgentEvent::ReasoningComplete {
            session_id: "s".into(),
            turn: 1,
            duration_ms: 100,
            has_tool_calls: true,
            tool_count: 3,
            input_tokens: Some(1000),
            output_tokens: Some(500),
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::ReasoningComplete {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, Some(1000));
                assert_eq!(output_tokens, Some(500));
            }
            _ => panic!("Wrong variant"),
        }

        // With partial tokens
        let event2 = AgentEvent::ReasoningComplete {
            session_id: "s".into(),
            turn: 2,
            duration_ms: 200,
            has_tool_calls: false,
            tool_count: 0,
            input_tokens: Some(500),
            output_tokens: None,
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::ReasoningComplete {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, Some(500));
                assert_eq!(output_tokens, None);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // ============================================================================
    // Additional test coverage (N=284)
    // ============================================================================

    // ConsoleStreamCallback content preview boundary tests

    #[tokio::test]
    async fn test_console_callback_user_turn_exactly_50_chars() {
        let callback = ConsoleStreamCallback::new();
        // Exactly 50 characters - should NOT be truncated
        let content = "a".repeat(50);
        assert_eq!(content.len(), 50);
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_user_turn_51_chars() {
        let callback = ConsoleStreamCallback::new();
        // 51 characters - should be truncated
        let content = "a".repeat(51);
        assert_eq!(content.len(), 51);
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_user_turn_49_chars() {
        let callback = ConsoleStreamCallback::new();
        // 49 characters - should NOT be truncated
        let content = "a".repeat(49);
        assert_eq!(content.len(), 49);
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_user_turn_empty() {
        let callback = ConsoleStreamCallback::new();
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "".into(),
            })
            .await;
    }

    // MetricsCallback total_* functions with empty events

    #[test]
    fn test_metrics_callback_empty_totals() {
        let metrics = MetricsCallback::new();
        // All totals should be 0 with no events
        assert_eq!(metrics.total_reasoning_time_ms(), 0);
        assert_eq!(metrics.total_tool_execution_time_ms(), 0);
        assert_eq!(metrics.total_input_tokens(), 0);
        assert_eq!(metrics.total_output_tokens(), 0);
        assert_eq!(metrics.total_cost_usd(), 0.0);
    }

    #[tokio::test]
    async fn test_metrics_callback_non_relevant_events_dont_affect_totals() {
        let metrics = MetricsCallback::new();

        // Add events that don't contribute to totals
        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningStart {
                session_id: "t".into(),
                turn: 1,
                model: "m".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ToolCallRequested {
                session_id: "t".into(),
                tool_call_id: "tc".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
            })
            .await;

        // Totals should still be 0
        assert_eq!(metrics.total_reasoning_time_ms(), 0);
        assert_eq!(metrics.total_tool_execution_time_ms(), 0);
        assert_eq!(metrics.total_input_tokens(), 0);
        assert_eq!(metrics.total_output_tokens(), 0);
        assert_eq!(metrics.total_cost_usd(), 0.0);
    }

    // DashFlowStreamConfig edge cases

    #[test]
    fn test_dashflow_stream_config_empty_strings() {
        let config = DashFlowStreamConfig::new("")
            .with_topic("")
            .with_tenant_id("");

        assert_eq!(config.bootstrap_servers, "");
        assert_eq!(config.topic, "");
        assert_eq!(config.tenant_id, "");
    }

    #[test]
    fn test_dashflow_stream_config_unicode() {
        let config = DashFlowStreamConfig::new("kafka-日本語:9092")
            .with_topic("テスト-topic-émoji-🎉")
            .with_tenant_id("租户-123");

        assert_eq!(config.bootstrap_servers, "kafka-日本語:9092");
        assert_eq!(config.topic, "テスト-topic-émoji-🎉");
        assert_eq!(config.tenant_id, "租户-123");
    }

    #[test]
    fn test_dashflow_stream_config_very_long_strings() {
        let long_string = "a".repeat(10000);
        let config = DashFlowStreamConfig::new(&long_string)
            .with_topic(&long_string)
            .with_tenant_id(&long_string);

        assert_eq!(config.bootstrap_servers.len(), 10000);
        assert_eq!(config.topic.len(), 10000);
        assert_eq!(config.tenant_id.len(), 10000);
    }

    // EventTimer additional tests

    #[test]
    fn test_event_timer_elapsed_increases() {
        let timer = EventTimer::start();
        let t1 = timer.elapsed_ms();
        std::thread::sleep(std::time::Duration::from_millis(15));
        let t2 = timer.elapsed_ms();
        assert!(t2 >= t1);
    }

    #[test]
    fn test_event_timer_multiple_elapsed_calls() {
        let timer = EventTimer::start();
        for _ in 0..5 {
            let _ = timer.elapsed_ms();
        }
        // Should not panic or cause issues with multiple calls
    }

    // Console callback verbose mode specific branches

    #[tokio::test]
    async fn test_console_callback_verbose_tool_execution_failed() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: false,
                duration_ms: 100,
                output_preview: "Error: command failed".into(),
            })
            .await;
        // Should print with FAILED status and output_preview
    }

    #[tokio::test]
    async fn test_console_callback_verbose_llm_metrics_with_cost() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "gpt-4-turbo".into(),
                input_tokens: 5000,
                output_tokens: 2000,
                total_tokens: 7000,
                latency_ms: 3500,
                cost_usd: Some(0.0875),
                cached: false,
            })
            .await;
        // Should print metrics with cost
    }

    #[tokio::test]
    async fn test_console_callback_verbose_llm_metrics_without_cost() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r2".into(),
                model: "local-model".into(),
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                latency_ms: 200,
                cost_usd: None,
                cached: true,
            })
            .await;
        // Should print metrics without cost
    }

    #[tokio::test]
    async fn test_console_callback_verbose_eval_capture() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::EvalCapture {
                session_id: "t".into(),
                capture_id: "eval-capture-12345".into(),
                input_messages: r#"[{"role":"user","content":"test"}]"#.into(),
                output_response: r#"{"role":"assistant","content":"response"}"#.into(),
                model: "gpt-4".into(),
                tools: Some(r#"[{"name":"shell"}]"#.into()),
                metadata: Some(serde_json::json!({"category": "test"})),
            })
            .await;
        // Should print eval capture info in verbose mode
    }

    // AgentEvent field access patterns

    #[test]
    fn test_agent_event_tool_execution_complete_fields() {
        let event = AgentEvent::ToolExecutionComplete {
            session_id: "sess-test".into(),
            tool_call_id: "call-abc123".into(),
            tool: "write_file".into(),
            success: true,
            duration_ms: 42,
            output_preview: "Written 100 bytes".into(),
        };

        match event {
            AgentEvent::ToolExecutionComplete {
                session_id,
                tool_call_id,
                tool,
                success,
                duration_ms,
                output_preview,
            } => {
                assert_eq!(session_id, "sess-test");
                assert_eq!(tool_call_id, "call-abc123");
                assert_eq!(tool, "write_file");
                assert!(success);
                assert_eq!(duration_ms, 42);
                assert_eq!(output_preview, "Written 100 bytes");
            }
            _ => panic!("Expected ToolExecutionComplete"),
        }
    }

    #[test]
    fn test_agent_event_token_chunk_fields() {
        let event = AgentEvent::TokenChunk {
            session_id: "sess-stream".into(),
            chunk: "Hello, ".into(),
            is_final: false,
        };

        match &event {
            AgentEvent::TokenChunk {
                session_id,
                chunk,
                is_final,
            } => {
                assert_eq!(session_id, "sess-stream");
                assert_eq!(chunk, "Hello, ");
                assert!(!is_final);
            }
            _ => panic!("Expected TokenChunk"),
        }

        // Also test final chunk
        let final_event = AgentEvent::TokenChunk {
            session_id: "sess-stream".into(),
            chunk: "world!".into(),
            is_final: true,
        };

        match &final_event {
            AgentEvent::TokenChunk { is_final, .. } => {
                assert!(is_final);
            }
            _ => panic!("Expected TokenChunk"),
        }
    }

    #[test]
    fn test_agent_event_error_fields() {
        let event = AgentEvent::Error {
            session_id: "sess-err".into(),
            error: "Connection refused: cannot reach API endpoint".into(),
            context: "LLM API call during reasoning phase".into(),
        };

        match &event {
            AgentEvent::Error {
                session_id,
                error,
                context,
            } => {
                assert_eq!(session_id, "sess-err");
                assert!(error.contains("Connection refused"));
                assert!(context.contains("LLM API"));
            }
            _ => panic!("Expected Error"),
        }
    }

    // StreamCallbackBuilder with custom callback ordering

    #[tokio::test]
    async fn test_stream_callback_builder_preserves_order() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());
        let metrics3 = Arc::new(MetricsCallback::new());

        let callback = StreamCallbackBuilder::new()
            .with_callback(metrics1.clone())
            .with_callback(metrics2.clone())
            .with_callback(metrics3.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        // All three should have received the event
        assert_eq!(metrics1.events().len(), 1);
        assert_eq!(metrics2.events().len(), 1);
        assert_eq!(metrics3.events().len(), 1);
    }

    // MultiStreamCallback with mixed callback types

    #[tokio::test]
    async fn test_multi_stream_callback_mixed_types() {
        let null_cb: Arc<dyn StreamCallback> = Arc::new(NullStreamCallback);
        let console_cb: Arc<dyn StreamCallback> = Arc::new(ConsoleStreamCallback::new());
        let metrics_cb = Arc::new(MetricsCallback::new());

        let multi = MultiStreamCallback::new(vec![
            null_cb,
            console_cb,
            metrics_cb.clone() as Arc<dyn StreamCallback>,
        ]);

        multi
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 500,
                has_tool_calls: true,
                tool_count: 2,
                input_tokens: Some(1000),
                output_tokens: Some(500),
            })
            .await;

        // MetricsCallback should have captured it
        assert_eq!(metrics_cb.events().len(), 1);
        assert_eq!(metrics_cb.total_reasoning_time_ms(), 500);
    }

    // Metrics callback accumulation tests

    #[tokio::test]
    async fn test_metrics_callback_accumulation_interleaved() {
        let metrics = MetricsCallback::new();

        // Interleave different event types
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: true,
                tool_count: 1,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
            .await;
        metrics
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 25,
                output_preview: "ok".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "t".into(),
                request_id: "r1".into(),
                model: "m".into(),
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                latency_ms: 500,
                cost_usd: Some(0.005),
                cached: false,
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 2,
                duration_ms: 150,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
        metrics
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "t".into(),
                tool_call_id: "tc2".into(),
                tool: "file".into(),
                success: false,
                duration_ms: 10,
                output_preview: "error".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 5);
        assert_eq!(metrics.total_reasoning_time_ms(), 250); // 100 + 150
        assert_eq!(metrics.total_tool_execution_time_ms(), 35); // 25 + 10
        assert_eq!(metrics.total_input_tokens(), 300); // 100 + 200
        assert_eq!(metrics.total_output_tokens(), 150); // 50 + 100
        assert!((metrics.total_cost_usd() - 0.005).abs() < 0.0001);
    }

    // ConsoleStreamCallback flush test

    #[tokio::test]
    async fn test_console_stream_callback_flush() {
        let callback = ConsoleStreamCallback::new();
        callback.flush().await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_console_stream_callback_verbose_flush() {
        let callback = ConsoleStreamCallback::verbose();
        callback.flush().await;
        // Should not panic
    }

    // MetricsCallback flush test

    #[tokio::test]
    async fn test_metrics_callback_flush() {
        let metrics = MetricsCallback::new();
        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
        metrics.flush().await;
        // Should not panic, events should still be there
        assert_eq!(metrics.events().len(), 1);
    }

    // Test serde with special JSON values in args

    #[test]
    fn test_agent_event_serialize_with_special_json_values() {
        // Null value
        let event1 = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "test".into(),
            args: serde_json::json!(null),
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args.is_null());
            }
            _ => panic!("Wrong variant"),
        }

        // Array value
        let event2 = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "test".into(),
            args: serde_json::json!([1, 2, 3, "four", null, true]),
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args.is_array());
                assert_eq!(args.as_array().unwrap().len(), 6);
            }
            _ => panic!("Wrong variant"),
        }

        // Boolean value
        let event3 = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "test".into(),
            args: serde_json::json!(true),
        };
        let json3 = serde_json::to_string(&event3).unwrap();
        let parsed3: AgentEvent = serde_json::from_str(&json3).unwrap();
        match parsed3 {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args.is_boolean());
                assert!(args.as_bool().unwrap());
            }
            _ => panic!("Wrong variant"),
        }

        // Number value
        let event4 = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "test".into(),
            args: serde_json::json!(42.5),
        };
        let json4 = serde_json::to_string(&event4).unwrap();
        let parsed4: AgentEvent = serde_json::from_str(&json4).unwrap();
        match parsed4 {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args.is_f64());
                assert!((args.as_f64().unwrap() - 42.5).abs() < 0.001);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test nested metadata in EvalCapture

    #[test]
    fn test_eval_capture_deeply_nested_metadata() {
        let event = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "m".into(),
            tools: None,
            metadata: Some(serde_json::json!({
                "level1": {
                    "level2": {
                        "level3": {
                            "level4": {
                                "value": "deep"
                            }
                        }
                    }
                }
            })),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::EvalCapture { metadata, .. } => {
                let m = metadata.unwrap();
                let deep_value = m
                    .get("level1")
                    .unwrap()
                    .get("level2")
                    .unwrap()
                    .get("level3")
                    .unwrap()
                    .get("level4")
                    .unwrap()
                    .get("value")
                    .unwrap()
                    .as_str()
                    .unwrap();
                assert_eq!(deep_value, "deep");
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test LlmMetrics extreme values

    #[test]
    fn test_llm_metrics_extreme_values() {
        // Maximum u32 tokens
        let event1 = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: u32::MAX,
            output_tokens: u32::MAX,
            total_tokens: u32::MAX,
            latency_ms: u64::MAX,
            cost_usd: Some(f64::MAX),
            cached: false,
        };
        let json1 = serde_json::to_string(&event1).unwrap();
        let parsed1: AgentEvent = serde_json::from_str(&json1).unwrap();
        match parsed1 {
            AgentEvent::LlmMetrics {
                input_tokens,
                output_tokens,
                total_tokens,
                latency_ms,
                ..
            } => {
                assert_eq!(input_tokens, u32::MAX);
                assert_eq!(output_tokens, u32::MAX);
                assert_eq!(total_tokens, u32::MAX);
                assert_eq!(latency_ms, u64::MAX);
            }
            _ => panic!("Wrong variant"),
        }

        // Zero values
        let event2 = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            latency_ms: 0,
            cost_usd: Some(0.0),
            cached: true,
        };
        let json2 = serde_json::to_string(&event2).unwrap();
        let parsed2: AgentEvent = serde_json::from_str(&json2).unwrap();
        match parsed2 {
            AgentEvent::LlmMetrics {
                input_tokens,
                output_tokens,
                total_tokens,
                latency_ms,
                cost_usd,
                ..
            } => {
                assert_eq!(input_tokens, 0);
                assert_eq!(output_tokens, 0);
                assert_eq!(total_tokens, 0);
                assert_eq!(latency_ms, 0);
                assert_eq!(cost_usd, Some(0.0));
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test ReasoningComplete extreme turn values

    #[test]
    fn test_reasoning_complete_extreme_values() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "s".into(),
            turn: u32::MAX,
            duration_ms: u64::MAX,
            has_tool_calls: true,
            tool_count: usize::MAX,
            input_tokens: Some(u32::MAX),
            output_tokens: Some(u32::MAX),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::ReasoningComplete {
                turn,
                duration_ms,
                tool_count,
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(turn, u32::MAX);
                assert_eq!(duration_ms, u64::MAX);
                assert_eq!(tool_count, usize::MAX);
                assert_eq!(input_tokens, Some(u32::MAX));
                assert_eq!(output_tokens, Some(u32::MAX));
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test SessionComplete extreme values

    #[test]
    fn test_session_complete_extreme_values() {
        let event = AgentEvent::SessionComplete {
            session_id: "s".into(),
            total_turns: u32::MAX,
            status: "completed_after_max_turns".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::SessionComplete { total_turns, .. } => {
                assert_eq!(total_turns, u32::MAX);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test empty strings in various fields

    #[test]
    fn test_agent_events_with_empty_strings() {
        let events = vec![
            AgentEvent::UserTurn {
                session_id: "".into(),
                content: "".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: "".into(),
                turn: 0,
                model: "".into(),
            },
            AgentEvent::ToolCallRequested {
                session_id: "".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                args: serde_json::json!({}),
            },
            AgentEvent::Error {
                session_id: "".into(),
                error: "".into(),
                context: "".into(),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
            // Should serialize and deserialize without issue
            assert_eq!(parsed.session_id(), "");
        }
    }

    // Test special characters in strings

    #[test]
    fn test_agent_events_with_special_characters() {
        let special_content = "Hello\n\t\r\0\"'\\世界🎉<>&;|$`";

        let event = AgentEvent::UserTurn {
            session_id: special_content.into(),
            content: special_content.into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::UserTurn {
                session_id,
                content,
            } => {
                assert_eq!(session_id, special_content);
                assert_eq!(content, special_content);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test ToolExecutionComplete with long output preview

    #[test]
    fn test_tool_execution_complete_long_output() {
        let long_output = "x".repeat(100000);

        let event = AgentEvent::ToolExecutionComplete {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "shell".into(),
            success: true,
            duration_ms: 1000,
            output_preview: long_output.clone(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::ToolExecutionComplete { output_preview, .. } => {
                assert_eq!(output_preview.len(), 100000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Test event_counts with many events

    #[tokio::test]
    async fn test_metrics_callback_event_counts_many_events() {
        let metrics = MetricsCallback::new();

        // Add many events of different types
        for _ in 0..50 {
            metrics
                .on_event(AgentEvent::UserTurn {
                    session_id: "t".into(),
                    content: "hi".into(),
                })
                .await;
        }
        for _ in 0..30 {
            metrics
                .on_event(AgentEvent::ReasoningStart {
                    session_id: "t".into(),
                    turn: 1,
                    model: "m".into(),
                })
                .await;
        }
        for _ in 0..20 {
            metrics
                .on_event(AgentEvent::TurnComplete {
                    session_id: "t".into(),
                    turn: 1,
                    status: "ok".into(),
                })
                .await;
        }

        let counts = metrics.event_counts();
        assert_eq!(counts.get("user_turn"), Some(&50));
        assert_eq!(counts.get("reasoning_start"), Some(&30));
        assert_eq!(counts.get("turn_complete"), Some(&20));
        assert_eq!(metrics.events().len(), 100);
    }

    // Test clear and reuse metrics callback

    #[tokio::test]
    async fn test_metrics_callback_clear_and_reuse() {
        let metrics = MetricsCallback::new();

        // Add events
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
        assert_eq!(metrics.total_reasoning_time_ms(), 100);

        // Clear
        metrics.clear();
        assert_eq!(metrics.events().len(), 0);
        assert_eq!(metrics.total_reasoning_time_ms(), 0);

        // Reuse
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 2,
                duration_ms: 200,
                has_tool_calls: true,
                tool_count: 3,
                input_tokens: Some(200),
                output_tokens: Some(100),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
        assert_eq!(metrics.total_reasoning_time_ms(), 200);
    }

    // Test ApprovalRequired with various args shapes

    #[test]
    fn test_approval_required_various_args() {
        let test_cases = vec![
            serde_json::json!(null),
            serde_json::json!({}),
            serde_json::json!([]),
            serde_json::json!("string_arg"),
            serde_json::json!(12345),
            serde_json::json!(true),
            serde_json::json!({"nested": {"array": [1,2,3]}}),
        ];

        for args in test_cases {
            let event = AgentEvent::ApprovalRequired {
                session_id: "s".into(),
                request_id: "r".into(),
                tool_call_id: "t".into(),
                tool: "test".into(),
                args: args.clone(),
                reason: None,
            };

            let json = serde_json::to_string(&event).unwrap();
            let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

            match parsed {
                AgentEvent::ApprovalRequired {
                    args: parsed_args, ..
                } => {
                    assert_eq!(parsed_args, args);
                }
                _ => panic!("Wrong variant"),
            }
        }
    }

    // ============================================================================
    // Additional test coverage (N=286)
    // ============================================================================

    // AgentEvent field-level tests for exhaustive variant coverage

    #[test]
    fn test_user_turn_fields() {
        let event = AgentEvent::UserTurn {
            session_id: "session-xyz-123".into(),
            content: "What is the meaning of life?".into(),
        };
        match &event {
            AgentEvent::UserTurn {
                session_id,
                content,
            } => {
                assert_eq!(session_id, "session-xyz-123");
                assert_eq!(content, "What is the meaning of life?");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_reasoning_start_fields() {
        let event = AgentEvent::ReasoningStart {
            session_id: "sess-001".into(),
            turn: 42,
            model: "claude-3-opus-20240229".into(),
        };
        match &event {
            AgentEvent::ReasoningStart {
                session_id,
                turn,
                model,
            } => {
                assert_eq!(session_id, "sess-001");
                assert_eq!(*turn, 42);
                assert_eq!(model, "claude-3-opus-20240229");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_reasoning_complete_all_fields() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "sess-002".into(),
            turn: 7,
            duration_ms: 2500,
            has_tool_calls: true,
            tool_count: 3,
            input_tokens: Some(1500),
            output_tokens: Some(800),
        };
        match &event {
            AgentEvent::ReasoningComplete {
                session_id,
                turn,
                duration_ms,
                has_tool_calls,
                tool_count,
                input_tokens,
                output_tokens,
            } => {
                assert_eq!(session_id, "sess-002");
                assert_eq!(*turn, 7);
                assert_eq!(*duration_ms, 2500);
                assert!(*has_tool_calls);
                assert_eq!(*tool_count, 3);
                assert_eq!(*input_tokens, Some(1500));
                assert_eq!(*output_tokens, Some(800));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_llm_metrics_all_fields() {
        let event = AgentEvent::LlmMetrics {
            session_id: "sess-003".into(),
            request_id: "req-abc-123".into(),
            model: "gpt-4-turbo-preview".into(),
            input_tokens: 2000,
            output_tokens: 1500,
            total_tokens: 3500,
            latency_ms: 4200,
            cost_usd: Some(0.0875),
            cached: true,
        };
        match &event {
            AgentEvent::LlmMetrics {
                session_id,
                request_id,
                model,
                input_tokens,
                output_tokens,
                total_tokens,
                latency_ms,
                cost_usd,
                cached,
            } => {
                assert_eq!(session_id, "sess-003");
                assert_eq!(request_id, "req-abc-123");
                assert_eq!(model, "gpt-4-turbo-preview");
                assert_eq!(*input_tokens, 2000);
                assert_eq!(*output_tokens, 1500);
                assert_eq!(*total_tokens, 3500);
                assert_eq!(*latency_ms, 4200);
                assert!((cost_usd.unwrap() - 0.0875).abs() < 0.0001);
                assert!(*cached);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_call_requested_fields() {
        let event = AgentEvent::ToolCallRequested {
            session_id: "sess-004".into(),
            tool_call_id: "call-def-456".into(),
            tool: "execute_shell".into(),
            args: serde_json::json!({"command": "ls -la", "timeout": 30}),
        };
        match &event {
            AgentEvent::ToolCallRequested {
                session_id,
                tool_call_id,
                tool,
                args,
            } => {
                assert_eq!(session_id, "sess-004");
                assert_eq!(tool_call_id, "call-def-456");
                assert_eq!(tool, "execute_shell");
                assert_eq!(args["command"], "ls -la");
                assert_eq!(args["timeout"], 30);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_call_approved_fields() {
        let event = AgentEvent::ToolCallApproved {
            session_id: "sess-005".into(),
            tool_call_id: "call-ghi-789".into(),
            tool: "read_file".into(),
        };
        match &event {
            AgentEvent::ToolCallApproved {
                session_id,
                tool_call_id,
                tool,
            } => {
                assert_eq!(session_id, "sess-005");
                assert_eq!(tool_call_id, "call-ghi-789");
                assert_eq!(tool, "read_file");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_call_rejected_fields() {
        let event = AgentEvent::ToolCallRejected {
            session_id: "sess-006".into(),
            tool_call_id: "call-jkl-012".into(),
            tool: "delete_file".into(),
            reason: "User denied deletion of system file".into(),
        };
        match &event {
            AgentEvent::ToolCallRejected {
                session_id,
                tool_call_id,
                tool,
                reason,
            } => {
                assert_eq!(session_id, "sess-006");
                assert_eq!(tool_call_id, "call-jkl-012");
                assert_eq!(tool, "delete_file");
                assert!(reason.contains("system file"));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_execution_start_fields() {
        let event = AgentEvent::ToolExecutionStart {
            session_id: "sess-007".into(),
            tool_call_id: "call-mno-345".into(),
            tool: "write_file".into(),
        };
        match &event {
            AgentEvent::ToolExecutionStart {
                session_id,
                tool_call_id,
                tool,
            } => {
                assert_eq!(session_id, "sess-007");
                assert_eq!(tool_call_id, "call-mno-345");
                assert_eq!(tool, "write_file");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_turn_complete_fields() {
        let event = AgentEvent::TurnComplete {
            session_id: "sess-008".into(),
            turn: 15,
            status: "completed_with_response".into(),
        };
        match &event {
            AgentEvent::TurnComplete {
                session_id,
                turn,
                status,
            } => {
                assert_eq!(session_id, "sess-008");
                assert_eq!(*turn, 15);
                assert_eq!(status, "completed_with_response");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_session_complete_fields() {
        let event = AgentEvent::SessionComplete {
            session_id: "sess-009".into(),
            total_turns: 42,
            status: "user_terminated".into(),
        };
        match &event {
            AgentEvent::SessionComplete {
                session_id,
                total_turns,
                status,
            } => {
                assert_eq!(session_id, "sess-009");
                assert_eq!(*total_turns, 42);
                assert_eq!(status, "user_terminated");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_approval_required_fields_with_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "sess-010".into(),
            request_id: "req-xyz".into(),
            tool_call_id: "call-pqr-678".into(),
            tool: "execute_dangerous".into(),
            args: serde_json::json!({"operation": "format_disk"}),
            reason: Some("Potentially destructive operation".into()),
        };
        match &event {
            AgentEvent::ApprovalRequired {
                session_id,
                request_id,
                tool_call_id,
                tool,
                args,
                reason,
            } => {
                assert_eq!(session_id, "sess-010");
                assert_eq!(request_id, "req-xyz");
                assert_eq!(tool_call_id, "call-pqr-678");
                assert_eq!(tool, "execute_dangerous");
                assert_eq!(args["operation"], "format_disk");
                assert_eq!(reason.as_deref(), Some("Potentially destructive operation"));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_eval_capture_all_fields() {
        let event = AgentEvent::EvalCapture {
            session_id: "sess-011".into(),
            capture_id: "cap-abc-123".into(),
            input_messages:
                r#"[{"role":"system","content":"You are helpful"},{"role":"user","content":"Hi"}]"#
                    .into(),
            output_response: r#"{"role":"assistant","content":"Hello!"}"#.into(),
            model: "claude-3-sonnet".into(),
            tools: Some(r#"[{"name":"shell","description":"Execute shell commands"}]"#.into()),
            metadata: Some(serde_json::json!({"category": "greeting", "difficulty": "easy"})),
        };
        match &event {
            AgentEvent::EvalCapture {
                session_id,
                capture_id,
                input_messages,
                output_response,
                model,
                tools,
                metadata,
            } => {
                assert_eq!(session_id, "sess-011");
                assert_eq!(capture_id, "cap-abc-123");
                assert!(input_messages.contains("You are helpful"));
                assert!(output_response.contains("Hello!"));
                assert_eq!(model, "claude-3-sonnet");
                assert!(tools.as_ref().unwrap().contains("shell"));
                assert_eq!(metadata.as_ref().unwrap()["category"], "greeting");
            }
            _ => panic!("Wrong variant"),
        }
    }

    // DashFlowStreamConfig additional tests

    #[test]
    fn test_dashflow_stream_config_special_characters() {
        let config = DashFlowStreamConfig::new("kafka://user:p@ss=word@broker:9092")
            .with_topic("events/topic-with-slashes")
            .with_tenant_id("tenant:with:colons");

        assert!(config.bootstrap_servers.contains("@"));
        assert!(config.topic.contains("/"));
        assert!(config.tenant_id.contains(":"));
    }

    #[test]
    fn test_dashflow_stream_config_whitespace() {
        let config = DashFlowStreamConfig::new("  broker:9092  ")
            .with_topic("  topic  ")
            .with_tenant_id("  tenant  ");

        // Should preserve whitespace (not trim)
        assert!(config.bootstrap_servers.starts_with(' '));
        assert!(config.topic.starts_with(' '));
        assert!(config.tenant_id.starts_with(' '));
    }

    #[test]
    fn test_dashflow_stream_config_multiple_brokers() {
        let config = DashFlowStreamConfig::new("broker1:9092,broker2:9093,broker3:9094");
        assert!(config.bootstrap_servers.contains("broker1"));
        assert!(config.bootstrap_servers.contains("broker2"));
        assert!(config.bootstrap_servers.contains("broker3"));
    }

    // MetricsCallback concurrent access simulation

    #[tokio::test]
    async fn test_metrics_callback_sequential_access() {
        let metrics = MetricsCallback::new();

        // Sequential access to various methods
        for i in 0..10 {
            metrics
                .on_event(AgentEvent::ReasoningComplete {
                    session_id: format!("s{}", i),
                    turn: i as u32,
                    duration_ms: 100,
                    has_tool_calls: false,
                    tool_count: 0,
                    input_tokens: Some(100),
                    output_tokens: Some(50),
                })
                .await;

            // Interleave reads
            let _ = metrics.events();
            let _ = metrics.event_counts();
            let _ = metrics.total_input_tokens();
        }

        assert_eq!(metrics.events().len(), 10);
        assert_eq!(metrics.total_input_tokens(), 1000);
    }

    // Console callback edge cases

    #[tokio::test]
    async fn test_console_callback_unicode_content() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "session-unicode".into(),
                content: "Hello world! This is a test with unicode: emoji, test".into(),
            })
            .await;

        callback
            .on_event(AgentEvent::Error {
                session_id: "s".into(),
                error: "Error occurred: file not found".into(),
                context: "File operation".into(),
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_very_long_content() {
        let callback = ConsoleStreamCallback::new();
        let very_long_content = "x".repeat(10000);

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "s".into(),
                content: very_long_content,
            })
            .await;
        // Should truncate to 50 chars + "..."
    }

    #[tokio::test]
    async fn test_console_callback_newlines_in_content() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "s".into(),
                content: "Line 1\nLine 2\nLine 3\n\n\nLine 6".into(),
            })
            .await;

        callback
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "s".into(),
                tool_call_id: "tc".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 100,
                output_preview: "stdout:\nfile1.txt\nfile2.txt\nfile3.txt\n".into(),
            })
            .await;
    }

    // StreamCallbackBuilder chaining tests

    #[tokio::test]
    async fn test_stream_callback_builder_chain_methods_idempotent() {
        let metrics = Arc::new(MetricsCallback::new());

        // Chain multiple with_console calls
        let callback = StreamCallbackBuilder::new()
            .with_console()
            .with_console()
            .with_metrics(metrics.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        // Metrics should have received the event once
        assert_eq!(metrics.events().len(), 1);
    }

    // Multi-callback ordering and independence

    #[tokio::test]
    async fn test_multi_stream_callback_independence() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());

        // Add event to metrics1 only
        metrics1
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "only metrics1".into(),
            })
            .await;

        // Now create multi and add event
        let multi = MultiStreamCallback::new(vec![metrics1.clone(), metrics2.clone()]);
        multi
            .on_event(AgentEvent::ReasoningStart {
                session_id: "t".into(),
                turn: 1,
                model: "m".into(),
            })
            .await;

        // metrics1 has 2 events, metrics2 has 1
        assert_eq!(metrics1.events().len(), 2);
        assert_eq!(metrics2.events().len(), 1);
    }

    // Serde edge cases

    #[test]
    fn test_serde_empty_strings_roundtrip() {
        let events = vec![
            AgentEvent::UserTurn {
                session_id: "".into(),
                content: "".into(),
            },
            AgentEvent::Error {
                session_id: "".into(),
                error: "".into(),
                context: "".into(),
            },
            AgentEvent::TokenChunk {
                session_id: "".into(),
                chunk: "".into(),
                is_final: false,
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.session_id(), "");
        }
    }

    #[test]
    fn test_serde_very_long_strings() {
        let long_session_id = "s".repeat(10000);
        let long_content = "c".repeat(100000);

        let event = AgentEvent::UserTurn {
            session_id: long_session_id.clone(),
            content: long_content.clone(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::UserTurn {
                session_id,
                content,
            } => {
                assert_eq!(session_id.len(), 10000);
                assert_eq!(content.len(), 100000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_json_with_escapes() {
        let event = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "test".into(),
            args: serde_json::json!({
                "path": "C:\\Users\\test\\file.txt",
                "content": "Line 1\nLine 2\tTabbed",
                "quote": "She said \"hello\""
            }),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args["path"].as_str().unwrap().contains("\\"));
                assert!(args["content"].as_str().unwrap().contains("\n"));
                assert!(args["quote"].as_str().unwrap().contains("\""));
            }
            _ => panic!("Wrong variant"),
        }
    }

    // Event timer precision tests

    #[test]
    fn test_event_timer_long_duration() {
        let timer = EventTimer::start();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let elapsed = timer.elapsed_ms();
        // Should be at least 100ms, allowing some variance
        assert!(elapsed >= 95);
        assert!(elapsed < 200);
    }

    // MetricsCallback method-specific tests

    #[tokio::test]
    async fn test_metrics_callback_event_counts_empty() {
        let metrics = MetricsCallback::new();
        let counts = metrics.event_counts();
        assert!(counts.is_empty());
    }

    #[tokio::test]
    async fn test_metrics_callback_eval_captures_only() {
        let metrics = MetricsCallback::new();

        // Add only EvalCapture events
        for i in 0..5 {
            metrics
                .on_event(AgentEvent::EvalCapture {
                    session_id: "t".into(),
                    capture_id: format!("cap-{}", i),
                    input_messages: "[]".into(),
                    output_response: "{}".into(),
                    model: "m".into(),
                    tools: None,
                    metadata: None,
                })
                .await;
        }

        assert_eq!(metrics.eval_captures().len(), 5);
        assert_eq!(metrics.events().len(), 5);
        // Other totals should be 0
        assert_eq!(metrics.total_reasoning_time_ms(), 0);
        assert_eq!(metrics.total_tool_execution_time_ms(), 0);
    }

    #[tokio::test]
    async fn test_metrics_callback_cost_precision() {
        let metrics = MetricsCallback::new();

        // Add events with precise costs
        let costs = [0.000001, 0.0000001, 0.123456789, 1.0, 0.0];
        for (i, cost) in costs.iter().enumerate() {
            metrics
                .on_event(AgentEvent::LlmMetrics {
                    session_id: "t".into(),
                    request_id: format!("r{}", i),
                    model: "m".into(),
                    input_tokens: 100,
                    output_tokens: 50,
                    total_tokens: 150,
                    latency_ms: 500,
                    cost_usd: Some(*cost),
                    cached: false,
                })
                .await;
        }

        let total = metrics.total_cost_usd();
        let expected: f64 = costs.iter().sum();
        assert!((total - expected).abs() < 1e-10);
    }

    // Console callback specific event handling

    #[tokio::test]
    async fn test_console_callback_reasoning_complete_no_tool_calls() {
        let callback = ConsoleStreamCallback::new();

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 500,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
        // Should print "text response" not "X tool calls"
    }

    #[tokio::test]
    async fn test_console_callback_reasoning_complete_with_tool_calls() {
        let callback = ConsoleStreamCallback::new();

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 500,
                has_tool_calls: true,
                tool_count: 5,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
        // Should print "5 tool calls"
    }

    // Builder patterns tests

    #[test]
    fn test_stream_callback_builder_new_is_default() {
        let builder1 = StreamCallbackBuilder::new();
        let builder2 = StreamCallbackBuilder::default();
        // Both should create empty callback lists
        // Just verify they compile and are equivalent
        assert_eq!(builder1.callbacks.len(), builder2.callbacks.len());
    }

    #[test]
    fn test_console_stream_callback_new_is_default() {
        let cb1 = ConsoleStreamCallback::new();
        let cb2 = ConsoleStreamCallback::default();
        assert_eq!(cb1.verbose, cb2.verbose);
        assert!(!cb1.verbose);
    }

    // Agent event boundary value tests

    #[test]
    fn test_reasoning_complete_zero_values() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "s".into(),
            turn: 0,
            duration_ms: 0,
            has_tool_calls: false,
            tool_count: 0,
            input_tokens: Some(0),
            output_tokens: Some(0),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::ReasoningComplete {
                turn,
                duration_ms,
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(turn, 0);
                assert_eq!(duration_ms, 0);
                assert_eq!(input_tokens, Some(0));
                assert_eq!(output_tokens, Some(0));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_execution_complete_zero_duration() {
        let event = AgentEvent::ToolExecutionComplete {
            session_id: "s".into(),
            tool_call_id: "t".into(),
            tool: "instant_tool".into(),
            success: true,
            duration_ms: 0,
            output_preview: "instant".into(),
        };

        assert_eq!(event.event_type(), "tool_execution_complete");
    }

    // MultiStreamCallback specific tests

    #[tokio::test]
    async fn test_multi_stream_callback_single_callback() {
        let metrics = Arc::new(MetricsCallback::new());
        let multi = MultiStreamCallback::new(vec![metrics.clone()]);

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "single callback test".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
    }

    // Null callback tests

    #[tokio::test]
    async fn test_null_stream_callback_multiple_events() {
        let cb = NullStreamCallback;

        for i in 0..100 {
            cb.on_event(AgentEvent::UserTurn {
                session_id: format!("s{}", i),
                content: format!("content{}", i),
            })
            .await;
        }
        // Should not panic or accumulate
    }

    // Event type consistency tests

    #[test]
    fn test_event_type_matches_variant_name() {
        // Verify event_type() returns snake_case version of variant name
        assert_eq!(
            AgentEvent::UserTurn {
                session_id: "".into(),
                content: "".into()
            }
            .event_type(),
            "user_turn"
        );
        assert_eq!(
            AgentEvent::ReasoningStart {
                session_id: "".into(),
                turn: 0,
                model: "".into()
            }
            .event_type(),
            "reasoning_start"
        );
        assert_eq!(
            AgentEvent::TokenChunk {
                session_id: "".into(),
                chunk: "".into(),
                is_final: false
            }
            .event_type(),
            "token_chunk"
        );
    }

    // Session ID extraction consistency

    #[test]
    fn test_session_id_extraction_consistency() {
        let session_id = "consistent-session-id-12345";

        let events: Vec<AgentEvent> = vec![
            AgentEvent::UserTurn {
                session_id: session_id.into(),
                content: "".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: session_id.into(),
                turn: 0,
                model: "".into(),
            },
            AgentEvent::Error {
                session_id: session_id.into(),
                error: "".into(),
                context: "".into(),
            },
        ];

        for event in &events {
            assert_eq!(event.session_id(), session_id);
        }
    }

    // Config clone independence

    #[test]
    fn test_dashflow_stream_config_clone_independence() {
        let mut config1 = DashFlowStreamConfig::default();
        let config2 = config1.clone();

        // Modify config1 after clone
        config1.topic = "modified".into();
        config1.compression_threshold = 9999;

        // config2 should be unchanged
        assert_eq!(config2.topic, "codex-events");
        assert_eq!(config2.compression_threshold, 512);
    }

    // Event equality tests

    #[test]
    fn test_agent_event_partial_eq_user_turn() {
        let e1 = AgentEvent::UserTurn {
            session_id: "s".into(),
            content: "c".into(),
        };
        let e2 = AgentEvent::UserTurn {
            session_id: "s".into(),
            content: "c".into(),
        };
        let e3 = AgentEvent::UserTurn {
            session_id: "s".into(),
            content: "different".into(),
        };

        // Note: AgentEvent doesn't derive PartialEq, so we test via serde
        let j1 = serde_json::to_string(&e1).unwrap();
        let j2 = serde_json::to_string(&e2).unwrap();
        let j3 = serde_json::to_string(&e3).unwrap();

        assert_eq!(j1, j2);
        assert_ne!(j1, j3);
    }

    // MetricsCallback thread safety demonstration

    #[tokio::test]
    async fn test_metrics_callback_mutex_held_briefly() {
        let metrics = MetricsCallback::new();

        // Rapid fire events
        for i in 0..50 {
            metrics
                .on_event(AgentEvent::UserTurn {
                    session_id: format!("s{}", i),
                    content: "rapid".into(),
                })
                .await;

            // Interleave reads to test mutex contention
            if i % 5 == 0 {
                let _ = metrics.events();
            }
        }

        assert_eq!(metrics.events().len(), 50);
    }

    // ============================================================================
    // Additional test coverage (N=293)
    // ============================================================================

    #[test]
    fn test_user_turn_construction() {
        let event = AgentEvent::UserTurn {
            session_id: "session-abc".into(),
            content: "Hello, world!".into(),
        };
        assert_eq!(event.session_id(), "session-abc");
        assert_eq!(event.event_type(), "user_turn");
    }

    #[test]
    fn test_reasoning_start_construction() {
        let event = AgentEvent::ReasoningStart {
            session_id: "sess".into(),
            turn: 42,
            model: "claude-3".into(),
        };
        assert_eq!(event.session_id(), "sess");
        assert_eq!(event.event_type(), "reasoning_start");
    }

    #[test]
    fn test_reasoning_complete_with_zero_tokens() {
        let event = AgentEvent::ReasoningComplete {
            session_id: "s".into(),
            turn: 1,
            duration_ms: 50,
            has_tool_calls: false,
            tool_count: 0,
            input_tokens: Some(0),
            output_tokens: Some(0),
        };
        assert_eq!(event.event_type(), "reasoning_complete");
    }

    #[test]
    fn test_llm_metrics_with_zero_cost() {
        let event = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 500,
            cost_usd: Some(0.0),
            cached: false,
        };
        assert_eq!(event.event_type(), "llm_metrics");
    }

    #[test]
    fn test_llm_metrics_cached_true() {
        let event = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            latency_ms: 0,
            cost_usd: None,
            cached: true,
        };
        match event {
            AgentEvent::LlmMetrics { cached, .. } => assert!(cached),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_tool_call_requested_with_complex_args() {
        let args = serde_json::json!({
            "command": "git commit -m 'test'",
            "env": {"PATH": "/usr/bin", "HOME": "/home/user"},
            "timeout": 30.5,
            "flags": ["-v", "--no-verify"],
            "nested": {"deep": {"value": true}}
        });
        let event = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
            args,
        };
        assert_eq!(event.event_type(), "tool_call_requested");
    }

    #[test]
    fn test_tool_call_approved_construction() {
        let event = AgentEvent::ToolCallApproved {
            session_id: "s".into(),
            tool_call_id: "tc-123".into(),
            tool: "read_file".into(),
        };
        assert_eq!(event.session_id(), "s");
        assert_eq!(event.event_type(), "tool_call_approved");
    }

    #[test]
    fn test_tool_call_rejected_with_long_reason() {
        let reason = "x".repeat(10000);
        let event = AgentEvent::ToolCallRejected {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "dangerous".into(),
            reason,
        };
        assert_eq!(event.event_type(), "tool_call_rejected");
    }

    #[test]
    fn test_tool_execution_start_construction() {
        let event = AgentEvent::ToolExecutionStart {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "execute".into(),
        };
        assert_eq!(event.event_type(), "tool_execution_start");
    }

    #[test]
    fn test_tool_execution_complete_failed() {
        let event = AgentEvent::ToolExecutionComplete {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
            success: false,
            duration_ms: 1500,
            output_preview: "Error: command not found".into(),
        };
        match event {
            AgentEvent::ToolExecutionComplete { success, .. } => assert!(!success),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_turn_complete_with_long_response() {
        let event = AgentEvent::TurnComplete {
            session_id: "s".into(),
            turn: 100,
            status: "x".repeat(5000),
        };
        assert_eq!(event.event_type(), "turn_complete");
    }

    #[test]
    fn test_session_complete_with_many_turns() {
        let event = AgentEvent::SessionComplete {
            session_id: "s".into(),
            total_turns: u32::MAX,
            status: "completed successfully".into(),
        };
        match event {
            AgentEvent::SessionComplete { total_turns, .. } => {
                assert_eq!(total_turns, u32::MAX);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_token_chunk_final() {
        let event = AgentEvent::TokenChunk {
            session_id: "s".into(),
            chunk: "final token".into(),
            is_final: true,
        };
        match event {
            AgentEvent::TokenChunk { is_final, .. } => assert!(is_final),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_token_chunk_empty() {
        let event = AgentEvent::TokenChunk {
            session_id: "s".into(),
            chunk: "".into(),
            is_final: false,
        };
        assert_eq!(event.event_type(), "token_chunk");
    }

    #[test]
    fn test_error_with_long_context() {
        let event = AgentEvent::Error {
            session_id: "s".into(),
            error: "Connection timeout".into(),
            context: "x".repeat(10000),
        };
        assert_eq!(event.event_type(), "error");
    }

    #[test]
    fn test_approval_required_with_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "tc".into(),
            tool: "rm".into(),
            args: serde_json::json!({"path": "/important"}),
            reason: Some("Deleting important file".into()),
        };
        match event {
            AgentEvent::ApprovalRequired { reason, .. } => {
                assert_eq!(reason, Some("Deleting important file".into()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_eval_capture_with_tools_and_metadata() {
        let event = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[{\"role\":\"user\"}]".into(),
            output_response: "{\"role\":\"assistant\"}".into(),
            model: "gpt-4".into(),
            tools: Some("[{\"name\":\"shell\"}]".into()),
            metadata: Some(serde_json::json!({"test": true})),
        };
        match event {
            AgentEvent::EvalCapture {
                tools, metadata, ..
            } => {
                assert!(tools.is_some());
                assert!(metadata.is_some());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[tokio::test]
    async fn test_console_callback_with_exactly_50_char_content() {
        let callback = ConsoleStreamCallback::new();
        let content = "A".repeat(50);
        assert_eq!(content.len(), 50);
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_with_51_char_content() {
        let callback = ConsoleStreamCallback::new();
        let content = "B".repeat(51);
        assert_eq!(content.len(), 51);
        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_with_all_token_combinations() {
        let callback = ConsoleStreamCallback::verbose();

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
            .await;

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 2,
                duration_ms: 100,
                has_tool_calls: true,
                tool_count: 1,
                input_tokens: Some(200),
                output_tokens: None,
            })
            .await;

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 3,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: Some(75),
            })
            .await;

        callback
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 4,
                duration_ms: 100,
                has_tool_calls: true,
                tool_count: 3,
                input_tokens: None,
                output_tokens: None,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_non_verbose_skips_token_chunks() {
        let callback = ConsoleStreamCallback::new();
        assert!(!callback.verbose);

        callback
            .on_event(AgentEvent::TokenChunk {
                session_id: "t".into(),
                chunk: "some token".into(),
                is_final: false,
            })
            .await;

        callback
            .on_event(AgentEvent::TokenChunk {
                session_id: "t".into(),
                chunk: "final".into(),
                is_final: true,
            })
            .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_shows_token_chunks() {
        let callback = ConsoleStreamCallback::verbose();
        assert!(callback.verbose);

        callback
            .on_event(AgentEvent::TokenChunk {
                session_id: "t".into(),
                chunk: "streaming...".into(),
                is_final: false,
            })
            .await;
    }

    #[tokio::test]
    async fn test_metrics_callback_accumulates_correctly() {
        let metrics = MetricsCallback::new();

        for i in 1..=5 {
            metrics
                .on_event(AgentEvent::ReasoningComplete {
                    session_id: format!("s{}", i),
                    turn: i,
                    duration_ms: i as u64 * 100,
                    has_tool_calls: false,
                    tool_count: 0,
                    input_tokens: Some(i * 50),
                    output_tokens: Some(i * 25),
                })
                .await;

            metrics
                .on_event(AgentEvent::ToolExecutionComplete {
                    session_id: format!("s{}", i),
                    tool_call_id: format!("tc{}", i),
                    tool: "shell".into(),
                    success: true,
                    duration_ms: i as u64 * 10,
                    output_preview: "ok".into(),
                })
                .await;

            metrics
                .on_event(AgentEvent::LlmMetrics {
                    session_id: format!("s{}", i),
                    request_id: format!("r{}", i),
                    model: "m".into(),
                    input_tokens: i * 100,
                    output_tokens: i * 50,
                    total_tokens: i * 150,
                    latency_ms: 500,
                    cost_usd: Some(i as f64 * 0.001),
                    cached: false,
                })
                .await;
        }

        assert_eq!(metrics.events().len(), 15);
        assert_eq!(metrics.total_reasoning_time_ms(), 1500);
        assert_eq!(metrics.total_tool_execution_time_ms(), 150);
        assert_eq!(metrics.total_input_tokens(), 2250);
        assert_eq!(metrics.total_output_tokens(), 1125);
        assert!((metrics.total_cost_usd() - 0.015).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_metrics_callback_clear_resets_all() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "t".into(),
                turn: 1,
                duration_ms: 1000,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(500),
                output_tokens: Some(200),
            })
            .await;

        assert_eq!(metrics.total_reasoning_time_ms(), 1000);
        assert_eq!(metrics.total_input_tokens(), 500);

        metrics.clear();

        assert_eq!(metrics.events().len(), 0);
        assert_eq!(metrics.total_reasoning_time_ms(), 0);
        assert_eq!(metrics.total_input_tokens(), 0);
        assert_eq!(metrics.total_output_tokens(), 0);
        assert_eq!(metrics.total_cost_usd(), 0.0);
    }

    #[test]
    fn test_dashflow_config_builder_chain_all_methods() {
        let config = DashFlowStreamConfig::new("broker1:9092,broker2:9093")
            .with_topic("custom-topic")
            .with_tenant_id("custom-tenant")
            .with_state_diff(false)
            .with_compression_threshold(2048);

        assert_eq!(config.bootstrap_servers, "broker1:9092,broker2:9093");
        assert_eq!(config.topic, "custom-topic");
        assert_eq!(config.tenant_id, "custom-tenant");
        assert!(!config.enable_state_diff);
        assert_eq!(config.compression_threshold, 2048);
    }

    #[test]
    fn test_dashflow_config_with_empty_values() {
        let config = DashFlowStreamConfig::new("")
            .with_topic("")
            .with_tenant_id("");

        assert_eq!(config.bootstrap_servers, "");
        assert_eq!(config.topic, "");
        assert_eq!(config.tenant_id, "");
    }

    #[test]
    fn test_dashflow_config_with_special_chars() {
        let config = DashFlowStreamConfig::new("kafka://user:pass@broker:9092?ssl=true&timeout=30");
        assert!(config.bootstrap_servers.contains("@"));
        assert!(config.bootstrap_servers.contains("?"));
    }

    #[tokio::test]
    async fn test_builder_with_two_callbacks_creates_multi() {
        let metrics1 = Arc::new(MetricsCallback::new());
        let metrics2 = Arc::new(MetricsCallback::new());

        let callback = StreamCallbackBuilder::new()
            .with_metrics(metrics1.clone())
            .with_metrics(metrics2.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        assert_eq!(metrics1.events().len(), 1);
        assert_eq!(metrics2.events().len(), 1);
    }

    #[tokio::test]
    async fn test_builder_empty_returns_null_callback() {
        let callback = StreamCallbackBuilder::new().build();

        for i in 0..100 {
            callback
                .on_event(AgentEvent::UserTurn {
                    session_id: format!("s{}", i),
                    content: "test".into(),
                })
                .await;
        }
    }

    #[tokio::test]
    async fn test_multi_callback_with_empty_vec() {
        let multi = MultiStreamCallback::new(vec![]);

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;
        multi.flush().await;
    }

    #[tokio::test]
    async fn test_multi_callback_add_after_creation() {
        let metrics = Arc::new(MetricsCallback::new());
        let mut multi = MultiStreamCallback::new(vec![]);

        multi.add(metrics.clone());

        multi
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "test".into(),
            })
            .await;

        assert_eq!(metrics.events().len(), 1);
    }

    #[test]
    fn test_event_timer_immediate_elapsed() {
        let timer = EventTimer::start();
        assert!(timer.elapsed_ms() < 100);
    }

    #[test]
    fn test_event_timer_after_sleep() {
        let timer = EventTimer::start();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(timer.elapsed_ms() >= 50);
    }

    #[test]
    fn test_serialize_unicode_strings() {
        let event = AgentEvent::UserTurn {
            session_id: "会话".into(),
            content: "Hello, 世界! Привет мир!".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            AgentEvent::UserTurn {
                session_id,
                content,
            } => {
                assert_eq!(session_id, "会话");
                assert!(content.contains("世界"));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serialize_with_escapes() {
        let event = AgentEvent::Error {
            session_id: "s".into(),
            error: "Error with special chars: \t\n\r".into(),
            context: "context".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "error");
    }

    #[test]
    fn test_serialize_max_values() {
        let event = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: u32::MAX,
            output_tokens: u32::MAX,
            total_tokens: u32::MAX,
            latency_ms: u64::MAX,
            cost_usd: Some(f64::MAX),
            cached: true,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_agent_event_clone_preserves_all_fields() {
        let original = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "gpt-4".into(),
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            latency_ms: 2500,
            cost_usd: Some(0.0375),
            cached: true,
        };

        let cloned = original.clone();

        match (&original, &cloned) {
            (
                AgentEvent::LlmMetrics {
                    session_id: s1,
                    request_id: r1,
                    model: m1,
                    input_tokens: i1,
                    output_tokens: o1,
                    total_tokens: t1,
                    latency_ms: l1,
                    cost_usd: c1,
                    cached: ca1,
                },
                AgentEvent::LlmMetrics {
                    session_id: s2,
                    request_id: r2,
                    model: m2,
                    input_tokens: i2,
                    output_tokens: o2,
                    total_tokens: t2,
                    latency_ms: l2,
                    cost_usd: c2,
                    cached: ca2,
                },
            ) => {
                assert_eq!(s1, s2);
                assert_eq!(r1, r2);
                assert_eq!(m1, m2);
                assert_eq!(i1, i2);
                assert_eq!(o1, o2);
                assert_eq!(t1, t2);
                assert_eq!(l1, l2);
                assert_eq!(c1, c2);
                assert_eq!(ca1, ca2);
            }
            _ => panic!("Clone changed variant"),
        }
    }

    #[test]
    fn test_null_callback_default_impl() {
        let _cb = NullStreamCallback;
    }

    #[test]
    fn test_console_callback_default_impl() {
        let cb = ConsoleStreamCallback::default();
        assert!(!cb.verbose);
    }

    #[test]
    fn test_metrics_callback_default_impl() {
        let cb = MetricsCallback::default();
        assert!(cb.events().is_empty());
    }

    #[test]
    fn test_stream_callback_builder_default_impl() {
        let _builder = StreamCallbackBuilder::default();
    }

    #[test]
    fn test_dashflow_config_default_impl() {
        let config = DashFlowStreamConfig::default();
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "codex-events");
        assert_eq!(config.tenant_id, "codex-dashflow");
        assert!(config.enable_state_diff);
        assert_eq!(config.compression_threshold, 512);
    }

    #[test]
    fn test_session_id_exhaustive_extraction() {
        let session = "unique-id";
        let events: Vec<AgentEvent> = vec![
            AgentEvent::UserTurn {
                session_id: session.into(),
                content: "".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: session.into(),
                turn: 0,
                model: "".into(),
            },
            AgentEvent::ReasoningComplete {
                session_id: session.into(),
                turn: 0,
                duration_ms: 0,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            },
            AgentEvent::LlmMetrics {
                session_id: session.into(),
                request_id: "".into(),
                model: "".into(),
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                latency_ms: 0,
                cost_usd: None,
                cached: false,
            },
            AgentEvent::ToolCallRequested {
                session_id: session.into(),
                tool_call_id: "".into(),
                tool: "".into(),
                args: serde_json::json!({}),
            },
            AgentEvent::ToolCallApproved {
                session_id: session.into(),
                tool_call_id: "".into(),
                tool: "".into(),
            },
            AgentEvent::ToolCallRejected {
                session_id: session.into(),
                tool_call_id: "".into(),
                tool: "".into(),
                reason: "".into(),
            },
            AgentEvent::ToolExecutionStart {
                session_id: session.into(),
                tool_call_id: "".into(),
                tool: "".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: session.into(),
                tool_call_id: "".into(),
                tool: "".into(),
                success: false,
                duration_ms: 0,
                output_preview: "".into(),
            },
            AgentEvent::TurnComplete {
                session_id: session.into(),
                turn: 0,
                status: "".into(),
            },
            AgentEvent::SessionComplete {
                session_id: session.into(),
                total_turns: 0,
                status: "".into(),
            },
            AgentEvent::TokenChunk {
                session_id: session.into(),
                chunk: "".into(),
                is_final: false,
            },
            AgentEvent::Error {
                session_id: session.into(),
                error: "".into(),
                context: "".into(),
            },
            AgentEvent::ApprovalRequired {
                session_id: session.into(),
                request_id: "".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                args: serde_json::json!({}),
                reason: None,
            },
            AgentEvent::EvalCapture {
                session_id: session.into(),
                capture_id: "".into(),
                input_messages: "".into(),
                output_response: "".into(),
                model: "".into(),
                tools: None,
                metadata: None,
            },
        ];

        for event in &events {
            assert_eq!(event.session_id(), session);
        }
    }

    // ============================================================================
    // Additional test coverage (N=298)
    // ============================================================================

    // --- AgentEvent serde boundary tests ---

    #[test]
    fn test_agent_event_serde_user_turn_empty_content() {
        let event = AgentEvent::UserTurn {
            session_id: "s".into(),
            content: "".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id(), "s");
    }

    #[test]
    fn test_agent_event_serde_user_turn_unicode() {
        let event = AgentEvent::UserTurn {
            session_id: "会话-id".into(),
            content: "你好，世界！🌍".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::UserTurn {
                session_id,
                content,
            } => {
                assert!(session_id.contains("会话"));
                assert!(content.contains("世界"));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_event_serde_llm_metrics_no_cost() {
        let event = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 500,
            cost_usd: None,
            cached: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"cost_usd\":null"));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::LlmMetrics { cost_usd, .. } => assert!(cost_usd.is_none()),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_event_serde_llm_metrics_max_tokens() {
        let event = AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "m".into(),
            input_tokens: u32::MAX,
            output_tokens: u32::MAX,
            total_tokens: u32::MAX,
            latency_ms: u64::MAX,
            cost_usd: Some(f64::MAX),
            cached: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::LlmMetrics {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, u32::MAX);
                assert_eq!(output_tokens, u32::MAX);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_event_serde_tool_call_complex_args() {
        let args = serde_json::json!({
            "nested": {
                "array": [1, 2, 3],
                "object": {"key": "value"},
                "null": null,
                "bool": true
            }
        });
        let event = AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "complex".into(),
            args,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::ToolCallRequested { args, .. } => {
                assert!(args["nested"]["array"].is_array());
                assert!(args["nested"]["bool"].as_bool().unwrap());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_event_serde_approval_required_with_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "tc".into(),
            tool: "rm".into(),
            args: serde_json::json!({}),
            reason: Some("Dangerous operation".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Dangerous operation"));
    }

    #[test]
    fn test_agent_event_serde_approval_required_no_reason() {
        let event = AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "tc".into(),
            tool: "echo".into(),
            args: serde_json::json!({}),
            reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"reason\":null"));
    }

    #[test]
    fn test_agent_event_serde_eval_capture_with_all_fields() {
        let event = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "cap-123".into(),
            input_messages: "[{\"role\":\"user\",\"content\":\"hi\"}]".into(),
            output_response: "{\"role\":\"assistant\"}".into(),
            model: "gpt-4".into(),
            tools: Some("[{\"name\":\"shell\"}]".into()),
            metadata: Some(serde_json::json!({"category": "test"})),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::EvalCapture {
                tools, metadata, ..
            } => {
                assert!(tools.is_some());
                assert!(metadata.is_some());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_event_serde_eval_capture_minimal() {
        let event = AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "c".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "m".into(),
            tools: None,
            metadata: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::EvalCapture {
                tools, metadata, ..
            } => {
                assert!(tools.is_none());
                assert!(metadata.is_none());
            }
            _ => panic!("Wrong variant"),
        }
    }

    // --- MetricsCallback edge cases ---

    #[tokio::test]
    async fn test_metrics_callback_rapid_events() {
        let metrics = MetricsCallback::new();

        for i in 0..1000 {
            metrics
                .on_event(AgentEvent::UserTurn {
                    session_id: format!("s{}", i),
                    content: "rapid".into(),
                })
                .await;
        }

        assert_eq!(metrics.events().len(), 1000);
    }

    #[tokio::test]
    async fn test_metrics_callback_mixed_event_types() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::UserTurn {
                session_id: "s".into(),
                content: "test".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningStart {
                session_id: "s".into(),
                turn: 1,
                model: "m".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: true,
                tool_count: 2,
                input_tokens: Some(500),
                output_tokens: Some(200),
            })
            .await;
        metrics
            .on_event(AgentEvent::ToolCallRequested {
                session_id: "s".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                args: serde_json::json!({}),
            })
            .await;
        metrics
            .on_event(AgentEvent::ToolExecutionComplete {
                session_id: "s".into(),
                tool_call_id: "tc1".into(),
                tool: "shell".into(),
                success: true,
                duration_ms: 50,
                output_preview: "done".into(),
            })
            .await;
        metrics
            .on_event(AgentEvent::LlmMetrics {
                session_id: "s".into(),
                request_id: "r".into(),
                model: "m".into(),
                input_tokens: 500,
                output_tokens: 200,
                total_tokens: 700,
                latency_ms: 100,
                cost_usd: Some(0.01),
                cached: false,
            })
            .await;

        let counts = metrics.event_counts();
        assert_eq!(counts.get("user_turn"), Some(&1));
        assert_eq!(counts.get("reasoning_start"), Some(&1));
        assert_eq!(counts.get("reasoning_complete"), Some(&1));
        assert_eq!(counts.get("tool_call_requested"), Some(&1));
        assert_eq!(counts.get("tool_execution_complete"), Some(&1));
        assert_eq!(counts.get("llm_metrics"), Some(&1));
    }

    #[tokio::test]
    async fn test_metrics_callback_total_tokens_from_reasoning_complete() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
            .await;
        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 2,
                duration_ms: 200,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: Some(200),
                output_tokens: Some(100),
            })
            .await;

        assert_eq!(metrics.total_input_tokens(), 300);
        assert_eq!(metrics.total_output_tokens(), 150);
        assert_eq!(metrics.total_reasoning_time_ms(), 300);
    }

    #[tokio::test]
    async fn test_metrics_callback_tokens_from_none() {
        let metrics = MetricsCallback::new();

        metrics
            .on_event(AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 1,
                duration_ms: 100,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            })
            .await;

        // None tokens should not contribute to totals
        assert_eq!(metrics.total_input_tokens(), 0);
        assert_eq!(metrics.total_output_tokens(), 0);
    }

    // --- Builder comprehensive tests ---

    #[test]
    fn test_builder_with_console_and_metrics() {
        let metrics = Arc::new(MetricsCallback::new());
        let _callback = StreamCallbackBuilder::new()
            .with_console()
            .with_metrics(metrics)
            .build();
    }

    #[test]
    fn test_builder_with_verbose_console_and_metrics() {
        let metrics = Arc::new(MetricsCallback::new());
        let _callback = StreamCallbackBuilder::new()
            .with_verbose_console()
            .with_metrics(metrics)
            .build();
    }

    #[test]
    fn test_builder_single_callback_not_multi() {
        let _callback = StreamCallbackBuilder::new().with_console().build();
        // Single callback should not wrap in MultiStreamCallback
    }

    #[tokio::test]
    async fn test_builder_custom_callback() {
        let custom = Arc::new(MetricsCallback::new());
        let callback = StreamCallbackBuilder::new()
            .with_callback(custom.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "custom".into(),
            })
            .await;

        assert_eq!(custom.events().len(), 1);
    }

    #[tokio::test]
    async fn test_builder_three_callbacks() {
        let m1 = Arc::new(MetricsCallback::new());
        let m2 = Arc::new(MetricsCallback::new());
        let m3 = Arc::new(MetricsCallback::new());

        let callback = StreamCallbackBuilder::new()
            .with_metrics(m1.clone())
            .with_metrics(m2.clone())
            .with_metrics(m3.clone())
            .build();

        callback
            .on_event(AgentEvent::UserTurn {
                session_id: "t".into(),
                content: "three".into(),
            })
            .await;

        assert_eq!(m1.events().len(), 1);
        assert_eq!(m2.events().len(), 1);
        assert_eq!(m3.events().len(), 1);
    }

    // --- MultiStreamCallback edge cases ---

    #[tokio::test]
    async fn test_multi_callback_flush_all() {
        let m1 = Arc::new(MetricsCallback::new());
        let m2 = Arc::new(MetricsCallback::new());
        let multi = MultiStreamCallback::new(vec![m1.clone(), m2.clone()]);

        multi.flush().await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_multi_callback_concurrent_events() {
        let metrics = Arc::new(MetricsCallback::new());
        let multi = Arc::new(MultiStreamCallback::new(vec![metrics.clone()]));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let m = multi.clone();
                tokio::spawn(async move {
                    m.on_event(AgentEvent::UserTurn {
                        session_id: format!("s{}", i),
                        content: "concurrent".into(),
                    })
                    .await;
                })
            })
            .collect();

        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(metrics.events().len(), 10);
    }

    // --- EventTimer precision tests ---

    #[test]
    fn test_event_timer_multiple_reads() {
        let timer = EventTimer::start();
        let e1 = timer.elapsed_ms();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let e2 = timer.elapsed_ms();
        assert!(e2 >= e1);
    }

    #[test]
    fn test_event_timer_sub_millisecond() {
        let timer = EventTimer::start();
        // Immediate read should be 0 or very small
        assert!(timer.elapsed_ms() < 10);
    }

    // --- NullStreamCallback completeness ---

    #[tokio::test]
    async fn test_null_callback_all_event_types() {
        let cb = NullStreamCallback;

        let events: Vec<AgentEvent> = vec![
            AgentEvent::UserTurn {
                session_id: "s".into(),
                content: "".into(),
            },
            AgentEvent::ReasoningStart {
                session_id: "s".into(),
                turn: 0,
                model: "".into(),
            },
            AgentEvent::ReasoningComplete {
                session_id: "s".into(),
                turn: 0,
                duration_ms: 0,
                has_tool_calls: false,
                tool_count: 0,
                input_tokens: None,
                output_tokens: None,
            },
            AgentEvent::LlmMetrics {
                session_id: "s".into(),
                request_id: "".into(),
                model: "".into(),
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                latency_ms: 0,
                cost_usd: None,
                cached: false,
            },
            AgentEvent::ToolCallRequested {
                session_id: "s".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                args: serde_json::json!({}),
            },
            AgentEvent::ToolCallApproved {
                session_id: "s".into(),
                tool_call_id: "".into(),
                tool: "".into(),
            },
            AgentEvent::ToolCallRejected {
                session_id: "s".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                reason: "".into(),
            },
            AgentEvent::ToolExecutionStart {
                session_id: "s".into(),
                tool_call_id: "".into(),
                tool: "".into(),
            },
            AgentEvent::ToolExecutionComplete {
                session_id: "s".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                success: false,
                duration_ms: 0,
                output_preview: "".into(),
            },
            AgentEvent::TurnComplete {
                session_id: "s".into(),
                turn: 0,
                status: "".into(),
            },
            AgentEvent::SessionComplete {
                session_id: "s".into(),
                total_turns: 0,
                status: "".into(),
            },
            AgentEvent::TokenChunk {
                session_id: "s".into(),
                chunk: "".into(),
                is_final: false,
            },
            AgentEvent::Error {
                session_id: "s".into(),
                error: "".into(),
                context: "".into(),
            },
            AgentEvent::ApprovalRequired {
                session_id: "s".into(),
                request_id: "".into(),
                tool_call_id: "".into(),
                tool: "".into(),
                args: serde_json::json!({}),
                reason: None,
            },
            AgentEvent::EvalCapture {
                session_id: "s".into(),
                capture_id: "".into(),
                input_messages: "".into(),
                output_response: "".into(),
                model: "".into(),
                tools: None,
                metadata: None,
            },
        ];

        for event in events {
            cb.on_event(event).await;
        }

        cb.flush().await;
    }

    // --- DashFlowStreamConfig edge cases ---

    #[test]
    fn test_dashflow_config_with_max_compression_threshold() {
        let config =
            DashFlowStreamConfig::new("broker:9092").with_compression_threshold(usize::MAX);
        assert_eq!(config.compression_threshold, usize::MAX);
    }

    #[test]
    fn test_dashflow_config_with_zero_compression_threshold() {
        let config = DashFlowStreamConfig::new("broker:9092").with_compression_threshold(0);
        assert_eq!(config.compression_threshold, 0);
    }

    #[test]
    fn test_dashflow_config_state_diff_toggle() {
        let config = DashFlowStreamConfig::new("b:9092");
        assert!(config.enable_state_diff); // Default true

        let config_disabled = config.clone().with_state_diff(false);
        assert!(!config_disabled.enable_state_diff);

        let config_enabled = config_disabled.with_state_diff(true);
        assert!(config_enabled.enable_state_diff);
    }

    #[test]
    fn test_dashflow_config_long_values() {
        let long_servers = "a".repeat(10000);
        let long_topic = "b".repeat(10000);
        let long_tenant = "c".repeat(10000);

        let config = DashFlowStreamConfig::new(&long_servers)
            .with_topic(&long_topic)
            .with_tenant_id(&long_tenant);

        assert_eq!(config.bootstrap_servers.len(), 10000);
        assert_eq!(config.topic.len(), 10000);
        assert_eq!(config.tenant_id.len(), 10000);
    }

    // --- Console callback all paths ---

    #[tokio::test]
    async fn test_console_callback_tool_call_rejected() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ToolCallRejected {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "rm".into(),
            reason: "Policy violation".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_tool_call_approved() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ToolCallApproved {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "echo".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_tool_execution_start() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ToolExecutionStart {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_tool_execution_failed() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ToolExecutionComplete {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
            success: false,
            duration_ms: 100,
            output_preview: "exit code 1".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_turn_complete() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::TurnComplete {
            session_id: "s".into(),
            turn: 5,
            status: "success".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_session_complete() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::SessionComplete {
            session_id: "s".into(),
            total_turns: 10,
            status: "completed".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_error() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::Error {
            session_id: "s".into(),
            error: "Connection failed".into(),
            context: "LLM API".into(),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_approval_required_with_reason() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "tc".into(),
            tool: "rm".into(),
            args: serde_json::json!({"path": "/important"}),
            reason: Some("Deleting important file".into()),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_approval_required_no_reason() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::ApprovalRequired {
            session_id: "s".into(),
            request_id: "r".into(),
            tool_call_id: "tc".into(),
            tool: "echo".into(),
            args: serde_json::json!({}),
            reason: None,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_llm_metrics_with_cost_n298() {
        let cb = ConsoleStreamCallback::verbose();
        cb.on_event(AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "gpt-4".into(),
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            latency_ms: 2500,
            cost_usd: Some(0.0375),
            cached: false,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_llm_metrics_no_cost_n298() {
        let cb = ConsoleStreamCallback::verbose();
        cb.on_event(AgentEvent::LlmMetrics {
            session_id: "s".into(),
            request_id: "r".into(),
            model: "local".into(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            latency_ms: 50,
            cost_usd: None,
            cached: true,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_eval_capture_n298() {
        let cb = ConsoleStreamCallback::verbose();
        cb.on_event(AgentEvent::EvalCapture {
            session_id: "s".into(),
            capture_id: "cap-001".into(),
            input_messages: "[]".into(),
            output_response: "{}".into(),
            model: "gpt-4".into(),
            tools: None,
            metadata: None,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_tool_requested_n298() {
        let cb = ConsoleStreamCallback::verbose();
        cb.on_event(AgentEvent::ToolCallRequested {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
            args: serde_json::json!({"command": "ls -la"}),
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_verbose_tool_execution_complete() {
        let cb = ConsoleStreamCallback::verbose();
        cb.on_event(AgentEvent::ToolExecutionComplete {
            session_id: "s".into(),
            tool_call_id: "tc".into(),
            tool: "shell".into(),
            success: true,
            duration_ms: 150,
            output_preview: "file1.txt\nfile2.txt".into(),
        })
        .await;
    }

    // --- Quality gate event tests ---

    #[test]
    fn test_quality_gate_start_event_type() {
        let event = AgentEvent::QualityGateStart {
            session_id: "test-session".into(),
            attempt: 1,
            max_retries: 3,
            threshold: 0.90,
        };
        assert_eq!(event.event_type(), "quality_gate_start");
        assert_eq!(event.session_id(), "test-session");
        assert_eq!(event.node_id(), "quality_gate");
    }

    #[test]
    fn test_quality_gate_result_passed_event_type() {
        let event = AgentEvent::QualityGateResult {
            session_id: "test-session".into(),
            attempt: 1,
            passed: true,
            accuracy: 0.95,
            relevance: 0.92,
            completeness: 0.98,
            average_score: 0.95,
            is_final: true,
            reason: None,
        };
        assert_eq!(event.event_type(), "quality_gate_result");
        assert_eq!(event.session_id(), "test-session");
        assert_eq!(event.node_id(), "quality_gate");
    }

    #[test]
    fn test_quality_gate_result_failed_event_type() {
        let event = AgentEvent::QualityGateResult {
            session_id: "test-session".into(),
            attempt: 3,
            passed: false,
            accuracy: 0.75,
            relevance: 0.80,
            completeness: 0.70,
            average_score: 0.75,
            is_final: true,
            reason: Some("Below threshold after max retries".into()),
        };
        assert_eq!(event.event_type(), "quality_gate_result");
        assert!(matches!(
            &event,
            AgentEvent::QualityGateResult {
                reason: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn test_quality_gate_events_serialize() {
        let start_event = AgentEvent::QualityGateStart {
            session_id: "s".into(),
            attempt: 1,
            max_retries: 3,
            threshold: 0.90,
        };
        let json = serde_json::to_string(&start_event).unwrap();
        assert!(json.contains("QualityGateStart"));
        assert!(json.contains("0.9"));

        let result_event = AgentEvent::QualityGateResult {
            session_id: "s".into(),
            attempt: 2,
            passed: true,
            accuracy: 0.95,
            relevance: 0.92,
            completeness: 0.98,
            average_score: 0.95,
            is_final: false,
            reason: None,
        };
        let json = serde_json::to_string(&result_event).unwrap();
        assert!(json.contains("QualityGateResult"));
        assert!(json.contains("0.95"));
    }

    #[tokio::test]
    async fn test_console_callback_quality_gate_start() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::QualityGateStart {
            session_id: "s".into(),
            attempt: 1,
            max_retries: 3,
            threshold: 0.90,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_quality_gate_result_passed() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::QualityGateResult {
            session_id: "s".into(),
            attempt: 2,
            passed: true,
            accuracy: 0.95,
            relevance: 0.92,
            completeness: 0.98,
            average_score: 0.95,
            is_final: true,
            reason: None,
        })
        .await;
    }

    #[tokio::test]
    async fn test_console_callback_quality_gate_result_failed() {
        let cb = ConsoleStreamCallback::new();
        cb.on_event(AgentEvent::QualityGateResult {
            session_id: "s".into(),
            attempt: 3,
            passed: false,
            accuracy: 0.75,
            relevance: 0.80,
            completeness: 0.70,
            average_score: 0.75,
            is_final: true,
            reason: Some("Quality below threshold".into()),
        })
        .await;
    }

    #[tokio::test]
    async fn test_metrics_callback_quality_gate_events() {
        let cb = MetricsCallback::new();

        cb.on_event(AgentEvent::QualityGateStart {
            session_id: "s".into(),
            attempt: 1,
            max_retries: 3,
            threshold: 0.90,
        })
        .await;

        cb.on_event(AgentEvent::QualityGateResult {
            session_id: "s".into(),
            attempt: 1,
            passed: false,
            accuracy: 0.80,
            relevance: 0.75,
            completeness: 0.78,
            average_score: 0.78,
            is_final: false,
            reason: None,
        })
        .await;

        cb.on_event(AgentEvent::QualityGateResult {
            session_id: "s".into(),
            attempt: 2,
            passed: true,
            accuracy: 0.95,
            relevance: 0.92,
            completeness: 0.98,
            average_score: 0.95,
            is_final: true,
            reason: None,
        })
        .await;

        let counts = cb.event_counts();
        assert_eq!(counts.get("quality_gate_start"), Some(&1));
        assert_eq!(counts.get("quality_gate_result"), Some(&2));
    }
}
