// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Evaluation Metrics
//!
//! Defines metrics for scoring `DashFlow` application runs.
//!
//! Metrics are organized into three categories:
//! - **Quality**: Correctness, relevance, safety, hallucination rate
//! - **Performance**: Latency, success rate, error rate
//! - **Cost**: Token usage, API costs, tool calls

use serde::{Deserialize, Serialize};

/// Evaluation metrics for a `DashFlow` run
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::EvalMetrics;
///
/// let metrics = EvalMetrics {
///     correctness: Some(0.95),
///     relevance: Some(0.88),
///     safety: Some(1.0),
///     hallucination_rate: Some(0.02),
///     p95_latency: 1850.0,
///     avg_latency: 1200.0,
///     success_rate: 1.0,
///     error_rate: 0.0,
///     total_tokens: 150,
///     cost_per_run: 0.00045,
///     tool_calls: 1,
/// };
///
/// assert!(metrics.correctness.unwrap() > 0.9);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMetrics {
    // Quality metrics (0.0-1.0 scores, optional if not evaluated)
    /// Correctness score: Did the app produce correct output?
    pub correctness: Option<f64>,

    /// Relevance score: Are retrieved documents relevant to query?
    pub relevance: Option<f64>,

    /// Safety score: Does output contain harmful content?
    pub safety: Option<f64>,

    /// Hallucination rate: Percentage of made-up facts (0.0-1.0)
    pub hallucination_rate: Option<f64>,

    // Performance metrics (always present)
    /// 95th percentile latency (milliseconds)
    pub p95_latency: f64,

    /// Average latency (milliseconds)
    pub avg_latency: f64,

    /// Success rate (0.0-1.0)
    pub success_rate: f64,

    /// Error rate (0.0-1.0)
    pub error_rate: f64,

    // Cost metrics (always present)
    /// Total tokens used (prompt + completion)
    pub total_tokens: u64,

    /// Estimated cost per run (USD)
    pub cost_per_run: f64,

    /// Number of tool calls
    pub tool_calls: u64,
}

impl EvalMetrics {
    /// Create new metrics with default performance/cost values
    ///
    /// Quality metrics are set to None (not evaluated).
    #[must_use]
    pub fn new() -> Self {
        Self {
            correctness: None,
            relevance: None,
            safety: None,
            hallucination_rate: None,
            p95_latency: 0.0,
            avg_latency: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            total_tokens: 0,
            cost_per_run: 0.0,
            tool_calls: 0,
        }
    }

    /// Check if all quality metrics pass thresholds
    ///
    /// Returns true if:
    /// - correctness >= threshold (if present)
    /// - relevance >= threshold (if present)
    /// - safety >= threshold (if present)
    /// - `hallucination_rate` <= threshold (if present)
    #[must_use]
    pub fn quality_passes(&self, threshold: f64) -> bool {
        if let Some(c) = self.correctness {
            if c < threshold {
                return false;
            }
        }
        if let Some(r) = self.relevance {
            if r < threshold {
                return false;
            }
        }
        if let Some(s) = self.safety {
            if s < threshold {
                return false;
            }
        }
        if let Some(h) = self.hallucination_rate {
            if h > threshold {
                return false;
            }
        }
        true
    }

    /// Check if performance metrics are acceptable
    ///
    /// Returns true if:
    /// - `success_rate` >= `min_success_rate`
    /// - `error_rate` <= `max_error_rate`
    /// - `p95_latency` <= `max_p95_latency`
    #[must_use]
    pub fn performance_passes(
        &self,
        min_success_rate: f64,
        max_error_rate: f64,
        max_p95_latency: f64,
    ) -> bool {
        self.success_rate >= min_success_rate
            && self.error_rate <= max_error_rate
            && self.p95_latency <= max_p95_latency
    }

    /// Check if cost metrics are acceptable
    ///
    /// Returns true if:
    /// - `total_tokens` <= `max_tokens`
    /// - `cost_per_run` <= `max_cost`
    #[must_use]
    pub fn cost_passes(&self, max_tokens: u64, max_cost: f64) -> bool {
        self.total_tokens <= max_tokens && self.cost_per_run <= max_cost
    }
}

impl Default for EvalMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Pricing for LLM API costs
///
/// Prices are in USD per 1M tokens.
#[derive(Debug, Clone, Copy)]
pub struct LlmPricing {
    /// Input/prompt token price (USD per 1M tokens)
    pub input_price: f64,

    /// Output/completion token price (USD per 1M tokens)
    pub output_price: f64,
}

impl LlmPricing {
    /// GPT-4o pricing (as of Nov 2025)
    pub const GPT_4O: Self = Self {
        input_price: 2.50,
        output_price: 10.00,
    };

    /// GPT-3.5-turbo pricing
    pub const GPT_35_TURBO: Self = Self {
        input_price: 0.50,
        output_price: 1.50,
    };

    /// Claude 3.5 Sonnet pricing
    pub const CLAUDE_35_SONNET: Self = Self {
        input_price: 3.00,
        output_price: 15.00,
    };

    /// Calculate cost for given token counts
    ///
    /// # Arguments
    ///
    /// * `prompt_tokens` - Number of input/prompt tokens
    /// * `completion_tokens` - Number of output/completion tokens
    ///
    /// # Returns
    ///
    /// Total cost in USD
    #[must_use]
    pub fn calculate_cost(self, prompt_tokens: u64, completion_tokens: u64) -> f64 {
        let input_cost = (prompt_tokens as f64 / 1_000_000.0) * self.input_price;
        let output_cost = (completion_tokens as f64 / 1_000_000.0) * self.output_price;
        input_cost + output_cost
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_metrics_default() {
        let metrics = EvalMetrics::default();
        assert_eq!(metrics.success_rate, 1.0);
        assert_eq!(metrics.error_rate, 0.0);
        assert_eq!(metrics.total_tokens, 0);
    }

    #[test]
    fn test_quality_passes() {
        let metrics = EvalMetrics {
            correctness: Some(0.95),
            relevance: Some(0.88),
            ..Default::default()
        };

        assert!(metrics.quality_passes(0.80));
        assert!(!metrics.quality_passes(0.90));
    }

    #[test]
    fn test_performance_passes() {
        let metrics = EvalMetrics {
            p95_latency: 1500.0,
            success_rate: 0.98,
            error_rate: 0.02,
            ..Default::default()
        };

        assert!(metrics.performance_passes(0.95, 0.05, 2000.0));
        assert!(!metrics.performance_passes(0.99, 0.01, 1000.0));
    }

    #[test]
    fn test_cost_passes() {
        let metrics = EvalMetrics {
            total_tokens: 150,
            cost_per_run: 0.00045,
            ..Default::default()
        };

        assert!(metrics.cost_passes(200, 0.001));
        assert!(!metrics.cost_passes(100, 0.0001));
    }

    #[test]
    fn test_llm_pricing_gpt4o() {
        let cost = LlmPricing::GPT_4O.calculate_cost(1000, 500);
        // (1000/1M * $2.50) + (500/1M * $10.00) = $0.0025 + $0.005 = $0.0075
        assert!((cost - 0.0075).abs() < 0.0001);
    }

    #[test]
    fn test_llm_pricing_gpt35() {
        let cost = LlmPricing::GPT_35_TURBO.calculate_cost(1000, 500);
        // (1000/1M * $0.50) + (500/1M * $1.50) = $0.0005 + $0.00075 = $0.00125
        assert!((cost - 0.00125).abs() < 0.0001);
    }
}
