// Committee Judge Example
//
// Demonstrates INNOVATION 13: Hierarchical Quality Checks (Committee Voting)
//
// PROBLEM: Single judge can be wrong:
// - One model has blind spots
// - Hallucinations in judge responses
// - Inconsistent scoring across queries
//
// SOLUTION: Use MULTIPLE judges and aggregate their scores!
//
// BENEFITS:
// - More reliable quality assessment (wisdom of crowds)
// - Detect judge disagreements (flag for human review)
// - Balance cost vs accuracy (cheap + expensive judges)
// - Reduce single-model bias
//
// ARCHITECTURE:
//
//                     ┌→ judge_gpt4o_mini →┐
//     response → fanout → judge_gpt4 ────→ aggregate_votes → END
//                     └→ judge_claude ──→┘
//                          (PARALLEL)

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CommitteeState {
    // Input
    query: String,
    response: String,
    tool_results: Option<String>,

    // Individual judge scores (0.0-1.0)
    score_gpt4o_mini: f64,
    score_gpt4: f64,
    score_claude: f64,

    // Judge explanations
    reasoning_gpt4o_mini: String,
    reasoning_gpt4: String,
    reasoning_claude: String,

    // Aggregated results
    average_score: f64,
    min_score: f64,
    max_score: f64,
    disagreement: f64, // max - min
    needs_human_review: bool,

    // Final decision
    passed: bool,
}

impl MergeableState for CommitteeState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
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
        if other.tool_results.is_some() {
            self.tool_results = other.tool_results.clone();
        }
        self.score_gpt4o_mini = self.score_gpt4o_mini.max(other.score_gpt4o_mini);
        self.score_gpt4 = self.score_gpt4.max(other.score_gpt4);
        self.score_claude = self.score_claude.max(other.score_claude);
        if !other.reasoning_gpt4o_mini.is_empty() {
            if self.reasoning_gpt4o_mini.is_empty() {
                self.reasoning_gpt4o_mini = other.reasoning_gpt4o_mini.clone();
            } else {
                self.reasoning_gpt4o_mini.push('\n');
                self.reasoning_gpt4o_mini
                    .push_str(&other.reasoning_gpt4o_mini);
            }
        }
        if !other.reasoning_gpt4.is_empty() {
            if self.reasoning_gpt4.is_empty() {
                self.reasoning_gpt4 = other.reasoning_gpt4.clone();
            } else {
                self.reasoning_gpt4.push('\n');
                self.reasoning_gpt4.push_str(&other.reasoning_gpt4);
            }
        }
        if !other.reasoning_claude.is_empty() {
            if self.reasoning_claude.is_empty() {
                self.reasoning_claude = other.reasoning_claude.clone();
            } else {
                self.reasoning_claude.push('\n');
                self.reasoning_claude.push_str(&other.reasoning_claude);
            }
        }
        self.average_score = self.average_score.max(other.average_score);
        self.min_score = self.min_score.max(other.min_score);
        self.max_score = self.max_score.max(other.max_score);
        self.disagreement = self.disagreement.max(other.disagreement);
        self.needs_human_review = self.needs_human_review || other.needs_human_review;
        self.passed = self.passed || other.passed;
    }
}

// ============================================================================
// COMMITTEE JUDGE NODE (all judges in one node for proper state accumulation)
// ============================================================================

// Helper functions for individual judge logic
fn judge_gpt4o_mini(response: &str, query: &str) -> (f64, String) {
    // Simulate GPT-4o-mini scoring (tends to be lenient)
    let has_content = response.len() > 30;
    let addresses_query = response
        .to_lowercase()
        .contains(query.to_lowercase().split_whitespace().next().unwrap_or(""));

    let score = if has_content && addresses_query {
        0.85 // Lenient - accepts most responses
    } else if has_content {
        0.70
    } else {
        0.50
    };

    let reasoning = if score >= 0.80 {
        "Response looks good, has content and addresses the query.".to_string()
    } else {
        "Response is too short or doesn't address the query well.".to_string()
    };

    (score, reasoning)
}

