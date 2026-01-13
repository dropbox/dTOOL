// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph execution engine
//!
//! The executor runs compiled graphs, traversing nodes and edges
//! according to the graph structure.
//!
//! # Module Structure
//!
//! - [`validation`] - Graph validation types (warnings, results)
//! - [`introspection`] - Executor introspection types for AI self-awareness

// Submodules
mod decision_context;
mod execution;
mod execution_hierarchy;
pub mod introspection;
mod trace;
pub mod validation;

// FIX-014: Decision tracking API for nodes
pub use decision_context::{
    record_decision, record_decision_with_context, record_outcome, record_outcome_with_details,
};
#[allow(unused)] // Architectural: Reserved for executor to initialize decision tracking context
pub(crate) use decision_context::init_decision_context;

// Internal re-exports from trace module
use trace::is_live_introspection_enabled;

pub(crate) fn current_execution_hierarchy_ids() -> Option<(String, Option<String>, Option<String>, u32)> {
    execution_hierarchy::current_ids().map(|ids| {
        (
            ids.execution_id,
            ids.parent_execution_id,
            ids.root_execution_id,
            ids.depth,
        )
    })
}

/// Get the current graph execution context (FIX-009).
///
/// Returns `None` if called outside of a graph execution context.
/// This is the public API for accessing execution hierarchy information
/// as documented in `reports/dashflow-observability-redesign.md`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::telemetry::GraphContext;
/// use dashflow::executor::current_graph_context;
///
/// // Inside a graph node
/// if let Some(ctx) = current_graph_context() {
///     println!("Running in execution: {}", ctx.execution_id);
///     if let Some(parent) = &ctx.parent_execution_id {
///         println!("Parent execution: {}", parent);
///     }
/// }
/// ```
pub fn current_graph_context() -> Option<crate::telemetry::GraphContext> {
    execution_hierarchy::current_ids().map(|ids| {
        if ids.depth == 0 {
            crate::telemetry::GraphContext::new(ids.execution_id)
        } else {
            crate::telemetry::GraphContext::with_parent(
                ids.execution_id,
                ids.parent_execution_id.unwrap_or_default(),
                ids.root_execution_id.unwrap_or_default(),
                ids.depth,
            )
        }
    })
}

// Re-exports for backwards compatibility
pub use introspection::{GraphIntrospection, UnifiedIntrospection};
pub use validation::{GraphValidationResult, GraphValidationWarning};

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use tracing::Instrument;

use crate::checkpoint::{Checkpoint, CheckpointPolicy, Checkpointer, MemoryCheckpointer, ThreadId};
use crate::core::retry::RetryPolicy;
use crate::edge::{ConditionalEdge, Edge, ParallelEdge, END};
use crate::error::Result;
use crate::event::{EventCallback, GraphEvent};
use crate::introspection::PerformanceMetrics;
use crate::metrics::ExecutionMetrics;
use crate::node::BoxedNode;
use crate::scheduler::WorkStealingScheduler;

/// Default timeout for individual node execution (5 minutes).
///
/// Prevents any single node from running indefinitely. This applies when
/// `with_node_timeout()` is not explicitly called. External API calls,
/// database queries, or any blocking operation in a node will be cancelled
/// after this duration.
///
/// # Rationale
/// - 5 minutes is generous for most operations (API calls, LLM queries)
/// - Prevents indefinite hangs from network issues or unresponsive services
/// - Can be overridden via `with_node_timeout()` for long-running operations
pub const DEFAULT_NODE_TIMEOUT: Duration = Duration::from_secs(300);

/// Default timeout for entire graph execution (1 hour).
///
/// Prevents graphs from running indefinitely even if individual nodes
/// complete within their timeouts. This applies when `with_timeout()`
/// is not explicitly called.
///
/// # Rationale
/// - 1 hour accommodates complex multi-step workflows
/// - Prevents resource exhaustion from runaway graph executions
/// - Can be overridden via `with_timeout()` for longer workflows
pub const DEFAULT_GRAPH_TIMEOUT: Duration = Duration::from_secs(3600);

/// Default maximum state size (100MB).
///
/// Prevents memory exhaustion from states that grow unboundedly during
/// execution. This applies when `with_max_state_size()` is not explicitly called.
///
/// # Rationale
/// - 100MB is generous for most workflow states
/// - Prevents OOM from runaway state accumulation
/// - Can be overridden via `with_max_state_size()` for larger states
/// - Can be disabled via `without_limits()` for unlimited state size
pub const DEFAULT_MAX_STATE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

/// Default maximum concurrent parallel tasks (64).
///
/// Limits how many parallel node executions can run concurrently within
/// a single graph execution. Prevents unbounded task spawning when a
/// graph has large fan-out (many parallel edges).
///
/// # Rationale
/// - 64 is generous for most parallel workflows
/// - Prevents tokio runtime exhaustion from unbounded task spawning
/// - Can be overridden via `with_max_parallel_tasks()` for higher concurrency
/// - Can be disabled via `without_limits()` for unlimited parallelism
pub const DEFAULT_MAX_PARALLEL_TASKS: usize = 64;

// ============================================================================
// Graph Execution Types
// ============================================================================

/// A compiled graph ready for execution
///
/// `CompiledGraph` is created by calling `compile()` on a [`crate::StateGraph`].
/// It validates the graph structure and provides execution methods.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::prelude::*;
///
/// // Build and compile a graph
/// let graph = StateGraph::new()
///     .add_node("process", my_node)
///     .add_edge("process", END)
///     .set_entry_point("process");
///
/// let app = graph.compile()?;
///
/// // Execute the graph
/// let result = app.invoke(initial_state).await?;
/// println!("Final state: {:?}", result.state());
///
/// // Or stream events during execution
/// let mut stream = app.stream(initial_state).await;
/// while let Some(event) = stream.next().await {
///     println!("Event: {:?}", event);
/// }
/// ```
///
/// # Configuration
///
/// Use builder methods to configure execution behavior:
///
/// ```rust,ignore
/// let app = graph.compile()?
///     .with_name("my_workflow")           // Name for tracing
///     .with_graph_timeout(Duration::from_secs(60))  // Total timeout
///     .with_checkpointer(my_checkpointer) // State persistence
///     .with_interrupt_before(&["human_review"])     // Human-in-the-loop
///     .with_retry_policy(RetryPolicy::exponential()); // Retry on failure
/// ```
///
/// # Errors
///
/// Execution can fail with:
/// - [`crate::Error::Timeout`] - Graph execution exceeded timeout
/// - [`crate::Error::NodeNotFound`] - Referenced node doesn't exist
/// - [`crate::Error::RecursionLimit`] - Graph exceeded step limit (default: 25)
/// - [`crate::Error::NodeExecution`] - Node function returned an error
///
/// # See Also
///
/// - [`crate::StateGraph`] - Builder for creating graphs
/// - [`ExecutionResult`] - Returned by `invoke()`
/// - [`crate::StreamEvent`] - Events from `stream()`
/// - [`crate::MergeableState`] - Required trait for parallel execution
///
/// **Note (v1.12.0):** The struct now accepts `S: GraphState`, but execution
/// methods (invoke, stream) require `S: MergeableState` to support parallel
/// edge merging. Sequential-only graphs can use `invoke_sequential()` without
/// implementing `MergeableState`.
pub struct CompiledGraph<S>
where
    S: crate::state::GraphState,
{
    /// Optional name for the graph (used in tracing spans)
    name: Option<String>,
    /// Base directory used for trace persistence (default: current working directory).
    ///
    /// When set, traces are persisted to `{trace_base_dir}/.dashflow/traces/` regardless of the
    /// process working directory. This avoids relying on global `std::env::current_dir()`.
    trace_base_dir: Option<std::path::PathBuf>,
    /// Nodes in the graph (Arc-wrapped to avoid cloning in stream_multi)
    nodes: Arc<HashMap<String, BoxedNode<S>>>,
    /// Simple edges (Arc-wrapped to avoid cloning in stream_multi)
    edges: Arc<Vec<Edge>>,
    /// Conditional edges
    conditional_edges: Arc<Vec<Arc<ConditionalEdge<S>>>>,
    /// Parallel edges (Arc-wrapped to avoid cloning in stream_multi)
    parallel_edges: Arc<Vec<ParallelEdge>>,
    /// Entry point node (Arc-wrapped to avoid cloning in stream_multi)
    entry_point: Arc<String>,
    /// Total graph execution timeout
    graph_timeout: Option<Duration>,
    /// Per-node execution timeout
    node_timeout: Option<Duration>,
    /// Event callbacks
    callbacks: Vec<Arc<dyn EventCallback<S>>>,
    /// Optional checkpointer for state persistence
    checkpointer: Option<Arc<dyn Checkpointer<S>>>,
    /// Thread ID for checkpoint isolation
    thread_id: Option<ThreadId>,
    /// Policy controlling when checkpoints are taken (default: Every)
    checkpoint_policy: CheckpointPolicy,
    /// Counter for nodes executed (used by EveryN policy)
    checkpoint_node_count: std::sync::atomic::AtomicUsize,
    /// State size at last checkpoint (used by OnStateChange policy)
    last_checkpoint_size: std::sync::atomic::AtomicUsize,
    /// Execution metrics
    metrics: Arc<Mutex<ExecutionMetrics>>,
    /// Optional work-stealing scheduler for distributed execution
    scheduler: Option<Arc<WorkStealingScheduler<S>>>,
    /// Nodes where execution pauses BEFORE running (human-in-the-loop)
    interrupt_before: Vec<String>,
    /// Nodes where execution pauses AFTER running (human-in-the-loop)
    interrupt_after: Vec<String>,
    /// Maximum number of execution steps before raising `RecursionLimit` error (default: 25)
    /// Prevents infinite loops in graphs with cycles
    recursion_limit: u32,
    /// Custom channel capacity for streaming (default: uses DEFAULT_STREAM_CHANNEL_CAPACITY)
    stream_channel_capacity: Option<usize>,
    /// Maximum state size in bytes (default: 100MB via DEFAULT_MAX_STATE_SIZE)
    /// Set to None for unlimited state size
    max_state_size: Option<u64>,
    /// Whether introspection methods are enabled (default: true)
    /// Opt-out via `without_introspection()`
    introspection_enabled: bool,
    /// Retry policy for transient node failures (default: 3 retries with exponential backoff)
    /// Set to None to disable retries via `without_retries()`
    retry_policy: Option<RetryPolicy>,
    /// Whether metrics collection is enabled (default: true)
    /// Opt-out via `without_metrics()`
    metrics_enabled: bool,
    /// Execution tracker for live introspection (default: ON per Invariant 6).
    /// Opt-out via `DASHFLOW_LIVE_INTROSPECTION=false` env var or `without_live_introspection()`.
    /// Override with custom tracker via `with_execution_tracker()`.
    execution_tracker: Option<Arc<crate::live_introspection::ExecutionTracker>>,
    /// Node metadata for visualization (descriptions, types, etc.)
    /// Populated from StateGraph::add_node_with_metadata()
    node_metadata: HashMap<String, crate::schema::NodeMetadata>,
    /// Runtime-mutable node configurations (prompts, parameters, etc.)
    /// Used for config versioning in telemetry
    node_configs: std::collections::HashMap<String, crate::introspection::NodeConfig>,
    /// Maximum concurrent parallel task executions (default: 64 via DEFAULT_MAX_PARALLEL_TASKS)
    /// Limits how many parallel nodes can execute simultaneously to prevent resource exhaustion.
    /// Set to None for unlimited parallelism via `without_limits()`.
    max_parallel_tasks: Option<usize>,
}

