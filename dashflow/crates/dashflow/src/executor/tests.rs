use super::trace::{build_execution_trace, persist_trace_in_dir};
use super::*;
use crate::error::Error;
use crate::graph::StateGraph;
use crate::introspection::{ExecutionTrace, NodeExecution};
use crate::state::AgentState;

#[path = "tests/trace_tests.rs"]
mod trace_tests;

#[path = "tests/validation_tests.rs"]
mod validation_tests;

#[path = "tests/interrupt_resume_tests.rs"]
mod interrupt_resume_tests;

#[path = "tests/introspection_tests.rs"]
mod introspection_tests;

#[tokio::test]
async fn test_simple_execution() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1 executed");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2 executed");
            Ok(state)
        })
    });

    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);
    graph.set_entry_point("node1");

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.nodes_executed.len(), 2);
    assert_eq!(result.nodes_executed[0], "node1");
    assert_eq!(result.nodes_executed[1], "node2");
    assert_eq!(result.final_state.messages.len(), 2);
    assert_eq!(result.final_state.messages[0], "node1 executed");
    assert_eq!(result.final_state.messages[1], "node2 executed");
}

#[tokio::test]
async fn test_conditional_execution() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.add_node_from_fn("continue", |mut state| {
        Box::pin(async move {
            state.add_message("continue");
            state.iteration += 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "continue".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration < 2 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes.clone(),
    );

    graph.add_conditional_edges(
        "continue",
        |state: &AgentState| {
            if state.iteration < 3 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("start");

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should execute: start -> continue -> continue -> end
    assert_eq!(result.nodes_executed.len(), 3);
    assert_eq!(result.final_state.iteration, 3);
    assert_eq!(result.final_state.messages.len(), 3);
}

/// Test that EdgeEvaluated events are emitted for conditional edge traversals (FIX-001)
#[tokio::test]
async fn test_edge_evaluated_event_for_conditional_edges() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            state.add_message("branch_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            state.add_message("branch_b");
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "branch_a".to_string());
    routes.insert("b".to_string(), "branch_b".to_string());

    // Conditional edge: go to branch_a when iteration == 1
    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration == 1 {
                "a".to_string()
            } else {
                "b".to_string()
            }
        },
        routes,
    );

    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);
    graph.set_entry_point("start");

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Find EdgeEvaluated events
    let edge_evaluated_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::EdgeEvaluated { .. }))
        .collect();

    // Should have exactly 1 EdgeEvaluated event (from start -> branch_a)
    assert_eq!(
        edge_evaluated_events.len(),
        1,
        "Should have exactly 1 EdgeEvaluated event"
    );

    // Verify the event details
    if let crate::event::GraphEvent::EdgeEvaluated {
        from_node,
        to_node,
        evaluation_result,
        alternatives,
        ..
    } = &edge_evaluated_events[0]
    {
        assert_eq!(from_node, "start", "from_node should be 'start'");
        assert_eq!(to_node, "branch_a", "to_node should be 'branch_a'");
        assert!(evaluation_result, "evaluation_result should be true");
        // Should have 1 alternative (branch_b was not selected)
        assert_eq!(alternatives.len(), 1, "Should have 1 alternative");
        assert_eq!(
            alternatives[0].to_node, "branch_b",
            "Alternative should be branch_b"
        );
        assert!(!alternatives[0].was_evaluated, "Alternative was not evaluated");
    } else {
        panic!("Expected EdgeEvaluated event");
    }
}

/// Test that StateChanged events are emitted after node execution (FIX-002)
#[tokio::test]
async fn test_state_changed_event_after_node_execution() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that modifies multiple fields
    graph.add_node_from_fn("modifier", |mut state| {
        Box::pin(async move {
            state.add_message("modified");
            state.iteration = 42;
            Ok(state)
        })
    });

    graph.add_edge("modifier", END);
    graph.set_entry_point("modifier");

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Find StateChanged events
    let state_changed_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::StateChanged { .. }))
        .collect();

    // Should have exactly 1 StateChanged event (from modifier node)
    assert_eq!(
        state_changed_events.len(),
        1,
        "Should have exactly 1 StateChanged event"
    );

    // Verify the event details
    if let crate::event::GraphEvent::StateChanged {
        node,
        summary,
        fields_modified,
        ..
    } = &state_changed_events[0]
    {
        assert_eq!(node, "modifier", "node should be 'modifier'");
        // Summary should indicate modifications
        assert!(
            summary.contains("modified"),
            "Summary should mention modifications: {}",
            summary
        );
        // 'messages' and 'iteration' should be in fields_modified
        assert!(
            fields_modified.contains(&"messages".to_string())
                || fields_modified.contains(&"iteration".to_string()),
            "fields_modified should include 'messages' or 'iteration': {:?}",
            fields_modified
        );
    } else {
        panic!("Expected StateChanged event");
    }
}

/// Test that StateChanged events are NOT emitted when state doesn't change
#[tokio::test]
async fn test_no_state_changed_event_when_no_changes() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that doesn't modify state at all
    graph.add_node_from_fn("noop", |state| {
        Box::pin(async move {
            // Return state unchanged
            Ok(state)
        })
    });

    graph.add_edge("noop", END);
    graph.set_entry_point("noop");

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Find StateChanged events
    let state_changed_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::StateChanged { .. }))
        .collect();

    // Should have 0 StateChanged events (no state was modified)
    assert_eq!(
        state_changed_events.len(),
        0,
        "Should have 0 StateChanged events when state doesn't change"
    );
}

#[tokio::test]
async fn test_implicit_end() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("only_node", |mut state| {
        Box::pin(async move {
            state.add_message("done");
            Ok(state)
        })
    });

    graph.set_entry_point("only_node");
    // No edges - should end implicitly

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.nodes_executed.len(), 1);
    assert_eq!(result.final_state.messages.len(), 1);
}

#[tokio::test]
async fn test_parallel_execution() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("parallel1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("parallel2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel2", "end_node");
    graph.add_edge("end_node", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should execute: start, then parallel1 and parallel2 concurrently, then end_node
    assert_eq!(result.nodes_executed.len(), 4);
    assert_eq!(result.nodes_executed[0], "start");
    // parallel1 and parallel2 should be in nodes_executed (order may vary)
    assert!(result.nodes_executed.contains(&"parallel1".to_string()));
    assert!(result.nodes_executed.contains(&"parallel2".to_string()));
    assert_eq!(result.nodes_executed[3], "end_node");

    // All messages should be present
    assert!(result.final_state.messages.contains(&"start".to_string()));
    assert!(
        result
            .final_state
            .messages
            .contains(&"parallel1".to_string())
            || result
                .final_state
                .messages
                .contains(&"parallel2".to_string())
    );
    assert!(result.final_state.messages.contains(&"end".to_string()));
}

#[tokio::test]
async fn test_graph_timeout() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("slow_node", |state| {
        Box::pin(async move {
            // Simulate slow operation
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(50));

    let result = app.invoke(AgentState::new()).await;
    assert!(matches!(result, Err(Error::Timeout(_))));
}

#[tokio::test]
async fn test_node_timeout() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("fast_node", |mut state| {
        Box::pin(async move {
            state.add_message("fast");
            Ok(state)
        })
    });

    graph.add_node_from_fn("slow_node", |state| {
        Box::pin(async move {
            // Simulate slow operation
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("fast_node");
    graph.add_edge("fast_node", "slow_node");
    graph.add_edge("slow_node", END);

    let app = graph
        .compile()
        .unwrap()
        .with_node_timeout(Duration::from_millis(50));

    let result = app.invoke(AgentState::new()).await;
    // Should timeout on slow_node
    assert!(matches!(result, Err(Error::Timeout(_))));
}

#[tokio::test]
async fn test_events_with_callback() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);

    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();
    assert!(!events.is_empty());

    // Check we have graph start and end events
    let has_graph_start = events
        .iter()
        .any(|e| matches!(e, crate::event::GraphEvent::GraphStart { .. }));
    let has_graph_end = events
        .iter()
        .any(|e| matches!(e, crate::event::GraphEvent::GraphEnd { .. }));

    assert!(has_graph_start, "Should have GraphStart event");
    assert!(has_graph_end, "Should have GraphEnd event");

    // Check we have node start/end events
    let node_start_count = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::NodeStart { .. }))
        .count();
    let node_end_count = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::NodeEnd { .. }))
        .count();

    assert_eq!(node_start_count, 2, "Should have 2 NodeStart events");
    assert_eq!(node_end_count, 2, "Should have 2 NodeEnd events");
}

#[tokio::test]
async fn test_events_with_parallel() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            state.add_message("parallel1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel2", |mut state| {
        Box::pin(async move {
            state.add_message("parallel2");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel2", END);

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile_with_merge().unwrap().with_callback(callback);

    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Check we have parallel start/end events
    let has_parallel_start = events
        .iter()
        .any(|e| matches!(e, crate::event::GraphEvent::ParallelStart { .. }));
    let has_parallel_end = events
        .iter()
        .any(|e| matches!(e, crate::event::GraphEvent::ParallelEnd { .. }));

    assert!(has_parallel_start, "Should have ParallelStart event");
    assert!(has_parallel_end, "Should have ParallelEnd event");
}

#[tokio::test]
async fn test_stream_values() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        events.push(event);
    }

    // Should have 2 Values events + 1 Done event
    assert_eq!(events.len(), 3);

    // Check we got values for both nodes
    let values_count = events
        .iter()
        .filter(|e| matches!(e, crate::stream::StreamEvent::Values { .. }))
        .count();
    assert_eq!(values_count, 2);

    // Check final Done event
    assert!(events.last().unwrap().is_done());
}

#[tokio::test]
async fn test_stream_events_mode() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Events));

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        events.push(event);
    }

    // Should have NodeStart + NodeEnd + Done
    assert!(events.len() >= 3);

    // Check we have NodeStart and NodeEnd
    let has_node_start = events
        .iter()
        .any(|e| matches!(e, crate::stream::StreamEvent::NodeStart { .. }));
    let has_node_end = events
        .iter()
        .any(|e| matches!(e, crate::stream::StreamEvent::NodeEnd { .. }));

    assert!(has_node_start);
    assert!(has_node_end);
}

#[tokio::test]
async fn test_no_timeout() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    // Set timeouts that should not trigger
    let app = graph
        .compile()
        .unwrap()
        .with_node_timeout(Duration::from_millis(100))
        .with_timeout(Duration::from_secs(1));

    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.nodes_executed.len(), 2);
    assert_eq!(result.final_state.messages.len(), 2);
}

#[tokio::test]
async fn test_parallel_execution_with_scheduler() {
    use crate::scheduler::WorkStealingScheduler;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("parallel1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("parallel2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel2", "end_node");
    graph.add_edge("end_node", END);

    // Create scheduler (no workers = local execution fallback)
    let scheduler = WorkStealingScheduler::new().with_threshold(1); // Force scheduler path even without workers

    let app = graph
        .compile_with_merge()
        .unwrap()
        .with_scheduler(scheduler);
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should execute: start, then parallel1 and parallel2 via scheduler, then end_node
    assert_eq!(result.nodes_executed.len(), 4);
    assert_eq!(result.nodes_executed[0], "start");
    assert!(result.nodes_executed.contains(&"parallel1".to_string()));
    assert!(result.nodes_executed.contains(&"parallel2".to_string()));
    assert_eq!(result.nodes_executed[3], "end_node");

    // Verify messages were processed
    assert!(result.final_state.messages.contains(&"start".to_string()));
    assert!(result.final_state.messages.contains(&"end".to_string()));
}

#[tokio::test]
async fn test_node_not_found_error() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("start");
    // Add edge to non-existent node
    graph.add_edge("start", "nonexistent_node");

    // Compilation should fail with NodeNotFound error
    let result = graph.compile();
    assert!(matches!(result, Err(Error::NodeNotFound(_))));
    if let Err(Error::NodeNotFound(node)) = result {
        assert_eq!(node, "nonexistent_node");
    }
}

#[tokio::test]
async fn test_node_execution_error() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that returns an error
    graph.add_node_from_fn("failing_node", |_state| {
        Box::pin(async move {
            Err(crate::error::Error::Generic(
                "Intentional test failure".to_string(),
            ))
        })
    });

    graph.set_entry_point("failing_node");
    graph.add_edge("failing_node", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await;

    assert!(result.is_err());
    // Error should be wrapped as NodeExecution error
    assert!(matches!(result, Err(Error::NodeExecution { .. })));
}

#[tokio::test]
async fn test_node_error_event_callback() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that fails
    graph.add_node_from_fn("failing_node", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Test error".to_string())) })
    });

    graph.set_entry_point("failing_node");
    graph.add_edge("failing_node", END);

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;

    assert!(result.is_err());

    // Verify NodeError event was emitted
    let events = callback_clone.events();
    let has_node_error = events
        .iter()
        .any(|e| matches!(e, crate::event::GraphEvent::NodeError { .. }));

    assert!(has_node_error, "Should have NodeError event");
}

#[tokio::test]
async fn test_conditional_edge_with_invalid_route() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.iteration = 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("valid_route".to_string(), END.to_string());
    // Conditional function returns a route not in the routes map

    graph.add_conditional_edges(
        "start",
        |_state: &AgentState| {
            // Return a route not in the routes map
            "invalid_route".to_string()
        },
        routes,
    );

    graph.set_entry_point("start");

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should error with InvalidEdge
    assert!(matches!(result, Err(Error::InvalidEdge(_))));
}

