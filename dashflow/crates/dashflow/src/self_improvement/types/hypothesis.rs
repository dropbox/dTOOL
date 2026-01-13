// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Hypothesis tracking types for self-improvement predictions.
//!
//! This module provides types for creating and tracking hypotheses about
//! expected outcomes from implementing improvement plans.

use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::citations::Citation;

// =============================================================================
// HypothesisSource - Source of Hypothesis
// =============================================================================

/// Source of a hypothesis for tracking accuracy by category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum HypothesisSource {
    /// Created from a capability gap analysis
    CapabilityGap,
    /// Created from an execution plan
    ExecutionPlan,
    /// Created from a deprecation recommendation
    Deprecation,
    /// Created manually
    #[default]
    Manual,
}

impl std::fmt::Display for HypothesisSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityGap => write!(f, "Capability Gap"),
            Self::ExecutionPlan => write!(f, "Execution Plan"),
            Self::Deprecation => write!(f, "Deprecation"),
            Self::Manual => write!(f, "Manual"),
        }
    }
}

// =============================================================================
// Hypothesis - Prediction About Future Outcomes
// =============================================================================

/// A hypothesis about future outcomes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hypothesis {
    /// Unique identifier
    pub id: Uuid,

    /// The hypothesis statement
    pub statement: String,

    /// Reasoning behind the hypothesis
    pub reasoning: String,

    /// What data would validate this hypothesis
    pub expected_evidence: Vec<ExpectedEvidence>,

    /// When to evaluate this hypothesis
    pub evaluation_trigger: EvaluationTrigger,

    /// Current status
    pub status: HypothesisStatus,

    /// If evaluated, the outcome
    pub outcome: Option<HypothesisOutcome>,

    /// Lessons learned from this hypothesis
    pub lessons: Vec<String>,

    /// Source of this hypothesis for accuracy tracking
    #[serde(default)]
    pub source: HypothesisSource,

    /// When the hypothesis was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl Hypothesis {
    /// Create a new hypothesis
    #[must_use]
    pub fn new(statement: impl Into<String>, reasoning: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            statement: statement.into(),
            reasoning: reasoning.into(),
            expected_evidence: Vec::new(),
            evaluation_trigger: EvaluationTrigger::Manual,
            status: HypothesisStatus::Active,
            outcome: None,
            lessons: Vec::new(),
            source: HypothesisSource::Manual,
            created_at: Utc::now(),
        }
    }

    /// Set the source of this hypothesis
    #[must_use]
    pub fn with_source(mut self, source: HypothesisSource) -> Self {
        self.source = source;
        self
    }

    /// Add expected evidence
    #[must_use]
    pub fn with_expected_evidence(mut self, evidence: Vec<ExpectedEvidence>) -> Self {
        self.expected_evidence = evidence;
        self
    }

    /// Set evaluation trigger
    #[must_use]
    pub fn with_trigger(mut self, trigger: EvaluationTrigger) -> Self {
        self.evaluation_trigger = trigger;
        self
    }

    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("**Statement:** {}\n\n", self.statement));
        md.push_str(&format!("**Reasoning:** {}\n\n", self.reasoning));

        if !self.expected_evidence.is_empty() {
            md.push_str("**Expected Evidence:**\n");
            for e in &self.expected_evidence {
                md.push_str(&format!(
                    "- {}: {} (measure: {})\n",
                    e.metric, e.expected_value, e.measurement_method
                ));
            }
            md.push('\n');
        }

        md.push_str(&format!(
            "**Evaluation Trigger:** {:?}\n\n",
            self.evaluation_trigger
        ));
        md.push_str(&format!("**Status:** {:?}\n", self.status));

        if let Some(outcome) = &self.outcome {
            md.push_str(&format!(
                "\n**Outcome:** {}\n",
                if outcome.correct {
                    "CORRECT"
                } else {
                    "INCORRECT"
                }
            ));
            md.push_str(&format!("**Analysis:** {}\n", outcome.analysis));
        }

        md
    }
}

