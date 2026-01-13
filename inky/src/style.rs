//! Style definitions for layout and appearance.
//!
//! Styles are used to control both layout (via Taffy) and visual appearance
//! (colors, text formatting) of nodes.

use std::borrow::Cow;
use taffy::Overflow as TaffyOverflow;

// === Layout Style ===

/// Complete style for a node, covering layout and appearance.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // Display
    /// Display mode (Flex or None).
    pub display: Display,

    // Flex container properties
    /// Direction of the main axis.
    pub flex_direction: FlexDirection,
    /// Whether items wrap to multiple lines.
    pub flex_wrap: FlexWrap,
    /// Alignment along the main axis.
    pub justify_content: JustifyContent,
    /// Alignment along the cross axis.
    pub align_items: AlignItems,
    /// Alignment of wrapped lines.
    pub align_content: AlignContent,
    /// Gap between items.
    pub gap: f32,

    // Flex item properties
    /// How much the item grows to fill available space.
    pub flex_grow: f32,
    /// How much the item shrinks when space is limited.
    pub flex_shrink: f32,
    /// Initial size before flex grow/shrink.
    pub flex_basis: Dimension,
    /// Override align-items for this specific item.
    pub align_self: AlignSelf,

    // Size properties
    /// Width of the node.
    pub width: Dimension,
    /// Height of the node.
    pub height: Dimension,
    /// Minimum width.
    pub min_width: Dimension,
    /// Minimum height.
    pub min_height: Dimension,
    /// Maximum width.
    pub max_width: Dimension,
    /// Maximum height.
    pub max_height: Dimension,

    // Spacing
    /// Inner spacing (inside the border).
    pub padding: Edges,
    /// Outer spacing (outside the border).
    pub margin: Edges,

    // Border
    /// Border style and color.
    pub border: BorderStyle,

    // Overflow
    /// How to handle content that exceeds the node's bounds.
    pub overflow: Overflow,

    // Background
    /// Background color.
    pub background_color: Option<Color>,
}

impl Style {
    /// Create a new style with default values.
    ///
    /// This is a const fn, enabling compile-time style initialization.
    pub const fn new() -> Self {
        Self {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            align_content: AlignContent::Stretch,
            gap: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            align_self: AlignSelf::Auto,
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_width: Dimension::Auto,
            min_height: Dimension::Auto,
            max_width: Dimension::Auto,
            max_height: Dimension::Auto,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            border: BorderStyle::None,
            overflow: Overflow::Visible,
            background_color: None,
        }
    }

    /// Default style as a compile-time constant.
    pub const DEFAULT: Self = Self::new();

    /// Column layout style constant.
    pub const COLUMN: Self = Self {
        flex_direction: FlexDirection::Column,
        ..Self::DEFAULT
    };

    /// Row layout style constant.
    pub const ROW: Self = Self {
        flex_direction: FlexDirection::Row,
        ..Self::DEFAULT
    };

    /// Centered content style constant.
    pub const CENTERED: Self = Self {
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Self::DEFAULT
    };
}

impl Default for Style {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Style {
    /// Convert to Taffy style for layout computation.
    pub fn to_taffy(&self) -> taffy::Style {
        taffy::Style {
            display: self.display.to_taffy(),
            flex_direction: self.flex_direction.to_taffy(),
            flex_wrap: self.flex_wrap.to_taffy(),
            justify_content: Some(self.justify_content.to_taffy()),
            align_items: Some(self.align_items.to_taffy()),
            align_content: Some(self.align_content.to_taffy()),
            gap: taffy::Size {
                width: taffy::LengthPercentage::Length(self.gap),
                height: taffy::LengthPercentage::Length(self.gap),
            },
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            flex_basis: self.flex_basis.to_taffy(),
            align_self: Some(self.align_self.to_taffy()),
            size: taffy::Size {
                width: self.width.to_taffy(),
                height: self.height.to_taffy(),
            },
            min_size: taffy::Size {
                width: self.min_width.to_taffy(),
                height: self.min_height.to_taffy(),
            },
            max_size: taffy::Size {
                width: self.max_width.to_taffy(),
                height: self.max_height.to_taffy(),
            },
            padding: self.padding.to_taffy(),
            margin: self.margin.to_taffy_margin(),
            border: self.border.to_taffy_border(),
            overflow: taffy::Point {
                x: self.overflow.to_taffy(),
                y: self.overflow.to_taffy(),
            },
            ..Default::default()
        }
    }
}

// === Display ===

/// Display mode for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    /// Flexbox layout.
    #[default]
    Flex,
    /// Node is hidden and takes no space.
    None,
}

impl Display {
    #[inline]
    fn to_taffy(self) -> taffy::Display {
        match self {
            Display::Flex => taffy::Display::Flex,
            Display::None => taffy::Display::None,
        }
    }
}

// === Flex Direction ===

/// Direction of the main axis in flexbox layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    /// Main axis is horizontal, left to right.
    #[default]
    Row,
    /// Main axis is horizontal, right to left.
    RowReverse,
    /// Main axis is vertical, top to bottom.
    Column,
    /// Main axis is vertical, bottom to top.
    ColumnReverse,
}

impl FlexDirection {
    #[inline]
    fn to_taffy(self) -> taffy::FlexDirection {
        match self {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        }
    }
}

// === Flex Wrap ===

/// Whether flex items wrap to multiple lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    /// Items stay on a single line.
    #[default]
    NoWrap,
    /// Items wrap to multiple lines.
    Wrap,
    /// Items wrap in reverse order.
    WrapReverse,
}

impl FlexWrap {
    #[inline]
    fn to_taffy(self) -> taffy::FlexWrap {
        match self {
            FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        }
    }
}

// === Justify Content ===

/// Alignment of items along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    /// Items packed at the start.
    #[default]
    Start,
    /// Items packed at the end.
    End,
    /// Items centered.
    Center,
    /// Items evenly distributed; first at start, last at end.
    SpaceBetween,
    /// Items evenly distributed with equal space around.
    SpaceAround,
    /// Items evenly distributed with equal space between.
    SpaceEvenly,
}

