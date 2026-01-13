// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for the graph registry module.

use super::*;
use std::time::{Duration, SystemTime};

// ==================== RegistryMetadata Tests ====================

#[test]
fn test_registry_metadata_new() {
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");
    assert_eq!(meta.name, "Test Agent");
    assert_eq!(meta.version, "1.0.0");
    assert!(meta.tags.is_empty());
}

#[test]
fn test_registry_metadata_builder() {
    let meta = RegistryMetadata::new("Test Agent", "1.0.0")
        .with_description("A test agent")
        .with_tag("test")
        .with_tag("demo")
        .with_author("Test Author");

    assert_eq!(meta.description, "A test agent");
    assert_eq!(meta.tags, vec!["test", "demo"]);
    assert_eq!(meta.author, Some("Test Author".to_string()));
}

#[test]
fn test_registry_metadata_with_tags() {
    let meta = RegistryMetadata::new("Test", "1.0.0").with_tags(vec!["a", "b", "c"]);

    assert_eq!(meta.tags, vec!["a", "b", "c"]);
}

#[test]
fn test_registry_metadata_has_tag() {
    let meta = RegistryMetadata::new("Test", "1.0.0").with_tag("coding");

    assert!(meta.has_tag("coding"));
    assert!(!meta.has_tag("testing"));
}

#[test]
fn test_registry_metadata_custom() {
    let meta = RegistryMetadata::new("Test", "1.0.0").with_custom("priority", serde_json::json!(1));

    assert_eq!(meta.custom.get("priority"), Some(&serde_json::json!(1)));
}

// ==================== RegistryEntry Tests ====================

#[test]
fn test_registry_entry_new() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test", "1.0.0");
    let entry = RegistryEntry::new("test_id", manifest, meta);

    assert_eq!(entry.graph_id, "test_id");
    assert_eq!(entry.execution_count, 0);
    assert!(entry.active);
}

#[test]
fn test_registry_entry_record_execution() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test", "1.0.0");
    let mut entry = RegistryEntry::new("test_id", manifest, meta);

    entry.record_execution();
    assert_eq!(entry.execution_count, 1);

    entry.record_execution();
    assert_eq!(entry.execution_count, 2);
}

#[test]
fn test_registry_entry_activate_deactivate() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test", "1.0.0");
    let mut entry = RegistryEntry::new("test_id", manifest, meta);

    assert!(entry.active);
    entry.deactivate();
    assert!(!entry.active);
    entry.activate();
    assert!(entry.active);
}

// ==================== GraphRegistry Tests ====================

#[test]
fn test_graph_registry_new() {
    let registry = GraphRegistry::new();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_graph_registry_register() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");

    registry.register("agent_1", manifest, meta);

    assert_eq!(registry.count(), 1);
    assert!(registry.contains("agent_1"));
}

#[test]
fn test_graph_registry_get() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");

    registry.register("agent_1", manifest, meta);

    let entry = registry.get("agent_1").unwrap();
    assert_eq!(entry.metadata.name, "Test Agent");

    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_graph_registry_unregister() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");

    registry.register("agent_1", manifest, meta);
    assert!(registry.contains("agent_1"));

    let removed = registry.unregister("agent_1");
    assert!(removed.is_some());
    assert!(!registry.contains("agent_1"));
}

#[test]
fn test_graph_registry_list_graphs() {
    let registry = GraphRegistry::new();

    for i in 0..3 {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .build()
            .unwrap();
        let meta = RegistryMetadata::new(format!("Agent {i}"), "1.0.0");
        registry.register(format!("agent_{i}"), manifest, meta);
    }

    let graphs = registry.list_graphs();
    assert_eq!(graphs.len(), 3);
}

#[test]
fn test_graph_registry_list_active() {
    let registry = GraphRegistry::new();

    for i in 0..3 {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .build()
            .unwrap();
        let meta = RegistryMetadata::new(format!("Agent {i}"), "1.0.0");
        registry.register(format!("agent_{i}"), manifest, meta);
    }

    registry.deactivate("agent_1");

    let active = registry.list_active();
    assert_eq!(active.len(), 2);
}

#[test]
fn test_graph_registry_find_by_tag() {
    let registry = GraphRegistry::new();

    let manifest1 = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta1 = RegistryMetadata::new("Coding Agent", "1.0.0")
        .with_tag("coding")
        .with_tag("production");
    registry.register("coding_1", manifest1, meta1);

    let manifest2 = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta2 = RegistryMetadata::new("Research Agent", "1.0.0").with_tag("research");
    registry.register("research_1", manifest2, meta2);

    let coding_agents = registry.find_by_tag("coding");
    assert_eq!(coding_agents.len(), 1);
    assert_eq!(coding_agents[0].graph_id, "coding_1");

    let production_agents = registry.find_by_tag("production");
    assert_eq!(production_agents.len(), 1);

    let empty = registry.find_by_tag("nonexistent");
    assert!(empty.is_empty());
}

#[test]
fn test_graph_registry_find_by_name() {
    let registry = GraphRegistry::new();

    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("My Coding Agent", "1.0.0");
    registry.register("agent_1", manifest, meta);

    let found = registry.find_by_name("coding");
    assert_eq!(found.len(), 1);

    let found_upper = registry.find_by_name("CODING");
    assert_eq!(found_upper.len(), 1);

    let not_found = registry.find_by_name("xyz");
    assert!(not_found.is_empty());
}

#[test]
fn test_graph_registry_find_by_author() {
    let registry = GraphRegistry::new();

    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0").with_author("John Doe");
    registry.register("agent_1", manifest, meta);

    let found = registry.find_by_author("john");
    assert_eq!(found.len(), 1);

    let not_found = registry.find_by_author("jane");
    assert!(not_found.is_empty());
}

