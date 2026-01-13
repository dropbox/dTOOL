//! Structured output support for chat models
//!
//! This module provides utilities for extracting structured, type-safe outputs
//! from LLM responses using JSON schemas.
//!
//! # Overview
//!
//! The `with_structured_output()` method allows you to configure a chat model
//! to return responses that conform to a specific Rust type. The model receives
//! a JSON schema describing the expected output format and is instructed to
//! return valid JSON matching that schema.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::structured::ChatModelStructuredExt;
//! use dashflow_openai::ChatOpenAI;
//! use serde::{Serialize, Deserialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct GradeHallucinations {
//!     binary_score: bool,
//!     reasoning: String,
//! }
//!
//! let llm = ChatOpenAI::with_config(Default::default())
//!     .with_model("gpt-4")
//!     .with_structured_output::<GradeHallucinations>()?;
//!
//! let result: GradeHallucinations = llm.invoke(messages).await?;
//! if result.binary_score {
//!     println!("No hallucinations detected");
//! }
//! ```
//!
//! # Design
//!
//! The structured output system consists of:
//!
//! - `ChatModelStructuredExt`: Extension trait adding `with_structured_output<T>()` to all `ChatModel` implementations
//! - `StructuredChatModel<T>`: Wrapper that adds structured output parsing to any `ChatModel`
//! - Response parsing logic with JSON extraction from markdown code blocks

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::core::callbacks::CallbackManager;
use crate::core::error::{Error, Result};
use crate::core::language_models::{ChatModel, ChatResult, ToolChoice, ToolDefinition};
use crate::core::messages::BaseMessage;
use crate::core::schema::json_schema::json_schema;

/// Extension trait adding structured output support to `ChatModel`.
///
/// This trait is automatically implemented for all types that implement `ChatModel`,
/// providing the `with_structured_output<T>()` method.
pub trait ChatModelStructuredExt: ChatModel {
    /// Configure this chat model to return structured outputs of type `T`.
    ///
    /// Returns a `StructuredChatModel<T>` that wraps this model and automatically
    /// parses responses into the specified type. The type must implement:
    /// - `serde::Deserialize` for JSON parsing
    /// - `schemars::JsonSchema` for schema generation
    ///
    /// # Type Parameters
    ///
    /// * `T` - The output type to parse responses into
    ///
    /// # Returns
    ///
    /// A `StructuredChatModel<T>` wrapper, or an error if schema generation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// #[derive(Serialize, Deserialize, JsonSchema)]
    /// struct Answer {
    ///     answer: String,
    ///     confidence: f64,
    /// }
    ///
    /// let structured_llm = llm.with_structured_output::<Answer>()?;
    /// let result: Answer = structured_llm.invoke(messages).await?;
    /// ```
    fn with_structured_output<T>(self) -> Result<StructuredChatModel<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
        Self: Sized + 'static;
}

/// Blanket implementation of `ChatModelStructuredExt` for all `ChatModel` implementations.
impl<M: ChatModel + Sized + 'static> ChatModelStructuredExt for M {
    fn with_structured_output<T>(self) -> Result<StructuredChatModel<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
    {
        StructuredChatModel::new(self)
    }
}

/// A chat model wrapper that parses responses into structured outputs.
///
/// This struct wraps any `ChatModel` and adds automatic JSON parsing of
/// responses into a specified Rust type `T`. It generates a JSON schema
/// for the type and includes it in requests to guide the model's output.
///
/// # Type Parameters
///
/// * `T` - The output type to parse responses into. Must implement `Deserialize` and `JsonSchema`.
///
/// # Example
///
/// ```rust,ignore
/// // Typically created via with_structured_output() extension method
/// let structured_llm = StructuredChatModel::new(base_llm)?;
/// let result: T = structured_llm.invoke(messages).await?;
/// ```
pub struct StructuredChatModel<T> {
    /// The underlying chat model
    inner: Arc<dyn ChatModel>,

    /// JSON schema for the output type
    schema: serde_json::Value,

    /// Phantom data to track the output type
    _phantom: PhantomData<T>,
}

