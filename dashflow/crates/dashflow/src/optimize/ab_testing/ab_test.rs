// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for A/B testing
// - expect_used: expect() on variant selection - guaranteed to succeed for valid configs
#![allow(clippy::expect_used)]

//! Core A/B testing functionality

use crate::optimize::ab_testing::analysis::StatisticalAnalysis;
use crate::optimize::ab_testing::report::{ResultsReport, VariantReport};
use crate::optimize::ab_testing::traffic::TrafficSplitter;
use crate::optimize::ab_testing::variant::Variant;

/// A/B test coordinator
///
/// Manages variants, traffic splitting, and statistical analysis
/// for production A/B testing of optimized modules.
///
/// # Example
///
/// ```
/// use dashflow::optimize::ab_testing::ABTest;
///
/// let mut test = ABTest::new("optimizer_comparison")
///     .with_minimum_sample_size(10)  // Small sample size for example
///     .with_significance_level(0.05);
///
/// test.add_variant("control", 0.5);
/// test.add_variant("treatment", 0.5);
///
/// // Record results for multiple users
/// for i in 0..20 {
///     let user_id = format!("user_{}", i);
///     let variant_name = test.assign_variant(&user_id).to_string();
///     // Simulate metric (accuracy, latency, etc.)
///     let metric = if variant_name == "control" { 0.80 } else { 0.85 };
///     test.record_result(&variant_name, metric).unwrap();
/// }
///
/// // Analyze results
/// let report = test.analyze().unwrap();
/// println!("{}", report.summary());
/// ```
pub struct ABTest {
    /// Test name
    name: String,

    /// Traffic splitter for variant assignment
    splitter: Option<TrafficSplitter>,

    /// Variants being tested
    variants: std::collections::HashMap<String, Variant>,

    /// Minimum sample size per variant
    minimum_sample_size: usize,

    /// Significance level for statistical tests (default: 0.05)
    significance_level: f64,
}