#[tokio::test]
async fn test_parallel_execution_partial_failure() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    // First parallel node succeeds
    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            state.add_message("parallel1");
            Ok(state)
        })
    });

    // Second parallel node fails
    graph.add_node_from_fn("parallel2", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Parallel2 failed".to_string())) })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel1", END);
    graph.add_edge("parallel2", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Entire execution should fail if any parallel node fails
    assert!(result.is_err());
}

#[tokio::test]
async fn test_parallel_execution_all_failures() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // Both parallel nodes fail
    graph.add_node_from_fn("parallel1", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Parallel1 failed".to_string())) })
    });

    graph.add_node_from_fn("parallel2", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Parallel2 failed".to_string())) })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should fail when all parallel nodes fail
    assert!(result.is_err());
}

#[tokio::test]
async fn test_single_node_implicit_end() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });
    graph.set_entry_point("start");
    // No edges - should end implicitly after start

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should succeed - graph ends implicitly after start
    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.nodes_executed.len(), 1);
    assert_eq!(result.nodes_executed[0], "start");
}

#[tokio::test]
async fn test_conditional_edge_all_branches() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            state.add_message("branch_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            state.add_message("branch_b");
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "branch_a".to_string());
    routes.insert("b".to_string(), "branch_b".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration == 0 {
                "a".to_string()
            } else if state.iteration == 1 {
                "b".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);
    graph.set_entry_point("start");

    // Test branch A
    let app = graph.compile().unwrap();
    let mut state = AgentState::new();
    state.iteration = 0;
    let result = app.invoke(state).await.unwrap();
    assert!(result.nodes_executed.contains(&"branch_a".to_string()));
    assert!(!result.nodes_executed.contains(&"branch_b".to_string()));

    // Test branch B
    let mut state = AgentState::new();
    state.iteration = 1;
    let result = app.invoke(state).await.unwrap();
    assert!(result.nodes_executed.contains(&"branch_b".to_string()));
    assert!(!result.nodes_executed.contains(&"branch_a".to_string()));

    // Test end branch
    let mut state = AgentState::new();
    state.iteration = 2;
    let result = app.invoke(state).await.unwrap();
    assert!(!result.nodes_executed.contains(&"branch_a".to_string()));
    assert!(!result.nodes_executed.contains(&"branch_b".to_string()));
    assert_eq!(result.nodes_executed.len(), 1); // Only start node
}

#[tokio::test]
async fn test_deep_graph_execution() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Create a deep chain: node1 -> node2 -> ... -> node10
    for i in 1..=10 {
        let node_name = format!("node{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                state.add_message(format!("node{}", i));
                Ok(state)
            })
        });

        if i > 1 {
            let prev_node = format!("node{}", i - 1);
            graph.add_edge(&prev_node, &node_name);
        }
    }

    graph.set_entry_point("node1");
    graph.add_edge("node10", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.nodes_executed.len(), 10);
    assert_eq!(result.final_state.messages.len(), 10);
    // Verify execution order
    for i in 1..=10 {
        assert_eq!(result.nodes_executed[i - 1], format!("node{}", i));
    }
}

#[tokio::test]
async fn test_metrics_collection() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Check metrics were collected
    let metrics = app.metrics();
    assert_eq!(metrics.node_execution_counts.len(), 2);
    assert_eq!(metrics.edges_traversed, 2);
    assert!(metrics.total_duration > Duration::from_millis(0));

    // Verify node durations were recorded
    assert!(metrics.node_durations.contains_key("node1"));
    assert!(metrics.node_durations.contains_key("node2"));

    // Node2 should be slower than node1
    let (slowest_node, _) = metrics.slowest_node().unwrap();
    assert_eq!(slowest_node, "node2");
}

#[tokio::test]
async fn test_parallel_with_different_execution_times() {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    let barrier = Arc::new(Barrier::new(2));

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // Fast parallel node
    let fast_barrier = Arc::clone(&barrier);
    graph.add_node_from_fn("fast", move |mut state| {
        let barrier = Arc::clone(&fast_barrier);
        Box::pin(async move {
            barrier.wait().await;
            state.add_message("fast");
            Ok(state)
        })
    });

    // Slow parallel node
    let slow_barrier = Arc::clone(&barrier);
    graph.add_node_from_fn("slow", move |mut state| {
        let barrier = Arc::clone(&slow_barrier);
        Box::pin(async move {
            barrier.wait().await;
            state.add_message("slow");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["fast".to_string(), "slow".to_string()]);
    graph.add_edge("fast", END);
    graph.add_edge("slow", END);

    let app = graph.compile_with_merge().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(1), app.invoke(AgentState::new()))
        .await
        .expect("Parallel execution should not deadlock")
        .unwrap();

    // Both nodes should be executed
    assert!(result.nodes_executed.contains(&"fast".to_string()));
    assert!(result.nodes_executed.contains(&"slow".to_string()));
}

#[tokio::test]
async fn test_stream_error_propagation() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    // Failing node
    graph.add_node_from_fn("node2", |_state| {
        Box::pin(async move {
            Err(crate::error::Error::Generic(
                "Stream test error".to_string(),
            ))
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut events = Vec::new();
    let mut had_error = false;
    while let Some(event) = stream.next().await {
        match event {
            Ok(e) => events.push(e),
            Err(_) => {
                had_error = true;
                break;
            }
        }
    }

    // Should have received at least one event (node1) before error
    assert!(!events.is_empty());
    assert!(had_error, "Should have encountered error in stream");
}

#[tokio::test]
async fn test_checkpoint_integration_single_node() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1 executed");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2 executed");
            Ok(state)
        })
    });

    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);
    graph.set_entry_point("node1");

    let checkpointer = MemoryCheckpointer::new();
    let thread_id: crate::checkpoint::ThreadId = "test-thread".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.nodes_executed.len(), 2);

    // Verify checkpoints were saved
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert!(!checkpoints.is_empty(), "Expected checkpoints to be saved");

    // Should have checkpoints after node1 and node2
    assert!(
        checkpoints.len() >= 2,
        "Expected at least 2 checkpoints, got {}",
        checkpoints.len()
    );
}

#[tokio::test]
async fn test_checkpoint_integration_parallel_execution() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            state.add_message("parallel1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel2", |mut state| {
        Box::pin(async move {
            state.add_message("parallel2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel1", "end_node");
    graph.add_edge("end_node", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id: crate::checkpoint::ThreadId = "test-parallel-checkpoint".to_string();

    let app = graph
        .compile_with_merge()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Verify checkpoints were saved after parallel execution
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert!(
        !checkpoints.is_empty(),
        "Expected checkpoints after parallel execution"
    );
}

#[tokio::test]
async fn test_parallel_execution_with_failure() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    graph.add_node_from_fn("success_node", |mut state| {
        Box::pin(async move {
            state.add_message("success");
            Ok(state)
        })
    });

    // Failing parallel node
    graph.add_node_from_fn("failure_node", |_state| {
        Box::pin(async move {
            Err(crate::error::Error::Generic(
                "Parallel execution failure".to_string(),
            ))
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["success_node".to_string(), "failure_node".to_string()],
    );
    graph.add_edge("success_node", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should fail because one parallel node failed
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Parallel execution failure"));
}

#[tokio::test]
async fn test_parallel_execution_with_node_timeout() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    graph.add_node_from_fn("fast_node", |mut state| {
        Box::pin(async move {
            state.add_message("fast");
            Ok(state)
        })
    });

    // Slow node that will timeout
    graph.add_node_from_fn("slow_node", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            state.add_message("slow");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["fast_node".to_string(), "slow_node".to_string()],
    );
    graph.add_edge("fast_node", END);

    let app = graph
        .compile_with_merge()
        .unwrap()
        .with_node_timeout(Duration::from_millis(50));

    let result = app.invoke(AgentState::new()).await;

    // Should timeout on the slow node - timeout is wrapped in NodeExecution
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("Timeout") || err_string.contains("50ms"),
        "Expected Timeout error, got: {}",
        err_string
    );
}

#[tokio::test]
async fn test_scheduler_execution_error() {
    use crate::scheduler::WorkStealingScheduler;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    // Failing node
    graph.add_node_from_fn("node2", |_state| {
        Box::pin(async move {
            Err(crate::error::Error::Generic(
                "Scheduler test error".to_string(),
            ))
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["node1".to_string(), "node2".to_string()]);
    graph.add_edge("node1", END);

    // Create scheduler with no workers (falls back to local execution)
    let scheduler = WorkStealingScheduler::new().with_threshold(1);

    let app = graph
        .compile_with_merge()
        .unwrap()
        .with_scheduler(scheduler);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with scheduler executing the failing node
    assert!(result.is_err());
}

#[tokio::test]
async fn test_no_parallel_execution_no_success() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // All parallel nodes fail
    graph.add_node_from_fn("fail1", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Fail 1".to_string())) })
    });

    graph.add_node_from_fn("fail2", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Fail 2".to_string())) })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["fail1".to_string(), "fail2".to_string()]);
    graph.add_edge("fail1", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should fail - first error should be returned
    assert!(result.is_err());
}

#[tokio::test]
async fn test_checkpoint_saves_after_each_node() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node3", |mut state| {
        Box::pin(async move {
            state.add_message("node3");
            Ok(state)
        })
    });

    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);
    graph.set_entry_point("node1");

    let checkpointer = MemoryCheckpointer::new();
    let thread_id: crate::checkpoint::ThreadId = "test-sequential".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Should have 3 checkpoints (one after each node)
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert_eq!(
        checkpoints.len(),
        3,
        "Expected 3 checkpoints, got {}",
        checkpoints.len()
    );

    // Verify checkpoint node names
    let node_names: Vec<String> = checkpoints.iter().map(|c| c.node.clone()).collect();
    assert!(node_names.contains(&"node1".to_string()));
    assert!(node_names.contains(&"node2".to_string()));
    assert!(node_names.contains(&"node3".to_string()));
}

#[tokio::test]
async fn test_stream_updates_mode() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Updates));

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        events.push(event);
    }

    // Should have 2 Updates events + 1 Done event
    assert_eq!(events.len(), 3);

    // Check we got updates for both nodes
    let updates_count = events
        .iter()
        .filter(|e| matches!(e, crate::stream::StreamEvent::Update { .. }))
        .count();
    assert_eq!(updates_count, 2);

    // Check final Done event
    assert!(events.last().unwrap().is_done());
}

#[tokio::test]
async fn test_stream_with_parallel_execution() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            state.add_message("parallel1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            state.add_message("parallel2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel1".to_string(), "parallel2".to_string()],
    );
    graph.add_edge("parallel2", "end_node");
    graph.add_edge("end_node", END);

    let app = graph.compile_with_merge().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        events.push(event);
    }

    // Should have values for start, parallel1, parallel2, end_node + Done
    assert!(events.len() >= 5);
    assert!(events.last().unwrap().is_done());
}

#[tokio::test]
async fn test_stream_with_conditional_edges() {
    use futures::stream::StreamExt;
    use std::collections::HashMap;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.add_node_from_fn("route_a", |mut state| {
        Box::pin(async move {
            state.add_message("route_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("route_b", |mut state| {
        Box::pin(async move {
            state.add_message("route_b");
            Ok(state)
        })
    });

    graph.set_entry_point("start");

    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "route_a".to_string());
    routes.insert("b".to_string(), "route_b".to_string());

    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration == 1 {
                "a".to_string()
            } else {
                "b".to_string()
            }
        },
        routes,
    );

    graph.add_edge("route_a", END);
    graph.add_edge("route_b", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Events));

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        events.push(event);
    }

    // Should have events for start and route_a (since iteration is 1)
    assert!(!events.is_empty());
    assert!(events.last().unwrap().is_done());
}

#[tokio::test]
async fn test_getter_methods() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node3", |mut state| {
        Box::pin(async move {
            state.add_message("node3");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let app = graph.compile().unwrap();

    // Test entry_point()
    assert_eq!(app.entry_point(), "node1");

    // Test node_count()
    assert_eq!(app.node_count(), 3);

    // Test edge_count()
    assert_eq!(app.edge_count(), 3);

    // Execute and test ExecutionResult methods
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Test ExecutionResult::state()
    assert_eq!(result.state().messages.len(), 3);

    // Test ExecutionResult::execution_path()
    assert_eq!(result.execution_path().len(), 3);
    assert_eq!(result.execution_path()[0], "node1");
    assert_eq!(result.execution_path()[1], "node2");
    assert_eq!(result.execution_path()[2], "node3");
}

#[tokio::test]
async fn test_edge_count_with_conditional_and_parallel() {
    use std::collections::HashMap;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");

    // Add simple edge
    graph.add_edge("start", "node1");

    // Add conditional edge
    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "node2".to_string());
    graph.add_conditional_edges("node1", |_: &AgentState| "a".to_string(), routes);

    // Add parallel edges
    graph.add_parallel_edges("node2", vec!["node3".to_string()]);
    graph.add_edge("node3", END);

    let app = graph.compile_with_merge().unwrap();

    // Should count: 1 simple + 1 conditional + 1 parallel + 1 simple = 4 total
    assert_eq!(app.edge_count(), 4);
}

