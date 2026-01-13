//! Resource Usage Awareness
//!
//! This module provides types for tracking resource usage including tokens,
//! API calls, costs, and execution time budgets.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Resource Usage Awareness
// ============================================================================

/// Resource usage tracking - monitors budget and consumption
///
/// This struct enables AI agents to track their resource consumption including
/// token usage, API calls, cost, and execution time. Agents can monitor budgets
/// and make decisions to avoid exceeding limits.
///
/// # Example
///
/// ```rust,ignore
/// let usage = ResourceUsage::new()
///     .with_tokens_used(5000)
///     .with_tokens_budget(10000)
///     .with_api_calls(25)
///     .with_cost_usd(0.15);
///
/// // AI checks its budget
/// if usage.is_near_token_limit(0.9) {
///     // 90% of token budget used, wrap up
/// }
///
/// if usage.is_over_cost_limit(1.0) {
///     // Cost limit exceeded
///     return Err(Error::BudgetExceeded);
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Total tokens used (input + output)
    pub tokens_used: u64,
    /// Token budget limit (0 = unlimited)
    pub tokens_budget: u64,
    /// Input tokens used
    pub input_tokens: u64,
    /// Output tokens used
    pub output_tokens: u64,
    /// Number of API calls made
    pub api_calls: u64,
    /// API call budget limit (0 = unlimited)
    pub api_calls_budget: u64,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Cost budget limit in USD (0.0 = unlimited)
    pub cost_budget_usd: f64,
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
    /// Execution time budget in milliseconds (0 = unlimited)
    pub execution_time_budget_ms: u64,
    /// Thread ID associated with this usage
    pub thread_id: Option<String>,
    /// Execution ID for this usage tracking
    pub execution_id: Option<String>,
    /// Timestamp when tracking started (ISO 8601)
    pub started_at: Option<String>,
    /// Timestamp of last update (ISO 8601)
    pub updated_at: Option<String>,
    /// Custom resource tracking
    pub custom: HashMap<String, f64>,
}

impl ResourceUsage {
    /// Create a new resource usage tracker
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for resource usage
    #[must_use]
    pub fn builder() -> ResourceUsageBuilder {
        ResourceUsageBuilder::new()
    }

    /// Set total tokens used
    #[must_use]
    pub fn with_tokens_used(mut self, tokens: u64) -> Self {
        self.tokens_used = tokens;
        self
    }

    /// Set token budget
    #[must_use]
    pub fn with_tokens_budget(mut self, budget: u64) -> Self {
        self.tokens_budget = budget;
        self
    }

    /// Set input tokens used
    #[must_use]
    pub fn with_input_tokens(mut self, tokens: u64) -> Self {
        self.input_tokens = tokens;
        self
    }

    /// Set output tokens used
    #[must_use]
    pub fn with_output_tokens(mut self, tokens: u64) -> Self {
        self.output_tokens = tokens;
        self
    }

    /// Set number of API calls
    #[must_use]
    pub fn with_api_calls(mut self, calls: u64) -> Self {
        self.api_calls = calls;
        self
    }

    /// Set API call budget
    #[must_use]
    pub fn with_api_calls_budget(mut self, budget: u64) -> Self {
        self.api_calls_budget = budget;
        self
    }

    /// Set cost in USD
    #[must_use]
    pub fn with_cost_usd(mut self, cost: f64) -> Self {
        self.cost_usd = cost.max(0.0);
        self
    }

    /// Set cost budget in USD
    #[must_use]
    pub fn with_cost_budget_usd(mut self, budget: f64) -> Self {
        self.cost_budget_usd = budget.max(0.0);
        self
    }

    /// Set execution time in milliseconds
    #[must_use]
    pub fn with_execution_time_ms(mut self, time: u64) -> Self {
        self.execution_time_ms = time;
        self
    }

    /// Set execution time budget in milliseconds
    #[must_use]
    pub fn with_execution_time_budget_ms(mut self, budget: u64) -> Self {
        self.execution_time_budget_ms = budget;
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn with_thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    /// Set execution ID
    #[must_use]
    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    /// Set start timestamp
    #[must_use]
    pub fn with_started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = Some(timestamp.into());
        self
    }

    /// Set last update timestamp
    #[must_use]
    pub fn with_updated_at(mut self, timestamp: impl Into<String>) -> Self {
        self.updated_at = Some(timestamp.into());
        self
    }

    /// Add a custom resource metric
    #[must_use]
    pub fn with_custom(mut self, key: impl Into<String>, value: f64) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Get a custom resource metric
    #[must_use]
    pub fn get_custom(&self, key: &str) -> Option<f64> {
        self.custom.get(key).copied()
    }

    // ========================================================================
    // Token budget monitoring
    // ========================================================================

    /// Get remaining tokens (returns 0 if over budget or no budget set)
    #[must_use]
    pub fn remaining_tokens(&self) -> u64 {
        if self.tokens_budget == 0 {
            return u64::MAX; // Unlimited
        }
        self.tokens_budget.saturating_sub(self.tokens_used)
    }

    /// Get token usage percentage (0.0-100.0, returns 0 if no budget)
    #[must_use]
    pub fn token_usage_percentage(&self) -> f64 {
        if self.tokens_budget == 0 {
            return 0.0;
        }
        (self.tokens_used as f64 / self.tokens_budget as f64) * 100.0
    }

    /// Check if token usage is near the limit (above threshold, 0.0-1.0)
    #[must_use]
    pub fn is_near_token_limit(&self, threshold: f64) -> bool {
        if self.tokens_budget == 0 {
            return false;
        }
        let usage_ratio = self.tokens_used as f64 / self.tokens_budget as f64;
        usage_ratio >= threshold.clamp(0.0, 1.0)
    }

    /// Check if over token budget
    #[must_use]
    pub fn is_over_token_budget(&self) -> bool {
        self.tokens_budget > 0 && self.tokens_used > self.tokens_budget
    }

    /// Check if token budget is set
    #[must_use]
    pub fn has_token_budget(&self) -> bool {
        self.tokens_budget > 0
    }

    // ========================================================================
    // API call budget monitoring
    // ========================================================================

    /// Get remaining API calls
    #[must_use]
    pub fn remaining_api_calls(&self) -> u64 {
        if self.api_calls_budget == 0 {
            return u64::MAX;
        }
        self.api_calls_budget.saturating_sub(self.api_calls)
    }

    /// Get API call usage percentage
    #[must_use]
    pub fn api_call_usage_percentage(&self) -> f64 {
        if self.api_calls_budget == 0 {
            return 0.0;
        }
        (self.api_calls as f64 / self.api_calls_budget as f64) * 100.0
    }

    /// Check if near API call limit
    #[must_use]
    pub fn is_near_api_call_limit(&self, threshold: f64) -> bool {
        if self.api_calls_budget == 0 {
            return false;
        }
        let usage_ratio = self.api_calls as f64 / self.api_calls_budget as f64;
        usage_ratio >= threshold.clamp(0.0, 1.0)
    }

