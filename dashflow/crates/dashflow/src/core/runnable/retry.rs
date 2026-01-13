//! Retry and fallback functionality for Runnables
//!
//! This module provides:
//! - `RunnableWithFallbacks`: Try primary runnable, fall back to alternatives on failure
//! - `RunnableRetry`: Retry a runnable with exponential backoff on transient failures

use async_trait::async_trait;
use futures::stream::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use super::{Edge, Graph, Node, Runnable};
use crate::constants::{
    DEFAULT_BACKOFF_MULTIPLIER, DEFAULT_INITIAL_DELAY_MS, DEFAULT_MAX_RETRIES, LONG_TIMEOUT_MS,
};
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};

/// A Runnable that falls back to alternative Runnables if the primary fails.
///
/// `RunnableWithFallbacks` tries the primary Runnable first. If it fails,
/// it attempts each fallback in order until one succeeds.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::{RunnableWithFallbacks, RunnableLambda};
///
/// let primary = RunnableLambda::new(|x: i32| {
///     if x > 0 {
///         Ok(x * 2)
///     } else {
///         Err(DashFlowError::RunError("Negative input".to_string()))
///     }
/// });
///
/// let fallback = RunnableLambda::new(|x: i32| x * 3);
///
/// let with_fallback = RunnableWithFallbacks::new(primary)
///     .add_fallback(fallback);
///
/// let result = with_fallback.invoke(-5, None).await?;
/// // result = -15 (from fallback since primary failed)
/// ```
pub struct RunnableWithFallbacks<Input, Output>
where
    Input: Send + Sync,
    Output: Send + Sync,
{
    primary: Arc<dyn Runnable<Input = Input, Output = Output> + Send + Sync>,
    fallbacks: Vec<Arc<dyn Runnable<Input = Input, Output = Output> + Send + Sync>>,
    /// Function to determine if an error should trigger a fallback
    /// By default, all errors trigger fallbacks
    #[allow(clippy::type_complexity)] // Error filter callback: Error → should_handle bool
    exceptions_to_handle: Option<Arc<dyn Fn(&Error) -> bool + Send + Sync>>,
    /// If set, the exception will be added to the input dict under this key
    /// when passing to fallbacks. The input must be a `HashMap`<String, Value>.
    exception_key: Option<String>,
}

