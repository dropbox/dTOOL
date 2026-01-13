// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for the self-improvement storage system.

use super::*;
use crate::self_improvement::types::{
    CapabilityGap, GapCategory, GapManifestation, IntrospectionScope, PlanCategory,
};
use tempfile::tempdir;

#[test]
fn test_storage_initialization() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));

    assert!(!storage.is_initialized());

    storage.initialize().unwrap();

    assert!(storage.is_initialized());
    assert!(storage.reports_dir().exists());
    assert!(storage.plans_dir().join("pending").exists());
    assert!(storage.plans_dir().join("approved").exists());
    assert!(storage.hypotheses_dir().join("active").exists());
}

#[test]
fn test_report_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    let mut report = IntrospectionReport::new(IntrospectionScope::System);
    report.add_capability_gap(CapabilityGap::new(
        "Test gap",
        GapCategory::MissingTool {
            tool_description: "test".to_string(),
        },
        GapManifestation::Errors {
            count: 1,
            sample_messages: vec![],
        },
    ));

    let (json_path, md_path) = storage.save_report(&report).unwrap();

    assert!(json_path.exists());
    assert!(md_path.exists());

    let loaded = storage.load_report(report.id).unwrap();
    assert_eq!(loaded.id, report.id);
    assert_eq!(loaded.capability_gaps.len(), 1);
}

#[test]
fn test_latest_report() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Initially no reports
    assert!(storage.latest_report().unwrap().is_none());

    // Add a report
    let report = IntrospectionReport::new(IntrospectionScope::System);
    storage.save_report(&report).unwrap();

    let latest = storage.latest_report().unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().id, report.id);
}

#[test]
fn test_plan_lifecycle() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Create and save a plan
    let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement)
        .with_description("A test plan")
        .with_priority(1);

    let path = storage.save_plan(&plan).unwrap();
    assert!(path.exists());

    // Should be in pending
    let pending = storage.pending_plans().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, plan.id);

    // Approve the plan
    storage.approve_plan(plan.id, "AI Worker").unwrap();

    // Should now be in approved
    let pending = storage.pending_plans().unwrap();
    assert!(pending.is_empty());

    let approved = storage.approved_plans().unwrap();
    assert_eq!(approved.len(), 1);

    // Complete the plan
    storage.complete_plan(plan.id, "abc1234").unwrap();

    let approved = storage.approved_plans().unwrap();
    assert!(approved.is_empty());
}

#[test]
fn test_hypothesis_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    let hypothesis = Hypothesis::new("Test hypothesis", "Testing storage");
    storage.save_hypothesis(&hypothesis).unwrap();

    let loaded = storage.load_hypothesis(hypothesis.id).unwrap();
    assert_eq!(loaded.id, hypothesis.id);
    assert_eq!(loaded.statement, "Test hypothesis");

    // List active
    let active = storage.active_hypotheses().unwrap();
    assert_eq!(active.len(), 1);

    // Evaluate
    storage.evaluate_hypothesis(hypothesis).unwrap();

    let active = storage.active_hypotheses().unwrap();
    assert!(active.is_empty());
}

// ========================================================================
// Generic Storable Trait Tests
// ========================================================================

#[test]
fn test_generic_storable_plan() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Create a plan using the specific method
    let plan = ExecutionPlan::new("Generic Test Plan", PlanCategory::Optimization);
    let plan_id = plan.id;

    // Save using generic method
    let path = storage.save(&plan).unwrap();
    assert!(path.exists());
    assert!(path.to_string_lossy().contains("pending"));

    // Load using generic method
    let loaded: ExecutionPlan = storage.load(plan_id).unwrap();
    assert_eq!(loaded.id, plan_id);
    assert_eq!(loaded.title, "Generic Test Plan");

    // List using generic method
    let all_plans: Vec<ExecutionPlan> = storage.list().unwrap();
    assert!(!all_plans.is_empty());

    // Delete using generic method
    storage.delete::<ExecutionPlan>(plan_id).unwrap();
    assert!(storage.load::<ExecutionPlan>(plan_id).is_err());
}

