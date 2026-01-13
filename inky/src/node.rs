//! Node types for the virtual DOM tree.
//!
//! The node tree represents the UI structure before layout and rendering.
//! Each node corresponds to a terminal UI element (box, text, etc.).
//!
//! # Custom Widgets
//!
//! For applications that need custom rendering beyond the built-in node types,
//! implement the [`Widget`](crate::node::Widget) trait and use [`CustomNode`](crate::node::CustomNode) to wrap your widget:
//!
//! ```
//! use inky::prelude::*;
//! use inky::node::{Widget, WidgetContext, CustomNode};
//! use inky::render::Painter;
//!
//! struct MyWidget {
//!     value: i32,
//! }
//!
//! impl Widget for MyWidget {
//!     fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
//!         // Draw directly to the buffer at the computed layout position
//!         let cell = inky::render::Cell::new('*').with_fg(Color::Green);
//!         painter.buffer_mut().set(ctx.x, ctx.y, cell);
//!     }
//!
//!     fn measure(&self, _available_width: u16, _available_height: u16) -> (u16, u16) {
//!         (1, 1) // Our widget is 1x1
//!     }
//! }
//!
//! // Use in the node tree:
//! let node = CustomNode::new(MyWidget { value: 42 });
//! ```

use crate::render::Painter;
use crate::style::{Color, Style, TextStyle};
use smallvec::SmallVec;
use smartstring::alias::String as SmartString;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Type alias for node children collections.
/// Uses SmallVec with boxed nodes - the Box provides necessary indirection for
/// the recursive Node type while SmallVec avoids Vec overhead for small child counts.
/// The first 8 Box pointers are stored inline (64 bytes on 64-bit), spilling to heap only for larger trees.
pub type NodeChildren = SmallVec<[Box<Node>; 8]>;

/// Unique identifier for nodes in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Generate a new unique node ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        NodeId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

// === Widget Trait for Custom Renderables ===

/// Context passed to custom widgets during rendering.
///
/// Provides information about the computed layout and position.
#[derive(Debug, Clone, Copy)]
pub struct WidgetContext {
    /// Absolute X position in terminal columns.
    pub x: u16,
    /// Absolute Y position in terminal rows.
    pub y: u16,
    /// Computed width in terminal columns.
    pub width: u16,
    /// Computed height in terminal rows.
    pub height: u16,
}

/// Trait for custom renderable widgets.
///
/// Implement this trait to create custom node types that can be embedded
/// in the inky node tree. This enables porting complex existing UIs or
/// creating specialized rendering that doesn't fit the standard node types.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
/// use inky::node::{Widget, WidgetContext};
/// use inky::render::Painter;
///
/// struct ProgressBar {
///     progress: f32,
///     fill_char: char,
/// }
///
/// impl Widget for ProgressBar {
///     fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
///         let filled = (ctx.width as f32 * self.progress) as u16;
///         for x in 0..filled.min(ctx.width) {
///             let cell = inky::render::Cell::new(self.fill_char).with_fg(Color::Green);
///             painter.buffer_mut().set(ctx.x + x, ctx.y, cell);
///         }
///     }
///
///     fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
///         (available_width, 1)
///     }
/// }
/// ```
pub trait Widget: Send + Sync {
    /// Render the widget to the buffer.
    ///
    /// The `ctx` contains the computed layout position and dimensions.
    /// Use the `painter` to draw to the buffer.
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter);

    /// Measure the widget's preferred size.
    ///
    /// Returns (width, height) in terminal cells.
    /// The layout engine calls this to determine the widget's intrinsic size.
    fn measure(&self, available_width: u16, available_height: u16) -> (u16, u16);

    /// Optional: Get children of this widget (for container widgets).
    ///
    /// Default implementation returns an empty slice.
    fn children(&self) -> &[Box<Node>] {
        &[]
    }
}

