//! Continuous Learning Module
//!
//! This module provides infrastructure for self-improving evaluation systems:
//! - Convert failed scenarios into new test cases
//! - Human feedback loops for test refinement
//! - Active learning to focus on uncertain scenarios
//! - Self-improving test suite evolution
//! - Promote approved test cases to `GoldenScenario` entries
//!
//! **Status**: MVP implementation that writes reviewable JSON test cases to disk
//! with opt-in conversion to `GoldenScenario` via [`GeneratedTestCase::to_golden_scenario`].

use crate::eval_runner::ScenarioResult;
use crate::golden_dataset::GoldenScenario;
use crate::test_generation::Difficulty;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Continuous learning engine that generates new test cases from failures
#[derive(Debug)]
pub struct ContinuousLearning {
    config: LearningConfig,
    feedback_store: FeedbackStore,
}

/// Configuration for continuous learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    /// Automatically generate test cases from failures?
    pub auto_generate_from_failures: bool,

    /// Minimum quality threshold to consider a failure
    pub failure_quality_threshold: f64,

    /// Maximum number of new tests to generate per run
    pub max_new_tests_per_run: usize,

    /// Require human approval before adding to golden dataset?
    pub require_human_approval: bool,

    /// Active learning: prioritize scenarios with high uncertainty
    pub enable_active_learning: bool,

    /// Uncertainty threshold for active learning (higher = more uncertain)
    pub uncertainty_threshold: f64,

    /// Path to store pending test cases
    pub pending_tests_dir: PathBuf,

    /// Path to human feedback database
    pub feedback_db_path: PathBuf,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            auto_generate_from_failures: true,
            failure_quality_threshold: 0.70,
            max_new_tests_per_run: 10,
            require_human_approval: true,
            enable_active_learning: true,
            uncertainty_threshold: 0.15,
            pending_tests_dir: PathBuf::from("eval_data/pending_tests"),
            feedback_db_path: PathBuf::from("eval_data/feedback.json"),
        }
    }
}

/// Human feedback on a test scenario or result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanFeedback {
    /// Scenario ID being reviewed
    pub scenario_id: String,

    /// Timestamp of feedback
    pub timestamp: String,

    /// Reviewer name
    pub reviewer: String,

    /// Was the LLM judge correct?
    pub judge_correctness: JudgeCorrectness,

    /// Human's quality assessment (if judge was wrong)
    pub human_quality_score: Option<f64>,

    /// Human's reasoning
    pub reasoning: String,

    /// Should this be added to golden dataset?
    pub approved_for_golden_dataset: bool,

    /// Suggested modifications to test case
    pub suggested_modifications: Option<TestCaseModifications>,

    /// Tags for categorization
    pub tags: Vec<String>,
}

/// Judgment of LLM judge correctness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JudgeCorrectness {
    /// Judge was correct
    Correct,
    /// Judge was too harsh
    TooHarsh,
    /// Judge was too lenient
    TooLenient,
    /// Judge completely wrong
    Wrong,
}

/// Suggested modifications to a test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseModifications {
    /// New expected output
    pub new_expected_output: Option<String>,

    /// Adjusted quality thresholds
    pub new_quality_threshold: Option<f64>,

    /// Adjusted `must_contain` requirements
    pub new_must_contain: Option<Vec<String>>,

    /// Adjusted `must_not_contain` requirements
    pub new_must_not_contain: Option<Vec<String>>,

    /// New difficulty classification
    pub new_difficulty: Option<Difficulty>,

    /// Additional context to add
    pub additional_context: Option<String>,
}

/// Storage for human feedback
#[derive(Debug)]
struct FeedbackStore {
    db_path: PathBuf,
    feedback: HashMap<String, Vec<HumanFeedback>>,
}

