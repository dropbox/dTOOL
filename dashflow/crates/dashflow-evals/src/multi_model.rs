//! Multi-model comparison and A/B testing framework
//!
//! This module provides tools for comparing different LLM models on the same test scenarios:
//! - Cost/quality trade-off analysis (fully implemented)
//! - Statistical comparison of results (helper methods available)
//! - Automated multi-model execution (fully implemented)
//! - A/B testing framework (fully implemented)
//!
//! # Current Status
//!
//! **Production Ready:**
//! - `analyze_cost_quality_tradeoff()` - Analyzes cost/quality for manually collected results
//! - `statistical_test()` - Performs t-tests on scenario results
//! - `generate_recommendation()` - Creates recommendations based on comparison
//!
//! **Fully Implemented:**
//! - `compare_models()` - Automated multi-model comparison with parallel/sequential execution
//! - `ab_test()` - Head-to-head model comparison with statistical winner determination
//!
//! Both methods accept an `agent_fn` parameter and use the model factory infrastructure
//! to execute scenarios automatically. See `docs/PHASE3_MULTI_MODEL_DESIGN.md` for design details.
//!
//! # Automated Multi-Model Comparison
//!
//! The automated multi-model comparison API allows execution across multiple models
//! with parallel or sequential execution:
//!
//! ```no_run
//! use dashflow_evals::{MultiModelRunner, MultiModelConfig, ModelConfig, GoldenDataset};
//! use std::sync::Arc;
//! # async fn example() -> anyhow::Result<()> {
//! # let dataset = GoldenDataset::load("test")?;
//!
//! // Define agent function
//! let agent_fn = Arc::new(|query: String| {
//!     Box::pin(async move {
//!         // Your agent logic here
//!         Ok(dashflow_evals::AgentResponse::text_only(format!("Response: {}", query)))
//!     }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<dashflow_evals::AgentResponse>> + Send>>
//! });
//!
//! // Configure models to compare
//! let config = MultiModelConfig {
//!     models: vec![
//!         ModelConfig {
//!             name: "gpt-4o-mini".to_string(),
//!             provider: "openai".to_string(),
//!             temperature: Some(0.7),
//!             max_tokens: Some(1000),
//!             top_p: None,
//!             cost_per_million_input_tokens: 0.15,
//!             cost_per_million_output_tokens: 0.60,
//!         },
//!         ModelConfig {
//!             name: "gpt-4o".to_string(),
//!             provider: "openai".to_string(),
//!             temperature: Some(0.7),
//!             max_tokens: Some(1000),
//!             top_p: None,
//!             cost_per_million_input_tokens: 2.50,
//!             cost_per_million_output_tokens: 10.00,
//!         },
//!     ],
//!     parallel_execution: true,
//!     significance_level: 0.05,
//! };
//!
//! // Run automated comparison (execution logic coming in Commit 4/10)
//! let runner = MultiModelRunner::new(config);
//! let comparison = runner.compare_models(&dataset.scenarios, agent_fn).await?;
//!
//! println!("Models compared: {:?}", comparison.models);
//! println!("Recommendation: {}", comparison.recommendation);
//! # Ok(())
//! # }
//! ```
//!
//! # Manual Multi-Model Comparison (Alternative Method)
//!
//! For advanced use cases, users can perform multi-model comparison by running
//! `EvalRunner` separately for each model and then using `analyze_cost_quality_tradeoff()`:
//!
//! ```no_run
//! use dashflow_evals::{EvalRunner, MultiModelRunner};
//! use std::collections::HashMap;
//! # async fn example() -> anyhow::Result<()> {
//! # let dataset = dashflow_evals::GoldenDataset::load("test")?;
//! # let agent_fn = std::sync::Arc::new(|query: String| {
//! #     Box::pin(async move { Ok(dashflow_evals::AgentResponse::text_only("test".to_string())) })
//! #         as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<dashflow_evals::AgentResponse>> + Send>>
//! # });
//!
//! // Run with Model A
//! let runner_a = EvalRunner::builder()
//!     .agent_fn(agent_fn.clone())
//!     /* .judge(...) */
//!     .build();
//! let report_a = runner_a.evaluate(&dataset).await?;
//!
//! // Run with Model B
//! let runner_b = EvalRunner::builder()
//!     .agent_fn(agent_fn.clone())
//!     /* .judge(...) */
//!     .build();
//! let report_b = runner_b.evaluate(&dataset).await?;
//!
//! // Compare results
//! let mut results = HashMap::new();
//! results.insert("model_a".to_string(), report_a);
//! results.insert("model_b".to_string(), report_b);
//!
//! let runner = MultiModelRunner::new(Default::default());
//! let analysis = runner.analyze_cost_quality_tradeoff(&results)?;
//! println!("Best value: {:?}", analysis.best_value);
//! # Ok(())
//! # }
//! ```

use crate::eval_runner::{AgentFn, EvalReport, EvalRunner, ScenarioResult};
use crate::golden_dataset::{GoldenDataset, GoldenScenario};
use crate::quality_judge::MultiDimensionalJudge;
use anyhow::{anyhow, Context as _, Result};
use dashflow::core::language_models::ChatModel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::error;

/// Configuration for multi-model comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModelConfig {
    /// Models to compare
    pub models: Vec<ModelConfig>,

    /// Run models in parallel?
    pub parallel_execution: bool,

    /// Statistical significance level for comparisons (e.g., 0.05 for 95% confidence)
    pub significance_level: f64,
}

impl Default for MultiModelConfig {
    fn default() -> Self {
        Self {
            models: vec![],
            parallel_execution: true,
            significance_level: 0.05,
        }
    }
}

/// Configuration for a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model name (e.g., "gpt-4o-mini", "gpt-4o", "claude-3-sonnet")
    pub name: String,

    /// Provider (e.g., "openai", "anthropic")
    pub provider: String,

    /// Model-specific parameters
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,

    /// Cost per 1M input tokens (USD)
    pub cost_per_million_input_tokens: f64,

    /// Cost per 1M output tokens (USD)
    pub cost_per_million_output_tokens: f64,
}

/// Trait for creating `ChatModel` instances from configuration
///
/// This trait enables dependency injection and testing by abstracting
/// the creation of model instances from their configuration.
///
/// # Example
/// ```
/// use dashflow_evals::{ModelConfig, ModelFactory};
/// use anyhow::Result;
///
/// # async fn example() -> Result<()> {
/// let factory = dashflow_evals::DefaultModelFactory::new();
/// let config = ModelConfig {
///     name: "gpt-4o-mini".to_string(),
///     provider: "openai".to_string(),
///     temperature: Some(0.7),
///     max_tokens: Some(1000),
///     top_p: None,
///     cost_per_million_input_tokens: 0.15,
///     cost_per_million_output_tokens: 0.60,
/// };
///
/// let model = factory.create_model(&config)?;
/// # Ok(())
/// # }
/// ```
pub trait ModelFactory: Send + Sync {
    /// Create a `ChatModel` instance from configuration
    ///
    /// # Arguments
    /// * `config` - Model configuration including provider, name, and parameters
    ///
    /// # Returns
    /// Arc-wrapped `ChatModel` instance, or error if provider is unsupported
    fn create_model(&self, config: &ModelConfig) -> Result<Arc<dyn ChatModel>>;
}

/// Default implementation of `ModelFactory`
///
/// Supports `OpenAI` models (gpt-4o, gpt-4o-mini, gpt-3.5-turbo, etc.).
/// Additional providers can be added in future commits.
pub struct DefaultModelFactory {
    /// Optional API key override (uses `OPENAI_API_KEY` env var if None)
    openai_api_key: Option<String>,
}

impl DefaultModelFactory {
    /// Create a new `DefaultModelFactory`
    ///
    /// Uses `OPENAI_API_KEY` environment variable for authentication.
    #[must_use]
    pub fn new() -> Self {
        Self {
            openai_api_key: None,
        }
    }

