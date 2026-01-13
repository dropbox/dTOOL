//! Cassandra/ScyllaDB-based chat message history storage
//!
//! Stores chat message history in Apache Cassandra or ScyllaDB. Messages are serialized to JSON
//! and stored in a table with partition key (session_id) and clustering key (timeuuid) for
//! time-ordered retrieval.
//!
//! # Overview
//!
//! The Cassandra backend uses:
//! - **Cassandra/ScyllaDB** for distributed storage
//! - **Partition key**: session_id for message grouping
//! - **Clustering key**: timeuuid for time-ordered retrieval
//! - **JSON serialization** for message persistence
//! - **Optional TTL** for automatic expiration
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::CassandraChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//! use scylla::SessionBuilder;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create Cassandra session
//! let session = SessionBuilder::new()
//!     .known_node("127.0.0.1:9042")
//!     .build()
//!     .await?;
//!
//! // Create message history
//! let history = CassandraChatMessageHistory::builder()
//!     .session(session)
//!     .keyspace("my_keyspace")
//!     .session_id("session-123")
//!     .build()
//!     .await?;
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
//! Matches `CassandraChatMessageHistory` from
//! `dashflow_community.chat_message_histories.cassandra:33-131`.
//!
//! Key features:
//! - Uses Cassandra/ScyllaDB for distributed storage
//! - Partition by session_id, cluster by timeuuid
//! - Supports optional TTL for automatic expiration
//! - Automatic table creation (unless skip_provisioning=true)

use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use futures::stream::TryStreamExt;
use scylla::frame::value::CqlTimeuuid;
use scylla::{Session, SessionBuilder};
use serde_json;
use std::sync::Arc;

/// Cassandra/ScyllaDB-based chat message history implementation.
///
/// Stores chat messages in a Cassandra or ScyllaDB table, with partition key
/// (session_id) and clustering key (timeuuid) for time-ordered retrieval.
/// Messages are serialized as JSON and stored in the `body_blob` column.
///
/// # Storage Format
///
/// - **Partition Key**: session_id (TEXT)
/// - **Clustering Key**: row_id (TIMEUUID)
/// - **Message Column**: body_blob (TEXT, JSON-serialized Message)
/// - **Ordering**: DESC by timeuuid (newest first in partition)
///
/// # Features
///
/// - **Session-based**: Each session_id gets its own partition
/// - **Time-ordered**: Messages ordered by timeuuid (chronological)
/// - **TTL support**: Optional expiration time for automatic cleanup
/// - **Distributed**: Leverages Cassandra/ScyllaDB scalability
#[derive(Clone)]
pub struct CassandraChatMessageHistory {
    session: Arc<Session>,
    keyspace: String,
    table_name: String,
    session_id: String,
    ttl_seconds: Option<i32>,
}

impl CassandraChatMessageHistory {
    /// Creates a new builder for CassandraChatMessageHistory
    pub fn builder() -> CassandraChatMessageHistoryBuilder {
        CassandraChatMessageHistoryBuilder::default()
    }

    /// Get the full table name with keyspace
    fn full_table_name(&self) -> String {
        format!("{}.{}", self.keyspace, self.table_name)
    }

