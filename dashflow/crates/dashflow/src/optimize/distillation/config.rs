// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Configuration for model distillation

use serde::{Deserialize, Serialize};

/// Configuration for the distillation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationConfig {
    /// Number of synthetic examples to generate with the teacher
    pub num_synthetic_examples: usize,

    /// Number of examples to use for training the student
    pub num_training_examples: usize,

    /// Number of bootstrap iterations for student training
    pub num_bootstrap_iterations: usize,

    /// Maximum number of demonstrations per predictor
    pub max_demos: usize,

    /// Batch size for processing examples
    pub batch_size: usize,

    /// Temperature for teacher model (higher = more diverse)
    pub teacher_temperature: f32,

    /// Temperature for student model
    pub student_temperature: f32,

    /// Maximum quality gap threshold (fail if gap exceeds this)
    pub max_quality_gap: f64,

    /// Whether to validate distilled model on test set
    pub validate_on_test: bool,

    /// Random seed for reproducibility
    pub random_seed: Option<u64>,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            num_synthetic_examples: 500,
            num_training_examples: 400,
            num_bootstrap_iterations: 3,
            max_demos: 5,
            batch_size: 10,
            teacher_temperature: 0.7,
            student_temperature: 0.0,
            max_quality_gap: 0.10, // 10% max gap
            validate_on_test: true,
            random_seed: None,
        }
    }
}

/// Builder for DistillationConfig
pub struct DistillationConfigBuilder {
    config: DistillationConfig,
}

impl DistillationConfigBuilder {
    /// Create a new builder with default configuration values.
    pub fn new() -> Self {
        Self {
            config: DistillationConfig::default(),
        }
    }

    /// Set the number of synthetic examples to generate with the teacher model.
    #[must_use]
    pub fn with_num_synthetic_examples(mut self, n: usize) -> Self {
        self.config.num_synthetic_examples = n;
        self
    }

    /// Set the number of examples to use for training the student model.
    #[must_use]
    pub fn with_num_training_examples(mut self, n: usize) -> Self {
        self.config.num_training_examples = n;
        self
    }

    /// Set the number of bootstrap iterations for student training.
    #[must_use]
    pub fn with_num_bootstrap_iterations(mut self, n: usize) -> Self {
        self.config.num_bootstrap_iterations = n;
        self
    }

    /// Set the maximum number of demonstrations per predictor.
    #[must_use]
    pub fn with_max_demos(mut self, n: usize) -> Self {
        self.config.max_demos = n;
        self
    }

    /// Set the batch size for processing examples.
    #[must_use]
    pub fn with_batch_size(mut self, n: usize) -> Self {
        self.config.batch_size = n;
        self
    }

    /// Set the temperature for the teacher model (higher = more diverse outputs).
    #[must_use]
    pub fn with_teacher_temperature(mut self, temp: f32) -> Self {
        self.config.teacher_temperature = temp;
        self
    }

    /// Set the temperature for the student model.
    #[must_use]
    pub fn with_student_temperature(mut self, temp: f32) -> Self {
        self.config.student_temperature = temp;
        self
    }

    /// Set the maximum quality gap threshold (distillation fails if gap exceeds this).
    #[must_use]
    pub fn with_max_quality_gap(mut self, gap: f64) -> Self {
        self.config.max_quality_gap = gap;
        self
    }

    /// Set whether to validate the distilled model on a test set.
    #[must_use]
    pub fn with_validation(mut self, validate: bool) -> Self {
        self.config.validate_on_test = validate;
        self
    }

    /// Set a random seed for reproducibility.
    #[must_use]
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.config.random_seed = Some(seed);
        self
    }

    /// Build the configuration.
    pub fn build(self) -> DistillationConfig {
        self.config
    }
}

impl Default for DistillationConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DistillationConfig {
    /// Create a new configuration builder.
    pub fn builder() -> DistillationConfigBuilder {
        DistillationConfigBuilder::new()
    }

    /// Set the number of synthetic examples to generate.
    #[must_use]
    pub fn with_num_synthetic_examples(mut self, n: usize) -> Self {
        self.num_synthetic_examples = n;
        self
    }