impl FeedbackStore {
    fn new(db_path: PathBuf) -> Self {
        let feedback = if db_path.exists() {
            std::fs::read_to_string(&db_path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        Self { db_path, feedback }
    }

    fn add_feedback(&mut self, feedback: HumanFeedback) -> Result<()> {
        self.feedback
            .entry(feedback.scenario_id.clone())
            .or_default()
            .push(feedback);
        self.save()
    }

    fn get_feedback(&self, scenario_id: &str) -> Vec<&HumanFeedback> {
        self.feedback
            .get(scenario_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.feedback)?;
        std::fs::write(&self.db_path, content)?;
        Ok(())
    }
}

/// Generated test case from a failure or uncertainty
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTestCase {
    /// Generated scenario (simplified - full version would use `GoldenScenario`)
    pub scenario_id: String,

    /// Query text
    pub query: String,

    /// Expected output or patterns
    pub expected_output: Option<String>,

    /// Output observed during evaluation (for reviewer context)
    #[serde(default)]
    pub observed_output: Option<String>,

    /// Source of generation
    pub generation_source: GenerationSource,

    /// Confidence in this test case (0-1)
    pub confidence: f64,

    /// Reason for generation
    pub generation_reason: String,

    /// Needs human review?
    pub needs_review: bool,
}

/// Source of test case generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenerationSource {
    /// Generated from a failure
    Failure { original_scenario_id: String },
    /// Generated from high uncertainty
    Uncertainty { original_scenario_id: String },
    /// Generated from human feedback
    HumanFeedback { feedback_id: String },
    /// Synthesized from similar scenarios
    Synthesis { source_scenario_ids: Vec<String> },
}

/// Human-provided fields required to promote a `GeneratedTestCase` to a `GoldenScenario`.
///
/// These fields cannot be inferred automatically and require human judgment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenPromotionInput {
    /// Human-readable description of what this scenario tests
    pub description: String,

    /// Strings that MUST appear in the output (human-curated, not inferred from observed output)
    #[serde(default)]
    pub expected_output_contains: Vec<String>,

    /// Strings that MUST NOT appear in the output
    #[serde(default)]
    pub expected_output_not_contains: Vec<String>,

    /// Minimum quality score threshold (0.0-1.0), defaults to 0.85
    #[serde(default = "default_quality_threshold")]
    pub quality_threshold: f64,

    /// Additional context for the scenario
    #[serde(default)]
    pub context: Option<String>,

    /// Difficulty level (if known)
    #[serde(default)]
    pub difficulty: Option<Difficulty>,

    /// Maximum allowed latency in milliseconds
    #[serde(default)]
    pub max_latency_ms: Option<u64>,

    /// Expected tool calls (for validation)
    #[serde(default)]
    pub expected_tool_calls: Vec<String>,
}

fn default_quality_threshold() -> f64 {
    0.85
}

impl Default for GoldenPromotionInput {
    fn default() -> Self {
        Self {
            description: String::new(),
            expected_output_contains: Vec::new(),
            expected_output_not_contains: Vec::new(),
            quality_threshold: default_quality_threshold(),
            context: None,
            difficulty: None,
            max_latency_ms: None,
            expected_tool_calls: Vec::new(),
        }
    }
}

impl GeneratedTestCase {
    /// Convert this generated test case to a `GoldenScenario` for inclusion in the golden dataset.
    ///
    /// This method requires human-provided input to fill in fields that cannot be inferred
    /// automatically. The `observed_output` is intentionally NOT used as `expected_output_contains`
    /// since failed outputs should not become golden expectations.
    ///
    /// # Arguments
    /// * `input` - Human-provided fields required for the golden scenario
    ///
    /// # Returns
    /// A fully-formed `GoldenScenario` ready to be saved to the golden dataset directory.
    ///
    /// # Example
    /// ```ignore
    /// let generated = learning.load_pending_tests()?
    ///     .into_iter()
    ///     .find(|t| t.scenario_id == "my_test_failure_variant")
    ///     .unwrap();
    ///
    /// let input = GoldenPromotionInput {
    ///     description: "Validates correct tokio usage".to_string(),
    ///     expected_output_contains: vec!["tokio".to_string(), "async".to_string()],
    ///     quality_threshold: 0.90,
    ///     ..Default::default()
    /// };
    ///
    /// let golden = generated.to_golden_scenario(input);
    /// ```
    #[must_use]
    pub fn to_golden_scenario(&self, input: GoldenPromotionInput) -> GoldenScenario {
        GoldenScenario {
            id: self.scenario_id.clone(),
            description: input.description,
            query: self.query.clone(),
            context: input.context,
            expected_output_contains: input.expected_output_contains,
            expected_output_not_contains: input.expected_output_not_contains,
            quality_threshold: input.quality_threshold,
            max_latency_ms: input.max_latency_ms,
            expected_tool_calls: input.expected_tool_calls,
            max_cost_usd: None,
            max_tokens: None,
            accuracy_threshold: None,
            relevance_threshold: None,
            completeness_threshold: None,
            safety_threshold: None,
            coherence_threshold: None,
            conciseness_threshold: None,
            case_insensitive_validation: false,
            difficulty: input.difficulty,
        }
    }
}

