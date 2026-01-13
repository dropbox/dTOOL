//! Selection rule types and smart selection engine.
//!
//! Provides regex-based rules for context-aware text selection.

use regex::Regex;
use std::cmp::Ordering;

/// Priority levels for selection rules.
///
/// Higher priority rules are checked first. When multiple rules match,
/// the highest priority match wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RulePriority {
    /// Lowest priority - fallback rules
    Low = 0,
    /// Normal priority - most built-in rules
    Normal = 1,
    /// High priority - specific patterns that should override general ones
    High = 2,
    /// Highest priority - user-defined overrides
    Override = 3,
}

impl Default for RulePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// The kind of semantic unit a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionRuleKind {
    /// URL (http, https, ftp, file, etc.)
    Url,
    /// File path (absolute or relative)
    FilePath,
    /// Email address
    Email,
    /// IPv4 or IPv6 address
    IpAddress,
    /// Git commit hash (7+ hex characters)
    GitHash,
    /// Quoted string (single or double quotes)
    QuotedString,
    /// UUID
    Uuid,
    /// Semantic version (semver)
    SemVer,
    /// Custom user-defined pattern
    Custom,
}

impl SelectionRuleKind {
    /// Get a human-readable name for this kind.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::FilePath => "file_path",
            Self::Email => "email",
            Self::IpAddress => "ip_address",
            Self::GitHash => "git_hash",
            Self::QuotedString => "quoted_string",
            Self::Uuid => "uuid",
            Self::SemVer => "semver",
            Self::Custom => "custom",
        }
    }
}

/// A selection rule that matches semantic text units.
#[derive(Clone)]
pub struct SelectionRule {
    /// Human-readable name for this rule
    name: String,
    /// The kind of pattern this rule matches
    kind: SelectionRuleKind,
    /// Compiled regex pattern
    pattern: Regex,
    /// Priority for rule ordering
    priority: RulePriority,
    /// Whether this rule is enabled
    enabled: bool,
}

impl std::fmt::Debug for SelectionRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionRule")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("pattern", &self.pattern.as_str())
            .field("priority", &self.priority)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl SelectionRule {
    /// Create a new selection rule.
    ///
    /// # Panics
    ///
    /// Panics if the pattern is not a valid regex.
    pub fn new(name: &str, kind: SelectionRuleKind, pattern: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            pattern: Regex::new(pattern).expect("Invalid regex pattern"),
            priority: RulePriority::Normal,
            enabled: true,
        }
    }

    /// Create a new selection rule, returning an error if the pattern is invalid.
    pub fn try_new(
        name: &str,
        kind: SelectionRuleKind,
        pattern: &str,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            name: name.to_string(),
            kind,
            pattern: Regex::new(pattern)?,
            priority: RulePriority::Normal,
            enabled: true,
        })
    }

    /// Set the priority for this rule.
    #[must_use]
    pub fn with_priority(mut self, priority: RulePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Enable or disable this rule.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if this rule is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the rule name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the rule kind.
    #[must_use]
    pub fn kind(&self) -> SelectionRuleKind {
        self.kind
    }

    /// Get the priority.
    #[must_use]
    pub fn priority(&self) -> RulePriority {
        self.priority
    }

    /// Get the regex pattern string.
    #[must_use]
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }

    /// Find all matches in the given text.
    pub fn find_all<'a>(&'a self, text: &'a str) -> impl Iterator<Item = regex::Match<'a>> + 'a {
        self.pattern.find_iter(text)
    }

    /// Find the match that contains the given byte position.
    ///
    /// Returns `None` if no match contains that position.
    #[must_use]
    pub fn find_at_position<'a>(&self, text: &'a str, byte_pos: usize) -> Option<regex::Match<'a>> {
        self.pattern
            .find_iter(text)
            .find(move |m| m.start() <= byte_pos && byte_pos < m.end())
    }
}

/// A match result from the smart selection system.
#[derive(Debug, Clone)]
pub struct SelectionMatch {
    /// The matched text
    text: String,
    /// Start byte offset in the original text
    start: usize,
    /// End byte offset in the original text (exclusive)
    end: usize,
    /// The rule that matched
    rule_name: String,
    /// The kind of match
    kind: SelectionRuleKind,
}

