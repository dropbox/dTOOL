//! StreamingMarkdown component for incremental markdown rendering.
//!
//! Optimized for LLM token streaming where markdown arrives in chunks and may
//! be incomplete at any moment. The component re-renders the full markdown
//! content each frame and includes helpers for tracking appended content.
//!
//! # Example
//!
//! ```ignore
//! use inky::components::StreamingMarkdown;
//!
//! let stream = StreamingMarkdown::new();
//! let handle = stream.handle();
//!
//! handle.append("# Title\n\n");
//! handle.append("```rust\nfn main() {}\n");
//!
//! let node = stream.to_node();
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::hooks::request_render;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::Color;

use super::markdown::{CodeTheme, Markdown};

/// Handle for appending markdown from another thread/task.
#[derive(Clone)]
pub struct StreamingMarkdownHandle {
    buffer: Arc<RwLock<String>>,
    rendered_len: Arc<AtomicUsize>,
}

impl StreamingMarkdownHandle {
    /// Append markdown text to the stream.
    pub fn append(&self, text: &str) {
        {
            let mut buffer = self
                .buffer
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            buffer.push_str(text);
        }
        request_render();
    }

    /// Append multiple markdown fragments at once.
    pub fn append_batch(&self, texts: impl IntoIterator<Item = impl AsRef<str>>) {
        {
            let mut buffer = self
                .buffer
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for text in texts {
                buffer.push_str(text.as_ref());
            }
        }
        request_render();
    }

    /// Clear the markdown content.
    pub fn clear(&self) {
        {
            let mut buffer = self
                .buffer
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            buffer.clear();
        }
        self.rendered_len.store(0, Ordering::SeqCst);
        request_render();
    }

    /// Get the current content length.
    pub fn len(&self) -> usize {
        self.buffer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    /// Check if the stream is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a copy of the current content.
    pub fn content(&self) -> String {
        self.buffer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Check if there is new content since last render.
    pub fn has_new_content(&self) -> bool {
        let current_len = self.len();
        let rendered = self.rendered_len.load(Ordering::SeqCst);
        current_len > rendered
    }

    /// Get only the new content since last render (for incremental updates).
    pub fn take_new_content(&self) -> Option<String> {
        let buffer = self
            .buffer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current_len = buffer.len();
        let rendered = self.rendered_len.load(Ordering::SeqCst);

        if current_len > rendered {
            let new_content = buffer[rendered..].to_string();
            self.rendered_len.store(current_len, Ordering::SeqCst);
            Some(new_content)
        } else {
            None
        }
    }

    /// Mark all current content as rendered.
    pub fn mark_rendered(&self) {
        let len = self.len();
        self.rendered_len.store(len, Ordering::SeqCst);
    }
}

/// StreamingMarkdown component for efficient markdown streaming.
#[derive(Clone)]
pub struct StreamingMarkdown {
    buffer: Arc<RwLock<String>>,
    rendered_len: Arc<AtomicUsize>,
    code_theme: CodeTheme,
    placeholder: Option<String>,
    placeholder_color: Color,
}

impl Default for StreamingMarkdown {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingMarkdown {
    /// Create a new streaming markdown component.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(RwLock::new(String::new())),
            rendered_len: Arc::new(AtomicUsize::new(0)),
            code_theme: CodeTheme::default(),
            placeholder: None,
            placeholder_color: Color::BrightBlack,
        }
    }

    /// Create a streaming markdown component with initial content.
    pub fn with_content(content: impl Into<String>) -> Self {
        let content = content.into();
        let len = content.len();
        Self {
            buffer: Arc::new(RwLock::new(content)),
            rendered_len: Arc::new(AtomicUsize::new(len)),
            code_theme: CodeTheme::default(),
            placeholder: None,
            placeholder_color: Color::BrightBlack,
        }
    }

    /// Get a handle for appending content from another thread.
    pub fn handle(&self) -> StreamingMarkdownHandle {
        StreamingMarkdownHandle {
            buffer: Arc::clone(&self.buffer),
            rendered_len: Arc::clone(&self.rendered_len),
        }
    }

    /// Append markdown directly (for same-thread use).
    pub fn append(&self, text: &str) {
        {
            let mut buffer = self
                .buffer
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            buffer.push_str(text);
        }
        request_render();
    }

    /// Clear the content.
    pub fn clear(&self) {
        {
            let mut buffer = self
                .buffer
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            buffer.clear();
        }
        self.rendered_len.store(0, Ordering::SeqCst);
        request_render();
    }

    /// Get the current content.
    pub fn content(&self) -> String {
        self.buffer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Get the content length.
    pub fn len(&self) -> usize {
        self.buffer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set the code theme for code blocks.
    pub fn code_theme(mut self, theme: CodeTheme) -> Self {
        self.code_theme = theme;
        self
    }

    /// Set placeholder text for empty state.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set placeholder color.
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = color;
        self
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let content = self.content();

        if content.is_empty() {
            if let Some(ref placeholder) = self.placeholder {
                return TextNode::new(placeholder.clone())
                    .color(self.placeholder_color)
                    .dim()
                    .into();
            }
            return TextNode::new("").into();
        }

        self.rendered_len.store(content.len(), Ordering::SeqCst);

        Markdown::new(content).code_theme(self.code_theme).to_node()
    }

    /// Convert to a Node wrapped in a box with fixed dimensions.
    pub fn to_node_with_size(self, width: u16, height: u16) -> Node {
        BoxNode::new()
            .width(width)
            .height(height)
            .child(self.to_node())
            .into()
    }
}

impl From<StreamingMarkdown> for Node {
    fn from(stream: StreamingMarkdown) -> Node {
        stream.to_node()
    }
}

impl From<&StreamingMarkdown> for Node {
    fn from(stream: &StreamingMarkdown) -> Node {
        stream.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn node_to_text(node: &Node) -> String {
        match node {
            Node::Text(t) => t.content.to_string(),
            Node::Box(b) => {
                let mut result = String::new();
                for child in &b.children {
                    result.push_str(&node_to_text(child));
                }
                result
            }
            Node::Root(r) => {
                let mut result = String::new();
                for child in &r.children {
                    result.push_str(&node_to_text(child));
                }
                result
            }
            Node::Static(s) => {
                let mut result = String::new();
                for child in &s.children {
                    result.push_str(&node_to_text(child));
                }
                result
            }
            Node::Custom(c) => {
                let mut result = String::new();
                for child in c.widget().children() {
                    result.push_str(&node_to_text(child));
                }
                result
            }
        }
    }

    #[test]
    fn test_streaming_markdown_basic() {
        let stream = StreamingMarkdown::new();
        assert!(stream.is_empty());

        stream.append("# Title");
        assert_eq!(stream.len(), 7);
        assert_eq!(stream.content(), "# Title");
    }

    #[test]
    fn test_streaming_markdown_handle_tracking() {
        let stream = StreamingMarkdown::new();
        let handle = stream.handle();

        assert!(!handle.has_new_content());

        handle.append("Hello");
        assert!(handle.has_new_content());
        assert_eq!(handle.take_new_content(), Some("Hello".to_string()));
        assert!(!handle.has_new_content());
    }

    #[test]
    fn test_streaming_markdown_incomplete_code_block() {
        let stream = StreamingMarkdown::new();
        stream.append("```rust\nfn main() {\n    println!(\"hi\");\n}\n");

        let node = stream.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("fn main"));
        assert!(text.contains("rust"));
    }

    #[test]
    fn test_streaming_markdown_placeholder() {
        let stream = StreamingMarkdown::new().placeholder("Waiting...");
        let node = stream.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Waiting..."));
    }
}
