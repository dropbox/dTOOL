//! Edge Case Tests for DashFlow
//!
//! These tests verify boundary conditions and edge cases that may cause issues
//! in production. Categories:
//!
//! 1. **Empty Collections**: Empty state fields, empty graphs, empty messages
//! 2. **Zero/Negative Values**: Zero recursion limits, zero timeouts, negative values
//! 3. **Large Inputs**: Large state objects, many messages, deep nesting
//! 4. **Boundary Conditions**: Max integers, empty strings, unicode edge cases
//! 5. **Clock/Time Issues**: Timestamp edge cases, duration overflow

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::checkpoint::{Checkpoint, Checkpointer, MemoryCheckpointer};
use dashflow::core::messages::Message;
use dashflow::reducer::{add_messages, MessageExt};
use dashflow::state::{AgentState, JsonState, MergeableState};
use dashflow::{Error, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

// =============================================================================
// Test State Definitions
// =============================================================================

/// State for edge case testing
#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq)]
struct EdgeCaseState {
    values: Vec<i64>,
    strings: Vec<String>,
    counter: u64,
    optional: Option<String>,
    nested: HashMap<String, Vec<i32>>,
}

impl MergeableState for EdgeCaseState {
    fn merge(&mut self, other: &Self) {
        self.values.extend(other.values.clone());
        self.strings.extend(other.strings.clone());
        self.counter = self.counter.saturating_add(other.counter);
        if self.optional.is_none() {
            self.optional = other.optional.clone();
        }
        for (key, vals) in &other.nested {
            self.nested
                .entry(key.clone())
                .or_default()
                .extend(vals.clone());
        }
    }
}

// =============================================================================
// Empty Collection Tests
// =============================================================================

mod empty_collections {
    use super::*;

    #[tokio::test]
    async fn test_empty_state_execution() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("passthrough", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("passthrough");
        graph.add_edge("passthrough", END);

        let app = graph.compile().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();

        assert_eq!(result.final_state, EdgeCaseState::default());
    }

    #[tokio::test]
    async fn test_empty_messages_reducer() {
        // Both empty
        let result = add_messages(vec![], vec![]);
        assert!(result.is_empty());

        // Left empty
        let result = add_messages(vec![], vec![Message::human("test")]);
        assert_eq!(result.len(), 1);

        // Right empty
        let result = add_messages(vec![Message::human("test")], vec![]);
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_empty_string_message() {
        let msg = Message::human("");
        assert_eq!(msg.as_text(), "");

        let merged = add_messages(vec![msg], vec![]);
        assert_eq!(merged[0].as_text(), "");
    }

    #[tokio::test]
    async fn test_empty_json_state() {
        let state = JsonState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);

        // Operations on empty state
        assert!(state.get("missing").is_none());
        assert!(state.get_str("missing").is_none());
        assert!(state.get_i64("missing").is_none());
        assert!(state.get_bool("missing").is_none());
        assert!(state.get_array("missing").is_none());
        assert!(state.get_object("missing").is_none());
        assert!(!state.contains("anything"));

        // Iteration over empty state
        let count: usize = state.iter().count();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_merge_empty_json_states() {
        let mut state1 = JsonState::new();
        let state2 = JsonState::new();
        state1.merge(&state2);
        assert!(state1.is_empty());

        // Merge non-empty into empty
        let mut state1 = JsonState::new();
        let state2 = JsonState::from(serde_json::json!({"key": "value"}));
        state1.merge(&state2);
        assert_eq!(state1.get_str("key"), Some("value"));
    }

    #[tokio::test]
    async fn test_empty_agent_state() {
        let state = AgentState::new();
        assert!(state.messages.is_empty());
        assert_eq!(state.iteration, 0);
        assert_eq!(state.next, None);
    }

    #[tokio::test]
    async fn test_merge_empty_agent_states() {
        let mut state1 = AgentState::new();
        let state2 = AgentState::new();
        state1.merge(&state2);

        assert!(state1.messages.is_empty());
        assert_eq!(state1.iteration, 0);
    }

    #[tokio::test]
    async fn test_checkpoint_empty_state() {
        let checkpointer = MemoryCheckpointer::new();
        let checkpoint = Checkpoint::new(
            "thread_empty".to_string(),
            EdgeCaseState::default(),
            "node".to_string(),
            None,
        );
        let id = checkpoint.id.clone();

        checkpointer.save(checkpoint.clone()).await.unwrap();
        let loaded = checkpointer.load(&id).await.unwrap();

        assert_eq!(loaded.map(|c| c.state), Some(EdgeCaseState::default()));
    }

    #[tokio::test]
    async fn test_list_empty_thread() {
        let checkpointer = MemoryCheckpointer::<EdgeCaseState>::new();
        let list = checkpointer.list("nonexistent_thread").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_get_latest_empty_thread() {
        let checkpointer = MemoryCheckpointer::<EdgeCaseState>::new();
        let latest = checkpointer.get_latest("nonexistent_thread").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_checkpoint() {
        let checkpointer = MemoryCheckpointer::<EdgeCaseState>::new();
        // Delete should not error on nonexistent checkpoint
        let result = checkpointer.delete("nonexistent_id").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_nonexistent_checkpoint() {
        let checkpointer = MemoryCheckpointer::<EdgeCaseState>::new();
        let loaded = checkpointer.load("nonexistent_id").await.unwrap();
        assert!(loaded.is_none());
    }
}

// =============================================================================
// Zero and Negative Value Tests
// =============================================================================

mod zero_negative_values {
    use super::*;

    #[tokio::test]
    async fn test_zero_iteration_counter() {
        let mut state = AgentState::new();
        assert_eq!(state.iteration, 0);

        // Should work even with 0
        let state2 = AgentState::new();
        state.merge(&state2);
        assert_eq!(state.iteration, 0); // max(0, 0) = 0
    }

    #[tokio::test]
    async fn test_max_iteration_counter() {
        let mut state1 = AgentState::new();
        state1.iteration = u32::MAX;

        let mut state2 = AgentState::new();
        state2.iteration = u32::MAX;

        state1.merge(&state2);
        assert_eq!(state1.iteration, u32::MAX); // max should not overflow
    }

    #[tokio::test]
    async fn test_negative_values_in_json() {
        let state = JsonState::from(serde_json::json!({
            "negative": -42,
            "zero": 0,
            "min_i64": i64::MIN,
        }));

        assert_eq!(state.get_i64("negative"), Some(-42));
        assert_eq!(state.get_i64("zero"), Some(0));
        assert_eq!(state.get_i64("min_i64"), Some(i64::MIN));
    }

    #[tokio::test]
    async fn test_counter_saturating_add() {
        let mut state1 = EdgeCaseState {
            counter: u64::MAX,
            ..Default::default()
        };
        let state2 = EdgeCaseState {
            counter: 1,
            ..Default::default()
        };

        state1.merge(&state2);
        // Should saturate instead of overflow
        assert_eq!(state1.counter, u64::MAX);
    }

    #[tokio::test]
    async fn test_recursion_limit_boundary() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("loop", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                Ok(state)
            })
        });

        graph.set_entry_point("loop");

        // Create a self-loop using conditional edges
        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "loop".to_string());
        routes.insert(END.to_string(), END.to_string());

        graph.add_conditional_edges(
            "loop",
            |state: &EdgeCaseState| {
                if state.counter < 100 {
                    "loop".to_string()
                } else {
                    END.to_string()
                }
            },
            routes,
        );

        // Default recursion limit is 25, so this should hit the limit
        let app = graph.compile().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await;

        // Should hit recursion limit before counter reaches 100
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, Error::RecursionLimit { .. }),
            "Expected RecursionLimit error, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_very_small_timeout() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("slow", |state| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(state)
            })
        });

        graph.set_entry_point("slow");
        graph.add_edge("slow", END);

        // 1 nanosecond timeout - essentially instant
        let app = graph
            .compile()
            .unwrap()
            .with_node_timeout(Duration::from_nanos(1));

        let result = app.invoke(EdgeCaseState::default()).await;

        // Should timeout
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Check for any timeout-related error
        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("timeout") || err_str.contains("timed out"),
            "Expected timeout error, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_zero_duration_timeout() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("instant", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("instant");
        graph.add_edge("instant", END);

        // Zero timeout (no timeout)
        let app = graph.compile().unwrap().with_timeout(Duration::ZERO);

        // Zero timeout means no timeout should be applied
        let result = app.invoke(EdgeCaseState::default()).await;
        assert!(result.is_ok());
    }
}

