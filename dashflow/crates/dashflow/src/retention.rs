// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Checkpoint retention policies for automatic cleanup
//!
//! Retention policies define rules for automatically cleaning up old checkpoints
//! to manage storage costs and prevent unbounded growth.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::retention::RetentionPolicy;
//! use std::time::Duration;
//!
//! let policy = RetentionPolicy::builder()
//!     .keep_last_n(10)              // Keep last 10 checkpoints always
//!     .keep_daily_for(Duration::from_secs(30 * 86400))  // Keep one per day for 30 days
//!     .keep_weekly_for(Duration::from_secs(12 * 7 * 86400))  // Keep one per week for 12 weeks
//!     .delete_after(Duration::from_secs(90 * 86400))  // Delete everything older than 90 days
//!     .build();
//! ```

use crate::checkpoint::CheckpointMetadata;
use crate::constants::SECONDS_PER_DAY;
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

/// Retention policy for automatic checkpoint cleanup
///
/// Policies are applied in this priority order:
/// 1. Keep the latest N checkpoints (always)
/// 2. Keep daily checkpoints within time window
/// 3. Keep weekly checkpoints within time window
/// 4. Delete everything older than max age
///
/// A checkpoint is kept if it matches ANY retention rule.
#[derive(Clone, Debug, PartialEq)]
pub struct RetentionPolicy {
    /// Always keep the last N checkpoints (regardless of age)
    pub keep_last_n: Option<usize>,

    /// Keep one checkpoint per day for this duration
    pub keep_daily_for: Option<Duration>,

    /// Keep one checkpoint per week for this duration
    pub keep_weekly_for: Option<Duration>,

    /// Delete all checkpoints older than this
    pub delete_after: Option<Duration>,
}

impl RetentionPolicy {
    /// Create a new builder for retention policies
    #[must_use]
    pub fn builder() -> RetentionPolicyBuilder {
        RetentionPolicyBuilder::default()
    }

    /// Evaluate which checkpoints should be kept vs deleted
    ///
    /// Returns two sets:
    /// - `to_keep`: Checkpoint IDs to keep
    /// - `to_delete`: Checkpoint IDs to delete
    ///
    /// # Arguments
    /// - `checkpoints`: All checkpoints for a thread (must be sorted newest first)
    /// - `now`: Current time for age calculations
    #[must_use]
    pub fn evaluate(
        &self,
        checkpoints: &[CheckpointMetadata],
        now: SystemTime,
    ) -> (HashSet<String>, HashSet<String>) {
        let mut to_keep = HashSet::new();
        let mut to_delete = HashSet::new();

        // Rule 1: Keep last N checkpoints (highest priority)
        if let Some(n) = self.keep_last_n {
            for checkpoint in checkpoints.iter().take(n) {
                to_keep.insert(checkpoint.id.clone());
            }
        }

        // Rule 2: Delete everything older than max age (if specified)
        if let Some(max_age) = self.delete_after {
            let cutoff = now.checked_sub(max_age).unwrap_or(SystemTime::UNIX_EPOCH);
            for checkpoint in checkpoints {
                if checkpoint.timestamp < cutoff {
                    // Only delete if not already kept by rule 1
                    if !to_keep.contains(&checkpoint.id) {
                        to_delete.insert(checkpoint.id.clone());
                    }
                }
            }
        }

        // Rule 3: Keep daily checkpoints
        if let Some(duration) = self.keep_daily_for {
            let cutoff = now.checked_sub(duration).unwrap_or(SystemTime::UNIX_EPOCH);
            let mut daily_kept = HashSet::new();

            for checkpoint in checkpoints {
                // Skip checkpoints before cutoff
                if checkpoint.timestamp < cutoff {
                    continue;
                }
                // Get day bucket (skip if timestamp is before epoch)
                let Ok(duration_since_epoch) =
                    checkpoint.timestamp.duration_since(SystemTime::UNIX_EPOCH)
                else {
                    continue;
                };
                let day = duration_since_epoch.as_secs() / SECONDS_PER_DAY;

                // Keep first checkpoint of each day
                if daily_kept.insert(day) {
                    to_keep.insert(checkpoint.id.clone());
                    to_delete.remove(&checkpoint.id);
                }
            }
        }

        // Rule 4: Keep weekly checkpoints
        if let Some(duration) = self.keep_weekly_for {
            let cutoff = now.checked_sub(duration).unwrap_or(SystemTime::UNIX_EPOCH);
            let mut weekly_kept = HashSet::new();

            for checkpoint in checkpoints {
                // Skip checkpoints before cutoff
                if checkpoint.timestamp < cutoff {
                    continue;
                }
                // Get week bucket (skip if timestamp is before epoch)
                let Ok(duration_since_epoch) =
                    checkpoint.timestamp.duration_since(SystemTime::UNIX_EPOCH)
                else {
                    continue;
                };
                // ISO week starts on Monday; days since epoch / 7 gives week number
                let week = duration_since_epoch.as_secs() / (7 * SECONDS_PER_DAY);

                // Keep first checkpoint of each week
                if weekly_kept.insert(week) {
                    to_keep.insert(checkpoint.id.clone());
                    to_delete.remove(&checkpoint.id);
                }
            }
        }

        // If no delete rules specified, default to keeping everything not explicitly marked for deletion
        if self.delete_after.is_none() {
            // Only return explicit deletes
            (to_keep, to_delete)
        } else {
            // Mark everything not kept for deletion (if beyond max age)
            for checkpoint in checkpoints {
                if !to_keep.contains(&checkpoint.id) {
                    if let Some(max_age) = self.delete_after {
                        let cutoff = now.checked_sub(max_age).unwrap_or(SystemTime::UNIX_EPOCH);
                        if checkpoint.timestamp < cutoff {
                            to_delete.insert(checkpoint.id.clone());
                        }
                    }
                }
            }
            (to_keep, to_delete)
        }
    }
}

