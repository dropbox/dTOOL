// Quality Assurance Subgraph Example
//
// Demonstrates INNOVATION 11: Subgraph for Quality Assurance
//
// PROBLEM: Quality checking logic is duplicated across multiple agents:
// - Every agent needs judge + validator + citation checker
// - Copy-pasted quality logic is hard to maintain
// - Inconsistent quality standards across agents
//
// SOLUTION: Build quality checking as a REUSABLE SUBGRAPH!
//
// BENEFITS:
// - Single source of truth for quality checks
// - Plug into any agent via add_subgraph_with_mapping()
// - Update quality logic in one place
// - Consistent quality standards everywhere
//
// ARCHITECTURE:
//
// MAIN AGENT:
//   agent → quality_check (subgraph) → [conditional] → retry OR accept
//
// QA SUBGRAPH (reusable):
//   judge → validate → check_citations → [conditional] → pass OR fix_needed

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// QA SUBGRAPH STATE (internal to quality checking)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct QAState {
    // Input from parent
    query: String,
    response: String,
    tool_results: Option<String>,

    // Quality scores (0.0-1.0)
    accuracy_score: f64,
    relevance_score: f64,
    citation_score: f64,

    // Validation results
    has_proper_format: bool,
    has_citations: bool,
    citations_valid: bool,

    // Output to parent
    overall_quality: f64,
    passed: bool,
    issues: Vec<String>,
    fix_instructions: String,
}

impl MergeableState for QAState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            self.query = other.query.clone();
        }
        if !other.response.is_empty() {
            self.response = other.response.clone();
        }
        if other.tool_results.is_some() {
            self.tool_results = other.tool_results.clone();
        }
        self.accuracy_score = self.accuracy_score.max(other.accuracy_score);
        self.relevance_score = self.relevance_score.max(other.relevance_score);
        self.citation_score = self.citation_score.max(other.citation_score);
        self.has_proper_format = self.has_proper_format || other.has_proper_format;
        self.has_citations = self.has_citations || other.has_citations;
        self.citations_valid = self.citations_valid || other.citations_valid;
        self.overall_quality = self.overall_quality.max(other.overall_quality);
        self.passed = self.passed || other.passed;
        self.issues.extend(other.issues.clone());
        if !other.fix_instructions.is_empty() {
            self.fix_instructions = other.fix_instructions.clone();
        }
    }
}

// ============================================================================
// QA SUBGRAPH NODES
// ============================================================================

