//! Pattern matching rules for platform usage linter

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during lint pattern operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum LintError {
    /// Failed to parse YAML patterns
    #[error("Failed to parse lint patterns: {0}")]
    YamlParse(#[from] serde_yml::Error),

    /// Invalid regex in pattern
    #[error("Invalid regex in pattern '{pattern}': {source}")]
    InvalidRegex {
        /// Name of the pattern containing the invalid regex
        pattern: String,
        /// The regex parsing error
        source: regex::Error,
    },

    /// Failed to read file
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        /// Path to the file that couldn't be read
        path: String,
        /// The underlying I/O error
        source: std::io::Error,
    },

    /// Failed to serialize result
    #[error("Failed to serialize lint result: {0}")]
    Serialize(#[from] serde_json::Error),

    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Other error with custom message
    #[error("{0}")]
    Other(String),
}

/// Result type for lint operations
pub type LintResult<T> = std::result::Result<T, LintError>;

/// Severity level for lint warnings
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational only
    Info,
    /// Should use platform feature
    #[default]
    Warn,
    /// Must use platform feature (blocks CI)
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warn => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A single lint pattern definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintPattern {
    /// Unique identifier for the pattern
    pub name: String,

    /// Category for grouping related patterns
    pub category: String,

    /// Severity level
    #[serde(default)]
    pub severity: Severity,

    /// Regex patterns that trigger this lint
    pub triggers: Vec<String>,

    /// Primary DashFlow module that provides this functionality
    pub platform_module: String,

    /// Alternative modules that also provide this functionality
    #[serde(default)]
    pub alternate_modules: Vec<String>,

    /// User-friendly explanation
    pub message: String,

    /// Example code showing platform usage
    #[serde(default)]
    pub example_usage: Option<String>,

    /// Documentation URL
    #[serde(default)]
    pub docs_url: Option<String>,

    /// Paths/patterns where this rule doesn't apply
    #[serde(default)]
    pub exceptions: Vec<String>,

    /// Capability tags for automatic correlation with introspection types
    /// e.g., ["bm25", "retriever", "search"] allows automatic matching
    /// of patterns to types with the same capability tags
    #[serde(default)]
    pub capability_tags: Vec<String>,

    /// Compiled regex patterns (not serialized)
    #[serde(skip)]
    compiled_triggers: Vec<Regex>,
}

impl LintPattern {
    /// Compile the regex triggers for this pattern
    pub fn compile(&mut self) -> LintResult<()> {
        self.compiled_triggers = Vec::new();
        for t in &self.triggers {
            let regex = Regex::new(t).map_err(|source| LintError::InvalidRegex {
                pattern: self.name.clone(),
                source,
            })?;
            self.compiled_triggers.push(regex);
        }
        Ok(())
    }

    /// Check if a line matches any of the triggers
    pub fn matches(&self, line: &str) -> Option<&str> {
        for (i, regex) in self.compiled_triggers.iter().enumerate() {
            if regex.is_match(line) {
                return Some(&self.triggers[i]);
            }
        }
        None
    }

    /// Check if a path should be excluded from this pattern
    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for exception in &self.exceptions {
            if glob_match(exception, &path_str) {
                return true;
            }
        }
        false
    }
}

/// Collection of lint patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintPatterns {
    /// Version of the pattern format
    pub version: String,

    /// List of patterns
    pub patterns: Vec<LintPattern>,

    /// Index by name for quick lookup
    #[serde(skip)]
    by_name: HashMap<String, usize>,

    /// Index by category
    #[serde(skip)]
    by_category: HashMap<String, Vec<usize>>,
}

impl LintPatterns {
    /// Path for custom patterns override file in workspace
    pub const CUSTOM_PATTERNS_PATH: &'static str = ".dashflow/lint/patterns.yaml";

    /// Load patterns from the default embedded YAML
    pub fn load_default() -> LintResult<Self> {
        let yaml = include_str!("patterns.yaml");
        Self::from_yaml(yaml)
    }

