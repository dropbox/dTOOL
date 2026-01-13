//! Message types for chat models
//!
//! Messages are the inputs and outputs of chat models. This module provides
//! type-safe message enums and content types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::usage::UsageMetadata;

/// Recursively merge JSON objects (follows Python `merge_dicts` logic from `dashflow_core/utils`/_merge.py)
fn merge_json_objects(
    left: &mut serde_json::Map<String, serde_json::Value>,
    right: &serde_json::Map<String, serde_json::Value>,
) {
    for (key, right_val) in right {
        match left.get_mut(key) {
            Some(left_val) => {
                // Key exists in both, merge according to type
                if right_val.is_null() {
                    // Skip null values from right
                    continue;
                } else if left_val.is_null() {
                    // Left is null, use right value
                    *left_val = right_val.clone();
                } else if left_val.is_string() && right_val.is_string() {
                    // Concatenate strings (except special keys like 'id', 'index')
                    if key == "id"
                        || key == "index"
                        || key == "output_version"
                        || key == "model_provider"
                    {
                        // Don't concatenate these special keys
                        if left_val.as_str() == right_val.as_str() {
                            continue;
                        }
                    }
                    // Concatenate string values
                    if let (Some(left_str), Some(right_str)) =
                        (left_val.as_str(), right_val.as_str())
                    {
                        *left_val = serde_json::Value::String(format!("{left_str}{right_str}"));
                    }
                } else if left_val.is_object() && right_val.is_object() {
                    // Recursively merge objects
                    if let (Some(left_obj), Some(right_obj)) =
                        (left_val.as_object_mut(), right_val.as_object())
                    {
                        merge_json_objects(left_obj, right_obj);
                    }
                } else if left_val.is_array() && right_val.is_array() {
                    // Merge arrays by extending
                    if let (Some(left_arr), Some(right_arr)) =
                        (left_val.as_array_mut(), right_val.as_array())
                    {
                        left_arr.extend_from_slice(right_arr);
                    }
                } else if left_val.is_number() && right_val.is_number() {
                    // Add numbers
                    if let (Some(left_num), Some(right_num)) =
                        (left_val.as_i64(), right_val.as_i64())
                    {
                        *left_val = serde_json::Value::Number(serde_json::Number::from(
                            left_num + right_num,
                        ));
                    } else if let (Some(left_num), Some(right_num)) =
                        (left_val.as_f64(), right_val.as_f64())
                    {
                        let sum = left_num + right_num;
                        if let Some(num) = serde_json::Number::from_f64(sum) {
                            *left_val = serde_json::Value::Number(num);
                        } else {
                            // M-983: from_f64 returns None for NaN/Infinity
                            tracing::warn!(
                                key = %key,
                                sum = %sum,
                                "JSON merge skipping non-finite f64 sum (NaN or Infinity)"
                            );
                        }
                    }
                } else if left_val == right_val {
                    // Same value, skip
                    continue;
                } else {
                    // Type mismatch or unsupported type - use right value
                    *left_val = right_val.clone();
                }
            }
            None => {
                // Key doesn't exist in left, add it
                left.insert(key.clone(), right_val.clone());
            }
        }
    }
}

/// Content block in a message
///
/// Messages can contain different types of content blocks:
/// - Text content
/// - Image content (with URLs or base64 data)
/// - Tool calls and results
/// - Reasoning content (for models with chain-of-thought)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text {
        /// The text content
        text: String,
    },

    /// Image content (URL or base64)
    Image {
        /// Image source (URL or data URI)
        source: ImageSource,
        /// Optional detail level for the image
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },

    /// Tool call (request to execute a tool)
    ToolUse {
        /// Unique ID for this tool call
        id: String,
        /// Name of the tool to call
        name: String,
        /// Arguments for the tool (JSON object)
        input: serde_json::Value,
    },

    /// Tool result (response from tool execution)
    ToolResult {
        /// ID of the tool call this is responding to
        tool_use_id: String,
        /// Result content
        content: String,
        /// Whether the tool execution resulted in an error
        #[serde(default)]
        is_error: bool,
    },

    /// Reasoning content (chain-of-thought from models like `OpenAI` o1)
    Reasoning {
        /// The reasoning text
        reasoning: String,
    },

    /// Anthropic extended thinking block (reasoning tokens)
    #[serde(rename = "thinking")]
    Thinking {
        /// The thinking/reasoning text
        thinking: String,
        /// Optional signature field
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    /// Anthropic redacted thinking block (Claude 4 models)
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        /// Redacted/summarized thinking data
        data: String,
    },
}

impl ContentBlock {
    /// Get the text content from this block
    #[must_use]
    pub fn as_text(&self) -> &str {
        match self {
            ContentBlock::Text { text } => text.as_str(),
            ContentBlock::ToolResult { content, .. } => content.as_str(),
            ContentBlock::Reasoning { reasoning } => reasoning.as_str(),
            ContentBlock::Thinking { thinking, .. } => thinking.as_str(),
            ContentBlock::RedactedThinking { data } => data.as_str(),
            ContentBlock::Image { .. } => "",
            ContentBlock::ToolUse { .. } => "",
        }
    }
}

/// Image source for image content blocks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// URL to an image
    Url {
        /// The image URL
        url: String,
    },
    /// Base64-encoded image data
    Base64 {
        /// Media type (e.g., "image/png")
        media_type: String,
        /// Base64-encoded data
        data: String,
    },
}

/// Detail level for image processing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// Low detail (faster, cheaper)
    Low,
    /// High detail (slower, more expensive, but more accurate)
    High,
    /// Automatic detail selection
    Auto,
}

/// Message content - can be a simple string or structured content blocks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),
    /// List of structured content blocks
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Get the text content of the message
    ///
    /// For simple text content, returns the string.
    /// For blocks, concatenates all text blocks.
    #[must_use]
    pub fn as_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Check if content is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            MessageContent::Text(s) => s.is_empty(),
            MessageContent::Blocks(blocks) => blocks.is_empty(),
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl From<Vec<ContentBlock>> for MessageContent {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        MessageContent::Blocks(blocks)
    }
}

/// A tool call made by an AI model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments as a JSON object
    pub args: serde_json::Value,
    /// Type tag (always "`tool_call`" for compatibility with Python)
    #[serde(rename = "type", default = "tool_call_type")]
    pub tool_type: String,
    /// Index for this tool call (used in streaming to merge chunks)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub index: Option<usize>,
}

fn tool_call_type() -> String {
    "tool_call".to_string()
}

/// An invalid tool call (malformed by the model)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvalidToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool (if parseable)
    pub name: Option<String>,
    /// Raw arguments string
    pub args: Option<String>,
    /// Error message describing why the call is invalid
    pub error: String,
}

/// Base message type containing common fields
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BaseMessageFields {
    /// Optional unique identifier for the message
    pub id: Option<String>,

    /// Optional name for the message
    pub name: Option<String>,

    /// Additional data from the provider (e.g., raw tool calls)
    #[serde(default)]
    pub additional_kwargs: HashMap<String, serde_json::Value>,

    /// Response metadata (headers, logprobs, token counts, etc.)
    #[serde(default)]
    pub response_metadata: HashMap<String, serde_json::Value>,
}

/// Core message enum representing all message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    /// Message from a human/user
    Human {
        /// Message content
        content: MessageContent,
        /// Common message fields
        #[serde(flatten)]
        fields: BaseMessageFields,
    },

    /// Message from an AI assistant
    #[serde(rename = "ai")]
    AI {
        /// Message content
        content: MessageContent,
        /// Tool calls made by the AI
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        /// Invalid tool calls (malformed)
        #[serde(default)]
        invalid_tool_calls: Vec<InvalidToolCall>,
        /// Usage metadata (token counts)
        usage_metadata: Option<UsageMetadata>,
        /// Common message fields
        #[serde(flatten)]
        fields: BaseMessageFields,
    },

    /// System message for priming AI behavior
    System {
        /// Message content
        content: MessageContent,
        /// Common message fields
        #[serde(flatten)]
        fields: BaseMessageFields,
    },

    /// Tool/function call message
    Tool {
        /// Tool result content
        content: MessageContent,
        /// ID of the tool call this is responding to
        tool_call_id: String,
        /// Optional artifact data
        artifact: Option<serde_json::Value>,
        /// Status of the tool execution
        status: Option<String>,
        /// Common message fields
        #[serde(flatten)]
        fields: BaseMessageFields,
    },

    /// Function call message (for `OpenAI`'s older function calling API, prefer Tool for new code)
    Function {
        /// Function result content
        content: MessageContent,
        /// Name of the function
        name: String,
        /// Common message fields
        #[serde(flatten)]
        fields: BaseMessageFields,
    },
}