#[test]
fn test_generic_storable_hypothesis() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Create a hypothesis
    let hypothesis = Hypothesis::new("Generic Test Hypothesis", "Testing the Storable trait");
    let hyp_id = hypothesis.id;

    // Save using generic method
    let path = storage.save(&hypothesis).unwrap();
    assert!(path.exists());
    assert!(path.to_string_lossy().contains("active"));

    // Load using generic method
    let loaded: Hypothesis = storage.load(hyp_id).unwrap();
    assert_eq!(loaded.id, hyp_id);
    assert_eq!(loaded.statement, "Generic Test Hypothesis");

    // List using generic method
    let all_hypotheses: Vec<Hypothesis> = storage.list().unwrap();
    assert!(!all_hypotheses.is_empty());

    // Delete using generic method
    storage.delete::<Hypothesis>(hyp_id).unwrap();
    assert!(storage.load::<Hypothesis>(hyp_id).is_err());
}

#[tokio::test]
async fn test_generic_storable_async() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    // Test async plan operations
    let plan = ExecutionPlan::new("Async Generic Plan", PlanCategory::PlatformImprovement);
    let plan_id = plan.id;

    let path = storage.save_async(&plan).await.unwrap();
    assert!(path.exists());

    let loaded: ExecutionPlan = storage.load_async(plan_id).await.unwrap();
    assert_eq!(loaded.id, plan_id);

    // Test async hypothesis operations
    let hypothesis = Hypothesis::new("Async Hypothesis", "Testing async storage");
    let hyp_id = hypothesis.id;

    let path = storage.save_async(&hypothesis).await.unwrap();
    assert!(path.exists());

    let loaded: Hypothesis = storage.load_async(hyp_id).await.unwrap();
    assert_eq!(loaded.id, hyp_id);
}

// ========================================================================
// Async Storage Tests
// ========================================================================

#[tokio::test]
async fn test_async_storage_initialization() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));

    assert!(!storage.is_initialized());

    storage.initialize_async().await.unwrap();

    assert!(storage.is_initialized());
    assert!(storage.reports_dir().exists());
    assert!(storage.plans_dir().join("pending").exists());
    assert!(storage.plans_dir().join("approved").exists());
    assert!(storage.hypotheses_dir().join("active").exists());
}

#[tokio::test]
async fn test_async_report_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    let mut report = IntrospectionReport::new(IntrospectionScope::System);
    report.add_capability_gap(CapabilityGap::new(
        "Test async gap",
        GapCategory::MissingTool {
            tool_description: "async test".to_string(),
        },
        GapManifestation::Errors {
            count: 1,
            sample_messages: vec![],
        },
    ));

    let (json_path, md_path) = storage.save_report_async(&report).await.unwrap();

    assert!(json_path.exists());
    assert!(md_path.exists());

    let loaded = storage.load_report_async(report.id).await.unwrap();
    assert_eq!(loaded.id, report.id);
    assert_eq!(loaded.capability_gaps.len(), 1);
}

#[tokio::test]
async fn test_async_latest_report() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    // Initially no reports
    assert!(storage.latest_report_async().await.unwrap().is_none());

    // Add a report
    let report = IntrospectionReport::new(IntrospectionScope::System);
    storage.save_report_async(&report).await.unwrap();

    let latest = storage.latest_report_async().await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().id, report.id);
}

#[tokio::test]
async fn test_async_plan_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    let plan = ExecutionPlan::new("Async Test Plan", PlanCategory::ApplicationImprovement)
        .with_description("A test plan for async operations")
        .with_priority(1);

    let path = storage.save_plan_async(&plan).await.unwrap();
    assert!(path.exists());

    // Should be in pending
    let pending = storage.pending_plans_async().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, plan.id);

    // Load by ID
    let loaded = storage.load_plan_async(plan.id).await.unwrap();
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.title, "Async Test Plan");
}

