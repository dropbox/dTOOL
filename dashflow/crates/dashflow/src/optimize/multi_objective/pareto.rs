// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Pareto frontier implementation for multi-objective optimization.

use crate::optimize::multi_objective::objectives::{ObjectiveType, ObjectiveValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur when working with Pareto frontiers.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ParetoError {
    /// No solution in the frontier satisfies the given constraints.
    ///
    /// This occurs when filtering solutions by objective bounds yields
    /// an empty set.
    #[error("No solution found satisfying constraints: {0}")]
    NoSolutionFound(String),

    /// The requested objective type was not found in the solution.
    ///
    /// Each solution must have values for all expected objective types.
    #[error("Objective type not found: {0}")]
    ObjectiveNotFound(String),

    /// The Pareto frontier has no solutions.
    ///
    /// Operations like `best_by_objective` require at least one solution.
    #[error("Empty frontier")]
    EmptyFrontier,
}

/// A solution on the Pareto frontier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoSolution {
    /// Unique identifier for this solution
    pub id: String,

    /// Objective values for this solution
    pub objectives: HashMap<ObjectiveType, ObjectiveValue>,

    /// Optional metadata about this solution
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ParetoSolution {
    /// Create a new Pareto solution.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            objectives: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add an objective value to this solution.
    #[must_use]
    pub fn with_objective(mut self, objective: ObjectiveValue) -> Self {
        self.objectives.insert(objective.objective_type, objective);
        self
    }

    /// Add metadata to this solution.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the value for a specific objective type.
    pub fn get_objective(&self, objective_type: ObjectiveType) -> Option<&ObjectiveValue> {
        self.objectives.get(&objective_type)
    }

    /// Get the raw value for a specific objective type.
    pub fn get_value(&self, objective_type: ObjectiveType) -> Option<f64> {
        self.get_objective(objective_type).map(|obj| obj.value)
    }

    /// Check if this solution dominates another solution.
    ///
    /// Solution A dominates solution B if:
    /// - A is at least as good as B on all objectives
    /// - A is strictly better than B on at least one objective
    pub fn dominates(&self, other: &Self) -> bool {
        let mut strictly_better_count = 0;
        let mut at_least_as_good_count = 0;

        for (obj_type, self_value) in &self.objectives {
            if let Some(other_value) = other.objectives.get(obj_type) {
                if self_value.is_better_than(other_value) {
                    strictly_better_count += 1;
                    at_least_as_good_count += 1;
                } else if self_value.dominates(other_value) {
                    // Equal or better
                    at_least_as_good_count += 1;
                } else {
                    // Worse on this objective
                    return false;
                }
            }
        }

        // Must be at least as good on all objectives AND strictly better on at least one
        at_least_as_good_count == self.objectives.len() && strictly_better_count > 0
    }
}

/// A Pareto frontier containing non-dominated solutions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoFrontier {
    /// Solutions on the Pareto frontier (non-dominated solutions)
    pub solutions: Vec<ParetoSolution>,
}

impl ParetoFrontier {
    /// Create an empty Pareto frontier.
    pub fn new() -> Self {
        Self {
            solutions: Vec::new(),
        }
    }

    /// Create a Pareto frontier from a set of solutions.
    /// Automatically filters to keep only non-dominated solutions.
    pub fn from_solutions(solutions: Vec<ParetoSolution>) -> Self {
        let mut frontier = Self::new();
        for solution in solutions {
            frontier.add_solution(solution);
        }
        frontier
    }

    /// Add a solution to the frontier.
    /// If the solution is dominated by existing solutions, it won't be added.
    /// If the solution dominates existing solutions, those will be removed.
    pub fn add_solution(&mut self, new_solution: ParetoSolution) {
        // Check if new solution is dominated by any existing solution
        for existing in &self.solutions {
            if existing.dominates(&new_solution) {
                // New solution is dominated, don't add it
                return;
            }
        }

        // Remove any existing solutions dominated by the new solution
        self.solutions
            .retain(|existing| !new_solution.dominates(existing));

        // Add the new solution
        self.solutions.push(new_solution);
    }

