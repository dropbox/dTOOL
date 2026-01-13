//! Redis-based chat message history storage
//!
//! Stores chat message history in Redis using lists. Messages are serialized to JSON
//! and stored in a Redis list, with optional TTL (time-to-live) for automatic expiration.
//!
//! # Overview
//!
//! The Redis backend uses:
//! - **Redis lists** (LPUSH/LRANGE) for message storage
//! - **JSON serialization** for message persistence
//! - **Session-based keys** (key_prefix + session_id)
//! - **Optional TTL** for automatic expiration
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::RedisChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create Redis history with default settings
//! let history = RedisChatMessageHistory::new(
//!     "session-123".to_string(),
//!     "redis://localhost:6379/0".to_string(),
//!     None, // default key_prefix
//!     None, // no TTL
//! ).await?;
//!
//! // Add messages
//! history.add_user_message("Hello!").await?;
//! history.add_ai_message("Hi there!").await?;
//!
//! // Retrieve messages
//! let messages = history.get_messages().await?;
//! assert_eq!(messages.len(), 2);
//!
//! // Clear history
//! history.clear().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Python Baseline Compatibility
//!
//! Matches `RedisChatMessageHistory` from
//! `dashflow_community.chat_message_histories.redis:17-121`.
//!
//! Key features:
//! - Uses Redis lists (LPUSH for append, LRANGE for retrieval)
//! - Stores messages as JSON with message_to_dict serialization
//! - Supports optional TTL for automatic expiration
//! - Configurable key prefix for namespace isolation

use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client, RedisError};
use serde_json;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Redis-based chat message history implementation.
///
/// Stores chat messages in a Redis list, with each message serialized as JSON.
/// Messages are pushed to the list using LPUSH and retrieved in chronological order.
///
/// # Storage Format
///
/// - **Key**: `{key_prefix}{session_id}` (default prefix: "message_store:")
/// - **Structure**: Redis list
/// - **Message format**: JSON-serialized Message objects
/// - **Order**: Chronological (oldest first when retrieved)
///
/// # Features
///
/// - **Session-based**: Each session_id gets its own Redis key
/// - **TTL support**: Optional expiration time for automatic cleanup
/// - **JSON serialization**: Uses serde_json for Message serialization
/// - **Thread-safe**: Uses Arc<RwLock<>> for safe concurrent access
///
/// # Python Baseline Compatibility
///
/// Matches Python implementation in `dashflow_community/chat_message_histories/redis.py:17-121`.
///
/// Differences:
/// - Rust uses async/await (Python has sync with optional async)
/// - Connection is established in constructor (async)
/// - Thread-safe by design (Arc<RwLock<>>)
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use dashflow_memory::RedisChatMessageHistory;
/// use dashflow::core::chat_history::BaseChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let history = RedisChatMessageHistory::new(
///     "user-123-session".to_string(),
///     "redis://localhost:6379/0".to_string(),
///     None,
///     None,
/// ).await?;
///
/// history.add_user_message("What is Rust?").await?;
/// history.add_ai_message("Rust is a systems programming language.").await?;
///
/// let messages = history.get_messages().await?;
/// assert_eq!(messages.len(), 2);
/// # Ok(())
/// # }
/// ```
///
/// ## With TTL and Custom Key Prefix
///
/// ```rust,ignore
/// use dashflow_memory::RedisChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// // Messages expire after 3600 seconds (1 hour)
/// let history = RedisChatMessageHistory::new(
///     "session-456".to_string(),
///     "redis://localhost:6379/0".to_string(),
///     Some("chat:history:".to_string()),
///     Some(3600),
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct RedisChatMessageHistory {
    session_id: String,
    key_prefix: String,
    ttl: Option<u64>,
    connection: Arc<RwLock<MultiplexedConnection>>,
}

impl RedisChatMessageHistory {
    /// Create a new Redis chat message history.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Unique identifier for this chat session
    /// * `url` - Redis connection URL (e.g., "redis://localhost:6379/0")
    /// * `key_prefix` - Optional key prefix (default: "message_store:")
    /// * `ttl` - Optional time-to-live in seconds for automatic expiration
    ///
    /// # Returns
    ///
    /// Returns the history instance or a connection error.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Redis connection fails
    /// - URL is malformed
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `__init__` in Python baseline (line 56-90).
    pub async fn new(
        session_id: String,
        url: String,
        key_prefix: Option<String>,
        ttl: Option<u64>,
    ) -> Result<Self, RedisError> {
        let client = Client::open(url)?;
        let connection = client.get_multiplexed_async_connection().await?;

        Ok(Self {
            session_id,
            key_prefix: key_prefix.unwrap_or_else(|| "message_store:".to_string()),
            ttl,
            connection: Arc::new(RwLock::new(connection)),
        })
    }

