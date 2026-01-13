// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Evaluation and comparison reporting module.
//!
//! Provides metrics and reporting for comparing distillation approaches.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Metrics for a single distillation approach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationMetrics {
    /// Accuracy on test set (0.0 to 1.0)
    pub accuracy: f64,

    /// Cost per inference request in USD
    pub cost_per_request: f64,

    /// Average latency per request in milliseconds
    pub latency_ms: f64,

    /// One-time setup time in minutes (training/optimization)
    pub setup_time_minutes: f64,
}

impl DistillationMetrics {
    /// Calculates monthly cost for given request volume.
    pub fn monthly_cost(&self, requests_per_day: u64) -> f64 {
        self.cost_per_request * requests_per_day as f64 * 30.0
    }

    /// Calculates cost reduction percentage compared to another approach.
    pub fn cost_reduction_vs(&self, other: &DistillationMetrics) -> f64 {
        if other.cost_per_request == 0.0 {
            return 0.0;
        }
        ((other.cost_per_request - self.cost_per_request) / other.cost_per_request) * 100.0
    }

    /// Calculates accuracy gain percentage compared to another approach.
    pub fn accuracy_gain_vs(&self, other: &DistillationMetrics) -> f64 {
        if other.accuracy == 0.0 {
            return 0.0;
        }
        ((self.accuracy - other.accuracy) / other.accuracy) * 100.0
    }
}

/// Comprehensive comparison report for all distillation approaches.
///
/// Contains metrics from each distillation method (teacher, fine-tuned, prompt-optimized)
/// along with dataset statistics for context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    /// Baseline metrics from the teacher model (e.g., GPT-4)
    pub teacher_metrics: DistillationMetrics,
    /// Metrics from OpenAI fine-tuning (if performed)
    pub openai_metrics: Option<DistillationMetrics>,
    /// Metrics from local model fine-tuning (if performed)
    pub local_metrics: Option<DistillationMetrics>,
    /// Metrics from prompt optimization approach (if performed)
    pub prompt_metrics: Option<DistillationMetrics>,
    /// Number of examples used for training/optimization
    pub num_training_examples: usize,
    /// Number of examples used for evaluation
    pub num_test_examples: usize,
}

impl ComparisonReport {
    /// Identifies the best approach based on weighted score.
    ///
    /// Score = accuracy * 0.5 + (1 - normalized_cost) * 0.3 + (1 - normalized_latency) * 0.2
    pub fn best_approach(&self) -> &'static str {
        #[allow(unused_assignments)] // Initialized, then compared in subsequent if blocks
        let mut best_score = 0.0;
        let mut best_name = "teacher";

        // Calculate normalized values for scoring
        let max_cost = [
            Some(self.teacher_metrics.cost_per_request),
            self.openai_metrics.as_ref().map(|m| m.cost_per_request),
            self.local_metrics.as_ref().map(|m| m.cost_per_request),
            self.prompt_metrics.as_ref().map(|m| m.cost_per_request),
        ]
        .into_iter()
        .flatten()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(1.0);

        let max_latency = [
            Some(self.teacher_metrics.latency_ms),
            self.openai_metrics.as_ref().map(|m| m.latency_ms),
            self.local_metrics.as_ref().map(|m| m.latency_ms),
            self.prompt_metrics.as_ref().map(|m| m.latency_ms),
        ]
        .into_iter()
        .flatten()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(1000.0);

        // Score teacher
        best_score = self.teacher_metrics.accuracy * 0.5
            + (1.0 - self.teacher_metrics.cost_per_request / max_cost) * 0.3
            + (1.0 - self.teacher_metrics.latency_ms / max_latency) * 0.2;

        // Score OpenAI
        if let Some(ref metrics) = self.openai_metrics {
            let score = metrics.accuracy * 0.5
                + (1.0 - metrics.cost_per_request / max_cost) * 0.3
                + (1.0 - metrics.latency_ms / max_latency) * 0.2;
            if score > best_score {
                best_score = score;
                best_name = "openai_finetune";
            }
        }

        // Score Local
        if let Some(ref metrics) = self.local_metrics {
            let score = metrics.accuracy * 0.5
                + (1.0 - metrics.cost_per_request / max_cost) * 0.3
                + (1.0 - metrics.latency_ms / max_latency) * 0.2;
            if score > best_score {
                best_score = score;
                best_name = "local_finetune";
            }
        }

