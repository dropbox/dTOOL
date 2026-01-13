//! Anthropic Claude chat models implementation
//!
//! This module provides integration with Anthropic's Claude models via the Messages API.

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config::RunnableConfig,
    config_loader::env_vars::{
        anthropic_api_url, env_string_or_default, ANTHROPIC_API_KEY,
        DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT,
    },
    error::Error as DashFlowError,
    http_client,
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, Message, ToolCall},
    rate_limiters::RateLimiter,
    runnable::Runnable,
    serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION},
    usage::UsageMetadata,
};
use eventsource_stream::Eventsource;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
struct AnthropicErrorEnvelope {
    #[serde(rename = "type")]
    _envelope_type: Option<String>,
    error: AnthropicErrorBody,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicErrorBody {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Claude model names
pub mod models {
    pub const CLAUDE_3_7_SONNET: &str = "claude-3-7-sonnet-20250219";
    pub const CLAUDE_SONNET_4: &str = "claude-sonnet-4";
    pub const CLAUDE_OPUS_4: &str = "claude-opus-4";
    pub const CLAUDE_3_5_SONNET: &str = "claude-3-5-sonnet-latest";
    pub const CLAUDE_3_5_HAIKU: &str = "claude-3-5-haiku-latest";
}

/// Configuration for extended thinking feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Type of thinking - currently only "enabled" is supported
    #[serde(rename = "type")]
    pub thinking_type: String,
    /// Budget for reasoning tokens
    pub budget_tokens: u32,
}

impl ThinkingConfig {
    /// Create a new thinking configuration with specified token budget
    #[must_use]
    pub fn enabled(budget_tokens: u32) -> Self {
        Self {
            thinking_type: "enabled".to_string(),
            budget_tokens,
        }
    }
}

/// Request format for Anthropic Messages API
#[derive(Debug, Clone, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

/// Tool definition for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicTool {
    /// Tool type - for built-in tools (e.g., "`bash_20241022`", "`text_editor_20241022`")
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Input schema - required for regular tools, optional for built-in tools
    #[serde(skip_serializing_if = "Option::is_none")]
    input_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
    /// Max uses - for built-in tools like `web_search`
    #[serde(skip_serializing_if = "Option::is_none")]
    max_uses: Option<u32>,
}

/// Tool choice parameter for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicToolChoice {
    Auto { r#type: String },
    Any { r#type: String },
    Tool { r#type: String, name: String },
}

/// Convert `DashFlow` `ToolDefinition` to Anthropic `AnthropicTool`
fn convert_tool_definition_anthropic(tool: &ToolDefinition) -> AnthropicTool {
    AnthropicTool {
        r#type: None, // Regular tools don't have a type field
        name: tool.name.clone(),
        description: if tool.description.is_empty() {
            None
        } else {
            Some(tool.description.clone())
        },
        input_schema: Some(tool.parameters.clone()),
        cache_control: None,
        max_uses: None,
    }
}

/// Convert `DashFlow` `ToolChoice` to Anthropic `AnthropicToolChoice`
fn convert_tool_choice_anthropic(choice: &ToolChoice) -> AnthropicToolChoice {
    match choice {
        ToolChoice::Auto => AnthropicToolChoice::Auto {
            r#type: "auto".to_string(),
        },
        ToolChoice::None => AnthropicToolChoice::Auto {
            r#type: "auto".to_string(),
        }, // Anthropic doesn't have "none", use "auto"
        ToolChoice::Required => AnthropicToolChoice::Any {
            r#type: "any".to_string(),
        },
        ToolChoice::Specific(name) => AnthropicToolChoice::Tool {
            r#type: "tool".to_string(),
            name: name.clone(),
        },
    }
}

/// Cache control for prompt caching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheControl {
    /// Cache type - currently only "ephemeral" is supported
    #[serde(rename = "type")]
    pub cache_type: String,
    /// Time to live - "5m" or "1h"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

impl CacheControl {
    /// Create a new cache control with 5 minute TTL
    #[must_use]
    pub fn ephemeral() -> Self {
        Self {
            cache_type: "ephemeral".to_string(),
            ttl: Some("5m".to_string()),
        }
    }

    /// Create a cache control with custom TTL
    #[must_use]
    pub fn with_ttl(ttl: &str) -> Self {
        Self {
            cache_type: "ephemeral".to_string(),
            ttl: Some(ttl.to_string()),
        }
    }
}

/// System block for prompt caching in system messages
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// System content can be string or array of blocks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum SystemContent {
    Text(String),
    Blocks(Vec<SystemBlock>),
}

/// Message format for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String, // "user" or "assistant"
    content: AnthropicContent,
}

/// Content can be string or list of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Content block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Image {
        source: AnthropicImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Extended thinking block (reasoning tokens)
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Redacted thinking block (Claude 4 models)
    RedactedThinking {
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

/// Image source for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum AnthropicImageSource {
    Url { url: String },
    Base64 { media_type: String, data: String },
}

