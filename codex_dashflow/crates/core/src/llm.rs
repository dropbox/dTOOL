//! LLM Provider using DashFlow LLM crates
//!
//! This module provides a unified LLM interface using DashFlow's provider crates:
//! - `dashflow-openai` for OpenAI GPT models (API key mode)
//! - `dashflow-anthropic` for Claude models
//!
//! For ChatGPT OAuth users, this module implements a direct HTTP client that
//! communicates with `https://chatgpt.com/backend-api/codex` using the
//! `chatgpt-account-id` header.
//!
//! Following DashFlow's patterns for LLM integration.

use async_openai::config::OpenAIConfig;
use dashflow::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use dashflow::core::messages::{
    BaseMessageFields, Message, MessageContent, ToolCall as DashflowToolCall,
};
use dashflow_anthropic::ChatAnthropic;
use dashflow_openai::ChatOpenAI;
use serde::{Deserialize, Serialize};

use crate::state::{Message as AgentMessage, MessageRole, ToolCall};
use crate::streaming::{AgentEvent, StreamCallback};
use crate::tools::ToolDefinition as AgentToolDefinition;
use crate::Result;
use std::sync::Arc;

/// LLM provider type
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[default]
    OpenAI,
    Anthropic,
}

/// Authentication mode for LLM calls
///
/// ChatGPT OAuth tokens are scoped for `chatgpt.com/backend-api/codex` and
/// require the `chatgpt-account-id` header. API keys use the standard
/// `api.openai.com/v1` endpoint.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// Standard OpenAI API key authentication (api.openai.com/v1)
    #[default]
    ApiKey,
    /// ChatGPT OAuth authentication (chatgpt.com/backend-api/codex)
    ChatGpt,
}

/// ChatGPT backend configuration for OAuth users
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatGptConfig {
    /// The access token from OAuth flow
    pub access_token: String,
    /// The account ID for the chatgpt-account-id header
    pub account_id: String,
    /// Base URL for ChatGPT backend (default: `https://chatgpt.com/backend-api/codex`)
    pub base_url: Option<String>,
}

impl ChatGptConfig {
    /// Get the base URL, using default if not set
    pub fn base_url(&self) -> &str {
        self.base_url
            .as_deref()
            .unwrap_or("https://chatgpt.com/backend-api/codex")
    }
}

/// Configuration for the LLM provider
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider to use (openai or anthropic)
    pub provider: LlmProvider,
    /// Model name (e.g., "gpt-4", "gpt-4o-mini", "claude-3-5-sonnet-latest")
    pub model: String,
    /// Sampling temperature (0.0 to 2.0)
    pub temperature: Option<f32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// API base URL (for OpenAI-compatible providers)
    pub api_base: Option<String>,
    /// Authentication mode (ApiKey or ChatGpt)
    #[serde(default)]
    pub auth_mode: AuthMode,
    /// ChatGPT backend configuration (required when auth_mode is ChatGpt)
    #[serde(default)]
    pub chatgpt_config: Option<ChatGptConfig>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            model: "gpt-4o-mini".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            api_base: None,
            auth_mode: AuthMode::ApiKey,
            chatgpt_config: None,
        }
    }
}

impl LlmConfig {
    /// Create a new config with the specified model
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Create an LlmConfig from AuthStatus, automatically detecting ChatGPT OAuth
    ///
    /// This is the recommended way to create an LlmConfig when using the auth system.
    /// It will automatically configure the ChatGPT backend for OAuth users.
    pub fn from_auth_status(
        model: impl Into<String>,
        auth_status: &crate::auth::AuthStatus,
        account_id: Option<String>,
        access_token: Option<String>,
    ) -> Self {
        use crate::auth::AuthStatus;

        match auth_status {
            AuthStatus::ChatGpt { .. } => {
                // ChatGPT OAuth mode - requires access_token and account_id
                if let (Some(token), Some(acct)) = (access_token, account_id) {
                    Self::chatgpt(model, token, acct)
                } else {
                    // Fallback to API key mode if credentials not provided
                    tracing::warn!(
                        "ChatGPT auth status detected but missing credentials, falling back to API key mode"
                    );
                    Self::with_model(model)
                }
            }
            AuthStatus::ApiKey | AuthStatus::EnvApiKey | AuthStatus::NotAuthenticated => {
                // Standard API key mode
                Self::with_model(model)
            }
        }
    }

    /// Create a config for Anthropic Claude models
    pub fn anthropic(model: impl Into<String>) -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            model: model.into(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            api_base: None,
            auth_mode: AuthMode::ApiKey,
            chatgpt_config: None,
        }
    }

    /// Create a config for ChatGPT OAuth authentication
    ///
    /// This uses the ChatGPT backend at `chatgpt.com/backend-api/codex` instead
    /// of the standard OpenAI API. Required for users authenticated via ChatGPT OAuth.
    pub fn chatgpt(model: impl Into<String>, access_token: String, account_id: String) -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            model: model.into(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            api_base: None,
            auth_mode: AuthMode::ChatGpt,
            chatgpt_config: Some(ChatGptConfig {
                access_token,
                account_id,
                base_url: None,
            }),
        }
    }

    /// Set the ChatGPT configuration
    pub fn with_chatgpt_config(mut self, config: ChatGptConfig) -> Self {
        self.auth_mode = AuthMode::ChatGpt;
        self.chatgpt_config = Some(config);
        self
    }

    /// Check if using ChatGPT backend
    pub fn is_chatgpt_mode(&self) -> bool {
        matches!(self.auth_mode, AuthMode::ChatGpt)
    }
}

/// Response from the LLM
#[derive(Clone, Debug)]
pub struct LlmResponse {
    /// Text content (if any)
    pub content: Option<String>,
    /// Tool calls requested by the model
    pub tool_calls: Vec<ToolCall>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Token usage
    pub usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// Number of tokens served from cache (OpenAI prompt caching)
    /// When > 0, indicates a cache hit occurred
    pub cached_tokens: u32,
}

/// Retry configuration for LLM calls (Audit #38)
#[derive(Clone, Debug)]
pub struct LlmRetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_retries: u32,
    /// Initial delay in milliseconds before first retry (default: 500)
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds between retries (default: 5000)
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff (default: 2.0)
    pub backoff_multiplier: f64,
}

impl Default for LlmRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 500,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

impl LlmRetryConfig {
    /// Check if an error is retryable
    fn is_retryable_error(error_msg: &str) -> bool {
        // Rate limiting errors (HTTP 429)
        if error_msg.contains("429") || error_msg.contains("rate limit") {
            return true;
        }
        // Server errors (HTTP 5xx)
        if error_msg.contains("500")
            || error_msg.contains("502")
            || error_msg.contains("503")
            || error_msg.contains("504")
        {
            return true;
        }
        // Network/connection errors
        if error_msg.contains("timeout")
            || error_msg.contains("connection")
            || error_msg.contains("network")
        {
            return true;
        }
        // OpenAI specific overload messages
        if error_msg.contains("overloaded") || error_msg.contains("capacity") {
            return true;
        }
        false
    }

    /// Calculate delay for a given retry attempt (0-indexed)
    fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let delay_ms = (self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32))
            .min(self.max_delay_ms as f64) as u64;
        std::time::Duration::from_millis(delay_ms)
    }
}

/// Unified LLM client using DashFlow providers
pub struct LlmClient {
    config: LlmConfig,
    tools: Vec<AgentToolDefinition>,
    /// Retry configuration for transient failures (Audit #38)
    retry_config: LlmRetryConfig,
    /// Stream callback for emitting token chunks during streaming (Audit #73)
    stream_callback: Option<Arc<dyn StreamCallback>>,
    /// Session ID for streaming events
    session_id: Option<String>,
}

impl LlmClient {
    /// Create a new LLM client with default configuration
    ///
    /// Uses `OPENAI_API_KEY` environment variable for authentication
    pub fn new() -> Self {
        Self::with_config(LlmConfig::default())
    }

    /// Create a new LLM client with custom configuration
    pub fn with_config(config: LlmConfig) -> Self {
        Self {
            config,
            tools: Vec::new(),
            retry_config: LlmRetryConfig::default(),
            stream_callback: None,
            session_id: None,
        }
    }

    /// Set custom retry configuration
    pub fn with_retry_config(mut self, retry_config: LlmRetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Set the model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    /// Set the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.config.temperature = Some(temperature);
        self
    }

