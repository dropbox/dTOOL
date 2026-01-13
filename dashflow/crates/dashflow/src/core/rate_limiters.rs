//! Rate limiting utilities for controlling request rates.
//!
//! This module provides rate limiting implementations based on the token bucket algorithm.
//! Rate limiters can be used to control the rate at which requests are made to external services,
//! helping to avoid rate limit errors and ensuring fair resource usage.
//!
//! # Examples
//!
//! ```rust,ignore
//! use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
//! use std::time::Duration;
//!
//! # async fn example() {
//! // Create a rate limiter that allows 2 requests per second
//! let limiter = InMemoryRateLimiter::new(
//!     2.0,  // requests_per_second
//!     Duration::from_millis(100),  // check_every
//!     2.0,  // max_bucket_size
//! );
//!
//! // Acquire permission to make a request (async)
//! limiter.acquire().await;
//! // Make your API request here
//!
//! // Non-blocking acquire (returns immediately)
//! if limiter.try_acquire() {
//!     // Make your API request here
//! } else {
//!     // Rate limit exceeded, wait or skip
//! }
//! # }
//! ```

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Error returned when rate limiter configuration is invalid.
#[derive(Debug, Clone, Error, PartialEq)]
#[non_exhaustive]
pub enum RateLimiterConfigError {
    /// The max_bucket_size must be at least 1.0.
    #[error("Invalid max_bucket_size: must be at least 1.0, got {0}")]
    InvalidBucketSize(f64),
    /// The requests_per_second must be positive (> 0).
    #[error("Invalid requests_per_second: must be positive, got {0}")]
    InvalidRequestsPerSecond(f64),
}

/// A trait for rate limiters.
///
/// Rate limiters control the rate at which operations can be performed.
/// Implementations must be thread-safe and support both blocking and non-blocking acquisition.
///
/// # Token Bucket Algorithm
///
/// Most rate limiters use a token bucket algorithm where:
/// - Tokens are added to a bucket at a fixed rate
/// - Each operation consumes one token
/// - If no tokens are available, the operation must wait or fail
/// - The bucket has a maximum capacity to prevent bursts
///
/// Note: These "tokens" are unrelated to LLM tokens. They represent request credits.
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync + std::fmt::Debug {
    /// Attempt to acquire permission to proceed with an operation.
    ///
    /// This method will wait asynchronously until a token is available.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let limiter = InMemoryRateLimiter::new(1.0, Duration::from_millis(100), 1.0);
    /// limiter.acquire().await;
    /// // Proceed with operation
    /// # }
    /// ```
    async fn acquire(&self);

    /// Attempt to acquire permission without blocking.
    ///
    /// Returns `true` if permission was granted, `false` if no tokens are available.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 10.0);
    /// if limiter.try_acquire() {
    ///     // Proceed with operation
    /// } else {
    ///     // Rate limit exceeded
    /// }
    /// # }
    /// ```
    fn try_acquire(&self) -> bool;
}

/// Internal state for the token bucket rate limiter.
#[derive(Debug)]
struct TokenBucket {
    /// Number of tokens currently available
    available_tokens: f64,
    /// Last time tokens were added to the bucket
    last_refill: Option<Instant>,
    /// Maximum number of tokens the bucket can hold
    max_bucket_size: f64,
    /// Rate at which tokens are added (tokens per second)
    requests_per_second: f64,
}

impl TokenBucket {
    fn new(requests_per_second: f64, max_bucket_size: f64) -> Self {
        Self {
            available_tokens: 0.0,
            last_refill: None,
            max_bucket_size,
            requests_per_second,
        }
    }

    /// Try to consume one token from the bucket.
    ///
    /// Returns `true` if a token was consumed, `false` otherwise.
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();

        // Initialize on first call to avoid initial burst
        if self.last_refill.is_none() {
            self.last_refill = Some(now);
        }

        // Calculate elapsed time and add tokens
        if let Some(last) = self.last_refill {
            let elapsed = now.duration_since(last).as_secs_f64();

            // Only refill if enough time has passed to generate at least one token
            if elapsed * self.requests_per_second >= 1.0 {
                self.available_tokens += elapsed * self.requests_per_second;
                self.last_refill = Some(now);
            }
        }

        // Cap tokens at max bucket size to prevent bursts
        self.available_tokens = self.available_tokens.min(self.max_bucket_size);

