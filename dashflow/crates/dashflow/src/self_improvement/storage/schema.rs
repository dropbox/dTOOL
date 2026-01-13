// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Schema versioning and validation for self-improvement storage.
//!
//! This module provides:
//! - Schema version tracking for stored data
//! - Automatic migration of older formats
//! - JSON Schema generation and validation

use schemars::schema_for;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Schema Versioning
// ============================================================================

/// Current schema version for stored data.
///
/// Increment this when making breaking changes to storage format:
/// - v1: Initial schema (all existing data)
/// - v2: (future) Add new required fields, change structure, etc.
///
/// Version history:
/// - 1: 2025-12-15 - Initial versioned storage format
pub const SCHEMA_VERSION: u32 = 1;

/// Minimum supported schema version for migration.
/// Data older than this cannot be automatically migrated.
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Versioned wrapper for stored data.
///
/// This wrapper is used to track schema versions for stored JSON files,
/// enabling automatic migration when the schema changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedData<T> {
    /// Schema version of this data.
    #[serde(rename = "_schema_version")]
    pub schema_version: u32,

    /// The actual data payload.
    #[serde(flatten)]
    pub data: T,
}

impl<T> VersionedData<T> {
    /// Create a new versioned wrapper with current schema version.
    #[must_use]
    pub fn new(data: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            data,
        }
    }

    /// Create a versioned wrapper with a specific version.
    #[must_use]
    pub fn with_version(data: T, version: u32) -> Self {
        Self {
            schema_version: version,
            data,
        }
    }

    /// Check if this data needs migration.
    #[must_use]
    pub fn needs_migration(&self) -> bool {
        self.schema_version < SCHEMA_VERSION
    }

    /// Unwrap the data, consuming the versioned wrapper.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.data
    }
}

/// Result of attempting to load and migrate versioned data.
#[derive(Debug, Clone)]
pub struct MigrationResult<T> {
    /// The migrated data.
    pub data: T,
    /// Original schema version.
    pub original_version: u32,
    /// Whether migration was performed.
    pub migrated: bool,
    /// Migration steps applied (if any).
    pub steps: Vec<MigrationStep>,
}

/// A single migration step that was applied.
#[derive(Debug, Clone)]
pub struct MigrationStep {
    /// From version.
    pub from: u32,
    /// To version.
    pub to: u32,
    /// Description of the migration.
    pub description: String,
}

/// Errors that can occur during schema migration.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum MigrationError {
    /// Schema version is too old and cannot be migrated.
    #[error("Schema version {version} is too old (minimum: {minimum})")]
    VersionTooOld {
        /// The version found in the data.
        version: u32,
        /// The minimum supported version.
        minimum: u32,
    },
    /// Schema version is newer than supported (from future version).
    #[error(
        "Schema version {version} is newer than current {current} (created by future version)"
    )]
    VersionTooNew {
        /// The version found in the data.
        version: u32,
        /// The current supported version.
        current: u32,
    },
    /// Data parsing failed.
    #[error("Parse error: {0}")]
    ParseError(String),
    /// Migration step failed.
    #[error("Migration from v{from} to v{to} failed: {reason}")]
    MigrationFailed {
        /// Source version.
        from: u32,
        /// Target version.
        to: u32,
        /// Reason for the failure.
        reason: String,
    },
}

impl From<MigrationError> for std::io::Error {
    fn from(e: MigrationError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    }
}

/// Schema migration registry for handling version upgrades.
///
/// This struct holds migration functions for each version transition.
/// When the schema version is incremented, add a migration function here.
#[derive(Default)]
pub struct SchemaMigrator {
    /// Whether to auto-migrate on load.
    pub auto_migrate: bool,
}

impl SchemaMigrator {
    /// Create a new schema migrator with auto-migration enabled.
    #[must_use]
    pub fn new() -> Self {
        Self { auto_migrate: true }
    }

