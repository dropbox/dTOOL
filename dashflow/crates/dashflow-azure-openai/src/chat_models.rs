//! Azure `OpenAI` chat model implementation

use async_openai::{
    config::AzureConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent, ChatCompletionTool,
        ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs, FunctionCall,
        FunctionObject, ResponseFormat, ResponseFormatJsonSchema,
    },
    Client,
};
use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{
        env_string, env_string_or_default, AZURE_OPENAI_API_KEY, AZURE_OPENAI_API_VERSION,
        AZURE_OPENAI_DEPLOYMENT_NAME, AZURE_OPENAI_ENDPOINT,
    },
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, Message, ToolCall},
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

/// Azure `OpenAI` chat model configuration and client
///
/// This provides Azure-specific endpoints, authentication, and deployment-based model selection.
///
/// # Example
/// ```no_run
/// use dashflow_azure_openai::ChatAzureOpenAI;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatAzureOpenAI::new()
///         .with_deployment_name("gpt-4")
///         .with_endpoint("https://my-resource.openai.azure.com")
///         .with_api_key("your-api-key")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
#[derive(Clone)]
pub struct ChatAzureOpenAI {
    /// Azure `OpenAI` client
    client: Arc<Client<AzureConfig>>,

    /// Deployment name (Azure-specific, maps to model)
    deployment_name: String,

    /// Azure endpoint URL (stored for builder accumulation)
    azure_endpoint: Option<String>,

    /// Azure API key (stored for builder accumulation)
    azure_api_key: Option<String>,

    /// Azure API version (stored for builder accumulation)
    azure_api_version: Option<String>,

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

// Custom Debug implementation to prevent API key exposure in logs
impl std::fmt::Debug for ChatAzureOpenAI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatAzureOpenAI")
            .field("client", &"Arc<Client<AzureConfig>>")
            .field("deployment_name", &self.deployment_name)
            .field("azure_endpoint", &self.azure_endpoint)
            .field("azure_api_key", &self.azure_api_key.as_ref().map(|_| "[REDACTED]"))
            .field("azure_api_version", &self.azure_api_version)
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("top_p", &self.top_p)
            .field("frequency_penalty", &self.frequency_penalty)
            .field("presence_penalty", &self.presence_penalty)
            .field("n", &self.n)
            .field("tools", &self.tools)
            .field("tool_choice", &self.tool_choice)
            .field("response_format", &self.response_format)
            .field("retry_policy", &self.retry_policy)
            .field("rate_limiter", &self.rate_limiter.as_ref().map(|_| "RateLimiter"))
            .finish()
    }
}

