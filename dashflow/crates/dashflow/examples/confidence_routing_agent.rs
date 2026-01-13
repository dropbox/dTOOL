//! Confidence-Based Routing Agent (INNOVATION 2)
//!
//! This example demonstrates how to use LLM self-assessment (confidence scoring)
//! to intelligently route between direct answers and search-assisted answers.
//!
//! # Key Innovation
//!
//! Instead of hoping the LLM will decide when to search, we ask it to rate its
//! own confidence (0.0-1.0) and use that score for routing:
//! - High confidence (≥0.7): Proceed with direct answer
//! - Low confidence (<0.7): Force search before answering
//!
//! This creates a self-aware agent that KNOWS when it needs more information.
//!
//! # Graph Structure
//!
//! ```text
//! START → agent → extract_confidence → [conditional router]
//!                         ↓                      ↓
//!                  (0.0-1.0 score)      High (≥0.7) → END
//!                                              ↓
//!                                       Low (<0.7) → forced_search → agent → END
//! ```
//!
//! # Run Example
//!
//! ```bash
//! cargo run --package dashflow --example confidence_routing_agent
//! ```

use dashflow::quality::{ConfidenceScore, ConfidenceScorer};
use dashflow::{CompiledGraph, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State for confidence-routed agent.
///
/// Tracks:
/// - Query and response
/// - Confidence score from LLM
/// - Whether search was forced
/// - Search results (if any)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfidenceRoutedState {
    /// User query
    query: String,
    /// Agent's response (may include confidence metadata)
    response: Option<String>,
    /// Extracted confidence score
    confidence: Option<ConfidenceScore>,
    /// Whether we forced a search
    search_forced: bool,
    /// Search results (if search was performed)
    search_results: Option<String>,
    /// Clean response (metadata stripped)
    clean_response: Option<String>,
}

impl MergeableState for ConfidenceRoutedState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        if other.response.is_some() {
            self.response = other.response.clone();
        }
        if other.confidence.is_some() {
            self.confidence = other.confidence.clone();
        }
        self.search_forced = self.search_forced || other.search_forced;
        if other.search_results.is_some() {
            self.search_results = other.search_results.clone();
        }
        if other.clean_response.is_some() {
            self.clean_response = other.clean_response.clone();
        }
    }
}

impl ConfidenceRoutedState {
    fn new(query: String) -> Self {
        Self {
            query,
            response: None,
            confidence: None,
            search_forced: false,
            search_results: None,
            clean_response: None,
        }
    }
}

/// Mock agent node - generates response with confidence metadata.
///
/// In a real implementation, this would:
/// 1. Call LLM with system prompt including confidence instructions
/// 2. LLM returns response + confidence metadata
/// 3. Extract confidence and use for routing
fn agent_node(
    mut state: ConfidenceRoutedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceRoutedState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[AGENT] Processing query: {}", state.query);

        // Simulate LLM response based on whether search results are available
        let response = if state.search_results.is_some() {
            // Second pass: Have search results, high confidence
            println!("[AGENT] Have search results, generating confident response");
            format!(
                "Based on the search results, I can provide a detailed answer about {}. \
                 The documentation shows comprehensive information.\n\n\
                 CONFIDENCE: 0.95\n\
                 SHOULD_SEARCH: false\n\
                 REASON: I have complete search results to work with",
                state.query
            )
        } else if state.query.to_lowercase().contains("rust") {
            // First pass: Common topic, medium-high confidence
            println!("[AGENT] Familiar topic, medium-high confidence");
            format!(
                "I have some knowledge about {}. Rust is a systems programming language \
                 focused on safety and performance.\n\n\
                 CONFIDENCE: 0.75\n\
                 SHOULD_SEARCH: false\n\
                 REASON: I have general knowledge about this topic",
                state.query
            )
        } else {
            // First pass: Unfamiliar topic, low confidence
            println!("[AGENT] Unfamiliar topic, low confidence");
            format!(
                "I'm not completely certain about {}. I would benefit from searching \
                 for more specific information.\n\n\
                 CONFIDENCE: 0.45\n\
                 SHOULD_SEARCH: true\n\
                 REASON: This is outside my strong knowledge areas",
                state.query
            )
        };

        state.response = Some(response);
        Ok(state)
    })
}

/// Extract confidence node - parses confidence metadata from response.
///
/// This implements INNOVATION 2: Confidence Scoring.
fn extract_confidence_node(
    mut state: ConfidenceRoutedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceRoutedState>> + Send>,
> {
    Box::pin(async move {
        if let Some(response) = &state.response {
            println!("\n[CONFIDENCE EXTRACTOR] Parsing confidence metadata...");

            let scorer = ConfidenceScorer::new();
            let confidence = scorer.extract(response);

            println!(
                "[CONFIDENCE EXTRACTOR] Confidence: {:.2}",
                confidence.confidence
            );
            println!(
                "[CONFIDENCE EXTRACTOR] Should search: {}",
                confidence.should_have_searched
            );
            if let Some(reason) = &confidence.explanation {
                println!("[CONFIDENCE EXTRACTOR] Reason: {}", reason);
            }

            // Strip metadata to get clean response
            state.clean_response = Some(ConfidenceScorer::strip_metadata(response));
            state.confidence = Some(confidence);
        }

        Ok(state)
    })
}

/// Forced search node - simulates search before generating response.
///
/// In a real implementation, this would:
/// 1. Call search tool (vectorstore, web search, etc.)
/// 2. Retrieve relevant documents
/// 3. Set search_forced = true
/// 4. Route back to agent to generate response with search results
fn forced_search_node(
    mut state: ConfidenceRoutedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceRoutedState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[FORCED SEARCH] Low confidence detected, searching...");
        println!("[FORCED SEARCH] Query: {}", state.query);

        // Simulate search results
        state.search_results = Some(format!(
            "Search results for '{}':\n\
             - Comprehensive documentation available\n\
             - Multiple code examples\n\
             - Best practices guide\n\
             - Common pitfalls",
            state.query
        ));
        state.search_forced = true;

        println!("[FORCED SEARCH] ✅ Retrieved search results");
        println!("[FORCED SEARCH] Routing back to agent for second attempt");

        Ok(state)
    })
}

