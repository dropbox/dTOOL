//! Documentation generation from code

use anyhow::Result;
use dashflow::core::language_models::{ChatModel, ChatModelBuildExt};
use dashflow::core::messages::{HumanMessage, Message};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Documentation style for generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocsStyle {
    /// Rustdoc-style documentation (/// comments)
    #[default]
    Rustdoc,
    /// Python-style docstrings ("""...""")
    Docstring,
    /// TypeScript/JavaScript JSDoc comments (/** ... */)
    Jsdoc,
    /// Markdown documentation file
    Markdown,
}

impl std::str::FromStr for DocsStyle {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "rustdoc" => Ok(Self::Rustdoc),
            "docstring" => Ok(Self::Docstring),
            "jsdoc" => Ok(Self::Jsdoc),
            "markdown" | "md" => Ok(Self::Markdown),
            _ => Err(anyhow::anyhow!("Invalid docs style: {}", s)),
        }
    }
}

/// Generated documentation output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedDocs {
    /// The generated documentation
    pub content: String,
    /// Style of documentation
    pub style: String,
    /// Whether examples were included
    pub has_examples: bool,
}

/// Generates documentation for code
pub struct DocsGenerator {
    model: Arc<dyn ChatModel>,
}

impl DocsGenerator {
    /// Create a new documentation generator
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }

    /// Generate documentation for the given code
    pub async fn generate(
        &self,
        code: &str,
        style: DocsStyle,
        with_examples: bool,
    ) -> Result<GeneratedDocs> {
        let style_instruction = match style {
            DocsStyle::Rustdoc => {
                r#"Generate Rust documentation using:
- /// for item documentation
- //! for module documentation
- # sections for Examples, Panics, Errors, Safety
- [`link`] syntax for cross-references
- Code examples in ```rust blocks"#
            }
            DocsStyle::Docstring => {
                r#"Generate Python-style docstrings using:
- Triple quotes """..."""
- Args section for parameters
- Returns section for return values
- Raises section for exceptions
- Examples section with >>> prompts"#
            }
            DocsStyle::Jsdoc => {
                r#"Generate JSDoc documentation using:
- /** ... */ blocks for functions, methods, classes, and exported constants
- @param and @returns tags where applicable
- @throws for error conditions
- @example tags with realistic usage
- Avoid redundant type annotations if TypeScript types already exist"#
            }
            DocsStyle::Markdown => {
                r#"Generate Markdown documentation with:
- # headers for sections
- Code blocks with language tags
- Tables for parameters
- Lists for features
- Links to related items"#
            }
        };

        let examples_instruction = if with_examples {
            "\n\nInclude practical code examples that demonstrate:\n\
            - Basic usage\n\
            - Common patterns\n\
            - Edge case handling"
        } else {
            ""
        };

        let system_prompt = format!(
            r#"You are an expert technical writer who creates clear, comprehensive documentation.

{}{}

Output ONLY the documentation. For Rustdoc, include the code with documentation added.
For Markdown, output a complete .md file."#,
            style_instruction, examples_instruction
        );

        let human_msg = format!("Generate documentation for this code:\n\n```\n{}\n```", code);
        let base_messages: Vec<dashflow::core::messages::BaseMessage> = vec![
            Message::system(system_prompt.as_str()),
            HumanMessage::new(human_msg.as_str()).into(),
        ];

        // Use the builder pattern with automatic telemetry
        let result = self.model.build_generate(&base_messages)
            .await
            .map_err(|e| anyhow::anyhow!("Documentation generation failed: {}", e))?;

        let content = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        let style_name = match style {
            DocsStyle::Rustdoc => "rustdoc",
            DocsStyle::Docstring => "docstring",
            DocsStyle::Jsdoc => "jsdoc",
            DocsStyle::Markdown => "markdown",
        };

        Ok(GeneratedDocs {
            content,
            style: style_name.to_string(),
            has_examples: with_examples,
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
        assert_eq!("rustdoc".parse::<DocsStyle>().unwrap(), DocsStyle::Rustdoc);
        assert_eq!(
            "docstring".parse::<DocsStyle>().unwrap(),
            DocsStyle::Docstring
        );
        assert_eq!("jsdoc".parse::<DocsStyle>().unwrap(), DocsStyle::Jsdoc);
        assert_eq!(
            "markdown".parse::<DocsStyle>().unwrap(),
            DocsStyle::Markdown
        );
        assert_eq!("md".parse::<DocsStyle>().unwrap(), DocsStyle::Markdown);
    }
}
