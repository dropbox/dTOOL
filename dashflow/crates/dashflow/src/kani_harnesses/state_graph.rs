//! Kani harnesses for StateGraph state transitions (KANI-002).
//!
//! These harnesses verify that StateGraph operations do not panic
//! under symbolic inputs and that invariants are maintained.

use crate::graph::StateGraph;
use serde::{Deserialize, Serialize};

/// Minimal state type for Kani verification.
/// Uses bounded fields to keep state space manageable.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct MinimalState {
    counter: u8,
}

/// Verify that creating a new StateGraph does not panic.
#[kani::proof]
fn proof_state_graph_new_no_panic() {
    let graph: StateGraph<MinimalState> = StateGraph::new();
    // Verify initial state - use count() since node_names() returns iterator
    assert_eq!(graph.node_names().count(), 0);
    assert!(graph.get_entry_point().is_none());
}

/// Verify that set_entry_point with empty string does not panic.
#[kani::proof]
fn proof_state_graph_set_entry_point_empty() {
    let mut graph: StateGraph<MinimalState> = StateGraph::new();
    // Empty string entry point should be accepted (even if invalid for execution)
    graph.set_entry_point("");
    assert_eq!(graph.get_entry_point(), Some(""));
}

/// Verify that set_entry_point with a single character does not panic.
#[kani::proof]
fn proof_state_graph_set_entry_point_char() {
    let mut graph: StateGraph<MinimalState> = StateGraph::new();

    // Generate a single lowercase letter
    let c: u8 = kani::any();
    kani::assume(c >= b'a' && c <= b'z');
    let name = (c as char).to_string();

    graph.set_entry_point(&name);
    assert_eq!(graph.get_entry_point(), Some(name.as_str()));
}

/// Verify that clone produces equivalent graph.
#[kani::proof]
fn proof_state_graph_clone_equivalent() {
    let mut graph: StateGraph<MinimalState> = StateGraph::new();

    // Set up some state
    let entry: u8 = kani::any();
    kani::assume(entry >= b'a' && entry <= b'z');
    let entry_name = (entry as char).to_string();
    graph.set_entry_point(&entry_name);

    // Clone and verify equivalence
    let cloned = graph.clone();
    assert_eq!(cloned.get_entry_point(), graph.get_entry_point());
    assert_eq!(cloned.node_names().count(), graph.node_names().count());
}

/// Verify that enabling strict mode does not panic.
#[kani::proof]
fn proof_state_graph_strict_mode() {
    let graph: StateGraph<MinimalState> = StateGraph::new();
    // Toggle strict mode - should never panic
    // strict() takes ownership and returns Self
    let strict: bool = kani::any();
    let graph = if strict { graph.strict() } else { graph };
    // Graph should still be usable
    assert_eq!(graph.node_names().count(), 0);
}
