// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Baseline Storage
//!
//! Stores and loads baseline metrics for comparison.
//!
//! Baselines are stored as JSON files with metadata (app name, version, date)
//! and metrics (`EvalMetrics` struct).

use super::metrics::EvalMetrics;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Baseline metrics for an application
///
/// # Examples
///
/// ```no_run
/// use dashflow_streaming::evals::{Baseline, EvalMetrics};
///
/// // Create baseline
/// let baseline = Baseline {
///     app_name: "librarian".to_string(),
///     version: "1.0.0".to_string(),
///     date: "2025-11-12T07:15:00Z".to_string(),
///     metrics: EvalMetrics::default(),
/// };
///
/// // Save to file
/// baseline.save("baselines/librarian_v1.0.0.json").unwrap();
///
/// // Load from file
/// let loaded = Baseline::load("baselines/librarian_v1.0.0.json").unwrap();
/// assert_eq!(loaded.app_name, "librarian");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Application name (e.g., "`librarian`")
    pub app_name: String,

    /// Version (e.g., "1.0.0", "1.1.0")
    pub version: String,

    /// Date when baseline was created (ISO 8601 format)
    pub date: String,

    /// Evaluation metrics for this baseline
    pub metrics: EvalMetrics,
}

impl Baseline {
    /// Create new baseline with current timestamp
    ///
    /// # Arguments
    ///
    /// * `app_name` - Application name
    /// * `version` - Version string
    /// * `metrics` - Evaluation metrics
    ///
    /// # Returns
    ///
    /// New baseline with current UTC timestamp
    #[must_use]
    pub fn new(app_name: String, version: String, metrics: EvalMetrics) -> Self {
        let date = chrono::Utc::now().to_rfc3339();
        Self {
            app_name,
            version,
            date,
            metrics,
        }
    }

    /// Load baseline from JSON file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to baseline JSON file
    ///
    /// # Returns
    ///
    /// Loaded baseline, or error if file doesn't exist or is invalid
    ///
    /// # Errors
    ///
    /// - File not found
    /// - Invalid JSON format
    /// - Missing required fields
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read baseline file: {}", path.display()))?;

        let baseline: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse baseline JSON: {}", path.display()))?;

        Ok(baseline)
    }

    /// Save baseline to JSON file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save baseline JSON file
    ///
    /// # Returns
    ///
    /// Ok(()) on success, error on failure
    ///
    /// # Errors
    ///
    /// - Cannot create parent directory
    /// - Cannot write file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize baseline to JSON")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write baseline file: {}", path.display()))?;

        Ok(())
    }

    /// Get baseline file path for app name and version
    ///
    /// # Arguments
    ///
    /// * `app_name` - Application name
    /// * `version` - Version string
    ///
    /// # Returns
    ///
    /// Path string like "`baselines/{app_name`}_v{version}.json"
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_streaming::evals::Baseline;
    ///
    /// let path = Baseline::path("librarian", "1.0.0");
    /// assert_eq!(path, "baselines/librarian_v1.0.0.json");
    /// ```
    #[must_use]
    pub fn path(app_name: &str, version: &str) -> String {
        format!("baselines/{app_name}_v{version}.json")
    }

    /// Check if baseline file exists
    ///
    /// # Arguments
    ///
    /// * `path` - Path to baseline file
    ///
    /// # Returns
    ///
    /// true if file exists, false otherwise
    pub fn exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_baseline_new() {
        let metrics = EvalMetrics::default();
        let baseline = Baseline::new("librarian".to_string(), "1.0.0".to_string(), metrics);

        assert_eq!(baseline.app_name, "librarian");
        assert_eq!(baseline.version, "1.0.0");
        assert!(!baseline.date.is_empty());
    }

    #[test]
    fn test_baseline_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_baseline.json");

        let metrics = EvalMetrics {
            correctness: Some(0.95),
            p95_latency: 1850.0,
            total_tokens: 150,
            ..Default::default()
        };

        let baseline = Baseline::new("librarian".to_string(), "1.0.0".to_string(), metrics);

        // Save
        baseline.save(&path).unwrap();

        // Load
        let loaded = Baseline::load(&path).unwrap();

        assert_eq!(loaded.app_name, baseline.app_name);
        assert_eq!(loaded.version, baseline.version);
        assert_eq!(loaded.metrics.correctness, baseline.metrics.correctness);
        assert_eq!(loaded.metrics.p95_latency, baseline.metrics.p95_latency);
        assert_eq!(loaded.metrics.total_tokens, baseline.metrics.total_tokens);
    }

    #[test]
    fn test_baseline_path() {
        let path = Baseline::path("librarian", "1.0.0");
        assert_eq!(path, "baselines/librarian_v1.0.0.json");
    }

    #[test]
    fn test_baseline_exists() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_baseline.json");

        assert!(!Baseline::exists(&path));

        let baseline = Baseline::new(
            "test".to_string(),
            "1.0.0".to_string(),
            EvalMetrics::default(),
        );
        baseline.save(&path).unwrap();

        assert!(Baseline::exists(&path));
    }

    #[test]
    fn test_baseline_load_nonexistent() {
        let result = Baseline::load("/nonexistent/path.json");
        assert!(result.is_err());
    }
}