// =============================================================================
// Large Input Tests
// =============================================================================

mod large_inputs {
    use super::*;

    #[tokio::test]
    async fn test_large_message_list() {
        // 1000 messages
        let mut messages: Vec<Message> = Vec::with_capacity(1000);
        for i in 0..1000 {
            messages.push(Message::human(format!("Message {}", i)).with_id(format!("id_{}", i)));
        }

        // Should handle large message lists
        let left = messages[..500].to_vec();
        let right = messages[500..].to_vec();
        let merged = add_messages(left, right);
        assert_eq!(merged.len(), 1000);
    }

    #[tokio::test]
    async fn test_large_state_values() {
        // State with many values
        let mut state = EdgeCaseState::default();
        for i in 0..10000 {
            state.values.push(i);
        }

        // Should serialize/deserialize correctly
        let json = serde_json::to_string(&state).unwrap();
        let restored: EdgeCaseState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.values.len(), restored.values.len());
    }

    #[tokio::test]
    async fn test_deeply_nested_json() {
        // Create deeply nested JSON
        let mut value = serde_json::json!("leaf");
        for _ in 0..50 {
            value = serde_json::json!({ "nested": value });
        }

        let state = JsonState::from(value);

        // Should be able to access deeply nested value
        let mut current = state.as_value();
        for _ in 0..50 {
            current = current.get("nested").unwrap();
        }
        assert_eq!(current.as_str(), Some("leaf"));
    }

    #[tokio::test]
    async fn test_wide_parallel_execution() {
        let execution_count = Arc::new(AtomicU32::new(0));

        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        // Add 20 parallel nodes
        let node_names: Vec<String> = (0..20).map(|i| format!("node_{}", i)).collect();

        for name in &node_names {
            let counter = Arc::clone(&execution_count);
            let node_name = name.clone();
            graph.add_node_from_fn(&node_name, move |mut state| {
                let c = Arc::clone(&counter);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    state.counter += 1;
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("end", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");

        // Create parallel edges - need Vec<String>
        let node_strings: Vec<String> = node_names.clone();
        graph.add_parallel_edges("start", node_strings);

        // All parallel nodes converge to end
        for name in &node_names {
            graph.add_edge(name, "end");
        }
        graph.add_edge("end", END);

        let app = graph.compile_with_merge().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();

        // All 20 nodes should have executed
        assert_eq!(execution_count.load(Ordering::SeqCst), 20);
        assert_eq!(result.final_state.counter, 20);
    }

    #[tokio::test]
    async fn test_many_checkpoint_saves() {
        let checkpointer = MemoryCheckpointer::new();
        let thread_id = "stress_test_thread".to_string();

        // Save 100 checkpoints
        for i in 0..100u64 {
            let state = EdgeCaseState {
                counter: i,
                ..Default::default()
            };
            let checkpoint = Checkpoint::new(thread_id.clone(), state, format!("node_{}", i), None);
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List should return all 100
        let list = checkpointer.list(&thread_id).await.unwrap();
        assert_eq!(list.len(), 100);

        // Get latest should return the last one (highest counter)
        let latest = checkpointer.get_latest(&thread_id).await.unwrap().unwrap();
        assert_eq!(latest.state.counter, 99);
    }

    #[tokio::test]
    async fn test_long_string_values() {
        // 1MB string
        let long_string: String = "x".repeat(1_000_000);

        let mut state = EdgeCaseState::default();
        state.strings.push(long_string.clone());

        // Should serialize/deserialize correctly
        let json = serde_json::to_string(&state).unwrap();
        let restored: EdgeCaseState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.strings[0].len(), 1_000_000);

        // Should checkpoint correctly
        let checkpointer = MemoryCheckpointer::new();
        let checkpoint = Checkpoint::new("thread".to_string(), state, "node".to_string(), None);
        let id = checkpoint.id.clone();

        checkpointer.save(checkpoint).await.unwrap();
        let loaded = checkpointer.load(&id).await.unwrap().unwrap();
        assert_eq!(loaded.state.strings[0].len(), 1_000_000);
    }
}

// =============================================================================
// Boundary Condition Tests
// =============================================================================

mod boundary_conditions {
    use super::*;

    #[tokio::test]
    async fn test_unicode_edge_cases() {
        // Various unicode edge cases
        let unicode_strings = vec![
            "",           // empty
            "\u{0000}",   // null character
            "🦀🔥💾",     // emoji
            "مرحبا",      // Arabic RTL
            "こんにちは", // Japanese
            "Ω≈ç√∫",      // mathematical symbols
            "\u{200B}",   // zero-width space
            "a\u{0300}",  // combining character
            "\u{FFFF}",   // max BMP character
            "𐀀",          // supplementary plane character
        ];

        for s in &unicode_strings {
            let msg = Message::human(*s);
            assert_eq!(msg.as_text(), *s);

            // Test in JsonState
            let mut state = JsonState::new();
            state.set("unicode", serde_json::json!(s));
            assert_eq!(state.get_str("unicode"), Some(*s));
        }
    }

    #[tokio::test]
    async fn test_special_json_values() {
        let state = JsonState::from(serde_json::json!({
            "null": null,
            "bool_true": true,
            "bool_false": false,
            "int_zero": 0,
            "float_zero": 0.0,
            "empty_string": "",
            "empty_array": [],
            "empty_object": {},
        }));

        assert!(state.get("null").unwrap().is_null());
        assert_eq!(state.get_bool("bool_true"), Some(true));
        assert_eq!(state.get_bool("bool_false"), Some(false));
        assert_eq!(state.get_i64("int_zero"), Some(0));
        assert_eq!(state.get_f64("float_zero"), Some(0.0));
        assert_eq!(state.get_str("empty_string"), Some(""));
        assert!(state.get_array("empty_array").unwrap().is_empty());
        assert!(state.get_object("empty_object").unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_float_edge_cases() {
        // Note: JSON doesn't support infinity values - they serialize as null
        // So we test valid float edge cases instead
        let state = JsonState::from(serde_json::json!({
            "max": f64::MAX,
            "min_positive": f64::MIN_POSITIVE,
            "epsilon": f64::EPSILON,
            "zero": 0.0,
            "negative_zero": -0.0,
            "small": 1e-300,
            "large": 1e300,
        }));

        assert_eq!(state.get_f64("max"), Some(f64::MAX));
        assert_eq!(state.get_f64("min_positive"), Some(f64::MIN_POSITIVE));
        assert_eq!(state.get_f64("epsilon"), Some(f64::EPSILON));
        assert_eq!(state.get_f64("zero"), Some(0.0));
        // Note: -0.0 == 0.0 in IEEE 754, both are valid
        assert_eq!(state.get_f64("negative_zero"), Some(0.0));
        assert_eq!(state.get_f64("small"), Some(1e-300));
        assert_eq!(state.get_f64("large"), Some(1e300));
    }

    #[tokio::test]
    async fn test_integer_boundary_values() {
        // Test JSON can handle various integer ranges
        let state = JsonState::from(serde_json::json!({
            "i64_max": i64::MAX,
            "i64_min": i64::MIN,
            "i32_max": i32::MAX as i64,
            "i32_min": i32::MIN as i64,
        }));

        assert_eq!(state.get_i64("i64_max"), Some(i64::MAX));
        assert_eq!(state.get_i64("i64_min"), Some(i64::MIN));
        assert_eq!(state.get_i64("i32_max"), Some(i32::MAX as i64));
        assert_eq!(state.get_i64("i32_min"), Some(i32::MIN as i64));
    }

    #[tokio::test]
    async fn test_message_with_same_id_update() {
        // Test that messages with same ID update rather than duplicate
        let msg1 = Message::human("original").with_id("same_id");
        let msg2 = Message::human("updated").with_id("same_id");

        let merged = add_messages(vec![msg1], vec![msg2]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].as_text(), "updated");
    }

    #[tokio::test]
    async fn test_checkpoint_id_uniqueness() {
        // Create many checkpoints quickly and verify unique IDs
        let checkpointer = MemoryCheckpointer::new();
        let mut ids = std::collections::HashSet::new();

        for _ in 0..100 {
            let checkpoint = Checkpoint::new(
                "thread".to_string(),
                EdgeCaseState::default(),
                "node".to_string(),
                None,
            );
            let id = checkpoint.id.clone();
            assert!(ids.insert(id.clone()), "Duplicate checkpoint ID generated");
            checkpointer.save(checkpoint).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_thread_id_special_characters() {
        let checkpointer = MemoryCheckpointer::new();

        // Thread IDs with special characters
        let special_thread_ids = vec![
            "thread-with-dashes",
            "thread_with_underscores",
            "thread.with.dots",
            "thread/with/slashes",
            "thread:with:colons",
            "thread@with@at",
            "thread#with#hash",
            "thread space",
            "thread\twith\ttabs",
            "thread🦀emoji",
        ];

        for thread_id in special_thread_ids {
            let checkpoint = Checkpoint::new(
                thread_id.to_string(),
                EdgeCaseState::default(),
                "node".to_string(),
                None,
            );
            let id = checkpoint.id.clone();

            checkpointer.save(checkpoint).await.unwrap();
            let loaded = checkpointer.load(&id).await.unwrap();
            assert!(loaded.is_some(), "Failed for thread_id: {}", thread_id);
            assert_eq!(loaded.unwrap().thread_id, thread_id);
        }
    }
}

// =============================================================================
// Time/Clock Edge Cases
// =============================================================================

mod time_edge_cases {
    use super::*;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_checkpoint_timestamp_ordering() {
        let checkpointer = MemoryCheckpointer::new();
        let thread_id = "time_test".to_string();

        // Save checkpoints with small delays
        for i in 0..5u64 {
            let state = EdgeCaseState {
                counter: i,
                ..Default::default()
            };
            let checkpoint = Checkpoint::new(thread_id.clone(), state, format!("node_{}", i), None);
            checkpointer.save(checkpoint).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // List should be in descending timestamp order
        let list = checkpointer.list(&thread_id).await.unwrap();
        for i in 1..list.len() {
            assert!(
                list[i - 1].timestamp >= list[i].timestamp,
                "Checkpoints not in descending order"
            );
        }
    }

    #[tokio::test]
    async fn test_instant_node_execution() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("instant", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("instant");
        graph.add_edge("instant", END);

        let app = graph.compile().unwrap();

        // Execute many times quickly
        for _ in 0..100 {
            let result = app.invoke(EdgeCaseState::default()).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_checkpoint_with_system_time() {
        // Verify checkpoint timestamps are close to current time
        let before = SystemTime::now();

        let checkpoint = Checkpoint::new(
            "thread".to_string(),
            EdgeCaseState::default(),
            "node".to_string(),
            None,
        );

        let after = SystemTime::now();

        // Checkpoint timestamp should be between before and after
        assert!(checkpoint.timestamp >= before);
        assert!(checkpoint.timestamp <= after);
    }
}

// =============================================================================
// Graph Structure Edge Cases
// =============================================================================

mod graph_structure {
    use super::*;

    #[tokio::test]
    async fn test_single_node_graph() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("only_node", |mut state| {
            Box::pin(async move {
                state.counter = 1;
                Ok(state)
            })
        });

        graph.set_entry_point("only_node");
        graph.add_edge("only_node", END);

        let app = graph.compile().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();
        assert_eq!(result.final_state.counter, 1);
    }

    #[tokio::test]
    async fn test_linear_chain_execution() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        // Create a 10-node linear chain
        for i in 0..10 {
            let name = format!("node_{}", i);
            let i_val = i as u64;
            graph.add_node_from_fn(&name, move |mut state: EdgeCaseState| {
                Box::pin(async move {
                    state.values.push(i_val as i64);
                    Ok(state)
                })
            });
        }

        graph.set_entry_point("node_0");
        for i in 0..9 {
            graph.add_edge(format!("node_{}", i), format!("node_{}", i + 1));
        }
        graph.add_edge("node_9", END);

        let app = graph.compile().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();

        assert_eq!(result.final_state.values.len(), 10);
        for i in 0..10 {
            assert_eq!(result.final_state.values[i], i as i64);
        }
    }

    #[tokio::test]
    async fn test_diamond_graph_structure() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("left", |mut state| {
            Box::pin(async move {
                state.strings.push("left".to_string());
                Ok(state)
            })
        });
        graph.add_node_from_fn("right", |mut state| {
            Box::pin(async move {
                state.strings.push("right".to_string());
                Ok(state)
            })
        });
        graph.add_node_from_fn("merge", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");
        graph.add_parallel_edges("start", vec!["left".to_string(), "right".to_string()]);
        graph.add_edge("left", "merge");
        graph.add_edge("right", "merge");
        graph.add_edge("merge", END);

        let app = graph.compile_with_merge().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();

        // Both branches should have executed
        assert_eq!(result.final_state.strings.len(), 2);
        assert!(result.final_state.strings.contains(&"left".to_string()));
        assert!(result.final_state.strings.contains(&"right".to_string()));
    }

    #[tokio::test]
    async fn test_conditional_self_loop_to_end() {
        let execution_count = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&execution_count);

        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("loop", move |mut state| {
            let counter = Arc::clone(&counter_clone);
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                state.counter += 1;
                Ok(state)
            })
        });

        graph.set_entry_point("loop");

        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "loop".to_string());
        routes.insert(END.to_string(), END.to_string());

        graph.add_conditional_edges(
            "loop",
            |state: &EdgeCaseState| {
                if state.counter >= 3 {
                    END.to_string()
                } else {
                    "loop".to_string()
                }
            },
            routes,
        );

        let app = graph.compile().unwrap().with_recursion_limit(10);
        let result = app.invoke(EdgeCaseState::default()).await.unwrap();

        // Should have looped exactly 3 times
        assert_eq!(result.final_state.counter, 3);
        assert_eq!(execution_count.load(Ordering::SeqCst), 3);
    }
}

// =============================================================================
// Error Edge Cases
// =============================================================================

mod error_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_error_from_node_is_propagated() {
        // Note: panics in async code aren't caught by the executor as errors
        // Instead, we test that errors returned from nodes propagate correctly
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("error_node", |_state| {
            Box::pin(
                async move { Err(Error::Generic("Intentional error for testing".to_string())) },
            )
        });

        graph.set_entry_point("error_node");
        graph.add_edge("error_node", END);

        let app = graph.compile().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await;

        // Error should propagate
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Intentional error for testing"));
    }

    #[tokio::test]
    async fn test_error_propagation_in_parallel() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("ok_node", |mut state| {
            Box::pin(async move {
                state.strings.push("ok".to_string());
                Ok(state)
            })
        });
        graph.add_node_from_fn("failing_node", |_state| {
            Box::pin(async move { Err(Error::Generic("Intentional failure".to_string())) })
        });

        graph.set_entry_point("start");
        graph.add_parallel_edges(
            "start",
            vec!["ok_node".to_string(), "failing_node".to_string()],
        );
        graph.add_edge("ok_node", END);
        graph.add_edge("failing_node", END);

        let app = graph.compile_with_merge().unwrap();
        let result = app.invoke(EdgeCaseState::default()).await;

        // Error in parallel branch should propagate
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_graph_timeout_during_parallel() {
        let mut graph: StateGraph<EdgeCaseState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("slow", |state| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(state)
            })
        });
        graph.add_node_from_fn("fast", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");
        graph.add_parallel_edges("start", vec!["slow".to_string(), "fast".to_string()]);
        graph.add_edge("slow", END);
        graph.add_edge("fast", END);

        let app = graph
            .compile_with_merge()
            .unwrap()
            .with_timeout(Duration::from_millis(100));

        let result = app.invoke(EdgeCaseState::default()).await;

        // Should timeout
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("timeout") || err_str.contains("timed out"),
            "Expected timeout error, got: {:?}",
            err
        );
    }
}