#[test]
fn test_graph_registry_find_by_version_prefix() {
    let registry = GraphRegistry::new();

    let manifest1 = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta1 = RegistryMetadata::new("Agent v1", "1.0.0");
    registry.register("agent_v1", manifest1, meta1);

    let manifest2 = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta2 = RegistryMetadata::new("Agent v2", "2.0.0");
    registry.register("agent_v2", manifest2, meta2);

    let v1_agents = registry.find_by_version_prefix("1.");
    assert_eq!(v1_agents.len(), 1);

    let v2_agents = registry.find_by_version_prefix("2.");
    assert_eq!(v2_agents.len(), 1);
}

#[test]
fn test_graph_registry_most_executed() {
    let registry = GraphRegistry::new();

    for i in 0..3 {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .build()
            .unwrap();
        let meta = RegistryMetadata::new(format!("Agent {i}"), "1.0.0");
        registry.register(format!("agent_{i}"), manifest, meta);
    }

    // Record executions
    for _ in 0..5 {
        registry.record_execution("agent_0");
    }
    for _ in 0..3 {
        registry.record_execution("agent_1");
    }
    registry.record_execution("agent_2");

    let most_exec = registry.most_executed(2);
    assert_eq!(most_exec.len(), 2);
    assert_eq!(most_exec[0].graph_id, "agent_0");
    assert_eq!(most_exec[0].execution_count, 5);
}

#[test]
fn test_graph_registry_update_metadata() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Old Name", "1.0.0");
    registry.register("agent_1", manifest, meta);

    let updated = registry.update_metadata("agent_1", |m| {
        m.name = "New Name".to_string();
    });
    assert!(updated);

    let entry = registry.get("agent_1").unwrap();
    assert_eq!(entry.metadata.name, "New Name");

    let not_updated = registry.update_metadata("nonexistent", |_| {});
    assert!(!not_updated);
}

#[test]
fn test_graph_registry_to_json() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");
    registry.register("agent_1", manifest, meta);

    let json = registry.to_json().unwrap();
    assert!(json.contains("Test Agent"));
    assert!(json.contains("1.0.0"));
}

#[test]
fn test_graph_registry_graph_ids() {
    let registry = GraphRegistry::new();

    for i in 0..3 {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .build()
            .unwrap();
        let meta = RegistryMetadata::new(format!("Agent {i}"), "1.0.0");
        registry.register(format!("agent_{i}"), manifest, meta);
    }

    let ids = registry.graph_ids();
    assert_eq!(ids.len(), 3);
}

#[test]
fn test_graph_registry_clear() {
    let registry = GraphRegistry::new();

    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test Agent", "1.0.0");
    registry.register("agent_1", manifest, meta);

    assert_eq!(registry.count(), 1);
    registry.clear();
    assert_eq!(registry.count(), 0);
}

// ==================== ExecutionStatus Tests ====================

#[test]
fn test_execution_status_is_terminal() {
    assert!(!ExecutionStatus::Running.is_terminal());
    assert!(ExecutionStatus::Completed.is_terminal());
    assert!(ExecutionStatus::Failed.is_terminal());
    assert!(ExecutionStatus::Interrupted.is_terminal());
    assert!(ExecutionStatus::TimedOut.is_terminal());
}

#[test]
fn test_execution_status_is_running() {
    assert!(ExecutionStatus::Running.is_running());
    assert!(!ExecutionStatus::Completed.is_running());
}

#[test]
fn test_execution_status_is_success() {
    assert!(ExecutionStatus::Completed.is_success());
    assert!(!ExecutionStatus::Failed.is_success());
    assert!(!ExecutionStatus::Running.is_success());
}

#[test]
fn test_execution_status_display() {
    assert_eq!(format!("{}", ExecutionStatus::Running), "Running");
    assert_eq!(format!("{}", ExecutionStatus::Completed), "Completed");
    assert_eq!(format!("{}", ExecutionStatus::Failed), "Failed");
}

// ==================== ExecutionRecord Tests ====================

#[test]
fn test_execution_record_new() {
    let record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");

    assert_eq!(record.thread_id, "thread_1");
    assert_eq!(record.graph_id, "agent_1");
    assert_eq!(record.graph_version, "1.0.0");
    assert_eq!(record.status, ExecutionStatus::Running);
    assert!(record.nodes_executed.is_empty());
    assert_eq!(record.total_tokens, 0);
}

#[test]
fn test_execution_record_complete() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    record.complete(Some(serde_json::json!({"result": "ok"})));

    assert_eq!(record.status, ExecutionStatus::Completed);
    assert!(record.completed_at.is_some());
    assert_eq!(
        record.final_state,
        Some(serde_json::json!({"result": "ok"}))
    );
}

#[test]
fn test_execution_record_fail() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    record.fail("Something went wrong");

    assert_eq!(record.status, ExecutionStatus::Failed);
    assert!(record.completed_at.is_some());
    assert_eq!(record.error, Some("Something went wrong".to_string()));
}

#[test]
fn test_execution_record_nodes() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    record.record_node("start");
    record.record_node("process");
    record.record_node("end");

    assert_eq!(record.nodes_executed, vec!["start", "process", "end"]);
}

#[test]
fn test_execution_record_tokens() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    record.add_tokens(100);
    record.add_tokens(50);

    assert_eq!(record.total_tokens, 150);
}

#[test]
fn test_execution_record_duration() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    assert!(record.duration().is_none());

    record.complete(None);
    assert!(record.duration().is_some());
}

#[test]
fn test_execution_record_elapsed() {
    let record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    let elapsed = record.elapsed();
    assert!(elapsed < Duration::from_secs(1));
}

// ==================== ExecutionRegistry Tests ====================

#[test]
fn test_execution_registry_new() {
    let registry = ExecutionRegistry::new();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_execution_registry_record_start() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");

    assert_eq!(registry.count(), 1);
    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.status, ExecutionStatus::Running);
}

#[test]
fn test_execution_registry_record_completion() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_completion("thread_1", Some(serde_json::json!({"done": true})));

    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.status, ExecutionStatus::Completed);
}

