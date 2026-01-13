//! Text transformation component.
//!
//! [`Transform`] applies text transformations to its child content, such as
//! case conversion, truncation, and padding.
//!
//! # Example
//!
//! ```rust
//! use inky::components::Transform;
//! use inky::node::TextNode;
//!
//! // Create a transform that uppercases text
//! let transform = Transform::new(TextNode::new("hello"))
//!     .uppercase();
//!
//! // The rendered text will be "HELLO"
//! ```
//!
//! # Chaining Transforms
//!
//! Multiple transformations can be chained:
//!
//! ```rust
//! use inky::components::Transform;
//! use inky::node::TextNode;
//!
//! let transform = Transform::new(TextNode::new("hello world"))
//!     .uppercase()
//!     .truncate(5);
//!
//! // Result: "HELLO"
//! ```

use crate::node::{Node, TextNode};

/// Text alignment for padding operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Align text to the left, pad on right.
    #[default]
    Left,
    /// Center text, pad on both sides.
    Center,
    /// Align text to the right, pad on left.
    Right,
}

/// A text transformation to apply.
#[derive(Debug, Clone)]
pub enum TextTransform {
    /// Convert text to UPPERCASE.
    Uppercase,
    /// Convert text to lowercase.
    Lowercase,
    /// Capitalize first letter of each word.
    Capitalize,
    /// Truncate to max characters, adding ellipsis if truncated.
    Truncate(usize),
    /// Truncate without ellipsis.
    TruncateExact(usize),
    /// Pad to width with alignment.
    Pad {
        /// Target width.
        width: usize,
        /// Alignment within the padded area.
        align: Align,
        /// Character to use for padding.
        fill: char,
    },
    /// Replace occurrences of a string.
    Replace {
        /// String to find.
        from: String,
        /// String to replace with.
        to: String,
    },
    /// Trim whitespace from start and end.
    Trim,
    /// Trim whitespace from start only.
    TrimStart,
    /// Trim whitespace from end only.
    TrimEnd,
    /// Reverse the string.
    Reverse,
}

/// A component that applies text transformations to its child.
///
/// Transform wraps a child node and applies a series of text transformations
/// when rendering. This is useful for case conversion, truncation, and
/// formatting without modifying the source data.
///
/// # Example
///
/// ```rust
/// use inky::components::Transform;
/// use inky::node::TextNode;
///
/// // Create a transform that uppercases and pads
/// let transform = Transform::new(TextNode::new("hi"))
///     .uppercase()
///     .pad(10, inky::components::transform::Align::Center, '-');
///
/// // Renders as "----HI----"
/// ```
#[derive(Debug, Clone)]
pub struct Transform {
    /// The child node to transform.
    child: Box<Node>,
    /// Transformations to apply in order.
    transforms: Vec<TextTransform>,
}

impl Transform {
    /// Create a new transform wrapper around a node.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello"));
    /// ```
    pub fn new(child: impl Into<Node>) -> Self {
        Self {
            child: Box::new(child.into()),
            transforms: Vec::new(),
        }
    }

    /// Convert text to UPPERCASE.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello")).uppercase();
    /// // Renders as "HELLO"
    /// ```
    pub fn uppercase(mut self) -> Self {
        self.transforms.push(TextTransform::Uppercase);
        self
    }

    /// Convert text to lowercase.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("HELLO")).lowercase();
    /// // Renders as "hello"
    /// ```
    pub fn lowercase(mut self) -> Self {
        self.transforms.push(TextTransform::Lowercase);
        self
    }

    /// Capitalize first letter of each word.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello world")).capitalize();
    /// // Renders as "Hello World"
    /// ```
    pub fn capitalize(mut self) -> Self {
        self.transforms.push(TextTransform::Capitalize);
        self
    }

    /// Truncate text to max characters, adding "…" if truncated.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello world")).truncate(5);
    /// // Renders as "hell…"
    /// ```
    pub fn truncate(mut self, max: usize) -> Self {
        self.transforms.push(TextTransform::Truncate(max));
        self
    }

    /// Truncate text to max characters without ellipsis.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello")).truncate_exact(3);
    /// // Renders as "hel"
    /// ```
    pub fn truncate_exact(mut self, max: usize) -> Self {
        self.transforms.push(TextTransform::TruncateExact(max));
        self
    }

    /// Pad text to width with alignment.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::components::transform::Align;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hi"))
    ///     .pad(6, Align::Center, ' ');
    /// // Renders as "  hi  "
    /// ```
    pub fn pad(mut self, width: usize, align: Align, fill: char) -> Self {
        self.transforms
            .push(TextTransform::Pad { width, align, fill });
        self
    }

