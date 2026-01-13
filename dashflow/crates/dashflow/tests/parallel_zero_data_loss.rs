//! Test: Parallel Execution Zero Data Loss Validation
//!
//! This test validates that Gap #1 (parallel state merging data loss) is completely fixed.
//! Before the fix, parallel execution used last-write-wins, losing 71% of data.
//! After the fix, all parallel branches' data is preserved via MergeableState::merge().

use dashflow::state::MergeableState;
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct TestState {
    findings: Vec<String>,
    scores: Vec<i32>,
    max_value: i32,
    message_log: String,
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        // Extend vectors from parallel branches
        self.findings.extend(other.findings.clone());
        self.scores.extend(other.scores.clone());

        // Take maximum value
        self.max_value = self.max_value.max(other.max_value);

        // Concatenate logs
        if !other.message_log.is_empty() {
            if !self.message_log.is_empty() {
                self.message_log.push('\n');
            }
            self.message_log.push_str(&other.message_log);
        }
    }
}

#[tokio::test]
async fn test_parallel_execution_preserves_all_data() -> dashflow::Result<()> {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    // Start node
    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.message_log = "Started".to_string();
            Ok(state)
        })
    });

    // Parallel branch 1: Research findings
    graph.add_node_from_fn("researcher", |mut state| {
        Box::pin(async move {
            state.findings.push("Finding A".to_string());
            state.findings.push("Finding B".to_string());
            state.scores.push(10);
            state.max_value = 10;
            state.message_log.push_str("\nResearcher complete");
            Ok(state)
        })
    });

    // Parallel branch 2: Analysis findings
    graph.add_node_from_fn("analyst", |mut state| {
        Box::pin(async move {
            state.findings.push("Finding C".to_string());
            state.findings.push("Finding D".to_string());
            state.scores.push(20);
            state.max_value = 20;
            state.message_log.push_str("\nAnalyst complete");
            Ok(state)
        })
    });

    // End node
    graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

    // Build graph: start → [researcher || analyst] → end
    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["researcher".to_string(), "analyst".to_string()],
    );
    graph.add_edge("researcher", "end_node");
    graph.add_edge("analyst", "end_node");
    graph.add_edge("end_node", END);

    let app = graph.compile_with_merge()?;

    let initial_state = TestState {
        findings: vec![],
        scores: vec![],
        max_value: 0,
        message_log: String::new(),
    };

    let result = app.invoke(initial_state).await?;

    // CRITICAL ASSERTIONS: Zero Data Loss

    // All 4 findings from BOTH branches must be present
    assert_eq!(
        result.final_state.findings.len(),
        4,
        "Expected 4 findings (2 from researcher + 2 from analyst), got {}",
        result.final_state.findings.len()
    );

    assert!(
        result
            .final_state
            .findings
            .contains(&"Finding A".to_string()),
        "Missing Finding A from researcher"
    );
    assert!(
        result
            .final_state
            .findings
            .contains(&"Finding B".to_string()),
        "Missing Finding B from researcher"
    );
    assert!(
        result
            .final_state
            .findings
            .contains(&"Finding C".to_string()),
        "Missing Finding C from analyst"
    );
    assert!(
        result
            .final_state
            .findings
            .contains(&"Finding D".to_string()),
        "Missing Finding D from analyst"
    );

    // Both scores from BOTH branches must be present
    assert_eq!(
        result.final_state.scores.len(),
        2,
        "Expected 2 scores (1 from researcher + 1 from analyst), got {}",
        result.final_state.scores.len()
    );
    assert!(
        result.final_state.scores.contains(&10),
        "Missing score 10 from researcher"
    );
    assert!(
        result.final_state.scores.contains(&20),
        "Missing score 20 from analyst"
    );

    // Max value should be the maximum from all branches
    assert_eq!(
        result.final_state.max_value, 20,
        "Expected max_value=20 (max of 10 and 20), got {}",
        result.final_state.max_value
    );

    // Message log should contain entries from BOTH branches
    assert!(
        result.final_state.message_log.contains("Started"),
        "Missing 'Started' from start node"
    );
    assert!(
        result
            .final_state
            .message_log
            .contains("Researcher complete"),
        "Missing 'Researcher complete' from researcher branch"
    );
    assert!(
        result.final_state.message_log.contains("Analyst complete"),
        "Missing 'Analyst complete' from analyst branch"
    );

    println!("✅ Zero data loss validated:");
    println!("  - All 4 findings preserved");
    println!("  - All 2 scores preserved");
    println!("  - Max value computed correctly");
    println!("  - All log messages preserved");
    println!("  - Gap #1 is ACTUALLY FIXED");

    Ok(())
}