impl JustifyContent {
    #[inline]
    fn to_taffy(self) -> taffy::JustifyContent {
        match self {
            JustifyContent::Start => taffy::JustifyContent::Start,
            JustifyContent::End => taffy::JustifyContent::End,
            JustifyContent::Center => taffy::JustifyContent::Center,
            JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
            JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
            JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
        }
    }
}

// === Align Items ===

/// Alignment of items along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    /// Items aligned at the start of cross axis.
    Start,
    /// Items aligned at the end of cross axis.
    End,
    /// Items centered on cross axis.
    Center,
    /// Items aligned at text baseline.
    Baseline,
    /// Items stretched to fill cross axis.
    #[default]
    Stretch,
}

impl AlignItems {
    #[inline]
    fn to_taffy(self) -> taffy::AlignItems {
        match self {
            AlignItems::Start => taffy::AlignItems::Start,
            AlignItems::End => taffy::AlignItems::End,
            AlignItems::Center => taffy::AlignItems::Center,
            AlignItems::Baseline => taffy::AlignItems::Baseline,
            AlignItems::Stretch => taffy::AlignItems::Stretch,
        }
    }
}

// === Align Content ===

/// Alignment of wrapped lines along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignContent {
    /// Lines packed at the start.
    Start,
    /// Lines packed at the end.
    End,
    /// Lines centered.
    Center,
    /// Lines evenly distributed; first at start, last at end.
    SpaceBetween,
    /// Lines evenly distributed with equal space around.
    SpaceAround,
    /// Lines evenly distributed with equal space between.
    SpaceEvenly,
    /// Lines stretched to fill.
    #[default]
    Stretch,
}

impl AlignContent {
    #[inline]
    fn to_taffy(self) -> taffy::AlignContent {
        match self {
            AlignContent::Start => taffy::AlignContent::Start,
            AlignContent::End => taffy::AlignContent::End,
            AlignContent::Center => taffy::AlignContent::Center,
            AlignContent::SpaceBetween => taffy::AlignContent::SpaceBetween,
            AlignContent::SpaceAround => taffy::AlignContent::SpaceAround,
            AlignContent::SpaceEvenly => taffy::AlignContent::SpaceEvenly,
            AlignContent::Stretch => taffy::AlignContent::Stretch,
        }
    }
}

// === Align Self ===

/// Override align-items for a specific item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignSelf {
    /// Use parent's align-items value.
    #[default]
    Auto,
    /// Align at the start of cross axis.
    Start,
    /// Align at the end of cross axis.
    End,
    /// Center on cross axis.
    Center,
    /// Align at text baseline.
    Baseline,
    /// Stretch to fill cross axis.
    Stretch,
}

impl AlignSelf {
    #[inline]
    fn to_taffy(self) -> taffy::AlignSelf {
        match self {
            // Auto means "inherit from parent's align-items", which in Taffy is Stretch
            AlignSelf::Auto => taffy::AlignSelf::Stretch,
            AlignSelf::Start => taffy::AlignSelf::Start,
            AlignSelf::End => taffy::AlignSelf::End,
            AlignSelf::Center => taffy::AlignSelf::Center,
            AlignSelf::Baseline => taffy::AlignSelf::Baseline,
            AlignSelf::Stretch => taffy::AlignSelf::Stretch,
        }
    }
}

// === Dimension ===

/// A size dimension that can be fixed, percentage, or auto.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dimension {
    /// Automatically sized based on content.
    #[default]
    Auto,
    /// Fixed size in terminal cells.
    Length(f32),
    /// Percentage of parent's size.
    Percent(f32),
}

impl Dimension {
    #[inline]
    fn to_taffy(self) -> taffy::Dimension {
        match self {
            Dimension::Auto => taffy::Dimension::Auto,
            Dimension::Length(v) => taffy::Dimension::Length(v),
            Dimension::Percent(v) => taffy::Dimension::Percent(v / 100.0),
        }
    }
}

impl From<f32> for Dimension {
    fn from(v: f32) -> Self {
        Dimension::Length(v)
    }
}

impl From<i32> for Dimension {
    fn from(v: i32) -> Self {
        Dimension::Length(v as f32)
    }
}

impl From<u16> for Dimension {
    fn from(v: u16) -> Self {
        Dimension::Length(v as f32)
    }
}

/// Create a length dimension.
pub const fn length(v: f32) -> Dimension {
    Dimension::Length(v)
}

/// Create a percentage dimension.
pub const fn percent(v: f32) -> Dimension {
    Dimension::Percent(v)
}

/// Create an auto dimension.
pub const fn auto() -> Dimension {
    Dimension::Auto
}

// === Edges ===

/// Spacing values for all four edges.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    /// Top edge spacing.
    pub top: f32,
    /// Right edge spacing.
    pub right: f32,
    /// Bottom edge spacing.
    pub bottom: f32,
    /// Left edge spacing.
    pub left: f32,
}

impl Edges {
    /// Zero edges constant.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Create edges with all zeros.
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Create edges with the same value on all sides.
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create edges with horizontal (x) and vertical (y) values.
    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }

    /// Create edges with individual values.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    #[inline]
    fn to_taffy(self) -> taffy::Rect<taffy::LengthPercentage> {
        taffy::Rect {
            top: taffy::LengthPercentage::Length(self.top),
            right: taffy::LengthPercentage::Length(self.right),
            bottom: taffy::LengthPercentage::Length(self.bottom),
            left: taffy::LengthPercentage::Length(self.left),
        }
    }

    #[inline]
    fn to_taffy_margin(self) -> taffy::Rect<taffy::LengthPercentageAuto> {
        taffy::Rect {
            top: taffy::LengthPercentageAuto::Length(self.top),
            right: taffy::LengthPercentageAuto::Length(self.right),
            bottom: taffy::LengthPercentageAuto::Length(self.bottom),
            left: taffy::LengthPercentageAuto::Length(self.left),
        }
    }
}

