// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Quality Gate - Self-Correcting Agent Wrapper
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox
//
//! # Quality Gate: Guaranteed 100% Quality
//!
//! This module implements self-correcting retry loops to guarantee response quality.
//! Based on patterns from CLAUDE.md that achieved:
//! - 100% success rate (100/100 scenarios)
//! - 0.904 avg quality (target: 0.90)
//! - 0.03 avg retries (target: <1.5)
//!
//! ## Pattern: Self-Correcting Retry Loop
//!
//! ```text
//! Query → Agent → Response → Judge → Quality Check
//!                               ↓
//!                          < threshold?
//!                              YES ↓
//!                     Add feedback → Retry
//!                              NO ↓
//!                           Return ✅
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::quality_gate::{QualityGate, QualityConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = QualityConfig {
//!     quality_threshold: 0.90,
//!     max_retries: 3,
//!     ..Default::default()
//! };
//!
//! let gate = QualityGate::new(config);
//!
//! let response = gate.execute_with_quality_guarantee(
//!     "What is Send and Sync in Rust?",
//!     |query| async move {
//!         // Your agent execution here
//!         Ok("Send and Sync are traits...".to_string())
//!     }
//! ).await?;
//!
//! // Guaranteed: response.quality >= 0.90
//! # Ok(())
//! # }
//! ```

use crate::quality::{QualityJudge, QualityScore};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Hard timeout for judge calls to prevent hangs in production.
/// Used by both `execute_with_quality_guarantee()` and `validate_quality()`.
///
/// Note: Matches `dashflow::constants::DEFAULT_HTTP_REQUEST_TIMEOUT` (30s).
const JUDGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Hard timeout for agent execution to prevent hangs.
///
/// Note: Matches `dashflow::constants::LONG_TIMEOUT` (60s).
const EXECUTION_TIMEOUT: Duration = Duration::from_secs(60);

/// Configuration for quality gate
#[derive(Clone)]
pub struct QualityConfig {
    /// Minimum quality threshold (0.0-1.0)
    pub quality_threshold: f32,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Enable verbose logging
    pub verbose: bool,

    /// Judge for quality evaluation
    pub judge: Option<Arc<dyn QualityJudge>>,
}

impl std::fmt::Debug for QualityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QualityConfig")
            .field("quality_threshold", &self.quality_threshold)
            .field("max_retries", &self.max_retries)
            .field("verbose", &self.verbose)
            .field(
                "judge",
                &self.judge.as_ref().map(|_| "Arc<dyn QualityJudge>"),
            )
            .finish()
    }
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: None,
        }
    }
}

impl QualityConfig {
    /// Create configuration with judge
    pub fn with_judge(judge: Arc<dyn QualityJudge>) -> Self {
        Self {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: Some(judge),
        }
    }

    /// Set quality threshold (builder pattern)
    pub fn quality_threshold(mut self, threshold: f32) -> Self {
        self.quality_threshold = threshold;
        self
    }

    /// Set max retries (builder pattern)
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Enable verbose logging (builder pattern)
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// Quality gate with self-correcting retry loops
pub struct QualityGate {
    config: QualityConfig,
}

impl QualityGate {
    /// Create a new quality gate
    pub fn new(config: QualityConfig) -> Self {
        Self { config }
    }