    /// Create a migrator with auto-migration disabled.
    #[must_use]
    pub fn without_auto_migrate() -> Self {
        Self {
            auto_migrate: false,
        }
    }

    /// Check if a version can be migrated to current.
    #[must_use]
    pub fn can_migrate(&self, version: u32) -> bool {
        version >= MIN_SUPPORTED_SCHEMA_VERSION && version <= SCHEMA_VERSION
    }

    /// Detect schema version from JSON data.
    ///
    /// Returns the version if found, or 1 (legacy) if no version field exists.
    #[must_use]
    pub fn detect_version(json: &str) -> u32 {
        // Parse as generic Value to check for version field
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(version) = value.get("_schema_version").and_then(|v| v.as_u64()) {
                return version as u32;
            }
        }
        // No version field = legacy v1 data
        1
    }

    /// Migrate JSON data to current schema version.
    ///
    /// This function:
    /// 1. Detects the current version
    /// 2. Applies necessary migrations in sequence
    /// 3. Returns the migrated JSON string
    ///
    /// # Errors
    ///
    /// Returns error if version is unsupported or migration fails.
    pub fn migrate_json(&self, json: &str) -> Result<(String, Vec<MigrationStep>), MigrationError> {
        let version = Self::detect_version(json);
        let mut steps = Vec::new();

        // Check version bounds
        if version < MIN_SUPPORTED_SCHEMA_VERSION {
            return Err(MigrationError::VersionTooOld {
                version,
                minimum: MIN_SUPPORTED_SCHEMA_VERSION,
            });
        }

        if version > SCHEMA_VERSION {
            return Err(MigrationError::VersionTooNew {
                version,
                current: SCHEMA_VERSION,
            });
        }

        // Already at current version
        if version == SCHEMA_VERSION {
            return Ok((json.to_string(), steps));
        }

        // Parse JSON for migration
        let mut value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| MigrationError::ParseError(e.to_string()))?;

        // Apply migrations in sequence
        let mut current_version = version;
        while current_version < SCHEMA_VERSION {
            let next_version = current_version + 1;
            let description = self.apply_migration(&mut value, current_version, next_version)?;
            steps.push(MigrationStep {
                from: current_version,
                to: next_version,
                description,
            });
            current_version = next_version;
        }

        // Update version field
        value["_schema_version"] = serde_json::Value::Number(SCHEMA_VERSION.into());

        let migrated_json = serde_json::to_string_pretty(&value)
            .map_err(|e| MigrationError::ParseError(e.to_string()))?;

        Ok((migrated_json, steps))
    }

    /// Apply a single migration step.
    ///
    /// Add new migration cases here when incrementing SCHEMA_VERSION.
    fn apply_migration(
        &self,
        _value: &mut serde_json::Value,
        from: u32,
        to: u32,
    ) -> Result<String, MigrationError> {
        // No migrations currently needed between versions.
        // When migration is needed, add match arms like:
        //   match (from, to) {
        //     (1, 2) => { /* migrate v1 to v2 */ }
        //     (2, 3) => { /* migrate v2 to v3 */ }
        //   }
        Ok(format!("No-op migration from v{from} to v{to}"))
    }

    /// Load and migrate a typed value from JSON.
    ///
    /// # Errors
    ///
    /// Returns error if migration or parsing fails.
    pub fn load_versioned<T: serde::de::DeserializeOwned>(
        &self,
        json: &str,
    ) -> Result<MigrationResult<T>, MigrationError> {
        let original_version = Self::detect_version(json);

        let (migrated_json, steps) = if self.auto_migrate && original_version != SCHEMA_VERSION {
            self.migrate_json(json)?
        } else if original_version > SCHEMA_VERSION {
            return Err(MigrationError::VersionTooNew {
                version: original_version,
                current: SCHEMA_VERSION,
            });
        } else {
            (json.to_string(), Vec::new())
        };

        let data: T = serde_json::from_str(&migrated_json)
            .map_err(|e| MigrationError::ParseError(e.to_string()))?;

        Ok(MigrationResult {
            data,
            original_version,
            migrated: !steps.is_empty(),
            steps,
        })
    }

    /// Serialize data with current schema version.
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails.
    pub fn save_versioned<T: serde::Serialize>(data: &T) -> Result<String, serde_json::Error> {
        // Serialize to Value first
        let mut value = serde_json::to_value(data)?;

        // Add schema version
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "_schema_version".to_string(),
                serde_json::Value::Number(SCHEMA_VERSION.into()),
            );
        }

        serde_json::to_string_pretty(&value)
    }
}