    /// Get the number of solutions on the frontier.
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Check if the frontier is empty.
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Select the best solution within a budget constraint for a specific objective.
    ///
    /// # Arguments
    /// * `objective_type` - The objective type to constrain (e.g., Cost)
    /// * `max_value` - Maximum allowed value for that objective
    ///
    /// # Returns
    /// The solution with the best quality that satisfies the constraint,
    /// or an error if no solution meets the constraint.
    pub fn select_by_budget(
        &self,
        objective_type: ObjectiveType,
        max_value: f64,
    ) -> Result<&ParetoSolution, ParetoError> {
        if self.solutions.is_empty() {
            return Err(ParetoError::EmptyFrontier);
        }

        // Filter solutions that meet the budget constraint
        let within_budget: Vec<&ParetoSolution> = self
            .solutions
            .iter()
            .filter(|sol| {
                sol.get_value(objective_type)
                    .map(|v| {
                        if objective_type.is_minimized() {
                            v <= max_value
                        } else {
                            v >= max_value
                        }
                    })
                    .unwrap_or(false)
            })
            .collect();

        if within_budget.is_empty() {
            return Err(ParetoError::NoSolutionFound(format!(
                "No solution found with {} {} {}",
                objective_type,
                if objective_type.is_minimized() {
                    "<="
                } else {
                    ">="
                },
                max_value
            )));
        }

        // Among solutions within budget, find the one with best quality
        // (or best on the first maximized objective we find)
        let best = within_budget
            .iter()
            .max_by(|a, b| {
                // Try to compare on Quality first
                if let (Some(a_quality), Some(b_quality)) = (
                    a.get_value(ObjectiveType::Quality),
                    b.get_value(ObjectiveType::Quality),
                ) {
                    a_quality
                        .partial_cmp(&b_quality)
                        .unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    // Fallback: compare on first objective
                    let a_first = a.objectives.values().next().map(|v| v.value).unwrap_or(0.0);
                    let b_first = b.objectives.values().next().map(|v| v.value).unwrap_or(0.0);
                    a_first
                        .partial_cmp(&b_first)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
            })
            .ok_or_else(|| ParetoError::NoSolutionFound("Failed to find best solution".into()))?;

        Ok(*best)
    }

    /// Select the cheapest solution that meets a quality threshold.
    ///
    /// # Arguments
    /// * `quality_type` - The quality objective type (typically Quality)
    /// * `min_quality` - Minimum required quality value
    /// * `cost_type` - The cost objective to minimize
    ///
    /// # Returns
    /// The cheapest solution meeting the quality threshold,
    /// or an error if no solution meets the threshold.
    pub fn select_by_quality(
        &self,
        quality_type: ObjectiveType,
        min_quality: f64,
        cost_type: ObjectiveType,
    ) -> Result<&ParetoSolution, ParetoError> {
        if self.solutions.is_empty() {
            return Err(ParetoError::EmptyFrontier);
        }

        // Filter solutions that meet the quality threshold
        let meets_quality: Vec<&ParetoSolution> = self
            .solutions
            .iter()
            .filter(|sol| {
                sol.get_value(quality_type)
                    .map(|v| {
                        if quality_type.is_maximized() {
                            v >= min_quality
                        } else {
                            v <= min_quality
                        }
                    })
                    .unwrap_or(false)
            })
            .collect();

        if meets_quality.is_empty() {
            return Err(ParetoError::NoSolutionFound(format!(
                "No solution found with {} {} {}",
                quality_type,
                if quality_type.is_maximized() {
                    ">="
                } else {
                    "<="
                },
                min_quality
            )));
        }

        // Among solutions meeting quality, find the one with lowest cost
        let best = meets_quality
            .iter()
            .min_by(|a, b| {
                let a_cost = a.get_value(cost_type).unwrap_or(f64::MAX);
                let b_cost = b.get_value(cost_type).unwrap_or(f64::MAX);
                a_cost
                    .partial_cmp(&b_cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| ParetoError::NoSolutionFound("Failed to find best solution".into()))?;

        Ok(*best)
    }

    /// Get all solutions sorted by a specific objective.
    pub fn solutions_sorted_by(
        &self,
        objective_type: ObjectiveType,
        ascending: bool,
    ) -> Vec<&ParetoSolution> {
        let mut sorted: Vec<&ParetoSolution> = self.solutions.iter().collect();
        sorted.sort_by(|a, b| {
            let a_val = a.get_value(objective_type).unwrap_or(0.0);
            let b_val = b.get_value(objective_type).unwrap_or(0.0);

            let cmp = a_val
                .partial_cmp(&b_val)
                .unwrap_or(std::cmp::Ordering::Equal);

            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
        sorted
    }
}

impl Default for ParetoFrontier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_solution(id: &str, quality: f64, cost: f64) -> ParetoSolution {
        ParetoSolution::new(id)
            .with_objective(ObjectiveValue::new(ObjectiveType::Quality, quality))
            .with_objective(ObjectiveValue::new(ObjectiveType::Cost, cost))
    }

    #[test]
    fn test_solution_dominates() {
        let sol1 = create_test_solution("1", 0.9, 0.01); // High quality, low cost
        let sol2 = create_test_solution("2", 0.8, 0.02); // Lower quality, higher cost
        let sol3 = create_test_solution("3", 0.9, 0.02); // High quality, higher cost

        assert!(sol1.dominates(&sol2)); // Better on both
        assert!(sol1.dominates(&sol3)); // Better on cost, equal on quality
        assert!(!sol2.dominates(&sol1)); // Worse on both
        assert!(!sol3.dominates(&sol1)); // Worse on cost
    }

    #[test]
    fn test_solution_not_dominates_when_mixed() {
        let sol1 = create_test_solution("1", 0.9, 0.02); // High quality, higher cost
        let sol2 = create_test_solution("2", 0.8, 0.01); // Lower quality, low cost

        // Neither dominates - tradeoff between quality and cost
        assert!(!sol1.dominates(&sol2));
        assert!(!sol2.dominates(&sol1));
    }

    #[test]
    fn test_pareto_frontier_add_dominated() {
        let mut frontier = ParetoFrontier::new();

        let sol1 = create_test_solution("1", 0.9, 0.01); // Best solution
        let sol2 = create_test_solution("2", 0.8, 0.02); // Dominated by sol1

        frontier.add_solution(sol1);
        frontier.add_solution(sol2);

        // sol2 should not be added as it's dominated
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier.solutions[0].id, "1");
    }

    #[test]
    fn test_pareto_frontier_remove_dominated() {
        let mut frontier = ParetoFrontier::new();

        let sol1 = create_test_solution("1", 0.8, 0.02); // Weaker solution
        let sol2 = create_test_solution("2", 0.9, 0.01); // Better solution

        frontier.add_solution(sol1);
        frontier.add_solution(sol2);

        // sol1 should be removed as sol2 dominates it
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier.solutions[0].id, "2");
    }

    #[test]
    fn test_pareto_frontier_multiple_non_dominated() {
        let mut frontier = ParetoFrontier::new();

        let sol1 = create_test_solution("1", 0.9, 0.02); // High quality, higher cost
        let sol2 = create_test_solution("2", 0.8, 0.01); // Lower quality, low cost
        let sol3 = create_test_solution("3", 0.85, 0.015); // Medium tradeoff

        frontier.add_solution(sol1);
        frontier.add_solution(sol2);
        frontier.add_solution(sol3);

        // All three should be on the frontier (non-dominated)
        assert_eq!(frontier.len(), 3);
    }

    #[test]
    fn test_select_by_budget() {
        let frontier = ParetoFrontier::from_solutions(vec![
            create_test_solution("1", 0.9, 0.02),
            create_test_solution("2", 0.85, 0.015),
            create_test_solution("3", 0.8, 0.01),
        ]);

        // With budget of 0.015, should get sol2 (best quality within budget)
        let result = frontier
            .select_by_budget(ObjectiveType::Cost, 0.015)
            .unwrap();
        assert_eq!(result.id, "2");

        // With budget of 0.03, should get sol1 (best quality within budget)
        let result = frontier
            .select_by_budget(ObjectiveType::Cost, 0.03)
            .unwrap();
        assert_eq!(result.id, "1");
    }

    #[test]
    fn test_select_by_quality() {
        let frontier = ParetoFrontier::from_solutions(vec![
            create_test_solution("1", 0.9, 0.02),
            create_test_solution("2", 0.85, 0.015),
            create_test_solution("3", 0.8, 0.01),
        ]);

        // Quality threshold 0.85: should get sol2 (cheapest meeting threshold)
        let result = frontier
            .select_by_quality(ObjectiveType::Quality, 0.85, ObjectiveType::Cost)
            .unwrap();
        assert_eq!(result.id, "2");

        // Quality threshold 0.9: should get sol1 (only one meeting threshold)
        let result = frontier
            .select_by_quality(ObjectiveType::Quality, 0.9, ObjectiveType::Cost)
            .unwrap();
        assert_eq!(result.id, "1");
    }

    #[test]
    fn test_solutions_sorted_by() {
        // Create solutions that form a proper Pareto frontier (no dominance)
        // Solution 1: high quality, high cost
        // Solution 2: medium quality, medium cost
        // Solution 3: low quality, low cost
        let frontier = ParetoFrontier::from_solutions(vec![
            create_test_solution("1", 0.9, 0.02), // High quality, high cost
            create_test_solution("2", 0.85, 0.015), // Medium quality, medium cost
            create_test_solution("3", 0.8, 0.01), // Low quality, low cost
        ]);

        // All three solutions should be non-dominated (form Pareto frontier)
        assert_eq!(frontier.len(), 3);

        // Sort by quality descending
        let sorted = frontier.solutions_sorted_by(ObjectiveType::Quality, false);
        assert_eq!(sorted[0].id, "1"); // 0.9
        assert_eq!(sorted[1].id, "2"); // 0.85
        assert_eq!(sorted[2].id, "3"); // 0.8

        // Sort by cost ascending
        let sorted = frontier.solutions_sorted_by(ObjectiveType::Cost, true);
        assert_eq!(sorted[0].id, "3"); // 0.01
        assert_eq!(sorted[1].id, "2"); // 0.015
        assert_eq!(sorted[2].id, "1"); // 0.02
    }
}
