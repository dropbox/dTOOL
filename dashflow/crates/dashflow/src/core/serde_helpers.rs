//! Serialization and deserialization helpers
//!
//! This module provides convenience functions for saving and loading DashFlow objects
//! to/from JSON and other formats.
//!
//! # Supported Types
//!
//! All core data types support JSON serialization via serde:
//! - [`Document`](crate::core::documents::Document) - Documents with metadata
//! - [`Message`](crate::core::messages::Message) - Chat messages
//! - [`PromptTemplate`](crate::core::prompts::PromptTemplate) - Prompt templates
//! - [`Tool`](crate::core::tools::Tool) - Tool schemas
//! - [`ChatGeneration`](crate::core::language_models::ChatGeneration) - LLM responses
//!
//! # Example: Save and Load Documents
//!
//! ```rust
//! use dashflow::core::documents::Document;
//! use dashflow::core::serde_helpers::{to_json, from_json};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let doc = Document::new("Hello, world!")
//!     .with_metadata("source", "test.txt");
//!
//! // Serialize to JSON string
//! let json = to_json(&doc)?;
//! println!("{}", json); // {"page_content":"Hello, world!","metadata":{"source":"test.txt"}}
//!
//! // Deserialize from JSON string
//! let loaded: Document = from_json(&json)?;
//! assert_eq!(loaded.page_content, doc.page_content);
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Pretty-Print JSON
//!
//! ```rust
//! use dashflow::core::documents::Document;
//! use dashflow::core::serde_helpers::to_json_pretty;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let doc = Document::new("Hello, world!")
//!     .with_metadata("source", "test.txt");
//!
//! let json = to_json_pretty(&doc)?;
//! println!("{}", json);
//! // {
//! //   "page_content": "Hello, world!",
//! //   "metadata": {
//! //     "source": "test.txt"
//! //   }
//! // }
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Save to File
//!
//! ```rust,ignore
//! use dashflow::core::documents::Document;
//! use dashflow::core::serde_helpers::save_json;
//!
//! let doc = Document::new("Hello, world!");
//! save_json(&doc, "document.json")?;
//! ```
//!
//! # Example: Load from File
//!
//! ```rust,ignore
//! use dashflow::core::documents::Document;
//! use dashflow::core::serde_helpers::load_json;
//!
//! let doc: Document = load_json("document.json")?;
//! ```
//!
//! # Example: Batch Documents
//!
//! ```rust
//! use dashflow::core::documents::Document;
//! use dashflow::core::serde_helpers::{to_json, from_json};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let docs = vec![
//!     Document::new("Document 1"),
//!     Document::new("Document 2"),
//!     Document::new("Document 3"),
//! ];
//!
//! // Serialize collection
//! let json = to_json(&docs)?;
//!
//! // Deserialize collection
//! let loaded: Vec<Document> = from_json(&json)?;
//! assert_eq!(loaded.len(), 3);
//! # Ok(())
//! # }
//! ```
//!
//! # Notes
//!
//! - This module provides convenient JSON serialization for data types
//! - For full DashFlow-compatible serialization (chains, models with secrets),
//!   see future `dashflow::core::load` module
//! - File operations use UTF-8 encoding
//! - Secrets (API keys) should not be serialized - configure via environment variables

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::core::error::{Error, Result};

/// Serialize a value to a JSON string
///
/// # Arguments
///
/// * `value` - Any serializable value
///
/// # Returns
///
/// Compact JSON string representation
///
/// # Errors
///
/// Returns error if serialization fails
///
/// # Example
///
/// ```rust
/// use dashflow::core::documents::Document;
/// use dashflow::core::serde_helpers::to_json;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let doc = Document::new("Hello");
/// let json = to_json(&doc)?;
/// assert!(json.contains("Hello"));
/// # Ok(())
/// # }
/// ```
pub fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(Error::Serialization)
}

/// Serialize a value to a pretty-printed JSON string
///
/// # Arguments
///
/// * `value` - Any serializable value
///
/// # Returns
///
/// Pretty-printed JSON string with 2-space indentation
///
/// # Errors
///
/// Returns error if serialization fails
///
/// # Example
///
/// ```rust
/// use dashflow::core::documents::Document;
/// use dashflow::core::serde_helpers::to_json_pretty;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let doc = Document::new("Hello");
/// let json = to_json_pretty(&doc)?;
/// assert!(json.contains("  ")); // Has indentation
/// # Ok(())
/// # }
/// ```
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(Error::Serialization)
}

