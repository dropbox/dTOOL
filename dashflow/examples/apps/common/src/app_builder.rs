// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! DashFlowApp Builder - Production-ready application setup
//!
//! This module provides `DashFlowApp`, a builder that creates production-ready
//! DashFlow applications with sensible defaults for observability, resilience, and cost tracking.
//!
//! # Overview
//!
//! Instead of manually wiring together:
//! - LLM factory
//! - TracedChatModel with callbacks
//! - RetryPolicy with backoff
//! - RateLimiter for API throttling
//! - CostTracker for budget management
//! - RunnableConfig with tags and metadata
//!
//! Simply use the builder:
//!
//! ```rust,ignore
//! use common::app_builder::{DashFlowApp, DashFlowAppConfig};
//!
//! let app = DashFlowApp::builder()
//!     .name("my-rag-pipeline")
//!     .build()
//!     .await?;
//!
//! // Access pre-configured components
//! let llm = app.traced_llm();  // TracedChatModel with callbacks, retry, rate limiting
//! let callbacks = app.callbacks();
//! let cost_tracker = app.cost_tracker();
//! let config = app.runnable_config();
//! ```
//!
//! # Configuration Options
//!
//! ```rust,ignore
//! let app = DashFlowApp::builder()
//!     .name("research-agent")
//!     .with_llm_requirements(LLMRequirements {
//!         needs_tools: true,
//!         ..Default::default()
//!     })
//!     .with_rate_limit(20.0)  // 20 requests/second
//!     .with_daily_budget(100.0)  // $100/day budget
//!     .with_retry_attempts(5)
//!     .with_console_callbacks(true)
//!     .with_tags(&["production", "rag"])
//!     .build()
//!     .await?;
//! ```
//!
//! # Environment Variables
//!
//! The builder respects these environment variables:
//! - `LLM_RATE_LIMIT`: Requests per second (default: 10.0)
//! - `LLM_TIMEOUT_SECS`: Timeout for LLM calls (default: 30)
//! - `DAILY_BUDGET`: Daily cost budget in USD (default: unlimited)
//! - `MONTHLY_BUDGET`: Monthly cost budget in USD (default: unlimited)

use dashflow::core::{
    callbacks::{CallbackHandler, CallbackManager, ConsoleCallbackHandler},
    config::RunnableConfig,
    language_models::{traced::TracedChatModel, ChatModel},
    rate_limiters::{InMemoryRateLimiter, RateLimiter},
    retry::RetryPolicy,
};
use dashflow_factories::{create_llm, LLMRequirements};
use dashflow_observability::cost::CostTracker;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

/// Configuration for DashFlowApp
#[derive(Clone)]
pub struct DashFlowAppConfig {
    /// Application name (used in traces and logs)
    pub name: String,

    /// LLM requirements for provider selection
    pub llm_requirements: LLMRequirements,

    /// Maximum retry attempts for LLM calls
    pub max_retries: usize,

    /// Rate limit in requests per second
    pub rate_limit: f64,

    /// Burst capacity for rate limiter
    pub burst_capacity: f64,

    /// Timeout for LLM calls
    pub timeout: Duration,

    /// Daily budget limit in USD (None = unlimited)
    pub daily_budget: Option<f64>,

    /// Monthly budget limit in USD (None = unlimited)
    pub monthly_budget: Option<f64>,

    /// Enable console callback handler
    pub console_callbacks: bool,

    /// Use colored output for console callbacks
    pub colored_output: bool,

    /// Tags for RunnableConfig
    pub tags: Vec<String>,

    /// Maximum concurrency for parallel operations
    pub max_concurrency: usize,

    /// Maximum recursion depth for graph execution
    pub recursion_limit: usize,

    /// Custom callback handlers to add
    pub custom_handlers: Vec<Arc<dyn CallbackHandler>>,
}

