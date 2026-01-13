//! Selection system for terminal text.
//!
//! This module provides two selection systems:
//!
//! ## Smart Selection (Pattern-based)
//!
//! Intelligent selection rules that recognize and select semantic text units
//! like URLs, file paths, email addresses, IP addresses, git hashes, and
//! quoted strings.
//!
//! - **Pattern-based rules**: Use regex patterns to identify semantic text units
//! - **Built-in rules**: Pre-configured rules for common patterns (URLs, paths, etc.)
//! - **Extensible**: Add custom rules for application-specific patterns
//! - **Priority-based**: Rules are matched in priority order
//!
//! ### Example
//!
//! ```
//! use dterm_core::selection::{SmartSelection, SelectionMatch, BuiltinRules};
//!
//! let smart = SmartSelection::with_builtin_rules();
//!
//! // Given a line of text and a cursor column
//! let line = "Check out https://example.com for more info";
//! let matches = smart.find_at(line, 15); // cursor is on the URL
//!
//! if let Some(m) = matches {
//!     assert_eq!(m.matched_text(), "https://example.com");
//!     assert_eq!(m.rule_name(), "url");
//! }
//! ```
//!
//! ## Text Selection (Mouse-based)
//!
//! State machine for mouse-based text selection, implementing the TLA+ spec
//! in `tla/Selection.tla`. Supports:
//!
//! - **Simple selection**: Character-by-character (single click + drag)
//! - **Block selection**: Rectangular selection (Alt + click + drag)
//! - **Semantic selection**: Word/URL selection (double-click)
//! - **Line selection**: Full line selection (triple-click)
//!
//! ### Example
//!
//! ```
//! use dterm_core::selection::{TextSelection, SelectionType, SelectionSide};
//!
//! let mut sel = TextSelection::new();
//!
//! // Start selection on mouse down
//! sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Simple);
//!
//! // Update on mouse drag
//! sel.update_selection(0, 15, SelectionSide::Right);
//!
//! // Complete on mouse up
//! sel.complete_selection();
//!
//! // Check if a cell is selected
//! assert!(sel.contains(0, 10));
//! ```

mod rules;
mod text_selection;

pub use rules::{
    BuiltinRules, RulePriority, SelectionMatch, SelectionRule, SelectionRuleKind, SmartSelection,
};

pub use text_selection::{
    SelectionAnchor, SelectionSide, SelectionState, SelectionType, TextSelection,
};

#[cfg(test)]
mod tests;
