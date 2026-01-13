// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for interrupt, resume, and state management functionality.
//!
//! Extracted from executor/tests.rs by Worker #1695.

use super::*;

#[tokio::test]
async fn test_interrupt_before_without_checkpointer() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Set interrupt_before with checkpointing opt-out
    let app = graph
        .compile()
        .unwrap()
        .without_checkpointing() // Opt-out of default checkpointer
        .with_interrupt_before(vec!["node1"]);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with InterruptWithoutCheckpointer error
    assert!(matches!(
        result,
        Err(Error::InterruptWithoutCheckpointer(_))
    ));
}

#[tokio::test]
async fn test_interrupt_before_without_thread_id() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Set interrupt_before with checkpointer but no thread_id
    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_interrupt_before(vec!["node1"]);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with InterruptWithoutThreadId error
    assert!(matches!(result, Err(Error::InterruptWithoutThreadId(_))));
}

#[tokio::test]
async fn test_interrupt_after_without_checkpointer() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Set interrupt_after with checkpointing opt-out
    let app = graph
        .compile()
        .unwrap()
        .without_checkpointing() // Opt-out of default checkpointer
        .with_interrupt_after(vec!["node1"]);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with InterruptWithoutCheckpointer error
    assert!(matches!(
        result,
        Err(Error::InterruptWithoutCheckpointer(_))
    ));
}

#[tokio::test]
async fn test_interrupt_after_without_thread_id() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    // Set interrupt_after with checkpointer but no thread_id
    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_interrupt_after(vec!["node1"]);

    let result = app.invoke(AgentState::new()).await;

    // Should fail with InterruptWithoutThreadId error
    assert!(matches!(result, Err(Error::InterruptWithoutThreadId(_))));
}

#[tokio::test]
async fn test_interrupt_before_basic() {
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

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "test-interrupt-before".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone())
        .with_interrupt_before(vec!["node2"]);

    // First invocation - should interrupt before node2
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert!(result.interrupted_at.is_some());
    assert_eq!(result.interrupted_at.unwrap(), "node2");
    // nodes_executed includes node2 in the list even though it hasn't executed yet (it's in the execution path)
    assert!(result.nodes_executed.contains(&"node1".to_string())); // node1 executed
    assert_eq!(result.final_state.messages.len(), 1); // Only node1's message

    // Resume - should execute node2
    let result = app.resume().await.unwrap();
    assert!(result.interrupted_at.is_none());
    assert_eq!(result.nodes_executed.len(), 1); // node2
    assert_eq!(result.final_state.messages.len(), 2); // node1 and node2
}

#[tokio::test]
async fn test_interrupt_after_basic() {
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

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "test-interrupt-after".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone())
        .with_interrupt_after(vec!["node1"]);

    // First invocation - should interrupt after node1
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert!(result.interrupted_at.is_some());
    assert_eq!(result.interrupted_at.unwrap(), "node1");
    assert_eq!(result.nodes_executed.len(), 1); // node1 executed
    assert_eq!(result.final_state.messages.len(), 1); // node1's message
    assert_eq!(result.next_nodes, vec!["node2".to_string()]); // Next should be node2

    // Resume - should execute node2
    let result = app.resume().await.unwrap();
    assert!(result.interrupted_at.is_none());
    assert_eq!(result.nodes_executed.len(), 1); // node2
    assert_eq!(result.final_state.messages.len(), 2); // node1 and node2
}

#[tokio::test]
async fn test_interrupt_after_at_end() {
    use crate::checkpoint::MemoryCheckpointer;

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
    let thread_id = "test-interrupt-after-end".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone())
        .with_interrupt_after(vec!["node1"]);

    // First invocation - should interrupt after node1
    let result = app.invoke(AgentState::new()).await.unwrap();

    assert!(result.interrupted_at.is_some());
    assert_eq!(result.interrupted_at.unwrap(), "node1");
    assert_eq!(result.next_nodes, Vec::<String>::new()); // No next nodes

    // Resume - should complete immediately
    let result = app.resume().await.unwrap();
    assert!(result.interrupted_at.is_none());
    assert_eq!(result.nodes_executed.len(), 0); // No more nodes to execute
}