    /// Create the table if it doesn't exist
    async fn create_table(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                session_id TEXT,
                row_id TIMEUUID,
                body_blob TEXT,
                PRIMARY KEY (session_id, row_id)
            ) WITH CLUSTERING ORDER BY (row_id DESC)",
            self.full_table_name()
        );

        self.session.query_unpaged(create_table_query, &[]).await?;
        Ok(())
    }

    /// Read messages from Cassandra
    async fn read_messages(
        &self,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let query = format!(
            "SELECT body_blob FROM {} WHERE session_id = ?",
            self.full_table_name()
        );

        let rows = self.session.query_iter(query, (&self.session_id,)).await?;

        let mut messages = Vec::new();
        let rows_vec: Vec<_> = rows.try_collect().await?;

        // Rows are returned in DESC order (newest first), so we reverse them
        for row in rows_vec.into_iter().rev() {
            if let Some(Some(body_blob)) = row.columns[0].as_ref().map(|c| c.as_text()) {
                let message: Message = serde_json::from_str(body_blob)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }
}

#[async_trait]
impl BaseChatMessageHistory for CassandraChatMessageHistory {
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for message in messages {
            // Generate timeuuid for this message
            let row_id = CqlTimeuuid::from(uuid::Uuid::new_v4());

            // Serialize message to JSON
            let body_blob = serde_json::to_string(message)?;

            // Insert with optional TTL
            if let Some(ttl) = self.ttl_seconds {
                let query = format!(
                    "INSERT INTO {} (session_id, row_id, body_blob) VALUES (?, ?, ?) USING TTL ?",
                    self.full_table_name()
                );
                self.session
                    .query_unpaged(query, (&self.session_id, row_id, &body_blob, ttl))
                    .await?;
            } else {
                let query = format!(
                    "INSERT INTO {} (session_id, row_id, body_blob) VALUES (?, ?, ?)",
                    self.full_table_name()
                );
                self.session
                    .query_unpaged(query, (&self.session_id, row_id, &body_blob))
                    .await?;
            }
        }

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        self.read_messages().await
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let query = format!(
            "DELETE FROM {} WHERE session_id = ?",
            self.full_table_name()
        );
        self.session
            .query_unpaged(query, (&self.session_id,))
            .await?;
        Ok(())
    }
}

/// Builder for CassandraChatMessageHistory
#[derive(Default)]
pub struct CassandraChatMessageHistoryBuilder {
    session: Option<Arc<Session>>,
    contact_points: Vec<String>,
    keyspace: Option<String>,
    table_name: Option<String>,
    session_id: Option<String>,
    ttl_seconds: Option<i32>,
    skip_provisioning: bool,
}

impl CassandraChatMessageHistoryBuilder {
    /// Set an existing Cassandra session
    pub fn session(mut self, session: Session) -> Self {
        self.session = Some(Arc::new(session));
        self
    }

    /// Set an existing Cassandra session from an Arc (for sharing sessions)
    pub fn shared_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    /// Add a contact point (node address) for creating a new session
    /// Format: "host:port" (e.g., "127.0.0.1:9042")
    pub fn contact_point(mut self, contact_point: impl Into<String>) -> Self {
        self.contact_points.push(contact_point.into());
        self
    }

    /// Set the keyspace name (required)
    pub fn keyspace(mut self, keyspace: impl Into<String>) -> Self {
        self.keyspace = Some(keyspace.into());
        self
    }

    /// Set the table name (default: "message_store")
    pub fn table_name(mut self, table_name: impl Into<String>) -> Self {
        self.table_name = Some(table_name.into());
        self
    }