impl Default for DashFlowAppConfig {
    fn default() -> Self {
        // Read from environment or use defaults
        let rate_limit = std::env::var("LLM_RATE_LIMIT")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(10.0);

        let timeout_secs = std::env::var("LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        let daily_budget = std::env::var("DAILY_BUDGET")
            .ok()
            .and_then(|s| s.parse::<f64>().ok());

        let monthly_budget = std::env::var("MONTHLY_BUDGET")
            .ok()
            .and_then(|s| s.parse::<f64>().ok());

        Self {
            name: "dashflow-app".to_string(),
            llm_requirements: LLMRequirements::default(),
            max_retries: 3,
            rate_limit,
            burst_capacity: 20.0,
            timeout: Duration::from_secs(timeout_secs),
            daily_budget,
            monthly_budget,
            console_callbacks: true,
            colored_output: true,
            tags: vec!["production".to_string()],
            max_concurrency: 10,
            recursion_limit: 25,
            custom_handlers: Vec::new(),
        }
    }
}

/// Builder for DashFlowApp
pub struct DashFlowAppBuilder {
    config: DashFlowAppConfig,
}

impl DashFlowAppBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: DashFlowAppConfig::default(),
        }
    }

    /// Set the application name
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    /// Set LLM requirements for provider selection
    #[must_use]
    pub fn with_llm_requirements(mut self, requirements: LLMRequirements) -> Self {
        self.config.llm_requirements = requirements;
        self
    }

    /// Set maximum retry attempts
    #[must_use]
    pub fn with_retry_attempts(mut self, max_retries: usize) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Set rate limit in requests per second
    #[must_use]
    pub fn with_rate_limit(mut self, requests_per_second: f64) -> Self {
        self.config.rate_limit = requests_per_second;
        self
    }

    /// Set burst capacity for rate limiter
    #[must_use]
    pub fn with_burst_capacity(mut self, capacity: f64) -> Self {
        self.config.burst_capacity = capacity;
        self
    }

    /// Set timeout for LLM calls
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set daily budget limit in USD
    #[must_use]
    pub fn with_daily_budget(mut self, budget: f64) -> Self {
        self.config.daily_budget = Some(budget);
        self
    }

    /// Set monthly budget limit in USD
    #[must_use]
    pub fn with_monthly_budget(mut self, budget: f64) -> Self {
        self.config.monthly_budget = Some(budget);
        self
    }

    /// Enable or disable console callback handler
    #[must_use]
    pub fn with_console_callbacks(mut self, enabled: bool) -> Self {
        self.config.console_callbacks = enabled;
        self
    }

    /// Enable or disable colored output for console callbacks
    #[must_use]
    pub fn with_colored_output(mut self, colored: bool) -> Self {
        self.config.colored_output = colored;
        self
    }

    /// Add tags for RunnableConfig
    #[must_use]
    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.config.tags = tags.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Add a single tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.config.tags.push(tag.into());
        self
    }

    /// Set maximum concurrency for parallel operations
    #[must_use]
    pub fn with_max_concurrency(mut self, concurrency: usize) -> Self {
        self.config.max_concurrency = concurrency;
        self
    }

    /// Set maximum recursion depth for graph execution
    #[must_use]
    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.config.recursion_limit = limit;
        self
    }

    /// Add a custom callback handler
    #[must_use]
    pub fn with_callback_handler(mut self, handler: Arc<dyn CallbackHandler>) -> Self {
        self.config.custom_handlers.push(handler);
        self
    }

    /// Build the DashFlowApp
    ///
    /// # Errors
    ///
    /// Returns an error if no LLM provider is available
    pub async fn build(self) -> anyhow::Result<DashFlowApp> {
        // 1. Create raw LLM from factory
        let raw_llm = create_llm(self.config.llm_requirements.clone()).await?;

        // 2. Create rate limiter
        let rate_limiter: Arc<dyn RateLimiter> = Arc::new(InMemoryRateLimiter::new(
            self.config.rate_limit,
            Duration::from_millis(50),
            self.config.burst_capacity,
        ));

        // 3. Create retry policy with rate limiter
        let retry_policy = RetryPolicy::default_jitter(self.config.max_retries)
            .with_rate_limiter(rate_limiter.clone());

        // 4. Create callback manager
        let mut callbacks = CallbackManager::new();
        if self.config.console_callbacks {
            callbacks.add_handler(Arc::new(ConsoleCallbackHandler::new(
                self.config.colored_output,
            )));
        }
        for handler in &self.config.custom_handlers {
            callbacks.add_handler(handler.clone());
        }

        // 5. Create TracedChatModel with all features
        let traced_llm = TracedChatModel::builder_from_arc(raw_llm.clone())
            .service_name(&self.config.name)
            .callback_manager(callbacks.clone())
            .retry_policy(retry_policy.clone())
            .rate_limiter(rate_limiter.clone())
            .build();

        // 6. Create CostTracker
        let mut cost_tracker = CostTracker::with_defaults();
        if let Some(daily) = self.config.daily_budget {
            cost_tracker = cost_tracker.with_daily_budget(daily);
        }
        if let Some(monthly) = self.config.monthly_budget {
            cost_tracker = cost_tracker.with_monthly_budget(monthly);
        }

        // 7. Create RunnableConfig
        let mut runnable_config = RunnableConfig::new()
            .with_run_name(&self.config.name)
            .with_max_concurrency(self.config.max_concurrency)
            .with_recursion_limit(self.config.recursion_limit)
            .with_callbacks(callbacks.clone());

        for tag in &self.config.tags {
            runnable_config = runnable_config.with_tag(tag);
        }

        runnable_config = runnable_config
            .with_metadata("app", &self.config.name)
            .expect("valid metadata");

        Ok(DashFlowApp {
            config: self.config,
            raw_llm,
            traced_llm,
            callbacks,
            rate_limiter,
            retry_policy,
            cost_tracker: Arc::new(Mutex::new(cost_tracker)),
            runnable_config,
        })
    }
}