impl<T> StructuredChatModel<T>
where
    T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
{
    /// Create a new `StructuredChatModel` wrapping the given `ChatModel`.
    ///
    /// Generates a JSON schema for type `T` and stores it for use in requests.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model to wrap
    ///
    /// # Returns
    ///
    /// A new `StructuredChatModel<T>`, or an error if schema generation fails
    pub fn new<M: ChatModel + 'static>(model: M) -> Result<Self> {
        let schema = json_schema::<T>().map_err(|e| {
            Error::other(format!(
                "Failed to generate JSON schema for structured output: {e}"
            ))
        })?;
        Ok(Self {
            inner: Arc::new(model),
            schema,
            _phantom: PhantomData,
        })
    }

    /// Create a new `StructuredChatModel` from an existing `Arc<dyn ChatModel>`.
    ///
    /// This is useful when you have a provider-agnostic chat model from
    /// `llm_factory::create_llm()` and want to add structured output parsing.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying chat model wrapped in Arc
    ///
    /// # Returns
    ///
    /// A new `StructuredChatModel<T>`, or an error if schema generation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use common::llm_factory::{create_llm, LLMRequirements};
    /// use dashflow::core::language_models::structured::StructuredChatModel;
    ///
    /// let llm = create_llm(LLMRequirements::default()).await?;
    /// let structured_llm: StructuredChatModel<MyOutput> =
    ///     StructuredChatModel::from_arc(llm)?;
    /// ```
    pub fn from_arc(model: Arc<dyn ChatModel>) -> Result<Self> {
        let schema = json_schema::<T>().map_err(|e| {
            Error::other(format!(
                "Failed to generate JSON schema for structured output: {e}"
            ))
        })?;
        Ok(Self {
            inner: model,
            schema,
            _phantom: PhantomData,
        })
    }

    /// Get a reference to the JSON schema for this structured output.
    ///
    /// The schema is generated once during construction and reused for all requests.
    #[must_use]
    pub fn schema(&self) -> &serde_json::Value {
        &self.schema
    }

    /// Get a reference to the underlying chat model.
    #[must_use]
    pub fn inner(&self) -> &dyn ChatModel {
        &*self.inner
    }

    /// Parse a chat result into structured output of type T.
    ///
    /// Extracts JSON from the response content, handles markdown code blocks,
    /// and deserializes into the target type.
    ///
    /// # Arguments
    ///
    /// * `result` - The chat result to parse
    ///
    /// # Returns
    ///
    /// The parsed value of type T, or an error if parsing fails
    pub fn parse_result(&self, result: &ChatResult) -> Result<T> {
        // Get the first generation's message content
        let generation = result
            .generations
            .first()
            .ok_or_else(|| Error::OutputParsing("No generations in response".to_string()))?;

        let content = generation.message.content().as_text();

        // Extract and parse JSON
        extract_and_parse_json(&content)
    }

    /// Invoke the model with structured output.
    ///
    /// This is a convenience method that combines `generate()` and `parse_result()`.
    /// It calls the underlying model, parses the response, and returns the structured output.
    ///
    /// # Arguments
    ///
    /// * `messages` - The messages to send to the model
    ///
    /// # Returns
    ///
    /// The parsed structured output of type T
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result: Answer = structured_llm.invoke(&messages).await?;
    /// println!("Answer: {}", result.answer);
    /// ```
    pub async fn invoke(&self, messages: &[BaseMessage]) -> Result<T> {
        let chat_result = self
            ._generate(messages, None, None, None, None)
            .await
            .map_err(|e| Error::other(format!("Structured output LLM generation failed: {e}")))?;
        self.parse_result(&chat_result)
    }
}

// ============================================================================
// Response Parsing Functions
// ============================================================================