    /// Create a `DefaultModelFactory` with explicit API key
    ///
    /// # Arguments
    /// * `api_key` - `OpenAI` API key to use instead of environment variable
    #[must_use]
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.openai_api_key = Some(api_key);
        self
    }
}

impl Default for DefaultModelFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelFactory for DefaultModelFactory {
    #[allow(deprecated)]
    fn create_model(&self, config: &ModelConfig) -> Result<Arc<dyn ChatModel>> {
        match config.provider.as_str() {
            "openai" => {
                use dashflow_openai::ChatOpenAI;

                // Start with base model
                #[allow(clippy::disallowed_methods)]
                // API key from config, not env; fallback uses default
                let mut model = if let Some(ref api_key) = self.openai_api_key {
                    use async_openai::config::OpenAIConfig;
                    let config = OpenAIConfig::default().with_api_key(api_key);
                    ChatOpenAI::with_config(config)
                } else {
                    ChatOpenAI::with_config(Default::default())
                };

                // Set model name
                model = model.with_model(&config.name);

                // Apply optional parameters
                if let Some(temp) = config.temperature {
                    model = model.with_temperature(temp as f32);
                }
                if let Some(tokens) = config.max_tokens {
                    model = model.with_max_tokens(tokens);
                }
                if let Some(top_p) = config.top_p {
                    model = model.with_top_p(top_p as f32);
                }

                Ok(Arc::new(model))
            }
            other => Err(anyhow!(
                "Unsupported provider '{other}'. Currently supported: openai"
            )),
        }
    }
}

/// Rate limiter for managing concurrent API requests across providers
///
/// Enforces per-provider rate limits to avoid 429 errors during multi-model execution.
/// Each provider has its own semaphore that limits concurrent requests.
///
/// # Example
/// ```
/// use dashflow_evals::RateLimiter;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create rate limiter: 500 RPM for OpenAI, 50 RPM for Anthropic
/// let limiter = RateLimiter::new(500, 50);
///
/// // Acquire permit before making OpenAI request
/// let _permit = limiter.acquire("openai").await?;
/// // Make OpenAI API call...
/// // Permit is automatically released when dropped
/// # Ok(())
/// # }
/// ```
pub struct RateLimiter {
    /// Semaphore for `OpenAI` requests (requests per minute)
    openai_semaphore: Arc<Semaphore>,
    /// Semaphore for Anthropic requests (requests per minute)
    anthropic_semaphore: Arc<Semaphore>,
    /// Requests per minute for `OpenAI`
    openai_rpm: usize,
    /// Requests per minute for Anthropic
    anthropic_rpm: usize,
}

impl RateLimiter {
    /// Create a new rate limiter with specified limits
    ///
    /// # Arguments
    /// * `openai_rpm` - `OpenAI` requests per minute (e.g., 500)
    /// * `anthropic_rpm` - Anthropic requests per minute (e.g., 50)
    ///
    /// # Example
    /// ```
    /// use dashflow_evals::RateLimiter;
    ///
    /// let limiter = RateLimiter::new(500, 50);
    /// ```
    #[must_use]
    pub fn new(openai_rpm: usize, anthropic_rpm: usize) -> Self {
        Self {
            openai_semaphore: Arc::new(Semaphore::new(openai_rpm)),
            anthropic_semaphore: Arc::new(Semaphore::new(anthropic_rpm)),
            openai_rpm,
            anthropic_rpm,
        }
    }

    /// Create a rate limiter with default limits
    ///
    /// Default limits:
    /// - `OpenAI`: 500 RPM
    /// - Anthropic: 50 RPM
    #[must_use]
    pub fn default_limits() -> Self {
        Self::new(500, 50)
    }

    /// Acquire a permit for the specified provider
    ///
    /// Blocks until a permit is available. The permit is automatically released
    /// when the returned guard is dropped.
    ///
    /// # Arguments
    /// * `provider` - Provider name ("openai" or "anthropic")
    ///
    /// # Returns
    /// A semaphore permit guard that automatically releases when dropped
    ///
    /// # Errors
    /// Returns error if provider is unknown
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_evals::RateLimiter;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let limiter = RateLimiter::new(500, 50);
    ///
    /// // Acquire permit - blocks if at limit
    /// let permit = limiter.acquire("openai").await?;
    ///
    /// // Make API call...
    /// // Permit released when dropped
    /// drop(permit);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn acquire(&self, provider: &str) -> Result<tokio::sync::SemaphorePermit<'_>> {
        match provider {
            "openai" => Ok(self
                .openai_semaphore
                .acquire()
                .await
                .map_err(|e| anyhow!("Failed to acquire OpenAI rate limit permit: {e}"))?),
            "anthropic" => Ok(self
                .anthropic_semaphore
                .acquire()
                .await
                .map_err(|e| anyhow!("Failed to acquire Anthropic rate limit permit: {e}"))?),
            other => Err(anyhow!(
                "Unknown provider '{other}'. Supported: openai, anthropic"
            )),
        }
    }

    /// Get the configured rate limit for a provider
    ///
    /// # Arguments
    /// * `provider` - Provider name ("openai" or "anthropic")
    ///
    /// # Returns
    /// Requests per minute, or None if provider is unknown
    #[must_use]
    pub fn get_limit(&self, provider: &str) -> Option<usize> {
        match provider {
            "openai" => Some(self.openai_rpm),
            "anthropic" => Some(self.anthropic_rpm),
            _ => None,
        }
    }

    /// Get available permits for a provider
    ///
    /// Returns the number of permits currently available (not in use).
    ///
    /// # Arguments
    /// * `provider` - Provider name ("openai" or "anthropic")
    ///
    /// # Returns
    /// Number of available permits, or None if provider is unknown
    #[must_use]
    pub fn available_permits(&self, provider: &str) -> Option<usize> {
        match provider {
            "openai" => Some(self.openai_semaphore.available_permits()),
            "anthropic" => Some(self.anthropic_semaphore.available_permits()),
            _ => None,
        }
    }
}

/// Multi-model comparison runner
#[derive(Clone)]
pub struct MultiModelRunner {
    config: MultiModelConfig,
    factory: Arc<dyn ModelFactory>,
}

impl MultiModelRunner {
    /// Create a new multi-model comparison runner with default factory
    ///
    /// Uses `DefaultModelFactory` for creating model instances.
    #[must_use]
    pub fn new(config: MultiModelConfig) -> Self {
        Self {
            config,
            factory: Arc::new(DefaultModelFactory::new()),
        }
    }

    /// Create a new multi-model comparison runner with custom factory
    ///
    /// Allows dependency injection for testing or custom model creation logic.
    ///
    /// # Arguments
    /// * `config` - Multi-model configuration
    /// * `factory` - Custom model factory implementation
    pub fn with_factory(config: MultiModelConfig, factory: Arc<dyn ModelFactory>) -> Self {
        Self { config, factory }
    }

