//! Cohere chat models implementation
//!
//! This module provides integration with Cohere's Command series models via the Chat API.

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config::RunnableConfig,
    config_loader::env_vars::{
        cohere_api_base_url, env_string_or_default, COHERE_API_KEY, DEFAULT_COHERE_API_V1_PATH,
    },
    error::Error as DashFlowError,
    http_client,
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{BaseMessage, Message, ToolCall},
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
use tracing::warn;
use uuid::Uuid;

/// Cohere model names
pub mod models {
    pub const COMMAND_R_PLUS: &str = "command-r-plus";
    pub const COMMAND_R: &str = "command-r";
    pub const COMMAND: &str = "command";
    pub const COMMAND_LIGHT: &str = "command-light";
}

/// Request format for Cohere Chat API
#[derive(Debug, Clone, Serialize)]
struct CohereRequest {
    message: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_history: Option<Vec<CohereMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<CohereTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_results: Option<Vec<CohereToolResult>>,
}

/// Message in chat history
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CohereMessage {
    role: String,
    message: String,
}

/// Tool definition for Cohere API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CohereTool {
    name: String,
    description: String,
    parameter_definitions: serde_json::Value,
}

/// Tool result from previous tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CohereToolResult {
    call: CohereToolCall,
    outputs: Vec<serde_json::Value>,
}

/// Tool call information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CohereToolCall {
    name: String,
    parameters: serde_json::Value,
}

/// Response from Cohere Chat API
#[derive(Debug, Clone, Deserialize)]
struct CohereResponse {
    text: String,
    #[serde(default)]
    generation_id: Option<String>,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    meta: Option<CohereMetadata>,
    #[serde(default)]
    tool_calls: Option<Vec<CohereResponseToolCall>>,
}

/// Tool call in response
#[derive(Debug, Clone, Deserialize)]
struct CohereResponseToolCall {
    name: String,
    parameters: serde_json::Value,
}

/// Metadata about the response
#[derive(Debug, Clone, Deserialize)]
struct CohereMetadata {
    #[serde(default)]
    billed_units: Option<CohereBilledUnits>,
}

/// Billed units for usage tracking
#[derive(Debug, Clone, Deserialize)]
struct CohereBilledUnits {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

/// Streaming response chunk from Cohere
#[derive(Debug, Clone, Deserialize)]
struct CohereStreamChunk {
    /// Event type discriminator from Cohere API (e.g., "text-generation", "stream-end")
    #[serde(rename = "type")]
    #[allow(dead_code)] // Deserialize: Cohere streaming event type - must match API schema
    event_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    finish_reason: Option<String>,
    /// Generation ID from Cohere API
    #[serde(default)]
    #[allow(dead_code)] // Deserialize: Cohere generation ID - reserved for request correlation
    generation_id: Option<String>,
}

/// Cohere chat model client
#[derive(Clone)]
pub struct ChatCohere {
    /// API key for authentication
    api_key: String,
    /// Model name to use
    model: String,
    /// Base URL for API (defaults to <https://api.cohere.ai/v1>)
    base_url: String,
    /// Temperature parameter
    temperature: Option<f32>,
    /// Maximum tokens to generate
    max_tokens: Option<u32>,
    /// Top-k sampling
    k: Option<u32>,
    /// Top-p (nucleus) sampling
    p: Option<f32>,
    /// Stop sequences
    stop_sequences: Option<Vec<String>>,
    /// Frequency penalty
    frequency_penalty: Option<f32>,
    /// Presence penalty
    presence_penalty: Option<f32>,
    /// HTTP client
    client: reqwest::Client,
    /// Rate limiter
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl Default for ChatCohere {
    #[allow(clippy::expect_used)] // Default must be infallible; use try_new() for fallible creation
    fn default() -> Self {
        Self::try_new().expect("Failed to create HTTP client for ChatCohere")
    }
}

impl ChatCohere {
    /// Try to create a new `ChatCohere` instance.
    ///
    /// Reads API key from `COHERE_API_KEY` environment variable if set.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> Result<Self, DashFlowError> {
        Ok(Self {
            api_key: env_string_or_default(COHERE_API_KEY, ""),
            model: models::COMMAND_R_PLUS.to_string(),
            // Use centralized URL helper with env var override support
            base_url: format!("{}{}", cohere_api_base_url(), DEFAULT_COHERE_API_V1_PATH),
            temperature: None,
            max_tokens: None,
            k: None,
            p: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            client: http_client::create_llm_client()?,
            rate_limiter: None,
        })
    }

