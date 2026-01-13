// Allow clippy warnings for Together chat models
// - clone_on_ref_ptr: Arc cloned for streaming operations
// - redundant_clone: OpenAI types require owned strings
#![allow(clippy::clone_on_ref_ptr, clippy::redundant_clone)]

//! Together AI chat model implementation

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPartImage,
        ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageContentPart,
        ChatCompletionTool, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs,
        FunctionCall, FunctionObject, ImageDetail as OpenAIImageDetail, ImageUrl,
    },
    Client,
};
use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string, TOGETHER_API_KEY},
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessageChunk, BaseMessage, InvalidToolCall, Message, ToolCall},
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

/// Together AI default base URL
const TOGETHER_API_BASE: &str = "https://api.together.xyz/v1";

/// Together AI default model
const DEFAULT_MODEL: &str = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo";

/// Together AI chat model configuration and client
///
/// Together AI provides access to 100+ open-source models including Llama,
/// Mistral, `CodeLlama`, and more. The API is OpenAI-compatible.
///
/// # Example
/// ```no_run
/// use dashflow_together::ChatTogether;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatTogether::new()
///         .with_model("meta-llama/Llama-3-70b-chat-hf")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Authentication
/// Set the `TOGETHER_API_KEY` environment variable with your Together AI API key.
#[derive(Clone, Debug)]
pub struct ChatTogether {
    /// OpenAI-compatible client configured for Together AI
    client: Arc<Client<OpenAIConfig>>,

    /// Model name (e.g., "meta-llama/Llama-3-70b-chat-hf")
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

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl ChatTogether {
    /// Create a new `ChatTogether` instance with default settings
    ///
    /// Uses `TOGETHER_API_KEY` environment variable for authentication
    #[must_use]
    pub fn new() -> Self {
        let api_key = env_string(TOGETHER_API_KEY).unwrap_or_default();

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(TOGETHER_API_BASE);

        Self::with_config(config)
    }

    /// Create a new `ChatTogether` instance with custom configuration
    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            client: Arc::new(Client::with_config(config)),
            model: DEFAULT_MODEL.to_string(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            n: None,
            tools: None,
            tool_choice: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Create with explicit API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key.into())
            .with_api_base(TOGETHER_API_BASE);
        Self::with_config(config)
    }

    /// Set the model name
    ///
    /// Popular models:
    /// - meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo
    /// - meta-llama/Llama-3-70b-chat-hf
    /// - mistralai/Mixtral-8x7B-Instruct-v0.1
    /// - codellama/CodeLlama-70b-Instruct-hf
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
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Bind tools to the model for function calling
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
}

impl Default for ChatTogether {
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
                    _ => None,
                })
                .collect();

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

            // Set content if present
            if !content.as_text().is_empty() {
                builder.content(content.as_text());
            }

            // Convert tool calls if present
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

            let msg = builder
                .build()
                .map_err(|e| Error::invalid_input(format!("Failed to build AI message: {e}")))?;
            Ok(ChatCompletionRequestMessage::Assistant(msg))
        }
        Message::Tool {
            content,
            tool_call_id,
            ..
        } => {
            let msg = ChatCompletionRequestToolMessageArgs::default()
                .content(content.as_text())
                .tool_call_id(tool_call_id)
                .build()
                .map_err(|e| Error::invalid_input(format!("Failed to build tool message: {e}")))?;
            Ok(ChatCompletionRequestMessage::Tool(msg))
        }
        Message::Function { .. } => Err(Error::invalid_input(
            "Function messages are deprecated. Use Tool messages instead.".to_string(),
        )),
    }
}

