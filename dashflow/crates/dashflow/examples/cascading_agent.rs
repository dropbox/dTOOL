// Multi-Model Cascade Agent Example
//
// Demonstrates INNOVATION 8: Multi-Model Cascade
//
// PROBLEM: Premium models are expensive. Using them for every query wastes money
// on simple questions that cheaper models can handle.
//
// SOLUTION: Start with fast/cheap model. If quality is insufficient, automatically
// cascade to more expensive models. Only pay for premium when needed.
//
// GRAPH ARCHITECTURE (with conditional cascading):
//
// START → fast_model → judge_fast → [conditional router]
//                          ↓              ↓
//                     (score ≥0.95)  High (≥0.95) → END
//                                         ↓
//                                    Mid (≥0.85) → refine → END
//                                         ↓
//                                    Low (<0.85) → premium_model → judge_premium → END

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CascadeState {
    // Input
    query: String,
    query_complexity: String, // "simple", "medium", "complex"

    // Model tracking
    models_tried: Vec<String>,
    current_model: String,

    // Response
    response: String,
    quality_score: f64,

    // Costs (mock pricing)
    total_cost: f64,

    // Metadata
    final_verdict: String,
}

impl MergeableState for CascadeState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        if !other.query_complexity.is_empty() {
            if self.query_complexity.is_empty() {
                self.query_complexity = other.query_complexity.clone();
            } else {
                self.query_complexity.push('\n');
                self.query_complexity.push_str(&other.query_complexity);
            }
        }
        self.models_tried.extend(other.models_tried.clone());
        if !other.current_model.is_empty() {
            if self.current_model.is_empty() {
                self.current_model = other.current_model.clone();
            } else {
                self.current_model.push('\n');
                self.current_model.push_str(&other.current_model);
            }
        }
        if !other.response.is_empty() {
            if self.response.is_empty() {
                self.response = other.response.clone();
            } else {
                self.response.push('\n');
                self.response.push_str(&other.response);
            }
        }
        self.quality_score = self.quality_score.max(other.quality_score);
        self.total_cost = self.total_cost.max(other.total_cost);
        if !other.final_verdict.is_empty() {
            if self.final_verdict.is_empty() {
                self.final_verdict = other.final_verdict.clone();
            } else {
                self.final_verdict.push('\n');
                self.final_verdict.push_str(&other.final_verdict);
            }
        }
    }
}

