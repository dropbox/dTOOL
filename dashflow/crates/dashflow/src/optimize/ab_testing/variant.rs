// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Variant definitions for A/B testing

/// A test variant with recorded results
///
/// Represents one variant in an A/B test, tracking all metric
/// observations for statistical analysis.
#[derive(Debug, Clone)]
pub struct Variant {
    /// Variant name (e.g., "control", "treatment")
    pub name: String,

    /// Traffic allocation (0.0 to 1.0)
    pub traffic: f64,

    /// Recorded metric values
    pub observations: Vec<f64>,
}

impl Variant {
    /// Create a new variant
    ///
    /// # Arguments
    ///
    /// * `name` - Variant name
    /// * `traffic` - Traffic allocation (0.0 to 1.0)
    pub fn new(name: impl Into<String>, traffic: f64) -> Self {
        Self {
            name: name.into(),
            traffic,
            observations: Vec::new(),
        }
    }

    /// Record an observation for this variant
    pub fn record(&mut self, value: f64) {
        self.observations.push(value);
    }

    /// Get number of observations
    pub fn sample_size(&self) -> usize {
        self.observations.len()
    }

    /// Calculate mean of observations
    pub fn mean(&self) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        self.observations.iter().sum::<f64>() / self.observations.len() as f64
    }

    /// Calculate standard deviation of observations
    pub fn std_dev(&self) -> f64 {
        if self.observations.len() < 2 {
            return 0.0;
        }

        let mean = self.mean();
        let variance = self
            .observations
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / (self.observations.len() - 1) as f64;

        variance.sqrt()
    }

    /// Get observations
    pub fn observations(&self) -> &[f64] {
        &self.observations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_variant() {
        let variant = Variant::new("control", 0.5);
        assert_eq!(variant.name, "control");
        assert_eq!(variant.traffic, 0.5);
        assert_eq!(variant.sample_size(), 0);
    }

    #[test]
    fn test_record_observations() {
        let mut variant = Variant::new("test", 0.5);
        variant.record(0.8);
        variant.record(0.9);
        variant.record(0.7);

        assert_eq!(variant.sample_size(), 3);
        assert_eq!(variant.observations(), &[0.8, 0.9, 0.7]);
    }

    #[test]
    fn test_mean_empty() {
        let variant = Variant::new("test", 0.5);
        assert_eq!(variant.mean(), 0.0);
    }

    #[test]
    fn test_mean_single() {
        let mut variant = Variant::new("test", 0.5);
        variant.record(0.75);
        assert_eq!(variant.mean(), 0.75);
    }

    #[test]
    fn test_mean_multiple() {
        let mut variant = Variant::new("test", 0.5);
        variant.record(0.6);
        variant.record(0.8);
        variant.record(1.0);

        let mean = variant.mean();
        assert!((mean - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_std_dev_empty() {
        let variant = Variant::new("test", 0.5);
        assert_eq!(variant.std_dev(), 0.0);
    }

    #[test]
    fn test_std_dev_single() {
        let mut variant = Variant::new("test", 0.5);
        variant.record(0.5);
        assert_eq!(variant.std_dev(), 0.0);
    }

    #[test]
    fn test_std_dev_multiple() {
        let mut variant = Variant::new("test", 0.5);
        // Values: 2, 4, 6, 8
        // Mean: 5
        // Variance: ((2-5)^2 + (4-5)^2 + (6-5)^2 + (8-5)^2) / 3 = (9 + 1 + 1 + 9) / 3 = 20/3
        // StdDev: sqrt(20/3) = 2.582
        variant.record(2.0);
        variant.record(4.0);
        variant.record(6.0);
        variant.record(8.0);

        let std_dev = variant.std_dev();
        assert!((std_dev - 2.582).abs() < 0.01);
    }

    #[test]
    fn test_identical_values() {
        let mut variant = Variant::new("test", 0.5);
        variant.record(0.5);
        variant.record(0.5);
        variant.record(0.5);

        assert_eq!(variant.mean(), 0.5);
        assert_eq!(variant.std_dev(), 0.0);
    }
}
