//! Ollama chat model implementation

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{
        AIMessage, AIMessageChunk, BaseMessage, ContentBlock, ImageSource, Message, MessageContent,
    },
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use futures::Stream;
use futures::StreamExt;
use ollama_rs::{
    generation::{
        chat::{request::ChatMessageRequest, ChatMessage},
        images::Image,
        tools::ToolCall as OllamaToolCall,
    },
    models::ModelOptions,
    Ollama,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Ollama chat model configuration and client
///
/// Provides access to local LLM models via Ollama, enabling inference
/// without external API dependencies.
///
/// # Multi-Modal Support
///
/// `ChatOllama` supports vision models (like `llava`) that can process images
/// alongside text. Images must be provided as base64-encoded data via
/// `MessageContent::Blocks` with `ContentBlock::Image`. URL images are not
/// supported directly by Ollama - they must be downloaded and converted to
/// base64 first.
///
/// # Example
/// ```no_run
/// use dashflow_ollama::ChatOllama;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatOllama::with_base_url("http://localhost:11434")
///         .with_model("llama2")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Vision Model Example
/// ```no_run
/// use dashflow_ollama::ChatOllama;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::{ContentBlock, ImageSource, Message, MessageContent};
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatOllama::with_base_url("http://localhost:11434").with_model("llava");
///
///     let content = MessageContent::Blocks(vec![
///         ContentBlock::Text {
///             text: "What's in this image?".to_string(),
///         },
///         ContentBlock::Image {
///             source: ImageSource::Base64 {
///                 media_type: "image/png".to_string(),
///                 data: "iVBORw0KGgoAAAANS...".to_string(),
///             },
///             detail: None,
///         },
///     ]);
///
///     let messages = vec![Message::Human {
///         content,
///         fields: Default::default(),
///     }];
///
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct ChatOllama {
    /// Ollama client
    client: Arc<Ollama>,

    /// Model name (e.g., "llama2", "mistral", "phi")
    model: String,

    /// Sampling temperature (0.0 to 2.0)
    temperature: Option<f32>,

    /// Maximum tokens to generate
    num_predict: Option<i32>,

    /// Top-p sampling parameter
    top_p: Option<f32>,

    /// Top-k sampling parameter
    top_k: Option<u32>,

    /// Repeat penalty
    repeat_penalty: Option<f32>,

    /// Seed for deterministic generation
    seed: Option<i32>,

    /// Context window size
    num_ctx: Option<u64>,

    /// Tools available for the model to call (OpenAI-format JSON)
    tools: Option<Vec<serde_json::Value>>,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl ChatOllama {
    /// Create a new `ChatOllama` instance with default settings
    ///
    /// Connects to Ollama at <http://localhost:11434> by default
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_ollama::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url("http://localhost:11434")
    }

    /// Create a new `ChatOllama` instance with custom base URL
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::ChatOllama;
    ///
    /// let model = ChatOllama::with_base_url("http://localhost:11434")
    ///     .with_model("llama2");
    /// ```
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let ollama = Ollama::new(base_url.into(), 11434);
        Self {
            client: Arc::new(ollama),
            model: "llama2".to_string(),
            temperature: None,
            num_predict: None,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            seed: None,
            num_ctx: None,
            tools: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the model name
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::ChatOllama;
    ///
    /// let model = ChatOllama::with_base_url("http://localhost:11434")
    ///     .with_model("mistral");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the temperature
    ///
    /// Higher values make output more random, lower values more deterministic.
    /// Default: 0.8
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum tokens to generate
    ///
    /// -1 means no limit (default)
    #[must_use]
    pub fn with_num_predict(mut self, num_predict: i32) -> Self {
        self.num_predict = Some(num_predict);
        self
    }

    /// Set the top-p parameter
    ///
    /// Controls diversity via nucleus sampling.
    /// Default: 0.9
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set the top-k parameter
    ///
    /// Controls diversity by limiting to top k tokens.
    /// Default: 40
    #[must_use]
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set the repeat penalty
    ///
    /// Penalizes repetition. Higher values reduce repetition.
    /// Default: 1.1
    #[must_use]
    pub fn with_repeat_penalty(mut self, penalty: f32) -> Self {
        self.repeat_penalty = Some(penalty);
        self
    }

    /// Set the seed for deterministic generation
    ///
    /// Same seed with same inputs produces same outputs.
    #[must_use]
    pub fn with_seed(mut self, seed: i32) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set the context window size
    ///
    /// Number of tokens the model can consider.
    /// Default: 2048
    #[must_use]
    pub fn with_num_ctx(mut self, num_ctx: u64) -> Self {
        self.num_ctx = Some(num_ctx);
        self
    }

    /// Bind tools to this chat model
    ///
    /// Tools should be in `OpenAI` function calling format.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::ChatOllama;
    /// use serde_json::json;
    ///
    /// let tool = json!({
    ///     "type": "function",
    ///     "function": {
    ///         "name": "get_weather",
    ///         "description": "Get the weather for a location",
    ///         "parameters": {
    ///             "type": "object",
    ///             "properties": {
    ///                 "location": {"type": "string"}
    ///             },
    ///             "required": ["location"]
    ///         }
    ///     }
    /// });
    ///
    /// let model = ChatOllama::with_base_url("http://localhost:11434")
    ///     .with_model("llama2")
    ///     .with_tools(vec![tool]);
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
        self.tools = Some(tools);
        self
    }

    /// Set a rate limiter to control request rate
    ///
    /// Rate limiting is applied transparently in `generate()` and `stream()` methods.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_ollama::ChatOllama;
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
    /// let model = ChatOllama::with_base_url("http://localhost:11434")
    ///     .with_model("llama2")
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Build `ModelOptions` from configured parameters
    fn build_options(&self, stop: Option<&[String]>) -> Option<ModelOptions> {
        let mut has_options = false;
        let mut options = ModelOptions::default();

        if let Some(temp) = self.temperature {
            options = options.temperature(temp);
            has_options = true;
        }
        if let Some(num_predict) = self.num_predict {
            options = options.num_predict(num_predict);
            has_options = true;
        }
        if let Some(top_p) = self.top_p {
            options = options.top_p(top_p);
            has_options = true;
        }
        if let Some(top_k) = self.top_k {
            options = options.top_k(top_k);
            has_options = true;
        }
        if let Some(repeat_penalty) = self.repeat_penalty {
            options = options.repeat_penalty(repeat_penalty);
            has_options = true;
        }
        if let Some(seed) = self.seed {
            options = options.seed(seed);
            has_options = true;
        }
        if let Some(num_ctx) = self.num_ctx {
            options = options.num_ctx(num_ctx);
            has_options = true;
        }
        if let Some(stop_sequences) = stop {
            options = options.stop(stop_sequences.to_vec());
            has_options = true;
        }

        if has_options {
            Some(options)
        } else {
            None
        }
    }

    /// Create a `ChatOllama` instance from a configuration
    ///
    /// This method constructs a `ChatOllama` model from a `ChatModelConfig::Ollama` variant,
    /// applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be Ollama variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatOllama` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is not an Ollama variant.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow::core::config_loader::ChatModelConfig;
    /// use dashflow_ollama::ChatOllama;
    ///
    /// let config = ChatModelConfig::Ollama {
    ///     model: "llama3.2".to_string(),
    ///     base_url: "http://localhost:11434".to_string(),
    ///     temperature: Some(0.7),
    /// };
    ///
    /// let chat_model = ChatOllama::from_config(&config).unwrap();
    /// ```
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::Ollama {
                model,
                base_url,
                temperature,
            } => {
                // Create the ChatOllama instance
                let mut chat_model = Self::with_base_url(base_url).with_model(model);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(*temp);
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected Ollama config, got {} config",
                config.provider()
            ))),
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatOllama {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert `DashFlow` `ImageSource` to Ollama Image (base64 only)
///
/// Ollama API only accepts base64-encoded images. URLs and data URIs are not supported directly.
fn convert_image_source(source: &ImageSource) -> Result<Image> {
    match source {
        ImageSource::Base64 { data, .. } => {
            // Ollama expects raw base64 string without data URI prefix
            Ok(Image::from_base64(data))
        }
        ImageSource::Url { url } => {
            // Ollama doesn't support URLs directly - they need to be downloaded and converted to base64
            // For now, return an error. Future enhancement: download and convert automatically
            Err(Error::invalid_input(format!(
                "Ollama does not support image URLs directly. Please download '{url}' and convert to base64."
            )))
        }
    }
}

/// Extract images from message content blocks
fn extract_images(content: &MessageContent) -> Result<Vec<Image>> {
    match content {
        MessageContent::Text(_) => Ok(Vec::new()),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Image { source, .. } = block {
                    Some(convert_image_source(source))
                } else {
                    None
                }
            })
            .collect(),
    }
}