/// A custom node wrapping a user-defined widget.
///
/// Use this to embed custom [`Widget`] implementations in the node tree.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
/// use inky::node::{Widget, WidgetContext, CustomNode};
/// use inky::render::Painter;
///
/// struct MyWidget;
///
/// impl Widget for MyWidget {
///     fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
///         // Custom rendering
///     }
///     fn measure(&self, _w: u16, _h: u16) -> (u16, u16) { (10, 1) }
/// }
///
/// let node: Node = CustomNode::new(MyWidget).into();
/// ```
pub struct CustomNode {
    /// Unique identifier.
    pub id: NodeId,
    /// The widget implementation.
    widget: Arc<dyn Widget>,
    /// Layout style.
    pub style: Style,
}

impl CustomNode {
    /// Create a new custom node wrapping a widget.
    pub fn new<W: Widget + 'static>(widget: W) -> Self {
        Self {
            id: NodeId::new(),
            widget: Arc::new(widget),
            style: Style::default(),
        }
    }

    /// Get a reference to the widget.
    pub fn widget(&self) -> &dyn Widget {
        &*self.widget
    }

    /// Clone the widget Arc for layout measurement or sharing.
    pub fn widget_arc(&self) -> Arc<dyn Widget> {
        Arc::clone(&self.widget)
    }

    /// Set the style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set width.
    pub fn width(mut self, w: impl Into<crate::style::Dimension>) -> Self {
        self.style.width = w.into();
        self
    }

    /// Set height.
    pub fn height(mut self, h: impl Into<crate::style::Dimension>) -> Self {
        self.style.height = h.into();
        self
    }

    /// Set flex-grow factor.
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    /// Set flex-shrink factor.
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.style.flex_shrink = shrink;
        self
    }
}

impl Clone for CustomNode {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            widget: Arc::clone(&self.widget),
            style: self.style.clone(),
        }
    }
}

impl fmt::Debug for CustomNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomNode")
            .field("id", &self.id)
            .field("style", &self.style)
            .finish_non_exhaustive()
    }
}

impl From<CustomNode> for Node {
    fn from(node: CustomNode) -> Self {
        Node::Custom(node)
    }
}

/// A node in the virtual DOM tree.
#[derive(Debug, Clone)]
pub enum Node {
    /// Root container node.
    Root(RootNode),
    /// Flexbox container node.
    Box(BoxNode),
    /// Text content node.
    Text(TextNode),
    /// Static (non-updating) content node.
    Static(StaticNode),
    /// Custom widget node.
    Custom(CustomNode),
}

impl Node {
    /// Get the node's unique ID.
    #[inline]
    pub fn id(&self) -> NodeId {
        match self {
            Node::Root(n) => n.id,
            Node::Box(n) => n.id,
            Node::Text(n) => n.id,
            Node::Static(n) => n.id,
            Node::Custom(n) => n.id,
        }
    }

    /// Get the node's children (if any).
    pub fn children(&self) -> &[Box<Node>] {
        match self {
            Node::Root(n) => &n.children,
            Node::Box(n) => &n.children,
            Node::Text(_) => &[],
            Node::Static(n) => &n.children,
            Node::Custom(n) => n.widget().children(),
        }
    }

    /// Get mutable reference to children (if any).
    pub fn children_mut(&mut self) -> Option<&mut NodeChildren> {
        match self {
            Node::Root(n) => Some(&mut n.children),
            Node::Box(n) => Some(&mut n.children),
            Node::Text(_) => None,
            Node::Static(n) => Some(&mut n.children),
            Node::Custom(_) => None, // Custom widgets manage their own children
        }
    }

    /// Get the node's style.
    pub fn style(&self) -> &Style {
        match self {
            Node::Root(n) => &n.style,
            Node::Box(n) => &n.style,
            Node::Text(n) => &n.style,
            Node::Static(n) => &n.style,
            Node::Custom(n) => &n.style,
        }
    }
}

// === Root Node ===

/// Root container that holds the entire UI tree.
///
/// There is exactly one root node per application.
#[derive(Debug, Clone)]
pub struct RootNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Child nodes (SmallVec: stack-allocated for ≤8 children).
    pub children: NodeChildren,
    /// Layout style.
    pub style: Style,
}

impl RootNode {
    /// Create a new root node.
    pub fn new() -> Self {
        Self {
            id: NodeId::new(),
            children: SmallVec::new(),
            style: Style::default(),
        }
    }

