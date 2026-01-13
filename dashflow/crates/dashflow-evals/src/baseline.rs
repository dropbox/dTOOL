//! Baseline storage and management for regression detection.
//!
//! Baselines are stored evaluation reports that serve as reference points for detecting
//! quality regressions. Each baseline is tagged with metadata like git commit hash,
//! timestamp, and author for traceability.

use crate::eval_runner::EvalReport;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use tracing::warn;

/// Metadata for a stored baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMetadata {
    /// Baseline name (e.g., "main", "v1.0.0", "pre-refactor")
    pub name: String,

    /// Git commit hash at time of baseline creation
    pub git_commit: Option<String>,

    /// Timestamp when baseline was created
    pub created_at: DateTime<Utc>,

    /// Author who created the baseline
    pub author: Option<String>,

    /// Description of this baseline
    pub description: Option<String>,

    /// Application name (e.g., "`librarian`")
    pub app_name: String,

    /// File path where baseline is stored
    pub file_path: PathBuf,

    /// Number of scenarios in the baseline
    pub scenario_count: usize,

    /// Average quality score in the baseline
    pub avg_quality: f64,

    /// Pass rate in the baseline
    pub pass_rate: f64,
}

/// Storage for evaluation baselines.
pub struct BaselineStore {
    /// Directory where baselines are stored
    storage_path: PathBuf,
}

impl BaselineStore {
    /// Create a new baseline store.
    ///
    /// # Arguments
    ///
    /// * `storage_path` - Directory where baselines will be stored
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_evals::BaselineStore;
    /// use std::path::Path;
    ///
    /// let store = BaselineStore::new(Path::new("baselines"));
    /// ```
    pub fn new(storage_path: impl AsRef<Path>) -> Self {
        Self {
            storage_path: storage_path.as_ref().to_path_buf(),
        }
    }

    /// Save an evaluation report as a new baseline.
    ///
    /// # Arguments
    ///
    /// * `name` - Baseline name (e.g., "main", "v1.0.0")
    /// * `report` - Evaluation report to save
    /// * `git_commit` - Optional git commit hash
    /// * `author` - Optional author name
    /// * `description` - Optional description
    /// * `app_name` - Application name
    ///
    /// # Returns
    ///
    /// Returns the metadata for the saved baseline.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::{BaselineStore, EvalReport, EvalMetadata};
    /// # use chrono::Utc;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let report = EvalReport {
    /// #     total: 50,
    /// #     passed: 48,
    /// #     failed: 2,
    /// #     results: vec![],
    /// #     metadata: EvalMetadata {
    /// #         started_at: Utc::now(),
    /// #         completed_at: Utc::now(),
    /// #         duration_secs: 120.5,
    /// #         config: "{}".to_string(),
    /// #     },
    /// # };
    /// let store = BaselineStore::new("baselines");
    /// let metadata = store.save_baseline(
    ///     "main",
    ///     &report,
    ///     Some("abc123"),
    ///     Some("Alice"),
    ///     Some("Baseline after refactor"),
    ///     "librarian",
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_baseline(
        &self,
        name: &str,
        report: &EvalReport,
        git_commit: Option<&str>,
        author: Option<&str>,
        description: Option<&str>,
        app_name: &str,
    ) -> Result<BaselineMetadata> {
        // Create storage directory if it doesn't exist
        fs::create_dir_all(&self.storage_path)
            .context("Failed to create baseline storage directory")?;

        // Generate filename: <app_name>_<name>_<timestamp>.json
        let timestamp = Utc::now();
        let filename = format!(
            "{}_{}_{}",
            app_name,
            name.replace(['/', ' '], "_"),
            timestamp.format("%Y%m%d_%H%M%S")
        );
        let file_path = self.storage_path.join(format!("{filename}.json"));

        // Create metadata
        let metadata = BaselineMetadata {
            name: name.to_string(),
            git_commit: git_commit.map(String::from),
            created_at: timestamp,
            author: author.map(String::from),
            description: description.map(String::from),
            app_name: app_name.to_string(),
            file_path: file_path.clone(),
            scenario_count: report.total,
            avg_quality: report.avg_quality(),
            pass_rate: report.pass_rate(),
        };

        // Create wrapper structure for storage
        let stored_baseline = StoredBaseline {
            metadata: metadata.clone(),
            report: report.clone(),
        };

        // Write to file
        let file = File::create(&file_path)
            .context(format!("Failed to create baseline file: {file_path:?}"))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &stored_baseline)
            .context("Failed to serialize baseline")?;

