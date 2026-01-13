//! StreamingText component for AI token streaming.
//!
//! This component provides efficient rendering of incrementally streamed text,
//! optimized for LLM token-by-token output where new content is frequently
//! appended without full re-renders.
//!
//! # Example
//!
//! ```ignore
//! use inky::prelude::*;
//! use inky::components::StreamingText;
//!
//! // Create streaming text component
//! let stream = StreamingText::new();
//!
//! // Clone handle for async task
//! let stream_handle = stream.handle();
//!
//! // In LLM response handler (another thread/task)
//! stream_handle.append("Hello ");
//! stream_handle.append("World!");
//!
//! // In render function
//! stream.to_node()
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::ansi::parse_ansi;
use crate::hooks::request_render;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, StyledSpan, TextWrap};

/// Handle for appending text from another thread/task.
///
/// This is a cheap clone of the internal buffer that can be sent to async tasks
/// or other threads for streaming LLM output.
#[derive(Clone)]
pub struct StreamingTextHandle {
    buffer: Arc<RwLock<String>>,
    /// Tracks the length that has been rendered to avoid re-rendering unchanged content.
    rendered_len: Arc<AtomicUsize>,
}

impl StreamingTextHandle {
    /// Append text to the stream.
    ///
    /// This is safe to call from any thread. Each append triggers a render request.
    /// For batch updates, use `append_batch` to reduce render overhead.
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

    /// Append multiple text fragments at once.
    ///
    /// More efficient than calling `append` repeatedly as it only triggers
    /// one render request.
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

    /// Clear the stream content.
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
            // Update rendered length
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

/// StreamingText component for efficient token-by-token rendering.
///
/// Optimized for AI assistant use cases where text is streamed incrementally
/// and must be rendered with minimal latency. Supports ANSI escape sequences
/// for colored/styled output.
///
/// # Features
///
/// - Thread-safe appending via `StreamingTextHandle`
/// - ANSI escape sequence parsing
/// - Tracks rendered content to enable incremental updates
/// - Configurable text styling (color, wrap, etc.)
///
/// # Performance
///
/// - Append operations are O(1) amortized (string push)
/// - Render generates Node tree from full content (future: incremental)
/// - Lock contention minimized by quick critical sections
#[derive(Clone)]
pub struct StreamingText {
    buffer: Arc<RwLock<String>>,
    rendered_len: Arc<AtomicUsize>,
    /// Parse ANSI escape sequences.
    parse_ansi: bool,
    /// Default text color (when not specified by ANSI).
    color: Option<Color>,
    /// Text wrapping mode.
    wrap: TextWrap,
    /// Bold text.
    bold: bool,
    /// Dim text.
    dim: bool,
    /// Italic text.
    italic: bool,
    /// Placeholder when empty.
    placeholder: Option<String>,
    /// Placeholder color.
    placeholder_color: Color,
}

impl Default for StreamingText {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingText {
    /// Create a new streaming text component.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(RwLock::new(String::new())),
            rendered_len: Arc::new(AtomicUsize::new(0)),
            parse_ansi: true,
            color: None,
            wrap: TextWrap::Wrap,
            bold: false,
            dim: false,
            italic: false,
            placeholder: None,
            placeholder_color: Color::BrightBlack,
        }
    }

    /// Create a streaming text component with initial content.
    pub fn with_content(content: impl Into<String>) -> Self {
        let content = content.into();
        let len = content.len();
        Self {
            buffer: Arc::new(RwLock::new(content)),
            rendered_len: Arc::new(AtomicUsize::new(len)),
            parse_ansi: true,
            color: None,
            wrap: TextWrap::Wrap,
            bold: false,
            dim: false,
            italic: false,
            placeholder: None,
            placeholder_color: Color::BrightBlack,
        }
    }

    /// Get a handle for appending content from another thread.
    pub fn handle(&self) -> StreamingTextHandle {
        StreamingTextHandle {
            buffer: Arc::clone(&self.buffer),
            rendered_len: Arc::clone(&self.rendered_len),
        }
    }

    /// Append text directly (for same-thread use).
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

