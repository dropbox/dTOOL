// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Statistical analysis for A/B tests

use crate::optimize::ab_testing::variant::Variant;
use statrs::distribution::{ContinuousCDF, StudentsT};

/// Confidence interval for a statistic
#[derive(Debug, Clone)]
pub struct ConfidenceInterval {
    /// Lower bound of the interval
    pub lower: f64,
    /// Upper bound of the interval
    pub upper: f64,
    /// Confidence level (e.g., 0.95 for 95% confidence)
    pub confidence: f64,
}

impl ConfidenceInterval {
    /// Create a new confidence interval
    pub fn new(lower: f64, upper: f64, confidence: f64) -> Self {
        Self {
            lower,
            upper,
            confidence,
        }
    }

    /// Get the width of the confidence interval
    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }

    /// Check if a value is within the confidence interval
    pub fn contains(&self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }
}

/// Result of a two-sample t-test
#[derive(Debug, Clone)]
pub struct TTestResult {
    /// T-statistic value
    pub t_statistic: f64,
    /// Degrees of freedom
    pub degrees_of_freedom: f64,
    /// P-value (two-tailed)
    pub p_value: f64,
    /// Difference in means (variant1 - variant2)
    pub mean_difference: f64,
    /// Is the result statistically significant?
    pub is_significant: bool,
    /// Significance level used (e.g., 0.05)
    pub significance_level: f64,
}

/// Statistical analysis tools for A/B testing
pub struct StatisticalAnalysis;

impl StatisticalAnalysis {
    /// Perform Welch's t-test (unequal variances assumed)
    ///
    /// Compares means of two variants to determine if the difference
    /// is statistically significant.
    ///
    /// # Arguments
    ///
    /// * `variant1` - First variant (e.g., treatment)
    /// * `variant2` - Second variant (e.g., control)
    /// * `significance_level` - Significance level (typically 0.05)
    ///
    /// # Returns
    ///
    /// TTestResult containing test statistics and significance
    pub fn welch_t_test(
        variant1: &Variant,
        variant2: &Variant,
        significance_level: f64,
    ) -> crate::optimize::ab_testing::Result<TTestResult> {
        let n1 = variant1.sample_size() as f64;
        let n2 = variant2.sample_size() as f64;

        if n1 < 2.0 || n2 < 2.0 {
            return Err(crate::optimize::ab_testing::Error::InsufficientSampleSize {
                need: 2,
                got: n1.min(n2) as usize,
            });
        }

        let mean1 = variant1.mean();
        let mean2 = variant2.mean();
        let var1 = variant1.std_dev().powi(2);
        let var2 = variant2.std_dev().powi(2);

        // Welch's t-statistic
        let mean_diff = mean1 - mean2;
        let se = (var1 / n1 + var2 / n2).sqrt();

        if se == 0.0 {
            // Identical distributions
            return Ok(TTestResult {
                t_statistic: 0.0,
                degrees_of_freedom: n1 + n2 - 2.0,
                p_value: 1.0,
                mean_difference: mean_diff,
                is_significant: false,
                significance_level,
            });
        }

        let t_stat = mean_diff / se;

        // Welch-Satterthwaite degrees of freedom
        let df_numerator = (var1 / n1 + var2 / n2).powi(2);
        let df_denominator = (var1 / n1).powi(2) / (n1 - 1.0) + (var2 / n2).powi(2) / (n2 - 1.0);
        let df = df_numerator / df_denominator;

        // Calculate two-tailed p-value
        let t_dist = StudentsT::new(0.0, 1.0, df).map_err(|e| {
            crate::optimize::ab_testing::Error::StatisticalTestFailed(e.to_string())
        })?;

        let p_value = 2.0 * (1.0 - t_dist.cdf(t_stat.abs()));

        Ok(TTestResult {
            t_statistic: t_stat,
            degrees_of_freedom: df,
            p_value,
            mean_difference: mean_diff,
            is_significant: p_value < significance_level,
            significance_level,
        })
    }

