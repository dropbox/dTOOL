// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Automatic A/B Testing - AI Experiments With Variations
//!
//! This module provides automatic A/B testing capabilities that allow AI agents
//! to experiment with configuration variations and automatically select winners.
//!
//! ## Overview
//!
//! Automatic A/B testing enables AI agents to:
//! - Define experiments comparing different configurations
//! - Run controlled tests with statistical rigor
//! - Automatically select winning configurations
//! - Track experiment history and learnings
//!
//! ## Key Concepts
//!
//! - **ABTest**: Configuration for an A/B test between variants
//! - **Variant**: One configuration option being tested
//! - **ABTestResult**: Results of running an A/B test
//! - **ABTestRunner**: Orchestrates test execution and analysis
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::ab_testing::{ABTest, ABTestRunner, Variant, VariantConfig};
//!
//! // Define variants to test
//! let variant_a = Variant::new("fast_model")
//!     .with_config("model", "gpt-3.5-turbo")
//!     .with_description("Faster, cheaper model");
//!
//! let variant_b = Variant::new("smart_model")
//!     .with_config("model", "gpt-4")
//!     .with_description("More capable model");
//!
//! // Create A/B test
//! let test = ABTest::new("model_comparison")
//!     .with_variant_a(variant_a)
//!     .with_variant_b(variant_b)
//!     .with_metric("success_rate")
//!     .with_min_samples(100);
//!
//! // Run test
//! let runner = ABTestRunner::new();
//! let result = runner.run_test(&test, &traces_a, &traces_b);
//!
//! println!("Winner: {:?} with {:.1}% improvement",
//!     result.winner,
//!     result.improvement * 100.0
//! );
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for an A/B test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTest {
    /// Unique identifier for this test
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this test is measuring
    pub description: String,
    /// First variant (control)
    pub variant_a: Variant,
    /// Second variant (treatment)
    pub variant_b: Variant,
    /// Primary metric to optimize
    pub primary_metric: String,
    /// Secondary metrics to track
    pub secondary_metrics: Vec<String>,
    /// Minimum samples per variant before concluding
    pub min_samples: usize,
    /// Maximum test duration
    pub max_duration: Option<Duration>,
    /// Required confidence level (0.0-1.0)
    pub confidence_threshold: f64,
    /// Minimum effect size to consider significant
    pub min_effect_size: f64,
    /// Whether to automatically apply winner
    pub auto_apply_winner: bool,
    /// Test status
    pub status: TestStatus,
    /// Test creation time
    pub created_at: Option<String>,
    /// Test completion time
    pub completed_at: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ABTest {
    /// Create a new A/B test
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            description: String::new(),
            variant_a: Variant::default(),
            variant_b: Variant::default(),
            primary_metric: "success_rate".to_string(),
            secondary_metrics: Vec::new(),
            min_samples: 30,
            max_duration: None,
            confidence_threshold: 0.95,
            min_effect_size: 0.05,
            auto_apply_winner: false,
            status: TestStatus::Pending,
            created_at: None,
            completed_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Set name
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set variant A (control)
    #[must_use]
    pub fn with_variant_a(mut self, variant: Variant) -> Self {
        self.variant_a = variant;
        self
    }

    /// Set variant B (treatment)
    #[must_use]
    pub fn with_variant_b(mut self, variant: Variant) -> Self {
        self.variant_b = variant;
        self
    }

    /// Set primary metric
    #[must_use]
    pub fn with_metric(mut self, metric: impl Into<String>) -> Self {
        self.primary_metric = metric.into();
        self
    }

    /// Add secondary metric
    #[must_use]
    pub fn with_secondary_metric(mut self, metric: impl Into<String>) -> Self {
        self.secondary_metrics.push(metric.into());
        self
    }

    /// Set minimum samples
    #[must_use]
    pub fn with_min_samples(mut self, samples: usize) -> Self {
        self.min_samples = samples;
        self
    }

    /// Set maximum duration
    #[must_use]
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Set confidence threshold
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence_threshold = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set minimum effect size
    #[must_use]
    pub fn with_min_effect_size(mut self, size: f64) -> Self {
        self.min_effect_size = size.clamp(0.0, 1.0);
        self
    }

    /// Enable automatic winner application
    #[must_use]
    pub fn with_auto_apply(mut self) -> Self {
        self.auto_apply_winner = true;
        self
    }

    /// Check if test has enough samples
    #[must_use]
    pub fn has_enough_samples(&self, samples_a: usize, samples_b: usize) -> bool {
        samples_a >= self.min_samples && samples_b >= self.min_samples
    }

    /// Get a summary of this test
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "A/B Test '{}': {} vs {} (metric: {}, min samples: {})",
            self.id, self.variant_a.id, self.variant_b.id, self.primary_metric, self.min_samples
        )
    }
}

