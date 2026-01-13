//! Feedback collection for platform usage linter
//!
//! Collects feedback from users/AI on why platform features weren't used.
//! This helps DashFlow team understand gaps in the platform and prioritize improvements.
//!
//! # Usage
//!
//! ```bash
//! # Submit feedback for a specific pattern
//! dashflow lint --feedback "Platform CostTracker doesn't support per-query breakdown" src/
//!
//! # View collected feedback
//! dashflow lint feedback list
//!
//! # Export feedback for analysis
//! dashflow lint feedback export --json
//! ```

use crate::core::config_loader::env_vars::{env_is_set, env_string, CI, CLAUDE_CODE, DASHFLOW_WORKER_ID};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single feedback entry from user/AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    /// Unique identifier for this feedback
    pub id: String,

    /// When the feedback was submitted
    pub timestamp: DateTime<Utc>,

    /// Pattern that triggered the lint warning
    pub pattern: String,

    /// Category of the pattern (observability, retrieval, etc.)
    pub category: String,

    /// File where the warning occurred
    pub file: PathBuf,

    /// Line number of the warning
    pub line: usize,

    /// The reason provided for not using platform feature
    pub reason: String,

    /// Optional suggested enhancement to platform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_enhancement: Option<String>,

    /// Reporter identifier (e.g., "ai-worker-session-994" or "user-manual")
    pub reporter: String,

    /// Platform module that was suggested
    pub platform_module: String,

    /// Whether this feedback has been reviewed by DashFlow team
    #[serde(default)]
    pub reviewed: bool,

    /// Tags for categorizing feedback
    #[serde(default)]
    pub tags: Vec<String>,
}

impl FeedbackEntry {
    /// Create a new feedback entry
    pub fn new(
        pattern: String,
        category: String,
        file: PathBuf,
        line: usize,
        reason: String,
        platform_module: String,
        reporter: String,
    ) -> Self {
        let id = format!(
            "{}-{}-{}",
            chrono::Utc::now().timestamp(),
            pattern,
            uuid_short()
        );

        Self {
            id,
            timestamp: Utc::now(),
            pattern,
            category,
            file,
            line,
            reason,
            suggested_enhancement: None,
            reporter,
            platform_module,
            reviewed: false,
            tags: Vec::new(),
        }
    }

    /// Add a suggested enhancement
    #[must_use]
    pub fn with_suggested_enhancement(mut self, enhancement: String) -> Self {
        self.suggested_enhancement = Some(enhancement);
        self
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Generate a short UUID-like identifier
fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:08x}", nanos)
}

/// Collection of feedback entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeedbackStore {
    /// All feedback entries
    pub entries: Vec<FeedbackEntry>,

    /// Summary statistics by pattern
    #[serde(default)]
    pub stats_by_pattern: HashMap<String, PatternStats>,

    /// Summary statistics by category
    #[serde(default)]
    pub stats_by_category: HashMap<String, CategoryStats>,

    /// Version of the feedback store format
    pub version: String,
}

/// Statistics for a specific pattern
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternStats {
    /// Number of feedback entries for this pattern
    pub count: usize,

    /// Most common reasons (grouped)
    pub common_reasons: Vec<ReasonCount>,

    /// Number of enhancement suggestions
    pub enhancement_suggestions: usize,
}

/// Statistics for a category
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryStats {
    /// Number of feedback entries for this category
    pub count: usize,

    /// Patterns in this category
    pub patterns: Vec<String>,
}

/// Count of a specific reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonCount {
    /// The reason text (or normalized key)
    pub reason: String,

    /// How many times this reason appeared
    pub count: usize,
}