impl From<f32> for Edges {
    fn from(v: f32) -> Self {
        Edges::all(v)
    }
}

impl From<i32> for Edges {
    fn from(v: i32) -> Self {
        Edges::all(v as f32)
    }
}

impl From<(f32, f32)> for Edges {
    fn from((x, y): (f32, f32)) -> Self {
        Edges::xy(x, y)
    }
}

impl From<(f32, f32, f32, f32)> for Edges {
    fn from((t, r, b, l): (f32, f32, f32, f32)) -> Self {
        Edges::new(t, r, b, l)
    }
}

// === Border Style ===

/// Border style and appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// No border.
    #[default]
    None,
    /// Single line border.
    Single,
    /// Double line border.
    Double,
    /// Rounded corner border.
    Rounded,
    /// Bold/thick border.
    Bold,
}

impl BorderStyle {
    /// Width of this border style (0 or 1).
    #[inline]
    pub fn width(&self) -> f32 {
        match self {
            BorderStyle::None => 0.0,
            _ => 1.0,
        }
    }

    #[inline]
    fn to_taffy_border(self) -> taffy::Rect<taffy::LengthPercentage> {
        let w = self.width();
        taffy::Rect {
            top: taffy::LengthPercentage::Length(w),
            right: taffy::LengthPercentage::Length(w),
            bottom: taffy::LengthPercentage::Length(w),
            left: taffy::LengthPercentage::Length(w),
        }
    }

    /// Get the border characters for this style.
    pub fn chars(&self) -> BorderChars {
        match self {
            BorderStyle::None => BorderChars::NONE,
            BorderStyle::Single => BorderChars::SINGLE,
            BorderStyle::Double => BorderChars::DOUBLE,
            BorderStyle::Rounded => BorderChars::ROUNDED,
            BorderStyle::Bold => BorderChars::BOLD,
        }
    }
}

/// Characters used to draw borders.
#[derive(Debug, Clone, Copy)]
pub struct BorderChars {
    /// Top-left corner character.
    pub top_left: char,
    /// Top-right corner character.
    pub top_right: char,
    /// Bottom-left corner character.
    pub bottom_left: char,
    /// Bottom-right corner character.
    pub bottom_right: char,
    /// Horizontal line character.
    pub horizontal: char,
    /// Vertical line character.
    pub vertical: char,
}

impl BorderChars {
    /// No border (spaces).
    pub const NONE: Self = Self {
        top_left: ' ',
        top_right: ' ',
        bottom_left: ' ',
        bottom_right: ' ',
        horizontal: ' ',
        vertical: ' ',
    };

    /// Single-line border (─│┌┐└┘).
    pub const SINGLE: Self = Self {
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        horizontal: '─',
        vertical: '│',
    };

    /// Double-line border (═║╔╗╚╝).
    pub const DOUBLE: Self = Self {
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        horizontal: '═',
        vertical: '║',
    };

    /// Rounded corner border (─│╭╮╰╯).
    pub const ROUNDED: Self = Self {
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        horizontal: '─',
        vertical: '│',
    };

    /// Bold/thick border (━┃┏┓┗┛).
    pub const BOLD: Self = Self {
        top_left: '┏',
        top_right: '┓',
        bottom_left: '┗',
        bottom_right: '┛',
        horizontal: '━',
        vertical: '┃',
    };
}

// === Overflow ===

/// How to handle content that exceeds bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Content is visible outside bounds.
    #[default]
    Visible,
    /// Content is clipped at bounds.
    Hidden,
    /// Content scrolls within bounds.
    Scroll,
}

impl Overflow {
    #[inline]
    fn to_taffy(self) -> TaffyOverflow {
        match self {
            Overflow::Visible => TaffyOverflow::Visible,
            Overflow::Hidden => TaffyOverflow::Clip,
            Overflow::Scroll => TaffyOverflow::Scroll,
        }
    }
}

// === Text Style ===

/// Text-specific styling.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    /// Text color.
    pub color: Option<Color>,
    /// Background color.
    pub background_color: Option<Color>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Strikethrough text.
    pub strikethrough: bool,
    /// Dim/faint text.
    pub dim: bool,
    /// Inverse foreground/background.
    pub inverse: bool,
    /// Text wrapping mode.
    pub wrap: TextWrap,
}

impl TextStyle {
    /// Create a new text style with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            color: None,
            background_color: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
            inverse: false,
            wrap: TextWrap::Wrap,
        }
    }

    /// Merge this style with another, where `other` takes precedence.
    ///
    /// Colors from `other` override `self` if present. Boolean attributes
    /// are combined with OR (so setting bold in either results in bold).
    /// The wrap mode is taken from `other` if it differs from the default.
    ///
    /// This is similar to ratatui's `Style::patch()`.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let base = TextStyle::new().color(Color::White);
    /// let emphasis = TextStyle::new().bold();
    /// let combined = base.merge(&emphasis);
    /// assert!(combined.bold);
    /// assert_eq!(combined.color, Some(Color::White));
    /// ```
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            color: other.color.or(self.color),
            background_color: other.background_color.or(self.background_color),
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            underline: self.underline || other.underline,
            strikethrough: self.strikethrough || other.strikethrough,
            dim: self.dim || other.dim,
            inverse: self.inverse || other.inverse,
            wrap: if other.wrap != TextWrap::default() {
                other.wrap
            } else {
                self.wrap
            },
        }
    }

    /// Alias for `merge()` for ratatui compatibility.
    ///
    /// Patches this style with values from `other`, where `other` takes precedence.
    #[must_use]
    pub fn patch(&self, other: &Self) -> Self {
        self.merge(other)
    }

    /// Set the foreground color.
    #[must_use]
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the background color.
    #[must_use]
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    /// Set bold attribute.
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Set italic attribute.
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Set underline attribute.
    #[must_use]
    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set strikethrough attribute.
    #[must_use]
    pub const fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Set dim attribute.
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Set inverse attribute.
    #[must_use]
    pub const fn inverse(mut self) -> Self {
        self.inverse = true;
        self
    }

    /// Set text wrap mode.
    #[must_use]
    pub const fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Apply this style to text, creating a `StyledSpan`.
    ///
    /// This is a convenience method for creating styled spans from a style object.
    /// It allows you to define styles once and apply them to multiple pieces of text.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let error_style = TextStyle::new().color(Color::Red).bold();
    /// let warning_style = TextStyle::new().color(Color::Yellow);
    ///
    /// let error_span = error_style.apply("Error: file not found");
    /// let warning_span = warning_style.apply("Warning: deprecated API");
    ///
    /// // Build a line with styled spans
    /// let line = Line::new()
    ///     .span(error_span)
    ///     .span(" | ")
    ///     .span(warning_span);
    /// ```
    #[must_use]
    pub fn apply(&self, text: impl Into<String>) -> StyledSpanOwned {
        StyledSpan {
            text: Cow::Owned(text.into()),
            color: self.color,
            background_color: self.background_color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            dim: self.dim,
            inverse: self.inverse,
        }
    }

    /// Apply this style to text, consuming the style (builder pattern).
    ///
    /// This is identical to [`apply()`](Self::apply) but consumes `self`, making it
    /// more convenient for inline style construction.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// // Inline style construction
    /// let span = TextStyle::new()
    ///     .color(Color::Green)
    ///     .bold()
    ///     .with_text("Success!");
    ///
    /// // Equivalent to:
    /// // let span = StyledSpan::new("Success!").color(Color::Green).bold();
    /// ```
    #[must_use]
    pub fn with_text(self, text: impl Into<String>) -> StyledSpanOwned {
        self.apply(text)
    }
}

