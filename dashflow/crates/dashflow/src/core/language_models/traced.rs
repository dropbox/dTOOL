//! Traced wrapper for chat models with automatic observability
//!
//! This module provides `TracedChatModel`, a wrapper that adds automatic tracing,
//! callback emission, retry with backoff, and rate limiting to any `ChatModel`.
//!
//! # Overview
//!
//! The `TracedChatModel` wrapper instruments all LLM calls with:
//! - OpenTelemetry spans for distributed tracing
//! - Duration metrics
//! - Token count tracking (when available)
//! - Error tracking
//! - **Automatic callback emission** (on_chat_model_start/on_llm_end/on_llm_error)
//! - **Built-in retry** with configurable backoff
//! - **Rate limiting** support
//!
//! # Basic Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::traced::{ChatModelTracedExt, TracedChatModel};
//! use dashflow_openai::ChatOpenAI;
//!
//! // Wrap any chat model with tracing
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4");
//! let traced_llm = llm.with_tracing();
//!
//! // Or with a custom service name
//! let traced_llm = llm.with_tracing_named("my-agent");
//!
//! // All calls are now automatically traced
//! let result = traced_llm.generate(&messages, None, None, None, None).await?;
//! ```
//!
//! # Full-Featured Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::traced::TracedChatModel;
//! use dashflow::core::callbacks::{CallbackManager, ConsoleCallbackHandler};
//! use dashflow::core::retry::RetryPolicy;
//! use dashflow::core::rate_limiters::InMemoryRateLimiter;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! // Create production-ready traced model with all features
//! let traced_llm = TracedChatModel::builder(llm)
//!     .service_name("research-agent")
//!     .callback_manager(callback_manager)
//!     .retry_policy(RetryPolicy::exponential(3))
//!     .rate_limiter(Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0)))
//!     .build();
//! ```
//!
//! # From `Arc<dyn ChatModel>`
//!
//! When using provider-agnostic chat models from `llm_factory::create_llm()`:
//!
//! ```rust,ignore
//! use common::llm_factory::{create_llm, LLMRequirements};
//! use dashflow::core::language_models::traced::TracedChatModel;
//!
//! let llm = create_llm(LLMRequirements::default()).await?;
//! let traced_llm = TracedChatModel::from_arc(llm);
//! ```
//!
//! # Cost Tracking Integration
//!
//! Token usage is automatically extracted from `llm_output` and passed to callbacks.
//! Use a custom callback handler to integrate with `CostTracker`:
//!
//! ```rust,ignore
//! use dashflow::core::callbacks::CallbackHandler;
//! use dashflow_observability::cost::CostTracker;
//!
//! struct CostTrackingHandler {
//!     tracker: Arc<Mutex<CostTracker>>,
//! }
//!
//! #[async_trait]
//! impl CallbackHandler for CostTrackingHandler {
//!     async fn on_llm_end(
//!         &self,
//!         response: &HashMap<String, serde_json::Value>,
//!         _run_id: Uuid,
//!         _parent_run_id: Option<Uuid>,
//!     ) -> Result<()> {
//!         if let (Some(model), Some(input), Some(output)) = (
//!             response.get("model").and_then(|v| v.as_str()),
//!             response.get("input_tokens").and_then(|v| v.as_u64()),
//!             response.get("output_tokens").and_then(|v| v.as_u64()),
//!         ) {
//!             self.tracker.lock().unwrap().record_llm_call(model, input, output, None);
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use async_trait::async_trait;
use futures::stream::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info_span, Instrument};
use uuid::Uuid;

use crate::core::callbacks::CallbackManager;
use crate::core::error::{Error, Result};
use crate::core::language_models::{
    ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
};
use crate::core::messages::{BaseMessage, Message};
use crate::core::rate_limiters::RateLimiter;
use crate::core::retry::{RetryPolicy, RetryStrategy};

/// Extension trait adding tracing support to `ChatModel`.
///
/// This trait is automatically implemented for all types that implement `ChatModel`,
/// providing convenient methods to wrap models with automatic tracing.
pub trait ChatModelTracedExt: ChatModel {
    /// Wrap this chat model with automatic tracing.
    ///
    /// Returns a `TracedChatModel` that instruments all calls with OpenTelemetry spans.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced_llm = llm.with_tracing();
    /// ```
    fn with_tracing(self) -> TracedChatModel
    where
        Self: Sized + 'static;

