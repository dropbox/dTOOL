// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Distribution analysis and class imbalance detection

use crate::optimize::data_collection::types::TrainingExample;
use std::collections::HashMap;

/// Analysis of label distribution in a dataset
#[derive(Debug, Clone)]
pub struct DistributionAnalysis {
    /// Total number of examples
    pub total_examples: usize,

    /// Count of examples per class/label
    pub class_counts: HashMap<String, usize>,

    /// Percentage of examples per class (0.0-1.0)
    pub class_percentages: HashMap<String, f64>,

    /// Classes with insufficient examples
    pub sparse_classes: Vec<String>,

    /// Ratio of most common to least common class
    pub imbalance_ratio: f64,

    /// Most common class
    pub most_common_class: Option<String>,

    /// Least common class
    pub least_common_class: Option<String>,
}

impl DistributionAnalysis {
    /// Check if the dataset is imbalanced based on a threshold
    pub fn is_imbalanced(&self, threshold: f64) -> bool {
        self.imbalance_ratio > threshold
    }

    /// Get classes that need augmentation
    pub fn classes_needing_augmentation(&self, min_percentage: f64) -> Vec<String> {
        self.class_percentages
            .iter()
            .filter(|(_, &pct)| pct < min_percentage)
            .map(|(class, _)| class.clone())
            .collect()
    }
}

/// Analyzer for dataset label distributions
pub struct DistributionAnalyzer {
    /// Minimum number of examples required per class
    min_examples_per_class: usize,

    /// Minimum percentage of examples per class (0.0-1.0)
    balance_threshold: f64,

    /// Output field to analyze (for extracting labels)
    output_field: Option<String>,
}

impl DistributionAnalyzer {
    /// Create a new distribution analyzer with default settings
    pub fn new() -> Self {
        Self {
            min_examples_per_class: 100,
            balance_threshold: 0.1, // 10%
            output_field: None,
        }
    }

    /// Set minimum examples per class
    #[must_use]
    pub fn with_min_examples_per_class(mut self, min: usize) -> Self {
        self.min_examples_per_class = min;
        self
    }

    /// Set balance threshold (e.g., 0.1 = 10% minimum per class)
    #[must_use]
    pub fn with_balance_threshold(mut self, threshold: f64) -> Self {
        self.balance_threshold = threshold;
        self
    }

    /// Set which output field to analyze (defaults to first field if not set)
    #[must_use]
    pub fn with_output_field(mut self, field: impl Into<String>) -> Self {
        self.output_field = Some(field.into());
        self
    }

    /// Analyze the distribution of labels in a dataset
    pub fn analyze(&self, dataset: &[TrainingExample]) -> DistributionAnalysis {
        if dataset.is_empty() {
            return DistributionAnalysis {
                total_examples: 0,
                class_counts: HashMap::new(),
                class_percentages: HashMap::new(),
                sparse_classes: Vec::new(),
                imbalance_ratio: 1.0,
                most_common_class: None,
                least_common_class: None,
            };
        }

        // Count examples per class
        let mut class_counts: HashMap<String, usize> = HashMap::new();

        for example in dataset {
            if let Some(label) = self.extract_label(example) {
                *class_counts.entry(label).or_insert(0) += 1;
            }
        }

        if class_counts.is_empty() {
            return DistributionAnalysis {
                total_examples: dataset.len(),
                class_counts: HashMap::new(),
                class_percentages: HashMap::new(),
                sparse_classes: Vec::new(),
                imbalance_ratio: 1.0,
                most_common_class: None,
                least_common_class: None,
            };
        }

        let total = dataset.len();

        // Calculate percentages
        let class_percentages: HashMap<String, f64> = class_counts
            .iter()
            .map(|(class, &count)| (class.clone(), count as f64 / total as f64))
            .collect();

        // Find sparse classes
        let sparse_classes: Vec<String> = class_counts
            .iter()
            .filter(|(_, &count)| count < self.min_examples_per_class)
            .map(|(class, _)| class.clone())
            .collect();

        // Calculate imbalance ratio
        let max_count = class_counts.values().max().copied().unwrap_or(0);
        let min_count = class_counts.values().min().copied().unwrap_or(0);
        let imbalance_ratio = if min_count > 0 {
            max_count as f64 / min_count as f64
        } else {
            f64::INFINITY
        };

        // Find most/least common classes
        let most_common_class = class_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(class, _)| class.clone());

        let least_common_class = class_counts
            .iter()
            .min_by_key(|(_, &count)| count)
            .map(|(class, _)| class.clone());