    /// Load patterns with workspace override support.
    ///
    /// First loads the default embedded patterns, then merges with any custom
    /// patterns found at `.dashflow/lint/patterns.yaml` in the workspace.
    ///
    /// Custom patterns can:
    /// - Add new patterns not in the default set
    /// - Override existing patterns by name (custom takes precedence)
    pub fn load_with_workspace(workspace_root: &Path) -> LintResult<Self> {
        let mut patterns = Self::load_default()?;

        // Check for custom patterns file
        let custom_path = workspace_root.join(Self::CUSTOM_PATTERNS_PATH);
        if custom_path.exists() {
            let custom = Self::from_file(&custom_path)?;
            patterns.merge(custom);
        }

        Ok(patterns)
    }

    /// Merge another set of patterns into this one.
    /// Patterns from `other` override patterns with the same name.
    pub fn merge(&mut self, other: LintPatterns) {
        for pattern in other.patterns {
            // Remove existing pattern with same name (if any)
            if let Some(idx) = self.by_name.get(&pattern.name) {
                self.patterns.remove(*idx);
            }
            self.patterns.push(pattern);
        }
        self.build_indexes();
    }

    /// Load patterns from a YAML string
    pub fn from_yaml(yaml: &str) -> LintResult<Self> {
        let mut patterns: LintPatterns = serde_yml::from_str(yaml)?;

        // Compile all regex patterns
        for pattern in &mut patterns.patterns {
            pattern.compile()?;
        }

        // Build indexes
        patterns.build_indexes();

        Ok(patterns)
    }

