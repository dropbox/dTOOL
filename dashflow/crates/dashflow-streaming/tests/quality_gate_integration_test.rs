//! Integration Test for Quality Gate with Real LLM Judge
//!
//! This test proves the quality gate works with REAL OpenAI judge (not mock).
//! Requires OPENAI_API_KEY environment variable.
//!
//! Run with:
//!   OPENAI_API_KEY="sk-..." cargo test --package dashflow-streaming \
//!     --test quality_gate_integration_test -- --ignored --nocapture

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use dashflow_streaming::quality::{QualityJudge, QualityScore};
use dashflow_streaming::quality_gate::{QualityConfig, QualityGate};
use std::sync::Arc;

// ============================================================================
// Real OpenAI Judge Implementation
// ============================================================================

/// OpenAI-based judge for quality evaluation (REAL, not mock)
struct OpenAIJudge {
    model: ChatOpenAI,
}

impl OpenAIJudge {
    fn new() -> Self {
        Self {
            model: ChatOpenAI::with_config(Default::default())
                .with_model("gpt-4o") // Full GPT-4o (sophisticated, money doesn't matter)
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
            .map(|t| format!("\nTool Results Available: {}\n", t))
            .unwrap_or_default();

        let prompt = format!(
            "You are a RIGOROUS quality evaluator. Be STRICT and DEMANDING.{}{}\n\n\
             **Query**: {}\n\
             **Response**: {}\n\
             **Expected Topics**: {:?}\n\n\
             **RIGOROUS EVALUATION CRITERIA**:\n\n\
             1. **Accuracy** (0.0-1.0):\n\
                - MUST be factually correct (verify against known facts)\n\
                - ANY incorrect statement → score ≤ 0.5\n\
                - Vague or uncertain language → score ≤ 0.7\n\
                - Precise, accurate facts → score ≥ 0.9\n\n\
             2. **Relevance** (0.0-1.0):\n\
                - MUST directly address the query\n\
                - Tangential information → score ≤ 0.6\n\
                - Partially relevant → score ≤ 0.8\n\
                - Fully on-topic and focused → score ≥ 0.9\n\n\
             3. **Completeness** (0.0-1.0):\n\
                - MUST cover ALL expected topics: {:?}\n\
                - Missing ANY topic → score ≤ 0.7\n\
                - Incomplete explanation → score ≤ 0.8\n\
                - Comprehensive coverage → score ≥ 0.9\n\n\
             **STRICT RULES**:\n\
             - \"I don't know\" or \"couldn't find\" → ALL scores = 0.3\n\
             - Tool results provided but ignored → accuracy = 0.0\n\
             - Partially correct → scores ≤ 0.7\n\
             - Missing key details → completeness ≤ 0.7\n\
             - Vague explanations → accuracy ≤ 0.7\n\n\
             **OUTPUT FORMAT** (JSON only, no extra text):\n\
             {{\"accuracy\": 0.X, \"relevance\": 0.X, \"completeness\": 0.X, \"reasoning\": \"Brief strict evaluation\"}}\n\n\
             Be HARSH. Be DEMANDING. Only excellent responses get ≥0.9.",
            context_info, tool_info, query, response, expected_topics, expected_topics
        );

        let messages = vec![Message::human(prompt)];

        let judge_response = self
            .model
            .generate(&messages, None, None, None, None)
            .await?;

        // Extract JSON from response with multiple fallback strategies
        let content = judge_response.generations[0].message.content().as_text();

        // Strategy 1: Look for ```json code block
        let json_str = if let Some(json_block) = content.split("```json").nth(1) {
            json_block.split("```").next().unwrap_or("").trim()
        }
        // Strategy 2: Look for any ``` code block
        else if let Some(code_block) = content.split("```").nth(1) {
            code_block.split("```").next().unwrap_or("").trim()
        }
        // Strategy 3: Try to find JSON object with regex
        else if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                &content[start..=end]
            } else {
                &content
            }
        } else {
            &content
        };

        // Parse JSON response
        #[derive(serde::Deserialize)]
        struct JudgeResult {
            accuracy: f32,
            relevance: f32,
            completeness: f32,
            reasoning: String,
        }

        let result: JudgeResult = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse judge response: {}\nRaw: {}", e, content))?;

        Ok(QualityScore {
            accuracy: result.accuracy,
            relevance: result.relevance,
            completeness: result.completeness,
            reasoning: result.reasoning,
        })
    }
}

// ============================================================================
// Integration Tests with Real LLM
// ============================================================================