#[tokio::test]
async fn test_async_hypothesis_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    let hypothesis = Hypothesis::new("Async test hypothesis", "Testing async storage");
    storage.save_hypothesis_async(&hypothesis).await.unwrap();

    let loaded = storage.load_hypothesis_async(hypothesis.id).await.unwrap();
    assert_eq!(loaded.id, hypothesis.id);
    assert_eq!(loaded.statement, "Async test hypothesis");
}

#[tokio::test]
async fn test_async_stats() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    // Initially empty
    let stats = storage.stats_async().await.unwrap();
    assert_eq!(stats.report_count, 0);
    assert_eq!(stats.plan_counts, (0, 0, 0, 0));

    // Add a report
    let report = IntrospectionReport::new(IntrospectionScope::System);
    storage.save_report_async(&report).await.unwrap();

    let stats = storage.stats_async().await.unwrap();
    assert_eq!(stats.report_count, 1);
    assert!(stats.total_size_bytes > 0);
}

#[tokio::test]
async fn test_async_health_check() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize_async().await.unwrap();

    let health = storage.check_health_async().await.unwrap();
    assert!(health.is_healthy());
    assert!(!health.has_warnings());
    assert!(!health.cleanup_recommended);
}

// ========================================================================
// Graceful Degradation Tests
// ========================================================================

#[test]
fn test_degraded_mode_initial_state() {
    let mode = DegradedMode::new();
    assert!(!mode.is_degraded());
    assert!(!mode.storage_unavailable);
    assert!(!mode.prometheus_unavailable);
    assert!(!mode.alerts_unavailable);
    assert!(mode.degraded_since.is_none());
    assert_eq!(mode.summary(), "All systems operational");
}

#[test]
fn test_degraded_mode_mark_failed() {
    let mut mode = DegradedMode::new();

    mode.mark_failed(DegradedComponent::Storage, "Permission denied");

    assert!(mode.is_degraded());
    assert!(mode.storage_unavailable);
    assert!(!mode.prometheus_unavailable);
    assert!(mode.degraded_since.is_some());
    assert_eq!(mode.failures.len(), 1);
    assert_eq!(mode.failures[0].error, "Permission denied");
    assert_eq!(mode.failures[0].consecutive_failures, 1);
}

#[test]
fn test_degraded_mode_mark_recovered() {
    let mut mode = DegradedMode::new();

    mode.mark_failed(DegradedComponent::Storage, "Error");
    assert!(mode.is_degraded());

    mode.mark_recovered(DegradedComponent::Storage);
    assert!(!mode.is_degraded());
    assert!(!mode.storage_unavailable);
    assert!(mode.degraded_since.is_none());
}

#[test]
fn test_degraded_mode_multiple_components() {
    let mut mode = DegradedMode::new();

    mode.mark_failed(DegradedComponent::Storage, "Storage error");
    mode.mark_failed(DegradedComponent::Prometheus, "Connection refused");

    assert!(mode.is_degraded());
    assert!(mode.storage_unavailable);
    assert!(mode.prometheus_unavailable);

    // Recovering one still leaves system degraded
    mode.mark_recovered(DegradedComponent::Storage);
    assert!(mode.is_degraded());
    assert!(mode.prometheus_unavailable);

    // Recovering all clears degraded state
    mode.mark_recovered(DegradedComponent::Prometheus);
    assert!(!mode.is_degraded());
}

#[test]
fn test_degraded_mode_summary() {
    let mut mode = DegradedMode::new();

    mode.mark_failed(DegradedComponent::Storage, "Error");
    let summary = mode.summary();
    assert!(summary.contains("Storage"));
    assert!(summary.contains("unavailable"));
}

#[test]
fn test_degraded_mode_is_component_degraded() {
    let mut mode = DegradedMode::new();

    assert!(!mode.is_component_degraded(DegradedComponent::Storage));
    assert!(!mode.is_component_degraded(DegradedComponent::Prometheus));

    mode.mark_failed(DegradedComponent::Storage, "Error");

    assert!(mode.is_component_degraded(DegradedComponent::Storage));
    assert!(!mode.is_component_degraded(DegradedComponent::Prometheus));
}