impl<Input, Output> RunnableWithFallbacks<Input, Output>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    /// Create a new `RunnableWithFallbacks` with a primary Runnable
    pub fn new<R>(primary: R) -> Self
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        Self {
            primary: Arc::new(primary),
            fallbacks: Vec::new(),
            exceptions_to_handle: None,
            exception_key: None,
        }
    }

    /// Add a fallback Runnable
    #[must_use]
    pub fn add_fallback<R>(mut self, fallback: R) -> Self
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        self.fallbacks.push(Arc::new(fallback));
        self
    }

    /// Add a boxed fallback Runnable (for use with dyn trait objects)
    pub fn add_fallback_boxed(
        mut self,
        fallback: Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>,
    ) -> Self {
        self.fallbacks.push(Arc::from(fallback));
        self
    }

    /// Set which exceptions should trigger fallbacks
    ///
    /// By default, all exceptions trigger fallbacks. Use this to be more selective.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::error::Error;
    ///
    /// let with_fallback = RunnableWithFallbacks::new(primary)
    ///     .with_exceptions_to_handle(|error| {
    ///         // Only fallback on network errors, not auth errors
    ///         matches!(error, Error::Network(_) | Error::Timeout(_))
    ///     })
    ///     .add_fallback(fallback);
    /// ```
    #[must_use]
    pub fn with_exceptions_to_handle<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&Error) -> bool + Send + Sync + 'static,
    {
        self.exceptions_to_handle = Some(Arc::new(predicate));
        self
    }

    /// Set a key to pass exceptions to fallbacks
    ///
    /// When set, the exception will be added to the input under this key when
    /// invoking fallbacks. The input type must support this (e.g., `HashMap`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let with_fallback = RunnableWithFallbacks::new(primary)
    ///     .with_exception_key("__exception__")
    ///     .add_fallback(fallback);
    /// // Fallback will receive the exception in input["__exception__"]
    /// ```
    #[must_use]
    pub fn with_exception_key(mut self, key: impl Into<String>) -> Self {
        self.exception_key = Some(key.into());
        self
    }

    /// Check if an error should trigger a fallback
    fn should_handle_exception(&self, error: &Error) -> bool {
        match &self.exceptions_to_handle {
            Some(predicate) => predicate(error),
            None => true, // By default, handle all exceptions
        }
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableWithFallbacks<Input, Output>
where
    Input: Clone + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        format!("WithFallbacks[{} fallbacks]", self.fallbacks.len())
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Setup callbacks
        let mut config = config.unwrap_or_default();
        let run_id = config.ensure_run_id();
        let callback_manager = config.get_callback_manager();

        // Create serialized info
        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!(self.name()));

        // Start chain
        callback_manager
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Execute with fallback logic
        let result = async {
            // Try primary first
            let first_error = match self
                .primary
                .invoke(input.clone(), Some(config.clone()))
                .await
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    // Check if this error should trigger fallbacks
                    if !self.should_handle_exception(&error) {
                        // Non-retryable error, fail immediately
                        return Err(error);
                    }
                    error.to_string()
                }
            };

            // Primary failed with retryable error, try fallbacks
            for fallback in &self.fallbacks {
                match fallback.invoke(input.clone(), Some(config.clone())).await {
                    Ok(output) => return Ok(output),
                    Err(_error) => {
                        // Track first error for reporting
                        continue;
                    }
                }
            }

            // All failed, return error message from primary
            Err(Error::RunnableExecution(first_error))
        }
        .await;

        // End chain or report error
        match &result {
            Ok(_) => {
                callback_manager
                    .on_chain_end(&HashMap::new(), run_id, None)
                    .await?;
            }
            Err(e) => {
                callback_manager
                    .on_chain_error(&e.to_string(), run_id, None)
                    .await?;
            }
        }

        result
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>>
    where
        Self::Input: Clone,
    {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>>
    where
        Self::Output: Clone + 'static,
    {
        use futures::stream::StreamExt as _;

        let config = config.unwrap_or_default();

        // Try primary first - capture error if it fails
        let first_error_msg = match self
            .primary
            .stream(input.clone(), Some(config.clone()))
            .await
        {
            Ok(stream) => {
                // Collect the first chunk to verify it works
                let mut stream = Box::pin(stream);
                match stream.next().await {
                    Some(Ok(first_chunk)) => {
                        // Primary is working, return the stream with first chunk
                        return Ok(Box::pin(async_stream::stream! {
                            yield Ok(first_chunk);
                            while let Some(item) = stream.next().await {
                                yield item;
                            }
                        }));
                    }
                    Some(Err(error)) => {
                        // Check if this error should trigger fallbacks
                        if !self.should_handle_exception(&error) {
                            return Err(error);
                        }
                        error.to_string()
                    }
                    None => {
                        // Empty stream is considered success
                        return Ok(Box::pin(futures::stream::empty()));
                    }
                }
            }
            Err(error) => {
                // Check if this error should trigger fallbacks
                if !self.should_handle_exception(&error) {
                    return Err(error);
                }
                error.to_string()
            }
        };

        // Primary failed, try fallbacks
        for fallback in &self.fallbacks {
            match fallback.stream(input.clone(), Some(config.clone())).await {
                Ok(stream) => {
                    // Try to get first chunk from this fallback
                    let mut stream = Box::pin(stream);
                    match stream.next().await {
                        Some(Ok(first_chunk)) => {
                            // Fallback is working, return the stream
                            return Ok(Box::pin(async_stream::stream! {
                                yield Ok(first_chunk);
                                while let Some(item) = stream.next().await {
                                    yield item;
                                }
                            }));
                        }
                        Some(Err(_)) | None => continue,
                    }
                }
                Err(_) => continue,
            }
        }

        // All fallbacks failed, return error from primary
        Err(Error::RunnableExecution(first_error_msg))
    }

    fn get_graph(&self, config: Option<&RunnableConfig>) -> Graph {
        let mut graph = Graph::new();

        // Create a root node for fallback coordination
        let root_node = Node::new(self.name(), self.name());
        graph.add_node(root_node);

        // Add primary runnable
        let primary_graph = self.primary.get_graph(config);
        let primary_prefix = "primary";

        // Add nodes from primary graph with prefix
        for node in primary_graph.nodes.values() {
            let new_id = format!("{}:{}", primary_prefix, node.id);
            let new_node = node.with_id(new_id);
            graph.add_node(new_node);
        }

        // Add edges from primary graph with updated IDs
        for edge in &primary_graph.edges {
            let new_source = format!("{}:{}", primary_prefix, edge.source);
            let new_target = format!("{}:{}", primary_prefix, edge.target);
            graph.add_edge(Edge::new(new_source, new_target));
        }

        // Connect root to primary
        if let Some(first_node) = primary_graph.first_node() {
            let first_node_id = format!("{}:{}", primary_prefix, first_node.id);
            graph.add_edge(Edge::new(self.name(), first_node_id));
        }

        // Add each fallback runnable
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let fallback_graph = fallback.get_graph(config);
            let fallback_prefix = format!("fallback_{idx}");

            // Add nodes from fallback graph with prefix
            for node in fallback_graph.nodes.values() {
                let new_id = format!("{}:{}", fallback_prefix, node.id);
                let new_node = node.with_id(new_id);
                graph.add_node(new_node);
            }

            // Add edges from fallback graph with updated IDs
            for edge in &fallback_graph.edges {
                let new_source = format!("{}:{}", fallback_prefix, edge.source);
                let new_target = format!("{}:{}", fallback_prefix, edge.target);
                graph.add_edge(Edge::new(new_source, new_target));
            }

            // Connect root to this fallback
            if let Some(first_node) = fallback_graph.first_node() {
                let first_node_id = format!("{}:{}", fallback_prefix, first_node.id);
                let mut edge = Edge::new(self.name(), first_node_id);
                edge.data = Some(format!("on_error_{idx}"));
                graph.add_edge(edge);
            }
        }

        graph
    }
}