    /// Set the session ID (required)
    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set TTL in seconds (items will expire after this duration)
    pub fn ttl_seconds(mut self, ttl_seconds: i32) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }

    /// Skip automatic table creation
    pub fn skip_provisioning(mut self, skip: bool) -> Self {
        self.skip_provisioning = skip;
        self
    }

    /// Build the CassandraChatMessageHistory
    pub async fn build(
        self,
    ) -> Result<CassandraChatMessageHistory, Box<dyn std::error::Error + Send + Sync>> {
        let keyspace = self.keyspace.ok_or("keyspace is required")?;
        let session_id = self.session_id.ok_or("session_id is required")?;
        let table_name = self
            .table_name
            .unwrap_or_else(|| "message_store".to_string());

        // Get or create session
        let session = if let Some(session) = self.session {
            session
        } else if !self.contact_points.is_empty() {
            let mut builder = SessionBuilder::new();
            for contact_point in &self.contact_points {
                builder = builder.known_node(contact_point);
            }
            Arc::new(builder.build().await?)
        } else {
            return Err("Either session or contact_points must be provided".into());
        };

        // Use keyspace
        session.use_keyspace(&keyspace, false).await?;

        let history = CassandraChatMessageHistory {
            session,
            keyspace,
            table_name,
            session_id,
            ttl_seconds: self.ttl_seconds,
        };

        // Create table unless skip_provisioning is true
        if !self.skip_provisioning {
            history.create_table().await?;
        }

        Ok(history)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Note: These tests require a Cassandra/ScyllaDB instance
    // To run with Docker: docker run -p 9042:9042 scylladb/scylla
    // Then: cargo test --features cassandra-backend

    async fn create_test_session() -> Result<Session, Box<dyn std::error::Error + Send + Sync>> {
        SessionBuilder::new()
            .known_node("127.0.0.1:9042")
            .build()
            .await
            .map_err(|e| e.into())
    }

    async fn create_test_keyspace(
        session: &Session,
        keyspace: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let query = format!(
            "CREATE KEYSPACE IF NOT EXISTS {} WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
            keyspace
        );
        session.query_unpaged(query, &[]).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_history_basic() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_basic";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-1")
            .build()
            .await
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
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_history_multiple_messages() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_multiple";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-2")
            .build()
            .await
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
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_history_persistence() {
        let session = Arc::new(
            create_test_session()
                .await
                .expect("Failed to create session"),
        );
        let keyspace = "test_dashflow_persistence";
        create_test_keyspace(&session, keyspace).await.unwrap();
        let session_id = "test-session-3";

        // Create first instance and add messages
        {
            let history = CassandraChatMessageHistory::builder()
                .shared_session(Arc::clone(&session))
                .keyspace(keyspace)
                .session_id(session_id)
                .build()
                .await
                .expect("Failed to create history");

            let msg = Message::human("Persistent message");
            history.add_messages(&[msg]).await.unwrap();
        }

        // Create second instance with same session_id
        {
            let history = CassandraChatMessageHistory::builder()
                .shared_session(Arc::clone(&session))
                .keyspace(keyspace)
                .session_id(session_id)
                .build()
                .await
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
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_history_append_messages() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_append";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-4")
            .build()
            .await
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
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_history_with_contact_points() {
        let history = CassandraChatMessageHistory::builder()
            .contact_point("127.0.0.1:9042")
            .keyspace("test_dashflow_contact")
            .session_id("test-session-5")
            .build()
            .await
            .expect("Failed to create history");

        // Should be able to use the history
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Clean up (even if empty)
        history.clear().await.unwrap();
    }

    #[tokio::test]
    async fn test_cassandra_builder_validation() {
        // Should fail without keyspace
        let result = CassandraChatMessageHistory::builder()
            .contact_point("127.0.0.1:9042")
            .session_id("test")
            .build()
            .await;
        assert!(result.is_err());

        // Should fail without session_id
        let result = CassandraChatMessageHistory::builder()
            .contact_point("127.0.0.1:9042")
            .keyspace("test")
            .build()
            .await;
        assert!(result.is_err());

        // Should fail without session or contact_points
        let result = CassandraChatMessageHistory::builder()
            .keyspace("test")
            .session_id("test")
            .build()
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_chat_history_with_unicode() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_unicode";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-unicode")
            .build()
            .await
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
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_chat_history_empty_state() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_empty";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-empty")
            .build()
            .await
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

    #[tokio::test]
    #[ignore = "requires Cassandra/ScyllaDB"]
    async fn test_cassandra_chat_history_serialization_roundtrip() {
        let session = create_test_session()
            .await
            .expect("Failed to create session");
        let keyspace = "test_dashflow_serialization";
        create_test_keyspace(&session, keyspace).await.unwrap();

        let history = CassandraChatMessageHistory::builder()
            .session(session)
            .keyspace(keyspace)
            .session_id("test-session-serialization")
            .build()
            .await
            .expect("Failed to create history");

        // Test messages with special characters that need proper JSON serialization
        let special_messages = vec![
            "Message with\nnewlines\nand\ttabs",
            r#"Message with "quotes" and 'apostrophes'"#,
            "Multi-line message:\nLine 1\nLine 2\nLine 3",
        ];

        for msg in &special_messages {
            history.add_messages(&[Message::human(*msg)]).await.unwrap();
        }

        // Test system message (different message type)
        history
            .add_messages(&[Message::system("System notification")])
            .await
            .unwrap();

        // Verify all messages stored and retrieved correctly
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 4);

        for (i, expected) in special_messages.iter().enumerate() {
            assert_eq!(messages[i].content().as_text(), *expected);
        }

        // Verify system message
        assert_eq!(messages[3].content().as_text(), "System notification");

        // Clean up
        history.clear().await.unwrap();
    }
}
