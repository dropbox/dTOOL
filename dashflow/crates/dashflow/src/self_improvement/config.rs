// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Unified Configuration for Self-Improvement System.
//!
//! This module provides a centralized configuration struct that aggregates
//! all self-improvement related configurations:
//!
//! - **Daemon**: Analysis interval, thresholds, metrics source
//! - **Storage**: Retention limits, cleanup policies
//! - **Traces**: Retention, compression, cleanup
//! - **Health**: Check intervals, thresholds
//!
//! # Environment Variable Loading
//!
//! All configuration can be loaded from environment variables:
//!
//! ```rust,ignore
//! use dashflow::self_improvement::SelfImprovementConfig;
//!
//! let config = SelfImprovementConfig::from_env();
//! ```
//!
//! # Configuration Validation
//!
//! Configuration is validated on construction to catch invalid values early:
//!
//! ```rust,ignore
//! let config = SelfImprovementConfig::from_env();
//! if let Err(errors) = config.validate() {
//!     for error in errors {
//!         eprintln!("Config error: {}", error);
//!     }
//! }
//! ```

use super::daemon::{DaemonConfig, MetricsSource};
use super::health::HealthCheckConfig;
use super::storage::StoragePolicy;
use super::trace_retention::RetentionPolicy;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Unified configuration for the self-improvement system.
///
/// Aggregates all configuration options for:
/// - Analysis daemon (intervals, thresholds, metrics source)
/// - Storage (retention limits, cleanup policies)
/// - Trace retention (max count, age, compression)
/// - Health checks (paths, thresholds)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::SelfImprovementConfig;
///
/// // Load from environment variables
/// let config = SelfImprovementConfig::from_env();
///
/// // Or build programmatically
/// let config = SelfImprovementConfig::builder()
///     .with_interval_secs(30)
///     .with_slow_node_threshold_ms(5000)
///     .build();
///
/// // Validate before use
/// config.validate()?;
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SelfImprovementConfig {
    /// Daemon configuration
    pub daemon: DaemonConfig,
    /// Storage retention policy
    pub storage_policy: StoragePolicy,
    /// Trace retention policy
    pub trace_retention: RetentionPolicy,
    /// Health check configuration
    pub health: HealthCheckConfig,
}