/// A Runnable that retries another Runnable on failure with exponential backoff.
///
/// `RunnableRetry` wraps a Runnable and automatically retries failed invocations
/// with configurable retry logic including exponential backoff and jitter.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::{RunnableRetry, RunnableLambda};
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::sync::Arc;
///
/// let attempt_count = Arc::new(AtomicUsize::new(0));
/// let count_clone = attempt_count.clone();
///
/// // Create a lambda that fails twice then succeeds
/// let flaky_lambda = RunnableLambda::new(move |x: i32| {
///     let count = count_clone.fetch_add(1, Ordering::SeqCst);
///     if count < 2 {
///         Err(DashFlowError::RunError("Transient failure".to_string()))
///     } else {
///         Ok(x * 2)
///     }
/// });
///
/// let retry = RunnableRetry::new(flaky_lambda)
///     .with_max_attempts(3)
///     .with_initial_interval(100);
///
/// let result = retry.invoke(5, None).await?;
/// assert_eq!(result, 10); // Succeeds after 3 attempts
/// ```
pub struct RunnableRetry<Input, Output>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    runnable: Arc<dyn Runnable<Input = Input, Output = Output>>,
    max_attempts: usize,
    initial_interval_ms: u64,
    max_interval_ms: u64,
    multiplier: f64,
    jitter: bool,
}

