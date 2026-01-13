// Allow clippy warnings that are acceptable in this crate:
// - unwrap/expect: Used for mutex locks and iterator operations with guaranteed elements
// - clone on Arc: Intentional reference counting
// - needless_pass_by_value: Some APIs require owned values for consistency
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::clone_on_ref_ptr,
    clippy::needless_pass_by_value
)]
// M-2142: Unit tests frequently assert on known float thresholds/constants.
#![cfg_attr(test, allow(clippy::float_cmp))]

//! # `DashFlow` Evals - Production-Ready Evaluation Framework
//!
//! **dashflow-evals** is a comprehensive evaluation framework for `DashFlow` Rust agents,
//! providing world-class quality assurance for LLM applications.
//!
//! ## Features
//!
//! ### Core Evaluation
//! - **Golden Datasets**: Version-controlled test scenarios with expected outputs
//! - **LLM-as-Judge**: Multi-dimensional quality scoring (accuracy, relevance, completeness, safety, coherence, conciseness)
//! - **Validation**: String matching, latency checks, quality thresholds
//! - **Regression Detection**: Automatic detection of quality degradations
//!
//! ### Advanced Capabilities
//! - **Automated Test Generation**: Generate scenarios from production logs
//! - **Multi-Model Comparison**: A/B testing with cost/quality trade-off analysis
//! - **Continuous Learning**: Self-improving test suites from human feedback
//! - **Security Testing**: Prompt injection, jailbreak, PII leakage, bias detection
//! - **Performance Optimization**: Bottleneck identification and optimization suggestions
//!
//! ### Integration & Polish
//! - **Git Hooks**: Pre-commit/pre-push quality gates
//! - **CI/CD Integration**: GitHub Actions, exit codes, PR comments
//! - **Developer Tools**: Watch mode, interactive REPL, VS Code extension
//! - **Observability**: Kafka integration, real-time dashboards
//!
//! ## Quick Start
//!
//! ### 1. Create a Golden Dataset
//!
//! ```rust,no_run
//! use dashflow_evals::{GoldenDataset, GoldenScenario};
//!
//! let scenario = GoldenScenario {
//!     id: "01_simple_query".to_string(),
//!     description: "Basic factual query".to_string(),
//!     query: "What is tokio?".to_string(),
//!     expected_output_contains: vec!["async".to_string(), "runtime".to_string()],
//!     expected_output_not_contains: vec!["error".to_string()],
//!     quality_threshold: 0.90,
//!     max_latency_ms: Some(5000),
//!     context: None,
//!     expected_tool_calls: vec![],
//!     max_cost_usd: None,
//!     max_tokens: None,
//!     accuracy_threshold: None,
//!     relevance_threshold: None,
//!     completeness_threshold: None,
//!     safety_threshold: None,
//!     coherence_threshold: None,
//!     conciseness_threshold: None,
//!     case_insensitive_validation: false,
//!     difficulty: None,
//! };
//!
//! // In practice, load from JSON files:
//! // let dataset = GoldenDataset::load("examples/apps/librarian/data/golden_dataset")?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ### 2. Run Evaluations
//!
//! ```rust,no_run
//! use dashflow_evals::{EvalRunner, MultiDimensionalJudge};
//! use dashflow_openai::ChatOpenAI;
//!
//! # async fn example() -> anyhow::Result<()> {
//! use std::sync::Arc;
//!
//! // Setup judge
//! let model = Arc::new(ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini"));
//! let judge = MultiDimensionalJudge::new(model);
//!
//! // Setup agent function
//! let agent_fn = Arc::new(|query: String| {
//!     Box::pin(async move {
//!         // Run your agent here
//!         let output = "Your agent's response".to_string();
//!         Ok(dashflow_evals::AgentResponse::text_only(output))
//!     }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<dashflow_evals::AgentResponse>> + Send>>
//! });
//!
//! // Setup eval runner
//! let runner = EvalRunner::builder()
//!     .judge(judge)
//!     .agent_fn(agent_fn)
//!     .build();
//!
//! // Run evaluations
//! // let dataset = ...; // Load your dataset
//! // let report = runner.evaluate(&dataset).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 3. Check Results
//!
//! ```rust,no_run
//! # use dashflow_evals::EvalReport;
//! # fn example(report: EvalReport) {
//! println!("Pass Rate: {}/{}", report.passed, report.total);
//! println!("Avg Quality: {:.3}", report.avg_quality());
//! println!("Avg Latency: {}ms", report.avg_latency_ms());
//!
//! // Generate reports
//! dashflow_evals::generate_all_reports(&report, "my_app", "target/eval_reports", "report").ok();
//! # }
//! ```
//!
//! ## Module Overview
//!
//! - [`golden_dataset`] - Test scenario definitions and dataset management
//! - [`eval_runner`] - Evaluation execution engine
//! - [`quality_judge`] - LLM-as-judge quality scoring
//! - [`regression`] - Regression detection and alerts
//! - [`baseline`] - Baseline storage and comparison
//! - [`report`] - HTML, JSON, Markdown report generation
//! - [`security`] - Adversarial testing (prompt injection, bias, PII)
//! - [`performance`] - Performance analysis and optimization
//! - [`test_generation`] - Automated test scenario generation
//! - [`continuous_learning`] - Self-improving test suites
//! - [`multi_model`] - Multi-model comparison and A/B testing
//! - [`trends`] - Historical trend analysis and forecasting
//! - [`alerts`] - Alert generation for regressions
//! - [`ci`] - CI/CD quality gates
//! - [`notifications`] - Slack/GitHub notifications
//! - [`monitor`] - Continuous quality monitoring with Prometheus metrics (M-301)
//!
//! ## Examples
//!
//! See `examples/apps/librarian` for a complete integration example.
//!
//! ## Documentation
//!
//! - [Evaluation Guide](https://github.com/dropbox/dTOOL/dashflow/blob/main/docs/EVALUATION_GUIDE.md)
//! - [Developer Experience](https://github.com/dropbox/dTOOL/dashflow/blob/main/docs/DEVELOPER_EXPERIENCE.md)
//! - [Git Hooks Setup](https://github.com/dropbox/dTOOL/dashflow/blob/main/scripts/setup-eval-hooks.sh)
//!