    /// Check if over API call budget
    #[must_use]
    pub fn is_over_api_call_budget(&self) -> bool {
        self.api_calls_budget > 0 && self.api_calls > self.api_calls_budget
    }

    /// Check if API call budget is set
    #[must_use]
    pub fn has_api_call_budget(&self) -> bool {
        self.api_calls_budget > 0
    }

    // ========================================================================
    // Cost budget monitoring
    // ========================================================================

    /// Get remaining cost budget
    #[must_use]
    pub fn remaining_cost_usd(&self) -> f64 {
        if self.cost_budget_usd == 0.0 {
            return f64::MAX;
        }
        (self.cost_budget_usd - self.cost_usd).max(0.0)
    }

    /// Get cost usage percentage
    #[must_use]
    pub fn cost_usage_percentage(&self) -> f64 {
        if self.cost_budget_usd == 0.0 {
            return 0.0;
        }
        (self.cost_usd / self.cost_budget_usd) * 100.0
    }

    /// Check if near cost limit
    #[must_use]
    pub fn is_near_cost_limit(&self, threshold: f64) -> bool {
        if self.cost_budget_usd == 0.0 {
            return false;
        }
        let usage_ratio = self.cost_usd / self.cost_budget_usd;
        usage_ratio >= threshold.clamp(0.0, 1.0)
    }

    /// Check if over cost budget
    #[must_use]
    pub fn is_over_cost_budget(&self) -> bool {
        self.cost_budget_usd > 0.0 && self.cost_usd > self.cost_budget_usd
    }

    /// Check if cost budget is set
    #[must_use]
    pub fn has_cost_budget(&self) -> bool {
        self.cost_budget_usd > 0.0
    }

    // ========================================================================
    // Time budget monitoring
    // ========================================================================

    /// Get remaining execution time in milliseconds
    #[must_use]
    pub fn remaining_time_ms(&self) -> u64 {
        if self.execution_time_budget_ms == 0 {
            return u64::MAX;
        }
        self.execution_time_budget_ms
            .saturating_sub(self.execution_time_ms)
    }

    /// Get execution time usage percentage
    #[must_use]
    pub fn time_usage_percentage(&self) -> f64 {
        if self.execution_time_budget_ms == 0 {
            return 0.0;
        }
        (self.execution_time_ms as f64 / self.execution_time_budget_ms as f64) * 100.0
    }

    /// Check if near time limit
    #[must_use]
    pub fn is_near_time_limit(&self, threshold: f64) -> bool {
        if self.execution_time_budget_ms == 0 {
            return false;
        }
        let usage_ratio = self.execution_time_ms as f64 / self.execution_time_budget_ms as f64;
        usage_ratio >= threshold.clamp(0.0, 1.0)
    }

    /// Check if over time budget
    #[must_use]
    pub fn is_over_time_budget(&self) -> bool {
        self.execution_time_budget_ms > 0 && self.execution_time_ms > self.execution_time_budget_ms
    }

    /// Check if time budget is set
    #[must_use]
    pub fn has_time_budget(&self) -> bool {
        self.execution_time_budget_ms > 0
    }

    // ========================================================================
    // Overall budget status
    // ========================================================================

    /// Check if any budget is exceeded
    #[must_use]
    pub fn is_any_budget_exceeded(&self) -> bool {
        self.is_over_token_budget()
            || self.is_over_api_call_budget()
            || self.is_over_cost_budget()
            || self.is_over_time_budget()
    }

    /// Check if near any limit (using provided threshold)
    #[must_use]
    pub fn is_near_any_limit(&self, threshold: f64) -> bool {
        self.is_near_token_limit(threshold)
            || self.is_near_api_call_limit(threshold)
            || self.is_near_cost_limit(threshold)
            || self.is_near_time_limit(threshold)
    }