#[test]
fn test_degraded_result_ok() {
    let result = DegradedResult::ok(42);

    assert!(!result.is_degraded());
    assert!(result.degraded_component.is_none());
    assert!(result.warning.is_none());
    assert_eq!(result.into_value(), 42);
}

#[test]
fn test_degraded_result_degraded() {
    let result = DegradedResult::degraded(
        Vec::<String>::new(),
        DegradedComponent::Storage,
        "Storage unavailable, using empty list",
    );

    assert!(result.is_degraded());
    assert_eq!(result.degraded_component, Some(DegradedComponent::Storage));
    assert!(result.warning.is_some());
    assert!(result.value.is_empty());
}

#[test]
fn test_degraded_result_degraded_default() {
    let result: DegradedResult<Vec<i32>> =
        DegradedResult::degraded_default(DegradedComponent::Prometheus, "No metrics");

    assert!(result.is_degraded());
    assert!(result.value.is_empty());
}

#[test]
fn test_degraded_component_display() {
    assert_eq!(format!("{}", DegradedComponent::Storage), "Storage");
    assert_eq!(format!("{}", DegradedComponent::Prometheus), "Prometheus");
    assert_eq!(format!("{}", DegradedComponent::Alerts), "Alerts");
    assert_eq!(
        format!("{}", DegradedComponent::TraceWatcher),
        "TraceWatcher"
    );
}

// ========================================================================
// Schema Versioning Tests
// ========================================================================

#[test]
fn test_schema_version_constant() {
    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(MIN_SUPPORTED_SCHEMA_VERSION, 1);
}

#[test]
fn test_versioned_data_wrapper() {
    let data = "test data".to_string();
    let versioned = VersionedData::new(data.clone());

    assert_eq!(versioned.schema_version, SCHEMA_VERSION);
    assert_eq!(versioned.data, data);
    assert!(!versioned.needs_migration());
}

#[test]
fn test_versioned_data_needs_migration() {
    let data = "test".to_string();
    let versioned = VersionedData::with_version(data, 0);

    assert!(versioned.needs_migration());
}

#[test]
fn test_schema_migrator_detect_version() {
    // No version field = legacy v1
    let json = r#"{"id": "test"}"#;
    assert_eq!(SchemaMigrator::detect_version(json), 1);

    // With version field
    let json = r#"{"_schema_version": 2, "id": "test"}"#;
    assert_eq!(SchemaMigrator::detect_version(json), 2);

    // Invalid JSON = v1
    let json = "invalid json";
    assert_eq!(SchemaMigrator::detect_version(json), 1);
}

#[test]
fn test_schema_migrator_can_migrate() {
    let migrator = SchemaMigrator::new();

    assert!(migrator.can_migrate(1));
    assert!(migrator.can_migrate(SCHEMA_VERSION));
    assert!(!migrator.can_migrate(0)); // Too old
    assert!(!migrator.can_migrate(SCHEMA_VERSION + 1)); // Too new
}

#[test]
fn test_schema_migrator_save_versioned() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    let data = TestData {
        name: "test".to_string(),
        value: 42,
    };

    let json = SchemaMigrator::save_versioned(&data).unwrap();

    // Verify version field is present
    assert!(json.contains("\"_schema_version\""));
    assert!(json.contains(&format!("{}", SCHEMA_VERSION)));

    // Verify data is preserved
    assert!(json.contains("\"name\""));
    assert!(json.contains("\"test\""));
}

#[test]
fn test_schema_migrator_load_versioned() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
    }

    let migrator = SchemaMigrator::new();

    // Load legacy data (no version)
    let json = r#"{"name": "legacy"}"#;
    let result = migrator.load_versioned::<TestData>(json).unwrap();

    assert_eq!(result.data.name, "legacy");
    assert_eq!(result.original_version, 1);
    assert!(!result.migrated); // v1 is current

    // Load versioned data
    let json = r#"{"_schema_version": 1, "name": "versioned"}"#;
    let result = migrator.load_versioned::<TestData>(json).unwrap();

    assert_eq!(result.data.name, "versioned");
    assert_eq!(result.original_version, 1);
    assert!(!result.migrated);
}

