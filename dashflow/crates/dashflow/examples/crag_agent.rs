// CRAG (Corrective RAG) Agent Example
//
// Demonstrates INNOVATION 4: Retrieval Grading
//
// PROBLEM: LLMs sometimes receive low-quality or irrelevant search results,
// leading to poor responses even when the LLM itself is capable.
//
// SOLUTION: Grade search results BEFORE passing to agent. If quality is low:
// 1. Transform query and retry search
// 2. Try alternative search strategy
// 3. Only pass high-quality results to LLM
//
// GRAPH ARCHITECTURE (with cycles):
//
// START → search → grade_documents → [conditional router]
//           ↑           ↓                      ↓
//           |      (LLM scores       High (≥0.7) → generate_response → END
//           |       relevance)              ↓
//           |                          Low (<0.7) → transform_query
//           └─────────────────────────────────────────┘
//                            CYCLE!

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CragState {
    // Input
    query: String,

    // Search results
    search_results: Vec<String>,

    // Grading
    document_grade: f64,
    grading_explanation: String,

    // Retry control
    search_retries: u32,
    max_retries: u32,

    // Output
    final_response: String,
}

impl MergeableState for CragState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        self.search_results.extend(other.search_results.clone());
        self.document_grade = self.document_grade.max(other.document_grade);
        if !other.grading_explanation.is_empty() {
            if self.grading_explanation.is_empty() {
                self.grading_explanation = other.grading_explanation.clone();
            } else {
                self.grading_explanation.push('\n');
                self.grading_explanation
                    .push_str(&other.grading_explanation);
            }
        }
        self.search_retries = self.search_retries.max(other.search_retries);
        self.max_retries = self.max_retries.max(other.max_retries);
        if !other.final_response.is_empty() {
            if self.final_response.is_empty() {
                self.final_response = other.final_response.clone();
            } else {
                self.final_response.push('\n');
                self.final_response.push_str(&other.final_response);
            }
        }
    }
}

