//! Trigger system for pattern-based actions on terminal output.
//!
//! This module provides a regex-based trigger system similar to iTerm2's triggers,
//! allowing pattern matching on terminal output to invoke actions like highlighting,
//! alerts, running commands, or agent notifications.
//!
//! # Architecture
//!
//! The trigger system is designed with these principles:
//!
//! - **Hot/Cold path separation**: Trigger evaluation should not block parsing
//! - **Rate limiting**: Prevents CPU thrashing on rapid output
//! - **Idempotent actions**: Safe to re-evaluate the same content
//! - **Post-processing**: Clean match boundaries for URLs and paths
//!
//! # Example
//!
//! Using the builder pattern (recommended):
//!
//! ```no_run
//! use dterm_core::triggers::{TriggerBuilder, TriggerAction, TriggerSet};
//!
//! let mut triggers = TriggerSet::new();
//! triggers.add(TriggerBuilder::new()
//!     .pattern(r"error:.*")
//!     .action(TriggerAction::Highlight {
//!         foreground: Some([255, 0, 0]),
//!         background: None,
//!     })
//!     .build()
//!     .unwrap());
//! ```
//!
//! Or using the direct constructor:
//!
//! ```no_run
//! use dterm_core::triggers::{Trigger, TriggerAction, TriggerSet};
//!
//! let mut triggers = TriggerSet::new();
//! triggers.add(Trigger::new(
//!     r"error:.*",
//!     TriggerAction::Highlight {
//!         foreground: Some([255, 0, 0]),
//!         background: None,
//!     },
//! ).unwrap());
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Builder for creating [`Trigger`] instances.
///
/// This provides a clean builder pattern where all configuration is set first,
/// then validated at `build()` time. This is the recommended way to create triggers.
///
/// # Example
///
/// ```
/// use dterm_core::triggers::{TriggerBuilder, TriggerAction};
///
/// let trigger = TriggerBuilder::new()
///     .pattern(r"error:.*")
///     .action(TriggerAction::Bell)
///     .name("error_alert")
///     .partial_line(true)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct TriggerBuilder {
    pattern: Option<String>,
    action: Option<TriggerAction>,
    name: Option<String>,
    partial_line: bool,
    idempotent: bool,
    enabled: bool,
}

impl TriggerBuilder {
    /// Create a new trigger builder with default values.
    ///
    /// Default values:
    /// - `partial_line`: false
    /// - `idempotent`: true
    /// - `enabled`: true
    #[must_use]
    pub fn new() -> Self {
        Self {
            pattern: None,
            action: None,
            name: None,
            partial_line: false,
            idempotent: true,
            enabled: true,
        }
    }

    /// Set the regex pattern for the trigger.
    ///
    /// This pattern will be validated when `build()` is called.
    #[must_use]
    pub fn pattern(mut self, pattern: &str) -> Self {
        self.pattern = Some(pattern.to_string());
        self
    }