impl Message {
    /// Create a human message
    pub fn human(content: impl Into<MessageContent>) -> Self {
        Message::Human {
            content: content.into(),
            fields: BaseMessageFields::default(),
        }
    }

    /// Create an AI message
    pub fn ai(content: impl Into<MessageContent>) -> Self {
        Message::AI {
            content: content.into(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: BaseMessageFields::default(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Message::System {
            content: content.into(),
            fields: BaseMessageFields::default(),
        }
    }

    /// Create a tool message
    pub fn tool(content: impl Into<MessageContent>, tool_call_id: impl Into<String>) -> Self {
        Message::Tool {
            content: content.into(),
            tool_call_id: tool_call_id.into(),
            artifact: None,
            status: None,
            fields: BaseMessageFields::default(),
        }
    }

    /// Get the message content
    #[must_use]
    pub fn content(&self) -> &MessageContent {
        match self {
            Message::Human { content, .. } => content,
            Message::AI { content, .. } => content,
            Message::System { content, .. } => content,
            Message::Tool { content, .. } => content,
            Message::Function { content, .. } => content,
        }
    }

    /// Get the message content as text
    #[must_use]
    pub fn as_text(&self) -> String {
        self.content().as_text()
    }

    /// Get the message type as a string
    #[must_use]
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::Human { .. } => "human",
            Message::AI { .. } => "ai",
            Message::System { .. } => "system",
            Message::Tool { .. } => "tool",
            Message::Function { .. } => "function",
        }
    }

    /// Check if this is a human message
    #[must_use]
    pub fn is_human(&self) -> bool {
        matches!(self, Message::Human { .. })
    }

    /// Check if this is an AI message
    #[must_use]
    pub fn is_ai(&self) -> bool {
        matches!(self, Message::AI { .. })
    }

    /// Check if this is a system message
    #[must_use]
    pub fn is_system(&self) -> bool {
        matches!(self, Message::System { .. })
    }

    /// Get the fields (common metadata)
    #[must_use]
    pub fn fields(&self) -> &BaseMessageFields {
        match self {
            Message::Human { fields, .. } => fields,
            Message::AI { fields, .. } => fields,
            Message::System { fields, .. } => fields,
            Message::Tool { fields, .. } => fields,
            Message::Function { fields, .. } => fields,
        }
    }

    /// Get mutable fields
    pub fn fields_mut(&mut self) -> &mut BaseMessageFields {
        match self {
            Message::Human { fields, .. } => fields,
            Message::AI { fields, .. } => fields,
            Message::System { fields, .. } => fields,
            Message::Tool { fields, .. } => fields,
            Message::Function { fields, .. } => fields,
        }
    }

    /// Set the name field on this message (builder pattern)
    ///
    /// The name field can be used to identify the speaker or role in a conversation.
    /// For example, "`example_user`" or "`customer_support`".
    ///
    /// # Example
    /// ```
    /// # use dashflow::core::messages::Message;
    /// let msg = Message::human("hello").with_name("example_user");
    /// ```
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.fields_mut().name = Some(name.into());
        self
    }

    /// Create a human message with content blocks
    #[must_use]
    pub fn human_with_blocks(blocks: Vec<ContentBlock>) -> Self {
        Message::Human {
            content: MessageContent::Blocks(blocks),
            fields: BaseMessageFields::default(),
        }
    }

    /// Create an AI message with content blocks
    #[must_use]
    pub fn ai_with_blocks(blocks: Vec<ContentBlock>) -> Self {
        Message::AI {
            content: MessageContent::Blocks(blocks),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: BaseMessageFields::default(),
        }
    }

    /// Get tool calls from an AI message
    ///
    /// Returns an empty slice for non-AI messages.
    #[must_use]
    pub fn tool_calls(&self) -> &[ToolCall] {
        match self {
            Message::AI { tool_calls, .. } => tool_calls,
            _ => &[],
        }
    }

    /// Get invalid tool calls from an AI message
    ///
    /// Returns an empty slice for non-AI messages.
    #[must_use]
    pub fn invalid_tool_calls(&self) -> &[InvalidToolCall] {
        match self {
            Message::AI {
                invalid_tool_calls, ..
            } => invalid_tool_calls,
            _ => &[],
        }
    }

    /// Get the tool call ID from a Tool message
    ///
    /// Returns `None` for non-Tool messages.
    #[must_use]
    pub fn tool_call_id(&self) -> Option<&str> {
        match self {
            Message::Tool { tool_call_id, .. } => Some(tool_call_id),
            _ => None,
        }
    }
}

/// Trait for types that can be converted to LLM messages.
///
/// Applications can implement this trait for their custom message types
/// to enable seamless integration with DashFlow's context management,
/// token counting, and LLM client APIs.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{IntoLlmMessage, ToolCall};
///
/// struct MyAppMessage {
///     role: String,
///     content: String,
/// }
///
/// impl IntoLlmMessage for MyAppMessage {
///     fn role(&self) -> &str {
///         &self.role
///     }
///
///     fn content(&self) -> &str {
///         &self.content
///     }
///
///     fn tool_calls(&self) -> Option<&[ToolCall]> {
///         None
///     }
///
///     fn tool_call_id(&self) -> Option<&str> {
///         None
///     }
/// }
/// ```
pub trait IntoLlmMessage {
    /// Get the role of the message (e.g., "human", "ai", "system", "tool")
    fn role(&self) -> &str;

    /// Get the text content of the message
    fn content(&self) -> &str;

    /// Get tool calls from the message (for AI messages that request tool execution)
    fn tool_calls(&self) -> Option<&[ToolCall]>;

    /// Get the tool call ID (for Tool messages responding to a tool call)
    fn tool_call_id(&self) -> Option<&str>;
}

impl IntoLlmMessage for Message {
    fn role(&self) -> &str {
        self.message_type()
    }

    fn content(&self) -> &str {
        match self.content() {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Blocks(blocks) => {
                // For blocks, return first text block content or empty string
                // Note: This is a simplified implementation - full content
                // extraction may need as_text() which allocates
                blocks
                    .iter()
                    .find_map(|block| {
                        if let ContentBlock::Text { text } = block {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("")
            }
        }
    }

    fn tool_calls(&self) -> Option<&[ToolCall]> {
        match self {
            Message::AI { tool_calls, .. } if !tool_calls.is_empty() => Some(tool_calls),
            _ => None,
        }
    }

    fn tool_call_id(&self) -> Option<&str> {
        Message::tool_call_id(self)
    }
}

/// Borrowed view of a message for conversion to DashFlow's message types.
///
/// This struct provides a uniform representation of message data that can be
/// converted to DashFlow's `BaseMessage` type using [`to_base_message()`](LlmMessageRef::to_base_message).
///
/// Use this with [`AsLlmMessage`] to enable your custom message types to work
/// directly with `dashflow::generate()` without manual conversion.
#[derive(Debug, Clone)]
pub struct LlmMessageRef<'a> {
    /// Role: "user", "human", "assistant", "ai", "system", "tool"
    pub role: &'a str,
    /// Message content
    pub content: &'a str,
    /// Tool calls (for assistant messages)
    pub tool_calls: Option<&'a [ToolCall]>,
    /// Tool call ID (for tool response messages)
    pub tool_call_id: Option<&'a str>,
    /// Optional name
    pub name: Option<&'a str>,
}

impl LlmMessageRef<'_> {
    /// Convert to DashFlow's BaseMessage.
    ///
    /// This handles the mapping from role strings to the appropriate message type,
    /// including setting tool calls for AI messages and tool call IDs for tool messages.
    #[must_use]
    pub fn to_base_message(&self) -> BaseMessage {
        let mut fields = BaseMessageFields::default();
        if let Some(name) = self.name {
            fields.name = Some(name.to_string());
        }

        match self.role {
            "user" | "human" => Message::Human {
                content: MessageContent::Text(self.content.to_string()),
                fields,
            },
            "assistant" | "ai" => {
                let tool_calls = self.tool_calls.map(|tc| tc.to_vec()).unwrap_or_default();
                Message::AI {
                    content: MessageContent::Text(self.content.to_string()),
                    tool_calls,
                    invalid_tool_calls: Vec::new(),
                    usage_metadata: None,
                    fields,
                }
            }
            "system" | "developer" => Message::System {
                content: MessageContent::Text(self.content.to_string()),
                fields,
            },
            "tool" => Message::Tool {
                content: MessageContent::Text(self.content.to_string()),
                tool_call_id: self.tool_call_id.unwrap_or("").to_string(),
                artifact: None,
                status: None,
                fields,
            },
            // Fallback: treat unknown roles as human messages
            _ => Message::Human {
                content: MessageContent::Text(self.content.to_string()),
                fields,
            },
        }
    }
}

/// Trait for types that can be viewed as LLM messages.
///
/// Implement this trait for your application's message type to use it
/// directly with `dashflow::generate()` without manual conversion.
///
/// This is the recommended way to integrate custom message types with DashFlow.
/// Unlike the older [`IntoLlmMessage`] trait, this returns a unified [`LlmMessageRef`]
/// struct that can be easily converted to DashFlow's native message types.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{AsLlmMessage, LlmMessageRef};
///
/// struct MyMessage {
///     role: String,
///     content: String,
/// }
///
/// impl AsLlmMessage for MyMessage {
///     fn as_llm_message(&self) -> LlmMessageRef<'_> {
///         LlmMessageRef {
///             role: &self.role,
///             content: &self.content,
///             tool_calls: None,
///             tool_call_id: None,
///             name: None,
///         }
///     }
/// }
///
/// // Now use directly:
/// // dashflow::generate(model, &my_messages).await?
/// ```
pub trait AsLlmMessage {
    /// Get a borrowed view of this message's data.
    fn as_llm_message(&self) -> LlmMessageRef<'_>;
}

impl AsLlmMessage for Message {
    fn as_llm_message(&self) -> LlmMessageRef<'_> {
        // Get content as string reference
        let content = match self.content() {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Blocks(blocks) => {
                // For blocks, return first text block content or empty string
                blocks
                    .iter()
                    .find_map(|block| {
                        if let ContentBlock::Text { text } = block {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("")
            }
        };

        LlmMessageRef {
            role: self.message_type(),
            content,
            tool_calls: match self {
                Message::AI { tool_calls, .. } if !tool_calls.is_empty() => Some(tool_calls),
                _ => None,
            },
            tool_call_id: match self {
                Message::Tool { tool_call_id, .. } => Some(tool_call_id.as_str()),
                _ => None,
            },
            name: self.fields().name.as_deref(),
        }
    }
}

impl AsLlmMessage for HumanMessage {
    fn as_llm_message(&self) -> LlmMessageRef<'_> {
        let content = match &self.content {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .find_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or(""),
        };

        LlmMessageRef {
            role: "human",
            content,
            tool_calls: None,
            tool_call_id: None,
            name: self.fields.name.as_deref(),
        }
    }
}

impl AsLlmMessage for AIMessage {
    fn as_llm_message(&self) -> LlmMessageRef<'_> {
        let content = match &self.content {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Blocks(_) => "",
        };

