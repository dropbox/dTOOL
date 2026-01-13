//! Chaos Engineering Tests for DashFlow Graph Execution
//!
//! This module implements chaos engineering tests to validate the resilience
//! and fault tolerance of DashFlow's graph execution engine. These tests inject
//! various failure scenarios to ensure the system handles them gracefully.
//!
//! ## Test Categories
//!
//! 1. **Node Failure Tests**: Nodes that return errors at various points
//! 2. **Timeout Tests**: Nodes that exceed execution time limits
//! 3. **Concurrent Stress Tests**: High concurrency failure scenarios
//! 4. **Partial Failure Tests**: Failures in parallel execution branches
//! 5. **Recursion Limit Tests**: Infinite loop detection

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr
)]

use dashflow::state::MergeableState;
use dashflow::{Error, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Test state for chaos testing
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
struct ChaosState {
    /// Execution trace for debugging
    trace: Vec<String>,
    /// Counter for tracking node executions
    execution_count: u32,
    /// Flag to indicate if error was handled
    error_handled: bool,
    /// Data collected during execution
    data: Vec<i32>,
}

impl MergeableState for ChaosState {
    fn merge(&mut self, other: &Self) {
        self.trace.extend(other.trace.clone());
        self.execution_count += other.execution_count;
        self.error_handled = self.error_handled || other.error_handled;
        self.data.extend(other.data.clone());
    }
}

// =============================================================================
// Node Failure Tests
// =============================================================================

#[tokio::test]
async fn test_node_returns_error_propagates_correctly() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("failing_node", |mut state| {
        Box::pin(async move {
            state.trace.push("failing_node_entered".to_string());
            Err(Error::Generic("Intentional chaos failure".to_string()))
        })
    });

    graph.add_node_from_fn("should_not_run", |mut state| {
        Box::pin(async move {
            state.trace.push("should_not_run".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_edge("start", "failing_node");
    graph.add_edge("failing_node", "should_not_run");
    graph.add_edge("should_not_run", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    // Error should propagate
    assert!(
        result.is_err(),
        "Expected error to propagate from failing node"
    );

    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("failing_node") || err_string.contains("Intentional chaos failure"),
        "Error should identify failing node or error message: {}",
        err_string
    );
}

#[tokio::test]
async fn test_early_node_failure_prevents_downstream_execution() {
    let execution_counter = Arc::new(AtomicU32::new(0));
    let counter_clone = execution_counter.clone();

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("node1", move |mut state| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            state.trace.push("node1".to_string());
            Err(Error::Generic("Early failure".to_string()))
        })
    });

    let counter_clone2 = execution_counter.clone();
    graph.add_node_from_fn("node2", move |mut state| {
        let counter = counter_clone2.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            state.trace.push("node2".to_string());
            Ok(state)
        })
    });

    let counter_clone3 = execution_counter.clone();
    graph.add_node_from_fn("node3", move |mut state| {
        let counter = counter_clone3.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            state.trace.push("node3".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let app = graph.compile().unwrap();
    let _ = app.invoke(ChaosState::default()).await;

    // Only node1 should have executed
    assert_eq!(
        execution_counter.load(Ordering::SeqCst),
        1,
        "Only the failing node should have executed"
    );
}

#[tokio::test]
async fn test_intermittent_node_failure() {
    // Simulate a node that fails on first execution but succeeds on retry
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let counter_clone = attempt_counter.clone();
    graph.add_node_from_fn("intermittent", move |mut state| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            state.execution_count = attempt + 1;

            if attempt < 2 {
                state.trace.push(format!("attempt_{}_failed", attempt));
                Err(Error::Generic(format!("Intermittent failure #{}", attempt)))
            } else {
                state.trace.push(format!("attempt_{}_success", attempt));
                Ok(state)
            }
        })
    });

    graph.set_entry_point("intermittent");
    graph.add_edge("intermittent", END);

    let app = graph.compile().unwrap();

    // First two attempts should fail
    let result1 = app.invoke(ChaosState::default()).await;
    assert!(result1.is_err(), "First attempt should fail");

    let result2 = app.invoke(ChaosState::default()).await;
    assert!(result2.is_err(), "Second attempt should fail");

    // Third attempt should succeed
    let result3 = app.invoke(ChaosState::default()).await;
    assert!(result3.is_ok(), "Third attempt should succeed");
}

