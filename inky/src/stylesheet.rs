//! CSS-like named styles with cascading inheritance.
//!
//! This module provides a [`StyleSheet`] type that allows you to define named
//! styles and extend them with overrides, similar to CSS classes.
//!
//! # Example
//!
//! ```rust
//! use inky::stylesheet::StyleSheet;
//! use inky::style::{Style, Edges};
//!
//! let mut sheet = StyleSheet::new();
//!
//! // Define base styles
//! let base = Style { padding: Edges::all(1.0), ..Default::default() };
//! sheet.define("base", base);
//!
//! // Look up styles
//! if let Some(style) = sheet.get("base") {
//!     println!("Padding: {:?}", style.padding);
//! }
//! ```
//!
//! # Cascading
//!
//! When you extend a style, the child inherits all properties from the parent
//! and can override specific ones:
//!
//! ```rust
//! use inky::stylesheet::StyleSheet;
//! use inky::style::{Style, Edges};
//!
//! let mut sheet = StyleSheet::new();
//!
//! let button = Style { padding: Edges::all(1.0), ..Default::default() };
//! sheet.define("button", button);
//!
//! let primary = Style { margin: Edges::all(2.0), ..Default::default() };
//! sheet.extend("primary-button", "button", primary);
//!
//! // "primary-button" has padding from "button" plus its own margin
//! ```
//!
//! [`StyleSheet`]: crate::stylesheet::StyleSheet

use crate::style::{Edges, Style};
use rustc_hash::FxHashMap;

/// A collection of named styles with inheritance support.
///
/// StyleSheet allows you to:
/// - Define named styles for reuse across your UI
/// - Extend existing styles with overrides (cascading)
/// - Look up styles by name
///
/// # Example
///
/// ```rust
/// use inky::stylesheet::StyleSheet;
/// use inky::style::{Style, Edges};
///
/// let mut sheet = StyleSheet::new();
///
/// // Define a base style
/// let card = Style { padding: Edges::all(2.0), ..Default::default() };
/// sheet.define("card", card);
///
/// // Extend it for variants
/// sheet.extend("card-primary", "card", Style::default());
/// ```
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    /// Named styles.
    styles: FxHashMap<String, Style>,
    /// Parent relationships for cascading.
    parents: FxHashMap<String, String>,
}

impl StyleSheet {
    /// Create a new empty stylesheet.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::stylesheet::StyleSheet;
    ///
    /// let sheet = StyleSheet::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a named style.
    ///
    /// If a style with this name already exists, it will be replaced.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::stylesheet::StyleSheet;
    /// use inky::style::{Style, Edges};
    ///
    /// let mut sheet = StyleSheet::new();
    /// let header = Style { padding: Edges::all(2.0), ..Default::default() };
    /// sheet.define("header", header);
    /// ```
    pub fn define(&mut self, name: impl Into<String>, style: Style) -> &mut Self {
        let name = name.into();
        // Remove any parent relationship for a fresh definition
        self.parents.remove(&name);
        self.styles.insert(name, style);
        self
    }

    /// Extend an existing style with overrides.
    ///
    /// The new style inherits all properties from the base and can override
    /// specific ones via the `overrides` style.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::stylesheet::StyleSheet;
    /// use inky::style::{Style, Edges};
    ///
    /// let mut sheet = StyleSheet::new();
    /// let base = Style { padding: Edges::all(1.0), ..Default::default() };
    /// sheet.define("base", base);
    ///
    /// let derived = Style { margin: Edges::all(2.0), ..Default::default() };
    /// sheet.extend("derived", "base", derived);
    /// ```
    ///
    /// # Notes
    ///
    /// If the base style doesn't exist, the derived style will just use
    /// the overrides without any inherited properties.
    pub fn extend(
        &mut self,
        name: impl Into<String>,
        base: impl Into<String>,
        overrides: Style,
    ) -> &mut Self {
        let name = name.into();
        let base = base.into();

        // Store the parent relationship
        self.parents.insert(name.clone(), base);

        // Store the overrides
        self.styles.insert(name, overrides);

        self
    }