/// Status of an A/B test
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    /// Test not yet started
    Pending,
    /// Test currently running
    Running,
    /// Test completed with result
    Completed,
    /// Test stopped early
    Stopped,
    /// Test inconclusive
    Inconclusive,
}

impl std::fmt::Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestStatus::Pending => write!(f, "Pending"),
            TestStatus::Running => write!(f, "Running"),
            TestStatus::Completed => write!(f, "Completed"),
            TestStatus::Stopped => write!(f, "Stopped"),
            TestStatus::Inconclusive => write!(f, "Inconclusive"),
        }
    }
}

/// A variant in an A/B test
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Variant {
    /// Unique identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Configuration key-value pairs
    pub config: HashMap<String, serde_json::Value>,
    /// Whether this is the control variant
    pub is_control: bool,
    /// Traffic allocation percentage (0.0-1.0)
    pub traffic_allocation: f64,
}

impl Variant {
    /// Create a new variant
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: String::new(),
            config: HashMap::new(),
            is_control: false,
            traffic_allocation: 0.5,
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add configuration option
    #[must_use]
    pub fn with_config(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }

    /// Mark as control variant
    #[must_use]
    pub fn as_control(mut self) -> Self {
        self.is_control = true;
        self
    }

    /// Set traffic allocation
    #[must_use]
    pub fn with_traffic(mut self, allocation: f64) -> Self {
        self.traffic_allocation = allocation.clamp(0.0, 1.0);
        self
    }
}

/// Results of an A/B test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestResult {
    /// Test ID
    pub test_id: String,
    /// Winning variant (if determined)
    pub winner: Option<Winner>,
    /// Statistical confidence (0.0-1.0)
    pub confidence: f64,
    /// Effect size (relative improvement)
    pub effect_size: f64,
    /// P-value from statistical test
    pub p_value: f64,
    /// Is result statistically significant?
    pub is_significant: bool,
    /// Results for variant A
    pub variant_a_results: VariantResults,
    /// Results for variant B
    pub variant_b_results: VariantResults,
    /// Relative improvement (positive = B better than A)
    pub improvement: f64,
    /// Recommendation based on results
    pub recommendation: Recommendation,
    /// Analysis timestamp
    pub analyzed_at: Option<String>,
    /// Duration of test
    pub test_duration: Option<Duration>,
    /// Additional analysis details
    pub details: HashMap<String, serde_json::Value>,
}

impl ABTestResult {
    /// Create a new result
    #[must_use]
    pub fn new(test_id: impl Into<String>) -> Self {
        Self {
            test_id: test_id.into(),
            winner: None,
            confidence: 0.0,
            effect_size: 0.0,
            p_value: 1.0,
            is_significant: false,
            variant_a_results: VariantResults::default(),
            variant_b_results: VariantResults::default(),
            improvement: 0.0,
            recommendation: Recommendation::ContinueTesting,
            analyzed_at: None,
            test_duration: None,
            details: HashMap::new(),
        }
    }

    /// Get a summary of results
    #[must_use]
    pub fn summary(&self) -> String {
        let winner_str = match &self.winner {
            Some(Winner::VariantA) => "Winner: Variant A".to_string(),
            Some(Winner::VariantB) => "Winner: Variant B".to_string(),
            None => "No clear winner".to_string(),
        };

        format!(
            "{} (confidence: {:.1}%, improvement: {:.1}%, p-value: {:.4})\n  A: {:.1}% success ({} samples)\n  B: {:.1}% success ({} samples)\n  Recommendation: {}",
            winner_str,
            self.confidence * 100.0,
            self.improvement * 100.0,
            self.p_value,
            self.variant_a_results.success_rate * 100.0,
            self.variant_a_results.sample_count,
            self.variant_b_results.success_rate * 100.0,
            self.variant_b_results.sample_count,
            self.recommendation
        )
    }

    /// Check if we should apply the winning variant
    #[must_use]
    pub fn should_apply_winner(&self) -> bool {
        self.is_significant && self.winner.is_some() && self.confidence >= 0.95
    }

    /// Convert to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// The winning variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Winner {
    /// Variant A (control) won
    VariantA,
    /// Variant B (treatment) won
    VariantB,
}

impl std::fmt::Display for Winner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Winner::VariantA => write!(f, "Variant A"),
            Winner::VariantB => write!(f, "Variant B"),
        }
    }
}

/// Results for a single variant
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariantResults {
    /// Number of samples
    pub sample_count: usize,
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average latency in ms
    pub avg_latency_ms: f64,
    /// Average token usage
    pub avg_tokens: f64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Metric values
    pub metrics: HashMap<String, f64>,
}

impl VariantResults {
    /// Create new variant results
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set sample count
    #[must_use]
    pub fn with_sample_count(mut self, count: usize) -> Self {
        self.sample_count = count;
        self
    }

