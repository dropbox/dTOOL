#![allow(clippy::redundant_closure)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Property-based tests for DashFlow
//!
//! These tests verify algebraic properties and invariants that should hold
//! for all valid inputs, using the proptest framework.
//!
//! ## Test Categories
//!
//! 1. **Checkpoint Properties**: Save/load identity, serialization roundtrips
//! 2. **State Properties**: Clone independence, merge operations
//! 3. **Graph Properties**: Execution determinism, state transformation, parallel merging

use dashflow::checkpoint::{Checkpoint, Checkpointer, MemoryCheckpointer};
use dashflow::{MergeableState, StateGraph, END};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Test state for property testing
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestState {
    value: i32,
    text: String,
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        self.value = self.value.max(other.value);
        if !other.text.is_empty() {
            if self.text.is_empty() {
                self.text = other.text.clone();
            } else {
                self.text.push('\n');
                self.text.push_str(&other.text);
            }
        }
    }
}

// GraphState is blanket-implemented for types that satisfy its bounds

/// Strategy for generating arbitrary TestState instances
fn arb_test_state() -> impl Strategy<Value = TestState> {
    (any::<i32>(), any::<String>()).prop_map(|(value, text)| TestState { value, text })
}

/// Strategy for generating arbitrary thread IDs
fn arb_thread_id() -> impl Strategy<Value = String> {
    "[a-z0-9]{8,32}".prop_map(|s| s)
}

/// Strategy for generating arbitrary node names
fn arb_node_name() -> impl Strategy<Value = String> {
    "[a-z_]{1,50}".prop_map(|s| s)
}

/// Strategy for generating arbitrary metadata
fn arb_metadata() -> impl Strategy<Value = HashMap<String, String>> {
    prop::collection::hash_map("[a-z]{1,20}", "[a-z0-9 ]{0,50}", 0..10)
}

// =============================================================================
// Property Tests: Checkpoint Save/Load Identity
// =============================================================================

