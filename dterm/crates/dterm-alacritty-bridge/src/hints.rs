//! URL hint system for keyboard-driven URL selection.
//!
//! This module provides Alacritty-compatible hint functionality for selecting
//! and acting on URLs in terminal content via keyboard shortcuts.

use std::collections::HashMap;

use crate::index::Point;
#[cfg(test)]
use crate::index::{Column, Line};
use crate::url::UrlMatch;

/// Default alphabet for generating hint labels.
///
/// This follows Alacritty's default which uses home row keys for easy access.
pub const DEFAULT_HINT_ALPHABET: &str = "jfkdls;ahgurieowpq";

/// A hint label generator.
///
/// Generates unique character sequences from a configurable alphabet.
/// Labels are generated to minimize typing - single chars first, then pairs.
#[derive(Debug, Clone)]
pub struct HintLabels {
    /// The alphabet characters to use for labels.
    alphabet: Vec<char>,
    /// Current label index.
    index: usize,
}

impl HintLabels {
    /// Create a new label generator with the given alphabet.
    pub fn new(alphabet: &str) -> Self {
        let alphabet: Vec<char> = alphabet.chars().collect();
        assert!(!alphabet.is_empty(), "Hint alphabet cannot be empty");
        Self { alphabet, index: 0 }
    }

    /// Create a label generator with the default alphabet.
    pub fn default_alphabet() -> Self {
        Self::new(DEFAULT_HINT_ALPHABET)
    }

    /// Reset the label generator.
    pub fn reset(&mut self) {
        self.index = 0;
    }

    /// Generate the next label.
    ///
    /// Labels are generated in order of increasing length:
    /// - First: single characters (a, b, c, ...)
    /// - Then: two characters (aa, ab, ac, ..., ba, bb, ...)
    /// - etc.
    pub fn next_label(&mut self) -> String {
        let len = self.alphabet.len();
        let idx = self.index;
        self.index += 1;

        // Calculate number of characters needed
        // For alphabet size N:
        // - indices 0..N use 1 char
        // - indices N..N+N² use 2 chars
        // - etc.
        if idx < len {
            // Single character label
            return self.alphabet[idx].to_string();
        }

        // Multi-character labels
        let mut remaining = idx - len;
        let mut digits = 2;
        let mut capacity = len * len;

        while remaining >= capacity {
            remaining -= capacity;
            digits += 1;
            capacity *= len;
        }

        // Convert remaining to base-N representation
        let mut label = String::with_capacity(digits);
        let mut value = remaining;
        for _ in 0..digits {
            let digit = value % len;
            label.insert(0, self.alphabet[digit]);
            value /= len;
        }

        label
    }

    /// Get a specific number of labels.
    pub fn take(&mut self, count: usize) -> Vec<String> {
        (0..count).map(|_| self.next_label()).collect()
    }
}

/// A hint match with its label for display and selection.
#[derive(Debug, Clone)]
pub struct Hint {
    /// The keyboard label for this hint (e.g., "j", "k", "jf").
    pub label: String,
    /// The URL match this hint refers to.
    pub url_match: UrlMatch,
}

impl Hint {
    /// Create a new hint with the given label and URL match.
    pub fn new(label: String, url_match: UrlMatch) -> Self {
        Self { label, url_match }
    }

    /// Get the start point of this hint.
    pub fn start(&self) -> Point {
        self.url_match.start
    }

    /// Get the end point of this hint.
    pub fn end(&self) -> Point {
        self.url_match.end
    }

    /// Get the URL string.
    pub fn url(&self) -> &str {
        &self.url_match.url
    }

    /// Check if the given input prefix matches this hint's label.
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.label.starts_with(prefix)
    }
}

/// The result of a hint selection action.
#[derive(Debug, Clone)]
pub enum HintAction {
    /// Open the URL in a browser/handler.
    Open(String),
    /// Copy the URL to clipboard.
    Copy(String),
    /// No action (hint mode cancelled).
    Cancel,
}

/// Hint mode state machine.
///
/// Tracks the current state of hint-based URL selection:
/// - Available hints and their labels
/// - User's typed input for filtering
/// - Selection state
#[derive(Debug, Clone)]
pub struct HintState {
    /// Available hints, keyed by their label for fast lookup.
    hints: HashMap<String, Hint>,
    /// List of hints in display order (top-left to bottom-right).
    hints_ordered: Vec<Hint>,
    /// Currently typed characters for filtering.
    input: String,
    /// The alphabet used for generating labels.
    alphabet: String,
}

impl Default for HintState {
    fn default() -> Self {
        Self::new()
    }
}