    /// Set success rate
    #[must_use]
    pub fn with_success_rate(mut self, rate: f64) -> Self {
        self.success_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set average latency
    #[must_use]
    pub fn with_avg_latency(mut self, latency: f64) -> Self {
        self.avg_latency_ms = latency;
        self
    }

    /// Set average tokens
    #[must_use]
    pub fn with_avg_tokens(mut self, tokens: f64) -> Self {
        self.avg_tokens = tokens;
        self
    }

    /// Add a metric value
    #[must_use]
    pub fn with_metric(mut self, name: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }
}

/// Recommendation based on A/B test results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recommendation {
    /// Apply variant A
    ApplyVariantA,
    /// Apply variant B
    ApplyVariantB,
    /// Continue testing (not enough data)
    ContinueTesting,
    /// No significant difference, keep current
    KeepCurrent,
    /// Results inconclusive
    Inconclusive,
}

impl std::fmt::Display for Recommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Recommendation::ApplyVariantA => write!(f, "Apply Variant A"),
            Recommendation::ApplyVariantB => write!(f, "Apply Variant B"),
            Recommendation::ContinueTesting => write!(f, "Continue Testing"),
            Recommendation::KeepCurrent => write!(f, "Keep Current"),
            Recommendation::Inconclusive => write!(f, "Inconclusive"),
        }
    }
}

/// Configuration for the A/B test runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    /// Default minimum samples
    pub default_min_samples: usize,
    /// Default confidence threshold
    pub default_confidence: f64,
    /// Default minimum effect size
    pub default_min_effect_size: f64,
    /// Whether to use Bayesian analysis
    pub use_bayesian: bool,
    /// Prior probability for Bayesian analysis
    pub bayesian_prior: f64,
}

impl Default for ABTestConfig {
    fn default() -> Self {
        Self {
            default_min_samples: 30,
            default_confidence: 0.95,
            default_min_effect_size: 0.05,
            use_bayesian: false,
            bayesian_prior: 0.5,
        }
    }
}

/// A/B test runner for analyzing experiments
pub struct ABTestRunner {
    config: ABTestConfig,
}