    /// Create a new `ChatCohere` instance
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new()` for fallible creation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Set the model name
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the base URL
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set top-k sampling
    #[must_use]
    pub fn with_k(mut self, k: u32) -> Self {
        self.k = Some(k);
        self
    }

    /// Set top-p sampling
    #[must_use]
    pub fn with_p(mut self, p: f32) -> Self {
        self.p = Some(p);
        self
    }

    /// Set stop sequences
    #[must_use]
    pub fn with_stop_sequences(mut self, stop_sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(stop_sequences);
        self
    }

    /// Set frequency penalty
    #[must_use]
    pub fn with_frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.frequency_penalty = Some(frequency_penalty);
        self
    }

    /// Set presence penalty
    #[must_use]
    pub fn with_presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.presence_penalty = Some(presence_penalty);
        self
    }

    /// Set rate limiter
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Convert `DashFlow` messages to Cohere format
    fn messages_to_cohere(
        &self,
        messages: &[BaseMessage],
    ) -> Result<(String, Option<Vec<CohereMessage>>), DashFlowError> {
        if messages.is_empty() {
            return Err(DashFlowError::invalid_input("Messages cannot be empty"));
        }

        // Cohere's API expects the last message as the "message" field
        // and previous messages as "chat_history"
        let last_message = &messages[messages.len() - 1];
        let current_message = last_message.content().as_text();

        // Convert previous messages to chat history
        let chat_history = if messages.len() > 1 {
            let history: Vec<CohereMessage> = messages[..messages.len() - 1]
                .iter()
                .map(|msg| CohereMessage {
                    role: match msg {
                        Message::Human { .. } => "USER".to_string(),
                        Message::AI { .. } => "CHATBOT".to_string(),
                        Message::System { .. } => "SYSTEM".to_string(),
                        Message::Tool { .. } => "TOOL".to_string(),
                        _ => "USER".to_string(),
                    },
                    message: msg.content().as_text(),
                })
                .collect();
            Some(history)
        } else {
            None
        };

        Ok((current_message, chat_history))
    }

    /// Convert `DashFlow` `ToolDefinition` to Cohere `CohereTool`
    fn convert_tool_definition(tool: &ToolDefinition) -> CohereTool {
        CohereTool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameter_definitions: tool.parameters.clone(),
        }
    }

    /// Make API call to Cohere
    async fn call_api(&self, request: CohereRequest) -> Result<CohereResponse, DashFlowError> {
        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let url = format!("{}/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| DashFlowError::network(format!("Failed to send request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DashFlowError::api(format!(
                "Cohere API error ({status}): {error_text}"
            )));
        }

        response
            .json::<CohereResponse>()
            .await
            .map_err(|e| DashFlowError::api_format(format!("Failed to parse Cohere response: {e}")))
    }
}

#[async_trait]
impl ChatModel for ChatCohere {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult, DashFlowError> {
        let (current_message, chat_history) = self.messages_to_cohere(messages)?;

        // Stop sequences: prefer parameter over struct field
        let stop_sequences = if let Some(stop_seqs) = stop {
            Some(stop_seqs.to_vec())
        } else {
            self.stop_sequences.clone()
        };

        // Tools: prefer parameter over struct field (Cohere doesn't support tool_choice)
        let cohere_tools = tools.map(|tool_defs| {
            tool_defs
                .iter()
                .map(Self::convert_tool_definition)
                .collect()
        });

        // Log warning if tool_choice is specified (Cohere doesn't support this parameter)
        if tool_choice.is_some() {
            warn!("Cohere API does not support tool_choice parameter, ignoring");
        }

        let request = CohereRequest {
            message: current_message,
            model: self.model.clone(),
            chat_history,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            k: self.k,
            p: self.p,
            stop_sequences,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            tools: cohere_tools,
            tool_results: None,
        };

        let response = self.call_api(request).await?;

        // Extract usage metadata
        let usage_metadata = response.meta.as_ref().and_then(|meta| {
            meta.billed_units.as_ref().map(|units| UsageMetadata {
                input_tokens: units.input_tokens.unwrap_or(0),
                output_tokens: units.output_tokens.unwrap_or(0),
                total_tokens: units.input_tokens.unwrap_or(0) + units.output_tokens.unwrap_or(0),
                input_token_details: None,
                output_token_details: None,
            })
        });

        // Convert tool calls if present
        let tool_calls: Vec<ToolCall> = response
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|call| ToolCall {
                        id: Uuid::new_v4().to_string(),
                        name: call.name.clone(),
                        args: call.parameters.clone(),
                        tool_type: "tool_call".to_string(),
                        index: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Build response metadata
        let mut response_metadata = std::collections::HashMap::new();
        if let Some(gen_id) = &response.generation_id {
            response_metadata.insert("generation_id".to_string(), serde_json::json!(gen_id));
        }
        if let Some(reason) = &response.finish_reason {
            response_metadata.insert("finish_reason".to_string(), serde_json::json!(reason));
        }

        // Create AI message
        use dashflow::core::messages::AIMessage;
        let mut ai_message = AIMessage::new(response.text.clone());
        if let Some(usage) = usage_metadata {
            ai_message = ai_message.with_usage(usage);
        }
        ai_message = ai_message.with_tool_calls(tool_calls);

        // Convert to Message and add response metadata
        let mut message = Message::from(ai_message);
        message.fields_mut().response_metadata = response_metadata;

        // Build generation_info
        let mut generation_info = std::collections::HashMap::new();
        if let Some(reason) = &response.finish_reason {
            generation_info.insert("finish_reason".to_string(), serde_json::json!(reason));
        }

        let generation = ChatGeneration {
            message,
            generation_info: Some(generation_info),
        };

        let mut llm_output = std::collections::HashMap::new();
        llm_output.insert("model".to_string(), serde_json::json!(self.model));

        Ok(ChatResult {
            generations: vec![generation],
            llm_output: Some(llm_output),
        })
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk, DashFlowError>> + Send>>,
        DashFlowError,
    > {
        let (current_message, chat_history) = self.messages_to_cohere(messages)?;

        // Stop sequences: prefer parameter over struct field
        let stop_sequences = if let Some(stop_seqs) = stop {
            Some(stop_seqs.to_vec())
        } else {
            self.stop_sequences.clone()
        };

        // Tools: prefer parameter over struct field (Cohere doesn't support tool_choice)
        let cohere_tools = tools.map(|tool_defs| {
            tool_defs
                .iter()
                .map(Self::convert_tool_definition)
                .collect()
        });

        // Log warning if tool_choice is specified (Cohere doesn't support this parameter)
        if tool_choice.is_some() {
            warn!("Cohere API does not support tool_choice parameter, ignoring");
        }

        let request = CohereRequest {
            message: current_message,
            model: self.model.clone(),
            chat_history,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            k: self.k,
            p: self.p,
            stop_sequences,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            tools: cohere_tools,
            tool_results: None,
        };

        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let url = format!("{}/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request)
            .send()
            .await
            .map_err(|e| DashFlowError::network(format!("Failed to send request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DashFlowError::api(format!(
                "Cohere API error ({status}): {error_text}"
            )));
        }

        let event_stream = response.bytes_stream().eventsource();

        let stream = stream! {
            tokio::pin!(event_stream);
            while let Some(event) = event_stream.next().await {
                match event {
                    Ok(event) => {
                        if event.event == "text-generation" || event.event == "stream-end" {
                            if let Ok(chunk) = serde_json::from_str::<CohereStreamChunk>(&event.data) {
                                if let Some(text) = chunk.text {
                                    use dashflow::core::messages::AIMessageChunk;
                                    let ai_chunk = AIMessageChunk::new(text);

                                    let mut generation_info = std::collections::HashMap::new();
                                    if let Some(reason) = chunk.finish_reason {
                                        generation_info.insert("finish_reason".to_string(), serde_json::json!(reason));
                                    }

                                    yield Ok(ChatGenerationChunk {
                                        message: ai_chunk,
                                        generation_info: Some(generation_info),
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(DashFlowError::network(format!("Stream error: {e}")));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn llm_type(&self) -> &'static str {
        "cohere"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.rate_limiter.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Serializable for ChatCohere {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "cohere".to_string(),
            "ChatCohere".to_string(),
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

        if let Some(k) = self.k {
            kwargs.insert("k".to_string(), serde_json::json!(k));
        }

        if let Some(p) = self.p {
            kwargs.insert("p".to_string(), serde_json::json!(p));
        }

        if let Some(ref stop_seqs) = self.stop_sequences {
            if !stop_seqs.is_empty() {
                kwargs.insert("stop_sequences".to_string(), serde_json::json!(stop_seqs));
            }
        }

        if let Some(freq_pen) = self.frequency_penalty {
            kwargs.insert("frequency_penalty".to_string(), serde_json::json!(freq_pen));
        }

        if let Some(pres_pen) = self.presence_penalty {
            kwargs.insert("presence_penalty".to_string(), serde_json::json!(pres_pen));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        secrets.insert("api_key".to_string(), "COHERE_API_KEY".to_string());
        secrets
    }
}

#[async_trait]
impl Runnable for ChatCohere {
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)]
mod tests {
    use super::*;
    use dashflow::core::language_models::ToolDefinition;

    // ========== Model constants tests ==========

    #[test]
    fn test_model_constants() {
        assert_eq!(models::COMMAND_R_PLUS, "command-r-plus");
        assert_eq!(models::COMMAND_R, "command-r");
        assert_eq!(models::COMMAND, "command");
        assert_eq!(models::COMMAND_LIGHT, "command-light");
    }

    // ========== Builder method tests ==========

    #[test]
    fn test_builder_with_model() {
        let model = ChatCohere::new().with_model("command-r");
        assert_eq!(model.model, "command-r");
    }

    #[test]
    fn test_builder_with_api_key() {
        let model = ChatCohere::new().with_api_key("test-key");
        assert_eq!(model.api_key, "test-key");
    }

    #[test]
    fn test_builder_with_base_url() {
        let model = ChatCohere::new().with_base_url("https://custom.api.com");
        assert_eq!(model.base_url, "https://custom.api.com");
    }

    #[test]
    fn test_builder_with_temperature() {
        let model = ChatCohere::new().with_temperature(0.7);
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    fn test_builder_with_max_tokens() {
        let model = ChatCohere::new().with_max_tokens(100);
        assert_eq!(model.max_tokens, Some(100));
    }

    #[test]
    fn test_builder_with_k() {
        let model = ChatCohere::new().with_k(40);
        assert_eq!(model.k, Some(40));
    }

    #[test]
    fn test_builder_with_p() {
        let model = ChatCohere::new().with_p(0.9);
        assert_eq!(model.p, Some(0.9));
    }

    #[test]
    fn test_builder_with_stop_sequences() {
        let model = ChatCohere::new().with_stop_sequences(vec!["END".to_string()]);
        assert_eq!(model.stop_sequences, Some(vec!["END".to_string()]));
    }

    #[test]
    fn test_builder_with_frequency_penalty() {
        let model = ChatCohere::new().with_frequency_penalty(0.5);
        assert_eq!(model.frequency_penalty, Some(0.5));
    }

    #[test]
    fn test_builder_with_presence_penalty() {
        let model = ChatCohere::new().with_presence_penalty(0.3);
        assert_eq!(model.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_builder_chained() {
        let model = ChatCohere::new()
            .with_model("command")
            .with_temperature(0.5)
            .with_max_tokens(200)
            .with_k(50)
            .with_p(0.95);

        assert_eq!(model.model, "command");
        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_tokens, Some(200));
        assert_eq!(model.k, Some(50));
        assert_eq!(model.p, Some(0.95));
    }

    // ========== messages_to_cohere tests ==========

    #[test]
    fn test_messages_to_cohere_single_message() {
        let model = ChatCohere::new();
        let messages = vec![Message::human("Hello!")];

        let (current, history) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Hello!");
        assert!(history.is_none());
    }

    #[test]
    fn test_messages_to_cohere_with_history() {
        let model = ChatCohere::new();
        let messages = vec![
            Message::human("First message"),
            Message::AI {
                content: dashflow::core::messages::MessageContent::Text(
                    "Response".to_string(),
                ),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
            Message::human("Second message"),
        ];

        let (current, history) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Second message");
        let history = history.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "USER");
        assert_eq!(history[0].message, "First message");
        assert_eq!(history[1].role, "CHATBOT");
        assert_eq!(history[1].message, "Response");
    }

    #[test]
    fn test_messages_to_cohere_system_message() {
        let model = ChatCohere::new();
        let messages = vec![
            Message::system("You are helpful"),
            Message::human("Hello!"),
        ];

        let (current, history) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Hello!");
        let history = history.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, "SYSTEM");
    }

    #[test]
    fn test_messages_to_cohere_empty_returns_error() {
        let model = ChatCohere::new();
        let messages: Vec<Message> = vec![];

        let result = model.messages_to_cohere(&messages);
        assert!(result.is_err());
    }

    // ========== convert_tool_definition tests ==========

    #[test]
    fn test_convert_tool_definition() {
        let tool = ToolDefinition {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        };

        let result = ChatCohere::convert_tool_definition(&tool);

        assert_eq!(result.name, "search");
        assert_eq!(result.description, "Search the web");
    }

    // ========== llm_type test ==========

    #[test]
    fn test_llm_type() {
        let model = ChatCohere::new();
        assert_eq!(model.llm_type(), "cohere");
    }

    // ========== Serializable tests ==========

    #[test]
    fn test_lc_id() {
        let model = ChatCohere::new();
        let id = model.lc_id();

        assert_eq!(id.len(), 4);
        assert_eq!(id[0], "dashflow");
        assert_eq!(id[1], "chat_models");
        assert_eq!(id[2], "cohere");
        assert_eq!(id[3], "ChatCohere");
    }

    #[test]
    fn test_is_lc_serializable() {
        let model = ChatCohere::new();
        assert!(model.is_lc_serializable());
    }

    #[test]
    fn test_to_json_basic() {
        let model = ChatCohere::new().with_model("command");
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                assert_eq!(kwargs.get("model").unwrap(), "command");
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_with_options() {
        let model = ChatCohere::new()
            .with_model("command-r")
            .with_temperature(0.7)
            .with_max_tokens(100)
            .with_k(40)
            .with_p(0.9)
            .with_stop_sequences(vec!["END".to_string()])
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                // Use f64 comparison for floating point values
                let temp = kwargs.get("temperature").unwrap().as_f64().unwrap();
                assert!((temp - 0.7).abs() < 0.01);
                assert_eq!(kwargs.get("max_tokens").unwrap(), 100);
                assert_eq!(kwargs.get("k").unwrap(), 40);
                let p = kwargs.get("p").unwrap().as_f64().unwrap();
                assert!((p - 0.9).abs() < 0.01);
                assert_eq!(
                    kwargs.get("stop_sequences").unwrap(),
                    &serde_json::json!(["END"])
                );
                let freq = kwargs.get("frequency_penalty").unwrap().as_f64().unwrap();
                assert!((freq - 0.5).abs() < 0.01);
                let pres = kwargs.get("presence_penalty").unwrap().as_f64().unwrap();
                assert!((pres - 0.3).abs() < 0.01);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_empty_stop_sequences_not_included() {
        let model = ChatCohere::new().with_stop_sequences(vec![]);
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                // Empty stop_sequences should not be included
                assert!(kwargs.get("stop_sequences").is_none());
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_lc_secrets() {
        let model = ChatCohere::new();
        let secrets = model.lc_secrets();

        assert_eq!(secrets.get("api_key").unwrap(), "COHERE_API_KEY");
    }

    // ========== Default trait test ==========

    #[test]
    fn test_default() {
        let model = ChatCohere::default();
        assert_eq!(model.model, models::COMMAND_R_PLUS);
        // Default URL now uses centralized constant (api.cohere.com, not api.cohere.ai)
        assert_eq!(model.base_url, "https://api.cohere.com/v1");
    }

    // ========== try_new test ==========

    #[test]
    fn test_try_new() {
        let result = ChatCohere::try_new();
        assert!(result.is_ok());
    }

    // ========== Additional builder pattern tests ==========

    #[test]
    fn test_builder_with_all_models() {
        for model_name in &[
            models::COMMAND_R_PLUS,
            models::COMMAND_R,
            models::COMMAND,
            models::COMMAND_LIGHT,
        ] {
            let model = ChatCohere::new().with_model(*model_name);
            assert_eq!(model.model, *model_name);
        }
    }

    #[test]
    fn test_builder_temperature_zero() {
        let model = ChatCohere::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_builder_temperature_one() {
        let model = ChatCohere::new().with_temperature(1.0);
        assert_eq!(model.temperature, Some(1.0));
    }

    #[test]
    fn test_builder_max_tokens_zero() {
        let model = ChatCohere::new().with_max_tokens(0);
        assert_eq!(model.max_tokens, Some(0));
    }

    #[test]
    fn test_builder_max_tokens_large() {
        let model = ChatCohere::new().with_max_tokens(u32::MAX);
        assert_eq!(model.max_tokens, Some(u32::MAX));
    }

    #[test]
    fn test_builder_k_zero() {
        let model = ChatCohere::new().with_k(0);
        assert_eq!(model.k, Some(0));
    }

    #[test]
    fn test_builder_p_zero() {
        let model = ChatCohere::new().with_p(0.0);
        assert_eq!(model.p, Some(0.0));
    }

    #[test]
    fn test_builder_p_one() {
        let model = ChatCohere::new().with_p(1.0);
        assert_eq!(model.p, Some(1.0));
    }

    #[test]
    fn test_builder_frequency_penalty_zero() {
        let model = ChatCohere::new().with_frequency_penalty(0.0);
        assert_eq!(model.frequency_penalty, Some(0.0));
    }

    #[test]
    fn test_builder_presence_penalty_zero() {
        let model = ChatCohere::new().with_presence_penalty(0.0);
        assert_eq!(model.presence_penalty, Some(0.0));
    }

    #[test]
    fn test_builder_multiple_stop_sequences() {
        let stops = vec![
            "END".to_string(),
            "STOP".to_string(),
            "DONE".to_string(),
        ];
        let model = ChatCohere::new().with_stop_sequences(stops.clone());
        assert_eq!(model.stop_sequences, Some(stops));
    }

    #[test]
    fn test_builder_empty_stop_sequences() {
        let model = ChatCohere::new().with_stop_sequences(vec![]);
        assert_eq!(model.stop_sequences, Some(vec![]));
    }

    #[test]
    fn test_builder_api_key_with_special_chars() {
        let model = ChatCohere::new().with_api_key("key-with-dashes_and_underscores.and.dots");
        assert_eq!(
            model.api_key,
            "key-with-dashes_and_underscores.and.dots"
        );
    }

    #[test]
    fn test_builder_empty_api_key() {
        let model = ChatCohere::new().with_api_key("");
        assert_eq!(model.api_key, "");
    }

    #[test]
    fn test_builder_base_url_with_trailing_slash() {
        let model = ChatCohere::new().with_base_url("https://api.example.com/");
        assert_eq!(model.base_url, "https://api.example.com/");
    }

    #[test]
    fn test_builder_model_custom_name() {
        let model = ChatCohere::new().with_model("custom-model-v1");
        assert_eq!(model.model, "custom-model-v1");
    }

    #[test]
    fn test_builder_string_ownership() {
        let api_key = String::from("owned-key");
        let model = ChatCohere::new().with_api_key(api_key);
        assert_eq!(model.api_key, "owned-key");
    }

    #[test]
    fn test_builder_model_string_ownership() {
        let model_name = String::from("owned-model");
        let model = ChatCohere::new().with_model(model_name);
        assert_eq!(model.model, "owned-model");
    }

    // ========== Message conversion edge case tests ==========

    #[test]
    fn test_messages_to_cohere_tool_message() {
        let model = ChatCohere::new();
        let messages = vec![
            Message::human("Call the tool"),
            Message::Tool {
                content: dashflow::core::messages::MessageContent::Text(
                    "Tool result".to_string(),
                ),
                tool_call_id: "call_123".to_string(),
                status: None,
                artifact: None,
                fields: Default::default(),
            },
            Message::human("What's the result?"),
        ];

        let (current, history) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "What's the result?");
        let history = history.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "USER");
        assert_eq!(history[1].role, "TOOL");
    }

    #[test]
    fn test_messages_to_cohere_multiple_turns() {
        let model = ChatCohere::new();
        let messages = vec![
            Message::human("First"),
            Message::AI {
                content: dashflow::core::messages::MessageContent::Text("AI 1".to_string()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
            Message::human("Second"),
            Message::AI {
                content: dashflow::core::messages::MessageContent::Text("AI 2".to_string()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
            Message::human("Third"),
        ];

        let (current, history) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Third");
        let history = history.unwrap();
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].message, "First");
        assert_eq!(history[1].message, "AI 1");
        assert_eq!(history[2].message, "Second");
        assert_eq!(history[3].message, "AI 2");
    }

    #[test]
    fn test_messages_to_cohere_unicode_content() {
        let model = ChatCohere::new();
        let messages = vec![Message::human("Hello 世界! 🌍 Привет мир")];

        let (current, _) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Hello 世界! 🌍 Привет мир");
    }

    #[test]
    fn test_messages_to_cohere_long_message() {
        let model = ChatCohere::new();
        let long_text = "a".repeat(10000);
        let messages = vec![Message::human(long_text.as_str())];

        let (current, _) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, long_text);
    }

    #[test]
    fn test_messages_to_cohere_newlines() {
        let model = ChatCohere::new();
        let messages = vec![Message::human("Line 1\nLine 2\nLine 3")];

        let (current, _) = model.messages_to_cohere(&messages).unwrap();

        assert_eq!(current, "Line 1\nLine 2\nLine 3");
    }

    // ========== Tool definition conversion tests ==========

    #[test]
    fn test_convert_tool_definition_empty_description() {
        let tool = ToolDefinition {
            name: "empty_desc".to_string(),
            description: "".to_string(),
            parameters: serde_json::json!({}),
        };

        let result = ChatCohere::convert_tool_definition(&tool);

        assert_eq!(result.name, "empty_desc");
        assert_eq!(result.description, "");
    }

    #[test]
    fn test_convert_tool_definition_complex_parameters() {
        let tool = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A complex tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "User name"},
                    "age": {"type": "integer", "minimum": 0},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["name"]
            }),
        };

        let result = ChatCohere::convert_tool_definition(&tool);

        assert_eq!(result.name, "complex_tool");
        assert!(result.parameter_definitions.get("properties").is_some());
    }

    #[test]
    fn test_convert_tool_definition_unicode_name() {
        let tool = ToolDefinition {
            name: "工具_εργαλείο".to_string(),
            description: "Unicode tool".to_string(),
            parameters: serde_json::json!({}),
        };

        let result = ChatCohere::convert_tool_definition(&tool);

        assert_eq!(result.name, "工具_εργαλείο");
    }

    // ========== Clone trait tests ==========

    #[test]
    fn test_clone() {
        let model = ChatCohere::new()
            .with_model("command-r")
            .with_temperature(0.7)
            .with_max_tokens(100);

        let cloned = model.clone();

        assert_eq!(cloned.model, "command-r");
        assert_eq!(cloned.temperature, Some(0.7));
        assert_eq!(cloned.max_tokens, Some(100));
    }

    // ========== as_any tests ==========

    #[test]
    fn test_as_any_downcast() {
        let model = ChatCohere::new().with_model("command");
        let any_ref: &dyn std::any::Any = model.as_any();

        let downcast = any_ref.downcast_ref::<ChatCohere>();
        assert!(downcast.is_some());
        assert_eq!(downcast.unwrap().model, "command");
    }

    // ========== Request serialization tests ==========

    #[test]
    fn test_cohere_request_serialization() {
        let request = CohereRequest {
            message: "Hello".to_string(),
            model: "command-r".to_string(),
            chat_history: None,
            temperature: Some(0.5),
            max_tokens: Some(100),
            k: None,
            p: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            tools: None,
            tool_results: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"message\":\"Hello\""));
        assert!(json.contains("\"model\":\"command-r\""));
        assert!(json.contains("\"temperature\":0.5"));
        assert!(json.contains("\"max_tokens\":100"));
        // None fields should not be serialized
        assert!(!json.contains("\"chat_history\""));
        assert!(!json.contains("\"k\""));
    }

    #[test]
    fn test_cohere_request_with_chat_history() {
        let request = CohereRequest {
            message: "Current".to_string(),
            model: "command".to_string(),
            chat_history: Some(vec![
                CohereMessage {
                    role: "USER".to_string(),
                    message: "Previous".to_string(),
                },
            ]),
            temperature: None,
            max_tokens: None,
            k: None,
            p: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            tools: None,
            tool_results: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"chat_history\""));
        assert!(json.contains("\"role\":\"USER\""));
    }

    #[test]
    fn test_cohere_request_with_tools() {
        let request = CohereRequest {
            message: "Use tool".to_string(),
            model: "command-r-plus".to_string(),
            chat_history: None,
            temperature: None,
            max_tokens: None,
            k: None,
            p: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            tools: Some(vec![CohereTool {
                name: "search".to_string(),
                description: "Search the web".to_string(),
                parameter_definitions: serde_json::json!({"type": "object"}),
            }]),
            tool_results: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"name\":\"search\""));
    }

    // ========== Response deserialization tests ==========

    #[test]
    fn test_cohere_response_deserialization() {
        let json = r#"{
            "text": "Hello world",
            "generation_id": "gen-123",
            "finish_reason": "COMPLETE"
        }"#;

        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.text, "Hello world");
        assert_eq!(response.generation_id, Some("gen-123".to_string()));
        assert_eq!(response.finish_reason, Some("COMPLETE".to_string()));
    }

    #[test]
    fn test_cohere_response_minimal() {
        let json = r#"{"text": "Just text"}"#;

        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.text, "Just text");
        assert!(response.generation_id.is_none());
        assert!(response.finish_reason.is_none());
    }

    #[test]
    fn test_cohere_response_with_metadata() {
        let json = r#"{
            "text": "Hello",
            "meta": {
                "billed_units": {
                    "input_tokens": 10,
                    "output_tokens": 5
                }
            }
        }"#;

        let response: CohereResponse = serde_json::from_str(json).unwrap();
        let meta = response.meta.unwrap();
        let units = meta.billed_units.unwrap();
        assert_eq!(units.input_tokens, Some(10));
        assert_eq!(units.output_tokens, Some(5));
    }

    #[test]
    fn test_cohere_response_with_tool_calls() {
        let json = r#"{
            "text": "",
            "tool_calls": [
                {
                    "name": "search",
                    "parameters": {"query": "test"}
                }
            ]
        }"#;

        let response: CohereResponse = serde_json::from_str(json).unwrap();
        let tool_calls = response.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
    }

    // ========== Stream chunk tests ==========

    #[test]
    fn test_cohere_stream_chunk_text_generation() {
        let json = r#"{
            "type": "text-generation",
            "text": "Hello"
        }"#;

        let chunk: CohereStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.event_type, "text-generation");
        assert_eq!(chunk.text, Some("Hello".to_string()));
    }

    #[test]
    fn test_cohere_stream_chunk_stream_end() {
        let json = r#"{
            "type": "stream-end",
            "finish_reason": "COMPLETE",
            "generation_id": "gen-456"
        }"#;

        let chunk: CohereStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.event_type, "stream-end");
        assert_eq!(chunk.finish_reason, Some("COMPLETE".to_string()));
    }

    // ========== Serializable trait additional tests ==========

    #[test]
    fn test_to_json_only_model() {
        let model = ChatCohere::new();
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { kwargs, .. } => {
                // Only model should be present
                assert!(kwargs.get("model").is_some());
                // Temperature not set, should be absent
                assert!(kwargs.get("temperature").is_none());
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_serialization_version() {
        let model = ChatCohere::new();
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { lc, .. } => {
                assert_eq!(lc, SERIALIZATION_VERSION);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_to_json_lc_id_in_serialization() {
        let model = ChatCohere::new();
        let json = model.to_json();

        match json {
            SerializedObject::Constructor { id, .. } => {
                assert_eq!(id, vec![
                    "dashflow".to_string(),
                    "chat_models".to_string(),
                    "cohere".to_string(),
                    "ChatCohere".to_string(),
                ]);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    // ========== CohereMessage tests ==========

    #[test]
    fn test_cohere_message_serialization() {
        let message = CohereMessage {
            role: "USER".to_string(),
            message: "Hello".to_string(),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"role\":\"USER\""));
        assert!(json.contains("\"message\":\"Hello\""));
    }

    #[test]
    fn test_cohere_message_deserialization() {
        let json = r#"{"role": "CHATBOT", "message": "Hi there"}"#;
        let message: CohereMessage = serde_json::from_str(json).unwrap();
        assert_eq!(message.role, "CHATBOT");
        assert_eq!(message.message, "Hi there");
    }

    // ========== CohereTool tests ==========

    #[test]
    fn test_cohere_tool_serialization() {
        let tool = CohereTool {
            name: "calculator".to_string(),
            description: "Perform math".to_string(),
            parameter_definitions: serde_json::json!({"type": "object"}),
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"name\":\"calculator\""));
        assert!(json.contains("\"description\":\"Perform math\""));
    }

    // ========== CohereToolResult tests ==========

    #[test]
    fn test_cohere_tool_result_serialization() {
        let tool_result = CohereToolResult {
            call: CohereToolCall {
                name: "search".to_string(),
                parameters: serde_json::json!({"query": "test"}),
            },
            outputs: vec![serde_json::json!({"result": "found"})],
        };

        let json = serde_json::to_string(&tool_result).unwrap();
        assert!(json.contains("\"call\""));
        assert!(json.contains("\"outputs\""));
    }

    // ========== Default values verification ==========

    #[test]
    fn test_default_none_values() {
        let model = ChatCohere::new();
        assert!(model.temperature.is_none());
        assert!(model.max_tokens.is_none());
        assert!(model.k.is_none());
        assert!(model.p.is_none());
        assert!(model.stop_sequences.is_none());
        assert!(model.frequency_penalty.is_none());
        assert!(model.presence_penalty.is_none());
        assert!(model.rate_limiter.is_none());
    }

    // ========== rate_limiter tests ==========

    #[test]
    fn test_rate_limiter_returns_none_by_default() {
        let model = ChatCohere::new();
        assert!(model.rate_limiter().is_none());
    }
}
