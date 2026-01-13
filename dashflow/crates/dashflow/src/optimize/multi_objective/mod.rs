// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name multi_objective
//! @category optimize
//! @status stable
//!
//! # Multi-Objective Optimization
//!
//! This module provides multi-objective optimization for DashFlow programs, allowing you to
//! optimize for multiple competing objectives simultaneously (e.g., quality and cost).
//!
//! ## Key Features
//!
//! - **Multi-Objective Optimization**: Optimize for quality, cost, latency, and token usage
//! - **Pareto Frontier**: Find the set of non-dominated solutions
//! - **Budget Constraints**: Select best solution within cost/latency budgets
//! - **Quality Constraints**: Select cheapest solution meeting quality thresholds
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::optimize::multi_objective::*;
//!
//! // Define your state type
//! #[derive(Clone)]
//! struct QAState {
//!     question: String,
//!     answer: String,
//! }
//!
//! // Create optimizer with quality and cost objectives
//! let optimizer = MultiObjectiveOptimizer::new()
//!     .add_objective(Objective::new(ObjectiveType::Quality, 0.7))
//!     .add_objective(Objective::new(ObjectiveType::Cost, 0.3));
//!
//! // Create candidates with eval_fn for quality evaluation
//! // (eval_fn runs your model and returns a quality score)
//! let candidates = vec![
//!     Candidate::new(gpt4_config, "gpt-4")
//!         .with_eval_fn(|module, data| { /* evaluate model quality */ 0.95 }),
//!     Candidate::new(gpt35_config, "gpt-3.5-turbo")
//!         .with_eval_fn(|module, data| { /* evaluate model quality */ 0.85 }),
//! ];
//!
//! // Optimize (returns Pareto frontier)
//! let frontier = optimizer.evaluate_candidates(candidates, &trainset, Some(&valset))?;
//!
//! // Select solution by budget
//! let budget_solution = frontier.select_by_budget(ObjectiveType::Cost, 0.02)?; // $0.02/request
//!
//! // Or select by quality threshold
//! let quality_solution = frontier.select_by_quality(
//!     ObjectiveType::Quality,
//!     0.90,
//!     ObjectiveType::Cost
//! )?; // 90% accuracy, minimize cost
//! ```
//!
//! ## Use Cases
//!
//! 1. **Budget-Constrained Optimization**: Find the best quality model within a fixed budget
//! 2. **Quality-Constrained Cost Minimization**: Find the cheapest model meeting quality requirements
//! 3. **Tradeoff Analysis**: Understand the quality/cost curve to make informed decisions
//! 4. **Multi-Model Comparison**: Compare different models/prompts on multiple dimensions

pub mod objectives;
pub mod optimizer;
pub mod pareto;

pub use objectives::{Objective, ObjectiveType, ObjectiveValue};
pub use optimizer::{
    Candidate, MultiObjectiveConfig, MultiObjectiveError, MultiObjectiveOptimizer,
};
pub use pareto::{ParetoError, ParetoFrontier, ParetoSolution};