        LlmMessageRef {
            role: "ai",
            content,
            tool_calls: if self.tool_calls.is_empty() {
                None
            } else {
                Some(&self.tool_calls)
            },
            tool_call_id: None,
            name: self.fields.name.as_deref(),
        }
    }
}

/// Type alias for `BaseMessage` (any message type)
pub type BaseMessage = Message;

/// `HumanMessage` is a convenience wrapper for Human messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HumanMessage {
    content: MessageContent,
    fields: BaseMessageFields,
}

impl HumanMessage {
    /// Create a new `HumanMessage`
    pub fn new(content: impl Into<MessageContent>) -> Self {
        Self {
            content: content.into(),
            fields: BaseMessageFields::default(),
        }
    }

    /// Get the content
    #[must_use]
    pub fn content(&self) -> &MessageContent {
        &self.content
    }
}

impl From<HumanMessage> for Message {
    fn from(msg: HumanMessage) -> Self {
        Message::Human {
            content: msg.content,
            fields: msg.fields,
        }
    }
}

/// `AIMessage` is a convenience wrapper for AI messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIMessage {
    content: MessageContent,
    tool_calls: Vec<ToolCall>,
    invalid_tool_calls: Vec<InvalidToolCall>,
    usage_metadata: Option<UsageMetadata>,
    fields: BaseMessageFields,
}

impl AIMessage {
    /// Create a new `AIMessage`
    pub fn new(content: impl Into<MessageContent>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: BaseMessageFields::default(),
        }
    }

    /// Get the text content of this message.
    ///
    /// # Returns
    ///
    /// For `MessageContent::Text`, returns the text string.
    /// For `MessageContent::Blocks`, returns empty string (M-986: blocks content
    /// requires `Message::as_text()` which allocates to concatenate text blocks).
    ///
    /// Use `Message::from(self).as_text()` if you need to extract text from blocks.
    #[must_use]
    pub fn content(&self) -> &str {
        match &self.content {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Blocks(_) => "",
        }
    }

    /// Add tool calls
    #[must_use]
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Add usage metadata
    #[must_use]
    pub fn with_usage(mut self, usage_metadata: UsageMetadata) -> Self {
        self.usage_metadata = Some(usage_metadata);
        self
    }

    /// Add response metadata
    #[must_use]
    pub fn with_response_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.fields.response_metadata = metadata;
        self
    }

    /// Get response metadata
    #[must_use]
    pub fn response_metadata(&self) -> &HashMap<String, serde_json::Value> {
        &self.fields.response_metadata
    }
}

impl From<AIMessage> for Message {
    fn from(msg: AIMessage) -> Self {
        Message::AI {
            content: msg.content,
            tool_calls: msg.tool_calls,
            invalid_tool_calls: msg.invalid_tool_calls,
            usage_metadata: msg.usage_metadata,
            fields: msg.fields,
        }
    }
}

/// `AIMessageChunk` is a streamable chunk of an AI message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIMessageChunk {
    /// The text content of this chunk.
    pub content: String,
    /// Tool calls contained in this chunk.
    pub tool_calls: Vec<ToolCall>,
    /// Invalid tool calls that failed to parse.
    pub invalid_tool_calls: Vec<InvalidToolCall>,
    /// Token usage metadata for this chunk.
    pub usage_metadata: Option<UsageMetadata>,
    /// Base message fields inherited from parent message types.
    pub fields: BaseMessageFields,
}

impl AIMessageChunk {
    /// Create a new `AIMessageChunk`
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: BaseMessageFields::default(),
        }
    }

    /// Merge another chunk into this one
    #[must_use]
    pub fn merge(&self, other: AIMessageChunk) -> AIMessageChunk {
        let mut merged = self.clone();
        merged.content.push_str(&other.content);

        // Merge tool calls using index-aware logic (Python: libs/core/dashflow_core/utils/_merge.py:merge_lists)
        for other_tc in other.tool_calls {
            if let Some(index) = other_tc.index {
                // Look for existing tool call with same index
                if let Some(pos) = merged
                    .tool_calls
                    .iter()
                    .position(|tc| tc.index == Some(index))
                {
                    // Merge with existing tool call at this index
                    let existing = &mut merged.tool_calls[pos];

                    // Merge name (concatenate strings)
                    existing.name.push_str(&other_tc.name);

                    // Merge args
                    // Support for streaming tool call arguments
                    // Some providers send args incrementally as string chunks (e.g., "{"", "input", "\":", "3", "}")
                    // We concatenate strings during streaming, then parse complete JSON in to_message()
                    match (&mut existing.args, &other_tc.args) {
                        // Both are strings: concatenate (this is the streaming case)
                        (
                            serde_json::Value::String(existing_str),
                            serde_json::Value::String(other_str),
                        ) => {
                            existing_str.push_str(other_str);
                        }
                        // Existing is string, other is not: concatenate if other is string-like, else keep existing
                        (serde_json::Value::String(existing_str), _) => {
                            if let Some(other_str) = other_tc.args.as_str() {
                                existing_str.push_str(other_str);
                            }
                        }
                        // Other is string, existing is not: if existing is empty/null, use other string
                        (_, serde_json::Value::String(other_str)) => {
                            if existing.args.is_null()
                                || existing.args.as_object().is_some_and(|o| o.is_empty())
                            {
                                existing.args = serde_json::Value::String(other_str.clone());
                            }
                        }
                        // Both are objects: merge recursively (non-streaming case)
                        (
                            serde_json::Value::Object(existing_obj),
                            serde_json::Value::Object(other_obj),
                        ) if !other_obj.is_empty() => {
                            merge_json_objects(existing_obj, other_obj);
                        }
                        // Other is non-empty object: use it
                        (_, serde_json::Value::Object(other_obj)) if !other_obj.is_empty() => {
                            existing.args = other_tc.args.clone();
                        }
                        // Existing is non-empty object: keep it
                        (serde_json::Value::Object(existing_obj), _)
                            if !existing_obj.is_empty() =>
                        {
                            // Keep existing
                        }
                        // Default: keep existing unless it's null/empty
                        _ => {
                            if (existing.args.is_null()
                                || existing.args.as_object().is_some_and(|o| o.is_empty()))
                                && !other_tc.args.is_null()
                            {
                                existing.args = other_tc.args.clone();
                            }
                        }
                    }

                    // ID: if existing is empty, use other; otherwise concatenate
                    if existing.id.is_empty() {
                        existing.id = other_tc.id;
                    } else if !other_tc.id.is_empty() && existing.id != other_tc.id {
                        existing.id.push_str(&other_tc.id);
                    }

                    // index and tool_type stay the same (don't concatenate)
                } else {
                    // No existing tool call with this index, append as new
                    merged.tool_calls.push(other_tc);
                }
            } else {
                // No index field, just append
                merged.tool_calls.push(other_tc);
            }
        }

        merged.invalid_tool_calls.extend(other.invalid_tool_calls);

        // Merge usage metadata if present (M-984: preserve token details)
        if let (Some(usage1), Some(usage2)) = (&merged.usage_metadata, &other.usage_metadata) {
            merged.usage_metadata = Some(UsageMetadata {
                input_tokens: usage1.input_tokens + usage2.input_tokens,
                output_tokens: usage1.output_tokens + usage2.output_tokens,
                total_tokens: usage1.total_tokens + usage2.total_tokens,
                // Prefer second chunk's details (streaming: final chunk typically has totals)
                // Fall back to first chunk's details if second is None
                input_token_details: usage2
                    .input_token_details
                    .clone()
                    .or_else(|| usage1.input_token_details.clone()),
                output_token_details: usage2
                    .output_token_details
                    .clone()
                    .or_else(|| usage1.output_token_details.clone()),
            });
        } else if other.usage_metadata.is_some() {
            merged.usage_metadata = other.usage_metadata;
        }

        // Merge fields
        merged
            .fields
            .additional_kwargs
            .extend(other.fields.additional_kwargs);
        merged
            .fields
            .response_metadata
            .extend(other.fields.response_metadata);

        merged
    }

    /// Convert to a full `AIMessage`
    #[must_use]
    pub fn to_message(&self) -> AIMessage {
        // Parse any String args into JSON (streaming support)
        // During streaming, providers may send args as incremental string chunks which we accumulate
        // Now parse the complete accumulated string into proper JSON
        let tool_calls = self
            .tool_calls
            .iter()
            .map(|tc| {
                let args = if let serde_json::Value::String(args_str) = &tc.args {
                    // This is an accumulated args string from streaming - parse it now
                    serde_json::from_str(args_str).unwrap_or_else(|e| {
                        // M-985: Log warning when streaming args parsing fails
                        tracing::warn!(
                            tool_call_id = %tc.id,
                            tool_name = %tc.name,
                            args_str = %args_str,
                            error = %e,
                            "Failed to parse tool call args; using empty object"
                        );
                        serde_json::json!({})
                    })
                } else {
                    // Already a parsed value (non-streaming case)
                    tc.args.clone()
                };

                ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    args,
                    tool_type: tc.tool_type.clone(),
                    index: tc.index,
                }
            })
            .collect();

        AIMessage {
            content: MessageContent::Text(self.content.clone()),
            tool_calls,
            invalid_tool_calls: self.invalid_tool_calls.clone(),
            usage_metadata: self.usage_metadata.clone(),
            fields: self.fields.clone(),
        }
    }
}

