//! Tool binding support for chat models
//!
//! This module provides utilities for binding tools to chat models, allowing
//! models to call external functions during generation.
//!
//! # Overview
//!
//! The `bind_tools()` method creates a configured chat model that will pass
//! tool definitions to the underlying model during generation. This enables
//! function calling / tool use capabilities across different model providers.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelToolBindingExt;
//! use dashflow::core::language_models::ToolChoice;
//! use dashflow::core::tools::Tool;
//! use dashflow_openai::ChatOpenAI;
//! use std::sync::Arc;
//!
//! let search_tool: Arc<dyn Tool> = Arc::new(MySearchTool::new());
//! let calculator_tool: Arc<dyn Tool> = Arc::new(MyCalculatorTool::new());
//!
//! let model = ChatOpenAI::with_config(Default::default())
//!     .with_model("gpt-4")
//!     .bind_tools(vec![search_tool, calculator_tool], Some(ToolChoice::Auto));
//!
//! let result = model.generate(messages, None, None, None, None).await?;
//! // result.generations[0].message may contain tool_calls
//! ```
//!
//! # Design
//!
//! The tool binding system consists of:
//!
//! - `ChatModelToolBindingExt`: Extension trait adding `bind_tools()` to all `ChatModel` implementations
//! - `BoundChatModel`: Wrapper that adds tool configuration to any `ChatModel`
//! - Tool conversion utilities for converting `Arc<dyn Tool>` to `ToolDefinition`

use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;

use super::{ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition};
use crate::core::callbacks::CallbackManager;
use crate::core::error::Result;
use crate::core::messages::BaseMessage;
use crate::core::tools::Tool;

/// Extension trait that adds tool binding capability to all `ChatModel` implementations.
///
/// This trait provides the `bind_tools()` method which creates a new chat model
/// with pre-configured tools and tool choice settings.
pub trait ChatModelToolBindingExt: ChatModel + Sized {
    /// Bind tools to this chat model.
    ///
    /// Returns a new model that will pass the specified tools to the underlying
    /// model during generation. The `tool_choice` parameter controls whether
    /// and which tools the model should call.
    ///
    /// # Arguments
    ///
    /// * `tools` - Vector of tools to make available to the model
    /// * `tool_choice` - Optional specification of which tool(s) to call
    ///
    /// # Returns
    ///
    /// A `BoundChatModel` that wraps this model with tool configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::language_models::ChatModelToolBindingExt;
    /// use dashflow::core::language_models::ToolChoice;
    ///
    /// let model_with_tools = model.bind_tools(
    ///     vec![search_tool, calculator_tool],
    ///     Some(ToolChoice::Auto)
    /// );
    ///
    /// let result = model_with_tools.generate(messages, None, None, None, None).await?;
    /// ```
    fn bind_tools(
        self,
        tools: Vec<Arc<dyn Tool>>,
        tool_choice: Option<ToolChoice>,
    ) -> BoundChatModel<Self> {
        // Convert tools to definitions
        let tool_definitions: Vec<ToolDefinition> =
            tools.iter().map(|tool| tool.to_definition()).collect();

        BoundChatModel {
            model: self,
            tools: tool_definitions,
            tool_choice,
        }
    }
}

// Implement the extension trait for all ChatModel types
impl<T: ChatModel + Sized> ChatModelToolBindingExt for T {}

/// A chat model with bound tools.
///
/// This wrapper automatically passes tool definitions to the underlying model
/// during generation. Created by calling `bind_tools()` on any `ChatModel`.
///
/// # Example
///
/// ```rust,ignore
/// let bound_model = model.bind_tools(
///     vec![search_tool],
///     Some(ToolChoice::Required)
/// );
///
/// // Tool definitions are automatically passed to generate()
/// let result = bound_model.generate(messages, None, None, None, None).await?;
/// ```
#[derive(Clone)]
pub struct BoundChatModel<T: ChatModel> {
    /// The underlying chat model
    model: T,
    /// Tool definitions to pass to the model
    tools: Vec<ToolDefinition>,
    /// Tool choice specification
    tool_choice: Option<ToolChoice>,
}

impl<T: ChatModel> BoundChatModel<T> {
    /// Get a reference to the underlying model
    pub fn inner(&self) -> &T {
        &self.model
    }

    /// Get the bound tools
    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    /// Get the tool choice setting
    pub fn tool_choice(&self) -> Option<&ToolChoice> {
        self.tool_choice.as_ref()
    }
}