// Node 1: Search for documents
fn search_node(
    mut state: CragState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CragState>> + Send>> {
    Box::pin(async move {
        println!("\n[SEARCH] Query: '{}'", state.query);
        println!(
            "[SEARCH] Attempt: {}/{}",
            state.search_retries + 1,
            state.max_retries
        );

        // Simulate search results
        // First attempt: low-quality results
        // Retries: high-quality results
        if state.search_retries == 0 {
            // Bad results - not relevant to query
            state.search_results = vec![
                "This document discusses a completely unrelated topic.".to_string(),
                "Another irrelevant result about something else.".to_string(),
            ];
            println!("[SEARCH] ❌ Retrieved low-quality results");
        } else {
            // Good results - relevant to query
            state.search_results = vec![
                format!("High-quality document directly answering: {}", state.query),
                format!(
                    "Another relevant source with details about: {}",
                    state.query
                ),
                "Supporting evidence with citations and data.".to_string(),
            ];
            println!("[SEARCH] ✅ Retrieved high-quality results");
        }

        Ok(state)
    })
}

// Node 2: Grade document relevance
fn grade_documents_node(
    mut state: CragState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CragState>> + Send>> {
    Box::pin(async move {
        println!(
            "\n[DOCUMENT GRADER] Scoring {} results",
            state.search_results.len()
        );

        // Simulate LLM grading the search results
        // In production: this would be an actual LLM call that scores relevance
        let query_lower = state.query.to_lowercase();

        // Simple keyword-based relevance (mock LLM judge)
        let mut relevant_count = 0;

        for result in &state.search_results {
            let result_lower = result.to_lowercase();

            // Check if result contains query keywords
            let query_words: Vec<&str> = query_lower.split_whitespace().collect();
            let mut matches = 0;
            for word in query_words.iter() {
                if result_lower.contains(word) {
                    matches += 1;
                }
            }

            if matches > 0 {
                relevant_count += 1;
            }
        }

        // Compute grade based on relevance
        let relevance_score = (relevant_count as f64) / (state.search_results.len() as f64);

        state.document_grade = relevance_score;

        if relevance_score >= 0.7 {
            state.grading_explanation = format!(
                "✅ High relevance: {}/{} documents are relevant (score: {:.2})",
                relevant_count,
                state.search_results.len(),
                relevance_score
            );
        } else {
            state.grading_explanation = format!(
                "❌ Low relevance: {}/{} documents are relevant (score: {:.2})",
                relevant_count,
                state.search_results.len(),
                relevance_score
            );
        }

        println!("[DOCUMENT GRADER] {}", state.grading_explanation);

        Ok(state)
    })
}

// Node 3: Transform query to improve search
fn transform_query_node(
    mut state: CragState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CragState>> + Send>> {
    Box::pin(async move {
        println!("\n[QUERY TRANSFORMER] Original: '{}'", state.query);

        // Simulate LLM rewriting query for better results
        // In production: this would be an actual LLM call
        let transformed = if state.query.contains("simple") {
            state.query.replace("simple", "comprehensive detailed")
        } else {
            format!("{} detailed explanation with examples", state.query)
        };

        state.query = transformed.clone();
        state.search_retries += 1;

        println!("[QUERY TRANSFORMER] ✅ Transformed: '{}'", transformed);

        Ok(state)
    })
}

// Node 4: Generate final response
fn generate_response_node(
    mut state: CragState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<CragState>> + Send>> {
    Box::pin(async move {
        println!(
            "\n[RESPONSE GENERATOR] Using {} high-quality results",
            state.search_results.len()
        );

        // Simulate LLM generating response from good search results
        state.final_response = format!(
            "Based on high-quality search results (grade: {:.2}):\n\n\
         Here is a comprehensive answer to '{}'\n\
         \nSources consulted:\n{}\n\
         \n(This response is based on verified, relevant information)",
            state.document_grade,
            state.query,
            state
                .search_results
                .iter()
                .map(|r| format!("- {}", r))
                .collect::<Vec<_>>()
                .join("\n")
        );

        println!("[RESPONSE GENERATOR] ✅ Generated response with citations");

        Ok(state)
    })
}

// Conditional router: decide based on document grade
fn route_by_quality(state: &CragState) -> String {
    println!(
        "\n[ROUTER] Document grade: {:.2}, Retries: {}/{}",
        state.document_grade, state.search_retries, state.max_retries
    );

    if state.document_grade >= 0.7 {
        // High-quality results → generate response
        println!("[ROUTER] ✅ High quality → GENERATE RESPONSE");
        "generate_response".to_string()
    } else if state.search_retries < state.max_retries {
        // Low-quality results + retries remaining → transform query and retry
        println!("[ROUTER] 🔄 Low quality → TRANSFORM QUERY (cycle to search)");
        "transform_query".to_string()
    } else {
        // Exhausted retries → generate best-effort response
        println!("[ROUTER] ⚠️  Max retries reached → GENERATE ANYWAY");
        "generate_response".to_string()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CRAG (Corrective RAG) Agent Example ===");
    println!("\nDemonstrates INNOVATION 4: Retrieval Grading");
    println!("\nKey concept: Grade search results BEFORE passing to LLM");
    println!("If quality is low, transform query and retry search (CYCLE)");

    // Create state graph
    let mut graph = StateGraph::<CragState>::new();

    // Add nodes
    graph.add_node_from_fn("search", search_node);
    graph.add_node_from_fn("grade_documents", grade_documents_node);
    graph.add_node_from_fn("transform_query", transform_query_node);
    graph.add_node_from_fn("generate_response", generate_response_node);

    // Add edges
    graph.set_entry_point("search");
    graph.add_edge("search", "grade_documents");

    // Conditional routing based on document quality
    let mut routes = HashMap::new();
    routes.insert(
        "generate_response".to_string(),
        "generate_response".to_string(),
    );
    routes.insert("transform_query".to_string(), "transform_query".to_string());
    graph.add_conditional_edges("grade_documents", route_by_quality, routes);

    // Transform query cycles back to search (CORRECTIVE loop)
    graph.add_edge("transform_query", "search");

    // Response generation ends the graph
    graph.add_edge("generate_response", END);

    // Compile
    let app = graph.compile()?;

    // Test 1: Query that will get low-quality results initially
    println!("\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST: Simple query → Low-quality results → Transform → Retry → Success");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let initial_state = CragState {
            query: "What is Rust programming?".to_string(),
            max_retries: 2,
            ..Default::default()
        };

        app.invoke(initial_state).await
    })?;

    let final_state = result.final_state;

    println!("\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("FINAL RESULT");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("Total search attempts: {}", final_state.search_retries + 1);
    println!("Final document grade: {:.2}", final_state.document_grade);
    println!("\n{}", final_state.final_response);

    // Verify CRAG worked
    if final_state.search_retries > 0 && final_state.document_grade >= 0.7 {
        println!("\n✅ SUCCESS: CRAG agent automatically:");
        println!("  1. Detected low-quality search results (grade < 0.7)");
        println!("  2. Transformed query for better results");
        println!("  3. Retried search (CYCLED back through graph)");
        println!("  4. Achieved high-quality results (grade ≥ 0.7)");
        println!("  5. Generated response from verified good data");
    } else {
        println!("\n❌ UNEXPECTED: CRAG did not trigger as expected");
    }

    Ok(())
}
