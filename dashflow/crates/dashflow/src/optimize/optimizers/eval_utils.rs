//! @dashflow-module
//! @name optimizer_eval_utils
//! @category optimize
//! @status stable
//!
//! # Shared Evaluation Utilities for Optimizers
//!
//! This module provides shared utilities used by multiple optimizers for
//! evaluating candidates, normalizing scores, and sampling from distributions.
//!
//! ## Utilities
//!
//! - [`evaluate_examples`] - Evaluate a set of examples with a metric function
//! - [`softmax_normalize`] - Normalize scores using softmax
//! - [`weighted_sample`] - Sample from items using weighted probabilities
//! - [`min_max_normalize`] - Normalize scores to 0.0-1.0 range

use super::types::MetricFn;
use crate::optimize::example::Example;
use rand::Rng;

/// Evaluate a set of examples with a metric function
///
/// Returns a vector of scores for each (prediction, expected) pair.
///
/// # Arguments
///
/// * `predictions` - Predicted outputs from the model
/// * `expected` - Expected/ground truth outputs
/// * `metric` - Metric function to evaluate each pair
///
/// # Returns
///
/// Vector of scores, one per example pair.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::eval_utils::evaluate_examples;
/// use std::sync::Arc;
///
/// let predictions = vec![pred1, pred2, pred3];
/// let expected = vec![exp1, exp2, exp3];
/// let metric = Arc::new(|p, e| if p == e { 1.0 } else { 0.0 });
///
/// let scores = evaluate_examples(&predictions, &expected, &metric);
/// let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
/// ```
pub fn evaluate_examples(
    predictions: &[Example],
    expected: &[Example],
    metric: &MetricFn,
) -> Vec<f64> {
    predictions
        .iter()
        .zip(expected.iter())
        .map(|(pred, exp)| metric(pred, exp))
        .collect()
}

/// Compute average score from a list of scores
pub fn average_score(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    scores.iter().sum::<f64>() / scores.len() as f64
}

/// Normalize scores using softmax function
///
/// Converts a vector of scores to a probability distribution
/// that sums to 1.0, using the softmax function with temperature.
///
/// # Arguments
///
/// * `scores` - Raw scores to normalize
/// * `temperature` - Temperature parameter (higher = more uniform distribution)
///
/// # Returns
///
/// Vector of probabilities summing to 1.0.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::eval_utils::softmax_normalize;
///
/// let scores = vec![1.0, 2.0, 3.0];
/// let probs = softmax_normalize(&scores, 1.0);
/// // probs now contains probabilities that sum to 1.0
/// ```
pub fn softmax_normalize(scores: &[f64], temperature: f64) -> Vec<f64> {
    if scores.is_empty() {
        return vec![];
    }

    if temperature == 0.0 {
        // With temp=0, return one-hot for max score
        let max_idx = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let mut result = vec![0.0; scores.len()];
        result[max_idx] = 1.0;
        return result;
    }

    // Find max for numerical stability
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let exp_scores: Vec<f64> = scores
        .iter()
        .map(|s| ((s - max_score) / temperature).exp())
        .collect();

    let sum: f64 = exp_scores.iter().sum();

    if sum == 0.0 {
        // If all exp values are 0, return uniform distribution
        vec![1.0 / scores.len() as f64; scores.len()]
    } else {
        exp_scores.iter().map(|e| e / sum).collect()
    }
}

/// Sample from a distribution using provided probabilities
///
/// # Arguments
///
/// * `items` - Items to sample from
/// * `probs` - Probability for each item (should sum to ~1.0)
/// * `rng` - Random number generator
///
/// # Returns
///
/// A reference to the sampled item, or None if inputs are empty.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::eval_utils::weighted_sample;
/// use rand::thread_rng;
///
/// let items = vec!["a", "b", "c"];
/// let probs = vec![0.2, 0.5, 0.3];
/// let mut rng = thread_rng();
///
/// if let Some(sampled) = weighted_sample(&items, &probs, &mut rng) {
///     println!("Sampled: {}", sampled);
/// }
/// ```
pub fn weighted_sample<'a, T>(items: &'a [T], probs: &[f64], rng: &mut impl Rng) -> Option<&'a T> {
    if items.is_empty() || probs.is_empty() || items.len() != probs.len() {
        return None;
    }

    let r: f64 = rng.gen();
    let mut cumsum = 0.0;

    for (item, prob) in items.iter().zip(probs.iter()) {
        cumsum += prob;
        if r <= cumsum {
            return Some(item);
        }
    }

    // Fallback to last item (handles floating point imprecision)
    items.last()
}

/// Sample N items from a distribution using weighted probabilities (with replacement)
pub fn weighted_sample_n<T: Clone>(
    items: &[T],
    probs: &[f64],
    n: usize,
    rng: &mut impl Rng,
) -> Vec<T> {
    let mut result = Vec::with_capacity(n);
    for _ in 0..n {
        if let Some(item) = weighted_sample(items, probs, rng) {
            result.push(item.clone());
        }
    }
    result
}

/// Normalize scores to 0.0-1.0 range using min-max normalization
///
/// # Arguments
///
/// * `scores` - Raw scores to normalize
///
/// # Returns
///
/// Vector of normalized scores in [0.0, 1.0] range.
pub fn min_max_normalize(scores: &[f64]) -> Vec<f64> {
    if scores.is_empty() {
        return vec![];
    }

    let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let range = max_score - min_score;
    if range == 0.0 {
        // All scores are the same
        return vec![1.0; scores.len()];
    }

    scores.iter().map(|s| (s - min_score) / range).collect()
}