    /// Pad text to width, left-aligned with spaces.
    pub fn pad_right(self, width: usize) -> Self {
        self.pad(width, Align::Left, ' ')
    }

    /// Pad text to width, right-aligned with spaces.
    pub fn pad_left(self, width: usize) -> Self {
        self.pad(width, Align::Right, ' ')
    }

    /// Center text within width.
    pub fn center(self, width: usize) -> Self {
        self.pad(width, Align::Center, ' ')
    }

    /// Replace occurrences of a string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Transform;
    /// use inky::node::TextNode;
    ///
    /// let transform = Transform::new(TextNode::new("hello world"))
    ///     .replace("world", "rust");
    /// // Renders as "hello rust"
    /// ```
    pub fn replace(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.transforms.push(TextTransform::Replace {
            from: from.into(),
            to: to.into(),
        });
        self
    }

    /// Trim whitespace from start and end.
    pub fn trim(mut self) -> Self {
        self.transforms.push(TextTransform::Trim);
        self
    }

    /// Trim whitespace from start only.
    pub fn trim_start(mut self) -> Self {
        self.transforms.push(TextTransform::TrimStart);
        self
    }

    /// Trim whitespace from end only.
    pub fn trim_end(mut self) -> Self {
        self.transforms.push(TextTransform::TrimEnd);
        self
    }

    /// Reverse the text.
    pub fn reverse(mut self) -> Self {
        self.transforms.push(TextTransform::Reverse);
        self
    }

    /// Apply all transforms to a string.
    fn apply_transforms(&self, mut text: String) -> String {
        for transform in &self.transforms {
            text = match transform {
                TextTransform::Uppercase => text.to_uppercase(),
                TextTransform::Lowercase => text.to_lowercase(),
                TextTransform::Capitalize => capitalize_words(&text),
                TextTransform::Truncate(max) => truncate_with_ellipsis(&text, *max),
                TextTransform::TruncateExact(max) => text.chars().take(*max).collect(),
                TextTransform::Pad { width, align, fill } => {
                    pad_string(&text, *width, *align, *fill)
                }
                TextTransform::Replace { from, to } => text.replace(from, to),
                TextTransform::Trim => text.trim().to_string(),
                TextTransform::TrimStart => text.trim_start().to_string(),
                TextTransform::TrimEnd => text.trim_end().to_string(),
                TextTransform::Reverse => text.chars().rev().collect(),
            };
        }
        text
    }

    /// Convert to a Node for rendering.
    ///
    /// This extracts text from the child node, applies transforms, and
    /// creates a new TextNode with the result.
    pub fn to_node(&self) -> Node {
        let text = extract_text(&self.child);
        let transformed = self.apply_transforms(text);

        // Create a TextNode with the transformed text, preserving child's style
        let style = self.child.style().clone();
        let (text_style, line_style) = match self.child.as_ref() {
            Node::Text(t) => (t.text_style.clone(), t.line_style.clone()),
            _ => (Default::default(), None),
        };

        let mut text_node = TextNode::new(transformed);
        text_node.text_style = text_style;
        text_node.line_style = line_style;
        text_node.style = style;
        text_node.into()
    }
}

impl From<Transform> for Node {
    fn from(transform: Transform) -> Self {
        transform.to_node()
    }
}

/// Extract text content from a node recursively.
fn extract_text(node: &Node) -> String {
    match node {
        Node::Text(t) => t.content.as_str().into_owned(),
        Node::Box(b) => {
            let mut result = String::new();
            for child in &b.children {
                result.push_str(&extract_text(child.as_ref()));
            }
            result
        }
        Node::Root(r) => {
            let mut result = String::new();
            for child in &r.children {
                result.push_str(&extract_text(child.as_ref()));
            }
            result
        }
        Node::Static(s) => {
            let mut result = String::new();
            for child in &s.children {
                result.push_str(&extract_text(child.as_ref()));
            }
            result
        }
        Node::Custom(c) => {
            // Custom widgets may have children - extract from them
            let mut result = String::new();
            for child in c.widget().children() {
                result.push_str(&extract_text(child.as_ref()));
            }
            result
        }
    }
}

/// Capitalize the first letter of each word.
fn capitalize_words(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;

    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

/// Truncate string to max length, adding ellipsis if needed.
fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    let char_count: usize = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else if max <= 1 {
        "…".to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    }
}