/// Helper struct to deserialize `ToolCallFunction` fields
#[derive(Deserialize)]
struct ToolCallFunctionHelper {
    name: String,
    arguments: serde_json::Value,
}

/// Convert Ollama `ToolCall` to `DashFlow` `ToolCall`
///
/// Generates a unique ID for each tool call using UUID
fn convert_tool_call(
    ollama_tool_call: &OllamaToolCall,
) -> Result<dashflow::core::messages::ToolCall> {
    // Serialize and deserialize to access private fields
    let function_json = serde_json::to_value(&ollama_tool_call.function)
        .map_err(|e| Error::invalid_input(format!("Failed to serialize tool call: {e}")))?;

    let helper: ToolCallFunctionHelper = serde_json::from_value(function_json)
        .map_err(|e| Error::invalid_input(format!("Failed to deserialize tool call: {e}")))?;

    Ok(dashflow::core::messages::ToolCall {
        id: uuid::Uuid::new_v4().to_string(),
        name: helper.name,
        args: helper.arguments,
        tool_type: "tool_call".to_string(),
        index: None,
    })
}

/// Convert `DashFlow` `ToolDefinition` to Ollama `ToolInfo`
fn convert_tool_definition(
    tool: &ToolDefinition,
) -> Result<ollama_rs::generation::tools::ToolInfo> {
    // Note: ollama-rs v0.3.2 ToolType uses PascalCase ("Function" not "function")
    let json = serde_json::json!({
        "type": "Function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters
        }
    });

    serde_json::from_value(json)
        .map_err(|e| Error::invalid_input(format!("Failed to convert tool definition: {e}")))
}