impl SelectionMatch {
    /// Create a new selection match.
    pub fn new(
        text: impl Into<String>,
        start: usize,
        end: usize,
        rule_name: impl Into<String>,
        kind: SelectionRuleKind,
    ) -> Self {
        Self {
            text: text.into(),
            start,
            end,
            rule_name: rule_name.into(),
            kind,
        }
    }

    /// Get the matched text.
    #[must_use]
    pub fn matched_text(&self) -> &str {
        &self.text
    }

    /// Get the start byte offset.
    #[must_use]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Get the end byte offset (exclusive).
    #[must_use]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Get the name of the rule that matched.
    #[must_use]
    pub fn rule_name(&self) -> &str {
        &self.rule_name
    }

    /// Get the kind of match.
    #[must_use]
    pub fn kind(&self) -> SelectionRuleKind {
        self.kind
    }

    /// Get the byte length of the match.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if the match is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Built-in rule patterns.
pub struct BuiltinRules;

impl BuiltinRules {
    // URL pattern: matches http, https, ftp, file URLs
    // Captures the protocol and the rest of the URL
    // Uses non-greedy matching to avoid capturing trailing punctuation
    const URL_PATTERN: &'static str =
        r#"(?i)(?:https?|ftp|file)://[^\s<>\[\](){}'"`,;|\\^]+[^\s<>\[\](){}'"`,;|\\^.!?:]"#;

    // File path pattern: matches absolute and relative paths
    // Unix: /path/to/file, ./relative, ../parent
    // Windows: C:\path\to\file, .\relative
    const FILE_PATH_PATTERN: &'static str = r"(?:/(?:[a-zA-Z0-9._-]+/)*[a-zA-Z0-9._-]+|\.{1,2}/(?:[a-zA-Z0-9._-]+/)*[a-zA-Z0-9._-]+|[A-Za-z]:[/\\](?:[a-zA-Z0-9._-]+[/\\])*[a-zA-Z0-9._-]+)";

    // Email pattern: RFC 5321 compliant (simplified)
    const EMAIL_PATTERN: &'static str = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}";

    // IPv4 pattern with optional port
    const IPV4_PATTERN: &'static str = r"(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)(?::\d{1,5})?";

    // IPv6 pattern (simplified - matches most common formats)
    const IPV6_PATTERN: &'static str = r"\[?(?:(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|(?:[0-9a-fA-F]{1,4}:){1,7}:|(?:[0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|::(?:[0-9a-fA-F]{1,4}:){0,5}[0-9a-fA-F]{1,4}|::)\]?(?::\d{1,5})?";

    // Git hash pattern: 7-40 hex characters
    // Uses word boundaries to avoid matching partial hex strings
    // Note: This may match within larger hex strings at word boundaries
    const GIT_HASH_PATTERN: &'static str = r"\b[0-9a-fA-F]{7,40}\b";

    // Double-quoted string
    const DOUBLE_QUOTED_PATTERN: &'static str = r#""(?:[^"\\]|\\.)*""#;