#[test]
fn test_execution_registry_record_failure() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_failure("thread_1", "Error occurred");

    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.status, ExecutionStatus::Failed);
    assert_eq!(record.error, Some("Error occurred".to_string()));
}

#[test]
fn test_execution_registry_list_running() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_completion("thread_2", None);

    let running = registry.list_running();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].thread_id, "thread_1");
}

#[test]
fn test_execution_registry_list_by_status() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_start("thread_3", "agent_1", "1.0.0");
    registry.record_completion("thread_1", None);
    registry.record_failure("thread_2", "Error");

    let completed = registry.list_by_status(ExecutionStatus::Completed);
    assert_eq!(completed.len(), 1);

    let failed = registry.list_by_status(ExecutionStatus::Failed);
    assert_eq!(failed.len(), 1);

    let running = registry.list_by_status(ExecutionStatus::Running);
    assert_eq!(running.len(), 1);
}

#[test]
fn test_execution_registry_list_by_graph() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_2", "1.0.0");
    registry.record_start("thread_3", "agent_1", "1.0.0");

    let agent1_execs = registry.list_by_graph("agent_1");
    assert_eq!(agent1_execs.len(), 2);
}

#[test]
fn test_execution_registry_list_recent() {
    let registry = ExecutionRegistry::new();
    for i in 0..5 {
        registry.record_start(format!("thread_{i}"), "agent_1", "1.0.0");
    }

    let recent = registry.list_recent(3);
    assert_eq!(recent.len(), 3);
}

#[test]
fn test_execution_registry_count_by_status() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_completion("thread_1", None);

    assert_eq!(registry.count_by_status(ExecutionStatus::Completed), 1);
    assert_eq!(registry.count_by_status(ExecutionStatus::Running), 1);
}

#[test]
fn test_execution_registry_success_rate() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_start("thread_3", "agent_1", "1.0.0");
    registry.record_completion("thread_1", None);
    registry.record_completion("thread_2", None);
    registry.record_failure("thread_3", "Error");

    let rate = registry.success_rate();
    assert!((rate - 0.666_666_6).abs() < 0.001);
}

#[test]
fn test_execution_registry_total_tokens() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_tokens("thread_1", 100);
    registry.record_tokens("thread_2", 200);

    assert_eq!(registry.total_tokens(), 300);
}

#[test]
fn test_execution_registry_remove() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");

    let removed = registry.remove("thread_1");
    assert!(removed.is_some());
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_execution_registry_clear() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");

    registry.clear();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_execution_registry_clear_completed() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_start("thread_2", "agent_1", "1.0.0");
    registry.record_completion("thread_1", None);

    registry.clear_completed();
    assert_eq!(registry.count(), 1);
    assert!(registry.get("thread_2").is_some());
}

#[test]
fn test_execution_registry_with_max_records() {
    let registry = ExecutionRegistry::with_max_records(3);

    // Add 5 records, should only keep 3
    for i in 0..5 {
        registry.record_start(format!("thread_{i}"), "agent_1", "1.0.0");
        if i < 3 {
            registry.record_completion(&format!("thread_{i}"), None);
        }
    }

    // Max records should be enforced
    assert!(registry.count() <= 3);
}

#[test]
fn test_execution_registry_to_json() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");

    let json = registry.to_json().unwrap();
    assert!(json.contains("thread_1"));
    assert!(json.contains("agent_1"));
}

#[test]
fn test_execution_registry_record_node() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_node("thread_1", "start");
    registry.record_node("thread_1", "process");

    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.nodes_executed, vec!["start", "process"]);
}

#[test]
fn test_execution_registry_record_interrupt() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_interrupt("thread_1");

    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.status, ExecutionStatus::Interrupted);
}

#[test]
fn test_execution_registry_record_timeout() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");
    registry.record_timeout("thread_1");

    let record = registry.get("thread_1").unwrap();
    assert_eq!(record.status, ExecutionStatus::TimedOut);
}

// ==================== Clone Tests ====================

#[test]
fn test_graph_registry_clone_shares_data() {
    let registry = GraphRegistry::new();
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta = RegistryMetadata::new("Test", "1.0.0");
    registry.register("agent_1", manifest, meta);

    let cloned = registry.clone();
    assert_eq!(cloned.count(), 1);

    // Changes in clone should be visible in original
    let manifest2 = GraphManifest::builder()
        .entry_point("start")
        .build()
        .unwrap();
    let meta2 = RegistryMetadata::new("Test 2", "2.0.0");
    cloned.register("agent_2", manifest2, meta2);

    assert_eq!(registry.count(), 2);
}

#[test]
fn test_execution_registry_clone_shares_data() {
    let registry = ExecutionRegistry::new();
    registry.record_start("thread_1", "agent_1", "1.0.0");

    let cloned = registry.clone();
    assert_eq!(cloned.count(), 1);

    // Changes in clone should be visible in original
    cloned.record_start("thread_2", "agent_1", "1.0.0");
    assert_eq!(registry.count(), 2);
}

// ==================== Serialization Tests ====================

#[test]
fn test_registry_metadata_serialization() {
    let meta = RegistryMetadata::new("Test", "1.0.0")
        .with_tag("coding")
        .with_author("Author");

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: RegistryMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "Test");
    assert_eq!(deserialized.version, "1.0.0");
    assert!(deserialized.has_tag("coding"));
}

#[test]
fn test_execution_record_serialization() {
    let mut record = ExecutionRecord::new("thread_1", "agent_1", "1.0.0");
    record.record_node("start");
    record.add_tokens(100);

    let json = serde_json::to_string(&record).unwrap();
    let deserialized: ExecutionRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.thread_id, "thread_1");
    assert_eq!(deserialized.nodes_executed, vec!["start"]);
    assert_eq!(deserialized.total_tokens, 100);
}

#[test]
fn test_execution_status_serialization() {
    let status = ExecutionStatus::Completed;
    let json = serde_json::to_string(&status).unwrap();
    let deserialized: ExecutionStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, ExecutionStatus::Completed);
}