impl From<AIMessageChunk> for Message {
    fn from(chunk: AIMessageChunk) -> Self {
        chunk.to_message().into()
    }
}

/// Helper function to check if a message matches the specified type(s)
///
/// Types can be specified as strings (e.g., "human", "ai", "system") or as type names.
/// This matches Python's `_is_message_type()` helper.
fn is_message_type(message: &Message, types: &[MessageTypeFilter]) -> bool {
    for type_filter in types {
        match type_filter {
            MessageTypeFilter::String(type_str) => {
                if message.message_type() == type_str.as_str() {
                    return true;
                }
            }
            MessageTypeFilter::Type(type_enum) => match (message, type_enum) {
                (Message::Human { .. }, MessageType::Human) => return true,
                (Message::AI { .. }, MessageType::AI) => return true,
                (Message::System { .. }, MessageType::System) => return true,
                (Message::Tool { .. }, MessageType::Tool) => return true,
                (Message::Function { .. }, MessageType::Function) => return true,
                _ => {}
            },
        }
    }
    false
}

/// Type filter for message filtering
#[derive(Debug, Clone)]
pub enum MessageTypeFilter {
    /// String type (e.g., "human", "ai", "system")
    String(String),
    /// Enum type
    Type(MessageType),
}

/// Message type enum representing the role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// A message from a human user.
    Human,
    /// A message from an AI assistant.
    AI,
    /// A system prompt or instruction.
    System,
    /// A tool/function response.
    Tool,
    /// A function call (deprecated, use Tool instead).
    Function,
}

/// Trimming strategy for `trim_messages`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimStrategy {
    /// Keep first N tokens
    First,
    /// Keep last N tokens (most common for chat history)
    Last,
}

/// Error type for `trim_messages` operations
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TrimError {
    /// The token counter returned a negative or otherwise invalid value.
    #[error("Token counter returned negative or invalid value")]
    InvalidTokenCount,
    /// The messages cannot be trimmed to meet the constraints.
    #[error("Cannot trim: {0}")]
    CannotTrim(String),
    /// Invalid parameters were provided to the trim operation.
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
}

impl From<&str> for MessageTypeFilter {
    fn from(s: &str) -> Self {
        MessageTypeFilter::String(s.to_string())
    }
}

impl From<String> for MessageTypeFilter {
    fn from(s: String) -> Self {
        MessageTypeFilter::String(s)
    }
}

impl From<MessageType> for MessageTypeFilter {
    fn from(t: MessageType) -> Self {
        MessageTypeFilter::Type(t)
    }
}

/// Default text splitter for partial message trimming
///
/// Splits text on newlines, preserving separators so splits can be rejoined.
/// This matches Python's `_default_text_splitter()` behavior.
///
/// # Example
///
/// ```no_run
/// # // This function is private and will be made public when partial message support is added
/// # fn default_text_splitter(text: &str) -> Vec<String> {
/// #     if text.is_empty() {
/// #         return vec![];
/// #     }
/// #     let mut splits: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
/// #     if splits.len() > 1 {
/// #         for i in 0..splits.len() - 1 {
/// #             splits[i].push('\n');
/// #         }
/// #     }
/// #     splits
/// # }
/// let text = "line1\nline2\nline3";
/// let splits = default_text_splitter(text);
/// assert_eq!(splits, vec!["line1\n", "line2\n", "line3"]);
/// assert_eq!(splits.join(""), text); // Can be rejoined
/// ```
fn default_text_splitter(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    let mut splits: Vec<String> = text
        .split('\n')
        .map(std::string::ToString::to_string)
        .collect();

    // Add newlines back to all but the last split (preserves separators)
    if splits.len() > 1 {
        for i in 0..splits.len() - 1 {
            splits[i].push('\n');
        }
    }

    // Filter out empty splits (can occur when text ends with \n)
    splits.retain(|s| !s.is_empty());

    splits
}

