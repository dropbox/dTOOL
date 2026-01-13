//! Eval Runner - Orchestrates evaluation execution with parallel processing, retries, and timeouts.
//!
//! This module provides the core evaluation engine that:
//! - Executes test scenarios in parallel with configurable concurrency
//! - Integrates with `MultiDimensionalJudge` for quality scoring
//! - Validates output against expected criteria
//! - Handles retries with exponential backoff for flaky tests
//! - Enforces per-scenario timeouts
//! - Collects performance metrics (latency, cost, tokens)
//! - Generates comprehensive evaluation reports
//!
//! # Example
//!
//! ```no_run
//! use dashflow_evals::{EvalRunner, GoldenDataset, MultiDimensionalJudge};
//! use dashflow_openai::ChatOpenAI;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Load golden dataset
//! let dataset = GoldenDataset::load("golden_dataset")?;
//!
//! // Create quality judge
//! let model = Arc::new(ChatOpenAI::with_config(Default::default()).with_model("gpt-4o").with_temperature(0.0));
//! let judge = MultiDimensionalJudge::new(model);
//!
//! // Define agent function with detailed token tracking
//! let agent_fn = Arc::new(|query: String| {
//!     Box::pin(async move {
//!         // Your agent logic here
//!         let output = format!("Response to: {}", query);
//!
//!         // Option 1: Response without usage tracking
//!         // Ok(dashflow_evals::AgentResponse::text_only(output))
//!
//!         // Option 2: Response with detailed token usage and cost
//!         let token_usage = dashflow_evals::TokenUsage::new(100, 50); // 100 input, 50 output tokens
//!         Ok(dashflow_evals::AgentResponse::with_detailed_usage(
//!             output,
//!             token_usage,
//!             0.0001, // cost in USD
//!         ))
//!     }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<dashflow_evals::AgentResponse>> + Send>>
//! });
//!
//! // Create eval runner with configuration
//! let runner = EvalRunner::builder()
//!     .agent_fn(agent_fn)
//!     .judge(judge)
//!     .max_concurrency(5)
//!     .scenario_timeout(Duration::from_secs(30))
//!     .max_retries(2)
//!     .build();
//!
//! // Run evaluation
//! let report = runner.evaluate(&dataset).await?;
//!
//! println!("Pass rate: {}/{}", report.passed, report.total);
//! println!("Avg quality: {:.3}", report.avg_quality());
//! # Ok(())
//! # }
//! ```

use crate::golden_dataset::{GoldenDataset, GoldenScenario};
use crate::quality_judge::{MultiDimensionalJudge, QualityScore};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashflow::constants::DEFAULT_HTTP_REQUEST_TIMEOUT;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing;

/// Token usage details with input/output breakdown.
///
/// This struct provides accurate token tracking with separate input and output counts,
/// enabling precise cost calculation based on different pricing for prompt vs completion tokens.
///
/// # Example
///
/// ```
/// use dashflow_evals::{TokenUsage, AgentResponse};
///
/// // Create token usage with 100 input and 50 output tokens
/// let usage = TokenUsage::new(100, 50);
/// assert_eq!(usage.total_tokens, 150);
/// assert_eq!(usage.input_tokens, 100);
/// assert_eq!(usage.output_tokens, 50);
///
/// // Create agent response with token tracking
/// let response = AgentResponse::with_token_usage(
///     "Hello, world!".to_string(),
///     usage,
/// );
///
/// // Calculate cost with different input/output pricing
/// let cost = response.calculate_cost(
///     0.15,  // $0.15 per million input tokens
///     0.60,  // $0.60 per million output tokens
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of input (prompt) tokens
    pub input_tokens: u32,

    /// Number of output (completion) tokens
    pub output_tokens: u32,

    /// Total tokens (input + output)
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Create token usage from input and output counts
    #[must_use]
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }

    /// Create token usage from total only (legacy)
    #[must_use]
    pub fn from_total(total_tokens: u32) -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens,
        }
    }
}

/// Response from an agent execution, including output and optional usage metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// The text output from the agent
    pub output: String,

    /// Optional detailed token usage information
    pub token_usage: Option<TokenUsage>,

    /// Optional token usage information (legacy - total only)
    /// Deprecated: Use `token_usage` instead for detailed breakdown
    #[deprecated(since = "1.11.3", note = "Use token_usage instead")]
    pub tokens_used: Option<u32>,

    /// Optional cost in USD
    pub cost_usd: Option<f64>,
}

