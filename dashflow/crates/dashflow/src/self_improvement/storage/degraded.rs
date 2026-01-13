// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graceful degradation support for the storage system.
//!
//! When storage or related components fail, the system continues operating
//! with reduced functionality rather than failing completely.

// ============================================================================
// Graceful Degradation
// ============================================================================

/// Represents which components are in degraded mode.
///
/// When a component fails (e.g., storage is inaccessible), the system
/// continues to operate with reduced functionality rather than failing
/// completely.
#[derive(Debug, Clone, Default)]
pub struct DegradedMode {
    /// Storage is unavailable - reports/plans/hypotheses won't be persisted.
    pub storage_unavailable: bool,
    /// Prometheus is unavailable - no metrics will be collected.
    pub prometheus_unavailable: bool,
    /// Alert system is unavailable - alerts won't be dispatched.
    pub alerts_unavailable: bool,
    /// Timestamp when degraded mode started.
    pub degraded_since: Option<chrono::DateTime<chrono::Utc>>,
    /// List of specific failures that caused degradation.
    pub failures: Vec<DegradationFailure>,
}

/// A specific failure that caused the system to enter degraded mode.
#[derive(Debug, Clone)]
pub struct DegradationFailure {
    /// Component that failed.
    pub component: DegradedComponent,
    /// Error message.
    pub error: String,
    /// When the failure occurred.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Number of consecutive failures.
    pub consecutive_failures: usize,
}

/// Components that can operate in degraded mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DegradedComponent {
    /// File-based storage system.
    Storage,
    /// Prometheus metrics collection.
    Prometheus,
    /// Alert dispatching system.
    Alerts,
    /// Trace file watching.
    TraceWatcher,
}

impl std::fmt::Display for DegradedComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage => write!(f, "Storage"),
            Self::Prometheus => write!(f, "Prometheus"),
            Self::Alerts => write!(f, "Alerts"),
            Self::TraceWatcher => write!(f, "TraceWatcher"),
        }
    }
}

impl DegradedMode {
    /// Create a new DegradedMode with all components operational.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the system is operating in any degraded mode.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.storage_unavailable || self.prometheus_unavailable || self.alerts_unavailable
    }

    /// Check if a specific component is degraded.
    #[must_use]
    pub fn is_component_degraded(&self, component: DegradedComponent) -> bool {
        match component {
            DegradedComponent::Storage => self.storage_unavailable,
            DegradedComponent::Prometheus => self.prometheus_unavailable,
            DegradedComponent::Alerts => self.alerts_unavailable,
            DegradedComponent::TraceWatcher => false, // Not tracked separately
        }
    }

    /// Mark a component as failed and enter degraded mode.
    pub fn mark_failed(&mut self, component: DegradedComponent, error: impl Into<String>) {
        let error = error.into();
        let now = chrono::Utc::now();

        // Update component status
        match component {
            DegradedComponent::Storage => self.storage_unavailable = true,
            DegradedComponent::Prometheus => self.prometheus_unavailable = true,
            DegradedComponent::Alerts => self.alerts_unavailable = true,
            DegradedComponent::TraceWatcher => {} // Not tracked separately
        }

        // Set degraded_since if this is the first failure
        if self.degraded_since.is_none() {
            self.degraded_since = Some(now);
        }

        // Track consecutive failures
        let consecutive = self
            .failures
            .iter()
            .filter(|f| f.component == component)
            .map(|f| f.consecutive_failures)
            .max()
            .unwrap_or(0)
            + 1;

        self.failures.push(DegradationFailure {
            component,
            error,
            timestamp: now,
            consecutive_failures: consecutive,
        });

        // Keep only last 100 failures to avoid unbounded growth
        if self.failures.len() > 100 {
            self.failures.drain(0..50);
        }
    }

    /// Mark a component as recovered and potentially exit degraded mode.
    pub fn mark_recovered(&mut self, component: DegradedComponent) {
        match component {
            DegradedComponent::Storage => self.storage_unavailable = false,
            DegradedComponent::Prometheus => self.prometheus_unavailable = false,
            DegradedComponent::Alerts => self.alerts_unavailable = false,
            DegradedComponent::TraceWatcher => {}
        }

        // Clear degraded_since if all components are recovered
        if !self.is_degraded() {
            self.degraded_since = None;
        }
    }

    /// Get summary of current degradation status.
    #[must_use]
    pub fn summary(&self) -> String {
        if !self.is_degraded() {
            return "All systems operational".to_string();
        }

        let mut degraded = Vec::new();
        if self.storage_unavailable {
            degraded.push("Storage");
        }
        if self.prometheus_unavailable {
            degraded.push("Prometheus");
        }
        if self.alerts_unavailable {
            degraded.push("Alerts");
        }

        format!(
            "Degraded mode: {} unavailable{}",
            degraded.join(", "),
            self.degraded_since
                .map(|t| format!(" (since {})", t.format("%H:%M:%S")))
                .unwrap_or_default()
        )
    }

    /// Get the number of recent failures for a component.
    #[must_use]
    pub fn recent_failures(&self, component: DegradedComponent, window: chrono::Duration) -> usize {
        let cutoff = chrono::Utc::now() - window;
        self.failures
            .iter()
            .filter(|f| f.component == component && f.timestamp > cutoff)
            .count()
    }
}

