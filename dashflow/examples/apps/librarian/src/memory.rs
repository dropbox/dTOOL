//! Memory module for the Superhuman Librarian
//!
//! Provides persistent storage for conversation history, reading progress,
//! bookmarks, and user preferences across sessions.
//!
//! Uses simple file-based storage for the demo. In production, this could
//! be backed by Redis, PostgreSQL, or S3.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// A single conversation turn
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    /// User's message
    pub user: String,
    /// Librarian's response
    pub assistant: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Books referenced in this turn
    pub referenced_books: Vec<String>,
}

/// Reading progress for a book
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadingProgress {
    /// Book ID
    pub book_id: String,
    /// Book title
    pub title: String,
    /// Current chapter or section
    pub current_chapter: Option<String>,
    /// Last chunk index read
    pub last_chunk_index: i64,
    /// Progress percentage (0-100)
    pub progress_percent: f32,
    /// Last read timestamp
    pub last_read: chrono::DateTime<chrono::Utc>,
}

/// A bookmark in a book
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    /// Unique bookmark ID
    pub id: Uuid,
    /// Book ID
    pub book_id: String,
    /// Chunk index
    pub chunk_index: i64,
    /// Optional note
    pub note: Option<String>,
    /// Created timestamp
    pub created: chrono::DateTime<chrono::Utc>,
}

/// A note about a book or passage
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    /// Unique note ID
    pub id: Uuid,
    /// Book ID
    pub book_id: String,
    /// Chunk index (if specific)
    pub chunk_index: Option<i64>,
    /// Note content
    pub content: String,
    /// Created timestamp
    pub created: chrono::DateTime<chrono::Utc>,
}

/// Search query history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The query text
    pub query: String,
    /// Filters used
    pub filters: Option<crate::search::SearchFilters>,
    /// Number of results returned
    pub result_count: usize,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Complete librarian memory state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LibrarianMemory {
    /// User ID (for multi-user support)
    pub user_id: String,

    /// Conversation history (last N turns)
    pub conversation: Vec<Turn>,

    /// Reading progress per book
    pub reading_progress: HashMap<String, ReadingProgress>,

    /// Currently reading book
    pub current_book: Option<String>,

    /// User's favorite authors (learned over time)
    pub favorite_authors: Vec<String>,

    /// User's favorite genres/themes
    pub favorite_themes: Vec<String>,

    /// Bookmarks
    pub bookmarks: Vec<Bookmark>,

    /// Notes
    pub notes: Vec<Note>,

    /// Recent search queries
    pub recent_searches: Vec<SearchQuery>,
}

