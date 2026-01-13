//! Cost tracking and rate limiting utilities for E2E tests with real LLM APIs
//!
//! This module provides utilities for E2E test cost tracking:
//! - Test budget tracking (warn if >$1/test run)
//! - Model selection helper (prefer cheaper models for tests)
//! - Rate limit retry logic with exponential backoff
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_test_utils::test_cost::{TestCostTracker, with_rate_limit_retry};
//!
//! // Initialize cost tracker for test run
//! let tracker = TestCostTracker::new().with_budget(1.0);
//!
//! // Record token usage after each LLM call
//! tracker.record_usage("gpt-3.5-turbo", 1000, 500)?;
//!
//! // Check if budget exceeded
//! if tracker.is_over_budget() {
//!     println!("WARNING: Test budget exceeded!");
//! }
//!
//! // Wrap API calls with retry logic
//! let result = with_rate_limit_retry(|| async {
//!     // Your API call here
//! }).await?;
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Approximate cost per 1M input tokens for common models (USD)
const COST_GPT_4_TURBO_INPUT: f64 = 10.0;
const COST_GPT_4_TURBO_OUTPUT: f64 = 30.0;
const COST_GPT_4O_INPUT: f64 = 2.5;
const COST_GPT_4O_OUTPUT: f64 = 10.0;
const COST_GPT_4O_MINI_INPUT: f64 = 0.15;
const COST_GPT_4O_MINI_OUTPUT: f64 = 0.60;
const COST_GPT_35_TURBO_INPUT: f64 = 0.50;
const COST_GPT_35_TURBO_OUTPUT: f64 = 1.50;

/// Test cost tracker for monitoring LLM API usage during E2E tests
#[derive(Clone)]
pub struct TestCostTracker {
    /// Budget in USD (default: $1.00)
    budget: f64,
    /// Total cost accumulated (stored as microdollars for atomic ops)
    total_cost_microdollars: Arc<AtomicU64>,
    /// Total input tokens
    total_input_tokens: Arc<AtomicU64>,
    /// Total output tokens
    total_output_tokens: Arc<AtomicU64>,
    /// API call count
    call_count: Arc<AtomicU64>,
}

impl Default for TestCostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCostTracker {
    /// Create a new test cost tracker with default $1.00 budget
    #[must_use]
    pub fn new() -> Self {
        Self {
            budget: 1.0,
            total_cost_microdollars: Arc::new(AtomicU64::new(0)),
            total_input_tokens: Arc::new(AtomicU64::new(0)),
            total_output_tokens: Arc::new(AtomicU64::new(0)),
            call_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Set the budget limit (in USD)
    #[must_use]
    pub fn with_budget(mut self, budget: f64) -> Self {
        self.budget = budget;
        self
    }

    /// Record token usage for a model
    pub fn record_usage(&self, model: &str, input_tokens: u64, output_tokens: u64) {
        let (input_rate, output_rate) = get_model_rates(model);
        let cost =
            (input_tokens as f64 * input_rate + output_tokens as f64 * output_rate) / 1_000_000.0;

        // Store as microdollars (millionths of a dollar) for atomic operations
        let cost_microdollars = (cost * 1_000_000.0) as u64;
        self.total_cost_microdollars
            .fetch_add(cost_microdollars, Ordering::Relaxed);
        self.total_input_tokens
            .fetch_add(input_tokens, Ordering::Relaxed);
        self.total_output_tokens
            .fetch_add(output_tokens, Ordering::Relaxed);
        self.call_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total cost in USD
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.total_cost_microdollars.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Check if budget is exceeded
    #[must_use]
    pub fn is_over_budget(&self) -> bool {
        self.total_cost() > self.budget
    }

    /// Get remaining budget in USD
    #[must_use]
    pub fn remaining_budget(&self) -> f64 {
        self.budget - self.total_cost()
    }

    /// Get budget utilization as a percentage
    #[must_use]
    pub fn budget_utilization(&self) -> f64 {
        (self.total_cost() / self.budget) * 100.0
    }

    /// Get total API call count
    #[must_use]
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::Relaxed)
    }

    /// Get summary report
    #[must_use]
    pub fn report(&self) -> TestCostReport {
        TestCostReport {
            total_cost: self.total_cost(),
            budget: self.budget,
            remaining: self.remaining_budget(),
            utilization_percent: self.budget_utilization(),
            input_tokens: self.total_input_tokens.load(Ordering::Relaxed),
            output_tokens: self.total_output_tokens.load(Ordering::Relaxed),
            call_count: self.call_count(),
            over_budget: self.is_over_budget(),
        }
    }

    /// Print a warning if budget is exceeded or nearly exceeded
    pub fn check_and_warn(&self) {
        let report = self.report();
        if report.over_budget {
            eprintln!(
                "⚠️  TEST BUDGET EXCEEDED: ${:.4} spent (budget: ${:.2})",
                report.total_cost, report.budget
            );
        } else if report.utilization_percent > 80.0 {
            eprintln!(
                "⚠️  Test budget at {:.1}%: ${:.4} of ${:.2}",
                report.utilization_percent, report.total_cost, report.budget
            );
        }
    }
}

/// Cost report for test run
#[derive(Debug, Clone)]
pub struct TestCostReport {
    /// Total cost in USD
    pub total_cost: f64,
    /// Budget limit in USD
    pub budget: f64,
    /// Remaining budget in USD
    pub remaining: f64,
    /// Budget utilization percentage
    pub utilization_percent: f64,
    /// Total input tokens
    pub input_tokens: u64,
    /// Total output tokens
    pub output_tokens: u64,
    /// Total API calls
    pub call_count: u64,
    /// Whether budget was exceeded
    pub over_budget: bool,
}

/// Get input/output rates for a model (per 1M tokens)
fn get_model_rates(model: &str) -> (f64, f64) {
    match model {
        m if m.contains("gpt-4-turbo") => (COST_GPT_4_TURBO_INPUT, COST_GPT_4_TURBO_OUTPUT),
        m if m.contains("gpt-4o-mini") => (COST_GPT_4O_MINI_INPUT, COST_GPT_4O_MINI_OUTPUT),
        m if m.contains("gpt-4o") => (COST_GPT_4O_INPUT, COST_GPT_4O_OUTPUT),
        m if m.contains("gpt-4") => (COST_GPT_4_TURBO_INPUT, COST_GPT_4_TURBO_OUTPUT), // Default to turbo rates
        m if m.contains("gpt-3.5") => (COST_GPT_35_TURBO_INPUT, COST_GPT_35_TURBO_OUTPUT),
        _ => (COST_GPT_4O_MINI_INPUT, COST_GPT_4O_MINI_OUTPUT), // Default to cheapest
    }
}

/// Recommended model for tests (cheapest while still capable)
pub const RECOMMENDED_TEST_MODEL: &str = "gpt-4o-mini";

/// Get the recommended model for testing
///
/// Returns gpt-4o-mini by default, or gpt-3.5-turbo if specified via
/// TEST_LLM_MODEL environment variable.
#[must_use]
pub fn recommended_test_model() -> &'static str {
    // Allow override via environment variable
    if std::env::var("TEST_LLM_MODEL").is_ok() {
        // Check if it's requesting a cheaper model
        if let Ok(model) = std::env::var("TEST_LLM_MODEL") {
            if model.contains("3.5") {
                return "gpt-3.5-turbo";
            }
        }
    }
    RECOMMENDED_TEST_MODEL
}

