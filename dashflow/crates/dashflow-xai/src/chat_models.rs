// OpenAI-compatible client clippy exceptions for async boundaries:
// - clone_on_ref_ptr: Arc::clone() is idiomatic for sharing client across async tasks
// - needless_pass_by_value: async move closures require owned values
// - redundant_clone: Clone before async move prevents use-after-move
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! xAI AI chat model implementation

use crate::XAI_DEFAULT_API_BASE;

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPartImage,
        ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageContentPart,
        ChatCompletionTool, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs,
        FunctionCall, FunctionObject, ImageDetail as OpenAIImageDetail, ImageUrl, ResponseFormat,
        ResponseFormatJsonSchema,
    },
    Client,
};
use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::config_loader::env_vars::{
    env_string, env_string_or_default, XAI_API_BASE as XAI_API_BASE_ENV, XAI_API_KEY,
};
use dashflow::core::{
    callbacks::CallbackManager,
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, InvalidToolCall, Message, ToolCall},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use futures::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// xAI AI chat model configuration and client
///
/// xAI AI provides powerful LLM inference with an OpenAI-compatible API.
///
/// # Example
/// ```no_run
/// use dashflow_xai::ChatXAI;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatXAI::new()
///         .with_model("grok-beta")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Available Models
/// - `grok-beta` (default) - General-purpose chat model
/// - `grok-vision-beta` - Specialized for code generation
/// - `grok-beta` - Advanced reasoning capabilities
#[derive(Clone, Debug)]
pub struct ChatXAI {
    /// xAI client (using OpenAI-compatible interface)
    client: Arc<Client<OpenAIConfig>>,

    /// Model name (e.g., "grok-beta", "grok-vision-beta", "grok-beta")
    model: String,

    /// Sampling temperature (0.0 to 2.0)
    temperature: Option<f32>,

    /// Maximum tokens to generate
    max_tokens: Option<u32>,

    /// Top-p sampling parameter
    top_p: Option<f32>,

    /// Frequency penalty (-2.0 to 2.0)
    frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 to 2.0)
    presence_penalty: Option<f32>,

    /// Tools available for the model to call
    tools: Option<Vec<ChatCompletionTool>>,

    /// Controls which (if any) tool is called by the model
    tool_choice: Option<ChatCompletionToolChoiceOption>,

    /// Response format (text, `json_object`, or `json_schema`)
    response_format: Option<ResponseFormat>,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl ChatXAI {
    /// Create a new `ChatXAI` instance with default settings
    ///
    /// Uses `XAI_API_KEY` environment variable for authentication
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_xai::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        let api_base = env_string_or_default(XAI_API_BASE_ENV, XAI_DEFAULT_API_BASE);

        let mut config = OpenAIConfig::new().with_api_base(&api_base);
        if let Some(api_key) = env_string(XAI_API_KEY) {
            config = config.with_api_key(&api_key);
        }

        Self::with_config(config)
    }

    /// Create a new `ChatXAI` instance with custom configuration
    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            model: "grok-beta".to_string(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the model name
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the top-p parameter
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set the frequency penalty
    #[must_use]
    pub fn with_frequency_penalty(mut self, penalty: f32) -> Self {
        self.frequency_penalty = Some(penalty);
        self
    }

    /// Set the presence penalty
    #[must_use]
    pub fn with_presence_penalty(mut self, penalty: f32) -> Self {
        self.presence_penalty = Some(penalty);
        self
    }

    /// Set the retry policy
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter to control request rate
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_xai::ChatXAI;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::time::Duration;
    /// use std::sync::Arc;
    ///
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let model = ChatXAI::new()
    ///     .with_model("grok-beta")
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Bind tools to the model for function calling
    ///
    /// # Arguments
    /// * `tools` - Vector of tool schemas in JSON format. Each tool should have:
    ///   - `name`: Function name (required)
    ///   - `description`: What the function does (optional but recommended)
    ///   - `parameters`: JSON Schema describing function parameters (optional)
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_xai::ChatXAI;
    /// use serde_json::json;
    ///
    /// let tool = json!({
    ///     "name": "get_weather",
    ///     "description": "Get current weather for a location",
    ///     "parameters": {
    ///         "type": "object",
    ///         "properties": {
    ///             "location": {
    ///                 "type": "string",
    ///                 "description": "City name"
    ///             }
    ///         },
    ///         "required": ["location"]
    ///     }
    /// });
    ///
    /// let model = ChatXAI::new().with_tools(vec![tool]);
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
        let openai_tools: Vec<ChatCompletionTool> = tools
            .into_iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?.to_string();
                let description = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(std::string::ToString::to_string);
                let parameters = tool.get("parameters").cloned();

                Some(ChatCompletionTool {
                    r#type: async_openai::types::ChatCompletionToolType::Function,
                    function: FunctionObject {
                        name,
                        description,
                        parameters,
                        strict: None,
                    },
                })
            })
            .collect();

        self.tools = if openai_tools.is_empty() {
            None
        } else {
            Some(openai_tools)
        };
        self
    }

    /// Control which tool the model should call
    ///
    /// # Arguments
    /// * `choice` - Tool choice option:
    ///   - `None` or `"none"`: Model will not call any tools
    ///   - `"auto"`: Model decides whether to call a tool (default)
    ///   - `"required"`: Model must call at least one tool
    ///   - Function name string: Force model to call specific function
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_xai::ChatXAI;
    ///
    /// // Force model to use tools
    /// let model = ChatXAI::new()
    ///     .with_tool_choice(Some("required".to_string()));
    ///
    /// // Force specific tool
    /// let model = ChatXAI::new()
    ///     .with_tool_choice(Some("get_weather".to_string()));
    /// ```
    #[must_use]
    pub fn with_tool_choice(mut self, choice: Option<String>) -> Self {
        self.tool_choice = choice.map(|c| match c.as_str() {
            "none" => ChatCompletionToolChoiceOption::None,
            "auto" => ChatCompletionToolChoiceOption::Auto,
            "required" => ChatCompletionToolChoiceOption::Required,
            function_name => ChatCompletionToolChoiceOption::Named(
                async_openai::types::ChatCompletionNamedToolChoice {
                    r#type: async_openai::types::ChatCompletionToolType::Function,
                    function: async_openai::types::FunctionName {
                        name: function_name.to_string(),
                    },
                },
            ),
        });
        self
    }

    /// Enable JSON mode - forces the model to output valid JSON
    ///
    /// When JSON mode is enabled, the model will always produce valid JSON output.
    /// You must include instructions for JSON output in your system or user message.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_xai::ChatXAI;
    /// use dashflow::core::messages::Message;
    /// use dashflow::core::language_models::ChatModel;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ChatXAI::new()
    ///     .with_model("grok-beta")
    ///     .with_json_mode();
    ///
    /// let messages = vec![
    ///     Message::system("You are a helpful assistant that outputs JSON."),
    ///     Message::human("List 3 colors as JSON array"),
    /// ];
    ///
    /// let result = model.generate(&messages, None, None, None, None).await?;
    /// // Response will be valid JSON, e.g., ["red", "blue", "green"]
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_json_mode(mut self) -> Self {
        self.response_format = Some(ResponseFormat::JsonObject);
        self
    }

    /// Enable structured output with a JSON schema
    ///
    /// Forces the model to output JSON that conforms to the provided schema.
    /// `OpenAI`'s structured outputs feature guarantees the response matches the schema.
    ///
    /// # Arguments
    /// * `name` - Name for the response format (alphanumeric, underscores, dashes only)
    /// * `schema` - JSON Schema describing the expected output structure
    /// * `description` - Optional description of what the response format is for
    /// * `strict` - Whether to enable strict schema adherence (recommended: true)
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_xai::ChatXAI;
    /// use dashflow::core::messages::Message;
    /// use dashflow::core::language_models::ChatModel;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let schema = json!({
    ///     "type": "object",
    ///     "properties": {
    ///         "name": {"type": "string"},
    ///         "age": {"type": "number"},
    ///         "email": {"type": "string", "format": "email"}
    ///     },
    ///     "required": ["name", "age"],
    ///     "additionalProperties": false
    /// });
    ///
    /// let model = ChatXAI::new()
    ///     .with_model("grok-beta")
    ///     .with_structured_output(
    ///         "user_info",
    ///         schema,
    ///         Some("User information extraction format".to_string()),
    ///         true
    ///     );
    ///
    /// let messages = vec![
    ///     Message::human("Extract user info: John Doe, 30 years old, john@example.com"),
    /// ];
    ///
    /// let result = model.generate(&messages, None, None, None, None).await?;
    /// // Response will match the schema exactly
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_structured_output(
        mut self,
        name: impl Into<String>,
        schema: serde_json::Value,
        description: Option<String>,
        strict: bool,
    ) -> Self {
        self.response_format = Some(ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: name.into(),
                description,
                schema: Some(schema),
                strict: Some(strict),
            },
        });
        self
    }

    /// Create a `ChatXAI` instance from a configuration
    ///
    /// This method constructs a `ChatXAI` model from a `ChatModelConfig::XAI` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be XAI variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatXAI` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not a XAI variant
    /// - API key environment variable cannot be resolved
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::XAI {
                model,
                api_key,
                temperature,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                let api_base = env_string_or_default(XAI_API_BASE_ENV, XAI_DEFAULT_API_BASE);

                // Build the XAI config with custom API key
                let openai_config = OpenAIConfig::new()
                    .with_api_key(&resolved_api_key)
                    .with_api_base(&api_base);

                // Create the ChatXAI instance
                let mut chat_model = Self::with_config(openai_config).with_model(model);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(*temp);
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected XAI config, got {} config",
                config.provider()
            ))),
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatXAI {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert `DashFlow` `ImageSource` to `OpenAI` `ImageUrl`
fn convert_image_source(
    source: &dashflow::core::messages::ImageSource,
    detail: Option<dashflow::core::messages::ImageDetail>,
) -> ImageUrl {
    use dashflow::core::messages::ImageSource;

    let url = match source {
        ImageSource::Url { url } => url.clone(),
        ImageSource::Base64 { media_type, data } => {
            format!("data:{media_type};base64,{data}")
        }
    };

    let openai_detail = detail.map(|d| match d {
        dashflow::core::messages::ImageDetail::Low => OpenAIImageDetail::Low,
        dashflow::core::messages::ImageDetail::High => OpenAIImageDetail::High,
        dashflow::core::messages::ImageDetail::Auto => OpenAIImageDetail::Auto,
    });

    ImageUrl {
        url,
        detail: openai_detail,
    }
}

/// Convert `DashFlow` `MessageContent` to `OpenAI` user message content format
fn convert_content(
    content: &dashflow::core::messages::MessageContent,
) -> ChatCompletionRequestUserMessageContent {
    use dashflow::core::messages::{ContentBlock, MessageContent};

    match content {
        MessageContent::Text(text) => ChatCompletionRequestUserMessageContent::Text(text.clone()),
        MessageContent::Blocks(blocks) => {
            let parts: Vec<ChatCompletionRequestUserMessageContentPart> = blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => {
                        if text.is_empty() {
                            None
                        } else {
                            Some(ChatCompletionRequestUserMessageContentPart::Text(
                                ChatCompletionRequestMessageContentPartText { text: text.clone() },
                            ))
                        }
                    }
                    ContentBlock::Image { source, detail } => {
                        Some(ChatCompletionRequestUserMessageContentPart::ImageUrl(
                            ChatCompletionRequestMessageContentPartImage {
                                image_url: convert_image_source(source, *detail),
                            },
                        ))
                    }
                    // Other content block types (ToolUse, ToolResult, Reasoning) are not user message content
                    _ => None,
                })
                .collect();

            // If we only have one text part, use text format instead
            if parts.len() == 1 {
                if let ChatCompletionRequestUserMessageContentPart::Text(text_part) = &parts[0] {
                    return ChatCompletionRequestUserMessageContent::Text(text_part.text.clone());
                }
            }

            ChatCompletionRequestUserMessageContent::Array(parts)
        }
    }
}