    /// Set the action to execute when the pattern matches.
    #[must_use]
    pub fn action(mut self, action: TriggerAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Set a human-readable name for this trigger.
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Set whether to fire on partial lines (before newline).
    ///
    /// Default: false
    #[must_use]
    pub fn partial_line(mut self, enabled: bool) -> Self {
        self.partial_line = enabled;
        self
    }

    /// Set whether this action is safe to re-run on the same match.
    ///
    /// Default: true
    #[must_use]
    pub fn idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    /// Set whether the trigger is enabled.
    ///
    /// Default: true
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the trigger, validating the pattern.
    ///
    /// Returns an error if:
    /// - No pattern was specified
    /// - No action was specified
    /// - The pattern is invalid regex
    pub fn build(self) -> Result<Trigger, TriggerError> {
        let pattern = self.pattern.ok_or(TriggerError::MissingPattern)?;
        let action = self.action.ok_or(TriggerError::MissingAction)?;

        // Validate regex on build
        #[cfg(not(kani))]
        {
            regex::Regex::new(&pattern).map_err(|e| TriggerError::InvalidPattern {
                pattern: pattern.clone(),
                reason: e.to_string(),
            })?;
        }

        Ok(Trigger {
            name: self.name.unwrap_or_default(),
            pattern,
            #[cfg(not(kani))]
            compiled: None,
            action,
            partial_line: self.partial_line,
            idempotent: self.idempotent,
            enabled: self.enabled,
        })
    }
}

/// A compiled trigger pattern with associated action.
#[derive(Debug, Clone)]
pub struct Trigger {
    /// Human-readable name for this trigger
    pub name: String,
    /// The regex pattern (as string, compiled lazily)
    pattern: String,
    /// Compiled regex (lazily populated)
    #[cfg(not(kani))]
    compiled: Option<regex::Regex>,
    /// The action to execute on match
    pub action: TriggerAction,
    /// Whether to fire on partial lines (before newline)
    pub partial_line: bool,
    /// Whether this action is safe to re-run on the same match
    pub idempotent: bool,
    /// Enable/disable this trigger
    pub enabled: bool,
}

impl Trigger {
    /// Create a new trigger with the given pattern and action.
    ///
    /// Returns an error if the pattern is invalid regex.
    pub fn new(pattern: &str, action: TriggerAction) -> Result<Self, TriggerError> {
        // Validate regex on creation
        #[cfg(not(kani))]
        {
            regex::Regex::new(pattern).map_err(|e| TriggerError::InvalidPattern {
                pattern: pattern.to_string(),
                reason: e.to_string(),
            })?;
        }

        Ok(Self {
            name: String::new(),
            pattern: pattern.to_string(),
            #[cfg(not(kani))]
            compiled: None,
            action,
            partial_line: false,
            idempotent: true,
            enabled: true,
        })
    }

    /// Create a named trigger.
    #[must_use]
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set partial line matching.
    #[must_use]
    pub fn with_partial_line(mut self, enabled: bool) -> Self {
        self.partial_line = enabled;
        self
    }

    /// Set idempotency flag.
    #[must_use]
    pub fn with_idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    /// Get the pattern string.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Get or compile the regex.
    #[cfg(not(kani))]
    fn regex(&mut self) -> &regex::Regex {
        if self.compiled.is_none() {
            // Pattern was validated in new(), so this should never fail
            self.compiled = Some(regex::Regex::new(&self.pattern).unwrap());
        }
        self.compiled.as_ref().unwrap()
    }

    /// Check if the pattern matches the given text.
    ///
    /// Returns the match range if found.
    #[cfg(not(kani))]
    pub fn matches(&mut self, text: &str) -> Option<TriggerMatch> {
        let regex = self.regex();
        regex.find(text).map(|m| TriggerMatch {
            start: m.start(),
            end: m.end(),
            text: m.as_str().to_string(),
        })
    }

    /// Find all matches in the given text.
    #[cfg(not(kani))]
    pub fn find_all(&mut self, text: &str) -> Vec<TriggerMatch> {
        let regex = self.regex();
        regex
            .find_iter(text)
            .map(|m| TriggerMatch {
                start: m.start(),
                end: m.end(),
                text: m.as_str().to_string(),
            })
            .collect()
    }

    /// Stub for Kani proofs
    #[cfg(kani)]
    pub fn matches(&mut self, _text: &str) -> Option<TriggerMatch> {
        None
    }

    /// Stub for Kani proofs
    #[cfg(kani)]
    pub fn find_all(&mut self, _text: &str) -> Vec<TriggerMatch> {
        Vec::new()
    }
}

/// A match found by a trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerMatch {
    /// Start byte offset in the matched text
    pub start: usize,
    /// End byte offset in the matched text
    pub end: usize,
    /// The matched text
    pub text: String,
}