#[test]
fn test_schema_migrator_version_too_new() {
    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct TestData {
        name: String,
    }

    let migrator = SchemaMigrator::new();
    let json = format!(
        r#"{{"_schema_version": {}, "name": "future"}}"#,
        SCHEMA_VERSION + 1
    );

    let result = migrator.load_versioned::<TestData>(&json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MigrationError::VersionTooNew { .. }
    ));
}

#[test]
fn test_migration_error_display() {
    let err = MigrationError::VersionTooOld {
        version: 0,
        minimum: 1,
    };
    assert!(err.to_string().contains("too old"));

    let err = MigrationError::VersionTooNew {
        version: 5,
        current: 1,
    };
    assert!(err.to_string().contains("newer than current"));

    let err = MigrationError::ParseError("invalid json".to_string());
    assert!(err.to_string().contains("Parse error"));
}

#[test]
fn test_versioned_storage_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    assert!(storage.is_versioned());

    // Save a report
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let (json_path, _) = storage.save_report(&report).unwrap();

    // Verify JSON contains version
    let contents = std::fs::read_to_string(&json_path).unwrap();
    assert!(contents.contains("\"_schema_version\""));

    // Load and verify
    let loaded = storage.load_report(report.id).unwrap();
    assert_eq!(loaded.id, report.id);
}

#[test]
fn test_versioned_storage_disabled() {
    let dir = tempdir().unwrap();
    let storage =
        IntrospectionStorage::new(dir.path().join("introspection")).with_versioning(false);
    storage.initialize().unwrap();

    assert!(!storage.is_versioned());

    // Save a report
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let (json_path, _) = storage.save_report(&report).unwrap();

    // Verify JSON does NOT contain version
    let contents = std::fs::read_to_string(&json_path).unwrap();
    assert!(!contents.contains("\"_schema_version\""));

    // Load and verify (still works)
    let loaded = storage.load_report(report.id).unwrap();
    assert_eq!(loaded.id, report.id);
}

#[test]
fn test_versioned_plan_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
    let path = storage.save_plan(&plan).unwrap();

    // Verify version in JSON
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("\"_schema_version\""));

    // Load and verify
    let loaded = storage.load_plan(plan.id).unwrap();
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.title, "Test Plan");
}

#[test]
fn test_versioned_hypothesis_save_and_load() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    let hyp = Hypothesis::new("Test hypothesis", "For testing");
    storage.save_hypothesis(&hyp).unwrap();

    let loaded = storage.load_hypothesis(hyp.id).unwrap();
    assert_eq!(loaded.id, hyp.id);
    assert_eq!(loaded.statement, "Test hypothesis");
}

#[test]
fn test_migration_result_fields() {
    let result = MigrationResult {
        data: "test".to_string(),
        original_version: 1,
        migrated: false,
        steps: vec![],
    };

    assert!(!result.migrated);
    assert!(result.steps.is_empty());
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
    assert!(!step.description.is_empty());
}

// ========================================================================
// JSON Schema Validation Tests
// ========================================================================

#[test]
fn test_schema_generator_introspection_report() {
    let schema = SchemaGenerator::introspection_report_schema();
    let json = serde_json::to_string_pretty(&schema).unwrap();

    // Schema should contain type information
    assert!(json.contains("IntrospectionReport"));
    assert!(!json.is_empty());
}

#[test]
fn test_schema_generator_execution_plan() {
    let schema = SchemaGenerator::execution_plan_schema();
    let json = serde_json::to_string_pretty(&schema).unwrap();

    assert!(json.contains("ExecutionPlan"));
    assert!(!json.is_empty());
}

#[test]
fn test_schema_generator_hypothesis() {
    let schema = SchemaGenerator::hypothesis_schema();
    let json = serde_json::to_string_pretty(&schema).unwrap();

    assert!(json.contains("Hypothesis"));
    assert!(!json.is_empty());
}