impl FeedbackStore {
    /// Create a new empty store
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            stats_by_pattern: HashMap::new(),
            stats_by_category: HashMap::new(),
            version: "1.0".to_string(),
        }
    }

    /// Add a feedback entry
    pub fn add(&mut self, entry: FeedbackEntry) {
        // Update pattern stats
        let pattern_stats = self
            .stats_by_pattern
            .entry(entry.pattern.clone())
            .or_default();
        pattern_stats.count += 1;
        if entry.suggested_enhancement.is_some() {
            pattern_stats.enhancement_suggestions += 1;
        }

        // Update category stats
        let category_stats = self
            .stats_by_category
            .entry(entry.category.clone())
            .or_default();
        category_stats.count += 1;
        if !category_stats.patterns.contains(&entry.pattern) {
            category_stats.patterns.push(entry.pattern.clone());
        }

        self.entries.push(entry);
    }

    /// Get entries for a specific pattern
    pub fn entries_for_pattern(&self, pattern: &str) -> Vec<&FeedbackEntry> {
        self.entries
            .iter()
            .filter(|e| e.pattern == pattern)
            .collect()
    }

    /// Get entries for a specific category
    pub fn entries_for_category(&self, category: &str) -> Vec<&FeedbackEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Get unreviewed entries
    pub fn unreviewed(&self) -> Vec<&FeedbackEntry> {
        self.entries.iter().filter(|e| !e.reviewed).collect()
    }

    /// Mark an entry as reviewed
    pub fn mark_reviewed(&mut self, id: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.reviewed = true;
            true
        } else {
            false
        }
    }

    /// Get summary report
    pub fn summary_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Lint Feedback Summary ===\n\n");
        report.push_str(&format!("Total entries: {}\n", self.entries.len()));
        report.push_str(&format!("Unreviewed: {}\n\n", self.unreviewed().len()));

        report.push_str("By Category:\n");
        for (category, stats) in &self.stats_by_category {
            report.push_str(&format!("  {}: {} entries\n", category, stats.count));
        }

        report.push_str("\nBy Pattern:\n");
        let mut pattern_vec: Vec<_> = self.stats_by_pattern.iter().collect();
        pattern_vec.sort_by(|a, b| b.1.count.cmp(&a.1.count));
        for (pattern, stats) in pattern_vec.iter().take(10) {
            report.push_str(&format!(
                "  {}: {} entries ({} enhancement suggestions)\n",
                pattern, stats.count, stats.enhancement_suggestions
            ));
        }

        report
    }
}

/// Collector for saving/loading feedback to disk
pub struct FeedbackCollector {
    /// Path to the feedback file
    store_path: PathBuf,

    /// In-memory store
    store: FeedbackStore,
}

impl FeedbackCollector {
    /// Default feedback directory
    pub const DEFAULT_DIR: &'static str = ".dashflow/feedback";

    /// Default feedback filename
    pub const DEFAULT_FILENAME: &'static str = "lint_feedback.json";

    /// Create a new collector with default path
    pub fn new() -> Self {
        let store_path = Self::default_store_path();
        let store = Self::load_store_from(&store_path).unwrap_or_default();
        Self { store_path, store }
    }

    /// Create a collector with a custom store path
    #[must_use]
    pub fn with_store_path(store_path: PathBuf) -> Self {
        let store = Self::load_store_from(&store_path).unwrap_or_default();
        Self { store_path, store }
    }

    /// Get the default store path
    pub fn default_store_path() -> PathBuf {
        // Look for workspace root first
        let candidates = vec![
            PathBuf::from("."),
            PathBuf::from(".."),
            PathBuf::from("../.."),
        ];

        for candidate in candidates {
            let cargo_toml = candidate.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[workspace]") {
                        return candidate
                            .join(Self::DEFAULT_DIR)
                            .join(Self::DEFAULT_FILENAME);
                    }
                }
            }
        }

        // Fall back to current directory
        PathBuf::from(Self::DEFAULT_DIR).join(Self::DEFAULT_FILENAME)
    }

    /// Load store from a specific path
    fn load_store_from(path: &Path) -> Option<FeedbackStore> {
        if path.exists() {
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Add feedback and save
    pub fn add_feedback(&mut self, entry: FeedbackEntry) -> std::io::Result<()> {
        self.store.add(entry);
        self.save()
    }

    /// Save the store to disk
    pub fn save(&self) -> std::io::Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.store)?;
        std::fs::write(&self.store_path, json)
    }

    /// Get reference to the store
    pub fn store(&self) -> &FeedbackStore {
        &self.store
    }

    /// Get mutable reference to the store
    pub fn store_mut(&mut self) -> &mut FeedbackStore {
        &mut self.store
    }

    /// Export store as JSON string
    pub fn export_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.store)
    }

    /// Get the store path
    pub fn store_path(&self) -> &Path {
        &self.store_path
    }
}