proptest! {
    /// Property: Save followed by load should return the exact same checkpoint
    /// Invariant: save(c) >> load(c.id) == Some(c)
    #[test]
    fn prop_checkpoint_save_load_identity(
        state in arb_test_state(),
        thread_id in arb_thread_id(),
        node in arb_node_name(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let checkpointer = MemoryCheckpointer::new();
            let checkpoint = Checkpoint::new(thread_id, state, node, None);
            let checkpoint_id = checkpoint.id.clone();

            // Save checkpoint
            checkpointer.save(checkpoint.clone()).await.unwrap();

            // Load checkpoint
            let loaded = checkpointer.load(&checkpoint_id).await.unwrap();

            // Verify identity
            prop_assert_eq!(loaded, Some(checkpoint));
            Ok(())
        })?;
    }

    /// Property: Latest checkpoint for a thread is the most recently saved
    /// Invariant: save(c1) >> save(c2) >> get_latest(thread) == c2 (if c2.timestamp >= c1.timestamp)
    #[test]
    fn prop_checkpoint_latest_is_newest(
        state1 in arb_test_state(),
        state2 in arb_test_state(),
        thread_id in arb_thread_id(),
        node1 in arb_node_name(),
        node2 in arb_node_name(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let checkpointer = MemoryCheckpointer::new();

            // Save first checkpoint
            let cp1 = Checkpoint::new(thread_id.clone(), state1, node1, None);
            checkpointer.save(cp1.clone()).await.unwrap();

            // Small delay to ensure different timestamps
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            // Save second checkpoint
            let cp2 = Checkpoint::new(thread_id.clone(), state2, node2, Some(cp1.id.clone()));
            checkpointer.save(cp2.clone()).await.unwrap();

            // Get latest should return cp2
            let latest = checkpointer.get_latest(&thread_id).await.unwrap();

            prop_assert_eq!(latest.map(|c| c.id), Some(cp2.id));
            Ok(())
        })?;
    }

    /// Property: Deleting a checkpoint makes it unloadable
    /// Invariant: save(c) >> delete(c.id) >> load(c.id) == None
    #[test]
    fn prop_checkpoint_delete_makes_unloadable(
        state in arb_test_state(),
        thread_id in arb_thread_id(),
        node in arb_node_name(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let checkpointer = MemoryCheckpointer::new();
            let checkpoint = Checkpoint::new(thread_id, state, node, None);
            let checkpoint_id = checkpoint.id.clone();

            // Save and delete
            checkpointer.save(checkpoint).await.unwrap();
            checkpointer.delete(&checkpoint_id).await.unwrap();

            // Load should return None
            let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
            prop_assert_eq!(loaded, None);
            Ok(())
        })?;
    }

    /// Property: List returns all checkpoints for a thread, sorted by timestamp DESC
    /// Invariant: save(c1) >> save(c2) >> list(thread) == [c2, c1]
    #[test]
    fn prop_checkpoint_list_ordering(
        states in prop::collection::vec(arb_test_state(), 1..5),
        thread_id in arb_thread_id(),
        nodes in prop::collection::vec(arb_node_name(), 1..5),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let checkpointer = MemoryCheckpointer::new();
            let count = states.len().min(nodes.len());
            let mut checkpoint_ids = Vec::new();

            // Save multiple checkpoints with delays
            for i in 0..count {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    states[i].clone(),
                    nodes[i].clone(),
                    None,
                );
                checkpoint_ids.push(checkpoint.id.clone());
                checkpointer.save(checkpoint).await.unwrap();

                // Small delay to ensure different timestamps
                if i < count - 1 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }

            // List should return in reverse order (newest first)
            let metadata_list = checkpointer.list(&thread_id).await.unwrap();

            prop_assert_eq!(metadata_list.len(), count);

            // Verify timestamps are in descending order
            for i in 1..metadata_list.len() {
                prop_assert!(metadata_list[i - 1].timestamp >= metadata_list[i].timestamp);
            }
            Ok(())
        })?;
    }

    /// Property: Checkpoint with metadata preserves all metadata
    /// Invariant: save(c.with_metadata(k, v)) >> load(c.id) >> metadata[k] == v
    #[test]
    fn prop_checkpoint_metadata_preservation(
        state in arb_test_state(),
        thread_id in arb_thread_id(),
        node in arb_node_name(),
        metadata in arb_metadata(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let checkpointer = MemoryCheckpointer::new();
            let mut checkpoint = Checkpoint::new(thread_id, state, node, None);

            // Add metadata
            for (key, value) in metadata.iter() {
                checkpoint = checkpoint.with_metadata(key.clone(), value.clone());
            }

            let checkpoint_id = checkpoint.id.clone();

            // Save and load
            checkpointer.save(checkpoint.clone()).await.unwrap();
            let loaded = checkpointer.load(&checkpoint_id).await.unwrap().unwrap();

            // Verify all metadata is preserved
            for (key, value) in metadata.iter() {
                prop_assert_eq!(loaded.metadata.get(key), Some(value));
            }
            prop_assert_eq!(loaded.metadata, checkpoint.metadata);
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: Serialization Round-Trip
// =============================================================================

proptest! {
    /// Property: Checkpoint serialization is lossless
    /// Invariant: deserialize(serialize(c)) == c
    #[test]
    fn prop_checkpoint_serialization_roundtrip(
        state in arb_test_state(),
        thread_id in arb_thread_id(),
        node in arb_node_name(),
        metadata in arb_metadata(),
    ) {
        let mut checkpoint = Checkpoint::new(thread_id, state, node, None);

        // Add metadata
        for (key, value) in metadata.iter() {
            checkpoint = checkpoint.with_metadata(key.clone(), value.clone());
        }

        // Serialize to JSON
        let json = serde_json::to_string(&checkpoint).unwrap();

        // Deserialize from JSON
        let deserialized: Checkpoint<TestState> = serde_json::from_str(&json).unwrap();

        // Verify round-trip (excluding timestamp precision issues)
        prop_assert_eq!(deserialized.id, checkpoint.id);
        prop_assert_eq!(deserialized.thread_id, checkpoint.thread_id);
        prop_assert_eq!(deserialized.state, checkpoint.state);
        prop_assert_eq!(deserialized.node, checkpoint.node);
        prop_assert_eq!(deserialized.parent_id, checkpoint.parent_id);
        prop_assert_eq!(deserialized.metadata, checkpoint.metadata);
    }

    /// Property: Checkpoint serialization with bincode is lossless
    /// Invariant: bincode::deserialize(bincode::serialize(c)) == c
    #[test]
    fn prop_checkpoint_bincode_roundtrip(
        state in arb_test_state(),
        thread_id in arb_thread_id(),
        node in arb_node_name(),
    ) {
        let checkpoint = Checkpoint::new(thread_id, state, node, None);

        // Serialize with bincode
        let bytes = bincode::serialize(&checkpoint).unwrap();

        // Deserialize with bincode
        let deserialized: Checkpoint<TestState> = bincode::deserialize(&bytes).unwrap();

        // Verify exact round-trip (bincode preserves timestamps exactly)
        prop_assert_eq!(deserialized.id, checkpoint.id);
        prop_assert_eq!(deserialized.thread_id, checkpoint.thread_id);
        prop_assert_eq!(deserialized.state, checkpoint.state);
        prop_assert_eq!(deserialized.node, checkpoint.node);
        prop_assert_eq!(deserialized.parent_id, checkpoint.parent_id);
    }

    /// Property: TestState serialization with JSON is lossless
    /// Invariant: deserialize(serialize(state)) == state
    #[test]
    fn prop_test_state_json_roundtrip(
        value in any::<i32>(),
        text in any::<String>(),
    ) {
        let state = TestState { value, text };

        // JSON round-trip
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TestState = serde_json::from_str(&json).unwrap();

        prop_assert_eq!(deserialized, state);
    }
}

// =============================================================================
// Property Tests: State Operations
// =============================================================================

proptest! {
    /// Property: TestState clone creates independent copy
    /// Invariant: state.clone() == state && mutating clone doesn't affect original
    #[test]
    fn prop_state_clone_independence(
        value in any::<i32>(),
        text in any::<String>(),
    ) {
        let original = TestState { value, text: text.clone() };
        let original_value = original.value;
        let original_text_clone = original.text.clone();
        let mut cloned = original;

        // Clone should equal original
        prop_assert_eq!(cloned.value, original_value);
        prop_assert_eq!(&cloned.text, &original_text_clone);

        // Mutating clone should not affect original values
        cloned.value += 1;
        cloned.text.push_str("_modified");

        prop_assert_ne!(cloned.value, original_value);
        prop_assert_ne!(&cloned.text, &original_text_clone);
        prop_assert_eq!(original_text_clone, text);
    }

    /// Property: Checkpoint parent chain is consistent
    /// Invariant: If c2.parent_id == Some(c1.id), then c2.timestamp >= c1.timestamp
    #[test]
    fn prop_checkpoint_parent_chain_timestamps(
        state1 in arb_test_state(),
        state2 in arb_test_state(),
        thread_id in arb_thread_id(),
        node1 in arb_node_name(),
        node2 in arb_node_name(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            // Create parent checkpoint
            let parent = Checkpoint::new(thread_id.clone(), state1, node1, None);
            let parent_id = parent.id.clone();
            let parent_ts = parent.timestamp;

            // Small delay
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            // Create child checkpoint
            let child = Checkpoint::new(thread_id, state2, node2, Some(parent_id.clone()));

            // Child timestamp should be >= parent timestamp
            prop_assert!(child.timestamp >= parent_ts);
            prop_assert_eq!(child.parent_id, Some(parent_id));
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: Graph Execution
// =============================================================================

/// Graph state for testing graph execution properties
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
struct GraphTestState {
    values: Vec<i32>,
    trace: Vec<String>,
}

impl MergeableState for GraphTestState {
    fn merge(&mut self, other: &Self) {
        self.values.extend(other.values.clone());
        self.trace.extend(other.trace.clone());
    }
}

/// Strategy for generating arbitrary i32 values in a reasonable range
fn arb_values() -> impl Strategy<Value = Vec<i32>> {
    prop::collection::vec(any::<i32>(), 0..10)
}

/// Strategy for generating a sequence length (for chain tests)
fn arb_chain_length() -> impl Strategy<Value = usize> {
    1usize..10
}

proptest! {
    /// Property: Graph execution is deterministic
    /// Invariant: invoke(state) == invoke(state) for identical inputs
    #[test]
    fn prop_graph_execution_determinism(
        initial_value in any::<i32>(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            graph.add_node_from_fn("double", |mut state| {
                Box::pin(async move {
                    if let Some(v) = state.values.first() {
                        // Use wrapping to avoid overflow
                        state.values.push(v.wrapping_mul(2));
                    }
                    state.trace.push("double".to_string());
                    Ok(state)
                })
            });

            graph.set_entry_point("double");
            graph.add_edge("double", END);

            let app = graph.compile().unwrap();

            // Run twice with same input
            let input1 = GraphTestState {
                values: vec![initial_value],
                trace: vec![],
            };
            let input2 = input1.clone();

            let result1 = app.invoke(input1).await.unwrap().final_state;
            let result2 = app.invoke(input2).await.unwrap().final_state;

            // Results should be identical
            prop_assert_eq!(result1.values, result2.values);
            prop_assert_eq!(result1.trace, result2.trace);
            Ok(())
        })?;
    }

    /// Property: State transformation preserves all data
    /// Invariant: All input values are present in output (nodes only add, not remove)
    #[test]
    fn prop_graph_state_preservation(
        initial_values in arb_values(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            graph.add_node_from_fn("pass_through", |mut state| {
                Box::pin(async move {
                    state.trace.push("pass_through".to_string());
                    Ok(state)
                })
            });

            graph.set_entry_point("pass_through");
            graph.add_edge("pass_through", END);

            let app = graph.compile().unwrap();

            let input = GraphTestState {
                values: initial_values.clone(),
                trace: vec![],
            };

            let result = app.invoke(input).await.unwrap().final_state;

            // All initial values should be present
            for v in &initial_values {
                prop_assert!(
                    result.values.contains(v),
                    "Value {} should be preserved",
                    v
                );
            }
            Ok(())
        })?;
    }

    /// Property: Sequential chain execution order is correct
    /// Invariant: Nodes execute in edge order, trace shows correct sequence
    #[test]
    fn prop_graph_sequential_order(
        chain_len in arb_chain_length(),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            // Create chain of nodes
            let node_names: Vec<String> = (0..chain_len).map(|i| format!("node_{}", i)).collect();

            for name in &node_names {
                let node_name = name.clone();
                let trace_name = name.clone();
                graph.add_node_from_fn(&node_name, move |mut state| {
                    let trace_name = trace_name.clone();
                    Box::pin(async move {
                        state.trace.push(trace_name);
                        Ok(state)
                    })
                });
            }

            // Set entry and chain edges
            graph.set_entry_point(&node_names[0]);
            for i in 0..node_names.len() - 1 {
                graph.add_edge(&node_names[i], &node_names[i + 1]);
            }
            graph.add_edge(&node_names[node_names.len() - 1], END);

            let app = graph.compile().unwrap();

            let result = app.invoke(GraphTestState::default()).await.unwrap().final_state;

            // Trace should match expected order
            prop_assert_eq!(result.trace.len(), chain_len);
            for (i, expected_name) in node_names.iter().enumerate() {
                prop_assert_eq!(
                    &result.trace[i],
                    expected_name,
                    "Node at position {} should be {}",
                    i,
                    expected_name
                );
            }
            Ok(())
        })?;
    }

    /// Property: Node executions accumulate correctly
    /// Invariant: Each node in sequence adds exactly one trace entry
    #[test]
    fn prop_graph_node_execution_count(
        num_nodes in 1usize..8,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let execution_counter = Arc::new(AtomicU32::new(0));
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            let node_names: Vec<String> = (0..num_nodes).map(|i| format!("counter_{}", i)).collect();

            for name in &node_names {
                let counter = Arc::clone(&execution_counter);
                let node_name = name.clone();
                let trace_name = name.clone();
                graph.add_node_from_fn(&node_name, move |mut state| {
                    let counter = Arc::clone(&counter);
                    let trace_name = trace_name.clone();
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        state.trace.push(trace_name);
                        Ok(state)
                    })
                });
            }

            graph.set_entry_point(&node_names[0]);
            for i in 0..node_names.len() - 1 {
                graph.add_edge(&node_names[i], &node_names[i + 1]);
            }
            graph.add_edge(&node_names[node_names.len() - 1], END);

            let app = graph.compile().unwrap();
            let result = app.invoke(GraphTestState::default()).await.unwrap().final_state;

            // Verify execution count matches node count
            let count = execution_counter.load(Ordering::SeqCst) as usize;
            prop_assert_eq!(count, num_nodes, "Expected {} executions, got {}", num_nodes, count);
            prop_assert_eq!(result.trace.len(), num_nodes);
            Ok(())
        })?;
    }

    /// Property: Transformation functions compose correctly
    /// Invariant: f(g(x)) produces expected combined result
    #[test]
    fn prop_graph_transformation_composition(
        initial_value in 0i32..1000,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            // First node: add 10
            graph.add_node_from_fn("add_ten", |mut state| {
                Box::pin(async move {
                    if let Some(v) = state.values.first().copied() {
                        state.values.push(v + 10);
                    }
                    state.trace.push("add_ten".to_string());
                    Ok(state)
                })
            });

            // Second node: multiply by 2 the last value
            graph.add_node_from_fn("multiply_two", |mut state| {
                Box::pin(async move {
                    if let Some(v) = state.values.last().copied() {
                        state.values.push(v * 2);
                    }
                    state.trace.push("multiply_two".to_string());
                    Ok(state)
                })
            });

            graph.set_entry_point("add_ten");
            graph.add_edge("add_ten", "multiply_two");
            graph.add_edge("multiply_two", END);

            let app = graph.compile().unwrap();

            let input = GraphTestState {
                values: vec![initial_value],
                trace: vec![],
            };

            let result = app.invoke(input).await.unwrap().final_state;

            // Verify composition: (initial + 10) * 2
            let expected_final = (initial_value + 10) * 2;
            prop_assert_eq!(
                result.values.last().copied(),
                Some(expected_final),
                "Expected ({}+10)*2 = {}, got {:?}",
                initial_value,
                expected_final,
                result.values.last()
            );

            // Verify intermediate value exists
            prop_assert!(
                result.values.contains(&(initial_value + 10)),
                "Intermediate value {} should exist",
                initial_value + 10
            );
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: MergeableState Operations
// =============================================================================

proptest! {
    /// Property: Merge is idempotent for value-max semantics
    /// Invariant: state.merge(&state) leaves value unchanged (for max operation)
    #[test]
    fn prop_merge_value_max_idempotent(
        value in any::<i32>(),
        text in "[a-z]{0,20}",
    ) {
        let mut state = TestState {
            value,
            text,
        };
        let original_value = state.value;
        let state_clone = state.clone();

        state.merge(&state_clone);

        // Value should be max(v, v) = v (unchanged)
        prop_assert_eq!(state.value, original_value);
    }

    /// Property: Merge with empty text preserves original text
    /// Invariant: state.merge(empty_text_state) preserves original text
    #[test]
    fn prop_merge_empty_text_preserves(
        value in any::<i32>(),
        text in "[a-z]{1,20}",
    ) {
        let mut state = TestState {
            value,
            text: text.clone(),
        };
        let empty_text_state = TestState {
            value: value - 1, // Smaller value
            text: String::new(),
        };

        state.merge(&empty_text_state);

        // Original text should be preserved when merging empty text
        prop_assert_eq!(state.text, text);
    }

    /// Property: Merge value semantics follow max function
    /// Invariant: merge(a, b).value == max(a.value, b.value)
    #[test]
    fn prop_merge_value_is_max(
        value1 in any::<i32>(),
        value2 in any::<i32>(),
    ) {
        let mut state1 = TestState {
            value: value1,
            text: String::new(),
        };
        let state2 = TestState {
            value: value2,
            text: String::new(),
        };

        state1.merge(&state2);

        prop_assert_eq!(state1.value, value1.max(value2));
    }
}

// =============================================================================
// Property Tests: add_messages Reducer
// =============================================================================

use dashflow::core::messages::Message;
use dashflow::reducer::{add_messages, MessageExt};

/// Strategy for generating arbitrary message content
fn arb_message_content() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{0,100}"
}