/// Convert `DashFlow` Message to Ollama `ChatMessage`
fn convert_message(message: &Message) -> Result<ChatMessage> {
    match message {
        Message::System { content, .. } => Ok(ChatMessage::system(content.as_text())),
        Message::Human { content, .. } => {
            let text = content.as_text();
            let mut chat_message = ChatMessage::user(text);

            // Add images if present
            let images = extract_images(content)?;
            if !images.is_empty() {
                chat_message = chat_message.with_images(images);
            }

            Ok(chat_message)
        }
        Message::AI {
            content,
            tool_calls,
            ..
        } => {
            // Note: Tool calls are handled by ollama-rs v0.3.2+ native support
            // For message history, we include tool calls in the text representation
            let text = if tool_calls.is_empty() {
                content.as_text()
            } else {
                // Include tool calls in the content for now
                format!("{}\n\nTool calls: {:?}", content.as_text(), tool_calls)
            };
            Ok(ChatMessage::assistant(text))
        }
        Message::Tool {
            content,
            tool_call_id,
            ..
        } => {
            // Ollama uses "tool" role for tool responses
            // Include tool_call_id in the content
            let text = format!("Tool {} result: {}", tool_call_id, content.as_text());
            Ok(ChatMessage::tool(text))
        }
        Message::Function { content, name, .. } => {
            // Convert function message to tool message
            let text = format!("Function {} result: {}", name, content.as_text());
            Ok(ChatMessage::tool(text))
        }
    }
}

#[async_trait]
impl ChatModel for ChatOllama {
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Ollama> cloned for retry closure
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