/// Convert `DashFlow` Message to `OpenAI` `ChatCompletionRequestMessage`
fn convert_message(message: &Message) -> Result<ChatCompletionRequestMessage> {
    match message {
        Message::System { content, .. } => {
            let msg = ChatCompletionRequestSystemMessageArgs::default()
                .content(content.as_text())
                .build()
                .map_err(|e| {
                    Error::invalid_input(format!("Failed to build system message: {e}"))
                })?;
            Ok(ChatCompletionRequestMessage::System(msg))
        }
        Message::Human { content, .. } => {
            let msg = ChatCompletionRequestUserMessageArgs::default()
                .content(convert_content(content))
                .build()
                .map_err(|e| Error::invalid_input(format!("Failed to build user message: {e}")))?;
            Ok(ChatCompletionRequestMessage::User(msg))
        }
        Message::AI {
            content,
            tool_calls,
            ..
        } => {
            let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
            builder.content(content.as_text());

            // Convert tool calls to OpenAI format
            if !tool_calls.is_empty() {
                let openai_tool_calls: Vec<ChatCompletionMessageToolCall> = tool_calls
                    .iter()
                    .map(|tc| ChatCompletionMessageToolCall {
                        id: tc.id.clone(),
                        r#type: async_openai::types::ChatCompletionToolType::Function,
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.args.to_string(),
                        },
                    })
                    .collect();
                builder.tool_calls(openai_tool_calls);
            }

            let msg = builder.build().map_err(|e| {
                Error::invalid_input(format!("Failed to build assistant message: {e}"))
            })?;
            Ok(ChatCompletionRequestMessage::Assistant(msg))
        }
        Message::Tool {
            content,
            tool_call_id,
            ..
        } => {
            let msg = ChatCompletionRequestToolMessageArgs::default()
                .content(content.as_text())
                .tool_call_id(tool_call_id.clone())
                .build()
                .map_err(|e| Error::invalid_input(format!("Failed to build tool message: {e}")))?;
            Ok(ChatCompletionRequestMessage::Tool(msg))
        }
        Message::Function { content, name, .. } => {
            // OpenAI deprecated function messages in favor of tool messages
            // Convert to tool message with a generated tool_call_id
            let tool_call_id = format!("func_{name}");
            let msg = ChatCompletionRequestToolMessageArgs::default()
                .content(content.as_text())
                .tool_call_id(tool_call_id)
                .build()
                .map_err(|e| {
                    Error::invalid_input(format!("Failed to build tool message from function: {e}"))
                })?;
            Ok(ChatCompletionRequestMessage::Tool(msg))
        }
    }
}

/// Convert `DashFlow` `ToolDefinition` to `OpenAI` `ChatCompletionTool` (xAI uses `OpenAI` format)
fn convert_tool_definition(tool: &ToolDefinition) -> ChatCompletionTool {
    ChatCompletionTool {
        r#type: async_openai::types::ChatCompletionToolType::Function,
        function: FunctionObject {
            name: tool.name.clone(),
            description: if tool.description.is_empty() {
                None
            } else {
                Some(tool.description.clone())
            },
            parameters: Some(tool.parameters.clone()),
            strict: None,
        },
    }
}