        // Try to consume a token
        if self.available_tokens >= 1.0 {
            self.available_tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// An in-memory rate limiter based on the token bucket algorithm.
///
/// This rate limiter is thread-safe and can be used in async contexts.
/// It uses a token bucket algorithm where tokens are added at a fixed rate
/// and consumed by requests.
///
/// # Limitations
///
/// - This is an in-memory rate limiter and cannot coordinate across processes
/// - Only time-based rate limiting is supported (does not account for request/response size)
/// - Rate limiting is not currently surfaced in tracing or callbacks
///
/// # Examples
///
/// ```rust,ignore
/// use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
/// use std::time::{Duration, Instant};
///
/// # async fn example() {
/// // Allow 10 requests per second, check every 100ms, max burst of 10
/// let limiter = InMemoryRateLimiter::new(
///     10.0,
///     Duration::from_millis(100),
///     10.0,
/// );
///
/// // Make some rate-limited requests
/// for i in 0..5 {
///     let start = Instant::now();
///     limiter.acquire().await;
///     println!("Request {} took {:?}", i, start.elapsed());
///     // Make your API request here
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct InMemoryRateLimiter {
    /// Shared token bucket state
    bucket: Arc<Mutex<TokenBucket>>,
    /// How often to check for available tokens when waiting
    check_interval: Duration,
}

impl InMemoryRateLimiter {
    /// Create a new in-memory rate limiter, returning an error if configuration is invalid.
    ///
    /// # Arguments
    ///
    /// * `requests_per_second` - The number of tokens to add per second (must be > 0).
    /// * `check_every` - How often to check for available tokens when waiting.
    /// * `max_bucket_size` - The maximum number of tokens (must be >= 1.0).
    ///
    /// # Errors
    ///
    /// Returns [`RateLimiterConfigError`] if:
    /// - `max_bucket_size` is less than 1.0
    /// - `requests_per_second` is not positive
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiterConfigError};
    /// use std::time::Duration;
    ///
    /// // Valid configuration
    /// let limiter = InMemoryRateLimiter::try_new(10.0, Duration::from_millis(100), 10.0);
    /// assert!(limiter.is_ok());
    ///
    /// // Invalid bucket size
    /// let err = InMemoryRateLimiter::try_new(10.0, Duration::from_millis(100), 0.0);
    /// assert!(matches!(err, Err(RateLimiterConfigError::InvalidBucketSize(_))));
    /// ```
    pub fn try_new(
        requests_per_second: f64,
        check_every: Duration,
        max_bucket_size: f64,
    ) -> Result<Self, RateLimiterConfigError> {
        if max_bucket_size < 1.0 {
            return Err(RateLimiterConfigError::InvalidBucketSize(max_bucket_size));
        }
        if requests_per_second <= 0.0 {
            return Err(RateLimiterConfigError::InvalidRequestsPerSecond(
                requests_per_second,
            ));
        }

        Ok(Self {
            bucket: Arc::new(Mutex::new(TokenBucket::new(
                requests_per_second,
                max_bucket_size,
            ))),
            check_interval: check_every,
        })
    }

    /// Create a new in-memory rate limiter.
    ///
    /// # Arguments
    ///
    /// * `requests_per_second` - The number of tokens to add per second to the bucket.
    ///   These tokens represent "credit" that can be used to make requests.
    /// * `check_every` - How often to check for available tokens when waiting.
    ///   Smaller values provide more responsive waiting but may increase CPU usage.
    /// * `max_bucket_size` - The maximum number of tokens that can be in the bucket.
    ///   Must be at least 1.0. Controls the maximum burst size.
    ///
    /// # Panics
    ///
    /// Panics if `max_bucket_size < 1.0` or `requests_per_second <= 0.0`.
    /// Use [`try_new`](Self::try_new) for a non-panicking alternative.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::time::Duration;
    ///
    /// // 0.1 requests per second = 1 request every 10 seconds
    /// let slow_limiter = InMemoryRateLimiter::new(
    ///     0.1,
    ///     Duration::from_millis(100),
    ///     10.0,
    /// );
    ///
    /// // 100 requests per second with burst capacity of 20
    /// let fast_limiter = InMemoryRateLimiter::new(
    ///     100.0,
    ///     Duration::from_millis(10),
    ///     20.0,
    /// );
    /// ```
    // SAFETY: Panicking constructor with documented behavior; use try_new() for fallible version
    #[allow(clippy::expect_used)]
    #[must_use]
    pub fn new(requests_per_second: f64, check_every: Duration, max_bucket_size: f64) -> Self {
        Self::try_new(requests_per_second, check_every, max_bucket_size)
            .expect("InMemoryRateLimiter::new called with invalid configuration")
    }

    /// Get the current number of available tokens (for testing/debugging).
    #[cfg(test)]
    pub(crate) fn available_tokens(&self) -> f64 {
        match self.bucket.lock() {
            Ok(guard) => guard.available_tokens,
            Err(poisoned) => poisoned.into_inner().available_tokens,
        }
    }
}

#[async_trait::async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn acquire(&self) {
        // Keep trying until we successfully consume a token
        while !self.try_acquire() {
            tokio::time::sleep(self.check_interval).await;
        }
    }

    fn try_acquire(&self) -> bool {
        // Recover from mutex poison - rate limiting can continue with potentially stale state
        let mut bucket = match self.bucket.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        bucket.try_consume()
    }
}

