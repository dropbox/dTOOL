//! Graph state management
//!
//! State flows through the graph, being transformed by each node.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A value in the graph state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StateValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<StateValue>),
    Object(HashMap<String, StateValue>),
    /// Binary data (base64 encoded in JSON)
    Binary(Vec<u8>),
}

impl Default for StateValue {
    fn default() -> Self {
        StateValue::Null
    }
}

impl From<bool> for StateValue {
    fn from(v: bool) -> Self {
        StateValue::Bool(v)
    }
}

impl From<i64> for StateValue {
    fn from(v: i64) -> Self {
        StateValue::Int(v)
    }
}

impl From<f64> for StateValue {
    fn from(v: f64) -> Self {
        StateValue::Float(v)
    }
}

impl From<String> for StateValue {
    fn from(v: String) -> Self {
        StateValue::String(v)
    }
}

impl From<&str> for StateValue {
    fn from(v: &str) -> Self {
        StateValue::String(v.to_string())
    }
}

impl<T: Into<StateValue>> From<Vec<T>> for StateValue {
    fn from(v: Vec<T>) -> Self {
        StateValue::Array(v.into_iter().map(Into::into).collect())
    }
}

/// The complete state of a graph execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphState {
    /// Named values in the state
    values: HashMap<String, StateValue>,
    /// Message history (for chat-style graphs)
    messages: Vec<Message>,
    /// Execution metadata
    metadata: StateMetadata,
}

impl GraphState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&StateValue> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<StateValue>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: &str) -> Option<StateValue> {
        self.values.remove(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    pub fn values(&self) -> &HashMap<String, StateValue> {
        &self.values
    }

    /// Add a message to the history
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Get message history
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get execution metadata
    pub fn metadata(&self) -> &StateMetadata {
        &self.metadata
    }

    /// Merge another state into this one
    pub fn merge(&mut self, other: GraphState) {
        for (k, v) in other.values {
            self.values.insert(k, v);
        }
        self.messages.extend(other.messages);
    }
}

/// A message in the conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// Tool calls made in this message
    pub tool_calls: Vec<ToolCall>,
    /// Tool response (if this is a tool message)
    pub tool_response: Option<String>,
    /// Timestamp (ms since epoch)
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Execution metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateMetadata {
    /// Current node being executed
    pub current_node: Option<String>,
    /// Execution trace (node IDs in order)
    pub trace: Vec<String>,
    /// Total execution time so far (ms)
    pub elapsed_ms: u64,
    /// Number of LLM calls made
    pub llm_calls: u64,
    /// Total tokens used
    pub total_tokens: u64,
}
