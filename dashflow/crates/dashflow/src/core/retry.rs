// Allow clippy warnings for retry logic
// - panic: panic!() in config validation to catch invalid configuration at startup
// - expect_used: expect() on backoff calculations with validated inputs
#![allow(clippy::panic, clippy::expect_used)]

//! Retry logic for LLM API calls with exponential backoff.
//!
//! This module provides utilities for retrying failed API calls with configurable
//! backoff strategies. It is designed to handle transient failures like network
//! issues, rate limits, and temporary service unavailability.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::retry::{RetryPolicy, with_retry};
//!
//! let policy = RetryPolicy::exponential(3); // 3 retries with exponential backoff
//! let result = with_retry(policy, || async {
//!     // API call that might fail
//!     api_client.generate("prompt").await
//! }).await?;
//! ```

use crate::constants::{
    DEFAULT_BACKOFF_MULTIPLIER, DEFAULT_INITIAL_DELAY_MS, DEFAULT_JITTER_MS, DEFAULT_MAX_DELAY_MS,
    DEFAULT_MAX_RETRIES, MAX_RETRIES_LIMIT,
};
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};
use crate::core::rate_limiters::RateLimiter;
use crate::core::runnable::Runnable;
use std::sync::Arc;
use std::time::Duration;

/// Retry policy configuration for API calls.
///
/// Defines how many times to retry and what backoff strategy to use.
/// Optionally includes a rate limiter to prevent API quota exhaustion
/// when many concurrent operations retry simultaneously.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retry::{RetryPolicy, RetryStrategy};
///
/// // Simple exponential backoff with 3 retries
/// let policy = RetryPolicy::exponential(3);
///
/// // Custom configuration with jitter (recommended for production)
/// let policy = RetryPolicy::exponential_jitter(5)
///     .with_initial_delay_ms(500)
///     .with_max_delay_ms(30000);
///
/// // Fixed interval (useful for testing)
/// let policy = RetryPolicy::fixed(3, 1000); // 3 retries, 1s apart
///
/// // With rate limiter to prevent quota exhaustion
/// use std::sync::Arc;
/// let limiter = Arc::new(dashflow::core::rate_limiters::TokenBucketLimiter::new(10, 1.0));
/// let policy = RetryPolicy::exponential(3).with_rate_limiter(limiter);
///
/// // Use with_retry helper
/// use dashflow::core::retry::with_retry;
/// let result = with_retry(policy, || async {
///     api_client.generate("prompt").await
/// }).await?;
/// ```
///
/// # See Also
///
/// - [`RetryStrategy`] - Backoff strategies (exponential, jitter, fixed)
/// - [`with_retry`] - Execute a closure with retry logic
/// - [`RunnableConfig`] - Configure retries per node
/// - [`CircuitBreaker`](crate::self_improvement::CircuitBreaker) - For failure-based protection
#[derive(Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_retries: usize,

    /// Backoff strategy
    pub strategy: RetryStrategy,

    /// Optional rate limiter to prevent API quota exhaustion.
    ///
    /// When set, each retry attempt will acquire permission from the rate limiter
    /// before making the API call. This prevents thundering herd scenarios where
    /// many concurrent operations all retry at the same time.
    pub rate_limiter: Option<Arc<dyn RateLimiter>>,
}

/// Backoff strategy for retries.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RetryStrategy {
    /// Exponential backoff: wait time doubles after each retry
    Exponential {
        /// Initial delay in milliseconds
        initial_delay_ms: u64,
        /// Maximum delay in milliseconds
        max_delay_ms: u64,
        /// Multiplier for each retry (typically 2.0)
        multiplier: u64,
    },

    /// Exponential backoff with jitter: adds randomness to prevent thundering herd
    ExponentialJitter {
        /// Initial delay in milliseconds
        initial_delay_ms: u64,
        /// Maximum delay in milliseconds
        max_delay_ms: u64,
        /// Base for exponential calculation (typically 2.0)
        exp_base: f64,
        /// Maximum random jitter to add (in milliseconds)
        jitter_ms: u64,
    },

    /// Fixed interval: wait the same amount of time between retries
    Fixed {
        /// Delay between retries in milliseconds
        delay_ms: u64,
    },
}

impl Default for RetryPolicy {
    /// Default retry policy with jitter to prevent thundering herd.
    ///
    /// Uses exponential backoff with jitter (M-195 fix):
    /// - DEFAULT_MAX_RETRIES (3) retries max
    /// - 1s initial delay, 10s max delay
    /// - 2x exponential base
    /// - Up to 1s random jitter
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES as usize,
            strategy: RetryStrategy::ExponentialJitter {
                initial_delay_ms: DEFAULT_INITIAL_DELAY_MS, // 1 second
                max_delay_ms: DEFAULT_MAX_DELAY_MS,         // 10 seconds
                exp_base: DEFAULT_BACKOFF_MULTIPLIER,       // 2.0
                jitter_ms: DEFAULT_JITTER_MS,               // 1 second
            },
            rate_limiter: None,
        }
    }
}

