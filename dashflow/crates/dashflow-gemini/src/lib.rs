//! Google Gemini integration for DashFlow
//!
//! This crate provides integration with Google's Gemini models through the
//! Generative Language API. It implements the `ChatModel` and `Embeddings` traits
//! from dashflow::core.
//!
//! # Features
//!
//! - Chat completions with Gemini 2.0 Flash, Pro, and other models
//! - Text embeddings with text-embedding-004
//! - Streaming responses
//! - Function calling
//! - Multimodal support (text, images, video, audio)
//! - System instructions
//! - Safety settings
//! - Thinking mode (extended reasoning)
//!
//! # Chat Example
//!
//! ```no_run
//! use dashflow_gemini::ChatGemini;
//! use dashflow::core::messages::Message;
//! use dashflow::core::language_models::ChatModel;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ChatGemini::new()
//!     .with_api_key("your-api-key")
//!     .with_model("gemini-2.0-flash-exp");
//!
//! let messages = vec![Message::human("Explain quantum computing")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Embeddings Example
//!
//! ```no_run
//! use dashflow_gemini::GeminiEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = GeminiEmbeddings::new()
//!     .with_api_key("your-api-key");
//!
//! let vector = embedder.embed_query("What is machine learning?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Authentication
//!
//! The Gemini API requires an API key for authentication. Get your API key from:
//! <https://ai.google.dev/>
//!
//! Set it via environment variable:
//! ```bash
//! export GEMINI_API_KEY="your-api-key"
//! ```
//!
//! Or pass it directly:
//! ```no_run
//! # use dashflow_gemini::ChatGemini;
//! let model = ChatGemini::new().with_api_key("your-api-key");
//! ```
//!
//! # Configuration via YAML
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelConfig;
//!
//! let config: ChatModelConfig = serde_yml::from_str(r#"
//!     provider: gemini
//!     model: gemini-2.0-flash-exp
//!     temperature: 0.7
//! "#)?;
//!
//! let model = config.build()?;
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::embeddings::Embeddings`] - The trait implemented by embedding models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_anthropic`](https://docs.rs/dashflow-anthropic) - Anthropic/Claude integration
//! - [Google AI Studio](https://ai.google.dev/) - API key management

pub mod chat_models;
pub mod embeddings;

pub use chat_models::{ChatGemini, SafetySettings};
pub use embeddings::{GeminiEmbeddings, TaskType};