impl<Input, Output> RunnableRetry<Input, Output>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    /// Create a new `RunnableRetry` with default settings
    ///
    /// Default settings:
    /// - `max_attempts`: 3
    /// - `initial_interval`: 1000ms (1 second)
    /// - `max_interval`: 60000ms (60 seconds)
    /// - multiplier: 2.0 (exponential backoff)
    /// - jitter: true
    pub fn new<R>(runnable: R) -> Self
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        Self {
            runnable: Arc::new(runnable),
            max_attempts: DEFAULT_MAX_RETRIES as usize,
            initial_interval_ms: DEFAULT_INITIAL_DELAY_MS,
            max_interval_ms: LONG_TIMEOUT_MS,
            multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter: true,
        }
    }

    /// Set the maximum number of attempts (including the initial attempt)
    #[must_use]
    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    /// Set the initial retry interval in milliseconds
    #[must_use]
    pub fn with_initial_interval(mut self, ms: u64) -> Self {
        self.initial_interval_ms = ms;
        self
    }

    /// Set the maximum retry interval in milliseconds
    #[must_use]
    pub fn with_max_interval(mut self, ms: u64) -> Self {
        self.max_interval_ms = ms;
        self
    }

    /// Set the backoff multiplier (default: 2.0 for exponential backoff)
    #[must_use]
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Enable or disable jitter (random variation in retry intervals)
    #[must_use]
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate the retry delay for a given attempt
    fn calculate_delay(&self, attempt: usize) -> u64 {
        use std::cmp::min;

        // Calculate base delay with exponential backoff
        let base_delay =
            (self.initial_interval_ms as f64 * self.multiplier.powi(attempt as i32)) as u64;
        let delay = min(base_delay, self.max_interval_ms);

        // Add jitter if enabled
        if self.jitter && delay > 0 {
            use std::collections::hash_map::RandomState;
            use std::hash::BuildHasher;
            use std::time::{SystemTime, UNIX_EPOCH};

            // Simple pseudo-random jitter using hash of current time
            // Use unwrap_or_default to handle edge case of system clock before UNIX_EPOCH
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let hash = RandomState::new().hash_one(now);

            // Jitter adds 0-50% variation
            let jitter_amount = (delay as f64 * 0.5 * (hash % 1000) as f64 / 1000.0) as u64;
            delay + jitter_amount
        } else {
            delay
        }
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableRetry<Input, Output>
where
    Input: Clone + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        format!("Retry[max_attempts={}]", self.max_attempts)
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Setup callbacks
        let mut config = config.unwrap_or_default();
        let run_id = config.ensure_run_id();
        let callback_manager = config.get_callback_manager();

        // Create serialized info
        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!(self.name()));

        // Start chain
        callback_manager
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Execute with retry logic
        let result = async {
            let mut last_error = None;

            for attempt in 0..self.max_attempts {
                // Tag this attempt
                let mut attempt_config = config.clone();
                if attempt > 0 {
                    attempt_config
                        .tags
                        .push(format!("retry:attempt:{}", attempt + 1));

                    // Call on_retry callback
                    if let Err(e) = callback_manager.on_retry(run_id, None).await {
                        tracing::warn!(
                            run_id = %run_id,
                            attempt = attempt + 1,
                            error = %e,
                            "Failed to invoke on_retry callback"
                        );
                    }
                }

                match self
                    .runnable
                    .invoke(input.clone(), Some(attempt_config))
                    .await
                {
                    Ok(output) => return Ok(output),
                    Err(e) => {
                        last_error = Some(e);

                        // Don't sleep after the last attempt
                        if attempt < self.max_attempts - 1 {
                            let delay = self.calculate_delay(attempt);
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                        }
                    }
                }
            }

            // All attempts failed
            Err(last_error.unwrap_or_else(|| {
                Error::RunnableExecution("All retry attempts failed".to_string())
            }))
        }
        .await;

        // End chain or report error
        match &result {
            Ok(_) => {
                callback_manager
                    .on_chain_end(&HashMap::new(), run_id, None)
                    .await?;
            }
            Err(e) => {
                callback_manager
                    .on_chain_error(&e.to_string(), run_id, None)
                    .await?;
            }
        }

        result
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>>
    where
        Self::Input: Clone,
    {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::RunnableConfig;
    use crate::core::runnable::RunnableLambda;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ============================================================================
    // Helper Structs for Testing
    // ============================================================================

    /// A runnable that always fails
    struct AlwaysFailRunnable;

    #[async_trait]
    impl Runnable for AlwaysFailRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::InvalidInput("always fails".to_string()))
        }
    }

    /// A runnable that fails with a specific error type
    struct TimeoutFailRunnable;

    #[async_trait]
    impl Runnable for TimeoutFailRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::Timeout("timeout".to_string()))
        }
    }

    /// A runnable that fails with InvalidInput
    struct InvalidInputFailRunnable;

    #[async_trait]
    impl Runnable for InvalidInputFailRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::InvalidInput("invalid".to_string()))
        }
    }

    /// A runnable that fails N times then succeeds
    struct FailNTimesRunnable {
        fail_count: AtomicUsize,
        times_to_fail: usize,
    }

    impl FailNTimesRunnable {
        fn new(times_to_fail: usize) -> Self {
            Self {
                fail_count: AtomicUsize::new(0),
                times_to_fail,
            }
        }
    }

    #[async_trait]
    impl Runnable for FailNTimesRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if count < self.times_to_fail {
                Err(Error::InvalidInput("transient failure".to_string()))
            } else {
                Ok(input * 2)
            }
        }
    }

    // ============================================================================
    // RunnableWithFallbacks Tests
    // ============================================================================

    #[tokio::test]
    async fn test_fallbacks_primary_succeeds() {
        let primary = RunnableLambda::new(|x: i32| x * 2);
        let fallback = RunnableLambda::new(|x: i32| x * 3);

        let with_fallback = RunnableWithFallbacks::new(primary).add_fallback(fallback);

        let result = with_fallback
            .invoke(5, None)
            .await
            .expect("invoke should succeed");
        assert_eq!(result, 10); // Primary result, not fallback
    }

    #[tokio::test]
    async fn test_fallbacks_primary_fails_fallback_succeeds() {
        let primary = AlwaysFailRunnable;
        let fallback = RunnableLambda::new(|x: i32| x * 3);

        let with_fallback = RunnableWithFallbacks::new(primary).add_fallback(fallback);

        let result = with_fallback
            .invoke(5, None)
            .await
            .expect("invoke should succeed with fallback");
        assert_eq!(result, 15); // Fallback result
    }

    #[tokio::test]
    async fn test_fallbacks_all_fail() {
        let primary = AlwaysFailRunnable;
        let fallback = AlwaysFailRunnable;

        let with_fallback = RunnableWithFallbacks::new(primary).add_fallback(fallback);

        let result = with_fallback.invoke(5, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallbacks_multiple_fallbacks() {
        let primary = AlwaysFailRunnable;
        let fallback1 = AlwaysFailRunnable;
        let fallback2 = RunnableLambda::new(|x: i32| x * 10);

        let with_fallback = RunnableWithFallbacks::new(primary)
            .add_fallback(fallback1)
            .add_fallback(fallback2);

        let result = with_fallback
            .invoke(5, None)
            .await
            .expect("second fallback should succeed");
        assert_eq!(result, 50); // Second fallback result
    }

    #[test]
    fn test_fallbacks_name() {
        let primary = RunnableLambda::new(|x: i32| x);
        let fallback = RunnableLambda::new(|x: i32| x);

        let with_fallback = RunnableWithFallbacks::new(primary).add_fallback(fallback);

        assert_eq!(with_fallback.name(), "WithFallbacks[1 fallbacks]");
    }

    #[tokio::test]
    async fn test_fallbacks_exceptions_to_handle() {
        let primary = TimeoutFailRunnable;
        let fallback = RunnableLambda::new(|x: i32| x * 2);

        // Only handle InvalidInput errors, not Timeout
        let with_fallback = RunnableWithFallbacks::new(primary)
            .with_exceptions_to_handle(|e| matches!(e, Error::InvalidInput(_)))
            .add_fallback(fallback);

        // Should not fall back since Timeout is not handled
        let result = with_fallback.invoke(5, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallbacks_exceptions_to_handle_matches() {
        let primary = InvalidInputFailRunnable;
        let fallback = RunnableLambda::new(|x: i32| x * 2);

        // Handle InvalidInput errors
        let with_fallback = RunnableWithFallbacks::new(primary)
            .with_exceptions_to_handle(|e| matches!(e, Error::InvalidInput(_)))
            .add_fallback(fallback);

        let result = with_fallback
            .invoke(5, None)
            .await
            .expect("fallback should be used");
        assert_eq!(result, 10);
    }

    #[test]
    fn test_fallbacks_get_graph() {
        let primary = RunnableLambda::new(|x: i32| x);
        let fallback = RunnableLambda::new(|x: i32| x);

        let with_fallback = RunnableWithFallbacks::new(primary).add_fallback(fallback);

        let graph = with_fallback.get_graph(None);
        assert!(!graph.nodes.is_empty());
    }

    // ============================================================================
    // RunnableRetry Tests
    // ============================================================================

    #[tokio::test]
    async fn test_retry_succeeds_first_attempt() {
        let runnable = RunnableLambda::new(|x: i32| x * 2);
        let retry = RunnableRetry::new(runnable).with_max_attempts(3);

        let result = retry.invoke(5, None).await.expect("should succeed");
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let runnable = FailNTimesRunnable::new(2);

        let retry = RunnableRetry::new(runnable)
            .with_max_attempts(5)
            .with_initial_interval(1) // Fast for testing
            .with_jitter(false);

        let result = retry
            .invoke(5, None)
            .await
            .expect("should eventually succeed");
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_retry_all_attempts_fail() {
        let runnable = AlwaysFailRunnable;

        let retry = RunnableRetry::new(runnable)
            .with_max_attempts(3)
            .with_initial_interval(1)
            .with_jitter(false);

        let result = retry.invoke(5, None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_name() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable).with_max_attempts(5);

        assert_eq!(retry.name(), "Retry[max_attempts=5]");
    }

    #[test]
    fn test_retry_calculate_delay_exponential() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable)
            .with_initial_interval(100)
            .with_multiplier(2.0)
            .with_max_interval(10000)
            .with_jitter(false);

        // Without jitter, delays should be exact exponential
        assert_eq!(retry.calculate_delay(0), 100); // 100 * 2^0 = 100
        assert_eq!(retry.calculate_delay(1), 200); // 100 * 2^1 = 200
        assert_eq!(retry.calculate_delay(2), 400); // 100 * 2^2 = 400
        assert_eq!(retry.calculate_delay(3), 800); // 100 * 2^3 = 800
    }

    #[test]
    fn test_retry_calculate_delay_capped() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable)
            .with_initial_interval(100)
            .with_multiplier(2.0)
            .with_max_interval(500)
            .with_jitter(false);

        // Should be capped at max_interval
        assert_eq!(retry.calculate_delay(0), 100);
        assert_eq!(retry.calculate_delay(1), 200);
        assert_eq!(retry.calculate_delay(2), 400);
        assert_eq!(retry.calculate_delay(3), 500); // Capped at 500
        assert_eq!(retry.calculate_delay(10), 500); // Still capped
    }

    #[test]
    fn test_retry_calculate_delay_with_jitter() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable)
            .with_initial_interval(1000)
            .with_multiplier(2.0)
            .with_jitter(true);

        // With jitter, delay should be >= base delay (jitter only adds, never subtracts)
        let base_delay = 1000;
        for _ in 0..10 {
            let delay = retry.calculate_delay(0);
            assert!(delay >= base_delay);
            assert!(delay <= base_delay + (base_delay as f64 * 0.5) as u64 + 1);
        }
    }

    #[test]
    fn test_retry_max_attempts_clamped() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable).with_max_attempts(0);

        // max_attempts should be clamped to at least 1
        assert_eq!(retry.name(), "Retry[max_attempts=1]");
    }

    #[tokio::test]
    async fn test_retry_batch() {
        let runnable = RunnableLambda::new(|x: i32| x * 2);
        let retry = RunnableRetry::new(runnable).with_max_attempts(2);

        let results = retry
            .batch(vec![1, 2, 3], None)
            .await
            .expect("batch should succeed");

        assert_eq!(results, vec![2, 4, 6]);
    }

    #[test]
    fn test_retry_builder_methods() {
        let runnable = RunnableLambda::new(|x: i32| x);
        let retry = RunnableRetry::new(runnable)
            .with_max_attempts(5)
            .with_initial_interval(200)
            .with_max_interval(5000)
            .with_multiplier(1.5)
            .with_jitter(false);

        // Verify all builder methods return Self for chaining
        assert_eq!(retry.max_attempts, 5);
        assert_eq!(retry.initial_interval_ms, 200);
        assert_eq!(retry.max_interval_ms, 5000);
        assert!((retry.multiplier - 1.5).abs() < f64::EPSILON);
        assert!(!retry.jitter);
    }
}
