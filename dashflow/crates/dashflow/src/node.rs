// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for node module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Node trait and implementations
//!
//! Nodes are the computational units in a DashFlow graph. Each node receives state,
//! processes it, and returns updated state.
//!
//! # Performance Tips
//!
//! - Return modified state directly (no unnecessary clones)
//! - Prefer `Vec` over `HashMap` for state collections
//! - Use `Arc` for large read-only data shared across nodes
//! - Keep state minimal - only data needed for decision-making
//!
//! See module-level docs in `graph.rs` and `docs/ARCHITECTURE.md` for details.

use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

use crate::error::Result;

#[cfg(feature = "dashstream")]
use dashflow_streaming::{
    attribute_value, metric_value, producer::DashStreamProducer, AttributeValue,
    Error as ProtoError, Event, EventType, Header, MessageType, MetricValue, Metrics, TokenChunk,
    ToolExecution,
};
#[cfg(feature = "dashstream")]
use std::collections::HashMap;
#[cfg(feature = "dashstream")]
use std::sync::atomic::{AtomicU64, Ordering};

/// A node in the graph that processes state
///
/// Nodes are the core computational units. They receive state, perform
/// operations (LLM calls, tool execution, data processing), and return
/// updated state.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::Node;
/// use async_trait::async_trait;
///
/// struct ResearchNode;
///
/// #[async_trait]
/// impl Node<AgentState> for ResearchNode {
///     async fn execute(&self, state: AgentState) -> Result<AgentState> {
///         // Perform research
///         let mut state = state;
///         state.messages.push("Research complete".to_string());
///         Ok(state)
///     }
/// }
/// ```
///
/// # See Also
///
/// - [`FunctionNode`] - Use functions as nodes without implementing the trait
/// - [`BoxedNode`] - Type-erased node for dynamic dispatch
/// - [`StateGraph`](crate::StateGraph) - Builder for creating graphs with nodes
/// - [`CompiledGraph`](crate::CompiledGraph) - Executes graphs of nodes
/// - `NodeContext` - Streaming context for telemetry (with `dashstream` feature)
#[async_trait]
pub trait Node<S>: Send + Sync + std::any::Any
where
    S: Send + Sync,
{
    /// Execute with streaming context (NEW)
    ///
    /// Nodes that want to emit telemetry during execution should
    /// override this method. Default implementation ignores context
    /// and calls execute() for backward compatibility.
    ///
    /// # Arguments
    /// * `state` - Current graph state
    /// * `ctx` - Execution context for emitting telemetry
    ///
    /// # Returns
    /// Updated state after node execution
    ///
    /// # Example
    /// ```rust,ignore
    /// async fn execute_with_context(&self, state: S, ctx: &NodeContext) -> Result<S> {
    ///     ctx.send_progress("Starting...", 0.1).await?;
    ///     // ... work ...
    ///     ctx.send_progress("Complete", 1.0).await?;
    ///     Ok(state)
    /// }
    /// ```
    #[cfg(feature = "dashstream")]
    async fn execute_with_context(&self, state: S, _ctx: &NodeContext) -> Result<S>
    where
        S: 'static,
    {
        // Default: Ignore context, call existing execute method
        self.execute(state).await
    }

    /// Execute this node with the given state
    ///
    /// # Arguments
    ///
    /// * `state` - Current graph state
    ///
    /// # Returns
    ///
    /// Updated state after node execution
    async fn execute(&self, state: S) -> Result<S>;

    /// Does this node support streaming? (NEW)
    ///
    /// Return true if this node overrides execute_with_context() and wants
    /// to emit telemetry. Executor will only create context for nodes that
    /// return true here.
    ///
    /// Default: false (most nodes don't need streaming)
    #[cfg(feature = "dashstream")]
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Get the name of this node (for debugging and tracing)
    fn name(&self) -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("Node")
            .to_string()
    }

    /// Indicates whether this node is read-only (does not mutate state).
    ///
    /// When this returns `true`, the executor may skip cloning the state
    /// before passing it to this node, improving performance for large states.
    ///
    /// # Safety
    ///
    /// Returning `true` when the node actually mutates state will cause
    /// undefined behavior in the execution flow. Only return `true` if:
    /// - The node only reads from state
    /// - The node returns the input state unmodified (or with only additions)
    /// - The node does not rely on having exclusive ownership of state
    ///
    /// # Default
    ///
    /// Returns `false` for safety - all nodes are assumed to potentially
    /// mutate state unless explicitly marked as read-only.
    fn is_read_only(&self) -> bool {
        false
    }

    /// Indicates whether this node is optimizable via DashOpt algorithms.
    ///
    /// Optimizable nodes implement the `Optimizable` trait and can be improved
    /// using techniques like BootstrapFewShot, MIPROv2, and other optimization
    /// algorithms.
    ///
    /// # Default
    ///
    /// Returns `false` - most nodes are not optimizable by default.
    /// Override this in nodes that implement `Optimizable`.
    fn is_optimizable(&self) -> bool {
        false
    }

    /// Indicates whether this node makes LLM/model calls.
    ///
    /// Nodes that return `true` but `is_optimizable()` returns `false` will
    /// trigger a validation warning, since LLM calls should flow through
    /// optimizable abstractions for production use.
    ///
    /// # Default
    ///
    /// Returns `false` - most nodes don't make LLM calls.
    /// Override this in nodes that wrap ChatModel or similar.
    fn may_use_llm(&self) -> bool {
        false
    }

    /// Provide access to Any for downcasting
    ///
    /// This enables checking if a node implements specific traits at runtime.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Provide mutable access to Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Type-erased node for dynamic dispatch
///
/// This is `Arc<dyn Node<S>>`, allowing nodes of different concrete types to be
/// stored in the same collection. The `Arc` wrapper enables shared ownership and
/// cheap cloning of node references.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::BoxedNode;
/// use std::sync::Arc;
///
/// // Convert a concrete node to BoxedNode
/// let node: BoxedNode<MyState> = Arc::new(MyNode::new());
///
/// // Nodes can be cloned cheaply (just Arc clone)
/// let node_copy = node.clone();
/// ```
///
/// # See Also
///
/// - [`Node`] - The underlying trait
/// - [`StateGraph`](crate::StateGraph) - Uses `BoxedNode` internally
pub type BoxedNode<S> = Arc<dyn Node<S>>;