    /// Add a child node.
    pub fn child(mut self, node: impl Into<Node>) -> Self {
        self.children.push(Box::new(node.into()));
        self
    }

    /// Add multiple children.
    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<Node>>) -> Self {
        self.children
            .extend(nodes.into_iter().map(|n| Box::new(n.into())));
        self
    }

    /// Set the style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Default for RootNode {
    fn default() -> Self {
        Self::new()
    }
}

impl From<RootNode> for Node {
    fn from(node: RootNode) -> Self {
        Node::Root(node)
    }
}

// === Box Node ===

/// Flexbox container node (equivalent to Ink's `<Box>`).
///
/// Provides CSS Flexbox layout capabilities.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// let ui = BoxNode::new()
///     .flex_direction(FlexDirection::Column)
///     .padding(1)
///     .child(TextNode::new("Hello"))
///     .child(TextNode::new("World"));
/// ```
#[derive(Debug, Clone)]
pub struct BoxNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Child nodes (SmallVec: stack-allocated for ≤8 children).
    pub children: NodeChildren,
    /// Layout and appearance style.
    pub style: Style,
}

impl BoxNode {
    /// Create a new box node.
    pub fn new() -> Self {
        Self {
            id: NodeId::new(),
            children: SmallVec::new(),
            style: Style::default(),
        }
    }

    /// Set a custom node ID.
    ///
    /// Useful for tracking specific nodes in hit testing or focus management.
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = id;
        self
    }

    // === Child management ===

    /// Add a child node.
    pub fn child(mut self, node: impl Into<Node>) -> Self {
        self.children.push(Box::new(node.into()));
        self
    }

    /// Add multiple children.
    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<Node>>) -> Self {
        self.children
            .extend(nodes.into_iter().map(|n| Box::new(n.into())));
        self
    }

    // === Flex container properties ===

    /// Set flex direction (row, column, row-reverse, column-reverse).
    pub fn flex_direction(mut self, dir: crate::style::FlexDirection) -> Self {
        self.style.flex_direction = dir;
        self
    }

    /// Set flex wrap behavior.
    pub fn flex_wrap(mut self, wrap: crate::style::FlexWrap) -> Self {
        self.style.flex_wrap = wrap;
        self
    }

    /// Set justify-content (main axis alignment).
    pub fn justify_content(mut self, justify: crate::style::JustifyContent) -> Self {
        self.style.justify_content = justify;
        self
    }

    /// Set align-items (cross axis alignment).
    pub fn align_items(mut self, align: crate::style::AlignItems) -> Self {
        self.style.align_items = align;
        self
    }

    /// Set align-content (multi-line cross axis alignment).
    pub fn align_content(mut self, align: crate::style::AlignContent) -> Self {
        self.style.align_content = align;
        self
    }

    /// Set gap between children.
    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self
    }

    // === Flex item properties ===

    /// Set flex-grow factor.
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    /// Set flex-shrink factor.
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.style.flex_shrink = shrink;
        self
    }

    /// Set flex-basis (initial size).
    pub fn flex_basis(mut self, basis: impl Into<crate::style::Dimension>) -> Self {
        self.style.flex_basis = basis.into();
        self
    }

    /// Set align-self (override align-items for this item).
    pub fn align_self(mut self, align: crate::style::AlignSelf) -> Self {
        self.style.align_self = align;
        self
    }

    // === Size properties ===

    /// Set width.
    pub fn width(mut self, w: impl Into<crate::style::Dimension>) -> Self {
        self.style.width = w.into();
        self
    }

    /// Set height.
    pub fn height(mut self, h: impl Into<crate::style::Dimension>) -> Self {
        self.style.height = h.into();
        self
    }

    /// Set minimum width.
    pub fn min_width(mut self, w: impl Into<crate::style::Dimension>) -> Self {
        self.style.min_width = w.into();
        self
    }

    /// Set minimum height.
    pub fn min_height(mut self, h: impl Into<crate::style::Dimension>) -> Self {
        self.style.min_height = h.into();
        self
    }

    /// Set maximum width.
    pub fn max_width(mut self, w: impl Into<crate::style::Dimension>) -> Self {
        self.style.max_width = w.into();
        self
    }

    /// Set maximum height.
    pub fn max_height(mut self, h: impl Into<crate::style::Dimension>) -> Self {
        self.style.max_height = h.into();
        self
    }

    // === Spacing properties ===

    /// Set padding on all sides.
    pub fn padding(mut self, p: impl Into<crate::style::Edges>) -> Self {
        self.style.padding = p.into();
        self
    }

    /// Set padding on specific sides.
    pub fn padding_xy(mut self, x: f32, y: f32) -> Self {
        self.style.padding = crate::style::Edges::xy(x, y);
        self
    }

    /// Set margin on all sides.
    pub fn margin(mut self, m: impl Into<crate::style::Edges>) -> Self {
        self.style.margin = m.into();
        self
    }

    /// Set margin on specific sides.
    pub fn margin_xy(mut self, x: f32, y: f32) -> Self {
        self.style.margin = crate::style::Edges::xy(x, y);
        self
    }

    // === Border ===

    /// Set border style.
    pub fn border(mut self, border: crate::style::BorderStyle) -> Self {
        self.style.border = border;
        self
    }

    // === Overflow ===

    /// Set overflow behavior.
    pub fn overflow(mut self, overflow: crate::style::Overflow) -> Self {
        self.style.overflow = overflow;
        self
    }

    // === Display ===

    /// Set display mode.
    pub fn display(mut self, display: crate::style::Display) -> Self {
        self.style.display = display;
        self
    }

    /// Hide this node.
    pub fn hidden(mut self) -> Self {
        self.style.display = crate::style::Display::None;
        self
    }

    // === Background ===

    /// Set background color.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.style.background_color = Some(color.into());
        self
    }

    // === Style ===

    /// Replace the entire style.
    ///
    /// Useful when you have a pre-built Style object to apply.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Default for BoxNode {
    fn default() -> Self {
        Self::new()
    }
}

