//! Google Gemini chat models implementation

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string, GEMINI_API_KEY},
    error::Error as DashFlowError,
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, Message, MessageContent, ToolCall},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    usage::UsageMetadata,
};
use eventsource_stream::Eventsource;
use futures::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

const DEFAULT_MODEL: &str = "gemini-2.0-flash-exp";
const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini chat model
///
/// Supports Gemini 2.0 Flash, Pro, and other models with streaming,
/// function calling, and multimodal capabilities.
///
/// # Example
///
/// ```no_run
/// use dashflow_gemini::ChatGemini;
/// use dashflow::core::messages::Message;
/// use dashflow::core::language_models::ChatModel;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ChatGemini::new()
///     .with_api_key(std::env::var("GEMINI_API_KEY")?)
///     .with_model("gemini-2.0-flash-exp");
///
/// let messages = vec![Message::human("Hello!")];
/// let response = model.generate(&messages, None, None, None, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ChatGemini {
    /// API key for authentication
    api_key: Option<String>,
    /// Model name (e.g., "gemini-2.0-flash-exp")
    model: String,
    /// HTTP client
    client: Client,
    /// Temperature for sampling (0.0 to 2.0)
    temperature: Option<f32>,
    /// Maximum number of tokens to generate
    max_tokens: Option<u32>,
    /// Top-p sampling parameter
    top_p: Option<f32>,
    /// Top-k sampling parameter
    top_k: Option<u32>,
    /// System instruction for the model
    system_instruction: Option<String>,
    /// Safety settings
    safety_settings: Vec<SafetySettings>,
    /// Enable thinking mode (extended reasoning)
    enable_thinking: bool,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

// Custom Debug to prevent API key exposure in logs
impl std::fmt::Debug for ChatGemini {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatGemini")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("model", &self.model)
            .field("client", &"[reqwest::Client]")
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("top_p", &self.top_p)
            .field("top_k", &self.top_k)
            .field("system_instruction", &self.system_instruction)
            .field("safety_settings", &self.safety_settings)
            .field("enable_thinking", &self.enable_thinking)
            .field("retry_policy", &self.retry_policy)
            .field("rate_limiter", &self.rate_limiter.as_ref().map(|_| "[RateLimiter]"))
            .finish()
    }
}

