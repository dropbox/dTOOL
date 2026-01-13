//! Declarative macros for building UI trees.
//!
//! These macros provide convenient shorthand for common UI patterns,
//! reducing boilerplate while maintaining the full power of the builder API.
//!
//! # Layout Macros
//!
//! ```
//! use inky::prelude::*;
//!
//! // Vertical layout (flex-direction: column)
//! let ui = vbox![
//!     TextNode::new("Header"),
//!     TextNode::new("Content"),
//!     TextNode::new("Footer"),
//! ];
//!
//! // Horizontal layout (flex-direction: row)
//! let row = hbox![
//!     TextNode::new("Left"),
//!     Spacer::new(),
//!     TextNode::new("Right"),
//! ];
//! ```
//!
//! # Style Macro
//!
//! ```
//! use inky::prelude::*;
//!
//! let my_style = style! {
//!     padding: 2,
//!     border: BorderStyle::Rounded,
//!     flex_direction: FlexDirection::Column,
//! };
//! ```
//!
//! # Text Macro
//!
//! ```
//! use inky::prelude::*;
//!
//! let styled = text!("Hello", color = Color::Blue, bold);
//! ```

/// Creates a vertical box layout (flex-direction: column).
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// let ui = vbox![
///     TextNode::new("Header"),
///     TextNode::new("Content"),
///     TextNode::new("Footer"),
/// ];
/// ```
#[macro_export]
macro_rules! vbox {
    // Empty vbox
    [] => {
        $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Column)
    };
    // vbox with children
    [$($child:expr),+ $(,)?] => {
        $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Column)
            $(.child($child))+
    };
}

/// Creates a horizontal box layout (flex-direction: row).
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// let row = hbox![
///     TextNode::new("Left"),
///     Spacer::new(),
///     TextNode::new("Right"),
/// ];
/// ```
#[macro_export]
macro_rules! hbox {
    // Empty hbox
    [] => {
        $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Row)
    };
    // hbox with children
    [$($child:expr),+ $(,)?] => {
        $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Row)
            $(.child($child))+
    };
}

/// Creates a Style struct with the specified properties.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// let my_style = style! {
///     padding: 2,
///     border: BorderStyle::Rounded,
///     flex_direction: FlexDirection::Column,
/// };
/// ```
#[macro_export]
macro_rules! style {
    // Empty style
    {} => {
        $crate::style::Style::default()
    };
    // Style with properties
    { $($field:ident : $value:expr),+ $(,)? } => {{
        #[allow(unused_mut)]
        let mut style = $crate::style::Style::default();
        $(style.$field = $value.into();)+
        style
    }};
}

/// Creates a styled TextNode.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// // Basic text
/// let simple = text!("Hello");
///
/// // Text with color
/// let colored = text!("Hello", color = Color::Blue);
///
/// // Text with multiple styles
/// let styled = text!("Hello", color = Color::Blue, bold, italic);
/// ```
#[macro_export]
macro_rules! text {
    // Just content
    ($content:expr) => {
        $crate::node::TextNode::new($content)
    };
    // Content with modifiers
    ($content:expr, $($modifier:tt)+) => {
        $crate::__text_impl!($crate::node::TextNode::new($content), $($modifier)+)
    };
}