/// Result type for operations that can gracefully degrade.
///
/// Instead of returning an error, operations can return this type
/// to indicate that they succeeded but in a degraded mode.
#[derive(Debug, Clone)]
pub struct DegradedResult<T> {
    /// The result value (may be default/empty if degraded).
    pub value: T,
    /// Whether the operation completed in degraded mode.
    pub degraded: bool,
    /// What component was degraded (if any).
    pub degraded_component: Option<DegradedComponent>,
    /// Warning message about the degradation.
    pub warning: Option<String>,
}

impl<T> DegradedResult<T> {
    /// Create a successful result (not degraded).
    #[must_use]
    pub fn ok(value: T) -> Self {
        Self {
            value,
            degraded: false,
            degraded_component: None,
            warning: None,
        }
    }

    /// Create a degraded result with a fallback value.
    #[must_use]
    pub fn degraded(value: T, component: DegradedComponent, warning: impl Into<String>) -> Self {
        Self {
            value,
            degraded: true,
            degraded_component: Some(component),
            warning: Some(warning.into()),
        }
    }

    /// Check if this result is degraded.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Get the value, ignoring degradation status.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T: Default> DegradedResult<T> {
    /// Create a degraded result with the default value.
    #[must_use]
    pub fn degraded_default(component: DegradedComponent, warning: impl Into<String>) -> Self {
        Self::degraded(T::default(), component, warning)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // DegradedComponent Tests
    // ========================================================================

    #[test]
    fn test_degraded_component_display_storage() {
        assert_eq!(DegradedComponent::Storage.to_string(), "Storage");
    }

    #[test]
    fn test_degraded_component_display_prometheus() {
        assert_eq!(DegradedComponent::Prometheus.to_string(), "Prometheus");
    }

    #[test]
    fn test_degraded_component_display_alerts() {
        assert_eq!(DegradedComponent::Alerts.to_string(), "Alerts");
    }

    #[test]
    fn test_degraded_component_display_trace_watcher() {
        assert_eq!(DegradedComponent::TraceWatcher.to_string(), "TraceWatcher");
    }

    #[test]
    fn test_degraded_component_eq() {
        assert_eq!(DegradedComponent::Storage, DegradedComponent::Storage);
        assert_ne!(DegradedComponent::Storage, DegradedComponent::Prometheus);
    }

    #[test]
    fn test_degraded_component_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(DegradedComponent::Storage);
        set.insert(DegradedComponent::Prometheus);
        set.insert(DegradedComponent::Storage); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_degraded_component_clone() {
        let component = DegradedComponent::Alerts;
        let cloned = component;
        assert_eq!(component, cloned);
    }

    #[test]
    fn test_degraded_component_copy() {
        let component = DegradedComponent::Storage;
        let copied: DegradedComponent = component;
        assert_eq!(component, copied);
    }

    // ========================================================================
    // DegradationFailure Tests
    // ========================================================================

    #[test]
    fn test_degradation_failure_creation() {
        let failure = DegradationFailure {
            component: DegradedComponent::Storage,
            error: "disk full".to_string(),
            timestamp: chrono::Utc::now(),
            consecutive_failures: 3,
        };
        assert_eq!(failure.component, DegradedComponent::Storage);
        assert_eq!(failure.error, "disk full");
        assert_eq!(failure.consecutive_failures, 3);
    }

    #[test]
    fn test_degradation_failure_clone() {
        let failure = DegradationFailure {
            component: DegradedComponent::Prometheus,
            error: "connection refused".to_string(),
            timestamp: chrono::Utc::now(),
            consecutive_failures: 1,
        };
        let cloned = failure.clone();
        assert_eq!(cloned.component, failure.component);
        assert_eq!(cloned.error, failure.error);
        assert_eq!(cloned.consecutive_failures, failure.consecutive_failures);
    }

    // ========================================================================
    // DegradedMode Tests
    // ========================================================================

    #[test]
    fn test_degraded_mode_new() {
        let mode = DegradedMode::new();
        assert!(!mode.storage_unavailable);
        assert!(!mode.prometheus_unavailable);
        assert!(!mode.alerts_unavailable);
        assert!(mode.degraded_since.is_none());
        assert!(mode.failures.is_empty());
    }

    #[test]
    fn test_degraded_mode_default() {
        let mode = DegradedMode::default();
        assert!(!mode.is_degraded());
    }

    #[test]
    fn test_degraded_mode_is_degraded_none() {
        let mode = DegradedMode::new();
        assert!(!mode.is_degraded());
    }

    #[test]
    fn test_degraded_mode_is_degraded_storage() {
        let mut mode = DegradedMode::new();
        mode.storage_unavailable = true;
        assert!(mode.is_degraded());
    }

    #[test]
    fn test_degraded_mode_is_degraded_prometheus() {
        let mut mode = DegradedMode::new();
        mode.prometheus_unavailable = true;
        assert!(mode.is_degraded());
    }

    #[test]
    fn test_degraded_mode_is_degraded_alerts() {
        let mut mode = DegradedMode::new();
        mode.alerts_unavailable = true;
        assert!(mode.is_degraded());
    }

    #[test]
    fn test_degraded_mode_is_degraded_multiple() {
        let mut mode = DegradedMode::new();
        mode.storage_unavailable = true;
        mode.prometheus_unavailable = true;
        assert!(mode.is_degraded());
    }

    #[test]
    fn test_is_component_degraded_storage() {
        let mut mode = DegradedMode::new();
        assert!(!mode.is_component_degraded(DegradedComponent::Storage));
        mode.storage_unavailable = true;
        assert!(mode.is_component_degraded(DegradedComponent::Storage));
    }

    #[test]
    fn test_is_component_degraded_prometheus() {
        let mut mode = DegradedMode::new();
        assert!(!mode.is_component_degraded(DegradedComponent::Prometheus));
        mode.prometheus_unavailable = true;
        assert!(mode.is_component_degraded(DegradedComponent::Prometheus));
    }

    #[test]
    fn test_is_component_degraded_alerts() {
        let mut mode = DegradedMode::new();
        assert!(!mode.is_component_degraded(DegradedComponent::Alerts));
        mode.alerts_unavailable = true;
        assert!(mode.is_component_degraded(DegradedComponent::Alerts));
    }

    #[test]
    fn test_is_component_degraded_trace_watcher() {
        // TraceWatcher is not tracked separately
        let mode = DegradedMode::new();
        assert!(!mode.is_component_degraded(DegradedComponent::TraceWatcher));
    }

    #[test]
    fn test_mark_failed_storage() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "disk error");
        assert!(mode.storage_unavailable);
        assert!(mode.degraded_since.is_some());
        assert_eq!(mode.failures.len(), 1);
        assert_eq!(mode.failures[0].component, DegradedComponent::Storage);
        assert_eq!(mode.failures[0].error, "disk error");
        assert_eq!(mode.failures[0].consecutive_failures, 1);
    }

    #[test]
    fn test_mark_failed_prometheus() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Prometheus, "connection refused");
        assert!(mode.prometheus_unavailable);
        assert_eq!(mode.failures.len(), 1);
    }

    #[test]
    fn test_mark_failed_alerts() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Alerts, "smtp error");
        assert!(mode.alerts_unavailable);
    }

    #[test]
    fn test_mark_failed_trace_watcher() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::TraceWatcher, "file not found");
        // TraceWatcher doesn't set any flag, but failure is recorded
        assert!(!mode.is_degraded());
        assert_eq!(mode.failures.len(), 1);
    }

    #[test]
    fn test_mark_failed_consecutive_failures() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error 1");
        mode.mark_failed(DegradedComponent::Storage, "error 2");
        mode.mark_failed(DegradedComponent::Storage, "error 3");

        assert_eq!(mode.failures.len(), 3);
        assert_eq!(mode.failures[2].consecutive_failures, 3);
    }

    #[test]
    fn test_mark_failed_degraded_since_preserved() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error 1");
        let first_degraded_since = mode.degraded_since;

        mode.mark_failed(DegradedComponent::Prometheus, "error 2");
        // degraded_since should not change on subsequent failures
        assert_eq!(mode.degraded_since, first_degraded_since);
    }

    #[test]
    fn test_mark_failed_failures_bounded() {
        let mut mode = DegradedMode::new();
        for i in 0..150 {
            mode.mark_failed(DegradedComponent::Storage, format!("error {}", i));
        }
        // Should keep only last ~50 after draining
        assert!(mode.failures.len() <= 100);
    }

    #[test]
    fn test_mark_recovered_storage() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        assert!(mode.storage_unavailable);

        mode.mark_recovered(DegradedComponent::Storage);
        assert!(!mode.storage_unavailable);
    }

    #[test]
    fn test_mark_recovered_clears_degraded_since() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        assert!(mode.degraded_since.is_some());

        mode.mark_recovered(DegradedComponent::Storage);
        // All components recovered, degraded_since should be cleared
        assert!(mode.degraded_since.is_none());
    }

    #[test]
    fn test_mark_recovered_partial() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        mode.mark_failed(DegradedComponent::Prometheus, "error");

        mode.mark_recovered(DegradedComponent::Storage);
        // Still degraded because Prometheus is unavailable
        assert!(mode.is_degraded());
        assert!(mode.degraded_since.is_some());
    }

    #[test]
    fn test_mark_recovered_prometheus() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Prometheus, "error");
        mode.mark_recovered(DegradedComponent::Prometheus);
        assert!(!mode.prometheus_unavailable);
    }

    #[test]
    fn test_mark_recovered_alerts() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Alerts, "error");
        mode.mark_recovered(DegradedComponent::Alerts);
        assert!(!mode.alerts_unavailable);
    }

    #[test]
    fn test_mark_recovered_trace_watcher() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::TraceWatcher, "error");
        mode.mark_recovered(DegradedComponent::TraceWatcher);
        // No-op for TraceWatcher
        assert!(!mode.is_degraded());
    }

    #[test]
    fn test_summary_all_operational() {
        let mode = DegradedMode::new();
        assert_eq!(mode.summary(), "All systems operational");
    }

    #[test]
    fn test_summary_storage_unavailable() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        let summary = mode.summary();
        assert!(summary.contains("Degraded mode"));
        assert!(summary.contains("Storage"));
    }

    #[test]
    fn test_summary_multiple_unavailable() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        mode.mark_failed(DegradedComponent::Prometheus, "error");
        let summary = mode.summary();
        assert!(summary.contains("Storage"));
        assert!(summary.contains("Prometheus"));
    }

    #[test]
    fn test_summary_includes_since_time() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        let summary = mode.summary();
        assert!(summary.contains("(since"));
    }

    #[test]
    fn test_recent_failures_empty() {
        let mode = DegradedMode::new();
        let count = mode.recent_failures(DegradedComponent::Storage, chrono::Duration::minutes(5));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_recent_failures_within_window() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error 1");
        mode.mark_failed(DegradedComponent::Storage, "error 2");

        let count = mode.recent_failures(DegradedComponent::Storage, chrono::Duration::minutes(5));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_recent_failures_different_component() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");
        mode.mark_failed(DegradedComponent::Prometheus, "error");

        let storage_count =
            mode.recent_failures(DegradedComponent::Storage, chrono::Duration::minutes(5));
        let prometheus_count =
            mode.recent_failures(DegradedComponent::Prometheus, chrono::Duration::minutes(5));

        assert_eq!(storage_count, 1);
        assert_eq!(prometheus_count, 1);
    }

    #[test]
    fn test_degraded_mode_clone() {
        let mut mode = DegradedMode::new();
        mode.mark_failed(DegradedComponent::Storage, "error");

        let cloned = mode.clone();
        assert_eq!(cloned.storage_unavailable, mode.storage_unavailable);
        assert_eq!(cloned.failures.len(), mode.failures.len());
    }

    // ========================================================================
    // DegradedResult Tests
    // ========================================================================

    #[test]
    fn test_degraded_result_ok() {
        let result = DegradedResult::ok(42);
        assert_eq!(result.value, 42);
        assert!(!result.degraded);
        assert!(result.degraded_component.is_none());
        assert!(result.warning.is_none());
    }

    #[test]
    fn test_degraded_result_degraded() {
        let result = DegradedResult::degraded(0, DegradedComponent::Storage, "using default");
        assert_eq!(result.value, 0);
        assert!(result.degraded);
        assert_eq!(result.degraded_component, Some(DegradedComponent::Storage));
        assert_eq!(result.warning, Some("using default".to_string()));
    }

    #[test]
    fn test_degraded_result_is_degraded() {
        let ok = DegradedResult::ok("value");
        let degraded =
            DegradedResult::degraded("fallback", DegradedComponent::Prometheus, "warning");

        assert!(!ok.is_degraded());
        assert!(degraded.is_degraded());
    }

    #[test]
    fn test_degraded_result_into_value() {
        let result = DegradedResult::ok(vec![1, 2, 3]);
        let value = result.into_value();
        assert_eq!(value, vec![1, 2, 3]);
    }

    #[test]
    fn test_degraded_result_degraded_into_value() {
        let result = DegradedResult::degraded(
            "fallback".to_string(),
            DegradedComponent::Alerts,
            "alert system down",
        );
        let value = result.into_value();
        assert_eq!(value, "fallback");
    }

    #[test]
    fn test_degraded_result_degraded_default() {
        let result: DegradedResult<Vec<i32>> =
            DegradedResult::degraded_default(DegradedComponent::Storage, "storage offline");

        assert!(result.degraded);
        assert!(result.value.is_empty()); // Vec default is empty
        assert_eq!(result.degraded_component, Some(DegradedComponent::Storage));
    }

    #[test]
    fn test_degraded_result_degraded_default_string() {
        let result: DegradedResult<String> =
            DegradedResult::degraded_default(DegradedComponent::Prometheus, "metrics unavailable");

        assert!(result.degraded);
        assert!(result.value.is_empty()); // String default is empty
    }

    #[test]
    fn test_degraded_result_degraded_default_number() {
        let result: DegradedResult<i32> =
            DegradedResult::degraded_default(DegradedComponent::Alerts, "alert system down");

        assert!(result.degraded);
        assert_eq!(result.value, 0); // i32 default is 0
    }

    #[test]
    fn test_degraded_result_clone() {
        let result = DegradedResult::degraded(42, DegradedComponent::Storage, "warning");
        let cloned = result.clone();

        assert_eq!(cloned.value, result.value);
        assert_eq!(cloned.degraded, result.degraded);
        assert_eq!(cloned.degraded_component, result.degraded_component);
        assert_eq!(cloned.warning, result.warning);
    }

    #[test]
    fn test_degraded_result_debug() {
        let result = DegradedResult::ok(42);
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("DegradedResult"));
        assert!(debug_str.contains("42"));
    }
}