// ==================== GraphVersion Tests ====================

fn create_test_manifest() -> GraphManifest {
    use crate::introspection::{EdgeManifest, NodeManifest, NodeType};

    GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node(
            "start",
            NodeManifest::new("start", NodeType::Function).with_description("Start node"),
        )
        .add_node(
            "process",
            NodeManifest::new("process", NodeType::Function).with_description("Process node"),
        )
        .add_edge("start", EdgeManifest::simple("start", "process"))
        .add_edge("process", EdgeManifest::simple("process", "__end__"))
        .build()
        .unwrap()
}

#[test]
fn test_graph_version_from_manifest() {
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0");

    assert_eq!(version.graph_id, "test_graph");
    assert_eq!(version.version, "1.0.0");
    assert!(!version.content_hash.is_empty());
    assert_eq!(version.node_count, 2);
    assert_eq!(version.edge_count, 2);
    assert_eq!(version.node_versions.len(), 2);
}

#[test]
fn test_graph_version_content_hash_deterministic() {
    let manifest = create_test_manifest();
    let version1 = GraphVersion::from_manifest(&manifest, "1.0.0");
    let version2 = GraphVersion::from_manifest(&manifest, "1.0.0");

    assert_eq!(version1.content_hash, version2.content_hash);
}

#[test]
fn test_graph_version_content_hash_changes() {
    use crate::introspection::{EdgeManifest, NodeManifest, NodeType};

    let manifest1 = create_test_manifest();
    let manifest2 = GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node(
            "start",
            NodeManifest::new("start", NodeType::Function).with_description("Start node"),
        )
        .add_node(
            "different",
            NodeManifest::new("different", NodeType::Function).with_description("Different node"),
        )
        .add_edge("start", EdgeManifest::simple("start", "different"))
        .build()
        .unwrap();

    let version1 = GraphVersion::from_manifest(&manifest1, "1.0.0");
    let version2 = GraphVersion::from_manifest(&manifest2, "1.0.0");

    assert_ne!(version1.content_hash, version2.content_hash);
}

#[test]
fn test_graph_version_has_changed_since() {
    use crate::introspection::{EdgeManifest, NodeManifest, NodeType};

    let manifest1 = create_test_manifest();
    let manifest2 = GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node(
            "new_node",
            NodeManifest::new("new_node", NodeType::Function),
        )
        .add_edge("start", EdgeManifest::simple("start", "new_node"))
        .build()
        .unwrap();

    let v1 = GraphVersion::from_manifest(&manifest1, "1.0.0");
    let v2 = GraphVersion::from_manifest(&manifest2, "2.0.0");

    assert!(v2.has_changed_since(&v1));
}

#[test]
fn test_graph_version_with_source_hash() {
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0").with_source_hash("abc123");

    assert_eq!(version.source_hash, Some("abc123".to_string()));
}

#[test]
fn test_graph_version_diff_no_changes() {
    let manifest = create_test_manifest();
    let v1 = GraphVersion::from_manifest(&manifest, "1.0.0");
    let v2 = GraphVersion::from_manifest(&manifest, "1.0.1");

    let diff = v2.diff(&v1);
    assert!(!diff.has_changes());
    assert!(diff.nodes_added.is_empty());
    assert!(diff.nodes_removed.is_empty());
    assert!(diff.nodes_modified.is_empty());
}

#[test]
fn test_graph_version_diff_nodes_added() {
    use crate::introspection::{EdgeManifest, NodeManifest, NodeType};

    let manifest1 = create_test_manifest();
    let manifest2 = GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node(
            "start",
            NodeManifest::new("start", NodeType::Function).with_description("Start node"),
        )
        .add_node(
            "process",
            NodeManifest::new("process", NodeType::Function).with_description("Process node"),
        )
        .add_node(
            "new_node",
            NodeManifest::new("new_node", NodeType::Function).with_description("New node"),
        )
        .add_edge("start", EdgeManifest::simple("start", "process"))
        .add_edge("process", EdgeManifest::simple("process", "new_node"))
        .build()
        .unwrap();

    let v1 = GraphVersion::from_manifest(&manifest1, "1.0.0");
    let v2 = GraphVersion::from_manifest(&manifest2, "2.0.0");

    let diff = v2.diff(&v1);
    assert!(diff.has_changes());
    assert_eq!(diff.nodes_added, vec!["new_node"]);
}

#[test]
fn test_graph_version_diff_nodes_removed() {
    use crate::introspection::{NodeManifest, NodeType};

    let manifest1 = create_test_manifest();
    let manifest2 = GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .build()
        .unwrap();

    let v1 = GraphVersion::from_manifest(&manifest1, "1.0.0");
    let v2 = GraphVersion::from_manifest(&manifest2, "2.0.0");

    let diff = v2.diff(&v1);
    assert!(diff.has_changes());
    assert!(diff.nodes_removed.contains(&"process".to_string()));
}

#[test]
fn test_graph_version_change_summary() {
    use crate::introspection::{NodeManifest, NodeType};

    let manifest1 = create_test_manifest();
    let manifest2 = GraphManifest::builder()
        .graph_id("test_graph")
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node("new", NodeManifest::new("new", NodeType::Function))
        .build()
        .unwrap();

    let v1 = GraphVersion::from_manifest(&manifest1, "1.0.0");
    let v2 = GraphVersion::from_manifest(&manifest2, "2.0.0");

    let summary = v2.change_summary(&v1);
    assert!(summary.contains("1.0.0"));
    assert!(summary.contains("2.0.0"));
}

#[test]
fn test_graph_version_to_json() {
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0");

    let json = version.to_json().unwrap();
    assert!(json.contains("test_graph"));
    assert!(json.contains("1.0.0"));
}

// ==================== NodeVersion Tests ====================