    /// Check all budgets and return alerts
    #[must_use]
    pub fn check_budgets(&self, warning_threshold: f64) -> Vec<BudgetAlert> {
        let mut alerts = Vec::new();
        let threshold = warning_threshold.clamp(0.0, 1.0);

        // Token alerts
        if self.is_over_token_budget() {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::TokensExceeded,
                resource_name: "tokens".to_string(),
                current_value: self.tokens_used as f64,
                budget_value: self.tokens_budget as f64,
                severity: BudgetAlertSeverity::Critical,
                message: format!(
                    "Token budget exceeded: {} / {} ({}% over)",
                    self.tokens_used,
                    self.tokens_budget,
                    ((self.tokens_used as f64 / self.tokens_budget as f64 - 1.0) * 100.0) as i32
                ),
            });
        } else if self.is_near_token_limit(threshold) {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::TokensNearLimit,
                resource_name: "tokens".to_string(),
                current_value: self.tokens_used as f64,
                budget_value: self.tokens_budget as f64,
                severity: BudgetAlertSeverity::Warning,
                message: format!(
                    "Token budget {:.1}% used: {} / {}",
                    self.token_usage_percentage(),
                    self.tokens_used,
                    self.tokens_budget
                ),
            });
        }

        // API call alerts
        if self.is_over_api_call_budget() {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::ApiCallsExceeded,
                resource_name: "api_calls".to_string(),
                current_value: self.api_calls as f64,
                budget_value: self.api_calls_budget as f64,
                severity: BudgetAlertSeverity::Critical,
                message: format!(
                    "API call budget exceeded: {} / {}",
                    self.api_calls, self.api_calls_budget
                ),
            });
        } else if self.is_near_api_call_limit(threshold) {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::ApiCallsNearLimit,
                resource_name: "api_calls".to_string(),
                current_value: self.api_calls as f64,
                budget_value: self.api_calls_budget as f64,
                severity: BudgetAlertSeverity::Warning,
                message: format!(
                    "API call budget {:.1}% used: {} / {}",
                    self.api_call_usage_percentage(),
                    self.api_calls,
                    self.api_calls_budget
                ),
            });
        }

        // Cost alerts
        if self.is_over_cost_budget() {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::CostExceeded,
                resource_name: "cost_usd".to_string(),
                current_value: self.cost_usd,
                budget_value: self.cost_budget_usd,
                severity: BudgetAlertSeverity::Critical,
                message: format!(
                    "Cost budget exceeded: ${:.4} / ${:.4}",
                    self.cost_usd, self.cost_budget_usd
                ),
            });
        } else if self.is_near_cost_limit(threshold) {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::CostNearLimit,
                resource_name: "cost_usd".to_string(),
                current_value: self.cost_usd,
                budget_value: self.cost_budget_usd,
                severity: BudgetAlertSeverity::Warning,
                message: format!(
                    "Cost budget {:.1}% used: ${:.4} / ${:.4}",
                    self.cost_usage_percentage(),
                    self.cost_usd,
                    self.cost_budget_usd
                ),
            });
        }

        // Time alerts
        if self.is_over_time_budget() {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::TimeExceeded,
                resource_name: "execution_time_ms".to_string(),
                current_value: self.execution_time_ms as f64,
                budget_value: self.execution_time_budget_ms as f64,
                severity: BudgetAlertSeverity::Critical,
                message: format!(
                    "Time budget exceeded: {}ms / {}ms",
                    self.execution_time_ms, self.execution_time_budget_ms
                ),
            });
        } else if self.is_near_time_limit(threshold) {
            alerts.push(BudgetAlert {
                alert_type: BudgetAlertType::TimeNearLimit,
                resource_name: "execution_time_ms".to_string(),
                current_value: self.execution_time_ms as f64,
                budget_value: self.execution_time_budget_ms as f64,
                severity: BudgetAlertSeverity::Warning,
                message: format!(
                    "Time budget {:.1}% used: {}ms / {}ms",
                    self.time_usage_percentage(),
                    self.execution_time_ms,
                    self.execution_time_budget_ms
                ),
            });
        }

        alerts
    }

    /// Calculate cost per token (average)
    #[must_use]
    pub fn cost_per_token(&self) -> f64 {
        if self.tokens_used == 0 {
            return 0.0;
        }
        self.cost_usd / self.tokens_used as f64
    }

    /// Calculate tokens per API call (average)
    #[must_use]
    pub fn tokens_per_api_call(&self) -> f64 {
        if self.api_calls == 0 {
            return 0.0;
        }
        self.tokens_used as f64 / self.api_calls as f64
    }

    /// Generate a summary of resource usage
    #[must_use]
    pub fn summarize(&self) -> String {
        let mut summary = String::from("Resource Usage:\n");

        // Tokens
        if self.has_token_budget() {
            summary.push_str(&format!(
                "- Tokens: {} / {} ({:.1}%)\n",
                self.tokens_used,
                self.tokens_budget,
                self.token_usage_percentage()
            ));
        } else {
            summary.push_str(&format!("- Tokens: {}\n", self.tokens_used));
        }

        if self.input_tokens > 0 || self.output_tokens > 0 {
            summary.push_str(&format!(
                "  - Input: {}, Output: {}\n",
                self.input_tokens, self.output_tokens
            ));
        }

        // API calls
        if self.has_api_call_budget() {
            summary.push_str(&format!(
                "- API calls: {} / {} ({:.1}%)\n",
                self.api_calls,
                self.api_calls_budget,
                self.api_call_usage_percentage()
            ));
        } else {
            summary.push_str(&format!("- API calls: {}\n", self.api_calls));
        }

        // Cost
        if self.has_cost_budget() {
            summary.push_str(&format!(
                "- Cost: ${:.4} / ${:.4} ({:.1}%)\n",
                self.cost_usd,
                self.cost_budget_usd,
                self.cost_usage_percentage()
            ));
        } else {
            summary.push_str(&format!("- Cost: ${:.4}\n", self.cost_usd));
        }

        // Time
        if self.has_time_budget() {
            summary.push_str(&format!(
                "- Time: {}ms / {}ms ({:.1}%)\n",
                self.execution_time_ms,
                self.execution_time_budget_ms,
                self.time_usage_percentage()
            ));
        } else {
            summary.push_str(&format!("- Time: {}ms\n", self.execution_time_ms));
        }

        // Status
        let status = if self.is_any_budget_exceeded() {
            "OVER BUDGET"
        } else if self.is_near_any_limit(0.9) {
            "NEAR LIMITS"
        } else {
            "OK"
        };
        summary.push_str(&format!("- Status: {}\n", status));

        summary
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Builder for creating resource usage trackers
#[derive(Debug, Default)]
pub struct ResourceUsageBuilder {
    tokens_used: u64,
    tokens_budget: u64,
    input_tokens: u64,
    output_tokens: u64,
    api_calls: u64,
    api_calls_budget: u64,
    cost_usd: f64,
    cost_budget_usd: f64,
    execution_time_ms: u64,
    execution_time_budget_ms: u64,
    thread_id: Option<String>,
    execution_id: Option<String>,
    started_at: Option<String>,
    updated_at: Option<String>,
    custom: HashMap<String, f64>,
}

impl ResourceUsageBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set tokens used
    #[must_use]
    pub fn tokens_used(mut self, tokens: u64) -> Self {
        self.tokens_used = tokens;
        self
    }

    /// Set token budget
    #[must_use]
    pub fn tokens_budget(mut self, budget: u64) -> Self {
        self.tokens_budget = budget;
        self
    }

    /// Set input tokens
    #[must_use]
    pub fn input_tokens(mut self, tokens: u64) -> Self {
        self.input_tokens = tokens;
        self
    }

    /// Set output tokens
    #[must_use]
    pub fn output_tokens(mut self, tokens: u64) -> Self {
        self.output_tokens = tokens;
        self
    }

    /// Set API calls
    #[must_use]
    pub fn api_calls(mut self, calls: u64) -> Self {
        self.api_calls = calls;
        self
    }

    /// Set API calls budget
    #[must_use]
    pub fn api_calls_budget(mut self, budget: u64) -> Self {
        self.api_calls_budget = budget;
        self
    }

    /// Set cost in USD
    #[must_use]
    pub fn cost_usd(mut self, cost: f64) -> Self {
        self.cost_usd = cost.max(0.0);
        self
    }

    /// Set cost budget in USD
    #[must_use]
    pub fn cost_budget_usd(mut self, budget: f64) -> Self {
        self.cost_budget_usd = budget.max(0.0);
        self
    }

    /// Set execution time in milliseconds
    #[must_use]
    pub fn execution_time_ms(mut self, time: u64) -> Self {
        self.execution_time_ms = time;
        self
    }

    /// Set execution time budget
    #[must_use]
    pub fn execution_time_budget_ms(mut self, budget: u64) -> Self {
        self.execution_time_budget_ms = budget;
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    /// Set execution ID
    #[must_use]
    pub fn execution_id(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    /// Set start timestamp
    #[must_use]
    pub fn started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = Some(timestamp.into());
        self
    }

    /// Set last update timestamp
    #[must_use]
    pub fn updated_at(mut self, timestamp: impl Into<String>) -> Self {
        self.updated_at = Some(timestamp.into());
        self
    }

    /// Add a custom resource metric
    #[must_use]
    pub fn custom(mut self, key: impl Into<String>, value: f64) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Build the resource usage tracker
    #[must_use]
    pub fn build(self) -> ResourceUsage {
        ResourceUsage {
            tokens_used: self.tokens_used,
            tokens_budget: self.tokens_budget,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            api_calls: self.api_calls,
            api_calls_budget: self.api_calls_budget,
            cost_usd: self.cost_usd,
            cost_budget_usd: self.cost_budget_usd,
            execution_time_ms: self.execution_time_ms,
            execution_time_budget_ms: self.execution_time_budget_ms,
            thread_id: self.thread_id,
            execution_id: self.execution_id,
            started_at: self.started_at,
            updated_at: self.updated_at,
            custom: self.custom,
        }
    }
}

/// Budget alert type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetAlertType {
    /// Token usage near limit
    TokensNearLimit,
    /// Token budget exceeded
    TokensExceeded,
    /// API calls near limit
    ApiCallsNearLimit,
    /// API call budget exceeded
    ApiCallsExceeded,
    /// Cost near limit
    CostNearLimit,
    /// Cost budget exceeded
    CostExceeded,
    /// Execution time near limit
    TimeNearLimit,
    /// Time budget exceeded
    TimeExceeded,
}