        // Tools: prefer parameter over struct field
        // Convert to ToolInfo (ollama-rs v0.3.2+ native support)
        let ollama_tools: Option<Vec<ollama_rs::generation::tools::ToolInfo>> =
            if let Some(tool_defs) = tools {
                Some(
                    tool_defs
                        .iter()
                        .map(convert_tool_definition)
                        .collect::<Result<Vec<_>>>()?,
                )
            } else if let Some(ref json_tools) = self.tools {
                Some(
                    json_tools
                        .iter()
                        .map(|json| {
                            serde_json::from_value(json.clone()).map_err(|e| {
                                Error::invalid_input(format!("Failed to parse tool: {e}"))
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                )
            } else {
                None
            };

        // Log warning if tool_choice is specified (Ollama doesn't support this parameter)
        if tool_choice.is_some() {
            tracing::warn!(
                "Ollama API does not support tool_choice parameter. Ignoring tool_choice setting."
            );
        }

        // Convert messages
        let ollama_messages: Vec<ChatMessage> = messages
            .iter()
            .map(convert_message)
            .collect::<Result<Vec<_>>>()?;

        // Build request
        let mut request = ChatMessageRequest::new(self.model.clone(), ollama_messages);

        if let Some(options) = self.build_options(stop) {
            request = request.options(options);
        }

        // Add tools if present (ollama-rs v0.3.2+ native support)
        if let Some(tools) = ollama_tools {
            request = request.tools(tools);
        }

        // Make API call with retry
        let client = self.client.clone();
        let response_result = with_retry(&self.retry_policy, move || {
            let client = client.clone();
            let request = request.clone();
            async move {
                client
                    .send_chat_messages(request)
                    .await
                    .map_err(|e| Error::api(format!("Ollama API error: {e}")))
            }
        })
        .await;

        // Handle error callback
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(manager) = run_manager {
                    if let Err(cb_err) = manager.on_llm_error(&e.to_string(), run_id, None).await {
                        tracing::warn!("Failed to send LLM error callback: {}", cb_err);
                    }
                }
                return Err(e);
            }
        };

        // Convert response to ChatResult
        let content = response.message.content.clone();

        // Extract tool calls if present
        let tool_calls: Vec<dashflow::core::messages::ToolCall> = response
            .message
            .tool_calls
            .iter()
            .map(convert_tool_call)
            .collect::<Result<Vec<_>>>()?;

        // Create AI message with tool calls
        let mut message = AIMessage::new(content);
        if !tool_calls.is_empty() {
            message = message.with_tool_calls(tool_calls);
        }

        // Build generation info with timing/token data if available
        let mut generation_info = HashMap::new();
        if let Some(final_data) = response.final_data {
            generation_info.insert(
                "prompt_eval_count".to_string(),
                serde_json::json!(final_data.prompt_eval_count),
            );
            generation_info.insert(
                "eval_count".to_string(),
                serde_json::json!(final_data.eval_count),
            );
            generation_info.insert(
                "total_duration".to_string(),
                serde_json::json!(final_data.total_duration),
            );
        }

        let generation = if generation_info.is_empty() {
            ChatGeneration::new(message.into())
        } else {
            ChatGeneration::with_info(message.into(), generation_info)
        };

        let chat_result = ChatResult::new(generation);

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
        // Note: Ollama API does not support streaming with tools
        // See ChatMessageRequest: stream field comment says "Must be false if tools are provided"
        // Tools are checked but ignored in streaming mode
        if tools.is_some() || self.tools.is_some() {
            tracing::warn!("Ollama API does not support streaming with tools. Tools will be ignored in streaming mode.");
        }

        // Log warning if tool_choice is specified (Ollama doesn't support this parameter)
        if tool_choice.is_some() {
            tracing::warn!(
                "Ollama API does not support tool_choice parameter. Ignoring tool_choice setting."
            );
        }

        // Convert messages
        let ollama_messages: Vec<ChatMessage> = messages
            .iter()
            .map(convert_message)
            .collect::<Result<Vec<_>>>()?;

        // Build request
        let mut request = ChatMessageRequest::new(self.model.clone(), ollama_messages);

        if let Some(options) = self.build_options(stop) {
            request = request.options(options);
        }

        // Get streaming response
        let mut response_stream = self
            .client
            .send_chat_messages_stream(request)
            .await
            .map_err(|e| Error::api(format!("Ollama streaming error: {e}")))?;

        // Convert Ollama stream to DashFlow stream
        let s = stream! {
            while let Some(result) = response_stream.next().await {
                if let Ok(response) = result {
                    let content = response.message.content.clone();

                    // Extract tool calls if present
                    let tool_calls_result: Result<Vec<dashflow::core::messages::ToolCall>> =
                        response.message.tool_calls
                            .iter()
                            .map(convert_tool_call)
                            .collect();

                    match tool_calls_result {
                        Ok(tool_calls) => {
                            let mut chunk = AIMessageChunk::new(content);
                            chunk.tool_calls = tool_calls;
                            yield Ok(ChatGenerationChunk::new(chunk));
                        }
                        Err(e) => {
                            yield Err(e);
                            break;
                        }
                    }
                } else {
                    yield Err(Error::api("Error in Ollama stream".to_string()));
                    break;
                }
            }
        };

        Ok(Box::pin(s))
    }

    fn llm_type(&self) -> &'static str {
        "ollama"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.rate_limiter.clone()
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
        if let Some(num_predict) = self.num_predict {
            params.insert("num_predict".to_string(), serde_json::json!(num_predict));
        }
        params
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::redundant_clone
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_constructor() {
        let model = ChatOllama::with_base_url("http://localhost:11434");
        assert_eq!(model.model, "llama2");
        assert_eq!(model.llm_type(), "ollama");
    }

    #[test]
    fn test_with_model() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_model("mistral");
        assert_eq!(model.model, "mistral");
    }