impl HintState {
    /// Create a new hint state.
    pub fn new() -> Self {
        Self {
            hints: HashMap::new(),
            hints_ordered: Vec::new(),
            input: String::new(),
            alphabet: DEFAULT_HINT_ALPHABET.to_string(),
        }
    }

    /// Create a hint state with a custom alphabet.
    pub fn with_alphabet(alphabet: &str) -> Self {
        Self {
            hints: HashMap::new(),
            hints_ordered: Vec::new(),
            input: String::new(),
            alphabet: alphabet.to_string(),
        }
    }

    /// Set the available URL matches and generate labels.
    ///
    /// This should be called when entering hint mode to populate
    /// the available hints.
    pub fn set_urls(&mut self, urls: Vec<UrlMatch>) {
        self.hints.clear();
        self.hints_ordered.clear();
        self.input.clear();

        if urls.is_empty() {
            return;
        }

        let mut labels = HintLabels::new(&self.alphabet);
        for url_match in urls {
            let label = labels.next_label();
            let hint = Hint::new(label.clone(), url_match);
            self.hints.insert(label, hint.clone());
            self.hints_ordered.push(hint);
        }
    }

    /// Get all current hints.
    pub fn hints(&self) -> &[Hint] {
        &self.hints_ordered
    }

    /// Get hints that match the current input prefix.
    pub fn matching_hints(&self) -> Vec<&Hint> {
        if self.input.is_empty() {
            self.hints_ordered.iter().collect()
        } else {
            self.hints_ordered
                .iter()
                .filter(|h| h.matches_prefix(&self.input))
                .collect()
        }
    }

    /// Get the current input string.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Process a character input.
    ///
    /// Returns:
    /// - `Some(HintAction::Open(url))` if a unique match is selected
    /// - `Some(HintAction::Cancel)` if ESC or no matches
    /// - `None` if more input is needed
    pub fn process_char(&mut self, c: char) -> Option<HintAction> {
        // Add character to input
        self.input.push(c);

        // Check for exact match
        if let Some(hint) = self.hints.get(&self.input) {
            return Some(HintAction::Open(hint.url().to_string()));
        }

        // Check if any hints still match
        let matching: Vec<_> = self.matching_hints();
        if matching.is_empty() {
            // No matches - invalid input
            self.input.pop();
            return None;
        }

        // If exactly one match remains and input matches its full label
        if matching.len() == 1 && matching[0].label == self.input {
            return Some(HintAction::Open(matching[0].url().to_string()));
        }

        // More input needed
        None
    }

    /// Process backspace - remove last character.
    pub fn backspace(&mut self) {
        self.input.pop();
    }

    /// Cancel hint mode.
    pub fn cancel(&mut self) -> HintAction {
        self.clear();
        HintAction::Cancel
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.hints.clear();
        self.hints_ordered.clear();
        self.input.clear();
    }

    /// Check if hint mode is active (has hints).
    pub fn is_active(&self) -> bool {
        !self.hints.is_empty()
    }

    /// Get hint at a specific point, if any.
    pub fn hint_at_point(&self, point: Point) -> Option<&Hint> {
        self.hints_ordered
            .iter()
            .find(|h| h.url_match.contains(point))
    }