/// Convert `DashFlow` `ToolChoice` to `OpenAI` `ChatCompletionToolChoiceOption` (xAI uses `OpenAI` format)
fn convert_tool_choice(choice: &ToolChoice) -> ChatCompletionToolChoiceOption {
    match choice {
        ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
        ToolChoice::None => ChatCompletionToolChoiceOption::None,
        ToolChoice::Required => ChatCompletionToolChoiceOption::Required,
        ToolChoice::Specific(name) => ChatCompletionToolChoiceOption::Named(
            async_openai::types::ChatCompletionNamedToolChoice {
                r#type: async_openai::types::ChatCompletionToolType::Function,
                function: async_openai::types::FunctionName { name: name.clone() },
            },
        ),
    }
}

#[async_trait]
impl ChatModel for ChatXAI {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
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

        // Convert messages
        let openai_messages: Vec<ChatCompletionRequestMessage> = messages
            .iter()
            .map(convert_message)
            .collect::<Result<Vec<_>>>()?;

        // Build request
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder.model(&self.model).messages(openai_messages);

        if let Some(temp) = self.temperature {
            request_builder.temperature(temp);
        }
        if let Some(max_tok) = self.max_tokens {
            request_builder.max_tokens(max_tok);
        }
        if let Some(top_p) = self.top_p {
            request_builder.top_p(top_p);
        }
        if let Some(freq_penalty) = self.frequency_penalty {
            request_builder.frequency_penalty(freq_penalty);
        }
        if let Some(pres_penalty) = self.presence_penalty {
            request_builder.presence_penalty(pres_penalty);
        }
        // xAI requires n=1 (does not support multiple completions)
        request_builder.n(1);

        // Add stop sequences (parameter overrides struct field)
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Convert and add tools if provided (parameter overrides struct field)
        if let Some(tool_defs) = tools {
            let xai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(xai_tools);

            // Handle tool_choice if provided
            if let Some(tc) = tool_choice {
                let choice = convert_tool_choice(tc);
                request_builder.tool_choice(choice);
            }
        } else {
            // Fall back to struct fields if parameters not provided
            if let Some(ref tools) = self.tools {
                request_builder.tools(tools.clone());
            }
            if let Some(ref tool_choice) = self.tool_choice {
                request_builder.tool_choice(tool_choice.clone());
            }
        }

        if let Some(ref response_format) = self.response_format {
            request_builder.response_format(response_format.clone());
        }

        let request = request_builder
            .build()
            .map_err(|e| Error::api(format!("Failed to build request: {e}")))?;

        // Make API call with retry
        let client = self.client.clone();
        let response_result = with_retry(&self.retry_policy, move || {
            let client = client.clone();
            let request = request.clone();
            async move {
                client
                    .chat()
                    .create(request)
                    .await
                    .map_err(|e| Error::api(format!("xAI API error: {e}")))
            }
        })
        .await;

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

        // Convert response to ChatResult
        let generations: Vec<ChatGeneration> = response
            .choices
            .into_iter()
            .map(|choice| {
                let content = choice.message.content.unwrap_or_default();

                // Parse tool calls from response
                let mut tool_calls = Vec::new();
                let mut invalid_tool_calls = Vec::new();

                for tc in choice.message.tool_calls.unwrap_or_default() {
                    // Try to parse arguments as JSON
                    match serde_json::from_str::<serde_json::Value>(&tc.function.arguments) {
                        Ok(args) => tool_calls.push(ToolCall {
                            id: tc.id,
                            name: tc.function.name,
                            args,
                            tool_type: "tool_call".to_string(),
                            index: None,
                        }),
                        Err(e) => invalid_tool_calls.push(InvalidToolCall {
                            id: tc.id,
                            name: Some(tc.function.name),
                            args: Some(tc.function.arguments),
                            error: format!("Failed to parse tool call arguments: {e}"),
                        }),
                    }
                }

                // Build AIMessage with tool calls
                let message = if !tool_calls.is_empty() || !invalid_tool_calls.is_empty() {
                    // Need to manually construct Message::AI since AIMessage doesn't expose setters for invalid_tool_calls
                    Message::AI {
                        content: content.into(),
                        tool_calls,
                        invalid_tool_calls,
                        usage_metadata: None,
                        fields: Default::default(),
                    }
                } else {
                    AIMessage::new(content).into()
                };

                ChatGeneration {
                    message,
                    generation_info: Default::default(),
                }
            })
            .collect();

        let llm_output = response.usage.map(|usage| {
            let mut map = HashMap::new();
            map.insert(
                "prompt_tokens".to_string(),
                serde_json::json!(usage.prompt_tokens),
            );
            map.insert(
                "completion_tokens".to_string(),
                serde_json::json!(usage.completion_tokens),
            );
            map.insert(
                "total_tokens".to_string(),
                serde_json::json!(usage.total_tokens),
            );
            map
        });

        let chat_result = ChatResult {
            generations,
            llm_output,
        };

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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Convert messages
        let openai_messages: Vec<ChatCompletionRequestMessage> = messages
            .iter()
            .map(convert_message)
            .collect::<Result<Vec<_>>>()?;

        // Build request with streaming enabled
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(&self.model)
            .messages(openai_messages)
            .stream(true);

        if let Some(temp) = self.temperature {
            request_builder.temperature(temp);
        }
        if let Some(max_tok) = self.max_tokens {
            request_builder.max_tokens(max_tok);
        }
        if let Some(top_p) = self.top_p {
            request_builder.top_p(top_p);
        }
        if let Some(freq_penalty) = self.frequency_penalty {
            request_builder.frequency_penalty(freq_penalty);
        }
        if let Some(pres_penalty) = self.presence_penalty {
            request_builder.presence_penalty(pres_penalty);
        }
        // xAI requires n=1 (does not support multiple completions)
        request_builder.n(1);

        // Add stop sequences (parameter overrides struct field)
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Convert and add tools if provided (parameter overrides struct field)
        if let Some(tool_defs) = tools {
            let xai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(xai_tools);

            // Handle tool_choice if provided
            if let Some(tc) = tool_choice {
                let choice = convert_tool_choice(tc);
                request_builder.tool_choice(choice);
            }
        } else {
            // Fall back to struct fields if parameters not provided
            if let Some(ref tools) = self.tools {
                request_builder.tools(tools.clone());
            }
            if let Some(ref tool_choice) = self.tool_choice {
                request_builder.tool_choice(tool_choice.clone());
            }
        }

        if let Some(ref response_format) = self.response_format {
            request_builder.response_format(response_format.clone());
        }

        let request = request_builder
            .build()
            .map_err(|e| Error::api(format!("Failed to build streaming request: {e}")))?;

        // Make streaming API call
        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .map_err(|e| Error::api(format!("xAI streaming API error: {e}")))?;