#[test]
fn test_node_version_new() {
    let version = NodeVersion::new("test_node", "1.0.0", "abc123");

    assert_eq!(version.node_name, "test_node");
    assert_eq!(version.version, "1.0.0");
    assert_eq!(version.code_hash, "abc123");
    assert!(version.source_file.is_none());
}

#[test]
fn test_node_version_with_source() {
    let version =
        NodeVersion::new("test_node", "1.0.0", "abc123").with_source("src/nodes/test.rs", 42);

    assert_eq!(version.source_file, Some("src/nodes/test.rs".to_string()));
    assert_eq!(version.source_line, Some(42));
}

// ==================== GraphDiff Tests ====================

#[test]
fn test_graph_diff_has_changes() {
    let diff = GraphDiff {
        from_version: "1.0.0".to_string(),
        to_version: "2.0.0".to_string(),
        nodes_added: vec!["new_node".to_string()],
        nodes_removed: vec![],
        nodes_modified: vec![],
        edges_changed: false,
        content_hash_changed: true,
    };

    assert!(diff.has_changes());
}

#[test]
fn test_graph_diff_no_changes() {
    let diff = GraphDiff {
        from_version: "1.0.0".to_string(),
        to_version: "1.0.1".to_string(),
        nodes_added: vec![],
        nodes_removed: vec![],
        nodes_modified: vec![],
        edges_changed: false,
        content_hash_changed: false,
    };

    assert!(!diff.has_changes());
}

#[test]
fn test_graph_diff_node_change_count() {
    let diff = GraphDiff {
        from_version: "1.0.0".to_string(),
        to_version: "2.0.0".to_string(),
        nodes_added: vec!["a".to_string(), "b".to_string()],
        nodes_removed: vec!["c".to_string()],
        nodes_modified: vec!["d".to_string()],
        edges_changed: false,
        content_hash_changed: true,
    };

    assert_eq!(diff.node_change_count(), 4);
}

#[test]
fn test_graph_diff_detailed_report() {
    let diff = GraphDiff {
        from_version: "1.0.0".to_string(),
        to_version: "2.0.0".to_string(),
        nodes_added: vec!["new_node".to_string()],
        nodes_removed: vec!["old_node".to_string()],
        nodes_modified: vec!["changed_node".to_string()],
        edges_changed: true,
        content_hash_changed: true,
    };

    let report = diff.detailed_report();
    assert!(report.contains("1.0.0"));
    assert!(report.contains("2.0.0"));
    assert!(report.contains("+ new_node"));
    assert!(report.contains("- old_node"));
    assert!(report.contains("~ changed_node"));
}

#[test]
fn test_graph_diff_to_json() {
    let diff = GraphDiff {
        from_version: "1.0.0".to_string(),
        to_version: "2.0.0".to_string(),
        nodes_added: vec!["new".to_string()],
        nodes_removed: vec![],
        nodes_modified: vec![],
        edges_changed: false,
        content_hash_changed: true,
    };

    let json = diff.to_json().unwrap();
    assert!(json.contains("from_version"));
    assert!(json.contains("to_version"));
}

// ==================== VersionStore Tests ====================

#[test]
fn test_version_store_new() {
    let store = VersionStore::new();
    assert_eq!(store.version_count("nonexistent"), 0);
}

#[test]
fn test_version_store_save_and_get_latest() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0");

    store.save(version.clone());

    let latest = store.get_latest("test_graph").unwrap();
    assert_eq!(latest.version, "1.0.0");
}

#[test]
fn test_version_store_multiple_versions() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    store.save(GraphVersion::from_manifest(&manifest, "1.1.0"));
    store.save(GraphVersion::from_manifest(&manifest, "2.0.0"));

    let latest = store.get_latest("test_graph").unwrap();
    assert_eq!(latest.version, "2.0.0");

    assert_eq!(store.version_count("test_graph"), 3);
}

#[test]
fn test_version_store_get_version() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    store.save(GraphVersion::from_manifest(&manifest, "2.0.0"));

    let v1 = store.get_version("test_graph", "1.0.0").unwrap();
    assert_eq!(v1.version, "1.0.0");

    let v2 = store.get_version("test_graph", "2.0.0").unwrap();
    assert_eq!(v2.version, "2.0.0");

    assert!(store.get_version("test_graph", "3.0.0").is_none());
}

#[test]
fn test_version_store_get_previous() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    assert!(store.get_previous("test_graph").is_none());

    store.save(GraphVersion::from_manifest(&manifest, "2.0.0"));
    let prev = store.get_previous("test_graph").unwrap();
    assert_eq!(prev.version, "1.0.0");
}

#[test]
fn test_version_store_list_versions() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    store.save(GraphVersion::from_manifest(&manifest, "2.0.0"));

    let versions = store.list_versions("test_graph");
    assert_eq!(versions.len(), 2);
}

#[test]
fn test_version_store_version_history() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    store.save(GraphVersion::from_manifest(&manifest, "2.0.0"));
    store.save(GraphVersion::from_manifest(&manifest, "3.0.0"));

    let history = store.version_history("test_graph", 2);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, "3.0.0"); // Newest first
    assert_eq!(history[1].version, "2.0.0");
}

#[test]
fn test_version_store_has_changed() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0");

    // No previous version - has changed
    assert!(store.has_changed("test_graph", &version.content_hash));

    store.save(version.clone());

    // Same hash - no change
    assert!(!store.has_changed("test_graph", &version.content_hash));

    // Different hash - has changed
    assert!(store.has_changed("test_graph", "different_hash"));
}

#[test]
fn test_version_store_clear() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    assert_eq!(store.version_count("test_graph"), 1);

    store.clear();
    assert_eq!(store.version_count("test_graph"), 0);
}

#[test]
fn test_version_store_clear_graph() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));
    store.clear_graph("test_graph");
    assert_eq!(store.version_count("test_graph"), 0);
}

