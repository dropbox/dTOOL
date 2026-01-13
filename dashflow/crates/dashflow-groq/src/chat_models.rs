// OpenAI-compatible client clippy exceptions for async boundaries:
// - clone_on_ref_ptr: Arc::clone() is idiomatic for sharing client across async tasks
// - needless_pass_by_value: async move closures require owned values
// - redundant_clone: Clone before async move prevents use-after-move
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Groq chat model implementation

use crate::GROQ_API_BASE;

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

/// Groq chat model configuration and client
///
/// Groq provides fast LLM inference with an OpenAI-compatible API.
///
/// # Example
/// ```no_run
/// use dashflow_groq::ChatGroq;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatGroq::new()
///         .with_model("llama-3.3-70b-versatile")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Available Models
/// - `llama-3.1-8b-instant` (default) - Fast, good for simple tasks
/// - `llama-3.3-70b-versatile` - More capable, versatile model
/// - `mixtral-8x7b-32768` - Large context window
/// - `deepseek-r1-distill-llama-70b` - `DeepSeek` reasoning model
#[derive(Clone, Debug)]
pub struct ChatGroq {
    /// Groq client (using OpenAI-compatible interface)
    client: Arc<Client<OpenAIConfig>>,

    /// Model name (e.g., "llama-3.3-70b-versatile", "mixtral-8x7b-32768")
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

impl ChatGroq {
    /// Create a new `ChatGroq` instance with default settings
    ///
    /// Uses `GROQ_API_KEY` environment variable for authentication
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_groq::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        let config = OpenAIConfig::new().with_api_base(GROQ_API_BASE);
        Self::with_config(config)
    }

    /// Create a new `ChatGroq` instance with custom configuration
    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            model: "llama-3.1-8b-instant".to_string(),
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
    /// use dashflow_groq::ChatGroq;
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
    /// let model = ChatGroq::new()
    ///     .with_model("llama-3.3-70b-versatile")
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
    /// use dashflow_groq::ChatGroq;
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
    /// let model = ChatGroq::new().with_tools(vec![tool]);
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
    /// use dashflow_groq::ChatGroq;
    ///
    /// // Force model to use tools
    /// let model = ChatGroq::new()
    ///     .with_tool_choice(Some("required".to_string()));
    ///
    /// // Force specific tool
    /// let model = ChatGroq::new()
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
    /// use dashflow_groq::ChatGroq;
    /// use dashflow::core::messages::Message;
    /// use dashflow::core::language_models::ChatModel;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ChatGroq::new()
    ///     .with_model("llama-3.3-70b-versatile")
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
    /// use dashflow_groq::ChatGroq;
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
    /// let model = ChatGroq::new()
    ///     .with_model("llama-3.3-70b-versatile")
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

