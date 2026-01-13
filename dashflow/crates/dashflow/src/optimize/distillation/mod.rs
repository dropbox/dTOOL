// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @cli dashflow train distill
//! @cli-status wired
//!
//! Model Distillation Framework
//!
//! This module provides teacher-student model distillation for cost optimization.
//! A teacher model (e.g., GPT-4) generates high-quality training data, which is
//! then used to train a student model (e.g., GPT-3.5) to achieve similar quality
//! at significantly lower cost.
//!
//! # Overview
//!
//! Model distillation allows you to:
//! 1. Use an expensive, high-quality teacher model (GPT-4) to generate training data
//! 2. Train a cheaper student model (GPT-3.5, local model) on this data
//! 3. Achieve 90-95% of teacher quality at 5-10x lower cost per request
//! 4. Compare multiple distillation approaches (fine-tuning vs prompt optimization)
//!
//! # Quick Start: Three-Way Comparison
//!
//! The recommended workflow is to compare all three distillation approaches:
//!
//! ```rust,ignore
//! use dashflow::optimize::distillation::{
//!     Teacher, ThreeWayDistiller,
//!     PromptOptimizationStudent, OpenAIFineTuneStudent, LocalFineTuneStudent
//! };
//! use dashflow::optimize::signature::make_signature;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Define your task signature
//! let signature = make_signature(
//!     "question -> answer",
//!     "Answer questions concisely and accurately"
//! )?;
//!
//! // 2. Create teacher with expensive model (GPT-4)
//! let teacher_llm = create_gpt4_model(); // Your GPT-4 ChatModel
//! let teacher = Teacher::new(Arc::new(teacher_llm), signature.clone())?;
//!
//! // 3. Create all three student types
//! let student_llm = create_gpt35_model(); // Your GPT-3.5 ChatModel
//!
//! // A. Prompt optimization (fastest, no fine-tuning)
//! let prompt_student = PromptOptimizationStudent::new(
//!     Arc::new(student_llm.clone()),
//!     signature.clone(),
//!     Some(8) // max 8 few-shot examples
//! );
//!
//! // B. OpenAI fine-tuning (best accuracy, medium cost)
//! let openai_student = OpenAIFineTuneStudent::new(
//!     signature.clone(),
//!     "your-openai-api-key".to_string(),
//!     "gpt-3.5-turbo".to_string(),
//!     None // auto-generated suffix
//! );
//!
//! // C. Local fine-tuning (zero inference cost, longest setup)
//! // NOTE: LocalFineTuneStudent is a placeholder. Full implementation requires
//! // external MLX + Ollama setup. The API shown below is illustrative.
//! let local_student = LocalFineTuneStudent::new(
//!     "llama-3-8b".to_string(), // Currently only takes base model name
//! );
//!
//! // 4. Run three-way comparison
//! let distiller = ThreeWayDistiller::new(teacher)
//!     .with_prompt_student(prompt_student)
//!     .with_openai_student(openai_student)
//!     .with_local_student(local_student);
//!
//! // Prepare data
//! let unlabeled_questions = load_questions(); // Vec<YourState>
//! let test_set = load_test_set();            // Vec<YourState>
//!
//! // 5. Run distillation and get comparison report
//! let report = distiller
//!     .distill_and_compare(unlabeled_questions, test_set)
//!     .await?;
//!
//! // 6. Analyze results
//! println!("{}", report.table());
//! println!("\nBest approach: {}", report.best_approach());
//! println!("\nMonthly cost at 10k requests/day:");
//! println!("{}", report.monthly_cost_comparison(10_000));
//! # Ok(())
//! # }
//! ```
//!
//! # Output Example
//!
//! The comparison report shows metrics for all approaches:
//!
//! ```text
//! ╔═══════════════════════════╦══════════╦══════════════╦═══════════╦══════════════════╗
//! ║ Approach                  ║ Accuracy ║ Cost/Request ║ Latency   ║ Setup Time (min) ║
//! ╠═══════════════════════════╬══════════╬══════════════╬═══════════╬══════════════════╣
//! ║ Teacher (GPT-4)           ║  94.0%   ║  $0.004500   ║   850ms   ║        0         ║
//! ║ Prompt Optimization       ║  87.5%   ║  $0.000420   ║   320ms   ║        5         ║
//! ║ OpenAI Fine-tune          ║  91.8%   ║  $0.000420   ║   310ms   ║       20         ║
//! ║ Local Fine-tune (Llama)   ║  89.2%   ║  $0.000000   ║   180ms   ║       60         ║
//! ╚═══════════════════════════╩══════════╩══════════════╩═══════════╩══════════════════╝
//!
//! Best approach: OpenAI Fine-tune (weighted score: 0.89)
//!   - Achieves 97.7% of teacher quality
//!   - 10.7x cost reduction
//!   - Moderate setup time
//!
//! Monthly cost comparison (10,000 requests/day):
//!   Teacher:           $1,350.00/month
//!   Prompt Opt:        $  126.00/month  (saves $1,224/month, payback: 1.3 hours)
//!   OpenAI Fine-tune:  $  126.00/month  (saves $1,224/month, payback: 2.8 hours)
//!   Local Fine-tune:   $    0.00/month  (saves $1,350/month, payback: 8.2 hours)
//! ```
//!
//! # Individual Components
//!
//! You can also use components individually:
//!
//! ## Teacher-Only: Generate Training Data
//!
//! ```rust,ignore
//! use dashflow::optimize::distillation::Teacher;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let teacher = Teacher::new(gpt4_model, signature)?;
//!
//! let unlabeled = vec![/* your states */];
//! let training_data = teacher
//!     .generate_training_data(unlabeled, None)
//!     .await?;
//!
//! println!("Generated {} examples", training_data.len());
//! println!("Total cost: ${:.2}", teacher.total_cost());
//! # Ok(())
//! # }
//! ```
//!
//! ## Prompt Optimization Student
//!
//! ```rust,ignore
//! use dashflow::optimize::distillation::PromptOptimizationStudent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let student = PromptOptimizationStudent::new(gpt35_model, signature, Some(8));
//!
//! let few_shot_examples = student.optimize(training_data).await?;
//! println!("Selected {} few-shot examples", few_shot_examples.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## OpenAI Fine-Tuning Student
//!
//! ```rust,ignore
//! use dashflow::optimize::distillation::OpenAIFineTuneStudent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let student = OpenAIFineTuneStudent::new(
//!     signature,
//!     openai_api_key,
//!     "gpt-3.5-turbo".to_string(),
//!     None
//! );
//!
//! let result = student.fine_tune(training_data).await?;
//! println!("Fine-tuned model: {}", result.model_id);
//! println!("Status: {}", result.status);
//! # Ok(())
//! # }
//! ```
//!
//! # Cost-Benefit Analysis
//!
//! Typical results for a Q&A system with GPT-4 teacher → GPT-3.5 student:
//!
//! | Metric                    | Teacher (GPT-4) | Student (Fine-tuned) | Improvement |
//! |---------------------------|-----------------|----------------------|-------------|
//! | Accuracy                  | 94%             | 92%                  | -2 pp       |
//! | Cost per request          | $0.0045         | $0.00042             | 10.7x       |
//! | Latency                   | 850ms           | 310ms                | 2.7x faster |
//! | Setup cost (one-time)     | $0              | $2.25                | -           |
//!
//! **ROI at 10,000 requests/day:**
//! - Monthly savings: $1,224
//! - Payback period: 2.8 hours
//! - Break-even after: ~170 requests
//!
//! # When to Use Distillation
//!
//! **Good fit:**
//! - High-volume applications (>1,000 requests/day)
//! - Well-defined tasks with consistent input/output structure
//! - Cost is a concern but quality cannot drop significantly
//! - Teacher model is much more expensive than student
//!
//! **Not recommended:**
//! - Low-volume applications (<100 requests/day) - setup cost may not pay back
//! - Highly dynamic tasks requiring latest knowledge - teacher may not generalize
//! - Extreme quality requirements - even small quality drops are unacceptable
//!
//! # Advanced: Core Distillation API
//!
//! For more control, use the lower-level `ModelDistillation` API:
//!
//! ```rust,ignore
//! use dashflow::optimize::distillation::{ModelDistillation, DistillationConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create teacher and student LLM nodes
//! let teacher_node = create_teacher_node();
//! let student_node = create_student_node();
//! let unlabeled_questions = load_unlabeled_data();
//!
//! let config = DistillationConfig::default()
//!     .with_num_synthetic_examples(500);
//!
//! let distiller = ModelDistillation::new()
//!     .with_teacher_node(Arc::new(teacher_node))
//!     .with_config(config);
//!
//! // Generate synthetic data with teacher
//! let synthetic_data = distiller
//!     .generate_synthetic_data(unlabeled_questions)
//!     .await?;
//!
//! // Train student on teacher-generated examples
//! let metric_fn = create_metric_fn();
//! let distilled_node = distiller
//!     .distill_node(Arc::new(student_node), synthetic_data, Arc::new(metric_fn))
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod analysis;
pub mod config;
pub mod distiller;
pub mod evaluation;
pub mod student;
pub mod synthetic;
pub mod teacher;
pub mod three_way;