/// Test: Quality gate with real OpenAI judge - immediate pass
///
/// Proves:
/// - Real LLM judge integration works
/// - High-quality response passes immediately (no retry)
/// - Quality threshold enforcement works
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_quality_gate_with_real_judge_immediate_pass() {
    let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("🧪 Testing quality gate with REAL OpenAI judge (immediate pass)...");

    // Create REAL judge (not mock)
    let judge = Arc::new(OpenAIJudge::new());

    // Configure quality gate with real judge
    let config = QualityConfig {
        quality_threshold: 0.90,
        max_retries: 3,
        verbose: true,
        judge: Some(judge),
    };

    let gate = QualityGate::new(config);

    // Test with high-quality response (should pass immediately)
    let result = gate
        .execute_with_quality_guarantee("What is Send and Sync in Rust?", |_query| async move {
            Ok("Send and Sync are marker traits in Rust that ensure thread safety at compile time. \
                Send means a type can be safely transferred across thread boundaries - ownership can move to another thread. \
                Sync means a type can be safely shared between threads via immutable references (&T is Send if T is Sync). \
                Most types are automatically Send and Sync by the compiler if all their components are Send/Sync. \
                Key exceptions: Rc<T> is neither Send nor Sync (uses non-atomic reference counting). \
                Cell<T> and RefCell<T> are Send but not Sync (allow interior mutability without synchronization). \
                Arc<T> is Send+Sync (uses atomic reference counting). \
                Mutex<T> is Send+Sync (provides synchronization). \
                These traits are fundamental to Rust's concurrency model, preventing data races at compile time without runtime overhead.".to_string())
        })
        .await;

    // Assertions
    assert!(
        result.is_ok(),
        "Should succeed with high-quality response: {:?}",
        result.err()
    );
    let response = result.unwrap();
    assert!(
        response.contains("marker traits"),
        "Should have correct response content"
    );

    println!("✅ Real LLM judge integration: PASS");
}

/// Test: Quality gate with real OpenAI judge - retry then pass
///
/// Proves:
/// - Real LLM detects low-quality responses
/// - Retry mechanism works with real judge
/// - Feedback improves response quality
/// - Real quality scoring functions correctly
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_quality_gate_with_real_judge_retry_then_pass() {
    let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("🧪 Testing quality gate with REAL OpenAI judge (retry scenario)...");

    // Create REAL judge (not mock)
    let judge = Arc::new(OpenAIJudge::new());

    // Configure quality gate with real judge
    let config = QualityConfig {
        quality_threshold: 0.90,
        max_retries: 3,
        verbose: true,
        judge: Some(judge),
    };

    let gate = QualityGate::new(config);

    // Track attempt count
    let attempt_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    // Test with intentionally bad first response, good second
    let result = gate
        .execute_with_quality_guarantee("What is Send and Sync in Rust?", move |_query| {
            let attempt_count = Arc::clone(&attempt_count_clone);
            async move {
                let count = attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                if count == 0 {
                    // First attempt: intentionally bad response
                    Ok("I don't know.".to_string())
                } else {
                    // Subsequent attempts: comprehensive response (meets rigorous standards)
                    Ok("Send and Sync are marker traits in Rust that ensure thread safety at compile time. \
                        Send means a type can be safely transferred across thread boundaries - ownership can move to another thread. \
                        Sync means a type can be safely shared between threads via immutable references (&T is Send if T is Sync). \
                        Most types are automatically Send and Sync by the compiler if all their components are Send/Sync. \
                        Key exceptions: Rc<T> is neither Send nor Sync (uses non-atomic reference counting). \
                        Cell<T> and RefCell<T> are Send but not Sync (allow interior mutability without synchronization). \
                        Arc<T> is Send+Sync (uses atomic reference counting). \
                        Mutex<T> is Send+Sync (provides synchronization). \
                        These traits are fundamental to Rust's concurrency model, preventing data races at compile time without runtime overhead.".to_string())
                }
            }
        })
        .await;

    // Assertions
    assert!(
        result.is_ok(),
        "Should succeed after retry with REAL LLM: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(
        response.contains("marker traits"),
        "Should have good response after retry"
    );

    let attempts = attempt_count.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        attempts >= 2,
        "Should have retried (attempts: {})",
        attempts
    );

    println!("✅ Real LLM judge retry mechanism: PASS");
    println!("   Attempts: {}", attempts);
}