// === Styled Span ===

/// A segment of text with its own styling.
///
/// The text is stored as `Cow<'a, str>` to support zero-copy parsing.
/// This enables efficient ANSI parsing for streaming LLM output by avoiding
/// allocations when borrowing from the input string.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// // Owned span (allocates)
/// let span = StyledSpan::new("Hello")
///     .color(Color::Red)
///     .bold();
///
/// // Borrowed span (zero-copy)
/// let text = "World";
/// let borrowed = StyledSpan::borrowed(text).color(Color::Blue);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StyledSpan<'a> {
    /// The text content of this span.
    pub text: Cow<'a, str>,
    /// Text color (foreground).
    pub color: Option<Color>,
    /// Background color.
    pub background_color: Option<Color>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Strikethrough text.
    pub strikethrough: bool,
    /// Dim/faint text.
    pub dim: bool,
    /// Inverse foreground/background.
    pub inverse: bool,
}

/// Type alias for owned spans (no lifetime constraints).
///
/// Use this when storing spans in data structures or when the span
/// needs to outlive the source string.
pub type StyledSpanOwned = StyledSpan<'static>;

impl StyledSpan<'static> {
    /// Create a new owned styled span with default styling.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Cow::Owned(text.into()),
            color: None,
            background_color: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
            inverse: false,
        }
    }
}

impl<'a> StyledSpan<'a> {
    /// Create a borrowed styled span (zero-copy).
    ///
    /// This is more efficient when you have a reference to existing text
    /// and don't need to own the data.
    pub fn borrowed(text: &'a str) -> Self {
        Self {
            text: Cow::Borrowed(text),
            color: None,
            background_color: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
            inverse: false,
        }
    }

    /// Convert to an owned span.
    ///
    /// This copies the text if it was borrowed, or moves it if already owned.
    pub fn into_owned(self) -> StyledSpanOwned {
        StyledSpan {
            text: Cow::Owned(self.text.into_owned()),
            color: self.color,
            background_color: self.background_color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            dim: self.dim,
            inverse: self.inverse,
        }
    }

    /// Set foreground color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set background color.
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    /// Make text bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Make text italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Underline text.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Strikethrough text.
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Dim text.
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Inverse colors.
    pub fn inverse(mut self) -> Self {
        self.inverse = true;
        self
    }

    /// Convert this span to a TextStyle (for rendering).
    pub fn to_text_style(&self) -> TextStyle {
        TextStyle {
            color: self.color,
            background_color: self.background_color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            dim: self.dim,
            inverse: self.inverse,
            wrap: TextWrap::default(),
        }
    }
}

impl Default for StyledSpan<'static> {
    fn default() -> Self {
        Self::new("")
    }
}

impl From<&str> for StyledSpan<'static> {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for StyledSpan<'static> {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

// === Line ===

/// A line of styled text spans.
///
/// `Line` is a first-class type for line-oriented terminal UIs. Unlike `TextNode`,
/// which is part of the layout tree, `Line` represents a single logical line of
/// styled text that can be manipulated, measured, truncated, or wrapped before
/// being converted to a node.
///
/// This type bridges the gap between terminal-native thinking (lines and spans)
/// and inky's node tree model.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
///
/// // Build a status line with multiple styled segments
/// let status = Line::new()
///     .span(StyledSpan::new("✓ ").color(Color::Green))
///     .span(StyledSpan::new("Build succeeded").bold())
///     .span(" in 1.2s");
///
/// // Measure and truncate if needed
/// if status.display_width() > 40 {
///     let truncated = status.truncate(40, Some("…"));
/// }
///
/// // Convert to Node for layout
/// let node: Node = status.into();
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    /// The styled spans that make up this line.
    spans: Vec<StyledSpanOwned>,
    /// Optional line-level style (applied to the entire line background).
    style: Option<TextStyle>,
}

impl Line {
    /// Create a new empty line.
    #[must_use]
    pub fn new() -> Self {
        Self {
            spans: Vec::new(),
            style: None,
        }
    }

    /// Create a line from a single span.
    #[must_use]
    pub fn from_span(span: impl Into<StyledSpanOwned>) -> Self {
        Self {
            spans: vec![span.into()],
            style: None,
        }
    }

    /// Create a line from multiple spans.
    #[must_use]
    pub fn from_spans(spans: impl IntoIterator<Item = impl Into<StyledSpanOwned>>) -> Self {
        Self {
            spans: spans.into_iter().map(Into::into).collect(),
            style: None,
        }
    }

