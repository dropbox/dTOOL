//! RandomSearch Optimization Example
//!
//! This example demonstrates the RandomSearch optimizer, which explores the optimization
//! space through systematic random sampling:
//! 1. Generates candidate programs with different configurations
//! 2. Four search strategies (encoded by seed):
//!    - seed=-3: Zero-shot baseline (no demonstrations)
//!    - seed=-2: Labeled few-shot only (use pre-labeled examples)
//!    - seed=-1: Unshuffled bootstrap (deterministic demo selection)
//!    - seed≥0: Shuffled bootstrap with random demo counts
//! 3. Evaluates each candidate on validation set
//! 4. Returns best performing candidate
//!
//! RandomSearch is effective as a baseline optimizer and for exploring different
//! configuration strategies systematically.
//!
//! This is a simplified conceptual example. For production use, see the
//! integration tests in crates/dashflow/tests/dashoptimize_tests.rs
//!
//! Run with: cargo run --package dashflow --example random_search_optimizer_example

use dashflow::optimize::{exact_match, make_signature, MetricFn, RandomSearch};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Example state for question answering
#[derive(Clone, Debug, Serialize, Deserialize)]
struct QAState {
    question: String,
    answer: String,
}

fn main() -> dashflow::Result<()> {
    println!("=== RandomSearch Optimizer - Concept Demo ===\n");

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
        QAState {
            question: "What is the first letter of the alphabet?".to_string(),
            answer: "A".to_string(),
        },
    ];
    println!("✓ Prepared training set: {} examples\n", trainset.len());

    // 3. Prepare validation set (separate from training)
    let valset = [
        QAState {
            question: "What is 3+3?".to_string(),
            answer: "6".to_string(),
        },
        QAState {
            question: "What is the capital of Italy?".to_string(),
            answer: "Rome".to_string(),
        },
    ];
    println!("✓ Prepared validation set: {} examples\n", valset.len());

    // 4. Define metric (exact match)
    let metric: MetricFn<QAState> = Arc::new(|expected: &QAState, predicted: &QAState| {
        Ok(exact_match(&expected.answer, &predicted.answer))
    });
    println!("✓ Defined metric: exact_match\n");

    // 5. Create RandomSearch optimizer
    let _optimizer = RandomSearch::new()
        .with_num_candidate_programs(20) // Try 20 different configurations
        .with_max_bootstrapped_demos(5) // Maximum demos per prompt
        .with_max_labeled_demos(3); // Maximum labeled examples
    println!("✓ Created RandomSearch optimizer:");
    println!("  • Number of candidates: 20");
    println!("  • Max bootstrapped demos: 5");
    println!("  • Max labeled demos: 3\n");

    // 6. Demonstrate RandomSearch flow
    println!("=== How RandomSearch Works ===\n");

    println!("PHASE 1: Candidate Generation");
    println!("  RandomSearch generates candidates using 4 strategies:\n");

    println!("  Strategy 1 (seed=-3): Zero-Shot Baseline");
    println!("    • No demonstrations included");
    println!("    • Prompt contains only instruction");
    println!("    • Use case: Establish baseline performance\n");

    println!("  Strategy 2 (seed=-2): Labeled Few-Shot");
    println!("    • Use pre-labeled examples as demonstrations");
    println!("    • Select up to max_labeled_demos examples");
    println!("    • Use case: When you have high-quality labeled data\n");

    println!("  Strategy 3 (seed=-1): Unshuffled Bootstrap");
    println!("    • Run model on training data");
    println!("    • Select successful predictions (score > threshold)");
    println!("    • Use original order (deterministic)");
    println!("    • Use case: Deterministic few-shot selection\n");

    println!("  Strategy 4 (seed≥0): Shuffled Bootstrap");
    println!("    • Same as Strategy 3, but shuffle selected demos");
    println!("    • Random demo count [1, max_bootstrapped_demos]");
    println!("    • Different seed → different random configuration");
    println!("    • Use case: Explore different demo combinations\n");

    println!("PHASE 2: Candidate Evaluation");
    println!("  1. For each candidate configuration:");
    println!("     → Run model on validation set");
    println!("     → Calculate average score: mean(metric(expected, predicted))");
    println!("     → Track best candidate so far\n");

    println!("  2. Early stopping (optional):");
    println!("     → If candidate reaches target score");
    println!("     → Stop search and return\n");

    println!("PHASE 3: Best Candidate Selection");
    println!("  1. Compare all candidates by validation score");
    println!("  2. Select highest scoring configuration");
    println!("  3. Return optimized node with best configuration\n");

    // 7. Show seed convention
    println!("=== Seed Convention ===\n");
    println!("RandomSearch uses seed values to encode optimization strategy:\n");
    println!("| Seed   | Strategy           | Demos Source         | Shuffled | Demo Count      |");
    println!("|--------|--------------------|--------------------- |----------|-----------------|");
    println!("| -3     | Zero-shot          | None                 | N/A      | 0               |");
    println!("| -2     | Labeled few-shot   | Pre-labeled          | No       | ≤ max_labeled   |");
    println!("| -1     | Unshuffled         | Bootstrap successful | No       | ≤ max_bootstrap |");
    println!("| 0-N    | Shuffled + Random  | Bootstrap successful | Yes      | Random [1, max] |");
    println!("\nWith num_candidates=20, RandomSearch will systematically try:");
    println!("  • 1 zero-shot baseline (seed=-3)");
    println!("  • 1 labeled few-shot config (seed=-2)");
    println!("  • 1 unshuffled bootstrap (seed=-1)");
    println!("  • 17 shuffled bootstrap configs (seeds=0-16)\n");

    // 8. Show example metric usage
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
        "Expected: '{}', Predicted: '{}' → Score: {} ✓",
        example1.answer, correct_pred.answer, score_correct
    );
    println!(
        "Expected: '{}', Predicted: '{}' → Score: {} ✗",
        example1.answer, incorrect_pred.answer, score_incorrect
    );
    println!("\nIf 15/20 candidates score > 0.5, those become potential demonstrations\n");

    println!("=== Key Advantages of RandomSearch ===\n");
    println!("1. Systematic exploration: Tries multiple configuration strategies");
    println!("2. No gradient required: Pure sampling-based optimization");
    println!("3. Baseline comparison: Always includes zero-shot baseline");
    println!("4. Reproducible: Fixed seed → deterministic results");
    println!("5. Simple and fast: No complex algorithms, just evaluate and compare");
    println!("6. Effective baseline: Often competitive with more complex optimizers\n");

    println!("=== When to Use RandomSearch ===\n");
    println!("✓ As a baseline before trying more complex optimizers");
    println!("✓ When you want to compare multiple configuration strategies");
    println!("✓ When compute budget is limited (fewer candidates = faster)");
    println!("✓ When you have both labeled and unlabeled data");
    println!("✓ When reproducibility is important (fixed seed)");
    println!("✗ When you need iterative refinement (use SIMBA or GEPA instead)");
    println!("✗ When you have very large search spaces (>100 candidates)\n");

    println!("=== Comparison with Other Optimizers ===\n");
    println!("RandomSearch vs BootstrapFewShot:");
    println!("  • RandomSearch: Tries multiple strategies (zero-shot, labeled, bootstrap)");
    println!("  • BootstrapFewShot: Single strategy (bootstrap only)");
    println!("  → Use RandomSearch when you want to explore different approaches\n");

    println!("RandomSearch vs SIMBA:");
    println!("  • RandomSearch: One-shot optimization (generate → evaluate → select)");
    println!("  • SIMBA: Iterative optimization (evaluate → improve → repeat)");
    println!("  → Use SIMBA when you need iterative refinement\n");

    println!("RandomSearch vs GEPA:");
    println!("  • RandomSearch: Random sampling of configurations");
    println!("  • GEPA: Evolutionary search with LLM reflection");
    println!("  → Use GEPA when you want LLM-guided optimization\n");

    println!("=== Production Usage ===\n");
    println!("For full working examples with LLM calls, see:");
    println!("• crates/dashflow/tests/dashoptimize_tests.rs");
    println!("• docs/DASHOPTIMIZE_GUIDE.md (complete documentation)");
    println!("\nThis conceptual example shows the optimizer structure without");
    println!("requiring API keys or making actual LLM calls.");

    Ok(())
}

// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