// =============================================================================
// Timeout Tests
// =============================================================================

#[tokio::test]
async fn test_node_timeout_is_enforced() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("slow_node", |mut state| {
        Box::pin(async move {
            state.trace.push("slow_node_started".to_string());
            // Sleep longer than the timeout
            tokio::time::sleep(Duration::from_secs(5)).await;
            state.trace.push("slow_node_completed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(100));

    let start = std::time::Instant::now();
    let result = app.invoke(ChaosState::default()).await;
    let elapsed = start.elapsed();

    // Should timeout, not take 5 seconds
    assert!(elapsed < Duration::from_secs(1), "Should timeout quickly");
    assert!(result.is_err(), "Should return timeout error");

    let err = result.unwrap_err();
    assert!(
        matches!(err, Error::Timeout(_)),
        "Error should be Timeout variant: {:?}",
        err
    );
}

#[tokio::test]
async fn test_timeout_in_middle_of_graph() {
    let execution_trace = Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let trace1 = execution_trace.clone();
    graph.add_node_from_fn("fast_node1", move |mut state| {
        let trace = trace1.clone();
        Box::pin(async move {
            trace.lock().unwrap().push("fast1");
            state.trace.push("fast_node1".to_string());
            Ok(state)
        })
    });

    let trace2 = execution_trace.clone();
    graph.add_node_from_fn("slow_node", move |mut state| {
        let trace = trace2.clone();
        Box::pin(async move {
            trace.lock().unwrap().push("slow_start");
            tokio::time::sleep(Duration::from_secs(5)).await;
            trace.lock().unwrap().push("slow_end");
            state.trace.push("slow_node".to_string());
            Ok(state)
        })
    });

    let trace3 = execution_trace.clone();
    graph.add_node_from_fn("fast_node2", move |mut state| {
        let trace = trace3.clone();
        Box::pin(async move {
            trace.lock().unwrap().push("fast2");
            state.trace.push("fast_node2".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("fast_node1");
    graph.add_edge("fast_node1", "slow_node");
    graph.add_edge("slow_node", "fast_node2");
    graph.add_edge("fast_node2", END);

    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(200));

    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err(), "Should timeout");

    let trace = execution_trace.lock().unwrap();
    assert!(trace.contains(&"fast1"), "Fast node 1 should have run");
    assert!(
        trace.contains(&"slow_start"),
        "Slow node should have started"
    );
    assert!(!trace.contains(&"fast2"), "Fast node 2 should not have run");
}

// =============================================================================
// Concurrent Stress Tests
// =============================================================================

#[tokio::test]
async fn test_concurrent_graph_executions_with_failures() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let failure_counter = Arc::new(AtomicU32::new(0));
    let counter_clone = failure_counter.clone();

    graph.add_node_from_fn("conditional_failure", move |mut state| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            let count = counter.fetch_add(1, Ordering::SeqCst);
            state.execution_count = count + 1;

            // Fail every other execution
            if count % 2 == 0 {
                state.trace.push(format!("execution_{}_failed", count));
                Err(Error::Generic(format!("Controlled failure #{}", count)))
            } else {
                state.trace.push(format!("execution_{}_success", count));
                state.data.push(count as i32);
                Ok(state)
            }
        })
    });

    graph.set_entry_point("conditional_failure");
    graph.add_edge("conditional_failure", END);

    let app = Arc::new(graph.compile().unwrap());

    // Launch 10 concurrent executions
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let app = app.clone();
            tokio::spawn(async move { app.invoke(ChaosState::default()).await })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let successes = results.iter().filter(|r| r.is_ok()).count();
    let failures = results.iter().filter(|r| r.is_err()).count();

    // Should have a mix of successes and failures
    assert!(successes > 0, "Should have some successes");
    assert!(failures > 0, "Should have some failures");
    assert_eq!(successes + failures, 10, "All executions should complete");
}

#[tokio::test]
async fn test_high_concurrency_stress() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let total_executions = Arc::new(AtomicU32::new(0));
    let counter_clone = total_executions.clone();

    graph.add_node_from_fn("stress_node", move |mut state| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            // Small delay to simulate work
            tokio::time::sleep(Duration::from_micros(100)).await;
            state.data.push(1);
            Ok(state)
        })
    });

    graph.set_entry_point("stress_node");
    graph.add_edge("stress_node", END);

    let app = Arc::new(graph.compile().unwrap());

    // Launch 100 concurrent executions
    let handles: Vec<_> = (0..100)
        .map(|_| {
            let app = app.clone();
            tokio::spawn(async move { app.invoke(ChaosState::default()).await })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles).await;

    let all_ok = results
        .iter()
        .all(|r| r.is_ok() && r.as_ref().unwrap().is_ok());
    assert!(all_ok, "All concurrent executions should succeed");

    assert_eq!(
        total_executions.load(Ordering::SeqCst),
        100,
        "All 100 executions should have run"
    );
}