    /// Set the max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.config.max_tokens = Some(max_tokens);
        self
    }

    /// Add a tool definition
    pub fn with_tool(mut self, tool: AgentToolDefinition) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tool definitions
    pub fn with_tools(mut self, tools: Vec<AgentToolDefinition>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Get the available tools
    pub fn tools(&self) -> &[AgentToolDefinition] {
        &self.tools
    }

    /// Set stream callback for emitting token chunks during streaming (Audit #73)
    ///
    /// When set, the client will emit `TokenChunk` events for each text delta
    /// received during streaming responses (ChatGPT backend).
    pub fn with_stream_callback(mut self, callback: Arc<dyn StreamCallback>) -> Self {
        self.stream_callback = Some(callback);
        self
    }

    /// Set session ID for streaming events
    ///
    /// The session ID is included in `TokenChunk` events emitted during streaming.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Generate a response from the LLM using DashFlow providers
    ///
    /// Includes automatic retry with exponential backoff for transient failures (Audit #38).
    ///
    /// # Arguments
    /// * `messages` - Conversation history
    /// * `tools` - Optional tool definitions (uses client's tools if None)
    pub async fn generate(
        &self,
        messages: &[AgentMessage],
        tools: Option<&[AgentToolDefinition]>,
    ) -> Result<LlmResponse> {
        // Convert tools to DashFlow format
        let tools_to_use = tools.unwrap_or(&self.tools);
        let dashflow_tools: Vec<ToolDefinition> = tools_to_use
            .iter()
            .map(|t| ToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            })
            .collect();

        // Check if using ChatGPT backend (OAuth mode)
        if self.config.is_chatgpt_mode() {
            return self
                .generate_with_retry(|| async {
                    self.generate_chatgpt_backend(messages, tools_to_use).await
                })
                .await;
        }

        // Convert messages to DashFlow format for standard providers
        let dashflow_messages = self.convert_messages(messages)?;

        // Call the appropriate provider with retry
        match self.config.provider {
            LlmProvider::OpenAI => {
                self.generate_with_retry(|| async {
                    self.generate_openai(&dashflow_messages, &dashflow_tools)
                        .await
                })
                .await
            }
            LlmProvider::Anthropic => {
                self.generate_with_retry(|| async {
                    self.generate_anthropic(&dashflow_messages, &dashflow_tools)
                        .await
                })
                .await
            }
        }
    }

    /// Execute an LLM call with retry logic for transient failures (Audit #38)
    async fn generate_with_retry<F, Fut>(&self, f: F) -> Result<LlmResponse>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<LlmResponse>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.retry_config.max_retries {
            match f().await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let error_msg = e.to_string();
                    let is_retryable = LlmRetryConfig::is_retryable_error(&error_msg);

                    if !is_retryable || attempt == self.retry_config.max_retries {
                        // Non-retryable error or exhausted retries
                        if attempt > 0 {
                            tracing::warn!(
                                attempt = attempt + 1,
                                max_retries = self.retry_config.max_retries + 1,
                                error = %error_msg,
                                "LLM call failed after retries"
                            );
                        }
                        return Err(e);
                    }

                    // Calculate delay and wait before retry
                    let delay = self.retry_config.delay_for_attempt(attempt);
                    tracing::info!(
                        attempt = attempt + 1,
                        max_retries = self.retry_config.max_retries + 1,
                        delay_ms = delay.as_millis(),
                        error = %error_msg,
                        "LLM call failed with retryable error, retrying"
                    );

                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        // Should not reach here, but return last error if we do
        Err(last_error.unwrap_or_else(|| crate::Error::LlmApi("Unknown error".to_string())))
    }

    /// Generate using OpenAI
    async fn generate_openai(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        // Create OpenAI config with optional custom api_base (Audit #36, #41)
        let mut model = if let Some(api_base) = &self.config.api_base {
            let openai_config = OpenAIConfig::default().with_api_base(api_base);
            tracing::debug!(api_base = %api_base, "Using custom OpenAI API base");
            ChatOpenAI::with_config(openai_config).with_model(&self.config.model)
        } else {
            ChatOpenAI::with_config(OpenAIConfig::default()).with_model(&self.config.model)
        };

        if let Some(temp) = self.config.temperature {
            model = model.with_temperature(temp);
        }
        if let Some(max_tokens) = self.config.max_tokens {
            model = model.with_max_tokens(max_tokens);
        }

        tracing::debug!(model = %self.config.model, api_base = ?self.config.api_base, "Sending OpenAI request");

        let tool_choice = if tools.is_empty() {
            None
        } else {
            Some(ToolChoice::Auto)
        };

        let result = model
            .generate(messages, None, Some(tools), tool_choice.as_ref(), None)
            .await
            .map_err(|e| crate::Error::LlmApi(format!("OpenAI API error: {}", e)))?;

        self.convert_result(&result)
    }

    /// Generate using Anthropic
    async fn generate_anthropic(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let mut model = ChatAnthropic::try_new()
            .map_err(|e| crate::Error::LlmApi(format!("Failed to create Anthropic client: {}", e)))?
            .with_model(&self.config.model);

        if let Some(temp) = self.config.temperature {
            model = model.with_temperature(temp);
        }
        if let Some(max_tokens) = self.config.max_tokens {
            model = model.with_max_tokens(max_tokens);
        }

        tracing::debug!(model = %self.config.model, "Sending Anthropic request");

        let tool_choice = if tools.is_empty() {
            None
        } else {
            Some(ToolChoice::Auto)
        };

        let result = model
            .generate(messages, None, Some(tools), tool_choice.as_ref(), None)
            .await
            .map_err(|e| crate::Error::LlmApi(format!("Anthropic API error: {}", e)))?;

        self.convert_result(&result)
    }

    /// Generate using ChatGPT backend (for OAuth users)
    ///
    /// This uses a direct HTTP client instead of DashFlow's ChatOpenAI because
    /// the ChatGPT backend requires custom headers (ChatGPT-Account-Id) and a
    /// different base URL (chatgpt.com/backend-api/codex).
    ///
    /// The ChatGPT backend uses the Responses API format (/responses endpoint)
    /// which differs from the Chat Completions API (/chat/completions).
    /// Streaming is required - the backend does not support non-streaming requests.
    async fn generate_chatgpt_backend(
        &self,
        messages: &[AgentMessage],
        tools: &[AgentToolDefinition],
    ) -> Result<LlmResponse> {
        let chatgpt_config = self.config.chatgpt_config.as_ref().ok_or_else(|| {
            crate::Error::LlmApi("ChatGPT config required for ChatGPT auth mode".to_string())
        })?;

        let base_url = chatgpt_config.base_url();
        // ChatGPT backend uses the Responses API at /responses
        let url = format!("{}/responses", base_url);

        // Build User-Agent matching codex CLI format
        let user_agent = format!(
            "codex_dashflow/{} ({} {}; {})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        );

        tracing::debug!(
            model = %self.config.model,
            base_url = %base_url,
            url = %url,
            "Sending ChatGPT backend request"
        );

        // Build the request body in Responses API format
        let request_body = self.build_chatgpt_request_body(messages, tools)?;

        tracing::trace!(request_body = %serde_json::to_string_pretty(&request_body).unwrap_or_default(), "ChatGPT request body");

        // Create HTTP client with proper headers (matching codex CLI)
        let client = reqwest::Client::builder()
            .user_agent(&user_agent)
            .build()
            .map_err(|e| crate::Error::LlmApi(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .post(&url)
            .bearer_auth(&chatgpt_config.access_token)
            .header("ChatGPT-Account-Id", &chatgpt_config.account_id)
            .header("originator", "codex_dashflow")
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| crate::Error::LlmApi(format!("ChatGPT backend request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::Error::LlmApi(format!(
                "ChatGPT backend error ({}): {}",
                status, body
            )));
        }

        // Parse SSE (Server-Sent Events) response
        self.parse_chatgpt_sse_response(response).await
    }

    /// Parse SSE stream from ChatGPT backend using true streaming (Audit #37)
    ///
    /// The ChatGPT backend returns Server-Sent Events (SSE) with events like:
    /// - response.created
    /// - response.output_item.added
    /// - response.output_text.delta
    /// - response.completed
    ///
    /// This implementation uses `bytes_stream()` to process events incrementally
    /// as they arrive, reducing latency and memory usage compared to buffering
    /// the entire response body.
    async fn parse_chatgpt_sse_response(&self, response: reqwest::Response) -> Result<LlmResponse> {
        use futures::StreamExt;

        let mut content_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut finish_reason: Option<String> = None;
        let mut usage: Option<TokenUsage> = None;

        // Buffer for accumulating partial SSE data across chunks
        let mut buffer = String::new();

        // Stream the response bytes incrementally
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| crate::Error::LlmApi(format!("SSE stream error: {}", e)))?;

            // Append chunk to buffer
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete SSE events (terminated by double newline)
            while let Some(event_end) = buffer.find("\n\n") {
                let event_block = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                // Parse the event
                if let Some((event_type, event_data)) = self.parse_sse_event(&event_block) {
                    self.process_chatgpt_sse_event(
                        event_type,
                        event_data,
                        &mut content_parts,
                        &mut tool_calls,
                        &mut finish_reason,
                        &mut usage,
                    );
                }
            }
        }

        // Process any remaining data in buffer (shouldn't happen with well-formed SSE)
        if !buffer.trim().is_empty() {
            if let Some((event_type, event_data)) = self.parse_sse_event(&buffer) {
                self.process_chatgpt_sse_event(
                    event_type,
                    event_data,
                    &mut content_parts,
                    &mut tool_calls,
                    &mut finish_reason,
                    &mut usage,
                );
            }
        }

        // Combine content parts
        let content = if content_parts.is_empty() {
            None
        } else {
            Some(content_parts.join(""))
        };

        tracing::debug!(
            has_content = content.is_some(),
            tool_calls = tool_calls.len(),
            finish_reason = ?finish_reason,
            "ChatGPT SSE response parsed"
        );

        Ok(LlmResponse {
            content,
            tool_calls,
            finish_reason,
            usage,
        })
    }

    /// Parse a single SSE event block into event type and data
    fn parse_sse_event(&self, event_block: &str) -> Option<(String, serde_json::Value)> {
        let event_block = event_block.trim();
        if event_block.is_empty() {
            return None;
        }

        let mut event_type = String::new();
        let mut event_data = String::new();

        for line in event_block.lines() {
            if let Some(t) = line.strip_prefix("event: ") {
                event_type = t.trim().to_string();
            } else if let Some(d) = line.strip_prefix("data: ") {
                event_data = d.trim().to_string();
            }
        }

        if event_data.is_empty() || event_data == "[DONE]" {
            return None;
        }

        match serde_json::from_str(&event_data) {
            Ok(data) => Some((event_type, data)),
            Err(e) => {
                tracing::warn!(event_type = %event_type, error = %e, "Failed to parse SSE event data");
                None
            }
        }
    }

    /// Process a single ChatGPT SSE event and update result accumulators
    fn process_chatgpt_sse_event(
        &self,
        event_type: String,
        data: serde_json::Value,
        content_parts: &mut Vec<String>,
        tool_calls: &mut Vec<ToolCall>,
        finish_reason: &mut Option<String>,
        usage: &mut Option<TokenUsage>,
    ) {
        match event_type.as_str() {
            "response.output_text.delta" => {
                // Extract text delta - this is where true streaming helps reduce latency
                if let Some(delta) = data.get("delta").and_then(|d| d.as_str()) {
                    tracing::trace!(delta_len = delta.len(), "Received text delta");
                    content_parts.push(delta.to_string());

                    // Emit TokenChunk event for streaming consumers (Audit #73)
                    // Use tokio::spawn to send event asynchronously (same pattern as state.emit_event)
                    if let (Some(ref callback), Some(ref session_id)) =
                        (&self.stream_callback, &self.session_id)
                    {
                        let callback = callback.clone();
                        let event = AgentEvent::TokenChunk {
                            session_id: session_id.clone(),
                            chunk: delta.to_string(),
                            is_final: false,
                        };
                        tokio::spawn(async move {
                            callback.on_event(event).await;
                        });
                    }
                }
            }
            "response.output_item.done" => {
                // Extract completed output item
                if let Some(item) = data.get("item") {
                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    if item_type == "message" {
                        // Extract text from message content
                        if let Some(content_arr) = item.get("content").and_then(|c| c.as_array()) {
                            for c in content_arr {
                                if let Some(text) = c.get("text").and_then(|t| t.as_str()) {
                                    // Only add if we don't have this text from deltas
                                    if content_parts.is_empty() {
                                        content_parts.push(text.to_string());
                                    }
                                }
                            }
                        }
                    } else if item_type == "function_call" {
                        // Extract function call
                        let call_id = item
                            .get("call_id")
                            .and_then(|c| c.as_str())
                            .unwrap_or_default();
                        let name = item
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or_default();
                        let arguments = item
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .unwrap_or("{}");

                        let args: serde_json::Value =
                            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

                        if !call_id.is_empty() && !name.is_empty() {
                            tool_calls.push(ToolCall {
                                id: call_id.to_string(),
                                tool: name.to_string(),
                                args,
                            });
                        }
                    }
                }
            }
            "response.completed" => {
                // Extract final status and usage
                if let Some(resp) = data.get("response") {
                    *finish_reason = resp
                        .get("status")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());

                    if let Some(u) = resp.get("usage") {
                        *usage = Some(TokenUsage {
                            prompt_tokens: u
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            completion_tokens: u
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            total_tokens: u
                                .get("total_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            // ChatGPT backend uses cache_read_input_tokens for prompt caching
                            cached_tokens: u
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                        });
                    }
                }
            }
            _ => {
                // Ignore other event types
            }
        }
    }

    /// Build request body for ChatGPT backend using Responses API format
    ///
    /// The Responses API uses a different format than Chat Completions:
    /// - `instructions` field for system prompt
    /// - `input` array for conversation items
    /// - Items use `{"type": "message", "role": "...", "content": [{"type": "input_text", "text": "..."}]}`
    fn build_chatgpt_request_body(
        &self,
        messages: &[AgentMessage],
        tools: &[AgentToolDefinition],
    ) -> Result<serde_json::Value> {
        // Collect system messages as instructions
        let mut instructions = String::new();
        let mut input_items: Vec<serde_json::Value> = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // System messages become the instructions field
                    if !instructions.is_empty() {
                        instructions.push_str("\n\n");
                    }
                    instructions.push_str(&msg.content);
                }
                MessageRole::User => {
                    // User messages become input items with content array
                    input_items.push(serde_json::json!({
                        "type": "message",
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": msg.content
                        }]
                    }));
                }
                MessageRole::Assistant => {
                    if msg.has_tool_calls() {
                        // Assistant message with tool calls - add function_call items
                        // First add text content if any
                        if !msg.content.is_empty() {
                            input_items.push(serde_json::json!({
                                "type": "message",
                                "role": "assistant",
                                "content": [{
                                    "type": "output_text",
                                    "text": msg.content
                                }]
                            }));
                        }
                        // Then add each tool call as a function_call item
                        for tc in &msg.tool_calls {
                            input_items.push(serde_json::json!({
                                "type": "function_call",
                                "name": tc.tool,
                                "arguments": tc.args.to_string(),
                                "call_id": tc.id
                            }));
                        }
                    } else {
                        // Regular assistant message
                        input_items.push(serde_json::json!({
                            "type": "message",
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": msg.content
                            }]
                        }));
                    }
                }
                MessageRole::Tool => {
                    // Tool result becomes function_call_output
                    // The Responses API expects different shapes for success vs failure:
                    // - success → output is a plain string
                    // - failure → output is an object { content, success: false }
                    if let Some(ref call_id) = msg.tool_call_id {
                        // For now, assume all tool results are successful (plain string output)
                        input_items.push(serde_json::json!({
                            "type": "function_call_output",
                            "call_id": call_id,
                            "output": msg.content
                        }));
                    }
                }
            }
        }

        // ChatGPT backend requires instructions to start with a specific prefix
        // for certain models. For gpt-5-codex, it REQUIRES the full Codex prompt.
        // The backend validates the instructions content, not just the prefix.
        if self.config.model.contains("gpt-5") || self.config.model.contains("codex") {
            // Use the exact Codex prompt that the backend expects
            instructions = include_str!("gpt_5_codex_prompt.md").to_string();
        } else if instructions.is_empty() {
            instructions = "You are a helpful coding assistant.".to_string();
        }

        // Build the Responses API request body
        // Match the format used by original codex
        // ChatGPT backend requires stream: true
        // Reasoning and text parameters are required by the ChatGPT backend
        let mut body = serde_json::json!({
            "model": self.config.model,
            "instructions": instructions,
            "input": input_items,
            "stream": true,  // Required for ChatGPT backend
            "store": false,
            "parallel_tool_calls": false,
            "include": [],
            "reasoning": {
                "effort": "medium",
                "summary": "auto"
            },
            "text": {
                "format": {
                    "type": "text"
                }
            }
        });

        // Add tools if any
        if !tools.is_empty() {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tool_defs);
            body["tool_choice"] = serde_json::Value::String("auto".to_string());
        }

        Ok(body)
    }

    /// Parse ChatGPT backend response (Responses API format)
    ///
    /// The Responses API returns:
    /// - `output` array with items like `{"type": "message", "role": "assistant", "content": [...]}`
    /// - `status` field ("completed", "failed", etc.)
    /// - `usage` object with token counts
    ///
    /// Note: This method is used by tests. Production code uses parse_chatgpt_sse_response.
    #[cfg_attr(not(test), allow(dead_code))]
    fn parse_chatgpt_response(&self, response: &serde_json::Value) -> Result<LlmResponse> {
        // Responses API uses "output" array instead of "choices"
        let output = response
            .get("output")
            .and_then(|o| o.as_array())
            .ok_or_else(|| {
                crate::Error::LlmApi(format!(
                    "Missing 'output' in response: {}",
                    serde_json::to_string(response).unwrap_or_default()
                ))
            })?;

        let mut content_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        // Process each output item
        for item in output {
            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match item_type {
                "message" => {
                    // Message items have content array
                    if let Some(content_arr) = item.get("content").and_then(|c| c.as_array()) {
                        for content_item in content_arr {
                            let content_type = content_item
                                .get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if content_type == "output_text" || content_type == "text" {
                                if let Some(text) =
                                    content_item.get("text").and_then(|t| t.as_str())
                                {
                                    content_parts.push(text.to_string());
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    // Extract function call details
                    let call_id = item
                        .get("call_id")
                        .and_then(|c| c.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");

                    // Parse arguments as JSON
                    let args: serde_json::Value =
                        serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

                    if !call_id.is_empty() && !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id: call_id,
                            tool: name,
                            args,
                        });
                    }
                }
                _ => {
                    // Skip other item types (reasoning, etc.)
                }
            }
        }

        // Combine content parts
        let content = if content_parts.is_empty() {
            None
        } else {
            Some(content_parts.join("\n"))
        };

        // Extract status as finish reason
        let finish_reason = response
            .get("status")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        // Extract usage
        let usage = response.get("usage").map(|u| TokenUsage {
            prompt_tokens: u
                .get("input_tokens")
                .or_else(|| u.get("prompt_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("output_tokens")
                .or_else(|| u.get("completion_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            // OpenAI uses cache_read_input_tokens for prompt caching hits
            cached_tokens: u
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        });

        tracing::debug!(
            has_content = content.is_some(),
            tool_calls = tool_calls.len(),
            finish_reason = ?finish_reason,
            "ChatGPT backend response received"
        );

        Ok(LlmResponse {
            content,
            tool_calls,
            finish_reason,
            usage,
        })
    }

    /// Convert our messages to DashFlow format
    fn convert_messages(&self, messages: &[AgentMessage]) -> Result<Vec<Message>> {
        let mut result = Vec::new();

        for msg in messages {
            let dashflow_msg = match msg.role {
                MessageRole::System => Message::system(msg.content.clone()),
                MessageRole::User => Message::human(msg.content.clone()),
                MessageRole::Assistant => {
                    // Handle assistant messages with or without tool calls
                    if msg.has_tool_calls() {
                        // Create AI message with tool calls
                        let tool_calls: Vec<DashflowToolCall> = msg
                            .tool_calls
                            .iter()
                            .map(|tc| DashflowToolCall {
                                id: tc.id.clone(),
                                name: tc.tool.clone(),
                                args: tc.args.clone(),
                                tool_type: "tool_call".to_string(),
                                index: None,
                            })
                            .collect();

                        Message::AI {
                            content: MessageContent::Text(msg.content.clone()),
                            tool_calls,
                            invalid_tool_calls: Vec::new(),
                            usage_metadata: None,
                            fields: BaseMessageFields::default(),
                        }
                    } else {
                        Message::ai(msg.content.clone())
                    }
                }
                MessageRole::Tool => {
                    let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
                        crate::Error::LlmApi("Tool message missing tool_call_id".to_string())
                    })?;
                    Message::tool(msg.content.clone(), tool_call_id.clone())
                }
            };
            result.push(dashflow_msg);
        }

        Ok(result)
    }

    /// Convert DashFlow result to our format
    fn convert_result(
        &self,
        result: &dashflow::core::language_models::ChatResult,
    ) -> Result<LlmResponse> {
        let generation = result
            .generations
            .first()
            .ok_or_else(|| crate::Error::LlmApi("No response generations".to_string()))?;

        let ai_message = &generation.message;
        let content_text = ai_message.content().as_text();
        let content = if content_text.is_empty() {
            None
        } else {
            Some(content_text)
        };

        // Convert tool calls
        let tool_calls: Vec<ToolCall> = ai_message
            .tool_calls()
            .iter()
            .map(|tc| ToolCall {
                id: tc.id.clone(),
                tool: tc.name.clone(),
                args: tc.args.clone(),
            })
            .collect();

        // Extract usage if available
        let usage = generation.generation_info.as_ref().and_then(|info| {
            let prompt_tokens = info.get("prompt_tokens")?.as_u64()? as u32;
            let completion_tokens = info.get("completion_tokens")?.as_u64()? as u32;
            let cached_tokens = info
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Some(TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                cached_tokens,
            })
        });

        tracing::debug!(
            has_content = content.is_some(),
            tool_calls = tool_calls.len(),
            "LLM response received"
        );

        Ok(LlmResponse {
            content,
            tool_calls,
            finish_reason: generation
                .generation_info
                .as_ref()
                .and_then(|i| i.get("finish_reason"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            usage,
        })
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LlmClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            tools: self.tools.clone(),
            retry_config: self.retry_config.clone(),
            stream_callback: self.stream_callback.clone(),
            session_id: self.session_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_llm_config_anthropic() {
        let config = LlmConfig::anthropic("claude-3-5-sonnet-latest");
        assert_eq!(config.model, "claude-3-5-sonnet-latest");
        assert_eq!(config.provider, LlmProvider::Anthropic);
    }

    #[test]
    fn test_llm_client_creation() {
        let client = LlmClient::new();
        assert!(client.tools().is_empty());

        let tools = crate::tools::get_tool_definitions();
        let client_with_tools = LlmClient::new().with_tools(tools);
        assert_eq!(client_with_tools.tools().len(), 6);
    }

    #[test]
    fn test_tool_definition_creation() {
        let tools = crate::tools::get_tool_definitions();
        assert_eq!(tools.len(), 6);
        assert_eq!(tools[0].name, "shell");
        assert_eq!(tools[1].name, "read_file");
        assert_eq!(tools[2].name, "write_file");
        assert_eq!(tools[3].name, "apply_patch");
        assert_eq!(tools[4].name, "search_files");
        assert_eq!(tools[5].name, "list_dir");
    }

    #[test]
    fn test_message_conversion() {
        let client = LlmClient::new();
        let messages = vec![
            AgentMessage::system("You are a helpful assistant"),
            AgentMessage::user("Hello"),
            AgentMessage::assistant("Hi there"),
        ];

        let converted = client.convert_messages(&messages);
        assert!(converted.is_ok());
        let converted = converted.unwrap();
        assert_eq!(converted.len(), 3);
    }

    #[test]
    fn test_message_conversion_with_tool_calls() {
        let client = LlmClient::new();
        let tool_call = ToolCall::new("shell", serde_json::json!({"command": "ls -la"}));

        let messages = vec![
            AgentMessage::system("You are a helpful assistant"),
            AgentMessage::user("List files"),
            AgentMessage::assistant_with_tool_calls(None, vec![tool_call.clone()]),
            AgentMessage::tool("file1.txt\nfile2.txt", &tool_call.id),
        ];

        let converted = client.convert_messages(&messages);
        assert!(converted.is_ok());
        let converted = converted.unwrap();
        assert_eq!(converted.len(), 4);
    }

    #[test]
    fn test_provider_serialization() {
        let openai = LlmProvider::OpenAI;
        let anthropic = LlmProvider::Anthropic;

        let openai_json = serde_json::to_string(&openai).unwrap();
        let anthropic_json = serde_json::to_string(&anthropic).unwrap();

        assert_eq!(openai_json, "\"openai\"");
        assert_eq!(anthropic_json, "\"anthropic\"");
    }

    #[test]
    fn test_auth_mode_default() {
        let config = LlmConfig::default();
        assert_eq!(config.auth_mode, AuthMode::ApiKey);
        assert!(config.chatgpt_config.is_none());
        assert!(!config.is_chatgpt_mode());
    }

    #[test]
    fn test_auth_mode_serialization() {
        let api_key = AuthMode::ApiKey;
        let chatgpt = AuthMode::ChatGpt;

        let api_key_json = serde_json::to_string(&api_key).unwrap();
        let chatgpt_json = serde_json::to_string(&chatgpt).unwrap();

        assert_eq!(api_key_json, "\"api_key\"");
        assert_eq!(chatgpt_json, "\"chat_gpt\"");
    }

    #[test]
    fn test_llm_config_chatgpt() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-access-token".to_string(),
            "account-123".to_string(),
        );
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.auth_mode, AuthMode::ChatGpt);
        assert!(config.is_chatgpt_mode());

        let chatgpt_config = config.chatgpt_config.unwrap();
        assert_eq!(chatgpt_config.access_token, "test-access-token");
        assert_eq!(chatgpt_config.account_id, "account-123");
        assert_eq!(
            chatgpt_config.base_url(),
            "https://chatgpt.com/backend-api/codex"
        );
    }

    #[test]
    fn test_chatgpt_config_custom_base_url() {
        let config = ChatGptConfig {
            access_token: "token".to_string(),
            account_id: "account".to_string(),
            base_url: Some("https://custom.chatgpt.com/api".to_string()),
        };
        assert_eq!(config.base_url(), "https://custom.chatgpt.com/api");
    }

    #[test]
    fn test_llm_config_with_chatgpt_config() {
        let chatgpt_config = ChatGptConfig {
            access_token: "token".to_string(),
            account_id: "account".to_string(),
            base_url: None,
        };
        let config = LlmConfig::with_model("gpt-4").with_chatgpt_config(chatgpt_config);
        assert!(config.is_chatgpt_mode());
        assert_eq!(config.auth_mode, AuthMode::ChatGpt);
    }

    #[test]
    fn test_build_chatgpt_request_body() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        let messages = vec![
            AgentMessage::system("You are a helpful assistant"),
            AgentMessage::user("Hello"),
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // Responses API format uses "input" array instead of "messages"
        assert_eq!(body["model"], "gpt-4o");
        assert!(body["input"].is_array());
        // System message becomes instructions, so only user message in input
        assert_eq!(body["input"].as_array().unwrap().len(), 1);
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["type"], "message");
        // System message is in instructions field
        assert_eq!(body["instructions"], "You are a helpful assistant");
    }

    #[test]
    fn test_build_chatgpt_request_body_with_tools() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        let messages = vec![AgentMessage::user("List files")];
        let tools = crate::tools::get_tool_definitions();

        let body = client
            .build_chatgpt_request_body(&messages, &tools)
            .unwrap();

        assert!(body["tools"].is_array());
        assert_eq!(body["tools"].as_array().unwrap().len(), 6);
        assert_eq!(body["tool_choice"], "auto");
    }

    #[test]
    fn test_build_chatgpt_request_body_with_tool_calls() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        let tool_call = ToolCall::new("shell", serde_json::json!({"command": "ls"}));
        let messages = vec![
            AgentMessage::user("List files"),
            AgentMessage::assistant_with_tool_calls(None, vec![tool_call.clone()]),
            AgentMessage::tool("file1.txt", &tool_call.id),
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // Responses API format: user message + function_call + function_call_output
        let input_arr = body["input"].as_array().unwrap();
        assert_eq!(input_arr.len(), 3);

        // First item is user message
        assert_eq!(input_arr[0]["type"], "message");
        assert_eq!(input_arr[0]["role"], "user");

        // Second item is function_call (from assistant with tool_calls)
        assert_eq!(input_arr[1]["type"], "function_call");
        assert_eq!(input_arr[1]["name"], "shell");
        assert!(input_arr[1].get("call_id").is_some());

        // Third item is function_call_output (from tool message)
        assert_eq!(input_arr[2]["type"], "function_call_output");
        assert!(input_arr[2].get("call_id").is_some());
    }

    #[test]
    fn test_parse_chatgpt_response_simple() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        // Responses API format uses "output" array instead of "choices"
        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello! How can I help you?"
                }]
            }],
            "status": "completed",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert_eq!(
            result.content,
            Some("Hello! How can I help you?".to_string())
        );
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.finish_reason, Some("completed".to_string()));
        assert!(result.usage.is_some());
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_parse_chatgpt_response_with_tool_calls() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        // Responses API format uses "function_call" items in "output" array
        let response = serde_json::json!({
            "output": [{
                "type": "function_call",
                "call_id": "call_123",
                "name": "shell",
                "arguments": "{\"command\": \"ls -la\"}"
            }],
            "status": "requires_action"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert!(result.content.is_none());
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "call_123");
        assert_eq!(result.tool_calls[0].tool, "shell");
        assert_eq!(result.tool_calls[0].args["command"], "ls -la");
        assert_eq!(result.finish_reason, Some("requires_action".to_string()));
    }

    #[test]
    fn test_parse_chatgpt_response_error_missing_output() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        // Missing "output" array should return error
        let response = serde_json::json!({});
        let result = client.parse_chatgpt_response(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_config_from_auth_status_api_key() {
        use crate::auth::AuthStatus;

        let status = AuthStatus::ApiKey;
        let config = LlmConfig::from_auth_status("gpt-4o", &status, None, None);

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.auth_mode, AuthMode::ApiKey);
        assert!(!config.is_chatgpt_mode());
    }

    #[test]
    fn test_llm_config_from_auth_status_env_api_key() {
        use crate::auth::AuthStatus;

        let status = AuthStatus::EnvApiKey;
        let config = LlmConfig::from_auth_status("gpt-4o", &status, None, None);

        assert_eq!(config.auth_mode, AuthMode::ApiKey);
        assert!(!config.is_chatgpt_mode());
    }

    #[test]
    fn test_llm_config_from_auth_status_chatgpt() {
        use crate::auth::AuthStatus;

        let status = AuthStatus::ChatGpt {
            email: Some("user@example.com".to_string()),
        };
        let config = LlmConfig::from_auth_status(
            "gpt-4o",
            &status,
            Some("account-123".to_string()),
            Some("access-token".to_string()),
        );

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.auth_mode, AuthMode::ChatGpt);
        assert!(config.is_chatgpt_mode());

        let chatgpt_config = config.chatgpt_config.unwrap();
        assert_eq!(chatgpt_config.access_token, "access-token");
        assert_eq!(chatgpt_config.account_id, "account-123");
    }

    #[test]
    fn test_llm_config_from_auth_status_chatgpt_missing_credentials() {
        use crate::auth::AuthStatus;

        let status = AuthStatus::ChatGpt { email: None };
        // Missing credentials should fallback to API key mode
        let config = LlmConfig::from_auth_status("gpt-4o", &status, None, None);

        assert_eq!(config.auth_mode, AuthMode::ApiKey);
        assert!(!config.is_chatgpt_mode());
    }

    #[test]
    fn test_parse_chatgpt_response_with_cache_hit() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        // Response with cache_read_input_tokens indicating a prompt cache hit
        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello!"
                }]
            }],
            "status": "completed",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 10,
                "total_tokens": 110,
                "cache_read_input_tokens": 80
            }
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 10);
        assert_eq!(usage.cached_tokens, 80);
    }

    #[test]
    fn test_parse_chatgpt_response_no_cache_hit() {
        let config = LlmConfig::chatgpt(
            "gpt-4o",
            "test-token".to_string(),
            "test-account".to_string(),
        );
        let client = LlmClient::with_config(config);

        // Response without cache_read_input_tokens (no cache hit)
        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello!"
                }]
            }],
            "status": "completed",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 10,
                "total_tokens": 110
            }
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.cached_tokens, 0);
    }

    #[test]
    fn test_token_usage_default_has_zero_cached() {
        let usage = TokenUsage::default();
        assert_eq!(usage.cached_tokens, 0);
    }

    // Additional tests for expanded coverage

    #[test]
    fn test_llm_provider_debug() {
        let openai = LlmProvider::OpenAI;
        let anthropic = LlmProvider::Anthropic;
        assert!(format!("{:?}", openai).contains("OpenAI"));
        assert!(format!("{:?}", anthropic).contains("Anthropic"));
    }

    #[test]
    fn test_llm_provider_clone() {
        let provider = LlmProvider::OpenAI;
        let cloned = provider.clone();
        assert_eq!(cloned, LlmProvider::OpenAI);
    }

    #[test]
    fn test_llm_provider_default() {
        let provider = LlmProvider::default();
        assert_eq!(provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_llm_provider_partial_eq() {
        assert_eq!(LlmProvider::OpenAI, LlmProvider::OpenAI);
        assert_ne!(LlmProvider::OpenAI, LlmProvider::Anthropic);
    }

    #[test]
    fn test_auth_mode_debug() {
        let api_key = AuthMode::ApiKey;
        let chatgpt = AuthMode::ChatGpt;
        assert!(format!("{:?}", api_key).contains("ApiKey"));
        assert!(format!("{:?}", chatgpt).contains("ChatGpt"));
    }

    #[test]
    fn test_auth_mode_clone() {
        let mode = AuthMode::ChatGpt;
        let cloned = mode.clone();
        assert_eq!(cloned, AuthMode::ChatGpt);
    }

    #[test]
    fn test_auth_mode_default_trait() {
        let mode = AuthMode::default();
        assert_eq!(mode, AuthMode::ApiKey);
    }

    #[test]
    fn test_auth_mode_partial_eq() {
        assert_eq!(AuthMode::ApiKey, AuthMode::ApiKey);
        assert_ne!(AuthMode::ApiKey, AuthMode::ChatGpt);
    }

    #[test]
    fn test_chatgpt_config_debug() {
        let config = ChatGptConfig {
            access_token: "token".to_string(),
            account_id: "account".to_string(),
            base_url: None,
        };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("ChatGptConfig"));
        assert!(debug_str.contains("token"));
    }

    #[test]
    fn test_chatgpt_config_clone() {
        let config = ChatGptConfig {
            access_token: "token".to_string(),
            account_id: "account".to_string(),
            base_url: Some("https://custom.url".to_string()),
        };
        let cloned = config.clone();
        assert_eq!(cloned.access_token, "token");
        assert_eq!(cloned.account_id, "account");
        assert_eq!(cloned.base_url, Some("https://custom.url".to_string()));
    }

    #[test]
    fn test_chatgpt_config_default() {
        let config = ChatGptConfig::default();
        assert!(config.access_token.is_empty());
        assert!(config.account_id.is_empty());
        assert!(config.base_url.is_none());
        assert_eq!(config.base_url(), "https://chatgpt.com/backend-api/codex");
    }

    #[test]
    fn test_chatgpt_config_serialization() {
        let config = ChatGptConfig {
            access_token: "test_token".to_string(),
            account_id: "test_account".to_string(),
            base_url: Some("https://custom.url".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("test_account"));
        assert!(json.contains("custom.url"));

        let parsed: ChatGptConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "test_token");
        assert_eq!(parsed.account_id, "test_account");
        assert_eq!(parsed.base_url, Some("https://custom.url".to_string()));
    }

    #[test]
    fn test_llm_config_debug() {
        let config = LlmConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("LlmConfig"));
        assert!(debug_str.contains("gpt-4o-mini"));
    }

    #[test]
    fn test_llm_config_clone() {
        let config = LlmConfig::chatgpt("gpt-4", "token".to_string(), "account".to_string());
        let cloned = config.clone();
        assert_eq!(cloned.model, "gpt-4");
        assert!(cloned.is_chatgpt_mode());
    }

    #[test]
    fn test_llm_config_serialization() {
        let config = LlmConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("gpt-4o-mini"));
        assert!(json.contains("openai"));

        let parsed: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, "gpt-4o-mini");
        assert_eq!(parsed.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_llm_config_with_model() {
        let config = LlmConfig::with_model("gpt-4-turbo");
        assert_eq!(config.model, "gpt-4-turbo");
        assert_eq!(config.provider, LlmProvider::OpenAI);
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(4096));
    }

    #[test]
    fn test_llm_response_debug() {
        let response = LlmResponse {
            content: Some("Hello".to_string()),
            tool_calls: vec![],
            finish_reason: Some("stop".to_string()),
            usage: None,
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("LlmResponse"));
        assert!(debug_str.contains("Hello"));
    }

    #[test]
    fn test_llm_response_clone() {
        let tool_call = ToolCall::new("shell", serde_json::json!({"command": "ls"}));
        let response = LlmResponse {
            content: Some("Output".to_string()),
            tool_calls: vec![tool_call],
            finish_reason: Some("tool_calls".to_string()),
            usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cached_tokens: 20,
            }),
        };
        let cloned = response.clone();
        assert_eq!(cloned.content, Some("Output".to_string()));
        assert_eq!(cloned.tool_calls.len(), 1);
        assert_eq!(cloned.finish_reason, Some("tool_calls".to_string()));
        let usage = cloned.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.cached_tokens, 20);
    }

    #[test]
    fn test_token_usage_debug() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_tokens: 30,
        };
        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("TokenUsage"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_token_usage_clone() {
        let usage = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 100,
            total_tokens: 300,
            cached_tokens: 50,
        };
        let cloned = usage.clone();
        assert_eq!(cloned.prompt_tokens, 200);
        assert_eq!(cloned.completion_tokens, 100);
        assert_eq!(cloned.total_tokens, 300);
        assert_eq!(cloned.cached_tokens, 50);
    }

    #[test]
    fn test_llm_client_default() {
        let client = LlmClient::default();
        assert!(client.tools().is_empty());
    }

    #[test]
    fn test_llm_client_clone() {
        let client = LlmClient::new()
            .with_model("gpt-4")
            .with_temperature(0.5)
            .with_max_tokens(2048);
        let cloned = client.clone();
        assert_eq!(cloned.config.model, "gpt-4");
        assert_eq!(cloned.config.temperature, Some(0.5));
        assert_eq!(cloned.config.max_tokens, Some(2048));
    }

    #[test]
    fn test_llm_client_with_model() {
        let client = LlmClient::new().with_model("gpt-4-turbo");
        assert_eq!(client.config.model, "gpt-4-turbo");
    }

    #[test]
    fn test_llm_client_with_temperature() {
        let client = LlmClient::new().with_temperature(0.3);
        assert_eq!(client.config.temperature, Some(0.3));
    }

    #[test]
    fn test_llm_client_with_max_tokens() {
        let client = LlmClient::new().with_max_tokens(8192);
        assert_eq!(client.config.max_tokens, Some(8192));
    }

    #[test]
    fn test_llm_client_with_tool() {
        let tool = AgentToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let client = LlmClient::new().with_tool(tool);
        assert_eq!(client.tools().len(), 1);
        assert_eq!(client.tools()[0].name, "test_tool");
    }

    #[test]
    fn test_llm_client_with_tools() {
        let tools = crate::tools::get_tool_definitions();
        let initial_len = tools.len();
        let client = LlmClient::new().with_tools(tools);
        assert_eq!(client.tools().len(), initial_len);
    }

    #[test]
    fn test_llm_client_chained_builder() {
        let tool = AgentToolDefinition {
            name: "custom".to_string(),
            description: "Custom tool".to_string(),
            parameters: serde_json::json!({}),
        };
        let client = LlmClient::new()
            .with_model("gpt-4")
            .with_temperature(0.8)
            .with_max_tokens(4000)
            .with_tool(tool);

        assert_eq!(client.config.model, "gpt-4");
        assert_eq!(client.config.temperature, Some(0.8));
        assert_eq!(client.config.max_tokens, Some(4000));
        assert_eq!(client.tools().len(), 1);
    }

    #[test]
    fn test_message_conversion_tool_message_missing_id() {
        let client = LlmClient::new();
        let messages = vec![AgentMessage {
            role: MessageRole::Tool,
            content: "result".to_string(),
            tool_call_id: None, // Missing tool_call_id
            tool_calls: vec![],
        }];

        let result = client.convert_messages(&messages);
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_provider_deserialization() {
        let openai: LlmProvider = serde_json::from_str("\"openai\"").unwrap();
        let anthropic: LlmProvider = serde_json::from_str("\"anthropic\"").unwrap();

        assert_eq!(openai, LlmProvider::OpenAI);
        assert_eq!(anthropic, LlmProvider::Anthropic);
    }

    #[test]
    fn test_auth_mode_deserialization() {
        let api_key: AuthMode = serde_json::from_str("\"api_key\"").unwrap();
        let chatgpt: AuthMode = serde_json::from_str("\"chat_gpt\"").unwrap();

        assert_eq!(api_key, AuthMode::ApiKey);
        assert_eq!(chatgpt, AuthMode::ChatGpt);
    }

    #[test]
    fn test_llm_config_api_base() {
        let config = LlmConfig {
            api_base: Some("https://custom-api.example.com".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("custom-api.example.com"));

        let parsed: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.api_base,
            Some("https://custom-api.example.com".to_string())
        );
    }

    #[test]
    fn test_llm_config_no_temperature() {
        let config = LlmConfig {
            temperature: None,
            ..Default::default()
        };

        assert!(config.temperature.is_none());
    }

    #[test]
    fn test_llm_config_no_max_tokens() {
        let config = LlmConfig {
            max_tokens: None,
            ..Default::default()
        };

        assert!(config.max_tokens.is_none());
    }

    #[test]
    fn test_llm_response_empty() {
        let response = LlmResponse {
            content: None,
            tool_calls: vec![],
            finish_reason: None,
            usage: None,
        };

        assert!(response.content.is_none());
        assert!(response.tool_calls.is_empty());
        assert!(response.finish_reason.is_none());
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_build_chatgpt_request_body_multiple_system_messages() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![
            AgentMessage::system("System prompt 1"),
            AgentMessage::system("System prompt 2"),
            AgentMessage::user("Hello"),
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // Multiple system messages should be concatenated in instructions
        let instructions = body["instructions"].as_str().unwrap();
        assert!(instructions.contains("System prompt 1"));
        assert!(instructions.contains("System prompt 2"));
    }

    #[test]
    fn test_build_chatgpt_request_body_assistant_with_content() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![
            AgentMessage::user("Hello"),
            AgentMessage::assistant("Hi there!"),
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        let input_arr = body["input"].as_array().unwrap();
        assert_eq!(input_arr.len(), 2);
        assert_eq!(input_arr[1]["role"], "assistant");
        assert_eq!(input_arr[1]["content"][0]["text"], "Hi there!");
    }

    #[test]
    fn test_build_chatgpt_request_body_assistant_with_tool_calls_and_content() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let tool_call = ToolCall::new("shell", serde_json::json!({"command": "ls"}));
        let messages = vec![
            AgentMessage::user("List files"),
            AgentMessage {
                role: MessageRole::Assistant,
                content: "Let me list the files for you.".to_string(),
                tool_call_id: None,
                tool_calls: vec![tool_call],
            },
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        let input_arr = body["input"].as_array().unwrap();
        // User message + assistant message with content + function_call
        assert_eq!(input_arr.len(), 3);
        assert_eq!(input_arr[1]["role"], "assistant");
        assert_eq!(input_arr[2]["type"], "function_call");
    }

    #[test]
    fn test_build_chatgpt_request_body_gpt5_codex_model() {
        let config = LlmConfig::chatgpt("gpt-5-codex", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![
            AgentMessage::system("Custom system prompt"),
            AgentMessage::user("Hello"),
        ];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // gpt-5-codex models should use the full Codex prompt, ignoring custom system prompt
        let instructions = body["instructions"].as_str().unwrap();
        // Should NOT contain our custom prompt (it's overridden)
        assert!(!instructions.contains("Custom system prompt"));
    }

    #[test]
    fn test_build_chatgpt_request_body_no_system_message() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![AgentMessage::user("Hello")];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // Should use default instructions when no system message
        let instructions = body["instructions"].as_str().unwrap();
        assert_eq!(instructions, "You are a helpful coding assistant.");
    }

    #[test]
    fn test_build_chatgpt_request_body_stream_required() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![AgentMessage::user("Hello")];

        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // ChatGPT backend requires stream: true
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_parse_chatgpt_response_multiple_tool_calls() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let response = serde_json::json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "read_file",
                    "arguments": "{\"path\": \"file1.txt\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_2",
                    "name": "read_file",
                    "arguments": "{\"path\": \"file2.txt\"}"
                }
            ],
            "status": "requires_action"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].id, "call_1");
        assert_eq!(result.tool_calls[1].id, "call_2");
    }

    #[test]
    fn test_parse_chatgpt_response_with_text_type() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        // Some responses use "text" type instead of "output_text"
        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": "Response with text type"
                }]
            }],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert_eq!(result.content, Some("Response with text type".to_string()));
    }

    #[test]
    fn test_parse_chatgpt_response_empty_output() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let response = serde_json::json!({
            "output": [],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert!(result.content.is_none());
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_chatgpt_response_unknown_item_type() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        // Unknown item types should be skipped
        let response = serde_json::json!({
            "output": [
                {
                    "type": "reasoning",
                    "content": "thinking..."
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "Hello"
                    }]
                }
            ],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert_eq!(result.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_chatgpt_response_malformed_function_call() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        // Function call with missing required fields should be skipped
        let response = serde_json::json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "",
                    "name": "",
                    "arguments": "{}"
                }
            ],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        // Empty call_id and name should be skipped
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_chatgpt_response_invalid_arguments_json() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        // Invalid JSON in arguments should default to empty object
        let response = serde_json::json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "shell",
                    "arguments": "not valid json"
                }
            ],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].args, serde_json::json!({}));
    }

    #[test]
    fn test_llm_config_from_auth_status_not_authenticated() {
        use crate::auth::AuthStatus;

        let status = AuthStatus::NotAuthenticated;
        let config = LlmConfig::from_auth_status("gpt-4o", &status, None, None);

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.auth_mode, AuthMode::ApiKey);
    }

    #[test]
    fn test_parse_chatgpt_response_with_alternate_usage_fields() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        // Test with prompt_tokens/completion_tokens instead of input_tokens/output_tokens
        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello"
                }]
            }],
            "status": "completed",
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 25,
                "total_tokens": 75
            }
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 50);
        assert_eq!(usage.completion_tokens, 25);
    }

    #[test]
    fn test_build_chatgpt_request_required_fields() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let messages = vec![AgentMessage::user("Hello")];
        let body = client.build_chatgpt_request_body(&messages, &[]).unwrap();

        // Check all required fields for ChatGPT backend
        assert!(body.get("model").is_some());
        assert!(body.get("instructions").is_some());
        assert!(body.get("input").is_some());
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert_eq!(body["parallel_tool_calls"], false);
        assert!(body.get("reasoning").is_some());
        assert!(body.get("text").is_some());
    }

    #[test]
    fn test_message_conversion_all_roles() {
        let client = LlmClient::new();
        let tool_call = ToolCall::new("test", serde_json::json!({}));

        let messages = vec![
            AgentMessage::system("System"),
            AgentMessage::user("User"),
            AgentMessage::assistant("Assistant"),
            AgentMessage::assistant_with_tool_calls(None, vec![tool_call.clone()]),
            AgentMessage::tool("Tool result", &tool_call.id),
        ];

        let converted = client.convert_messages(&messages).unwrap();
        assert_eq!(converted.len(), 5);
    }

    #[test]
    fn test_llm_client_with_config_chatgpt() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        assert!(client.config.is_chatgpt_mode());
        assert_eq!(client.config.model, "gpt-4o");
    }

    #[test]
    fn test_llm_client_with_config_anthropic() {
        let config = LlmConfig::anthropic("claude-3-5-sonnet-latest");
        let client = LlmClient::with_config(config);

        assert_eq!(client.config.provider, LlmProvider::Anthropic);
        assert_eq!(client.config.model, "claude-3-5-sonnet-latest");
    }

    #[test]
    fn test_parse_chatgpt_response_multiple_content_parts() {
        let config = LlmConfig::chatgpt("gpt-4o", "token".to_string(), "account".to_string());
        let client = LlmClient::with_config(config);

        let response = serde_json::json!({
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "Part 1"},
                    {"type": "output_text", "text": "Part 2"}
                ]
            }],
            "status": "completed"
        });

        let result = client.parse_chatgpt_response(&response).unwrap();
        // Multiple content parts should be joined with newlines
        assert_eq!(result.content, Some("Part 1\nPart 2".to_string()));
    }

    // --- LlmClient builder pattern tests ---

    #[test]
    fn test_llm_client_new_creates_default() {
        let client = LlmClient::new();
        assert_eq!(client.config.model, "gpt-4o-mini");
        assert_eq!(client.config.provider, LlmProvider::OpenAI);
        assert!(client.tools.is_empty());
    }

    #[test]
    fn test_llm_client_with_model_builder_chain() {
        let client = LlmClient::new().with_model("custom-model");
        assert_eq!(client.config.model, "custom-model");
    }

    #[test]
    fn test_llm_client_with_temperature_builder_chain() {
        let client = LlmClient::new().with_temperature(0.5);
        assert_eq!(client.config.temperature, Some(0.5));
    }

    #[test]
    fn test_llm_client_with_max_tokens_builder_chain() {
        let client = LlmClient::new().with_max_tokens(2048);
        assert_eq!(client.config.max_tokens, Some(2048));
    }

    #[test]
    fn test_llm_client_with_single_tool() {
        let tool = AgentToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({}),
        };
        let client = LlmClient::new().with_tool(tool);
        assert_eq!(client.tools.len(), 1);
        assert_eq!(client.tools[0].name, "test_tool");
    }

    #[test]
    fn test_llm_client_with_multiple_tools_vec() {
        let tools = vec![
            AgentToolDefinition {
                name: "tool1".to_string(),
                description: "Tool 1".to_string(),
                parameters: serde_json::json!({}),
            },
            AgentToolDefinition {
                name: "tool2".to_string(),
                description: "Tool 2".to_string(),
                parameters: serde_json::json!({}),
            },
        ];
        let client = LlmClient::new().with_tools(tools);
        assert_eq!(client.tools.len(), 2);
    }

    #[test]
    fn test_llm_client_tools_getter_returns_slice() {
        let client = LlmClient::new();
        assert!(client.tools().is_empty());
    }

    #[test]
    fn test_llm_client_full_builder_chain_all_options() {
        let tool = AgentToolDefinition {
            name: "my_tool".to_string(),
            description: "My tool".to_string(),
            parameters: serde_json::json!({}),
        };
        let client = LlmClient::new()
            .with_model("gpt-4")
            .with_temperature(0.9)
            .with_max_tokens(8192)
            .with_tool(tool);

        assert_eq!(client.config.model, "gpt-4");
        assert_eq!(client.config.temperature, Some(0.9));
        assert_eq!(client.config.max_tokens, Some(8192));
        assert_eq!(client.tools.len(), 1);
    }

    // --- TokenUsage additional tests ---

    #[test]
    fn test_token_usage_clone_all_fields() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_tokens: 25,
        };
        let cloned = usage.clone();
        assert_eq!(usage.prompt_tokens, cloned.prompt_tokens);
        assert_eq!(usage.completion_tokens, cloned.completion_tokens);
        assert_eq!(usage.total_tokens, cloned.total_tokens);
        assert_eq!(usage.cached_tokens, cloned.cached_tokens);
    }

    // --- LlmResponse tests ---

    #[test]
    fn test_llm_response_with_content_only() {
        let response = LlmResponse {
            content: Some("Hello world".to_string()),
            tool_calls: vec![],
            finish_reason: Some("stop".to_string()),
            usage: None,
        };
        assert_eq!(response.content, Some("Hello world".to_string()));
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_llm_response_with_tool_calls_only() {
        let response = LlmResponse {
            content: None,
            tool_calls: vec![ToolCall::new("test", serde_json::json!({}))],
            finish_reason: Some("tool_calls".to_string()),
            usage: None,
        };
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
    }

    #[test]
    fn test_llm_response_with_usage_info() {
        let response = LlmResponse {
            content: Some("Hi".to_string()),
            tool_calls: vec![],
            finish_reason: None,
            usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cached_tokens: 0,
            }),
        };
        assert!(response.usage.is_some());
        let usage = response.usage.unwrap();
        assert_eq!(usage.total_tokens, 15);
    }

    // --- LlmConfig additional tests ---

    #[test]
    fn test_llm_config_with_chatgpt_config_builder_method() {
        let chatgpt_config = ChatGptConfig {
            access_token: "token123".to_string(),
            account_id: "acc456".to_string(),
            base_url: Some("https://custom.chatgpt.com".to_string()),
        };
        let config = LlmConfig::default().with_chatgpt_config(chatgpt_config);
        assert!(config.chatgpt_config.is_some());
    }

    #[test]
    fn test_llm_config_is_chatgpt_mode_returns_false() {
        let config = LlmConfig::default();
        assert!(!config.is_chatgpt_mode());
    }

    #[test]
    fn test_llm_config_is_chatgpt_mode_returns_true() {
        let config = LlmConfig::chatgpt("model", "token".to_string(), "account".to_string());
        assert!(config.is_chatgpt_mode());
    }

    // --- Serialization round-trip tests ---

    #[test]
    fn test_llm_config_serialization_round_trip_full() {
        let config = LlmConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-3".to_string(),
            temperature: Some(0.8),
            max_tokens: Some(2000),
            api_base: Some("https://custom.api.com".to_string()),
            auth_mode: AuthMode::ApiKey,
            chatgpt_config: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.model, parsed.model);
        assert_eq!(config.temperature, parsed.temperature);
    }

    // --- Edge case tests ---

    #[test]
    fn test_llm_config_zero_temperature_value() {
        let config = LlmConfig {
            temperature: Some(0.0),
            ..Default::default()
        };
        assert_eq!(config.temperature, Some(0.0));
    }

    #[test]
    fn test_llm_config_max_temperature_value() {
        let config = LlmConfig {
            temperature: Some(2.0),
            ..Default::default()
        };
        assert_eq!(config.temperature, Some(2.0));
    }

    #[test]
    fn test_llm_config_zero_max_tokens_value() {
        let config = LlmConfig {
            max_tokens: Some(0),
            ..Default::default()
        };
        assert_eq!(config.max_tokens, Some(0));
    }

    #[test]
    fn test_llm_config_large_max_tokens_value() {
        let config = LlmConfig {
            max_tokens: Some(128000),
            ..Default::default()
        };
        assert_eq!(config.max_tokens, Some(128000));
    }

    #[test]
    fn test_llm_client_add_tools_incrementally() {
        let tool1 = AgentToolDefinition {
            name: "tool1".to_string(),
            description: "Tool 1".to_string(),
            parameters: serde_json::json!({}),
        };
        let tool2 = AgentToolDefinition {
            name: "tool2".to_string(),
            description: "Tool 2".to_string(),
            parameters: serde_json::json!({}),
        };
        let client = LlmClient::new().with_tool(tool1).with_tool(tool2);
        assert_eq!(client.tools.len(), 2);
    }

    #[test]
    fn test_token_usage_all_zero_values() {
        let usage = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cached_tokens: 0,
        };
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_token_usage_max_u32_values() {
        let usage = TokenUsage {
            prompt_tokens: u32::MAX,
            completion_tokens: u32::MAX,
            total_tokens: u32::MAX,
            cached_tokens: u32::MAX,
        };
        assert_eq!(usage.prompt_tokens, u32::MAX);
    }

    #[test]
    fn test_llm_response_all_none_fields() {
        let response = LlmResponse {
            content: None,
            tool_calls: vec![],
            finish_reason: None,
            usage: None,
        };
        assert!(response.content.is_none());
        assert!(response.tool_calls.is_empty());
        assert!(response.finish_reason.is_none());
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_llm_config_empty_model_name_string() {
        let config = LlmConfig::with_model("");
        assert_eq!(config.model, "");
    }

    #[test]
    fn test_llm_config_unicode_model_name_string() {
        let config = LlmConfig::with_model("模型-3.5");
        assert_eq!(config.model, "模型-3.5");
    }

    #[test]
    fn test_chatgpt_base_url_with_trailing_slash_preserved() {
        let config = ChatGptConfig {
            access_token: "token".to_string(),
            account_id: "account".to_string(),
            base_url: Some("https://example.com/api/".to_string()),
        };
        assert_eq!(config.base_url(), "https://example.com/api/");
    }

    // ===================== Retry Config Tests (Audit #38) =====================

    #[test]
    fn test_llm_retry_config_default() {
        let config = LlmRetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 5000);
        assert!((config.backoff_multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_llm_retry_config_delay_exponential_backoff() {
        let config = LlmRetryConfig {
            max_retries: 5,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
        };

        // Attempt 0: 100ms
        assert_eq!(config.delay_for_attempt(0).as_millis(), 100);
        // Attempt 1: 200ms
        assert_eq!(config.delay_for_attempt(1).as_millis(), 200);
        // Attempt 2: 400ms
        assert_eq!(config.delay_for_attempt(2).as_millis(), 400);
        // Attempt 3: 800ms
        assert_eq!(config.delay_for_attempt(3).as_millis(), 800);
        // Attempt 4: 1600ms
        assert_eq!(config.delay_for_attempt(4).as_millis(), 1600);
    }

    #[test]
    fn test_llm_retry_config_delay_capped_at_max() {
        let config = LlmRetryConfig {
            max_retries: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 3000,
            backoff_multiplier: 2.0,
        };

        // Attempt 0: 1000ms
        assert_eq!(config.delay_for_attempt(0).as_millis(), 1000);
        // Attempt 1: 2000ms
        assert_eq!(config.delay_for_attempt(1).as_millis(), 2000);
        // Attempt 2: would be 4000ms but capped at 3000ms
        assert_eq!(config.delay_for_attempt(2).as_millis(), 3000);
        // Attempt 3: capped at 3000ms
        assert_eq!(config.delay_for_attempt(3).as_millis(), 3000);
    }

    #[test]
    fn test_llm_retry_config_is_retryable_rate_limit() {
        assert!(LlmRetryConfig::is_retryable_error(
            "Error: 429 Too Many Requests"
        ));
        assert!(LlmRetryConfig::is_retryable_error("rate limit exceeded"));
    }

    #[test]
    fn test_llm_retry_config_is_retryable_server_errors() {
        assert!(LlmRetryConfig::is_retryable_error(
            "Internal Server Error 500"
        ));
        assert!(LlmRetryConfig::is_retryable_error("502 Bad Gateway"));
        assert!(LlmRetryConfig::is_retryable_error(
            "503 Service Unavailable"
        ));
        assert!(LlmRetryConfig::is_retryable_error("504 Gateway Timeout"));
    }

    #[test]
    fn test_llm_retry_config_is_retryable_network_errors() {
        assert!(LlmRetryConfig::is_retryable_error("connection refused"));
        assert!(LlmRetryConfig::is_retryable_error("timeout error"));
        assert!(LlmRetryConfig::is_retryable_error("network error"));
    }

    #[test]
    fn test_llm_retry_config_is_retryable_overload_errors() {
        assert!(LlmRetryConfig::is_retryable_error("server is overloaded"));
        assert!(LlmRetryConfig::is_retryable_error("insufficient capacity"));
    }

    #[test]
    fn test_llm_retry_config_not_retryable() {
        assert!(!LlmRetryConfig::is_retryable_error("Invalid API key"));
        assert!(!LlmRetryConfig::is_retryable_error("Model not found"));
        assert!(!LlmRetryConfig::is_retryable_error("400 Bad Request"));
        assert!(!LlmRetryConfig::is_retryable_error("401 Unauthorized"));
        assert!(!LlmRetryConfig::is_retryable_error("403 Forbidden"));
    }

    #[test]
    fn test_llm_client_with_retry_config() {
        let retry_config = LlmRetryConfig {
            max_retries: 5,
            initial_delay_ms: 200,
            max_delay_ms: 8000,
            backoff_multiplier: 1.5,
        };
        let client = LlmClient::new().with_retry_config(retry_config);
        assert_eq!(client.retry_config.max_retries, 5);
        assert_eq!(client.retry_config.initial_delay_ms, 200);
    }

    #[test]
    fn test_llm_client_clone_includes_retry_config() {
        let retry_config = LlmRetryConfig {
            max_retries: 7,
            initial_delay_ms: 250,
            max_delay_ms: 6000,
            backoff_multiplier: 1.8,
        };
        let client = LlmClient::new().with_retry_config(retry_config);
        let cloned = client.clone();
        assert_eq!(cloned.retry_config.max_retries, 7);
        assert_eq!(cloned.retry_config.initial_delay_ms, 250);
    }
}