impl ChatAzureOpenAI {
    /// Create a new `ChatAzureOpenAI` instance with environment-based configuration
    ///
    /// Requires the following environment variables:
    /// - `AZURE_OPENAI_API_KEY`
    /// - `AZURE_OPENAI_ENDPOINT` (e.g., <https://my-resource.openai.azure.com>)
    /// - `AZURE_OPENAI_DEPLOYMENT_NAME` (optional, defaults to "gpt-35-turbo")
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(AzureConfig::default())
    }

    /// Create a new `ChatAzureOpenAI` instance with custom Azure configuration
    #[must_use]
    pub fn with_config(config: AzureConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            deployment_name: env_string_or_default(AZURE_OPENAI_DEPLOYMENT_NAME, "gpt-35-turbo"),
            azure_endpoint: env_string(AZURE_OPENAI_ENDPOINT),
            azure_api_key: env_string(AZURE_OPENAI_API_KEY),
            azure_api_version: env_string(AZURE_OPENAI_API_VERSION),
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

    /// Rebuilds the client from accumulated configuration values.
    /// Called internally by builder methods to ensure all settings are preserved.
    fn rebuild_client(&mut self) {
        let mut config = AzureConfig::new().with_deployment_id(&self.deployment_name);

        if let Some(endpoint) = &self.azure_endpoint {
            config = config.with_api_base(endpoint.clone());
        }
        if let Some(api_key) = &self.azure_api_key {
            config = config.with_api_key(api_key.clone());
        }
        if let Some(api_version) = &self.azure_api_version {
            config = config.with_api_version(api_version.clone());
        } else {
            // Default API version if not explicitly set
            config = config.with_api_version("2024-10-21");
        }

        self.client = Arc::new(Client::with_config(config));
    }

    /// Set the deployment name (Azure-specific model identifier)
    #[must_use]
    pub fn with_deployment_name(mut self, deployment_name: impl Into<String>) -> Self {
        self.deployment_name = deployment_name.into();
        self.rebuild_client();
        self
    }

    /// Set the Azure endpoint URL
    ///
    /// This method accumulates with other builder methods - you can chain
    /// `.with_endpoint()`, `.with_api_key()`, `.with_api_version()` in any order
    /// and all values will be preserved.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_azure_openai::ChatAzureOpenAI;
    ///
    /// let model = ChatAzureOpenAI::new()
    ///     .with_endpoint("https://my-resource.openai.azure.com")
    ///     .with_api_key("my-key")
    ///     .with_api_version("2024-10-21");
    /// // All three values are preserved in the final model
    /// ```
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.azure_endpoint = Some(endpoint.into());
        self.rebuild_client();
        self
    }

    /// Set the API key for Azure `OpenAI`
    ///
    /// This method accumulates with other builder methods - you can chain
    /// `.with_endpoint()`, `.with_api_key()`, `.with_api_version()` in any order
    /// and all values will be preserved.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.azure_api_key = Some(api_key.into());
        self.rebuild_client();
        self
    }

    /// Set the API version (defaults to "2024-10-21")
    ///
    /// This method accumulates with other builder methods - you can chain
    /// `.with_endpoint()`, `.with_api_key()`, `.with_api_version()` in any order
    /// and all values will be preserved.
    #[must_use]
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.azure_api_version = Some(version.into());
        self.rebuild_client();
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
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Bind tools to the model for function calling
    ///
    /// # Arguments
    /// * `tools` - Vector of tool definitions
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_azure_openai::ChatAzureOpenAI;
    /// use dashflow::core::language_models::ToolDefinition;
    /// use serde_json::json;
    ///
    /// let tools = vec![
    ///     ToolDefinition {
    ///         name: "get_weather".to_string(),
    ///         description: "Get weather for a location".to_string(),
    ///         parameters: json!({
    ///             "type": "object",
    ///             "properties": {
    ///                 "location": {"type": "string"}
    ///             }
    ///         }),
    ///     }
    /// ];
    ///
    /// # #[allow(deprecated)]
    /// # fn build_model(tools: Vec<ToolDefinition>) {
    /// let model = ChatAzureOpenAI::new().with_tools(tools);
    /// # let _ = model;
    /// # }
    /// ```
    #[must_use]
    #[deprecated(
        since = "1.9.0",
        note = "Use bind_tools() from ChatModelToolBindingExt trait instead. \
                bind_tools() is type-safe and works consistently across all providers. \
                Example: `use dashflow::core::language_models::ChatModelToolBindingExt; \
                model.bind_tools(vec![Arc::new(tool)], None)`"
    )]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        let openai_tools = tools
            .into_iter()
            .map(|tool| ChatCompletionTool {
                r#type: async_openai::types::ChatCompletionToolType::Function,
                function: FunctionObject {
                    name: tool.name,
                    description: if tool.description.is_empty() {
                        None
                    } else {
                        Some(tool.description)
                    },
                    parameters: Some(tool.parameters),
                    strict: None,
                },
            })
            .collect();
        self.tools = Some(openai_tools);
        self
    }

    /// Set the tool choice option
    ///
    /// Controls which (if any) tool is called by the model.
    #[must_use]
    pub fn with_tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        use async_openai::types::{
            ChatCompletionNamedToolChoice, ChatCompletionToolChoiceOption, FunctionName,
        };

        let openai_choice = match tool_choice {
            ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
            ToolChoice::None => ChatCompletionToolChoiceOption::None,
            ToolChoice::Required => ChatCompletionToolChoiceOption::Required,
            ToolChoice::Specific(name) => {
                ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
                    r#type: async_openai::types::ChatCompletionToolType::Function,
                    function: FunctionName { name },
                })
            }
        };
        self.tool_choice = Some(openai_choice);
        self
    }

    /// Enable JSON mode for structured outputs
    ///
    /// When enabled, the model's output will be valid JSON.
    #[must_use]
    pub fn with_json_mode(mut self) -> Self {
        self.response_format = Some(ResponseFormat::JsonObject);
        self
    }

    /// Set structured output with a JSON schema
    ///
    /// # Arguments
    /// * `name` - Name for the schema
    /// * `schema` - JSON schema for the output
    /// * `strict` - Whether to enforce strict schema validation
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_azure_openai::ChatAzureOpenAI;
    /// use serde_json::json;
    ///
    /// let schema = json!({
    ///     "type": "object",
    ///     "properties": {
    ///         "name": {"type": "string"},
    ///         "age": {"type": "integer"}
    ///     },
    ///     "required": ["name", "age"]
    /// });
    ///
    /// let model = ChatAzureOpenAI::new()
    ///     .with_structured_output("Person", schema, true);
    /// ```
    pub fn with_structured_output(
        mut self,
        name: impl Into<String>,
        schema: serde_json::Value,
        strict: bool,
    ) -> Self {
        self.response_format = Some(ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: name.into(),
                description: None,
                schema: Some(schema),
                strict: Some(strict),
            },
        });
        self
    }

    /// Convert `DashFlow` messages to Azure `OpenAI` format
    fn convert_messages(messages: &[Message]) -> Result<Vec<ChatCompletionRequestMessage>> {
        messages
            .iter()
            .map(|msg| match msg {
                Message::System { content, .. } => {
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(content.as_text())
                        .build()
                        .map(ChatCompletionRequestMessage::System)
                        .map_err(|e| Error::Other(format!("Failed to build system message: {e}")))
                }
                Message::Human { content, .. } => ChatCompletionRequestUserMessageArgs::default()
                    .content(ChatCompletionRequestUserMessageContent::Text(
                        content.as_text(),
                    ))
                    .build()
                    .map(ChatCompletionRequestMessage::User)
                    .map_err(|e| Error::Other(format!("Failed to build user message: {e}"))),
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
                    builder.content(content.as_text());

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

                    builder
                        .build()
                        .map(ChatCompletionRequestMessage::Assistant)
                        .map_err(|e| {
                            Error::Other(format!("Failed to build assistant message: {e}"))
                        })
                }
                Message::Tool {
                    content,
                    tool_call_id,
                    ..
                } => ChatCompletionRequestToolMessageArgs::default()
                    .content(content.as_text())
                    .tool_call_id(tool_call_id.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::Tool)
                    .map_err(|e| Error::Other(format!("Failed to build tool message: {e}"))),
                Message::Function { content, name, .. } => {
                    // OpenAI deprecated function messages in favor of tool messages
                    // Convert to tool message with a generated tool_call_id
                    let tool_call_id = format!("func_{name}");
                    ChatCompletionRequestToolMessageArgs::default()
                        .content(content.as_text())
                        .tool_call_id(tool_call_id)
                        .build()
                        .map(ChatCompletionRequestMessage::Tool)
                        .map_err(|e| {
                            Error::Other(format!("Failed to build tool message from function: {e}"))
                        })
                }
            })
            .collect()
    }

    /// Generate chat completions
    ///
    /// # Arguments
    /// * `messages` - The messages to send
    /// * `stop` - Per-call stop sequences (overrides instance-level)
    /// * `tools` - Per-call tool definitions (overrides instance-level)
    /// * `tool_choice` - Per-call tool choice (overrides instance-level)
    /// * `_callbacks` - Optional callback manager
    /// * `_tags` - Optional tags
    /// * `_metadata` - Optional metadata
    #[allow(clippy::too_many_arguments)] // Core chat generation API requires separate optional params for tools, callbacks, tags, metadata
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Client> cloned for retry closure
    async fn generate_impl(
        &self,
        messages: &[Message],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _callbacks: Option<Arc<CallbackManager>>,
        _tags: Option<Arc<[String]>>,
        _metadata: Option<Arc<HashMap<String, serde_json::Value>>>,
    ) -> Result<ChatResult> {
        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            let () = limiter.acquire().await;
        }

        // Convert messages to Azure OpenAI format
        let openai_messages = Self::convert_messages(messages)?;

        // Build request
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(&self.deployment_name)
            .messages(openai_messages);

        if let Some(temp) = self.temperature {
            request_builder.temperature(temp);
        }
        if let Some(max_tokens) = self.max_tokens {
            request_builder.max_tokens(max_tokens);
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

        // Per-call stop sequences override instance-level (note: async_openai uses `stop` field)
        if let Some(stop_seqs) = stop {
            if !stop_seqs.is_empty() {
                request_builder.stop(stop_seqs.to_vec());
            }
        }

        // Per-call tools override instance-level tools
        if let Some(per_call_tools) = tools {
            let openai_tools: Vec<ChatCompletionTool> = per_call_tools
                .iter()
                .map(|tool| ChatCompletionTool {
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
                })
                .collect();
            request_builder.tools(openai_tools);
        } else if let Some(instance_tools) = &self.tools {
            request_builder.tools(instance_tools.clone());
        }

        // Per-call tool_choice overrides instance-level tool_choice
        if let Some(per_call_choice) = tool_choice {
            use async_openai::types::{
                ChatCompletionNamedToolChoice, ChatCompletionToolChoiceOption, FunctionName,
            };

            let openai_choice = match per_call_choice {
                ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
                ToolChoice::None => ChatCompletionToolChoiceOption::None,
                ToolChoice::Required => ChatCompletionToolChoiceOption::Required,
                ToolChoice::Specific(name) => {
                    ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
                        r#type: async_openai::types::ChatCompletionToolType::Function,
                        function: FunctionName { name: name.clone() },
                    })
                }
            };
            request_builder.tool_choice(openai_choice);
        } else if let Some(instance_choice) = &self.tool_choice {
            request_builder.tool_choice(instance_choice.clone());
        }

        if let Some(response_format) = &self.response_format {
            request_builder.response_format(response_format.clone());
        }

        let request = request_builder
            .build()
            .map_err(|e| Error::Other(format!("Failed to build chat request: {e}")))?;

        // Call API with retry logic
        let client = self.client.clone();
        let response = with_retry(&self.retry_policy, move || {
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
        .await?;

        // Extract usage metadata
        let usage_metadata = response
            .usage
            .as_ref()
            .map(|usage| UsageMetadata::new(usage.prompt_tokens, usage.completion_tokens));

        // Convert response to ChatResult
        let generations: Vec<ChatGeneration> = response
            .choices
            .into_iter()
            .map(|choice| {
                let message = &choice.message;
                let mut ai_msg = AIMessage::new(message.content.clone().unwrap_or_default());

                // Extract tool calls if present
                if let Some(tool_calls) = &message.tool_calls {
                    let dashflow_tool_calls: Vec<ToolCall> = tool_calls
                        .iter()
                        .map(|tc| ToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            args: serde_json::from_str(&tc.function.arguments)
                                .unwrap_or_else(|_| serde_json::json!({})),
                            tool_type: "tool_call".to_string(),
                            index: None,
                        })
                        .collect();
                    ai_msg = ai_msg.with_tool_calls(dashflow_tool_calls);
                }

                // Add usage metadata if available
                if let Some(ref usage_meta) = usage_metadata {
                    ai_msg = ai_msg.with_usage(usage_meta.clone());
                }

                ChatGeneration {
                    message: ai_msg.into(),
                    generation_info: None,
                }
            })
            .collect();

        Ok(ChatResult {
            generations,
            llm_output: None,
        })
    }
}