/// Filter messages based on name, type, ID, or tool calls
///
/// This function provides flexible filtering of message sequences based on multiple criteria.
/// You can include or exclude messages by name, type, ID, or tool call presence.
///
/// # Arguments
///
/// * `messages` - Sequence of messages to filter
/// * `include_names` - Optional list of names to include (whitelist)
/// * `exclude_names` - Optional list of names to exclude (blacklist)
/// * `include_types` - Optional list of types to include (whitelist)
/// * `exclude_types` - Optional list of types to exclude (blacklist)
/// * `include_ids` - Optional list of IDs to include (whitelist)
/// * `exclude_ids` - Optional list of IDs to exclude (blacklist)
/// * `exclude_tool_calls` - Optional tool call exclusion:
///   - `Some(ExcludeToolCalls::All)`: Exclude all `AIMessage` with tool calls and all `ToolMessage`
///   - `Some(ExcludeToolCalls::Ids(ids))`: Exclude specific tool call IDs
///   - `None`: No tool call filtering
///
/// # Returns
///
/// A list of messages that meet at least one of the include conditions (if any) and none
/// of the exclude conditions. If no include conditions are specified, anything that is not
/// explicitly excluded will be included.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{Message, filter_messages};
///
/// let messages = vec![
///     Message::system("you're a good assistant."),
///     Message::human("what's your name").with_name("example_user"),
///     Message::ai("steve-o").with_name("example_assistant"),
///     Message::human("what's your favorite color"),
///     Message::ai("silicon blue"),
/// ];
///
/// let filtered = filter_messages(
///     messages,
///     Some(&["example_user".into(), "example_assistant".into()]),
///     None,
///     Some(&["system".into()]),
///     None,
///     None,
///     Some(&["bar".into()]),
///     None,
/// );
/// // Result: [SystemMessage(...), HumanMessage(name="example_user")]
/// ```
#[must_use]
#[allow(clippy::too_many_arguments)] // Message filtering API: include/exclude by name, type, id, tool_calls
#[allow(clippy::needless_pass_by_value)] // exclude_tool_calls by value for simpler API (avoids lifetime complexity)
pub fn filter_messages(
    messages: Vec<Message>,
    include_names: Option<&[String]>,
    exclude_names: Option<&[String]>,
    include_types: Option<&[MessageTypeFilter]>,
    exclude_types: Option<&[MessageTypeFilter]>,
    include_ids: Option<&[String]>,
    exclude_ids: Option<&[String]>,
    exclude_tool_calls: Option<ExcludeToolCalls>,
) -> Vec<Message> {
    let mut filtered: Vec<Message> = Vec::new();

    for mut msg in messages {
        // Extract immutable data first (before any mutable operations)
        let msg_name = msg.fields().name.clone();
        let msg_id = msg.fields().id.clone();
        let msg_type_matches_exclude =
            exclude_types.is_some_and(|types| is_message_type(&msg, types));
        let msg_type_matches_include =
            include_types.is_some_and(|types| is_message_type(&msg, types));

        // Check exclusion criteria
        // Exclude by name
        if let (Some(names), Some(name)) = (exclude_names, &msg_name) {
            if names.contains(name) {
                continue;
            }
        }

        // Exclude by type
        if msg_type_matches_exclude {
            continue;
        }

        // Exclude by ID
        if let (Some(ids), Some(id)) = (exclude_ids, &msg_id) {
            if ids.contains(id) {
                continue;
            }
        }

        // Exclude tool calls (all)
        if let Some(ExcludeToolCalls::All) = exclude_tool_calls {
            match &msg {
                Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
                    continue;
                }
                Message::Tool { .. } => {
                    continue;
                }
                _ => {}
            }
        }

        // Exclude specific tool call IDs (this requires mutation)
        if let Some(ExcludeToolCalls::Ids(exclude_ids)) = &exclude_tool_calls {
            match &mut msg {
                Message::AI {
                    tool_calls,
                    content,
                    ..
                } => {
                    if !tool_calls.is_empty() {
                        // Filter out excluded tool calls
                        tool_calls.retain(|tc| !exclude_ids.contains(&tc.id));

                        // If no tool calls remain, skip this message
                        if tool_calls.is_empty() {
                            continue;
                        }

                        // Handle content blocks with tool use
                        if let MessageContent::Blocks(blocks) = content {
                            blocks.retain(|block| {
                                if let ContentBlock::ToolUse { id, .. } = block {
                                    !exclude_ids.contains(id)
                                } else {
                                    true
                                }
                            });
                        }
                    }
                }
                Message::Tool { tool_call_id, .. } => {
                    if exclude_ids.contains(tool_call_id) {
                        continue;
                    }
                }
                _ => {}
            }
        }

        // Check inclusion criteria (default to inclusion if no criteria specified)
        let has_include_criteria =
            include_types.is_some() || include_ids.is_some() || include_names.is_some();

        if !has_include_criteria {
            // No inclusion criteria, include by default
            filtered.push(msg);
            continue;
        }

        // Check if message meets any inclusion criteria
        let mut should_include = false;

        if let Some(names) = include_names {
            if let Some(ref name) = msg_name {
                if names.contains(name) {
                    should_include = true;
                }
            }
        }

        if msg_type_matches_include {
            should_include = true;
        }

        if let Some(ids) = include_ids {
            if let Some(ref id) = msg_id {
                if ids.contains(id) {
                    should_include = true;
                }
            }
        }

        if should_include {
            filtered.push(msg);
        }
    }

    filtered
}

/// Tool call exclusion options
#[derive(Debug, Clone)]
pub enum ExcludeToolCalls {
    /// Exclude all tool calls
    All,
    /// Exclude specific tool call IDs
    Ids(Vec<String>),
}

/// Partial message strategy for `trim_messages`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PartialStrategy {
    /// Keep first tokens of partial message
    First,
    /// Keep last tokens of partial message
    Last,
}

/// Helper function to keep first N tokens from messages
///
/// This implements the core algorithm for trimming messages to fit within a token budget
/// using a "first" strategy (keep earliest messages).
///
/// # Arguments
///
/// * `messages` - Sequence of messages to trim
/// * `max_tokens` - Maximum number of tokens to keep
/// * `token_counter` - Function to count tokens in a slice of messages
/// * `text_splitter` - Function to split text into chunks
/// * `partial_strategy` - If Some, allow partial messages using specified strategy
/// * `end_on` - Optional types to end on (remove messages after last occurrence)
///
/// # Returns
///
/// A list of messages that fit within the token budget
fn _first_max_tokens<F>(
    messages: Vec<Message>,
    max_tokens: usize,
    token_counter: F,
    text_splitter: fn(&str) -> Vec<String>,
    partial_strategy: Option<PartialStrategy>,
    end_on: Option<&[MessageTypeFilter]>,
) -> Result<Vec<Message>, TrimError>
where
    F: Fn(&[Message]) -> usize,
{
    let total_tokens = token_counter(&messages);

    // Fast path: all messages fit
    if total_tokens <= max_tokens {
        return Ok(if let Some(end_types) = end_on {
            apply_end_on(messages, end_types)
        } else {
            messages
        });
    }

    // Binary search for max number of messages that fit
    let mut left = 0;
    let mut right = messages.len();

    // Number of iterations needed for binary search
    // Safety: log2 of any valid Vec length (max usize) produces a small value
    // that fits comfortably in usize. On 64-bit systems, log2(2^64) = 64, so
    // iterations <= 65, well within usize range. The +1 handles edge cases.
    let iterations = if messages.is_empty() {
        0
    } else {
        (messages.len() as f64).log2().ceil() as usize + 1
    };

    for _ in 0..iterations {
        let mid = (left + right).div_ceil(2);
        if mid > messages.len() {
            break;
        }

        let tokens = token_counter(&messages[..mid]);
        if tokens <= max_tokens {
            left = mid;
        } else {
            right = mid - 1;
        }
    }

    let idx = left;

    // Try to include partial message if allowed
    let mut result: Vec<Message> = if let Some(strategy) = partial_strategy {
        if idx < messages.len() {
            if let Some(partial_msg) = try_add_partial_message(
                &messages,
                idx,
                max_tokens,
                &token_counter,
                text_splitter,
                strategy,
            )? {
                // Successfully added partial message
                let mut msgs: Vec<Message> = messages.iter().take(idx).cloned().collect();
                msgs.push(partial_msg);
                msgs
            } else {
                // Couldn't add partial, just take idx messages
                messages.into_iter().take(idx).collect()
            }
        } else {
            messages.into_iter().take(idx).collect()
        }
    } else {
        messages.into_iter().take(idx).collect()
    };

    // Apply end_on filtering if specified
    if let Some(end_types) = end_on {
        result = apply_end_on(result, end_types);
    }

    Ok(result)
}

/// Try to add a partial message to fit within token budget
///
/// Returns `Some(partial_message)` if we can fit part of the message, None otherwise
fn try_add_partial_message<F>(
    messages: &[Message],
    idx: usize,
    max_tokens: usize,
    token_counter: F,
    text_splitter: fn(&str) -> Vec<String>,
    strategy: PartialStrategy,
) -> Result<Option<Message>, TrimError>
where
    F: Fn(&[Message]) -> usize,
{
    if idx >= messages.len() {
        return Ok(None);
    }

    let base_message_count = token_counter(&messages[..idx]);
    if base_message_count >= max_tokens {
        return Ok(None);
    }

    let mut excluded = messages[idx].clone();

    // Try to trim content blocks first (if content is a list)
    if let Some(partial) = try_trim_content_blocks(
        &mut excluded,
        messages,
        idx,
        max_tokens,
        &token_counter,
        strategy,
    )? {
        return Ok(Some(partial));
    }

    // Try to trim text content
    try_trim_text_content(
        &mut excluded,
        messages,
        idx,
        max_tokens,
        &token_counter,
        text_splitter,
        strategy,
    )
}

