//! MongoDB-based chat message history storage
//!
//! Stores chat message history in MongoDB using collections. Messages are serialized to JSON
//! and stored as documents with a session_id field for filtering.
//!
//! # Overview
//!
//! The MongoDB backend uses:
//! - **MongoDB collections** for message storage
//! - **JSON serialization** for message persistence
//! - **Session-based filtering** (SessionId field)
//! - **Automatic indexing** on SessionId for performance
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::MongoDBChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create MongoDB history
//! let history = MongoDBChatMessageHistory::new(
//!     "mongodb://localhost:27017".to_string(),
//!     "session-123".to_string(),
//!     None, // default database name
//!     None, // default collection name
//!     true, // create index
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
//! Matches `MongoDBChatMessageHistory` from
//! `dashflow_community.chat_message_histories.mongodb:24-102`.
//!
//! Key features:
//! - Stores each message as a separate document
//! - Uses SessionId field for filtering messages by session
//! - Creates index on SessionId for query performance
//! - JSON serialization with message_to_dict format

use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use mongodb::bson::{doc, Document};
use mongodb::options::IndexOptions;
use mongodb::{Client, Collection, IndexModel};
use std::sync::Arc;

const DEFAULT_DBNAME: &str = "chat_history";
const DEFAULT_COLLECTION_NAME: &str = "message_store";

/// MongoDB-based chat message history implementation.
///
/// Stores chat messages in a MongoDB collection, with each message as a separate document.
/// Messages are filtered by session_id and retrieved in insertion order.
///
/// # Storage Format
///
/// - **Database**: Configurable (default: "chat_history")
/// - **Collection**: Configurable (default: "message_store")
/// - **Document fields**:
///   - `SessionId`: String (indexed)
///   - `History`: JSON-serialized Message object
///
/// # Features
///
/// - **Session-based**: Messages filtered by SessionId field
/// - **Automatic indexing**: Creates index on SessionId for performance
/// - **JSON serialization**: Uses serde_json for Message serialization
/// - **Thread-safe**: Uses Arc for safe concurrent access
///
/// # Python Baseline Compatibility
///
/// Matches Python implementation in `dashflow_community/chat_message_histories/mongodb.py:24-102`.
///
/// Differences:
/// - Rust uses async/await throughout
/// - Connection is established in constructor
/// - Uses mongodb async driver instead of pymongo
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use dashflow_memory::MongoDBChatMessageHistory;
/// use dashflow::core::chat_history::BaseChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let history = MongoDBChatMessageHistory::new(
///     "mongodb://localhost:27017".to_string(),
///     "user-123-session".to_string(),
///     None,
///     None,
///     true,
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
/// ## Custom Database and Collection
///
/// ```rust,ignore
/// use dashflow_memory::MongoDBChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let history = MongoDBChatMessageHistory::new(
///     "mongodb://localhost:27017".to_string(),
///     "session-456".to_string(),
///     Some("my_app".to_string()),
///     Some("conversations".to_string()),
///     true,
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MongoDBChatMessageHistory {
    session_id: String,
    collection: Arc<Collection<Document>>,
}

impl MongoDBChatMessageHistory {
    /// Create a new MongoDB chat message history.
    ///
    /// # Arguments
    ///
    /// * `connection_string` - MongoDB connection string
    /// * `session_id` - Unique identifier for this chat session
    /// * `database_name` - Optional database name (default: "chat_history")
    /// * `collection_name` - Optional collection name (default: "message_store")
    /// * `create_index` - Whether to create an index on SessionId (default: true)
    ///
    /// # Returns
    ///
    /// Returns the history instance or a connection error.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - MongoDB connection fails
    /// - Connection string is malformed
    /// - Index creation fails (if create_index is true)
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `__init__` in Python baseline (line 37-60).
    pub async fn new(
        connection_string: String,
        session_id: String,
        database_name: Option<String>,
        collection_name: Option<String>,
        create_index: bool,
    ) -> Result<Self, mongodb::error::Error> {
        let client = Client::with_uri_str(&connection_string).await?;
        let db_name = database_name.unwrap_or_else(|| DEFAULT_DBNAME.to_string());
        let coll_name = collection_name.unwrap_or_else(|| DEFAULT_COLLECTION_NAME.to_string());

        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&coll_name);

        // Create index on SessionId if requested
        if create_index {
            let index = IndexModel::builder()
                .keys(doc! { "SessionId": 1 })
                .options(IndexOptions::builder().build())
                .build();

            collection.create_index(index).await?;
        }

        Ok(Self {
            session_id,
            collection: Arc::new(collection),
        })
    }
}

#[async_trait]
impl BaseChatMessageHistory for MongoDBChatMessageHistory {
    /// Get all messages from MongoDB.
    ///
    /// Retrieves all messages for this session by querying documents
    /// with matching SessionId. Messages are returned in insertion order.
    ///
    /// # Returns
    ///
    /// List of messages in chronological order (oldest first).
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - MongoDB connection fails
    /// - Query fails
    /// - Message deserialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `messages` property in Python baseline (line 62-78).
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let filter = doc! { "SessionId": &self.session_id };

        let mut cursor = self.collection.find(filter).await?;
        let mut messages = Vec::new();

        // Use the cursor as an async stream
        use futures::stream::StreamExt;

        while let Some(result) = cursor.next().await {
            let document = result?;

            // Extract the History field which contains the JSON message
            if let Some(history_value) = document.get("History") {
                if let Some(history_str) = history_value.as_str() {
                    let message: Message = serde_json::from_str(history_str)?;
                    messages.push(message);
                }
            }
        }

        Ok(messages)
    }

    /// Add multiple messages to MongoDB.
    ///
    /// Inserts each message as a separate document with SessionId and History fields.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to add
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - MongoDB connection fails
    /// - Insert operation fails
    /// - Message serialization fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_message` in Python baseline (line 80-92).
    /// Python adds one at a time; we do the same for consistency.
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for message in messages {
            let history_json = serde_json::to_string(message)?;

            let document = doc! {
                "SessionId": &self.session_id,
                "History": history_json,
            };

            self.collection.insert_one(document).await?;
        }

        Ok(())
    }

    /// Clear all messages for this session from MongoDB.
    ///
    /// Deletes all documents with matching SessionId.
    ///
    /// # Errors
    ///
    /// Returns error if MongoDB delete operation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear` in Python baseline (line 94-100).
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let filter = doc! { "SessionId": &self.session_id };
        self.collection.delete_many(filter).await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Helper function to create a test MongoDB connection
    async fn create_test_history(session_id: &str) -> MongoDBChatMessageHistory {
        MongoDBChatMessageHistory::new(
            "mongodb://localhost:27017".to_string(),
            session_id.to_string(),
            Some("test_chat_history".to_string()),
            Some("test_messages".to_string()),
            true,
        )
        .await
        .expect("MongoDB must be running on localhost to run ignored tests")
    }

    #[tokio::test]
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_basic() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_add_messages() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_clear() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_multiple_sessions() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_with_unicode() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_empty_state() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_persistence() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_serialization_roundtrip() {
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
    #[ignore = "requires MongoDB running on localhost"]
    async fn test_mongodb_chat_history_large_batch() {
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