impl std::fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryPolicy")
            .field("max_retries", &self.max_retries)
            .field("strategy", &self.strategy)
            .field(
                "rate_limiter",
                &self.rate_limiter.as_ref().map(|_| "<RateLimiter>"),
            )
            .finish()
    }
}

impl RetryPolicy {
    /// Validate the retry policy configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `max_retries` is greater than 1000 (prevents runaway retry loops)
    /// - Strategy delays are invalid (initial > max, zero multiplier, etc.)
    ///
    /// Note: `max_retries` of 0 is allowed (means no retries, fail immediately).
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.max_retries > MAX_RETRIES_LIMIT as usize {
            return Err(format!(
                "max_retries too large: {} (max {})",
                self.max_retries, MAX_RETRIES_LIMIT
            ));
        }

        match &self.strategy {
            RetryStrategy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                multiplier,
            } => {
                if *multiplier == 0 {
                    return Err("exponential multiplier must be at least 1".to_string());
                }
                if *initial_delay_ms > *max_delay_ms {
                    return Err(format!(
                        "initial_delay_ms ({}) must not exceed max_delay_ms ({})",
                        initial_delay_ms, max_delay_ms
                    ));
                }
            }
            RetryStrategy::ExponentialJitter {
                initial_delay_ms,
                max_delay_ms,
                exp_base,
                ..
            } => {
                if *exp_base <= 0.0 {
                    return Err(format!("exp_base must be positive, got {}", exp_base));
                }
                if *initial_delay_ms > *max_delay_ms {
                    return Err(format!(
                        "initial_delay_ms ({}) must not exceed max_delay_ms ({})",
                        initial_delay_ms, max_delay_ms
                    ));
                }
            }
            RetryStrategy::Fixed { .. } => {
                // Fixed delay has no invalid configurations
            }
        }

        Ok(())
    }

    /// Create a retry policy with exponential backoff.
    ///
    /// Uses default values:
    /// - Initial delay: 1 second
    /// - Max delay: 10 seconds
    /// - Multiplier: 2x
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    #[must_use]
    pub fn exponential(max_retries: usize) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Add a rate limiter to this policy.
    ///
    /// The rate limiter will be called before each retry attempt to prevent
    /// API quota exhaustion when many concurrent operations retry simultaneously.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::retry::RetryPolicy;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::time::Duration;
    /// use std::sync::Arc;
    ///
    /// // 10 requests per second with burst capacity of 20
    /// let limiter = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));
    ///
    /// let policy = RetryPolicy::exponential(3)
    ///     .with_rate_limiter(limiter);
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Create a retry policy with custom exponential backoff parameters.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `initial_delay_ms` - Initial delay in milliseconds
    /// * `max_delay_ms` - Maximum delay in milliseconds
    #[must_use]
    pub fn exponential_with_params(
        max_retries: usize,
        initial_delay_ms: u64,
        max_delay_ms: u64,
    ) -> Self {
        Self {
            max_retries,
            strategy: RetryStrategy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                multiplier: 2,
            },
            rate_limiter: None,
        }
    }

    /// Create a retry policy with fixed interval backoff.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `delay_ms` - Fixed delay between retries in milliseconds
    #[must_use]
    pub fn fixed(max_retries: usize, delay_ms: u64) -> Self {
        Self {
            max_retries,
            strategy: RetryStrategy::Fixed { delay_ms },
            rate_limiter: None,
        }
    }

    /// Create a policy with no retries (fail immediately).
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            strategy: RetryStrategy::Fixed { delay_ms: 0 },
            rate_limiter: None,
        }
    }

    /// Create a retry policy with exponential backoff and jitter.
    ///
    /// Jitter adds randomness to prevent thundering herd problem where
    /// many clients retry at the same time.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `initial_delay_ms` - Initial delay in milliseconds
    /// * `max_delay_ms` - Maximum delay in milliseconds
    /// * `exp_base` - Base for exponential calculation (typically 2.0)
    /// * `jitter_ms` - Maximum random jitter to add (in milliseconds)
    #[must_use]
    pub fn exponential_jitter(
        max_retries: usize,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        exp_base: f64,
        jitter_ms: u64,
    ) -> Self {
        Self {
            max_retries,
            strategy: RetryStrategy::ExponentialJitter {
                initial_delay_ms,
                max_delay_ms,
                exp_base,
                jitter_ms,
            },
            rate_limiter: None,
        }
    }

    /// Create a retry policy with exponential jitter using default parameters.
    ///
    /// Defaults:
    /// - Initial delay: 1000ms (1 second)
    /// - Max delay: 10000ms (10 seconds)
    /// - Exp base: 2.0
    /// - Jitter: 1000ms (1 second)
    #[must_use]
    pub fn default_jitter(max_retries: usize) -> Self {
        Self::exponential_jitter(
            max_retries,
            DEFAULT_INITIAL_DELAY_MS,
            DEFAULT_MAX_DELAY_MS,
            DEFAULT_BACKOFF_MULTIPLIER,
            DEFAULT_JITTER_MS,
        )
    }

    /// Check if an error is retryable.
    ///
    /// Currently retries on:
    /// - Network errors
    /// - Timeout errors
    /// - Rate limit errors (with backoff)
    ///
    /// Does NOT retry on:
    /// - Invalid input errors
    /// - Authentication errors
    /// - Not found errors
    #[must_use]
    pub fn is_retryable(error: &Error) -> bool {
        matches!(
            error,
            Error::Network(_) | Error::Timeout(_) | Error::RateLimit(_)
        )
    }
}