        // Score Prompt
        if let Some(ref metrics) = self.prompt_metrics {
            let score = metrics.accuracy * 0.5
                + (1.0 - metrics.cost_per_request / max_cost) * 0.3
                + (1.0 - metrics.latency_ms / max_latency) * 0.2;
            if score > best_score {
                best_name = "prompt_optimization";
            }
        }

        best_name
    }

    /// Generates a formatted comparison table.
    pub fn table(&self) -> String {
        let mut output = String::new();

        output.push_str(
            "┌──────────────────────┬──────────┬──────────────┬─────────────┬─────────────┐\n",
        );
        output.push_str(
            "│ Approach             │ Accuracy │ Cost/Request │ Latency (ms)│ Setup Time  │\n",
        );
        output.push_str(
            "├──────────────────────┼──────────┼──────────────┼─────────────┼─────────────┤\n",
        );

        // Teacher
        output.push_str(&format!(
            "│ Teacher (GPT-4)      │  {:5.1}%  │   ${:.5}    │   {:6.0}    │     N/A     │\n",
            self.teacher_metrics.accuracy * 100.0,
            self.teacher_metrics.cost_per_request,
            self.teacher_metrics.latency_ms,
        ));

        // OpenAI
        if let Some(ref metrics) = self.openai_metrics {
            output.push_str(&format!(
                "│ OpenAI Fine-tune     │  {:5.1}%  │   ${:.5}    │   {:6.0}    │   {:3.0} min    │\n",
                metrics.accuracy * 100.0,
                metrics.cost_per_request,
                metrics.latency_ms,
                metrics.setup_time_minutes,
            ));
        }

        // Local
        if let Some(ref metrics) = self.local_metrics {
            output.push_str(&format!(
                "│ Local Fine-tune      │  {:5.1}%  │   ${:.5}    │   {:6.0}    │   {:3.0} min    │\n",
                metrics.accuracy * 100.0,
                metrics.cost_per_request,
                metrics.latency_ms,
                metrics.setup_time_minutes,
            ));
        }

        // Prompt optimization
        if let Some(ref metrics) = self.prompt_metrics {
            output.push_str(&format!(
                "│ Prompt Optimization  │  {:5.1}%  │   ${:.5}    │   {:6.0}    │   {:3.0} min    │\n",
                metrics.accuracy * 100.0,
                metrics.cost_per_request,
                metrics.latency_ms,
                metrics.setup_time_minutes,
            ));
        }

        output.push_str(
            "└──────────────────────┴──────────┴──────────────┴─────────────┴─────────────┘\n",
        );

        output
    }

    /// Generates monthly cost comparison.
    pub fn monthly_cost_comparison(&self, requests_per_day: u64) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "\n=== Monthly Cost ({} requests/day) ===\n",
            requests_per_day
        ));
        output.push_str(&format!(
            "Teacher:            ${:.2}\n",
            self.teacher_metrics.monthly_cost(requests_per_day)
        ));

        if let Some(ref metrics) = self.openai_metrics {
            output.push_str(&format!(
                "OpenAI Fine-tune:   ${:.2} ({:.1}% reduction)\n",
                metrics.monthly_cost(requests_per_day),
                metrics.cost_reduction_vs(&self.teacher_metrics)
            ));
        }

        if let Some(ref metrics) = self.local_metrics {
            output.push_str(&format!(
                "Local Fine-tune:    ${:.2} ({:.1}% reduction)\n",
                metrics.monthly_cost(requests_per_day),
                metrics.cost_reduction_vs(&self.teacher_metrics)
            ));
        }

        if let Some(ref metrics) = self.prompt_metrics {
            output.push_str(&format!(
                "Prompt Opt:         ${:.2} ({:.1}% reduction)\n",
                metrics.monthly_cost(requests_per_day),
                metrics.cost_reduction_vs(&self.teacher_metrics)
            ));
        }

        output
    }
}