impl ABTest {
    /// Create a new A/B test
    ///
    /// # Arguments
    ///
    /// * `name` - Test name for reporting
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            splitter: None,
            variants: std::collections::HashMap::new(),
            minimum_sample_size: 100,
            significance_level: 0.05,
        }
    }

    /// Set minimum sample size per variant (default: 100)
    pub fn with_minimum_sample_size(mut self, size: usize) -> Self {
        self.minimum_sample_size = size;
        self
    }

    /// Set significance level for statistical tests (default: 0.05)
    pub fn with_significance_level(mut self, level: f64) -> Self {
        self.significance_level = level;
        self
    }

    /// Add a variant to the test
    ///
    /// # Arguments
    ///
    /// * `name` - Variant name (e.g., "control", "treatment")
    /// * `traffic` - Traffic allocation (0.0 to 1.0)
    ///
    /// Call this for each variant before starting the test.
    /// After all variants are added, the traffic splitter is automatically configured.
    pub fn add_variant(&mut self, name: impl Into<String>, traffic: f64) {
        let name = name.into();
        self.variants
            .insert(name.clone(), Variant::new(name, traffic));

        // Rebuild splitter with current variants
        self.rebuild_splitter();
    }

    /// Rebuild traffic splitter after variant changes
    fn rebuild_splitter(&mut self) {
        let variant_list: Vec<(String, f64)> = self
            .variants
            .values()
            .map(|v| (v.name.clone(), v.traffic))
            .collect();

        // Only build if we have variants
        if !variant_list.is_empty() {
            self.splitter = TrafficSplitter::new(variant_list).ok();
        }
    }

    /// Assign a variant to a unique identifier
    ///
    /// Uses deterministic hashing to ensure consistent assignment.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier (user ID, session ID, etc.)
    ///
    /// # Returns
    ///
    /// Variant name assigned to this ID
    ///
    /// # Panics
    ///
    /// Panics if no variants have been added yet
    pub fn assign_variant(&self, id: &str) -> &str {
        self.splitter
            .as_ref()
            .expect("No variants configured")
            .assign_variant(id)
    }

    /// Record a metric observation for a variant
    ///
    /// # Arguments
    ///
    /// * `variant_name` - Name of the variant
    /// * `value` - Metric value (e.g., accuracy, conversion rate)
    pub fn record_result(
        &mut self,
        variant_name: &str,
        value: f64,
    ) -> crate::optimize::ab_testing::Result<()> {
        let variant = self.variants.get_mut(variant_name).ok_or_else(|| {
            crate::optimize::ab_testing::Error::VariantNotFound(variant_name.to_string())
        })?;

        variant.record(value);
        Ok(())
    }

    /// Get a variant by name
    pub fn get_variant(&self, name: &str) -> Option<&Variant> {
        self.variants.get(name)
    }

    /// Get mutable variant by name
    pub fn get_variant_mut(&mut self, name: &str) -> Option<&mut Variant> {
        self.variants.get_mut(name)
    }

    /// Check if test has reached minimum sample size
    pub fn has_minimum_samples(&self) -> bool {
        self.variants
            .values()
            .all(|v| v.sample_size() >= self.minimum_sample_size)
    }

    /// Analyze test results and generate report
    ///
    /// Performs statistical analysis and determines winner (if any).
    ///
    /// # Returns
    ///
    /// ResultsReport with analysis and recommendations
    ///
    /// # Errors
    ///
    /// Returns error if insufficient sample size or statistical test fails
    pub fn analyze(&self) -> crate::optimize::ab_testing::Result<ResultsReport> {
        let mut report = ResultsReport::new(self.name.clone());

        // Check minimum sample size
        for variant in self.variants.values() {
            if variant.sample_size() < self.minimum_sample_size {
                return Err(crate::optimize::ab_testing::Error::InsufficientSampleSize {
                    need: self.minimum_sample_size,
                    got: variant.sample_size(),
                });
            }
        }

        // Generate variant reports
        for variant in self.variants.values() {
            let ci = StatisticalAnalysis::confidence_interval(variant, 0.95)?;
            let vr = VariantReport::new(
                variant.name.clone(),
                variant.sample_size(),
                variant.mean(),
                variant.std_dev(),
                ci,
            );
            report.add_variant(vr);
        }

        // For two-variant tests, run t-test
        if self.variants.len() == 2 {
            let variants: Vec<&Variant> = self.variants.values().collect();
            let v1 = variants[0];
            let v2 = variants[1];

            let t_test = StatisticalAnalysis::welch_t_test(v1, v2, self.significance_level)?;
            report.set_t_test(t_test.clone());

            if t_test.is_significant {
                let winner = if t_test.mean_difference > 0.0 {
                    &v1.name
                } else {
                    &v2.name
                };

                let recommendation = format!(
                    "Deploy {} variant (p < {:.3})",
                    winner, self.significance_level
                );

                report.set_winner(winner.clone(), recommendation);
            } else {
                report.set_winner(
                    "No clear winner".to_string(),
                    format!(
                        "Continue testing or implement based on other criteria (p = {:.3})",
                        t_test.p_value
                    ),
                );
            }
        }

        Ok(report)
    }

    /// Get test name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get all variant names
    pub fn variant_names(&self) -> Vec<&str> {
        self.variants.keys().map(|s| s.as_str()).collect()
    }

    /// Get total number of observations across all variants
    pub fn total_observations(&self) -> usize {
        self.variants.values().map(|v| v.sample_size()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_ab_test() {
        let test = ABTest::new("test1");
        assert_eq!(test.name(), "test1");
        assert_eq!(test.total_observations(), 0);
    }

    #[test]
    fn test_add_variants() {
        let mut test = ABTest::new("test1");
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        assert_eq!(test.variant_names().len(), 2);
        assert!(test.get_variant("control").is_some());
        assert!(test.get_variant("treatment").is_some());
    }

    #[test]
    fn test_assign_variant() {
        let mut test = ABTest::new("test1");
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        // Should be deterministic
        let v1 = test.assign_variant("user_123");
        let v2 = test.assign_variant("user_123");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_record_result() {
        let mut test = ABTest::new("test1");
        test.add_variant("control", 0.5);

        test.record_result("control", 0.8).unwrap();
        test.record_result("control", 0.9).unwrap();

        let variant = test.get_variant("control").unwrap();
        assert_eq!(variant.sample_size(), 2);
        assert!((variant.mean() - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_record_invalid_variant() {
        let mut test = ABTest::new("test1");
        test.add_variant("control", 0.5);

        let result = test.record_result("invalid", 0.8);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_minimum_samples() {
        let mut test = ABTest::new("test1").with_minimum_sample_size(10);
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        assert!(!test.has_minimum_samples());

        // Add 10 samples to each
        for _ in 0..10 {
            test.record_result("control", 0.8).unwrap();
            test.record_result("treatment", 0.9).unwrap();
        }

        assert!(test.has_minimum_samples());
    }

    #[test]
    fn test_analyze_insufficient_samples() {
        let mut test = ABTest::new("test1").with_minimum_sample_size(100);
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        test.record_result("control", 0.8).unwrap();
        test.record_result("treatment", 0.9).unwrap();

        let result = test.analyze();
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_with_sufficient_samples() {
        let mut test = ABTest::new("test1").with_minimum_sample_size(30);
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        // Add samples with clear difference
        for _ in 0..50 {
            test.record_result("control", 0.6).unwrap();
            test.record_result("treatment", 0.9).unwrap();
        }

        let report = test.analyze().unwrap();
        assert_eq!(report.variants.len(), 2);
        assert!(report.t_test.is_some());

        let t_test = report.t_test.unwrap();
        assert!(t_test.is_significant);
    }

    #[test]
    fn test_analyze_no_significant_difference() {
        let mut test = ABTest::new("test1").with_minimum_sample_size(30);
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        // Add identical samples
        for _ in 0..50 {
            test.record_result("control", 0.8).unwrap();
            test.record_result("treatment", 0.8).unwrap();
        }

        let report = test.analyze().unwrap();
        let t_test = report.t_test.unwrap();
        assert!(!t_test.is_significant);
    }

    #[test]
    fn test_total_observations() {
        let mut test = ABTest::new("test1");
        test.add_variant("control", 0.5);
        test.add_variant("treatment", 0.5);

        test.record_result("control", 0.8).unwrap();
        test.record_result("treatment", 0.9).unwrap();
        test.record_result("control", 0.7).unwrap();

        assert_eq!(test.total_observations(), 3);
    }

    #[test]
    fn test_with_minimum_sample_size() {
        let test = ABTest::new("test1").with_minimum_sample_size(200);
        assert_eq!(test.minimum_sample_size, 200);
    }

    #[test]
    fn test_with_significance_level() {
        let test = ABTest::new("test1").with_significance_level(0.01);
        assert_eq!(test.significance_level, 0.01);
    }
}
