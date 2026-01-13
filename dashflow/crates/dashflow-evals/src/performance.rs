//! Performance Optimization Tools
//!
//! This module provides tools for identifying performance bottlenecks,
//! profiling evaluation runs, and suggesting optimizations.
//!
//! Key capabilities:
//! - Bottleneck identification (slow scenarios, expensive operations)
//! - Performance profiling (timing breakdown by component)
//! - Optimization suggestions (caching, batching, model selection)
//! - Historical comparison against baselines (detect performance regressions)
//! - Performance tracking over time (trends, regressions)

use crate::EvalReport;
use serde::{Deserialize, Serialize};

/// Performance analyzer for evaluation runs
pub struct PerformanceAnalyzer {
    config: PerformanceConfig,
}

/// Configuration for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Threshold for identifying slow scenarios (ms)
    pub slow_threshold_ms: u64,

    /// Threshold for identifying expensive scenarios (USD)
    pub expensive_threshold_usd: f64,

    /// Enable optimization suggestions
    pub suggest_optimizations: bool,

    /// Track historical performance data
    pub track_history: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            slow_threshold_ms: 5000,       // 5 seconds
            expensive_threshold_usd: 0.05, // $0.05
            suggest_optimizations: true,
            track_history: true,
        }
    }
}

/// Results from performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    /// Overall performance summary
    pub summary: PerformanceSummary,

    /// Identified bottlenecks
    pub bottlenecks: Vec<Bottleneck>,

    /// Optimization suggestions
    pub suggestions: Vec<OptimizationSuggestion>,

    /// Performance breakdown by scenario
    pub scenario_breakdown: Vec<ScenarioPerformance>,

    /// Historical comparison (if available)
    pub historical_comparison: Option<HistoricalComparison>,
}

/// Overall performance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    /// Total evaluation time (ms)
    pub total_time_ms: u64,

    /// Average time per scenario (ms)
    pub avg_time_ms: u64,

    /// Median time per scenario (ms)
    pub median_time_ms: u64,

    /// P95 latency (ms)
    pub p95_latency_ms: u64,

    /// P99 latency (ms)
    pub p99_latency_ms: u64,

    /// Total cost (USD)
    pub total_cost_usd: f64,

    /// Average cost per scenario (USD)
    pub avg_cost_usd: f64,

    /// Number of slow scenarios
    pub slow_scenario_count: usize,

    /// Number of expensive scenarios
    pub expensive_scenario_count: usize,
}

/// Performance bottleneck
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Type of bottleneck
    pub bottleneck_type: BottleneckType,

    /// Severity (0-1, higher is worse)
    pub severity: f64,

    /// Description
    pub description: String,

    /// Affected scenarios
    pub affected_scenarios: Vec<String>,

    /// Impact estimate
    pub impact: BottleneckImpact,
}

/// Type of performance bottleneck
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BottleneckType {
    /// LLM API latency
    LlmLatency,

    /// Judge scoring time
    JudgeScoring,

    /// Tool execution time
    ToolExecution,

    /// Sequential execution (could be parallelized)
    SequentialExecution,

    /// Large context windows
    LargeContext,

    /// Expensive model usage
    ExpensiveModel,

    /// Other bottleneck
    Other(String),
}

/// Impact of a bottleneck
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckImpact {
    /// Additional time cost (ms)
    pub time_cost_ms: u64,

    /// Additional monetary cost (USD)
    pub monetary_cost_usd: f64,

    /// Percentage of total time
    pub time_percentage: f64,

    /// Percentage of total cost
    pub cost_percentage: f64,
}

/// Optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Type of optimization
    pub optimization_type: OptimizationType,

    /// Priority (higher = more impactful)
    pub priority: Priority,

    /// Description
    pub description: String,

    /// Expected improvement
    pub expected_improvement: ExpectedImprovement,

    /// Implementation complexity
    pub complexity: Complexity,
}

/// Type of optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OptimizationType {
    /// Use faster model (e.g., GPT-4o-mini instead of GPT-4o)
    UseFasterModel,

    /// Implement caching for repeated queries
    ImplementCaching,

    /// Batch LLM requests
    BatchRequests,

    /// Parallelize independent operations
    Parallelize,

    /// Reduce context window size
    ReduceContext,

    /// Use streaming responses
    UseStreaming,

    /// Optimize prompts (shorter, more efficient)
    OptimizePrompts,

    /// Other optimization
    Other(String),
}