#[tokio::test]
async fn test_parallel_execution_three_branches() -> dashflow::Result<()> {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // Three parallel branches adding data
    graph.add_node_from_fn("branch1", |mut state| {
        Box::pin(async move {
            state.findings.push("B1".to_string());
            state.scores.push(100);
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch2", |mut state| {
        Box::pin(async move {
            state.findings.push("B2".to_string());
            state.scores.push(200);
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch3", |mut state| {
        Box::pin(async move {
            state.findings.push("B3".to_string());
            state.scores.push(300);
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec![
            "branch1".to_string(),
            "branch2".to_string(),
            "branch3".to_string(),
        ],
    );
    graph.add_edge("branch1", END);
    graph.add_edge("branch2", END);
    graph.add_edge("branch3", END);

    let app = graph.compile_with_merge()?;

    let result = app
        .invoke(TestState {
            findings: vec![],
            scores: vec![],
            max_value: 0,
            message_log: String::new(),
        })
        .await
        ?;

    // All 3 findings must be present
    assert_eq!(result.final_state.findings.len(), 3);
    assert!(result.final_state.findings.contains(&"B1".to_string()));
    assert!(result.final_state.findings.contains(&"B2".to_string()));
    assert!(result.final_state.findings.contains(&"B3".to_string()));

    // All 3 scores must be present
    assert_eq!(result.final_state.scores.len(), 3);
    assert!(result.final_state.scores.contains(&100));
    assert!(result.final_state.scores.contains(&200));
    assert!(result.final_state.scores.contains(&300));

    Ok(())
}

#[tokio::test]
async fn test_sequential_execution_still_works() -> dashflow::Result<()> {
    // Verify that sequential (non-parallel) execution still works correctly
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("step1", |mut state| {
        Box::pin(async move {
            state.findings.push("Step1".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("step2", |mut state| {
        Box::pin(async move {
            state.findings.push("Step2".to_string());
            Ok(state)
        })
    });

    graph.set_entry_point("step1");
    graph.add_edge("step1", "step2");
    graph.add_edge("step2", END);

    let app = graph.compile()?;

    let result = app
        .invoke(TestState {
            findings: vec![],
            scores: vec![],
            max_value: 0,
            message_log: String::new(),
        })
        .await
        ?;

    // Sequential execution: findings processed in order
    assert_eq!(result.final_state.findings, vec!["Step1", "Step2"]);

    Ok(())
}

#[tokio::test]
async fn test_parallel_would_fail_with_last_write_wins() -> dashflow::Result<()> {
    // This test PROVES that last-write-wins is NOT being used
    // If last-write-wins were used, this test would FAIL
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // Branch A: Only adds to findings
    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            state.findings.push("ONLY_IN_A".to_string());
            // Explicitly NOT touching scores
            Ok(state)
        })
    });

    // Branch B: Only adds to scores
    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            state.scores.push(999);
            // Explicitly NOT touching findings
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

    let app = graph.compile_with_merge()?;

    let result = app
        .invoke(TestState {
            findings: vec![],
            scores: vec![],
            max_value: 0,
            message_log: String::new(),
        })
        .await
        ?;

    // With last-write-wins: Would have EITHER findings OR scores, NOT both
    // With proper merge: MUST have BOTH

    // This assertion would FAIL with last-write-wins
    assert_eq!(
        result.final_state.findings.len(),
        1,
        "CRITICAL: If this is 0, last-write-wins is being used and Gap #1 is NOT fixed"
    );
    assert_eq!(
        result.final_state.scores.len(),
        1,
        "CRITICAL: If this is 0, last-write-wins is being used and Gap #1 is NOT fixed"
    );

    assert!(
        result
            .final_state
            .findings
            .contains(&"ONLY_IN_A".to_string()),
        "Missing data from branch_a - last-write-wins detected!"
    );
    assert!(
        result.final_state.scores.contains(&999),
        "Missing data from branch_b - last-write-wins detected!"
    );

    println!("✅ DEFINITIVE PROOF: Both branches' data present. Last-write-wins NOT used.");

    Ok(())
}