impl From<BoxNode> for Node {
    fn from(node: BoxNode) -> Self {
        Node::Box(node)
    }
}

// === Text Node ===

/// Text content - either plain text or styled spans.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// // Plain text
/// let plain = TextContent::Plain("Hello".into());
///
/// // Styled spans (e.g., from ANSI parsing)
/// let styled = TextContent::Spans(vec![
///     StyledSpan::new("Hello, ").color(Color::Blue),
///     StyledSpan::new("World!").color(Color::Red).bold(),
/// ]);
/// ```
#[derive(Debug, Clone)]
pub enum TextContent {
    /// Plain text with uniform styling (most common case).
    Plain(SmartString),
    /// Multiple spans with individual styling (for ANSI passthrough, syntax highlighting).
    Spans(Vec<crate::style::StyledSpanOwned>),
}

impl Default for TextContent {
    fn default() -> Self {
        Self::Plain(SmartString::new())
    }
}

impl TextContent {
    /// Get the plain text content, concatenating spans if needed.
    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        match self {
            TextContent::Plain(s) => std::borrow::Cow::Borrowed(s.as_str()),
            TextContent::Spans(spans) => {
                let mut result = String::new();
                for span in spans {
                    result.push_str(&span.text);
                }
                std::borrow::Cow::Owned(result)
            }
        }
    }

    /// Check if the content is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            TextContent::Plain(s) => s.is_empty(),
            TextContent::Spans(spans) => spans.iter().all(|s| s.text.is_empty()),
        }
    }

    /// Get the total character length.
    pub fn len(&self) -> usize {
        match self {
            TextContent::Plain(s) => s.len(),
            TextContent::Spans(spans) => spans.iter().map(|s| s.text.len()).sum(),
        }
    }

    /// Check if the content contains a substring.
    pub fn contains(&self, pattern: &str) -> bool {
        match self {
            TextContent::Plain(s) => s.contains(pattern),
            TextContent::Spans(_) => {
                // For spans, we need to check the concatenated text
                // This is less efficient but handles patterns that span multiple spans
                self.as_str().contains(pattern)
            }
        }
    }

    /// Check if plain text is stored inline (SmartString optimization).
    /// Returns false for spans (they use heap allocation).
    pub fn is_inline(&self) -> bool {
        match self {
            TextContent::Plain(s) => s.is_inline(),
            TextContent::Spans(_) => false,
        }
    }
}