/// Priority level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Expected improvement from optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImprovement {
    /// Expected time savings (ms)
    pub time_savings_ms: u64,

    /// Expected cost savings (USD)
    pub cost_savings_usd: f64,

    /// Expected time reduction (%)
    pub time_reduction_percent: f64,

    /// Expected cost reduction (%)
    pub cost_reduction_percent: f64,
}

/// Implementation complexity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Complexity {
    /// Easy (< 1 hour)
    Easy,

    /// Medium (1-4 hours)
    Medium,

    /// Hard (> 4 hours)
    Hard,
}

/// Performance breakdown for a single scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioPerformance {
    pub scenario_id: String,
    pub latency_ms: u64,
    pub cost_usd: f64,
    pub is_slow: bool,
    pub is_expensive: bool,
    pub component_breakdown: Vec<ComponentTiming>,
}

/// Timing for a specific component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTiming {
    pub component: String,
    pub duration_ms: u64,
    pub percentage: f64,
}

/// Historical performance comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalComparison {
    /// Current vs previous evaluation
    pub time_delta_ms: i64,

    /// Time change percentage
    pub time_delta_percent: f64,

    /// Cost delta
    pub cost_delta_usd: f64,

    /// Cost change percentage
    pub cost_delta_percent: f64,

    /// Performance trend
    pub trend: PerformanceTrend,
}

/// Performance trend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
}

