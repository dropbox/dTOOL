//! Code explanation and understanding

use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::{HumanMessage, Message};
use dashflow::generate;
use std::sync::Arc;

/// Detail level for code explanations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailLevel {
    /// Brief one-line summary
    Brief,
    /// Normal explanation with key points
    #[default]
    Normal,
    /// Detailed line-by-line analysis
    Detailed,
}

impl std::str::FromStr for DetailLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "brief" => Ok(Self::Brief),
            "normal" => Ok(Self::Normal),
            "detailed" => Ok(Self::Detailed),
            _ => Err(anyhow::anyhow!("Invalid detail level: {}", s)),
        }
    }
}

/// Explains code in natural language
pub struct CodeExplainer {
    model: Arc<dyn ChatModel>,
}

impl CodeExplainer {
    /// Create a new code explainer
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }

    /// Explain a piece of code
    pub async fn explain(&self, code: &str, detail: DetailLevel) -> Result<String> {
        let detail_instruction = match detail {
            DetailLevel::Brief => "Provide a one-sentence summary of what this code does.",
            DetailLevel::Normal => {
                "Explain what this code does, covering the main logic and any important details."
            }
            DetailLevel::Detailed => {
                "Provide a detailed line-by-line explanation of this code, including the purpose of each section, algorithms used, and any edge cases handled."
            }
        };

        let system_prompt = format!(
            r#"You are an expert programmer who explains code clearly.

{}

Be concise but thorough. Use plain English that a junior developer could understand."#,
            detail_instruction
        );

        let human_msg = format!("Explain this code:\n\n```\n{}\n```", code);
        let messages: Vec<Message> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        let result = generate(Arc::clone(&self.model), &messages)
            .await
            .map_err(|e| anyhow::anyhow!("Code explanation failed: {}", e))?;

        let explanation = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        Ok(explanation)
    }

    /// Explain a specific function or symbol in a file
    pub async fn explain_symbol(&self, code: &str, symbol: &str) -> Result<String> {
        let system_prompt = r#"You are an expert programmer who explains code clearly.

Focus on explaining the specified symbol (function, struct, etc.) and its role in the codebase.
Explain:
1. What it does
2. Its inputs and outputs
3. How it's typically used
4. Any important implementation details"#;

        let human_msg = format!(
            "In this code, explain the `{}` symbol:\n\n```\n{}\n```",
            symbol, code
        );
        let messages: Vec<Message> = vec![
            Message::system(system_prompt),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        let result = generate(Arc::clone(&self.model), &messages)
            .await
            .map_err(|e| anyhow::anyhow!("Symbol explanation failed: {}", e))?;

        let explanation = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        Ok(explanation)
    }
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_detail_level_parsing() {
        assert_eq!("brief".parse::<DetailLevel>().unwrap(), DetailLevel::Brief);
        assert_eq!(
            "normal".parse::<DetailLevel>().unwrap(),
            DetailLevel::Normal
        );
        assert_eq!(
            "detailed".parse::<DetailLevel>().unwrap(),
            DetailLevel::Detailed
        );
    }
}