    /// Wrap this chat model with automatic tracing and a custom service name.
    ///
    /// The service name is included in span attributes for easier filtering.
    ///
    /// # Arguments
    ///
    /// * `service_name` - Name to identify this service in traces
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced_llm = llm.with_tracing_named("research-agent");
    /// ```
    fn with_tracing_named(self, service_name: impl Into<String>) -> TracedChatModel
    where
        Self: Sized + 'static;
}

/// Blanket implementation of `ChatModelTracedExt` for all `ChatModel` implementations.
impl<M: ChatModel + Sized + 'static> ChatModelTracedExt for M {
    fn with_tracing(self) -> TracedChatModel {
        TracedChatModel::new(self)
    }

    fn with_tracing_named(self, service_name: impl Into<String>) -> TracedChatModel {
        TracedChatModel::with_service_name(self, service_name)
    }
}

/// A chat model wrapper that adds automatic tracing and observability.
///
/// This struct wraps any `ChatModel` and instruments all LLM calls with
/// OpenTelemetry spans, automatic callback emission, retry with backoff,
/// and rate limiting. This enables:
///
/// - Distributed tracing across service boundaries
/// - Performance monitoring and latency tracking
/// - Token usage tracking
/// - Error monitoring and alerting
/// - **Automatic callbacks** for integration with cost tracking, logging, etc.
/// - **Built-in retry** with configurable exponential backoff
/// - **Rate limiting** to prevent API quota exhaustion
///
/// # Span Attributes
///
/// Each traced call includes the following span attributes:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `llm.type` | The type of language model (e.g., "openai", "anthropic") |
/// | `llm.model` | The specific model name (if available in identifying_params) |
/// | `llm.message_count` | Number of input messages |
/// | `llm.has_tools` | Whether tools are provided |
/// | `llm.tool_count` | Number of tools (if any) |
/// | `llm.duration_ms` | Call duration in milliseconds |
/// | `llm.success` | Whether the call succeeded |
/// | `llm.input_tokens` | Input tokens (if available in response) |
/// | `llm.output_tokens` | Output tokens (if available in response) |
/// | `service.name` | Service name (if configured) |
///
/// # Callback Integration
///
/// When a `CallbackManager` is configured, the following callbacks are emitted:
/// - `on_chat_model_start` - Before the LLM call
/// - `on_llm_end` - After successful completion (includes token usage)
/// - `on_llm_error` - On failure
///
/// The `on_llm_end` response includes:
/// - `model`: The model name
/// - `input_tokens`: Prompt tokens (if available)
/// - `output_tokens`: Completion tokens (if available)
/// - `duration_ms`: Call duration
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::language_models::traced::TracedChatModel;
///
/// // Simple usage - just tracing
/// let traced = TracedChatModel::new(my_model);
///
/// // Full-featured usage with builder
/// let traced = TracedChatModel::builder(my_model)
///     .service_name("research-agent")
///     .callback_manager(callbacks)
///     .retry_policy(RetryPolicy::exponential(3))
///     .rate_limiter(limiter)
///     .build();
/// ```
pub struct TracedChatModel {
    /// The underlying chat model
    inner: Arc<dyn ChatModel>,

    /// Optional service name for trace attribution
    service_name: Option<String>,

    /// Optional callback manager for automatic callback emission
    callback_manager: Option<CallbackManager>,

    /// Optional retry policy for automatic retry with backoff
    retry_policy: Option<RetryPolicy>,

    /// Optional rate limiter (separate from inner model's rate limiter)
    custom_rate_limiter: Option<Arc<dyn RateLimiter>>,
}