    /// Get the Redis key for this session.
    ///
    /// # Returns
    ///
    /// The full key: `{key_prefix}{session_id}`
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `key` property in Python baseline (line 92-95).
    fn key(&self) -> String {
        format!("{}{}", self.key_prefix, self.session_id)
    }
}

#[async_trait]
impl BaseChatMessageHistory for RedisChatMessageHistory {
    /// Get all messages from Redis.
    ///
    /// Retrieves all messages for this session from Redis using LRANGE.
    /// Messages are stored in reverse order in Redis (LPUSH appends to left),
    /// so we reverse them to return chronological order.
    ///
    /// # Returns
    ///
    /// List of messages in chronological order (oldest first).
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Redis connection fails
    /// - Message deserialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `messages` property in Python baseline (line 97-103).
    /// Python uses `lrange(key, 0, -1)` and reverses the list.
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.connection.write().await;
        let key = self.key();

        // Get all messages from Redis list (0 to -1 means all)
        let raw_messages: Vec<String> = conn.lrange(&key, 0, -1).await?;

        // Messages are stored in reverse order (LPUSH adds to front)
        // so we need to reverse them to get chronological order
        let messages: Result<Vec<Message>, _> = raw_messages
            .iter()
            .rev()
            .map(|s| serde_json::from_str(s))
            .collect();

