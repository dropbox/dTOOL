//! Code refactoring suggestions

use anyhow::Result;
use dashflow::core::language_models::{ChatModel, ChatModelBuildExt};
use dashflow::core::messages::{HumanMessage, Message};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Focus area for refactoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefactorFocus {
    /// Improve code performance
    Performance,
    /// Improve code readability
    Readability,
    /// Improve code safety (error handling, type safety)
    Safety,
    /// All areas
    #[default]
    All,
}

impl std::str::FromStr for RefactorFocus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "performance" => Ok(Self::Performance),
            "readability" => Ok(Self::Readability),
            "safety" => Ok(Self::Safety),
            "all" => Ok(Self::All),
            _ => Err(anyhow::anyhow!("Invalid refactor focus: {}", s)),
        }
    }
}

/// A single refactoring suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSuggestion {
    /// Description of the suggestion
    pub description: String,
    /// Category (performance, readability, safety)
    pub category: String,
    /// Priority (high, medium, low)
    pub priority: String,
    /// Original code snippet
    pub original: Option<String>,
    /// Suggested replacement
    pub suggested: Option<String>,
    /// Line numbers affected (if applicable)
    pub lines: Option<(usize, usize)>,
}

/// Suggests refactoring improvements
pub struct RefactorSuggester {
    model: Arc<dyn ChatModel>,
}

impl RefactorSuggester {
    /// Create a new refactor suggester
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }

    /// Analyze code and suggest refactoring improvements
    pub async fn suggest(&self, code: &str, focus: RefactorFocus) -> Result<Vec<RefactorSuggestion>> {
        let focus_instruction = match focus {
            RefactorFocus::Performance => {
                "Focus on performance improvements: algorithm efficiency, unnecessary allocations, caching opportunities."
            }
            RefactorFocus::Readability => {
                "Focus on readability: naming, structure, comments, reducing complexity."
            }
            RefactorFocus::Safety => {
                "Focus on safety: error handling, type safety, edge cases, potential panics."
            }
            RefactorFocus::All => {
                "Analyze all aspects: performance, readability, and safety."
            }
        };

        let system_prompt = format!(
            r#"You are an expert code reviewer who suggests practical refactoring improvements.

{}

For each suggestion, provide:
1. A clear description
2. The category (performance/readability/safety)
3. Priority (high/medium/low)
4. The specific code to change (if applicable)
5. The suggested improvement

Format your response as a numbered list of suggestions."#,
            focus_instruction
        );

        let human_msg = format!("Review this code for refactoring:\n\n```\n{}\n```", code);
        let base_messages: Vec<dashflow::core::messages::BaseMessage> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        // Use the builder pattern with automatic telemetry
        let result = self.model.build_generate(&base_messages)
            .await
            .map_err(|e| anyhow::anyhow!("Refactor analysis failed: {}", e))?;

        let response = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Parse response into suggestions
        // For now, return a simple parsed version
        let suggestions = self.parse_suggestions(&response);
        Ok(suggestions)
    }

    fn parse_suggestions(&self, response: &str) -> Vec<RefactorSuggestion> {
        // Simple parsing - in production this would be more sophisticated
        let mut suggestions = Vec::new();

        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                suggestions.push(RefactorSuggestion {
                    description: trimmed.to_string(),
                    category: "general".to_string(),
                    priority: "medium".to_string(),
                    original: None,
                    suggested: None,
                    lines: None,
                });
            }
        }

        suggestions
    }

    /// Apply a refactoring suggestion and return the modified code
    pub async fn apply_suggestion(
        &self,
        code: &str,
        suggestion: &RefactorSuggestion,
    ) -> Result<String> {
        let system_prompt = r#"You are an expert programmer who applies refactoring changes.

Apply the suggested change to the code. Return ONLY the modified code, no explanations."#;

        let human_msg = format!(
            "Apply this refactoring:\n{}\n\nTo this code:\n```\n{}\n```",
            suggestion.description, code
        );
        let base_messages: Vec<dashflow::core::messages::BaseMessage> = vec![
            Message::system(system_prompt),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        // Use the builder pattern with automatic telemetry
        let result = self.model.build_generate(&base_messages)
            .await
            .map_err(|e| anyhow::anyhow!("Refactor application failed: {}", e))?;

        let modified = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        Ok(modified)
    }
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_focus_parsing() {
        assert_eq!(
            "performance".parse::<RefactorFocus>().unwrap(),
            RefactorFocus::Performance
        );
        assert_eq!("all".parse::<RefactorFocus>().unwrap(), RefactorFocus::All);
    }

    #[test]
    fn test_suggestion_serialization() {
        let suggestion = RefactorSuggestion {
            description: "Use collect() instead of push loop".to_string(),
            category: "readability".to_string(),
            priority: "medium".to_string(),
            original: Some("for x in iter { vec.push(x) }".to_string()),
            suggested: Some("iter.collect()".to_string()),
            lines: Some((10, 12)),
        };

        let json = serde_json::to_string(&suggestion).unwrap();
        assert!(json.contains("collect"));
    }
}
