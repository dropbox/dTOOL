//! Code generation from natural language

use crate::config::CodexConfig;
use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::{HumanMessage, Message};
use dashflow::generate;
use std::sync::Arc;

/// Generates code from natural language descriptions
pub struct CodeGenerator {
    model: Arc<dyn ChatModel>,
    config: CodexConfig,
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new(model: Arc<dyn ChatModel>, config: CodexConfig) -> Self {
        Self { model, config }
    }

    /// Generate code from a natural language description
    pub async fn generate(&self, description: &str) -> Result<String> {
        let system_prompt = format!(
            r#"You are an expert {} programmer. Generate clean, idiomatic code based on the user's description.

Guidelines:
- Follow {} conventions and best practices
- Include type annotations where appropriate
- Add meaningful comments explaining non-obvious logic
- Handle errors appropriately
- Make the code production-ready

Output ONLY the code, no explanations."#,
            self.config.default_language, self.config.default_language
        );

        let messages: Vec<Message> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(description).into(),
        ];

        let result = generate(Arc::clone(&self.model), &messages)
            .await
            .map_err(|e| anyhow::anyhow!("Code generation failed: {}", e))?;

        let code = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        Ok(code)
    }

    /// Generate code with unit tests
    pub async fn generate_with_tests(&self, description: &str) -> Result<String> {
        let system_prompt = format!(
            r#"You are an expert {} programmer. Generate clean, idiomatic code based on the user's description.

Guidelines:
- Follow {} conventions and best practices
- Include type annotations where appropriate
- Add meaningful comments explaining non-obvious logic
- Handle errors appropriately
- Include comprehensive unit tests

Output the main code followed by the test module."#,
            self.config.default_language, self.config.default_language
        );

        let messages: Vec<Message> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(description).into(),
        ];

        let result = generate(Arc::clone(&self.model), &messages)
            .await
            .map_err(|e| anyhow::anyhow!("Code generation failed: {}", e))?;

        let code = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_config() {
        let config = CodexConfig::for_rust();
        assert_eq!(config.default_language, "rust");
    }
}