fn judge_gpt4(response: &str, tool_results: &Option<String>) -> (f64, String) {
    // Simulate GPT-4 scoring (strict on quality)
    let has_citations =
        response.contains("Source:") || response.contains("[") || response.contains("According to");

    let matches_tools = if let Some(ref tools) = tool_results {
        !response.contains("couldn't find") || tools.is_empty()
    } else {
        true
    };

    let is_detailed = response.len() > 50;

    let score = if has_citations && matches_tools && is_detailed {
        0.95 // Strict - needs citations and tool usage
    } else if matches_tools && is_detailed {
        0.75 // Missing citations
    } else if is_detailed {
        0.60 // Ignoring tools
    } else {
        0.40 // Too short
    };

    let reasoning = if score >= 0.90 {
        "Excellent response with citations and proper tool usage.".to_string()
    } else if score >= 0.70 {
        "Good response but missing citations or tool integration.".to_string()
    } else {
        "Response lacks quality - needs citations and better tool usage.".to_string()
    };

    (score, reasoning)
}

fn judge_claude(response: &str, query: &str, tool_results: &Option<String>) -> (f64, String) {
    // Simulate Claude scoring (balanced approach)
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let first_words = if query_words.len() >= 2 {
        query_words[..2].join(" ")
    } else {
        query_words.join(" ")
    };

    let is_relevant = response.to_lowercase().contains(&first_words);

    let is_helpful =
        response.len() > 40 && !response.starts_with("Error") && !response.contains("I don't know");

    let uses_tools = if let Some(ref tools) = tool_results {
        response.len() > 60 && !tools.is_empty()
    } else {
        true
    };

    let score = if is_relevant && is_helpful && uses_tools {
        0.88 // Balanced - good middle ground
    } else if is_relevant && is_helpful {
        0.70
    } else if is_relevant {
        0.55
    } else {
        0.35
    };

    let reasoning = if score >= 0.85 {
        "Response is relevant, helpful, and uses tools effectively.".to_string()
    } else if score >= 0.65 {
        "Response is relevant but could be more helpful or detailed.".to_string()
    } else {
        "Response lacks relevance or helpfulness.".to_string()
    };

    (score, reasoning)
}