// =============================================================================
// Parallel Branch Failure Tests
// =============================================================================

#[tokio::test]
async fn test_parallel_branch_single_failure() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    // Branch that succeeds
    graph.add_node_from_fn("success_branch", |mut state| {
        Box::pin(async move {
            state.trace.push("success_branch".to_string());
            state.data.push(1);
            Ok(state)
        })
    });

    // Branch that fails
    graph.add_node_from_fn("failure_branch", |mut state| {
        Box::pin(async move {
            state.trace.push("failure_branch_started".to_string());
            Err(Error::Generic("Branch failure".to_string()))
        })
    });

    graph.add_node_from_fn("merge_node", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["success_branch".to_string(), "failure_branch".to_string()],
    );
    graph.add_edge("success_branch", "merge_node");
    graph.add_edge("failure_branch", "merge_node");
    graph.add_edge("merge_node", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    // When one parallel branch fails, the entire execution should fail
    assert!(
        result.is_err(),
        "Parallel execution should fail when any branch fails"
    );
}

#[tokio::test]
async fn test_parallel_branch_all_succeed() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            state.trace.push("branch_a".to_string());
            state.data.push(10);
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            state.trace.push("branch_b".to_string());
            state.data.push(20);
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_c", |mut state| {
        Box::pin(async move {
            state.trace.push("branch_c".to_string());
            state.data.push(30);
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec![
            "branch_a".to_string(),
            "branch_b".to_string(),
            "branch_c".to_string(),
        ],
    );
    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);
    graph.add_edge("branch_c", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "All branches should succeed");

    let state = result.unwrap().final_state;
    assert_eq!(state.data.len(), 3, "All branch data should be merged");
    assert!(state.data.contains(&10), "Branch A data missing");
    assert!(state.data.contains(&20), "Branch B data missing");
    assert!(state.data.contains(&30), "Branch C data missing");
}

// =============================================================================
// Recursion Limit Tests
// =============================================================================