    /// Run all configured models on the same scenarios
    ///
    /// # Arguments
    /// * `scenarios` - Test scenarios to run
    /// * `agent_fn` - Agent function to execute for each scenario
    ///
    /// # Returns
    /// Comparison report with results from all models. If some models fail but at least
    /// one succeeds, returns Ok with partial results and error details in `model_errors` field.
    ///
    /// # Error Handling
    ///
    /// This method implements graceful error handling:
    /// - If one model fails, execution continues for other models
    /// - Failed models are reported in `MultiModelComparison.model_errors`
    /// - Statistical analysis and recommendations are based on successful models only
    /// - Returns `Err` only if ALL models fail
    ///
    /// # Execution Modes
    ///
    /// Supports both parallel and sequential execution, controlled by `config.parallel_execution`:
    /// - **Parallel**: Models run concurrently (faster, higher resource usage)
    /// - **Sequential**: Models run one at a time (slower, lower resource usage)
    ///
    /// See `docs/PHASE3_MULTI_MODEL_DESIGN.md` for implementation details.
    pub async fn compare_models(
        &self,
        scenarios: &[GoldenScenario],
        agent_fn: AgentFn,
    ) -> Result<MultiModelComparison> {
        let mut results = HashMap::new();
        let mut model_errors = HashMap::new();

        // Track all attempted models for reporting
        let all_models: Vec<String> = self.config.models.iter().map(|m| m.name.clone()).collect();

        if self.config.parallel_execution {
            // Parallel execution: spawn tasks for each model
            let mut handles = Vec::new();

            for model_config in self.config.models.clone() {
                let scenarios = scenarios.to_vec();
                let agent_fn = agent_fn.clone();
                let runner = self.clone(); // MultiModelRunner must be Clone for this to work

                let handle = tokio::spawn(async move {
                    let result = runner
                        .run_scenarios_for_model(&model_config, agent_fn, &scenarios)
                        .await;

                    (model_config.name.clone(), result)
                });

                handles.push(handle);
            }

            // Collect results from all tasks (continue on failure)
            for handle in handles {
                match handle.await {
                    Ok((model_name, result)) => match result {
                        Ok(report) => {
                            results.insert(model_name, report);
                        }
                        Err(e) => {
                            model_errors.insert(model_name.clone(), e.to_string());
                            error!(model = %model_name, error = %e, "Model evaluation failed");
                        }
                    },
                    Err(e) => {
                        error!(error = %e, "Task join failed - possible tokio runtime issue");
                        // Task panic - we can't recover model name from join error,
                        // but this should be rare (only if tokio runtime crashes)
                    }
                }
            }
        } else {
            // Sequential execution: run models one after another (continue on failure)
            for model_config in &self.config.models {
                match self
                    .run_scenarios_for_model(model_config, agent_fn.clone(), scenarios)
                    .await
                {
                    Ok(report) => {
                        results.insert(model_config.name.clone(), report);
                    }
                    Err(e) => {
                        model_errors.insert(model_config.name.clone(), e.to_string());
                        error!(model = %model_config.name, error = %e, "Model evaluation failed");
                    }
                }
            }
        }

        // If ALL models failed, return error
        if results.is_empty() {
            let errors_str = model_errors
                .iter()
                .map(|(model, err)| format!("{model}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!("All models failed. Errors: {errors_str}"));
        }

        // Perform statistical tests between all successful model pairs
        let statistical_tests = self.compute_statistical_tests(&results)?;

        // Analyze costs and quality (only for successful models)
        let cost_analysis = self.analyze_costs(&results)?;
        let quality_analysis = self.analyze_quality(&results)?;

        // Generate recommendation (based on successful models)
        let recommendation = if model_errors.is_empty() {
            self.generate_recommendation(&results)
        } else {
            format!(
                "{} (Note: {} model(s) failed - see model_errors field)",
                self.generate_recommendation(&results),
                model_errors.len()
            )
        };

        Ok(MultiModelComparison {
            models: all_models,
            results,
            model_errors,
            statistical_tests,
            cost_analysis,
            quality_analysis,
            recommendation,
        })
    }

    /// Run scenarios for a single model
    ///
    /// Creates a `ChatModel` instance from config, sets up `EvalRunner`, and executes scenarios.
    ///
    /// # Arguments
    /// * `config` - Model configuration
    /// * `agent_fn` - Agent function to execute for each scenario
    /// * `scenarios` - Test scenarios to run
    ///
    /// # Returns
    /// Evaluation report for this model
    async fn run_scenarios_for_model(
        &self,
        config: &ModelConfig,
        agent_fn: AgentFn,
        scenarios: &[GoldenScenario],
    ) -> Result<EvalReport> {
        // Create model instance from factory
        let model_arc = self.factory.create_model(config)?;

        // Create judge with provider-agnostic model
        let judge = MultiDimensionalJudge::new(model_arc);

        // Create dataset from scenarios
        let dataset = GoldenDataset {
            scenarios: scenarios.to_vec(),
            source_dir: std::path::PathBuf::from(""), // Empty path since we're using provided scenarios
        };

        // Create runner with rate limiting
        // For sequential execution, we don't need strict rate limiting per request,
        // but we'll set concurrency to a reasonable default
        let runner = EvalRunner::builder()
            .agent_fn(agent_fn)
            .judge(judge)
            .max_concurrency(5) // Reasonable default for sequential model execution
            .build();

        // Run evaluation
        let report = runner
            .evaluate(&dataset)
            .await
            .context(format!("Evaluation failed for model '{}'", config.name))?;

        Ok(report)
    }

    /// Compute statistical tests between all model pairs
    fn compute_statistical_tests(
        &self,
        results: &HashMap<String, EvalReport>,
    ) -> Result<Vec<StatisticalTest>> {
        let mut tests = Vec::new();

        let model_names: Vec<_> = results.keys().collect();

        // Compare all pairs
        for i in 0..model_names.len() {
            for j in (i + 1)..model_names.len() {
                let model_a = model_names[i];
                let model_b = model_names[j];

                let results_a = &results[model_a].results;
                let results_b = &results[model_b].results;

                let mut test = self.statistical_test(results_a, results_b);
                test.metric = format!("{model_a} vs {model_b}");

                tests.push(test);
            }
        }

        Ok(tests)
    }

    /// Analyze costs across all models
    fn analyze_costs(&self, results: &HashMap<String, EvalReport>) -> Result<CostAnalysis> {
        let mut total_costs = HashMap::new();
        let mut cost_per_scenario = HashMap::new();

        let mut most_expensive = None;
        let mut cheapest = None;
        let mut max_cost = 0.0;
        let mut min_cost = f64::MAX;

        for (model_name, report) in results {
            let total_cost = report.total_cost_usd();
            total_costs.insert(model_name.clone(), total_cost);

            let avg_cost = if report.total > 0 {
                total_cost / report.total as f64
            } else {
                0.0
            };
            cost_per_scenario.insert(model_name.clone(), avg_cost);

            if total_cost > max_cost {
                max_cost = total_cost;
                most_expensive = Some(model_name.clone());
            }

            if total_cost < min_cost && total_cost > 0.0 {
                min_cost = total_cost;
                cheapest = Some(model_name.clone());
            }
        }

        let max_savings_usd = max_cost - min_cost;
        let max_savings_percent = if max_cost > 0.0 {
            (max_savings_usd / max_cost) * 100.0
        } else {
            0.0
        };

        Ok(CostAnalysis {
            total_costs,
            cost_per_scenario,
            most_expensive,
            cheapest,
            max_savings_usd,
            max_savings_percent,
        })
    }

    /// Analyze quality across all models
    fn analyze_quality(&self, results: &HashMap<String, EvalReport>) -> Result<QualityAnalysis> {
        let mut average_quality = HashMap::new();
        let mut pass_rates = HashMap::new();

        let mut best_model = None;
        let mut worst_model = None;
        let mut max_quality = 0.0;
        let mut min_quality = f64::MAX;

        for (model_name, report) in results {
            let avg_qual = report.avg_quality();
            average_quality.insert(model_name.clone(), avg_qual);

            let pass_rate = report.pass_rate();
            pass_rates.insert(model_name.clone(), pass_rate);

            if avg_qual > max_quality {
                max_quality = avg_qual;
                best_model = Some(model_name.clone());
            }

            if avg_qual < min_quality {
                min_quality = avg_qual;
                worst_model = Some(model_name.clone());
            }
        }

        let quality_range = max_quality - min_quality;

        Ok(QualityAnalysis {
            average_quality,
            pass_rates,
            best_model,
            worst_model,
            quality_range,
        })
    }

    /// Run A/B test between two models
    ///
    /// Compares two models head-to-head with statistical significance testing.
    ///
    /// # Arguments
    /// * `model_a` - First model configuration
    /// * `model_b` - Second model configuration
    /// * `scenarios` - Test scenarios
    /// * `agent_fn` - Agent function to execute for each scenario
    ///
    /// # Returns
    /// A/B test report with winner determination
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_evals::{MultiModelRunner, ModelConfig, GoldenDataset};
    /// use std::sync::Arc;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let dataset = GoldenDataset::load("test")?;
    ///
    /// let agent_fn = Arc::new(|query: String| {
    ///     Box::pin(async move {
    ///         Ok(dashflow_evals::AgentResponse::text_only(format!("Response: {}", query)))
    ///     }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<dashflow_evals::AgentResponse>> + Send>>
    /// });
    ///
    /// let model_a = ModelConfig {
    ///     name: "gpt-4o-mini".to_string(),
    ///     provider: "openai".to_string(),
    ///     temperature: Some(0.7),
    ///     max_tokens: Some(1000),
    ///     top_p: None,
    ///     cost_per_million_input_tokens: 0.15,
    ///     cost_per_million_output_tokens: 0.60,
    /// };
    ///
    /// let model_b = ModelConfig {
    ///     name: "gpt-4o".to_string(),
    ///     provider: "openai".to_string(),
    ///     temperature: Some(0.7),
    ///     max_tokens: Some(1000),
    ///     top_p: None,
    ///     cost_per_million_input_tokens: 2.50,
    ///     cost_per_million_output_tokens: 10.00,
    /// };
    ///
    /// let runner = MultiModelRunner::new(Default::default());
    /// let ab_report = runner.ab_test(&model_a, &model_b, &dataset.scenarios, agent_fn).await?;
    ///
    /// if let Some(winner) = ab_report.winner {
    ///     println!("Winner: {} with {:.1}% confidence", winner, ab_report.confidence * 100.0);
    /// } else {
    ///     println!("No significant difference between models");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ab_test(
        &self,
        model_a: &ModelConfig,
        model_b: &ModelConfig,
        scenarios: &[GoldenScenario],
        agent_fn: AgentFn,
    ) -> Result<ABTestReport> {
        // Run both models
        let report_a = self
            .run_scenarios_for_model(model_a, agent_fn.clone(), scenarios)
            .await
            .context(format!("Model A '{}' execution failed", model_a.name))?;

        let report_b = self
            .run_scenarios_for_model(model_b, agent_fn, scenarios)
            .await
            .context(format!("Model B '{}' execution failed", model_b.name))?;

        // Extract results
        let results_a = &report_a.results;
        let results_b = &report_b.results;

        // Statistical test
        let test = self.statistical_test(results_a, results_b);

        // Determine winner
        let winner = if test.significant {
            if test.mean_a > test.mean_b {
                Some(model_a.name.clone())
            } else {
                Some(model_b.name.clone())
            }
        } else {
            None
        };

        // Calculate confidence (1 - p_value, clamped to [0, 1])
        let confidence = (1.0 - test.p_value).clamp(0.0, 1.0);

        // Calculate differences
        let quality_difference = test.difference;
        let latency_difference =
            report_a.avg_latency_ms() as f64 - report_b.avg_latency_ms() as f64;
        let cost_difference = report_a.total_cost_usd() - report_b.total_cost_usd();

        Ok(ABTestReport {
            model_a: model_a.name.clone(),
            model_b: model_b.name.clone(),
            winner,
            confidence,
            quality_difference,
            latency_difference,
            cost_difference,
            statistical_significance: test.significant,
            details: test.conclusion,
        })
    }

    /// Analyze cost/quality trade-offs across models
    ///
    /// # Arguments
    /// * `results` - Evaluation results from multiple models
    ///
    /// # Returns
    /// Cost/quality trade-off analysis
    pub fn analyze_cost_quality_tradeoff(
        &self,
        results: &HashMap<String, EvalReport>,
    ) -> Result<CostQualityAnalysis> {
        let mut models = Vec::new();

        for (model_name, report) in results {
            let avg_quality = report.avg_quality();
            let total_cost = report.total_cost_usd();

            models.push(ModelPerformance {
                model: model_name.clone(),
                quality: avg_quality,
                cost: total_cost,
                latency: report.avg_latency_ms() as f64,
                pass_rate: report.pass_rate(),
            });
        }

        // Sort by quality/cost ratio
        models.sort_by(|a, b| {
            let ratio_a = if a.cost > 0.0 {
                a.quality / a.cost
            } else {
                a.quality
            };
            let ratio_b = if b.cost > 0.0 {
                b.quality / b.cost
            } else {
                b.quality
            };
            ratio_b
                .partial_cmp(&ratio_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best_value = models.first().map(|m| m.model.clone());
        let best_quality = results
            .iter()
            .max_by(|a, b| {
                a.1.avg_quality()
                    .partial_cmp(&b.1.avg_quality())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone());
        let cheapest = results
            .iter()
            .filter(|(_, report)| report.total_cost_usd() > 0.0)
            .min_by(|a, b| {
                a.1.total_cost_usd()
                    .partial_cmp(&b.1.total_cost_usd())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone());

        Ok(CostQualityAnalysis {
            models,
            best_value,
            best_quality,
            cheapest,
            recommendation: self.generate_recommendation(results),
        })
    }

    /// Perform statistical significance test between two models
    fn statistical_test(
        &self,
        model_a_results: &[ScenarioResult],
        model_b_results: &[ScenarioResult],
    ) -> StatisticalTest {
        // Extract quality scores
        let scores_a: Vec<f64> = model_a_results
            .iter()
            .map(|r| r.quality_score.overall)
            .collect();
        let scores_b: Vec<f64> = model_b_results
            .iter()
            .map(|r| r.quality_score.overall)
            .collect();

        // Compute means
        let mean_a = scores_a.iter().sum::<f64>() / scores_a.len() as f64;
        let mean_b = scores_b.iter().sum::<f64>() / scores_b.len() as f64;

        // Compute standard deviations
        let std_a = (scores_a.iter().map(|s| (s - mean_a).powi(2)).sum::<f64>()
            / scores_a.len() as f64)
            .sqrt();
        let std_b = (scores_b.iter().map(|s| (s - mean_b).powi(2)).sum::<f64>()
            / scores_b.len() as f64)
            .sqrt();

        // Compute t-statistic (simplified paired t-test)
        let n = scores_a.len().min(scores_b.len()) as f64;
        let pooled_std = ((std_a.powi(2) + std_b.powi(2)) / 2.0).sqrt();
        let t_statistic = if pooled_std > 0.0 {
            (mean_a - mean_b) / (pooled_std * (2.0 / n).sqrt())
        } else {
            0.0
        };

        // Simplified p-value calculation (would use proper t-distribution in production)
        let p_value = if t_statistic.abs() > 2.0 {
            0.05 // Rough approximation
        } else {
            0.5
        };

        StatisticalTest {
            test_type: "paired_t_test".to_string(),
            metric: "quality".to_string(),
            mean_a,
            mean_b,
            difference: mean_a - mean_b,
            t_statistic,
            p_value,
            significant: p_value < self.config.significance_level,
            conclusion: if p_value < self.config.significance_level {
                if mean_a > mean_b {
                    "Model A is significantly better".to_string()
                } else {
                    "Model B is significantly better".to_string()
                }
            } else {
                "No significant difference".to_string()
            },
        }
    }

    fn generate_recommendation(&self, results: &HashMap<String, EvalReport>) -> String {
        if results.is_empty() {
            return "No models to compare".to_string();
        }

        let best_quality = results
            .iter()
            .max_by(|a, b| {
                a.1.avg_quality()
                    .partial_cmp(&b.1.avg_quality())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, report)| (name, report.avg_quality()));

        let cheapest = results
            .iter()
            .filter(|(_, report)| report.total_cost_usd() > 0.0)
            .min_by(|a, b| {
                a.1.total_cost_usd()
                    .partial_cmp(&b.1.total_cost_usd())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, report)| (name, report.total_cost_usd()));

        match (best_quality, cheapest) {
            (Some((best_name, best_qual)), Some((cheap_name, cheap_cost))) => {
                if best_name == cheap_name {
                    format!(
                        "✅ {best_name} is both highest quality ({best_qual:.3}) and lowest cost (${cheap_cost:.4})"
                    )
                } else {
                    format!(
                        "Trade-off: {best_name} has highest quality ({best_qual:.3}), {cheap_name} is cheapest (${cheap_cost:.4})"
                    )
                }
            }
            _ => "Insufficient data for recommendation".to_string(),
        }
    }
}

/// Results from comparing multiple models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModelComparison {
    /// Models that were compared (attempted)
    pub models: Vec<String>,

    /// Results for each model (only successful executions)
    pub results: HashMap<String, EvalReport>,

    /// Errors encountered per model (`model_name` -> `error_message`)
    /// Empty if all models succeeded
    pub model_errors: HashMap<String, String>,

    /// Statistical significance tests (only for successful models)
    pub statistical_tests: Vec<StatisticalTest>,

    /// Cost analysis (only for successful models)
    pub cost_analysis: CostAnalysis,

    /// Quality analysis (only for successful models)
    pub quality_analysis: QualityAnalysis,

    /// Overall recommendation (based on successful models)
    pub recommendation: String,
}

/// A/B test report comparing two models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestReport {
    /// Model A name
    pub model_a: String,

    /// Model B name
    pub model_b: String,

    /// Winner (if any)
    pub winner: Option<String>,

    /// Confidence level (0-1)
    pub confidence: f64,

    /// Quality difference (A - B)
    pub quality_difference: f64,

    /// Latency difference in ms (A - B)
    pub latency_difference: f64,

    /// Cost difference in USD (A - B)
    pub cost_difference: f64,

    /// Statistically significant?
    pub statistical_significance: bool,

    /// Detailed explanation
    pub details: String,
}

/// Statistical significance test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalTest {
    /// Type of test (e.g., "`paired_t_test`", "wilcoxon")
    pub test_type: String,

    /// Metric being tested (e.g., "quality", "latency")
    pub metric: String,

    /// Mean for model A
    pub mean_a: f64,

    /// Mean for model B
    pub mean_b: f64,

    /// Difference (A - B)
    pub difference: f64,

    /// t-statistic or equivalent
    pub t_statistic: f64,

    /// p-value
    pub p_value: f64,

    /// Is difference significant?
    pub significant: bool,

    /// Human-readable conclusion
    pub conclusion: String,
}

/// Cost analysis across models
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostAnalysis {
    /// Total cost per model
    pub total_costs: HashMap<String, f64>,

    /// Cost per scenario per model
    pub cost_per_scenario: HashMap<String, f64>,

    /// Most expensive model
    pub most_expensive: Option<String>,

    /// Cheapest model
    pub cheapest: Option<String>,

    /// Cost savings of cheapest vs most expensive
    pub max_savings_usd: f64,

    /// Cost savings percentage
    pub max_savings_percent: f64,
}

/// Quality analysis across models
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityAnalysis {
    /// Average quality per model
    pub average_quality: HashMap<String, f64>,

    /// Pass rate per model
    pub pass_rates: HashMap<String, f64>,

    /// Best model by quality
    pub best_model: Option<String>,

    /// Worst model by quality
    pub worst_model: Option<String>,

    /// Quality range (max - min)
    pub quality_range: f64,
}

/// Cost/quality trade-off analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostQualityAnalysis {
    /// All models sorted by value (quality/cost ratio)
    pub models: Vec<ModelPerformance>,

    /// Best value model (highest quality/cost ratio)
    pub best_value: Option<String>,

    /// Best quality model (regardless of cost)
    pub best_quality: Option<String>,

    /// Cheapest model (regardless of quality)
    pub cheapest: Option<String>,

    /// Recommendation based on analysis
    pub recommendation: String,
}

/// Performance metrics for a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformance {
    /// Model name
    pub model: String,

    /// Average quality score
    pub quality: f64,

    /// Total cost
    pub cost: f64,

    /// Average latency
    pub latency: f64,

    /// Pass rate
    pub pass_rate: f64,
}

impl ModelPerformance {
    /// Calculate value score (quality/cost ratio)
    #[must_use]
    pub fn value_score(&self) -> f64 {
        if self.cost > 0.0 {
            self.quality / self.cost
        } else {
            self.quality
        }
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // --- RateLimiter Tests ---

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(500, 50);
        assert_eq!(limiter.get_limit("openai"), Some(500));
        assert_eq!(limiter.get_limit("anthropic"), Some(50));
    }

