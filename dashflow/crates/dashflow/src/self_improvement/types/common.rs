// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Common types shared across the self-improvement type system.
//!
//! This module contains fundamental types used by multiple other modules:
//! - `ModelIdentifier` - Identifies AI models by provider and name
//! - `Priority` - Task/gap priority levels
//! - `AnalysisDepth` - Depth of introspection analysis to perform

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =============================================================================
// ModelIdentifier - AI Model Identification
// =============================================================================

/// Model identifier for AI model tracking.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ModelIdentifier {
    /// An Anthropic model (e.g. Claude).
    Anthropic {
        /// Provider-specific model identifier (e.g. `claude-3-5-sonnet`).
        model: String,
    },
    /// An OpenAI model (e.g. GPT-4 class models).
    OpenAI {
        /// Provider-specific model identifier (e.g. `gpt-4o-mini`).
        model: String,
    },
    /// A Google model (e.g. Gemini).
    Google {
        /// Provider-specific model identifier (e.g. `gemini-1.5-pro`).
        model: String,
    },
    /// A model from another provider.
    Other {
        /// Provider name (e.g. `together`, `mistral`, `local`).
        provider: String,
        /// Provider-specific model identifier.
        model: String,
    },
}

// =============================================================================
// Priority - Task Priority Levels
// =============================================================================

/// Priority level for gaps, plans, and other actionable items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Priority {
    /// Highest priority: should be addressed next.
    High,
    /// Medium priority: important, but not immediately blocking.
    Medium,
    /// Lowest priority: backlog or opportunistic work.
    Low,
}

impl From<&str> for Priority {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" | "p0" | "critical" => Priority::High,
            "medium" | "p1" | "normal" => Priority::Medium,
            _ => Priority::Low,
        }
    }
}

impl From<String> for Priority {
    fn from(s: String) -> Self {
        Priority::from(s.as_str())
    }
}

impl From<u8> for Priority {
    fn from(n: u8) -> Self {
        match n {
            0..=1 => Priority::High,
            2..=3 => Priority::Medium,
            _ => Priority::Low,
        }
    }
}

// =============================================================================
// AnalysisDepth - Tiered Analysis Depth
// =============================================================================

/// Depth of analysis to perform during introspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AnalysisDepth {
    /// Per-execution: just collect metrics
    Metrics,
    /// Periodic: local analysis only
    LocalAnalysis,
    /// On-demand: full analysis with multi-model consensus
    DeepAnalysis,
}

impl From<&str> for AnalysisDepth {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "deep" | "deepanalysis" | "full" => AnalysisDepth::DeepAnalysis,
            "local" | "localanalysis" => AnalysisDepth::LocalAnalysis,
            _ => AnalysisDepth::Metrics,
        }
    }
}

impl From<String> for AnalysisDepth {
    fn from(s: String) -> Self {
        AnalysisDepth::from(s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_from_str() {
        assert_eq!(Priority::from("high"), Priority::High);
        assert_eq!(Priority::from("p0"), Priority::High);
        assert_eq!(Priority::from("critical"), Priority::High);
        assert_eq!(Priority::from("medium"), Priority::Medium);
        assert_eq!(Priority::from("p1"), Priority::Medium);
        assert_eq!(Priority::from("low"), Priority::Low);
        assert_eq!(Priority::from("anything"), Priority::Low);
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(Priority::from(0u8), Priority::High);
        assert_eq!(Priority::from(1u8), Priority::High);
        assert_eq!(Priority::from(2u8), Priority::Medium);
        assert_eq!(Priority::from(3u8), Priority::Medium);
        assert_eq!(Priority::from(4u8), Priority::Low);
    }

    #[test]
    fn test_analysis_depth_from_str() {
        assert_eq!(AnalysisDepth::from("deep"), AnalysisDepth::DeepAnalysis);
        assert_eq!(AnalysisDepth::from("full"), AnalysisDepth::DeepAnalysis);
        assert_eq!(AnalysisDepth::from("local"), AnalysisDepth::LocalAnalysis);
        assert_eq!(AnalysisDepth::from("metrics"), AnalysisDepth::Metrics);
        assert_eq!(AnalysisDepth::from("anything"), AnalysisDepth::Metrics);
    }
}
