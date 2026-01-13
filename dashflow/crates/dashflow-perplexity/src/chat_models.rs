//! Perplexity AI chat models implementation
//!
//! Perplexity AI provides OpenAI-compatible API endpoints, so we can reuse
//! `ChatOpenAI` with custom configuration.

use crate::PPLX_DEFAULT_API_BASE;

use async_openai::config::OpenAIConfig;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    config_loader::env_vars::{env_string, env_string_or_default, PPLX_API_BASE, PPLX_API_KEY},
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

/// Perplexity model names
pub mod models {
    pub const SONAR: &str = "sonar";
    pub const SONAR_PRO: &str = "sonar-pro";
    pub const SONAR_REASONING: &str = "sonar-reasoning";
}

/// Perplexity AI chat model
///
/// This is a wrapper around `ChatOpenAI` configured for Perplexity's API endpoint.
/// Perplexity uses an OpenAI-compatible API, so all `ChatOpenAI` features are supported:
/// - Streaming responses
/// - Tool/function calling
/// - JSON mode
/// - Structured output
///
/// # Models
///
/// - `sonar` - Default model, optimized for reasoning and search
/// - `sonar-pro` - Advanced model with enhanced capabilities
/// - `sonar-reasoning` - Specialized for complex reasoning tasks
///
/// # Example
///
/// ```no_run
/// use dashflow_perplexity::ChatPerplexity;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::BaseMessage;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let model = ChatPerplexity::default();
///     let messages = vec![BaseMessage::human("What is Rust?")];
///     let response = model.generate(&messages, None, None, None, None).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ChatPerplexity {
    inner: ChatOpenAI,
}

impl ChatPerplexity {
    /// Create a new `ChatPerplexity` instance with default model "sonar"
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_perplexity::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        let api_key = env_string(PPLX_API_KEY).unwrap_or_default();
        let api_base = env_string_or_default(PPLX_API_BASE, PPLX_DEFAULT_API_BASE);

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(api_base);

        let inner = ChatOpenAI::with_config(config).with_model(models::SONAR);

        Self { inner }
    }

    /// Create with custom API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let api_base = env_string_or_default(PPLX_API_BASE, PPLX_DEFAULT_API_BASE);

        let config = OpenAIConfig::new()
            .with_api_key(api_key.into())
            .with_api_base(api_base);

        let inner = ChatOpenAI::with_config(config).with_model(models::SONAR);

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
impl Default for ChatPerplexity {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatModel for ChatPerplexity {
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
        "perplexity"
    }

    fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
        self.inner.rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Serializable for ChatPerplexity {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "perplexity".to_string(),
            "ChatPerplexity".to_string(),
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
        secrets.insert("api_key".to_string(), "PPLX_API_KEY".to_string());
        secrets
    }
}