impl SelfImprovementConfig {
    /// Create configuration from environment variables.
    ///
    /// This loads all sub-configurations from their respective environment
    /// variables. See each config type's documentation for variable names.
    ///
    /// # Environment Variable Prefixes
    ///
    /// | Prefix | Config |
    /// |--------|--------|
    /// | `DASHFLOW_SELF_IMPROVE_*` | DaemonConfig |
    /// | `DASHFLOW_STORAGE_*` | StoragePolicy |
    /// | `DASHFLOW_TRACE_*` | RetentionPolicy |
    /// | `DASHFLOW_HEALTH_*` | HealthCheckConfig |
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            daemon: DaemonConfig::from_env(),
            storage_policy: StoragePolicy::from_env(),
            trace_retention: RetentionPolicy::from_env(),
            health: HealthCheckConfig::from_env(),
        }
    }

    /// Create a builder for programmatic configuration.
    #[must_use]
    pub fn builder() -> SelfImprovementConfigBuilder {
        SelfImprovementConfigBuilder::new()
    }

    /// Validate all configuration values.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - Intervals must be > 0
    /// - Thresholds must be in valid ranges (0.0-1.0 for rates)
    /// - Paths must be valid (not empty)
    /// - Sizes must be > 0
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SelfImprovementConfig::from_env();
    /// match config.validate() {
    ///     Ok(()) => println!("Configuration is valid"),
    ///     Err(errors) => {
    ///         for error in &errors {
    ///             eprintln!("Config error: {}", error);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn validate(&self) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        // Validate daemon config
        self.validate_daemon(&mut errors);

        // Validate storage policy
        self.validate_storage(&mut errors);

        // Validate trace retention
        self.validate_trace_retention(&mut errors);

        // Validate health config
        self.validate_health(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_daemon(&self, errors: &mut Vec<ConfigValidationError>) {
        // Interval must be > 0
        if self.daemon.interval.as_secs() == 0 {
            errors.push(ConfigValidationError {
                field: "daemon.interval".to_string(),
                message: "Interval must be greater than 0 seconds".to_string(),
                suggestion: Some("Set DASHFLOW_SELF_IMPROVE_INTERVAL to at least 1".to_string()),
            });
        }

        // Error rate threshold must be 0.0-1.0
        if !(0.0..=1.0).contains(&self.daemon.error_rate_threshold) {
            errors.push(ConfigValidationError {
                field: "daemon.error_rate_threshold".to_string(),
                message: format!(
                    "Error rate threshold {} must be between 0.0 and 1.0",
                    self.daemon.error_rate_threshold
                ),
                suggestion: Some(
                    "Set DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD to a value between 0 and 1"
                        .to_string(),
                ),
            });
        }

        // Slow node threshold must be > 0
        if self.daemon.slow_node_threshold_ms == 0 {
            errors.push(ConfigValidationError {
                field: "daemon.slow_node_threshold_ms".to_string(),
                message: "Slow node threshold must be greater than 0".to_string(),
                suggestion: Some(
                    "Set DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS to at least 1".to_string(),
                ),
            });
        }

        // Traces dir path should not be empty
        if self.daemon.traces_dir.as_os_str().is_empty() {
            errors.push(ConfigValidationError {
                field: "daemon.traces_dir".to_string(),
                message: "Traces directory path cannot be empty".to_string(),
                suggestion: Some(
                    "Set DASHFLOW_SELF_IMPROVE_TRACES_DIR to a valid path".to_string(),
                ),
            });
        }

        // Validate Prometheus URL if metrics source is HTTP
        if matches!(self.daemon.metrics_source, MetricsSource::Http) {
            if let Some(ref url) = self.daemon.prometheus_endpoint {
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    errors.push(ConfigValidationError {
                        field: "daemon.prometheus_endpoint".to_string(),
                        message: format!(
                            "Prometheus URL '{}' must start with http:// or https://",
                            url
                        ),
                        suggestion: Some(
                            "Set DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL to a valid HTTP URL"
                                .to_string(),
                        ),
                    });
                }
            }
        }
    }

    fn validate_storage(&self, errors: &mut Vec<ConfigValidationError>) {
        // Max reports must be > 0 if set
        if let Some(max_reports) = self.storage_policy.max_reports {
            if max_reports == 0 {
                errors.push(ConfigValidationError {
                    field: "storage_policy.max_reports".to_string(),
                    message: "Max reports must be greater than 0".to_string(),
                    suggestion: Some("Set DASHFLOW_STORAGE_MAX_REPORTS to at least 1".to_string()),
                });
            }
        }

        // Max plans per status must be > 0 if set
        if let Some(max_plans) = self.storage_policy.max_plans_per_status {
            if max_plans == 0 {
                errors.push(ConfigValidationError {
                    field: "storage_policy.max_plans_per_status".to_string(),
                    message: "Max plans per status must be greater than 0".to_string(),
                    suggestion: Some("Set DASHFLOW_STORAGE_MAX_PLANS to at least 1".to_string()),
                });
            }
        }
    }

    fn validate_trace_retention(&self, errors: &mut Vec<ConfigValidationError>) {
        // Max traces must be > 0 if set
        if let Some(max_traces) = self.trace_retention.max_traces {
            if max_traces == 0 {
                errors.push(ConfigValidationError {
                    field: "trace_retention.max_traces".to_string(),
                    message: "Max traces must be greater than 0".to_string(),
                    suggestion: Some("Set DASHFLOW_TRACE_MAX_COUNT to at least 1".to_string()),
                });
            }
        }

        // Max size must be > 0 if set
        if let Some(max_size) = self.trace_retention.max_size_bytes {
            if max_size == 0 {
                errors.push(ConfigValidationError {
                    field: "trace_retention.max_size_bytes".to_string(),
                    message: "Max trace size must be greater than 0".to_string(),
                    suggestion: Some("Set DASHFLOW_TRACE_MAX_SIZE_MB to at least 1".to_string()),
                });
            }
        }
    }

    fn validate_health(&self, errors: &mut Vec<ConfigValidationError>) {
        // Max storage size must be > 0
        if self.health.max_storage_size == 0 {
            errors.push(ConfigValidationError {
                field: "health.max_storage_size".to_string(),
                message: "Max storage size must be greater than 0".to_string(),
                suggestion: Some("Set DASHFLOW_HEALTH_MAX_STORAGE_MB to at least 1".to_string()),
            });
        }

        // Max trace count must be > 0
        if self.health.max_trace_count == 0 {
            errors.push(ConfigValidationError {
                field: "health.max_trace_count".to_string(),
                message: "Max trace count must be greater than 0".to_string(),
                suggestion: Some("Set DASHFLOW_HEALTH_MAX_TRACES to at least 1".to_string()),
            });
        }
    }

    /// Get the daemon configuration.
    #[must_use]
    pub fn daemon(&self) -> &DaemonConfig {
        &self.daemon
    }

    /// Get the storage policy.
    #[must_use]
    pub fn storage_policy(&self) -> &StoragePolicy {
        &self.storage_policy
    }

    /// Get the trace retention policy.
    #[must_use]
    pub fn trace_retention(&self) -> &RetentionPolicy {
        &self.trace_retention
    }

    /// Get the health check configuration.
    #[must_use]
    pub fn health(&self) -> &HealthCheckConfig {
        &self.health
    }
}