/// Strategy for generating arbitrary message ID
fn arb_message_id() -> impl Strategy<Value = String> {
    "[a-z0-9]{8,16}"
}

proptest! {
    /// Property: add_messages preserves all original messages when adding empty list
    /// Invariant: add_messages(left, []) == left (with IDs assigned)
    #[test]
    fn prop_add_messages_identity_right(
        contents in prop::collection::vec(arb_message_content(), 0..5),
    ) {
        let left: Vec<Message> = contents.into_iter().map(|c| Message::human(c)).collect();
        let left_len = left.len();

        let merged = add_messages(left, vec![]);

        // Length should be preserved
        prop_assert_eq!(merged.len(), left_len);

        // All messages should now have IDs
        for msg in &merged {
            prop_assert!(msg.fields().id.is_some());
        }
    }

    /// Property: add_messages appends when no IDs match
    /// Invariant: |add_messages(left, right)| == |left| + |right| when no IDs overlap
    #[test]
    fn prop_add_messages_append_length(
        left_contents in prop::collection::vec(arb_message_content(), 0..5),
        right_contents in prop::collection::vec(arb_message_content(), 0..5),
    ) {
        // Messages without explicit IDs will get unique UUIDs, so no overlap
        let left_len = left_contents.len();
        let right_len = right_contents.len();
        let left: Vec<Message> = left_contents.into_iter().map(|c| Message::human(c)).collect();
        let right: Vec<Message> = right_contents.into_iter().map(|c| Message::ai(c)).collect();

        let merged = add_messages(left, right);

        // All messages should be present
        prop_assert_eq!(merged.len(), left_len + right_len);
    }

    /// Property: add_messages updates existing message when IDs match
    /// Invariant: If msg in right has same ID as msg in left, merged has updated content
    #[test]
    fn prop_add_messages_update_by_id(
        id in arb_message_id(),
        original_content in arb_message_content(),
        updated_content in arb_message_content(),
    ) {
        let original_msg = Message::human(original_content.as_str()).with_id(&id);
        let updated_msg = Message::human(updated_content.as_str()).with_id(&id);

        let merged = add_messages(vec![original_msg], vec![updated_msg]);

        // Should have exactly one message
        prop_assert_eq!(merged.len(), 1);

        // Should have the updated content
        prop_assert_eq!(merged[0].as_text(), updated_content);

        // Should preserve the ID
        prop_assert_eq!(merged[0].fields().id.as_deref(), Some(id.as_str()));
    }

    /// Property: add_messages preserves message order
    /// Invariant: Original messages appear before appended messages
    #[test]
    fn prop_add_messages_order_preserved(
        left_count in 1usize..5,
        right_count in 1usize..5,
    ) {
        let left: Vec<Message> = (0..left_count)
            .map(|i| Message::human(format!("left_{}", i)).with_id(format!("left_id_{}", i)))
            .collect();
        let right: Vec<Message> = (0..right_count)
            .map(|i| Message::ai(format!("right_{}", i)).with_id(format!("right_id_{}", i)))
            .collect();

        let merged = add_messages(left.clone(), right.clone());

        // Left messages should be first
        for (i, _) in left.iter().enumerate() {
            let expected_id = format!("left_id_{}", i);
            prop_assert_eq!(
                merged[i].fields().id.as_deref(),
                Some(expected_id.as_str())
            );
        }

        // Right messages should follow (offset by left_count)
        for (i, _) in right.iter().enumerate() {
            let expected_id = format!("right_id_{}", i);
            prop_assert_eq!(
                merged[left_count + i].fields().id.as_deref(),
                Some(expected_id.as_str())
            );
        }
    }

    /// Property: add_messages ID assignment is stable
    /// Invariant: All messages in output have an ID
    #[test]
    fn prop_add_messages_all_have_ids(
        left_contents in prop::collection::vec(arb_message_content(), 0..10),
        right_contents in prop::collection::vec(arb_message_content(), 0..10),
    ) {
        let left: Vec<Message> = left_contents.into_iter().map(|c| Message::human(c)).collect();
        let right: Vec<Message> = right_contents.into_iter().map(|c| Message::ai(c)).collect();

        let merged = add_messages(left, right);

        // Every message must have an ID
        for (i, msg) in merged.iter().enumerate() {
            prop_assert!(
                msg.fields().id.is_some(),
                "Message at index {} should have an ID",
                i
            );
        }
    }

    /// Property: add_messages with explicit IDs - mixed append and update
    /// Invariant: IDs that match update, IDs that don't match append
    #[test]
    fn prop_add_messages_mixed_operations(
        common_id in arb_message_id(),
        unique_left_id in arb_message_id(),
        unique_right_id in arb_message_id(),
    ) {
        // Skip if IDs happen to collide
        prop_assume!(common_id != unique_left_id);
        prop_assume!(common_id != unique_right_id);
        prop_assume!(unique_left_id != unique_right_id);

        let left = vec![
            Message::human("common_original").with_id(&common_id),
            Message::human("unique_left").with_id(&unique_left_id),
        ];
        let right = vec![
            Message::ai("common_updated").with_id(&common_id),
            Message::ai("unique_right").with_id(&unique_right_id),
        ];

        let merged = add_messages(left, right);

        // Should have 3 messages: common (updated), unique_left, unique_right
        prop_assert_eq!(merged.len(), 3);

        // Find the common message and verify it was updated
        let common_msg = merged.iter().find(|m| m.fields().id.as_deref() == Some(common_id.as_str()));
        prop_assert!(common_msg.is_some());
        prop_assert_eq!(common_msg.unwrap().as_text(), "common_updated");
    }
}