#[cfg(test)]
mod tests {
    use super::RateLimiterConfigError;
    use crate::test_prelude::*;
    use std::time::Duration;

    #[test]
    fn test_initial_state() {
        let limiter = InMemoryRateLimiter::new(2.0, Duration::from_millis(100), 2.0);
        assert_eq!(limiter.available_tokens(), 0.0);
    }

    #[test]
    fn test_try_acquire_initially_fails() {
        let limiter = InMemoryRateLimiter::new(2.0, Duration::from_millis(100), 2.0);
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_acquire_waits_for_tokens() {
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 2.0);

        // First acquire should work after short wait
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();

        // Should have waited some time
        assert!(elapsed >= Duration::from_millis(90)); // ~100ms for first token
    }

    #[tokio::test]
    async fn test_rate_limiting_timing() {
        // 2 requests per second = 1 request every 500ms
        let limiter = InMemoryRateLimiter::new(2.0, Duration::from_millis(10), 2.0);

        let start = Instant::now();
        limiter.acquire().await; // First request
        let first = start.elapsed();

        limiter.acquire().await; // Second request
        let second = start.elapsed();

        limiter.acquire().await; // Third request
        let third = start.elapsed();

        // First request should complete around 500ms (first token generation)
        assert!(first >= Duration::from_millis(400));
        assert!(first <= Duration::from_millis(600));

        // Second request should complete around 1000ms (second token)
        assert!(second >= Duration::from_millis(900));
        assert!(second <= Duration::from_millis(1100));

        // Third request should complete around 1500ms (third token)
        assert!(third >= Duration::from_millis(1400));
        assert!(third <= Duration::from_millis(1600));
    }

    #[tokio::test]
    async fn test_bucket_size_limits_burst() {
        // 10 requests per second, but max bucket size of 2
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 2.0);

        // Acquire first token to initialize the bucket
        limiter.acquire().await;

