//! Memory system for agent conversation history and context management.
//!
//! This module provides memory implementations that store conversation history
//! for agents. Memory allows agents to maintain context across multiple interactions.
//!
//! # Implementations
//!
//! - [`BufferMemory`]: Stores all conversation history (unbounded)
//! - [`ConversationBufferWindowMemory`]: Keeps only the last N turns (bounded)
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::agents::{BufferMemory, ConversationBufferWindowMemory, Memory};
//!
//! # async fn example() -> dashflow::core::error::Result<()> {
//! // Unbounded memory
//! let mut memory = BufferMemory::new();
//! memory.save_context("What is 2+2?", "4").await?;
//!
//! // Bounded window memory (keeps last 5 exchanges)
//! let mut window_memory = ConversationBufferWindowMemory::new(5);
//! window_memory.save_context("Hello", "Hi there!").await?;
//!
//! // Load formatted context for prompts
//! let _context = memory.load_context().await?;
//! # Ok(())
//! # }
//! ```

use super::AgentConfigError;
use crate::core::error::Result;

/// Trait for agent memory implementations.
///
/// Memory stores conversation history and provides it as formatted context
/// for LLM prompts. Implementations must be thread-safe (`Send + Sync`).
#[async_trait::async_trait]
pub trait Memory: Send + Sync {
    /// Loads the conversation history as a formatted string.
    ///
    /// The format is typically "Human: ...\nAI: ..." pairs.
    async fn load_context(&self) -> Result<String>;

    /// Saves a conversation turn (input/output pair) to memory.
    async fn save_context(&mut self, input: &str, output: &str) -> Result<()>;

    /// Clears all stored history.
    async fn clear(&mut self) -> Result<()>;

    /// Returns the raw history as (input, output) pairs.
    fn get_history(&self) -> Vec<(String, String)>;
}

/// Unbounded conversation memory.
///
/// Stores all conversation history without limit. Suitable for short conversations
/// but may cause context length issues with very long conversations.
#[derive(Debug, Clone, Default)]
pub struct BufferMemory {
    history: Vec<(String, String)>,
}

impl BufferMemory {
    /// Creates a new empty buffer memory.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Creates a buffer memory with pre-existing history.
    #[must_use]
    pub const fn with_history(history: Vec<(String, String)>) -> Self {
        Self { history }
    }

