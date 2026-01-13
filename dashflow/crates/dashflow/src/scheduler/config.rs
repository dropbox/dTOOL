// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Scheduler configuration

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum local queue size before distributing to workers
    pub local_queue_threshold: usize,
    /// Enable work stealing between workers
    pub enable_stealing: bool,
    /// Number of steal attempts per cycle
    pub steal_attempts: usize,
    /// Worker selection strategy
    pub selection_strategy: SelectionStrategy,
    /// M-572: Optional random seed for deterministic Random selection strategy
    /// If None, uses thread_rng() for non-deterministic randomness
    /// If Some(seed), uses StdRng seeded with the given value
    pub random_seed: Option<u64>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            local_queue_threshold: 10,
            enable_stealing: true,
            steal_attempts: 3,
            selection_strategy: SelectionStrategy::LeastLoaded,
            random_seed: None,
        }
    }
}

impl SchedulerConfig {
    /// Create a new scheduler configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the local queue threshold (max tasks before distributing to workers).
    #[must_use]
    pub fn with_local_queue_threshold(mut self, threshold: usize) -> Self {
        self.local_queue_threshold = threshold;
        self
    }

    /// Enable or disable work stealing between workers.
    #[must_use]
    pub fn with_stealing(mut self, enabled: bool) -> Self {
        self.enable_stealing = enabled;
        self
    }

    /// Set the number of steal attempts per cycle.
    #[must_use]
    pub fn with_steal_attempts(mut self, attempts: usize) -> Self {
        self.steal_attempts = attempts;
        self
    }

    /// Set the worker selection strategy.
    #[must_use]
    pub fn with_selection_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.selection_strategy = strategy;
        self
    }

    /// Set a random seed for deterministic Random selection strategy.
    #[must_use]
    pub fn with_random_seed(mut self, seed: Option<u64>) -> Self {
        self.random_seed = seed;
        self
    }
}

/// Strategy for selecting workers to execute tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Select worker randomly
    Random,
    /// Select worker with least current load
    LeastLoaded,
    /// Select workers in round-robin order
    RoundRobin,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();

        assert_eq!(config.local_queue_threshold, 10);
        assert!(config.enable_stealing);
        assert_eq!(config.steal_attempts, 3);
        assert_eq!(config.selection_strategy, SelectionStrategy::LeastLoaded);
    }

    #[test]
    fn test_scheduler_config_clone() {
        let config = SchedulerConfig {
            local_queue_threshold: 20,
            enable_stealing: false,
            steal_attempts: 5,
            selection_strategy: SelectionStrategy::Random,
            random_seed: Some(42),
        };

        let cloned = config.clone();

        assert_eq!(cloned.local_queue_threshold, config.local_queue_threshold);
        assert_eq!(cloned.enable_stealing, config.enable_stealing);
        assert_eq!(cloned.steal_attempts, config.steal_attempts);
        assert_eq!(cloned.selection_strategy, config.selection_strategy);
        assert_eq!(cloned.random_seed, config.random_seed);
    }

    #[test]
    fn test_scheduler_config_debug_format() {
        let config = SchedulerConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("SchedulerConfig"));
        assert!(debug_str.contains("local_queue_threshold"));
    }

    #[test]
    fn test_selection_strategy_equality() {
        assert_eq!(SelectionStrategy::Random, SelectionStrategy::Random);
        assert_eq!(
            SelectionStrategy::LeastLoaded,
            SelectionStrategy::LeastLoaded
        );
        assert_eq!(SelectionStrategy::RoundRobin, SelectionStrategy::RoundRobin);

        assert_ne!(SelectionStrategy::Random, SelectionStrategy::LeastLoaded);
        assert_ne!(
            SelectionStrategy::LeastLoaded,
            SelectionStrategy::RoundRobin
        );
        assert_ne!(SelectionStrategy::Random, SelectionStrategy::RoundRobin);
    }

    #[test]
    fn test_selection_strategy_clone() {
        let strategy = SelectionStrategy::LeastLoaded;
        let cloned = strategy;

        assert_eq!(cloned, strategy);
    }

    #[test]
    fn test_selection_strategy_debug_format() {
        let random = format!("{:?}", SelectionStrategy::Random);
        let least_loaded = format!("{:?}", SelectionStrategy::LeastLoaded);
        let round_robin = format!("{:?}", SelectionStrategy::RoundRobin);

        assert_eq!(random, "Random");
        assert_eq!(least_loaded, "LeastLoaded");
        assert_eq!(round_robin, "RoundRobin");
    }

    #[test]
    fn test_scheduler_config_new() {
        let config = SchedulerConfig::new();
        assert_eq!(config.local_queue_threshold, 10);
        assert!(config.enable_stealing);
        assert_eq!(config.steal_attempts, 3);
        assert_eq!(config.selection_strategy, SelectionStrategy::LeastLoaded);
        assert_eq!(config.random_seed, None);
    }

    #[test]
    fn test_scheduler_config_builder_local_queue_threshold() {
        let config = SchedulerConfig::new().with_local_queue_threshold(50);
        assert_eq!(config.local_queue_threshold, 50);
    }

    #[test]
    fn test_scheduler_config_builder_stealing() {
        let config = SchedulerConfig::new().with_stealing(false);
        assert!(!config.enable_stealing);
    }

    #[test]
    fn test_scheduler_config_builder_steal_attempts() {
        let config = SchedulerConfig::new().with_steal_attempts(10);
        assert_eq!(config.steal_attempts, 10);
    }

    #[test]
    fn test_scheduler_config_builder_selection_strategy() {
        let config = SchedulerConfig::new().with_selection_strategy(SelectionStrategy::RoundRobin);
        assert_eq!(config.selection_strategy, SelectionStrategy::RoundRobin);
    }

    #[test]
    fn test_scheduler_config_builder_random_seed() {
        let config = SchedulerConfig::new().with_random_seed(Some(12345));
        assert_eq!(config.random_seed, Some(12345));
    }

    #[test]
    fn test_scheduler_config_builder_chaining() {
        let config = SchedulerConfig::new()
            .with_local_queue_threshold(25)
            .with_stealing(false)
            .with_steal_attempts(7)
            .with_selection_strategy(SelectionStrategy::Random)
            .with_random_seed(Some(42));

        assert_eq!(config.local_queue_threshold, 25);
        assert!(!config.enable_stealing);
        assert_eq!(config.steal_attempts, 7);
        assert_eq!(config.selection_strategy, SelectionStrategy::Random);
        assert_eq!(config.random_seed, Some(42));
    }
}
