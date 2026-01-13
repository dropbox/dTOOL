//! Mock tools for integration tests
//!
//! Provides standardized mock tools that can be shared across multiple test suites.

use dashflow::core::tools::{Tool, ToolInput};

/// Mock vector store search tool
///
/// Simulates searching technical documentation in a vector store.
/// Returns predefined responses for common programming concepts.
pub struct VectorStoreSearchTool;

#[async_trait::async_trait]
impl Tool for VectorStoreSearchTool {
    fn name(&self) -> &'static str {
        "vectorstore_search"
    }

    fn description(&self) -> &'static str {
        "Search technical documentation in vector store. Good for programming concepts and code examples."
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::error::Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .to_string(),
        };

        let response = if query.to_lowercase().contains("ownership") {
            "Rust Ownership: Each value has a single owner. When owner goes out of scope, value is dropped. \
             No garbage collector needed. Ownership can be transferred (moved) or borrowed."
        } else if query.to_lowercase().contains("borrow") {
            "Borrowing: References allow using a value without taking ownership. &T for immutable borrow, \
             &mut T for mutable borrow. Only one mutable OR multiple immutable borrows at a time."
        } else if query.to_lowercase().contains("lifetime") {
            "Lifetimes: Annotations that ensure references are valid. Prevent dangling references. \
             Example: fn longest<'a>(x: &'a str, y: &'a str) -> &'a str"
        } else {
            "Generic programming documentation content"
        };

        Ok(response.to_string())
    }
}

/// Mock web search tool
///
/// Simulates web searches for current events and news.
/// Returns predefined responses for common queries.
pub struct WebSearchTool;

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web for current events, news, and up-to-date information."
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::error::Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .to_string(),
        };

        let response = if query.to_lowercase().contains("rust")
            && query.to_lowercase().contains("2024")
        {
            "Rust 2024 Edition: Released in October 2024. Major features include improved async traits, \
             better error messages, and enhanced const generics."
        } else if query.to_lowercase().contains("ai") && query.to_lowercase().contains("2024") {
            "AI developments in 2024: Large language models continue advancing with improved reasoning, \
             multimodal capabilities, and efficiency improvements."
        } else {
            "Current web search results for your query"
        };

        Ok(response.to_string())
    }
}