    /// Calculate confidence interval for a variant's mean
    ///
    /// # Arguments
    ///
    /// * `variant` - Variant to analyze
    /// * `confidence` - Confidence level (e.g., 0.95 for 95%)
    ///
    /// # Returns
    ///
    /// ConfidenceInterval for the variant's mean
    pub fn confidence_interval(
        variant: &Variant,
        confidence: f64,
    ) -> crate::optimize::ab_testing::Result<ConfidenceInterval> {
        let n = variant.sample_size() as f64;

        if n < 2.0 {
            return Err(crate::optimize::ab_testing::Error::InsufficientSampleSize {
                need: 2,
                got: n as usize,
            });
        }

        let mean = variant.mean();
        let std_dev = variant.std_dev();
        let se = std_dev / n.sqrt();

        // t-critical value for confidence level
        let df = n - 1.0;
        let alpha = 1.0 - confidence;
        let _t_dist = StudentsT::new(0.0, 1.0, df).map_err(|e| {
            crate::optimize::ab_testing::Error::StatisticalTestFailed(e.to_string())
        })?;

        // Find t-critical using inverse CDF
        let t_critical = Self::t_critical(df, alpha / 2.0)?;

        let margin_of_error = t_critical * se;

        Ok(ConfidenceInterval::new(
            mean - margin_of_error,
            mean + margin_of_error,
            confidence,
        ))
    }

    /// Calculate minimum sample size needed for desired statistical power
    ///
    /// Uses Cohen's d effect size and power analysis.
    ///
    /// # Arguments
    ///
    /// * `effect_size` - Minimum detectable effect (Cohen's d)
    /// * `power` - Desired statistical power (typically 0.8)
    /// * `significance_level` - Significance level (typically 0.05)
    ///
    /// # Returns
    ///
    /// Minimum sample size per variant
    pub fn minimum_sample_size(
        effect_size: f64,
        power: f64,
        significance_level: f64,
    ) -> crate::optimize::ab_testing::Result<usize> {
        // Simplified formula for equal-sized groups
        // n = 2 * ((z_alpha/2 + z_beta) / d)^2
        // where z_alpha/2 is critical value for significance level
        // z_beta is critical value for power
        // d is Cohen's d effect size

        let z_alpha = Self::z_critical(significance_level / 2.0)?;
        let z_beta = Self::z_critical(1.0 - power)?;

        let n = 2.0 * ((z_alpha + z_beta) / effect_size).powi(2);

        Ok(n.ceil() as usize)
    }

    /// Calculate t-critical value for given degrees of freedom and alpha
    fn t_critical(df: f64, alpha: f64) -> crate::optimize::ab_testing::Result<f64> {
        let t_dist = StudentsT::new(0.0, 1.0, df).map_err(|e| {
            crate::optimize::ab_testing::Error::StatisticalTestFailed(e.to_string())
        })?;

        // Binary search for inverse CDF
        let mut low = 0.0;
        let mut high = 10.0;
        let target_prob = 1.0 - alpha;

        for _ in 0..50 {
            let mid = (low + high) / 2.0;
            let prob = t_dist.cdf(mid);

            if (prob - target_prob).abs() < 1e-6 {
                return Ok(mid);
            }

            if prob < target_prob {
                low = mid;
            } else {
                high = mid;
            }
        }

        Ok((low + high) / 2.0)
    }

