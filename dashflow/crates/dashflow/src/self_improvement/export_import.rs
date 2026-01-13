// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Export/Import API for introspection data.
//!
//! This module provides functionality to:
//! - Export all introspection data to a single JSON archive
//! - Import introspection data from an archive
//! - Selectively export/import specific data types
//! - Validate data during import
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{IntrospectionStorage, ExportConfig, ImportConfig};
//! use dashflow::self_improvement::{export_introspection, import_introspection};
//!
//! // Export all data
//! let storage = IntrospectionStorage::default();
//! let archive = export_introspection(&storage, &ExportConfig::default())?;
//! std::fs::write("backup.json", archive)?;
//!
//! // Import data
//! let archive = std::fs::read_to_string("backup.json")?;
//! let result = import_introspection(&storage, &archive, ImportConfig::default())?;
//! println!("Imported {} reports, {} plans", result.reports_imported, result.plans_imported);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::warn;

use super::error::{Result, SelfImprovementError};
use super::storage::IntrospectionStorage;
use super::types::{ExecutionPlan, Hypothesis, IntrospectionReport};

// =============================================================================
// Archive Format
// =============================================================================

/// Version of the export archive format.
pub const ARCHIVE_VERSION: u32 = 1;

/// An export archive containing introspection data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionArchive {
    /// Archive format version
    pub version: u32,
    /// Timestamp when the archive was created
    pub created_at: DateTime<Utc>,
    /// Optional description of the archive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Source storage path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Exported reports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reports: Vec<IntrospectionReport>,
    /// Exported plans
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plans: Vec<ExecutionPlan>,
    /// Exported hypotheses
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hypotheses: Vec<Hypothesis>,
    /// Metadata about the export
    pub metadata: ArchiveMetadata,
}

/// Metadata about an archive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    /// Number of reports in the archive
    pub report_count: usize,
    /// Number of plans in the archive
    pub plan_count: usize,
    /// Number of hypotheses in the archive
    pub hypothesis_count: usize,
    /// Total size of original data (before serialization)
    #[serde(default)]
    pub original_size_bytes: u64,
}

impl IntrospectionArchive {
    /// Creates a new empty archive.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: ARCHIVE_VERSION,
            created_at: Utc::now(),
            description: None,
            source_path: None,
            reports: Vec::new(),
            plans: Vec::new(),
            hypotheses: Vec::new(),
            metadata: ArchiveMetadata::default(),
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the source path.
    #[must_use]
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Updates the metadata based on current contents.
    ///
    /// This updates counts and estimates the original data size based on
    /// in-memory JSON serialization size of each item.
    pub fn update_metadata(&mut self) {
        self.metadata.report_count = self.reports.len();
        self.metadata.plan_count = self.plans.len();
        self.metadata.hypothesis_count = self.hypotheses.len();

        // Estimate original size from in-memory data
        // Note: This is an approximation based on JSON serialization
        let mut size_estimate: u64 = 0;
        for report in &self.reports {
            size_estimate += serde_json::to_string(report)
                .map(|s| s.len() as u64)
                .unwrap_or(0);
        }
        for plan in &self.plans {
            size_estimate += serde_json::to_string(plan)
                .map(|s| s.len() as u64)
                .unwrap_or(0);
        }
        for hypothesis in &self.hypotheses {
            size_estimate += serde_json::to_string(hypothesis)
                .map(|s| s.len() as u64)
                .unwrap_or(0);
        }
        self.metadata.original_size_bytes = size_estimate;
    }

    /// Serializes the archive to JSON.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserializes an archive from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        let archive: Self = serde_json::from_str(json)?;

        // Validate version
        if archive.version > ARCHIVE_VERSION {
            return Err(SelfImprovementError::ValidationFailed(format!(
                "Archive version {} is newer than supported version {}",
                archive.version, ARCHIVE_VERSION
            )));
        }

