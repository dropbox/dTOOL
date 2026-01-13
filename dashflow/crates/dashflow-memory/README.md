# dashflow-memory

Memory implementations for DashFlow - maintain conversation context and state across chain executions with flexible storage backends.

## Overview

Memory is essential for building conversational AI applications that remember past interactions. This crate provides 10 memory types with 7 persistent storage backends, enabling sophisticated conversation state management:

**Memory Types:**
- **ConversationBufferMemory** - Full conversation history without truncation
- **ConversationBufferWindowMemory** - Keep only the last K conversation turns
- **ConversationSummaryMemory** - Summarize conversation history using an LLM
- **ConversationEntityMemory** - Extract and track entities with LLM-generated summaries
- **ConversationKGMemory** - Extract and store knowledge triples in a knowledge graph
- **ConversationTokenBufferMemory** - Token-limited buffer that prunes old messages
- **VectorStoreRetrieverMemory** - Store memories in vector store for semantic retrieval
- **ReadOnlyMemory** - Read-only wrapper preventing memory modification
- **SimpleMemory** - Static key-value memory that never changes
- **CombinedMemory** - Combine multiple memory types into unified memory

**Storage Backends:**
- **FileChatMessageHistory** - Local JSON file storage (always available)
- **RedisChatMessageHistory** - Redis-backed storage with optional TTL
- **MongoDBChatMessageHistory** - MongoDB document storage
- **PostgresChatMessageHistory** - PostgreSQL with JSONB support
- **DynamoDBChatMessageHistory** - AWS DynamoDB with optional TTL
- **UpstashRedisChatMessageHistory** - Serverless Redis with REST API
- **CassandraChatMessageHistory** - Cassandra/ScyllaDB distributed storage

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-memory = "1.11"
```

**With storage backends:**
```toml
[dependencies]
dashflow-memory = { version = "1.11", features = ["redis-backend", "postgres-backend"] }
```

**Available features:**
- `redis-backend` - Redis storage support
- `mongodb-backend` - MongoDB storage support
- `postgres-backend` - PostgreSQL storage support
- `dynamodb-backend` - AWS DynamoDB storage support
- `upstash-backend` - Upstash Redis storage support
- `cassandra-backend` - Cassandra/ScyllaDB storage support

## Quick Start

```rust
use dashflow_memory::{ConversationBufferMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create memory with in-memory history
    let chat_history = InMemoryChatMessageHistory::new();
    let memory = ConversationBufferMemory::new(chat_history);

    // Save a conversation turn
    memory.save_context(
        &[("input", "Hi, I'm Alice")],
        &[("output", "Hello Alice! Nice to meet you.")],
    ).await?;

    // Load memory variables for next interaction
    let vars = memory.load_memory_variables(&[]).await?;
    println!("{}", vars.get("history").unwrap());
    // Output: Human: Hi, I'm Alice\nAI: Hello Alice! Nice to meet you.

    Ok(())
}
```

## Core Trait: BaseMemory

All memory types implement the `BaseMemory` trait:

```rust
#[async_trait]
pub trait BaseMemory: Send + Sync {
    /// Load memory variables for the given input keys
    async fn load_memory_variables(
        &self,
        inputs: &[&str],
    ) -> MemoryResult<HashMap<String, String>>;

    /// Save context from this conversation turn
    async fn save_context(
        &self,
        inputs: &[(&str, &str)],
        outputs: &[(&str, &str)],
    ) -> MemoryResult<()>;

    /// Clear memory contents
    async fn clear(&self) -> MemoryResult<()>;