pub use analysis::{CostAnalysis, DistillationReport, QualityGap, ROIMetrics};
pub use config::{DistillationConfig, DistillationConfigBuilder};
pub use distiller::ModelDistillation;
pub use evaluation::{ComparisonReport, DistillationMetrics};
pub use student::{LocalFineTuneStudent, OpenAIFineTuneStudent, PromptOptimizationStudent};
pub use synthetic::{SyntheticDataConfig, SyntheticDataGenerator};
pub use teacher::Teacher;
pub use three_way::ThreeWayDistiller;

use crate::node::Node;
use std::sync::Arc;

/// Result of a distillation process
#[derive(Clone)]
pub struct DistillationResult<S> {
    /// The distilled (student) node
    pub distilled_node: Arc<dyn Node<S>>,

    /// Synthetic training data generated by teacher (as state objects)
    pub synthetic_data: Vec<S>,

    /// Quality metrics for teacher model
    pub teacher_quality: f64,

    /// Quality metrics for student model (baseline, before distillation)
    pub student_baseline_quality: f64,

    /// Quality metrics for distilled student model
    pub distilled_quality: f64,

    /// Quality gap between teacher and distilled student
    pub quality_gap: f64,

    /// Cost per request for teacher model
    pub teacher_cost_per_request: f64,