/// Builder for `RetentionPolicy`
#[derive(Default, Debug)]
pub struct RetentionPolicyBuilder {
    keep_last_n: Option<usize>,
    keep_daily_for: Option<Duration>,
    keep_weekly_for: Option<Duration>,
    delete_after: Option<Duration>,
}

impl RetentionPolicyBuilder {
    /// Always keep the last N checkpoints (regardless of age)
    #[must_use]
    pub fn keep_last_n(mut self, n: usize) -> Self {
        self.keep_last_n = Some(n);
        self
    }

    /// Keep one checkpoint per day for this duration
    #[must_use]
    pub fn keep_daily_for(mut self, duration: Duration) -> Self {
        self.keep_daily_for = Some(duration);
        self
    }

    /// Keep one checkpoint per week for this duration
    #[must_use]
    pub fn keep_weekly_for(mut self, duration: Duration) -> Self {
        self.keep_weekly_for = Some(duration);
        self
    }

    /// Delete all checkpoints older than this
    #[must_use]
    pub fn delete_after(mut self, duration: Duration) -> Self {
        self.delete_after = Some(duration);
        self
    }

    /// Build the retention policy
    #[must_use]
    pub fn build(self) -> RetentionPolicy {
        RetentionPolicy {
            keep_last_n: self.keep_last_n,
            keep_daily_for: self.keep_daily_for,
            keep_weekly_for: self.keep_weekly_for,
            delete_after: self.delete_after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::CheckpointMetadata;
    use std::collections::HashMap;

    fn make_checkpoint(id: &str, thread_id: &str, days_ago: u64) -> CheckpointMetadata {
        let now = SystemTime::now();
        make_checkpoint_at(id, thread_id, days_ago, now)
    }

    fn make_checkpoint_at(
        id: &str,
        thread_id: &str,
        days_ago: u64,
        base_time: SystemTime,
    ) -> CheckpointMetadata {
        let timestamp = base_time
            .checked_sub(Duration::from_secs(days_ago * 86400))
            .unwrap();

        CheckpointMetadata {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            node: "test_node".to_string(),
            timestamp,
            parent_id: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_keep_last_n() {
        let policy = RetentionPolicy::builder().keep_last_n(3).build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0), // newest
            make_checkpoint("cp2", "thread1", 1),
            make_checkpoint("cp3", "thread1", 2),
            make_checkpoint("cp4", "thread1", 3),
            make_checkpoint("cp5", "thread1", 4), // oldest
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Should keep the 3 newest
        assert!(to_keep.contains("cp1"));
        assert!(to_keep.contains("cp2"));
        assert!(to_keep.contains("cp3"));
        assert_eq!(to_keep.len(), 3);

        // Without delete_after, we don't mark others for deletion
        assert_eq!(to_delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n_with_delete_after() {
        let policy = RetentionPolicy::builder()
            .keep_last_n(3)
            .delete_after(Duration::from_secs(10 * 86400)) // 10 days
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0), // newest
            make_checkpoint("cp2", "thread1", 1),
            make_checkpoint("cp3", "thread1", 2),
            make_checkpoint("cp4", "thread1", 15), // older than 10 days
            make_checkpoint("cp5", "thread1", 20), // older than 10 days
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Should keep the 3 newest
        assert!(to_keep.contains("cp1"));
        assert!(to_keep.contains("cp2"));
        assert!(to_keep.contains("cp3"));
        assert_eq!(to_keep.len(), 3);

        // Should delete the ones older than 10 days (and not in last 3)
        assert!(to_delete.contains("cp4"));
        assert!(to_delete.contains("cp5"));
        assert_eq!(to_delete.len(), 2);
    }

    #[test]
    fn test_delete_after() {
        let policy = RetentionPolicy::builder()
            .delete_after(Duration::from_secs(30 * 86400)) // 30 days
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 10), // within 30 days
            make_checkpoint("cp2", "thread1", 20), // within 30 days
            make_checkpoint("cp3", "thread1", 40), // older than 30 days
            make_checkpoint("cp4", "thread1", 50), // older than 30 days
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Should keep nothing (no keep rules)
        assert_eq!(to_keep.len(), 0);

        // Should delete the ones older than 30 days
        assert!(to_delete.contains("cp3"));
        assert!(to_delete.contains("cp4"));
        assert_eq!(to_delete.len(), 2);
    }

    #[test]
    fn test_daily_retention() {
        let policy = RetentionPolicy::builder()
            .keep_daily_for(Duration::from_secs(7 * 86400)) // 7 days
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),  // day 0
            make_checkpoint("cp2", "thread1", 0),  // day 0 (duplicate)
            make_checkpoint("cp3", "thread1", 1),  // day 1
            make_checkpoint("cp4", "thread1", 2),  // day 2
            make_checkpoint("cp5", "thread1", 10), // day 10 (outside window)
        ];

        let (to_keep, _to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Should keep first checkpoint of each day within window
        assert!(to_keep.contains("cp1")); // first of day 0
        assert!(!to_keep.contains("cp2")); // duplicate day 0
        assert!(to_keep.contains("cp3")); // day 1
        assert!(to_keep.contains("cp4")); // day 2
        assert!(!to_keep.contains("cp5")); // outside window

        // We have 3 daily checkpoints (day 0, 1, 2)
        assert_eq!(to_keep.len(), 3);
    }

    #[test]
    fn test_weekly_retention() {
        // Use fixed timestamp: Monday, January 1, 2024, 00:00:00 UTC (1704067200)
        // This ensures the test is deterministic and not dependent on when it runs
        let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1704067200);

        let policy = RetentionPolicy::builder()
            .keep_weekly_for(Duration::from_secs(4 * 7 * 86400)) // 4 weeks
            .build();

        let checkpoints = vec![
            make_checkpoint_at("cp1", "thread1", 0, base_time), // week 0 (current week)
            make_checkpoint_at("cp2", "thread1", 1, base_time), // week 0 (duplicate - same week)
            make_checkpoint_at("cp3", "thread1", 8, base_time), // week 1
            make_checkpoint_at("cp4", "thread1", 15, base_time), // week 2
            make_checkpoint_at("cp5", "thread1", 40, base_time), // week 5 (outside window)
        ];

        let (to_keep, _to_delete) = policy.evaluate(&checkpoints, base_time);

        // Should keep first checkpoint of each week within window
        assert!(to_keep.contains("cp1")); // first of week 0
        assert!(!to_keep.contains("cp2")); // duplicate week 0 (only 1 day difference, same week)
        assert!(to_keep.contains("cp3")); // week 1
        assert!(to_keep.contains("cp4")); // week 2
        assert!(!to_keep.contains("cp5")); // outside window

        assert_eq!(to_keep.len(), 3);
    }

    #[test]
    fn test_combined_policy() {
        let policy = RetentionPolicy::builder()
            .keep_last_n(2)
            .keep_daily_for(Duration::from_secs(10 * 86400))
            .delete_after(Duration::from_secs(30 * 86400))
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),  // newest, in last 2
            make_checkpoint("cp2", "thread1", 1),  // in last 2
            make_checkpoint("cp3", "thread1", 5),  // in daily window
            make_checkpoint("cp4", "thread1", 15), // outside daily window, within 30 days
            make_checkpoint("cp5", "thread1", 40), // older than 30 days
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // cp1, cp2: kept by keep_last_n
        // cp3: kept by daily retention
        // cp4: within 30 days but not kept by any rule
        // cp5: older than 30 days, should be deleted

        assert!(to_keep.contains("cp1"));
        assert!(to_keep.contains("cp2"));
        assert!(to_keep.contains("cp3"));
        assert!(!to_keep.contains("cp4"));
        assert!(!to_keep.contains("cp5"));

        assert!(to_delete.contains("cp5"));
        // cp4 is not deleted because it's within 30 days
        assert!(!to_delete.contains("cp4"));
    }

