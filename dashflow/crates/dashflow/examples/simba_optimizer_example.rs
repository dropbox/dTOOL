//! SIMBA Optimization Example
//!
//! This example demonstrates the SIMBA (Sequential Instruction Management via Behavior Analysis) optimizer.
//! SIMBA improves model prompts through iterative optimization:
//! 1. Evaluates candidate instructions on training data
//! 2. Buckets examples by performance (high/medium/low scorers)
//! 3. Applies optimization strategies:
//!    - AppendADemo: Add successful examples from high scorers
//!    - AppendARule: Generate improvement rules via LLM reflection
//! 4. Selects best candidate using softmax sampling with temperature
//!
//! SIMBA is particularly effective for complex reasoning tasks where instruction quality
//! significantly impacts performance.
//!
//! This is a simplified conceptual example. For production use, see the
//! integration tests in crates/dashflow/tests/dashoptimize_tests.rs
//!
//! Run with: cargo run --package dashflow --example simba_optimizer_example

use dashflow::optimize::{exact_match, make_signature, MetricFn, SIMBA};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Example state for reasoning tasks
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReasoningState {
    problem: String,
    solution: String,
}

fn main() -> dashflow::Result<()> {
    println!("=== SIMBA Optimizer - Concept Demo ===\n");

    // 1. Create signature for reasoning tasks
    let signature = make_signature("problem -> solution", "Solve problems with clear reasoning")?;
    println!(
        "✓ Created signature: {} input fields, {} output fields",
        signature.input_fields.len(),
        signature.output_fields.len()
    );

    // 2. Prepare training data (math word problems)
    let trainset = [
        ReasoningState {
            problem: "If John has 5 apples and gives 2 away, how many remain?".to_string(),
            solution: "3".to_string(),
        },
        ReasoningState {
            problem: "A train travels 60 mph for 2 hours. How far does it go?".to_string(),
            solution: "120 miles".to_string(),
        },
        ReasoningState {
            problem: "If a shirt costs $20 and is 25% off, what is the sale price?".to_string(),
            solution: "$15".to_string(),
        },
        ReasoningState {
            problem: "What is 15% of 200?".to_string(),
            solution: "30".to_string(),
        },
        ReasoningState {
            problem: "If there are 24 hours in a day, how many minutes is that?".to_string(),
            solution: "1440 minutes".to_string(),
        },
        ReasoningState {
            problem: "A rectangle is 8 units long and 5 units wide. What is its area?".to_string(),
            solution: "40 square units".to_string(),
        },
    ];
    println!("✓ Prepared training set: {} examples\n", trainset.len());

    // 3. Define metric (exact match with normalization)
    let metric: MetricFn<ReasoningState> =
        Arc::new(|expected: &ReasoningState, predicted: &ReasoningState| {
            Ok(exact_match(&expected.solution, &predicted.solution))
        });
    println!("✓ Defined metric: exact_match\n");

    // 4. Create SIMBA optimizer
    let _optimizer = SIMBA::<ReasoningState>::new()
        .with_bsize(32) // Mini-batch size for sampling
        .with_max_steps(5) // Number of optimization iterations
        .with_num_candidates(3) // Generate 3 candidate improvements per iteration
        .with_max_demos(4); // Maximum demonstrations per prompt
    println!("✓ Created SIMBA optimizer:");
    println!("  • Strategies: AppendADemo (add successful examples), AppendARule (generate improvement rules)");
    println!("  • Max steps: 5");
    println!("  • Mini-batch size: 32");
    println!("  • Candidates per iteration: 3");
    println!("  • Max demos: 4\n");

    // 5. Demonstrate SIMBA optimization flow
    println!("=== How SIMBA Works ===\n");

    println!("ITERATION 1: Baseline Evaluation");
    println!("  1. Run current instruction on all training examples");
    println!("  2. Evaluate: metric(expected, predicted) → scores");
    println!("  3. Bucket examples by performance:");
    println!("     • HIGH scorers (≥75th percentile): Strong examples");
    println!("     • MEDIUM scorers (25-75th percentile): Average examples");
    println!("     • LOW scorers (<25th percentile): Failure cases\n");

    println!("ITERATION 2-5: Iterative Improvement");
    println!("  4. Generate candidate improvements:");
    println!("     Strategy 1: AppendADemo");
    println!("       → Sample high-scoring example (Poisson distribution)");
    println!("       → Add to few-shot demonstrations");
    println!("       → Candidate instruction includes successful pattern");
    println!("     Strategy 2: AppendARule");
    println!("       → Analyze low-scoring failures");
    println!("       → Generate improvement rule via LLM reflection");
    println!("       → Candidate instruction includes learned rule\n");

    println!("  5. Evaluate all candidates on training set");
    println!("     → Each candidate gets performance score");
    println!("     → Track score improvements over iterations\n");

    println!("  6. Select best candidate (softmax sampling):");
    println!("     → Higher scores → higher selection probability");
    println!("     → Temperature controls exploration/exploitation");
    println!("     → Top-K + baseline ensures stability\n");

    println!("  7. Repeat until:");
    println!("     → Max iterations reached (5)");
    println!("     → Or convergence detected (no improvement)\n");

    // 6. Demonstrate performance bucketing concept
    println!("=== Performance Bucketing Example ===\n");
    println!("Example scores: [0.2, 0.4, 0.6, 0.8, 0.9, 1.0]");
    println!("Percentiles: 25th=0.45, 75th=0.85");
    println!("Buckets:");
    println!("  • HIGH (≥0.85): [0.9, 1.0] ← Use for AppendADemo");
    println!("  • MEDIUM (0.45-0.85): [0.6, 0.8]");
    println!("  • LOW (<0.45): [0.2, 0.4] ← Analyze for AppendARule\n");

    // 7. Show strategy comparison
    println!("=== Strategy Comparison ===\n");
    println!("AppendADemo (Pattern Learning):");
    println!("  • Best for: Learning from successful examples");
    println!("  • Mechanism: Add high-scoring example to few-shot demos");
    println!("  • When effective: Tasks with clear patterns to copy");
    println!("  • Example: 'Problem: [X], Solution: [Y]' → LLM learns format\n");

    println!("AppendARule (Reflective Learning):");
    println!("  • Best for: Learning from failures");
    println!("  • Mechanism: LLM reflects on mistakes, generates improvement rule");
    println!("  • When effective: Complex reasoning where patterns aren't obvious");
    println!(
        "  • Example: 'Always convert units before calculating' → Rule added to instruction\n"
    );

    // 8. Show metric usage
    println!("=== Metric Example ===\n");
    let example = &trainset[0];
    let correct_pred = ReasoningState {
        problem: example.problem.clone(),
        solution: "3".to_string(),
    };
    let incorrect_pred = ReasoningState {
        problem: example.problem.clone(),
        solution: "2".to_string(),
    };

    let score_correct = metric(example, &correct_pred)?;
    let score_incorrect = metric(example, &incorrect_pred)?;

    println!(
        "Expected: '{}', Predicted: '{}' → Score: {} (HIGH bucket)",
        example.solution, correct_pred.solution, score_correct
    );
    println!(
        "Expected: '{}', Predicted: '{}' → Score: {} (LOW bucket)",
        example.solution, incorrect_pred.solution, score_incorrect
    );

    println!("\n=== Key Advantages of SIMBA ===\n");
    println!("1. Dual strategy: Learn from both successes (demos) and failures (rules)");
    println!("2. Performance bucketing: Target improvements where needed most");
    println!("3. Probabilistic sampling: Avoid overfitting to single examples");
    println!("4. Temperature control: Balance exploration vs exploitation");
    println!("5. Iterative refinement: Gradual improvement over multiple rounds\n");

    println!("=== When to Use SIMBA ===\n");
    println!("✓ Complex reasoning tasks (math, logic, multi-step problems)");
    println!("✓ High variance in performance across examples");
    println!("✓ Need both pattern learning AND rule extraction");
    println!("✓ Willing to invest compute in iterative optimization");
    println!("✗ Simple classification (use BootstrapFewShot instead)");
    println!("✗ Limited training data (<10 examples)\n");

    println!("=== Production Usage ===\n");
    println!("For full working examples with LLM calls, see:");
    println!("• crates/dashflow/tests/dashoptimize_tests.rs");
    println!("• docs/DASHOPTIMIZE_GUIDE.md (complete documentation)");
    println!("\nThis conceptual example shows the optimizer structure without");
    println!("requiring API keys or making actual LLM calls.");

    Ok(())
}

// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
