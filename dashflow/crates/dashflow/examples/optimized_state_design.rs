//! Optimized State Design Examples
//!
//! This example demonstrates performance-optimized state design patterns
//! for DashFlow. State cloning overhead can be 50%+ of execution time
//! when using inefficient data structures.
//!
//! Run with: cargo run --example optimized_state_design --release

use async_trait::async_trait;
use dashflow::{MergeableState, Node, Result, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Example 1: Vec vs HashMap Performance
// ============================================================================

/// GOOD: Vec for sequential message history (34% cloning overhead)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OptimizedState {
    messages: Vec<String>, // Sequential access, cache-friendly
    current_step: usize,
}

impl MergeableState for OptimizedState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.current_step = self.current_step.max(other.current_step);
    }
}

/// SLOW: HashMap for message history (53% cloning overhead)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SuboptimalState {
    messages: HashMap<usize, String>, // Many heap allocations
    current_step: usize,
}

impl MergeableState for SuboptimalState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.current_step = self.current_step.max(other.current_step);
    }
}

struct AddMessageNode {
    message: String,
}

#[async_trait]
impl Node<OptimizedState> for AddMessageNode {
    async fn execute(&self, mut state: OptimizedState) -> Result<OptimizedState> {
        state.messages.push(self.message.clone());
        state.current_step += 1;
        Ok(state)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[async_trait]
impl Node<SuboptimalState> for AddMessageNode {
    async fn execute(&self, mut state: SuboptimalState) -> Result<SuboptimalState> {
        state
            .messages
            .insert(state.current_step, self.message.clone());
        state.current_step += 1;
        Ok(state)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Example 2: Arc for Large Read-Only Data
// ============================================================================

/// OPTIMAL: Arc for large read-only reference data (<1% cloning overhead)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ArcOptimizedState {
    messages: Vec<String>,
    #[serde(skip)] // Don't serialize Arc (reconstruct on load)
    document_corpus: Arc<Vec<String>>, // Shared, no clone cost
}

impl MergeableState for ArcOptimizedState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.document_corpus = Arc::clone(&other.document_corpus);
    }
}

struct SearchDocumentsNode;

#[async_trait]
impl Node<ArcOptimizedState> for SearchDocumentsNode {
    async fn execute(&self, mut state: ArcOptimizedState) -> Result<ArcOptimizedState> {
        // Search documents (read-only access, no clone)
        let query = state.messages.last().unwrap_or(&"".to_string()).clone();
        let results = state
            .document_corpus
            .iter()
            .filter(|doc| doc.contains(&query))
            .take(3)
            .cloned()
            .collect::<Vec<_>>();

        // Add results to messages
        state
            .messages
            .push(format!("Found {} results", results.len()));
        Ok(state)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Example 3: Minimal State (Only Essential Data)
// ============================================================================

/// GOOD: Minimal state with only decision-making data
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MinimalState {
    messages: Vec<String>,
    next_action: String,
}

impl MergeableState for MinimalState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        if !other.next_action.is_empty() {
            self.next_action = other.next_action.clone();
        }
    }
}

/// AVOID: Bloated state with derived and debug data
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BloatedState {
    messages: Vec<String>,
    all_tool_results: HashMap<String, String>, // Can be derived from messages
    thinking_log: Vec<String>,                 // Debug-only data
    intermediate_states: Vec<MinimalState>,    // Recursive overhead (expensive!)
}

impl MergeableState for BloatedState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.all_tool_results.extend(other.all_tool_results.clone());
        self.thinking_log.extend(other.thinking_log.clone());
        self.intermediate_states
            .extend(other.intermediate_states.clone());
    }
}

// ============================================================================
// Benchmarking Examples
// ============================================================================