/// Deserialize a value from a JSON string
///
/// # Arguments
///
/// * `json` - JSON string to deserialize
///
/// # Returns
///
/// Deserialized value of type T
///
/// # Errors
///
/// Returns error if deserialization fails or JSON is invalid
///
/// # Example
///
/// ```rust
/// use dashflow::core::documents::Document;
/// use dashflow::core::serde_helpers::{to_json, from_json};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let json = r#"{"page_content":"Hello","metadata":{}}"#;
/// let doc: Document = from_json(json)?;
/// assert_eq!(doc.page_content, "Hello");
/// # Ok(())
/// # }
/// ```
pub fn from_json<'a, T: Deserialize<'a>>(json: &'a str) -> Result<T> {
    serde_json::from_str(json).map_err(Error::Serialization)
}

/// Save a value to a JSON file
///
/// # Arguments
///
/// * `value` - Any serializable value
/// * `path` - File path to write to (will be created or overwritten)
///
/// # Returns
///
/// Ok(()) on success
///
/// # Errors
///
/// Returns error if serialization or file write fails
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::Document;
/// use dashflow::core::serde_helpers::save_json;
///
/// let doc = Document::new("Hello");
/// save_json(&doc, "output.json")?;
/// ```
pub fn save_json<T: Serialize, P: AsRef<Path>>(value: &T, path: P) -> Result<()> {
    let json = to_json_pretty(value)?;
    fs::write(path, json).map_err(Error::Io)
}

/// Load a value from a JSON file
///
/// # Arguments
///
/// * `path` - File path to read from
///
/// # Returns
///
/// Deserialized value of type T
///
/// # Errors
///
/// Returns error if file read, deserialization, or JSON parsing fails
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::Document;
/// use dashflow::core::serde_helpers::load_json;
///
/// let doc: Document = load_json("input.json")?;
/// ```
pub fn load_json<T: for<'a> Deserialize<'a>, P: AsRef<Path>>(path: P) -> Result<T> {
    let json = fs::read_to_string(path).map_err(Error::Io)?;
    from_json(&json)
}

#[cfg(test)]
mod tests {
    use crate::core::documents::Document;
    use crate::core::messages::Message;
    use crate::test_prelude::*;

    #[test]
    fn test_to_json_document() {
        let doc = Document::new("Test content").with_metadata("key", "value");
        let json = to_json(&doc).unwrap();
        assert!(json.contains("Test content"));
        assert!(json.contains("key"));
    }

    #[test]
    fn test_to_json_pretty_document() {
        let doc = Document::new("Test");
        let json = to_json_pretty(&doc).unwrap();
        assert!(json.contains("  ")); // Check for indentation
        assert!(json.contains("page_content"));
    }

    #[test]
    fn test_from_json_document() {
        let json = r#"{"page_content":"Hello","metadata":{"source":"test"}}"#;
        let doc: Document = from_json(json).unwrap();
        assert_eq!(doc.page_content, "Hello");
        assert_eq!(doc.metadata.get("source").unwrap(), "test");
    }

    #[test]
    fn test_roundtrip_document() {
        let original = Document::new("Test content")
            .with_metadata("source", "test.txt")
            .with_metadata("author", "Alice");

        let json = to_json(&original).unwrap();
        let loaded: Document = from_json(&json).unwrap();

        assert_eq!(loaded.page_content, original.page_content);
        assert_eq!(loaded.metadata, original.metadata);
    }

    #[test]
    fn test_roundtrip_vec_documents() {
        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];

        let json = to_json(&docs).unwrap();
        let loaded: Vec<Document> = from_json(&json).unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].page_content, "Doc 1");
        assert_eq!(loaded[1].page_content, "Doc 2");
        assert_eq!(loaded[2].page_content, "Doc 3");
    }

    #[test]
    fn test_to_json_message() {
        let msg = Message::human("Hello, AI!");
        let json = to_json(&msg).unwrap();
        assert!(json.contains("Hello, AI!"));
    }

    #[test]
    fn test_roundtrip_message() {
        let original = Message::ai("I am an AI assistant");
        let json = to_json(&original).unwrap();
        let loaded: Message = from_json(&json).unwrap();

        assert_eq!(loaded.content().as_text(), original.content().as_text());
    }

    #[test]
    fn test_roundtrip_vec_messages() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::human("Hi"),
            Message::ai("Hello!"),
        ];

        let json = to_json(&messages).unwrap();
        let loaded: Vec<Message> = from_json(&json).unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].content().as_text(), "You are helpful");
        assert_eq!(loaded[1].content().as_text(), "Hi");
        assert_eq!(loaded[2].content().as_text(), "Hello!");
    }

    #[test]
    fn test_invalid_json() {
        let result: Result<Document> = from_json("invalid json");
        assert!(result.is_err());
    }
}
