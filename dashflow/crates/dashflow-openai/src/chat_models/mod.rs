//! `OpenAI` chat model implementation

use async_openai::{
    config::{AzureConfig, OpenAIConfig},
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
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{
        env_string_or_default, AZURE_OPENAI_API_KEY, AZURE_OPENAI_ENDPOINT, OPENAI_API_KEY,
    },
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, InvalidToolCall, Message, ToolCall},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION},
    usage::UsageMetadata,
};
use futures::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// OpenAI chat model configuration and client
///
/// `ChatOpenAI` implements the [`ChatModel`] trait for OpenAI's chat completions API.
/// It supports GPT-4, GPT-3.5-turbo, and other OpenAI models with streaming, tool calling,
/// and structured outputs.
///
/// # Example
///
/// ```no_run
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatOpenAI::with_config(Default::default())
///         .with_model("gpt-4")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Configuration Options
///
/// | Method | Description | Default |
/// |--------|-------------|---------|
/// | `with_model()` | Model name (gpt-4, gpt-3.5-turbo, etc.) | gpt-3.5-turbo |
/// | `with_temperature()` | Sampling temperature (0.0-2.0) | None |
/// | `with_max_tokens()` | Maximum tokens to generate | None |
/// | `with_top_p()` | Top-p nucleus sampling | None |
/// | `with_retry_policy()` | Retry configuration | Exponential(3) |
/// | `with_rate_limiter()` | Rate limiting | None |
///
/// # See Also
///
/// - [`AzureChatOpenAI`] - Azure OpenAI Service variant
/// - [`OpenAIEmbeddings`](crate::OpenAIEmbeddings) - Embedding models
/// - [`OpenAIStructuredChatModel`](crate::OpenAIStructuredChatModel) - Structured outputs
/// - [`ChatModel`] - The trait implemented by this type
/// - [`RetryPolicy`] - Configure retry behavior
///
/// [`ChatModel`]: dashflow::core::language_models::ChatModel
/// [`RetryPolicy`]: dashflow::core::retry::RetryPolicy
#[derive(Clone, Debug)]
pub struct ChatOpenAI {
    /// `OpenAI` client
    client: Arc<Client<OpenAIConfig>>,

    /// Model name (e.g., "gpt-4", "gpt-3.5-turbo")
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

    /// Number of completions to generate
    n: Option<u8>,

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

impl ChatOpenAI {
    /// Create a new `ChatOpenAI` instance with default settings
    ///
    /// Uses `OPENAI_API_KEY` environment variable for authentication
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_openai::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(OpenAIConfig::default())
    }