    /// Calculate z-critical value (standard normal) for given alpha
    fn z_critical(alpha: f64) -> crate::optimize::ab_testing::Result<f64> {
        // Approximate inverse CDF for standard normal
        // Using Abramowitz and Stegun approximation
        let t = (-2.0 * alpha.ln()).sqrt();

        let c0 = 2.515517;
        let c1 = 0.802853;
        let c2 = 0.010328;
        let d1 = 1.432788;
        let d2 = 0.189269;
        let d3 = 0.001308;

        let z = t - (c0 + c1 * t + c2 * t * t) / (1.0 + d1 * t + d2 * t * t + d3 * t * t * t);

        Ok(z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_interval() {
        let mut variant = Variant::new("test", 0.5);
        for i in 1..=10 {
            variant.record(i as f64);
        }

        let ci = StatisticalAnalysis::confidence_interval(&variant, 0.95).unwrap();

        // Mean should be 5.5, CI should contain it
        assert!(ci.contains(5.5));
        assert!(ci.lower < 5.5);
        assert!(ci.upper > 5.5);
    }

    #[test]
    fn test_t_test_identical() {
        let mut variant1 = Variant::new("v1", 0.5);
        let mut variant2 = Variant::new("v2", 0.5);

        for i in 1..=10 {
            variant1.record(i as f64);
            variant2.record(i as f64);
        }

        let result = StatisticalAnalysis::welch_t_test(&variant1, &variant2, 0.05).unwrap();

        // Identical distributions should not be significant
        assert!(!result.is_significant);
        assert!((result.mean_difference).abs() < 0.001);
        assert!(result.p_value > 0.9); // Very high p-value
    }

    #[test]
    fn test_t_test_significant_difference() {
        let mut variant1 = Variant::new("v1", 0.5);
        let mut variant2 = Variant::new("v2", 0.5);

        // Variant1: mean ~= 8, Variant2: mean ~= 2 with small variance
        for i in 0..50 {
            variant1.record(8.0 + (i % 3) as f64 * 0.1);
            variant2.record(2.0 + (i % 3) as f64 * 0.1);
        }

        let result = StatisticalAnalysis::welch_t_test(&variant1, &variant2, 0.05).unwrap();

        // Large difference should be significant
        assert!(result.is_significant);
        assert!((result.mean_difference - 6.0).abs() < 0.1);
        assert!(result.p_value < 0.001);
    }

    #[test]
    fn test_t_test_marginal_difference() {
        let mut variant1 = Variant::new("v1", 0.5);
        let mut variant2 = Variant::new("v2", 0.5);

        // Small sample with small effect - should not be significant
        for i in 1..=10 {
            variant1.record(i as f64);
            variant2.record((i as f64) + 0.1); // Very small difference
        }

        let result = StatisticalAnalysis::welch_t_test(&variant1, &variant2, 0.05).unwrap();

        // Small difference with small sample should not be significant
        assert!(!result.is_significant);
    }

    #[test]
    fn test_insufficient_sample_size() {
        let variant1 = Variant::new("v1", 0.5);
        let variant2 = Variant::new("v2", 0.5);

        let result = StatisticalAnalysis::welch_t_test(&variant1, &variant2, 0.05);
        assert!(result.is_err());
    }

    #[test]
    fn test_minimum_sample_size() {
        // Small effect size should require large sample
        let n_small = StatisticalAnalysis::minimum_sample_size(0.2, 0.8, 0.05).unwrap();
        assert!(n_small > 300);

        // Large effect size should require smaller sample
        let n_large = StatisticalAnalysis::minimum_sample_size(0.8, 0.8, 0.05).unwrap();
        assert!(n_large < n_small);
        assert!(n_large > 20);
    }

    #[test]
    fn test_confidence_interval_width() {
        let mut variant = Variant::new("test", 0.5);
        for i in 1..=100 {
            variant.record(i as f64);
        }

        let ci_95 = StatisticalAnalysis::confidence_interval(&variant, 0.95).unwrap();
        let ci_99 = StatisticalAnalysis::confidence_interval(&variant, 0.99).unwrap();

        // 99% CI should be wider than 95% CI
        assert!(ci_99.width() > ci_95.width());
    }

    #[test]
    fn test_confidence_interval_contains_mean() {
        let mut variant = Variant::new("test", 0.5);
        for i in 1..=20 {
            variant.record(i as f64 * 2.0);
        }

        let ci = StatisticalAnalysis::confidence_interval(&variant, 0.95).unwrap();
        let mean = variant.mean();

        assert!(ci.contains(mean));
    }
}