    /// Get a style by name, with cascading inheritance resolved.
    ///
    /// If the style extends another, the returned style will have all
    /// parent properties merged with the child's overrides.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::stylesheet::StyleSheet;
    /// use inky::style::{Style, Edges};
    ///
    /// let mut sheet = StyleSheet::new();
    /// let base = Style { padding: Edges::all(1.0), ..Default::default() };
    /// sheet.define("base", base);
    ///
    /// if let Some(style) = sheet.get("base") {
    ///     // Use the style...
    /// }
    /// ```
    pub fn get(&self, name: &str) -> Option<Style> {
        // Start with the named style
        let style = self.styles.get(name)?;

        // Check if it has a parent
        if let Some(parent_name) = self.parents.get(name) {
            // Recursively get parent (handles multi-level inheritance)
            if let Some(parent_style) = self.get(parent_name) {
                // Merge: start with parent, overlay child
                return Some(merge_styles(&parent_style, style));
            }
        }

        Some(style.clone())
    }

    /// Check if a style exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::stylesheet::StyleSheet;
    /// use inky::style::Style;
    ///
    /// let mut sheet = StyleSheet::new();
    /// sheet.define("header", Style::default());
    ///
    /// assert!(sheet.contains("header"));
    /// assert!(!sheet.contains("footer"));
    /// ```
    pub fn contains(&self, name: &str) -> bool {
        self.styles.contains_key(name)
    }

    /// Remove a style by name.
    ///
    /// Returns the removed style if it existed.
    pub fn remove(&mut self, name: &str) -> Option<Style> {
        self.parents.remove(name);
        self.styles.remove(name)
    }

    /// Get the number of defined styles.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Check if the stylesheet is empty.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Iterate over all style names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.styles.keys().map(|s| s.as_str())
    }

    /// Clear all styles.
    pub fn clear(&mut self) {
        self.styles.clear();
        self.parents.clear();
    }
}

/// Merge two styles, with child properties overriding parent.
///
/// For Option fields, if the child has Some, use it; otherwise use parent.
/// For primitive fields that have meaningful defaults, child takes precedence
/// if it differs from the default.
fn merge_styles(parent: &Style, child: &Style) -> Style {
    let mut result = parent.clone();

    // Merge display if child specifies it
    if child.display != crate::style::Display::default() {
        result.display = child.display;
    }

    // Merge flex properties
    if child.flex_direction != crate::style::FlexDirection::default() {
        result.flex_direction = child.flex_direction;
    }
    if child.flex_wrap != crate::style::FlexWrap::default() {
        result.flex_wrap = child.flex_wrap;
    }
    if child.justify_content != crate::style::JustifyContent::default() {
        result.justify_content = child.justify_content;
    }
    if child.align_items != crate::style::AlignItems::default() {
        result.align_items = child.align_items;
    }
    if child.align_content != crate::style::AlignContent::default() {
        result.align_content = child.align_content;
    }
    if child.align_self != crate::style::AlignSelf::default() {
        result.align_self = child.align_self;
    }

    // Merge gap
    if child.gap != 0.0 {
        result.gap = child.gap;
    }

    // Merge flex values
    if child.flex_grow != 0.0 {
        result.flex_grow = child.flex_grow;
    }
    if child.flex_shrink != 1.0 {
        result.flex_shrink = child.flex_shrink;
    }

    // Merge dimension fields
    if child.width != crate::style::Dimension::Auto {
        result.width = child.width;
    }
    if child.height != crate::style::Dimension::Auto {
        result.height = child.height;
    }
    if child.min_width != crate::style::Dimension::Auto {
        result.min_width = child.min_width;
    }
    if child.min_height != crate::style::Dimension::Auto {
        result.min_height = child.min_height;
    }
    if child.max_width != crate::style::Dimension::Auto {
        result.max_width = child.max_width;
    }
    if child.max_height != crate::style::Dimension::Auto {
        result.max_height = child.max_height;
    }

    // Merge edges (padding, margin)
    result.padding = merge_edges(&parent.padding, &child.padding);
    result.margin = merge_edges(&parent.margin, &child.margin);

    // Merge border if set
    if child.border != crate::style::BorderStyle::None {
        result.border = child.border;
    }

    // Merge background color if set
    if child.background_color.is_some() {
        result.background_color = child.background_color;
    }

    // Merge overflow
    if child.overflow != crate::style::Overflow::default() {
        result.overflow = child.overflow;
    }

    result
}

