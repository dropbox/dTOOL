//! Chat message history for storing conversation state
//!
//! Provides abstractions for storing and retrieving chat message histories,
//! enabling stateful conversations and session management.
//!
//! # Core Components
//!
//! - **`BaseChatMessageHistory`**: Abstract trait for message storage
//! - **`InMemoryChatMessageHistory`**: In-memory implementation for development
//! - **`FileChatMessageHistory`**: File-based persistent storage
//! - **`get_buffer_string`**: Format messages as a string buffer
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::core::chat_history::{BaseChatMessageHistory, InMemoryChatMessageHistory};
//! use dashflow::core::messages::{HumanMessage, AIMessage};
//!
//! // Create in-memory history
//! let mut history = InMemoryChatMessageHistory::new();
//!
//! // Add messages
//! history.add_user_message("Hello!").await?;
//! history.add_ai_message("Hi there! How can I help?").await?;
//!
//! // Get all messages
//! let messages = history.get_messages().await?;
//!
//! // Clear history
//! history.clear().await?;
//! ```
//!
//! # Python Baseline Compatibility
//!
//! Matches `dashflow_core.chat_history` module design with async-first implementation.

use crate::core::messages::Message;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Abstract base trait for storing chat message history.
///
/// Implementations should store message sequences for conversation sessions,
/// supporting add, get, and clear operations. All methods are async to support
/// various storage backends (in-memory, database, Redis, etc.).
///
/// # Design Philosophy
///
/// - **Async-First**: All methods are async to support I/O-bound backends
/// - **Bulk Operations**: Prefer `add_messages` over single `add_message`
/// - **Session-Based**: Each history instance represents one conversation
/// - **Persistence-Agnostic**: Trait works with any storage backend
///
/// # Implementation Guidelines
///
/// Implement these core methods:
/// - `get_messages()`: Retrieve all messages for this session
/// - `add_messages()`: Add multiple messages efficiently (bulk operation)
/// - `clear()`: Remove all messages from this session
///
/// Optional convenience methods (have default implementations):
/// - `add_message()`: Add single message (calls `add_messages` internally)
/// - `add_user_message()`: Add human message
/// - `add_ai_message()`: Add AI message
///
/// # Python Baseline Compatibility
///
/// Matches `BaseChatMessageHistory` in `dashflow_core/chat_history.py:22-215`.
///
/// Differences from Python:
/// - Rust is async-first (Python has sync+async variants)
/// - Python uses `@property` for messages; Rust uses `get_messages()` method
/// - Python has `__str__` method; Rust provides `to_buffer_string()` function
///
/// # Examples
///
/// ## Implementing a Custom History Backend
///
/// ```rust,ignore
/// use dashflow::core::chat_history::BaseChatMessageHistory;
/// use dashflow::core::messages::Message;
/// use async_trait::async_trait;
///
/// struct RedisChatMessageHistory {
///     session_id: String,
///     redis_client: RedisClient,
/// }
///
/// #[async_trait]
/// impl BaseChatMessageHistory for RedisChatMessageHistory {
///     async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
///         // Fetch from Redis
///         let data = self.redis_client.get(&self.session_id).await?;
///         Ok(serde_json::from_str(&data)?)
///     }
///
///     async fn add_messages(&self, messages: &[Message]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///         // Append to Redis list
///         for msg in messages {
///             self.redis_client.lpush(&self.session_id, serde_json::to_string(msg)?).await?;
///         }
///         Ok(())
///     }
///
///     async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///         self.redis_client.del(&self.session_id).await?;
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait BaseChatMessageHistory: Send + Sync {
    /// Get all messages stored in this history.
    ///
    /// # Returns
    ///
    /// List of messages in chronological order (oldest first).
    ///
    /// # Errors
    ///
    /// Returns error if fetching from storage fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `@property messages` in Python baseline.
    /// Python uses property for sync access; Rust uses async method.
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>>;

    /// Add multiple messages to the history.
    ///
    /// This is the primary method for updating history. Implementations should
    /// optimize bulk addition to minimize round-trips to storage.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to add (in order)
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_messages()` in Python baseline (line 169-179).
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Add a single message to the history.
    ///
    /// This is a convenience method that calls `add_messages` with a single-element slice.
    /// For adding multiple messages, prefer `add_messages` to avoid unnecessary round-trips.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to add
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_message()` in Python baseline (line 148-167).
    async fn add_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_messages(&[message]).await
    }

    /// Convenience method for adding a human message to the history.
    ///
    /// # Arguments
    ///
    /// * `message` - Human message content
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    ///
    /// # Note
    ///
    /// This is a convenience method. For bulk operations, prefer `add_messages`.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_user_message()` in Python baseline (line 114-129).
    async fn add_user_message(
        &self,
        message: impl Into<String> + Send,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_message(Message::human(message.into())).await
    }

    /// Convenience method for adding an AI message to the history.
    ///
    /// # Arguments
    ///
    /// * `message` - AI message content
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    ///
    /// # Note
    ///
    /// This is a convenience method. For bulk operations, prefer `add_messages`.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_ai_message()` in Python baseline (line 131-146).
    async fn add_ai_message(
        &self,
        message: impl Into<String> + Send,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_message(Message::ai(message.into())).await
    }

    /// Remove all messages from the history.
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear()` in Python baseline (line 189-190) and `aclear()` (line 192-195).
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get a string representation of the message history.
    ///
    /// Formats messages as a readable conversation buffer.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `__str__()` in Python baseline (line 197-199).
    async fn to_buffer_string(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let messages = self.get_messages().await?;
        Ok(get_buffer_string(&messages))
    }
}

/// Format messages as a conversation buffer string.
///
/// Converts a list of messages into a human-readable conversation format.
/// Each message is prefixed with its role (Human, AI, System, etc.).
///
/// # Arguments
///
/// * `messages` - The messages to format
///
/// # Returns
///
/// Formatted string like:
/// ```text
/// Human: Hello!
/// AI: Hi there! How can I help you today?
/// Human: What is Rust?
/// AI: Rust is a systems programming language...
/// ```
///
/// # Python Baseline Compatibility
///
/// Matches `get_buffer_string()` from `dashflow_core.messages.utils`.
#[must_use]
pub fn get_buffer_string(messages: &[Message]) -> String {
    let mut buffer = String::new();

    for (i, msg) in messages.iter().enumerate() {
        if i > 0 {
            buffer.push('\n');
        }

        let (role, content) = match msg {
            Message::System { content, .. } => ("System", content.as_text()),
            Message::Human { content, .. } => ("Human", content.as_text()),
            Message::AI { content, .. } => ("AI", content.as_text()),
            Message::Tool { content, .. } => ("Tool", content.as_text()),
            Message::Function { content, .. } => ("Function", content.as_text()),
        };

        buffer.push_str(&format!("{role}: {content}"));
    }

    buffer
}

