// Allow clippy warnings for runnable core
// - expect_used: Runnable configuration uses expect() for required fields
// - clone_on_ref_ptr: Runnables clone Arc for shared callbacks/config
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Core Runnable trait and implementations
//!
//! The Runnable trait is the foundation of DashFlow's composable architecture.
//! It provides a unified interface for units of work that can be invoked, batched,
//! streamed, and composed together.

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

// Note: backoff constants moved to retry.rs
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};

// Submodules
mod batch;
mod branch;
mod graph;
mod history;
mod lambda;
mod parallel;
mod passthrough;
mod retry;
mod router;
mod sequence;
mod stream_events;

// Re-exports from submodules
pub use batch::{RunnableEach, RunnableGenerator};
pub use branch::RunnableBranch;
pub use graph::{Edge, Graph, Node};
pub use history::{GetSessionHistoryFn, RunnableWithMessageHistory};
pub use lambda::RunnableLambda;
pub use parallel::{RunnableAssign, RunnableParallel};
pub use passthrough::{RunnablePassthrough, RunnablePick};
pub use retry::{RunnableRetry, RunnableWithFallbacks};
pub use router::{RouterInput, RouterRunnable};
pub use sequence::RunnableSequence;
pub use stream_events::{
    StreamEvent, StreamEventData, StreamEventType, StreamEventsOptions, StreamEventsVersion,
};

/// A unit of work that can be invoked, batched, streamed, and composed.
///
/// # Key Methods
///
/// - **`invoke`/`ainvoke`**: Transform a single input into an output
/// - **`batch`/`abatch`**: Efficiently transform multiple inputs into outputs
/// - **`stream`/`astream`**: Stream output as it's produced
///
/// # Composition
///
/// Runnables can be composed together using:
/// - `pipe()`: Sequential composition (`RunnableSequence`)
/// - `RunnableParallel`: Concurrent execution
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::Runnable;
///
/// // Compose runnables
/// let chain = runnable1.pipe(runnable2);
///
/// // Invoke synchronously (runs async internally)
/// let result = chain.invoke(input, None).await?;
///
/// // Batch process
/// let results = chain.batch(vec![input1, input2], None).await?;
///
/// // Stream results
/// let mut stream = chain.stream(input, None).await?;
/// while let Some(chunk) = stream.next().await {
///     // Process chunk
/// }
/// ```
#[async_trait]
pub trait Runnable: Send + Sync {
    /// Input type for this Runnable
    type Input: Send;

    /// Output type for this Runnable
    type Output: Send;