/// Router after confidence extraction.
///
/// Routes based on confidence score:
/// - High confidence (≥0.7) AND not suggested search → END (use response)
/// - Low confidence (<0.7) OR suggested search → forced_search (get more data)
fn route_by_confidence(state: &ConfidenceRoutedState) -> String {
    println!("\n[ROUTER] Deciding based on confidence...");

    if let Some(confidence) = &state.confidence {
        let threshold = 0.7;

        println!("[ROUTER] Confidence: {:.2}", confidence.confidence);
        println!("[ROUTER] Threshold: {:.2}", threshold);
        println!(
            "[ROUTER] Should search: {}",
            confidence.should_have_searched
        );

        // Check if we should force search
        if confidence.should_force_search(threshold) {
            // Only force search on first pass (not if we already searched)
            if !state.search_forced {
                println!("[ROUTER] 🔍 Low confidence → FORCE SEARCH");
                return "forced_search".to_string();
            } else {
                // Already searched, accept this response
                println!("[ROUTER] ✅ Already searched → END (accept response)");
                return "end".to_string();
            }
        }

        println!("[ROUTER] ✅ High confidence → END");
        "end".to_string()
    } else {
        // No confidence score, default to END
        println!("[ROUTER] ⚠️  No confidence score → END (default)");
        "end".to_string()
    }
}

/// Build confidence-routed agent graph.
///
/// The graph demonstrates INNOVATION 2: Confidence-Based Routing.
///
/// Flow:
/// 1. agent → generates response with confidence metadata
/// 2. extract_confidence → parses confidence score
/// 3. route_by_confidence → decides END or forced_search
/// 4. If forced_search: → forced_search → agent (second attempt with data)
/// 5. → END
fn build_confidence_routed_agent() -> dashflow::Result<CompiledGraph<ConfidenceRoutedState>> {
    let mut graph = StateGraph::<ConfidenceRoutedState>::new();

    // Add nodes
    graph.add_node_from_fn("agent", agent_node);
    graph.add_node_from_fn("extract_confidence", extract_confidence_node);
    graph.add_node_from_fn("forced_search", forced_search_node);

    // Set entry point
    graph.set_entry_point("agent");

    // Linear flow: agent → extract_confidence
    graph.add_edge("agent", "extract_confidence");

    // Conditional routing based on confidence
    let mut routes = HashMap::new();
    routes.insert("end".to_string(), END.to_string());
    routes.insert("forced_search".to_string(), "forced_search".to_string());
    graph.add_conditional_edges("extract_confidence", route_by_confidence, routes);

    // Cycle: forced_search loops back to agent for second attempt
    graph.add_edge("forced_search", "agent");

    // Compile
    graph.compile()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Confidence-Based Routing Agent ===\n");
    println!("This example demonstrates INNOVATION 2: Confidence Scoring");
    println!("Key features:");
    println!("  - LLM rates its own confidence (0.0-1.0)");
    println!("  - Low confidence triggers automatic search");
    println!("  - Self-aware agent knows when it needs more data\n");

    // Build agent
    let agent = build_confidence_routed_agent()?;

    // Test 1: Familiar topic (high confidence, no search needed)
    println!("\n{}", "=".repeat(70));
    println!("TEST 1: Familiar topic (high confidence expected)");
    println!("{}", "=".repeat(70));

    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let state = ConfidenceRoutedState::new("What is Rust programming language?".to_string());
        agent.invoke(state).await
    })?;

    let final_state = result.final_state;
    print_results(&final_state);

    // Test 2: Unfamiliar topic (low confidence, search forced)
    println!("\n{}", "=".repeat(70));
    println!("TEST 2: Unfamiliar topic (low confidence expected)");
    println!("{}", "=".repeat(70));

    let result = runtime.block_on(async {
        let state =
            ConfidenceRoutedState::new("What is the Zig build system architecture?".to_string());
        agent.invoke(state).await
    })?;

    let final_state = result.final_state;
    print_results(&final_state);

    println!("\n=== ARCHITECTURAL INSIGHT ===");
    println!("Instead of hoping the LLM will search when needed, we:");
    println!("  1. Ask LLM to rate its confidence");
    println!("  2. Use conditional routing based on confidence");
    println!("  3. Force search when confidence is low");
    println!("  4. Cycle back to agent with search results");
    println!("\nThis GUARANTEES the agent searches when uncertain!");

    Ok(())
}

fn print_results(state: &ConfidenceRoutedState) {
    println!("\n=== FINAL RESULTS ===");
    if let Some(confidence) = &state.confidence {
        println!("Confidence: {:.2}", confidence.confidence);
        println!("Should search: {}", confidence.should_have_searched);
        if let Some(reason) = &confidence.explanation {
            println!("Reason: {}", reason);
        }
    }
    println!("Search forced: {}", state.search_forced);
    if state.search_forced {
        println!("✅ Agent searched because confidence was low");
    } else {
        println!("✅ Agent answered directly (confidence was sufficient)");
    }
    println!("\nClean response:");
    println!(
        "{}",
        state.clean_response.as_deref().unwrap_or("(no response)")
    );
}