    /// Cost per request for student model
    pub student_cost_per_request: f64,

    /// Cost reduction factor (teacher_cost / student_cost)
    pub cost_reduction_factor: f64,

    /// Number of synthetic examples generated
    pub num_synthetic_examples: usize,

    /// Cost to generate synthetic data (one-time)
    pub synthetic_data_cost: f64,

    /// Estimated monthly savings at given request volume
    pub monthly_savings: Option<f64>,

    /// Payback period in hours (time to recoup synthetic data generation cost)
    pub payback_hours: Option<f64>,
}

/// Average days per month used for ROI calculations.
/// This is an approximation; actual months vary from 28-31 days.
const DAYS_PER_MONTH: f64 = 30.0;

impl<S> DistillationResult<S> {
    /// Calculate ROI metrics based on expected request volume.
    ///
    /// This method computes `monthly_savings` and `payback_hours` based on the
    /// cost difference between teacher and student models.
    ///
    /// # Arguments
    ///
    /// * `requests_per_day` - Expected number of requests per day
    ///
    /// # Notes
    ///
    /// - Uses 30 days as an approximation for a month
    /// - `monthly_savings` can be **negative** if the student model costs more
    ///   than the teacher. This can happen with:
    ///   - Inefficient fine-tuning that requires more tokens
    ///   - Higher per-token costs for certain student models
    ///   - A negative value indicates distillation is not cost-effective
    /// - `payback_hours` is only set when `daily_savings > 0`. When savings are
    ///   zero or negative, `payback_hours` remains `None` (no payback possible).
    pub fn calculate_roi(&mut self, requests_per_day: usize) {
        let daily_teacher_cost = self.teacher_cost_per_request * requests_per_day as f64;
        let daily_student_cost = self.student_cost_per_request * requests_per_day as f64;
        let daily_savings = daily_teacher_cost - daily_student_cost;

        self.monthly_savings = Some(daily_savings * DAYS_PER_MONTH);

        // Payback period: time to recoup synthetic data generation cost.
        // Only meaningful when there are positive savings.
        if daily_savings > 0.0 {
            let payback_days = self.synthetic_data_cost / daily_savings;
            self.payback_hours = Some(payback_days * 24.0);
        }
    }