/// Builder for creating a `TracedChatModel` with full configuration.
///
/// Use this builder when you need callbacks, retry, or rate limiting.
/// For simple tracing, use `TracedChatModel::new()` or `.with_tracing()`.
///
/// # Example
///
/// ```rust,ignore
/// let traced = TracedChatModel::builder(model)
///     .service_name("my-service")
///     .callback_manager(callbacks)
///     .retry_policy(RetryPolicy::exponential(3))
///     .build();
/// ```
pub struct TracedChatModelBuilder {
    inner: Arc<dyn ChatModel>,
    service_name: Option<String>,
    callback_manager: Option<CallbackManager>,
    retry_policy: Option<RetryPolicy>,
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl TracedChatModelBuilder {
    /// Create a new builder from a chat model.
    fn new<M: ChatModel + 'static>(model: M) -> Self {
        Self {
            inner: Arc::new(model),
            service_name: None,
            callback_manager: None,
            retry_policy: None,
            rate_limiter: None,
        }
    }

    /// Create a new builder from an Arc<dyn ChatModel>.
    fn from_arc(model: Arc<dyn ChatModel>) -> Self {
        Self {
            inner: model,
            service_name: None,
            callback_manager: None,
            retry_policy: None,
            rate_limiter: None,
        }
    }

    /// Set the service name for trace attribution.
    #[must_use]
    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = Some(name.into());
        self
    }

    /// Set the callback manager for automatic callback emission.
    ///
    /// When set, the traced model will automatically emit:
    /// - `on_chat_model_start` before each call
    /// - `on_llm_end` after successful calls (with token usage)
    /// - `on_llm_error` on failures
    #[must_use]
    pub fn callback_manager(mut self, manager: CallbackManager) -> Self {
        self.callback_manager = Some(manager);
        self
    }

    /// Set the retry policy for automatic retry with backoff.
    ///
    /// When set, failed calls will be retried according to the policy.
    /// Only retryable errors (network, timeout, rate limit) are retried.
    #[must_use]
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// Set a custom rate limiter.
    ///
    /// This rate limiter is applied in addition to any rate limiter
    /// on the underlying model.
    #[must_use]
    pub fn rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Build the `TracedChatModel`.
    #[must_use]
    pub fn build(self) -> TracedChatModel {
        TracedChatModel {
            inner: self.inner,
            service_name: self.service_name,
            callback_manager: self.callback_manager,
            retry_policy: self.retry_policy,
            custom_rate_limiter: self.rate_limiter,
        }
    }
}