impl AgentResponse {
    /// Create a response with just output text (no usage metadata)
    #[must_use]
    pub fn text_only(output: String) -> Self {
        Self {
            output,
            token_usage: None,
            #[allow(deprecated)]
            tokens_used: None,
            cost_usd: None,
        }
    }

    /// Create a response with output and detailed token usage
    #[must_use]
    pub fn with_token_usage(output: String, token_usage: TokenUsage) -> Self {
        #[allow(deprecated)]
        Self {
            output,
            token_usage: Some(token_usage.clone()),
            tokens_used: Some(token_usage.total_tokens),
            cost_usd: None,
        }
    }

    /// Create a response with output and token usage (legacy - total only)
    #[deprecated(since = "1.11.3", note = "Use with_token_usage instead")]
    #[must_use]
    pub fn with_tokens(output: String, tokens_used: u32) -> Self {
        #[allow(deprecated)]
        Self {
            output,
            token_usage: Some(TokenUsage::from_total(tokens_used)),
            tokens_used: Some(tokens_used),
            cost_usd: None,
        }
    }

    /// Create a response with output, tokens, and cost (legacy)
    #[deprecated(since = "1.11.3", note = "Use with_detailed_usage instead")]
    #[must_use]
    pub fn with_usage(output: String, tokens_used: u32, cost_usd: f64) -> Self {
        #[allow(deprecated)]
        Self {
            output,
            token_usage: Some(TokenUsage::from_total(tokens_used)),
            tokens_used: Some(tokens_used),
            cost_usd: Some(cost_usd),
        }
    }

    /// Create a response with output, detailed token usage, and cost
    #[must_use]
    pub fn with_detailed_usage(output: String, token_usage: TokenUsage, cost_usd: f64) -> Self {
        #[allow(deprecated)]
        Self {
            output,
            token_usage: Some(token_usage.clone()),
            tokens_used: Some(token_usage.total_tokens),
            cost_usd: Some(cost_usd),
        }
    }

    /// Get total tokens (from new field if available, else legacy field)
    #[must_use]
    pub fn total_tokens(&self) -> Option<u32> {
        #[allow(deprecated)]
        {
            self.token_usage
                .as_ref()
                .map(|u| u.total_tokens)
                .or(self.tokens_used)
        }
    }

    /// Get input tokens (0 if not tracked)
    #[must_use]
    pub fn input_tokens(&self) -> u32 {
        self.token_usage.as_ref().map_or(0, |u| u.input_tokens)
    }

    /// Get output tokens (0 if not tracked)
    #[must_use]
    pub fn output_tokens(&self) -> u32 {
        self.token_usage.as_ref().map_or(0, |u| u.output_tokens)
    }

    /// Calculate cost from token usage and pricing
    #[must_use]
    pub fn calculate_cost(
        &self,
        cost_per_million_input: f64,
        cost_per_million_output: f64,
    ) -> Option<f64> {
        self.token_usage.as_ref().map(|usage| {
            let input_cost = (f64::from(usage.input_tokens) / 1_000_000.0) * cost_per_million_input;
            let output_cost =
                (f64::from(usage.output_tokens) / 1_000_000.0) * cost_per_million_output;
            input_cost + output_cost
        })
    }
}

/// Type alias for agent functions that process scenarios.
///
/// The function takes a scenario query string and returns the agent's response with optional usage metadata.
pub type AgentFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<AgentResponse>> + Send>> + Send + Sync,
>;

/// Configuration for evaluation execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    /// Run scenarios in parallel (default: true)
    pub parallel_execution: bool,

    /// Maximum number of concurrent scenario executions (default: 5)
    pub max_concurrency: usize,

    /// Retry failed scenarios (default: true)
    pub retry_on_failure: bool,

    /// Maximum number of retries per scenario (default: 2)
    pub max_retries: u32,

    /// Timeout per scenario execution (default: 30s)
    pub scenario_timeout: Duration,

    /// Enable verbose logging (default: false)
    pub verbose: bool,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            parallel_execution: true,
            max_concurrency: 5,
            retry_on_failure: true,
            max_retries: 2,
            // Uses centralized DEFAULT_HTTP_REQUEST_TIMEOUT (30s) for consistency
            scenario_timeout: DEFAULT_HTTP_REQUEST_TIMEOUT,
            verbose: false,
        }
    }
}

