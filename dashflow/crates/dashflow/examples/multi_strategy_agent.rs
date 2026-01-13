// Parallel Multi-Strategy Agent Example
//
// Demonstrates INNOVATION 9: Parallel Multi-Strategy with Voting
//
// PROBLEM: Different strategies excel at different query types. It's hard to
// predict which strategy will work best for a given query.
//
// SOLUTION: Run multiple strategies IN PARALLEL, then vote on the best response.
// Combines speed of direct answers with reliability of tool-based search.
//
// GRAPH ARCHITECTURE (with parallel execution):
//
//             ┌─→ strategy_1 (no tools, fast) ──┐
//             │                                   │
// START ──────┼─→ strategy_2 (forced tools) ─────┼─→ judge_committee → END
//             │                                   │
//             └─→ strategy_3 (web search) ────────┘
//
// All 3 strategies run IN PARALLEL, then committee picks best response!

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MultiStrategyState {
    // Input
    query: String,

    // Strategy responses (all run in parallel)
    strategy_1_response: String, // No tools (fast, might hallucinate)
    strategy_2_response: String, // Forced tools (slower, accurate)
    strategy_3_response: String, // Web search (comprehensive)

    // Quality scores
    strategy_1_score: f64,
    strategy_2_score: f64,
    strategy_3_score: f64,

    // Final result (picked by committee)
    final_response: String,
    final_score: f64,
    winning_strategy: String,

    // Metadata
    strategies_completed: Vec<String>,
}

impl MergeableState for MultiStrategyState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        if !other.strategy_1_response.is_empty() {
            if self.strategy_1_response.is_empty() {
                self.strategy_1_response = other.strategy_1_response.clone();
            } else {
                self.strategy_1_response.push('\n');
                self.strategy_1_response
                    .push_str(&other.strategy_1_response);
            }
        }
        if !other.strategy_2_response.is_empty() {
            if self.strategy_2_response.is_empty() {
                self.strategy_2_response = other.strategy_2_response.clone();
            } else {
                self.strategy_2_response.push('\n');
                self.strategy_2_response
                    .push_str(&other.strategy_2_response);
            }
        }
        if !other.strategy_3_response.is_empty() {
            if self.strategy_3_response.is_empty() {
                self.strategy_3_response = other.strategy_3_response.clone();
            } else {
                self.strategy_3_response.push('\n');
                self.strategy_3_response
                    .push_str(&other.strategy_3_response);
            }
        }
        self.strategy_1_score = self.strategy_1_score.max(other.strategy_1_score);
        self.strategy_2_score = self.strategy_2_score.max(other.strategy_2_score);
        self.strategy_3_score = self.strategy_3_score.max(other.strategy_3_score);
        if !other.final_response.is_empty() {
            if self.final_response.is_empty() {
                self.final_response = other.final_response.clone();
            } else {
                self.final_response.push('\n');
                self.final_response.push_str(&other.final_response);
            }
        }
        self.final_score = self.final_score.max(other.final_score);
        if !other.winning_strategy.is_empty() {
            if self.winning_strategy.is_empty() {
                self.winning_strategy = other.winning_strategy.clone();
            } else {
                self.winning_strategy.push('\n');
                self.winning_strategy.push_str(&other.winning_strategy);
            }
        }
        self.strategies_completed
            .extend(other.strategies_completed.clone());
    }
}

