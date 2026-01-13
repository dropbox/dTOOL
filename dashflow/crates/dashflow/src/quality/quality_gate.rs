// Allow clippy warnings for this module
// - panic: panic!() in gate validation for configuration errors
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::panic
)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Quality gate with automatic retry loops.
//!
//! # Problem
//!
//! Relying on prompts alone gives ~90% quality. We need 98%+ for production.
//!
//! # Solution (INNOVATION 5)
//!
//! Use DashFlow's cycle feature to create a retry loop. Response quality is
//! judged, and if it's below threshold, the graph automatically loops back and
//! retries with improvements.
//!
//! # Architecture
//!
//! ```text
//! Agent → Response → Judge Quality → Score ≥ 0.95? → END
//!                         ↓               ↓
//!                    Calculate score    Failed
//!                         ↓               ↓
//!                    Emit telemetry    Retry count < max?
//!                                        ↓         ↓
//!                                       Yes       No
//!                                        ↓         ↓
//!                                    ← Retry    Accept
//! ```

use crate::constants::DEFAULT_MAX_RETRIES;
use crate::core::rate_limiters::RateLimiter;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Configuration for quality gate.
#[derive(Clone)]
#[non_exhaustive]
pub struct QualityGateConfig {
    /// Minimum quality score threshold (0.0-1.0)
    pub threshold: f32,

    /// Maximum retry attempts
    pub max_retries: usize,

    /// Strategy for retrying
    pub retry_strategy: RetryStrategy,

    /// Whether to emit telemetry events
    pub emit_telemetry: bool,

    /// Optional rate limiter to prevent API quota exhaustion.
    ///
    /// When set, each retry attempt will acquire permission from the rate limiter
    /// before calling the generate and judge functions. This prevents thundering
    /// herd scenarios when many concurrent quality gates retry simultaneously.
    pub rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl std::fmt::Debug for QualityGateConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QualityGateConfig")
            .field("threshold", &self.threshold)
            .field("max_retries", &self.max_retries)
            .field("retry_strategy", &self.retry_strategy)
            .field("emit_telemetry", &self.emit_telemetry)
            .field(
                "rate_limiter",
                &self.rate_limiter.as_ref().map(|_| "<RateLimiter>"),
            )
            .finish()
    }
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            threshold: 0.95,
            max_retries: DEFAULT_MAX_RETRIES as usize,
            retry_strategy: RetryStrategy::StrongerPrompt,
            emit_telemetry: true,
            rate_limiter: None,
        }
    }
}

impl QualityGateConfig {
    /// Add a rate limiter to this configuration.
    ///
    /// The rate limiter will be called before each retry attempt to prevent
    /// API quota exhaustion when many concurrent quality gates retry simultaneously.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::quality::QualityGateConfig;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::time::Duration;
    /// use std::sync::Arc;
    ///
    /// // 5 requests per second with burst capacity of 10
    /// let limiter = Arc::new(InMemoryRateLimiter::new(5.0, Duration::from_millis(100), 10.0));
    ///
    /// let config = QualityGateConfig::default()
    ///     .with_rate_limiter(limiter);
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }
}

impl QualityGateConfig {
    /// Validate the configuration values.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `max_retries` is 0 (must be at least 1)
    /// - `max_retries` is greater than 100 (prevents runaway loops)
    /// - `threshold` is not in range 0.0-1.0
    pub fn validate(&self) -> Result<(), String> {
        if self.max_retries == 0 {
            return Err("max_retries must be at least 1".to_string());
        }
        if self.max_retries > 100 {
            return Err(format!(
                "max_retries too large: {} (max 100)",
                self.max_retries
            ));
        }
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err(format!("threshold must be 0.0-1.0, got {}", self.threshold));
        }
        Ok(())
    }
}

/// Strategy to use when retrying after low quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetryStrategy {
    /// Retry with same inputs (LLM temperature may give different result)
    SameInputs,

    /// Add stronger system prompt emphasizing quality requirements
    StrongerPrompt,

    /// Upgrade to stronger model (e.g., gpt-4o-mini → gpt-4)
    UpgradeModel,

    /// Refine the response using another LLM call
    RefineResponse,

    /// Force tool calling if not done already
    ForceToolCall,
}

/// Quality score from LLM-as-judge evaluation.
///
/// Represents the multi-dimensional quality assessment of an LLM response.
/// Used by [`QualityGate`] to determine if a response meets quality thresholds.
///
/// # Dimensions
///
/// - **accuracy**: How factually correct is the response (0.0-1.0)
/// - **relevance**: How well does it address the question (0.0-1.0)
/// - **completeness**: Does it cover all aspects needed (0.0-1.0)
///
/// # Example
///
/// ```rust
/// use dashflow::quality::QualityScore;
///
/// let score = QualityScore::new(0.95, 0.90, 0.85);
///
/// // Check average score
/// assert!(score.average() > 0.8);
///
/// // Check against threshold
/// if score.meets_threshold(0.7) {
///     println!("Response passed quality check");
/// }
/// ```
///
/// # See Also
///
/// - [`QualityGate`] - Automatic retry until quality threshold is met
/// - [`QualityGateResult`] - Result of quality gate evaluation
/// - [`QualityGateConfig`] - Configuration for quality gate behavior
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct QualityScore {
    /// How factually correct the response is (0.0-1.0)
    pub accuracy: f32,
    /// How well the response addresses the question (0.0-1.0)
    pub relevance: f32,
    /// Whether the response covers all necessary aspects (0.0-1.0)
    pub completeness: f32,
}

