//! AWS Bedrock integration for DashFlow
//!
//! This crate provides integration with AWS Bedrock's managed AI models.
//! Bedrock provides access to multiple foundation models including:
//! - Anthropic Claude (all versions)
//! - Meta Llama (2 and 3)
//! - Mistral AI models
//! - Cohere models
//! - Amazon Titan models
//!
//! # Features
//!
//! - **Chat Models**: Claude, Llama, Mistral, Cohere, Titan
//! - **Embeddings**: Titan Text Embeddings v1/v2, Cohere Embed v3
//!
//! # Authentication
//!
//! Uses standard AWS SDK authentication chain:
//! - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
//! - AWS credentials file (~/.aws/credentials)
//! - IAM instance profile (for EC2/ECS)
//! - IAM role (for Lambda)
//!
//! # Examples
//!
//! ## Chat Model
//!
//! ```no_run
//! use dashflow_bedrock::ChatBedrock;
//! use dashflow::core::messages::Message;
//! use dashflow::core::language_models::ChatModel;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ChatBedrock::new("us-east-1")
//!     .await?
//!     .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0");
//!
//! let messages = vec![Message::human("Hello from Bedrock!")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Embeddings
//!
//! ```no_run
//! use dashflow_bedrock::BedrockEmbeddings;
//! use dashflow::embed_query;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(BedrockEmbeddings::new("us-east-1").await?);
//!
//! let vector = embed_query(embedder, "What is machine learning?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration via YAML
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelConfig;
//!
//! let config: ChatModelConfig = serde_yml::from_str(r#"
//!     provider: bedrock
//!     model: anthropic.claude-3-5-sonnet-20241022-v2:0
//!     region: us-east-1
//! "#)?;
//!
//! let model = config.build()?;
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_anthropic`](https://docs.rs/dashflow-anthropic) - Direct Anthropic API (alternative)
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [AWS Bedrock Console](https://console.aws.amazon.com/bedrock) - Model access management

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatBedrock;
pub use embeddings::{BedrockEmbeddings, InputType, NormalizeMode};