impl TracedChatModel {
    /// Create a new `TracedChatModel` wrapping the given `ChatModel`.
    ///
    /// For simple tracing without callbacks, retry, or rate limiting.
    /// Use `TracedChatModel::builder()` for full configuration.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model to wrap
    ///
    /// # Returns
    ///
    /// A new `TracedChatModel` with tracing enabled
    pub fn new<M: ChatModel + 'static>(model: M) -> Self {
        Self {
            inner: Arc::new(model),
            service_name: None,
            callback_manager: None,
            retry_policy: None,
            custom_rate_limiter: None,
        }
    }

    /// Create a builder for configuring a `TracedChatModel`.
    ///
    /// Use the builder for full configuration including callbacks, retry, and rate limiting.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced = TracedChatModel::builder(model)
    ///     .service_name("my-service")
    ///     .callback_manager(callbacks)
    ///     .retry_policy(RetryPolicy::exponential(3))
    ///     .rate_limiter(limiter)
    ///     .build();
    /// ```
    pub fn builder<M: ChatModel + 'static>(model: M) -> TracedChatModelBuilder {
        TracedChatModelBuilder::new(model)
    }

    /// Create a builder from an existing `Arc<dyn ChatModel>`.
    ///
    /// Use when you have a provider-agnostic model from `llm_factory::create_llm()`.
    pub fn builder_from_arc(model: Arc<dyn ChatModel>) -> TracedChatModelBuilder {
        TracedChatModelBuilder::from_arc(model)
    }

    /// Create a new `TracedChatModel` with a service name.
    ///
    /// The service name is included in trace attributes for easier filtering
    /// and grouping in observability platforms.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model to wrap
    /// * `service_name` - Name to identify this service in traces
    pub fn with_service_name<M: ChatModel + 'static>(
        model: M,
        service_name: impl Into<String>,
    ) -> Self {
        Self {
            inner: Arc::new(model),
            service_name: Some(service_name.into()),
            callback_manager: None,
            retry_policy: None,
            custom_rate_limiter: None,
        }
    }

    /// Create a new `TracedChatModel` from an existing `Arc<dyn ChatModel>`.
    ///
    /// This is useful when you have a provider-agnostic chat model from
    /// `llm_factory::create_llm()` and want to add tracing.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model wrapped in Arc
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use common::llm_factory::{create_llm, LLMRequirements};
    /// use dashflow::core::language_models::traced::TracedChatModel;
    ///
    /// let llm = create_llm(LLMRequirements::default()).await?;
    /// let traced_llm = TracedChatModel::from_arc(llm);
    /// ```
    pub fn from_arc(model: Arc<dyn ChatModel>) -> Self {
        Self {
            inner: model,
            service_name: None,
            callback_manager: None,
            retry_policy: None,
            custom_rate_limiter: None,
        }
    }

    /// Create a new `TracedChatModel` from Arc with a service name.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model wrapped in Arc
    /// * `service_name` - Name to identify this service in traces
    pub fn from_arc_named(model: Arc<dyn ChatModel>, service_name: impl Into<String>) -> Self {
        Self {
            inner: model,
            service_name: Some(service_name.into()),
            callback_manager: None,
            retry_policy: None,
            custom_rate_limiter: None,
        }
    }

    /// Get a reference to the underlying chat model.
    #[must_use]
    pub fn inner(&self) -> &dyn ChatModel {
        &*self.inner
    }

    /// Get the service name, if configured.
    #[must_use]
    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }

    /// Get the callback manager, if configured.
    #[must_use]
    pub fn callback_manager(&self) -> Option<&CallbackManager> {
        self.callback_manager.as_ref()
    }

    /// Get the retry policy, if configured.
    #[must_use]
    pub fn retry_policy(&self) -> Option<&RetryPolicy> {
        self.retry_policy.as_ref()
    }

    /// Extract model name from identifying params if available.
    fn model_name(&self) -> Option<String> {
        let params = self.inner.identifying_params();
        params
            .get("model")
            .or_else(|| params.get("model_name"))
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Extract token usage from llm_output.
    fn extract_token_usage(
        llm_output: &Option<HashMap<String, serde_json::Value>>,
    ) -> (Option<u64>, Option<u64>) {
        let Some(output) = llm_output else {
            return (None, None);
        };

        // Try common patterns for token usage
        // OpenAI style: usage.prompt_tokens, usage.completion_tokens
        if let Some(usage) = output.get("usage").and_then(|v| v.as_object()) {
            let input = usage.get("prompt_tokens").and_then(|v| v.as_u64());
            let output = usage.get("completion_tokens").and_then(|v| v.as_u64());
            if input.is_some() || output.is_some() {
                return (input, output);
            }
        }

        // Direct token counts
        let input = output
            .get("prompt_tokens")
            .or_else(|| output.get("input_tokens"))
            .and_then(|v| v.as_u64());
        let output_tokens = output
            .get("completion_tokens")
            .or_else(|| output.get("output_tokens"))
            .and_then(|v| v.as_u64());

        (input, output_tokens)
    }

    /// Check if an error is retryable.
    fn is_retryable(error: &Error) -> bool {
        RetryPolicy::is_retryable(error)
    }

    /// Calculate delay for a retry attempt.
    fn calculate_delay(&self, attempt: usize) -> std::time::Duration {
        let Some(policy) = &self.retry_policy else {
            return std::time::Duration::ZERO;
        };

        match &policy.strategy {
            RetryStrategy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                multiplier,
            } => {
                let delay = initial_delay_ms * multiplier.pow(attempt as u32);
                std::time::Duration::from_millis(delay.min(*max_delay_ms))
            }
            RetryStrategy::ExponentialJitter {
                initial_delay_ms,
                max_delay_ms,
                exp_base,
                jitter_ms,
            } => {
                use rand::Rng;
                let exp_delay = (*initial_delay_ms as f64) * exp_base.powi(attempt as i32);
                let base_delay = exp_delay.min(*max_delay_ms as f64) as u64;
                let jitter = rand::thread_rng().gen_range(0..=*jitter_ms);
                std::time::Duration::from_millis((base_delay + jitter).min(*max_delay_ms))
            }
            RetryStrategy::Fixed { delay_ms } => std::time::Duration::from_millis(*delay_ms),
        }
    }
}