// ============================================================================
// JSON Schema Validation
// ============================================================================

/// Schema validation result.
#[derive(Debug, Clone)]
pub struct SchemaValidationResult {
    /// Whether validation passed.
    pub valid: bool,
    /// Validation errors (if any).
    pub errors: Vec<SchemaValidationError>,
}

/// A single schema validation error.
#[derive(Debug, Clone)]
pub struct SchemaValidationError {
    /// JSON path where the error occurred.
    pub path: String,
    /// Description of the error.
    pub message: String,
}

impl std::fmt::Display for SchemaValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl SchemaValidationResult {
    /// Create a successful validation result.
    #[must_use]
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    /// Create a failed validation result with errors.
    #[must_use]
    pub fn invalid(errors: Vec<SchemaValidationError>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }

    /// Check if validation passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

/// JSON Schema generator and validator for self-improvement types.
///
/// Provides schema generation for documentation and validation of stored data.
pub struct SchemaGenerator;

impl SchemaGenerator {
    /// Generate JSON Schema for IntrospectionReport.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let schema = SchemaGenerator::introspection_report_schema();
    /// println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    /// ```
    #[must_use]
    pub fn introspection_report_schema() -> schemars::Schema {
        schema_for!(crate::self_improvement::types::IntrospectionReport)
    }

    /// Generate JSON Schema for ExecutionPlan.
    #[must_use]
    pub fn execution_plan_schema() -> schemars::Schema {
        schema_for!(crate::self_improvement::types::ExecutionPlan)
    }

    /// Generate JSON Schema for Hypothesis.
    #[must_use]
    pub fn hypothesis_schema() -> schemars::Schema {
        schema_for!(crate::self_improvement::types::Hypothesis)
    }

    /// Generate JSON Schema for CapabilityGap.
    #[must_use]
    pub fn capability_gap_schema() -> schemars::Schema {
        schema_for!(crate::self_improvement::types::CapabilityGap)
    }

    /// Generate JSON Schema for ConsensusResult.
    #[must_use]
    pub fn consensus_result_schema() -> schemars::Schema {
        schema_for!(crate::self_improvement::types::ConsensusResult)
    }

    /// Get all available schemas as a map.
    ///
    /// Returns a map from type name to JSON schema string.
    ///
    /// # Errors
    ///
    /// Returns error if schema serialization fails.
    pub fn all_schemas() -> Result<std::collections::HashMap<String, String>, serde_json::Error> {
        let mut schemas = std::collections::HashMap::new();

        schemas.insert(
            "IntrospectionReport".to_string(),
            serde_json::to_string_pretty(&Self::introspection_report_schema())?,
        );
        schemas.insert(
            "ExecutionPlan".to_string(),
            serde_json::to_string_pretty(&Self::execution_plan_schema())?,
        );
        schemas.insert(
            "Hypothesis".to_string(),
            serde_json::to_string_pretty(&Self::hypothesis_schema())?,
        );
        schemas.insert(
            "CapabilityGap".to_string(),
            serde_json::to_string_pretty(&Self::capability_gap_schema())?,
        );
        schemas.insert(
            "ConsensusResult".to_string(),
            serde_json::to_string_pretty(&Self::consensus_result_schema())?,
        );

        Ok(schemas)
    }

