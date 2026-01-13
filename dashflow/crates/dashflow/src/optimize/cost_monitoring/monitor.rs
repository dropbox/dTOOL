// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Cost monitoring and usage tracking

// Allow internal use of deprecated types within this deprecated module
#![allow(deprecated)]

use crate::optimize::cost_monitoring::error::Result;
use crate::optimize::cost_monitoring::pricing::{ModelPricing, TokenUsage};
use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single usage record
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::CostRecord` instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Timestamp
    pub timestamp: DateTime<Local>,
    /// Model name
    pub model: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Cost in USD
    pub cost: f64,
}

/// Cost report summarizing usage
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::CostReport` instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// Total spent today (USD)
    pub spent_today: f64,
    /// Total spent this month (USD)
    pub spent_month: f64,
    /// Total all-time (USD)
    pub spent_total: f64,
    /// Daily budget limit (USD)
    pub daily_limit: Option<f64>,
    /// Monthly budget limit (USD)
    pub monthly_limit: Option<f64>,
    /// Percentage of daily budget used
    pub daily_usage_percent: Option<f64>,
    /// Percentage of monthly budget used
    pub monthly_usage_percent: Option<f64>,
    /// Total requests tracked
    pub total_requests: usize,
    /// Average cost per request
    pub avg_cost_per_request: f64,
    /// Breakdown by model
    pub by_model: HashMap<String, f64>,
}

/// Internal state for cost monitor
struct MonitorState {
    pricing: ModelPricing,
    records: Vec<UsageRecord>,
    daily_budget: Option<f64>,
    monthly_budget: Option<f64>,
    alert_threshold: f64,
    alert_callback: Option<Box<dyn Fn(f64, f64) + Send + Sync>>,
}