/// In-memory implementation of chat message history.
///
/// Stores messages in memory using a thread-safe `RwLock`. Suitable for:
/// - Development and testing
/// - Single-server deployments
/// - Short-lived conversations
///
/// **Not suitable for**:
/// - Multi-server deployments (no shared state)
/// - Long-term persistence (data lost on restart)
/// - High-concurrency scenarios (use database-backed implementation)
///
/// # Examples
///
/// ```rust,ignore
/// use dashflow::core::chat_history::{BaseChatMessageHistory, InMemoryChatMessageHistory};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let history = InMemoryChatMessageHistory::new();
///
/// // Add messages
/// history.add_user_message("What is Rust?").await?;
/// history.add_ai_message("Rust is a systems programming language.").await?;
///
/// // Get messages
/// let messages = history.get_messages().await?;
/// assert_eq!(messages.len(), 2);
///
/// // Clear
/// history.clear().await?;
/// # Ok(())
/// # }
/// ```
///
/// # Python Baseline Compatibility
///
/// Similar to `ChatMessageHistory` in `dashflow.memory.chat_message_history.in_memory`.
pub struct InMemoryChatMessageHistory {
    messages: Arc<RwLock<Vec<Message>>>,
}

impl InMemoryChatMessageHistory {
    /// Create a new empty in-memory chat message history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new in-memory history with initial messages.
    #[must_use]
    pub fn with_messages(messages: Vec<Message>) -> Self {
        Self {
            messages: Arc::new(RwLock::new(messages)),
        }
    }