#[tokio::test]
async fn test_resume_without_checkpointer() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph.compile().unwrap().without_checkpointing(); // Opt-out of default

    let result = app.resume().await;

    // Should fail with ResumeWithoutCheckpointer
    assert!(matches!(result, Err(Error::ResumeWithoutCheckpointer)));
}

#[tokio::test]
async fn test_resume_without_thread_id() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new());

    let result = app.resume().await;

    // Should fail with ResumeWithoutThreadId
    assert!(matches!(result, Err(Error::ResumeWithoutThreadId)));
}

#[tokio::test]
async fn test_resume_no_checkpoint() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("nonexistent-thread");

    let result = app.resume().await;

    // Should fail with NoCheckpointToResume
    assert!(matches!(result, Err(Error::NoCheckpointToResume(_))));
}

#[tokio::test]
async fn test_get_current_state_without_checkpointer() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph.compile().unwrap().without_checkpointing(); // Opt-out of default

    let result = app.get_current_state().await;

    // Should fail with generic error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_current_state_without_thread_id() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new());

    let result = app.get_current_state().await;

    // Should fail with generic error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_current_state_no_checkpoint() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("nonexistent");

    let result = app.get_current_state().await;

    // Should fail - no checkpoint exists
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_current_state_success() {
    use crate::checkpoint::MemoryCheckpointer;

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
    let thread_id = "test-get-state".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer)
        .with_thread_id(thread_id);

    // Execute to create checkpoint
    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Get current state
    let state = app.get_current_state().await.unwrap();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0], "node1");
}

#[tokio::test]
async fn test_update_state_without_checkpointer() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph.compile().unwrap().without_checkpointing(); // Opt-out of default

    let result = app.update_state(|state| state).await;

    // Should fail with generic error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_state_without_thread_id() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new());

    let result = app.update_state(|state| state).await;

    // Should fail with generic error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_state_no_checkpoint() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.set_entry_point("node1");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("nonexistent");

    let result = app.update_state(|state| state).await;

    // Should fail - no checkpoint exists
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_state_success() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1");
            state.iteration = 1;
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "test-update-state".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    // Execute to create checkpoint
    let _result = app.invoke(AgentState::new()).await.unwrap();

    // Update state
    app.update_state(|mut state| {
        state.add_message("updated");
        state.iteration = 42;
        state
    })
    .await
    .unwrap();

    // Verify state was updated
    let state = app.get_current_state().await.unwrap();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[1], "updated");
    assert_eq!(state.iteration, 42);
}

#[tokio::test]
async fn test_multiple_interrupt_before_resume() {
    use crate::checkpoint::MemoryCheckpointer;
    use uuid::Uuid;

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

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = format!("test-multiple-interrupts-{}", Uuid::new_v4());

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer)
        .with_thread_id(thread_id)
        .with_interrupt_before(vec!["node2", "node3"]);

    // First invoke - interrupt at node2
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.interrupted_at, Some("node2".to_string()));
    assert_eq!(result.final_state.messages.len(), 1);

    // First resume - interrupt at node3
    let result = app.resume().await.unwrap();
    assert_eq!(result.interrupted_at, Some("node3".to_string()));
    assert_eq!(result.final_state.messages.len(), 2);

    // Second resume - complete
    let result = app.resume().await.unwrap();
    assert!(result.interrupted_at.is_none());
    assert_eq!(result.final_state.messages.len(), 3);
}

#[tokio::test]
async fn test_interrupt_before_with_conditional() {
    use crate::checkpoint::MemoryCheckpointer;

    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
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

    graph.set_entry_point("start");
    graph.add_edge("branch_a", END);
    graph.add_edge("branch_b", END);

    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "test-interrupt-conditional".to_string();

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(checkpointer)
        .with_thread_id(thread_id)
        .with_interrupt_before(vec!["branch_a"]);

    // First invoke - interrupt at branch_a
    let result = app.invoke(AgentState::new()).await.unwrap();
    assert_eq!(result.interrupted_at, Some("branch_a".to_string()));

    // Resume - complete
    let result = app.resume().await.unwrap();
    assert!(result.interrupted_at.is_none());
    assert_eq!(result.final_state.messages[0], "branch_a");
}
