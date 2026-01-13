//! Multi-Objective Optimization Example
//!
//! This example demonstrates how to use multi-objective optimization to find
//! Pareto-optimal solutions that balance quality and cost tradeoffs.
//!
//! Run with:
//! ```bash
//! cargo run --example multi_objective_optimization
//! ```

#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(clippy::useless_vec)]

use dashflow::optimize::multi_objective::{
    Candidate, MultiObjectiveOptimizer, Objective, ObjectiveType, ObjectiveValue, ParetoSolution,
};

// Simple state type for demonstration
#[derive(Clone, Debug)]
struct QAState {
    question: String,
    answer: String,
}

// Mock model configuration (in practice, this would be an actual LLM config)
#[derive(Clone)]
struct ModelConfig {
    name: String,
    // Mock quality and cost for demonstration
    quality: f64,
    cost: f64,
}

fn main() {
    println!("=== Multi-Objective Optimization Example ===\n");

    // Create optimizer with multiple objectives
    // Note: Quality evaluation is done via Candidate::with_eval_fn(), not optimizer-level metrics
    let optimizer: MultiObjectiveOptimizer<QAState> = MultiObjectiveOptimizer::new()
        .add_objective(Objective::new(ObjectiveType::Quality, 0.7))
        .add_objective(Objective::new(ObjectiveType::Cost, 0.3));

    println!("Optimizer configured with:");
    println!("  - Quality objective (weight: 0.7, maximize)");
    println!("  - Cost objective (weight: 0.3, minimize)");
    println!();

    // Create mock model configurations
    let gpt4 = ModelConfig {
        name: "gpt-4".to_string(),
        quality: 0.98,
        cost: 0.05,
    };

    let gpt35 = ModelConfig {
        name: "gpt-3.5-turbo".to_string(),
        quality: 0.85,
        cost: 0.015,
    };

    let claude = ModelConfig {
        name: "claude-3-opus".to_string(),
        quality: 0.95,
        cost: 0.03,
    };

    let claude_sonnet = ModelConfig {
        name: "claude-3-sonnet".to_string(),
        quality: 0.80,
        cost: 0.01,
    };

    // Create candidates with custom evaluation functions
    let candidates = vec![
        Candidate::new(gpt4, "gpt-4")
            .with_eval_fn(move |model: &ModelConfig, _data: &[QAState]| model.quality),
        Candidate::new(gpt35, "gpt-3.5-turbo")
            .with_eval_fn(move |model: &ModelConfig, _data: &[QAState]| model.quality),
        Candidate::new(claude, "claude-3-opus")
            .with_eval_fn(move |model: &ModelConfig, _data: &[QAState]| model.quality),
        Candidate::new(claude_sonnet, "claude-3-sonnet")
            .with_eval_fn(move |model: &ModelConfig, _data: &[QAState]| model.quality),
    ];

    // Mock training data (not actually used in this demo)
    let trainset = vec![
        QAState {
            question: "What is 2+2?".to_string(),
            answer: "4".to_string(),
        },
        QAState {
            question: "What is the capital of France?".to_string(),
            answer: "Paris".to_string(),
        },
    ];

    println!("Simulating evaluation of 4 model configurations:");
    println!("  GPT-4: High quality (0.98), High cost ($0.05)");
    println!("  GPT-3.5-Turbo: Medium quality (0.85), Medium cost ($0.015)");
    println!("  Claude-3-Opus: High quality (0.95), High cost ($0.03)");
    println!("  Claude-3-Sonnet: Lower quality (0.80), Low cost ($0.01)");
    println!();

    // For demonstration, we'll manually create the Pareto frontier
    // In practice, the optimizer would evaluate candidates and measure cost/quality
    let mut frontier_with_costs = dashflow::optimize::multi_objective::ParetoFrontier::new();

    // Add all candidates with their quality and cost values
    let solutions = vec![
        ParetoSolution::new("gpt-4")
            .with_objective(ObjectiveValue::new(ObjectiveType::Quality, 0.98))
            .with_objective(ObjectiveValue::new(ObjectiveType::Cost, 0.05)),
        ParetoSolution::new("gpt-3.5-turbo")
            .with_objective(ObjectiveValue::new(ObjectiveType::Quality, 0.85))
            .with_objective(ObjectiveValue::new(ObjectiveType::Cost, 0.015)),
        ParetoSolution::new("claude-3-opus")
            .with_objective(ObjectiveValue::new(ObjectiveType::Quality, 0.95))
            .with_objective(ObjectiveValue::new(ObjectiveType::Cost, 0.03)),
        ParetoSolution::new("claude-3-sonnet")
            .with_objective(ObjectiveValue::new(ObjectiveType::Quality, 0.80))
            .with_objective(ObjectiveValue::new(ObjectiveType::Cost, 0.01)),
    ];

    for solution in solutions {
        frontier_with_costs.add_solution(solution);
    }

    println!("Pareto Frontier Analysis:");
    println!("  Solutions on frontier: {}", frontier_with_costs.len());
    println!();

    // Display all solutions on the frontier
    println!("Non-dominated solutions:");
    for solution in &frontier_with_costs.solutions {
        let Some(quality) = solution
            .get_objective(ObjectiveType::Quality)
            .map(|objective| objective.value)
        else {
            continue;
        };
        let Some(cost) = solution
            .get_objective(ObjectiveType::Cost)
            .map(|objective| objective.value)
        else {
            continue;
        };
        println!(
            "  {}: Quality={:.2}, Cost=${:.3}",
            solution.id, quality, cost
        );
    }
    println!();

    // Demonstrate budget-constrained selection
    println!("Budget-Constrained Selection:");
    println!("  Finding best quality solution with cost <= $0.02");
    match frontier_with_costs.select_by_budget(ObjectiveType::Cost, 0.02) {
        Ok(solution) => {
            let quality = solution
                .get_objective(ObjectiveType::Quality)
                .map(|objective| objective.value);
            let cost = solution
                .get_objective(ObjectiveType::Cost)
                .map(|objective| objective.value);

            if let (Some(quality), Some(cost)) = (quality, cost) {
                println!("  Selected: {}", solution.id);
                println!("    Quality: {:.2}", quality);
                println!("    Cost: ${:.3}", cost);
            } else {
                println!("  Selected: {} (missing quality/cost objectives)", solution.id);
            }
        }
        Err(e) => println!("  No solution found within budget: {}", e),
    }
    println!();

    // Demonstrate quality-constrained selection
    println!("Quality-Constrained Selection:");
    println!("  Finding cheapest solution with quality >= 0.90");
    match frontier_with_costs.select_by_quality(ObjectiveType::Quality, 0.90, ObjectiveType::Cost) {
        Ok(solution) => {
            let quality = solution
                .get_objective(ObjectiveType::Quality)
                .map(|objective| objective.value);
            let cost = solution
                .get_objective(ObjectiveType::Cost)
                .map(|objective| objective.value);

            if let (Some(quality), Some(cost)) = (quality, cost) {
                println!("  Selected: {}", solution.id);
                println!("    Quality: {:.2}", quality);
                println!("    Cost: ${:.3}", cost);
            } else {
                println!("  Selected: {} (missing quality/cost objectives)", solution.id);
            }
        }
        Err(e) => println!("  No solution found meeting quality threshold: {}", e),
    }
    println!();

    // Demonstrate sorted views
    println!("Solutions sorted by quality (descending):");
    let by_quality = frontier_with_costs.solutions_sorted_by(ObjectiveType::Quality, false);
    for solution in by_quality {
        let Some(quality) = solution
            .get_objective(ObjectiveType::Quality)
            .map(|objective| objective.value)
        else {
            continue;
        };
        let Some(cost) = solution
            .get_objective(ObjectiveType::Cost)
            .map(|objective| objective.value)
        else {
            continue;
        };
        println!(
            "  {}: Quality={:.2}, Cost=${:.3}",
            solution.id, quality, cost
        );
    }
    println!();

    println!("Solutions sorted by cost (ascending):");
    let by_cost = frontier_with_costs.solutions_sorted_by(ObjectiveType::Cost, true);
    for solution in &by_cost {
        let Some(quality) = solution
            .get_objective(ObjectiveType::Quality)
            .map(|objective| objective.value)
        else {
            continue;
        };
        let Some(cost) = solution
            .get_objective(ObjectiveType::Cost)
            .map(|objective| objective.value)
        else {
            continue;
        };
        println!(
            "  {}: Quality={:.2}, Cost=${:.3}",
            solution.id, quality, cost
        );
    }
    println!();

    // Demonstrate tradeoff analysis
    println!("Tradeoff Analysis:");
    println!("  Moving from cheapest to most expensive on frontier:");
    for solution in &by_cost {
        let Some(quality) = solution
            .get_objective(ObjectiveType::Quality)
            .map(|objective| objective.value)
        else {
            continue;
        };
        let Some(cost) = solution
            .get_objective(ObjectiveType::Cost)
            .map(|objective| objective.value)
        else {
            continue;
        };
        let cost_per_quality_point = cost / quality;
        println!(
            "    {}: ${:.3}/request, {:.0}% quality, ${:.4} per quality point",
            solution.id,
            cost,
            quality * 100.0,
            cost_per_quality_point
        );
    }
    println!();

    println!("=== Key Insights ===");
    println!();
    println!("1. Pareto Frontier: Only non-dominated solutions are kept");
    println!("   - No solution is strictly better than another in all objectives");
    println!();
    println!("2. Budget Constraints: Find best quality within cost limit");
    println!("   - Useful for production deployment with fixed budgets");
    println!();
    println!("3. Quality Constraints: Find cheapest solution meeting quality bar");
    println!("   - Useful when minimum quality is required but cost matters");
    println!();
    println!("4. Tradeoff Analysis: Understand cost vs quality curve");
    println!("   - See diminishing returns (e.g., 2x cost for 5% quality gain)");
    println!("   - Make informed decisions about quality/cost tradeoffs");
}