    /// Set the number of training examples.
    #[must_use]
    pub fn with_num_training_examples(mut self, n: usize) -> Self {
        self.num_training_examples = n;
        self
    }

    /// Set the maximum number of demonstrations per predictor.
    #[must_use]
    pub fn with_max_demos(mut self, n: usize) -> Self {
        self.max_demos = n;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DistillationConfig::default();
        assert_eq!(config.num_synthetic_examples, 500);
        assert_eq!(config.num_training_examples, 400);
        assert_eq!(config.max_demos, 5);
        assert_eq!(config.max_quality_gap, 0.10);
    }

    #[test]
    fn test_config_builder() {
        let config = DistillationConfig::builder()
            .with_num_synthetic_examples(1000)
            .with_max_demos(10)
            .with_teacher_temperature(0.8)
            .with_random_seed(42)
            .build();

        assert_eq!(config.num_synthetic_examples, 1000);
        assert_eq!(config.max_demos, 10);
        assert_eq!(config.teacher_temperature, 0.8);
        assert_eq!(config.random_seed, Some(42));
    }

    #[test]
    fn test_config_with_methods() {
        let config = DistillationConfig::default()
            .with_num_synthetic_examples(200)
            .with_num_training_examples(150)
            .with_max_demos(3);

        assert_eq!(config.num_synthetic_examples, 200);
        assert_eq!(config.num_training_examples, 150);
        assert_eq!(config.max_demos, 3);
    }

    #[test]
    fn test_builder_all_options() {
        let config = DistillationConfigBuilder::new()
            .with_num_synthetic_examples(1000)
            .with_num_training_examples(800)
            .with_num_bootstrap_iterations(5)
            .with_max_demos(10)
            .with_batch_size(20)
            .with_teacher_temperature(0.9)
            .with_student_temperature(0.1)
            .with_max_quality_gap(0.15)
            .with_validation(false)
            .with_random_seed(12345)
            .build();

        assert_eq!(config.num_synthetic_examples, 1000);
        assert_eq!(config.num_training_examples, 800);
        assert_eq!(config.num_bootstrap_iterations, 5);
        assert_eq!(config.max_demos, 10);
        assert_eq!(config.batch_size, 20);
        assert_eq!(config.teacher_temperature, 0.9);
        assert_eq!(config.student_temperature, 0.1);
        assert_eq!(config.max_quality_gap, 0.15);
        assert!(!config.validate_on_test);
        assert_eq!(config.random_seed, Some(12345));
    }

    #[test]
    fn test_builder_default() {
        let builder = DistillationConfigBuilder::default();
        let config = builder.build();

        // Should match DistillationConfig::default()
        let default_config = DistillationConfig::default();
        assert_eq!(
            config.num_synthetic_examples,
            default_config.num_synthetic_examples
        );
        assert_eq!(
            config.num_training_examples,
            default_config.num_training_examples
        );
        assert_eq!(config.max_demos, default_config.max_demos);
    }

    #[test]
    fn test_config_builder_static_method() {
        let config = DistillationConfig::builder()
            .with_num_synthetic_examples(250)
            .build();

        assert_eq!(config.num_synthetic_examples, 250);
        // Other values should remain default
        assert_eq!(config.num_training_examples, 400);
    }

    #[test]
    fn test_config_serialization() {
        let config = DistillationConfig::default()
            .with_num_synthetic_examples(100)
            .with_max_demos(3);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DistillationConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.num_synthetic_examples, 100);
        assert_eq!(deserialized.max_demos, 3);
    }

    #[test]
    fn test_config_quality_gap_threshold() {
        let strict = DistillationConfig::builder()
            .with_max_quality_gap(0.05)
            .build();
        assert_eq!(strict.max_quality_gap, 0.05);

        let lenient = DistillationConfig::builder()
            .with_max_quality_gap(0.20)
            .build();
        assert_eq!(lenient.max_quality_gap, 0.20);
    }

    #[test]
    fn test_config_temperature_settings() {
        // Test that teacher has higher temperature for diversity
        // and student has lower temperature for determinism
        let config = DistillationConfig::default();
        assert!(config.teacher_temperature > config.student_temperature);
    }
}