/// Execute an async operation with retry logic.
///
/// Retries the operation according to the retry policy if it fails with a retryable error.
///
/// # Arguments
///
/// * `policy` - The retry policy to use
/// * `operation` - The async operation to retry
///
/// # Returns
///
/// The result of the operation, or the last error if all retries failed
///
/// # Example
///
/// ```rust,ignore
/// let result = with_retry(&RetryPolicy::exponential(3), || async {
///     api_call().await
/// }).await?;
/// ```
pub async fn with_retry<F, Fut, T>(policy: &RetryPolicy, operation: F) -> Result<T>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<T>> + Send,
{
    if policy.max_retries == 0 {
        return operation().await;
    }

    // Build retry delays based on strategy
    let mut delays = Vec::new();
    match &policy.strategy {
        RetryStrategy::Exponential {
            initial_delay_ms,
            max_delay_ms,
            multiplier,
        } => {
            let mut delay = *initial_delay_ms;
            for _ in 0..=policy.max_retries {
                delays.push(Duration::from_millis(delay));
                delay = (delay * multiplier).min(*max_delay_ms);
            }
        }
        RetryStrategy::ExponentialJitter {
            initial_delay_ms,
            max_delay_ms,
            exp_base,
            jitter_ms,
        } => {
            use rand::Rng;
            let mut rng = rand::thread_rng();

            for attempt in 0..=policy.max_retries {
                // Calculate exponential delay: initial * (exp_base ^ attempt)
                let exp_delay = (*initial_delay_ms as f64) * exp_base.powi(attempt as i32);
                let base_delay = exp_delay.min(*max_delay_ms as f64) as u64;

                // Add random jitter: base_delay + random(0, jitter_ms)
                let jitter = rng.gen_range(0..=*jitter_ms);
                let total_delay = base_delay + jitter;

                delays.push(Duration::from_millis(total_delay.min(*max_delay_ms)));
            }
        }
        RetryStrategy::Fixed { delay_ms } => {
            for _ in 0..=policy.max_retries {
                delays.push(Duration::from_millis(*delay_ms));
            }
        }
    }

    let mut last_error = None;

    for (attempt, delay) in delays.iter().enumerate() {
        if attempt > 0 {
            tokio::time::sleep(*delay).await;
        }

        // Acquire rate limiter permission before making the API call.
        // This prevents thundering herd when many concurrent operations retry.
        if let Some(ref limiter) = policy.rate_limiter {
            limiter.acquire().await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if !RetryPolicy::is_retryable(&err) {
                    // Non-retryable error, return immediately
                    return Err(err);
                }
                last_error = Some(err);
            }
        }
    }

    // All retries exhausted - last_error is always set because delays always has at least one element
    Err(last_error.expect("internal error: retry loop completed without capturing an error"))
}

/// A Runnable wrapper that adds retry logic to another Runnable.
///
/// This wraps any Runnable and automatically retries it according to the
/// configured retry policy when it encounters retryable errors.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retry::{RunnableRetry, RetryPolicy};
/// use dashflow::core::runnable::Runnable;
///
/// let model = /* some Runnable */;
/// let policy = RetryPolicy::default_jitter(3);
/// let retryable = RunnableRetry::new(model, policy);
///
/// // Or use the with_retry() method:
/// let retryable = model.with_retry();
/// ```
pub struct RunnableRetry<R>
where
    R: Runnable,
{
    /// The underlying Runnable to retry
    bound: Arc<R>,
    /// The retry policy to use
    policy: RetryPolicy,
}

impl<R> RunnableRetry<R>
where
    R: Runnable,
{
    /// Create a new `RunnableRetry` wrapper.
    ///
    /// # Arguments
    ///
    /// * `bound` - The Runnable to wrap with retry logic
    /// * `policy` - The retry policy to use
    pub fn new(bound: R, policy: RetryPolicy) -> Self {
        Self {
            bound: Arc::new(bound),
            policy,
        }
    }

    /// Create a `RunnableRetry` with default jitter settings.
    ///
    /// Uses 3 retries with exponential backoff and jitter.
    #[must_use]
    pub fn with_defaults(bound: R) -> Self {
        Self::new(bound, RetryPolicy::default_jitter(3))
    }
}

