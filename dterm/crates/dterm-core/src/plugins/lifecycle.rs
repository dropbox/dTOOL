//! Plugin lifecycle management.
//!
//! This module provides lifecycle control for plugins including:
//! - Enable/disable plugins
//! - Budget enforcement with backoff
//! - Configuration updates
//! - Health monitoring

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::types::{PluginId, PluginMetrics, PluginState};

/// Budget enforcement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffPolicy {
    /// No backoff, disable immediately on budget violation.
    Immediate,
    /// Linear backoff: multiply delay by attempt count.
    Linear {
        /// Base delay in milliseconds.
        base_delay_ms: u64,
        /// Maximum attempts before disabling.
        max_attempts: u32,
    },
    /// Exponential backoff: double delay each attempt.
    Exponential {
        /// Initial delay in milliseconds.
        initial_delay_ms: u64,
        /// Maximum delay in milliseconds.
        max_delay_ms: u64,
        /// Maximum attempts before disabling.
        max_attempts: u32,
    },
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self::Exponential {
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            max_attempts: 5,
        }
    }
}

/// Budget configuration for a plugin.
#[derive(Debug, Clone)]
pub struct PluginBudget {
    /// Maximum fuel (CPU) per event.
    pub fuel_per_event: u64,
    /// Maximum processing time per event.
    pub max_time_per_event: Duration,
    /// Maximum total processing time per second.
    pub max_time_per_second: Duration,
    /// Maximum events per second.
    pub max_events_per_second: u32,
    /// Backoff policy for budget violations.
    pub backoff_policy: BackoffPolicy,
}

impl Default for PluginBudget {
    fn default() -> Self {
        Self {
            fuel_per_event: 10_000_000,
            max_time_per_event: Duration::from_millis(100),
            max_time_per_second: Duration::from_millis(500),
            max_events_per_second: 1000,
            backoff_policy: BackoffPolicy::default(),
        }
    }
}

/// Lifecycle state for a single plugin.
#[derive(Debug)]
pub struct PluginLifecycle {
    /// Plugin ID.
    pub id: PluginId,
    /// Whether the plugin is enabled.
    pub enabled: bool,
    /// Current state.
    pub state: PluginState,
    /// Budget configuration.
    pub budget: PluginBudget,
    /// Current backoff state.
    backoff: BackoffState,
    /// Rolling window for rate limiting.
    rate_window: RateWindow,
    /// Cumulative metrics.
    metrics: PluginMetrics,
    /// Last state change time.
    last_state_change: Instant,
}

impl PluginLifecycle {
    /// Create a new lifecycle tracker for a plugin.
    pub fn new(id: PluginId) -> Self {
        Self::with_budget(id, PluginBudget::default())
    }

    /// Create with custom budget.
    pub fn with_budget(id: PluginId, budget: PluginBudget) -> Self {
        Self {
            id,
            enabled: true,
            state: PluginState::Ready,
            budget,
            backoff: BackoffState::default(),
            rate_window: RateWindow::new(Duration::from_secs(1)),
            metrics: PluginMetrics::default(),
            last_state_change: Instant::now(),
        }
    }

    /// Check if the plugin can process an event right now.
    pub fn can_process(&self) -> bool {
        self.enabled
            && self.state == PluginState::Ready
            && !self.backoff.is_backing_off()
            && !self.rate_window.is_rate_limited(self.budget.max_events_per_second)
    }

    /// Record that an event was processed successfully.
    pub fn record_success(&mut self, processing_time: Duration) {
        self.rate_window.record_event();

        // Saturate at u64::MAX for very long executions (unlikely in practice)
        // The min() ensures the value fits in u64, making the truncation safe.
        #[allow(clippy::cast_possible_truncation)]
        let time_us = processing_time.as_micros().min(u128::from(u64::MAX)) as u64;
        self.metrics.events_processed += 1;
        self.metrics.total_processing_us = self.metrics.total_processing_us.saturating_add(time_us);

        // Reset backoff on successful processing
        self.backoff.reset();
    }

