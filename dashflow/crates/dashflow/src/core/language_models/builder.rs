//! Builder pattern for ChatModel with automatic telemetry.
//!
//! This module provides the "batteries included" API for LLM generation.
//! The builder automatically handles telemetry recording.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelBuildExt;
//!
//! // Simple usage - telemetry is automatic
//! let result = model.build_generate(&messages).await?;
//!
//! // With options
//! let result = model
//!     .build_generate(&messages)
//!     .temperature(0.7)
//!     .max_tokens(1000)
//!     .await?;
//! ```

use crate::core::error::Result;
use crate::core::language_models::{ChatModel, ChatResult, ToolChoice, ToolDefinition};
use crate::core::messages::BaseMessage;
use crate::telemetry;
use async_trait::async_trait;
use std::future::{Future, IntoFuture};
use std::pin::Pin;

/// Options for generation.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    /// Temperature for sampling (0.0 to 2.0).
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Stop sequences.
    pub stop: Option<Vec<String>>,
    /// Tool definitions for function calling.
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice configuration.
    pub tool_choice: Option<ToolChoice>,
}

/// Builder for chat generation with automatic telemetry.
///
/// This builder provides a fluent API for configuring generation options
/// and automatically records telemetry when the generation completes.
pub struct GenerateBuilder<'a, M: ChatModel + ?Sized> {
    model: &'a M,
    messages: &'a [BaseMessage],
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    stop: Option<Vec<String>>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,
}

impl<'a, M: ChatModel + ?Sized> GenerateBuilder<'a, M> {
    /// Create a new generate builder.
    pub fn new(model: &'a M, messages: &'a [BaseMessage]) -> Self {
        Self {
            model,
            messages,
            temperature: None,
            max_tokens: None,
            stop: None,
            tools: None,
            tool_choice: None,
        }
    }

    /// Set the temperature for sampling.
    #[must_use]
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set the maximum tokens to generate.
    #[must_use]
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Set stop sequences.
    #[must_use]
    pub fn stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Set tool definitions for function calling.
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice configuration.
    #[must_use]
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }
}

impl<'a, M: ChatModel + ?Sized + Sync> IntoFuture for GenerateBuilder<'a, M> {
    type Output = Result<ChatResult>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // Start telemetry recording
            let record = telemetry::llm_call()
                .model(self.model.model_name().unwrap_or("unknown"))
                .provider(self.model.llm_type())
                .messages(self.messages)
                .start();

            // Make the actual call using the existing generate method
            let result = self
                .model
                .generate(
                    self.messages,
                    self.stop.as_deref(),
                    self.tools.as_deref(),
                    self.tool_choice.as_ref(),
                    None, // No config needed, telemetry is automatic
                )
                .await;

            // Record result
            match &result {
                Ok(response) => {
                    let text = response
                        .generations
                        .first()
                        .map(|g| g.message.content().as_text())
                        .unwrap_or_default();
                    record.success().response_text(&text).finish();
                }
                Err(e) => {
                    record.error(e).finish();
                }
            }

            result
        })
    }
}

/// Extension trait providing the builder pattern for ChatModel.
///
/// This trait adds the `build_generate` method to all ChatModel implementations,
/// providing the "batteries included" API with automatic telemetry.
#[async_trait]
pub trait ChatModelBuildExt: ChatModel {
    /// Create a generation builder with automatic telemetry.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::language_models::ChatModelBuildExt;
    ///
    /// // Simple usage
    /// let result = model.build_generate(&messages).await?;
    ///
    /// // With options
    /// let result = model
    ///     .build_generate(&messages)
    ///     .temperature(0.7)
    ///     .await?;
    /// ```
    fn build_generate<'a>(&'a self, messages: &'a [BaseMessage]) -> GenerateBuilder<'a, Self>
    where
        Self: Sized,
    {
        GenerateBuilder::new(self, messages)
    }
}

// Blanket implementation for all ChatModel types
impl<T: ChatModel> ChatModelBuildExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::language_models::ChatGeneration;
    use crate::core::messages::AIMessage;

    #[derive(Clone)]
    struct MockChatModel {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(self.response.clone()).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_builder_simple() {
        let model = MockChatModel {
            response: "Hello!".to_string(),
        };
        let messages = vec![crate::core::messages::HumanMessage::new("Hi").into()];

        let result = model.build_generate(&messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(result.generations[0].message.content().as_text(), "Hello!");
    }

    #[tokio::test]
    async fn test_builder_with_options() {
        let model = MockChatModel {
            response: "Configured!".to_string(),
        };
        let messages = vec![crate::core::messages::HumanMessage::new("Test").into()];

        let result = model
            .build_generate(&messages)
            .temperature(0.7)
            .max_tokens(100)
            .await
            .unwrap();

        assert_eq!(
            result.generations[0].message.content().as_text(),
            "Configured!"
        );
    }
}
