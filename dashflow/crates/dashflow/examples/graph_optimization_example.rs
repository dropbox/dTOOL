//! Multi-Node Graph Optimization Example
//!
//! This example demonstrates the concept of graph-level optimization.
//! GraphOptimizer jointly optimizes multiple LLM nodes by evaluating
//! the entire workflow end-to-end, rather than optimizing each node
//! independently.
//!
//! Why Graph-Level Optimization:
//! - Sequential optimization optimizes nodes for local tasks, not global success
//! - Earlier nodes affect downstream performance
//! - Joint optimization considers these dependencies
//!
//! This is a simplified conceptual example. For production use, see the
//! integration tests in crates/dashflow/tests/dashoptimize_tests.rs
//!
//! Run with: cargo run --package dashflow --example graph_optimization_example

use dashflow::optimize::OptimizationStrategy;
use serde::{Deserialize, Serialize};

/// Example state for multi-step customer support workflow
/// Note: This is a simplified example. In production, SupportState would need
/// to implement GraphState and MergeableState traits properly.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SupportState {
    // Input
    query: String,

    // Intermediate outputs
    category: Option<String>,
    entities: Option<Vec<String>>,

    // Final output
    response: Option<String>,
}

fn main() -> dashflow::Result<()> {
    println!("=== Multi-Node Graph Optimization - Concept Demo ===\n");

    // 1. Explain the workflow
    println!("=== Three-Node Support Workflow ===\n");
    println!("Node 1 (classify): query → category");
    println!("  ↓");
    println!("Node 2 (extract): query, category → entities");
    println!("  ↓");
    println!("Node 3 (respond): query, entities → response\n");

    // 2. Show training data
    let _trainset = [
        SupportState {
            query: "I can't log in to my account".to_string(),
            category: Some("authentication".to_string()),
            entities: Some(vec!["login".to_string(), "account".to_string()]),
            response: Some("Let me help you reset your password...".to_string()),
        },
        SupportState {
            query: "My order hasn't arrived".to_string(),
            category: Some("shipping".to_string()),
            entities: Some(vec!["order".to_string(), "delivery".to_string()]),
            response: Some("I'll check your order status...".to_string()),
        },
    ];
    println!("✓ Prepared training set: 2 examples\n");

    // 3. Show optimization strategies
    println!("=== Optimization Strategies ===\n");

    let _sequential = OptimizationStrategy::Sequential;
    println!("1. Sequential (default):");
    println!("   - Optimize nodes in dependency order: classify → extract → respond");
    println!("   - Fastest, good for linear pipelines\n");

    let _joint = OptimizationStrategy::Joint;
    println!("2. Joint:");
    println!("   - Optimize all nodes simultaneously");
    println!("   - Best for complex node interactions\n");

    let _alternating = OptimizationStrategy::Alternating;
    println!("3. Alternating:");
    println!("   - Multiple rounds of sequential optimization");
    println!("   - Best quality, slower\n");

    // 4. Show optimizer concept (actual creation requires trait implementations)
    println!("=== GraphOptimizer Concept ===\n");
    println!("Creating optimizer:");
    println!("  GraphOptimizer::<SupportState>::new()");
    println!("    .with_strategy(Sequential)");
    println!("    .with_global_metric(|initial, final| {{ ... }})");
    println!("    .optimize(graph, trainset)\n");

    println!("Global metric evaluates entire workflow:");
    println!("  fn global_metric(initial: &State, final: &State) -> f64 {{");
    println!("    // Measure response quality, accuracy, completeness");
    println!("    evaluate_response(&final.response)");
    println!("  }}\n");

    // 5. Explain how it works
    println!("=== How Graph Optimization Works ===\n");

    println!("1. Run entire graph on training examples");
    println!("   → query → classify → extract → respond → final state\n");

    println!("2. Evaluate with global metric");
    println!("   → global_metric(initial_state, final_state)");
    println!("   → Score based on final response quality\n");

    println!("3. Optimize nodes using strategy");
    println!("   Sequential: optimize classify → extract → respond");
    println!("   Joint: optimize all nodes together");
    println!("   Alternating: multiple rounds of sequential\n");

    println!("4. Return optimized graph");
    println!("   → All nodes improved for end-to-end performance");
    println!("   → Better global quality than node-by-node optimization\n");

    // 6. Show key concepts
    println!("=== Key Concepts ===\n");

    println!("Global Metric:");
    println!("  - Evaluates entire workflow (initial state → final state)");
    println!("  - Examples: response accuracy, user satisfaction, latency");
    println!("  - Drives optimization toward business goals\n");

    println!("Strategy Selection:");
    println!("  - Sequential: Linear pipelines (A → B → C)");
    println!("  - Joint: Complex node interactions");
    println!("  - Alternating: Best quality, longer optimization time\n");

    println!("=== Production Usage ===\n");
    println!("For full working examples with actual graphs and LLMs, see:");
    println!("• crates/dashflow/tests/dashoptimize_tests.rs");
    println!("• docs/DASHOPTIMIZE_GUIDE.md (complete documentation)");
    println!("\nThis conceptual example shows graph optimization structure without");
    println!("requiring API keys or building actual DashFlow workflows.");

    Ok(())
}

// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