/// Validation result for output against expected criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Overall validation passed
    pub passed: bool,

    /// Strings that should be present but are missing
    pub missing_contains: Vec<String>,

    /// Strings that should not be present but were found
    pub forbidden_found: Vec<String>,

    /// Details about validation failures
    pub failure_reason: Option<String>,
}

/// Result of evaluating a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    /// Scenario identifier
    pub scenario_id: String,

    /// Overall pass/fail status
    pub passed: bool,

    /// Agent's output
    pub output: String,

    /// Multi-dimensional quality score
    pub quality_score: QualityScore,

    /// Execution latency in milliseconds
    pub latency_ms: u64,

    /// Validation result (`must_contain/must_not_contain` checks)
    pub validation: ValidationResult,

    /// Error message if scenario failed
    pub error: Option<String>,

    /// Number of retry attempts made
    pub retry_attempts: u32,

    /// Timestamp of evaluation
    pub timestamp: DateTime<Utc>,

    /// Input query (optional, for continuous learning)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input: Option<String>,

    /// Total tokens used (input + output)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tokens_used: Option<u32>,

    /// Estimated cost in USD
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost_usd: Option<f64>,
}

/// Aggregated evaluation report for all scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    /// Total number of scenarios
    pub total: usize,

    /// Number of passed scenarios
    pub passed: usize,

    /// Number of failed scenarios
    pub failed: usize,

    /// Individual scenario results
    pub results: Vec<ScenarioResult>,

    /// Metadata about the evaluation run
    pub metadata: EvalMetadata,
}

/// Metadata about an evaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMetadata {
    /// Start time
    pub started_at: DateTime<Utc>,

    /// End time
    pub completed_at: DateTime<Utc>,

    /// Total execution time in seconds
    pub duration_secs: f64,

    /// Configuration used
    pub config: String, // JSON-serialized config for storage
}

impl EvalReport {
    /// Calculate pass rate (0.0-1.0)
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.passed as f64 / self.total as f64
    }

    /// Calculate average quality score across all scenarios
    #[must_use]
    pub fn avg_quality(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.results.iter().map(|r| r.quality_score.overall).sum();
        sum / self.results.len() as f64
    }

    /// Calculate average latency in milliseconds
    #[must_use]
    pub fn avg_latency_ms(&self) -> u64 {
        if self.results.is_empty() {
            return 0;
        }
        let sum: u64 = self.results.iter().map(|r| r.latency_ms).sum();
        sum / self.results.len() as u64
    }

    /// Get all failed scenario results
    #[must_use]
    pub fn failed_scenarios(&self) -> Vec<&ScenarioResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Get scenarios that required retries
    #[must_use]
    pub fn retried_scenarios(&self) -> Vec<&ScenarioResult> {
        self.results
            .iter()
            .filter(|r| r.retry_attempts > 0)
            .collect()
    }

    /// Calculate total cost in USD across all scenarios
    #[must_use]
    pub fn total_cost_usd(&self) -> f64 {
        self.results.iter().filter_map(|r| r.cost_usd).sum()
    }

    /// Calculate average cost in USD per scenario
    #[must_use]
    pub fn avg_cost_usd(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let costs: Vec<f64> = self.results.iter().filter_map(|r| r.cost_usd).collect();
        if costs.is_empty() {
            return 0.0;
        }
        costs.iter().sum::<f64>() / costs.len() as f64
    }

    /// Calculate total tokens used across all scenarios
    #[must_use]
    pub fn total_tokens(&self) -> u32 {
        self.results.iter().filter_map(|r| r.tokens_used).sum()
    }

    /// Calculate average tokens per scenario
    #[must_use]
    pub fn avg_tokens(&self) -> u32 {
        if self.results.is_empty() {
            return 0;
        }
        let tokens: Vec<u32> = self.results.iter().filter_map(|r| r.tokens_used).collect();
        if tokens.is_empty() {
            return 0;
        }
        tokens.iter().sum::<u32>() / tokens.len() as u32
    }
}

/// Builder for `EvalRunner` with fluent API.
pub struct EvalRunnerBuilder {
    agent_fn: Option<AgentFn>,
    judge: Option<MultiDimensionalJudge>,
    config: EvalConfig,
}

impl EvalRunnerBuilder {
    /// Set the agent function
    pub fn agent_fn(mut self, agent_fn: AgentFn) -> Self {
        self.agent_fn = Some(agent_fn);
        self
    }

