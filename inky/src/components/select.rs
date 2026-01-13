//! Select component - selection list.

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{BorderStyle, Color, FlexDirection};

/// A selectable option in the list.
#[derive(Debug, Clone)]
pub struct SelectOption {
    /// Display label.
    pub label: String,
    /// Optional value (defaults to label if not set).
    pub value: Option<String>,
    /// Whether this option is disabled.
    pub disabled: bool,
}

impl SelectOption {
    /// Create a new option with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: None,
            disabled: false,
        }
    }

    /// Set the value (different from display label).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Mark this option as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Get the value (or label if no value set).
    pub fn get_value(&self) -> &str {
        self.value.as_deref().unwrap_or(&self.label)
    }
}

impl<S: Into<String>> From<S> for SelectOption {
    fn from(s: S) -> Self {
        SelectOption::new(s)
    }
}

/// Selection list component.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let select = Select::new()
///     .options(vec!["Option 1", "Option 2", "Option 3"])
///     .selected(0);
/// ```
#[derive(Debug, Clone)]
pub struct Select {
    /// Available options.
    options: Vec<SelectOption>,
    /// Currently selected index.
    selected: usize,
    /// Whether the select is focused.
    focused: bool,
    /// Border style.
    border: BorderStyle,
    /// Normal text color.
    color: Option<Color>,
    /// Selected item color.
    selected_color: Color,
    /// Disabled item color.
    disabled_color: Color,
    /// Focus indicator color.
    focus_color: Color,
    /// Indicator for selected item.
    indicator: String,
    /// Indicator for unselected items.
    unselected_indicator: String,
    /// Maximum visible items (for scrolling).
    max_visible: Option<usize>,
    /// Scroll offset.
    scroll_offset: usize,
}

