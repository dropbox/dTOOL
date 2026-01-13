// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Storage system for introspection data.
//!
//! All introspection data is stored in `.dashflow/introspection/` for history tracking.
//! The storage structure follows this layout:
//!
//! ```text
//! .dashflow/
//! └── introspection/
//!     ├── reports/
//!     │   ├── 2025-12-09T15-30-00_abc123.md    # Human-readable report
//!     │   ├── 2025-12-09T15-30-00_abc123.json  # Machine-readable data
//!     │   └── ...
//!     ├── plans/
//!     │   ├── pending/
//!     │   │   └── plan_001.json
//!     │   ├── approved/
//!     │   │   └── plan_000.json
//!     │   ├── implemented/
//!     │   │   └── ...
//!     │   └── failed/
//!     │       └── ...
//!     ├── hypotheses/
//!     │   ├── active/
//!     │   │   └── hyp_001.json
//!     │   └── evaluated/
//!     │       └── ...
//!     └── meta/
//!         ├── patterns.json
//!         └── momentum.json
//! ```

// Submodules
mod degraded;
mod schema;

#[cfg(test)]
mod tests;

// Re-exports from submodules
pub use degraded::*;
pub use schema::*;

use crate::core::config_loader::env_vars::{
    env_bool, env_u64, env_usize, DASHFLOW_STORAGE_CRITICAL_SIZE_MB,
    DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS, DASHFLOW_STORAGE_MAX_PLANS, DASHFLOW_STORAGE_MAX_REPORTS,
    DASHFLOW_STORAGE_PLAN_AGE_DAYS, DASHFLOW_STORAGE_PLAN_WARNING_COUNT,
    DASHFLOW_STORAGE_REPORT_WARNING_COUNT, DASHFLOW_STORAGE_RETENTION,
    DASHFLOW_STORAGE_WARNING_SIZE_MB,
};
use crate::self_improvement::metrics::{
    record_plan_approved, record_plan_failed, record_plan_implemented, record_storage_operation,
};
use crate::self_improvement::traits::Storable;
use crate::self_improvement::types::{
    ExecutionPlan, Hypothesis, HypothesisStatus, IntrospectionReport, PlanStatus,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::warn;
use uuid::Uuid;

/// Default base directory for introspection storage
pub const DEFAULT_INTROSPECTION_DIR: &str = ".dashflow/introspection";

// ============================================================================
// Type Aliases
// ============================================================================

/// Paths to saved report files (markdown path, JSON path).
pub type ReportPathPair = (PathBuf, PathBuf);

/// Error entry for batch report saving (report UUID, IO error).
pub type ReportSaveError = (Uuid, std::io::Error);

/// Result of batch report saving: (successful saves, errors).
pub type BatchReportResult = (Vec<ReportPathPair>, Vec<ReportSaveError>);

// ============================================================================
// Storage Limits
// ============================================================================

/// Default maximum number of reports to retain.
pub const DEFAULT_MAX_REPORTS: usize = 100;

/// Default maximum number of plans per status directory.
pub const DEFAULT_MAX_PLANS_PER_STATUS: usize = 200;

/// Default maximum age for implemented/failed plans (30 days).
pub const DEFAULT_PLAN_ARCHIVE_AGE_DAYS: u64 = 30;

/// Default maximum age for evaluated hypotheses (90 days).
pub const DEFAULT_HYPOTHESIS_ARCHIVE_AGE_DAYS: u64 = 90;

/// Default warning threshold for storage size (100 MB).
pub const DEFAULT_STORAGE_WARNING_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Default critical threshold for storage size (500 MB).
pub const DEFAULT_STORAGE_CRITICAL_SIZE_BYTES: u64 = 500 * 1024 * 1024;

/// Default warning threshold for report count.
pub const DEFAULT_REPORT_WARNING_COUNT: usize = 80;

/// Default warning threshold for plan count (per status dir).
pub const DEFAULT_PLAN_WARNING_COUNT: usize = 160;

/// Storage health status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageHealthLevel {
    /// Storage is healthy - under all thresholds.
    Healthy,
    /// Storage is approaching limits - warnings issued.
    Warning,
    /// Storage is at critical levels - immediate action needed.
    Critical,
}

/// Storage health check result.
#[derive(Debug, Clone)]
pub struct StorageHealthStatus {
    /// Overall health level.
    pub level: StorageHealthLevel,
    /// Current storage statistics.
    pub stats: StorageStats,
    /// List of warnings.
    pub warnings: Vec<String>,
    /// Whether cleanup is recommended.
    pub cleanup_recommended: bool,
}

impl StorageHealthStatus {
    /// Check if storage is healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.level == StorageHealthLevel::Healthy
    }

    /// Check if storage has warnings.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Storage policy configuration for retention limits.
///
/// Configured via environment variables:
/// - `DASHFLOW_STORAGE_MAX_REPORTS`: Max reports (default: 100)
/// - `DASHFLOW_STORAGE_MAX_PLANS`: Max plans per status (default: 200)
/// - `DASHFLOW_STORAGE_PLAN_AGE_DAYS`: Max age for archived plans (default: 30)
/// - `DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS`: Max age for evaluated hypotheses (default: 90)
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StoragePolicy {
    /// Maximum number of reports to keep (oldest deleted first).
    pub max_reports: Option<usize>,
    /// Maximum plans per status directory (pending, approved, implemented, failed).
    pub max_plans_per_status: Option<usize>,
    /// Maximum age for implemented/failed plans.
    pub plan_archive_age: Option<Duration>,
    /// Maximum age for evaluated hypotheses.
    pub hypothesis_archive_age: Option<Duration>,
    /// Whether policy enforcement is enabled.
    pub enabled: bool,
}

impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            max_reports: Some(DEFAULT_MAX_REPORTS),
            max_plans_per_status: Some(DEFAULT_MAX_PLANS_PER_STATUS),
            plan_archive_age: Some(Duration::from_secs(
                DEFAULT_PLAN_ARCHIVE_AGE_DAYS * 24 * 60 * 60,
            )),
            hypothesis_archive_age: Some(Duration::from_secs(
                DEFAULT_HYPOTHESIS_ARCHIVE_AGE_DAYS * 24 * 60 * 60,
            )),
            enabled: true,
        }
    }
}

impl StoragePolicy {
    /// Create a policy with no limits.
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            max_reports: None,
            max_plans_per_status: None,
            plan_archive_age: None,
            hypothesis_archive_age: None,
            enabled: true,
        }
    }

    /// Create policy from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = env_bool(DASHFLOW_STORAGE_RETENTION, true);
        let max_reports = Some(env_usize(DASHFLOW_STORAGE_MAX_REPORTS, DEFAULT_MAX_REPORTS));
        let max_plans_per_status = Some(env_usize(
            DASHFLOW_STORAGE_MAX_PLANS,
            DEFAULT_MAX_PLANS_PER_STATUS,
        ));
        let plan_age_days = env_u64(DASHFLOW_STORAGE_PLAN_AGE_DAYS, DEFAULT_PLAN_ARCHIVE_AGE_DAYS);
        let hypothesis_age_days = env_u64(
            DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS,
            DEFAULT_HYPOTHESIS_ARCHIVE_AGE_DAYS,
        );

        Self {
            max_reports,
            max_plans_per_status,
            plan_archive_age: Some(Duration::from_secs(plan_age_days * 24 * 60 * 60)),
            hypothesis_archive_age: Some(Duration::from_secs(hypothesis_age_days * 24 * 60 * 60)),
            enabled,
        }
    }

    /// Builder: set maximum reports.
    #[must_use]
    pub fn with_max_reports(mut self, count: usize) -> Self {
        self.max_reports = Some(count);
        self
    }

    /// Builder: set maximum plans per status.
    #[must_use]
    pub fn with_max_plans_per_status(mut self, count: usize) -> Self {
        self.max_plans_per_status = Some(count);
        self
    }

    /// Builder: set plan archive age in days.
    #[must_use]
    pub fn with_plan_archive_age_days(mut self, days: u64) -> Self {
        self.plan_archive_age = Some(Duration::from_secs(days * 24 * 60 * 60));
        self
    }

    /// Builder: set hypothesis archive age in days.
    #[must_use]
    pub fn with_hypothesis_archive_age_days(mut self, days: u64) -> Self {
        self.hypothesis_archive_age = Some(Duration::from_secs(days * 24 * 60 * 60));
        self
    }

    /// Builder: enable/disable policy.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Statistics about a storage cleanup operation.