impl Default for DashFlowAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A production-ready DashFlow application with pre-configured components
///
/// DashFlowApp bundles together all the infrastructure needed for a production
/// LLM application:
/// - Provider-agnostic LLM with automatic tracing
/// - Retry with exponential backoff and jitter
/// - Rate limiting to prevent API quota exhaustion
/// - Cost tracking with budget alerts
/// - Callback management for observability
/// - RunnableConfig with tags and metadata
///
/// # Example
///
/// ```rust,ignore
/// use common::app_builder::DashFlowApp;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let app = DashFlowApp::builder()
///         .name("my-app")
///         .with_daily_budget(50.0)
///         .build()
///         .await?;
///
///     // Use the traced LLM - it has callbacks, retry, and rate limiting built-in
///     let llm = app.traced_llm();
///     let response = llm.generate(&messages, None, None, None, None).await?;
///
///     // Track costs
///     app.record_llm_call("gpt-4o", 1000, 500, Some("my_node"));
///
///     // Get cost report
///     let report = app.cost_report().await;
///     println!("Total cost: ${:.4}", report.total_cost());
///
///     Ok(())
/// }
/// ```
pub struct DashFlowApp {
    config: DashFlowAppConfig,
    raw_llm: Arc<dyn ChatModel>,
    traced_llm: TracedChatModel,
    callbacks: CallbackManager,
    rate_limiter: Arc<dyn RateLimiter>,
    retry_policy: RetryPolicy,
    cost_tracker: Arc<Mutex<CostTracker>>,
    runnable_config: RunnableConfig,
}

impl DashFlowApp {
    /// Create a new builder for DashFlowApp
    pub fn builder() -> DashFlowAppBuilder {
        DashFlowAppBuilder::new()
    }

    /// Get the application name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the raw LLM (without tracing/callbacks/retry)
    ///
    /// Use this when you need direct access to the underlying model.
    /// For most use cases, prefer `traced_llm()` which includes all production features.
    #[must_use]
    pub fn raw_llm(&self) -> Arc<dyn ChatModel> {
        self.raw_llm.clone()
    }