    /// Generate a summary report
    pub fn summary(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Model Distillation Results ===\n\n");

        report.push_str("Quality Metrics:\n");
        report.push_str(&format!(
            "  Teacher:           {:.1}%\n",
            self.teacher_quality * 100.0
        ));
        report.push_str(&format!(
            "  Student Baseline:  {:.1}%\n",
            self.student_baseline_quality * 100.0
        ));
        report.push_str(&format!(
            "  Distilled Student: {:.1}%\n",
            self.distilled_quality * 100.0
        ));
        report.push_str(&format!(
            "  Quality Gap:       {:.1}%\n\n",
            self.quality_gap * 100.0
        ));

        report.push_str("Cost Metrics:\n");
        report.push_str(&format!(
            "  Teacher Cost:      ${:.6}/request\n",
            self.teacher_cost_per_request
        ));
        report.push_str(&format!(
            "  Student Cost:      ${:.6}/request\n",
            self.student_cost_per_request
        ));
        report.push_str(&format!(
            "  Cost Reduction:    {:.1}x\n\n",
            self.cost_reduction_factor
        ));

        report.push_str("Training:\n");
        report.push_str(&format!(
            "  Synthetic Examples: {}\n",
            self.num_synthetic_examples
        ));
        report.push_str(&format!(
            "  Generation Cost:    ${:.2}\n\n",
            self.synthetic_data_cost
        ));

        if let Some(savings) = self.monthly_savings {
            report.push_str("ROI Analysis:\n");
            report.push_str(&format!("  Monthly Savings:   ${:.2}\n", savings));
            if let Some(payback) = self.payback_hours {
                report.push_str(&format!("  Payback Period:    {:.1} hours\n", payback));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MergeableState;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        value: String,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value.clone();
        }
    }

    #[test]
    fn test_distillation_result_roi_calculation() {
        let mut result = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.92,
            student_baseline_quality: 0.78,
            distilled_quality: 0.895,
            quality_gap: 0.025,
            teacher_cost_per_request: 0.0045,
            student_cost_per_request: 0.00042,
            cost_reduction_factor: 10.7,
            num_synthetic_examples: 500,
            synthetic_data_cost: 2.25,
            monthly_savings: None,
            payback_hours: None,
        };

        // Calculate ROI for 10,000 requests/day
        result.calculate_roi(10_000);

        assert!(result.monthly_savings.is_some());
        let savings = result.monthly_savings.unwrap();

        // Expected: (0.0045 - 0.00042) * 10,000 * 30 = 1,224
        assert!((savings - 1224.0).abs() < 1.0);

        // Payback: 2.25 / ((0.0045 - 0.00042) * 10,000) = 0.055 days = 1.32 hours
        assert!(result.payback_hours.is_some());
        let payback = result.payback_hours.unwrap();
        assert!(payback > 1.0 && payback < 2.0);
    }

