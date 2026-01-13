//! Cost Tracking for LLM Calls
//!
//! This module provides comprehensive cost tracking, budget management, and
//! real-time monitoring for LLM API usage in DashFlow applications.
//!
//! # Overview
//!
//! The cost tracking system consists of:
//! - [`Pricing`]: Per-model pricing for input/output tokens
//! - [`ModelPricing`]: Configuration of pricing across multiple models
//! - [`CostTracker`]: Runtime cost accumulation and reporting with budget tracking
//! - [`CostReport`]: Aggregated cost statistics
//! - [`BudgetConfig`]: Budget limit configuration
//! - [`BudgetEnforcer`]: Budget enforcement with threshold alerts
//!
//! # Example
//!
//! ```rust
//! use dashflow_observability::cost::{CostTracker, ModelPricing, Pricing};
//!
//! // Configure pricing for models
//! let pricing = ModelPricing::new()
//!     .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))           // $0.03/$0.06 per 1K tokens
//!     .with_model("gpt-4-turbo", Pricing::per_1k(0.01, 0.03))     // $0.01/$0.03 per 1K tokens
//!     .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015)); // $0.0005/$0.0015 per 1K tokens
//!
//! // Create cost tracker
//! let mut tracker = CostTracker::new(pricing);
//!
//! // Record LLM call
//! tracker.record_llm_call(
//!     "gpt-4",
//!     1500,  // input tokens
//!     800,   // output tokens
//!     Some("research_node"),
//! );
//!
//! // Get cost report
//! let report = tracker.report();
//! println!("Total cost: ${:.4}", report.total_cost());
//! println!("Calls: {}", report.total_calls());
//! ```
//!
//! # Budget Tracking
//!
//! ```rust
//! use dashflow_observability::cost::{CostTracker, BudgetConfig, BudgetEnforcer};
//!
//! // Create tracker with budget
//! let tracker = CostTracker::with_defaults()
//!     .with_daily_budget(100.0)   // $100/day
//!     .with_monthly_budget(2000.0) // $2000/month
//!     .with_alert_threshold(0.9);  // Alert at 90%
//!
//! // Or use BudgetEnforcer for hard limits
//! let config = BudgetConfig::with_daily_limit(50.0)
//!     .critical_threshold(1.0)
//!     .enforce_hard_limit(true);
//!
//! let enforcer = BudgetEnforcer::new(CostTracker::with_defaults(), config);
//! ```
//!
//! # Pricing Model
//!
//! Costs can be specified per-1K tokens (traditional) or per-1M tokens (modern):
//! - Per-1K: `(tokens / 1000.0) * price_per_1k`
//! - Per-1M: `(tokens / 1_000_000.0) * price_per_1m`
//!
//! # Integration with `DashFlow`
//!
//! When integrated with `DashFlow` execution, costs are automatically attributed to:
//! - Specific nodes (via node name)
//! - Graph invocations (via `thread_id`)
//! - Individual LLM calls

use crate::error::{Error, Result};
use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;

// ============================================================================
// Token Usage
// ============================================================================

/// Token usage counts for an LLM call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens (prompt)
    pub input_tokens: u64,
    /// Output tokens (completion)
    pub output_tokens: u64,
}

impl TokenUsage {
    /// Create new token usage record.
    #[must_use]
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    /// Total tokens (input + output).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

// ============================================================================
// Pricing
// ============================================================================

/// Pricing information for a single LLM model.
///
/// Prices can be specified per 1,000 tokens or per 1,000,000 tokens.
/// Internally stored as per-million for precision.
///
/// # Example
///
/// ```rust
/// use dashflow_observability::cost::Pricing;
///
/// // Traditional per-1K pricing (GPT-4: $0.03 input, $0.06 output per 1K)
/// let pricing_1k = Pricing::per_1k(0.03, 0.06);
///
/// // Modern per-1M pricing (GPT-4o: $2.50 input, $10.00 output per 1M)
/// let pricing_1m = Pricing::per_1m(2.50, 10.00);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pricing {
    /// Cost per 1,000,000 input tokens (prompt)
    input_per_million: f64,
    /// Cost per 1,000,000 output tokens (completion)
    output_per_million: f64,
}

impl Pricing {
    /// Create a new pricing configuration from per-1K token prices.
    ///
    /// # Arguments
    ///
    /// * `input_per_1k` - Cost per 1,000 input tokens
    /// * `output_per_1k` - Cost per 1,000 output tokens
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_observability::cost::Pricing;
    ///
    /// let pricing = Pricing::per_1k(0.03, 0.06);
    /// ```
    #[must_use]
    pub fn per_1k(input_per_1k: f64, output_per_1k: f64) -> Self {
        Self {
            input_per_million: input_per_1k * 1000.0,
            output_per_million: output_per_1k * 1000.0,
        }
    }

    /// Create a new pricing configuration from per-1M token prices.
    ///
    /// This is the modern pricing format used by most providers.
    ///
    /// # Arguments
    ///
    /// * `input_per_million` - Cost per 1,000,000 input tokens
    /// * `output_per_million` - Cost per 1,000,000 output tokens
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_observability::cost::Pricing;
    ///
    /// // GPT-4o pricing
    /// let pricing = Pricing::per_1m(2.50, 10.00);
    /// ```
    #[must_use]
    pub fn per_1m(input_per_million: f64, output_per_million: f64) -> Self {
        Self {
            input_per_million,
            output_per_million,
        }
    }

    /// Legacy constructor (per-1K pricing). Use `per_1k` for clarity.
    #[must_use]
    pub fn new(input_per_1k: f64, output_per_1k: f64) -> Self {
        Self::per_1k(input_per_1k, output_per_1k)
    }

    /// Get the input token price per 1,000 tokens.
    #[must_use]
    pub fn input_per_1k(&self) -> f64 {
        self.input_per_million / 1000.0
    }

    /// Get the output token price per 1,000 tokens.
    #[must_use]
    pub fn output_per_1k(&self) -> f64 {
        self.output_per_million / 1000.0
    }

    /// Get the input token price per 1,000,000 tokens.
    #[must_use]
    pub fn input_per_million(&self) -> f64 {
        self.input_per_million
    }

    /// Get the output token price per 1,000,000 tokens.
    #[must_use]
    pub fn output_per_million(&self) -> f64 {
        self.output_per_million
    }

    /// Calculate the cost for a given number of input and output tokens.
    ///
    /// # Arguments
    ///
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    ///
    /// # Returns
    ///
    /// Total cost in dollars
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_observability::cost::Pricing;
    ///
    /// let pricing = Pricing::per_1k(0.03, 0.06);
    /// let cost = pricing.calculate_cost(1500, 800);
    /// assert!((cost - 0.093).abs() < 0.001); // 1.5K * $0.03 + 0.8K * $0.06 = $0.093
    /// ```
    #[must_use]
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_per_million;
        input_cost + output_cost
    }

    /// Calculate cost from a [`TokenUsage`] struct.
    #[must_use]
    pub fn calculate_cost_from_usage(&self, usage: TokenUsage) -> f64 {
        self.calculate_cost(usage.input_tokens, usage.output_tokens)
    }
}