    fn format_history(&self) -> String {
        self.history
            .iter()
            .map(|(i, o)| format!("Human: {i}\nAI: {o}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait::async_trait]
impl Memory for BufferMemory {
    async fn load_context(&self) -> Result<String> {
        Ok(self.format_history())
    }
    async fn save_context(&mut self, input: &str, output: &str) -> Result<()> {
        self.history.push((input.to_string(), output.to_string()));
        Ok(())
    }
    async fn clear(&mut self) -> Result<()> {
        self.history.clear();
        Ok(())
    }
    fn get_history(&self) -> Vec<(String, String)> {
        self.history.clone()
    }
}

/// Bounded sliding-window conversation memory.
///
/// Keeps only the most recent N conversation turns, automatically discarding
/// older entries. This prevents context length overflow for long conversations.
#[derive(Debug, Clone)]
pub struct ConversationBufferWindowMemory {
    window_size: usize,
    history: Vec<(String, String)>,
}

impl ConversationBufferWindowMemory {
    /// Creates a new window memory with the specified size.
    ///
    /// # Panics
    ///
    /// Panics if `window_size` is 0. Use [`try_new`] for a fallible alternative.
    ///
    /// [`try_new`]: Self::try_new
    #[must_use]
    pub fn new(window_size: usize) -> Self {
        Self::try_new(window_size).expect("Window size must be greater than 0")
    }

    /// Creates a new window memory, returning an error if size is invalid.
    ///
    /// # Errors
    ///
    /// Returns [`AgentConfigError::InvalidWindowSize`] if `window_size` is 0.
    pub fn try_new(window_size: usize) -> std::result::Result<Self, AgentConfigError> {
        if window_size == 0 {
            return Err(AgentConfigError::InvalidWindowSize { size: window_size });
        }
        Ok(Self {
            window_size,
            history: Vec::new(),
        })
    }

    /// Creates a window memory with pre-existing history.
    ///
    /// If history exceeds `window_size`, only the most recent entries are kept.
    ///
    /// # Panics
    ///
    /// Panics if `window_size` is 0.
    #[must_use]
    pub fn with_history(window_size: usize, history: Vec<(String, String)>) -> Self {
        Self::try_with_history(window_size, history).expect("Window size must be greater than 0")
    }

    /// Creates a window memory with history, returning an error if size is invalid.
    ///
    /// # Errors
    ///
    /// Returns [`AgentConfigError::InvalidWindowSize`] if `window_size` is 0.
    pub fn try_with_history(
        window_size: usize,
        mut history: Vec<(String, String)>,
    ) -> std::result::Result<Self, AgentConfigError> {
        if window_size == 0 {
            return Err(AgentConfigError::InvalidWindowSize { size: window_size });
        }
        if history.len() > window_size {
            history.drain(0..history.len() - window_size);
        }
        Ok(Self {
            window_size,
            history,
        })
    }

    fn format_history(&self) -> String {
        self.history
            .iter()
            .map(|(i, o)| format!("Human: {i}\nAI: {o}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait::async_trait]
impl Memory for ConversationBufferWindowMemory {
    async fn load_context(&self) -> Result<String> {
        Ok(self.format_history())
    }
    async fn save_context(&mut self, input: &str, output: &str) -> Result<()> {
        self.history.push((input.to_string(), output.to_string()));
        if self.history.len() > self.window_size {
            self.history.remove(0);
        }
        Ok(())
    }
    async fn clear(&mut self) -> Result<()> {
        self.history.clear();
        Ok(())
    }
    fn get_history(&self) -> Vec<(String, String)> {
        self.history.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================== BufferMemory =====================

    #[test]
    fn buffer_memory_new_is_empty() {
        let mem = BufferMemory::new();
        assert!(mem.history.is_empty());
    }

    #[test]
    fn buffer_memory_default_is_empty() {
        let mem = BufferMemory::default();
        assert!(mem.history.is_empty());
    }

    #[test]
    fn buffer_memory_with_history_preserves_entries() {
        let history = vec![
            ("Hello".to_string(), "Hi there!".to_string()),
            ("How are you?".to_string(), "I'm good.".to_string()),
        ];
        let mem = BufferMemory::with_history(history.clone());
        assert_eq!(mem.history, history);
    }

    #[test]
    fn buffer_memory_format_history_empty() {
        let mem = BufferMemory::new();
        assert!(mem.format_history().is_empty());
    }

    #[test]
    fn buffer_memory_format_history_single() {
        let mem = BufferMemory::with_history(vec![
            ("Hello".to_string(), "Hi!".to_string()),
        ]);
        let formatted = mem.format_history();
        assert!(formatted.contains("Human: Hello"));
        assert!(formatted.contains("AI: Hi!"));
    }

    #[test]
    fn buffer_memory_format_history_multiple() {
        let mem = BufferMemory::with_history(vec![
            ("First".to_string(), "Response1".to_string()),
            ("Second".to_string(), "Response2".to_string()),
        ]);
        let formatted = mem.format_history();
        assert!(formatted.contains("Human: First"));
        assert!(formatted.contains("AI: Response1"));
        assert!(formatted.contains("Human: Second"));
        assert!(formatted.contains("AI: Response2"));
    }

    #[tokio::test]
    async fn buffer_memory_load_context_returns_formatted_history() {
        let mem = BufferMemory::with_history(vec![
            ("Hello".to_string(), "Hi!".to_string()),
        ]);
        let context = mem.load_context().await.expect("load_context");
        assert!(context.contains("Human: Hello"));
        assert!(context.contains("AI: Hi!"));
    }

    #[tokio::test]
    async fn buffer_memory_save_context_appends() {
        let mut mem = BufferMemory::new();
        mem.save_context("Q1", "A1").await.expect("save");
        mem.save_context("Q2", "A2").await.expect("save");
        assert_eq!(mem.history.len(), 2);
        assert_eq!(mem.history[0], ("Q1".to_string(), "A1".to_string()));
        assert_eq!(mem.history[1], ("Q2".to_string(), "A2".to_string()));
    }

    #[tokio::test]
    async fn buffer_memory_clear_removes_all() {
        let mut mem = BufferMemory::with_history(vec![
            ("Q".to_string(), "A".to_string()),
        ]);
        mem.clear().await.expect("clear");
        assert!(mem.history.is_empty());
    }

    #[test]
    fn buffer_memory_get_history_returns_clone() {
        let mem = BufferMemory::with_history(vec![
            ("Q".to_string(), "A".to_string()),
        ]);
        let history = mem.get_history();
        assert_eq!(history, mem.history);
    }

    // ===================== ConversationBufferWindowMemory =====================

    #[test]
    fn window_memory_new_creates_with_size() {
        let mem = ConversationBufferWindowMemory::new(5);
        assert_eq!(mem.window_size, 5);
        assert!(mem.history.is_empty());
    }

    #[test]
    fn window_memory_try_new_success() {
        let result = ConversationBufferWindowMemory::try_new(3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().window_size, 3);
    }

    #[test]
    fn window_memory_try_new_zero_fails() {
        let result = ConversationBufferWindowMemory::try_new(0);
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentConfigError::InvalidWindowSize { size } => assert_eq!(size, 0),
            _ => panic!("Expected InvalidWindowSize error"),
        }
    }

    #[test]
    #[should_panic(expected = "Window size must be greater than 0")]
    fn window_memory_new_zero_panics() {
        let _ = ConversationBufferWindowMemory::new(0);
    }

    #[test]
    fn window_memory_with_history_preserves_under_limit() {
        let history = vec![
            ("Q1".to_string(), "A1".to_string()),
            ("Q2".to_string(), "A2".to_string()),
        ];
        let mem = ConversationBufferWindowMemory::with_history(5, history.clone());
        assert_eq!(mem.history, history);
    }

    #[test]
    fn window_memory_with_history_truncates_over_limit() {
        let history = vec![
            ("Q1".to_string(), "A1".to_string()),
            ("Q2".to_string(), "A2".to_string()),
            ("Q3".to_string(), "A3".to_string()),
            ("Q4".to_string(), "A4".to_string()),
        ];
        let mem = ConversationBufferWindowMemory::with_history(2, history);
        assert_eq!(mem.history.len(), 2);
        // Should keep the last 2 entries
        assert_eq!(mem.history[0].0, "Q3");
        assert_eq!(mem.history[1].0, "Q4");
    }

    #[test]
    fn window_memory_try_with_history_success() {
        let history = vec![("Q".to_string(), "A".to_string())];
        let result = ConversationBufferWindowMemory::try_with_history(5, history);
        assert!(result.is_ok());
    }

    #[test]
    fn window_memory_try_with_history_zero_fails() {
        let history = vec![("Q".to_string(), "A".to_string())];
        let result = ConversationBufferWindowMemory::try_with_history(0, history);
        assert!(result.is_err());
    }

    #[test]
    fn window_memory_format_history_empty() {
        let mem = ConversationBufferWindowMemory::new(3);
        assert!(mem.format_history().is_empty());
    }

    #[test]
    fn window_memory_format_history_formats_correctly() {
        let mem = ConversationBufferWindowMemory::with_history(5, vec![
            ("Hello".to_string(), "Hi!".to_string()),
        ]);
        let formatted = mem.format_history();
        assert!(formatted.contains("Human: Hello"));
        assert!(formatted.contains("AI: Hi!"));
    }

    #[tokio::test]
    async fn window_memory_load_context_returns_formatted() {
        let mem = ConversationBufferWindowMemory::with_history(5, vec![
            ("Q".to_string(), "A".to_string()),
        ]);
        let context = mem.load_context().await.expect("load");
        assert!(context.contains("Human: Q"));
        assert!(context.contains("AI: A"));
    }

    #[tokio::test]
    async fn window_memory_save_context_appends() {
        let mut mem = ConversationBufferWindowMemory::new(5);
        mem.save_context("Q1", "A1").await.expect("save");
        mem.save_context("Q2", "A2").await.expect("save");
        assert_eq!(mem.history.len(), 2);
    }

    #[tokio::test]
    async fn window_memory_save_context_enforces_window() {
        let mut mem = ConversationBufferWindowMemory::new(2);
        mem.save_context("Q1", "A1").await.expect("save");
        mem.save_context("Q2", "A2").await.expect("save");
        mem.save_context("Q3", "A3").await.expect("save");

        assert_eq!(mem.history.len(), 2);
        // Should have removed Q1 and kept Q2, Q3
        assert_eq!(mem.history[0].0, "Q2");
        assert_eq!(mem.history[1].0, "Q3");
    }

    #[tokio::test]
    async fn window_memory_window_size_1_keeps_only_last() {
        let mut mem = ConversationBufferWindowMemory::new(1);
        mem.save_context("First", "R1").await.expect("save");
        mem.save_context("Second", "R2").await.expect("save");
        mem.save_context("Third", "R3").await.expect("save");

        assert_eq!(mem.history.len(), 1);
        assert_eq!(mem.history[0].0, "Third");
    }

    #[tokio::test]
    async fn window_memory_clear_removes_all() {
        let mut mem = ConversationBufferWindowMemory::with_history(5, vec![
            ("Q".to_string(), "A".to_string()),
        ]);
        mem.clear().await.expect("clear");
        assert!(mem.history.is_empty());
    }

    #[test]
    fn window_memory_get_history_returns_clone() {
        let mem = ConversationBufferWindowMemory::with_history(5, vec![
            ("Q".to_string(), "A".to_string()),
        ]);
        let history = mem.get_history();
        assert_eq!(history.len(), 1);
    }

    // ===================== Memory Trait Common Behavior =====================

    #[tokio::test]
    async fn buffer_and_window_both_impl_memory() {
        // This is a compile-time check that both types implement Memory
        let _buf: Box<dyn Memory> = Box::new(BufferMemory::new());
        let _win: Box<dyn Memory> = Box::new(ConversationBufferWindowMemory::new(5));
    }

    #[tokio::test]
    async fn memory_trait_polymorphic_usage() {
        async fn use_memory(mem: &mut dyn Memory) {
            mem.save_context("Test", "Result").await.expect("save");
            let _ = mem.load_context().await.expect("load");
        }

        let mut buf = BufferMemory::new();
        use_memory(&mut buf).await;

        let mut win = ConversationBufferWindowMemory::new(5);
        use_memory(&mut win).await;
    }
}
