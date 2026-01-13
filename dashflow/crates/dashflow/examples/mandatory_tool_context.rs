// INNOVATION 14: Mandatory Tool Results in Context
//
// Problem: Tool results appear once in conversation, then can be "forgotten" by LLM
// Solution: Re-inject tool results into context at EVERY agent turn
//
// Architecture:
//   query → agent → prepare_context (inject tools) → response → judge → good? → END
//                        ↑                                          ↓
//                        └─────────── retry loop ──────────────────┘
//
// At each iteration: context = tool_results + conversation_history

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ContextManagedState {
    query: String,
    response: String,
    tool_results: Option<String>,
    context_messages: Vec<String>, // Simplified: strings instead of Message objects
    retry_count: usize,
    quality_score: f64,
}

impl MergeableState for ContextManagedState {
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
        self.context_messages.extend(other.context_messages.clone());
        self.retry_count = self.retry_count.max(other.retry_count);
        self.quality_score = self.quality_score.max(other.quality_score);
    }
}

// Node 1: Prepare context with tool results re-injected
fn prepare_context_node(
    mut state: ContextManagedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ContextManagedState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[PREPARE_CONTEXT] Re-injecting tool results into context...");

        let mut context = Vec::new();

        // Add original query
        context.push(format!("USER: {}", state.query));

        // If we have tool results, RE-INJECT them before this turn
        if let Some(results) = &state.tool_results {
            println!("[PREPARE_CONTEXT] Tool results exist - injecting prominent reminder");

            // Add emphatic system message reminding LLM about tool results
            let reminder = format!(
                "SYSTEM [CRITICAL REMINDER]: Your search tool found the following information:\n\
             \n\
             {}\n\
             \n\
             You MUST incorporate this information into your response. \
             Do NOT say 'couldn't find' when data is provided above. \
             Use the specific details from the search results.",
                results
            );

            context.push(reminder);

            // Also add the original query again to keep it fresh
            if state.retry_count > 0 {
                context.push(format!("SYSTEM: Original query reminder: {}", state.query));
            }
        } else {
            println!("[PREPARE_CONTEXT] No tool results yet");
        }

        println!("[PREPARE_CONTEXT] Context size: {} messages", context.len());

        state.context_messages = context;
        Ok(state)
    })
}

// Node 2: Agent generates response (simulated)
fn agent_node(
    mut state: ContextManagedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ContextManagedState>> + Send>,
> {
    Box::pin(async move {
        println!(
            "\n[AGENT] Generating response (iteration {})...",
            state.retry_count + 1
        );
        println!(
            "[AGENT] Context contains {} messages",
            state.context_messages.len()
        );

        // Simulate agent behavior based on context visibility
        if state.retry_count == 0 {
            // First attempt: Agent ignores tool results (simulates the bug)
            println!("[AGENT] First attempt - simulating tool ignorance bug");
            state.response = format!("I couldn't find any information about '{}'.", state.query);

            // Simulate tool call that found results
            state.tool_results = Some(format!(
                "Documentation found for '{}':\n\
             - Feature X is available in version 2.0\n\
             - Configuration: set feature_x = true\n\
             - API endpoint: /api/v2/feature-x",
                state.query
            ));
        } else if state.retry_count == 1 {
            // Second attempt: After context re-injection, agent sees the results
            println!("[AGENT] Retry #1 - tool results NOW VISIBLE in context");

            // Check if context includes tool results reminder
            let has_reminder = state
                .context_messages
                .iter()
                .any(|msg| msg.contains("CRITICAL REMINDER"));

            if has_reminder {
                println!("[AGENT] ✓ Tool results reminder found in context!");
                state.response = format!(
                    "Based on the documentation search:\n\
                 \n\
                 Feature X is available in version 2.0 of {}. \
                 To enable it, set feature_x = true in your configuration. \
                 You can access it via the API endpoint /api/v2/feature-x.",
                    state.query
                );
            } else {
                println!("[AGENT] ✗ Tool results reminder NOT found - context injection failed");
                state.response = "I still couldn't find the information.".to_string();
            }
        } else {
            // Subsequent attempts: Use refined context
            println!(
                "[AGENT] Retry #{} - using refined context",
                state.retry_count
            );
            state.response =
                "Comprehensive response based on repeated context injection.".to_string();
        }

        println!("[AGENT] Response: {}", state.response);

        Ok(state)
    })
}

// Node 3: Judge response quality
fn judge_node(
    mut state: ContextManagedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ContextManagedState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[JUDGE] Evaluating response quality...");

        // Detect tool ignorance patterns
        let ignorance_patterns = [
            "couldn't find",
            "wasn't able to find",
            "no information available",
        ];

        let has_ignorance = ignorance_patterns
            .iter()
            .any(|pattern| state.response.contains(pattern));

        // Check if tool results exist but were ignored
        let tool_results_ignored = state.tool_results.is_some() && has_ignorance;

        if tool_results_ignored {
            println!("[JUDGE] ❌ CRITICAL: Tool results exist but response says 'couldn't find'");
            state.quality_score = 0.20; // Very low quality
        } else if state.response.len() > 100 && state.tool_results.is_some() {
            println!("[JUDGE] ✓ Good response using tool results");
            state.quality_score = 0.95; // High quality
        } else {
            println!("[JUDGE] Response quality: moderate");
            state.quality_score = 0.70;
        }

        println!("[JUDGE] Quality score: {:.2}", state.quality_score);

        Ok(state)
    })
}

