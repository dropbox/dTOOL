// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Scheduler performance metrics

use std::time::Duration;

/// Performance metrics for the work-stealing scheduler
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    /// Total tasks submitted to scheduler
    pub tasks_submitted: u64,
    /// Tasks executed locally
    pub tasks_executed_local: u64,
    /// Tasks executed remotely
    pub tasks_executed_remote: u64,
    /// Tasks stolen by workers (future work)
    pub tasks_stolen: u64,
    /// Total time spent executing locally
    pub execution_time_local: Duration,
    /// Total time spent executing remotely
    pub execution_time_remote: Duration,
    /// Time spent distributing tasks to workers
    pub task_distribution_latency: Duration,
}

impl SchedulerMetrics {
    /// Create new metrics instance
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate average local execution time per task
    #[must_use]
    pub fn avg_local_execution_time(&self) -> Option<Duration> {
        if self.tasks_executed_local == 0 {
            None
        } else {
            // Use checked conversion to handle potential u32 overflow for very large task counts
            let divisor = u32::try_from(self.tasks_executed_local).unwrap_or(u32::MAX);
            Some(self.execution_time_local / divisor)
        }
    }

    /// Calculate average remote execution time per task
    #[must_use]
    pub fn avg_remote_execution_time(&self) -> Option<Duration> {
        if self.tasks_executed_remote == 0 {
            None
        } else {
            // Use checked conversion to handle potential u32 overflow for very large task counts
            let divisor = u32::try_from(self.tasks_executed_remote).unwrap_or(u32::MAX);
            Some(self.execution_time_remote / divisor)
        }
    }

    /// Calculate percentage of tasks executed remotely
    #[must_use]
    pub fn remote_execution_ratio(&self) -> f64 {
        let total = self.tasks_executed_local + self.tasks_executed_remote;
        if total == 0 {
            0.0
        } else {
            (self.tasks_executed_remote as f64) / (total as f64)
        }
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metrics() {
        let metrics = SchedulerMetrics::new();

        assert_eq!(metrics.tasks_submitted, 0);
        assert_eq!(metrics.tasks_executed_local, 0);
        assert_eq!(metrics.tasks_executed_remote, 0);
        assert_eq!(metrics.tasks_stolen, 0);
        assert_eq!(metrics.execution_time_local, Duration::ZERO);
        assert_eq!(metrics.execution_time_remote, Duration::ZERO);
        assert_eq!(metrics.task_distribution_latency, Duration::ZERO);
    }

    #[test]
    fn test_avg_local_execution_time() {
        let mut metrics = SchedulerMetrics::new();

        // Initially no tasks executed
        assert_eq!(metrics.avg_local_execution_time(), None);

        // Add some local execution data
        metrics.tasks_executed_local = 5;
        metrics.execution_time_local = Duration::from_millis(500);

        let avg = metrics.avg_local_execution_time().unwrap();
        assert_eq!(avg, Duration::from_millis(100)); // 500ms / 5 tasks
    }

    #[test]
    fn test_avg_remote_execution_time() {
        let mut metrics = SchedulerMetrics::new();

        // Initially no tasks executed
        assert_eq!(metrics.avg_remote_execution_time(), None);

        // Add some remote execution data
        metrics.tasks_executed_remote = 10;
        metrics.execution_time_remote = Duration::from_secs(2);

        let avg = metrics.avg_remote_execution_time().unwrap();
        assert_eq!(avg, Duration::from_millis(200)); // 2000ms / 10 tasks
    }

    #[test]
    fn test_remote_execution_ratio_no_tasks() {
        let metrics = SchedulerMetrics::new();

        // No tasks executed → ratio is 0.0
        assert_eq!(metrics.remote_execution_ratio(), 0.0);
    }

    #[test]
    fn test_remote_execution_ratio_all_local() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_executed_local = 10;
        metrics.tasks_executed_remote = 0;

        // All local → ratio is 0.0
        assert_eq!(metrics.remote_execution_ratio(), 0.0);
    }

    #[test]
    fn test_remote_execution_ratio_all_remote() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_executed_local = 0;
        metrics.tasks_executed_remote = 10;