/// Uncertainty analysis for active learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncertaintyAnalysis {
    /// Scenario ID
    pub scenario_id: String,

    /// Variance in scores across dimensions
    pub score_variance: f64,

    /// Confidence estimate from judge
    pub judge_confidence: Option<f64>,

    /// Historical consistency (if run multiple times)
    pub historical_variance: Option<f64>,

    /// Overall uncertainty score (0-1, higher = more uncertain)
    pub overall_uncertainty: f64,

    /// Reasons for uncertainty
    pub uncertainty_factors: Vec<String>,
}

impl ContinuousLearning {
    /// Create a new continuous learning engine
    #[must_use]
    pub fn new(config: LearningConfig) -> Self {
        let feedback_store = FeedbackStore::new(config.feedback_db_path.clone());

        Self {
            config,
            feedback_store,
        }
    }

    /// Process evaluation results and generate new test cases
    ///
    /// This is a stub implementation. Full version would:
    /// - Analyze failures and generate test variants
    /// - Identify high-uncertainty scenarios
    /// - Create new `GoldenScenario` objects
    pub fn process_results(
        &mut self,
        results: &[ScenarioResult],
    ) -> Result<Vec<GeneratedTestCase>> {
        let mut generated_tests = Vec::new();

        // 1. Generate tests from failures
        if self.config.auto_generate_from_failures {
            for result in results {
                if !result.passed
                    && result.quality_score.overall < self.config.failure_quality_threshold
                {
                    if let Some(test) = self.generate_from_failure(result)? {
                        generated_tests.push(test);
                    }
                }
            }
        }

        // 2. Active learning: generate tests from high-uncertainty scenarios
        if self.config.enable_active_learning {
            for result in results {
                let uncertainty = self.analyze_uncertainty(result)?;
                if uncertainty.overall_uncertainty > self.config.uncertainty_threshold {
                    if let Some(test) = self.generate_from_uncertainty(result, &uncertainty)? {
                        generated_tests.push(test);
                    }
                }
            }
        }

        // 3. Limit number of new tests
        generated_tests.truncate(self.config.max_new_tests_per_run);

        // 4. Save pending tests for review
        self.save_pending_tests(&generated_tests)?;

        Ok(generated_tests)
    }

    /// Generate a reviewable test case from a failure.
    ///
    /// Important: the observed output is almost certainly *not* the correct expected output.
    /// We persist it for reviewer context, but leave `expected_output` empty.
    fn generate_from_failure(
        &self,
        result: &ScenarioResult,
    ) -> Result<Option<GeneratedTestCase>> {
        let failure_reasons = self.analyze_failure(result);

        Ok(Some(GeneratedTestCase {
            scenario_id: format!("{}_failure_variant", result.scenario_id),
            query: result
                .input
                .clone()
                .unwrap_or_else(|| "Unknown query".to_string()),
            expected_output: None,
            observed_output: Some(result.output.clone()),
            generation_source: GenerationSource::Failure {
                original_scenario_id: result.scenario_id.clone(),
            },
            confidence: 0.7,
            generation_reason: failure_reasons.join("; "),
            needs_review: self.config.require_human_approval,
        }))
    }

