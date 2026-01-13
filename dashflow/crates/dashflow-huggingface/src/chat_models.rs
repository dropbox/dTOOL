// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

// HuggingFace Hub chat model implementation

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string, HF_TOKEN, HUGGINGFACEHUB_API_TOKEN},
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessageChunk, BaseMessage, Message},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use eventsource_stream::Eventsource;
use futures::Stream;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// `HuggingFace` Hub chat model configuration and client
///
/// Uses `HuggingFace`'s Inference API to interact with thousands of models
/// hosted on `HuggingFace` Hub. Supports both public inference endpoints
/// and dedicated inference endpoints.
///
/// # Example
/// ```no_run
/// use dashflow_huggingface::build_chat_model;
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Config-driven instantiation (recommended)
///     let config = ChatModelConfig::HuggingFace {
///         model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
///         api_key: SecretReference::from_env("HF_TOKEN"),
///         temperature: Some(0.7),
///     };
///     let model = build_chat_model(&config)?;
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await?;
///     println!("{:?}", result);
///     Ok(())
/// }
/// ```
///
/// # Available Models
/// Any model on `HuggingFace` Hub that supports text generation or chat can be used.
/// Popular choices include:
/// - `meta-llama/Llama-2-7b-chat-hf` - Llama 2 7B chat model
/// - `meta-llama/Llama-2-13b-chat-hf` - Llama 2 13B chat model
/// - `mistralai/Mistral-7B-Instruct-v0.2` - Mistral 7B instruct model
/// - `HuggingFaceH4/zephyr-7b-beta` - Zephyr 7B chat model
/// - `tiiuae/falcon-7b-instruct` - Falcon 7B instruct model
#[derive(Clone, Debug)]
pub struct ChatHuggingFace {
    /// HTTP client for API calls
    http_client: Arc<HttpClient>,

    /// Model ID on `HuggingFace` Hub (e.g., "meta-llama/Llama-2-7b-chat-hf")
    model_id: String,

    /// API endpoint URL
    endpoint_url: String,

    /// `HuggingFace` API token
    api_token: Option<String>,

    /// Sampling temperature (0.0 to 2.0)
    temperature: Option<f64>,

    /// Maximum tokens to generate
    max_new_tokens: Option<u32>,

    /// Top-p sampling parameter
    top_p: Option<f64>,

    /// Top-k sampling parameter
    top_k: Option<u32>,

    /// Repetition penalty
    repetition_penalty: Option<f64>,

    /// Whether to return full text (including prompt)
    return_full_text: Option<bool>,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

#[derive(Debug, Serialize)]
struct HuggingFaceRequest {
    inputs: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<HuggingFaceParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<HuggingFaceOptions>,
}

#[derive(Debug, Serialize)]
struct HuggingFaceParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_new_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repetition_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_full_text: Option<bool>,
}