/// Try to trim content blocks from a message
fn try_trim_content_blocks<F>(
    excluded: &mut Message,
    messages: &[Message],
    idx: usize,
    max_tokens: usize,
    token_counter: F,
    strategy: PartialStrategy,
) -> Result<Option<Message>, TrimError>
where
    F: Fn(&[Message]) -> usize,
{
    // Check if this message has content blocks and extract them
    let blocks_opt = match excluded {
        Message::AI { content, .. } | Message::Human { content, .. } => {
            if let MessageContent::Blocks(blocks) = content {
                Some(blocks.clone())
            } else {
                None
            }
        }
        _ => None,
    };

    let mut blocks = match blocks_opt {
        Some(b) if !b.is_empty() => b,
        _ => return Ok(None),
    };

    let num_blocks = blocks.len();

    // Reverse blocks if strategy is Last
    if strategy == PartialStrategy::Last {
        blocks.reverse();
    }

    // Try removing blocks one by one until it fits
    for i in 1..num_blocks {
        let mut test_blocks = blocks[..num_blocks - i].to_vec();

        // Reverse back for testing if needed
        if strategy == PartialStrategy::Last {
            test_blocks.reverse();
        }

        // Create test message with trimmed blocks
        let mut test_excluded = excluded.clone();
        match &mut test_excluded {
            Message::AI { content, .. } | Message::Human { content, .. } => {
                *content = MessageContent::Blocks(test_blocks.clone());
            }
            _ => {}
        }

        let mut test_msgs = messages[..idx].to_vec();
        test_msgs.push(test_excluded.clone());

        if token_counter(&test_msgs) <= max_tokens {
            // Found a fit! Update the original message
            match excluded {
                Message::AI { content, .. } | Message::Human { content, .. } => {
                    *content = MessageContent::Blocks(test_blocks);
                }
                _ => {}
            }
            return Ok(Some(excluded.clone()));
        }
    }

    Ok(None)
}

/// Try to trim text content from a message
fn try_trim_text_content<F>(
    excluded: &mut Message,
    messages: &[Message],
    idx: usize,
    max_tokens: usize,
    token_counter: F,
    text_splitter: fn(&str) -> Vec<String>,
    strategy: PartialStrategy,
) -> Result<Option<Message>, TrimError>
where
    F: Fn(&[Message]) -> usize,
{
    // Extract text content
    let text = match excluded {
        Message::AI { content, .. }
        | Message::Human { content, .. }
        | Message::System { content, .. } => {
            match content {
                MessageContent::Text(s) => Some(s.clone()),
                MessageContent::Blocks(blocks) => {
                    // Find first text block
                    for block in blocks {
                        if let ContentBlock::Text { text: _ } = block {
                            return Ok(None); // For now, skip blocks with text
                        }
                    }
                    None
                }
            }
        }
        _ => None,
    };

    let text = match text {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(None),
    };

    // Split text
    let split_texts = text_splitter(&text);
    if split_texts.is_empty() {
        return Ok(None);
    }

    let num_splits = split_texts.len();

    // Binary search for max number of splits that fit
    let _base_message_count = token_counter(&messages[..idx]);
    let mut left = 0;
    let mut right = num_splits;

    // Safety: log2 of any valid usize produces a small value (max ~64 on 64-bit).
    let iterations = if num_splits > 0 {
        (num_splits as f64).log2().ceil() as usize + 1
    } else {
        0
    };

    for _iter in 0..iterations {
        if left >= right {
            break;
        }
        let mid = (left + right).div_ceil(2);

        // Create test message with partial content
        // For Last strategy, take from the end; for First, take from the start
        let partial_content = if strategy == PartialStrategy::Last {
            split_texts[num_splits - mid..].join("")
        } else {
            split_texts[..mid].join("")
        };

        let mut test_msg = excluded.clone();
        match &mut test_msg {
            Message::AI { content, .. }
            | Message::Human { content, .. }
            | Message::System { content, .. } => {
                *content = MessageContent::Text(partial_content.clone());
            }
            _ => {}
        }

        let mut test_msgs = messages[..idx].to_vec();
        test_msgs.push(test_msg);

        let total_tokens = token_counter(&test_msgs);
        let fits = total_tokens <= max_tokens;

        if fits {
            left = mid;
        } else {
            right = mid - 1;
        }
    }

    if left > 0 {
        // We can include some splits
        let partial_content = if strategy == PartialStrategy::Last {
            split_texts[num_splits - left..].join("")
        } else {
            split_texts[..left].join("")
        };

        match excluded {
            Message::AI { content, .. }
            | Message::Human { content, .. }
            | Message::System { content, .. } => {
                *content = MessageContent::Text(partial_content);
            }
            _ => {}
        }

        Ok(Some(excluded.clone()))
    } else {
        Ok(None)
    }
}

/// Apply `end_on` filtering: remove messages from the end until we hit a message of the specified type
/// If no matching message is found, returns empty vector (Python behavior)
fn apply_end_on(mut messages: Vec<Message>, end_on: &[MessageTypeFilter]) -> Vec<Message> {
    // Walk backwards from end, removing messages until we find one matching end_on
    while !messages.is_empty() && !is_message_type(&messages[messages.len() - 1], end_on) {
        messages.pop();
    }
    // If we removed all messages without finding a match, messages is now empty
    messages
}

/// Trim messages to fit within a token budget
///
/// This function selectively removes messages to fit within a specified token limit.
/// Useful for managing context windows in LLM conversations.
///
/// # Arguments
///
/// * `messages` - Sequence of messages to trim
/// * `max_tokens` - Maximum number of tokens to keep
/// * `token_counter` - Function to count tokens in a slice of messages
/// * `strategy` - Which messages to keep (First or Last)
/// * `allow_partial` - Whether to split a message if only part can be included
/// * `text_splitter` - Optional function to split text (defaults to newline splitting)
/// * `include_system` - Whether to preserve system message (only for Last strategy)
/// * `start_on` - Message types to start on (only for Last strategy)
/// * `end_on` - Optional message types to end on
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{Message, trim_messages, TrimStrategy};
///
/// let messages = vec![
///     Message::system("You are a helpful assistant"),
///     Message::human("Hello!"),
///     Message::ai("Hi there!"),
///     Message::human("How are you?"),
/// ];
///
/// // Simple token counter for testing (counts 10 tokens per message)
/// let token_counter = |msgs: &[Message]| msgs.len() * 10;
///
/// // Keep last 25 tokens (last 2-3 messages), preserving system message
/// let trimmed = trim_messages(
///     messages,
///     25,
///     token_counter,
///     TrimStrategy::Last,
///     false,  // allow_partial
///     None,  // text_splitter
///     true,  // include_system
///     None,  // start_on
///     None,  // end_on
/// ).unwrap();
/// ```
#[allow(clippy::too_many_arguments)] // Trim config: max_tokens, strategy, partial, splitter, system, start/end filters
pub fn trim_messages<F>(
    messages: Vec<Message>,
    max_tokens: usize,
    token_counter: F,
    strategy: TrimStrategy,
    allow_partial: bool,
    text_splitter: Option<fn(&str) -> Vec<String>>,
    include_system: bool,
    start_on: Option<&[MessageTypeFilter]>,
    end_on: Option<&[MessageTypeFilter]>,
) -> Result<Vec<Message>, TrimError>
where
    F: Fn(&[Message]) -> usize,
{
    let splitter = text_splitter.unwrap_or(default_text_splitter);

    let partial_strategy = if allow_partial {
        match strategy {
            TrimStrategy::First => Some(PartialStrategy::First),
            TrimStrategy::Last => Some(PartialStrategy::Last),
        }
    } else {
        None
    };

    match strategy {
        TrimStrategy::First => {
            if include_system {
                return Err(TrimError::InvalidParameters(
                    "include_system should only be specified for strategy='Last'".to_string(),
                ));
            }
            if start_on.is_some() {
                return Err(TrimError::InvalidParameters(
                    "start_on should only be specified for strategy='Last'".to_string(),
                ));
            }
            _first_max_tokens(
                messages,
                max_tokens,
                token_counter,
                splitter,
                partial_strategy,
                end_on,
            )
        }
        TrimStrategy::Last => {
            // Handle end_on filtering first (remove messages after last occurrence)
            let mut msgs = messages;
            if let Some(end_types) = end_on {
                while !msgs.is_empty() && !is_message_type(&msgs[msgs.len() - 1], end_types) {
                    msgs.pop();
                }
            }

            if msgs.is_empty() {
                return Ok(vec![]);
            }

            // Handle system message preservation (extract before any filtering)
            let system_message = if include_system && !msgs.is_empty() && msgs[0].is_system() {
                Some(msgs.remove(0))
            } else {
                None
            };

            // Calculate remaining tokens after system message
            let remaining_tokens = if let Some(ref sys_msg) = system_message {
                let system_tokens = token_counter(std::slice::from_ref(sys_msg));
                max_tokens.saturating_sub(system_tokens)
            } else {
                max_tokens
            };

            // Reverse messages to use _first_max_tokens with reversed logic
            // Python passes start_on as end_on to reversed _first_max_tokens
            let mut reversed: Vec<Message> = msgs.into_iter().rev().collect();
            reversed = _first_max_tokens(
                reversed,
                remaining_tokens,
                token_counter,
                splitter,
                partial_strategy,
                start_on,
            )?;
            reversed.reverse();

            // Add back system message if needed
            if let Some(sys_msg) = system_message {
                reversed.insert(0, sys_msg);
            }

            Ok(reversed)
        }
    }
}

#[cfg(test)]
mod tests;

