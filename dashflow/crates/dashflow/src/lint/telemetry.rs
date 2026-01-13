//! Opt-in telemetry for platform usage linter
//!
//! Collects anonymous, aggregated reports about lint patterns to help the DashFlow
//! team understand which platform features are being reimplemented and why.
//!
//! **Privacy-first design:**
//! - Disabled by default
//! - Must explicitly opt-in via config or env var
//! - No source code, file paths, or personal data transmitted
//! - Only aggregated pattern counts and anonymous feedback summaries
//!
//! # Usage
//!
//! ```bash
//! # Enable telemetry for this session
//! dashflow lint --enable-telemetry src/
//!
//! # Enable globally via config
//! dashflow config set lint.telemetry.enabled true
//!
//! # Or via environment variable
//! DASHFLOW_LINT_TELEMETRY=1 dashflow lint src/
//!
//! # View pending telemetry report (not sent until you confirm)
//! dashflow lint telemetry preview
//!
//! # Send telemetry report manually
//! dashflow lint telemetry send
//! ```

use super::patterns::{LintError, LintResult};
use crate::core::config_loader::env_vars::{env_string, DASHFLOW_LINT_TELEMETRY};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TelemetryConfig {
    /// Whether telemetry is enabled (default: false)
    pub enabled: bool,

    /// Send reports automatically (default: false, requires manual send)
    pub auto_send: bool,

    /// Anonymous installation ID (generated on first use)
    pub installation_id: Option<String>,

    /// Last time telemetry was sent
    pub last_sent: Option<DateTime<Utc>>,

    /// Minimum number of lint runs before sending (privacy)
    pub min_runs_before_send: usize,

    /// Path to telemetry data file
    #[serde(skip)]
    pub data_path: PathBuf,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_send: false,
            installation_id: None,
            last_sent: None,
            min_runs_before_send: 10,
            data_path: PathBuf::from(".dashflow/telemetry/lint_telemetry.json"),
        }
    }
}

impl TelemetryConfig {
    /// Load config from file or create default
    pub fn load() -> Self {
        let config_path = Self::default_config_path();
        Self::load_from(&config_path).unwrap_or_default()
    }

    /// Load from a specific path
    pub fn load_from(path: &Path) -> Option<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Save config to file
    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::default_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self)?;
        std::fs::write(&config_path, json)
    }

    /// Get default config path
    pub fn default_config_path() -> PathBuf {
        PathBuf::from(".dashflow/config/lint_telemetry.json")
    }

    /// Check if telemetry is enabled (respects env var override)
    pub fn is_enabled(&self) -> bool {
        // Environment variable override
        if let Some(val) = env_string(DASHFLOW_LINT_TELEMETRY) {
            return val == "1" || val.to_lowercase() == "true";
        }
        self.enabled
    }

    /// Get or generate installation ID
    pub fn get_or_create_installation_id(&mut self) -> String {
        if let Some(ref id) = self.installation_id {
            id.clone()
        } else {
            let id = generate_anonymous_id();
            self.installation_id = Some(id.clone());
            // Best-effort save - log warning but don't fail ID generation
            if let Err(e) = self.save() {
                tracing::warn!("Failed to persist telemetry installation ID: {}", e);
            }
            id
        }
    }
}

/// Anonymous, aggregated telemetry report
///
/// This is the ONLY data that can be transmitted. It contains:
/// - Aggregated pattern counts (no file paths)
/// - Anonymous feedback summaries (no raw text)
/// - Usage statistics (lint runs, warnings)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct TelemetryReport {
    /// Report version (for schema evolution)
    pub version: String,

    /// Anonymous installation ID
    pub installation_id: String,

    /// Report timestamp
    pub generated_at: DateTime<Utc>,

    /// Number of lint runs included in this report
    pub lint_runs: usize,

    /// Aggregated pattern match counts
    pub pattern_counts: HashMap<String, PatternTelemetry>,

    /// Aggregated category statistics
    pub category_stats: HashMap<String, CategoryTelemetry>,

    /// Feedback summary (no raw text)
    pub feedback_summary: FeedbackTelemetry,

    /// DashFlow version used
    pub dashflow_version: String,
}