#[derive(Debug, Clone, Default)]
pub struct StorageCleanupStats {
    /// Reports deleted.
    pub reports_deleted: usize,
    /// Plans deleted.
    pub plans_deleted: usize,
    /// Hypotheses deleted.
    pub hypotheses_deleted: usize,
    /// Total files deleted.
    pub total_deleted: usize,
    /// Total bytes freed.
    pub bytes_freed: u64,
    /// Errors encountered (non-fatal).
    pub errors: Vec<String>,
}

/// Statistics about the storage directory.
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// Total reports count.
    pub report_count: usize,
    /// Plans by status: (pending, approved, implemented, failed).
    pub plan_counts: (usize, usize, usize, usize),
    /// Hypotheses: (active, evaluated).
    pub hypothesis_counts: (usize, usize),
    /// Total storage size in bytes.
    pub total_size_bytes: u64,
}

// ============================================================================
// Plan Index
// ============================================================================

/// Index of plan IDs to their status directory for O(1) lookups.
///
/// Stored in `.dashflow/introspection/plans/index.json`.
/// Eliminates need to search all status directories when finding a plan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanIndex {
    /// Map of plan UUID to status directory name (pending, approved, etc.)
    entries: std::collections::HashMap<Uuid, String>,
    /// Last update timestamp (ISO 8601)
    last_updated: Option<String>,
}

impl PlanIndex {
    /// Create a new empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            last_updated: None,
        }
    }

    /// Get the status directory for a plan ID.
    #[must_use]
    pub fn get(&self, id: &Uuid) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    /// Insert or update a plan's status directory.
    pub fn insert(&mut self, id: Uuid, status_dir: impl Into<String>) {
        self.entries.insert(id, status_dir.into());
        self.last_updated = Some(Utc::now().to_rfc3339());
    }

    /// Remove a plan from the index.
    pub fn remove(&mut self, id: &Uuid) -> Option<String> {
        let result = self.entries.remove(id);
        if result.is_some() {
            self.last_updated = Some(Utc::now().to_rfc3339());
        }
        result
    }

    /// Get the number of indexed plans.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all plan IDs.
    #[must_use]
    pub fn ids(&self) -> Vec<Uuid> {
        self.entries.keys().copied().collect()
    }
}

/// Storage for introspection data.
///
/// The primary persistence layer for the self-improvement system. Stores reports,
/// plans, hypotheses, and meta-analysis data in a structured directory hierarchy
/// under `.dashflow/introspection/`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{
///     IntrospectionStorage, IntrospectionReport, IntrospectionScope,
///     ExecutionPlan, PlanStatus, StoragePolicy,
/// };
///
/// // Create storage (defaults to .dashflow/introspection/)
/// let storage = IntrospectionStorage::default();
///
/// // Initialize the directory structure
/// storage.initialize()?;
///
/// // Save a report
/// let report = IntrospectionReport::new(IntrospectionScope::System);
/// let (md_path, json_path) = storage.save_report(&report)?;
/// println!("Saved to: {}", json_path.display());
///
/// // Save a plan
/// let plan = ExecutionPlan::new("Fix bug", PlanCategory::ApplicationImprovement);
/// storage.save_plan(&plan)?;
///
/// // List pending plans
/// let pending = storage.list_plans(PlanStatus::Pending)?;
/// println!("Found {} pending plans", pending.len());
///
/// // Approve a plan
/// storage.update_plan_status(&plan.id, PlanStatus::Approved)?;
///
/// // Check storage health
/// let health = storage.check_health()?;
/// println!("Storage health: {:?}", health.level);
/// ```
///
/// # Storage Structure
///
/// ```text
/// .dashflow/introspection/
/// ├── reports/           # Analysis reports (JSON + markdown)
/// ├── plans/
/// │   ├── pending/       # Plans awaiting approval
/// │   ├── approved/      # Approved plans
/// │   ├── implemented/   # Completed plans
/// │   └── failed/        # Failed plans
/// ├── hypotheses/
/// │   ├── active/        # Active predictions
/// │   └── evaluated/     # Resolved predictions
/// └── meta/              # Pattern and momentum data
/// ```
///
/// # Errors
///
/// - [`std::io::Error`] - File system operations (create, read, write)
/// - [`serde_json::Error`] - JSON serialization/deserialization
/// - [`MigrationError`] - Schema version migration failures
///
/// # See Also
///
/// - [`StoragePolicy`] - Configure retention limits and cleanup
/// - [`IntrospectionReport`] - Reports to store
/// - [`ExecutionPlan`] - Plans to track
/// - [`StorageHealthStatus`] - Monitor storage health
#[derive(Debug, Clone)]
pub struct IntrospectionStorage {
    /// Base directory for storage
    base_dir: PathBuf,
    /// Retention policy
    policy: StoragePolicy,
    /// Whether to use versioned storage.
    /// When true, saved files include schema version.
    versioned: bool,
}

impl Default for IntrospectionStorage {
    fn default() -> Self {
        Self::new(DEFAULT_INTROSPECTION_DIR)
    }
}

