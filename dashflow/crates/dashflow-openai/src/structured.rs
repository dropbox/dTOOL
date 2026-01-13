//! Structured output support for `OpenAI` chat models
//!
//! This module provides Python DashFlow-compatible structured output capabilities
//! for `OpenAI` models, supporting multiple methods:
//!
//! - `json_mode`: Uses `OpenAI`'s JSON mode API (`response_format`: {type: "`json_object`"})
//! - `json_schema`: Uses `OpenAI`'s Structured Outputs API with JSON schema
//! - `function_calling`: Uses `OpenAI`'s tool calling API
//!
//! # Methods Comparison
//!
//! ## `JsonMode`
//! - Uses `response_format: { type: "json_object" }`
//! - Requires prompt engineering (system message with schema instructions)
//! - Less strict - model may deviate from schema
//! - Compatible with most `OpenAI` models
//!
//! ## `JsonSchema`
//! - Uses `response_format: { type: "json_schema", json_schema: {...} }`
//! - Schema is strictly enforced by the API
//! - More reliable, guaranteed to match schema
//! - Requires models with structured output support (e.g., gpt-4o)
//!
//! ## `FunctionCalling`
//! - Uses `OpenAI`'s tool/function calling API
//! - Binds schema as a tool definition and forces model to call it
//! - Tool call arguments become the structured output
//! - Compatible with all function-calling capable models
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_openai::{ChatOpenAI, StructuredOutputMethod};
//! use serde::{Serialize, Deserialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct Joke {
//!     setup: String,
//!     punchline: String,
//! }
//!
//! // Using JSON mode
//! let model = ChatOpenAI::with_config(Default::default())
//!     .with_model("gpt-4")
//!     .with_structured_output_typed::<Joke>(StructuredOutputMethod::JsonMode)?;
//! let result: Joke = model.invoke("Tell me a joke about cats").await?;
//!
//! // Using function calling
//! let model = ChatOpenAI::with_config(Default::default())
//!     .with_model("gpt-4")
//!     .with_structured_output_typed::<Joke>(StructuredOutputMethod::FunctionCalling)?;
//! let result: Joke = model.invoke("Tell me a joke about cats").await?;
//! ```

use async_trait::async_trait;
use dashflow::core::callbacks::CallbackManager;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, ChatResult, ToolChoice, ToolDefinition};
use dashflow::core::messages::{BaseMessage, Message};
use dashflow::core::schema::json_schema::json_schema;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

use crate::chat_models::ChatOpenAI;

/// Method for generating structured outputs.
///
/// Matches Python `DashFlow`'s `with_structured_output(method=...)` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredOutputMethod {
    /// Use `OpenAI`'s JSON mode API.
    ///
    /// Sets `response_format: { type: "json_object" }`.
    /// Requires instructing the model in the prompt to return JSON.
    /// Does not enforce a specific schema - relies on prompt engineering.
    JsonMode,

    /// Use `OpenAI`'s Structured Outputs API with JSON schema.
    ///
    /// Sets `response_format: { type: "json_schema", json_schema: {...} }`.
    /// Enforces the exact schema in the response.
    /// More reliable than JSON mode.
    JsonSchema,

    /// Use `OpenAI`'s function/tool calling API.
    ///
    /// Binds a tool definition and forces the model to call it.
    /// The tool's output becomes the structured output.
    FunctionCalling,
}

/// A `ChatOpenAI` wrapper that parses responses into structured outputs.
///
/// This struct wraps a `ChatOpenAI` model and adds automatic JSON parsing
/// of responses into a specified Rust type `T`. It supports multiple methods
/// for steering the model to produce structured outputs.
///
/// # Type Parameters
///
/// * `T` - The output type to parse responses into. Must implement `Deserialize` and `JsonSchema`.
///
/// # Example
///
/// ```rust,ignore
/// // Created via with_structured_output_typed() method
/// let structured = model.with_structured_output_typed::<MySchema>(
///     StructuredOutputMethod::JsonMode
/// )?;
/// let result: MySchema = structured.invoke("Generate data").await?;
/// ```
pub struct OpenAIStructuredChatModel<T> {
    /// The underlying `OpenAI` chat model
    inner: ChatOpenAI,