/// Aggregated statistics for a single pattern
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct PatternTelemetry {
    /// How many times this pattern was matched
    pub match_count: usize,

    /// How many times it was suppressed
    pub suppression_count: usize,

    /// How many feedback entries mentioned this pattern
    pub feedback_count: usize,

    /// Common feedback categories (e.g., "api_mismatch", "missing_feature")
    pub feedback_categories: HashMap<String, usize>,
}

/// Aggregated statistics for a category
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct CategoryTelemetry {
    /// Total matches in this category
    pub total_matches: usize,

    /// Patterns in this category
    pub patterns: Vec<String>,
}

/// Aggregated feedback summary (no raw text)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct FeedbackTelemetry {
    /// Total feedback entries
    pub total_entries: usize,

    /// Entries with enhancement suggestions
    pub enhancement_suggestions: usize,

    /// Common feedback themes (classified, not raw)
    pub themes: HashMap<String, usize>,
}

/// Telemetry collector for lint runs
pub struct TelemetryCollector {
    config: TelemetryConfig,
    data: TelemetryData,
}

/// Accumulated telemetry data (not yet aggregated into report)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct TelemetryData {
    /// Number of lint runs since last report
    pub lint_runs: usize,

    /// Pattern match counts
    pub pattern_matches: HashMap<String, usize>,

    /// Pattern suppression counts
    pub pattern_suppressions: HashMap<String, usize>,

    /// Category match counts
    pub category_matches: HashMap<String, usize>,

    /// Feedback entries received
    pub feedback_entries: Vec<AnonymizedFeedback>,

    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Anonymized feedback entry for telemetry (no file paths, no raw text)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AnonymizedFeedback {
    /// Pattern name
    pub pattern: String,

    /// Category
    pub category: String,

    /// Classified reason theme (not raw text)
    pub reason_theme: String,

    /// Has enhancement suggestion
    pub has_enhancement: bool,
}

impl AnonymizedFeedback {
    /// Create a new anonymized feedback entry
    pub fn new(
        pattern: impl Into<String>,
        category: impl Into<String>,
        reason_theme: impl Into<String>,
        has_enhancement: bool,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            category: category.into(),
            reason_theme: reason_theme.into(),
            has_enhancement,
        }
    }
}

impl TelemetryCollector {
    /// Create a new collector
    pub fn new() -> Self {
        let config = TelemetryConfig::load();
        let data = Self::load_data(&config.data_path).unwrap_or_default();
        Self { config, data }
    }

    /// Create with explicit config
    #[must_use]
    pub fn with_config(config: TelemetryConfig) -> Self {
        let data = Self::load_data(&config.data_path).unwrap_or_default();
        Self { config, data }
    }

    /// Load accumulated data from disk
    fn load_data(path: &Path) -> Option<TelemetryData> {
        if path.exists() {
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Save accumulated data to disk
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config.data_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.config.data_path, json)
    }

    /// Record a lint run
    pub fn record_lint_run(&mut self) {
        if !self.config.is_enabled() {
            return;
        }

        self.data.lint_runs += 1;
        self.data.last_updated = Utc::now();
    }

    /// Record a pattern match
    pub fn record_pattern_match(&mut self, pattern: &str, category: &str) {
        if !self.config.is_enabled() {
            return;
        }

        *self
            .data
            .pattern_matches
            .entry(pattern.to_string())
            .or_default() += 1;
        *self
            .data
            .category_matches
            .entry(category.to_string())
            .or_default() += 1;
    }

    /// Record a pattern suppression
    pub fn record_suppression(&mut self, pattern: &str) {
        if !self.config.is_enabled() {
            return;
        }

        *self
            .data
            .pattern_suppressions
            .entry(pattern.to_string())
            .or_default() += 1;
    }

    /// Record feedback (anonymized)
    pub fn record_feedback(
        &mut self,
        pattern: &str,
        category: &str,
        reason: &str,
        has_enhancement: bool,
    ) {
        if !self.config.is_enabled() {
            return;
        }

        let reason_theme = classify_feedback_theme(reason);

        self.data.feedback_entries.push(AnonymizedFeedback::new(
            pattern,
            category,
            reason_theme,
            has_enhancement,
        ));
    }