        Ok(archive)
    }

    /// Returns true if the archive is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty() && self.plans.is_empty() && self.hypotheses.is_empty()
    }

    /// Returns the total number of items in the archive.
    #[must_use]
    pub fn total_items(&self) -> usize {
        self.reports.len() + self.plans.len() + self.hypotheses.len()
    }
}

impl Default for IntrospectionArchive {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Export Configuration
// =============================================================================

/// Configuration for exporting introspection data.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Include reports in export
    pub include_reports: bool,
    /// Include plans in export
    pub include_plans: bool,
    /// Include hypotheses in export
    pub include_hypotheses: bool,
    /// Optional filter for report IDs
    pub report_ids: Option<HashSet<String>>,
    /// Optional filter for plan IDs
    pub plan_ids: Option<HashSet<String>>,
    /// Optional filter for hypothesis IDs
    pub hypothesis_ids: Option<HashSet<String>>,
    /// Optional description for the archive
    pub description: Option<String>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            include_reports: true,
            include_plans: true,
            include_hypotheses: true,
            report_ids: None,
            plan_ids: None,
            hypothesis_ids: None,
            description: None,
        }
    }
}

impl ExportConfig {
    /// Creates a new export config that exports everything.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Creates a config that only exports reports.
    #[must_use]
    pub fn reports_only() -> Self {
        Self {
            include_reports: true,
            include_plans: false,
            include_hypotheses: false,
            ..Default::default()
        }
    }

    /// Creates a config that only exports plans.
    #[must_use]
    pub fn plans_only() -> Self {
        Self {
            include_reports: false,
            include_plans: true,
            include_hypotheses: false,
            ..Default::default()
        }
    }

    /// Creates a config that only exports hypotheses.
    #[must_use]
    pub fn hypotheses_only() -> Self {
        Self {
            include_reports: false,
            include_plans: false,
            include_hypotheses: true,
            ..Default::default()
        }
    }

    /// Sets a filter for specific report IDs.
    #[must_use]
    pub fn with_report_ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.report_ids = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Sets a filter for specific plan IDs.
    #[must_use]
    pub fn with_plan_ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.plan_ids = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Sets a filter for specific hypothesis IDs.
    #[must_use]
    pub fn with_hypothesis_ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.hypothesis_ids = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the archive description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// =============================================================================
// Import Configuration
// =============================================================================

/// What to do when an item already exists during import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictResolution {
    /// Skip items that already exist
    #[default]
    Skip,
    /// Overwrite existing items
    Overwrite,
    /// Fail the import if any conflicts exist
    Fail,
}

/// Configuration for importing introspection data.
///
/// # Validation Behavior
///
/// When `validate = true`, the archive is validated for structural issues (nil UUIDs,
/// empty titles/statements). Validation errors are accumulated in [`ImportResult::validation_errors`].
///
/// **Important:** Validation errors only abort the import when `conflict_resolution = Fail`.
/// With `Skip` or `Overwrite` conflict resolution, validation errors are recorded but the
/// import continues. This allows importing partially valid archives while tracking issues.
/// Check [`ImportResult::has_errors()`] after import to detect validation problems.
#[derive(Debug, Clone, Copy)]
pub struct ImportConfig {
    /// Import reports
    pub import_reports: bool,
    /// Import plans
    pub import_plans: bool,
    /// Import hypotheses
    pub import_hypotheses: bool,
    /// How to handle conflicts
    pub conflict_resolution: ConflictResolution,
    /// Validate data before importing. See struct-level docs for behavior details.
    pub validate: bool,
    /// Dry run (don't actually import)
    pub dry_run: bool,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            import_reports: true,
            import_plans: true,
            import_hypotheses: true,
            conflict_resolution: ConflictResolution::Skip,
            validate: true,
            dry_run: false,
        }
    }
}

impl ImportConfig {
    /// Creates a new import config that imports everything.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Sets conflict resolution to overwrite.
    #[must_use]
    pub fn overwrite_conflicts(mut self) -> Self {
        self.conflict_resolution = ConflictResolution::Overwrite;
        self
    }