#[test]
fn test_version_store_clone_shares_data() {
    let store = VersionStore::new();
    let manifest = create_test_manifest();

    store.save(GraphVersion::from_manifest(&manifest, "1.0.0"));

    let cloned = store.clone();
    cloned.save(GraphVersion::from_manifest(&manifest, "2.0.0"));

    assert_eq!(store.version_count("test_graph"), 2);
}

#[test]
fn test_graph_version_serialization() {
    let manifest = create_test_manifest();
    let version = GraphVersion::from_manifest(&manifest, "1.0.0");

    let json = serde_json::to_string(&version).unwrap();
    let deserialized: GraphVersion = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.graph_id, version.graph_id);
    assert_eq!(deserialized.version, version.version);
    assert_eq!(deserialized.content_hash, version.content_hash);
}

#[test]
fn test_node_version_serialization() {
    let version = NodeVersion::new("test", "1.0.0", "hash123");

    let json = serde_json::to_string(&version).unwrap();
    let deserialized: NodeVersion = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.node_name, "test");
    assert_eq!(deserialized.code_hash, "hash123");
}

// ==================== StateSnapshot Tests ====================

#[test]
fn test_state_snapshot_new() {
    let snapshot = StateSnapshot::new(
        "thread_1",
        "reasoning",
        serde_json::json!({
            "messages": ["Hello"],
            "step": 1
        }),
    );

    assert_eq!(snapshot.thread_id, "thread_1");
    assert_eq!(snapshot.node, "reasoning");
    assert!(snapshot.size_bytes > 0);
    assert!(snapshot.checkpoint_id.is_none());
}

#[test]
fn test_state_snapshot_with_checkpoint_id() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({}))
        .with_checkpoint_id("cp_123");

    assert_eq!(snapshot.checkpoint_id, Some("cp_123".to_string()));
}

#[test]
fn test_state_snapshot_with_description() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({}))
        .with_description("After first tool call");

    assert_eq!(
        snapshot.description,
        Some("After first tool call".to_string())
    );
}

#[test]
fn test_state_snapshot_with_metadata() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({}))
        .with_metadata("custom_key", serde_json::json!("value"));

    assert_eq!(
        snapshot.metadata.get("custom_key"),
        Some(&serde_json::json!("value"))
    );
}

#[test]
fn test_state_snapshot_get_field() {
    let snapshot = StateSnapshot::new(
        "thread_1",
        "reasoning",
        serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "step": 1,
            "config": {"model": "gpt-4"}
        }),
    );

    assert_eq!(snapshot.get_field("step"), Some(&serde_json::json!(1)));
    assert_eq!(
        snapshot.get_field("config.model"),
        Some(&serde_json::json!("gpt-4"))
    );
    assert!(snapshot.get_field("nonexistent").is_none());
}

#[test]
fn test_state_snapshot_to_json() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({"a": 1}));

    let json = snapshot.to_json().unwrap();
    assert!(json.contains("thread_1"));
    assert!(json.contains("reasoning"));
}

#[test]
fn test_state_snapshot_elapsed() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({}));
    let elapsed = snapshot.elapsed();
    assert!(elapsed < Duration::from_secs(1));
}

#[test]
fn test_state_snapshot_serialization() {
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({"a": 1}))
        .with_checkpoint_id("cp_123")
        .with_description("test");

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: StateSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.thread_id, "thread_1");
    assert_eq!(deserialized.node, "reasoning");
    assert_eq!(deserialized.checkpoint_id, Some("cp_123".to_string()));
}

// ==================== StateRegistry Tests ====================

#[test]
fn test_state_registry_new() {
    let registry = StateRegistry::new();
    assert_eq!(registry.total_count(), 0);
}

#[test]
fn test_state_registry_snapshot() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "reasoning", serde_json::json!({"step": 1}));

    assert_eq!(registry.snapshot_count("thread_1"), 1);
    assert_eq!(registry.total_count(), 1);
}

#[test]
fn test_state_registry_add_snapshot() {
    let registry = StateRegistry::new();
    let snapshot = StateSnapshot::new("thread_1", "reasoning", serde_json::json!({}))
        .with_checkpoint_id("cp_123");

    registry.add_snapshot(snapshot);

    assert_eq!(registry.snapshot_count("thread_1"), 1);
    let latest = registry.get_latest("thread_1").unwrap();
    assert_eq!(latest.checkpoint_id, Some("cp_123".to_string()));
}

#[test]
fn test_state_registry_get_history() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "start", serde_json::json!({"step": 1}));
    registry.snapshot("thread_1", "process", serde_json::json!({"step": 2}));
    registry.snapshot("thread_1", "end", serde_json::json!({"step": 3}));

    let history = registry.get_history("thread_1");
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].node, "start");
    assert_eq!(history[2].node, "end");
}

#[test]
fn test_state_registry_get_latest() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "start", serde_json::json!({"step": 1}));
    registry.snapshot("thread_1", "end", serde_json::json!({"step": 2}));

    let latest = registry.get_latest("thread_1").unwrap();
    assert_eq!(latest.node, "end");

    assert!(registry.get_latest("nonexistent").is_none());
}

#[test]
fn test_state_registry_get_at_checkpoint() {
    let registry = StateRegistry::new();
    let snapshot1 =
        StateSnapshot::new("thread_1", "start", serde_json::json!({})).with_checkpoint_id("cp_1");
    let snapshot2 =
        StateSnapshot::new("thread_1", "end", serde_json::json!({})).with_checkpoint_id("cp_2");

    registry.add_snapshot(snapshot1);
    registry.add_snapshot(snapshot2);

    let at_cp1 = registry.get_at_checkpoint("thread_1", "cp_1").unwrap();
    assert_eq!(at_cp1.node, "start");

    let at_cp2 = registry.get_at_checkpoint("thread_1", "cp_2").unwrap();
    assert_eq!(at_cp2.node, "end");

    assert!(registry.get_at_checkpoint("thread_1", "cp_999").is_none());
}