/// Merge edge values, preferring child non-zero values.
fn merge_edges(parent: &Edges, child: &Edges) -> Edges {
    Edges {
        top: if child.top != 0.0 {
            child.top
        } else {
            parent.top
        },
        right: if child.right != 0.0 {
            child.right
        } else {
            parent.right
        },
        bottom: if child.bottom != 0.0 {
            child.bottom
        } else {
            parent.bottom
        },
        left: if child.left != 0.0 {
            child.left
        } else {
            parent.left
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_define_and_get() {
        let mut sheet = StyleSheet::new();
        let style = Style {
            padding: Edges::all(5.0),
            ..Default::default()
        };
        sheet.define("test", style);

        let result = sheet.get("test").unwrap();
        assert_eq!(result.padding.top, 5.0);
        assert_eq!(result.padding.left, 5.0);
    }

    #[test]
    fn test_contains() {
        let mut sheet = StyleSheet::new();
        sheet.define("exists", Style::default());

        assert!(sheet.contains("exists"));
        assert!(!sheet.contains("missing"));
    }

    #[test]
    fn test_extend_basic() {
        let mut sheet = StyleSheet::new();
        let parent = Style {
            padding: Edges::all(10.0),
            ..Default::default()
        };
        sheet.define("parent", parent);

        let child_overrides = Style {
            margin: Edges::all(5.0),
            ..Default::default()
        };
        sheet.extend("child", "parent", child_overrides);

        let child = sheet.get("child").unwrap();
        // Should inherit padding from parent
        assert_eq!(child.padding.top, 10.0);
        // Should have its own margin
        assert_eq!(child.margin.top, 5.0);
    }

    #[test]
    fn test_extend_override() {
        let mut sheet = StyleSheet::new();
        let parent = Style {
            padding: Edges::all(10.0),
            margin: Edges::all(20.0),
            ..Default::default()
        };
        sheet.define("parent", parent);

        let child_overrides = Style {
            padding: Edges::all(5.0),
            ..Default::default()
        };
        sheet.extend("child", "parent", child_overrides);

        let child = sheet.get("child").unwrap();
        // Padding should be overridden
        assert_eq!(child.padding.top, 5.0);
        // Margin should be inherited
        assert_eq!(child.margin.top, 20.0);
    }

    #[test]
    fn test_multi_level_inheritance() {
        let mut sheet = StyleSheet::new();
        let grandparent = Style {
            padding: Edges::all(1.0),
            ..Default::default()
        };
        sheet.define("grandparent", grandparent);

        let parent_overrides = Style {
            margin: Edges::all(2.0),
            ..Default::default()
        };
        sheet.extend("parent", "grandparent", parent_overrides);
        sheet.extend("child", "parent", Style::default());

        let child = sheet.get("child").unwrap();
        // Should inherit padding from grandparent through parent
        assert_eq!(child.padding.top, 1.0);
        // Should inherit margin from parent
        assert_eq!(child.margin.top, 2.0);
    }

    #[test]
    fn test_missing_parent() {
        let mut sheet = StyleSheet::new();
        // Extend from non-existent parent
        let orphan = Style {
            padding: Edges::all(5.0),
            ..Default::default()
        };
        sheet.extend("orphan", "missing", orphan);

        // Should still work, just without parent properties
        let style = sheet.get("orphan").unwrap();
        assert_eq!(style.padding.top, 5.0);
    }

    #[test]
    fn test_remove() {
        let mut sheet = StyleSheet::new();
        sheet.define("removeme", Style::default());

        assert!(sheet.contains("removeme"));
        sheet.remove("removeme");
        assert!(!sheet.contains("removeme"));
    }

    #[test]
    fn test_len_and_empty() {
        let mut sheet = StyleSheet::new();
        assert!(sheet.is_empty());
        assert_eq!(sheet.len(), 0);

        sheet.define("one", Style::default());
        assert!(!sheet.is_empty());
        assert_eq!(sheet.len(), 1);

        sheet.define("two", Style::default());
        assert_eq!(sheet.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut sheet = StyleSheet::new();
        sheet.define("one", Style::default());
        sheet.define("two", Style::default());
        sheet.extend("three", "one", Style::default());

        sheet.clear();

        assert!(sheet.is_empty());
        assert!(!sheet.contains("one"));
    }

    #[test]
    fn test_names_iterator() {
        let mut sheet = StyleSheet::new();
        sheet.define("alpha", Style::default());
        sheet.define("beta", Style::default());

        let names: Vec<_> = sheet.names().collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_chained_definition() {
        let mut sheet = StyleSheet::new();
        sheet
            .define("one", Style::default())
            .define("two", Style::default())
            .extend("three", "one", Style::default());

        assert_eq!(sheet.len(), 3);
    }
}