    /// Set the quality judge
    #[must_use]
    pub fn judge(mut self, judge: MultiDimensionalJudge) -> Self {
        self.judge = Some(judge);
        self
    }

    /// Set maximum concurrency
    #[must_use]
    pub fn max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.config.max_concurrency = max_concurrency;
        self
    }

    /// Set scenario timeout
    #[must_use]
    pub fn scenario_timeout(mut self, timeout: Duration) -> Self {
        self.config.scenario_timeout = timeout;
        self
    }

    /// Set maximum retries
    #[must_use]
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Enable/disable parallel execution
    #[must_use]
    pub fn parallel_execution(mut self, enabled: bool) -> Self {
        self.config.parallel_execution = enabled;
        self
    }

    /// Enable/disable verbose logging
    #[must_use]
    pub fn verbose(mut self, enabled: bool) -> Self {
        self.config.verbose = enabled;
        self
    }

    /// Build the `EvalRunner`
    #[must_use]
    pub fn build(self) -> EvalRunner {
        EvalRunner {
            agent_fn: self.agent_fn.expect("agent_fn is required"),
            judge: self.judge.expect("judge is required"),
            config: self.config,
        }
    }
}

/// Evaluation runner that orchestrates scenario execution.
pub struct EvalRunner {
    /// Function that runs the agent for a scenario
    agent_fn: AgentFn,

    /// Quality judge for scoring outputs
    judge: MultiDimensionalJudge,

    /// Configuration
    config: EvalConfig,
}

impl EvalRunner {
    /// Create a new builder for `EvalRunner`
    #[must_use]
    pub fn builder() -> EvalRunnerBuilder {
        EvalRunnerBuilder {
            agent_fn: None,
            judge: None,
            config: EvalConfig::default(),
        }
    }

    /// Create a new `EvalRunner` with default configuration
    pub fn new(agent_fn: AgentFn, judge: MultiDimensionalJudge) -> Self {
        Self {
            agent_fn,
            judge,
            config: EvalConfig::default(),
        }
    }

    /// Run evaluation on entire dataset
    pub async fn evaluate(&self, dataset: &GoldenDataset) -> Result<EvalReport> {
        let start_time = Utc::now();
        let start_instant = Instant::now();

        if self.config.verbose {
            tracing::info!(
                scenario_count = dataset.scenarios.len(),
                max_concurrency = self.config.max_concurrency,
                timeout_secs = ?self.config.scenario_timeout,
                max_retries = self.config.max_retries,
                "Starting evaluation"
            );
        }

        let results = if self.config.parallel_execution {
            self.evaluate_parallel(&dataset.scenarios).await?
        } else {
            self.evaluate_sequential(&dataset.scenarios).await?
        };

        let end_time = Utc::now();
        let duration = start_instant.elapsed();

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;

        if self.config.verbose {
            tracing::info!(
                passed,
                total = results.len(),
                failed,
                pass_rate_pct = %format!("{:.1}", (passed as f64 / results.len() as f64) * 100.0),
                "Evaluation complete"
            );
        }

        Ok(EvalReport {
            total: results.len(),
            passed,
            failed,
            results,
            metadata: EvalMetadata {
                started_at: start_time,
                completed_at: end_time,
                duration_secs: duration.as_secs_f64(),
                config: serde_json::to_string(&self.config).unwrap_or_default(),
            },
        })
    }

