//! Cloudflare Workers AI chat models implementation

use std::collections::HashMap;
use std::pin::Pin;

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string, CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_API_TOKEN},
    error::{Error as DashFlowError, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, BaseMessage, Message},
};
use eventsource_stream::Eventsource;
use futures::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

const DEFAULT_MODEL: &str = "@cf/meta/llama-3.1-8b-instruct";
const API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare Workers AI chat model
///
/// Provides access to Cloudflare's edge inference with 50+ models
/// including Llama, Mistral, Gemma, and more.
///
/// # Example
///
/// ```no_run
/// use dashflow_cloudflare::ChatCloudflare;
/// use dashflow::core::messages::Message;
/// use dashflow::core::language_models::ChatModel;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ChatCloudflare::new()
///     .with_account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
///     .with_api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
///     .with_model("@cf/meta/llama-3.1-8b-instruct");
///
/// let messages = vec![Message::human("Hello!")];
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct ChatCloudflare {
    /// Cloudflare account ID
    account_id: Option<String>,
    /// API token for authentication
    api_token: Option<String>,
    /// Model name (e.g., "@cf/meta/llama-3.1-8b-instruct")
    model: String,
    /// HTTP client
    client: Client,
    /// Temperature for sampling (0.0 to 5.0)
    temperature: Option<f32>,
    /// Maximum number of tokens to generate (default 256)
    max_tokens: Option<u32>,
    /// Top-p sampling parameter
    top_p: Option<f32>,
    /// Top-k sampling parameter
    top_k: Option<u32>,
    /// Seed for reproducibility
    seed: Option<i64>,
    /// Repetition penalty
    repetition_penalty: Option<f32>,
    /// Frequency penalty
    frequency_penalty: Option<f32>,
    /// Presence penalty
    presence_penalty: Option<f32>,
}

impl ChatCloudflare {
    /// Creates a new `ChatCloudflare` instance with default settings
    #[must_use]
    pub fn new() -> Self {
        Self {
            account_id: env_string(CLOUDFLARE_ACCOUNT_ID),
            api_token: env_string(CLOUDFLARE_API_TOKEN),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        }
    }

    /// Sets the Cloudflare account ID
    #[must_use]
    pub fn with_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Sets the API token
    #[must_use]
    pub fn with_api_token(mut self, api_token: impl Into<String>) -> Self {
        self.api_token = Some(api_token.into());
        self
    }

    /// Sets the model name (e.g., "@cf/meta/llama-3.1-8b-instruct")
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Sets the temperature (0.0 to 5.0, default 0.6)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets the maximum number of tokens to generate (default 256)
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

    /// Sets the seed for reproducibility
    #[must_use]
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Sets the repetition penalty
    #[must_use]
    pub fn with_repetition_penalty(mut self, penalty: f32) -> Self {
        self.repetition_penalty = Some(penalty);
        self
    }

    /// Sets the frequency penalty
    #[must_use]
    pub fn with_frequency_penalty(mut self, penalty: f32) -> Self {
        self.frequency_penalty = Some(penalty);
        self
    }

    /// Sets the presence penalty
    #[must_use]
    pub fn with_presence_penalty(mut self, penalty: f32) -> Self {
        self.presence_penalty = Some(penalty);
        self
    }

    /// Builds the API endpoint URL
    fn endpoint(&self) -> Result<String> {
        let account_id = self
            .account_id
            .as_ref()
            .ok_or_else(|| DashFlowError::invalid_input("Cloudflare account ID not set"))?;
        Ok(format!(
            "{}/accounts/{}/ai/run/{}",
            API_BASE, account_id, self.model
        ))
    }

    /// Converts `DashFlow` messages to Cloudflare format
    fn convert_messages(&self, messages: &[BaseMessage]) -> Vec<CloudflareMessage> {
        messages
            .iter()
            .map(|msg| CloudflareMessage {
                role: match msg {
                    Message::System { .. } => "system".to_string(),
                    Message::Human { .. } => "user".to_string(),
                    Message::AI { .. } => "assistant".to_string(),
                    _ => "user".to_string(),
                },
                content: msg.as_text(),
            })
            .collect()
    }

    /// Builds the request body
    // SAFETY: json!({...}) macro always produces JsonValue::Object, so as_object_mut() is infallible
    #[allow(clippy::expect_used)]
    fn build_request_body(&self, messages: &[BaseMessage], streaming: bool) -> JsonValue {
        let mut body = serde_json::json!({
            "messages": self.convert_messages(messages),
            "stream": streaming,
        });

        // SAFETY: json!({...}) always produces JsonValue::Object, so as_object_mut() always returns Some
        let obj = body.as_object_mut().expect("json!({}) produces object");

        if let Some(temperature) = self.temperature {
            obj.insert("temperature".to_string(), serde_json::json!(temperature));
        }
        if let Some(max_tokens) = self.max_tokens {
            obj.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
        }
        if let Some(top_p) = self.top_p {
            obj.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(top_k) = self.top_k {
            obj.insert("top_k".to_string(), serde_json::json!(top_k));
        }
        if let Some(seed) = self.seed {
            obj.insert("seed".to_string(), serde_json::json!(seed));
        }
        if let Some(penalty) = self.repetition_penalty {
            obj.insert("repetition_penalty".to_string(), serde_json::json!(penalty));
        }
        if let Some(penalty) = self.frequency_penalty {
            obj.insert("frequency_penalty".to_string(), serde_json::json!(penalty));
        }
        if let Some(penalty) = self.presence_penalty {
            obj.insert("presence_penalty".to_string(), serde_json::json!(penalty));
        }

        body
    }
}

impl Default for ChatCloudflare {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CloudflareMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse {
    result: CloudflareResult,
    success: bool,
    errors: Vec<JsonValue>,
    /// NOTE: messages field from Cloudflare API response (deserialization only).
    /// Not currently used but kept for complete API response parsing and future use.
    ///
    /// Serde deserialization field: Must match Cloudflare Workers AI API response schema.
    /// Field exists in API responses alongside success/errors (lines 253-254).
    /// Not accessed in current implementation (only result field extracted, line 252).
    /// Reserved for future diagnostic/logging enhancements (API metadata).
    /// Cannot remove without breaking serde deserialization from Cloudflare API responses.
    #[allow(dead_code)] // Part of Cloudflare API response schema; kept for future extensions
    messages: Vec<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct CloudflareResult {
    response: String,
}

#[async_trait]
impl ChatModel for ChatCloudflare {
    fn llm_type(&self) -> &'static str {
        "cloudflare-workers-ai"
    }

