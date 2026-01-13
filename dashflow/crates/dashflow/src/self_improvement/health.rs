// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Health Check Endpoints for Self-Improvement Components.
//!
//! This module provides health check functionality for monitoring the
//! self-improvement system components: daemon, storage, analyzers, and cache.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::health::{HealthChecker, ComponentHealth};
//!
//! let checker = HealthChecker::default();
//! let health = checker.check_all();
//!
//! if health.is_healthy() {
//!     println!("All components healthy");
//! } else {
//!     for warning in &health.warnings {
//!         println!("Warning: {}", warning);
//!     }
//! }
//! ```
//!
//! ## Health Levels
//!
//! - `Healthy`: Component is operating normally
//! - `Degraded`: Component is operating with reduced functionality
//! - `Unhealthy`: Component is not functioning

use crate::core::config_loader::env_vars::{
    env_bool, env_string_or_default, env_u64, env_usize, DASHFLOW_HEALTH_CHECK_CACHE,
    DASHFLOW_HEALTH_CHECK_STORAGE, DASHFLOW_HEALTH_CHECK_TRACES, DASHFLOW_HEALTH_MAX_STORAGE_MB,
    DASHFLOW_HEALTH_MAX_TRACES, DASHFLOW_HEALTH_STORAGE_PATH, DASHFLOW_HEALTH_TRACES_PATH,
};
use crate::self_improvement::storage::{IntrospectionStorage, StorageHealthLevel};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Health level for a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthLevel {
    /// Component is operating normally
    Healthy,
    /// Component is operating with reduced functionality
    Degraded,
    /// Component is not functioning
    Unhealthy,
}

impl HealthLevel {
    /// Check if this level represents a healthy state.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if this level represents a degraded state.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded)
    }

    /// Check if this level represents an unhealthy state.
    #[must_use]
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy)
    }
}

impl std::fmt::Display for HealthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

impl From<StorageHealthLevel> for HealthLevel {
    fn from(level: StorageHealthLevel) -> Self {
        match level {
            StorageHealthLevel::Healthy => Self::Healthy,
            StorageHealthLevel::Warning => Self::Degraded,
            StorageHealthLevel::Critical => Self::Unhealthy,
        }
    }
}

/// Health status for an individual component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Name of the component
    pub name: String,
    /// Overall health level
    pub level: HealthLevel,
    /// Human-readable status message
    pub message: String,
    /// Detailed check results
    pub checks: Vec<HealthCheck>,
    /// Time taken to check this component
    pub check_duration_ms: u64,
}

impl ComponentHealth {
    /// Create a healthy component status.
    #[must_use]
    pub fn healthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            level: HealthLevel::Healthy,
            message: message.into(),
            checks: Vec::new(),
            check_duration_ms: 0,
        }
    }

    /// Create a degraded component status.
    #[must_use]
    pub fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            level: HealthLevel::Degraded,
            message: message.into(),
            checks: Vec::new(),
            check_duration_ms: 0,
        }
    }

    /// Create an unhealthy component status.
    #[must_use]
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            level: HealthLevel::Unhealthy,
            message: message.into(),
            checks: Vec::new(),
            check_duration_ms: 0,
        }
    }

    /// Add a check result.
    #[must_use]
    pub fn with_check(mut self, check: HealthCheck) -> Self {
        // Update level based on check results
        if check.passed {
            // Keep current level
        } else if check.critical {
            self.level = HealthLevel::Unhealthy;
        } else if self.level == HealthLevel::Healthy {
            self.level = HealthLevel::Degraded;
        }
        self.checks.push(check);
        self
    }

    /// Set the check duration.
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.check_duration_ms = duration.as_millis() as u64;
        self
    }
}

/// Individual health check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Name of the check
    pub name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Whether failure of this check is critical
    pub critical: bool,
    /// Details about the check result
    pub details: String,
}

impl HealthCheck {
    /// Create a passed check.
    #[must_use]
    pub fn passed(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            critical: false,
            details: details.into(),
        }
    }

    /// Create a failed check.
    #[must_use]
    pub fn failed(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            critical: false,
            details: details.into(),
        }
    }

    /// Create a critical failed check.
    #[must_use]
    pub fn critical_failed(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            critical: true,
            details: details.into(),
        }
    }

    /// Mark this check as critical.
    #[must_use]
    pub fn mark_critical(mut self) -> Self {
        self.critical = true;
        self
    }
}