/// Extract JSON from a string that may contain markdown code blocks or plain JSON.
///
/// Handles various common LLM response formats:
/// - Plain JSON: `{"key": "value"}`
/// - Markdown JSON block: ```json\n{"key": "value"}\n```
/// - Markdown block without language: ```\n{"key": "value"}\n```
/// - JSON with surrounding text (extracts first {...} or [...])
///
/// # Arguments
///
/// * `content` - The text content to extract JSON from
///
/// # Returns
///
/// The extracted JSON string (without markdown delimiters)
///
/// # Examples
///
/// ```rust
/// use dashflow::core::language_models::structured::extract_json;
///
/// // Plain JSON
/// let json = extract_json(r#"{"key": "value"}"#).unwrap();
/// assert_eq!(json, r#"{"key": "value"}"#);
///
/// // Markdown code block
/// let json = extract_json("```json\n{\"key\": \"value\"}\n```").unwrap();
/// assert_eq!(json, r#"{"key": "value"}"#);
/// ```
pub fn extract_json(content: &str) -> Result<String> {
    let content = content.trim();

    // Case 1: Markdown code block with language specifier (```json ... ```)
    if content.contains("```json") {
        if let Some(start) = content.find("```json") {
            let after_start = &content[start + 7..]; // Skip past ```json
            if let Some(end) = after_start.find("```") {
                return Ok(after_start[..end].trim().to_string());
            }
        }
    }

    // Case 2: Markdown code block without language (``` ... ```)
    if let Some(without_start) = content.strip_prefix("```") {
        if let Some(end) = without_start.find("```") {
            return Ok(without_start[..end].trim().to_string());
        }
    }

    // Case 3: JSON object or array with surrounding text
    // Find whichever bracket comes first
    let obj_pos = content.find('{');
    let arr_pos = content.find('[');

    match (obj_pos, arr_pos) {
        (Some(obj), Some(arr)) => {
            // Both found - use whichever comes first
            if obj < arr {
                if let Some(json_str) = extract_balanced_json(&content[obj..], '{', '}') {
                    return Ok(json_str);
                }
            } else if let Some(json_str) = extract_balanced_json(&content[arr..], '[', ']') {
                return Ok(json_str);
            }
        }
        (Some(obj), None) => {
            if let Some(json_str) = extract_balanced_json(&content[obj..], '{', '}') {
                return Ok(json_str);
            }
        }
        (None, Some(arr)) => {
            if let Some(json_str) = extract_balanced_json(&content[arr..], '[', ']') {
                return Ok(json_str);
            }
        }
        (None, None) => {
            // No brackets found, fall through to error
        }
    }

    // Case 4: Content is already JSON (no markdown, no surrounding text)
    // This should have been caught by Case 3, but keep as fallback
    if content.starts_with('{') || content.starts_with('[') {
        return Ok(content.to_string());
    }

    // Failed to extract JSON
    Err(Error::OutputParsing(format!(
        "Could not extract JSON from response. Content: {}",
        if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.to_string()
        }
    )))
}