#[async_trait]
impl ChatModel for ChatTogether {
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
                .on_llm_start(&serialized, &prompts, run_id, None, &[], &HashMap::new())
                .await?;
        }

        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        // Build request with retry
        let result = with_retry(&self.retry_policy, || async {
            // Convert messages
            let mut openai_messages: Vec<ChatCompletionRequestMessage> =
                Vec::with_capacity(messages.len());
            for msg in messages {
                openai_messages.push(convert_message(msg)?);
            }

            // Build request
            let mut request = CreateChatCompletionRequestArgs::default();
            request.model(&self.model).messages(openai_messages);

            // Add optional parameters
            if let Some(temp) = self.temperature {
                request.temperature(temp);
            }
            if let Some(max_tok) = self.max_tokens {
                request.max_tokens(max_tok);
            }
            if let Some(p) = self.top_p {
                request.top_p(p);
            }
            if let Some(fp) = self.frequency_penalty {
                request.frequency_penalty(fp);
            }
            if let Some(pp) = self.presence_penalty {
                request.presence_penalty(pp);
            }
            if let Some(n) = self.n {
                request.n(n);
            }
            if let Some(stop_seqs) = stop {
                request.stop(stop_seqs.to_vec());
            }

            // Add tools if provided
            let combined_tools: Vec<ChatCompletionTool> = if let Some(tool_defs) = tools {
                tool_defs.iter().map(convert_tool_definition).collect()
            } else {
                self.tools.clone().unwrap_or_default()
            };

            if !combined_tools.is_empty() {
                request.tools(combined_tools);

                // Set tool choice
                if let Some(tc) = tool_choice {
                    request.tool_choice(convert_tool_choice(tc));
                } else if let Some(tc) = &self.tool_choice {
                    request.tool_choice(tc.clone());
                }
            }

            let request = request
                .build()
                .map_err(|e| Error::invalid_input(format!("Failed to build request: {e}")))?;

            // Make API call
            let response = self
                .client
                .chat()
                .create(request)
                .await
                .map_err(|e| Error::api(format!("Together AI API error: {e}")))?;

            Ok(response)
        })
        .await?;

        // Extract usage metadata
        let usage = result.usage.as_ref().map(|u| UsageMetadata {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            input_token_details: None,
            output_token_details: None,
        });

        // Convert response to ChatResult
        let generations: Vec<ChatGeneration> = result
            .choices
            .iter()
            .map(|choice| {
                let content = choice.message.content.clone().unwrap_or_default();

                // Extract tool calls
                let tool_calls: Vec<ToolCall> = choice
                    .message
                    .tool_calls
                    .as_ref()
                    .map(|tcs| {
                        tcs.iter()
                            .filter_map(|tc| {
                                serde_json::from_str(&tc.function.arguments)
                                    .ok()
                                    .map(|args| ToolCall {
                                        id: tc.id.clone(),
                                        name: tc.function.name.clone(),
                                        args,
                                        tool_type: "tool_call".to_string(),
                                        index: None,
                                    })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let invalid_tool_calls: Vec<InvalidToolCall> = choice
                    .message
                    .tool_calls
                    .as_ref()
                    .map(|tcs| {
                        tcs.iter()
                            .filter_map(|tc| {
                                if serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                                    .is_err()
                                {
                                    Some(InvalidToolCall {
                                        id: tc.id.clone(),
                                        name: Some(tc.function.name.clone()),
                                        args: Some(tc.function.arguments.clone()),
                                        error: "Failed to parse tool call arguments".to_string(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let message = Message::AI {
                    content: dashflow::core::messages::MessageContent::Text(content.clone()),
                    tool_calls,
                    invalid_tool_calls,
                    usage_metadata: usage.clone(),
                    fields: Default::default(),
                };

                ChatGeneration {
                    message,
                    generation_info: Some(HashMap::new()),
                }
            })
            .collect();

        let mut llm_output = HashMap::new();
        if let Some(u) = &usage {
            llm_output.insert(
                "usage".to_string(),
                serde_json::to_value(u).unwrap_or_default(),
            );
        }

        let result = ChatResult {
            generations,
            llm_output: Some(llm_output.clone()),
        };

        // Call on_llm_end callback
        if let Some(manager) = run_manager {
            manager.on_llm_end(&llm_output, run_id, None).await?;
        }

        Ok(result)
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Clone data we need for the stream (to avoid lifetime issues)
        let rate_limiter = self.rate_limiter.clone();
        let client = self.client.clone();
        let model = self.model.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        let top_p = self.top_p;
        let frequency_penalty = self.frequency_penalty;
        let presence_penalty = self.presence_penalty;
        let stop_seqs = stop.map(<[std::string::String]>::to_vec);
        let tools_owned = tools.map(<[dashflow::core::language_models::ToolDefinition]>::to_vec);
        let tool_choice_owned = tool_choice.cloned();
        let self_tools = self.tools.clone();
        let self_tool_choice = self.tool_choice.clone();

        // Convert messages upfront
        let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        for msg in messages {
            openai_messages.push(convert_message(msg)?);
        }

        Ok(Box::pin(stream! {
            // Apply rate limiting if configured
            if let Some(limiter) = &rate_limiter {
                limiter.acquire().await;
            }

            // Build request
            let mut request = CreateChatCompletionRequestArgs::default();
            request.model(&model).messages(openai_messages).stream(true);

            // Add optional parameters
            if let Some(temp) = temperature {
                request.temperature(temp);
            }
            if let Some(max_tok) = max_tokens {
                request.max_tokens(max_tok);
            }
            if let Some(p) = top_p {
                request.top_p(p);
            }
            if let Some(fp) = frequency_penalty {
                request.frequency_penalty(fp);
            }
            if let Some(pp) = presence_penalty {
                request.presence_penalty(pp);
            }
            if let Some(ref stop_seqs_vec) = stop_seqs {
                request.stop(stop_seqs_vec.clone());
            }

            // Add tools if provided
            let combined_tools: Vec<ChatCompletionTool> = if let Some(ref tool_defs) = tools_owned {
                tool_defs.iter().map(convert_tool_definition).collect()
            } else {
                self_tools.unwrap_or_default()
            };

            if !combined_tools.is_empty() {
                request.tools(combined_tools);

                if let Some(ref tc) = tool_choice_owned {
                    request.tool_choice(convert_tool_choice(tc));
                } else if let Some(ref tc) = self_tool_choice {
                    request.tool_choice(tc.clone());
                }
            }

            let request = match request.build() {
                Ok(r) => r,
                Err(e) => {
                    yield Err(Error::invalid_input(format!("Failed to build request: {e}")));
                    return;
                }
            };

            // Make streaming API call
            let mut stream = match client.chat().create_stream(request).await {
                Ok(s) => s,
                Err(e) => {
                    yield Err(Error::api(format!("Together AI API error: {e}")));
                    return;
                }
            };

            // Stream responses
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            let content = choice.delta.content.unwrap_or_default();

                            let chunk = AIMessageChunk {
                                content: content.clone(),
                                tool_calls: Vec::new(),
                                invalid_tool_calls: Vec::new(),
                                usage_metadata: None,
                                fields: Default::default(),
                            };

                            yield Ok(ChatGenerationChunk {
                                message: chunk,
                                generation_info: Some(HashMap::new()),
                            });
                        }
                    }
                    Err(e) => {
                        yield Err(Error::api(format!("Stream error: {e}")));
                        return;
                    }
                }
            }
        }))
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        params.insert("model_name".to_string(), serde_json::json!(self.model));
        params.insert("provider".to_string(), serde_json::json!("together"));

        if let Some(temp) = self.temperature {
            params.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tok) = self.max_tokens {
            params.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }
        if let Some(p) = self.top_p {
            params.insert("top_p".to_string(), serde_json::json!(p));
        }

        params
    }

    fn llm_type(&self) -> &'static str {
        "together-chat"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Serializable for ChatTogether {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "together".to_string(),
            "ChatTogether".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Model name (required)
        kwargs.insert("model".to_string(), serde_json::json!(self.model));

        // Optional parameters
        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(max_tok) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), serde_json::json!(max_tok));
        }

        if let Some(tp) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(tp));
        }

        if let Some(fp) = self.frequency_penalty {
            kwargs.insert("frequency_penalty".to_string(), serde_json::json!(fp));
        }

        if let Some(pp) = self.presence_penalty {
            kwargs.insert("presence_penalty".to_string(), serde_json::json!(pp));
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

    fn lc_secrets(&self) -> std::collections::HashMap<String, String> {
        let mut secrets = std::collections::HashMap::new();
        secrets.insert("api_key".to_string(), "TOGETHER_API_KEY".to_string());
        secrets
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)]
mod tests {
    use super::*;
    use dashflow::core::language_models::ToolChoice;
    use dashflow::core::messages::{ContentBlock, ImageDetail, ImageSource, MessageContent};

    // ========== convert_image_source tests ==========

    #[test]
    fn test_convert_image_source_url() {
        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert_eq!(result.url, "https://example.com/image.png");
        assert!(result.detail.is_none());
    }

    #[test]
    fn test_convert_image_source_base64() {
        let source = ImageSource::Base64 {
            media_type: "image/png".to_string(),
            data: "abc123".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert_eq!(result.url, "data:image/png;base64,abc123");
    }

    #[test]
    fn test_convert_image_source_with_detail() {
        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };

        assert!(matches!(
            convert_image_source(&source, Some(ImageDetail::Low)).detail,
            Some(OpenAIImageDetail::Low)
        ));
        assert!(matches!(
            convert_image_source(&source, Some(ImageDetail::High)).detail,
            Some(OpenAIImageDetail::High)
        ));
        assert!(matches!(
            convert_image_source(&source, Some(ImageDetail::Auto)).detail,
            Some(OpenAIImageDetail::Auto)
        ));
    }

    // ========== convert_content tests ==========

    #[test]
    fn test_convert_content_text() {
        let content = MessageContent::Text("Hello world".to_string());
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s == "Hello world"
        ));
    }

    #[test]
    fn test_convert_content_single_text_block() {
        let content = MessageContent::Blocks(vec![ContentBlock::Text {
            text: "Single block".to_string(),
        }]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s == "Single block"
        ));
    }

    #[test]
    fn test_convert_content_multiple_blocks() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "First".to_string(),
            },
            ContentBlock::Text {
                text: "Second".to_string(),
            },
        ]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Array(ref parts) if parts.len() == 2
        ));
    }

    #[test]
    fn test_convert_content_filters_empty_text() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "".to_string(),
            },
            ContentBlock::Text {
                text: "Non-empty".to_string(),
            },
        ]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s == "Non-empty"
        ));
    }

    // ========== convert_tool_definition tests ==========

    #[test]
    fn test_convert_tool_definition_basic() {
        let tool = ToolDefinition {
            name: "my_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let result = convert_tool_definition(&tool);
        assert_eq!(result.function.name, "my_tool");
        assert_eq!(result.function.description, Some("A test tool".to_string()));
    }

    #[test]
    fn test_convert_tool_definition_empty_description() {
        let tool = ToolDefinition {
            name: "tool_no_desc".to_string(),
            description: "".to_string(),
            parameters: serde_json::json!({}),
        };
        let result = convert_tool_definition(&tool);
        assert!(result.function.description.is_none());
    }

    // ========== convert_tool_choice tests ==========

    #[test]
    fn test_convert_tool_choice_variants() {
        assert!(matches!(
            convert_tool_choice(&ToolChoice::Auto),
            ChatCompletionToolChoiceOption::Auto
        ));
        assert!(matches!(
            convert_tool_choice(&ToolChoice::None),
            ChatCompletionToolChoiceOption::None
        ));
        assert!(matches!(
            convert_tool_choice(&ToolChoice::Required),
            ChatCompletionToolChoiceOption::Required
        ));
    }

    #[test]
    fn test_convert_tool_choice_specific() {
        let choice = ToolChoice::Specific("my_function".to_string());
        let result = convert_tool_choice(&choice);
        assert!(matches!(
            result,
            ChatCompletionToolChoiceOption::Named(ref named) if named.function.name == "my_function"
        ));
    }

    // ========== convert_message tests ==========

    #[test]
    fn test_convert_message_system() {
        let msg = Message::system("You are a helpful assistant");
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::System(_)));
    }

    #[test]
    fn test_convert_message_human() {
        let msg = Message::human("Hello!");
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::User(_)));
    }

    #[test]
    fn test_convert_message_ai() {
        let msg = Message::AI {
            content: MessageContent::Text("I'm here to help!".to_string()),
            tool_calls: vec![],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::Assistant(_)));
    }

    #[test]
    fn test_convert_message_tool() {
        let msg = Message::tool("Result: 42", "call_123");
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::Tool(_)));
    }

    #[test]
    fn test_convert_message_function_deprecated() {
        let msg = Message::Function {
            content: MessageContent::Text("result".to_string()),
            name: "old_func".to_string(),
            fields: Default::default(),
        };
        assert!(convert_message(&msg).is_err());
    }

    // ========== Builder method tests ==========

    #[test]
    fn test_builder_with_model() {
        let model = ChatTogether::new().with_model("meta-llama/Llama-3-70b-chat-hf");
        assert_eq!(model.model, "meta-llama/Llama-3-70b-chat-hf");
    }

    #[test]
    fn test_builder_with_temperature() {
        let model = ChatTogether::new().with_temperature(0.7);
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    fn test_builder_with_max_tokens() {
        let model = ChatTogether::new().with_max_tokens(100);
        assert_eq!(model.max_tokens, Some(100));
    }

    #[test]
    fn test_builder_with_top_p() {
        let model = ChatTogether::new().with_top_p(0.9);
        assert_eq!(model.top_p, Some(0.9));
    }

    #[test]
    fn test_builder_with_frequency_penalty() {
        let model = ChatTogether::new().with_frequency_penalty(0.5);
        assert_eq!(model.frequency_penalty, Some(0.5));
    }

    #[test]
    fn test_builder_with_presence_penalty() {
        let model = ChatTogether::new().with_presence_penalty(0.3);
        assert_eq!(model.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_builder_with_n() {
        let model = ChatTogether::new().with_n(2);
        assert_eq!(model.n, Some(2));
    }

    #[test]
    fn test_builder_chained() {
        let model = ChatTogether::new()
            .with_model("custom-model")
            .with_temperature(0.5)
            .with_max_tokens(200)
            .with_top_p(0.95);

        assert_eq!(model.model, "custom-model");
        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_tokens, Some(200));
        assert_eq!(model.top_p, Some(0.95));
    }

    // ========== with_tool_choice tests ==========

    #[test]
    fn test_with_tool_choice_keywords() {
        assert!(matches!(
            ChatTogether::new().with_tool_choice(Some("none".to_string())).tool_choice,
            Some(ChatCompletionToolChoiceOption::None)
        ));
        assert!(matches!(
            ChatTogether::new().with_tool_choice(Some("auto".to_string())).tool_choice,
            Some(ChatCompletionToolChoiceOption::Auto)
        ));
        assert!(matches!(
            ChatTogether::new().with_tool_choice(Some("required".to_string())).tool_choice,
            Some(ChatCompletionToolChoiceOption::Required)
        ));
    }

    #[test]
    fn test_with_tool_choice_specific_function() {
        let model = ChatTogether::new().with_tool_choice(Some("get_weather".to_string()));
        assert!(matches!(
            model.tool_choice,
            Some(ChatCompletionToolChoiceOption::Named(ref named)) if named.function.name == "get_weather"
        ));
    }

    // ========== identifying_params tests ==========

    #[test]
    fn test_identifying_params_basic() {
        let model = ChatTogether::new().with_model("test-model");
        let params = model.identifying_params();

        assert_eq!(params.get("model_name").unwrap(), "test-model");
        assert_eq!(params.get("provider").unwrap(), "together");
    }

    #[test]
    fn test_identifying_params_with_options() {
        let model = ChatTogether::new()
            .with_model("test-model")
            .with_temperature(0.8)
            .with_max_tokens(500)
            .with_top_p(0.9);

        let params = model.identifying_params();

        // Use f64 comparison for floating point values
        let temp = params.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.8).abs() < 0.01);
        assert_eq!(params.get("max_tokens").unwrap(), 500);
        let top_p = params.get("top_p").unwrap().as_f64().unwrap();
        assert!((top_p - 0.9).abs() < 0.01);
    }

    // ========== llm_type test ==========

    #[test]
    fn test_llm_type() {
        let model = ChatTogether::new();
        assert_eq!(model.llm_type(), "together-chat");
    }

    // ========== Serializable tests ==========

    #[test]
    fn test_lc_id() {
        let model = ChatTogether::new();
        let id = model.lc_id();

        assert_eq!(id.len(), 4);
        assert_eq!(id[0], "dashflow");
        assert_eq!(id[1], "chat_models");
        assert_eq!(id[2], "together");
        assert_eq!(id[3], "ChatTogether");
    }

    #[test]
    fn test_is_lc_serializable() {
        let model = ChatTogether::new();
        assert!(model.is_lc_serializable());
    }

    #[test]
    fn test_to_json_basic() {
        let model = ChatTogether::new().with_model("test-model");
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                assert_eq!(kwargs.get("model").unwrap(), "test-model");
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_with_options() {
        let model = ChatTogether::new()
            .with_model("test-model")
            .with_temperature(0.7)
            .with_max_tokens(100)
            .with_n(2);

        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                // Use f64 comparison for floating point values
                let temp = kwargs.get("temperature").unwrap().as_f64().unwrap();
                assert!((temp - 0.7).abs() < 0.01);
                assert_eq!(kwargs.get("max_tokens").unwrap(), 100);
                assert_eq!(kwargs.get("n").unwrap(), 2);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_lc_secrets() {
        let model = ChatTogether::new();
        let secrets = model.lc_secrets();

        assert_eq!(secrets.get("api_key").unwrap(), "TOGETHER_API_KEY");
    }

    // ========== Default trait test ==========

    #[test]
    fn test_default() {
        let model = ChatTogether::default();
        assert_eq!(model.model, DEFAULT_MODEL);
    }
}

// ========================================================================
// Comprehensive unit tests that do not require API key
// ========================================================================
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp
)]
mod comprehensive_unit_tests {
    use super::*;
    use dashflow::core::language_models::ToolChoice;
    use dashflow::core::messages::{ContentBlock, ImageDetail, ImageSource, MessageContent};
    use dashflow::core::retry::RetryPolicy;