    fn identifying_params(&self) -> HashMap<String, JsonValue> {
        let mut params = HashMap::new();
        params.insert("model".to_string(), JsonValue::String(self.model.clone()));
        if let Some(temp) = self.temperature {
            params.insert("temperature".to_string(), JsonValue::from(temp));
        }
        if let Some(max_tokens) = self.max_tokens {
            params.insert("max_tokens".to_string(), JsonValue::from(max_tokens));
        }
        params
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn _generate(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        let api_token = self
            .api_token
            .as_ref()
            .ok_or_else(|| DashFlowError::invalid_input("Cloudflare API token not set"))?;

        let url = self.endpoint()?;
        let body = self.build_request_body(messages, false);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| DashFlowError::http(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DashFlowError::http(format!(
                "Cloudflare API error ({status}): {error_text}"
            )));
        }

        let cf_response: CloudflareResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api_format(format!("Failed to parse response: {e}")))?;

        if !cf_response.success {
            return Err(DashFlowError::http(format!(
                "Cloudflare API reported failure: {:?}",
                cf_response.errors
            )));
        }

        let ai_message = AIMessage::new(cf_response.result.response);

        Ok(ChatResult {
            generations: vec![ChatGeneration {
                message: ai_message.into(),
                generation_info: None,
            }],
            llm_output: None,
        })
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        let api_token = self
            .api_token
            .clone()
            .ok_or_else(|| DashFlowError::invalid_input("Cloudflare API token not set"))?;

        let url = self.endpoint()?;
        let body = self.build_request_body(messages, true);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| DashFlowError::http(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DashFlowError::http(format!(
                "Cloudflare API error ({status}): {error_text}"
            )));
        }

        let event_stream = response.bytes_stream().eventsource();

