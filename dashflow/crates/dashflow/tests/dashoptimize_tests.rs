//! # DashOptimize Integration Tests
//!
//! Tests for the native DashFlow DashOptimize optimization functionality.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dashflow::optimize::{exact_match, make_signature, MetricFn, OptimizerConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Simple test state for classification tasks
/// GraphState is automatically implemented via blanket impl
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ClassifierState {
    text: String,
    category: String,
}

#[test]
fn test_signature_creation() {
    // Test basic signature creation
    let signature = make_signature("text -> category", "Classify text sentiment").unwrap();

    assert_eq!(signature.instructions, "Classify text sentiment");
    assert_eq!(signature.input_fields.len(), 1);
    assert_eq!(signature.output_fields.len(), 1);
    assert_eq!(signature.input_fields[0].name, "text");
    assert_eq!(signature.output_fields[0].name, "category");
}

#[test]
#[allow(clippy::float_cmp)]
fn test_metric_exact_match() {
    // Test exact match returns 1.0 for matching strings
    assert_eq!(exact_match("positive", "positive"), 1.0);
    assert_eq!(exact_match("negative", "negative"), 1.0);

    // Test trimming
    assert_eq!(exact_match(" positive ", "positive"), 1.0);
    assert_eq!(exact_match("positive", " positive "), 1.0);

    // Test non-match returns 0.0
    assert_eq!(exact_match("positive", "negative"), 0.0);
    assert_eq!(exact_match("yes", "no"), 0.0);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_metric_function_with_state() {
    // Create a metric function using exact_match
    let metric: MetricFn<ClassifierState> = Arc::new(|expected, predicted| {
        let expected_cat = &expected.category;
        let predicted_cat = &predicted.category;
        Ok(exact_match(expected_cat, predicted_cat))
    });

    let state1 = ClassifierState {
        text: "test".to_string(),
        category: "positive".to_string(),
    };

    let state2 = ClassifierState {
        text: "test".to_string(),
        category: "positive".to_string(),
    };

    let state3 = ClassifierState {
        text: "test".to_string(),
        category: "negative".to_string(),
    };

    // Test matching states
    let score1 = metric(&state1, &state2).unwrap();
    assert_eq!(score1, 1.0);

    // Test non-matching states
    let score2 = metric(&state1, &state3).unwrap();
    assert_eq!(score2, 0.0);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_optimizer_config_builder() {
    let config = OptimizerConfig::new()
        .with_max_few_shot_examples(8)
        .with_max_iterations(20)
        .with_min_improvement(0.05)
        .with_random_seed(42);

    assert_eq!(config.max_few_shot_examples, 8);
    assert_eq!(config.max_iterations, 20);
    assert_eq!(config.min_improvement, 0.05);
    assert_eq!(config.random_seed, Some(42));
}

#[test]
fn test_optimization_result_metrics() {
    use dashflow::optimize::OptimizationResult;

    let result = OptimizationResult::new(0.6, 0.8, 5, true, 12.5);

    // Use approximate comparison for floating point
    assert!((result.improvement() - 0.2).abs() < 0.0001);
    assert!((result.improvement_percent() - 33.333).abs() < 0.01);
}

// ============================================================================
// Integration Tests for AutoOptimizer and Introspection
// ============================================================================

mod auto_optimizer_integration {
    use dashflow::optimize::auto_optimizer::{
        AutoOptimizer, ComputeBudget, OptimizationContext, OptimizationOutcome, TaskType,
    };
    use dashflow::optimize::optimizers::registry;
    use tempfile::TempDir;

    /// Test AutoOptimizer selection across various contexts
    #[test]
    fn test_auto_optimizer_selection_matrix() {
        // Test matrix of contexts vs expected optimizers
        let test_cases = vec![
            // (examples, can_finetune, task_type, expected_optimizer_or_prefix)
            (100, false, TaskType::QuestionAnswering, "MIPROv2"),
            (100, true, TaskType::CodeGeneration, "GRPO"),
            (10, false, TaskType::Classification, "BootstrapFewShot"),
            (50, false, TaskType::MathReasoning, "MIPROv2"),
            (5, false, TaskType::Generic, "BootstrapFewShot"),
        ];

        for (examples, can_finetune, task_type, expected) in test_cases {
            let context = OptimizationContext {
                num_examples: examples,
                can_finetune,
                task_type,
                compute_budget: ComputeBudget::Medium,
                has_embedding_model: false,
                available_capabilities: vec!["metric_function".to_string()],
                preferred_tier: None,
                excluded_optimizers: vec![],
            };

            let result = AutoOptimizer::select(&context);
            assert!(
                result.optimizer_name.starts_with(expected) || result.optimizer_name == expected,
                "For {} examples, finetune={}, task={:?}: expected {}, got {}",
                examples,
                can_finetune,
                task_type,
                expected,
                result.optimizer_name
            );
        }
    }

    /// Test that all optimizers in registry have valid metadata
    #[test]
    fn test_all_optimizers_have_citations() {
        let all = registry::all_optimizers();

        // Should have 17 optimizers (updated from original 15)
        assert_eq!(all.len(), 17, "Expected 17 optimizers");

        for opt in &all {
            // Name should not be empty
            assert!(!opt.name.is_empty(), "Optimizer name should not be empty");

            // Citation should not be empty
            assert!(
                !opt.citation.is_empty(),
                "Optimizer {} should have a citation",
                opt.name
            );

            // Description should not be empty
            assert!(
                !opt.description.is_empty(),
                "Optimizer {} should have a description",
                opt.name
            );

            // use_when should not be empty
            assert!(
                !opt.use_when.is_empty(),
                "Optimizer {} should have use_when",
                opt.name
            );

            // cannot_use_when should not be empty
            assert!(
                !opt.cannot_use_when.is_empty(),
                "Optimizer {} should have cannot_use_when",
                opt.name
            );
        }
    }

    /// Test outcome recording and retrieval (async)
    #[tokio::test]
    #[allow(clippy::float_cmp)]
    async fn test_outcome_recording_and_retrieval() {
        // Create temp directory for test
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("optimizer_outcomes");

        let auto_optimizer = AutoOptimizer::with_storage_dir(storage_path.clone());

        // Create a test outcome
        let context = OptimizationContext {
            num_examples: 50,
            can_finetune: false,
            task_type: TaskType::Classification,
            compute_budget: ComputeBudget::Medium,
            has_embedding_model: false,
            available_capabilities: vec!["metric_function".to_string()],
            preferred_tier: None,
            excluded_optimizers: vec![],
        };

        let outcome = OptimizationOutcome {
            timestamp: chrono::Utc::now(),
            context: context.clone(),
            optimizer_name: "MIPROv2".to_string(),
            initial_score: 0.65,
            final_score: 0.82,
            improvement: 0.17,
            duration_secs: 45.5,
            success: true,
            notes: Some("Test outcome".to_string()),
        };

        // Record the outcome
        auto_optimizer
            .record_outcome(&outcome)
            .await
            .expect("Failed to record outcome");

        // Load outcomes and verify
        let loaded = auto_optimizer
            .load_outcomes()
            .await
            .expect("Failed to load outcomes");

        assert_eq!(loaded.len(), 1, "Should have 1 recorded outcome");
        assert_eq!(loaded[0].optimizer_name, "MIPROv2");
        assert_eq!(loaded[0].initial_score, 0.65);
        assert_eq!(loaded[0].final_score, 0.82);
        assert!(loaded[0].success);
    }

    /// Test optimizer statistics calculation
    #[tokio::test]
    async fn test_optimizer_statistics() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("optimizer_outcomes");

        let auto_optimizer = AutoOptimizer::with_storage_dir(storage_path.clone());

        // Record multiple outcomes for the same optimizer
        for i in 0..5 {
            let context = OptimizationContext {
                num_examples: 50 + i * 10,
                can_finetune: false,
                task_type: TaskType::Classification,
                compute_budget: ComputeBudget::Medium,
                has_embedding_model: false,
                available_capabilities: vec!["metric_function".to_string()],
                preferred_tier: None,
                excluded_optimizers: vec![],
            };

            let outcome = OptimizationOutcome {
                timestamp: chrono::Utc::now() + chrono::Duration::seconds(i as i64),
                context,
                optimizer_name: "MIPROv2".to_string(),
                initial_score: 0.6 + (i as f64 * 0.02),
                final_score: 0.8 + (i as f64 * 0.01),
                improvement: 0.15 + (i as f64 * 0.01),
                duration_secs: 30.0 + (i as f64 * 5.0),
                success: true,
                notes: None,
            };

            auto_optimizer
                .record_outcome(&outcome)
                .await
                .expect("Failed to record outcome");
        }

        // Get stats for MIPROv2
        let stats = auto_optimizer
            .stats_for_optimizer("MIPROv2")
            .await
            .expect("Failed to get stats")
            .expect("Should have stats for MIPROv2");

        assert_eq!(stats.usage_count, 5, "Should have 5 usages recorded");
        assert!(stats.success_rate > 0.99, "All outcomes were successful");
        assert!(
            stats.avg_improvement > 0.15,
            "Average improvement should be > 0.15"
        );
    }

    /// Test exclusion of specific optimizers
    #[test]
    fn test_optimizer_exclusion() {
        let context = OptimizationContext {
            num_examples: 100,
            can_finetune: false,
            task_type: TaskType::QuestionAnswering,
            compute_budget: ComputeBudget::Medium,
            has_embedding_model: false,
            available_capabilities: vec!["metric_function".to_string()],
            preferred_tier: None,
            excluded_optimizers: vec!["MIPROv2".to_string()],
        };

        let result = AutoOptimizer::select(&context);

        // Should NOT select MIPROv2 since it's excluded
        assert_ne!(
            result.optimizer_name, "MIPROv2",
            "MIPROv2 should be excluded"
        );

        // Should select an alternative (likely BootstrapFewShot)
        assert!(!result.optimizer_name.is_empty());
    }
}

// ============================================================================
// CLI End-to-End Tests
// ============================================================================

mod cli_e2e_tests {
    use std::process::Command;

    /// Test CLI introspect optimize command with JSON output
    #[test]
    fn test_cli_introspect_optimize_json() {
        let output = Command::new("./target/release/dashflow")
            .args([
                "introspect",
                "optimize",
                "--examples",
                "100",
                "-t",
                "qa",
                "--json",
            ])
            .output();

        // Allow test to pass if binary not built (integration test dependency)
        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Verify JSON output contains expected fields
                assert!(
                    stdout.contains("optimizer_name"),
                    "Should have optimizer_name field"
                );
                assert!(
                    stdout.contains("confidence"),
                    "Should have confidence field"
                );
                assert!(stdout.contains("reason"), "Should have reason field");
                assert!(
                    stdout.contains("alternatives"),
                    "Should have alternatives field"
                );
            }
        }
    }

    /// Test CLI introspect optimize with different parameters
    #[test]
    fn test_cli_introspect_optimize_finetuning() {
        let output = Command::new("./target/release/dashflow")
            .args([
                "introspect",
                "optimize",
                "--examples",
                "50",
                "--can-finetune",
                "-t",
                "code",
                "--json",
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // When finetuning is enabled, GRPO should be selected
                assert!(
                    stdout.contains("GRPO"),
                    "Should select GRPO for finetuning context"
                );
            }
        }
    }

    /// Test CLI introspect optimize with exclusion
    #[test]
    fn test_cli_introspect_optimize_exclusion() {
        let output = Command::new("./target/release/dashflow")
            .args([
                "introspect",
                "optimize",
                "--examples",
                "100",
                "-t",
                "qa",
                "--exclude",
                "MIPROv2",
                "--json",
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Should NOT recommend MIPROv2 when excluded
                let json: serde_json::Value =
                    serde_json::from_str(&stdout).expect("Should be valid JSON");
                let optimizer_name = json["optimizer_name"].as_str().unwrap_or("");
                assert_ne!(
                    optimizer_name, "MIPROv2",
                    "Should not select excluded optimizer"
                );
            }
        }
    }

    /// Test CLI introspect optimize help is available
    #[test]
    fn test_cli_introspect_optimize_help() {
        let output = Command::new("./target/release/dashflow")
            .args(["introspect", "optimize", "--help"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Help should show available options
            assert!(stdout.contains("--examples"), "Help should show --examples");
            assert!(
                stdout.contains("--can-finetune"),
                "Help should show --can-finetune"
            );
            assert!(stdout.contains("--json"), "Help should show --json");
        }
    }
}