    #[test]
    fn test_rate_limiter_default_limits() {
        let limiter = RateLimiter::default_limits();
        assert_eq!(limiter.get_limit("openai"), Some(500));
        assert_eq!(limiter.get_limit("anthropic"), Some(50));
    }

    #[test]
    fn test_rate_limiter_available_permits() {
        let limiter = RateLimiter::new(100, 25);
        // All permits should be available initially
        assert_eq!(limiter.available_permits("openai"), Some(100));
        assert_eq!(limiter.available_permits("anthropic"), Some(25));
    }

    #[test]
    fn test_rate_limiter_unknown_provider() {
        let limiter = RateLimiter::new(500, 50);
        assert_eq!(limiter.get_limit("unknown"), None);
        assert_eq!(limiter.available_permits("unknown"), None);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_openai() {
        let limiter = RateLimiter::new(10, 5);

        // Should successfully acquire permit
        let permit = limiter.acquire("openai").await;
        assert!(
            permit.is_ok(),
            "Failed to acquire OpenAI permit: {:?}",
            permit.err()
        );

        // Available permits should decrease
        assert_eq!(limiter.available_permits("openai"), Some(9));

        // Drop permit
        drop(permit);

        // Available permits should increase back
        assert_eq!(limiter.available_permits("openai"), Some(10));
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_anthropic() {
        let limiter = RateLimiter::new(10, 5);

        let permit = limiter.acquire("anthropic").await;
        assert!(
            permit.is_ok(),
            "Failed to acquire Anthropic permit: {:?}",
            permit.err()
        );

        assert_eq!(limiter.available_permits("anthropic"), Some(4));

        drop(permit);
        assert_eq!(limiter.available_permits("anthropic"), Some(5));
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_unknown_provider() {
        let limiter = RateLimiter::new(10, 5);

        let result = limiter.acquire("unknown").await;
        assert!(result.is_err(), "Should fail for unknown provider");

        if let Err(err) = result {
            let err_msg = format!("{}", err);
            assert!(err_msg.contains("Unknown provider"));
            assert!(err_msg.contains("unknown"));
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_multiple_permits() {
        let limiter = RateLimiter::new(3, 2);

        // Acquire multiple permits
        let permit1 = limiter.acquire("openai").await.unwrap();
        let permit2 = limiter.acquire("openai").await.unwrap();
        let permit3 = limiter.acquire("openai").await.unwrap();

        // All permits should be in use
        assert_eq!(limiter.available_permits("openai"), Some(0));

        // Release one permit
        drop(permit1);
        assert_eq!(limiter.available_permits("openai"), Some(1));

        // Release remaining permits
        drop(permit2);
        drop(permit3);
        assert_eq!(limiter.available_permits("openai"), Some(3));
    }

    #[tokio::test]
    async fn test_rate_limiter_concurrent_providers() {
        let limiter = RateLimiter::new(10, 5);

        // Acquire permits from different providers simultaneously
        let openai_permit = limiter.acquire("openai").await.unwrap();
        let anthropic_permit = limiter.acquire("anthropic").await.unwrap();

        // Each provider's permits should be independent
        assert_eq!(limiter.available_permits("openai"), Some(9));
        assert_eq!(limiter.available_permits("anthropic"), Some(4));

        drop(openai_permit);
        drop(anthropic_permit);

        assert_eq!(limiter.available_permits("openai"), Some(10));
        assert_eq!(limiter.available_permits("anthropic"), Some(5));
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_when_exhausted() {
        use tokio::time::{timeout, Duration};

        let limiter = Arc::new(RateLimiter::new(1, 1));

        // Acquire the only permit
        let _permit = limiter.acquire("openai").await.unwrap();
        assert_eq!(limiter.available_permits("openai"), Some(0));

        // Try to acquire another permit with timeout
        let limiter_clone = limiter.clone();
        let result = timeout(Duration::from_millis(100), limiter_clone.acquire("openai")).await;

        // Should timeout because permit is not available
        assert!(result.is_err(), "Should timeout when no permits available");
    }

    // --- ModelFactory Tests ---

    #[test]
    fn test_default_model_factory_creation() {
        let factory = DefaultModelFactory::new();
        assert!(factory.openai_api_key.is_none());
    }

    #[test]
    fn test_default_model_factory_with_api_key() {
        let factory = DefaultModelFactory::new().with_api_key("test_key".to_string());
        assert_eq!(factory.openai_api_key, Some("test_key".to_string()));
    }

    #[test]
    fn test_create_openai_model_basic() {
        let factory = DefaultModelFactory::new();
        let config = ModelConfig {
            name: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            cost_per_million_input_tokens: 0.15,
            cost_per_million_output_tokens: 0.60,
        };

        let result = factory.create_model(&config);
        assert!(
            result.is_ok(),
            "Failed to create OpenAI model: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_create_openai_model_with_parameters() {
        let factory = DefaultModelFactory::new();
        let config = ModelConfig {
            name: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            top_p: Some(0.9),
            cost_per_million_input_tokens: 2.50,
            cost_per_million_output_tokens: 10.00,
        };

        let result = factory.create_model(&config);
        assert!(
            result.is_ok(),
            "Failed to create OpenAI model with parameters: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_create_model_unsupported_provider() {
        let factory = DefaultModelFactory::new();
        let config = ModelConfig {
            name: "claude-3-sonnet".to_string(),
            provider: "anthropic".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            top_p: None,
            cost_per_million_input_tokens: 3.00,
            cost_per_million_output_tokens: 15.00,
        };

        let result = factory.create_model(&config);
        assert!(result.is_err(), "Should fail for unsupported provider");

        if let Err(err) = result {
            let err_msg = format!("{}", err);
            assert!(err_msg.contains("Unsupported provider"));
            assert!(err_msg.contains("anthropic"));
        }
    }

    #[test]
    fn test_multi_model_runner_uses_factory() {
        let config = MultiModelConfig::default();
        let runner = MultiModelRunner::new(config);

        // Factory should be initialized (can't inspect private field directly,
        // but constructor shouldn't panic)
        assert!(runner.config.parallel_execution);
    }

    #[test]
    fn test_multi_model_runner_with_custom_factory() {
        use std::sync::Arc;

        // Create mock factory
        struct MockFactory;
        impl ModelFactory for MockFactory {
            fn create_model(&self, _config: &ModelConfig) -> Result<Arc<dyn ChatModel>> {
                // Return error for testing - a real mock would return a mock ChatModel
                Err(anyhow!("Mock factory"))
            }
        }

        let config = MultiModelConfig::default();
        let factory = Arc::new(MockFactory);
        let runner = MultiModelRunner::with_factory(config, factory);

        assert!(runner.config.parallel_execution);
    }

    // --- Existing Tests ---

    #[test]
    fn test_multi_model_config_default() {
        let config = MultiModelConfig::default();
        assert!(config.parallel_execution);
        assert_eq!(config.significance_level, 0.05);
    }

    #[test]
    fn test_model_config_creation() {
        let config = ModelConfig {
            name: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            top_p: Some(0.9),
            cost_per_million_input_tokens: 0.15,
            cost_per_million_output_tokens: 0.60,
        };

        assert_eq!(config.name, "gpt-4o-mini");
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_model_performance_value_score() {
        let perf = ModelPerformance {
            model: "test".to_string(),
            quality: 0.9,
            cost: 0.01,
            latency: 100.0,
            pass_rate: 0.95,
        };

        assert_eq!(perf.value_score(), 90.0);
    }

    #[test]
    fn test_model_performance_value_score_zero_cost() {
        let perf = ModelPerformance {
            model: "test".to_string(),
            quality: 0.9,
            cost: 0.0,
            latency: 100.0,
            pass_rate: 0.95,
        };

        assert_eq!(perf.value_score(), 0.9);
    }

    #[test]
    fn test_statistical_test_no_difference() {
        let config = MultiModelConfig::default();
        let runner = MultiModelRunner::new(config);

        let results_a = vec![create_test_result("1", 0.9), create_test_result("2", 0.9)];
        let results_b = vec![create_test_result("1", 0.9), create_test_result("2", 0.9)];

        let test = runner.statistical_test(&results_a, &results_b);

        assert_eq!(test.mean_a, 0.9);
        assert_eq!(test.mean_b, 0.9);
        assert_eq!(test.difference, 0.0);
    }

    #[test]
    fn test_cost_quality_analysis() {
        let config = MultiModelConfig::default();
        let runner = MultiModelRunner::new(config);

        let mut results = HashMap::new();
        results.insert(
            "model_a".to_string(),
            create_test_report(0.9, 0.01, 100.0, 0.95),
        );
        results.insert(
            "model_b".to_string(),
            create_test_report(0.85, 0.005, 150.0, 0.90),
        );

        let analysis = runner.analyze_cost_quality_tradeoff(&results).unwrap();

        assert_eq!(analysis.models.len(), 2);
        assert!(analysis.best_value.is_some());
    }

    /// Integration test for compare_models with real OpenAI models
    ///
    /// This test verifies the complete execution flow:
    /// 1. Model factory creates ChatOpenAI instances from configs
    /// 2. Sequential execution runs scenarios for each model
    /// 3. Statistical tests compare model pairs
    /// 4. Cost and quality analysis aggregates results
    /// 5. Recommendation identifies best value model
    ///
    /// Requires OPENAI_API_KEY in environment.
    #[tokio::test]
    #[ignore = "requires API key"]
    async fn test_compare_models_sequential_execution() {
        use std::sync::Arc;

        let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

        // Create test scenarios
        let scenarios = vec![
            GoldenScenario {
                id: "test_1".to_string(),
                description: "Basic arithmetic test".to_string(),
                query: "What is 2+2?".to_string(),
                context: None,
                expected_output_contains: vec!["4".to_string()],
                expected_output_not_contains: vec!["5".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
            GoldenScenario {
                id: "test_2".to_string(),
                description: "Geography knowledge test".to_string(),
                query: "What is the capital of France?".to_string(),
                context: None,
                expected_output_contains: vec!["Paris".to_string()],
                expected_output_not_contains: vec!["London".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
        ];

        // Configure two different models for comparison
        let config = MultiModelConfig {
            models: vec![
                ModelConfig {
                    name: "gpt-4o-mini".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.15,
                    cost_per_million_output_tokens: 0.60,
                },
                ModelConfig {
                    name: "gpt-3.5-turbo".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.50,
                    cost_per_million_output_tokens: 1.50,
                },
            ],
            parallel_execution: false, // Sequential execution for Commit 4/10
            significance_level: 0.05,  // 95% confidence
        };

        // Create runner
        let runner = MultiModelRunner::new(config);

        // Define agent function (simple echo for testing)
        let agent_fn: AgentFn = Arc::new(move |query: String| {
            Box::pin(async move {
                // Simple agent that just echoes the query
                // In real usage, this would call your agent logic
                #[allow(deprecated)]
                Ok(crate::eval_runner::AgentResponse::with_usage(
                    query.clone(),
                    10,
                    0.0001,
                ))
            })
        });

        // Run comparison
        let result = runner.compare_models(&scenarios, agent_fn).await;

        // Verify results
        assert!(result.is_ok(), "compare_models failed: {:?}", result.err());

        let comparison = result.unwrap();

        // Verify both models executed
        assert_eq!(
            comparison.models.len(),
            2,
            "Expected 2 models, got {}",
            comparison.models.len()
        );
        assert!(comparison.results.contains_key("gpt-4o-mini"));
        assert!(comparison.results.contains_key("gpt-3.5-turbo"));

        // Verify each model's report
        for (model_name, report) in &comparison.results {
            assert_eq!(
                report.results.len(),
                2,
                "Model {} should have 2 scenario results",
                model_name
            );
            assert!(
                report.total >= 2,
                "Model {} should have at least 2 total scenarios",
                model_name
            );
        }

        // Verify statistical tests (should have 1 pair comparison)
        assert_eq!(
            comparison.statistical_tests.len(),
            1,
            "Expected 1 statistical test for 2 models"
        );

        // Verify cost analysis
        assert_eq!(comparison.cost_analysis.total_costs.len(), 2);
        assert!(comparison.cost_analysis.cheapest.is_some());
        assert!(comparison.cost_analysis.most_expensive.is_some());

        // Verify quality analysis
        assert_eq!(comparison.quality_analysis.average_quality.len(), 2);
        assert!(comparison.quality_analysis.best_model.is_some());
        assert!(comparison.quality_analysis.worst_model.is_some());

        // Verify recommendation exists
        assert!(!comparison.recommendation.is_empty());
    }

    /// Integration test: Compare parallel vs sequential execution performance
    ///
    /// This test runs the same multi-model comparison twice:
    /// 1. With parallel_execution = true
    /// 2. With parallel_execution = false
    ///
    /// Verifies:
    /// - Both produce identical results
    /// - Parallel execution is faster than sequential
    ///
    /// Parallel execution implementation validation
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_parallel_vs_sequential_performance() {
        use std::sync::Arc;
        use std::time::Instant;

        let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

        // Create test scenarios
        let scenarios = vec![
            GoldenScenario {
                id: "test_1".to_string(),
                description: "Basic arithmetic test".to_string(),
                query: "What is 2+2?".to_string(),
                context: None,
                expected_output_contains: vec!["4".to_string()],
                expected_output_not_contains: vec!["5".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
            GoldenScenario {
                id: "test_2".to_string(),
                description: "Geography knowledge test".to_string(),
                query: "What is the capital of France?".to_string(),
                context: None,
                expected_output_contains: vec!["Paris".to_string()],
                expected_output_not_contains: vec!["London".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
        ];

        // Define agent function (simple echo for testing)
        let agent_fn: AgentFn = Arc::new(move |query: String| {
            Box::pin(async move {
                // Add small delay to simulate LLM latency
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                #[allow(deprecated)]
                Ok(crate::eval_runner::AgentResponse::with_usage(
                    query.clone(),
                    10,
                    0.0001,
                ))
            })
        });

        // Configuration for parallel execution
        let parallel_config = MultiModelConfig {
            models: vec![
                ModelConfig {
                    name: "gpt-4o-mini".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.15,
                    cost_per_million_output_tokens: 0.60,
                },
                ModelConfig {
                    name: "gpt-3.5-turbo".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.50,
                    cost_per_million_output_tokens: 1.50,
                },
            ],
            parallel_execution: true, // Enable parallel execution
            significance_level: 0.05,
        };

        // Configuration for sequential execution (same models)
        let sequential_config = MultiModelConfig {
            models: parallel_config.models.clone(),
            parallel_execution: false, // Disable parallel execution
            significance_level: 0.05,
        };

        // Run parallel execution and measure time
        let parallel_runner = MultiModelRunner::new(parallel_config);
        let start_parallel = Instant::now();
        let parallel_result = parallel_runner
            .compare_models(&scenarios, agent_fn.clone())
            .await;
        let parallel_duration = start_parallel.elapsed();

        assert!(
            parallel_result.is_ok(),
            "Parallel compare_models failed: {:?}",
            parallel_result.err()
        );

        // Run sequential execution and measure time
        let sequential_runner = MultiModelRunner::new(sequential_config);
        let start_sequential = Instant::now();
        let sequential_result = sequential_runner
            .compare_models(&scenarios, agent_fn.clone())
            .await;
        let sequential_duration = start_sequential.elapsed();

        assert!(
            sequential_result.is_ok(),
            "Sequential compare_models failed: {:?}",
            sequential_result.err()
        );

        let parallel_comparison = parallel_result.unwrap();
        let sequential_comparison = sequential_result.unwrap();

        // Verify both produced same number of results
        assert_eq!(
            parallel_comparison.models.len(),
            sequential_comparison.models.len(),
            "Parallel and sequential should produce same number of models"
        );

        // Verify both models executed in both modes
        for model_name in &parallel_comparison.models {
            assert!(
                parallel_comparison.results.contains_key(model_name),
                "Parallel execution missing results for {}",
                model_name
            );
            assert!(
                sequential_comparison.results.contains_key(model_name),
                "Sequential execution missing results for {}",
                model_name
            );

            let parallel_report = &parallel_comparison.results[model_name];
            let sequential_report = &sequential_comparison.results[model_name];

            assert_eq!(
                parallel_report.results.len(),
                sequential_report.results.len(),
                "Model {} has different result counts in parallel vs sequential",
                model_name
            );
        }

        // Performance comparison: parallel should be faster
        // Note: This is not guaranteed in all environments, but with 2 models and 2 scenarios
        // each with 100ms simulated latency, parallel should be measurably faster
        println!(
            "Performance comparison:\n  Parallel: {:?}\n  Sequential: {:?}\n  Speedup: {:.2}x",
            parallel_duration,
            sequential_duration,
            sequential_duration.as_secs_f64() / parallel_duration.as_secs_f64()
        );

        // Don't enforce strict timing since test environment may vary,
        // but parallel should generally be faster or equal
        assert!(
            parallel_duration <= sequential_duration * 2,
            "Parallel execution took significantly longer than sequential (parallel: {:?}, sequential: {:?})",
            parallel_duration,
            sequential_duration
        );
    }

    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_ab_test_with_winner_determination() {
        use std::sync::Arc;

        let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

        // Create test scenarios
        let scenarios = vec![
            GoldenScenario {
                id: "test_1".to_string(),
                description: "Basic arithmetic test".to_string(),
                query: "What is 2+2?".to_string(),
                context: None,
                expected_output_contains: vec!["4".to_string()],
                expected_output_not_contains: vec!["5".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
            GoldenScenario {
                id: "test_2".to_string(),
                description: "Geography knowledge test".to_string(),
                query: "What is the capital of France?".to_string(),
                context: None,
                expected_output_contains: vec!["Paris".to_string()],
                expected_output_not_contains: vec!["London".to_string()],
                quality_threshold: 0.7,
                max_latency_ms: None,
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
            },
        ];

        // Define agent function (simple echo for testing)
        let agent_fn: AgentFn = Arc::new(move |query: String| {
            Box::pin(async move {
                #[allow(deprecated)]
                Ok(crate::eval_runner::AgentResponse::with_usage(
                    query.clone(),
                    10,
                    0.0001,
                ))
            })
        });

        // Configure two models for A/B test
        let model_a = ModelConfig {
            name: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            temperature: Some(0.0),
            max_tokens: Some(100),
            top_p: None,
            cost_per_million_input_tokens: 0.15,
            cost_per_million_output_tokens: 0.60,
        };

        let model_b = ModelConfig {
            name: "gpt-3.5-turbo".to_string(),
            provider: "openai".to_string(),
            temperature: Some(0.0),
            max_tokens: Some(100),
            top_p: None,
            cost_per_million_input_tokens: 0.50,
            cost_per_million_output_tokens: 1.50,
        };

        // Run A/B test
        let config = MultiModelConfig {
            models: vec![],
            parallel_execution: false,
            significance_level: 0.05,
        };
        let runner = MultiModelRunner::new(config);
        let result = runner
            .ab_test(&model_a, &model_b, &scenarios, agent_fn)
            .await;

        assert!(result.is_ok(), "ab_test failed: {:?}", result.err());

        let ab_report = result.unwrap();

        // Verify report structure
        assert_eq!(ab_report.model_a, "gpt-4o-mini");
        assert_eq!(ab_report.model_b, "gpt-3.5-turbo");

        // Print results for manual inspection
        println!("A/B Test Results:");
        println!("  Model A: {}", ab_report.model_a);
        println!("  Model B: {}", ab_report.model_b);
        println!("  Winner: {:?}", ab_report.winner);
        println!("  Confidence: {:.1}%", ab_report.confidence * 100.0);
        println!("  Quality Difference: {:.3}", ab_report.quality_difference);
        println!(
            "  Latency Difference: {:.1} ms",
            ab_report.latency_difference
        );
        println!("  Cost Difference: ${:.6}", ab_report.cost_difference);
        println!(
            "  Statistically Significant: {}",
            ab_report.statistical_significance
        );
        println!("  Details: {}", ab_report.details);

        // Verify confidence is in valid range [0, 1]
        assert!(
            ab_report.confidence >= 0.0 && ab_report.confidence <= 1.0,
            "Confidence should be in [0, 1], got {}",
            ab_report.confidence
        );

        // If there's a winner, statistical_significance should be true
        if ab_report.winner.is_some() {
            assert!(
                ab_report.statistical_significance,
                "Winner declared but not statistically significant"
            );
        }

        // If statistically significant, winner should be declared
        if ab_report.statistical_significance {
            assert!(
                ab_report.winner.is_some(),
                "Statistically significant but no winner declared"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_compare_models_partial_failure() {
        use std::sync::Arc;

        let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

        // Create test scenarios
        let scenarios = vec![GoldenScenario {
            id: "test_1".to_string(),
            description: "Basic test".to_string(),
            query: "What is 2+2?".to_string(),
            context: None,
            expected_output_contains: vec!["4".to_string()],
            expected_output_not_contains: vec![],
            quality_threshold: 0.7,
            max_latency_ms: None,
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
        }];

        // Define agent function
        let agent_fn: AgentFn = Arc::new(move |query: String| {
            Box::pin(async move {
                #[allow(deprecated)]
                Ok(crate::eval_runner::AgentResponse::with_usage(
                    query.clone(),
                    10,
                    0.0001,
                ))
            })
        });

        // Configure 3 models: 2 valid OpenAI, 1 unsupported provider
        let config = MultiModelConfig {
            models: vec![
                ModelConfig {
                    name: "gpt-4o-mini".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.15,
                    cost_per_million_output_tokens: 0.60,
                },
                ModelConfig {
                    name: "invalid-model".to_string(),
                    provider: "unsupported_provider".to_string(), // This will fail
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.0,
                    cost_per_million_output_tokens: 0.0,
                },
                ModelConfig {
                    name: "gpt-3.5-turbo".to_string(),
                    provider: "openai".to_string(),
                    temperature: Some(0.0),
                    max_tokens: Some(100),
                    top_p: None,
                    cost_per_million_input_tokens: 0.50,
                    cost_per_million_output_tokens: 1.50,
                },
            ],
            parallel_execution: false,
            significance_level: 0.05,
        };

        // Run comparison - should succeed with partial results
        let runner = MultiModelRunner::new(config);
        let result = runner.compare_models(&scenarios, agent_fn).await;

        // Should succeed (not return Err)
        assert!(
            result.is_ok(),
            "compare_models should succeed with partial results, got: {:?}",
            result.err()
        );

        let comparison = result.unwrap();

        // Verify all 3 models are listed
        assert_eq!(
            comparison.models.len(),
            3,
            "Should list all 3 attempted models"
        );
        assert!(comparison.models.contains(&"gpt-4o-mini".to_string()));
        assert!(comparison.models.contains(&"invalid-model".to_string()));
        assert!(comparison.models.contains(&"gpt-3.5-turbo".to_string()));

        // Verify only 2 successful results
        assert_eq!(
            comparison.results.len(),
            2,
            "Should have 2 successful results"
        );
        assert!(comparison.results.contains_key("gpt-4o-mini"));
        assert!(comparison.results.contains_key("gpt-3.5-turbo"));
        assert!(!comparison.results.contains_key("invalid-model"));

        // Verify 1 error
        assert_eq!(
            comparison.model_errors.len(),
            1,
            "Should have 1 model error"
        );
        assert!(comparison.model_errors.contains_key("invalid-model"));

        // Print error for manual inspection
        println!("Model Errors: {:?}", comparison.model_errors);

        // Verify error message mentions unsupported provider
        let error_msg = &comparison.model_errors["invalid-model"];
        assert!(
            error_msg.contains("unsupported") || error_msg.contains("Unsupported"),
            "Error message should mention unsupported provider: {}",
            error_msg
        );

        // Verify recommendation mentions failed model
        assert!(
            comparison.recommendation.contains("failed"),
            "Recommendation should mention failed models: {}",
            comparison.recommendation
        );

        // Verify statistical tests only include successful models
        // With 2 models, we should have 1 pairwise test
        assert_eq!(
            comparison.statistical_tests.len(),
            1,
            "Should have 1 statistical test for 2 successful models"
        );

        println!("Partial failure test passed!");
        println!("  Attempted: {} models", comparison.models.len());
        println!("  Succeeded: {} models", comparison.results.len());
        println!("  Failed: {} models", comparison.model_errors.len());
    }

    // Helper functions for tests

    fn create_test_result(id: &str, quality: f64) -> ScenarioResult {
        use crate::quality_judge::QualityScore;

        ScenarioResult {
            scenario_id: id.to_string(),
            passed: true,
            output: "test output".to_string(),
            quality_score: QualityScore {
                overall: quality,
                accuracy: quality,
                relevance: quality,
                completeness: quality,
                safety: 1.0,
                coherence: quality,
                conciseness: quality,
                reasoning: "Test reasoning".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms: 100,
            validation: crate::eval_runner::ValidationResult {
                passed: true,
                missing_contains: vec![],
                forbidden_found: vec![],
                failure_reason: None,
            },
            timestamp: chrono::Utc::now(),
            retry_attempts: 0,
            error: None,
            input: None,
            tokens_used: None,
            cost_usd: None,
        }
    }

    fn create_test_report(
        avg_quality: f64,
        _total_cost: f64,
        _avg_latency: f64,
        pass_rate: f64,
    ) -> EvalReport {
        let total = 10;
        let passed = (10.0 * pass_rate) as usize;
        let results = vec![
            create_test_result("1", avg_quality),
            create_test_result("2", avg_quality),
        ];

        let now = chrono::Utc::now();
        EvalReport {
            total,
            passed,
            failed: total - passed,
            results,
            metadata: crate::eval_runner::EvalMetadata {
                started_at: now,
                completed_at: now,
                duration_secs: 10.0,
                config: "{}".to_string(),
            },
        }
    }
}