        DistributionAnalysis {
            total_examples: total,
            class_counts,
            class_percentages,
            sparse_classes,
            imbalance_ratio,
            most_common_class,
            least_common_class,
        }
    }

    /// Calculate how many synthetic examples are needed per class to achieve balance
    pub fn calculate_augmentation_needed(
        &self,
        analysis: &DistributionAnalysis,
    ) -> HashMap<String, usize> {
        if analysis.class_counts.is_empty() {
            return HashMap::new();
        }

        // Target: Equal number of examples per class (use max class count as target)
        let target_per_class = analysis.class_counts.values().max().copied().unwrap_or(0);

        analysis
            .class_counts
            .iter()
            .filter_map(|(class, &current_count)| {
                let needed = target_per_class.saturating_sub(current_count);
                if needed > 0 {
                    Some((class.clone(), needed))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Extract label from a training example
    fn extract_label(&self, example: &TrainingExample) -> Option<String> {
        if let Some(ref field) = self.output_field {
            example.get_output_field(field)
        } else {
            // Use first output field if not specified
            example.output.values().next().and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                _ => None,
            })
        }
    }
}

impl Default for DistributionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::data_collection::types::DataSource;
    use std::collections::HashMap;

    fn create_example(label: &str) -> TrainingExample {
        let mut input = HashMap::new();
        input.insert("text".to_string(), serde_json::json!("test"));

        let mut output = HashMap::new();
        output.insert("label".to_string(), serde_json::json!(label));

        TrainingExample::new(input, output, DataSource::Production)
    }

    #[test]
    fn test_balanced_distribution() {
        let examples = vec![
            create_example("positive"),
            create_example("negative"),
            create_example("neutral"),
            create_example("positive"),
            create_example("negative"),
            create_example("neutral"),
        ];

        let analyzer = DistributionAnalyzer::new();
        let analysis = analyzer.analyze(&examples);

        assert_eq!(analysis.total_examples, 6);
        assert_eq!(analysis.class_counts.len(), 3);
        assert_eq!(analysis.class_counts.get("positive"), Some(&2));
        assert_eq!(analysis.imbalance_ratio, 1.0); // All classes have 2 examples
    }

    #[test]
    fn test_imbalanced_distribution() {
        let mut examples = Vec::new();

        // 80 positive, 15 neutral, 5 negative
        for _ in 0..80 {
            examples.push(create_example("positive"));
        }
        for _ in 0..15 {
            examples.push(create_example("neutral"));
        }
        for _ in 0..5 {
            examples.push(create_example("negative"));
        }

        let analyzer = DistributionAnalyzer::new().with_min_examples_per_class(100);

        let analysis = analyzer.analyze(&examples);

        assert_eq!(analysis.total_examples, 100);
        assert_eq!(analysis.imbalance_ratio, 80.0 / 5.0); // 16x imbalance

        // Check percentages
        assert!((analysis.class_percentages.get("positive").unwrap() - 0.80).abs() < 0.01);
        assert!((analysis.class_percentages.get("neutral").unwrap() - 0.15).abs() < 0.01);
        assert!((analysis.class_percentages.get("negative").unwrap() - 0.05).abs() < 0.01);

        // All classes are sparse (< 100 examples)
        assert_eq!(analysis.sparse_classes.len(), 3);

        assert_eq!(analysis.most_common_class, Some("positive".to_string()));
        assert_eq!(analysis.least_common_class, Some("negative".to_string()));
    }

    #[test]
    fn test_augmentation_calculation() {
        let mut examples = Vec::new();
        for _ in 0..100 {
            examples.push(create_example("positive"));
        }
        for _ in 0..30 {
            examples.push(create_example("negative"));
        }

        let analyzer = DistributionAnalyzer::new();
        let analysis = analyzer.analyze(&examples);
        let augmentation = analyzer.calculate_augmentation_needed(&analysis);

        // Negative class needs 70 more examples to match positive
        assert_eq!(augmentation.get("negative"), Some(&70));
        // Positive class doesn't need augmentation
        assert_eq!(augmentation.get("positive"), None);
    }

    #[test]
    fn test_is_imbalanced() {
        let examples = vec![
            create_example("a"),
            create_example("a"),
            create_example("a"),
            create_example("a"),
            create_example("a"),
            create_example("b"),
        ];

        let analyzer = DistributionAnalyzer::new();
        let analysis = analyzer.analyze(&examples);

        assert!(analysis.is_imbalanced(3.0)); // 5:1 ratio > 3.0 threshold
        assert!(!analysis.is_imbalanced(10.0)); // 5:1 ratio < 10.0 threshold
    }

    #[test]
    fn test_classes_needing_augmentation() {
        let mut examples = Vec::new();
        for _ in 0..80 {
            examples.push(create_example("common"));
        }
        for _ in 0..15 {
            examples.push(create_example("medium"));
        }
        for _ in 0..5 {
            examples.push(create_example("rare"));
        }

        let analyzer = DistributionAnalyzer::new();
        let analysis = analyzer.analyze(&examples);

        let classes = analysis.classes_needing_augmentation(0.2); // 20% threshold

        // Only common class (80%) is above 20%
        assert_eq!(classes.len(), 2);
        assert!(classes.contains(&"medium".to_string()));
        assert!(classes.contains(&"rare".to_string()));
    }

    #[test]
    fn test_empty_dataset() {
        let analyzer = DistributionAnalyzer::new();
        let analysis = analyzer.analyze(&[]);

        assert_eq!(analysis.total_examples, 0);
        assert_eq!(analysis.class_counts.len(), 0);
        assert_eq!(analysis.imbalance_ratio, 1.0);
        assert_eq!(analysis.most_common_class, None);
    }

    #[test]
    fn test_custom_output_field() {
        let mut input = HashMap::new();
        input.insert("text".to_string(), serde_json::json!("test"));

        let mut output = HashMap::new();
        output.insert("category".to_string(), serde_json::json!("billing"));
        output.insert("label".to_string(), serde_json::json!("ignored"));

        let example = TrainingExample::new(input, output, DataSource::Production);

        let analyzer = DistributionAnalyzer::new().with_output_field("category".to_string());

        let analysis = analyzer.analyze(&[example]);

        assert_eq!(analysis.class_counts.get("billing"), Some(&1));
        assert_eq!(analysis.class_counts.get("ignored"), None);
    }
}
