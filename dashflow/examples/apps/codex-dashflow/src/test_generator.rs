//! Test generation from code

use anyhow::Result;
use dashflow::core::language_models::{ChatModel, ChatModelBuildExt};
use dashflow::core::messages::{HumanMessage, Message};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Test style for generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestStyle {
    /// Unit tests (isolated function tests)
    #[default]
    Unit,
    /// Integration tests (tests across modules)
    Integration,
    /// Property-based tests (randomized inputs)
    Property,
}

impl std::str::FromStr for TestStyle {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "unit" => Ok(Self::Unit),
            "integration" => Ok(Self::Integration),
            "property" => Ok(Self::Property),
            _ => Err(anyhow::anyhow!("Invalid test style: {}", s)),
        }
    }
}

/// Generated test output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTests {
    /// The generated test code
    pub code: String,
    /// Number of test functions generated
    pub test_count: usize,
    /// Brief description of what's being tested
    pub description: String,
}

/// Generates unit tests for code
pub struct TestGenerator {
    model: Arc<dyn ChatModel>,
}

impl TestGenerator {
    /// Create a new test generator
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }

    /// Generate tests for the given code
    pub async fn generate(&self, code: &str, style: TestStyle) -> Result<GeneratedTests> {
        let style_instruction = match style {
            TestStyle::Unit => {
                "Generate comprehensive unit tests that:\n\
                - Test each public function in isolation\n\
                - Cover edge cases (empty inputs, boundary values)\n\
                - Test error handling\n\
                - Use descriptive test names"
            }
            TestStyle::Integration => {
                "Generate integration tests that:\n\
                - Test interactions between components\n\
                - Test realistic usage scenarios\n\
                - Verify end-to-end behavior\n\
                - Use setup/teardown when appropriate"
            }
            TestStyle::Property => {
                "Generate property-based tests that:\n\
                - Define invariants that should always hold\n\
                - Use randomized inputs to find edge cases\n\
                - Test algebraic properties (e.g., inverse operations)\n\
                - Use proptest or quickcheck patterns"
            }
        };

        let system_prompt = format!(
            r#"You are an expert Rust developer who writes thorough, idiomatic tests.

{}

Output ONLY the test code in a #[cfg(test)] module. Include all necessary imports.
Make tests complete and runnable."#,
            style_instruction
        );

        let human_msg = format!("Generate tests for this code:\n\n```rust\n{}\n```", code);
        let base_messages: Vec<dashflow::core::messages::BaseMessage> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        // Use the builder pattern with automatic telemetry
        let result = self.model.build_generate(&base_messages)
            .await
            .map_err(|e| anyhow::anyhow!("Test generation failed: {}", e))?;

        let test_code = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Count test functions
        let test_count = test_code.matches("#[test]").count();

        Ok(GeneratedTests {
            code: test_code,
            test_count,
            description: format!("Generated {} {:?} test(s)", test_count, style),
        })
    }

    /// Generate tests for a specific function
    pub async fn generate_for_function(
        &self,
        code: &str,
        function_name: &str,
        style: TestStyle,
    ) -> Result<GeneratedTests> {
        let style_instruction = match style {
            TestStyle::Unit => "unit tests",
            TestStyle::Integration => "integration tests",
            TestStyle::Property => "property-based tests",
        };

        let system_prompt = format!(
            r#"You are an expert Rust developer who writes thorough, idiomatic tests.

Generate {} specifically for the function `{}`.

Focus on:
- Normal operation with typical inputs
- Edge cases and boundary conditions
- Error conditions
- Return value verification

Output ONLY the test code in a #[cfg(test)] module. Include all necessary imports."#,
            style_instruction, function_name
        );

        let human_msg = format!(
            "Generate tests for the `{}` function in this code:\n\n```rust\n{}\n```",
            function_name, code
        );
        let base_messages: Vec<dashflow::core::messages::BaseMessage> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        // Use the builder pattern with automatic telemetry
        let result = self.model.build_generate(&base_messages)
            .await
            .map_err(|e| anyhow::anyhow!("Test generation failed: {}", e))?;

        let test_code = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        let test_count = test_code.matches("#[test]").count();

        Ok(GeneratedTests {
            code: test_code,
            test_count,
            description: format!(
                "Generated {} {} for `{}`",
                test_count, style_instruction, function_name
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_style_parsing() {
        assert_eq!("unit".parse::<TestStyle>().unwrap(), TestStyle::Unit);
        assert_eq!(
            "integration".parse::<TestStyle>().unwrap(),
            TestStyle::Integration
        );
        assert_eq!(
            "property".parse::<TestStyle>().unwrap(),
            TestStyle::Property
        );
    }
}