    /// Get the memory keys (variable names) this memory provides
    fn memory_variables(&self) -> Vec<String>;
}
```

## Memory Types

### 1. ConversationBufferMemory

Stores the full conversation history without truncation. Best for short conversations that fit in context.

```rust
use dashflow_memory::{ConversationBufferMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;

let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationBufferMemory::new(chat_history)
    .with_memory_key("history")      // Variable name (default: "history")
    .with_input_key("input")         // Input field name (default: "input")
    .with_output_key("output")       // Output field name (default: "output")
    .with_return_messages(false);    // Return as string (default) or Message objects

memory.save_context(
    &[("input", "What's the weather?")],
    &[("output", "It's sunny!")],
).await?;

let vars = memory.load_memory_variables(&[]).await?;
// vars["history"] = "Human: What's the weather?\nAI: It's sunny!"
```

**Configuration:**
- `memory_key` - Variable name in memory dict (default: `"history"`)
- `input_key` - Key for human input (default: `"input"`)
- `output_key` - Key for AI output (default: `"output"`)
- `return_messages` - Return Message objects instead of string (default: `false`)
- `human_prefix` - Prefix for human messages (default: `"Human"`)
- `ai_prefix` - Prefix for AI messages (default: `"AI"`)

**Use Cases:**
- Short conversations (< 10 turns)
- When you need full conversation context
- Debugging and testing

**Limitations:**
- No truncation - can exceed context window
- Memory usage grows linearly with conversation length

---

### 2. ConversationBufferWindowMemory

Keeps only the last K conversation turns, automatically discarding older messages.

```rust
use dashflow_memory::{ConversationBufferWindowMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;

let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationBufferWindowMemory::new(chat_history)
    .with_k(3);  // Keep last 3 turns (6 messages: 3 human + 3 AI)

// Add 5 conversation turns
for i in 1..=5 {
    memory.save_context(
        &[("input", &format!("Question {}", i))],
        &[("output", &format!("Answer {}", i))],
    ).await?;
}

let vars = memory.load_memory_variables(&[]).await?;
// vars["history"] only contains turns 3, 4, 5 (last 3 turns)
```

**Configuration:**
- `k` - Number of conversation turns to keep (default: `5`)
- All options from ConversationBufferMemory

**Use Cases:**
- Long-running conversations
- Chatbots with limited context windows
- When recent context is most relevant

**Tradeoffs:**
- Fixed window size - doesn't adapt to context length
- Loses older context that might be relevant
- Simple and predictable behavior

---

### 3. ConversationSummaryMemory

Summarizes conversation history using an LLM, keeping a running summary instead of full messages.

```rust
use dashflow_memory::{ConversationSummaryMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

let llm = Box::new(ChatOpenAI::default());
let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationSummaryMemory::new(llm, chat_history);

// First turn creates initial summary
memory.save_context(
    &[("input", "Hi, I'm Alice and I love hiking")],
    &[("output", "Hello Alice! Hiking is wonderful.")],
).await?;

// Subsequent turns update the summary
memory.save_context(
    &[("input", "What mountains should I visit?")],
    &[("output", "Consider the Rockies or the Alps.")],
).await?;

let vars = memory.load_memory_variables(&[]).await?;
// vars["history"] = "Alice introduced herself as a hiking enthusiast.
//                    The AI recommended the Rockies and Alps for mountain visits."
```

**Configuration:**
- `llm` - Language model for summarization (required)
- `buffer` - Temporary buffer for new messages before summarization
- `max_token_limit` - Trigger summarization when exceeded (default: `2000`)
- All options from ConversationBufferMemory

**Use Cases:**
- Very long conversations (100+ turns)
- When you need to preserve key information without full history
- Memory-constrained applications

**Tradeoffs:**
- LLM cost for summarization (extra API calls)
- Information loss from summarization
- Summary quality depends on LLM capabilities

**Performance:**
- LLM calls: 1 per save_context() when buffer exceeds token limit
- Token usage: ~100-200 tokens per summarization

**Example:** `cargo run --example conversation_summary_memory` (104 lines)

---

### 4. ConversationEntityMemory

Extracts and tracks entities (people, places, concepts) from conversation with LLM-generated summaries for each entity.

```rust
use dashflow_memory::{ConversationEntityMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

let llm = ChatOpenAI::default();
let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationEntityMemory::new(llm, chat_history);

memory.save_context(
    &[("input", "Alice works at Anthropic in San Francisco")],
    &[("output", "That's great! Anthropic is an AI safety company.")],
).await?;

let vars = memory.load_memory_variables(&["Alice"]).await?;
// vars["history"] = "Current conversation: ...\n
//                    About Alice: Works at Anthropic in San Francisco."
// vars["entities"] contains JSON with entity summaries:
// {"Alice": "Works at Anthropic in San Francisco",
//  "Anthropic": "AI safety company",
//  "San Francisco": "Location of Anthropic"}
```

**Configuration:**
- `llm` - Language model for entity extraction and summarization (required)
- `entity_extraction_prompt` - Custom prompt for extracting entities
- `entity_summarization_prompt` - Custom prompt for summarizing entity info
- `k` - Number of recent conversation turns to include (default: `3`)
- All options from ConversationBufferMemory

**Use Cases:**
- Personalized chatbots that remember user details
- Customer service applications tracking customer information
- Multi-topic conversations requiring context switching
- CRM integration and entity relationship tracking

**Tradeoffs:**
- High LLM cost (2 LLM calls per save_context: extraction + summarization)
- Requires entity names as input to load_memory_variables()
- More complex than simple buffer memory

**Performance:**
- LLM calls: 2 per save_context() (extraction + summarization per entity)
- Token usage: ~200-400 tokens per conversation turn

**Example:** `cargo run --example conversation_entity_memory` (125 lines)

---

### 5. ConversationKGMemory

Extracts knowledge triples (subject-predicate-object) from conversation and stores them in a knowledge graph.

```rust
use dashflow_memory::{ConversationKGMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

let llm = ChatOpenAI::default();
let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationKGMemory::new(llm, chat_history)?;

memory.save_context(
    &[("input", "Alice works at Anthropic")],
    &[("output", "Anthropic is focused on AI safety")],
).await?;

// Extracts triples:
// (Alice, works_at, Anthropic)
// (Anthropic, focused_on, AI_safety)

let vars = memory.load_memory_variables(&["Alice"]).await?;
// vars["history"] contains triples related to Alice:
// "Alice works_at Anthropic\nAnthropic focused_on AI_safety"
```

**Configuration:**
- `llm` - Language model for triple extraction (required)
- `k` - Number of related triples to retrieve (default: `2`)
- `kg_triple_delimiter` - Delimiter for triples in output (default: `"->"`)

**Use Cases:**
- Building knowledge bases from conversations
- Relationship tracking (who knows whom, what relates to what)
- Question answering over conversation history
- Research and note-taking applications

**Tradeoffs:**
- LLM cost for triple extraction
- Knowledge graph requires structure in conversation
- Triple extraction quality depends on LLM capabilities

**Performance:**
- LLM calls: 1 per save_context() (triple extraction)
- Token usage: ~100-200 tokens per conversation turn

---

### 6. ConversationTokenBufferMemory

Token-limited buffer that automatically prunes old messages when token limit is exceeded.

```rust
use dashflow_memory::{ConversationTokenBufferMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

let llm = Box::new(ChatOpenAI::default());
let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationTokenBufferMemory::new(llm, chat_history)
    .with_max_token_limit(500);  // Keep messages fitting in 500 tokens

// Add messages until token limit exceeded
memory.save_context(
    &[("input", "Tell me a long story...")],
    &[("output", "Once upon a time...")],
).await?;

// Older messages automatically pruned when limit exceeded
```

**Configuration:**
- `llm` - Language model for token counting (required)
- `max_token_limit` - Maximum tokens to keep (default: `2000`)
- All options from ConversationBufferMemory

**Use Cases:**
- Precise context window management
- Cost optimization (control token usage)
- Adaptive conversation length based on message size

**Tradeoffs:**
- Requires LLM for token counting
- More complex than simple window memory
- Token counting adds latency

**Performance:**
- LLM calls: 0 (uses tiktoken-rs for counting)
- Token counting: Fast (local, no API calls)

---

### 7. VectorStoreRetrieverMemory

Stores all memories in a vector store and retrieves semantically relevant memories based on current input.

```rust
use dashflow_memory::{VectorStoreRetrieverMemory, BaseMemory};
use dashflow_openai::{ChatOpenAI, OpenAIEmbeddings};
use dashflow_qdrant::Qdrant;

let embeddings = OpenAIEmbeddings::default();
let vector_store = Qdrant::from_texts(
    Vec::new(),
    embeddings,
    "http://localhost:6333",
    "memory",
).await?;

let retriever = vector_store.as_retriever().with_k(5);
let memory = VectorStoreRetrieverMemory::new(retriever);

// Save memories (automatically embedded and stored)
memory.save_context(
    &[("input", "I love pizza")],
    &[("output", "Pizza is delicious!")],
).await?;

memory.save_context(
    &[("input", "What's the weather?")],
    &[("output", "It's sunny today.")],
).await?;

// Load relevant memories (semantic search)
let vars = memory.load_memory_variables(&["food"]).await?;
// vars["history"] contains pizza conversation (semantically similar to "food")
// Weather conversation not included (not relevant)
```

**Configuration:**
- `retriever` - Vector store retriever for semantic search (required)
- `memory_key` - Variable name (default: `"history"`)
- `input_key` - Input field name (default: `"input"`)
- `output_key` - Output field name (default: `"output"`)

**Use Cases:**
- Very long conversations (1000+ turns)
- Multi-topic conversations with context switching
- RAG-style memory (retrieve relevant past context)
- Applications requiring semantic memory search

**Tradeoffs:**
- Requires vector store infrastructure
- Embedding cost for each message
- Retrieval may miss chronologically important context
- More complex setup than buffer memory

**Performance:**
- Embedding calls: 1 per save_context() and load_memory_variables()
- Vector search: Fast (ms latency with proper indexing)
- Scales to millions of messages

**Example:** `cargo run --example vectorstore_retriever_memory` (213 lines)

---

### 8. ReadOnlyMemory

Wraps any memory type to prevent modifications. Useful for debugging and testing.

```rust
use dashflow_memory::{ConversationBufferMemory, ReadOnlyMemory, BaseMemory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;

let chat_history = InMemoryChatMessageHistory::new();
let inner_memory = ConversationBufferMemory::new(chat_history);

// Wrap in read-only memory
let memory = ReadOnlyMemory::new(inner_memory);

// Can read memory
let vars = memory.load_memory_variables(&[]).await?;

// Cannot modify (returns error)
let result = memory.save_context(
    &[("input", "test")],
    &[("output", "test")],
).await;
assert!(result.is_err());  // MemoryError: "Memory is read-only"
```

**Use Cases:**
- Preventing accidental memory modification
- Testing chains without side effects
- Debugging memory behavior
- Sharing memory across multiple chains safely

---

### 9. SimpleMemory

Static key-value memory that never changes after creation.

```rust
use dashflow_memory::{SimpleMemory, BaseMemory};
use std::collections::HashMap;

let mut initial_data = HashMap::new();
initial_data.insert("context".to_string(), "Customer service chatbot".to_string());
initial_data.insert("company".to_string(), "Acme Corp".to_string());

let memory = SimpleMemory::new(initial_data);

// Can read memory
let vars = memory.load_memory_variables(&[]).await?;
// vars["context"] = "Customer service chatbot"
// vars["company"] = "Acme Corp"

// save_context() is a no-op (does nothing)
memory.save_context(&[("input", "test")], &[("output", "test")]).await?;

// Memory unchanged
let vars = memory.load_memory_variables(&[]).await?;
// Still the same initial values
```

**Use Cases:**
- Providing static context to chains
- System prompts and instructions
- Configuration data that doesn't change
- Testing with fixed memory state

---

### 10. CombinedMemory

Combines multiple memory types into a single unified memory. Each sub-memory contributes its variables.

```rust
use dashflow_memory::{
    CombinedMemory, ConversationSummaryMemory, ConversationEntityMemory, BaseMemory
};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

// Create summary memory
let llm1 = Box::new(ChatOpenAI::default());
let chat_history1 = InMemoryChatMessageHistory::new();
let summary_memory = ConversationSummaryMemory::new(llm1, chat_history1)
    .with_memory_key("summary");

// Create entity memory
let llm2 = ChatOpenAI::default();
let chat_history2 = InMemoryChatMessageHistory::new();
let entity_memory = ConversationEntityMemory::new(llm2, chat_history2)
    .with_memory_key("entities");

// Combine them
let combined = CombinedMemory::new(vec![
    Box::new(summary_memory),
    Box::new(entity_memory),
]);

// Save context to both memories
combined.save_context(
    &[("input", "Alice works at Anthropic")],
    &[("output", "That's great!")],
).await?;

// Load from both memories
let vars = combined.load_memory_variables(&["Alice"]).await?;
// vars["summary"] = conversation summary
// vars["entities"] = entity information about Alice
```

**Use Cases:**
- Sophisticated conversational AI requiring multiple context types
- Combining short-term (buffer) and long-term (summary) memory
- Multi-modal memory (entities + conversation + knowledge graph)
- Experiments comparing different memory strategies

**Tradeoffs:**
- Increased complexity
- Higher LLM costs (multiple memories making API calls)
- Potential for conflicting memory_key names

**Example:** `cargo run --example combined_memory` (114 lines)

---

## Storage Backends

All memory types use chat message history backends for persistence. Choose the backend that matches your infrastructure.

### FileChatMessageHistory (Always Available)

Local JSON file storage. No external dependencies.

```rust
use dashflow_memory::{FileChatMessageHistory, ConversationBufferMemory, BaseMemory};
use dashflow::core::chat_history::BaseChatMessageHistory;

let file_history = FileChatMessageHistory::new("conversations/session-123.json".into())?;
let memory = ConversationBufferMemory::new(file_history);

// Messages automatically persisted to JSON file
memory.save_context(
    &[("input", "Hello")],
    &[("output", "Hi there!")],
).await?;

// File contents:
// [
//   {"type": "human", "content": "Hello"},
//   {"type": "ai", "content": "Hi there!"}
// ]
```

**Features:**
- Zero configuration
- Human-readable JSON format
- Simple backup and restore
- Cross-platform file paths

**Use Cases:**
- Development and testing
- Single-user applications
- Desktop applications
- Simple persistence needs

---

### RedisChatMessageHistory

In-memory key-value store with optional TTL (time-to-live).

**Feature flag:** `redis-backend`

```rust
use dashflow_memory::RedisChatMessageHistory;

let history = RedisChatMessageHistory::new(
    "session-123".to_string(),
    "redis://localhost:6379/0".to_string(),
    Some(3600),  // TTL: 1 hour
    Some("chat:".to_string()),  // Key prefix
).await?;
```

**Features:**
- Fast in-memory storage
- Automatic expiration with TTL
- Atomic operations
- Scales horizontally with Redis Cluster

**Configuration:**
- `session_id` - Unique session identifier (required)
- `url` - Redis connection URL (required)
- `ttl` - Time-to-live in seconds (optional)
- `key_prefix` - Key prefix for namespacing (optional)

**Use Cases:**
- Web applications with session management
- High-throughput chatbots
- Temporary conversation storage
- Distributed systems

---

### MongoDBChatMessageHistory

Document-based NoSQL database with rich querying.

**Feature flag:** `mongodb-backend`

```rust
use dashflow_memory::MongoDBChatMessageHistory;

let history = MongoDBChatMessageHistory::new(
    "mongodb://localhost:27017".to_string(),
    "chatbot".to_string(),      // Database name
    "histories".to_string(),     // Collection name
    "session-123".to_string(),   // Session ID
).await?;
```

**Features:**
- Rich document storage
- Flexible schema
- Powerful querying and aggregation
- Scales with MongoDB sharding

**Use Cases:**
- Applications with complex metadata
- Analytics on conversation data
- Multi-tenant systems
- Applications already using MongoDB

---

### PostgresChatMessageHistory

Relational database with JSONB support for message storage.

**Feature flag:** `postgres-backend`

```rust
use dashflow_memory::PostgresChatMessageHistory;

let history = PostgresChatMessageHistory::new(
    "postgresql://user:pass@localhost/chatbot".to_string(),
    "chat_histories".to_string(),  // Table name
    "session-123".to_string(),
).await?;
```

**Features:**
- ACID transactions
- Relational data integrity
- JSONB support for messages
- Strong consistency guarantees

**Use Cases:**
- Enterprise applications requiring ACID
- Integration with existing PostgreSQL infrastructure
- Applications with complex relational data
- Regulatory compliance requirements

---

### DynamoDBChatMessageHistory

AWS NoSQL database with serverless scaling and optional TTL.

**Feature flag:** `dynamodb-backend`

```rust
use dashflow_memory::DynamoDBChatMessageHistory;

let history = DynamoDBChatMessageHistory::new(
    "chat-histories".to_string(),   // Table name
    "session-123".to_string(),
    Some(3600),                     // TTL: 1 hour
).await?;
```

**Features:**
- Serverless scaling
- Global replication
- Automatic TTL expiration
- Pay-per-request pricing

**Use Cases:**
- AWS-native applications
- Variable/unpredictable traffic
- Global applications
- Serverless architectures

---

### UpstashRedisChatMessageHistory

Serverless Redis with REST API (no connection pooling needed).

**Feature flag:** `upstash-backend`

```rust
use dashflow_memory::UpstashRedisChatMessageHistory;

let history = UpstashRedisChatMessageHistory::new(
    "https://your-endpoint.upstash.io".to_string(),
    "your-token".to_string(),
    "session-123".to_string(),
    Some(3600),  // TTL
).await?;
```

**Features:**
- No connection management (REST API)
- Pay-per-request pricing
- Global edge caching
- Zero cold starts

**Use Cases:**
- Serverless functions (Lambda, Vercel, Cloudflare Workers)
- Edge computing
- Applications with sporadic traffic
- Prototyping and development

---

### CassandraChatMessageHistory

Apache Cassandra/ScyllaDB distributed database for massive scale.

**Feature flag:** `cassandra-backend`

```rust
use dashflow_memory::CassandraChatMessageHistory;

let history = CassandraChatMessageHistory::new(
    vec!["127.0.0.1:9042".to_string()],  // Contact points
    "chatbot".to_string(),               // Keyspace
    "chat_histories".to_string(),        // Table
    "session-123".to_string(),
).await?;
```

**Features:**
- Linear scalability
- Multi-datacenter replication
- High write throughput
- Tunable consistency

**Use Cases:**
- Massive scale applications (millions of sessions)
- Multi-region deployments
- High write throughput requirements
- Always-on availability needs

---

## Choosing the Right Memory

### Decision Tree

```
1. Do you need persistence?
   NO  → Use InMemoryChatMessageHistory with any memory type
   YES → Continue to #2

2. What's your infrastructure?
   Local/Desktop  → FileChatMessageHistory
   Web App        → Redis or Postgres
   AWS            → DynamoDB
   Serverless     → Upstash Redis
   Massive Scale  → Cassandra

3. What's your conversation pattern?
   Short (<10 turns)           → ConversationBufferMemory
   Medium (10-50 turns)        → ConversationBufferWindowMemory
   Long (50+ turns)            → ConversationSummaryMemory
   Very Long (100+ turns)      → VectorStoreRetrieverMemory

4. Do you need entity tracking?
   YES → ConversationEntityMemory or ConversationKGMemory
   NO  → Continue to #5

5. Do you need precise token control?
   YES → ConversationTokenBufferMemory
   NO  → Use window or summary memory

6. Do you need multiple memory types?
   YES → CombinedMemory
   NO  → Use single memory type
```

### Memory Type Comparison

| Memory Type | Context Length | LLM Calls | Best For | Cost |
|-------------|----------------|-----------|----------|------|
| Buffer | Short (< 10 turns) | 0 | Development, short chats | Free |
| Buffer Window | Fixed (K turns) | 0 | Predictable truncation | Free |
| Summary | Long (50+ turns) | 1/turn | Long conversations | Medium |
| Entity | Variable | 2/turn | Personalization | High |
| KG | Variable | 1/turn | Knowledge extraction | Medium |
| Token Buffer | Token-limited | 0 | Precise control | Free |
| Vector Store | Unlimited | 1/turn | Semantic search | High |
| Read-Only | Any | 0 | Testing, safety | Free |
| Simple | Static | 0 | Fixed context | Free |
| Combined | Any | Variable | Complex needs | Variable |

### Storage Backend Comparison

| Backend | Latency | Throughput | Scalability | Cost | Setup |
|---------|---------|------------|-------------|------|-------|
| File | Low | Low | Single machine | Free | None |
| Redis | Very Low | Very High | Horizontal | Low | Easy |
| MongoDB | Low | High | Horizontal | Medium | Easy |
| PostgreSQL | Low | Medium | Vertical | Low | Easy |
| DynamoDB | Low | Very High | Horizontal | Medium | Easy |
| Upstash | Medium | Medium | Global | Medium | None |
| Cassandra | Low | Very High | Massive | Medium | Complex |

## Best Practices

### 1. Chunk Size and Token Limits

```rust
// For GPT-3.5-turbo (4K context)
let memory = ConversationTokenBufferMemory::new(llm, chat_history)
    .with_max_token_limit(2000);  // Leave ~2000 tokens for prompt + completion

// For GPT-4 (8K context)
let memory = ConversationTokenBufferMemory::new(llm, chat_history)
    .with_max_token_limit(4000);

// For GPT-4-turbo (128K context)
let memory = ConversationTokenBufferMemory::new(llm, chat_history)
    .with_max_token_limit(64000);
```

**General guidelines:**
- Use 50% of context window for memory
- Reserve 25% for system prompt and input
- Reserve 25% for completion

### 2. Error Handling

```rust
use dashflow_memory::{BaseMemory, MemoryError};

match memory.save_context(inputs, outputs).await {
    Ok(()) => println!("Context saved"),
    Err(MemoryError::SerializationError(e)) => {
        eprintln!("Failed to serialize: {}", e);
        // Handle serialization error
    }
    Err(MemoryError::StorageError(e)) => {
        eprintln!("Storage backend error: {}", e);
        // Handle storage error (retry, fallback, etc.)
    }
    Err(e) => {
        eprintln!("Unknown error: {}", e);
    }
}
```

### 3. Memory Key Configuration

```rust
// Avoid conflicts when using multiple memories
let summary = ConversationSummaryMemory::new(llm1, history1)
    .with_memory_key("summary");  // Not "history"

let entities = ConversationEntityMemory::new(llm2, history2)
    .with_memory_key("entities");  // Not "history"

let combined = CombinedMemory::new(vec![
    Box::new(summary),
    Box::new(entities),
]);

// Now you can access both:
let vars = combined.load_memory_variables(&[]).await?;
// vars["summary"] = summary text
// vars["entities"] = entity JSON
```

### 4. Testing and Development

```rust
// Use SimpleMemory for testing with fixed context
let mut test_data = HashMap::new();
test_data.insert("context".to_string(), "Test scenario".to_string());
let memory = SimpleMemory::new(test_data);

// Or wrap real memory in ReadOnlyMemory
let readonly = ReadOnlyMemory::new(production_memory);
```

### 5. Cost Optimization

```rust
// Use buffer memory when possible (free)
let cheap_memory = ConversationBufferWindowMemory::new(chat_history)
    .with_k(5);  // 0 LLM calls

// Use summary memory for longer conversations (paid)
let expensive_memory = ConversationSummaryMemory::new(llm, chat_history);
// 1 LLM call per turn (~$0.0001-0.001 per turn with GPT-3.5)

// Entity memory is most expensive (paid)
let very_expensive = ConversationEntityMemory::new(llm, chat_history);
// 2 LLM calls per turn (~$0.0002-0.002 per turn)
```

## Runnable Examples

All examples require `OPENAI_API_KEY` environment variable:

```bash
export OPENAI_API_KEY="sk-..."
```

**Available Examples:**

1. **Combined Memory** (114 lines)
   ```bash
   cargo run --example combined_memory
   ```
   Demonstrates combining ConversationSummaryMemory and ConversationEntityMemory.

2. **Conversation Entity Memory** (125 lines)
   ```bash
   cargo run --example conversation_entity_memory
   ```
   Demonstrates entity extraction and tracking with LLM-generated summaries.

3. **Conversation Summary Memory** (104 lines)
   ```bash
   cargo run --example conversation_summary_memory
   ```
   Demonstrates conversation summarization for long conversations.

4. **Vector Store Retriever Memory** (213 lines)
   ```bash
   cargo run --example vectorstore_retriever_memory
   ```
   Demonstrates semantic memory retrieval using Qdrant vector store.

## Python to Rust Migration

### Import Changes

**Python:**
```python
from dashflow.memory import ConversationBufferMemory
from dashflow.memory.chat_message_histories import FileChatMessageHistory
```

**Rust:**
```rust
use dashflow_memory::{ConversationBufferMemory, FileChatMessageHistory};
use dashflow::core::chat_history::InMemoryChatMessageHistory;
```

### Async/Await

**Python:**
```python
# Sync version
memory.save_context({"input": "hi"}, {"output": "hello"})
history = memory.load_memory_variables({})

# Async version
await memory.asave_context({"input": "hi"}, {"output": "hello"})
history = await memory.aload_memory_variables({})
```

**Rust (async-first):**
```rust
memory.save_context(
    &[("input", "hi")],
    &[("output", "hello")],
).await?;

let history = memory.load_memory_variables(&[]).await?;
```

### Error Handling

**Python (exceptions):**
```python
try:
    memory.save_context(inputs, outputs)
except Exception as e:
    print(f"Error: {e}")
```

**Rust (Result types):**
```rust
match memory.save_context(inputs, outputs).await {
    Ok(()) => println!("Success"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Memory Construction

**Python:**
```python
from dashflow.memory import ConversationBufferMemory
from dashflow_openai import ChatOpenAI

memory = ConversationBufferMemory(
    memory_key="history",
    return_messages=False,
)

# With LLM (for summary/entity memory)
llm = ChatOpenAI()
summary_memory = ConversationSummaryMemory(llm=llm)
```

**Rust:**
```rust
use dashflow_memory::ConversationBufferMemory;
use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_openai::ChatOpenAI;

let chat_history = InMemoryChatMessageHistory::new();
let memory = ConversationBufferMemory::new(chat_history)
    .with_memory_key("history")
    .with_return_messages(false);

// With LLM
let llm = Box::new(ChatOpenAI::default());
let chat_history = InMemoryChatMessageHistory::new();
let summary_memory = ConversationSummaryMemory::new(llm, chat_history);
```

## Troubleshooting

### Memory Not Persisting

**Problem:** Changes to memory are lost between runs.

**Solution:** Use a persistent storage backend:
```rust
// NOT persistent (in-memory)
let history = InMemoryChatMessageHistory::new();

// PERSISTENT (file)
let history = FileChatMessageHistory::new("session.json".into())?;
```

### Context Window Exceeded

**Problem:** "This model's maximum context length is X tokens" error.

**Solution:** Use token-limited or window memory:
```rust
// Option 1: Token buffer
let memory = ConversationTokenBufferMemory::new(llm, chat_history)
    .with_max_token_limit(2000);

// Option 2: Window memory
let memory = ConversationBufferWindowMemory::new(chat_history)
    .with_k(5);
```

### High LLM Costs

**Problem:** Memory operations are expensive.

**Solution:** Use simpler memory types:
```rust
// Expensive (2 LLM calls per turn)
let expensive = ConversationEntityMemory::new(llm, chat_history);

// Cheaper (1 LLM call per turn)
let cheaper = ConversationSummaryMemory::new(llm, chat_history);

// Free (0 LLM calls)
let free = ConversationBufferWindowMemory::new(chat_history);
```

## Performance Characteristics

### Memory Operations (Latency)

| Operation | Buffer | Window | Summary | Entity | Vector Store |
|-----------|--------|--------|---------|--------|--------------|
| save_context | ~1ms | ~1ms | ~500ms | ~1000ms | ~100ms |
| load_memory_variables | ~1ms | ~1ms | ~1ms | ~1ms | ~50ms |
| clear | ~1ms | ~1ms | ~1ms | ~1ms | ~10ms |

*Latency includes LLM API calls (if applicable) and storage operations*

### Storage Operations (Latency)

| Operation | File | Redis | MongoDB | PostgreSQL | DynamoDB |
|-----------|------|-------|---------|------------|----------|
| Add message | ~1ms | <1ms | ~5ms | ~3ms | ~10ms |
| Get messages | ~1ms | <1ms | ~5ms | ~3ms | ~10ms |
| Clear | ~1ms | <1ms | ~5ms | ~3ms | ~10ms |

*Latencies for local/same-region deployments*

## Documentation

- **[AI Parts Catalog](../../docs/AI_PARTS_CATALOG.md#memory-systems)** - Comprehensive component reference
- **API Reference** - Generate with `cargo doc --package dashflow-memory --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Implementation Details

- **Lines of Code:** 12,274
- **Test Coverage:** 199 tests
- **Memory Types:** 10 implementations
- **Storage Backends:** 7 implementations (1 always available, 6 feature-gated)
- **Python Compatibility:** Matches `dashflow.memory` and `dashflow_community.chat_message_histories`

## Version

Current version: **1.11**

See [CHANGELOG.md](../../CHANGELOG.md) for version history and migration guides.

## License

This project is licensed under the MIT License - see the [LICENSE](../../LICENSE) file for details.
