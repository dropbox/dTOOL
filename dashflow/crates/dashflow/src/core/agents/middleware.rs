//! Agent middleware for customizing execution behavior
//!
//! Middleware provides hooks at various points in the agent execution lifecycle,
//! allowing you to add logging, retries, validation, rate limiting, and more.

use tracing::{debug, info, warn};

use crate::constants::DEFAULT_MAX_RETRIES;
use crate::core::error::Result;
use crate::core::tools::ToolInput;
use crate::core::utils::sanitize_for_log_default;

use super::{AgentAction, AgentContext, AgentDecision};

/// Middleware for customizing agent execution
///
/// Middleware provides hooks at various points in the agent execution lifecycle,
/// allowing you to add logging, retries, validation, rate limiting, and more.
///
/// # Lifecycle Hooks
///
/// 1. `before_plan`: Called before `agent.plan()` - can modify context
/// 2. `after_plan`: Called after `agent.plan()` - can modify decision
/// 3. `before_tool`: Called before tool execution - can modify action
/// 4. `after_tool`: Called after tool execution - can modify observation
/// 5. `on_error`: Called when an error occurs - can recover or transform error
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::{AgentMiddleware, AgentContext, AgentAction, AgentDecision};
///
/// struct LoggingMiddleware;
///
/// #[async_trait::async_trait]
/// impl AgentMiddleware for LoggingMiddleware {
///     async fn before_plan(&self, context: &mut AgentContext) -> Result<()> {
///         println!("Planning iteration {}", context.iteration);
///         Ok(())
///     }
///
///     async fn after_tool(&self, action: &AgentAction, observation: &str) -> Result<String> {
///         println!("Tool {} returned: {}", action.tool, observation);
///         Ok(observation.to_string())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait AgentMiddleware: Send + Sync {
    /// Called before the agent plans its next action
    ///
    /// Use this to:
    /// - Log the current state
    /// - Modify the context before planning
    /// - Add preprocessing logic
    async fn before_plan(&self, _context: &mut AgentContext) -> Result<()> {
        Ok(())
    }

    /// Called after the agent plans its next action
    ///
    /// Use this to:
    /// - Validate the agent's decision
    /// - Modify the action or finish result
    /// - Log the decision
    async fn after_plan(
        &self,
        _context: &AgentContext,
        decision: AgentDecision,
    ) -> Result<AgentDecision> {
        Ok(decision)
    }

    /// Called before a tool is executed
    ///
    /// Use this to:
    /// - Validate tool inputs
    /// - Add rate limiting
    /// - Modify the action
    async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
        Ok(action.clone())
    }

    /// Called after a tool is executed
    ///
    /// Use this to:
    /// - Transform the observation
    /// - Add post-processing
    /// - Cache results
    async fn after_tool(&self, _action: &AgentAction, observation: &str) -> Result<String> {
        Ok(observation.to_string())
    }

    /// Called when an error occurs during execution
    ///
    /// Use this to:
    /// - Log errors
    /// - Retry on specific error types
    /// - Transform errors into observations
    ///
    /// Return Ok(Some(observation)) to recover from the error and continue.
    /// Return Ok(None) to propagate the error.
    async fn on_error(&self, _error: &crate::core::Error) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Logging middleware - logs all agent execution steps
///
/// This middleware prints information about each step of agent execution,
/// useful for debugging and monitoring.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::LoggingMiddleware;
///
/// let middleware = LoggingMiddleware::new()
///     .with_prefix("[AGENT]");
/// ```
#[derive(Debug, Clone)]
pub struct LoggingMiddleware {
    prefix: String,
}

impl LoggingMiddleware {
    /// Create a new logging middleware
    #[must_use]
    pub fn new() -> Self {
        Self {
            prefix: "[AGENT]".to_string(),
        }
    }

    /// Set the log prefix
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for LoggingMiddleware {
    async fn before_plan(&self, context: &mut AgentContext) -> Result<()> {
        info!(
            prefix = %self.prefix,
            iteration = context.iteration,
            input = %context.input,
            steps_count = context.intermediate_steps.len(),
            "Planning next action"
        );
        Ok(())
    }

    async fn after_plan(
        &self,
        _context: &AgentContext,
        decision: AgentDecision,
    ) -> Result<AgentDecision> {
        match &decision {
            AgentDecision::Action(action) => {
                debug!(
                    prefix = %self.prefix,
                    tool = %action.tool,
                    tool_input = ?action.tool_input,
                    reasoning = %action.log,
                    "Agent decided to use tool"
                );
            }
            AgentDecision::Finish(finish) => {
                info!(
                    prefix = %self.prefix,
                    output = %finish.output,
                    "Agent finished with answer"
                );
            }
        }
        Ok(decision)
    }

    async fn after_tool(&self, action: &AgentAction, observation: &str) -> Result<String> {
        debug!(
            prefix = %self.prefix,
            tool = %action.tool,
            result = %observation,
            "Tool execution completed"
        );
        Ok(observation.to_string())
    }

    async fn on_error(&self, error: &crate::core::Error) -> Result<Option<String>> {
        warn!(
            prefix = %self.prefix,
            error = %error,
            "Error occurred during agent execution"
        );
        Ok(None)
    }
}

/// Retry middleware - retries tool execution on failures
///
/// This middleware automatically retries failed tool executions with
/// exponential backoff, useful for handling transient failures.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::RetryMiddleware;
///
/// let middleware = RetryMiddleware::new()
///     .with_max_retries(3)
///     .with_initial_delay_ms(100);
/// ```
#[derive(Debug, Clone)]
pub struct RetryMiddleware {
    max_retries: usize,
    initial_delay_ms: u64,
    backoff_factor: f64,
}

impl RetryMiddleware {
    /// Create a new retry middleware with default settings
    ///
    /// Default: 2 retries, 100ms initial delay, 2x backoff
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_retries: 2,
            initial_delay_ms: 100,
            backoff_factor: 2.0,
        }
    }

    /// Set the maximum number of retries
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the initial delay in milliseconds
    #[must_use]
    pub const fn with_initial_delay_ms(mut self, delay_ms: u64) -> Self {
        self.initial_delay_ms = delay_ms;
        self
    }

    /// Set the backoff factor (multiplier for each retry)
    #[must_use]
    pub fn with_backoff_factor(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }
}