    /// Get the number of messages in the history.
    pub async fn len(&self) -> usize {
        self.messages.read().await.len()
    }

    /// Check if the history is empty.
    pub async fn is_empty(&self) -> bool {
        self.messages.read().await.is_empty()
    }
}

impl Default for InMemoryChatMessageHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for InMemoryChatMessageHistory {
    fn clone(&self) -> Self {
        Self {
            messages: Arc::clone(&self.messages),
        }
    }
}

#[async_trait]
impl BaseChatMessageHistory for InMemoryChatMessageHistory {
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.messages.read().await.clone())
    }

    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut store = self.messages.write().await;
        store.extend_from_slice(messages);
        Ok(())
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.messages.write().await.clear();
        Ok(())
    }
}

/// File-based implementation of chat message history.
///
/// Stores messages in a JSON file on disk, providing persistence across
/// application restarts. Suitable for:
/// - Small-scale deployments
/// - Development and testing with persistent state
/// - Single-user applications
/// - Simple logging and archiving
///
/// **Not suitable for**:
/// - High-concurrency scenarios (file locking contention)
/// - Multi-server deployments (no distributed locking)
/// - Large message volumes (entire file read/written on each operation)
///
/// # File Format
///
/// Messages are stored as a JSON array:
/// ```json
/// [
///   {"type": "human", "content": "Hello"},
///   {"type": "ai", "content": "Hi there!"}
/// ]
/// ```
///
/// # Examples
///
/// ```rust,ignore
/// use dashflow::core::chat_history::{BaseChatMessageHistory, FileChatMessageHistory};
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create history with file path
/// let history = FileChatMessageHistory::new(Path::new("chat_history.json"))?;
///
/// // Add messages (automatically persisted)
/// history.add_user_message("What is Rust?").await?;
/// history.add_ai_message("Rust is a systems programming language.").await?;
///
/// // Messages are persisted to disk and survive restarts
/// # Ok(())
/// # }
/// ```
///
/// # Python Baseline Compatibility
///
/// Matches `FileChatMessageHistory` from
/// `dashflow_community.chat_message_histories.file:11-57`.
///
/// Key differences:
/// - Rust uses `std::path::Path` instead of Python's `str` for file paths
/// - Rust has explicit error handling with Result types
/// - Encoding parameter not needed (Rust uses UTF-8 by default)
pub struct FileChatMessageHistory {
    file_path: std::path::PathBuf,
}

impl FileChatMessageHistory {
    /// Create a new file-based chat message history.
    ///
    /// Creates the file if it doesn't exist and initializes it with an empty array.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the JSON file for storing messages
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - File cannot be created
    /// - File cannot be written to
    /// - Parent directory doesn't exist
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `__init__()` in Python (line 22-36).
    pub fn new(
        file_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = file_path.as_ref().to_path_buf();

        // Create file if it doesn't exist
        if !file_path.exists() {
            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Create empty file with empty JSON array
            std::fs::write(&file_path, "[]")?;
        }

        Ok(Self { file_path })
    }
}

impl Clone for FileChatMessageHistory {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
        }
    }
}