    /// Generate a reviewable test case from high uncertainty.
    fn generate_from_uncertainty(
        &self,
        result: &ScenarioResult,
        uncertainty: &UncertaintyAnalysis,
    ) -> Result<Option<GeneratedTestCase>> {
        Ok(Some(GeneratedTestCase {
            scenario_id: format!("{}_uncertainty_probe", result.scenario_id),
            query: format!(
                "{} (uncertainty probe)",
                result
                    .input
                    .clone()
                    .unwrap_or_else(|| "Unknown query".to_string())
            ),
            expected_output: None, // Uncertain cases need human-provided expected output
            observed_output: Some(result.output.clone()),
            generation_source: GenerationSource::Uncertainty {
                original_scenario_id: result.scenario_id.clone(),
            },
            confidence: 0.5,
            generation_reason: format!(
                "High uncertainty ({}): {}",
                uncertainty.overall_uncertainty,
                uncertainty.uncertainty_factors.join(", ")
            ),
            needs_review: true,
        }))
    }

    /// Analyze why a scenario failed
    fn analyze_failure(&self, result: &ScenarioResult) -> Vec<String> {
        let mut reasons = Vec::new();

        if result.quality_score.accuracy < 0.7 {
            reasons.push(format!(
                "Low accuracy: {:.2}",
                result.quality_score.accuracy
            ));
        }
        if result.quality_score.relevance < 0.7 {
            reasons.push(format!(
                "Low relevance: {:.2}",
                result.quality_score.relevance
            ));
        }
        if result.quality_score.completeness < 0.7 {
            reasons.push(format!(
                "Incomplete: {:.2}",
                result.quality_score.completeness
            ));
        }
        if result.quality_score.coherence < 0.7 {
            reasons.push(format!("Incoherent: {:.2}", result.quality_score.coherence));
        }

        if !result.validation.passed {
            reasons.push("Validation failed".to_string());
        }

        for issue in &result.quality_score.issues {
            reasons.push(format!("{}: {}", issue.dimension, issue.description));
        }

        if reasons.is_empty() {
            reasons.push("Unknown failure reason".to_string());
        }

        reasons
    }

    /// Analyze uncertainty in a scenario result
    fn analyze_uncertainty(&self, result: &ScenarioResult) -> Result<UncertaintyAnalysis> {
        let scores = [
            result.quality_score.accuracy,
            result.quality_score.relevance,
            result.quality_score.completeness,
            result.quality_score.coherence,
        ];

        let mean: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance: f64 =
            scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;

        let mut uncertainty_factors = Vec::new();

        if variance > 0.05 {
            uncertainty_factors.push(format!("High score variance: {variance:.3}"));
        }

        let threshold_proximity = (result.quality_score.overall - 0.80).abs();
        if threshold_proximity < 0.05 {
            uncertainty_factors.push(format!(
                "Near quality threshold: {:.3}",
                result.quality_score.overall
            ));
        }

        if result.quality_score.issues.len() >= 2 {
            uncertainty_factors.push(format!(
                "{} quality issues detected",
                result.quality_score.issues.len()
            ));
        }

        let overall_uncertainty =
            (variance + (uncertainty_factors.len() as f64 * 0.1)).clamp(0.0, 1.0);

        Ok(UncertaintyAnalysis {
            scenario_id: result.scenario_id.clone(),
            score_variance: variance,
            judge_confidence: None,
            historical_variance: None,
            overall_uncertainty,
            uncertainty_factors,
        })
    }

    /// Add human feedback for a scenario
    pub fn add_feedback(&mut self, feedback: HumanFeedback) -> Result<()> {
        self.feedback_store.add_feedback(feedback)
    }

    /// Get all feedback for a scenario
    #[must_use]
    pub fn get_feedback(&self, scenario_id: &str) -> Vec<&HumanFeedback> {
        self.feedback_store.get_feedback(scenario_id)
    }

    /// Save pending tests to disk
    fn save_pending_tests(&self, tests: &[GeneratedTestCase]) -> Result<()> {
        std::fs::create_dir_all(&self.config.pending_tests_dir)?;

        for test in tests {
            let filename = format!("{}.json", test.scenario_id);
            let path = self.config.pending_tests_dir.join(filename);
            let content = serde_json::to_string_pretty(test)?;
            std::fs::write(path, content)?;
        }

        Ok(())
    }