/// Internal helper macro for text! to handle modifiers.
#[macro_export]
#[doc(hidden)]
macro_rules! __text_impl {
    // Base case: just the node, no more modifiers
    ($node:expr) => { $node };
    // Base case: node with trailing comma
    ($node:expr,) => { $node };

    // Key-value modifiers (color = Color::Blue)
    ($node:expr, color = $value:expr) => {
        $node.color($value)
    };
    ($node:expr, color = $value:expr,) => {
        $node.color($value)
    };
    ($node:expr, color = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.color($value), $($rest)+)
    };
    ($node:expr, bg = $value:expr) => {
        $node.bg($value)
    };
    ($node:expr, bg = $value:expr,) => {
        $node.bg($value)
    };
    ($node:expr, bg = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.bg($value), $($rest)+)
    };
    ($node:expr, line_style = $value:expr) => {
        $node.line_style($value)
    };
    ($node:expr, line_style = $value:expr,) => {
        $node.line_style($value)
    };
    ($node:expr, line_style = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.line_style($value), $($rest)+)
    };
    ($node:expr, wrap = $value:expr) => {
        $node.wrap($value)
    };
    ($node:expr, wrap = $value:expr,) => {
        $node.wrap($value)
    };
    ($node:expr, wrap = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.wrap($value), $($rest)+)
    };
    ($node:expr, width = $value:expr) => {
        $node.width($value)
    };
    ($node:expr, width = $value:expr,) => {
        $node.width($value)
    };
    ($node:expr, width = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.width($value), $($rest)+)
    };
    ($node:expr, flex_grow = $value:expr) => {
        $node.flex_grow($value)
    };
    ($node:expr, flex_grow = $value:expr,) => {
        $node.flex_grow($value)
    };
    ($node:expr, flex_grow = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.flex_grow($value), $($rest)+)
    };
    ($node:expr, flex_shrink = $value:expr) => {
        $node.flex_shrink($value)
    };
    ($node:expr, flex_shrink = $value:expr,) => {
        $node.flex_shrink($value)
    };
    ($node:expr, flex_shrink = $value:expr, $($rest:tt)+) => {
        $crate::__text_impl!($node.flex_shrink($value), $($rest)+)
    };

    // Flag modifiers (bold, italic, etc.)
    ($node:expr, bold) => {
        $node.bold()
    };
    ($node:expr, bold,) => {
        $node.bold()
    };
    ($node:expr, bold, $($rest:tt)+) => {
        $crate::__text_impl!($node.bold(), $($rest)+)
    };
    ($node:expr, italic) => {
        $node.italic()
    };
    ($node:expr, italic,) => {
        $node.italic()
    };
    ($node:expr, italic, $($rest:tt)+) => {
        $crate::__text_impl!($node.italic(), $($rest)+)
    };
    ($node:expr, underline) => {
        $node.underline()
    };
    ($node:expr, underline,) => {
        $node.underline()
    };
    ($node:expr, underline, $($rest:tt)+) => {
        $crate::__text_impl!($node.underline(), $($rest)+)
    };
    ($node:expr, strikethrough) => {
        $node.strikethrough()
    };
    ($node:expr, strikethrough,) => {
        $node.strikethrough()
    };
    ($node:expr, strikethrough, $($rest:tt)+) => {
        $crate::__text_impl!($node.strikethrough(), $($rest)+)
    };
    ($node:expr, dim) => {
        $node.dim()
    };
    ($node:expr, dim,) => {
        $node.dim()
    };
    ($node:expr, dim, $($rest:tt)+) => {
        $crate::__text_impl!($node.dim(), $($rest)+)
    };
    ($node:expr, inverse) => {
        $node.inverse()
    };
    ($node:expr, inverse,) => {
        $node.inverse()
    };
    ($node:expr, inverse, $($rest:tt)+) => {
        $crate::__text_impl!($node.inverse(), $($rest)+)
    };
    ($node:expr, truncate) => {
        $node.truncate()
    };
    ($node:expr, truncate,) => {
        $node.truncate()
    };
    ($node:expr, truncate, $($rest:tt)+) => {
        $crate::__text_impl!($node.truncate(), $($rest)+)
    };
}

/// Creates a declarative UI tree with an AI-friendly syntax.
///
/// The `ink!` macro provides a simpler declarative syntax for building
/// UI trees. It's designed to be easy for AI models to generate while still
/// being readable and maintainable by humans.
///
/// # Syntax
///
/// The macro wraps children in a container with optional properties.
///
/// ## Basic usage
///
/// ```rust
/// use inky::prelude::*;
///
/// // Create a column with children
/// let ui = ink!(column;
///     text!("Header", bold),
///     text!("Content"),
///     text!("Footer"),
/// );
///
/// // Create a row with children
/// let row = ink!(row;
///     text!("Left"),
///     Spacer::new(),
///     text!("Right"),
/// );
/// ```
///
/// ## With properties
///
/// ```rust
/// use inky::prelude::*;
///
/// let ui = ink!(column, padding = 2, gap = 1.0, border = BorderStyle::Rounded;
///     text!("Hello", color = Color::Blue),
///     text!("World"),
/// );
/// ```
///
/// # Supported Containers
///
/// - `column` - Vertical flex container (flex-direction: column)
/// - `row` - Horizontal flex container (flex-direction: row)
/// - `box` - Default flex container
///
/// # Properties
///
/// Common properties supported in element declarations:
///
/// - `padding = N` - Set padding on all sides
/// - `margin = N` - Set margin on all sides
/// - `gap = N` - Gap between flex children
/// - `border = BorderStyle::X` - Border style
/// - `width = Dimension::X` - Width constraint
/// - `height = Dimension::X` - Height constraint
/// - `bg = Color::X` - Background color
///
/// # AI Generation Tips
///
/// When generating UI code with this macro:
///
/// 1. Use `column` for vertical layouts, `row` for horizontal
/// 2. Use `text!()` macro for styled text children
/// 3. Use `Spacer::new()` to push elements apart
/// 4. Properties go before the `;` semicolon
/// 5. Children follow the `;`, separated by commas
#[macro_export]
macro_rules! ink {
    // Column with properties and children
    (column, $($prop:ident = $val:expr),+ ; $($child:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut node = $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Column);
        $(node = $crate::__ink_prop!(node, $prop = $val);)*
        $(node = node.child($child);)*
        node
    }};

    // Column with children only
    (column ; $($child:expr),* $(,)?) => {{
        let mut node = $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Column);
        $(node = node.child($child);)*
        node
    }};

    // Row with properties and children
    (row, $($prop:ident = $val:expr),+ ; $($child:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut node = $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Row);
        $(node = $crate::__ink_prop!(node, $prop = $val);)*
        $(node = node.child($child);)*
        node
    }};

    // Row with children only
    (row ; $($child:expr),* $(,)?) => {{
        let mut node = $crate::node::BoxNode::new()
            .flex_direction($crate::style::FlexDirection::Row);
        $(node = node.child($child);)*
        node
    }};

    // Box with properties and children
    (box, $($prop:ident = $val:expr),+ ; $($child:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut node = $crate::node::BoxNode::new();
        $(node = $crate::__ink_prop!(node, $prop = $val);)*
        $(node = node.child($child);)*
        node
    }};

    // Box with children only
    (box ; $($child:expr),* $(,)?) => {{
        let mut node = $crate::node::BoxNode::new();
        $(node = node.child($child);)*
        node
    }};

    // Empty variants
    (column) => { $crate::node::BoxNode::new().flex_direction($crate::style::FlexDirection::Column) };
    (row) => { $crate::node::BoxNode::new().flex_direction($crate::style::FlexDirection::Row) };
    (box) => { $crate::node::BoxNode::new() };
}