/// Overall health status for the self-improvement system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Overall health level
    pub level: HealthLevel,
    /// Component health statuses
    pub components: Vec<ComponentHealth>,
    /// Warnings that don't affect health level
    pub warnings: Vec<String>,
    /// Total time taken for all health checks
    pub total_check_duration_ms: u64,
    /// Timestamp of the health check
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

impl SystemHealth {
    /// Create a new system health status.
    #[must_use]
    pub fn new() -> Self {
        Self {
            level: HealthLevel::Healthy,
            components: Vec::new(),
            warnings: Vec::new(),
            total_check_duration_ms: 0,
            checked_at: chrono::Utc::now(),
        }
    }

    /// Add a component health status.
    #[must_use]
    pub fn with_component(mut self, component: ComponentHealth) -> Self {
        // Update overall level based on component health
        match component.level {
            HealthLevel::Unhealthy => self.level = HealthLevel::Unhealthy,
            HealthLevel::Degraded if self.level == HealthLevel::Healthy => {
                self.level = HealthLevel::Degraded;
            }
            _ => {}
        }
        self.total_check_duration_ms += component.check_duration_ms;
        self.components.push(component);
        self
    }

    /// Add a warning.
    #[must_use]
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Check if the system is healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.level.is_healthy()
    }

    /// Get the number of healthy components.
    #[must_use]
    pub fn healthy_count(&self) -> usize {
        self.components
            .iter()
            .filter(|c| c.level.is_healthy())
            .count()
    }

    /// Get the number of degraded components.
    #[must_use]
    pub fn degraded_count(&self) -> usize {
        self.components
            .iter()
            .filter(|c| c.level.is_degraded())
            .count()
    }

    /// Get the number of unhealthy components.
    #[must_use]
    pub fn unhealthy_count(&self) -> usize {
        self.components
            .iter()
            .filter(|c| c.level.is_unhealthy())
            .count()
    }
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for health checks.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Path to storage directory
    pub storage_path: PathBuf,
    /// Path to traces directory
    pub traces_path: PathBuf,
    /// Check storage health
    pub check_storage: bool,
    /// Check traces directory
    pub check_traces: bool,
    /// Check cache status
    pub check_cache: bool,
    /// Maximum acceptable storage size (bytes)
    pub max_storage_size: u64,
    /// Maximum acceptable trace count
    pub max_trace_count: usize,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(".dashflow/introspection"),
            traces_path: PathBuf::from(".dashflow/traces"),
            check_storage: true,
            check_traces: true,
            check_cache: true,
            max_storage_size: 500 * 1024 * 1024, // 500 MB
            max_trace_count: 10000,
        }
    }
}

impl HealthCheckConfig {
    /// Create configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// | Variable | Type | Default | Description |
    /// |----------|------|---------|-------------|
    /// | `DASHFLOW_HEALTH_STORAGE_PATH` | path | `.dashflow/introspection` | Storage directory |
    /// | `DASHFLOW_HEALTH_TRACES_PATH` | path | `.dashflow/traces` | Traces directory |
    /// | `DASHFLOW_HEALTH_CHECK_STORAGE` | bool | true | Check storage health |
    /// | `DASHFLOW_HEALTH_CHECK_TRACES` | bool | true | Check traces health |
    /// | `DASHFLOW_HEALTH_CHECK_CACHE` | bool | true | Check cache health |
    /// | `DASHFLOW_HEALTH_MAX_STORAGE_MB` | u64 | 500 | Max storage size in MB |
    /// | `DASHFLOW_HEALTH_MAX_TRACES` | usize | 10000 | Max trace count |
    #[must_use]
    pub fn from_env() -> Self {
        let storage_path = PathBuf::from(env_string_or_default(
            DASHFLOW_HEALTH_STORAGE_PATH,
            ".dashflow/introspection",
        ));
        let traces_path = PathBuf::from(env_string_or_default(
            DASHFLOW_HEALTH_TRACES_PATH,
            ".dashflow/traces",
        ));
        let check_storage = env_bool(DASHFLOW_HEALTH_CHECK_STORAGE, true);
        let check_traces = env_bool(DASHFLOW_HEALTH_CHECK_TRACES, true);
        let check_cache = env_bool(DASHFLOW_HEALTH_CHECK_CACHE, true);
        let max_storage_mb = env_u64(DASHFLOW_HEALTH_MAX_STORAGE_MB, 500);
        let max_trace_count = env_usize(DASHFLOW_HEALTH_MAX_TRACES, 10000);

        Self {
            storage_path,
            traces_path,
            check_storage,
            check_traces,
            check_cache,
            max_storage_size: max_storage_mb * 1024 * 1024,
            max_trace_count,
        }
    }
}