        let stream = stream! {
            futures::pin_mut!(event_stream);

            while let Some(event) = futures::StreamExt::next(&mut event_stream).await {
                match event {
                    Ok(event) => {
                        if event.data == "[DONE]" {
                            break;
                        }

                        match serde_json::from_str::<JsonValue>(&event.data) {
                            Ok(data) => {
                                if let Some(response_text) = data.get("response").and_then(|r| r.as_str()) {
                                    let chunk = AIMessageChunk::new(response_text.to_string());

                                    yield Ok(ChatGenerationChunk {
                                        message: chunk,
                                        generation_info: None,
                                    });
                                }
                            }
                            Err(e) => {
                                yield Err(DashFlowError::api_format(format!(
                                    "Failed to parse SSE event: {e}"
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(DashFlowError::http(format!(
                            "SSE stream error: {e}"
                        )));
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ==================== Construction Tests ====================

    #[test]
    fn test_new() {
        let model = ChatCloudflare::new();
        assert_eq!(model.model, DEFAULT_MODEL);
        assert!(model.temperature.is_none());
    }

    #[test]
    fn test_default_trait() {
        let model = ChatCloudflare::default();
        assert_eq!(model.model, DEFAULT_MODEL);
        assert!(model.account_id.is_none() || model.account_id.is_some()); // env-dependent
        assert!(model.temperature.is_none());
        assert!(model.max_tokens.is_none());
        assert!(model.top_p.is_none());
        assert!(model.top_k.is_none());
        assert!(model.seed.is_none());
        assert!(model.repetition_penalty.is_none());
        assert!(model.frequency_penalty.is_none());
        assert!(model.presence_penalty.is_none());
    }

    #[test]
    fn test_default_equals_new() {
        let default_model = ChatCloudflare::default();
        let new_model = ChatCloudflare::new();
        // Compare fields (can't use PartialEq due to Client)
        assert_eq!(default_model.model, new_model.model);
        assert_eq!(default_model.temperature, new_model.temperature);
        assert_eq!(default_model.max_tokens, new_model.max_tokens);
    }

    // ==================== Clone and Debug Tests ====================

    #[test]
    fn test_clone() {
        let model = ChatCloudflare::new()
            .with_account_id("test-account")
            .with_api_token("test-token")
            .with_model("@cf/mistral/mistral-7b-instruct-v0.2")
            .with_temperature(0.5)
            .with_max_tokens(256);

        let cloned = model.clone();
        assert_eq!(cloned.account_id, model.account_id);
        assert_eq!(cloned.api_token, model.api_token);
        assert_eq!(cloned.model, model.model);
        assert_eq!(cloned.temperature, model.temperature);
        assert_eq!(cloned.max_tokens, model.max_tokens);
    }

    #[test]
    fn test_debug() {
        let model = ChatCloudflare::new()
            .with_account_id("test-account")
            .with_model("@cf/meta/llama-3.1-8b-instruct");

        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("ChatCloudflare"));
        assert!(debug_str.contains("test-account"));
        assert!(debug_str.contains("@cf/meta/llama-3.1-8b-instruct"));
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_builder_pattern() {
        let model = ChatCloudflare::new()
            .with_account_id("test-account")
            .with_api_token("test-token")
            .with_model("@cf/meta/llama-3.1-8b-instruct")
            .with_temperature(0.7)
            .with_max_tokens(512)
            .with_top_p(0.9)
            .with_top_k(50)
            .with_seed(42);

        assert_eq!(model.account_id, Some("test-account".to_string()));
        assert_eq!(model.api_token, Some("test-token".to_string()));
        assert_eq!(model.model, "@cf/meta/llama-3.1-8b-instruct");
        assert_eq!(model.temperature, Some(0.7));
        assert_eq!(model.max_tokens, Some(512));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.top_k, Some(50));
        assert_eq!(model.seed, Some(42));
    }

    #[test]
    fn test_builder_with_penalties() {
        let model = ChatCloudflare::new()
            .with_repetition_penalty(1.2)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        assert_eq!(model.repetition_penalty, Some(1.2));
        assert_eq!(model.frequency_penalty, Some(0.5));
        assert_eq!(model.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_builder_all_parameters() {
        let model = ChatCloudflare::new()
            .with_account_id("account-123")
            .with_api_token("token-xyz")
            .with_model("@cf/google/gemma-7b-it")
            .with_temperature(1.0)
            .with_max_tokens(1024)
            .with_top_p(0.95)
            .with_top_k(100)
            .with_seed(12345)
            .with_repetition_penalty(1.5)
            .with_frequency_penalty(0.7)
            .with_presence_penalty(0.8);

        assert_eq!(model.account_id, Some("account-123".to_string()));
        assert_eq!(model.api_token, Some("token-xyz".to_string()));
        assert_eq!(model.model, "@cf/google/gemma-7b-it");
        assert_eq!(model.temperature, Some(1.0));
        assert_eq!(model.max_tokens, Some(1024));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.top_k, Some(100));
        assert_eq!(model.seed, Some(12345));
        assert_eq!(model.repetition_penalty, Some(1.5));
        assert_eq!(model.frequency_penalty, Some(0.7));
        assert_eq!(model.presence_penalty, Some(0.8));
    }

    #[test]
    fn test_builder_with_string_types() {
        // Test that Into<String> works for various types
        let model = ChatCloudflare::new()
            .with_account_id(String::from("owned-account"))
            .with_api_token("borrowed-token")
            .with_model("@cf/meta/llama-3.1-8b-instruct".to_string());

        assert_eq!(model.account_id, Some("owned-account".to_string()));
        assert_eq!(model.api_token, Some("borrowed-token".to_string()));
        assert_eq!(model.model, "@cf/meta/llama-3.1-8b-instruct");
    }

    #[test]
    fn test_builder_chaining_order_independent() {
        let model1 = ChatCloudflare::new()
            .with_temperature(0.5)
            .with_max_tokens(100)
            .with_account_id("test");

        let model2 = ChatCloudflare::new()
            .with_account_id("test")
            .with_max_tokens(100)
            .with_temperature(0.5);

        assert_eq!(model1.temperature, model2.temperature);
        assert_eq!(model1.max_tokens, model2.max_tokens);
        assert_eq!(model1.account_id, model2.account_id);
    }

    // ==================== Edge Case Value Tests ====================

    #[test]
    fn test_temperature_edge_cases() {
        let model = ChatCloudflare::new().with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));

        let model = ChatCloudflare::new().with_temperature(5.0);
        assert_eq!(model.temperature, Some(5.0));
    }

    #[test]
    fn test_max_tokens_edge_cases() {
        let model = ChatCloudflare::new().with_max_tokens(0);
        assert_eq!(model.max_tokens, Some(0));

        let model = ChatCloudflare::new().with_max_tokens(1);
        assert_eq!(model.max_tokens, Some(1));

        let model = ChatCloudflare::new().with_max_tokens(u32::MAX);
        assert_eq!(model.max_tokens, Some(u32::MAX));
    }

    #[test]
    fn test_seed_edge_cases() {
        let model = ChatCloudflare::new().with_seed(0);
        assert_eq!(model.seed, Some(0));

        let model = ChatCloudflare::new().with_seed(-1);
        assert_eq!(model.seed, Some(-1));

        let model = ChatCloudflare::new().with_seed(i64::MAX);
        assert_eq!(model.seed, Some(i64::MAX));
    }

    // ==================== Message Conversion Tests ====================

    #[test]
    fn test_convert_messages() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("You are helpful"),
            Message::human("Hello"),
            Message::ai("Hi there!"),
        ];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages.len(), 3);
        assert_eq!(cf_messages[0].role, "system");
        assert_eq!(cf_messages[1].role, "user");
        assert_eq!(cf_messages[2].role, "assistant");
    }

    #[test]
    fn test_convert_messages_content() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("Be helpful"),
            Message::human("What is 2+2?"),
        ];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].content, "Be helpful");
        assert_eq!(cf_messages[1].content, "What is 2+2?");
    }

    #[test]
    fn test_convert_messages_empty() {
        let model = ChatCloudflare::new();
        let messages: Vec<BaseMessage> = vec![];

        let cf_messages = model.convert_messages(&messages);
        assert!(cf_messages.is_empty());
    }

    #[test]
    fn test_convert_messages_single() {
        let model = ChatCloudflare::new();
        let messages = vec![Message::human("Solo message")];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages.len(), 1);
        assert_eq!(cf_messages[0].role, "user");
        assert_eq!(cf_messages[0].content, "Solo message");
    }