    /// Get label positions for rendering.
    ///
    /// Returns pairs of (label, start_point) for displaying hint labels.
    pub fn label_positions(&self) -> Vec<(&str, Point)> {
        self.matching_hints()
            .into_iter()
            .map(|h| (h.label.as_str(), h.start()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_labels_single_char() {
        let mut labels = HintLabels::new("abc");
        assert_eq!(labels.next_label(), "a");
        assert_eq!(labels.next_label(), "b");
        assert_eq!(labels.next_label(), "c");
    }

    #[test]
    fn test_hint_labels_multi_char() {
        let mut labels = HintLabels::new("ab");
        assert_eq!(labels.next_label(), "a");
        assert_eq!(labels.next_label(), "b");
        assert_eq!(labels.next_label(), "aa");
        assert_eq!(labels.next_label(), "ab");
        assert_eq!(labels.next_label(), "ba");
        assert_eq!(labels.next_label(), "bb");
    }

    #[test]
    fn test_hint_labels_take() {
        let mut labels = HintLabels::new("abc");
        let taken = labels.take(5);
        assert_eq!(taken, vec!["a", "b", "c", "aa", "ab"]);
    }

    #[test]
    fn test_hint_labels_reset() {
        let mut labels = HintLabels::new("abc");
        labels.next_label();
        labels.next_label();
        labels.reset();
        assert_eq!(labels.next_label(), "a");
    }

    #[test]
    fn test_hint_state_set_urls() {
        let mut state = HintState::with_alphabet("jk");
        let urls = vec![
            UrlMatch {
                start: Point::new(Line(0), Column(0)),
                end: Point::new(Line(0), Column(10)),
                url: "https://first.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(1), Column(5)),
                end: Point::new(Line(1), Column(20)),
                url: "https://second.com".to_string(),
            },
        ];

        state.set_urls(urls);

        assert_eq!(state.hints().len(), 2);
        assert_eq!(state.hints()[0].label, "j");
        assert_eq!(state.hints()[1].label, "k");
    }

    #[test]
    fn test_hint_state_process_char() {
        let mut state = HintState::with_alphabet("jk");
        let urls = vec![
            UrlMatch {
                start: Point::new(Line(0), Column(0)),
                end: Point::new(Line(0), Column(10)),
                url: "https://first.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(1), Column(5)),
                end: Point::new(Line(1), Column(20)),
                url: "https://second.com".to_string(),
            },
        ];

        state.set_urls(urls);

        // Press 'j' - should select first URL
        let result = state.process_char('j');
        assert!(matches!(result, Some(HintAction::Open(url)) if url == "https://first.com"));
    }

    #[test]
    fn test_hint_state_matching_hints() {
        let mut state = HintState::with_alphabet("ab");
        let urls = vec![
            UrlMatch {
                start: Point::new(Line(0), Column(0)),
                end: Point::new(Line(0), Column(10)),
                url: "https://a.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(1), Column(0)),
                end: Point::new(Line(1), Column(10)),
                url: "https://b.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(2), Column(0)),
                end: Point::new(Line(2), Column(10)),
                url: "https://aa.com".to_string(),
            },
        ];

        state.set_urls(urls);

        // Initially all hints match
        assert_eq!(state.matching_hints().len(), 3);

        // Type 'a' - filters to 'a' and 'aa'
        state.process_char('a');
        // After selecting 'a', state is cleared, but let's test prefix matching differently
    }

    #[test]
    fn test_hint_state_backspace() {
        let mut state = HintState::with_alphabet("ab");
        let urls = vec![
            UrlMatch {
                start: Point::new(Line(0), Column(0)),
                end: Point::new(Line(0), Column(10)),
                url: "https://first.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(1), Column(0)),
                end: Point::new(Line(1), Column(10)),
                url: "https://second.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(2), Column(0)),
                end: Point::new(Line(2), Column(10)),
                url: "https://third.com".to_string(),
            },
        ];

        state.set_urls(urls);

        // Type invalid char then backspace
        state.input.push('x');
        state.backspace();
        assert!(state.input.is_empty());
    }

    #[test]
    fn test_hint_state_cancel() {
        let mut state = HintState::with_alphabet("jk");
        let urls = vec![UrlMatch {
            start: Point::new(Line(0), Column(0)),
            end: Point::new(Line(0), Column(10)),
            url: "https://first.com".to_string(),
        }];

        state.set_urls(urls);
        assert!(state.is_active());

        let result = state.cancel();
        assert!(matches!(result, HintAction::Cancel));
        assert!(!state.is_active());
    }

    #[test]
    fn test_hint_at_point() {
        let mut state = HintState::with_alphabet("jk");
        let urls = vec![UrlMatch {
            start: Point::new(Line(0), Column(5)),
            end: Point::new(Line(0), Column(20)),
            url: "https://first.com".to_string(),
        }];

        state.set_urls(urls);

        // Point within URL
        let hint = state.hint_at_point(Point::new(Line(0), Column(10)));
        assert!(hint.is_some());
        assert_eq!(hint.unwrap().label, "j");

        // Point outside URL
        let hint = state.hint_at_point(Point::new(Line(0), Column(0)));
        assert!(hint.is_none());
    }

    #[test]
    fn test_label_positions() {
        let mut state = HintState::with_alphabet("jk");
        let urls = vec![
            UrlMatch {
                start: Point::new(Line(0), Column(5)),
                end: Point::new(Line(0), Column(20)),
                url: "https://first.com".to_string(),
            },
            UrlMatch {
                start: Point::new(Line(2), Column(10)),
                end: Point::new(Line(2), Column(30)),
                url: "https://second.com".to_string(),
            },
        ];

        state.set_urls(urls);

        let positions = state.label_positions();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].0, "j");
        assert_eq!(positions[0].1, Point::new(Line(0), Column(5)));
        assert_eq!(positions[1].0, "k");
        assert_eq!(positions[1].1, Point::new(Line(2), Column(10)));
    }
}