#[async_trait]
impl ChatModel for TracedChatModel {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        let llm_type = self.inner.llm_type();
        let model_name = self.model_name();
        let message_count = messages.len();
        let has_tools = tools.is_some() && !tools.map_or(true, |t| t.is_empty());
        let tool_count = tools.map_or(0, |t| t.len());

        // Generate run_id for callbacks
        let run_id = Uuid::new_v4();

        // Create span with attributes
        let span = if let Some(ref service) = self.service_name {
            if let Some(ref model) = model_name {
                info_span!(
                    "llm.generate",
                    llm.r#type = llm_type,
                    llm.model = model.as_str(),
                    llm.message_count = message_count,
                    llm.has_tools = has_tools,
                    llm.tool_count = tool_count,
                    service.name = service.as_str(),
                    llm.duration_ms = tracing::field::Empty,
                    llm.success = tracing::field::Empty,
                    llm.input_tokens = tracing::field::Empty,
                    llm.output_tokens = tracing::field::Empty,
                    llm.retry_count = tracing::field::Empty,
                )
            } else {
                info_span!(
                    "llm.generate",
                    llm.r#type = llm_type,
                    llm.message_count = message_count,
                    llm.has_tools = has_tools,
                    llm.tool_count = tool_count,
                    service.name = service.as_str(),
                    llm.duration_ms = tracing::field::Empty,
                    llm.success = tracing::field::Empty,
                    llm.input_tokens = tracing::field::Empty,
                    llm.output_tokens = tracing::field::Empty,
                    llm.retry_count = tracing::field::Empty,
                )
            }
        } else if let Some(ref model) = model_name {
            info_span!(
                "llm.generate",
                llm.r#type = llm_type,
                llm.model = model.as_str(),
                llm.message_count = message_count,
                llm.has_tools = has_tools,
                llm.tool_count = tool_count,
                llm.duration_ms = tracing::field::Empty,
                llm.success = tracing::field::Empty,
                llm.input_tokens = tracing::field::Empty,
                llm.output_tokens = tracing::field::Empty,
                llm.retry_count = tracing::field::Empty,
            )
        } else {
            info_span!(
                "llm.generate",
                llm.r#type = llm_type,
                llm.message_count = message_count,
                llm.has_tools = has_tools,
                llm.tool_count = tool_count,
                llm.duration_ms = tracing::field::Empty,
                llm.success = tracing::field::Empty,
                llm.input_tokens = tracing::field::Empty,
                llm.output_tokens = tracing::field::Empty,
                llm.retry_count = tracing::field::Empty,
            )
        };

        // Emit on_chat_model_start callback
        if let Some(ref callbacks) = self.callback_manager {
            let serialized = self.identifying_params();
            let messages_for_callback: Vec<Vec<Message>> = vec![messages.to_vec()];
            let _ = callbacks
                .on_chat_model_start(
                    &serialized,
                    &messages_for_callback,
                    run_id,
                    None,
                    &[],
                    &HashMap::new(),
                )
                .await;
        }

        let start = Instant::now();
        let max_retries = self.retry_policy.as_ref().map_or(0, |p| p.max_retries);
        let mut retry_count = 0_usize;

        // Retry loop
        let result = 'retry: loop {
            // Rate limiter check (custom rate limiter)
            if let Some(ref limiter) = self.custom_rate_limiter {
                limiter.acquire().await;
            }

            // Also check retry policy's rate limiter if present
            if retry_count > 0 {
                if let Some(ref policy) = self.retry_policy {
                    if let Some(ref limiter) = policy.rate_limiter {
                        limiter.acquire().await;
                    }
                }
            }

            let attempt_result = async {
                self.inner
                    ._generate(messages, stop, tools, tool_choice, run_manager)
                    .await
            }
            .instrument(span.clone())
            .await;