/// Response from Anthropic Messages API
#[derive(Debug, Clone, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    _response_type: String,
    #[serde(rename = "role")]
    _role: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Usage,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
    /// Tokens used to create a cache entry (prompt caching)
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
    /// Tokens read from cache (prompt caching)
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
}

/// Streaming event types from Anthropic SSE
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // JUSTIFICATION: Serde deserialization enum from Anthropic API. All variants
                    // are used by serde during JSON parsing (type="message_start", etc). Individual fields
                    // like MessageStart::message will be accessed when tool call support and metadata extraction
                    // are fully implemented. Not dead code - actively used by serde during stream parsing.
enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: StreamContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: Usage,
    },
    MessageStop,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // JUSTIFICATION: Serde deserialization struct for Anthropic message_start event.
                    // All fields (id, role, model, usage) are populated by serde from JSON. Will be accessed
                    // when message metadata tracking and usage statistics features are implemented. Not dead
                    // code - actively used by serde, awaiting feature completion for direct field access.
struct MessageStartData {
    id: String,
    #[serde(rename = "type")]
    _message_type: String,
    role: String,
    model: String,
    usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum StreamContentBlock {
    Text {
        // JUSTIFICATION: Serde deserialization field for Anthropic text content blocks.
        // Field is populated by serde from JSON. Used in pattern matching (enum variant exists)
        // and will be directly accessed when text content extraction features are implemented.
        #[allow(dead_code)]
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ContentDelta {
    TextDelta {
        text: String,
    },
    InputJsonDelta {
        // JUSTIFICATION: Serde deserialization field for Anthropic tool input deltas.
        // Field is populated by serde from JSON during streaming tool calls. Will be directly
        // accessed when full streaming tool support (incremental JSON parsing) is implemented.
        // Not dead code - part of Anthropic streaming protocol, awaiting feature completion.
        #[allow(dead_code)]
        partial_json: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct MessageDeltaData {
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

/// State for accumulating streaming tool calls
#[derive(Debug, Default)]
struct StreamToolCallState {
    /// Currently accumulating tool call ID
    current_tool_id: Option<String>,
    /// Currently accumulating tool name
    current_tool_name: Option<String>,
    /// Accumulated JSON string for current tool
    accumulated_json: String,
}

impl StreamToolCallState {
    fn new() -> Self {
        Self::default()
    }

    /// Start accumulating a new tool call
    fn start_tool(&mut self, id: String, name: String) {
        self.current_tool_id = Some(id);
        self.current_tool_name = Some(name);
        self.accumulated_json.clear();
    }

    /// Accumulate a partial JSON chunk
    fn accumulate_json(&mut self, partial: &str) {
        self.accumulated_json.push_str(partial);
    }

    /// Complete the current tool call and return it
    fn finish_tool(&mut self) -> Option<ToolCall> {
        if let (Some(id), Some(name)) = (self.current_tool_id.take(), self.current_tool_name.take())
        {
            // Parse the accumulated JSON
            let args = if self.accumulated_json.is_empty() {
                serde_json::json!({})
            } else {
                match serde_json::from_str(&self.accumulated_json) {
                    Ok(value) => value,
                    Err(e) => {
                        // If parsing fails, wrap the raw string in an error object
                        serde_json::json!({
                            "error": format!("Failed to parse tool arguments: {}", e),
                            "raw": self.accumulated_json.clone()
                        })
                    }
                }
            };

            self.accumulated_json.clear();

            Some(ToolCall {
                id,
                name,
                args,
                tool_type: "tool_call".to_string(),
                index: None,
            })
        } else {
            None
        }
    }
}

/// Convert streaming event to `AIMessageChunk` with stateful tool call accumulation
fn convert_stream_event_to_chunk_stateful(
    event: StreamEvent,
    state: &mut StreamToolCallState,
) -> Option<AIMessageChunk> {
    match event {
        StreamEvent::MessageStart { message } => {
            // Return chunk with model metadata
            let mut chunk = AIMessageChunk::new("");
            chunk
                .fields
                .response_metadata
                .insert("model".to_string(), serde_json::json!(message.model));
            Some(chunk)
        }
        StreamEvent::ContentBlockStart { content_block, .. } => {
            match content_block {
                StreamContentBlock::Text { .. } => {
                    // For text blocks, we don't need to emit anything here
                    // The actual content comes in ContentBlockDelta events
                    None
                }
                StreamContentBlock::ToolUse { id, name } => {
                    // Start accumulating this tool call
                    state.start_tool(id, name);
                    // Don't emit anything yet - wait for all JSON to accumulate
                    None
                }
            }
        }
        StreamEvent::ContentBlockDelta { delta, .. } => {
            match delta {
                ContentDelta::TextDelta { text } => Some(AIMessageChunk::new(text)),
                ContentDelta::InputJsonDelta { partial_json } => {
                    // Accumulate the partial JSON
                    state.accumulate_json(&partial_json);
                    // Don't emit a chunk yet - wait for block to complete
                    None
                }
            }
        }
        StreamEvent::ContentBlockStop { .. } => {
            // Finish accumulating and emit the complete tool call if we have one
            if let Some(tool_call) = state.finish_tool() {
                let mut chunk = AIMessageChunk::new("");
                chunk.tool_calls.push(tool_call);
                Some(chunk)
            } else {
                None
            }
        }
        StreamEvent::MessageDelta { delta, usage } => {
            // Return chunk with final usage metadata
            let mut chunk = AIMessageChunk::new("");
            chunk.usage_metadata = Some(UsageMetadata {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.input_tokens + usage.output_tokens,
                input_token_details: None,
                output_token_details: None,
            });
            chunk.fields.response_metadata.insert(
                "stop_reason".to_string(),
                serde_json::json!(delta.stop_reason),
            );
            chunk.fields.response_metadata.insert(
                "stop_sequence".to_string(),
                serde_json::json!(delta.stop_sequence),
            );
            Some(chunk)
        }
        StreamEvent::MessageStop | StreamEvent::Unknown => {
            // No chunk needed for message stop or unknown events
            None
        }
    }
}

/// Anthropic Claude chat model
///
/// # Example
///
/// ```no_run
/// use dashflow_anthropic::ChatAnthropic;
/// use dashflow::core::messages::Message;
/// use dashflow::core::language_models::ChatModel;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ChatAnthropic::try_new()?
///     .with_api_key("your-api-key")
///     .with_model("claude-3-7-sonnet-20250219")
///     .with_max_tokens(1024);
///
/// let messages = vec![Message::human("What is Rust?")];
/// let response = model.generate(&messages, None, None, None, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ChatAnthropic {
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    stop_sequences: Option<Vec<String>>,
    tools: Option<Vec<AnthropicTool>>,
    tool_choice: Option<AnthropicToolChoice>,
    thinking: Option<ThinkingConfig>,
    api_url: String,
    api_version: String,
    http_client: reqwest::Client,
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

// Custom Debug to prevent API key exposure in logs
impl std::fmt::Debug for ChatAnthropic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatAnthropic")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("top_p", &self.top_p)
            .field("top_k", &self.top_k)
            .field("stop_sequences", &self.stop_sequences)
            .field("tools", &self.tools)
            .field("tool_choice", &self.tool_choice)
            .field("thinking", &self.thinking)
            .field("api_url", &self.api_url)
            .field("api_version", &self.api_version)
            .field("http_client", &"[reqwest::Client]")
            .field("rate_limiter", &self.rate_limiter.as_ref().map(|_| "[RateLimiter]"))
            .finish()
    }
}

/// Check if a tool is a built-in Anthropic tool
///
/// Built-in tools are identified by their type field starting with specific prefixes.
/// See [Anthropic docs](https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/overview)
fn is_builtin_tool(tool: &serde_json::Value) -> bool {
    const BUILTIN_TOOL_PREFIXES: &[&str] = &[
        "text_editor_",
        "computer_",
        "bash_",
        "web_search_",
        "web_fetch_",
        "code_execution_",
        "memory_",
    ];

    if let Some(tool_type) = tool.get("type").and_then(|t| t.as_str()) {
        return BUILTIN_TOOL_PREFIXES
            .iter()
            .any(|prefix| tool_type.starts_with(prefix));
    }
    false
}

impl ChatAnthropic {
    fn map_http_error(
        status: reqwest::StatusCode,
        retry_after: Option<&str>,
        body: &str,
    ) -> DashFlowError {
        let parsed = serde_json::from_str::<AnthropicErrorEnvelope>(body).ok();
        let (maybe_type, maybe_message) = parsed
            .as_ref()
            .map(|e| (Some(e.error.error_type.as_str()), Some(e.error.message.as_str())))
            .unwrap_or((None, None));

        let error_type = maybe_type.unwrap_or("unknown_error");
        let message = maybe_message.unwrap_or(body).trim();
        let message = if message.is_empty() { "Unknown error" } else { message };

        let message = match retry_after {
            Some(v) if !v.trim().is_empty() => format!("{message} (retry_after={})", v.trim()),
            _ => message.to_string(),
        };

        match (status, error_type) {
            (reqwest::StatusCode::TOO_MANY_REQUESTS, _) | (_, "rate_limit_error") => {
                return DashFlowError::rate_limit(message);
            }
            (reqwest::StatusCode::UNAUTHORIZED, _)
            | (reqwest::StatusCode::FORBIDDEN, _)
            | (_, "authentication_error")
            | (_, "permission_error") => {
                return DashFlowError::authentication(message);
            }
            (reqwest::StatusCode::BAD_REQUEST, _)
            | (reqwest::StatusCode::NOT_FOUND, _)
            | (reqwest::StatusCode::PAYLOAD_TOO_LARGE, _)
            | (_, "invalid_request_error")
            | (_, "not_found_error")
            | (_, "request_too_large") => {
                return DashFlowError::invalid_input(message);
            }
            (_, "overloaded_error") => return DashFlowError::network(message),
            _ if status.is_server_error() => return DashFlowError::network(message),
            _ => {}
        }

        DashFlowError::api(format!(
            "Anthropic API error ({status}): {message}"
        ))
    }

    /// Create a new `ChatAnthropic` instance
    ///
    /// Automatically loads API key from `ANTHROPIC_API_KEY` environment variable.
    /// If not set, the API key will be empty and must be set with `with_api_key()`.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new` for a fallible alternative.
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_anthropic::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client for ChatAnthropic")
    }

    /// Try to create a new `ChatAnthropic` instance
    ///
    /// Automatically loads API key from `ANTHROPIC_API_KEY` environment variable.
    /// If not set, the API key will be empty and must be set with `with_api_key()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> Result<Self, DashFlowError> {
        let api_key = env_string_or_default(ANTHROPIC_API_KEY, "");

        Ok(Self {
            api_key,
            model: models::CLAUDE_3_7_SONNET.to_string(),
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            api_url: anthropic_api_url(DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT),
            api_version: "2023-06-01".to_string(),
            http_client: http_client::create_llm_client()?,
            rate_limiter: None,
        })
    }

    /// Set the API key
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Set the model name
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set `max_tokens` (required by Anthropic API)
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set temperature (0.0 to 1.0)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set `top_p` (nucleus sampling)
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set `top_k`
    #[must_use]
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set stop sequences
    #[must_use]
    pub fn with_stop_sequences(mut self, stop_sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(stop_sequences);
        self
    }

    /// Set the API URL for the Anthropic API
    ///
    /// Use this to point to a different endpoint, such as:
    /// - A proxy server
    /// - A local testing server
    /// - A compatible API (e.g., AWS Bedrock via proxy)
    ///
    /// Defaults to `https://api.anthropic.com/v1/messages`.
    /// The base URL can also be overridden via `ANTHROPIC_API_BASE_URL` env var.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_anthropic::ChatAnthropic;
    ///
    /// let model = ChatAnthropic::try_new()?
    ///     .with_api_url("http://localhost:8080/v1/messages");
    /// ```
    #[must_use]
    pub fn with_api_url(mut self, api_url: impl Into<String>) -> Self {
        self.api_url = api_url.into();
        self
    }

    /// Set the API version header for the Anthropic API
    ///
    /// Defaults to `2023-06-01`. Only change this if you need a specific API version.
    #[must_use]
    pub fn with_api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = api_version.into();
        self
    }

    /// Bind tools to the model
    ///
    /// Supports both regular tools (with JSON Schema) and Anthropic built-in tools.
    ///
    /// # Regular Tools
    ///
    /// Regular tools must have `name`, `description`, and `input_schema` fields:
    ///
    /// ```json
    /// {
    ///   "name": "get_weather",
    ///   "description": "Get weather for a location",
    ///   "input_schema": { "type": "object", "properties": {...} }
    /// }
    /// ```
    ///
    /// # Built-in Tools
    ///
    /// Built-in tools are identified by their `type` field and don't require `input_schema`.
    /// See [Anthropic docs](https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/overview)
    ///
    /// Supported built-in tool types:
    /// - `text_editor_*`: Text editing tools
    /// - `computer_*`: Computer use tools
    /// - `bash_*`: Bash execution tools
    /// - `web_search_*`: Web search tools
    /// - `web_fetch_*`: Web fetching tools
    /// - `code_execution_*`: Code execution tools
    /// - `memory_*`: Memory tools
    ///
    /// Example:
    ///
    /// ```json
    /// {
    ///   "type": "bash_20241022",
    ///   "name": "bash"
    /// }
    /// ```
    #[deprecated(
        since = "1.9.0",
        note = "Use bind_tools() from ChatModelToolBindingExt trait instead. \
                bind_tools() is type-safe and works consistently across all providers. \
                Example: `use dashflow::core::language_models::ChatModelToolBindingExt; \
                model.bind_tools(vec![Arc::new(tool)], None)`"
    )]
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        let anthropic_tools = tools
            .into_iter()
            .map(|tool| {
                if is_builtin_tool(&tool) {
                    // Built-in tool - pass through with type field
                    AnthropicTool {
                        r#type: tool["type"].as_str().map(std::string::ToString::to_string),
                        name: tool["name"].as_str().unwrap_or("").to_string(),
                        description: tool
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(std::string::ToString::to_string),
                        input_schema: None,
                        cache_control: None,
                        max_uses: tool
                            .get("max_uses")
                            .and_then(serde_json::Value::as_u64)
                            .map(|m| m as u32),
                    }
                } else {
                    // Regular tool - convert to Anthropic format
                    AnthropicTool {
                        r#type: None,
                        name: tool["name"].as_str().unwrap_or("").to_string(),
                        description: tool
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(std::string::ToString::to_string),
                        input_schema: Some(tool["input_schema"].clone()),
                        cache_control: None,
                        max_uses: None,
                    }
                }
            })
            .collect();
        self.tools = Some(anthropic_tools);
        self
    }

    /// Set tool choice parameter
    ///
    /// Options:
    /// - `"auto"` or `None`: Model decides whether to call tools
    /// - `"any"`: Model must call at least one tool
    /// - Tool name string: Model must call the specified tool
    #[must_use]
    pub fn with_tool_choice(mut self, tool_choice: Option<String>) -> Self {
        self.tool_choice = match tool_choice.as_deref() {
            None | Some("auto") => Some(AnthropicToolChoice::Auto {
                r#type: "auto".to_string(),
            }),
            Some("any") => Some(AnthropicToolChoice::Any {
                r#type: "any".to_string(),
            }),
            Some(name) => Some(AnthropicToolChoice::Tool {
                r#type: "tool".to_string(),
                name: name.to_string(),
            }),
        };
        self
    }

    /// Enable extended thinking (reasoning tokens)
    ///
    /// Extended thinking allows supported Claude models to output step-by-step
    /// reasoning process. Requires specifying a token budget for thinking.
    ///
    /// Supported models: claude-3-7-sonnet-latest, claude-sonnet-4, claude-opus-4
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_anthropic::ChatAnthropic;
    /// use dashflow_anthropic::ThinkingConfig;
    ///
    /// let model = ChatAnthropic::try_new().unwrap()
    ///     .with_model("claude-3-7-sonnet-latest")
    ///     .with_thinking(ThinkingConfig::enabled(2000));
    /// ```
    #[must_use]
    pub fn with_thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Set a rate limiter to control request rate
    ///
    /// Rate limiting is applied transparently in `generate()` and `stream()` methods.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_anthropic::ChatAnthropic;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let model = ChatAnthropic::try_new().unwrap()
    ///     .with_api_key("your-api-key")
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Create a `ChatAnthropic` instance from a configuration
    ///
    /// This method constructs a `ChatAnthropic` model from a `ChatModelConfig::Anthropic` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be Anthropic variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatAnthropic` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not an Anthropic variant
    /// - API key environment variable cannot be resolved
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
    /// use dashflow_anthropic::ChatAnthropic;
    ///
    /// let config = ChatModelConfig::Anthropic {
    ///     model: "claude-3-5-sonnet-20241022".to_string(),
    ///     api_key: SecretReference::EnvVar { env: "ANTHROPIC_API_KEY".to_string() },
    ///     temperature: Some(0.7),
    ///     max_tokens: Some(1024),
    /// };
    ///
    /// let chat_model = ChatAnthropic::from_config(&config).unwrap();
    /// ```
    #[allow(deprecated)]
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::Anthropic {
                model,
                api_key,
                temperature,
                max_tokens,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                // Create the ChatAnthropic instance
                #[allow(clippy::disallowed_methods)] // Self::new() may access env vars for default config
                let mut chat_model = Self::new()
                    .with_api_key(&resolved_api_key)
                    .with_model(model);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(*temp);
                }

                if let Some(max_tok) = max_tokens {
                    chat_model = chat_model.with_max_tokens(*max_tok);
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected Anthropic config, got {} config",
                config.provider()
            ))),
        }
    }

    /// Convert `DashFlow` `ImageSource` to Anthropic format
    fn convert_image_source(
        source: &dashflow::core::messages::ImageSource,
    ) -> AnthropicImageSource {
        match source {
            dashflow::core::messages::ImageSource::Url { url } => {
                AnthropicImageSource::Url { url: url.clone() }
            }
            dashflow::core::messages::ImageSource::Base64 { media_type, data } => {
                AnthropicImageSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                }
            }
        }
    }

    /// Convert `DashFlow` `MessageContent` to Anthropic content blocks
    fn convert_content(content: &dashflow::core::messages::MessageContent) -> Vec<ContentBlock> {
        use dashflow::core::messages::MessageContent;

        match content {
            MessageContent::Text(text) => {
                if text.is_empty() {
                    vec![]
                } else {
                    vec![ContentBlock::Text {
                        text: text.clone(),
                        cache_control: None,
                    }]
                }
            }
            MessageContent::Blocks(blocks) => {
                blocks
                    .iter()
                    .filter_map(|block| {
                        match block {
                            dashflow::core::messages::ContentBlock::Text { text } => {
                                if text.is_empty() {
                                    None
                                } else {
                                    Some(ContentBlock::Text {
                                        text: text.clone(),
                                        cache_control: None,
                                    })
                                }
                            }
                            dashflow::core::messages::ContentBlock::Image { source, .. } => {
                                Some(ContentBlock::Image {
                                    source: Self::convert_image_source(source),
                                    cache_control: None,
                                })
                            }
                            // Other content block types are not directly convertible to Anthropic request format
                            _ => None,
                        }
                    })
                    .collect()
            }
        }
    }

    /// Convert `DashFlow` messages to Anthropic format
    fn convert_messages(
        &self,
        messages: &[BaseMessage],
    ) -> Result<(Option<SystemContent>, Vec<AnthropicMessage>), DashFlowError> {
        let mut system_prompt: Option<String> = None;
        let mut anthropic_messages = Vec::new();

        for message in messages {
            match message {
                Message::System { content, .. } => {
                    // Anthropic uses a top-level system parameter
                    if system_prompt.is_some() {
                        return Err(DashFlowError::InvalidInput(
                            "Multiple system messages are not supported by Anthropic".to_string(),
                        ));
                    }
                    system_prompt = Some(content.as_text());
                }
                Message::Human { content, .. } => {
                    let blocks = Self::convert_content(content);

                    // Convert to appropriate content format
                    let anthropic_content = if blocks.is_empty() {
                        // Empty message - use empty string
                        AnthropicContent::Text(String::new())
                    } else if blocks.len() == 1 {
                        // Single text block can be sent as string
                        if let ContentBlock::Text { text, .. } = &blocks[0] {
                            AnthropicContent::Text(text.clone())
                        } else {
                            AnthropicContent::Blocks(blocks)
                        }
                    } else {
                        // Multiple blocks or non-text blocks need Blocks format
                        AnthropicContent::Blocks(blocks)
                    };

                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: anthropic_content,
                    });
                }
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    // Build content blocks for AI message
                    let mut blocks = Self::convert_content(content);

                    // Add tool use blocks
                    for tool_call in tool_calls {
                        blocks.push(ContentBlock::ToolUse {
                            id: tool_call.id.clone(),
                            name: tool_call.name.clone(),
                            input: tool_call.args.clone(),
                            cache_control: None,
                        });
                    }

                    let anthropic_content = if blocks.is_empty() {
                        // Empty AI message
                        AnthropicContent::Text(String::new())
                    } else if blocks.len() == 1 {
                        // Single text block can be sent as string
                        if let ContentBlock::Text { text, .. } = &blocks[0] {
                            AnthropicContent::Text(text.clone())
                        } else {
                            AnthropicContent::Blocks(blocks)
                        }
                    } else {
                        AnthropicContent::Blocks(blocks)
                    };

                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: anthropic_content,
                    });
                }
                Message::Tool {
                    content,
                    tool_call_id,
                    status,
                    ..
                } => {
                    // Tool results go in user messages
                    let is_error = status.as_deref() == Some("error");
                    let tool_result = ContentBlock::ToolResult {
                        tool_use_id: tool_call_id.clone(),
                        content: content.as_text(),
                        is_error,
                        cache_control: None,
                    };

                    // Merge with previous user message or create new one
                    // For now, create new user message (merging can be added later)
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![tool_result]),
                    });
                }
                Message::Function { .. } => {
                    // Function messages are OpenAI's legacy function calling format.
                    // Anthropic Claude uses tool_call/tool_result format instead.
                    // Users should convert Function messages to Tool messages.
                    return Err(DashFlowError::InvalidInput(
                        "Function messages are not supported by Anthropic. Use Tool messages instead (Message::Tool for results, or tool_calls on AI messages)".to_string(),
                    ));
                }
            }
        }

        // Wrap system prompt in SystemContent if present
        let system = system_prompt.map(SystemContent::Text);
        Ok((system, anthropic_messages))
    }

    /// Make a request to the Anthropic API
    async fn make_request(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AnthropicResponse, DashFlowError> {
        if self.api_key.is_empty() {
            return Err(DashFlowError::authentication(
                "API key is required. Set it with with_api_key() or the ANTHROPIC_API_KEY environment variable"
            ));
        }

        let (system, anthropic_messages) = self.convert_messages(messages)?;

        // Stop sequences: prefer parameter over struct field
        let stop_sequences = if let Some(stop_seqs) = stop {
            Some(stop_seqs.to_vec())
        } else {
            self.stop_sequences.clone()
        };

        // Tools: prefer parameter over struct field
        let anthropic_tools = if let Some(tool_defs) = tools {
            Some(
                tool_defs
                    .iter()
                    .map(convert_tool_definition_anthropic)
                    .collect(),
            )
        } else {
            self.tools.clone()
        };

        // Tool choice: prefer parameter over struct field
        let anthropic_tool_choice = if let Some(tc) = tool_choice {
            Some(convert_tool_choice_anthropic(tc))
        } else {
            self.tool_choice.clone()
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: anthropic_messages,
            system,
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences,
            tools: anthropic_tools,
            tool_choice: anthropic_tool_choice,
            thinking: self.thinking.clone(),
        };

        let response = self
            .http_client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| DashFlowError::Api(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ChatAnthropic::map_http_error(
                status,
                retry_after.as_deref(),
                &error_text,
            ));
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::Api(format!("Failed to parse response: {e}")))?;

        Ok(anthropic_response)
    }

    /// Make a streaming request to the Anthropic API
    async fn make_streaming_request(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, DashFlowError>> + Send>>, DashFlowError>
    {
        if self.api_key.is_empty() {
            return Err(DashFlowError::authentication(
                "API key is required. Set it with with_api_key() or the ANTHROPIC_API_KEY environment variable"
            ));
        }

        let (system, anthropic_messages) = self.convert_messages(messages)?;

        // Stop sequences: prefer parameter over struct field
        let stop_sequences = if let Some(stop_seqs) = stop {
            Some(stop_seqs.to_vec())
        } else {
            self.stop_sequences.clone()
        };

        // Tools: prefer parameter over struct field
        let anthropic_tools = if let Some(tool_defs) = tools {
            Some(
                tool_defs
                    .iter()
                    .map(convert_tool_definition_anthropic)
                    .collect(),
            )
        } else {
            self.tools.clone()
        };

        // Tool choice: prefer parameter over struct field
        let anthropic_tool_choice = if let Some(tc) = tool_choice {
            Some(convert_tool_choice_anthropic(tc))
        } else {
            self.tool_choice.clone()
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: anthropic_messages,
            system,
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences,
            tools: anthropic_tools,
            tool_choice: anthropic_tool_choice,
            thinking: self.thinking.clone(),
        };

        // Add stream parameter via additional field
        let mut request_json = serde_json::to_value(&request)
            .map_err(|e| DashFlowError::Api(format!("Failed to serialize request: {e}")))?;

        if let Some(obj) = request_json.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::json!(true));
        }

        let response = self
            .http_client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json")
            .json(&request_json)
            .send()
            .await
            .map_err(|e| DashFlowError::Api(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ChatAnthropic::map_http_error(
                status,
                retry_after.as_deref(),
                &error_text,
            ));
        }

        // Convert response body to SSE stream
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource().map(|result| {
            result
                .map_err(|e| DashFlowError::Api(format!("SSE parse error: {e}")))
                .and_then(|event| {
                    serde_json::from_str::<StreamEvent>(&event.data)
                        .map_err(|e| DashFlowError::Api(format!("Failed to parse event: {e}")))
                })
        });

        Ok(Box::pin(event_stream))
    }

    /// Convert Anthropic response to `DashFlow` `AIMessage`
    fn convert_response(&self, response: AnthropicResponse) -> ChatResult {
        // Extract text content and tool calls from content blocks
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut has_thinking = false;
        let mut content_blocks = Vec::new();

        // First pass: check if response contains thinking blocks
        for block in &response.content {
            if matches!(
                block,
                ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
            ) {
                has_thinking = true;
                break;
            }
        }

        // Convert response blocks
        for block in &response.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    text_parts.push(text.as_str());
                    if has_thinking {
                        content_blocks.push(dashflow::core::messages::ContentBlock::Text {
                            text: text.clone(),
                        });
                    }
                }
                ContentBlock::Image { .. } => {
                    // Image blocks in responses are echoed back, not generated
                    // Ignore for text extraction
                }
                ContentBlock::ToolUse {
                    id, name, input, ..
                } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        args: input.clone(),
                        tool_type: "tool_call".to_string(),
                        index: None,
                    });
                    if has_thinking {
                        content_blocks.push(dashflow::core::messages::ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });
                    }
                }
                ContentBlock::ToolResult { .. } => {
                    // ToolResult should not appear in assistant messages
                }
                ContentBlock::Thinking {
                    thinking,
                    signature,
                    ..
                } => {
                    content_blocks.push(dashflow::core::messages::ContentBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                    });
                }
                ContentBlock::RedactedThinking { data, .. } => {
                    content_blocks.push(dashflow::core::messages::ContentBlock::RedactedThinking {
                        data: data.clone(),
                    });
                }
            }
        }

        // Build message content - use blocks if thinking present, otherwise simple text
        let content = if has_thinking {
            dashflow::core::messages::MessageContent::Blocks(content_blocks)
        } else {
            dashflow::core::messages::MessageContent::Text(text_parts.join(""))
        };

        // Build response metadata
        let mut response_metadata = HashMap::new();
        response_metadata.insert("id".to_string(), serde_json::json!(response.id));
        response_metadata.insert("model".to_string(), serde_json::json!(response.model));
        response_metadata.insert(
            "stop_reason".to_string(),
            serde_json::json!(response.stop_reason),
        );
        response_metadata.insert(
            "stop_sequence".to_string(),
            serde_json::json!(response.stop_sequence),
        );
        response_metadata.insert(
            "usage".to_string(),
            serde_json::json!({
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens,
            }),
        );

        // Create usage metadata
        let usage_metadata = UsageMetadata {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
            input_token_details: None,
            output_token_details: None,
        };

        // Construct AI message with tool calls
        let ai_message = AIMessage::new(content)
            .with_usage(usage_metadata)
            .with_tool_calls(tool_calls.clone());

        // Convert to Message enum
        let mut message = Message::from(ai_message);
        message.fields_mut().response_metadata = response_metadata;

        // Build generation_info with response metadata
        let mut generation_info = HashMap::new();
        generation_info.insert("id".to_string(), serde_json::json!(response.id));
        generation_info.insert("model".to_string(), serde_json::json!(response.model));
        generation_info.insert(
            "stop_reason".to_string(),
            serde_json::json!(response.stop_reason),
        );
        if let Some(stop_sequence) = &response.stop_sequence {
            generation_info.insert(
                "stop_sequence".to_string(),
                serde_json::json!(stop_sequence),
            );
        }
        generation_info.insert(
            "usage".to_string(),
            serde_json::json!({
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens,
            }),
        );

        // Add prompt caching metrics at top level (for easy access in generation_info)
        generation_info.insert(
            "input_tokens".to_string(),
            serde_json::json!(response.usage.input_tokens),
        );
        generation_info.insert(
            "output_tokens".to_string(),
            serde_json::json!(response.usage.output_tokens),
        );
        if let Some(cache_creation) = response.usage.cache_creation_input_tokens {
            generation_info.insert(
                "cache_creation_input_tokens".to_string(),
                serde_json::json!(cache_creation),
            );
        }
        if let Some(cache_read) = response.usage.cache_read_input_tokens {
            generation_info.insert(
                "cache_read_input_tokens".to_string(),
                serde_json::json!(cache_read),
            );
        }

        let generation = ChatGeneration {
            message,
            generation_info: Some(generation_info),
        };

        ChatResult {
            generations: vec![generation],
            llm_output: None,
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatAnthropic {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization implementation for ChatAnthropic
impl Serializable for ChatAnthropic {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "anthropic".to_string(),
            "ChatAnthropic".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Model name (required)
        kwargs.insert("model".to_string(), serde_json::json!(self.model));

        // Max tokens (required for Anthropic)
        kwargs.insert("max_tokens".to_string(), serde_json::json!(self.max_tokens));

        // Optional parameters (only include if set)
        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(tp) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(tp));
        }

        if let Some(tk) = self.top_k {
            kwargs.insert("top_k".to_string(), serde_json::json!(tk));
        }

        if let Some(ref stop_seqs) = self.stop_sequences {
            if !stop_seqs.is_empty() {
                kwargs.insert("stop_sequences".to_string(), serde_json::json!(stop_seqs));
            }
        }

        // Note: tools, tool_choice, thinking, rate_limiter, and http_client are not serialized
        // They should be configured at runtime for security and flexibility

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        // Mark API key as a secret - it will be loaded from ANTHROPIC_API_KEY env var
        secrets.insert("api_key".to_string(), "ANTHROPIC_API_KEY".to_string());
        secrets
    }
}