impl LibrarianMemory {
    /// Create a new memory for a user
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            ..Default::default()
        }
    }

    /// Add a conversation turn
    pub fn add_turn(
        &mut self,
        user: impl Into<String>,
        assistant: impl Into<String>,
        books: Vec<String>,
    ) {
        // Keep only last 50 turns
        if self.conversation.len() >= 50 {
            self.conversation.remove(0);
        }

        self.conversation.push(Turn {
            user: user.into(),
            assistant: assistant.into(),
            timestamp: chrono::Utc::now(),
            referenced_books: books,
        });
    }

    /// Update reading progress for a book
    pub fn update_reading_progress(
        &mut self,
        book_id: impl Into<String>,
        title: impl Into<String>,
        chunk_index: i64,
        chapter: Option<String>,
    ) {
        let book_id = book_id.into();
        let title = title.into();

        let progress = self
            .reading_progress
            .entry(book_id.clone())
            .or_insert_with(|| ReadingProgress {
                book_id: book_id.clone(),
                title: title.clone(),
                current_chapter: None,
                last_chunk_index: 0,
                progress_percent: 0.0,
                last_read: chrono::Utc::now(),
            });

        progress.last_chunk_index = chunk_index;
        progress.current_chapter = chapter;
        progress.last_read = chrono::Utc::now();

        // Update current book
        self.current_book = Some(book_id);
    }

    /// Add a bookmark
    pub fn add_bookmark(
        &mut self,
        book_id: impl Into<String>,
        chunk_index: i64,
        note: Option<String>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.bookmarks.push(Bookmark {
            id,
            book_id: book_id.into(),
            chunk_index,
            note,
            created: chrono::Utc::now(),
        });
        id
    }

    /// Add a note
    pub fn add_note(
        &mut self,
        book_id: impl Into<String>,
        content: impl Into<String>,
        chunk_index: Option<i64>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.notes.push(Note {
            id,
            book_id: book_id.into(),
            chunk_index,
            content: content.into(),
            created: chrono::Utc::now(),
        });
        id
    }

    /// Record a search query
    pub fn record_search(
        &mut self,
        query: impl Into<String>,
        filters: Option<crate::search::SearchFilters>,
        result_count: usize,
    ) {
        // Keep only last 100 searches
        if self.recent_searches.len() >= 100 {
            self.recent_searches.remove(0);
        }

        self.recent_searches.push(SearchQuery {
            query: query.into(),
            filters,
            result_count,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Update favorite authors based on searches and bookmarks
    pub fn update_favorites(&mut self, author: impl Into<String>) {
        let author = author.into();
        if !self.favorite_authors.contains(&author) {
            self.favorite_authors.push(author);
        }
        // Keep top 10
        if self.favorite_authors.len() > 10 {
            self.favorite_authors.remove(0);
        }
    }

    /// Get context for LLM prompts (recent conversation summary)
    pub fn get_context_summary(&self) -> String {
        let mut context = String::new();

        // Recent conversation
        if !self.conversation.is_empty() {
            context.push_str("Recent conversation:\n");
            for turn in self.conversation.iter().rev().take(3).rev() {
                context.push_str(&format!("  User: {}\n", turn.user));
                context.push_str(&format!("  Assistant: {}\n", turn.assistant));
            }
            context.push('\n');
        }

        // Currently reading
        if let Some(book_id) = &self.current_book {
            if let Some(progress) = self.reading_progress.get(book_id) {
                context.push_str(&format!(
                    "Currently reading: {} (last at chunk {})\n",
                    progress.title, progress.last_chunk_index
                ));
            }
        }

        // Recent bookmarks
        if !self.bookmarks.is_empty() {
            context.push_str(&format!("Recent bookmarks: {}\n", self.bookmarks.len()));
        }

        context
    }
}

/// Memory manager with file-based persistence
pub struct MemoryManager {
    /// Directory for storing memory files
    data_dir: PathBuf,
    /// In-memory cache
    cache: Arc<RwLock<HashMap<String, LibrarianMemory>>>,
}

impl MemoryManager {
    /// Create a new memory manager with file storage
    pub fn new(data_dir: PathBuf) -> Self {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            data_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create an in-memory only manager (for testing)
    pub fn in_memory() -> Self {
        Self {
            data_dir: PathBuf::from("/dev/null"),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the file path for a user's memory
    fn memory_path(&self, user_id: &str) -> PathBuf {
        self.data_dir.join(format!("{}.json", user_id))
    }

    /// Load memory for a user (creates new if doesn't exist)
    pub async fn load(&self, user_id: &str) -> Result<LibrarianMemory> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(memory) = cache.get(user_id) {
                return Ok(memory.clone());
            }
        }

        // Try to load from file
        let path = self.memory_path(user_id);
        let memory = if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            serde_json::from_str(&content).unwrap_or_else(|_| LibrarianMemory::new(user_id))
        } else {
            LibrarianMemory::new(user_id)
        };

        // Cache it
        let mut cache = self.cache.write().await;
        cache.insert(user_id.to_string(), memory.clone());

        Ok(memory)
    }

    /// Save memory for a user
    pub async fn save(&self, memory: &LibrarianMemory) -> Result<()> {
        // Save to file (unless in-memory mode)
        if self.data_dir != Path::new("/dev/null") {
            let path = self.memory_path(&memory.user_id);
            let content = serde_json::to_string_pretty(memory)?;
            tokio::fs::write(&path, content).await?;
        }

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(memory.user_id.clone(), memory.clone());

        Ok(())
    }

    /// Clear cache (useful for testing)
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_memory_add_turn() {
        let mut memory = LibrarianMemory::new("test_user");
        memory.add_turn(
            "What is Pride and Prejudice about?",
            "It's a novel by Jane Austen...",
            vec!["pride_prejudice".to_string()],
        );

        assert_eq!(memory.conversation.len(), 1);
        assert_eq!(
            memory.conversation[0].user,
            "What is Pride and Prejudice about?"
        );
    }

    #[test]
    fn test_memory_reading_progress() {
        let mut memory = LibrarianMemory::new("test_user");
        memory.update_reading_progress(
            "1342",
            "Pride and Prejudice",
            100,
            Some("Chapter 10".to_string()),
        );

        assert_eq!(memory.current_book, Some("1342".to_string()));
        assert!(memory.reading_progress.contains_key("1342"));
    }

    #[test]
    fn test_memory_bookmark() {
        let mut memory = LibrarianMemory::new("test_user");
        let id = memory.add_bookmark("1342", 50, Some("Important passage".to_string()));

        assert_eq!(memory.bookmarks.len(), 1);
        assert_eq!(memory.bookmarks[0].id, id);
    }

    #[tokio::test]
    async fn test_memory_manager_in_memory() {
        let manager = MemoryManager::in_memory();

        // Load creates new
        let memory = manager.load("test_user").await.unwrap();
        assert_eq!(memory.user_id, "test_user");

        // Modify and save
        let mut memory = memory;
        memory.add_turn("Hello", "Hi there!", vec![]);
        manager.save(&memory).await.unwrap();

        // Clear cache and reload (from cache since in-memory mode)
        let memory = manager.load("test_user").await.unwrap();
        assert_eq!(memory.conversation.len(), 1);
    }
}