/// Configuration validation error with helpful message.
#[derive(Debug, Clone, Error)]
#[error("{field}: {message}{}", suggestion.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default())]
pub struct ConfigValidationError {
    /// The field that failed validation
    pub field: String,
    /// Human-readable error message
    pub message: String,
    /// Optional suggestion for how to fix
    pub suggestion: Option<String>,
}

/// Builder for SelfImprovementConfig.
#[derive(Debug, Clone)]
pub struct SelfImprovementConfigBuilder {
    config: SelfImprovementConfig,
}

impl SelfImprovementConfigBuilder {
    /// Create a new builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SelfImprovementConfig::default(),
        }
    }

    /// Set the analysis interval in seconds.
    #[must_use]
    pub fn with_interval_secs(mut self, seconds: u64) -> Self {
        self.config.daemon.interval = Duration::from_secs(seconds);
        self
    }

    /// Set the traces directory.
    #[must_use]
    pub fn with_traces_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.daemon.traces_dir = path.into();
        self
    }

    /// Set the Prometheus endpoint.
    #[must_use]
    pub fn with_prometheus_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.daemon.prometheus_endpoint = Some(endpoint.into());
        self
    }

    /// Set the metrics source.
    #[must_use]
    pub fn with_metrics_source(mut self, source: MetricsSource) -> Self {
        self.config.daemon.metrics_source = source;
        self
    }

    /// Use in-process metrics instead of Prometheus HTTP.
    #[must_use]
    pub fn with_in_process_metrics(mut self) -> Self {
        self.config.daemon.metrics_source = MetricsSource::InProcess;
        self
    }

    /// Set the slow node threshold in milliseconds.
    #[must_use]
    pub fn with_slow_node_threshold_ms(mut self, ms: u64) -> Self {
        self.config.daemon.slow_node_threshold_ms = ms;
        self
    }

    /// Set the error rate threshold (0.0-1.0).
    #[must_use]
    pub fn with_error_rate_threshold(mut self, threshold: f64) -> Self {
        self.config.daemon.error_rate_threshold = threshold;
        self
    }

    /// Set the retry threshold.
    #[must_use]
    pub fn with_retry_threshold(mut self, threshold: usize) -> Self {
        self.config.daemon.retry_threshold = threshold;
        self
    }

    /// Set the minimum traces required for analysis.
    #[must_use]
    pub fn with_min_traces(mut self, count: usize) -> Self {
        self.config.daemon.min_traces_for_analysis = count;
        self
    }

    /// Enable or disable automatic cleanup.
    #[must_use]
    pub fn with_cleanup_enabled(mut self, enabled: bool) -> Self {
        self.config.daemon.cleanup_enabled = enabled;
        self
    }

    /// Set the cleanup interval in cycles.
    #[must_use]
    pub fn with_cleanup_interval_cycles(mut self, cycles: usize) -> Self {
        self.config.daemon.cleanup_interval_cycles = cycles;
        self
    }

    /// Set the maximum number of traces to retain.
    #[must_use]
    pub fn with_max_traces(mut self, count: usize) -> Self {
        self.config.trace_retention.max_traces = Some(count);
        self
    }

    /// Set the maximum trace age.
    #[must_use]
    pub fn with_max_trace_age(mut self, duration: Duration) -> Self {
        self.config.trace_retention.max_age = Some(duration);
        self
    }

    /// Set the maximum trace storage size in bytes.
    #[must_use]
    pub fn with_max_trace_size_bytes(mut self, bytes: u64) -> Self {
        self.config.trace_retention.max_size_bytes = Some(bytes);
        self
    }

    /// Set the maximum number of reports to retain.
    #[must_use]
    pub fn with_max_reports(mut self, count: usize) -> Self {
        self.config.storage_policy.max_reports = Some(count);
        self
    }

    /// Set the maximum plans per status directory.
    #[must_use]
    pub fn with_max_plans_per_status(mut self, count: usize) -> Self {
        self.config.storage_policy.max_plans_per_status = Some(count);
        self
    }

    /// Set the storage directory path.
    #[must_use]
    pub fn with_storage_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.health.storage_path = path.into();
        self
    }

    /// Set the health check maximum storage size in bytes.
    #[must_use]
    pub fn with_health_max_storage_size(mut self, bytes: u64) -> Self {
        self.config.health.max_storage_size = bytes;
        self
    }

    /// Set the health check maximum trace count.
    #[must_use]
    pub fn with_health_max_trace_count(mut self, count: usize) -> Self {
        self.config.health.max_trace_count = count;
        self
    }

    /// Build the configuration, consuming the builder.
    #[must_use]
    pub fn build(self) -> SelfImprovementConfig {
        self.config
    }

    /// Build and validate the configuration.
    ///
    /// Returns the configuration if valid, or validation errors if not.
    pub fn build_and_validate(self) -> Result<SelfImprovementConfig, Vec<ConfigValidationError>> {
        let config = self.config;
        config.validate()?;
        Ok(config)
    }
}