    /// Sets conflict resolution to fail.
    #[must_use]
    pub fn fail_on_conflicts(mut self) -> Self {
        self.conflict_resolution = ConflictResolution::Fail;
        self
    }

    /// Enables dry run mode.
    #[must_use]
    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// Disables validation.
    #[must_use]
    pub fn skip_validation(mut self) -> Self {
        self.validate = false;
        self
    }
}

// =============================================================================
// Import Result
// =============================================================================

/// Result of an import operation.
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    /// Number of reports imported
    pub reports_imported: usize,
    /// Number of reports skipped (already exist)
    pub reports_skipped: usize,
    /// Number of reports overwritten
    pub reports_overwritten: usize,
    /// Number of plans imported
    pub plans_imported: usize,
    /// Number of plans skipped
    pub plans_skipped: usize,
    /// Number of plans overwritten
    pub plans_overwritten: usize,
    /// Number of hypotheses imported
    pub hypotheses_imported: usize,
    /// Number of hypotheses skipped
    pub hypotheses_skipped: usize,
    /// Number of hypotheses overwritten
    pub hypotheses_overwritten: usize,
    /// Validation errors (if any)
    pub validation_errors: Vec<String>,
    /// Whether this was a dry run
    pub dry_run: bool,
}

impl ImportResult {
    /// Returns the total number of items imported.
    #[must_use]
    pub fn total_imported(&self) -> usize {
        self.reports_imported + self.plans_imported + self.hypotheses_imported
    }

    /// Returns the total number of items skipped.
    #[must_use]
    pub fn total_skipped(&self) -> usize {
        self.reports_skipped + self.plans_skipped + self.hypotheses_skipped
    }

    /// Returns the total number of items overwritten.
    #[must_use]
    pub fn total_overwritten(&self) -> usize {
        self.reports_overwritten + self.plans_overwritten + self.hypotheses_overwritten
    }

    /// Returns true if the import had any validation errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.validation_errors.is_empty()
    }
}

// =============================================================================
// Export Function
// =============================================================================

/// Exports introspection data from storage to an archive.
///
/// # Arguments
/// * `storage` - The storage to export from
/// * `config` - Export configuration
///
/// # Returns
/// A JSON string containing the archive.
pub fn export_introspection(
    storage: &IntrospectionStorage,
    config: &ExportConfig,
) -> Result<String> {
    let mut archive = IntrospectionArchive::new();

    if let Some(ref desc) = config.description {
        archive.description = Some(desc.clone());
    }

    // Export reports
    if config.include_reports {
        let report_ids = storage.list_reports()?;
        for id in report_ids {
            let id_str = id.to_string();
            // Apply filter if set
            if let Some(ref filter) = config.report_ids {
                if !filter.contains(&id_str) {
                    continue;
                }
            }

            match storage.load_report(id) {
                Ok(report) => archive.reports.push(report),
                Err(e) => {
                    warn!(report_id = %id, error = %e, "Failed to load report during export - skipping");
                }
            }
        }
    }

    // Export plans - combine from all status directories
    if config.include_plans {
        let mut all_plans = Vec::new();

        // Collect plans from all status directories (some may not exist, log others)
        match storage.pending_plans() {
            Ok(plans) => all_plans.extend(plans),
            Err(e) => warn!(error = %e, "Failed to load pending plans during export"),
        }
        match storage.approved_plans() {
            Ok(plans) => all_plans.extend(plans),
            Err(e) => warn!(error = %e, "Failed to load approved plans during export"),
        }
        match storage.list_implemented_plans() {
            Ok(plans) => all_plans.extend(plans),
            Err(e) => warn!(error = %e, "Failed to load implemented plans during export"),
        }
        match storage.list_failed_plans() {
            Ok(plans) => all_plans.extend(plans),
            Err(e) => warn!(error = %e, "Failed to load failed plans during export"),
        }

        for plan in all_plans {
            let id_str = plan.id.to_string();
            // Apply filter if set
            if let Some(ref filter) = config.plan_ids {
                if !filter.contains(&id_str) {
                    continue;
                }
            }
            archive.plans.push(plan);
        }
    }

    // Export hypotheses
    if config.include_hypotheses {
        let hypotheses = storage.all_hypotheses()?;
        for hypothesis in hypotheses {
            let id_str = hypothesis.id.to_string();
            // Apply filter if set
            if let Some(ref filter) = config.hypothesis_ids {
                if !filter.contains(&id_str) {
                    continue;
                }
            }
            archive.hypotheses.push(hypothesis);
        }
    }

    archive.update_metadata();
    archive.to_json()
}