/// Actions that can be triggered on pattern matches.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerAction {
    /// Highlight the matched text with specified colors.
    /// Colors are RGB values [r, g, b].
    Highlight {
        /// Foreground color (if any)
        foreground: Option<[u8; 3]>,
        /// Background color (if any)
        background: Option<[u8; 3]>,
    },

    /// Show a system notification/alert.
    Alert {
        /// Alert title
        title: String,
        /// Alert message (can use $0 for full match, $1-$9 for groups)
        message: String,
    },

    /// Play a bell sound.
    Bell,

    /// Add a navigation mark at the matched line.
    Mark {
        /// Optional mark type/name
        mark_type: Option<String>,
    },

    /// Send text to the terminal (as if typed).
    SendText {
        /// Text to send (can use $0 for full match, $1-$9 for groups)
        text: String,
    },

    /// Run an external command.
    RunCommand {
        /// Command to run (can use $0 for full match, $1-$9 for groups)
        command: String,
        /// Arguments
        args: Vec<String>,
    },

    /// Capture the matched output.
    Capture {
        /// Target for captured output
        target: CaptureTarget,
    },

    /// Add an annotation to the matched region.
    Annotate {
        /// Annotation text (can use $0 for full match, $1-$9 for groups)
        text: String,
    },

    /// Set detected hostname (for semantic shell integration).
    SetHostname,

    /// Set detected directory (for semantic shell integration).
    SetDirectory,

    /// Detect shell prompt (alternative to OSC 133).
    DetectPrompt,

    /// Agent-specific: Notify the AI agent about this match.
    NotifyAgent {
        /// Context type for the agent
        context_type: String,
        /// Priority level (0 = low, 1 = normal, 2 = high)
        priority: u8,
    },

    /// Agent-specific: Require user approval before proceeding.
    RequireApproval {
        /// Description of what needs approval
        description: String,
        /// What action to take if approved
        approved_action: Box<TriggerAction>,
    },

    /// Custom action via callback ID.
    Custom {
        /// Unique identifier for the callback
        callback_id: u32,
    },
}

/// Target for captured output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureTarget {
    /// Capture to clipboard
    Clipboard,
    /// Capture to a named variable
    Variable(String),
    /// Capture to the toolbelt/sidebar
    Toolbelt,
}

/// Errors that can occur in the trigger system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerError {
    /// Invalid regex pattern
    InvalidPattern {
        /// The problematic pattern
        pattern: String,
        /// Description of the error
        reason: String,
    },
    /// Pattern not specified (builder error)
    MissingPattern,
    /// Action not specified (builder error)
    MissingAction,
    /// Trigger with this name already exists
    DuplicateName(String),
    /// Trigger not found
    NotFound(String),
}

impl std::fmt::Display for TriggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerError::InvalidPattern { pattern, reason } => {
                write!(f, "invalid trigger pattern '{}': {}", pattern, reason)
            }
            TriggerError::MissingPattern => {
                write!(f, "trigger pattern not specified")
            }
            TriggerError::MissingAction => {
                write!(f, "trigger action not specified")
            }
            TriggerError::DuplicateName(name) => {
                write!(f, "trigger with name '{}' already exists", name)
            }
            TriggerError::NotFound(name) => {
                write!(f, "trigger '{}' not found", name)
            }
        }
    }
}

impl std::error::Error for TriggerError {}

/// A collection of triggers.
#[derive(Debug, Clone, Default)]
pub struct TriggerSet {
    triggers: Vec<Trigger>,
}

impl TriggerSet {
    /// Create an empty trigger set.
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
        }
    }

    /// Add a trigger to the set.
    pub fn add(&mut self, trigger: Trigger) {
        self.triggers.push(trigger);
    }

    /// Add a trigger and return its index.
    pub fn add_with_index(&mut self, trigger: Trigger) -> usize {
        let idx = self.triggers.len();
        self.triggers.push(trigger);
        idx
    }

    /// Remove a trigger by index.
    pub fn remove(&mut self, index: usize) -> Option<Trigger> {
        if index < self.triggers.len() {
            Some(self.triggers.remove(index))
        } else {
            None
        }
    }

    /// Get a trigger by index.
    pub fn get(&self, index: usize) -> Option<&Trigger> {
        self.triggers.get(index)
    }

    /// Get a mutable trigger by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Trigger> {
        self.triggers.get_mut(index)
    }

    /// Get the number of triggers.
    pub fn len(&self) -> usize {
        self.triggers.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    /// Iterate over triggers.
    pub fn iter(&self) -> impl Iterator<Item = &Trigger> {
        self.triggers.iter()
    }

    /// Iterate over triggers mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Trigger> {
        self.triggers.iter_mut()
    }

    /// Clear all triggers.
    pub fn clear(&mut self) {
        self.triggers.clear();
    }
}

/// Result of evaluating triggers on a line.
#[derive(Debug, Clone)]
pub struct TriggerResult {
    /// Index of the trigger that matched
    pub trigger_index: usize,
    /// The match information
    pub match_info: TriggerMatch,
    /// The action to execute
    pub action: TriggerAction,
}

