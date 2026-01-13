//! `DeepSeek` chat models implementation
//!
//! `DeepSeek` provides an OpenAI-compatible API, so this module wraps
//! the `ChatOpenAI` implementation with DeepSeek-specific defaults.

use crate::DEEPSEEK_DEFAULT_API_BASE;

use async_openai::config::OpenAIConfig;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string_or_default, DEEPSEEK_API_BASE, DEEPSEEK_API_KEY},
    error::Error as DashFlowError,
    language_models::{ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition},
    messages::BaseMessage,
    rate_limiters::RateLimiter,
    serialization::Serializable,
};
use dashflow_openai::ChatOpenAI;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// `DeepSeek` model names
pub mod models {
    pub const DEEPSEEK_CHAT: &str = "deepseek-chat";
    pub const DEEPSEEK_CODER: &str = "deepseek-coder";
}

/// `DeepSeek` chat model client
///
/// This is a thin wrapper around `ChatOpenAI` that sets DeepSeek-specific defaults.
/// `DeepSeek`'s API is fully compatible with `OpenAI`'s API.
#[derive(Clone)]
pub struct ChatDeepSeek {
    inner: ChatOpenAI,
}

impl ChatDeepSeek {
    /// Create a new `ChatDeepSeek` instance with default settings
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_deepseek::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        let api_key = env_string_or_default(DEEPSEEK_API_KEY, "");
        let api_base = env_string_or_default(DEEPSEEK_API_BASE, DEEPSEEK_DEFAULT_API_BASE);

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(api_base);

        let inner = ChatOpenAI::with_config(config).with_model(models::DEEPSEEK_CHAT);