#[async_trait::async_trait]
impl<R> Runnable for RunnableRetry<R>
where
    R: Runnable,
    R::Input: Clone + Sync,
    R::Output: Sync,
{
    type Input = R::Input;
    type Output = R::Output;

    fn name(&self) -> String {
        format!("RunnableRetry<{}>", self.bound.name())
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        let bound = Arc::clone(&self.bound);
        let config_clone = config.clone();

        with_retry(&self.policy, || {
            let bound = Arc::clone(&bound);
            let input = input.clone();
            let config = config_clone.clone();
            async move { bound.invoke(input, config).await }
        })
        .await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        // For batch, we retry each input individually
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::sync::{Arc, Mutex};
    // Override test_prelude's RunnableRetry with this module's version
    use super::RunnableRetry;

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[tokio::test]
    async fn test_retry_policy_exponential() {
        let policy = RetryPolicy::exponential(3);
        assert_eq!(policy.max_retries, 3);
        matches!(policy.strategy, RetryStrategy::Exponential { .. });
    }

    #[tokio::test]
    async fn test_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(5, 100);
        assert_eq!(policy.max_retries, 5);
        matches!(policy.strategy, RetryStrategy::Fixed { delay_ms: 100 });
    }

    #[tokio::test]
    async fn test_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        assert_eq!(policy.max_retries, 0);
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_first_attempt() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(3), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Ok::<i32, Error>(42)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*lock(&counter), 1); // Only called once
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_after_failures() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(3), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 3 {
                    Err(Error::Network("Transient network error".to_string()))
                } else {
                    Ok::<i32, Error>(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*lock(&counter), 3); // Called 3 times
    }

    #[tokio::test]
    async fn test_with_retry_all_fail() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(3), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Network error".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // Should be called max_retries + 1 times (initial + retries)
        assert_eq!(*lock(&counter), 4);
    }

    #[tokio::test]
    async fn test_with_retry_non_retryable_error() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(3), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::InvalidInput("Bad input".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // Should only be called once (non-retryable error)
        assert_eq!(*lock(&counter), 1);
    }

    #[tokio::test]
    async fn test_no_retry_policy() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::no_retry(), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Network error".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(*lock(&counter), 1); // Only called once
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors
        assert!(RetryPolicy::is_retryable(&Error::Network(
            "test".to_string()
        )));
        assert!(RetryPolicy::is_retryable(&Error::Timeout(
            "test".to_string()
        )));
        assert!(RetryPolicy::is_retryable(&Error::RateLimit(
            "test".to_string()
        )));

        // Non-retryable errors
        assert!(!RetryPolicy::is_retryable(&Error::InvalidInput(
            "test".to_string()
        )));
        assert!(!RetryPolicy::is_retryable(&Error::NotImplemented(
            "test".to_string()
        )));
    }

    // ============================================================================
    // Additional comprehensive tests for retry module
    // ============================================================================

    #[tokio::test]
    async fn test_exponential_with_params() {
        let policy = RetryPolicy::exponential_with_params(5, 500, 8000);
        assert_eq!(policy.max_retries, 5);
        match policy.strategy {
            RetryStrategy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                multiplier,
            } => {
                assert_eq!(initial_delay_ms, 500);
                assert_eq!(max_delay_ms, 8000);
                assert_eq!(multiplier, 2);
            }
            _ => panic!("Expected exponential strategy"),
        }
    }

    #[tokio::test]
    async fn test_exponential_backoff_reaches_max() {
        // Test that exponential backoff caps at max_delay_ms
        let policy = RetryPolicy::exponential_with_params(10, 100, 500);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        let start = std::time::Instant::now();

        let _ = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Network error".to_string()))
            }
        })
        .await;

        let elapsed = start.elapsed();
        // With max_delay_ms=500, most delays should be capped
        // Initial delays: 100, 200, 400, 500, 500, 500...
        // Should take at least a few seconds but not grow exponentially forever
        assert!(*lock(&counter) == 11); // initial + 10 retries
        assert!(elapsed.as_millis() >= 3000); // At least some delays happened
    }

    #[tokio::test]
    async fn test_fixed_backoff_timing() {
        let policy = RetryPolicy::fixed(2, 100);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        let start = std::time::Instant::now();

        let _ = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Timeout("Timeout".to_string()))
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert_eq!(*lock(&counter), 3); // initial + 2 retries
                                        // Fixed 100ms between retries, so should be at least 200ms
        assert!(elapsed.as_millis() >= 200);
    }

    #[tokio::test]
    async fn test_default_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        // M-195: Default now uses ExponentialJitter to prevent thundering herd
        match policy.strategy {
            RetryStrategy::ExponentialJitter {
                initial_delay_ms,
                max_delay_ms,
                exp_base,
                jitter_ms,
            } => {
                assert_eq!(initial_delay_ms, 1000);
                assert_eq!(max_delay_ms, 10000);
                assert!((exp_base - 2.0).abs() < f64::EPSILON);
                assert_eq!(jitter_ms, 1000);
            }
            _ => panic!("Expected ExponentialJitter strategy"),
        }
    }

    #[tokio::test]
    async fn test_retry_with_rate_limit_error() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(2), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 2 {
                    Err(Error::RateLimit("Rate limit exceeded".to_string()))
                } else {
                    Ok::<i32, Error>(100)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
        assert_eq!(*lock(&counter), 2);
    }

    #[tokio::test]
    async fn test_retry_with_timeout_error() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::fixed(3, 50), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 3 {
                    Err(Error::Timeout("Request timeout".to_string()))
                } else {
                    Ok::<String, Error>("success".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(*lock(&counter), 3);
    }

    #[tokio::test]
    async fn test_mixed_retryable_and_non_retryable() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&RetryPolicy::exponential(5), || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count == 1 {
                    Err(Error::Network("Network error".to_string()))
                } else if *count == 2 {
                    // Non-retryable error should stop retries
                    Err(Error::InvalidInput("Invalid request".to_string()))
                } else {
                    Ok::<i32, Error>(200)
                }
            }
        })
        .await;

        assert!(result.is_err());
        // Should stop at the non-retryable error
        assert_eq!(*lock(&counter), 2);
        match result.unwrap_err() {
            Error::InvalidInput(_) => {} // Expected
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_zero_delay_fixed() {
        let policy = RetryPolicy::fixed(3, 0);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        let start = std::time::Instant::now();

        let _ = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Network error".to_string()))
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert_eq!(*lock(&counter), 4); // initial + 3 retries
                                        // With zero delay, should complete very quickly
        assert!(elapsed.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_large_max_retries() {
        let policy = RetryPolicy::fixed(100, 0);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 50 {
                    Err(Error::Network("Network error".to_string()))
                } else {
                    Ok::<i32, Error>(999)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 999);
        assert_eq!(*lock(&counter), 50);
    }

    #[tokio::test]
    async fn test_retry_with_different_return_types() {
        // Test with String
        let result_string = with_retry(&RetryPolicy::no_retry(), || async {
            Ok::<String, Error>("hello".to_string())
        })
        .await;
        assert!(result_string.is_ok());
        assert_eq!(result_string.unwrap(), "hello");

        // Test with Vec
        let result_vec = with_retry(&RetryPolicy::no_retry(), || async {
            Ok::<Vec<i32>, Error>(vec![1, 2, 3])
        })
        .await;
        assert!(result_vec.is_ok());
        assert_eq!(result_vec.unwrap(), vec![1, 2, 3]);

        // Test with custom struct
        #[derive(Debug, PartialEq)]
        struct CustomResult {
            value: i32,
            message: String,
        }
        let result_custom = with_retry(&RetryPolicy::no_retry(), || async {
            Ok::<CustomResult, Error>(CustomResult {
                value: 42,
                message: "test".to_string(),
            })
        })
        .await;
        assert!(result_custom.is_ok());
        assert_eq!(result_custom.unwrap().value, 42);
    }

    #[tokio::test]
    async fn test_retry_policy_clone() {
        let original = RetryPolicy::exponential(5);
        let cloned = original.clone();
        assert_eq!(original.max_retries, cloned.max_retries);
    }

    #[tokio::test]
    async fn test_retry_policy_debug() {
        let policy = RetryPolicy::exponential(3);
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("RetryPolicy"));
        assert!(debug_str.contains("max_retries"));
    }

    #[tokio::test]
    async fn test_retry_strategy_debug() {
        let strategy = RetryStrategy::Exponential {
            initial_delay_ms: 1000,
            max_delay_ms: 10000,
            multiplier: 2,
        };
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("Exponential"));
        assert!(debug_str.contains("initial_delay_ms"));
    }

    #[tokio::test]
    async fn test_successful_operation_no_delay() {
        // When operation succeeds immediately, no delay should occur
        let start = std::time::Instant::now();
        let result = with_retry(&RetryPolicy::fixed(3, 1000), || async {
            Ok::<i32, Error>(42)
        })
        .await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        // Should complete almost instantly, well under the 1000ms retry delay
        assert!(elapsed.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_all_error_types_retryability() {
        // Test all retryable error types
        let retryable_errors = vec![
            Error::Network("test".to_string()),
            Error::Timeout("test".to_string()),
            Error::RateLimit("test".to_string()),
        ];

        for error in retryable_errors {
            assert!(RetryPolicy::is_retryable(&error));
        }

        // Test all non-retryable error types
        let non_retryable_errors = vec![
            Error::InvalidInput("test".to_string()),
            Error::NotImplemented("test".to_string()),
            Error::Io(std::io::Error::other("test")),
        ];

        for error in non_retryable_errors {
            assert!(!RetryPolicy::is_retryable(&error));
        }
    }

    #[tokio::test]
    async fn test_exponential_delay_sequence() {
        // Verify exponential backoff produces expected delay sequence
        let policy = RetryPolicy::exponential_with_params(5, 100, 1000);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        let start = std::time::Instant::now();

        let _ = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Network error".to_string()))
            }
        })
        .await;

        let elapsed = start.elapsed();
        // Delays: 100, 200, 400, 800, 1000 (capped)
        // Total: 100 + 200 + 400 + 800 + 1000 = 2500ms minimum
        assert_eq!(*lock(&counter), 6); // initial + 5 retries
        assert!(elapsed.as_millis() >= 2500);
    }

    #[tokio::test]
    async fn test_retry_preserves_error_message() {
        let result = with_retry(&RetryPolicy::exponential(2), || async {
            Err::<i32, Error>(Error::Network("specific error message".to_string()))
        })
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Network(msg) => assert_eq!(msg, "specific error message"),
            _ => panic!("Expected Network error"),
        }
    }

    #[tokio::test]
    async fn test_one_retry_succeeds() {
        let policy = RetryPolicy::exponential(1);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count == 1 {
                    Err(Error::Network("First failure".to_string()))
                } else {
                    Ok::<i32, Error>(123)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123);
        assert_eq!(*lock(&counter), 2); // initial + 1 retry
    }

    #[tokio::test]
    async fn test_one_retry_fails() {
        let policy = RetryPolicy::exponential(1);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Always fails".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(*lock(&counter), 2); // initial + 1 retry
    }

    #[tokio::test]
    async fn test_very_small_initial_delay() {
        let policy = RetryPolicy::exponential_with_params(3, 1, 1000);
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 3 {
                    Err(Error::Timeout("Timeout".to_string()))
                } else {
                    Ok::<i32, Error>(777)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 777);
        assert_eq!(*lock(&counter), 3);
    }

    #[tokio::test]
    async fn test_fixed_strategy_clone() {
        let strategy = RetryStrategy::Fixed { delay_ms: 500 };
        let cloned = strategy.clone();
        match (strategy, cloned) {
            (RetryStrategy::Fixed { delay_ms: d1 }, RetryStrategy::Fixed { delay_ms: d2 }) => {
                assert_eq!(d1, d2)
            }
            _ => panic!("Clone failed"),
        }
    }

    #[tokio::test]
    async fn test_exponential_strategy_clone() {
        let strategy = RetryStrategy::Exponential {
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            multiplier: 3,
        };
        let cloned = strategy.clone();
        match (strategy, cloned) {
            (
                RetryStrategy::Exponential {
                    initial_delay_ms: i1,
                    max_delay_ms: m1,
                    multiplier: mul1,
                },
                RetryStrategy::Exponential {
                    initial_delay_ms: i2,
                    max_delay_ms: m2,
                    multiplier: mul2,
                },
            ) => {
                assert_eq!(i1, i2);
                assert_eq!(m1, m2);
                assert_eq!(mul1, mul2);
            }
            _ => panic!("Clone failed"),
        }
    }

    // ============================================================================
    // RunnableRetry tests
    // ============================================================================

    use crate::core::config::RunnableConfig;
    use crate::core::runnable::Runnable;

    // Simple test runnable that fails N times then succeeds
    struct FailNTimes {
        fail_count: Arc<Mutex<usize>>,
        target_failures: usize,
    }

    #[async_trait::async_trait]
    impl Runnable for FailNTimes {
        type Input = String;
        type Output = String;

        async fn invoke(
            &self,
            input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            let mut count = lock(&self.fail_count);
            *count += 1;
            if *count <= self.target_failures {
                Err(Error::Network(format!(
                    "Failure {} of {}",
                    *count, self.target_failures
                )))
            } else {
                Ok(format!(
                    "Success after {} attempts with input: {}",
                    *count, input
                ))
            }
        }
    }

    #[tokio::test]
    async fn test_runnable_retry_succeeds_after_failures() {
        let runnable = FailNTimes {
            fail_count: Arc::new(Mutex::new(0)),
            target_failures: 2,
        };

        let policy = RetryPolicy::fixed(3, 10); // Fast retries for testing
        let retryable = RunnableRetry::new(runnable, policy);

        let result = retryable
            .invoke("test input".to_string(), None)
            .await
            .unwrap();

        assert!(result.contains("Success after 3 attempts"));
    }

    #[tokio::test]
    async fn test_runnable_retry_fails_after_max_retries() {
        let runnable = FailNTimes {
            fail_count: Arc::new(Mutex::new(0)),
            target_failures: 10, // Will never succeed
        };

        let policy = RetryPolicy::fixed(3, 10);
        let retryable = RunnableRetry::new(runnable, policy);

        let result = retryable.invoke("test input".to_string(), None).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Network(msg) => assert!(msg.contains("Failure")),
            _ => panic!("Expected Network error"),
        }
    }

    #[tokio::test]
    async fn test_runnable_retry_with_jitter() {
        let runnable = FailNTimes {
            fail_count: Arc::new(Mutex::new(0)),
            target_failures: 1,
        };

        let policy = RetryPolicy::exponential_jitter(2, 50, 500, 2.0, 50);
        let retryable = RunnableRetry::new(runnable, policy);

        let result = retryable
            .invoke("test input".to_string(), None)
            .await
            .unwrap();

        assert!(result.contains("Success after 2 attempts"));
    }

    #[tokio::test]
    async fn test_runnable_retry_name() {
        let runnable = FailNTimes {
            fail_count: Arc::new(Mutex::new(0)),
            target_failures: 0,
        };

        let policy = RetryPolicy::no_retry();
        let retryable = RunnableRetry::new(runnable, policy);

        let name = retryable.name();
        assert!(name.contains("RunnableRetry"));
        assert!(name.contains("FailNTimes"));
    }

    #[tokio::test]
    async fn test_runnable_retry_batch() {
        let runnable = FailNTimes {
            fail_count: Arc::new(Mutex::new(0)),
            target_failures: 1,
        };

        let policy = RetryPolicy::fixed(2, 10);
        let retryable = RunnableRetry::new(runnable, policy);

        let inputs = vec!["input1".to_string(), "input2".to_string()];
        let results = retryable.batch(inputs, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].contains("input1"));
        assert!(results[1].contains("input2"));
    }

    #[tokio::test]
    async fn test_default_jitter_policy() {
        let policy = RetryPolicy::default_jitter(5);
        assert_eq!(policy.max_retries, 5);
        match policy.strategy {
            RetryStrategy::ExponentialJitter {
                initial_delay_ms,
                max_delay_ms,
                exp_base,
                jitter_ms,
            } => {
                assert_eq!(initial_delay_ms, 1000);
                assert_eq!(max_delay_ms, 10000);
                assert_eq!(exp_base, 2.0);
                assert_eq!(jitter_ms, 1000);
            }
            _ => panic!("Expected ExponentialJitter strategy"),
        }
    }

    #[tokio::test]
    async fn test_exponential_jitter_has_randomness() {
        // This test verifies that jitter adds randomness by running multiple
        // retries and checking that delays are not all identical
        let mut delays = Vec::new();

        for _ in 0..5 {
            let policy = RetryPolicy::exponential_jitter(3, 100, 1000, 2.0, 500);
            let start = std::time::Instant::now();

            let _ = with_retry(&policy, || async {
                Err::<(), Error>(Error::Network("test".to_string()))
            })
            .await;

            delays.push(start.elapsed().as_millis());
        }

        // With jitter, not all delays should be identical
        // (very low probability of all being the same with 500ms jitter)
        let first = delays[0];
        let all_same = delays.iter().all(|&d| d == first);
        assert!(!all_same, "Expected jitter to produce varying delays");
    }

    // ------------------------------------------------------------------------
    // RetryPolicy Validation Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_retry_policy_validate_valid() {
        let policy = RetryPolicy::default();
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_retry_policy_validate_too_many_retries() {
        let policy = RetryPolicy {
            max_retries: 1001,
            ..Default::default()
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max_retries too large"));
    }

    #[test]
    fn test_retry_policy_validate_zero_retries_valid() {
        // Zero retries is valid (means no retry, fail immediately)
        let policy = RetryPolicy::no_retry();
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_retry_policy_validate_max_retries_boundary() {
        // max_retries = 1000 is valid (boundary)
        let policy = RetryPolicy {
            max_retries: 1000,
            ..Default::default()
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_retry_policy_validate_exponential_zero_multiplier() {
        let policy = RetryPolicy {
            max_retries: 3,
            strategy: RetryStrategy::Exponential {
                initial_delay_ms: 100,
                max_delay_ms: 1000,
                multiplier: 0,
            },
            rate_limiter: None,
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("multiplier must be at least 1"));
    }

    #[test]
    fn test_retry_policy_validate_exponential_initial_exceeds_max() {
        let policy = RetryPolicy {
            max_retries: 3,
            strategy: RetryStrategy::Exponential {
                initial_delay_ms: 2000,
                max_delay_ms: 1000,
                multiplier: 2,
            },
            rate_limiter: None,
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("initial_delay_ms (2000) must not exceed max_delay_ms (1000)"));
    }

    #[test]
    fn test_retry_policy_validate_jitter_negative_exp_base() {
        let policy = RetryPolicy {
            max_retries: 3,
            strategy: RetryStrategy::ExponentialJitter {
                initial_delay_ms: 100,
                max_delay_ms: 1000,
                exp_base: -1.0,
                jitter_ms: 100,
            },
            rate_limiter: None,
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exp_base must be positive"));
    }

    #[test]
    fn test_retry_policy_validate_jitter_zero_exp_base() {
        let policy = RetryPolicy {
            max_retries: 3,
            strategy: RetryStrategy::ExponentialJitter {
                initial_delay_ms: 100,
                max_delay_ms: 1000,
                exp_base: 0.0,
                jitter_ms: 100,
            },
            rate_limiter: None,
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exp_base must be positive"));
    }

    #[test]
    fn test_retry_policy_validate_jitter_initial_exceeds_max() {
        let policy = RetryPolicy {
            max_retries: 3,
            strategy: RetryStrategy::ExponentialJitter {
                initial_delay_ms: 5000,
                max_delay_ms: 1000,
                exp_base: 2.0,
                jitter_ms: 100,
            },
            rate_limiter: None,
        };
        let result = policy.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("initial_delay_ms (5000) must not exceed max_delay_ms (1000)"));
    }

    #[test]
    fn test_retry_policy_validate_fixed_always_valid() {
        // Fixed strategy has no invalid configurations
        let policy = RetryPolicy::fixed(3, 0);
        assert!(policy.validate().is_ok());

        let policy = RetryPolicy::fixed(3, u64::MAX);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_retry_policy_validate_valid_exponential() {
        let policy = RetryPolicy::exponential(5);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_retry_policy_validate_valid_jitter() {
        let policy = RetryPolicy::default_jitter(5);
        assert!(policy.validate().is_ok());
    }

    // ============================================================================
    // Rate Limiter Integration Tests
    // ============================================================================

    use crate::core::rate_limiters::InMemoryRateLimiter;

    #[tokio::test]
    async fn test_retry_with_rate_limiter() {
        // Create a rate limiter that allows 10 requests per second
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let policy = RetryPolicy::fixed(3, 0).with_rate_limiter(limiter.clone());

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                if *count < 3 {
                    Err(Error::Network("Transient error".to_string()))
                } else {
                    Ok::<i32, Error>(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*lock(&counter), 3);
    }

    #[tokio::test]
    async fn test_retry_with_rate_limiter_enforces_rate() {
        // Create a slow rate limiter: 2 requests per second
        let limiter = Arc::new(InMemoryRateLimiter::new(
            2.0,
            Duration::from_millis(10),
            2.0,
        ));

        let policy = RetryPolicy::fixed(3, 0).with_rate_limiter(limiter);

        let start = Instant::now();

        // This will fail all attempts but should take time due to rate limiting
        let _ = with_retry(&policy, || async {
            Err::<i32, Error>(Error::Network("Always fails".to_string()))
        })
        .await;

        let elapsed = start.elapsed();

        // With 2 req/sec and 4 attempts (initial + 3 retries), should take ~1.5-2s
        // First token takes ~500ms, subsequent tokens also ~500ms each
        assert!(
            elapsed >= Duration::from_millis(1000),
            "Rate limiting should enforce minimum delay, but took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_retry_policy_with_rate_limiter_builder() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            100.0,
            Duration::from_millis(10),
            10.0,
        ));

        let policy = RetryPolicy::exponential(3).with_rate_limiter(limiter.clone());

        assert!(policy.rate_limiter.is_some());
        assert_eq!(policy.max_retries, 3);
    }

    #[tokio::test]
    async fn test_retry_without_rate_limiter_is_fast() {
        // Without rate limiter, retries should complete quickly
        let policy = RetryPolicy::fixed(5, 0);

        let start = Instant::now();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let _ = with_retry(&policy, || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let mut count = lock(&counter);
                *count += 1;
                Err::<i32, Error>(Error::Network("Always fails".to_string()))
            }
        })
        .await;

        let elapsed = start.elapsed();

        // Should complete very quickly without rate limiting
        assert!(
            elapsed < Duration::from_millis(100),
            "Without rate limiter, should be fast, but took {:?}",
            elapsed
        );
        assert_eq!(*lock(&counter), 6); // initial + 5 retries
    }

    #[tokio::test]
    async fn test_retry_policy_debug_with_rate_limiter() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let policy = RetryPolicy::exponential(3).with_rate_limiter(limiter);
        let debug_str = format!("{:?}", policy);

        assert!(debug_str.contains("RetryPolicy"));
        assert!(debug_str.contains("max_retries"));
        assert!(debug_str.contains("<RateLimiter>"));
    }

    #[tokio::test]
    async fn test_retry_policy_clone_with_rate_limiter() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let policy = RetryPolicy::exponential(3).with_rate_limiter(limiter);
        let cloned = policy.clone();

        assert!(cloned.rate_limiter.is_some());
        assert_eq!(cloned.max_retries, 3);
    }
}