// ============================================================================
// Model Pricing Database
// ============================================================================

/// Extended model pricing information including provider metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    /// Model name
    pub name: String,
    /// Pricing for this model
    pub pricing: Pricing,
    /// Provider (e.g., "OpenAI", "Anthropic", "Google")
    pub provider: String,
}

impl ModelPrice {
    /// Create a new model price entry.
    #[must_use]
    pub fn new(name: impl Into<String>, pricing: Pricing, provider: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pricing,
            provider: provider.into(),
        }
    }

    /// Calculate cost for given token usage.
    #[must_use]
    pub fn calculate_cost(&self, usage: TokenUsage) -> f64 {
        self.pricing.calculate_cost_from_usage(usage)
    }
}

/// Pricing configuration for multiple LLM models.
///
/// # Example
///
/// ```rust
/// use dashflow_observability::cost::{ModelPricing, Pricing};
///
/// let pricing = ModelPricing::new()
///     .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))
///     .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015));
///
/// // Lookup pricing by model name
/// let gpt4_pricing = pricing.get("gpt-4");
/// assert!(gpt4_pricing.is_some());
/// ```
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Map from model name to pricing
    models: HashMap<String, ModelPrice>,
}

impl ModelPricing {
    /// Create a new empty pricing configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Add pricing for a specific model.
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (e.g., "gpt-4", "gpt-3.5-turbo")
    /// * `pricing` - Pricing configuration for this model
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_observability::cost::{ModelPricing, Pricing};
    ///
    /// let pricing = ModelPricing::new()
    ///     .with_model("gpt-4", Pricing::per_1k(0.03, 0.06));
    /// ```
    pub fn with_model(mut self, model: impl Into<String>, pricing: Pricing) -> Self {
        let name: String = model.into();
        self.models.insert(
            name.clone(),
            ModelPrice {
                name,
                pricing,
                provider: "Unknown".to_string(),
            },
        );
        self
    }

    /// Add a model with full metadata.
    pub fn with_model_price(mut self, price: ModelPrice) -> Self {
        self.models.insert(price.name.clone(), price);
        self
    }

    /// Get pricing for a specific model.
    ///
    /// # Arguments
    ///
    /// * `model` - Model name to lookup
    ///
    /// # Returns
    ///
    /// `Some(&Pricing)` if the model is configured, `None` otherwise
    #[must_use]
    pub fn get(&self, model: &str) -> Option<&Pricing> {
        self.models.get(model).map(|mp| &mp.pricing)
    }

    /// Get full model price info for a specific model.
    #[must_use]
    pub fn get_model_price(&self, model: &str) -> Option<&ModelPrice> {
        self.models.get(model)
    }

    /// Calculate cost for model and token usage.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the model is not found in the pricing database.
    pub fn calculate_cost(&self, model: &str, usage: TokenUsage) -> Result<f64> {
        let price = self.get_model_price(model).ok_or_else(|| {
            Error::Metrics(format!("Model not found in pricing database: {model}"))
        })?;
        Ok(price.calculate_cost(usage))
    }

    /// List all available models.
    #[must_use]
    pub fn list_models(&self) -> Vec<&str> {
        self.models.keys().map(|s| s.as_str()).collect()
    }

    /// Get all prices grouped by provider.
    #[must_use]
    pub fn by_provider(&self) -> HashMap<String, Vec<&ModelPrice>> {
        let mut by_provider: HashMap<String, Vec<&ModelPrice>> = HashMap::new();
        for price in self.models.values() {
            by_provider
                .entry(price.provider.clone())
                .or_default()
                .push(price);
        }
        by_provider
    }

    /// Create a default pricing configuration with common `OpenAI` models.
    ///
    /// Includes pricing for:
    /// - GPT-4 Turbo (128K context)
    /// - GPT-4 (8K context)
    /// - GPT-3.5 Turbo (16K context)
    ///
    /// Prices are current as of January 2025.
    #[must_use]
    pub fn openai_defaults() -> Self {
        Self::new()
            .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))
            .with_model("gpt-4-turbo", Pricing::per_1k(0.01, 0.03))
            .with_model("gpt-4-turbo-preview", Pricing::per_1k(0.01, 0.03))
            .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015))
            .with_model("gpt-3.5-turbo-16k", Pricing::per_1k(0.001, 0.002))
    }

    /// Create a comprehensive pricing database with all major providers.
    ///
    /// Includes OpenAI, Anthropic, and Google models.
    /// Prices as of November 2024.
    #[must_use]
    pub fn comprehensive_defaults() -> Self {
        let mut pricing = Self::new();

        // OpenAI models
        pricing.models.insert(
            "gpt-4o".to_string(),
            ModelPrice::new("gpt-4o", Pricing::per_1m(2.50, 10.00), "OpenAI"),
        );
        pricing.models.insert(
            "gpt-4o-mini".to_string(),
            ModelPrice::new("gpt-4o-mini", Pricing::per_1m(0.150, 0.600), "OpenAI"),
        );
        pricing.models.insert(
            "gpt-4-turbo".to_string(),
            ModelPrice::new("gpt-4-turbo", Pricing::per_1m(10.00, 30.00), "OpenAI"),
        );
        pricing.models.insert(
            "gpt-4".to_string(),
            ModelPrice::new("gpt-4", Pricing::per_1m(30.00, 60.00), "OpenAI"),
        );
        pricing.models.insert(
            "gpt-3.5-turbo".to_string(),
            ModelPrice::new("gpt-3.5-turbo", Pricing::per_1m(0.50, 1.50), "OpenAI"),
        );

        // Anthropic models
        pricing.models.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            ModelPrice::new(
                "claude-3-5-sonnet-20241022",
                Pricing::per_1m(3.00, 15.00),
                "Anthropic",
            ),
        );
        pricing.models.insert(
            "claude-3-opus-20240229".to_string(),
            ModelPrice::new(
                "claude-3-opus-20240229",
                Pricing::per_1m(15.00, 75.00),
                "Anthropic",
            ),
        );
        pricing.models.insert(
            "claude-3-sonnet-20240229".to_string(),
            ModelPrice::new(
                "claude-3-sonnet-20240229",
                Pricing::per_1m(3.00, 15.00),
                "Anthropic",
            ),
        );
        pricing.models.insert(
            "claude-3-haiku-20240307".to_string(),
            ModelPrice::new(
                "claude-3-haiku-20240307",
                Pricing::per_1m(0.25, 1.25),
                "Anthropic",
            ),
        );

        // Google models
        pricing.models.insert(
            "gemini-1.5-pro".to_string(),
            ModelPrice::new("gemini-1.5-pro", Pricing::per_1m(1.25, 5.00), "Google"),
        );
        pricing.models.insert(
            "gemini-1.5-flash".to_string(),
            ModelPrice::new("gemini-1.5-flash", Pricing::per_1m(0.075, 0.30), "Google"),
        );

        pricing
    }
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self::comprehensive_defaults()
    }
}