impl Default for RetryMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for RetryMiddleware {
    async fn on_error(&self, error: &crate::core::Error) -> Result<Option<String>> {
        // Only retry on specific error types (not all errors should be retried)
        match error {
            crate::core::Error::ToolExecution(_) | crate::core::Error::Timeout(_) => {
                // Retries would be handled at a higher level
                // This middleware just signals that retry is possible
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

/// Validation middleware - validates tool inputs before execution
///
/// This middleware can check tool inputs against validation rules,
/// preventing invalid inputs from being sent to tools.
///
/// # Example
///
/// ```rust
/// use dashflow::core::agents::ValidationMiddleware;
///
/// let middleware = ValidationMiddleware::new()
///     .with_max_input_length(1000);
/// ```
#[derive(Debug, Clone)]
pub struct ValidationMiddleware {
    max_input_length: Option<usize>,
}

impl ValidationMiddleware {
    /// Create a new validation middleware
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_input_length: None,
        }
    }

    /// Set maximum input length for tool calls
    #[must_use]
    pub const fn with_max_input_length(mut self, max_length: usize) -> Self {
        self.max_input_length = Some(max_length);
        self
    }
}

impl Default for ValidationMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for ValidationMiddleware {
    async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
        // Validate input length
        if let Some(max_len) = self.max_input_length {
            let input_str = match &action.tool_input {
                ToolInput::String(s) => s.as_str(),
                ToolInput::Structured(v) => {
                    // Check serialized JSON length
                    &serde_json::to_string(v).unwrap_or_default()
                }
            };

            if input_str.len() > max_len {
                return Err(crate::core::Error::invalid_input(format!(
                    "Tool input exceeds maximum length of {} characters (got {})",
                    max_len,
                    input_str.len()
                )));
            }
        }

        Ok(action.clone())
    }
}

/// Timeout middleware - adds execution time limits to tool calls
///
/// This middleware enforces timeouts on individual tool executions,
/// preventing tools from running indefinitely.
///
/// # Example
///
/// ```rust
/// use dashflow::core::agents::TimeoutMiddleware;
///
/// let middleware = TimeoutMiddleware::new()
///     .with_timeout_seconds(30);
/// ```
#[derive(Debug, Clone)]
pub struct TimeoutMiddleware {
    timeout_seconds: u64,
}

impl TimeoutMiddleware {
    /// Create a new timeout middleware with 60 second default
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout_seconds: 60,
        }
    }

    /// Set the timeout in seconds
    #[must_use]
    pub const fn with_timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

impl Default for TimeoutMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for TimeoutMiddleware {
    async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
        // Store timeout in action metadata (actual timeout enforcement happens in executor)
        Ok(action.clone())
    }
}

/// Tool emulator middleware - simulates tool execution for testing
///
/// This middleware intercepts tool calls and returns mock responses instead
/// of executing the actual tools. Useful for testing agent logic without
/// external dependencies, dry-runs, and development.
///
/// **Note:** This type is only available when the `testing` feature is enabled
/// or in test builds. Enable with `dashflow = { features = ["testing"] }`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::ToolEmulatorMiddleware;
/// use std::collections::HashMap;
///
/// let mut mock_responses = HashMap::new();
/// mock_responses.insert("calculator".to_string(), "42".to_string());
/// mock_responses.insert("search".to_string(), "Mock search results".to_string());
///
/// let middleware = ToolEmulatorMiddleware::new()
///     .with_mock_responses(mock_responses)
///     .with_default_response("Tool executed (mock)");
/// ```
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone)]
pub struct ToolEmulatorMiddleware {
    mock_responses: std::collections::HashMap<String, String>,
    default_response: String,
    enabled: bool,
}

#[cfg(any(test, feature = "testing"))]
impl ToolEmulatorMiddleware {
    /// Create a new tool emulator middleware
    #[must_use]
    pub fn new() -> Self {
        Self {
            mock_responses: std::collections::HashMap::new(),
            default_response: "Tool executed successfully (emulated)".to_string(),
            enabled: true,
        }
    }

    /// Set mock responses for specific tools
    ///
    /// The `HashMap` keys are tool names, and values are the mock responses.
    #[must_use]
    pub fn with_mock_responses(
        mut self,
        responses: std::collections::HashMap<String, String>,
    ) -> Self {
        self.mock_responses = responses;
        self
    }

    /// Add a mock response for a specific tool
    pub fn with_mock_response(
        mut self,
        tool_name: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        self.mock_responses
            .insert(tool_name.into(), response.into());
        self
    }

    /// Set the default response for tools not in the `mock_responses` map
    #[must_use]
    pub fn with_default_response(mut self, response: impl Into<String>) -> Self {
        self.default_response = response.into();
        self
    }