/// Pad a string to width with specified alignment.
fn pad_string(s: &str, width: usize, align: Align, fill: char) -> String {
    let char_count = s.chars().count();
    if char_count >= width {
        return s.to_string();
    }

    let padding = width - char_count;

    match align {
        Align::Left => {
            let mut result = s.to_string();
            result.extend(std::iter::repeat(fill).take(padding));
            result
        }
        Align::Right => {
            let mut result: String = std::iter::repeat(fill).take(padding).collect();
            result.push_str(s);
            result
        }
        Align::Center => {
            let left = padding / 2;
            let right = padding - left;
            let mut result: String = std::iter::repeat(fill).take(left).collect();
            result.push_str(s);
            result.extend(std::iter::repeat(fill).take(right));
            result
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_uppercase() {
        let t = Transform::new(TextNode::new("hello")).uppercase();
        let text = t.apply_transforms("hello".to_string());
        assert_eq!(text, "HELLO");
    }

    #[test]
    fn test_lowercase() {
        let t = Transform::new(TextNode::new("HELLO")).lowercase();
        let text = t.apply_transforms("HELLO".to_string());
        assert_eq!(text, "hello");
    }

    #[test]
    fn test_capitalize() {
        let t = Transform::new(TextNode::new("hello world")).capitalize();
        let text = t.apply_transforms("hello world".to_string());
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_truncate() {
        let t = Transform::new(TextNode::new("hello world")).truncate(5);
        let text = t.apply_transforms("hello world".to_string());
        assert_eq!(text, "hell…");
    }

    #[test]
    fn test_truncate_exact() {
        let t = Transform::new(TextNode::new("hello")).truncate_exact(3);
        let text = t.apply_transforms("hello".to_string());
        assert_eq!(text, "hel");
    }

    #[test]
    fn test_truncate_no_change() {
        let t = Transform::new(TextNode::new("hi")).truncate(10);
        let text = t.apply_transforms("hi".to_string());
        assert_eq!(text, "hi");
    }

    #[test]
    fn test_pad_left() {
        let t = Transform::new(TextNode::new("hi")).pad(5, Align::Left, ' ');
        let text = t.apply_transforms("hi".to_string());
        assert_eq!(text, "hi   ");
    }

    #[test]
    fn test_pad_right() {
        let t = Transform::new(TextNode::new("hi")).pad(5, Align::Right, ' ');
        let text = t.apply_transforms("hi".to_string());
        assert_eq!(text, "   hi");
    }

    #[test]
    fn test_pad_center() {
        let t = Transform::new(TextNode::new("hi")).pad(6, Align::Center, '-');
        let text = t.apply_transforms("hi".to_string());
        assert_eq!(text, "--hi--");
    }

    #[test]
    fn test_replace() {
        let t = Transform::new(TextNode::new("hello world")).replace("world", "rust");
        let text = t.apply_transforms("hello world".to_string());
        assert_eq!(text, "hello rust");
    }

    #[test]
    fn test_trim() {
        let t = Transform::new(TextNode::new("  hello  ")).trim();
        let text = t.apply_transforms("  hello  ".to_string());
        assert_eq!(text, "hello");
    }

    #[test]
    fn test_reverse() {
        let t = Transform::new(TextNode::new("hello")).reverse();
        let text = t.apply_transforms("hello".to_string());
        assert_eq!(text, "olleh");
    }

    #[test]
    fn test_chained_transforms() {
        let t = Transform::new(TextNode::new("hello world"))
            .uppercase()
            .truncate(8)
            .pad(10, Align::Center, '*');
        let text = t.apply_transforms("hello world".to_string());
        // HELLO WO -> HELLO W… (8 chars with ellipsis) -> *HELLO W…*
        assert_eq!(text, "*HELLO W…*");
    }

    #[test]
    fn test_extract_text_from_text_node() {
        let node: Node = TextNode::new("hello").into();
        assert_eq!(extract_text(&node), "hello");
    }

    #[test]
    fn test_capitalize_words_helper() {
        assert_eq!(capitalize_words("hello world"), "Hello World");
        assert_eq!(capitalize_words("HELLO"), "HELLO"); // Doesn't lowercase first
        assert_eq!(capitalize_words(""), "");
        assert_eq!(capitalize_words("a b c"), "A B C");
    }

    #[test]
    fn test_truncate_edge_cases() {
        assert_eq!(truncate_with_ellipsis("hi", 1), "…");
        assert_eq!(truncate_with_ellipsis("", 5), "");
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
        assert_eq!(truncate_with_ellipsis("hello", 4), "hel…");
    }
}