    /// Validate JSON string against the schema for a type.
    ///
    /// This performs basic structural validation by attempting to deserialize
    /// the JSON into the target type.
    ///
    /// # Errors
    ///
    /// Returns validation result indicating success or failure with error details.
    pub fn validate_json<T: serde::de::DeserializeOwned>(json: &str) -> SchemaValidationResult {
        match serde_json::from_str::<T>(json) {
            Ok(_) => SchemaValidationResult::valid(),
            Err(e) => {
                let error = SchemaValidationError {
                    path: format!("line {} column {}", e.line(), e.column()),
                    message: e.to_string(),
                };
                SchemaValidationResult::invalid(vec![error])
            }
        }
    }

    /// Validate an IntrospectionReport JSON string.
    #[must_use]
    pub fn validate_introspection_report(json: &str) -> SchemaValidationResult {
        Self::validate_json::<crate::self_improvement::types::IntrospectionReport>(json)
    }

    /// Validate an ExecutionPlan JSON string.
    #[must_use]
    pub fn validate_execution_plan(json: &str) -> SchemaValidationResult {
        Self::validate_json::<crate::self_improvement::types::ExecutionPlan>(json)
    }

    /// Validate a Hypothesis JSON string.
    #[must_use]
    pub fn validate_hypothesis(json: &str) -> SchemaValidationResult {
        Self::validate_json::<crate::self_improvement::types::Hypothesis>(json)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Schema Version Constants Tests
    // ========================================================================

    #[test]
    fn test_schema_version_is_positive() {
        assert!(SCHEMA_VERSION >= 1, "Schema version must be at least 1");
    }

    #[test]
    fn test_min_supported_version_valid() {
        assert!(
            MIN_SUPPORTED_SCHEMA_VERSION <= SCHEMA_VERSION,
            "Min supported version must not exceed current version"
        );
    }

    // ========================================================================
    // VersionedData Tests
    // ========================================================================

    #[test]
    fn test_versioned_data_new() {
        let data = VersionedData::new("test_data".to_string());
        assert_eq!(data.schema_version, SCHEMA_VERSION);
        assert_eq!(data.data, "test_data");
    }

    #[test]
    fn test_versioned_data_with_version() {
        let data = VersionedData::with_version("test_data".to_string(), 42);
        assert_eq!(data.schema_version, 42);
        assert_eq!(data.data, "test_data");
    }

    #[test]
    fn test_versioned_data_needs_migration_current() {
        let data = VersionedData::new("test".to_string());
        assert!(!data.needs_migration());
    }

    #[test]
    fn test_versioned_data_needs_migration_old() {
        let data = VersionedData::with_version("test".to_string(), SCHEMA_VERSION - 1);
        // Only true if SCHEMA_VERSION > 1
        if SCHEMA_VERSION > 1 {
            assert!(data.needs_migration());
        }
    }

    #[test]
    fn test_versioned_data_into_inner() {
        let data = VersionedData::new(vec![1, 2, 3]);
        let inner = data.into_inner();
        assert_eq!(inner, vec![1, 2, 3]);
    }

    #[test]
    fn test_versioned_data_clone() {
        let data = VersionedData::new("original".to_string());
        let cloned = data.clone();
        assert_eq!(cloned.schema_version, data.schema_version);
        assert_eq!(cloned.data, data.data);
    }

    #[test]
    fn test_versioned_data_serialization() {
        // VersionedData uses #[serde(flatten)] which only works with structs/maps
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestPayload {
            value: String,
        }

        let data = VersionedData::new(TestPayload {
            value: "test".to_string(),
        });
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("_schema_version"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_versioned_data_deserialization() {
        // Note: with flatten, the data fields are at root level
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestPayload {
            value: String,
        }

        let json = format!(r#"{{"_schema_version": {}, "value": "hello"}}"#, SCHEMA_VERSION);
        let data: VersionedData<TestPayload> = serde_json::from_str(&json).unwrap();
        assert_eq!(data.data.value, "hello");

        let json2 = format!(r#"{{"_schema_version": {}}}"#, SCHEMA_VERSION);
        let data: Result<VersionedData<Option<String>>, _> = serde_json::from_str(&json2);
        assert!(data.is_ok());
    }

    // ========================================================================
    // MigrationResult Tests
    // ========================================================================

    #[test]
    fn test_migration_result_fields() {
        let result = MigrationResult {
            data: "migrated".to_string(),
            original_version: 1,
            migrated: true,
            steps: vec![MigrationStep {
                from: 1,
                to: 2,
                description: "test migration".to_string(),
            }],
        };
        assert_eq!(result.data, "migrated");
        assert_eq!(result.original_version, 1);
        assert!(result.migrated);
        assert_eq!(result.steps.len(), 1);
    }

    #[test]
    fn test_migration_step_fields() {
        let step = MigrationStep {
            from: 1,
            to: 2,
            description: "Added new field".to_string(),
        };
        assert_eq!(step.from, 1);
        assert_eq!(step.to, 2);
        assert_eq!(step.description, "Added new field");
    }

    // ========================================================================
    // MigrationError Tests
    // ========================================================================

    #[test]
    fn test_migration_error_version_too_old() {
        let err = MigrationError::VersionTooOld {
            version: 0,
            minimum: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("too old"));
        assert!(msg.contains("0"));
        assert!(msg.contains("1"));
    }

    #[test]
    fn test_migration_error_version_too_new() {
        let err = MigrationError::VersionTooNew {
            version: 99,
            current: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("newer"));
        assert!(msg.contains("99"));
    }

    #[test]
    fn test_migration_error_parse_error() {
        let err = MigrationError::ParseError("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_migration_error_migration_failed() {
        let err = MigrationError::MigrationFailed {
            from: 1,
            to: 2,
            reason: "field missing".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("v1"));
        assert!(msg.contains("v2"));
        assert!(msg.contains("field missing"));
    }

    #[test]
    fn test_migration_error_to_io_error() {
        let err = MigrationError::ParseError("test".to_string());
        let io_err: std::io::Error = err.into();
        assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidData);
    }

    // ========================================================================
    // SchemaMigrator Tests
    // ========================================================================

    #[test]
    fn test_schema_migrator_new() {
        let migrator = SchemaMigrator::new();
        assert!(migrator.auto_migrate);
    }

    #[test]
    fn test_schema_migrator_default() {
        let migrator = SchemaMigrator::default();
        assert!(!migrator.auto_migrate);
    }

    #[test]
    fn test_schema_migrator_without_auto_migrate() {
        let migrator = SchemaMigrator::without_auto_migrate();
        assert!(!migrator.auto_migrate);
    }

    #[test]
    fn test_schema_migrator_can_migrate_current() {
        let migrator = SchemaMigrator::new();
        assert!(migrator.can_migrate(SCHEMA_VERSION));
    }

    #[test]
    fn test_schema_migrator_can_migrate_min_supported() {
        let migrator = SchemaMigrator::new();
        assert!(migrator.can_migrate(MIN_SUPPORTED_SCHEMA_VERSION));
    }

    #[test]
    fn test_schema_migrator_cannot_migrate_too_old() {
        let migrator = SchemaMigrator::new();
        if MIN_SUPPORTED_SCHEMA_VERSION > 0 {
            assert!(!migrator.can_migrate(MIN_SUPPORTED_SCHEMA_VERSION - 1));
        }
    }

    #[test]
    fn test_schema_migrator_cannot_migrate_too_new() {
        let migrator = SchemaMigrator::new();
        assert!(!migrator.can_migrate(SCHEMA_VERSION + 1));
    }

    #[test]
    fn test_detect_version_with_version_field() {
        let json = r#"{"_schema_version": 5, "data": "test"}"#;
        assert_eq!(SchemaMigrator::detect_version(json), 5);
    }

    #[test]
    fn test_detect_version_without_version_field() {
        let json = r#"{"data": "test"}"#;
        assert_eq!(SchemaMigrator::detect_version(json), 1);
    }

    #[test]
    fn test_detect_version_invalid_json() {
        let json = "not valid json";
        assert_eq!(SchemaMigrator::detect_version(json), 1);
    }

    #[test]
    fn test_detect_version_version_not_number() {
        let json = r#"{"_schema_version": "one", "data": "test"}"#;
        assert_eq!(SchemaMigrator::detect_version(json), 1);
    }

    #[test]
    fn test_migrate_json_current_version() {
        let migrator = SchemaMigrator::new();
        let json = format!(r#"{{"_schema_version": {}, "data": "test"}}"#, SCHEMA_VERSION);
        let result = migrator.migrate_json(&json);
        assert!(result.is_ok());
        let (migrated, steps) = result.unwrap();
        assert!(steps.is_empty());
        assert!(migrated.contains("_schema_version"));
    }

    #[test]
    fn test_migrate_json_too_old() {
        let migrator = SchemaMigrator::new();
        if MIN_SUPPORTED_SCHEMA_VERSION > 0 {
            let json = r#"{"_schema_version": 0, "data": "test"}"#;
            let result = migrator.migrate_json(json);
            assert!(matches!(result, Err(MigrationError::VersionTooOld { .. })));
        }
    }

    #[test]
    fn test_migrate_json_too_new() {
        let migrator = SchemaMigrator::new();
        let json = format!(
            r#"{{"_schema_version": {}, "data": "test"}}"#,
            SCHEMA_VERSION + 1
        );
        let result = migrator.migrate_json(&json);
        assert!(matches!(result, Err(MigrationError::VersionTooNew { .. })));
    }

    #[test]
    fn test_migrate_json_invalid_json() {
        let migrator = SchemaMigrator::new();
        // This will be detected as v1 (legacy), but if v1 == SCHEMA_VERSION, no migration needed
        let json = "not valid json";
        if SCHEMA_VERSION == 1 {
            // Legacy data detected as v1, returns original (still invalid)
            let result = migrator.migrate_json(json);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_load_versioned_current_version() {
        let migrator = SchemaMigrator::new();
        let json = format!(
            r#"{{"_schema_version": {}, "value": 42}}"#,
            SCHEMA_VERSION
        );

        #[derive(Debug, serde::Deserialize)]
        struct TestData {
            #[allow(dead_code)] // Test-only field: Required for serde Deserialize
            value: i32,
        }

        let result: Result<MigrationResult<TestData>, _> = migrator.load_versioned(&json);
        assert!(result.is_ok());
        let migration = result.unwrap();
        assert_eq!(migration.original_version, SCHEMA_VERSION);
        assert!(!migration.migrated);
    }

    #[test]
    fn test_load_versioned_too_new_no_auto_migrate() {
        let migrator = SchemaMigrator::without_auto_migrate();
        let json = format!(
            r#"{{"_schema_version": {}, "value": 42}}"#,
            SCHEMA_VERSION + 1
        );

        #[derive(Debug, serde::Deserialize)]
        struct TestData {
            #[allow(dead_code)] // Test-only field: Required for serde Deserialize
            value: i32,
        }

        let result: Result<MigrationResult<TestData>, _> = migrator.load_versioned(&json);
        assert!(matches!(result, Err(MigrationError::VersionTooNew { .. })));
    }

    #[test]
    fn test_save_versioned() {
        #[derive(Debug, serde::Serialize)]
        struct TestData {
            value: i32,
        }

        let data = TestData { value: 42 };
        let result = SchemaMigrator::save_versioned(&data);
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("_schema_version"));
        assert!(json.contains(&SCHEMA_VERSION.to_string()));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_save_versioned_non_object() {
        // When data is not an object, schema version won't be added
        let data = vec![1, 2, 3];
        let result = SchemaMigrator::save_versioned(&data);
        assert!(result.is_ok());
        let json = result.unwrap();
        // Array won't have _schema_version added (only objects get it)
        assert!(json.contains("["));
    }

    // ========================================================================
    // SchemaValidationResult Tests
    // ========================================================================

    #[test]
    fn test_schema_validation_result_valid() {
        let result = SchemaValidationResult::valid();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result.is_valid());
    }

    #[test]
    fn test_schema_validation_result_invalid() {
        let errors = vec![SchemaValidationError {
            path: "/field".to_string(),
            message: "missing required field".to_string(),
        }];
        let result = SchemaValidationResult::invalid(errors);
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(!result.is_valid());
    }

    // ========================================================================
    // SchemaValidationError Tests
    // ========================================================================

    #[test]
    fn test_schema_validation_error_display() {
        let error = SchemaValidationError {
            path: "/nested/field".to_string(),
            message: "type mismatch".to_string(),
        };
        let display = error.to_string();
        assert!(display.contains("/nested/field"));
        assert!(display.contains("type mismatch"));
    }

    #[test]
    fn test_schema_validation_error_clone() {
        let error = SchemaValidationError {
            path: "/field".to_string(),
            message: "error".to_string(),
        };
        let cloned = error.clone();
        assert_eq!(cloned.path, error.path);
        assert_eq!(cloned.message, error.message);
    }

    // ========================================================================
    // SchemaGenerator Tests
    // ========================================================================

    #[test]
    fn test_schema_generator_introspection_report_schema() {
        let schema = SchemaGenerator::introspection_report_schema();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_schema_generator_execution_plan_schema() {
        let schema = SchemaGenerator::execution_plan_schema();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_schema_generator_hypothesis_schema() {
        let schema = SchemaGenerator::hypothesis_schema();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_schema_generator_capability_gap_schema() {
        let schema = SchemaGenerator::capability_gap_schema();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_schema_generator_consensus_result_schema() {
        let schema = SchemaGenerator::consensus_result_schema();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_schema_generator_all_schemas() {
        let result = SchemaGenerator::all_schemas();
        assert!(result.is_ok());
        let schemas = result.unwrap();
        assert!(schemas.contains_key("IntrospectionReport"));
        assert!(schemas.contains_key("ExecutionPlan"));
        assert!(schemas.contains_key("Hypothesis"));
        assert!(schemas.contains_key("CapabilityGap"));
        assert!(schemas.contains_key("ConsensusResult"));
        assert_eq!(schemas.len(), 5);
    }

    #[test]
    fn test_schema_generator_validate_json_valid() {
        #[derive(Debug, serde::Deserialize)]
        struct SimpleData {
            #[allow(dead_code)] // Test-only field: Required for serde Deserialize
            name: String,
        }

        let json = r#"{"name": "test"}"#;
        let result = SchemaGenerator::validate_json::<SimpleData>(json);
        assert!(result.is_valid());
    }

    #[test]
    fn test_schema_generator_validate_json_invalid() {
        #[derive(Debug, serde::Deserialize)]
        struct SimpleData {
            #[allow(dead_code)] // Test-only field: Required for serde Deserialize
            name: String,
        }

        let json = r#"{"wrong_field": "test"}"#;
        let result = SchemaGenerator::validate_json::<SimpleData>(json);
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_schema_generator_validate_json_parse_error() {
        #[derive(Debug, serde::Deserialize)]
        struct SimpleData {
            #[allow(dead_code)] // Test-only field: Required for serde Deserialize
            name: String,
        }

        let json = "not json";
        let result = SchemaGenerator::validate_json::<SimpleData>(json);
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
        // Error should contain line/column info
        assert!(result.errors[0].path.contains("line"));
    }
}