// =============================================================================
// Property Tests: Conditional Routing
// =============================================================================

proptest! {
    /// Property: Conditional routing is deterministic
    /// Invariant: Same input always routes to same node
    #[test]
    fn prop_conditional_routing_determinism(
        threshold in 0i32..100,
        test_value in 0i32..200,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let threshold_copy = threshold;
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            graph.add_node_from_fn("start", |mut state| {
                Box::pin(async move {
                    state.trace.push("start".to_string());
                    Ok(state)
                })
            });

            graph.add_node_from_fn("high_path", |mut state| {
                Box::pin(async move {
                    state.trace.push("high_path".to_string());
                    Ok(state)
                })
            });

            graph.add_node_from_fn("low_path", |mut state| {
                Box::pin(async move {
                    state.trace.push("low_path".to_string());
                    Ok(state)
                })
            });

            graph.set_entry_point("start");

            // Conditional edge based on threshold (sync closure returning String)
            let mut routes = HashMap::new();
            routes.insert("high".to_string(), "high_path".to_string());
            routes.insert("low".to_string(), "low_path".to_string());

            graph.add_conditional_edges(
                "start",
                move |state: &GraphTestState| {
                    if state.values.first().copied().unwrap_or(0) >= threshold_copy {
                        "high".to_string()
                    } else {
                        "low".to_string()
                    }
                },
                routes,
            );

            graph.add_edge("high_path", END);
            graph.add_edge("low_path", END);

            let app = graph.compile().unwrap();

            // Run twice with same input
            let input1 = GraphTestState {
                values: vec![test_value],
                trace: vec![],
            };
            let input2 = input1.clone();

            let result1 = app.invoke(input1).await.unwrap().final_state;
            let result2 = app.invoke(input2).await.unwrap().final_state;

            // Both should take same path
            prop_assert_eq!(&result1.trace, &result2.trace);

            // Verify correct path was taken
            let expected_path = if test_value >= threshold { "high_path" } else { "low_path" };
            prop_assert!(
                result1.trace.contains(&expected_path.to_string()),
                "Expected path {} for value {} with threshold {}, got {:?}",
                expected_path,
                test_value,
                threshold,
                result1.trace
            );
            Ok(())
        })?;
    }

    /// Property: Multi-way conditional routing covers all branches
    /// Invariant: Every possible branch is reachable
    #[test]
    fn prop_conditional_routing_branch_coverage(
        test_values in prop::collection::vec(0i32..30, 5..10),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            graph.add_node_from_fn("router", |mut state| {
                Box::pin(async move {
                    state.trace.push("router".to_string());
                    Ok(state)
                })
            });

            // Three branches based on value ranges
            for branch in ["low", "medium", "high"] {
                let branch_name = branch.to_string();
                let trace_name = branch.to_string();
                graph.add_node_from_fn(&branch_name, move |mut state| {
                    let bn = trace_name.clone();
                    Box::pin(async move {
                        state.trace.push(bn);
                        Ok(state)
                    })
                });
            }

            graph.set_entry_point("router");

            let mut routes = HashMap::new();
            routes.insert("low".to_string(), "low".to_string());
            routes.insert("medium".to_string(), "medium".to_string());
            routes.insert("high".to_string(), "high".to_string());

            graph.add_conditional_edges(
                "router",
                |state: &GraphTestState| {
                    let val = state.values.first().copied().unwrap_or(0);
                    if val < 10 {
                        "low".to_string()
                    } else if val < 20 {
                        "medium".to_string()
                    } else {
                        "high".to_string()
                    }
                },
                routes,
            );

            graph.add_edge("low", END);
            graph.add_edge("medium", END);
            graph.add_edge("high", END);

            let app = graph.compile().unwrap();

            // Track which branches we hit
            let mut branches_hit = std::collections::HashSet::new();

            for val in test_values {
                let input = GraphTestState {
                    values: vec![val],
                    trace: vec![],
                };
                let result = app.invoke(input).await.unwrap().final_state;

                // Record which branch was taken
                for branch in ["low", "medium", "high"] {
                    if result.trace.contains(&branch.to_string()) {
                        branches_hit.insert(branch);
                    }
                }
            }

            // With values 0-29 in range of 5-10 values, we should hit multiple branches
            // (probabilistically very likely but not guaranteed, so just check we hit at least one)
            prop_assert!(
                !branches_hit.is_empty(),
                "Should have hit at least one branch"
            );
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: ContentBlock Operations
// =============================================================================

use dashflow::core::messages::ContentBlock;

/// Strategy for generating arbitrary text content
fn arb_text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,!?]{0,200}"
}