    /// Get the traced LLM with callbacks, retry, and rate limiting
    ///
    /// This is the recommended way to access the LLM for production use.
    /// It includes:
    /// - OpenTelemetry tracing spans
    /// - Automatic callback emission (on_chat_model_start, on_llm_end, on_llm_error)
    /// - Retry with exponential backoff and jitter
    /// - Rate limiting
    #[must_use]
    pub fn traced_llm(&self) -> &TracedChatModel {
        &self.traced_llm
    }

    /// Get the callback manager
    #[must_use]
    pub fn callbacks(&self) -> &CallbackManager {
        &self.callbacks
    }

    /// Get a clone of the callback manager
    #[must_use]
    pub fn callbacks_clone(&self) -> CallbackManager {
        self.callbacks.clone()
    }

    /// Get the rate limiter
    #[must_use]
    pub fn rate_limiter(&self) -> Arc<dyn RateLimiter> {
        self.rate_limiter.clone()
    }

    /// Get the retry policy
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// Get a clone of the retry policy wrapped in Arc for sharing
    #[must_use]
    pub fn retry_policy_arc(&self) -> Arc<RetryPolicy> {
        Arc::new(self.retry_policy.clone())
    }

    /// Get the cost tracker (thread-safe, async)
    #[must_use]
    pub fn cost_tracker(&self) -> Arc<Mutex<CostTracker>> {
        self.cost_tracker.clone()
    }

    /// Get the runnable config
    #[must_use]
    pub fn runnable_config(&self) -> &RunnableConfig {
        &self.runnable_config
    }

    /// Get a clone of the runnable config
    #[must_use]
    pub fn runnable_config_clone(&self) -> RunnableConfig {
        self.runnable_config.clone()
    }

    /// Get the configured timeout
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.config.timeout
    }

    /// Record an LLM call for cost tracking
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "gpt-4o", "claude-3-sonnet")
    /// * `input_tokens` - Number of input/prompt tokens
    /// * `output_tokens` - Number of output/completion tokens
    /// * `node_name` - Optional node name for attribution
    pub async fn record_llm_call(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        node_name: Option<&str>,
    ) {
        let mut tracker = self.cost_tracker.lock().await;
        let _ = tracker.record_llm_call(model, input_tokens, output_tokens, node_name);
    }

    /// Get the current cost report
    pub async fn cost_report(&self) -> dashflow_observability::cost::CostReport {
        let tracker = self.cost_tracker.lock().await;
        tracker.report()
    }

    /// Get total cost so far
    pub async fn total_cost(&self) -> f64 {
        let tracker = self.cost_tracker.lock().await;
        tracker.report().total_cost()
    }

    /// Get total number of LLM calls
    pub async fn total_calls(&self) -> usize {
        let tracker = self.cost_tracker.lock().await;
        tracker.report().total_calls()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_defaults() {
        let builder = DashFlowAppBuilder::new();
        assert_eq!(builder.config.name, "dashflow-app");
        assert_eq!(builder.config.max_retries, 3);
        assert!(builder.config.console_callbacks);
    }

    #[tokio::test]
    async fn test_builder_chaining() {
        let builder = DashFlowApp::builder()
            .name("test-app")
            .with_retry_attempts(5)
            .with_rate_limit(20.0)
            .with_daily_budget(50.0)
            .with_tag("test")
            .with_console_callbacks(false);

        assert_eq!(builder.config.name, "test-app");
        assert_eq!(builder.config.max_retries, 5);
        assert!((builder.config.rate_limit - 20.0).abs() < f64::EPSILON);
        assert_eq!(builder.config.daily_budget, Some(50.0));
        assert!(builder.config.tags.contains(&"test".to_string()));
        assert!(!builder.config.console_callbacks);
    }

    #[tokio::test]
    async fn test_config_defaults() {
        let config = DashFlowAppConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!((config.burst_capacity - 20.0).abs() < f64::EPSILON);
        assert_eq!(config.max_concurrency, 10);
        assert_eq!(config.recursion_limit, 25);
    }
}