impl PerformanceAnalyzer {
    /// Create a new performance analyzer
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: PerformanceConfig::default(),
        }
    }

    /// Create with custom config
    #[must_use]
    pub fn with_config(config: PerformanceConfig) -> Self {
        Self { config }
    }

    /// Analyze performance of an evaluation report
    #[must_use]
    pub fn analyze(&self, report: &EvalReport) -> PerformanceAnalysis {
        let summary = self.calculate_summary(report);
        let bottlenecks = self.identify_bottlenecks(report);
        let suggestions = if self.config.suggest_optimizations {
            self.generate_suggestions(&bottlenecks, report)
        } else {
            Vec::new()
        };
        let scenario_breakdown = self.breakdown_by_scenario(report);
        let historical_comparison = None;

        PerformanceAnalysis {
            summary,
            bottlenecks,
            suggestions,
            scenario_breakdown,
            historical_comparison,
        }
    }

    /// Analyze performance with historical comparison against a baseline
    ///
    /// # Arguments
    ///
    /// * `report` - Current evaluation report
    /// * `baseline` - Previous evaluation report to compare against
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::{PerformanceAnalyzer, EvalReport, BaselineStore};
    /// # async fn example(report: EvalReport) -> anyhow::Result<()> {
    /// let analyzer = PerformanceAnalyzer::new();
    /// let store = BaselineStore::new("baselines");
    ///
    /// // Load baseline for comparison
    /// if let Ok(baseline) = store.load_baseline("main", "my_app") {
    ///     let analysis = analyzer.analyze_with_baseline(&report, &baseline);
    ///     if let Some(comparison) = analysis.historical_comparison {
    ///         println!("Performance trend: {:?}", comparison.trend);
    ///         println!("Time delta: {}ms ({:.1}%)",
    ///                  comparison.time_delta_ms,
    ///                  comparison.time_delta_percent);
    ///     }
    /// } else {
    ///     // No baseline available, use regular analysis
    ///     let analysis = analyzer.analyze(&report);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn analyze_with_baseline(
        &self,
        report: &EvalReport,
        baseline: &EvalReport,
    ) -> PerformanceAnalysis {
        let summary = self.calculate_summary(report);
        let bottlenecks = self.identify_bottlenecks(report);
        let suggestions = if self.config.suggest_optimizations {
            self.generate_suggestions(&bottlenecks, report)
        } else {
            Vec::new()
        };
        let scenario_breakdown = self.breakdown_by_scenario(report);
        let historical_comparison = self.compare_with_baseline(report, baseline);

        PerformanceAnalysis {
            summary,
            bottlenecks,
            suggestions,
            scenario_breakdown,
            historical_comparison,
        }
    }

    /// Compare current report against baseline to detect performance changes
    fn compare_with_baseline(
        &self,
        current: &EvalReport,
        baseline: &EvalReport,
    ) -> Option<HistoricalComparison> {
        // Calculate average latency for both reports
        let current_latencies: Vec<u64> = current.results.iter().map(|r| r.latency_ms).collect();
        let baseline_latencies: Vec<u64> = baseline.results.iter().map(|r| r.latency_ms).collect();

        if current_latencies.is_empty() || baseline_latencies.is_empty() {
            return None;
        }

        let current_avg =
            current_latencies.iter().sum::<u64>() as f64 / current_latencies.len() as f64;
        let baseline_avg =
            baseline_latencies.iter().sum::<u64>() as f64 / baseline_latencies.len() as f64;

        // Calculate time delta
        let time_delta_ms = current_avg as i64 - baseline_avg as i64;
        let time_delta_percent = if baseline_avg > 0.0 {
            ((current_avg - baseline_avg) / baseline_avg) * 100.0
        } else {
            0.0
        };

        // For cost comparison, we'd need actual token usage data
        // For now, use rough estimates based on scenario count
        let current_cost = current.results.len() as f64 * 0.01;
        let baseline_cost = baseline.results.len() as f64 * 0.01;
        let cost_delta_usd = current_cost - baseline_cost;
        let cost_delta_percent = if baseline_cost > 0.0 {
            ((current_cost - baseline_cost) / baseline_cost) * 100.0
        } else {
            0.0
        };

        // Determine performance trend
        let trend = if time_delta_percent < -5.0 {
            // More than 5% faster
            PerformanceTrend::Improving
        } else if time_delta_percent > 10.0 {
            // More than 10% slower
            PerformanceTrend::Degrading
        } else {
            // Within ±5-10% range
            PerformanceTrend::Stable
        };

        Some(HistoricalComparison {
            time_delta_ms,
            time_delta_percent,
            cost_delta_usd,
            cost_delta_percent,
            trend,
        })
    }

    /// Calculate performance summary
    fn calculate_summary(&self, report: &EvalReport) -> PerformanceSummary {
        let mut latencies: Vec<u64> = report.results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();

        let total_time_ms: u64 = latencies.iter().sum();
        let avg_time_ms = if latencies.is_empty() {
            0
        } else {
            total_time_ms / latencies.len() as u64
        };

        let median_time_ms = if latencies.is_empty() {
            0
        } else {
            latencies[latencies.len() / 2]
        };

        let p95_latency_ms = if latencies.is_empty() {
            0
        } else {
            let idx = ((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1);
            latencies[idx]
        };

        let p99_latency_ms = if latencies.is_empty() {
            0
        } else {
            let idx = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
            latencies[idx]
        };

        // Note: Cost calculation would require actual token usage data
        // For now, estimate based on average costs
        let total_cost_usd = latencies.len() as f64 * 0.01; // Rough estimate
        let avg_cost_usd = if latencies.is_empty() {
            0.0
        } else {
            total_cost_usd / latencies.len() as f64
        };

        let slow_scenario_count = report
            .results
            .iter()
            .filter(|r| r.latency_ms > self.config.slow_threshold_ms)
            .count();

        let expensive_scenario_count = report
            .results
            .iter()
            .filter(|_r| {
                // Would need actual cost data
                false
            })
            .count();

        PerformanceSummary {
            total_time_ms,
            avg_time_ms,
            median_time_ms,
            p95_latency_ms,
            p99_latency_ms,
            total_cost_usd,
            avg_cost_usd,
            slow_scenario_count,
            expensive_scenario_count,
        }
    }

    /// Identify performance bottlenecks
    fn identify_bottlenecks(&self, report: &EvalReport) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();

        // Identify slow scenarios
        let slow_scenarios: Vec<_> = report
            .results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.latency_ms > self.config.slow_threshold_ms)
            .collect();

        if !slow_scenarios.is_empty() {
            let total_time: u64 = report.results.iter().map(|r| r.latency_ms).sum();
            let slow_time: u64 = slow_scenarios.iter().map(|(_, r)| r.latency_ms).sum();
            let time_percentage = if total_time > 0 {
                (slow_time as f64 / total_time as f64) * 100.0
            } else {
                0.0
            };

            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::LlmLatency,
                severity: (slow_scenarios.len() as f64 / report.results.len() as f64).min(1.0),
                description: format!(
                    "{} scenarios exceed {}ms threshold ({}% of total time)",
                    slow_scenarios.len(),
                    self.config.slow_threshold_ms,
                    time_percentage as u32
                ),
                affected_scenarios: slow_scenarios
                    .iter()
                    .map(|(_, r)| r.scenario_id.clone())
                    .collect(),
                impact: BottleneckImpact {
                    time_cost_ms: slow_time,
                    monetary_cost_usd: 0.0, // Would need actual data
                    time_percentage,
                    cost_percentage: 0.0,
                },
            });
        }

        // Identify judge scoring bottleneck
        // (In practice, would parse this from DashStream events or timing data)
        let avg_latency = if report.results.is_empty() {
            0
        } else {
            report.results.iter().map(|r| r.latency_ms).sum::<u64>() / report.results.len() as u64
        };

        if avg_latency > 3000 {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::JudgeScoring,
                severity: 0.6,
                description: "LLM-as-judge scoring adds significant latency".to_string(),
                affected_scenarios: vec![], // All scenarios
                impact: BottleneckImpact {
                    time_cost_ms: avg_latency / 2, // Estimate judge takes ~50% of time
                    monetary_cost_usd: 0.01,       // Estimate
                    time_percentage: 50.0,
                    cost_percentage: 20.0,
                },
            });
        }

        bottlenecks
    }

    /// Generate optimization suggestions
    fn generate_suggestions(
        &self,
        bottlenecks: &[Bottleneck],
        report: &EvalReport,
    ) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        for bottleneck in bottlenecks {
            match bottleneck.bottleneck_type {
                BottleneckType::LlmLatency => {
                    // Suggest faster model
                    suggestions.push(OptimizationSuggestion {
                        optimization_type: OptimizationType::UseFasterModel,
                        priority: Priority::High,
                        description: "Use GPT-4o-mini instead of GPT-4o for judge scoring to reduce latency by ~60%".to_string(),
                        expected_improvement: ExpectedImprovement {
                            time_savings_ms: (bottleneck.impact.time_cost_ms as f64 * 0.6) as u64,
                            cost_savings_usd: bottleneck.impact.monetary_cost_usd * 0.8,
                            time_reduction_percent: bottleneck.impact.time_percentage * 0.6,
                            cost_reduction_percent: 80.0,
                        },
                        complexity: Complexity::Easy,
                    });

                    // Suggest parallelization
                    if report.results.len() > 5 {
                        suggestions.push(OptimizationSuggestion {
                            optimization_type: OptimizationType::Parallelize,
                            priority: Priority::Medium,
                            description: format!(
                                "Run {} scenarios in parallel (currently sequential)",
                                report.results.len()
                            ),
                            expected_improvement: ExpectedImprovement {
                                time_savings_ms: (bottleneck.impact.time_cost_ms as f64 * 0.7)
                                    as u64,
                                cost_savings_usd: 0.0,
                                time_reduction_percent: 70.0,
                                cost_reduction_percent: 0.0,
                            },
                            complexity: Complexity::Medium,
                        });
                    }
                }
                BottleneckType::JudgeScoring => {
                    suggestions.push(OptimizationSuggestion {
                        optimization_type: OptimizationType::BatchRequests,
                        priority: Priority::Medium,
                        description: "Batch judge scoring requests to reduce API overhead"
                            .to_string(),
                        expected_improvement: ExpectedImprovement {
                            time_savings_ms: 500 * report.results.len() as u64,
                            cost_savings_usd: 0.0,
                            time_reduction_percent: 20.0,
                            cost_reduction_percent: 0.0,
                        },
                        complexity: Complexity::Medium,
                    });
                }
                BottleneckType::LargeContext => {
                    suggestions.push(OptimizationSuggestion {
                        optimization_type: OptimizationType::ReduceContext,
                        priority: Priority::Low,
                        description:
                            "Reduce context window size to improve latency and reduce costs"
                                .to_string(),
                        expected_improvement: ExpectedImprovement {
                            time_savings_ms: 200,
                            cost_savings_usd: 0.005,
                            time_reduction_percent: 10.0,
                            cost_reduction_percent: 15.0,
                        },
                        complexity: Complexity::Easy,
                    });
                }
                _ => {}
            }
        }

        // Sort by priority
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions
    }

    /// Break down performance by scenario
    fn breakdown_by_scenario(&self, report: &EvalReport) -> Vec<ScenarioPerformance> {
        report
            .results
            .iter()
            .map(|result| {
                let is_slow = result.latency_ms > self.config.slow_threshold_ms;
                let is_expensive = false; // Would need actual cost data

                // Estimate component breakdown
                // In practice, would parse from DashStream events
                let component_breakdown = vec![
                    ComponentTiming {
                        component: "Agent execution".to_string(),
                        duration_ms: result.latency_ms / 2,
                        percentage: 50.0,
                    },
                    ComponentTiming {
                        component: "Judge scoring".to_string(),
                        duration_ms: result.latency_ms / 2,
                        percentage: 50.0,
                    },
                ];

                ScenarioPerformance {
                    scenario_id: result.scenario_id.clone(),
                    latency_ms: result.latency_ms,
                    cost_usd: 0.01, // Estimate
                    is_slow,
                    is_expensive,
                    component_breakdown,
                }
            })
            .collect()
    }

    /// Compare performance to historical baseline
    #[must_use]
    pub fn compare_to_baseline(
        &self,
        current: &EvalReport,
        baseline: &EvalReport,
    ) -> HistoricalComparison {
        let current_time: u64 = current.results.iter().map(|r| r.latency_ms).sum();
        let baseline_time: u64 = baseline.results.iter().map(|r| r.latency_ms).sum();

        let time_delta_ms = current_time as i64 - baseline_time as i64;
        let time_delta_percent = if baseline_time > 0 {
            (time_delta_ms as f64 / baseline_time as f64) * 100.0
        } else {
            0.0
        };

        // Estimate costs (would need actual data)
        let current_cost = current.results.len() as f64 * 0.01;
        let baseline_cost = baseline.results.len() as f64 * 0.01;
        let cost_delta_usd = current_cost - baseline_cost;
        let cost_delta_percent = if baseline_cost > 0.0 {
            (cost_delta_usd / baseline_cost) * 100.0
        } else {
            0.0
        };

        let trend = if time_delta_percent < -5.0 {
            PerformanceTrend::Improving
        } else if time_delta_percent > 10.0 {
            PerformanceTrend::Degrading
        } else {
            PerformanceTrend::Stable
        };

        HistoricalComparison {
            time_delta_ms,
            time_delta_percent,
            cost_delta_usd,
            cost_delta_percent,
            trend,
        }
    }
}