            match attempt_result {
                Ok(result) => break 'retry Ok(result),
                Err(e) => {
                    if retry_count < max_retries && Self::is_retryable(&e) {
                        // Log retry
                        tracing::info!(
                            parent: &span,
                            attempt = retry_count + 1,
                            max_retries = max_retries,
                            error = %e,
                            "Retrying LLM call"
                        );

                        // Emit retry callback
                        if let Some(ref callbacks) = self.callback_manager {
                            let _ = callbacks.on_retry(run_id, None).await;
                        }

                        // Wait before retry
                        let delay = self.calculate_delay(retry_count);
                        if !delay.is_zero() {
                            tokio::time::sleep(delay).await;
                        }

                        retry_count += 1;
                        continue 'retry;
                    }
                    break 'retry Err(e);
                }
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = result.is_ok();

        // Record final attributes
        span.record("llm.duration_ms", duration_ms);
        span.record("llm.success", success);
        span.record("llm.retry_count", retry_count as u64);

        // Extract token usage and emit callbacks
        match &result {
            Ok(chat_result) => {
                let (input_tokens, output_tokens) =
                    Self::extract_token_usage(&chat_result.llm_output);

                if let Some(input) = input_tokens {
                    span.record("llm.input_tokens", input);
                }
                if let Some(output) = output_tokens {
                    span.record("llm.output_tokens", output);
                }

                tracing::info!(
                    parent: &span,
                    duration_ms = duration_ms,
                    input_tokens = ?input_tokens,
                    output_tokens = ?output_tokens,
                    retry_count = retry_count,
                    "LLM generation completed"
                );

                // Emit on_llm_end callback with token usage
                if let Some(ref callbacks) = self.callback_manager {
                    let mut response = HashMap::new();
                    response.insert("duration_ms".to_string(), serde_json::json!(duration_ms));
                    if let Some(ref model) = model_name {
                        response.insert("model".to_string(), serde_json::json!(model));
                    }
                    if let Some(input) = input_tokens {
                        response.insert("input_tokens".to_string(), serde_json::json!(input));
                    }
                    if let Some(output) = output_tokens {
                        response.insert("output_tokens".to_string(), serde_json::json!(output));
                    }
                    response.insert("retry_count".to_string(), serde_json::json!(retry_count));
                    let _ = callbacks.on_llm_end(&response, run_id, None).await;
                }
            }
            Err(e) => {
                tracing::warn!(
                    parent: &span,
                    duration_ms = duration_ms,
                    error = %e,
                    retry_count = retry_count,
                    "LLM generation failed"
                );

                // Emit on_llm_error callback
                if let Some(ref callbacks) = self.callback_manager {
                    let _ = callbacks.on_llm_error(&e.to_string(), run_id, None).await;
                }
            }
        }

        result
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        let llm_type = self.inner.llm_type();
        let model_name = self.model_name();
        let message_count = messages.len();
        let has_tools = tools.is_some() && !tools.map_or(true, |t| t.is_empty());
        let tool_count = tools.map_or(0, |t| t.len());

        // Generate run_id for callbacks
        let run_id = Uuid::new_v4();

        // Create span for stream initiation
        let span = if let Some(ref service) = self.service_name {
            if let Some(ref model) = model_name {
                info_span!(
                    "llm.stream",
                    llm.r#type = llm_type,
                    llm.model = model.as_str(),
                    llm.message_count = message_count,
                    llm.has_tools = has_tools,
                    llm.tool_count = tool_count,
                    service.name = service.as_str(),
                )
            } else {
                info_span!(
                    "llm.stream",
                    llm.r#type = llm_type,
                    llm.message_count = message_count,
                    llm.has_tools = has_tools,
                    llm.tool_count = tool_count,
                    service.name = service.as_str(),
                )
            }
        } else if let Some(ref model) = model_name {
            info_span!(
                "llm.stream",
                llm.r#type = llm_type,
                llm.model = model.as_str(),
                llm.message_count = message_count,
                llm.has_tools = has_tools,
                llm.tool_count = tool_count,
            )
        } else {
            info_span!(
                "llm.stream",
                llm.r#type = llm_type,
                llm.message_count = message_count,
                llm.has_tools = has_tools,
                llm.tool_count = tool_count,
            )
        };

        // Rate limiter check for stream initiation
        if let Some(ref limiter) = self.custom_rate_limiter {
            limiter.acquire().await;
        }

        // Emit on_chat_model_start callback
        if let Some(ref callbacks) = self.callback_manager {
            let serialized = self.identifying_params();
            let messages_for_callback: Vec<Vec<Message>> = vec![messages.to_vec()];
            let _ = callbacks
                .on_chat_model_start(
                    &serialized,
                    &messages_for_callback,
                    run_id,
                    None,
                    &[],
                    &HashMap::new(),
                )
                .await;
        }

        tracing::info!(parent: &span, "LLM stream started");

        // Note: Individual chunks are not traced to avoid overhead.
        // Stream completion/error callbacks would require wrapping the stream,
        // which is deferred to avoid complexity. Use the run_manager for detailed tracking.
        self.inner
            ._stream(messages, stop, tools, tool_choice, run_manager)
            .instrument(span)
            .await
    }