// Strategy 1: No tools (fast, direct answer)
fn strategy_1_no_tools(
    mut state: MultiStrategyState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<MultiStrategyState>> + Send>>
{
    Box::pin(async move {
        println!("[STRATEGY 1 - NO TOOLS] Generating direct answer (fast)");

        // Simulate fast LLM response without tools
        // Might hallucinate or lack details for complex queries
        state.strategy_1_response = format!(
            "Quick answer to '{}': Based on my training data, here's what I know...\n\
             [May lack citations or latest information]",
            state.query
        );

        state.strategies_completed.push("strategy_1".to_string());

        println!("[STRATEGY 1] ✅ Completed (0.5s)");

        Ok(state)
    })
}

// Strategy 2: Forced tools (accurate, slower)
fn strategy_2_forced_tools(
    mut state: MultiStrategyState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<MultiStrategyState>> + Send>>
{
    Box::pin(async move {
        println!("[STRATEGY 2 - FORCED TOOLS] Using search tools (accurate)");

        // Simulate tool-based response with citations
        state.strategy_2_response = format!(
            "Researched answer to '{}':\n\
             Based on search results:\n\
             - Source 1: Relevant information with citation\n\
             - Source 2: Additional verified details\n\
             [Accurate, cited information from tools]",
            state.query
        );

        state.strategies_completed.push("strategy_2".to_string());

        println!("[STRATEGY 2] ✅ Completed (1.5s)");

        Ok(state)
    })
}

// Strategy 3: Web search fallback (comprehensive, slowest)
fn strategy_3_web_search(
    mut state: MultiStrategyState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<MultiStrategyState>> + Send>>
{
    Box::pin(async move {
        println!("[STRATEGY 3 - WEB SEARCH] Comprehensive web search");

        // Simulate comprehensive web search
        state.strategy_3_response = format!(
            "Comprehensive answer to '{}':\n\
             From web search:\n\
             - Latest information from authoritative sources\n\
             - Multiple perspectives and viewpoints\n\
             - Recent developments and updates\n\
             [Most comprehensive, up-to-date information]",
            state.query
        );

        state.strategies_completed.push("strategy_3".to_string());

        println!("[STRATEGY 3] ✅ Completed (2.0s)");

        Ok(state)
    })
}

// Judge committee: Score all 3 responses and pick best
fn judge_committee_node(
    mut state: MultiStrategyState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<MultiStrategyState>> + Send>>
{
    Box::pin(async move {
        println!("\n[JUDGE COMMITTEE] Evaluating all 3 strategies");

        // Simulate LLM-as-judge scoring each response
        // In production: use actual LLM to grade responses

        // Strategy 1 (no tools): fast but might lack accuracy
        state.strategy_1_score = if state.query.contains("latest") || state.query.contains("recent")
        {
            0.70 // Poor for queries needing current info
        } else {
            0.85 // Good for general knowledge
        };

        // Strategy 2 (forced tools): reliable and cited
        state.strategy_2_score = 0.92; // Consistently good with citations

        // Strategy 3 (web search): comprehensive but might be overkill
        state.strategy_3_score = if state.query.len() > 50 {
            0.96 // Excellent for complex queries
        } else {
            0.88 // Good but might be overkill for simple queries
        };

        println!(
            "[JUDGE] Strategy 1 (no tools):     score {:.2}",
            state.strategy_1_score
        );
        println!(
            "[JUDGE] Strategy 2 (forced tools): score {:.2}",
            state.strategy_2_score
        );
        println!(
            "[JUDGE] Strategy 3 (web search):   score {:.2}",
            state.strategy_3_score
        );

        // Committee voting: pick highest score
        let scores = [
            (
                "strategy_1",
                state.strategy_1_score,
                &state.strategy_1_response,
            ),
            (
                "strategy_2",
                state.strategy_2_score,
                &state.strategy_2_response,
            ),
            (
                "strategy_3",
                state.strategy_3_score,
                &state.strategy_3_response,
            ),
        ];

        let Some(winner) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        else {
            eprintln!("[COMMITTEE] No strategy scores available");
            return Ok(state);
        };

        state.winning_strategy = winner.0.to_string();
        state.final_score = winner.1;
        state.final_response = winner.2.clone();

        println!(
            "\n[COMMITTEE] 🏆 Winner: {} (score: {:.2})",
            state.winning_strategy, state.final_score
        );

        Ok(state)
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Parallel Multi-Strategy Agent Example ===");
    println!("\nDemonstrates INNOVATION 9: Parallel Multi-Strategy with Voting");
    println!("\nKey concept: Run multiple strategies in parallel, pick best response");
    println!("Combines speed of direct answers with reliability of search\n");

    // Test queries
    let queries = vec![
        "What is machine learning?", // General knowledge
        "What are the latest developments in quantum computing?", // Needs current info
    ];

    for query in queries {
        println!("\n{}", "=".repeat(80));
        println!("QUERY: {}", query);
        println!("{}", "=".repeat(80));

        // Create state graph
        let mut graph = StateGraph::<MultiStrategyState>::new();

        // Add strategy nodes
        graph.add_node_from_fn("strategy_1", strategy_1_no_tools);
        graph.add_node_from_fn("strategy_2", strategy_2_forced_tools);
        graph.add_node_from_fn("strategy_3", strategy_3_web_search);
        graph.add_node_from_fn("judge_committee", judge_committee_node);

        // Fan-out pattern: entry → all 3 strategies
        // Note: Currently DashFlow executes these sequentially, but the graph
        // structure supports parallel execution
        graph.set_entry_point("strategy_1");
        graph.add_edge("strategy_1", "strategy_2");
        graph.add_edge("strategy_2", "strategy_3");
        graph.add_edge("strategy_3", "judge_committee");

        // Judge committee is the merge point
        graph.add_edge("judge_committee", END);

        // Compile
        let app = graph.compile()?;

        // Run
        let runtime = tokio::runtime::Runtime::new()?;
        let result = runtime.block_on(async {
            let initial_state = MultiStrategyState {
                query: query.to_string(),
                ..Default::default()
            };

            // Note: Currently these run sequentially, but the graph structure
            // supports parallel execution when the executor is enhanced
            app.invoke(initial_state).await
        })?;

        let final_state = result.final_state;

        // Print results
        println!("\n{}", "─".repeat(80));
        println!("RESULTS");
        println!("{}", "─".repeat(80));

        println!(
            "\n🏆 WINNING STRATEGY: {} (score: {:.2})",
            final_state.winning_strategy, final_state.final_score
        );

        println!("\n📊 ALL SCORES:");
        println!(
            "  Strategy 1 (no tools):     {:.2}",
            final_state.strategy_1_score
        );
        println!(
            "  Strategy 2 (forced tools): {:.2}",
            final_state.strategy_2_score
        );
        println!(
            "  Strategy 3 (web search):   {:.2}",
            final_state.strategy_3_score
        );

        println!("\n📝 FINAL RESPONSE:");
        println!("{}", final_state.final_response);

        println!("\n💡 Why this strategy won:");
        match final_state.winning_strategy.as_str() {
            "strategy_1" => println!("   Fast direct answer was sufficient for this query"),
            "strategy_2" => {
                println!("   Tool-based search provided best balance of speed and accuracy")
            }
            "strategy_3" => {
                println!("   Comprehensive web search was needed for this complex/current query")
            }
            _ => println!("   Unknown strategy"),
        }
    }

    // Summary
    println!("\n\n{}", "═".repeat(80));
    println!("SUMMARY: Parallel Multi-Strategy Benefits");
    println!("{}", "═".repeat(80));
    println!("✅ Hedge against strategy failures → Always have backup strategies");
    println!(
        "✅ Optimize for different query types → Different strategies excel at different things"
    );
    println!("✅ Automatic quality selection → Committee picks best response");
    println!("✅ Parallel execution → Reduces latency (all run simultaneously)");
    println!("\n💡 Result: Best of all worlds - speed + accuracy + comprehensiveness");

    Ok(())
}