// Node 1: Judge - Score response quality
fn judge_node(
    mut state: QAState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<QAState>> + Send>> {
    Box::pin(async move {
        println!("\n[QA-JUDGE] Scoring response quality...");

        // Simulate LLM-as-judge scoring
        // In production: actual GPT-4 or Claude judge

        // Accuracy: Does response match tool results?
        state.accuracy_score = if let Some(ref tools) = state.tool_results {
            if state.response.contains("couldn't find") && !tools.is_empty() {
                0.3 // Low - ignored tool results
            } else if state.response.len() > 50 {
                0.9 // High - substantive response
            } else {
                0.6 // Medium
            }
        } else {
            0.8 // No tools to check against
        };

        // Relevance: Does response answer the query?
        let query_words: Vec<&str> = state.query.split_whitespace().collect();
        let response_lower = state.response.to_lowercase();
        let matching_words = query_words
            .iter()
            .filter(|w| response_lower.contains(&w.to_lowercase()))
            .count();

        state.relevance_score = (matching_words as f64 / query_words.len() as f64).min(1.0);

        // Citation score: Has proper citations?
        state.citation_score = if state.response.contains("Source:")
            || state.response.contains("[")
            || state.response.contains("According to")
        {
            0.95
        } else {
            0.4
        };

        println!("[QA-JUDGE] Accuracy: {:.2}", state.accuracy_score);
        println!("[QA-JUDGE] Relevance: {:.2}", state.relevance_score);
        println!("[QA-JUDGE] Citation: {:.2}", state.citation_score);

        Ok(state)
    })
}

// Node 2: Validate - Check response format
fn validate_node(
    mut state: QAState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<QAState>> + Send>> {
    Box::pin(async move {
        println!("\n[QA-VALIDATE] Checking response format...");

        // Check proper format
        state.has_proper_format = state.response.len() >= 20
            && !state.response.is_empty()
            && !state.response.starts_with("Error");

        // Check has citations
        state.has_citations = state.response.contains("Source:")
            || state.response.contains("[")
            || state.response.contains("According to");

        println!(
            "[QA-VALIDATE] Proper format: {}",
            if state.has_proper_format {
                "✅"
            } else {
                "❌"
            }
        );
        println!(
            "[QA-VALIDATE] Has citations: {}",
            if state.has_citations { "✅" } else { "❌" }
        );

        Ok(state)
    })
}

// Node 3: Check Citations - Verify sources
fn check_citations_node(
    mut state: QAState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<QAState>> + Send>> {
    Box::pin(async move {
        println!("\n[QA-CITATIONS] Verifying citations...");

        // If has citations, verify they're real
        if state.has_citations {
            // Simulate checking citations against tool results
            state.citations_valid = if let Some(ref tools) = state.tool_results {
                // In production: parse citations and verify against tools
                !tools.is_empty()
            } else {
                false // Can't verify without tool results
            };
        } else {
            state.citations_valid = false;
        }

        println!(
            "[QA-CITATIONS] Valid: {}",
            if state.citations_valid { "✅" } else { "❌" }
        );

        // Calculate overall quality
        state.overall_quality =
            (state.accuracy_score + state.relevance_score + state.citation_score) / 3.0;

        println!(
            "\n[QA-SUMMARY] Overall quality: {:.2}",
            state.overall_quality
        );

        Ok(state)
    })
}

// Node 4: Generate fix instructions (if quality is low)
fn generate_fix_instructions_node(
    mut state: QAState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<QAState>> + Send>> {
    Box::pin(async move {
        println!("\n[QA-FIX] Generating fix instructions...");

        state.issues.clear();

        // Collect specific issues
        if state.accuracy_score < 0.7 {
            state
                .issues
                .push("Low accuracy - response doesn't match tool results".to_string());
        }
        if state.relevance_score < 0.7 {
            state
                .issues
                .push("Low relevance - response doesn't address query".to_string());
        }
        if state.citation_score < 0.7 {
            state
                .issues
                .push("Missing citations - add sources".to_string());
        }
        if !state.has_proper_format {
            state
                .issues
                .push("Improper format - needs more detail".to_string());
        }
        if state.has_citations && !state.citations_valid {
            state
                .issues
                .push("Invalid citations - cite actual sources".to_string());
        }

        // Generate fix instructions
        state.fix_instructions = if !state.issues.is_empty() {
            format!(
                "Response needs improvement:\n{}",
                state
                    .issues
                    .iter()
                    .map(|i| format!("  - {}", i))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            "Response passed all checks!".to_string()
        };

        println!("[QA-FIX] Found {} issues", state.issues.len());
        for issue in &state.issues {
            println!("[QA-FIX]   - {}", issue);
        }

        Ok(state)
    })
}

// Conditional router: Pass or needs fixing?
fn qa_router(state: &QAState) -> String {
    const QUALITY_THRESHOLD: f64 = 0.75;

    if state.overall_quality >= QUALITY_THRESHOLD
        && state.has_proper_format
        && (state.has_citations == state.citations_valid || !state.has_citations)
    {
        println!(
            "\n[QA-ROUTER] ✅ PASSED (quality: {:.2})",
            state.overall_quality
        );
        END.to_string()
    } else {
        println!(
            "\n[QA-ROUTER] ❌ NEEDS FIX (quality: {:.2})",
            state.overall_quality
        );
        "generate_fix_instructions".to_string()
    }
}

// ============================================================================
// BUILD REUSABLE QA SUBGRAPH
// ============================================================================

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            self.query = other.query.clone();
        }
        if !other.response.is_empty() {
            self.response = other.response.clone();
        }
        if other.tool_results.is_some() {
            self.tool_results = other.tool_results.clone();
        }
        self.quality_score = self.quality_score.max(other.quality_score);
        self.quality_passed = self.quality_passed || other.quality_passed;
        self.quality_issues.extend(other.quality_issues.clone());
        if !other.fix_instructions.is_empty() {
            self.fix_instructions = other.fix_instructions.clone();
        }
        self.retry_count = self.retry_count.max(other.retry_count);
    }
}

fn build_qa_subgraph() -> dashflow::Result<StateGraph<QAState>> {
    let mut qa_graph = StateGraph::<QAState>::new();

    // Add nodes
    qa_graph.add_node_from_fn("judge", judge_node);
    qa_graph.add_node_from_fn("validate", validate_node);
    qa_graph.add_node_from_fn("check_citations", check_citations_node);
    qa_graph.add_node_from_fn("generate_fix_instructions", generate_fix_instructions_node);

    // Linear flow through quality checks
    qa_graph.set_entry_point("judge");
    qa_graph.add_edge("judge", "validate");
    qa_graph.add_edge("validate", "check_citations");

    // Conditional: Pass or fix?
    let mut qa_routes = HashMap::new();
    qa_routes.insert(END.to_string(), END.to_string());
    qa_routes.insert(
        "generate_fix_instructions".to_string(),
        "generate_fix_instructions".to_string(),
    );
    qa_graph.add_conditional_edges("check_citations", qa_router, qa_routes);

    // Fix instructions → END
    qa_graph.add_edge("generate_fix_instructions", END);

    Ok(qa_graph)
}

// ============================================================================
// MAIN AGENT STATE (uses QA subgraph)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AgentState {
    // Input
    query: String,

    // Agent response
    response: String,
    tool_results: Option<String>,

    // Quality check results (from subgraph)
    quality_score: f64,
    quality_passed: bool,
    quality_issues: Vec<String>,
    fix_instructions: String,

    // Retry control
    retry_count: u32,
}

// ============================================================================
// MAIN AGENT NODES
// ============================================================================

fn agent_node(
    mut state: AgentState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<AgentState>> + Send>> {
    Box::pin(async move {
        println!("\n[AGENT] Generating response...");
        println!("[AGENT] Query: '{}'", state.query);

        // Simulate agent response
        if state.retry_count == 0 {
            // First attempt: low quality (no citations)
            state.response = format!("I think {}. This is my answer.", state.query.to_lowercase());
            state.tool_results = Some(
                "Tool found: Recent data about the topic with statistics [source1, source2]"
                    .to_string(),
            );
        } else {
            // Retry: high quality (with citations and tool results)
            state.response = format!(
                "According to the search results, {}. Source: [1, 2]",
                state.query.to_lowercase()
            );
        }

        println!("[AGENT] Response: '{}'", state.response);

        Ok(state)
    })
}

// Conditional router: Retry or accept?
fn main_router(state: &AgentState) -> String {
    if state.quality_passed {
        println!("\n[MAIN-ROUTER] ✅ Quality passed → ACCEPT");
        END.to_string()
    } else if state.retry_count < 2 {
        println!(
            "\n[MAIN-ROUTER] 🔄 Quality failed → RETRY (attempt {})",
            state.retry_count + 2
        );
        "agent".to_string()
    } else {
        println!("\n[MAIN-ROUTER] ⚠️  Max retries → ACCEPT ANYWAY");
        END.to_string()
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QA Subgraph (INNOVATION 11) ===\n");
    println!("Demonstrating REUSABLE quality checking as a subgraph!\n");

    // Build QA subgraph
    let qa_subgraph = build_qa_subgraph()?;

    // Build main agent graph
    let mut main_graph = StateGraph::<AgentState>::new();

    // Add agent node
    main_graph.add_node_from_fn("agent", agent_node);

    // Add QA subgraph with state mapping
    main_graph.add_subgraph_with_mapping(
        "quality_check",
        qa_subgraph,
        // Map AgentState → QAState
        |agent_state: &AgentState| QAState {
            query: agent_state.query.clone(),
            response: agent_state.response.clone(),
            tool_results: agent_state.tool_results.clone(),
            ..Default::default()
        },
        // Map AgentState + QAState → AgentState
        |mut agent_state: AgentState, qa_state: QAState| {
            agent_state.quality_score = qa_state.overall_quality;
            agent_state.quality_passed = qa_state.passed;
            agent_state.quality_issues = qa_state.issues;
            agent_state.fix_instructions = qa_state.fix_instructions;
            agent_state.retry_count += 1;
            agent_state
        },
    )?;

    // Connect: agent → quality_check → conditional → retry OR accept
    main_graph.set_entry_point("agent");
    main_graph.add_edge("agent", "quality_check");

    let mut main_routes = HashMap::new();
    main_routes.insert(END.to_string(), END.to_string());
    main_routes.insert("agent".to_string(), "agent".to_string());
    main_graph.add_conditional_edges("quality_check", main_router, main_routes);

    // Compile main graph
    let app = main_graph.compile()?;

    // Test
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST: Agent with QA subgraph + retry loop");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let state = AgentState {
        query: "What are the latest trends in AI?".to_string(),
        ..Default::default()
    };

    let result = app.invoke(state).await?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("FINAL RESULT");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Query: {}", result.final_state.query);
    println!("Response: {}", result.final_state.response);
    println!("Quality score: {:.2}", result.final_state.quality_score);
    println!("Passed: {}", result.final_state.quality_passed);
    println!("Retries: {}", result.final_state.retry_count);

    if !result.final_state.quality_issues.is_empty() {
        println!("\nIssues:");
        for issue in &result.final_state.quality_issues {
            println!("  - {}", issue);
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("INNOVATION 11: BENEFITS");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Single source of truth for quality checks");
    println!("✅ Reusable across all agents");
    println!("✅ Update quality logic in one place");
    println!("✅ Consistent quality standards");
    println!("✅ Automatic retry with fix instructions");
    println!("\n🎯 RESULT: Modular quality assurance!\n");

    Ok(())
}