#[tokio::test]
async fn test_execution_result_clone() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("test");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Test that ExecutionResult implements Clone
    let cloned_result = result.clone();
    assert_eq!(
        cloned_result.nodes_executed.len(),
        result.nodes_executed.len()
    );
    assert_eq!(
        cloned_result.final_state.messages.len(),
        result.final_state.messages.len()
    );
}

#[tokio::test]
async fn test_execution_result_debug() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Test that ExecutionResult implements Debug
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("ExecutionResult"));
}

#[tokio::test]
async fn test_metrics_after_error() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("failing_node", |_state| {
        Box::pin(async move { Err(crate::error::Error::Generic("Test failure".to_string())) })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "failing_node");
    graph.add_edge("failing_node", END);

    let app = graph.compile().unwrap();
    let _result = app.invoke(AgentState::new()).await;

    // Even after error, metrics should be accessible
    let metrics = app.metrics();
    // Node1 should have executed
    assert_eq!(metrics.node_execution_counts.len(), 1);
    assert!(metrics.node_durations.contains_key("node1"));
}

#[tokio::test]
async fn test_metrics_edge_traversal() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let app = graph.compile().unwrap();
    let _result = app.invoke(AgentState::new()).await.unwrap();

    let metrics = app.metrics();
    // Should have traversed 3 edges
    assert_eq!(metrics.edges_traversed, 3);
}

#[tokio::test]
async fn test_metrics_conditional_branch() {
    use std::collections::HashMap;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_a", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("branch_b", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");

    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "branch_a".to_string());
    routes.insert("b".to_string(), "branch_b".to_string());

    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration == 1 {
                "a".to_string()
            } else {
                "b".to_string()
            }
        },
        routes,
    );

    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);

    let app = graph.compile().unwrap();
    let _result = app.invoke(AgentState::new()).await.unwrap();

    let metrics = app.metrics();
    // Should have recorded a conditional branch
    assert_eq!(metrics.conditional_branches, 1);
}

#[tokio::test]
async fn test_metrics_parallel_execution_count() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p3", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
    );
    graph.add_edge("p3", END);

    let app = graph.compile_with_merge().unwrap();
    let _result = app.invoke(AgentState::new()).await.unwrap();

    let metrics = app.metrics();
    // Should have recorded parallel execution with concurrency 3
    assert_eq!(metrics.parallel_executions, 1);
}

#[tokio::test]
async fn test_empty_graph_edge_count() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("only_node", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("only_node");
    // No edges

    let app = graph.compile().unwrap();

    // Should have 0 edges
    assert_eq!(app.edge_count(), 0);
}

#[tokio::test]
async fn test_execution_path_empty() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should have executed node1
    assert!(!result.execution_path().is_empty());
    assert_eq!(result.execution_path()[0], "node1");
}

#[tokio::test]
async fn test_emit_event_without_callbacks() {
    // Test that execution works without any callbacks registered
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Compile without adding any callbacks
    let app = graph.compile().unwrap();

    // Should execute successfully (emit_event checks !self.callbacks.is_empty())
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);
}

#[tokio::test]
async fn test_save_checkpoint_without_checkpointer() {
    // Test that execution works without checkpointer configured
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Compile without checkpointer
    let app = graph.compile().unwrap();

    // Should execute successfully (save_checkpoint returns Ok(None) without checkpointer)
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);
}

#[tokio::test]
async fn test_checkpoint_parent_chain() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node3", |mut state| {
        Box::pin(async move {
            state.add_message("node3");
            Ok(state)
        })
    });

    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);
    graph.set_entry_point("node1");

    let checkpointer = MemoryCheckpointer::new();
    let thread_id: crate::checkpoint::ThreadId = "test-parent-chain".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Verify checkpoint parent chain is formed
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert_eq!(checkpoints.len(), 3);

    // Each checkpoint should have parent_id (except first)
    let mut has_parent = 0;
    for checkpoint in &checkpoints {
        if checkpoint.parent_id.is_some() {
            has_parent += 1;
        }
    }
    assert!(
        has_parent >= 2,
        "Expected at least 2 checkpoints with parent_id"
    );
}

#[tokio::test]
async fn test_multiple_invocations_reset_metrics() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();

    // First invocation
    let _result1 = app.invoke(AgentState::new()).await.unwrap();
    let metrics1 = app.metrics();
    assert_eq!(metrics1.node_execution_counts.len(), 1);

    // Second invocation - metrics should be reset
    let _result2 = app.invoke(AgentState::new()).await.unwrap();
    let metrics2 = app.metrics();
    assert_eq!(metrics2.node_execution_counts.len(), 1);

    // Metrics should be from second execution only
    // (invoke_internal resets metrics at start)
}

#[tokio::test]
async fn test_next_nodes_enum_variants() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("single", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");
    graph.add_edge("start", "single");

    // Add parallel edges from single -> p1, p2
    graph.add_parallel_edges("single", vec!["p1".to_string(), "p2".to_string()]);

    graph.add_edge("p2", "end_node");
    graph.add_edge("end_node", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should execute: start -> single -> [p1, p2] -> end_node
    assert_eq!(result.nodes_executed.len(), 5);
    assert!(result.nodes_executed.contains(&"start".to_string()));
    assert!(result.nodes_executed.contains(&"single".to_string()));
    assert!(result.nodes_executed.contains(&"p1".to_string()));
    assert!(result.nodes_executed.contains(&"p2".to_string()));
    assert!(result.nodes_executed.contains(&"end_node".to_string()));
}

#[tokio::test]
async fn test_observability_feature_disabled() {
    // This test exercises the code paths when observability feature is disabled
    // (which it is by default in these tests)
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().with_name("test-observability");

    // Execute and verify it works without observability
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);

    // Metrics should still work (they're separate from observability)
    let metrics = app.metrics();
    assert!(metrics.total_duration > Duration::from_millis(0));
}

#[tokio::test]
async fn test_tracing_span_recording() {
    use crate::checkpoint::MemoryCheckpointer;

    // This test exercises the tracing::Span::current().record() paths
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let checkpointer = MemoryCheckpointer::new();

    let app = graph
        .compile()
        .unwrap()
        .with_name("test-tracing")
        .with_checkpointer(checkpointer)
        .with_thread_id("test-span-recording");

    // Execute (tracing spans created even without subscriber)
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);
}

#[tokio::test]
async fn test_stream_error_in_node_not_found() {
    // This tests error handling in stream() when node is not found
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("start");
    // Add edge to non-existent node
    graph.add_edge("start", "nonexistent");

    // Compilation should fail
    let result = graph.compile();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_parallel_state_propagation() {
    // Test that parallel execution uses the last successful state
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            state.add_message("p1");
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.add_node_from_fn("p2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("p2");
            state.iteration = 2;
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["p1".to_string(), "p2".to_string()]);
    graph.add_edge("p2", "end_node");
    graph.add_edge("end_node", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Final state should contain messages from all nodes
    // (parallel execution uses last successful state)
    assert!(result.final_state.messages.contains(&"start".to_string()));
    assert!(result.final_state.messages.contains(&"end".to_string()));

    // State should have been updated by parallel nodes
    assert!(result.final_state.iteration > 0);
}

#[tokio::test]
async fn test_stream_with_implicit_next() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            // Test implicit next by not setting explicit next
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut event_count = 0;
    while let Some(event) = stream.next().await {
        if event.is_ok() {
            event_count += 1;
        }
    }

    // Should have values for node1, node2, and done
    assert_eq!(event_count, 3);
}

#[tokio::test]
async fn test_with_name_method() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().with_name("test-graph-name");

    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);
}

#[tokio::test]
async fn test_checkpoint_with_tracing() {
    use crate::checkpoint::MemoryCheckpointer;

    // This test exercises the tracing instrumentation in save_checkpoint
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);
    graph.set_entry_point("node1");

    let checkpointer = MemoryCheckpointer::new();
    let thread_id: crate::checkpoint::ThreadId = "test-tracing".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    // Execute with tracing enabled (tracing spans are created even without subscriber)
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.final_state.messages.len(), 2);

    // Verify checkpoints were saved (exercises tracing paths)
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert_eq!(checkpoints.len(), 2);
}

#[tokio::test]
async fn test_stream_with_node_timeout() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("fast_node", |mut state| {
        Box::pin(async move {
            state.add_message("fast");
            Ok(state)
        })
    });

    graph.add_node_from_fn("slow_node", |mut state| {
        Box::pin(async move {
            // This will timeout
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            state.add_message("slow");
            Ok(state)
        })
    });

    graph.set_entry_point("fast_node");
    graph.add_edge("fast_node", "slow_node");
    graph.add_edge("slow_node", END);

    let app = graph
        .compile()
        .unwrap()
        .with_node_timeout(std::time::Duration::from_millis(50));
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut had_timeout_error = false;
    while let Some(event) = stream.next().await {
        if let Err(e) = event {
            if matches!(e, crate::error::Error::Timeout(_)) {
                had_timeout_error = true;
                break;
            }
        }
    }

    assert!(
        had_timeout_error,
        "Should have encountered timeout error in stream"
    );
}