impl PartialEq<&str> for TextContent {
    fn eq(&self, other: &&str) -> bool {
        match self {
            TextContent::Plain(s) => s.as_str() == *other,
            TextContent::Spans(_) => self.as_str() == *other,
        }
    }
}

impl PartialEq<str> for TextContent {
    fn eq(&self, other: &str) -> bool {
        match self {
            TextContent::Plain(s) => s.as_str() == other,
            TextContent::Spans(_) => self.as_str() == other,
        }
    }
}

impl std::fmt::Display for TextContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextContent::Plain(s) => write!(f, "{}", s),
            TextContent::Spans(spans) => {
                for span in spans {
                    write!(f, "{}", span.text)?;
                }
                Ok(())
            }
        }
    }
}

impl From<&str> for TextContent {
    fn from(s: &str) -> Self {
        TextContent::Plain(SmartString::from(s))
    }
}

impl From<String> for TextContent {
    fn from(s: String) -> Self {
        TextContent::Plain(SmartString::from(s))
    }
}

impl From<SmartString> for TextContent {
    fn from(s: SmartString) -> Self {
        TextContent::Plain(s)
    }
}

impl From<Vec<crate::style::StyledSpanOwned>> for TextContent {
    fn from(spans: Vec<crate::style::StyledSpanOwned>) -> Self {
        TextContent::Spans(spans)
    }
}

/// Text content node (equivalent to Ink's `<Text>`).
///
/// Renders styled text content. Supports both plain text with uniform styling
/// and styled spans for ANSI passthrough or syntax highlighting.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// // Simple plain text
/// let text = TextNode::new("Hello, World!")
///     .color(Color::Blue)
///     .bold();
///
/// // Styled spans (e.g., from ANSI parsing)
/// let styled = TextNode::from_spans(vec![
///     StyledSpan::new("Error: ").color(Color::Red).bold(),
///     StyledSpan::new("file not found"),
/// ]);
/// ```
#[derive(Debug, Clone)]
pub struct TextNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Text content - either plain or styled spans.
    pub content: TextContent,
    /// Layout style.
    pub style: Style,
    /// Text-specific styling (applies to plain text or as default for spans).
    pub text_style: TextStyle,
    /// Optional line-level style (applied to the full line background).
    pub line_style: Option<TextStyle>,
    /// Optional cursor position (character index in text).
    /// When set, the render pipeline will track and report the screen coordinates
    /// of this position, enabling proper terminal cursor placement.
    pub cursor_position: Option<usize>,
}

impl TextNode {
    /// Create a new text node with plain text.
    ///
    /// The content is stored in a SmartString, which uses inline storage for
    /// strings up to 23 bytes, avoiding heap allocation for short text like
    /// labels, buttons, and status messages.
    pub fn new(content: impl AsRef<str>) -> Self {
        Self {
            id: NodeId::new(),
            content: TextContent::Plain(SmartString::from(content.as_ref())),
            style: Style::default(),
            text_style: TextStyle::default(),
            line_style: None,
            cursor_position: None,
        }
    }

    /// Create a text node from styled spans.
    ///
    /// Use this when rendering ANSI-styled output or syntax-highlighted code
    /// where different parts of the text have different colors/attributes.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let text = TextNode::from_spans(vec![
    ///     StyledSpan::new("Error: ").color(Color::Red).bold(),
    ///     StyledSpan::new("file not found"),
    /// ]);
    /// ```
    pub fn from_spans(spans: Vec<crate::style::StyledSpanOwned>) -> Self {
        Self {
            id: NodeId::new(),
            content: TextContent::Spans(spans),
            style: Style::default(),
            text_style: TextStyle::default(),
            line_style: None,
            cursor_position: None,
        }
    }