#[async_trait]
impl ChatModel for ChatAnthropic {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult, DashFlowError> {
        // Generate run ID for tracing
        let run_id = Uuid::new_v4();

        // Call on_llm_start callback
        if let Some(manager) = run_manager {
            let serialized = self.identifying_params();
            let prompts: Vec<String> = messages
                .iter()
                .map(dashflow::core::messages::Message::as_text)
                .collect();

            manager
                .on_llm_start(
                    &serialized,
                    &prompts,
                    run_id,
                    None,            // parent_run_id
                    &[],             // tags
                    &HashMap::new(), // metadata
                )
                .await?;
        }

        // Make API request
        let response_result = self.make_request(messages, stop, tools, tool_choice).await;

        // Handle error callback
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(manager) = run_manager {
                    if let Err(cb_err) = manager.on_llm_error(&e.to_string(), run_id, None).await {
                        eprintln!("[WARN] Failed to send LLM error callback: {}", cb_err);
                    }
                }
                return Err(e);
            }
        };

        let chat_result = self.convert_response(response);

        // Call on_llm_end callback
        if let Some(manager) = run_manager {
            let mut outputs = HashMap::new();
            outputs.insert(
                "generations".to_string(),
                serde_json::to_value(&chat_result.generations)?,
            );
            manager.on_llm_end(&outputs, run_id, None).await?;
        }

        Ok(chat_result)
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk, DashFlowError>> + Send + 'static>>,
        DashFlowError,
    > {
        let mut event_stream = self
            .make_streaming_request(messages, stop, tools, tool_choice)
            .await?;

        // Create a stateful stream that accumulates tool call JSON
        let chunk_stream = stream! {
            let mut state = StreamToolCallState::new();

            while let Some(result) = event_stream.next().await {
                match result {
                    Ok(event) => {
                        // Convert event to chunk with stateful accumulation
                        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
                            yield Ok(ChatGenerationChunk::new(chunk));
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }

    fn llm_type(&self) -> &'static str {
        "anthropic"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.rate_limiter.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl Runnable for ChatAnthropic {
    type Input = Vec<Message>;
    type Output = Message;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output, DashFlowError> {
        let result = self
            .generate(&input, None, None, None, config.as_ref())
            .await?;
        result
            .generations
            .into_iter()
            .next()
            .map(|gen| gen.message)
            .ok_or_else(|| DashFlowError::Api("No generations returned".to_string()))
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>, DashFlowError> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, None).await?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod standard_tests;