impl ChatGemini {
    /// Creates a new `ChatGemini` instance with default settings
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_key: env_string(GEMINI_API_KEY),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            system_instruction: None,
            safety_settings: Vec::new(),
            enable_thinking: false,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Sets the API key
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the model name
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Sets the temperature (0.0 to 2.0)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets the maximum number of tokens to generate
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets the top-p sampling parameter
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Sets the top-k sampling parameter
    #[must_use]
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Sets the system instruction
    #[must_use]
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }

    /// Adds a safety setting
    #[must_use]
    pub fn with_safety_setting(mut self, setting: SafetySettings) -> Self {
        self.safety_settings.push(setting);
        self
    }

    /// Enables thinking mode (extended reasoning)
    #[must_use]
    pub fn with_thinking(mut self, enable: bool) -> Self {
        self.enable_thinking = enable;
        self
    }

    /// Set the retry policy for API calls
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_gemini::ChatGemini;
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// let model = ChatGemini::new()
    ///     .with_retry_policy(RetryPolicy::exponential(5));
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter to control request rate
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_gemini::ChatGemini;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,                        // 10 requests per second
    ///     Duration::from_millis(100),  // check every 100ms
    ///     20.0,                        // max bucket size
    /// );
    ///
    /// let model = ChatGemini::new()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// # }
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Builds the API endpoint URL
    fn endpoint(&self, streaming: bool) -> String {
        let method = if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        format!("{}/models/{}:{}", API_BASE, self.model, method)
    }

    /// Creates a request builder with authentication
    fn request(&self, url: &str) -> Result<reqwest::RequestBuilder, DashFlowError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| {
                DashFlowError::authentication(
                    "API key is required. Set it with with_api_key() or the GEMINI_API_KEY environment variable",
                )
            })?;

        Ok(self.client.post(url).header("x-goog-api-key", api_key))
    }

    /// Converts `DashFlow` messages to Gemini API format
    fn convert_messages(
        &self,
        messages: &[BaseMessage],
    ) -> Result<Vec<GeminiContent>, DashFlowError> {
        let mut gemini_contents = Vec::new();

        for message in messages {
            match message {
                Message::System { .. } => {
                    // System messages handled separately in system_instruction
                    continue;
                }
                Message::Human { content, .. } => {
                    let text = content.as_text();
                    let parts = vec![GeminiPart {
                        text: Some(text),
                        inline_data: None,
                        function_call: None,
                        function_response: None,
                    }];

                    gemini_contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts,
                    });
                }
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut parts = Vec::new();

                    // Add text content
                    let text = content.as_text();
                    if !text.is_empty() {
                        parts.push(GeminiPart {
                            text: Some(text),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        });
                    }

                    // Add tool calls
                    for tool_call in tool_calls {
                        parts.push(GeminiPart {
                            text: None,
                            inline_data: None,
                            function_call: Some(GeminiFunctionCall {
                                name: tool_call.name.clone(),
                                args: tool_call.args.clone(),
                            }),
                            function_response: None,
                        });
                    }

                    gemini_contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts,
                    });
                }
                Message::Tool {
                    content,
                    tool_call_id,
                    ..
                } => {
                    // Tool results need to be sent back as function responses
                    let parts = vec![GeminiPart {
                        text: None,
                        inline_data: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: tool_call_id.clone(),
                            response: serde_json::json!({ "result": content.as_text() }),
                        }),
                    }];

                    gemini_contents.push(GeminiContent {
                        role: "function".to_string(),
                        parts,
                    });
                }
                Message::Function { content, name, .. } => {
                    // Function messages - legacy format similar to Tool
                    let parts = vec![GeminiPart {
                        text: None,
                        inline_data: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: name.clone(),
                            response: serde_json::json!({ "result": content.as_text() }),
                        }),
                    }];

                    gemini_contents.push(GeminiContent {
                        role: "function".to_string(),
                        parts,
                    });
                }
            }
        }

        Ok(gemini_contents)
    }

    /// Builds the request payload
    fn build_request(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
    ) -> Result<GeminiRequest, DashFlowError> {
        let contents = self.convert_messages(messages)?;

        // Build generation config
        let generation_config = GeminiGenerationConfig {
            temperature: self.temperature,
            max_output_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences: stop.map(<[std::string::String]>::to_vec),
        };

        // Convert tools to function declarations
        let tool_config = tools.map(|tools_slice| {
            let function_declarations: Vec<_> = tools_slice
                .iter()
                .map(|tool| GeminiFunctionDeclaration {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                })
                .collect();

            vec![GeminiTool {
                function_declarations,
            }]
        });

        // Build system instruction
        let system_instruction = if let Some(inst) = &self.system_instruction {
            Some(GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: Some(inst.clone()),
                    inline_data: None,
                    function_call: None,
                    function_response: None,
                }],
            })
        } else {
            // Check for system messages in input
            messages.iter().find_map(|msg| {
                if let Message::System { content, .. } = msg {
                    Some(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart {
                            text: Some(content.as_text()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        }],
                    })
                } else {
                    None
                }
            })
        };

        Ok(GeminiRequest {
            contents,
            generation_config: Some(generation_config),
            safety_settings: if self.safety_settings.is_empty() {
                None
            } else {
                Some(self.safety_settings.clone())
            },
            tools: tool_config,
            system_instruction,
        })
    }

    /// Parses the response from Gemini API
    fn parse_response(&self, response: GeminiResponse) -> Result<ChatResult, DashFlowError> {
        let candidate = response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| DashFlowError::api_format("No candidates in response"))?;

        let content = candidate.content;
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for part in content.parts {
            if let Some(text) = part.text {
                text_parts.push(text);
            }
            if let Some(function_call) = part.function_call {
                tool_calls.push(ToolCall {
                    id: Uuid::new_v4().to_string(),
                    name: function_call.name,
                    args: function_call.args,
                    tool_type: "tool_call".to_string(),
                    index: None,
                });
            }
        }

        let message_content = text_parts.join("");

        // Extract token usage (clone to use twice)
        let usage_metadata = response.usage_metadata.as_ref().map(|usage| UsageMetadata {
            input_tokens: usage.prompt_token_count,
            output_tokens: usage.candidates_token_count,
            total_tokens: usage.total_token_count,
            input_token_details: None,
            output_token_details: None,
        });

        // Build response metadata
        let mut response_metadata = HashMap::new();
        if let Some(finish_reason) = candidate.finish_reason {
            response_metadata.insert(
                "finish_reason".to_string(),
                serde_json::json!(finish_reason),
            );
        }

        // Create AI message
        let ai_message = AIMessage::new(MessageContent::Text(message_content))
            .with_tool_calls(tool_calls.clone());

        let ai_message = if let Some(usage) = usage_metadata {
            ai_message.with_usage(usage)
        } else {
            ai_message
        };

        // Convert to Message enum
        let mut message = Message::from(ai_message);
        message.fields_mut().response_metadata = response_metadata;

        // Build generation_info
        let mut generation_info = HashMap::new();
        if let Some(usage) = &response.usage_metadata {
            generation_info.insert(
                "usage".to_string(),
                serde_json::json!({
                    "prompt_token_count": usage.prompt_token_count,
                    "candidates_token_count": usage.candidates_token_count,
                    "total_token_count": usage.total_token_count,
                }),
            );
        }

        let generation = ChatGeneration {
            message,
            generation_info: Some(generation_info),
        };

        Ok(ChatResult {
            generations: vec![generation],
            llm_output: None,
        })
    }
}

