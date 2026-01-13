// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Multi-model consensus types for AI review and validation.
//!
//! This module provides types for gathering and synthesizing reviews
//! from multiple AI models to validate improvement proposals.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::ModelIdentifier;

// =============================================================================
// ConsensusResult - Aggregated Multi-Model Review
// =============================================================================

/// Results from consulting multiple AI models.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsensusResult {
    /// Reviews from different models
    pub reviews: Vec<ModelReview>,

    /// Aggregated consensus score
    pub consensus_score: f64,

    /// Points of agreement
    pub agreements: Vec<String>,

    /// Points of disagreement
    pub disagreements: Vec<Disagreement>,

    /// Final synthesized judgment
    pub synthesis: String,

    /// Whether the original proposals were validated
    pub validated: bool,

    /// Modifications suggested by consensus
    pub modifications: Vec<String>,
}

impl ConsensusResult {
    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("### Reviewers\n\n");
        for review in &self.reviews {
            md.push_str(&format!(
                "- {:?}: {:?} (confidence: {:.0}%)\n",
                review.model,
                review.assessment,
                review.confidence * 100.0
            ));
        }

        md.push_str(&format!(
            "\n**Consensus Score:** {:.2}\n\n",
            self.consensus_score
        ));

        if !self.agreements.is_empty() {
            md.push_str("### Agreements\n\n");
            for a in &self.agreements {
                md.push_str(&format!("- {a}\n"));
            }
            md.push('\n');
        }

        if !self.disagreements.is_empty() {
            md.push_str("### Disagreements\n\n");
            for d in &self.disagreements {
                md.push_str(&format!(
                    "- **{}**: {} vs {}\n",
                    d.topic, d.position_a, d.position_b
                ));
            }
            md.push('\n');
        }

        md.push_str(&format!("### Synthesis\n\n{}\n\n", self.synthesis));
        md.push_str(&format!(
            "**Validated:** {}\n",
            if self.validated { "Yes" } else { "No" }
        ));

        md
    }
}

// =============================================================================
// ModelReview - Single Model's Review
// =============================================================================

/// A review from a single model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelReview {
    /// Which model provided this review
    pub model: ModelIdentifier,

    /// Overall assessment
    pub assessment: Assessment,

    /// Specific critiques
    pub critiques: Vec<Critique>,

    /// Suggestions for improvement
    pub suggestions: Vec<String>,

    /// Confidence in review
    pub confidence: f64,

    /// Raw response for transparency
    pub raw_response: String,
}

// =============================================================================
// Assessment - Review Assessment Level
// =============================================================================

/// Assessment level for model reviews.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Assessment {
    /// Strong approval with minimal reservations.
    StronglyAgree,
    /// Approval, possibly with minor concerns.
    Agree,
    /// Neither approval nor rejection (unclear or balanced tradeoffs).
    Neutral,
    /// Rejection, with substantive concerns.
    Disagree,
    /// Strong rejection (likely blocking issues).
    StronglyDisagree,
}

impl From<&str> for Assessment {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "stronglyagree" | "strongly_agree" | "strong_yes" => Assessment::StronglyAgree,
            "agree" | "yes" | "approve" => Assessment::Agree,
            "disagree" | "no" | "reject" => Assessment::Disagree,
            "stronglydisagree" | "strongly_disagree" | "strong_no" => Assessment::StronglyDisagree,
            _ => Assessment::Neutral,
        }
    }
}

impl From<String> for Assessment {
    fn from(s: String) -> Self {
        Assessment::from(s.as_str())
    }
}

// =============================================================================
// Critique - Specific Critique
// =============================================================================

/// A specific critique from a model review.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Critique {
    /// What is being critiqued
    pub target: String,

    /// The critique
    pub criticism: String,

    /// Severity
    pub severity: CritiqueSeverity,

    /// Suggested fix
    pub suggested_fix: Option<String>,
}

// =============================================================================
// CritiqueSeverity - Severity of Critique
// =============================================================================

/// Severity of a critique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CritiqueSeverity {
    /// Minor issue; does not materially affect correctness.
    Minor,
    /// Moderate issue; may affect quality or maintainability.
    Moderate,
    /// Major issue; likely affects correctness or feasibility.
    Major,
    /// Critical issue; blocking until addressed.
    Critical,
}

impl From<&str> for CritiqueSeverity {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" | "blocking" | "high" => CritiqueSeverity::Critical,
            "major" | "important" => CritiqueSeverity::Major,
            "moderate" | "medium" => CritiqueSeverity::Moderate,
            _ => CritiqueSeverity::Minor,
        }
    }
}

impl From<String> for CritiqueSeverity {
    fn from(s: String) -> Self {
        CritiqueSeverity::from(s.as_str())
    }
}

// =============================================================================
// Disagreement - Disagreement Between Models
// =============================================================================

/// A disagreement between models.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Disagreement {
    /// Short description of what the models disagree about.
    pub topic: String,
    /// Summary of one position.
    pub position_a: String,
    /// Summary of the opposing position.
    pub position_b: String,
    /// Models that support `position_a`.
    pub models_a: Vec<ModelIdentifier>,
    /// Models that support `position_b`.
    pub models_b: Vec<ModelIdentifier>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assessment_from_str() {
        assert_eq!(
            Assessment::from("strongly_agree"),
            Assessment::StronglyAgree
        );
        assert_eq!(Assessment::from("agree"), Assessment::Agree);
        assert_eq!(Assessment::from("yes"), Assessment::Agree);
        assert_eq!(Assessment::from("disagree"), Assessment::Disagree);
        assert_eq!(Assessment::from("no"), Assessment::Disagree);
        assert_eq!(Assessment::from("neutral"), Assessment::Neutral);
        assert_eq!(Assessment::from("unknown"), Assessment::Neutral);
    }

    #[test]
    fn test_critique_severity_from_str() {
        assert_eq!(
            CritiqueSeverity::from("critical"),
            CritiqueSeverity::Critical
        );
        assert_eq!(CritiqueSeverity::from("major"), CritiqueSeverity::Major);
        assert_eq!(
            CritiqueSeverity::from("moderate"),
            CritiqueSeverity::Moderate
        );
        assert_eq!(CritiqueSeverity::from("minor"), CritiqueSeverity::Minor);
        assert_eq!(CritiqueSeverity::from("unknown"), CritiqueSeverity::Minor);
    }

    #[test]
    fn test_consensus_result_to_markdown() {
        let consensus = ConsensusResult {
            reviews: vec![ModelReview {
                model: ModelIdentifier::Anthropic {
                    model: "claude-3".to_string(),
                },
                assessment: Assessment::Agree,
                critiques: vec![],
                suggestions: vec![],
                confidence: 0.85,
                raw_response: String::new(),
            }],
            consensus_score: 0.85,
            agreements: vec!["Implementation looks good".to_string()],
            disagreements: vec![],
            synthesis: "Proposal validated by consensus".to_string(),
            validated: true,
            modifications: vec![],
        };

        let md = consensus.to_markdown();
        assert!(md.contains("Reviewers"));
        assert!(md.contains("Consensus Score"));
        assert!(md.contains("**Validated:** Yes"));
    }
}