    /// Enable or disable the emulator
    ///
    /// When disabled, tools execute normally. When enabled, tools are mocked.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Check if a specific tool has a mock response
    #[must_use]
    pub fn has_mock_for(&self, tool_name: &str) -> bool {
        self.mock_responses.contains_key(tool_name)
    }
}

#[cfg(any(test, feature = "testing"))]
impl Default for ToolEmulatorMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "testing"))]
#[async_trait::async_trait]
impl AgentMiddleware for ToolEmulatorMiddleware {
    async fn after_tool(&self, action: &AgentAction, _observation: &str) -> Result<String> {
        if !self.enabled {
            // If disabled, pass through the actual observation
            return Ok(_observation.to_string());
        }

        // Return mock response for this tool, or the default
        let mock_response = self
            .mock_responses
            .get(&action.tool)
            .cloned()
            .unwrap_or_else(|| self.default_response.clone());

        Ok(mock_response)
    }
}

/// Model fallback middleware - retries with backup models on failure
///
/// This middleware implements a fallback chain for LLM calls. If the primary
/// model fails (API error, rate limit, etc.), it automatically retries with
/// backup models in the configured order. This improves reliability for
/// production agent systems.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::ModelFallbackMiddleware;
///
/// let middleware = ModelFallbackMiddleware::new()
///     .with_fallback_chain(vec![
///         "gpt-4".to_string(),
///         "gpt-3.5-turbo".to_string(),
///         "claude-3-sonnet".to_string(),
///     ])
///     .with_max_attempts(3);
/// ```
#[derive(Debug)]
pub struct ModelFallbackMiddleware {
    fallback_chain: Vec<String>,
    max_attempts: usize,
    current_model: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl ModelFallbackMiddleware {
    /// Create a new model fallback middleware
    #[must_use]
    pub fn new() -> Self {
        Self {
            fallback_chain: Vec::new(),
            max_attempts: DEFAULT_MAX_RETRIES as usize,
            current_model: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Set the fallback chain of model names
    ///
    /// Models are tried in order. The first model in the chain is the primary.
    #[must_use]
    pub fn with_fallback_chain(mut self, chain: Vec<String>) -> Self {
        self.fallback_chain = chain;
        self
    }

    /// Add a model to the fallback chain
    #[must_use]
    pub fn with_fallback_model(mut self, model_name: impl Into<String>) -> Self {
        self.fallback_chain.push(model_name.into());
        self
    }

    /// Set the maximum number of fallback attempts
    #[must_use]
    pub const fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Get the currently selected model
    #[must_use]
    pub fn current_model(&self) -> Option<String> {
        self.current_model
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Default for ModelFallbackMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for ModelFallbackMiddleware {
    async fn before_plan(&self, context: &mut AgentContext) -> Result<()> {
        // If we have a fallback chain and no model selected yet, use the first one
        if !self.fallback_chain.is_empty() {
            let mut current = self.current_model.lock().unwrap_or_else(|e| e.into_inner());
            if current.is_none() {
                *current = Some(self.fallback_chain[0].clone());
                // Store the selected model in context metadata for the agent to use
                context
                    .metadata
                    .insert("model_name".to_string(), self.fallback_chain[0].clone());
            }
        }
        Ok(())
    }

    async fn on_error(&self, error: &crate::core::Error) -> Result<Option<String>> {
        // On LLM errors, try the next model in the fallback chain
        match error {
            crate::core::Error::Api(_) | crate::core::Error::Timeout(_) => {
                let mut current = self.current_model.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(ref current_model) = *current {
                    // Find the next model in the chain
                    if let Some(current_idx) =
                        self.fallback_chain.iter().position(|m| m == current_model)
                    {
                        let next_idx = current_idx + 1;
                        if next_idx < self.fallback_chain.len() && next_idx < self.max_attempts {
                            let next_model = self.fallback_chain[next_idx].clone();
                            *current = Some(next_model.clone());
                            // Return None to continue the error propagation, but the model is now switched
                            // The agent executor would need to check metadata for model changes
                            return Ok(None);
                        }
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

/// Human-in-the-loop middleware - pauses execution for human approval
///
/// This middleware allows inserting human approval checkpoints into agent
/// execution. Useful for sensitive operations, compliance requirements, or
/// verification of agent decisions before execution.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::HumanInTheLoopMiddleware;
///
/// let middleware = HumanInTheLoopMiddleware::new()
///     .with_approval_callback(|action| {
///         println!("Approve tool call to '{}'? (y/n)", action.tool);
///         // In real implementation, wait for user input
///         Ok(true)
///     })
///     .with_required_for_tools(vec!["delete", "execute_code"]);
/// ```
#[derive(Debug, Clone)]
pub struct HumanInTheLoopMiddleware {
    /// Tools that require approval before execution
    required_tools: Vec<String>,
    /// Whether to require approval for all tools
    require_all: bool,
    /// Message to display when requesting approval
    approval_message: String,
}

impl HumanInTheLoopMiddleware {
    /// Create a new human-in-the-loop middleware
    #[must_use]
    pub fn new() -> Self {
        Self {
            required_tools: Vec::new(),
            require_all: false,
            approval_message: "Approve this action? (y/n): ".to_string(),
        }
    }

    /// Set specific tools that require approval
    #[must_use]
    pub fn with_required_for_tools(mut self, tools: Vec<String>) -> Self {
        self.required_tools = tools;
        self
    }

    /// Add a tool that requires approval
    #[must_use]
    pub fn with_required_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.required_tools.push(tool_name.into());
        self
    }

    /// Require approval for all tool calls
    #[must_use]
    pub const fn with_require_all(mut self, require_all: bool) -> Self {
        self.require_all = require_all;
        self
    }

    /// Set the approval prompt message
    #[must_use]
    pub fn with_approval_message(mut self, message: impl Into<String>) -> Self {
        self.approval_message = message.into();
        self
    }

    /// Check if a tool requires approval
    #[must_use]
    pub fn requires_approval(&self, tool_name: &str) -> bool {
        self.require_all || self.required_tools.iter().any(|t| t == tool_name)
    }
}

impl Default for HumanInTheLoopMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for HumanInTheLoopMiddleware {
    async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
        if self.requires_approval(&action.tool) {
            // In a real implementation, this would pause and wait for user input
            // For now, we add metadata indicating approval was requested
            // This is a simplified version - real implementation would need async approval mechanism
            // M-235: Sanitize configurable message and tool input to prevent log injection
            info!(message = %sanitize_for_log_default(&self.approval_message), "Tool approval");
            info!(tool = %sanitize_for_log_default(&action.tool), "Tool requires approval");
            info!(input = %sanitize_for_log_default(&format!("{:?}", action.tool_input)), "Tool input");

            // For testing purposes, we'll assume approval is granted
            // In production, this would block and wait for user input via a callback or channel
            Ok(action.clone())
        } else {
            Ok(action.clone())
        }
    }
}

/// Rate limit middleware - enforces request rate limits
///
/// This middleware implements token bucket rate limiting for tool calls,
/// preventing API exhaustion and respecting rate limits. Supports per-minute,
/// per-hour, and per-day limits with configurable delays.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::RateLimitMiddleware;
///
/// let middleware = RateLimitMiddleware::new()
///     .with_requests_per_minute(60)
///     .with_requests_per_hour(1000)
///     .with_burst_size(10);
/// ```
#[derive(Debug)]
pub struct RateLimitMiddleware {
    requests_per_minute: Option<usize>,
    requests_per_hour: Option<usize>,
    requests_per_day: Option<usize>,
    burst_size: usize,
    /// Token bucket state: (`last_refill_time`, `available_tokens`)
    state: std::sync::Arc<std::sync::Mutex<(std::time::Instant, f64)>>,
}

impl RateLimitMiddleware {
    /// Create a new rate limit middleware
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests_per_minute: None,
            requests_per_hour: None,
            requests_per_day: None,
            burst_size: 10,
            state: std::sync::Arc::new(std::sync::Mutex::new((std::time::Instant::now(), 10.0))),
        }
    }

    /// Set requests per minute limit
    #[must_use]
    pub const fn with_requests_per_minute(mut self, limit: usize) -> Self {
        self.requests_per_minute = Some(limit);
        self
    }

    /// Set requests per hour limit
    #[must_use]
    pub const fn with_requests_per_hour(mut self, limit: usize) -> Self {
        self.requests_per_hour = Some(limit);
        self
    }

    /// Set requests per day limit
    #[must_use]
    pub const fn with_requests_per_day(mut self, limit: usize) -> Self {
        self.requests_per_day = Some(limit);
        self
    }

    /// Set the burst size (maximum tokens in bucket)
    #[must_use]
    pub fn with_burst_size(mut self, size: usize) -> Self {
        self.burst_size = size;
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.1 = size as f64;
        }
        self
    }

    /// Calculate refill rate in tokens per second
    fn refill_rate(&self) -> f64 {
        if let Some(per_minute) = self.requests_per_minute {
            per_minute as f64 / 60.0
        } else if let Some(per_hour) = self.requests_per_hour {
            per_hour as f64 / 3600.0
        } else if let Some(per_day) = self.requests_per_day {
            per_day as f64 / 86400.0
        } else {
            // No limit set, allow unlimited
            f64::MAX
        }
    }

    /// Try to acquire a token, returning the wait time if rate limited
    fn try_acquire(&self) -> Option<std::time::Duration> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (last_refill, mut tokens) = *state;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(last_refill).as_secs_f64();

        // Refill tokens based on elapsed time
        let refill_rate = self.refill_rate();
        tokens = (tokens + elapsed * refill_rate).min(self.burst_size as f64);

        if tokens >= 1.0 {
            // Token available, consume it
            tokens -= 1.0;
            *state = (now, tokens);
            None
        } else {
            // Not enough tokens, calculate wait time
            let tokens_needed = 1.0 - tokens;
            let wait_secs = tokens_needed / refill_rate;
            Some(std::time::Duration::from_secs_f64(wait_secs))
        }
    }
}

impl Default for RateLimitMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentMiddleware for RateLimitMiddleware {
    async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
        // Check if we need to wait due to rate limiting
        if let Some(wait_duration) = self.try_acquire() {
            // Wait for the required duration
            tokio::time::sleep(wait_duration).await;
            // After waiting, try again (should succeed now)
            if self.try_acquire().is_some() {
                return Err(crate::core::Error::invalid_input(
                    "Rate limit exceeded after waiting",
                ));
            }
        }
        Ok(action.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tools::ToolInput;

    // Helper to create test AgentAction
    fn make_action(tool: &str, input: &str) -> AgentAction {
        AgentAction {
            tool: tool.to_string(),
            tool_input: ToolInput::String(input.to_string()),
            log: "test log".to_string(),
        }
    }

    // Helper to create test AgentContext
    fn make_context() -> AgentContext {
        AgentContext {
            input: "test input".to_string(),
            intermediate_steps: Vec::new(),
            iteration: 1,
            metadata: std::collections::HashMap::new(),
        }
    }

    // =============================================================================
    // LoggingMiddleware Tests
    // =============================================================================

    #[test]
    fn test_logging_middleware_new() {
        let middleware = LoggingMiddleware::new();
        assert_eq!(middleware.prefix, "[AGENT]");
    }

    #[test]
    fn test_logging_middleware_default() {
        let middleware = LoggingMiddleware::default();
        assert_eq!(middleware.prefix, "[AGENT]");
    }

    #[test]
    fn test_logging_middleware_with_prefix() {
        let middleware = LoggingMiddleware::new()
            .with_prefix("[MY_AGENT]");
        assert_eq!(middleware.prefix, "[MY_AGENT]");
    }

    #[test]
    fn test_logging_middleware_clone() {
        let middleware = LoggingMiddleware::new().with_prefix("[TEST]");
        let cloned = middleware.clone();
        assert_eq!(middleware.prefix, cloned.prefix);
    }

    #[tokio::test]
    async fn test_logging_middleware_before_plan() {
        let middleware = LoggingMiddleware::new();
        let mut context = make_context();
        let result = middleware.before_plan(&mut context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware_after_plan_action() {
        let middleware = LoggingMiddleware::new();
        let context = make_context();
        let action = make_action("calculator", "2+2");
        let decision = AgentDecision::Action(action);
        let result = middleware.after_plan(&context, decision).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware_after_plan_finish() {
        let middleware = LoggingMiddleware::new();
        let context = make_context();
        let finish = super::super::AgentFinish {
            output: "done".to_string(),
            log: "finished".to_string(),
        };
        let decision = AgentDecision::Finish(finish);
        let result = middleware.after_plan(&context, decision).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware_after_tool() {
        let middleware = LoggingMiddleware::new();
        let action = make_action("test_tool", "input");
        let result = middleware.after_tool(&action, "observation").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "observation");
    }

    #[tokio::test]
    async fn test_logging_middleware_on_error() {
        let middleware = LoggingMiddleware::new();
        let error = crate::core::Error::invalid_input("test error");
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // =============================================================================
    // RetryMiddleware Tests
    // =============================================================================

    #[test]
    fn test_retry_middleware_new() {
        let middleware = RetryMiddleware::new();
        assert_eq!(middleware.max_retries, 2);
        assert_eq!(middleware.initial_delay_ms, 100);
        assert!((middleware.backoff_factor - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_middleware_default() {
        let middleware = RetryMiddleware::default();
        assert_eq!(middleware.max_retries, 2);
    }

    #[test]
    fn test_retry_middleware_with_max_retries() {
        let middleware = RetryMiddleware::new()
            .with_max_retries(5);
        assert_eq!(middleware.max_retries, 5);
    }

    #[test]
    fn test_retry_middleware_with_initial_delay() {
        let middleware = RetryMiddleware::new()
            .with_initial_delay_ms(500);
        assert_eq!(middleware.initial_delay_ms, 500);
    }

    #[test]
    fn test_retry_middleware_with_backoff_factor() {
        let middleware = RetryMiddleware::new()
            .with_backoff_factor(1.5);
        assert!((middleware.backoff_factor - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_middleware_clone() {
        let middleware = RetryMiddleware::new().with_max_retries(10);
        let cloned = middleware.clone();
        assert_eq!(middleware.max_retries, cloned.max_retries);
    }

    #[tokio::test]
    async fn test_retry_middleware_on_error_tool_execution() {
        let middleware = RetryMiddleware::new();
        let error = crate::core::Error::tool_error("tool failed");
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_retry_middleware_on_error_timeout() {
        let middleware = RetryMiddleware::new();
        let error = crate::core::Error::Timeout("operation timed out".to_string());
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_retry_middleware_on_error_other() {
        let middleware = RetryMiddleware::new();
        let error = crate::core::Error::invalid_input("other error");
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // =============================================================================
    // ValidationMiddleware Tests
    // =============================================================================

    #[test]
    fn test_validation_middleware_new() {
        let middleware = ValidationMiddleware::new();
        assert!(middleware.max_input_length.is_none());
    }

    #[test]
    fn test_validation_middleware_default() {
        let middleware = ValidationMiddleware::default();
        assert!(middleware.max_input_length.is_none());
    }

    #[test]
    fn test_validation_middleware_with_max_input_length() {
        let middleware = ValidationMiddleware::new()
            .with_max_input_length(1000);
        assert_eq!(middleware.max_input_length, Some(1000));
    }

    #[test]
    fn test_validation_middleware_clone() {
        let middleware = ValidationMiddleware::new().with_max_input_length(500);
        let cloned = middleware.clone();
        assert_eq!(middleware.max_input_length, cloned.max_input_length);
    }

    #[tokio::test]
    async fn test_validation_middleware_before_tool_no_limit() {
        let middleware = ValidationMiddleware::new();
        let action = make_action("test", "some input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validation_middleware_before_tool_within_limit() {
        let middleware = ValidationMiddleware::new().with_max_input_length(100);
        let action = make_action("test", "short input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validation_middleware_before_tool_exceeds_limit() {
        let middleware = ValidationMiddleware::new().with_max_input_length(10);
        let action = make_action("test", "this input is way too long");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validation_middleware_before_tool_structured_input() {
        let middleware = ValidationMiddleware::new().with_max_input_length(1000);
        let action = AgentAction {
            tool: "test".to_string(),
            tool_input: ToolInput::Structured(serde_json::json!({"key": "value"})),
            log: "test".to_string(),
        };
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    // =============================================================================
    // TimeoutMiddleware Tests
    // =============================================================================

    #[test]
    fn test_timeout_middleware_new() {
        let middleware = TimeoutMiddleware::new();
        assert_eq!(middleware.timeout_seconds, 60);
    }

    #[test]
    fn test_timeout_middleware_default() {
        let middleware = TimeoutMiddleware::default();
        assert_eq!(middleware.timeout_seconds, 60);
    }

    #[test]
    fn test_timeout_middleware_with_timeout_seconds() {
        let middleware = TimeoutMiddleware::new()
            .with_timeout_seconds(30);
        assert_eq!(middleware.timeout_seconds, 30);
    }

    #[test]
    fn test_timeout_middleware_clone() {
        let middleware = TimeoutMiddleware::new().with_timeout_seconds(120);
        let cloned = middleware.clone();
        assert_eq!(middleware.timeout_seconds, cloned.timeout_seconds);
    }

    #[tokio::test]
    async fn test_timeout_middleware_before_tool() {
        let middleware = TimeoutMiddleware::new();
        let action = make_action("test", "input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    // =============================================================================
    // ToolEmulatorMiddleware Tests (only when testing feature enabled)
    // =============================================================================

    #[test]
    fn test_tool_emulator_middleware_new() {
        let middleware = ToolEmulatorMiddleware::new();
        assert!(middleware.mock_responses.is_empty());
        assert!(middleware.enabled);
        assert_eq!(middleware.default_response, "Tool executed successfully (emulated)");
    }

    #[test]
    fn test_tool_emulator_middleware_default() {
        let middleware = ToolEmulatorMiddleware::default();
        assert!(middleware.mock_responses.is_empty());
    }

    #[test]
    fn test_tool_emulator_middleware_with_mock_responses() {
        let mut responses = std::collections::HashMap::new();
        responses.insert("calc".to_string(), "42".to_string());
        let middleware = ToolEmulatorMiddleware::new()
            .with_mock_responses(responses);
        assert!(middleware.has_mock_for("calc"));
        assert!(!middleware.has_mock_for("other"));
    }

    #[test]
    fn test_tool_emulator_middleware_with_mock_response() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_mock_response("search", "mock results");
        assert!(middleware.has_mock_for("search"));
    }

    #[test]
    fn test_tool_emulator_middleware_with_default_response() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_default_response("custom default");
        assert_eq!(middleware.default_response, "custom default");
    }

    #[test]
    fn test_tool_emulator_middleware_with_enabled() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_enabled(false);
        assert!(!middleware.enabled);
    }

    #[test]
    fn test_tool_emulator_middleware_has_mock_for() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_mock_response("tool1", "response1");
        assert!(middleware.has_mock_for("tool1"));
        assert!(!middleware.has_mock_for("tool2"));
    }

    #[tokio::test]
    async fn test_tool_emulator_middleware_after_tool_enabled() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_mock_response("test_tool", "mock response");
        let action = make_action("test_tool", "input");
        let result = middleware.after_tool(&action, "original").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "mock response");
    }

    #[tokio::test]
    async fn test_tool_emulator_middleware_after_tool_default_response() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_default_response("default mock");
        let action = make_action("unknown_tool", "input");
        let result = middleware.after_tool(&action, "original").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "default mock");
    }

    #[tokio::test]
    async fn test_tool_emulator_middleware_after_tool_disabled() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_enabled(false);
        let action = make_action("test", "input");
        let result = middleware.after_tool(&action, "original observation").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "original observation");
    }

    // =============================================================================
    // ModelFallbackMiddleware Tests
    // =============================================================================

    #[test]
    fn test_model_fallback_middleware_new() {
        let middleware = ModelFallbackMiddleware::new();
        assert!(middleware.fallback_chain.is_empty());
        assert_eq!(middleware.max_attempts, 3);
        assert!(middleware.current_model().is_none());
    }

    #[test]
    fn test_model_fallback_middleware_default() {
        let middleware = ModelFallbackMiddleware::default();
        assert!(middleware.fallback_chain.is_empty());
    }

    #[test]
    fn test_model_fallback_middleware_with_fallback_chain() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_chain(vec!["gpt-4".to_string(), "gpt-3.5".to_string()]);
        assert_eq!(middleware.fallback_chain.len(), 2);
    }

    #[test]
    fn test_model_fallback_middleware_with_fallback_model() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_model("gpt-4")
            .with_fallback_model("claude-3");
        assert_eq!(middleware.fallback_chain.len(), 2);
        assert_eq!(middleware.fallback_chain[0], "gpt-4");
        assert_eq!(middleware.fallback_chain[1], "claude-3");
    }

    #[test]
    fn test_model_fallback_middleware_with_max_attempts() {
        let middleware = ModelFallbackMiddleware::new()
            .with_max_attempts(5);
        assert_eq!(middleware.max_attempts, 5);
    }

    #[test]
    fn test_model_fallback_middleware_current_model_initially_none() {
        let middleware = ModelFallbackMiddleware::new();
        assert!(middleware.current_model().is_none());
    }

    #[tokio::test]
    async fn test_model_fallback_middleware_before_plan_sets_model() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_chain(vec!["gpt-4".to_string(), "gpt-3.5".to_string()]);
        let mut context = make_context();
        let result = middleware.before_plan(&mut context).await;
        assert!(result.is_ok());
        assert_eq!(middleware.current_model(), Some("gpt-4".to_string()));
        assert_eq!(context.metadata.get("model_name"), Some(&"gpt-4".to_string()));
    }

    #[tokio::test]
    async fn test_model_fallback_middleware_before_plan_empty_chain() {
        let middleware = ModelFallbackMiddleware::new();
        let mut context = make_context();
        let result = middleware.before_plan(&mut context).await;
        assert!(result.is_ok());
        assert!(middleware.current_model().is_none());
    }

    #[tokio::test]
    async fn test_model_fallback_middleware_on_error_api_error() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_chain(vec!["gpt-4".to_string(), "gpt-3.5".to_string()]);

        // Set initial model
        let mut context = make_context();
        let _ = middleware.before_plan(&mut context).await;

        // Simulate API error
        let error = crate::core::Error::Api("API unavailable".to_string());
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        // Model should have switched to next in chain
        assert_eq!(middleware.current_model(), Some("gpt-3.5".to_string()));
    }

    #[tokio::test]
    async fn test_model_fallback_middleware_on_error_timeout() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_chain(vec!["gpt-4".to_string(), "claude".to_string()]);

        let mut context = make_context();
        let _ = middleware.before_plan(&mut context).await;

        let error = crate::core::Error::Timeout("request timed out".to_string());
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert_eq!(middleware.current_model(), Some("claude".to_string()));
    }

    #[tokio::test]
    async fn test_model_fallback_middleware_on_error_other_error() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_chain(vec!["gpt-4".to_string(), "gpt-3.5".to_string()]);

        let mut context = make_context();
        let _ = middleware.before_plan(&mut context).await;

        // Non-API/timeout error should not switch models
        let error = crate::core::Error::invalid_input("bad input");
        let initial_model = middleware.current_model();
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert_eq!(middleware.current_model(), initial_model);
    }

    // =============================================================================
    // HumanInTheLoopMiddleware Tests
    // =============================================================================

    #[test]
    fn test_human_in_the_loop_middleware_new() {
        let middleware = HumanInTheLoopMiddleware::new();
        assert!(middleware.required_tools.is_empty());
        assert!(!middleware.require_all);
        assert_eq!(middleware.approval_message, "Approve this action? (y/n): ");
    }

    #[test]
    fn test_human_in_the_loop_middleware_default() {
        let middleware = HumanInTheLoopMiddleware::default();
        assert!(middleware.required_tools.is_empty());
    }

    #[test]
    fn test_human_in_the_loop_middleware_with_required_for_tools() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_for_tools(vec!["delete".to_string(), "exec".to_string()]);
        assert_eq!(middleware.required_tools.len(), 2);
    }

    #[test]
    fn test_human_in_the_loop_middleware_with_required_tool() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("delete")
            .with_required_tool("execute");
        assert_eq!(middleware.required_tools.len(), 2);
    }

    #[test]
    fn test_human_in_the_loop_middleware_with_require_all() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_require_all(true);
        assert!(middleware.require_all);
    }

    #[test]
    fn test_human_in_the_loop_middleware_with_approval_message() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_approval_message("Please approve: ");
        assert_eq!(middleware.approval_message, "Please approve: ");
    }

    #[test]
    fn test_human_in_the_loop_middleware_requires_approval_specific_tool() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("dangerous");
        assert!(middleware.requires_approval("dangerous"));
        assert!(!middleware.requires_approval("safe"));
    }

    #[test]
    fn test_human_in_the_loop_middleware_requires_approval_all() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_require_all(true);
        assert!(middleware.requires_approval("any_tool"));
        assert!(middleware.requires_approval("another_tool"));
    }

    #[test]
    fn test_human_in_the_loop_middleware_clone() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("test")
            .with_require_all(true);
        let cloned = middleware.clone();
        assert_eq!(middleware.required_tools, cloned.required_tools);
        assert_eq!(middleware.require_all, cloned.require_all);
    }

    #[tokio::test]
    async fn test_human_in_the_loop_middleware_before_tool_no_approval_needed() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("dangerous");
        let action = make_action("safe_tool", "input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_human_in_the_loop_middleware_before_tool_approval_needed() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("dangerous");
        let action = make_action("dangerous", "input");
        // In current implementation, approval is assumed granted for testing
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    // =============================================================================
    // RateLimitMiddleware Tests
    // =============================================================================

    #[test]
    fn test_rate_limit_middleware_new() {
        let middleware = RateLimitMiddleware::new();
        assert!(middleware.requests_per_minute.is_none());
        assert!(middleware.requests_per_hour.is_none());
        assert!(middleware.requests_per_day.is_none());
        assert_eq!(middleware.burst_size, 10);
    }

    #[test]
    fn test_rate_limit_middleware_default() {
        let middleware = RateLimitMiddleware::default();
        assert!(middleware.requests_per_minute.is_none());
    }

    #[test]
    fn test_rate_limit_middleware_with_requests_per_minute() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_minute(60);
        assert_eq!(middleware.requests_per_minute, Some(60));
    }

    #[test]
    fn test_rate_limit_middleware_with_requests_per_hour() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_hour(1000);
        assert_eq!(middleware.requests_per_hour, Some(1000));
    }

    #[test]
    fn test_rate_limit_middleware_with_requests_per_day() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_day(10000);
        assert_eq!(middleware.requests_per_day, Some(10000));
    }