impl Default for ChatGemini {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatModel for ChatGemini {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult, DashFlowError> {
        let request = self.build_request(messages, stop, tools)?;
        let url = self.endpoint(false);

        // Clone necessary data for retry closure
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        // Make API call with retry
        let gemini_response = with_retry(&self.retry_policy, move || {
            let api_key = api_key.clone();
            let client = client.clone();
            let url = url.clone();
            let request = request.clone();
            async move {
                let request_builder = match &api_key {
                    Some(key) => client.post(&url).header("x-goog-api-key", key),
                    None => {
                        return Err(DashFlowError::authentication(
                            "API key is required. Set it with with_api_key() or the GEMINI_API_KEY environment variable",
                        ))
                    }
                };

                let response = request_builder
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| DashFlowError::api(format!("HTTP request failed: {e}")))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(DashFlowError::api(format!(
                        "Gemini API error ({status}): {error_text}"
                    )));
                }

                response
                    .json::<GeminiResponse>()
                    .await
                    .map_err(|e| DashFlowError::api_format(format!("Failed to parse response: {e}")))
            }
        })
        .await?;

        self.parse_response(gemini_response)
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk, DashFlowError>> + Send + 'static>>,
        DashFlowError,
    > {
        let request = self.build_request(messages, stop, tools)?;
        let url = self.endpoint(true);

        let response = self
            .request(&url)?
            .json(&request)
            .send()
            .await
            .map_err(|e| DashFlowError::api(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DashFlowError::api(format!(
                "Gemini API error ({status}): {error_text}"
            )));
        }

        // Gemini uses Server-Sent Events for streaming
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let chunk_stream = stream! {
            use futures::StreamExt;

            let mut event_stream = event_stream;
            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        // Parse the JSON data
                        if let Ok(response) = serde_json::from_str::<GeminiResponse>(&event.data) {
                            // Extract text from first candidate
                            if let Some(candidate) = response.candidates.first() {
                                for part in &candidate.content.parts {
                                    if let Some(text) = &part.text {
                                        let chunk = AIMessageChunk::new(text.clone());
                                        yield Ok(ChatGenerationChunk::new(chunk));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(DashFlowError::other(format!("SSE stream error: {e}")));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }

    fn llm_type(&self) -> &'static str {
        "gemini"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.rate_limiter.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// API Types
// ============================================================================

/// Gemini API request
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<SafetySettings>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
}

/// Gemini content (message)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

/// Gemini content part (text, image, function call, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inline_data: Option<GeminiInlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_response: Option<GeminiFunctionResponse>,
}

/// Inline data (images, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String, // Base64 encoded
}

/// Function call from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: JsonValue,
}

/// Function response back to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: JsonValue,
}

/// Generation configuration
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

/// Tool configuration (function declarations)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

/// Function declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: JsonValue,
}

/// Safety settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct SafetySettings {
    pub category: SafetyCategory,
    pub threshold: SafetyThreshold,
}

/// Safety categories
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SafetyCategory {
    HarmCategoryUnspecified,
    HarmCategoryDerogatory,
    HarmCategoryToxicity,
    HarmCategoryViolence,
    HarmCategorySexual,
    HarmCategoryMedical,
    HarmCategoryDangerous,
    HarmCategoryHarassment,
    HarmCategoryHateSpeech,
    HarmCategorySexuallyExplicit,
    HarmCategoryDangerousContent,
}

/// Safety thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SafetyThreshold {
    HarmBlockThresholdUnspecified,
    BlockLowAndAbove,
    BlockMediumAndAbove,
    BlockOnlyHigh,
    BlockNone,
}

/// Gemini API response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsageMetadata>,
}

/// Response candidate
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(default)]
    finish_reason: Option<String>,
}

