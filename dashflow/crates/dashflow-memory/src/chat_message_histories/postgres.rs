//! PostgreSQL-based chat message history storage
//!
//! Stores chat message history in PostgreSQL using tables. Messages are serialized to JSON
//! and stored in a JSONB column with a session_id field for filtering.
//!
//! # Overview
//!
//! The PostgreSQL backend uses:
//! - **PostgreSQL tables** for message storage
//! - **JSONB column** for efficient JSON storage and querying
//! - **Session-based filtering** (session_id column)
//! - **Automatic table creation** with proper schema
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::PostgresChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create PostgreSQL history
//! let history = PostgresChatMessageHistory::new(
//!     "session-123".to_string(),
//!     "postgresql://postgres:password@localhost/chat_history".to_string(),
//!     None, // default table name
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
//! Matches `PostgresChatMessageHistory` from
//! `dashflow_community.chat_message_histories.postgres:30-101`.
//!
//! Key features:
//! - Uses JSONB column for efficient JSON storage
//! - Automatic table creation with proper schema
//! - Session-based message filtering
//! - Ordered by insertion (id column)

use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};
use tracing;

const DEFAULT_TABLE_NAME: &str = "message_store";

/// PostgreSQL-based chat message history implementation.
///
/// Stores chat messages in a PostgreSQL table with JSONB column for messages.
/// Messages are filtered by session_id and retrieved in insertion order.
///
/// # Table Schema
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS {table_name} (
///     id SERIAL PRIMARY KEY,
///     session_id TEXT NOT NULL,
///     message JSONB NOT NULL
/// );
/// ```
///
/// # Features
///
/// - **Session-based**: Messages filtered by session_id column
/// - **JSONB storage**: Efficient JSON storage with query capabilities
/// - **Auto table creation**: Creates table if it doesn't exist
/// - **Thread-safe**: Uses Arc<Mutex<>> for safe concurrent access
///
/// # Python Baseline Compatibility
///
/// Matches Python implementation in `dashflow_community/chat_message_histories/postgres.py:30-101`.
///
/// Differences:
/// - Rust uses async/await throughout
/// - Uses tokio-postgres instead of psycopg
/// - Connection is Arc<Mutex<>> wrapped for thread safety
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use dashflow_memory::PostgresChatMessageHistory;
/// use dashflow::core::chat_history::BaseChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let history = PostgresChatMessageHistory::new(
///     "user-123-session".to_string(),
///     "postgresql://localhost/chatdb".to_string(),
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
/// ## Custom Table Name
///
/// ```rust,ignore
/// use dashflow_memory::PostgresChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let history = PostgresChatMessageHistory::new(
///     "session-456".to_string(),
///     "postgresql://localhost/chatdb".to_string(),
///     Some("conversations".to_string()),
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct PostgresChatMessageHistory {
    session_id: String,
    table_name: String,
    client: Arc<Mutex<Client>>,
}

impl PostgresChatMessageHistory {
    /// Create a new PostgreSQL chat message history.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Unique identifier for this chat session
    /// * `connection_string` - PostgreSQL connection string
    /// * `table_name` - Optional table name (default: "message_store")
    ///
    /// # Returns
    ///
    /// Returns the history instance or a connection error.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - PostgreSQL connection fails
    /// - Connection string is malformed
    /// - Table creation fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `__init__` in Python baseline (line 38-56).
    pub async fn new(
        session_id: String,
        connection_string: String,
        table_name: Option<String>,
    ) -> Result<Self, tokio_postgres::Error> {
        let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;

        // Spawn the connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(error = %e, "PostgreSQL connection error");
            }
        });

        let table_name = table_name.unwrap_or_else(|| DEFAULT_TABLE_NAME.to_string());

        let history = Self {
            session_id,
            table_name: table_name.clone(),
            client: Arc::new(Mutex::new(client)),
        };

        // Create table if not exists
        history.create_table_if_not_exists().await?;

        Ok(history)
    }

    /// Create the message_store table if it doesn't exist.
    ///
    /// Creates a table with schema:
    /// - id: SERIAL PRIMARY KEY (auto-incrementing)
    /// - session_id: TEXT (for filtering by session)
    /// - message: JSONB (for storing serialized messages)
    ///
    /// # Errors
    ///
    /// Returns error if table creation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `_create_table_if_not_exists` in Python baseline (line 58-65).
    async fn create_table_if_not_exists(&self) -> Result<(), tokio_postgres::Error> {
        let client = self.client.lock().await;

        let query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id SERIAL PRIMARY KEY,
                session_id TEXT NOT NULL,
                message JSONB NOT NULL
            );",
            self.table_name
        );

        client.execute(&query, &[]).await?;

        Ok(())
    }
}