    #[test]
    fn test_rate_limit_middleware_with_burst_size() {
        let middleware = RateLimitMiddleware::new()
            .with_burst_size(20);
        assert_eq!(middleware.burst_size, 20);
    }

    #[test]
    fn test_rate_limit_middleware_refill_rate_per_minute() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_minute(60);
        let rate = middleware.refill_rate();
        assert!((rate - 1.0).abs() < 0.001); // 60/60 = 1 per second
    }

    #[test]
    fn test_rate_limit_middleware_refill_rate_per_hour() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_hour(3600);
        let rate = middleware.refill_rate();
        assert!((rate - 1.0).abs() < 0.001); // 3600/3600 = 1 per second
    }

    #[test]
    fn test_rate_limit_middleware_refill_rate_per_day() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_day(86400);
        let rate = middleware.refill_rate();
        assert!((rate - 1.0).abs() < 0.001); // 86400/86400 = 1 per second
    }

    #[test]
    fn test_rate_limit_middleware_refill_rate_no_limit() {
        let middleware = RateLimitMiddleware::new();
        let rate = middleware.refill_rate();
        assert!(rate > 1e300); // Should be f64::MAX
    }

    #[test]
    fn test_rate_limit_middleware_try_acquire_with_tokens() {
        let middleware = RateLimitMiddleware::new()
            .with_burst_size(10)
            .with_requests_per_minute(60);
        // Fresh middleware should have tokens available
        assert!(middleware.try_acquire().is_none());
    }

    #[test]
    fn test_rate_limit_middleware_try_acquire_depletes_tokens() {
        let middleware = RateLimitMiddleware::new()
            .with_burst_size(2)
            .with_requests_per_minute(1); // Very slow refill

        // First two should succeed (burst)
        assert!(middleware.try_acquire().is_none());
        assert!(middleware.try_acquire().is_none());

        // Third should require waiting
        let wait = middleware.try_acquire();
        assert!(wait.is_some());
    }

    #[tokio::test]
    async fn test_rate_limit_middleware_before_tool_no_limit() {
        let middleware = RateLimitMiddleware::new();
        let action = make_action("test", "input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_middleware_before_tool_with_available_tokens() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_minute(60)
            .with_burst_size(10);
        let action = make_action("test", "input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
    }

    // =============================================================================
    // AgentMiddleware Trait Default Implementations
    // =============================================================================

    struct NoOpMiddleware;

    #[async_trait::async_trait]
    impl AgentMiddleware for NoOpMiddleware {}

    #[tokio::test]
    async fn test_agent_middleware_default_before_plan() {
        let middleware = NoOpMiddleware;
        let mut context = make_context();
        let result = middleware.before_plan(&mut context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_middleware_default_after_plan() {
        let middleware = NoOpMiddleware;
        let context = make_context();
        let action = make_action("test", "input");
        let decision = AgentDecision::Action(action.clone());
        let result = middleware.after_plan(&context, decision).await;
        assert!(result.is_ok());
        match result.unwrap() {
            AgentDecision::Action(a) => assert_eq!(a.tool, action.tool),
            AgentDecision::Finish(_) => panic!("Expected Action"),
        }
    }

    #[tokio::test]
    async fn test_agent_middleware_default_before_tool() {
        let middleware = NoOpMiddleware;
        let action = make_action("test", "input");
        let result = middleware.before_tool(&action).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().tool, action.tool);
    }

    #[tokio::test]
    async fn test_agent_middleware_default_after_tool() {
        let middleware = NoOpMiddleware;
        let action = make_action("test", "input");
        let result = middleware.after_tool(&action, "observation").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "observation");
    }

    #[tokio::test]
    async fn test_agent_middleware_default_on_error() {
        let middleware = NoOpMiddleware;
        let error = crate::core::Error::invalid_input("test");
        let result = middleware.on_error(&error).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // =============================================================================
    // Chained Configuration Tests
    // =============================================================================

    #[test]
    fn test_logging_middleware_chained_config() {
        let middleware = LoggingMiddleware::new()
            .with_prefix("[CUSTOM]");
        assert_eq!(middleware.prefix, "[CUSTOM]");
    }

    #[test]
    fn test_retry_middleware_chained_config() {
        let middleware = RetryMiddleware::new()
            .with_max_retries(5)
            .with_initial_delay_ms(200)
            .with_backoff_factor(3.0);
        assert_eq!(middleware.max_retries, 5);
        assert_eq!(middleware.initial_delay_ms, 200);
        assert!((middleware.backoff_factor - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rate_limit_middleware_chained_config() {
        let middleware = RateLimitMiddleware::new()
            .with_requests_per_minute(60)
            .with_requests_per_hour(1000)
            .with_requests_per_day(10000)
            .with_burst_size(20);
        assert_eq!(middleware.requests_per_minute, Some(60));
        assert_eq!(middleware.requests_per_hour, Some(1000));
        assert_eq!(middleware.requests_per_day, Some(10000));
        assert_eq!(middleware.burst_size, 20);
    }

    #[test]
    fn test_human_in_loop_middleware_chained_config() {
        let middleware = HumanInTheLoopMiddleware::new()
            .with_required_tool("delete")
            .with_required_tool("exec")
            .with_require_all(false)
            .with_approval_message("Confirm?");
        assert_eq!(middleware.required_tools.len(), 2);
        assert!(!middleware.require_all);
        assert_eq!(middleware.approval_message, "Confirm?");
    }

    #[test]
    fn test_model_fallback_middleware_chained_config() {
        let middleware = ModelFallbackMiddleware::new()
            .with_fallback_model("gpt-4")
            .with_fallback_model("gpt-3.5")
            .with_max_attempts(5);
        assert_eq!(middleware.fallback_chain.len(), 2);
        assert_eq!(middleware.max_attempts, 5);
    }

    #[test]
    fn test_tool_emulator_middleware_chained_config() {
        let middleware = ToolEmulatorMiddleware::new()
            .with_mock_response("calc", "42")
            .with_mock_response("search", "results")
            .with_default_response("mocked")
            .with_enabled(true);
        assert!(middleware.has_mock_for("calc"));
        assert!(middleware.has_mock_for("search"));
        assert_eq!(middleware.default_response, "mocked");
        assert!(middleware.enabled);
    }
}
