// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Conversation history type for multi-turn interactions

use serde::{Deserialize, Serialize};

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System message (instructions)
    System,
    /// User message
    User,
    /// Assistant/AI message
    Assistant,
    /// Tool/function result message
    Tool,
}

impl Role {
    /// Get string representation for LLM APIs
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Message role
    pub role: Role,

    /// Message content
    pub content: String,

    /// Optional name (for multi-agent scenarios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional tool call ID (for tool responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a new message
    ///
    /// # Arguments
    /// * `role` - Message role
    /// * `content` - Message content
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            name: None,
            tool_call_id: None,
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Create a tool result message
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        let mut msg = Self::new(Role::Tool, content);
        msg.tool_call_id = Some(tool_call_id.into());
        msg
    }

    /// Set message name
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Check if this is a system message
    pub fn is_system(&self) -> bool {
        matches!(self.role, Role::System)
    }

    /// Check if this is a user message
    pub fn is_user(&self) -> bool {
        matches!(self.role, Role::User)
    }

    /// Check if this is an assistant message
    pub fn is_assistant(&self) -> bool {
        matches!(self.role, Role::Assistant)
    }

    /// Check if this is a tool message
    pub fn is_tool(&self) -> bool {
        matches!(self.role, Role::Tool)
    }

    /// Get content length
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.role, self.content)
    }
}

/// Conversation history container
///
/// Maintains a sequence of messages for multi-turn conversations,
/// with utilities for context management and truncation.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::{History, Message};
///
/// let mut history = History::new();
/// history.add(Message::system("You are a helpful assistant."));
/// history.add(Message::user("What is 2 + 2?"));
/// history.add(Message::assistant("2 + 2 equals 4."));
///
/// // Get recent context
/// let context = history.last_n(2);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    /// Messages in chronological order
    messages: Vec<Message>,

    /// Optional maximum token/character limit
    #[serde(skip_serializing_if = "Option::is_none")]
    max_chars: Option<usize>,
}

impl History {
    /// Create empty history
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_chars: None,
        }
    }

    /// Create history with messages
    pub fn from_messages(messages: Vec<Message>) -> Self {
        Self {
            messages,
            max_chars: None,
        }
    }

    /// Create history with system message
    #[must_use]
    pub fn with_system(system: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::system(system)],
            max_chars: None,
        }
    }

    /// Set maximum character limit for context
    #[must_use]
    pub fn with_max_chars(mut self, max: usize) -> Self {
        self.max_chars = Some(max);
        self
    }

    /// Add a message to history
    pub fn add(&mut self, message: Message) {
        self.messages.push(message);

        // Auto-truncate if limit is set
        if let Some(max) = self.max_chars {
            self.truncate_to_chars(max);
        }
    }

    /// Add a user message
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.add(Message::user(content));
    }

    /// Add an assistant message
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.add(Message::assistant(content));
    }

    /// Get number of messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get total character count
    pub fn total_chars(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get mutable messages
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Get message by index
    pub fn get(&self, index: usize) -> Option<&Message> {
        self.messages.get(index)
    }

    /// Get last message
    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get the system message if present
    pub fn system_message(&self) -> Option<&Message> {
        self.messages.first().filter(|m| m.is_system())
    }

    /// Get last N messages (preserving system message)
    pub fn last_n(&self, n: usize) -> Self {
        if n >= self.messages.len() {
            return self.clone();
        }

        let system = self.system_message().cloned();
        let skip = if system.is_some() { 1 } else { 0 };
        let recent: Vec<Message> = self
            .messages
            .iter()
            .skip(skip)
            .rev()
            .take(n.saturating_sub(skip))
            .rev()
            .cloned()
            .collect();

        let mut result = Vec::with_capacity(recent.len() + skip);
        if let Some(sys) = system {
            result.push(sys);
        }
        result.extend(recent);

        Self {
            messages: result,
            max_chars: self.max_chars,
        }
    }

    /// Truncate history to fit within character limit
    /// Preserves system message and removes oldest non-system messages
    pub fn truncate_to_chars(&mut self, max_chars: usize) {
        let system = self.system_message().cloned();
        let system_chars = system.as_ref().map(|m| m.len()).unwrap_or(0);

        if system_chars >= max_chars {
            // Only system message fits
            if let Some(sys) = system {
                self.messages = vec![sys];
            } else {
                self.messages.clear();
            }
            return;
        }

        let available = max_chars - system_chars;
        let mut kept = Vec::new();
        let mut total = 0;

        // Keep messages from end until limit reached
        let start_idx = if system.is_some() { 1 } else { 0 };
        for msg in self.messages[start_idx..].iter().rev() {
            if total + msg.len() <= available {
                kept.push(msg.clone());
                total += msg.len();
            } else {
                break;
            }
        }

        kept.reverse();

        let mut result = Vec::with_capacity(kept.len() + 1);
        if let Some(sys) = system {
            result.push(sys);
        }
        result.extend(kept);

        self.messages = result;
    }

    /// Clear all messages except system message
    pub fn clear_keep_system(&mut self) {
        let system = self.system_message().cloned();
        self.messages.clear();
        if let Some(sys) = system {
            self.messages.push(sys);
        }
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Iterate over messages
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    /// Convert to OpenAI-style message format
    pub fn to_openai_format(&self) -> Vec<serde_json::Value> {
        self.messages
            .iter()
            .map(|m| {
                let mut obj = serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content
                });
                if let Some(name) = &m.name {
                    obj["name"] = serde_json::Value::String(name.clone());
                }
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::Value::String(id.clone());
                }
                obj
            })
            .collect()
    }

    /// Convert to Anthropic-style message format
    pub fn to_anthropic_format(&self) -> (Option<String>, Vec<serde_json::Value>) {
        let system = self.system_message().map(|m| m.content.clone());

        let messages = self
            .messages
            .iter()
            .filter(|m| !m.is_system())
            .map(|m| {
                serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content
                })
            })
            .collect();

        (system, messages)
    }
}

