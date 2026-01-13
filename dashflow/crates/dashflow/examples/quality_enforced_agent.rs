//! Self-Correcting Agent with Quality Enforcement (INNOVATION 1)
//!
//! This example demonstrates DashFlow's ARCHITECTURE-based approach to 100% quality:
//! - **Cycles**: Retry loops until quality threshold met
//! - **Conditional edges**: Route based on quality scores
//! - **Quality gates**: Mandatory validation before END
//! - **Tool result validation**: Pre-check before LLM sees data
//! - **Response validation**: Post-check for "couldn't find" patterns
//!
//! Instead of hoping the LLM will use tools correctly, we GUARANTEE it through
//! graph structure.
//!
//! # Graph Structure
//!
//! ```text
//! START → agent → validate_response → quality_check → END or retry
//!                         ↓                 ↓
//!                  (check patterns)    (score ≥ 0.95?)
//!                         ↓                 ↓
//!                    ← retry if bad    ← retry if low
//! ```
//!
//! # Key Innovation
//!
//! The graph uses a **CYCLE** (retry → agent) to automatically retry until quality
//! is achieved. This is a fundamental architectural feature of DashFlow that
//! guarantees quality, not just hopes for it.
//!
//! # Run Example
//!
//! ```bash
//! cargo run --package dashflow --example quality_enforced_agent
//! ```

use dashflow::quality::{ResponseValidator, ValidationResult};
use dashflow::{CompiledGraph, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State for self-correcting agent with quality enforcement.
///
/// This state tracks:
/// - Query and response
/// - Tool usage and results
/// - Quality scores
/// - Retry count (for cycle termination)
/// - Validation issues (for diagnostics)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SelfCorrectingState {
    /// User query
    query: String,
    /// Agent's response
    response: Option<String>,
    /// Tool results (if any)
    tool_results: Option<String>,
    /// Whether tools were called
    tool_called: bool,
    /// Quality score (0.0-1.0)
    quality_score: Option<f32>,
    /// Retry count
    retry_count: usize,
    /// Maximum retries allowed
    max_retries: usize,
    /// Validation issues detected
    issues: Vec<String>,
}

impl MergeableState for SelfCorrectingState {
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
        if other.tool_results.is_some() {
            self.tool_results = other.tool_results.clone();
        }
        self.tool_called = self.tool_called || other.tool_called;
        if other.quality_score.is_some() {
            self.quality_score = other.quality_score;
        }
        self.retry_count = self.retry_count.max(other.retry_count);
        self.max_retries = self.max_retries.max(other.max_retries);
        self.issues.extend(other.issues.clone());
    }
}

impl SelfCorrectingState {
    fn new(query: String) -> Self {
        Self {
            query,
            response: None,
            tool_results: None,
            tool_called: false,
            quality_score: None,
            retry_count: 0,
            max_retries: 3,
            issues: Vec::new(),
        }
    }
}

