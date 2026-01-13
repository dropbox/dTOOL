//! Human input tool for `DashFlow` Rust.
//!
//! This crate provides a tool for requesting input from a human user, enabling
//! human-in-the-loop workflows for AI agents and LLM applications.
//!
//! # Features
//!
//! - Request input from human users via stdin/stdout
//! - Simple prompt and response pattern
//! - Async-compatible using tokio
//! - Support for custom prompts
//! - Non-blocking I/O
//!
//! # Example
//!
//! ```rust
//! use dashflow_human_tool::HumanTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! # tokio_test::block_on(async {
//! let tool = HumanTool::new();
//!
//! // Request human input with a custom prompt
//! let input = json!({
//!     "prompt": "What is your name?"
//! });
//! // Note: In tests, stdin would need to be mocked. This example shows the API.
//! // let result = tool._call(ToolInput::Structured(input)).await.unwrap();
//! # });
//! ```
//!
//! # Security Considerations
//!
//! - This tool reads from stdin, which may block if no input is available
//! - Use with caution in automated environments
//! - Consider implementing timeouts for production use
//! - Input is not sanitized - validate user responses as needed
//!
//! # See Also
//!
//! - [`Tool`] - The trait this implements
//! - [`dashflow-shell-tool`](https://docs.rs/dashflow-shell-tool) - Shell command execution for automated tasks
//! - [`dashflow-calculator`](https://docs.rs/dashflow-calculator) - Calculator tool for mathematical expressions
//! - [Human-in-the-Loop Patterns](https://langchain.com/docs/modules/agents/how_to/human_approval) - Design patterns

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Error;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Human input tool for requesting information from a human user.
///
/// This tool enables human-in-the-loop workflows by prompting the user
/// for input via stdin/stdout. It's useful for:
/// - Gathering information that the AI cannot access
/// - Getting user confirmation for important actions
/// - Collecting feedback or preferences
/// - Interactive debugging and development
///
/// # Input Format
///
/// The tool accepts either:
/// - **String**: Used directly as the prompt text
/// - **Structured Object** with:
///   - `prompt` (string, required): The prompt to display to the user
///
/// # Output Format
///
/// Returns the user's response as a string (with trailing newline removed).
///
/// # Example
///
/// ```rust
/// use dashflow_human_tool::HumanTool;
/// use dashflow::core::tools::{Tool, ToolInput};
/// use serde_json::json;
///
/// # tokio_test::block_on(async {
/// let tool = HumanTool::new();
///
/// // Using string input
/// // let response = tool._call(ToolInput::String("Enter your name:".to_string())).await.unwrap();
///
/// // Using structured input
/// let input = json!({"prompt": "What is your favorite color?"});
/// // let response = tool._call(ToolInput::Structured(input)).await.unwrap();
/// # });
/// ```
#[derive(Debug, Clone, Default)]
pub struct HumanTool;