impl IntoIterator for History {
    type Item = Message;
    type IntoIter = std::vec::IntoIter<Message>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}

impl<'a> IntoIterator for &'a History {
    type Item = &'a Message;
    type IntoIter = std::slice::Iter<'a, Message>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

impl FromIterator<Message> for History {
    fn from_iter<I: IntoIterator<Item = Message>>(iter: I) -> Self {
        Self {
            messages: iter.into_iter().collect(),
            max_chars: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_roles() {
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
        assert_eq!(Role::Tool.as_str(), "tool");
    }

    #[test]
    fn test_message_constructors() {
        let sys = Message::system("System prompt");
        assert!(sys.is_system());
        assert_eq!(sys.content, "System prompt");

        let user = Message::user("Hello");
        assert!(user.is_user());

        let asst = Message::assistant("Hi there");
        assert!(asst.is_assistant());

        let tool = Message::tool("Result", "call-123");
        assert!(tool.is_tool());
        assert_eq!(tool.tool_call_id, Some("call-123".to_string()));
    }

    #[test]
    fn test_history_add() {
        let mut history = History::new();
        history.add(Message::user("Hello"));
        history.add(Message::assistant("Hi"));

        assert_eq!(history.len(), 2);
        assert_eq!(history.total_chars(), 7);
    }

    #[test]
    fn test_history_with_system() {
        let mut history = History::with_system("You are helpful.");
        history.add_user("Question");
        history.add_assistant("Answer");

        assert_eq!(history.len(), 3);
        assert!(history.system_message().is_some());
    }

    #[test]
    fn test_history_last_n() {
        let mut history = History::with_system("System");
        history.add_user("Q1");
        history.add_assistant("A1");
        history.add_user("Q2");
        history.add_assistant("A2");

        let recent = history.last_n(3);
        assert_eq!(recent.len(), 3);
        assert!(recent.system_message().is_some());
        assert_eq!(recent.messages[1].content, "Q2");
        assert_eq!(recent.messages[2].content, "A2");
    }

    #[test]
    fn test_history_truncate_to_chars() {
        let mut history = History::with_system("Sys"); // 3 chars
        history.add_user("Hello"); // 5 chars
        history.add_assistant("World"); // 5 chars
        history.add_user("Test"); // 4 chars

        history.truncate_to_chars(12); // 3 + 5 + 4 = 12

        assert_eq!(history.len(), 3);
        assert!(history.system_message().is_some());
        assert_eq!(history.messages[1].content, "World");
        assert_eq!(history.messages[2].content, "Test");
    }

    #[test]
    fn test_history_clear_keep_system() {
        let mut history = History::with_system("System");
        history.add_user("User");
        history.add_assistant("Assistant");

        history.clear_keep_system();
        assert_eq!(history.len(), 1);
        assert!(history.system_message().is_some());
    }

    #[test]
    fn test_openai_format() {
        let mut history = History::new();
        history.add(Message::user("Hello"));
        history.add(Message::assistant("Hi"));

        let format = history.to_openai_format();
        assert_eq!(format.len(), 2);
        assert_eq!(format[0]["role"], "user");
        assert_eq!(format[1]["role"], "assistant");
    }

    #[test]
    fn test_anthropic_format() {
        let mut history = History::with_system("System prompt");
        history.add_user("Hello");

        let (system, messages) = history.to_anthropic_format();
        assert_eq!(system, Some("System prompt".to_string()));
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn test_serialization() {
        let mut history = History::new();
        history.add(Message::user("Hello"));

        let json = serde_json::to_string(&history).unwrap();
        let deserialized: History = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
    }
}
