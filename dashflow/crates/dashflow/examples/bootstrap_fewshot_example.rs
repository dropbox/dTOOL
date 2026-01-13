//! BootstrapFewShot Optimization Example
//!
//! This example demonstrates the concept of BootstrapFewShot optimization.
//! BootstrapFewShot automatically generates few-shot demonstrations by:
//! 1. Running the LLM on training examples
//! 2. Collecting successful predictions (high scores)
//! 3. Using those examples as few-shot demos in future prompts
//!
//! This is a simplified conceptual example. For production use, see the
//! integration tests in crates/dashflow/tests/dashoptimize_tests.rs
//!
//! Run with: cargo run --package dashflow --example bootstrap_fewshot_example

use dashflow::optimize::{exact_match, make_signature, BootstrapFewShot, MetricFn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Example state for question answering
#[derive(Clone, Debug, Serialize, Deserialize)]
struct QAState {
    question: String,
    answer: String,
}

fn main() -> dashflow::Result<()> {
    println!("=== BootstrapFewShot Optimization - Concept Demo ===\n");

    // 1. Create signature for question answering
    let signature = make_signature("question -> answer", "Answer questions accurately")?;
    println!(
        "✓ Created signature: {} input fields, {} output fields",
        signature.input_fields.len(),
        signature.output_fields.len()
    );

    // 2. Prepare training data
    let trainset = [
        QAState {
            question: "What is 2+2?".to_string(),
            answer: "4".to_string(),
        },
        QAState {
            question: "What is the capital of France?".to_string(),
            answer: "Paris".to_string(),
        },
        QAState {
            question: "What color is the sky?".to_string(),
            answer: "blue".to_string(),
        },
        QAState {
            question: "What is 10-5?".to_string(),
            answer: "5".to_string(),
        },
        QAState {
            question: "How many days in a week?".to_string(),
            answer: "7".to_string(),
        },
    ];
    println!("✓ Prepared training set: {} examples\n", trainset.len());

    // 3. Define metric (exact match)
    let metric: MetricFn<QAState> = Arc::new(|expected: &QAState, predicted: &QAState| {
        Ok(exact_match(&expected.answer, &predicted.answer))
    });
    println!("✓ Defined metric: exact_match\n");

    // 4. Create BootstrapFewShot optimizer
    let _optimizer = BootstrapFewShot::new().with_max_demos(3);
    println!("✓ Created BootstrapFewShot optimizer (max_demos=3)\n");

    // 5. Demonstrate concept
    println!("=== How BootstrapFewShot Works ===\n");
    println!("1. Run LLM on training examples");
    println!("   → Execute node.execute(question)");
    println!("   → Get predicted answer\n");

    println!("2. Evaluate each prediction");
    println!("   → metric(expected, predicted)");
    println!("   → Score: 1.0 (correct) or 0.0 (incorrect)\n");

    println!("3. Collect successful examples (score > 0.5)");
    println!("   → Add to few-shot demonstrations\n");

    println!("4. Update node with demonstrations");
    println!("   → Future prompts include successful examples");
    println!("   → LLM learns from good predictions\n");

    // 6. Show example metric usage
    println!("=== Metric Example ===\n");
    let example1 = &trainset[0];
    let correct_pred = QAState {
        question: "What is 2+2?".to_string(),
        answer: "4".to_string(),
    };
    let incorrect_pred = QAState {
        question: "What is 2+2?".to_string(),
        answer: "5".to_string(),
    };

    let score_correct = metric(example1, &correct_pred)?;
    let score_incorrect = metric(example1, &incorrect_pred)?;

    println!(
        "Expected: '{}', Predicted: '{}' → Score: {}",
        example1.answer, correct_pred.answer, score_correct
    );
    println!(
        "Expected: '{}', Predicted: '{}' → Score: {}",
        example1.answer, incorrect_pred.answer, score_incorrect
    );

    println!("\n=== Production Usage ===\n");
    println!("For full working examples with LLM calls, see:");
    println!("• crates/dashflow/tests/dashoptimize_tests.rs");
    println!("• docs/DASHOPTIMIZE_GUIDE.md (complete documentation)");
    println!("\nThis conceptual example shows the optimizer structure without");
    println!("requiring API keys or making actual LLM calls.");

    Ok(())
}

// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