#[async_trait]
impl<T: ChatModel + 'static> ChatModel for BoundChatModel<T> {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        // Merge bound tools with provided tools
        let effective_tools = if tools.is_some() {
            // If caller provides tools, use those (allows override)
            tools
        } else {
            // Otherwise use bound tools
            Some(&self.tools[..])
        };

        // Use bound tool_choice if caller doesn't provide one
        let effective_tool_choice = tool_choice.or(self.tool_choice.as_ref());

        self.model
            ._generate(
                messages,
                stop,
                effective_tools,
                effective_tool_choice,
                run_manager,
            )
            .await
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Merge bound tools with provided tools
        let effective_tools = if tools.is_some() {
            // If caller provides tools, use those (allows override)
            tools
        } else {
            // Otherwise use bound tools
            Some(&self.tools[..])
        };

        // Use bound tool_choice if caller doesn't provide one
        let effective_tool_choice = tool_choice.or(self.tool_choice.as_ref());

        self.model
            ._stream(
                messages,
                stop,
                effective_tools,
                effective_tool_choice,
                run_manager,
            )
            .await
    }

    fn llm_type(&self) -> &str {
        self.model.llm_type()
    }

    fn rate_limiter(&self) -> Option<Arc<dyn crate::core::rate_limiters::RateLimiter>> {
        self.model.rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::ChatModelToolBindingExt;
    use crate::core::language_models::FakeChatModel;
    use crate::core::messages::BaseMessage;
    use crate::core::tools::sync_function_tool;
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_bind_tools_basic() {
        let model = FakeChatModel::new(vec!["test response".to_string()]);

        let tool = sync_function_tool("calculator", "Performs calculations", |input: String| {
            Ok(format!("Result: {}", input))
        });

        let bound_model = model.bind_tools(vec![Arc::new(tool)], Some(ToolChoice::Auto));

        assert_eq!(bound_model.tools().len(), 1);
        assert_eq!(bound_model.tools()[0].name, "calculator");
        assert_eq!(bound_model.tool_choice(), Some(&ToolChoice::Auto));
    }

    #[tokio::test]
    async fn test_bound_model_passes_tools() {
        let model = FakeChatModel::new(vec!["test response".to_string()]);

        let tool = sync_function_tool("search", "Searches for information", |input: String| {
            Ok(format!("Found: {}", input))
        });

        let bound_model = model.bind_tools(vec![Arc::new(tool)], Some(ToolChoice::Required));

        let messages = vec![BaseMessage::human("test")];

        // Generate should succeed and pass through to underlying model
        let result = bound_model
            .generate(&messages, None, None, None, None)
            .await;

        assert!(result.is_ok());
        let chat_result = result.unwrap();
        assert!(!chat_result.generations.is_empty());
    }

    #[tokio::test]
    async fn test_bind_tools_multiple_tools() {
        let model = FakeChatModel::new(vec!["test response".to_string()]);

        let tool1 = sync_function_tool("search", "Searches", |input: String| Ok(input));
        let tool2 = sync_function_tool("calculator", "Calculates", |input: String| Ok(input));
        let tool3 = sync_function_tool("translator", "Translates", |input: String| Ok(input));

        let bound_model = model.bind_tools(
            vec![Arc::new(tool1), Arc::new(tool2), Arc::new(tool3)],
            Some(ToolChoice::Auto),
        );

        assert_eq!(bound_model.tools().len(), 3);
        assert_eq!(bound_model.tools()[0].name, "search");
        assert_eq!(bound_model.tools()[1].name, "calculator");
        assert_eq!(bound_model.tools()[2].name, "translator");
    }

    #[tokio::test]
    async fn test_bind_tools_without_tool_choice() {
        let model = FakeChatModel::new(vec!["test response".to_string()]);

        let tool = sync_function_tool("test", "Test tool", |input: String| Ok(input));

        let bound_model = model.bind_tools(vec![Arc::new(tool)], None);

        assert_eq!(bound_model.tools().len(), 1);
        assert_eq!(bound_model.tool_choice(), None);
    }

    #[tokio::test]
    async fn test_bind_tools_on_arc_dyn_chatmodel() {
        use crate::core::language_models::ChatModel;

        // Create Arc<dyn ChatModel> from FakeChatModel
        let model: Arc<dyn ChatModel> =
            Arc::new(FakeChatModel::new(vec!["test response".to_string()]));

        let tool = sync_function_tool("search", "Search tool", |input: String| {
            Ok(format!("Search result: {}", input))
        });

        // This should work with the new impl ChatModel for Arc<dyn ChatModel>
        let bound_model = model.bind_tools(vec![Arc::new(tool)], Some(ToolChoice::Auto));

        assert_eq!(bound_model.tools().len(), 1);
        assert_eq!(bound_model.tools()[0].name, "search");
        assert_eq!(bound_model.tool_choice(), Some(&ToolChoice::Auto));

        // Test that generate works
        let messages = vec![BaseMessage::human("test")];
        let result = bound_model
            .generate(&messages, None, None, None, None)
            .await;

        assert!(result.is_ok());
        let chat_result = result.unwrap();
        assert!(!chat_result.generations.is_empty());
    }
}