/// A node that wraps a simple async function
///
/// This allows using closures or functions as nodes without implementing the [`Node`]
/// trait directly. The function receives the current state and returns the updated state.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, FunctionNode, END};
///
/// // Using add_node_from_fn (recommended)
/// graph.add_node_from_fn("process", |state: MyState| {
///     Box::pin(async move {
///         let mut state = state;
///         state.count += 1;
///         Ok(state)
///     })
/// });
///
/// // Or manually create a FunctionNode
/// let node = FunctionNode::new("process", |state: MyState| {
///     Box::pin(async move {
///         Ok(state)
///     })
/// });
/// graph.add_node("process", node);
/// ```
///
/// # See Also
///
/// - [`Node`] - The trait this wraps
/// - [`StateGraph::add_node_from_fn`](crate::StateGraph::add_node_from_fn) - Convenience method
/// - [`BoxedNode`] - Type-erased node storage
pub struct FunctionNode<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + 'static,
{
    func: F,
    name: String,
    _phantom: std::marker::PhantomData<S>,
}

impl<S, F> FunctionNode<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + 'static,
{
    /// Create a new function node
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            func,
            name: name.into(),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<S, F> Node<S> for FunctionNode<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        (self.func)(state).await
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S, F> fmt::Debug for FunctionNode<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionNode")
            .field("name", &self.name)
            .finish()
    }
}

/// Default maximum concurrent telemetry sends for NodeContext
/// Limits runtime pressure from telemetry spikes in nodes
#[cfg(feature = "dashstream")]
pub const DEFAULT_NODE_MAX_CONCURRENT_SENDS: usize = 32;

/// Execution context for nodes that emit telemetry during execution
///
/// NodeContext provides nodes with the ability to stream telemetry messages
/// (progress, thinking, tokens, tools, metrics, errors) during execution.
/// When a producer is available, messages are sent fire-and-forget to avoid
/// blocking node execution. When no producer is available, all methods are no-ops.
///
/// Flow control: Telemetry sends are bounded by a semaphore
/// to prevent runtime starvation during high telemetry volume.
///
/// # Example
///
/// ```rust,ignore
/// async fn execute_with_context(&self, state: S, ctx: &NodeContext) -> Result<S> {
///     ctx.send_progress("Starting analysis...", 0.1).await?;
///     // ... work ...
///     ctx.send_thinking("User wants X, searching Y", 1).await?;
///     // ... work ...
///     ctx.send_progress("Complete", 1.0).await?;
///     Ok(state)
/// }
/// ```
#[cfg(feature = "dashstream")]
#[derive(Clone)]
pub struct NodeContext {
    /// Node name (for message attribution)
    node_name: String,

    /// Optional producer for streaming (None = no-op mode)
    producer: Option<Arc<DashStreamProducer>>,

    /// Thread/session metadata
    thread_id: String,
    tenant_id: String,

    /// Sequence counter for intra-node messages
    sequence: Arc<AtomicU64>,

    /// Flow control semaphore for bounded telemetry sends
    telemetry_semaphore: Arc<tokio::sync::Semaphore>,

    /// Counter for dropped telemetry messages
    telemetry_dropped: Arc<AtomicU64>,
}

#[cfg(feature = "dashstream")]
impl NodeContext {
    /// Create context with producer (streaming enabled)
    pub fn new(
        node_name: String,
        producer: Option<Arc<DashStreamProducer>>,
        thread_id: String,
        tenant_id: String,
    ) -> Self {
        Self::with_max_concurrent(
            node_name,
            producer,
            thread_id,
            tenant_id,
            DEFAULT_NODE_MAX_CONCURRENT_SENDS,
        )
    }