/// Convert a sequence of messages to strings and concatenate them into one string.
///
/// This utility formats messages with role prefixes and concatenates them with newlines.
///
/// # Arguments
///
/// * `messages` - Messages to be converted to strings
/// * `human_prefix` - The prefix to prepend to contents of `HumanMessages` (default: "Human")
/// * `ai_prefix` - The prefix to prepend to contents of `AIMessages` (default: "AI")
///
/// # Returns
///
/// A single string concatenation of all input messages.
///
/// # Errors
///
/// Returns an error if an unsupported message type is encountered.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{get_buffer_string, Message};
///
/// let messages = vec![
///     Message::human("Hi, how are you?"),
///     Message::ai("Good, how are you?"),
/// ];
/// let result = get_buffer_string(&messages, "Human", "AI").unwrap();
/// assert_eq!(result, "Human: Hi, how are you?\nAI: Good, how are you?");
/// ```
pub fn get_buffer_string(
    messages: &[Message],
    human_prefix: &str,
    ai_prefix: &str,
) -> Result<String, String> {
    let mut string_messages = Vec::new();

    for m in messages {
        let role = match m {
            Message::Human { .. } => human_prefix,
            Message::AI { .. } => ai_prefix,
            Message::System { .. } => "System",
            Message::Function { .. } => "Function",
            Message::Tool { .. } => "Tool",
        };

        let mut message_text = format!("{}: {}", role, m.as_text());

        // Add function_call if present in AI message
        if let Message::AI { fields, .. } = m {
            if let Some(function_call) = fields.additional_kwargs.get("function_call") {
                message_text.push_str(&format!("{function_call}"));
            }
        }

        string_messages.push(message_text);
    }

    Ok(string_messages.join("\n"))
}