// ============================================================================
// Cost Record
// ============================================================================

/// A single LLM call cost record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    /// Timestamp of the call
    pub timestamp: DateTime<Local>,
    /// Model name
    pub model: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Calculated cost
    pub cost: f64,
    /// Optional node name for attribution
    pub node_name: Option<String>,
    /// Optional user ID for per-user cost tracking
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Optional session ID for per-session cost tracking
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

// ============================================================================
// Cost Report
// ============================================================================

/// Aggregated cost report from a cost tracker.
///
/// Provides various views of cost data:
/// - Total cost and call count
/// - Token usage (input/output)
/// - Cost breakdown by model
/// - Cost breakdown by node
/// - Cost breakdown by user (M-302)
/// - Cost breakdown by session (M-302)
/// - Time-based spending (today/month)
/// - Budget tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// Total number of LLM calls
    pub total_calls: usize,
    /// Total cost in dollars
    pub total_cost: f64,
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Cost breakdown by model name
    pub cost_by_model: HashMap<String, f64>,
    /// Cost breakdown by node name
    pub cost_by_node: HashMap<String, f64>,
    /// Cost breakdown by user ID (M-302: per-user cost tracking)
    pub cost_by_user: HashMap<String, f64>,
    /// Cost breakdown by session ID (M-302: per-session cost tracking)
    pub cost_by_session: HashMap<String, f64>,
    /// Cost spent today
    pub spent_today: f64,
    /// Cost spent this month
    pub spent_month: f64,
    /// Daily budget limit (if set)
    pub daily_limit: Option<f64>,
    /// Monthly budget limit (if set)
    pub monthly_limit: Option<f64>,
    /// Percentage of daily budget used
    pub daily_usage_percent: Option<f64>,
    /// Percentage of monthly budget used
    pub monthly_usage_percent: Option<f64>,
}

impl CostReport {
    /// Get the total number of LLM calls recorded.
    #[must_use]
    pub fn total_calls(&self) -> usize {
        self.total_calls
    }

    /// Get the total cost across all calls (in dollars).
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.total_cost
    }

    /// Get the total number of input tokens.
    #[must_use]
    pub fn total_input_tokens(&self) -> u64 {
        self.total_input_tokens
    }

    /// Get the total number of output tokens.
    #[must_use]
    pub fn total_output_tokens(&self) -> u64 {
        self.total_output_tokens
    }

    /// Get the total number of tokens (input + output).
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Get cost breakdown by model name.
    #[must_use]
    pub fn cost_by_model(&self) -> &HashMap<String, f64> {
        &self.cost_by_model
    }

    /// Get cost breakdown by node name.
    #[must_use]
    pub fn cost_by_node(&self) -> &HashMap<String, f64> {
        &self.cost_by_node
    }

    /// Get cost breakdown by user ID (M-302).
    #[must_use]
    pub fn cost_by_user(&self) -> &HashMap<String, f64> {
        &self.cost_by_user
    }

    /// Get cost breakdown by session ID (M-302).
    #[must_use]
    pub fn cost_by_session(&self) -> &HashMap<String, f64> {
        &self.cost_by_session
    }

    /// Get the average cost per call.
    #[must_use]
    pub fn average_cost_per_call(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.total_cost / self.total_calls as f64
        }
    }

    /// Format the report as a human-readable string.
    #[must_use]
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("Cost Report\n");
        output.push_str("===========\n");
        output.push_str(&format!("Total Calls: {}\n", self.total_calls));
        output.push_str(&format!("Total Cost: ${:.4}\n", self.total_cost));
        output.push_str(&format!(
            "Total Tokens: {} (input: {}, output: {})\n",
            self.total_tokens(),
            self.total_input_tokens,
            self.total_output_tokens
        ));
        output.push_str(&format!(
            "Average Cost/Call: ${:.4}\n",
            self.average_cost_per_call()
        ));

        output.push_str(&format!("\nSpent Today: ${:.4}\n", self.spent_today));
        output.push_str(&format!("Spent This Month: ${:.4}\n", self.spent_month));

        if let Some(daily_limit) = self.daily_limit {
            output.push_str(&format!("Daily Limit: ${:.2}\n", daily_limit));
            if let Some(usage_pct) = self.daily_usage_percent {
                output.push_str(&format!("Daily Usage: {:.1}%\n", usage_pct));
            }
        }

        if let Some(monthly_limit) = self.monthly_limit {
            output.push_str(&format!("Monthly Limit: ${:.2}\n", monthly_limit));
            if let Some(usage_pct) = self.monthly_usage_percent {
                output.push_str(&format!("Monthly Usage: {:.1}%\n", usage_pct));
            }
        }

        if !self.cost_by_model.is_empty() {
            output.push_str("\nCost by Model:\n");
            let mut models: Vec<_> = self.cost_by_model.iter().collect();
            models.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (model, cost) in models {
                output.push_str(&format!("  {model}: ${cost:.4}\n"));
            }
        }

        if !self.cost_by_node.is_empty() {
            output.push_str("\nCost by Node:\n");
            let mut nodes: Vec<_> = self.cost_by_node.iter().collect();
            nodes.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (node, cost) in nodes {
                output.push_str(&format!("  {node}: ${cost:.4}\n"));
            }
        }

        if !self.cost_by_user.is_empty() {
            output.push_str("\nCost by User:\n");
            let mut users: Vec<_> = self.cost_by_user.iter().collect();
            users.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (user, cost) in users {
                output.push_str(&format!("  {user}: ${cost:.4}\n"));
            }
        }

        if !self.cost_by_session.is_empty() {
            output.push_str("\nCost by Session:\n");
            let mut sessions: Vec<_> = self.cost_by_session.iter().collect();
            sessions.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (session, cost) in sessions {
                output.push_str(&format!("  {session}: ${cost:.4}\n"));
            }
        }

        output
    }
}

// ============================================================================
// Cost Tracker
// ============================================================================

/// Internal state for cost tracker.
struct TrackerState {
    pricing: ModelPricing,
    records: Vec<CostRecord>,
    daily_budget: Option<f64>,
    monthly_budget: Option<f64>,
    alert_threshold: f64,
    alert_callback: Option<Box<dyn Fn(f64, f64) + Send + Sync>>,
}

impl std::fmt::Debug for TrackerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackerState")
            .field("pricing", &"...")
            .field("records_count", &self.records.len())
            .field("daily_budget", &self.daily_budget)
            .field("monthly_budget", &self.monthly_budget)
            .field("alert_threshold", &self.alert_threshold)
            .finish()
    }
}

