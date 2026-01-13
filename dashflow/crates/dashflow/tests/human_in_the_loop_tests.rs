//! Human-in-the-Loop Tests
//!
//! Tests for interrupt detection, resume functionality, and state management
//! in human-in-the-loop workflows. These tests verify that execution can be
//! paused before or after specified nodes, resumed from checkpoints, and that
//! state can be inspected and modified during interruptions.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::checkpoint::MemoryCheckpointer;
use dashflow::{Error, GraphBuilder, MergeableState, END};
use serde::{Deserialize, Serialize};

/// Test state for human-in-the-loop workflows
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ReviewState {
    content: String,
    generated: bool,
    reviewed: bool,
    approved: bool,
    finalized: bool,
    reviewer_comments: Option<String>,
}

impl MergeableState for ReviewState {
    fn merge(&mut self, other: &Self) {
        if !other.content.is_empty() {
            if self.content.is_empty() {
                self.content = other.content.clone();
            } else {
                self.content.push('\n');
                self.content.push_str(&other.content);
            }
        }
        self.generated = self.generated || other.generated;
        self.reviewed = self.reviewed || other.reviewed;
        self.approved = self.approved || other.approved;
        self.finalized = self.finalized || other.finalized;
        if other.reviewer_comments.is_some() {
            self.reviewer_comments = other.reviewer_comments.clone();
        }
    }
}

/// Generate content node
async fn generate_node(mut state: ReviewState) -> Result<ReviewState, Error> {
    state.content = "Draft content generated".to_string();
    state.generated = true;
    Ok(state)
}

/// Review content node
async fn review_node(mut state: ReviewState) -> Result<ReviewState, Error> {
    state.reviewed = true;
    Ok(state)
}

/// Finalize content node
async fn finalize_node(mut state: ReviewState) -> Result<ReviewState, Error> {
    state.finalized = true;
    Ok(state)
}

/// Approval decision node
async fn approval_node(state: ReviewState) -> Result<ReviewState, Error> {
    // This node just marks that approval decision was made
    // The actual approval value comes from state updates during interrupt
    Ok(state)
}

