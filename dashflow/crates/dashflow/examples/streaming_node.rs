//! Intra-Node Streaming Telemetry Example
//!
//! This example demonstrates how nodes can emit telemetry during execution,
//! providing visibility into long-running operations, LLM reasoning steps,
//! and internal progress.
//!
//! # Features Demonstrated
//!
//! - **Progress Updates**: Track completion percentage during execution
//! - **Thinking Steps**: Capture LLM chain-of-thought reasoning
//! - **Substeps**: Track internal operations within a node
//! - **Token Streaming**: Stream LLM token generation (simulated)
//! - **Tool Execution**: Track tool call stages
//! - **Metrics**: Emit custom metrics during execution
//! - **Warnings**: Send non-fatal warnings
//!
//! # Prerequisites
//!
//! Start Kafka using Docker Compose:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml up -d
//! ```
//!
//! # Running
//!
//! ```bash
//! cargo run --example streaming_node --features dashstream
//! ```
//!
//! # Monitoring
//!
//! Monitor the Kafka topic to see streaming telemetry:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml exec kafka \
//!     kafka-console-consumer --bootstrap-server localhost:9092 \
//!     --topic dashstream-streaming-demo --from-beginning
//! ```
//!
//! You'll see events in real-time as nodes execute, including:
//! - NodeStart (when node begins)
//! - NodeProgress (progress updates during execution)
//! - NodeThinking (LLM reasoning steps)
//! - NodeSubstep (internal operation completion)
//! - TokenChunk (token streaming)
//! - ToolExecution (tool call tracking)
//! - Metrics (custom metrics)
//! - NodeEnd (when node completes)

use async_trait::async_trait;
use dashflow::node::NodeContext;
use dashflow::{
    DashStreamCallback, DashStreamConfig, Error, MergeableState, Node, StateGraph, END,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ============================================================================
// State Definition
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct AnalysisState {
    query: String,
    analysis_result: String,
    search_results: Vec<String>,
    final_answer: String,
    step_count: i32,
}

impl MergeableState for AnalysisState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            self.query = other.query.clone();
        }
        if !other.analysis_result.is_empty() {
            self.analysis_result = other.analysis_result.clone();
        }
        if !other.final_answer.is_empty() {
            self.final_answer = other.final_answer.clone();
        }
        self.search_results.extend(other.search_results.clone());
        self.step_count = self.step_count.max(other.step_count);
    }
}

// ============================================================================
// Example 1: Query Analyzer with Progress Updates
// ============================================================================

/// Node that analyzes a user query and emits progress updates
struct QueryAnalyzerNode;

#[async_trait]
impl Node<AnalysisState> for QueryAnalyzerNode {
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn execute_with_context(
        &self,
        mut state: AnalysisState,
        ctx: &NodeContext,
    ) -> Result<AnalysisState, Error> {
        println!(
            "\n🔍 [QUERY ANALYZER] Starting analysis of: '{}'",
            state.query
        );

        state.step_count += 1;

        // Step 1: Parse query
        ctx.send_progress("Parsing query structure...", 0.1).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        println!("   ✓ Query parsed");

        // Step 2: Extract intent
        ctx.send_progress("Extracting user intent...", 0.3).await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        println!("   ✓ Intent extracted: Information retrieval");

        // Step 3: Identify entities
        ctx.send_progress("Identifying key entities...", 0.5)
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        println!("   ✓ Entities identified: [topic, context]");

        // Step 4: Analyze complexity
        ctx.send_progress("Analyzing query complexity...", 0.7)
            .await?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        println!("   ✓ Complexity: Medium (requires 2-3 search steps)");

        // Step 5: Finalize analysis
        ctx.send_progress("Finalizing analysis...", 0.9).await?;
        state.analysis_result = "Query analysis complete:\n\
             - Intent: Information retrieval\n\
             - Complexity: Medium\n\
             - Required steps: Search → Synthesize → Answer"
            .to_string();
        tokio::time::sleep(Duration::from_millis(200)).await;

        ctx.send_progress("Analysis complete!", 1.0).await?;
        println!("   ✅ Analysis complete!");

        Ok(state)
    }