async fn benchmark_vec_vs_hashmap() -> Result<()> {
    println!("\n=== Vec vs HashMap Performance ===\n");

    // Build optimized graph (Vec)
    let mut optimized_graph = StateGraph::<OptimizedState>::new();
    optimized_graph.add_node(
        "step1",
        AddMessageNode {
            message: "Message 1".to_string(),
        },
    );
    optimized_graph.add_node(
        "step2",
        AddMessageNode {
            message: "Message 2".to_string(),
        },
    );
    optimized_graph.add_node(
        "step3",
        AddMessageNode {
            message: "Message 3".to_string(),
        },
    );
    optimized_graph.add_edge("step1", "step2");
    optimized_graph.add_edge("step2", "step3");
    optimized_graph.add_edge("step3", END);
    optimized_graph.set_entry_point("step1");
    let optimized_app = optimized_graph.compile()?;

    // Build suboptimal graph (HashMap)
    let mut suboptimal_graph = StateGraph::<SuboptimalState>::new();
    suboptimal_graph.add_node(
        "step1",
        AddMessageNode {
            message: "Message 1".to_string(),
        },
    );
    suboptimal_graph.add_node(
        "step2",
        AddMessageNode {
            message: "Message 2".to_string(),
        },
    );
    suboptimal_graph.add_node(
        "step3",
        AddMessageNode {
            message: "Message 3".to_string(),
        },
    );
    suboptimal_graph.add_edge("step1", "step2");
    suboptimal_graph.add_edge("step2", "step3");
    suboptimal_graph.add_edge("step3", END);
    suboptimal_graph.set_entry_point("step1");
    let suboptimal_app = suboptimal_graph.compile()?;

    // Benchmark optimized (Vec)
    let optimized_state = OptimizedState {
        messages: vec![],
        current_step: 0,
    };
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = optimized_app
            .invoke(optimized_state.clone())
            .await?
            .final_state;
    }
    let optimized_time = start.elapsed();

    // Benchmark suboptimal (HashMap)
    let suboptimal_state = SuboptimalState {
        messages: HashMap::new(),
        current_step: 0,
    };
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = suboptimal_app
            .invoke(suboptimal_state.clone())
            .await?
            .final_state;
    }
    let suboptimal_time = start.elapsed();

    println!("Optimized (Vec):     {:?}", optimized_time);
    println!("Suboptimal (HashMap): {:?}", suboptimal_time);
    println!(
        "Speedup: {:.2}× faster",
        suboptimal_time.as_secs_f64() / optimized_time.as_secs_f64()
    );
    println!("\nVec is faster due to:");
    println!("  - Single contiguous memory allocation");
    println!("  - Cache-friendly access patterns");
    println!("  - No bucket iteration or rehashing on clone");

    Ok(())
}

async fn demonstrate_arc_usage() -> Result<()> {
    println!("\n=== Arc for Large Read-Only Data ===\n");

    // Create large document corpus (shared across all node executions)
    let documents = Arc::new(vec![
        "Rust is a systems programming language".to_string(),
        "DashFlow enables multi-agent workflows".to_string(),
        "Performance optimization requires profiling".to_string(),
        "Vec is faster than HashMap for sequential data".to_string(),
        "Arc provides zero-cost cloning for shared data".to_string(),
    ]);

    // Build graph
    let mut graph = StateGraph::<ArcOptimizedState>::new();
    graph.add_node("search", SearchDocumentsNode);
    graph.add_edge("search", END);
    graph.set_entry_point("search");
    let app = graph.compile()?;

    // Run with different queries
    let queries = vec!["Rust", "DashFlow", "performance"];

    for query in queries {
        let state = ArcOptimizedState {
            messages: vec![query.to_string()],
            document_corpus: Arc::clone(&documents), // Arc clone is cheap (pointer copy)
        };

        let result = app.invoke(state).await?;
        println!("Query: {}", query);
        if let Some(last) = result.final_state.messages.last() {
            println!("Result: {last}");
        }
        println!();
    }

    println!("Arc benefits:");
    println!("  - Large data (100+ KB) shared without copying");
    println!("  - Clone only copies Arc pointer (8 bytes)");
    println!("  - <1% cloning overhead vs 50%+ for HashMap");
    let serde_attr = "skip";
    println!(
        "  - Note: Use #[serde({})] and reconstruct on deserialization",
        serde_attr
    );

    Ok(())
}

fn demonstrate_minimal_state() -> Result<()> {
    println!("\n=== Minimal State Best Practices ===\n");

    println!("GOOD: Minimal state");
    println!(
        "{}",
        serde_json::to_string_pretty(&MinimalState {
            messages: vec!["Hello".to_string()],
            next_action: "continue".to_string(),
        })?
    );

    println!("\nAVOID: Bloated state");
    let mut bloated = BloatedState {
        messages: vec!["Hello".to_string()],
        all_tool_results: HashMap::new(),
        thinking_log: vec!["Thinking...".to_string()],
        intermediate_states: vec![],
    };
    bloated
        .all_tool_results
        .insert("search".to_string(), "result".to_string());
    println!("{}", serde_json::to_string_pretty(&bloated)?);

    println!("\nMinimal state principles:");
    println!("  - Only store data needed for decision-making");
    println!("  - Derive computed fields in node functions");
    let serde_attr = "skip";
    println!("  - Use #[serde({})] for debug/logging fields", serde_attr);
    println!("  - Avoid recursive nesting of state (expensive clones)");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", "=".repeat(70));
    println!("Optimized State Design Examples");
    println!("{}", "=".repeat(70));

    println!("\nThese examples show how state design affects DashFlow performance.");
    println!("State is cloned between each node, so efficient data structures matter!");

    benchmark_vec_vs_hashmap().await?;
    demonstrate_arc_usage().await?;
    demonstrate_minimal_state()?;

    println!("\n=== Summary ===\n");
    println!("1. Use Vec for sequential collections → 34% overhead (good)");
    println!("2. Use Arc for large read-only data → <1% overhead (optimal)");
    println!("3. Avoid HashMap for large collections → 53% overhead (slow)");
    println!("4. Keep state minimal → only decision-making data");
    println!("\nSee docs/ARCHITECTURE.md for detailed benchmarks and trade-offs.");

    Ok(())
}