#[tokio::test]
async fn test_recursion_limit_enforced() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let iteration = Arc::new(AtomicU32::new(0));
    let iter_clone = iteration.clone();

    graph.add_node_from_fn("loop_node", move |mut state| {
        let iter = iter_clone.clone();
        Box::pin(async move {
            let count = iter.fetch_add(1, Ordering::SeqCst);
            state.execution_count = count + 1;
            state.trace.push(format!("iteration_{}", count));
            Ok(state)
        })
    });

    // Create a cycle: loop_node -> loop_node (via conditional)
    graph.set_entry_point("loop_node");

    let mut routes = HashMap::new();
    routes.insert("loop".to_string(), "loop_node".to_string());
    graph.add_conditional_edges(
        "loop_node",
        |_state: &ChaosState| "loop".to_string(),
        routes,
    );

    let app = graph.compile().unwrap().with_recursion_limit(5);

    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err(), "Should hit recursion limit");

    let err = result.unwrap_err();
    assert!(
        matches!(err, Error::RecursionLimit { .. }),
        "Should be RecursionLimit error: {:?}",
        err
    );

    // Should have executed approximately 5 times (the limit)
    let count = iteration.load(Ordering::SeqCst);
    assert!(
        count <= 6,
        "Should stop near recursion limit, got {}",
        count
    );
}

// =============================================================================
// State Validation Tests
// =============================================================================