/// Evaluator that runs triggers on terminal output.
///
/// Handles rate limiting and tracking of evaluated lines to support
/// idempotent re-evaluation.
#[derive(Debug)]
pub struct TriggerEvaluator {
    /// The triggers to evaluate
    triggers: TriggerSet,
    /// Last evaluation time per line (for rate limiting)
    last_evaluated: HashMap<u64, Instant>,
    /// Rate limit duration (default 500ms)
    rate_limit: Duration,
    /// Maximum lines to track for rate limiting
    max_tracked_lines: usize,
    /// Counter for line IDs
    next_line_id: u64,
}

impl TriggerEvaluator {
    /// Create a new trigger evaluator.
    pub fn new() -> Self {
        Self {
            triggers: TriggerSet::new(),
            last_evaluated: HashMap::new(),
            rate_limit: Duration::from_millis(500),
            max_tracked_lines: 1000,
            next_line_id: 0,
        }
    }

    /// Create with a pre-populated trigger set.
    pub fn with_triggers(triggers: TriggerSet) -> Self {
        Self {
            triggers,
            last_evaluated: HashMap::new(),
            rate_limit: Duration::from_millis(500),
            max_tracked_lines: 1000,
            next_line_id: 0,
        }
    }

    /// Set the rate limit duration.
    pub fn set_rate_limit(&mut self, duration: Duration) {
        self.rate_limit = duration;
    }

    /// Get a reference to the trigger set.
    pub fn triggers(&self) -> &TriggerSet {
        &self.triggers
    }

    /// Get a mutable reference to the trigger set.
    pub fn triggers_mut(&mut self) -> &mut TriggerSet {
        &mut self.triggers
    }

    /// Add a trigger.
    pub fn add_trigger(&mut self, trigger: Trigger) {
        self.triggers.add(trigger);
    }

    /// Allocate a new line ID for tracking.
    pub fn allocate_line_id(&mut self) -> u64 {
        let id = self.next_line_id;
        self.next_line_id = self.next_line_id.wrapping_add(1);
        id
    }

    /// Check if a line should be evaluated (respects rate limiting).
    fn should_evaluate(&self, line_id: u64) -> bool {
        match self.last_evaluated.get(&line_id) {
            Some(last) => last.elapsed() >= self.rate_limit,
            None => true,
        }
    }

    /// Mark a line as evaluated.
    fn mark_evaluated(&mut self, line_id: u64) {
        // Clean up old entries if we have too many
        if self.last_evaluated.len() >= self.max_tracked_lines {
            let cutoff = Instant::now()
                .checked_sub(self.rate_limit * 2)
                .unwrap_or(Instant::now());
            self.last_evaluated.retain(|_, v| *v > cutoff);
        }

        self.last_evaluated.insert(line_id, Instant::now());
    }

    /// Evaluate all triggers on a line of text.
    ///
    /// # Arguments
    /// * `text` - The line text to evaluate
    /// * `line_id` - A unique identifier for this line (for rate limiting)
    /// * `is_partial` - Whether this is a partial line (no newline yet)
    ///
    /// # Returns
    /// A vector of trigger results for all matches found.
    pub fn evaluate(&mut self, text: &str, line_id: u64, is_partial: bool) -> Vec<TriggerResult> {
        // Check rate limiting
        if !self.should_evaluate(line_id) {
            return Vec::new();
        }

        let mut results = Vec::new();

        for (index, trigger) in self.triggers.iter_mut().enumerate() {
            // Skip disabled triggers
            if !trigger.enabled {
                continue;
            }

            // Skip non-partial triggers on partial lines
            if is_partial && !trigger.partial_line {
                continue;
            }

            // Find all matches
            for match_info in trigger.find_all(text) {
                results.push(TriggerResult {
                    trigger_index: index,
                    match_info,
                    action: trigger.action.clone(),
                });
            }
        }

        // Mark as evaluated
        self.mark_evaluated(line_id);

        results
    }

    /// Evaluate triggers on multiple lines.
    ///
    /// Returns results grouped by line.
    pub fn evaluate_lines(
        &mut self,
        lines: &[(u64, &str, bool)],
    ) -> Vec<(u64, Vec<TriggerResult>)> {
        lines
            .iter()
            .map(|(line_id, text, is_partial)| {
                let results = self.evaluate(text, *line_id, *is_partial);
                (*line_id, results)
            })
            .filter(|(_, results)| !results.is_empty())
            .collect()
    }