    fn llm_type(&self) -> &str {
        self.inner.llm_type()
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        let mut params = self.inner.identifying_params();
        params.insert("traced".to_string(), serde_json::json!(true));
        if let Some(ref service) = self.service_name {
            params.insert("service_name".to_string(), serde_json::json!(service));
        }
        params
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        // Return custom rate limiter if set, otherwise inner model's rate limiter
        self.custom_rate_limiter
            .clone()
            .or_else(|| self.inner.rate_limiter())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::language_models::ChatGeneration;
    use crate::core::messages::{AIMessage, HumanMessage};

    /// Mock ChatModel for testing
    struct MockChatModel {
        response: String,
        llm_type: String,
    }

    impl MockChatModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
                llm_type: "mock".to_string(),
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(self.response.clone()).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            &self.llm_type
        }

        fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
            let mut params = HashMap::new();
            params.insert("model".to_string(), serde_json::json!("mock-model-v1"));
            params
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_traced_chat_model_creation() {
        let mock = MockChatModel::new("test response");
        let traced = mock.with_tracing();

        assert_eq!(traced.llm_type(), "mock");
        assert!(traced.identifying_params().contains_key("traced"));
    }

    #[tokio::test]
    async fn test_traced_chat_model_with_service_name() {
        let mock = MockChatModel::new("test response");
        let traced = mock.with_tracing_named("test-service");

        assert_eq!(traced.service_name(), Some("test-service"));
        let params = traced.identifying_params();
        assert_eq!(
            params.get("service_name"),
            Some(&serde_json::json!("test-service"))
        );
    }

