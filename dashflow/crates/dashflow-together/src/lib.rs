//! Together AI integration for `DashFlow` Rust
//!
//! This crate provides Together AI implementations for `DashFlow` Rust.
//!
//! # Features
//! - `ChatTogether`: Chat model using Together AI's OpenAI-compatible API
//! - Access to 100+ open-source models (Llama, Mistral, `CodeLlama`, etc.)
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Cost-effective inference
//!
//! # Example
//! ```no_run
//! use dashflow_together::ChatTogether;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatTogether::new()
//!         .with_model("meta-llama/Llama-3-70b-chat-hf")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```
//!
//! # Authentication
//! Set the `TOGETHER_API_KEY` environment variable with your Together AI API key.
//! You can also provide the key directly using `.with_api_key()`.
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_fireworks`](https://docs.rs/dashflow-fireworks) - Fireworks AI alternative
//! - [Together AI](https://www.together.ai/) - API key management

pub mod chat_models;

pub use chat_models::ChatTogether;