impl Select {
    /// Create a new empty select.
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            selected: 0,
            focused: false,
            border: BorderStyle::None,
            color: None,
            selected_color: Color::BrightCyan,
            disabled_color: Color::BrightBlack,
            focus_color: Color::BrightCyan,
            indicator: "❯ ".to_string(),
            unselected_indicator: "  ".to_string(),
            max_visible: None,
            scroll_offset: 0,
        }
    }

    /// Set options from an iterator of items.
    pub fn options<I, T>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<SelectOption>,
    {
        self.options = options.into_iter().map(Into::into).collect();
        self
    }

    /// Add a single option.
    pub fn option(mut self, option: impl Into<SelectOption>) -> Self {
        self.options.push(option.into());
        self
    }

    /// Set the selected index.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index.min(self.options.len().saturating_sub(1));
        self
    }

    /// Set whether the select is focused.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set selected item color.
    pub fn selected_color(mut self, color: Color) -> Self {
        self.selected_color = color;
        self
    }

    /// Set disabled item color.
    pub fn disabled_color(mut self, color: Color) -> Self {
        self.disabled_color = color;
        self
    }

    /// Set focus indicator color.
    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = color;
        self
    }

    /// Set the indicator shown before selected item.
    pub fn indicator(mut self, indicator: impl Into<String>) -> Self {
        self.indicator = indicator.into();
        self
    }

    /// Set the indicator shown before unselected items.
    pub fn unselected_indicator(mut self, indicator: impl Into<String>) -> Self {
        self.unselected_indicator = indicator.into();
        self
    }

    /// Set maximum visible items (enables scrolling).
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = Some(max);
        self
    }

    /// Get the currently selected index.
    pub fn get_selected(&self) -> usize {
        self.selected
    }

    /// Get the selected option.
    pub fn get_selected_option(&self) -> Option<&SelectOption> {
        self.options.get(self.selected)
    }

    /// Get the selected value.
    pub fn get_selected_value(&self) -> Option<&str> {
        self.get_selected_option().map(|o| o.get_value())
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.options.is_empty() {
            return;
        }

        // Find previous non-disabled option
        let mut idx = self.selected;
        loop {
            idx = if idx == 0 {
                self.options.len() - 1
            } else {
                idx - 1
            };

            if idx == self.selected || !self.options[idx].disabled {
                break;
            }
        }
        self.selected = idx;
        self.ensure_visible();
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.options.is_empty() {
            return;
        }

        // Find next non-disabled option
        let mut idx = self.selected;
        loop {
            idx = (idx + 1) % self.options.len();

            if idx == self.selected || !self.options[idx].disabled {
                break;
            }
        }
        self.selected = idx;
        self.ensure_visible();
    }

    /// Move to first option.
    pub fn select_first(&mut self) {
        for (i, opt) in self.options.iter().enumerate() {
            if !opt.disabled {
                self.selected = i;
                self.ensure_visible();
                break;
            }
        }
    }

    /// Move to last option.
    pub fn select_last(&mut self) {
        for (i, opt) in self.options.iter().enumerate().rev() {
            if !opt.disabled {
                self.selected = i;
                self.ensure_visible();
                break;
            }
        }
    }

    /// Ensure selected item is visible in scroll view.
    fn ensure_visible(&mut self) {
        if let Some(max) = self.max_visible {
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            } else if self.selected >= self.scroll_offset + max {
                self.scroll_offset = self.selected - max + 1;
            }
        }
    }

    /// Get the number of options.
    pub fn len(&self) -> usize {
        self.options.len()
    }

    /// Check if there are no options.
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Select> for Node {
    fn from(select: Select) -> Self {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(select.border);

        // Determine visible range
        let start = select.scroll_offset;
        let end = if let Some(max) = select.max_visible {
            (start + max).min(select.options.len())
        } else {
            select.options.len()
        };

        // Create option rows
        for (i, opt) in select
            .options
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            let is_selected = i == select.selected;

            // Build indicator
            let indicator = if is_selected {
                &select.indicator
            } else {
                &select.unselected_indicator
            };

            // Avoid format! allocation by building string directly
            let mut display = String::with_capacity(indicator.len() + opt.label.len());
            display.push_str(indicator);
            display.push_str(&opt.label);

            // Create text node with appropriate color
            let mut text = TextNode::new(display);

            if opt.disabled {
                text = text.color(select.disabled_color).dim();
            } else if is_selected {
                text = text.color(select.selected_color);
                if select.focused {
                    text = text.bold();
                }
            } else if let Some(color) = select.color {
                text = text.color(color);
            }

            container = container.child(text);
        }

        // Add scroll indicators if needed
        if let Some(_max) = select.max_visible {
            if start > 0 {
                // Could add "▲ more" indicator
            }
            if end < select.options.len() {
                // Could add "▼ more" indicator
            }
        }

        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_select_new() {
        let select = Select::new();
        assert!(select.is_empty());
        assert_eq!(select.get_selected(), 0);
    }

    #[test]
    fn test_select_options() {
        let select = Select::new()
            .options(vec!["One", "Two", "Three"])
            .selected(1);

        assert_eq!(select.len(), 3);
        assert_eq!(select.get_selected(), 1);
        assert_eq!(select.get_selected_value(), Some("Two"));
    }

    #[test]
    fn test_select_navigation() {
        let mut select = Select::new()
            .options(vec!["One", "Two", "Three"])
            .selected(0);

        select.select_next();
        assert_eq!(select.get_selected(), 1);

        select.select_next();
        assert_eq!(select.get_selected(), 2);

        select.select_next();
        assert_eq!(select.get_selected(), 0); // Wraps around

        select.select_prev();
        assert_eq!(select.get_selected(), 2); // Wraps around
    }

    #[test]
    fn test_select_disabled_skip() {
        let mut select = Select::new()
            .option(SelectOption::new("One"))
            .option(SelectOption::new("Two").disabled())
            .option(SelectOption::new("Three"))
            .selected(0);

        select.select_next();
        assert_eq!(select.get_selected(), 2); // Skips disabled

        select.select_prev();
        assert_eq!(select.get_selected(), 0); // Skips disabled
    }

    #[test]
    fn test_select_option_value() {
        let opt = SelectOption::new("Display").value("actual_value");

        assert_eq!(opt.label, "Display");
        assert_eq!(opt.get_value(), "actual_value");
    }

    #[test]
    fn test_select_to_node() {
        let select = Select::new()
            .options(vec!["A", "B", "C"])
            .border(BorderStyle::Single);

        let node: Node = select.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_select_first_last() {
        let mut select = Select::new()
            .option(SelectOption::new("Disabled").disabled())
            .option(SelectOption::new("First enabled"))
            .option(SelectOption::new("Middle"))
            .option(SelectOption::new("Last enabled"))
            .option(SelectOption::new("Also disabled").disabled())
            .selected(2);

        select.select_first();
        assert_eq!(select.get_selected(), 1);

        select.select_last();
        assert_eq!(select.get_selected(), 3);
    }
}