impl QualityScore {
    fn sanitize_component(value: f32) -> f32 {
        if !value.is_finite() {
            return 0.0;
        }
        value.clamp(0.0, 1.0)
    }

    /// Create a new quality score.
    #[must_use]
    pub fn new(accuracy: f32, relevance: f32, completeness: f32) -> Self {
        Self {
            accuracy: Self::sanitize_component(accuracy),
            relevance: Self::sanitize_component(relevance),
            completeness: Self::sanitize_component(completeness),
        }
    }

    /// Average score across all dimensions.
    #[must_use]
    pub fn average(&self) -> f32 {
        let accuracy = Self::sanitize_component(self.accuracy);
        let relevance = Self::sanitize_component(self.relevance);
        let completeness = Self::sanitize_component(self.completeness);
        (accuracy + relevance + completeness) / 3.0
    }

    /// Checks if score meets threshold.
    #[must_use]
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.average() + f32::EPSILON >= threshold
    }
}

/// Result of quality gate evaluation.
///
/// Contains the response along with quality metadata, regardless of whether
/// the quality threshold was met. This allows handling both success and
/// failure cases with full context.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::quality::{QualityGate, QualityGateConfig, QualityGateResult};
///
/// let result = quality_gate.evaluate(response).await?;
///
/// match result {
///     QualityGateResult::Passed { response, score, attempts } => {
///         println!("Passed on attempt {} with score {:.2}", attempts, score.average());
///         use_response(response);
///     }
///     QualityGateResult::Failed { response, score, attempts, reason } => {
///         println!("Failed after {} attempts: {}", attempts, reason);
///         // Still have access to best-effort response
///         handle_low_quality(response, score);
///     }
/// }
/// ```
///
/// # See Also
///
/// - [`QualityGate`] - The gate that produces this result
/// - [`QualityScore`] - The multi-dimensional quality score
/// - [`QualityGateConfig`] - Configuration for retry behavior
#[derive(Debug)]
#[non_exhaustive]
pub enum QualityGateResult<T> {
    /// Response met quality threshold
    Passed {
        /// The response that passed quality checks
        response: T,
        /// Quality scores achieved
        score: QualityScore,
        /// Number of attempts before passing
        attempts: usize,
    },

    /// Response failed quality check after max retries
    Failed {
        /// Best-effort response (may still be usable)
        response: T,
        /// Final quality scores (below threshold)
        score: QualityScore,
        /// Total attempts made
        attempts: usize,
        /// Explanation of why quality check failed
        reason: String,
    },
}

impl<T> QualityGateResult<T> {
    /// Extracts the response regardless of pass/fail.
    #[must_use]
    pub fn into_response(self) -> T {
        match self {
            Self::Passed { response, .. } | Self::Failed { response, .. } => response,
        }
    }

    /// Gets the score.
    pub fn score(&self) -> QualityScore {
        match self {
            Self::Passed { score, .. } | Self::Failed { score, .. } => *score,
        }
    }

    /// Number of attempts made.
    pub fn attempts(&self) -> usize {
        match self {
            Self::Passed { attempts, .. } | Self::Failed { attempts, .. } => *attempts,
        }
    }

    /// Whether quality gate passed.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Passed { .. })
    }
}

/// Quality gate that automatically retries until quality threshold is met.
///
/// This is a core architectural innovation. Instead of hoping the LLM produces
/// high-quality output, we GUARANTEE it by using a retry loop.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::quality::{QualityGate, QualityGateConfig, QualityScore};
///
/// let config = QualityGateConfig {
///     threshold: 0.95,
///     max_retries: 3,
///     ..Default::default()
/// };
///
/// let gate = QualityGate::new(config);
///
/// // Attempt to generate response with quality guarantee
/// let result = gate.check_with_retry(
///     |attempt| async move {
///         // Generate response (with stronger prompt on retries)
///         let response = agent.invoke(state).await?;
///         Ok(response)
///     },
///     |response| async move {
///         // Judge the response
///         let score = judge.judge_response(query, response, topics, None, None).await?;
///         Ok(score)
///     }
/// ).await?;
///
/// match result {
///     QualityGateResult::Passed { response, score, attempts } => {
///         println!("Quality {} after {} attempts", score.average(), attempts);
///         Ok(response)
///     }
///     QualityGateResult::Failed { response, score, .. } => {
///         eprintln!("Quality only {} after retries", score.average());
///         Ok(response)  // Use best attempt
///     }
/// }
/// ```
///
/// # See Also
///
/// - [`QualityGateConfig`] - Configuration options for thresholds and retries
/// - [`QualityGateResult`] - Result type with pass/fail status and score
/// - [`QualityScore`] - Multi-dimensional quality scoring
/// - [`RetryPolicy`](crate::core::retry::RetryPolicy) - Lower-level retry configuration
/// - LLM-as-judge patterns for quality assessment (see `optimize::metrics::llm_metric`)
pub struct QualityGate {
    config: QualityGateConfig,
}