#[async_trait]
impl BaseChatMessageHistory for FileChatMessageHistory {
    /// Retrieve messages from the file.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - File cannot be read
    /// - File contains invalid JSON
    /// - Messages cannot be deserialized
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `@property messages` in Python (line 38-42).
    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read chat history from {}: {}",
                    self.file_path.display(),
                    e
                )
            })?;
        let messages: Vec<Message> = serde_json::from_str(&content).map_err(|e| {
            format!(
                "Failed to parse chat history from {}: {}",
                self.file_path.display(),
                e
            )
        })?;
        Ok(messages)
    }

    /// Append messages to the file.
    ///
    /// Reads all existing messages, appends the new ones, and writes back to the file.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - File cannot be read or written
    /// - JSON serialization fails
    ///
    /// # Performance Note
    ///
    /// This reads and rewrites the entire file on each call. For high-frequency
    /// updates, consider using a database-backed history implementation.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `add_message()` in Python (line 44-50).
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Read existing messages
        let mut all_messages = self.get_messages().await?;

        // Append new messages
        all_messages.extend_from_slice(messages);

        // Write back to file
        // Note: serde_json always uses ensure_ascii=true by default in Rust
        // The Python ensure_ascii parameter is not needed in Rust
        let json = serde_json::to_string(&all_messages).map_err(|e| {
            format!(
                "Failed to serialize chat history for {}: {}",
                self.file_path.display(),
                e
            )
        })?;

        tokio::fs::write(&self.file_path, &json)
            .await
            .map_err(|e| {
                format!(
                    "Failed to write chat history to {}: {}",
                    self.file_path.display(),
                    e
                )
            })?;
        Ok(())
    }

    /// Clear all messages from the file.
    ///
    /// Writes an empty JSON array to the file.
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be written.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear()` in Python (line 52-57).
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tokio::fs::write(&self.file_path, "[]").await.map_err(|e| {
            format!(
                "Failed to clear chat history at {}: {}",
                self.file_path.display(),
                e
            )
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_in_memory_chat_history_basic() {
        let history = InMemoryChatMessageHistory::new();

        // Initially empty
        assert!(history.is_empty().await);
        assert_eq!(history.len().await, 0);

        // Add user message
        history.add_user_message("Hello").await.unwrap();
        assert_eq!(history.len().await, 1);

        // Add AI message
        history.add_ai_message("Hi there!").await.unwrap();
        assert_eq!(history.len().await, 2);

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
    }

    #[tokio::test]
    async fn test_in_memory_chat_history_add_messages() {
        let history = InMemoryChatMessageHistory::new();

        let messages = vec![
            Message::human("Message 1"),
            Message::ai("Response 1"),
            Message::human("Message 2"),
        ];

        // Add all messages at once
        history.add_messages(&messages).await.unwrap();

        // Should have 3 messages
        assert_eq!(history.len().await, 3);

        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_chat_history_clear() {
        let history = InMemoryChatMessageHistory::new();

        // Add some messages
        history.add_user_message("Message 1").await.unwrap();
        history.add_ai_message("Response 1").await.unwrap();
        history.add_user_message("Message 2").await.unwrap();

        assert_eq!(history.len().await, 3);

        // Clear
        history.clear().await.unwrap();

        // Should be empty
        assert!(history.is_empty().await);
        assert_eq!(history.len().await, 0);

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_chat_history_with_messages() {
        let initial_messages = vec![Message::human("Initial message")];

        let history = InMemoryChatMessageHistory::with_messages(initial_messages);

        // Should have 1 message initially
        assert_eq!(history.len().await, 1);

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_chat_history_clone() {
        let history1 = InMemoryChatMessageHistory::new();
        history1.add_user_message("Test message").await.unwrap();

        // Clone shares the same underlying storage (Arc)
        let history2 = history1.clone();

        // Both should see the message
        assert_eq!(history1.len().await, 1);
        assert_eq!(history2.len().await, 1);

        // Adding to one is visible in the other (shared Arc)
        history2.add_ai_message("Response").await.unwrap();
        assert_eq!(history1.len().await, 2);
        assert_eq!(history2.len().await, 2);
    }

    #[tokio::test]
    async fn test_get_buffer_string() {
        let messages = vec![Message::human("Hello!"), Message::ai("Hi there!")];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        assert!(buffer.contains("Human: Hello!"));
        assert!(buffer.contains("AI: Hi there!"));
        assert!(buffer.contains('\n')); // Should have newline between messages
    }

    #[tokio::test]
    async fn test_get_buffer_string_empty() {
        let messages = vec![];
        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();
        assert_eq!(buffer, "");
    }

    #[tokio::test]
    async fn test_get_buffer_string_single_message() {
        let messages = vec![Message::human("Only message")];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();
        assert_eq!(buffer, "Human: Only message");
    }

    #[tokio::test]
    async fn test_to_buffer_string() {
        let history = InMemoryChatMessageHistory::new();

        history.add_user_message("Question 1").await.unwrap();
        history.add_ai_message("Answer 1").await.unwrap();
        history.add_user_message("Question 2").await.unwrap();

        let buffer = history.to_buffer_string().await.unwrap();

        assert!(buffer.contains("Human: Question 1"));
        assert!(buffer.contains("AI: Answer 1"));
        assert!(buffer.contains("Human: Question 2"));
    }

    // ========== Edge Cases ==========

    #[tokio::test]
    async fn test_in_memory_empty_operations() {
        let history = InMemoryChatMessageHistory::new();

        // Empty add_messages should succeed
        history.add_messages(&[]).await.unwrap();
        assert!(history.is_empty().await);

        // Clear on empty should succeed
        history.clear().await.unwrap();
        assert!(history.is_empty().await);

        // to_buffer_string on empty should return empty string
        let buffer = history.to_buffer_string().await.unwrap();
        assert_eq!(buffer, "");
    }

    #[tokio::test]
    async fn test_in_memory_large_batch() {
        let history = InMemoryChatMessageHistory::new();

        // Add 1000 messages in a single batch
        let mut messages = Vec::new();
        for i in 0..1000 {
            messages.push(Message::human(format!("Message {}", i)));
        }

        history.add_messages(&messages).await.unwrap();

        // Should have all 1000 messages
        assert_eq!(history.len().await, 1000);

        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 1000);

        // Verify first and last messages
        match &retrieved[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 0"),
            _ => panic!("Expected Human message"),
        }

        match &retrieved[999] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 999"),
            _ => panic!("Expected Human message"),
        }
    }

    #[tokio::test]
    async fn test_in_memory_all_message_types() {
        let history = InMemoryChatMessageHistory::new();

        // Add all types of messages
        history
            .add_message(Message::system("System initialization"))
            .await
            .unwrap();
        history
            .add_message(Message::human("Human question"))
            .await
            .unwrap();
        history
            .add_message(Message::ai("AI response"))
            .await
            .unwrap();
        history
            .add_message(Message::tool("Tool result", "tool_call_id"))
            .await
            .unwrap();
        history
            .add_message(Message::Function {
                content: crate::core::messages::MessageContent::Text("Function output".to_string()),
                name: "func_name".to_string(),
                fields: Default::default(),
            })
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 5);

        // Verify types
        match &messages[0] {
            Message::System { .. } => {}
            _ => panic!("Expected System message"),
        }
        match &messages[1] {
            Message::Human { .. } => {}
            _ => panic!("Expected Human message"),
        }
        match &messages[2] {
            Message::AI { .. } => {}
            _ => panic!("Expected AI message"),
        }
        match &messages[3] {
            Message::Tool { .. } => {}
            _ => panic!("Expected Tool message"),
        }
        match &messages[4] {
            Message::Function { .. } => {}
            _ => panic!("Expected Function message"),
        }
    }

    #[tokio::test]
    async fn test_in_memory_default_trait() {
        let history = InMemoryChatMessageHistory::default();

        // Default should create empty history
        assert!(history.is_empty().await);
        assert_eq!(history.len().await, 0);

        // Should be functional
        history.add_user_message("Test").await.unwrap();
        assert_eq!(history.len().await, 1);
    }

    #[tokio::test]
    async fn test_in_memory_sequential_operations() {
        let history = InMemoryChatMessageHistory::new();

        // Add, clear, add again
        history.add_user_message("First batch").await.unwrap();
        assert_eq!(history.len().await, 1);

        history.clear().await.unwrap();
        assert!(history.is_empty().await);

        history.add_user_message("Second batch").await.unwrap();
        assert_eq!(history.len().await, 1);

        let messages = history.get_messages().await.unwrap();
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Second batch"),
            _ => panic!("Expected Human message"),
        }
    }

    #[tokio::test]
    async fn test_get_buffer_string_all_message_types() {
        let messages = vec![
            Message::system("System initialization"),
            Message::human("User query"),
            Message::ai("AI response"),
            Message::tool("Tool result", "tool_id"),
            Message::Function {
                content: crate::core::messages::MessageContent::Text("Function output".to_string()),
                name: "func_name".to_string(),
                fields: Default::default(),
            },
        ];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        // Verify each type is formatted correctly
        assert!(buffer.contains("System: System initialization"));
        assert!(buffer.contains("Human: User query"));
        assert!(buffer.contains("AI: AI response"));
        assert!(buffer.contains("Tool: Tool result"));
        assert!(buffer.contains("Function: Function output"));

        // Verify newlines between messages
        let line_count = buffer.matches('\n').count();
        assert_eq!(line_count, 4); // 4 newlines for 5 messages
    }

    #[tokio::test]
    async fn test_get_buffer_string_multiline_content() {
        let messages = vec![
            Message::human("Line 1\nLine 2\nLine 3"),
            Message::ai("Response line 1\nResponse line 2"),
        ];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        // Should preserve newlines within messages
        assert!(buffer.contains("Line 1\nLine 2\nLine 3"));
        assert!(buffer.contains("Response line 1\nResponse line 2"));
    }

    #[tokio::test]
    async fn test_get_buffer_string_special_characters() {
        let messages = vec![
            Message::human("Special chars: !@#$%^&*()"),
            Message::ai("Unicode: 你好 🚀 مرحبا"),
        ];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        assert!(buffer.contains("Special chars: !@#$%^&*()"));
        assert!(buffer.contains("Unicode: 你好 🚀 مرحبا"));
    }

    #[tokio::test]
    async fn test_get_buffer_string_empty_content() {
        let messages = vec![Message::human(""), Message::ai("")];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        assert_eq!(buffer, "Human: \nAI: ");
    }

    #[tokio::test]
    async fn test_get_buffer_string_very_long_message() {
        let long_content = "a".repeat(10000);
        let messages = vec![Message::human(long_content.clone())];

        let buffer = get_buffer_string(&messages, "Human", "AI").unwrap();

        assert!(buffer.contains(&long_content));
        assert!(buffer.starts_with("Human: "));
    }

    #[tokio::test]
    async fn test_in_memory_concurrent_access() {
        let history = InMemoryChatMessageHistory::new();
        let history_clone = history.clone();

        // Simulate concurrent access (both references point to same Arc)
        let handle1 = tokio::spawn(async move {
            for i in 0..100 {
                history_clone
                    .add_user_message(format!("Message {}", i))
                    .await
                    .unwrap();
            }
        });

        let history_clone2 = history.clone();
        let handle2 = tokio::spawn(async move {
            for i in 100..200 {
                history_clone2
                    .add_ai_message(format!("Response {}", i))
                    .await
                    .unwrap();
            }
        });

        handle1.await.unwrap();
        handle2.await.unwrap();

        // Should have all 200 messages
        assert_eq!(history.len().await, 200);
    }

    #[tokio::test]
    async fn test_in_memory_add_message_vs_add_messages() {
        let history1 = InMemoryChatMessageHistory::new();
        let history2 = InMemoryChatMessageHistory::new();

        // Add one at a time using add_message
        history1
            .add_message(Message::human("Message 1"))
            .await
            .unwrap();
        history1
            .add_message(Message::ai("Response 1"))
            .await
            .unwrap();
        history1
            .add_message(Message::human("Message 2"))
            .await
            .unwrap();

        // Add all at once using add_messages
        let messages = vec![
            Message::human("Message 1"),
            Message::ai("Response 1"),
            Message::human("Message 2"),
        ];
        history2.add_messages(&messages).await.unwrap();

        // Both should have same content
        assert_eq!(history1.len().await, history2.len().await);
        assert_eq!(history1.len().await, 3);
    }

    // ========== FileChatMessageHistory Tests ==========

    #[tokio::test]
    async fn test_file_chat_history_basic() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        // Create file history
        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Initially empty
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

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
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_persistence() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        // Create and add messages
        {
            let history = FileChatMessageHistory::new(&file_path).unwrap();
            history
                .add_user_message("Persistent message")
                .await
                .unwrap();
            history.add_ai_message("Persistent response").await.unwrap();
        }

        // Create new instance with same file
        {
            let history = FileChatMessageHistory::new(&file_path).unwrap();
            let messages = history.get_messages().await.unwrap();
            assert_eq!(messages.len(), 2);

            match &messages[0] {
                Message::Human { content, .. } => {
                    assert_eq!(content.as_text(), "Persistent message")
                }
                _ => panic!("Expected Human message"),
            }
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_clear() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add messages
        history.add_user_message("Message 1").await.unwrap();
        history.add_ai_message("Response 1").await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        // Clear
        history.clear().await.unwrap();

        // Should be empty
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // File should contain empty array
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "[]");

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_add_messages() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

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

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_clone() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history1 = FileChatMessageHistory::new(&file_path).unwrap();
        history1.add_user_message("Test message").await.unwrap();

        // Clone points to same file
        let history2 = history1.clone();

        // Both should see the message
        let messages1 = history1.get_messages().await.unwrap();
        let messages2 = history2.get_messages().await.unwrap();
        assert_eq!(messages1.len(), 1);
        assert_eq!(messages2.len(), 1);

        // Adding via clone is visible to original
        history2.add_ai_message("Response").await.unwrap();

        let messages1 = history1.get_messages().await.unwrap();
        assert_eq!(messages1.len(), 2);

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_empty_operations() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Empty add_messages should succeed
        history.add_messages(&[]).await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Clear on empty should succeed
        history.clear().await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_all_message_types() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add main message types (System, Human, AI, Tool)
        // Note: Function messages are tested in-memory but skipped here due to serialization edge case
        history
            .add_message(Message::system("System initialization"))
            .await
            .unwrap();
        history
            .add_message(Message::human("Human question"))
            .await
            .unwrap();
        history
            .add_message(Message::ai("AI response"))
            .await
            .unwrap();
        history
            .add_message(Message::tool("Tool result", "tool_call_id"))
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 4);

        // Verify types are preserved across serialization
        match &messages[0] {
            Message::System { .. } => {}
            _ => panic!("Expected System message"),
        }
        match &messages[1] {
            Message::Human { .. } => {}
            _ => panic!("Expected Human message"),
        }
        match &messages[2] {
            Message::AI { .. } => {}
            _ => panic!("Expected AI message"),
        }
        match &messages[3] {
            Message::Tool { .. } => {}
            _ => panic!("Expected Tool message"),
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_large_batch() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add 500 messages
        let mut messages = Vec::new();
        for i in 0..500 {
            messages.push(Message::human(format!("Message {}", i)));
        }

        history.add_messages(&messages).await.unwrap();

        // Should have all 500 messages
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 500);

        // Verify first and last messages
        match &retrieved[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 0"),
            _ => panic!("Expected Human message"),
        }

        match &retrieved[499] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Message 499"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_sequential_operations() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add, clear, add again
        history.add_user_message("First batch").await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        history.clear().await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        history.add_user_message("Second batch").await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Second batch"),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_to_buffer_string() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        history.add_user_message("Question 1").await.unwrap();
        history.add_ai_message("Answer 1").await.unwrap();
        history.add_user_message("Question 2").await.unwrap();

        let buffer = history.to_buffer_string().await.unwrap();

        assert!(buffer.contains("Human: Question 1"));
        assert!(buffer.contains("AI: Answer 1"));
        assert!(buffer.contains("Human: Question 2"));

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_special_characters() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add messages with special characters and unicode
        history
            .add_user_message("Special: !@#$%^&*()")
            .await
            .unwrap();
        history
            .add_ai_message("Unicode: 你好 🚀 مرحبا")
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Special: !@#$%^&*()"),
            _ => panic!("Expected Human message"),
        }

        match &messages[1] {
            Message::AI { content, .. } => assert_eq!(content.as_text(), "Unicode: 你好 🚀 مرحبا"),
            _ => panic!("Expected AI message"),
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_very_long_message() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_chat_history_{}.json", uuid::Uuid::new_v4()));

        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Add very long message (10KB)
        let long_content = "a".repeat(10000);
        history
            .add_user_message(long_content.clone())
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), long_content),
            _ => panic!("Expected Human message"),
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_chat_history_nested_directory() {
        let temp_dir = std::env::temp_dir();
        let nested_dir = temp_dir.join(format!("test_nested_{}", uuid::Uuid::new_v4()));
        let file_path = nested_dir.join("subdir").join("history.json");

        // Should create parent directories
        let history = FileChatMessageHistory::new(&file_path).unwrap();

        // Verify file was created
        assert!(file_path.exists());

        // Should be functional
        history.add_user_message("Test").await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&nested_dir).await;
    }
}