    /// Load pending tests from disk
    pub fn load_pending_tests(&self) -> Result<Vec<GeneratedTestCase>> {
        if !self.config.pending_tests_dir.exists() {
            return Ok(Vec::new());
        }

        let mut tests = Vec::new();

        for entry in std::fs::read_dir(&self.config.pending_tests_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip feedback.json - only load test case files
            if path.file_name().and_then(|n| n.to_str()) == Some("feedback.json") {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                // Gracefully skip files that don't parse as GeneratedTestCase
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(test) = serde_json::from_str::<GeneratedTestCase>(&content) {
                        tests.push(test);
                    }
                }
            }
        }

        Ok(tests)
    }

    /// Approve a pending test for inclusion in golden dataset
    pub fn approve_test(&self, test_id: &str) -> Result<()> {
        let filename = format!("{test_id}.json");
        let pending_path = self.config.pending_tests_dir.join(&filename);

        if !pending_path.exists() {
            anyhow::bail!("Pending test not found: {test_id}");
        }

        let approved_dir = self
            .config
            .pending_tests_dir
            .parent()
            .unwrap_or(Path::new("."))
            .join("approved_tests");
        std::fs::create_dir_all(&approved_dir)?;

        let approved_path = approved_dir.join(&filename);
        std::fs::rename(&pending_path, &approved_path)?;

        Ok(())
    }

    /// Reject a pending test
    pub fn reject_test(&self, test_id: &str, reason: &str) -> Result<()> {
        let filename = format!("{test_id}.json");
        let pending_path = self.config.pending_tests_dir.join(&filename);

        if !pending_path.exists() {
            anyhow::bail!("Pending test not found: {test_id}");
        }

        let rejected_dir = self
            .config
            .pending_tests_dir
            .parent()
            .unwrap_or(Path::new("."))
            .join("rejected_tests");
        std::fs::create_dir_all(&rejected_dir)?;

        let rejected_path = rejected_dir.join(&filename);
        std::fs::rename(&pending_path, &rejected_path)?;

        let reason_path = rejected_dir.join(format!("{test_id}_reason.txt"));
        std::fs::write(reason_path, reason)?;

        Ok(())
    }

    /// Promote a pending test to a golden scenario and save it to the golden dataset.
    ///
    /// This is the final step in the human-approval workflow:
    /// 1. Load the pending test by ID
    /// 2. Convert it to a `GoldenScenario` using human-provided input
    /// 3. Save the `GoldenScenario` to the specified golden dataset directory
    /// 4. Move the pending test to the approved directory
    ///
    /// # Arguments
    /// * `test_id` - ID of the pending test to promote
    /// * `input` - Human-provided fields for the golden scenario
    /// * `golden_dataset_dir` - Directory where golden scenarios are stored
    ///
    /// # Returns
    /// * `Ok(GoldenScenario)` - The newly created golden scenario
    /// * `Err(...)` - If the pending test doesn't exist or I/O fails
    ///
    /// # Example
    /// ```ignore
    /// let input = GoldenPromotionInput {
    ///     description: "Validates Tokio async runtime usage".to_string(),
    ///     expected_output_contains: vec!["tokio".to_string(), "async".to_string()],
    ///     quality_threshold: 0.90,
    ///     ..Default::default()
    /// };
    ///
    /// let golden = learning.promote_to_golden(
    ///     "test_001_failure_variant",
    ///     input,
    ///     "examples/apps/librarian/data/golden_dataset",
    /// )?;
    /// ```
    pub fn promote_to_golden<P: AsRef<Path>>(
        &self,
        test_id: &str,
        input: GoldenPromotionInput,
        golden_dataset_dir: P,
    ) -> Result<GoldenScenario> {
        // 1. Load the pending test
        let filename = format!("{test_id}.json");
        let pending_path = self.config.pending_tests_dir.join(&filename);

        if !pending_path.exists() {
            anyhow::bail!("Pending test not found: {test_id}");
        }

        let content = std::fs::read_to_string(&pending_path)?;
        let generated_test: GeneratedTestCase = serde_json::from_str(&content)?;

        // 2. Validate the input has required fields
        if input.description.trim().is_empty() {
            anyhow::bail!("Description is required for golden scenario promotion");
        }

        // 3. Convert to GoldenScenario
        let golden = generated_test.to_golden_scenario(input);

        // 4. Save to golden dataset directory
        let golden_dir = golden_dataset_dir.as_ref();
        std::fs::create_dir_all(golden_dir)?;

        let golden_path = golden_dir.join(&filename);
        let golden_content = serde_json::to_string_pretty(&golden)?;
        std::fs::write(&golden_path, golden_content)?;

        // 5. Move pending test to approved directory
        let approved_dir = self
            .config
            .pending_tests_dir
            .parent()
            .unwrap_or(Path::new("."))
            .join("approved_tests");
        std::fs::create_dir_all(&approved_dir)?;

        let approved_path = approved_dir.join(&filename);
        std::fs::rename(&pending_path, &approved_path)?;

        Ok(golden)
    }

    /// Generate statistics about continuous learning
    pub fn get_statistics(&self) -> Result<LearningStatistics> {
        let pending_tests = self.load_pending_tests()?;

        let total_feedback = self
            .feedback_store
            .feedback
            .values()
            .map(std::vec::Vec::len)
            .sum();

        let approved_count = if let Ok(entries) = std::fs::read_dir(
            self.config
                .pending_tests_dir
                .parent()
                .unwrap_or(Path::new("."))
                .join("approved_tests"),
        ) {
            entries.count()
        } else {
            0
        };

        let rejected_count = if let Ok(entries) = std::fs::read_dir(
            self.config
                .pending_tests_dir
                .parent()
                .unwrap_or(Path::new("."))
                .join("rejected_tests"),
        ) {
            entries.count()
        } else {
            0
        };

        Ok(LearningStatistics {
            pending_tests: pending_tests.len(),
            approved_tests: approved_count,
            rejected_tests: rejected_count,
            total_feedback,
            feedback_by_scenario: self.feedback_store.feedback.len(),
        })
    }
}

