//! Input component - text input field with undo, completions, and history.
//!
//! # Basic Usage
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! let input = Input::new()
//!     .value("Hello")
//!     .placeholder("Enter text...")
//!     .width(30);
//! ```
//!
//! # With Undo Stack
//!
//! ```ignore
//! use inky::components::Input;
//!
//! let mut input = Input::new().enable_undo();
//! input.insert('a');
//! input.insert('b');
//! input.commit_undo(); // Save state
//! input.insert('c');
//! input.undo(); // Restores "ab"
//! ```
//!
//! # With Token Estimate
//!
//! ```ignore
//! use inky::components::Input;
//!
//! let input = Input::new()
//!     .value("Hello world")
//!     .token_estimate(3); // Display "~3 tokens"
//! ```
//!
//! # With Completions
//!
//! ```ignore
//! use inky::components::Input;
//!
//! let input = Input::new()
//!     .value("/he")
//!     .completions(vec!["/help", "/history", "/clear"])
//!     .completion_selected(0);
//! ```

use std::borrow::Cow;
use std::time::Instant;

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{BorderStyle, Color, FlexDirection};

/// Maximum undo history entries.
const MAX_UNDO_HISTORY: usize = 100;

/// Time threshold for coalescing rapid typing into single undo units (ms).
const UNDO_COALESCE_MS: u128 = 500;

/// An entry in the undo history.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// The text value.
    pub value: String,
    /// Cursor position.
    pub cursor: usize,
}

/// Text input field component with optional undo, completions, and history.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let input = Input::new()
///     .value("Hello")
///     .placeholder("Enter text...")
///     .width(30);
/// ```
#[derive(Debug, Clone)]
pub struct Input {
    /// Current value.
    value: String,
    /// Placeholder text shown when empty.
    placeholder: Option<String>,
    /// Whether the input is focused.
    focused: bool,
    /// Cursor position (character index).
    cursor: usize,
    /// Width of the input field.
    width: Option<u16>,
    /// Border style.
    border: BorderStyle,
    /// Text color.
    color: Option<Color>,
    /// Placeholder color.
    placeholder_color: Color,
    /// Focus color (border/cursor when focused).
    focus_color: Color,
    /// Whether to mask input (password mode).
    masked: bool,
    /// Mask character for password mode.
    mask_char: char,

    // Undo support
    /// Whether undo is enabled.
    undo_enabled: bool,
    /// Undo history stack.
    undo_stack: Vec<UndoEntry>,
    /// Redo history stack.
    redo_stack: Vec<UndoEntry>,
    /// Last edit time for coalescing.
    last_edit_time: Option<Instant>,

    // Token estimate support
    /// Token estimate to display.
    token_estimate: Option<usize>,
    /// Color for token estimate.
    token_color: Color,

    // Completion support
    /// Available completions.
    completions: Vec<String>,
    /// Whether completions popup is visible.
    completions_visible: bool,
    /// Currently selected completion index.
    completion_selected: usize,
    /// Maximum completions to show in popup.
    completion_max_visible: usize,

    // History support
    /// History entries.
    history: Vec<String>,
    /// Whether history search is active.
    history_search_active: bool,
    /// History search query.
    history_search_query: String,
    /// Filtered history matches.
    history_matches: Vec<usize>,
    /// Selected history match index.
    history_selected: usize,
}

impl Input {
    /// Create a new empty input field.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: None,
            focused: false,
            cursor: 0,
            width: None,
            border: BorderStyle::Single,
            color: None,
            placeholder_color: Color::BrightBlack,
            focus_color: Color::BrightCyan,
            masked: false,
            mask_char: '•',

            // Undo support
            undo_enabled: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: None,

            // Token estimate
            token_estimate: None,
            token_color: Color::BrightBlack,

            // Completions
            completions: Vec::new(),
            completions_visible: false,
            completion_selected: 0,
            completion_max_visible: 5,