    /// Enable or disable ANSI parsing (default: enabled).
    pub fn parse_ansi(mut self, enabled: bool) -> Self {
        self.parse_ansi = enabled;
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set text wrapping mode.
    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Enable bold text.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Enable dim text.
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Enable italic text.
    pub fn italic(mut self) -> Self {
        self.italic = true;
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
    ///
    /// This creates a TextNode from the current buffer content.
    /// If ANSI parsing is enabled and the content contains escape sequences,
    /// the node will use styled spans.
    pub fn to_node(&self) -> Node {
        let content = self.content();

        // Handle empty state with placeholder
        if content.is_empty() {
            if let Some(ref placeholder) = self.placeholder {
                return TextNode::new(placeholder.clone())
                    .color(self.placeholder_color)
                    .dim()
                    .wrap(self.wrap)
                    .into();
            }
            return TextNode::new("").into();
        }

        // Mark content as rendered
        self.rendered_len.store(content.len(), Ordering::SeqCst);

        // Parse ANSI and create appropriate node
        if self.parse_ansi && content.contains('\x1b') {
            let spans = parse_ansi(&content);
            // Apply default styling to spans that don't have explicit styling
            let styled_spans: Vec<StyledSpan> = spans
                .into_iter()
                .map(|mut span| {
                    // Apply base style if not explicitly set
                    if span.color.is_none() {
                        span.color = self.color;
                    }
                    if self.bold && !span.bold {
                        span.bold = true;
                    }
                    if self.dim && !span.dim {
                        span.dim = true;
                    }
                    if self.italic && !span.italic {
                        span.italic = true;
                    }
                    span
                })
                .collect();

            TextNode::from_spans(styled_spans).wrap(self.wrap).into()
        } else {
            // Plain text - apply base styling
            let mut node = TextNode::new(content).wrap(self.wrap);
            if let Some(color) = self.color {
                node = node.color(color);
            }
            if self.bold {
                node = node.bold();
            }
            if self.dim {
                node = node.dim();
            }
            if self.italic {
                node = node.italic();
            }
            node.into()
        }
    }

    /// Convert to a Node wrapped in a box with fixed dimensions.
    ///
    /// Useful for embedding in layouts that need explicit sizing.
    pub fn to_node_with_size(self, width: u16, height: u16) -> Node {
        BoxNode::new()
            .width(width)
            .height(height)
            .child(self.to_node())
            .into()
    }
}

/// Convert StreamingText directly to Node.
impl From<StreamingText> for Node {
    fn from(stream: StreamingText) -> Node {
        stream.to_node()
    }
}

/// Convert &StreamingText to Node.
impl From<&StreamingText> for Node {
    fn from(stream: &StreamingText) -> Node {
        stream.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_text_basic() {
        let stream = StreamingText::new();
        assert!(stream.is_empty());

        stream.append("Hello");
        assert_eq!(stream.len(), 5);
        assert_eq!(stream.content(), "Hello");

        stream.append(" World");
        assert_eq!(stream.content(), "Hello World");
    }

    #[test]
    fn test_streaming_text_handle() {
        let stream = StreamingText::new();
        let handle = stream.handle();

        handle.append("Token 1 ");
        handle.append("Token 2");

        assert_eq!(stream.content(), "Token 1 Token 2");
    }

    #[test]
    fn test_streaming_text_clear() {
        let stream = StreamingText::new();
        stream.append("Some content");
        assert!(!stream.is_empty());

        stream.clear();
        assert!(stream.is_empty());
    }

    #[test]
    fn test_streaming_text_with_content() {
        let stream = StreamingText::with_content("Initial");
        assert_eq!(stream.content(), "Initial");
        assert_eq!(stream.len(), 7);
    }

    #[test]
    fn test_streaming_text_handle_batch() {
        let stream = StreamingText::new();
        let handle = stream.handle();

        handle.append_batch(["Hello", " ", "World"]);
        assert_eq!(stream.content(), "Hello World");
    }

    #[test]
    fn test_streaming_text_new_content_tracking() {
        let stream = StreamingText::new();
        let handle = stream.handle();

        // Initially no new content
        assert!(!handle.has_new_content());

        // Append triggers new content
        handle.append("Hello");
        assert!(handle.has_new_content());

        // Take new content
        let new = handle.take_new_content();
        assert_eq!(new, Some("Hello".to_string()));

        // No new content after take
        assert!(!handle.has_new_content());

        // Append more
        handle.append(" World");
        assert!(handle.has_new_content());

        let new = handle.take_new_content();
        assert_eq!(new, Some(" World".to_string()));
    }

    #[test]
    fn test_streaming_text_to_node() {
        let stream = StreamingText::new().color(Color::Blue).bold();
        stream.append("Test content");

        let node = stream.to_node();
        // Node should be a text node (implementation detail, just verify it converts)
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_streaming_text_placeholder() {
        let stream = StreamingText::new()
            .placeholder("Waiting for response...")
            .placeholder_color(Color::Yellow);

        // Empty shows placeholder
        let node = stream.to_node();
        if let Node::Text(text) = node {
            assert_eq!(text.content.as_str(), "Waiting for response...");
        } else {
            panic!("Expected Text node");
        }

        // With content shows actual content
        stream.append("Real content");
        let node = stream.to_node();
        if let Node::Text(text) = node {
            assert!(text.content.as_str().contains("Real content"));
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn test_streaming_text_ansi() {
        let stream = StreamingText::new().parse_ansi(true);
        stream.append("\x1b[31mRed\x1b[0m Normal");

        let node = stream.to_node();
        // Should parse ANSI and create styled spans
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_streaming_text_thread_safety() {
        use std::thread;

        let stream = StreamingText::new();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let handle = stream.handle();
                thread::spawn(move || {
                    for j in 0..10 {
                        handle.append(&format!("{}:{} ", i, j));
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All content should be present
        let content = stream.content();
        // 4 threads * 10 items * at least 3 chars each
        assert!(content.len() >= 120);
    }
}
