//! Memory implementations for `DashFlow` Rust
//!
//! This crate provides memory abstractions for maintaining conversation state and context
//! across chain executions. Memory allows chains to remember information from past
//! interactions and use that information to inform future responses.
//!
//! # Overview
//!
//! Memory types include:
//! - **`ConversationBufferMemory`**: Stores full conversation history without truncation
//! - **`ConversationBufferWindowMemory`**: Keeps only the last K conversation turns
//! - **`ConversationSummaryMemory`**: Summarizes conversation history using an LLM
//! - **`ConversationEntityMemory`**: Extracts and tracks entities with LLM-generated summaries
//! - **`ConversationKGMemory`**: Extracts and stores knowledge triples in a knowledge graph
//! - **`ConversationTokenBufferMemory`**: Token-limited buffer that prunes old messages
//! - **`VectorStoreRetrieverMemory`**: Stores memories in vector store for semantic retrieval
//! - **`ReadOnlyMemory`**: Read-only wrapper that prevents memory modification
//! - **`SimpleMemory`**: Static key-value memory that never changes
//! - **`CombinedMemory`**: Combines multiple memory types into a single unified memory
//!
//! # Chat Message History Backends
//!
//! Persistent storage backends for chat histories:
//! - **`FileChatMessageHistory`**: Local JSON file storage (always available)
//! - **`RedisChatMessageHistory`**: Redis-backed storage (feature: `redis-backend`)
//! - **`MongoDBChatMessageHistory`**: MongoDB-backed storage (feature: `mongodb-backend`)
//! - **`PostgresChatMessageHistory`**: PostgreSQL-backed storage (feature: `postgres-backend`)
//! - **`DynamoDBChatMessageHistory`**: AWS DynamoDB-backed storage (feature: `dynamodb-backend`)
//! - **`UpstashRedisChatMessageHistory`**: Upstash Redis REST API storage (feature: `upstash-backend`)
//! - **`CassandraChatMessageHistory`**: Cassandra/ScyllaDB-backed storage (feature: `cassandra-backend`)
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::ConversationSummaryMemory;
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::chat_history::InMemoryChatMessageHistory;
//!
//! // Create memory with an LLM for summarization
//! let llm = ChatOpenAI::default();
//! let chat_memory = InMemoryChatMessageHistory::new();
//! let memory = ConversationSummaryMemory::new(llm, chat_memory);
//!
//! // Use with a chain
//! memory.save_context(
//!     &[("input", "Hi, I'm Alice")],
//!     &[("output", "Hello Alice! Nice to meet you.")],
//! ).await?;
//!
//! // Load memory variables for next interaction
//! let vars = memory.load_memory_variables(&[]).await?;
//! // vars["history"] contains the summary
//! ```
//!
//! # Python Baseline Compatibility
//!
//! This implementation matches the Python `DashFlow` memory abstractions from
//! `dashflow.memory` (deprecated in Python v0.3+, but still widely used).
//!
//! Key differences:
//! - Rust is async-first (Python has sync+async variants)
//! - Rust uses Result types for error handling (Python uses exceptions)
//! - Rust memory types are more strongly typed

mod base_memory;
mod combined;
mod conversation_buffer;
mod conversation_buffer_window;
mod conversation_entity;
mod conversation_summary;
mod entity_store;
mod kg;
mod prompts;
mod readonly;
mod simple;
mod token_buffer;
mod vectorstore;

pub mod utils;

// Chat message history backends (file backend always available, others feature-gated)
pub mod chat_message_histories;

pub use base_memory::{BaseMemory, MemoryError, MemoryResult};
pub use combined::CombinedMemory;
pub use conversation_buffer::ConversationBufferMemory;
pub use conversation_buffer_window::ConversationBufferWindowMemory;
pub use conversation_entity::ConversationEntityMemory;
pub use conversation_summary::ConversationSummaryMemory;
pub use entity_store::{EntityStore, InMemoryEntityStore};
pub use kg::{
    get_entities, parse_triples, ConversationKGMemory, KnowledgeTriple, NetworkxEntityGraph,
};
pub use prompts::{
    create_entity_extraction_prompt, create_entity_summarization_prompt,
    create_knowledge_triple_extraction_prompt, create_summary_prompt, ENTITY_EXTRACTION_PROMPT,
    ENTITY_EXTRACTION_PROMPT_TEMPLATE, ENTITY_SUMMARIZATION_PROMPT,
    ENTITY_SUMMARIZATION_PROMPT_TEMPLATE, KG_TRIPLE_DELIMITER, KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT,
    KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE, SUMMARY_PROMPT, SUMMARY_PROMPT_TEMPLATE,
};
pub use readonly::ReadOnlyMemory;
pub use simple::SimpleMemory;
pub use token_buffer::ConversationTokenBufferMemory;
pub use vectorstore::VectorStoreRetrieverMemory;

// Re-export chat message history backends
pub use chat_message_histories::FileChatMessageHistory;

#[cfg(feature = "redis-backend")]
pub use chat_message_histories::RedisChatMessageHistory;

#[cfg(feature = "mongodb-backend")]
pub use chat_message_histories::MongoDBChatMessageHistory;

#[cfg(feature = "postgres-backend")]
pub use chat_message_histories::PostgresChatMessageHistory;

#[cfg(feature = "dynamodb-backend")]
pub use chat_message_histories::DynamoDBChatMessageHistory;

#[cfg(feature = "upstash-backend")]
pub use chat_message_histories::UpstashRedisChatMessageHistory;

#[cfg(feature = "cassandra-backend")]
pub use chat_message_histories::CassandraChatMessageHistory;