/// Internal helper for applying box properties.
#[macro_export]
#[doc(hidden)]
macro_rules! __ink_prop {
    ($node:expr, padding = $val:expr) => {
        $node.padding($val)
    };
    ($node:expr, margin = $val:expr) => {
        $node.margin($val)
    };
    ($node:expr, gap = $val:expr) => {
        $node.gap($val)
    };
    ($node:expr, border = $val:expr) => {
        $node.border($val)
    };
    ($node:expr, width = $val:expr) => {
        $node.width($val)
    };
    ($node:expr, height = $val:expr) => {
        $node.height($val)
    };
    ($node:expr, bg = $val:expr) => {
        $node.background($val)
    };
    ($node:expr, flex_grow = $val:expr) => {
        $node.flex_grow($val)
    };
    ($node:expr, flex_shrink = $val:expr) => {
        $node.flex_shrink($val)
    };
    ($node:expr, justify = $val:expr) => {
        $node.justify_content($val)
    };
    ($node:expr, align = $val:expr) => {
        $node.align_items($val)
    };
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn test_vbox_empty() {
        let node = vbox![];
        assert_eq!(node.style.flex_direction, FlexDirection::Column);
        assert_eq!(node.children.len(), 0);
    }

    #[test]
    fn test_vbox_with_children() {
        let node = vbox![
            TextNode::new("Header"),
            TextNode::new("Content"),
            TextNode::new("Footer"),
        ];
        assert_eq!(node.style.flex_direction, FlexDirection::Column);
        assert_eq!(node.children.len(), 3);
    }

    #[test]
    fn test_hbox_empty() {
        let node = hbox![];
        assert_eq!(node.style.flex_direction, FlexDirection::Row);
        assert_eq!(node.children.len(), 0);
    }

    #[test]
    fn test_hbox_with_children() {
        let node = hbox![TextNode::new("Left"), Spacer::new(), TextNode::new("Right"),];
        assert_eq!(node.style.flex_direction, FlexDirection::Row);
        assert_eq!(node.children.len(), 3);
    }

    #[test]
    fn test_hbox_trailing_comma() {
        let node = hbox![TextNode::new("One"), TextNode::new("Two"),];
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_style_empty() {
        let s = style! {};
        assert_eq!(s, Style::default());
    }

    #[test]
    fn test_style_with_properties() {
        let s = style! {
            padding: 2,
            border: BorderStyle::Rounded,
            flex_direction: FlexDirection::Column,
        };
        assert_eq!(s.padding, Edges::all(2.0));
        assert_eq!(s.border, BorderStyle::Rounded);
        assert_eq!(s.flex_direction, FlexDirection::Column);
    }

    #[test]
    fn test_style_single_property() {
        let s = style! { gap: 5.0 };
        assert_eq!(s.gap, 5.0);
    }

    #[test]
    fn test_text_simple() {
        let node = text!("Hello");
        assert_eq!(node.content, "Hello");
    }

    #[test]
    fn test_text_with_color() {
        let node = text!("Hello", color = Color::Blue);
        assert_eq!(node.content, "Hello");
        assert_eq!(node.text_style.color, Some(Color::Blue));
    }

    #[test]
    fn test_text_with_bold() {
        let node = text!("Hello", bold);
        assert!(node.text_style.bold);
    }

    #[test]
    fn test_text_with_multiple_flags() {
        let node = text!("Hello", bold, italic, underline);
        assert!(node.text_style.bold);
        assert!(node.text_style.italic);
        assert!(node.text_style.underline);
    }

    #[test]
    fn test_text_with_color_and_flags() {
        let node = text!("Hello", color = Color::Red, bold, italic);
        assert_eq!(node.text_style.color, Some(Color::Red));
        assert!(node.text_style.bold);
        assert!(node.text_style.italic);
    }

    #[test]
    fn test_text_with_bg() {
        let node = text!("Hello", bg = Color::Yellow);
        assert_eq!(node.text_style.background_color, Some(Color::Yellow));
    }

    #[test]
    fn test_text_with_line_style() {
        let line_style = TextStyle::new().bg(Color::Yellow).bold();
        let node = text!("Hello", line_style = line_style.clone());
        assert_eq!(node.line_style, Some(line_style));
    }

    #[test]
    fn test_text_with_all_modifiers() {
        let node = text!(
            "Test",
            color = Color::Blue,
            bg = Color::White,
            bold,
            italic,
            underline,
            dim,
        );
        assert_eq!(node.text_style.color, Some(Color::Blue));
        assert_eq!(node.text_style.background_color, Some(Color::White));
        assert!(node.text_style.bold);
        assert!(node.text_style.italic);
        assert!(node.text_style.underline);
        assert!(node.text_style.dim);
    }

    #[test]
    fn test_nested_macros() {
        let ui = vbox![
            hbox![text!("Left", bold), Spacer::new(), text!("Right", italic),],
            text!("Footer", color = Color::Green),
        ];
        assert_eq!(ui.style.flex_direction, FlexDirection::Column);
        assert_eq!(ui.children.len(), 2);
    }

    #[test]
    fn test_vbox_converted_to_node() {
        let _node: Node = vbox![TextNode::new("Test")].into();
    }

    #[test]
    fn test_hbox_converted_to_node() {
        let _node: Node = hbox![TextNode::new("Test")].into();
    }

    // ink! macro tests

    #[test]
    fn test_ink_empty_column() {
        let node = ink!(column);
        assert_eq!(node.style.flex_direction, FlexDirection::Column);
        assert_eq!(node.children.len(), 0);
    }

    #[test]
    fn test_ink_empty_row() {
        let node = ink!(row);
        assert_eq!(node.style.flex_direction, FlexDirection::Row);
        assert_eq!(node.children.len(), 0);
    }

    #[test]
    fn test_ink_column_simple() {
        let node = ink!(column;
            text!("Hello"),
            text!("World"),
        );
        assert_eq!(node.style.flex_direction, FlexDirection::Column);
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_ink_row_simple() {
        let node = ink!(row;
            text!("Left"),
            text!("Right"),
        );
        assert_eq!(node.style.flex_direction, FlexDirection::Row);
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_ink_with_properties() {
        let node = ink!(column, padding = 2, gap = 1.0;
            text!("Header"),
        );
        assert_eq!(node.style.padding, Edges::all(2.0));
        assert_eq!(node.style.gap, 1.0);
    }

    #[test]
    fn test_ink_text_with_styles() {
        let _node = ink!(column;
            text!("Bold", bold),
            text!("Colored", color = Color::Blue),
        );
        // Just verify it compiles
    }

    #[test]
    fn test_ink_spacer() {
        let node = ink!(row;
            text!("Left"),
            Spacer::new(),
            text!("Right"),
        );
        assert_eq!(node.children.len(), 3);
    }

    #[test]
    fn test_ink_progress() {
        let node = ink!(column;
            text!("Loading..."),
            Progress::new().value(50, 100),
        );
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_ink_nested() {
        let node = ink!(column;
            ink!(row;
                text!("Left"),
                text!("Right"),
            ),
            text!("Footer"),
        );
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_ink_box_with_border() {
        let node = ink!(box, border = BorderStyle::Rounded, padding = 1;
            text!("Boxed content"),
        );
        assert_eq!(node.style.border, BorderStyle::Rounded);
        assert_eq!(node.style.padding, Edges::all(1.0));
    }

    #[test]
    fn test_ink_trailing_comma() {
        let node = ink!(column;
            text!("One"),
            text!("Two"),
        );
        assert_eq!(node.children.len(), 2);
    }
}