// =============================================================================
// ExpectedEvidence - Evidence for Hypothesis Validation
// =============================================================================

/// Expected evidence for hypothesis validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExpectedEvidence {
    /// What metric or observation
    pub metric: String,

    /// Expected value or range
    pub expected_value: String,

    /// How to measure
    pub measurement_method: String,
}

impl ExpectedEvidence {
    /// Create new expected evidence
    #[must_use]
    pub fn new(
        metric: impl Into<String>,
        expected_value: impl Into<String>,
        measurement_method: impl Into<String>,
    ) -> Self {
        Self {
            metric: metric.into(),
            expected_value: expected_value.into(),
            measurement_method: measurement_method.into(),
        }
    }
}

// =============================================================================
// EvaluationTrigger - When to Evaluate Hypothesis
// =============================================================================

/// When to evaluate a hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum EvaluationTrigger {
    /// After N executions
    AfterExecutions(usize),
    /// After time period
    AfterDuration(Duration),
    /// After specific plan is implemented
    AfterPlan(Uuid),
    /// Manual trigger
    Manual,
}

// =============================================================================
// HypothesisStatus - Current Status
// =============================================================================

/// Status of a hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum HypothesisStatus {
    /// Actively tracked and awaiting evaluation.
    Active,
    /// Created but awaiting a prerequisite before it becomes active.
    Pending {
        /// Human-readable description of what the hypothesis is waiting on.
        waiting_for: String,
    },
    /// Evaluated with an outcome recorded.
    Evaluated,
    /// Superseded by a newer hypothesis.
    Superseded {
        /// Identifier of the hypothesis that replaced this one.
        by: Uuid,
    },
}

// =============================================================================
// HypothesisOutcome - Evaluation Result
// =============================================================================

/// Outcome of hypothesis evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HypothesisOutcome {
    /// Was the hypothesis correct?
    pub correct: bool,

    /// Actual observed evidence
    pub observed_evidence: Vec<ObservedEvidence>,

    /// Analysis of why correct/incorrect
    pub analysis: String,

    /// What to do differently next time
    pub improvements_for_future: Vec<String>,
}

// =============================================================================
// ObservedEvidence - Evidence Observed During Evaluation
// =============================================================================

/// Evidence observed during hypothesis evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservedEvidence {
    /// What was measured
    pub metric: String,

    /// What was observed
    pub observed_value: String,

    /// Matches expectation?
    pub matches_expected: bool,

    /// Citation to source data
    pub citation: Citation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypothesis_creation() {
        let hypothesis = Hypothesis::new(
            "Retry rate will drop below 6%",
            "Sentiment tool will reduce sentiment-related retries",
        )
        .with_expected_evidence(vec![ExpectedEvidence::new(
            "retry_rate",
            "< 6%",
            "Measure retry rate over 50 executions",
        )])
        .with_trigger(EvaluationTrigger::AfterExecutions(50));

        assert!(matches!(hypothesis.status, HypothesisStatus::Active));
        assert!(hypothesis.outcome.is_none());
        assert_eq!(hypothesis.expected_evidence.len(), 1);
    }

    #[test]
    fn test_hypothesis_source_display() {
        assert_eq!(
            HypothesisSource::CapabilityGap.to_string(),
            "Capability Gap"
        );
        assert_eq!(
            HypothesisSource::ExecutionPlan.to_string(),
            "Execution Plan"
        );
        assert_eq!(HypothesisSource::Deprecation.to_string(), "Deprecation");
        assert_eq!(HypothesisSource::Manual.to_string(), "Manual");
    }

    #[test]
    fn test_hypothesis_to_markdown() {
        let hypothesis =
            Hypothesis::new("Test hypothesis", "Test reasoning").with_expected_evidence(vec![
                ExpectedEvidence::new("test_metric", "expected_value", "measurement"),
            ]);

        let md = hypothesis.to_markdown();
        assert!(md.contains("Statement:"));
        assert!(md.contains("Reasoning:"));
        assert!(md.contains("Expected Evidence:"));
    }
}
