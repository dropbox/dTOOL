//! @dashflow-module
//! @name optimizer_types
//! @category optimize
//! @status stable
//!
//! # Shared Types for Optimizer Algorithms
//!
//! This module provides shared type definitions used across all optimizers.
//! By consolidating these types, we ensure consistency and reduce code duplication.
//!
//! ## Types
//!
//! - [`MetricFn`] - Standard metric function comparing prediction to expected output
//! - [`MetricWithFeedbackFn`] - Metric function that also returns textual feedback
//! - [`Candidate`] - A candidate solution being evaluated during optimization
//! - [`CandidatePool`] - Collection of candidates for evolutionary/search optimizers

use crate::optimize::example::Example;
use std::sync::Arc;

/// Standard metric function: compares prediction to expected, returns score 0.0-1.0
///
/// This is the most common metric function type used by optimizers.
/// Higher scores indicate better matches between predicted and expected outputs.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::types::MetricFn;
/// use std::sync::Arc;
///
/// let metric: MetricFn = Arc::new(|pred, expected| {
///     if pred.get("output") == expected.get("output") {
///         1.0
///     } else {
///         0.0
///     }
/// });
/// ```
pub type MetricFn = Arc<dyn Fn(&Example, &Example) -> f64 + Send + Sync>;

/// Metric function that also returns optional textual feedback
///
/// The feedback string can provide explanation for why a particular score
/// was given, useful for debugging and for optimizers that use feedback
/// to guide search (like SIMBA or COPRO).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::types::MetricWithFeedbackFn;
/// use std::sync::Arc;
///
/// let metric: MetricWithFeedbackFn = Arc::new(|pred, expected| {
///     let score = if pred.get("output") == expected.get("output") {
///         1.0
///     } else {
///         0.0
///     };
///     let feedback = if score < 1.0 {
///         Some("Output does not match expected".to_string())
///     } else {
///         None
///     };
///     (score, feedback)
/// });
/// ```
pub type MetricWithFeedbackFn =
    Arc<dyn Fn(&Example, &Example) -> (f64, Option<String>) + Send + Sync>;

/// A candidate solution being evaluated during optimization
///
/// Candidates represent potential optimized versions of a signature,
/// including the instruction text, optional prefix, few-shot examples,
/// and evaluation score.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// The instruction text for this candidate
    pub instruction: String,

    /// Optional output prefix (hint for the model)
    pub prefix: Option<String>,

    /// Few-shot examples (if any)
    pub demos: Vec<Example>,

    /// Evaluation score (0.0-1.0, higher is better)
    pub score: f64,

    /// Which iteration/depth this candidate came from
    pub iteration: usize,
}

impl Default for Candidate {
    fn default() -> Self {
        Self {
            instruction: String::new(),
            prefix: None,
            demos: vec![],
            score: 0.0,
            iteration: 0,
        }
    }
}

impl Candidate {
    /// Create a new candidate with the given instruction
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            ..Default::default()
        }
    }

    /// Set the evaluation score
    #[must_use]
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = score;
        self
    }

    /// Set the few-shot examples
    #[must_use]
    pub fn with_demos(mut self, demos: Vec<Example>) -> Self {
        self.demos = demos;
        self
    }

    /// Set the output prefix
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the iteration this candidate came from
    #[must_use]
    pub fn with_iteration(mut self, iteration: usize) -> Self {
        self.iteration = iteration;
        self
    }

    /// Check if this candidate has a better score than another
    pub fn is_better_than(&self, other: &Candidate) -> bool {
        self.score > other.score
    }
}

/// Pool of candidates for evolutionary/search optimizers
///
/// CandidatePool manages a collection of candidates, automatically
/// sorting by score and optionally enforcing a maximum size.
#[derive(Clone, Debug, Default)]
pub struct CandidatePool {
    candidates: Vec<Candidate>,
    max_size: Option<usize>,
}

impl CandidatePool {
    /// Create a new empty candidate pool
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum pool size (removes worst candidates when exceeded)
    #[must_use]
    pub fn with_max_size(mut self, max: usize) -> Self {
        self.max_size = Some(max);
        self
    }

    /// Add a candidate to the pool
    ///
    /// If max_size is set and exceeded, the worst candidate is removed.
    pub fn add(&mut self, candidate: Candidate) {
        self.candidates.push(candidate);
        if let Some(max) = self.max_size {
            if self.candidates.len() > max {
                // Sort by score descending and truncate
                self.candidates.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.candidates.truncate(max);
            }
        }
    }