impl fmt::Display for ComparisonReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n=== Distillation Comparison Report ===\n\n")?;
        writeln!(f, "Training Examples: {}", self.num_training_examples)?;
        write!(f, "Test Examples: {}\n\n", self.num_test_examples)?;
        write!(f, "{}", self.table())?;
        write!(f, "{}", self.monthly_cost_comparison(10_000))?;
        write!(f, "\nBest Approach: {}\n", self.best_approach())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_monthly_cost() {
        let metrics = DistillationMetrics {
            accuracy: 0.9,
            cost_per_request: 0.001,
            latency_ms: 500.0,
            setup_time_minutes: 30.0,
        };

        let monthly = metrics.monthly_cost(10_000);
        assert_eq!(monthly, 300.0); // 0.001 * 10000 * 30
    }

    #[test]
    fn test_cost_reduction() {
        let expensive = DistillationMetrics {
            accuracy: 0.95,
            cost_per_request: 0.004,
            latency_ms: 800.0,
            setup_time_minutes: 0.0,
        };

        let cheap = DistillationMetrics {
            accuracy: 0.90,
            cost_per_request: 0.001,
            latency_ms: 500.0,
            setup_time_minutes: 20.0,
        };

        let reduction = cheap.cost_reduction_vs(&expensive);
        assert_eq!(reduction, 75.0); // (0.004 - 0.001) / 0.004 * 100
    }

    #[test]
    fn test_report_creation() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: Some(DistillationMetrics {
                accuracy: 0.92,
                cost_per_request: 0.0012,
                latency_ms: 600.0,
                setup_time_minutes: 20.0,
            }),
            local_metrics: None,
            prompt_metrics: None,
            num_training_examples: 150,
            num_test_examples: 50,
        };

        let table = report.table();
        assert!(table.contains("Teacher"));
        assert!(table.contains("OpenAI"));
    }

    #[test]
    fn test_best_approach_scoring() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: Some(DistillationMetrics {
                accuracy: 0.92,
                cost_per_request: 0.0012,
                latency_ms: 600.0,
                setup_time_minutes: 20.0,
            }),
            local_metrics: None,
            prompt_metrics: Some(DistillationMetrics {
                accuracy: 0.88,
                cost_per_request: 0.0015,
                latency_ms: 700.0,
                setup_time_minutes: 10.0,
            }),
            num_training_examples: 150,
            num_test_examples: 50,
        };

        // OpenAI should win: good accuracy, much lower cost, faster latency
        let best = report.best_approach();
        assert_eq!(best, "openai_finetune");
    }

    #[test]
    fn test_accuracy_gain() {
        let baseline = DistillationMetrics {
            accuracy: 0.80,
            cost_per_request: 0.002,
            latency_ms: 500.0,
            setup_time_minutes: 0.0,
        };

        let improved = DistillationMetrics {
            accuracy: 0.90,
            cost_per_request: 0.002,
            latency_ms: 500.0,
            setup_time_minutes: 30.0,
        };

        let gain = improved.accuracy_gain_vs(&baseline);
        assert!((gain - 12.5).abs() < 0.1); // (0.90 - 0.80) / 0.80 * 100 = 12.5%
    }

    #[test]
    fn test_accuracy_gain_with_zero_baseline() {
        let baseline = DistillationMetrics {
            accuracy: 0.0,
            cost_per_request: 0.001,
            latency_ms: 500.0,
            setup_time_minutes: 0.0,
        };

        let improved = DistillationMetrics {
            accuracy: 0.90,
            cost_per_request: 0.002,
            latency_ms: 500.0,
            setup_time_minutes: 30.0,
        };

        // Should return 0.0 to avoid division by zero
        let gain = improved.accuracy_gain_vs(&baseline);
        assert_eq!(gain, 0.0);
    }

    #[test]
    fn test_cost_reduction_with_zero_baseline() {
        let expensive = DistillationMetrics {
            accuracy: 0.95,
            cost_per_request: 0.0,
            latency_ms: 800.0,
            setup_time_minutes: 0.0,
        };

        let cheap = DistillationMetrics {
            accuracy: 0.90,
            cost_per_request: 0.001,
            latency_ms: 500.0,
            setup_time_minutes: 20.0,
        };

        // Should return 0.0 to avoid division by zero
        let reduction = cheap.cost_reduction_vs(&expensive);
        assert_eq!(reduction, 0.0);
    }

    #[test]
    fn test_best_approach_local_wins() {
        // Scenario where local fine-tuning wins due to zero cost
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: None,
            local_metrics: Some(DistillationMetrics {
                accuracy: 0.93,
                cost_per_request: 0.0, // Free after setup
                latency_ms: 300.0,     // Very fast local
                setup_time_minutes: 60.0,
            }),
            prompt_metrics: None,
            num_training_examples: 150,
            num_test_examples: 50,
        };

        let best = report.best_approach();
        assert_eq!(best, "local_finetune");
    }

    #[test]
    fn test_best_approach_teacher_only() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: None,
            local_metrics: None,
            prompt_metrics: None,
            num_training_examples: 0,
            num_test_examples: 50,
        };

        let best = report.best_approach();
        assert_eq!(best, "teacher");
    }

    #[test]
    fn test_report_display() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: Some(DistillationMetrics {
                accuracy: 0.92,
                cost_per_request: 0.0012,
                latency_ms: 600.0,
                setup_time_minutes: 20.0,
            }),
            local_metrics: None,
            prompt_metrics: None,
            num_training_examples: 150,
            num_test_examples: 50,
        };

        let display = format!("{}", report);
        assert!(display.contains("Distillation Comparison Report"));
        assert!(display.contains("Training Examples: 150"));
        assert!(display.contains("Test Examples: 50"));
        assert!(display.contains("Teacher"));
        assert!(display.contains("OpenAI"));
    }

    #[test]
    fn test_monthly_cost_comparison_output() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.004,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: Some(DistillationMetrics {
                accuracy: 0.92,
                cost_per_request: 0.001,
                latency_ms: 600.0,
                setup_time_minutes: 20.0,
            }),
            local_metrics: None,
            prompt_metrics: None,
            num_training_examples: 150,
            num_test_examples: 50,
        };

        let cost_comparison = report.monthly_cost_comparison(1000);
        assert!(cost_comparison.contains("Monthly Cost"));
        assert!(cost_comparison.contains("1000 requests/day"));
        assert!(cost_comparison.contains("Teacher"));
        assert!(cost_comparison.contains("reduction"));
    }

    #[test]
    fn test_metrics_serialization() {
        let metrics = DistillationMetrics {
            accuracy: 0.92,
            cost_per_request: 0.0012,
            latency_ms: 600.0,
            setup_time_minutes: 20.0,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: DistillationMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.accuracy, 0.92);
        assert_eq!(deserialized.cost_per_request, 0.0012);
        assert_eq!(deserialized.latency_ms, 600.0);
        assert_eq!(deserialized.setup_time_minutes, 20.0);
    }

    #[test]
    fn test_comparison_report_serialization() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: None,
            local_metrics: None,
            prompt_metrics: None,
            num_training_examples: 100,
            num_test_examples: 20,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: ComparisonReport = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.teacher_metrics.accuracy, 0.95);
        assert_eq!(deserialized.num_training_examples, 100);
        assert!(deserialized.openai_metrics.is_none());
    }

    #[test]
    fn test_table_with_all_approaches() {
        let report = ComparisonReport {
            teacher_metrics: DistillationMetrics {
                accuracy: 0.95,
                cost_per_request: 0.0045,
                latency_ms: 850.0,
                setup_time_minutes: 0.0,
            },
            openai_metrics: Some(DistillationMetrics {
                accuracy: 0.92,
                cost_per_request: 0.0012,
                latency_ms: 600.0,
                setup_time_minutes: 20.0,
            }),
            local_metrics: Some(DistillationMetrics {
                accuracy: 0.88,
                cost_per_request: 0.0,
                latency_ms: 400.0,
                setup_time_minutes: 60.0,
            }),
            prompt_metrics: Some(DistillationMetrics {
                accuracy: 0.90,
                cost_per_request: 0.0015,
                latency_ms: 700.0,
                setup_time_minutes: 5.0,
            }),
            num_training_examples: 150,
            num_test_examples: 50,
        };

        let table = report.table();
        assert!(table.contains("Teacher"));
        assert!(table.contains("OpenAI"));
        assert!(table.contains("Local"));
        assert!(table.contains("Prompt"));
    }
}
