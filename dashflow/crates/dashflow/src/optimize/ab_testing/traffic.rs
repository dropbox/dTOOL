// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for traffic splitting
// - float_cmp: Comparing split ratios - exact equality intentional for boundary conditions
// - expect_used: expect() on hash-based traffic routing with valid inputs
#![allow(clippy::float_cmp, clippy::expect_used)]

//! Traffic splitting for A/B testing
//!
//! Provides deterministic, hash-based traffic routing to ensure
//! consistent user experiences across test variants.

use sha2::{Digest, Sha256};

/// Traffic splitter for deterministic variant assignment
///
/// Uses SHA-256 hashing to provide:
/// - Deterministic routing: same ID always gets same variant
/// - Uniform distribution: traffic split matches desired percentages
/// - Sticky sessions: users consistently see the same variant
///
/// # Example
///
/// ```
/// use dashflow::optimize::ab_testing::TrafficSplitter;
///
/// let splitter = TrafficSplitter::new(vec![
///     ("control".to_string(), 0.5),
///     ("treatment".to_string(), 0.5),
/// ]).unwrap();
///
/// // Same ID always routes to same variant
/// let v1 = splitter.assign_variant("user_123");
/// let v2 = splitter.assign_variant("user_123");
/// assert_eq!(v1, v2);
/// ```
#[derive(Debug, Clone)]
pub struct TrafficSplitter {
    variants: Vec<(String, f64)>,
    cumulative_weights: Vec<f64>,
}

impl TrafficSplitter {
    /// Create a new traffic splitter with variant allocations
    ///
    /// # Arguments
    ///
    /// * `variants` - List of (variant_name, traffic_weight) tuples
    ///   Traffic weights must sum to 1.0
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - variants list is empty
    /// - traffic weights don't sum to 1.0 (within 0.001 tolerance)
    pub fn new(variants: Vec<(String, f64)>) -> crate::optimize::ab_testing::Result<Self> {
        if variants.is_empty() {
            return Err(crate::optimize::ab_testing::Error::InvalidTrafficAllocation(0.0));
        }

        let total_weight: f64 = variants.iter().map(|(_, w)| w).sum();

        // Allow small floating point error
        if (total_weight - 1.0).abs() > 0.001 {
            return Err(crate::optimize::ab_testing::Error::InvalidTrafficAllocation(total_weight));
        }

        // Build cumulative weights for binary search
        let mut cumulative = Vec::with_capacity(variants.len());
        let mut sum = 0.0;
        for (_, weight) in &variants {
            sum += weight;
            cumulative.push(sum);
        }

        Ok(Self {
            variants,
            cumulative_weights: cumulative,
        })
    }

    /// Assign a variant based on a unique identifier
    ///
    /// Uses SHA-256 hashing to deterministically map IDs to variants.
    /// The same ID will always map to the same variant.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier (user ID, session ID, request ID, etc.)
    ///
    /// # Returns
    ///
    /// The variant name assigned to this ID
    pub fn assign_variant(&self, id: &str) -> &str {
        // Hash the ID to get a deterministic value
        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        let hash = hasher.finalize();

        // Convert first 8 bytes to u64 and normalize to [0, 1)
        let hash_value = u64::from_be_bytes([
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        ]);

        let normalized = (hash_value as f64) / (u64::MAX as f64);

        // Binary search in cumulative weights
        for (i, &cumulative) in self.cumulative_weights.iter().enumerate() {
            if normalized < cumulative {
                return &self.variants[i].0;
            }
        }

        // Fallback to last variant (should never happen due to cumulative = 1.0)
        &self
            .variants
            .last()
            .expect("variants list is guaranteed non-empty by constructor validation")
            .0
    }

    /// Get the list of variants and their traffic allocations
    pub fn variants(&self) -> &[(String, f64)] {
        &self.variants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid_allocation() {
        let splitter = TrafficSplitter::new(vec![
            ("control".to_string(), 0.5),
            ("treatment".to_string(), 0.5),
        ]);
        assert!(splitter.is_ok());
    }

    #[test]
    fn test_new_invalid_allocation() {
        let result = TrafficSplitter::new(vec![
            ("control".to_string(), 0.3),
            ("treatment".to_string(), 0.5),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_assignment() {
        let splitter = TrafficSplitter::new(vec![
            ("control".to_string(), 0.5),
            ("treatment".to_string(), 0.5),
        ])
        .unwrap();

        // Same ID should always get same variant
        let v1 = splitter.assign_variant("user_123");
        let v2 = splitter.assign_variant("user_123");
        let v3 = splitter.assign_variant("user_123");

        assert_eq!(v1, v2);
        assert_eq!(v2, v3);
    }

    #[test]
    fn test_different_ids_vary() {
        let splitter = TrafficSplitter::new(vec![
            ("control".to_string(), 0.5),
            ("treatment".to_string(), 0.5),
        ])
        .unwrap();

        // Different IDs should eventually produce different variants
        let mut control_count = 0;
        let mut treatment_count = 0;

        for i in 0..100 {
            let variant = splitter.assign_variant(&format!("user_{}", i));
            if variant == "control" {
                control_count += 1;
            } else {
                treatment_count += 1;
            }
        }

        // With 100 samples and 50/50 split, we expect roughly 40-60 in each bucket
        assert!((30..=70).contains(&control_count));
        assert!((30..=70).contains(&treatment_count));
    }

    #[test]
    fn test_traffic_distribution() {
        let splitter = TrafficSplitter::new(vec![
            ("a".to_string(), 0.25),
            ("b".to_string(), 0.25),
            ("c".to_string(), 0.5),
        ])
        .unwrap();

        let mut counts = std::collections::HashMap::new();
        counts.insert("a", 0);
        counts.insert("b", 0);
        counts.insert("c", 0);

        // Test with 10000 IDs for better statistical accuracy
        for i in 0..10000 {
            let variant = splitter.assign_variant(&format!("user_{}", i));
            *counts.get_mut(variant).unwrap() += 1;
        }

        // Check distribution is reasonable (within 5% of expected)
        assert!((counts["a"] as f64 / 10000.0 - 0.25).abs() < 0.05);
        assert!((counts["b"] as f64 / 10000.0 - 0.25).abs() < 0.05);
        assert!((counts["c"] as f64 / 10000.0 - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_single_variant() {
        let splitter = TrafficSplitter::new(vec![("only".to_string(), 1.0)]).unwrap();

        for i in 0..100 {
            assert_eq!(splitter.assign_variant(&format!("user_{}", i)), "only");
        }
    }

    #[test]
    fn test_three_way_split() {
        let splitter = TrafficSplitter::new(vec![
            ("control".to_string(), 0.34),
            ("treatment_a".to_string(), 0.33),
            ("treatment_b".to_string(), 0.33),
        ])
        .unwrap();

        let mut counts = std::collections::HashMap::new();
        for i in 0..1000 {
            let variant = splitter.assign_variant(&format!("user_{}", i));
            *counts.entry(variant).or_insert(0) += 1;
        }

        // All three variants should have some traffic
        assert!(counts.len() == 3);
        assert!(counts.values().all(|&c| c > 200)); // At least 20% each
    }
}
