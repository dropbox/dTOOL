//! DashFlow Streaming Quality Monitoring Example
//!
//! Demonstrates automatic quality monitoring with LLM-as-judge integrated into DashStream telemetry.
//!
//! # Architecture
//!
//! ```text
//! Agent → Response → QualityMonitor → Judge LLM → Emit Metrics → Kafka
//!                         ↓
//!                    (async, non-blocking)
//! ```
//!
//! # Features
//!
//! - **Automatic Quality Scoring**: Every response judged by LLM
//! - **Non-Blocking**: Quality evaluation runs asynchronously
//! - **Telemetry Integration**: Scores emitted to Kafka
//! - **Alerting**: Low-quality responses trigger alerts
//!
//! # Run Example
//!
//! ```bash
//! # Start Kafka (required)
//! docker run -d --name kafka -p 9092:9092 apache/kafka:latest
//!
//! # Set OpenAI API key
//! export OPENAI_API_KEY="your-key-here"
//!
//! # Run example
//! cargo run --example dashstream_quality_monitoring
//! ```
//!
//! # Expected Output
//!
//! ```text
//! [AGENT] Processing query: "What is machine learning?"
//! [AGENT] Response: "Machine learning is..."
//! [QUALITY] Evaluating response quality...
//! [QUALITY] Score: 0.92 (Acc:0.95, Rel:0.90, Comp:0.91)
//! [LANGSTREAM] Quality metrics emitted to Kafka
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use dashflow_streaming::producer::DashStreamProducer;
use dashflow_streaming::quality::{QualityJudge, QualityMonitor, QualityScore};
use std::sync::Arc;

/// OpenAI-based quality judge implementation
struct OpenAIJudge {
    model: ChatOpenAI,
}

impl OpenAIJudge {
    fn new() -> Self {
        Self {
            model: ChatOpenAI::with_config(Default::default())
                .with_model("gpt-4o-mini")
                .with_temperature(0.0), // Deterministic for consistency
        }
    }
}

#[async_trait::async_trait]
impl QualityJudge for OpenAIJudge {
    async fn judge_response(
        &self,
        query: &str,
        response: &str,
        expected_topics: &[&str],
        context: Option<&str>,
        tool_results: Option<&str>,
    ) -> Result<QualityScore, Box<dyn std::error::Error>> {
        let context_info = context
            .map(|c| format!("\nPrevious Context: {}\n", c))
            .unwrap_or_default();

        let tool_info = tool_results
            .map(|t| format!("\nTool Results: {}\n", t))
            .unwrap_or_default();

        let prompt = format!(
            "You are evaluating an AI assistant's response quality.{}{}\
             User Query: {}\n\
             AI Response: {}\n\
             Expected Topics: {:?}\n\n\
             Evaluate the response on three dimensions (0.0-1.0 scale):\n\n\
             1. **Accuracy** (0.0-1.0): Is the information factually correct?\n\
             2. **Relevance** (0.0-1.0): Does it directly address the user's query?\n\
             3. **Completeness** (0.0-1.0): Does it cover all important aspects?\n\n\
             Respond with ONLY valid JSON in this exact format:\n\
             {{\"accuracy\": 0.9, \"relevance\": 0.95, \"completeness\": 0.85, \"reasoning\": \"Brief explanation\"}}\n\n\
             Important: Respond ONLY with JSON, no additional text.",
            context_info, tool_info, query, response, expected_topics
        );

        let messages = vec![Message::human(prompt)];

        let judge_response = self
            .model
            .generate(&messages, None, None, None, None)
            .await?;

        // Extract JSON from response
        let content = judge_response.generations[0].message.content().as_text();
        let json_str = if content.contains("```json") {
            content
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&content)
                .trim()
        } else if content.contains("```") {
            content.split("```").nth(1).unwrap_or(&content).trim()
        } else {
            content.trim()
        };

        let score: QualityScore = serde_json::from_str(json_str)?;
        Ok(score)
    }
}