    /// Create a line from plain text.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            spans: vec![StyledSpan::new(text)],
            style: None,
        }
    }

    /// Add a span to this line.
    #[must_use]
    pub fn span(mut self, span: impl Into<StyledSpanOwned>) -> Self {
        self.spans.push(span.into());
        self
    }

    /// Add multiple spans to this line.
    #[must_use]
    pub fn spans(mut self, spans: impl IntoIterator<Item = impl Into<StyledSpanOwned>>) -> Self {
        self.spans.extend(spans.into_iter().map(Into::into));
        self
    }

    /// Set line-level style (applies to the entire line, e.g., background color).
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Get the line-level style, if any.
    #[must_use]
    pub fn get_style(&self) -> Option<&TextStyle> {
        self.style.as_ref()
    }

    /// Get the spans in this line.
    #[must_use]
    pub fn get_spans(&self) -> &[StyledSpanOwned] {
        &self.spans
    }

    /// Consume this line and return its spans.
    #[must_use]
    pub fn into_spans(self) -> Vec<StyledSpanOwned> {
        self.spans
    }

    /// Calculate the display width of this line.
    ///
    /// Accounts for Unicode character widths (e.g., CJK characters are width 2).
    #[must_use]
    pub fn display_width(&self) -> usize {
        use unicode_width::UnicodeWidthChar;

        self.spans
            .iter()
            .map(|span| {
                span.text
                    .chars()
                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
                    .sum::<usize>()
            })
            .sum()
    }

    /// Truncate the line to fit within a maximum width.
    ///
    /// If the line is longer than `max_width`, it will be truncated and the
    /// optional ellipsis string will be appended.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let line = Line::text("This is a very long message that needs truncation");
    /// let truncated = line.truncate(20, Some("…"));
    /// assert!(truncated.display_width() <= 20);
    /// ```
    #[must_use]
    pub fn truncate(&self, max_width: usize, ellipsis: Option<&str>) -> Line {
        use unicode_width::UnicodeWidthChar;

        let current_width = self.display_width();
        if current_width <= max_width {
            return self.clone();
        }

        let ellipsis_width = ellipsis
            .map(|e| {
                e.chars()
                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
                    .sum()
            })
            .unwrap_or(0);
        let target_width = max_width.saturating_sub(ellipsis_width);

        let mut result_spans = Vec::new();
        let mut accumulated_width = 0;

        for span in &self.spans {
            if accumulated_width >= target_width {
                break;
            }

            let mut truncated_text = String::new();
            for c in span.text.chars() {
                let char_width = UnicodeWidthChar::width(c).unwrap_or(1);
                if accumulated_width + char_width > target_width {
                    break;
                }
                truncated_text.push(c);
                accumulated_width += char_width;
            }

            if !truncated_text.is_empty() {
                result_spans.push(StyledSpan {
                    text: Cow::Owned(truncated_text),
                    color: span.color,
                    background_color: span.background_color,
                    bold: span.bold,
                    italic: span.italic,
                    underline: span.underline,
                    strikethrough: span.strikethrough,
                    dim: span.dim,
                    inverse: span.inverse,
                });
            }
        }

        // Add ellipsis if provided
        if let Some(e) = ellipsis {
            if !e.is_empty() {
                result_spans.push(StyledSpan::new(e));
            }
        }

        Line {
            spans: result_spans,
            style: self.style.clone(),
        }
    }

    /// Pad the line to a minimum width.
    ///
    /// Adds spaces to the end of the line to reach the target width.
    /// If the line is already wider than `width`, it is returned unchanged.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let line = Line::text("Hi");
    /// let padded = line.pad(10);
    /// assert_eq!(padded.display_width(), 10);
    /// ```
    #[must_use]
    pub fn pad(&self, width: usize) -> Line {
        let current_width = self.display_width();
        if current_width >= width {
            return self.clone();
        }

        let padding = width - current_width;
        let mut result = self.clone();

        // Add padding as a plain span
        result.spans.push(StyledSpan::new(" ".repeat(padding)));

        result
    }

    /// Wrap this line to fit within a maximum width.
    ///
    /// Returns multiple lines, each fitting within `max_width`. Word boundaries
    /// are respected where possible.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::prelude::*;
    ///
    /// let line = Line::text("The quick brown fox jumps over the lazy dog");
    /// let wrapped = line.wrap(20);
    /// for wrapped_line in wrapped {
    ///     assert!(wrapped_line.display_width() <= 20);
    /// }
    /// ```
    #[must_use]
    pub fn wrap(&self, max_width: usize) -> Vec<Line> {
        use unicode_width::UnicodeWidthChar;

        if max_width == 0 {
            return vec![Line::new()];
        }

        // If line fits, return as-is
        if self.display_width() <= max_width {
            return vec![self.clone()];
        }

        let mut result_lines: Vec<Line> = Vec::new();
        let mut current_line = Line::new();
        let mut current_width = 0;

        // Process each span
        for span in &self.spans {
            let mut remaining = span.text.as_ref();

            while !remaining.is_empty() {
                // Find word boundary or fill to width
                let mut word_end = 0;
                let mut word_width = 0;

                // Find next word (or character if no spaces)
                for (i, c) in remaining.char_indices() {
                    let char_width = UnicodeWidthChar::width(c).unwrap_or(1);

                    if c.is_whitespace() {
                        // Include whitespace in current word
                        word_end = i + c.len_utf8();
                        word_width += char_width;
                        break;
                    }

                    word_width += char_width;
                    word_end = i + c.len_utf8();

                    // If single word exceeds line, break at character boundary
                    if current_width + word_width > max_width && current_width > 0 {
                        // Start new line with this word
                        result_lines.push(std::mem::take(&mut current_line));
                        current_line.style.clone_from(&self.style);
                        current_width = 0;
                    }
                }

                // Handle case where we reached end without finding whitespace
                if word_end == 0 {
                    break;
                }

                let word = &remaining[..word_end];
                remaining = &remaining[word_end..];

                // Check if word fits on current line
                if current_width + word_width <= max_width {
                    current_line.spans.push(StyledSpan {
                        text: Cow::Owned(word.to_string()),
                        color: span.color,
                        background_color: span.background_color,
                        bold: span.bold,
                        italic: span.italic,
                        underline: span.underline,
                        strikethrough: span.strikethrough,
                        dim: span.dim,
                        inverse: span.inverse,
                    });
                    current_width += word_width;
                } else {
                    // Start new line
                    if !current_line.spans.is_empty() {
                        result_lines.push(std::mem::take(&mut current_line));
                        current_line.style.clone_from(&self.style);
                    }

                    // Handle word that's too long for any line
                    if word_width > max_width {
                        let mut chunk = String::new();
                        let mut chunk_width = 0;

                        for c in word.chars() {
                            let char_width = UnicodeWidthChar::width(c).unwrap_or(1);

                            if chunk_width + char_width > max_width && !chunk.is_empty() {
                                current_line.spans.push(StyledSpan {
                                    text: Cow::Owned(std::mem::take(&mut chunk)),
                                    color: span.color,
                                    background_color: span.background_color,
                                    bold: span.bold,
                                    italic: span.italic,
                                    underline: span.underline,
                                    strikethrough: span.strikethrough,
                                    dim: span.dim,
                                    inverse: span.inverse,
                                });
                                result_lines.push(std::mem::take(&mut current_line));
                                current_line.style.clone_from(&self.style);
                                chunk_width = 0;
                            }

                            chunk.push(c);
                            chunk_width += char_width;
                        }

                        if !chunk.is_empty() {
                            current_line.spans.push(StyledSpan {
                                text: Cow::Owned(chunk),
                                color: span.color,
                                background_color: span.background_color,
                                bold: span.bold,
                                italic: span.italic,
                                underline: span.underline,
                                strikethrough: span.strikethrough,
                                dim: span.dim,
                                inverse: span.inverse,
                            });
                            current_width = chunk_width;
                        }
                    } else {
                        current_line.spans.push(StyledSpan {
                            text: Cow::Owned(word.to_string()),
                            color: span.color,
                            background_color: span.background_color,
                            bold: span.bold,
                            italic: span.italic,
                            underline: span.underline,
                            strikethrough: span.strikethrough,
                            dim: span.dim,
                            inverse: span.inverse,
                        });
                        current_width = word_width;
                    }
                }
            }
        }

        // Don't forget the last line
        if !current_line.spans.is_empty() {
            result_lines.push(current_line);
        }

        if result_lines.is_empty() {
            result_lines.push(Line::new());
        }

        result_lines
    }

    /// Check if the line is empty (contains no spans or only empty spans).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty() || self.spans.iter().all(|s| s.text.is_empty())
    }

    /// Get the plain text content of this line (without styling).
    #[must_use]
    pub fn as_str(&self) -> String {
        self.spans.iter().map(|s| s.text.as_ref()).collect()
    }
}