            // History
            history: Vec::new(),
            history_search_active: false,
            history_search_query: String::new(),
            history_matches: Vec::new(),
            history_selected: 0,
        }
    }

    /// Set the current value.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor = self.value.len();
        self
    }

    /// Set placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set whether the input is focused.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set cursor position.
    pub fn cursor(mut self, pos: usize) -> Self {
        self.cursor = pos.min(self.value.len());
        self
    }

    /// Set the width of the input field.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Remove border.
    pub fn no_border(mut self) -> Self {
        self.border = BorderStyle::None;
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set placeholder color.
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = color;
        self
    }

    /// Set focus color (border when focused).
    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = color;
        self
    }

    /// Enable password mode (mask characters).
    pub fn password(mut self) -> Self {
        self.masked = true;
        self
    }

    /// Set mask character for password mode.
    pub fn mask_char(mut self, c: char) -> Self {
        self.mask_char = c;
        self
    }

    /// Get the current value.
    pub fn get_value(&self) -> &str {
        &self.value
    }

    /// Get cursor position.
    pub fn get_cursor(&self) -> usize {
        self.cursor
    }

    /// Insert a character at cursor position.
    pub fn insert(&mut self, c: char) {
        if self.cursor <= self.value.len() {
            self.value.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Delete character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.value.remove(self.cursor);
        }
    }

    /// Delete character at cursor (delete).
    pub fn delete(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn move_end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Clear the input.
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Get the display text (masked if password mode).
    fn display_text(&self) -> Cow<'_, str> {
        if self.masked {
            Cow::Owned(self.mask_char.to_string().repeat(self.value.len()))
        } else {
            Cow::Borrowed(&self.value)
        }
    }

    // ========== Undo Support ==========

    /// Enable undo support.
    ///
    /// When enabled, edits can be undone with `undo()` and redone with `redo()`.
    /// Rapid typing is coalesced into single undo units.
    pub fn enable_undo(mut self) -> Self {
        self.undo_enabled = true;
        self
    }

    /// Check if undo is enabled.
    pub fn is_undo_enabled(&self) -> bool {
        self.undo_enabled
    }

    /// Manually commit the current state to the undo stack.
    ///
    /// This is useful for creating explicit undo points, e.g., after
    /// completing a word or pressing Enter.
    pub fn commit_undo(&mut self) {
        if !self.undo_enabled {
            return;
        }
        self.push_undo_state();
        self.last_edit_time = None; // Reset coalescing
    }

    /// Push current state to undo stack.
    fn push_undo_state(&mut self) {
        self.undo_stack.push(UndoEntry {
            value: self.value.clone(),
            cursor: self.cursor,
        });
        // Limit stack size
        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        // Clear redo stack on new edit
        self.redo_stack.clear();
    }

    /// Record an edit for undo (with coalescing for rapid typing).
    fn record_edit(&mut self) {
        if !self.undo_enabled {
            return;
        }

        let now = Instant::now();
        let should_coalesce = self
            .last_edit_time
            .map(|t| now.duration_since(t).as_millis() < UNDO_COALESCE_MS)
            .unwrap_or(false);

        if !should_coalesce {
            self.push_undo_state();
        }

        self.last_edit_time = Some(now);
    }

    /// Undo the last edit.
    ///
    /// Returns `true` if an undo was performed.
    pub fn undo(&mut self) -> bool {
        if !self.undo_enabled {
            return false;
        }

        if let Some(entry) = self.undo_stack.pop() {
            // Save current state to redo stack
            self.redo_stack.push(UndoEntry {
                value: self.value.clone(),
                cursor: self.cursor,
            });
            // Restore previous state
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.last_edit_time = None;
            true
        } else {
            false
        }
    }

    /// Redo a previously undone edit.
    ///
    /// Returns `true` if a redo was performed.
    pub fn redo(&mut self) -> bool {
        if !self.undo_enabled {
            return false;
        }

        if let Some(entry) = self.redo_stack.pop() {
            // Save current state to undo stack
            self.undo_stack.push(UndoEntry {
                value: self.value.clone(),
                cursor: self.cursor,
            });
            // Restore redo state
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.last_edit_time = None;
            true
        } else {
            false
        }
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        self.undo_enabled && !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        self.undo_enabled && !self.redo_stack.is_empty()
    }

    /// Get the undo stack depth.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    // ========== Token Estimate Support ==========

    /// Set the token estimate to display.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::Input;
    ///
    /// let input = Input::new()
    ///     .value("Hello world")
    ///     .token_estimate(3);
    /// ```
    pub fn token_estimate(mut self, tokens: usize) -> Self {
        self.token_estimate = Some(tokens);
        self
    }

    /// Clear the token estimate display.
    pub fn clear_token_estimate(mut self) -> Self {
        self.token_estimate = None;
        self
    }

    /// Set the color for the token estimate display.
    pub fn token_color(mut self, color: Color) -> Self {
        self.token_color = color;
        self
    }

    /// Get the current token estimate.
    pub fn get_token_estimate(&self) -> Option<usize> {
        self.token_estimate
    }

    // ========== Completion Support ==========

    /// Set the available completions.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::Input;
    ///
    /// let input = Input::new()
    ///     .completions(vec!["/help", "/history", "/clear"]);
    /// ```
    pub fn completions(mut self, completions: Vec<impl Into<String>>) -> Self {
        self.completions = completions.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Show or hide the completions popup.
    pub fn completions_visible(mut self, visible: bool) -> Self {
        self.completions_visible = visible;
        self
    }

    /// Set the selected completion index.
    pub fn completion_selected(mut self, index: usize) -> Self {
        self.completion_selected = index.min(self.completions.len().saturating_sub(1));
        self
    }

    /// Set maximum visible completions in the popup.
    pub fn completion_max_visible(mut self, max: usize) -> Self {
        self.completion_max_visible = max;
        self
    }

    /// Show the completions popup.
    pub fn show_completions(&mut self) {
        self.completions_visible = true;
        self.completion_selected = 0;
    }

    /// Hide the completions popup.
    pub fn hide_completions(&mut self) {
        self.completions_visible = false;
    }

    /// Toggle completions popup visibility.
    pub fn toggle_completions(&mut self) {
        self.completions_visible = !self.completions_visible;
        if self.completions_visible {
            self.completion_selected = 0;
        }
    }

    /// Select next completion.
    pub fn next_completion(&mut self) {
        if !self.completions.is_empty() {
            self.completion_selected = (self.completion_selected + 1) % self.completions.len();
        }
    }

    /// Select previous completion.
    pub fn prev_completion(&mut self) {
        if !self.completions.is_empty() {
            self.completion_selected = self
                .completion_selected
                .checked_sub(1)
                .unwrap_or(self.completions.len() - 1);
        }
    }

    /// Accept the selected completion.
    ///
    /// Returns `Some(completion)` if a completion was selected, `None` if no
    /// completions are visible or available.
    pub fn accept_completion(&mut self) -> Option<String> {
        if !self.completions_visible || self.completions.is_empty() {
            return None;
        }

        let completion = self.completions.get(self.completion_selected)?.clone();
        self.record_edit();
        self.value.clone_from(&completion);
        self.cursor = self.value.len();
        self.completions_visible = false;
        Some(completion)
    }

    /// Get filtered completions that match the current input.
    pub fn filtered_completions(&self) -> Vec<&str> {
        let prefix = self.value.to_lowercase();
        self.completions
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&prefix))
            .map(|s| s.as_str())
            .collect()
    }

    /// Check if completions popup is visible.
    pub fn is_completions_visible(&self) -> bool {
        self.completions_visible
    }

    /// Get the currently selected completion.
    pub fn get_selected_completion(&self) -> Option<&str> {
        self.completions
            .get(self.completion_selected)
            .map(|s| s.as_str())
    }

    // ========== History Support ==========

    /// Set the history entries.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::Input;
    ///
    /// let input = Input::new()
    ///     .history(vec!["previous command 1", "previous command 2"]);
    /// ```
    pub fn history(mut self, history: Vec<impl Into<String>>) -> Self {
        self.history = history.into_iter().map(|h| h.into()).collect();
        self
    }

    /// Start history search mode.
    pub fn start_history_search(&mut self) {
        self.history_search_active = true;
        self.history_search_query.clear();
        self.update_history_matches();
        self.history_selected = 0;
    }

    /// Stop history search mode.
    pub fn stop_history_search(&mut self) {
        self.history_search_active = false;
        self.history_search_query.clear();
        self.history_matches.clear();
    }

    /// Update the history search query.
    pub fn set_history_query(&mut self, query: &str) {
        self.history_search_query = query.to_string();
        self.update_history_matches();
        self.history_selected = 0;
    }

    /// Add a character to the history search query.
    pub fn history_search_insert(&mut self, c: char) {
        self.history_search_query.push(c);
        self.update_history_matches();
        self.history_selected = 0;
    }

    /// Remove a character from the history search query.
    pub fn history_search_backspace(&mut self) {
        self.history_search_query.pop();
        self.update_history_matches();
        self.history_selected = 0;
    }

    /// Update filtered history matches based on current query.
    fn update_history_matches(&mut self) {
        let query = self.history_search_query.to_lowercase();
        self.history_matches = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, h)| h.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
    }

    /// Select next history match.
    pub fn next_history_match(&mut self) {
        if !self.history_matches.is_empty() {
            self.history_selected = (self.history_selected + 1) % self.history_matches.len();
        }
    }

    /// Select previous history match.
    pub fn prev_history_match(&mut self) {
        if !self.history_matches.is_empty() {
            self.history_selected = self
                .history_selected
                .checked_sub(1)
                .unwrap_or(self.history_matches.len() - 1);
        }
    }

    /// Accept the selected history entry.
    ///
    /// Returns `Some(entry)` if a history entry was selected.
    pub fn accept_history(&mut self) -> Option<String> {
        if !self.history_search_active || self.history_matches.is_empty() {
            return None;
        }

        let idx = *self.history_matches.get(self.history_selected)?;
        let entry = self.history.get(idx)?.clone();
        self.record_edit();
        self.value.clone_from(&entry);
        self.cursor = self.value.len();
        self.stop_history_search();
        Some(entry)
    }

    /// Check if history search is active.
    pub fn is_history_search_active(&self) -> bool {
        self.history_search_active
    }

    /// Get the current history search query.
    pub fn get_history_query(&self) -> &str {
        &self.history_search_query
    }

    /// Get the currently selected history entry.
    pub fn get_selected_history(&self) -> Option<&str> {
        if self.history_matches.is_empty() {
            return None;
        }
        let idx = *self.history_matches.get(self.history_selected)?;
        self.history.get(idx).map(|s| s.as_str())
    }

    /// Get filtered history matches.
    pub fn get_history_matches(&self) -> Vec<&str> {
        self.history_matches
            .iter()
            .filter_map(|&idx| self.history.get(idx).map(|s| s.as_str()))
            .collect()
    }

    // ========== Enhanced Edit Methods ==========

    /// Insert a character at cursor position (with undo support).
    pub fn insert_with_undo(&mut self, c: char) {
        self.record_edit();
        self.insert(c);
    }

    /// Delete character before cursor (with undo support).
    pub fn backspace_with_undo(&mut self) {
        if self.cursor > 0 {
            self.record_edit();
            self.backspace();
        }
    }

    /// Delete character at cursor (with undo support).
    pub fn delete_with_undo(&mut self) {
        if self.cursor < self.value.len() {
            self.record_edit();
            self.delete();
        }
    }

    /// Clear the input (with undo support).
    pub fn clear_with_undo(&mut self) {
        if !self.value.is_empty() {
            self.record_edit();
            self.clear();
        }
    }

    /// Set value (with undo support).
    pub fn set_value_with_undo(&mut self, value: impl Into<String>) {
        self.record_edit();
        self.value = value.into();
        self.cursor = self.value.len();
    }

    // ========== Rendering Helpers ==========

    /// Build the completions popup node.
    fn build_completions_popup(&self) -> Option<Node> {
        if !self.completions_visible || self.completions.is_empty() {
            return None;
        }

        let filtered = self.filtered_completions();
        if filtered.is_empty() {
            return None;
        }

        let mut popup = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(BorderStyle::Single);

        for (i, completion) in filtered
            .iter()
            .take(self.completion_max_visible)
            .enumerate()
        {
            let mut text = TextNode::new(*completion);
            if i == self.completion_selected {
                text = text.bold().color(Color::BrightCyan);
            }
            popup = popup.child(text);
        }

        Some(popup.into())
    }

    /// Build the history search overlay node.
    fn build_history_overlay(&self) -> Option<Node> {
        if !self.history_search_active {
            return None;
        }

        let mut overlay = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(BorderStyle::Single);

        // Search prompt
        let prompt = format!("(reverse-i-search)`{}': ", self.history_search_query);
        overlay = overlay.child(TextNode::new(prompt).color(Color::BrightBlack));

        // Matching entries
        for (i, entry) in self.get_history_matches().iter().take(5).enumerate() {
            let mut text = TextNode::new(*entry);
            if i == self.history_selected {
                text = text.bold().color(Color::BrightCyan);
            } else {
                text = text.dim();
            }
            overlay = overlay.child(text);
        }

        Some(overlay.into())
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Input> for Node {
    fn from(input: Input) -> Self {
        let display = input.display_text();
        let is_empty = display.is_empty();

        // Create the text content
        let text_content = if is_empty {
            input.placeholder.clone().unwrap_or_default()
        } else {
            display.into_owned()
        };

        // Create text node with appropriate color
        let mut text = TextNode::new(&text_content);
        if is_empty {
            text = text.color(input.placeholder_color);
        } else if let Some(color) = input.color {
            text = text.color(color);
        }

        // Set cursor position for terminal cursor placement when focused
        // This tells inky where to position the actual terminal cursor,
        // which the App will show/hide and position automatically.
        if input.focused && !is_empty {
            text = text.cursor_at(input.cursor);
        } else if input.focused && is_empty {
            // Cursor at start of placeholder
            text = text.cursor_at(0);
        }

        // Create main row with input and token estimate
        let mut input_row = BoxNode::new().flex_direction(FlexDirection::Row);

        // Input box with border
        let mut input_box = BoxNode::new().border(input.border).child(text);
        if let Some(width) = input.width {
            input_box = input_box.width(width);
        }
        input_row = input_row.child(input_box);

        // Token estimate display
        if let Some(tokens) = input.token_estimate {
            let token_text = format!(" [~{} tokens]", tokens);
            input_row = input_row.child(TextNode::new(token_text).color(input.token_color));
        }

        // Build container with optional popups
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);
        container = container.child(input_row);

        // Add completions popup if visible
        if let Some(popup) = input.build_completions_popup() {
            container = container.child(popup);
        }

        // Add history search overlay if active
        if let Some(overlay) = input.build_history_overlay() {
            container = container.child(overlay);
        }

        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_input_new() {
        let input = Input::new();
        assert_eq!(input.get_value(), "");
        assert_eq!(input.get_cursor(), 0);
    }

    #[test]
    fn test_input_value() {
        let input = Input::new().value("hello");
        assert_eq!(input.get_value(), "hello");
        assert_eq!(input.get_cursor(), 5);
    }

    #[test]
    fn test_input_insert() {
        let mut input = Input::new().value("hllo");
        input.cursor = 1;
        input.insert('e');
        assert_eq!(input.get_value(), "hello");
        assert_eq!(input.get_cursor(), 2);
    }

    #[test]
    fn test_input_backspace() {
        let mut input = Input::new().value("hello");
        input.backspace();
        assert_eq!(input.get_value(), "hell");
        assert_eq!(input.get_cursor(), 4);
    }

    #[test]
    fn test_input_delete() {
        let mut input = Input::new().value("hello");
        input.cursor = 0;
        input.delete();
        assert_eq!(input.get_value(), "ello");
        assert_eq!(input.get_cursor(), 0);
    }

    #[test]
    fn test_input_navigation() {
        let mut input = Input::new().value("hello");
        assert_eq!(input.get_cursor(), 5);

        input.move_left();
        assert_eq!(input.get_cursor(), 4);

        input.move_home();
        assert_eq!(input.get_cursor(), 0);

        input.move_right();
        assert_eq!(input.get_cursor(), 1);

        input.move_end();
        assert_eq!(input.get_cursor(), 5);
    }

    #[test]
    fn test_input_password() {
        let input = Input::new().value("secret").password();
        assert_eq!(input.display_text(), "••••••");
    }

    #[test]
    fn test_input_to_node() {
        let input = Input::new()
            .value("test")
            .width(20)
            .border(BorderStyle::Rounded);
        let node: Node = input.into();
        assert!(matches!(node, Node::Box(_)));
    }

    // ========== Undo Tests ==========

    #[test]
    fn test_undo_disabled_by_default() {
        let input = Input::new();
        assert!(!input.is_undo_enabled());
    }

    #[test]
    fn test_undo_enabled() {
        let input = Input::new().enable_undo();
        assert!(input.is_undo_enabled());
    }

    #[test]
    fn test_undo_basic() {
        let mut input = Input::new().enable_undo();
        input.value = "hello".to_string();
        input.cursor = 5;

        input.commit_undo();
        input.value = "hello world".to_string();
        input.cursor = 11;

        assert!(input.can_undo());
        assert!(input.undo());
        assert_eq!(input.get_value(), "hello");
        assert_eq!(input.get_cursor(), 5);
    }

    #[test]
    fn test_redo_basic() {
        let mut input = Input::new().enable_undo();
        input.value = "hello".to_string();
        input.cursor = 5;

        input.commit_undo();
        input.value = "hello world".to_string();
        input.cursor = 11;

        input.undo();
        assert!(input.can_redo());
        assert!(input.redo());
        assert_eq!(input.get_value(), "hello world");
        assert_eq!(input.get_cursor(), 11);
    }

    #[test]
    fn test_undo_clears_redo_on_new_edit() {
        let mut input = Input::new().enable_undo();
        input.value = "hello".to_string();
        input.commit_undo();
        input.value = "hello world".to_string();

        input.undo();
        assert!(input.can_redo());

        // New edit should clear redo stack
        input.commit_undo();
        input.value = "hello there".to_string();

        assert!(!input.can_redo());
    }

    #[test]
    fn test_undo_depth() {
        let mut input = Input::new().enable_undo();
        assert_eq!(input.undo_depth(), 0);

        input.commit_undo();
        assert_eq!(input.undo_depth(), 1);

        input.commit_undo();
        assert_eq!(input.undo_depth(), 2);
    }

    #[test]
    fn test_insert_with_undo() {
        let mut input = Input::new().enable_undo();
        input.value = "hello".to_string();
        input.cursor = 5;

        // Wait a bit to avoid coalescing (in tests this is immediate)
        std::thread::sleep(std::time::Duration::from_millis(600));

        input.insert_with_undo('!');
        assert_eq!(input.get_value(), "hello!");

        assert!(input.undo());
        assert_eq!(input.get_value(), "hello");
    }

    // ========== Token Estimate Tests ==========

    #[test]
    fn test_token_estimate() {
        let input = Input::new().value("hello").token_estimate(5);
        assert_eq!(input.get_token_estimate(), Some(5));
    }

    #[test]
    fn test_clear_token_estimate() {
        let input = Input::new().token_estimate(5).clear_token_estimate();
        assert_eq!(input.get_token_estimate(), None);
    }

    // ========== Completion Tests ==========

    #[test]
    fn test_completions() {
        let input = Input::new().completions(vec!["/help", "/history", "/clear"]);
        assert_eq!(input.filtered_completions().len(), 3);
    }

    #[test]
    fn test_filtered_completions() {
        let input = Input::new()
            .value("/h")
            .completions(vec!["/help", "/history", "/clear"]);
        let filtered = input.filtered_completions();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"/help"));
        assert!(filtered.contains(&"/history"));
    }

    #[test]
    fn test_completion_navigation() {
        let mut input = Input::new().completions(vec!["/help", "/history", "/clear"]);
        input.show_completions();
        assert!(input.is_completions_visible());
        assert_eq!(input.get_selected_completion(), Some("/help"));

        input.next_completion();
        assert_eq!(input.get_selected_completion(), Some("/history"));

        input.next_completion();
        assert_eq!(input.get_selected_completion(), Some("/clear"));

        input.next_completion(); // Wrap around
        assert_eq!(input.get_selected_completion(), Some("/help"));

        input.prev_completion(); // Wrap back
        assert_eq!(input.get_selected_completion(), Some("/clear"));
    }

    #[test]
    fn test_accept_completion() {
        let mut input = Input::new()
            .completions(vec!["/help", "/history"])
            .completions_visible(true);
        input.next_completion(); // Select /history

        let result = input.accept_completion();
        assert_eq!(result, Some("/history".to_string()));
        assert_eq!(input.get_value(), "/history");
        assert!(!input.is_completions_visible());
    }

    // ========== History Tests ==========

    #[test]
    fn test_history() {
        let input = Input::new().history(vec!["cmd1", "cmd2", "cmd3"]);
        assert_eq!(input.history.len(), 3);
    }

    #[test]
    fn test_history_search() {
        let mut input = Input::new().history(vec!["ls -la", "git status", "git commit"]);

        input.start_history_search();
        assert!(input.is_history_search_active());

        input.set_history_query("git");
        let matches = input.get_history_matches();
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&"git status"));
        assert!(matches.contains(&"git commit"));
    }

    #[test]
    fn test_history_navigation() {
        let mut input = Input::new().history(vec!["cmd1", "cmd2", "cmd3"]);
        input.start_history_search();

        assert_eq!(input.get_selected_history(), Some("cmd1"));

        input.next_history_match();
        assert_eq!(input.get_selected_history(), Some("cmd2"));

        input.prev_history_match();
        assert_eq!(input.get_selected_history(), Some("cmd1"));
    }

    #[test]
    fn test_accept_history() {
        let mut input = Input::new().history(vec!["ls -la", "git status"]);
        input.start_history_search();
        input.next_history_match(); // Select "git status"

        let result = input.accept_history();
        assert_eq!(result, Some("git status".to_string()));
        assert_eq!(input.get_value(), "git status");
        assert!(!input.is_history_search_active());
    }

    #[test]
    fn test_history_search_incremental() {
        let mut input = Input::new().history(vec!["ls -la", "git status", "git commit -m"]);
        input.start_history_search();

        input.history_search_insert('g');
        assert_eq!(input.get_history_matches().len(), 2);

        input.history_search_insert('i');
        input.history_search_insert('t');
        input.history_search_insert(' ');
        input.history_search_insert('c');
        assert_eq!(input.get_history_matches().len(), 1);
        assert_eq!(input.get_selected_history(), Some("git commit -m"));

        input.history_search_backspace();
        assert_eq!(input.get_history_matches().len(), 2);
    }

    #[test]
    fn test_stop_history_search() {
        let mut input = Input::new().history(vec!["cmd1"]);
        input.start_history_search();
        input.set_history_query("cmd");
        assert!(input.is_history_search_active());

        input.stop_history_search();
        assert!(!input.is_history_search_active());
        assert!(input.get_history_query().is_empty());
    }
}