/// Statistics about continuous learning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStatistics {
    pub pending_tests: usize,
    pub approved_tests: usize,
    pub rejected_tests: usize,
    pub total_feedback: usize,
    pub feedback_by_scenario: usize,
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_learning_config_default() {
        let config = LearningConfig::default();
        assert!(config.auto_generate_from_failures);
        assert_eq!(config.failure_quality_threshold, 0.70);
        assert_eq!(config.max_new_tests_per_run, 10);
        assert!(config.require_human_approval);
        assert!(config.enable_active_learning);
    }

    #[test]
    fn test_uncertainty_analysis_high_variance() {
        let mut result = create_test_result();
        result.quality_score.accuracy = 0.95;
        result.quality_score.relevance = 0.50; // Low variance here
        result.quality_score.completeness = 0.90;
        result.quality_score.coherence = 0.85;

        let config = LearningConfig::default();
        let learning = ContinuousLearning::new(config);

        let uncertainty = learning.analyze_uncertainty(&result).unwrap();
        assert!(uncertainty.score_variance > 0.02); // High variance
    }

    #[test]
    fn test_generate_from_failure_preserves_observed_output_only() {
        let result = create_test_result();
        let config = LearningConfig::default();
        let learning = ContinuousLearning::new(config);

        let generated = learning.generate_from_failure(&result).unwrap().unwrap();
        assert!(generated.expected_output.is_none());
        assert_eq!(generated.observed_output.as_deref(), Some("Test output"));
    }

    #[test]
    fn test_feedback_store() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_feedback_cl.json");

        let _ = std::fs::remove_file(&db_path);

        let mut store = FeedbackStore::new(db_path.clone());

        let feedback = HumanFeedback {
            scenario_id: "test_001".to_string(),
            timestamp: "2025-11-16T12:00:00Z".to_string(),
            reviewer: "test_reviewer".to_string(),
            judge_correctness: JudgeCorrectness::Correct,
            human_quality_score: Some(0.95),
            reasoning: "Test reasoning".to_string(),
            approved_for_golden_dataset: true,
            suggested_modifications: None,
            tags: vec!["test".to_string()],
        };

        store.add_feedback(feedback).unwrap();

        let retrieved = store.get_feedback("test_001");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].reviewer, "test_reviewer");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_learning_statistics() {
        let temp_dir = std::env::temp_dir().join("learning_stats_test_cl");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let config = LearningConfig {
            pending_tests_dir: temp_dir.clone(),
            feedback_db_path: temp_dir.join("feedback.json"),
            ..Default::default()
        };

        let mut learning = ContinuousLearning::new(config);

        let feedback = HumanFeedback {
            scenario_id: "test_001".to_string(),
            timestamp: "2025-11-16T12:00:00Z".to_string(),
            reviewer: "test".to_string(),
            judge_correctness: JudgeCorrectness::Correct,
            human_quality_score: None,
            reasoning: "Test".to_string(),
            approved_for_golden_dataset: true,
            suggested_modifications: None,
            tags: vec![],
        };

        learning.add_feedback(feedback).unwrap();

        let stats = learning.get_statistics().unwrap();
        assert_eq!(stats.total_feedback, 1);
        assert_eq!(stats.feedback_by_scenario, 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_to_golden_scenario_conversion() {
        let generated = GeneratedTestCase {
            scenario_id: "test_failure_variant".to_string(),
            query: "How do I use async in Rust?".to_string(),
            expected_output: None,
            observed_output: Some("Some incorrect output".to_string()),
            generation_source: GenerationSource::Failure {
                original_scenario_id: "test".to_string(),
            },
            confidence: 0.7,
            generation_reason: "Low accuracy".to_string(),
            needs_review: true,
        };

        let input = GoldenPromotionInput {
            description: "Tests async/await usage in Rust".to_string(),
            expected_output_contains: vec!["async".to_string(), "await".to_string()],
            expected_output_not_contains: vec!["panic".to_string()],
            quality_threshold: 0.90,
            context: Some("First turn".to_string()),
            difficulty: Some(Difficulty::Simple),
            max_latency_ms: Some(5000),
            expected_tool_calls: vec!["search_docs".to_string()],
        };

        let golden = generated.to_golden_scenario(input);

        assert_eq!(golden.id, "test_failure_variant");
        assert_eq!(golden.query, "How do I use async in Rust?");
        assert_eq!(golden.description, "Tests async/await usage in Rust");
        assert_eq!(
            golden.expected_output_contains,
            vec!["async", "await"]
        );
        assert_eq!(golden.expected_output_not_contains, vec!["panic"]);
        assert_eq!(golden.quality_threshold, 0.90);
        assert_eq!(golden.context.as_deref(), Some("First turn"));
        assert_eq!(golden.difficulty, Some(Difficulty::Simple));
        assert_eq!(golden.max_latency_ms, Some(5000));
        assert_eq!(golden.expected_tool_calls, vec!["search_docs"]);
    }

    #[test]
    fn test_promote_to_golden_workflow() {
        let temp_dir = std::env::temp_dir().join("promote_golden_test_cl");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let pending_dir = temp_dir.join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        let golden_dir = temp_dir.join("golden_dataset");

        // Create a pending test file
        let test_case = GeneratedTestCase {
            scenario_id: "my_test_case".to_string(),
            query: "What is Rust?".to_string(),
            expected_output: None,
            observed_output: Some("Rust is a language".to_string()),
            generation_source: GenerationSource::Failure {
                original_scenario_id: "orig".to_string(),
            },
            confidence: 0.8,
            generation_reason: "Test".to_string(),
            needs_review: true,
        };
        let test_content = serde_json::to_string_pretty(&test_case).unwrap();
        std::fs::write(pending_dir.join("my_test_case.json"), &test_content).unwrap();

        let config = LearningConfig {
            pending_tests_dir: pending_dir.clone(),
            feedback_db_path: temp_dir.join("feedback.json"),
            ..Default::default()
        };

        let learning = ContinuousLearning::new(config);

        let input = GoldenPromotionInput {
            description: "Tests basic Rust knowledge".to_string(),
            expected_output_contains: vec!["systems programming".to_string()],
            quality_threshold: 0.85,
            ..Default::default()
        };

        let golden = learning
            .promote_to_golden("my_test_case", input, &golden_dir)
            .expect("Promotion should succeed");

        // Verify the golden scenario
        assert_eq!(golden.id, "my_test_case");
        assert_eq!(golden.description, "Tests basic Rust knowledge");
        assert_eq!(golden.query, "What is Rust?");

        // Verify golden file was created
        assert!(golden_dir.join("my_test_case.json").exists());

        // Verify pending test was moved to approved
        assert!(!pending_dir.join("my_test_case.json").exists());
        assert!(temp_dir.join("approved_tests/my_test_case.json").exists());

        // Verify the golden file can be parsed as GoldenScenario
        let golden_content =
            std::fs::read_to_string(golden_dir.join("my_test_case.json")).unwrap();
        let parsed: crate::golden_dataset::GoldenScenario =
            serde_json::from_str(&golden_content).unwrap();
        assert_eq!(parsed.id, "my_test_case");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_promote_to_golden_rejects_empty_description() {
        let temp_dir = std::env::temp_dir().join("promote_golden_empty_desc_cl");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let pending_dir = temp_dir.join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        let test_case = GeneratedTestCase {
            scenario_id: "empty_desc_test".to_string(),
            query: "Test query".to_string(),
            expected_output: None,
            observed_output: None,
            generation_source: GenerationSource::Failure {
                original_scenario_id: "orig".to_string(),
            },
            confidence: 0.5,
            generation_reason: "Test".to_string(),
            needs_review: true,
        };
        let test_content = serde_json::to_string_pretty(&test_case).unwrap();
        std::fs::write(pending_dir.join("empty_desc_test.json"), &test_content).unwrap();

        let config = LearningConfig {
            pending_tests_dir: pending_dir,
            feedback_db_path: temp_dir.join("feedback.json"),
            ..Default::default()
        };

        let learning = ContinuousLearning::new(config);

        let input = GoldenPromotionInput {
            description: "   ".to_string(), // Empty/whitespace description
            ..Default::default()
        };

        let result = learning.promote_to_golden(
            "empty_desc_test",
            input,
            temp_dir.join("golden_dataset"),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Description is required"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_promote_to_golden_missing_pending_test() {
        let temp_dir = std::env::temp_dir().join("promote_golden_missing_cl");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let pending_dir = temp_dir.join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        let config = LearningConfig {
            pending_tests_dir: pending_dir,
            feedback_db_path: temp_dir.join("feedback.json"),
            ..Default::default()
        };

        let learning = ContinuousLearning::new(config);

        let input = GoldenPromotionInput {
            description: "Valid description".to_string(),
            ..Default::default()
        };

        let result = learning.promote_to_golden(
            "nonexistent_test",
            input,
            temp_dir.join("golden_dataset"),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Pending test not found"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_golden_promotion_input_default() {
        let input = GoldenPromotionInput::default();
        assert!(input.description.is_empty());
        assert!(input.expected_output_contains.is_empty());
        assert!(input.expected_output_not_contains.is_empty());
        assert_eq!(input.quality_threshold, 0.85);
        assert!(input.context.is_none());
        assert!(input.difficulty.is_none());
        assert!(input.max_latency_ms.is_none());
        assert!(input.expected_tool_calls.is_empty());
    }

    // Helper to create a test ScenarioResult
    fn create_test_result() -> ScenarioResult {
        use crate::eval_runner::ValidationResult;
        use crate::quality_judge::QualityScore;

        ScenarioResult {
            scenario_id: "test_001".to_string(),
            passed: false,
            output: "Test output".to_string(),
            quality_score: QualityScore {
                accuracy: 0.80,
                relevance: 0.80,
                completeness: 0.80,
                safety: 1.0,
                coherence: 0.80,
                conciseness: 0.80,
                overall: 0.80,
                reasoning: "Test".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms: 1000,
            validation: ValidationResult {
                passed: false,
                missing_contains: vec![],
                forbidden_found: vec![],
                failure_reason: None,
            },
            error: None,
            retry_attempts: 0,
            timestamp: Utc::now(),
            input: Some("Test query".to_string()),
            tokens_used: None,
            cost_usd: None,
        }
    }
}