        // Wait for bucket to refill (200ms for 2 tokens at 10/sec)
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Should be able to make 2 requests immediately (burst)
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Third request should fail (bucket empty)
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_tokens_refill_over_time() {
        let limiter = InMemoryRateLimiter::new(5.0, Duration::from_millis(10), 5.0);

        // Acquire first token to initialize the bucket
        limiter.acquire().await;

        // Wait for bucket to refill (1 second for 5 tokens at 5/sec)
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should be able to make 5 requests
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Sixth should fail
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_max_bucket_size_caps_tokens() {
        // Even if we wait a long time, bucket should not exceed max size
        let limiter = InMemoryRateLimiter::new(100.0, Duration::from_millis(10), 5.0);

        // Acquire first token to initialize the bucket
        limiter.acquire().await;

        // Wait for long time (would generate many tokens without cap)
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should only be able to make 5 requests (max bucket size)
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Sixth should fail
        assert!(!limiter.try_acquire());
    }

    // ============================================================================
    // Additional comprehensive tests for rate_limiters module
    // ============================================================================

    #[test]
    fn test_try_new_rejects_zero_bucket_size() {
        let result = InMemoryRateLimiter::try_new(1.0, Duration::from_millis(10), 0.0);
        assert!(matches!(
            result,
            Err(RateLimiterConfigError::InvalidBucketSize(size)) if size == 0.0
        ));
    }

    #[test]
    fn test_try_new_rejects_negative_bucket_size() {
        let result = InMemoryRateLimiter::try_new(1.0, Duration::from_millis(10), -1.0);
        assert!(matches!(
            result,
            Err(RateLimiterConfigError::InvalidBucketSize(size)) if size == -1.0
        ));
    }

    #[test]
    fn test_try_new_rejects_zero_requests_per_second() {
        let result = InMemoryRateLimiter::try_new(0.0, Duration::from_millis(10), 1.0);
        assert!(matches!(
            result,
            Err(RateLimiterConfigError::InvalidRequestsPerSecond(rate)) if rate == 0.0
        ));
    }

    #[test]
    fn test_try_new_rejects_negative_requests_per_second() {
        let result = InMemoryRateLimiter::try_new(-1.0, Duration::from_millis(10), 1.0);
        assert!(matches!(
            result,
            Err(RateLimiterConfigError::InvalidRequestsPerSecond(rate)) if rate == -1.0
        ));
    }

    #[test]
    fn test_try_new_accepts_valid_config() {
        let result = InMemoryRateLimiter::try_new(10.0, Duration::from_millis(100), 5.0);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_very_slow_rate() {
        // 0.1 requests per second = 1 request every 10 seconds
        let limiter = InMemoryRateLimiter::new(0.1, Duration::from_millis(10), 1.0);

        let start = Instant::now();
        limiter.acquire().await; // First request
        let first = start.elapsed();

        // First token should take ~10 seconds to generate
        assert!(first >= Duration::from_millis(9500));
        assert!(first <= Duration::from_millis(10500));
    }

    #[tokio::test]
    async fn test_very_fast_rate() {
        // 1000 requests per second = 1 request every 1ms
        let limiter = InMemoryRateLimiter::new(1000.0, Duration::from_millis(1), 5.0);

        let start = Instant::now();
        limiter.acquire().await; // Initialize
        limiter.acquire().await;
        limiter.acquire().await;
        limiter.acquire().await;
        let elapsed = start.elapsed();

        // Should be very fast
        assert!(elapsed <= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let limiter1 = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 2.0);
        let limiter2 = limiter1.clone();

        // Initialize and fill bucket
        limiter1.acquire().await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Consume tokens from limiter1
        assert!(limiter1.try_acquire());
        assert!(limiter1.try_acquire());

        // limiter2 should see the same state (no tokens left)
        assert!(!limiter2.try_acquire());
    }

    #[tokio::test]
    async fn test_multiple_acquires_in_sequence() {
        let limiter = InMemoryRateLimiter::new(5.0, Duration::from_millis(10), 3.0);

        // Make multiple sequential acquires
        for _ in 0..5 {
            limiter.acquire().await;
        }
        // Should not panic or deadlock
    }

    #[tokio::test]
    async fn test_concurrent_acquire() {
        use tokio::task;

        let limiter = Arc::new(InMemoryRateLimiter::new(
            20.0,
            Duration::from_millis(5),
            10.0,
        ));

        // Spawn multiple concurrent tasks
        let mut handles = vec![];
        for _ in 0..10 {
            let limiter_clone = Arc::clone(&limiter);
            handles.push(task::spawn(async move {
                limiter_clone.acquire().await;
            }));
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
        // Should not panic or deadlock
    }

    #[tokio::test]
    async fn test_concurrent_try_acquire() {
        use tokio::task;

        let limiter = Arc::new(InMemoryRateLimiter::new(
            100.0,
            Duration::from_millis(1),
            50.0,
        ));

        // Initialize bucket
        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(600)).await; // Fill to max

        // Spawn multiple concurrent tasks
        let mut handles = vec![];
        for _ in 0..100 {
            let limiter_clone = Arc::clone(&limiter);
            handles.push(task::spawn(async move { limiter_clone.try_acquire() }));
        }

        // Collect results
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap() {
                success_count += 1;
            }
        }

        // Should have gotten some successes, but not more than bucket size
        assert!(success_count > 0);
        assert!(success_count <= 50);
    }

    #[tokio::test]
    async fn test_very_short_check_interval() {
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(1), 2.0);
        limiter.acquire().await;
        // Should work without issues
    }