        // Convert OpenAI stream to DashFlow stream
        let output_stream = stream! {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            if let Some(content) = choice.delta.content {
                                let chunk = AIMessageChunk::new(content);
                                yield Ok(ChatGenerationChunk {
                                    message: chunk,
                                    generation_info: Default::default(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::api(format!("Streaming error: {e}")));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    fn llm_type(&self) -> &'static str {
        "grok-beta"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        params.insert("model".to_string(), serde_json::json!(self.model));
        if let Some(temp) = self.temperature {
            params.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = self.max_tokens {
            params.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }
        params
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.rate_limiter.clone()
    }
}

#[cfg(test)]
#[allow(
    deprecated,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)] // Tests use deprecated with_tools() method
mod tests {
    use super::*;

    #[test]
    fn test_chat_xai_builder() {
        let model = ChatXAI::new()
            .with_model("gpt-4")
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_top_p(0.9);

        assert_eq!(model.model, "gpt-4");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(1000));
        assert_eq!(model.top_p, Some(0.9));
    }

    #[test]
    fn test_message_conversion_system() {
        let msg = Message::system("You are a helpful assistant");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    #[test]
    fn test_message_conversion_human() {
        let msg = Message::human("Hello");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(_) => {}
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_ai() {
        let msg = Message::ai("Hi there!");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Assistant(_) => {}
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_message_conversion_ai_with_tool_calls() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            args: serde_json::json!({"location": "San Francisco"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let msg = Message::AI {
            content: "Let me check the weather.".into(),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                assert!(assistant_msg.tool_calls.is_some());
                let tool_calls = assistant_msg.tool_calls.unwrap();
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].id, "call_123");
                assert_eq!(tool_calls[0].function.name, "get_weather");
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_message_conversion_tool() {
        let msg = Message::Tool {
            content: "The weather is sunny, 72°F".into(),
            tool_call_id: "call_123".to_string(),
            artifact: None,
            status: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                assert_eq!(tool_msg.tool_call_id, "call_123");
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_with_tools_builder() {
        let tool = serde_json::json!({
            "name": "get_weather",
            "description": "Get current weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        });

        let model = ChatXAI::new().with_tools(vec![tool]);

        assert!(model.tools.is_some());
        let tools = model.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "get_weather");
        assert_eq!(
            tools[0].function.description,
            Some("Get current weather for a location".to_string())
        );
        assert!(tools[0].function.parameters.is_some());
    }

    #[test]
    fn test_with_tools_multiple() {
        let tools = vec![
            serde_json::json!({"name": "get_weather", "description": "Get weather"}),
            serde_json::json!({"name": "search_web", "description": "Search the web"}),
        ];

        let model = ChatXAI::new().with_tools(tools);

        assert!(model.tools.is_some());
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 2);
        assert_eq!(bound_tools[0].function.name, "get_weather");
        assert_eq!(bound_tools[1].function.name, "search_web");
    }

    #[test]
    fn test_with_tools_empty() {
        let model = ChatXAI::new().with_tools(vec![]);
        assert!(model.tools.is_none());
    }

    #[test]
    fn test_with_tools_invalid_schema() {
        // Tool without name should be filtered out
        let tools = vec![
            serde_json::json!({"description": "No name"}),
            serde_json::json!({"name": "valid_tool"}),
        ];

        let model = ChatXAI::new().with_tools(tools);

        assert!(model.tools.is_some());
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 1);
        assert_eq!(bound_tools[0].function.name, "valid_tool");
    }

    #[test]
    fn test_with_tool_choice_none() {
        let model = ChatXAI::new().with_tool_choice(Some("none".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::None => {}
            _ => panic!("Expected None tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_auto() {
        let model = ChatXAI::new().with_tool_choice(Some("auto".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::Auto => {}
            _ => panic!("Expected Auto tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_required() {
        let model = ChatXAI::new().with_tool_choice(Some("required".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::Required => {}
            _ => panic!("Expected Required tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_named() {
        let model = ChatXAI::new().with_tool_choice(Some("get_weather".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::Named(choice) => {
                assert_eq!(choice.function.name, "get_weather");
            }
            _ => panic!("Expected Named tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_unset() {
        let model = ChatXAI::new().with_tool_choice(None);
        assert!(model.tool_choice.is_none());
    }

    #[test]
    fn test_tools_and_tool_choice_together() {
        let tool = serde_json::json!({
            "name": "calculate",
            "description": "Perform calculation"
        });

        let model = ChatXAI::new()
            .with_tools(vec![tool])
            .with_tool_choice(Some("required".to_string()));

        assert!(model.tools.is_some());
        assert!(model.tool_choice.is_some());
    }

    #[test]
    fn test_with_json_mode() {
        let model = ChatXAI::new().with_json_mode();

        assert!(model.response_format.is_some());
        match model.response_format.unwrap() {
            ResponseFormat::JsonObject => {}
            _ => panic!("Expected JsonObject response format"),
        }
    }

    #[test]
    fn test_with_structured_output() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            },
            "required": ["name", "age"]
        });

        let model = ChatXAI::new().with_structured_output(
            "user_info",
            schema.clone(),
            Some("User information format".to_string()),
            true,
        );

        assert!(model.response_format.is_some());
        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "user_info");
                assert_eq!(
                    json_schema.description,
                    Some("User information format".to_string())
                );
                assert_eq!(json_schema.strict, Some(true));
                assert_eq!(json_schema.schema, Some(schema));
            }
            _ => panic!("Expected JsonSchema response format"),
        }
    }

    #[test]
    fn test_with_structured_output_no_description() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "value": {"type": "string"}
            }
        });

        let model =
            ChatXAI::new().with_structured_output("simple_output", schema.clone(), None, false);

        assert!(model.response_format.is_some());
        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "simple_output");
                assert_eq!(json_schema.description, None);
                assert_eq!(json_schema.strict, Some(false));
                assert_eq!(json_schema.schema, Some(schema));
            }
            _ => panic!("Expected JsonSchema response format"),
        }
    }

    #[test]
    fn test_json_mode_and_tools_incompatible() {
        // OpenAI API doesn't allow tools and response_format together in most cases
        // But at the builder level, we allow it (API will error if incompatible)
        let tool = serde_json::json!({"name": "test_tool"});

        let model = ChatXAI::new().with_tools(vec![tool]).with_json_mode();

        assert!(model.tools.is_some());
        assert!(model.response_format.is_some());
    }

    #[test]
    fn test_response_format_unset_by_default() {
        let model = ChatXAI::new();
        assert!(model.response_format.is_none());
    }

    #[test]
    fn test_structured_output_with_complex_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "users": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "email": {"type": "string", "format": "email"},
                            "roles": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        },
                        "required": ["name", "email"]
                    }
                },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "total": {"type": "number"},
                        "page": {"type": "number"}
                    }
                }
            },
            "required": ["users"],
            "additionalProperties": false
        });

        let model = ChatXAI::new().with_structured_output(
            "user_list_response",
            schema.clone(),
            Some("Paginated user list with metadata".to_string()),
            true,
        );

        assert!(model.response_format.is_some());
        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "user_list_response");
                assert_eq!(json_schema.strict, Some(true));
                // Verify the schema is preserved correctly
                assert!(json_schema.schema.is_some());
                let stored_schema = json_schema.schema.unwrap();
                assert_eq!(stored_schema.get("type").unwrap(), "object");
                assert!(stored_schema
                    .get("properties")
                    .unwrap()
                    .get("users")
                    .is_some());
            }
            _ => panic!("Expected JsonSchema response format"),
        }
    }

    #[test]
    fn test_message_conversion_human_with_image_url() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        let msg = Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "What's in this image?".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image.jpg".to_string(),
                    },
                    detail: None,
                },
            ]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(user_msg) => {
                match user_msg.content {
                    ChatCompletionRequestUserMessageContent::Array(parts) => {
                        assert_eq!(parts.len(), 2);

                        // First part: text
                        match &parts[0] {
                            ChatCompletionRequestUserMessageContentPart::Text(text_part) => {
                                assert_eq!(text_part.text, "What's in this image?");
                            }
                            _ => panic!("Expected text part"),
                        }

                        // Second part: image
                        match &parts[1] {
                            ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                                assert_eq!(img_part.image_url.url, "https://example.com/image.jpg");
                                assert!(img_part.image_url.detail.is_none());
                            }
                            _ => panic!("Expected image part"),
                        }
                    }
                    _ => panic!("Expected array content"),
                }
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_with_base64_image() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        let msg = Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Analyze this image".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/png".to_string(),
                        data: "iVBORw0KGgoAAAANS...".to_string(),
                    },
                    detail: Some(dashflow::core::messages::ImageDetail::High),
                },
            ]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(user_msg) => {
                match user_msg.content {
                    ChatCompletionRequestUserMessageContent::Array(parts) => {
                        assert_eq!(parts.len(), 2);

                        // Second part: base64 image
                        match &parts[1] {
                            ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                                assert_eq!(
                                    img_part.image_url.url,
                                    "data:image/png;base64,iVBORw0KGgoAAAANS..."
                                );
                                assert_eq!(
                                    img_part.image_url.detail,
                                    Some(OpenAIImageDetail::High)
                                );
                            }
                            _ => panic!("Expected image part"),
                        }
                    }
                    _ => panic!("Expected array content"),
                }
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_image_only() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        let msg = Message::Human {
            content: MessageContent::Blocks(vec![ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/photo.jpg".to_string(),
                },
                detail: Some(dashflow::core::messages::ImageDetail::Auto),
            }]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
                ChatCompletionRequestUserMessageContent::Array(parts) => {
                    assert_eq!(parts.len(), 1);

                    match &parts[0] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                            assert_eq!(img_part.image_url.url, "https://example.com/photo.jpg");
                            assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Auto));
                        }
                        _ => panic!("Expected image part"),
                    }
                }
                _ => panic!("Expected array content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_multiple_images() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        let msg = Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Compare these images".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image1.jpg".to_string(),
                    },
                    detail: Some(dashflow::core::messages::ImageDetail::Low),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image2.jpg".to_string(),
                    },
                    detail: Some(dashflow::core::messages::ImageDetail::Low),
                },
            ]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(user_msg) => {
                match user_msg.content {
                    ChatCompletionRequestUserMessageContent::Array(parts) => {
                        assert_eq!(parts.len(), 3);

                        // First: text
                        match &parts[0] {
                            ChatCompletionRequestUserMessageContentPart::Text(text_part) => {
                                assert_eq!(text_part.text, "Compare these images");
                            }
                            _ => panic!("Expected text part"),
                        }

                        // Second: first image
                        match &parts[1] {
                            ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                                assert_eq!(
                                    img_part.image_url.url,
                                    "https://example.com/image1.jpg"
                                );
                                assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Low));
                            }
                            _ => panic!("Expected image part"),
                        }

                        // Third: second image
                        match &parts[2] {
                            ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                                assert_eq!(
                                    img_part.image_url.url,
                                    "https://example.com/image2.jpg"
                                );
                                assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Low));
                            }
                            _ => panic!("Expected image part"),
                        }
                    }
                    _ => panic!("Expected array content"),
                }
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_text_only_optimized() {
        use dashflow::core::messages::{ContentBlock, MessageContent};

        // Single text block should be optimized to text format, not array
        let msg = Message::Human {
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
                ChatCompletionRequestUserMessageContent::Text(text) => {
                    assert_eq!(text, "Hello");
                }
                _ => panic!("Expected text content, not array (should be optimized)"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_image_detail_conversions() {
        use dashflow::core::messages::{ImageDetail, ImageSource};

        // Test all ImageDetail variants
        let low_image = ImageSource::Url {
            url: "https://example.com/image.jpg".to_string(),
        };
        let converted = convert_image_source(&low_image, Some(ImageDetail::Low));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::Low));

        let converted = convert_image_source(&low_image, Some(ImageDetail::High));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::High));

        let converted = convert_image_source(&low_image, Some(ImageDetail::Auto));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::Auto));

        let converted = convert_image_source(&low_image, None);
        assert!(converted.detail.is_none());
    }

    #[tokio::test]
    async fn test_rate_limiter_integration() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::{Duration, Instant};

        // Create a rate limiter that allows 2 requests per second
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            2.0, // 2 requests per second = 1 request every 500ms
            Duration::from_millis(10),
            2.0,
        ));

        let model = ChatXAI::new()
            .with_model("gpt-3.5-turbo")
            .with_rate_limiter(rate_limiter.clone());

        // Verify rate limiter is set
        assert!(model.rate_limiter().is_some());

        // Test that rate limiter controls timing
        let start = Instant::now();

        // First request should complete after ~500ms (first token generation)
        model.rate_limiter().unwrap().acquire().await;
        let first = start.elapsed();

        // Second request should complete after ~1000ms (second token)
        model.rate_limiter().unwrap().acquire().await;
        let second = start.elapsed();

        // Third request should complete after ~1500ms (third token)
        model.rate_limiter().unwrap().acquire().await;
        let third = start.elapsed();

        // Verify timing (with some tolerance for test execution)
        assert!(
            first >= Duration::from_millis(400),
            "First request took {:?}",
            first
        );
        assert!(
            first <= Duration::from_millis(600),
            "First request took {:?}",
            first
        );

        assert!(
            second >= Duration::from_millis(900),
            "Second request took {:?}",
            second
        );
        assert!(
            second <= Duration::from_millis(1100),
            "Second request took {:?}",
            second
        );

        assert!(
            third >= Duration::from_millis(1400),
            "Third request took {:?}",
            third
        );
        assert!(
            third <= Duration::from_millis(1600),
            "Third request took {:?}",
            third
        );
    }

    // ========================================================================
    // COMPREHENSIVE BUILDER PATTERN TESTS
    // ========================================================================

    #[test]
    fn test_builder_default_values() {
        let model = ChatXAI::new();
        assert_eq!(model.model, "grok-beta");
        assert!(model.temperature.is_none());
        assert!(model.max_tokens.is_none());
        assert!(model.top_p.is_none());
        assert!(model.frequency_penalty.is_none());
        assert!(model.presence_penalty.is_none());
        assert!(model.tools.is_none());
        assert!(model.tool_choice.is_none());
        assert!(model.response_format.is_none());
        assert!(model.rate_limiter.is_none());
    }

    #[test]
    fn test_builder_with_model_various_names() {
        // Test various xAI model names
        let model_names = ["grok-beta", "grok-vision-beta", "grok-2", "grok-2-mini"];
        for name in model_names {
            let model = ChatXAI::new().with_model(name);
            assert_eq!(model.model, name);
        }
    }

    #[test]
    fn test_builder_with_model_string_ownership() {
        // Test that Into<String> works for both &str and String
        let model1 = ChatXAI::new().with_model("grok-beta");
        let model2 = ChatXAI::new().with_model(String::from("grok-beta"));
        assert_eq!(model1.model, model2.model);
    }

    #[test]
    fn test_builder_temperature_boundary_values() {
        // Test temperature at various values
        let model = ChatXAI::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));

        let model = ChatXAI::new().with_temperature(1.0);
        assert_eq!(model.temperature, Some(1.0));

        let model = ChatXAI::new().with_temperature(2.0);
        assert_eq!(model.temperature, Some(2.0));

        let model = ChatXAI::new().with_temperature(0.5);
        assert_eq!(model.temperature, Some(0.5));
    }

    #[test]
    fn test_builder_max_tokens_various_values() {
        let values = [1u32, 100, 1000, 4096, 8192, 32768];
        for val in values {
            let model = ChatXAI::new().with_max_tokens(val);
            assert_eq!(model.max_tokens, Some(val));
        }
    }

    #[test]
    fn test_builder_top_p_boundary_values() {
        let model = ChatXAI::new().with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));

        let model = ChatXAI::new().with_top_p(0.5);
        assert_eq!(model.top_p, Some(0.5));

        let model = ChatXAI::new().with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    #[test]
    fn test_builder_frequency_penalty_boundary_values() {
        // Frequency penalty can be -2.0 to 2.0
        let model = ChatXAI::new().with_frequency_penalty(-2.0);
        assert_eq!(model.frequency_penalty, Some(-2.0));

        let model = ChatXAI::new().with_frequency_penalty(0.0);
        assert_eq!(model.frequency_penalty, Some(0.0));

        let model = ChatXAI::new().with_frequency_penalty(2.0);
        assert_eq!(model.frequency_penalty, Some(2.0));

        let model = ChatXAI::new().with_frequency_penalty(-0.5);
        assert_eq!(model.frequency_penalty, Some(-0.5));

        let model = ChatXAI::new().with_frequency_penalty(1.5);
        assert_eq!(model.frequency_penalty, Some(1.5));
    }

    #[test]
    fn test_builder_presence_penalty_boundary_values() {
        // Presence penalty can be -2.0 to 2.0
        let model = ChatXAI::new().with_presence_penalty(-2.0);
        assert_eq!(model.presence_penalty, Some(-2.0));

        let model = ChatXAI::new().with_presence_penalty(0.0);
        assert_eq!(model.presence_penalty, Some(0.0));

        let model = ChatXAI::new().with_presence_penalty(2.0);
        assert_eq!(model.presence_penalty, Some(2.0));
    }

    #[test]
    fn test_builder_chaining_all_options() {
        let model = ChatXAI::new()
            .with_model("grok-vision-beta")
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .with_top_p(0.95)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(-0.5)
            .with_json_mode();

        assert_eq!(model.model, "grok-vision-beta");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(2048));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.frequency_penalty, Some(0.5));
        assert_eq!(model.presence_penalty, Some(-0.5));
        assert!(model.response_format.is_some());
    }

    #[test]
    fn test_builder_retry_policy() {
        use dashflow::core::retry::RetryPolicy;

        let policy = RetryPolicy::exponential(5);
        let model = ChatXAI::new().with_retry_policy(policy);
        // RetryPolicy doesn't expose its internals, but we can verify it's set
        // by checking the model was created successfully
        assert_eq!(model.model, "grok-beta");
    }

    #[test]
    fn test_builder_retry_policy_fixed() {
        use dashflow::core::retry::RetryPolicy;

        let policy = RetryPolicy::fixed(3, 100); // max_retries, delay_ms
        let model = ChatXAI::new().with_retry_policy(policy);
        assert_eq!(model.model, "grok-beta");
    }

    #[test]
    fn test_builder_override_values() {
        // Test that later builder calls override earlier ones
        let model = ChatXAI::new()
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.temperature, Some(0.9));

        let model = ChatXAI::new()
            .with_model("grok-beta")
            .with_model("grok-vision-beta");
        assert_eq!(model.model, "grok-vision-beta");
    }

    #[test]
    fn test_default_trait() {
        let model: ChatXAI = Default::default();
        assert_eq!(model.model, "grok-beta");
        assert!(model.temperature.is_none());
    }

    #[test]
    fn test_model_clone() {
        let model = ChatXAI::new()
            .with_model("grok-vision-beta")
            .with_temperature(0.8)
            .with_max_tokens(1000);

        let cloned = model.clone();
        assert_eq!(cloned.model, "grok-vision-beta");
        assert_eq!(cloned.temperature, Some(0.8));
        assert_eq!(cloned.max_tokens, Some(1000));
    }

    #[test]
    fn test_model_debug_format() {
        let model = ChatXAI::new().with_model("grok-beta");
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("ChatXAI"));
        assert!(debug_str.contains("grok-beta"));
    }

    // ========================================================================
    // TOOL DEFINITION CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_convert_tool_definition_basic() {
        let tool_def = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };

        let converted = convert_tool_definition(&tool_def);
        assert_eq!(converted.function.name, "get_weather");
        assert_eq!(
            converted.function.description,
            Some("Get current weather".to_string())
        );
        assert!(converted.function.parameters.is_some());
    }

    #[test]
    fn test_convert_tool_definition_empty_description() {
        let tool_def = ToolDefinition {
            name: "simple_tool".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
        };

        let converted = convert_tool_definition(&tool_def);
        assert_eq!(converted.function.name, "simple_tool");
        assert!(converted.function.description.is_none());
    }

    #[test]
    fn test_convert_tool_definition_complex_parameters() {
        let tool_def = ToolDefinition {
            name: "create_user".to_string(),
            description: "Create a new user".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "User's name"},
                    "age": {"type": "integer", "minimum": 0},
                    "email": {"type": "string", "format": "email"},
                    "roles": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["name", "email"]
            }),
        };

        let converted = convert_tool_definition(&tool_def);
        assert_eq!(converted.function.name, "create_user");

        let params = converted.function.parameters.unwrap();
        assert_eq!(params.get("type").unwrap(), "object");
        assert!(params.get("properties").unwrap().get("name").is_some());
        assert!(params.get("properties").unwrap().get("roles").is_some());
    }

    // ========================================================================
    // TOOL CHOICE CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_convert_tool_choice_auto() {
        let choice = ToolChoice::Auto;
        let converted = convert_tool_choice(&choice);
        match converted {
            ChatCompletionToolChoiceOption::Auto => {}
            _ => panic!("Expected Auto"),
        }
    }

    #[test]
    fn test_convert_tool_choice_none() {
        let choice = ToolChoice::None;
        let converted = convert_tool_choice(&choice);
        match converted {
            ChatCompletionToolChoiceOption::None => {}
            _ => panic!("Expected None"),
        }
    }

    #[test]
    fn test_convert_tool_choice_required() {
        let choice = ToolChoice::Required;
        let converted = convert_tool_choice(&choice);
        match converted {
            ChatCompletionToolChoiceOption::Required => {}
            _ => panic!("Expected Required"),
        }
    }

    #[test]
    fn test_convert_tool_choice_specific() {
        let choice = ToolChoice::Specific("get_weather".to_string());
        let converted = convert_tool_choice(&choice);
        match converted {
            ChatCompletionToolChoiceOption::Named(named) => {
                assert_eq!(named.function.name, "get_weather");
            }
            _ => panic!("Expected Named"),
        }
    }

    // ========================================================================
    // MESSAGE CONVERSION EDGE CASES
    // ========================================================================

    #[test]
    fn test_message_conversion_function_to_tool() {
        let msg = Message::Function {
            content: "Function result".into(),
            name: "my_function".to_string(),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                // Function messages are converted to tool messages with generated ID
                assert_eq!(tool_msg.tool_call_id, "func_my_function");
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_message_conversion_empty_system() {
        let msg = Message::system("");
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    #[test]
    fn test_message_conversion_empty_human() {
        let msg = Message::human("");
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::User(_) => {}
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_empty_ai() {
        let msg = Message::ai("");
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::Assistant(_) => {}
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_message_conversion_ai_multiple_tool_calls() {
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"location": "NYC"}),
                tool_type: "tool_call".to_string(),
                index: Some(0),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "get_time".to_string(),
                args: serde_json::json!({"timezone": "EST"}),
                tool_type: "tool_call".to_string(),
                index: Some(1),
            },
            ToolCall {
                id: "call_3".to_string(),
                name: "calculate".to_string(),
                args: serde_json::json!({"expression": "2+2"}),
                tool_type: "tool_call".to_string(),
                index: Some(2),
            },
        ];

        let msg = Message::AI {
            content: "Let me check multiple things.".into(),
            tool_calls,
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                let calls = assistant_msg.tool_calls.unwrap();
                assert_eq!(calls.len(), 3);
                assert_eq!(calls[0].id, "call_1");
                assert_eq!(calls[1].id, "call_2");
                assert_eq!(calls[2].id, "call_3");
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_message_conversion_system_long_content() {
        let long_content = "a".repeat(10000);
        let msg = Message::system(long_content);
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    #[test]
    fn test_message_conversion_human_unicode() {
        let msg = Message::human("こんにちは世界 🌍 مرحبا");
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
                ChatCompletionRequestUserMessageContent::Text(text) => {
                    assert_eq!(text, "こんにちは世界 🌍 مرحبا");
                }
                _ => panic!("Expected text content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_newlines() {
        let msg = Message::human("Line 1\nLine 2\nLine 3");
        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
                ChatCompletionRequestUserMessageContent::Text(text) => {
                    assert!(text.contains('\n'));
                }
                _ => panic!("Expected text content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_tool_with_artifact() {
        let msg = Message::Tool {
            content: "Tool result".into(),
            tool_call_id: "call_abc".to_string(),
            artifact: Some(serde_json::json!({"data": "artifact_data"})),
            status: Some("success".to_string()),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                assert_eq!(tool_msg.tool_call_id, "call_abc");
            }
            _ => panic!("Expected tool message"),
        }
    }

    // ========================================================================
    // IMAGE CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_convert_image_source_url_no_detail() {
        use dashflow::core::messages::ImageSource;

        let source = ImageSource::Url {
            url: "https://example.com/img.png".to_string(),
        };
        let converted = convert_image_source(&source, None);
        assert_eq!(converted.url, "https://example.com/img.png");
        assert!(converted.detail.is_none());
    }

    #[test]
    fn test_convert_image_source_base64_formats() {
        use dashflow::core::messages::ImageSource;

        let formats = ["image/png", "image/jpeg", "image/gif", "image/webp"];
        for format in formats {
            let source = ImageSource::Base64 {
                media_type: format.to_string(),
                data: "SGVsbG8=".to_string(),
            };
            let converted = convert_image_source(&source, None);
            assert!(converted.url.starts_with(&format!("data:{};base64,", format)));
        }
    }

    #[test]
    fn test_message_conversion_blocks_with_empty_text_filtered() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        // Empty text blocks should be filtered out
        let msg = Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: String::new(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/img.jpg".to_string(),
                    },
                    detail: None,
                },
            ]),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();
        match converted {
            ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
                ChatCompletionRequestUserMessageContent::Array(parts) => {
                    // Only the image should remain, empty text is filtered
                    assert_eq!(parts.len(), 1);
                    match &parts[0] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(_) => {}
                        _ => panic!("Expected image part"),
                    }
                }
                _ => panic!("Expected array content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    // ========================================================================
    // IDENTIFYING PARAMS TESTS
    // ========================================================================

    #[test]
    fn test_identifying_params_minimal() {
        let model = ChatXAI::new();
        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), "grok-beta");
        assert!(params.get("temperature").is_none());
        assert!(params.get("max_tokens").is_none());
    }

    #[test]
    fn test_identifying_params_with_temperature() {
        let model = ChatXAI::new().with_temperature(0.7);
        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), "grok-beta");
        assert_eq!(params.get("temperature").unwrap(), 0.7);
    }

    #[test]
    fn test_identifying_params_with_max_tokens() {
        let model = ChatXAI::new().with_max_tokens(1000);
        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), "grok-beta");
        assert_eq!(params.get("max_tokens").unwrap(), 1000);
    }

    #[test]
    fn test_identifying_params_full() {
        let model = ChatXAI::new()
            .with_model("grok-vision-beta")
            .with_temperature(0.5)
            .with_max_tokens(2000);
        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), "grok-vision-beta");
        assert_eq!(params.get("temperature").unwrap(), 0.5);
        assert_eq!(params.get("max_tokens").unwrap(), 2000);
    }

    // ========================================================================
    // LLM_TYPE TESTS
    // ========================================================================

    #[test]
    fn test_llm_type_returns_static() {
        use dashflow::core::language_models::ChatModel;
        let model = ChatXAI::new();
        // llm_type returns a static string, not the configured model name
        assert_eq!(model.llm_type(), "grok-beta");
    }

    // ========================================================================
    // AS_ANY TESTS
    // ========================================================================

    #[test]
    fn test_as_any_downcast() {
        use dashflow::core::language_models::ChatModel;
        let model = ChatXAI::new().with_model("grok-vision-beta");
        let any = model.as_any();
        let downcast = any.downcast_ref::<ChatXAI>().unwrap();
        assert_eq!(downcast.model, "grok-vision-beta");
    }

    // ========================================================================
    // FROM_CONFIG TESTS
    // ========================================================================

    #[test]
    fn test_from_config_wrong_provider() {
        use dashflow::core::config_loader::{ChatModelConfig, SecretReference};

        let config = ChatModelConfig::OpenAI {
            model: "gpt-4".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let result = ChatXAI::from_config(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Expected XAI config"));
    }

    #[test]
    fn test_from_config_anthropic_provider() {
        use dashflow::core::config_loader::{ChatModelConfig, SecretReference};

        let config = ChatModelConfig::Anthropic {
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = ChatXAI::from_config(&config);
        assert!(result.is_err());
    }

    // ========================================================================
    // RATE LIMITER TESTS
    // ========================================================================

    #[test]
    fn test_rate_limiter_none_by_default() {
        use dashflow::core::language_models::ChatModel;
        let model = ChatXAI::new();
        assert!(model.rate_limiter().is_none());
    }

    #[test]
    fn test_rate_limiter_set() {
        use dashflow::core::language_models::ChatModel;
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let model = ChatXAI::new().with_rate_limiter(limiter);
        assert!(model.rate_limiter().is_some());
    }

    // ========================================================================
    // WITH_CONFIG TESTS
    // ========================================================================

    #[test]
    fn test_with_config_custom_base_url() {
        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base("https://custom-api.example.com/v1");

        let model = ChatXAI::with_config(config);
        // The model is created successfully with custom config
        assert_eq!(model.model, "grok-beta");
    }

    #[test]
    fn test_with_config_empty_api_key() {
        let config = OpenAIConfig::new()
            .with_api_key("")
            .with_api_base("https://api.x.ai/v1");

        let model = ChatXAI::with_config(config);
        // Model is created even with empty key (will fail on API call)
        assert_eq!(model.model, "grok-beta");
    }

    // ========================================================================
    // STRUCTURED OUTPUT EDGE CASES
    // ========================================================================

    #[test]
    fn test_structured_output_empty_schema() {
        let model = ChatXAI::new().with_structured_output(
            "empty_schema",
            serde_json::json!({}),
            None,
            false,
        );

        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "empty_schema");
                assert_eq!(json_schema.schema, Some(serde_json::json!({})));
            }
            _ => panic!("Expected JsonSchema"),
        }
    }

    #[test]
    fn test_structured_output_override() {
        let schema1 = serde_json::json!({"type": "string"});
        let schema2 = serde_json::json!({"type": "number"});

        let model = ChatXAI::new()
            .with_structured_output("first", schema1, None, false)
            .with_structured_output("second", schema2.clone(), None, true);

        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "second");
                assert_eq!(json_schema.schema, Some(schema2));
                assert_eq!(json_schema.strict, Some(true));
            }
            _ => panic!("Expected JsonSchema"),
        }
    }

    #[test]
    fn test_json_mode_override_structured_output() {
        let schema = serde_json::json!({"type": "object"});

        let model = ChatXAI::new()
            .with_structured_output("schema", schema, None, true)
            .with_json_mode();

        // JSON mode overrides structured output
        match model.response_format.unwrap() {
            ResponseFormat::JsonObject => {}
            _ => panic!("Expected JsonObject"),
        }
    }

    // ========================================================================
    // TOOL BUILDER EDGE CASES
    // ========================================================================

    #[test]
    fn test_with_tools_name_only() {
        let tool = serde_json::json!({"name": "minimal_tool"});
        let model = ChatXAI::new().with_tools(vec![tool]);

        let tools = model.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "minimal_tool");
        assert!(tools[0].function.description.is_none());
        assert!(tools[0].function.parameters.is_none());
    }

    #[test]
    fn test_with_tools_all_invalid_filtered() {
        // All tools without names should be filtered out
        let tools = vec![
            serde_json::json!({"description": "No name 1"}),
            serde_json::json!({"parameters": {"type": "object"}}),
            serde_json::json!({}),
        ];

        let model = ChatXAI::new().with_tools(tools);
        assert!(model.tools.is_none());
    }

    #[test]
    fn test_with_tools_mixed_valid_invalid() {
        let tools = vec![
            serde_json::json!({"name": "valid_1"}),
            serde_json::json!({"description": "no name"}),
            serde_json::json!({"name": "valid_2", "description": "Second tool"}),
            serde_json::json!({"invalid": true}),
        ];

        let model = ChatXAI::new().with_tools(tools);
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 2);
        assert_eq!(bound_tools[0].function.name, "valid_1");
        assert_eq!(bound_tools[1].function.name, "valid_2");
    }

    #[test]
    fn test_with_tools_duplicate_names() {
        // Duplicate names are allowed at builder level (API may reject)
        let tools = vec![
            serde_json::json!({"name": "same_name", "description": "First"}),
            serde_json::json!({"name": "same_name", "description": "Second"}),
        ];

        let model = ChatXAI::new().with_tools(tools);
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 2);
    }

    #[test]
    fn test_with_tools_special_characters_in_name() {
        let tool = serde_json::json!({
            "name": "get_user-info_v2",
            "description": "Tool with special chars"
        });

        let model = ChatXAI::new().with_tools(vec![tool]);
        let tools = model.tools.unwrap();
        assert_eq!(tools[0].function.name, "get_user-info_v2");
    }

    // ========================================================================
    // CONTENT CONVERSION EDGE CASES
    // ========================================================================

    #[test]
    fn test_convert_content_text_only() {
        use dashflow::core::messages::MessageContent;

        let content = MessageContent::Text("Simple text".to_string());
        let converted = convert_content(&content);
        match converted {
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "Simple text");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_convert_content_empty_blocks() {
        use dashflow::core::messages::MessageContent;

        let content = MessageContent::Blocks(vec![]);
        let converted = convert_content(&content);
        match converted {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert!(parts.is_empty());
            }
            _ => panic!("Expected empty array"),
        }
    }

    #[test]
    fn test_convert_content_tool_use_block_filtered() {
        use dashflow::core::messages::{ContentBlock, MessageContent};

        // ToolUse and ToolResult blocks should be filtered from user content
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Check this".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "get_data".to_string(),
                input: serde_json::json!({}),
            },
        ]);

        let converted = convert_content(&content);
        match converted {
            // Single text block optimized to text format
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "Check this");
            }
            _ => panic!("Expected optimized text content"),
        }
    }
}

