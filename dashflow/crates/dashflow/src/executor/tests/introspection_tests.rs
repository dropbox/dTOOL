//! Tests for executor introspection types.
//!
//! Tests for GraphIntrospection and UnifiedIntrospection types that provide
//! AI agents with self-awareness capabilities.

use crate::executor::introspection::{GraphIntrospection, UnifiedIntrospection};
use crate::introspection::{CapabilityManifest, GraphManifest};
use crate::live_introspection::{ExecutionSummary, LiveExecutionStatus};
use crate::platform_introspection::PlatformIntrospection;
use crate::platform_registry::{AppArchitecture, PlatformRegistry};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_app_architecture() -> AppArchitecture {
    AppArchitecture::builder()
        .graph_structure(crate::platform_registry::ArchitectureGraphInfo {
            name: Some("Test Graph".to_string()),
            entry_point: "start".to_string(),
            node_count: 2,
            edge_count: 1,
            node_names: vec!["start".to_string(), "end".to_string()],
            has_cycles: false,
            has_conditional_edges: false,
            has_parallel_edges: false,
        })
        .metadata(crate::platform_registry::ArchitectureMetadata {
            dashflow_version: "1.0.0".to_string(),
            analyzed_at: Some("2025-01-01T00:00:00Z".to_string()),
            notes: vec![],
        })
        .build()
}

fn create_test_graph_introspection() -> GraphIntrospection {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .graph_name("Test Graph")
        .build()
        .unwrap();

    let platform = PlatformRegistry::discover();
    let architecture = create_test_app_architecture();
    let capabilities = CapabilityManifest::default();

    GraphIntrospection {
        manifest,
        platform,
        architecture,
        capabilities,
    }
}

fn create_test_execution_summary(
    execution_id: &str,
    status: LiveExecutionStatus,
) -> ExecutionSummary {
    ExecutionSummary {
        execution_id: execution_id.to_string(),
        graph_name: "Test Graph".to_string(),
        started_at: "2025-01-01T00:00:00Z".to_string(),
        current_node: "node1".to_string(),
        iteration: 1,
        status,
    }
}

fn create_test_unified_introspection(live: Vec<ExecutionSummary>) -> UnifiedIntrospection {
    let app = create_test_graph_introspection();
    let platform = PlatformIntrospection::discover();

    UnifiedIntrospection { platform, app, live }
}

// ============================================================================
// GraphIntrospection Tests
// ============================================================================

#[test]
fn test_graph_introspection_construction() {
    let introspection = create_test_graph_introspection();

    // Verify the struct was constructed correctly
    assert_eq!(
        introspection.manifest.graph_name,
        Some("Test Graph".to_string())
    );
}

#[test]
fn test_graph_introspection_to_json() {
    let introspection = create_test_graph_introspection();

    let json = introspection.to_json().expect("should serialize to JSON");

    // Verify the JSON structure
    assert!(json.contains("\"manifest\":{"));
    assert!(json.contains("\"platform\":{"));
    assert!(json.contains("\"architecture\":{"));
    assert!(json.contains("\"capabilities\":{"));

    // Verify the graph name is in the JSON
    assert!(json.contains("Test Graph"));
}

#[test]
fn test_graph_introspection_json_parseable() {
    let introspection = create_test_graph_introspection();

    let json = introspection.to_json().expect("should serialize");

    // Verify the JSON is valid and parseable
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

    assert!(parsed.get("manifest").is_some());
    assert!(parsed.get("platform").is_some());
    assert!(parsed.get("architecture").is_some());
    assert!(parsed.get("capabilities").is_some());
}

#[test]
fn test_graph_introspection_clone() {
    let introspection = create_test_graph_introspection();
    let cloned = introspection.clone();

    assert_eq!(cloned.manifest.graph_name, introspection.manifest.graph_name);
}

#[test]
fn test_graph_introspection_debug() {
    let introspection = create_test_graph_introspection();

    let debug_str = format!("{:?}", introspection);
    assert!(debug_str.contains("GraphIntrospection"));
}