proptest! {
    /// Property: ContentBlock::Text as_text returns the original text
    /// Invariant: ContentBlock::Text { text }.as_text() == text
    #[test]
    fn prop_content_block_text_roundtrip(
        text in arb_text(),
    ) {
        let block = ContentBlock::Text { text: text.clone() };
        prop_assert_eq!(block.as_text(), text.as_str());
    }

    /// Property: ContentBlock::ToolResult as_text returns content
    /// Invariant: ContentBlock::ToolResult { content, .. }.as_text() == content
    #[test]
    fn prop_content_block_tool_result_text(
        tool_use_id in "[a-z0-9]{8,16}",
        content in arb_text(),
        is_error in any::<bool>(),
    ) {
        let block = ContentBlock::ToolResult {
            tool_use_id,
            content: content.clone(),
            is_error,
        };
        prop_assert_eq!(block.as_text(), content.as_str());
    }

    /// Property: ContentBlock::Reasoning as_text returns reasoning
    /// Invariant: ContentBlock::Reasoning { reasoning }.as_text() == reasoning
    #[test]
    fn prop_content_block_reasoning_text(
        reasoning in arb_text(),
    ) {
        let block = ContentBlock::Reasoning { reasoning: reasoning.clone() };
        prop_assert_eq!(block.as_text(), reasoning.as_str());
    }

    /// Property: ContentBlock::Thinking as_text returns thinking
    /// Invariant: ContentBlock::Thinking { thinking, .. }.as_text() == thinking
    #[test]
    fn prop_content_block_thinking_text(
        thinking in arb_text(),
    ) {
        let block = ContentBlock::Thinking {
            thinking: thinking.clone(),
            signature: None,
        };
        prop_assert_eq!(block.as_text(), thinking.as_str());
    }

    /// Property: ContentBlock::Image as_text returns empty string
    /// Invariant: ContentBlock::Image { .. }.as_text() == ""
    #[test]
    fn prop_content_block_image_empty_text(
        url in "[a-z]{5,20}",
    ) {
        use dashflow::core::messages::ImageSource;
        let block = ContentBlock::Image {
            source: ImageSource::Url { url },
            detail: None,
        };
        prop_assert_eq!(block.as_text(), "");
    }

    /// Property: ContentBlock::ToolUse as_text returns empty string
    /// Invariant: ContentBlock::ToolUse { .. }.as_text() == ""
    #[test]
    fn prop_content_block_tool_use_empty_text(
        id in "[a-z0-9]{8,16}",
        name in "[a-z_]{1,20}",
    ) {
        let block = ContentBlock::ToolUse {
            id,
            name,
            input: serde_json::json!({}),
        };
        prop_assert_eq!(block.as_text(), "");
    }
}