impl Default for ChatAzureOpenAI {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatModel for ChatAzureOpenAI {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        self.generate_impl(messages, stop, tools, tool_choice, None, None, None)
            .await
    }

    #[allow(clippy::clone_on_ref_ptr)] // Arc<Client> cloned for stream! macro
    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            let () = limiter.acquire().await;
        }

        // Convert messages to Azure OpenAI format
        let openai_messages = Self::convert_messages(messages)?;

        // Build request
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(&self.deployment_name)
            .messages(openai_messages);

        if let Some(temp) = self.temperature {
            request_builder.temperature(temp);
        }
        if let Some(max_tokens) = self.max_tokens {
            request_builder.max_tokens(max_tokens);
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

        // Per-call stop sequences
        if let Some(stop_seqs) = stop {
            if !stop_seqs.is_empty() {
                request_builder.stop(stop_seqs.to_vec());
            }
        }

        // Per-call tools override instance-level tools
        if let Some(per_call_tools) = tools {
            let openai_tools: Vec<ChatCompletionTool> = per_call_tools
                .iter()
                .map(|tool| ChatCompletionTool {
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
                })
                .collect();
            request_builder.tools(openai_tools);
        } else if let Some(instance_tools) = &self.tools {
            request_builder.tools(instance_tools.clone());
        }

        // Per-call tool_choice overrides instance-level tool_choice
        if let Some(per_call_choice) = tool_choice {
            use async_openai::types::{
                ChatCompletionNamedToolChoice, ChatCompletionToolChoiceOption, FunctionName,
            };

            let openai_choice = match per_call_choice {
                ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
                ToolChoice::None => ChatCompletionToolChoiceOption::None,
                ToolChoice::Required => ChatCompletionToolChoiceOption::Required,
                ToolChoice::Specific(name) => {
                    ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
                        r#type: async_openai::types::ChatCompletionToolType::Function,
                        function: FunctionName { name: name.clone() },
                    })
                }
            };
            request_builder.tool_choice(openai_choice);
        } else if let Some(instance_choice) = &self.tool_choice {
            request_builder.tool_choice(instance_choice.clone());
        }

        if let Some(response_format) = &self.response_format {
            request_builder.response_format(response_format.clone());
        }

        let request = request_builder
            .build()
            .map_err(|e| Error::Other(format!("Failed to build chat request: {e}")))?;

        let client = self.client.clone();

        let stream = stream! {
            let mut stream = match client.chat().create_stream(request).await {
                Ok(s) => s,
                Err(e) => {
                    yield Err(Error::api(format!("Azure OpenAI API error: {e}")));
                    return;
                }
            };

            // Counter for generating unique tool call IDs when streaming returns None
            let mut tool_call_counter: u32 = 0;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            let delta = &choice.delta;

                            let mut chunk = AIMessageChunk::new(
                                delta.content.clone().unwrap_or_default()
                            );

                            // Extract tool calls if present
                            if let Some(tool_calls) = &delta.tool_calls {
                                let dashflow_tool_calls: Vec<ToolCall> = tool_calls
                                    .iter()
                                    .map(|tc| {
                                        // Generate a unique ID if streaming doesn't provide one
                                        // Empty string IDs can break chunk merging
                                        let id = tc.id.clone().unwrap_or_else(|| {
                                            tool_call_counter += 1;
                                            format!("call_stream_{tool_call_counter}")
                                        });
                                        ToolCall {
                                            id,
                                            name: tc.function.as_ref()
                                                .and_then(|f| f.name.clone())
                                                .unwrap_or_default(),
                                            args: tc.function.as_ref()
                                                .and_then(|f| f.arguments.as_ref())
                                                .and_then(|a| serde_json::from_str(a).ok())
                                                .unwrap_or_else(|| serde_json::json!({})),
                                            tool_type: "tool_call".to_string(),
                                            index: Some(tc.index as usize),
                                        }
                                    })
                                    .collect();
                                chunk.tool_calls = dashflow_tool_calls;
                            }

                            yield Ok(ChatGenerationChunk {
                                message: chunk,
                                generation_info: None,
                            });
                        }
                    }
                    Err(e) => {
                        yield Err(Error::api(format!("Azure OpenAI stream error: {e}")));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn llm_type(&self) -> &'static str {
        "azure-openai-chat"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        params.insert(
            "deployment_name".to_string(),
            serde_json::json!(self.deployment_name),
        );
        if let Some(temp) = self.temperature {
            params.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = self.max_tokens {
            params.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }
        params
    }
}