    /// Get the name of this Runnable (for debugging and tracing)
    fn name(&self) -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("Runnable")
            .to_string()
    }

    /// Transform a single input into an output
    ///
    /// # Arguments
    ///
    /// * `input` - The input to process
    /// * `config` - Optional configuration for execution
    ///
    /// # Returns
    ///
    /// The output result, or an error if execution fails
    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output>;

    /// Efficiently transform multiple inputs into outputs
    ///
    /// By default, this runs `invoke()` concurrently using tokio tasks.
    /// Override this method to provide custom batching logic.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Vector of inputs to process
    /// * `config` - Optional configuration for execution
    ///
    /// # Returns
    ///
    /// Vector of outputs. Order matches input order when `max_concurrency` is `None`;
    /// when `max_concurrency` is set, results are in completion order (not input order).
    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>>
    where
        Self: Sized,
        Self::Input: Clone,
    {
        // Extract max_concurrency from config if set
        let max_concurrency = config.as_ref().and_then(|c| c.max_concurrency);

        let tasks: Vec<_> = inputs
            .into_iter()
            .map(|input| {
                let config = config.clone();
                async move { self.invoke(input, config).await }
            })
            .collect();

        // If max_concurrency is set, use bounded concurrency; otherwise run all at once
        let results = if let Some(limit) = max_concurrency {
            // Use buffer_unordered to limit concurrent execution
            // Note: buffer_unordered returns results in completion order, NOT input order.
            // For strict ordering, use buffer() instead (but that's sequential).
            // join_all below DOES preserve order (returns Vec with same index mapping).
            futures::stream::iter(tasks)
                .buffer_unordered(limit.max(1)) // At least 1 concurrent task
                .collect::<Vec<_>>()
                .await
        } else {
            // No limit - run all tasks concurrently, preserves input order
            futures::future::join_all(tasks).await
        };

        // Collect results, returning early if any failed
        results.into_iter().collect()
    }

    /// Stream output from a single input as it's produced
    ///
    /// # Arguments
    ///
    /// * `input` - The input to process
    /// * `config` - Optional configuration for execution
    ///
    /// # Returns
    ///
    /// A stream of output chunks
    ///
    /// # Default Implementation
    ///
    /// By default, this invokes the Runnable once and yields a single result.
    /// Override this method to provide true streaming behavior.
    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>>
    where
        Self::Output: Clone + 'static,
    {
        // Default implementation: invoke once and yield the result
        let result = self.invoke(input, config).await;
        Ok(Box::pin(async_stream::stream! {
            yield result;
        }))
    }

    /// Compose this Runnable with another in sequence
    ///
    /// Creates a `RunnableSequence` where the output of this Runnable
    /// becomes the input to the next.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = runnable1.pipe(runnable2).pipe(runnable3);
    /// ```
    fn pipe<R>(self, next: R) -> RunnableSequence<Self, R>
    where
        Self: Sized,
        R: Runnable<Input = Self::Output>,
    {
        RunnableSequence::new(self, next)
    }

    /// Bind lifecycle listeners to this Runnable
    ///
    /// Returns a new Runnable with the listeners attached. Listeners are called
    /// on the root run only (child runs are ignored).
    ///
    /// # Arguments
    ///
    /// * `on_start` - Called when the Runnable starts executing
    /// * `on_end` - Called when the Runnable finishes successfully
    /// * `on_error` - Called when the Runnable encounters an error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::runnable::Runnable;
    /// use dashflow::core::tracers::{RunTree, AsyncListener};
    /// use std::sync::Arc;
    ///
    /// let on_start: AsyncListener = Arc::new(|run, _config| {
    ///     Box::pin(async move {
    ///         println!("Started run: {:?}", run.id);
    ///     })
    /// });
    ///
    /// let on_end: AsyncListener = Arc::new(|run, _config| {
    ///     Box::pin(async move {
    ///         println!("Completed run: {:?}", run.id);
    ///     })
    /// });
    ///
    /// let chain = runnable.with_listeners(Some(on_start), Some(on_end), None);
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `with_listeners()` in `dashflow_core/runnables/base.py:1640-1707`.
    fn with_listeners(
        self,
        on_start: Option<crate::core::tracers::AsyncListener>,
        on_end: Option<crate::core::tracers::AsyncListener>,
        on_error: Option<crate::core::tracers::AsyncListener>,
    ) -> RunnableBindingBase<Self, Self::Input, Self::Output>
    where
        Self: Sized,
    {
        use crate::core::callbacks::CallbackManager;
        use crate::core::tracers::RootListenersTracer;
        use std::sync::Arc;

        // Create a config factory that adds the RootListenersTracer to callbacks
        let config_factory = Arc::new(move |mut config: RunnableConfig| {
            let tracer = RootListenersTracer::new(
                config.clone(),
                on_start.clone(),
                on_end.clone(),
                on_error.clone(),
            );

            // Add tracer to callbacks
            let callbacks = config.callbacks.get_or_insert_with(CallbackManager::new);
            callbacks.add_handler(Arc::new(tracer));

            config
        });

        RunnableBindingBase::simple(self).with_config_factory(config_factory)
    }

    /// Add retry logic to this Runnable
    ///
    /// Returns a new Runnable that will automatically retry failed invocations
    /// according to the specified retry policy. This is useful for handling
    /// transient errors like network failures, rate limits, and temporary
    /// service unavailability.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts (default: 3)
    /// * `wait_exponential_jitter` - Whether to use exponential backoff with jitter (default: true)
    /// * `initial_delay_ms` - Initial retry delay in milliseconds (default: 1000)
    /// * `max_delay_ms` - Maximum retry delay in milliseconds (default: 10000)
    /// * `exp_base` - Exponential base for jitter calculation (default: 2.0)
    /// * `jitter_ms` - Maximum random jitter to add in milliseconds (default: 1000)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::runnable::Runnable;
    ///
    /// // Use default retry settings (3 retries with jitter)
    /// let retryable = model.with_retry(None, None, None, None, None, None);
    ///
    /// // Custom retry settings
    /// let retryable = model.with_retry(Some(5), Some(true), Some(500), Some(5000), Some(2.0), Some(500));
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `with_retry()` in `dashflow_core/runnables/base.py:1825-1890`.
    fn with_retry(
        self,
        max_retries: Option<usize>,
        wait_exponential_jitter: Option<bool>,
        initial_delay_ms: Option<u64>,
        max_delay_ms: Option<u64>,
        exp_base: Option<f64>,
        jitter_ms: Option<u64>,
    ) -> crate::core::retry::RunnableRetry<Self>
    where
        Self: Sized,
        Self::Input: Clone + Sync,
        Self::Output: Sync,
    {
        use crate::core::retry::{RetryPolicy, RetryStrategy, RunnableRetry};

        // Defaults match Python baseline
        let max_retries = max_retries.unwrap_or(3);
        let use_jitter = wait_exponential_jitter.unwrap_or(true);
        let initial = initial_delay_ms.unwrap_or(1000);
        let max = max_delay_ms.unwrap_or(10000);
        let base = exp_base.unwrap_or(2.0);
        let jitter = jitter_ms.unwrap_or(1000);

        let policy = if use_jitter {
            RetryPolicy {
                max_retries,
                strategy: RetryStrategy::ExponentialJitter {
                    initial_delay_ms: initial,
                    max_delay_ms: max,
                    exp_base: base,
                    jitter_ms: jitter,
                },
                rate_limiter: None,
            }
        } else {
            RetryPolicy {
                max_retries,
                strategy: RetryStrategy::Exponential {
                    initial_delay_ms: initial,
                    max_delay_ms: max,
                    multiplier: 2,
                },
                rate_limiter: None,
            }
        };

        RunnableRetry::new(self, policy)
    }

    /// Add fallback Runnables to handle failures
    ///
    /// Returns a new Runnable that will try fallbacks in order if the primary
    /// Runnable fails. This is useful for handling service degradation by
    /// falling back to alternative LLM providers or hardcoded responses.
    ///
    /// # Arguments
    ///
    /// * `fallbacks` - Vector of Runnables to try in order if the primary fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::runnable::Runnable;
    /// use dashflow_openai::ChatOpenAI;
    /// use dashflow_anthropic::ChatAnthropic;
    ///
    /// // Try OpenAI first, fallback to Anthropic if it fails
    /// let model = ChatOpenAI::with_config(Default::default())
    ///     .with_fallbacks(vec![
    ///         ChatAnthropic::new().into_boxed()
    ///     ]);
    ///
    /// // Will use ChatOpenAI normally, but switch to ChatAnthropic
    /// // if ChatOpenAI fails
    /// let response = model.invoke("Hello", None).await?;
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `with_fallbacks()` in `dashflow_core/runnables/base.py:1805-1823`.
    fn with_fallbacks(
        self,
        fallbacks: Vec<Box<dyn Runnable<Input = Self::Input, Output = Self::Output>>>,
    ) -> RunnableWithFallbacks<Self::Input, Self::Output>
    where
        Self: Sized + 'static,
        Self::Input: Clone + Sync,
        Self::Output: Sync,
    {
        let mut with_fallbacks = RunnableWithFallbacks::new(self);
        for fallback in fallbacks {
            with_fallbacks = with_fallbacks.add_fallback_boxed(fallback);
        }
        with_fallbacks
    }

    /// Get the graph representation of this Runnable
    ///
    /// Returns a Graph that visualizes the structure and flow of this Runnable.
    /// Useful for debugging complex chains and understanding data flow.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::runnable::Runnable;
    ///
    /// let chain = runnable1.pipe(runnable2);
    /// let graph = chain.get_graph();
    ///
    /// // Print ASCII visualization
    /// println!("{}", graph.draw_ascii());
    /// ```
    fn get_graph(&self, _config: Option<&RunnableConfig>) -> Graph {
        // Default implementation: single node with the runnable's name
        let mut graph = Graph::new();
        let node = Node::new(self.name(), self.name());
        graph.add_node(node);
        graph
    }

    /// Stream execution events in real-time
    ///
    /// Provides comprehensive observability by emitting events as the Runnable executes.
    /// Events include start/stream/end for all components in the execution chain.
    ///
    /// This matches Python `DashFlow`'s `astream_events()` API, enabling real-time
    /// monitoring, debugging, and observability for complex chains and agents.
    ///
    /// # Event Types
    ///
    /// Events follow the pattern `on_{type}_{stage}`:
    /// - `on_chain_start`, `on_chain_stream`, `on_chain_end`
    /// - `on_chat_model_start`, `on_chat_model_stream`, `on_chat_model_end`
    /// - `on_llm_start`, `on_llm_stream`, `on_llm_end`
    /// - `on_tool_start`, `on_tool_stream`, `on_tool_end`
    /// - `on_prompt_start`, `on_prompt_end`
    /// - `on_retriever_start`, `on_retriever_end`
    ///
    /// # Arguments
    ///
    /// * `input` - The input to process
    /// * `config` - Optional configuration (tags, metadata, etc.)
    ///
    /// # Returns
    ///
    /// A stream of `StreamEvent` objects containing execution details
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::runnable::{Runnable, StreamEventType};
    /// use futures::StreamExt;
    ///
    /// let mut stream = chain.stream_events(input, None, None).await?;
    /// while let Some(event) = stream.next().await {
    ///     match event.event_type {
    ///         StreamEventType::ChainStart => {
    ///             println!("Started: {} (run_id: {})", event.name, event.run_id);
    ///         }
    ///         StreamEventType::ChainStream => {
    ///             println!("Streaming chunk: {:?}", event.data);
    ///         }
    ///         StreamEventType::ChainEnd => {
    ///             println!("Completed: {}", event.name);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `astream_events()` in `dashflow_core/runnables/base.py:1140-1280`.
    ///
    /// # Default Implementation
    ///
    /// The default implementation emits basic start/end events around `invoke()`.
    /// Override this method to provide detailed streaming events for your component.
    async fn stream_events(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
        options: Option<StreamEventsOptions>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + 'static>>>
    where
        Self: Sized + Clone + 'static,
        Self::Input: Clone + Send + serde::Serialize + 'static,
        Self::Output: Clone + Send + serde::Serialize + 'static,
    {
        // Default implementation: emit start/end events around invoke
        let run_id = uuid::Uuid::new_v4();
        let name = self.name();
        let options = options.unwrap_or_default();

        // Extract tags and metadata from config
        let (tags, metadata) = if let Some(ref cfg) = config {
            (cfg.tags.clone(), cfg.metadata.clone())
        } else {
            (Vec::new(), HashMap::new())
        };

        // Clone self and inputs for the stream
        let runnable = self.clone();
        let input_clone = input.clone();
        let config_clone = config.clone();

        let stream = async_stream::stream! {
            // Emit start event
            let start_data = StreamEventData::Input(
                serde_json::to_value(&input_clone).unwrap_or(serde_json::Value::Null)
            );
            let start_event = StreamEvent::new(
                StreamEventType::ChainStart,
                name.clone(),
                run_id,
                start_data,
            )
            .with_tags(tags.clone())
            .with_metadata(metadata.clone());

            // Apply filters
            if options.should_include(&start_event) {
                yield start_event;
            }

            // Execute the runnable
            match runnable.invoke(input_clone, config_clone).await {
                Ok(output) => {
                    // Emit end event with output
                    let end_data = StreamEventData::Output(
                        serde_json::to_value(&output).unwrap_or(serde_json::Value::Null)
                    );
                    let end_event = StreamEvent::new(
                        StreamEventType::ChainEnd,
                        name,
                        run_id,
                        end_data,
                    )
                    .with_tags(tags)
                    .with_metadata(metadata);

                    // Apply filters
                    if options.should_include(&end_event) {
                        yield end_event;
                    }
                }
                Err(e) => {
                    // Emit error event
                    let error_data = StreamEventData::Error(e.to_string());
                    let error_event = StreamEvent::new(
                        StreamEventType::ChainEnd,
                        name,
                        run_id,
                        error_data,
                    )
                    .with_tags(tags)
                    .with_metadata(metadata);

                    // Apply filters
                    if options.should_include(&error_event) {
                        yield error_event;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    /// Convert this Runnable to a Tool that can be called by language models.
    ///
    /// This enables the Python `DashFlow` pattern where any runnable chain can be
    /// converted to a tool via `.as_tool()`. The returned tool can then be bound
    /// to chat models for function calling.
    ///
    /// # Arguments
    ///
    /// * `name` - The tool's unique name
    /// * `description` - Description of what the tool does (helps LLMs decide when to use it)
    /// * `args_schema` - JSON Schema describing the tool's expected parameters
    ///
    /// # Requirements
    ///
    /// The Runnable must:
    /// - Accept `serde_json::Value` as input
    /// - Return `String` as output
    /// - Be `Clone` (to capture in the closure)
    ///
    /// # Returns
    ///
    /// A `RunnableTool` that wraps this runnable and implements the `Tool` trait.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::prompts::ChatPromptTemplate;
    /// use dashflow::core::runnable::Runnable;
    /// use dashflow_openai::ChatOpenAI;
    /// use serde_json::json;
    ///
    /// // Create a runnable chain
    /// let prompt = ChatPromptTemplate::from_messages(vec![
    ///     ("human", "Hello. Please respond in the style of {answer_style}.")
    /// ]);
    /// let llm = ChatOpenAI::with_config(Default::default());
    /// let chain = prompt.pipe(llm);
    ///
    /// // Convert to tool
    /// let tool = chain.as_tool(
    ///     "greeting_generator",
    ///     "Generate a greeting in a particular style of speaking.",
    ///     json!({
    ///         "type": "object",
    ///         "properties": {
    ///             "answer_style": {
    ///                 "type": "string",
    ///                 "description": "The style to respond in (e.g., 'pirate', 'formal', 'casual')"
    ///             }
    ///         },
    ///         "required": ["answer_style"]
    ///     })
    /// );
    ///
    /// // Now use the tool with bind_tools()
    /// let model = ChatOpenAI::with_config(Default::default()).bind_tools(vec![Arc::new(tool)], None);
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `as_tool()` in Python `DashFlow` (`dashflow_core/runnables/base.py`).
    /// This enables patterns like:
    ///
    /// ```python
    /// chain = prompt | llm | parser
    /// tool = chain.as_tool(
    ///     name="my_tool",
    ///     description="Tool description"
    /// )
    /// model_with_tools = model.bind_tools([tool])
    /// ```
    #[allow(clippy::wrong_self_convention)] // Intentional: converts to tool by consuming self
    fn as_tool(
        self,
        name: impl Into<String>,
        description: impl Into<String>,
        args_schema: serde_json::Value,
    ) -> crate::core::tools::RunnableTool
    where
        Self: Sized + Clone + 'static,
        Self::Input: From<serde_json::Value>,
        Self::Output: Into<String>,
    {
        use crate::core::tools::RunnableTool;

        let runnable = self.clone();
        let wrapped_fn = Box::new(move |input: serde_json::Value| {
            let runnable = runnable.clone();
            Box::pin(async move {
                let typed_input = Self::Input::from(input);
                let output = runnable.invoke(typed_input, None).await?;
                Ok(output.into())
            })
                as Pin<
                    Box<
                        dyn std::future::Future<Output = crate::core::error::Result<String>> + Send,
                    >,
                >
        });

        RunnableTool::new(name, description, wrapped_fn, args_schema)
    }
}

// Note: RunnableSequence struct and impls moved to sequence.rs
// Note: BitOr impls for RunnableSequence and RunnableLambda moved to sequence.rs

// Note: RunnablePassthrough struct, Default, and Runnable impls moved to passthrough.rs
// Note: RunnablePassthrough::pick() moved to passthrough.rs
// RunnablePassthrough::assign() kept here because it depends on RunnableParallel

impl RunnablePassthrough<HashMap<String, serde_json::Value>> {
    /// Create a `RunnableAssign` that adds new key-value pairs to input dict
    ///
    /// This merges the input `HashMap` with the output produced by the mapper.
    /// The mapper is a `RunnableParallel` that computes new values based on the input.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use dashflow::core::runnable::{RunnablePassthrough, RunnableParallel, RunnableLambda};
    ///
    /// let mut mapper = RunnableParallel::new();
    /// mapper.add("doubled", RunnableLambda::new(|x: HashMap<String, serde_json::Value>| {
    ///     let val = x.get("input").unwrap().as_i64().unwrap();
    ///     serde_json::json!(val * 2)
    /// }));
    ///
    /// let assign = RunnablePassthrough::assign(mapper);
    /// let input = HashMap::from([("input".to_string(), serde_json::json!(5))]);
    /// let result = assign.invoke(input, None).await?;
    /// // result = {"input": 5, "doubled": 10}
    /// ```
    #[must_use]
    pub fn assign(
        mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value>,
    ) -> RunnableAssign {
        RunnableAssign::new(mapper)
    }
}

// Note: RunnableAssign moved to parallel.rs
// Note: RunnablePick moved to passthrough.rs
// Note: RunnableParallel moved to parallel.rs
// Note: RunnableBranch moved to branch.rs

#[cfg(test)]
mod tests;

// ============================================================================
// ConfigurableFieldSpec - Field specification for configurable runnables
// ============================================================================

/// Specification for a field that can be configured by the user.
///
/// This type is used to describe dynamic configuration parameters that can be
/// passed at runtime through the `configurable` section of `RunnableConfig`.
///
/// # Purpose
///
/// `ConfigurableFieldSpec` is used by advanced runnables like:
/// - `RunnableWithMessageHistory` - Specify `session_id` and other history parameters
/// - `RunnableConfigurableFields` - Define which fields can be configured
/// - `RunnableConfigurableAlternatives` - Specify alternative implementations
///
/// # Python Baseline Compatibility
///
/// Matches `ConfigurableFieldSpec` in `dashflow_core/runnables/utils.py:615-632`.
///
/// # Fields
///
/// - `id`: Unique identifier used in config dictionary
/// - `annotation`: Type annotation (stored as string, e.g., "String", "i32")
/// - `name`: Human-readable name for UI/docs (optional)
/// - `description`: Help text explaining the field (optional)
/// - `default`: Default value as JSON (optional)
/// - `is_shared`: Whether this config is shared across runnable boundaries (optional)
/// - `dependencies`: Other field IDs this field depends on (optional)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::ConfigurableFieldSpec;
///
/// let session_id_spec = ConfigurableFieldSpec {
///     id: "session_id".to_string(),
///     annotation: "String".to_string(),
///     name: Some("Session ID".to_string()),
///     description: Some("Unique identifier for the conversation session".to_string()),
///     default: Some(serde_json::json!("")),
///     is_shared: true,
///     dependencies: None,
/// };
/// ```
///
/// # Use in `RunnableWithMessageHistory`
///
/// ```rust,ignore
/// let history_config = vec![
///     ConfigurableFieldSpec {
///         id: "user_id".to_string(),
///         annotation: "String".to_string(),
///         name: Some("User ID".to_string()),
///         description: Some("Unique identifier for the user".to_string()),
///         default: Some(serde_json::json!("")),
///         is_shared: true,
///         dependencies: None,
///     },
///     ConfigurableFieldSpec {
///         id: "conversation_id".to_string(),
///         annotation: "String".to_string(),
///         name: Some("Conversation ID".to_string()),
///         description: Some("Unique identifier for the conversation".to_string()),
///         default: Some(serde_json::json!("")),
///         is_shared: true,
///         dependencies: None,
///     },
/// ];
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigurableFieldSpec {
    /// Unique identifier for this field (used in config dictionary)
    pub id: String,

    /// Type annotation as a string (e.g., "String", "i32", "bool")
    pub annotation: String,

    /// Human-readable name for UI/documentation
    pub name: Option<String>,

    /// Description explaining what this field is for
    pub description: Option<String>,

    /// Default value for this field (as JSON)
    pub default: Option<serde_json::Value>,

    /// Whether this field is shared across runnable boundaries
    pub is_shared: bool,

    /// IDs of other fields this field depends on
    pub dependencies: Option<Vec<String>>,
}

impl ConfigurableFieldSpec {
    /// Create a new `ConfigurableFieldSpec` with required fields
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the field
    /// * `annotation` - Type annotation as string
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let spec = ConfigurableFieldSpec::new("session_id", "String");
    /// ```
    pub fn new(id: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            annotation: annotation.into(),
            name: None,
            description: None,
            default: None,
            is_shared: false,
            dependencies: None,
        }
    }

    /// Set the human-readable name
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the default value
    #[must_use]
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Mark this field as shared
    #[must_use]
    pub fn with_shared(mut self, is_shared: bool) -> Self {
        self.is_shared = is_shared;
        self
    }

    /// Set dependencies
    #[must_use]
    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = Some(dependencies);
        self
    }
}

/// Get unique config specs from a sequence, detecting conflicts
///
/// This function deduplicates config specs by ID, ensuring that specs with
/// the same ID are identical. If conflicting specs with the same ID are found,
/// an error is returned.
///
/// # Arguments
///
/// * `specs` - Iterator of `ConfigurableFieldSpec` to deduplicate
///
/// # Returns
///
/// Vector of unique specs, or error if conflicts are found
///
/// # Errors
///
/// Returns error if multiple specs with the same ID have different values
///
/// # Python Baseline Compatibility
///
/// Matches `get_unique_config_specs()` in `dashflow_core/runnables/utils.py:634-663`.
///
/// # Example
///
/// ```rust,ignore
/// let spec1 = ConfigurableFieldSpec::new("session_id", "String");
/// let spec2 = ConfigurableFieldSpec::new("session_id", "String"); // same
/// let spec3 = ConfigurableFieldSpec::new("user_id", "String");
///
/// let unique = get_unique_config_specs(vec![spec1, spec2, spec3])?;
/// assert_eq!(unique.len(), 2); // session_id appears once
/// ```
pub fn get_unique_config_specs(
    specs: impl IntoIterator<Item = ConfigurableFieldSpec>,
) -> Result<Vec<ConfigurableFieldSpec>> {
    use std::collections::HashMap;

    // Group by ID
    let mut by_id: HashMap<String, Vec<ConfigurableFieldSpec>> = HashMap::new();
    for spec in specs {
        by_id.entry(spec.id.clone()).or_default().push(spec);
    }

    // Check for conflicts and collect unique specs
    let mut unique = Vec::new();
    for (id, group) in by_id {
        if group.is_empty() {
            continue;
        }

        let first = &group[0];

        // Check if all specs with this ID are identical
        if group.iter().all(|s| s == first) {
            unique.push(first.clone());
        } else {
            return Err(Error::InvalidInput(format!(
                "Conflicting config specs for id '{}': found {} different specs",
                id,
                group.len()
            )));
        }
    }

    // Sort by ID for deterministic output
    unique.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(unique)
}

// ============================================================================
// RunnableBindingBase - Advanced binding with kwargs and config
// ============================================================================

/// Runnable that delegates calls to another Runnable with bound kwargs and config
///
/// This is the base implementation for binding additional parameters to a Runnable.
/// Use `RunnableBinding` for the public API.
///
/// # Purpose
///
/// `RunnableBindingBase` wraps another Runnable and allows you to:
/// - Bind kwargs that will be passed to all invocations
/// - Bind a config that will be merged with runtime config
/// - Apply config factories to dynamically modify config
/// - Override input/output types for type compatibility
///
/// # Python Equivalent
///
/// This corresponds to `RunnableBindingBase` in `dashflow_core/runnables/base.py`
///
/// # Generic Parameters
///
/// - `R`: The concrete type of the bound Runnable
/// - `Input`: The input type of the Runnable
/// - `Output`: The output type of the Runnable
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::{RunnableBindingBase, Runnable};
/// use std::collections::HashMap;
///
/// // Create a binding that passes extra kwargs
/// let mut kwargs = HashMap::new();
/// kwargs.insert("temperature".to_string(), serde_json::json!(0.7));
///
/// let binding = RunnableBindingBase::new(
///     my_runnable,
///     kwargs,
///     None,
///     vec![],
/// );
///
/// // When invoked, the bound runnable will receive temperature=0.7
/// let result = binding.invoke(input, None).await?;
/// ```
pub struct RunnableBindingBase<R, Input, Output>
where
    R: Runnable<Input = Input, Output = Output>,
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// The underlying Runnable that this binding delegates to
    bound: R,

    /// Additional kwargs to pass to the bound Runnable
    ///
    /// These will be merged with any kwargs passed at invoke time.
    /// NOTE: Not yet used - kwargs merging not implemented in invoke/batch/stream.
    /// Architectural field for Python DashFlow API parity.
    #[allow(dead_code)] // API Parity: Python LangChain kwargs support pending implementation
    kwargs: HashMap<String, serde_json::Value>,

    /// Config to bind to the underlying Runnable
    ///
    /// This will be merged with any config passed at runtime
    config: Option<RunnableConfig>,

    /// Config factories to apply before invoking
    ///
    /// Each factory takes a config and returns a modified config.
    /// Factories are applied in order after merging the bound config.
    config_factories: Vec<Arc<dyn Fn(RunnableConfig) -> RunnableConfig + Send + Sync>>,
}

impl<R, Input, Output> RunnableBindingBase<R, Input, Output>
where
    R: Runnable<Input = Input, Output = Output>,
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// Create a new `RunnableBindingBase`
    ///
    /// # Arguments
    ///
    /// * `bound` - The underlying Runnable to wrap
    /// * `kwargs` - Additional kwargs to pass to the bound Runnable
    /// * `config` - Optional config to bind
    /// * `config_factories` - Functions to dynamically modify config
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use dashflow::core::runnable::RunnableBindingBase;
    ///
    /// let mut kwargs = HashMap::new();
    /// kwargs.insert("temperature".to_string(), serde_json::json!(0.7));
    ///
    /// let binding = RunnableBindingBase::new(
    ///     model,
    ///     kwargs,
    ///     None,
    ///     vec![],
    /// );
    /// ```
    pub fn new(
        bound: R,
        kwargs: HashMap<String, serde_json::Value>,
        config: Option<RunnableConfig>,
        config_factories: Vec<Arc<dyn Fn(RunnableConfig) -> RunnableConfig + Send + Sync>>,
    ) -> Self {
        Self {
            bound,
            kwargs,
            config,
            config_factories,
        }
    }

    /// Create a simple `RunnableBindingBase` with just the bound Runnable
    ///
    /// This is a convenience constructor that creates a binding with empty
    /// kwargs, no config, and no config factories.
    pub fn simple(bound: R) -> Self {
        Self::new(bound, HashMap::new(), None, vec![])
    }

    /// Add a config factory to this binding
    ///
    /// Config factories are functions that transform the `RunnableConfig`
    /// before passing it to the bound Runnable. They are applied in order.
    pub fn with_config_factory(
        mut self,
        factory: Arc<dyn Fn(RunnableConfig) -> RunnableConfig + Send + Sync>,
    ) -> Self {
        self.config_factories.push(factory);
        self
    }

    /// Merge configs: bound config, runtime config, and config factories
    ///
    /// This implements the Python _`merge_configs` method:
    /// 1. Start with bound config
    /// 2. Merge runtime config on top
    /// 3. Apply each config factory in sequence
    fn merge_configs(&self, runtime_config: Option<RunnableConfig>) -> RunnableConfig {
        // Start with bound config or default
        let mut merged = self.config.clone().unwrap_or_default();

        // Merge runtime config if provided
        if let Some(runtime) = runtime_config {
            merged = merged.merge(runtime);
        }

        // Apply config factories
        for factory in &self.config_factories {
            merged = factory(merged);
        }

        merged
    }
}

#[async_trait]
impl<R, Input, Output> Runnable for RunnableBindingBase<R, Input, Output>
where
    R: Runnable<Input = Input, Output = Output>,
    Input: Send + Clone + 'static,
    Output: Send + Clone + 'static,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        self.bound.name()
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        let merged_config = self.merge_configs(config);
        self.bound.invoke(input, Some(merged_config)).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let merged_config = self.merge_configs(config);
        self.bound.batch(inputs, Some(merged_config)).await
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send>>> {
        let merged_config = self.merge_configs(config);
        self.bound.stream(input, Some(merged_config)).await
    }
}