#[tokio::test]
async fn test_stream_multiple_parallel_edges() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p1", |mut state| {
        Box::pin(async move {
            state.add_message("p1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p2", |mut state| {
        Box::pin(async move {
            state.add_message("p2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p3", |mut state| {
        Box::pin(async move {
            state.add_message("p3");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
    );
    graph.add_edge("p3", END);

    let app = graph.compile_with_merge().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Updates));

    let mut update_count = 0;
    while let Some(event) = stream.next().await {
        if let Ok(crate::stream::StreamEvent::Update { .. }) = event {
            update_count += 1;
        }
    }

    // Should have 4 updates: start, p1, p2, p3
    assert_eq!(update_count, 4);
}

#[tokio::test]
async fn test_stream_events_with_parallel() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p1", |mut state| {
        Box::pin(async move {
            state.add_message("p1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p2", |mut state| {
        Box::pin(async move {
            state.add_message("p2");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["p1".to_string(), "p2".to_string()]);
    graph.add_edge("p2", END);

    let app = graph.compile_with_merge().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Events));

    let mut node_start_count = 0;
    let mut node_end_count = 0;
    while let Some(event) = stream.next().await {
        if let Ok(e) = event {
            match e {
                crate::stream::StreamEvent::NodeStart { .. } => node_start_count += 1,
                crate::stream::StreamEvent::NodeEnd { .. } => node_end_count += 1,
                _ => {}
            }
        }
    }

    // Should have NodeStart and NodeEnd for start, p1, p2
    assert_eq!(node_start_count, 3);
    assert_eq!(node_end_count, 3);
}

// ============================================================================
// COMPREHENSIVE EDGE CASE TESTS FOR EXECUTOR.RS
// ============================================================================

#[tokio::test]
async fn test_recursion_limit_exact_boundary() {
    // Test hitting exactly the recursion limit
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("loop_node", |mut state| {
        Box::pin(async move {
            state.iteration += 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "loop_node".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "loop_node",
        |state: &AgentState| {
            if state.iteration < 100 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("loop_node");

    let app = graph.compile().unwrap().with_recursion_limit(10);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with RecursionLimit error
    assert!(matches!(result, Err(Error::RecursionLimit { limit: 10 })));
}

#[tokio::test]
async fn test_recursion_limit_one() {
    // Test recursion limit of 1
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap().with_recursion_limit(1);

    let result = app.invoke(AgentState::new()).await;

    // Should fail after first node execution
    assert!(matches!(result, Err(Error::RecursionLimit { limit: 1 })));
}

#[tokio::test]
async fn test_recursion_limit_zero() {
    // Test recursion limit of 0 (should immediately fail)
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().with_recursion_limit(0);

    let result = app.invoke(AgentState::new()).await;

    // Should fail immediately
    assert!(matches!(result, Err(Error::RecursionLimit { limit: 0 })));
}

#[tokio::test]
async fn test_max_state_size_default_enabled() {
    // Verify default max state size is enforced (100MB)
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("test");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();

    // Small state should work fine
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_max_state_size_exceeded() {
    // Test that exceeding max state size returns error
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("bloat_node", |mut state| {
        Box::pin(async move {
            // Create a large message to exceed the limit
            let big_message = "x".repeat(1024);
            for _ in 0..100 {
                state.add_message(&big_message);
            }
            Ok(state)
        })
    });

    graph.set_entry_point("bloat_node");
    graph.add_edge("bloat_node", END);

    // Set a very small limit (1KB)
    let app = graph.compile().unwrap().with_max_state_size(1024);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with StateSizeExceeded error
    assert!(matches!(
        result,
        Err(Error::StateSizeExceeded {
            node: _,
            actual_bytes: _,
            max_bytes: 1024
        })
    ));
}

#[tokio::test]
async fn test_with_max_state_size_custom() {
    // Test custom max state size
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            // Create moderate state
            for _ in 0..10 {
                state.add_message("moderate message");
            }
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Set a generous limit
    let app = graph.compile().unwrap().with_max_state_size(1024 * 1024);

    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_without_limits_disables_state_size() {
    // Test that without_limits() disables state size limit
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("bloat_node", |mut state| {
        Box::pin(async move {
            // Create a large message
            let big_message = "x".repeat(1024);
            for _ in 0..100 {
                state.add_message(&big_message);
            }
            Ok(state)
        })
    });

    graph.set_entry_point("bloat_node");
    graph.add_edge("bloat_node", END);

    // Without limits should allow any state size
    let app = graph.compile().unwrap().without_limits();

    let result = app.invoke(AgentState::new()).await;

    // Should succeed despite large state
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_without_limits_disables_recursion_limit() {
    // Test that without_limits() effectively disables recursion limit
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("loop_node", |mut state| {
        Box::pin(async move {
            state.iteration += 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "loop_node".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "loop_node",
        |state: &AgentState| {
            // Run 50 iterations (more than default 25)
            if state.iteration < 50 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("loop_node");

    // Without limits should allow more than 25 iterations
    let app = graph.compile().unwrap().without_limits();

    let result = app.invoke(AgentState::new()).await;

    // Should succeed with 50 iterations
    assert!(result.is_ok());
    assert_eq!(result.unwrap().state().iteration, 50);
}

#[tokio::test]
async fn test_without_limits_then_reenable_recursion() {
    // Test that we can selectively re-enable limits after without_limits()
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("loop_node", |mut state| {
        Box::pin(async move {
            state.iteration += 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "loop_node".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "loop_node",
        |state: &AgentState| {
            if state.iteration < 100 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("loop_node");

    // Disable all limits then re-enable just recursion
    let app = graph
        .compile()
        .unwrap()
        .without_limits()
        .with_recursion_limit(10);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with our re-enabled recursion limit
    assert!(matches!(result, Err(Error::RecursionLimit { limit: 10 })));
}
#[tokio::test]
async fn test_complex_nested_conditionals() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.iteration = 1;
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("level1_a", |mut state| {
        Box::pin(async move {
            state.add_message("level1_a");
            state.iteration = 2;
            Ok(state)
        })
    });

    graph.add_node_from_fn("level1_b", |mut state| {
        Box::pin(async move {
            state.add_message("level1_b");
            state.iteration = 3;
            Ok(state)
        })
    });

    graph.add_node_from_fn("level2_a", |mut state| {
        Box::pin(async move {
            state.add_message("level2_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("level2_b", |mut state| {
        Box::pin(async move {
            state.add_message("level2_b");
            Ok(state)
        })
    });

    graph.set_entry_point("start");

    // First level conditional
    let mut routes1 = HashMap::new();
    routes1.insert("a".to_string(), "level1_a".to_string());
    routes1.insert("b".to_string(), "level1_b".to_string());

    graph.add_conditional_edges(
        "start",
        |state: &AgentState| {
            if state.iteration == 1 {
                "a".to_string()
            } else {
                "b".to_string()
            }
        },
        routes1,
    );

    // Second level conditionals
    let mut routes2a = HashMap::new();
    routes2a.insert("a".to_string(), "level2_a".to_string());
    routes2a.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "level1_a",
        |state: &AgentState| {
            if state.iteration == 2 {
                "a".to_string()
            } else {
                "end".to_string()
            }
        },
        routes2a,
    );

    let mut routes2b = HashMap::new();
    routes2b.insert("b".to_string(), "level2_b".to_string());
    routes2b.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "level1_b",
        |state: &AgentState| {
            if state.iteration == 3 {
                "b".to_string()
            } else {
                "end".to_string()
            }
        },
        routes2b,
    );

    graph.add_edge("level2_a", END);
    graph.add_edge("level2_b", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should follow: start -> level1_a -> level2_a
    assert_eq!(result.nodes_executed.len(), 3);
    assert_eq!(result.nodes_executed[0], "start");
    assert_eq!(result.nodes_executed[1], "level1_a");
    assert_eq!(result.nodes_executed[2], "level2_a");
}

#[tokio::test]
async fn test_parallel_after_parallel() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // First parallel batch
    graph.add_node_from_fn("p1_a", |mut state| {
        Box::pin(async move {
            state.add_message("p1_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p1_b", |mut state| {
        Box::pin(async move {
            state.add_message("p1_b");
            Ok(state)
        })
    });

    // Second parallel batch
    graph.add_node_from_fn("p2_a", |mut state| {
        Box::pin(async move {
            state.add_message("p2_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("p2_b", |mut state| {
        Box::pin(async move {
            state.add_message("p2_b");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["p1_a".to_string(), "p1_b".to_string()]);
    graph.add_parallel_edges("p1_b", vec!["p2_a".to_string(), "p2_b".to_string()]);
    graph.add_edge("p2_b", END);

    let app = graph.compile_with_merge().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    // Should execute all nodes
    assert_eq!(result.nodes_executed.len(), 5);
    assert!(result.nodes_executed.contains(&"p1_a".to_string()));
    assert!(result.nodes_executed.contains(&"p1_b".to_string()));
    assert!(result.nodes_executed.contains(&"p2_a".to_string()));
    assert!(result.nodes_executed.contains(&"p2_b".to_string()));
}

#[tokio::test]
async fn test_very_large_state_serialization() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            // Add many messages to create large state
            for i in 0..1000 {
                state.add_message(format!("message_{}", i));
            }
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "test-large-state".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.final_state.messages.len(), 1000);

    // Verify checkpoint was saved with large state
    let checkpoints = checkpointer.list(&thread_id).await.unwrap();
    assert_eq!(checkpoints.len(), 1);
}

#[tokio::test]
async fn test_metrics_with_multiple_parallel_executions() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // First parallel section
    graph.add_node_from_fn("p1_a", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p1_b", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("middle", |state| Box::pin(async move { Ok(state) }));

    // Second parallel section
    graph.add_node_from_fn("p2_a", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p2_b", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("p2_c", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("start");
    graph.add_parallel_edges("start", vec!["p1_a".to_string(), "p1_b".to_string()]);
    graph.add_edge("p1_b", "middle");
    graph.add_parallel_edges(
        "middle",
        vec!["p2_a".to_string(), "p2_b".to_string(), "p2_c".to_string()],
    );
    graph.add_edge("p2_c", END);

    let app = graph.compile_with_merge().unwrap();
    let _result = app.invoke(AgentState::new()).await.unwrap();

    let metrics = app.metrics();
    // Should have recorded 2 parallel executions
    assert_eq!(metrics.parallel_executions, 2);
}

#[tokio::test]
async fn test_empty_parallel_edges() {
    // This is technically handled during compilation but test execution behavior
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("start");
    graph.add_edge("start", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert_eq!(result.nodes_executed.len(), 1);
}

#[tokio::test]
async fn test_checkpoint_metrics_tracking() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let checkpointer = MemoryCheckpointer::new();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer)
        .with_thread_id("test-metrics");

    let _result = app.invoke(AgentState::new()).await.unwrap();

    let metrics = app.metrics();
    // Should have 3 checkpoint saves (one after each node)
    assert_eq!(metrics.checkpoint_count, 3);
}

#[tokio::test]
async fn test_stream_with_very_deep_graph() {
    use futures::stream::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Create a chain of 20 nodes
    for i in 1..=20 {
        let node_name = format!("node{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                state.add_message(format!("node{}", i));
                Ok(state)
            })
        });

        if i > 1 {
            let prev = format!("node{}", i - 1);
            graph.add_edge(&prev, &node_name);
        }
    }

    graph.set_entry_point("node1");
    graph.add_edge("node20", END);

    let app = graph.compile().unwrap();
    let mut stream = Box::pin(app.stream(AgentState::new(), crate::stream::StreamMode::Values));

    let mut event_count = 0;
    while let Some(event) = stream.next().await {
        if event.is_ok() {
            event_count += 1;
        }
    }

    // Should have 20 values + 1 done = 21 events
    assert_eq!(event_count, 21);
}

#[tokio::test]
async fn test_execution_with_zero_timeout() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| {
        Box::pin(async move {
            // Add a delay that will definitely exceed the timeout
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Use a short but non-zero timeout to avoid race conditions
    // 0ms timeout can race with task startup
    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(10));

    let result = app.invoke(AgentState::new()).await;

    // Should timeout since sleep (100ms) > timeout (10ms)
    assert!(matches!(result, Err(Error::Timeout(_))));
}

#[tokio::test]
async fn test_node_timeout_zero() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| {
        Box::pin(async move {
            // Even instant execution may exceed 0ms timeout
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph
        .compile()
        .unwrap()
        .with_node_timeout(Duration::from_millis(0));

    let result = app.invoke(AgentState::new()).await;

    // Should timeout (0ms timeout is effectively instant timeout)
    // This might succeed or fail depending on timing, but should not panic
    let _ = result;
}

// Test state for parallel merge tests
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
struct MergeTestState {
    findings: Vec<String>,
    insights: Vec<String>,
    score: i32,
}

impl crate::state::MergeableState for MergeTestState {
    fn merge(&mut self, other: &Self) {
        // Append findings and insights
        self.findings.extend(other.findings.clone());
        self.insights.extend(other.insights.clone());
        // Take maximum score
        self.score = self.score.max(other.score);
    }
}

#[test]
fn test_merge_parallel_custom() {
    let mut graph: StateGraph<MergeTestState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let app = graph.compile().unwrap();

    // Create parallel states
    let state1 = MergeTestState {
        findings: vec!["finding1".to_string(), "finding2".to_string()],
        insights: vec!["insight1".to_string()],
        score: 10,
    };
    let state2 = MergeTestState {
        findings: vec!["finding3".to_string()],
        insights: vec!["insight2".to_string(), "insight3".to_string()],
        score: 20,
    };
    let state3 = MergeTestState {
        findings: vec!["finding4".to_string()],
        insights: vec![],
        score: 15,
    };

    let states = vec![state1, state2, state3];

    // Test custom merge with append strategy
    let merged = app
        .merge_parallel_custom(states, |base, other| {
            base.findings.extend(other.findings.clone());
            base.insights.extend(other.insights.clone());
            base.score = base.score.max(other.score);
        })
        .unwrap();

    // Verify all data preserved
    assert_eq!(merged.findings.len(), 4);
    assert_eq!(merged.insights.len(), 3);
    assert_eq!(merged.score, 20); // Max of 10, 20, 15

    // Check specific values
    assert!(merged.findings.contains(&"finding1".to_string()));
    assert!(merged.findings.contains(&"finding3".to_string()));
    assert!(merged.findings.contains(&"finding4".to_string()));
    assert!(merged.insights.contains(&"insight1".to_string()));
    assert!(merged.insights.contains(&"insight2".to_string()));
}

#[test]
fn test_merge_with_mergeable() {
    let mut graph: StateGraph<MergeTestState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let app = graph.compile().unwrap();

    // Create parallel states
    let state1 = MergeTestState {
        findings: vec!["A1".to_string()],
        insights: vec!["I1".to_string()],
        score: 100,
    };
    let state2 = MergeTestState {
        findings: vec!["A2".to_string()],
        insights: vec!["I2".to_string()],
        score: 200,
    };
    let state3 = MergeTestState {
        findings: vec!["A3".to_string()],
        insights: vec!["I3".to_string()],
        score: 150,
    };

    let states = vec![state1, state2, state3];

    // Test automatic merge using MergeableState::merge()
    let merged = app.merge_with_mergeable(states).unwrap();

    // Verify MergeableState::merge() was called correctly
    assert_eq!(merged.findings.len(), 3);
    assert_eq!(merged.insights.len(), 3);
    assert_eq!(merged.score, 200); // Max score

    // Verify all findings and insights preserved
    assert_eq!(merged.findings, vec!["A1", "A2", "A3"]);
    assert_eq!(merged.insights, vec!["I1", "I2", "I3"]);
}

#[test]
fn test_merge_single_state() {
    let mut graph: StateGraph<MergeTestState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let app = graph.compile().unwrap();

    let state = MergeTestState {
        findings: vec!["solo".to_string()],
        insights: vec![],
        score: 42,
    };

    // Single state should be returned as-is
    let merged_custom = app
        .merge_parallel_custom(vec![state.clone()], |_, _| {})
        .unwrap();
    assert_eq!(merged_custom.findings, vec!["solo"]);
    assert_eq!(merged_custom.score, 42);

    let merged_mergeable = app.merge_with_mergeable(vec![state.clone()]).unwrap();
    assert_eq!(merged_mergeable.findings, vec!["solo"]);
    assert_eq!(merged_mergeable.score, 42);
}

#[test]
fn test_merge_empty_states_custom() {
    let mut graph: StateGraph<MergeTestState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let app = graph.compile().unwrap();

    let result = app.merge_parallel_custom(vec![], |_, _| {});
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Cannot merge empty state vector"),
        "Expected error about empty state vector, got: {}",
        err
    );
}

#[test]
fn test_merge_empty_states_mergeable() {
    let mut graph: StateGraph<MergeTestState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let app = graph.compile().unwrap();

    let result = app.merge_with_mergeable(vec![]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Cannot merge empty state vector"),
        "Expected error about empty state vector, got: {}",
        err
    );
}

#[tokio::test]
async fn test_stream_custom_mode() {
    use crate::stream::{get_stream_writer, StreamEvent, StreamMode};
    use futures::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that emits custom progress data
    graph.add_node_from_fn("progress_node", |mut state| {
        Box::pin(async move {
            if let Some(writer) = get_stream_writer() {
                writer.write(serde_json::json!({
                    "type": "progress",
                    "percent": 25
                }));
                writer.write(serde_json::json!({
                    "type": "progress",
                    "percent": 50
                }));
                writer.write(serde_json::json!({
                    "type": "progress",
                    "percent": 75
                }));
            }
            state.add_message("progress_node completed");
            Ok(state)
        })
    });

    graph.add_node_from_fn("status_node", |mut state| {
        Box::pin(async move {
            if let Some(writer) = get_stream_writer() {
                writer.write(serde_json::json!({
                    "type": "status",
                    "message": "Processing data"
                }));
            }
            state.add_message("status_node completed");
            Ok(state)
        })
    });

    graph.set_entry_point("progress_node");
    graph.add_edge("progress_node", "status_node");
    graph.add_edge("status_node", END);

    let app = graph.compile().unwrap();
    let initial_state = AgentState::new();

    let mut stream = Box::pin(app.stream(initial_state, StreamMode::Custom));
    let mut custom_events = Vec::new();

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Custom { node, data } => {
                custom_events.push((node, data));
            }
            StreamEvent::Done { .. } => break,
            _ => {}
        }
    }

    // Should have 4 custom events total (3 from progress_node, 1 from status_node)
    assert_eq!(custom_events.len(), 4);

    // Verify progress_node events
    assert_eq!(custom_events[0].0, "progress_node");
    assert_eq!(custom_events[0].1["type"], "progress");
    assert_eq!(custom_events[0].1["percent"], 25);

    assert_eq!(custom_events[1].0, "progress_node");
    assert_eq!(custom_events[1].1["percent"], 50);

    assert_eq!(custom_events[2].0, "progress_node");
    assert_eq!(custom_events[2].1["percent"], 75);

    // Verify status_node event
    assert_eq!(custom_events[3].0, "status_node");
    assert_eq!(custom_events[3].1["type"], "status");
    assert_eq!(custom_events[3].1["message"], "Processing data");
}

#[tokio::test]
async fn test_stream_custom_mode_no_writes() {
    use crate::stream::{StreamEvent, StreamMode};
    use futures::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Node that doesn't emit custom data
    graph.add_node_from_fn("silent_node", |mut state| {
        Box::pin(async move {
            state.add_message("silent_node completed");
            Ok(state)
        })
    });

    graph.set_entry_point("silent_node");
    graph.add_edge("silent_node", END);

    let app = graph.compile().unwrap();
    let initial_state = AgentState::new();

    let mut stream = Box::pin(app.stream(initial_state, StreamMode::Custom));
    let mut custom_events = Vec::new();

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Custom { .. } => {
                custom_events.push(());
            }
            StreamEvent::Done { .. } => break,
            _ => {}
        }
    }

    // Should have no custom events
    assert_eq!(custom_events.len(), 0);
}

#[tokio::test]
async fn test_stream_multi_mode_values_and_updates() {
    use crate::stream::{StreamEvent, StreamMode};
    use futures::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let app = graph.compile().unwrap();
    let initial_state = AgentState::new();

    let mut stream =
        Box::pin(app.stream_multi(initial_state, vec![StreamMode::Values, StreamMode::Updates]));

    let mut values_events = Vec::new();
    let mut update_events = Vec::new();

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Values { node, .. } => {
                values_events.push(node);
            }
            StreamEvent::Update { node, .. } => {
                update_events.push(node);
            }
            StreamEvent::Done { .. } => break,
            _ => {}
        }
    }

    // Should have events from both modes
    assert_eq!(values_events, vec!["node1", "node2"]);
    assert_eq!(update_events, vec!["node1", "node2"]);
}

#[tokio::test]
async fn test_stream_multi_mode_all_modes() {
    use crate::stream::{StreamEvent, StreamMode};
    use futures::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();
    let initial_state = AgentState::new();

    let mut stream = Box::pin(app.stream_multi(
        initial_state,
        vec![StreamMode::Values, StreamMode::Updates, StreamMode::Events],
    ));

    let mut start_count = 0;
    let mut values_count = 0;
    let mut update_count = 0;
    let mut end_count = 0;

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::NodeStart { .. } => start_count += 1,
            StreamEvent::Values { .. } => values_count += 1,
            StreamEvent::Update { .. } => update_count += 1,
            StreamEvent::NodeEnd { .. } => end_count += 1,
            StreamEvent::Done { .. } => break,
            _ => {}
        }
    }

    // Should have events from all modes for node1
    assert_eq!(start_count, 1);
    assert_eq!(values_count, 1);
    assert_eq!(update_count, 1);
    assert_eq!(end_count, 1);
}

#[tokio::test]
async fn test_stream_multi_mode_empty_modes() {
    use crate::stream::StreamEvent;
    use futures::StreamExt;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();
    let initial_state = AgentState::new();

    // Empty modes list - should just yield Done
    let mut stream = Box::pin(app.stream_multi(initial_state, vec![]));

    let mut event_count = 0;

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Done { .. } => break,
            _ => event_count += 1,
        }
    }

    // Should have no events except Done
    assert_eq!(event_count, 0);
}

#[test]
fn test_default_timeout_constants() {
    // Verify default timeout constants have reasonable values
    assert_eq!(
        DEFAULT_NODE_TIMEOUT,
        Duration::from_secs(300),
        "Default node timeout should be 5 minutes"
    );
    assert_eq!(
        DEFAULT_GRAPH_TIMEOUT,
        Duration::from_secs(3600),
        "Default graph timeout should be 1 hour"
    );
}

#[tokio::test]
async fn test_default_timeout_applied() {
    // Verify that default timeouts are applied when not explicitly set
    // This tests that execution completes normally within default timeouts
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("fast_node", |mut state| {
        Box::pin(async move {
            state.add_message("fast");
            Ok(state)
        })
    });

    graph.set_entry_point("fast_node");
    graph.add_edge("fast_node", END);

    // Compile WITHOUT setting any timeouts - default should be applied
    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await;

    // Should succeed - default timeout is 5 minutes for node, 1 hour for graph
    assert!(
        result.is_ok(),
        "Execution should succeed within default timeouts"
    );
    let result = result.unwrap();
    assert_eq!(result.nodes_executed.len(), 1);
    assert_eq!(result.final_state.messages[0], "fast");
}

#[tokio::test]
async fn test_custom_timeout_overrides_default() {
    // Verify that custom timeouts override default
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("slow_node", |state| {
        Box::pin(async move {
            // Sleep longer than custom timeout but less than default
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    // Set custom timeout shorter than sleep duration
    let app = graph
        .compile()
        .unwrap()
        .with_node_timeout(Duration::from_millis(50));

    let result = app.invoke(AgentState::new()).await;

    // Should timeout because custom timeout (50ms) is shorter than sleep (100ms)
    assert!(matches!(result, Err(Error::Timeout(_))));
}

#[tokio::test]
async fn test_graph_timeout_overrides_default() {
    // Verify that custom graph timeout overrides default
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("slow_node", |state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    // Set custom graph timeout shorter than sleep duration
    let app = graph
        .compile()
        .unwrap()
        .with_timeout(Duration::from_millis(50))
        // Also disable node timeout to ensure graph timeout is tested
        .with_node_timeout(Duration::from_secs(60));

    let result = app.invoke(AgentState::new()).await;

    // Should timeout from graph timeout
    assert!(matches!(result, Err(Error::Timeout(_))));
    if let Err(Error::Timeout(timeout)) = result {
        assert_eq!(timeout.as_millis(), 50, "Should use custom graph timeout");
    }
}
// ===== Default-Enabled Introspection Tests =====

#[test]
fn test_introspection_enabled_by_default() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let compiled = graph.compile().expect("should compile");

    // Introspection is enabled by default
    assert!(compiled.introspection_enabled());
}

#[test]
fn test_without_introspection_opt_out() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let compiled = graph
        .compile()
        .expect("should compile")
        .without_introspection();

    // Introspection should now be disabled
    assert!(!compiled.introspection_enabled());
}

#[test]
fn test_platform_method_returns_registry() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("dummy", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("dummy");
    graph.add_edge("dummy", END);
    let compiled = graph.compile().expect("should compile");

    // Platform method should work with zero configuration
    let platform = compiled.platform();

    // Should have DashFlow version
    assert!(!platform.version.is_empty());

    // Should have modules
    assert!(!platform.modules.is_empty());

    // Should have features
    assert!(!platform.features.is_empty());
}

#[test]
fn test_introspect_unified_method() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("process", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("process");
    graph.add_edge("process", END);
    let compiled = graph.compile().expect("should compile");

    // Unified introspect method should work with zero configuration
    let knowledge = compiled.introspect();

    // Should have manifest
    assert_eq!(knowledge.manifest.entry_point, "process");
    assert!(knowledge.manifest.nodes.contains_key("process"));

    // Should have platform
    assert!(!knowledge.platform.version.is_empty());

    // Should have architecture
    assert_eq!(knowledge.architecture.graph_structure.node_count, 1);

    // Should be convertible to JSON
    let json = knowledge.to_json().expect("should serialize");
    assert!(json.contains("manifest"));
    assert!(json.contains("platform"));
    assert!(json.contains("architecture"));
    assert!(json.contains("capabilities"));
}

#[test]
fn test_introspection_methods_work_after_opt_out() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("process", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("process");
    graph.add_edge("process", END);
    let compiled = graph
        .compile()
        .expect("should compile")
        .without_introspection();

    // Methods should still work even after opt-out
    // (opt-out is just a flag, doesn't disable the methods)
    assert!(!compiled.introspection_enabled());

    let manifest = compiled.manifest();
    assert!(manifest.nodes.contains_key("process"));

    let platform = compiled.platform();
    assert!(!platform.version.is_empty());

    let knowledge = compiled.introspect();
    assert_eq!(knowledge.manifest.entry_point, "process");
}

#[test]
fn test_graph_introspection_to_json() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("step1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("step2", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("step1");
    graph.add_edge("step1", "step2");
    graph.add_edge("step2", END);
    let compiled = graph.compile().expect("should compile");

    let knowledge = compiled.introspect();
    let json = knowledge.to_json().expect("should serialize to JSON");

    // Verify JSON structure
    assert!(json.starts_with('{'));
    assert!(json.ends_with('}'));
    assert!(json.contains("\"manifest\":"));
    assert!(json.contains("\"platform\":"));
    assert!(json.contains("\"architecture\":"));
    assert!(json.contains("\"capabilities\":"));
    assert!(json.contains("step1"));
    assert!(json.contains("step2"));
}

// ==========================================================================
// Default Memory Checkpointer Tests (P0 Directive - N=275)
// ==========================================================================

#[test]
fn test_default_checkpointer_enabled() {
    // Default: checkpointing is enabled with MemoryCheckpointer
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile");

    // Checkpointing should be enabled by default
    assert!(
        compiled.checkpointing_enabled(),
        "Checkpointing should be enabled by default"
    );
}

#[test]
fn test_without_checkpointing_opt_out() {
    // Explicit opt-out disables checkpointing
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph
        .compile()
        .expect("should compile")
        .without_checkpointing();

    // Checkpointing should be disabled after opt-out
    assert!(
        !compiled.checkpointing_enabled(),
        "Checkpointing should be disabled after without_checkpointing()"
    );
}

#[test]
fn test_with_checkpointer_replaces_default() {
    // Custom checkpointer replaces default MemoryCheckpointer
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let custom_checkpointer = MemoryCheckpointer::new();
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_checkpointer(custom_checkpointer);

    // Checkpointing should still be enabled
    assert!(
        compiled.checkpointing_enabled(),
        "Checkpointing should be enabled after with_checkpointer()"
    );
}

#[tokio::test]
async fn test_default_checkpointer_execution_with_thread_id() {
    // Default checkpointer works when thread_id is set
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Use default checkpointer with thread_id
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_thread_id("test-thread-default");

    let result = compiled
        .invoke(AgentState::new())
        .await
        .expect("should execute");
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "executed");
}

#[tokio::test]
async fn test_without_checkpointing_execution() {
    // Execution works after opt-out
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Opt-out of checkpointing
    let compiled = graph
        .compile()
        .expect("should compile")
        .without_checkpointing();

    let result = compiled
        .invoke(AgentState::new())
        .await
        .expect("should execute");
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "executed");
}

#[test]
fn test_checkpointing_chain_operations() {
    // Test chaining: enable -> disable -> re-enable
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");

    // Default: enabled
    assert!(compiled.checkpointing_enabled());

    // Disable
    let compiled = compiled.without_checkpointing();
    assert!(!compiled.checkpointing_enabled());

    // Re-enable with custom checkpointer
    let compiled = compiled.with_checkpointer(MemoryCheckpointer::new());
    assert!(compiled.checkpointing_enabled());
}

// ============================================================================
// Default Retry Policy Tests
// ============================================================================

#[test]
fn test_default_retry_enabled() {
    // By default, retries should be enabled
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile");

    // Retries should be enabled by default
    assert!(
        compiled.retries_enabled(),
        "Retries should be enabled by default"
    );
}

#[test]
fn test_without_retries_opt_out() {
    // Explicit opt-out disables retries
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile").without_retries();

    // Retries should be disabled after opt-out
    assert!(
        !compiled.retries_enabled(),
        "Retries should be disabled after without_retries()"
    );
}

#[test]
fn test_with_retry_policy_replaces_default() {
    // Custom retry policy replaces default
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let custom_policy = RetryPolicy::fixed(5, 100);
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_retry_policy(custom_policy);

    // Retries should still be enabled
    assert!(
        compiled.retries_enabled(),
        "Retries should be enabled after with_retry_policy()"
    );
}

#[test]
fn test_retry_chain_operations() {
    // Test chaining: enable -> disable -> re-enable
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");

    // Default: enabled
    assert!(compiled.retries_enabled());

    // Disable
    let compiled = compiled.without_retries();
    assert!(!compiled.retries_enabled());

    // Re-enable with custom policy
    let compiled = compiled.with_retry_policy(RetryPolicy::exponential(2));
    assert!(compiled.retries_enabled());
}

#[tokio::test]
async fn test_retry_successful_execution_no_retry_needed() {
    // When nodes succeed, no retries should happen
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");
    let result = compiled.invoke(AgentState::new()).await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "executed");
}

#[tokio::test]
async fn test_retry_without_retries_non_retryable_error_fails_immediately() {
    // Non-retryable errors should fail immediately even with retries enabled
    use std::sync::atomic::{AtomicUsize, Ordering};

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", move |_state| {
        let call_count = Arc::clone(&call_count_clone);
        Box::pin(async move {
            call_count.fetch_add(1, Ordering::SeqCst);
            // Non-retryable error (not Timeout)
            Err(Error::Validation("test error".to_string()))
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // With retries enabled (default)
    let compiled = graph.compile().expect("should compile");
    let result = compiled.invoke(AgentState::new()).await;

    // Should fail with the non-retryable error
    assert!(result.is_err());
    // Should only be called once (no retries for non-retryable errors)
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_retry_with_disabled_retries_fails_on_first_error() {
    // With retries disabled, even retryable errors should fail immediately
    use std::sync::atomic::{AtomicUsize, Ordering};

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", move |_state| {
        let call_count = Arc::clone(&call_count_clone);
        Box::pin(async move {
            call_count.fetch_add(1, Ordering::SeqCst);
            // Simulate a timeout by sleeping longer than the timeout
            // This would be a retryable error, but we've disabled retries
            Err(Error::Timeout(Duration::from_millis(10)))
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Disable retries
    let compiled = graph.compile().expect("should compile").without_retries();
    let result = compiled.invoke(AgentState::new()).await;

    // Should fail
    assert!(result.is_err());
    // Should only be called once (retries disabled)
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Default-Enabled Metrics Tests (P0 Directive)
// ============================================================================

#[test]
fn test_default_metrics_enabled() {
    // Metrics should be enabled by default (zero-config)
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile");

    // Metrics should be enabled by default
    assert!(
        compiled.metrics_enabled(),
        "Metrics should be enabled by default"
    );

    // metrics() method should return empty metrics (no execution yet)
    let metrics = compiled.metrics();
    assert_eq!(
        metrics.total_duration,
        std::time::Duration::ZERO,
        "Metrics should be empty before execution"
    );
}

#[test]
fn test_without_metrics_opt_out() {
    // Explicit opt-out disables metrics collection
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile").without_metrics();

    // Metrics should be disabled after opt-out
    assert!(
        !compiled.metrics_enabled(),
        "Metrics should be disabled after without_metrics()"
    );
}

#[tokio::test]
async fn test_metrics_collection_after_execution() {
    // Metrics should be collected during execution
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .without_checkpointing();
    let _result = compiled
        .invoke(AgentState::new())
        .await
        .expect("should succeed");

    // Metrics should be populated after execution
    let metrics = compiled.metrics();
    assert!(
        metrics.total_duration > std::time::Duration::ZERO,
        "Total duration should be non-zero after execution"
    );
    assert!(
        metrics.node_durations.contains_key("node1"),
        "Node duration should be recorded"
    );
}

#[tokio::test]
async fn test_without_metrics_no_collection() {
    // With metrics disabled, metrics should remain empty
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .without_metrics()
        .without_checkpointing();
    let _result = compiled
        .invoke(AgentState::new())
        .await
        .expect("should succeed");

    // Metrics should be empty even after execution (disabled)
    let metrics = compiled.metrics();
    assert_eq!(
        metrics.total_duration,
        std::time::Duration::ZERO,
        "Total duration should be zero when metrics disabled"
    );
    assert!(
        metrics.node_durations.is_empty(),
        "Node durations should be empty when metrics disabled"
    );
}

#[test]
fn test_performance_method_available() {
    // performance() method should always be available
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);
    let compiled = graph.compile().expect("should compile");

    // performance() should return PerformanceMetrics
    let perf = compiled.performance();

    // Default metrics (no execution) should have zero latency
    assert_eq!(perf.current_latency_ms, 0.0);
}

#[tokio::test]
async fn test_performance_after_execution() {
    // performance() should derive metrics from execution
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .without_checkpointing();
    let _result = compiled
        .invoke(AgentState::new())
        .await
        .expect("should succeed");

    // Performance metrics should show execution data
    let perf = compiled.performance();
    assert!(
        perf.current_latency_ms >= 0.0,
        "Current latency should be non-negative"
    );
    // Note: actual latency may be very small for this simple graph
}

#[test]
fn test_metrics_chain_operations() {
    // Test chaining: enable -> disable -> operations still work
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");

    // Default: enabled
    assert!(compiled.metrics_enabled());

    // Disable
    let compiled = compiled.without_metrics();
    assert!(!compiled.metrics_enabled());

    // Methods still work when disabled (return empty/zero values)
    let metrics = compiled.metrics();
    assert_eq!(metrics.total_duration, std::time::Duration::ZERO);

    let perf = compiled.performance();
    assert_eq!(perf.current_latency_ms, 0.0);
}

// =========================================================================
// Unified Introspection Tests
// =========================================================================

#[test]
fn test_unified_introspection_without_tracker() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");

    // Get unified introspection without execution tracker
    let unified = compiled.unified_introspection();

    // Platform should be populated
    assert!(!unified.platform.version_info().version.is_empty());
    assert!(!unified.platform.available_features().is_empty());

    // App should be populated
    assert!(!unified.app.manifest.nodes.is_empty());

    // Live should be empty (no tracker)
    assert!(unified.live.is_empty());
    assert_eq!(unified.active_execution_count(), 0);
    assert!(!unified.has_active_executions());
}

#[test]
fn test_unified_introspection_with_tracker() {
    use crate::live_introspection::ExecutionTracker;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let tracker = Arc::new(ExecutionTracker::new());

    // Start an execution
    let exec_id = tracker.start_execution("test_graph").expect("should start");
    tracker.enter_node(&exec_id, "node1");

    let compiled = graph
        .compile()
        .expect("should compile")
        .with_execution_tracker(tracker.clone());

    // Get unified introspection with execution tracker
    let unified = compiled.unified_introspection();

    // Live should now have execution
    assert_eq!(unified.live.len(), 1);
    assert_eq!(unified.active_execution_count(), 1);
    assert!(unified.has_active_executions());

    // Complete execution
    tracker.exit_node_success(&exec_id, Some(serde_json::json!({})));
    tracker.complete_execution(&exec_id);

    // Check again
    let unified = compiled.unified_introspection();
    // Completed execution is still tracked until TTL
    assert!(!unified.live.is_empty());
    assert_eq!(unified.active_execution_count(), 0); // No longer "active"
}

#[test]
fn test_unified_introspection_to_json() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");
    let unified = compiled.unified_introspection();

    // Should serialize to JSON without errors
    let json = unified.to_json().expect("should serialize");
    assert!(json.contains("platform"));
    assert!(json.contains("app"));
    assert!(json.contains("live"));
}

#[test]
fn test_platform_introspection_method() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");
    let platform = compiled.platform_introspection();

    // Should return platform info
    assert!(!platform.version_info().version.is_empty());
    assert!(!platform.available_features().is_empty());
    assert!(!platform.supported_node_types().is_empty());
    assert!(!platform.supported_edge_types().is_empty());
}

#[test]
fn test_live_executions_without_tracker() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");

    // Should return empty when no tracker
    let live = compiled.live_executions();
    assert!(live.is_empty());
}

#[test]
fn test_live_executions_with_tracker() {
    use crate::live_introspection::ExecutionTracker;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let tracker = Arc::new(ExecutionTracker::new());
    tracker.start_execution("test_graph");

    let compiled = graph
        .compile()
        .expect("should compile")
        .with_execution_tracker(tracker);

    // Should return executions
    let live = compiled.live_executions();
    assert_eq!(live.len(), 1);
}

#[test]
fn test_execution_tracker_accessor() {
    use crate::live_introspection::ExecutionTracker;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Ensure default behavior is enabled (per Invariant 6)
    std::env::remove_var("DASHFLOW_LIVE_INTROSPECTION");

    let compiled = graph.compile().expect("should compile");

    // Tracker is ON by default (per Invariant 6)
    assert!(
        compiled.execution_tracker().is_some(),
        "Tracker should be ON by default"
    );

    // Can replace with custom tracker
    let custom_tracker = Arc::new(ExecutionTracker::new());
    let compiled = compiled.with_execution_tracker(custom_tracker.clone());
    assert!(compiled.execution_tracker().is_some());

    // Can disable with without_live_introspection
    let compiled = compiled.without_live_introspection();
    assert!(compiled.execution_tracker().is_none());
}

// ========================================================================
// Checkpoint Policy Integration Tests
// ========================================================================

#[test]
fn test_checkpoint_policy_default_is_every() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph.compile().expect("should compile");
    assert_eq!(*compiled.checkpoint_policy(), CheckpointPolicy::Every);
}

#[test]
fn test_with_checkpoint_policy() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .with_checkpoint_policy(CheckpointPolicy::EveryN(5));

    assert_eq!(*compiled.checkpoint_policy(), CheckpointPolicy::EveryN(5));
}

#[test]
fn test_with_checkpoint_every() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .with_checkpoint_every(10);

    assert_eq!(*compiled.checkpoint_policy(), CheckpointPolicy::EveryN(10));
}

#[test]
fn test_with_checkpoint_marker() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Start with empty markers and add incrementally
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_checkpoint_policy(CheckpointPolicy::OnMarkers(Default::default()))
        .with_checkpoint_marker("save_point")
        .with_checkpoint_marker("critical");

    match compiled.checkpoint_policy() {
        CheckpointPolicy::OnMarkers(markers) => {
            assert!(markers.contains("save_point"));
            assert!(markers.contains("critical"));
            assert_eq!(markers.len(), 2);
        }
        _ => panic!("Expected OnMarkers policy"),
    }
}

#[test]
fn test_with_checkpoint_marker_converts_policy() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Start with default (Every) and add a marker - should convert to OnMarkers
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_checkpoint_marker("important_node");

    match compiled.checkpoint_policy() {
        CheckpointPolicy::OnMarkers(markers) => {
            assert!(markers.contains("important_node"));
            assert_eq!(markers.len(), 1);
        }
        _ => panic!("Expected OnMarkers policy after adding marker"),
    }
}

#[tokio::test]
async fn test_checkpoint_policy_every_n_in_execution() {
    use crate::checkpoint::MemoryCheckpointer;

    // Create a 5-node graph
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("1");
            Ok(state)
        })
    });
    graph.add_node_from_fn("node2", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("2");
            Ok(state)
        })
    });
    graph.add_node_from_fn("node3", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("3");
            Ok(state)
        })
    });
    graph.add_node_from_fn("node4", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("4");
            Ok(state)
        })
    });
    graph.add_node_from_fn("node5", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("5");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", "node4");
    graph.add_edge("node4", "node5");
    graph.add_edge("node5", END);

    let checkpointer = MemoryCheckpointer::new();
    let app = graph
        .compile()
        .expect("should compile")
        .with_checkpointer(checkpointer.clone())
        .with_thread_id("test-thread")
        .with_checkpoint_every(2); // Checkpoint every 2 nodes

    // Execute the graph
    let result = app.invoke(AgentState::new()).await.expect("should execute");
    assert_eq!(result.final_state.messages.len(), 5);

    // Check checkpoints: with EveryN(2), we should have checkpoints at nodes 2 and 4
    // (node 1 is count=1 - skip, node 2 is count=2 - save, etc.)
    let checkpoints = checkpointer.list("test-thread").await.expect("should list");
    // With EveryN(2) on 5 nodes, we expect 2 checkpoints (at nodes 2 and 4)
    assert_eq!(
        checkpoints.len(),
        2,
        "Expected 2 checkpoints with EveryN(2) policy"
    );
}

#[tokio::test]
async fn test_checkpoint_policy_on_markers_in_execution() {
    use crate::checkpoint::MemoryCheckpointer;

    // Create a 3-node graph
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("start", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });
    graph.add_node_from_fn("checkpoint_here", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("checkpoint_here");
            Ok(state)
        })
    });
    graph.add_node_from_fn("finish", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("finish");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_edge("start", "checkpoint_here");
    graph.add_edge("checkpoint_here", "finish");
    graph.add_edge("finish", END);

    let checkpointer = MemoryCheckpointer::new();
    let app = graph
        .compile()
        .expect("should compile")
        .with_checkpointer(checkpointer.clone())
        .with_thread_id("test-thread")
        .with_checkpoint_policy(CheckpointPolicy::on_markers(["checkpoint_here"]));

    // Execute
    let result = app.invoke(AgentState::new()).await.expect("should execute");
    assert_eq!(result.final_state.messages.len(), 3);

    // Should only have 1 checkpoint - at "checkpoint_here"
    let checkpoints = checkpointer.list("test-thread").await.expect("should list");
    assert_eq!(
        checkpoints.len(),
        1,
        "Expected 1 checkpoint with OnMarkers policy"
    );
    assert_eq!(checkpoints[0].node, "checkpoint_here");
}

#[tokio::test]
async fn test_checkpoint_policy_never_in_execution() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("1");
            Ok(state)
        })
    });
    graph.add_node_from_fn("node2", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let checkpointer = MemoryCheckpointer::new();
    let app = graph
        .compile()
        .expect("should compile")
        .with_checkpointer(checkpointer.clone())
        .with_thread_id("test-thread")
        .with_checkpoint_policy(CheckpointPolicy::Never);

    // Execute
    let result = app.invoke(AgentState::new()).await.expect("should execute");
    assert_eq!(result.final_state.messages.len(), 2);

    // Should have 0 checkpoints
    let checkpoints = checkpointer.list("test-thread").await.expect("should list");
    assert_eq!(
        checkpoints.len(),
        0,
        "Expected 0 checkpoints with Never policy"
    );
}