/// Standard conformance tests
///
/// These tests verify that ChatXAI behaves consistently with other
/// ChatModel implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(
    deprecated,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::unwrap_used
)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::chat_model_tests::*;
    use dashflow_test_utils::{init_test_env, xai_credentials};

    /// Helper function to create a test model with standard settings
    ///
    /// Uses grok-beta for testing
    fn create_test_model() -> ChatXAI {
        ChatXAI::new()
            .with_model("grok-beta")
            .with_temperature(0.0) // Deterministic for testing
            .with_max_tokens(100) // Limit tokens for cost/speed
    }

    /// Require API key for ignored integration tests.
    fn check_credentials() {
        init_test_env().ok();
        xai_credentials().expect("XAI_API_KEY must be set");
    }

    /// Standard Test 1: Basic invoke
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_invoke_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_invoke(&model).await;
    }

    /// Standard Test 2: Streaming
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_stream_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_stream(&model).await;
    }

    /// Standard Test 3: Batch processing
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_batch_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_batch(&model).await;
    }

    /// Standard Test 4: Multi-turn conversation
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_conversation_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_conversation(&model).await;
    }

    /// Standard Test 4b: Double messages conversation
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_double_messages_conversation_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_double_messages_conversation(&model).await;
    }

    /// Standard Test 4c: Message with name field
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_message_with_name_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_message_with_name(&model).await;
    }

    /// Standard Test 5: Stop sequences
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_stop_sequence_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_stop_sequence(&model).await;
    }

    /// Standard Test 6: Usage metadata
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_usage_metadata_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_usage_metadata(&model).await;
    }

    /// Standard Test 7: Empty messages
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_empty_messages_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_empty_messages(&model).await;
    }

    /// Standard Test 8: Long conversation
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_long_conversation_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_long_conversation(&model).await;
    }

    /// Standard Test 9: Special characters
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_special_characters_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_special_characters(&model).await;
    }

    /// Standard Test 10: Unicode and emoji
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_unicode_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_unicode(&model).await;
    }

    /// Standard Test 11: Tool calling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_tool_calling_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_tool_calling(&model).await;
    }

    /// Standard Test 12: Structured output
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_structured_output_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_structured_output(&model).await;
    }

    /// Standard Test 13: JSON mode
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_json_mode_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_json_mode(&model).await;
    }

    /// Standard Test 14: Usage metadata in streaming
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_usage_metadata_streaming_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_usage_metadata_streaming(&model).await;
    }

    /// Standard Test 15: System message handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_system_message_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_system_message(&model).await;
    }

    /// Standard Test 16: Empty content handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_empty_content_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_empty_content(&model).await;
    }

    /// Standard Test 17: Large input handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_large_input_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_large_input(&model).await;
    }

    /// Standard Test 18: Concurrent generation
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_concurrent_generation_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_concurrent_generation(&model).await;
    }

    /// Standard Test 19: Error recovery
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_error_recovery_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_error_recovery(&model).await;
    }

    /// Standard Test 20: Response consistency
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_response_consistency_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_response_consistency(&model).await;
    }

    /// Standard Test 21: Tool calling with no arguments
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_tool_calling_with_no_arguments_standard() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_tool_calling_with_no_arguments(&model).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS - Advanced Edge Cases
    // ========================================================================

    /// Comprehensive Test 1: Streaming with timeout
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_stream_with_timeout_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_stream_with_timeout(&model).await;
    }

    /// Comprehensive Test 2: Streaming interruption handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_stream_interruption_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_stream_interruption(&model).await;
    }

    /// Comprehensive Test 3: Empty stream handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_stream_empty_response_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_stream_empty_response(&model).await;
    }

    /// Comprehensive Test 4: Multiple system messages
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_multiple_system_messages_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_multiple_system_messages(&model).await;
    }

    /// Comprehensive Test 5: Empty system message
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_empty_system_message_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_empty_system_message(&model).await;
    }

    /// Comprehensive Test 6: Temperature edge cases
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_temperature_extremes_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_temperature_extremes(&model).await;
    }

    /// Comprehensive Test 7: Max tokens enforcement
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_max_tokens_limit_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_max_tokens_limit(&model).await;
    }

    /// Comprehensive Test 8: Invalid stop sequences
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_invalid_stop_sequences_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_invalid_stop_sequences(&model).await;
    }

    /// Comprehensive Test 9: Context window overflow
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_context_window_overflow_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_context_window_overflow(&model).await;
    }

    /// Comprehensive Test 10: Rapid consecutive calls
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_rapid_consecutive_calls_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_rapid_consecutive_calls(&model).await;
    }

    /// Comprehensive Test 11: Network error handling
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_network_error_handling_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_network_error_handling(&model).await;
    }

    /// Comprehensive Test 12: Malformed input recovery
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_malformed_input_recovery_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_malformed_input_recovery(&model).await;
    }

    /// Comprehensive Test 13: Very long single message
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_very_long_single_message_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_very_long_single_message(&model).await;
    }

    /// Comprehensive Test 14: Response format consistency
    #[tokio::test]
    #[ignore = "requires XAI_API_KEY"]
    async fn test_response_format_consistency_comprehensive() {
        check_credentials();
        if xai_credentials().is_err() {
            return;
        }
        let model = create_test_model();
        test_response_format_consistency(&model).await;
    }
}