/// Convert a message to a message chunk.
///
/// This is used internally for merging consecutive messages of the same type.
fn msg_to_chunk(message: &Message) -> AIMessageChunk {
    match message {
        Message::AI {
            content,
            tool_calls,
            invalid_tool_calls,
            usage_metadata,
            fields,
        } => AIMessageChunk {
            content: content.as_text(),
            tool_calls: tool_calls.clone(),
            invalid_tool_calls: invalid_tool_calls.clone(),
            usage_metadata: usage_metadata.clone(),
            fields: fields.clone(),
        },
        Message::Human { content, fields } => AIMessageChunk {
            content: content.as_text(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: fields.clone(),
        },
        Message::System { content, fields } => AIMessageChunk {
            content: content.as_text(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: fields.clone(),
        },
        Message::Tool {
            content, fields, ..
        } => AIMessageChunk {
            content: content.as_text(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: fields.clone(),
        },
        Message::Function {
            content, fields, ..
        } => AIMessageChunk {
            content: content.as_text(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: fields.clone(),
        },
    }
}

/// Convert a message chunk back to a full message.
///
/// This is used internally for merging consecutive messages of the same type.
fn chunk_to_msg(chunk: AIMessageChunk, original_type: &Message) -> Message {
    match original_type {
        Message::AI { .. } => Message::AI {
            content: MessageContent::Text(chunk.content),
            tool_calls: chunk.tool_calls,
            invalid_tool_calls: chunk.invalid_tool_calls,
            usage_metadata: chunk.usage_metadata,
            fields: chunk.fields,
        },
        Message::Human { .. } => Message::Human {
            content: MessageContent::Text(chunk.content),
            fields: chunk.fields,
        },
        Message::System { .. } => Message::System {
            content: MessageContent::Text(chunk.content),
            fields: chunk.fields,
        },
        Message::Tool { tool_call_id, .. } => Message::Tool {
            content: MessageContent::Text(chunk.content),
            tool_call_id: tool_call_id.clone(),
            artifact: None,
            status: None,
            fields: chunk.fields,
        },
        Message::Function { name, .. } => Message::Function {
            content: MessageContent::Text(chunk.content),
            name: name.clone(),
            fields: chunk.fields,
        },
    }
}

/// Merge consecutive messages of the same type.
///
/// This function takes a sequence of messages and merges consecutive messages of the same type
/// into single messages. Tool messages are NOT merged, as each has a distinct tool call ID.
///
/// # Arguments
///
/// * `messages` - Sequence of messages to merge
/// * `chunk_separator` - String to insert between message chunks (default: "\n")
///
/// # Returns
///
/// A list of messages with consecutive runs of the same type merged.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{merge_message_runs, Message};
///
/// let messages = vec![
///     Message::system("You're a good assistant."),
///     Message::human("What's your favorite color?"),
///     Message::human("Wait, your favorite food?"),
///     Message::ai("My favorite color is blue"),
/// ];
///
/// let merged = merge_message_runs(messages, "\n");
/// assert_eq!(merged.len(), 3); // System, Human (merged), AI
/// ```
#[must_use]
pub fn merge_message_runs(messages: Vec<Message>, chunk_separator: &str) -> Vec<Message> {
    if messages.is_empty() {
        return vec![];
    }

    let mut merged: Vec<Message> = Vec::new();

    for msg in messages {
        let last = merged.pop();

        match last {
            None => {
                // First message
                merged.push(msg);
            }
            Some(last_msg) => {
                // Check if we should merge: same type AND not a ToolMessage
                let should_merge = match (&last_msg, &msg) {
                    (Message::Tool { .. }, _) | (_, Message::Tool { .. }) => false,
                    (Message::AI { .. }, Message::AI { .. }) => true,
                    (Message::Human { .. }, Message::Human { .. }) => true,
                    (Message::System { .. }, Message::System { .. }) => true,
                    (Message::Function { .. }, Message::Function { .. }) => true,
                    _ => false,
                };

                if should_merge {
                    // Merge the messages
                    let mut last_chunk = msg_to_chunk(&last_msg);
                    let mut curr_chunk = msg_to_chunk(&msg);

                    // Clear response_metadata from current chunk (Python behavior)
                    curr_chunk.fields.response_metadata.clear();

                    // Add separator if both contents are non-empty strings
                    if !last_chunk.content.is_empty() && !curr_chunk.content.is_empty() {
                        last_chunk.content.push_str(chunk_separator);
                    }

                    // Merge chunks
                    let merged_chunk = last_chunk.merge(curr_chunk);

                    // Convert back to message, preserving ID from first message
                    let merged_msg = chunk_to_msg(merged_chunk, &last_msg);

                    merged.push(merged_msg);
                } else {
                    // Different types, don't merge
                    merged.push(last_msg);
                    merged.push(msg);
                }
            }
        }
    }

    merged
}

/// A type representing the various ways a message can be represented.
///
/// This matches Python's `MessageLikeRepresentation` type.
#[derive(Debug, Clone)]
pub enum MessageLike {
    /// Already a message (boxed to reduce enum size)
    Message(Box<Message>),
    /// String content (converted to `HumanMessage`)
    String(String),
    /// Tuple of (role, content)
    Tuple(String, String),
    /// Dictionary representation
    Dict(serde_json::Value),
}

impl From<Message> for MessageLike {
    fn from(msg: Message) -> Self {
        MessageLike::Message(Box::new(msg))
    }
}

impl From<String> for MessageLike {
    fn from(s: String) -> Self {
        MessageLike::String(s)
    }
}

impl From<&str> for MessageLike {
    fn from(s: &str) -> Self {
        MessageLike::String(s.to_string())
    }
}

impl From<(String, String)> for MessageLike {
    fn from((role, content): (String, String)) -> Self {
        MessageLike::Tuple(role, content)
    }
}

impl From<(&str, &str)> for MessageLike {
    fn from((role, content): (&str, &str)) -> Self {
        MessageLike::Tuple(role.to_string(), content.to_string())
    }
}

impl From<serde_json::Value> for MessageLike {
    fn from(v: serde_json::Value) -> Self {
        MessageLike::Dict(v)
    }
}

/// Create a message from a message type string and content.
///
/// This matches Python's `_create_message_from_message_type()` function.
fn create_message_from_type(
    message_type: &str,
    content: String,
    name: Option<String>,
    tool_call_id: Option<String>,
    id: Option<String>,
    additional_kwargs: std::collections::HashMap<String, serde_json::Value>,
) -> Result<Message, String> {
    let fields = BaseMessageFields {
        id,
        name,
        additional_kwargs,
        response_metadata: std::collections::HashMap::new(),
    };

    match message_type {
        "human" | "user" => Ok(Message::Human {
            content: MessageContent::Text(content),
            fields,
        }),
        "ai" | "assistant" => Ok(Message::AI {
            content: MessageContent::Text(content),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields,
        }),
        "system" | "developer" => Ok(Message::System {
            content: MessageContent::Text(content),
            fields,
        }),
        "tool" => {
            if let Some(tc_id) = tool_call_id {
                Ok(Message::Tool {
                    content: MessageContent::Text(content),
                    tool_call_id: tc_id,
                    artifact: None,
                    status: None,
                    fields,
                })
            } else {
                Err("tool_call_id is required for tool messages".to_string())
            }
        }
        "function" => {
            let func_name = fields.name.clone().unwrap_or_default();
            Ok(Message::Function {
                content: MessageContent::Text(content),
                name: func_name,
                fields,
            })
        }
        _ => Err(format!("Unknown message type: {message_type}")),
    }
}

/// Convert a message-like representation to a message.
///
/// This matches Python's `_convert_to_message()` function.
fn convert_to_message(message_like: MessageLike) -> Result<Message, String> {
    match message_like {
        MessageLike::Message(msg) => Ok(*msg),
        MessageLike::String(s) => Ok(Message::human(s)),
        MessageLike::Tuple(role, content) => create_message_from_type(
            &role,
            content,
            None,
            None,
            None,
            std::collections::HashMap::new(),
        ),
        MessageLike::Dict(dict) => {
            // Extract role/type and content from dict
            let obj = dict.as_object().ok_or("Dict must be a JSON object")?;

            let msg_type = obj
                .get("role")
                .or_else(|| obj.get("type"))
                .and_then(|v| v.as_str())
                .ok_or("Dict must contain 'role' or 'type' key")?;

            let content = obj
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let tool_call_id = obj
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let id = obj
                .get("id")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Extract additional_kwargs
            let mut additional_kwargs = std::collections::HashMap::new();
            for (k, v) in obj {
                if !["role", "type", "content", "name", "tool_call_id", "id"].contains(&k.as_str())
                {
                    additional_kwargs.insert(k.clone(), v.clone());
                }
            }

            create_message_from_type(msg_type, content, name, tool_call_id, id, additional_kwargs)
        }
    }
}

/// Convert a sequence of message-like representations to a list of messages.
///
/// This utility accepts various message representations and normalizes them into Message objects:
/// - Already Message objects (passed through)
/// - Strings (converted to `HumanMessage`)
/// - (role, content) tuples
/// - Dictionaries with 'role'/'type' and 'content' keys
///
/// # Arguments
///
/// * `messages` - Sequence of message-like representations
///
/// # Returns
///
/// A list of Messages.
///
/// # Errors
///
/// Returns an error if a message representation is invalid.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{convert_to_messages, MessageLike, Message};
///
/// let messages = vec![
///     MessageLike::from("Hello"), // String -> HumanMessage
///     MessageLike::from(("ai", "Hi there!")), // Tuple -> AIMessage
///     MessageLike::from(Message::system("You are helpful")), // Already a message
/// ];
///
/// let converted = convert_to_messages(messages).unwrap();
/// assert_eq!(converted.len(), 3);
/// ```
pub fn convert_to_messages(messages: Vec<MessageLike>) -> Result<Vec<Message>, String> {
    messages.into_iter().map(convert_to_message).collect()
}

/// Convert a message to a dictionary with Python `DashFlow` format.
///
/// The dictionary has the format: `{"type": "...", "data": {...}}`
/// where `type` is the message type and `data` contains all message fields.
///
/// This matches Python's `message_to_dict()` function for compatibility.
///
/// # Arguments
///
/// * `message` - Message to serialize
///
/// # Returns
///
/// A JSON value with format `{"type": "...", "data": {...}}`
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{Message, message_to_dict};
///
/// let msg = Message::human("Hello");
/// let dict = message_to_dict(&msg).unwrap();
///
/// // dict has format: {"type": "human", "data": {"content": "Hello", ...}}
/// ```
pub fn message_to_dict(message: &Message) -> Result<serde_json::Value, String> {
    let message_type = message.message_type();
    let data =
        serde_json::to_value(message).map_err(|e| format!("Failed to serialize message: {e}"))?;

    Ok(serde_json::json!({
        "type": message_type,
        "data": data
    }))
}

/// Convert a sequence of messages to a list of dictionaries.
///
/// This is the batch version of `message_to_dict()`.
///
/// # Arguments
///
/// * `messages` - Messages to serialize
///
/// # Returns
///
/// A list of JSON values in the format `[{"type": "...", "data": {...}}, ...]`
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{Message, messages_to_dict};
///
/// let messages = vec![
///     Message::human("Hello"),
///     Message::ai("Hi there"),
/// ];
/// let dicts = messages_to_dict(&messages).unwrap();
/// assert_eq!(dicts.len(), 2);
/// ```
pub fn messages_to_dict(messages: &[Message]) -> Result<Vec<serde_json::Value>, String> {
    messages.iter().map(message_to_dict).collect()
}

/// Convert a dictionary to a Message object.
///
/// Expects the Python `DashFlow` format: `{"type": "...", "data": {...}}`
///
/// This matches Python's `_message_from_dict()` function.
///
/// # Arguments
///
/// * `dict` - Dictionary with `type` and `data` keys
///
/// # Returns
///
/// A Message object
///
/// # Errors
///
/// Returns an error if the dictionary format is invalid or the message type is unknown.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{message_from_dict, Message};
/// use serde_json::json;
///
/// let dict = json!({
///     "type": "human",
///     "data": {
///         "content": "Hello",
///         "additional_kwargs": {},
///         "response_metadata": {},
///         "type": "human",
///         "name": null,
///         "id": null
///     }
/// });
///
/// let msg = message_from_dict(&dict).unwrap();
/// assert_eq!(msg.as_text(), "Hello");
/// ```
pub fn message_from_dict(dict: &serde_json::Value) -> Result<Message, String> {
    let obj = dict.as_object().ok_or("Expected a JSON object")?;

    let msg_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("Dictionary must contain 'type' key")?;

    let data = obj
        .get("data")
        .ok_or("Dictionary must contain 'data' key")?;

    // Deserialize based on message type
    // Note: Python supports chunk types here too, but we'll implement that separately
    match msg_type {
        "human" | "Human" => serde_json::from_value(data.clone())
            .map_err(|e| format!("Failed to deserialize human message: {e}")),
        "ai" | "AI" => serde_json::from_value(data.clone())
            .map_err(|e| format!("Failed to deserialize AI message: {e}")),
        "system" | "System" => serde_json::from_value(data.clone())
            .map_err(|e| format!("Failed to deserialize system message: {e}")),
        "tool" | "Tool" => serde_json::from_value(data.clone())
            .map_err(|e| format!("Failed to deserialize tool message: {e}")),
        "function" | "Function" => serde_json::from_value(data.clone())
            .map_err(|e| format!("Failed to deserialize function message: {e}")),
        _ => Err(format!("Unknown message type: {msg_type}")),
    }
}

/// Convert a sequence of dictionaries to messages.
///
/// This is the batch version of `message_from_dict()`.
///
/// # Arguments
///
/// * `dicts` - List of dictionaries in format `{"type": "...", "data": {...}}`
///
/// # Returns
///
/// A list of Messages
///
/// # Errors
///
/// Returns an error if any dictionary is invalid.
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{messages_from_dict, messages_to_dict, Message};
///
/// let original = vec![
///     Message::human("Hello"),
///     Message::ai("Hi there"),
/// ];
///
/// // Serialize and deserialize
/// let dicts = messages_to_dict(&original).unwrap();
/// let restored = messages_from_dict(&dicts).unwrap();
///
/// assert_eq!(restored.len(), 2);
/// assert_eq!(restored[0].as_text(), "Hello");
/// assert_eq!(restored[1].as_text(), "Hi there");
/// ```
pub fn messages_from_dict(dicts: &[serde_json::Value]) -> Result<Vec<Message>, String> {
    dicts.iter().map(message_from_dict).collect()
}

/// Convert an `AIMessageChunk` to a full AI Message.
///
/// This matches Python's `message_chunk_to_message()` function.
///
/// `AIMessageChunk` is used for streaming responses. This function converts
/// a completed chunk into a full Message suitable for storage or further processing.
///
/// # Arguments
///
/// * `chunk` - `AIMessageChunk` to convert
///
/// # Returns
///
/// A full AI Message
///
/// # Example
///
/// ```
/// use dashflow::core::messages::{AIMessageChunk, message_chunk_to_message};
///
/// let chunk = AIMessageChunk::new("Hello world");
/// let msg = message_chunk_to_message(chunk);
/// assert_eq!(msg.as_text(), "Hello world");
/// ```
#[must_use]
pub fn message_chunk_to_message(chunk: AIMessageChunk) -> Message {
    Message::AI {
        content: MessageContent::Text(chunk.content),
        tool_calls: chunk.tool_calls,
        invalid_tool_calls: chunk.invalid_tool_calls,
        usage_metadata: chunk.usage_metadata,
        fields: chunk.fields,
    }
}