        Ok(metadata)
    }

    /// Load a baseline by name.
    ///
    /// If multiple baselines exist with the same name, returns the most recent one.
    ///
    /// # Arguments
    ///
    /// * `name` - Baseline name to load
    /// * `app_name` - Application name
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::BaselineStore;
    /// # async fn example() -> anyhow::Result<()> {
    /// let store = BaselineStore::new("baselines");
    /// let report = store.load_baseline("main", "librarian")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_baseline(&self, name: &str, app_name: &str) -> Result<EvalReport> {
        let stored = self.load_baseline_with_metadata(name, app_name)?;
        Ok(stored.report)
    }

    /// Load a baseline with its metadata.
    ///
    /// If multiple baselines exist with the same name, returns the most recent one.
    fn load_baseline_with_metadata(&self, name: &str, app_name: &str) -> Result<StoredBaseline> {
        let baselines = self.list_baselines(app_name)?;

        // Find all baselines with matching name
        let matching: Vec<_> = baselines.into_iter().filter(|m| m.name == name).collect();

        if matching.is_empty() {
            anyhow::bail!("No baseline found with name '{name}' for app '{app_name}'");
        }

        // Return the most recent one
        let most_recent = matching.into_iter().max_by_key(|m| m.created_at).unwrap(); // Safe: we know matching is not empty

        // Load from file
        self.load_baseline_from_path(&most_recent.file_path)
    }

    /// Load a baseline from a specific file path.
    fn load_baseline_from_path(&self, path: &Path) -> Result<StoredBaseline> {
        let file = File::open(path).context(format!("Failed to open baseline file: {path:?}"))?;
        let reader = BufReader::new(file);
        let stored: StoredBaseline =
            serde_json::from_reader(reader).context("Failed to deserialize baseline")?;
        Ok(stored)
    }

    /// List all available baselines for an application.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::BaselineStore;
    /// # async fn example() -> anyhow::Result<()> {
    /// let store = BaselineStore::new("baselines");
    /// let baselines = store.list_baselines("librarian")?;
    /// for baseline in baselines {
    ///     println!("{}: {} scenarios, {:.3} quality",
    ///              baseline.name, baseline.scenario_count, baseline.avg_quality);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_baselines(&self, app_name: &str) -> Result<Vec<BaselineMetadata>> {
        if !self.storage_path.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&self.storage_path)
            .context("Failed to read baseline storage directory")?;

        let mut baselines = Vec::new();

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip non-JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Skip files that don't match the app name
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                if !filename.starts_with(&format!("{app_name}_")) {
                    continue;
                }
            }

            // Try to load metadata
            match self.load_baseline_from_path(&path) {
                Ok(stored) => baselines.push(stored.metadata),
                Err(e) => {
                    // Log error but don't fail the entire listing
                    warn!(path = ?path, error = %e, "Failed to load baseline");
                }
            }
        }

        // Sort by creation time (most recent first)
        baselines.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(baselines)
    }

    /// Delete a baseline by name.
    ///
    /// If multiple baselines exist with the same name, deletes only the most recent one.
    pub fn delete_baseline(&self, name: &str, app_name: &str) -> Result<()> {
        let baselines = self.list_baselines(app_name)?;

        let matching: Vec<_> = baselines.into_iter().filter(|m| m.name == name).collect();

        if matching.is_empty() {
            anyhow::bail!("No baseline found with name '{name}' for app '{app_name}'");
        }

        // Delete the most recent one
        let to_delete = matching.into_iter().max_by_key(|m| m.created_at).unwrap(); // Safe: we know matching is not empty

        fs::remove_file(&to_delete.file_path).context(format!(
            "Failed to delete baseline file: {:?}",
            to_delete.file_path
        ))?;

        Ok(())
    }

    /// Get the current git commit hash (if in a git repository).
    #[must_use]
    pub fn current_git_commit() -> Option<String> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()?;

        if output.status.success() {
            let commit = String::from_utf8(output.stdout).ok()?;
            Some(commit.trim().to_string())
        } else {
            None
        }
    }

    /// Get the current git author (if in a git repository).
    #[must_use]
    pub fn current_git_author() -> Option<String> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["config", "user.name"])
            .output()
            .ok()?;

        if output.status.success() {
            let author = String::from_utf8(output.stdout).ok()?;
            Some(author.trim().to_string())
        } else {
            None
        }
    }
}