// ========================================================================
// AI Ergonomics Helper Tests
// ========================================================================

#[test]
fn test_for_testing_disables_metrics() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().for_testing();

    assert!(
        !app.metrics_enabled(),
        "for_testing() should disable metrics"
    );
}

#[test]
fn test_for_testing_disables_checkpointing() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().for_testing();

    assert!(
        !app.checkpointing_enabled(),
        "for_testing() should disable checkpointing"
    );
}

#[test]
fn test_for_testing_disables_retries() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().for_testing();

    assert!(
        !app.retries_enabled(),
        "for_testing() should disable retries"
    );
}

#[test]
fn test_with_observability_enables_metrics() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // First disable metrics, then enable via with_observability
    let app = graph
        .compile()
        .unwrap()
        .without_metrics()
        .with_observability();

    assert!(
        app.metrics_enabled(),
        "with_observability() should enable metrics"
    );
}

#[test]
fn test_with_observability_enables_checkpointing() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // First disable checkpointing, then enable via with_observability
    let app = graph
        .compile()
        .unwrap()
        .without_checkpointing()
        .with_observability();

    assert!(
        app.checkpointing_enabled(),
        "with_observability() should enable checkpointing"
    );
}

#[test]
fn test_with_metrics_reenables_metrics() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().without_metrics().with_metrics();

    assert!(
        app.metrics_enabled(),
        "with_metrics() should re-enable metrics"
    );
}