#[test]
fn test_graph_introspection_with_custom_name() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .graph_name("Custom Name")
        .build()
        .unwrap();

    let platform = PlatformRegistry::discover();
    let architecture = create_test_app_architecture();
    let capabilities = CapabilityManifest::default();

    let introspection = GraphIntrospection {
        manifest,
        platform,
        architecture,
        capabilities,
    };

    let json = introspection.to_json().expect("should serialize");
    assert!(json.contains("Custom Name"));
}

// ============================================================================
// UnifiedIntrospection Tests
// ============================================================================

#[test]
fn test_unified_introspection_empty_live() {
    let unified = create_test_unified_introspection(vec![]);

    assert_eq!(unified.live.len(), 0);
    assert_eq!(unified.active_execution_count(), 0);
    assert!(!unified.has_active_executions());
}

#[test]
fn test_unified_introspection_active_running() {
    let executions = vec![create_test_execution_summary("exec1", LiveExecutionStatus::Running)];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.live.len(), 1);
    assert_eq!(unified.active_execution_count(), 1);
    assert!(unified.has_active_executions());
}

#[test]
fn test_unified_introspection_active_paused() {
    let executions = vec![create_test_execution_summary("exec1", LiveExecutionStatus::Paused)];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 1);
    assert!(unified.has_active_executions());
}

#[test]
fn test_unified_introspection_active_waiting_for_input() {
    let executions = vec![create_test_execution_summary(
        "exec1",
        LiveExecutionStatus::WaitingForInput,
    )];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 1);
    assert!(unified.has_active_executions());
}

#[test]
fn test_unified_introspection_completed_not_active() {
    let executions = vec![create_test_execution_summary(
        "exec1",
        LiveExecutionStatus::Completed,
    )];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.live.len(), 1);
    assert_eq!(unified.active_execution_count(), 0);
    assert!(!unified.has_active_executions());
}

#[test]
fn test_unified_introspection_failed_not_active() {
    let executions = vec![create_test_execution_summary(
        "exec1",
        LiveExecutionStatus::Failed,
    )];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.live.len(), 1);
    assert_eq!(unified.active_execution_count(), 0);
    assert!(!unified.has_active_executions());
}

#[test]
fn test_unified_introspection_mixed_statuses() {
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Running),
        create_test_execution_summary("exec2", LiveExecutionStatus::Completed),
        create_test_execution_summary("exec3", LiveExecutionStatus::Paused),
        create_test_execution_summary("exec4", LiveExecutionStatus::Failed),
        create_test_execution_summary("exec5", LiveExecutionStatus::WaitingForInput),
    ];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.live.len(), 5);
    // Active: Running, Paused, WaitingForInput = 3
    assert_eq!(unified.active_execution_count(), 3);
    assert!(unified.has_active_executions());
}

#[test]
fn test_unified_introspection_to_json() {
    let executions = vec![create_test_execution_summary("exec1", LiveExecutionStatus::Running)];

    let unified = create_test_unified_introspection(executions);

    let json = unified.to_json().expect("should serialize to JSON");

    // Verify the JSON structure
    assert!(json.contains("\"platform\":{"));
    assert!(json.contains("\"app\":{"));
    assert!(json.contains("\"live\":["));

    // Verify the execution ID is in the JSON
    assert!(json.contains("exec1"));
}

#[test]
fn test_unified_introspection_json_parseable() {
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Running),
        create_test_execution_summary("exec2", LiveExecutionStatus::Completed),
    ];

    let unified = create_test_unified_introspection(executions);

    let json = unified.to_json().expect("should serialize");

    // Verify the JSON is valid and parseable
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

    assert!(parsed.get("platform").is_some());
    assert!(parsed.get("app").is_some());
    assert!(parsed.get("live").is_some());

    // Verify the live array has 2 elements
    let live = parsed.get("live").unwrap().as_array().unwrap();
    assert_eq!(live.len(), 2);
}

#[test]
fn test_unified_introspection_clone() {
    let executions = vec![create_test_execution_summary("exec1", LiveExecutionStatus::Running)];

    let unified = create_test_unified_introspection(executions);

    let cloned = unified.clone();

    assert_eq!(cloned.live.len(), unified.live.len());
    assert_eq!(
        cloned.active_execution_count(),
        unified.active_execution_count()
    );
}

#[test]
fn test_unified_introspection_debug() {
    let unified = create_test_unified_introspection(vec![]);

    let debug_str = format!("{:?}", unified);
    assert!(debug_str.contains("UnifiedIntrospection"));
}