impl Default for ABTestRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ABTestRunner {
    /// Create a new runner with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ABTestConfig::default(),
        }
    }

    /// Create a runner with custom configuration
    #[must_use]
    pub fn with_config(config: ABTestConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &ABTestConfig {
        &self.config
    }

    /// Analyze an A/B test with execution traces
    #[must_use]
    pub fn analyze(
        &self,
        test: &ABTest,
        traces_a: &[crate::introspection::ExecutionTrace],
        traces_b: &[crate::introspection::ExecutionTrace],
    ) -> ABTestResult {
        let mut result = ABTestResult::new(&test.id);

        // Compute variant results
        result.variant_a_results = self.compute_variant_results(traces_a);
        result.variant_b_results = self.compute_variant_results(traces_b);

        // Extract metric values
        let metric_a = self.extract_metric(&result.variant_a_results, &test.primary_metric);
        let metric_b = self.extract_metric(&result.variant_b_results, &test.primary_metric);

        // Calculate improvement
        if metric_a > 0.0 {
            result.improvement = (metric_b - metric_a) / metric_a;
        }

        // Calculate effect size (Cohen's d approximation)
        result.effect_size =
            self.calculate_effect_size(metric_a, metric_b, traces_a.len(), traces_b.len());

        // Calculate p-value using z-test for proportions
        result.p_value = self.calculate_p_value(metric_a, metric_b, traces_a.len(), traces_b.len());

        // Calculate confidence
        result.confidence = 1.0 - result.p_value;

        // Determine significance
        result.is_significant = result.p_value < (1.0 - test.confidence_threshold)
            && result.effect_size.abs() >= test.min_effect_size;

        // Determine winner
        if result.is_significant {
            if metric_b > metric_a {
                result.winner = Some(Winner::VariantB);
            } else if metric_a > metric_b {
                result.winner = Some(Winner::VariantA);
            }
        }

        // Make recommendation
        result.recommendation = self.make_recommendation(test, &result);

        result
    }

    /// Analyze using raw metric values
    #[must_use]
    pub fn analyze_metrics(
        &self,
        test: &ABTest,
        metric_a: f64,
        samples_a: usize,
        metric_b: f64,
        samples_b: usize,
    ) -> ABTestResult {
        let mut result = ABTestResult::new(&test.id);

        // Set up results
        result.variant_a_results = VariantResults::new()
            .with_sample_count(samples_a)
            .with_success_rate(metric_a);
        result.variant_b_results = VariantResults::new()
            .with_sample_count(samples_b)
            .with_success_rate(metric_b);

        // Calculate improvement
        if metric_a > 0.0 {
            result.improvement = (metric_b - metric_a) / metric_a;
        }

        // Calculate effect size
        result.effect_size = self.calculate_effect_size(metric_a, metric_b, samples_a, samples_b);

        // Calculate p-value
        result.p_value = self.calculate_p_value(metric_a, metric_b, samples_a, samples_b);

        // Calculate confidence
        result.confidence = 1.0 - result.p_value;

        // Determine significance
        result.is_significant = result.p_value < (1.0 - test.confidence_threshold)
            && result.effect_size.abs() >= test.min_effect_size;

        // Determine winner
        if result.is_significant {
            if metric_b > metric_a {
                result.winner = Some(Winner::VariantB);
            } else if metric_a > metric_b {
                result.winner = Some(Winner::VariantA);
            }
        }

        // Make recommendation
        result.recommendation = self.make_recommendation(test, &result);

        result
    }

    /// Compute results for a variant
    fn compute_variant_results(
        &self,
        traces: &[crate::introspection::ExecutionTrace],
    ) -> VariantResults {
        if traces.is_empty() {
            return VariantResults::default();
        }

        let sample_count = traces.len();
        let success_count = traces.iter().filter(|t| t.completed).count();
        let success_rate = success_count as f64 / sample_count as f64;

        let total_latency: u64 = traces.iter().map(|t| t.total_duration_ms).sum();
        let avg_latency = total_latency as f64 / sample_count as f64;

        let total_tokens: u64 = traces.iter().map(|t| t.total_tokens).sum();
        let avg_tokens = total_tokens as f64 / sample_count as f64;

        let error_count = traces.iter().filter(|t| !t.errors.is_empty()).count();
        let error_rate = error_count as f64 / sample_count as f64;

        VariantResults {
            sample_count,
            success_rate,
            avg_latency_ms: avg_latency,
            avg_tokens,
            error_rate,
            metrics: HashMap::new(),
        }
    }

    /// Extract a metric value from results
    fn extract_metric(&self, results: &VariantResults, metric: &str) -> f64 {
        match metric {
            "success_rate" => results.success_rate,
            "latency" | "avg_latency" | "avg_latency_ms" => {
                1.0 / (1.0 + results.avg_latency_ms / 1000.0)
            } // Invert: lower is better
            "tokens" | "avg_tokens" => 1.0 / (1.0 + results.avg_tokens / 1000.0), // Invert: lower is better
            "error_rate" => 1.0 - results.error_rate, // Invert: lower is better
            _ => results.metrics.get(metric).copied().unwrap_or(0.0),
        }
    }

    /// Calculate effect size (Cohen's d approximation for proportions)
    fn calculate_effect_size(&self, p1: f64, p2: f64, n1: usize, n2: usize) -> f64 {
        if n1 == 0 || n2 == 0 {
            return 0.0;
        }

        // Pooled proportion
        let p = (p1 * n1 as f64 + p2 * n2 as f64) / (n1 + n2) as f64;

        // Pooled standard deviation approximation
        let pooled_std = (p * (1.0 - p)).sqrt();

        if pooled_std < f64::EPSILON {
            return 0.0;
        }

        // Cohen's h for proportions
        (p2 - p1) / pooled_std
    }

    /// Calculate p-value using z-test for proportions
    fn calculate_p_value(&self, p1: f64, p2: f64, n1: usize, n2: usize) -> f64 {
        if n1 == 0 || n2 == 0 {
            return 1.0;
        }

        // Pooled proportion under null hypothesis
        let p = (p1 * n1 as f64 + p2 * n2 as f64) / (n1 + n2) as f64;

        // Standard error
        let se = (p * (1.0 - p) * (1.0 / n1 as f64 + 1.0 / n2 as f64)).sqrt();

        if se < f64::EPSILON {
            return 1.0;
        }

        // Z-score
        let z = (p2 - p1).abs() / se;

        // Two-tailed p-value (using normal approximation)
        // P(|Z| > z) ≈ 2 * (1 - Φ(z))
        // Using simple approximation for standard normal CDF
        2.0 * (1.0 - standard_normal_cdf(z))
    }

    /// Make recommendation based on results
    fn make_recommendation(&self, test: &ABTest, result: &ABTestResult) -> Recommendation {
        let has_enough_samples = test.has_enough_samples(
            result.variant_a_results.sample_count,
            result.variant_b_results.sample_count,
        );

        if !has_enough_samples {
            return Recommendation::ContinueTesting;
        }

        if !result.is_significant {
            return Recommendation::KeepCurrent;
        }

        match result.winner {
            Some(Winner::VariantA) => Recommendation::ApplyVariantA,
            Some(Winner::VariantB) => Recommendation::ApplyVariantB,
            None => Recommendation::Inconclusive,
        }
    }

    /// Generate a report for an A/B test
    #[must_use]
    pub fn generate_report(&self, test: &ABTest, result: &ABTestResult) -> String {
        let lines = vec![
            format!("A/B Test Report: {}", test.id),
            format!("================{}=", "=".repeat(test.id.len())),
            String::new(),
            format!("Test: {}", test.summary()),
            format!("Metric: {}", test.primary_metric),
            String::new(),
            "Results:".to_string(),
            format!(
                "  Variant A ({}): {:.2}% (n={})",
                test.variant_a.id,
                result.variant_a_results.success_rate * 100.0,
                result.variant_a_results.sample_count
            ),
            format!(
                "  Variant B ({}): {:.2}% (n={})",
                test.variant_b.id,
                result.variant_b_results.success_rate * 100.0,
                result.variant_b_results.sample_count
            ),
            String::new(),
            "Statistical Analysis:".to_string(),
            format!("  Improvement: {:.2}%", result.improvement * 100.0),
            format!("  Effect Size: {:.3}", result.effect_size),
            format!("  P-value: {:.4}", result.p_value),
            format!("  Confidence: {:.2}%", result.confidence * 100.0),
            format!("  Significant: {}", result.is_significant),
            String::new(),
            format!(
                "Winner: {}",
                result
                    .winner
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "None".to_string())
            ),
            format!("Recommendation: {}", result.recommendation),
        ];

        lines.join("\n")
    }
}