#[test]
fn test_state_registry_get_at_time() {
    let registry = StateRegistry::new();

    let now = SystemTime::now();
    let past = now - Duration::from_secs(10);
    let future = now + Duration::from_secs(10);

    let snapshot1 =
        StateSnapshot::new("thread_1", "past", serde_json::json!({})).with_timestamp(past);
    let snapshot2 =
        StateSnapshot::new("thread_1", "now", serde_json::json!({})).with_timestamp(now);
    let snapshot3 =
        StateSnapshot::new("thread_1", "future", serde_json::json!({})).with_timestamp(future);

    registry.add_snapshot(snapshot1);
    registry.add_snapshot(snapshot2);
    registry.add_snapshot(snapshot3);

    // Get snapshot closest to now
    let at_now = registry.get_at_time("thread_1", now).unwrap();
    assert_eq!(at_now.node, "now");

    // Get snapshot closest to past
    let at_past = registry.get_at_time("thread_1", past).unwrap();
    assert_eq!(at_past.node, "past");
}

#[test]
fn test_state_registry_get_by_node() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "reasoning", serde_json::json!({"step": 1}));
    registry.snapshot("thread_1", "tool_call", serde_json::json!({"step": 2}));
    registry.snapshot("thread_1", "reasoning", serde_json::json!({"step": 3}));

    let reasoning_snapshots = registry.get_by_node("thread_1", "reasoning");
    assert_eq!(reasoning_snapshots.len(), 2);

    let tool_snapshots = registry.get_by_node("thread_1", "tool_call");
    assert_eq!(tool_snapshots.len(), 1);
}

#[test]
fn test_state_registry_get_recent() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({}));
    registry.snapshot("thread_2", "b", serde_json::json!({}));
    registry.snapshot("thread_1", "c", serde_json::json!({}));

    let recent = registry.get_recent(2);
    assert_eq!(recent.len(), 2);
}

#[test]
fn test_state_registry_thread_ids() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({}));
    registry.snapshot("thread_2", "b", serde_json::json!({}));
    registry.snapshot("thread_3", "c", serde_json::json!({}));

    let ids = registry.thread_ids();
    assert_eq!(ids.len(), 3);
}

#[test]
fn test_state_registry_get_changes() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "start", serde_json::json!({"step": 1}));
    registry.snapshot(
        "thread_1",
        "process",
        serde_json::json!({"step": 2, "result": "ok"}),
    );
    registry.snapshot(
        "thread_1",
        "end",
        serde_json::json!({"step": 3, "result": "ok"}),
    );

    let changes = registry.get_changes("thread_1");
    assert_eq!(changes.len(), 2);

    // First change: step 1->2, result added
    assert!(changes[0].has_changes());

    // Second change: step 2->3
    assert!(changes[1].has_changes());
}

#[test]
fn test_state_registry_diff_snapshots() {
    let before = StateSnapshot::new(
        "thread_1",
        "start",
        serde_json::json!({
            "messages": [],
            "step": 1
        }),
    );
    let after = StateSnapshot::new(
        "thread_1",
        "end",
        serde_json::json!({
            "messages": ["Hello"],
            "step": 2,
            "result": "ok"
        }),
    );

    let diff = StateRegistry::diff_snapshots(&before, &after);
    assert!(diff.has_changes());
    assert!(diff.added.contains(&"result".to_string()));
}

#[test]
fn test_state_registry_clear_thread() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({}));
    registry.snapshot("thread_2", "b", serde_json::json!({}));

    registry.clear_thread("thread_1");
    assert_eq!(registry.snapshot_count("thread_1"), 0);
    assert_eq!(registry.snapshot_count("thread_2"), 1);
}

#[test]
fn test_state_registry_clear() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({}));
    registry.snapshot("thread_2", "b", serde_json::json!({}));

    registry.clear();
    assert_eq!(registry.total_count(), 0);
}

#[test]
fn test_state_registry_with_max_snapshots() {
    let registry = StateRegistry::with_max_snapshots(3);
    registry.snapshot("thread_1", "a", serde_json::json!({}));
    registry.snapshot("thread_1", "b", serde_json::json!({}));
    registry.snapshot("thread_1", "c", serde_json::json!({}));
    registry.snapshot("thread_1", "d", serde_json::json!({}));

    assert_eq!(registry.snapshot_count("thread_1"), 3);
    // First snapshot should have been pruned
    let history = registry.get_history("thread_1");
    assert_eq!(history[0].node, "b");
}

#[test]
fn test_state_registry_clone_shares_data() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({}));

    let cloned = registry.clone();
    cloned.snapshot("thread_1", "b", serde_json::json!({}));

    assert_eq!(registry.snapshot_count("thread_1"), 2);
}

#[test]
fn test_state_registry_to_json() {
    let registry = StateRegistry::new();
    registry.snapshot("thread_1", "a", serde_json::json!({"key": "value"}));

    let json = registry.to_json().unwrap();
    assert!(json.contains("thread_1"));
    assert!(json.contains("key"));
}

// ==================== StateDiff Tests ====================

#[test]
fn test_state_diff_empty() {
    let diff = StateDiff::empty();
    assert!(!diff.has_changes());
    assert_eq!(diff.change_count(), 0);
}

#[test]
fn test_state_diff_has_changes() {
    let diff = StateDiff {
        added: vec!["new_field".to_string()],
        removed: vec![],
        modified: vec![],
    };
    assert!(diff.has_changes());
}

#[test]
fn test_state_diff_change_count() {
    let diff = StateDiff {
        added: vec!["a".to_string(), "b".to_string()],
        removed: vec!["c".to_string()],
        modified: vec![FieldDiff::new(
            "d",
            serde_json::json!(1),
            serde_json::json!(2),
        )],
    };
    assert_eq!(diff.change_count(), 4);
}