    #[test]
    fn test_with_temperature() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_temperature(0.5);
        assert_eq!(model.temperature, Some(0.5));
    }

    #[test]
    fn test_with_num_predict() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_num_predict(100);
        assert_eq!(model.num_predict, Some(100));
    }

    #[test]
    fn test_builder_chaining() {
        let model = ChatOllama::with_base_url("http://localhost:11434")
            .with_model("phi")
            .with_temperature(0.3)
            .with_top_p(0.95)
            .with_seed(42);

        assert_eq!(model.model, "phi");
        assert_eq!(model.temperature, Some(0.3));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.seed, Some(42));
    }

    #[test]
    fn test_identifying_params() {
        let model = ChatOllama::with_base_url("http://localhost:11434")
            .with_model("llama2")
            .with_temperature(0.7);

        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), &serde_json::json!("llama2"));
        // Check temperature is present and close to expected value (floating point precision)
        let temp = params.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_with_base_url() {
        let model = ChatOllama::with_base_url("http://custom-host:8080");
        assert_eq!(model.model, "llama2");
    }

    #[test]
    fn test_build_options_none_when_unset() {
        let model = ChatOllama::with_base_url("http://localhost:11434");
        assert!(model.build_options(None).is_none());
    }

    #[test]
    fn test_build_options_temperature_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_temperature(0.7);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        let temp = json.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
        assert!(json.get("num_predict").is_none());
    }

    #[test]
    fn test_build_options_num_predict_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_num_predict(123);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        assert_eq!(json.get("num_predict").unwrap(), &serde_json::json!(123));
        assert!(json.get("temperature").is_none());
    }

    #[test]
    fn test_build_options_top_p_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_top_p(0.9);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        let top_p = json.get("top_p").unwrap().as_f64().unwrap();
        assert!((top_p - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_build_options_top_k_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_top_k(42);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        assert_eq!(json.get("top_k").unwrap(), &serde_json::json!(42));
    }

    #[test]
    fn test_build_options_repeat_penalty_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_repeat_penalty(1.23);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        let penalty = json.get("repeat_penalty").unwrap().as_f64().unwrap();
        assert!((penalty - 1.23).abs() < 0.001);
    }

    #[test]
    fn test_build_options_seed_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_seed(7);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        assert_eq!(json.get("seed").unwrap(), &serde_json::json!(7));
    }

    #[test]
    fn test_build_options_num_ctx_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434").with_num_ctx(4096);
        let options = model.build_options(None).unwrap();
        let json = serde_json::to_value(options).unwrap();
        assert_eq!(json.get("num_ctx").unwrap(), &serde_json::json!(4096));
    }

    #[test]
    fn test_build_options_stop_sequences_only() {
        let model = ChatOllama::with_base_url("http://localhost:11434");
        let stop = vec!["STOP".to_string(), "END".to_string()];
        let options = model.build_options(Some(&stop)).unwrap();
        let json = serde_json::to_value(options).unwrap();
        assert_eq!(
            json.get("stop").unwrap(),
            &serde_json::json!(["STOP", "END"])
        );
    }

    #[test]
    fn test_convert_image_source_url_error_includes_url() {
        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };
        let err = convert_image_source(&source).unwrap_err().to_string();
        assert!(err.contains("Ollama does not support image URLs"));
        assert!(err.contains("https://example.com/image.png"));
    }

    #[test]
    fn test_extract_images_preserves_order() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "img1".to_string(),
                },
                detail: None,
            },
            ContentBlock::Text {
                text: "between".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "img2".to_string(),
                },
                detail: None,
            },
        ]);

        let images = extract_images(&content).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].to_base64(), "img1");
        assert_eq!(images[1].to_base64(), "img2");
    }

    #[test]
    fn test_extract_images_url_fails() {
        let content = MessageContent::Blocks(vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/image.png".to_string(),
            },
            detail: None,
        }]);

        let err = extract_images(&content).unwrap_err().to_string();
        assert!(err.contains("Ollama does not support image URLs"));
    }

    #[test]
    fn test_convert_message_system_text() {
        let message = Message::system("Be helpful.");
        let ollama_msg = convert_message(&message).unwrap();
        assert_eq!(
            ollama_msg.role,
            ollama_rs::generation::chat::MessageRole::System
        );
        assert_eq!(ollama_msg.content, "Be helpful.");
    }

    #[test]
    fn test_convert_message_ai_text_only() {
        let message = Message::ai("Hello from the assistant");
        let ollama_msg = convert_message(&message).unwrap();
        assert_eq!(
            ollama_msg.role,
            ollama_rs::generation::chat::MessageRole::Assistant
        );
        assert_eq!(ollama_msg.content, "Hello from the assistant");
    }

    #[test]
    fn test_convert_message_ai_includes_tool_calls_in_text() {
        use dashflow::core::messages::{BaseMessageFields, ToolCall};

        let message = Message::AI {
            content: MessageContent::Text("Planning...".to_string()),
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"location":"SF"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }],
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: BaseMessageFields::default(),
        };

        let ollama_msg = convert_message(&message).unwrap();
        assert!(ollama_msg.content.contains("Planning..."));
        assert!(ollama_msg.content.contains("Tool calls:"));
        assert!(ollama_msg.content.contains("get_weather"));
    }

    #[test]
    fn test_convert_message_tool_includes_tool_call_id() {
        let message = Message::tool("42", "tool_call_123");
        let ollama_msg = convert_message(&message).unwrap();
        assert_eq!(
            ollama_msg.role,
            ollama_rs::generation::chat::MessageRole::Tool
        );
        assert_eq!(ollama_msg.content, "Tool tool_call_123 result: 42");
    }

    #[test]
    fn test_convert_message_function_maps_to_tool_role() {
        use dashflow::core::messages::{BaseMessageFields, MessageContent};

        let message = Message::Function {
            content: MessageContent::Text("ok".to_string()),
            name: "calculate".to_string(),
            fields: BaseMessageFields::default(),
        };

        let ollama_msg = convert_message(&message).unwrap();
        assert_eq!(
            ollama_msg.role,
            ollama_rs::generation::chat::MessageRole::Tool
        );
        assert_eq!(ollama_msg.content, "Function calculate result: ok");
    }

    #[test]
    fn test_convert_tool_definition_to_tool_info() {
        let tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"location": {"type": "string"}},
                "required": ["location"]
            }),
        };

        let tool_info = convert_tool_definition(&tool).unwrap();
        let json = serde_json::to_value(&tool_info).unwrap();
        assert_eq!(json.get("type").unwrap(), &serde_json::json!("Function"));
        assert_eq!(
            json.pointer("/function/name").unwrap(),
            &serde_json::json!("get_weather")
        );
        assert_eq!(
            json.pointer("/function/description").unwrap(),
            &serde_json::json!("Get the weather")
        );
        assert_eq!(
            json.pointer("/function/parameters/type").unwrap(),
            &serde_json::json!("object")
        );
    }

    #[test]
    fn test_convert_tool_call_to_dashflow_tool_call() {
        let ollama_tool_call = OllamaToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: "get_weather".to_string(),
                arguments: serde_json::json!({"location":"SF"}),
            },
        };

        let tool_call = convert_tool_call(&ollama_tool_call).unwrap();
        assert_eq!(tool_call.name, "get_weather");
        assert_eq!(tool_call.args, serde_json::json!({"location":"SF"}));
        assert_eq!(tool_call.tool_type, "tool_call");
        assert!(tool_call.index.is_none());
        assert!(Uuid::parse_str(&tool_call.id).is_ok());
    }

    #[test]
    fn test_message_conversion_human_with_base64_image() {
        use dashflow::core::messages::{
            BaseMessageFields, ContentBlock, ImageDetail, ImageSource, MessageContent,
        };

        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "What's in this image?".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgoAAAANS...".to_string(),
                },
                detail: Some(ImageDetail::High),
            },
        ]);

        let message = Message::Human {
            content,
            fields: BaseMessageFields::default(),
        };

        let result = convert_message(&message);
        assert!(result.is_ok());
        let ollama_msg = result.unwrap();
        assert_eq!(
            ollama_msg.role,
            ollama_rs::generation::chat::MessageRole::User
        );
        assert_eq!(ollama_msg.content, "What's in this image?");
        assert!(ollama_msg.images.is_some());
        assert_eq!(ollama_msg.images.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_message_conversion_human_image_only() {
        use dashflow::core::messages::{
            BaseMessageFields, ContentBlock, ImageSource, MessageContent,
        };

        let content = MessageContent::Blocks(vec![ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/jpeg".to_string(),
                data: "/9j/4AAQSkZJRg...".to_string(),
            },
            detail: None,
        }]);

        let message = Message::Human {
            content,
            fields: BaseMessageFields::default(),
        };

        let result = convert_message(&message);
        assert!(result.is_ok());
        let ollama_msg = result.unwrap();
        assert!(ollama_msg.images.is_some());
        assert_eq!(ollama_msg.images.as_ref().unwrap().len(), 1);
        assert_eq!(
            ollama_msg.images.as_ref().unwrap()[0].to_base64(),
            "/9j/4AAQSkZJRg..."
        );
    }

    #[test]
    fn test_message_conversion_human_multiple_images() {
        use dashflow::core::messages::{
            BaseMessageFields, ContentBlock, ImageSource, MessageContent,
        };

        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Compare these two images".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "image1_base64".to_string(),
                },
                detail: None,
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "image2_base64".to_string(),
                },
                detail: None,
            },
        ]);

        let message = Message::Human {
            content,
            fields: BaseMessageFields::default(),
        };

        let result = convert_message(&message);
        assert!(result.is_ok());
        let ollama_msg = result.unwrap();
        assert_eq!(ollama_msg.content, "Compare these two images");
        assert!(ollama_msg.images.is_some());
        assert_eq!(ollama_msg.images.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_message_conversion_human_text_only() {
        use dashflow::core::messages::{BaseMessageFields, MessageContent};

        let content = MessageContent::Text("Hello".to_string());
        let message = Message::Human {
            content,
            fields: BaseMessageFields::default(),
        };

        let result = convert_message(&message);
        assert!(result.is_ok());
        let ollama_msg = result.unwrap();
        assert_eq!(ollama_msg.content, "Hello");
        assert!(ollama_msg.images.is_none());
    }

    #[test]
    fn test_message_conversion_human_with_url_image_fails() {
        use dashflow::core::messages::{
            BaseMessageFields, ContentBlock, ImageSource, MessageContent,
        };

        let content = MessageContent::Blocks(vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/image.png".to_string(),
            },
            detail: None,
        }]);

        let message = Message::Human {
            content,
            fields: BaseMessageFields::default(),
        };

        let result = convert_message(&message);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Ollama does not support image URLs"));
    }

    #[test]
    fn test_extract_images_from_blocks() {
        use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "text".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "base64data".to_string(),
                },
                detail: None,
            },
        ]);

        let images = extract_images(&content).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].to_base64(), "base64data");
    }

    #[test]
    fn test_extract_images_from_text() {
        use dashflow::core::messages::MessageContent;

        let content = MessageContent::Text("just text".to_string());
        let images = extract_images(&content).unwrap();
        assert_eq!(images.len(), 0);
    }

    #[test]
    #[allow(deprecated)] // Testing deprecated with_tools() method
    fn test_with_tools() {
        use serde_json::json;

        let tool = json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        });

        #[allow(deprecated)] // Testing deprecated with_tools() method
        let model =
            ChatOllama::with_base_url("http://localhost:11434").with_tools(vec![tool.clone()]);
        assert!(model.tools.is_some());
        assert_eq!(model.tools.unwrap().len(), 1);
    }

    #[test]
    #[allow(deprecated)] // Testing deprecated with_tools() method
    fn test_with_multiple_tools() {
        use serde_json::json;

        let tool1 = json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                }
            }
        });

        let tool2 = json!({
            "type": "function",
            "function": {
                "name": "calculator",
                "description": "Perform calculations",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "expression": {"type": "string"}
                    }
                }
            }
        });

        let model =
            ChatOllama::with_base_url("http://localhost:11434").with_tools(vec![tool1, tool2]);
        assert!(model.tools.is_some());
        assert_eq!(model.tools.unwrap().len(), 2);
    }

    #[test]
    fn test_tool_format_validation() {
        use serde_json::json;

        // Valid tool - using ollama-rs v0.3.2 native ToolInfo
        // Note: ToolType expects "Function" with capital F
        let valid_tool = json!({
            "type": "Function",
            "function": {
                "name": "test",
                "description": "Test tool",
                "parameters": {"type": "object"}
            }
        });

        let _: ollama_rs::generation::tools::ToolInfo = serde_json::from_value(valid_tool).unwrap();

        // Invalid tool (missing function)
        let invalid_tool = json!({
            "type": "Function"
        });

        let result: std::result::Result<ollama_rs::generation::tools::ToolInfo, _> =
            serde_json::from_value(invalid_tool);
        assert!(result.is_err());
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

        let model = ChatOllama::with_base_url("http://localhost:11434")
            .with_model("llama2")
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

/// Standard conformance tests
///
/// These tests verify that ChatOllama behaves consistently with other
/// ChatModel implementations across the DashFlow ecosystem.
#[cfg(test)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::chat_model_tests::*;
    use dashflow_test_utils::init_test_env;

    /// Helper function to create a test model with standard settings
    ///
    /// Uses llama2 as default model for local testing
    fn create_test_model() -> ChatOllama {
        ChatOllama::with_base_url("http://localhost:11434")
            .with_model("llama2") // Common Ollama model
            .with_temperature(0.0) // Deterministic for testing
            .with_num_predict(100) // Limit tokens for speed
    }

    /// Standard Test 1: Basic invoke
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_invoke_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_invoke(&model).await;
    }

    /// Standard Test 2: Streaming
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_stream_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream(&model).await;
    }

    /// Standard Test 3: Batch processing
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_batch_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_batch(&model).await;
    }

    /// Standard Test 4: Multi-turn conversation
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_conversation(&model).await;
    }

    /// Standard Test 4b: Double messages conversation
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_double_messages_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_double_messages_conversation(&model).await;
    }

    /// Standard Test 4c: Message with name field
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_message_with_name_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_message_with_name(&model).await;
    }

    /// Standard Test 5: Stop sequences
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_stop_sequence_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_stop_sequence(&model).await;
    }

    /// Standard Test 6: Usage metadata
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_usage_metadata_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_usage_metadata(&model).await;
    }

    /// Standard Test 7: Empty messages
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_empty_messages_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_messages(&model).await;
    }

    /// Standard Test 8: Long conversation
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_long_conversation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_long_conversation(&model).await;
    }

    /// Standard Test 9: Special characters
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_special_characters_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_special_characters(&model).await;
    }

    /// Standard Test 10: Unicode and emoji
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_unicode_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_unicode(&model).await;
    }

    /// Standard Test 11: Tool calling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_tool_calling_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_tool_calling(&model).await;
    }

    /// Standard Test 12: Structured output
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_structured_output_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_structured_output(&model).await;
    }

    /// Standard Test 13: JSON mode
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_json_mode_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_json_mode(&model).await;
    }

    /// Standard Test 14: Usage metadata in streaming
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_usage_metadata_streaming_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_usage_metadata_streaming(&model).await;
    }

    /// Standard Test 15: System message handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_system_message_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_system_message(&model).await;
    }

    /// Standard Test 16: Empty content handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_empty_content_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_content(&model).await;
    }

    /// Standard Test 17: Large input handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_large_input_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_large_input(&model).await;
    }

    /// Standard Test 18: Concurrent generation
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_concurrent_generation_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_concurrent_generation(&model).await;
    }

    /// Standard Test 19: Error recovery
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_error_recovery_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_error_recovery(&model).await;
    }

    /// Standard Test 20: Response consistency
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_response_consistency_standard() {
        init_test_env().ok();
        let model = create_test_model();
        test_response_consistency(&model).await;
    }

    /// Standard Test 21: Tool calling with no arguments
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
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
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_stream_with_timeout_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_with_timeout(&model).await;
    }

    /// Comprehensive Test 2: Streaming interruption handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_stream_interruption_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_interruption(&model).await;
    }

    /// Comprehensive Test 3: Empty stream handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_stream_empty_response_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_stream_empty_response(&model).await;
    }

    /// Comprehensive Test 4: Multiple system messages
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_multiple_system_messages_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_multiple_system_messages(&model).await;
    }

    /// Comprehensive Test 5: Empty system message
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_empty_system_message_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_empty_system_message(&model).await;
    }

    /// Comprehensive Test 6: Temperature edge cases
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_temperature_extremes_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_temperature_extremes(&model).await;
    }

    /// Comprehensive Test 7: Max tokens enforcement
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_max_tokens_limit_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_max_tokens_limit(&model).await;
    }

    /// Comprehensive Test 8: Invalid stop sequences
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_invalid_stop_sequences_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_invalid_stop_sequences(&model).await;
    }

    /// Comprehensive Test 9: Context window overflow
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_context_window_overflow_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_context_window_overflow(&model).await;
    }

    /// Comprehensive Test 10: Rapid consecutive calls
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_rapid_consecutive_calls_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_rapid_consecutive_calls(&model).await;
    }

    /// Comprehensive Test 11: Network error handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_network_error_handling_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_network_error_handling(&model).await;
    }

    /// Comprehensive Test 12: Malformed input recovery
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_malformed_input_recovery_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_malformed_input_recovery(&model).await;
    }

    /// Comprehensive Test 13: Very long single message
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_very_long_single_message_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_very_long_single_message(&model).await;
    }

    /// Comprehensive Test 14: Response format consistency
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_response_format_consistency_comprehensive() {
        init_test_env().ok();
        let model = create_test_model();
        test_response_format_consistency(&model).await;
    }
}