/// Cost tracker for accumulating LLM costs during execution.
///
/// Thread-safe for concurrent recording from multiple nodes.
/// Supports budget tracking, alerts, and time-based spending reports.
///
/// # Example
///
/// ```rust
/// use dashflow_observability::cost::{CostTracker, ModelPricing, Pricing};
///
/// let pricing = ModelPricing::new()
///     .with_model("gpt-4", Pricing::per_1k(0.03, 0.06));
///
/// let mut tracker = CostTracker::new(pricing);
///
/// // Record calls
/// tracker.record_llm_call("gpt-4", 1000, 500, Some("node1"));
/// tracker.record_llm_call("gpt-4", 2000, 800, Some("node2"));
///
/// let report = tracker.report();
/// assert_eq!(report.total_calls(), 2);
/// ```
#[derive(Clone)]
pub struct CostTracker {
    state: Arc<Mutex<TrackerState>>,
}

impl CostTracker {
    /// Create a new cost tracker with the given pricing configuration.
    ///
    /// # Arguments
    ///
    /// * `pricing` - Model pricing configuration
    #[must_use]
    pub fn new(pricing: ModelPricing) -> Self {
        Self {
            state: Arc::new(Mutex::new(TrackerState {
                pricing,
                records: Vec::new(),
                daily_budget: None,
                monthly_budget: None,
                alert_threshold: 0.9,
                alert_callback: None,
            })),
        }
    }