// =============================================================================
// Import Function
// =============================================================================

/// Imports introspection data from an archive to storage.
///
/// # Arguments
/// * `storage` - The storage to import into
/// * `json` - JSON string containing the archive
/// * `config` - Import configuration
///
/// # Returns
/// Import result with statistics.
pub fn import_introspection(
    storage: &IntrospectionStorage,
    json: &str,
    config: ImportConfig,
) -> Result<ImportResult> {
    let archive = IntrospectionArchive::from_json(json)?;
    let mut result = ImportResult {
        dry_run: config.dry_run,
        ..Default::default()
    };

    // Validate if requested
    if config.validate {
        validate_archive(&archive, &mut result)?;
        if result.has_errors() && config.conflict_resolution == ConflictResolution::Fail {
            return Err(SelfImprovementError::ValidationFailed(
                result.validation_errors.join("; "),
            ));
        }
    }

    // Import reports
    if config.import_reports {
        for report in &archive.reports {
            let exists = storage.load_report(report.id).is_ok();

            if exists {
                match config.conflict_resolution {
                    ConflictResolution::Skip => {
                        result.reports_skipped += 1;
                        continue;
                    }
                    ConflictResolution::Fail => {
                        return Err(SelfImprovementError::ValidationFailed(format!(
                            "Report {} already exists",
                            report.id
                        )));
                    }
                    ConflictResolution::Overwrite => {
                        if !config.dry_run {
                            storage.save_report(report)?;
                        }
                        result.reports_overwritten += 1;
                        continue;
                    }
                }
            }

            if !config.dry_run {
                storage.save_report(report)?;
            }
            result.reports_imported += 1;
        }
    }

    // Import plans
    if config.import_plans {
        for plan in &archive.plans {
            let exists = storage.load_plan(plan.id).is_ok();

            if exists {
                match config.conflict_resolution {
                    ConflictResolution::Skip => {
                        result.plans_skipped += 1;
                        continue;
                    }
                    ConflictResolution::Fail => {
                        return Err(SelfImprovementError::ValidationFailed(format!(
                            "Plan {} already exists",
                            plan.id
                        )));
                    }
                    ConflictResolution::Overwrite => {
                        if !config.dry_run {
                            storage.save_plan(plan)?;
                        }
                        result.plans_overwritten += 1;
                        continue;
                    }
                }
            }

            if !config.dry_run {
                storage.save_plan(plan)?;
            }
            result.plans_imported += 1;
        }
    }

    // Import hypotheses
    if config.import_hypotheses {
        for hypothesis in &archive.hypotheses {
            let exists = storage.load_hypothesis(hypothesis.id).is_ok();

            if exists {
                match config.conflict_resolution {
                    ConflictResolution::Skip => {
                        result.hypotheses_skipped += 1;
                        continue;
                    }
                    ConflictResolution::Fail => {
                        return Err(SelfImprovementError::ValidationFailed(format!(
                            "Hypothesis {} already exists",
                            hypothesis.id
                        )));
                    }
                    ConflictResolution::Overwrite => {
                        if !config.dry_run {
                            storage.save_hypothesis(hypothesis)?;
                        }
                        result.hypotheses_overwritten += 1;
                        continue;
                    }
                }
            }

            if !config.dry_run {
                storage.save_hypothesis(hypothesis)?;
            }
            result.hypotheses_imported += 1;
        }
    }

    Ok(result)
}