impl Default for SelfImprovementConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SelfImprovementConfig::default();
        assert_eq!(config.daemon.interval.as_secs(), 60);
        assert_eq!(config.daemon.slow_node_threshold_ms, 10_000);
        assert!((config.daemon.error_rate_threshold - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder() {
        let config = SelfImprovementConfig::builder()
            .with_interval_secs(30)
            .with_slow_node_threshold_ms(5000)
            .with_error_rate_threshold(0.10)
            .with_max_traces(500)
            .build();

        assert_eq!(config.daemon.interval.as_secs(), 30);
        assert_eq!(config.daemon.slow_node_threshold_ms, 5000);
        assert!((config.daemon.error_rate_threshold - 0.10).abs() < f64::EPSILON);
        assert_eq!(config.trace_retention.max_traces, Some(500));
    }

    #[test]
    fn test_validation_passes_for_defaults() {
        let config = SelfImprovementConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_fails_for_zero_interval() {
        let mut config = SelfImprovementConfig::default();
        config.daemon.interval = Duration::ZERO;

        let result = config.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "daemon.interval"));
    }

    #[test]
    fn test_validation_fails_for_invalid_error_rate() {
        let mut config = SelfImprovementConfig::default();
        config.daemon.error_rate_threshold = 1.5; // Invalid: > 1.0

        let result = config.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.field == "daemon.error_rate_threshold"));
    }

    #[test]
    fn test_validation_fails_for_zero_max_traces() {
        let mut config = SelfImprovementConfig::default();
        config.trace_retention.max_traces = Some(0);

        let result = config.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.field == "trace_retention.max_traces"));
    }

    #[test]
    fn test_validation_error_display() {
        let error = ConfigValidationError {
            field: "daemon.interval".to_string(),
            message: "Must be > 0".to_string(),
            suggestion: Some("Set to at least 1".to_string()),
        };

        let display = format!("{}", error);
        assert!(display.contains("daemon.interval"));
        assert!(display.contains("Must be > 0"));
        assert!(display.contains("Set to at least 1"));
    }

    #[test]
    fn test_build_and_validate_success() {
        let result = SelfImprovementConfig::builder()
            .with_interval_secs(60)
            .build_and_validate();

        assert!(result.is_ok());
    }

    #[test]
    fn test_build_and_validate_failure() {
        let result = SelfImprovementConfig::builder()
            .with_interval_secs(0)
            .build_and_validate();

        assert!(result.is_err());
    }

    #[test]
    fn test_prometheus_url_validation() {
        let mut config = SelfImprovementConfig::default();
        config.daemon.prometheus_endpoint = Some("not-a-url".to_string());

        let result = config.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.field == "daemon.prometheus_endpoint"));
    }

    #[test]
    fn test_in_process_metrics_skips_prometheus_validation() {
        let mut config = SelfImprovementConfig::default();
        config.daemon.metrics_source = MetricsSource::InProcess;
        config.daemon.prometheus_endpoint = Some("not-a-url".to_string());

        // Should pass because we're not using HTTP metrics
        let result = config.validate();
        assert!(result.is_ok());
    }
}
