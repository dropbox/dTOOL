//! Chat message history storage backends
//!
//! Provides persistent storage implementations for chat message histories,
//! enabling conversation state to be stored in various databases.
//!
//! # Available Backends
//!
//! - **File**: Local JSON file storage (no external dependencies)
//! - **Redis**: In-memory key-value store with optional TTL
//! - **`MongoDB`**: Document-based `NoSQL` database
//! - **`PostgreSQL`**: Relational database with JSONB support
//! - **`DynamoDB`**: AWS `NoSQL` database with optional TTL
//! - **Upstash Redis**: Serverless Redis with REST API
//! - **Cassandra**: Apache Cassandra/ScyllaDB distributed database
//!
//! # Usage
//!
//! All backends implement the `BaseChatMessageHistory` trait from `dashflow::core`,
//! providing a consistent interface for storing and retrieving messages.
//!
//! ```rust,ignore
//! use dashflow_memory::RedisChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create a Redis-backed history
//! let history = RedisChatMessageHistory::new(
//!     "session-123".to_string(),
//!     "redis://localhost:6379/0".to_string(),
//!     None,
//!     None,
//! ).await?;
//!
//! // Use the history
//! history.add_user_message("Hello!").await?;
//! let messages = history.get_messages().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Python Baseline Compatibility
//!
//! These implementations match the storage backends from
//! `dashflow_community.chat_message_histories` module.

pub mod file;

#[cfg(feature = "redis-backend")]
pub mod redis;

#[cfg(feature = "mongodb-backend")]
pub mod mongodb;

#[cfg(feature = "postgres-backend")]
pub mod postgres;

#[cfg(feature = "dynamodb-backend")]
pub mod dynamodb;

#[cfg(feature = "upstash-backend")]
pub mod upstash_redis;

#[cfg(feature = "cassandra-backend")]
pub mod cassandra;

pub use file::FileChatMessageHistory;

#[cfg(feature = "redis-backend")]
pub use redis::RedisChatMessageHistory;

#[cfg(feature = "mongodb-backend")]
pub use mongodb::MongoDBChatMessageHistory;

#[cfg(feature = "postgres-backend")]
pub use postgres::PostgresChatMessageHistory;

#[cfg(feature = "dynamodb-backend")]
pub use dynamodb::DynamoDBChatMessageHistory;

#[cfg(feature = "upstash-backend")]
pub use upstash_redis::UpstashRedisChatMessageHistory;

#[cfg(feature = "cassandra-backend")]
pub use cassandra::CassandraChatMessageHistory;