/// Validates an archive before import.
fn validate_archive(archive: &IntrospectionArchive, result: &mut ImportResult) -> Result<()> {
    // Check for empty IDs
    for report in &archive.reports {
        if report.id.is_nil() {
            result
                .validation_errors
                .push("Report has nil UUID".to_string());
        }
    }

    for plan in &archive.plans {
        if plan.id.is_nil() {
            result
                .validation_errors
                .push(format!("Plan '{}' has nil UUID", plan.title));
        }
        if plan.title.is_empty() {
            result
                .validation_errors
                .push(format!("Plan {} has empty title", plan.id));
        }
    }

    for hypothesis in &archive.hypotheses {
        if hypothesis.id.is_nil() {
            result.validation_errors.push(format!(
                "Hypothesis '{}' has nil UUID",
                hypothesis.statement
            ));
        }
        if hypothesis.statement.is_empty() {
            result
                .validation_errors
                .push(format!("Hypothesis {} has empty statement", hypothesis.id));
        }
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::self_improvement::types::{IntrospectionScope, PlanCategory};
    use tempfile::TempDir;

    fn create_test_storage() -> (TempDir, IntrospectionStorage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = IntrospectionStorage::new(temp_dir.path().join(".dashflow/introspection"));
        storage.initialize().unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_archive_creation() {
        let archive = IntrospectionArchive::new()
            .with_description("Test archive")
            .with_source_path("/test/path");

        assert_eq!(archive.version, ARCHIVE_VERSION);
        assert_eq!(archive.description, Some("Test archive".to_string()));
        assert_eq!(archive.source_path, Some("/test/path".to_string()));
        assert!(archive.is_empty());
    }

    #[test]
    fn test_archive_serialization() {
        let mut archive = IntrospectionArchive::new();
        archive
            .reports
            .push(IntrospectionReport::new(IntrospectionScope::System));
        archive.update_metadata();

        let json = archive.to_json().unwrap();
        let restored = IntrospectionArchive::from_json(&json).unwrap();

        assert_eq!(restored.version, archive.version);
        assert_eq!(restored.reports.len(), 1);
        assert_eq!(restored.metadata.report_count, 1);
    }

    #[test]
    fn test_export_config_builders() {
        let config = ExportConfig::reports_only();
        assert!(config.include_reports);
        assert!(!config.include_plans);
        assert!(!config.include_hypotheses);

        let config = ExportConfig::plans_only();
        assert!(!config.include_reports);
        assert!(config.include_plans);
        assert!(!config.include_hypotheses);

        let config = ExportConfig::hypotheses_only();
        assert!(!config.include_reports);
        assert!(!config.include_plans);
        assert!(config.include_hypotheses);
    }

    #[test]
    fn test_export_with_filter() {
        let (_temp_dir, storage) = create_test_storage();

        // Create some reports
        let report1 = IntrospectionReport::new(IntrospectionScope::System);
        let report2 = IntrospectionReport::new(IntrospectionScope::System);
        storage.save_report(&report1).unwrap();
        storage.save_report(&report2).unwrap();

        // Export only one report
        let config = ExportConfig::reports_only().with_report_ids([report1.id.to_string()]);
        let json = export_introspection(&storage, &config).unwrap();
        let archive = IntrospectionArchive::from_json(&json).unwrap();

        assert_eq!(archive.reports.len(), 1);
        assert_eq!(archive.reports[0].id, report1.id);
    }

    #[test]
    fn test_import_result_totals() {
        let result = ImportResult {
            reports_imported: 2,
            reports_skipped: 1,
            reports_overwritten: 0,
            plans_imported: 3,
            plans_skipped: 2,
            plans_overwritten: 1,
            hypotheses_imported: 1,
            hypotheses_skipped: 0,
            hypotheses_overwritten: 0,
            ..Default::default()
        };

        assert_eq!(result.total_imported(), 6);
        assert_eq!(result.total_skipped(), 3);
        assert_eq!(result.total_overwritten(), 1);
    }

    #[test]
    fn test_export_import_roundtrip() {
        let (_temp_dir, storage) = create_test_storage();

        // Create test data
        let report = IntrospectionReport::new(IntrospectionScope::System);
        let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
        let hypothesis = Hypothesis::new("Test hypothesis", "Test rationale");

        storage.save_report(&report).unwrap();
        storage.save_plan(&plan).unwrap();
        storage.save_hypothesis(&hypothesis).unwrap();

        // Export
        let json = export_introspection(&storage, &ExportConfig::all()).unwrap();
        let archive = IntrospectionArchive::from_json(&json).unwrap();

        assert_eq!(archive.reports.len(), 1);
        assert_eq!(archive.plans.len(), 1);
        assert_eq!(archive.hypotheses.len(), 1);

        // Import to new storage
        let (_temp_dir2, storage2) = create_test_storage();
        let result = import_introspection(&storage2, &json, ImportConfig::all()).unwrap();

        assert_eq!(result.reports_imported, 1);
        assert_eq!(result.plans_imported, 1);
        assert_eq!(result.hypotheses_imported, 1);
        assert_eq!(result.total_skipped(), 0);
    }

    #[test]
    fn test_import_skip_existing() {
        let (_temp_dir, storage) = create_test_storage();

        let report = IntrospectionReport::new(IntrospectionScope::System);
        storage.save_report(&report).unwrap();

        // Create archive with the same report
        let mut archive = IntrospectionArchive::new();
        archive.reports.push(report);
        let json = archive.to_json().unwrap();

        // Import with skip (default)
        let result = import_introspection(&storage, &json, ImportConfig::all()).unwrap();
        assert_eq!(result.reports_skipped, 1);
        assert_eq!(result.reports_imported, 0);
    }

    #[test]
    fn test_import_overwrite_existing() {
        let (_temp_dir, storage) = create_test_storage();

        let report = IntrospectionReport::new(IntrospectionScope::System);
        storage.save_report(&report).unwrap();

        // Create archive with the same report
        let mut archive = IntrospectionArchive::new();
        archive.reports.push(report);
        let json = archive.to_json().unwrap();

        // Import with overwrite
        let config = ImportConfig::all().overwrite_conflicts();
        let result = import_introspection(&storage, &json, config).unwrap();
        assert_eq!(result.reports_overwritten, 1);
        assert_eq!(result.reports_imported, 0);
    }

    #[test]
    fn test_import_dry_run() {
        let (_temp_dir, storage) = create_test_storage();

        let mut archive = IntrospectionArchive::new();
        archive
            .reports
            .push(IntrospectionReport::new(IntrospectionScope::System));
        let json = archive.to_json().unwrap();

        // Dry run
        let config = ImportConfig::all().dry_run();
        let result = import_introspection(&storage, &json, config).unwrap();

        assert!(result.dry_run);
        assert_eq!(result.reports_imported, 1);

        // Verify nothing was actually saved
        assert!(storage.list_reports().unwrap().is_empty());
    }

    #[test]
    fn test_conflict_resolution_fail() {
        let (_temp_dir, storage) = create_test_storage();

        let report = IntrospectionReport::new(IntrospectionScope::System);
        storage.save_report(&report).unwrap();

        // Create archive with the same report
        let mut archive = IntrospectionArchive::new();
        archive.reports.push(report);
        let json = archive.to_json().unwrap();

        // Import with fail_on_conflicts
        let config = ImportConfig::all().fail_on_conflicts();
        let result = import_introspection(&storage, &json, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_archive_version_check() {
        let mut archive = IntrospectionArchive::new();
        archive.version = ARCHIVE_VERSION + 1; // Newer version
        let json = serde_json::to_string(&archive).unwrap();

        let result = IntrospectionArchive::from_json(&json);
        assert!(result.is_err());
    }
}