// Conditional: Decide next step based on quality
fn decide_next(state: &ContextManagedState) -> String {
    println!("\n[ROUTER] Deciding next step...");
    println!(
        "[ROUTER] Quality: {:.2}, Retries: {}",
        state.quality_score, state.retry_count
    );

    if state.quality_score >= 0.90 {
        println!("[ROUTER] ✓ Quality acceptable → END");
        END.to_string()
    } else if state.retry_count >= 3 {
        println!("[ROUTER] ⚠ Max retries reached → END (give up)");
        END.to_string()
    } else {
        println!("[ROUTER] ↻ Quality low → RETRY with context re-injection");
        "prepare_context".to_string()
    }
}

// Node 4: Increment retry counter
fn increment_retry(
    mut state: ContextManagedState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ContextManagedState>> + Send>,
> {
    Box::pin(async move {
        state.retry_count += 1;
        Ok(state)
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sep = "=".repeat(80);
    println!("{}", sep);
    println!("INNOVATION 14: Mandatory Tool Results in Context");
    println!("{}", sep);

    println!("\n📋 Problem:");
    println!("   - Tool results appear once in conversation");
    println!("   - LLM can 'forget' about them in subsequent turns");
    println!("   - Says 'couldn't find' even when data exists");

    println!("\n💡 Solution:");
    println!("   - Re-inject tool results into context at EVERY turn");
    println!("   - Add prominent system message reminding LLM");
    println!("   - Keep tool results visible throughout retry loop");

    println!("\n🏗️ Architecture:");
    println!("   query → prepare_context → agent → judge → retry?");
    println!("             ↑                               ↓");
    println!("             └────── re-inject tools ────────┘");

    println!("\n{}", sep);
    println!("TEST 1: Tool Results Ignored → Context Re-injection → Fixed");
    println!("{}", sep);

    // Build graph
    let mut graph = StateGraph::<ContextManagedState>::new();

    graph.add_node_from_fn("prepare_context", prepare_context_node);
    graph.add_node_from_fn("agent", agent_node);
    graph.add_node_from_fn("judge", judge_node);
    graph.add_node_from_fn("increment_retry", increment_retry);

    graph.set_entry_point("prepare_context");
    graph.add_edge("prepare_context", "agent");
    graph.add_edge("agent", "judge");

    // Set up routing after judge
    let mut judge_routes = std::collections::HashMap::new();
    judge_routes.insert("prepare_context".to_string(), "increment_retry".to_string());
    judge_routes.insert(END.to_string(), END.to_string());

    graph.add_conditional_edges("judge", decide_next, judge_routes);
    graph.add_edge("increment_retry", "prepare_context");

    let app = graph.compile()?;

    // Test case: User asks about feature, tool finds it, but agent ignores
    let initial_state = ContextManagedState {
        query: "feature X documentation".to_string(),
        response: String::new(),
        tool_results: None,
        context_messages: vec![],
        retry_count: 0,
        quality_score: 0.0,
    };

    println!("\n📥 User query: Tell me about feature X in our documentation");

    // Run graph
    let execution_result = app.invoke(initial_state).await?;
    let current_state = execution_result.final_state;

    println!("\n{}", sep);
    println!("FINAL RESULT");
    println!("{}", sep);
    println!("\n📊 Statistics:");
    println!("   - Retry count: {}", current_state.retry_count);
    println!("   - Final quality: {:.2}", current_state.quality_score);

    println!("\n📝 Final response:");
    println!("{}", current_state.response);

    println!("\n{}", sep);
    println!("ANALYSIS");
    println!("{}", sep);

    println!("\n✓ Context re-injection FORCES tool visibility:");
    println!("   1. First attempt: Agent ignores tool results → quality 0.20");
    println!("   2. Judge detects: 'couldn't find' + tool results exist = FAILED");
    println!("   3. Retry triggered: prepare_context re-injects tool results");
    println!("   4. Second attempt: Agent sees CRITICAL REMINDER in context");
    println!("   5. Agent uses tool results → quality 0.95 → SUCCESS");

    println!("\n💡 Key Benefits:");
    println!("   ✓ Tool results never forgotten across retry cycles");
    println!("   ✓ Prominent system message keeps data visible");
    println!("   ✓ Automatic correction without model changes");
    println!("   ✓ Works with ANY model (context management, not prompting)");

    println!("\n🎯 Impact on 100% Quality Goal:");
    println!("   - Eliminates 'couldn't find' errors when data exists");
    println!("   - Ensures tool results always incorporated");
    println!("   - Architectural solution (not prompt engineering)");
    println!("   - Combines with other innovations (judge, retry, etc.)");

    // Verify success
    if current_state.quality_score >= 0.90 {
        println!("\n✅ SUCCESS: Context re-injection fixed tool ignorance!");
    } else {
        println!("\n❌ FAILED: Quality still below threshold");
    }

    println!("\n{}", sep);

    Ok(())
}