    // ========== API Base URL and Constants Tests ==========

    #[test]
    fn test_together_api_base_is_https() {
        assert!(TOGETHER_API_BASE.starts_with("https://"));
    }

    #[test]
    fn test_together_api_base_correct_endpoint() {
        assert_eq!(TOGETHER_API_BASE, "https://api.together.xyz/v1");
    }

    #[test]
    fn test_default_model_is_set() {
        assert!(!DEFAULT_MODEL.is_empty());
        assert!(DEFAULT_MODEL.contains('/'));
    }

    #[test]
    fn test_default_model_is_llama() {
        assert_eq!(DEFAULT_MODEL, "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo");
    }

    // ========== Default Field Values Tests ==========

    #[test]
    fn test_default_temperature_is_none() {
        let model = ChatTogether::new();
        assert!(model.temperature.is_none());
    }

    #[test]
    fn test_default_max_tokens_is_none() {
        let model = ChatTogether::new();
        assert!(model.max_tokens.is_none());
    }

    #[test]
    fn test_default_top_p_is_none() {
        let model = ChatTogether::new();
        assert!(model.top_p.is_none());
    }

    #[test]
    fn test_default_frequency_penalty_is_none() {
        let model = ChatTogether::new();
        assert!(model.frequency_penalty.is_none());
    }