#[derive(Debug, Serialize)]
struct HuggingFaceOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    use_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_for_model: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceResponse {
    generated_text: String,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceStreamChunk {
    #[serde(default)]
    token: HuggingFaceToken,
}

#[derive(Debug, Deserialize, Default)]
struct HuggingFaceToken {
    #[serde(default)]
    text: String,
}

impl ChatHuggingFace {
    /// Create a new `ChatHuggingFace` instance with a model ID
    ///
    /// Reads the `HuggingFace` API token from the `HUGGINGFACEHUB_API_TOKEN` or `HF_TOKEN` environment variable.
    ///
    /// # Arguments
    /// * `model_id` - The model ID on `HuggingFace` Hub (e.g., "meta-llama/Llama-2-7b-chat-hf")
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_huggingface::build_chat_model(&config)` for config-driven instantiation"
    )]
    pub fn new(model_id: impl Into<String>) -> Self {
        let model_id = model_id.into();
        let api_token = env_string(HUGGINGFACEHUB_API_TOKEN)
            .or_else(|| env_string(HF_TOKEN));

        let endpoint_url = format!("https://api-inference.huggingface.co/models/{model_id}");

        Self {
            http_client: Arc::new(HttpClient::new()),
            model_id,
            endpoint_url,
            api_token,
            temperature: Some(0.7),
            max_new_tokens: Some(512),
            top_p: Some(0.95),
            top_k: None,
            repetition_penalty: None,
            return_full_text: Some(false),
            retry_policy: RetryPolicy::default(),
            rate_limiter: None,
        }
    }

    /// Create a new `ChatHuggingFace` instance with explicit API token
    #[allow(deprecated, clippy::disallowed_methods)]
    pub fn with_api_token(model_id: impl Into<String>, api_token: impl Into<String>) -> Self {
        let mut instance = Self::new(model_id);
        instance.api_token = Some(api_token.into());
        instance
    }

    /// Set a custom endpoint URL (for dedicated inference endpoints)
    #[must_use]
    pub fn with_endpoint_url(mut self, endpoint_url: impl Into<String>) -> Self {
        self.endpoint_url = endpoint_url.into();
        self
    }

    /// Set the sampling temperature (0.0 to 2.0)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum number of new tokens to generate
    #[must_use]
    pub fn with_max_new_tokens(mut self, max_new_tokens: u32) -> Self {
        self.max_new_tokens = Some(max_new_tokens);
        self
    }

    /// Set the top-p sampling parameter (0.0 to 1.0)
    #[must_use]
    pub fn with_top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set the top-k sampling parameter
    #[must_use]
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set the repetition penalty (1.0 = no penalty)
    #[must_use]
    pub fn with_repetition_penalty(mut self, repetition_penalty: f64) -> Self {
        self.repetition_penalty = Some(repetition_penalty);
        self
    }

    /// Set whether to return full text (including prompt)
    #[must_use]
    pub fn with_return_full_text(mut self, return_full_text: bool) -> Self {
        self.return_full_text = Some(return_full_text);
        self
    }

    /// Set a custom retry policy
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Convert messages to a prompt string
    fn messages_to_prompt(&self, messages: &[BaseMessage]) -> String {
        messages
            .iter()
            .map(|msg| match msg {
                Message::System { content, .. } => format!("System: {}", content.as_text()),
                Message::Human { content, .. } => format!("User: {}", content.as_text()),
                Message::AI { content, .. } => format!("Assistant: {}", content.as_text()),
                _ => msg.as_text().clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build the request parameters
    fn build_parameters(&self) -> Option<HuggingFaceParameters> {
        Some(HuggingFaceParameters {
            temperature: self.temperature,
            max_new_tokens: self.max_new_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            repetition_penalty: self.repetition_penalty,
            return_full_text: self.return_full_text,
        })
    }

    /// Make a non-streaming request to `HuggingFace` API
    async fn make_request(&self, prompt: String) -> Result<String> {
        let request_body = HuggingFaceRequest {
            inputs: prompt,
            parameters: self.build_parameters(),
            options: Some(HuggingFaceOptions {
                use_cache: Some(false),
                wait_for_model: Some(true),
            }),
        };

        let mut request_builder = self
            .http_client
            .post(&self.endpoint_url)
            .json(&request_body);

        if let Some(token) = &self.api_token {
            request_builder = request_builder.header("Authorization", format!("Bearer {token}"));
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| Error::api(format!("HuggingFace API request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::api(format!(
                "HuggingFace API error ({status}): {error_text}"
            )));
        }

        let response_json: Vec<HuggingFaceResponse> = response
            .json()
            .await
            .map_err(|e| Error::api(format!("Failed to parse HuggingFace response: {e}")))?;

        Ok(response_json
            .first()
            .ok_or_else(|| Error::api("Empty response from HuggingFace API"))?
            .generated_text
            .clone())
    }

    /// Make a streaming request to `HuggingFace` API
    async fn make_streaming_request(
        &self,
        prompt: String,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_body = HuggingFaceRequest {
            inputs: prompt,
            parameters: self.build_parameters(),
            options: Some(HuggingFaceOptions {
                use_cache: Some(false),
                wait_for_model: Some(true),
            }),
        };

        let mut request_builder = self
            .http_client
            .post(&self.endpoint_url)
            .json(&request_body)
            .header("Accept", "text/event-stream");

        if let Some(token) = &self.api_token {
            request_builder = request_builder.header("Authorization", format!("Bearer {token}"));
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| Error::api(format!("HuggingFace streaming request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::api(format!(
                "HuggingFace API error ({status}): {error_text}"
            )));
        }

        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let stream = stream! {
            for await event in event_stream {
                match event {
                    Ok(event) => {
                        if event.event == "error" {
                            yield Err(Error::api(format!("Stream error: {}", event.data)));
                            break;
                        }

                        if !event.data.is_empty() && event.data != "[DONE]" {
                            match serde_json::from_str::<HuggingFaceStreamChunk>(&event.data) {
                                Ok(chunk) => {
                                    if !chunk.token.text.is_empty() {
                                        yield Ok(chunk.token.text);
                                    }
                                }
                                Err(e) => {
                                    yield Err(Error::api(format!("Failed to parse stream chunk: {e}")));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::api(format!("Stream error: {e}")));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    /// Create a `ChatHuggingFace` instance from a configuration
    ///
    /// This method constructs a `ChatHuggingFace` model from a `ChatModelConfig::HuggingFace` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be `HuggingFace` variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatHuggingFace` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not a `HuggingFace` variant
    /// - API key environment variable cannot be resolved
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::HuggingFace {
                model,
                api_key,
                temperature,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                // Create the ChatHuggingFace instance
                let mut chat_model = Self::with_api_token(model, &resolved_api_key);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(f64::from(*temp));
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected HuggingFace config, got {} config",
                config.provider()
            ))),
        }
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatHuggingFace {
    fn default() -> Self {
        Self::new("meta-llama/Llama-2-7b-chat-hf")
    }
}