    #[test]
    fn test_distillation_result_summary() {
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.92,
            student_baseline_quality: 0.78,
            distilled_quality: 0.895,
            quality_gap: 0.025,
            teacher_cost_per_request: 0.0045,
            student_cost_per_request: 0.00042,
            cost_reduction_factor: 10.7,
            num_synthetic_examples: 500,
            synthetic_data_cost: 2.25,
            monthly_savings: Some(1224.0),
            payback_hours: Some(1.32),
        };

        let summary = result.summary();
        assert!(summary.contains("Quality Gap"));
        assert!(summary.contains("92.0%"));
        assert!(summary.contains("89.5%"));
        assert!(summary.contains("10.7x"));
        assert!(summary.contains("1224"));
        assert!(summary.contains("1.3 hours"));
    }

    // Mock node for testing
    struct MockNode;

    #[async_trait]
    impl Node<TestState> for MockNode {
        async fn execute(&self, state: TestState) -> Result<TestState, crate::error::Error> {
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    // ============================================================================
    // Additional comprehensive tests
    // ============================================================================

    #[test]
    fn test_distillation_result_roi_no_savings() {
        // Test when student cost equals teacher cost (no savings)
        let mut result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.90,
            student_baseline_quality: 0.70,
            distilled_quality: 0.85,
            quality_gap: 0.05,
            teacher_cost_per_request: 0.001,
            student_cost_per_request: 0.001, // Same cost
            cost_reduction_factor: 1.0,
            num_synthetic_examples: 100,
            synthetic_data_cost: 1.00,
            monthly_savings: None,
            payback_hours: None,
        };

        result.calculate_roi(1000);

        assert!(result.monthly_savings.is_some());
        let savings = result.monthly_savings.unwrap();
        // No savings when costs are equal
        assert!((savings - 0.0).abs() < 0.01);
        // No payback period when no savings
        assert!(result.payback_hours.is_none());
    }

    #[test]
    fn test_distillation_result_roi_negative_savings() {
        // Test when student costs MORE than teacher (negative savings).
        // This can happen with inefficient fine-tuning or higher per-token costs.
        let mut result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.90,
            student_baseline_quality: 0.70,
            distilled_quality: 0.85,
            quality_gap: 0.05,
            teacher_cost_per_request: 0.001,
            student_cost_per_request: 0.002, // Student costs MORE
            cost_reduction_factor: 0.5,      // Actually a cost INCREASE
            num_synthetic_examples: 100,
            synthetic_data_cost: 1.00,
            monthly_savings: None,
            payback_hours: None,
        };

        result.calculate_roi(1000);

        assert!(result.monthly_savings.is_some());
        let savings = result.monthly_savings.unwrap();
        // Negative savings: (0.001 - 0.002) * 1000 * 30 = -30.0
        assert!(savings < 0.0, "Expected negative savings");
        assert!((savings - (-30.0)).abs() < 0.1);
        // No payback period when savings are negative (distillation not cost-effective)
        assert!(result.payback_hours.is_none());
    }

    #[test]
    fn test_distillation_result_roi_low_volume() {
        // Test with very low request volume
        let mut result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.95,
            student_baseline_quality: 0.80,
            distilled_quality: 0.92,
            quality_gap: 0.03,
            teacher_cost_per_request: 0.01,
            student_cost_per_request: 0.001,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 200,
            synthetic_data_cost: 5.00,
            monthly_savings: None,
            payback_hours: None,
        };

        result.calculate_roi(10); // Only 10 requests/day

        assert!(result.monthly_savings.is_some());
        let savings = result.monthly_savings.unwrap();
        // Expected: (0.01 - 0.001) * 10 * 30 = 2.7
        assert!((savings - 2.7).abs() < 0.1);

        assert!(result.payback_hours.is_some());
        let payback = result.payback_hours.unwrap();
        // Payback: 5.00 / ((0.01 - 0.001) * 10) = 5.00 / 0.09 = 55.55 days = 1333 hours
        assert!(payback > 1000.0);
    }

    #[test]
    fn test_distillation_result_roi_high_volume() {
        // Test with very high request volume
        let mut result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.94,
            student_baseline_quality: 0.75,
            distilled_quality: 0.90,
            quality_gap: 0.04,
            teacher_cost_per_request: 0.005,
            student_cost_per_request: 0.0005,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 1000,
            synthetic_data_cost: 10.00,
            monthly_savings: None,
            payback_hours: None,
        };

        result.calculate_roi(100_000); // 100k requests/day

        assert!(result.monthly_savings.is_some());
        let savings = result.monthly_savings.unwrap();
        // Expected: (0.005 - 0.0005) * 100,000 * 30 = 13,500
        assert!((savings - 13500.0).abs() < 1.0);

        assert!(result.payback_hours.is_some());
        let payback = result.payback_hours.unwrap();
        // Very fast payback with high volume
        assert!(payback < 1.0); // Less than 1 hour
    }

    #[test]
    fn test_distillation_result_with_synthetic_data() {
        let test_data = vec![
            TestState {
                value: "example1".to_string(),
            },
            TestState {
                value: "example2".to_string(),
            },
            TestState {
                value: "example3".to_string(),
            },
        ];

        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: test_data.clone(),
            teacher_quality: 0.90,
            student_baseline_quality: 0.75,
            distilled_quality: 0.87,
            quality_gap: 0.03,
            teacher_cost_per_request: 0.003,
            student_cost_per_request: 0.0003,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 3,
            synthetic_data_cost: 0.009,
            monthly_savings: None,
            payback_hours: None,
        };

        assert_eq!(result.synthetic_data.len(), 3);
        assert_eq!(result.num_synthetic_examples, 3);
        assert_eq!(result.synthetic_data[0].value, "example1");
        assert_eq!(result.synthetic_data[2].value, "example3");
    }

    #[test]
    fn test_distillation_result_summary_without_roi() {
        // Test summary when ROI has not been calculated
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.88,
            student_baseline_quality: 0.72,
            distilled_quality: 0.84,
            quality_gap: 0.04,
            teacher_cost_per_request: 0.002,
            student_cost_per_request: 0.0002,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 250,
            synthetic_data_cost: 0.50,
            monthly_savings: None,
            payback_hours: None,
        };

        let summary = result.summary();
        // Should still contain quality metrics
        assert!(summary.contains("Quality Metrics"));
        assert!(summary.contains("88.0%"));
        assert!(summary.contains("84.0%"));
        // Should not contain ROI section
        assert!(!summary.contains("ROI Analysis"));
    }

    #[test]
    fn test_distillation_result_summary_format() {
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.90,
            student_baseline_quality: 0.70,
            distilled_quality: 0.85,
            quality_gap: 0.05,
            teacher_cost_per_request: 0.004,
            student_cost_per_request: 0.0004,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 100,
            synthetic_data_cost: 0.40,
            monthly_savings: Some(1080.0),
            payback_hours: Some(0.1),
        };

        let summary = result.summary();

        // Verify section headers
        assert!(summary.contains("=== Model Distillation Results ==="));
        assert!(summary.contains("Quality Metrics:"));
        assert!(summary.contains("Cost Metrics:"));
        assert!(summary.contains("Training:"));
        assert!(summary.contains("ROI Analysis:"));

        // Verify content formatting
        assert!(summary.contains("Teacher:"));
        assert!(summary.contains("Student Baseline:"));
        assert!(summary.contains("Distilled Student:"));
        assert!(summary.contains("Synthetic Examples: 100"));
    }

    #[test]
    fn test_distillation_result_edge_cases() {
        // Test with zero values
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.0,
            student_baseline_quality: 0.0,
            distilled_quality: 0.0,
            quality_gap: 0.0,
            teacher_cost_per_request: 0.0,
            student_cost_per_request: 0.0,
            cost_reduction_factor: 0.0,
            num_synthetic_examples: 0,
            synthetic_data_cost: 0.0,
            monthly_savings: None,
            payback_hours: None,
        };

        let summary = result.summary();
        assert!(summary.contains("0.0%"));
        assert!(summary.contains("Synthetic Examples: 0"));
    }

    #[test]
    fn test_distillation_result_clone() {
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![TestState {
                value: "test".to_string(),
            }],
            teacher_quality: 0.90,
            student_baseline_quality: 0.70,
            distilled_quality: 0.85,
            quality_gap: 0.05,
            teacher_cost_per_request: 0.001,
            student_cost_per_request: 0.0001,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 1,
            synthetic_data_cost: 0.001,
            monthly_savings: Some(100.0),
            payback_hours: Some(0.5),
        };

        let cloned = result.clone();

        assert_eq!(cloned.teacher_quality, result.teacher_quality);
        assert_eq!(cloned.distilled_quality, result.distilled_quality);
        assert_eq!(cloned.cost_reduction_factor, result.cost_reduction_factor);
        assert_eq!(cloned.synthetic_data.len(), result.synthetic_data.len());
        assert_eq!(cloned.monthly_savings, result.monthly_savings);
        assert_eq!(cloned.payback_hours, result.payback_hours);
    }

    #[test]
    fn test_distillation_result_quality_gap_calculation() {
        // Test that quality gap is correctly represented
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.95,
            student_baseline_quality: 0.75,
            distilled_quality: 0.92,
            quality_gap: 0.03, // 95% - 92% = 3%
            teacher_cost_per_request: 0.01,
            student_cost_per_request: 0.001,
            cost_reduction_factor: 10.0,
            num_synthetic_examples: 500,
            synthetic_data_cost: 5.0,
            monthly_savings: None,
            payback_hours: None,
        };

        // Quality gap should be the difference between teacher and distilled quality
        assert!((result.quality_gap - 0.03).abs() < 0.001);

        let summary = result.summary();
        assert!(summary.contains("3.0%")); // Quality gap formatted
    }

    #[test]
    fn test_distillation_result_cost_reduction() {
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.90,
            student_baseline_quality: 0.70,
            distilled_quality: 0.85,
            quality_gap: 0.05,
            teacher_cost_per_request: 0.0100,  // $0.01
            student_cost_per_request: 0.00025, // $0.00025
            cost_reduction_factor: 40.0,       // 40x reduction
            num_synthetic_examples: 1000,
            synthetic_data_cost: 10.0,
            monthly_savings: None,
            payback_hours: None,
        };

        // Verify cost reduction factor calculation: 0.01 / 0.00025 = 40
        assert!((result.cost_reduction_factor - 40.0).abs() < 0.1);

        let summary = result.summary();
        assert!(summary.contains("40.0x"));
    }

    #[test]
    fn test_distillation_result_large_scale() {
        // Test with large scale values
        let result: DistillationResult<TestState> = DistillationResult {
            distilled_node: Arc::new(MockNode),
            synthetic_data: vec![],
            teacher_quality: 0.95,
            student_baseline_quality: 0.80,
            distilled_quality: 0.93,
            quality_gap: 0.02,
            teacher_cost_per_request: 0.05,     // $0.05 per request
            student_cost_per_request: 0.0005,   // $0.0005 per request
            cost_reduction_factor: 100.0,       // 100x reduction
            num_synthetic_examples: 100_000,    // 100k examples
            synthetic_data_cost: 5000.0,        // $5000 for training data
            monthly_savings: Some(1_400_850.0), // ~$1.4M monthly
            payback_hours: Some(2.4),           // Quick payback
        };

        let summary = result.summary();
        assert!(summary.contains("100.0x"));
        assert!(summary.contains("100000"));
        assert!(summary.contains("5000"));
    }
}