// Node 1: Try fast model first (gpt-4o-mini)
fn fast_model_node(
    mut state: CascadeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CascadeState>> + Send>> {
    Box::pin(async move {
        println!("\n[FAST MODEL] Using gpt-4o-mini (cheap, fast)");
        println!("[FAST MODEL] Query: '{}'", state.query);

        state.current_model = "gpt-4o-mini".to_string();
        state.models_tried.push("gpt-4o-mini".to_string());

        // Simulate model response based on query complexity
        state.response = match state.query_complexity.as_str() {
            "simple" => {
                // Fast model excels at simple queries
                format!(
                    "Detailed answer to '{}' - fast model succeeded!",
                    state.query
                )
            }
            "medium" => {
                // Fast model gives partial answer
                format!("Partial answer to '{}' - some details missing", state.query)
            }
            "complex" => {
                // Fast model struggles with complex queries
                format!(
                    "Basic answer to '{}' - lacks depth and accuracy",
                    state.query
                )
            }
            _ => "Unknown complexity".to_string(),
        };

        // Mock cost: $0.50 per 1M tokens (very cheap)
        state.total_cost += 0.0005;

        println!("[FAST MODEL] ✅ Generated response (cost: $0.0005)");

        Ok(state)
    })
}

// Node 2: Judge fast model response
fn judge_fast_node(
    mut state: CascadeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CascadeState>> + Send>> {
    Box::pin(async move {
        println!("\n[JUDGE] Evaluating gpt-4o-mini response");

        // Simulate quality scoring based on complexity
        state.quality_score = match state.query_complexity.as_str() {
            "simple" => 0.98,  // Excellent for simple queries
            "medium" => 0.88,  // Acceptable but could be better
            "complex" => 0.65, // Poor for complex queries
            _ => 0.5,
        };

        println!(
            "[JUDGE] Quality score: {:.2} (complexity: {})",
            state.quality_score, state.query_complexity
        );

        Ok(state)
    })
}

// Node 3: Refine response (for "close enough" responses)
fn refine_response_node(
    mut state: CascadeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CascadeState>> + Send>> {
    Box::pin(async move {
        println!("\n[REFINE] Improving response with same model");

        // Simulate refinement
        state.response = format!(
            "{}\n\n[Refined with additional context and details]",
            state.response
        );
        state.quality_score = 0.96; // Refinement boosts quality

        // Mock cost: small additional cost
        state.total_cost += 0.0003;

        println!("[REFINE] ✅ Enhanced response (cost: $0.0003)");

        Ok(state)
    })
}

// Node 4: Try premium model (gpt-4)
fn premium_model_node(
    mut state: CascadeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CascadeState>> + Send>> {
    Box::pin(async move {
        println!("\n[PREMIUM MODEL] Cascading to gpt-4 (expensive, powerful)");

        state.current_model = "gpt-4".to_string();
        state.models_tried.push("gpt-4".to_string());

        // Premium model handles complex queries well
        state.response = format!(
            "Comprehensive, accurate answer to '{}'\n\
             - Deep analysis with citations\n\
             - Multiple perspectives considered\n\
             - Expert-level accuracy",
            state.query
        );

        // Mock cost: $30 per 1M tokens (60x more expensive!)
        state.total_cost += 0.030;

        println!("[PREMIUM MODEL] ✅ Generated high-quality response (cost: $0.030)");

        Ok(state)
    })
}

// Node 5: Judge premium model response
fn judge_premium_node(
    mut state: CascadeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CascadeState>> + Send>> {
    Box::pin(async move {
        println!("\n[JUDGE] Evaluating gpt-4 response");

        // Premium model consistently delivers high quality
        state.quality_score = 0.98;

        println!("[JUDGE] Quality score: {:.2}", state.quality_score);

        Ok(state)
    })
}

// Conditional router after fast model
fn route_after_fast(state: &CascadeState) -> String {
    println!("\n[ROUTER] Fast model score: {:.2}", state.quality_score);

    if state.quality_score >= 0.95 {
        // Excellent quality - use fast model response
        println!("[ROUTER] ✅ High quality → END (saved money!)");
        END.to_string()
    } else if state.quality_score >= 0.85 {
        // Good but could be better - try refining
        println!("[ROUTER] 🔧 Good quality → REFINE");
        "refine".to_string()
    } else {
        // Poor quality - cascade to premium model
        println!("[ROUTER] ⬆️  Low quality → CASCADE TO PREMIUM");
        "premium_model".to_string()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multi-Model Cascade Agent Example ===");
    println!("\nDemonstrates INNOVATION 8: Multi-Model Cascade");
    println!("\nKey concept: Start cheap, cascade to expensive only when needed");
    println!("This optimizes cost vs quality automatically\n");

    // Test 3 scenarios
    let scenarios = vec![
        ("simple", "What is 2+2?"),
        ("medium", "Explain how HTTP works"),
        (
            "complex",
            "Design a distributed consensus algorithm with Byzantine fault tolerance",
        ),
    ];

    for (complexity, query) in scenarios {
        println!("\n\n{}", "=".repeat(80));
        println!("TEST: {} query", complexity.to_uppercase());
        println!("{}", "=".repeat(80));

        // Create state graph
        let mut graph = StateGraph::<CascadeState>::new();

        // Add nodes
        graph.add_node_from_fn("fast_model", fast_model_node);
        graph.add_node_from_fn("judge_fast", judge_fast_node);
        graph.add_node_from_fn("refine", refine_response_node);
        graph.add_node_from_fn("premium_model", premium_model_node);
        graph.add_node_from_fn("judge_premium", judge_premium_node);

        // Add edges
        graph.set_entry_point("fast_model");
        graph.add_edge("fast_model", "judge_fast");

        // Conditional routing after fast model
        let mut routes_fast = HashMap::new();
        routes_fast.insert(END.to_string(), END.to_string());
        routes_fast.insert("refine".to_string(), "refine".to_string());
        routes_fast.insert("premium_model".to_string(), "premium_model".to_string());
        graph.add_conditional_edges("judge_fast", route_after_fast, routes_fast);

        // Refine goes to END
        graph.add_edge("refine", END);

        // Premium model path
        graph.add_edge("premium_model", "judge_premium");
        graph.add_edge("judge_premium", END);

        // Compile
        let app = graph.compile()?;

        // Run
        let runtime = tokio::runtime::Runtime::new()?;
        let result = runtime.block_on(async {
            let initial_state = CascadeState {
                query: query.to_string(),
                query_complexity: complexity.to_string(),
                ..Default::default()
            };

            app.invoke(initial_state).await
        })?;

        let final_state = result.final_state;

        // Print results
        println!("\n{}", "─".repeat(80));
        println!("FINAL RESULT");
        println!("{}", "─".repeat(80));
        println!("Query: {}", final_state.query);
        println!("Complexity: {}", final_state.query_complexity);
        println!("Models tried: {:?}", final_state.models_tried);
        println!("Final model: {}", final_state.current_model);
        println!("Quality score: {:.2}", final_state.quality_score);
        println!("Total cost: ${:.4}", final_state.total_cost);
        println!("\nResponse:\n{}", final_state.response);

        // Verdict
        if final_state.models_tried.len() == 1 && final_state.current_model == "gpt-4o-mini" {
            println!("\n💰 COST SAVINGS: Fast model was sufficient!");
            println!(
                "   Saved: ${:.4} (vs always using gpt-4)",
                0.030 - final_state.total_cost
            );
        } else if final_state.models_tried.contains(&"gpt-4".to_string()) {
            println!("\n⬆️  CASCADED: Complex query required premium model");
            println!("   Automatic quality guarantee through escalation");
        }
    }

    // Summary
    println!("\n\n{}", "═".repeat(80));
    println!("SUMMARY: Multi-Model Cascade Benefits");
    println!("{}", "═".repeat(80));
    println!("✅ Simple queries → Fast model (90% of queries) → 60x cost savings");
    println!("✅ Complex queries → Automatic cascade to premium → Quality guarantee");
    println!("✅ Medium queries → Refinement with fast model → Balance cost/quality");
    println!("✅ No manual routing needed → Graph architecture handles it");
    println!("\n💡 Result: 90% cost savings while maintaining 100% quality threshold");

    Ok(())
}
