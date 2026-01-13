// Allow clippy warnings for API integration patterns
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::clone_on_ref_ptr,
    clippy::needless_pass_by_value
)]

//! Anthropic/Claude integration for DashFlow
//!
//! This crate provides integration with Anthropic's Claude models through the
//! Messages API. It implements the [`ChatModel`] trait from dashflow::core.
//!
//! # Features
//!
//! - [`ChatAnthropic`] - Chat completions with Claude 3.5 Sonnet, Opus, and Haiku
//! - [`ThinkingConfig`] - Extended thinking (chain-of-thought) support
//! - [`CacheControl`] - Prompt caching for cost optimization
//! - [`SystemBlock`] - System prompt configuration
//! - Tool/function calling support
//! - Streaming responses
//! - Rate limiting support (retry logic available via executor's RetryPolicy)
//!
//! # Quick Start
//!
//! ```no_run
//! use dashflow_anthropic::ChatAnthropic;
//! use dashflow::core::messages::Message;
//! use dashflow::core::language_models::ChatModel;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a model (uses ANTHROPIC_API_KEY env var)
//! let model = ChatAnthropic::try_new()?
//!     .with_model("claude-3-5-sonnet-20241022");
//!
//! let messages = vec![Message::human("Hello, Claude!")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! println!("{}", response.generations[0].message.as_text());
//! # Ok(())
//! # }
//! ```
//!
//! # With Extended Thinking
//!
//! ```rust,ignore
//! use dashflow_anthropic::{ChatAnthropic, ThinkingConfig};
//!
//! let model = ChatAnthropic::try_new()?
//!     .with_model("claude-3-5-sonnet-20241022")
//!     .with_thinking(ThinkingConfig::enabled(10_000));  // 10k thinking tokens
//!
//! // Model will think through complex problems step-by-step
//! let messages = vec![Message::human("Solve this math problem: ...")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! ```
//!
//! # Configuration via YAML
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelConfig;
//!
//! let config: ChatModelConfig = serde_yml::from_str(r#"
//!     provider: anthropic
//!     model: claude-3-5-sonnet-20241022
//!     temperature: 0.7
//!     max_tokens: 4096
//! "#)?;
//!
//! let model = config.build()?;
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::messages::Message`] - Message types for conversations
//! - [`dashflow::core::retry::RetryPolicy`] - Configure retry behavior
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_bedrock`](https://docs.rs/dashflow-bedrock) - AWS Bedrock (Claude via AWS)
//!
//! [`ChatModel`]: dashflow::core::language_models::ChatModel

pub mod chat_models;

pub use chat_models::{CacheControl, ChatAnthropic, SystemBlock, ThinkingConfig};

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};