    #[test]
    fn test_empty_policy() {
        let policy = RetentionPolicy::builder().build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 100),
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // No retention rules, no deletion rules -> everything stays
        assert_eq!(to_keep.len(), 0);
        assert_eq!(to_delete.len(), 0);
    }

    // ===== RetentionPolicyBuilder tests =====

    #[test]
    fn test_builder_default() {
        let policy = RetentionPolicy::builder().build();
        assert_eq!(policy.keep_last_n, None);
        assert_eq!(policy.keep_daily_for, None);
        assert_eq!(policy.keep_weekly_for, None);
        assert_eq!(policy.delete_after, None);
    }

    #[test]
    fn test_builder_keep_last_n_only() {
        let policy = RetentionPolicy::builder().keep_last_n(5).build();
        assert_eq!(policy.keep_last_n, Some(5));
        assert_eq!(policy.keep_daily_for, None);
        assert_eq!(policy.keep_weekly_for, None);
        assert_eq!(policy.delete_after, None);
    }

    #[test]
    fn test_builder_keep_daily_for_only() {
        let duration = Duration::from_secs(30 * 86400);
        let policy = RetentionPolicy::builder().keep_daily_for(duration).build();
        assert_eq!(policy.keep_last_n, None);
        assert_eq!(policy.keep_daily_for, Some(duration));
        assert_eq!(policy.keep_weekly_for, None);
        assert_eq!(policy.delete_after, None);
    }

    #[test]
    fn test_builder_keep_weekly_for_only() {
        let duration = Duration::from_secs(12 * 7 * 86400);
        let policy = RetentionPolicy::builder().keep_weekly_for(duration).build();
        assert_eq!(policy.keep_last_n, None);
        assert_eq!(policy.keep_daily_for, None);
        assert_eq!(policy.keep_weekly_for, Some(duration));
        assert_eq!(policy.delete_after, None);
    }

    #[test]
    fn test_builder_delete_after_only() {
        let duration = Duration::from_secs(90 * 86400);
        let policy = RetentionPolicy::builder().delete_after(duration).build();
        assert_eq!(policy.keep_last_n, None);
        assert_eq!(policy.keep_daily_for, None);
        assert_eq!(policy.keep_weekly_for, None);
        assert_eq!(policy.delete_after, Some(duration));
    }

    #[test]
    fn test_builder_chaining() {
        let daily = Duration::from_secs(30 * 86400);
        let weekly = Duration::from_secs(12 * 7 * 86400);
        let max_age = Duration::from_secs(90 * 86400);

        let policy = RetentionPolicy::builder()
            .keep_last_n(10)
            .keep_daily_for(daily)
            .keep_weekly_for(weekly)
            .delete_after(max_age)
            .build();

        assert_eq!(policy.keep_last_n, Some(10));
        assert_eq!(policy.keep_daily_for, Some(daily));
        assert_eq!(policy.keep_weekly_for, Some(weekly));
        assert_eq!(policy.delete_after, Some(max_age));
    }

    // ===== RetentionPolicy trait tests =====

    #[test]
    fn test_retention_policy_clone() {
        let daily = Duration::from_secs(30 * 86400);
        let policy = RetentionPolicy::builder()
            .keep_last_n(5)
            .keep_daily_for(daily)
            .build();

        let cloned = policy.clone();
        assert_eq!(cloned.keep_last_n, Some(5));
        assert_eq!(cloned.keep_daily_for, Some(daily));
        assert_eq!(cloned.keep_weekly_for, None);
        assert_eq!(cloned.delete_after, None);
    }

    #[test]
    fn test_retention_policy_partial_eq() {
        let daily = Duration::from_secs(30 * 86400);

        let policy1 = RetentionPolicy::builder()
            .keep_last_n(5)
            .keep_daily_for(daily)
            .build();

        let policy2 = RetentionPolicy::builder()
            .keep_last_n(5)
            .keep_daily_for(daily)
            .build();

        let policy3 = RetentionPolicy::builder()
            .keep_last_n(10)
            .keep_daily_for(daily)
            .build();

        assert_eq!(policy1, policy2);
        assert_ne!(policy1, policy3);
    }

    // ===== Edge case tests =====

    #[test]
    fn test_evaluate_empty_checkpoints() {
        let policy = RetentionPolicy::builder().keep_last_n(5).build();
        let checkpoints = vec![];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        assert_eq!(to_keep.len(), 0);
        assert_eq!(to_delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n_zero() {
        let policy = RetentionPolicy::builder().keep_last_n(0).build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 1),
            make_checkpoint("cp3", "thread1", 2),
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // keep_last_n(0) keeps nothing
        assert_eq!(to_keep.len(), 0);
        assert_eq!(to_delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n_larger_than_checkpoints() {
        let policy = RetentionPolicy::builder().keep_last_n(100).build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 1),
            make_checkpoint("cp3", "thread1", 2),
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Should keep all 3 checkpoints (less than 100)
        assert_eq!(to_keep.len(), 3);
        assert_eq!(to_delete.len(), 0);
        assert!(to_keep.contains("cp1"));
        assert!(to_keep.contains("cp2"));
        assert!(to_keep.contains("cp3"));
    }

    #[test]
    fn test_delete_after_very_recent() {
        let policy = RetentionPolicy::builder()
            .delete_after(Duration::from_secs(1))
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 1),
            make_checkpoint("cp3", "thread1", 2),
        ];

        let (_to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // cp2 and cp3 are older than 1 second (1 day and 2 days ago)
        assert!(to_delete.contains("cp2"));
        assert!(to_delete.contains("cp3"));
        assert!(!to_delete.contains("cp1")); // cp1 is today
    }

    #[test]
    fn test_delete_after_very_old() {
        let policy = RetentionPolicy::builder()
            .delete_after(Duration::from_secs(1000 * 86400))
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 100),
            make_checkpoint("cp3", "thread1", 200),
        ];

        let (_to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Nothing is older than 1000 days
        assert_eq!(to_delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n_priority_over_delete_after() {
        // keep_last_n should prevent deletion even if beyond max age
        let policy = RetentionPolicy::builder()
            .keep_last_n(2)
            .delete_after(Duration::from_secs(1))
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 0),
            make_checkpoint("cp2", "thread1", 10), // 10 days old, beyond delete_after
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Both should be kept due to keep_last_n(2)
        assert_eq!(to_keep.len(), 2);
        assert!(to_keep.contains("cp1"));
        assert!(to_keep.contains("cp2"));
        assert!(!to_delete.contains("cp2")); // Not deleted despite age
    }

    #[test]
    fn test_single_checkpoint() {
        let policy = RetentionPolicy::builder()
            .keep_last_n(5)
            .delete_after(Duration::from_secs(30 * 86400))
            .build();

        let checkpoints = vec![make_checkpoint("cp1", "thread1", 0)];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        assert_eq!(to_keep.len(), 1);
        assert!(to_keep.contains("cp1"));
        assert_eq!(to_delete.len(), 0);
    }

    #[test]
    fn test_all_checkpoints_deleted() {
        let policy = RetentionPolicy::builder()
            .delete_after(Duration::from_secs(1))
            .build();

        let checkpoints = vec![
            make_checkpoint("cp1", "thread1", 100),
            make_checkpoint("cp2", "thread1", 200),
            make_checkpoint("cp3", "thread1", 300),
        ];

        let (to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // All are beyond 1 second ago
        assert_eq!(to_keep.len(), 0);
        assert_eq!(to_delete.len(), 3);
        assert!(to_delete.contains("cp1"));
        assert!(to_delete.contains("cp2"));
        assert!(to_delete.contains("cp3"));
    }
}