/// Error type for rate limit and retry operations
#[derive(Debug, Clone)]
pub enum RetryError {
    /// Rate limit hit, includes wait duration
    RateLimited(Duration),
    /// Maximum retries exceeded
    MaxRetriesExceeded,
    /// Other API error
    ApiError(String),
}

impl std::fmt::Display for RetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited(d) => write!(f, "Rate limited, retry after {:?}", d),
            Self::MaxRetriesExceeded => write!(f, "Maximum retries exceeded"),
            Self::ApiError(s) => write!(f, "API error: {}", s),
        }
    }
}

impl std::error::Error for RetryError {}

/// Configuration for rate limit retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

/// Execute a closure with rate limit retry logic
///
/// This function wraps an async operation and retries on rate limit errors
/// with exponential backoff.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_test_utils::test_cost::with_rate_limit_retry;
///
/// let result = with_rate_limit_retry(RetryConfig::default(), || async {
///     // Your API call here
///     Ok(())
/// }).await;
/// ```
pub async fn with_rate_limit_retry<F, Fut, T, E>(config: RetryConfig, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempts = 0;
    let mut backoff = config.initial_backoff;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let error_msg = e.to_string().to_lowercase();

                // Check if it's a rate limit error
                let is_rate_limit = error_msg.contains("rate limit")
                    || error_msg.contains("rate_limit")
                    || error_msg.contains("429")
                    || error_msg.contains("too many requests");

                if !is_rate_limit || attempts >= config.max_retries {
                    return Err(e);
                }

                attempts += 1;
                eprintln!(
                    "Rate limited, attempt {}/{}, waiting {:?}...",
                    attempts, config.max_retries, backoff
                );

                tokio::time::sleep(backoff).await;

                // Exponential backoff with cap
                backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * config.multiplier)
                        .min(config.max_backoff.as_secs_f64()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker_basic() {
        let tracker = TestCostTracker::new().with_budget(1.0);

        // Record some usage
        tracker.record_usage("gpt-4o-mini", 1000, 500);

        let report = tracker.report();
        assert!(report.total_cost > 0.0);
        assert!(report.total_cost < 0.01); // Should be very cheap
        assert_eq!(report.input_tokens, 1000);
        assert_eq!(report.output_tokens, 500);
        assert_eq!(report.call_count, 1);
    }

    #[test]
    fn test_budget_exceeded() {
        let tracker = TestCostTracker::new().with_budget(0.0001); // Very low budget

        // Record enough to exceed budget
        tracker.record_usage("gpt-4o-mini", 10000, 5000);

        assert!(tracker.is_over_budget());
        assert!(tracker.remaining_budget() < 0.0);
    }

    #[test]
    fn test_model_rates() {
        // gpt-4o-mini should be cheap
        let (input, output) = get_model_rates("gpt-4o-mini");
        assert!(input < 1.0); // Less than $1/1M tokens
        assert!(output < 1.0);

        // gpt-4-turbo should be expensive
        let (input, output) = get_model_rates("gpt-4-turbo-preview");
        assert!(input > 5.0); // More than $5/1M tokens
        assert!(output > 20.0);
    }

    #[test]
    fn test_recommended_model() {
        // Default should be gpt-4o-mini
        let model = recommended_test_model();
        assert_eq!(model, "gpt-4o-mini");
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff, Duration::from_secs(1));
    }
}