    /// Create a new `ChatOpenAI` instance with custom configuration
    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            model: "gpt-3.5-turbo".to_string(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            n: None,
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

    /// Set the number of completions to generate
    #[must_use]
    pub fn with_n(mut self, n: u8) -> Self {
        self.n = Some(n);
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
    /// use dashflow_openai::ChatOpenAI;
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
    /// let model = ChatOpenAI::with_config(Default::default())
    ///     .with_model("gpt-4")
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
    /// use dashflow_openai::ChatOpenAI;
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
    /// let model = ChatOpenAI::with_config(Default::default()).with_tools(vec![tool]);
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
    /// use dashflow_openai::ChatOpenAI;
    ///
    /// // Force model to use tools
    /// let model = ChatOpenAI::with_config(Default::default())
    ///     .with_tool_choice(Some("required".to_string()));
    ///
    /// // Force specific tool
    /// let model = ChatOpenAI::with_config(Default::default())
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
    /// use dashflow_openai::ChatOpenAI;
    /// use dashflow::core::messages::Message;
    /// use dashflow::core::language_models::ChatModel;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ChatOpenAI::with_config(Default::default())
    ///     .with_model("gpt-4")
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
    /// use dashflow_openai::ChatOpenAI;
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
    /// let model = ChatOpenAI::with_config(Default::default())
    ///     .with_model("gpt-4")
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

    /// Create a `ChatOpenAI` instance from a configuration
    ///
    /// This method constructs a `ChatOpenAI` model from a `ChatModelConfig::OpenAI` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be `OpenAI` variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatOpenAI` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not an `OpenAI` variant
    /// - API key environment variable cannot be resolved
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
    /// use dashflow_openai::ChatOpenAI;
    ///
    /// let config = ChatModelConfig::OpenAI {
    ///     model: "gpt-4".to_string(),
    ///     api_key: SecretReference::EnvVar { env: "OPENAI_API_KEY".to_string() },
    ///     temperature: Some(0.7),
    ///     max_tokens: None,
    ///     base_url: None,
    ///     organization: None,
    /// };
    ///
    /// let chat_model = ChatOpenAI::from_config(&config).unwrap();
    /// ```
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::OpenAI {
                model,
                api_key,
                temperature,
                max_tokens,
                base_url,
                organization,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                // Build the OpenAI config
                let mut openai_config = OpenAIConfig::default().with_api_key(&resolved_api_key);

                // Set base URL if provided
                if let Some(url) = base_url {
                    openai_config = openai_config.with_api_base(url);
                }

                // Set organization if provided
                if let Some(org) = organization {
                    openai_config = openai_config.with_org_id(org);
                }

                // Create the ChatOpenAI instance
                let mut chat_model = Self::with_config(openai_config).with_model(model);

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
                "Expected OpenAI config, got {} config",
                config.provider()
            ))),
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatOpenAI {
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

/// Convert `DashFlow` `ToolDefinition` to `OpenAI` `ChatCompletionTool`
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

/// Convert `DashFlow` `ToolChoice` to `OpenAI` `ChatCompletionToolChoiceOption`
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

#[async_trait]
impl ChatModel for ChatOpenAI {
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Client> cloned for retry closure
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
        if let Some(n) = self.n {
            request_builder.n(n);
        }

        // Stop sequences: prefer parameter over struct field
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Tools: prefer parameter over struct field
        if let Some(tool_defs) = tools {
            let openai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(openai_tools);
        } else if let Some(ref tools) = self.tools {
            request_builder.tools(tools.clone());
        }

        // Tool choice: prefer parameter over struct field
        if let Some(tc) = tool_choice {
            request_builder.tool_choice(convert_tool_choice(tc));
        } else if let Some(ref tool_choice) = self.tool_choice {
            request_builder.tool_choice(tool_choice.clone());
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
                    .map_err(|e| Error::api(format!("OpenAI API error: {e}")))
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
        // Extract usage metadata from response (applies to all generations)
        let usage_metadata = response
            .usage
            .as_ref()
            .map(|usage| UsageMetadata::new(usage.prompt_tokens, usage.completion_tokens));

        let model_name = response.model.clone();
        let system_fingerprint = response.system_fingerprint.clone();

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

                // Build AIMessage with tool calls and usage metadata
                let message = if !tool_calls.is_empty() || !invalid_tool_calls.is_empty() {
                    // Need to manually construct Message::AI since AIMessage doesn't expose setters for invalid_tool_calls
                    Message::AI {
                        content: content.into(),
                        tool_calls,
                        invalid_tool_calls,
                        usage_metadata: usage_metadata.clone(),
                        fields: Default::default(),
                    }
                } else {
                    let mut ai_msg = AIMessage::new(content);
                    if let Some(usage) = usage_metadata.clone() {
                        ai_msg = ai_msg.with_usage(usage);
                    }
                    ai_msg.into()
                };

                // Build generation_info with response metadata
                let mut generation_info = HashMap::new();
                if let Some(finish_reason) = choice.finish_reason {
                    generation_info.insert(
                        "finish_reason".to_string(),
                        serde_json::json!(finish_reason),
                    );
                }
                generation_info.insert(
                    "model_name".to_string(),
                    serde_json::json!(model_name.clone()),
                );
                if let Some(fingerprint) = &system_fingerprint {
                    generation_info.insert(
                        "system_fingerprint".to_string(),
                        serde_json::json!(fingerprint),
                    );
                }
                if let Some(logprobs) = choice.logprobs {
                    generation_info.insert(
                        "logprobs".to_string(),
                        serde_json::to_value(logprobs).unwrap_or(serde_json::json!(null)),
                    );
                }

                ChatGeneration {
                    message,
                    generation_info: Some(generation_info),
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
        // Note: n parameter is not supported with streaming

        // Stop sequences: prefer parameter over struct field
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Tools: prefer parameter over struct field
        if let Some(tool_defs) = tools {
            let openai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(openai_tools);
        } else if let Some(ref tools) = self.tools {
            request_builder.tools(tools.clone());
        }

        // Tool choice: prefer parameter over struct field
        if let Some(tc) = tool_choice {
            request_builder.tool_choice(convert_tool_choice(tc));
        } else if let Some(ref tool_choice) = self.tool_choice {
            request_builder.tool_choice(tool_choice.clone());
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
            .map_err(|e| Error::api(format!("OpenAI streaming API error: {e}")))?;

        // Convert OpenAI stream to DashFlow stream
        let output_stream = stream! {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            let content = choice.delta.content.unwrap_or_default();
                            let tool_calls = choice.delta.tool_calls.as_ref().map(|calls| {
                                calls.iter().map(|tc| {
                                    // OpenAI streaming: args sent as incremental string chunks
                                    // E.g.: "{"", "input", "\":", "3", "}" sent separately
                                    // Core's merge() accumulates strings, to_message() parses complete JSON
                                    let args_str = tc.function.as_ref()
                                        .and_then(|f| f.arguments.as_deref())
                                        .unwrap_or("");
                                    // Store as String value (will be accumulated in core's merge logic)
                                    let args = serde_json::Value::String(args_str.to_string());

                                    ToolCall {
                                        name: tc.function.as_ref().map(|f| f.name.clone()).unwrap_or_default().unwrap_or_default(),
                                        args,
                                        id: tc.id.clone().unwrap_or_default(),
                                        tool_type: "tool_call".to_string(),
                                        index: Some(tc.index as usize),
                                    }
                                }).collect::<Vec<_>>()
                            }).unwrap_or_default();

                            let mut chunk = AIMessageChunk::new(content);
                            if !tool_calls.is_empty() {
                                chunk.tool_calls = tool_calls;
                            }

                            yield Ok(ChatGenerationChunk {
                                message: chunk,
                                generation_info: Default::default(),
                            });
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
        "openai-chat"
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Azure `OpenAI` chat model configuration and client
///
/// Use this for Azure `OpenAI` deployments. It wraps the Azure-specific configuration
/// including deployment name, API version, and Azure endpoint.
///
/// # Example
/// ```no_run
/// use dashflow_openai::AzureChatOpenAI;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = AzureChatOpenAI::with_config(Default::default())
///         .with_deployment("my-gpt4-deployment")
///         .with_api_version("2024-05-01-preview")
///         .with_azure_endpoint("https://my-resource.openai.azure.com")
///         .with_api_key("my-azure-api-key")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Environment Variables
/// - `AZURE_OPENAI_API_KEY` or `OPENAI_API_KEY`: API key for authentication
/// - `AZURE_OPENAI_ENDPOINT`: Azure endpoint URL
/// - `AZURE_OPENAI_API_VERSION`: API version (e.g., "2024-05-01-preview")
#[derive(Clone, Debug)]
pub struct AzureChatOpenAI {
    /// Azure `OpenAI` client
    client: Arc<Client<AzureConfig>>,

    /// Model name for tracing (e.g., "gpt-4", "gpt-35-turbo")
    /// Note: This is the underlying `OpenAI` model, not the Azure deployment name
    model: String,

    /// Deployment name (required for Azure)
    deployment_name: Option<String>,

    /// API version (required for Azure)
    api_version: Option<String>,

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

    /// Number of completions to generate
    n: Option<u8>,

    /// Tools available for the model to call
    tools: Option<Vec<ChatCompletionTool>>,

    /// Controls which (if any) tool is called by the model
    tool_choice: Option<ChatCompletionToolChoiceOption>,

    /// Response format (text, `json_object`, or `json_schema`)
    response_format: Option<ResponseFormat>,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,
}

// Serialization implementation for ChatOpenAI
impl Serializable for ChatOpenAI {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "openai".to_string(),
            "ChatOpenAI".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Model name (required)
        kwargs.insert("model".to_string(), serde_json::json!(self.model));

        // Optional parameters (only include if set)
        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(max_tok) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }

        if let Some(tp) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(tp));
        }

        if let Some(freq_pen) = self.frequency_penalty {
            kwargs.insert("frequency_penalty".to_string(), serde_json::json!(freq_pen));
        }

        if let Some(pres_pen) = self.presence_penalty {
            kwargs.insert("presence_penalty".to_string(), serde_json::json!(pres_pen));
        }

        if let Some(n_val) = self.n {
            kwargs.insert("n".to_string(), serde_json::json!(n_val));
        }

        // Note: tools, tool_choice, response_format, and rate_limiter are not serialized
        // They should be configured at runtime for security and flexibility

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        // Mark API key as a secret - it will be loaded from OPENAI_API_KEY env var
        secrets.insert("api_key".to_string(), "OPENAI_API_KEY".to_string());
        secrets
    }
}

// Serialization implementation for AzureChatOpenAI
impl Serializable for AzureChatOpenAI {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "azure_openai".to_string(),
            "AzureChatOpenAI".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Model name (required)
        kwargs.insert("model".to_string(), serde_json::json!(self.model));

        // Azure-specific parameters
        if let Some(ref deployment) = self.deployment_name {
            kwargs.insert("deployment_name".to_string(), serde_json::json!(deployment));
        }

        if let Some(ref api_version) = self.api_version {
            kwargs.insert("api_version".to_string(), serde_json::json!(api_version));
        }

        // Optional parameters (only include if set)
        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(max_tok) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }

        if let Some(tp) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(tp));
        }

