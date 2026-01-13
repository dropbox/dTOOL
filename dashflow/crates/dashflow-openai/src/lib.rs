// clone_on_ref_ptr: Moved to function-level allows where Arc::clone() pattern needed

//! OpenAI integration for DashFlow
//!
//! This crate provides OpenAI implementations for chat models, embeddings,
//! assistants, and structured outputs.
//!
//! # Features
//!
//! - [`ChatOpenAI`] - Chat completions API (GPT-4, GPT-3.5-turbo, etc.)
//! - [`AzureChatOpenAI`] - Azure OpenAI Service integration
//! - [`OpenAIEmbeddings`] - Embedding models (text-embedding-3-small/large, ada-002)
//! - [`OpenAIAssistantRunnable`] - OpenAI Assistants API integration
//! - [`OpenAIStructuredChatModel`] - Structured outputs with JSON schemas
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Configurable retry logic and rate limiting
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a chat model (uses OPENAI_API_KEY env var)
//!     let model = ChatOpenAI::with_config(Default::default())
//!         .with_model("gpt-4")
//!         .with_temperature(0.7);
//!
//!     // Generate a response
//!     let messages = vec![Message::human("What is the capital of France?")];
//!     let result = model.generate(&messages, None, None, None, None).await?;
//!
//!     println!("{}", result.generations[0].message.content());
//!     Ok(())
//! }
//! ```
//!
//! # Streaming Example
//!
//! ```rust,ignore
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let model = ChatOpenAI::with_config(Default::default())
//!         .with_model("gpt-4");
//!
//!     let messages = vec![Message::human("Tell me a story")];
//!     let mut stream = model.stream(&messages, None, None).await?;
//!
//!     while let Some(chunk) = stream.next().await {
//!         let chunk = chunk?;
//!         print!("{}", chunk.message.content());
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # With Tool Calling
//!
//! ```rust,ignore
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::language_models::{ChatModel, ToolDefinition, ToolChoice};
//! use dashflow::core::messages::Message;
//!
//! let model = ChatOpenAI::with_config(Default::default())
//!     .with_model("gpt-4");
//!
//! let tools = vec![ToolDefinition {
//!     name: "get_weather".to_string(),
//!     description: "Get current weather for a location".to_string(),
//!     parameters: serde_json::json!({
//!         "type": "object",
//!         "properties": {
//!             "location": {"type": "string"}
//!         }
//!     }),
//! }];
//!
//! let messages = vec![Message::human("What's the weather in Paris?")];
//! let result = model.generate(&messages, Some(&tools), Some(ToolChoice::Auto), None, None).await?;
//! ```
//!
//! # Configuration via YAML
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelConfig;
//!
//! let config: ChatModelConfig = serde_yml::from_str(r#"
//!     provider: openai
//!     model: gpt-4
//!     temperature: 0.7
//!     max_tokens: 1000
//! "#)?;
//!
//! let model = config.build()?;  // Returns Box<dyn ChatModel>
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::messages::Message`] - Message types for conversations
//! - [`dashflow::core::retry::RetryPolicy`] - Configure retry behavior
//! - [`dashflow_anthropic`](https://docs.rs/dashflow-anthropic) - Anthropic/Claude integration
//! - [`dashflow_azure_openai`](https://docs.rs/dashflow-azure-openai) - Dedicated Azure OpenAI crate

pub mod assistant;
pub mod chat_models;
pub mod embeddings;
pub mod structured;

pub use assistant::{
    AssistantOutput, OpenAIAssistantAction, OpenAIAssistantFinish, OpenAIAssistantRunnable,
};
pub use chat_models::{AzureChatOpenAI, ChatOpenAI};
pub use embeddings::OpenAIEmbeddings;
pub use structured::{OpenAIStructuredChatModel, StructuredOutputMethod};

// Re-export OpenAIConfig for ChatModelConfig::build()
pub use async_openai::config::OpenAIConfig;

mod config_ext;
pub use config_ext::{build_chat_model, build_embeddings, build_llm_node};