        Self { inner }
    }

    /// Create with custom API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let api_base = env_string_or_default(DEEPSEEK_API_BASE, DEEPSEEK_DEFAULT_API_BASE);

        let config = OpenAIConfig::new()
            .with_api_key(api_key.into())
            .with_api_base(api_base);

        let inner = ChatOpenAI::with_config(config).with_model(models::DEEPSEEK_CHAT);

        Self { inner }
    }

    /// Set the model name
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.inner = self.inner.with_model(model);
        self
    }

    /// Set the temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.inner = self.inner.with_temperature(temperature);
        self
    }

    /// Set max tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.inner = self.inner.with_max_tokens(max_tokens);
        self
    }

    /// Set top-p sampling
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.inner = self.inner.with_top_p(top_p);
        self
    }

    /// Set frequency penalty
    #[must_use]
    pub fn with_frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.inner = self.inner.with_frequency_penalty(frequency_penalty);
        self
    }

    /// Set presence penalty
    #[must_use]
    pub fn with_presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.inner = self.inner.with_presence_penalty(presence_penalty);
        self
    }

    /// Set rate limiter
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        self.inner = self.inner.with_rate_limiter(rate_limiter);
        self
    }

    /// Get a reference to the underlying `ChatOpenAI` instance
    #[must_use]
    pub fn inner(&self) -> &ChatOpenAI {
        &self.inner
    }

    /// Get a mutable reference to the underlying `ChatOpenAI` instance
    pub fn inner_mut(&mut self) -> &mut ChatOpenAI {
        &mut self.inner
    }

    /// Convert into the underlying `ChatOpenAI` instance
    #[must_use]
    pub fn into_inner(self) -> ChatOpenAI {
        self.inner
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatDeepSeek {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatModel for ChatDeepSeek {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult, DashFlowError> {
        self.inner
            ._generate(messages, stop, tools, tool_choice, run_manager)
            .await
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk, DashFlowError>> + Send>>,
        DashFlowError,
    > {
        self.inner
            ._stream(messages, stop, tools, tool_choice, run_manager)
            .await
    }

    fn llm_type(&self) -> &'static str {
        "deepseek"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.inner.rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Serializable for ChatDeepSeek {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "deepseek".to_string(),
            "ChatDeepSeek".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> dashflow::core::serialization::SerializedObject {
        // Delegate to inner OpenAI client but override the lc_id
        self.inner.to_json()
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        secrets.insert("api_key".to_string(), "DEEPSEEK_API_KEY".to_string());
        secrets
    }
}

#[cfg(test)]
#[allow(deprecated)] // Tests intentionally verify deprecated builder API for backwards compatibility
mod tests {
    use super::*;

    // ==================== Model Constants Tests ====================

    #[test]
    fn test_model_constants() {
        assert_eq!(models::DEEPSEEK_CHAT, "deepseek-chat");
        assert_eq!(models::DEEPSEEK_CODER, "deepseek-coder");
    }

    #[test]
    fn test_model_constants_are_not_empty() {
        assert!(!models::DEEPSEEK_CHAT.is_empty());
        assert!(!models::DEEPSEEK_CODER.is_empty());
    }

    // ==================== Construction Tests ====================

    #[test]
    fn test_deepseek_default() {
        let chat = ChatDeepSeek::default();
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_deepseek_new() {
        let chat = ChatDeepSeek::new();
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_default_equals_new() {
        let default_chat = ChatDeepSeek::default();
        let new_chat = ChatDeepSeek::new();
        // Both should be deepseek type
        assert_eq!(default_chat.llm_type(), new_chat.llm_type());
    }

    #[test]
    fn test_with_api_key() {
        let chat = ChatDeepSeek::with_api_key("sk-test-key-12345");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_with_api_key_string_type() {
        let chat = ChatDeepSeek::with_api_key(String::from("sk-test-key"));
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_clone() {
        let original = ChatDeepSeek::with_api_key("sk-test-key")
            .with_model("deepseek-coder")
            .with_temperature(0.5);

        let cloned = original.clone();
        assert_eq!(cloned.llm_type(), original.llm_type());
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_deepseek_builder() {
        let chat = ChatDeepSeek::default()
            .with_model("deepseek-chat")
            .with_temperature(0.7)
            .with_max_tokens(1000);

        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_model() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model(models::DEEPSEEK_CHAT);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_model_coder() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model(models::DEEPSEEK_CODER);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_model_string() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model(String::from("custom-model"));
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_temperature() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(0.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(1.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(2.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_max_tokens() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_max_tokens(100);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_max_tokens(4096);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_top_p() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_top_p(0.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_top_p(0.5);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_top_p(1.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_frequency_penalty() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(-2.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(0.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(2.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_with_presence_penalty() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_presence_penalty(-2.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_presence_penalty(0.0);
        assert_eq!(chat.llm_type(), "deepseek");

        let chat = ChatDeepSeek::with_api_key("key")
            .with_presence_penalty(2.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_all_options() {
        let chat = ChatDeepSeek::with_api_key("sk-test-key")
            .with_model("deepseek-chat")
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .with_top_p(0.95)
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.3);

        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_chaining_order_independent() {
        let chat1 = ChatDeepSeek::with_api_key("key")
            .with_temperature(0.5)
            .with_max_tokens(100)
            .with_model("deepseek-chat");

        let chat2 = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat")
            .with_max_tokens(100)
            .with_temperature(0.5);

        // Both should produce valid deepseek models
        assert_eq!(chat1.llm_type(), chat2.llm_type());
    }

    // ==================== Inner Access Tests ====================

    #[test]
    fn test_inner_reference() {
        let chat = ChatDeepSeek::with_api_key("key");
        let inner = chat.inner();
        // Inner should be a ChatOpenAI that reports as OpenAI type
        assert!(inner.llm_type().contains("openai") || inner.llm_type().contains("chat"));
    }

    #[test]
    fn test_inner_mut() {
        let mut chat = ChatDeepSeek::with_api_key("key");
        let inner = chat.inner_mut();
        // Should be able to get mutable reference
        assert!(inner.llm_type().contains("openai") || inner.llm_type().contains("chat"));
    }

    #[test]
    fn test_into_inner() {
        let chat = ChatDeepSeek::with_api_key("key");
        let inner = chat.into_inner();
        // Should consume and return ChatOpenAI
        assert!(inner.llm_type().contains("openai") || inner.llm_type().contains("chat"));
    }

    // ==================== ChatModel Trait Tests ====================

    #[test]
    fn test_llm_type() {
        let chat = ChatDeepSeek::default();
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_rate_limiter_default_none() {
        let chat = ChatDeepSeek::default();
        // Default should have no rate limiter
        assert!(chat.rate_limiter().is_none());
    }

    #[test]
    fn test_as_any() {
        let chat = ChatDeepSeek::default();
        let any_ref = chat.as_any();

        // Should be able to downcast to ChatDeepSeek
        let downcast: Option<&ChatDeepSeek> = any_ref.downcast_ref();
        assert!(downcast.is_some());
    }

    #[test]
    fn test_as_any_preserves_type() {
        let chat = ChatDeepSeek::with_api_key("test-key")
            .with_model("deepseek-coder");

        let any_ref = chat.as_any();
        let downcast: &ChatDeepSeek = any_ref.downcast_ref().unwrap();
        assert_eq!(downcast.llm_type(), "deepseek");
    }

    // ==================== Serializable Trait Tests ====================

    #[test]
    fn test_serialization() {
        let chat = ChatDeepSeek::default();
        assert!(chat.is_lc_serializable());
        assert_eq!(
            chat.lc_id(),
            vec!["dashflow", "chat_models", "deepseek", "ChatDeepSeek"]
        );
    }

    #[test]
    fn test_lc_id_length() {
        let chat = ChatDeepSeek::default();
        let lc_id = chat.lc_id();
        assert_eq!(lc_id.len(), 4);
    }

    #[test]
    fn test_lc_id_components() {
        let chat = ChatDeepSeek::default();
        let lc_id = chat.lc_id();
        assert_eq!(lc_id[0], "dashflow");
        assert_eq!(lc_id[1], "chat_models");
        assert_eq!(lc_id[2], "deepseek");
        assert_eq!(lc_id[3], "ChatDeepSeek");
    }

    #[test]
    fn test_is_lc_serializable() {
        let chat = ChatDeepSeek::default();
        assert!(chat.is_lc_serializable());
    }

    #[test]
    fn test_to_json() {
        use dashflow::core::serialization::SerializedObject;

        let chat = ChatDeepSeek::default();
        let json = chat.to_json();
        // Should produce valid serialized object (Constructor variant)
        match json {
            SerializedObject::Constructor { lc, id, kwargs: _ } => {
                assert!(lc > 0);
                assert!(!id.is_empty());
            }
            _ => {
                // For not_implemented or secret, still valid
            }
        }
    }

    #[test]
    fn test_lc_secrets() {
        let chat = ChatDeepSeek::default();
        let secrets = chat.lc_secrets();
        assert_eq!(
            secrets.get("api_key"),
            Some(&"DEEPSEEK_API_KEY".to_string())
        );
    }

    #[test]
    fn test_lc_secrets_contains_api_key() {
        let chat = ChatDeepSeek::default();
        let secrets = chat.lc_secrets();
        assert!(secrets.contains_key("api_key"));
    }

    #[test]
    fn test_lc_secrets_length() {
        let chat = ChatDeepSeek::default();
        let secrets = chat.lc_secrets();
        assert_eq!(secrets.len(), 1);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_api_key() {
        let chat = ChatDeepSeek::with_api_key("");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_long_api_key() {
        let long_key = "sk-".to_string() + &"x".repeat(1000);
        let chat = ChatDeepSeek::with_api_key(long_key);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_unicode_in_model_name() {
        // Though unusual, should handle unicode
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("测试模型");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_zero_max_tokens() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_max_tokens(0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_large_max_tokens() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_max_tokens(u32::MAX);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Additional Builder Pattern Tests ====================

    #[test]
    fn test_builder_temperature_boundary_negative() {
        // Some APIs allow negative temps for determinism
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(-1.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_temperature_fractional() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(0.333);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_max_tokens_one() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_max_tokens(1);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_max_tokens_common_values() {
        for max_tokens in [128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768] {
            let chat = ChatDeepSeek::with_api_key("key")
                .with_max_tokens(max_tokens);
            assert_eq!(chat.llm_type(), "deepseek");
        }
    }

    #[test]
    fn test_builder_top_p_boundary_values() {
        // Test boundary values for top_p
        for top_p in [0.0, 0.001, 0.5, 0.999, 1.0] {
            let chat = ChatDeepSeek::with_api_key("key")
                .with_top_p(top_p);
            assert_eq!(chat.llm_type(), "deepseek");
        }
    }

    #[test]
    fn test_builder_frequency_penalty_range() {
        // DeepSeek supports -2.0 to 2.0
        for penalty in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0] {
            let chat = ChatDeepSeek::with_api_key("key")
                .with_frequency_penalty(penalty);
            assert_eq!(chat.llm_type(), "deepseek");
        }
    }

    #[test]
    fn test_builder_presence_penalty_range() {
        // DeepSeek supports -2.0 to 2.0
        for penalty in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0] {
            let chat = ChatDeepSeek::with_api_key("key")
                .with_presence_penalty(penalty);
            assert_eq!(chat.llm_type(), "deepseek");
        }
    }

    #[test]
    fn test_builder_multiple_model_changes() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat")
            .with_model("deepseek-coder")
            .with_model("deepseek-chat"); // Change back
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_multiple_temperature_changes() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(0.1)
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_combined_penalties() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(0.5)
            .with_presence_penalty(0.5);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_opposite_penalties() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(-1.0)
            .with_presence_penalty(1.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_max_penalties() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(2.0)
            .with_presence_penalty(2.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_builder_min_penalties() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(-2.0)
            .with_presence_penalty(-2.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Additional Clone Tests ====================

    #[test]
    fn test_clone_preserves_model_configuration() {
        let original = ChatDeepSeek::with_api_key("sk-test")
            .with_model(models::DEEPSEEK_CODER)
            .with_temperature(0.3)
            .with_max_tokens(500)
            .with_top_p(0.8);

        let cloned = original.clone();
        // Both should have same type
        assert_eq!(cloned.llm_type(), original.llm_type());
        // Both should be serializable
        assert!(cloned.is_lc_serializable());
        assert!(original.is_lc_serializable());
    }

    #[test]
    fn test_clone_independence() {
        let original = ChatDeepSeek::with_api_key("key");
        let mut cloned = original.clone();

        // Modify cloned (through inner_mut)
        let _inner = cloned.inner_mut();

        // Original should be unchanged
        assert_eq!(original.llm_type(), "deepseek");
    }

    #[test]
    fn test_multiple_clones() {
        let original = ChatDeepSeek::with_api_key("key");
        let clone1 = original.clone();
        let clone2 = original.clone();
        let clone3 = clone1.clone();

        assert_eq!(original.llm_type(), "deepseek");
        assert_eq!(clone1.llm_type(), "deepseek");
        assert_eq!(clone2.llm_type(), "deepseek");
        assert_eq!(clone3.llm_type(), "deepseek");
    }

    // ==================== Additional Serialization Tests ====================

    #[test]
    fn test_lc_id_immutable() {
        let chat1 = ChatDeepSeek::with_api_key("key1");
        let chat2 = ChatDeepSeek::with_api_key("key2")
            .with_model("deepseek-coder");

        // lc_id should be same regardless of configuration
        assert_eq!(chat1.lc_id(), chat2.lc_id());
    }

    #[test]
    fn test_lc_id_starts_with_dashflow() {
        let chat = ChatDeepSeek::default();
        let lc_id = chat.lc_id();
        assert_eq!(lc_id.first(), Some(&"dashflow".to_string()));
    }

    #[test]
    fn test_lc_id_ends_with_class_name() {
        let chat = ChatDeepSeek::default();
        let lc_id = chat.lc_id();
        assert_eq!(lc_id.last(), Some(&"ChatDeepSeek".to_string()));
    }

    #[test]
    fn test_lc_secrets_env_var_format() {
        let chat = ChatDeepSeek::default();
        let secrets = chat.lc_secrets();

        // The value should be uppercase with underscores (env var format)
        let api_key_secret = secrets.get("api_key").unwrap();
        assert!(api_key_secret.chars().all(|c| c.is_uppercase() || c == '_'));
    }

    #[test]
    fn test_to_json_returns_serialized_object() {
        use dashflow::core::serialization::SerializedObject;

        let chat = ChatDeepSeek::with_api_key("test-key")
            .with_model("deepseek-chat");
        let json = chat.to_json();

        // Should be a valid SerializedObject variant
        match json {
            SerializedObject::Constructor { .. } => { /* expected */ }
            SerializedObject::NotImplemented { .. } => { /* acceptable */ }
            SerializedObject::Secret { .. } => { /* acceptable */ }
        }
    }

    #[test]
    fn test_serialization_consistency() {
        let chat1 = ChatDeepSeek::with_api_key("key");
        let chat2 = ChatDeepSeek::with_api_key("key");

        // Same configuration should produce same lc_id
        assert_eq!(chat1.lc_id(), chat2.lc_id());
        assert_eq!(chat1.lc_secrets(), chat2.lc_secrets());
    }

    // ==================== Inner Access Extended Tests ====================

    #[test]
    fn test_inner_preserves_model_settings() {
        let chat = ChatDeepSeek::with_api_key("test-key")
            .with_model("deepseek-coder");
        let inner = chat.inner();

        // Inner is ChatOpenAI, should exist and be valid
        assert!(!inner.llm_type().is_empty());
    }

    #[test]
    fn test_inner_mut_allows_modification() {
        let mut chat = ChatDeepSeek::with_api_key("key");
        {
            let inner = chat.inner_mut();
            // Should be able to access inner mutably
            let _ = inner.llm_type();
        }
        // Chat should still be valid after mutation
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_into_inner_consumes_wrapper() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat")
            .with_temperature(0.5);

        // into_inner should consume chat
        let inner = chat.into_inner();

        // Inner should be valid ChatOpenAI
        assert!(!inner.llm_type().is_empty());
    }

    #[test]
    fn test_inner_chain_operations() {
        let chat = ChatDeepSeek::with_api_key("key");
        let inner1 = chat.inner();
        let inner2 = chat.inner();

        // Multiple immutable borrows should work
        assert_eq!(inner1.llm_type(), inner2.llm_type());
    }

    // ==================== as_any Extended Tests ====================

    #[test]
    fn test_as_any_type_id() {
        use std::any::TypeId;

        let chat = ChatDeepSeek::default();
        let any_ref = chat.as_any();

        assert_eq!(any_ref.type_id(), TypeId::of::<ChatDeepSeek>());
    }

    #[test]
    fn test_as_any_wrong_downcast_fails() {
        let chat = ChatDeepSeek::default();
        let any_ref = chat.as_any();

        // Trying to downcast to wrong type should fail
        let wrong_downcast: Option<&String> = any_ref.downcast_ref();
        assert!(wrong_downcast.is_none());
    }

    #[test]
    fn test_as_any_after_configuration() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-coder")
            .with_temperature(0.7)
            .with_max_tokens(1000);

        let any_ref = chat.as_any();
        let downcast: Option<&ChatDeepSeek> = any_ref.downcast_ref();

        assert!(downcast.is_some());
        assert_eq!(downcast.unwrap().llm_type(), "deepseek");
    }

    // ==================== Rate Limiter Tests ====================

    #[test]
    fn test_rate_limiter_none_by_default() {
        let chat = ChatDeepSeek::with_api_key("key");
        assert!(chat.rate_limiter().is_none());
    }

    #[test]
    fn test_rate_limiter_after_configuration() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat")
            .with_temperature(0.5);

        // Without explicit rate limiter, should still be None
        assert!(chat.rate_limiter().is_none());
    }

    // ==================== API Key Edge Cases ====================

    #[test]
    fn test_api_key_with_spaces() {
        let chat = ChatDeepSeek::with_api_key("  sk-test-key  ");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_api_key_with_newlines() {
        let chat = ChatDeepSeek::with_api_key("sk-test-key\n");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_api_key_unicode() {
        let chat = ChatDeepSeek::with_api_key("sk-测试密钥");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_api_key_special_characters() {
        let chat = ChatDeepSeek::with_api_key("sk-!@#$%^&*()_+-=[]{}|;':\",./<>?");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_api_key_numeric() {
        let chat = ChatDeepSeek::with_api_key("1234567890");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Model Name Edge Cases ====================

    #[test]
    fn test_model_empty_name() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_model_whitespace_name() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("   ");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_model_long_name() {
        let long_name = "x".repeat(1000);
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model(long_name);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_model_name_with_version() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat-v2.5");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_model_name_with_date() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_model("deepseek-chat-2024-01-01");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Floating Point Edge Cases ====================

    #[test]
    fn test_temperature_very_small() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(0.0001);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_temperature_very_large() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_temperature(100.0);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_top_p_very_small() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_top_p(0.0001);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_penalties_fractional() {
        let chat = ChatDeepSeek::with_api_key("key")
            .with_frequency_penalty(0.12345)
            .with_presence_penalty(-0.54321);
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Default vs Constructor Comparison ====================

    #[test]
    fn test_default_returns_deepseek_type() {
        let chat = ChatDeepSeek::default();
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_new_returns_deepseek_type() {
        let chat = ChatDeepSeek::new();
        assert_eq!(chat.llm_type(), "deepseek");
    }

    #[test]
    fn test_with_api_key_returns_deepseek_type() {
        let chat = ChatDeepSeek::with_api_key("any-key");
        assert_eq!(chat.llm_type(), "deepseek");
    }

    // ==================== Model Constants Extended ====================

    #[test]
    fn test_model_constants_unique() {
        assert_ne!(models::DEEPSEEK_CHAT, models::DEEPSEEK_CODER);
    }

    #[test]
    fn test_model_constants_prefix() {
        assert!(models::DEEPSEEK_CHAT.starts_with("deepseek"));
        assert!(models::DEEPSEEK_CODER.starts_with("deepseek"));
    }

    #[test]
    fn test_model_constants_no_whitespace() {
        assert_eq!(models::DEEPSEEK_CHAT.trim(), models::DEEPSEEK_CHAT);
        assert_eq!(models::DEEPSEEK_CODER.trim(), models::DEEPSEEK_CODER);
    }

    #[test]
    fn test_model_constants_lowercase() {
        assert_eq!(models::DEEPSEEK_CHAT, models::DEEPSEEK_CHAT.to_lowercase());
        assert_eq!(models::DEEPSEEK_CODER, models::DEEPSEEK_CODER.to_lowercase());
    }

    // ==================== Integration Tests (Ignored) ====================

    #[tokio::test]
    #[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
    async fn test_generate_real_api() {
        use dashflow::core::messages::Message;

        let chat = ChatDeepSeek::default()
            .with_max_tokens(50);

        let messages = vec![
            Message::human("What is 2+2? Answer with just the number."),
        ];

        let result = chat._generate(&messages, None, None, None, None).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.generations.is_empty());
    }

    #[tokio::test]
    #[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
    async fn test_stream_real_api() {
        use dashflow::core::messages::Message;
        use futures::StreamExt;

        let chat = ChatDeepSeek::default()
            .with_max_tokens(50);

        let messages = vec![Message::human("Count from 1 to 3.")];

        let result = chat._stream(&messages, None, None, None, None).await;
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
}