    /// Execute scenarios in parallel with concurrency control
    async fn evaluate_parallel(&self, scenarios: &[GoldenScenario]) -> Result<Vec<ScenarioResult>> {
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));

        let tasks: Vec<_> = scenarios
            .iter()
            .enumerate()
            .map(|(idx, scenario)| {
                let sem = semaphore.clone();
                let scenario = scenario.clone();
                async move {
                    let _permit = sem.acquire().await.unwrap();
                    if self.config.verbose {
                        tracing::debug!(
                            progress = idx + 1,
                            total = scenarios.len(),
                            scenario_id = %scenario.id,
                            "Evaluating scenario"
                        );
                    }
                    self.evaluate_scenario(&scenario).await
                }
            })
            .collect();

        let stream = stream::iter(tasks).buffer_unordered(self.config.max_concurrency);
        let results: Vec<Result<ScenarioResult>> = stream.collect().await;

        // Collect results, propagating first error if any
        results.into_iter().collect()
    }

    /// Execute scenarios sequentially
    async fn evaluate_sequential(
        &self,
        scenarios: &[GoldenScenario],
    ) -> Result<Vec<ScenarioResult>> {
        let mut results = Vec::with_capacity(scenarios.len());
        for (idx, scenario) in scenarios.iter().enumerate() {
            if self.config.verbose {
                tracing::debug!(
                    progress = idx + 1,
                    total = scenarios.len(),
                    scenario_id = %scenario.id,
                    "Evaluating scenario"
                );
            }
            let result = self.evaluate_scenario(scenario).await?;
            results.push(result);
        }
        Ok(results)
    }

    /// Evaluate a single scenario with retry logic
    async fn evaluate_scenario(&self, scenario: &GoldenScenario) -> Result<ScenarioResult> {
        let mut last_error = None;
        let mut retry_attempts = 0;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 && self.config.verbose {
                tracing::warn!(
                    attempt,
                    scenario_id = %scenario.id,
                    "Retrying scenario"
                );
            }

            match self.evaluate_scenario_once(scenario).await {
                Ok(mut result) => {
                    result.retry_attempts = retry_attempts;
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    retry_attempts += 1;

                    if attempt < self.config.max_retries && self.config.retry_on_failure {
                        // Exponential backoff: 1s, 2s, 4s, ...
                        let backoff = Duration::from_secs(2u64.pow(attempt));
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        // All retries failed, return error result
        let error_msg = last_error.map_or_else(|| "Unknown error".to_string(), |e| e.to_string());

        Ok(ScenarioResult {
            scenario_id: scenario.id.clone(),
            passed: false,
            output: String::new(),
            quality_score: QualityScore::default_failed(),
            latency_ms: 0,
            validation: ValidationResult {
                passed: false,
                missing_contains: vec![],
                forbidden_found: vec![],
                failure_reason: Some(format!("Failed after {retry_attempts} retries")),
            },
            error: Some(error_msg),
            retry_attempts,
            timestamp: Utc::now(),
            input: Some(scenario.query.clone()),
            tokens_used: None,
            cost_usd: None,
        })
    }

    /// Evaluate a scenario once (no retries) with timeout
    async fn evaluate_scenario_once(&self, scenario: &GoldenScenario) -> Result<ScenarioResult> {
        let start = Instant::now();

        // Run agent with timeout
        let response = match tokio::time::timeout(
            self.config.scenario_timeout,
            (self.agent_fn)(scenario.query.clone()),
        )
        .await
        {
            Err(_) => anyhow::bail!("Scenario timeout after {:?}", self.config.scenario_timeout),
            Ok(Err(e)) => anyhow::bail!("Agent execution failed: {e:#}"),
            Ok(Ok(response)) => response,
        };

        let latency = start.elapsed();

        // Extract output and usage metadata
        let output = response.output.clone();
        let tokens_used = response.total_tokens();
        let cost_usd = response.cost_usd;

        // Validate must_contain / must_not_contain
        let validation = self.validate_output(&output, scenario);

        // Score with LLM judge
        let quality_score = self
            .judge
            .score(
                &scenario.query,
                &output,
                "", // No expected response text for now
            )
            .await
            .context("Quality scoring failed")?;

        // Check quality thresholds
        let quality_passed = self.check_quality_thresholds(&quality_score, scenario);

        // Check latency threshold
        let latency_ms = latency.as_millis() as u64;
        let latency_passed = scenario
            .max_latency_ms
            .map_or(true, |max| latency_ms <= max);

        // Overall pass/fail
        let passed = validation.passed && quality_passed && latency_passed;

        let error = if passed {
            None
        } else {
            let mut errors = vec![];
            if !validation.passed {
                errors.push(format!(
                    "Validation failed: {:?}",
                    validation.failure_reason
                ));
            }
            if !quality_passed {
                errors.push(format!(
                    "Quality below threshold: {:.3} < {:.3}",
                    quality_score.overall, scenario.quality_threshold
                ));
            }
            if !latency_passed {
                errors.push(format!(
                    "Latency exceeded: {}ms > {}ms",
                    latency_ms,
                    scenario.max_latency_ms.unwrap()
                ));
            }
            Some(errors.join("; "))
        };

        Ok(ScenarioResult {
            scenario_id: scenario.id.clone(),
            passed,
            output,
            quality_score,
            latency_ms,
            validation,
            error,
            retry_attempts: 0, // Will be updated by evaluate_scenario
            timestamp: Utc::now(),
            input: Some(scenario.query.clone()),
            tokens_used,
            cost_usd,
        })
    }

    /// Validate output against expected criteria
    fn validate_output(&self, output: &str, scenario: &GoldenScenario) -> ValidationResult {
        let mut missing_contains = Vec::new();
        let mut forbidden_found = Vec::new();

        // Determine comparison function based on case sensitivity setting
        #[allow(clippy::type_complexity)] // Dynamic comparison strategy: case-sensitive or case-insensitive
        let contains_fn: Box<dyn Fn(&str, &str) -> bool> = if scenario.case_insensitive_validation {
            Box::new(|text: &str, pattern: &str| {
                text.to_lowercase().contains(&pattern.to_lowercase())
            })
        } else {
            Box::new(|text: &str, pattern: &str| text.contains(pattern))
        };

        // Check must_contain
        for expected in &scenario.expected_output_contains {
            if !contains_fn(output, expected) {
                missing_contains.push(expected.clone());
            }
        }

        // Check must_not_contain
        for forbidden in &scenario.expected_output_not_contains {
            if contains_fn(output, forbidden) {
                forbidden_found.push(forbidden.clone());
            }
        }

        let passed = missing_contains.is_empty() && forbidden_found.is_empty();

        let failure_reason = if passed {
            None
        } else {
            let mut reasons = vec![];
            if !missing_contains.is_empty() {
                reasons.push(format!("Missing required strings: {missing_contains:?}"));
            }
            if !forbidden_found.is_empty() {
                reasons.push(format!("Found forbidden strings: {forbidden_found:?}"));
            }
            Some(reasons.join("; "))
        };

        ValidationResult {
            passed,
            missing_contains,
            forbidden_found,
            failure_reason,
        }
    }

    /// Check if quality score meets all thresholds
    fn check_quality_thresholds(&self, score: &QualityScore, scenario: &GoldenScenario) -> bool {
        // Check overall threshold
        if score.overall < scenario.quality_threshold {
            return false;
        }

        // Check per-dimension thresholds if specified
        if let Some(threshold) = scenario.accuracy_threshold {
            if score.accuracy < threshold {
                return false;
            }
        }

        if let Some(threshold) = scenario.relevance_threshold {
            if score.relevance < threshold {
                return false;
            }
        }

        if let Some(threshold) = scenario.completeness_threshold {
            if score.completeness < threshold {
                return false;
            }
        }

        if let Some(threshold) = scenario.safety_threshold {
            if score.safety < threshold {
                return false;
            }
        }

        if let Some(threshold) = scenario.coherence_threshold {
            if score.coherence < threshold {
                return false;
            }
        }

        if let Some(threshold) = scenario.conciseness_threshold {
            if score.conciseness < threshold {
                return false;
            }
        }

        true
    }
}

/// Extension trait for `QualityScore` to provide default failed score
impl QualityScore {
    /// Create a default failed quality score (all zeros)
    #[must_use]
    pub fn default_failed() -> Self {
        Self {
            accuracy: 0.0,
            relevance: 0.0,
            completeness: 0.0,
            safety: 0.0,
            coherence: 0.0,
            conciseness: 0.0,
            overall: 0.0,
            reasoning: "Scenario failed before quality scoring could be performed".to_string(),
            issues: vec![],
            suggestions: vec![],
        }
    }
}

// Re-export types for backward compatibility
pub type EvalResult = ScenarioResult;
pub type EvalSummary = EvalReport;

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    /// Create a mock agent function for testing
    fn mock_agent_success() -> AgentFn {
        Arc::new(|query: String| {
            Box::pin(async move {
                let output = format!("Mock response to: {}", query);
                #[allow(deprecated)]
                Ok(AgentResponse::with_tokens(output, 100))
            }) as Pin<Box<dyn Future<Output = Result<AgentResponse>> + Send>>
        })
    }

    /// Create a mock agent function that always fails (available for future tests)
    #[allow(dead_code)] // Test: Mock for future failure scenario tests
    fn mock_agent_failure() -> AgentFn {
        Arc::new(|_query: String| {
            Box::pin(async move { Err(anyhow::anyhow!("Mock agent error")) })
                as Pin<Box<dyn Future<Output = Result<AgentResponse>> + Send>>
        })
    }

    /// Create a mock agent function that times out (available for future tests)
    #[allow(dead_code)] // Test: Mock for future timeout scenario tests
    fn mock_agent_timeout() -> AgentFn {
        Arc::new(|_query: String| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(100)).await;
                Ok(AgentResponse::text_only(
                    "This should never be reached".to_string(),
                ))
            }) as Pin<Box<dyn Future<Output = Result<AgentResponse>> + Send>>
        })
    }

    /// Create a mock quality judge for testing
    fn mock_judge_high_quality() -> MultiDimensionalJudge {
        // This will fail in tests without OPENAI_API_KEY, but structure is correct
        // In actual tests, we'd mock the ChatOpenAI
        use dashflow_openai::ChatOpenAI;
        use std::sync::Arc;
        let model = Arc::new(
            ChatOpenAI::with_config(Default::default())
                .with_model("gpt-4o-mini")
                .with_temperature(0.0),
        );
        MultiDimensionalJudge::new(model)
    }

    /// Create a test scenario
    fn test_scenario() -> GoldenScenario {
        GoldenScenario {
            id: "test_01".to_string(),
            description: "Test scenario".to_string(),
            query: "What is Rust?".to_string(),
            context: None,
            expected_output_contains: vec!["Mock response".to_string()],
            expected_output_not_contains: vec!["error".to_string()],
            quality_threshold: 0.8,
            max_latency_ms: Some(5000),
            expected_tool_calls: vec![],
            max_cost_usd: None,
            max_tokens: None,
            accuracy_threshold: None,
            relevance_threshold: None,
            completeness_threshold: None,
            safety_threshold: None,
            coherence_threshold: None,
            conciseness_threshold: None,
            case_insensitive_validation: false,
            difficulty: None,
        }
    }

    #[test]
    fn test_eval_config_default() {
        let config = EvalConfig::default();
        assert!(config.parallel_execution);
        assert_eq!(config.max_concurrency, 5);
        assert!(config.retry_on_failure);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.scenario_timeout, Duration::from_secs(30));
        assert!(!config.verbose);
    }

    #[test]
    fn test_validation_result_pass() {
        let output = "Mock response to: What is Rust?";
        let scenario = test_scenario();

        let runner = EvalRunner::new(mock_agent_success(), mock_judge_high_quality());
        let validation = runner.validate_output(output, &scenario);

        assert!(validation.passed);
        assert!(validation.missing_contains.is_empty());
        assert!(validation.forbidden_found.is_empty());
        assert!(validation.failure_reason.is_none());
    }

    #[test]
    fn test_validation_result_missing_required() {
        let output = "Some response without the required text";
        let scenario = test_scenario();

        let runner = EvalRunner::new(mock_agent_success(), mock_judge_high_quality());
        let validation = runner.validate_output(output, &scenario);

        assert!(!validation.passed);
        assert_eq!(validation.missing_contains, vec!["Mock response"]);
        assert!(validation.forbidden_found.is_empty());
        assert!(validation.failure_reason.is_some());
    }

    #[test]
    fn test_validation_result_forbidden_found() {
        let output = "Mock response with error in it";
        let scenario = test_scenario();

        let runner = EvalRunner::new(mock_agent_success(), mock_judge_high_quality());
        let validation = runner.validate_output(output, &scenario);

        assert!(!validation.passed);
        assert!(validation.missing_contains.is_empty());
        assert_eq!(validation.forbidden_found, vec!["error"]);
        assert!(validation.failure_reason.is_some());
    }

    #[test]
    fn test_check_quality_thresholds_overall() {
        let runner = EvalRunner::new(mock_agent_success(), mock_judge_high_quality());
        let scenario = test_scenario();

        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            safety: 1.0,
            coherence: 0.9,
            conciseness: 0.8,
            overall: 0.85,
            reasoning: "Good quality".to_string(),
            issues: vec![],
            suggestions: vec![],
        };

        assert!(runner.check_quality_thresholds(&score, &scenario));

        let low_score = QualityScore {
            overall: 0.7,
            ..score
        };
        assert!(!runner.check_quality_thresholds(&low_score, &scenario));
    }

    #[test]
    fn test_check_quality_thresholds_per_dimension() {
        let runner = EvalRunner::new(mock_agent_success(), mock_judge_high_quality());
        let mut scenario = test_scenario();
        scenario.accuracy_threshold = Some(0.9);
        scenario.relevance_threshold = Some(0.85);

        let score = QualityScore {
            accuracy: 0.95,
            relevance: 0.90,
            completeness: 0.85,
            safety: 1.0,
            coherence: 0.90,
            conciseness: 0.85,
            overall: 0.88,
            reasoning: "High quality".to_string(),
            issues: vec![],
            suggestions: vec![],
        };

        assert!(runner.check_quality_thresholds(&score, &scenario));

        let low_accuracy = QualityScore {
            accuracy: 0.85,
            ..score.clone()
        };
        assert!(!runner.check_quality_thresholds(&low_accuracy, &scenario));

        let low_relevance = QualityScore {
            relevance: 0.80,
            ..score
        };
        assert!(!runner.check_quality_thresholds(&low_relevance, &scenario));
    }

    #[test]
    fn test_eval_report_calculations() {
        let results = vec![
            ScenarioResult {
                scenario_id: "s1".to_string(),
                passed: true,
                output: "Output 1".to_string(),
                quality_score: QualityScore {
                    accuracy: 0.9,
                    relevance: 0.9,
                    completeness: 0.9,
                    safety: 1.0,
                    coherence: 0.9,
                    conciseness: 0.8,
                    overall: 0.90,
                    reasoning: String::new(),
                    issues: vec![],
                    suggestions: vec![],
                },
                latency_ms: 100,
                validation: ValidationResult {
                    passed: true,
                    missing_contains: vec![],
                    forbidden_found: vec![],
                    failure_reason: None,
                },
                error: None,
                retry_attempts: 0,
                timestamp: Utc::now(),
                input: None,
                tokens_used: Some(500),
                cost_usd: Some(0.001),
            },
            ScenarioResult {
                scenario_id: "s2".to_string(),
                passed: false,
                output: "Output 2".to_string(),
                quality_score: QualityScore {
                    accuracy: 0.7,
                    relevance: 0.7,
                    completeness: 0.7,
                    safety: 1.0,
                    coherence: 0.7,
                    conciseness: 0.7,
                    overall: 0.70,
                    reasoning: String::new(),
                    issues: vec![],
                    suggestions: vec![],
                },
                latency_ms: 200,
                validation: ValidationResult {
                    passed: false,
                    missing_contains: vec!["required".to_string()],
                    forbidden_found: vec![],
                    failure_reason: Some("Missing required strings".to_string()),
                },
                error: Some("Validation failed".to_string()),
                retry_attempts: 1,
                timestamp: Utc::now(),
                input: None,
                tokens_used: Some(800),
                cost_usd: Some(0.002),
            },
        ];

        let report = EvalReport {
            total: 2,
            passed: 1,
            failed: 1,
            results,
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 1.5,
                config: String::new(),
            },
        };

        assert_eq!(report.pass_rate(), 0.5);
        assert_eq!(report.avg_quality(), 0.80);
        assert_eq!(report.avg_latency_ms(), 150);
        assert_eq!(report.failed_scenarios().len(), 1);
        assert_eq!(report.retried_scenarios().len(), 1);

        // Test new cost/token methods
        assert_eq!(report.total_cost_usd(), 0.003);
        assert_eq!(report.avg_cost_usd(), 0.0015);
        assert_eq!(report.total_tokens(), 1300);
        assert_eq!(report.avg_tokens(), 650);
    }

    #[test]
    fn test_quality_score_default_failed() {
        let score = QualityScore::default_failed();
        assert_eq!(score.overall, 0.0);
        assert_eq!(score.accuracy, 0.0);
        assert_eq!(score.relevance, 0.0);
        assert!(score.reasoning.contains("failed"));
    }

    #[test]
    fn test_builder_pattern() {
        let agent_fn = mock_agent_success();
        let judge = mock_judge_high_quality();

        let runner = EvalRunner::builder()
            .agent_fn(agent_fn)
            .judge(judge)
            .max_concurrency(10)
            .scenario_timeout(Duration::from_secs(60))
            .max_retries(5)
            .parallel_execution(false)
            .verbose(true)
            .build();

        assert_eq!(runner.config.max_concurrency, 10);
        assert_eq!(runner.config.scenario_timeout, Duration::from_secs(60));
        assert_eq!(runner.config.max_retries, 5);
        assert!(!runner.config.parallel_execution);
        assert!(runner.config.verbose);
    }

    // Note: Integration tests that actually run scenarios with real LLM calls
    // should be in a separate test file with #[ignore] or conditional compilation
    // to avoid requiring OPENAI_API_KEY in CI.
}