#[test]
fn test_schema_generator_all_schemas() {
    let schemas = SchemaGenerator::all_schemas().unwrap();

    assert!(schemas.contains_key("IntrospectionReport"));
    assert!(schemas.contains_key("ExecutionPlan"));
    assert!(schemas.contains_key("Hypothesis"));
    assert!(schemas.contains_key("CapabilityGap"));
    assert!(schemas.contains_key("ConsensusResult"));
    assert_eq!(schemas.len(), 5);
}

#[test]
fn test_schema_validation_valid_report() {
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let json = report.to_json().unwrap();

    let result = SchemaGenerator::validate_introspection_report(&json);
    assert!(result.is_valid());
    assert!(result.errors.is_empty());
}

#[test]
fn test_schema_validation_invalid_json() {
    let invalid_json = r#"{"id": "not-a-uuid", "timestamp": "invalid"}"#;

    let result = SchemaGenerator::validate_introspection_report(invalid_json);
    assert!(!result.is_valid());
    assert!(!result.errors.is_empty());
}

#[test]
fn test_schema_validation_result_methods() {
    let valid = SchemaValidationResult::valid();
    assert!(valid.is_valid());
    assert!(valid.errors.is_empty());

    let error = SchemaValidationError {
        path: "/id".to_string(),
        message: "Invalid UUID".to_string(),
    };
    let invalid = SchemaValidationResult::invalid(vec![error.clone()]);
    assert!(!invalid.is_valid());
    assert_eq!(invalid.errors.len(), 1);

    // Test error display
    assert!(error.to_string().contains("Invalid UUID"));
}

#[test]
fn test_schema_validation_execution_plan() {
    let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
    let json = serde_json::to_string(&plan).unwrap();

    let result = SchemaGenerator::validate_execution_plan(&json);
    assert!(result.is_valid());
}

#[test]
fn test_schema_validation_hypothesis() {
    let hyp = Hypothesis::new("Test statement", "Test origin");
    let json = serde_json::to_string(&hyp).unwrap();

    let result = SchemaGenerator::validate_hypothesis(&json);
    assert!(result.is_valid());
}

// ========================================================================
// Backward Compatibility Tests
// ========================================================================

#[test]
fn test_backward_compat_load_unversioned_report() {
    // Create a report without schema version (simulating legacy data)
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let unversioned_json = serde_json::to_string_pretty(&report).unwrap();

    // Verify no version field in raw JSON
    assert!(!unversioned_json.contains("_schema_version"));

    // Versioned storage should still load it
    let migrator = SchemaMigrator::new();
    let result = migrator
        .load_versioned::<IntrospectionReport>(&unversioned_json)
        .unwrap();

    // Should detect as version 1 (legacy)
    assert_eq!(result.original_version, 1);
    assert!(!result.migrated); // v1 is current, no migration needed
    assert_eq!(result.data.id, report.id);
}

#[test]
fn test_backward_compat_load_unversioned_plan() {
    // Create a plan without schema version
    let plan = ExecutionPlan::new("Legacy Plan", PlanCategory::ApplicationImprovement);
    let unversioned_json = serde_json::to_string_pretty(&plan).unwrap();

    // Verify no version field
    assert!(!unversioned_json.contains("_schema_version"));

    // Should still load correctly
    let migrator = SchemaMigrator::new();
    let result = migrator
        .load_versioned::<ExecutionPlan>(&unversioned_json)
        .unwrap();

    assert_eq!(result.data.id, plan.id);
    assert_eq!(result.data.title, "Legacy Plan");
}

#[test]
fn test_backward_compat_load_unversioned_hypothesis() {
    // Create hypothesis without schema version
    let hyp = Hypothesis::new("Legacy hypothesis", "Legacy origin");
    let unversioned_json = serde_json::to_string_pretty(&hyp).unwrap();

    // Should load correctly
    let migrator = SchemaMigrator::new();
    let result = migrator
        .load_versioned::<Hypothesis>(&unversioned_json)
        .unwrap();

    assert_eq!(result.data.id, hyp.id);
    assert_eq!(result.data.statement, "Legacy hypothesis");
}