    /// Create a `ChatGroq` instance from a configuration
    ///
    /// This method constructs a `ChatGroq` model from a `ChatModelConfig::Groq` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be Groq variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatGroq` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not a Groq variant
    /// - API key environment variable cannot be resolved
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::Groq {
                model,
                api_key,
                temperature,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                // Build the Groq config with custom API key
                let openai_config = OpenAIConfig::new()
                    .with_api_key(&resolved_api_key)
                    .with_api_base(GROQ_API_BASE);

                // Create the ChatGroq instance
                let mut chat_model = Self::with_config(openai_config).with_model(model);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(*temp);
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected Groq config, got {} config",
                config.provider()
            ))),
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatGroq {
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

/// Convert `DashFlow` `ToolDefinition` to `OpenAI` `ChatCompletionTool` (Groq uses `OpenAI` format)
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

/// Convert `DashFlow` `ToolChoice` to `OpenAI` `ChatCompletionToolChoiceOption` (Groq uses `OpenAI` format)
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
impl ChatModel for ChatGroq {
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
        // Groq requires n=1 (does not support multiple completions)
        request_builder.n(1);

        // Add stop sequences (parameter overrides struct field)
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Convert and add tools if provided (parameter overrides struct field)
        if let Some(tool_defs) = tools {
            let groq_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(groq_tools);

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
                    .map_err(|e| Error::api(format!("Groq API error: {e}")))
            }
        })
        .await;

        // Handle error callback
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(manager) = run_manager {
                    if let Err(cb_err) = manager.on_llm_error(&e.to_string(), run_id, None).await {
                        tracing::warn!(error = %cb_err, "Failed to send LLM error callback");
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
        // Groq requires n=1 (does not support multiple completions)
        request_builder.n(1);

        // Add stop sequences (parameter overrides struct field)
        if let Some(stop_seqs) = stop {
            request_builder.stop(stop_seqs.to_vec());
        }

        // Convert and add tools if provided (parameter overrides struct field)
        if let Some(tool_defs) = tools {
            let groq_tools: Vec<ChatCompletionTool> =
                tool_defs.iter().map(convert_tool_definition).collect();
            request_builder.tools(groq_tools);

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
            .map_err(|e| Error::api(format!("Groq streaming API error: {e}")))?;

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
        "groq-chat"
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
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)] // Tests use deprecated with_tools() method
mod tests {
    use super::*;

    #[test]
    fn test_chat_groq_builder() {
        let model = ChatGroq::new()
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

        let model = ChatGroq::new().with_tools(vec![tool]);

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

        let model = ChatGroq::new().with_tools(tools);

        assert!(model.tools.is_some());
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 2);
        assert_eq!(bound_tools[0].function.name, "get_weather");
        assert_eq!(bound_tools[1].function.name, "search_web");
    }

    #[test]
    fn test_with_tools_empty() {
        let model = ChatGroq::new().with_tools(vec![]);
        assert!(model.tools.is_none());
    }

    #[test]
    fn test_with_tools_invalid_schema() {
        // Tool without name should be filtered out
        let tools = vec![
            serde_json::json!({"description": "No name"}),
            serde_json::json!({"name": "valid_tool"}),
        ];

        let model = ChatGroq::new().with_tools(tools);

        assert!(model.tools.is_some());
        let bound_tools = model.tools.unwrap();
        assert_eq!(bound_tools.len(), 1);
        assert_eq!(bound_tools[0].function.name, "valid_tool");
    }

    #[test]
    fn test_with_tool_choice_none() {
        let model = ChatGroq::new().with_tool_choice(Some("none".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::None => {}
            _ => panic!("Expected None tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_auto() {
        let model = ChatGroq::new().with_tool_choice(Some("auto".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::Auto => {}
            _ => panic!("Expected Auto tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_required() {
        let model = ChatGroq::new().with_tool_choice(Some("required".to_string()));

        assert!(model.tool_choice.is_some());
        match model.tool_choice.unwrap() {
            ChatCompletionToolChoiceOption::Required => {}
            _ => panic!("Expected Required tool choice"),
        }
    }

    #[test]
    fn test_with_tool_choice_named() {
        let model = ChatGroq::new().with_tool_choice(Some("get_weather".to_string()));

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
        let model = ChatGroq::new().with_tool_choice(None);
        assert!(model.tool_choice.is_none());
    }

    #[test]
    fn test_tools_and_tool_choice_together() {
        let tool = serde_json::json!({
            "name": "calculate",
            "description": "Perform calculation"
        });

        let model = ChatGroq::new()
            .with_tools(vec![tool])
            .with_tool_choice(Some("required".to_string()));

        assert!(model.tools.is_some());
        assert!(model.tool_choice.is_some());
    }

    #[test]
    fn test_with_json_mode() {
        let model = ChatGroq::new().with_json_mode();

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

        let model = ChatGroq::new().with_structured_output(
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
            ChatGroq::new().with_structured_output("simple_output", schema.clone(), None, false);

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

        let model = ChatGroq::new().with_tools(vec![tool]).with_json_mode();

        assert!(model.tools.is_some());
        assert!(model.response_format.is_some());
    }

    #[test]
    fn test_response_format_unset_by_default() {
        let model = ChatGroq::new();
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

        let model = ChatGroq::new().with_structured_output(
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

        let model = ChatGroq::new()
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
}

// ============================================================================
// COMPREHENSIVE UNIT TESTS - No API key required
// ============================================================================

#[cfg(test)]
#[allow(
    deprecated,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)]
mod comprehensive_unit_tests {
    use super::*;

    // ========================================================================
    // API Base URL Tests
    // ========================================================================

    #[test]
    fn test_groq_api_base_url() {
        assert_eq!(GROQ_API_BASE, "https://api.groq.com/openai/v1");
    }

    #[test]
    fn test_groq_api_base_is_https() {
        assert!(GROQ_API_BASE.starts_with("https://"));
    }

    // ========================================================================
    // Default Model Tests
    // ========================================================================

    #[test]
    fn test_default_model_name() {
        let model = ChatGroq::new();
        assert_eq!(model.model, "llama-3.1-8b-instant");
    }

    #[test]
    fn test_default_temperature_is_none() {
        let model = ChatGroq::new();
        assert!(model.temperature.is_none());
    }

    #[test]
    fn test_default_max_tokens_is_none() {
        let model = ChatGroq::new();
        assert!(model.max_tokens.is_none());
    }

    #[test]
    fn test_default_top_p_is_none() {
        let model = ChatGroq::new();
        assert!(model.top_p.is_none());
    }

    #[test]
    fn test_default_frequency_penalty_is_none() {
        let model = ChatGroq::new();
        assert!(model.frequency_penalty.is_none());
    }

    #[test]
    fn test_default_presence_penalty_is_none() {
        let model = ChatGroq::new();
        assert!(model.presence_penalty.is_none());
    }

    #[test]
    fn test_default_tools_is_none() {
        let model = ChatGroq::new();
        assert!(model.tools.is_none());
    }

    #[test]
    fn test_default_tool_choice_is_none() {
        let model = ChatGroq::new();
        assert!(model.tool_choice.is_none());
    }

    #[test]
    fn test_default_response_format_is_none() {
        let model = ChatGroq::new();
        assert!(model.response_format.is_none());
    }

    #[test]
    fn test_default_rate_limiter_is_none() {
        let model = ChatGroq::new();
        assert!(model.rate_limiter.is_none());
    }

    #[test]
    fn test_default_trait_implementation() {
        let model: ChatGroq = Default::default();
        assert_eq!(model.model, "llama-3.1-8b-instant");
    }

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_with_model_various_models() {
        let models = [
            "llama-3.1-8b-instant",
            "llama-3.1-70b-versatile",
            "llama-3.3-70b-versatile",
            "mixtral-8x7b-32768",
            "gemma-7b-it",
            "deepseek-r1-distill-llama-70b",
        ];

        for model_name in models {
            let model = ChatGroq::new().with_model(model_name);
            assert_eq!(model.model, model_name);
        }
    }

    #[test]
    fn test_with_model_string_type() {
        let model = ChatGroq::new().with_model(String::from("custom-model"));
        assert_eq!(model.model, "custom-model");
    }

    #[test]
    fn test_with_temperature_zero() {
        let model = ChatGroq::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_with_temperature_one() {
        let model = ChatGroq::new().with_temperature(1.0);
        assert_eq!(model.temperature, Some(1.0));
    }

    #[test]
    fn test_with_temperature_two() {
        let model = ChatGroq::new().with_temperature(2.0);
        assert_eq!(model.temperature, Some(2.0));
    }

    #[test]
    fn test_with_temperature_fractional() {
        let model = ChatGroq::new().with_temperature(0.42);
        assert_eq!(model.temperature, Some(0.42));
    }

    #[test]
    fn test_with_max_tokens_small() {
        let model = ChatGroq::new().with_max_tokens(1);
        assert_eq!(model.max_tokens, Some(1));
    }

    #[test]
    fn test_with_max_tokens_large() {
        let model = ChatGroq::new().with_max_tokens(32768);
        assert_eq!(model.max_tokens, Some(32768));
    }

    #[test]
    fn test_with_max_tokens_typical() {
        let model = ChatGroq::new().with_max_tokens(1024);
        assert_eq!(model.max_tokens, Some(1024));
    }

    #[test]
    fn test_with_top_p_zero() {
        let model = ChatGroq::new().with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_with_top_p_one() {
        let model = ChatGroq::new().with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    #[test]
    fn test_with_top_p_typical() {
        let model = ChatGroq::new().with_top_p(0.95);
        assert_eq!(model.top_p, Some(0.95));
    }

    #[test]
    fn test_with_frequency_penalty_negative() {
        let model = ChatGroq::new().with_frequency_penalty(-2.0);
        assert_eq!(model.frequency_penalty, Some(-2.0));
    }

    #[test]
    fn test_with_frequency_penalty_zero() {
        let model = ChatGroq::new().with_frequency_penalty(0.0);
        assert_eq!(model.frequency_penalty, Some(0.0));
    }

    #[test]
    fn test_with_frequency_penalty_positive() {
        let model = ChatGroq::new().with_frequency_penalty(2.0);
        assert_eq!(model.frequency_penalty, Some(2.0));
    }

    #[test]
    fn test_with_presence_penalty_negative() {
        let model = ChatGroq::new().with_presence_penalty(-2.0);
        assert_eq!(model.presence_penalty, Some(-2.0));
    }

    #[test]
    fn test_with_presence_penalty_zero() {
        let model = ChatGroq::new().with_presence_penalty(0.0);
        assert_eq!(model.presence_penalty, Some(0.0));
    }

    #[test]
    fn test_with_presence_penalty_positive() {
        let model = ChatGroq::new().with_presence_penalty(2.0);
        assert_eq!(model.presence_penalty, Some(2.0));
    }

    #[test]
    fn test_with_retry_policy_custom() {
        let policy = RetryPolicy::fixed(5, 1000); // 5 retries, 1000ms delay
        let model = ChatGroq::new().with_retry_policy(policy);
        // Verify policy was set (we can't directly inspect it, but the builder should accept it)
        assert!(model.temperature.is_none()); // Just verify model is still valid
    }

    #[test]
    fn test_builder_chaining_all_parameters() {
        let model = ChatGroq::new()
            .with_model("llama-3.3-70b-versatile")
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .with_top_p(0.9)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        assert_eq!(model.model, "llama-3.3-70b-versatile");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(2048));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.frequency_penalty, Some(0.5));
        assert_eq!(model.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_builder_overwrite_previous_values() {
        let model = ChatGroq::new()
            .with_temperature(0.1)
            .with_temperature(0.5)
            .with_temperature(0.9);

        assert_eq!(model.temperature, Some(0.9));
    }

    #[test]
    fn test_builder_model_name_overwrite() {
        let model = ChatGroq::new()
            .with_model("model-a")
            .with_model("model-b")
            .with_model("model-c");

        assert_eq!(model.model, "model-c");
    }

    // ========================================================================
    // with_config Tests
    // ========================================================================

    #[test]
    fn test_with_config_creates_client() {
        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(GROQ_API_BASE);
        let model = ChatGroq::with_config(config);
        assert_eq!(model.model, "llama-3.1-8b-instant");
    }

    #[test]
    fn test_with_config_preserves_defaults() {
        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(GROQ_API_BASE);
        let model = ChatGroq::with_config(config);

        assert!(model.temperature.is_none());
        assert!(model.max_tokens.is_none());
        assert!(model.tools.is_none());
    }

    // ========================================================================
    // Clone Tests
    // ========================================================================

    #[test]
    fn test_clone_preserves_model() {
        let model = ChatGroq::new().with_model("test-model");
        let cloned = model.clone();
        assert_eq!(cloned.model, "test-model");
    }

    #[test]
    fn test_clone_preserves_temperature() {
        let model = ChatGroq::new().with_temperature(0.42);
        let cloned = model.clone();
        assert_eq!(cloned.temperature, Some(0.42));
    }

    #[test]
    fn test_clone_preserves_all_settings() {
        let model = ChatGroq::new()
            .with_model("llama-3.3-70b-versatile")
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .with_top_p(0.9)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3)
            .with_json_mode();

        let cloned = model.clone();

        assert_eq!(cloned.model, "llama-3.3-70b-versatile");
        assert_eq!(cloned.temperature, Some(0.7));
        assert_eq!(cloned.max_tokens, Some(2048));
        assert_eq!(cloned.top_p, Some(0.9));
        assert_eq!(cloned.frequency_penalty, Some(0.5));
        assert_eq!(cloned.presence_penalty, Some(0.3));
        assert!(cloned.response_format.is_some());
    }

    // ========================================================================
    // Tool Definition Conversion Tests
    // ========================================================================

    #[test]
    fn test_convert_tool_definition_basic() {
        let tool_def = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        };

        let converted = convert_tool_definition(&tool_def);
        assert_eq!(converted.function.name, "test_tool");
        assert_eq!(
            converted.function.description,
            Some("A test tool".to_string())
        );
    }

    #[test]
    fn test_convert_tool_definition_empty_description() {
        let tool_def = ToolDefinition {
            name: "no_desc_tool".to_string(),
            description: "".to_string(),
            parameters: serde_json::json!({}),
        };

        let converted = convert_tool_definition(&tool_def);
        assert_eq!(converted.function.name, "no_desc_tool");
        assert!(converted.function.description.is_none()); // Empty string becomes None
    }

    #[test]
    fn test_convert_tool_definition_complex_parameters() {
        let tool_def = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A tool with complex parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "integer"},
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["name"]
            }),
        };

        let converted = convert_tool_definition(&tool_def);
        assert!(converted.function.parameters.is_some());
        let params = converted.function.parameters.unwrap();
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
    }

    // ========================================================================
    // Tool Choice Conversion Tests
    // ========================================================================

    #[test]
    fn test_convert_tool_choice_auto() {
        let choice = ToolChoice::Auto;
        let converted = convert_tool_choice(&choice);
        assert!(matches!(converted, ChatCompletionToolChoiceOption::Auto));
    }

    #[test]
    fn test_convert_tool_choice_none() {
        let choice = ToolChoice::None;
        let converted = convert_tool_choice(&choice);
        assert!(matches!(converted, ChatCompletionToolChoiceOption::None));
    }

    #[test]
    fn test_convert_tool_choice_required() {
        let choice = ToolChoice::Required;
        let converted = convert_tool_choice(&choice);
        assert!(matches!(
            converted,
            ChatCompletionToolChoiceOption::Required
        ));
    }

    #[test]
    fn test_convert_tool_choice_specific() {
        let choice = ToolChoice::Specific("my_function".to_string());
        let converted = convert_tool_choice(&choice);
        match converted {
            ChatCompletionToolChoiceOption::Named(named) => {
                assert_eq!(named.function.name, "my_function");
            }
            _ => panic!("Expected Named tool choice"),
        }
    }

    // ========================================================================
    // Image Source Conversion Tests
    // ========================================================================

    #[test]
    fn test_convert_image_source_url() {
        use dashflow::core::messages::ImageSource;

        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let converted = convert_image_source(&source, None);
        assert_eq!(converted.url, "https://example.com/image.png");
        assert!(converted.detail.is_none());
    }

    #[test]
    fn test_convert_image_source_base64() {
        use dashflow::core::messages::ImageSource;

        let source = ImageSource::Base64 {
            media_type: "image/jpeg".to_string(),
            data: "ABC123==".to_string(),
        };
        let converted = convert_image_source(&source, None);
        assert_eq!(converted.url, "data:image/jpeg;base64,ABC123==");
    }

    #[test]
    fn test_convert_image_source_with_detail_low() {
        use dashflow::core::messages::{ImageDetail, ImageSource};

        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let converted = convert_image_source(&source, Some(ImageDetail::Low));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::Low));
    }

    #[test]
    fn test_convert_image_source_with_detail_high() {
        use dashflow::core::messages::{ImageDetail, ImageSource};

        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let converted = convert_image_source(&source, Some(ImageDetail::High));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::High));
    }

    #[test]
    fn test_convert_image_source_with_detail_auto() {
        use dashflow::core::messages::{ImageDetail, ImageSource};

        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let converted = convert_image_source(&source, Some(ImageDetail::Auto));
        assert_eq!(converted.detail, Some(OpenAIImageDetail::Auto));
    }

    // ========================================================================
    // Content Conversion Tests
    // ========================================================================

    #[test]
    fn test_convert_content_text() {
        use dashflow::core::messages::MessageContent;

        let content = MessageContent::Text("Hello, world!".to_string());
        let converted = convert_content(&content);
        match converted {
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "Hello, world!");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_convert_content_empty_text() {
        use dashflow::core::messages::MessageContent;

        let content = MessageContent::Text("".to_string());
        let converted = convert_content(&content);
        match converted {
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_convert_content_filters_empty_text_blocks() {
        use dashflow::core::messages::{ContentBlock, MessageContent};

        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "".to_string(),
            },
            ContentBlock::Text {
                text: "Real content".to_string(),
            },
        ]);

        let converted = convert_content(&content);
        // Single text block should be optimized to text format
        match converted {
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "Real content");
            }
            _ => panic!("Expected text content after filtering"),
        }
    }

    #[test]
    fn test_convert_content_empty_blocks() {
        use dashflow::core::messages::{ContentBlock, MessageContent};

        // All empty text blocks should result in empty array
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "".to_string(),
            },
            ContentBlock::Text {
                text: "".to_string(),
            },
        ]);

        let converted = convert_content(&content);
        match converted {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert!(parts.is_empty());
            }
            _ => panic!("Expected array content"),
        }
    }

    // ========================================================================
    // Function Message Conversion Tests
    // ========================================================================

    #[test]
    fn test_message_conversion_function() {
        let msg = Message::Function {
            content: "Function result here".into(),
            name: "my_function".to_string(),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                // Function messages are converted to tool messages
                assert_eq!(tool_msg.tool_call_id, "func_my_function");
            }
            _ => panic!("Expected tool message (function converted to tool)"),
        }
    }

    #[test]
    fn test_message_conversion_function_special_name() {
        let msg = Message::Function {
            content: "Result".into(),
            name: "get_weather_v2".to_string(),
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                assert_eq!(tool_msg.tool_call_id, "func_get_weather_v2");
            }
            _ => panic!("Expected tool message"),
        }
    }

    // ========================================================================
    // Identifying Params Tests
    // ========================================================================

    #[test]
    fn test_identifying_params_model_only() {
        let model = ChatGroq::new();
        let params = model.identifying_params();

        assert_eq!(
            params.get("model").unwrap(),
            &serde_json::json!("llama-3.1-8b-instant")
        );
        assert!(params.get("temperature").is_none());
        assert!(params.get("max_tokens").is_none());
    }

    #[test]
    fn test_identifying_params_with_temperature() {
        // Use 0.5 which is exactly representable in f32
        let model = ChatGroq::new().with_temperature(0.5);
        let params = model.identifying_params();

        assert_eq!(params.get("temperature").unwrap(), &serde_json::json!(0.5));
    }

    #[test]
    fn test_identifying_params_with_max_tokens() {
        let model = ChatGroq::new().with_max_tokens(512);
        let params = model.identifying_params();

        assert_eq!(params.get("max_tokens").unwrap(), &serde_json::json!(512));
    }

    #[test]
    fn test_identifying_params_all_set() {
        let model = ChatGroq::new()
            .with_model("custom-model")
            .with_temperature(0.5)
            .with_max_tokens(1024);
        let params = model.identifying_params();

        assert_eq!(
            params.get("model").unwrap(),
            &serde_json::json!("custom-model")
        );
        assert_eq!(params.get("temperature").unwrap(), &serde_json::json!(0.5));
        assert_eq!(params.get("max_tokens").unwrap(), &serde_json::json!(1024));
    }

    // ========================================================================
    // LLM Type Tests
    // ========================================================================

    #[test]
    fn test_llm_type() {
        let model = ChatGroq::new();
        assert_eq!(model.llm_type(), "groq-chat");
    }

    #[test]
    fn test_llm_type_custom_model() {
        let model = ChatGroq::new().with_model("mixtral-8x7b-32768");
        assert_eq!(model.llm_type(), "groq-chat"); // Type is always groq-chat
    }

    // ========================================================================
    // as_any Tests
    // ========================================================================

    #[test]
    fn test_as_any_downcast() {
        let model = ChatGroq::new().with_model("test-model");
        let any_ref = model.as_any();
        let downcasted: Option<&ChatGroq> = any_ref.downcast_ref();
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().model, "test-model");
    }

    // ========================================================================
    // Structured Output Tests (Additional)
    // ========================================================================

    #[test]
    fn test_structured_output_empty_schema() {
        let schema = serde_json::json!({});
        let model = ChatGroq::new().with_structured_output("empty", schema, None, false);

        assert!(model.response_format.is_some());
        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "empty");
                assert_eq!(json_schema.schema, Some(serde_json::json!({})));
            }
            _ => panic!("Expected JsonSchema"),
        }
    }

    #[test]
    fn test_structured_output_non_strict() {
        let schema = serde_json::json!({"type": "object"});
        let model = ChatGroq::new().with_structured_output("test", schema, None, false);

        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.strict, Some(false));
            }
            _ => panic!("Expected JsonSchema"),
        }
    }

    #[test]
    fn test_structured_output_strict() {
        let schema = serde_json::json!({"type": "object"});
        let model = ChatGroq::new().with_structured_output("test", schema, None, true);

        match model.response_format.unwrap() {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.strict, Some(true));
            }
            _ => panic!("Expected JsonSchema"),
        }
    }

    // ========================================================================
    // Debug Trait Tests
    // ========================================================================

    #[test]
    fn test_debug_format() {
        let model = ChatGroq::new().with_model("test-model").with_temperature(0.5);
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("ChatGroq"));
        assert!(debug_str.contains("test-model"));
        assert!(debug_str.contains("0.5"));
    }

    // ========================================================================
    // AI Message with Multiple Tool Calls
    // ========================================================================

    #[test]
    fn test_message_conversion_ai_multiple_tool_calls() {
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"city": "NYC"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "get_time".to_string(),
                args: serde_json::json!({"timezone": "EST"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
        ];

        let msg = Message::AI {
            content: "Let me check that for you.".into(),
            tool_calls,
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                let tool_calls = assistant_msg.tool_calls.unwrap();
                assert_eq!(tool_calls.len(), 2);
                assert_eq!(tool_calls[0].function.name, "get_weather");
                assert_eq!(tool_calls[1].function.name, "get_time");
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_message_conversion_ai_empty_tool_calls() {
        let msg = Message::AI {
            content: "Just a text response.".into(),
            tool_calls: vec![],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                // Empty tool_calls should result in None (not empty vec)
                assert!(
                    assistant_msg.tool_calls.is_none()
                        || assistant_msg.tool_calls.as_ref().map_or(true, Vec::is_empty)
                );
            }
            _ => panic!("Expected assistant message"),
        }
    }

    // ========================================================================
    // Tool Message with Various Content
    // ========================================================================

    #[test]
    fn test_message_conversion_tool_json_content() {
        let msg = Message::Tool {
            content: r#"{"temperature": 72, "unit": "F"}"#.into(),
            tool_call_id: "call_weather".to_string(),
            artifact: None,
            status: None,
            fields: Default::default(),
        };

        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                assert_eq!(tool_msg.tool_call_id, "call_weather");
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_message_conversion_tool_empty_content() {
        let msg = Message::Tool {
            content: "".into(),
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

    // ========================================================================
    // System Message Tests
    // ========================================================================

    #[test]
    fn test_message_conversion_system_long_content() {
        let long_content = "A".repeat(10000);
        let msg = Message::system(long_content.as_str());
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    #[test]
    fn test_message_conversion_system_unicode() {
        let msg = Message::system("你好世界 🌍 مرحبا");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    #[test]
    fn test_message_conversion_system_empty() {
        let msg = Message::system("");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected system message"),
        }
    }

    // ========================================================================
    // Human Message Tests (Additional)
    // ========================================================================

    #[test]
    fn test_message_conversion_human_unicode() {
        let msg = Message::human("Привет мир! 🚀 日本語テスト");
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(_) => {}
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_message_conversion_human_very_long() {
        let long_content = "test ".repeat(5000);
        let msg = Message::human(long_content.as_str());
        let converted = convert_message(&msg).unwrap();

        match converted {
            ChatCompletionRequestMessage::User(_) => {}
            _ => panic!("Expected user message"),
        }
    }

    // ========================================================================
    // Rate Limiter Configuration Tests
    // ========================================================================

    #[test]
    fn test_rate_limiter_method_returns_none_by_default() {
        let model = ChatGroq::new();
        assert!(model.rate_limiter().is_none());
    }

    #[test]
    fn test_rate_limiter_method_returns_some_when_set() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let model = ChatGroq::new().with_rate_limiter(limiter);
        assert!(model.rate_limiter().is_some());
    }
}

/// Standard conformance tests
///
/// These tests verify that ChatGroq behaves consistently with other
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
    use dashflow_test_utils::init_test_env;

    /// Helper function to create a test model with standard settings
    ///
    /// Uses llama3-8b-8192 for testing
    fn create_test_model() -> ChatGroq {
        ChatGroq::new()
            .with_model("llama3-8b-8192")
            .with_temperature(0.0) // Deterministic for testing
            .with_max_tokens(100) // Limit tokens for cost/speed
    }

    /// Standard Test 1: Basic invoke
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_invoke_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_invoke(&model).await;
    }

    /// Standard Test 2: Streaming
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_stream_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream(&model).await;
    }

    /// Standard Test 3: Batch processing
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_batch_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_batch(&model).await;
    }

    /// Standard Test 4: Multi-turn conversation
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_conversation(&model).await;
    }

    /// Standard Test 4b: Double messages conversation
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_double_messages_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_double_messages_conversation(&model).await;
    }

    /// Standard Test 4c: Message with name field
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_message_with_name_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_message_with_name(&model).await;
    }

    /// Standard Test 5: Stop sequences
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_stop_sequence_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_stop_sequence(&model).await;
    }

    /// Standard Test 6: Usage metadata
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_usage_metadata_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_usage_metadata(&model).await;
    }

    /// Standard Test 7: Empty messages
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_empty_messages_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_messages(&model).await;
    }

    /// Standard Test 8: Long conversation
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_long_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_long_conversation(&model).await;
    }

    /// Standard Test 9: Special characters
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_special_characters_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_special_characters(&model).await;
    }

    /// Standard Test 10: Unicode and emoji
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_unicode_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_unicode(&model).await;
    }

    /// Standard Test 11: Tool calling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_tool_calling_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_tool_calling(&model).await;
    }

    /// Standard Test 12: Structured output
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_structured_output_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_structured_output(&model).await;
    }

    /// Standard Test 13: JSON mode
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_json_mode_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_json_mode(&model).await;
    }

    /// Standard Test 14: Usage metadata in streaming
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_usage_metadata_streaming_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_usage_metadata_streaming(&model).await;
    }

    /// Standard Test 15: System message handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_system_message_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_system_message(&model).await;
    }

    /// Standard Test 16: Empty content handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_empty_content_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_content(&model).await;
    }

    /// Standard Test 17: Large input handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_large_input_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_large_input(&model).await;
    }

    /// Standard Test 18: Concurrent generation
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_concurrent_generation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_concurrent_generation(&model).await;
    }

    /// Standard Test 19: Error recovery
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_error_recovery_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_error_recovery(&model).await;
    }

    /// Standard Test 20: Response consistency
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_response_consistency_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_response_consistency(&model).await;
    }

    /// Standard Test 21: Tool calling with no arguments
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_tool_calling_with_no_arguments_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_tool_calling_with_no_arguments(&model).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS - Advanced Edge Cases
    // ========================================================================

    /// Comprehensive Test 1: Streaming with timeout
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_stream_with_timeout_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_with_timeout(&model).await;
    }

    /// Comprehensive Test 2: Streaming interruption handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_stream_interruption_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_interruption(&model).await;
    }

    /// Comprehensive Test 3: Empty stream handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_stream_empty_response_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_empty_response(&model).await;
    }

    /// Comprehensive Test 4: Multiple system messages
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_multiple_system_messages_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_multiple_system_messages(&model).await;
    }

    /// Comprehensive Test 5: Empty system message
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_empty_system_message_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_system_message(&model).await;
    }

    /// Comprehensive Test 6: Temperature edge cases
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_temperature_extremes_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_temperature_extremes(&model).await;
    }

    /// Comprehensive Test 7: Max tokens enforcement
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_max_tokens_limit_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_max_tokens_limit(&model).await;
    }

    /// Comprehensive Test 8: Invalid stop sequences
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_invalid_stop_sequences_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_invalid_stop_sequences(&model).await;
    }

    /// Comprehensive Test 9: Context window overflow
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_context_window_overflow_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_context_window_overflow(&model).await;
    }

    /// Comprehensive Test 10: Rapid consecutive calls
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_rapid_consecutive_calls_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_rapid_consecutive_calls(&model).await;
    }

    /// Comprehensive Test 11: Network error handling
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_network_error_handling_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_network_error_handling(&model).await;
    }

    /// Comprehensive Test 12: Malformed input recovery
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_malformed_input_recovery_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_malformed_input_recovery(&model).await;
    }

    /// Comprehensive Test 13: Very long single message
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_very_long_single_message_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_very_long_single_message(&model).await;
    }

    /// Comprehensive Test 14: Response format consistency
    #[tokio::test]
    #[ignore = "requires GROQ_API_KEY"]
    async fn test_response_format_consistency_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_response_format_consistency(&model).await;
    }
}
