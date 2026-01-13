//! Quality Gate Demonstration
//!
//! This example demonstrates how quality gates achieve 100% quality through
//! self-correcting retry loops with feedback.
//!
//! ## Pattern
//!
//! ```text
//! Query → Agent → Response → Judge → Quality Check
//!                               ↓
//!                          < threshold?
//!                              YES ↓
//!                     Add feedback → Retry
//!                              NO ↓
//!                           Return ✅
//! ```
//!
//! ## Example Output
//!
//! ```text
//! 🎯 Quality Gate Demonstration
//! ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//! Query: What is Send and Sync in Rust?
//!
//! 🔄 Attempt 1 of 4
//! 📝 Query: What is Send and Sync in Rust?
//! 📤 Response: I couldn't find that information...
//! ⚖️  Quality: 0.70 (threshold: 0.90)
//!    Accuracy: 0.70, Relevance: 0.70, Completeness: 0.70
//! 🔁 Retrying with feedback...
//!
//! 🔄 Attempt 2 of 4
//! 📝 Query: What is Send and Sync in Rust? [FEEDBACK] ...
//! 📤 Response: Send and Sync are marker traits...
//! ⚖️  Quality: 0.95 (threshold: 0.90)
//!    Accuracy: 0.95, Relevance: 0.95, Completeness: 0.95
//! ✅ Quality check passed!
//!
//! ✅ Final Response (Quality Guaranteed ≥0.90):
//! Send and Sync are marker traits in Rust. Send means...
//!
//! 🎉 SUCCESS: Quality gate achieved 100% quality through self-correction!
//! ```

use dashflow_streaming::quality::{QualityJudge, QualityScore};
use dashflow_streaming::quality_gate::{QualityConfig, QualityGate};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Mock agent that intentionally produces low-quality responses on first attempt
///
/// This simulates a real agent that might sometimes produce insufficient responses.
/// The quality gate detects this and retries with feedback.
struct BadAgent {
    attempt: Arc<AtomicU32>,
}

impl BadAgent {
    fn new() -> Self {
        Self {
            attempt: Arc::new(AtomicU32::new(0)),
        }
    }

    async fn execute(&self, query: &str) -> Result<String, Box<dyn std::error::Error>> {
        let attempt = self.attempt.fetch_add(1, Ordering::SeqCst);

        match attempt {
            0 => {
                // First attempt: BAD response (quality ~0.70)
                // This simulates an agent that fails to answer properly
                Ok("I couldn't find that information. Maybe try rephrasing?".to_string())
            }
            1 => {
                // Second attempt: GOOD response (quality ~0.95)
                // With feedback, the agent provides a proper answer
                Ok("Send and Sync are marker traits in Rust. Send means a type can be safely transferred between threads (ownership can be moved to another thread). Sync means a type can be safely shared between threads via references (&T can be sent to another thread). Most types are both Send and Sync. Notable exceptions: Rc is not Send or Sync, Cell and RefCell are not Sync.".to_string())
            }
            _ => {
                // Subsequent attempts: always good
                Ok(format!("High quality response to: {}", query))
            }
        }
    }
}

/// Mock judge that evaluates response quality
///
/// In production, this would be an LLM-as-judge (e.g., GPT-4 or Claude).
/// For this demo, we use pattern matching to simulate quality evaluation.
struct MockJudge;

#[async_trait::async_trait]
impl QualityJudge for MockJudge {
    async fn judge_response(
        &self,
        _query: &str,
        response: &str,
        _expected_topics: &[&str],
        _context: Option<&str>,
        _tool_results: Option<&str>,
    ) -> Result<QualityScore, Box<dyn std::error::Error>> {
        // Evaluate based on response patterns
        let quality = if response.contains("couldn't find") || response.contains("Maybe try") {
            // Low quality: Agent failed to answer
            0.70
        } else if response.contains("Send and Sync are marker traits") {
            // High quality: Comprehensive answer with details
            0.95
        } else if response.len() > 50 && response.contains("Rust") {
            // Good quality: Has some substance
            0.92
        } else {
            // Medium quality
            0.85
        };

        Ok(QualityScore {
            accuracy: quality,
            relevance: quality,
            completeness: quality,
            reasoning: format!(
                "Quality: {:.2}. Response length: {} chars. Contains key terms: {}",
                quality,
                response.len(),
                response.contains("Send") && response.contains("Sync")
            ),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Quality Gate Demonstration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Create mock agent and judge
    let agent = Arc::new(BadAgent::new());
    let judge = Arc::new(MockJudge);

    // Configure quality gate
    let config = QualityConfig::with_judge(judge)
        .quality_threshold(0.90) // Require 0.90+ quality
        .max_retries(3) // Allow up to 3 retries
        .verbose(true); // Show detailed retry information

    let gate = QualityGate::new(config);

    // Execute with quality guarantee
    let query = "What is Send and Sync in Rust?";

    println!("Query: {}\n", query);

    let response = gate
        .execute_with_quality_guarantee(query, |q| {
            let agent_clone = Arc::clone(&agent);
            Box::pin(async move { agent_clone.execute(&q).await })
        })
        .await?;

    println!("\n✅ Final Response (Quality Guaranteed ≥0.90):");
    println!("{}\n", response);

    println!("🎉 SUCCESS: Quality gate achieved 100% quality through self-correction!");
    println!("\n📊 Key Metrics:");
    println!("   • Attempt 1: Quality 0.70 → RETRY");
    println!("   • Attempt 2: Quality 0.95 → PASS");
    println!("   • Final: 100% quality guaranteed (response meets threshold)");
    println!("\n💡 How It Works:");
    println!("   1. Agent produces low-quality response");
    println!("   2. Judge detects quality < 0.90");
    println!("   3. Feedback added to query");
    println!("   4. Agent retries with improved response");
    println!("   5. Judge confirms quality ≥ 0.90");
    println!("   6. Success!");

    Ok(())
}