    /// Create context with custom max concurrent sends
    pub fn with_max_concurrent(
        node_name: String,
        producer: Option<Arc<DashStreamProducer>>,
        thread_id: String,
        tenant_id: String,
        max_concurrent_sends: usize,
    ) -> Self {
        Self {
            node_name,
            producer,
            thread_id,
            tenant_id,
            sequence: Arc::new(AtomicU64::new(0)),
            telemetry_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent_sends)),
            telemetry_dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create no-op context (for nodes without streaming)
    pub fn empty() -> Self {
        Self {
            node_name: String::new(),
            producer: None,
            thread_id: String::new(),
            tenant_id: String::new(),
            sequence: Arc::new(AtomicU64::new(0)),
            telemetry_semaphore: Arc::new(tokio::sync::Semaphore::new(
                DEFAULT_NODE_MAX_CONCURRENT_SENDS,
            )),
            telemetry_dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get count of dropped telemetry messages due to flow control
    pub fn telemetry_dropped_count(&self) -> u64 {
        self.telemetry_dropped.load(Ordering::Relaxed)
    }

    /// Spawn a telemetry task with flow control
    ///
    /// Uses try_acquire for non-blocking flow control. If at capacity,
    /// the message is dropped and counted.
    fn spawn_with_flow_control<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        match self.telemetry_semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                tokio::spawn(async move {
                    future.await;
                    drop(permit);
                });
            }
            Err(_) => {
                let dropped = self.telemetry_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped % 100 == 1 {
                    tracing::warn!(
                        node = %self.node_name,
                        dropped_count = dropped,
                        "NodeContext telemetry dropped due to flow control"
                    );
                }
            }
        }
    }

    /// Create message header (internal helper)
    fn create_header(&self, message_type: MessageType) -> Header {
        Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            // Use unwrap_or_default to handle edge case of system clock before UNIX_EPOCH
            // Safety: Saturate to i64::MAX on overflow. In practice, i64 microseconds
            // can represent timestamps until year ~292,471 CE, well beyond practical use.
            timestamp_us: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros()
                .try_into()
                .unwrap_or(i64::MAX),
            tenant_id: self.tenant_id.clone(),
            thread_id: self.thread_id.clone(),
            // Sequence numbers are 1-based to align with DashStreamProducer and SequenceValidator.
            sequence: self.sequence.fetch_add(1, Ordering::SeqCst) + 1,
            r#type: message_type as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
        }
    }

    // ========================================
    // HIGH-LEVEL API (Simple to use)
    // ========================================

    /// Send progress update (percent 0.0-1.0)
    pub async fn send_progress(&self, message: &str, percent: f64) -> Result<()> {
        if let Some(producer) = &self.producer {
            let mut attributes = HashMap::new();
            attributes.insert(
                "message".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::StringValue(message.to_string())),
                },
            );
            attributes.insert(
                "percent".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::FloatValue(percent)),
                },
            );

            let event = Event {
                header: Some(self.create_header(MessageType::Event)),
                event_type: EventType::NodeProgress as i32,
                node_id: self.node_name.clone(),
                attributes,
                duration_us: 0,
                llm_request_id: String::new(),
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_event(event).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send progress telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    /// Send thinking/reasoning step (for LLM chain-of-thought)
    pub async fn send_thinking(&self, thought: &str, step: u32) -> Result<()> {
        if let Some(producer) = &self.producer {
            let mut attributes = HashMap::new();
            attributes.insert(
                "thought".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::StringValue(thought.to_string())),
                },
            );
            attributes.insert(
                "step".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::IntValue(step as i64)),
                },
            );

            let event = Event {
                header: Some(self.create_header(MessageType::Event)),
                event_type: EventType::NodeThinking as i32,
                node_id: self.node_name.clone(),
                attributes,
                duration_us: 0,
                llm_request_id: String::new(),
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_event(event).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send thinking telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    /// Send substep completion
    pub async fn send_substep(&self, name: &str, status: &str) -> Result<()> {
        if let Some(producer) = &self.producer {
            let mut attributes = HashMap::new();
            attributes.insert(
                "substep_name".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::StringValue(name.to_string())),
                },
            );
            attributes.insert(
                "status".to_string(),
                AttributeValue {
                    value: Some(attribute_value::Value::StringValue(status.to_string())),
                },
            );

            let event = Event {
                header: Some(self.create_header(MessageType::Event)),
                event_type: EventType::NodeSubstep as i32,
                node_id: self.node_name.clone(),
                attributes,
                duration_us: 0,
                llm_request_id: String::new(),
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_event(event).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send substep telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    // ========================================
    // LOW-LEVEL API (Full protocol access)
    // ========================================

    /// Send token chunk (for LLM streaming)
    pub async fn send_token(
        &self,
        text: &str,
        chunk_index: u32,
        is_final: bool,
        request_id: &str,
    ) -> Result<()> {
        if let Some(producer) = &self.producer {
            let chunk = TokenChunk {
                header: Some(self.create_header(MessageType::TokenChunk)),
                request_id: request_id.to_string(),
                text: text.to_string(),
                token_ids: vec![],
                logprobs: vec![],
                chunk_index,
                is_final,
                finish_reason: 0,
                model: String::new(),
                stats: None,
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_token_chunk(chunk).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send token chunk telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    /// Send tool execution event
    pub async fn send_tool_event(
        &self,
        call_id: &str,
        tool_name: &str,
        stage: i32, // ExecutionStage as i32
        duration_us: i64,
    ) -> Result<()> {
        if let Some(producer) = &self.producer {
            let tool = ToolExecution {
                header: Some(self.create_header(MessageType::ToolExecution)),
                call_id: call_id.to_string(),
                tool_name: tool_name.to_string(),
                stage,
                arguments: vec![],
                result: vec![],
                error: String::new(),
                error_details: None,
                duration_us,
                retry_count: 0,
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_tool_execution(tool).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send tool execution telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    /// Send custom metric
    pub async fn send_metric(&self, metric_name: &str, value: f64, unit: &str) -> Result<()> {
        if let Some(producer) = &self.producer {
            let mut metrics_map = HashMap::new();
            let metric_value = MetricValue {
                value: Some(metric_value::Value::FloatValue(value)),
                unit: unit.to_string(),
                r#type: 2, // METRIC_TYPE_GAUGE
            };
            metrics_map.insert(metric_name.to_string(), metric_value);

            let metrics = Metrics {
                header: Some(self.create_header(MessageType::Metrics)),
                scope: "node".to_string(),
                scope_id: self.node_name.clone(),
                metrics: metrics_map,
                tags: HashMap::new(),
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_metrics(metrics).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send metrics telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }

    /// Send non-fatal error/warning
    pub async fn send_error(&self, error_code: &str, message: &str, severity: i32) -> Result<()> {
        if let Some(producer) = &self.producer {
            let error = ProtoError {
                header: Some(self.create_header(MessageType::Error)),
                error_code: error_code.to_string(),
                message: message.to_string(),
                stack_trace: String::new(),
                context: HashMap::new(),
                severity,
                exception_type: String::new(),
                suggestions: vec![],
            };

            let prod = producer.clone();
            let thread_id = self.thread_id.clone();
            let node_name = self.node_name.clone();
            self.spawn_with_flow_control(async move {
                if let Err(e) = prod.send_error(error).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        node = %node_name,
                        "Failed to send error telemetry: {e}"
                    );
                }
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestState {
        value: i32,
    }

    struct IncrementNode;

    #[async_trait]
    impl Node<TestState> for IncrementNode {
        async fn execute(&self, state: TestState) -> Result<TestState> {
            Ok(TestState {
                value: state.value + 1,
            })
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_node_execution() {
        let node = IncrementNode;
        let state = TestState { value: 5 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 6);
    }

    #[tokio::test]
    async fn test_function_node() {
        let node = FunctionNode::new("double", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value * 2,
                })
            })
        });

        let state = TestState { value: 3 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 6);
        assert_eq!(node.name(), "double");
    }

    #[test]
    fn test_node_default_name() {
        // Test default name() implementation for IncrementNode
        let node = IncrementNode;
        let name = node.name();
        // Should extract last component of type name
        assert!(name.contains("IncrementNode") || name == "IncrementNode");
    }

    #[test]
    fn test_function_node_debug_format() {
        let node = FunctionNode::new("test_node", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });

        // Verify Debug trait implementation
        let debug_output = format!("{:?}", node);
        assert!(debug_output.contains("FunctionNode"));
        assert!(debug_output.contains("test_node"));
    }

    #[tokio::test]
    async fn test_function_node_error_propagation() {
        use crate::error::Error;

        let node = FunctionNode::new("error_node", |_state: TestState| {
            Box::pin(async move {
                Err(Error::NodeExecution {
                    node: "error_node".to_string(),
                    source: "test error".into(),
                })
            })
        });

        let state = TestState { value: 1 };
        let result = node.execute(state).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_function_node_creation() {
        // Test FunctionNode::new constructor
        let node = FunctionNode::new("increment", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        assert_eq!(node.name(), "increment");
    }

    #[tokio::test]
    async fn test_node_trait_with_boxed_node() {
        // Test BoxedNode type alias
        let node: BoxedNode<TestState> = Arc::new(IncrementNode);
        let state = TestState { value: 10 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 11);
    }

    #[tokio::test]
    async fn test_node_sequential_execution() {
        // Test multiple sequential node executions
        let node = IncrementNode;
        let mut state = TestState { value: 0 };

        for i in 1..=10 {
            state = node.execute(state).await.unwrap();
            assert_eq!(state.value, i);
        }
    }

    #[tokio::test]
    async fn test_function_node_complex_transformation() {
        // Test FunctionNode with complex state transformation
        #[derive(Clone)]
        struct ComplexState {
            numbers: Vec<i32>,
            sum: i32,
            product: i32,
        }

        let node = FunctionNode::new("calculator", |state: ComplexState| {
            Box::pin(async move {
                let sum: i32 = state.numbers.iter().sum();
                let product: i32 = state.numbers.iter().product();
                Ok(ComplexState {
                    numbers: state.numbers,
                    sum,
                    product,
                })
            })
        });

        let state = ComplexState {
            numbers: vec![2, 3, 4],
            sum: 0,
            product: 0,
        };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.sum, 9);
        assert_eq!(result.product, 24);
        assert_eq!(node.name(), "calculator");
    }

    #[tokio::test]
    async fn test_function_node_stateful_logic() {
        // Test FunctionNode with conditional logic based on state
        let node = FunctionNode::new("conditional", |state: TestState| {
            Box::pin(async move {
                let new_value = if state.value < 0 {
                    0 // Floor at zero
                } else if state.value > 100 {
                    100 // Cap at 100
                } else {
                    state.value * 2
                };
                Ok(TestState { value: new_value })
            })
        });

        let state_negative = TestState { value: -5 };
        let result = node.execute(state_negative).await.unwrap();
        assert_eq!(result.value, 0);

        let state_normal = TestState { value: 25 };
        let result = node.execute(state_normal).await.unwrap();
        assert_eq!(result.value, 50);

        let state_large = TestState { value: 150 };
        let result = node.execute(state_large).await.unwrap();
        assert_eq!(result.value, 100);
    }

    #[test]
    fn test_function_node_name_special_characters() {
        // Test FunctionNode with special characters in name
        let node1 = FunctionNode::new("node-with-hyphens", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node1.name(), "node-with-hyphens");

        let node2 = FunctionNode::new("node.with.dots", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node2.name(), "node.with.dots");

        let node3 = FunctionNode::new("node:with:colons", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node3.name(), "node:with:colons");

        let node4 = FunctionNode::new("node_with_underscores", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node4.name(), "node_with_underscores");
    }

    #[tokio::test]
    async fn test_boxed_node_with_function_node() {
        // Test BoxedNode with FunctionNode
        let node: BoxedNode<TestState> =
            Arc::new(FunctionNode::new("multiply", |state: TestState| {
                Box::pin(async move {
                    Ok(TestState {
                        value: state.value * 3,
                    })
                })
            }));

        let state = TestState { value: 7 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 21);
        assert_eq!(node.name(), "multiply");
    }

    #[tokio::test]
    async fn test_multiple_boxed_nodes() {
        // Test multiple BoxedNode instances executing sequentially
        let node1: BoxedNode<TestState> = Arc::new(IncrementNode);
        let node2: BoxedNode<TestState> =
            Arc::new(FunctionNode::new("double", |state: TestState| {
                Box::pin(async move {
                    Ok(TestState {
                        value: state.value * 2,
                    })
                })
            }));
        let node3: BoxedNode<TestState> =
            Arc::new(FunctionNode::new("subtract_ten", |state: TestState| {
                Box::pin(async move {
                    Ok(TestState {
                        value: state.value - 10,
                    })
                })
            }));

        let state = TestState { value: 5 };
        let state = node1.execute(state).await.unwrap(); // 5 + 1 = 6
        assert_eq!(state.value, 6);

        let state = node2.execute(state).await.unwrap(); // 6 * 2 = 12
        assert_eq!(state.value, 12);

        let state = node3.execute(state).await.unwrap(); // 12 - 10 = 2
        assert_eq!(state.value, 2);
    }

    #[tokio::test]
    async fn test_function_node_large_state() {
        // Test FunctionNode with large state modification
        #[derive(Clone)]
        struct LargeState {
            data: Vec<u8>,
            checksum: u32,
        }

        let node = FunctionNode::new("checksum_calculator", |state: LargeState| {
            Box::pin(async move {
                let checksum: u32 = state.data.iter().map(|&x| x as u32).sum();
                Ok(LargeState {
                    data: state.data,
                    checksum,
                })
            })
        });

        let large_data = vec![42u8; 10000]; // 10KB of data
        let state = LargeState {
            data: large_data,
            checksum: 0,
        };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.checksum, 42 * 10000);
        assert_eq!(result.data.len(), 10000);
    }

    #[tokio::test]
    async fn test_function_node_async_simulation() {
        // Test FunctionNode with simulated async I/O
        use tokio::time::{sleep, Duration};

        let node = FunctionNode::new("async_processor", |state: TestState| {
            Box::pin(async move {
                // Simulate async I/O delay
                sleep(Duration::from_millis(10)).await;
                Ok(TestState {
                    value: state.value + 100,
                })
            })
        });

        let state = TestState { value: 5 };
        let start = std::time::Instant::now();
        let result = node.execute(state).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result.value, 105);
        assert!(elapsed >= Duration::from_millis(10));
    }

    struct MultiplyNode {
        factor: i32,
    }

    #[async_trait]
    impl Node<TestState> for MultiplyNode {
        async fn execute(&self, state: TestState) -> Result<TestState> {
            Ok(TestState {
                value: state.value * self.factor,
            })
        }

        fn name(&self) -> String {
            format!("MultiplyBy{}", self.factor)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_custom_node_implementation() {
        // Test custom node implementation with state
        let node = MultiplyNode { factor: 5 };
        let state = TestState { value: 8 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 40);
        assert_eq!(node.name(), "MultiplyBy5");
    }

    #[tokio::test]
    async fn test_node_execution_order() {
        // Test that nodes execute in the correct order and maintain state
        let nodes: Vec<BoxedNode<TestState>> = vec![
            Arc::new(IncrementNode),
            Arc::new(IncrementNode),
            Arc::new(MultiplyNode { factor: 2 }),
            Arc::new(IncrementNode),
        ];

        let mut state = TestState { value: 0 };
        for node in nodes {
            state = node.execute(state).await.unwrap();
        }

        // 0 -> 1 -> 2 -> 4 -> 5
        assert_eq!(state.value, 5);
    }

    #[test]
    fn test_function_node_empty_name() {
        // Test FunctionNode with empty string name
        let node = FunctionNode::new("", |state: TestState| Box::pin(async move { Ok(state) }));
        assert_eq!(node.name(), "");
    }

    #[tokio::test]
    async fn test_function_node_unicode_name() {
        // Test FunctionNode with Unicode name
        let node = FunctionNode::new("节点处理器", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });
        assert_eq!(node.name(), "节点处理器");

        let state = TestState { value: 10 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 11);
    }

    #[tokio::test]
    async fn test_node_with_different_state_types() {
        // Test nodes with different state types
        #[derive(Clone)]
        struct StringState {
            text: String,
        }

        let node = FunctionNode::new("uppercase", |state: StringState| {
            Box::pin(async move {
                Ok(StringState {
                    text: state.text.to_uppercase(),
                })
            })
        });

        let state = StringState {
            text: "hello world".to_string(),
        };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.text, "HELLO WORLD");
    }

    #[tokio::test]
    async fn test_function_node_error_types() {
        // Test different error types from FunctionNode
        use crate::error::Error;

        let node1 = FunctionNode::new("validation_error", |_state: TestState| {
            Box::pin(async move { Err(Error::Validation("Validation failed".to_string())) })
        });

        let state = TestState { value: 1 };
        let result = node1.execute(state).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Validation(msg) => {
                assert_eq!(msg, "Validation failed");
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_boxed_node_clone() {
        // Test that BoxedNode can be cloned (Arc clones)
        let node1: BoxedNode<TestState> = Arc::new(IncrementNode);
        let node2 = Arc::clone(&node1);

        // Both should work identically
        let state1 = TestState { value: 5 };
        let result1 = node1.execute(state1).await.unwrap();
        assert_eq!(result1.value, 6);

        let state2 = TestState { value: 10 };
        let result2 = node2.execute(state2).await.unwrap();
        assert_eq!(result2.value, 11);

        // Verify they point to the same node
        assert!(Arc::ptr_eq(&node1, &node2));
    }

    #[tokio::test]
    async fn test_function_node_state_immutability() {
        // Test that FunctionNode doesn't mutate original state (ownership model)
        let node = FunctionNode::new("modifier", |state: TestState| {
            Box::pin(async move {
                // State is moved, so original is consumed
                Ok(TestState {
                    value: state.value + 10,
                })
            })
        });

        let original_state = TestState { value: 5 };
        let result = node.execute(original_state).await.unwrap();

        // Original state is moved/consumed
        // Result is a new state
        assert_eq!(result.value, 15);
    }

    #[tokio::test]
    async fn test_custom_node_with_custom_name() {
        // Test custom node with overridden name() method
        struct NamedNode {
            custom_name: String,
        }

        #[async_trait]
        impl Node<TestState> for NamedNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(TestState {
                    value: state.value * 10,
                })
            }

            fn name(&self) -> String {
                self.custom_name.clone()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = NamedNode {
            custom_name: "MyCustomProcessor".to_string(),
        };

        assert_eq!(node.name(), "MyCustomProcessor");

        let state = TestState { value: 3 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 30);
    }

    #[tokio::test]
    async fn test_function_node_with_zero_value() {
        // Test FunctionNode handling zero values
        let node = FunctionNode::new("zero_handler", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: if state.value == 0 { 1 } else { state.value },
                })
            })
        });

        let state_zero = TestState { value: 0 };
        let result = node.execute(state_zero).await.unwrap();
        assert_eq!(result.value, 1);

        let state_nonzero = TestState { value: 5 };
        let result = node.execute(state_nonzero).await.unwrap();
        assert_eq!(result.value, 5);
    }

    #[tokio::test]
    async fn test_function_node_with_negative_values() {
        // Test FunctionNode with negative values
        let node = FunctionNode::new("absolute", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value.abs(),
                })
            })
        });

        let state = TestState { value: -42 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 42);
    }

    #[tokio::test]
    async fn test_multiple_function_nodes_same_logic() {
        // Test multiple FunctionNode instances with same logic
        let node1 = FunctionNode::new("add_one_a", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        let node2 = FunctionNode::new("add_one_b", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        let state = TestState { value: 10 };
        let result1 = node1.execute(state.clone()).await.unwrap();
        let result2 = node2.execute(state).await.unwrap();

        assert_eq!(result1.value, 11);
        assert_eq!(result2.value, 11);
        assert_eq!(node1.name(), "add_one_a");
        assert_eq!(node2.name(), "add_one_b");
    }

    #[tokio::test]
    async fn test_boxed_node_heterogeneous_collection() {
        // Test collection of different node types via BoxedNode
        let nodes: Vec<BoxedNode<TestState>> = vec![
            Arc::new(IncrementNode),
            Arc::new(FunctionNode::new("double", |state: TestState| {
                Box::pin(async move {
                    Ok(TestState {
                        value: state.value * 2,
                    })
                })
            })),
            Arc::new(MultiplyNode { factor: 3 }),
        ];

        let mut state = TestState { value: 2 };
        for node in nodes {
            state = node.execute(state).await.unwrap();
        }

        // 2 -> 3 -> 6 -> 18
        assert_eq!(state.value, 18);
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_function_node_extremely_long_name() {
        // Test FunctionNode with extremely long name (10K chars)
        let long_name = "a".repeat(10000);
        let node = FunctionNode::new(long_name.clone(), |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node.name(), long_name);
        assert_eq!(node.name().len(), 10000);
    }

    #[test]
    fn test_function_node_additional_unicode() {
        // Test FunctionNode with additional Unicode edge cases (emoji, RTL)
        let node1 = FunctionNode::new("نود", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node1.name(), "نود");

        let node2 = FunctionNode::new("🚀🎯🔥", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node2.name(), "🚀🎯🔥");
    }

    #[test]
    fn test_function_node_whitespace_variants() {
        // Test FunctionNode with various whitespace patterns
        let node1 = FunctionNode::new("   ", |state: TestState| Box::pin(async move { Ok(state) }));
        assert_eq!(node1.name(), "   ");

        let node2 = FunctionNode::new("\t\t", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node2.name(), "\t\t");

        let node3 = FunctionNode::new("\n\n", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node3.name(), "\n\n");
    }

    #[test]
    fn test_function_node_name_with_null_bytes() {
        // Test FunctionNode with null bytes in name
        let node = FunctionNode::new("node\0name", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        assert_eq!(node.name(), "node\0name");
    }

    #[tokio::test]
    async fn test_node_with_zero_value() {
        // Test node execution with zero value
        let node = IncrementNode;
        let state = TestState { value: 0 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 1);
    }

    #[tokio::test]
    async fn test_node_with_negative_value() {
        // Test node execution with negative value
        let node = IncrementNode;
        let state = TestState { value: -100 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, -99);
    }

    #[tokio::test]
    async fn test_node_with_max_value() {
        // Test node execution with maximum i32 value
        let node = FunctionNode::new("identity", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        let state = TestState { value: i32::MAX };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, i32::MAX);
    }

    #[tokio::test]
    async fn test_node_with_min_value() {
        // Test node execution with minimum i32 value
        let node = FunctionNode::new("identity", |state: TestState| {
            Box::pin(async move { Ok(state) })
        });
        let state = TestState { value: i32::MIN };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, i32::MIN);
    }

    #[tokio::test]
    async fn test_concurrent_node_execution() {
        // Test concurrent execution of same node
        use tokio::task::JoinSet;

        let node = Arc::new(IncrementNode);
        let mut join_set = JoinSet::new();

        for i in 0..100 {
            let node_clone = Arc::clone(&node);
            join_set.spawn(async move {
                let state = TestState { value: i };
                node_clone.execute(state).await.unwrap()
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap().value);
        }

        assert_eq!(results.len(), 100);
        // Check all values are in expected range (1 to 100)
        for value in results {
            assert!((1..=100).contains(&value));
        }
    }

    #[tokio::test]
    async fn test_concurrent_function_node_execution() {
        // Test concurrent execution of function nodes
        use tokio::task::JoinSet;

        let node = Arc::new(FunctionNode::new("double", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value * 2,
                })
            })
        }));

        let mut join_set = JoinSet::new();

        for i in 1..=50 {
            let node_clone = Arc::clone(&node);
            join_set.spawn(async move {
                let state = TestState { value: i };
                node_clone.execute(state).await.unwrap()
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap().value);
        }

        assert_eq!(results.len(), 50);
        // Check all values are doubled
        results.sort();
        for &value in results.iter() {
            assert!((2..=100).contains(&value));
        }
    }

    #[tokio::test]
    async fn test_node_state_with_complex_nesting() {
        // Test node with deeply nested state structure
        #[derive(Clone)]
        struct NestedState {
            level1: Vec<Vec<Vec<i32>>>,
            metadata: Option<String>,
        }

        let node = FunctionNode::new("nested_processor", |mut state: NestedState| {
            Box::pin(async move {
                // Flatten and sum all values
                let sum: i32 = state
                    .level1
                    .iter()
                    .flat_map(|l2| l2.iter())
                    .flat_map(|l3| l3.iter())
                    .sum();
                state.metadata = Some(format!("sum={}", sum));
                Ok(state)
            })
        });

        let state = NestedState {
            level1: vec![
                vec![vec![1, 2, 3], vec![4, 5]],
                vec![vec![6, 7, 8, 9], vec![10]],
            ],
            metadata: None,
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.metadata, Some("sum=55".to_string()));
    }

    #[tokio::test]
    async fn test_node_with_empty_collections() {
        // Test node with empty collection state
        #[derive(Clone)]
        struct CollectionState {
            numbers: Vec<i32>,
            names: Vec<String>,
        }

        let node = FunctionNode::new("collection_processor", |state: CollectionState| {
            Box::pin(async move {
                Ok(CollectionState {
                    numbers: state.numbers,
                    names: state.names,
                })
            })
        });

        let state = CollectionState {
            numbers: vec![],
            names: vec![],
        };

        let result = node.execute(state).await.unwrap();
        assert!(result.numbers.is_empty());
        assert!(result.names.is_empty());
    }

    #[tokio::test]
    async fn test_node_memory_efficiency() {
        // Test that node doesn't unnecessarily clone large data
        #[derive(Clone)]
        struct LargeState {
            large_vec: Vec<u64>,
            counter: u64,
        }

        let node = FunctionNode::new("counter_increment", |mut state: LargeState| {
            Box::pin(async move {
                state.counter += 1;
                // Don't modify large_vec, should be efficient
                Ok(state)
            })
        });

        let large_data = vec![42u64; 1_000_000]; // 8MB of data
        let state = LargeState {
            large_vec: large_data,
            counter: 0,
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.counter, 1);
        assert_eq!(result.large_vec.len(), 1_000_000);
        assert_eq!(result.large_vec[0], 42);
    }

    #[tokio::test]
    async fn test_function_node_with_panic_safety() {
        // Test that errors are properly propagated, not panics
        use crate::error::Error;

        let node = FunctionNode::new("panic_safe", |_state: TestState| {
            Box::pin(async move {
                Err(Error::Generic(
                    "Expected error for panic safety test".to_string(),
                ))
            })
        });

        let state = TestState { value: 1 };
        let result = node.execute(state).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Generic(msg) => {
                assert_eq!(msg, "Expected error for panic safety test");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[tokio::test]
    async fn test_boxed_node_clone_semantics() {
        // Test that BoxedNode clones correctly share Arc
        let node: BoxedNode<TestState> = Arc::new(IncrementNode);
        let node_clone = Arc::clone(&node);

        let state1 = TestState { value: 5 };
        let result1 = node.execute(state1).await.unwrap();

        let state2 = TestState { value: 10 };
        let result2 = node_clone.execute(state2).await.unwrap();

        assert_eq!(result1.value, 6);
        assert_eq!(result2.value, 11);
        assert_eq!(Arc::strong_count(&node), 2);
    }

    #[test]
    fn test_function_node_send_sync_bounds() {
        // Test that FunctionNode is Send + Sync
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        type TestFn = fn(
            TestState,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<TestState>> + Send>,
        >;

        assert_send::<FunctionNode<TestState, TestFn>>();
        assert_sync::<FunctionNode<TestState, TestFn>>();
    }

    #[tokio::test]
    async fn test_node_execution_ordering() {
        // Test that node execution preserves ordering
        use std::sync::Mutex;

        let execution_order = Arc::new(Mutex::new(Vec::new()));

        let node1 = {
            let order = Arc::clone(&execution_order);
            FunctionNode::new("first", move |state: TestState| {
                let order = Arc::clone(&order);
                Box::pin(async move {
                    order.lock().unwrap().push(1);
                    Ok(state)
                })
            })
        };

        let node2 = {
            let order = Arc::clone(&execution_order);
            FunctionNode::new("second", move |state: TestState| {
                let order = Arc::clone(&order);
                Box::pin(async move {
                    order.lock().unwrap().push(2);
                    Ok(state)
                })
            })
        };

        let node3 = {
            let order = Arc::clone(&execution_order);
            FunctionNode::new("third", move |state: TestState| {
                let order = Arc::clone(&order);
                Box::pin(async move {
                    order.lock().unwrap().push(3);
                    Ok(state)
                })
            })
        };

        let state = TestState { value: 0 };
        let state = node1.execute(state).await.unwrap();
        let state = node2.execute(state).await.unwrap();
        let _state = node3.execute(state).await.unwrap();

        let order = execution_order.lock().unwrap();
        assert_eq!(*order, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_node_with_optional_fields() {
        // Test node with optional state fields
        #[derive(Clone)]
        struct OptionalState {
            required: i32,
            optional: Option<String>,
            nested_optional: Option<Option<Vec<i32>>>,
        }

        let node = FunctionNode::new("optional_processor", |mut state: OptionalState| {
            Box::pin(async move {
                state.optional = Some("processed".to_string());
                state.nested_optional = Some(Some(vec![1, 2, 3]));
                Ok(state)
            })
        });

        let state = OptionalState {
            required: 42,
            optional: None,
            nested_optional: None,
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.required, 42);
        assert_eq!(result.optional, Some("processed".to_string()));
        assert_eq!(result.nested_optional, Some(Some(vec![1, 2, 3])));
    }

    #[test]
    fn test_node_trait_object_safety() {
        // Test that Node trait is object-safe
        let node: Box<dyn Node<TestState>> = Box::new(IncrementNode);
        let name = node.name();
        assert!(name.contains("IncrementNode") || name == "IncrementNode");
    }

    #[tokio::test]
    async fn test_function_node_with_closure_capture() {
        // Test FunctionNode with closure that captures variables
        let multiplier = 5;
        let offset = 10;

        let node = FunctionNode::new("captured", move |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value * multiplier + offset,
                })
            })
        });

        let state = TestState { value: 3 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 25); // 3 * 5 + 10
    }

    #[tokio::test]
    async fn test_node_with_borrowed_data_patterns() {
        // Test node execution patterns that involve string operations
        #[derive(Clone)]
        struct StringState {
            text: String,
            processed: bool,
        }

        let node = FunctionNode::new("string_processor", |mut state: StringState| {
            Box::pin(async move {
                state.text = state.text.to_uppercase();
                state.processed = true;
                Ok(state)
            })
        });

        let state = StringState {
            text: "hello world".to_string(),
            processed: false,
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.text, "HELLO WORLD");
        assert!(result.processed);
    }

    #[tokio::test]
    async fn test_node_execution_with_await_points() {
        // Test node with multiple await points
        use tokio::time::{sleep, Duration};

        let node = FunctionNode::new("multi_await", |mut state: TestState| {
            Box::pin(async move {
                sleep(Duration::from_millis(1)).await;
                state.value += 1;
                sleep(Duration::from_millis(1)).await;
                state.value *= 2;
                sleep(Duration::from_millis(1)).await;
                state.value -= 3;
                Ok(state)
            })
        });

        let state = TestState { value: 10 };
        let result = node.execute(state).await.unwrap();
        // (10 + 1) * 2 - 3 = 19
        assert_eq!(result.value, 19);
    }

    #[test]
    fn test_default_node_name_full_path() {
        // Test that default name() extracts last component of full type path
        struct MyCustomNode;

        #[async_trait]
        impl Node<TestState> for MyCustomNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = MyCustomNode;
        let name = node.name();
        // Should contain "MyCustomNode" or be exactly "MyCustomNode"
        assert!(
            name.contains("MyCustomNode") || name == "MyCustomNode",
            "Expected name to contain 'MyCustomNode', got: {}",
            name
        );
    }

    #[tokio::test]
    async fn test_node_with_result_chaining() {
        // Test chaining multiple node results
        let node1 = FunctionNode::new("add_5", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 5,
                })
            })
        });

        let node2 = FunctionNode::new("multiply_3", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value * 3,
                })
            })
        });

        let node3 = FunctionNode::new("subtract_7", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value - 7,
                })
            })
        });

        let state1 = node1.execute(TestState { value: 2 }).await.unwrap();
        let state2 = node2.execute(state1).await.unwrap();
        let result = node3.execute(state2).await.unwrap();

        // (2 + 5) * 3 - 7 = 14
        assert_eq!(result.value, 14);
    }

    #[tokio::test]
    async fn test_boxed_nodes_heterogeneous_collection() {
        // Test collection of different node types as BoxedNode
        struct CustomNode {
            increment: i32,
        }

        #[async_trait]
        impl Node<TestState> for CustomNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(TestState {
                    value: state.value + self.increment,
                })
            }

            fn name(&self) -> String {
                format!("CustomNode(+{})", self.increment)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let nodes: Vec<BoxedNode<TestState>> = vec![
            Arc::new(CustomNode { increment: 10 }),
            Arc::new(FunctionNode::new("halve", |state: TestState| {
                Box::pin(async move {
                    Ok(TestState {
                        value: state.value / 2,
                    })
                })
            })),
            Arc::new(CustomNode { increment: -3 }),
        ];

        let mut state = TestState { value: 4 };
        for node in nodes {
            state = node.execute(state).await.unwrap();
        }

        // 4 + 10 = 14, 14 / 2 = 7, 7 - 3 = 4
        assert_eq!(state.value, 4);
    }

    // ========================================
    // NodeContext Tests (dashstream feature)
    // ========================================

    #[cfg(feature = "dashstream")]
    #[test]
    fn test_node_context_empty() {
        let ctx = NodeContext::empty();
        assert!(ctx.producer.is_none());
        assert_eq!(ctx.node_name, "");
        assert_eq!(ctx.thread_id, "");
        assert_eq!(ctx.tenant_id, "");
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_progress_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_progress("test", 0.5).await;
        assert!(result.is_ok()); // No-op succeeds
    }

    #[cfg(feature = "dashstream")]
    #[test]
    fn test_node_context_new() {
        let ctx = NodeContext::new(
            "test_node".to_string(),
            None,
            "thread123".to_string(),
            "tenant456".to_string(),
        );
        assert!(ctx.producer.is_none());
        assert_eq!(ctx.node_name, "test_node");
        assert_eq!(ctx.thread_id, "thread123");
        assert_eq!(ctx.tenant_id, "tenant456");
    }

    #[cfg(feature = "dashstream")]
    #[test]
    fn test_node_context_sequence_increment() {
        let ctx = NodeContext::new(
            "test".to_string(),
            None,
            "thread".to_string(),
            "tenant".to_string(),
        );

        // Create multiple headers and verify sequence numbers increment
        let h1 = ctx.create_header(dashflow_streaming::MessageType::Event);
        let h2 = ctx.create_header(dashflow_streaming::MessageType::Event);
        let h3 = ctx.create_header(dashflow_streaming::MessageType::Event);

        assert_eq!(h1.sequence, 1);
        assert_eq!(h2.sequence, 2);
        assert_eq!(h3.sequence, 3);
    }

    #[cfg(feature = "dashstream")]
    #[test]
    fn test_node_context_message_ids_unique() {
        let ctx = NodeContext::new(
            "test".to_string(),
            None,
            "thread".to_string(),
            "tenant".to_string(),
        );

        let h1 = ctx.create_header(dashflow_streaming::MessageType::Event);
        let h2 = ctx.create_header(dashflow_streaming::MessageType::Event);

        assert_ne!(h1.message_id, h2.message_id);
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_thinking_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_thinking("analyzing data", 1).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_substep_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_substep("validation", "complete").await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_token_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_token("test", 0, false, "req123").await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_tool_event_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_tool_event("call1", "search", 1, 1000).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_metric_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_metric("latency", 42.5, "ms").await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_context_send_error_no_producer() {
        let ctx = NodeContext::empty();
        let result = ctx.send_error("E001", "test error", 3).await;
        assert!(result.is_ok());
    }

    // ========================================
    // Node Trait Tests (execute_with_context)
    // ========================================

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_default_execute_with_context_calls_execute() {
        // Verify default impl calls execute()
        struct TestNode;

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(TestState {
                    value: state.value + 1,
                })
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = TestNode;
        let state = TestState { value: 5 };
        let ctx = NodeContext::empty();

        let result = node.execute_with_context(state, &ctx).await.unwrap();
        assert_eq!(result.value, 6);
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_supports_streaming_default_false() {
        struct TestNode;

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = TestNode;
        assert!(!node.supports_streaming());
    }

    #[cfg(feature = "dashstream")]
    #[tokio::test]
    async fn test_node_custom_execute_with_context() {
        // Test node that overrides execute_with_context
        struct StreamingNode;

        #[async_trait]
        impl Node<TestState> for StreamingNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            async fn execute_with_context(
                &self,
                state: TestState,
                ctx: &NodeContext,
            ) -> Result<TestState>
            where
                TestState: 'static,
            {
                // Send progress messages
                ctx.send_progress("Starting", 0.0).await?;
                ctx.send_progress("Complete", 1.0).await?;
                Ok(TestState {
                    value: state.value + 10,
                })
            }

            fn supports_streaming(&self) -> bool {
                true
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = StreamingNode;
        let state = TestState { value: 5 };
        let ctx = NodeContext::empty();

        let result = node.execute_with_context(state, &ctx).await.unwrap();
        assert_eq!(result.value, 15);
        assert!(node.supports_streaming());
    }

    #[tokio::test]
    async fn test_is_read_only_default_false() {
        // Test that is_read_only() defaults to false for safety
        struct MutatingNode;

        #[async_trait]
        impl Node<TestState> for MutatingNode {
            async fn execute(&self, mut state: TestState) -> Result<TestState> {
                state.value += 1;
                Ok(state)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = MutatingNode;
        // Default should be false (conservative - assumes mutation)
        assert!(!node.is_read_only());
    }

    #[tokio::test]
    async fn test_is_read_only_can_be_overridden() {
        // Test that is_read_only() can be overridden for read-only nodes
        struct ReadOnlyNode;

        #[async_trait]
        impl Node<TestState> for ReadOnlyNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                // This node doesn't mutate state, just returns it
                Ok(state)
            }

            fn is_read_only(&self) -> bool {
                true // Override to indicate this node is read-only
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let node = ReadOnlyNode;
        assert!(node.is_read_only());

        // Execute should still work correctly
        let state = TestState { value: 42 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 42);
    }
}