impl QualityGate {
    /// Creates a new quality gate with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if configuration validation fails. Use `try_new()` for fallible construction.
    #[must_use]
    pub fn new(config: QualityGateConfig) -> Self {
        config
            .validate()
            .expect("Invalid QualityGateConfig (use try_new() for fallible construction)");
        Self { config }
    }

    /// Creates a new quality gate with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails:
    /// - `max_retries` must be 1-100
    /// - `threshold` must be 0.0-1.0
    pub fn try_new(config: QualityGateConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Creates a quality gate with default configuration.
    #[must_use]
    pub fn default_gate() -> Self {
        Self::new(QualityGateConfig::default())
    }

    /// Creates a quality gate with custom threshold.
    ///
    /// # Panics
    ///
    /// Panics if threshold is not in range 0.0-1.0.
    #[must_use]
    pub fn with_threshold(threshold: f32) -> Self {
        Self::new(QualityGateConfig {
            threshold,
            ..Default::default()
        })
    }

    /// Attempts to generate a response with quality guarantee.
    ///
    /// # Arguments
    ///
    /// * `generate_fn` - Async function that generates a response. Receives attempt number.
    /// * `judge_fn` - Async function that judges the response quality.
    ///
    /// # Returns
    ///
    /// * `QualityGateResult::Passed` if quality threshold met
    /// * `QualityGateResult::Failed` if max retries exceeded
    ///
    /// # Type Parameters
    ///
    /// * `T` - Response type
    /// * `E` - Error type
    /// * `F` - Generate function type
    /// * `J` - Judge function type
    pub async fn check_with_retry<T, E, F, J>(
        &self,
        mut generate_fn: F,
        mut judge_fn: J,
    ) -> Result<QualityGateResult<T>, E>
    where
        T: Clone,
        E: std::error::Error,
        F: FnMut(usize) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>> + Send,
        J: FnMut(&T) -> Pin<Box<dyn Future<Output = Result<QualityScore, E>> + Send>> + Send,
    {
        let mut best_response = None;
        let mut best_score = None;

        for attempt in 1..=self.config.max_retries {
            // Acquire rate limiter permission before making API calls.
            // This prevents thundering herd when many concurrent quality gates retry.
            if let Some(ref limiter) = self.config.rate_limiter {
                limiter.acquire().await;
            }

            // Generate response
            let response = generate_fn(attempt).await?;

            // Judge quality
            let score = judge_fn(&response).await?;

            // Track best attempt
            if best_score.map_or(true, |s: QualityScore| score.average() > s.average()) {
                best_response = Some(response.clone());
                best_score = Some(score);
            }

            // Check if quality threshold met
            if score.meets_threshold(self.config.threshold) {
                return Ok(QualityGateResult::Passed {
                    response,
                    score,
                    attempts: attempt,
                });
            }

            // Log retry
            if attempt < self.config.max_retries {
                tracing::info!(
                    "Quality gate: attempt {} scored {:.2} (threshold {:.2}), retrying...",
                    attempt,
                    score.average(),
                    self.config.threshold
                );
            }
        }

        // Max retries exceeded - return best attempt
        let response = best_response.expect("At least one attempt should have been made");
        let score = best_score.expect("At least one score should exist");

        Ok(QualityGateResult::Failed {
            response,
            score,
            attempts: self.config.max_retries,
            reason: format!(
                "Quality {:.2} below threshold {:.2} after {} attempts",
                score.average(),
                self.config.threshold,
                self.config.max_retries
            ),
        })
    }

    /// Gets the configured threshold.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.config.threshold
    }

    /// Gets the configured max retries.
    #[must_use]
    pub fn max_retries(&self) -> usize {
        self.config.max_retries
    }