impl Default for FeedbackCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick feedback submission from lint warning
pub fn submit_feedback(
    pattern: &str,
    category: &str,
    file: &Path,
    line: usize,
    reason: &str,
    platform_module: &str,
) -> std::io::Result<String> {
    let mut collector = FeedbackCollector::new();

    // Detect reporter type
    let reporter = detect_reporter();

    let entry = FeedbackEntry::new(
        pattern.to_string(),
        category.to_string(),
        file.to_path_buf(),
        line,
        reason.to_string(),
        platform_module.to_string(),
        reporter,
    );

    let id = entry.id.clone();
    collector.add_feedback(entry)?;

    Ok(id)
}

/// Detect if we're running in an AI worker context
fn detect_reporter() -> String {
    // Check for AI worker environment variables
    if let Some(worker_id) = env_string(DASHFLOW_WORKER_ID) {
        return format!("ai-worker-{}", worker_id);
    }

    // Check for CI environment
    if env_is_set(CI) {
        return "ci-automated".to_string();
    }

    // Check for Claude Code
    if env_is_set(CLAUDE_CODE) {
        return "ai-claude-code".to_string();
    }

    // Default to manual user
    "user-manual".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_feedback_entry_creation() {
        let entry = FeedbackEntry::new(
            "cost_tracking".to_string(),
            "observability".to_string(),
            PathBuf::from("src/cost.rs"),
            15,
            "Platform doesn't support per-query breakdown".to_string(),
            "dashflow_observability::cost".to_string(),
            "test-reporter".to_string(),
        );

        assert!(entry.id.contains("cost_tracking"));
        assert_eq!(entry.pattern, "cost_tracking");
        assert!(!entry.reviewed);
    }

    #[test]
    fn test_feedback_store() {
        let mut store = FeedbackStore::new();

        let entry1 = FeedbackEntry::new(
            "cost_tracking".to_string(),
            "observability".to_string(),
            PathBuf::from("src/cost.rs"),
            15,
            "Reason 1".to_string(),
            "dashflow_observability::cost".to_string(),
            "test".to_string(),
        );

        let entry2 = FeedbackEntry::new(
            "cost_tracking".to_string(),
            "observability".to_string(),
            PathBuf::from("src/other.rs"),
            20,
            "Reason 2".to_string(),
            "dashflow_observability::cost".to_string(),
            "test".to_string(),
        );

        store.add(entry1);
        store.add(entry2);

        assert_eq!(store.entries.len(), 2);
        assert_eq!(store.entries_for_pattern("cost_tracking").len(), 2);
        assert_eq!(
            store.stats_by_pattern.get("cost_tracking").unwrap().count,
            2
        );
    }

    #[test]
    fn test_feedback_collector_persistence() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("feedback.json");

        // Create and save
        {
            let mut collector = FeedbackCollector::with_store_path(store_path.clone());
            let entry = FeedbackEntry::new(
                "test_pattern".to_string(),
                "test_category".to_string(),
                PathBuf::from("test.rs"),
                1,
                "Test reason".to_string(),
                "test::module".to_string(),
                "test".to_string(),
            );
            collector.add_feedback(entry).unwrap();
        }

        // Load and verify
        {
            let collector = FeedbackCollector::with_store_path(store_path);
            assert_eq!(collector.store().entries.len(), 1);
            assert_eq!(collector.store().entries[0].pattern, "test_pattern");
        }
    }

    #[test]
    fn test_summary_report() {
        let mut store = FeedbackStore::new();

        for i in 0..5 {
            let entry = FeedbackEntry::new(
                "pattern_a".to_string(),
                "category_1".to_string(),
                PathBuf::from(format!("file{}.rs", i)),
                i,
                format!("Reason {}", i),
                "module::a".to_string(),
                "test".to_string(),
            );
            store.add(entry);
        }

        let report = store.summary_report();
        assert!(report.contains("Total entries: 5"));
        assert!(report.contains("pattern_a: 5 entries"));
    }
}