    /// Check if ready to generate report (enough data accumulated)
    pub fn is_ready_for_report(&self) -> bool {
        self.data.lint_runs >= self.config.min_runs_before_send
    }

    /// Generate aggregated report
    pub fn generate_report(&mut self) -> TelemetryReport {
        let mut report = TelemetryReport {
            version: "1.0".to_string(),
            installation_id: self.config.get_or_create_installation_id(),
            generated_at: Utc::now(),
            lint_runs: self.data.lint_runs,
            dashflow_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        };

        // Aggregate pattern statistics
        for (pattern, count) in &self.data.pattern_matches {
            let mut pattern_telemetry = PatternTelemetry {
                match_count: *count,
                suppression_count: *self.data.pattern_suppressions.get(pattern).unwrap_or(&0),
                ..Default::default()
            };

            // Count feedback for this pattern
            for feedback in &self.data.feedback_entries {
                if &feedback.pattern == pattern {
                    pattern_telemetry.feedback_count += 1;
                    *pattern_telemetry
                        .feedback_categories
                        .entry(feedback.reason_theme.clone())
                        .or_default() += 1;
                }
            }

            report
                .pattern_counts
                .insert(pattern.clone(), pattern_telemetry);
        }

        // Aggregate category statistics
        for (category, count) in &self.data.category_matches {
            let patterns: Vec<String> = self
                .data
                .pattern_matches
                .keys()
                .filter(|p| {
                    self.data
                        .feedback_entries
                        .iter()
                        .any(|f| &f.category == category && f.pattern == **p)
                })
                .cloned()
                .collect();

            report.category_stats.insert(
                category.clone(),
                CategoryTelemetry {
                    total_matches: *count,
                    patterns,
                },
            );
        }

        // Aggregate feedback summary
        report.feedback_summary.total_entries = self.data.feedback_entries.len();
        report.feedback_summary.enhancement_suggestions = self
            .data
            .feedback_entries
            .iter()
            .filter(|f| f.has_enhancement)
            .count();

        for feedback in &self.data.feedback_entries {
            *report
                .feedback_summary
                .themes
                .entry(feedback.reason_theme.clone())
                .or_default() += 1;
        }

        report
    }

    /// Clear accumulated data after successful send
    pub fn clear_data(&mut self) -> std::io::Result<()> {
        self.data = TelemetryData::default();
        self.data.last_updated = Utc::now();
        self.config.last_sent = Some(Utc::now());
        self.config.save()?;
        self.save()
    }