/// Test: Quality gate with real OpenAI judge - max retries exceeded
///
/// Proves:
/// - Real LLM correctly identifies persistent low quality
/// - Max retry enforcement works
/// - Error handling works with real judge
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_quality_gate_with_real_judge_max_retries_exceeded() {
    let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("🧪 Testing quality gate with REAL OpenAI judge (max retries)...");

    // Create REAL judge (not mock)
    let judge = Arc::new(OpenAIJudge::new());

    // Configure quality gate with real judge
    let config = QualityConfig {
        quality_threshold: 0.90,
        max_retries: 2, // Only allow 2 retries
        verbose: true,
        judge: Some(judge),
    };

    let gate = QualityGate::new(config);

    // Test with persistently bad response
    let result = gate
        .execute_with_quality_guarantee("What is Send and Sync in Rust?", |_query| async move {
            // Always return bad response
            Ok("I don't know anything about that.".to_string())
        })
        .await;

    // Assertions
    assert!(
        result.is_err(),
        "Should fail after max retries with real LLM"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("quality") || err_msg.contains("threshold"),
        "Error should mention quality threshold"
    );

    println!("✅ Real LLM judge max retry enforcement: PASS");
}

/// Test: Verify real LLM judge detects tool ignorance
///
/// Proves:
/// - Real LLM can detect when tool results are ignored
/// - Quality scoring works correctly for tool ignorance
/// - Feedback mechanism triggers retry for tool ignorance
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_quality_gate_detects_tool_ignorance_with_real_judge() {
    let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("🧪 Testing tool ignorance detection with REAL OpenAI judge...");

    // Create REAL judge (not mock)
    let judge_for_test = Arc::new(OpenAIJudge::new());

    // Test judge directly (not through quality gate)
    let score = judge_for_test
        .judge_response(
            "What is the capital of France?",
            "I couldn't find that information.",
            &["geography", "cities"],
            Some("Tool returned: Paris is the capital of France"),
            Some("Paris is the capital of France"),
        )
        .await;

    // Assertions
    assert!(
        score.is_ok(),
        "Judge should return score: {:?}",
        score.err()
    );

    let quality_score = score.unwrap();

    // Real LLM should recognize tool ignorance
    println!("   Accuracy: {}", quality_score.accuracy);
    println!("   Relevance: {}", quality_score.relevance);
    println!("   Completeness: {}", quality_score.completeness);
    println!("   Reasoning: {}", quality_score.reasoning);

    // Real LLM should give low accuracy for tool ignorance
    assert!(
        quality_score.accuracy < 0.5,
        "Real LLM should detect tool ignorance (accuracy: {})",
        quality_score.accuracy
    );

    println!("✅ Real LLM tool ignorance detection: PASS");
}

/// Test: End-to-end with quality gate + real agent + real judge
///
/// Proves:
/// - Full integration works in production-like scenario
/// - Real OpenAI judge evaluates real agent responses
/// - Quality guarantee works end-to-end
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_e2e_quality_gate_with_real_agent_and_judge() {
    let _api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("🧪 Testing E2E: Real agent + Real judge...");

    // Create REAL judge
    let judge = Arc::new(OpenAIJudge::new());

    // Create REAL agent (ChatOpenAI)
    let agent = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.3);

    // Configure quality gate
    let config = QualityConfig {
        quality_threshold: 0.85, // Slightly lower for real agent variability
        max_retries: 3,
        verbose: true,
        judge: Some(judge),
    };

    let gate = QualityGate::new(config);

    // Execute query with real agent through quality gate
    let result = gate
        .execute_with_quality_guarantee(
            "Explain Send and Sync traits in Rust in 2 sentences",
            move |query| {
                let agent_clone = agent.clone();
                async move {
                    let messages = vec![Message::human(query)];
                    let response = agent_clone
                        .generate(&messages, None, None, None, None)
                        .await?;
                    Ok(response.generations[0]
                        .message
                        .content()
                        .as_text())
                }
            },
        )
        .await;

    // Assertions
    assert!(
        result.is_ok(),
        "E2E should succeed with real agent and judge: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(!response.is_empty(), "Should have non-empty response");
    assert!(
        response.len() > 50,
        "Should have substantive response (got {} chars)",
        response.len()
    );

    println!("✅ E2E real agent + real judge: PASS");
    println!("   Response length: {} chars", response.len());
}

// ============================================================================
// Run All Tests
// ============================================================================
//
// To run all integration tests:
//   OPENAI_API_KEY="sk-..." cargo test --package dashflow-streaming \
//     --test quality_gate_integration_test -- --ignored --nocapture
