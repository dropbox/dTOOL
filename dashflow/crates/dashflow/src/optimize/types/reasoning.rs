// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Reasoning type for o1-series models with native reasoning capabilities

use serde::{Deserialize, Serialize};

/// Reasoning effort level for o1-series models
///
/// Controls how much computation the model spends on internal reasoning
/// before producing a response. Higher effort typically produces better
/// results for complex tasks but costs more tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Minimal reasoning - fastest but may miss nuance
    Low,
    /// Balanced reasoning - good for most tasks
    #[default]
    Medium,
    /// Deep reasoning - best for complex problems
    High,
}

impl ReasoningEffort {
    /// Get string representation for API calls
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Get approximate token multiplier for cost estimation
    pub fn token_multiplier(&self) -> f64 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 2.0,
            Self::High => 4.0,
        }
    }
}

impl std::fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Native reasoning configuration for o1-series models
///
/// Enables explicit control over the model's reasoning process,
/// including effort level and output preferences.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::{Reasoning, ReasoningEffort};
///
/// let reasoning = Reasoning::new()
///     .with_effort(ReasoningEffort::High)
///     .with_summary(true);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reasoning {
    /// Reasoning effort level
    pub effort: ReasoningEffort,

    /// Whether to include reasoning summary in output
    #[serde(default)]
    pub include_summary: bool,

    /// Whether to include full reasoning trace
    #[serde(default)]
    pub include_trace: bool,

    /// Maximum reasoning tokens (None = no limit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_reasoning_tokens: Option<u32>,

    /// Optional reasoning hint/guidance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Reasoning {
    /// Create new reasoning configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific effort level
    #[must_use]
    pub fn with_effort(mut self, effort: ReasoningEffort) -> Self {
        self.effort = effort;
        self
    }

    /// Enable reasoning summary in output
    #[must_use]
    pub fn with_summary(mut self, include: bool) -> Self {
        self.include_summary = include;
        self
    }

    /// Enable full reasoning trace in output
    #[must_use]
    pub fn with_trace(mut self, include: bool) -> Self {
        self.include_trace = include;
        self
    }

    /// Set maximum reasoning tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_reasoning_tokens = Some(max);
        self
    }

    /// Set reasoning hint/guidance
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Create low-effort reasoning
    pub fn low() -> Self {
        Self::new().with_effort(ReasoningEffort::Low)
    }

    /// Create medium-effort reasoning (default)
    pub fn medium() -> Self {
        Self::new().with_effort(ReasoningEffort::Medium)
    }

    /// Create high-effort reasoning
    pub fn high() -> Self {
        Self::new().with_effort(ReasoningEffort::High)
    }

    /// Convert to API parameters
    pub fn to_api_params(&self) -> serde_json::Value {
        let mut params = serde_json::json!({
            "reasoning_effort": self.effort.as_str()
        });

        if let Some(max) = self.max_reasoning_tokens {
            params["max_reasoning_tokens"] = serde_json::Value::Number(max.into());
        }

        params
    }
}

/// Reasoning output from o1-series models
///
/// Contains the model's reasoning trace and final answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningOutput {
    /// The final answer/response
    pub answer: String,

    /// Optional reasoning summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    /// Full reasoning trace (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<ReasoningStep>>,

    /// Tokens used for reasoning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,

    /// Total tokens used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

impl ReasoningOutput {
    /// Create output with just an answer
    pub fn new(answer: impl Into<String>) -> Self {
        Self {
            answer: answer.into(),
            summary: None,
            trace: None,
            reasoning_tokens: None,
            total_tokens: None,
        }
    }

    /// Add reasoning summary
    #[must_use]
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Add reasoning trace
    #[must_use]
    pub fn with_trace(mut self, trace: Vec<ReasoningStep>) -> Self {
        self.trace = Some(trace);
        self
    }