impl Serializable for ChatAzureOpenAI {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "azure_openai".to_string(),
            "ChatAzureOpenAI".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Deployment name (required)
        kwargs.insert(
            "deployment_name".to_string(),
            serde_json::json!(self.deployment_name),
        );

        // Optional parameters (only include if set)
        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }
        if let Some(top_p) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(freq_penalty) = self.frequency_penalty {
            kwargs.insert(
                "frequency_penalty".to_string(),
                serde_json::json!(freq_penalty),
            );
        }
        if let Some(pres_penalty) = self.presence_penalty {
            kwargs.insert(
                "presence_penalty".to_string(),
                serde_json::json!(pres_penalty),
            );
        }
        if let Some(n) = self.n {
            kwargs.insert("n".to_string(), serde_json::json!(n));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::Value::Object(kwargs),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_azure_openai_builder() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_temperature(0.7)
            .with_max_tokens(100);

        assert_eq!(chat.deployment_name, "gpt-4");
        assert_eq!(chat.temperature, Some(0.7));
        assert_eq!(chat.max_tokens, Some(100));
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![Message::human("Hello!"), Message::ai("Hi there!")];

        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();
        assert_eq!(openai_messages.len(), 2);
    }

    #[test]
    fn test_with_tools() {
        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }];

        let chat = ChatAzureOpenAI::new().with_tools(tools);
        assert!(chat.tools.is_some());
        assert_eq!(chat.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_with_json_mode() {
        let chat = ChatAzureOpenAI::new().with_json_mode();
        assert!(matches!(
            chat.response_format,
            Some(ResponseFormat::JsonObject)
        ));
    }

    #[test]
    fn test_with_structured_output() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let chat = ChatAzureOpenAI::new().with_structured_output("Person", schema, true);
        assert!(chat.response_format.is_some());
    }

    #[test]
    fn test_serialization() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_temperature(0.7);

        let serialized = chat.to_json();
        match serialized {
            SerializedObject::Constructor { id, kwargs, .. } => {
                assert_eq!(id[3], "ChatAzureOpenAI");
                assert_eq!(kwargs["deployment_name"], "gpt-4");
                // f32 has precision issues, so compare approximately
                let temp = kwargs["temperature"].as_f64().unwrap();
                assert!(
                    (temp - 0.7).abs() < 0.001,
                    "Temperature should be approximately 0.7, got {}",
                    temp
                );
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_builder_accumulates_azure_config() {
        // Test that chaining with_endpoint, with_api_key, with_api_version
        // preserves all values (this was a bug - each method would reset the config)
        let chat = ChatAzureOpenAI::new()
            .with_endpoint("https://test.openai.azure.com")
            .with_api_key("test-key-123")
            .with_api_version("2024-10-21");

        // Verify all values are preserved (stored in struct fields)
        assert_eq!(
            chat.azure_endpoint,
            Some("https://test.openai.azure.com".to_string())
        );
        assert_eq!(chat.azure_api_key, Some("test-key-123".to_string()));
        assert_eq!(chat.azure_api_version, Some("2024-10-21".to_string()));
    }

    #[test]
    fn test_builder_accumulates_in_any_order() {
        // Test that builder methods work in any order
        let chat1 = ChatAzureOpenAI::new()
            .with_api_key("key1")
            .with_endpoint("https://endpoint1.azure.com")
            .with_api_version("2024-01-01");

        let chat2 = ChatAzureOpenAI::new()
            .with_api_version("2024-01-01")
            .with_api_key("key1")
            .with_endpoint("https://endpoint1.azure.com");

        // Both should have the same config regardless of order
        assert_eq!(chat1.azure_endpoint, chat2.azure_endpoint);
        assert_eq!(chat1.azure_api_key, chat2.azure_api_key);
        assert_eq!(chat1.azure_api_version, chat2.azure_api_version);
    }

    #[test]
    fn test_builder_preserves_model_params_with_azure_config() {
        // Test that model parameters (temperature, max_tokens) are preserved
        // when azure config methods are called
        let chat = ChatAzureOpenAI::new()
            .with_temperature(0.5)
            .with_max_tokens(100)
            .with_endpoint("https://test.azure.com")
            .with_api_key("key")
            .with_top_p(0.9);

        // Model params should be preserved
        assert_eq!(chat.temperature, Some(0.5));
        assert_eq!(chat.max_tokens, Some(100));
        assert_eq!(chat.top_p, Some(0.9));

        // Azure config should be set
        assert_eq!(
            chat.azure_endpoint,
            Some("https://test.azure.com".to_string())
        );
        assert_eq!(chat.azure_api_key, Some("key".to_string()));
    }

    // ============================================
    // Debug and Default trait tests
    // ============================================

    #[test]
    fn test_debug_redacts_api_key() {
        let chat = ChatAzureOpenAI::new()
            .with_api_key("super-secret-key-12345");

        let debug_str = format!("{:?}", chat);

        // Should NOT contain the actual key
        assert!(!debug_str.contains("super-secret-key-12345"));
        // Should contain redacted marker
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_debug_without_api_key() {
        let chat = ChatAzureOpenAI::new();
        let debug_str = format!("{:?}", chat);

        // Should still render without panic
        assert!(debug_str.contains("ChatAzureOpenAI"));
        assert!(debug_str.contains("deployment_name"));
    }

    #[test]
    fn test_debug_shows_rate_limiter_indicator() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let chat = ChatAzureOpenAI::new()
            .with_rate_limiter(rate_limiter);

        let debug_str = format!("{:?}", chat);
        assert!(debug_str.contains("RateLimiter"));
    }

    #[test]
    fn test_default_trait() {
        let chat = ChatAzureOpenAI::default();
        // Default should be same as new()
        assert_eq!(chat.deployment_name, "gpt-35-turbo");
        assert!(chat.temperature.is_none());
        assert!(chat.max_tokens.is_none());
    }

    // ============================================
    // Builder method tests
    // ============================================

    #[test]
    fn test_with_top_p() {
        let chat = ChatAzureOpenAI::new().with_top_p(0.95);
        assert_eq!(chat.top_p, Some(0.95));
    }

    #[test]
    fn test_with_frequency_penalty() {
        let chat = ChatAzureOpenAI::new().with_frequency_penalty(0.5);
        assert_eq!(chat.frequency_penalty, Some(0.5));
    }

    #[test]
    fn test_with_frequency_penalty_negative() {
        let chat = ChatAzureOpenAI::new().with_frequency_penalty(-1.5);
        assert_eq!(chat.frequency_penalty, Some(-1.5));
    }

    #[test]
    fn test_with_presence_penalty() {
        let chat = ChatAzureOpenAI::new().with_presence_penalty(0.8);
        assert_eq!(chat.presence_penalty, Some(0.8));
    }

    #[test]
    fn test_with_presence_penalty_negative() {
        let chat = ChatAzureOpenAI::new().with_presence_penalty(-2.0);
        assert_eq!(chat.presence_penalty, Some(-2.0));
    }

    #[test]
    fn test_with_n() {
        let chat = ChatAzureOpenAI::new().with_n(3);
        assert_eq!(chat.n, Some(3));
    }

    #[test]
    fn test_with_n_one() {
        let chat = ChatAzureOpenAI::new().with_n(1);
        assert_eq!(chat.n, Some(1));
    }

    #[test]
    fn test_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let chat = ChatAzureOpenAI::new().with_retry_policy(policy.clone());
        // RetryPolicy doesn't implement PartialEq, just verify it was set
        assert_eq!(format!("{:?}", chat.retry_policy), format!("{:?}", policy));
    }

    #[test]
    fn test_with_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, 1000); // 1000ms = 1 second
        let chat = ChatAzureOpenAI::new().with_retry_policy(policy);
        // Just verify it doesn't panic
        let _ = format!("{:?}", chat);
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let chat = ChatAzureOpenAI::new().with_rate_limiter(rate_limiter);
        assert!(chat.rate_limiter.is_some());
    }

    // ============================================
    // Tool choice tests
    // ============================================

    #[test]
    fn test_with_tool_choice_auto() {
        let chat = ChatAzureOpenAI::new().with_tool_choice(ToolChoice::Auto);
        assert!(chat.tool_choice.is_some());
    }

    #[test]
    fn test_with_tool_choice_none() {
        let chat = ChatAzureOpenAI::new().with_tool_choice(ToolChoice::None);
        assert!(chat.tool_choice.is_some());
    }

    #[test]
    fn test_with_tool_choice_required() {
        let chat = ChatAzureOpenAI::new().with_tool_choice(ToolChoice::Required);
        assert!(chat.tool_choice.is_some());
    }

    #[test]
    fn test_with_tool_choice_specific() {
        let chat = ChatAzureOpenAI::new()
            .with_tool_choice(ToolChoice::Specific("get_weather".to_string()));
        assert!(chat.tool_choice.is_some());
    }

    // ============================================
    // ChatModel trait tests
    // ============================================

    #[test]
    fn test_llm_type() {
        let chat = ChatAzureOpenAI::new();
        assert_eq!(chat.llm_type(), "azure-openai-chat");
    }

    #[test]
    fn test_as_any() {
        let chat = ChatAzureOpenAI::new();
        let any = chat.as_any();
        assert!(any.downcast_ref::<ChatAzureOpenAI>().is_some());
    }

    #[test]
    fn test_identifying_params_minimal() {
        let chat = ChatAzureOpenAI::new().with_deployment_name("gpt-4");
        let params = chat.identifying_params();

        assert_eq!(params.get("deployment_name"), Some(&serde_json::json!("gpt-4")));
        assert!(!params.contains_key("temperature"));
        assert!(!params.contains_key("max_tokens"));
    }

    #[test]
    fn test_identifying_params_with_temperature() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_temperature(0.7);
        let params = chat.identifying_params();

        assert_eq!(params.get("deployment_name"), Some(&serde_json::json!("gpt-4")));
        // f32 precision - check approximately
        let temp = params.get("temperature").and_then(|v| v.as_f64()).unwrap();
        assert!((temp - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_identifying_params_with_max_tokens() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_max_tokens(500);
        let params = chat.identifying_params();

        assert_eq!(params.get("max_tokens"), Some(&serde_json::json!(500)));
    }

    #[test]
    fn test_identifying_params_full() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4-turbo")
            .with_temperature(0.5)
            .with_max_tokens(1000);
        let params = chat.identifying_params();

        assert_eq!(params.len(), 3);
        assert!(params.contains_key("deployment_name"));
        assert!(params.contains_key("temperature"));
        assert!(params.contains_key("max_tokens"));
    }

    // ============================================
    // Serializable trait tests
    // ============================================

    #[test]
    fn test_lc_id() {
        let chat = ChatAzureOpenAI::new();
        let id = chat.lc_id();

        assert_eq!(id.len(), 4);
        assert_eq!(id[0], "dashflow");
        assert_eq!(id[1], "chat_models");
        assert_eq!(id[2], "azure_openai");
        assert_eq!(id[3], "ChatAzureOpenAI");
    }

    #[test]
    fn test_is_lc_serializable() {
        let chat = ChatAzureOpenAI::new();
        assert!(chat.is_lc_serializable());
    }

    #[test]
    fn test_serialization_minimal() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4");

        let serialized = chat.to_json();
        match serialized {
            SerializedObject::Constructor { kwargs, .. } => {
                assert_eq!(kwargs["deployment_name"], "gpt-4");
                // No optional params should be present
                assert!(kwargs.get("temperature").is_none());
                assert!(kwargs.get("max_tokens").is_none());
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_serialization_with_all_params() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_temperature(0.8)
            .with_max_tokens(2000)
            .with_top_p(0.95)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3)
            .with_n(2);

        let serialized = chat.to_json();
        match serialized {
            SerializedObject::Constructor { kwargs, .. } => {
                assert_eq!(kwargs["deployment_name"], "gpt-4");
                assert!(kwargs.get("temperature").is_some());
                assert!(kwargs.get("max_tokens").is_some());
                assert!(kwargs.get("top_p").is_some());
                assert!(kwargs.get("frequency_penalty").is_some());
                assert!(kwargs.get("presence_penalty").is_some());
                assert!(kwargs.get("n").is_some());
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_serialization_version() {
        let chat = ChatAzureOpenAI::new();
        let serialized = chat.to_json();

        match serialized {
            SerializedObject::Constructor { lc, .. } => {
                assert_eq!(lc, SERIALIZATION_VERSION);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    // ============================================
    // Message conversion tests
    // ============================================

    #[test]
    fn test_convert_messages_system() {
        let messages = vec![Message::system("You are a helpful assistant.")];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::System(_) => {}
            _ => panic!("Expected System message"),
        }
    }

    #[test]
    fn test_convert_messages_human() {
        let messages = vec![Message::human("Hello!")];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::User(_) => {}
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_convert_messages_ai() {
        let messages = vec![Message::ai("Hello! How can I help you?")];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::Assistant(_) => {}
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_convert_messages_ai_with_tool_calls() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            args: serde_json::json!({"location": "NYC"}),
            tool_type: "function".to_string(),
            index: None,
        };

        let mut ai_msg = AIMessage::new("I'll check the weather for you.");
        ai_msg = ai_msg.with_tool_calls(vec![tool_call]);

        let messages = vec![ai_msg.into()];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::Assistant(msg) => {
                assert!(msg.tool_calls.is_some());
                let calls = msg.tool_calls.as_ref().unwrap();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_123");
            }
            _ => panic!("Expected Assistant message with tool calls"),
        }
    }

    #[test]
    fn test_convert_messages_tool() {
        let messages = vec![Message::tool("The weather is sunny.", "call_123")];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::Tool(msg) => {
                assert_eq!(msg.tool_call_id, "call_123");
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[test]
    fn test_convert_messages_function_to_tool() {
        // Function messages should be converted to tool messages
        use dashflow::core::messages::MessageContent;
        let messages = vec![Message::Function {
            content: MessageContent::Text("Result data".to_string()),
            name: "my_function".to_string(),
            fields: Default::default(),
        }];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();

        assert_eq!(openai_messages.len(), 1);
        match &openai_messages[0] {
            ChatCompletionRequestMessage::Tool(msg) => {
                assert_eq!(msg.tool_call_id, "func_my_function");
            }
            _ => panic!("Expected Tool message (converted from function)"),
        }
    }

    #[test]
    fn test_convert_messages_conversation() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::human("What's the weather like?"),
            Message::ai("I'll check that for you."),
            Message::tool("Sunny, 72°F", "call_weather"),
            Message::ai("The weather is sunny and 72°F."),
        ];

        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();
        assert_eq!(openai_messages.len(), 5);
    }

    #[test]
    fn test_convert_messages_empty() {
        let messages: Vec<Message> = vec![];
        let openai_messages = ChatAzureOpenAI::convert_messages(&messages).unwrap();
        assert!(openai_messages.is_empty());
    }

    // ============================================
    // Tools configuration tests
    // ============================================

    #[test]
    fn test_with_tools_empty_description() {
        let tools = vec![ToolDefinition {
            name: "no_desc_tool".to_string(),
            description: "".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }];

        let chat = ChatAzureOpenAI::new().with_tools(tools);
        assert!(chat.tools.is_some());

        let openai_tools = chat.tools.unwrap();
        assert_eq!(openai_tools.len(), 1);
        // Empty description should become None
        assert!(openai_tools[0].function.description.is_none());
    }

    #[test]
    fn test_with_tools_multiple() {
        let tools = vec![
            ToolDefinition {
                name: "tool1".to_string(),
                description: "First tool".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
            ToolDefinition {
                name: "tool2".to_string(),
                description: "Second tool".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
            ToolDefinition {
                name: "tool3".to_string(),
                description: "Third tool".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        ];

        let chat = ChatAzureOpenAI::new().with_tools(tools);
        assert_eq!(chat.tools.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_with_tools_complex_parameters() {
        let tools = vec![ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A tool with complex parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                    "filters": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["query"]
            }),
        }];

        let chat = ChatAzureOpenAI::new().with_tools(tools);
        assert!(chat.tools.is_some());
    }

    // ============================================
    // Response format tests
    // ============================================

    #[test]
    fn test_with_structured_output_strict_false() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"}
            }
        });

        let chat = ChatAzureOpenAI::new().with_structured_output("Answer", schema, false);

        match &chat.response_format {
            Some(ResponseFormat::JsonSchema { json_schema }) => {
                assert_eq!(json_schema.name, "Answer");
                assert_eq!(json_schema.strict, Some(false));
            }
            _ => panic!("Expected JsonSchema response format"),
        }
    }

    #[test]
    fn test_with_structured_output_with_description() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let chat = ChatAzureOpenAI::new().with_structured_output("Person", schema.clone(), true);

        match &chat.response_format {
            Some(ResponseFormat::JsonSchema { json_schema }) => {
                assert_eq!(json_schema.name, "Person");
                assert_eq!(json_schema.schema, Some(schema));
                assert_eq!(json_schema.strict, Some(true));
            }
            _ => panic!("Expected JsonSchema response format"),
        }
    }

    // ============================================
    // Full builder chain tests
    // ============================================

    #[test]
    fn test_full_builder_chain() {
        let tools = vec![ToolDefinition {
            name: "test".to_string(),
            description: "Test tool".to_string(),
            parameters: serde_json::json!({}),
        }];

        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4-turbo")
            .with_endpoint("https://myresource.openai.azure.com")
            .with_api_key("my-key")
            .with_api_version("2024-10-21")
            .with_temperature(0.7)
            .with_max_tokens(4096)
            .with_top_p(0.95)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3)
            .with_n(1)
            .with_tools(tools)
            .with_tool_choice(ToolChoice::Auto)
            .with_json_mode();

        assert_eq!(chat.deployment_name, "gpt-4-turbo");
        assert_eq!(chat.azure_endpoint, Some("https://myresource.openai.azure.com".to_string()));
        assert_eq!(chat.azure_api_key, Some("my-key".to_string()));
        assert_eq!(chat.azure_api_version, Some("2024-10-21".to_string()));
        assert_eq!(chat.temperature, Some(0.7));
        assert_eq!(chat.max_tokens, Some(4096));
        assert_eq!(chat.top_p, Some(0.95));
        assert_eq!(chat.frequency_penalty, Some(0.5));
        assert_eq!(chat.presence_penalty, Some(0.3));
        assert_eq!(chat.n, Some(1));
        assert!(chat.tools.is_some());
        assert!(chat.tool_choice.is_some());
        assert!(matches!(chat.response_format, Some(ResponseFormat::JsonObject)));
    }

    #[test]
    fn test_clone() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("gpt-4")
            .with_temperature(0.5);

        let cloned = chat.clone();

        assert_eq!(cloned.deployment_name, "gpt-4");
        assert_eq!(cloned.temperature, Some(0.5));
    }

    // ============================================
    // Edge case tests
    // ============================================

    #[test]
    fn test_temperature_bounds() {
        // Temperature 0
        let chat = ChatAzureOpenAI::new().with_temperature(0.0);
        assert_eq!(chat.temperature, Some(0.0));

        // Temperature 2 (max)
        let chat = ChatAzureOpenAI::new().with_temperature(2.0);
        assert_eq!(chat.temperature, Some(2.0));
    }

    #[test]
    fn test_max_tokens_large() {
        let chat = ChatAzureOpenAI::new().with_max_tokens(128000);
        assert_eq!(chat.max_tokens, Some(128000));
    }

    #[test]
    fn test_deployment_name_with_special_chars() {
        let chat = ChatAzureOpenAI::new()
            .with_deployment_name("my-gpt-4-deployment_v2");
        assert_eq!(chat.deployment_name, "my-gpt-4-deployment_v2");
    }

    #[test]
    fn test_empty_endpoint() {
        let chat = ChatAzureOpenAI::new()
            .with_endpoint("");
        assert_eq!(chat.azure_endpoint, Some("".to_string()));
    }

    #[test]
    fn test_with_api_version_custom() {
        let chat = ChatAzureOpenAI::new()
            .with_api_version("2025-01-01-preview");
        assert_eq!(chat.azure_api_version, Some("2025-01-01-preview".to_string()));
    }
}