    #[test]
    fn test_default_presence_penalty_is_none() {
        let model = ChatTogether::new();
        assert!(model.presence_penalty.is_none());
    }

    #[test]
    fn test_default_n_is_none() {
        let model = ChatTogether::new();
        assert!(model.n.is_none());
    }

    #[test]
    fn test_default_tools_is_none() {
        let model = ChatTogether::new();
        assert!(model.tools.is_none());
    }

    #[test]
    fn test_default_tool_choice_is_none() {
        let model = ChatTogether::new();
        assert!(model.tool_choice.is_none());
    }

    #[test]
    fn test_default_rate_limiter_is_none() {
        let model = ChatTogether::new();
        assert!(model.rate_limiter.is_none());
    }

    #[test]
    fn test_default_model_value() {
        let model = ChatTogether::new();
        assert_eq!(model.model, DEFAULT_MODEL);
    }

    // ========== Builder Pattern Tests ==========

    #[test]
    fn test_with_api_key_creates_instance() {
        let model = ChatTogether::with_api_key("test-api-key");
        assert_eq!(model.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_api_key_string_owned() {
        let key = String::from("my-api-key");
        let model = ChatTogether::with_api_key(key);
        assert_eq!(model.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_model_updates_model() {
        let model = ChatTogether::new().with_model("mistralai/Mixtral-8x7B-Instruct-v0.1");
        assert_eq!(model.model, "mistralai/Mixtral-8x7B-Instruct-v0.1");
    }

    #[test]
    fn test_with_model_accepts_string() {
        let model_name = String::from("codellama/CodeLlama-70b-Instruct-hf");
        let model = ChatTogether::new().with_model(model_name);
        assert_eq!(model.model, "codellama/CodeLlama-70b-Instruct-hf");
    }

    #[test]
    fn test_with_temperature_zero() {
        let model = ChatTogether::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_with_temperature_max() {
        let model = ChatTogether::new().with_temperature(2.0);
        assert_eq!(model.temperature, Some(2.0));
    }

    #[test]
    fn test_with_max_tokens_small() {
        let model = ChatTogether::new().with_max_tokens(1);
        assert_eq!(model.max_tokens, Some(1));
    }

    #[test]
    fn test_with_max_tokens_large() {
        let model = ChatTogether::new().with_max_tokens(128000);
        assert_eq!(model.max_tokens, Some(128000));
    }

    #[test]
    fn test_with_top_p_zero() {
        let model = ChatTogether::new().with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_with_top_p_one() {
        let model = ChatTogether::new().with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    #[test]
    fn test_with_frequency_penalty_negative() {
        let model = ChatTogether::new().with_frequency_penalty(-2.0);
        assert_eq!(model.frequency_penalty, Some(-2.0));
    }

    #[test]
    fn test_with_frequency_penalty_positive() {
        let model = ChatTogether::new().with_frequency_penalty(2.0);
        assert_eq!(model.frequency_penalty, Some(2.0));
    }

    #[test]
    fn test_with_presence_penalty_negative() {
        let model = ChatTogether::new().with_presence_penalty(-2.0);
        assert_eq!(model.presence_penalty, Some(-2.0));
    }

    #[test]
    fn test_with_presence_penalty_positive() {
        let model = ChatTogether::new().with_presence_penalty(2.0);
        assert_eq!(model.presence_penalty, Some(2.0));
    }

    #[test]
    fn test_with_n_one() {
        let model = ChatTogether::new().with_n(1);
        assert_eq!(model.n, Some(1));
    }

    #[test]
    fn test_with_n_max() {
        let model = ChatTogether::new().with_n(255);
        assert_eq!(model.n, Some(255));
    }

    #[test]
    fn test_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let model = ChatTogether::new().with_retry_policy(policy);
        // Policy was set (we can't easily inspect it, but at least no panic)
        assert!(model.temperature.is_none()); // Other fields unchanged
    }

    #[test]
    fn test_with_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        let model = ChatTogether::new().with_retry_policy(policy);
        assert_eq!(model.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_builder_chaining_all_options() {
        let model = ChatTogether::new()
            .with_model("custom-model")
            .with_temperature(0.5)
            .with_max_tokens(1000)
            .with_top_p(0.9)
            .with_frequency_penalty(0.1)
            .with_presence_penalty(0.2)
            .with_n(3);

        assert_eq!(model.model, "custom-model");
        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_tokens, Some(1000));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.frequency_penalty, Some(0.1));
        assert_eq!(model.presence_penalty, Some(0.2));
        assert_eq!(model.n, Some(3));
    }

    #[test]
    fn test_builder_order_independence() {
        let model1 = ChatTogether::new()
            .with_temperature(0.5)
            .with_model("model-a");

        let model2 = ChatTogether::new()
            .with_model("model-a")
            .with_temperature(0.5);

        assert_eq!(model1.model, model2.model);
        assert_eq!(model1.temperature, model2.temperature);
    }

    // ========== with_tool_choice Additional Tests ==========

    #[test]
    fn test_with_tool_choice_none_value() {
        let model = ChatTogether::new().with_tool_choice(None);
        assert!(model.tool_choice.is_none());
    }

    #[test]
    fn test_with_tool_choice_case_sensitive() {
        // "None" (capital N) should be treated as a function name, not keyword
        let model = ChatTogether::new().with_tool_choice(Some("None".to_string()));
        assert!(matches!(
            model.tool_choice,
            Some(ChatCompletionToolChoiceOption::Named(ref named)) if named.function.name == "None"
        ));
    }

    #[test]
    fn test_with_tool_choice_custom_function() {
        let model = ChatTogether::new().with_tool_choice(Some("calculate_sum".to_string()));
        assert!(matches!(
            model.tool_choice,
            Some(ChatCompletionToolChoiceOption::Named(ref named)) if named.function.name == "calculate_sum"
        ));
    }

    // ========== Clone Trait Tests ==========

    #[test]
    fn test_clone_preserves_model() {
        let model = ChatTogether::new().with_model("test-model");
        let cloned = model.clone();
        assert_eq!(cloned.model, "test-model");
    }

    #[test]
    fn test_clone_preserves_temperature() {
        let model = ChatTogether::new().with_temperature(0.5);
        let cloned = model.clone();
        assert_eq!(cloned.temperature, Some(0.5));
    }

    #[test]
    fn test_clone_preserves_all_fields() {
        let model = ChatTogether::new()
            .with_model("test")
            .with_temperature(0.5)
            .with_max_tokens(100)
            .with_top_p(0.9)
            .with_frequency_penalty(0.1)
            .with_presence_penalty(0.2)
            .with_n(2);

        let cloned = model.clone();
        assert_eq!(cloned.model, model.model);
        assert_eq!(cloned.temperature, model.temperature);
        assert_eq!(cloned.max_tokens, model.max_tokens);
        assert_eq!(cloned.top_p, model.top_p);
        assert_eq!(cloned.frequency_penalty, model.frequency_penalty);
        assert_eq!(cloned.presence_penalty, model.presence_penalty);
        assert_eq!(cloned.n, model.n);
    }

    #[test]
    fn test_clone_is_independent() {
        let model = ChatTogether::new().with_model("original");
        let mut cloned = model.clone();
        cloned.model = "modified".to_string();
        assert_eq!(model.model, "original");
        assert_eq!(cloned.model, "modified");
    }

    // ========== Debug Trait Tests ==========

    #[test]
    fn test_debug_output_contains_model() {
        let model = ChatTogether::new().with_model("debug-test");
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_debug_output_contains_temperature() {
        let model = ChatTogether::new().with_temperature(0.7);
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("0.7"));
    }

    // ========== convert_image_source Additional Tests ==========

    #[test]
    fn test_convert_image_source_url_with_query_params() {
        let source = ImageSource::Url {
            url: "https://example.com/image.png?size=large&format=rgb".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert_eq!(
            result.url,
            "https://example.com/image.png?size=large&format=rgb"
        );
    }

    #[test]
    fn test_convert_image_source_base64_jpeg() {
        let source = ImageSource::Base64 {
            media_type: "image/jpeg".to_string(),
            data: "dGVzdA==".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert!(result.url.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn test_convert_image_source_base64_gif() {
        let source = ImageSource::Base64 {
            media_type: "image/gif".to_string(),
            data: "R0lGODlh".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert!(result.url.starts_with("data:image/gif;base64,"));
    }

    #[test]
    fn test_convert_image_source_base64_webp() {
        let source = ImageSource::Base64 {
            media_type: "image/webp".to_string(),
            data: "UklGRg==".to_string(),
        };
        let result = convert_image_source(&source, None);
        assert!(result.url.starts_with("data:image/webp;base64,"));
    }

    // ========== convert_content Additional Tests ==========

    #[test]
    fn test_convert_content_empty_text() {
        let content = MessageContent::Text(String::new());
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s.is_empty()
        ));
    }

    #[test]
    fn test_convert_content_multiline_text() {
        let content = MessageContent::Text("Line 1\nLine 2\nLine 3".to_string());
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s.contains('\n')
        ));
    }

    #[test]
    fn test_convert_content_unicode_text() {
        let content = MessageContent::Text("Hello 世界 🌍".to_string());
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Text(ref s) if s == "Hello 世界 🌍"
        ));
    }

    #[test]
    fn test_convert_content_empty_blocks() {
        let content = MessageContent::Blocks(vec![]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Array(ref arr) if arr.is_empty()
        ));
    }

    #[test]
    fn test_convert_content_all_empty_text_blocks() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: String::new(),
            },
            ContentBlock::Text {
                text: String::new(),
            },
        ]);
        let result = convert_content(&content);
        // All empty blocks are filtered, resulting in empty array
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Array(ref arr) if arr.is_empty()
        ));
    }

    #[test]
    fn test_convert_content_with_image_block() {
        let content = MessageContent::Blocks(vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/img.png".to_string(),
            },
            detail: None,
        }]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Array(ref arr) if arr.len() == 1
        ));
    }

    #[test]
    fn test_convert_content_mixed_text_and_image() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Describe this image:".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/img.png".to_string(),
                },
                detail: Some(ImageDetail::High),
            },
        ]);
        let result = convert_content(&content);
        assert!(matches!(
            result,
            ChatCompletionRequestUserMessageContent::Array(ref arr) if arr.len() == 2
        ));
    }

    // ========== convert_tool_definition Additional Tests ==========

    #[test]
    fn test_convert_tool_definition_with_complex_params() {
        let tool = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A tool with complex parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "integer"},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["name"]
            }),
        };
        let result = convert_tool_definition(&tool);
        assert_eq!(result.function.name, "complex_tool");
        assert!(result.function.parameters.is_some());
    }

    #[test]
    fn test_convert_tool_definition_whitespace_description() {
        let tool = ToolDefinition {
            name: "tool".to_string(),
            description: "   ".to_string(), // Just whitespace
            parameters: serde_json::json!({}),
        };
        let result = convert_tool_definition(&tool);
        // Non-empty string (whitespace) is kept as description
        assert_eq!(result.function.description, Some("   ".to_string()));
    }

    // ========== convert_tool_choice Additional Tests ==========

    #[test]
    fn test_convert_tool_choice_specific_unicode() {
        let choice = ToolChoice::Specific("获取天气".to_string());
        let result = convert_tool_choice(&choice);
        assert!(matches!(
            result,
            ChatCompletionToolChoiceOption::Named(ref named) if named.function.name == "获取天气"
        ));
    }

    // ========== convert_message Additional Tests ==========

    #[test]
    fn test_convert_message_system_long_content() {
        let long_content = "x".repeat(10000);
        let msg = Message::system(long_content);
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::System(_)));
    }

    #[test]
    fn test_convert_message_human_unicode() {
        let msg = Message::human("你好世界！🌍");
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::User(_)));
    }

    #[test]
    fn test_convert_message_ai_with_tool_calls() {
        let msg = Message::AI {
            content: MessageContent::Text("I'll help you.".to_string()),
            tool_calls: vec![ToolCall {
                id: "call_123".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"location": "NYC"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::Assistant(_)));
    }

    #[test]
    fn test_convert_message_ai_empty_content() {
        let msg = Message::AI {
            content: MessageContent::Text(String::new()),
            tool_calls: vec![],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        };
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::Assistant(_)));
    }

    #[test]
    fn test_convert_message_tool_long_content() {
        let long_result = serde_json::json!({"data": "x".repeat(5000)}).to_string();
        let msg = Message::tool(long_result, "call_456");
        let result = convert_message(&msg).unwrap();
        assert!(matches!(result, ChatCompletionRequestMessage::Tool(_)));
    }

    // ========== identifying_params Additional Tests ==========

    #[test]
    fn test_identifying_params_without_options() {
        let model = ChatTogether::new();
        let params = model.identifying_params();
        assert!(!params.contains_key("temperature"));
        assert!(!params.contains_key("max_tokens"));
        assert!(!params.contains_key("top_p"));
    }

    #[test]
    fn test_identifying_params_provider_always_together() {
        let model = ChatTogether::new().with_model("any-model");
        let params = model.identifying_params();
        assert_eq!(params.get("provider").unwrap(), "together");
    }

    #[test]
    fn test_identifying_params_with_exact_temperature() {
        let model = ChatTogether::new().with_temperature(0.5); // Exactly representable
        let params = model.identifying_params();
        let temp = params.get("temperature").unwrap().as_f64().unwrap();
        assert_eq!(temp, 0.5);
    }

    // ========== Serializable Additional Tests ==========

    #[test]
    fn test_to_json_includes_frequency_penalty() {
        let model = ChatTogether::new().with_frequency_penalty(0.5);
        let json = model.to_json();
        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                let fp = kwargs.get("frequency_penalty").unwrap().as_f64().unwrap();
                assert_eq!(fp, 0.5);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_includes_presence_penalty() {
        let model = ChatTogether::new().with_presence_penalty(-0.5);
        let json = model.to_json();
        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                let pp = kwargs.get("presence_penalty").unwrap().as_f64().unwrap();
                assert_eq!(pp, -0.5);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_includes_top_p() {
        let model = ChatTogether::new().with_top_p(0.95);
        let json = model.to_json();
        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                let tp = kwargs.get("top_p").unwrap().as_f64().unwrap();
                assert!((tp - 0.95).abs() < 0.01);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_lc_version() {
        let model = ChatTogether::new();
        let json = model.to_json();
        match json {
            SerializedObject::Constructor { lc, .. } => {
                assert_eq!(lc, SERIALIZATION_VERSION);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_lc_id_length() {
        let model = ChatTogether::new();
        assert_eq!(model.lc_id().len(), 4);
    }

    // ========== llm_type Tests ==========

    #[test]
    fn test_llm_type_is_static() {
        let model1 = ChatTogether::new();
        let model2 = ChatTogether::new().with_model("different");
        assert_eq!(model1.llm_type(), model2.llm_type());
    }

    #[test]
    fn test_llm_type_value() {
        let model = ChatTogether::new();
        assert_eq!(model.llm_type(), "together-chat");
    }

    // ========== as_any Tests ==========

    #[test]
    fn test_as_any_returns_self() {
        let model = ChatTogether::new();
        let any_ref = model.as_any();
        assert!(any_ref.downcast_ref::<ChatTogether>().is_some());
    }

    #[test]
    fn test_as_any_downcast_preserves_model() {
        let model = ChatTogether::new().with_model("test-model");
        let any_ref = model.as_any();
        let downcasted = any_ref.downcast_ref::<ChatTogether>().unwrap();
        assert_eq!(downcasted.model, "test-model");
    }

    // ========== Deprecated with_tools Tests ==========

    #[test]
    #[allow(deprecated)]
    fn test_with_tools_empty_array() {
        let model = ChatTogether::new().with_tools(vec![]);
        assert!(model.tools.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn test_with_tools_valid_tool() {
        let tool = serde_json::json!({
            "name": "get_weather",
            "description": "Get weather info",
            "parameters": {"type": "object"}
        });
        let model = ChatTogether::new().with_tools(vec![tool]);
        assert!(model.tools.is_some());
        assert_eq!(model.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_with_tools_invalid_tool_filtered() {
        // Tool without "name" field should be filtered out
        let invalid_tool = serde_json::json!({
            "description": "No name field"
        });
        let model = ChatTogether::new().with_tools(vec![invalid_tool]);
        assert!(model.tools.is_none()); // Empty after filtering
    }

    #[test]
    #[allow(deprecated)]
    fn test_with_tools_mixed_valid_invalid() {
        let valid_tool = serde_json::json!({
            "name": "valid_tool",
            "description": "Valid"
        });
        let invalid_tool = serde_json::json!({
            "no_name": true
        });
        let model = ChatTogether::new().with_tools(vec![valid_tool, invalid_tool]);
        assert!(model.tools.is_some());
        assert_eq!(model.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_with_tools_optional_description() {
        let tool = serde_json::json!({
            "name": "minimal_tool"
        });
        let model = ChatTogether::new().with_tools(vec![tool]);
        assert!(model.tools.is_some());
        let tools = model.tools.unwrap();
        assert!(tools[0].function.description.is_none());
    }

    // ========== Edge Cases and Boundary Tests ==========

    #[test]
    fn test_empty_model_name() {
        let model = ChatTogether::new().with_model("");
        assert_eq!(model.model, "");
    }

    #[test]
    fn test_very_long_model_name() {
        let long_name = "a".repeat(1000);
        let model = ChatTogether::new().with_model(&long_name);
        assert_eq!(model.model, long_name);
    }

    #[test]
    fn test_special_characters_in_model_name() {
        let model = ChatTogether::new().with_model("org/model-v1.0_test");
        assert_eq!(model.model, "org/model-v1.0_test");
    }

    #[test]
    fn test_temperature_near_zero() {
        let model = ChatTogether::new().with_temperature(0.001);
        assert_eq!(model.temperature, Some(0.001));
    }

    #[test]
    fn test_multiple_model_changes() {
        let model = ChatTogether::new()
            .with_model("first")
            .with_model("second")
            .with_model("third");
        assert_eq!(model.model, "third");
    }

    #[test]
    fn test_multiple_temperature_changes() {
        let model = ChatTogether::new()
            .with_temperature(0.1)
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.temperature, Some(0.9));
    }

    // ========== Integration-style Unit Tests ==========

    #[test]
    fn test_full_configuration() {
        let model = ChatTogether::new()
            .with_model("meta-llama/Llama-3-70b-chat-hf")
            .with_temperature(0.7)
            .with_max_tokens(4096)
            .with_top_p(0.95)
            .with_frequency_penalty(0.0)
            .with_presence_penalty(0.0)
            .with_n(1)
            .with_tool_choice(Some("auto".to_string()));

        // Verify all settings
        assert_eq!(model.model, "meta-llama/Llama-3-70b-chat-hf");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(4096));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.frequency_penalty, Some(0.0));
        assert_eq!(model.presence_penalty, Some(0.0));
        assert_eq!(model.n, Some(1));
        assert!(matches!(
            model.tool_choice,
            Some(ChatCompletionToolChoiceOption::Auto)
        ));
    }

    #[test]
    fn test_serialization_roundtrip_fields() {
        let model = ChatTogether::new()
            .with_model("test-model")
            .with_temperature(0.5)
            .with_max_tokens(100);

        let json = model.to_json();
        let params = model.identifying_params();

        // Verify JSON and params are consistent
        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                assert_eq!(kwargs.get("model").unwrap(), "test-model");
                assert_eq!(params.get("model_name").unwrap(), "test-model");
            }
            _ => panic!("Expected Constructor"),
        }
    }
}