    /// Execute query with quality guarantee
    ///
    /// This function guarantees that the response meets the quality threshold
    /// or returns an error after max_retries attempts.
    ///
    /// # Arguments
    ///
    /// * `query` - The query to execute
    /// * `executor` - Function that executes the query and returns a response
    ///
    /// # Returns
    ///
    /// A response that meets the quality threshold, or an error
    pub async fn execute_with_quality_guarantee<F, Fut>(
        &self,
        original_query: &str,
        executor: F,
    ) -> Result<String, QualityGateError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<String, Box<dyn std::error::Error>>>,
    {
        let judge = self
            .config
            .judge
            .as_ref()
            .ok_or(QualityGateError::NoJudgeConfigured)?;

        let mut query = original_query.to_string();
        let mut retries = 0;

        // Hard timeouts to prevent hangs in production.
        // EXECUTION_TIMEOUT and JUDGE_TIMEOUT are module-level constants.

        loop {
            if self.config.verbose {
                debug!(
                    attempt = retries + 1,
                    max_attempts = self.config.max_retries + 1,
                    query = %query,
                    "Quality gate attempt starting"
                );
            }

            // Execute query with timeout to avoid indefinite hangs.
            let response =
                match tokio::time::timeout(EXECUTION_TIMEOUT, executor(query.clone())).await {
                    Ok(res) => res.map_err(|e| QualityGateError::ExecutionFailed(e.to_string()))?,
                    Err(_) => {
                        return Err(QualityGateError::ExecutionFailed(format!(
                            "Execution timed out after {:?}",
                            EXECUTION_TIMEOUT
                        )))
                    }
                };

            if self.config.verbose {
                let preview: String = response.chars().take(100).collect();
                debug!(
                    response_preview = %preview,
                    "Response received"
                );
            }

            // Judge quality with timeout.
            let score = match tokio::time::timeout(
                JUDGE_TIMEOUT,
                judge.judge_response(original_query, &response, &[], None, None),
            )
            .await
            {
                Ok(res) => res.map_err(|e| QualityGateError::JudgeFailed(e.to_string()))?,
                Err(_) => {
                    return Err(QualityGateError::JudgeFailed(format!(
                        "Judge timed out after {:?}",
                        JUDGE_TIMEOUT
                    )))
                }
            };

            let avg_quality = score.average();

            if self.config.verbose {
                debug!(
                    quality = format!("{:.2}", avg_quality),
                    threshold = format!("{:.2}", self.config.quality_threshold),
                    accuracy = format!("{:.2}", score.accuracy),
                    relevance = format!("{:.2}", score.relevance),
                    completeness = format!("{:.2}", score.completeness),
                    reasoning = %score.reasoning,
                    "Quality score calculated"
                );
            }

            // Check if quality meets threshold
            // Use epsilon tolerance for floating point comparison (fixes 0.8999999 vs 0.9 issue)
            const EPSILON: f32 = 0.001;
            if avg_quality >= self.config.quality_threshold - EPSILON {
                if self.config.verbose {
                    info!(
                        quality = format!("{:.2}", avg_quality),
                        "Quality check passed"
                    );
                }
                return Ok(response);
            }

            // Check retry limit
            if retries >= self.config.max_retries {
                return Err(QualityGateError::QualityThresholdNotMet {
                    attempts: retries + 1,
                    final_quality: avg_quality,
                    threshold: self.config.quality_threshold,
                    last_reasoning: score.reasoning,
                });
            }

            // Prepare retry with feedback
            query = format!(
                "{}\n\n[FEEDBACK] Previous response was insufficient (quality: {:.2}/{:.2}):\n\
                 - Accuracy: {:.2}\n\
                 - Relevance: {:.2}\n\
                 - Completeness: {:.2}\n\
                 - Issue: {}\n\n\
                 Please provide a more accurate, relevant, and complete answer.",
                original_query,
                avg_quality,
                self.config.quality_threshold,
                score.accuracy,
                score.relevance,
                score.completeness,
                score.reasoning
            );

            retries += 1;

            if self.config.verbose {
                debug!(retry = retries, "Retrying with feedback");
            }
        }
    }