/// Health checker for self-improvement components.
pub struct HealthChecker {
    config: HealthCheckConfig,
    storage: Option<IntrospectionStorage>,
}

impl HealthChecker {
    /// Create a new health checker with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: HealthCheckConfig::default(),
            storage: None,
        }
    }

    /// Create a health checker with custom configuration.
    #[must_use]
    pub fn with_config(config: HealthCheckConfig) -> Self {
        Self {
            config,
            storage: None,
        }
    }

    /// Attach a storage instance for detailed checks.
    #[must_use]
    pub fn with_storage(mut self, storage: IntrospectionStorage) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Run all health checks.
    pub fn check_all(&self) -> SystemHealth {
        let mut health = SystemHealth::new();

        if self.config.check_storage {
            health = health.with_component(self.check_storage());
        }

        if self.config.check_traces {
            health = health.with_component(self.check_traces());
        }

        if self.config.check_cache {
            health = health.with_component(self.check_cache());
        }

        health
    }

    /// Check storage health.
    pub fn check_storage(&self) -> ComponentHealth {
        let start = Instant::now();
        let mut component = ComponentHealth::healthy("storage", "Storage is accessible");

        // Check if storage path exists
        if !self.config.storage_path.exists() {
            component = component.with_check(HealthCheck::passed(
                "directory_exists",
                "Directory does not exist yet (will be created on first use)",
            ));
        } else {
            component = component.with_check(HealthCheck::passed(
                "directory_exists",
                format!("Directory exists at {:?}", self.config.storage_path),
            ));

            // If we have a storage instance, use its health check
            if let Some(ref storage) = self.storage {
                match storage.check_health() {
                    Ok(storage_health) => {
                        component.level = HealthLevel::from(storage_health.level);

                        for warning in &storage_health.warnings {
                            component = component.with_check(HealthCheck::failed(
                                "storage_warning",
                                warning.clone(),
                            ));
                        }
                    }
                    Err(e) => {
                        component = component.with_check(HealthCheck::critical_failed(
                            "storage_health_check",
                            format!("Failed to check storage health: {}", e),
                        ));
                    }
                }
            }

            // Check directory is writable
            let test_file = self.config.storage_path.join(".health_check");
            match std::fs::write(&test_file, "test") {
                Ok(()) => {
                    let _ = std::fs::remove_file(&test_file);
                    component = component
                        .with_check(HealthCheck::passed("writable", "Directory is writable"));
                }
                Err(e) => {
                    component = component.with_check(HealthCheck::critical_failed(
                        "writable",
                        format!("Directory is not writable: {}", e),
                    ));
                }
            }
        }

        component.with_duration(start.elapsed())
    }

    /// Check traces directory health.
    pub fn check_traces(&self) -> ComponentHealth {
        let start = Instant::now();
        let mut component = ComponentHealth::healthy("traces", "Traces directory is accessible");

        if !self.config.traces_path.exists() {
            component = component.with_check(HealthCheck::passed(
                "directory_exists",
                "Traces directory does not exist yet (will be created on first execution)",
            ));
        } else {
            component = component.with_check(HealthCheck::passed(
                "directory_exists",
                format!("Directory exists at {:?}", self.config.traces_path),
            ));

            // Count trace files
            let trace_count = std::fs::read_dir(&self.config.traces_path)
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "json" || ext == "gz")
                                .unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0);

            if trace_count > self.config.max_trace_count {
                component = component.with_check(HealthCheck::failed(
                    "trace_count",
                    format!(
                        "Too many traces: {} (max: {}). Consider running cleanup.",
                        trace_count, self.config.max_trace_count
                    ),
                ));
            } else {
                component = component.with_check(HealthCheck::passed(
                    "trace_count",
                    format!(
                        "{} traces (max: {})",
                        trace_count, self.config.max_trace_count
                    ),
                ));
            }
        }

        component.with_duration(start.elapsed())
    }

    /// Check cache health (basic check without actual cache instance).
    pub fn check_cache(&self) -> ComponentHealth {
        let start = Instant::now();
        // Since we don't have direct access to the cache here, we just return a healthy status
        // The actual cache health would be checked via the daemon if available
        ComponentHealth::healthy("cache", "Cache module available")
            .with_check(HealthCheck::passed(
                "module_available",
                "Cache module is compiled and available",
            ))
            .with_duration(start.elapsed())
    }

    /// Check daemon health (requires daemon instance).
    ///
    /// This is called separately since it requires a daemon instance.
    pub fn check_daemon_from_stats(
        running: bool,
        cycles_completed: u64,
        cache_hits: u64,
        cache_misses: u64,
    ) -> ComponentHealth {
        let start = Instant::now();
        let mut component = if running {
            ComponentHealth::healthy("daemon", "Daemon is running")
        } else {
            ComponentHealth::degraded("daemon", "Daemon is not running")
        };

        component = component.with_check(if running {
            HealthCheck::passed("running", "Daemon process is active")
        } else {
            HealthCheck::failed("running", "Daemon is not currently running")
        });

        component = component.with_check(HealthCheck::passed(
            "cycles",
            format!("{} analysis cycles completed", cycles_completed),
        ));

        let hit_rate = if cache_hits + cache_misses > 0 {
            cache_hits as f64 / (cache_hits + cache_misses) as f64 * 100.0
        } else {
            0.0
        };

        component = component.with_check(HealthCheck::passed(
            "cache_efficiency",
            format!(
                "Cache hit rate: {:.1}% ({} hits, {} misses)",
                hit_rate, cache_hits, cache_misses
            ),
        ));

        component.with_duration(start.elapsed())
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_level_conversion() {
        assert!(HealthLevel::Healthy.is_healthy());
        assert!(!HealthLevel::Healthy.is_degraded());
        assert!(!HealthLevel::Healthy.is_unhealthy());

        assert!(!HealthLevel::Degraded.is_healthy());
        assert!(HealthLevel::Degraded.is_degraded());
        assert!(!HealthLevel::Degraded.is_unhealthy());

        assert!(!HealthLevel::Unhealthy.is_healthy());
        assert!(!HealthLevel::Unhealthy.is_degraded());
        assert!(HealthLevel::Unhealthy.is_unhealthy());
    }

    #[test]
    fn test_component_health_builder() {
        let component = ComponentHealth::healthy("test", "All good")
            .with_check(HealthCheck::passed("check1", "Passed"))
            .with_check(HealthCheck::passed("check2", "Also passed"));

        assert_eq!(component.level, HealthLevel::Healthy);
        assert_eq!(component.checks.len(), 2);
    }

    #[test]
    fn test_component_health_degradation() {
        let component = ComponentHealth::healthy("test", "All good")
            .with_check(HealthCheck::passed("check1", "Passed"))
            .with_check(HealthCheck::failed("check2", "Failed but not critical"));

        assert_eq!(component.level, HealthLevel::Degraded);
    }

    #[test]
    fn test_component_health_critical_failure() {
        let component = ComponentHealth::healthy("test", "All good")
            .with_check(HealthCheck::passed("check1", "Passed"))
            .with_check(HealthCheck::critical_failed("check2", "Critical failure"));

        assert_eq!(component.level, HealthLevel::Unhealthy);
    }

    #[test]
    fn test_system_health_aggregation() {
        let health = SystemHealth::new()
            .with_component(ComponentHealth::healthy("comp1", "OK"))
            .with_component(ComponentHealth::degraded("comp2", "Warning"))
            .with_component(ComponentHealth::healthy("comp3", "OK"));

        assert_eq!(health.level, HealthLevel::Degraded);
        assert_eq!(health.healthy_count(), 2);
        assert_eq!(health.degraded_count(), 1);
        assert_eq!(health.unhealthy_count(), 0);
    }

    #[test]
    fn test_system_health_unhealthy() {
        let health = SystemHealth::new()
            .with_component(ComponentHealth::healthy("comp1", "OK"))
            .with_component(ComponentHealth::unhealthy("comp2", "Failed"));

        assert_eq!(health.level, HealthLevel::Unhealthy);
    }

    #[test]
    fn test_health_checker_basic() {
        let checker = HealthChecker::default();
        let health = checker.check_all();

        // Should return results for all configured components
        assert!(!health.components.is_empty());
    }

    #[test]
    fn test_daemon_health_from_stats() {
        let health = HealthChecker::check_daemon_from_stats(true, 100, 80, 20);
        assert_eq!(health.level, HealthLevel::Healthy);
        assert!(health
            .checks
            .iter()
            .any(|c| c.name == "running" && c.passed));
        assert!(health.checks.iter().any(|c| c.name == "cycles"));
        assert!(health.checks.iter().any(|c| c.name == "cache_efficiency"));
    }

    #[test]
    fn test_daemon_health_not_running() {
        let health = HealthChecker::check_daemon_from_stats(false, 0, 0, 0);
        assert_eq!(health.level, HealthLevel::Degraded);
        assert!(health
            .checks
            .iter()
            .any(|c| c.name == "running" && !c.passed));
    }
}
