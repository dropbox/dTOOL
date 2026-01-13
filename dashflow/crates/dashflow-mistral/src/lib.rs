// Allow clippy warnings for API integration patterns
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::redundant_clone,
    clippy::clone_on_ref_ptr
)]

//! Mistral AI integration for `DashFlow` Rust
//!
//! This crate provides integration with Mistral AI's language models,
//! allowing you to use Mistral's powerful open-source and proprietary models
//! within the `DashFlow` framework.
//!
//! # Features
//!
//! - Full support for Mistral AI chat models
//! - Streaming support for real-time generation
//! - Tool calling (function calling) support
//! - Rate limiting and retry logic
//! - Multiple model support (Mistral Small, Medium, Large, Codestral, Open models)
//!
//! # Quick Start
//!
//! ```no_run
//! use dashflow_mistral::ChatMistralAI;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a Mistral chat model (requires MISTRAL_API_KEY environment variable)
//!     let model = ChatMistralAI::new()
//!         .with_model("mistral-small-latest")
//!         .with_temperature(0.7);
//!
//!     // Generate a response
//!     let messages = vec![Message::human("What is the capital of France?")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!
//!     println!("{}", result.generations[0].message.as_text());
//! }
//! ```
//!
//! # Streaming
//!
//! ```no_run
//! use dashflow_mistral::ChatMistralAI;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatMistralAI::new();
//!     let messages = vec![Message::human("Tell me a short story")];
//!
//!     let mut stream = model.stream(&messages, None, None, None, None).await.unwrap();
//!     while let Some(chunk) = stream.next().await {
//!         match chunk {
//!             Ok(chunk) => print!("{}", chunk.message.content),
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//! }
//! ```
//!
//! # Available Models
//!
//! - `mistral-small-latest` - Fast and efficient for simple tasks (default)
//! - `mistral-medium-latest` - Balanced performance and capability
//! - `mistral-large-latest` - Most capable model for complex tasks
//! - `codestral-latest` - Specialized for code generation
//! - `open-mistral-7b` - Open-source 7B parameter model
//! - `open-mixtral-8x7b` - Open-source mixture of experts model
//! - `open-mixtral-8x22b` - Larger mixture of experts model
//!
//! # Authentication
//!
//! Set the `MISTRAL_API_KEY` environment variable with your Mistral API key.
//! You can get an API key from [Mistral AI Console](https://console.mistral.ai/).
//!
//! ```bash
//! export MISTRAL_API_KEY=your-api-key
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_anthropic`](https://docs.rs/dashflow-anthropic) - Anthropic/Claude integration
//! - [Mistral AI Console](https://console.mistral.ai/) - API key management

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatMistralAI;
pub use embeddings::MistralEmbeddings;

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};

// Re-export commonly used types from dashflow::core
pub use dashflow::core::{
    error::{Error, Result},
    language_models::{ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult},
    messages::{BaseMessage, Message},
};