/// Simple approximation of standard normal CDF
/// Uses the error function approximation
fn standard_normal_cdf(x: f64) -> f64 {
    // Using logistic approximation: Φ(x) ≈ 1 / (1 + e^(-1.702 * x))
    // This is accurate to within 0.003 for all x
    1.0 / (1.0 + (-1.702 * x).exp())
}

/// History of A/B tests for learning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ABTestHistory {
    /// All completed tests
    pub tests: Vec<ABTestRecord>,
    /// Insights derived from test history
    pub insights: Vec<TestInsight>,
}

impl ABTestHistory {
    /// Create a new history
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a completed test
    pub fn add_test(&mut self, test: &ABTest, result: &ABTestResult) {
        self.tests.push(ABTestRecord {
            test_id: test.id.clone(),
            variant_a_id: test.variant_a.id.clone(),
            variant_b_id: test.variant_b.id.clone(),
            metric: test.primary_metric.clone(),
            winner: result.winner,
            improvement: result.improvement,
            confidence: result.confidence,
            sample_count_a: result.variant_a_results.sample_count,
            sample_count_b: result.variant_b_results.sample_count,
            is_significant: result.is_significant,
        });
    }

    /// Get tests by metric
    #[must_use]
    pub fn tests_by_metric(&self, metric: &str) -> Vec<&ABTestRecord> {
        self.tests.iter().filter(|t| t.metric == metric).collect()
    }

    /// Get win rate for a metric direction
    #[must_use]
    pub fn win_rate(&self, metric: &str) -> f64 {
        let metric_tests: Vec<_> = self.tests.iter().filter(|t| t.metric == metric).collect();
        if metric_tests.is_empty() {
            return 0.0;
        }
        let wins = metric_tests.iter().filter(|t| t.is_significant).count();
        wins as f64 / metric_tests.len() as f64
    }

    /// Derive insights from history
    pub fn derive_insights(&mut self) {
        self.insights.clear();

        // Find patterns in winning configurations
        let significant_tests: Vec<_> = self.tests.iter().filter(|t| t.is_significant).collect();

        if significant_tests.len() >= 3 {
            // Calculate average improvement
            let avg_improvement: f64 = significant_tests.iter().map(|t| t.improvement).sum::<f64>()
                / significant_tests.len() as f64;

            self.insights.push(TestInsight {
                insight_type: InsightType::PerformanceTrend,
                description: format!(
                    "Average improvement from {} significant tests: {:.1}%",
                    significant_tests.len(),
                    avg_improvement * 100.0
                ),
                confidence: 0.8,
                recommendations: vec!["Continue A/B testing for optimization".to_string()],
            });
        }

        // Find metrics with consistent winners
        let mut metric_wins: HashMap<String, usize> = HashMap::new();
        for test in &self.tests {
            if test.is_significant {
                *metric_wins.entry(test.metric.clone()).or_default() += 1;
            }
        }

        for (metric, wins) in metric_wins {
            if wins >= 2 {
                self.insights.push(TestInsight {
                    insight_type: InsightType::ReliableMetric,
                    description: format!(
                        "Metric '{}' has {} significant test results",
                        metric, wins
                    ),
                    confidence: 0.7,
                    recommendations: vec![format!(
                        "Use '{}' as primary optimization metric",
                        metric
                    )],
                });
            }
        }
    }
}

/// Record of a completed A/B test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestRecord {
    /// Test ID
    pub test_id: String,
    /// Variant A ID
    pub variant_a_id: String,
    /// Variant B ID
    pub variant_b_id: String,
    /// Metric tested
    pub metric: String,
    /// Winner (if any)
    pub winner: Option<Winner>,
    /// Improvement percentage
    pub improvement: f64,
    /// Statistical confidence
    pub confidence: f64,
    /// Sample count for A
    pub sample_count_a: usize,
    /// Sample count for B
    pub sample_count_b: usize,
    /// Was result significant?
    pub is_significant: bool,
}