    /// Load patterns from a file
    pub fn from_file(path: &Path) -> LintResult<Self> {
        let yaml = std::fs::read_to_string(path).map_err(|source| LintError::FileRead {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_yaml(&yaml)
    }

    /// Build internal indexes for quick lookup
    fn build_indexes(&mut self) {
        self.by_name.clear();
        self.by_category.clear();

        for (i, pattern) in self.patterns.iter().enumerate() {
            self.by_name.insert(pattern.name.clone(), i);
            self.by_category
                .entry(pattern.category.clone())
                .or_default()
                .push(i);
        }
    }

    /// Get a pattern by name
    pub fn get(&self, name: &str) -> Option<&LintPattern> {
        self.by_name.get(name).map(|&i| &self.patterns[i])
    }

    /// Get all patterns in a category
    pub fn by_category(&self, category: &str) -> Vec<&LintPattern> {
        self.by_category
            .get(category)
            .map(|indices| indices.iter().map(|&i| &self.patterns[i]).collect())
            .unwrap_or_default()
    }

    /// Get all categories
    pub fn categories(&self) -> Vec<&str> {
        self.by_category.keys().map(|s| s.as_str()).collect()
    }

    /// Iterate over all patterns
    pub fn iter(&self) -> impl Iterator<Item = &LintPattern> {
        self.patterns.iter()
    }

    /// Create `LintPatterns` from a `ModulePatternRegistry`.
    ///
    /// This enables the linter to use the introspection-based capability registry
    /// instead of YAML patterns. The registry provides richer metadata and is
    /// populated via `#[dashflow::capability(...)]` attributes.
    ///
    /// # Category Mapping
    ///
    /// Categories are derived from capability tags:
    /// - `["cost", "tracking", ...]` → "observability"
    /// - `["search", "retriever", ...]` → "retrievers"
    /// - `["eval", "evaluation", ...]` → "evaluation"
    /// - `["llm", "chat", ...]` → "models"
    /// - `["memory", "history", ...]` → "memory"
    /// - `["document", "loader", ...]` → "loaders"
    /// - `["chain", "rag", ...]` → "chains"
    pub fn from_registry(
        registry: &crate::introspection::ModulePatternRegistry,
    ) -> LintResult<Self> {
        let mut patterns = Vec::new();

        for entry in registry.entries() {
            // Derive category from capability tags
            let category = derive_category(&entry.capability_tags);

            // Derive pattern name from module path (last component, snake_case)
            let name = entry
                .module_path
                .split("::")
                .last()
                .unwrap_or(&entry.module_path)
                .to_lowercase()
                .replace("::", "_");

            // Convert each replacement pattern
            for replace_pattern in &entry.replaces_patterns {
                let mut pattern = LintPattern {
                    name: name.clone(),
                    category: category.clone(),
                    severity: convert_severity(replace_pattern.severity),
                    triggers: replace_pattern.triggers.clone(),
                    platform_module: entry.module_path.clone(),
                    alternate_modules: Vec::new(),
                    message: replace_pattern.message.clone(),
                    example_usage: if entry.example_usage.is_empty() {
                        None
                    } else {
                        Some(entry.example_usage.clone())
                    },
                    docs_url: entry.docs_url.clone(),
                    exceptions: Vec::new(),
                    capability_tags: entry.capability_tags.clone(),
                    compiled_triggers: Vec::new(),
                };

                // Compile regex triggers
                pattern.compile()?;
                patterns.push(pattern);
            }
        }

        let mut lint_patterns = Self {
            version: "2.0-registry".to_string(),
            patterns,
            by_name: HashMap::new(),
            by_category: HashMap::new(),
        };

        lint_patterns.build_indexes();
        Ok(lint_patterns)
    }
}

/// Derive category from capability tags
///
/// Order matters: more specific categories are checked first to avoid
/// ambiguous matches (e.g., "chat" could match both memory and models).
fn derive_category(tags: &[String]) -> String {
    let tags_lower: Vec<_> = tags.iter().map(|t| t.to_lowercase()).collect();

    // Check memory first - "history" and "conversation" are more specific than "chat"
    if tags_lower
        .iter()
        .any(|t| t == "memory" || t == "history" || t == "conversation")
    {
        return "memory".to_string();
    }

    if tags_lower.iter().any(|t| {
        t == "cost" || t == "tracking" || t == "telemetry" || t == "observability"
    }) {
        return "observability".to_string();
    }

    if tags_lower
        .iter()
        .any(|t| t == "search" || t == "retriever" || t == "bm25" || t == "vector")
    {
        return "retrievers".to_string();
    }

    if tags_lower.iter().any(|t| t == "eval" || t == "evaluation") {
        return "evaluation".to_string();
    }

    if tags_lower
        .iter()
        .any(|t| t == "llm" || t == "chat" || t == "embedding" || t == "model")
    {
        return "models".to_string();
    }

    if tags_lower
        .iter()
        .any(|t| t == "document" || t == "loader" || t == "splitter" || t == "chunk")
    {
        return "loaders".to_string();
    }

    if tags_lower
        .iter()
        .any(|t| t == "chain" || t == "rag" || t == "qa")
    {
        return "chains".to_string();
    }

    // Default fallback
    "general".to_string()
}

/// Convert introspection Severity to lint Severity
fn convert_severity(sev: crate::introspection::Severity) -> Severity {
    match sev {
        crate::introspection::Severity::Info => Severity::Info,
        crate::introspection::Severity::Warn => Severity::Warn,
        crate::introspection::Severity::Error => Severity::Error,
    }
}

/// A match found by the linter
#[derive(Debug, Clone, Serialize)]
pub struct PatternMatch {
    /// The pattern that matched
    pub pattern_name: String,

    /// The trigger regex that matched
    pub trigger: String,

    /// The matched line content
    pub line_content: String,

    /// Line number (1-indexed)
    pub line_number: usize,

    /// Column where match starts (1-indexed)
    pub column: usize,
}

/// Simple glob matching (supports * and **)
fn glob_match(pattern: &str, path: &str) -> bool {
    // Convert glob to regex
    let regex_pattern = pattern
        .replace(".", "\\.")
        .replace("**", "{{DOUBLE_STAR}}")
        .replace("*", "[^/]*")
        .replace("{{DOUBLE_STAR}}", ".*");

    if let Ok(regex) = Regex::new(&format!("^{}$", regex_pattern)) {
        regex.is_match(path)
    } else {
        // Fallback to simple contains check
        path.contains(pattern.trim_matches('*'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_patterns() {
        let patterns = LintPatterns::load_default().unwrap();
        assert!(!patterns.patterns.is_empty());
        assert!(patterns.get("cost_tracking").is_some());
        assert!(patterns.get("bm25_search").is_some());
    }

    #[test]
    fn test_pattern_matching() {
        let patterns = LintPatterns::load_default().unwrap();
        let cost_pattern = patterns.get("cost_tracking").unwrap();

        assert!(cost_pattern.matches("pub struct CostTracker {").is_some());
        assert!(cost_pattern.matches("fn track_cost(").is_some());
        assert!(cost_pattern.matches("let x = 1;").is_none());
    }

    #[test]
    fn test_glob_match() {
        // Single * matches any non-slash characters
        assert!(glob_match("*/test*", "src/test_foo.rs"));
        // Note: */test* does NOT match lib/testing/mod.rs because * doesn't match /
        assert!(!glob_match("*/test*", "lib/testing/mod.rs"));
        assert!(!glob_match("*/test*", "src/main.rs"));

        // ** matches any characters including slashes
        assert!(glob_match("**/*.rs", "src/foo/bar.rs"));
        // **/test* matches paths ending with a segment starting with "test"
        assert!(glob_match("**/test*", "lib/testing")); // ends with test* segment
        assert!(glob_match("**/test*", "src/foo/test_utils.rs")); // ends with test* segment
        assert!(!glob_match("**/test*", "lib/testing/mod.rs")); // ends with mod.rs, not test*
        assert!(glob_match("src/*", "src/main.rs"));
        assert!(!glob_match("src/*", "src/foo/bar.rs"));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
    }

    #[test]
    fn test_categories() {
        let patterns = LintPatterns::load_default().unwrap();
        let categories = patterns.categories();

        assert!(categories.contains(&"observability"));
        assert!(categories.contains(&"retrievers"));
        assert!(categories.contains(&"evaluation"));
    }

    #[test]
    fn test_from_registry() {
        use crate::introspection::ModulePatternRegistry;

        let registry = ModulePatternRegistry::with_defaults();
        let patterns = LintPatterns::from_registry(&registry).unwrap();

        // Should have 17 patterns from registry
        assert_eq!(patterns.patterns.len(), 17);

        // Check version indicates registry source
        assert_eq!(patterns.version, "2.0-registry");

        // Categories should be derived from capability tags
        let categories = patterns.categories();
        assert!(categories.contains(&"observability"));
        assert!(categories.contains(&"retrievers"));
        assert!(categories.contains(&"evaluation"));
        assert!(categories.contains(&"models"));
        assert!(categories.contains(&"memory"));
        assert!(categories.contains(&"loaders"));
        assert!(categories.contains(&"chains"));
    }

    #[test]
    fn test_from_registry_pattern_matching() {
        use crate::introspection::ModulePatternRegistry;

        let registry = ModulePatternRegistry::with_defaults();
        let patterns = LintPatterns::from_registry(&registry).unwrap();

        // Test that patterns from registry match expected code
        let matches_cost: Vec<_> = patterns
            .iter()
            .filter(|p| p.matches("pub struct CostTracker {").is_some())
            .collect();
        assert!(!matches_cost.is_empty());

        let matches_bm25: Vec<_> = patterns
            .iter()
            .filter(|p| p.matches("fn bm25_search(query: &str)").is_some())
            .collect();
        assert!(!matches_bm25.is_empty());
    }

    #[test]
    fn test_derive_category() {
        assert_eq!(derive_category(&["cost".to_string()]), "observability");
        assert_eq!(derive_category(&["tracking".to_string()]), "observability");
        assert_eq!(derive_category(&["search".to_string()]), "retrievers");
        assert_eq!(derive_category(&["retriever".to_string()]), "retrievers");
        assert_eq!(derive_category(&["eval".to_string()]), "evaluation");
        assert_eq!(derive_category(&["llm".to_string()]), "models");
        assert_eq!(derive_category(&["chat".to_string()]), "models");
        assert_eq!(derive_category(&["memory".to_string()]), "memory");
        assert_eq!(derive_category(&["document".to_string()]), "loaders");
        assert_eq!(derive_category(&["chain".to_string()]), "chains");
        assert_eq!(derive_category(&["unknown".to_string()]), "general");
    }
}