#[tokio::test]
async fn test_interrupt_before() {
    // Verify execution pauses BEFORE specified node

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-1")
        .with_interrupt_before(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    let result = app.invoke(initial_state).await.unwrap();

    // Should have executed "generate" but stopped before "review"
    assert_eq!(result.interrupted_at, Some("review".to_string()));
    // Note: nodes_executed includes "review" even though it hasn't executed yet
    // This is the current behavior - the node is added to the list before checking interrupt
    assert_eq!(result.nodes_executed, vec!["generate", "review"]);
    assert!(result.final_state.generated);
    assert!(!result.final_state.reviewed); // Verify node didn't actually execute
    assert!(!result.final_state.finalized);
}

#[tokio::test]
async fn test_interrupt_after() {
    // Verify execution pauses AFTER specified node

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-2")
        .with_interrupt_after(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    let result = app.invoke(initial_state).await.unwrap();

    // Should have executed "generate" and "review", then stopped
    assert_eq!(result.interrupted_at, Some("review".to_string()));
    assert_eq!(result.nodes_executed, vec!["generate", "review"]);
    assert!(result.final_state.generated);
    assert!(result.final_state.reviewed);
    assert!(!result.final_state.finalized);
}

#[tokio::test]
async fn test_resume_from_interrupt() {
    // Verify resume() continues execution from checkpoint

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-3")
        .with_interrupt_before(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // First invoke - should interrupt before "review"
    let result1 = app.invoke(initial_state).await.unwrap();
    assert_eq!(result1.interrupted_at, Some("review".to_string()));
    assert!(result1.final_state.generated);
    assert!(!result1.final_state.reviewed);

    // Resume - should complete remaining nodes
    let result2 = app.resume().await.unwrap();
    assert_eq!(result2.interrupted_at, None);
    assert_eq!(result2.nodes_executed, vec!["review", "finalize"]);
    assert!(result2.final_state.generated);
    assert!(result2.final_state.reviewed);
    assert!(result2.final_state.finalized);
}

#[tokio::test]
async fn test_update_state_before_resume() {
    // Verify state updates persist and affect resumed execution

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-4")
        .with_interrupt_after(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // First invoke - interrupt after review
    let result1 = app.invoke(initial_state).await.unwrap();
    assert_eq!(result1.interrupted_at, Some("review".to_string()));
    assert!(!result1.final_state.approved);
    assert!(result1.final_state.reviewer_comments.is_none());

    // Update state to approve
    app.update_state(|mut state| {
        state.approved = true;
        state.reviewer_comments = Some("Looks great!".to_string());
        state
    })
    .await
    .unwrap();

    // Verify state was updated
    let current_state = app.get_current_state().await.unwrap();
    assert!(current_state.approved);
    assert_eq!(
        current_state.reviewer_comments,
        Some("Looks great!".to_string())
    );

    // Resume with updated state
    let result2 = app.resume().await.unwrap();
    assert_eq!(result2.interrupted_at, None);
    assert!(result2.final_state.approved);
    assert_eq!(
        result2.final_state.reviewer_comments,
        Some("Looks great!".to_string())
    );
}

#[tokio::test]
async fn test_interrupt_requires_checkpointer() {
    // Verify error when interrupt configured without checkpointer

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_edge("generate", "review")
        .add_edge("review", END)
        .set_entry_point("generate");

    // Explicitly disable checkpointer (default is MemoryCheckpointer)
    let app = graph
        .compile()
        .unwrap()
        .without_checkpointing()
        .with_interrupt_before(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    let result = app.invoke(initial_state).await;

    // Should error because interrupt requires checkpointer
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::InterruptWithoutCheckpointer(node) => {
            assert_eq!(node, "review");
        }
        other => panic!("Expected InterruptWithoutCheckpointer, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_interrupt_requires_thread_id() {
    // Verify error when interrupt configured without thread_id

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_edge("generate", "review")
        .add_edge("review", END)
        .set_entry_point("generate");

    // Has checkpointer but no thread_id
    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_interrupt_before(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    let result = app.invoke(initial_state).await;

    // Should error because interrupt requires thread_id
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::InterruptWithoutThreadId(node) => {
            assert_eq!(node, "review");
        }
        other => panic!("Expected InterruptWithoutThreadId, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_multiple_interrupts() {
    // Verify graph can have multiple interrupt points in sequence

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("approval", |state: ReviewState| {
            Box::pin(approval_node(state))
        })
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "approval")
        .add_edge("approval", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-7")
        .with_interrupt_before(vec!["review", "approval"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // First interrupt at "review"
    let result1 = app.invoke(initial_state).await.unwrap();
    assert_eq!(result1.interrupted_at, Some("review".to_string()));
    assert!(result1.final_state.generated);
    assert!(!result1.final_state.reviewed);

    // Resume - should hit second interrupt at "approval"
    let result2 = app.resume().await.unwrap();
    assert_eq!(result2.interrupted_at, Some("approval".to_string()));
    assert!(result2.final_state.reviewed);
    assert!(!result2.final_state.finalized);

    // Resume again - should complete
    let result3 = app.resume().await.unwrap();
    assert_eq!(result3.interrupted_at, None);
    assert!(result3.final_state.finalized);
}

#[tokio::test]
async fn test_interrupt_with_conditional_edges() {
    // Verify interrupts work correctly with conditional routing

    // Conditional routing function
    fn route_decision(state: &ReviewState) -> String {
        if state.approved {
            "finalize".to_string()
        } else {
            "review".to_string() // Loop back for re-review
        }
    }

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_conditional_edges(
            "review",
            route_decision,
            [
                ("finalize".to_string(), "finalize".to_string()),
                ("review".to_string(), "review".to_string()),
            ]
            .into_iter()
            .collect(),
        )
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-8")
        .with_interrupt_after(vec!["review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false, // Not approved initially
        finalized: false,
        reviewer_comments: None,
    };

    // First execution - interrupt after review
    let result1 = app.invoke(initial_state).await.unwrap();
    assert_eq!(result1.interrupted_at, Some("review".to_string()));
    assert!(!result1.final_state.approved);

    // Update to reject (should loop back to review)
    app.update_state(|mut state| {
        state.approved = false;
        state
    })
    .await
    .unwrap();

    // Resume - should loop back to review and interrupt again
    let result2 = app.resume().await.unwrap();
    assert_eq!(result2.interrupted_at, Some("review".to_string()));

    // Now approve
    app.update_state(|mut state| {
        state.approved = true;
        state
    })
    .await
    .unwrap();

    // Resume - should go to finalize and complete
    let result3 = app.resume().await.unwrap();
    assert_eq!(result3.interrupted_at, None);
    assert!(result3.final_state.finalized);
}

#[tokio::test]
async fn test_get_current_state() {
    // Verify get_current_state() returns the latest checkpoint state

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_edge("generate", "review")
        .add_edge("review", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-9")
        .with_interrupt_after(vec!["generate"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // Invoke - interrupt after generate
    app.invoke(initial_state).await.unwrap();

    // Get current state
    let current_state = app.get_current_state().await.unwrap();
    assert!(current_state.generated);
    assert!(!current_state.reviewed);
    assert_eq!(current_state.content, "Draft content generated");
}

#[tokio::test]
async fn test_resume_errors() {
    // Verify resume() errors appropriately

    // Test 1: Resume without checkpointer
    let mut graph1 = GraphBuilder::new();
    graph1
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_edge("generate", END)
        .set_entry_point("generate");

    // Explicitly disable checkpointer (default is MemoryCheckpointer)
    let app_no_checkpointer = graph1.compile().unwrap().without_checkpointing();
    let result = app_no_checkpointer.resume().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::ResumeWithoutCheckpointer => {}
        other => panic!("Expected ResumeWithoutCheckpointer, got: {:?}", other),
    }

    // Test 2: Resume without thread_id
    let mut graph2 = GraphBuilder::new();
    graph2
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_edge("generate", END)
        .set_entry_point("generate");

    let app_no_thread = graph2
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new());
    let result = app_no_thread.resume().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::ResumeWithoutThreadId => {}
        other => panic!("Expected ResumeWithoutThreadId, got: {:?}", other),
    }

    // Test 3: Resume when no checkpoint exists
    let mut graph3 = GraphBuilder::new();
    graph3
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_edge("generate", END)
        .set_entry_point("generate");

    let app_no_checkpoint = graph3
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("no-checkpoint-thread");
    let result = app_no_checkpoint.resume().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::NoCheckpointToResume(thread_id) => {
            assert_eq!(thread_id, "no-checkpoint-thread");
        }
        other => panic!("Expected NoCheckpointToResume, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_multiple_interrupt_resume_cycles() {
    // Verify multiple interrupt/resume cycles work correctly

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-11")
        .with_interrupt_after(vec!["generate", "review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // Cycle 1: Interrupt after generate
    let result1 = app.invoke(initial_state).await.unwrap();
    assert_eq!(result1.interrupted_at, Some("generate".to_string()));
    assert!(result1.final_state.generated);
    assert!(!result1.final_state.reviewed);

    // Cycle 2: Resume, interrupt after review
    let result2 = app.resume().await.unwrap();
    assert_eq!(result2.interrupted_at, Some("review".to_string()));
    assert!(result2.final_state.reviewed);
    assert!(!result2.final_state.finalized);

    // Cycle 3: Resume, complete
    let result3 = app.resume().await.unwrap();
    assert_eq!(result3.interrupted_at, None);
    assert!(result3.final_state.finalized);
}

#[tokio::test]
async fn test_state_updates_between_interrupts() {
    // Verify state can be updated multiple times between different interrupts

    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("generate", |state: ReviewState| {
            Box::pin(generate_node(state))
        })
        .add_node_from_fn("review", |state: ReviewState| Box::pin(review_node(state)))
        .add_node_from_fn("finalize", |state: ReviewState| {
            Box::pin(finalize_node(state))
        })
        .add_edge("generate", "review")
        .add_edge("review", "finalize")
        .add_edge("finalize", END)
        .set_entry_point("generate");

    let app = graph
        .compile()
        .unwrap()
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("test-thread-12")
        .with_interrupt_after(vec!["generate", "review"]);

    let initial_state = ReviewState {
        content: String::new(),
        generated: false,
        reviewed: false,
        approved: false,
        finalized: false,
        reviewer_comments: None,
    };

    // First interrupt
    app.invoke(initial_state).await.unwrap();

    // First state update
    app.update_state(|mut state| {
        state.reviewer_comments = Some("Initial review".to_string());
        state
    })
    .await
    .unwrap();

    // Resume to second interrupt
    app.resume().await.unwrap();

    // Second state update
    app.update_state(|mut state| {
        state.approved = true;
        state.reviewer_comments = Some("Final approval".to_string());
        state
    })
    .await
    .unwrap();

    // Resume to completion
    let final_result = app.resume().await.unwrap();

    // Verify all updates persisted
    assert!(final_result.final_state.approved);
    assert_eq!(
        final_result.final_state.reviewer_comments,
        Some("Final approval".to_string())
    );
}