impl std::fmt::Debug for MonitorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorState")
            .field("pricing", &self.pricing)
            .field("records", &self.records)
            .field("daily_budget", &self.daily_budget)
            .field("monthly_budget", &self.monthly_budget)
            .field("alert_threshold", &self.alert_threshold)
            .field(
                "alert_callback",
                &self.alert_callback.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

/// Cost monitor for tracking LLM usage and costs
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::CostTracker` instead"
)]
#[derive(Clone)]
pub struct CostMonitor {
    state: Arc<Mutex<MonitorState>>,
}

impl CostMonitor {
    /// Create a new cost monitor
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MonitorState {
                pricing: ModelPricing::new(),
                records: Vec::new(),
                daily_budget: None,
                monthly_budget: None,
                alert_threshold: 0.9,
                alert_callback: None,
            })),
        }
    }

    /// Set daily budget limit
    #[must_use]
    pub fn with_daily_budget(self, budget: f64) -> Self {
        if let Ok(mut state) = self.state.lock() {
            state.daily_budget = Some(budget);
        }
        self
    }

    /// Set monthly budget limit
    #[must_use]
    pub fn with_monthly_budget(self, budget: f64) -> Self {
        if let Ok(mut state) = self.state.lock() {
            state.monthly_budget = Some(budget);
        }
        self
    }

    /// Set alert threshold (0.0-1.0, default 0.9)
    #[must_use]
    pub fn with_alert_threshold(self, threshold: f64) -> Self {
        if let Ok(mut state) = self.state.lock() {
            state.alert_threshold = threshold;
        }
        self
    }

    /// Set alert callback function
    #[must_use]
    pub fn with_alert_callback<F>(self, callback: F) -> Self
    where
        F: Fn(f64, f64) + Send + Sync + 'static,
    {
        if let Ok(mut state) = self.state.lock() {
            state.alert_callback = Some(Box::new(callback));
        }
        self
    }

    /// Record usage for a model
    pub fn record_usage(&self, model: &str, input_tokens: u64, output_tokens: u64) -> Result<f64> {
        let mut state = self.state.lock().map_err(|e| {
            crate::optimize::cost_monitoring::error::CostMonitorError::LockPoisoned(format!("{e}"))
        })?;

        let usage = TokenUsage::new(input_tokens, output_tokens);
        let cost = state.pricing.calculate_cost(model, usage)?;

        let record = UsageRecord {
            timestamp: Local::now(),
            model: model.to_string(),
            usage,
            cost,
        };

        state.records.push(record);

        // Check if we should trigger alert
        if let Some(daily_budget) = state.daily_budget {
            let spent_today = self.calculate_spent_today(&state.records);
            let usage_percent = spent_today / daily_budget;

            if usage_percent >= state.alert_threshold {
                if let Some(callback) = &state.alert_callback {
                    callback(spent_today, daily_budget);
                }
            }
        }

        Ok(cost)
    }

    /// Generate cost report
    pub fn report(&self) -> CostReport {
        let state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => {
                // If lock is poisoned, return empty report
                return CostReport {
                    spent_today: 0.0,
                    spent_month: 0.0,
                    spent_total: 0.0,
                    daily_limit: None,
                    monthly_limit: None,
                    daily_usage_percent: None,
                    monthly_usage_percent: None,
                    total_requests: 0,
                    avg_cost_per_request: 0.0,
                    by_model: HashMap::new(),
                };
            }
        };

        let spent_today = self.calculate_spent_today(&state.records);
        let spent_month = self.calculate_spent_month(&state.records);
        let spent_total: f64 = state.records.iter().map(|r| r.cost).sum();

        let daily_usage_percent = state
            .daily_budget
            .map(|budget| (spent_today / budget) * 100.0);

        let monthly_usage_percent = state
            .monthly_budget
            .map(|budget| (spent_month / budget) * 100.0);

        let total_requests = state.records.len();
        let avg_cost_per_request = if total_requests > 0 {
            spent_total / total_requests as f64
        } else {
            0.0
        };

        let mut by_model: HashMap<String, f64> = HashMap::new();
        for record in &state.records {
            *by_model.entry(record.model.clone()).or_insert(0.0) += record.cost;
        }

        CostReport {
            spent_today,
            spent_month,
            spent_total,
            daily_limit: state.daily_budget,
            monthly_limit: state.monthly_budget,
            daily_usage_percent,
            monthly_usage_percent,
            total_requests,
            avg_cost_per_request,
            by_model,
        }
    }

    /// Get all usage records
    pub fn get_records(&self) -> Vec<UsageRecord> {
        self.state
            .lock()
            .map(|state| state.records.clone())
            .unwrap_or_default()
    }

    /// Clear all records
    pub fn clear_records(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.records.clear();
        }
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let report = self.report();

        let mut output = String::new();
        output.push_str("# HELP llm_cost_total Total cost in USD\n");
        output.push_str("# TYPE llm_cost_total counter\n");
        output.push_str(&format!("llm_cost_total {}\n", report.spent_total));

        output.push_str("# HELP llm_cost_today Cost today in USD\n");
        output.push_str("# TYPE llm_cost_today gauge\n");
        output.push_str(&format!("llm_cost_today {}\n", report.spent_today));

        output.push_str("# HELP llm_requests_total Total number of requests\n");
        output.push_str("# TYPE llm_requests_total counter\n");
        output.push_str(&format!("llm_requests_total {}\n", report.total_requests));

        output.push_str("# HELP llm_cost_per_request Average cost per request\n");
        output.push_str("# TYPE llm_cost_per_request gauge\n");
        output.push_str(&format!(
            "llm_cost_per_request {}\n",
            report.avg_cost_per_request
        ));

        for (model, cost) in &report.by_model {
            output.push_str(&format!(
                "llm_cost_by_model{{model=\"{}\"}} {}\n",
                model, cost
            ));
        }

        output
    }

    fn calculate_spent_today(&self, records: &[UsageRecord]) -> f64 {
        let today = Local::now().date_naive();
        records
            .iter()
            .filter(|r| r.timestamp.date_naive() == today)
            .map(|r| r.cost)
            .sum()
    }

    fn calculate_spent_month(&self, records: &[UsageRecord]) -> f64 {
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

impl Default for CostMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_monitor_creation() {
        let monitor = CostMonitor::new();
        let report = monitor.report();

        assert_eq!(report.spent_total, 0.0);
        assert_eq!(report.total_requests, 0);
    }

    #[test]
    fn test_record_usage() {
        let monitor = CostMonitor::new();

        let cost = monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        assert!(cost > 0.0);

        let report = monitor.report();
        assert_eq!(report.total_requests, 1);
        assert_eq!(report.spent_total, cost);
    }

    #[test]
    fn test_multiple_records() {
        let monitor = CostMonitor::new();

        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        monitor.record_usage("gpt-4o-mini", 2000, 1000).unwrap();
        monitor.record_usage("gpt-3.5-turbo", 1500, 750).unwrap();

        let report = monitor.report();
        assert_eq!(report.total_requests, 3);
        assert!(report.spent_total > 0.0);
        assert_eq!(report.by_model.len(), 2);
    }

    #[test]
    fn test_budget_tracking() {
        let monitor = CostMonitor::new().with_daily_budget(10.0);

        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();

        let report = monitor.report();
        assert_eq!(report.daily_limit, Some(10.0));
        assert!(report.daily_usage_percent.is_some());
        assert!(report.daily_usage_percent.unwrap() < 1.0);
    }

    #[test]
    fn test_alert_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let alert_fired = Arc::new(AtomicBool::new(false));
        let alert_fired_clone = alert_fired.clone();

        let monitor = CostMonitor::new()
            .with_daily_budget(0.004) // Very low budget to trigger alert
            .with_alert_threshold(0.5)
            .with_alert_callback(move |spent, limit| {
                assert!(spent >= limit * 0.5);
                alert_fired_clone.store(true, Ordering::SeqCst);
            });

        // This should trigger the alert (~$0.0045 spent, $0.004 limit = 112.5%)
        monitor.record_usage("gpt-4o-mini", 10000, 5000).unwrap();

        assert!(alert_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn test_clear_records() {
        let monitor = CostMonitor::new();

        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        assert_eq!(monitor.report().total_requests, 1);

        monitor.clear_records();
        assert_eq!(monitor.report().total_requests, 0);
    }

    #[test]
    fn test_avg_cost_per_request() {
        let monitor = CostMonitor::new();

        let cost1 = monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        let cost2 = monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();

        let report = monitor.report();
        let expected_avg = (cost1 + cost2) / 2.0;
        assert!((report.avg_cost_per_request - expected_avg).abs() < 0.0001);
    }

    #[test]
    fn test_by_model_breakdown() {
        let monitor = CostMonitor::new();

        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();
        monitor.record_usage("gpt-3.5-turbo", 2000, 1000).unwrap();

        let report = monitor.report();
        assert_eq!(report.by_model.len(), 2);
        assert!(report.by_model.contains_key("gpt-4o-mini"));
        assert!(report.by_model.contains_key("gpt-3.5-turbo"));
    }

    #[test]
    fn test_prometheus_export() {
        let monitor = CostMonitor::new();

        monitor.record_usage("gpt-4o-mini", 1000, 500).unwrap();

        let metrics = monitor.export_prometheus();
        assert!(metrics.contains("llm_cost_total"));
        assert!(metrics.contains("llm_requests_total"));
        assert!(metrics.contains("llm_cost_by_model"));
    }
}