        Ok(messages?)
    }

    /// Add multiple messages to Redis.
    ///
    /// Appends messages to the Redis list using LPUSH (left push).
    /// If TTL is configured, updates the expiration time.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to add
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Redis connection fails
    /// - Message serialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_message` in Python baseline (line 112-116).
    /// Python adds one at a time; we batch them for efficiency.
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.connection.write().await;
        let key = self.key();

        // Serialize and push each message
        for message in messages {
            let json = serde_json::to_string(message)?;
            conn.lpush::<_, _, ()>(&key, json).await?;
        }

        // Update TTL if configured
        if let Some(ttl) = self.ttl {
            conn.expire::<_, ()>(&key, ttl as i64).await?;
        }

        Ok(())
    }

    /// Clear all messages for this session from Redis.
    ///
    /// Deletes the Redis key for this session, removing all messages.
    ///
    /// # Errors
    ///
    /// Returns error if Redis connection fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear` in Python baseline (line 118-120).
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.connection.write().await;
        let key = self.key();
        conn.del::<_, ()>(&key).await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Helper function to create a test Redis connection
    async fn create_test_history(session_id: &str) -> RedisChatMessageHistory {
        RedisChatMessageHistory::new(
            session_id.to_string(),
            "redis://localhost:6379/0".to_string(),
            Some("test:".to_string()),
            None,
        )
        .await
        .expect("Redis must be running on localhost to run ignored tests")
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_basic() {
        let history = create_test_history("test-basic").await;

        // Clear any existing data
        history.clear().await.unwrap();

        // Add messages
        history.add_user_message("Hello").await.unwrap();
        history.add_ai_message("Hi there!").await.unwrap();

        // Get messages
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Hello"),
            _ => panic!("Expected Human message"),
        }

        match &messages[1] {
            Message::AI { content, .. } => assert_eq!(content.as_text(), "Hi there!"),
            _ => panic!("Expected AI message"),
        }

        // Cleanup
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_add_messages() {
        let history = create_test_history("test-batch").await;
        history.clear().await.unwrap();

        let messages = vec![
            Message::human("Message 1"),
            Message::ai("Response 1"),
            Message::human("Message 2"),
        ];

        // Add all messages at once
        history.add_messages(&messages).await.unwrap();

        // Should have 3 messages
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 3);

        // Verify order is preserved
        match &retrieved[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 1"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_clear() {
        let history = create_test_history("test-clear").await;
        history.clear().await.unwrap();

        // Add some messages
        history.add_user_message("Message 1").await.unwrap();
        history.add_ai_message("Response 1").await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        // Clear
        history.clear().await.unwrap();

        // Should be empty
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_multiple_sessions() {
        let history1 = create_test_history("session-1").await;
        let history2 = create_test_history("session-2").await;

        history1.clear().await.unwrap();
        history2.clear().await.unwrap();

        // Add messages to session 1
        history1
            .add_user_message("Session 1 message")
            .await
            .unwrap();

        // Add messages to session 2
        history2
            .add_user_message("Session 2 message")
            .await
            .unwrap();

        // Each session should only see its own messages
        let messages1 = history1.get_messages().await.unwrap();
        let messages2 = history2.get_messages().await.unwrap();

        assert_eq!(messages1.len(), 1);
        assert_eq!(messages2.len(), 1);

        match &messages1[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Session 1 message"),
            _ => panic!("Expected Human message"),
        }

        match &messages2[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Session 2 message"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        history1.clear().await.unwrap();
        history2.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_with_ttl() {
        let history = RedisChatMessageHistory::new(
            "test-ttl".to_string(),
            "redis://localhost:6379/0".to_string(),
            Some("test:".to_string()),
            Some(60), // 60 seconds TTL
        )
        .await
        .expect("Redis must be running on localhost to run ignored tests");

        history.clear().await.unwrap();

        // Add message with TTL
        history.add_user_message("This will expire").await.unwrap();

        // Should be able to retrieve immediately
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        // Note: We don't wait 60 seconds to test expiration in unit tests
        // That would be better suited for integration tests

        // Cleanup
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_with_unicode() {
        let history = create_test_history("test-unicode").await;
        history.clear().await.unwrap();

        // Test various Unicode characters
        let unicode_messages = vec![
            "Hello 世界",       // Chinese
            "Привет мир",       // Russian
            "مرحبا بالعالم",    // Arabic
            "🚀 Emoji test 🎉", // Emojis
            "Math: ∑ ∫ ∂",      // Mathematical symbols
        ];

        for msg in &unicode_messages {
            history.add_user_message(*msg).await.unwrap();
        }

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), unicode_messages.len());

        for (i, msg) in messages.iter().enumerate() {
            match msg {
                Message::Human { content, .. } => {
                    assert_eq!(content.as_text(), unicode_messages[i]);
                }
                _ => panic!("Expected Human message"),
            }
        }

        // Cleanup
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_empty_state() {
        let history = create_test_history("test-empty").await;
        history.clear().await.unwrap();

        // Test reading from empty state
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Test multiple reads from empty state
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Test clear on empty state (should not error)
        history.clear().await.unwrap();

        // Add message and verify
        history.add_user_message("Test").await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        // Cleanup
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_with_prefix() {
        // Test with custom prefix
        let history = RedisChatMessageHistory::new(
            "test-prefix-session".to_string(),
            "redis://localhost:6379/0".to_string(),
            Some("custom_prefix:".to_string()),
            None,
        )
        .await
        .expect("Redis must be running on localhost to run ignored tests");

        history.clear().await.unwrap();

        // Add message
        history.add_user_message("Prefix test").await.unwrap();

        // Verify message is stored
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Prefix test"),
            _ => panic!("Expected Human message"),
        }

        // Test that different prefixes are isolated
        let history2 = RedisChatMessageHistory::new(
            "test-prefix-session".to_string(),
            "redis://localhost:6379/0".to_string(),
            Some("different_prefix:".to_string()),
            None,
        )
        .await
        .expect("Redis must be running on localhost to run ignored tests");

        // Should be empty (different prefix)
        let messages2 = history2.get_messages().await.unwrap();
        assert_eq!(messages2.len(), 0);

        // Cleanup
        history.clear().await.unwrap();
        history2.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis running on localhost"]
    async fn test_redis_chat_history_serialization_roundtrip() {
        let history = create_test_history("test-serialization").await;
        history.clear().await.unwrap();

        // Create messages with various content types
        let messages = vec![
            Message::human("Simple text"),
            Message::ai("Response with special chars: \n\t\"quotes\""),
            Message::system("System message"),
            Message::human("Multi-line\nmessage\ntest"),
        ];

        // Add messages
        history.add_messages(&messages).await.unwrap();

        // Retrieve and verify exact match
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), messages.len());

        for (i, (original, retrieved)) in messages.iter().zip(retrieved.iter()).enumerate() {
            match (original, retrieved) {
                (Message::Human { content: c1, .. }, Message::Human { content: c2, .. }) => {
                    assert_eq!(c1.as_text(), c2.as_text(), "Message {} mismatch", i);
                }
                (Message::AI { content: c1, .. }, Message::AI { content: c2, .. }) => {
                    assert_eq!(c1.as_text(), c2.as_text(), "Message {} mismatch", i);
                }
                (Message::System { content: c1, .. }, Message::System { content: c2, .. }) => {
                    assert_eq!(c1.as_text(), c2.as_text(), "Message {} mismatch", i);
                }
                _ => panic!("Message type mismatch at index {}", i),
            }
        }

        // Cleanup
        history.clear().await.unwrap();
    }
}