#[async_trait]
impl BaseChatMessageHistory for PostgresChatMessageHistory {
    /// Get all messages from PostgreSQL.
    ///
    /// Retrieves all messages for this session by querying rows
    /// with matching session_id, ordered by id (insertion order).
    ///
    /// # Returns
    ///
    /// List of messages in chronological order (oldest first).
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - PostgreSQL connection fails
    /// - Query fails
    /// - Message deserialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `messages` property in Python baseline (line 67-76).
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = format!(
            "SELECT message FROM {} WHERE session_id = $1 ORDER BY id;",
            self.table_name
        );

        let rows = client.query(&query, &[&self.session_id]).await?;

        let messages: Result<Vec<Message>, _> = rows
            .iter()
            .map(|row| {
                let json_value: serde_json::Value = row.get(0);
                serde_json::from_value(json_value)
            })
            .collect();

        Ok(messages?)
    }

    /// Add multiple messages to PostgreSQL.
    ///
    /// Inserts each message as a separate row with session_id and message JSONB.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to add
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - PostgreSQL connection fails
    /// - Insert operation fails
    /// - Message serialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_message` in Python baseline (line 78-88).
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = format!(
            "INSERT INTO {} (session_id, message) VALUES ($1, $2);",
            self.table_name
        );

        for message in messages {
            let message_json = serde_json::to_value(message)?;
            client
                .execute(&query, &[&self.session_id, &message_json])
                .await?;
        }

        Ok(())
    }

    /// Clear all messages for this session from PostgreSQL.
    ///
    /// Deletes all rows with matching session_id.
    ///
    /// # Errors
    ///
    /// Returns error if PostgreSQL delete operation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear` in Python baseline (line 90-93).
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = format!("DELETE FROM {} WHERE session_id = $1;", self.table_name);

        client.execute(&query, &[&self.session_id]).await?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Helper function to create a test PostgreSQL connection
    async fn create_test_history(session_id: &str) -> PostgresChatMessageHistory {
        PostgresChatMessageHistory::new(
            session_id.to_string(),
            "postgresql://postgres:postgres@localhost/test_chat_history".to_string(),
            Some("test_messages".to_string()),
        )
        .await
        .expect("PostgreSQL must be running on localhost to run ignored tests")
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_basic() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_add_messages() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_clear() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_multiple_sessions() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_with_unicode() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_empty_state() {
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
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_persistence() {
        let session_id = "test-persistence";

        // Create first history instance
        let history1 = create_test_history(session_id).await;
        history1.clear().await.unwrap();

        // Add messages with first instance
        history1
            .add_user_message("Persistent message 1")
            .await
            .unwrap();
        history1
            .add_ai_message("Persistent response 1")
            .await
            .unwrap();

        // Drop first instance
        drop(history1);

        // Create second instance with same session ID
        let history2 = create_test_history(session_id).await;

        // Should still have messages from first instance
        let messages = history2.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Persistent message 1"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        history2.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_serialization_roundtrip() {
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

    #[tokio::test]
    #[ignore = "requires PostgreSQL running on localhost"]
    async fn test_postgres_chat_history_large_batch() {
        let history = create_test_history("test-large-batch").await;
        history.clear().await.unwrap();

        // Add 100 messages in bulk
        let mut messages = Vec::new();
        for i in 0..100 {
            messages.push(Message::human(format!("Message {}", i)));
        }

        history.add_messages(&messages).await.unwrap();

        // Verify all messages stored
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 100);

        // Verify first and last message
        match &retrieved[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 0"),
            _ => panic!("Expected Human message"),
        }

        match &retrieved[99] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 99"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        history.clear().await.unwrap();
    }
}