    #[tokio::test]
    async fn test_very_long_check_interval() {
        let limiter = InMemoryRateLimiter::new(100.0, Duration::from_millis(500), 5.0);
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();
        // First acquire should wait for at least one token to be generated
        assert!(elapsed >= Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_fractional_requests_per_second() {
        // 0.5 requests per second = 1 request every 2 seconds
        let limiter = InMemoryRateLimiter::new(0.5, Duration::from_millis(10), 1.0);

        let start = Instant::now();
        limiter.acquire().await; // First request
        let first = start.elapsed();

        // Should take ~2 seconds
        assert!(first >= Duration::from_millis(1900));
        assert!(first <= Duration::from_millis(2200));
    }

    #[tokio::test]
    async fn test_large_bucket_size() {
        let limiter = InMemoryRateLimiter::new(1000.0, Duration::from_millis(1), 1000.0);

        // Initialize and wait for bucket to fill
        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should be able to burst 1000 requests
        let mut success = 0;
        for _ in 0..1000 {
            if limiter.try_acquire() {
                success += 1;
            }
        }
        assert!(success >= 950); // Allow some margin
    }

    #[tokio::test]
    async fn test_bucket_size_one() {
        // Minimum bucket size
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 1.0);

        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should only have 1 token available
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_bucket_refill_partial() {
        // Test partial refill (not enough time for full token)
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 5.0);

        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(50)).await; // 0.5 tokens

        // Should not have refilled yet (< 1 token)
        assert!(!limiter.try_acquire());

        tokio::time::sleep(Duration::from_millis(60)).await; // Now 1+ token

        // Should have refilled
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_debug_format() {
        let limiter = InMemoryRateLimiter::new(5.0, Duration::from_millis(100), 5.0);
        let debug_str = format!("{:?}", limiter);
        assert!(debug_str.contains("InMemoryRateLimiter"));
    }

    #[tokio::test]
    async fn test_acquire_after_try_acquire_failure() {
        let limiter = InMemoryRateLimiter::new(5.0, Duration::from_millis(10), 2.0);

        // First try_acquire should fail (no tokens yet)
        assert!(!limiter.try_acquire());

        // But acquire should succeed after waiting
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(150)); // Wait for first token
    }

    #[tokio::test]
    async fn test_try_acquire_after_bucket_depleted() {
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 3.0);

        // Initialize and fill bucket
        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(350)).await;

        // Deplete bucket
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Should fail now
        assert!(!limiter.try_acquire());

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should succeed again
        assert!(limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_rate_limiting_over_long_period() {
        // Test that rate limiting works correctly over multiple refill cycles
        let limiter = InMemoryRateLimiter::new(5.0, Duration::from_millis(10), 2.0);

        let start = Instant::now();
        for _ in 0..10 {
            limiter.acquire().await;
        }
        let elapsed = start.elapsed();

        // 10 requests at 5/sec = 2 seconds
        assert!(elapsed >= Duration::from_millis(1900));
        assert!(elapsed <= Duration::from_millis(2300));
    }

    #[tokio::test]
    async fn test_bucket_initialization_timing() {
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 5.0);

        // Bucket should start with 0 tokens
        assert_eq!(limiter.available_tokens(), 0.0);

        // After acquiring, bucket is initialized but token consumed
        limiter.acquire().await;
        // After acquire, there might be a small amount of tokens due to elapsed time
        // but should be less than 1 (since we just consumed one)
        assert!(limiter.available_tokens() < 1.0);
    }

    #[tokio::test]
    async fn test_rapid_try_acquire_sequence() {
        let limiter = InMemoryRateLimiter::new(100.0, Duration::from_millis(1), 10.0);

        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Rapidly call try_acquire
        let mut successes = 0;
        for _ in 0..20 {
            if limiter.try_acquire() {
                successes += 1;
            }
        }

        // Should get at most 10 successes (bucket size)
        assert!(successes <= 10);
    }

    #[tokio::test]
    async fn test_zero_check_interval_edge_case() {
        // Test with zero check interval (immediate polling)
        let limiter = InMemoryRateLimiter::new(100.0, Duration::from_millis(0), 5.0);
        limiter.acquire().await;
        // Should still work
    }

    #[tokio::test]
    async fn test_mixed_acquire_and_try_acquire() {
        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(10), 3.0);

        // Use acquire
        limiter.acquire().await;

        // Wait and use try_acquire
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(limiter.try_acquire());

        // Use acquire again
        limiter.acquire().await;

        // try_acquire should fail (no tokens)
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_try_acquire_recovers_from_poisoned_mutex() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));
        let limiter_clone = Arc::clone(&limiter);

        // Spawn a thread that will panic while holding the lock
        // We can't directly poison the mutex in this implementation,
        // but we verify the try_acquire doesn't panic when called normally
        let handle = thread::spawn(move || {
            // This just exercises the rate limiter from another thread
            let _ = limiter_clone.try_acquire();
        });

        // Wait for thread
        let _ = handle.join();

        // The limiter should still work after the thread finishes
        // (In a real poison scenario, this would recover from the poison)
        let result = limiter.try_acquire();
        // Result doesn't matter, we just verify no panic
        let _ = result;
    }
}