    /// Add token counts
    #[must_use]
    pub fn with_tokens(mut self, reasoning: u32, total: u32) -> Self {
        self.reasoning_tokens = Some(reasoning);
        self.total_tokens = Some(total);
        self
    }

    /// Get number of reasoning steps
    pub fn step_count(&self) -> usize {
        self.trace.as_ref().map(|t| t.len()).unwrap_or(0)
    }
}

/// A single step in the reasoning trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Step number
    pub step: usize,

    /// Thinking/reasoning at this step
    pub thinking: String,

    /// Optional conclusion from this step
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,

    /// Confidence in this step (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

impl ReasoningStep {
    /// Create a new reasoning step
    pub fn new(step: usize, thinking: impl Into<String>) -> Self {
        Self {
            step,
            thinking: thinking.into(),
            conclusion: None,
            confidence: None,
        }
    }

    /// Add conclusion
    #[must_use]
    pub fn with_conclusion(mut self, conclusion: impl Into<String>) -> Self {
        self.conclusion = Some(conclusion.into());
        self
    }

    /// Add confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }
}

impl std::fmt::Display for ReasoningStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Step {}: {}", self.step, self.thinking)?;
        if let Some(conclusion) = &self.conclusion {
            write!(f, " -> {}", conclusion)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_effort_levels() {
        assert_eq!(ReasoningEffort::Low.as_str(), "low");
        assert_eq!(ReasoningEffort::Medium.as_str(), "medium");
        assert_eq!(ReasoningEffort::High.as_str(), "high");

        assert_eq!(ReasoningEffort::Low.token_multiplier(), 1.0);
        assert_eq!(ReasoningEffort::High.token_multiplier(), 4.0);
    }

    #[test]
    fn test_reasoning_builders() {
        let r = Reasoning::new()
            .with_effort(ReasoningEffort::High)
            .with_summary(true)
            .with_trace(true)
            .with_max_tokens(1000)
            .with_hint("Think step by step");

        assert_eq!(r.effort, ReasoningEffort::High);
        assert!(r.include_summary);
        assert!(r.include_trace);
        assert_eq!(r.max_reasoning_tokens, Some(1000));
        assert_eq!(r.hint, Some("Think step by step".to_string()));
    }

    #[test]
    fn test_reasoning_shortcuts() {
        let low = Reasoning::low();
        assert_eq!(low.effort, ReasoningEffort::Low);

        let high = Reasoning::high();
        assert_eq!(high.effort, ReasoningEffort::High);
    }

    #[test]
    fn test_reasoning_to_api_params() {
        let r = Reasoning::high().with_max_tokens(500);
        let params = r.to_api_params();

        assert_eq!(params["reasoning_effort"], "high");
        assert_eq!(params["max_reasoning_tokens"], 500);
    }

    #[test]
    fn test_reasoning_output() {
        let output = ReasoningOutput::new("The answer is 42")
            .with_summary("Calculated based on universal constants")
            .with_tokens(150, 200);

        assert_eq!(output.answer, "The answer is 42");
        assert!(output.summary.is_some());
        assert_eq!(output.reasoning_tokens, Some(150));
    }

    #[test]
    fn test_reasoning_step() {
        let step = ReasoningStep::new(1, "First, consider the constraints")
            .with_conclusion("Constraint A is limiting")
            .with_confidence(0.85);

        assert_eq!(step.step, 1);
        assert!(step.conclusion.is_some());
        assert_eq!(step.confidence, Some(0.85));
    }

    #[test]
    fn test_reasoning_output_with_trace() {
        let trace = vec![
            ReasoningStep::new(1, "Step one"),
            ReasoningStep::new(2, "Step two"),
        ];

        let output = ReasoningOutput::new("Final answer").with_trace(trace);
        assert_eq!(output.step_count(), 2);
    }

    #[test]
    fn test_serialization() {
        let r = Reasoning::high().with_summary(true);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("high"));

        let deserialized: Reasoning = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.effort, ReasoningEffort::High);
    }
}