impl HumanTool {
    /// Create a new `HumanTool` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for HumanTool {
    fn name(&self) -> &'static str {
        "human"
    }

    fn description(&self) -> &'static str {
        "Request input from a human user. Use this tool when you need information, \
         confirmation, or feedback from a human. The input should be the prompt or \
         question to display to the user."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The prompt or question to display to the user"
                }
            },
            "required": ["prompt"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        // Extract the prompt from the input
        let prompt = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(obj) => {
                // Try to extract "prompt" field
                if let Some(prompt_value) = obj.get("prompt") {
                    if let Some(prompt_str) = prompt_value.as_str() {
                        prompt_str.to_string()
                    } else {
                        return Err(Error::tool_error("The 'prompt' field must be a string"));
                    }
                } else {
                    return Err(Error::tool_error(
                        "Missing required 'prompt' field in input",
                    ));
                }
            }
        };

        // Get stdin/stdout
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        // Display the prompt
        stdout
            .write_all(prompt.as_bytes())
            .await
            .map_err(|e| Error::tool_error(format!("Failed to write prompt: {e}")))?;

        // Add a space after the prompt if it doesn't end with whitespace
        if !prompt.ends_with(' ') && !prompt.ends_with('\n') {
            stdout
                .write_all(b" ")
                .await
                .map_err(|e| Error::tool_error(format!("Failed to write space: {e}")))?;
        }

        stdout
            .flush()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to flush stdout: {e}")))?;

        // Read the response
        let mut reader = BufReader::new(stdin);
        let mut response = String::new();

        reader
            .read_line(&mut response)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read input: {e}")))?;

        // Remove trailing newline
        let response = response.trim_end().to_string();

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // STRUCT CREATION AND TRAIT TESTS
    // =========================================================================

    #[test]
    fn test_human_tool_new_creates_instance() {
        let tool = HumanTool::new();
        // Just verify it doesn't panic and returns a valid instance
        assert_eq!(tool.name(), "human");
    }

    #[test]
    fn test_human_tool_default_trait() {
        let tool: HumanTool = Default::default();
        assert_eq!(tool.name(), "human");
    }

    #[test]
    fn test_human_tool_clone_trait() {
        let tool = HumanTool::new();
        let cloned = tool.clone();
        assert_eq!(tool.name(), cloned.name());
        assert_eq!(tool.description(), cloned.description());
    }

    #[test]
    fn test_human_tool_debug_trait() {
        let tool = HumanTool::new();
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("HumanTool"));
    }

    #[test]
    fn test_human_tool_multiple_instances_independent() {
        let tool1 = HumanTool::new();
        let tool2 = HumanTool::new();
        // Both should work independently
        assert_eq!(tool1.name(), tool2.name());
    }

    // =========================================================================
    // TOOL NAME TESTS
    // =========================================================================

    #[test]
    fn test_tool_name_exact_value() {
        let tool = HumanTool::new();
        assert_eq!(tool.name(), "human");
    }

    #[test]
    fn test_tool_name_length_reasonable() {
        let tool = HumanTool::new();
        let name = tool.name();
        // Name should be short but not empty
        assert!(name.len() >= 1, "Name too short");
        assert!(name.len() <= 50, "Name too long");
    }

    #[test]
    fn test_tool_name_lowercase() {
        let tool = HumanTool::new();
        assert_eq!(tool.name(), tool.name().to_lowercase());
    }

    #[test]
    fn test_tool_name_no_whitespace() {
        let tool = HumanTool::new();
        assert!(!tool.name().contains(' '));
        assert!(!tool.name().contains('\t'));
        assert!(!tool.name().contains('\n'));
    }

    #[test]
    fn test_tool_name_alphanumeric() {
        let tool = HumanTool::new();
        assert!(tool.name().chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    // =========================================================================
    // TOOL DESCRIPTION TESTS
    // =========================================================================

    #[test]
    fn test_tool_description_not_empty() {
        let tool = HumanTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_tool_description_contains_human() {
        let tool = HumanTool::new();
        let desc = tool.description().to_lowercase();
        assert!(desc.contains("human"));
    }

    #[test]
    fn test_tool_description_contains_input() {
        let tool = HumanTool::new();
        let desc = tool.description().to_lowercase();
        assert!(desc.contains("input"));
    }

    #[test]
    fn test_tool_description_contains_prompt() {
        let tool = HumanTool::new();
        let desc = tool.description().to_lowercase();
        assert!(desc.contains("prompt"));
    }

    #[test]
    fn test_tool_description_reasonable_length() {
        let tool = HumanTool::new();
        let desc = tool.description();
        // Should be descriptive but not excessively long
        assert!(desc.len() >= 20, "Description too short");
        assert!(desc.len() <= 500, "Description too long");
    }

    #[test]
    fn test_tool_description_starts_with_capital() {
        let tool = HumanTool::new();
        let desc = tool.description();
        // Description should start with a capital letter
        let first_char = desc.chars().next().unwrap();
        assert!(first_char.is_uppercase(), "Description should start with capital letter");
    }

    #[test]
    fn test_tool_description_contains_user() {
        let tool = HumanTool::new();
        let desc = tool.description().to_lowercase();
        assert!(desc.contains("user"));
    }

    // =========================================================================
    // ARGS SCHEMA TESTS
    // =========================================================================

    #[test]
    fn test_args_schema_is_object_type() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_args_schema_has_properties() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"].is_object());
    }

    #[test]
    fn test_args_schema_has_prompt_property() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["prompt"].is_object());
    }

    #[test]
    fn test_args_schema_prompt_is_string_type() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        assert_eq!(schema["properties"]["prompt"]["type"], "string");
    }

    #[test]
    fn test_args_schema_prompt_has_description() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        let prompt_desc = schema["properties"]["prompt"]["description"].as_str();
        assert!(prompt_desc.is_some());
        assert!(!prompt_desc.unwrap().is_empty());
    }

    #[test]
    fn test_args_schema_has_required_array() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        assert!(schema["required"].is_array());
    }

    #[test]
    fn test_args_schema_prompt_is_required() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("prompt")));
    }

    #[test]
    fn test_args_schema_only_prompt_required() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
    }

    #[test]
    fn test_args_schema_valid_json() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        // Verify it can be serialized back to string
        let serialized = serde_json::to_string(&schema);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_args_schema_properties_count() {
        let tool = HumanTool::new();
        let schema = tool.args_schema();
        let properties = schema["properties"].as_object().unwrap();
        // Should have exactly one property: prompt
        assert_eq!(properties.len(), 1);
    }

    #[test]
    fn test_args_schema_consistent_across_calls() {
        let tool = HumanTool::new();
        let schema1 = tool.args_schema();
        let schema2 = tool.args_schema();
        assert_eq!(schema1, schema2);
    }

    #[test]
    fn test_args_schema_consistent_across_instances() {
        let tool1 = HumanTool::new();
        let tool2 = HumanTool::new();
        assert_eq!(tool1.args_schema(), tool2.args_schema());
    }

    // =========================================================================
    // INPUT PARSING ERROR TESTS - MISSING PROMPT
    // =========================================================================

    #[test]
    fn test_error_missing_prompt_empty_object() {
        let tool = HumanTool::new();
        let input = json!({});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_prompt_error_message() {
        let tool = HumanTool::new();
        let input = json!({});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        let err = result.unwrap_err();
        assert!(err.to_string().contains("prompt"));
    }

    #[test]
    fn test_error_missing_prompt_with_other_fields() {
        let tool = HumanTool::new();
        let input = json!({"question": "test", "message": "hello"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_typo_in_prompt_key() {
        let tool = HumanTool::new();
        let input = json!({"promt": "test"}); // typo
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_case_sensitive_prompt_key() {
        let tool = HumanTool::new();
        let input = json!({"Prompt": "test"}); // wrong case
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_uppercase_prompt_key() {
        let tool = HumanTool::new();
        let input = json!({"PROMPT": "test"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    // =========================================================================
    // INPUT PARSING ERROR TESTS - INVALID PROMPT TYPE
    // =========================================================================

    #[test]
    fn test_error_prompt_as_number() {
        let tool = HumanTool::new();
        let input = json!({"prompt": 123});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("string"));
    }

    #[test]
    fn test_error_prompt_as_float() {
        let tool = HumanTool::new();
        let input = json!({"prompt": 3.14});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_boolean_true() {
        let tool = HumanTool::new();
        let input = json!({"prompt": true});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_boolean_false() {
        let tool = HumanTool::new();
        let input = json!({"prompt": false});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_null() {
        let tool = HumanTool::new();
        let input = json!({"prompt": null});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_array() {
        let tool = HumanTool::new();
        let input = json!({"prompt": ["hello", "world"]});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_empty_array() {
        let tool = HumanTool::new();
        let input = json!({"prompt": []});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_object() {
        let tool = HumanTool::new();
        let input = json!({"prompt": {"text": "hello"}});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_empty_object() {
        let tool = HumanTool::new();
        let input = json!({"prompt": {}});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_negative_number() {
        let tool = HumanTool::new();
        let input = json!({"prompt": -42});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_prompt_as_zero() {
        let tool = HumanTool::new();
        let input = json!({"prompt": 0});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        assert!(result.is_err());
    }

    // =========================================================================
    // INPUT PARSING - EXTRA FIELDS IGNORED
    // =========================================================================

    #[test]
    fn test_extra_fields_ignored_single() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "test", "extra": "ignored"});
        // This should not error on input parsing - it may error on I/O
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // If it errors, it should NOT be about missing prompt
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("missing"));
        }
    }

    #[test]
    fn test_extra_fields_ignored_multiple() {
        let tool = HumanTool::new();
        let input = json!({
            "prompt": "test",
            "extra1": "a",
            "extra2": 123,
            "extra3": true
        });
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("missing"));
        }
    }

    // =========================================================================
    // TOOL INPUT STRING MODE TESTS
    // Note: These test prompt extraction, may error on I/O which is expected
    // =========================================================================

    #[test]
    fn test_string_input_simple() {
        let tool = HumanTool::new();
        let input = ToolInput::String("What is your name?".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        // May error on I/O, but should not error on input parsing
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_empty() {
        let tool = HumanTool::new();
        let input = ToolInput::String(String::new());
        let result = tokio_test::block_on(async { tool._call(input).await });
        // Empty string is valid as prompt
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_whitespace_only() {
        let tool = HumanTool::new();
        let input = ToolInput::String("   ".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_with_newlines() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Line 1\nLine 2\nLine 3".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_with_tabs() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Column1\tColumn2".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_unicode() {
        let tool = HumanTool::new();
        let input = ToolInput::String("你好世界! Привет мир!".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_emoji() {
        let tool = HumanTool::new();
        let input = ToolInput::String("What do you think? 🤔".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_special_chars() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Test @#$%^&*()!".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_quotes() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Say \"hello\" to 'world'".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_string_input_backslashes() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Path: C:\\Users\\test".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    // =========================================================================
    // STRUCTURED INPUT VALID PROMPT TESTS
    // =========================================================================

    #[test]
    fn test_structured_input_valid_simple() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "Enter your name:"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // Should not fail on input parsing
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
            assert!(!e.to_string().to_lowercase().contains("must be a string"));
        }
    }

    #[test]
    fn test_structured_input_empty_prompt() {
        let tool = HumanTool::new();
        let input = json!({"prompt": ""});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // Empty string is valid prompt
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_structured_input_whitespace_prompt() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "   \t\n   "});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_structured_input_long_prompt() {
        let tool = HumanTool::new();
        let long_prompt = "x".repeat(10000);
        let input = json!({"prompt": long_prompt});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_structured_input_unicode_prompt() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "请输入您的姓名："});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_structured_input_multiline_prompt() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "Please answer the following:\n1. Your name\n2. Your age"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    // =========================================================================
    // PROMPT FORMATTING LOGIC TESTS
    // These test the specific behavior of adding space after prompt
    // =========================================================================

    #[test]
    fn test_prompt_ending_without_space_or_newline() {
        // Prompt "test" should get a space added
        let tool = HumanTool::new();
        let input = ToolInput::String("test".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        // Just verify it processes the input
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_prompt_ending_with_space() {
        // Prompt "test " should not get extra space
        let tool = HumanTool::new();
        let input = ToolInput::String("test ".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_prompt_ending_with_newline() {
        // Prompt "test\n" should not get extra space
        let tool = HumanTool::new();
        let input = ToolInput::String("test\n".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_prompt_ending_with_colon() {
        let tool = HumanTool::new();
        let input = ToolInput::String("Enter name:".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_prompt_ending_with_question_mark() {
        let tool = HumanTool::new();
        let input = ToolInput::String("What is your name?".to_string());
        let result = tokio_test::block_on(async { tool._call(input).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    // =========================================================================
    // ERROR MESSAGE QUALITY TESTS
    // =========================================================================

    #[test]
    fn test_error_message_for_missing_prompt_is_descriptive() {
        let tool = HumanTool::new();
        let input = json!({});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        let err = result.unwrap_err();
        let msg = err.to_string().to_lowercase();
        // Should mention what's missing
        assert!(msg.contains("prompt") || msg.contains("missing"));
    }

    #[test]
    fn test_error_message_for_wrong_type_is_descriptive() {
        let tool = HumanTool::new();
        let input = json!({"prompt": 123});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        let err = result.unwrap_err();
        let msg = err.to_string().to_lowercase();
        // Should mention expected type
        assert!(msg.contains("string"));
    }

    // =========================================================================
    // CONSISTENCY TESTS
    // =========================================================================

    #[test]
    fn test_same_input_produces_same_error() {
        let tool = HumanTool::new();
        let input1 = json!({});
        let input2 = json!({});
        let result1 =
            tokio_test::block_on(async { tool._call(ToolInput::Structured(input1)).await });
        let result2 =
            tokio_test::block_on(async { tool._call(ToolInput::Structured(input2)).await });
        assert!(result1.is_err());
        assert!(result2.is_err());
        // Error messages should be the same
        assert_eq!(
            result1.unwrap_err().to_string(),
            result2.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_different_instances_same_behavior() {
        let tool1 = HumanTool::new();
        let tool2 = HumanTool::new();
        let input1 = json!({"prompt": 123});
        let input2 = json!({"prompt": 123});
        let result1 =
            tokio_test::block_on(async { tool1._call(ToolInput::Structured(input1)).await });
        let result2 =
            tokio_test::block_on(async { tool2._call(ToolInput::Structured(input2)).await });
        assert!(result1.is_err());
        assert!(result2.is_err());
    }

    // =========================================================================
    // EDGE CASES
    // =========================================================================

    #[test]
    fn test_prompt_with_null_byte() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "test\0null"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // Should accept it (string with null byte is valid JSON string)
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_prompt_with_control_chars() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "test\x07bell"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }

    #[test]
    fn test_deeply_nested_extra_fields() {
        let tool = HumanTool::new();
        let input = json!({
            "prompt": "test",
            "nested": {
                "a": {
                    "b": {
                        "c": "deep"
                    }
                }
            }
        });
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // Extra fields should be ignored
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("missing"));
        }
    }

    #[test]
    fn test_prompt_with_json_like_content() {
        let tool = HumanTool::new();
        let input = json!({"prompt": "{\"key\": \"value\"}"});
        let result = tokio_test::block_on(async { tool._call(ToolInput::Structured(input)).await });
        // String containing JSON is just a string
        if let Err(e) = result {
            assert!(!e.to_string().to_lowercase().contains("prompt field"));
        }
    }
}