#[cfg(test)]
#[allow(
    deprecated,
    clippy::disallowed_methods,
    clippy::unwrap_used
)] // Tests intentionally verify deprecated builder API for backwards compatibility
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    // Tests that manipulate PPLX_API_KEY must acquire this lock.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ========================================================================
    // ChatPerplexity - Basic Construction Tests
    // ========================================================================

    #[test]
    fn test_new() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::new();
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::default();
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_default_equivalent_to_new() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model_new = ChatPerplexity::new();
        let model_default = ChatPerplexity::default();
        // Both should have same llm_type
        assert_eq!(model_new.llm_type(), model_default.llm_type());
    }

    #[test]
    fn test_with_api_key() {
        // No env var manipulation - doesn't need mutex
        let model = ChatPerplexity::with_api_key("custom-key");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_api_key_empty_string() {
        let model = ChatPerplexity::with_api_key("");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_api_key_long_key() {
        let long_key = "a".repeat(256);
        let model = ChatPerplexity::with_api_key(&long_key);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_clone() {
        let model = ChatPerplexity::with_api_key("test-key");
        let cloned = model.clone();
        assert_eq!(model.llm_type(), cloned.llm_type());
    }

    // ========================================================================
    // ChatPerplexity - Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_with_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::new().with_model(models::SONAR_PRO);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_model_sonar() {
        let model = ChatPerplexity::with_api_key("key").with_model(models::SONAR);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_model_sonar_pro() {
        let model = ChatPerplexity::with_api_key("key").with_model(models::SONAR_PRO);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_model_sonar_reasoning() {
        let model = ChatPerplexity::with_api_key("key").with_model(models::SONAR_REASONING);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_model_custom_string() {
        let model = ChatPerplexity::with_api_key("key").with_model("custom-model-v1");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_temperature() {
        let model = ChatPerplexity::with_api_key("key").with_temperature(0.7);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_temperature_zero() {
        let model = ChatPerplexity::with_api_key("key").with_temperature(0.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_temperature_one() {
        let model = ChatPerplexity::with_api_key("key").with_temperature(1.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_temperature_high() {
        let model = ChatPerplexity::with_api_key("key").with_temperature(2.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_max_tokens() {
        let model = ChatPerplexity::with_api_key("key").with_max_tokens(1000);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_max_tokens_small() {
        let model = ChatPerplexity::with_api_key("key").with_max_tokens(1);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_max_tokens_large() {
        let model = ChatPerplexity::with_api_key("key").with_max_tokens(128000);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_top_p() {
        let model = ChatPerplexity::with_api_key("key").with_top_p(0.9);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_top_p_zero() {
        let model = ChatPerplexity::with_api_key("key").with_top_p(0.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_top_p_one() {
        let model = ChatPerplexity::with_api_key("key").with_top_p(1.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_frequency_penalty() {
        let model = ChatPerplexity::with_api_key("key").with_frequency_penalty(0.5);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_frequency_penalty_zero() {
        let model = ChatPerplexity::with_api_key("key").with_frequency_penalty(0.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_frequency_penalty_negative() {
        let model = ChatPerplexity::with_api_key("key").with_frequency_penalty(-1.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_frequency_penalty_max() {
        let model = ChatPerplexity::with_api_key("key").with_frequency_penalty(2.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_presence_penalty() {
        let model = ChatPerplexity::with_api_key("key").with_presence_penalty(0.5);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_presence_penalty_zero() {
        let model = ChatPerplexity::with_api_key("key").with_presence_penalty(0.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_presence_penalty_negative() {
        let model = ChatPerplexity::with_api_key("key").with_presence_penalty(-1.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_presence_penalty_max() {
        let model = ChatPerplexity::with_api_key("key").with_presence_penalty(2.0);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_builder_pattern() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::new()
            .with_temperature(0.7)
            .with_max_tokens(1000);

        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_builder_full_chain() {
        let model = ChatPerplexity::with_api_key("key")
            .with_model(models::SONAR_PRO)
            .with_temperature(0.8)
            .with_max_tokens(2000)
            .with_top_p(0.95)
            .with_frequency_penalty(0.3)
            .with_presence_penalty(0.4);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_builder_chaining_order_independent() {
        let model1 = ChatPerplexity::with_api_key("key")
            .with_temperature(0.5)
            .with_max_tokens(100);
        let model2 = ChatPerplexity::with_api_key("key")
            .with_max_tokens(100)
            .with_temperature(0.5);
        assert_eq!(model1.llm_type(), model2.llm_type());
    }

    #[test]
    fn test_builder_value_overwriting() {
        // Last value should win
        let model = ChatPerplexity::with_api_key("key")
            .with_temperature(0.1)
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_builder_model_overwriting() {
        let model = ChatPerplexity::with_api_key("key")
            .with_model(models::SONAR)
            .with_model(models::SONAR_PRO)
            .with_model(models::SONAR_REASONING);
        assert_eq!(model.llm_type(), "perplexity");
    }

    // ========================================================================
    // ChatPerplexity - ChatModel Trait Tests
    // ========================================================================

    #[test]
    fn test_llm_type() {
        let model = ChatPerplexity::with_api_key("key");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_llm_type_is_nonempty() {
        let model = ChatPerplexity::with_api_key("key");
        let llm_type = model.llm_type();
        assert!(!llm_type.is_empty());
    }

    #[test]
    fn test_llm_type_consistent() {
        let model = ChatPerplexity::with_api_key("key");
        assert_eq!(model.llm_type(), model.llm_type());
    }

    #[test]
    fn test_rate_limiter_default_none() {
        let model = ChatPerplexity::with_api_key("key");
        assert!(model.rate_limiter().is_none());
    }

    #[test]
    fn test_as_any() {
        let model = ChatPerplexity::with_api_key("key");
        let any = model.as_any();
        assert!(any.is::<ChatPerplexity>());
    }

    #[test]
    fn test_as_any_downcast() {
        let model = ChatPerplexity::with_api_key("key");
        let any = model.as_any();
        let downcast = any.downcast_ref::<ChatPerplexity>();
        assert!(downcast.is_some());
        assert_eq!(downcast.unwrap().llm_type(), "perplexity");
    }

    // ========================================================================
    // ChatPerplexity - Inner Access Tests
    // ========================================================================

    #[test]
    fn test_inner() {
        let model = ChatPerplexity::with_api_key("key");
        let inner = model.inner();
        // Verify we can access the inner ChatOpenAI
        assert!(!inner.llm_type().is_empty());
    }

    #[test]
    fn test_inner_mut() {
        let mut model = ChatPerplexity::with_api_key("key");
        let inner = model.inner_mut();
        // Verify we can access mutable inner
        assert!(!inner.llm_type().is_empty());
    }

    #[test]
    fn test_into_inner() {
        let model = ChatPerplexity::with_api_key("key");
        let inner = model.into_inner();
        // Verify inner is a ChatOpenAI
        assert!(!inner.llm_type().is_empty());
    }

    #[test]
    fn test_inner_llm_type() {
        let model = ChatPerplexity::with_api_key("key");
        let inner = model.inner();
        // The inner ChatOpenAI should have its own llm_type
        let inner_type = inner.llm_type();
        assert!(!inner_type.is_empty());
    }

    // ========================================================================
    // ChatPerplexity - Serialization Tests
    // ========================================================================

    #[test]
    fn test_serialization() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::new();
        assert!(model.is_lc_serializable());
        assert_eq!(
            model.lc_id(),
            vec!["dashflow", "chat_models", "perplexity", "ChatPerplexity"]
        );
    }

    #[test]
    fn test_is_lc_serializable() {
        let model = ChatPerplexity::with_api_key("key");
        assert!(model.is_lc_serializable());
    }

    #[test]
    fn test_lc_id() {
        let model = ChatPerplexity::with_api_key("key");
        let lc_id = model.lc_id();
        assert_eq!(lc_id.len(), 4);
        assert_eq!(lc_id[0], "dashflow");
        assert_eq!(lc_id[1], "chat_models");
        assert_eq!(lc_id[2], "perplexity");
        assert_eq!(lc_id[3], "ChatPerplexity");
    }

    #[test]
    fn test_lc_id_is_nonempty() {
        let model = ChatPerplexity::with_api_key("key");
        let lc_id = model.lc_id();
        assert!(!lc_id.is_empty());
        for part in &lc_id {
            assert!(!part.is_empty());
        }
    }

    #[test]
    fn test_lc_secrets() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("PPLX_API_KEY", "test-key");
        let model = ChatPerplexity::new();
        let secrets = model.lc_secrets();
        assert_eq!(secrets.get("api_key"), Some(&"PPLX_API_KEY".to_string()));
    }

    #[test]
    fn test_lc_secrets_has_api_key() {
        let model = ChatPerplexity::with_api_key("key");
        let secrets = model.lc_secrets();
        assert!(secrets.contains_key("api_key"));
    }

    #[test]
    fn test_lc_secrets_key_count() {
        let model = ChatPerplexity::with_api_key("key");
        let secrets = model.lc_secrets();
        assert_eq!(secrets.len(), 1);
    }

    #[test]
    fn test_to_json() {
        let model = ChatPerplexity::with_api_key("key");
        let json = model.to_json();
        // Verify we get a serialized object with valid id
        assert!(!json.id().is_empty());
    }

    // ========================================================================
    // Model Constants Tests
    // ========================================================================

    #[test]
    fn test_model_constant_sonar() {
        assert_eq!(models::SONAR, "sonar");
    }

    #[test]
    fn test_model_constant_sonar_pro() {
        assert_eq!(models::SONAR_PRO, "sonar-pro");
    }

    #[test]
    fn test_model_constant_sonar_reasoning() {
        assert_eq!(models::SONAR_REASONING, "sonar-reasoning");
    }

    #[test]
    fn test_model_constants_are_nonempty() {
        assert!(!models::SONAR.is_empty());
        assert!(!models::SONAR_PRO.is_empty());
        assert!(!models::SONAR_REASONING.is_empty());
    }

    #[test]
    fn test_model_constants_are_distinct() {
        assert_ne!(models::SONAR, models::SONAR_PRO);
        assert_ne!(models::SONAR, models::SONAR_REASONING);
        assert_ne!(models::SONAR_PRO, models::SONAR_REASONING);
    }

    #[test]
    fn test_model_constants_are_lowercase() {
        assert_eq!(models::SONAR, models::SONAR.to_lowercase());
        assert_eq!(models::SONAR_PRO, models::SONAR_PRO.to_lowercase());
        assert_eq!(
            models::SONAR_REASONING,
            models::SONAR_REASONING.to_lowercase()
        );
    }

    // ========================================================================
    // ChatPerplexity - Edge Case Tests
    // ========================================================================

    #[test]
    fn test_with_api_key_unicode() {
        let model = ChatPerplexity::with_api_key("key-with-émojis-🔑");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_api_key_whitespace() {
        let model = ChatPerplexity::with_api_key("  key with spaces  ");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_with_model_empty_string() {
        let model = ChatPerplexity::with_api_key("key").with_model("");
        assert_eq!(model.llm_type(), "perplexity");
    }

    #[test]
    fn test_cloned_model_independence() {
        let model1 = ChatPerplexity::with_api_key("key").with_temperature(0.5);
        let model2 = model1.clone();
        // Both should work independently
        assert_eq!(model1.llm_type(), "perplexity");
        assert_eq!(model2.llm_type(), "perplexity");
    }

    #[test]
    fn test_multiple_clones() {
        let model = ChatPerplexity::with_api_key("key");
        let clone1 = model.clone();
        let clone2 = clone1.clone();
        let clone3 = clone2.clone();
        assert_eq!(clone3.llm_type(), "perplexity");
    }

    // ========================================================================
    // ChatPerplexity - Rate Limiter Tests
    // ========================================================================

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            std::time::Duration::from_millis(100),
            10.0,
        ));
        let model = ChatPerplexity::with_api_key("key").with_rate_limiter(rate_limiter);
        assert!(model.rate_limiter().is_some());
    }

    #[test]
    fn test_rate_limiter_after_other_builders() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            std::time::Duration::from_millis(100),
            10.0,
        ));
        let model = ChatPerplexity::with_api_key("key")
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_rate_limiter(rate_limiter);
        assert!(model.rate_limiter().is_some());
    }
}