    /// Record a budget violation (timeout or fuel exhaustion).
    pub fn record_violation(&mut self, violation: BudgetViolation) -> LifecycleAction {
        match violation {
            BudgetViolation::Timeout => self.metrics.timeout_count += 1,
            BudgetViolation::FuelExhausted => self.metrics.trap_count += 1,
            BudgetViolation::RateLimited => {}
        }

        // Apply backoff policy
        match self.budget.backoff_policy {
            BackoffPolicy::Immediate => {
                self.disable();
                LifecycleAction::Disabled
            }
            BackoffPolicy::Linear {
                base_delay_ms,
                max_attempts,
            } => {
                self.backoff.attempt += 1;
                if self.backoff.attempt >= max_attempts {
                    self.disable();
                    LifecycleAction::Disabled
                } else {
                    let delay =
                        Duration::from_millis(base_delay_ms * u64::from(self.backoff.attempt));
                    self.backoff.until = Some(Instant::now() + delay);
                    LifecycleAction::BackingOff { duration: delay }
                }
            }
            BackoffPolicy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                max_attempts,
            } => {
                self.backoff.attempt += 1;
                if self.backoff.attempt >= max_attempts {
                    self.disable();
                    LifecycleAction::Disabled
                } else {
                    let multiplier = 1u64 << self.backoff.attempt.min(30);
                    let delay_ms = initial_delay_ms.saturating_mul(multiplier).min(max_delay_ms);
                    let delay = Duration::from_millis(delay_ms);
                    self.backoff.until = Some(Instant::now() + delay);
                    LifecycleAction::BackingOff { duration: delay }
                }
            }
        }
    }

    /// Enable the plugin.
    pub fn enable(&mut self) {
        self.enabled = true;
        self.state = PluginState::Ready;
        self.backoff.reset();
        self.last_state_change = Instant::now();
    }

    /// Disable the plugin.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.state = PluginState::Error;
        self.last_state_change = Instant::now();
    }

    /// Pause the plugin (temporary disable, keeps backoff state).
    pub fn pause(&mut self) {
        if self.state == PluginState::Ready {
            self.state = PluginState::Paused;
            self.last_state_change = Instant::now();
        }
    }

    /// Resume the plugin from paused state.
    pub fn resume(&mut self) {
        if self.state == PluginState::Paused {
            self.state = PluginState::Ready;
            self.last_state_change = Instant::now();
        }
    }

    /// Get the current metrics.
    pub fn metrics(&self) -> &PluginMetrics {
        &self.metrics
    }

    /// Get mutable access to metrics (for runtime updates).
    pub fn metrics_mut(&mut self) -> &mut PluginMetrics {
        &mut self.metrics
    }

    /// Update the budget configuration.
    pub fn set_budget(&mut self, budget: PluginBudget) {
        self.budget = budget;
    }

    /// Get time since last state change.
    pub fn time_in_state(&self) -> Duration {
        self.last_state_change.elapsed()
    }
}

/// Types of budget violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetViolation {
    /// Processing took too long.
    Timeout,
    /// WASM fuel was exhausted.
    FuelExhausted,
    /// Too many events per second.
    RateLimited,
}

/// Action to take after a lifecycle event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    /// Continue normal operation.
    Continue,
    /// Plugin is backing off.
    BackingOff {
        /// How long the backoff will last.
        duration: Duration,
    },
    /// Plugin has been disabled.
    Disabled,
}

/// Backoff state tracking.
#[derive(Debug, Default)]
struct BackoffState {
    /// Current attempt count.
    attempt: u32,
    /// Time until backoff ends.
    until: Option<Instant>,
}

impl BackoffState {
    fn is_backing_off(&self) -> bool {
        self.until.is_some_and(|t| t > Instant::now())
    }

    fn reset(&mut self) {
        self.attempt = 0;
        self.until = None;
    }
}

/// Rolling window for rate limiting.
#[derive(Debug)]
struct RateWindow {
    /// Window duration.
    window: Duration,
    /// Event timestamps in the current window.
    events: Vec<Instant>,
    /// Last cleanup time.
    last_cleanup: Instant,
}

impl RateWindow {
    fn new(window: Duration) -> Self {
        Self {
            window,
            events: Vec::with_capacity(100),
            last_cleanup: Instant::now(),
        }
    }

    fn record_event(&mut self) {
        let now = Instant::now();
        self.events.push(now);

        // Periodic cleanup
        if now.duration_since(self.last_cleanup) > self.window {
            self.cleanup(now);
        }
    }

    fn is_rate_limited(&self, max_events: u32) -> bool {
        let now = Instant::now();
        // If checked_sub fails (window > time since process start), count all events
        let count = match now.checked_sub(self.window) {
            Some(cutoff) => self.events.iter().filter(|&&t| t > cutoff).count(),
            None => self.events.len(),
        };
        count >= max_events as usize
    }