impl Default for PerformanceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvalMetadata, EvalReport, QualityScore, ScenarioResult, ValidationResult};

    fn create_test_report(scenario_count: usize, avg_latency_ms: u64) -> EvalReport {
        let mut results = Vec::new();
        let mut passed_count = 0;

        for i in 0..scenario_count {
            let result = ScenarioResult {
                scenario_id: format!("test_{}", i),
                passed: true,
                output: "Test output".to_string(),
                quality_score: QualityScore {
                    accuracy: 0.95,
                    relevance: 0.95,
                    completeness: 0.95,
                    safety: 1.0,
                    coherence: 0.95,
                    conciseness: 0.90,
                    overall: 0.95,
                    reasoning: "Test".to_string(),
                    issues: vec![],
                    suggestions: vec![],
                },
                latency_ms: avg_latency_ms + (i as u64 * 100), // Vary latency
                validation: ValidationResult {
                    passed: true,
                    missing_contains: vec![],
                    forbidden_found: vec![],
                    failure_reason: None,
                },
                error: None,
                retry_attempts: 0,
                timestamp: chrono::Utc::now(),
                input: Some("test input".to_string()),
                tokens_used: None,
                cost_usd: None,
            };
            if result.passed {
                passed_count += 1;
            }
            results.push(result);
        }

        let now = chrono::Utc::now();
        EvalReport {
            total: scenario_count,
            passed: passed_count,
            failed: scenario_count - passed_count,
            results,
            metadata: EvalMetadata {
                started_at: now,
                completed_at: now,
                duration_secs: 10.0,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.slow_threshold_ms, 5000);
        assert_eq!(config.expensive_threshold_usd, 0.05);
        assert!(config.suggest_optimizations);
        assert!(config.track_history);
    }

    #[test]
    fn test_calculate_summary() {
        let analyzer = PerformanceAnalyzer::new();
        let report = create_test_report(10, 2000);

        let summary = analyzer.calculate_summary(&report);

        assert_eq!(summary.total_time_ms, 24500); // 2000*10 + (0+100+200+...+900)
        assert_eq!(summary.avg_time_ms, 2450);
        assert!(summary.median_time_ms > 0);
        assert!(summary.p95_latency_ms > summary.median_time_ms);
        assert!(summary.p99_latency_ms >= summary.p95_latency_ms);
    }

    #[test]
    fn test_identify_bottlenecks() {
        let analyzer = PerformanceAnalyzer::new();
        let report = create_test_report(10, 6000); // Above threshold

        let bottlenecks = analyzer.identify_bottlenecks(&report);

        assert!(!bottlenecks.is_empty());
        assert!(bottlenecks
            .iter()
            .any(|b| b.bottleneck_type == BottleneckType::LlmLatency));
    }

    #[test]
    fn test_generate_suggestions() {
        let analyzer = PerformanceAnalyzer::new();
        let report = create_test_report(10, 6000);
        let bottlenecks = analyzer.identify_bottlenecks(&report);

        let suggestions = analyzer.generate_suggestions(&bottlenecks, &report);

        assert!(!suggestions.is_empty());
        assert!(suggestions
            .iter()
            .any(|s| s.optimization_type == OptimizationType::UseFasterModel));
    }

    #[test]
    fn test_full_analysis() {
        let analyzer = PerformanceAnalyzer::new();
        let report = create_test_report(10, 6000);

        let analysis = analyzer.analyze(&report);

        assert!(analysis.summary.total_time_ms > 0);
        assert!(!analysis.bottlenecks.is_empty());
        assert!(!analysis.suggestions.is_empty());
        assert_eq!(analysis.scenario_breakdown.len(), 10);
    }

    #[test]
    fn test_historical_comparison() {
        let analyzer = PerformanceAnalyzer::new();
        let current = create_test_report(10, 3000);
        let baseline = create_test_report(10, 2000);

        let comparison = analyzer.compare_to_baseline(&current, &baseline);

        assert!(comparison.time_delta_ms > 0); // Current is slower
        assert!(comparison.time_delta_percent > 0.0);
        assert_eq!(comparison.trend, PerformanceTrend::Degrading);
    }

    #[test]
    fn test_performance_analysis_serialization() {
        let analyzer = PerformanceAnalyzer::new();
        let report = create_test_report(5, 2000);
        let analysis = analyzer.analyze(&report);

        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: PerformanceAnalysis = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.summary.total_time_ms,
            analysis.summary.total_time_ms
        );
        assert_eq!(deserialized.bottlenecks.len(), analysis.bottlenecks.len());
    }

    #[test]
    fn test_analyze_with_baseline() {
        let analyzer = PerformanceAnalyzer::new();
        let current = create_test_report(10, 3000);
        let baseline = create_test_report(10, 2000);

        let analysis = analyzer.analyze_with_baseline(&current, &baseline);

        // Check that historical comparison is populated
        assert!(analysis.historical_comparison.is_some());
        let comparison = analysis.historical_comparison.unwrap();

        // Current is slower than baseline (3000ms avg vs 2000ms avg)
        assert!(comparison.time_delta_ms > 0);
        assert!(comparison.time_delta_percent > 0.0);
        assert_eq!(comparison.trend, PerformanceTrend::Degrading);

        // Other analysis fields should also be populated
        assert!(analysis.summary.total_time_ms > 0);
        assert_eq!(analysis.scenario_breakdown.len(), 10);
    }

    #[test]
    fn test_analyze_with_baseline_improving() {
        let analyzer = PerformanceAnalyzer::new();
        let current = create_test_report(10, 1800); // Faster
        let baseline = create_test_report(10, 2000);

        let analysis = analyzer.analyze_with_baseline(&current, &baseline);

        let comparison = analysis.historical_comparison.unwrap();

        // Current is faster than baseline
        assert!(comparison.time_delta_ms < 0);
        assert!(comparison.time_delta_percent < -5.0);
        assert_eq!(comparison.trend, PerformanceTrend::Improving);
    }

    #[test]
    fn test_analyze_with_baseline_stable() {
        let analyzer = PerformanceAnalyzer::new();
        let current = create_test_report(10, 2040); // Slightly slower but within stable range
        let baseline = create_test_report(10, 2000);

        let analysis = analyzer.analyze_with_baseline(&current, &baseline);

        let comparison = analysis.historical_comparison.unwrap();

        // Within stable range (< 10% slower)
        assert_eq!(comparison.trend, PerformanceTrend::Stable);
    }

    #[test]
    fn test_analyze_with_baseline_empty_reports() {
        let analyzer = PerformanceAnalyzer::new();
        let current = create_test_report(0, 2000); // Empty report
        let baseline = create_test_report(10, 2000);

        let analysis = analyzer.analyze_with_baseline(&current, &baseline);

        // Should return None for historical comparison when current is empty
        assert!(analysis.historical_comparison.is_none());
    }
}