    // Single-quoted string
    const SINGLE_QUOTED_PATTERN: &'static str = r"'(?:[^'\\]|\\.)*'";

    // Backtick-quoted string (common in markdown and shells)
    const BACKTICK_QUOTED_PATTERN: &'static str = r"`(?:[^`\\]|\\.)*`";

    // UUID pattern (8-4-4-4-12 hex format)
    const UUID_PATTERN: &'static str =
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

    // Semantic version pattern
    const SEMVER_PATTERN: &'static str = r"v?\d+\.\d+\.\d+(?:-[a-zA-Z0-9]+(?:\.[a-zA-Z0-9]+)*)?(?:\+[a-zA-Z0-9]+(?:\.[a-zA-Z0-9]+)*)?";

    /// Create the URL selection rule.
    #[must_use]
    pub fn url() -> SelectionRule {
        SelectionRule::new("url", SelectionRuleKind::Url, Self::URL_PATTERN)
            .with_priority(RulePriority::High)
    }

    /// Create the file path selection rule.
    #[must_use]
    pub fn file_path() -> SelectionRule {
        SelectionRule::new(
            "file_path",
            SelectionRuleKind::FilePath,
            Self::FILE_PATH_PATTERN,
        )
    }

    /// Create the email selection rule.
    #[must_use]
    pub fn email() -> SelectionRule {
        SelectionRule::new("email", SelectionRuleKind::Email, Self::EMAIL_PATTERN)
            .with_priority(RulePriority::High)
    }

    /// Create the IPv4 address selection rule.
    #[must_use]
    pub fn ipv4() -> SelectionRule {
        SelectionRule::new("ipv4", SelectionRuleKind::IpAddress, Self::IPV4_PATTERN)
    }

    /// Create the IPv6 address selection rule.
    #[must_use]
    pub fn ipv6() -> SelectionRule {
        SelectionRule::new("ipv6", SelectionRuleKind::IpAddress, Self::IPV6_PATTERN)
            .with_priority(RulePriority::Low)
    }

    /// Create the git hash selection rule.
    #[must_use]
    pub fn git_hash() -> SelectionRule {
        SelectionRule::new(
            "git_hash",
            SelectionRuleKind::GitHash,
            Self::GIT_HASH_PATTERN,
        )
    }

    /// Create the double-quoted string selection rule.
    #[must_use]
    pub fn double_quoted_string() -> SelectionRule {
        SelectionRule::new(
            "double_quoted",
            SelectionRuleKind::QuotedString,
            Self::DOUBLE_QUOTED_PATTERN,
        )
    }

    /// Create the single-quoted string selection rule.
    #[must_use]
    pub fn single_quoted_string() -> SelectionRule {
        SelectionRule::new(
            "single_quoted",
            SelectionRuleKind::QuotedString,
            Self::SINGLE_QUOTED_PATTERN,
        )
    }

    /// Create the backtick-quoted string selection rule.
    #[must_use]
    pub fn backtick_quoted_string() -> SelectionRule {
        SelectionRule::new(
            "backtick_quoted",
            SelectionRuleKind::QuotedString,
            Self::BACKTICK_QUOTED_PATTERN,
        )
    }

    /// Create the UUID selection rule.
    #[must_use]
    pub fn uuid() -> SelectionRule {
        SelectionRule::new("uuid", SelectionRuleKind::Uuid, Self::UUID_PATTERN)
    }

    /// Create the semantic version selection rule.
    #[must_use]
    pub fn semver() -> SelectionRule {
        SelectionRule::new("semver", SelectionRuleKind::SemVer, Self::SEMVER_PATTERN)
            .with_priority(RulePriority::Low)
    }

    /// Get all built-in rules.
    #[must_use]
    pub fn all() -> Vec<SelectionRule> {
        vec![
            Self::url(),
            Self::file_path(),
            Self::email(),
            Self::ipv4(),
            Self::ipv6(),
            Self::git_hash(),
            Self::double_quoted_string(),
            Self::single_quoted_string(),
            Self::backtick_quoted_string(),
            Self::uuid(),
            Self::semver(),
        ]
    }
}

/// Smart selection engine that applies rules to find semantic text units.
#[derive(Debug, Clone)]
pub struct SmartSelection {
    /// Selection rules, sorted by priority (highest first)
    rules: Vec<SelectionRule>,
}

impl Default for SmartSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartSelection {
    /// Create a new empty smart selection engine.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a smart selection engine with all built-in rules.
    #[must_use]
    pub fn with_builtin_rules() -> Self {
        let mut selection = Self::new();
        for rule in BuiltinRules::all() {
            selection.add_rule(rule);
        }
        selection
    }

    /// Add a selection rule.
    ///
    /// Rules are maintained in priority order (highest first).
    pub fn add_rule(&mut self, rule: SelectionRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove a rule by name.
    ///
    /// Returns `true` if a rule was removed.
    pub fn remove_rule(&mut self, name: &str) -> bool {
        let len_before = self.rules.len();
        self.rules.retain(|r| r.name != name);
        self.rules.len() < len_before
    }

    /// Get a rule by name.
    #[must_use]
    pub fn get_rule(&self, name: &str) -> Option<&SelectionRule> {
        self.rules.iter().find(|r| r.name == name)
    }

    /// Get a mutable reference to a rule by name.
    pub fn get_rule_mut(&mut self, name: &str) -> Option<&mut SelectionRule> {
        self.rules.iter_mut().find(|r| r.name == name)
    }

    /// Get all rules.
    #[must_use]
    pub fn rules(&self) -> &[SelectionRule] {
        &self.rules
    }

    /// Enable or disable a rule by name.
    ///
    /// Returns `true` if the rule was found.
    pub fn set_rule_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(rule) = self.get_rule_mut(name) {
            rule.set_enabled(enabled);
            true
        } else {
            false
        }
    }

    /// Find the best match at the given byte position in the text.
    ///
    /// Returns the highest-priority match that contains the position.
    #[must_use]
    pub fn find_at(&self, text: &str, byte_pos: usize) -> Option<SelectionMatch> {
        // Check bounds
        if byte_pos > text.len() {
            return None;
        }

        // Try each rule in priority order
        for rule in &self.rules {
            if !rule.is_enabled() {
                continue;
            }

            if let Some(m) = rule.find_at_position(text, byte_pos) {
                return Some(SelectionMatch::new(
                    m.as_str(),
                    m.start(),
                    m.end(),
                    &rule.name,
                    rule.kind,
                ));
            }
        }

        None
    }

    /// Find the best match at the given column position in the text.
    ///
    /// This converts the column (character count) to a byte position.
    /// Useful for terminal selection where positions are in columns.
    #[must_use]
    pub fn find_at_column(&self, text: &str, column: usize) -> Option<SelectionMatch> {
        // Convert column to byte position
        let byte_pos = column_to_byte_pos(text, column);
        self.find_at(text, byte_pos)
    }

    /// Find all matches in the text.
    ///
    /// Returns matches from all enabled rules, sorted by start position.
    /// Overlapping matches from different rules are all included.
    #[must_use]
    pub fn find_all(&self, text: &str) -> Vec<SelectionMatch> {
        let mut matches = Vec::new();

        for rule in &self.rules {
            if !rule.is_enabled() {
                continue;
            }

            for m in rule.find_all(text) {
                matches.push(SelectionMatch::new(
                    m.as_str(),
                    m.start(),
                    m.end(),
                    &rule.name,
                    rule.kind,
                ));
            }
        }

        // Sort by start position, then by priority (via rule order which maintains priority)
        matches.sort_by(|a, b| {
            match a.start.cmp(&b.start) {
                Ordering::Equal => a.len().cmp(&b.len()).reverse(), // longer matches first
                other => other,
            }
        });

        matches
    }

    /// Find all matches of a specific kind.
    #[must_use]
    pub fn find_by_kind(&self, text: &str, kind: SelectionRuleKind) -> Vec<SelectionMatch> {
        self.find_all(text)
            .into_iter()
            .filter(|m| m.kind == kind)
            .collect()
    }

    /// Check if the text at the given position matches any rule.
    #[must_use]
    pub fn has_match_at(&self, text: &str, byte_pos: usize) -> bool {
        self.find_at(text, byte_pos).is_some()
    }

    /// Get the word boundaries for smart selection at a position.
    ///
    /// If a rule matches at the position, returns the match boundaries.
    /// Otherwise, returns word boundaries based on whitespace/punctuation.
    #[must_use]
    pub fn word_boundaries_at(&self, text: &str, byte_pos: usize) -> Option<(usize, usize)> {
        // First try smart selection rules
        if let Some(m) = self.find_at(text, byte_pos) {
            return Some((m.start, m.end));
        }

        // Fall back to basic word boundaries
        Self::basic_word_boundaries(text, byte_pos)
    }

    /// Get basic word boundaries (fallback when no rule matches).
    ///
    /// Treats sequences of alphanumeric characters and underscores as words.
    #[must_use]
    pub fn basic_word_boundaries(text: &str, byte_pos: usize) -> Option<(usize, usize)> {
        if byte_pos > text.len() || text.is_empty() {
            return None;
        }

        let bytes = text.as_bytes();

        // Handle position at end of string
        let pos = byte_pos.min(text.len().saturating_sub(1));

        // Check if position is on a word character
        let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

        if !is_word_char(bytes[pos]) {
            return None;
        }

        // Find start of word
        let start = (0..=pos)
            .rev()
            .take_while(|&i| is_word_char(bytes[i]))
            .last()
            .unwrap_or(pos);

        // Find end of word
        let end = (pos..bytes.len())
            .take_while(|&i| is_word_char(bytes[i]))
            .last()
            .map(|i| i + 1)
            .unwrap_or(pos + 1);

        Some((start, end))
    }
}

/// Convert a column (character) position to a byte position.
fn column_to_byte_pos(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod rule_tests {
    use super::*;

    #[test]
    fn test_url_pattern() {
        let rule = BuiltinRules::url();
        let text = "Check https://example.com/path?q=1 for info";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "https://example.com/path?q=1");
    }

    #[test]
    fn test_url_with_trailing_punctuation() {
        let rule = BuiltinRules::url();
        let text = "See https://example.com.";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "https://example.com");
    }

    #[test]
    fn test_file_path_unix() {
        let rule = BuiltinRules::file_path();
        let text = "File at /home/user/file.txt exists";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "/home/user/file.txt");
    }