pub mod alerts;
pub mod baseline;
pub mod ci;
pub mod continuous_learning;
pub mod eval_runner;
pub mod golden_dataset;
pub mod monitor;
pub mod multi_model;
pub mod notifications;
pub mod performance;
pub mod quality_judge;
pub mod regression;
pub mod report;
pub mod security;
pub mod test_generation;
pub mod trends;

pub use alerts::{Alert, AlertConfig, AlertGenerator, AlertSeverity};
pub use baseline::{BaselineMetadata, BaselineStore};
pub use ci::{GateResult, GateViolation, QualityGate, QualityGateConfig};
pub use continuous_learning::{
    ContinuousLearning, GeneratedTestCase, GenerationSource, GoldenPromotionInput, HumanFeedback,
    JudgeCorrectness, LearningConfig, LearningStatistics, TestCaseModifications,
    UncertaintyAnalysis,
};
pub use eval_runner::{
    AgentFn, AgentResponse, EvalConfig, EvalMetadata, EvalReport, EvalResult, EvalRunner,
    EvalRunnerBuilder, EvalSummary, ScenarioResult, TokenUsage, ValidationResult,
};
pub use golden_dataset::{GoldenDataset, GoldenScenario};
pub use multi_model::{
    ABTestReport, CostAnalysis, CostQualityAnalysis, DefaultModelFactory, ModelConfig,
    ModelFactory, ModelPerformance, MultiModelComparison, MultiModelConfig, MultiModelRunner,
    QualityAnalysis, RateLimiter, StatisticalTest,
};
pub use notifications::{SlackConfig, SlackMessage, SlackNotifier};
pub use performance::{
    Bottleneck, BottleneckImpact, BottleneckType, Complexity, ComponentTiming, ExpectedImprovement,
    HistoricalComparison, OptimizationSuggestion, OptimizationType, PerformanceAnalysis,
    PerformanceAnalyzer, PerformanceConfig, PerformanceSummary, PerformanceTrend, Priority,
    ScenarioPerformance,
};
pub use quality_judge::{IssueSeverity, MultiDimensionalJudge, QualityIssue, QualityScore};
pub use regression::{
    Regression, RegressionConfig, RegressionDetector, RegressionReport, RegressionType, Severity,
};
pub use report::{exit_code, generate_all_reports};
pub use security::{
    AdversarialFailure, AdversarialResults, AdversarialTestCase, BiasDimension, BiasIndicator,
    BiasResults, BiasScore, InjectionType, PiiInstance, PiiLeakageResults, PiiType,
    PromptInjectionResults, SecurityCategory, SecurityConfig, SecurityIssue, SecurityReport,
    SecurityTester, SecurityWarning, Severity as SecuritySeverity,
};
pub use test_generation::{
    AdversarialType, CoverageGoals, Difficulty, ProductionLog, ScenarioGenerator,
    TestGenerationConfig, UserFeedback,
};
pub use trends::{Anomaly, QualityForecast, TrendAnalyzer, TrendDirection, TrendInfo, TrendReport};

// Quality monitoring for continuous regression detection (M-301)
pub use monitor::{MonitorConfig, MonitorMetrics, QualityMonitor, RegressionCheckResult};