/// Token usage metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: u32,
    candidates_token_count: u32,
    total_token_count: u32,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::rate_limiters::InMemoryRateLimiter;
    use std::time::Duration;

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_builder() {
        let model = ChatGemini::new()
            .with_api_key("test-key")
            .with_model("gemini-2.0-flash-exp")
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_top_p(0.9)
            .with_top_k(40)
            .with_system_instruction("You are a helpful assistant");

        assert_eq!(model.api_key, Some("test-key".to_string()));
        assert_eq!(model.model, "gemini-2.0-flash-exp");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(1000));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.top_k, Some(40));
        assert_eq!(
            model.system_instruction,
            Some("You are a helpful assistant".to_string())
        );
    }

    #[test]
    fn test_with_api_key() {
        let model = ChatGemini::new().with_api_key("sk-test-key-123");
        assert_eq!(model.api_key, Some("sk-test-key-123".to_string()));
    }

    #[test]
    fn test_with_api_key_string() {
        let key = String::from("sk-string-key");
        let model = ChatGemini::new().with_api_key(key);
        assert_eq!(model.api_key, Some("sk-string-key".to_string()));
    }

    #[test]
    fn test_with_api_key_empty() {
        let model = ChatGemini::new().with_api_key("");
        assert_eq!(model.api_key, Some(String::new()));
    }

    #[test]
    fn test_with_model() {
        let model = ChatGemini::new().with_model("gemini-pro");
        assert_eq!(model.model, "gemini-pro");
    }

    #[test]
    fn test_with_model_string() {
        let model_name = String::from("gemini-2.0-flash-exp");
        let model = ChatGemini::new().with_model(model_name);
        assert_eq!(model.model, "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_with_temperature() {
        let model = ChatGemini::new().with_temperature(0.5);
        assert_eq!(model.temperature, Some(0.5));
    }

    #[test]
    fn test_with_temperature_zero() {
        let model = ChatGemini::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_with_temperature_max() {
        let model = ChatGemini::new().with_temperature(2.0);
        assert_eq!(model.temperature, Some(2.0));
    }

    #[test]
    fn test_with_max_tokens() {
        let model = ChatGemini::new().with_max_tokens(4096);
        assert_eq!(model.max_tokens, Some(4096));
    }

    #[test]
    fn test_with_max_tokens_small() {
        let model = ChatGemini::new().with_max_tokens(1);
        assert_eq!(model.max_tokens, Some(1));
    }

    #[test]
    fn test_with_max_tokens_large() {
        let model = ChatGemini::new().with_max_tokens(1_000_000);
        assert_eq!(model.max_tokens, Some(1_000_000));
    }

    #[test]
    fn test_with_top_p() {
        let model = ChatGemini::new().with_top_p(0.95);
        assert_eq!(model.top_p, Some(0.95));
    }

    #[test]
    fn test_with_top_p_zero() {
        let model = ChatGemini::new().with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_with_top_p_one() {
        let model = ChatGemini::new().with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    #[test]
    fn test_with_top_k() {
        let model = ChatGemini::new().with_top_k(40);
        assert_eq!(model.top_k, Some(40));
    }

    #[test]
    fn test_with_top_k_one() {
        let model = ChatGemini::new().with_top_k(1);
        assert_eq!(model.top_k, Some(1));
    }

    #[test]
    fn test_with_top_k_large() {
        let model = ChatGemini::new().with_top_k(100);
        assert_eq!(model.top_k, Some(100));
    }

    #[test]
    fn test_with_system_instruction() {
        let model = ChatGemini::new().with_system_instruction("Be concise");
        assert_eq!(model.system_instruction, Some("Be concise".to_string()));
    }

    #[test]
    fn test_with_system_instruction_string() {
        let instruction = String::from("You are a helpful coding assistant");
        let model = ChatGemini::new().with_system_instruction(instruction);
        assert_eq!(
            model.system_instruction,
            Some("You are a helpful coding assistant".to_string())
        );
    }

    #[test]
    fn test_with_system_instruction_unicode() {
        let model = ChatGemini::new().with_system_instruction("你好世界 🌍");
        assert_eq!(model.system_instruction, Some("你好世界 🌍".to_string()));
    }

    #[test]
    fn test_with_thinking_true() {
        let model = ChatGemini::new().with_thinking(true);
        assert!(model.enable_thinking);
    }

    #[test]
    fn test_with_thinking_false() {
        let model = ChatGemini::new().with_thinking(false);
        assert!(!model.enable_thinking);
    }

    #[test]
    fn test_with_retry_policy() {
        let model = ChatGemini::new().with_retry_policy(RetryPolicy::exponential(5));
        // Verify the retry policy is set (comparing max_retries field)
        assert_eq!(model.retry_policy.max_retries, 5);
    }

    #[test]
    fn test_with_retry_policy_no_retry() {
        let model = ChatGemini::new().with_retry_policy(RetryPolicy::no_retry());
        assert_eq!(model.retry_policy.max_retries, 0);
    }

    #[test]
    fn test_with_rate_limiter() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));
        let model = ChatGemini::new().with_rate_limiter(rate_limiter);
        assert!(model.rate_limiter.is_some());
    }

    #[test]
    fn test_rate_limiter_method() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));
        let model = ChatGemini::new().with_rate_limiter(rate_limiter.clone());
        assert!(model.rate_limiter().is_some());
    }

    #[test]
    fn test_rate_limiter_none_by_default() {
        let model = ChatGemini::new();
        assert!(model.rate_limiter().is_none());
    }

    // ========================================================================
    // Default and Clone Tests
    // ========================================================================

    #[test]
    fn test_default() {
        let model = ChatGemini::default();
        assert_eq!(model.model, DEFAULT_MODEL);
        assert_eq!(model.temperature, None);
        assert_eq!(model.max_tokens, None);
    }

    #[test]
    fn test_default_model_name() {
        let model = ChatGemini::new();
        assert_eq!(model.model, "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_default_no_system_instruction() {
        let model = ChatGemini::new();
        assert!(model.system_instruction.is_none());
    }

    #[test]
    fn test_default_no_safety_settings() {
        let model = ChatGemini::new();
        assert!(model.safety_settings.is_empty());
    }

    #[test]
    fn test_default_thinking_disabled() {
        let model = ChatGemini::new();
        assert!(!model.enable_thinking);
    }

    #[test]
    fn test_clone() {
        let model = ChatGemini::new()
            .with_api_key("test-key")
            .with_model("gemini-pro")
            .with_temperature(0.7);

        let cloned = model.clone();
        assert_eq!(cloned.api_key, model.api_key);
        assert_eq!(cloned.model, model.model);
        assert_eq!(cloned.temperature, model.temperature);
    }

    #[test]
    fn test_clone_with_safety_settings() {
        let model = ChatGemini::new().with_safety_setting(SafetySettings {
            category: SafetyCategory::HarmCategoryHarassment,
            threshold: SafetyThreshold::BlockMediumAndAbove,
        });

        let cloned = model.clone();
        assert_eq!(cloned.safety_settings.len(), 1);
    }

    // ========================================================================
    // Debug Implementation Tests
    // ========================================================================

    #[test]
    fn test_debug_redacts_api_key() {
        let model = ChatGemini::new().with_api_key("sk-secret-key-12345");
        let debug_output = format!("{:?}", model);

        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("sk-secret-key-12345"));
    }

    #[test]
    fn test_debug_shows_model() {
        let model = ChatGemini::new().with_model("gemini-pro");
        let debug_output = format!("{:?}", model);

        assert!(debug_output.contains("gemini-pro"));
    }

    #[test]
    fn test_debug_shows_temperature() {
        let model = ChatGemini::new().with_temperature(0.7);
        let debug_output = format!("{:?}", model);

        assert!(debug_output.contains("0.7"));
    }

    #[test]
    fn test_debug_without_api_key() {
        let model = ChatGemini {
            api_key: None,
            model: "gemini-pro".to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            system_instruction: None,
            safety_settings: Vec::new(),
            enable_thinking: false,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let debug_output = format!("{:?}", model);

        assert!(debug_output.contains("api_key: None"));
    }

    #[test]
    fn test_debug_redacts_rate_limiter() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));
        let model = ChatGemini::new().with_rate_limiter(rate_limiter);
        let debug_output = format!("{:?}", model);

        assert!(debug_output.contains("[RateLimiter]"));
    }

    // ========================================================================
    // Endpoint Tests
    // ========================================================================

    #[test]
    fn test_endpoint() {
        let model = ChatGemini::new().with_model("gemini-2.0-flash-exp");
        assert_eq!(
            model.endpoint(false),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent"
        );
        assert_eq!(
            model.endpoint(true),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:streamGenerateContent"
        );
    }

    #[test]
    fn test_endpoint_different_model() {
        let model = ChatGemini::new().with_model("gemini-pro");
        assert_eq!(
            model.endpoint(false),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_endpoint_custom_model() {
        let model = ChatGemini::new().with_model("custom-model-v1");
        assert!(model.endpoint(false).contains("custom-model-v1"));
        assert!(model.endpoint(true).contains("custom-model-v1"));
    }

    // ========================================================================
    // Message Conversion Tests
    // ========================================================================

    #[test]
    fn test_convert_messages_simple() {
        let model = ChatGemini::new();
        let messages = vec![Message::human("Hello"), Message::ai("Hi there!")];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].parts[0].text, Some("Hello".to_string()));
        assert_eq!(result[1].role, "model");
        assert_eq!(result[1].parts[0].text, Some("Hi there!".to_string()));
    }

    #[test]
    fn test_convert_messages_with_system() {
        let model = ChatGemini::new();
        let messages = vec![Message::system("You are helpful"), Message::human("Hello")];

        // System messages are skipped in convert_messages (handled separately)
        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
    }

    #[test]
    fn test_convert_messages_empty() {
        let model = ChatGemini::new();
        let messages: Vec<BaseMessage> = vec![];

        let result = model.convert_messages(&messages).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_messages_unicode() {
        let model = ChatGemini::new();
        let messages = vec![
            Message::human("你好！"),
            Message::ai("こんにちは！🌸"),
        ];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result[0].parts[0].text, Some("你好！".to_string()));
        assert_eq!(result[1].parts[0].text, Some("こんにちは！🌸".to_string()));
    }

    #[test]
    fn test_convert_messages_multiline() {
        let model = ChatGemini::new();
        let messages = vec![Message::human("Line 1\nLine 2\nLine 3")];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(
            result[0].parts[0].text,
            Some("Line 1\nLine 2\nLine 3".to_string())
        );
    }

    #[test]
    fn test_convert_messages_with_tool_calls() {
        let model = ChatGemini::new();

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            args: serde_json::json!({"location": "NYC"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let ai_message = AIMessage::new(MessageContent::Text("I'll check the weather".to_string()))
            .with_tool_calls(vec![tool_call]);

        let messages = vec![
            Message::human("What's the weather?"),
            Message::from(ai_message),
        ];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 2);
        // AI message should have both text and function_call parts
        assert!(result[1].parts.len() >= 1);
    }

    #[test]
    fn test_convert_messages_tool_response() {
        let model = ChatGemini::new();

        let messages = vec![
            Message::human("What's the weather?"),
            Message::tool("72°F and sunny", "get_weather"),
        ];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].role, "function");
        assert!(result[1].parts[0].function_response.is_some());
    }

    #[test]
    fn test_convert_messages_only_system() {
        let model = ChatGemini::new();

        // System messages are filtered out in convert_messages
        let messages = vec![Message::system("You are helpful")];

        let result = model.convert_messages(&messages).unwrap();
        // System messages are skipped (handled separately)
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_messages_empty_ai_content() {
        let model = ChatGemini::new();

        // AI message with empty text but tool calls
        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "search".to_string(),
            args: serde_json::json!({"query": "test"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let ai_message =
            AIMessage::new(MessageContent::Text(String::new())).with_tool_calls(vec![tool_call]);

        let messages = vec![Message::from(ai_message)];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 1);
        // Should have function_call part but no text part
        assert!(result[0]
            .parts
            .iter()
            .any(|p| p.function_call.is_some()));
    }

    // ========================================================================
    // Safety Settings Tests
    // ========================================================================

    #[test]
    fn test_safety_settings() {
        let model = ChatGemini::new()
            .with_safety_setting(SafetySettings {
                category: SafetyCategory::HarmCategoryHarassment,
                threshold: SafetyThreshold::BlockMediumAndAbove,
            })
            .with_safety_setting(SafetySettings {
                category: SafetyCategory::HarmCategoryDangerousContent,
                threshold: SafetyThreshold::BlockOnlyHigh,
            });

        assert_eq!(model.safety_settings.len(), 2);
    }

    #[test]
    fn test_safety_settings_single() {
        let model = ChatGemini::new().with_safety_setting(SafetySettings {
            category: SafetyCategory::HarmCategoryViolence,
            threshold: SafetyThreshold::BlockNone,
        });

        assert_eq!(model.safety_settings.len(), 1);
    }

    #[test]
    fn test_safety_category_serialization() {
        let setting = SafetySettings {
            category: SafetyCategory::HarmCategoryHarassment,
            threshold: SafetyThreshold::BlockMediumAndAbove,
        };

        let json = serde_json::to_string(&setting).unwrap();
        assert!(json.contains("HARM_CATEGORY_HARASSMENT"));
        assert!(json.contains("BLOCK_MEDIUM_AND_ABOVE"));
    }

    #[test]
    fn test_all_safety_categories() {
        let categories = vec![
            SafetyCategory::HarmCategoryUnspecified,
            SafetyCategory::HarmCategoryDerogatory,
            SafetyCategory::HarmCategoryToxicity,
            SafetyCategory::HarmCategoryViolence,
            SafetyCategory::HarmCategorySexual,
            SafetyCategory::HarmCategoryMedical,
            SafetyCategory::HarmCategoryDangerous,
            SafetyCategory::HarmCategoryHarassment,
            SafetyCategory::HarmCategoryHateSpeech,
            SafetyCategory::HarmCategorySexuallyExplicit,
            SafetyCategory::HarmCategoryDangerousContent,
        ];

        for category in categories {
            let json = serde_json::to_string(&category).unwrap();
            assert!(json.starts_with('"'));
            assert!(json.contains("HARM_CATEGORY"));
        }
    }

    #[test]
    fn test_all_safety_thresholds() {
        let thresholds = vec![
            SafetyThreshold::HarmBlockThresholdUnspecified,
            SafetyThreshold::BlockLowAndAbove,
            SafetyThreshold::BlockMediumAndAbove,
            SafetyThreshold::BlockOnlyHigh,
            SafetyThreshold::BlockNone,
        ];

        for threshold in thresholds {
            let json = serde_json::to_string(&threshold).unwrap();
            assert!(json.starts_with('"'));
        }
    }

    #[test]
    fn test_safety_settings_clone() {
        let settings = SafetySettings {
            category: SafetyCategory::HarmCategoryViolence,
            threshold: SafetyThreshold::BlockOnlyHigh,
        };

        let cloned = settings.clone();
        let json1 = serde_json::to_string(&settings).unwrap();
        let json2 = serde_json::to_string(&cloned).unwrap();
        assert_eq!(json1, json2);
    }

    // ========================================================================
    // Request Building Tests
    // ========================================================================

    #[test]
    fn test_build_request_basic() {
        let model = ChatGemini::new()
            .with_api_key("test")
            .with_temperature(0.7);

        let messages = vec![Message::human("Hello")];
        let request = model.build_request(&messages, None, None).unwrap();

        assert!(!request.contents.is_empty());
        assert!(request.generation_config.is_some());
        let config = request.generation_config.unwrap();
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_build_request_with_stop() {
        let model = ChatGemini::new().with_api_key("test");

        let messages = vec![Message::human("Hello")];
        let stop = vec!["STOP".to_string(), "END".to_string()];
        let request = model.build_request(&messages, Some(&stop), None).unwrap();

        let config = request.generation_config.unwrap();
        assert_eq!(
            config.stop_sequences,
            Some(vec!["STOP".to_string(), "END".to_string()])
        );
    }

    #[test]
    fn test_build_request_with_tools() {
        let model = ChatGemini::new().with_api_key("test");

        let tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };

        let messages = vec![Message::human("What's the weather?")];
        let request = model
            .build_request(&messages, None, Some(&[tool]))
            .unwrap();

        assert!(request.tools.is_some());
        let tools = request.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function_declarations.len(), 1);
        assert_eq!(tools[0].function_declarations[0].name, "get_weather");
    }

    #[test]
    fn test_build_request_with_system_instruction() {
        let model = ChatGemini::new()
            .with_api_key("test")
            .with_system_instruction("You are a helpful assistant");

        let messages = vec![Message::human("Hello")];
        let request = model.build_request(&messages, None, None).unwrap();

        assert!(request.system_instruction.is_some());
        let instruction = request.system_instruction.unwrap();
        assert_eq!(instruction.parts[0].text, Some("You are a helpful assistant".to_string()));
    }

    #[test]
    fn test_build_request_system_from_messages() {
        let model = ChatGemini::new().with_api_key("test");

        let messages = vec![
            Message::system("Be concise"),
            Message::human("Hello"),
        ];
        let request = model.build_request(&messages, None, None).unwrap();

        // System instruction should be extracted from messages
        assert!(request.system_instruction.is_some());
    }

    #[test]
    fn test_build_request_with_safety_settings() {
        let model = ChatGemini::new()
            .with_api_key("test")
            .with_safety_setting(SafetySettings {
                category: SafetyCategory::HarmCategoryHarassment,
                threshold: SafetyThreshold::BlockMediumAndAbove,
            });

        let messages = vec![Message::human("Hello")];
        let request = model.build_request(&messages, None, None).unwrap();

        assert!(request.safety_settings.is_some());
        assert_eq!(request.safety_settings.unwrap().len(), 1);
    }

    #[test]
    fn test_build_request_no_safety_settings() {
        let model = ChatGemini::new().with_api_key("test");

        let messages = vec![Message::human("Hello")];
        let request = model.build_request(&messages, None, None).unwrap();

        assert!(request.safety_settings.is_none());
    }

    // ========================================================================
    // LLM Type and Other Tests
    // ========================================================================

    #[test]
    fn test_llm_type() {
        let model = ChatGemini::new();
        assert_eq!(model.llm_type(), "gemini");
    }

    #[test]
    fn test_thinking_mode() {
        let model = ChatGemini::new().with_thinking(true);
        assert!(model.enable_thinking);
    }

    #[test]
    fn test_as_any() {
        let model = ChatGemini::new();
        let any = model.as_any();
        assert!(any.downcast_ref::<ChatGemini>().is_some());
    }

    // ========================================================================
    // API Request Builder Tests
    // ========================================================================

    #[test]
    fn test_request_requires_api_key() {
        let model = ChatGemini {
            api_key: None,
            model: "gemini-pro".to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            system_instruction: None,
            safety_settings: Vec::new(),
            enable_thinking: false,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };

        let result = model.request("https://example.com");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("API key is required"));
    }

    #[test]
    fn test_request_with_api_key() {
        let model = ChatGemini::new().with_api_key("test-key");
        let result = model.request("https://example.com");
        assert!(result.is_ok());
    }

    // ========================================================================
    // Response Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_response_basic() {
        let model = ChatGemini::new();

        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: Some("Hello!".to_string()),
                        inline_data: None,
                        function_call: None,
                        function_response: None,
                    }],
                },
                finish_reason: Some("STOP".to_string()),
            }],
            usage_metadata: Some(GeminiUsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 5,
                total_token_count: 15,
            }),
        };

        let result = model.parse_response(response).unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(result.generations[0].message.as_text(), "Hello!");
    }

    #[test]
    fn test_parse_response_with_function_call() {
        let model = ChatGemini::new();

        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: None,
                        inline_data: None,
                        function_call: Some(GeminiFunctionCall {
                            name: "get_weather".to_string(),
                            args: serde_json::json!({"location": "NYC"}),
                        }),
                        function_response: None,
                    }],
                },
                finish_reason: Some("FUNCTION_CALL".to_string()),
            }],
            usage_metadata: None,
        };

        let result = model.parse_response(response).unwrap();
        assert_eq!(result.generations.len(), 1);

        // Should have tool calls
        if let Message::AI { tool_calls, .. } = &result.generations[0].message {
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "get_weather");
        }
    }

    #[test]
    fn test_parse_response_empty_candidates() {
        let model = ChatGemini::new();

        let response = GeminiResponse {
            candidates: vec![],
            usage_metadata: None,
        };

        let result = model.parse_response(response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No candidates"));
    }

    #[test]
    fn test_parse_response_multiple_parts() {
        let model = ChatGemini::new();

        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![
                        GeminiPart {
                            text: Some("Part 1".to_string()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        },
                        GeminiPart {
                            text: Some("Part 2".to_string()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        },
                    ],
                },
                finish_reason: Some("STOP".to_string()),
            }],
            usage_metadata: None,
        };

        let result = model.parse_response(response).unwrap();
        assert_eq!(result.generations[0].message.as_text(), "Part 1Part 2");
    }

    // ========================================================================
    // Builder Chaining Tests
    // ========================================================================

    #[test]
    fn test_full_builder_chain() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let model = ChatGemini::new()
            .with_api_key("sk-test")
            .with_model("gemini-2.0-flash-exp")
            .with_temperature(0.5)
            .with_max_tokens(2000)
            .with_top_p(0.95)
            .with_top_k(50)
            .with_system_instruction("Be helpful")
            .with_thinking(true)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter)
            .with_safety_setting(SafetySettings {
                category: SafetyCategory::HarmCategoryDangerousContent,
                threshold: SafetyThreshold::BlockMediumAndAbove,
            });

        assert_eq!(model.api_key, Some("sk-test".to_string()));
        assert_eq!(model.model, "gemini-2.0-flash-exp");
        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_tokens, Some(2000));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.top_k, Some(50));
        assert_eq!(model.system_instruction, Some("Be helpful".to_string()));
        assert!(model.enable_thinking);
        assert!(model.rate_limiter.is_some());
        assert_eq!(model.safety_settings.len(), 1);
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_with_api_key_special_chars() {
        let model = ChatGemini::new().with_api_key("sk-test_key@123!#$%");
        assert_eq!(model.api_key, Some("sk-test_key@123!#$%".to_string()));
    }

    #[test]
    fn test_with_model_with_version() {
        let model = ChatGemini::new().with_model("gemini-1.5-pro-002");
        assert_eq!(model.model, "gemini-1.5-pro-002");
    }

    #[test]
    fn test_convert_messages_long_conversation() {
        let model = ChatGemini::new();
        let mut messages = Vec::new();

        for i in 0..20 {
            if i % 2 == 0 {
                messages.push(Message::human(format!("Question {}", i)));
            } else {
                messages.push(Message::ai(format!("Answer {}", i)));
            }
        }

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn test_convert_messages_whitespace_only() {
        let model = ChatGemini::new();
        let messages = vec![Message::human("   \n\t  ")];

        let result = model.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].parts[0].text, Some("   \n\t  ".to_string()));
    }

    #[test]
    fn test_gemini_generation_config_default() {
        let config = GeminiGenerationConfig::default();
        assert!(config.temperature.is_none());
        assert!(config.max_output_tokens.is_none());
        assert!(config.top_p.is_none());
        assert!(config.top_k.is_none());
        assert!(config.stop_sequences.is_none());
    }

    #[test]
    fn test_gemini_request_serialization() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: Some("Hello".to_string()),
                    inline_data: None,
                    function_call: None,
                    function_response: None,
                }],
            }],
            generation_config: None,
            safety_settings: None,
            tools: None,
            system_instruction: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("contents"));
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_gemini_content_deserialization() {
        let json = r#"{"role": "model", "parts": [{"text": "Hi!"}]}"#;
        let content: GeminiContent = serde_json::from_str(json).unwrap();
        assert_eq!(content.role, "model");
        assert_eq!(content.parts[0].text, Some("Hi!".to_string()));
    }

    #[test]
    fn test_gemini_function_call_serialization() {
        let call = GeminiFunctionCall {
            name: "test_function".to_string(),
            args: serde_json::json!({"arg1": "value1"}),
        };

        let json = serde_json::to_string(&call).unwrap();
        assert!(json.contains("test_function"));
        assert!(json.contains("arg1"));
    }

    #[test]
    fn test_gemini_tool_serialization() {
        let tool = GeminiTool {
            function_declarations: vec![GeminiFunctionDeclaration {
                name: "search".to_string(),
                description: "Search for info".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            }],
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("functionDeclarations"));
        assert!(json.contains("search"));
    }
}