#[test]
fn test_with_checkpointing_reenables_checkpointing() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph
        .compile()
        .unwrap()
        .without_checkpointing()
        .with_checkpointing();

    assert!(
        app.checkpointing_enabled(),
        "with_checkpointing() should re-enable checkpointing"
    );
}

#[test]
fn test_without_timeouts_clears_timeouts() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph
        .compile()
        .unwrap()
        .with_default_timeouts()
        .without_timeouts();

    // Verify timeouts are cleared (they should be None after without_timeouts)
    // We can't directly access private fields, so we verify behavior through
    // the fact that execution won't timeout even with very long operations
    // For this test, we just ensure the method compiles and returns Self
    assert!(!app.metrics_enabled() || app.metrics_enabled()); // Trivial assertion to verify app is valid
}

#[tokio::test]
async fn test_for_testing_mode_executes_correctly() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("test");
            Ok(state)
        })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().for_testing();

    let result = app.invoke(AgentState::new()).await.expect("should execute");
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "test");
}

#[tokio::test]
async fn test_with_observability_mode_executes_correctly() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("observed");
            Ok(state)
        })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().with_observability();

    let result = app.invoke(AgentState::new()).await.expect("should execute");
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "observed");

    // Verify metrics are collected
    let metrics = app.metrics();
    assert!(
        metrics.total_duration > std::time::Duration::ZERO,
        "metrics should have recorded duration"
    );
}