/// Mock agent node - simulates agent behavior for demonstration.
///
/// In a real implementation, this would:
/// 1. Call LLM with bound tools
/// 2. Execute tool calls
/// 3. Call LLM again with tool results
/// 4. Return final response
fn agent_node(
    mut state: SelfCorrectingState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<SelfCorrectingState>> + Send>,
> {
    Box::pin(async move {
        println!(
            "\n[AGENT] Attempt {}/{}",
            state.retry_count + 1,
            state.max_retries
        );
        println!("[AGENT] Query: {}", state.query);

        // Simulate agent behavior:
        // - On first attempt: bad response (ignores tools)
        // - On second attempt: good response (uses tools)
        if state.retry_count == 0 {
            // Simulate bad response
            state.response =
                Some("I couldn't find specific documentation about that topic.".to_string());
            state.tool_called = true;
            state.tool_results = Some("Detailed documentation about the topic...".to_string());
            println!("[AGENT] ❌ Generated bad response (ignoring tool results)");
        } else {
            // Simulate good response
            state.response = Some("Based on the documentation, here's the answer: The topic is well-documented and includes detailed examples...".to_string());
            state.tool_called = true;
            state.tool_results = Some("Detailed documentation about the topic...".to_string());
            println!("[AGENT] ✅ Generated good response (using tool results)");
        }

        Ok(state)
    })
}

/// Validate response node - checks for "couldn't find" patterns.
///
/// This implements INNOVATION 10: Response Validator.
/// It detects when the LLM ignores tool results and says "couldn't find".
fn validate_response_node(
    mut state: SelfCorrectingState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<SelfCorrectingState>> + Send>,
> {
    Box::pin(async move {
        if let Some(response) = &state.response {
            println!("\n[RESPONSE VALIDATOR] Checking response...");

            let validator = ResponseValidator::new();
            let validation =
                validator.validate(response, state.tool_called, state.tool_results.as_deref());

            match validation {
                ValidationResult::Valid => {
                    println!("[RESPONSE VALIDATOR] ✅ Response valid");
                }
                ValidationResult::ToolResultsIgnored { phrase, .. } => {
                    println!("[RESPONSE VALIDATOR] ❌ Tool results ignored!");
                    println!("[RESPONSE VALIDATOR] Detected phrase: '{}'", phrase);
                    state
                        .issues
                        .push(format!("Tool results ignored: {}", phrase));
                }
                ValidationResult::MissingCitations { .. } => {
                    println!("[RESPONSE VALIDATOR] ⚠️  Missing citations");
                    state.issues.push("Missing citations".to_string());
                }
            }
        }

        Ok(state)
    })
}

/// Quality check node - evaluates response quality.
///
/// This implements INNOVATION 5: Quality Gate.
/// It judges every response and enforces quality thresholds.
fn quality_check_node(
    mut state: SelfCorrectingState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<SelfCorrectingState>> + Send>,
> {
    Box::pin(async move {
        if let Some(response) = &state.response {
            println!("\n[QUALITY GATE] Checking quality...");

            let threshold = 0.95;

            // Mock quality scoring
            // In production, this would call LLM-as-judge
            let mock_score = if state.issues.is_empty() && response.len() > 100 {
                0.96 // Good quality
            } else {
                0.85 // Low quality
            };

            state.quality_score = Some(mock_score);
            println!("[QUALITY GATE] Score: {:.2}", mock_score);

            if mock_score >= threshold {
                println!("[QUALITY GATE] ✅ Quality threshold met");
            } else {
                println!(
                    "[QUALITY GATE] ❌ Below threshold ({} < {})",
                    mock_score, threshold
                );
                state
                    .issues
                    .push(format!("Quality {} < {}", mock_score, threshold));
            }
        }

        Ok(state)
    })
}

/// Conditional routing based on quality and retry count.
///
/// This is where the CYCLE is controlled. The router decides:
/// - END if quality is good enough
/// - END if max retries reached
/// - RETRY otherwise (creates the cycle)
fn route_after_quality(state: &SelfCorrectingState) -> String {
    println!("\n[ROUTER] Deciding next step...");
    println!(
        "[ROUTER] Quality: {:?}, Retry: {}/{}, Issues: {}",
        state.quality_score,
        state.retry_count,
        state.max_retries,
        state.issues.len()
    );

    // Check if quality is good enough
    if let Some(score) = state.quality_score {
        if score >= 0.95 && state.issues.is_empty() {
            println!("[ROUTER] ✅ Quality sufficient → END");
            return "end".to_string();
        }
    }

    // Check retry limit
    if state.retry_count >= state.max_retries {
        println!("[ROUTER] ⚠️  Max retries reached → END (best effort)");
        return "end".to_string();
    }

    // Retry - THIS CREATES THE CYCLE!
    println!("[ROUTER] 🔄 Retrying (cycle back to agent)");
    "retry".to_string()
}

/// Retry node - prepares state for retry.
///
/// This increments the retry counter and clears previous results
/// so the agent generates a new response.
fn retry_node(
    mut state: SelfCorrectingState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<SelfCorrectingState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[RETRY] Preparing retry {}", state.retry_count + 1);

        state.retry_count += 1;
        state.response = None;
        state.quality_score = None;
        state.issues.clear();

        println!("[RETRY] State reset for new attempt");

        Ok(state)
    })
}

/// Build self-correcting agent graph with quality enforcement.
///
/// This demonstrates the key architectural innovation: using DashFlow's
/// CYCLES and CONDITIONAL EDGES to guarantee quality.
///
/// The graph structure:
/// 1. agent → validate_response → quality_check → [conditional]
/// 2. If quality low: → retry → agent (CYCLE!)
/// 3. If quality good: → END
fn build_self_correcting_agent() -> dashflow::Result<CompiledGraph<SelfCorrectingState>> {
    let mut graph = StateGraph::<SelfCorrectingState>::new();

    // Add nodes
    graph.add_node_from_fn("agent", agent_node);
    graph.add_node_from_fn("validate_response", validate_response_node);
    graph.add_node_from_fn("quality_check", quality_check_node);
    graph.add_node_from_fn("retry", retry_node);

    // Set entry point
    graph.set_entry_point("agent");

    // Add linear edges
    graph.add_edge("agent", "validate_response");
    graph.add_edge("validate_response", "quality_check");

    // Conditional edge: quality check → END or retry
    let mut routes = HashMap::new();
    routes.insert("end".to_string(), END.to_string());
    routes.insert("retry".to_string(), "retry".to_string());
    graph.add_conditional_edges("quality_check", route_after_quality, routes);

    // THE CYCLE: retry loops back to agent
    // This is the key architectural innovation!
    graph.add_edge("retry", "agent");

    // Compile the graph
    graph.compile()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Self-Correcting Agent with Quality Enforcement ===\n");
    println!("This example demonstrates INNOVATION 1: Self-Correcting Graph Architecture");
    println!("Key features:");
    println!("  - Cycles: Automatic retry until quality threshold met");
    println!("  - Conditional edges: Route based on quality scores");
    println!("  - Response validation: Detect 'couldn't find' patterns");
    println!("  - Quality gates: Enforce minimum quality thresholds\n");

    // Build agent
    let agent = build_self_correcting_agent()?;

    // Run agent synchronously using block_on
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let initial_state =
            SelfCorrectingState::new("What is the purpose of tokio in Rust?".to_string());
        agent.invoke(initial_state).await
    })?;

    let final_state = result.final_state;

    // Print results
    println!("\n=== FINAL RESULTS ===");
    println!("Total attempts: {}", final_state.retry_count + 1);
    println!(
        "Quality score: {:.2}",
        final_state.quality_score.unwrap_or(0.0)
    );
    println!("Issues encountered: {}", final_state.issues.len());
    if !final_state.issues.is_empty() {
        println!("Issues:");
        for issue in &final_state.issues {
            println!("  - {}", issue);
        }
    }
    println!("\nFinal response:");
    println!("{}", final_state.response.unwrap_or_default());

    // Success criteria
    if let Some(score) = final_state.quality_score {
        if score >= 0.95 && final_state.issues.is_empty() {
            println!("\n✅ SUCCESS: Quality threshold met!");
            println!(
                "The graph AUTOMATICALLY retried {} time(s) until quality was achieved.",
                final_state.retry_count
            );
        } else {
            println!(
                "\n⚠️  WARNING: Quality below threshold after {} retries",
                final_state.retry_count
            );
        }
    }

    println!("\n=== ARCHITECTURAL INSIGHT ===");
    println!("This example demonstrates how DashFlow's graph features (cycles,");
    println!("conditional edges) can GUARANTEE quality, not just hope for it.");
    println!("The retry loop is built into the graph structure, not ad-hoc code.");

    Ok(())
}