/// Extract balanced JSON from a string starting with an opening bracket.
///
/// Handles nested brackets correctly.
fn extract_balanced_json(content: &str, open: char, close: char) -> Option<String> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in content.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            c if c == open && !in_string => depth += 1,
            c if c == close && !in_string => {
                depth -= 1;
                if depth == 0 {
                    // i is the byte index of the closing bracket
                    // We want to include it, so add its UTF-8 length
                    let end_index = i + ch.len_utf8();
                    return Some(content[..end_index].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// Extract JSON from content and parse it into type T.
///
/// Combines JSON extraction and deserialization with comprehensive error handling.
///
/// # Arguments
///
/// * `content` - The text content to extract and parse JSON from
///
/// # Returns
///
/// The parsed value of type T, or an error with details about what went wrong
fn extract_and_parse_json<T: DeserializeOwned>(content: &str) -> Result<T> {
    // Extract JSON
    let json_str = extract_json(content)?;

    // Parse JSON into T
    serde_json::from_str(&json_str).map_err(|e| {
        Error::OutputParsing(format!(
            "Failed to deserialize JSON into target type: {}. JSON: {}",
            e,
            if json_str.len() > 200 {
                format!("{}...", &json_str[..200])
            } else {
                json_str
            }
        ))
    })
}

#[async_trait]
impl<T> ChatModel for StructuredChatModel<T>
where
    T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
{
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        // Step 4: Implement structured output for generic ChatModel
        // Strategy:
        // 1. Prepend system message with JSON schema and instructions
        // 2. Call underlying model
        // 3. Parse response as JSON
        // 4. Validate against schema (implicit via deserialization)

        // Create system message with JSON schema
        let schema_json =
            serde_json::to_string_pretty(&self.schema).unwrap_or_else(|_| self.schema.to_string());

        let system_content = format!(
            "You are a helpful assistant that responds in JSON format.\n\
             Your response must be valid JSON that conforms to this schema:\n\n\
             ```json\n{schema_json}\n```\n\n\
             Respond with ONLY the JSON object, no additional text or explanation."
        );

        // Prepend system message to existing messages
        use crate::core::messages::Message;
        let mut new_messages: Vec<BaseMessage> = vec![Message::system(system_content.clone())];

        // Check if first message is already a system message - if so, merge
        let mut messages_to_add = messages;
        if let Some(Message::System { content, .. }) = messages.first() {
            // Replace our system message with merged content
            let merged_content = format!("{}\n\n{}", system_content, content.as_text());
            new_messages[0] = Message::system(merged_content);
            // Skip the first message since we merged it
            messages_to_add = &messages[1..];
        }

        new_messages.extend_from_slice(messages_to_add);

        // Call underlying model with modified messages
        let result = self
            .inner
            ._generate(&new_messages, stop, tools, tool_choice, run_manager)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Underlying chat model generation failed for structured output: {e}"
                ))
            })?;

        // Response is already in ChatResult format, just return it
        // The user will call parse_result() if they want the structured output
        // Or we could parse here and store in generation_info... but that changes the API
        Ok(result)
    }

    fn llm_type(&self) -> &str {
        self.inner.llm_type()
    }

    fn identifying_params(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut params = self.inner.identifying_params();
        params.insert("structured_output".to_string(), serde_json::json!(true));
        params.insert("output_schema".to_string(), self.schema.clone());
        params
    }

    fn rate_limiter(&self) -> Option<std::sync::Arc<dyn crate::core::rate_limiters::RateLimiter>> {
        self.inner.rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_and_parse_json, ChatModelStructuredExt, StructuredChatModel};
    use crate::core::language_models::{ChatGeneration, ChatResult};
    use crate::core::messages::{AIMessage, HumanMessage};
    use crate::test_prelude::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    // Mock ChatModel for testing
    struct MockChatModel {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(self.response.clone()).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq)]
    struct TestOutput {
        answer: String,
        confidence: f64,
    }

    #[tokio::test]
    async fn test_structured_chat_model_creation() {
        let mock = MockChatModel {
            response: r#"{"answer": "42", "confidence": 0.95}"#.to_string(),
        };

        let structured = mock.with_structured_output::<TestOutput>();
        assert!(structured.is_ok());

        let structured = structured.unwrap();
        assert!(structured.schema().is_object());
    }

    #[tokio::test]
    async fn test_structured_chat_model_schema() {
        let mock = MockChatModel {
            response: "test".to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let schema = structured.schema();

        // Verify schema has expected structure
        assert!(schema.is_object());
        let schema_obj = schema.as_object().unwrap();
        assert!(schema_obj.contains_key("properties"));

        let properties = schema_obj.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("answer"));
        assert!(properties.contains_key("confidence"));
    }

    #[tokio::test]
    async fn test_structured_chat_model_llm_type() {
        let mock = MockChatModel {
            response: "test".to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        assert_eq!(structured.llm_type(), "mock");
    }

    #[tokio::test]
    async fn test_structured_chat_model_identifying_params() {
        let mock = MockChatModel {
            response: "test".to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let params = structured.identifying_params();

        assert!(params.contains_key("structured_output"));
        assert!(params.contains_key("output_schema"));
        assert_eq!(
            params.get("structured_output").unwrap(),
            &serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn test_structured_chat_model_generate_passthrough() {
        let mock = MockChatModel {
            response: "test response".to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();

        let messages = vec![HumanMessage::new("test").into()];
        let result = structured.generate(&messages, None, None, None, None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(
            result.generations[0].message.content().as_text(),
            "test response"
        );
    }

    #[test]
    fn test_extension_trait_available() {
        // Verify that the extension trait is available for MockChatModel
        let mock = MockChatModel {
            response: "test".to_string(),
        };

        // This compiles if the extension trait is properly implemented
        let _structured = mock.with_structured_output::<TestOutput>();
    }

    #[test]
    fn test_nested_struct_schema() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Address {
            street: String,
            city: String,
        }

        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Person {
            name: String,
            address: Address,
        }

        let mock = MockChatModel {
            response: "test".to_string(),
        };

        let structured = StructuredChatModel::<Person>::new(mock).unwrap();
        let schema = structured.schema();

        // Verify nested schema is generated
        assert!(schema.is_object());
        let properties = schema
            .as_object()
            .unwrap()
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("address"));
    }

    // ========================================================================
    // Parsing Function Tests
    // ========================================================================

    #[test]
    fn test_extract_json_plain() {
        let content = r#"{"answer": "42", "confidence": 0.95}"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"answer": "42", "confidence": 0.95}"#);
    }

    #[test]
    fn test_extract_json_markdown_with_language() {
        let content = r#"```json
{"answer": "42", "confidence": 0.95}
```"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"answer": "42", "confidence": 0.95}"#);
    }

    #[test]
    fn test_extract_json_markdown_without_language() {
        let content = r#"```
{"answer": "42", "confidence": 0.95}
```"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"answer": "42", "confidence": 0.95}"#);
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let content = r#"Here is the JSON response:
{"answer": "42", "confidence": 0.95}
Hope this helps!"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"answer": "42", "confidence": 0.95}"#);
    }

    #[test]
    fn test_extract_json_nested_objects() {
        let content = r#"{"outer": {"inner": {"value": 42}}}"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"outer": {"inner": {"value": 42}}}"#);
    }

    #[test]
    fn test_extract_json_array() {
        let content = r#"[{"a": 1}, {"b": 2}]"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"[{"a": 1}, {"b": 2}]"#);
    }

    #[test]
    fn test_extract_json_with_quotes_in_strings() {
        let content = r#"{"message": "He said \"hello\""}"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"message": "He said \"hello\""}"#);
    }

    #[test]
    fn test_extract_json_with_brackets_in_strings() {
        let content = r#"{"code": "if (x > 5) { return true; }"}"#;
        let result = extract_json(content);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            r#"{"code": "if (x > 5) { return true; }"}"#
        );
    }

    #[test]
    fn test_extract_json_failure_no_json() {
        let content = "This is just plain text with no JSON";
        let result = extract_json(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Could not extract JSON"));
    }

    #[test]
    fn test_extract_and_parse_json_valid() {
        let content = r#"{"answer": "42", "confidence": 0.95}"#;
        let result: std::result::Result<TestOutput, Error> = extract_and_parse_json(content);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.answer, "42");
        assert_eq!(output.confidence, 0.95);
    }

    #[test]
    fn test_extract_and_parse_json_markdown() {
        let content = r#"```json
{"answer": "yes", "confidence": 0.99}
```"#;
        let result: std::result::Result<TestOutput, Error> = extract_and_parse_json(content);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.answer, "yes");
        assert_eq!(output.confidence, 0.99);
    }

    #[test]
    fn test_extract_and_parse_json_invalid_json() {
        let content = r#"{"answer": "42", invalid}"#;
        let result: std::result::Result<TestOutput, Error> = extract_and_parse_json(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to deserialize"));
    }

    #[test]
    fn test_extract_and_parse_json_missing_field() {
        let content = r#"{"answer": "42"}"#; // Missing confidence field
        let result: std::result::Result<TestOutput, Error> = extract_and_parse_json(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to deserialize"));
    }

    #[test]
    fn test_extract_and_parse_json_wrong_type() {
        let content = r#"{"answer": "42", "confidence": "not a number"}"#;
        let result: std::result::Result<TestOutput, Error> = extract_and_parse_json(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to deserialize"));
    }

    #[tokio::test]
    async fn test_parse_result_valid() {
        let mock = MockChatModel {
            response: r#"{"answer": "test", "confidence": 0.85}"#.to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let messages = vec![HumanMessage::new("test").into()];
        let result = structured
            .generate(&messages, None, None, None, None)
            .await
            .unwrap();

        let parsed = structured.parse_result(&result);
        assert!(parsed.is_ok());
        let output = parsed.unwrap();
        assert_eq!(output.answer, "test");
        assert_eq!(output.confidence, 0.85);
    }

    #[tokio::test]
    async fn test_parse_result_markdown() {
        let mock = MockChatModel {
            response: r#"```json
{"answer": "markdown", "confidence": 0.75}
```"#
                .to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let messages = vec![HumanMessage::new("test").into()];
        let result = structured
            .generate(&messages, None, None, None, None)
            .await
            .unwrap();

        let parsed = structured.parse_result(&result);
        assert!(parsed.is_ok());
        let output = parsed.unwrap();
        assert_eq!(output.answer, "markdown");
        assert_eq!(output.confidence, 0.75);
    }

    #[tokio::test]
    async fn test_parse_result_with_surrounding_text() {
        let mock = MockChatModel {
            response: r#"Here is my response:
{"answer": "surrounded", "confidence": 0.65}
I hope this helps!"#
                .to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let messages = vec![HumanMessage::new("test").into()];
        let result = structured
            .generate(&messages, None, None, None, None)
            .await
            .unwrap();

        let parsed = structured.parse_result(&result);
        assert!(parsed.is_ok());
        let output = parsed.unwrap();
        assert_eq!(output.answer, "surrounded");
        assert_eq!(output.confidence, 0.65);
    }

    #[tokio::test]
    async fn test_parse_result_invalid() {
        let mock = MockChatModel {
            response: "This is not JSON at all".to_string(),
        };

        let structured = StructuredChatModel::<TestOutput>::new(mock).unwrap();
        let messages = vec![HumanMessage::new("test").into()];
        let result = structured
            .generate(&messages, None, None, None, None)
            .await
            .unwrap();

        let parsed = structured.parse_result(&result);
        assert!(parsed.is_err());
    }
}