/// Core methods for `CompiledGraph` (construction and configuration)
///
/// These methods are available for all graph state types that implement `GraphState`.
impl<S> CompiledGraph<S>
where
    S: crate::state::GraphState,
{
    /// Create a new compiled graph (internal use only)
    pub(crate) fn new(
        nodes: HashMap<String, BoxedNode<S>>,
        edges: Vec<Edge>,
        conditional_edges: Vec<Arc<ConditionalEdge<S>>>,
        parallel_edges: Vec<ParallelEdge>,
        entry_point: String,
    ) -> Self {
        // PERF-002 FIX: Don't auto-wire WALEventCallback as it creates expensive WALWriter per graph.
        // Users can add it explicitly with .with_callback(WALEventCallback::from_env()?) if needed.
        // Trace persistence uses global EventStore singleton instead.
        let callbacks: Vec<Arc<dyn EventCallback<S>>> = Vec::new();

        Self {
            name: None,
            trace_base_dir: None,
            nodes: Arc::new(nodes),
            edges: Arc::new(edges),
            conditional_edges: Arc::new(conditional_edges),
            parallel_edges: Arc::new(parallel_edges),
            entry_point: Arc::new(entry_point),
            graph_timeout: None,
            node_timeout: None,
            callbacks,
            checkpointer: Some(Arc::new(MemoryCheckpointer::new())),
            thread_id: None,
            checkpoint_policy: CheckpointPolicy::default(),
            checkpoint_node_count: std::sync::atomic::AtomicUsize::new(0),
            last_checkpoint_size: std::sync::atomic::AtomicUsize::new(0),
            metrics: Arc::new(Mutex::new(ExecutionMetrics::new())),
            scheduler: None,
            interrupt_before: Vec::new(),
            interrupt_after: Vec::new(),
            recursion_limit: 25,
            stream_channel_capacity: None,
            max_state_size: Some(DEFAULT_MAX_STATE_SIZE),
            introspection_enabled: true,
            retry_policy: Some(RetryPolicy::default()),
            metrics_enabled: true,
            execution_tracker: if is_live_introspection_enabled() {
                Some(Arc::new(crate::live_introspection::ExecutionTracker::new()))
            } else {
                None
            },
            node_metadata: HashMap::new(),
            node_configs: std::collections::HashMap::new(),
            max_parallel_tasks: Some(DEFAULT_MAX_PARALLEL_TASKS),
        }
    }

    /// Set node metadata for visualization (descriptions, types, etc.)
    ///
    /// This is typically called during compile() to pass metadata from StateGraph.
    /// Use `StateGraph::add_node_with_metadata()` to add metadata.
    #[must_use]
    pub fn with_node_metadata(
        mut self,
        metadata: HashMap<String, crate::schema::NodeMetadata>,
    ) -> Self {
        self.node_metadata = metadata;
        self
    }

    /// Set the graph name (used in tracing spans)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_name("my-agent");
    /// ```
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Replace the default execution tracker with a custom one.
    ///
    /// Live introspection is ON by default (per DESIGN_INVARIANTS.md Invariant 6),
    /// which creates a default `ExecutionTracker`. Use this method to share a
    /// tracker across multiple graphs or to use a custom configuration.
    ///
    /// The execution tracker enables real-time monitoring of graph executions
    /// through the unified introspection API and MCP live endpoints.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::live_introspection::ExecutionTracker;
    /// use std::sync::Arc;
    ///
    /// // Share a tracker across multiple graphs
    /// let tracker = Arc::new(ExecutionTracker::new());
    /// let app1 = graph1.compile()?.with_execution_tracker(tracker.clone());
    /// let app2 = graph2.compile()?.with_execution_tracker(tracker.clone());
    ///
    /// // Later, query live execution state from either graph
    /// for exec in tracker.active_executions() {
    ///     println!("Execution: {} at node {}", exec.execution_id, exec.current_node);
    /// }
    /// ```
    pub fn with_execution_tracker(
        mut self,
        tracker: Arc<crate::live_introspection::ExecutionTracker>,
    ) -> Self {
        self.execution_tracker = Some(tracker);
        self
    }

    /// Get a reference to the execution tracker, if attached.
    #[must_use]
    pub fn execution_tracker(&self) -> Option<&Arc<crate::live_introspection::ExecutionTracker>> {
        self.execution_tracker.as_ref()
    }

    /// Disable live introspection for this graph.
    ///
    /// Live introspection is ON by default (per DESIGN_INVARIANTS.md Invariant 6).
    /// Use this method to explicitly disable it for performance-sensitive scenarios.
    ///
    /// Note: You can also disable live introspection globally by setting
    /// `DASHFLOW_LIVE_INTROSPECTION=false` in your environment.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .without_live_introspection();  // Disable live tracking
    /// ```
    #[must_use]
    pub fn without_live_introspection(mut self) -> Self {
        self.execution_tracker = None;
        self
    }

    /// Set the node configurations for config versioning telemetry.
    ///
    /// This enables tracking config version and hash in telemetry events,
    /// allowing correlation of executions with specific config versions
    /// for A/B testing and debugging.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::introspection::NodeConfig;
    /// use std::collections::HashMap;
    ///
    /// let mut configs = HashMap::new();
    /// configs.insert("llm".to_string(), NodeConfig::new("llm", "llm.chat")
    ///     .with_config(json!({"temperature": 0.7})));
    ///
    /// let app = graph.compile()?
    ///     .with_node_configs(configs);
    /// ```
    pub fn with_node_configs(
        mut self,
        configs: std::collections::HashMap<String, crate::introspection::NodeConfig>,
    ) -> Self {
        self.node_configs = configs;
        self
    }

    /// Get a reference to the node configurations.
    #[must_use]
    pub fn node_configs(
        &self,
    ) -> &std::collections::HashMap<String, crate::introspection::NodeConfig> {
        &self.node_configs
    }

    /// Get a node configuration by name.
    #[must_use]
    pub fn get_node_config(&self, name: &str) -> Option<&crate::introspection::NodeConfig> {
        self.node_configs.get(name)
    }

    /// Add an event callback to the graph
    ///
    /// Callbacks receive events during graph execution for monitoring,
    /// debugging, or custom logic.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::PrintCallback;
    ///
    /// let app = graph.compile()?
    ///     .with_callback(PrintCallback);
    /// ```
    #[must_use]
    pub fn with_callback<C>(mut self, callback: C) -> Self
    where
        C: EventCallback<S> + 'static,
    {
        self.callbacks.push(Arc::new(callback));
        self
    }

    /// Add a simple closure-based tracer for node execution
    ///
    /// This is a convenience method that wraps the closure in an `FnTracer`.
    /// For full control over all events, use `with_callback()` instead.
    ///
    /// The tracer receives `TracerEvent` for node start, end, and error events.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::TracerEvent;
    ///
    /// let app = graph.compile()?
    ///     .with_tracer(|event| {
    ///         match event {
    ///             TracerEvent::NodeStart { node, .. } => {
    ///                 println!("Starting: {}", node);
    ///             }
    ///             TracerEvent::NodeEnd { node, duration, .. } => {
    ///                 println!("Completed: {} in {:?}", node, duration);
    ///             }
    ///             TracerEvent::NodeError { node, error, .. } => {
    ///                 eprintln!("Failed: {} - {}", node, error);
    ///             }
    ///         }
    ///     });
    /// ```
    #[must_use]
    pub fn with_tracer<F>(self, tracer: F) -> Self
    where
        F: Fn(crate::event::TracerEvent<'_, S>) + Send + Sync + 'static,
    {
        self.with_callback(crate::event::FnTracer::new(tracer))
    }

    /// Set the base directory used for trace persistence.
    ///
    /// By default, traces are written relative to the process working directory (`.`).
    /// Setting this allows callers (and tests) to route traces to a specific directory without
    /// mutating global process state via `std::env::set_current_dir`.
    #[must_use]
    pub fn with_trace_base_dir(mut self, base_dir: impl Into<std::path::PathBuf>) -> Self {
        self.trace_base_dir = Some(base_dir.into());
        self
    }

    /// Replace the default checkpointer with a custom one
    ///
    /// By default, graphs use an in-memory checkpointer (`MemoryCheckpointer`).
    /// Use this method to replace it with a persistent storage backend like
    /// `FileCheckpointer`, `PostgresCheckpointer`, etc.
    ///
    /// Checkpointers enable resuming execution from failures,
    /// state snapshots, and audit trails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::FileCheckpointer;
    ///
    /// // Replace default in-memory checkpointer with file-based storage
    /// let app = graph.compile()?
    ///     .with_checkpointer(FileCheckpointer::new("./checkpoints")?)
    ///     .with_thread_id("thread-1");
    /// ```
    ///
    /// To disable checkpointing entirely, use `without_checkpointing()` instead.
    #[must_use]
    pub fn with_checkpointer<C>(mut self, checkpointer: C) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        self.checkpointer = Some(Arc::new(checkpointer));
        self
    }

    /// Set the thread ID for checkpoint isolation
    ///
    /// When using checkpointing, thread IDs separate different execution contexts.
    /// Multiple graph executions with different thread IDs maintain separate checkpoint histories.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_checkpointer(checkpointer)
    ///     .with_thread_id("user-session-123");
    /// ```
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<ThreadId>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Disable checkpointing (opt-out)
    ///
    /// By default, graphs use an in-memory checkpointer for state persistence.
    /// Use this method to disable checkpointing entirely when you don't need
    /// state persistence, resume capabilities, or audit trails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Default: Uses MemoryCheckpointer
    /// let app = graph.compile()?;
    /// // Checkpointing is enabled
    ///
    /// // Explicit opt-out:
    /// let app = graph.compile()?
    ///     .without_checkpointing();
    /// // Checkpointing is disabled
    /// ```
    pub fn without_checkpointing(mut self) -> Self {
        self.checkpointer = None;
        self
    }

    /// Check if checkpointing is enabled
    ///
    /// Returns `true` if a checkpointer is configured (default behavior),
    /// or `false` if checkpointing was disabled via `without_checkpointing()`.
    pub fn checkpointing_enabled(&self) -> bool {
        self.checkpointer.is_some()
    }

    /// Set the checkpoint policy (controls when checkpoints are saved).
    ///
    /// By default, checkpoints are saved after every node (`CheckpointPolicy::Every`).
    /// For graphs with many nodes or expensive state serialization, this can
    /// significantly impact performance. Use this method to reduce checkpoint
    /// frequency.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::{StateGraph, CheckpointPolicy, MemoryCheckpointer};
    ///
    /// let app = graph.compile()?
    ///     .with_checkpointer(MemoryCheckpointer::new())
    ///     .with_thread_id("session-1")
    ///     // Only checkpoint every 5 nodes
    ///     .with_checkpoint_policy(CheckpointPolicy::EveryN(5));
    ///
    /// // Or checkpoint only at specific nodes
    /// let app = graph.compile()?
    ///     .with_checkpoint_policy(CheckpointPolicy::on_markers(["save_point", "critical_node"]));
    /// ```
    #[must_use]
    pub fn with_checkpoint_policy(mut self, policy: CheckpointPolicy) -> Self {
        self.checkpoint_policy = policy;
        self
    }

    /// Convenience method to checkpoint every N nodes.
    ///
    /// Equivalent to `with_checkpoint_policy(CheckpointPolicy::EveryN(n))`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Checkpoint every 5 nodes instead of every node
    /// let app = graph.compile()?
    ///     .with_checkpoint_every(5);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `n` is 0.
    #[must_use]
    pub fn with_checkpoint_every(self, n: usize) -> Self {
        self.with_checkpoint_policy(CheckpointPolicy::every_n(n))
    }

    /// Get the current checkpoint policy.
    pub fn checkpoint_policy(&self) -> &CheckpointPolicy {
        &self.checkpoint_policy
    }

    /// Add a checkpoint marker node.
    ///
    /// When using `CheckpointPolicy::OnMarkers`, this method adds a node
    /// to the marker set. Checkpoints will only be saved at nodes in the set.
    ///
    /// If the current policy is not `OnMarkers`, this method will convert
    /// the policy to `OnMarkers` with the given node as the first marker.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Start with OnMarkers policy and add markers incrementally
    /// let app = graph.compile()?
    ///     .with_checkpoint_policy(CheckpointPolicy::OnMarkers(Default::default()))
    ///     .with_checkpoint_marker("save_point")
    ///     .with_checkpoint_marker("critical_node");
    ///
    /// // Or convert an existing policy to markers
    /// let app = graph.compile()?
    ///     .with_checkpoint_marker("important_node"); // Converts to OnMarkers
    /// ```
    #[must_use]
    pub fn with_checkpoint_marker(mut self, node: impl Into<String>) -> Self {
        let node = node.into();
        match &mut self.checkpoint_policy {
            CheckpointPolicy::OnMarkers(markers) => {
                markers.insert(node);
            }
            _ => {
                // Convert to OnMarkers policy with this node as the first marker
                let mut markers = std::collections::HashSet::new();
                markers.insert(node);
                self.checkpoint_policy = CheckpointPolicy::OnMarkers(markers);
            }
        }
        self
    }

    /// Add a work-stealing scheduler for distributed parallel execution
    ///
    /// When a scheduler is configured, parallel edges will distribute execution
    /// across remote workers using work-stealing load balancing. If no scheduler
    /// is set, parallel edges execute locally using `tokio::spawn`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::{WorkStealingScheduler, SelectionStrategy};
    ///
    /// let scheduler = WorkStealingScheduler::new()
    ///     .with_workers(vec!["worker1:50051", "worker2:50051"])
    ///     .with_threshold(10)
    ///     .with_strategy(SelectionStrategy::LeastLoaded);
    ///
    /// let app = graph.compile()?
    ///     .with_scheduler(scheduler);
    /// ```
    #[must_use]
    pub fn with_scheduler(mut self, scheduler: WorkStealingScheduler<S>) -> Self {
        self.scheduler = Some(Arc::new(scheduler));
        self
    }

    /// Set nodes where execution pauses BEFORE node execution (human-in-the-loop)
    ///
    /// When execution reaches one of these nodes, the graph will:
    /// 1. Save a checkpoint with current state
    /// 2. Return an `ExecutionResult` with `interrupted_at` set
    /// 3. NOT execute the node yet
    ///
    /// Use `resume()` to continue execution from the checkpoint.
    ///
    /// **Requires**: Both `with_checkpointer()` and `with_thread_id()` must be configured.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_checkpointer(MemoryCheckpointer::new())
    ///     .with_thread_id("conversation-1")
    ///     .with_interrupt_before(vec!["human_review", "approval"]);
    ///
    /// // First invocation - pauses at "human_review"
    /// let result = app.invoke(initial_state).await?;
    /// assert!(result.interrupted_at.is_some());
    ///
    /// // ... human reviews via external system ...
    ///
    /// // Resume from checkpoint
    /// let final_result = app.resume().await?;
    /// ```
    #[must_use]
    pub fn with_interrupt_before(mut self, nodes: Vec<impl Into<String>>) -> Self {
        self.interrupt_before = nodes.into_iter().map(std::convert::Into::into).collect();
        self
    }

    /// Set nodes where execution pauses AFTER node execution (human-in-the-loop)
    ///
    /// When execution completes one of these nodes, the graph will:
    /// 1. Execute the node normally
    /// 2. Save a checkpoint with updated state
    /// 3. Return an `ExecutionResult` with `interrupted_at` set
    ///
    /// Use `resume()` to continue execution from the checkpoint.
    ///
    /// **Requires**: Both `with_checkpointer()` and `with_thread_id()` must be configured.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_checkpointer(PostgresCheckpointer::new(pool))
    ///     .with_thread_id("workflow-456")
    ///     .with_interrupt_after(vec!["sensitive_operation"]);
    ///
    /// // Executes "sensitive_operation" then pauses
    /// let result = app.invoke(initial_state).await?;
    /// // Can inspect state, validate results, etc.
    ///
    /// // Resume to continue
    /// let final_result = app.resume().await?;
    /// ```
    #[must_use]
    pub fn with_interrupt_after(mut self, nodes: Vec<impl Into<String>>) -> Self {
        self.interrupt_after = nodes.into_iter().map(std::convert::Into::into).collect();
        self
    }

    /// Set the maximum number of execution steps (recursion limit)
    ///
    /// The recursion limit prevents infinite loops in graphs with cycles.
    /// Execution will raise a `RecursionLimit` error if more than this many
    /// nodes are executed in a single invocation.
    ///
    /// Default: 25 (matches upstream DashFlow default)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_recursion_limit(100);  // Allow up to 100 node executions
    ///
    /// let result = app.invoke(initial_state).await?;
    /// ```
    #[must_use]
    pub fn with_recursion_limit(mut self, limit: u32) -> Self {
        self.recursion_limit = limit;
        self
    }

    /// Set the channel capacity for custom stream events
    ///
    /// When using `StreamMode::Custom`, nodes emit custom data via a bounded channel.
    /// By default, the channel holds up to 10,000 messages. If the channel fills up
    /// (consumer is slow), additional messages are dropped with a warning.
    ///
    /// Use this method to:
    /// - Increase capacity for high-volume custom events
    /// - Decrease capacity to limit memory usage
    /// - Match capacity to your application's throughput
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Large capacity for high-volume streaming
    /// let app = graph.compile()?
    ///     .with_stream_channel_capacity(100_000);
    ///
    /// // Small capacity to limit memory
    /// let app = graph.compile()?
    ///     .with_stream_channel_capacity(100);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics at runtime if `capacity` is 0.
    #[must_use]
    pub fn with_stream_channel_capacity(mut self, capacity: usize) -> Self {
        self.stream_channel_capacity = Some(capacity);
        self
    }

    /// Set the maximum state size in bytes
    ///
    /// If any node execution produces a state larger than this limit,
    /// execution will fail with a `StateSizeExceeded` error.
    ///
    /// Default: 100MB (see [`DEFAULT_MAX_STATE_SIZE`]).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_max_state_size(200 * 1024 * 1024);  // Allow up to 200MB state
    /// ```
    #[must_use]
    pub fn with_max_state_size(mut self, max_bytes: u64) -> Self {
        self.max_state_size = Some(max_bytes);
        self
    }

    /// Set maximum concurrent parallel task executions
    ///
    /// By default, graphs limit parallel node execution to 64 concurrent tasks
    /// (via `DEFAULT_MAX_PARALLEL_TASKS`). This prevents resource exhaustion
    /// when a graph has large fan-out (many parallel edges).
    ///
    /// Use this method to increase or decrease the limit based on your
    /// application's resource constraints and parallelism requirements.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Allow up to 128 concurrent parallel tasks
    /// let app = graph.compile()?
    ///     .with_max_parallel_tasks(128);
    ///
    /// // Restrict to sequential execution (1 task at a time)
    /// let app = graph.compile()?
    ///     .with_max_parallel_tasks(1);
    /// ```
    #[must_use]
    pub fn with_max_parallel_tasks(mut self, max_tasks: usize) -> Self {
        self.max_parallel_tasks = Some(max_tasks.max(1)); // At least 1 task allowed
        self
    }

    /// Disable all default resource limits
    ///
    /// By default, `CompiledGraph` enforces:
    /// - Recursion limit: 25 steps
    /// - Node timeout: 5 minutes
    /// - Graph timeout: 1 hour
    /// - Max state size: 100MB
    /// - Max parallel tasks: 64
    ///
    /// Call this method to disable all limits for workflows that intentionally
    /// exceed these defaults (e.g., long-running data pipelines, infinite event loops).
    ///
    /// **Warning**: Disabling limits may lead to:
    /// - Infinite loops (no recursion limit)
    /// - Hung processes (no timeouts)
    /// - Memory exhaustion (no state size limit)
    /// - Task exhaustion (no parallel task limit)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable all limits for a long-running pipeline
    /// let app = graph.compile()?
    ///     .without_limits();
    ///
    /// // Or selectively re-enable specific limits after disabling all:
    /// let app = graph.compile()?
    ///     .without_limits()
    ///     .with_recursion_limit(1000);  // Re-enable just recursion limit
    /// ```
    #[must_use]
    pub fn without_limits(mut self) -> Self {
        self.recursion_limit = u32::MAX;
        self.node_timeout = Some(Duration::MAX);
        self.graph_timeout = Some(Duration::MAX);
        self.max_state_size = None;
        self.max_parallel_tasks = None;
        self
    }

    /// Replace the default retry policy with a custom one
    ///
    /// By default, graphs use a retry policy with 3 retries and exponential backoff
    /// (1s, 2s, 4s delays) for transient errors (network, timeout, rate limit).
    ///
    /// Use this method to customize retry behavior for your application's needs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// // Custom: 5 retries with fixed 500ms delay
    /// let app = graph.compile()?
    ///     .with_retry_policy(RetryPolicy::fixed(5, 500));
    ///
    /// // Custom: No retries
    /// let app = graph.compile()?
    ///     .with_retry_policy(RetryPolicy::no_retry());
    ///
    /// // Custom: Exponential with jitter to prevent thundering herd
    /// let app = graph.compile()?
    ///     .with_retry_policy(RetryPolicy::default_jitter(3));
    /// ```
    ///
    /// To disable retries entirely, use `without_retries()` instead.
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// Disable automatic retries for transient failures (opt-out)
    ///
    /// By default, graphs automatically retry node executions that fail with
    /// transient errors (network issues, timeouts, rate limits) using exponential
    /// backoff. Use this method to disable retries entirely.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Default: Uses RetryPolicy::default() (3 retries, exponential backoff)
    /// let app = graph.compile()?;
    /// assert!(app.retries_enabled());
    ///
    /// // Explicit opt-out:
    /// let app = graph.compile()?
    ///     .without_retries();
    /// assert!(!app.retries_enabled());
    /// ```
    #[must_use]
    pub fn without_retries(mut self) -> Self {
        self.retry_policy = None;
        self
    }

    /// Check if automatic retries are enabled
    ///
    /// Returns `true` if a retry policy is configured (default behavior),
    /// or `false` if retries were disabled via `without_retries()`.
    #[must_use]
    pub fn retries_enabled(&self) -> bool {
        self.retry_policy.is_some()
    }

    /// Disable metrics collection (opt-out)
    ///
    /// By default, graphs automatically collect execution metrics including node
    /// durations, checkpoint operations, and state sizes. Use this method to disable
    /// metrics collection entirely when you need minimal overhead.
    ///
    /// **Note**: Even with metrics disabled, basic timing information is still
    /// collected for tracing spans. This opt-out disables the structured
    /// `ExecutionMetrics` collection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Default: Metrics collection enabled
    /// let app = graph.compile()?;
    /// assert!(app.metrics_enabled());
    /// let m = app.metrics();  // Returns collected metrics
    ///
    /// // Explicit opt-out:
    /// let app = graph.compile()?
    ///     .without_metrics();
    /// assert!(!app.metrics_enabled());
    /// let m = app.metrics();  // Returns empty metrics
    /// ```
    #[must_use]
    pub fn without_metrics(mut self) -> Self {
        self.metrics_enabled = false;
        self
    }

    /// Check if metrics collection is enabled
    ///
    /// Returns `true` if metrics are being collected (default behavior),
    /// or `false` if metrics were disabled via `without_metrics()`.
    #[must_use]
    pub fn metrics_enabled(&self) -> bool {
        self.metrics_enabled
    }

    /// Enable metrics collection (opt-in after disabling)
    ///
    /// Re-enables metrics collection if it was previously disabled via `without_metrics()`.
    /// Metrics are enabled by default, so this method is only needed to re-enable them
    /// after explicitly disabling.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable then re-enable metrics
    /// let app = graph.compile()?
    ///     .without_metrics()  // Disable for testing
    ///     .with_metrics();    // Re-enable for production
    /// assert!(app.metrics_enabled());
    /// ```
    #[must_use]
    pub fn with_metrics(mut self) -> Self {
        self.metrics_enabled = true;
        self
    }

    /// Enable checkpointing with default in-memory checkpointer
    ///
    /// Re-enables checkpointing if it was previously disabled via `without_checkpointing()`.
    /// This uses the default `MemoryCheckpointer` for state persistence.
    ///
    /// For custom checkpointers (file-based, database, etc.), use `with_checkpointer()` instead.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable then re-enable checkpointing
    /// let app = graph.compile()?
    ///     .without_checkpointing()  // Disable for fast iteration
    ///     .with_checkpointing();    // Re-enable with default memory storage
    /// assert!(app.checkpointing_enabled());
    /// ```
    #[must_use]
    pub fn with_checkpointing(mut self) -> Self {
        self.checkpointer = Some(Arc::new(MemoryCheckpointer::new()));
        self
    }

    /// Set sensible default timeouts for production use
    ///
    /// Configures both graph and node timeouts with reasonable defaults:
    /// - Graph timeout: 5 minutes (300 seconds)
    /// - Node timeout: 1 minute (60 seconds)
    ///
    /// These defaults balance reliability with flexibility for most workflows.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_default_timeouts();
    /// // Graph timeout: 300s, Node timeout: 60s
    /// ```
    #[must_use]
    pub fn with_default_timeouts(self) -> Self {
        self.with_timeout(Duration::from_secs(300))
            .with_node_timeout(Duration::from_secs(60))
    }

    /// Disable all timeouts (useful for long-running AI tasks)
    ///
    /// Removes both graph and node timeouts, allowing execution to run indefinitely.
    /// Use with caution - this can cause resource exhaustion if nodes hang.
    ///
    /// This is useful for:
    /// - Long-running data processing tasks
    /// - AI workflows with unpredictable latency
    /// - Debugging without timeout interference
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .without_timeouts();
    /// // No timeouts - execution can run indefinitely
    /// ```
    #[must_use]
    pub fn without_timeouts(mut self) -> Self {
        self.graph_timeout = None;
        self.node_timeout = None;
        self
    }

    /// Configure graph for lightweight testing mode
    ///
    /// Bundles multiple opt-outs for fast, low-overhead test execution:
    /// - Disables metrics collection
    /// - Disables checkpointing
    /// - Disables automatic retries
    /// - Sets short timeout (30 seconds)
    ///
    /// This is ideal for unit tests and rapid iteration where you want
    /// minimal overhead and fast feedback.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .for_testing();
    ///
    /// // Equivalent to:
    /// let app = graph.compile()?
    ///     .without_metrics()
    ///     .without_checkpointing()
    ///     .without_retries()
    ///     .with_timeout(Duration::from_secs(30));
    /// ```
    #[must_use]
    pub fn for_testing(self) -> Self {
        self.without_metrics()
            .without_checkpointing()
            .without_retries()
            .with_timeout(Duration::from_secs(30))
    }

    /// Configure graph for full production observability
    ///
    /// Bundles multiple opt-ins for comprehensive production monitoring:
    /// - Enables metrics collection
    /// - Enables checkpointing (default in-memory)
    /// - Uses default production timeouts (5 min graph, 1 min node)
    ///
    /// Note: Retries remain at their default (enabled with exponential backoff).
    /// Use `with_retry_policy()` if you need custom retry behavior.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .with_observability();
    ///
    /// // Equivalent to:
    /// let app = graph.compile()?
    ///     .with_metrics()
    ///     .with_checkpointing()
    ///     .with_default_timeouts();
    /// ```
    #[must_use]
    pub fn with_observability(self) -> Self {
        self.with_metrics()
            .with_checkpointing()
            .with_default_timeouts()
    }

    /// Get performance metrics derived from execution data
    ///
    /// Returns a `PerformanceMetrics` snapshot derived from the most recent
    /// execution's `ExecutionMetrics`. This provides a higher-level view of
    /// performance including latency statistics and throughput estimates.
    ///
    /// This method is always available (enabled by default). Use `without_metrics()`
    /// to disable metrics collection, in which case this returns empty metrics.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = app.invoke(state).await?;
    /// let perf = app.performance();
    ///
    /// if perf.is_latency_high(1000.0) {
    ///     println!("Warning: High latency detected");
    /// }
    ///
    /// println!("Performance summary:\n{}", perf.summarize());
    /// ```
    #[must_use]
    pub fn performance(&self) -> PerformanceMetrics {
        let metrics = self.metrics();

        // Convert ExecutionMetrics to PerformanceMetrics
        let current_latency_ms = metrics.total_duration.as_secs_f64() * 1000.0;

        // Calculate average node latency
        let avg_node_latency_ms = if metrics.node_durations.is_empty() {
            0.0
        } else {
            let total: f64 = metrics
                .node_durations
                .values()
                .map(|d| d.as_secs_f64() * 1000.0)
                .sum();
            total / metrics.node_durations.len() as f64
        };

        // Estimate throughput based on nodes executed per second
        let total_nodes: usize = metrics.node_execution_counts.values().sum();
        let nodes_per_second = if metrics.total_duration.is_zero() {
            0.0
        } else {
            total_nodes as f64 / metrics.total_duration.as_secs_f64()
        };

        PerformanceMetrics::new()
            .with_current_latency_ms(current_latency_ms)
            .with_average_latency_ms(avg_node_latency_ms)
            .with_tokens_per_second(nodes_per_second) // Using nodes/sec as proxy for throughput
    }

    /// Emit an event to all registered callbacks
    // ALLOW: Taking GraphEvent by value is intentional - callbacks receive owned events for flexibility
    #[allow(clippy::needless_pass_by_value)]
    fn emit_event(&self, event: GraphEvent<S>) {
        for callback in &self.callbacks {
            callback.on_event(&event);
        }
    }

    /// Check if a checkpoint should be saved based on the current policy and context.
    ///
    /// Returns true if the policy indicates a checkpoint should be saved.
    /// Also increments the internal node counter.
    fn should_checkpoint(&self, state: &S, node: &str) -> bool {
        use std::sync::atomic::Ordering;

        // Increment node counter
        let node_count = self.checkpoint_node_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Get state size for OnStateChange policy
        let state_size = bincode::serialized_size(state).unwrap_or(0) as usize;
        let last_size = self.last_checkpoint_size.load(Ordering::Relaxed);

        self.checkpoint_policy
            .should_checkpoint(node, node_count, state_size, last_size)
    }

    /// Save a checkpoint if checkpointer is configured and policy allows.
    ///
    /// This method respects the configured `CheckpointPolicy`:
    /// - `Every`: Always saves (default behavior)
    /// - `EveryN(n)`: Saves every N nodes
    /// - `OnMarkers(set)`: Saves only at nodes in the marker set
    /// - `OnStateChange { min_delta }`: Saves when state size changes significantly
    /// - `Never`: Never saves
    async fn save_checkpoint(
        &self,
        state: &S,
        node: &str,
        parent_id: Option<String>,
    ) -> Result<Option<String>> {
        // Check policy first
        if !self.should_checkpoint(state, node) {
            return Ok(parent_id); // Return parent_id unchanged to preserve chain
        }

        self.save_checkpoint_unconditional(state, node, parent_id)
            .await
    }

    /// Force save a checkpoint, bypassing the policy check.
    ///
    /// Used for interrupt points where checkpoints are always required.
    async fn save_checkpoint_unconditional(
        &self,
        state: &S,
        node: &str,
        parent_id: Option<String>,
    ) -> Result<Option<String>> {
        if let (Some(checkpointer), Some(thread_id)) = (&self.checkpointer, &self.thread_id) {
            // Calculate state size for tracing (skip when metrics disabled)
            let state_size = if self.metrics_enabled {
                bincode::serialized_size(state).unwrap_or(0)
            } else {
                0 // Skip expensive serialization when metrics disabled
            };

            // Update last checkpoint size for OnStateChange policy
            self.last_checkpoint_size
                .store(state_size as usize, std::sync::atomic::Ordering::Relaxed);

            let span = tracing::info_span!(
                "checkpoint.save",
                thread_id = %thread_id,
                node = node,
                state_size_bytes = state_size,
                duration_ms = tracing::field::Empty
            );

            async move {
                let start = std::time::Instant::now();

                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    state.clone(),
                    node.to_string(),
                    parent_id,
                );
                let checkpoint_id = checkpoint.id.clone();
                checkpointer.save(checkpoint).await?;

                // Record checkpoint save in metrics (if enabled)
                // Note: tokio::sync::Mutex is async-safe and doesn't poison
                if self.metrics_enabled {
                    let mut metrics = self.metrics.lock().await;
                    metrics.record_checkpoint_save();
                }

                // Record duration in span
                let duration_ms = start.elapsed().as_millis() as u64;
                tracing::Span::current().record("duration_ms", duration_ms);

                Ok(Some(checkpoint_id))
            }
            .instrument(span)
            .await
        } else {
            Ok(None)
        }
    }

    /// Set a timeout for the entire graph execution
    ///
    /// If the graph execution takes longer than this duration,
    /// it will be cancelled and return a timeout error.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let app = graph.compile()?
    ///     .with_timeout(Duration::from_secs(30));
    /// ```
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.graph_timeout = Some(timeout);
        self
    }

    /// Set a timeout for individual node execution
    ///
    /// If any single node takes longer than this duration to execute,
    /// it will be cancelled and return a timeout error.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let app = graph.compile()?
    ///     .with_node_timeout(Duration::from_secs(5));
    /// ```
    #[must_use]
    pub fn with_node_timeout(mut self, timeout: Duration) -> Self {
        self.node_timeout = Some(timeout);
        self
    }

    /// Get execution metrics
    ///
    /// Returns metrics collected during graph execution including node durations,
    /// total execution time, checkpoint operations, and state sizes.
    ///
    /// Metrics are reset at the start of each invocation, so this method returns
    /// metrics from the most recent execution.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = app.invoke(state).await?;
    /// let metrics = app.metrics();
    ///
    /// println!("Total time: {:?}", metrics.total_duration);
    /// if let Some((node, duration)) = metrics.slowest_node() {
    ///     println!("Slowest node: {} ({:?})", node, duration);
    /// }
    /// ```
    #[must_use]
    pub fn metrics(&self) -> ExecutionMetrics {
        // Use try_lock() to avoid blocking. If lock is held (concurrent execution),
        // return empty metrics. After invoke() completes, lock should always be available.
        self.metrics
            .try_lock()
            .ok()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Validate graph structure for common issues
    ///
    /// This method checks for potential problems that wouldn't be caught by `compile()`:
    ///
    /// - **Unreachable nodes**: Nodes that exist but cannot be reached from the entry point
    /// - **No path to END**: Graphs where execution may never terminate
    /// - **Dead-end nodes**: Nodes with no outgoing edges (that aren't END)
    ///
    /// Unlike `compile()`, which returns errors for invalid graphs, `validate()`
    /// returns warnings for suspicious-but-valid graphs that may indicate bugs.
    ///
    /// # Returns
    ///
    /// A `GraphValidationResult` containing any warnings found. Use `.is_valid()` to
    /// check if the graph passed all validations, or iterate over `.warnings()` to
    /// inspect individual issues.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?;
    ///
    /// let validation = app.validate();
    /// if !validation.is_valid() {
    ///     for warning in validation.warnings() {
    ///         eprintln!("Warning: {}", warning);
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn validate(&self) -> GraphValidationResult {
        let mut result = GraphValidationResult::new();

        // 1. Find all reachable nodes from entry point using BFS
        let reachable = self.find_reachable_nodes();

        // 2. Check for unreachable nodes
        for node_name in self.nodes.keys() {
            if !reachable.contains(node_name) {
                result.add_warning(GraphValidationWarning::UnreachableNode {
                    node: node_name.clone(),
                });
            }
        }

        // 3. Check if there's a path to END
        let has_end_path = self.has_path_to_end(&reachable);
        if !has_end_path {
            result.add_warning(GraphValidationWarning::NoPathToEnd);
        }

        // 4. Check for dead-end nodes (nodes with no outgoing edges, but not END)
        for node_name in &reachable {
            if !self.has_outgoing_edge(node_name) {
                result.add_warning(GraphValidationWarning::DeadEndNode {
                    node: node_name.clone(),
                });
            }
        }

        result
    }

    /// Find all nodes reachable from the entry point
    fn find_reachable_nodes(&self) -> HashSet<String> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((*self.entry_point).clone());

        while let Some(node) = queue.pop_front() {
            if reachable.contains(&node) {
                continue;
            }
            reachable.insert(node.clone());

            // Add all successor nodes to the queue
            for successor in self.get_successors(&node) {
                if successor != END && !reachable.contains(&successor) {
                    queue.push_back(successor);
                }
            }
        }

        reachable
    }

    /// Get all possible successor nodes from a given node
    fn get_successors(&self, node: &str) -> Vec<String> {
        let mut successors = Vec::new();

        // Check simple edges
        for edge in self.edges.iter() {
            if edge.from.as_str() == node {
                successors.push(edge.to.as_str().to_string());
            }
        }

        // Check conditional edges (all possible routes)
        for edge in self.conditional_edges.iter() {
            if edge.from.as_str() == node {
                // Routes values are Arc<String>, so we clone the Arc and convert to String
                successors.extend(edge.routes.values().map(|v| (**v).clone()));
            }
        }

        // Check parallel edges
        for edge in self.parallel_edges.iter() {
            if edge.from.as_str() == node {
                successors.extend(edge.to.iter().cloned());
            }
        }

        successors
    }

    /// Check if the graph has any path to END from reachable nodes
    fn has_path_to_end(&self, reachable: &HashSet<String>) -> bool {
        // Check if any reachable node has an edge to END
        for node in reachable {
            for successor in self.get_successors(node) {
                if successor == END {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a node has any outgoing edges
    fn has_outgoing_edge(&self, node: &str) -> bool {
        // Check simple edges
        for edge in self.edges.iter() {
            if edge.from.as_str() == node {
                return true;
            }
        }

        // Check conditional edges
        for edge in self.conditional_edges.iter() {
            if edge.from.as_str() == node {
                return true;
            }
        }

        // Check parallel edges
        for edge in self.parallel_edges.iter() {
            if edge.from.as_str() == node {
                return true;
            }
        }

        false
    }

    /// Generate a graph manifest for AI introspection
    ///
    /// Creates a complete manifest of the compiled graph structure that an AI agent
    /// can use to understand its own capabilities and structure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?;
    /// let manifest = app.manifest();
    ///
    /// // AI can query: "What nodes do I have?"
    /// for name in manifest.node_names() {
    ///     println!("I have node: {}", name);
    /// }
    ///
    /// // AI can query: "What are my decision points?"
    /// let decisions = manifest.decision_points();
    ///
    /// // Export as JSON for AI consumption
    /// let json = manifest.to_json().unwrap();
    /// ```
    // SAFETY: expect() is valid because entry_point is always set in CompiledGraph construction
    #[allow(clippy::expect_used)]
    #[must_use]
    pub fn manifest(&self) -> crate::introspection::GraphManifest {
        use crate::introspection::{
            EdgeManifest, GraphManifest, GraphMetadata, NodeManifest, NodeType,
        };

        let mut builder = GraphManifest::builder().entry_point((*self.entry_point).clone());

        // Add graph name if set
        if let Some(name) = &self.name {
            builder = builder.graph_name(name.clone());
        }

        // Add nodes with full metadata from node_metadata
        for name in self.nodes.keys() {
            // Map schema::NodeType to introspection::NodeType
            let introspection_node_type = if let Some(metadata) = self.node_metadata.get(name) {
                match metadata.node_type {
                    crate::schema::NodeType::Llm => NodeType::Agent,
                    crate::schema::NodeType::Tool => NodeType::ToolExecutor,
                    crate::schema::NodeType::Router => NodeType::Custom("router".to_string()),
                    crate::schema::NodeType::Aggregator => {
                        NodeType::Custom("aggregator".to_string())
                    }
                    crate::schema::NodeType::Validator => NodeType::Custom("validator".to_string()),
                    crate::schema::NodeType::HumanInLoop => NodeType::Approval,
                    crate::schema::NodeType::Checkpoint => {
                        NodeType::Custom("checkpoint".to_string())
                    }
                    crate::schema::NodeType::Transform | crate::schema::NodeType::Custom(_) => {
                        NodeType::Function
                    }
                }
            } else {
                NodeType::Function
            };

            let mut node_manifest = NodeManifest::new(name.clone(), introspection_node_type);

            // Include all metadata from node_metadata if available
            if let Some(metadata) = self.node_metadata.get(name) {
                // Add description
                if let Some(desc) = &metadata.description {
                    node_manifest = node_manifest.with_description(desc.clone());
                }
                // Add input_fields as metadata
                if !metadata.input_fields.is_empty() {
                    node_manifest = node_manifest
                        .with_metadata("input_fields", serde_json::json!(metadata.input_fields));
                }
                // Add output_fields as metadata
                if !metadata.output_fields.is_empty() {
                    node_manifest = node_manifest
                        .with_metadata("output_fields", serde_json::json!(metadata.output_fields));
                }
                // Add node_type string for UI display
                node_manifest = node_manifest.with_metadata(
                    "node_type",
                    serde_json::json!(format!("{:?}", metadata.node_type).to_lowercase()),
                );
            }
            builder = builder.add_node(name.clone(), node_manifest);
        }

        // Add simple edges
        for edge in self.edges.iter() {
            let edge_manifest = EdgeManifest::simple(edge.from.as_str(), edge.to.as_str());
            builder = builder.add_edge(edge.from.as_str(), edge_manifest);
        }

        // Add conditional edges
        for cond_edge in self.conditional_edges.iter() {
            for (label, target) in &cond_edge.routes {
                let edge_manifest = EdgeManifest::conditional(
                    cond_edge.from.as_str(),
                    target.as_str(),
                    label.clone(),
                );
                builder = builder.add_edge(cond_edge.from.as_str(), edge_manifest);
            }
        }

        // Add parallel edges
        for par_edge in self.parallel_edges.iter() {
            for target in par_edge.to.iter() {
                let edge_manifest = EdgeManifest::parallel(par_edge.from.as_str(), target.as_str());
                builder = builder.add_edge(par_edge.from.as_str(), edge_manifest);
            }
        }

        // Add metadata
        let has_parallel = !self.parallel_edges.is_empty();
        let metadata = GraphMetadata::new().with_parallel_edges(has_parallel);

        builder = builder.metadata(metadata);

        builder
            .build()
            .expect("Entry point is always set for CompiledGraph")
    }

    /// Analyze application architecture for AI self-awareness
    ///
    /// This method enables AI agents to understand how their application is built:
    /// - What DashFlow features are being used?
    /// - What is the graph structure?
    /// - What capabilities does the application have?
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::{StateGraph, platform_registry::AppArchitecture};
    ///
    /// let app = graph.compile()?;
    /// let arch = app.analyze_architecture();
    ///
    /// // AI asks: "What DashFlow features am I using?"
    /// for feature in &arch.dashflow_features_used {
    ///     println!("Using: {} - {}", feature.name, feature.description);
    /// }
    ///
    /// // AI asks: "What's my graph structure?"
    /// println!("I have {} nodes and {} edges",
    ///     arch.graph_structure.node_count,
    ///     arch.graph_structure.edge_count);
    ///
    /// // Export for AI consumption
    /// let json = arch.to_json().unwrap();
    /// ```
    #[must_use]
    pub fn analyze_architecture(&self) -> crate::platform_registry::AppArchitecture {
        use crate::platform_registry::{
            AppArchitecture, ArchitectureGraphInfo, ArchitectureMetadata, Dependency, FeatureUsage,
        };

        let mut builder = AppArchitecture::builder();

        // Analyze graph structure
        let node_names: Vec<String> = self.nodes.keys().cloned().collect();
        let has_cycles = self.detect_cycles();
        let edge_count = self.edges.len()
            + self
                .conditional_edges
                .iter()
                .map(|e| e.routes.len())
                .sum::<usize>()
            + self
                .parallel_edges
                .iter()
                .map(|e| e.to.len())
                .sum::<usize>();

        let graph_info = ArchitectureGraphInfo::new((*self.entry_point).clone())
            .with_node_count(self.nodes.len())
            .with_edge_count(edge_count)
            .with_node_names(node_names)
            .with_cycles(has_cycles)
            .with_conditional_edges(!self.conditional_edges.is_empty())
            .with_parallel_edges(!self.parallel_edges.is_empty());

        let graph_info = if let Some(name) = &self.name {
            graph_info.with_name(name.clone())
        } else {
            graph_info
        };

        builder = builder.graph_structure(graph_info);

        // Detect and add features based on graph structure
        // Core: StateGraph (always used)
        builder.add_feature(
            FeatureUsage::new("StateGraph", "core", "Graph-based workflow orchestration")
                .with_apis(vec![
                    "StateGraph::new",
                    "StateGraph::compile",
                    "CompiledGraph::invoke",
                ])
                .core(),
        );

        // Conditional routing
        if !self.conditional_edges.is_empty() {
            builder.add_feature(
                FeatureUsage::new(
                    "Conditional Routing",
                    "core",
                    "Dynamic routing based on state conditions",
                )
                .with_api("StateGraph::add_conditional_edges"),
            );
        }

        // Parallel execution
        if !self.parallel_edges.is_empty() {
            builder.add_feature(
                FeatureUsage::new(
                    "Parallel Execution",
                    "core",
                    "Execute multiple nodes concurrently",
                )
                .with_api("StateGraph::add_parallel_edges"),
            );
        }

        // Cycles/loops
        if has_cycles {
            builder.add_feature(
                FeatureUsage::new(
                    "Graph Cycles",
                    "core",
                    "Iterative execution with graph cycles",
                )
                .with_api("StateGraph::add_edge"),
            );
        }

        // Checkpointing (if configured)
        if self.checkpointer.is_some() {
            builder.add_feature(
                FeatureUsage::new(
                    "Checkpointing",
                    "checkpoint",
                    "State persistence and recovery",
                )
                .with_apis(vec![
                    "Checkpointer",
                    "MemoryCheckpointer",
                    "SqliteCheckpointer",
                ]),
            );
        }

        // Streaming (if callbacks configured)
        if !self.callbacks.is_empty() {
            builder.add_feature(
                FeatureUsage::new(
                    "Event Callbacks",
                    "streaming",
                    "Real-time execution event handling",
                )
                .with_api("EventCallback"),
            );
        }

        // Add core dependency
        builder.add_dependency(
            Dependency::new("dashflow", "Core graph orchestration framework")
                .with_version(env!("CARGO_PKG_VERSION"))
                .dashflow(),
        );

        // Add metadata
        let metadata = ArchitectureMetadata::new();
        builder = builder.metadata(metadata);

        builder.build()
    }

    /// Detect if the graph contains cycles
    fn detect_cycles(&self) -> bool {
        use std::collections::HashSet;

        // Build adjacency list
        let mut adjacency: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();

        for edge in self.edges.iter() {
            adjacency
                .entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }

        for cond_edge in self.conditional_edges.iter() {
            for target in cond_edge.routes.values() {
                adjacency
                    .entry(cond_edge.from.as_str())
                    .or_default()
                    .push(target.as_str());
            }
        }

        for par_edge in self.parallel_edges.iter() {
            for target in par_edge.to.iter() {
                adjacency
                    .entry(par_edge.from.as_str())
                    .or_default()
                    .push(target.as_str());
            }
        }

        // DFS-based cycle detection
        let mut visited: HashSet<&str> = HashSet::new();
        let mut rec_stack: HashSet<&str> = HashSet::new();

        fn has_cycle<'a>(
            node: &'a str,
            adjacency: &std::collections::HashMap<&'a str, Vec<&'a str>>,
            visited: &mut HashSet<&'a str>,
            rec_stack: &mut HashSet<&'a str>,
        ) -> bool {
            if rec_stack.contains(node) {
                return true;
            }
            if visited.contains(node) {
                return false;
            }

            visited.insert(node);
            rec_stack.insert(node);

            if let Some(neighbors) = adjacency.get(node) {
                for &neighbor in neighbors {
                    if neighbor != "__end__" && has_cycle(neighbor, adjacency, visited, rec_stack) {
                        return true;
                    }
                }
            }

            rec_stack.remove(node);
            false
        }

        for node in self.nodes.keys() {
            if has_cycle(node.as_str(), &adjacency, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    /// Create an execution context for a given node
    ///
    /// This method generates an `ExecutionContext` that represents the current
    /// execution state at a specific node. AI agents can use this context to
    /// understand where they are in the graph and make informed decisions.
    ///
    /// # Arguments
    ///
    /// * `current_node` - The name of the node currently being executed
    /// * `iteration` - The current iteration count
    /// * `nodes_executed` - List of nodes already executed in this run
    /// * `state` - Optional state to include as a JSON snapshot
    ///
    /// # Returns
    ///
    /// An `ExecutionContext` with all fields populated
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let ctx = graph.create_execution_context(
    ///     "reasoning",
    ///     5,
    ///     &["start".to_string(), "fetch".to_string()],
    ///     Some(&current_state),
    /// );
    ///
    /// // AI can use context to make decisions
    /// if ctx.is_near_limit() {
    ///     // Switch to faster strategy
    /// }
    /// ```
    // SAFETY: expect() is valid because current_node parameter is always provided as required arg
    #[allow(clippy::expect_used)]
    pub fn create_execution_context(
        &self,
        current_node: &str,
        iteration: u32,
        nodes_executed: &[String],
        state: Option<&S>,
    ) -> crate::introspection::ExecutionContext
    where
        S: serde::Serialize,
    {
        use crate::introspection::ExecutionContext;

        // Compute available next nodes based on graph structure
        // Note: For conditional edges, we list ALL possible targets since we can't
        // evaluate the condition without the actual state transition
        let mut available_next = Vec::new();

        // Check simple edges
        for edge in self.edges.iter() {
            if edge.from.as_str() == current_node && edge.to.as_str() != "__end__" {
                available_next.push(edge.to.to_string());
            }
        }

        // Check conditional edges - list all possible targets
        for cond_edge in self.conditional_edges.iter() {
            if cond_edge.from.as_str() == current_node {
                for target in cond_edge.routes.values() {
                    if target.as_str() != "__end__" && !available_next.contains(&target.to_string())
                    {
                        available_next.push(target.to_string());
                    }
                }
            }
        }

        // Check parallel edges
        for edge in self.parallel_edges.iter() {
            if edge.from.as_str() == current_node {
                for target in edge.to.iter() {
                    if target.as_str() != "__end__" && !available_next.contains(target) {
                        available_next.push(target.clone());
                    }
                }
            }
        }

        // Create state snapshot if state is provided
        let state_snapshot = state.and_then(|s| serde_json::to_value(s).ok());

        // Get thread ID if set
        let thread_id_str = self.thread_id.clone();

        ExecutionContext::builder()
            .current_node(current_node)
            .iteration(iteration)
            .nodes_executed(nodes_executed.to_vec())
            .available_next_nodes(available_next)
            .recursion_limit(self.recursion_limit)
            .thread_id(thread_id_str.unwrap_or_default())
            .is_interrupted(false)
            .state_snapshot(state_snapshot.unwrap_or(serde_json::Value::Null))
            .build()
            .expect("current_node is always set")
    }

    /// Get the capability manifest for this graph
    ///
    /// This method returns a `CapabilityManifest` that enumerates all the tools,
    /// models, and storage backends available to the graph. AI agents can use
    /// this to discover what capabilities they have access to.
    ///
    /// # Returns
    ///
    /// A `CapabilityManifest` describing available capabilities
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let caps = graph.capabilities();
    ///
    /// // AI can ask: "Can I write files?"
    /// if caps.has_tool("write_file") {
    ///     // File writing is available
    /// }
    ///
    /// // AI can ask: "Which LLMs can I use?"
    /// for model_name in caps.model_names() {
    ///     println!("Available model: {}", model_name);
    /// }
    /// ```
    #[must_use]
    pub fn capabilities(&self) -> crate::introspection::CapabilityManifest {
        use crate::introspection::{
            CapabilityManifestBuilder, StorageBackend, StorageFeature, StorageType,
        };

        // Collect storage backends
        let mut storage = Vec::new();
        if self.checkpointer.is_some() {
            storage.push(
                StorageBackend::new("checkpointer", StorageType::Database)
                    .with_description("Graph checkpoint storage")
                    .with_feature(StorageFeature::Persistent),
            );
        }

        // Build the capability manifest
        // Note: Tools and models need to be registered separately via the builder
        // when the graph is configured with specific LLM providers or tool executors.
        // Future enhancement: store tool/model metadata in node configurations.
        let mut builder = CapabilityManifestBuilder::new();

        if !storage.is_empty() {
            builder = builder.storage(storage);
        }

        builder.build()
    }

    /// Get DashFlow platform knowledge for AI self-awareness
    ///
    /// This method enables AI agents to understand what the DashFlow platform provides:
    /// - What APIs are available?
    /// - What features can I use?
    /// - What crates exist in the ecosystem?
    ///
    /// **Default-enabled:** Works with zero configuration.
    /// **Opt-out:** Use `without_introspection()` to disable.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::StateGraph;
    ///
    /// let app = graph.compile()?;
    /// let platform = app.platform();
    ///
    /// // AI asks: "What can DashFlow do?"
    /// for feature in &platform.features {
    ///     println!("Feature: {} - {}", feature.name, feature.description);
    /// }
    ///
    /// // AI asks: "What modules are available?"
    /// for module in &platform.modules {
    ///     println!("Module: {}", module.name);
    /// }
    ///
    /// // Export as JSON for AI consumption
    /// let json = platform.to_json().unwrap();
    /// ```
    #[must_use]
    pub fn platform(&self) -> crate::platform_registry::PlatformRegistry {
        crate::platform_registry::PlatformRegistry::discover()
    }

    /// Check if introspection is enabled for this graph
    ///
    /// Returns `true` if introspection methods are enabled (default),
    /// `false` if `without_introspection()` was called.
    #[must_use]
    pub fn introspection_enabled(&self) -> bool {
        self.introspection_enabled
    }

    /// Unified AI self-knowledge API
    ///
    /// This method provides a combined interface for all AI introspection capabilities:
    /// - Graph structure (manifest)
    /// - Platform knowledge
    /// - App architecture
    /// - Runtime capabilities
    ///
    /// **Default-enabled:** Works with zero configuration.
    /// **Opt-out:** Use `without_introspection()` to disable.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::StateGraph;
    ///
    /// let app = graph.compile()?;
    ///
    /// // Get all introspection data at once
    /// let knowledge = app.introspect();
    ///
    /// // AI asks: "What am I?"
    /// println!("Graph: {:?}", knowledge.manifest.graph_name);
    /// println!("Nodes: {}", knowledge.manifest.nodes.len());
    ///
    /// // AI asks: "What can DashFlow do?"
    /// println!("Features: {}", knowledge.platform.features.len());
    ///
    /// // AI asks: "What features am I using?"
    /// println!("Using: {:?}", knowledge.architecture.dashflow_features_used);
    /// ```
    #[must_use]
    pub fn introspect(&self) -> GraphIntrospection {
        GraphIntrospection {
            manifest: self.manifest(),
            platform: self.platform(),
            architecture: self.analyze_architecture(),
            capabilities: self.capabilities(),
        }
    }

    /// Unified three-level introspection API.
    ///
    /// Returns a complete view of all three introspection levels:
    /// - **Platform**: DashFlow framework capabilities (shared by all apps)
    /// - **App**: Application-specific configuration (this compiled graph)
    /// - **Live**: Runtime execution state (requires attached `ExecutionTracker`)
    ///
    /// This is the recommended method for AI agents to achieve complete self-awareness.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::live_introspection::ExecutionTracker;
    /// use std::sync::Arc;
    ///
    /// let tracker = Arc::new(ExecutionTracker::new());
    /// let app = graph.compile()?
    ///     .with_execution_tracker(tracker.clone());
    ///
    /// // Get unified introspection
    /// let unified = app.unified_introspection();
    ///
    /// // Platform level - "What is DashFlow?"
    /// println!("DashFlow version: {}", unified.platform.version_info().version);
    /// println!("Available features: {:?}", unified.platform.available_features());
    ///
    /// // App level - "What is this specific application?"
    /// println!("Graph name: {}", unified.app.manifest.graph_name);
    /// println!("Nodes: {:?}", unified.app.manifest.nodes);
    ///
    /// // Live level - "What's happening right now?"
    /// println!("Active executions: {}", unified.active_execution_count());
    /// for exec in &unified.live {
    ///     println!("  {} at {} ({})", exec.execution_id, exec.current_node, exec.status);
    /// }
    /// ```
    #[must_use]
    pub fn unified_introspection(&self) -> UnifiedIntrospection {
        let live = self
            .execution_tracker
            .as_ref()
            .map(|tracker| tracker.all_executions())
            .unwrap_or_default();

        UnifiedIntrospection {
            platform: self.platform_introspection(),
            app: self.introspect(),
            live,
        }
    }

    /// Platform-level introspection: DashFlow framework capabilities.
    ///
    /// Returns information about the DashFlow framework itself - its version,
    /// available features, supported node types, edge types, and built-in templates.
    /// This information is shared by ALL DashFlow applications.
    ///
    /// For app-specific information, use `introspect()` or `unified_introspection()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?;
    ///
    /// // Get platform capabilities
    /// let platform = app.platform_introspection();
    ///
    /// println!("DashFlow version: {}", platform.version_info().version);
    ///
    /// // List available features
    /// for feature in platform.available_features() {
    ///     println!("{}: {}", feature.name, feature.description);
    /// }
    ///
    /// // Query specific capabilities
    /// if let Some(cap) = platform.query_capability("checkpointing") {
    ///     println!("Checkpointing: {}", cap.description);
    /// }
    /// ```
    #[must_use]
    pub fn platform_introspection(&self) -> crate::platform_introspection::PlatformIntrospection {
        crate::platform_introspection::PlatformIntrospection::discover()
    }

    /// Live execution summaries: Currently active and recently completed executions.
    ///
    /// Returns execution summaries from the attached `ExecutionTracker`, or an
    /// empty vector if no tracker is attached.
    ///
    /// For complete introspection including platform and app levels, use
    /// `unified_introspection()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::live_introspection::ExecutionTracker;
    /// use std::sync::Arc;
    ///
    /// let tracker = Arc::new(ExecutionTracker::new());
    /// let app = graph.compile()?
    ///     .with_execution_tracker(tracker);
    ///
    /// // Get live execution state
    /// for exec in app.live_executions() {
    ///     println!("{}: {} at {}", exec.execution_id, exec.status, exec.current_node);
    /// }
    /// ```
    #[must_use]
    pub fn live_executions(&self) -> Vec<crate::live_introspection::ExecutionSummary> {
        self.execution_tracker
            .as_ref()
            .map(|tracker| tracker.all_executions())
            .unwrap_or_default()
    }

    // ========================================================================
    // Dynamic Graph Reconfiguration
    // ========================================================================

    /// Apply a mutation to modify the graph structure or configuration.
    ///
    /// This enables AI agents to adapt their own execution graph at runtime
    /// based on performance data, learned patterns, or explicit directives.
    ///
    /// # Supported Mutations
    ///
    /// - `AdjustTimeout`: Change timeout for a specific node
    /// - `AdjustRecursionLimit`: Modify the graph's recursion limit
    /// - `SetInterrupt`/`ClearInterrupt`: Add or remove human-in-the-loop points
    /// - `AddRetry`: Configure retry behavior for a node (metadata only)
    /// - `AddEdge`/`RemoveEdge`: Modify graph connectivity
    /// - `ChangeToParallel`: Convert sequential nodes to parallel (metadata only)
    /// - `AddCache`: Insert caching before a node (metadata only)
    ///
    /// # Limitations
    ///
    /// Some mutations (like `AddCache`, `ChangeToParallel`, `AddRetry`) are recorded
    /// as metadata but require recompilation to take full effect. Use `recompile()`
    /// after applying these mutations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::graph_reconfiguration::{GraphMutation, MutationType};
    ///
    /// // Increase recursion limit based on graph complexity
    /// let mutation = GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 100 })
    ///     .with_reason("Complex recursive workflow detected");
    ///
    /// compiled.apply_mutation(mutation)?;
    /// ```
    pub fn apply_mutation(
        &mut self,
        mutation: crate::graph_reconfiguration::GraphMutation,
    ) -> Result<crate::graph_reconfiguration::MutationResult> {
        use crate::graph_reconfiguration::{validate_mutation, MutationResult, MutationType};

        // Validate the mutation
        let warnings = validate_mutation(&mutation, &self.nodes, &self.edges)?;

        let affected_nodes = mutation.mutation_type.target_nodes();

        // Apply the mutation based on type
        match &mutation.mutation_type {
            MutationType::AdjustRecursionLimit { limit } => {
                self.recursion_limit = *limit;
            }

            MutationType::AdjustTimeout { node, timeout } => {
                // Store per-node timeout in node timeouts map
                // For now, we set the global node timeout if the node matches entry
                // Full per-node timeout support requires additional infrastructure
                if node == self.entry_point.as_str() {
                    self.node_timeout = Some(*timeout);
                }
                // Note: Full per-node timeout support is recorded but requires
                // additional infrastructure to apply during execution
            }

            MutationType::SetInterrupt { node, before } => {
                if *before {
                    if !self.interrupt_before.contains(node) {
                        self.interrupt_before.push(node.clone());
                    }
                } else if !self.interrupt_after.contains(node) {
                    self.interrupt_after.push(node.clone());
                }
            }

            MutationType::ClearInterrupt { node, before } => {
                if *before {
                    self.interrupt_before.retain(|n| n != node);
                } else {
                    self.interrupt_after.retain(|n| n != node);
                }
            }

            MutationType::AddEdge { from, to } => {
                // Add a new edge (requires Arc::make_mut for modification)
                let edges = Arc::make_mut(&mut self.edges);
                let new_edge = Edge::new(from.clone(), to.clone());
                if !edges
                    .iter()
                    .any(|e| e.from.as_str() == from && e.to.as_str() == to)
                {
                    edges.push(new_edge);
                }
            }

            MutationType::RemoveEdge { from, to } => {
                // Remove an edge
                let edges = Arc::make_mut(&mut self.edges);
                edges.retain(|e| !(e.from.as_str() == from && e.to.as_str() == to));
            }

            // These mutations are recorded but require recompilation for full effect
            MutationType::AddCache { .. }
            | MutationType::ChangeToParallel { .. }
            | MutationType::AddRetry { .. } => {
                // These mutations affect node behavior and require graph restructuring
                // They are logged for the AI to track but need recompile() to apply
                tracing::info!(
                    mutation = %mutation.description(),
                    "Mutation recorded (requires recompilation for full effect)"
                );
            }
        }

        if warnings.is_empty() {
            Ok(MutationResult::success(mutation, affected_nodes))
        } else {
            Ok(MutationResult::success_with_warnings(
                mutation,
                affected_nodes,
                warnings,
            ))
        }
    }

    /// Analyze execution trace and generate optimization suggestions.
    ///
    /// This method enables AI self-optimization by analyzing execution bottlenecks
    /// and generating suggested mutations to improve performance.
    ///
    /// # Arguments
    ///
    /// * `trace` - An execution trace from a previous run
    ///
    /// # Returns
    ///
    /// An `OptimizationSuggestions` containing:
    /// - Suggested mutations based on detected bottlenecks
    /// - Health score (0.0 to 1.0)
    /// - Summary of analysis findings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After execution, get the trace
    /// let trace = compiled.get_execution_trace(&result);
    ///
    /// // Analyze and get suggestions
    /// let suggestions = compiled.suggest_optimizations(&trace);
    ///
    /// println!("Health score: {:.0}%", suggestions.health_score * 100.0);
    /// println!("{}", suggestions.summary);
    ///
    /// // Apply high-confidence suggestions
    /// for mutation in suggestions.high_confidence_mutations(0.7) {
    ///     println!("Applying: {}", mutation.description());
    ///     compiled.apply_mutation(mutation.clone())?;
    /// }
    /// ```
    #[must_use]
    pub fn suggest_optimizations(
        &self,
        trace: &crate::introspection::ExecutionTrace,
    ) -> crate::graph_reconfiguration::OptimizationSuggestions {
        use crate::graph_reconfiguration::OptimizationSuggestions;

        // Detect bottlenecks using the trace's built-in analysis
        let analysis = trace.detect_bottlenecks();

        // Generate optimization suggestions based on the analysis
        OptimizationSuggestions::new(analysis)
    }

    /// Analyze and automatically apply high-confidence optimizations.
    ///
    /// This method combines `suggest_optimizations` and `apply_mutation` to
    /// enable fully autonomous self-optimization. Only mutations above the
    /// confidence threshold are applied.
    ///
    /// # Arguments
    ///
    /// * `trace` - An execution trace from a previous run
    /// * `confidence_threshold` - Minimum confidence (0.0-1.0) for auto-apply
    ///
    /// # Returns
    ///
    /// A vector of `MutationResult`s for each applied mutation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Self-optimize with 70% confidence threshold
    /// let results = compiled.self_optimize(&trace, 0.7)?;
    ///
    /// println!("Applied {} optimizations:", results.len());
    /// for result in &results {
    ///     println!("  - {}", result.mutation.description());
    /// }
    /// ```
    pub fn self_optimize(
        &mut self,
        trace: &crate::introspection::ExecutionTrace,
        confidence_threshold: f64,
    ) -> Result<Vec<crate::graph_reconfiguration::MutationResult>> {
        let suggestions = self.suggest_optimizations(trace);
        let mut results = Vec::new();

        for mutation in suggestions.high_confidence_mutations(confidence_threshold) {
            let result = self.apply_mutation(mutation.clone())?;
            results.push(result);
        }

        if !results.is_empty() {
            tracing::info!(
                count = results.len(),
                health_score = suggestions.health_score,
                "Self-optimization applied"
            );
        }

        Ok(results)
    }

    /// Generate CLI help text at the specified level of detail.
    ///
    /// This is part of the MCP Self-Documentation Protocol, enabling DashFlow apps
    /// to auto-generate comprehensive help text for `--help`, `--help-more`, and
    /// `--help-implementation` CLI flags.
    ///
    /// # Arguments
    ///
    /// * `level` - The level of detail for the help output
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::mcp_self_doc::HelpLevel;
    ///
    /// let compiled = graph.compile()?;
    ///
    /// // In your main.rs:
    /// if args.contains(&"--help".to_string()) {
    ///     println!("{}", compiled.generate_help(HelpLevel::Brief));
    ///     return Ok(());
    /// }
    ///
    /// if args.contains(&"--help-more".to_string()) {
    ///     println!("{}", compiled.generate_help(HelpLevel::More));
    ///     return Ok(());
    /// }
    ///
    /// if args.contains(&"--help-implementation".to_string()) {
    ///     println!("{}", compiled.generate_help(HelpLevel::Implementation));
    ///     return Ok(());
    /// }
    /// ```
    #[must_use]
    pub fn generate_help(&self, level: crate::mcp_self_doc::HelpLevel) -> String {
        crate::mcp_self_doc::HelpGenerator::new(self.introspect()).generate(level)
    }

    /// Create an MCP Self-Documentation server for this graph.
    ///
    /// The server exposes endpoints for AI-to-AI communication:
    /// - `/mcp/about` - High-level description
    /// - `/mcp/capabilities` - Available tools and features
    /// - `/mcp/architecture` - Graph structure
    /// - `/mcp/implementation` - Code-level details
    /// - `/mcp/introspect` - Natural language query interface
    ///
    /// # Arguments
    ///
    /// * `port` - The port to bind the server to
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let compiled = graph.compile()?;
    /// let server = compiled.mcp_server(8080);
    ///
    /// // Start the server (requires mcp-server feature)
    /// #[cfg(feature = "mcp-server")]
    /// server.start().await?;
    /// ```
    #[must_use]
    pub fn mcp_server(&self, port: u16) -> crate::mcp_self_doc::McpSelfDocServer {
        crate::mcp_self_doc::McpSelfDocServer::new(self.introspect(), port)
    }

    /// Process command-line arguments for help flags.
    ///
    /// This is a convenience method for DashFlow applications to handle
    /// `--help`, `--help-more`, and `--help-implementation` CLI flags automatically.
    ///
    /// Returns `CliHelpResult::Displayed(level)` if help was shown (program should exit),
    /// or `CliHelpResult::Continue` if no help flag was found.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments (typically `std::env::args()`)
    /// * `config` - Optional configuration for customizing help output
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::mcp_self_doc::CliHelpConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let compiled = graph.compile()?;
    ///
    ///     // One-liner to handle all help flags
    ///     if compiled.process_cli_help(std::env::args(), None).should_exit() {
    ///         return Ok(());
    ///     }
    ///
    ///     // Or with custom configuration:
    ///     let config = CliHelpConfig::new()
    ///         .with_app_name("My Agent")
    ///         .with_app_version("2.0.0");
    ///
    ///     if compiled.process_cli_help(std::env::args(), Some(config)).should_exit() {
    ///         return Ok(());
    ///     }
    ///
    ///     // Normal execution...
    ///     compiled.invoke(state).await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn process_cli_help<I, A>(
        &self,
        args: I,
        config: Option<crate::mcp_self_doc::CliHelpConfig>,
    ) -> crate::mcp_self_doc::CliHelpResult
    where
        I: IntoIterator<Item = A>,
        A: AsRef<str>,
    {
        crate::mcp_self_doc::process_cli_help(args, self.introspect(), config)
    }

    /// Disable introspection for this graph
    ///
    /// **Opt-out:** This disables the introspection tracking flag.
    /// Introspection methods will still work, but `introspection_enabled()` will return `false`.
    ///
    /// Use this if you need to explicitly disable introspection for resource reasons
    /// or when building minimal graphs that don't need AI self-awareness.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?
    ///     .without_introspection();  // Explicit opt-out
    ///
    /// // Introspection methods still work, but flag is false
    /// assert!(!app.introspection_enabled());
    /// ```
    pub fn without_introspection(mut self) -> Self {
        self.introspection_enabled = false;
        self
    }

    /// Get possible next nodes from a given node (without evaluating conditions)
    ///
    /// Returns all nodes that could potentially be reached from the specified node,
    /// including all branches of conditional edges. This is useful for AI introspection
    /// to understand the graph structure without executing it.
    ///
    /// # Arguments
    ///
    /// * `node` - The node name to find successors for
    ///
    /// # Returns
    ///
    /// A vector of node names that could be reached from this node
    #[must_use]
    pub fn possible_next_nodes(&self, node: &str) -> Vec<String> {
        let mut result = Vec::new();

        // Simple edges
        for edge in self.edges.iter() {
            if edge.from.as_str() == node {
                result.push(edge.to.to_string());
            }
        }

        // Conditional edges - all possible targets
        for cond_edge in self.conditional_edges.iter() {
            if cond_edge.from.as_str() == node {
                for target in cond_edge.routes.values() {
                    if !result.contains(&target.to_string()) {
                        result.push(target.to_string());
                    }
                }
            }
        }

        // Parallel edges
        for edge in self.parallel_edges.iter() {
            if edge.from.as_str() == node {
                for target in edge.to.iter() {
                    if !result.contains(target) {
                        result.push(target.clone());
                    }
                }
            }
        }

        result
    }

    /// Compute a structural hash of the compiled graph for change detection.
    ///
    /// The hash is based on:
    /// - Node names (not node implementations)
    /// - Edge definitions (from/to)
    /// - Entry point
    ///
    /// This is used by `StateGraph::compile_delta()` to detect whether the
    /// graph structure has changed since the last compilation.
    ///
    /// # Returns
    ///
    /// A 64-bit hash of the graph structure.
    #[must_use]
    pub fn structural_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash entry point
        self.entry_point.hash(&mut hasher);

        // Hash node names (sorted for determinism)
        let mut node_names: Vec<_> = self.nodes.keys().collect();
        node_names.sort();
        for name in node_names {
            name.hash(&mut hasher);
        }

        // Hash simple edges (sorted for determinism)
        let mut edge_strs: Vec<_> = self
            .edges
            .iter()
            .map(|e| format!("{}→{}", e.from, e.to))
            .collect();
        edge_strs.sort();
        for edge in edge_strs {
            edge.hash(&mut hasher);
        }

        // Hash conditional edges (source nodes only, sorted)
        let mut cond_strs: Vec<_> = self
            .conditional_edges
            .iter()
            .map(|e| {
                let mut routes: Vec<_> = e
                    .routes
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect();
                routes.sort();
                format!("{}?{}", e.from, routes.join(","))
            })
            .collect();
        cond_strs.sort();
        for cond in cond_strs {
            cond.hash(&mut hasher);
        }

        // Hash parallel edges (sorted)
        let mut par_strs: Vec<_> = self
            .parallel_edges
            .iter()
            .map(|e| {
                let mut targets: Vec<_> = e.to.to_vec();
                targets.sort();
                format!("{}||{}", e.from, targets.join(","))
            })
            .collect();
        par_strs.sort();
        for par in par_strs {
            par.hash(&mut hasher);
        }

        hasher.finish()
    }
}

/// Result of graph execution
#[derive(Debug, Clone)]
pub struct ExecutionResult<S>
where
    S: crate::state::GraphState,
{
    /// Final state after execution
    pub final_state: S,
    /// List of nodes executed in order
    pub nodes_executed: Vec<String>,
    /// Node where execution was interrupted (for human-in-the-loop)
    /// If Some, execution paused at this node and can be resumed with `resume()`
    pub interrupted_at: Option<String>,
    /// Nodes to execute next when resuming from interrupt
    /// Empty if execution completed normally
    pub next_nodes: Vec<String>,
}

impl<S> ExecutionResult<S>
where
    S: crate::state::GraphState,
{
    /// Get the final state
    pub fn state(&self) -> &S {
        &self.final_state
    }

    /// Get the list of executed nodes
    pub fn execution_path(&self) -> &[String] {
        &self.nodes_executed
    }
}

// Note: Automatic dispatch to MergeableState::merge() requires trait specialization
// which is not yet stable in Rust. For now, users implementing MergeableState
// should add an explicit aggregator node after parallel execution to merge results.
//
// Example pattern:
// ```
// graph.add_parallel_edges("split", vec!["researcher", "analyst"]);
// graph.add_node("aggregate", |mut state: ResearchState| {
//     // Manual merge happens here via MergeableState::merge
//     Box::pin(async move { Ok(state) })
// });
// graph.add_edge("researcher", "aggregate");
// graph.add_edge("analyst", "aggregate");
// ```

#[cfg(test)]
mod tests;