    #[test]
    fn test_file_path_relative() {
        let rule = BuiltinRules::file_path();
        let text = "Check ./src/main.rs file";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "./src/main.rs");
    }

    #[test]
    fn test_email_pattern() {
        let rule = BuiltinRules::email();
        let text = "Contact user@example.com for info";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "user@example.com");
    }

    #[test]
    fn test_ipv4_pattern() {
        let rule = BuiltinRules::ipv4();
        let text = "Server at 192.168.1.100:8080 is up";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "192.168.1.100:8080");
    }

    #[test]
    fn test_git_hash_pattern() {
        let rule = BuiltinRules::git_hash();
        let text = "Commit abc1234 fixed the bug";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "abc1234");
    }

    #[test]
    fn test_git_hash_full() {
        let rule = BuiltinRules::git_hash();
        let text = "SHA: abcdef0123456789abcdef0123456789abcdef01";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].as_str(),
            "abcdef0123456789abcdef0123456789abcdef01"
        );
    }

    #[test]
    fn test_quoted_string_double() {
        let rule = BuiltinRules::double_quoted_string();
        let text = r#"echo "hello world" done"#;
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), r#""hello world""#);
    }

    #[test]
    fn test_quoted_string_with_escape() {
        let rule = BuiltinRules::double_quoted_string();
        let text = r#"echo "hello \"world\"" done"#;
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), r#""hello \"world\"""#);
    }

    #[test]
    fn test_uuid_pattern() {
        let rule = BuiltinRules::uuid();
        let text = "ID: 550e8400-e29b-41d4-a716-446655440000 found";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_semver_pattern() {
        let rule = BuiltinRules::semver();
        let text = "Version v1.2.3-beta.1+build.456 released";
        let matches: Vec<_> = rule.find_all(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "v1.2.3-beta.1+build.456");
    }

    #[test]
    fn test_basic_word_boundaries() {
        let text = "hello world_test foo";

        // Middle of "hello"
        let bounds = SmartSelection::basic_word_boundaries(text, 2);
        assert_eq!(bounds, Some((0, 5)));

        // Middle of "world_test"
        let bounds = SmartSelection::basic_word_boundaries(text, 8);
        assert_eq!(bounds, Some((6, 16)));

        // On space
        let bounds = SmartSelection::basic_word_boundaries(text, 5);
        assert_eq!(bounds, None);
    }

    #[test]
    fn test_column_to_byte_pos() {
        let text = "hello";
        assert_eq!(column_to_byte_pos(text, 0), 0);
        assert_eq!(column_to_byte_pos(text, 2), 2);
        assert_eq!(column_to_byte_pos(text, 5), 5);
        assert_eq!(column_to_byte_pos(text, 10), 5); // past end

        // With multibyte chars
        let text = "helloworldZ"; // "wo" is "wo" in bytes
        assert_eq!(column_to_byte_pos(text, 0), 0);
        assert_eq!(column_to_byte_pos(text, 5), 5);
    }
}