    /// Clear rate limiting state.
    pub fn clear_rate_limit_state(&mut self) {
        self.last_evaluated.clear();
    }
}

impl Default for TriggerEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Common built-in trigger patterns.
pub mod patterns {
    /// URL pattern (http, https, ftp)
    pub const URL: &str = r"https?://[^\s<>\[\]{}|\\^]+|ftp://[^\s<>\[\]{}|\\^]+";

    /// File path pattern (Unix-style)
    pub const FILE_PATH_UNIX: &str = r"(?:/[^\s:]+)+";

    /// File path pattern (Windows-style)
    pub const FILE_PATH_WINDOWS: &str = r#"[A-Za-z]:\\[^\s:*?"<>|]+"#;

    /// Email address pattern
    pub const EMAIL: &str = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}";

    /// IPv4 address pattern
    pub const IPV4: &str = r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b";

    /// Git SHA pattern (7+ hex chars)
    pub const GIT_SHA: &str = r"\b[0-9a-f]{7,40}\b";

    /// Error keyword pattern (common error indicators)
    pub const ERROR_KEYWORDS: &str = r"(?i)\b(error|failed|failure|exception|fatal|panic|abort)\b";

    /// Warning keyword pattern
    pub const WARNING_KEYWORDS: &str = r"(?i)\b(warning|warn|caution)\b";

    /// Success keyword pattern
    pub const SUCCESS_KEYWORDS: &str = r"(?i)\b(success|succeeded|passed|complete|done)\b";

    /// UUID pattern
    pub const UUID: &str = r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b";

    /// Semantic version pattern
    pub const SEMVER: &str = r"\b[vV]?\d+\.\d+\.\d+(?:-[a-zA-Z0-9.]+)?(?:\+[a-zA-Z0-9.]+)?\b";
}