/// Simple mock agent that generates responses
async fn mock_agent_response(query: &str) -> String {
    // In real scenario, this would be DashFlow agent with tools
    match query {
        q if q.contains("machine learning") => {
            "Machine learning is a subset of artificial intelligence that enables \
             computers to learn from data without explicit programming. It involves \
             algorithms that improve automatically through experience."
                .to_string()
        }
        q if q.contains("quantum computing") => {
            "Quantum computing leverages quantum mechanics principles to process \
             information using qubits, which can exist in superposition states."
                .to_string()
        }
        _ => format!("I can help answer questions about {}.", query),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 DashFlow Streaming Quality Monitoring Example");
    println!("==========================================\n");

    // Check for OpenAI API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("❌ Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set your OpenAI API key:");
        eprintln!("  export OPENAI_API_KEY=\"your-key-here\"");
        std::process::exit(1);
    }

    // Create DashStream producer (connects to Kafka)
    println!("📡 Connecting to Kafka at localhost:9092...");
    let producer = match DashStreamProducer::new("localhost:9092", "dashstream-events").await {
        Ok(p) => {
            println!("✅ Connected to Kafka\n");
            Arc::new(p)
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to Kafka: {}", e);
            eprintln!("Please start Kafka:");
            eprintln!("  docker run -d --name kafka -p 9092:9092 apache/kafka:latest");
            std::process::exit(1);
        }
    };

    // Create quality judge
    println!("🤖 Initializing quality judge (GPT-4o-mini)...");
    let judge = Arc::new(OpenAIJudge::new());
    println!("✅ Quality judge ready\n");

    // Create quality monitor with 0.95 threshold
    let monitor = QualityMonitor::with_judge(Arc::clone(&producer), 0.95, judge);

    // Test queries
    let test_cases = [
        (
            "What is machine learning?",
            vec!["AI", "algorithms", "data", "learning"],
        ),
        (
            "Explain quantum computing",
            vec!["quantum", "qubits", "superposition"],
        ),
    ];

    println!("📝 Running test queries with quality monitoring...\n");

    for (i, (query, expected_topics)) in test_cases.iter().enumerate() {
        println!("─────────────────────────────────────────────");
        println!("Query #{}: {}", i + 1, query);
        println!("Expected Topics: {:?}", expected_topics);
        println!();

        // Generate response (mock agent)
        println!("[AGENT] Generating response...");
        let response = mock_agent_response(query).await;
        println!("[AGENT] Response: \"{}\"", response);
        println!();

        // Evaluate quality and emit to DashStream
        println!("[QUALITY] Evaluating response quality...");
        let thread_id = format!("thread-{}", i + 1);

        let expected_topics_refs: Vec<&str> = expected_topics.iter().map(|s| s.as_ref()).collect();

        match monitor
            .evaluate_and_emit(
                &thread_id,
                query,
                &response,
                &expected_topics_refs,
                None,
                None,
            )
            .await
        {
            Ok((score, issues)) => {
                println!(
                    "[QUALITY] ✅ Score: {:.2} (Acc:{:.2}, Rel:{:.2}, Comp:{:.2})",
                    score.average(),
                    score.accuracy,
                    score.relevance,
                    score.completeness
                );
                println!("[QUALITY] Reasoning: {}", score.reasoning);

                if !issues.is_empty() {
                    println!(
                        "[QUALITY] ⚠️  Issues detected: {:?}",
                        issues.iter().map(|i| i.as_str()).collect::<Vec<_>>()
                    );
                } else {
                    println!("[QUALITY] ✅ No issues detected");
                }

                if score.average() >= 0.95 {
                    println!("[QUALITY] 🎯 Meets quality threshold (0.95)");
                } else {
                    println!("[QUALITY] ⚠️  Below threshold (0.95), alert triggered");
                }

                println!("[LANGSTREAM] 📤 Quality metrics emitted to Kafka");
            }
            Err(e) => {
                eprintln!("[QUALITY] ❌ Evaluation failed: {}", e);
            }
        }

        println!();
    }

    println!("─────────────────────────────────────────────");
    println!("\n✅ Quality monitoring complete!");
    println!("\n💡 Key Benefits:");
    println!("   • Automatic quality scoring for every response");
    println!("   • Non-blocking evaluation (doesn't slow down agent)");
    println!("   • Real-time metrics streamed to Kafka");
    println!("   • Alerts triggered for low-quality responses");
    println!("   • Production-ready quality gates");

    println!("\n📊 Next Steps:");
    println!("   1. View metrics in Kafka consumer:");
    println!("      docker exec -it kafka kafka-console-consumer.sh \\");
    println!("        --bootstrap-server localhost:9092 \\");
    println!("        --topic dashstream-events --from-beginning");
    println!();
    println!("   2. Use parse_events to analyze quality metrics:");
    println!("      cargo run --bin parse_events -- --input events.bin");
    println!();
    println!("   3. Integrate with your DashFlow agent:");
    println!("      See examples/quality_enforced_agent.rs for graph integration");

    Ok(())
}