    /// Create a text node from ANSI-escaped text.
    ///
    /// Parses ANSI escape sequences and converts them to styled spans.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let text = TextNode::from_ansi("\x1b[31mError:\x1b[0m file not found");
    /// ```
    pub fn from_ansi(ansi_text: &str) -> Self {
        Self {
            id: NodeId::new(),
            content: TextContent::Spans(crate::ansi::parse_ansi(ansi_text)),
            style: Style::default(),
            text_style: TextStyle::default(),
            line_style: None,
            cursor_position: None,
        }
    }

    /// Set text content (plain text).
    pub fn content(mut self, content: impl AsRef<str>) -> Self {
        self.content = TextContent::Plain(SmartString::from(content.as_ref()));
        self
    }

    /// Set text content from styled spans.
    pub fn spans(mut self, spans: Vec<crate::style::StyledSpanOwned>) -> Self {
        self.content = TextContent::Spans(spans);
        self
    }

    /// Get the plain text content (concatenates spans if needed).
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        self.content.as_str()
    }

    /// Apply style to the entire line (background, etc.).
    pub fn line_style(mut self, style: TextStyle) -> Self {
        self.line_style = Some(style);
        self
    }

    /// Set cursor position (character index in text).
    ///
    /// When set, the render pipeline will track the screen coordinates of this
    /// position, enabling proper terminal cursor placement for text input widgets.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let text = TextNode::new("Hello, World!")
    ///     .cursor_at(7); // Cursor after "Hello, "
    /// ```
    pub fn cursor_at(mut self, position: usize) -> Self {
        self.cursor_position = Some(position);
        self
    }

    /// Clear cursor position.
    pub fn no_cursor(mut self) -> Self {
        self.cursor_position = None;
        self
    }

    // === Text colors ===

    /// Set text color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.text_style.color = Some(color.into());
        self
    }

    /// Set background color.
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.text_style.background_color = Some(color.into());
        self
    }

    // === Text formatting ===

    /// Make text bold.
    pub fn bold(mut self) -> Self {
        self.text_style.bold = true;
        self
    }

    /// Make text italic.
    pub fn italic(mut self) -> Self {
        self.text_style.italic = true;
        self
    }

    /// Underline text.
    pub fn underline(mut self) -> Self {
        self.text_style.underline = true;
        self
    }

    /// Strikethrough text.
    pub fn strikethrough(mut self) -> Self {
        self.text_style.strikethrough = true;
        self
    }

    /// Dim text.
    pub fn dim(mut self) -> Self {
        self.text_style.dim = true;
        self
    }

    /// Inverse colors.
    pub fn inverse(mut self) -> Self {
        self.text_style.inverse = true;
        self
    }

    // === Text wrapping ===

    /// Set text wrap mode.
    pub fn wrap(mut self, wrap: crate::style::TextWrap) -> Self {
        self.text_style.wrap = wrap;
        self
    }

    /// Truncate text with ellipsis at end.
    pub fn truncate(mut self) -> Self {
        self.text_style.wrap = crate::style::TextWrap::Truncate;
        self
    }

    // === Size properties ===

    /// Set width.
    pub fn width(mut self, w: impl Into<crate::style::Dimension>) -> Self {
        self.style.width = w.into();
        self
    }

    /// Set flex-grow factor.
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    /// Set flex-shrink factor.
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.style.flex_shrink = shrink;
        self
    }
}

impl From<TextNode> for Node {
    fn from(node: TextNode) -> Self {
        Node::Text(node)
    }
}

impl From<&str> for TextNode {
    fn from(s: &str) -> Self {
        TextNode::new(s)
    }
}

impl From<String> for TextNode {
    fn from(s: String) -> Self {
        TextNode::new(s)
    }
}

impl From<crate::style::Line> for TextNode {
    fn from(line: crate::style::Line) -> Self {
        let line_style = line.get_style().cloned();
        let mut node = TextNode::from_spans(line.into_spans());
        node.line_style = line_style;
        node
    }
}

impl From<crate::style::Line> for Node {
    fn from(line: crate::style::Line) -> Self {
        Node::Text(TextNode::from(line))
    }
}

// === Static Node ===