/// Budget alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetAlertSeverity {
    /// Informational - resource usage is notable
    Info,
    /// Warning - approaching limit
    Warning,
    /// Critical - budget exceeded
    Critical,
}

impl BudgetAlertSeverity {
    /// Check if severity is critical
    #[must_use]
    pub fn is_critical(&self) -> bool {
        matches!(self, BudgetAlertSeverity::Critical)
    }
}

/// Budget alert - represents a budget warning or exceeded limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    /// Type of alert
    pub alert_type: BudgetAlertType,
    /// Name of the resource
    pub resource_name: String,
    /// Current value of the resource
    pub current_value: f64,
    /// Budget value for the resource
    pub budget_value: f64,
    /// Severity of the alert
    pub severity: BudgetAlertSeverity,
    /// Human-readable message
    pub message: String,
}

impl BudgetAlert {
    /// Check if this alert is critical
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.severity.is_critical()
    }

    /// Get usage ratio (current / budget)
    #[must_use]
    pub fn usage_ratio(&self) -> f64 {
        if self.budget_value == 0.0 {
            return 0.0;
        }
        self.current_value / self.budget_value
    }

    /// Get percentage over budget (0 if under budget)
    #[must_use]
    pub fn over_budget_percentage(&self) -> f64 {
        if self.budget_value == 0.0 || self.current_value <= self.budget_value {
            return 0.0;
        }
        ((self.current_value / self.budget_value) - 1.0) * 100.0
    }
}

/// Resource usage history - tracks usage over time
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsageHistory {
    /// Historical usage snapshots
    pub snapshots: Vec<ResourceUsage>,
    /// Maximum number of snapshots to retain
    pub max_snapshots: usize,
    /// Thread ID for this history
    pub thread_id: Option<String>,
}