/// Rank scores and return ranks (1-based, ties get same rank)
///
/// # Arguments
///
/// * `scores` - Scores to rank
/// * `descending` - If true, higher scores get lower ranks (rank 1 = highest)
///
/// # Returns
///
/// Vector of ranks (1-based).
pub fn rank_scores(scores: &[f64], descending: bool) -> Vec<usize> {
    if scores.is_empty() {
        return vec![];
    }

    let mut indexed: Vec<(usize, f64)> = scores.iter().cloned().enumerate().collect();

    if descending {
        indexed.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        indexed.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    }

    let mut ranks = vec![0usize; scores.len()];
    let mut current_rank = 1;

    for i in 0..indexed.len() {
        let (original_idx, score) = indexed[i];

        // Check if this is a tie with the previous score
        if i > 0 {
            let (_, prev_score) = indexed[i - 1];
            if (score - prev_score).abs() > f64::EPSILON {
                current_rank = i + 1;
            }
        }

        ranks[original_idx] = current_rank;
    }

    ranks
}

/// Compute standard deviation of scores
pub fn std_dev(scores: &[f64]) -> f64 {
    if scores.len() < 2 {
        return 0.0;
    }

    let mean = average_score(scores);
    let variance =
        scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (scores.len() - 1) as f64;
    variance.sqrt()
}

/// Compute percentile value from scores
///
/// # Arguments
///
/// * `scores` - Scores to analyze
/// * `percentile` - Percentile to compute (0.0 to 100.0)
///
/// # Returns
///
/// The percentile value.
pub fn percentile(scores: &[f64], percentile: f64) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }

    let mut sorted: Vec<f64> = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p = percentile.clamp(0.0, 100.0) / 100.0;
    let idx = ((sorted.len() - 1) as f64 * p) as usize;

    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_example(value: &str) -> Example {
        Example::new().with("output", serde_json::json!(value))
    }

    #[test]
    fn test_evaluate_examples() {
        let metric: MetricFn = Arc::new(|pred, exp| {
            if pred.get("output") == exp.get("output") {
                1.0
            } else {
                0.0
            }
        });

        let preds = vec![
            create_example("a"),
            create_example("b"),
            create_example("c"),
        ];
        let expected = vec![
            create_example("a"),
            create_example("x"),
            create_example("c"),
        ];

        let scores = evaluate_examples(&preds, &expected, &metric);
        assert_eq!(scores, vec![1.0, 0.0, 1.0]);
    }

    #[test]
    fn test_average_score() {
        assert_eq!(average_score(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(average_score(&[]), 0.0);
        assert_eq!(average_score(&[5.0]), 5.0);
    }

    #[test]
    fn test_softmax_normalize() {
        let scores = vec![1.0, 2.0, 3.0];
        let probs = softmax_normalize(&scores, 1.0);

        // Sum should be ~1.0
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 0.0001);

        // Higher scores should have higher probabilities
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_softmax_empty() {
        let probs = softmax_normalize(&[], 1.0);
        assert!(probs.is_empty());
    }

    #[test]
    fn test_softmax_temp_zero() {
        let scores = vec![1.0, 3.0, 2.0];
        let probs = softmax_normalize(&scores, 0.0);

        // Should be one-hot for max
        assert_eq!(probs[0], 0.0);
        assert_eq!(probs[1], 1.0);
        assert_eq!(probs[2], 0.0);
    }

    #[test]
    fn test_weighted_sample() {
        let items = vec!["a", "b", "c"];
        let probs = vec![0.0, 1.0, 0.0]; // Always select "b"
        let mut rng = rand::thread_rng();

        for _ in 0..10 {
            let sampled = weighted_sample(&items, &probs, &mut rng);
            assert_eq!(sampled, Some(&"b"));
        }
    }

    #[test]
    fn test_min_max_normalize() {
        let scores = vec![10.0, 20.0, 30.0, 40.0];
        let normalized = min_max_normalize(&scores);

        assert_eq!(normalized[0], 0.0);
        assert!((normalized[1] - 1.0 / 3.0).abs() < 0.0001);
        assert!((normalized[2] - 2.0 / 3.0).abs() < 0.0001);
        assert_eq!(normalized[3], 1.0);
    }

    #[test]
    fn test_min_max_normalize_same_values() {
        let scores = vec![5.0, 5.0, 5.0];
        let normalized = min_max_normalize(&scores);
        assert_eq!(normalized, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_rank_scores_descending() {
        let scores = vec![30.0, 10.0, 20.0, 40.0];
        let ranks = rank_scores(&scores, true);

        // 40.0 = rank 1, 30.0 = rank 2, 20.0 = rank 3, 10.0 = rank 4
        assert_eq!(ranks, vec![2, 4, 3, 1]);
    }

    #[test]
    fn test_rank_scores_ascending() {
        let scores = vec![30.0, 10.0, 20.0, 40.0];
        let ranks = rank_scores(&scores, false);

        // 10.0 = rank 1, 20.0 = rank 2, 30.0 = rank 3, 40.0 = rank 4
        assert_eq!(ranks, vec![3, 1, 2, 4]);
    }

    #[test]
    fn test_std_dev() {
        // Known standard deviation: [2, 4, 4, 4, 5, 5, 7, 9] has std_dev ≈ 2.14
        let scores = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&scores);
        assert!((sd - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_percentile() {
        let scores = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(percentile(&scores, 0.0), 1.0);
        assert_eq!(percentile(&scores, 50.0), 3.0);
        assert_eq!(percentile(&scores, 100.0), 5.0);
    }
}