    /// Get the best candidate (highest score)
    pub fn best(&self) -> Option<&Candidate> {
        self.candidates.iter().max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get all candidates sorted by score (best first)
    pub fn sorted_by_score(&self) -> Vec<&Candidate> {
        let mut sorted: Vec<_> = self.candidates.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Get the top N candidates by score
    pub fn top_n(&self, n: usize) -> Vec<&Candidate> {
        self.sorted_by_score().into_iter().take(n).collect()
    }

    /// Get number of candidates in pool
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get average score of all candidates
    pub fn average_score(&self) -> f64 {
        if self.candidates.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.candidates.iter().map(|c| c.score).sum();
        sum / self.candidates.len() as f64
    }

    /// Iterate over all candidates
    pub fn iter(&self) -> impl Iterator<Item = &Candidate> {
        self.candidates.iter()
    }

    /// Get mutable iterator over all candidates
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Candidate> {
        self.candidates.iter_mut()
    }

    /// Remove all candidates below a score threshold
    pub fn prune_below(&mut self, threshold: f64) {
        self.candidates.retain(|c| c.score >= threshold);
    }
}

impl IntoIterator for CandidatePool {
    type Item = Candidate;
    type IntoIter = std::vec::IntoIter<Candidate>;

    fn into_iter(self) -> Self::IntoIter {
        self.candidates.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_builder_pattern() {
        let candidate = Candidate::new("Test instruction")
            .with_score(0.85)
            .with_prefix("Answer:")
            .with_iteration(3);

        assert_eq!(candidate.instruction, "Test instruction");
        assert_eq!(candidate.score, 0.85);
        assert_eq!(candidate.prefix, Some("Answer:".to_string()));
        assert_eq!(candidate.iteration, 3);
    }

    #[test]
    fn test_candidate_is_better_than() {
        let c1 = Candidate::new("A").with_score(0.8);
        let c2 = Candidate::new("B").with_score(0.6);

        assert!(c1.is_better_than(&c2));
        assert!(!c2.is_better_than(&c1));
    }

    #[test]
    fn test_candidate_pool_add_and_best() {
        let mut pool = CandidatePool::new();
        pool.add(Candidate::new("A").with_score(0.5));
        pool.add(Candidate::new("B").with_score(0.9));
        pool.add(Candidate::new("C").with_score(0.7));

        let best = pool.best().unwrap();
        assert_eq!(best.instruction, "B");
        assert_eq!(best.score, 0.9);
    }

    #[test]
    fn test_candidate_pool_max_size() {
        let mut pool = CandidatePool::new().with_max_size(2);
        pool.add(Candidate::new("A").with_score(0.5));
        pool.add(Candidate::new("B").with_score(0.9));
        pool.add(Candidate::new("C").with_score(0.7));

        // Should only have 2 candidates: B (0.9) and C (0.7)
        assert_eq!(pool.len(), 2);

        let sorted = pool.sorted_by_score();
        assert_eq!(sorted[0].instruction, "B");
        assert_eq!(sorted[1].instruction, "C");
    }

    #[test]
    fn test_candidate_pool_average_score() {
        let mut pool = CandidatePool::new();
        pool.add(Candidate::new("A").with_score(0.4));
        pool.add(Candidate::new("B").with_score(0.6));
        pool.add(Candidate::new("C").with_score(0.8));

        let avg = pool.average_score();
        assert!((avg - 0.6).abs() < 0.0001);
    }

    #[test]
    fn test_candidate_pool_prune_below() {
        let mut pool = CandidatePool::new();
        pool.add(Candidate::new("A").with_score(0.3));
        pool.add(Candidate::new("B").with_score(0.7));
        pool.add(Candidate::new("C").with_score(0.5));

        pool.prune_below(0.5);

        assert_eq!(pool.len(), 2);
        assert!(pool.iter().all(|c| c.score >= 0.5));
    }

    #[test]
    fn test_candidate_pool_top_n() {
        let mut pool = CandidatePool::new();
        pool.add(Candidate::new("A").with_score(0.3));
        pool.add(Candidate::new("B").with_score(0.9));
        pool.add(Candidate::new("C").with_score(0.6));
        pool.add(Candidate::new("D").with_score(0.5));

        let top2 = pool.top_n(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].instruction, "B");
        assert_eq!(top2[1].instruction, "C");
    }

    #[test]
    fn test_metric_fn_type() {
        let metric: MetricFn = Arc::new(|pred, expected| {
            if pred.get("output") == expected.get("output") {
                1.0
            } else {
                0.0
            }
        });

        let pred = Example::new().with("output", serde_json::json!("hello"));
        let expected = Example::new().with("output", serde_json::json!("hello"));

        assert_eq!(metric(&pred, &expected), 1.0);
    }
}