        if let Some(freq_pen) = self.frequency_penalty {
            kwargs.insert("frequency_penalty".to_string(), serde_json::json!(freq_pen));
        }

        if let Some(pres_pen) = self.presence_penalty {
            kwargs.insert("presence_penalty".to_string(), serde_json::json!(pres_pen));
        }

        if let Some(n_val) = self.n {
            kwargs.insert("n".to_string(), serde_json::json!(n_val));
        }

        // Note: tools, tool_choice, response_format, and endpoint are not serialized
        // They should be configured at runtime for security and flexibility

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        // Mark API key as a secret - it will be loaded from AZURE_OPENAI_API_KEY or OPENAI_API_KEY env var
        secrets.insert("api_key".to_string(), "AZURE_OPENAI_API_KEY".to_string());
        secrets.insert(
            "azure_endpoint".to_string(),
            "AZURE_OPENAI_ENDPOINT".to_string(),
        );
        secrets
    }
}

impl AzureChatOpenAI {
    /// Create a new `AzureChatOpenAI` instance with default settings
    ///
    /// Uses environment variables:
    /// - `AZURE_OPENAI_API_KEY` or `OPENAI_API_KEY` for authentication
    /// - `AZURE_OPENAI_ENDPOINT` for Azure endpoint (optional, can be set via `with_azure_endpoint`)
    /// - `AZURE_OPENAI_API_VERSION` for API version (optional, can be set via `with_api_version`)
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_azure_openai::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(AzureConfig::default())
    }

    /// Create a new `AzureChatOpenAI` instance with custom Azure configuration
    #[must_use]
    pub fn with_config(config: AzureConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            model: "gpt-35-turbo".to_string(),
            deployment_name: None,
            api_version: None,
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            n: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            retry_policy: RetryPolicy::exponential(3),
        }
    }

    /// Set the deployment name (Azure-specific)
    ///
    /// This is the name of your Azure `OpenAI` deployment, not the underlying model name.
    /// Required for Azure `OpenAI`.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_openai::AzureChatOpenAI;
    ///
    /// let model = AzureChatOpenAI::with_config(Default::default())
    ///     .with_deployment("my-gpt4-deployment");
    /// ```
    pub fn with_deployment(self, deployment_name: impl Into<String>) -> Self {
        let deployment = deployment_name.into();
        let config = AzureConfig::default()
            .with_deployment_id(&deployment)
            .with_api_version(self.api_version.as_deref().unwrap_or("2024-05-01-preview"))
            .with_api_key({
                let azure_key = env_string_or_default(AZURE_OPENAI_API_KEY, "");
                if azure_key.is_empty() {
                    env_string_or_default(OPENAI_API_KEY, "")
                } else {
                    azure_key
                }
            })
            .with_api_base(env_string_or_default(AZURE_OPENAI_ENDPOINT, ""));

        Self {
            client: Arc::new(Client::with_config(config)),
            deployment_name: Some(deployment),
            ..self
        }
    }

    /// Set the API version (Azure-specific)
    ///
    /// Specifies which Azure `OpenAI` REST API version to use.
    /// Recommended: "2024-05-01-preview" or later.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_openai::AzureChatOpenAI;
    ///
    /// let model = AzureChatOpenAI::with_config(Default::default())
    ///     .with_api_version("2024-05-01-preview");
    /// ```
    #[must_use]
    #[allow(clippy::unwrap_used)] // SAFETY: self.api_version set to Some on line above
    pub fn with_api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = Some(api_version.into());
        // Need to rebuild client with new config
        if let Some(ref deployment) = self.deployment_name {
            let config = AzureConfig::default()
                .with_deployment_id(deployment)
                .with_api_version(self.api_version.as_deref().unwrap())
                .with_api_key({
                    let azure_key = env_string_or_default(AZURE_OPENAI_API_KEY, "");
                    if azure_key.is_empty() {
                        env_string_or_default(OPENAI_API_KEY, "")
                    } else {
                        azure_key
                    }
                })
                .with_api_base(env_string_or_default(AZURE_OPENAI_ENDPOINT, ""));
            self.client = Arc::new(Client::with_config(config));
        }
        self
    }

    /// Set the Azure endpoint
    ///
    /// Format: `https://your-resource-name.openai.azure.com`
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_openai::AzureChatOpenAI;
    ///
    /// let model = AzureChatOpenAI::with_config(Default::default())
    ///     .with_azure_endpoint("https://my-resource.openai.azure.com");
    /// ```
    pub fn with_azure_endpoint(self, endpoint: impl Into<String>) -> Self {
        let endpoint_str = endpoint.into();

        let config = AzureConfig::default()
            .with_api_base(&endpoint_str)
            .with_deployment_id(self.deployment_name.as_deref().unwrap_or(""))
            .with_api_version(self.api_version.as_deref().unwrap_or("2024-05-01-preview"))
            .with_api_key({
                let azure_key = env_string_or_default(AZURE_OPENAI_API_KEY, "");
                if azure_key.is_empty() {
                    env_string_or_default(OPENAI_API_KEY, "")
                } else {
                    azure_key
                }
            });

        Self {
            client: Arc::new(Client::with_config(config)),
            ..self
        }
    }

    /// Set the API key for Azure `OpenAI`
    ///
    /// If not provided, will use `AZURE_OPENAI_API_KEY` or `OPENAI_API_KEY` env var
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_openai::AzureChatOpenAI;
    ///
    /// let model = AzureChatOpenAI::with_config(Default::default())
    ///     .with_api_key("my-azure-api-key");
    /// ```
    pub fn with_api_key(self, api_key: impl Into<String>) -> Self {
        let key = api_key.into();

        let config = AzureConfig::default()
            .with_api_key(&key)
            .with_api_base(env_string_or_default(AZURE_OPENAI_ENDPOINT, ""))
            .with_deployment_id(self.deployment_name.as_deref().unwrap_or(""))
            .with_api_version(self.api_version.as_deref().unwrap_or("2024-05-01-preview"));

        Self {
            client: Arc::new(Client::with_config(config)),
            ..self
        }
    }

    /// Set the model name for tracing purposes
    ///
    /// This is the underlying `OpenAI` model (e.g., "gpt-4", "gpt-35-turbo"),
    /// not the Azure deployment name. Used for tracking and logging.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_openai::AzureChatOpenAI;
    ///
    /// let model = AzureChatOpenAI::with_config(Default::default())
    ///     .with_model("gpt-4");
    /// ```
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

    /// Set the number of completions to generate
    #[must_use]
    pub fn with_n(mut self, n: u8) -> Self {
        self.n = Some(n);
        self
    }

    /// Set the retry policy
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Bind tools to the model for function calling
    ///
    /// Same interface as `ChatOpenAI`. See `ChatOpenAI::with_tools` for details.
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
    /// Same interface as `ChatOpenAI`. See `ChatOpenAI::with_tool_choice` for details.
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
    /// Same interface as `ChatOpenAI`. See `ChatOpenAI::with_json_mode` for details.
    #[must_use]
    pub fn with_json_mode(mut self) -> Self {
        self.response_format = Some(ResponseFormat::JsonObject);
        self
    }

    /// Enable structured output with a JSON schema
    ///
    /// Same interface as `ChatOpenAI`. See `ChatOpenAI::with_structured_output` for details.
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
}