impl From<&str> for Line {
    fn from(s: &str) -> Self {
        Line::text(s)
    }
}

impl From<String> for Line {
    fn from(s: String) -> Self {
        Line::text(s)
    }
}

impl<'a> From<StyledSpan<'a>> for Line {
    fn from(span: StyledSpan<'a>) -> Self {
        Line::from_span(span.into_owned())
    }
}

impl<T: Into<StyledSpanOwned>> FromIterator<T> for Line {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Line::from_spans(iter)
    }
}

impl From<Vec<StyledSpanOwned>> for Line {
    fn from(spans: Vec<StyledSpanOwned>) -> Self {
        Line { spans, style: None }
    }
}

// === Text Wrap ===

/// How text wraps when it exceeds the available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    /// Wrap at word boundaries.
    #[default]
    Wrap,
    /// No wrapping.
    NoWrap,
    /// Truncate at end with ellipsis.
    Truncate,
    /// Truncate at start with ellipsis.
    TruncateStart,
    /// Truncate in middle with ellipsis.
    TruncateMiddle,
}

// === Color ===

/// Terminal color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Default terminal color.
    Default,
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
    /// Bright/light black (gray).
    BrightBlack,
    /// Bright/light red.
    BrightRed,
    /// Bright/light green.
    BrightGreen,
    /// Bright/light yellow.
    BrightYellow,
    /// Bright/light blue.
    BrightBlue,
    /// Bright/light magenta.
    BrightMagenta,
    /// Bright/light cyan.
    BrightCyan,
    /// Bright/light white.
    BrightWhite,
    /// 8-bit color (0-255).
    Ansi256(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Create an RGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgb(r, g, b)
    }

    /// Create a color from a hex string (e.g., "#ff0000" or "ff0000").
    pub fn hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    }

    /// Convert to crossterm color.
    pub fn to_crossterm(self) -> crossterm::style::Color {
        use crossterm::style::Color as C;
        match self {
            Color::Default => C::Reset,
            Color::Black => C::Black,
            Color::Red => C::DarkRed,
            Color::Green => C::DarkGreen,
            Color::Yellow => C::DarkYellow,
            Color::Blue => C::DarkBlue,
            Color::Magenta => C::DarkMagenta,
            Color::Cyan => C::DarkCyan,
            Color::White => C::Grey,
            Color::BrightBlack => C::DarkGrey,
            Color::BrightRed => C::Red,
            Color::BrightGreen => C::Green,
            Color::BrightYellow => C::Yellow,
            Color::BrightBlue => C::Blue,
            Color::BrightMagenta => C::Magenta,
            Color::BrightCyan => C::Cyan,
            Color::BrightWhite => C::White,
            Color::Ansi256(n) => C::AnsiValue(n),
            Color::Rgb(r, g, b) => C::Rgb { r, g, b },
        }
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "white" => Color::White,
            "gray" | "grey" => Color::BrightBlack,
            s if s.starts_with('#') || s.len() == 6 => Color::hex(s).unwrap_or(Color::Default),
            _ => Color::Default,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_style_to_taffy() {
        let style = Style {
            flex_direction: FlexDirection::Column,
            padding: Edges::all(2.0),
            ..Default::default()
        };
        let taffy_style = style.to_taffy();
        assert_eq!(taffy_style.flex_direction, taffy::FlexDirection::Column);
    }

    #[test]
    fn test_dimension_from() {
        assert_eq!(Dimension::from(10.0), Dimension::Length(10.0));
        assert_eq!(Dimension::from(10), Dimension::Length(10.0));
    }

    #[test]
    fn test_edges_from() {
        assert_eq!(Edges::from(5.0), Edges::all(5.0));
        assert_eq!(Edges::from((2.0, 3.0)), Edges::xy(2.0, 3.0));
    }

    #[test]
    fn test_color_hex() {
        assert_eq!(Color::hex("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(Color::hex("00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(Color::hex("invalid"), None);
    }

    #[test]
    fn test_color_from_str() {
        assert_eq!(Color::from("red"), Color::Red);
        assert_eq!(Color::from("Blue"), Color::Blue);
    }

    #[test]
    fn test_const_style_constructors() {
        // Verify const styles are compile-time constants
        const _: Style = Style::new();
        const _: Style = Style::DEFAULT;
        const _: Style = Style::COLUMN;
        const _: Style = Style::ROW;
        const _: Style = Style::CENTERED;

        // Verify values are correct
        assert_eq!(Style::COLUMN.flex_direction, FlexDirection::Column);
        assert_eq!(Style::ROW.flex_direction, FlexDirection::Row);
        assert_eq!(Style::CENTERED.justify_content, JustifyContent::Center);
        assert_eq!(Style::CENTERED.align_items, AlignItems::Center);
    }

    #[test]
    fn test_const_edges() {
        // Verify const edges
        const _: Edges = Edges::ZERO;
        const _: Edges = Edges::all(1.0);

        assert_eq!(Edges::ZERO.top, 0.0);
        assert_eq!(Edges::all(5.0).top, 5.0);
    }

    #[test]
    fn test_text_style_merge_colors() {
        let base = TextStyle::new().color(Color::White);
        let overlay = TextStyle::new().bg(Color::Blue);

        let merged = base.merge(&overlay);
        assert_eq!(merged.color, Some(Color::White)); // base color preserved
        assert_eq!(merged.background_color, Some(Color::Blue)); // overlay bg added
    }

    #[test]
    fn test_text_style_merge_color_override() {
        let base = TextStyle::new().color(Color::White);
        let overlay = TextStyle::new().color(Color::Red);

        let merged = base.merge(&overlay);
        assert_eq!(merged.color, Some(Color::Red)); // overlay takes precedence
    }

    #[test]
    fn test_text_style_merge_attributes() {
        let base = TextStyle::new().bold();
        let overlay = TextStyle::new().italic();

        let merged = base.merge(&overlay);
        assert!(merged.bold); // base attribute preserved
        assert!(merged.italic); // overlay attribute added
    }

    #[test]
    fn test_text_style_merge_all_attributes() {
        let base = TextStyle::new().bold().underline();
        let overlay = TextStyle::new().italic().dim().strikethrough().inverse();

        let merged = base.merge(&overlay);
        assert!(merged.bold);
        assert!(merged.italic);
        assert!(merged.underline);
        assert!(merged.dim);
        assert!(merged.strikethrough);
        assert!(merged.inverse);
    }

    #[test]
    fn test_text_style_patch_alias() {
        let base = TextStyle::new().color(Color::White);
        let overlay = TextStyle::new().bold();

        let merged = base.patch(&overlay);
        assert_eq!(merged.color, Some(Color::White));
        assert!(merged.bold);
    }

    #[test]
    fn test_text_style_builder_chain() {
        let style = TextStyle::new()
            .color(Color::Red)
            .bg(Color::Black)
            .bold()
            .italic()
            .underline()
            .strikethrough()
            .dim()
            .inverse()
            .wrap(TextWrap::NoWrap);

        assert_eq!(style.color, Some(Color::Red));
        assert_eq!(style.background_color, Some(Color::Black));
        assert!(style.bold);
        assert!(style.italic);
        assert!(style.underline);
        assert!(style.strikethrough);
        assert!(style.dim);
        assert!(style.inverse);
        assert_eq!(style.wrap, TextWrap::NoWrap);
    }

    #[test]
    fn test_text_style_wrap_merge() {
        let base = TextStyle::new().wrap(TextWrap::Truncate);
        let overlay = TextStyle::default(); // default wrap

        // Default wrap in overlay shouldn't override
        let merged = base.merge(&overlay);
        assert_eq!(merged.wrap, TextWrap::Truncate);

        // Non-default wrap in overlay should override
        let overlay2 = TextStyle::new().wrap(TextWrap::NoWrap);
        let merged2 = base.merge(&overlay2);
        assert_eq!(merged2.wrap, TextWrap::NoWrap);
    }

    // === Line Tests ===

    #[test]
    fn test_line_new() {
        let line = Line::new();
        assert!(line.is_empty());
        assert_eq!(line.display_width(), 0);
    }

    #[test]
    fn test_line_text() {
        let line = Line::text("Hello");
        assert_eq!(line.as_str(), "Hello");
        assert_eq!(line.display_width(), 5);
        assert!(!line.is_empty());
    }

    #[test]
    fn test_line_from_span() {
        let span = StyledSpan::new("Test").bold();
        let line = Line::from_span(span);
        assert_eq!(line.as_str(), "Test");
        assert_eq!(line.get_spans().len(), 1);
        assert!(line.get_spans()[0].bold);
    }

    #[test]
    fn test_line_builder() {
        let line = Line::new()
            .span(StyledSpan::new("Hello ").bold())
            .span("World");
        assert_eq!(line.as_str(), "Hello World");
        assert_eq!(line.get_spans().len(), 2);
    }

    #[test]
    fn test_line_display_width_unicode() {
        // CJK characters are width 2
        let line = Line::text("日本語");
        assert_eq!(line.display_width(), 6);

        // Mixed content
        let line2 = Line::text("Hi日本");
        assert_eq!(line2.display_width(), 6); // 2 + 2 + 2
    }

    #[test]
    fn test_line_truncate() {
        let line = Line::text("Hello, World!");

        // Truncate with ellipsis
        let truncated = line.truncate(8, Some("…"));
        assert!(truncated.display_width() <= 8);
        assert!(truncated.as_str().ends_with('…'));

        // Truncate without ellipsis
        let truncated_no_ellipsis = line.truncate(5, None);
        assert_eq!(truncated_no_ellipsis.display_width(), 5);
        assert_eq!(truncated_no_ellipsis.as_str(), "Hello");

        // No truncation needed
        let no_truncate = line.truncate(20, Some("…"));
        assert_eq!(no_truncate.as_str(), "Hello, World!");
    }

    #[test]
    fn test_line_truncate_preserves_style() {
        let line = Line::new()
            .span(StyledSpan::new("Error: ").color(Color::Red).bold())
            .span("file not found");

        let truncated = line.truncate(10, Some("…"));
        let spans = truncated.get_spans();

        // First span should preserve its style
        assert!(spans[0].bold);
        assert_eq!(spans[0].color, Some(Color::Red));
    }

    #[test]
    fn test_line_pad() {
        let line = Line::text("Hi");
        let padded = line.pad(10);
        assert_eq!(padded.display_width(), 10);
        assert_eq!(padded.as_str(), "Hi        ");

        // No padding needed
        let no_pad = line.pad(1);
        assert_eq!(no_pad.as_str(), "Hi");
    }

    #[test]
    fn test_line_wrap() {
        let line = Line::text("The quick brown fox");
        let wrapped = line.wrap(10);

        // All lines should fit within max width
        for wrapped_line in &wrapped {
            assert!(wrapped_line.display_width() <= 10);
        }
    }

    #[test]
    fn test_line_wrap_preserves_style() {
        let line = Line::from_span(StyledSpan::new("Hello World").bold());
        let wrapped = line.wrap(6);

        // All wrapped lines should preserve the bold style
        for wrapped_line in &wrapped {
            for span in wrapped_line.get_spans() {
                assert!(span.bold);
            }
        }
    }

    #[test]
    fn test_line_style() {
        let style = TextStyle::new().bg(Color::Blue);
        let line = Line::text("Highlighted").style(style.clone());

        assert_eq!(line.get_style(), Some(&style));
    }

    #[test]
    fn test_line_from_str() {
        let line: Line = "Test".into();
        assert_eq!(line.as_str(), "Test");
    }

    #[test]
    fn test_line_from_string() {
        let line: Line = String::from("Test").into();
        assert_eq!(line.as_str(), "Test");
    }

    #[test]
    fn test_line_from_iterator() {
        let spans = vec![StyledSpan::new("Hello "), StyledSpan::new("World")];
        let line: Line = spans.into_iter().collect();
        assert_eq!(line.as_str(), "Hello World");
    }

    #[test]
    fn test_line_into_spans() {
        let line = Line::new().span("Hello ").span("World");
        let spans = line.into_spans();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text.as_ref(), "Hello ");
        assert_eq!(spans[1].text.as_ref(), "World");
    }

    // === Style Accumulator (D2) Tests ===

    #[test]
    fn test_text_style_apply_basic() {
        let style = TextStyle::new().color(Color::Red).bold();
        let span = style.apply("Error");

        assert_eq!(span.text.as_ref(), "Error");
        assert_eq!(span.color, Some(Color::Red));
        assert!(span.bold);
    }

    #[test]
    fn test_text_style_apply_all_attributes() {
        let style = TextStyle::new()
            .color(Color::Red)
            .bg(Color::Black)
            .bold()
            .italic()
            .underline()
            .strikethrough()
            .dim()
            .inverse();

        let span = style.apply("styled text");

        assert_eq!(span.text.as_ref(), "styled text");
        assert_eq!(span.color, Some(Color::Red));
        assert_eq!(span.background_color, Some(Color::Black));
        assert!(span.bold);
        assert!(span.italic);
        assert!(span.underline);
        assert!(span.strikethrough);
        assert!(span.dim);
        assert!(span.inverse);
    }

    #[test]
    fn test_text_style_apply_reusable() {
        let error_style = TextStyle::new().color(Color::Red).bold();

        // Apply same style to multiple texts
        let span1 = error_style.apply("Error 1");
        let span2 = error_style.apply("Error 2");

        assert_eq!(span1.text.as_ref(), "Error 1");
        assert_eq!(span2.text.as_ref(), "Error 2");
        assert_eq!(span1.color, span2.color);
        assert_eq!(span1.bold, span2.bold);
    }

    #[test]
    fn test_text_style_with_text_basic() {
        let span = TextStyle::new()
            .color(Color::Green)
            .bold()
            .with_text("Success");

        assert_eq!(span.text.as_ref(), "Success");
        assert_eq!(span.color, Some(Color::Green));
        assert!(span.bold);
    }

    #[test]
    fn test_text_style_apply_from_string() {
        let style = TextStyle::new().italic();
        let span = style.apply(String::from("owned text"));

        assert_eq!(span.text.as_ref(), "owned text");
        assert!(span.italic);
    }

    #[test]
    fn test_text_style_apply_in_line_builder() {
        let bold = TextStyle::new().bold();
        let dim = TextStyle::new().dim();

        let line = Line::new()
            .span(bold.apply("Header: "))
            .span(dim.apply("details"));

        assert_eq!(line.as_str(), "Header: details");
        assert_eq!(line.get_spans().len(), 2);
        assert!(line.get_spans()[0].bold);
        assert!(line.get_spans()[1].dim);
    }

    #[test]
    fn test_text_style_apply_default_style() {
        let style = TextStyle::new();
        let span = style.apply("plain");

        assert_eq!(span.text.as_ref(), "plain");
        assert_eq!(span.color, None);
        assert_eq!(span.background_color, None);
        assert!(!span.bold);
        assert!(!span.italic);
    }
}