/// Public API for `RunnableBinding`
///
/// This is a type alias for `RunnableBindingBase` that provides the main public interface.
/// In Python, `RunnableBinding` is a subclass of `RunnableBindingBase`, but in Rust we use
/// a type alias since there's no additional functionality.
///
/// # Purpose
///
/// `RunnableBinding` allows you to wrap a Runnable with additional parameters:
/// - Bind kwargs to pass when invoking (e.g., `temperature`, `stop` tokens)
/// - Bind config for tracing, callbacks, metadata
/// - Apply dynamic config transformations
///
/// # Use Cases
///
/// ## 1. Bind Model Parameters
///
/// ```rust,ignore
/// use dashflow::core::runnable::RunnableBinding;
/// use std::collections::HashMap;
///
/// let mut kwargs = HashMap::new();
/// kwargs.insert("temperature".to_string(), serde_json::json!(0.7));
/// kwargs.insert("stop".to_string(), serde_json::json!(["-"]));
///
/// let bound_model = RunnableBinding::new(
///     model,
///     kwargs,
///     None,
///     vec![],
/// );
///
/// // Now all invocations will use temperature=0.7 and stop=["-"]
/// let response = bound_model.invoke(input, None).await?;
/// ```
///
/// ## 2. Bind Configuration
///
/// ```rust,ignore
/// use dashflow::core::runnable::RunnableBinding;
/// use dashflow::core::config::RunnableConfig;
///
/// let config = RunnableConfig::new()
///     .with_tag("production")
///     .with_run_name("customer_query");
///
/// let bound = RunnableBinding::new(
///     chain,
///     HashMap::new(),
///     Some(config),
///     vec![],
/// );
///
/// // All invocations will have production tag and run name
/// let result = bound.invoke(input, None).await?;
/// ```
///
/// ## 3. Dynamic Config with Factories
///
/// ```rust,ignore
/// use dashflow::core::runnable::RunnableBinding;
/// use std::sync::Arc;
///
/// let factory = Arc::new(|mut config: RunnableConfig| {
///     config.tags.push("dynamic".to_string());
///     config
/// });
///
/// let bound = RunnableBinding::new(
///     runnable,
///     HashMap::new(),
///     None,
///     vec![factory],
/// );
/// ```
///
/// # Python Equivalent
///
/// ```python
/// from dashflow_core.runnables import RunnableBinding
///
/// # Python version
/// bound = RunnableBinding(
///     bound=model,
///     kwargs={"temperature": 0.7, "stop": ["-"]},
///     config={"tags": ["production"]},
/// )
/// ```
///
/// # See Also
///
/// - `Runnable::bind()` - Convenient method to create bindings
/// - `Runnable::with_config()` - Bind only config
/// - `RunnableWithFallbacks` - For error handling
/// - `RunnableRetry` - For retry logic
pub type RunnableBinding<R, Input, Output> = RunnableBindingBase<R, Input, Output>;