#[test]
fn test_state_diff_summary() {
    let diff = StateDiff {
        added: vec!["a".to_string()],
        removed: vec!["b".to_string()],
        modified: vec![FieldDiff::new(
            "c",
            serde_json::json!(1),
            serde_json::json!(2),
        )],
    };
    let summary = diff.summary();
    assert!(summary.contains("1 added"));
    assert!(summary.contains("1 removed"));
    assert!(summary.contains("1 modified"));
}

#[test]
fn test_state_diff_detailed_report() {
    let diff = StateDiff {
        added: vec!["new_field".to_string()],
        removed: vec!["old_field".to_string()],
        modified: vec![FieldDiff::new(
            "changed",
            serde_json::json!(1),
            serde_json::json!(2),
        )],
    };
    let report = diff.detailed_report();
    assert!(report.contains("+ new_field"));
    assert!(report.contains("- old_field"));
    assert!(report.contains("~ changed"));
}

#[test]
fn test_state_diff_to_json() {
    let diff = StateDiff {
        added: vec!["a".to_string()],
        removed: vec![],
        modified: vec![],
    };
    let json = diff.to_json().unwrap();
    assert!(json.contains("added"));
}

// ==================== FieldDiff Tests ====================

#[test]
fn test_field_diff_new() {
    let diff = FieldDiff::new("path.to.field", serde_json::json!(1), serde_json::json!(2));
    assert_eq!(diff.path, "path.to.field");
    assert_eq!(diff.before, serde_json::json!(1));
    assert_eq!(diff.after, serde_json::json!(2));
}

#[test]
fn test_field_diff_is_type_change() {
    let no_type_change = FieldDiff::new("x", serde_json::json!(1), serde_json::json!(2));
    assert!(!no_type_change.is_type_change());

    let type_change = FieldDiff::new("x", serde_json::json!(1), serde_json::json!("str"));
    assert!(type_change.is_type_change());
}

#[test]
fn test_field_diff_description() {
    let diff = FieldDiff::new("step", serde_json::json!(1), serde_json::json!(2));
    let desc = diff.description();
    assert!(desc.contains("step"));
    assert!(desc.contains("1"));
    assert!(desc.contains("2"));
}

// ==================== state_diff Function Tests ====================

#[test]
fn test_state_diff_no_changes() {
    let before = serde_json::json!({"a": 1, "b": "two"});
    let after = serde_json::json!({"a": 1, "b": "two"});

    let diff = state_diff(&before, &after);
    assert!(!diff.has_changes());
}

#[test]
fn test_state_diff_field_added() {
    let before = serde_json::json!({"a": 1});
    let after = serde_json::json!({"a": 1, "b": 2});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.added, vec!["b"]);
}

#[test]
fn test_state_diff_field_removed() {
    let before = serde_json::json!({"a": 1, "b": 2});
    let after = serde_json::json!({"a": 1});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.removed, vec!["b"]);
}

#[test]
fn test_state_diff_field_modified() {
    let before = serde_json::json!({"a": 1});
    let after = serde_json::json!({"a": 2});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].path, "a");
}

#[test]
fn test_state_diff_nested_object() {
    let before = serde_json::json!({
        "outer": {
            "inner": 1,
            "other": "hello"
        }
    });
    let after = serde_json::json!({
        "outer": {
            "inner": 2,
            "other": "hello"
        }
    });

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].path, "outer.inner");
}

#[test]
fn test_state_diff_nested_added() {
    let before = serde_json::json!({"outer": {}});
    let after = serde_json::json!({"outer": {"new": 1}});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.added, vec!["outer.new"]);
}

#[test]
fn test_state_diff_array_modified() {
    let before = serde_json::json!({"arr": [1, 2, 3]});
    let after = serde_json::json!({"arr": [1, 5, 3]});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].path, "arr[1]");
}

#[test]
fn test_state_diff_array_element_added() {
    let before = serde_json::json!({"arr": [1, 2]});
    let after = serde_json::json!({"arr": [1, 2, 3]});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.added, vec!["arr[2]"]);
}

#[test]
fn test_state_diff_array_element_removed() {
    let before = serde_json::json!({"arr": [1, 2, 3]});
    let after = serde_json::json!({"arr": [1, 2]});

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.removed, vec!["arr[2]"]);
}

#[test]
fn test_state_diff_complex() {
    let before = serde_json::json!({
        "messages": [
            {"role": "user", "content": "Hello"}
        ],
        "step": 1,
        "temp_data": "to_remove"
    });
    let after = serde_json::json!({
        "messages": [
            {"role": "user", "content": "Hello"},
            {"role": "assistant", "content": "Hi there"}
        ],
        "step": 2,
        "result": "success"
    });

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert!(diff.added.contains(&"messages[1]".to_string()));
    assert!(diff.added.contains(&"result".to_string()));
    assert!(diff.removed.contains(&"temp_data".to_string()));
    assert!(diff.modified.iter().any(|f| f.path == "step"));
}

#[test]
fn test_state_diff_root_value_changed() {
    let before = serde_json::json!(1);
    let after = serde_json::json!(2);

    let diff = state_diff(&before, &after);
    assert!(diff.has_changes());
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].path, "(root)");
}

#[test]
fn test_state_diff_serialization() {
    let diff = StateDiff {
        added: vec!["a".to_string()],
        removed: vec!["b".to_string()],
        modified: vec![FieldDiff::new(
            "c",
            serde_json::json!(1),
            serde_json::json!(2),
        )],
    };

    let json = serde_json::to_string(&diff).unwrap();
    let deserialized: StateDiff = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.added, vec!["a"]);
    assert_eq!(deserialized.removed, vec!["b"]);
    assert_eq!(deserialized.modified.len(), 1);
}

#[test]
fn test_field_diff_serialization() {
    let diff = FieldDiff::new("path", serde_json::json!(1), serde_json::json!(2));

    let json = serde_json::to_string(&diff).unwrap();
    let deserialized: FieldDiff = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.path, "path");
    assert_eq!(deserialized.before, serde_json::json!(1));
    assert_eq!(deserialized.after, serde_json::json!(2));
}