    /// Validate response quality without retry
    ///
    /// Useful for checking if a response meets quality threshold.
    /// Uses the same timeout as `execute_with_quality_guarantee()` for consistency.
    pub async fn validate_quality(
        &self,
        query: &str,
        response: &str,
    ) -> Result<QualityScore, QualityGateError> {
        let judge = self
            .config
            .judge
            .as_ref()
            .ok_or(QualityGateError::NoJudgeConfigured)?;

        match tokio::time::timeout(
            JUDGE_TIMEOUT,
            judge.judge_response(query, response, &[], None, None),
        )
        .await
        {
            Ok(res) => res.map_err(|e| QualityGateError::JudgeFailed(e.to_string())),
            Err(_) => Err(QualityGateError::JudgeFailed(format!(
                "Judge timed out after {:?}",
                JUDGE_TIMEOUT
            ))),
        }
    }
}

/// Quality gate errors
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QualityGateError {
    #[error("Quality threshold not met after {attempts} attempts. Final quality: {final_quality:.2}, Threshold: {threshold:.2}. Reason: {last_reasoning}")]
    /// The response failed to reach the configured quality threshold within the retry limit.
    QualityThresholdNotMet {
        /// Number of attempts made (including the final attempt).
        attempts: u32,
        /// Final quality score measured on the last attempt.
        final_quality: f32,
        /// Threshold that was required to pass.
        threshold: f32,
        /// The judge's reasoning from the final attempt.
        last_reasoning: String,
    },

    #[error("Execution failed: {0}")]
    /// The execution failed for reasons unrelated to judging (e.g., tool failure).
    ExecutionFailed(String),

    #[error("Judge evaluation failed: {0}")]
    /// The judge failed to evaluate the response (e.g., network error, timeout).
    JudgeFailed(String),

    #[error("No judge configured. Set QualityConfig::judge before using quality gates.")]
    /// No judge was configured in [`QualityConfig`].
    NoJudgeConfigured,
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::QualityScore;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockJudge {
        /// Number of times judge is called
        call_count: Arc<AtomicU32>,
        /// Quality scores to return in sequence
        scores: Vec<f32>,
    }

    impl MockJudge {
        fn new(scores: Vec<f32>) -> Self {
            Self {
                call_count: Arc::new(AtomicU32::new(0)),
                scores,
            }
        }
    }

    #[async_trait::async_trait]
    impl QualityJudge for MockJudge {
        async fn judge_response(
            &self,
            _query: &str,
            _response: &str,
            _expected_topics: &[&str],
            _context: Option<&str>,
            _tool_results: Option<&str>,
        ) -> Result<QualityScore, Box<dyn std::error::Error>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            let score = self.scores.get(count as usize).copied().unwrap_or(1.0);

            Ok(QualityScore {
                accuracy: score,
                relevance: score,
                completeness: score,
                reasoning: format!("Mock score: {}", score),
            })
        }
    }

    #[tokio::test]
    async fn test_quality_gate_passes_immediately() {
        // Setup: Judge returns high quality on first attempt
        let judge = Arc::new(MockJudge::new(vec![0.95]));
        let config = QualityConfig {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: Some(judge.clone()),
        };

        let gate = QualityGate::new(config);

        // Execute
        let result = gate
            .execute_with_quality_guarantee("test query", |_query| async move {
                Ok("high quality response".to_string())
            })
            .await;

        // Validate
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "high quality response");
        assert_eq!(judge.call_count.load(Ordering::SeqCst), 1); // Only 1 attempt
    }

    #[tokio::test]
    async fn test_quality_gate_retries_then_succeeds() {
        // Setup: Low quality first, then high quality
        let judge = Arc::new(MockJudge::new(vec![0.70, 0.95]));
        let config = QualityConfig {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: Some(judge.clone()),
        };

        let gate = QualityGate::new(config);

        // Execute
        let result = gate
            .execute_with_quality_guarantee("test query", |_query| async move {
                Ok("response".to_string())
            })
            .await;

        // Validate
        assert!(result.is_ok());
        assert_eq!(judge.call_count.load(Ordering::SeqCst), 2); // 2 attempts
    }

    #[tokio::test]
    async fn test_quality_gate_fails_after_max_retries() {
        // Setup: Always low quality
        let judge = Arc::new(MockJudge::new(vec![0.70, 0.75, 0.80, 0.85]));
        let config = QualityConfig {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: Some(judge.clone()),
        };

        let gate = QualityGate::new(config);

        // Execute
        let result = gate
            .execute_with_quality_guarantee("test query", |_query| async move {
                Ok("low quality response".to_string())
            })
            .await;

        // Validate
        assert!(result.is_err());
        match result.unwrap_err() {
            QualityGateError::QualityThresholdNotMet {
                attempts,
                final_quality,
                threshold,
                ..
            } => {
                assert_eq!(attempts, 4); // 1 + 3 retries
                assert!(final_quality < threshold);
            }
            e => panic!("Expected QualityThresholdNotMet, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_quality_gate_without_judge_fails() {
        let config = QualityConfig {
            quality_threshold: 0.90,
            max_retries: 3,
            verbose: false,
            judge: None, // No judge configured
        };

        let gate = QualityGate::new(config);

        // Execute
        let result = gate
            .execute_with_quality_guarantee("test query", |_query| async move {
                Ok("response".to_string())
            })
            .await;

        // Validate
        assert!(matches!(
            result.unwrap_err(),
            QualityGateError::NoJudgeConfigured
        ));
    }
}