    /// Get preview of what would be sent (as JSON)
    pub fn preview_report(&mut self) -> String {
        let report = self.generate_report();
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate anonymous installation ID
fn generate_anonymous_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Use hash of timestamp + random component for anonymity
    let random_part: u64 = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();

    format!("anon-{:016x}{:016x}", timestamp as u64, random_part)
}

use std::hash::{BuildHasher, Hasher};

/// Classify feedback reason into anonymous theme
fn classify_feedback_theme(reason: &str) -> String {
    let reason_lower = reason.to_lowercase();

    // Classify based on keywords (no raw text transmitted)
    if reason_lower.contains("api")
        || reason_lower.contains("interface")
        || reason_lower.contains("signature")
    {
        "api_mismatch".to_string()
    } else if reason_lower.contains("missing")
        || reason_lower.contains("doesn't support")
        || reason_lower.contains("not support")
    {
        "missing_feature".to_string()
    } else if reason_lower.contains("performance")
        || reason_lower.contains("slow")
        || reason_lower.contains("fast")
    {
        "performance".to_string()
    } else if reason_lower.contains("complex")
        || reason_lower.contains("simple")
        || reason_lower.contains("easy")
    {
        "complexity".to_string()
    } else if reason_lower.contains("document")
        || reason_lower.contains("example")
        || reason_lower.contains("unclear")
    {
        "documentation".to_string()
    } else if reason_lower.contains("bug")
        || reason_lower.contains("broken")
        || reason_lower.contains("error")
    {
        "bug".to_string()
    } else if reason_lower.contains("specific")
        || reason_lower.contains("custom")
        || reason_lower.contains("unique")
    {
        "specific_requirements".to_string()
    } else {
        "other".to_string()
    }
}

/// Telemetry report destination (placeholder for future implementation)
#[derive(Debug, Clone)]
pub enum ReportDestination {
    /// Save locally for manual review
    LocalFile(PathBuf),
    /// Send to DashFlow telemetry endpoint.
    /// Planned for centralized telemetry aggregation; requires backend service.
    #[allow(dead_code)] // Architectural: Planned for centralized telemetry service
    DashFlowEndpoint,
}

impl Default for ReportDestination {
    fn default() -> Self {
        Self::LocalFile(PathBuf::from(".dashflow/telemetry/pending_report.json"))
    }
}

/// Send a telemetry report
///
/// Currently saves to local file. Future: HTTP POST to telemetry endpoint.
pub fn send_report(report: &TelemetryReport, destination: ReportDestination) -> LintResult<()> {
    match destination {
        ReportDestination::LocalFile(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(report)?;
            std::fs::write(&path, json)?;
            tracing::info!("Telemetry report saved to {}", path.display());
            Ok(())
        }
        ReportDestination::DashFlowEndpoint => {
            // Return error instead of silently succeeding
            Err(LintError::Other(
                "Remote telemetry endpoint not yet implemented. Use LocalFile destination instead."
                    .to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Mutex to serialize env-var-dependent tests (parallel execution causes races)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_telemetry_disabled_by_default() {
        let config = TelemetryConfig::default();
        assert!(!config.enabled);
        assert!(!config.auto_send);
    }

    #[test]
    fn test_telemetry_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_LINT_TELEMETRY", "1");
        let config = TelemetryConfig::default();
        let is_enabled = config.is_enabled();
        std::env::remove_var("DASHFLOW_LINT_TELEMETRY");
        assert!(is_enabled);
    }

    #[test]
    fn test_classify_feedback_theme() {
        assert_eq!(classify_feedback_theme("API doesn't match"), "api_mismatch");
        assert_eq!(
            classify_feedback_theme("Missing feature X"),
            "missing_feature"
        );
        assert_eq!(
            classify_feedback_theme("Too slow for my use"),
            "performance"
        );
        assert_eq!(
            classify_feedback_theme("Need a custom solution"),
            "specific_requirements"
        );
        assert_eq!(classify_feedback_theme("Something random"), "other");
    }

    #[test]
    fn test_telemetry_collector() {
        let dir = tempdir().unwrap();
        let data_path = dir.path().join("telemetry.json");

        let mut config = TelemetryConfig::default();
        config.enabled = true;
        config.data_path = data_path;
        config.min_runs_before_send = 2;

        let mut collector = TelemetryCollector::with_config(config);

        // Record data
        collector.record_lint_run();
        collector.record_pattern_match("cost_tracking", "observability");
        collector.record_pattern_match("cost_tracking", "observability");
        collector.record_suppression("cost_tracking");
        collector.record_feedback(
            "cost_tracking",
            "observability",
            "API doesn't support X",
            true,
        );
        collector.record_lint_run();

        // Should be ready after 2 runs
        assert!(collector.is_ready_for_report());

        // Generate report
        let report = collector.generate_report();
        assert_eq!(report.lint_runs, 2);
        assert!(report.pattern_counts.contains_key("cost_tracking"));

        let pattern_stats = &report.pattern_counts["cost_tracking"];
        assert_eq!(pattern_stats.match_count, 2);
        assert_eq!(pattern_stats.suppression_count, 1);
        assert_eq!(pattern_stats.feedback_count, 1);
    }

    #[test]
    fn test_generate_anonymous_id() {
        let id1 = generate_anonymous_id();
        let id2 = generate_anonymous_id();

        assert!(id1.starts_with("anon-"));
        assert_ne!(id1, id2); // Should be unique
    }
}