        // All remote → ratio is 1.0
        assert_eq!(metrics.remote_execution_ratio(), 1.0);
    }

    #[test]
    fn test_remote_execution_ratio_mixed() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_executed_local = 7;
        metrics.tasks_executed_remote = 3;

        // 3 remote out of 10 total → ratio is 0.3
        assert_eq!(metrics.remote_execution_ratio(), 0.3);
    }

    #[test]
    fn test_remote_execution_ratio_equal_split() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_executed_local = 50;
        metrics.tasks_executed_remote = 50;

        // Equal split → ratio is 0.5
        assert_eq!(metrics.remote_execution_ratio(), 0.5);
    }

    #[test]
    fn test_reset() {
        let mut metrics = SchedulerMetrics::new();

        // Populate with data
        metrics.tasks_submitted = 100;
        metrics.tasks_executed_local = 70;
        metrics.tasks_executed_remote = 30;
        metrics.tasks_stolen = 5;
        metrics.execution_time_local = Duration::from_secs(10);
        metrics.execution_time_remote = Duration::from_secs(5);
        metrics.task_distribution_latency = Duration::from_millis(100);

        // Reset
        metrics.reset();

        // All fields should be zero
        assert_eq!(metrics.tasks_submitted, 0);
        assert_eq!(metrics.tasks_executed_local, 0);
        assert_eq!(metrics.tasks_executed_remote, 0);
        assert_eq!(metrics.tasks_stolen, 0);
        assert_eq!(metrics.execution_time_local, Duration::ZERO);
        assert_eq!(metrics.execution_time_remote, Duration::ZERO);
        assert_eq!(metrics.task_distribution_latency, Duration::ZERO);
    }

    #[test]
    fn test_metrics_clone() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_submitted = 50;
        metrics.tasks_executed_local = 30;
        metrics.tasks_executed_remote = 20;
        metrics.execution_time_local = Duration::from_millis(300);
        metrics.execution_time_remote = Duration::from_millis(200);

        let cloned = metrics.clone();

        // Verify all fields match
        assert_eq!(cloned.tasks_submitted, metrics.tasks_submitted);
        assert_eq!(cloned.tasks_executed_local, metrics.tasks_executed_local);
        assert_eq!(cloned.tasks_executed_remote, metrics.tasks_executed_remote);
        assert_eq!(cloned.execution_time_local, metrics.execution_time_local);
        assert_eq!(cloned.execution_time_remote, metrics.execution_time_remote);
    }

    #[test]
    fn test_metrics_debug_format() {
        let mut metrics = SchedulerMetrics::new();
        metrics.tasks_submitted = 10;
        metrics.tasks_executed_local = 5;

        let debug_str = format!("{:?}", metrics);

        // Verify debug output contains key information
        assert!(debug_str.contains("SchedulerMetrics"));
        assert!(debug_str.contains("tasks_submitted"));
    }

    #[test]
    fn test_avg_execution_time_single_task() {
        let mut metrics = SchedulerMetrics::new();

        // Single local task
        metrics.tasks_executed_local = 1;
        metrics.execution_time_local = Duration::from_millis(150);

        let avg = metrics.avg_local_execution_time().unwrap();
        assert_eq!(avg, Duration::from_millis(150));
    }

    #[test]
    fn test_avg_execution_time_large_numbers() {
        let mut metrics = SchedulerMetrics::new();

        // Large number of tasks
        metrics.tasks_executed_remote = 1000;
        metrics.execution_time_remote = Duration::from_secs(100);

        let avg = metrics.avg_remote_execution_time().unwrap();
        assert_eq!(avg, Duration::from_millis(100)); // 100,000ms / 1000 tasks
    }

    #[test]
    fn test_default_trait() {
        let metrics = SchedulerMetrics::default();

        // Default should match new()
        assert_eq!(metrics.tasks_submitted, 0);
        assert_eq!(metrics.tasks_executed_local, 0);
        assert_eq!(metrics.tasks_executed_remote, 0);
    }

    #[test]
    fn test_avg_execution_time_u32_overflow_protection() {
        let mut metrics = SchedulerMetrics::new();

        // Test with task count exceeding u32::MAX
        // This should not panic and should use u32::MAX as divisor
        metrics.tasks_executed_local = u64::from(u32::MAX) + 1000;
        metrics.execution_time_local = Duration::from_secs(u32::MAX as u64);

        // Should not panic, uses u32::MAX as divisor
        let avg_local = metrics.avg_local_execution_time();
        assert!(avg_local.is_some());

        // Remote test
        metrics.tasks_executed_remote = u64::from(u32::MAX) + 500;
        metrics.execution_time_remote = Duration::from_secs(u32::MAX as u64);

        let avg_remote = metrics.avg_remote_execution_time();
        assert!(avg_remote.is_some());
    }

    #[test]
    fn test_avg_execution_time_exact_u32_max() {
        let mut metrics = SchedulerMetrics::new();

        // Exactly u32::MAX tasks - should work without overflow
        metrics.tasks_executed_local = u64::from(u32::MAX);
        metrics.execution_time_local = Duration::from_secs(u32::MAX as u64);

        let avg = metrics.avg_local_execution_time();
        assert!(avg.is_some());
        // Should be approximately 1 second per task
        assert_eq!(avg.unwrap(), Duration::from_secs(1));
    }
}