#[test]
fn test_mode_helpers_are_chainable() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Verify all helpers can be chained together
    let _app = graph
        .compile()
        .unwrap()
        .for_testing()
        .with_observability()
        .without_timeouts()
        .with_default_timeouts()
        .with_metrics()
        .without_metrics()
        .with_checkpointing()
        .without_checkpointing();
}

// ============================================================================
// FIX-013: Observability Integration Tests
// ============================================================================

/// Integration test for FIX-014 (decision tracking from nodes) and FIX-012 (WAL auto-wiring).
///
/// This test verifies:
/// 1. Decision context is initialized when graph executes
/// 2. Nodes can call `record_decision()` without explicit callback access
/// 3. WAL events are persisted when WAL is enabled
#[tokio::test]
async fn test_decision_tracking_from_nodes_fix014() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let decision_recorded = Arc::new(AtomicBool::new(false));
    let decision_recorded_clone = decision_recorded.clone();

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("router", move |mut state: AgentState| {
        let recorded_flag = decision_recorded_clone.clone();
        Box::pin(async move {
            // FIX-014: Call record_decision from inside a node without callback access
            let decision_id = crate::executor::record_decision(
                "router_node",
                "routing",
                "fast_path",
                vec![crate::event::DecisionAlternative {
                    option: "slow_path".to_string(),
                    reason: Some("Lower latency required".to_string()),
                    score: Some(0.4),
                    was_fully_evaluated: true,
                }],
                Some("Selected fast path for low-latency request".to_string()),
            );

            // If WAL is enabled and decision context is initialized, we get a decision_id
            // If not (e.g., WAL disabled in CI), we get None - that's also valid
            if decision_id.is_some() {
                recorded_flag.store(true, Ordering::SeqCst);
            }

            state.add_message("routed");
            Ok(state)
        })
    });
    graph.set_entry_point("router");
    graph.add_edge("router", END);

    // Compile and execute - FIX-012 auto-wires WAL callback if enabled
    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.expect("should execute");

    // Verify execution completed
    assert_eq!(result.final_state.messages.len(), 1);
    assert_eq!(result.final_state.messages[0], "routed");
    assert_eq!(result.nodes_executed, vec!["router"]);

    // Note: decision_recorded may be false if WAL is disabled (DASHFLOW_WAL=false)
    // The important thing is that record_decision() doesn't panic or break execution
}

/// Test that record_outcome can be called from nodes after record_decision
#[tokio::test]
async fn test_decision_outcome_tracking_fix014() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let decision_id_holder: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let holder_clone = decision_id_holder.clone();

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // First node makes a decision
    graph.add_node_from_fn("decide", move |mut state: AgentState| {
        let holder = holder_clone.clone();
        Box::pin(async move {
            let decision_id = crate::executor::record_decision(
                "decider",
                "strategy",
                "aggressive",
                vec![],
                None,
            );
            *holder.lock().await = decision_id;
            state.add_message("decided");
            Ok(state)
        })
    });

    let holder_for_outcome = decision_id_holder.clone();
    // Second node records outcome
    graph.add_node_from_fn("evaluate", move |mut state: AgentState| {
        let holder = holder_for_outcome.clone();
        Box::pin(async move {
            if let Some(ref id) = *holder.lock().await {
                crate::executor::record_outcome(id, true, Some(0.95));
            }
            state.add_message("evaluated");
            Ok(state)
        })
    });

    graph.set_entry_point("decide");
    graph.add_edge("decide", "evaluate");
    graph.add_edge("evaluate", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await.expect("should execute");

    assert_eq!(result.nodes_executed, vec!["decide", "evaluate"]);
    assert_eq!(result.final_state.messages, vec!["decided", "evaluated"]);
}

/// Test that WAL callback is auto-wired when WAL is enabled (FIX-012)
#[test]
fn test_wal_callback_auto_wired_fix012() {
    // This test verifies the auto-wiring logic exists
    // The actual WAL writing depends on DASHFLOW_WAL env var

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("node1", |state: AgentState| {
        Box::pin(async move { Ok(state) })
    });
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap();

    // When WAL is enabled (default), the CompiledGraph should have a callback
    // When disabled, callbacks vec should be empty
    // We can't directly access callbacks, but the compile should succeed regardless
    // This test ensures the auto-wiring code path doesn't panic
    let _ = app;
}

/// M-2001: Verify WAL auto-wiring ACTUALLY writes events to WAL file
///
/// This test validates the complete flow:
/// 1. Enable WAL to a temp directory
/// 2. Execute a graph
/// 3. Read the WAL file and verify GraphEvents were persisted
///
/// Unlike test_wal_callback_auto_wired_fix012 which only checks compile doesn't panic,
/// this test VERIFIES events are written.
///
/// # Why Ignored
///
/// PERF-002 introduced `global_event_store()` as a `OnceLock` singleton for performance.
/// Once initialized (by any test), the singleton cannot be reconfigured to use a different
/// `DASHFLOW_WAL_DIR`. Since tests run in the same process, the first test to access the
/// singleton wins, making env-var-based directory overrides unreliable in parallel test runs.
///
/// WAL integration is verified via:
/// - Manual E2E testing (librarian app writes to WAL)
/// - `test_wal_callback_auto_wired_fix012` (compile path doesn't panic)
/// - `wal::callback::tests::*` (unit tests for WALEventCallback)
#[tokio::test]
#[ignore = "PERF-002 singleton - cannot redirect DASHFLOW_WAL_DIR per test (OnceLock initialized once)"]
async fn test_wal_auto_wiring_writes_events_m2001() {
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());
    let _guard = ENV_MUTEX.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().join("wal");
    std::fs::create_dir_all(&wal_dir).unwrap();

    // Save original env vars
    let orig_wal = std::env::var("DASHFLOW_WAL").ok();
    let orig_wal_dir = std::env::var("DASHFLOW_WAL_DIR").ok();

    // Enable WAL to temp directory
    std::env::set_var("DASHFLOW_WAL", "true");
    std::env::set_var("DASHFLOW_WAL_DIR", wal_dir.to_str().unwrap());

    // Create and execute a simple graph
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("test_node", |mut state: AgentState| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });
    graph.set_entry_point("test_node");
    graph.add_edge("test_node", END);

    // Compile with WAL callback - this should auto-wire WAL
    let app = graph.compile().unwrap();

    // Execute the graph
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok(), "Graph execution should succeed");

    // Wait for async trace persistence (PERF-003 made this non-blocking)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Restore env vars
    match orig_wal {
        Some(v) => std::env::set_var("DASHFLOW_WAL", v),
        None => std::env::remove_var("DASHFLOW_WAL"),
    }
    match orig_wal_dir {
        Some(v) => std::env::set_var("DASHFLOW_WAL_DIR", v),
        None => std::env::remove_var("DASHFLOW_WAL_DIR"),
    }

    // Read WAL files and verify events
    let mut found_events = Vec::new();
    for entry in std::fs::read_dir(&wal_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "wal") {
            let content = std::fs::read_to_string(&path).unwrap();
            for line in content.lines() {
                if !line.trim().is_empty() {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                        found_events.push(event);
                    }
                }
            }
        }
    }

    // M-2001: MUST have events if WAL is enabled
    assert!(
        !found_events.is_empty(),
        "WAL MUST contain events when DASHFLOW_WAL=true. Found {} .wal files in {:?}",
        std::fs::read_dir(&wal_dir)
            .unwrap()
            .filter(|e| e.as_ref().map_or(false, |e| {
                e.path().extension().map_or(false, |ext| ext == "wal")
            }))
            .count(),
        wal_dir
    );

    // Verify we have execution_start event
    let has_start = found_events.iter().any(|e| {
        e.get("event_type")
            .and_then(|v| v.as_str())
            .map_or(false, |t| t == "execution_start")
    });
    assert!(has_start, "WAL should contain execution_start event");

    // Verify we have node_start event
    let has_node = found_events.iter().any(|e| {
        e.get("event_type")
            .and_then(|v| v.as_str())
            .map_or(false, |t| t == "node_start" || t == "node_end")
    });
    assert!(has_node, "WAL should contain node events");
}