    fn cleanup(&mut self, now: Instant) {
        // If checked_sub fails, keep all events (process just started)
        if let Some(cutoff) = now.checked_sub(self.window) {
            self.events.retain(|&t| t > cutoff);
        }
        self.last_cleanup = now;
    }
}

/// Manages lifecycle for multiple plugins.
pub struct LifecycleManager {
    /// Per-plugin lifecycle state.
    plugins: RwLock<HashMap<PluginId, PluginLifecycle>>,
    /// Default budget for new plugins.
    default_budget: PluginBudget,
    /// Global enable state.
    global_enabled: AtomicU32, // 0 = disabled, 1 = enabled
}

impl LifecycleManager {
    /// Create a new lifecycle manager.
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            default_budget: PluginBudget::default(),
            global_enabled: AtomicU32::new(1),
        }
    }

    /// Create with custom default budget.
    pub fn with_default_budget(budget: PluginBudget) -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            default_budget: budget,
            global_enabled: AtomicU32::new(1),
        }
    }

    /// Register a new plugin.
    pub fn register(&self, id: PluginId) {
        let lifecycle = PluginLifecycle::with_budget(id, self.default_budget.clone());
        self.plugins.write().insert(id, lifecycle);
    }

    /// Register with custom budget.
    pub fn register_with_budget(&self, id: PluginId, budget: PluginBudget) {
        let lifecycle = PluginLifecycle::with_budget(id, budget);
        self.plugins.write().insert(id, lifecycle);
    }

    /// Unregister a plugin.
    pub fn unregister(&self, id: PluginId) {
        self.plugins.write().remove(&id);
    }

    /// Check if a plugin can process an event.
    pub fn can_process(&self, id: PluginId) -> bool {
        if self.global_enabled.load(Ordering::Relaxed) == 0 {
            return false;
        }

        self.plugins
            .read()
            .get(&id)
            .is_some_and(|p| p.can_process())
    }

    /// Record successful processing.
    pub fn record_success(&self, id: PluginId, processing_time: Duration) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.record_success(processing_time);
        }
    }

    /// Record a budget violation.
    pub fn record_violation(&self, id: PluginId, violation: BudgetViolation) -> LifecycleAction {
        self.plugins
            .write()
            .get_mut(&id)
            .map_or(LifecycleAction::Disabled, |p| p.record_violation(violation))
    }

    /// Enable a specific plugin.
    pub fn enable(&self, id: PluginId) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.enable();
        }
    }

    /// Disable a specific plugin.
    pub fn disable(&self, id: PluginId) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.disable();
        }
    }

    /// Pause a plugin.
    pub fn pause(&self, id: PluginId) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.pause();
        }
    }

    /// Resume a plugin.
    pub fn resume(&self, id: PluginId) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.resume();
        }
    }

    /// Enable all plugins globally.
    pub fn enable_all(&self) {
        self.global_enabled.store(1, Ordering::Relaxed);
    }

    /// Disable all plugins globally.
    pub fn disable_all(&self) {
        self.global_enabled.store(0, Ordering::Relaxed);
    }

    /// Check if plugins are globally enabled.
    pub fn is_globally_enabled(&self) -> bool {
        self.global_enabled.load(Ordering::Relaxed) == 1
    }

    /// Get metrics for a plugin.
    pub fn metrics(&self, id: PluginId) -> Option<PluginMetrics> {
        self.plugins.read().get(&id).map(|p| p.metrics().clone())
    }

    /// Get all plugin IDs.
    pub fn plugin_ids(&self) -> Vec<PluginId> {
        self.plugins.read().keys().copied().collect()
    }

    /// Get plugin state.
    pub fn state(&self, id: PluginId) -> Option<PluginState> {
        self.plugins.read().get(&id).map(|p| p.state)
    }

    /// Update budget for a plugin.
    pub fn set_budget(&self, id: PluginId, budget: PluginBudget) {
        if let Some(lifecycle) = self.plugins.write().get_mut(&id) {
            lifecycle.set_budget(budget);
        }
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_creation() {
        let lifecycle = PluginLifecycle::new(PluginId(1));
        assert!(lifecycle.enabled);
        assert_eq!(lifecycle.state, PluginState::Ready);
        assert!(lifecycle.can_process());
    }

    #[test]
    fn test_enable_disable() {
        let mut lifecycle = PluginLifecycle::new(PluginId(1));

        lifecycle.disable();
        assert!(!lifecycle.enabled);
        assert!(!lifecycle.can_process());

        lifecycle.enable();
        assert!(lifecycle.enabled);
        assert!(lifecycle.can_process());
    }

    #[test]
    fn test_pause_resume() {
        let mut lifecycle = PluginLifecycle::new(PluginId(1));

        lifecycle.pause();
        assert_eq!(lifecycle.state, PluginState::Paused);
        assert!(!lifecycle.can_process());

        lifecycle.resume();
        assert_eq!(lifecycle.state, PluginState::Ready);
        assert!(lifecycle.can_process());
    }

    #[test]
    fn test_record_success() {
        let mut lifecycle = PluginLifecycle::new(PluginId(1));

        lifecycle.record_success(Duration::from_millis(10));
        assert_eq!(lifecycle.metrics().events_processed, 1);
        assert!(lifecycle.metrics().total_processing_us >= 10_000);
    }

    #[test]
    fn test_immediate_backoff() {
        let mut lifecycle = PluginLifecycle::with_budget(
            PluginId(1),
            PluginBudget {
                backoff_policy: BackoffPolicy::Immediate,
                ..Default::default()
            },
        );

        let action = lifecycle.record_violation(BudgetViolation::Timeout);
        assert_eq!(action, LifecycleAction::Disabled);
        assert!(!lifecycle.enabled);
    }

    #[test]
    fn test_exponential_backoff() {
        let mut lifecycle = PluginLifecycle::with_budget(
            PluginId(1),
            PluginBudget {
                backoff_policy: BackoffPolicy::Exponential {
                    initial_delay_ms: 100,
                    max_delay_ms: 10000,
                    max_attempts: 3,
                },
                ..Default::default()
            },
        );

        // First violation: should backoff
        let action = lifecycle.record_violation(BudgetViolation::Timeout);
        assert!(matches!(action, LifecycleAction::BackingOff { .. }));
        assert!(lifecycle.enabled);
        assert!(!lifecycle.can_process()); // Backing off

        // Wait for backoff
        std::thread::sleep(Duration::from_millis(250));

        // Second violation
        let action = lifecycle.record_violation(BudgetViolation::Timeout);
        assert!(matches!(action, LifecycleAction::BackingOff { .. }));

        // Wait and third violation: should disable
        std::thread::sleep(Duration::from_millis(500));
        let action = lifecycle.record_violation(BudgetViolation::Timeout);
        assert_eq!(action, LifecycleAction::Disabled);
        assert!(!lifecycle.enabled);
    }

    #[test]
    fn test_success_resets_backoff() {
        let mut lifecycle = PluginLifecycle::with_budget(
            PluginId(1),
            PluginBudget {
                backoff_policy: BackoffPolicy::Exponential {
                    initial_delay_ms: 100,
                    max_delay_ms: 10000,
                    max_attempts: 3,
                },
                ..Default::default()
            },
        );

        // Cause a violation
        lifecycle.record_violation(BudgetViolation::Timeout);
        assert!(!lifecycle.can_process());

        // Wait for backoff
        std::thread::sleep(Duration::from_millis(150));

        // Success resets backoff
        lifecycle.record_success(Duration::from_millis(5));
        assert!(lifecycle.can_process());
    }

    #[test]
    fn test_manager_basic() {
        let manager = LifecycleManager::new();

        manager.register(PluginId(1));
        assert!(manager.can_process(PluginId(1)));

        manager.disable(PluginId(1));
        assert!(!manager.can_process(PluginId(1)));

        manager.enable(PluginId(1));
        assert!(manager.can_process(PluginId(1)));
    }

    #[test]
    fn test_manager_global_disable() {
        let manager = LifecycleManager::new();

        manager.register(PluginId(1));
        manager.register(PluginId(2));

        assert!(manager.can_process(PluginId(1)));
        assert!(manager.can_process(PluginId(2)));

        manager.disable_all();
        assert!(!manager.can_process(PluginId(1)));
        assert!(!manager.can_process(PluginId(2)));

        manager.enable_all();
        assert!(manager.can_process(PluginId(1)));
        assert!(manager.can_process(PluginId(2)));
    }

    #[test]
    fn test_manager_unregister() {
        let manager = LifecycleManager::new();

        manager.register(PluginId(1));
        assert!(manager.can_process(PluginId(1)));

        manager.unregister(PluginId(1));
        assert!(!manager.can_process(PluginId(1)));
    }
}