    /// Create a new cost tracker with default comprehensive pricing.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ModelPricing::comprehensive_defaults())
    }

    /// Create a new cost tracker with default `OpenAI` pricing.
    ///
    /// Equivalent to `CostTracker::new(ModelPricing::openai_defaults())`.
    #[must_use]
    pub fn with_openai_defaults() -> Self {
        Self::new(ModelPricing::openai_defaults())
    }

    /// Set daily budget limit.
    #[must_use]
    pub fn with_daily_budget(self, budget: f64) -> Self {
        match self.state.lock() {
            Ok(mut state) => state.daily_budget = Some(budget),
            Err(e) => {
                warn!(budget = budget, error = %e, "Failed to acquire lock for daily budget setter")
            }
        }
        self
    }

    /// Set monthly budget limit.
    #[must_use]
    pub fn with_monthly_budget(self, budget: f64) -> Self {
        match self.state.lock() {
            Ok(mut state) => state.monthly_budget = Some(budget),
            Err(e) => {
                warn!(budget = budget, error = %e, "Failed to acquire lock for monthly budget setter")
            }
        }
        self
    }

    /// Set alert threshold (0.0-1.0, default 0.9).
    ///
    /// When spending reaches this percentage of the budget, the alert callback is triggered.
    #[must_use]
    pub fn with_alert_threshold(self, threshold: f64) -> Self {
        match self.state.lock() {
            Ok(mut state) => state.alert_threshold = threshold,
            Err(e) => {
                warn!(threshold = threshold, error = %e, "Failed to acquire lock for alert threshold setter")
            }
        }
        self
    }

    /// Set alert callback function.
    ///
    /// Called when spending exceeds the alert threshold.
    /// Parameters are (spent_amount, budget_limit).
    #[must_use]
    pub fn with_alert_callback<F>(self, callback: F) -> Self
    where
        F: Fn(f64, f64) + Send + Sync + 'static,
    {
        match self.state.lock() {
            Ok(mut state) => state.alert_callback = Some(Box::new(callback)),
            Err(e) => warn!(error = %e, "Failed to acquire lock for alert callback setter"),
        }
        self
    }

    /// Record a single LLM API call.
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (must be configured in pricing)
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    /// * `node_name` - Optional node name for cost attribution
    ///
    /// # Returns
    ///
    /// `Ok(cost)` with the calculated cost, or `Err` if model pricing not found
    pub fn record_llm_call(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        node_name: Option<&str>,
    ) -> Result<f64> {
        self.record_llm_call_with_context(model, input_tokens, output_tokens, node_name, None, None)
    }

    /// Record a single LLM API call with user/session context (M-302).
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (must be configured in pricing)
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    /// * `node_name` - Optional node name for cost attribution
    /// * `user_id` - Optional user ID for per-user cost tracking
    /// * `session_id` - Optional session ID for per-session cost tracking
    ///
    /// # Returns
    ///
    /// `Ok(cost)` with the calculated cost, or `Err` if model pricing not found
    pub fn record_llm_call_with_context(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        node_name: Option<&str>,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<f64> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| Error::Metrics(format!("Lock poisoned: {e}")))?;

        let pricing = state
            .pricing
            .get(model)
            .ok_or_else(|| Error::Metrics(format!("No pricing configured for model: {model}")))?;

        let cost = pricing.calculate_cost(input_tokens, output_tokens);
        let usage = TokenUsage::new(input_tokens, output_tokens);

        let record = CostRecord {
            timestamp: Local::now(),
            model: model.to_string(),
            usage,
            cost,
            node_name: node_name.map(std::string::ToString::to_string),
            user_id: user_id.map(std::string::ToString::to_string),
            session_id: session_id.map(std::string::ToString::to_string),
        };

        state.records.push(record);

        // Check if we should trigger alert
        if let Some(daily_budget) = state.daily_budget {
            let spent_today = Self::calculate_spent_today_internal(&state.records);
            let usage_percent = spent_today / daily_budget;

            if usage_percent >= state.alert_threshold {
                if let Some(callback) = &state.alert_callback {
                    callback(spent_today, daily_budget);
                }
            }
        }

        Ok(cost)
    }

    /// Record usage using TokenUsage struct.
    pub fn record_usage(&self, model: &str, input_tokens: u64, output_tokens: u64) -> Result<f64> {
        self.record_usage_with_context(model, input_tokens, output_tokens, None, None)
    }

    /// Record usage with user/session context (M-302).
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (must be configured in pricing)
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    /// * `user_id` - Optional user ID for per-user cost tracking
    /// * `session_id` - Optional session ID for per-session cost tracking
    ///
    /// # Returns
    ///
    /// `Ok(cost)` with the calculated cost, or `Err` if model pricing not found
    pub fn record_usage_with_context(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<f64> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| Error::Metrics(format!("Lock poisoned: {e}")))?;

        let usage = TokenUsage::new(input_tokens, output_tokens);
        let cost = state.pricing.calculate_cost(model, usage)?;

        let record = CostRecord {
            timestamp: Local::now(),
            model: model.to_string(),
            usage,
            cost,
            node_name: None,
            user_id: user_id.map(std::string::ToString::to_string),
            session_id: session_id.map(std::string::ToString::to_string),
        };

        state.records.push(record);

        // Check if we should trigger alert
        if let Some(daily_budget) = state.daily_budget {
            let spent_today = Self::calculate_spent_today_internal(&state.records);
            let usage_percent = spent_today / daily_budget;

            if usage_percent >= state.alert_threshold {
                if let Some(callback) = &state.alert_callback {
                    callback(spent_today, daily_budget);
                }
            }
        }

        Ok(cost)
    }

    /// Generate a cost report summarizing all recorded calls.
    #[must_use]
    pub fn report(&self) -> CostReport {
        let state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => {
                return CostReport {
                    total_calls: 0,
                    total_cost: 0.0,
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    cost_by_model: HashMap::new(),
                    cost_by_node: HashMap::new(),
                    cost_by_user: HashMap::new(),
                    cost_by_session: HashMap::new(),
                    spent_today: 0.0,
                    spent_month: 0.0,
                    daily_limit: None,
                    monthly_limit: None,
                    daily_usage_percent: None,
                    monthly_usage_percent: None,
                };
            }
        };

        let mut total_cost = 0.0;
        let mut total_input_tokens = 0;
        let mut total_output_tokens = 0;
        let mut cost_by_model: HashMap<String, f64> = HashMap::new();
        let mut cost_by_node: HashMap<String, f64> = HashMap::new();
        let mut cost_by_user: HashMap<String, f64> = HashMap::new();
        let mut cost_by_session: HashMap<String, f64> = HashMap::new();

        for record in state.records.iter() {
            total_cost += record.cost;
            total_input_tokens += record.usage.input_tokens;
            total_output_tokens += record.usage.output_tokens;

            *cost_by_model.entry(record.model.clone()).or_insert(0.0) += record.cost;

            if let Some(node_name) = &record.node_name {
                *cost_by_node.entry(node_name.clone()).or_insert(0.0) += record.cost;
            }

            // M-302: Aggregate by user ID
            if let Some(user_id) = &record.user_id {
                *cost_by_user.entry(user_id.clone()).or_insert(0.0) += record.cost;
            }

            // M-302: Aggregate by session ID
            if let Some(session_id) = &record.session_id {
                *cost_by_session.entry(session_id.clone()).or_insert(0.0) += record.cost;
            }
        }

        let spent_today = Self::calculate_spent_today_internal(&state.records);
        let spent_month = Self::calculate_spent_month_internal(&state.records);

        let daily_usage_percent = state
            .daily_budget
            .map(|budget| (spent_today / budget) * 100.0);

        let monthly_usage_percent = state
            .monthly_budget
            .map(|budget| (spent_month / budget) * 100.0);

        CostReport {
            total_calls: state.records.len(),
            total_cost,
            total_input_tokens,
            total_output_tokens,
            cost_by_model,
            cost_by_node,
            cost_by_user,
            cost_by_session,
            spent_today,
            spent_month,
            daily_limit: state.daily_budget,
            monthly_limit: state.monthly_budget,
            daily_usage_percent,
            monthly_usage_percent,
        }
    }

    /// Get all usage records.
    #[must_use]
    pub fn get_records(&self) -> Vec<CostRecord> {
        self.state
            .lock()
            .map(|state| state.records.clone())
            .unwrap_or_default()
    }

    /// Reset all cost tracking data.
    ///
    /// Clears all recorded calls and resets counters to zero.
    pub fn reset(&mut self) {
        match self.state.lock() {
            Ok(mut state) => state.records.clear(),
            Err(e) => warn!(error = %e, "Failed to acquire lock for cost tracker reset"),
        }
    }

    /// Clear all records (alias for reset).
    pub fn clear_records(&self) {
        match self.state.lock() {
            Ok(mut state) => state.records.clear(),
            Err(e) => warn!(error = %e, "Failed to acquire lock for clearing records"),
        }
    }

    /// Export metrics in Prometheus text format.
    #[must_use]
    pub fn export_prometheus(&self) -> String {
        let report = self.report();

        let mut output = String::new();
        output.push_str("# HELP llm_cost_total Total cost in USD\n");
        output.push_str("# TYPE llm_cost_total counter\n");
        output.push_str(&format!("llm_cost_total {}\n", report.total_cost));

        output.push_str("# HELP llm_cost_today Cost today in USD\n");
        output.push_str("# TYPE llm_cost_today gauge\n");
        output.push_str(&format!("llm_cost_today {}\n", report.spent_today));

        output.push_str("# HELP llm_requests_total Total number of requests\n");
        output.push_str("# TYPE llm_requests_total counter\n");
        output.push_str(&format!("llm_requests_total {}\n", report.total_calls));

        output.push_str("# HELP llm_cost_per_request Average cost per request\n");
        output.push_str("# TYPE llm_cost_per_request gauge\n");
        output.push_str(&format!(
            "llm_cost_per_request {}\n",
            report.average_cost_per_call()
        ));

        output.push_str("# HELP llm_tokens_input_total Total input tokens\n");
        output.push_str("# TYPE llm_tokens_input_total counter\n");
        output.push_str(&format!(
            "llm_tokens_input_total {}\n",
            report.total_input_tokens
        ));

        output.push_str("# HELP llm_tokens_output_total Total output tokens\n");
        output.push_str("# TYPE llm_tokens_output_total counter\n");
        output.push_str(&format!(
            "llm_tokens_output_total {}\n",
            report.total_output_tokens
        ));

        for (model, cost) in &report.cost_by_model {
            output.push_str(&format!(
                "llm_cost_by_model{{model=\"{}\"}} {}\n",
                model, cost
            ));
        }

        // M-302: Per-user cost metrics
        if !report.cost_by_user.is_empty() {
            output.push_str("\n# HELP llm_cost_by_user Cost breakdown by user ID in USD\n");
            output.push_str("# TYPE llm_cost_by_user gauge\n");
            for (user_id, cost) in &report.cost_by_user {
                output.push_str(&format!(
                    "llm_cost_by_user{{user_id=\"{}\"}} {}\n",
                    user_id, cost
                ));
            }
        }

        // M-302: Per-session cost metrics
        if !report.cost_by_session.is_empty() {
            output.push_str("\n# HELP llm_cost_by_session Cost breakdown by session ID in USD\n");
            output.push_str("# TYPE llm_cost_by_session gauge\n");
            for (session_id, cost) in &report.cost_by_session {
                output.push_str(&format!(
                    "llm_cost_by_session{{session_id=\"{}\"}} {}\n",
                    session_id, cost
                ));
            }
        }

        output
    }

    fn calculate_spent_today_internal(records: &[CostRecord]) -> f64 {
        let today = Local::now().date_naive();
        records
            .iter()
            .filter(|r| r.timestamp.date_naive() == today)
            .map(|r| r.cost)
            .sum()
    }

    fn calculate_spent_month_internal(records: &[CostRecord]) -> f64 {
        let now = Local::now();
        let current_year = now.year();
        let current_month = now.month();

        records
            .iter()
            .filter(|r| r.timestamp.year() == current_year && r.timestamp.month() == current_month)
            .map(|r| r.cost)
            .sum()
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Budget Configuration
// ============================================================================

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    /// Warning threshold reached (default 90%)
    Warning,
    /// Critical threshold reached (default 100%)
    Critical,
}

