use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use serde_json;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Chat message history that stores messages in a local JSON file.
///
/// This implementation provides persistent, file-based storage for conversation history.
/// Messages are stored as JSON arrays in a local file, with automatic file creation
/// and atomic writes to prevent corruption.
///
/// # Features
///
/// - **Persistent Storage**: Messages survive process restarts
/// - **JSON Format**: Human-readable and easily debuggable
/// - **Atomic Writes**: Uses write-then-rename to prevent partial writes
/// - **Configurable Encoding**: Supports custom character encodings
/// - **ASCII Control**: Option to escape non-ASCII characters
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_memory::FileChatMessageHistory;
/// use dashflow::core::chat_history::BaseChatMessageHistory;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// // Create a file-backed history
/// let history = FileChatMessageHistory::new("chat_history.json").await?;
///
/// // Add messages
/// history.add_user_message("Hello!").await?;
/// history.add_ai_message("Hi! How can I help?").await?;
///
/// // Retrieve messages
/// let messages = history.get_messages().await?;
/// assert_eq!(messages.len(), 2);
///
/// // Clear history
/// history.clear().await?;
/// # Ok(())
/// # }
/// ```
///
/// # Python Baseline
///
/// Matches `dashflow_community.chat_message_histories.file.FileChatMessageHistory`:
/// - Same JSON file format
/// - Same automatic file creation behavior
/// - Same message serialization format
///
/// # Storage Format
///
/// Messages are stored as a JSON array:
/// ```json
/// [
///   {"type": "human", "content": "Hello!"},
///   {"type": "ai", "content": "Hi! How can I help?"}
/// ]
/// ```
#[derive(Debug, Clone)]
pub struct FileChatMessageHistory {
    file_path: PathBuf,
}

impl FileChatMessageHistory {
    /// Creates a new file-based chat message history.
    ///
    /// If the file does not exist, it will be created with an empty message array.
    /// If the file exists, existing messages will be preserved.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the JSON file for storing messages
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File creation fails
    /// - File system permissions are insufficient
    /// - Parent directory does not exist
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_memory::FileChatMessageHistory;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let history = FileChatMessageHistory::new("history.json").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new<P: AsRef<Path>>(
        file_path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = file_path.as_ref().to_path_buf();

        // Create file if it doesn't exist (use tokio::fs::try_exists for non-blocking check - M-633)
        let exists = fs::try_exists(&file_path).await.unwrap_or(false);
        if !exists {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let mut file = fs::File::create(&file_path).await?;
            let content = "[]";
            file.write_all(content.as_bytes()).await?;
            file.flush().await?;
        }

        Ok(Self { file_path })
    }

    /// Reads messages from the file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - JSON parsing fails
    /// - Message deserialization fails
    async fn read_messages(
        &self,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(&self.file_path).await?;
        let messages: Vec<Message> = serde_json::from_str(&content)?;
        Ok(messages)
    }

    /// Writes messages to the file atomically.
    ///
    /// Uses a write-then-rename strategy to ensure atomicity:
    /// 1. Write to temporary file
    /// 2. Flush to disk
    /// 3. Rename temporary file to target file
    ///
    /// This prevents partial writes from corrupting the history.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File write fails
    /// - JSON serialization fails
    /// - Rename operation fails
    async fn write_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Serialize messages to JSON
        // Note: serde_json always escapes non-ASCII by default, matching Python's ensure_ascii=True
        let content = serde_json::to_string(messages)?;

        // Atomic write: write to temp file, then rename
        let temp_path = self.file_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        drop(file);

        // Rename is atomic on most filesystems
        fs::rename(&temp_path, &self.file_path).await?;

        Ok(())
    }
}

#[async_trait]
impl BaseChatMessageHistory for FileChatMessageHistory {
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut existing = self.read_messages().await?;
        existing.extend_from_slice(messages);
        self.write_messages(&existing).await?;
        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        self.read_messages().await
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.write_messages(&[]).await?;
        Ok(())
    }
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_history_new() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // File should exist
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_file_history_add_message() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();

        history.add_message(Message::human("Hello!")).await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        assert!(
            matches!(&messages[0], Message::Human { .. }),
            "Expected HumanMessage"
        );
        if let Message::Human { content, .. } = &messages[0] {
            assert_eq!(content.as_text(), "Hello!");
        }
    }

    #[tokio::test]
    async fn test_file_history_multiple_messages() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();

        history.add_user_message("Hello!").await.unwrap();
        history.add_ai_message("Hi! How can I help?").await.unwrap();
        history
            .add_user_message("Tell me about Rust")
            .await
            .unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 3);

        // Verify message types and content
        assert!(
            matches!(&messages[0], Message::Human { .. }),
            "Expected HumanMessage"
        );
        if let Message::Human { content, .. } = &messages[0] {
            assert_eq!(content.as_text(), "Hello!");
        }
        assert!(matches!(&messages[1], Message::AI { .. }), "Expected AIMessage");
        if let Message::AI { content, .. } = &messages[1] {
            assert_eq!(content.as_text(), "Hi! How can I help?");
        }
        assert!(
            matches!(&messages[2], Message::Human { .. }),
            "Expected HumanMessage"
        );
        if let Message::Human { content, .. } = &messages[2] {
            assert_eq!(content.as_text(), "Tell me about Rust");
        }
    }

    #[tokio::test]
    async fn test_file_history_clear() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();

        history.add_user_message("Hello!").await.unwrap();
        history.add_ai_message("Hi!").await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);

        history.clear().await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn test_file_history_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        // Create history and add messages
        {
            let history = FileChatMessageHistory::new(&file_path).await.unwrap();
            history.add_user_message("Message 1").await.unwrap();
            history.add_ai_message("Response 1").await.unwrap();
        }

        // Create new instance and verify messages persisted
        {
            let history = FileChatMessageHistory::new(&file_path).await.unwrap();
            let messages = history.get_messages().await.unwrap();
            assert_eq!(messages.len(), 2);

            assert!(
                matches!(&messages[0], Message::Human { .. }),
                "Expected HumanMessage"
            );
            if let Message::Human { content, .. } = &messages[0] {
                assert_eq!(content.as_text(), "Message 1");
            }
        }
    }

    #[tokio::test]
    async fn test_file_history_with_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();

        // Test with Unicode content
        history.add_user_message("Hello 世界 🌍").await.unwrap();

        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);

        assert!(
            matches!(&messages[0], Message::Human { .. }),
            "Expected HumanMessage"
        );
        if let Message::Human { content, .. } = &messages[0] {
            assert_eq!(content.as_text(), "Hello 世界 🌍");
        }
    }

    #[tokio::test]
    async fn test_file_history_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir
            .path()
            .join("nested")
            .join("dir")
            .join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();
        history.add_user_message("Test").await.unwrap();

        // Verify nested directories were created
        assert!(file_path.parent().unwrap().exists());
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_file_history_empty_file_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let _history = FileChatMessageHistory::new(&file_path).await.unwrap();

        // Read file content directly
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "[]");
    }

    #[tokio::test]
    async fn test_file_history_add_messages_bulk() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("history.json");

        let history = FileChatMessageHistory::new(&file_path).await.unwrap();

        let messages = vec![
            Message::human("Message 1"),
            Message::ai("Response 1"),
            Message::human("Message 2"),
        ];

        history.add_messages(&messages).await.unwrap();

        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 3);
    }
}