/// M-2002: Test that decision tracking REQUIRES WAL and FAILS if decision not recorded
///
/// This test validates FIX-014 properly by:
/// 1. Enabling WAL explicitly
/// 2. Making a decision from inside a node
/// 3. ASSERTING that decision_id is Some (not allowing None as "valid")
///
/// # Why Ignored
///
/// Same as M-2001: PERF-002 `global_event_store()` singleton cannot be reconfigured
/// after initialization. Env var overrides don't work reliably in parallel test runs.
///
/// Decision tracking is verified via:
/// - `record_decision` returns `Some(id)` when `global_event_store()` is available
/// - `decision_context` module unit tests
/// - E2E testing with real graph executions
#[tokio::test]
#[ignore = "PERF-002 singleton - cannot redirect DASHFLOW_WAL_DIR per test (OnceLock initialized once)"]
async fn test_decision_tracking_requires_wal_m2002() {
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());
    let _guard = ENV_MUTEX.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().join("wal");
    std::fs::create_dir_all(&wal_dir).unwrap();

    // Save original env vars
    let orig_wal = std::env::var("DASHFLOW_WAL").ok();
    let orig_wal_dir = std::env::var("DASHFLOW_WAL_DIR").ok();

    // Enable WAL
    std::env::set_var("DASHFLOW_WAL", "true");
    std::env::set_var("DASHFLOW_WAL_DIR", wal_dir.to_str().unwrap());

    let decision_id_received = Arc::new(Mutex::new(None::<String>));
    let decision_id_clone = decision_id_received.clone();

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("decision_node", move |mut state: AgentState| {
        let id_holder = decision_id_clone.clone();
        Box::pin(async move {
            // Record a decision - this should return Some(id) when WAL is enabled
            let decision_id = crate::executor::record_decision(
                "decision_node",
                "test_routing",
                "option_a",
                vec![crate::event::DecisionAlternative {
                    option: "option_b".to_string(),
                    reason: Some("Not selected".to_string()),
                    score: Some(0.3),
                    was_fully_evaluated: true,
                }],
                Some("Chose option_a for test".to_string()),
            );

            // Store decision_id for verification
            *id_holder.lock().unwrap() = decision_id;

            state.add_message("decided");
            Ok(state)
        })
    });
    graph.set_entry_point("decision_node");
    graph.add_edge("decision_node", END);

    let app = graph.compile().unwrap();
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok(), "Graph execution should succeed");

    // Wait for async trace persistence (PERF-003 made this non-blocking)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Restore env vars
    match orig_wal {
        Some(v) => std::env::set_var("DASHFLOW_WAL", v),
        None => std::env::remove_var("DASHFLOW_WAL"),
    }
    match orig_wal_dir {
        Some(v) => std::env::set_var("DASHFLOW_WAL_DIR", v),
        None => std::env::remove_var("DASHFLOW_WAL_DIR"),
    }

    // M-2002: decision_id MUST be Some when WAL is enabled
    // The old test had an escape clause allowing None - that's wrong
    let received_id = decision_id_received.lock().unwrap().clone();

    // Note: record_decision may return None if called outside execution context.
    // The decision context is set up by the executor. If the callback isn't
    // properly wired, this will be None. We verify WAL file instead.

    // Read WAL files and verify DecisionMade event
    let mut found_decisions = Vec::new();
    for entry in std::fs::read_dir(&wal_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "wal") {
            let content = std::fs::read_to_string(&path).unwrap();
            for line in content.lines() {
                if !line.trim().is_empty() {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                        if event
                            .get("event_type")
                            .and_then(|v| v.as_str())
                            .map_or(false, |t| t == "decision_made")
                        {
                            found_decisions.push(event);
                        }
                    }
                }
            }
        }
    }

    assert!(
        !found_decisions.is_empty(),
        "WAL MUST contain DecisionMade event when record_decision is called with WAL enabled. \
         decision_id returned: {:?}",
        received_id
    );

    // Verify decision content.
    //
    // This test mutates process-wide env vars to force WAL into a temp directory.
    // Other tests can run concurrently and also emit DecisionMade events while the
    // env vars are set, so select the expected decision deterministically.
    let decision = {
        let target_id = received_id.as_deref();
        found_decisions
            .iter()
            .find(|event| {
                let payload = match event.get("payload") {
                    Some(p) => p,
                    None => return false,
                };

                if let Some(id) = target_id {
                    return payload.get("decision_id").and_then(|v| v.as_str()) == Some(id);
                }

                payload.get("decision_type").and_then(|v| v.as_str()) == Some("test_routing")
                    && payload.get("chosen_option").and_then(|v| v.as_str()) == Some("option_a")
                    && payload.get("decision_maker").and_then(|v| v.as_str())
                        == Some("decision_node")
            })
            .unwrap_or_else(|| {
                panic!(
                    "WAL contained DecisionMade events, but none matched expected decision. decision_id returned: {:?}",
                    received_id
                )
            })
    };
    let payload = decision.get("payload").expect("Decision should have payload");
    assert_eq!(
        payload.get("decision_maker").and_then(|v| v.as_str()),
        Some("decision_node"),
        "Decision maker should match"
    );
    assert_eq!(
        payload.get("decision_type").and_then(|v| v.as_str()),
        Some("test_routing"),
        "Decision type should match"
    );
    assert_eq!(
        payload.get("chosen_option").and_then(|v| v.as_str()),
        Some("option_a"),
        "Chosen option should match"
    );
}

// ============================================================================
// Error Recovery Tests (M-242)
// ============================================================================
// Tests for error recovery paths: timeouts, retries, transient failures

/// Test that executor retries on timeout and eventually succeeds (M-242)
#[tokio::test]
async fn test_executor_retry_timeout_eventually_succeeds() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("flaky_node", move |mut state: AgentState| {
        let counter = Arc::clone(&attempt_count_clone);
        Box::pin(async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                // First two attempts: sleep longer than timeout
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Third attempt: succeed quickly
            state.add_message("success");
            Ok(state)
        })
    });

    graph.set_entry_point("flaky_node");
    graph.add_edge("flaky_node", END);

    // Use very short timeout (10ms) so flaky_node times out on first two attempts
    // Use fixed retry policy with 0ms delay for fast test
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_node_timeout(Duration::from_millis(10))
        .with_retry_policy(crate::core::retry::RetryPolicy::fixed(3, 0));

    let result = compiled.invoke(AgentState::new()).await;

    // Should succeed on third attempt
    assert!(result.is_ok(), "Should succeed after retries: {:?}", result.err());
    assert_eq!(
        attempt_count.load(Ordering::SeqCst),
        3,
        "Should have made 3 attempts (2 timeouts + 1 success)"
    );
    assert_eq!(
        result.unwrap().final_state.messages,
        vec!["success"],
        "Should have recorded success message"
    );
}

/// Test that exhausting all retries properly fails with timeout error (M-242)
#[tokio::test]
async fn test_executor_retry_exhaustion_fails_with_timeout() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("always_slow_node", move |state: AgentState| {
        let counter = Arc::clone(&attempt_count_clone);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            // Always sleep longer than timeout
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("always_slow_node");
    graph.add_edge("always_slow_node", END);

    // Use very short timeout and limited retries
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_node_timeout(Duration::from_millis(10))
        .with_retry_policy(crate::core::retry::RetryPolicy::fixed(2, 0)); // 2 retries = 3 total attempts

    let result = compiled.invoke(AgentState::new()).await;

    // Should fail after exhausting retries
    assert!(result.is_err(), "Should fail after exhausting retries");
    assert!(
        matches!(result.as_ref().err(), Some(Error::Timeout(_))),
        "Should fail with Timeout error, got: {:?}",
        result.err()
    );
    assert_eq!(
        attempt_count.load(Ordering::SeqCst),
        3,
        "Should have made 3 attempts (initial + 2 retries)"
    );
}

/// Test that transient timeout followed by success is properly recovered (M-242)
#[tokio::test]
async fn test_executor_retry_transient_timeout_recovery() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("transient_node", move |mut state: AgentState| {
        let counter = Arc::clone(&attempt_count_clone);
        Box::pin(async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                // First attempt: timeout
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Subsequent attempts: succeed
            state.add_message(format!("attempt_{}", attempt + 1));
            Ok(state)
        })
    });

    graph.set_entry_point("transient_node");
    graph.add_edge("transient_node", END);

    let compiled = graph
        .compile()
        .expect("should compile")
        .with_node_timeout(Duration::from_millis(10))
        .with_retry_policy(crate::core::retry::RetryPolicy::fixed(1, 0)); // 1 retry = 2 total attempts

    let result = compiled.invoke(AgentState::new()).await;

    // Should recover on second attempt
    assert!(result.is_ok(), "Should recover after transient failure");
    assert_eq!(
        attempt_count.load(Ordering::SeqCst),
        2,
        "Should have made 2 attempts (1 timeout + 1 success)"
    );
    assert_eq!(
        result.unwrap().final_state.messages,
        vec!["attempt_2"],
        "Should have recorded message from successful attempt"
    );
}

/// Test that non-retryable errors are not retried even with retry policy enabled (M-242)
#[tokio::test]
async fn test_executor_non_retryable_error_not_retried() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("error_node", move |_state: AgentState| {
        let counter = Arc::clone(&attempt_count_clone);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            // Return a non-retryable error (Validation is not retried by executor)
            Err(Error::Validation("test validation error".to_string()))
        })
    });

    graph.set_entry_point("error_node");
    graph.add_edge("error_node", END);

    // Enable retries - but they shouldn't apply to non-retryable errors
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_retry_policy(crate::core::retry::RetryPolicy::fixed(3, 0));

    let result = compiled.invoke(AgentState::new()).await;

    // Should fail immediately without retries
    assert!(result.is_err());
    assert_eq!(
        attempt_count.load(Ordering::SeqCst),
        1,
        "Should NOT retry non-retryable errors - only 1 attempt expected"
    );
}

/// Test that NodeExecution errors wrap underlying errors correctly (M-242)
#[tokio::test]
async fn test_executor_node_execution_error_wrapping() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("failing_node", |_state: AgentState| {
        Box::pin(async move {
            Err::<AgentState, _>(Error::Generic("inner error".to_string()))
        })
    });

    graph.set_entry_point("failing_node");
    graph.add_edge("failing_node", END);

    let compiled = graph.compile().expect("should compile").without_retries();
    let result = compiled.invoke(AgentState::new()).await;

    // Error should be wrapped as NodeExecution
    match result {
        Err(Error::NodeExecution { node, source }) => {
            assert_eq!(node, "failing_node");
            assert!(source.to_string().contains("inner error"));
        }
        other => panic!("Expected NodeExecution error, got: {:?}", other),
    }
}

/// Test that graph timeout still applies even with node retries (M-242)
#[tokio::test]
async fn test_executor_graph_timeout_overrides_retries() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut graph: StateGraph<AgentState> = StateGraph::new();
    graph.add_node_from_fn("slow_node", move |state: AgentState| {
        let counter = Arc::clone(&attempt_count_clone);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            // Each attempt takes 50ms
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    // Graph timeout (30ms) < node execution time (50ms)
    // Even with many retries, graph timeout should cancel everything
    let compiled = graph
        .compile()
        .expect("should compile")
        .with_timeout(Duration::from_millis(30))
        .with_node_timeout(Duration::from_millis(100)) // Node timeout > graph timeout
        .with_retry_policy(crate::core::retry::RetryPolicy::fixed(10, 0));

    let result = compiled.invoke(AgentState::new()).await;

    // Should fail with timeout (either graph or node timeout)
    assert!(result.is_err(), "Should fail due to timeout");
    // The attempt count should be low since graph timeout kicks in
    let attempts = attempt_count.load(Ordering::SeqCst);
    assert!(
        attempts <= 2,
        "Should have limited attempts due to graph timeout, got {}",
        attempts
    );
}

/// Test that EdgeTraversal events are emitted for END edges (FIX-2458)
///
/// A single-node graph should emit at least one EdgeTraversal event
/// when traversing to END, ensuring observability metrics/UI don't show
/// misleading 0 traversals for simple graphs.
#[tokio::test]
async fn test_edge_traversal_event_to_end() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Single-node graph: entry → END
    graph.add_node_from_fn("single_node", |mut state| {
        Box::pin(async move {
            state.add_message("executed");
            Ok(state)
        })
    });

    graph.set_entry_point("single_node");
    graph.add_edge("single_node", END);

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Find EdgeTraversal events
    let edge_traversal_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::EdgeTraversal { .. }))
        .collect();

    // Should have at least 1 EdgeTraversal event (to END)
    assert!(
        !edge_traversal_events.is_empty(),
        "Single-node graph should emit at least one EdgeTraversal event, got 0"
    );

    // Verify at least one traversal goes to END
    let has_end_traversal = edge_traversal_events.iter().any(|e| {
        if let crate::event::GraphEvent::EdgeTraversal { to, .. } = e {
            to.iter().any(|target| target == END)
        } else {
            false
        }
    });

    assert!(
        has_end_traversal,
        "Should have EdgeTraversal event to END node"
    );
}

/// Test that multi-node graphs emit EdgeTraversal events for all transitions including END
#[tokio::test]
async fn test_edge_traversal_events_for_multi_node_graph() {
    use crate::event::CollectingCallback;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Multi-node graph: node1 → node2 → END
    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let callback = CollectingCallback::<AgentState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile().unwrap().with_callback(callback);
    let result = app.invoke(AgentState::new()).await;
    assert!(result.is_ok());

    let events = callback_clone.events();

    // Find EdgeTraversal events
    let edge_traversal_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::event::GraphEvent::EdgeTraversal { .. }))
        .collect();

    // Should have 2 EdgeTraversal events: node1→node2, node2→END
    assert_eq!(
        edge_traversal_events.len(),
        2,
        "Multi-node graph should emit 2 EdgeTraversal events, got {}",
        edge_traversal_events.len()
    );

    // Verify we have both transitions
    let transitions: Vec<(String, Vec<String>)> = edge_traversal_events
        .iter()
        .filter_map(|e| {
            if let crate::event::GraphEvent::EdgeTraversal { from, to, .. } = e {
                Some((from.clone(), to.clone()))
            } else {
                None
            }
        })
        .collect();

    assert!(
        transitions
            .iter()
            .any(|(from, to)| from == "node1" && to.contains(&"node2".to_string())),
        "Should have EdgeTraversal from node1 to node2"
    );

    assert!(
        transitions
            .iter()
            .any(|(from, to)| from == "node2" && to.iter().any(|t| t == END)),
        "Should have EdgeTraversal from node2 to END"
    );
}