/// Post-process a match to clean up boundaries.
///
/// This handles common issues like:
/// - Trailing punctuation
/// - Unbalanced brackets
/// - Trailing delimiters
pub fn post_process_match(text: &str) -> &str {
    let mut result = text;

    // Remove trailing punctuation that's likely not part of the match
    let trailing = &['.', ',', ':', ';', '?', '!', ')', ']', '}', '>', '\'', '"'];
    while !result.is_empty() {
        let last = result.chars().last().unwrap();
        if trailing.contains(&last) {
            // Check for balanced brackets before removing
            if last == ')' && result.matches('(').count() >= result.matches(')').count() {
                break;
            }
            if last == ']' && result.matches('[').count() >= result.matches(']').count() {
                break;
            }
            if last == '}' && result.matches('{').count() >= result.matches('}').count() {
                break;
            }
            result = &result[..result.len() - last.len_utf8()];
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_new_valid_pattern() {
        let trigger = Trigger::new(r"\d+", TriggerAction::Bell);
        assert!(trigger.is_ok());
    }

    #[test]
    fn trigger_new_invalid_pattern() {
        let trigger = Trigger::new(r"[invalid", TriggerAction::Bell);
        assert!(trigger.is_err());
        if let Err(TriggerError::InvalidPattern { pattern, .. }) = trigger {
            assert_eq!(pattern, "[invalid");
        } else {
            panic!("Expected InvalidPattern error");
        }
    }

    #[test]
    fn trigger_matches_simple() {
        let mut trigger = Trigger::new(r"error", TriggerAction::Bell).unwrap();
        let result = trigger.matches("An error occurred");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.text, "error");
        assert_eq!(m.start, 3);
        assert_eq!(m.end, 8);
    }

    #[test]
    fn trigger_matches_no_match() {
        let mut trigger = Trigger::new(r"error", TriggerAction::Bell).unwrap();
        let result = trigger.matches("All is well");
        assert!(result.is_none());
    }

    #[test]
    fn trigger_find_all() {
        let mut trigger = Trigger::new(r"\d+", TriggerAction::Bell).unwrap();
        let results = trigger.find_all("foo 123 bar 456 baz");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "123");
        assert_eq!(results[1].text, "456");
    }

    #[test]
    fn trigger_set_operations() {
        let mut set = TriggerSet::new();
        assert!(set.is_empty());

        let t1 = Trigger::new(r"error", TriggerAction::Bell).unwrap();
        let t2 = Trigger::new(r"warn", TriggerAction::Bell).unwrap();

        set.add(t1);
        assert_eq!(set.len(), 1);

        let idx = set.add_with_index(t2);
        assert_eq!(idx, 1);
        assert_eq!(set.len(), 2);

        assert!(set.get(0).is_some());
        assert!(set.get(2).is_none());

        set.remove(0);
        assert_eq!(set.len(), 1);

        set.clear();
        assert!(set.is_empty());
    }

    #[test]
    fn evaluator_basic() {
        let mut eval = TriggerEvaluator::new();
        eval.add_trigger(
            Trigger::new(
                r"error",
                TriggerAction::Highlight {
                    foreground: Some([255, 0, 0]),
                    background: None,
                },
            )
            .unwrap(),
        );

        let results = eval.evaluate("An error occurred", 1, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_info.text, "error");
    }

    #[test]
    fn evaluator_rate_limiting() {
        let mut eval = TriggerEvaluator::new();
        eval.set_rate_limit(Duration::from_secs(10)); // Long rate limit for test
        eval.add_trigger(Trigger::new(r"test", TriggerAction::Bell).unwrap());

        // First evaluation should work
        let results = eval.evaluate("test", 1, false);
        assert_eq!(results.len(), 1);

        // Second evaluation with same line_id should be rate limited
        let results = eval.evaluate("test", 1, false);
        assert!(results.is_empty());

        // Different line_id should work
        let results = eval.evaluate("test", 2, false);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn evaluator_partial_line() {
        let mut eval = TriggerEvaluator::new();

        let mut t1 = Trigger::new(r"error", TriggerAction::Bell).unwrap();
        t1.partial_line = true;
        eval.add_trigger(t1);

        let mut t2 = Trigger::new(r"warn", TriggerAction::Bell).unwrap();
        t2.partial_line = false;
        eval.add_trigger(t2);

        // Partial line: only t1 should match
        let results = eval.evaluate("error warn", 1, true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_info.text, "error");

        // Complete line: both should match
        eval.clear_rate_limit_state();
        let results = eval.evaluate("error warn", 2, false);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn evaluator_disabled_trigger() {
        let mut eval = TriggerEvaluator::new();

        let mut trigger = Trigger::new(r"test", TriggerAction::Bell).unwrap();
        trigger.enabled = false;
        eval.add_trigger(trigger);

        let results = eval.evaluate("test", 1, false);
        assert!(results.is_empty());
    }

    #[test]
    fn builtin_patterns() {
        // URL pattern
        let mut trigger = Trigger::new(patterns::URL, TriggerAction::Bell).unwrap();
        let m = trigger.matches("Visit https://example.com/path?query=1");
        assert!(m.is_some());
        assert!(m.unwrap().text.starts_with("https://"));

        // Email pattern
        let mut trigger = Trigger::new(patterns::EMAIL, TriggerAction::Bell).unwrap();
        let m = trigger.matches("Contact test@example.com for help");
        assert!(m.is_some());
        assert_eq!(m.unwrap().text, "test@example.com");

        // Error keywords
        let mut trigger = Trigger::new(patterns::ERROR_KEYWORDS, TriggerAction::Bell).unwrap();
        assert!(trigger.matches("An ERROR occurred").is_some());
        assert!(trigger.matches("FATAL: disk full").is_some());
        assert!(trigger.matches("All is well").is_none());
    }

    #[test]
    fn post_process_trailing_punctuation() {
        assert_eq!(
            post_process_match("https://example.com."),
            "https://example.com"
        );
        assert_eq!(post_process_match("path/to/file,"), "path/to/file");
        assert_eq!(post_process_match("word;"), "word");
    }

    #[test]
    fn post_process_balanced_brackets() {
        // Balanced - don't remove
        assert_eq!(post_process_match("func(arg)"), "func(arg)");
        assert_eq!(post_process_match("array[0]"), "array[0]");

        // Unbalanced - remove
        assert_eq!(post_process_match("word)"), "word");
        assert_eq!(post_process_match("url]"), "url");
    }

    #[test]
    fn trigger_action_clone() {
        let action = TriggerAction::Alert {
            title: "Test".to_string(),
            message: "Message".to_string(),
        };
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn capture_target_variants() {
        // Verify enum variants can be constructed and pattern matched
        let clipboard = CaptureTarget::Clipboard;
        let var = CaptureTarget::Variable("test".to_string());
        let toolbelt = CaptureTarget::Toolbelt;

        assert!(matches!(clipboard, CaptureTarget::Clipboard));
        assert!(matches!(var, CaptureTarget::Variable(_)));
        assert!(matches!(toolbelt, CaptureTarget::Toolbelt));
    }

    #[test]
    fn trigger_with_builders() {
        let trigger = Trigger::new(r"test", TriggerAction::Bell)
            .unwrap()
            .with_name("test_trigger")
            .with_partial_line(true)
            .with_idempotent(false);

        assert_eq!(trigger.name, "test_trigger");
        assert!(trigger.partial_line);
        assert!(!trigger.idempotent);
    }

    #[test]
    fn evaluator_evaluate_lines() {
        let mut eval = TriggerEvaluator::new();
        eval.add_trigger(Trigger::new(r"error", TriggerAction::Bell).unwrap());

        let lines = vec![
            (1, "line one", false),
            (2, "error here", false),
            (3, "line three", false),
        ];

        let results = eval.evaluate_lines(&lines);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 2);
        assert_eq!(results[0].1.len(), 1);
    }

    #[test]
    fn trigger_error_display() {
        let err = TriggerError::InvalidPattern {
            pattern: "[bad".to_string(),
            reason: "unclosed bracket".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("[bad"));
        assert!(msg.contains("unclosed bracket"));

        let err = TriggerError::DuplicateName("test".to_string());
        assert!(err.to_string().contains("test"));

        let err = TriggerError::NotFound("missing".to_string());
        assert!(err.to_string().contains("missing"));

        let err = TriggerError::MissingPattern;
        assert!(err.to_string().contains("pattern"));

        let err = TriggerError::MissingAction;
        assert!(err.to_string().contains("action"));
    }

    #[test]
    fn trigger_builder_basic() {
        let trigger = TriggerBuilder::new()
            .pattern(r"\d+")
            .action(TriggerAction::Bell)
            .build();
        assert!(trigger.is_ok());
        let t = trigger.unwrap();
        assert_eq!(t.pattern(), r"\d+");
        assert!(t.enabled);
        assert!(t.idempotent);
        assert!(!t.partial_line);
    }

    #[test]
    fn trigger_builder_all_options() {
        let trigger = TriggerBuilder::new()
            .pattern(r"error")
            .action(TriggerAction::Bell)
            .name("error_alert")
            .partial_line(true)
            .idempotent(false)
            .enabled(false)
            .build()
            .unwrap();

        assert_eq!(trigger.name, "error_alert");
        assert_eq!(trigger.pattern(), "error");
        assert!(trigger.partial_line);
        assert!(!trigger.idempotent);
        assert!(!trigger.enabled);
    }

    #[test]
    fn trigger_builder_missing_pattern() {
        let result = TriggerBuilder::new().action(TriggerAction::Bell).build();
        assert!(matches!(result, Err(TriggerError::MissingPattern)));
    }

    #[test]
    fn trigger_builder_missing_action() {
        let result = TriggerBuilder::new().pattern(r"test").build();
        assert!(matches!(result, Err(TriggerError::MissingAction)));
    }

    #[test]
    fn trigger_builder_invalid_pattern() {
        let result = TriggerBuilder::new()
            .pattern(r"[invalid")
            .action(TriggerAction::Bell)
            .build();
        assert!(matches!(result, Err(TriggerError::InvalidPattern { .. })));
    }

    #[test]
    fn trigger_builder_default() {
        let builder = TriggerBuilder::default();
        // Default builder has no pattern or action, so build should fail
        assert!(builder.build().is_err());
    }

    #[test]
    fn trigger_builder_clone() {
        let builder = TriggerBuilder::new()
            .pattern(r"test")
            .action(TriggerAction::Bell)
            .name("cloned");

        let cloned = builder.clone();
        let t1 = builder.build().unwrap();
        let t2 = cloned.build().unwrap();
        assert_eq!(t1.name, t2.name);
        assert_eq!(t1.pattern(), t2.pattern());
    }

    #[test]
    fn trigger_builder_matches() {
        let mut trigger = TriggerBuilder::new()
            .pattern(r"error")
            .action(TriggerAction::Bell)
            .build()
            .unwrap();

        let result = trigger.matches("An error occurred");
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "error");
    }
}