// Main committee judge node - calls all judges and aggregates
fn committee_judge_node(
    mut state: CommitteeState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CommitteeState>> + Send>> {
    Box::pin(async move {
        println!("\n[COMMITTEE] Evaluating response with 3 judges...");

        // Judge 1: GPT-4o-mini
        let (score1, reasoning1) = judge_gpt4o_mini(&state.response, &state.query);
        state.score_gpt4o_mini = score1;
        state.reasoning_gpt4o_mini = reasoning1;
        println!("\n[JUDGE 1 - GPT-4o-mini] Score: {:.2}", score1);
        println!("[JUDGE 1] {}", state.reasoning_gpt4o_mini);

        // Judge 2: GPT-4
        let (score2, reasoning2) = judge_gpt4(&state.response, &state.tool_results);
        state.score_gpt4 = score2;
        state.reasoning_gpt4 = reasoning2;
        println!("\n[JUDGE 2 - GPT-4] Score: {:.2}", score2);
        println!("[JUDGE 2] {}", state.reasoning_gpt4);

        // Judge 3: Claude
        let (score3, reasoning3) = judge_claude(&state.response, &state.query, &state.tool_results);
        state.score_claude = score3;
        state.reasoning_claude = reasoning3;
        println!("\n[JUDGE 3 - Claude] Score: {:.2}", score3);
        println!("[JUDGE 3] {}", state.reasoning_claude);

        // Aggregate scores
        println!("\n[AGGREGATE] Combining judge scores...");

        state.average_score = (score1 + score2 + score3) / 3.0;
        state.min_score = score1.min(score2).min(score3);
        state.max_score = score1.max(score2).max(score3);
        state.disagreement = state.max_score - state.min_score;

        println!(
            "[AGGREGATE] Scores: GPT-4o-mini={:.2}, GPT-4={:.2}, Claude={:.2}",
            score1, score2, score3
        );
        println!("[AGGREGATE] Average: {:.2}", state.average_score);
        println!(
            "[AGGREGATE] Range: {:.2} - {:.2}",
            state.min_score, state.max_score
        );
        println!("[AGGREGATE] Disagreement: {:.2}", state.disagreement);

        // Flag for human review if judges disagree significantly
        const DISAGREEMENT_THRESHOLD: f64 = 0.3;
        state.needs_human_review = state.disagreement > DISAGREEMENT_THRESHOLD;

        if state.needs_human_review {
            println!("[AGGREGATE] ⚠️  HIGH DISAGREEMENT → FLAG FOR HUMAN REVIEW");
        } else {
            println!("[AGGREGATE] ✅ Judges agree (low disagreement)");
        }

        // Final decision based on average
        const QUALITY_THRESHOLD: f64 = 0.75;
        state.passed = state.average_score >= QUALITY_THRESHOLD;

        println!(
            "\n[AGGREGATE] Final decision: {}",
            if state.passed {
                "✅ PASSED"
            } else {
                "❌ FAILED"
            }
        );

        Ok(state)
    })
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Committee Judge (INNOVATION 13) ===\n");
    println!("Using 3 judge models in parallel for more reliable quality assessment!\n");

    // Build graph (simple: single committee node)
    let mut graph = StateGraph::<CommitteeState>::new();

    // Add committee judge node (all judges + aggregation in one node)
    graph.add_node_from_fn("committee_judge", committee_judge_node);

    // Connect: START → committee_judge → END
    graph.set_entry_point("committee_judge");
    graph.add_edge("committee_judge", END);

    // Compile
    let app = graph.compile()?;

    // Test Case 1: Good response with citations
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 1: High-quality response (with citations)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let state1 = CommitteeState {
        query: "What are the latest trends in AI?".to_string(),
        response: "According to the search results, the latest trends in AI include \
                   large language models, multimodal AI, and AI safety research. \
                   Source: [1, 2, 3]"
            .to_string(),
        tool_results: Some("Search found: Recent AI trends report with statistics".to_string()),
        ..Default::default()
    };

    let result1 = app.invoke(state1).await?;

    println!("\n✅ TEST 1 RESULT:");
    println!("   Average score: {:.2}", result1.final_state.average_score);
    println!("   Disagreement: {:.2}", result1.final_state.disagreement);
    println!(
        "   Needs human review: {}",
        result1.final_state.needs_human_review
    );
    println!("   Passed: {}", result1.final_state.passed);

    // Test Case 2: Poor response (no citations, ignoring tools)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 2: Low-quality response (no citations)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let state2 = CommitteeState {
        query: "What are the latest trends in AI?".to_string(),
        response: "I think AI is getting better.".to_string(),
        tool_results: Some("Search found: Recent AI trends report with statistics".to_string()),
        ..Default::default()
    };

    let result2 = app.invoke(state2).await?;

    println!("\n✅ TEST 2 RESULT:");
    println!("   Average score: {:.2}", result2.final_state.average_score);
    println!("   Disagreement: {:.2}", result2.final_state.disagreement);
    println!(
        "   Needs human review: {}",
        result2.final_state.needs_human_review
    );
    println!("   Passed: {}", result2.final_state.passed);

    // Test Case 3: Controversial response (judges disagree)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 3: Controversial response (judge disagreement)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let state3 = CommitteeState {
        query: "What are the latest trends in AI?".to_string(),
        response: "AI trends include machine learning and neural networks. \
                   These technologies are advancing rapidly."
            .to_string(),
        tool_results: Some("Search found: Recent AI trends report with statistics".to_string()),
        ..Default::default()
    };

    let result3 = app.invoke(state3).await?;

    println!("\n✅ TEST 3 RESULT:");
    println!("   Average score: {:.2}", result3.final_state.average_score);
    println!("   Disagreement: {:.2}", result3.final_state.disagreement);
    println!(
        "   Needs human review: {}",
        result3.final_state.needs_human_review
    );
    println!("   Passed: {}", result3.final_state.passed);

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("INNOVATION 13: BENEFITS");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ More reliable assessment (wisdom of crowds)");
    println!("✅ Detect judge disagreements");
    println!("✅ Flag controversial cases for human review");
    println!("✅ Balance cost vs accuracy (cheap + expensive)");
    println!("✅ Reduce single-model bias");
    println!("\n🎯 RESULT: Hierarchical quality assurance with committee voting!\n");

    Ok(())
}