#[tokio::test]
async fn test_node_can_validate_state() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("producer", |mut state| {
        Box::pin(async move {
            state.data.push(100);
            state.trace.push("produced".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("validator", |mut state| {
        Box::pin(async move {
            // Validate that data was produced
            if state.data.is_empty() {
                return Err(Error::Generic(
                    "State validation failed: no data produced".to_string(),
                ));
            }
            if state.data[0] != 100 {
                return Err(Error::Generic(
                    "State validation failed: unexpected data value".to_string(),
                ));
            }
            state.trace.push("validated".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("consumer", |mut state| {
        Box::pin(async move {
            state.trace.push("consumed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("producer");
    graph.add_edge("producer", "validator");
    graph.add_edge("validator", "consumer");
    graph.add_edge("consumer", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Validation should pass");

    let state = result.unwrap().final_state;
    assert_eq!(state.trace, vec!["produced", "validated", "consumed"]);
}

#[tokio::test]
async fn test_state_validation_failure() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    // Skip producer - go directly to validator
    graph.add_node_from_fn("validator", |state| {
        Box::pin(async move {
            if state.data.is_empty() {
                return Err(Error::Generic(
                    "State validation failed: no data".to_string(),
                ));
            }
            Ok(state)
        })
    });

    graph.set_entry_point("validator");
    graph.add_edge("validator", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err(), "Validation should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("no data"),
        "Error should mention validation failure"
    );
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_empty_graph_execution() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    // Graph with just start -> END
    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");
    graph.add_edge("start", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Minimal graph should execute successfully");
}

#[tokio::test]
async fn test_very_long_sequential_chain() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let node_count = 50;
    let execution_counter = Arc::new(AtomicU32::new(0));

    // Create a long chain of nodes
    for i in 0..node_count {
        let counter = execution_counter.clone();
        let node_name = format!("node_{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            let c = counter.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                state.trace.push(format!("node_{}", state.trace.len()));
                Ok(state)
            })
        });
    }

    // Wire up the chain
    graph.set_entry_point("node_0");
    for i in 0..(node_count - 1) {
        graph.add_edge(format!("node_{}", i), format!("node_{}", i + 1));
    }
    graph.add_edge(format!("node_{}", node_count - 1), END);

    // Need to increase recursion limit for long chains (default is 25)
    let app = graph.compile().unwrap().with_recursion_limit(100);
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Long chain should complete");

    let state = result.unwrap().final_state;
    assert_eq!(
        state.trace.len(),
        node_count,
        "All nodes should have executed"
    );
    assert_eq!(
        execution_counter.load(Ordering::SeqCst),
        node_count as u32,
        "All nodes should have been called"
    );
}

#[tokio::test]
async fn test_diamond_pattern_with_failure() {
    //       start
    //      /     \
    //    left   right (fails)
    //      \     /
    //       merge
    //         |
    //        END

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("left", |mut state| {
        Box::pin(async move {
            state.trace.push("left".to_string());
            state.data.push(1);
            Ok(state)
        })
    });

    graph.add_node_from_fn("right", |mut state| {
        Box::pin(async move {
            state.trace.push("right_started".to_string());
            Err(Error::Generic("Right branch failure".to_string()))
        })
    });

    graph.add_node_from_fn("merge", |mut state| {
        Box::pin(async move {
            state.trace.push("merge".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["left".to_string(), "right".to_string()]);
    graph.add_edge("left", "merge");
    graph.add_edge("right", "merge");
    graph.add_edge("merge", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err(), "Diamond with failing branch should fail");
}

// =============================================================================
// Error Message Quality Tests
// =============================================================================

#[tokio::test]
async fn test_error_messages_are_informative() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("important_node", |_state| {
        Box::pin(async move {
            Err(Error::Generic(
                "Critical validation failed: missing required field 'user_id'".to_string(),
            ))
        })
    });

    graph.set_entry_point("important_node");
    graph.add_edge("important_node", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    // Error should contain useful debugging information
    assert!(
        err_msg.contains("important_node") || err_msg.contains("missing required field"),
        "Error message should be informative: {}",
        err_msg
    );
}

// =============================================================================
// Resource Cleanup Tests
// =============================================================================

#[tokio::test]
async fn test_resources_cleaned_up_on_error() {
    let resource_counter = Arc::new(AtomicU32::new(0));

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let counter1 = resource_counter.clone();
    graph.add_node_from_fn("acquire_resource", move |mut state| {
        let counter = counter1.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            state.trace.push("resource_acquired".to_string());
            Ok(state)
        })
    });

    let counter2 = resource_counter.clone();
    graph.add_node_from_fn("use_resource", move |mut state| {
        let counter = counter2.clone();
        Box::pin(async move {
            state.trace.push("using_resource".to_string());
            // Simulate failure during resource use
            counter.fetch_sub(1, Ordering::SeqCst); // Release resource even on failure
            Err(Error::Generic("Operation failed".to_string()))
        })
    });

    graph.set_entry_point("acquire_resource");
    graph.add_edge("acquire_resource", "use_resource");
    graph.add_edge("use_resource", END);

    let app = graph.compile().unwrap();
    let _ = app.invoke(ChaosState::default()).await;

    // Resource should be released (counter back to 0)
    assert_eq!(
        resource_counter.load(Ordering::SeqCst),
        0,
        "Resources should be cleaned up on error"
    );
}

// =============================================================================
// Multiple Error Types Tests
// =============================================================================

#[tokio::test]
async fn test_validation_error_type() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("validation_fail", |_state| {
        Box::pin(async move { Err(Error::Validation("Invalid graph configuration".to_string())) })
    });

    graph.set_entry_point("validation_fail");
    graph.add_edge("validation_fail", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // Check that Validation errors propagate with their message
    assert!(
        err.to_string().contains("Invalid graph configuration"),
        "Validation error message should be preserved: {}",
        err
    );
}

#[tokio::test]
async fn test_node_execution_error_wrapping() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("io_error_node", |_state| {
        Box::pin(async move {
            // Return a generic error that might be wrapped as NodeExecution
            Err(Error::Generic("Simulated IO error".to_string()))
        })
    });

    graph.set_entry_point("io_error_node");
    graph.add_edge("io_error_node", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    // Error should contain our message or node name
    assert!(
        err_str.contains("io_error_node") || err_str.contains("Simulated IO error"),
        "Error should be informative: {}",
        err_str
    );
}

// =============================================================================
// Memory Pressure Tests
// =============================================================================

#[tokio::test]
async fn test_large_state_handling() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("generate_large_data", |mut state| {
        Box::pin(async move {
            // Generate 10,000 items in the state
            for i in 0..10_000 {
                state.data.push(i);
            }
            state.trace.push("large_data_generated".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("process_data", |mut state| {
        Box::pin(async move {
            // Verify all data is present
            if state.data.len() != 10_000 {
                return Err(Error::Generic(format!(
                    "Expected 10000 items, got {}",
                    state.data.len()
                )));
            }
            state.trace.push("data_processed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("generate_large_data");
    graph.add_edge("generate_large_data", "process_data");
    graph.add_edge("process_data", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Large state should be handled correctly");
    let state = result.unwrap().final_state;
    assert_eq!(state.data.len(), 10_000);
}

#[tokio::test]
async fn test_large_trace_accumulation() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let node_count = 100;

    for i in 0..node_count {
        let node_name = format!("trace_node_{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                // Add a moderately long trace entry
                state.trace.push(format!("trace_entry_from_node_{:04}", i));
                Ok(state)
            })
        });
    }

    graph.set_entry_point("trace_node_0");
    for i in 0..(node_count - 1) {
        graph.add_edge(format!("trace_node_{}", i), format!("trace_node_{}", i + 1));
    }
    graph.add_edge(format!("trace_node_{}", node_count - 1), END);

    let app = graph.compile().unwrap().with_recursion_limit(150);
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Large trace accumulation should succeed");
    let state = result.unwrap().final_state;
    assert_eq!(
        state.trace.len(),
        node_count,
        "All trace entries should be accumulated"
    );
}

#[tokio::test]
async fn test_parallel_large_data_merge() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    // Each parallel branch generates 1000 items
    for i in 0..5 {
        let branch_name = format!("branch_{}", i);
        let offset = i * 1000;
        graph.add_node_from_fn(&branch_name, move |mut state| {
            Box::pin(async move {
                for j in 0..1000 {
                    state.data.push(offset + j);
                }
                state.trace.push(format!("branch_{}", offset / 1000));
                Ok(state)
            })
        });
    }

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec![
            "branch_0".to_string(),
            "branch_1".to_string(),
            "branch_2".to_string(),
            "branch_3".to_string(),
            "branch_4".to_string(),
        ],
    );

    for i in 0..5 {
        graph.add_edge(format!("branch_{}", i), END);
    }

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Parallel large data merge should succeed");
    let state = result.unwrap().final_state;
    assert_eq!(
        state.data.len(),
        5000,
        "All branch data should be merged: got {}",
        state.data.len()
    );
}

// =============================================================================
// Rapid Succession Tests
// =============================================================================

#[tokio::test]
async fn test_rapid_graph_creation_and_execution() {
    // Create and execute many graphs in rapid succession
    // This tests for resource leaks and cleanup
    for iteration in 0..50 {
        let mut graph: StateGraph<ChaosState> = StateGraph::new();

        graph.add_node_from_fn("quick_node", move |mut state| {
            Box::pin(async move {
                state.execution_count = iteration;
                Ok(state)
            })
        });

        graph.set_entry_point("quick_node");
        graph.add_edge("quick_node", END);

        let app = graph.compile().unwrap();
        let result = app.invoke(ChaosState::default()).await;

        assert!(result.is_ok(), "Iteration {} should succeed", iteration);
        assert_eq!(result.unwrap().final_state.execution_count, iteration);
    }
}

#[tokio::test]
async fn test_interleaved_concurrent_executions() {
    // Multiple graphs executing concurrently with interleaved work
    let graph1 = {
        let mut g: StateGraph<ChaosState> = StateGraph::new();
        g.add_node_from_fn("g1_node", |mut state| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                state.data.push(1);
                Ok(state)
            })
        });
        g.set_entry_point("g1_node");
        g.add_edge("g1_node", END);
        Arc::new(g.compile().unwrap())
    };

    let graph2 = {
        let mut g: StateGraph<ChaosState> = StateGraph::new();
        g.add_node_from_fn("g2_node", |mut state| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                state.data.push(2);
                Ok(state)
            })
        });
        g.set_entry_point("g2_node");
        g.add_edge("g2_node", END);
        Arc::new(g.compile().unwrap())
    };

    // Launch 20 executions of each graph concurrently
    let mut handles = Vec::new();
    for i in 0..40 {
        let graph = if i % 2 == 0 {
            graph1.clone()
        } else {
            graph2.clone()
        };
        handles.push(tokio::spawn(async move {
            graph.invoke(ChaosState::default()).await
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let all_ok = results
        .iter()
        .all(|r| r.is_ok() && r.as_ref().unwrap().is_ok());
    assert!(all_ok, "All interleaved executions should succeed");
}

// =============================================================================
// State Corruption Tests
// =============================================================================

#[tokio::test]
async fn test_state_isolation_between_branches() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.data.push(0);
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    // Branch A modifies data[0] to 100
    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            if let Some(first) = state.data.first_mut() {
                *first = 100;
            }
            state.trace.push("branch_a".to_string());
            state.data.push(10);
            Ok(state)
        })
    });

    // Branch B modifies data[0] to 200
    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            if let Some(first) = state.data.first_mut() {
                *first = 200;
            }
            state.trace.push("branch_b".to_string());
            state.data.push(20);
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["branch_a".to_string(), "branch_b".to_string()],
    );
    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Parallel branches should complete");
    let state = result.unwrap().final_state;

    // Both branches' additions should be present (10 and 20)
    assert!(
        state.data.contains(&10),
        "Branch A's data should be present"
    );
    assert!(
        state.data.contains(&20),
        "Branch B's data should be present"
    );
}

#[tokio::test]
async fn test_error_does_not_corrupt_shared_state() {
    let shared_counter = Arc::new(AtomicU32::new(0));

    // Run multiple times to check for state corruption
    for _ in 0..10 {
        let counter = shared_counter.clone();
        let mut graph: StateGraph<ChaosState> = StateGraph::new();

        graph.add_node_from_fn("increment", move |mut state| {
            let c = counter.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                state.execution_count += 1;
                Ok(state)
            })
        });

        graph.add_node_from_fn("maybe_fail", |state| {
            Box::pin(async move {
                // Fail 50% of the time
                if state.execution_count % 2 == 0 {
                    Err(Error::Generic("Controlled failure".to_string()))
                } else {
                    Ok(state)
                }
            })
        });

        graph.set_entry_point("increment");
        graph.add_edge("increment", "maybe_fail");
        graph.add_edge("maybe_fail", END);

        let app = graph.compile().unwrap();
        let _ = app.invoke(ChaosState::default()).await;
    }

    // Counter should have been incremented exactly 10 times
    assert_eq!(
        shared_counter.load(Ordering::SeqCst),
        10,
        "Shared state should be consistent despite errors"
    );
}

// =============================================================================
// Cancellation Tests
// =============================================================================

#[tokio::test]
async fn test_execution_can_be_cancelled() {
    let started = Arc::new(AtomicU32::new(0));
    let completed = Arc::new(AtomicU32::new(0));

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let started_clone = started.clone();
    let completed_clone = completed.clone();
    graph.add_node_from_fn("slow_cancellable", move |mut state| {
        let s = started_clone.clone();
        let c = completed_clone.clone();
        Box::pin(async move {
            s.fetch_add(1, Ordering::SeqCst);
            // Long operation that should be cancelled
            tokio::time::sleep(Duration::from_secs(10)).await;
            c.fetch_add(1, Ordering::SeqCst);
            state.trace.push("completed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("slow_cancellable");
    graph.add_edge("slow_cancellable", END);

    let app = graph.compile().unwrap();

    // Use tokio's select to race the execution against a timeout
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        app.invoke(ChaosState::default()),
    )
    .await;

    // Should have timed out
    assert!(
        result.is_err(),
        "Execution should be cancellable via timeout"
    );

    // Node should have started but not completed
    assert_eq!(
        started.load(Ordering::SeqCst),
        1,
        "Node should have started"
    );
    assert_eq!(
        completed.load(Ordering::SeqCst),
        0,
        "Node should not have completed"
    );
}

// =============================================================================
// Boundary Condition Tests
// =============================================================================

#[tokio::test]
async fn test_zero_timeout() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("instant", |mut state| {
        Box::pin(async move {
            state.trace.push("executed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("instant");
    graph.add_edge("instant", END);

    // Zero timeout should still allow fast execution or fail immediately
    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(0));
    let result = app.invoke(ChaosState::default()).await;

    // Either succeeds (if execution was fast enough) or times out
    // Both are valid behaviors for zero timeout
    assert!(
        result.is_ok() || matches!(result.as_ref().unwrap_err(), Error::Timeout(_)),
        "Zero timeout should either succeed or timeout: {:?}",
        result
    );
}

#[tokio::test]
async fn test_recursion_limit_of_one() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let execution_count = Arc::new(AtomicU32::new(0));
    let counter = execution_count.clone();

    graph.add_node_from_fn("single", move |mut state| {
        let c = counter.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            state.trace.push("executed".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("single");
    graph.add_edge("single", END);

    let app = graph.compile().unwrap().with_recursion_limit(1);
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Single execution should succeed");
    assert_eq!(
        execution_count.load(Ordering::SeqCst),
        1,
        "Node should execute exactly once"
    );
}

#[tokio::test]
async fn test_many_parallel_branches() {
    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.trace.push("start".to_string());
            Ok(state)
        })
    });

    // Create 20 parallel branches
    let branch_names: Vec<String> = (0..20).map(|i| format!("branch_{}", i)).collect();

    for (i, name) in branch_names.iter().enumerate() {
        let idx = i as i32;
        graph.add_node_from_fn(name, move |mut state| {
            Box::pin(async move {
                state.data.push(idx);
                Ok(state)
            })
        });
    }

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", branch_names.clone());

    for name in &branch_names {
        graph.add_edge(name, END);
    }

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "20 parallel branches should succeed");
    let state = result.unwrap().final_state;
    assert_eq!(
        state.data.len(),
        20,
        "All 20 branches should contribute data"
    );
}

// =============================================================================
// Error Recovery Pattern Tests
// =============================================================================

#[tokio::test]
async fn test_conditional_retry_on_error() {
    let attempt = Arc::new(AtomicU32::new(0));

    let mut graph: StateGraph<ChaosState> = StateGraph::new();

    let attempt_clone = attempt.clone();
    graph.add_node_from_fn("retry_node", move |mut state| {
        let a = attempt_clone.clone();
        Box::pin(async move {
            let current = a.fetch_add(1, Ordering::SeqCst);
            state.execution_count = current + 1;

            if current < 3 {
                state.error_handled = false;
                state.trace.push(format!("attempt_{}_failed", current));
            } else {
                state.error_handled = true;
                state.trace.push(format!("attempt_{}_success", current));
            }
            Ok(state)
        })
    });

    // Conditional edge that retries on failure
    let mut routes = HashMap::new();
    routes.insert("retry".to_string(), "retry_node".to_string());
    routes.insert("done".to_string(), END.to_string());

    graph.set_entry_point("retry_node");
    graph.add_conditional_edges(
        "retry_node",
        |state: &ChaosState| {
            if state.error_handled {
                "done".to_string()
            } else {
                "retry".to_string()
            }
        },
        routes,
    );

    let app = graph.compile().unwrap().with_recursion_limit(10);
    let result = app.invoke(ChaosState::default()).await;

    assert!(result.is_ok(), "Retry pattern should eventually succeed");
    let state = result.unwrap().final_state;
    assert!(
        state.error_handled,
        "Error should be marked as handled after retries"
    );
    assert_eq!(
        state.execution_count, 4,
        "Should have executed 4 times (3 failures + 1 success)"
    );
}