impl ResourceUsageHistory {
    /// Create a new resource usage history
    #[must_use]
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
            thread_id: None,
        }
    }

    /// Create with thread ID
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add a usage snapshot
    pub fn add(&mut self, usage: ResourceUsage) {
        self.snapshots.push(usage);
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Get number of snapshots
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Check if history is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get latest snapshot
    #[must_use]
    pub fn latest(&self) -> Option<&ResourceUsage> {
        self.snapshots.last()
    }

    /// Get total tokens used across all snapshots
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.snapshots
            .iter()
            .map(|s| s.tokens_used)
            .max()
            .unwrap_or(0)
    }

    /// Get total cost across all snapshots
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.snapshots
            .iter()
            .map(|s| s.cost_usd)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    /// Get total API calls across all snapshots
    #[must_use]
    pub fn total_api_calls(&self) -> u64 {
        self.snapshots
            .iter()
            .map(|s| s.api_calls)
            .max()
            .unwrap_or(0)
    }

    /// Calculate token usage rate (tokens per millisecond)
    #[must_use]
    pub fn token_rate(&self) -> f64 {
        if let (Some(first), Some(last)) = (self.snapshots.first(), self.snapshots.last()) {
            let token_diff = last.tokens_used.saturating_sub(first.tokens_used);
            let time_diff = last
                .execution_time_ms
                .saturating_sub(first.execution_time_ms);
            if time_diff > 0 {
                return token_diff as f64 / time_diff as f64;
            }
        }
        0.0
    }

    /// Calculate cost rate (USD per millisecond)
    #[must_use]
    pub fn cost_rate(&self) -> f64 {
        if let (Some(first), Some(last)) = (self.snapshots.first(), self.snapshots.last()) {
            let cost_diff = (last.cost_usd - first.cost_usd).max(0.0);
            let time_diff = last
                .execution_time_ms
                .saturating_sub(first.execution_time_ms);
            if time_diff > 0 {
                return cost_diff / time_diff as f64;
            }
        }
        0.0
    }

    /// Estimate remaining time until token budget exhaustion
    #[must_use]
    pub fn estimate_time_to_token_limit_ms(&self) -> Option<u64> {
        let rate = self.token_rate();
        if rate <= 0.0 {
            return None;
        }
        if let Some(latest) = self.latest() {
            if latest.tokens_budget > 0 && latest.tokens_used < latest.tokens_budget {
                let remaining = latest.tokens_budget - latest.tokens_used;
                return Some((remaining as f64 / rate) as u64);
            }
        }
        None
    }

    /// Estimate remaining time until cost budget exhaustion
    #[must_use]
    pub fn estimate_time_to_cost_limit_ms(&self) -> Option<u64> {
        let rate = self.cost_rate();
        if rate <= 0.0 {
            return None;
        }
        if let Some(latest) = self.latest() {
            if latest.cost_budget_usd > 0.0 && latest.cost_usd < latest.cost_budget_usd {
                let remaining = latest.cost_budget_usd - latest.cost_usd;
                return Some((remaining / rate) as u64);
            }
        }
        None
    }

    /// Get usage summary based on latest snapshot
    #[must_use]
    pub fn usage_summary(&self) -> String {
        match self.latest() {
            Some(usage) => {
                let status = if usage.is_any_budget_exceeded() {
                    "OVER BUDGET"
                } else if usage.is_near_any_limit(0.9) {
                    "NEAR LIMITS"
                } else {
                    "OK"
                };
                format!(
                    "Tokens: {}, Cost: ${:.4}, API calls: {}, Status: {}",
                    usage.tokens_used, usage.cost_usd, usage.api_calls, status
                )
            }
            None => "No usage data available".to_string(),
        }
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

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ResourceUsage - Basic Creation and Defaults
    // ========================================================================

    #[test]
    fn test_resource_usage_default() {
        let usage = ResourceUsage::default();
        assert_eq!(usage.tokens_used, 0);
        assert_eq!(usage.tokens_budget, 0);
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.api_calls, 0);
        assert_eq!(usage.api_calls_budget, 0);
        assert!((usage.cost_usd - 0.0).abs() < f64::EPSILON);
        assert!((usage.cost_budget_usd - 0.0).abs() < f64::EPSILON);
        assert_eq!(usage.execution_time_ms, 0);
        assert_eq!(usage.execution_time_budget_ms, 0);
        assert!(usage.thread_id.is_none());
        assert!(usage.execution_id.is_none());
        assert!(usage.custom.is_empty());
    }

    #[test]
    fn test_resource_usage_new() {
        let usage = ResourceUsage::new();
        assert_eq!(usage.tokens_used, 0);
        assert!(usage.custom.is_empty());
    }

    #[test]
    fn test_resource_usage_builder_chain() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_input_tokens(3000)
            .with_output_tokens(2000)
            .with_api_calls(25)
            .with_api_calls_budget(100)
            .with_cost_usd(0.15)
            .with_cost_budget_usd(1.0)
            .with_execution_time_ms(5000)
            .with_execution_time_budget_ms(60000)
            .with_thread_id("thread-123")
            .with_execution_id("exec-456")
            .with_started_at("2024-01-01T00:00:00Z")
            .with_updated_at("2024-01-01T00:05:00Z")
            .with_custom("gpu_memory_mb", 1024.0);

        assert_eq!(usage.tokens_used, 5000);
        assert_eq!(usage.tokens_budget, 10000);
        assert_eq!(usage.input_tokens, 3000);
        assert_eq!(usage.output_tokens, 2000);
        assert_eq!(usage.api_calls, 25);
        assert_eq!(usage.api_calls_budget, 100);
        assert!((usage.cost_usd - 0.15).abs() < f64::EPSILON);
        assert!((usage.cost_budget_usd - 1.0).abs() < f64::EPSILON);
        assert_eq!(usage.execution_time_ms, 5000);
        assert_eq!(usage.execution_time_budget_ms, 60000);
        assert_eq!(usage.thread_id.as_deref(), Some("thread-123"));
        assert_eq!(usage.execution_id.as_deref(), Some("exec-456"));
        assert_eq!(usage.started_at.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert_eq!(usage.updated_at.as_deref(), Some("2024-01-01T00:05:00Z"));
        assert_eq!(usage.get_custom("gpu_memory_mb"), Some(1024.0));
    }

    #[test]
    fn test_cost_usd_negative_clamped() {
        let usage = ResourceUsage::new().with_cost_usd(-5.0);
        assert!((usage.cost_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cost_budget_usd_negative_clamped() {
        let usage = ResourceUsage::new().with_cost_budget_usd(-10.0);
        assert!((usage.cost_budget_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_metrics() {
        let usage = ResourceUsage::new()
            .with_custom("metric1", 100.0)
            .with_custom("metric2", 200.0);

        assert_eq!(usage.get_custom("metric1"), Some(100.0));
        assert_eq!(usage.get_custom("metric2"), Some(200.0));
        assert_eq!(usage.get_custom("nonexistent"), None);
    }

    // ========================================================================
    // Token Budget Monitoring
    // ========================================================================

    #[test]
    fn test_remaining_tokens_with_budget() {
        let usage = ResourceUsage::new()
            .with_tokens_used(3000)
            .with_tokens_budget(10000);
        assert_eq!(usage.remaining_tokens(), 7000);
    }

    #[test]
    fn test_remaining_tokens_no_budget_returns_max() {
        let usage = ResourceUsage::new().with_tokens_used(5000);
        assert_eq!(usage.remaining_tokens(), u64::MAX);
    }

    #[test]
    fn test_remaining_tokens_over_budget_returns_zero() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000);
        assert_eq!(usage.remaining_tokens(), 0);
    }

    #[test]
    fn test_token_usage_percentage() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000);
        assert!((usage.token_usage_percentage() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_usage_percentage_no_budget() {
        let usage = ResourceUsage::new().with_tokens_used(5000);
        assert!((usage.token_usage_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_near_token_limit_true() {
        let usage = ResourceUsage::new()
            .with_tokens_used(9500)
            .with_tokens_budget(10000);
        assert!(usage.is_near_token_limit(0.9)); // 95% >= 90%
    }

    #[test]
    fn test_is_near_token_limit_false() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000);
        assert!(!usage.is_near_token_limit(0.9)); // 50% < 90%
    }

    #[test]
    fn test_is_near_token_limit_no_budget() {
        let usage = ResourceUsage::new().with_tokens_used(9999);
        assert!(!usage.is_near_token_limit(0.9));
    }

    #[test]
    fn test_is_near_token_limit_threshold_clamped() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000);
        // Threshold > 1.0 should be clamped to 1.0
        assert!(!usage.is_near_token_limit(2.0)); // 50% < 100%
                                                  // Threshold < 0.0 should be clamped to 0.0
        assert!(usage.is_near_token_limit(-1.0)); // 50% >= 0%
    }

    #[test]
    fn test_is_over_token_budget() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000);
        assert!(usage.is_over_token_budget());
    }

    #[test]
    fn test_is_not_over_token_budget() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000);
        assert!(!usage.is_over_token_budget());
    }

    #[test]
    fn test_has_token_budget() {
        let with_budget = ResourceUsage::new().with_tokens_budget(1000);
        let without_budget = ResourceUsage::new();

        assert!(with_budget.has_token_budget());
        assert!(!without_budget.has_token_budget());
    }

    // ========================================================================
    // API Call Budget Monitoring
    // ========================================================================

    #[test]
    fn test_remaining_api_calls() {
        let usage = ResourceUsage::new()
            .with_api_calls(30)
            .with_api_calls_budget(100);
        assert_eq!(usage.remaining_api_calls(), 70);
    }

    #[test]
    fn test_remaining_api_calls_no_budget() {
        let usage = ResourceUsage::new().with_api_calls(50);
        assert_eq!(usage.remaining_api_calls(), u64::MAX);
    }

    #[test]
    fn test_api_call_usage_percentage() {
        let usage = ResourceUsage::new()
            .with_api_calls(25)
            .with_api_calls_budget(100);
        assert!((usage.api_call_usage_percentage() - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_near_api_call_limit() {
        let usage = ResourceUsage::new()
            .with_api_calls(95)
            .with_api_calls_budget(100);
        assert!(usage.is_near_api_call_limit(0.9));
    }

    #[test]
    fn test_is_over_api_call_budget() {
        let usage = ResourceUsage::new()
            .with_api_calls(150)
            .with_api_calls_budget(100);
        assert!(usage.is_over_api_call_budget());
    }

    #[test]
    fn test_has_api_call_budget() {
        let with_budget = ResourceUsage::new().with_api_calls_budget(100);
        let without_budget = ResourceUsage::new();

        assert!(with_budget.has_api_call_budget());
        assert!(!without_budget.has_api_call_budget());
    }

    // ========================================================================
    // Cost Budget Monitoring
    // ========================================================================

    #[test]
    fn test_remaining_cost_usd() {
        let usage = ResourceUsage::new()
            .with_cost_usd(0.30)
            .with_cost_budget_usd(1.0);
        assert!((usage.remaining_cost_usd() - 0.70).abs() < 0.001);
    }

    #[test]
    fn test_remaining_cost_usd_no_budget() {
        let usage = ResourceUsage::new().with_cost_usd(10.0);
        assert_eq!(usage.remaining_cost_usd(), f64::MAX);
    }

    #[test]
    fn test_remaining_cost_usd_over_budget() {
        let usage = ResourceUsage::new()
            .with_cost_usd(1.50)
            .with_cost_budget_usd(1.0);
        assert!((usage.remaining_cost_usd() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cost_usage_percentage() {
        let usage = ResourceUsage::new()
            .with_cost_usd(0.25)
            .with_cost_budget_usd(1.0);
        assert!((usage.cost_usage_percentage() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_is_near_cost_limit() {
        let usage = ResourceUsage::new()
            .with_cost_usd(0.95)
            .with_cost_budget_usd(1.0);
        assert!(usage.is_near_cost_limit(0.9));
    }

    #[test]
    fn test_is_over_cost_budget() {
        let usage = ResourceUsage::new()
            .with_cost_usd(1.50)
            .with_cost_budget_usd(1.0);
        assert!(usage.is_over_cost_budget());
    }

    #[test]
    fn test_has_cost_budget() {
        let with_budget = ResourceUsage::new().with_cost_budget_usd(1.0);
        let without_budget = ResourceUsage::new();

        assert!(with_budget.has_cost_budget());
        assert!(!without_budget.has_cost_budget());
    }

    // ========================================================================
    // Time Budget Monitoring
    // ========================================================================

    #[test]
    fn test_remaining_time_ms() {
        let usage = ResourceUsage::new()
            .with_execution_time_ms(30000)
            .with_execution_time_budget_ms(60000);
        assert_eq!(usage.remaining_time_ms(), 30000);
    }

    #[test]
    fn test_remaining_time_ms_no_budget() {
        let usage = ResourceUsage::new().with_execution_time_ms(50000);
        assert_eq!(usage.remaining_time_ms(), u64::MAX);
    }

    #[test]
    fn test_time_usage_percentage() {
        let usage = ResourceUsage::new()
            .with_execution_time_ms(15000)
            .with_execution_time_budget_ms(60000);
        assert!((usage.time_usage_percentage() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_is_near_time_limit() {
        let usage = ResourceUsage::new()
            .with_execution_time_ms(55000)
            .with_execution_time_budget_ms(60000);
        assert!(usage.is_near_time_limit(0.9));
    }

    #[test]
    fn test_is_over_time_budget() {
        let usage = ResourceUsage::new()
            .with_execution_time_ms(70000)
            .with_execution_time_budget_ms(60000);
        assert!(usage.is_over_time_budget());
    }

    #[test]
    fn test_has_time_budget() {
        let with_budget = ResourceUsage::new().with_execution_time_budget_ms(60000);
        let without_budget = ResourceUsage::new();

        assert!(with_budget.has_time_budget());
        assert!(!without_budget.has_time_budget());
    }

    // ========================================================================
    // Combined Budget Checking
    // ========================================================================

    #[test]
    fn test_is_any_budget_exceeded_token() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000);
        assert!(usage.is_any_budget_exceeded());
    }

    #[test]
    fn test_is_any_budget_exceeded_api() {
        let usage = ResourceUsage::new()
            .with_api_calls(150)
            .with_api_calls_budget(100);
        assert!(usage.is_any_budget_exceeded());
    }

    #[test]
    fn test_is_any_budget_exceeded_cost() {
        let usage = ResourceUsage::new()
            .with_cost_usd(2.0)
            .with_cost_budget_usd(1.0);
        assert!(usage.is_any_budget_exceeded());
    }

    #[test]
    fn test_is_any_budget_exceeded_time() {
        let usage = ResourceUsage::new()
            .with_execution_time_ms(70000)
            .with_execution_time_budget_ms(60000);
        assert!(usage.is_any_budget_exceeded());
    }

    #[test]
    fn test_is_any_budget_exceeded_none() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_api_calls(50)
            .with_api_calls_budget(100);
        assert!(!usage.is_any_budget_exceeded());
    }

    #[test]
    fn test_is_near_any_limit() {
        let usage = ResourceUsage::new()
            .with_tokens_used(9500)
            .with_tokens_budget(10000)
            .with_api_calls(50)
            .with_api_calls_budget(100);
        assert!(usage.is_near_any_limit(0.9)); // Tokens at 95%
    }

    #[test]
    fn test_is_not_near_any_limit() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_api_calls(50)
            .with_api_calls_budget(100);
        assert!(!usage.is_near_any_limit(0.9));
    }

    // ========================================================================
    // Alert Generation
    // ========================================================================

    #[test]
    fn test_check_budgets_token_exceeded() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000);

        let alerts = usage.check_budgets(0.9);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, BudgetAlertType::TokensExceeded);
        assert_eq!(alerts[0].severity, BudgetAlertSeverity::Critical);
        assert!(alerts[0].is_critical());
    }

    #[test]
    fn test_check_budgets_token_near_limit() {
        let usage = ResourceUsage::new()
            .with_tokens_used(9500)
            .with_tokens_budget(10000);

        let alerts = usage.check_budgets(0.9);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, BudgetAlertType::TokensNearLimit);
        assert_eq!(alerts[0].severity, BudgetAlertSeverity::Warning);
        assert!(!alerts[0].is_critical());
    }

    #[test]
    fn test_check_budgets_multiple_alerts() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000)
            .with_cost_usd(1.5)
            .with_cost_budget_usd(1.0);

        let alerts = usage.check_budgets(0.9);
        assert_eq!(alerts.len(), 2);

        let token_alert = alerts
            .iter()
            .find(|a| matches!(a.alert_type, BudgetAlertType::TokensExceeded));
        let cost_alert = alerts
            .iter()
            .find(|a| matches!(a.alert_type, BudgetAlertType::CostExceeded));

        assert!(token_alert.is_some());
        assert!(cost_alert.is_some());
    }

    #[test]
    fn test_check_budgets_no_alerts() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_api_calls(50)
            .with_api_calls_budget(100);

        let alerts = usage.check_budgets(0.9);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_check_budgets_all_types() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000)
            .with_api_calls(150)
            .with_api_calls_budget(100)
            .with_cost_usd(2.0)
            .with_cost_budget_usd(1.0)
            .with_execution_time_ms(70000)
            .with_execution_time_budget_ms(60000);

        let alerts = usage.check_budgets(0.9);
        assert_eq!(alerts.len(), 4);

        // All should be critical
        for alert in &alerts {
            assert!(alert.is_critical());
        }
    }

    // ========================================================================
    // Utility Methods
    // ========================================================================

    #[test]
    fn test_cost_per_token() {
        let usage = ResourceUsage::new()
            .with_tokens_used(10000)
            .with_cost_usd(0.10);
        assert!((usage.cost_per_token() - 0.00001).abs() < 0.000001);
    }

    #[test]
    fn test_cost_per_token_zero_tokens() {
        let usage = ResourceUsage::new().with_cost_usd(0.10);
        assert!((usage.cost_per_token() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tokens_per_api_call() {
        let usage = ResourceUsage::new()
            .with_tokens_used(10000)
            .with_api_calls(10);
        assert!((usage.tokens_per_api_call() - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tokens_per_api_call_zero_calls() {
        let usage = ResourceUsage::new().with_tokens_used(10000);
        assert!((usage.tokens_per_api_call() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summarize_with_budgets() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_api_calls(50)
            .with_api_calls_budget(100)
            .with_cost_usd(0.50)
            .with_cost_budget_usd(1.0)
            .with_execution_time_ms(30000)
            .with_execution_time_budget_ms(60000);

        let summary = usage.summarize();
        assert!(summary.contains("Tokens: 5000 / 10000"));
        assert!(summary.contains("API calls: 50 / 100"));
        assert!(summary.contains("Cost: $0.5000 / $1.0000"));
        assert!(summary.contains("Time: 30000ms / 60000ms"));
        assert!(summary.contains("Status: OK"));
    }

    #[test]
    fn test_summarize_near_limits() {
        let usage = ResourceUsage::new()
            .with_tokens_used(9500)
            .with_tokens_budget(10000);

        let summary = usage.summarize();
        assert!(summary.contains("Status: NEAR LIMITS"));
    }

    #[test]
    fn test_summarize_over_budget() {
        let usage = ResourceUsage::new()
            .with_tokens_used(15000)
            .with_tokens_budget(10000);

        let summary = usage.summarize();
        assert!(summary.contains("Status: OVER BUDGET"));
    }

    #[test]
    fn test_summarize_with_input_output_tokens() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_input_tokens(3000)
            .with_output_tokens(2000);

        let summary = usage.summarize();
        assert!(summary.contains("Input: 3000, Output: 2000"));
    }

    // ========================================================================
    // JSON Serialization
    // ========================================================================

    #[test]
    fn test_to_json_and_from_json() {
        let usage = ResourceUsage::new()
            .with_tokens_used(5000)
            .with_tokens_budget(10000)
            .with_cost_usd(0.50)
            .with_thread_id("test-thread");

        let json = usage.to_json().expect("JSON serialization failed");
        let parsed = ResourceUsage::from_json(&json).expect("JSON parsing failed");

        assert_eq!(parsed.tokens_used, 5000);
        assert_eq!(parsed.tokens_budget, 10000);
        assert!((parsed.cost_usd - 0.50).abs() < f64::EPSILON);
        assert_eq!(parsed.thread_id.as_deref(), Some("test-thread"));
    }

    #[test]
    fn test_to_json_compact() {
        let usage = ResourceUsage::new().with_tokens_used(100);
        let json = usage.to_json_compact().expect("JSON serialization failed");
        assert!(!json.contains('\n')); // Compact should have no newlines
    }

    // ========================================================================
    // ResourceUsageBuilder
    // ========================================================================

    #[test]
    fn test_builder_all_fields() {
        let usage = ResourceUsageBuilder::new()
            .tokens_used(5000)
            .tokens_budget(10000)
            .input_tokens(3000)
            .output_tokens(2000)
            .api_calls(25)
            .api_calls_budget(100)
            .cost_usd(0.15)
            .cost_budget_usd(1.0)
            .execution_time_ms(5000)
            .execution_time_budget_ms(60000)
            .thread_id("builder-thread")
            .execution_id("builder-exec")
            .started_at("2024-01-01T00:00:00Z")
            .updated_at("2024-01-01T00:05:00Z")
            .custom("custom_metric", 42.0)
            .build();

        assert_eq!(usage.tokens_used, 5000);
        assert_eq!(usage.tokens_budget, 10000);
        assert_eq!(usage.input_tokens, 3000);
        assert_eq!(usage.output_tokens, 2000);
        assert_eq!(usage.api_calls, 25);
        assert_eq!(usage.api_calls_budget, 100);
        assert!((usage.cost_usd - 0.15).abs() < f64::EPSILON);
        assert!((usage.cost_budget_usd - 1.0).abs() < f64::EPSILON);
        assert_eq!(usage.execution_time_ms, 5000);
        assert_eq!(usage.execution_time_budget_ms, 60000);
        assert_eq!(usage.thread_id.as_deref(), Some("builder-thread"));
        assert_eq!(usage.execution_id.as_deref(), Some("builder-exec"));
        assert_eq!(usage.get_custom("custom_metric"), Some(42.0));
    }

    #[test]
    fn test_builder_defaults() {
        let usage = ResourceUsageBuilder::new().build();
        assert_eq!(usage.tokens_used, 0);
        assert!(usage.thread_id.is_none());
    }

    #[test]
    fn test_builder_cost_negative_clamped() {
        let usage = ResourceUsageBuilder::new()
            .cost_usd(-5.0)
            .cost_budget_usd(-10.0)
            .build();
        assert!((usage.cost_usd - 0.0).abs() < f64::EPSILON);
        assert!((usage.cost_budget_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_usage_builder_method() {
        let usage = ResourceUsage::builder().tokens_used(1000).build();
        assert_eq!(usage.tokens_used, 1000);
    }

    // ========================================================================
    // BudgetAlert
    // ========================================================================

    #[test]
    fn test_budget_alert_is_critical() {
        let critical_alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensExceeded,
            resource_name: "tokens".to_string(),
            current_value: 15000.0,
            budget_value: 10000.0,
            severity: BudgetAlertSeverity::Critical,
            message: "Test".to_string(),
        };
        assert!(critical_alert.is_critical());

        let warning_alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensNearLimit,
            resource_name: "tokens".to_string(),
            current_value: 9500.0,
            budget_value: 10000.0,
            severity: BudgetAlertSeverity::Warning,
            message: "Test".to_string(),
        };
        assert!(!warning_alert.is_critical());
    }

    #[test]
    fn test_budget_alert_usage_ratio() {
        let alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensNearLimit,
            resource_name: "tokens".to_string(),
            current_value: 7500.0,
            budget_value: 10000.0,
            severity: BudgetAlertSeverity::Warning,
            message: "Test".to_string(),
        };
        assert!((alert.usage_ratio() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_budget_alert_usage_ratio_zero_budget() {
        let alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensNearLimit,
            resource_name: "tokens".to_string(),
            current_value: 7500.0,
            budget_value: 0.0,
            severity: BudgetAlertSeverity::Warning,
            message: "Test".to_string(),
        };
        assert!((alert.usage_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_budget_alert_over_budget_percentage() {
        let alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensExceeded,
            resource_name: "tokens".to_string(),
            current_value: 15000.0,
            budget_value: 10000.0,
            severity: BudgetAlertSeverity::Critical,
            message: "Test".to_string(),
        };
        assert!((alert.over_budget_percentage() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_budget_alert_over_budget_percentage_under_budget() {
        let alert = BudgetAlert {
            alert_type: BudgetAlertType::TokensNearLimit,
            resource_name: "tokens".to_string(),
            current_value: 7500.0,
            budget_value: 10000.0,
            severity: BudgetAlertSeverity::Warning,
            message: "Test".to_string(),
        };
        assert!((alert.over_budget_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_budget_alert_severity_is_critical() {
        assert!(BudgetAlertSeverity::Critical.is_critical());
        assert!(!BudgetAlertSeverity::Warning.is_critical());
        assert!(!BudgetAlertSeverity::Info.is_critical());
    }

    // ========================================================================
    // ResourceUsageHistory
    // ========================================================================

    #[test]
    fn test_history_new() {
        let history = ResourceUsageHistory::new(10);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.max_snapshots, 10);
    }

    #[test]
    fn test_history_with_thread_id() {
        let history = ResourceUsageHistory::new(10).with_thread_id("test-thread");
        assert_eq!(history.thread_id.as_deref(), Some("test-thread"));
    }

    #[test]
    fn test_history_add_snapshots() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(ResourceUsage::new().with_tokens_used(100));
        history.add(ResourceUsage::new().with_tokens_used(200));

        assert_eq!(history.len(), 2);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_history_max_snapshots_eviction() {
        let mut history = ResourceUsageHistory::new(3);

        history.add(ResourceUsage::new().with_tokens_used(100));
        history.add(ResourceUsage::new().with_tokens_used(200));
        history.add(ResourceUsage::new().with_tokens_used(300));
        history.add(ResourceUsage::new().with_tokens_used(400)); // Should evict first

        assert_eq!(history.len(), 3);
        assert_eq!(history.snapshots[0].tokens_used, 200); // First is gone
        assert_eq!(history.snapshots[2].tokens_used, 400); // Latest
    }

    #[test]
    fn test_history_latest() {
        let mut history = ResourceUsageHistory::new(10);

        assert!(history.latest().is_none());

        history.add(ResourceUsage::new().with_tokens_used(100));
        history.add(ResourceUsage::new().with_tokens_used(200));

        let latest = history.latest().expect("Should have latest");
        assert_eq!(latest.tokens_used, 200);
    }

    #[test]
    fn test_history_total_tokens() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(ResourceUsage::new().with_tokens_used(100));
        history.add(ResourceUsage::new().with_tokens_used(500));
        history.add(ResourceUsage::new().with_tokens_used(300));

        // Should return max tokens across snapshots
        assert_eq!(history.total_tokens(), 500);
    }

    #[test]
    fn test_history_total_tokens_empty() {
        let history = ResourceUsageHistory::new(10);
        assert_eq!(history.total_tokens(), 0);
    }

    #[test]
    fn test_history_total_cost() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(ResourceUsage::new().with_cost_usd(0.10));
        history.add(ResourceUsage::new().with_cost_usd(0.50));
        history.add(ResourceUsage::new().with_cost_usd(0.30));

        // Should return max cost
        assert!((history.total_cost() - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_history_total_api_calls() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(ResourceUsage::new().with_api_calls(10));
        history.add(ResourceUsage::new().with_api_calls(50));
        history.add(ResourceUsage::new().with_api_calls(30));

        assert_eq!(history.total_api_calls(), 50);
    }

    #[test]
    fn test_history_token_rate() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(
            ResourceUsage::new()
                .with_tokens_used(0)
                .with_execution_time_ms(0),
        );
        history.add(
            ResourceUsage::new()
                .with_tokens_used(1000)
                .with_execution_time_ms(1000),
        );

        // 1000 tokens / 1000 ms = 1.0 tokens/ms
        assert!((history.token_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_history_token_rate_zero_time() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(ResourceUsage::new().with_tokens_used(0));
        history.add(ResourceUsage::new().with_tokens_used(1000));

        assert!((history.token_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_cost_rate() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(
            ResourceUsage::new()
                .with_cost_usd(0.0)
                .with_execution_time_ms(0),
        );
        history.add(
            ResourceUsage::new()
                .with_cost_usd(0.10)
                .with_execution_time_ms(1000),
        );

        // $0.10 / 1000 ms = 0.0001 $/ms
        assert!((history.cost_rate() - 0.0001).abs() < 0.00001);
    }

    #[test]
    fn test_history_estimate_time_to_token_limit() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(
            ResourceUsage::new()
                .with_tokens_used(0)
                .with_tokens_budget(10000)
                .with_execution_time_ms(0),
        );
        history.add(
            ResourceUsage::new()
                .with_tokens_used(1000)
                .with_tokens_budget(10000)
                .with_execution_time_ms(1000),
        );

        // Rate = 1.0 token/ms, remaining = 9000 tokens
        // Time to limit = 9000 / 1.0 = 9000 ms
        let estimate = history.estimate_time_to_token_limit_ms();
        assert!(estimate.is_some());
        assert_eq!(estimate.unwrap(), 9000);
    }

    #[test]
    fn test_history_estimate_time_to_token_limit_no_rate() {
        let mut history = ResourceUsageHistory::new(10);
        history.add(ResourceUsage::new().with_tokens_budget(10000));

        let estimate = history.estimate_time_to_token_limit_ms();
        assert!(estimate.is_none());
    }

    #[test]
    fn test_history_estimate_time_to_cost_limit() {
        let mut history = ResourceUsageHistory::new(10);

        history.add(
            ResourceUsage::new()
                .with_cost_usd(0.0)
                .with_cost_budget_usd(1.0)
                .with_execution_time_ms(0),
        );
        history.add(
            ResourceUsage::new()
                .with_cost_usd(0.10)
                .with_cost_budget_usd(1.0)
                .with_execution_time_ms(1000),
        );

        // Rate = 0.0001 $/ms, remaining = $0.90
        // Time = 0.90 / 0.0001 = 9000 ms
        let estimate = history.estimate_time_to_cost_limit_ms();
        assert!(estimate.is_some());
        assert_eq!(estimate.unwrap(), 9000);
    }

    #[test]
    fn test_history_usage_summary() {
        let mut history = ResourceUsageHistory::new(10);
        history.add(
            ResourceUsage::new()
                .with_tokens_used(5000)
                .with_cost_usd(0.25)
                .with_api_calls(10),
        );

        let summary = history.usage_summary();
        assert!(summary.contains("Tokens: 5000"));
        assert!(summary.contains("Cost: $0.2500"));
        assert!(summary.contains("API calls: 10"));
        assert!(summary.contains("Status: OK"));
    }

    #[test]
    fn test_history_usage_summary_empty() {
        let history = ResourceUsageHistory::new(10);
        let summary = history.usage_summary();
        assert!(summary.contains("No usage data available"));
    }

    #[test]
    fn test_history_json_serialization() {
        let mut history = ResourceUsageHistory::new(5).with_thread_id("json-test");
        history.add(ResourceUsage::new().with_tokens_used(100));
        history.add(ResourceUsage::new().with_tokens_used(200));

        let json = history.to_json().expect("JSON serialization failed");
        let parsed = ResourceUsageHistory::from_json(&json).expect("JSON parsing failed");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.max_snapshots, 5);
        assert_eq!(parsed.thread_id.as_deref(), Some("json-test"));
        assert_eq!(parsed.snapshots[0].tokens_used, 100);
        assert_eq!(parsed.snapshots[1].tokens_used, 200);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_usage_at_exact_budget() {
        let usage = ResourceUsage::new()
            .with_tokens_used(10000)
            .with_tokens_budget(10000);

        assert!(!usage.is_over_token_budget()); // At budget, not over
        assert!(usage.is_near_token_limit(1.0)); // At 100%
        assert_eq!(usage.remaining_tokens(), 0);
    }

    #[test]
    fn test_very_small_cost_values() {
        let usage = ResourceUsage::new()
            .with_cost_usd(0.00001)
            .with_cost_budget_usd(0.001);

        assert!((usage.remaining_cost_usd() - 0.00099).abs() < 0.00001);
        assert!((usage.cost_usage_percentage() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_large_values() {
        let usage = ResourceUsage::new()
            .with_tokens_used(u64::MAX - 1)
            .with_tokens_budget(u64::MAX);

        assert_eq!(usage.remaining_tokens(), 1);
        assert!(!usage.is_over_token_budget());
    }
}