#[test]
fn test_backward_compat_versioned_roundtrip() {
    // Save with versioning enabled
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let versioned_json = SchemaMigrator::save_versioned(&report).unwrap();

    // Verify version is present
    assert!(versioned_json.contains("_schema_version"));

    // Load with versioning
    let migrator = SchemaMigrator::new();
    let result = migrator
        .load_versioned::<IntrospectionReport>(&versioned_json)
        .unwrap();

    // Should match original
    assert_eq!(result.original_version, SCHEMA_VERSION);
    assert!(!result.migrated);
    assert_eq!(result.data.id, report.id);
}

#[test]
fn test_backward_compat_mixed_storage() {
    // Test that versioned storage can handle both versioned and unversioned files
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Save with versioning (default)
    let report1 = IntrospectionReport::new(IntrospectionScope::System);
    storage.save_report(&report1).unwrap();

    // Create an unversioned file manually (simulating legacy data)
    let report2 = IntrospectionReport::new(IntrospectionScope::System);
    let unversioned_json = serde_json::to_string_pretty(&report2).unwrap();
    let legacy_path = storage.reports_dir().join(format!(
        "{}_legacy.json",
        report2.timestamp.format("%Y-%m-%dT%H-%M-%S")
    ));
    std::fs::write(&legacy_path, &unversioned_json).unwrap();

    // Both should be loadable
    let loaded1 = storage.load_report(report1.id).unwrap();
    assert_eq!(loaded1.id, report1.id);

    // List should include both
    let all_ids = storage.list_reports().unwrap();
    assert!(all_ids.contains(&report1.id));
}

#[test]
fn test_backward_compat_plan_roundtrip_with_versioning() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Save versioned
    let plan = ExecutionPlan::new("Roundtrip Plan", PlanCategory::Optimization);
    let path = storage.save_plan(&plan).unwrap();

    // Read raw JSON and verify version
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("_schema_version"));

    // Load through storage (with migration support)
    let loaded = storage.load_plan(plan.id).unwrap();
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.title, "Roundtrip Plan");
}

#[test]
fn test_backward_compat_hypothesis_roundtrip() {
    let dir = tempdir().unwrap();
    let storage = IntrospectionStorage::new(dir.path().join("introspection"));
    storage.initialize().unwrap();

    // Save
    let hyp = Hypothesis::new("Roundtrip hypothesis", "Test");
    storage.save_hypothesis(&hyp).unwrap();

    // Load
    let loaded = storage.load_hypothesis(hyp.id).unwrap();
    assert_eq!(loaded.id, hyp.id);
    assert_eq!(loaded.statement, "Roundtrip hypothesis");
}

#[test]
fn test_backward_compat_version_too_new_error() {
    // Test that future version data is rejected
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let mut json_value: serde_json::Value = serde_json::to_value(&report).unwrap();

    // Add a future version
    json_value["_schema_version"] = serde_json::Value::Number((SCHEMA_VERSION + 10).into());
    let future_json = serde_json::to_string(&json_value).unwrap();

    // Should fail to load
    let migrator = SchemaMigrator::new();
    let result = migrator.load_versioned::<IntrospectionReport>(&future_json);

    assert!(result.is_err());
    match result.unwrap_err() {
        MigrationError::VersionTooNew { version, current } => {
            assert_eq!(version, SCHEMA_VERSION + 10);
            assert_eq!(current, SCHEMA_VERSION);
        }
        _ => panic!("Expected VersionTooNew error"),
    }
}

#[test]
fn test_backward_compat_without_versioning_disabled() {
    // Test with versioning disabled
    let dir = tempdir().unwrap();
    let storage =
        IntrospectionStorage::new(dir.path().join("introspection")).with_versioning(false);
    storage.initialize().unwrap();

    // Save without versioning
    let report = IntrospectionReport::new(IntrospectionScope::System);
    let (json_path, _) = storage.save_report(&report).unwrap();

    // Verify NO version field
    let raw = std::fs::read_to_string(&json_path).unwrap();
    assert!(!raw.contains("_schema_version"));

    // Should still load correctly
    let loaded = storage.load_report(report.id).unwrap();
    assert_eq!(loaded.id, report.id);
}