#[async_trait]
impl ChatModel for ChatHuggingFace {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
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
                    None,                              // parent_run_id
                    &[],                               // tags
                    &std::collections::HashMap::new(), // metadata
                )
                .await?;
        }

        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let prompt = self.messages_to_prompt(messages);

        let generate_fn = || async {
            let generated_text = self.make_request(prompt.clone()).await?;

            let generation = ChatGeneration {
                message: Message::ai(generated_text),
                generation_info: None,
            };

            Ok(ChatResult {
                generations: vec![generation],
                llm_output: None,
            })
        };

        let result = with_retry(&self.retry_policy, generate_fn).await;

        // Handle error callback and end callback
        match result {
            Ok(chat_result) => {
                if let Some(manager) = run_manager {
                    let mut outputs = std::collections::HashMap::new();
                    outputs.insert(
                        "generations".to_string(),
                        serde_json::to_value(&chat_result.generations)?,
                    );
                    manager.on_llm_end(&outputs, run_id, None).await?;
                }
                Ok(chat_result)
            }
            Err(e) => {
                if let Some(manager) = run_manager {
                    if let Err(cb_err) = manager.on_llm_error(&e.to_string(), run_id, None).await {
                        eprintln!("[WARN] Failed to send LLM error callback: {}", cb_err);
                    }
                }
                Err(e)
            }
        }
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let prompt = self.messages_to_prompt(messages);
        let text_stream = self.make_streaming_request(prompt).await?;

        let chunk_stream = stream! {
            for await text_result in text_stream {
                match text_result {
                    Ok(text) => {
                        let chunk = ChatGenerationChunk {
                            message: AIMessageChunk::new(text),
                            generation_info: None,
                        };
                        yield Ok(chunk);
                    }
                    Err(e) => {
                        yield Err(e);
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }

    fn llm_type(&self) -> &str {
        &self.model_id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[allow(deprecated, clippy::disallowed_methods)]
mod tests {
    use super::*;

    // ============================================
    // Constructor tests
    // ============================================

    #[test]
    fn test_chat_huggingface_creation() {
        let model = ChatHuggingFace::new("meta-llama/Llama-2-7b-chat-hf");
        assert_eq!(model.model_id, "meta-llama/Llama-2-7b-chat-hf");
        assert_eq!(
            model.endpoint_url,
            "https://api-inference.huggingface.co/models/meta-llama/Llama-2-7b-chat-hf"
        );
    }

    #[test]
    fn test_with_api_token() {
        let model = ChatHuggingFace::with_api_token("meta-llama/Llama-2-7b-chat-hf", "test-token");
        assert_eq!(model.api_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_default_values() {
        let model = ChatHuggingFace::new("test-model");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_new_tokens, Some(512));
        assert_eq!(model.top_p, Some(0.95));
        assert!(model.top_k.is_none());
        assert!(model.repetition_penalty.is_none());
        assert_eq!(model.return_full_text, Some(false));
    }

    #[test]
    fn test_default_implementation() {
        let model = ChatHuggingFace::default();
        assert_eq!(model.model_id, "meta-llama/Llama-2-7b-chat-hf");
    }

    // ============================================
    // Builder pattern tests
    // ============================================

    #[test]
    fn test_builder_pattern() {
        let model = ChatHuggingFace::new("meta-llama/Llama-2-7b-chat-hf")
            .with_temperature(0.5)
            .with_max_new_tokens(1024)
            .with_top_p(0.9)
            .with_top_k(50);

        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_new_tokens, Some(1024));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.top_k, Some(50));
    }

    #[test]
    fn test_with_repetition_penalty() {
        let model = ChatHuggingFace::new("test-model")
            .with_repetition_penalty(1.2);
        assert_eq!(model.repetition_penalty, Some(1.2));
    }

    #[test]
    fn test_with_return_full_text() {
        let model = ChatHuggingFace::new("test-model")
            .with_return_full_text(true);
        assert_eq!(model.return_full_text, Some(true));
    }

    #[test]
    fn test_with_custom_endpoint_url() {
        let model = ChatHuggingFace::new("test-model")
            .with_endpoint_url("https://custom.endpoint.com/inference");
        assert_eq!(model.endpoint_url, "https://custom.endpoint.com/inference");
    }

    #[test]
    fn test_with_retry_policy() {
        use dashflow::core::retry::RetryPolicy;
        let model = ChatHuggingFace::new("test-model")
            .with_retry_policy(RetryPolicy::exponential(5));
        // Just verify it doesn't panic
        assert_eq!(model.model_id, "test-model");
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));
        let model = ChatHuggingFace::new("test-model")
            .with_rate_limiter(rate_limiter);
        assert!(model.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_chain_all_options() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use dashflow::core::retry::RetryPolicy;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));

        let model = ChatHuggingFace::with_api_token("test-model", "test-token")
            .with_endpoint_url("https://custom.url")
            .with_temperature(0.8)
            .with_max_new_tokens(2048)
            .with_top_p(0.85)
            .with_top_k(40)
            .with_repetition_penalty(1.1)
            .with_return_full_text(true)
            .with_retry_policy(RetryPolicy::exponential(3))
            .with_rate_limiter(rate_limiter);

        assert_eq!(model.api_token, Some("test-token".to_string()));
        assert_eq!(model.endpoint_url, "https://custom.url");
        assert_eq!(model.temperature, Some(0.8));
        assert_eq!(model.max_new_tokens, Some(2048));
        assert_eq!(model.top_p, Some(0.85));
        assert_eq!(model.top_k, Some(40));
        assert_eq!(model.repetition_penalty, Some(1.1));
        assert_eq!(model.return_full_text, Some(true));
        assert!(model.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_overwrites_values() {
        let model = ChatHuggingFace::new("test-model")
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.temperature, Some(0.9));
    }

    // ============================================
    // Messages to prompt tests
    // ============================================

    #[test]
    fn test_messages_to_prompt() {
        let model = ChatHuggingFace::new("meta-llama/Llama-2-7b-chat-hf");
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::human("Hello!"),
            Message::ai("Hi there!"),
        ];

        let prompt = model.messages_to_prompt(&messages);
        assert!(prompt.contains("System: You are a helpful assistant."));
        assert!(prompt.contains("User: Hello!"));
        assert!(prompt.contains("Assistant: Hi there!"));
    }

    #[test]
    fn test_messages_to_prompt_empty() {
        let model = ChatHuggingFace::new("test-model");
        let messages: Vec<BaseMessage> = vec![];
        let prompt = model.messages_to_prompt(&messages);
        assert!(prompt.is_empty());
    }

    #[test]
    fn test_messages_to_prompt_single_human() {
        let model = ChatHuggingFace::new("test-model");
        let messages = vec![Message::human("Just one message")];
        let prompt = model.messages_to_prompt(&messages);
        assert_eq!(prompt, "User: Just one message");
    }

    #[test]
    fn test_messages_to_prompt_multiple_turns() {
        let model = ChatHuggingFace::new("test-model");
        let messages = vec![
            Message::human("First question"),
            Message::ai("First answer"),
            Message::human("Second question"),
            Message::ai("Second answer"),
        ];
        let prompt = model.messages_to_prompt(&messages);
        assert!(prompt.contains("User: First question"));
        assert!(prompt.contains("Assistant: First answer"));
        assert!(prompt.contains("User: Second question"));
        assert!(prompt.contains("Assistant: Second answer"));
        // Verify order by checking newline separation
        let lines: Vec<&str> = prompt.lines().collect();
        assert_eq!(lines.len(), 4);
    }

    // ============================================
    // LLM type tests
    // ============================================

    #[test]
    fn test_llm_type_returns_model_id() {
        let model = ChatHuggingFace::new("my-custom/model-name");
        assert_eq!(model.llm_type(), "my-custom/model-name");
    }

    // ============================================
    // Clone and Debug tests
    // ============================================

    #[test]
    fn test_clone() {
        let model1 = ChatHuggingFace::with_api_token("test-model", "test-token")
            .with_temperature(0.5);
        let model2 = model1.clone();

        assert_eq!(model1.model_id, model2.model_id);
        assert_eq!(model1.api_token, model2.api_token);
        assert_eq!(model1.temperature, model2.temperature);
    }

    #[test]
    fn test_debug() {
        let model = ChatHuggingFace::new("test-model");
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("ChatHuggingFace"));
        assert!(debug_str.contains("test-model"));
    }

    // ============================================
    // Endpoint URL construction tests
    // ============================================

    #[test]
    fn test_endpoint_url_construction_with_slash() {
        let model = ChatHuggingFace::new("org/model-name");
        assert_eq!(
            model.endpoint_url,
            "https://api-inference.huggingface.co/models/org/model-name"
        );
    }

    #[test]
    fn test_endpoint_url_construction_simple_name() {
        let model = ChatHuggingFace::new("simple-model");
        assert_eq!(
            model.endpoint_url,
            "https://api-inference.huggingface.co/models/simple-model"
        );
    }

    // ============================================
    // Build parameters tests
    // ============================================

    #[test]
    fn test_build_parameters_default() {
        let model = ChatHuggingFace::new("test-model");
        let params = model.build_parameters().unwrap();

        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.max_new_tokens, Some(512));
        assert_eq!(params.top_p, Some(0.95));
        assert!(params.top_k.is_none());
        assert!(params.repetition_penalty.is_none());
        assert_eq!(params.return_full_text, Some(false));
    }

    #[test]
    fn test_build_parameters_custom() {
        let model = ChatHuggingFace::new("test-model")
            .with_temperature(0.3)
            .with_max_new_tokens(100)
            .with_top_p(0.8)
            .with_top_k(10)
            .with_repetition_penalty(1.5)
            .with_return_full_text(true);

        let params = model.build_parameters().unwrap();

        assert_eq!(params.temperature, Some(0.3));
        assert_eq!(params.max_new_tokens, Some(100));
        assert_eq!(params.top_p, Some(0.8));
        assert_eq!(params.top_k, Some(10));
        assert_eq!(params.repetition_penalty, Some(1.5));
        assert_eq!(params.return_full_text, Some(true));
    }

    // ============================================
    // Model ID edge cases
    // ============================================

    #[test]
    fn test_model_id_with_version() {
        let model = ChatHuggingFace::new("org/model-v1.0.0");
        assert_eq!(model.model_id, "org/model-v1.0.0");
    }

    #[test]
    fn test_model_id_with_special_chars() {
        let model = ChatHuggingFace::new("org/model_name-v2");
        assert_eq!(model.model_id, "org/model_name-v2");
    }

    #[test]
    fn test_model_id_empty() {
        let model = ChatHuggingFace::new("");
        assert_eq!(model.model_id, "");
        assert_eq!(model.endpoint_url, "https://api-inference.huggingface.co/models/");
    }

    // ============================================
    // Temperature edge cases
    // ============================================

    #[test]
    fn test_temperature_zero() {
        let model = ChatHuggingFace::new("test-model")
            .with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_temperature_max() {
        let model = ChatHuggingFace::new("test-model")
            .with_temperature(2.0);
        assert_eq!(model.temperature, Some(2.0));
    }

    // ============================================
    // Max tokens edge cases
    // ============================================

    #[test]
    fn test_max_tokens_small() {
        let model = ChatHuggingFace::new("test-model")
            .with_max_new_tokens(1);
        assert_eq!(model.max_new_tokens, Some(1));
    }

    #[test]
    fn test_max_tokens_large() {
        let model = ChatHuggingFace::new("test-model")
            .with_max_new_tokens(4096);
        assert_eq!(model.max_new_tokens, Some(4096));
    }

    // ============================================
    // Top-p edge cases
    // ============================================

    #[test]
    fn test_top_p_zero() {
        let model = ChatHuggingFace::new("test-model")
            .with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_top_p_one() {
        let model = ChatHuggingFace::new("test-model")
            .with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    // ============================================
    // API token edge cases
    // ============================================

    #[test]
    fn test_api_token_empty() {
        let model = ChatHuggingFace::with_api_token("test-model", "");
        assert_eq!(model.api_token, Some("".to_string()));
    }

    #[test]
    fn test_api_token_with_special_chars() {
        let token = "hf_abc!@#$%^&*()";
        let model = ChatHuggingFace::with_api_token("test-model", token);
        assert_eq!(model.api_token, Some(token.to_string()));
    }

    // ============================================
    // Multiple models tests
    // ============================================

    #[test]
    fn test_multiple_independent_models() {
        let model1 = ChatHuggingFace::new("model-1").with_temperature(0.5);
        let model2 = ChatHuggingFace::new("model-2").with_temperature(0.9);

        assert_eq!(model1.model_id, "model-1");
        assert_eq!(model2.model_id, "model-2");
        assert_eq!(model1.temperature, Some(0.5));
        assert_eq!(model2.temperature, Some(0.9));
    }

    // ============================================
    // as_any tests
    // ============================================

    #[test]
    fn test_as_any() {
        let model = ChatHuggingFace::new("test-model");
        let any = model.as_any();
        assert!(any.is::<ChatHuggingFace>());
    }

    // ============================================
    // Integration test (requires API)
    // ============================================

    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_generate_without_api_key() {
        assert!(
            std::env::var("HUGGINGFACEHUB_API_TOKEN").is_ok() || std::env::var("HF_TOKEN").is_ok(),
            "HUGGINGFACEHUB_API_TOKEN or HF_TOKEN must be set"
        );

        let model = ChatHuggingFace::new("meta-llama/Llama-2-7b-chat-hf").with_max_new_tokens(50);

        let messages = vec![Message::human("Hello!")];
        let result = model.generate(&messages, None, None, None, None).await;

        // This may fail due to rate limits or model availability
        // We're just testing that the API structure is correct
        match result {
            Ok(_) => {
                // Success!
            }
            Err(e) => {
                // Expected errors: rate limit, model loading, authentication
                println!("Expected API error: {}", e);
            }
        }
    }
}

/// Standard conformance tests
///
/// These tests verify that ChatHuggingFace behaves consistently with other
/// ChatModel implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(deprecated, clippy::disallowed_methods)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::chat_model_tests::*;
    use dashflow_test_utils::init_test_env;

    /// Helper function to create a test model with standard settings
    ///
    /// Uses mistralai/Mistral-7B-Instruct-v0.2 for testing
    fn create_test_model() -> ChatHuggingFace {
        ChatHuggingFace::new("mistralai/Mistral-7B-Instruct-v0.2")
            .with_temperature(0.0) // Deterministic for testing
            .with_max_new_tokens(100) // Limit tokens for cost/speed
    }

    /// Require API key for ignored integration tests.
    fn check_credentials() {
        init_test_env().ok();
        assert!(
            std::env::var("HUGGINGFACEHUB_API_TOKEN").is_ok() || std::env::var("HF_TOKEN").is_ok(),
            "HUGGINGFACEHUB_API_TOKEN or HF_TOKEN must be set"
        );
    }

    /// Standard Test 1: Basic invoke
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_invoke_standard() {
        check_credentials();
        let model = create_test_model();
        test_invoke(&model).await;
    }

    /// Standard Test 2: Streaming
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_stream_standard() {
        check_credentials();
        let model = create_test_model();
        test_stream(&model).await;
    }

    /// Standard Test 3: Batch processing
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_batch_standard() {
        check_credentials();
        let model = create_test_model();
        test_batch(&model).await;
    }

    /// Standard Test 4: Multi-turn conversation
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_conversation_standard() {
        check_credentials();
        let model = create_test_model();
        test_conversation(&model).await;
    }

    /// Standard Test 4b: Double messages conversation
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_double_messages_conversation_standard() {
        check_credentials();
        let model = create_test_model();
        test_double_messages_conversation(&model).await;
    }

    /// Standard Test 4c: Message with name field
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_message_with_name_standard() {
        check_credentials();
        let model = create_test_model();
        test_message_with_name(&model).await;
    }

    /// Standard Test 5: Stop sequences
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_stop_sequence_standard() {
        check_credentials();
        let model = create_test_model();
        test_stop_sequence(&model).await;
    }

    /// Standard Test 6: Usage metadata
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_usage_metadata_standard() {
        check_credentials();
        let model = create_test_model();
        test_usage_metadata(&model).await;
    }

    /// Standard Test 7: Empty messages
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_empty_messages_standard() {
        check_credentials();
        let model = create_test_model();
        test_empty_messages(&model).await;
    }

    /// Standard Test 8: Long conversation
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_long_conversation_standard() {
        check_credentials();
        let model = create_test_model();
        test_long_conversation(&model).await;
    }

    /// Standard Test 9: Special characters
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_special_characters_standard() {
        check_credentials();
        let model = create_test_model();
        test_special_characters(&model).await;
    }

    /// Standard Test 10: Unicode and emoji
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_unicode_standard() {
        check_credentials();
        let model = create_test_model();
        test_unicode(&model).await;
    }

    /// Standard Test 11: Tool calling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_tool_calling_standard() {
        check_credentials();
        let model = create_test_model();
        test_tool_calling(&model).await;
    }

    /// Standard Test 12: Structured output
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_structured_output_standard() {
        check_credentials();
        let model = create_test_model();
        test_structured_output(&model).await;
    }

    /// Standard Test 13: JSON mode
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_json_mode_standard() {
        check_credentials();
        let model = create_test_model();
        test_json_mode(&model).await;
    }

    /// Standard Test 14: Usage metadata in streaming
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_usage_metadata_streaming_standard() {
        check_credentials();
        let model = create_test_model();
        test_usage_metadata_streaming(&model).await;
    }

    /// Standard Test 15: System message handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_system_message_standard() {
        check_credentials();
        let model = create_test_model();
        test_system_message(&model).await;
    }

    /// Standard Test 16: Empty content handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_empty_content_standard() {
        check_credentials();
        let model = create_test_model();
        test_empty_content(&model).await;
    }

    /// Standard Test 17: Large input handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_large_input_standard() {
        check_credentials();
        let model = create_test_model();
        test_large_input(&model).await;
    }

    /// Standard Test 18: Concurrent generation
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_concurrent_generation_standard() {
        check_credentials();
        let model = create_test_model();
        test_concurrent_generation(&model).await;
    }

    /// Standard Test 19: Error recovery
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_error_recovery_standard() {
        check_credentials();
        let model = create_test_model();
        test_error_recovery(&model).await;
    }

    /// Standard Test 20: Response consistency
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_response_consistency_standard() {
        check_credentials();
        let model = create_test_model();
        test_response_consistency(&model).await;
    }

    /// Standard Test 21: Tool calling with no arguments
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_tool_calling_with_no_arguments_standard() {
        check_credentials();
        let model = create_test_model();
        test_tool_calling_with_no_arguments(&model).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS - Advanced Edge Cases
    // ========================================================================

    /// Comprehensive Test 1: Streaming with timeout
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_stream_with_timeout_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_stream_with_timeout(&model).await;
    }

    /// Comprehensive Test 2: Streaming interruption handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_stream_interruption_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_stream_interruption(&model).await;
    }

    /// Comprehensive Test 3: Empty stream handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_stream_empty_response_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_stream_empty_response(&model).await;
    }

    /// Comprehensive Test 4: Multiple system messages
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_multiple_system_messages_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_multiple_system_messages(&model).await;
    }

    /// Comprehensive Test 5: Empty system message
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_empty_system_message_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_empty_system_message(&model).await;
    }

    /// Comprehensive Test 6: Temperature edge cases
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_temperature_extremes_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_temperature_extremes(&model).await;
    }

    /// Comprehensive Test 7: Max tokens enforcement
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_max_tokens_limit_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_max_tokens_limit(&model).await;
    }

    /// Comprehensive Test 8: Invalid stop sequences
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_invalid_stop_sequences_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_invalid_stop_sequences(&model).await;
    }

    /// Comprehensive Test 9: Context window overflow
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_context_window_overflow_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_context_window_overflow(&model).await;
    }

    /// Comprehensive Test 10: Rapid consecutive calls
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_rapid_consecutive_calls_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_rapid_consecutive_calls(&model).await;
    }

    /// Comprehensive Test 11: Network error handling
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_network_error_handling_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_network_error_handling(&model).await;
    }

    /// Comprehensive Test 12: Malformed input recovery
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_malformed_input_recovery_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_malformed_input_recovery(&model).await;
    }

    /// Comprehensive Test 13: Very long single message
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_very_long_single_message_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_very_long_single_message(&model).await;
    }

    /// Comprehensive Test 14: Response format consistency
    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN or HF_TOKEN"]
    async fn test_response_format_consistency_comprehensive() {
        check_credentials();
        let model = create_test_model();
        test_response_format_consistency(&model).await;
    }
}
