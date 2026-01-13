// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Task definition for work-stealing scheduler

use crate::state::GraphState;

/// A unit of work representing a node execution request
///
/// Tasks are submitted to the scheduler and executed either locally
/// or on remote workers.
#[derive(Debug, Clone)]
pub struct Task<S>
where
    S: GraphState,
{
    /// Name of the node to execute
    pub node_name: String,
    /// State to pass to the node
    pub state: S,
    /// Task priority (higher = more important, default = 0)
    pub priority: u8,
}

impl<S> Task<S>
where
    S: GraphState,
{
    /// Create a new task
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the node to execute
    /// * `state` - Graph state to pass to the node
    pub fn new(node_name: impl Into<String>, state: S) -> Self {
        Self {
            node_name: node_name.into(),
            state,
            priority: 0,
        }
    }

    /// Create a task with priority
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the node to execute
    /// * `state` - Graph state to pass to the node
    /// * `priority` - Task priority (0-255, higher = more important)
    #[must_use]
    pub fn with_priority(node_name: impl Into<String>, state: S, priority: u8) -> Self {
        Self {
            node_name: node_name.into(),
            state,
            priority,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// Test state for scheduler tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        message: String,
    }

    #[test]
    fn test_task_new() {
        let state = TestState {
            value: 42,
            message: "test".to_string(),
        };
        let task = Task::new("test_node".to_string(), state.clone());

        assert_eq!(task.node_name, "test_node");
        assert_eq!(task.state.value, 42);
        assert_eq!(task.state.message, "test");
        assert_eq!(task.priority, 0); // Default priority
    }

    #[test]
    fn test_task_with_priority() {
        let state = TestState {
            value: 100,
            message: "priority_test".to_string(),
        };
        let task = Task::with_priority("priority_node".to_string(), state.clone(), 5);

        assert_eq!(task.node_name, "priority_node");
        assert_eq!(task.state.value, 100);
        assert_eq!(task.state.message, "priority_test");
        assert_eq!(task.priority, 5);
    }

    #[test]
    fn test_task_with_max_priority() {
        let state = TestState {
            value: 1,
            message: "max".to_string(),
        };
        let task = Task::with_priority("urgent".to_string(), state, 255);

        assert_eq!(task.priority, 255); // Max u8 value
    }

    #[test]
    fn test_task_with_min_priority() {
        let state = TestState {
            value: 1,
            message: "min".to_string(),
        };
        let task = Task::with_priority("low".to_string(), state, 0);

        assert_eq!(task.priority, 0); // Min priority
    }

    #[test]
    fn test_task_clone() {
        let state = TestState {
            value: 99,
            message: "clone_test".to_string(),
        };
        let task = Task::with_priority("clone_node".to_string(), state, 3);
        let cloned = task.clone();

        assert_eq!(cloned.node_name, task.node_name);
        assert_eq!(cloned.state.value, task.state.value);
        assert_eq!(cloned.state.message, task.state.message);
        assert_eq!(cloned.priority, task.priority);
    }

    #[test]
    fn test_task_debug_format() {
        let state = TestState {
            value: 42,
            message: "debug".to_string(),
        };
        let task = Task::new("debug_node".to_string(), state);
        let debug_str = format!("{:?}", task);

        // Verify debug output contains key information
        assert!(debug_str.contains("Task"));
        assert!(debug_str.contains("debug_node"));
    }

    #[test]
    fn test_task_different_priorities() {
        let state = TestState {
            value: 1,
            message: "test".to_string(),
        };

        let low = Task::with_priority("low".to_string(), state.clone(), 1);
        let medium = Task::with_priority("medium".to_string(), state.clone(), 128);
        let high = Task::with_priority("high".to_string(), state, 255);

        assert!(low.priority < medium.priority);
        assert!(medium.priority < high.priority);
    }
}