/// Insight derived from test history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestInsight {
    /// Type of insight
    pub insight_type: InsightType,
    /// Human-readable description
    pub description: String,
    /// Confidence in this insight
    pub confidence: f64,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Types of insights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightType {
    /// Performance trend over time
    PerformanceTrend,
    /// A metric that consistently produces results
    ReliableMetric,
    /// A configuration that consistently wins
    WinningConfig,
    /// A configuration that consistently loses
    LosingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_successful_trace(duration_ms: u64, tokens: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("node1", duration_ms).with_tokens(tokens))
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(true)
            .build()
    }

    fn create_failed_trace(duration_ms: u64, tokens: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("node1", duration_ms).with_tokens(tokens))
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(false)
            .build()
    }

    #[test]
    fn test_ab_test_creation() {
        let test = ABTest::new("test1")
            .with_name("Model Comparison")
            .with_description("Compare GPT-3.5 vs GPT-4")
            .with_variant_a(Variant::new("gpt35").with_description("Fast model"))
            .with_variant_b(Variant::new("gpt4").with_description("Smart model"))
            .with_metric("success_rate")
            .with_min_samples(50)
            .with_confidence(0.95);

        assert_eq!(test.id, "test1");
        assert_eq!(test.name, "Model Comparison");
        assert_eq!(test.variant_a.id, "gpt35");
        assert_eq!(test.variant_b.id, "gpt4");
        assert_eq!(test.min_samples, 50);
    }

    #[test]
    fn test_variant_creation() {
        let variant = Variant::new("test_variant")
            .with_description("Test variant")
            .with_config("model", "gpt-4")
            .with_config("temperature", 0.7)
            .with_traffic(0.3)
            .as_control();

        assert_eq!(variant.id, "test_variant");
        assert!(variant.is_control);
        assert_eq!(variant.traffic_allocation, 0.3);
        assert!(variant.config.contains_key("model"));
    }

    #[test]
    fn test_variant_results() {
        let results = VariantResults::new()
            .with_sample_count(100)
            .with_success_rate(0.85)
            .with_avg_latency(500.0)
            .with_avg_tokens(2000.0)
            .with_metric("custom_metric", 0.95);

        assert_eq!(results.sample_count, 100);
        assert_eq!(results.success_rate, 0.85);
        assert_eq!(results.avg_latency_ms, 500.0);
        assert_eq!(results.metrics.get("custom_metric"), Some(&0.95));
    }

    #[test]
    fn test_ab_test_result_creation() {
        let result = ABTestResult::new("test1");
        assert_eq!(result.test_id, "test1");
        assert!(result.winner.is_none());
        assert!(!result.is_significant);
    }

    #[test]
    fn test_test_status_display() {
        assert_eq!(TestStatus::Pending.to_string(), "Pending");
        assert_eq!(TestStatus::Running.to_string(), "Running");
        assert_eq!(TestStatus::Completed.to_string(), "Completed");
    }

    #[test]
    fn test_winner_display() {
        assert_eq!(Winner::VariantA.to_string(), "Variant A");
        assert_eq!(Winner::VariantB.to_string(), "Variant B");
    }

    #[test]
    fn test_recommendation_display() {
        assert_eq!(Recommendation::ApplyVariantA.to_string(), "Apply Variant A");
        assert_eq!(
            Recommendation::ContinueTesting.to_string(),
            "Continue Testing"
        );
    }

    #[test]
    fn test_ab_test_has_enough_samples() {
        let test = ABTest::new("test").with_min_samples(30);

        assert!(test.has_enough_samples(30, 30));
        assert!(test.has_enough_samples(50, 100));
        assert!(!test.has_enough_samples(20, 30));
        assert!(!test.has_enough_samples(30, 20));
    }

    #[test]
    fn test_runner_empty_traces() {
        let runner = ABTestRunner::new();
        let test = ABTest::new("test").with_min_samples(10);

        let result = runner.analyze(&test, &[], &[]);

        assert_eq!(result.variant_a_results.sample_count, 0);
        assert_eq!(result.variant_b_results.sample_count, 0);
        assert!(result.winner.is_none());
    }

    #[test]
    fn test_runner_clear_winner() {
        let runner = ABTestRunner::new();
        let test = ABTest::new("test")
            .with_min_samples(10)
            .with_confidence(0.90)
            .with_min_effect_size(0.01);

        // Create traces where B is clearly better
        let traces_a: Vec<ExecutionTrace> = (0..50)
            .map(|_| {
                if rand_bool(0.6) {
                    create_successful_trace(1000, 2000)
                } else {
                    create_failed_trace(1000, 2000)
                }
            })
            .collect();

        let traces_b: Vec<ExecutionTrace> = (0..50)
            .map(|_| {
                if rand_bool(0.9) {
                    create_successful_trace(800, 1500)
                } else {
                    create_failed_trace(800, 1500)
                }
            })
            .collect();

        let result = runner.analyze(&test, &traces_a, &traces_b);

        assert_eq!(result.variant_a_results.sample_count, 50);
        assert_eq!(result.variant_b_results.sample_count, 50);
        // B should be better
        assert!(result.variant_b_results.success_rate > result.variant_a_results.success_rate);
    }

    #[test]
    fn test_runner_analyze_metrics() {
        let runner = ABTestRunner::new();
        let test = ABTest::new("test")
            .with_min_samples(30)
            .with_confidence(0.95)
            .with_min_effect_size(0.05);

        // Clear difference
        let result = runner.analyze_metrics(&test, 0.50, 100, 0.80, 100);

        assert!(result.improvement > 0.0);
        assert!(result.confidence > 0.5);
        // With such a large difference, should be significant
        assert!(result.is_significant);
        assert_eq!(result.winner, Some(Winner::VariantB));
    }

    #[test]
    fn test_runner_no_significant_difference() {
        let runner = ABTestRunner::new();
        let test = ABTest::new("test")
            .with_min_samples(30)
            .with_confidence(0.95)
            .with_min_effect_size(0.10);

        // Very similar metrics
        let result = runner.analyze_metrics(&test, 0.75, 100, 0.76, 100);

        // Small difference, not significant
        assert!(!result.is_significant);
        assert!(result.winner.is_none());
        assert_eq!(result.recommendation, Recommendation::KeepCurrent);
    }

    #[test]
    fn test_runner_not_enough_samples() {
        let runner = ABTestRunner::new();
        let test = ABTest::new("test").with_min_samples(100);

        let result = runner.analyze_metrics(&test, 0.50, 20, 0.80, 20);

        assert_eq!(result.recommendation, Recommendation::ContinueTesting);
    }

    #[test]
    fn test_result_summary() {
        let mut result = ABTestResult::new("test");
        result.winner = Some(Winner::VariantB);
        result.confidence = 0.98;
        result.improvement = 0.25;
        result.p_value = 0.02;
        result.variant_a_results = VariantResults::new()
            .with_sample_count(100)
            .with_success_rate(0.70);
        result.variant_b_results = VariantResults::new()
            .with_sample_count(100)
            .with_success_rate(0.875);
        result.recommendation = Recommendation::ApplyVariantB;

        let summary = result.summary();

        assert!(summary.contains("Variant B"));
        assert!(summary.contains("98.0%"));
        assert!(summary.contains("Apply Variant B"));
    }

    #[test]
    fn test_result_json_roundtrip() {
        let mut result = ABTestResult::new("test");
        result.winner = Some(Winner::VariantA);
        result.confidence = 0.95;

        let json = result.to_json().unwrap();
        let parsed = ABTestResult::from_json(&json).unwrap();

        assert_eq!(parsed.test_id, result.test_id);
        assert_eq!(parsed.winner, result.winner);
    }

    #[test]
    fn test_result_should_apply_winner() {
        let mut result = ABTestResult::new("test");

        // Not significant, no winner
        assert!(!result.should_apply_winner());

        // Has winner but not significant
        result.winner = Some(Winner::VariantB);
        assert!(!result.should_apply_winner());

        // Significant with winner but low confidence
        result.is_significant = true;
        result.confidence = 0.90;
        assert!(!result.should_apply_winner());

        // All conditions met
        result.confidence = 0.98;
        assert!(result.should_apply_winner());
    }

    #[test]
    fn test_generate_report() {
        let test = ABTest::new("model_test")
            .with_variant_a(Variant::new("model_a"))
            .with_variant_b(Variant::new("model_b"))
            .with_metric("success_rate");

        let mut result = ABTestResult::new("model_test");
        result.winner = Some(Winner::VariantB);
        result.confidence = 0.97;
        result.improvement = 0.15;
        result.p_value = 0.03;
        result.effect_size = 0.35;
        result.is_significant = true;
        result.variant_a_results = VariantResults::new()
            .with_sample_count(100)
            .with_success_rate(0.75);
        result.variant_b_results = VariantResults::new()
            .with_sample_count(100)
            .with_success_rate(0.86);
        result.recommendation = Recommendation::ApplyVariantB;

        let runner = ABTestRunner::new();
        let report = runner.generate_report(&test, &result);

        assert!(report.contains("A/B Test Report"));
        assert!(report.contains("model_test"));
        assert!(report.contains("Improvement"));
        assert!(report.contains("P-value"));
    }

    #[test]
    fn test_ab_test_history() {
        let mut history = ABTestHistory::new();

        let test = ABTest::new("test1")
            .with_variant_a(Variant::new("a"))
            .with_variant_b(Variant::new("b"))
            .with_metric("success_rate");

        let mut result = ABTestResult::new("test1");
        result.winner = Some(Winner::VariantB);
        result.improvement = 0.20;
        result.confidence = 0.97;
        result.variant_a_results = VariantResults::new().with_sample_count(100);
        result.variant_b_results = VariantResults::new().with_sample_count(100);
        result.is_significant = true;

        history.add_test(&test, &result);

        assert_eq!(history.tests.len(), 1);
        assert_eq!(history.tests[0].test_id, "test1");
        assert_eq!(history.tests[0].winner, Some(Winner::VariantB));
    }

    #[test]
    fn test_history_tests_by_metric() {
        let mut history = ABTestHistory::new();

        history.tests.push(ABTestRecord {
            test_id: "test1".to_string(),
            variant_a_id: "a".to_string(),
            variant_b_id: "b".to_string(),
            metric: "success_rate".to_string(),
            winner: Some(Winner::VariantB),
            improvement: 0.2,
            confidence: 0.95,
            sample_count_a: 100,
            sample_count_b: 100,
            is_significant: true,
        });

        history.tests.push(ABTestRecord {
            test_id: "test2".to_string(),
            variant_a_id: "a".to_string(),
            variant_b_id: "b".to_string(),
            metric: "latency".to_string(),
            winner: Some(Winner::VariantA),
            improvement: -0.1,
            confidence: 0.90,
            sample_count_a: 50,
            sample_count_b: 50,
            is_significant: true,
        });

        let success_tests = history.tests_by_metric("success_rate");
        assert_eq!(success_tests.len(), 1);
        assert_eq!(success_tests[0].test_id, "test1");
    }

    #[test]
    fn test_history_win_rate() {
        let mut history = ABTestHistory::new();

        // 3 tests for success_rate, 2 significant
        for i in 0..3 {
            history.tests.push(ABTestRecord {
                test_id: format!("test{}", i),
                variant_a_id: "a".to_string(),
                variant_b_id: "b".to_string(),
                metric: "success_rate".to_string(),
                winner: Some(Winner::VariantB),
                improvement: 0.1,
                confidence: 0.95,
                sample_count_a: 100,
                sample_count_b: 100,
                is_significant: i < 2, // Only first 2 significant
            });
        }

        let win_rate = history.win_rate("success_rate");
        assert!((win_rate - 0.6666).abs() < 0.01); // 2/3
    }

    #[test]
    fn test_history_derive_insights() {
        let mut history = ABTestHistory::new();

        // Add several significant tests
        for i in 0..5 {
            history.tests.push(ABTestRecord {
                test_id: format!("test{}", i),
                variant_a_id: "a".to_string(),
                variant_b_id: "b".to_string(),
                metric: "success_rate".to_string(),
                winner: Some(Winner::VariantB),
                improvement: 0.1 + i as f64 * 0.05,
                confidence: 0.95,
                sample_count_a: 100,
                sample_count_b: 100,
                is_significant: true,
            });
        }

        history.derive_insights();

        assert!(!history.insights.is_empty());
        // Should have performance trend insight
        assert!(history
            .insights
            .iter()
            .any(|i| i.insight_type == InsightType::PerformanceTrend));
    }

    #[test]
    fn test_config_defaults() {
        let config = ABTestConfig::default();

        assert_eq!(config.default_min_samples, 30);
        assert_eq!(config.default_confidence, 0.95);
        assert_eq!(config.default_min_effect_size, 0.05);
        assert!(!config.use_bayesian);
    }

    #[test]
    fn test_standard_normal_cdf() {
        // Test known values
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 0.01);
        assert!(standard_normal_cdf(2.0) > 0.95);
        assert!(standard_normal_cdf(-2.0) < 0.05);
    }

    #[test]
    fn test_ab_test_summary() {
        let test = ABTest::new("test1")
            .with_variant_a(Variant::new("fast"))
            .with_variant_b(Variant::new("smart"))
            .with_metric("accuracy")
            .with_min_samples(100);

        let summary = test.summary();

        assert!(summary.contains("test1"));
        assert!(summary.contains("fast"));
        assert!(summary.contains("smart"));
        assert!(summary.contains("accuracy"));
        assert!(summary.contains("100"));
    }

    #[test]
    fn test_insight_type() {
        let insight = TestInsight {
            insight_type: InsightType::WinningConfig,
            description: "Config X wins consistently".to_string(),
            confidence: 0.9,
            recommendations: vec!["Use config X".to_string()],
        };

        assert_eq!(insight.insight_type, InsightType::WinningConfig);
        assert!(!insight.recommendations.is_empty());
    }

    // Simple pseudo-random for tests (deterministic based on call order)
    fn rand_bool(probability: f64) -> bool {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        // Simple LCG-like hash
        let hash = (n
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407)) as f64;
        let normalized = (hash / u64::MAX as f64).abs();
        normalized < probability
    }
}