    #[tokio::test]
    async fn test_traced_chat_model_generate() {
        // Initialize tracing for test (ignore errors if already initialized)
        let _ = tracing_subscriber::fmt::try_init();

        let mock = MockChatModel::new("traced response");
        let traced = TracedChatModel::new(mock);

        let messages = vec![HumanMessage::new("test").into()];
        let result = traced.generate(&messages, None, None, None, None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(
            result.generations[0].message.content().as_text(),
            "traced response"
        );
    }

    #[tokio::test]
    async fn test_traced_chat_model_from_arc() {
        let mock: Arc<dyn ChatModel> = Arc::new(MockChatModel::new("arc response"));
        let traced = TracedChatModel::from_arc(mock);

        let messages = vec![HumanMessage::new("test").into()];
        let result = traced.generate(&messages, None, None, None, None).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_chat_model_from_arc_named() {
        let mock: Arc<dyn ChatModel> = Arc::new(MockChatModel::new("arc response"));
        let traced = TracedChatModel::from_arc_named(mock, "my-service");

        assert_eq!(traced.service_name(), Some("my-service"));
    }

    #[tokio::test]
    async fn test_extension_trait_available() {
        let mock = MockChatModel::new("test");

        // Verify extension trait works
        let _traced = mock.with_tracing();
    }

    #[tokio::test]
    async fn test_model_name_extraction() {
        let mock = MockChatModel::new("test");
        let traced = TracedChatModel::new(mock);

        // MockChatModel provides "model" in identifying_params
        assert_eq!(traced.model_name(), Some("mock-model-v1".to_string()));
    }

    #[tokio::test]
    async fn test_inner_access() {
        let mock = MockChatModel::new("test");
        let traced = TracedChatModel::new(mock);

        assert_eq!(traced.inner().llm_type(), "mock");
    }

    #[tokio::test]
    async fn test_builder_basic() {
        let mock = MockChatModel::new("builder response");
        let traced = TracedChatModel::builder(mock)
            .service_name("builder-service")
            .build();

        assert_eq!(traced.service_name(), Some("builder-service"));
        assert!(traced.callback_manager().is_none());
        assert!(traced.retry_policy().is_none());
    }

    #[tokio::test]
    async fn test_builder_with_callback_manager() {
        let mock = MockChatModel::new("callback response");
        let callbacks = CallbackManager::new();

        let traced = TracedChatModel::builder(mock)
            .callback_manager(callbacks)
            .build();

        assert!(traced.callback_manager().is_some());
    }

    #[tokio::test]
    async fn test_builder_with_retry_policy() {
        let mock = MockChatModel::new("retry response");
        let policy = RetryPolicy::exponential(3);

        let traced = TracedChatModel::builder(mock).retry_policy(policy).build();

        assert!(traced.retry_policy().is_some());
        assert_eq!(traced.retry_policy().unwrap().max_retries, 3);
    }

    #[tokio::test]
    async fn test_builder_from_arc() {
        let mock: Arc<dyn ChatModel> = Arc::new(MockChatModel::new("arc builder response"));

        let traced = TracedChatModel::builder_from_arc(mock)
            .service_name("arc-service")
            .build();

        assert_eq!(traced.service_name(), Some("arc-service"));
    }

    #[tokio::test]
    async fn test_builder_full_configuration() {
        let mock = MockChatModel::new("full config response");
        let callbacks = CallbackManager::new();
        let policy = RetryPolicy::exponential(2);

        let traced = TracedChatModel::builder(mock)
            .service_name("full-service")
            .callback_manager(callbacks)
            .retry_policy(policy)
            .build();

        assert_eq!(traced.service_name(), Some("full-service"));
        assert!(traced.callback_manager().is_some());
        assert!(traced.retry_policy().is_some());
    }

    #[tokio::test]
    async fn test_token_usage_extraction_openai_style() {
        // OpenAI style: usage object with prompt_tokens and completion_tokens
        let mut llm_output = HashMap::new();
        let mut usage = serde_json::Map::new();
        usage.insert("prompt_tokens".to_string(), serde_json::json!(100));
        usage.insert("completion_tokens".to_string(), serde_json::json!(50));
        llm_output.insert("usage".to_string(), serde_json::Value::Object(usage));

        let (input, output) = TracedChatModel::extract_token_usage(&Some(llm_output));

        assert_eq!(input, Some(100));
        assert_eq!(output, Some(50));
    }

    #[tokio::test]
    async fn test_token_usage_extraction_direct_style() {
        // Direct style: input_tokens and output_tokens at top level
        let mut llm_output = HashMap::new();
        llm_output.insert("input_tokens".to_string(), serde_json::json!(200));
        llm_output.insert("output_tokens".to_string(), serde_json::json!(75));

        let (input, output) = TracedChatModel::extract_token_usage(&Some(llm_output));

        assert_eq!(input, Some(200));
        assert_eq!(output, Some(75));
    }

    #[tokio::test]
    async fn test_token_usage_extraction_none() {
        let (input, output) = TracedChatModel::extract_token_usage(&None);

        assert_eq!(input, None);
        assert_eq!(output, None);
    }

    #[tokio::test]
    async fn test_generate_with_callbacks() {
        let _ = tracing_subscriber::fmt::try_init();

        let mock = MockChatModel::new("callback test response");
        let callbacks = CallbackManager::new();

        let traced = TracedChatModel::builder(mock)
            .callback_manager(callbacks)
            .build();

        let messages = vec![HumanMessage::new("test").into()];
        let result = traced.generate(&messages, None, None, None, None).await;

        assert!(result.is_ok());
    }
}