    async fn execute(&self, state: AnalysisState) -> Result<AnalysisState, Error> {
        self.execute_with_context(state, &NodeContext::empty())
            .await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Example 2: Search Agent with Tool Execution Tracking
// ============================================================================

/// Node that performs searches and tracks tool execution
struct SearchAgentNode;

#[async_trait]
impl Node<AnalysisState> for SearchAgentNode {
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn execute_with_context(
        &self,
        mut state: AnalysisState,
        ctx: &NodeContext,
    ) -> Result<AnalysisState, Error> {
        println!("\n🔎 [SEARCH AGENT] Performing searches...");

        state.step_count += 1;

        // Simulate 3 search operations with tool execution tracking
        let searches = [
            ("Wikipedia", "General information"),
            ("ArXiv", "Academic papers"),
            ("News", "Recent developments"),
        ];

        for (i, (source, purpose)) in searches.iter().enumerate() {
            let call_id = format!("search_{}", i + 1);
            let progress = (i as f64 + 1.0) / searches.len() as f64;

            // Tool execution: Start
            ctx.send_tool_event(&call_id, &format!("search_{}", source.to_lowercase()), 1, 0)
                .await?;

            ctx.send_substep(&format!("Search {}", source), "starting")
                .await?;
            println!("   🔧 Searching {} for {}", source, purpose);

            tokio::time::sleep(Duration::from_millis(500)).await;

            // Tool execution: Complete
            ctx.send_tool_event(
                &call_id,
                &format!("search_{}", source.to_lowercase()),
                3,
                500_000,
            )
            .await?;

            let result = format!("Results from {} ({} documents)", source, (i + 1) * 3);
            state.search_results.push(result.clone());

            ctx.send_substep(&format!("Search {}", source), "complete")
                .await?;
            ctx.send_progress(
                &format!("Searched {}/{} sources", i + 1, searches.len()),
                progress,
            )
            .await?;

            println!("   ✓ {}", result);

            // Emit metric for result count
            ctx.send_metric(
                &format!("{}_result_count", source.to_lowercase()),
                ((i + 1) * 3) as f64,
                "documents",
            )
            .await?;
        }

        ctx.send_progress("All searches complete!", 1.0).await?;
        println!(
            "   ✅ Search complete: {} results collected",
            state.search_results.len()
        );

        Ok(state)
    }

    async fn execute(&self, state: AnalysisState) -> Result<AnalysisState, Error> {
        self.execute_with_context(state, &NodeContext::empty())
            .await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Example 3: LLM Response Generator with Token Streaming & Thinking
// ============================================================================

/// Node that generates LLM response with token streaming and chain-of-thought
struct LLMResponseNode;

#[async_trait]
impl Node<AnalysisState> for LLMResponseNode {
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn execute_with_context(
        &self,
        mut state: AnalysisState,
        ctx: &NodeContext,
    ) -> Result<AnalysisState, Error> {
        println!("\n🤖 [LLM RESPONSE] Generating response...");

        state.step_count += 1;
        let request_id = uuid::Uuid::new_v4().to_string();

        // Chain-of-thought reasoning
        ctx.send_thinking("Analyzing the user's query and available search results", 1)
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        println!("   💭 Step 1: Analyzing query and results");

        ctx.send_thinking("Identifying key information from search results", 2)
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        println!("   💭 Step 2: Extracting key information");

        ctx.send_thinking(
            "Synthesizing information into a coherent response structure",
            3,
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        println!("   💭 Step 3: Synthesizing response");

        // Token streaming (simulate LLM generation)
        println!("   📝 Streaming response tokens:");

        let response_tokens = vec![
            "Based",
            " on",
            " the",
            " search",
            " results",
            ",",
            " I",
            " can",
            " provide",
            " the",
            " following",
            " answer",
            ":",
            "\n\n",
            "The",
            " information",
            " gathered",
            " from",
            " multiple",
            " sources",
            " indicates",
            " that",
            " your",
            " query",
            " relates",
            " to",
            " a",
            " complex",
            " topic",
            ".",
            " The",
            " key",
            " findings",
            " are",
            ":",
            "\n\n",
            "1",
            ".",
            " Recent",
            " developments",
            " show",
            "...",
            "\n",
            "2",
            ".",
            " Academic",
            " research",
            " suggests",
            "...",
            "\n",
            "3",
            ".",
            " General",
            " information",
            " confirms",
            "...",
        ];

        let mut generated_text = String::new();
        let total_tokens = response_tokens.len();

        for (i, token) in response_tokens.iter().enumerate() {
            ctx.send_token(token, i as u32, i == total_tokens - 1, &request_id)
                .await?;

            generated_text.push_str(token);

            // Update progress
            let progress = (i as f64 + 1.0) / total_tokens as f64;
            ctx.send_progress(
                &format!("Generated {}/{} tokens", i + 1, total_tokens),
                progress,
            )
            .await?;

            // Simulate token generation delay
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        state.final_answer = generated_text.trim().to_string();

        println!("\n   ✅ Response generation complete!");

        // Emit token generation metrics
        ctx.send_metric("tokens_generated", total_tokens as f64, "tokens")
            .await?;
        ctx.send_metric(
            "generation_time_ms",
            (total_tokens * 30) as f64,
            "milliseconds",
        )
        .await?;

        Ok(state)
    }

    async fn execute(&self, state: AnalysisState) -> Result<AnalysisState, Error> {
        self.execute_with_context(state, &NodeContext::empty())
            .await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Example 4: Non-Streaming Node (for comparison)
// ============================================================================

/// Traditional node without streaming (black box)
async fn traditional_validation_node(mut state: AnalysisState) -> Result<AnalysisState, Error> {
    println!("\n✔️  [VALIDATOR] Validating response (non-streaming)...");

    state.step_count += 1;

    // This node does work but emits no telemetry during execution
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("   ✅ Validation complete!");

    Ok(state)
}

// ============================================================================
// Main Application
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║       Intra-Node Streaming Telemetry Example                  ║");
    println!("║                                                                ║");
    println!("║  Demonstrates real-time visibility into node execution        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // Create DashStream configuration
    let config = DashStreamConfig {
        bootstrap_servers: "localhost:9092".to_string(),
        topic: "dashstream-streaming-demo".to_string(),
        tenant_id: "streaming-demo-tenant".to_string(),
        thread_id: format!("streaming-session-{}", uuid::Uuid::new_v4()),
        enable_state_diff: true,
        compression_threshold: 512,
        max_state_diff_size: 10 * 1024 * 1024, // 10MB limit for state diffs
        ..Default::default()
    };

    println!("📡 Configuration:");
    println!("   Bootstrap: {}", config.bootstrap_servers);
    println!("   Topic: {}", config.topic);
    println!("   Thread ID: {}", config.thread_id);
    println!();

    // Create DashStream callback
    println!("🔌 Connecting to Kafka...");
    let callback = DashStreamCallback::<AnalysisState>::with_config(config.clone())
        .await
        ?;

    println!("✅ Connected successfully!");
    println!();

    // Clone callback for flushing later
    let callback_for_flush = callback.clone();

    // Build the graph
    println!("🔧 Building StateGraph...");
    let mut graph = StateGraph::new();

    // Add streaming nodes
    graph.add_node("analyzer", QueryAnalyzerNode);
    graph.add_node("search", SearchAgentNode);
    graph.add_node("llm", LLMResponseNode);

    // Add traditional non-streaming node
    graph.add_node_from_fn("validate", |state: AnalysisState| {
        Box::pin(traditional_validation_node(state))
    });

    // Define graph flow
    graph.add_edge("analyzer", "search");
    graph.add_edge("search", "llm");
    graph.add_edge("llm", "validate");
    graph.add_edge("validate", END);
    graph.set_entry_point("analyzer");

    println!("✅ Graph structure:");
    println!("   analyzer → search → llm → validate → END");
    println!();

    // Compile with callback
    let compiled = graph
        .compile()?
        .with_callback(callback);

    println!("✅ Graph compiled with DashStream callback");
    println!();

    // Execute graph
    println!("▶️  Starting graph execution with streaming telemetry...");
    println!("{}", "=".repeat(68));

    let initial_state = AnalysisState {
        query: "What are the latest developments in Rust async programming?".to_string(),
        analysis_result: String::new(),
        search_results: vec![],
        final_answer: String::new(),
        step_count: 0,
    };

    let result = compiled
        .invoke(initial_state)
        .await
        ?
        .final_state;

    // Display results
    println!();
    println!("{}", "=".repeat(68));
    println!("\n📊 Execution Summary:");
    println!("   Total steps: {}", result.step_count);
    println!("   Search results: {} sources", result.search_results.len());
    println!();
    println!("📝 Final Answer:");
    println!("{}", "-".repeat(68));
    println!("{}", result.final_answer);
    println!("{}", "-".repeat(68));
    println!();

    // Flush Kafka messages
    println!("🔄 Flushing telemetry to Kafka...");
    callback_for_flush
        .flush()
        .await
        ?;

    println!("✅ All telemetry flushed successfully!");
    println!();

    // Display what was captured
    println!("📡 Telemetry Events Sent:");
    println!("   ✓ 4 NodeStart events (one per node)");
    println!("   ✓ 4 NodeEnd events (one per node)");
    println!("   ✓ ~15 NodeProgress events (from analyzer, search, llm)");
    println!("   ✓ 3 NodeThinking events (from llm)");
    println!("   ✓ 6 NodeSubstep events (from search)");
    println!("   ✓ 6 ToolExecution events (3 start + 3 complete from search)");
    println!("   ✓ ~60 TokenChunk events (from llm)");
    println!("   ✓ 5 Metrics events (result counts + generation stats)");
    println!("   ✓ Multiple StateDiff events (incremental state updates)");
    println!();

    println!("💡 Key Observations:");
    println!();
    println!("   • STREAMING NODES (analyzer, search, llm):");
    println!("     - Emit telemetry during execution");
    println!("     - Provide real-time visibility");
    println!("     - Enable progress monitoring");
    println!("     - Support debugging and observability");
    println!();
    println!("   • NON-STREAMING NODE (validate):");
    println!("     - Only emits NodeStart and NodeEnd");
    println!("     - No visibility during execution (black box)");
    println!("     - Still works correctly (backward compatible)");
    println!();

    println!("🔍 Monitor telemetry in real-time:");
    println!("   docker-compose -f docker-compose-kafka.yml exec kafka \\");
    println!("       kafka-console-consumer --bootstrap-server localhost:9092 \\");
    println!("       --topic {} --from-beginning", config.topic);
    println!();

    println!("✨ Example complete!");

    Ok(())
}
