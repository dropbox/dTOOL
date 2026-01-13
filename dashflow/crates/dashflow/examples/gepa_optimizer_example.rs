//! GEPA Optimization Example
//!
//! This example demonstrates the GEPA (Genetic Evolutionary Prompt Algorithm) optimizer.
//! GEPA combines evolutionary search with LLM-based reflection to optimize prompts:
//! 1. Maintains a population of candidate prompts
//! 2. Evaluates candidates on training/validation data
//! 3. Selects parent candidates (Pareto frontier or greedy best)
//! 4. Uses LLM reflection on failures to generate improved instructions
//! 5. Adds improved candidates to population
//! 6. Repeats until budget exhausted or target score reached
//!
//! GEPA is particularly effective when you want the LLM itself to reason about
//! how to improve prompts based on failure analysis.
//!
//! This is a simplified conceptual example. For production use, see the
//! integration tests in crates/dashflow/tests/dashoptimize_tests.rs
//!
//! Run with: cargo run --package dashflow --example gepa_optimizer_example

use dashflow::optimize::{exact_match, make_signature, MetricFn, SelectionStrategy, GEPA};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Example state for reasoning tasks
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReasoningState {
    problem: String,
    reasoning: String,
    solution: String,
}

fn main() -> dashflow::Result<()> {
    println!("=== GEPA Optimizer - Concept Demo ===\n");

    // 1. Create signature for reasoning tasks
    let signature = make_signature(
        "problem -> reasoning, solution",
        "Solve problems with clear step-by-step reasoning",
    )?;
    println!(
        "✓ Created signature: {} input fields, {} output fields",
        signature.input_fields.len(),
        signature.output_fields.len()
    );

    // 2. Prepare training data (math word problems)
    let trainset = [
        ReasoningState {
            problem: "If John has 5 apples and gives 2 away, how many remain?".to_string(),
            reasoning: "Start with 5, subtract 2: 5 - 2 = 3".to_string(),
            solution: "3".to_string(),
        },
        ReasoningState {
            problem: "A train travels 60 mph for 2 hours. How far does it go?".to_string(),
            reasoning: "Distance = speed × time: 60 mph × 2 hours = 120 miles".to_string(),
            solution: "120 miles".to_string(),
        },
        ReasoningState {
            problem: "If a shirt costs $20 and is 25% off, what is the sale price?".to_string(),
            reasoning: "25% of $20 = $5, so sale price = $20 - $5 = $15".to_string(),
            solution: "$15".to_string(),
        },
        ReasoningState {
            problem: "What is 15% of 200?".to_string(),
            reasoning: "15% = 0.15, so 0.15 × 200 = 30".to_string(),
            solution: "30".to_string(),
        },
    ];
    println!("✓ Prepared training set: {} examples\n", trainset.len());

    // 3. Prepare validation set
    let valset = [
        ReasoningState {
            problem: "If there are 24 hours in a day, how many minutes is that?".to_string(),
            reasoning: "24 hours × 60 minutes/hour = 1440 minutes".to_string(),
            solution: "1440 minutes".to_string(),
        },
        ReasoningState {
            problem: "A rectangle is 8 units long and 5 units wide. What is its area?".to_string(),
            reasoning: "Area = length × width: 8 × 5 = 40 square units".to_string(),
            solution: "40 square units".to_string(),
        },
    ];
    println!("✓ Prepared validation set: {} examples\n", valset.len());

    // 4. Define metric (exact match on solution)
    let metric: MetricFn<ReasoningState> =
        Arc::new(|expected: &ReasoningState, predicted: &ReasoningState| {
            Ok(exact_match(&expected.solution, &predicted.solution))
        });
    println!("✓ Defined metric: exact_match on solution field\n");

    // 5. Create GEPA optimizer with configuration
    let _optimizer = GEPA::new()
        .with_max_metric_calls(100) // Budget: Maximum 100 metric evaluations
        .with_max_full_evals(10) // Maximum 10 full validation passes
        .with_reflection_minibatch_size(3) // Use 3 examples for reflection
        .with_candidate_selection_strategy(SelectionStrategy::Pareto) // Pareto frontier selection
        .with_skip_perfect_score(true); // Skip examples with perfect scores during reflection
    println!("✓ Created GEPA optimizer:");
    println!("  • Max metric calls: 100 (budget control)");
    println!("  • Max full evaluations: 10");
    println!("  • Reflection minibatch size: 3");
    println!("  • Selection strategy: Pareto (probabilistic frontier sampling)");
    println!("  • Exclude perfect scores: true (focus on failures)\n");

    // 6. Demonstrate GEPA optimization flow
    println!("=== How GEPA Works ===\n");

    println!("INITIALIZATION:");
    println!("  1. Start with initial candidate (baseline instruction)");
    println!("  2. Evaluate on validation set → initial score");
    println!("  3. Initialize population: [candidate_0]\n");

    println!("EVOLUTIONARY LOOP (repeat until budget exhausted):");
    println!("  4. Parent Selection:");
    println!("     Strategy: Pareto (default)");
    println!("       → Select from Pareto frontier (non-dominated candidates)");
    println!("       → Probabilistic sampling favors higher scores");
    println!("     Strategy: CurrentBest (greedy)");
    println!("       → Always select highest scoring candidate\n");

    println!("  5. Reflection (LLM-based mutation):");
    println!("     a. Sample reflection_minibatch_size examples from trainset");
    println!("     b. Run parent candidate on sampled examples");
    println!("     c. Collect failures (score < perfect_score)");
    println!("     d. Send failures to LLM with prompt:");
    println!("        'Analyze these failures and propose an improved instruction");
    println!("        that addresses the mistakes.'");
    println!("     e. LLM generates improved instruction based on failure analysis\n");

    println!("  6. Evaluation:");
    println!("     a. Create new candidate with improved instruction");
    println!("     b. Evaluate on validation set");
    println!("     c. Add to population: [candidate_0, ..., candidate_N]\n");

    println!("  7. Budget Check:");
    println!("     → If max_metric_calls reached: STOP");
    println!("     → If max_full_evals reached: STOP");
    println!("     → Otherwise: Continue to step 4\n");

    println!("FINALIZATION:");
    println!("  8. Select best candidate from population");
    println!("  9. Return optimized node with best instruction\n");

    // 7. Demonstrate selection strategies
    println!("=== Selection Strategies ===\n");

    println!("Pareto Frontier (default):");
    println!("  • Definition: Candidates not dominated by any other candidate");
    println!("  • A dominates B if: score(A) ≥ score(B) on all examples AND");
    println!("                      score(A) > score(B) on at least one example");
    println!(
        "  • Selection: Probabilistic sampling from frontier (higher score → higher probability)"
    );
    println!("  • Use case: Multi-objective optimization, diverse exploration");
    println!("  • Example population:");
    println!("    - Candidate A: [0.9, 0.8, 0.7] → avg=0.80");
    println!("    - Candidate B: [0.8, 0.9, 0.8] → avg=0.83");
    println!("    - Candidate C: [0.7, 0.7, 0.9] → avg=0.77");
    println!("    Pareto frontier: {{A, B, C}} (none dominates others)");
    println!("    Selection probability: B > A > C (by average score)\n");

    println!("CurrentBest (greedy):");
    println!("  • Definition: Single candidate with highest average score");
    println!("  • Selection: Deterministic (always best)");
    println!("  • Use case: Single-objective optimization, exploitation");
    println!("  • Example: Always selects Candidate B (avg=0.83)\n");

    // 8. Show LLM reflection example
    println!("=== LLM Reflection Example ===\n");
    println!("Input to reflection LLM:");
    println!("---");
    println!("You are optimizing a prompt. Here are some failure cases:\n");
    println!("Example 1:");
    println!("  Problem: 'What is 15% of 200?'");
    println!("  Expected: '30'");
    println!("  Predicted: '15'");
    println!("  Score: 0.0 (incorrect)\n");
    println!("Example 2:");
    println!("  Problem: 'If a shirt costs $20 and is 25% off, what is the sale price?'");
    println!("  Expected: '$15'");
    println!("  Predicted: '$5'");
    println!("  Score: 0.0 (incorrect)\n");
    println!("Current instruction: 'Solve problems with clear reasoning'\n");
    println!("Propose an improved instruction that addresses these failures.");
    println!("---\n");

    println!("Output from reflection LLM:");
    println!("---");
    println!("The model is confusing percentage calculations. Improved instruction:");
    println!("'When solving percentage problems, first convert the percentage to a");
    println!("decimal (e.g., 15% = 0.15), then multiply by the base number. Show");
    println!("your calculation steps clearly.'");
    println!("---\n");

    // 9. Show metric example
    println!("=== Metric Example ===\n");
    let example = &trainset[0];
    let correct_pred = ReasoningState {
        problem: example.problem.clone(),
        reasoning: "5 - 2 = 3".to_string(),
        solution: "3".to_string(),
    };
    let incorrect_pred = ReasoningState {
        problem: example.problem.clone(),
        reasoning: "5 + 2 = 7".to_string(),
        solution: "7".to_string(),
    };

    let score_correct = metric(example, &correct_pred)?;
    let score_incorrect = metric(example, &incorrect_pred)?;

    println!(
        "Expected: '{}', Predicted: '{}' → Score: {} ✓",
        example.solution, correct_pred.solution, score_correct
    );
    println!(
        "Expected: '{}', Predicted: '{}' → Score: {} ✗ (used in reflection)",
        example.solution, incorrect_pred.solution, score_incorrect
    );

    println!("\n=== Key Advantages of GEPA ===\n");
    println!("1. LLM-guided optimization: Uses language model to reason about improvements");
    println!("2. Failure-focused: Analyzes what went wrong and proposes fixes");
    println!("3. Evolutionary approach: Maintains population, explores multiple directions");
    println!("4. Budget control: Explicit limits on metric calls and evaluations");
    println!("5. Multi-objective support: Pareto frontier balances multiple objectives");
    println!("6. Meta-optimization: 'LLM optimizing LLM' - reflection on own performance\n");

    println!("=== When to Use GEPA ===\n");
    println!("✓ Complex reasoning tasks where failure analysis is valuable");
    println!("✓ When you want LLM to propose improvements (not just select examples)");
    println!("✓ Multi-objective optimization (Pareto strategy)");
    println!("✓ Limited budget (metric_calls control)");
    println!("✓ Tasks with diverse failure modes requiring different fixes");
    println!("✗ Simple classification (use BootstrapFewShot instead)");
    println!("✗ Very large datasets (>1000 examples, high metric call cost)");
    println!("✗ When deterministic optimization preferred (GEPA has randomness)\n");

    println!("=== Comparison with Other Optimizers ===\n");
    println!("GEPA vs BootstrapFewShot:");
    println!("  • GEPA: LLM reflects on failures → generates new instructions");
    println!("  • BootstrapFewShot: Collects successful examples → adds as demos");
    println!("  → Use GEPA when instruction improvement is more valuable than demo selection\n");

    println!("GEPA vs SIMBA:");
    println!("  • GEPA: Evolutionary population + LLM reflection");
    println!("  • SIMBA: Iterative improvement with demo/rule strategies");
    println!("  → Use GEPA for multi-objective, SIMBA for single-objective iterative\n");

    println!("GEPA vs RandomSearch:");
    println!("  • GEPA: Guided search via LLM reflection");
    println!("  • RandomSearch: Exhaustive search over configuration space");
    println!("  → Use GEPA when you want intelligent exploration, RandomSearch for systematic baseline\n");

    println!("=== Production Usage ===\n");
    println!("For full working examples with LLM calls, see:");
    println!("• crates/dashflow/tests/dashoptimize_tests.rs");
    println!("• docs/DASHOPTIMIZE_GUIDE.md (complete documentation)");
    println!("\nThis conceptual example shows the optimizer structure without");
    println!("requiring API keys or making actual LLM calls.");

    Ok(())
}

// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