    /// JSON schema for the output type
    schema: serde_json::Value,

    /// Method for generating structured outputs
    method: StructuredOutputMethod,

    /// Phantom data to track the output type
    _phantom: PhantomData<T>,
}

impl<T> OpenAIStructuredChatModel<T>
where
    T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
{
    /// Create a new structured output model.
    ///
    /// # Arguments
    ///
    /// * `model` - The underlying `OpenAI` chat model
    /// * `method` - The method for generating structured outputs
    ///
    /// # Returns
    ///
    /// A new structured output wrapper, or an error if schema generation fails
    pub fn new(model: ChatOpenAI, method: StructuredOutputMethod) -> Result<Self> {
        let schema = json_schema::<T>()?;
        Ok(Self {
            inner: model,
            schema,
            method,
            _phantom: PhantomData,
        })
    }

    /// Get a reference to the JSON schema for this structured output.
    #[must_use]
    pub fn schema(&self) -> &serde_json::Value {
        &self.schema
    }

    /// Get the structured output method being used.
    #[must_use]
    pub fn method(&self) -> StructuredOutputMethod {
        self.method
    }

    /// Parse a chat result into structured output of type T.
    ///
    /// Extracts JSON from the response content, handles markdown code blocks,
    /// and deserializes into the target type.
    ///
    /// For `FunctionCalling` mode, extracts tool call arguments instead of message content.
    pub fn parse_result(&self, result: &ChatResult) -> Result<T> {
        use dashflow::core::language_models::structured::extract_json;

        let generation = result
            .generations
            .first()
            .ok_or_else(|| Error::OutputParsing("No generations in response".to_string()))?;

        // For FunctionCalling mode, extract tool call arguments
        if self.method == StructuredOutputMethod::FunctionCalling {
            // Get tool calls from the AI message
            if let Message::AI { tool_calls, .. } = &generation.message {
                if tool_calls.is_empty() {
                    return Err(Error::OutputParsing(
                        "Function calling mode: Expected tool call in response, but none found"
                            .to_string(),
                    ));
                }

                // Use the first tool call's arguments
                let tool_call = &tool_calls[0];
                let args = &tool_call.args;

                // Deserialize the tool call arguments into T
                return serde_json::from_value(args.clone()).map_err(|e| {
                    Error::OutputParsing(format!(
                        "Failed to deserialize tool call arguments into target type: {e}. Args: {args}"
                    ))
                });
            }
            return Err(Error::OutputParsing(
                "Function calling mode: Response is not an AI message".to_string(),
            ));
        }

        // For JsonMode and JsonSchema, extract from content
        let content = generation.message.content().as_text();

        // Extract JSON from response (handles markdown, plain JSON, etc.)
        let json_str = extract_json(&content)?;

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

    /// Invoke the model with structured output.
    ///
    /// This is a convenience method that combines `generate()` and `parse_result()`.
    pub async fn invoke(&self, messages: &[BaseMessage]) -> Result<T> {
        let chat_result = self.generate(messages, None, None, None, None).await?;
        self.parse_result(&chat_result)
    }
}

#[async_trait]
impl<T> ChatModel for OpenAIStructuredChatModel<T>
where
    T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
{
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        match self.method {
            StructuredOutputMethod::JsonMode => {
                // Use JSON mode - add system message with schema instructions
                let schema_json = serde_json::to_string_pretty(&self.schema)
                    .unwrap_or_else(|_| self.schema.to_string());

                let system_content = format!(
                    "You are a helpful assistant that responds in JSON format.\n\
                     Your response must be valid JSON that conforms to this schema:\n\n\
                     ```json\n{schema_json}\n```\n\n\
                     Respond with ONLY the JSON object, no additional text or explanation."
                );

                // Prepend system message
                let mut new_messages: Vec<BaseMessage> =
                    vec![Message::system(system_content.clone())];

                // Merge with existing system message if present
                let mut messages_to_add = messages;
                if let Some(Message::System { content, .. }) = messages.first() {
                    let merged_content = format!("{}\n\n{}", system_content, content.as_text());
                    new_messages[0] = Message::system(merged_content);
                    messages_to_add = &messages[1..];
                }

                new_messages.extend_from_slice(messages_to_add);

                // Call model with JSON mode enabled
                let model_with_json_mode = self.inner.clone().with_json_mode();
                model_with_json_mode
                    .generate(&new_messages, stop, tools, tool_choice, None)
                    .await
            }

            StructuredOutputMethod::JsonSchema => {
                // Use JSON schema mode - no system message needed, schema is enforced
                let model_with_schema = self.inner.clone().with_structured_output(
                    "response",
                    self.schema.clone(),
                    Some("Structured response following the provided schema".to_string()),
                    true, // strict
                );
                model_with_schema
                    .generate(messages, stop, tools, tool_choice, None)
                    .await
            }

            StructuredOutputMethod::FunctionCalling => {
                // Use function calling - bind tool and force invocation
                // Following Python DashFlow's implementation:
                // - Convert schema to a tool definition
                // - Force the model to call that specific tool
                // - Extract tool call arguments as the structured output

                // Create tool definition from schema
                // Tool name is derived from the schema title or defaults to "response"
                let tool_name = self
                    .schema
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("response")
                    .to_string();

                let tool_description = self
                    .schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("Structured response following the provided schema")
                    .to_string();

                let tool_def = ToolDefinition {
                    name: tool_name.clone(),
                    description: tool_description,
                    parameters: self.schema.clone(),
                };

                // Force the model to call this specific tool
                let forced_tool_choice = ToolChoice::Specific(tool_name);

                // Call model with the tool bound
                self.inner
                    .generate(
                        messages,
                        stop,
                        Some(&[tool_def]),
                        Some(&forced_tool_choice),
                        None,
                    )
                    .await
            }
        }
    }

    fn llm_type(&self) -> &str {
        self.inner.llm_type()
    }

    fn identifying_params(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut params = self.inner.identifying_params();
        params.insert("structured_output".to_string(), serde_json::json!(true));
        params.insert("output_schema".to_string(), self.schema.clone());
        params.insert(
            "method".to_string(),
            serde_json::json!(format!("{:?}", self.method)),
        );
        params
    }

    fn rate_limiter(
        &self,
    ) -> Option<std::sync::Arc<dyn dashflow::core::rate_limiters::RateLimiter>> {
        self.inner.rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Extension trait adding typed structured output support to `ChatOpenAI`.
pub trait ChatOpenAIStructuredExt {
    /// Configure this chat model to return structured outputs of type `T`.
    ///
    /// Returns an `OpenAIStructuredChatModel<T>` that wraps this model and
    /// automatically parses responses into the specified type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The output type to parse responses into
    ///
    /// # Arguments
    ///
    /// * `method` - The method for generating structured outputs
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
    /// let structured_llm = ChatOpenAI::with_config(Default::default())
    ///     .with_model("gpt-4")
    ///     .with_structured_output_typed::<Answer>(StructuredOutputMethod::JsonMode)?;
    /// let result: Answer = structured_llm.invoke(&messages).await?;
    /// ```
    fn with_structured_output_typed<T>(
        self,
        method: StructuredOutputMethod,
    ) -> Result<OpenAIStructuredChatModel<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + Sync + 'static;
}

impl ChatOpenAIStructuredExt for ChatOpenAI {
    fn with_structured_output_typed<T>(
        self,
        method: StructuredOutputMethod,
    ) -> Result<OpenAIStructuredChatModel<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
    {
        OpenAIStructuredChatModel::new(self, method)
    }
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::panic
)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq)]
    struct TestOutput {
        answer: String,
        confidence: f64,
    }

    #[test]
    fn test_structured_output_method_enum() {
        assert_eq!(
            StructuredOutputMethod::JsonMode,
            StructuredOutputMethod::JsonMode
        );
        assert_ne!(
            StructuredOutputMethod::JsonMode,
            StructuredOutputMethod::JsonSchema
        );
    }

    #[test]
    fn test_new_structured_model() {
        let model = ChatOpenAI::with_config(Default::default());
        let structured =
            OpenAIStructuredChatModel::<TestOutput>::new(model, StructuredOutputMethod::JsonMode);
        assert!(structured.is_ok());

        let structured = structured.unwrap();
        assert_eq!(structured.method(), StructuredOutputMethod::JsonMode);
        assert!(structured.schema().is_object());
    }

    #[test]
    fn test_extension_trait() {
        let model = ChatOpenAI::with_config(Default::default());
        let structured =
            model.with_structured_output_typed::<TestOutput>(StructuredOutputMethod::JsonMode);
        let structured = structured.expect("extension trait should create structured model");
        assert_eq!(structured.method(), StructuredOutputMethod::JsonMode);
        assert!(structured.schema().is_object());
    }

    #[test]
    fn test_function_calling_mode() {
        let model = ChatOpenAI::with_config(Default::default());
        let structured = OpenAIStructuredChatModel::<TestOutput>::new(
            model,
            StructuredOutputMethod::FunctionCalling,
        );
        assert!(structured.is_ok());

        let structured = structured.unwrap();
        assert_eq!(structured.method(), StructuredOutputMethod::FunctionCalling);
    }

    #[test]
    fn test_parse_result_with_tool_call() {
        use dashflow::core::language_models::{ChatGeneration, ChatResult};
        use dashflow::core::messages::{Message, ToolCall};

        let model = ChatOpenAI::with_config(Default::default());
        let structured = OpenAIStructuredChatModel::<TestOutput>::new(
            model,
            StructuredOutputMethod::FunctionCalling,
        )
        .unwrap();

        // Create a mock AI message with a tool call
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "response".to_string(),
            args: serde_json::json!({
                "answer": "Test answer",
                "confidence": 0.95
            }),
            tool_type: "function".to_string(),
            index: None,
        };

        let ai_message = Message::AI {
            content: dashflow::core::messages::MessageContent::Text("".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: dashflow::core::messages::BaseMessageFields::default(),
        };

        let chat_result = ChatResult {
            generations: vec![ChatGeneration {
                message: ai_message,
                generation_info: None,
            }],
            llm_output: None,
        };

        // Parse the result
        let result = structured.parse_result(&chat_result);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.answer, "Test answer");
        assert_eq!(output.confidence, 0.95);
    }

    #[test]
    fn test_parse_result_function_calling_without_tool_calls() {
        use dashflow::core::language_models::{ChatGeneration, ChatResult};
        use dashflow::core::messages::Message;

        let model = ChatOpenAI::with_config(Default::default());
        let structured = OpenAIStructuredChatModel::<TestOutput>::new(
            model,
            StructuredOutputMethod::FunctionCalling,
        )
        .unwrap();

        // Create a mock AI message without tool calls
        let ai_message = Message::ai("Some text response");

        let chat_result = ChatResult {
            generations: vec![ChatGeneration {
                message: ai_message,
                generation_info: None,
            }],
            llm_output: None,
        };

        // Parse should fail because no tool calls
        let result = structured.parse_result(&chat_result);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected tool call in response"));
    }

    #[test]
    fn test_parse_result_json_mode() {
        use dashflow::core::language_models::{ChatGeneration, ChatResult};
        use dashflow::core::messages::Message;

        let model = ChatOpenAI::with_config(Default::default());
        let structured =
            OpenAIStructuredChatModel::<TestOutput>::new(model, StructuredOutputMethod::JsonMode)
                .unwrap();

        // Create a mock AI message with JSON content
        let ai_message = Message::ai(r#"{"answer": "JSON answer", "confidence": 0.88}"#);

        let chat_result = ChatResult {
            generations: vec![ChatGeneration {
                message: ai_message,
                generation_info: None,
            }],
            llm_output: None,
        };

        // Parse the result
        let result = structured.parse_result(&chat_result);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.answer, "JSON answer");
        assert_eq!(output.confidence, 0.88);
    }
}
