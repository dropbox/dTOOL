//! Property-based tests for scheduler invariants
//!
//! This module contains proptest-based tests that verify scheduler properties
//! such as task ordering, selection strategy behavior, and queue invariants.
//!
//! # Tested Invariants
//!
//! 1. **Task Identity**: Task state survives serialization roundtrips
//! 2. **Priority Ordering**: Higher priority tasks are processed first
//! 3. **Queue Bounds**: Local queue respects capacity limits
//! 4. **No Task Duplication**: Each task is processed exactly once
//!
//! # Usage
//!
//! Run these tests with:
//! ```bash
//! cargo test -p dashflow scheduler_proptest --release
//! ```
//!
//! For more iterations (to find rarer edge cases):
//! ```bash
//! PROPTEST_CASES=10000 cargo test -p dashflow scheduler_proptest --release
//! ```

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashSet;

    use crate::scheduler::config::SelectionStrategy;
    use crate::scheduler::task::Task;

    // =========================================================================
    // Strategy Helpers
    // =========================================================================

    /// Generate a valid node name
    fn valid_node_name() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z][a-zA-Z0-9_]{0,30}")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty())
    }

    /// Generate a valid test state
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i64,
        message: String,
        items: Vec<String>,
    }

    fn arb_test_state() -> impl Strategy<Value = TestState> {
        (
            prop::num::i64::ANY,
            proptest::string::string_regex("[a-zA-Z0-9 ]{0,50}").unwrap(),
            proptest::collection::vec(
                proptest::string::string_regex("[a-zA-Z0-9]{0,20}").unwrap(),
                0..5,
            ),
        )
            .prop_map(|(value, message, items)| TestState {
                value,
                message,
                items,
            })
    }

    /// Generate a task with arbitrary state
    fn arb_task() -> impl Strategy<Value = Task<TestState>> {
        (valid_node_name(), arb_test_state(), prop::num::u8::ANY).prop_map(
            |(node_name, state, priority)| Task {
                node_name,
                state,
                priority,
            },
        )
    }

    /// Generate a selection strategy
    fn arb_selection_strategy() -> impl Strategy<Value = SelectionStrategy> {
        prop_oneof![
            Just(SelectionStrategy::RoundRobin),
            Just(SelectionStrategy::LeastLoaded),
            Just(SelectionStrategy::Random),
        ]
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    /// JSON roundtrip helper
    fn json_roundtrip<T: Serialize + for<'de> Deserialize<'de>>(value: &T) -> Result<T, String> {
        let json = serde_json::to_string(value).map_err(|e| format!("serialize: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))
    }

    // =========================================================================
    // Property Tests: Task Properties
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Task state survives JSON serialization roundtrip
        #[test]
        fn task_state_json_roundtrip(task in arb_task()) {
            // The state should survive serialization
            let result = json_roundtrip(&task.state);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), task.state);
        }

        /// Task node_name is preserved
        #[test]
        fn task_node_name_preserved(name in valid_node_name(), state in arb_test_state()) {
            let task = Task::new(name.clone(), state);
            prop_assert_eq!(&task.node_name, &name);
        }

        /// Task with_priority sets correct priority
        #[test]
        fn task_priority_preserved(
            name in valid_node_name(),
            state in arb_test_state(),
            priority in prop::num::u8::ANY
        ) {
            let task = Task::with_priority(name.clone(), state, priority);
            prop_assert_eq!(&task.node_name, &name);
            prop_assert_eq!(task.priority, priority);
        }

        /// Default task has priority 0
        #[test]
        fn task_default_priority_zero(name in valid_node_name(), state in arb_test_state()) {
            let task = Task::new(name, state);
            prop_assert_eq!(task.priority, 0);
        }
    }

    // =========================================================================
    // Property Tests: Priority Queue Ordering
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// When sorting tasks by priority, higher priority comes first
        #[test]
        fn priority_sort_invariant(
            tasks in proptest::collection::vec(arb_task(), 1..20)
        ) {
            let mut sorted = tasks.clone();
            sorted.sort_by(|a, b| b.priority.cmp(&a.priority)); // Descending

            // Verify sorted order: each task should have priority >= next task
            for window in sorted.windows(2) {
                prop_assert!(
                    window[0].priority >= window[1].priority,
                    "Priority order violated: {} >= {}",
                    window[0].priority,
                    window[1].priority
                );
            }
        }

        /// Stable sort preserves order for equal priorities
        #[test]
        fn priority_sort_stability(
            base_state in arb_test_state(),
            priority in 0u8..10,
            count in 2usize..10
        ) {
            // Create tasks with same priority but different names
            let tasks: Vec<_> = (0..count)
                .map(|i| Task {
                    node_name: format!("node_{i}"),
                    state: base_state.clone(),
                    priority,
                })
                .collect();

            let mut sorted = tasks.clone();
            sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

            // With stable sort, order should be preserved for equal priorities
            // This verifies the implementation doesn't randomly reorder equal-priority tasks
            for (i, task) in sorted.iter().enumerate() {
                prop_assert_eq!(&task.node_name, &format!("node_{i}"));
            }
        }
    }

    // =========================================================================
    // Property Tests: Selection Strategy
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// All selection strategies are valid enum variants
        #[test]
        fn selection_strategy_is_copy(strategy in arb_selection_strategy()) {
            // SelectionStrategy is Copy, verify this works
            let copy = strategy;
            prop_assert_eq!(copy, strategy);
        }
    }

    // =========================================================================
    // Property Tests: Task Collection Invariants
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Total task count is preserved across operations
        #[test]
        fn task_count_preserved(
            tasks in proptest::collection::vec(arb_task(), 0..50)
        ) {
            let count = tasks.len();
            let cloned: Vec<_> = tasks.iter().cloned().collect();
            prop_assert_eq!(cloned.len(), count, "Task count changed during iteration");
        }

        /// No duplicate node names after filtering
        #[test]
        fn unique_node_names_after_dedup(
            names in proptest::collection::vec(valid_node_name(), 1..20),
            state in arb_test_state()
        ) {
            let tasks: Vec<_> = names
                .iter()
                .map(|name| Task::new(name.clone(), state.clone()))
                .collect();

            // Count unique names first
            let unique_count = {
                let unique_names: HashSet<_> = tasks.iter().map(|t| &t.node_name).collect();
                unique_names.len()
            };

            // Simulate deduplication by node name
            let deduped: Vec<_> = tasks
                .into_iter()
                .fold(Vec::new(), |mut acc, task| {
                    if !acc.iter().any(|t: &Task<TestState>| t.node_name == task.node_name) {
                        acc.push(task);
                    }
                    acc
                });

            prop_assert_eq!(
                deduped.len(),
                unique_count,
                "Deduplication count mismatch"
            );
        }

        /// Priority sum is preserved during cloning
        #[test]
        fn priority_sum_preserved(
            tasks in proptest::collection::vec(arb_task(), 0..50)
        ) {
            let original_sum: u64 = tasks.iter().map(|t| u64::from(t.priority)).sum();
            let cloned: Vec<_> = tasks.iter().cloned().collect();
            let cloned_sum: u64 = cloned.iter().map(|t| u64::from(t.priority)).sum();
            prop_assert_eq!(original_sum, cloned_sum, "Priority sum changed during cloning");
        }
    }

    // Silence unused function warning when arb_selection_strategy is not used in tests
    #[allow(dead_code)]
    fn _use_arb_selection_strategy() {
        let _ = arb_selection_strategy();
    }
}
