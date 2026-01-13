// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for budget enforcement
// - deprecated: Internal use of deprecated types within this deprecated module
// - expect_used: validation failure is exposed via try_new()
#![allow(deprecated, clippy::expect_used)]

//! Budget enforcement and alerting

use crate::optimize::cost_monitoring::error::{CostMonitorError, Result};
use crate::optimize::cost_monitoring::monitor::CostMonitor;
use serde::{Deserialize, Serialize};

/// Alert severity level
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::AlertLevel` instead"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    /// Warning threshold reached (default 90%)
    Warning,
    /// Critical threshold reached (default 100%)
    Critical,
}

/// Budget configuration
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::BudgetConfig` instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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
            return Err(CostMonitorError::InvalidConfig(format!(
                "warning_threshold must be 0.0-1.0, got {}",
                self.warning_threshold
            )));
        }
        if !(0.0..=1.0).contains(&self.critical_threshold) {
            return Err(CostMonitorError::InvalidConfig(format!(
                "critical_threshold must be 0.0-1.0, got {}",
                self.critical_threshold
            )));
        }
        if self.warning_threshold > self.critical_threshold {
            return Err(CostMonitorError::InvalidConfig(format!(
                "warning_threshold ({}) must not exceed critical_threshold ({})",
                self.warning_threshold, self.critical_threshold
            )));
        }
        if let Some(limit) = self.daily_limit {
            if limit < 0.0 {
                return Err(CostMonitorError::InvalidConfig(format!(
                    "daily_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.monthly_limit {
            if limit < 0.0 {
                return Err(CostMonitorError::InvalidConfig(format!(
                    "monthly_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.per_request_limit {
            if limit < 0.0 {
                return Err(CostMonitorError::InvalidConfig(format!(
                    "per_request_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        if let Some(limit) = self.total_limit {
            if limit < 0.0 {
                return Err(CostMonitorError::InvalidConfig(format!(
                    "total_limit must be non-negative, got {}",
                    limit
                )));
            }
        }
        Ok(())
    }

    /// Create new budget config with daily limit
    #[must_use]
    pub fn with_daily_limit(limit: f64) -> Self {
        Self {
            daily_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with monthly limit
    #[must_use]
    pub fn with_monthly_limit(limit: f64) -> Self {
        Self {
            monthly_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with per-request limit
    #[must_use]
    pub fn with_per_request_limit(limit: f64) -> Self {
        Self {
            per_request_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Create new budget config with total limit
    #[must_use]
    pub fn with_total_limit(limit: f64) -> Self {
        Self {
            total_limit: Some(limit),
            ..Default::default()
        }
    }

    /// Set warning threshold
    pub fn warning_threshold(mut self, threshold: f64) -> Self {
        self.warning_threshold = threshold;
        self
    }

    /// Set critical threshold
    pub fn critical_threshold(mut self, threshold: f64) -> Self {
        self.critical_threshold = threshold;
        self
    }

    /// Enable hard limit enforcement
    pub fn enforce_hard_limit(mut self, enforce: bool) -> Self {
        self.enforce_hard_limit = enforce;
        self
    }
}

/// Budget enforcer with threshold alerts
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::BudgetEnforcer` instead"
)]
pub struct BudgetEnforcer {
    monitor: CostMonitor,
    config: BudgetConfig,
}

impl BudgetEnforcer {
    /// Create new budget enforcer.
    ///
    /// # Panics
    ///
    /// Panics if configuration validation fails. Use `try_new()` for fallible construction.
    pub fn new(monitor: CostMonitor, config: BudgetConfig) -> Self {
        Self::try_new(monitor, config)
            .expect("Invalid BudgetConfig (use try_new() for fallible construction)")
    }

    /// Create new budget enforcer with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails.
    pub fn try_new(mut monitor: CostMonitor, config: BudgetConfig) -> Result<Self> {
        config.validate()?;
        // Apply budget config to monitor
        if let Some(daily_limit) = config.daily_limit {
            monitor = monitor.with_daily_budget(daily_limit);
        }
        if let Some(monthly_limit) = config.monthly_limit {
            monitor = monitor.with_monthly_budget(monthly_limit);
        }
        Ok(Self { monitor, config })
    }

    /// Check if usage is within budget
    pub fn check_budget(&self) -> Result<()> {
        let report = self.monitor.report();

        // Check total budget
        if let Some(total_limit) = self.config.total_limit {
            let usage_percent = report.spent_total / total_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(CostMonitorError::BudgetExceeded {
                    spent: report.spent_total,
                    limit: total_limit,
                });
            }
        }

        // Check daily budget
        if let Some(daily_limit) = self.config.daily_limit {
            let usage_percent = report.spent_today / daily_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(CostMonitorError::BudgetExceeded {
                    spent: report.spent_today,
                    limit: daily_limit,
                });
            }
        }

        // Check monthly budget
        if let Some(monthly_limit) = self.config.monthly_limit {
            let usage_percent = report.spent_month / monthly_limit;

            if self.config.enforce_hard_limit && usage_percent >= self.config.critical_threshold {
                return Err(CostMonitorError::BudgetExceeded {
                    spent: report.spent_month,
                    limit: monthly_limit,
                });
            }
        }

        Ok(())
    }

    /// Get current alert level, if any
    pub fn alert_level(&self) -> Option<AlertLevel> {
        let report = self.monitor.report();

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

    /// Record usage and check budget
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
            .monitor
            .record_usage(model, input_tokens, output_tokens)?;

        // Check per-request limit (after recording to know actual cost)
        if let Some(per_request_limit) = self.config.per_request_limit {
            if self.config.enforce_hard_limit && cost > per_request_limit {
                return Err(CostMonitorError::BudgetExceeded {
                    spent: cost,
                    limit: per_request_limit,
                });
            }
        }

        Ok(cost)
    }

    /// Get the underlying monitor
    pub fn monitor(&self) -> &CostMonitor {
        &self.monitor
    }

    /// Get budget config
    pub fn config(&self) -> &BudgetConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_config_defaults() {
        let config = BudgetConfig::default();
        assert_eq!(config.daily_limit, None);
        assert_eq!(config.monthly_limit, None);
        assert_eq!(config.warning_threshold, 0.9);
        assert_eq!(config.critical_threshold, 1.0);
        assert!(!config.enforce_hard_limit);
    }

    #[test]
    fn test_budget_config_builders() {
        let config = BudgetConfig::with_daily_limit(100.0)
            .warning_threshold(0.8)
            .enforce_hard_limit(true);

        assert_eq!(config.daily_limit, Some(100.0));
        assert_eq!(config.warning_threshold, 0.8);
        assert!(config.enforce_hard_limit);
    }

    #[test]
    fn test_budget_enforcer_within_limit() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::with_daily_limit(10.0);
        let enforcer = BudgetEnforcer::new(monitor, config);

        // Small usage should be ok
        let result = enforcer.record_and_check("gpt-4o-mini", 1000, 500);
        assert!(result.is_ok());
        assert_eq!(enforcer.alert_level(), None);
    }

    #[test]
    fn test_budget_enforcer_warning() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::with_daily_limit(0.01) // Very low limit
            .warning_threshold(0.1); // Trigger at 10%

        let enforcer = BudgetEnforcer::new(monitor, config);

        // This usage should trigger warning
        enforcer
            .record_and_check("gpt-4o-mini", 5000, 2500)
            .unwrap();

        assert_eq!(enforcer.alert_level(), Some(AlertLevel::Warning));
    }

    #[test]
    fn test_budget_enforcer_hard_limit() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::with_daily_limit(0.002) // Very low limit
            .warning_threshold(0.4) // Trigger warning at 40%
            .critical_threshold(0.5) // Trigger at 50%
            .enforce_hard_limit(true);

        let enforcer = BudgetEnforcer::new(monitor, config);

        // First request should work (costs ~$0.00135, 67.5% of budget)
        enforcer
            .record_and_check("gpt-4o-mini", 3000, 1500)
            .unwrap();

        // Second request should fail (would exceed 50% threshold on check)
        let result = enforcer.record_and_check("gpt-4o-mini", 3000, 1500);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CostMonitorError::BudgetExceeded { .. }
        ));
    }

    #[test]
    fn test_budget_enforcer_soft_limit() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::with_daily_limit(0.002) // Very low limit
            .warning_threshold(0.4) // Trigger warning at 40%
            .critical_threshold(0.5) // Trigger at 50%
            .enforce_hard_limit(false); // Soft limit

        let enforcer = BudgetEnforcer::new(monitor, config);

        // Both requests should work (soft limit doesn't block)
        // First request: ~$0.00135 (67.5% of budget)
        enforcer
            .record_and_check("gpt-4o-mini", 3000, 1500)
            .unwrap();
        // Second request: total ~$0.0027 (135% of budget) - exceeds critical threshold
        enforcer
            .record_and_check("gpt-4o-mini", 3000, 1500)
            .unwrap();

        // But should show critical alert
        assert_eq!(enforcer.alert_level(), Some(AlertLevel::Critical));
    }

    #[test]
    fn test_monthly_budget() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::with_monthly_limit(100.0);
        let enforcer = BudgetEnforcer::new(monitor, config);

        enforcer.record_and_check("gpt-4o-mini", 1000, 500).unwrap();

        let report = enforcer.monitor().report();
        assert_eq!(report.monthly_limit, Some(100.0));
        assert!(report.spent_month > 0.0);
    }

    // ------------------------------------------------------------------------
    // BudgetConfig Validation Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_budget_config_validate_valid() {
        let config = BudgetConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_budget_config_validate_warning_threshold_below_zero() {
        let config = BudgetConfig {
            warning_threshold: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CostMonitorError::InvalidConfig(_)
        ));
    }

    #[test]
    fn test_budget_config_validate_warning_threshold_above_one() {
        let config = BudgetConfig {
            warning_threshold: 1.5,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_critical_threshold_below_zero() {
        let config = BudgetConfig {
            critical_threshold: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_critical_threshold_above_one() {
        let config = BudgetConfig {
            critical_threshold: 1.5,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_warning_exceeds_critical() {
        let config = BudgetConfig {
            warning_threshold: 0.95,
            critical_threshold: 0.9,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_negative_daily_limit() {
        let config = BudgetConfig {
            daily_limit: Some(-10.0),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_negative_monthly_limit() {
        let config = BudgetConfig {
            monthly_limit: Some(-100.0),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_negative_per_request_limit() {
        let config = BudgetConfig {
            per_request_limit: Some(-1.0),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_negative_total_limit() {
        let config = BudgetConfig {
            total_limit: Some(-1000.0),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_config_validate_boundary_values() {
        // warning_threshold = 0.0 is valid
        let config = BudgetConfig {
            warning_threshold: 0.0,
            critical_threshold: 0.5,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // warning_threshold = critical_threshold is valid (equal is ok)
        let config = BudgetConfig {
            warning_threshold: 0.9,
            critical_threshold: 0.9,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // critical_threshold = 1.0 is valid
        let config = BudgetConfig {
            critical_threshold: 1.0,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // Zero limit is valid (means no budget)
        let config = BudgetConfig {
            daily_limit: Some(0.0),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_budget_enforcer_try_new_valid() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig::default();
        let result = BudgetEnforcer::try_new(monitor, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_budget_enforcer_try_new_invalid() {
        let monitor = CostMonitor::new();
        let config = BudgetConfig {
            warning_threshold: 2.0, // Invalid
            ..Default::default()
        };
        let result = BudgetEnforcer::try_new(monitor, config);
        assert!(result.is_err());
    }

    // Note: The `new()` panicking behavior is covered by `test_budget_enforcer_try_new_invalid()`
    // which tests the same validation via the Result-returning `try_new()`.
}