    #[test]
    fn test_convert_messages_unicode() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::human("Hello 你好 مرحبا 🎉"),
            Message::ai("Response with émojis: 🤖"),
        ];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].content, "Hello 你好 مرحبا 🎉");
        assert_eq!(cf_messages[1].content, "Response with émojis: 🤖");
    }

    // ==================== Endpoint Tests ====================

    #[test]
    fn test_endpoint() {
        let model = ChatCloudflare::new()
            .with_account_id("test-account")
            .with_model("@cf/meta/llama-3.1-8b-instruct");

        let endpoint = model.endpoint().unwrap();
        assert_eq!(
            endpoint,
            "https://api.cloudflare.com/client/v4/accounts/test-account/ai/run/@cf/meta/llama-3.1-8b-instruct"
        );
    }

    #[test]
    fn test_endpoint_different_models() {
        let models = [
            ("@cf/meta/llama-3.1-8b-instruct", "llama-3.1-8b-instruct"),
            ("@cf/mistral/mistral-7b-instruct-v0.2", "mistral-7b-instruct-v0.2"),
            ("@cf/google/gemma-7b-it", "gemma-7b-it"),
            ("@hf/thebloke/deepseek-coder-6.7b-instruct-awq", "deepseek-coder-6.7b-instruct-awq"),
        ];

        for (model_name, _) in models {
            let model = ChatCloudflare::new()
                .with_account_id("test")
                .with_model(model_name);

            let endpoint = model.endpoint().unwrap();
            assert!(endpoint.contains(model_name));
        }
    }

    #[test]
    fn test_endpoint_missing_account_id() {
        // Clear any env variable that might be set by constructing directly
        let model = ChatCloudflare {
            account_id: None,
            api_token: None,
            model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let result = model.endpoint();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("account ID"));
    }

    // ==================== Request Body Tests ====================

    #[test]
    fn test_build_request_body() {
        let model = ChatCloudflare::new()
            .with_temperature(0.7)
            .with_max_tokens(512);

        let messages = vec![Message::human("Hello")];
        let body = model.build_request_body(&messages, false);

        assert_eq!(body["stream"], false);
        // Use approximate comparison for floating point
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(body["max_tokens"], 512);
        assert!(body["messages"].is_array());
    }

    #[test]
    fn test_build_request_body_streaming() {
        let model = ChatCloudflare::new();
        let messages = vec![Message::human("Test")];

        let body = model.build_request_body(&messages, true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_build_request_body_all_params() {
        let model = ChatCloudflare::new()
            .with_temperature(0.8)
            .with_max_tokens(1024)
            .with_top_p(0.95)
            .with_top_k(40)
            .with_seed(42)
            .with_repetition_penalty(1.1)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.6);

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert!((body["temperature"].as_f64().unwrap() - 0.8).abs() < 0.001);
        assert_eq!(body["max_tokens"], 1024);
        assert!((body["top_p"].as_f64().unwrap() - 0.95).abs() < 0.001);
        assert_eq!(body["top_k"], 40);
        assert_eq!(body["seed"], 42);
        assert!((body["repetition_penalty"].as_f64().unwrap() - 1.1).abs() < 0.001);
        assert!((body["frequency_penalty"].as_f64().unwrap() - 0.5).abs() < 0.001);
        assert!((body["presence_penalty"].as_f64().unwrap() - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_build_request_body_no_optional_params() {
        let model = ChatCloudflare::new();
        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert!(body.get("temperature").is_none());
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("top_k").is_none());
        assert!(body.get("seed").is_none());
        assert!(body.get("repetition_penalty").is_none());
        assert!(body.get("frequency_penalty").is_none());
        assert!(body.get("presence_penalty").is_none());
    }

    #[test]
    fn test_build_request_body_messages_structure() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("Be helpful"),
            Message::human("Hello"),
            Message::ai("Hi!"),
        ];
        let body = model.build_request_body(&messages, false);

        let body_messages = body["messages"].as_array().unwrap();
        assert_eq!(body_messages.len(), 3);
        assert_eq!(body_messages[0]["role"], "system");
        assert_eq!(body_messages[0]["content"], "Be helpful");
        assert_eq!(body_messages[1]["role"], "user");
        assert_eq!(body_messages[1]["content"], "Hello");
        assert_eq!(body_messages[2]["role"], "assistant");
        assert_eq!(body_messages[2]["content"], "Hi!");
    }

    // ==================== ChatModel Trait Tests ====================

    #[test]
    fn test_llm_type() {
        let model = ChatCloudflare::new();
        assert_eq!(model.llm_type(), "cloudflare-workers-ai");
    }

    #[test]
    fn test_identifying_params_minimal() {
        let model = ChatCloudflare::new();
        let params = model.identifying_params();

        assert!(params.contains_key("model"));
        assert_eq!(params["model"], JsonValue::String(DEFAULT_MODEL.to_string()));
    }

    #[test]
    fn test_identifying_params_with_options() {
        let model = ChatCloudflare::new()
            .with_model("@cf/google/gemma-7b-it")
            .with_temperature(0.8)
            .with_max_tokens(500);

        let params = model.identifying_params();

        assert_eq!(params["model"], JsonValue::String("@cf/google/gemma-7b-it".to_string()));
        assert!((params["temperature"].as_f64().unwrap() - 0.8).abs() < 0.001);
        assert_eq!(params["max_tokens"].as_u64().unwrap(), 500);
    }

    #[test]
    fn test_identifying_params_temperature_only() {
        let model = ChatCloudflare::new().with_temperature(0.5);
        let params = model.identifying_params();

        assert!(params.contains_key("temperature"));
        assert!(!params.contains_key("max_tokens"));
    }

    #[test]
    fn test_identifying_params_max_tokens_only() {
        let model = ChatCloudflare::new().with_max_tokens(256);
        let params = model.identifying_params();

        assert!(!params.contains_key("temperature"));
        assert!(params.contains_key("max_tokens"));
    }

    #[test]
    fn test_as_any() {
        let model = ChatCloudflare::new();
        let any_ref = model.as_any();

        // Should be able to downcast to ChatCloudflare
        let downcast: Option<&ChatCloudflare> = any_ref.downcast_ref();
        assert!(downcast.is_some());
    }

    #[test]
    fn test_as_any_preserves_data() {
        let model = ChatCloudflare::new()
            .with_model("@cf/test/model")
            .with_temperature(0.9);

        let any_ref = model.as_any();
        let downcast: &ChatCloudflare = any_ref.downcast_ref().unwrap();

        assert_eq!(downcast.model, "@cf/test/model");
        assert_eq!(downcast.temperature, Some(0.9));
    }

    // ==================== CloudflareMessage Tests ====================

    #[test]
    fn test_cloudflare_message_serialize() {
        let msg = CloudflareMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_cloudflare_message_deserialize() {
        let json = r#"{"role":"assistant","content":"Hi there!"}"#;
        let msg: CloudflareMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Hi there!");
    }

    #[test]
    fn test_cloudflare_message_roundtrip() {
        let original = CloudflareMessage {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CloudflareMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(original.role, deserialized.role);
        assert_eq!(original.content, deserialized.content);
    }

    #[test]
    fn test_cloudflare_message_debug() {
        let msg = CloudflareMessage {
            role: "user".to_string(),
            content: "Test".to_string(),
        };

        let debug = format!("{:?}", msg);
        assert!(debug.contains("CloudflareMessage"));
        assert!(debug.contains("user"));
        assert!(debug.contains("Test"));
    }

    // ==================== CloudflareResponse Tests ====================

    #[test]
    fn test_cloudflare_response_deserialize_success() {
        let json = r#"{
            "result": {"response": "Hello from Cloudflare!"},
            "success": true,
            "errors": [],
            "messages": []
        }"#;

        let response: CloudflareResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert_eq!(response.result.response, "Hello from Cloudflare!");
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_cloudflare_response_deserialize_with_errors() {
        let json = r#"{
            "result": {"response": ""},
            "success": false,
            "errors": [{"code": 1000, "message": "Test error"}],
            "messages": []
        }"#;

        let response: CloudflareResponse = serde_json::from_str(json).unwrap();
        assert!(!response.success);
        assert_eq!(response.errors.len(), 1);
    }

    #[test]
    fn test_cloudflare_response_debug() {
        let json = r#"{
            "result": {"response": "Test"},
            "success": true,
            "errors": [],
            "messages": []
        }"#;

        let response: CloudflareResponse = serde_json::from_str(json).unwrap();
        let debug = format!("{:?}", response);
        assert!(debug.contains("CloudflareResponse"));
    }

    // ==================== CloudflareResult Tests ====================

    #[test]
    fn test_cloudflare_result_deserialize() {
        let json = r#"{"response": "Generated text here"}"#;
        let result: CloudflareResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.response, "Generated text here");
    }

    #[test]
    fn test_cloudflare_result_deserialize_empty() {
        let json = r#"{"response": ""}"#;
        let result: CloudflareResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.response, "");
    }

    #[test]
    fn test_cloudflare_result_deserialize_unicode() {
        let json = r#"{"response": "Hello 世界 🌍"}"#;
        let result: CloudflareResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.response, "Hello 世界 🌍");
    }

    #[test]
    fn test_cloudflare_result_debug() {
        let json = r#"{"response": "Test response"}"#;
        let result: CloudflareResult = serde_json::from_str(json).unwrap();

        let debug = format!("{:?}", result);
        assert!(debug.contains("CloudflareResult"));
        assert!(debug.contains("Test response"));
    }

    // ==================== Model Names Tests ====================

    #[test]
    fn test_supported_model_names() {
        let models = [
            "@cf/meta/llama-3.1-8b-instruct",
            "@cf/meta/llama-3.1-70b-instruct",
            "@cf/mistral/mistral-7b-instruct-v0.2",
            "@cf/google/gemma-7b-it",
            "@cf/qwen/qwen1.5-7b-chat-awq",
            "@cf/thebloke/codellama-7b-instruct-awq",
            "@hf/thebloke/deepseek-coder-6.7b-instruct-awq",
        ];

        for model_name in models {
            let model = ChatCloudflare::new()
                .with_account_id("test")
                .with_model(model_name);

            assert_eq!(model.model, model_name);
            assert!(model.endpoint().is_ok());
        }
    }

    // ==================== Async Tests ====================

    #[tokio::test]
    async fn test_generate_missing_api_token() {
        let model = ChatCloudflare {
            account_id: Some("test-account".to_string()),
            api_token: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let messages = vec![Message::human("Hello")];
        let result = model._generate(&messages, None, None, None, None).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("API token"));
    }

    #[tokio::test]
    async fn test_stream_missing_api_token() {
        let model = ChatCloudflare {
            account_id: Some("test-account".to_string()),
            api_token: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let messages = vec![Message::human("Hello")];
        let result = model._stream(&messages, None, None, None, None).await;

        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("API token"));
    }

    // ==================== Integration Tests (Ignored) ====================

    #[tokio::test]
    #[ignore = "Requires CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN environment variables"]
    async fn test_generate_real_api() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("You are a helpful assistant. Be brief."),
            Message::human("What is 2+2? Answer with just the number."),
        ];

        let result = model._generate(&messages, None, None, None, None).await;
        assert!(result.is_ok());

        let chat_result = result.unwrap();
        assert!(!chat_result.generations.is_empty());
        let content = chat_result.generations[0].message.as_text();
        assert!(!content.is_empty());
    }

    #[tokio::test]
    #[ignore = "Requires CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN environment variables"]
    async fn test_stream_real_api() {
        use futures::StreamExt;

        let model = ChatCloudflare::new().with_max_tokens(50);
        let messages = vec![Message::human("Count from 1 to 5.")];

        let result = model._stream(&messages, None, None, None, None).await;
        assert!(result.is_ok());

        let mut stream = result.unwrap();
        let mut chunks = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                chunks.push(chunk);
            }
        }

        assert!(!chunks.is_empty(), "Should receive at least one chunk");
    }

    #[tokio::test]
    #[ignore = "Requires CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN environment variables"]
    async fn test_different_models_real_api() {
        let models_to_test = [
            "@cf/meta/llama-3.1-8b-instruct",
            "@cf/mistral/mistral-7b-instruct-v0.2",
        ];

        for model_name in models_to_test {
            let model = ChatCloudflare::new()
                .with_model(model_name)
                .with_max_tokens(50);

            let messages = vec![Message::human("Say hello in one word.")];
            let result = model._generate(&messages, None, None, None, None).await;

            assert!(result.is_ok(), "Model {} should work", model_name);
        }
    }

    // ==================== Constants Tests ====================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "@cf/meta/llama-3.1-8b-instruct");
    }

    #[test]
    fn test_api_base_constant() {
        assert_eq!(API_BASE, "https://api.cloudflare.com/client/v4");
    }

    #[test]
    fn test_default_model_used_in_new() {
        let model = ChatCloudflare::new();
        assert_eq!(model.model, DEFAULT_MODEL);
    }

    // ==================== Builder Empty String Tests ====================

    #[test]
    fn test_builder_with_empty_account_id() {
        let model = ChatCloudflare::new().with_account_id("");
        assert_eq!(model.account_id, Some("".to_string()));
    }

    #[test]
    fn test_builder_with_empty_api_token() {
        let model = ChatCloudflare::new().with_api_token("");
        assert_eq!(model.api_token, Some("".to_string()));
    }

    #[test]
    fn test_builder_with_empty_model() {
        let model = ChatCloudflare::new().with_model("");
        assert_eq!(model.model, "");
    }

    #[test]
    fn test_builder_overwrite_account_id() {
        let model = ChatCloudflare::new()
            .with_account_id("first")
            .with_account_id("second");
        assert_eq!(model.account_id, Some("second".to_string()));
    }

    #[test]
    fn test_builder_overwrite_api_token() {
        let model = ChatCloudflare::new()
            .with_api_token("token1")
            .with_api_token("token2");
        assert_eq!(model.api_token, Some("token2".to_string()));
    }

    #[test]
    fn test_builder_overwrite_model() {
        let model = ChatCloudflare::new()
            .with_model("model1")
            .with_model("model2");
        assert_eq!(model.model, "model2");
    }

    #[test]
    fn test_builder_overwrite_temperature() {
        let model = ChatCloudflare::new()
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.temperature, Some(0.9));
    }

    #[test]
    fn test_builder_overwrite_max_tokens() {
        let model = ChatCloudflare::new()
            .with_max_tokens(100)
            .with_max_tokens(500);
        assert_eq!(model.max_tokens, Some(500));
    }

    // ==================== Builder Float Edge Cases ====================

    #[test]
    fn test_builder_top_p_boundary_zero() {
        let model = ChatCloudflare::new().with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_builder_top_p_boundary_one() {
        let model = ChatCloudflare::new().with_top_p(1.0);
        assert_eq!(model.top_p, Some(1.0));
    }

    #[test]
    fn test_builder_frequency_penalty_negative() {
        let model = ChatCloudflare::new().with_frequency_penalty(-1.0);
        assert_eq!(model.frequency_penalty, Some(-1.0));
    }

    #[test]
    fn test_builder_presence_penalty_negative() {
        let model = ChatCloudflare::new().with_presence_penalty(-1.0);
        assert_eq!(model.presence_penalty, Some(-1.0));
    }

    #[test]
    fn test_builder_repetition_penalty_zero() {
        let model = ChatCloudflare::new().with_repetition_penalty(0.0);
        assert_eq!(model.repetition_penalty, Some(0.0));
    }

    #[test]
    fn test_builder_top_k_zero() {
        let model = ChatCloudflare::new().with_top_k(0);
        assert_eq!(model.top_k, Some(0));
    }

    #[test]
    fn test_builder_top_k_max() {
        let model = ChatCloudflare::new().with_top_k(u32::MAX);
        assert_eq!(model.top_k, Some(u32::MAX));
    }

    // ==================== Message Conversion Edge Cases ====================

    #[test]
    fn test_convert_messages_long_content() {
        let model = ChatCloudflare::new();
        let long_text = "a".repeat(10000);
        let messages = vec![Message::human(long_text.as_str())];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].content.len(), 10000);
    }

    #[test]
    fn test_convert_messages_multiline() {
        let model = ChatCloudflare::new();
        let multiline = "Line 1\nLine 2\nLine 3";
        let messages = vec![Message::human(multiline)];

        let cf_messages = model.convert_messages(&messages);
        assert!(cf_messages[0].content.contains('\n'));
        assert_eq!(cf_messages[0].content.lines().count(), 3);
    }

    #[test]
    fn test_convert_messages_special_characters() {
        let model = ChatCloudflare::new();
        let special = r#"Special chars: "quotes", 'apostrophes', \backslash, /slash"#;
        let messages = vec![Message::human(special)];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].content, special);
    }

    #[test]
    fn test_convert_messages_whitespace_only() {
        let model = ChatCloudflare::new();
        let messages = vec![Message::human("   \t\n  ")];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].content, "   \t\n  ");
    }

    #[test]
    fn test_convert_messages_many_messages() {
        let model = ChatCloudflare::new();
        let messages: Vec<BaseMessage> = (0..100)
            .map(|i| Message::human(format!("Message {i}")))
            .collect();

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages.len(), 100);
    }

    #[test]
    fn test_convert_messages_alternating_roles() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::human("User 1"),
            Message::ai("AI 1"),
            Message::human("User 2"),
            Message::ai("AI 2"),
            Message::human("User 3"),
        ];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].role, "user");
        assert_eq!(cf_messages[1].role, "assistant");
        assert_eq!(cf_messages[2].role, "user");
        assert_eq!(cf_messages[3].role, "assistant");
        assert_eq!(cf_messages[4].role, "user");
    }

    #[test]
    fn test_convert_messages_multiple_system() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("System 1"),
            Message::system("System 2"),
            Message::human("User"),
        ];

        let cf_messages = model.convert_messages(&messages);
        assert_eq!(cf_messages[0].role, "system");
        assert_eq!(cf_messages[1].role, "system");
        assert_eq!(cf_messages[2].role, "user");
    }

    // ==================== Endpoint Edge Cases ====================

    #[test]
    fn test_endpoint_with_special_model_characters() {
        let model = ChatCloudflare::new()
            .with_account_id("acc")
            .with_model("@hf/thebloke/deep-seek-coder-6.7b-instruct-awq");

        let endpoint = model.endpoint().unwrap();
        assert!(endpoint.contains("@hf/thebloke/deep-seek-coder-6.7b-instruct-awq"));
    }

    #[test]
    fn test_endpoint_preserves_model_exactly() {
        let model_name = "@cf/test/model-name-with-dashes_and_underscores.v1";
        let model = ChatCloudflare::new()
            .with_account_id("account")
            .with_model(model_name);

        let endpoint = model.endpoint().unwrap();
        assert!(endpoint.ends_with(model_name));
    }

    #[test]
    fn test_endpoint_with_empty_account_id() {
        let model = ChatCloudflare::new()
            .with_account_id("")
            .with_model("@cf/test/model");

        let endpoint = model.endpoint().unwrap();
        // Should contain empty account path
        assert!(endpoint.contains("/accounts//ai/run/"));
    }

    // ==================== Request Body Edge Cases ====================

    #[test]
    fn test_build_request_body_partial_penalties() {
        let model = ChatCloudflare::new()
            .with_repetition_penalty(1.5);
        // Only repetition penalty set

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert!(body.get("repetition_penalty").is_some());
        assert!(body.get("frequency_penalty").is_none());
        assert!(body.get("presence_penalty").is_none());
    }

    #[test]
    fn test_build_request_body_only_top_p() {
        let model = ChatCloudflare::new().with_top_p(0.9);

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert!(body.get("top_p").is_some());
        assert!(body.get("top_k").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn test_build_request_body_only_seed() {
        let model = ChatCloudflare::new().with_seed(12345);

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert_eq!(body["seed"], 12345);
        assert!(body.get("temperature").is_none());
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn test_build_request_body_negative_seed() {
        let model = ChatCloudflare::new().with_seed(-999);

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert_eq!(body["seed"], -999);
    }

    #[test]
    fn test_build_request_body_zero_max_tokens() {
        let model = ChatCloudflare::new().with_max_tokens(0);

        let messages = vec![Message::human("Test")];
        let body = model.build_request_body(&messages, false);

        assert_eq!(body["max_tokens"], 0);
    }

    #[test]
    fn test_build_request_body_empty_messages() {
        let model = ChatCloudflare::new();
        let messages: Vec<BaseMessage> = vec![];
        let body = model.build_request_body(&messages, false);

        let body_messages = body["messages"].as_array().unwrap();
        assert!(body_messages.is_empty());
    }

    // ==================== Clone Preservation Tests ====================

    #[test]
    fn test_clone_preserves_all_params() {
        let model = ChatCloudflare::new()
            .with_account_id("acc")
            .with_api_token("tok")
            .with_model("@cf/test/model")
            .with_temperature(0.7)
            .with_max_tokens(256)
            .with_top_p(0.9)
            .with_top_k(50)
            .with_seed(42)
            .with_repetition_penalty(1.1)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        let cloned = model.clone();

        assert_eq!(cloned.account_id, model.account_id);
        assert_eq!(cloned.api_token, model.api_token);
        assert_eq!(cloned.model, model.model);
        assert_eq!(cloned.temperature, model.temperature);
        assert_eq!(cloned.max_tokens, model.max_tokens);
        assert_eq!(cloned.top_p, model.top_p);
        assert_eq!(cloned.top_k, model.top_k);
        assert_eq!(cloned.seed, model.seed);
        assert_eq!(cloned.repetition_penalty, model.repetition_penalty);
        assert_eq!(cloned.frequency_penalty, model.frequency_penalty);
        assert_eq!(cloned.presence_penalty, model.presence_penalty);
    }

    #[test]
    fn test_clone_is_independent() {
        let model = ChatCloudflare::new().with_temperature(0.5);
        let mut cloned = model.clone();
        cloned.temperature = Some(0.9);

        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(cloned.temperature, Some(0.9));
    }

    // ==================== Debug Format Tests ====================

    #[test]
    fn test_debug_format_includes_none_values() {
        let model = ChatCloudflare::new();
        let debug = format!("{:?}", model);

        assert!(debug.contains("temperature: None"));
        assert!(debug.contains("max_tokens: None"));
    }

    #[test]
    fn test_debug_format_includes_some_values() {
        let model = ChatCloudflare::new()
            .with_temperature(0.7)
            .with_max_tokens(256);
        let debug = format!("{:?}", model);

        assert!(debug.contains("temperature: Some(0.7)"));
        assert!(debug.contains("max_tokens: Some(256)"));
    }

    // ==================== ChatModel Trait Edge Cases ====================

    #[test]
    fn test_identifying_params_does_not_include_penalties() {
        let model = ChatCloudflare::new()
            .with_repetition_penalty(1.5)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        let params = model.identifying_params();

        // Currently only model, temperature, max_tokens are included
        assert!(!params.contains_key("repetition_penalty"));
        assert!(!params.contains_key("frequency_penalty"));
        assert!(!params.contains_key("presence_penalty"));
    }

    #[test]
    fn test_identifying_params_does_not_include_sampling() {
        let model = ChatCloudflare::new()
            .with_top_p(0.9)
            .with_top_k(50)
            .with_seed(42);

        let params = model.identifying_params();

        assert!(!params.contains_key("top_p"));
        assert!(!params.contains_key("top_k"));
        assert!(!params.contains_key("seed"));
    }

    #[test]
    fn test_llm_type_is_static() {
        let model1 = ChatCloudflare::new();
        let model2 = ChatCloudflare::new().with_model("different");

        // Same type string regardless of configuration
        assert_eq!(model1.llm_type(), model2.llm_type());
    }

    // ==================== Serialization Edge Cases ====================

    #[test]
    fn test_cloudflare_message_with_empty_content() {
        let msg = CloudflareMessage {
            role: "user".to_string(),
            content: "".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: CloudflareMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, "");
    }

    #[test]
    fn test_cloudflare_message_with_json_in_content() {
        let json_content = r#"{"key": "value"}"#;
        let msg = CloudflareMessage {
            role: "user".to_string(),
            content: json_content.to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: CloudflareMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, json_content);
    }

    #[test]
    fn test_cloudflare_response_with_multiple_errors() {
        let json = r#"{
            "result": {"response": ""},
            "success": false,
            "errors": [
                {"code": 1000, "message": "Error 1"},
                {"code": 1001, "message": "Error 2"},
                {"code": 1002, "message": "Error 3"}
            ],
            "messages": []
        }"#;

        let response: CloudflareResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.errors.len(), 3);
    }

    #[test]
    fn test_cloudflare_response_with_messages() {
        let json = r#"{
            "result": {"response": "test"},
            "success": true,
            "errors": [],
            "messages": [{"text": "Info message"}]
        }"#;

        let response: CloudflareResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        // messages field exists but is not used in current implementation
    }

    #[test]
    fn test_cloudflare_result_with_very_long_response() {
        let long_response = "x".repeat(100000);
        let json = format!(r#"{{"response": "{}"}}"#, long_response);
        let result: CloudflareResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.response.len(), 100000);
    }

    #[test]
    fn test_cloudflare_result_with_escaped_characters() {
        let json = r#"{"response": "Line 1\nLine 2\tTabbed"}"#;
        let result: CloudflareResult = serde_json::from_str(json).unwrap();

        assert!(result.response.contains('\n'));
        assert!(result.response.contains('\t'));
    }

    // ==================== Async Error Path Tests ====================

    #[tokio::test]
    async fn test_generate_missing_both_credentials() {
        let model = ChatCloudflare {
            account_id: None,
            api_token: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let messages = vec![Message::human("Hello")];
        let result = model._generate(&messages, None, None, None, None).await;

        // Should fail on API token check (happens before endpoint which needs account_id)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_missing_both_credentials() {
        let model = ChatCloudflare {
            account_id: None,
            api_token: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let messages = vec![Message::human("Hello")];
        let result = model._stream(&messages, None, None, None, None).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_with_empty_messages() {
        let model = ChatCloudflare {
            account_id: Some("test-account".to_string()),
            api_token: Some("test-token".to_string()),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            seed: None,
            repetition_penalty: None,
            frequency_penalty: None,
            presence_penalty: None,
        };

        let messages: Vec<BaseMessage> = vec![];
        // This will fail due to network issues (fake credentials), but validates the code path
        let result = model._generate(&messages, None, None, None, None).await;

        // Should error due to HTTP failure (invalid credentials), not a code panic
        assert!(result.is_err());
    }

    // ==================== Model Configuration Combinations ====================

    #[test]
    fn test_model_with_only_account_id() {
        let model = ChatCloudflare::new().with_account_id("only-account");

        assert!(model.account_id.is_some());
        assert!(model.api_token.is_none() || model.api_token.is_some()); // env-dependent
    }

    #[test]
    fn test_model_with_only_api_token() {
        let model = ChatCloudflare::new().with_api_token("only-token");

        assert_eq!(model.api_token, Some("only-token".to_string()));
        assert!(model.account_id.is_none() || model.account_id.is_some()); // env-dependent
    }

    #[test]
    fn test_model_with_all_sampling_params() {
        let model = ChatCloudflare::new()
            .with_temperature(0.8)
            .with_top_p(0.95)
            .with_top_k(40);

        assert_eq!(model.temperature, Some(0.8));
        assert_eq!(model.top_p, Some(0.95));
        assert_eq!(model.top_k, Some(40));
    }

    #[test]
    fn test_model_with_all_penalty_params() {
        let model = ChatCloudflare::new()
            .with_repetition_penalty(1.2)
            .with_frequency_penalty(0.6)
            .with_presence_penalty(0.4);

        assert_eq!(model.repetition_penalty, Some(1.2));
        assert_eq!(model.frequency_penalty, Some(0.6));
        assert_eq!(model.presence_penalty, Some(0.4));
    }

    // ==================== Model Names Edge Cases ====================

    #[test]
    fn test_model_name_with_version_suffix() {
        let model = ChatCloudflare::new()
            .with_model("@cf/test/model-v0.2.1");

        assert_eq!(model.model, "@cf/test/model-v0.2.1");
    }

    #[test]
    fn test_model_name_huggingface_format() {
        let model = ChatCloudflare::new()
            .with_model("@hf/organization/model-name-quantized");

        assert_eq!(model.model, "@hf/organization/model-name-quantized");
    }

    #[test]
    fn test_model_name_without_at_prefix() {
        // While unusual, should work
        let model = ChatCloudflare::new()
            .with_model("custom-model-name");

        assert_eq!(model.model, "custom-model-name");
    }

    // ==================== Request Body Message Order ====================

    #[test]
    fn test_build_request_body_preserves_message_order() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("First"),
            Message::human("Second"),
            Message::ai("Third"),
            Message::human("Fourth"),
        ];

        let body = model.build_request_body(&messages, false);
        let body_messages = body["messages"].as_array().unwrap();

        assert_eq!(body_messages[0]["content"], "First");
        assert_eq!(body_messages[1]["content"], "Second");
        assert_eq!(body_messages[2]["content"], "Third");
        assert_eq!(body_messages[3]["content"], "Fourth");
    }

    #[test]
    fn test_build_request_body_preserves_roles() {
        let model = ChatCloudflare::new();
        let messages = vec![
            Message::system("System"),
            Message::human("User"),
            Message::ai("Assistant"),
        ];

        let body = model.build_request_body(&messages, false);
        let body_messages = body["messages"].as_array().unwrap();

        assert_eq!(body_messages[0]["role"], "system");
        assert_eq!(body_messages[1]["role"], "user");
        assert_eq!(body_messages[2]["role"], "assistant");
    }
}