    /// Gets the configured retry strategy.
    ///
    /// Use this to implement strategy-aware behavior in your generate_fn.
    /// For example, on retry you might:
    /// - `StrongerPrompt`: Add emphasis on quality requirements
    /// - `UpgradeModel`: Switch to a more capable model
    /// - `RefineResponse`: Use the previous response as input for refinement
    #[must_use]
    pub fn retry_strategy(&self) -> RetryStrategy {
        self.config.retry_strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[tokio::test]
    async fn test_quality_gate_passes_immediately() {
        let config = QualityGateConfig {
            threshold: 0.90,
            max_retries: 3,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let mut generate_count = 0;
        let result = gate
            .check_with_retry(
                |_attempt| {
                    generate_count += 1;
                    Box::pin(async move { Ok::<_, io::Error>("High quality response".to_string()) })
                },
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.95,
                            relevance: 0.95,
                            completeness: 0.95,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 1);
        assert_eq!(generate_count, 1);
        assert!(result.score().average() >= 0.90);
    }

    #[tokio::test]
    async fn test_quality_gate_retries_until_success() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = QualityGateConfig {
            threshold: 0.90,
            max_retries: 3,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let attempt_num = Arc::new(AtomicUsize::new(0));
        let attempt_num_gen = attempt_num.clone();
        let attempt_num_judge = attempt_num.clone();

        let result = gate
            .check_with_retry(
                move |_attempt| {
                    let num = attempt_num_gen.fetch_add(1, Ordering::SeqCst) + 1;
                    Box::pin(async move { Ok::<_, io::Error>(format!("Response {}", num)) })
                },
                move |_response| {
                    let num = attempt_num_judge.load(Ordering::SeqCst);
                    Box::pin(async move {
                        // First 2 attempts fail, 3rd succeeds
                        let score = if num < 3 {
                            0.85 // Below threshold
                        } else {
                            0.95 // Above threshold
                        };
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: score,
                            relevance: score,
                            completeness: score,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 3);
    }

    #[tokio::test]
    async fn test_quality_gate_fails_after_max_retries() {
        let config = QualityGateConfig {
            threshold: 0.95,
            max_retries: 2,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>("Low quality".to_string()) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.80,
                            relevance: 0.80,
                            completeness: 0.80,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(!result.passed());
        assert_eq!(result.attempts(), 2);
        assert_eq!(result.score().average(), 0.80);
    }

    #[tokio::test]
    async fn test_quality_gate_returns_best_attempt() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = QualityGateConfig {
            threshold: 0.95,
            max_retries: 3,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let attempt_num = Arc::new(AtomicUsize::new(0));
        let attempt_num_gen = attempt_num.clone();
        let attempt_num_judge = attempt_num.clone();

        let result = gate
            .check_with_retry(
                move |_attempt| {
                    let num = attempt_num_gen.fetch_add(1, Ordering::SeqCst) + 1;
                    Box::pin(async move { Ok::<_, io::Error>(format!("Response {}", num)) })
                },
                move |_response| {
                    let num = attempt_num_judge.load(Ordering::SeqCst);
                    Box::pin(async move {
                        // Scores: 0.85, 0.90, 0.88 (best is 0.90 but all below threshold)
                        let score = match num {
                            1 => 0.85,
                            2 => 0.90,
                            _ => 0.88,
                        };
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: score,
                            relevance: score,
                            completeness: score,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(!result.passed());
        assert!((result.score().average() - 0.90).abs() < 0.01); // Best attempt (within epsilon)
        assert_eq!(result.into_response(), "Response 2");
    }

    #[test]
    fn test_quality_score_average() {
        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.8,
            completeness: 0.85,
        };
        assert!((score.average() - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_quality_score_meets_threshold() {
        let score = QualityScore {
            accuracy: 0.95,
            relevance: 0.95,
            completeness: 0.95,
        };
        assert!(score.meets_threshold(0.90));
        assert!(score.meets_threshold(0.95));
        assert!(!score.meets_threshold(0.96));
    }

    #[test]
    fn test_quality_score_new_sanitizes_inputs() {
        let score = QualityScore::new(-1.0, 2.0, f32::NAN);
        assert_eq!(score.accuracy, 0.0);
        assert_eq!(score.relevance, 1.0);
        assert_eq!(score.completeness, 0.0);
        assert_eq!(score.average(), 1.0 / 3.0);

        let score = QualityScore {
            accuracy: -1.0,
            relevance: 2.0,
            completeness: f32::INFINITY,
        };
        assert_eq!(score.average(), 1.0 / 3.0);
        assert!(score.meets_threshold(0.333_333));
        assert!(!score.meets_threshold(0.333_334));
    }

    #[test]
    fn test_quality_gate_result_methods() {
        let result = QualityGateResult::Passed {
            response: "test".to_string(),
            score: QualityScore {
                accuracy: 0.95,
                relevance: 0.95,
                completeness: 0.95,
            },
            attempts: 2,
        };

        assert!(result.passed());
        assert_eq!(result.attempts(), 2);
        assert_eq!(result.score().average(), 0.95);
        assert_eq!(result.into_response(), "test");
    }

    // ============================================================================
    // Additional comprehensive tests for quality_gate.rs
    // ============================================================================

    // ------------------------------------------------------------------------
    // QualityGateConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_gate_config_default() {
        let config = QualityGateConfig::default();

        assert!((config.threshold - 0.95).abs() < 1e-6);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_strategy, RetryStrategy::StrongerPrompt);
        assert!(config.emit_telemetry);
    }

    #[test]
    fn test_quality_gate_config_custom() {
        let config = QualityGateConfig {
            threshold: 0.99,
            max_retries: 5,
            retry_strategy: RetryStrategy::UpgradeModel,
            emit_telemetry: false,
            rate_limiter: None,
        };

        assert!((config.threshold - 0.99).abs() < 1e-6);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.retry_strategy, RetryStrategy::UpgradeModel);
        assert!(!config.emit_telemetry);
    }

    #[test]
    fn test_quality_gate_config_clone() {
        let config = QualityGateConfig {
            threshold: 0.85,
            max_retries: 10,
            retry_strategy: RetryStrategy::RefineResponse,
            emit_telemetry: true,
            rate_limiter: None,
        };

        let cloned = config.clone();

        assert!((config.threshold - cloned.threshold).abs() < 1e-6);
        assert_eq!(config.max_retries, cloned.max_retries);
        assert_eq!(config.retry_strategy, cloned.retry_strategy);
        assert_eq!(config.emit_telemetry, cloned.emit_telemetry);
    }

    #[test]
    fn test_quality_gate_config_debug() {
        let config = QualityGateConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("QualityGateConfig"));
        assert!(debug_str.contains("threshold"));
        assert!(debug_str.contains("max_retries"));
    }

    // ------------------------------------------------------------------------
    // RetryStrategy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_retry_strategy_equality() {
        assert_eq!(RetryStrategy::SameInputs, RetryStrategy::SameInputs);
        assert_eq!(RetryStrategy::StrongerPrompt, RetryStrategy::StrongerPrompt);
        assert_eq!(RetryStrategy::UpgradeModel, RetryStrategy::UpgradeModel);
        assert_eq!(RetryStrategy::RefineResponse, RetryStrategy::RefineResponse);
        assert_eq!(RetryStrategy::ForceToolCall, RetryStrategy::ForceToolCall);

        assert_ne!(RetryStrategy::SameInputs, RetryStrategy::StrongerPrompt);
        assert_ne!(RetryStrategy::UpgradeModel, RetryStrategy::RefineResponse);
    }

    #[test]
    fn test_retry_strategy_clone() {
        let strategy = RetryStrategy::UpgradeModel;
        let cloned = strategy;

        assert_eq!(strategy, cloned);
    }

    #[test]
    fn test_retry_strategy_debug() {
        let strategy = RetryStrategy::ForceToolCall;
        let debug_str = format!("{:?}", strategy);

        assert!(debug_str.contains("ForceToolCall"));
    }

    // ------------------------------------------------------------------------
    // QualityScore Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_score_average_equal_values() {
        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
        };

        assert!((score.average() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_average_varied_values() {
        let score = QualityScore {
            accuracy: 0.8,
            relevance: 0.9,
            completeness: 1.0,
        };

        assert!((score.average() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_average_zero() {
        let score = QualityScore {
            accuracy: 0.0,
            relevance: 0.0,
            completeness: 0.0,
        };

        assert!((score.average()).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_average_max() {
        let score = QualityScore {
            accuracy: 1.0,
            relevance: 1.0,
            completeness: 1.0,
        };

        assert!((score.average() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_meets_threshold_exact() {
        let score = QualityScore {
            accuracy: 0.95,
            relevance: 0.95,
            completeness: 0.95,
        };

        assert!(score.meets_threshold(0.95));
    }

    #[test]
    fn test_quality_score_meets_threshold_above() {
        let score = QualityScore {
            accuracy: 0.99,
            relevance: 0.99,
            completeness: 0.99,
        };

        assert!(score.meets_threshold(0.90));
        assert!(score.meets_threshold(0.95));
        assert!(score.meets_threshold(0.98));
    }

    #[test]
    fn test_quality_score_meets_threshold_below() {
        let score = QualityScore {
            accuracy: 0.80,
            relevance: 0.80,
            completeness: 0.80,
        };

        assert!(!score.meets_threshold(0.85));
        assert!(!score.meets_threshold(0.90));
    }

    #[test]
    fn test_quality_score_meets_threshold_zero() {
        let score = QualityScore {
            accuracy: 0.5,
            relevance: 0.5,
            completeness: 0.5,
        };

        assert!(score.meets_threshold(0.0));
    }

    #[test]
    fn test_quality_score_clone() {
        let score = QualityScore {
            accuracy: 0.85,
            relevance: 0.90,
            completeness: 0.88,
        };

        let cloned = score;

        assert!((score.accuracy - cloned.accuracy).abs() < 1e-6);
        assert!((score.relevance - cloned.relevance).abs() < 1e-6);
        assert!((score.completeness - cloned.completeness).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_debug() {
        let score = QualityScore {
            accuracy: 0.95,
            relevance: 0.92,
            completeness: 0.98,
        };

        let debug_str = format!("{:?}", score);

        assert!(debug_str.contains("QualityScore"));
        assert!(debug_str.contains("accuracy"));
        assert!(debug_str.contains("relevance"));
        assert!(debug_str.contains("completeness"));
    }

    // ------------------------------------------------------------------------
    // QualityGateResult Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_gate_result_passed() {
        let result: QualityGateResult<String> = QualityGateResult::Passed {
            response: "success".to_string(),
            score: QualityScore {
                accuracy: 0.98,
                relevance: 0.97,
                completeness: 0.99,
            },
            attempts: 1,
        };

        assert!(result.passed());
        assert_eq!(result.attempts(), 1);
    }

    #[test]
    fn test_quality_gate_result_failed() {
        let result: QualityGateResult<String> = QualityGateResult::Failed {
            response: "best_attempt".to_string(),
            score: QualityScore {
                accuracy: 0.80,
                relevance: 0.82,
                completeness: 0.78,
            },
            attempts: 3,
            reason: "Below threshold".to_string(),
        };

        assert!(!result.passed());
        assert_eq!(result.attempts(), 3);
    }

    #[test]
    fn test_quality_gate_result_into_response_passed() {
        let result: QualityGateResult<String> = QualityGateResult::Passed {
            response: "passed_response".to_string(),
            score: QualityScore {
                accuracy: 0.95,
                relevance: 0.95,
                completeness: 0.95,
            },
            attempts: 2,
        };

        assert_eq!(result.into_response(), "passed_response");
    }

    #[test]
    fn test_quality_gate_result_into_response_failed() {
        let result: QualityGateResult<String> = QualityGateResult::Failed {
            response: "failed_response".to_string(),
            score: QualityScore {
                accuracy: 0.70,
                relevance: 0.70,
                completeness: 0.70,
            },
            attempts: 5,
            reason: "Max retries exceeded".to_string(),
        };

        assert_eq!(result.into_response(), "failed_response");
    }

    #[test]
    fn test_quality_gate_result_score_passed() {
        let result: QualityGateResult<String> = QualityGateResult::Passed {
            response: "test".to_string(),
            score: QualityScore {
                accuracy: 0.91,
                relevance: 0.92,
                completeness: 0.93,
            },
            attempts: 1,
        };

        let score = result.score();
        assert!((score.accuracy - 0.91).abs() < 1e-6);
        assert!((score.relevance - 0.92).abs() < 1e-6);
        assert!((score.completeness - 0.93).abs() < 1e-6);
    }

    #[test]
    fn test_quality_gate_result_score_failed() {
        let result: QualityGateResult<String> = QualityGateResult::Failed {
            response: "test".to_string(),
            score: QualityScore {
                accuracy: 0.81,
                relevance: 0.82,
                completeness: 0.83,
            },
            attempts: 3,
            reason: "test".to_string(),
        };

        let score = result.score();
        assert!((score.accuracy - 0.81).abs() < 1e-6);
        assert!((score.relevance - 0.82).abs() < 1e-6);
        assert!((score.completeness - 0.83).abs() < 1e-6);
    }

    #[test]
    fn test_quality_gate_result_debug() {
        let result: QualityGateResult<String> = QualityGateResult::Passed {
            response: "debug_test".to_string(),
            score: QualityScore {
                accuracy: 0.95,
                relevance: 0.95,
                completeness: 0.95,
            },
            attempts: 1,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Passed"));
        assert!(debug_str.contains("debug_test"));
    }

    // ------------------------------------------------------------------------
    // QualityGate Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_gate_new() {
        let config = QualityGateConfig {
            threshold: 0.92,
            max_retries: 4,
            ..Default::default()
        };

        let gate = QualityGate::new(config);

        assert!((gate.threshold() - 0.92).abs() < 1e-6);
        assert_eq!(gate.max_retries(), 4);
    }

    #[test]
    fn test_quality_gate_default_gate() {
        let gate = QualityGate::default_gate();

        assert!((gate.threshold() - 0.95).abs() < 1e-6);
        assert_eq!(gate.max_retries(), 3);
    }

    #[test]
    fn test_quality_gate_with_threshold() {
        let gate = QualityGate::with_threshold(0.85);

        assert!((gate.threshold() - 0.85).abs() < 1e-6);
        assert_eq!(gate.max_retries(), 3); // Default
    }

    #[test]
    fn test_quality_gate_threshold_getter() {
        let config = QualityGateConfig {
            threshold: 0.88,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        assert!((gate.threshold() - 0.88).abs() < 1e-6);
    }

    #[test]
    fn test_quality_gate_max_retries_getter() {
        let config = QualityGateConfig {
            max_retries: 7,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        assert_eq!(gate.max_retries(), 7);
    }

    #[test]
    fn test_quality_gate_retry_strategy_getter() {
        let config = QualityGateConfig {
            retry_strategy: RetryStrategy::UpgradeModel,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        assert_eq!(gate.retry_strategy(), RetryStrategy::UpgradeModel);
    }

    #[tokio::test]
    async fn test_quality_gate_single_retry() {
        let config = QualityGateConfig {
            threshold: 0.95,
            max_retries: 1,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>("response".to_string()) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.80,
                            relevance: 0.80,
                            completeness: 0.80,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(!result.passed());
        assert_eq!(result.attempts(), 1);
    }

    #[tokio::test]
    async fn test_quality_gate_low_threshold() {
        let config = QualityGateConfig {
            threshold: 0.50,
            max_retries: 3,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>("response".to_string()) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.60,
                            relevance: 0.60,
                            completeness: 0.60,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 1);
    }

    #[tokio::test]
    async fn test_quality_gate_many_retries() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = QualityGateConfig {
            threshold: 0.95,
            max_retries: 10,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let count = Arc::new(AtomicUsize::new(0));
        let count_gen = count.clone();
        let count_judge = count.clone();

        let result = gate
            .check_with_retry(
                move |_attempt| {
                    count_gen.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move { Ok::<_, io::Error>("response".to_string()) })
                },
                move |_response| {
                    let n = count_judge.load(Ordering::SeqCst);
                    Box::pin(async move {
                        // Pass on 8th attempt
                        let score = if n >= 8 { 0.98 } else { 0.80 };
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: score,
                            relevance: score,
                            completeness: score,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 8);
    }

    #[tokio::test]
    async fn test_quality_gate_with_varied_scores() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = QualityGateConfig {
            threshold: 0.90,
            max_retries: 4,
            ..Default::default()
        };
        let gate = QualityGate::new(config);

        let attempt = Arc::new(AtomicUsize::new(0));
        let attempt_judge = attempt.clone();

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>("response".to_string()) }),
                move |_response| {
                    let n = attempt_judge.fetch_add(1, Ordering::SeqCst) + 1;
                    Box::pin(async move {
                        // Variable scores: accuracy differs each time
                        let score = match n {
                            1 => QualityScore {
                                accuracy: 0.70,
                                relevance: 0.80,
                                completeness: 0.75,
                            },
                            2 => QualityScore {
                                accuracy: 0.85,
                                relevance: 0.85,
                                completeness: 0.85,
                            },
                            _ => QualityScore {
                                accuracy: 0.95,
                                relevance: 0.92,
                                completeness: 0.93,
                            },
                        };
                        Ok::<_, io::Error>(score)
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 3);
    }

    #[tokio::test]
    async fn test_quality_gate_perfect_score_first_try() {
        let gate = QualityGate::with_threshold(0.99);

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>("perfect".to_string()) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 1.0,
                            relevance: 1.0,
                            completeness: 1.0,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 1);
        assert!((result.score().average() - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_quality_gate_with_different_response_types() {
        let gate = QualityGate::with_threshold(0.90);

        // Test with i32
        let result_i32 = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>(42i32) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.95,
                            relevance: 0.95,
                            completeness: 0.95,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result_i32.passed());
        assert_eq!(result_i32.into_response(), 42);
    }

    #[tokio::test]
    async fn test_quality_gate_with_vec_response() {
        let gate = QualityGate::with_threshold(0.90);

        let result = gate
            .check_with_retry(
                |_attempt| Box::pin(async move { Ok::<_, io::Error>(vec![1, 2, 3]) }),
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.95,
                            relevance: 0.95,
                            completeness: 0.95,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.into_response(), vec![1, 2, 3]);
    }

    // ------------------------------------------------------------------------
    // Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_score_boundary_values() {
        // Test boundary at exactly 0.0
        let zero_score = QualityScore {
            accuracy: 0.0,
            relevance: 0.0,
            completeness: 0.0,
        };
        assert!(zero_score.meets_threshold(0.0));
        assert!(!zero_score.meets_threshold(0.001));

        // Test boundary at exactly 1.0
        let max_score = QualityScore {
            accuracy: 1.0,
            relevance: 1.0,
            completeness: 1.0,
        };
        assert!(max_score.meets_threshold(1.0));
    }

    #[test]
    fn test_quality_score_asymmetric_values() {
        let score = QualityScore {
            accuracy: 0.99,
            relevance: 0.50,
            completeness: 0.71,
        };

        // Average = (0.99 + 0.50 + 0.71) / 3 = 0.733...
        let avg = score.average();
        assert!((avg - 0.7333333).abs() < 0.001);
    }

    #[test]
    fn test_config_with_extreme_values() {
        let config = QualityGateConfig {
            threshold: 1.0,
            max_retries: 100,
            retry_strategy: RetryStrategy::SameInputs,
            emit_telemetry: false,
            rate_limiter: None,
        };

        assert!((config.threshold - 1.0).abs() < 1e-6);
        assert_eq!(config.max_retries, 100);
    }

    #[test]
    fn test_config_with_zero_threshold() {
        let config = QualityGateConfig {
            threshold: 0.0,
            max_retries: 1,
            ..Default::default()
        };

        assert!((config.threshold).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------
    // Configuration Validation Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_quality_gate_config_validate_valid() {
        let config = QualityGateConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_quality_gate_config_validate_zero_retries() {
        let config = QualityGateConfig {
            max_retries: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("max_retries must be at least 1"));
    }

    #[test]
    fn test_quality_gate_config_validate_too_many_retries() {
        let config = QualityGateConfig {
            max_retries: 101,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max_retries too large"));
    }

    #[test]
    fn test_quality_gate_config_validate_threshold_below_zero() {
        let config = QualityGateConfig {
            threshold: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("threshold must be 0.0-1.0"));
    }

    #[test]
    fn test_quality_gate_config_validate_threshold_above_one() {
        let config = QualityGateConfig {
            threshold: 1.5,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("threshold must be 0.0-1.0"));
    }

    #[test]
    fn test_quality_gate_config_validate_boundary_values() {
        // max_retries = 1 is valid
        let config = QualityGateConfig {
            max_retries: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // max_retries = 100 is valid
        let config = QualityGateConfig {
            max_retries: 100,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // threshold = 0.0 is valid
        let config = QualityGateConfig {
            threshold: 0.0,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // threshold = 1.0 is valid
        let config = QualityGateConfig {
            threshold: 1.0,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_quality_gate_try_new_valid() {
        let config = QualityGateConfig::default();
        let result = QualityGate::try_new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quality_gate_try_new_invalid() {
        let config = QualityGateConfig {
            max_retries: 0,
            ..Default::default()
        };
        let result = QualityGate::try_new(config);
        assert!(result.is_err());
    }

    // Note: The `new()` panicking behavior is covered by `test_quality_gate_try_new_invalid()`
    // which tests the same validation via the Result-returning `try_new()`.

    // ============================================================================
    // Rate Limiter Integration Tests
    // ============================================================================

    use crate::core::rate_limiters::InMemoryRateLimiter;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn test_quality_gate_with_rate_limiter() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Create a rate limiter that allows 10 requests per second
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let config = QualityGateConfig {
            threshold: 0.90,
            max_retries: 3,
            ..Default::default()
        }
        .with_rate_limiter(limiter);

        let gate = QualityGate::new(config);

        let attempt_counter = Arc::new(AtomicUsize::new(0));
        let attempt_counter_clone = attempt_counter.clone();

        let result = gate
            .check_with_retry(
                move |_attempt| {
                    attempt_counter_clone.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move { Ok::<_, io::Error>("response".to_string()) })
                },
                |_response| {
                    Box::pin(async move {
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.95,
                            relevance: 0.95,
                            completeness: 0.95,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert!(result.passed());
        assert_eq!(result.attempts(), 1);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_quality_gate_rate_limiter_enforces_rate() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Create a slow rate limiter: 2 requests per second
        let limiter = Arc::new(InMemoryRateLimiter::new(
            2.0,
            Duration::from_millis(10),
            2.0,
        ));

        let config = QualityGateConfig {
            threshold: 0.99, // Very high threshold - will fail all attempts
            max_retries: 3,
            ..Default::default()
        }
        .with_rate_limiter(limiter);

        let gate = QualityGate::new(config);

        let start = Instant::now();
        let attempt_counter = Arc::new(AtomicUsize::new(0));
        let attempt_counter_clone = attempt_counter.clone();

        let result = gate
            .check_with_retry(
                move |_attempt| {
                    attempt_counter_clone.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move { Ok::<_, io::Error>("response".to_string()) })
                },
                |_response| {
                    Box::pin(async move {
                        // Always return score below threshold
                        Ok::<_, io::Error>(QualityScore {
                            accuracy: 0.80,
                            relevance: 0.80,
                            completeness: 0.80,
                        })
                    })
                },
            )
            .await
            .unwrap();

        let elapsed = start.elapsed();

        assert!(!result.passed());
        assert_eq!(result.attempts(), 3);

        // With 2 req/sec and 3 attempts, should take ~1-1.5s
        assert!(
            elapsed >= Duration::from_millis(500),
            "Rate limiting should enforce minimum delay, but took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_quality_gate_config_with_rate_limiter_builder() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            100.0,
            Duration::from_millis(10),
            10.0,
        ));

        let config = QualityGateConfig::default().with_rate_limiter(limiter.clone());

        assert!(config.rate_limiter.is_some());
    }

    #[test]
    fn test_quality_gate_config_debug_with_rate_limiter() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let config = QualityGateConfig::default().with_rate_limiter(limiter);
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("QualityGateConfig"));
        assert!(debug_str.contains("<RateLimiter>"));
    }

    #[test]
    fn test_quality_gate_config_clone_with_rate_limiter() {
        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(10),
            10.0,
        ));

        let config = QualityGateConfig::default().with_rate_limiter(limiter);
        let cloned = config.clone();

        assert!(cloned.rate_limiter.is_some());
    }
}
