//! Upstash Redis-based chat message history storage
//!
//! Stores chat message history in Upstash Redis using REST API. Messages are serialized to JSON
//! and stored in a Redis list, with optional TTL (time-to-live) for automatic expiration.
//!
//! # Overview
//!
//! The Upstash Redis backend uses:
//! - **REST API** for communication (not Redis protocol)
//! - **Redis lists** (LPUSH/LRANGE) for message storage
//! - **JSON serialization** for message persistence
//! - **Session-based keys** (key_prefix + session_id)
//! - **Optional TTL** for automatic expiration
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::UpstashRedisChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create Upstash Redis history
//! let history = UpstashRedisChatMessageHistory::new(
//!     "session-123".to_string(),
//!     "https://example.upstash.io".to_string(),
//!     "your-token".to_string(),
//!     Some("message_store:".to_string()),
//!     None, // no TTL
//! )?;
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
//! Matches `UpstashRedisChatMessageHistory` from
//! `dashflow_community.chat_message_histories.upstash_redis:15-70`.
//!
//! Key features:
//! - Uses Upstash Redis REST API (not Redis protocol)
//! - Stores messages in Redis lists (LPUSH/LRANGE)
//! - Supports optional TTL for automatic expiration
//! - Configurable key prefix for namespace isolation

use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use reqwest::Client;
use serde_json;

/// Upstash Redis-based chat message history implementation.
///
/// Stores chat messages in Upstash Redis using the REST API, with each message
/// serialized as JSON. Messages are pushed to the list using LPUSH and retrieved
/// in chronological order.
///
/// # Storage Format
///
/// - **Key**: `{key_prefix}{session_id}` (default prefix: "message_store:")
/// - **Structure**: Redis list
/// - **Message format**: JSON-serialized Message objects
/// - **Order**: Chronological (oldest first when retrieved)
/// - **Protocol**: REST API (not Redis protocol)
///
/// # Features
///
/// - **Session-based**: Each session_id gets its own Redis key
/// - **TTL support**: Optional expiration time for automatic cleanup
/// - **JSON serialization**: Uses serde_json for Message serialization
/// - **REST API**: Uses HTTP for communication with Upstash
#[derive(Clone)]
pub struct UpstashRedisChatMessageHistory {
    client: Client,
    session_id: String,
    url: String,
    token: String,
    key_prefix: String,
    ttl: Option<u64>,
}

impl UpstashRedisChatMessageHistory {
    /// Creates a new Upstash Redis chat message history.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Unique identifier for the conversation session
    /// * `url` - Upstash Redis REST URL (e.g., <https://example.upstash.io>)
    /// * `token` - Upstash Redis REST token
    /// * `key_prefix` - Optional prefix for Redis keys (default: "message_store:")
    /// * `ttl` - Optional time-to-live in seconds for automatic expiration
    ///
    /// # Errors
    ///
    /// Returns an error if url or token is empty.
    pub fn new(
        session_id: String,
        url: String,
        token: String,
        key_prefix: Option<String>,
        ttl: Option<u64>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if url.is_empty() || token.is_empty() {
            return Err("UPSTASH_REDIS_REST_URL and UPSTASH_REDIS_REST_TOKEN are required".into());
        }

        Ok(Self {
            client: Client::new(),
            session_id,
            url,
            token,
            key_prefix: key_prefix.unwrap_or_else(|| "message_store:".to_string()),
            ttl,
        })
    }

    /// Get the full Redis key for this session
    fn key(&self) -> String {
        format!("{}{}", self.key_prefix, self.session_id)
    }

    /// Execute a Redis command via the REST API
    async fn execute_command(
        &self,
        command: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .post(&self.url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&command)
            .send()
            .await?;

        let result = response.json::<serde_json::Value>().await?;

        // Check for error in response
        if let Some(error) = result.get("error") {
            return Err(format!("Upstash Redis error: {}", error).into());
        }

        Ok(result)
    }