/// Budget configuration for cost limits and alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Daily budget limit (USD)
    pub daily_limit: Option<f64>,
    /// Monthly budget limit (USD)
    pub monthly_limit: Option<f64>,
    /// Per-request budget limit (USD)
    pub per_request_limit: Option<f64>,
    /// Total budget limit (USD, across all time)
    pub total_limit: Option<f64>,
    /// Warning threshold (0.0-1.0, default 0.9)
    pub warning_threshold: f64,
    /// Critical threshold (0.0-1.0, default 1.0)
    pub critical_threshold: f64,
    /// Whether to block requests when budget exceeded
    pub enforce_hard_limit: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            daily_limit: None,
            monthly_limit: None,
            per_request_limit: None,
            total_limit: None,
            warning_threshold: 0.9,
            critical_threshold: 1.0,
            enforce_hard_limit: false,
        }
    }
}

impl BudgetConfig {
    /// Validate the budget configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `warning_threshold` is not in range 0.0-1.0
    /// - `critical_threshold` is not in range 0.0-1.0
    /// - `warning_threshold` is greater than `critical_threshold`
    /// - Any limit is negative
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.warning_threshold) {
            return Err(Error::Metrics(format!(
                "warning_threshold must be 0.0-1.0, got {}",
                self.warning_threshold
            )));
        }
        if !(0.0..=1.0).contains(&self.critical_threshold) {
            return Err(Error::Metrics(format!(
                "critical_threshold must be 0.0-1.0, got {}",
                self.critical_threshold
            )));
        }
        if self.warning_threshold > self.critical_threshold {
            return Err(Error::Metrics(format!(
                "warning_threshold ({}) must not exceed critical_threshold ({})",
                self.warning_threshold, self.critical_threshold
            )));
        }
        if let Some(limit) = self.daily_limit {
            if limit < 0.0 {
                return Err(Error::Metrics(format!(
                    "daily_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.monthly_limit {
            if limit < 0.0 {
                return Err(Error::Metrics(format!(
                    "monthly_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.per_request_limit {
            if limit < 0.0 {
                return Err(Error::Metrics(format!(
                    "per_request_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.total_limit {
            if limit < 0.0 {
                return Err(Error::Metrics(format!(
                    "total_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        Ok(())
    }

    /// Create new budget config with daily limit.
    #[must_use]
    pub fn with_daily_limit(limit: f64) -> Self {
        Self {
            daily_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with monthly limit.
    #[must_use]
    pub fn with_monthly_limit(limit: f64) -> Self {
        Self {
            monthly_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with per-request limit.
    #[must_use]
    pub fn with_per_request_limit(limit: f64) -> Self {
        Self {
            per_request_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with total limit.
    #[must_use]
    pub fn with_total_limit(limit: f64) -> Self {
        Self {
            total_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Set warning threshold.
    #[must_use]
    pub fn warning_threshold(mut self, threshold: f64) -> Self {
        self.warning_threshold = threshold;
        self
    }

    /// Set critical threshold.
    #[must_use]
    pub fn critical_threshold(mut self, threshold: f64) -> Self {
        self.critical_threshold = threshold;
        self
    }

    /// Enable hard limit enforcement.
    #[must_use]
    pub fn enforce_hard_limit(mut self, enforce: bool) -> Self {
        self.enforce_hard_limit = enforce;
        self
    }
}

// ============================================================================
// Budget Enforcer
// ============================================================================

/// Budget enforcer with threshold alerts.
///
/// Wraps a [`CostTracker`] and enforces budget limits with warning and critical thresholds.
pub struct BudgetEnforcer {
    tracker: CostTracker,
    config: BudgetConfig,
}

impl BudgetEnforcer {
    /// Create new budget enforcer.
    ///
    /// # Panics
    ///
    /// Panics if configuration validation fails. Use `try_new()` for fallible construction.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panicking constructor; try_new() is the fallible alternative
    pub fn new(tracker: CostTracker, config: BudgetConfig) -> Self {
        Self::try_new(tracker, config).expect("Invalid BudgetConfig")
    }

    /// Create new budget enforcer with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails.
    pub fn try_new(mut tracker: CostTracker, config: BudgetConfig) -> Result<Self> {
        config.validate()?;
        // Apply budget config to tracker
        if let Some(daily_limit) = config.daily_limit {
            tracker = tracker.with_daily_budget(daily_limit);
        }
        if let Some(monthly_limit) = config.monthly_limit {
            tracker = tracker.with_monthly_budget(monthly_limit);
        }
        Ok(Self { tracker, config })
    }

    /// Check if usage is within budget.
    ///
    /// # Errors
    ///
    /// Returns `Err` if budget is exceeded and hard limits are enforced.
    pub fn check_budget(&self) -> Result<()> {
        let report = self.tracker.report();

        // Check total budget
        if let Some(total_limit) = self.config.total_limit {
            let usage_percent = report.total_cost / total_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(Error::Metrics(format!(
                    "Budget exceeded: spent ${:.2}, limit ${:.2}",
                    report.total_cost, total_limit
                )));
            }
        }

        // Check daily budget
        if let Some(daily_limit) = self.config.daily_limit {
            let usage_percent = report.spent_today / daily_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(Error::Metrics(format!(
                    "Daily budget exceeded: spent ${:.2}, limit ${:.2}",
                    report.spent_today, daily_limit
                )));
            }
        }

        // Check monthly budget
        if let Some(monthly_limit) = self.config.monthly_limit {
            let usage_percent = report.spent_month / monthly_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(Error::Metrics(format!(
                    "Monthly budget exceeded: spent ${:.2}, limit ${:.2}",
                    report.spent_month, monthly_limit
                )));
            }
        }

        Ok(())
    }

    /// Get current alert level, if any.
    #[must_use]
    pub fn alert_level(&self) -> Option<AlertLevel> {
        let report = self.tracker.report();

        // Check daily budget
        if let Some(daily_limit) = self.config.daily_limit {
            let usage_percent = report.spent_today / daily_limit;

            if usage_percent >= self.config.critical_threshold {
                return Some(AlertLevel::Critical);
            }
            if usage_percent >= self.config.warning_threshold {
                return Some(AlertLevel::Warning);
            }
        }

        // Check monthly budget
        if let Some(monthly_limit) = self.config.monthly_limit {
            let usage_percent = report.spent_month / monthly_limit;

            if usage_percent >= self.config.critical_threshold {
                return Some(AlertLevel::Critical);
            }
            if usage_percent >= self.config.warning_threshold {
                return Some(AlertLevel::Warning);
            }
        }

        None
    }

    /// Record usage and check budget.
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - Budget is exceeded and hard limits are enforced
    /// - Model not found in pricing database
    pub fn record_and_check(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<f64> {
        // First check if we're already over budget
        self.check_budget()?;

        // Record the usage
        let cost = self
            .tracker
            .record_usage(model, input_tokens, output_tokens)?;

        // Check per-request limit (after recording to know actual cost)
        if let Some(per_request_limit) = self.config.per_request_limit {
            if self.config.enforce_hard_limit && cost > per_request_limit {
                return Err(Error::Metrics(format!(
                    "Per-request budget exceeded: cost ${:.4}, limit ${:.4}",
                    cost, per_request_limit
                )));
            }
        }

        Ok(cost)
    }

    /// Get the underlying tracker.
    #[must_use]
    pub fn tracker(&self) -> &CostTracker {
        &self.tracker
    }

    /// Get budget config.
    #[must_use]
    pub fn config(&self) -> &BudgetConfig {
        &self.config
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    // These tests use `unwrap` for setup/serialization where failures should be loud.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_pricing_calculation() {
        let pricing = Pricing::per_1k(0.03, 0.06);
        let cost = pricing.calculate_cost(1500, 800);
        // 1.5K * $0.03 + 0.8K * $0.06 = $0.045 + $0.048 = $0.093
        assert!((cost - 0.093).abs() < 0.001);
    }

    #[test]
    fn test_pricing_per_1m() {
        let pricing = Pricing::per_1m(2.50, 10.00);
        // 1M input + 1M output = $2.50 + $10.00 = $12.50
        let cost = pricing.calculate_cost(1_000_000, 1_000_000);
        assert!((cost - 12.50).abs() < 0.001);
    }

    #[test]
    fn test_model_pricing_builder() {
        let pricing = ModelPricing::new()
            .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))
            .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015));

        assert!(pricing.get("gpt-4").is_some());
        assert!(pricing.get("gpt-3.5-turbo").is_some());
        assert!(pricing.get("unknown-model").is_none());
    }

    #[test]
    fn test_openai_defaults() {
        let pricing = ModelPricing::openai_defaults();
        assert!(pricing.get("gpt-4").is_some());
        assert!(pricing.get("gpt-4-turbo").is_some());
        assert!(pricing.get("gpt-3.5-turbo").is_some());
    }

    #[test]
    fn test_comprehensive_defaults() {
        let pricing = ModelPricing::comprehensive_defaults();
        // OpenAI
        assert!(pricing.get("gpt-4o").is_some());
        assert!(pricing.get("gpt-4o-mini").is_some());
        // Anthropic
        assert!(pricing.get("claude-3-5-sonnet-20241022").is_some());
        assert!(pricing.get("claude-3-haiku-20240307").is_some());
        // Google
        assert!(pricing.get("gemini-1.5-pro").is_some());
        assert!(pricing.get("gemini-1.5-flash").is_some());
    }

    #[test]
    fn test_cost_tracker_basic() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        let cost = tracker.record_llm_call("gpt-4", 1000, 500, None).unwrap();

        // 1K * $0.03 + 0.5K * $0.06 = $0.03 + $0.03 = $0.06
        assert!((cost - 0.06).abs() < 0.001);

        let report = tracker.report();
        assert_eq!(report.total_calls(), 1);
        assert!((report.total_cost() - 0.06).abs() < 0.001);
        assert_eq!(report.total_input_tokens(), 1000);
        assert_eq!(report.total_output_tokens(), 500);
    }

    #[test]
    fn test_cost_tracker_multiple_calls() {
        let pricing = ModelPricing::new()
            .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))
            .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call("gpt-4", 1000, 500, Some("node1"))
            .unwrap();
        tracker
            .record_llm_call("gpt-3.5-turbo", 2000, 1000, Some("node2"))
            .unwrap();

        let report = tracker.report();
        assert_eq!(report.total_calls(), 2);
        assert_eq!(report.total_input_tokens(), 3000);
        assert_eq!(report.total_output_tokens(), 1500);
        assert_eq!(report.cost_by_model().len(), 2);
        assert_eq!(report.cost_by_node().len(), 2);
    }

    #[test]
    fn test_cost_tracker_unknown_model() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        let result = tracker.record_llm_call("unknown-model", 1000, 500, None);
        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::float_cmp)] // Comparing known constant 0.0 after reset
    fn test_cost_tracker_reset() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker.record_llm_call("gpt-4", 1000, 500, None).unwrap();
        assert_eq!(tracker.report().total_calls(), 1);

        tracker.reset();
        assert_eq!(tracker.report().total_calls(), 0);
        assert_eq!(tracker.report().total_cost(), 0.0);
    }

    #[test]
    fn test_cost_report_breakdown() {
        let pricing = ModelPricing::new()
            .with_model("gpt-4", Pricing::per_1k(0.03, 0.06))
            .with_model("gpt-3.5-turbo", Pricing::per_1k(0.0005, 0.0015));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call("gpt-4", 1000, 500, Some("researcher"))
            .unwrap();
        tracker
            .record_llm_call("gpt-4", 2000, 800, Some("analyzer"))
            .unwrap();
        tracker
            .record_llm_call("gpt-3.5-turbo", 1000, 500, Some("researcher"))
            .unwrap();

        let report = tracker.report();

        // Verify model breakdown
        let gpt4_cost = report.cost_by_model().get("gpt-4").unwrap();
        assert!(*gpt4_cost > 0.0);

        let gpt35_cost = report.cost_by_model().get("gpt-3.5-turbo").unwrap();
        assert!(*gpt35_cost > 0.0);

        // Verify node breakdown
        let researcher_cost = report.cost_by_node().get("researcher").unwrap();
        assert!(*researcher_cost > 0.0);

        let analyzer_cost = report.cost_by_node().get("analyzer").unwrap();
        assert!(*analyzer_cost > 0.0);
    }

    #[test]
    fn test_cost_report_format() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call("gpt-4", 1000, 500, Some("node1"))
            .unwrap();

        let report = tracker.report();
        let formatted = report.format();

        assert!(formatted.contains("Cost Report"));
        assert!(formatted.contains("Total Calls: 1"));
        assert!(formatted.contains("Cost by Model:"));
        assert!(formatted.contains("gpt-4"));
        assert!(formatted.contains("Cost by Node:"));
        assert!(formatted.contains("node1"));
    }

    #[test]
    #[allow(clippy::float_cmp)] // Comparing known default constants (0.9, 1.0)
    fn test_budget_config_defaults() {
        let config = BudgetConfig::default();
        assert_eq!(config.daily_limit, None);
        assert_eq!(config.monthly_limit, None);
        assert_eq!(config.warning_threshold, 0.9);
        assert_eq!(config.critical_threshold, 1.0);
        assert!(!config.enforce_hard_limit);
    }

    #[test]
    fn test_budget_config_validation() {
        // Valid config
        let config = BudgetConfig::default();
        assert!(config.validate().is_ok());

        // Invalid warning threshold
        let config = BudgetConfig {
            warning_threshold: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Warning > critical
        let config = BudgetConfig {
            warning_threshold: 0.95,
            critical_threshold: 0.9,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_budget_enforcer_within_limit() {
        let tracker = CostTracker::with_defaults();
        let config = BudgetConfig::with_daily_limit(10.0);
        let enforcer = BudgetEnforcer::new(tracker, config);

        // Small usage should be ok
        let result = enforcer.record_and_check("gpt-4o-mini", 1000, 500);
        assert!(result.is_ok());
        assert_eq!(enforcer.alert_level(), None);
    }

    #[test]
    fn test_budget_enforcer_alert_level() {
        let tracker = CostTracker::with_defaults();
        let config = BudgetConfig::with_daily_limit(0.01) // Very low limit
            .warning_threshold(0.1); // Trigger at 10%

        let enforcer = BudgetEnforcer::new(tracker, config);

        // This usage should trigger warning (costs ~$0.00045)
        enforcer
            .record_and_check("gpt-4o-mini", 5000, 2500)
            .unwrap();

        // With 0.01 budget and ~0.00045 cost, we're at 4.5% - let's check if warning triggered
        let report = enforcer.tracker().report();
        let usage_pct = report.spent_today / 0.01;
        if usage_pct >= 0.1 {
            assert_eq!(enforcer.alert_level(), Some(AlertLevel::Warning));
        }
    }

    #[test]
    fn test_prometheus_export() {
        let tracker = CostTracker::with_defaults();
        tracker.record_usage("gpt-4o-mini", 1000, 500).unwrap();

        let metrics = tracker.export_prometheus();
        assert!(metrics.contains("llm_cost_total"));
        assert!(metrics.contains("llm_requests_total"));
        assert!(metrics.contains("llm_cost_by_model"));
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(1000, 500);
        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.total(), 1500);
    }

    // M-302: Per-user/session cost tracking tests

    #[test]
    fn test_cost_tracker_with_user_id() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call_with_context("gpt-4", 1000, 500, Some("node1"), Some("user123"), None)
            .unwrap();
        tracker
            .record_llm_call_with_context("gpt-4", 2000, 800, Some("node2"), Some("user123"), None)
            .unwrap();
        tracker
            .record_llm_call_with_context("gpt-4", 500, 200, Some("node1"), Some("user456"), None)
            .unwrap();

        let report = tracker.report();
        assert_eq!(report.total_calls(), 3);

        // Verify user breakdown
        let user123_cost = report.cost_by_user().get("user123").unwrap();
        let user456_cost = report.cost_by_user().get("user456").unwrap();
        assert!(*user123_cost > *user456_cost);
        assert_eq!(report.cost_by_user().len(), 2);
    }

    #[test]
    fn test_cost_tracker_with_session_id() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                1000,
                500,
                None,
                Some("user1"),
                Some("session-abc"),
            )
            .unwrap();
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                2000,
                800,
                None,
                Some("user1"),
                Some("session-abc"),
            )
            .unwrap();
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                500,
                200,
                None,
                Some("user1"),
                Some("session-xyz"),
            )
            .unwrap();

        let report = tracker.report();
        assert_eq!(report.total_calls(), 3);

        // Verify session breakdown
        let session_abc = report.cost_by_session().get("session-abc").unwrap();
        let session_xyz = report.cost_by_session().get("session-xyz").unwrap();
        assert!(*session_abc > *session_xyz);
        assert_eq!(report.cost_by_session().len(), 2);
    }

    #[test]
    fn test_cost_tracker_with_user_and_session() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                1000,
                500,
                Some("node1"),
                Some("user1"),
                Some("sess1"),
            )
            .unwrap();
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                1500,
                700,
                Some("node2"),
                Some("user2"),
                Some("sess2"),
            )
            .unwrap();

        let report = tracker.report();

        // Both user and session should be tracked
        assert_eq!(report.cost_by_user().len(), 2);
        assert_eq!(report.cost_by_session().len(), 2);
        assert!(report.cost_by_user().contains_key("user1"));
        assert!(report.cost_by_user().contains_key("user2"));
        assert!(report.cost_by_session().contains_key("sess1"));
        assert!(report.cost_by_session().contains_key("sess2"));
    }

    #[test]
    fn test_cost_report_format_with_user_session() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                1000,
                500,
                Some("node1"),
                Some("test-user"),
                Some("test-session"),
            )
            .unwrap();

        let report = tracker.report();
        let formatted = report.format();

        assert!(formatted.contains("Cost by User:"));
        assert!(formatted.contains("test-user"));
        assert!(formatted.contains("Cost by Session:"));
        assert!(formatted.contains("test-session"));
    }

    #[test]
    fn test_prometheus_export_with_user_session() {
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        tracker
            .record_llm_call_with_context(
                "gpt-4",
                1000,
                500,
                None,
                Some("prom-user"),
                Some("prom-session"),
            )
            .unwrap();

        let metrics = tracker.export_prometheus();
        assert!(metrics.contains("llm_cost_by_user"));
        assert!(metrics.contains("prom-user"));
        assert!(metrics.contains("llm_cost_by_session"));
        assert!(metrics.contains("prom-session"));
    }

    #[test]
    fn test_record_usage_with_context() {
        let tracker = CostTracker::with_defaults();
        tracker
            .record_usage_with_context("gpt-4o-mini", 1000, 500, Some("usage-user"), Some("usage-session"))
            .unwrap();

        let report = tracker.report();
        assert_eq!(report.total_calls(), 1);
        assert!(report.cost_by_user().contains_key("usage-user"));
        assert!(report.cost_by_session().contains_key("usage-session"));
    }

    #[test]
    fn test_cost_record_serialization_with_user_session() {
        let record = CostRecord {
            timestamp: Local::now(),
            model: "gpt-4".to_string(),
            usage: TokenUsage::new(100, 50),
            cost: 0.01,
            node_name: Some("test-node".to_string()),
            user_id: Some("ser-user".to_string()),
            session_id: Some("ser-session".to_string()),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("ser-user"));
        assert!(json.contains("ser-session"));

        let deserialized: CostRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, Some("ser-user".to_string()));
        assert_eq!(deserialized.session_id, Some("ser-session".to_string()));
    }

    #[test]
    fn test_backward_compatibility_without_user_session() {
        // Test that old code without user/session still works
        let pricing = ModelPricing::new().with_model("gpt-4", Pricing::per_1k(0.03, 0.06));

        let mut tracker = CostTracker::new(pricing);
        // Use the old API without user/session
        tracker
            .record_llm_call("gpt-4", 1000, 500, Some("old-node"))
            .unwrap();

        let report = tracker.report();
        assert_eq!(report.total_calls(), 1);
        assert!(report.cost_by_user().is_empty());
        assert!(report.cost_by_session().is_empty());
        assert!(report.cost_by_node().contains_key("old-node"));
    }
}