impl IntrospectionStorage {
    /// Create a new storage instance with the given base directory
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            policy: StoragePolicy::default(),
            versioned: true, // Enable versioned storage by default
        }
    }

    /// Create a storage instance from an absolute path
    #[must_use]
    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: path.into(),
            policy: StoragePolicy::default(),
            versioned: true,
        }
    }

    /// Builder: enable or disable versioned storage.
    ///
    /// When enabled (default), saved files include `_schema_version` field.
    #[must_use]
    pub fn with_versioning(mut self, enabled: bool) -> Self {
        self.versioned = enabled;
        self
    }

    /// Check if versioned storage is enabled.
    #[must_use]
    pub fn is_versioned(&self) -> bool {
        self.versioned
    }

    /// Create storage with a custom policy.
    #[must_use]
    pub fn with_policy(mut self, policy: StoragePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Get the current storage policy.
    #[must_use]
    pub fn policy(&self) -> &StoragePolicy {
        &self.policy
    }

    /// Set the storage policy.
    pub fn set_policy(&mut self, policy: StoragePolicy) {
        self.policy = policy;
    }

    /// Get the base directory path
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Initialize the storage directory structure
    ///
    /// # Errors
    ///
    /// Returns error if directory creation fails
    pub fn initialize(&self) -> std::io::Result<()> {
        // Create all necessary subdirectories
        let dirs = [
            self.reports_dir(),
            self.plans_dir().join("pending"),
            self.plans_dir().join("approved"),
            self.plans_dir().join("implemented"),
            self.plans_dir().join("failed"),
            self.hypotheses_dir().join("active"),
            self.hypotheses_dir().join("evaluated"),
            self.meta_dir(),
        ];

        for dir in dirs {
            fs::create_dir_all(dir)?;
        }

        Ok(())
    }

    /// Check if storage is initialized
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.base_dir.exists() && self.reports_dir().exists()
    }

    // Directory accessors

    /// Get the reports directory path
    #[must_use]
    pub fn reports_dir(&self) -> PathBuf {
        self.base_dir.join("reports")
    }

    /// Get the plans directory path
    #[must_use]
    pub fn plans_dir(&self) -> PathBuf {
        self.base_dir.join("plans")
    }

    /// Get the hypotheses directory path
    #[must_use]
    pub fn hypotheses_dir(&self) -> PathBuf {
        self.base_dir.join("hypotheses")
    }

    /// Get the meta directory path
    #[must_use]
    pub fn meta_dir(&self) -> PathBuf {
        self.base_dir.join("meta")
    }

    // Plan Index Operations

    /// Get the plan index file path
    #[must_use]
    pub fn plan_index_path(&self) -> PathBuf {
        self.plans_dir().join("index.json")
    }

    /// Load the plan index from disk.
    ///
    /// If the index doesn't exist, returns an empty index.
    /// If the index is corrupted, rebuilds it from directory contents.
    pub fn load_plan_index(&self) -> std::io::Result<PlanIndex> {
        let path = self.plan_index_path();
        if !path.exists() {
            // No index yet - try to rebuild from existing files
            return self.rebuild_plan_index();
        }

        let content = fs::read_to_string(&path)?;
        match serde_json::from_str(&content) {
            Ok(index) => Ok(index),
            Err(_) => {
                // Index corrupted - rebuild
                self.rebuild_plan_index()
            }
        }
    }

    /// Save the plan index to disk.
    pub fn save_plan_index(&self, index: &PlanIndex) -> std::io::Result<()> {
        self.ensure_dir(&self.plans_dir())?;
        let path = self.plan_index_path();
        let json = serde_json::to_string_pretty(index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(&path, json)
    }

    /// Rebuild the plan index by scanning all status directories.
    ///
    /// This is O(n) where n is the total number of plans, but only
    /// needs to run on first use or if the index is corrupted.
    pub fn rebuild_plan_index(&self) -> std::io::Result<PlanIndex> {
        let mut index = PlanIndex::new();

        for subdir in ["pending", "approved", "implemented", "failed"] {
            let dir = self.plans_dir().join(subdir);
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    // Extract UUID from filename (format: <uuid>.json)
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(id) = Uuid::parse_str(stem) {
                            index.insert(id, subdir);
                        }
                    }
                }
            }
        }

        // Save the rebuilt index
        if let Err(e) = self.save_plan_index(&index) {
            warn!("Failed to save rebuilt plan index: {}", e);
        }

        Ok(index)
    }

    /// Update the plan index for a single plan.
    ///
    /// Called automatically when plans are saved or moved.
    fn update_plan_index(&self, id: Uuid, status_dir: &str) -> std::io::Result<()> {
        let mut index = self.load_plan_index()?;
        index.insert(id, status_dir);
        self.save_plan_index(&index)
    }

    /// Remove a plan from the index.
    ///
    /// Called when a plan is deleted.
    /// Currently unused - designed for future plan deletion functionality.
    #[allow(dead_code)] // Architectural: Ready for plan deletion feature
    fn remove_from_plan_index(&self, id: &Uuid) -> std::io::Result<()> {
        let mut index = self.load_plan_index()?;
        index.remove(id);
        self.save_plan_index(&index)
    }

    // ==========================================================================
    // Generic Storable Operations
    // ==========================================================================

    /// Save any item that implements the Storable trait.
    ///
    /// This generic method handles:
    /// - Status-based subdirectory routing
    /// - Versioned storage (when enabled)
    /// - Directory creation
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let storage = IntrospectionStorage::default();
    /// let plan = ExecutionPlan::new("My Plan", PlanCategory::Performance);
    /// let path = storage.save(&plan)?;
    /// ```
    pub fn save<T: Storable>(&self, item: &T) -> std::io::Result<PathBuf> {
        let base_dir = self.base_dir.join(T::storage_dir_name());

        let dir = if let Some(subdir) = item.status_subdir() {
            base_dir.join(subdir)
        } else {
            base_dir
        };

        self.ensure_dir(&dir)?;

        let path = dir.join(format!("{}.json", item.id()));
        let json = if self.versioned {
            SchemaMigrator::save_versioned(item)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(item)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&path, json)?;

        Ok(path)
    }

    /// Load any item that implements the Storable trait by ID.
    ///
    /// Searches through all status subdirectories to find the item.
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let storage = IntrospectionStorage::default();
    /// let plan: ExecutionPlan = storage.load(plan_id)?;
    /// ```
    pub fn load<T: Storable>(&self, id: Uuid) -> std::io::Result<T> {
        let base_dir = self.base_dir.join(T::storage_dir_name());
        let filename = format!("{id}.json");

        // Search in all subdirectories
        for subdir in T::search_subdirs() {
            let path = base_dir.join(subdir).join(&filename);
            if path.exists() {
                let contents = fs::read_to_string(path)?;
                return self.parse_item(&contents);
            }
        }

        // Also check the base directory (for items without status subdirs)
        let path = base_dir.join(&filename);
        if path.exists() {
            let contents = fs::read_to_string(path)?;
            return self.parse_item(&contents);
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} {} not found", T::entity_name(), id),
        ))
    }

    /// Parse a storable item from JSON, handling versioned data and migration.
    fn parse_item<T: Storable>(&self, contents: &str) -> std::io::Result<T> {
        if self.versioned {
            let migrator = SchemaMigrator::new();
            let result = migrator
                .load_versioned::<T>(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            Ok(result.data)
        } else {
            serde_json::from_str(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }
    }

    /// Save any storable item asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    pub async fn save_async<T: Storable>(&self, item: &T) -> std::io::Result<PathBuf> {
        let base_dir = self.base_dir.join(T::storage_dir_name());

        let dir = if let Some(subdir) = item.status_subdir() {
            base_dir.join(subdir)
        } else {
            base_dir
        };

        self.ensure_dir_async(&dir).await?;

        let path = dir.join(format!("{}.json", item.id()));
        let json = if self.versioned {
            SchemaMigrator::save_versioned(item)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(item)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        tokio::fs::write(&path, json).await?;

        Ok(path)
    }

    /// Load any storable item asynchronously by ID.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails.
    pub async fn load_async<T: Storable>(&self, id: Uuid) -> std::io::Result<T> {
        let base_dir = self.base_dir.join(T::storage_dir_name());
        let filename = format!("{id}.json");

        // Search in all subdirectories
        for subdir in T::search_subdirs() {
            let path = base_dir.join(subdir).join(&filename);
            if path.exists() {
                let contents = tokio::fs::read_to_string(&path).await?;
                return self.parse_item(&contents);
            }
        }

        // Also check the base directory
        let path = base_dir.join(&filename);
        if path.exists() {
            let contents = tokio::fs::read_to_string(&path).await?;
            return self.parse_item(&contents);
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} {} not found", T::entity_name(), id),
        ))
    }

    /// List all items of a storable type.
    ///
    /// Returns items from all status subdirectories.
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails.
    pub fn list<T: Storable>(&self) -> std::io::Result<Vec<T>> {
        let base_dir = self.base_dir.join(T::storage_dir_name());
        let mut items = Vec::new();

        for subdir in T::search_subdirs() {
            let dir = base_dir.join(subdir);
            if dir.exists() {
                for entry in fs::read_dir(&dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        let contents = fs::read_to_string(&path)?;
                        if let Ok(item) = self.parse_item(&contents) {
                            items.push(item);
                        }
                    }
                }
            }
        }

        // Also check base directory
        if base_dir.exists() && T::search_subdirs().is_empty() {
            for entry in fs::read_dir(&base_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    let contents = fs::read_to_string(&path)?;
                    if let Ok(item) = self.parse_item(&contents) {
                        items.push(item);
                    }
                }
            }
        }

        Ok(items)
    }

    /// Delete a storable item by ID.
    ///
    /// Searches all status subdirectories to find and delete the item.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or delete fails.
    pub fn delete<T: Storable>(&self, id: Uuid) -> std::io::Result<()> {
        let base_dir = self.base_dir.join(T::storage_dir_name());
        let filename = format!("{id}.json");

        for subdir in T::search_subdirs() {
            let path = base_dir.join(subdir).join(&filename);
            if path.exists() {
                return fs::remove_file(path);
            }
        }

        // Check base directory
        let path = base_dir.join(&filename);
        if path.exists() {
            return fs::remove_file(path);
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} {} not found", T::entity_name(), id),
        ))
    }

    // ==========================================================================
    // Report operations (specialized - uses timestamp naming + markdown)
    // ==========================================================================

    /// Save an introspection report (both JSON and markdown)
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails
    #[tracing::instrument(skip(self, report), fields(report_id = %report.id))]
    pub fn save_report(&self, report: &IntrospectionReport) -> std::io::Result<ReportPathPair> {
        self.ensure_dir(&self.reports_dir())?;

        let timestamp = report.timestamp.format("%Y-%m-%dT%H-%M-%S");
        let id_short = &report.id.to_string()[..8];
        let base_name = format!("{timestamp}_{id_short}");

        let json_path = self.reports_dir().join(format!("{base_name}.json"));
        let md_path = self.reports_dir().join(format!("{base_name}.md"));

        // Save JSON (with version if enabled)
        let json = if self.versioned {
            SchemaMigrator::save_versioned(report)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            report
                .to_json()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&json_path, json)?;

        // Save markdown
        let md = report.to_markdown();
        fs::write(&md_path, md)?;

        // M-652: Record storage operation metric
        record_storage_operation("save", "report");

        Ok((json_path, md_path))
    }

    /// Load an introspection report by ID
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails
    #[tracing::instrument(skip(self), fields(report_id = %id))]
    pub fn load_report(&self, id: Uuid) -> std::io::Result<IntrospectionReport> {
        let id_short = &id.to_string()[..8];

        // Find the file matching the ID
        for entry in fs::read_dir(self.reports_dir())? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".json") && name_str.contains(id_short) {
                let contents = fs::read_to_string(entry.path())?;
                let report = self.parse_report(&contents)?;
                // M-652: Record storage operation metric
                record_storage_operation("load", "report");
                return Ok(report);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Report {id} not found"),
        ))
    }

    /// Parse a report from JSON, handling versioned data and migration.
    fn parse_report(&self, contents: &str) -> std::io::Result<IntrospectionReport> {
        if self.versioned {
            let migrator = SchemaMigrator::new();
            let result = migrator
                .load_versioned::<IntrospectionReport>(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            Ok(result.data)
        } else {
            IntrospectionReport::from_json(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }
    }

    /// Get the most recent report
    ///
    /// # Errors
    ///
    /// Returns error if no reports found or load fails
    pub fn latest_report(&self) -> std::io::Result<Option<IntrospectionReport>> {
        let mut reports: Vec<_> = fs::read_dir(self.reports_dir())?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();

        if reports.is_empty() {
            return Ok(None);
        }

        // Sort by filename (which includes timestamp) descending
        reports.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

        let path = reports[0].path();
        let contents = fs::read_to_string(path)?;
        let report = self.parse_report(&contents)?;

        Ok(Some(report))
    }

    /// List all report IDs
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn list_reports(&self) -> std::io::Result<Vec<Uuid>> {
        let mut ids = Vec::new();

        if !self.reports_dir().exists() {
            return Ok(ids);
        }

        for entry in fs::read_dir(self.reports_dir())? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".json") {
                // Try to load the file to get full UUID
                if let Ok(contents) = fs::read_to_string(entry.path()) {
                    if let Ok(report) = self.parse_report(&contents) {
                        ids.push(report.id);
                    }
                }
            }
        }

        Ok(ids)
    }

    // Plan operations

    /// Save an execution plan
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails
    #[tracing::instrument(skip(self, plan), fields(plan_id = %plan.id, status = ?plan.status))]
    pub fn save_plan(&self, plan: &ExecutionPlan) -> std::io::Result<PathBuf> {
        let subdir = match &plan.status {
            PlanStatus::Proposed => "pending",
            PlanStatus::Validated => "pending",
            PlanStatus::InProgress { .. } => "approved",
            PlanStatus::Implemented { .. } => "implemented",
            PlanStatus::Failed { .. } => "failed",
            PlanStatus::Superseded { .. } => "failed",
        };

        let dir = self.plans_dir().join(subdir);
        self.ensure_dir(&dir)?;

        let path = dir.join(format!("{}.json", plan.id));
        let json = if self.versioned {
            SchemaMigrator::save_versioned(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&path, json)?;

        // Update plan index
        if let Err(e) = self.update_plan_index(plan.id, subdir) {
            warn!(plan_id = %plan.id, "Failed to update plan index: {}", e);
        }

        // M-652: Record storage operation metric
        record_storage_operation("save", "plan");

        Ok(path)
    }

    /// Load a plan by ID
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails
    #[tracing::instrument(skip(self), fields(plan_id = %id))]
    pub fn load_plan(&self, id: Uuid) -> std::io::Result<ExecutionPlan> {
        let filename = format!("{id}.json");

        // Search in all subdirectories
        for subdir in ["pending", "approved", "implemented", "failed"] {
            let path = self.plans_dir().join(subdir).join(&filename);
            if path.exists() {
                let contents = fs::read_to_string(path)?;
                let plan = self.parse_plan(&contents)?;
                // M-652: Record storage operation metric
                record_storage_operation("load", "plan");
                return Ok(plan);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Plan {id} not found"),
        ))
    }

    /// Parse a plan from JSON, handling versioned data and migration.
    fn parse_plan(&self, contents: &str) -> std::io::Result<ExecutionPlan> {
        if self.versioned {
            let migrator = SchemaMigrator::new();
            let result = migrator
                .load_versioned::<ExecutionPlan>(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            Ok(result.data)
        } else {
            serde_json::from_str(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }
    }

    /// Parse a plan from JSON (public wrapper for lazy loading).
    ///
    /// # Errors
    ///
    /// Returns error if JSON parsing fails
    pub fn parse_plan_from_json(&self, contents: &str) -> std::io::Result<ExecutionPlan> {
        self.parse_plan(contents)
    }

    /// List pending plans
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn pending_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir("pending")
    }

    /// List approved plans (ready for implementation)
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn approved_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir("approved")
    }

    // Batch Operations

    /// Save multiple plans in a single batch operation.
    ///
    /// More efficient than calling `save_plan` multiple times because:
    /// - Only loads/saves the plan index once
    /// - Can be parallelized in the future
    ///
    /// # Errors
    ///
    /// Returns the paths of successfully saved plans and any errors.
    /// Continues saving even if some plans fail.
    pub fn save_plans_batch(
        &self,
        plans: &[ExecutionPlan],
    ) -> (Vec<PathBuf>, Vec<(Uuid, std::io::Error)>) {
        let mut saved = Vec::with_capacity(plans.len());
        let mut errors = Vec::new();

        // Pre-load the index once
        let mut index = self.load_plan_index().unwrap_or_default();

        for plan in plans {
            let subdir = match &plan.status {
                PlanStatus::Proposed => "pending",
                PlanStatus::Validated => "pending",
                PlanStatus::InProgress { .. } => "approved",
                PlanStatus::Implemented { .. } => "implemented",
                PlanStatus::Failed { .. } => "failed",
                PlanStatus::Superseded { .. } => "failed",
            };

            let dir = self.plans_dir().join(subdir);
            if let Err(e) = self.ensure_dir(&dir) {
                errors.push((plan.id, e));
                continue;
            }

            let path = dir.join(format!("{}.json", plan.id));
            let json_result = if self.versioned {
                SchemaMigrator::save_versioned(plan)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            } else {
                serde_json::to_string_pretty(plan)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            };

            match json_result {
                Ok(json) => {
                    if let Err(e) = fs::write(&path, json) {
                        errors.push((plan.id, e));
                    } else {
                        index.insert(plan.id, subdir);
                        saved.push(path);
                    }
                }
                Err(e) => {
                    errors.push((plan.id, e));
                }
            }
        }

        // Save the index once at the end
        if let Err(e) = self.save_plan_index(&index) {
            warn!("Failed to save plan index after batch save: {}", e);
        }

        (saved, errors)
    }

    /// Load multiple plans by their IDs in a single batch operation.
    ///
    /// More efficient than calling `load_plan` multiple times because:
    /// - Uses the plan index for O(1) lookups
    /// - Only loads the index once
    ///
    /// # Returns
    ///
    /// A tuple of (successfully loaded plans, failed IDs with errors).
    pub fn load_plans_batch(
        &self,
        ids: &[Uuid],
    ) -> (Vec<ExecutionPlan>, Vec<(Uuid, std::io::Error)>) {
        let mut loaded = Vec::with_capacity(ids.len());
        let mut errors = Vec::new();

        // Load index once
        let index = self.load_plan_index().unwrap_or_default();

        for &id in ids {
            let filename = format!("{id}.json");

            // Try index first
            let path = if let Some(subdir) = index.get(&id) {
                let path = self.plans_dir().join(subdir).join(&filename);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            } else {
                None
            };

            // Fallback to directory scan
            let path = path.or_else(|| {
                for subdir in ["pending", "approved", "implemented", "failed"] {
                    let p = self.plans_dir().join(subdir).join(&filename);
                    if p.exists() {
                        return Some(p);
                    }
                }
                None
            });

            match path {
                Some(p) => match fs::read_to_string(&p) {
                    Ok(content) => {
                        let plan_result = if self.versioned {
                            let migrator = SchemaMigrator::new();
                            migrator
                                .load_versioned::<ExecutionPlan>(&content)
                                .map(|r| r.data)
                                .map_err(|e| {
                                    std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        e.to_string(),
                                    )
                                })
                        } else {
                            serde_json::from_str(&content).map_err(|e| {
                                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                            })
                        };
                        match plan_result {
                            Ok(plan) => loaded.push(plan),
                            Err(e) => errors.push((id, e)),
                        }
                    }
                    Err(e) => errors.push((id, e)),
                },
                None => {
                    errors.push((
                        id,
                        std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            format!("Plan {id} not found"),
                        ),
                    ));
                }
            }
        }

        (loaded, errors)
    }

    /// Save multiple reports in a batch operation.
    ///
    /// # Returns
    ///
    /// A tuple of (successfully saved paths, failed reports with errors).
    pub fn save_reports_batch(&self, reports: &[IntrospectionReport]) -> BatchReportResult {
        let mut saved = Vec::with_capacity(reports.len());
        let mut errors = Vec::new();

        if let Err(e) = self.ensure_dir(&self.reports_dir()) {
            // If we can't create the directory, fail all reports
            for report in reports {
                errors.push((report.id, e.kind().into()));
            }
            return (saved, errors);
        }

        for report in reports {
            match self.save_report(report) {
                Ok(paths) => saved.push(paths),
                Err(e) => errors.push((report.id, e)),
            }
        }

        (saved, errors)
    }

    /// Move a plan to a new status directory
    ///
    /// # Errors
    ///
    /// Returns error if move fails
    pub fn update_plan_status(&self, id: Uuid, new_status: PlanStatus) -> std::io::Result<PathBuf> {
        // Find and load the plan
        let mut plan = self.load_plan(id)?;
        let old_path = self.find_plan_path(id)?;

        // Update status
        plan.status = new_status;

        // Remove old file
        fs::remove_file(&old_path)?;

        // Save to new location
        self.save_plan(&plan)
    }

    /// Approve a plan (move from pending to approved)
    ///
    /// # Errors
    ///
    /// Returns error if move fails
    pub fn approve_plan(&self, id: Uuid, assignee: impl Into<String>) -> std::io::Result<PathBuf> {
        let result = self.update_plan_status(
            id,
            PlanStatus::InProgress {
                started: Utc::now(),
                assignee: assignee.into(),
            },
        );
        // M-652: Record plan approval metric
        if result.is_ok() {
            record_plan_approved();
        }
        result
    }

    /// Mark plan as implemented
    ///
    /// # Errors
    ///
    /// Returns error if update fails
    pub fn complete_plan(
        &self,
        id: Uuid,
        commit_hash: impl Into<String>,
    ) -> std::io::Result<PathBuf> {
        let result = self.update_plan_status(
            id,
            PlanStatus::Implemented {
                completed: Utc::now(),
                commit_hash: commit_hash.into(),
            },
        );
        // M-652: Record plan implementation metric
        if result.is_ok() {
            record_plan_implemented();
        }
        result
    }

    /// Mark plan as failed
    ///
    /// # Errors
    ///
    /// Returns error if update fails
    pub fn fail_plan(&self, id: Uuid, reason: impl Into<String>) -> std::io::Result<PathBuf> {
        let result = self.update_plan_status(
            id,
            PlanStatus::Failed {
                reason: reason.into(),
            },
        );
        // M-652: Record plan failure metric
        if result.is_ok() {
            record_plan_failed();
        }
        result
    }

    /// Update an existing plan in place (without changing directories)
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if plan not found or write fails
    pub fn update_plan(&self, plan: &ExecutionPlan) -> std::io::Result<PathBuf> {
        let path = self.find_plan_path(plan.id)?;
        // M-926: Use versioned storage to match save_plan behavior
        let json = if self.versioned {
            SchemaMigrator::save_versioned(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&path, json)?;
        Ok(path)
    }

    /// List pending plans (alias for pending_plans for API consistency)
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn list_pending_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.pending_plans()
    }

    /// List implemented plans
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn list_implemented_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir("implemented")
    }

    /// List failed plans
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn list_failed_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir("failed")
    }

    /// Move a plan to the implemented directory
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if move fails
    pub fn move_plan_to_implemented(&self, id: &Uuid) -> std::io::Result<PathBuf> {
        let mut plan = self.load_plan(*id)?;
        let old_path = self.find_plan_path(*id)?;

        // Remove old file
        fs::remove_file(&old_path)?;

        // Update status if not already set
        if !matches!(plan.status, PlanStatus::Implemented { .. }) {
            plan.status = PlanStatus::Implemented {
                completed: Utc::now(),
                commit_hash: String::new(),
            };
        }

        // Save to implemented directory
        let dir = self.plans_dir().join("implemented");
        self.ensure_dir(&dir)?;
        let new_path = dir.join(format!("{}.json", id));
        // M-927: Use versioned storage to match save_plan behavior
        let json = if self.versioned {
            SchemaMigrator::save_versioned(&plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(&plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&new_path, json)?;

        // M-927: Update plan index to match save_plan behavior
        if let Err(e) = self.update_plan_index(*id, "implemented") {
            warn!(plan_id = %id, "Failed to update plan index: {}", e);
        }

        Ok(new_path)
    }

    /// Move a plan to the failed directory
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if move fails
    pub fn move_plan_to_failed(&self, id: &Uuid) -> std::io::Result<PathBuf> {
        let mut plan = self.load_plan(*id)?;
        let old_path = self.find_plan_path(*id)?;

        // Remove old file
        fs::remove_file(&old_path)?;

        // Update status if not already set
        if !matches!(
            plan.status,
            PlanStatus::Failed { .. } | PlanStatus::Superseded { .. }
        ) {
            plan.status = PlanStatus::Failed {
                reason: "Moved to failed".to_string(),
            };
        }

        // Save to failed directory
        let dir = self.plans_dir().join("failed");
        self.ensure_dir(&dir)?;
        let new_path = dir.join(format!("{}.json", id));
        // M-927: Use versioned storage to match save_plan behavior
        let json = if self.versioned {
            SchemaMigrator::save_versioned(&plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(&plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&new_path, json)?;

        // M-927: Update plan index to match save_plan behavior
        if let Err(e) = self.update_plan_index(*id, "failed") {
            warn!(plan_id = %id, "Failed to update plan index: {}", e);
        }

        Ok(new_path)
    }

    fn list_plans_in_dir(&self, subdir: &str) -> std::io::Result<Vec<ExecutionPlan>> {
        let dir = self.plans_dir().join(subdir);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut plans = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let contents = fs::read_to_string(entry.path())?;
                if let Ok(plan) = self.parse_plan(&contents) {
                    plans.push(plan);
                }
            }
        }

        // Sort by priority
        plans.sort_by_key(|p: &ExecutionPlan| p.priority);
        Ok(plans)
    }

    fn find_plan_path(&self, id: Uuid) -> std::io::Result<PathBuf> {
        // Use index for O(1) lookup
        let index = self.load_plan_index()?;
        let filename = format!("{id}.json");

        if let Some(subdir) = index.get(&id) {
            let path = self.plans_dir().join(subdir).join(&filename);
            if path.exists() {
                return Ok(path);
            }
            // Index was stale - fall through to directory scan
        }

        // Fallback: scan directories (handles index not existing or being stale)
        for subdir in ["pending", "approved", "implemented", "failed"] {
            let path = self.plans_dir().join(subdir).join(&filename);
            if path.exists() {
                // Update index with correct location
                if let Err(e) = self.update_plan_index(id, subdir) {
                    warn!(plan_id = %id, "Failed to update plan index during path lookup: {}", e);
                }
                return Ok(path);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Plan {id} not found"),
        ))
    }

    // Hypothesis operations

    /// Save a hypothesis
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails
    pub fn save_hypothesis(&self, hypothesis: &Hypothesis) -> std::io::Result<PathBuf> {
        let subdir = match &hypothesis.status {
            HypothesisStatus::Active => "active",
            HypothesisStatus::Pending { .. } => "active",
            HypothesisStatus::Evaluated => "evaluated",
            HypothesisStatus::Superseded { .. } => "evaluated",
        };

        let dir = self.hypotheses_dir().join(subdir);
        self.ensure_dir(&dir)?;

        let path = dir.join(format!("{}.json", hypothesis.id));
        let json = if self.versioned {
            SchemaMigrator::save_versioned(hypothesis)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(hypothesis)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        fs::write(&path, json)?;

        // M-652: Record storage operation metric
        record_storage_operation("save", "hypothesis");

        Ok(path)
    }

    /// Load a hypothesis by ID
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails
    pub fn load_hypothesis(&self, id: Uuid) -> std::io::Result<Hypothesis> {
        let filename = format!("{id}.json");

        for subdir in ["active", "evaluated"] {
            let path = self.hypotheses_dir().join(subdir).join(&filename);
            if path.exists() {
                let contents = fs::read_to_string(path)?;
                let hypothesis = self.parse_hypothesis(&contents)?;
                // M-652: Record storage operation metric
                record_storage_operation("load", "hypothesis");
                return Ok(hypothesis);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Hypothesis {id} not found"),
        ))
    }

    /// Parse a hypothesis from JSON, handling versioned data and migration.
    fn parse_hypothesis(&self, contents: &str) -> std::io::Result<Hypothesis> {
        if self.versioned {
            let migrator = SchemaMigrator::new();
            let result = migrator
                .load_versioned::<Hypothesis>(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            Ok(result.data)
        } else {
            serde_json::from_str(contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }
    }

    /// Parse a hypothesis from JSON (public wrapper for lazy loading).
    ///
    /// # Errors
    ///
    /// Returns error if JSON parsing fails
    pub fn parse_hypothesis_from_json(&self, contents: &str) -> std::io::Result<Hypothesis> {
        self.parse_hypothesis(contents)
    }

    /// List active hypotheses
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn active_hypotheses(&self) -> std::io::Result<Vec<Hypothesis>> {
        self.list_hypotheses_in_dir("active")
    }

    /// List evaluated hypotheses
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn evaluated_hypotheses(&self) -> std::io::Result<Vec<Hypothesis>> {
        self.list_hypotheses_in_dir("evaluated")
    }

    /// List all hypotheses (both active and evaluated)
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails
    pub fn all_hypotheses(&self) -> std::io::Result<Vec<Hypothesis>> {
        let mut all = self.active_hypotheses()?;
        all.extend(self.evaluated_hypotheses()?);
        // Sort by creation date, most recent first
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(all)
    }

    fn list_hypotheses_in_dir(&self, subdir: &str) -> std::io::Result<Vec<Hypothesis>> {
        let dir = self.hypotheses_dir().join(subdir);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut hypotheses = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let contents = fs::read_to_string(entry.path())?;
                if let Ok(hyp) = self.parse_hypothesis(&contents) {
                    hypotheses.push(hyp);
                }
            }
        }

        Ok(hypotheses)
    }

    /// Mark hypothesis as evaluated
    ///
    /// # Errors
    ///
    /// Returns error if update fails
    pub fn evaluate_hypothesis(&self, mut hypothesis: Hypothesis) -> std::io::Result<PathBuf> {
        // Remove from active
        let old_path = self
            .hypotheses_dir()
            .join("active")
            .join(format!("{}.json", hypothesis.id));
        if old_path.exists() {
            fs::remove_file(&old_path)?;
        }

        // Update status and save to evaluated
        hypothesis.status = HypothesisStatus::Evaluated;
        self.save_hypothesis(&hypothesis)
    }

    // Helper methods

    fn ensure_dir(&self, dir: &Path) -> std::io::Result<()> {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    // ========================================================================
    // Async Storage Variants
    // ========================================================================
    //
    // These async methods provide non-blocking I/O for use in async contexts.
    // They mirror the synchronous methods but use tokio::fs for file operations.

    /// Initialize the storage directory structure asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if directory creation fails.
    pub async fn initialize_async(&self) -> std::io::Result<()> {
        let dirs = [
            self.reports_dir(),
            self.plans_dir().join("pending"),
            self.plans_dir().join("approved"),
            self.plans_dir().join("implemented"),
            self.plans_dir().join("failed"),
            self.hypotheses_dir().join("active"),
            self.hypotheses_dir().join("evaluated"),
            self.meta_dir(),
        ];

        for dir in dirs {
            tokio::fs::create_dir_all(dir).await?;
        }

        Ok(())
    }

    async fn ensure_dir_async(&self, dir: &Path) -> std::io::Result<()> {
        if !dir.exists() {
            tokio::fs::create_dir_all(dir).await?;
        }
        Ok(())
    }

    /// Save an introspection report asynchronously (both JSON and markdown).
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    pub async fn save_report_async(
        &self,
        report: &IntrospectionReport,
    ) -> std::io::Result<ReportPathPair> {
        self.ensure_dir_async(&self.reports_dir()).await?;

        let timestamp = report.timestamp.format("%Y-%m-%dT%H-%M-%S");
        let id_short = &report.id.to_string()[..8];
        let base_name = format!("{timestamp}_{id_short}");

        let json_path = self.reports_dir().join(format!("{base_name}.json"));
        let md_path = self.reports_dir().join(format!("{base_name}.md"));

        // Save JSON (with version if enabled) - M-923: match sync version behavior
        let json = if self.versioned {
            SchemaMigrator::save_versioned(report)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            report
                .to_json()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        tokio::fs::write(&json_path, json).await?;

        // Save markdown
        let md = report.to_markdown();
        tokio::fs::write(&md_path, md).await?;

        // M-923: Record storage operation metric to match sync version
        record_storage_operation("save", "report");

        Ok((json_path, md_path))
    }

    /// Load an introspection report by ID asynchronously.
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails.
    pub async fn load_report_async(&self, id: Uuid) -> std::io::Result<IntrospectionReport> {
        let id_short = &id.to_string()[..8];

        // Find the file matching the ID
        let mut entries = tokio::fs::read_dir(self.reports_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".json") && name_str.contains(id_short) {
                let contents = tokio::fs::read_to_string(entry.path()).await?;
                // M-923: Use versioned parsing to match sync version
                let report = self.parse_report(&contents)?;
                record_storage_operation("load", "report");
                return Ok(report);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Report {id} not found"),
        ))
    }

    /// Get the most recent report asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if no reports found or load fails.
    pub async fn latest_report_async(&self) -> std::io::Result<Option<IntrospectionReport>> {
        let reports_dir = self.reports_dir();
        if !reports_dir.exists() {
            return Ok(None);
        }

        let mut reports: Vec<std::ffi::OsString> = Vec::new();
        let mut entries = tokio::fs::read_dir(&reports_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                reports.push(entry.file_name());
            }
        }

        if reports.is_empty() {
            return Ok(None);
        }

        // Sort by filename (which includes timestamp) descending
        reports.sort_by(|a, b| b.cmp(a));

        let path = reports_dir.join(&reports[0]);
        let contents = tokio::fs::read_to_string(path).await?;
        // M-923: Use versioned parsing to match sync version
        let report = self.parse_report(&contents)?;

        Ok(Some(report))
    }

    /// Save an execution plan asynchronously.
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    pub async fn save_plan_async(&self, plan: &ExecutionPlan) -> std::io::Result<PathBuf> {
        let subdir = match &plan.status {
            PlanStatus::Proposed => "pending",
            PlanStatus::Validated => "pending",
            PlanStatus::InProgress { .. } => "approved",
            PlanStatus::Implemented { .. } => "implemented",
            PlanStatus::Failed { .. } => "failed",
            PlanStatus::Superseded { .. } => "failed",
        };

        let dir = self.plans_dir().join(subdir);
        self.ensure_dir_async(&dir).await?;

        let path = dir.join(format!("{}.json", plan.id));
        // M-924: Use versioned storage to match sync version
        let json = if self.versioned {
            SchemaMigrator::save_versioned(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(plan)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        tokio::fs::write(&path, json).await?;

        // M-924: Update plan index to match sync version
        if let Err(e) = self.update_plan_index(plan.id, subdir) {
            warn!(plan_id = %plan.id, "Failed to update plan index: {}", e);
        }

        // M-924: Record storage operation metric to match sync version
        record_storage_operation("save", "plan");

        Ok(path)
    }

    /// Load a plan by ID asynchronously.
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails.
    pub async fn load_plan_async(&self, id: Uuid) -> std::io::Result<ExecutionPlan> {
        let filename = format!("{id}.json");

        // Search in all subdirectories
        for subdir in ["pending", "approved", "implemented", "failed"] {
            let path = self.plans_dir().join(subdir).join(&filename);
            if path.exists() {
                let contents = tokio::fs::read_to_string(&path).await?;
                // M-924: Use versioned parsing to match sync version
                let plan = self.parse_plan(&contents)?;
                record_storage_operation("load", "plan");
                return Ok(plan);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Plan {id} not found"),
        ))
    }

    /// List pending plans asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails.
    pub async fn pending_plans_async(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir_async("pending").await
    }

    /// List approved plans asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if directory read fails.
    pub async fn approved_plans_async(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.list_plans_in_dir_async("approved").await
    }

    async fn list_plans_in_dir_async(&self, subdir: &str) -> std::io::Result<Vec<ExecutionPlan>> {
        let dir = self.plans_dir().join(subdir);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut plans = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let contents = tokio::fs::read_to_string(&path).await?;
                // M-924: Use versioned parsing to match sync version
                if let Ok(plan) = self.parse_plan(&contents) {
                    plans.push(plan);
                }
            }
        }

        // Sort by priority
        plans.sort_by_key(|p: &ExecutionPlan| p.priority);
        Ok(plans)
    }

    /// Save a hypothesis asynchronously.
    ///
    /// When versioned storage is enabled, the JSON includes `_schema_version`.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    pub async fn save_hypothesis_async(&self, hypothesis: &Hypothesis) -> std::io::Result<PathBuf> {
        let subdir = match &hypothesis.status {
            HypothesisStatus::Active => "active",
            HypothesisStatus::Pending { .. } => "active",
            HypothesisStatus::Evaluated => "evaluated",
            HypothesisStatus::Superseded { .. } => "evaluated",
        };

        let dir = self.hypotheses_dir().join(subdir);
        self.ensure_dir_async(&dir).await?;

        let path = dir.join(format!("{}.json", hypothesis.id));
        // M-925: Use versioned storage to match sync version
        let json = if self.versioned {
            SchemaMigrator::save_versioned(hypothesis)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            serde_json::to_string_pretty(hypothesis)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        tokio::fs::write(&path, json).await?;

        // M-925: Record storage operation metric to match sync version
        record_storage_operation("save", "hypothesis");

        Ok(path)
    }

    /// Load a hypothesis by ID asynchronously.
    ///
    /// Automatically migrates older schema versions when versioned storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns error if file not found or parse fails.
    pub async fn load_hypothesis_async(&self, id: Uuid) -> std::io::Result<Hypothesis> {
        let filename = format!("{id}.json");

        for subdir in ["active", "evaluated"] {
            let path = self.hypotheses_dir().join(subdir).join(&filename);
            if path.exists() {
                let contents = tokio::fs::read_to_string(&path).await?;
                // M-925: Use versioned parsing to match sync version
                let hypothesis = self.parse_hypothesis(&contents)?;
                record_storage_operation("load", "hypothesis");
                return Ok(hypothesis);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Hypothesis {id} not found"),
        ))
    }

    /// Get storage statistics asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be read.
    pub async fn stats_async(&self) -> std::io::Result<StorageStats> {
        let mut stats = StorageStats::default();

        // Count reports
        if self.reports_dir().exists() {
            let mut entries = tokio::fs::read_dir(self.reports_dir()).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    stats.report_count += 1;
                    if let Ok(meta) = entry.metadata().await {
                        stats.total_size_bytes += meta.len();
                    }
                }
            }
        }

        // Count plans by status
        let plan_dirs = ["pending", "approved", "implemented", "failed"];
        let mut plan_counts = [0usize; 4];
        for (i, subdir) in plan_dirs.iter().enumerate() {
            let dir = self.plans_dir().join(subdir);
            if dir.exists() {
                let mut entries = tokio::fs::read_dir(&dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        plan_counts[i] += 1;
                        if let Ok(meta) = entry.metadata().await {
                            stats.total_size_bytes += meta.len();
                        }
                    }
                }
            }
        }
        stats.plan_counts = (
            plan_counts[0],
            plan_counts[1],
            plan_counts[2],
            plan_counts[3],
        );

        // Count hypotheses
        for (i, subdir) in ["active", "evaluated"].iter().enumerate() {
            let dir = self.hypotheses_dir().join(subdir);
            if dir.exists() {
                let mut entries = tokio::fs::read_dir(&dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        if i == 0 {
                            stats.hypothesis_counts.0 += 1;
                        } else {
                            stats.hypothesis_counts.1 += 1;
                        }
                        if let Ok(meta) = entry.metadata().await {
                            stats.total_size_bytes += meta.len();
                        }
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Check storage health status asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if stats cannot be retrieved.
    pub async fn check_health_async(&self) -> std::io::Result<StorageHealthStatus> {
        let stats = self.stats_async().await?;
        let mut warnings = Vec::new();
        let mut level = StorageHealthLevel::Healthy;

        // Get thresholds from environment or use defaults (env reads MB, we need bytes)
        let warning_size_mb = env_u64(
            DASHFLOW_STORAGE_WARNING_SIZE_MB,
            DEFAULT_STORAGE_WARNING_SIZE_BYTES / (1024 * 1024),
        );
        let warning_size = warning_size_mb * 1024 * 1024;

        let critical_size_mb = env_u64(
            DASHFLOW_STORAGE_CRITICAL_SIZE_MB,
            DEFAULT_STORAGE_CRITICAL_SIZE_BYTES / (1024 * 1024),
        );
        let critical_size = critical_size_mb * 1024 * 1024;

        let report_warning = env_usize(
            DASHFLOW_STORAGE_REPORT_WARNING_COUNT,
            DEFAULT_REPORT_WARNING_COUNT,
        );

        let plan_warning = env_usize(
            DASHFLOW_STORAGE_PLAN_WARNING_COUNT,
            DEFAULT_PLAN_WARNING_COUNT,
        );

        // Check total size
        if stats.total_size_bytes >= critical_size {
            level = StorageHealthLevel::Critical;
            warnings.push(format!(
                "Storage size critical: {} MB (threshold: {} MB)",
                stats.total_size_bytes / (1024 * 1024),
                critical_size / (1024 * 1024)
            ));
        } else if stats.total_size_bytes >= warning_size {
            level = StorageHealthLevel::Warning;
            warnings.push(format!(
                "Storage size warning: {} MB (threshold: {} MB)",
                stats.total_size_bytes / (1024 * 1024),
                warning_size / (1024 * 1024)
            ));
        }

        // Check report count
        if stats.report_count >= report_warning {
            if level == StorageHealthLevel::Healthy {
                level = StorageHealthLevel::Warning;
            }
            warnings.push(format!(
                "Report count warning: {} (threshold: {})",
                stats.report_count, report_warning
            ));
        }

        // Check plan counts
        let total_plans =
            stats.plan_counts.0 + stats.plan_counts.1 + stats.plan_counts.2 + stats.plan_counts.3;
        if total_plans >= plan_warning * 4 {
            if level == StorageHealthLevel::Healthy {
                level = StorageHealthLevel::Warning;
            }
            warnings.push(format!(
                "Total plan count warning: {} (threshold: {})",
                total_plans,
                plan_warning * 4
            ));
        }

        // Individual status directory checks
        if stats.plan_counts.2 >= plan_warning {
            warnings.push(format!(
                "Implemented plans warning: {} (threshold: {})",
                stats.plan_counts.2, plan_warning
            ));
        }
        if stats.plan_counts.3 >= plan_warning {
            warnings.push(format!(
                "Failed plans warning: {} (threshold: {})",
                stats.plan_counts.3, plan_warning
            ));
        }

        let cleanup_recommended = level != StorageHealthLevel::Healthy;

        Ok(StorageHealthStatus {
            level,
            stats,
            warnings,
            cleanup_recommended,
        })
    }

    // ========================================================================
    // Storage Limits
    // ========================================================================

    /// Get storage statistics.
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be read.
    pub fn stats(&self) -> std::io::Result<StorageStats> {
        let mut stats = StorageStats::default();

        // Count reports
        if self.reports_dir().exists() {
            for entry in fs::read_dir(self.reports_dir())? {
                let entry = entry?;
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    stats.report_count += 1;
                    if let Ok(meta) = entry.metadata() {
                        stats.total_size_bytes += meta.len();
                    }
                }
            }
        }

        // Count plans by status
        let plan_dirs = ["pending", "approved", "implemented", "failed"];
        let mut plan_counts = [0usize; 4];
        for (i, subdir) in plan_dirs.iter().enumerate() {
            let dir = self.plans_dir().join(subdir);
            if dir.exists() {
                for entry in fs::read_dir(&dir)? {
                    let entry = entry?;
                    if entry.path().extension().is_some_and(|ext| ext == "json") {
                        plan_counts[i] += 1;
                        if let Ok(meta) = entry.metadata() {
                            stats.total_size_bytes += meta.len();
                        }
                    }
                }
            }
        }
        stats.plan_counts = (
            plan_counts[0],
            plan_counts[1],
            plan_counts[2],
            plan_counts[3],
        );

        // Count hypotheses
        for (i, subdir) in ["active", "evaluated"].iter().enumerate() {
            let dir = self.hypotheses_dir().join(subdir);
            if dir.exists() {
                for entry in fs::read_dir(&dir)? {
                    let entry = entry?;
                    if entry.path().extension().is_some_and(|ext| ext == "json") {
                        if i == 0 {
                            stats.hypothesis_counts.0 += 1;
                        } else {
                            stats.hypothesis_counts.1 += 1;
                        }
                        if let Ok(meta) = entry.metadata() {
                            stats.total_size_bytes += meta.len();
                        }
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Run cleanup according to the storage policy.
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be read. Individual file
    /// deletion errors are recorded in stats but don't fail the operation.
    pub fn cleanup(&self) -> std::io::Result<StorageCleanupStats> {
        let mut stats = StorageCleanupStats::default();

        if !self.policy.enabled {
            return Ok(stats);
        }

        // Cleanup reports by count limit
        if let Some(max_reports) = self.policy.max_reports {
            let deleted = self.cleanup_reports_by_count(max_reports, &mut stats)?;
            stats.reports_deleted += deleted;
        }

        // Cleanup archived plans by age and count
        let now = SystemTime::now();
        for subdir in ["implemented", "failed"] {
            // Age-based cleanup
            if let Some(max_age) = self.policy.plan_archive_age {
                let deleted =
                    self.cleanup_by_age(&self.plans_dir().join(subdir), max_age, now, &mut stats)?;
                stats.plans_deleted += deleted;
            }

            // Count-based cleanup
            if let Some(max_plans) = self.policy.max_plans_per_status {
                let deleted = self.cleanup_dir_by_count(
                    &self.plans_dir().join(subdir),
                    max_plans,
                    &mut stats,
                )?;
                stats.plans_deleted += deleted;
            }
        }

        // Cleanup evaluated hypotheses by age
        if let Some(max_age) = self.policy.hypothesis_archive_age {
            let deleted = self.cleanup_by_age(
                &self.hypotheses_dir().join("evaluated"),
                max_age,
                now,
                &mut stats,
            )?;
            stats.hypotheses_deleted += deleted;
        }

        stats.total_deleted =
            stats.reports_deleted + stats.plans_deleted + stats.hypotheses_deleted;
        Ok(stats)
    }

    fn cleanup_reports_by_count(
        &self,
        max_count: usize,
        stats: &mut StorageCleanupStats,
    ) -> std::io::Result<usize> {
        let dir = self.reports_dir();
        if !dir.exists() {
            return Ok(0);
        }

        // List all report files with modification times
        let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-JSON files
            if !path.extension().is_some_and(|ext| ext == "json") {
                continue;
            }

            // Skip files we can't get metadata/mtime for
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(mtime) = meta.modified() else { continue };

            files.push((path, mtime, meta.len()));
        }

        // Sort by mtime (oldest first)
        files.sort_by_key(|(_, mtime, _)| *mtime);

        // Delete oldest files exceeding limit
        let mut deleted = 0;
        while files.len() > max_count {
            let (path, _, size) = files.remove(0);
            // Also delete matching .md file
            let md_path = path.with_extension("md");

            match fs::remove_file(&path) {
                Ok(()) => {
                    deleted += 1;
                    stats.bytes_freed += size;
                    // Try to remove .md too
                    if md_path.exists() {
                        if let Ok(md_meta) = fs::metadata(&md_path) {
                            stats.bytes_freed += md_meta.len();
                        }
                        let _ = fs::remove_file(&md_path);
                    }
                }
                Err(e) => {
                    stats
                        .errors
                        .push(format!("Failed to delete {}: {}", path.display(), e));
                }
            }
        }

        Ok(deleted)
    }

    fn cleanup_dir_by_count(
        &self,
        dir: &Path,
        max_count: usize,
        stats: &mut StorageCleanupStats,
    ) -> std::io::Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        files.push((path, mtime, meta.len()));
                    }
                }
            }
        }

        files.sort_by_key(|(_, mtime, _)| *mtime);

        let mut deleted = 0;
        while files.len() > max_count {
            let (path, _, size) = files.remove(0);
            match fs::remove_file(&path) {
                Ok(()) => {
                    deleted += 1;
                    stats.bytes_freed += size;
                }
                Err(e) => {
                    stats
                        .errors
                        .push(format!("Failed to delete {}: {}", path.display(), e));
                }
            }
        }

        Ok(deleted)
    }

    fn cleanup_by_age(
        &self,
        dir: &Path,
        max_age: Duration,
        now: SystemTime,
        stats: &mut StorageCleanupStats,
    ) -> std::io::Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let cutoff = now.checked_sub(max_age).unwrap_or(SystemTime::UNIX_EPOCH);

        let mut deleted = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-JSON files
            if !path.extension().is_some_and(|ext| ext == "json") {
                continue;
            }

            // Skip files we can't get metadata for
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(mtime) = meta.modified() else { continue };

            // Skip files newer than cutoff
            if mtime >= cutoff {
                continue;
            }

            // Delete old file
            let size = meta.len();
            match fs::remove_file(&path) {
                Ok(()) => {
                    deleted += 1;
                    stats.bytes_freed += size;
                }
                Err(e) => {
                    stats
                        .errors
                        .push(format!("Failed to delete {}: {}", path.display(), e));
                }
            }
        }

        Ok(deleted)
    }

    // ========================================================================
    // Storage Health Monitoring
    // ========================================================================

    /// Check storage health status.
    ///
    /// Returns health status with warnings if storage is approaching limits.
    /// Uses configurable thresholds from environment or defaults.
    ///
    /// # Errors
    ///
    /// Returns error if stats cannot be retrieved.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let storage = IntrospectionStorage::default();
    /// let health = storage.check_health()?;
    /// if !health.is_healthy() {
    ///     for warning in &health.warnings {
    ///         println!("Warning: {}", warning);
    ///     }
    ///     if health.cleanup_recommended {
    ///         storage.cleanup()?;
    ///     }
    /// }
    /// ```
    pub fn check_health(&self) -> std::io::Result<StorageHealthStatus> {
        let stats = self.stats()?;
        let mut warnings = Vec::new();
        let mut level = StorageHealthLevel::Healthy;

        // Get thresholds from environment or use defaults (env reads MB, we need bytes)
        let warning_size_mb = env_u64(
            DASHFLOW_STORAGE_WARNING_SIZE_MB,
            DEFAULT_STORAGE_WARNING_SIZE_BYTES / (1024 * 1024),
        );
        let warning_size = warning_size_mb * 1024 * 1024;

        let critical_size_mb = env_u64(
            DASHFLOW_STORAGE_CRITICAL_SIZE_MB,
            DEFAULT_STORAGE_CRITICAL_SIZE_BYTES / (1024 * 1024),
        );
        let critical_size = critical_size_mb * 1024 * 1024;

        let report_warning = env_usize(
            DASHFLOW_STORAGE_REPORT_WARNING_COUNT,
            DEFAULT_REPORT_WARNING_COUNT,
        );

        let plan_warning = env_usize(
            DASHFLOW_STORAGE_PLAN_WARNING_COUNT,
            DEFAULT_PLAN_WARNING_COUNT,
        );

        // Check total size
        if stats.total_size_bytes >= critical_size {
            level = StorageHealthLevel::Critical;
            warnings.push(format!(
                "Storage size critical: {} MB (threshold: {} MB)",
                stats.total_size_bytes / (1024 * 1024),
                critical_size / (1024 * 1024)
            ));
        } else if stats.total_size_bytes >= warning_size {
            level = StorageHealthLevel::Warning;
            warnings.push(format!(
                "Storage size warning: {} MB (threshold: {} MB)",
                stats.total_size_bytes / (1024 * 1024),
                warning_size / (1024 * 1024)
            ));
        }

        // Check report count
        if stats.report_count >= report_warning {
            if level == StorageHealthLevel::Healthy {
                level = StorageHealthLevel::Warning;
            }
            warnings.push(format!(
                "Report count warning: {} (threshold: {})",
                stats.report_count, report_warning
            ));
        }

        // Check plan counts (sum of all status directories)
        let total_plans =
            stats.plan_counts.0 + stats.plan_counts.1 + stats.plan_counts.2 + stats.plan_counts.3;
        if total_plans >= plan_warning * 4 {
            if level == StorageHealthLevel::Healthy {
                level = StorageHealthLevel::Warning;
            }
            warnings.push(format!(
                "Total plan count warning: {} (threshold: {})",
                total_plans,
                plan_warning * 4
            ));
        }

        // Individual status directory checks
        if stats.plan_counts.2 >= plan_warning {
            // implemented
            warnings.push(format!(
                "Implemented plans warning: {} (threshold: {})",
                stats.plan_counts.2, plan_warning
            ));
        }
        if stats.plan_counts.3 >= plan_warning {
            // failed
            warnings.push(format!(
                "Failed plans warning: {} (threshold: {})",
                stats.plan_counts.3, plan_warning
            ));
        }

        let cleanup_recommended = level != StorageHealthLevel::Healthy;

        Ok(StorageHealthStatus {
            level,
            stats,
            warnings,
            cleanup_recommended,
        })
    }
}