/// Static content node that doesn't re-render.
///
/// Used for log output that scrolls up and shouldn't change.
/// Similar to Ink's `<Static>` component.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// let logs = StaticNode::new()
///     .child(TextNode::new("[INFO] Server started"))
///     .child(TextNode::new("[INFO] Listening on port 8080"));
/// ```
#[derive(Debug, Clone)]
pub struct StaticNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Child nodes (SmallVec: stack-allocated for ≤8 children).
    pub children: NodeChildren,
    /// Layout style.
    pub style: Style,
}

impl StaticNode {
    /// Create a new static node.
    pub fn new() -> Self {
        Self {
            id: NodeId::new(),
            children: SmallVec::new(),
            style: Style::default(),
        }
    }

    /// Add a child node.
    pub fn child(mut self, node: impl Into<Node>) -> Self {
        self.children.push(Box::new(node.into()));
        self
    }

    /// Add multiple children.
    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<Node>>) -> Self {
        self.children
            .extend(nodes.into_iter().map(|n| Box::new(n.into())));
        self
    }
}

impl Default for StaticNode {
    fn default() -> Self {
        Self::new()
    }
}

impl From<StaticNode> for Node {
    fn from(node: StaticNode) -> Self {
        Node::Static(node)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_unique() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_box_builder() {
        let node = BoxNode::new()
            .flex_direction(crate::style::FlexDirection::Column)
            .padding(1.0)
            .child(TextNode::new("Hello"));

        assert_eq!(node.children.len(), 1);
        assert_eq!(
            node.style.flex_direction,
            crate::style::FlexDirection::Column
        );
    }

    #[test]
    fn test_text_builder() {
        let node = TextNode::new("Hello").bold().color(Color::Blue);

        assert_eq!(node.content, "Hello");
        assert!(node.text_style.bold);
        assert_eq!(node.text_style.color, Some(Color::Blue));
    }

    #[test]
    fn test_node_conversion() {
        let text: Node = TextNode::new("test").into();
        assert!(matches!(text, Node::Text(_)));

        let boxn: Node = BoxNode::new().into();
        assert!(matches!(boxn, Node::Box(_)));
    }

    #[test]
    fn test_smartstring_inline_storage() {
        // SmartString stores strings ≤23 bytes inline (no heap allocation)
        let short = TextNode::new("OK");
        assert_eq!(short.content.len(), 2);
        assert!(short.content.is_inline()); // Should be inline

        let medium = TextNode::new("Hello, World!"); // 13 bytes
        assert!(medium.content.is_inline());

        let exactly_23 = TextNode::new("12345678901234567890123"); // 23 bytes
        assert!(exactly_23.content.is_inline());

        // Long strings spill to heap
        let long = TextNode::new("This is a longer string that exceeds the inline threshold");
        assert!(!long.content.is_inline());
    }

    #[test]
    fn test_text_cursor_at() {
        let node = TextNode::new("Hello").cursor_at(3);
        assert_eq!(node.cursor_position, Some(3));
    }

    #[test]
    fn test_text_cursor_at_default() {
        let node = TextNode::new("Hello");
        assert_eq!(node.cursor_position, None);
    }

    #[test]
    fn test_text_no_cursor() {
        let node = TextNode::new("Hello").cursor_at(3).no_cursor();
        assert_eq!(node.cursor_position, None);
    }

    #[test]
    fn test_line_to_text_node() {
        use crate::style::{Line, StyledSpan};

        let line = Line::new()
            .span(StyledSpan::new("Hello ").bold())
            .span(StyledSpan::new("World").color(Color::Blue));

        let node: TextNode = line.into();

        // Should have spans
        assert!(matches!(node.content, TextContent::Spans(_)));
        if let TextContent::Spans(spans) = &node.content {
            assert_eq!(spans.len(), 2);
            assert!(spans[0].bold);
            assert_eq!(spans[1].color, Some(Color::Blue));
        }
    }

    #[test]
    fn test_line_to_node() {
        use crate::style::Line;

        let line = Line::text("Test");
        let node: Node = line.into();

        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_line_with_style_to_text_node() {
        use crate::style::Line;

        let line_style = TextStyle::new().bg(Color::Yellow);
        let line = Line::text("Highlighted").style(line_style.clone());

        let node: TextNode = line.into();

        assert_eq!(node.line_style, Some(line_style));
    }
}