/// Internal structure for storing baseline with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredBaseline {
    metadata: BaselineMetadata,
    report: EvalReport,
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::EvalMetadata;
    use tempfile::TempDir;

    fn create_test_report() -> EvalReport {
        EvalReport {
            total: 50,
            passed: 48,
            failed: 2,
            results: vec![],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 120.5,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_save_and_load_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let report = create_test_report();

        // Save baseline
        let metadata = store
            .save_baseline(
                "main",
                &report,
                Some("abc123"),
                Some("Alice"),
                Some("Test baseline"),
                "test_app",
            )
            .unwrap();

        assert_eq!(metadata.name, "main");
        assert_eq!(metadata.git_commit, Some("abc123".to_string()));
        assert_eq!(metadata.author, Some("Alice".to_string()));
        assert_eq!(metadata.app_name, "test_app");
        assert_eq!(metadata.scenario_count, 50);

        // Load baseline
        let loaded = store.load_baseline("main", "test_app").unwrap();
        assert_eq!(loaded.total, report.total);
        assert_eq!(loaded.passed, report.passed);
        assert_eq!(loaded.failed, report.failed);
    }

    #[test]
    fn test_list_baselines() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let report = create_test_report();

        // Save multiple baselines
        store
            .save_baseline("main", &report, Some("abc123"), None, None, "test_app")
            .unwrap();
        store
            .save_baseline("v1.0.0", &report, Some("def456"), None, None, "test_app")
            .unwrap();
        store
            .save_baseline("main", &report, Some("abc456"), None, None, "other_app")
            .unwrap();

        // List baselines for test_app
        let baselines = store.list_baselines("test_app").unwrap();
        assert_eq!(baselines.len(), 2);

        let names: Vec<_> = baselines.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"v1.0.0"));

        // List baselines for other_app
        let baselines = store.list_baselines("other_app").unwrap();
        assert_eq!(baselines.len(), 1);
        assert_eq!(baselines[0].name, "main");
    }

    #[test]
    fn test_load_most_recent_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let mut report1 = create_test_report();
        report1.passed = 45;

        let mut report2 = create_test_report();
        report2.passed = 48;

        // Save two baselines with same name
        store
            .save_baseline("main", &report1, Some("old"), None, None, "test_app")
            .unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        store
            .save_baseline("main", &report2, Some("new"), None, None, "test_app")
            .unwrap();

        // Load should return the most recent one
        let loaded = store.load_baseline("main", "test_app").unwrap();
        assert_eq!(loaded.passed, 48); // report2

        // Check metadata
        let stored = store
            .load_baseline_with_metadata("main", "test_app")
            .unwrap();
        assert_eq!(stored.metadata.git_commit, Some("new".to_string()));
    }

    #[test]
    fn test_delete_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let report = create_test_report();

        store
            .save_baseline("main", &report, None, None, None, "test_app")
            .unwrap();

        // Verify baseline exists
        let baselines = store.list_baselines("test_app").unwrap();
        assert_eq!(baselines.len(), 1);

        // Delete baseline
        store.delete_baseline("main", "test_app").unwrap();

        // Verify baseline is gone
        let baselines = store.list_baselines("test_app").unwrap();
        assert_eq!(baselines.len(), 0);
    }

    #[test]
    fn test_load_nonexistent_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let result = store.load_baseline("nonexistent", "test_app");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No baseline found"));
    }

    #[test]
    fn test_baseline_metadata_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let store = BaselineStore::new(temp_dir.path());

        let report = create_test_report(); // 48/50 = 0.96 pass rate

        let metadata = store
            .save_baseline("main", &report, None, None, None, "test_app")
            .unwrap();

        assert_eq!(metadata.scenario_count, 50);
        assert!((metadata.pass_rate - 0.96).abs() < 0.001);
    }
}