#[allow(deprecated)]
impl Default for AzureChatOpenAI {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatModel for AzureChatOpenAI {
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Client> cloned for retry closure
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

        // Convert messages (same as ChatOpenAI)
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
        if let Some(n) = self.n {
            request_builder.n(n);
        }

        // Stop sequences: prefer parameter over struct field
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Tools: prefer parameter over struct field
        if let Some(tool_defs) = tools {
            let openai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(openai_tools);
        } else if let Some(ref tools) = self.tools {
            request_builder.tools(tools.clone());
        }

        // Tool choice: prefer parameter over struct field
        if let Some(tc) = tool_choice {
            request_builder.tool_choice(convert_tool_choice(tc));
        } else if let Some(ref tool_choice) = self.tool_choice {
            request_builder.tool_choice(tool_choice.clone());
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
                    .map_err(|e| Error::api(format!("Azure OpenAI API error: {e}")))
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

        // Convert response to ChatResult (same logic as ChatOpenAI)
        // Extract usage metadata from response (applies to all generations)
        let usage_metadata = response
            .usage
            .as_ref()
            .map(|usage| UsageMetadata::new(usage.prompt_tokens, usage.completion_tokens));

        let model_name = response.model.clone();
        let system_fingerprint = response.system_fingerprint.clone();

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

                // Build AIMessage with tool calls and usage metadata
                let message = if !tool_calls.is_empty() || !invalid_tool_calls.is_empty() {
                    Message::AI {
                        content: content.into(),
                        tool_calls,
                        invalid_tool_calls,
                        usage_metadata: usage_metadata.clone(),
                        fields: Default::default(),
                    }
                } else {
                    let mut ai_msg = AIMessage::new(content);
                    if let Some(usage) = usage_metadata.clone() {
                        ai_msg = ai_msg.with_usage(usage);
                    }
                    ai_msg.into()
                };

                // Build generation_info with response metadata
                let mut generation_info = HashMap::new();
                if let Some(finish_reason) = choice.finish_reason {
                    generation_info.insert(
                        "finish_reason".to_string(),
                        serde_json::json!(finish_reason),
                    );
                }
                generation_info.insert(
                    "model_name".to_string(),
                    serde_json::json!(model_name.clone()),
                );
                if let Some(fingerprint) = &system_fingerprint {
                    generation_info.insert(
                        "system_fingerprint".to_string(),
                        serde_json::json!(fingerprint),
                    );
                }
                if let Some(logprobs) = choice.logprobs {
                    generation_info.insert(
                        "logprobs".to_string(),
                        serde_json::to_value(logprobs).unwrap_or(serde_json::json!(null)),
                    );
                }

                ChatGeneration {
                    message,
                    generation_info: Some(generation_info),
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

        // Stop sequences: prefer parameter over struct field
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Tools: prefer parameter over struct field
        if let Some(tool_defs) = tools {
            let openai_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(openai_tools);
        } else if let Some(ref tools) = self.tools {
            request_builder.tools(tools.clone());
        }

        // Tool choice: prefer parameter over struct field
        if let Some(tc) = tool_choice {
            request_builder.tool_choice(convert_tool_choice(tc));
        } else if let Some(ref tool_choice) = self.tool_choice {
            request_builder.tool_choice(tool_choice.clone());
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
            .map_err(|e| Error::api(format!("Azure OpenAI streaming API error: {e}")))?;

        // Convert OpenAI stream to DashFlow stream
        let output_stream = stream! {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            let content = choice.delta.content.unwrap_or_default();
                            let tool_calls = choice.delta.tool_calls.as_ref().map(|calls| {
                                calls.iter().map(|tc| {
                                    // OpenAI streaming: args sent as incremental string chunks
                                    // E.g.: "{"", "input", "\":", "3", "}" sent separately
                                    // Core's merge() accumulates strings, to_message() parses complete JSON
                                    let args_str = tc.function.as_ref()
                                        .and_then(|f| f.arguments.as_deref())
                                        .unwrap_or("");
                                    // Store as String value (will be accumulated in core's merge logic)
                                    let args = serde_json::Value::String(args_str.to_string());

                                    ToolCall {
                                        name: tc.function.as_ref().map(|f| f.name.clone()).unwrap_or_default().unwrap_or_default(),
                                        args,
                                        id: tc.id.clone().unwrap_or_default(),
                                        tool_type: "tool_call".to_string(),
                                        index: Some(tc.index as usize),
                                    }
                                }).collect::<Vec<_>>()
                            }).unwrap_or_default();

                            let mut chunk = AIMessageChunk::new(content);
                            if !tool_calls.is_empty() {
                                chunk.tool_calls = tool_calls;
                            }

                            yield Ok(ChatGenerationChunk {
                                message: chunk,
                                generation_info: Default::default(),
                            });
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
        "azure-openai-chat"
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        params.insert("model".to_string(), serde_json::json!(self.model));
        if let Some(ref deployment) = self.deployment_name {
            params.insert("deployment".to_string(), serde_json::json!(deployment));
        }
        if let Some(ref api_version) = self.api_version {
            params.insert("api_version".to_string(), serde_json::json!(api_version));
        }
        if let Some(temp) = self.temperature {
            params.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = self.max_tokens {
            params.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }
        params
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod standard_tests;