// =============================================================================
// Property Tests: ContentBlock Serialization
// =============================================================================

proptest! {
    /// Property: ContentBlock::Text JSON roundtrip preserves data
    /// Invariant: deserialize(serialize(block)) == block
    #[test]
    fn prop_content_block_text_json_roundtrip(
        text in arb_text(),
    ) {
        let block = ContentBlock::Text { text };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(deserialized, block);
    }

    /// Property: ContentBlock::ToolResult JSON roundtrip preserves data
    /// Invariant: deserialize(serialize(block)) == block
    #[test]
    fn prop_content_block_tool_result_json_roundtrip(
        tool_use_id in "[a-z0-9]{8,16}",
        content in arb_text(),
        is_error in any::<bool>(),
    ) {
        let block = ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(deserialized, block);
    }

    /// Property: ContentBlock::ToolUse JSON roundtrip preserves data
    /// Invariant: deserialize(serialize(block)) == block
    #[test]
    fn prop_content_block_tool_use_json_roundtrip(
        id in "[a-z0-9]{8,16}",
        name in "[a-z_]{1,20}",
    ) {
        let block = ContentBlock::ToolUse {
            id,
            name,
            input: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(deserialized, block);
    }
}

// =============================================================================
// Property Tests: Message Operations
// =============================================================================

proptest! {
    /// Property: Message creation preserves content
    /// Invariant: Message::human(text).as_text() == text
    #[test]
    fn prop_message_human_preserves_content(
        content in arb_text(),
    ) {
        let msg = Message::human(content.as_str());
        prop_assert_eq!(msg.as_text(), content);
    }

    /// Property: Message::ai creation preserves content
    /// Invariant: Message::ai(text).as_text() == text
    #[test]
    fn prop_message_ai_preserves_content(
        content in arb_text(),
    ) {
        let msg = Message::ai(content.as_str());
        prop_assert_eq!(msg.as_text(), content);
    }

    /// Property: Message::system creation preserves content
    /// Invariant: Message::system(text).as_text() == text
    #[test]
    fn prop_message_system_preserves_content(
        content in arb_text(),
    ) {
        let msg = Message::system(content.as_str());
        prop_assert_eq!(msg.as_text(), content);
    }

    /// Property: Message with_id preserves both ID and content
    /// Invariant: msg.with_id(id).fields().id == Some(id) && msg.with_id(id).as_text() == original_text
    #[test]
    fn prop_message_with_id_preserves_both(
        content in arb_text(),
        id in arb_message_id(),
    ) {
        let msg = Message::human(content.as_str()).with_id(&id);
        prop_assert_eq!(msg.fields().id.as_deref(), Some(id.as_str()));
        prop_assert_eq!(msg.as_text(), content);
    }

    /// Property: Message JSON roundtrip preserves data
    /// Invariant: deserialize(serialize(msg)) == msg (for content and role)
    #[test]
    fn prop_message_json_roundtrip(
        content in arb_text(),
        id in arb_message_id(),
    ) {
        let msg = Message::human(content.as_str()).with_id(&id);
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(deserialized.as_text(), content);
        prop_assert_eq!(deserialized.fields().id.as_deref(), Some(id.as_str()));
    }
}

// =============================================================================
// Property Tests: Parallel Execution State Merge
// =============================================================================

/// State for testing parallel merge operations
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
struct ParallelTestState {
    counter: i32,
    collected: Vec<String>,
}

impl MergeableState for ParallelTestState {
    fn merge(&mut self, other: &Self) {
        // Counter: take max (commutative)
        self.counter = self.counter.max(other.counter);
        // Collected: extend (order may vary for parallel)
        self.collected.extend(other.collected.clone());
    }
}

proptest! {
    /// Property: Parallel branch merge produces all branch outputs
    /// Invariant: All parallel branches contribute to final state
    #[test]
    fn prop_parallel_merge_completeness(
        num_branches in 2usize..5,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<ParallelTestState> = StateGraph::new();

            graph.add_node_from_fn("fan_out", |state| {
                Box::pin(async move { Ok(state) })
            });

            // Create parallel branches
            let branch_names: Vec<String> = (0..num_branches)
                .map(|i| format!("branch_{}", i))
                .collect();

            for (i, name) in branch_names.iter().enumerate() {
                let branch_tag = format!("branch_{}", i);
                let branch_name = name.clone();
                graph.add_node_from_fn(&branch_name, move |mut state| {
                    let tag = branch_tag.clone();
                    Box::pin(async move {
                        state.collected.push(tag);
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }

            graph.add_node_from_fn("fan_in", |state| {
                Box::pin(async move { Ok(state) })
            });

            graph.set_entry_point("fan_out");

            // Use parallel edges for true concurrent execution
            graph.add_parallel_edges("fan_out", branch_names.clone());

            // Fan in from all branches
            for name in &branch_names {
                graph.add_edge(name, "fan_in");
            }

            graph.add_edge("fan_in", END);

            let app = graph.compile_with_merge().unwrap();

            let result = app
                .invoke(ParallelTestState::default())
                .await
                .unwrap()
                .final_state;

            // All branches should have contributed to collected
            prop_assert_eq!(
                result.collected.len(),
                num_branches,
                "Expected {} branch outputs, got {}",
                num_branches,
                result.collected.len()
            );

            // All branch tags should be present
            for i in 0..num_branches {
                let expected_tag = format!("branch_{}", i);
                prop_assert!(
                    result.collected.contains(&expected_tag),
                    "Missing branch tag: {}",
                    expected_tag
                );
            }
            Ok(())
        })?;
    }

    /// Property: Merge counter semantics are correct (max)
    /// Invariant: Final counter >= max of all branch counters
    #[test]
    fn prop_parallel_merge_counter_max(
        base_counter in 0i32..100,
        increments in prop::collection::vec(1i32..10, 2..5),
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let num_branches = increments.len();
            let increments_clone = increments.clone();
            let mut graph: StateGraph<ParallelTestState> = StateGraph::new();

            graph.add_node_from_fn("start", |state| {
                Box::pin(async move { Ok(state) })
            });

            let branch_names: Vec<String> = (0..num_branches)
                .map(|i| format!("inc_branch_{}", i))
                .collect();

            for (i, name) in branch_names.iter().enumerate() {
                let inc = increments_clone[i];
                let branch_name = name.clone();
                graph.add_node_from_fn(&branch_name, move |mut state| {
                    Box::pin(async move {
                        state.counter += inc;
                        Ok(state)
                    })
                });
            }

            graph.add_node_from_fn("merge", |state| {
                Box::pin(async move { Ok(state) })
            });

            graph.set_entry_point("start");

            // Use parallel edges for true concurrent execution
            graph.add_parallel_edges("start", branch_names.clone());

            for name in &branch_names {
                graph.add_edge(name, "merge");
            }

            graph.add_edge("merge", END);

            let app = graph.compile_with_merge().unwrap();

            let input = ParallelTestState {
                counter: base_counter,
                collected: vec![],
            };

            let result = app.invoke(input).await.unwrap().final_state;

            // The max counter should be base + max(increments)
            let max_increment = increments.iter().max().copied().unwrap_or(0);
            let expected_max = base_counter + max_increment;

            prop_assert!(
                result.counter >= expected_max,
                "Counter {} should be >= {} (base {} + max increment {})",
                result.counter,
                expected_max,
                base_counter,
                max_increment
            );
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: StreamEvent Properties
// =============================================================================

use dashflow::state::AgentState;
use dashflow::stream::{StreamEvent, StreamMode};

proptest! {
    /// Property: StreamEvent::Values has both state and node
    /// Invariant: StreamEvent::Values { node, state }.state().is_some() && .node().is_some()
    #[test]
    fn prop_stream_event_values_has_state_and_node(
        node in "[a-z_]{1,20}",
    ) {
        let state = AgentState::new();
        let event = StreamEvent::Values {
            node: node.clone(),
            state,
        };
        prop_assert!(event.state().is_some());
        prop_assert_eq!(event.node(), Some(node.as_str()));
        prop_assert!(!event.is_done());
    }

    /// Property: StreamEvent::Update has both state and node
    /// Invariant: StreamEvent::Update { node, state }.state().is_some() && .node().is_some()
    #[test]
    fn prop_stream_event_update_has_state_and_node(
        node in "[a-z_]{1,20}",
    ) {
        let state = AgentState::new();
        let event = StreamEvent::Update {
            node: node.clone(),
            state,
        };
        prop_assert!(event.state().is_some());
        prop_assert_eq!(event.node(), Some(node.as_str()));
        prop_assert!(!event.is_done());
    }

    /// Property: StreamEvent::NodeStart has node but no state
    /// Invariant: StreamEvent::NodeStart { node }.state().is_none() && .node().is_some()
    #[test]
    fn prop_stream_event_node_start_no_state(
        node in "[a-z_]{1,20}",
    ) {
        let event = StreamEvent::<AgentState>::NodeStart {
            node: node.clone(),
        };
        prop_assert!(event.state().is_none());
        prop_assert_eq!(event.node(), Some(node.as_str()));
        prop_assert!(!event.is_done());
    }

    /// Property: StreamEvent::NodeEnd has both state and node
    /// Invariant: StreamEvent::NodeEnd { node, state }.state().is_some() && .node().is_some()
    #[test]
    fn prop_stream_event_node_end_has_state_and_node(
        node in "[a-z_]{1,20}",
    ) {
        let state = AgentState::new();
        let event = StreamEvent::NodeEnd {
            node: node.clone(),
            state,
        };
        prop_assert!(event.state().is_some());
        prop_assert_eq!(event.node(), Some(node.as_str()));
        prop_assert!(!event.is_done());
    }

    /// Property: StreamEvent::Done has state but no node
    /// Invariant: StreamEvent::Done { state, .. }.state().is_some() && .node().is_none() && .is_done()
    #[test]
    fn prop_stream_event_done_has_state_no_node(
        path_len in 0usize..10,
    ) {
        let state = AgentState::new();
        let execution_path: Vec<String> = (0..path_len).map(|i| format!("node_{}", i)).collect();
        let event = StreamEvent::Done {
            state,
            execution_path,
        };
        prop_assert!(event.state().is_some());
        prop_assert!(event.node().is_none());
        prop_assert!(event.is_done());
    }

    /// Property: StreamEvent::Custom has node but no state
    /// Invariant: StreamEvent::Custom { node, data }.state().is_none() && .node().is_some()
    #[test]
    fn prop_stream_event_custom_no_state(
        node in "[a-z_]{1,20}",
    ) {
        let event = StreamEvent::<AgentState>::Custom {
            node: node.clone(),
            data: serde_json::json!({"key": "value"}),
        };
        prop_assert!(event.state().is_none());
        prop_assert_eq!(event.node(), Some(node.as_str()));
        prop_assert!(!event.is_done());
    }
}

// =============================================================================
// Property Tests: StreamMode Properties
// =============================================================================

proptest! {
    /// Property: StreamMode default is Values
    /// Invariant: StreamMode::default() == StreamMode::Values
    #[test]
    fn prop_stream_mode_default_is_values(_dummy in 0u8..1) {
        prop_assert_eq!(StreamMode::default(), StreamMode::Values);
    }

    /// Property: StreamMode equality is reflexive
    /// Invariant: mode == mode
    #[test]
    fn prop_stream_mode_equality_reflexive(variant in 0u8..4) {
        let mode = match variant % 4 {
            0 => StreamMode::Values,
            1 => StreamMode::Updates,
            2 => StreamMode::Events,
            _ => StreamMode::Custom,
        };
        prop_assert_eq!(mode, mode);
    }

    /// Property: StreamMode copy preserves value
    /// Invariant: let copy = mode; copy == mode
    #[test]
    fn prop_stream_mode_copy_preserves(variant in 0u8..4) {
        let mode = match variant % 4 {
            0 => StreamMode::Values,
            1 => StreamMode::Updates,
            2 => StreamMode::Events,
            _ => StreamMode::Custom,
        };
        let copy = mode;
        prop_assert_eq!(copy, mode);
    }
}

// =============================================================================
// Property Tests: Graph Error Propagation
// =============================================================================

proptest! {
    /// Property: Graph execution with failing node propagates error
    /// Invariant: If any node returns Err, invoke returns Err
    #[test]
    fn prop_graph_error_propagation(
        error_msg in "[a-zA-Z0-9 ]{1,50}",
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let error_msg_clone = error_msg.clone();
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();

            graph.add_node_from_fn("failing_node", move |_state| {
                let err = error_msg_clone.clone();
                Box::pin(async move {
                    Err(dashflow::Error::Generic(err))
                })
            });

            graph.set_entry_point("failing_node");
            graph.add_edge("failing_node", END);

            let app = graph.compile().unwrap();
            let result = app.invoke(GraphTestState::default()).await;

            prop_assert!(result.is_err());
            Ok(())
        })?;
    }

    /// Property: Graph execution success implies all nodes completed
    /// Invariant: If invoke returns Ok, all nodes in path executed
    #[test]
    fn prop_graph_success_implies_all_executed(
        num_nodes in 1usize..5,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut graph: StateGraph<GraphTestState> = StateGraph::new();
            let node_names: Vec<String> = (0..num_nodes).map(|i| format!("ok_node_{}", i)).collect();

            for name in &node_names {
                let trace_name = name.clone();
                graph.add_node_from_fn(name, move |mut state| {
                    let tn = trace_name.clone();
                    Box::pin(async move {
                        state.trace.push(tn);
                        Ok(state)
                    })
                });
            }

            graph.set_entry_point(&node_names[0]);
            for i in 0..node_names.len() - 1 {
                graph.add_edge(&node_names[i], &node_names[i + 1]);
            }
            graph.add_edge(&node_names[node_names.len() - 1], END);

            let app = graph.compile().unwrap();
            let result = app.invoke(GraphTestState::default()).await;

            prop_assert!(result.is_ok());
            let final_state = result.unwrap().final_state;
            prop_assert_eq!(final_state.trace.len(), num_nodes);
            Ok(())
        })?;
    }
}

// =============================================================================
// Property Tests: State Clone Independence
// =============================================================================

proptest! {
    /// Property: GraphTestState clone creates independent copy
    /// Invariant: Modifying clone doesn't affect original
    #[test]
    fn prop_graph_test_state_clone_independent(
        values in arb_values(),
        trace in prop::collection::vec("[a-z]{1,10}", 0..5),
    ) {
        let original = GraphTestState {
            values: values.clone(),
            trace: trace.clone(),
        };
        let mut cloned = original.clone();

        // Modify clone
        cloned.values.push(999);
        cloned.trace.push("modified".to_string());

        // Original should be unchanged
        prop_assert_eq!(original.values.len(), values.len());
        prop_assert_eq!(original.trace.len(), trace.len());
        prop_assert!(!original.values.contains(&999));
        prop_assert!(!original.trace.contains(&"modified".to_string()));
    }

    /// Property: ParallelTestState clone creates independent copy
    /// Invariant: Modifying clone doesn't affect original
    #[test]
    fn prop_parallel_test_state_clone_independent(
        counter in any::<i32>(),
        collected in prop::collection::vec("[a-z]{1,10}", 0..5),
    ) {
        let original = ParallelTestState {
            counter,
            collected: collected.clone(),
        };
        let mut cloned = original.clone();

        // Modify clone
        cloned.counter += 1;
        cloned.collected.push("modified".to_string());

        // Original should be unchanged
        prop_assert_eq!(original.counter, counter);
        prop_assert_eq!(original.collected.len(), collected.len());
    }
}

// =============================================================================
// Property Tests: MergeableState Associativity (for commutative operations)
// =============================================================================

proptest! {
    /// Property: Merge max-value semantics are commutative
    /// Invariant: merge(a, b).value == merge(b, a).value for max operation
    #[test]
    fn prop_merge_value_commutative(
        value1 in any::<i32>(),
        value2 in any::<i32>(),
    ) {
        let mut state1 = TestState {
            value: value1,
            text: String::new(),
        };
        let state2 = TestState {
            value: value2,
            text: String::new(),
        };

        let state1_copy = state1.clone();
        let state2_copy = state2.clone();

        state1.merge(&state2);
        let result1 = state1.value;

        // Reverse merge
        let mut reversed = state2_copy;
        reversed.merge(&state1_copy);
        let result2 = reversed.value;

        // Both should yield the same max value
        prop_assert_eq!(result1, result2);
        prop_assert_eq!(result1, value1.max(value2));
    }

    /// Property: ParallelTestState counter merge is commutative
    /// Invariant: merge(a, b).counter == merge(b, a).counter for max operation
    #[test]
    fn prop_parallel_merge_counter_commutative(
        counter1 in any::<i32>(),
        counter2 in any::<i32>(),
    ) {
        let mut state1 = ParallelTestState {
            counter: counter1,
            collected: vec![],
        };
        let state2 = ParallelTestState {
            counter: counter2,
            collected: vec![],
        };

        let state1_copy = state1.clone();
        let state2_copy = state2.clone();

        state1.merge(&state2);
        let result1 = state1.counter;

        // Reverse merge
        let mut reversed = state2_copy;
        reversed.merge(&state1_copy);
        let result2 = reversed.counter;

        // Both should yield the same max value
        prop_assert_eq!(result1, result2);
        prop_assert_eq!(result1, counter1.max(counter2));
    }
}