#[test]
fn test_unified_introspection_empty_json() {
    let unified = create_test_unified_introspection(vec![]);

    let json = unified.to_json().expect("should serialize");

    // Verify empty live array
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
    let live = parsed.get("live").unwrap().as_array().unwrap();
    assert!(live.is_empty());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_active_execution_count_all_active() {
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Running),
        create_test_execution_summary("exec2", LiveExecutionStatus::Running),
        create_test_execution_summary("exec3", LiveExecutionStatus::Running),
    ];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 3);
}

#[test]
fn test_active_execution_count_none_active() {
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Completed),
        create_test_execution_summary("exec2", LiveExecutionStatus::Failed),
        create_test_execution_summary("exec3", LiveExecutionStatus::Completed),
    ];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 0);
    assert!(!unified.has_active_executions());
}

#[test]
fn test_unified_introspection_large_number_of_executions() {
    // Test with many executions
    let executions: Vec<ExecutionSummary> = (0..100)
        .map(|i| {
            let status = if i % 3 == 0 {
                LiveExecutionStatus::Running
            } else if i % 3 == 1 {
                LiveExecutionStatus::Completed
            } else {
                LiveExecutionStatus::Failed
            };
            create_test_execution_summary(&format!("exec{}", i), status)
        })
        .collect();

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.live.len(), 100);
    // Every 3rd execution (indices 0, 3, 6, ..., 99) is Running = 34 active
    assert_eq!(unified.active_execution_count(), 34);
    assert!(unified.has_active_executions());

    // JSON should still work
    let json = unified.to_json().expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
    let live = parsed.get("live").unwrap().as_array().unwrap();
    assert_eq!(live.len(), 100);
}

#[test]
fn test_execution_summary_all_status_variants() {
    // Ensure we can create summaries with all status variants
    let statuses = vec![
        LiveExecutionStatus::Running,
        LiveExecutionStatus::Paused,
        LiveExecutionStatus::WaitingForInput,
        LiveExecutionStatus::Completed,
        LiveExecutionStatus::Failed,
    ];

    for status in statuses {
        let summary = create_test_execution_summary("test", status);
        assert_eq!(summary.graph_name, "Test Graph");
    }
}

#[test]
fn test_graph_introspection_architecture_info() {
    let introspection = create_test_graph_introspection();

    // Verify architecture info is accessible
    assert_eq!(introspection.architecture.graph_structure.node_count, 2);
    assert_eq!(introspection.architecture.graph_structure.edge_count, 1);
    assert_eq!(
        introspection.architecture.graph_structure.entry_point,
        "start"
    );
    assert!(!introspection.architecture.graph_structure.has_cycles);
    assert_eq!(
        introspection.architecture.graph_structure.name,
        Some("Test Graph".to_string())
    );
}

#[test]
fn test_unified_introspection_platform_accessible() {
    let unified = create_test_unified_introspection(vec![]);

    // Platform should be accessible
    let platform_json = unified.platform.to_json();
    assert!(platform_json.contains("version"));
}

#[test]
fn test_unified_introspection_app_accessible() {
    let unified = create_test_unified_introspection(vec![]);

    // App introspection should be accessible
    assert_eq!(
        unified.app.manifest.graph_name,
        Some("Test Graph".to_string())
    );
}

#[test]
fn test_has_active_executions_boundary() {
    // Test with exactly one active execution
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Completed),
        create_test_execution_summary("exec2", LiveExecutionStatus::Running),
        create_test_execution_summary("exec3", LiveExecutionStatus::Failed),
    ];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 1);
    assert!(unified.has_active_executions());
}

#[test]
fn test_active_execution_count_with_all_active_types() {
    // Test with one of each active type
    let executions = vec![
        create_test_execution_summary("exec1", LiveExecutionStatus::Running),
        create_test_execution_summary("exec2", LiveExecutionStatus::Paused),
        create_test_execution_summary("exec3", LiveExecutionStatus::WaitingForInput),
    ];

    let unified = create_test_unified_introspection(executions);

    assert_eq!(unified.active_execution_count(), 3);
    assert!(unified.has_active_executions());
}