    /// Read messages from Upstash Redis
    async fn read_messages(
        &self,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        // LRANGE key 0 -1 returns all elements
        let command = serde_json::json!(["LRANGE", self.key(), "0", "-1"]);
        let result = self.execute_command(vec![command]).await?;

        // Parse the result array
        let items = match result.get("result") {
            Some(serde_json::Value::Array(arr)) => arr,
            _ => return Ok(Vec::new()),
        };

        // Items are in reverse order (newest first), so we reverse them
        let mut messages = Vec::new();
        for item in items.iter().rev() {
            if let serde_json::Value::String(json_str) = item {
                let message: Message = serde_json::from_str(json_str)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }
}

#[async_trait]
impl BaseChatMessageHistory for UpstashRedisChatMessageHistory {
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for message in messages {
            // Serialize message to JSON
            let json_str = serde_json::to_string(message)?;

            // LPUSH key value
            let lpush_command = serde_json::json!(["LPUSH", self.key(), json_str]);
            self.execute_command(vec![lpush_command]).await?;

            // Set TTL if configured
            if let Some(ttl_seconds) = self.ttl {
                let expire_command = serde_json::json!(["EXPIRE", self.key(), ttl_seconds]);
                self.execute_command(vec![expire_command]).await?;
            }
        }

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        self.read_messages().await
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // DEL key
        let command = serde_json::json!(["DEL", self.key()]);
        self.execute_command(vec![command]).await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Note: These tests require an Upstash Redis instance
    // Set UPSTASH_REDIS_REST_URL and UPSTASH_REDIS_REST_TOKEN environment variables
    // To run: UPSTASH_REDIS_REST_URL=https://... UPSTASH_REDIS_REST_TOKEN=... cargo test --features upstash-backend

    fn get_test_credentials() -> Option<(String, String)> {
        let url = std::env::var("UPSTASH_REDIS_REST_URL").ok()?;
        let token = std::env::var("UPSTASH_REDIS_REST_TOKEN").ok()?;
        Some((url, token))
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_history_basic() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-basic-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
            .expect("Failed to create history");

        // Initially empty
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Add a message
        let msg = Message::human("Hello!");
        history
            .add_messages(std::slice::from_ref(&msg))
            .await
            .unwrap();

        // Verify it was added
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content().as_text(), "Hello!");

        // Clean up
        history.clear().await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_history_multiple_messages() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-multiple-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
            .expect("Failed to create history");

        // Add multiple messages
        let messages = vec![
            Message::human("Hello!"),
            Message::ai("Hi there!"),
            Message::human("How are you?"),
        ];
        history.add_messages(&messages).await.unwrap();

        // Verify all messages
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 3);
        assert_eq!(retrieved[0].content().as_text(), "Hello!");
        assert_eq!(retrieved[1].content().as_text(), "Hi there!");
        assert_eq!(retrieved[2].content().as_text(), "How are you?");

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_history_persistence() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-persistence-{}", uuid::Uuid::new_v4());

        // Create first instance and add messages
        {
            let history = UpstashRedisChatMessageHistory::new(
                session_id.clone(),
                url.clone(),
                token.clone(),
                None,
                None,
            )
            .expect("Failed to create history");

            let msg = Message::human("Persistent message");
            history.add_messages(&[msg]).await.unwrap();
        }

        // Create second instance with same session_id
        {
            let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
                .expect("Failed to create history");

            // Should retrieve the message from first instance
            let messages = history.get_messages().await.unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].content().as_text(), "Persistent message");

            // Clean up
            history.clear().await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_history_custom_prefix() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-prefix-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(
            session_id.clone(),
            url,
            token,
            Some("custom_prefix:".to_string()),
            None,
        )
        .expect("Failed to create history");

        // Verify key is constructed correctly
        assert_eq!(history.key(), format!("custom_prefix:{}", session_id));

        // Add and retrieve message
        let msg = Message::human("Test message");
        history.add_messages(&[msg]).await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_history_append_messages() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-append-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
            .expect("Failed to create history");

        // Add first message
        history
            .add_messages(&[Message::human("First")])
            .await
            .unwrap();

        // Add second message
        history
            .add_messages(&[Message::ai("Second")])
            .await
            .unwrap();

        // Both should be present
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content().as_text(), "First");
        assert_eq!(messages[1].content().as_text(), "Second");

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    async fn test_upstash_validation() {
        // Should fail with empty URL
        let result = UpstashRedisChatMessageHistory::new(
            "test".to_string(),
            "".to_string(),
            "token".to_string(),
            None,
            None,
        );
        assert!(result.is_err());

        // Should fail with empty token
        let result = UpstashRedisChatMessageHistory::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "".to_string(),
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_key_construction() {
        let history = UpstashRedisChatMessageHistory::new(
            "session-123".to_string(),
            "https://example.com".to_string(),
            "token".to_string(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(history.key(), "message_store:session-123");

        let history_custom = UpstashRedisChatMessageHistory::new(
            "session-456".to_string(),
            "https://example.com".to_string(),
            "token".to_string(),
            Some("custom:".to_string()),
            None,
        )
        .unwrap();

        assert_eq!(history_custom.key(), "custom:session-456");
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_chat_history_with_unicode() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-unicode-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
            .expect("Failed to create history");

        // Test unicode messages: Chinese, Russian, Arabic, emojis, mathematical symbols
        let unicode_messages = vec![
            "你好世界",       // Chinese
            "Здравствуй мир", // Russian
            "مرحبا بالعالم",  // Arabic
            "🌍👋🎉💬",       // Emojis
            "∑∫∂√∞≈≠",        // Mathematical symbols
        ];

        for msg in &unicode_messages {
            history.add_messages(&[Message::human(*msg)]).await.unwrap();
        }

        // Verify all unicode messages stored correctly
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 5);

        for (i, expected) in unicode_messages.iter().enumerate() {
            assert_eq!(messages[i].content().as_text(), *expected);
        }

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Upstash Redis credentials"]
    async fn test_upstash_chat_history_empty_state() {
        let (url, token) = get_test_credentials().expect("Missing Upstash credentials");
        let session_id = format!("test-session-empty-{}", uuid::Uuid::new_v4());

        let history = UpstashRedisChatMessageHistory::new(session_id, url, token, None, None)
            .expect("Failed to create history");

        // Test 1: Multiple reads from empty state
        let messages1 = history.get_messages().await.unwrap();
        assert_eq!(messages1.len(), 0);

        let messages2 = history.get_messages().await.unwrap();
        assert_eq!(messages2.len(), 0);

        // Test 2: Clear on empty state should succeed
        history.clear().await.unwrap();

        // Test 3: Add after empty state verification
        history
            .add_messages(&[Message::human("First message")])
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content().as_text(), "First message");

        // Clean up
        history.clear().await.unwrap();
    }
}
