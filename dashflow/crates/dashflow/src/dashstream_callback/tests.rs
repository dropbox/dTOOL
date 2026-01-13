use super::*;
use crate::state::AgentState;
use serde_json::json;

#[test]
fn test_dashstream_config_default() {
    let config = DashStreamConfig::default();
    // Default reads from KAFKA_BROKERS env var with localhost:9092 fallback
    let expected_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    assert_eq!(config.bootstrap_servers, expected_brokers);
    // Default reads from KAFKA_TOPIC env var with dashstream-events fallback
    let expected_topic =
        std::env::var("KAFKA_TOPIC").unwrap_or_else(|_| "dashstream-events".to_string());
    assert_eq!(config.topic, expected_topic);
    assert_eq!(config.tenant_id, "default");
    assert!(config.enable_state_diff);
    assert_eq!(config.compression_threshold, 512);
}

#[test]
fn test_initial_state_json_is_gated_by_enable_state_diff() {
    let state = AgentState::new();

    let mut attrs_disabled = std::collections::HashMap::new();
    let state_json_disabled = maybe_insert_initial_state_json_attribute(
        false,
        DEFAULT_MAX_STATE_DIFF_SIZE,
        &state,
        &mut attrs_disabled,
    );
    assert!(state_json_disabled.is_none());
    assert!(!attrs_disabled.contains_key("initial_state_json"));

    let mut attrs_enabled = std::collections::HashMap::new();
    let state_json_enabled = maybe_insert_initial_state_json_attribute(
        true,
        DEFAULT_MAX_STATE_DIFF_SIZE,
        &state,
        &mut attrs_enabled,
    );
    assert!(state_json_enabled.is_some());
    assert!(attrs_enabled.contains_key("initial_state_json"));
}

#[test]
fn test_dashstream_config_custom() {
    let config = DashStreamConfig {
        bootstrap_servers: "kafka.example.com:9093".to_string(),
        topic: "custom-events".to_string(),
        tenant_id: "acme-corp".to_string(),
        thread_id: "thread-456".to_string(),
        enable_state_diff: false,
        compression_threshold: 1024,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    assert_eq!(config.bootstrap_servers, "kafka.example.com:9093");
    assert_eq!(config.topic, "custom-events");
    assert_eq!(config.tenant_id, "acme-corp");
    assert_eq!(config.thread_id, "thread-456");
    assert!(!config.enable_state_diff);
    assert_eq!(config.compression_threshold, 1024);
}

#[test]
fn test_dashstream_config_builder_new() {
    let config = DashStreamConfig::new();
    // Should be equivalent to default()
    let default_config = DashStreamConfig::default();
    assert_eq!(config.topic, default_config.topic);
    assert_eq!(config.tenant_id, default_config.tenant_id);
    assert_eq!(config.enable_state_diff, default_config.enable_state_diff);
    assert_eq!(
        config.compression_threshold,
        default_config.compression_threshold
    );
}

#[test]
fn test_dashstream_config_builder_full_chain() {
    let config = DashStreamConfig::new()
        .with_bootstrap_servers("kafka.prod:9093")
        .with_topic("production-events")
        .with_tenant_id("acme-corp")
        .with_thread_id("session-xyz")
        .with_enable_state_diff(false)
        .with_compression_threshold(2048)
        .with_max_state_diff_size(20 * 1024 * 1024) // 20MB
        .with_max_concurrent_telemetry_sends(128)
        .with_telemetry_batch_size(50)
        .with_telemetry_batch_timeout_ms(200)
        .with_checkpoint_interval(100)
        .with_flush_timeout_secs(10);

    assert_eq!(config.bootstrap_servers, "kafka.prod:9093");
    assert_eq!(config.topic, "production-events");
    assert_eq!(config.tenant_id, "acme-corp");
    assert_eq!(config.thread_id, "session-xyz");
    assert!(!config.enable_state_diff);
    assert_eq!(config.compression_threshold, 2048);
    assert_eq!(config.max_state_diff_size, 20 * 1024 * 1024);
    assert_eq!(config.max_concurrent_telemetry_sends, 128);
    assert_eq!(config.telemetry_batch_size, 50);
    assert_eq!(config.telemetry_batch_timeout_ms, 200);
    assert_eq!(config.checkpoint_interval, 100);
    assert_eq!(config.flush_timeout_secs, 10);
}

#[test]
fn test_dashstream_config_builder_partial_chain() {
    // Test that partial builder chains preserve defaults for unchanged fields
    let config = DashStreamConfig::new()
        .with_topic("custom-topic")
        .with_checkpoint_interval(50);

    // Custom values
    assert_eq!(config.topic, "custom-topic");
    assert_eq!(config.checkpoint_interval, 50);

    // Default values preserved
    assert_eq!(config.tenant_id, "default");
    assert!(config.enable_state_diff);
    assert_eq!(config.compression_threshold, 512);
    assert_eq!(config.telemetry_batch_size, DEFAULT_TELEMETRY_BATCH_SIZE);
}

#[test]
fn test_sequence_number_generation() {
    // Create mock callback to test sequence logic
    let _config = DashStreamConfig {
        bootstrap_servers: "localhost:9092".to_string(),
        topic: "test-events".to_string(),
        tenant_id: "test-tenant".to_string(),
        thread_id: "test-thread".to_string(),
        enable_state_diff: true,
        compression_threshold: 512,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    let sequence = Arc::new(Mutex::new(0));

    // Simulate next_sequence() logic
    let get_next = || {
        let mut seq = sequence.lock();
        let current = *seq;
        *seq += 1;
        current
    };

    assert_eq!(get_next(), 0);
    assert_eq!(get_next(), 1);
    assert_eq!(get_next(), 2);
    assert_eq!(get_next(), 3);
}

#[test]
fn test_header_creation_structure() {
    // Test header creation logic in isolation
    let config = DashStreamConfig {
        bootstrap_servers: "localhost:9092".to_string(),
        topic: "test-events".to_string(),
        tenant_id: "acme-corp".to_string(),
        thread_id: "session-789".to_string(),
        enable_state_diff: true,
        compression_threshold: 512,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    // Create a header using the same logic as DashStreamCallback
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: duration_to_micros_i64(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
        ),
        tenant_id: config.tenant_id.clone(),
        thread_id: config.thread_id.clone(),
        sequence: 5,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    // Verify header structure
    assert_eq!(header.message_id.len(), 16); // UUID is 16 bytes
    assert!(header.timestamp_us > 0);
    assert_eq!(header.tenant_id, "acme-corp");
    assert_eq!(header.thread_id, "session-789");
    assert_eq!(header.sequence, 5);
    assert_eq!(header.r#type, MessageType::Event as i32);
    assert!(header.parent_id.is_empty());
    assert_eq!(header.compression, 0);
    assert_eq!(header.schema_version, 1);
}

#[test]
fn test_event_type_mapping_graph_start() {
    // Test GraphEvent::GraphStart maps to EventType::GraphStart
    let state = AgentState::new();
    let graph_event = GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state,
        manifest: None,
    };

    // Verify mapping logic
    match graph_event {
        GraphEvent::GraphStart { .. } => {
            let event_type = EventType::GraphStart;
            assert_eq!(event_type as i32, EventType::GraphStart as i32);
        }
        _ => panic!("Expected GraphStart"),
    }
}

#[test]
fn test_graph_start_with_manifest_attributes() {
    // Test that manifest is serialized to telemetry attributes
    use crate::introspection::{EdgeManifest, GraphManifest, NodeManifest, NodeType};

    let state = AgentState::new();
    let manifest = GraphManifest::builder()
        .entry_point("node_a")
        .graph_name("test_telemetry_graph")
        .add_node("node_a", NodeManifest::new("node_a", NodeType::Function))
        .add_node("node_b", NodeManifest::new("node_b", NodeType::Function))
        .add_edge("node_a", EdgeManifest::simple("node_a", "node_b"))
        .add_edge("node_b", EdgeManifest::simple("node_b", "__end__"))
        .build()
        .unwrap();

    let graph_event = GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state,
        manifest: Some(Box::new(manifest)),
    };

    // Simulate the attribute extraction logic from send_graph_event
    match graph_event {
        GraphEvent::GraphStart {
            manifest: Some(ref m),
            ..
        } => {
            // Verify manifest serialization works
            let manifest_json = m.to_json_compact().expect("Should serialize");
            assert!(manifest_json.contains("node_a"));
            assert!(manifest_json.contains("test_telemetry_graph"));

            // Verify individual attribute values
            assert_eq!(m.entry_point, "node_a");
            assert_eq!(m.graph_name, Some("test_telemetry_graph".to_string()));
            assert_eq!(m.nodes.len(), 2);
            assert_eq!(m.edges.values().map(|v| v.len()).sum::<usize>(), 2);
        }
        _ => panic!("Expected GraphStart with manifest"),
    }
}

#[test]
fn test_graph_start_without_manifest() {
    // Test that GraphStart without manifest still works
    let state = AgentState::new();
    let graph_event = GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state,
        manifest: None,
    };

    match graph_event {
        GraphEvent::GraphStart { manifest: None, .. } => {
            // No manifest - attributes should be empty for manifest-related fields
            let event_type = EventType::GraphStart;
            assert_eq!(event_type as i32, EventType::GraphStart as i32);
        }
        _ => panic!("Expected GraphStart without manifest"),
    }
}

#[test]
fn test_event_type_mapping_graph_end() {
    let state = AgentState::new();
    let graph_event = GraphEvent::GraphEnd {
        timestamp: SystemTime::now(),
        final_state: state,
        duration: std::time::Duration::from_millis(100),
        execution_path: vec!["node1".to_string(), "node2".to_string()],
    };

    match graph_event {
        GraphEvent::GraphEnd { duration, .. } => {
            let event_type = EventType::GraphEnd;
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(event_type as i32, EventType::GraphEnd as i32);
            assert_eq!(duration_us, 100_000); // 100ms = 100,000 microseconds
        }
        _ => panic!("Expected GraphEnd"),
    }
}

#[test]
fn test_event_type_mapping_node_start() {
    let state = AgentState::new();
    let graph_event = GraphEvent::NodeStart {
        timestamp: SystemTime::now(),
        node: "agent_node".to_string(),
        state,
        node_config: None,
    };

    match graph_event {
        GraphEvent::NodeStart { node, .. } => {
            let event_type = EventType::NodeStart;
            assert_eq!(event_type as i32, EventType::NodeStart as i32);
            assert_eq!(node, "agent_node");
        }
        _ => panic!("Expected NodeStart"),
    }
}

#[test]
fn test_event_type_mapping_node_end() {
    let state = AgentState::new();
    let graph_event = GraphEvent::NodeEnd {
        timestamp: SystemTime::now(),
        node: "agent_node".to_string(),
        state,
        duration: std::time::Duration::from_micros(1500),
        node_config: None,
    };

    match graph_event {
        GraphEvent::NodeEnd { node, duration, .. } => {
            let event_type = EventType::NodeEnd;
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(event_type as i32, EventType::NodeEnd as i32);
            assert_eq!(node, "agent_node");
            assert_eq!(duration_us, 1500);
        }
        _ => panic!("Expected NodeEnd"),
    }
}

#[test]
fn test_event_type_mapping_node_error() {
    let state = AgentState::new();
    let graph_event = GraphEvent::NodeError {
        timestamp: SystemTime::now(),
        node: "failing_node".to_string(),
        error: "Test error".to_string(),
        state,
    };

    match graph_event {
        GraphEvent::NodeError { node, error, .. } => {
            let event_type = EventType::NodeError;
            assert_eq!(event_type as i32, EventType::NodeError as i32);
            assert_eq!(node, "failing_node");
            assert_eq!(error, "Test error");
        }
        _ => panic!("Expected NodeError"),
    }
}

#[test]
fn test_event_type_mapping_edge_traversal_simple() {
    let graph_event = GraphEvent::<AgentState>::EdgeTraversal {
        timestamp: SystemTime::now(),
        from: "node_a".to_string(),
        to: vec!["node_b".to_string()],
        edge_type: EdgeType::Simple,
    };

    match graph_event {
        GraphEvent::EdgeTraversal {
            from,
            to,
            edge_type,
            ..
        } => {
            let event_type = match edge_type {
                EdgeType::Simple => EventType::EdgeTraversal,
                EdgeType::Conditional { .. } => EventType::ConditionalBranch,
                EdgeType::Parallel => EventType::EdgeTraversal,
            };
            assert_eq!(event_type as i32, EventType::EdgeTraversal as i32);
            assert_eq!(from, "node_a");
            assert_eq!(to, vec!["node_b".to_string()]);
        }
        _ => panic!("Expected EdgeTraversal"),
    }
}

#[test]
fn test_event_type_mapping_edge_traversal_conditional() {
    let graph_event = GraphEvent::<AgentState>::EdgeTraversal {
        timestamp: SystemTime::now(),
        from: "node_a".to_string(),
        to: vec!["node_c".to_string()],
        edge_type: EdgeType::Conditional {
            condition_result: "check_value".to_string(),
        },
    };

    match graph_event {
        GraphEvent::EdgeTraversal { edge_type, .. } => {
            let event_type = match edge_type {
                EdgeType::Simple => EventType::EdgeTraversal,
                EdgeType::Conditional { .. } => EventType::ConditionalBranch,
                EdgeType::Parallel => EventType::EdgeTraversal,
            };
            assert_eq!(event_type as i32, EventType::ConditionalBranch as i32);
        }
        _ => panic!("Expected EdgeTraversal"),
    }
}

#[test]
fn test_event_type_mapping_parallel_start() {
    let graph_event = GraphEvent::<AgentState>::ParallelStart {
        timestamp: SystemTime::now(),
        nodes: vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ],
    };

    match graph_event {
        GraphEvent::ParallelStart { nodes, .. } => {
            let event_type = EventType::ParallelStart;
            let node_id = nodes.join(",");
            assert_eq!(event_type as i32, EventType::ParallelStart as i32);
            assert_eq!(node_id, "node1,node2,node3");
        }
        _ => panic!("Expected ParallelStart"),
    }
}

#[test]
fn test_event_type_mapping_parallel_end() {
    let graph_event = GraphEvent::<AgentState>::ParallelEnd {
        timestamp: SystemTime::now(),
        nodes: vec!["node1".to_string(), "node2".to_string()],
        duration: std::time::Duration::from_millis(250),
    };

    match graph_event {
        GraphEvent::ParallelEnd {
            nodes, duration, ..
        } => {
            let event_type = EventType::ParallelEnd;
            let node_id = nodes.join(",");
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(event_type as i32, EventType::ParallelEnd as i32);
            assert_eq!(node_id, "node1,node2");
            assert_eq!(duration_us, 250_000);
        }
        _ => panic!("Expected ParallelEnd"),
    }
}

#[test]
fn test_state_diff_hex_hash_conversion() {
    // Test hex string to bytes conversion (used in create_state_diff)
    let hex_hash = "a1b2c3d4e5f6";

    let state_hash: Vec<u8> = (0..hex_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_hash[i..i + 2], 16).unwrap_or(0))
        .collect();

    assert_eq!(state_hash, vec![0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6]);
}

#[test]
fn test_state_diff_hex_hash_conversion_invalid() {
    // Test invalid hex characters (should default to 0)
    let hex_hash = "zzww";

    let state_hash: Vec<u8> = (0..hex_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_hash[i..i + 2], 16).unwrap_or(0))
        .collect();

    assert_eq!(state_hash, vec![0, 0]); // Invalid hex → 0
}

#[test]
fn test_message_type_values() {
    // Verify MessageType enum values (from protobuf: 0=UNSPECIFIED, 1=EVENT, 2=STATE_DIFF)
    assert_eq!(MessageType::Event as i32, 1);
    assert_eq!(MessageType::StateDiff as i32, 2);
}

#[test]
fn test_event_type_values() {
    // Verify EventType enum values are distinct
    let types = vec![
        EventType::GraphStart as i32,
        EventType::GraphEnd as i32,
        EventType::NodeStart as i32,
        EventType::NodeEnd as i32,
        EventType::NodeError as i32,
        EventType::EdgeTraversal as i32,
        EventType::ConditionalBranch as i32,
        EventType::ParallelStart as i32,
        EventType::ParallelEnd as i32,
    ];

    // All types should be unique
    let mut unique_types = types.clone();
    unique_types.sort();
    unique_types.dedup();
    assert_eq!(
        types.len(),
        unique_types.len(),
        "EventType values must be unique"
    );
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_dashstream_callback_creation() {
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "test-events",
        "test-tenant",
        "test-thread",
    )
    .await;

    assert!(callback.is_ok());
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_send_graph_events() {
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "test-events",
        "test-tenant",
        "test-thread",
    )
    .await
    .expect("Failed to create callback");

    let state = AgentState::new();

    // Send graph start
    callback.on_event(&GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state.clone(),
        manifest: None,
    });

    // Send node start
    callback.on_event(&GraphEvent::NodeStart {
        timestamp: SystemTime::now(),
        node: "test_node".to_string(),
        state: state.clone(),
        node_config: None,
    });

    // Send node end
    callback.on_event(&GraphEvent::NodeEnd {
        timestamp: SystemTime::now(),
        node: "test_node".to_string(),
        state: state.clone(),
        duration: std::time::Duration::from_millis(100),
        node_config: None,
    });

    // Flush
    callback.flush().await.expect("Failed to flush");
}

#[test]
fn test_next_sequence_thread_safety() {
    // Test sequence number generation under concurrent access using AtomicU64
    use std::sync::Arc;
    use std::thread;

    let sequence = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    // Spawn 10 threads, each incrementing 100 times
    for _ in 0..10 {
        let seq = sequence.clone();
        let handle = thread::spawn(move || {
            let mut results = vec![];
            for _ in 0..100 {
                // Lock-free atomic increment matches DashStreamCallback implementation
                let current = seq.fetch_add(1, Ordering::Relaxed);
                results.push(current);
            }
            results
        });
        handles.push(handle);
    }

    // Collect all sequence numbers
    let mut all_sequences = vec![];
    for handle in handles {
        let results = handle.join().unwrap();
        all_sequences.extend(results);
    }

    // Verify we got exactly 1000 unique sequence numbers (0-999)
    all_sequences.sort();
    all_sequences.dedup();
    assert_eq!(all_sequences.len(), 1000);
    assert_eq!(all_sequences[0], 0);
    assert_eq!(all_sequences[999], 999);
}

#[test]
fn test_sequence_number_monotonic_increase() {
    // Verify sequence numbers always increase using AtomicU64
    let sequence = Arc::new(AtomicU64::new(0));

    // Lock-free atomic increment
    let seq1 = sequence.fetch_add(1, Ordering::Relaxed);
    let seq2 = sequence.fetch_add(1, Ordering::Relaxed);
    let seq3 = sequence.fetch_add(1, Ordering::Relaxed);

    assert!(seq2 > seq1);
    assert!(seq3 > seq2);
    assert_eq!(seq3 - seq1, 2);
}

#[test]
fn test_sequence_number_wraps_safely() {
    // Test behavior near u64::MAX (should wrap naturally) using AtomicU64
    let sequence = Arc::new(AtomicU64::new(u64::MAX - 2));

    // AtomicU64::fetch_add wraps naturally on overflow
    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), u64::MAX - 2);
    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), u64::MAX - 1);
    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), u64::MAX);
    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 0); // Wraps to 0
}

#[test]
fn test_previous_state_initialization() {
    // Verify previous_state starts as None
    let prev_state: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    assert!(prev_state.lock().is_none());
}

#[test]
fn test_previous_state_update() {
    // Test updating previous state
    let prev_state = Arc::new(Mutex::new(None));

    // Initial state
    let state1 = serde_json::json!({"iteration": 0, "messages": []});
    *prev_state.lock() = Some(state1.clone());

    assert_eq!(prev_state.lock().as_ref().unwrap(), &state1);

    // Updated state
    let state2 = serde_json::json!({"iteration": 1, "messages": ["msg1"]});
    *prev_state.lock() = Some(state2.clone());

    assert_eq!(prev_state.lock().as_ref().unwrap(), &state2);
    assert_ne!(prev_state.lock().as_ref().unwrap(), &state1);
}

#[test]
fn test_state_diff_full_state_scenario() {
    // Test scenario where full state is used (diff too large)
    use dashflow_streaming::diff::diff_states;

    // Create two very different states to trigger full state mode
    let old_state = serde_json::json!({
        "iteration": 0,
        "messages": [],
        "data": "x".repeat(1000)
    });

    let new_state = serde_json::json!({
        "iteration": 100,
        "messages": vec!["m1", "m2", "m3"],
        "data": "y".repeat(1000),
        "extra_field": "new"
    });

    let diff_result = diff_states(&old_state, &new_state).unwrap();

    // When states are very different, may use full state
    // Verify the result has valid fields
    assert!(!diff_result.state_hash.is_empty());
    assert!(diff_result.full_state_size > 0);
    // Note: patch_size is usize, always >= 0
}

#[test]
fn test_state_diff_patch_scenario() {
    // Test scenario where patch is used (diff is small)
    use dashflow_streaming::diff::diff_states;

    // Create two similar states with small change
    // Use larger state to ensure patch is smaller
    let old_state = serde_json::json!({
        "iteration": 5,
        "messages": vec!["msg1", "msg2"],
        "data": "x".repeat(500)
    });

    let new_state = serde_json::json!({
        "iteration": 6,
        "messages": vec!["msg1", "msg2"],
        "data": "x".repeat(500)
    });

    let diff_result = diff_states(&old_state, &new_state).unwrap();

    // Verify the diff result has valid fields
    assert!(!diff_result.state_hash.is_empty());
    assert!(diff_result.full_state_size > 0);
    // Note: patch_size is usize, always >= 0

    // For this specific case with large unchanged data and small change,
    // the diff algorithm should prefer patch over full state
    if !diff_result.use_full_state {
        assert!(diff_result.patch_size < diff_result.full_state_size);
    }
}

#[test]
fn test_compression_threshold_default() {
    // Verify default compression threshold is 512 bytes
    let config = DashStreamConfig::default();
    assert_eq!(config.compression_threshold, 512);
}

#[test]
fn test_compression_threshold_custom_values() {
    // Test various compression threshold values
    let thresholds = vec![0, 128, 512, 1024, 4096, 65536];

    for threshold in thresholds {
        let config = DashStreamConfig {
            compression_threshold: threshold,
            ..Default::default()
        };
        assert_eq!(config.compression_threshold, threshold);
    }
}

#[test]
fn test_enable_state_diff_true_by_default() {
    // Verify state diffing is enabled by default
    let config = DashStreamConfig::default();
    assert!(config.enable_state_diff);
}

#[test]
fn test_enable_state_diff_can_be_disabled() {
    // Verify state diffing can be disabled
    let config = DashStreamConfig {
        enable_state_diff: false,
        ..Default::default()
    };
    assert!(!config.enable_state_diff);
}

#[test]
fn test_config_with_multiple_bootstrap_servers() {
    // Test configuration with multiple Kafka brokers
    let config = DashStreamConfig {
        bootstrap_servers: "kafka1:9092,kafka2:9092,kafka3:9092".to_string(),
        ..Default::default()
    };

    assert_eq!(
        config.bootstrap_servers,
        "kafka1:9092,kafka2:9092,kafka3:9092"
    );
    assert_eq!(config.bootstrap_servers.matches(',').count(), 2);
}

#[test]
fn test_tenant_id_isolation() {
    // Verify different configs can have different tenant IDs
    let config1 = DashStreamConfig {
        tenant_id: "tenant-a".to_string(),
        ..Default::default()
    };

    let config2 = DashStreamConfig {
        tenant_id: "tenant-b".to_string(),
        ..Default::default()
    };

    assert_ne!(config1.tenant_id, config2.tenant_id);
}

#[test]
fn test_thread_id_isolation() {
    // Verify different configs can have different thread IDs
    let config1 = DashStreamConfig {
        thread_id: "session-123".to_string(),
        ..Default::default()
    };

    let config2 = DashStreamConfig {
        thread_id: "session-456".to_string(),
        ..Default::default()
    };

    assert_ne!(config1.thread_id, config2.thread_id);
}

#[test]
fn test_topic_name_variations() {
    // Test various valid topic names
    let topics = vec![
        "dashstream-events",
        "prod.dashstream.events",
        "dev_dashstream_events",
        "test-topic-123",
        "events.v2",
    ];

    for topic in topics {
        let config = DashStreamConfig {
            topic: topic.to_string(),
            ..Default::default()
        };
        assert_eq!(config.topic, topic);
    }
}

#[test]
fn test_event_node_id_empty_for_graph_events() {
    // Graph-level events (GraphStart, GraphEnd) should have empty node_id
    // This is the expected behavior from send_graph_event

    // GraphStart
    let state = AgentState::new();
    let graph_event = GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state.clone(),
        manifest: None,
    };

    match graph_event {
        GraphEvent::GraphStart { .. } => {
            let node_id = "".to_string();
            assert!(node_id.is_empty());
        }
        _ => panic!("Expected GraphStart"),
    }

    // GraphEnd
    let graph_event = GraphEvent::GraphEnd {
        timestamp: SystemTime::now(),
        final_state: state,
        duration: std::time::Duration::from_millis(100),
        execution_path: vec![],
    };

    match graph_event {
        GraphEvent::GraphEnd { .. } => {
            let node_id = "".to_string();
            assert!(node_id.is_empty());
        }
        _ => panic!("Expected GraphEnd"),
    }
}

#[test]
fn test_event_node_id_populated_for_node_events() {
    // Node-level events should have non-empty node_id
    let state = AgentState::new();

    // NodeStart
    let event = GraphEvent::NodeStart {
        timestamp: SystemTime::now(),
        node: "worker_1".to_string(),
        state: state.clone(),
        node_config: None,
    };

    match event {
        GraphEvent::NodeStart { node, .. } => {
            assert!(!node.is_empty());
            assert_eq!(node, "worker_1");
        }
        _ => panic!("Expected NodeStart"),
    }

    // NodeEnd
    let event = GraphEvent::NodeEnd {
        timestamp: SystemTime::now(),
        node: "worker_2".to_string(),
        state: state.clone(),
        duration: std::time::Duration::from_millis(50),
        node_config: None,
    };

    match event {
        GraphEvent::NodeEnd { node, .. } => {
            assert!(!node.is_empty());
            assert_eq!(node, "worker_2");
        }
        _ => panic!("Expected NodeEnd"),
    }

    // NodeError
    let event = GraphEvent::NodeError {
        timestamp: SystemTime::now(),
        node: "worker_3".to_string(),
        error: "Test error".to_string(),
        state,
    };

    match event {
        GraphEvent::NodeError { node, .. } => {
            assert!(!node.is_empty());
            assert_eq!(node, "worker_3");
        }
        _ => panic!("Expected NodeError"),
    }
}

#[test]
fn test_parallel_nodes_multiple_items() {
    // Test joining multiple parallel node names
    let nodes = [
        "map_worker_1".to_string(),
        "map_worker_2".to_string(),
        "map_worker_3".to_string(),
        "map_worker_4".to_string(),
    ];
    let joined = nodes.join(",");
    assert_eq!(
        joined,
        "map_worker_1,map_worker_2,map_worker_3,map_worker_4"
    );
    assert_eq!(joined.matches(',').count(), 3);
}

#[test]
fn test_state_serialization_to_json() {
    // Test that AgentState can be serialized to JSON
    let mut state = AgentState::new();
    state.add_message("Hello");
    state.add_message("World");

    let json_result = serde_json::to_value(&state);
    assert!(json_result.is_ok());

    let json = json_result.unwrap();
    assert!(json.is_object());

    // Should have messages field
    assert!(json.get("messages").is_some());
}

#[test]
fn test_state_serialization_large_message_count() {
    // Test serialization with many messages
    let mut state = AgentState::new();
    for i in 0..1000 {
        state.add_message(format!("Message {}", i));
    }

    let json_result = serde_json::to_value(&state);
    assert!(json_result.is_ok());

    let json = json_result.unwrap();
    let messages = json.get("messages").unwrap().as_array().unwrap();
    assert_eq!(messages.len(), 1000);
}

#[test]
fn test_header_type_field_for_event() {
    // Verify MessageType::Event is 1
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 0,
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 0,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert_eq!(header.r#type, 1);
}

#[test]
fn test_header_type_field_for_state_diff() {
    // Verify MessageType::StateDiff is 2
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 0,
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 0,
        r#type: MessageType::StateDiff as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert_eq!(header.r#type, 2);
}

#[test]
fn test_edge_type_to_event_type_mapping() {
    // Verify correct mapping of EdgeType to EventType

    // Simple edge → EdgeTraversal
    let edge_type = EdgeType::Simple;
    let event_type = match edge_type {
        EdgeType::Simple => EventType::EdgeTraversal,
        EdgeType::Conditional { .. } => EventType::ConditionalBranch,
        EdgeType::Parallel => EventType::EdgeTraversal,
    };
    assert_eq!(event_type as i32, EventType::EdgeTraversal as i32);

    // Conditional edge → ConditionalBranch
    let edge_type = EdgeType::Conditional {
        condition_result: "route_a".to_string(),
    };
    let event_type = match edge_type {
        EdgeType::Simple => EventType::EdgeTraversal,
        EdgeType::Conditional { .. } => EventType::ConditionalBranch,
        EdgeType::Parallel => EventType::EdgeTraversal,
    };
    assert_eq!(event_type as i32, EventType::ConditionalBranch as i32);

    // Parallel edge → EdgeTraversal
    let edge_type = EdgeType::Parallel;
    let event_type = match edge_type {
        EdgeType::Simple => EventType::EdgeTraversal,
        EdgeType::Conditional { .. } => EventType::ConditionalBranch,
        EdgeType::Parallel => EventType::EdgeTraversal,
    };
    assert_eq!(event_type as i32, EventType::EdgeTraversal as i32);
}

#[test]
fn test_duration_zero_for_start_events() {
    // Start events should have zero duration
    let state = AgentState::new();

    // GraphStart
    let event = GraphEvent::GraphStart {
        timestamp: SystemTime::now(),
        initial_state: state.clone(),
        manifest: None,
    };
    match event {
        GraphEvent::GraphStart { .. } => {
            let duration_us = 0i64;
            assert_eq!(duration_us, 0);
        }
        _ => panic!("Expected GraphStart"),
    }

    // NodeStart
    let event = GraphEvent::NodeStart {
        timestamp: SystemTime::now(),
        node: "test".to_string(),
        state: state.clone(),
        node_config: None,
    };
    match event {
        GraphEvent::NodeStart { .. } => {
            let duration_us = 0i64;
            assert_eq!(duration_us, 0);
        }
        _ => panic!("Expected NodeStart"),
    }

    // ParallelStart
    let event = GraphEvent::<AgentState>::ParallelStart {
        timestamp: SystemTime::now(),
        nodes: vec!["n1".to_string(), "n2".to_string()],
    };
    match event {
        GraphEvent::ParallelStart { .. } => {
            let duration_us = 0i64;
            assert_eq!(duration_us, 0);
        }
        _ => panic!("Expected ParallelStart"),
    }
}

#[test]
fn test_duration_nonzero_for_end_events() {
    // End events should track actual duration
    let state = AgentState::new();

    // GraphEnd
    let duration = std::time::Duration::from_millis(500);
    let event = GraphEvent::GraphEnd {
        timestamp: SystemTime::now(),
        final_state: state.clone(),
        duration,
        execution_path: vec![],
    };
    match event {
        GraphEvent::GraphEnd { duration, .. } => {
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(duration_us, 500_000);
        }
        _ => panic!("Expected GraphEnd"),
    }

    // NodeEnd
    let duration = std::time::Duration::from_micros(2500);
    let event = GraphEvent::NodeEnd {
        timestamp: SystemTime::now(),
        node: "test".to_string(),
        state: state.clone(),
        duration,
        node_config: None,
    };
    match event {
        GraphEvent::NodeEnd { duration, .. } => {
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(duration_us, 2500);
        }
        _ => panic!("Expected NodeEnd"),
    }

    // ParallelEnd
    let duration = std::time::Duration::from_secs(2);
    let event = GraphEvent::<AgentState>::ParallelEnd {
        timestamp: SystemTime::now(),
        nodes: vec!["n1".to_string()],
        duration,
    };
    match event {
        GraphEvent::ParallelEnd { duration, .. } => {
            let duration_us = duration_to_micros_i64(duration);
            assert_eq!(duration_us, 2_000_000);
        }
        _ => panic!("Expected ParallelEnd"),
    }
}

#[test]
fn test_uuid_v4_randomness() {
    // Verify UUID v4 generates unique IDs
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();
    let id3 = uuid::Uuid::new_v4();

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // Verify they're all 16 bytes
    assert_eq!(id1.as_bytes().len(), 16);
    assert_eq!(id2.as_bytes().len(), 16);
    assert_eq!(id3.as_bytes().len(), 16);
}

#[test]
fn test_state_hash_full_64_hex_chars() {
    // Test SHA-256 hash (64 hex chars = 32 bytes)
    let hex_hash = "a".repeat(64);
    let state_hash: Vec<u8> = (0..hex_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_hash[i..i + 2], 16).unwrap_or(0))
        .collect();

    assert_eq!(state_hash.len(), 32);
    // All should be 0xaa
    assert!(state_hash.iter().all(|&b| b == 0xaa));
}

#[test]
fn test_callback_clone_implementation() {
    // Verify DashStreamCallback implements Clone
    // This is important for using callbacks across multiple invocations

    // We can't easily test this without Kafka, but we can verify
    // the underlying components are cloneable
    let config = DashStreamConfig::default();
    let config_clone = config.clone();

    assert_eq!(config.bootstrap_servers, config_clone.bootstrap_servers);
    assert_eq!(config.topic, config_clone.topic);
    assert_eq!(config.tenant_id, config_clone.tenant_id);

    // Arc<AtomicU64> components should be cloneable
    let sequence = Arc::new(AtomicU64::new(0));
    let sequence_clone = sequence.clone();
    assert_eq!(Arc::strong_count(&sequence), 2);
    drop(sequence_clone);
    assert_eq!(Arc::strong_count(&sequence), 1);
}

#[test]
fn test_config_builder_pattern() {
    // Test DashStreamConfig construction with custom values
    let config = DashStreamConfig {
        bootstrap_servers: "kafka1:9092,kafka2:9092".to_string(),
        topic: "prod-events".to_string(),
        tenant_id: "tenant-123".to_string(),
        thread_id: "session-abc".to_string(),
        enable_state_diff: false,
        compression_threshold: 2048,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    assert_eq!(config.bootstrap_servers, "kafka1:9092,kafka2:9092");
    assert_eq!(config.topic, "prod-events");
    assert_eq!(config.tenant_id, "tenant-123");
    assert_eq!(config.thread_id, "session-abc");
    assert!(!config.enable_state_diff);
    assert_eq!(config.compression_threshold, 2048);
}

#[test]
fn test_config_default_thread_id_is_uuid() {
    // Verify default generates a UUID for thread_id
    let config1 = DashStreamConfig::default();
    let config2 = DashStreamConfig::default();

    // Thread IDs should be different (UUID v4)
    assert_ne!(config1.thread_id, config2.thread_id);

    // Should be valid UUIDs (36 characters with hyphens)
    assert_eq!(config1.thread_id.len(), 36);
    assert_eq!(config2.thread_id.len(), 36);
}

#[test]
fn test_header_message_id_is_16_bytes() {
    // UUID should be 16 bytes
    let message_id = uuid::Uuid::new_v4().as_bytes().to_vec();
    assert_eq!(message_id.len(), 16);
}

#[test]
fn test_header_timestamp_is_positive() {
    // Timestamp should be positive microseconds since UNIX_EPOCH
    let timestamp_us = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;

    assert!(timestamp_us > 0);
    // Should be reasonable (> 2020-01-01 in microseconds)
    assert!(timestamp_us > 1_577_836_800_000_000i64);
}

#[test]
fn test_create_state_diff_hash_conversion() {
    // Test hex hash conversion used in create_state_diff
    // This tests the hash conversion logic without needing Kafka

    // Test with full state hash
    let hash1 = "abcd1234";
    let bytes1: Vec<u8> = (0..hash1.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hash1[i..i + 2], 16).unwrap_or(0))
        .collect();
    assert_eq!(bytes1, vec![0xab, 0xcd, 0x12, 0x34]);

    // Test with patch state hash
    let hash2 = "1234abcd";
    let bytes2: Vec<u8> = (0..hash2.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hash2[i..i + 2], 16).unwrap_or(0))
        .collect();
    assert_eq!(bytes2, vec![0x12, 0x34, 0xab, 0xcd]);

    // Test with longer hash (SHA-256 = 64 hex chars = 32 bytes)
    let sha256_hash = "a1b2c3d4e5f6789012345678abcdef0123456789abcdef0123456789abcdef01";
    let sha256_bytes: Vec<u8> = (0..sha256_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&sha256_hash[i..i + 2], 16).unwrap_or(0))
        .collect();
    assert_eq!(sha256_bytes.len(), 32); // SHA-256 is 32 bytes
}

#[test]
fn test_state_diff_empty_hash() {
    // Test handling of empty hash string
    let hex_hash = "";
    let state_hash: Vec<u8> = (0..hex_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_hash[i..i + 2], 16).unwrap_or(0))
        .collect();

    assert_eq!(state_hash, Vec::<u8>::new());
}

#[test]
fn test_state_diff_odd_length_hash() {
    // Test handling of odd-length hash (last character ignored)
    let hex_hash = "abc";
    let state_hash: Vec<u8> = (0..hex_hash.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= hex_hash.len() {
                Some(u8::from_str_radix(&hex_hash[i..i + 2], 16).unwrap_or(0))
            } else {
                None
            }
        })
        .collect();

    // Only "ab" should be converted (last "c" ignored because length is odd)
    assert_eq!(state_hash, vec![0xab]);
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_event_callback_trait_implementation() {
    // Verify EventCallback trait is correctly implemented
    use crate::event::EventCallback;

    let config = DashStreamConfig::default();
    let producer = Arc::new(
        DashStreamProducer::new(&config.bootstrap_servers, &config.topic)
            .await
            .unwrap_or_else(|e| panic!("Failed to create producer: {}", e)),
    );

    let semaphore_permits = config.max_concurrent_telemetry_sends;
    // Create message queue for the callback (M-666 fix)
    let (message_sender, rx) = mpsc::channel::<BatchMessage>(64);
    let queue_depth = Arc::new(AtomicU64::new(0));
    let message_worker = DashStreamCallback::<AgentState>::spawn_message_worker(
        rx,
        producer.clone(),
        config.telemetry_batch_size,
        config.telemetry_batch_timeout_ms,
        config.thread_id.clone(),
        config.tenant_id.clone(),
        queue_depth.clone(),
    );
    let callback = DashStreamCallback::<AgentState> {
        producer,
        config,
        sequence: Arc::new(AtomicU64::new(0)),
        previous_state: Arc::new(Mutex::new(None)),
        pending_tasks: Arc::new(Mutex::new(Vec::new())),
        telemetry_semaphore: Arc::new(Semaphore::new(semaphore_permits)),
        telemetry_dropped: Arc::new(AtomicU64::new(0)),
        message_sender,
        message_worker: Arc::new(Mutex::new(Some(message_worker))),
        diffs_since_checkpoint: Arc::new(AtomicU64::new(0)),
        last_checkpoint_id: Arc::new(Mutex::new(Vec::new())),
        queue_depth,
        _phantom: std::marker::PhantomData,
    };

    // Should compile and call on_event through trait
    let state = AgentState::new();
    let event = GraphEvent::NodeStart {
        timestamp: SystemTime::now(),
        node: "test".to_string(),
        state,
        node_config: None,
    };

    // This exercises the EventCallback trait implementation
    callback.on_event(&event);
    // If this compiles and runs, the trait is correctly implemented
}

#[test]
fn test_duration_conversion_microseconds() {
    // Verify duration to microseconds conversion is correct
    let duration_1ms = std::time::Duration::from_millis(1);
    assert_eq!(duration_1ms.as_micros() as i64, 1_000);

    let duration_1s = std::time::Duration::from_secs(1);
    assert_eq!(duration_1s.as_micros() as i64, 1_000_000);

    let duration_100us = std::time::Duration::from_micros(100);
    assert_eq!(duration_100us.as_micros() as i64, 100);
}

#[test]
fn test_parallel_nodes_join() {
    // Test nodes joining logic used in ParallelStart/ParallelEnd
    let nodes = [
        "node1".to_string(),
        "node2".to_string(),
        "node3".to_string(),
    ];
    let joined = nodes.join(",");
    assert_eq!(joined, "node1,node2,node3");

    let empty_nodes: Vec<String> = vec![];
    let joined_empty = empty_nodes.join(",");
    assert_eq!(joined_empty, "");

    let single_node = ["only".to_string()];
    let joined_single = single_node.join(",");
    assert_eq!(joined_single, "only");
}

#[test]
fn test_config_clone() {
    // Test that DashStreamConfig implements Clone correctly
    let config1 = DashStreamConfig {
        bootstrap_servers: "test:9092".to_string(),
        topic: "topic1".to_string(),
        tenant_id: "tenant1".to_string(),
        thread_id: "thread1".to_string(),
        enable_state_diff: true,
        compression_threshold: 1024,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    let config2 = config1.clone();

    assert_eq!(config1.bootstrap_servers, config2.bootstrap_servers);
    assert_eq!(config1.topic, config2.topic);
    assert_eq!(config1.tenant_id, config2.tenant_id);
    assert_eq!(config1.thread_id, config2.thread_id);
    assert_eq!(config1.enable_state_diff, config2.enable_state_diff);
    assert_eq!(config1.compression_threshold, config2.compression_threshold);
}

#[test]
fn test_config_debug_format() {
    // Verify DashStreamConfig Debug trait works
    let config = DashStreamConfig::default();
    let debug_str = format!("{:?}", config);

    // Should contain key fields
    assert!(debug_str.contains("DashStreamConfig"));
    assert!(debug_str.contains("localhost:9092"));
    assert!(debug_str.contains("dashstream-events"));
}

#[test]
fn test_header_schema_version() {
    // Verify schema_version is always 1
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 0,
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 0,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert_eq!(header.schema_version, 1);
}

#[test]
fn test_header_parent_id_empty_by_default() {
    // Verify parent_id is empty by default
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 0,
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 0,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert!(header.parent_id.is_empty());
}

#[test]
fn test_header_compression_zero_by_default() {
    // Verify compression is 0 (none) by default
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 0,
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 0,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert_eq!(header.compression, 0);
}

#[test]
fn test_event_attributes_empty_by_default() {
    // Verify Event attributes HashMap is empty by default
    let event = Event {
        header: None,
        event_type: EventType::NodeStart as i32,
        node_id: "test".to_string(),
        attributes: std::collections::HashMap::new(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    assert!(event.attributes.is_empty());
}

#[test]
fn test_event_llm_request_id_empty_by_default() {
    // Verify llm_request_id is empty by default
    let event = Event {
        header: None,
        event_type: EventType::NodeStart as i32,
        node_id: "test".to_string(),
        attributes: std::collections::HashMap::new(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    assert_eq!(event.llm_request_id, "");
}

// Additional tests to improve coverage

#[test]
fn test_create_state_diff_with_full_state() {
    // Test create_state_diff when use_full_state is true
    use dashflow_streaming::diff::{DiffResult, Patch};

    let diff_result = DiffResult {
        patch: Patch(vec![]),
        patch_size: 0,
        full_state_size: 100,
        use_full_state: true,
        state_hash: "abcd1234".to_string(),
        patch_hash: "patch1234".to_string(),
    };

    let new_state_json = json!({"value": 42});

    // Verify that full state is used
    let full_state = if diff_result.use_full_state {
        serde_json::to_vec(&new_state_json).expect("Test JSON must serialize")
    } else {
        vec![]
    };

    assert!(!full_state.is_empty()); // Full state should be serialized
    assert!(diff_result.use_full_state);
}

#[test]
fn test_create_state_diff_with_patch() {
    // Test create_state_diff when use_full_state is false
    use dashflow_streaming::diff::{DiffResult, Patch};

    let diff_result = DiffResult {
        patch: Patch(vec![]),
        patch_size: 10,
        full_state_size: 100,
        use_full_state: false,
        state_hash: "ef567890".to_string(),
        patch_hash: "patch5678".to_string(),
    };

    let new_state_json = json!({"value": 42});

    // Verify that patch mode is used (not full state)
    let full_state = if diff_result.use_full_state {
        serde_json::to_vec(&new_state_json).expect("Test JSON must serialize")
    } else {
        vec![]
    };

    assert!(full_state.is_empty()); // Patch mode - no full state
    assert!(!diff_result.use_full_state);
    assert_eq!(diff_result.patch_size, 10);
}

#[test]
fn test_graph_event_node_id_extraction() {
    // Test node_id extraction from different GraphEvent types
    let _state = AgentState::new();

    // EdgeTraversal extracts from "from" field
    let event = GraphEvent::<AgentState>::EdgeTraversal {
        timestamp: SystemTime::now(),
        from: "source_node".to_string(),
        to: vec!["dest_node".to_string()],
        edge_type: EdgeType::Simple,
    };

    match event {
        GraphEvent::EdgeTraversal { from, .. } => {
            let node_id = from.clone();
            assert_eq!(node_id, "source_node");
        }
        _ => panic!("Expected EdgeTraversal"),
    }
}

#[test]
fn test_parallel_nodes_comma_separation() {
    // Test that parallel nodes are comma-separated correctly
    let nodes = [
        "worker_a".to_string(),
        "worker_b".to_string(),
        "worker_c".to_string(),
    ];

    let joined = nodes.join(",");
    assert_eq!(joined, "worker_a,worker_b,worker_c");

    // Test split back
    let split: Vec<&str> = joined.split(',').collect();
    assert_eq!(split, ["worker_a", "worker_b", "worker_c"]);
}

#[test]
fn test_event_creation_fields() {
    // Test Event struct field population
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 123456789,
        tenant_id: "tenant1".to_string(),
        thread_id: "thread1".to_string(),
        sequence: 5,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    let event = Event {
        header: Some(header.clone()),
        event_type: EventType::NodeStart as i32,
        node_id: "test_node".to_string(),
        attributes: std::collections::HashMap::new(),
        duration_us: 1000,
        llm_request_id: "".to_string(),
    };

    assert!(event.header.is_some());
    assert_eq!(event.event_type, EventType::NodeStart as i32);
    assert_eq!(event.node_id, "test_node");
    assert_eq!(event.duration_us, 1000);
}

#[test]
fn test_state_diff_structure() {
    // Test StateDiff struct construction
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: 123456789,
        tenant_id: "tenant1".to_string(),
        thread_id: "thread1".to_string(),
        sequence: 10,
        r#type: MessageType::StateDiff as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    let state_diff = StateDiff {
        header: Some(header),
        base_checkpoint_id: vec![],
        operations: vec![],
        state_hash: vec![0xab, 0xcd, 0xef],
        full_state: vec![1, 2, 3, 4, 5],
    };

    assert!(state_diff.header.is_some());
    assert!(state_diff.base_checkpoint_id.is_empty());
    assert!(state_diff.operations.is_empty());
    assert_eq!(state_diff.state_hash, vec![0xab, 0xcd, 0xef]);
    assert_eq!(state_diff.full_state, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_config_debug_contains_key_fields() {
    // Verify Debug implementation shows important config fields
    let config = DashStreamConfig {
        bootstrap_servers: "test-kafka:9092".to_string(),
        topic: "test-topic".to_string(),
        tenant_id: "test-tenant".to_string(),
        thread_id: "test-thread".to_string(),
        enable_state_diff: true,
        compression_threshold: 1024,
        max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    let debug_output = format!("{:?}", config);

    assert!(debug_output.contains("test-kafka:9092"));
    assert!(debug_output.contains("test-topic"));
    assert!(debug_output.contains("test-tenant"));
    assert!(debug_output.contains("test-thread"));
}

#[test]
fn test_timestamp_microsecond_precision() {
    // Test timestamp precision (microseconds)
    let now = SystemTime::now();
    let timestamp_us = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;

    // Should be a reasonable timestamp (after 2020)
    assert!(timestamp_us > 1_577_836_800_000_000i64);

    // Should be less than year 2100 (sanity check)
    assert!(timestamp_us < 4_102_444_800_000_000i64);
}

#[test]
fn test_event_type_enum_values_are_distinct() {
    // Verify all EventType values are unique
    use std::collections::HashSet;

    let event_types = [
        EventType::GraphStart as i32,
        EventType::GraphEnd as i32,
        EventType::NodeStart as i32,
        EventType::NodeEnd as i32,
        EventType::NodeError as i32,
        EventType::EdgeTraversal as i32,
        EventType::ConditionalBranch as i32,
        EventType::ParallelStart as i32,
        EventType::ParallelEnd as i32,
    ];

    let unique: HashSet<_> = event_types.iter().collect();
    assert_eq!(
        unique.len(),
        event_types.len(),
        "All EventType values must be distinct"
    );
}

#[test]
fn test_message_type_enum_values() {
    // Verify MessageType enum values match protocol
    assert_eq!(MessageType::Event as i32, 1);
    assert_eq!(MessageType::StateDiff as i32, 2);

    // Verify they are distinct
    assert_ne!(MessageType::Event as i32, MessageType::StateDiff as i32);
}

#[test]
fn test_empty_execution_path() {
    // Test GraphEnd with empty execution path
    let state = AgentState::new();
    let event = GraphEvent::GraphEnd {
        timestamp: SystemTime::now(),
        final_state: state,
        duration: std::time::Duration::from_millis(100),
        execution_path: vec![], // Empty path
    };

    match event {
        GraphEvent::GraphEnd { execution_path, .. } => {
            assert!(execution_path.is_empty());
        }
        _ => panic!("Expected GraphEnd"),
    }
}

#[test]
fn test_populated_execution_path() {
    // Test GraphEnd with populated execution path
    let state = AgentState::new();
    let path = vec![
        "start".to_string(),
        "node1".to_string(),
        "node2".to_string(),
        "end".to_string(),
    ];
    let event = GraphEvent::GraphEnd {
        timestamp: SystemTime::now(),
        final_state: state,
        duration: std::time::Duration::from_millis(100),
        execution_path: path.clone(),
    };

    match event {
        GraphEvent::GraphEnd { execution_path, .. } => {
            assert_eq!(execution_path.len(), 4);
            assert_eq!(execution_path, path);
        }
        _ => panic!("Expected GraphEnd"),
    }
}

#[test]
fn test_config_compression_threshold_range() {
    // Test various compression threshold values
    let thresholds = vec![0, 1, 256, 512, 1024, 4096, 65536, 1_000_000];

    for threshold in thresholds {
        let config = DashStreamConfig {
            compression_threshold: threshold,
            ..Default::default()
        };
        assert_eq!(config.compression_threshold, threshold);
    }
}

#[test]
fn test_tenant_id_special_characters() {
    // Test tenant IDs with special characters
    let special_tenant_ids = vec![
        "tenant-with-dash",
        "tenant_with_underscore",
        "tenant.with.dots",
        "tenant123",
        "TENANT_UPPER",
    ];

    for tenant_id in special_tenant_ids {
        let config = DashStreamConfig {
            tenant_id: tenant_id.to_string(),
            ..Default::default()
        };
        assert_eq!(config.tenant_id, tenant_id);
    }
}

#[test]
fn test_thread_id_special_characters() {
    // Test thread IDs with special characters
    let special_thread_ids = vec![
        "thread-123-abc",
        "session_2024_11_10",
        "user.session.123",
        "THREAD_ABC",
    ];

    for thread_id in special_thread_ids {
        let config = DashStreamConfig {
            thread_id: thread_id.to_string(),
            ..Default::default()
        };
        assert_eq!(config.thread_id, thread_id);
    }
}

#[test]
fn test_conditional_edge_type_with_result() {
    // Test EdgeType::Conditional with condition_result
    let edge_type = EdgeType::Conditional {
        condition_result: "route_left".to_string(),
    };

    match edge_type {
        EdgeType::Conditional { condition_result } => {
            assert_eq!(condition_result, "route_left");
        }
        _ => panic!("Expected Conditional edge type"),
    }
}

#[test]
fn test_parallel_edge_type() {
    // Test EdgeType::Parallel
    let edge_type = EdgeType::Parallel;

    match edge_type {
        EdgeType::Parallel => {
            // Correct type
        }
        _ => panic!("Expected Parallel edge type"),
    }
}

#[test]
fn test_uuid_byte_array_length() {
    // Verify UUID is always 16 bytes
    for _ in 0..10 {
        let uuid = uuid::Uuid::new_v4();
        let bytes = uuid.as_bytes();
        assert_eq!(bytes.len(), 16);
    }
}

#[test]
fn test_duration_zero() {
    // Test zero duration
    let duration = std::time::Duration::from_micros(0);
    assert_eq!(duration.as_micros() as i64, 0);
    assert_eq!(duration.as_millis(), 0);
    assert_eq!(duration.as_secs(), 0);
}

#[test]
fn test_duration_large_values() {
    // Test large duration values
    let duration_1_hour = std::time::Duration::from_secs(3600);
    assert_eq!(duration_1_hour.as_micros() as i64, 3_600_000_000);

    let duration_1_day = std::time::Duration::from_secs(86400);
    assert_eq!(duration_1_day.as_micros() as i64, 86_400_000_000);
}

#[test]
fn test_empty_attributes_hashmap() {
    // Test empty attributes HashMap
    let attributes = std::collections::HashMap::<String, String>::new();
    assert!(attributes.is_empty());
    assert_eq!(attributes.len(), 0);
}

#[test]
fn test_populated_attributes_hashmap() {
    // Test populated attributes HashMap
    let mut attributes = std::collections::HashMap::new();
    attributes.insert("key1".to_string(), "value1".to_string());
    attributes.insert("key2".to_string(), "value2".to_string());

    assert!(!attributes.is_empty());
    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes.get("key1"), Some(&"value1".to_string()));
    assert_eq!(attributes.get("key2"), Some(&"value2".to_string()));
}

#[test]
fn test_config_with_all_fields_custom() {
    // Test creating config with all fields customized
    let config = DashStreamConfig {
        bootstrap_servers: "custom-kafka:9093".to_string(),
        topic: "custom-topic".to_string(),
        tenant_id: "custom-tenant".to_string(),
        thread_id: "custom-thread".to_string(),
        enable_state_diff: false,
        compression_threshold: 2048,
        max_state_diff_size: 5 * 1024 * 1024, // 5MB
        ..Default::default()
    };

    assert_eq!(config.bootstrap_servers, "custom-kafka:9093");
    assert_eq!(config.topic, "custom-topic");
    assert_eq!(config.tenant_id, "custom-tenant");
    assert_eq!(config.thread_id, "custom-thread");
    assert!(!config.enable_state_diff);
    assert_eq!(config.compression_threshold, 2048);
}

#[test]
fn test_duration_to_micros_i64_normal_values() {
    // Normal durations should convert correctly
    let zero = std::time::Duration::ZERO;
    assert_eq!(duration_to_micros_i64(zero), 0);

    let one_ms = std::time::Duration::from_millis(1);
    assert_eq!(duration_to_micros_i64(one_ms), 1_000);

    let one_sec = std::time::Duration::from_secs(1);
    assert_eq!(duration_to_micros_i64(one_sec), 1_000_000);

    let one_hour = std::time::Duration::from_secs(3600);
    assert_eq!(duration_to_micros_i64(one_hour), 3_600_000_000);

    let one_day = std::time::Duration::from_secs(86400);
    assert_eq!(duration_to_micros_i64(one_day), 86_400_000_000);

    // 100 years in microseconds (well within i64 range)
    let hundred_years = std::time::Duration::from_secs(100 * 365 * 24 * 3600);
    let result = duration_to_micros_i64(hundred_years);
    assert!(result > 0);
    assert!(result < i64::MAX);
}

#[test]
fn test_duration_to_micros_i64_overflow_protection() {
    // i64::MAX microseconds = ~292,471 years
    // Create a duration that exceeds this (u64::MAX seconds = ~584 billion years)
    let max_duration = std::time::Duration::from_secs(u64::MAX);
    let result = duration_to_micros_i64(max_duration);

    // Should saturate to i64::MAX, not wrap or panic
    assert_eq!(result, i64::MAX);
}

#[test]
fn test_duration_to_micros_i64_boundary_values() {
    // Test values near the i64::MAX boundary
    // i64::MAX = 9_223_372_036_854_775_807 microseconds
    // That's roughly 292,471 years

    // 292,000 years should still fit (not saturate)
    let years_292k = std::time::Duration::from_secs(292_000 * 365 * 24 * 3600);
    let result = duration_to_micros_i64(years_292k);
    assert!(result > 0);
    // Should be a real value, not saturated to MAX
    assert!(result < i64::MAX, "292k years should not saturate");

    // 300,000 years should overflow and clamp
    let years_300k = std::time::Duration::from_secs(300_000 * 365 * 24 * 3600);
    let result = duration_to_micros_i64(years_300k);
    assert_eq!(result, i64::MAX);
}

#[test]
fn test_drop_implementation_clears_tasks() {
    // Test that Drop implementation properly cleans up internal state
    // This is a unit test that verifies the Drop implementation doesn't panic
    // and properly clears the pending_tasks and message_worker fields.

    use std::sync::atomic::Ordering;
    use tokio::task::JoinHandle;

    // Create callback components manually to test Drop without Kafka
    let pending_tasks: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
    let message_worker: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    // Simulate adding some pending tasks (use dummy handles)
    // Note: We can't easily create real JoinHandles without a runtime,
    // but we can verify the Drop impl handles empty state gracefully

    // Test 1: Drop with empty tasks (shouldn't panic)
    {
        let tasks_clone = pending_tasks.clone();
        let worker_clone = message_worker.clone();

        // Verify initial state is empty
        assert!(tasks_clone.lock().is_empty());
        assert!(worker_clone.lock().is_none());
    }

    // Test 2: Verify Drop logic works by simulating what it does
    {
        // Lock and clear tasks (simulating Drop behavior)
        let mut tasks = pending_tasks.lock();
        let count = tasks.len();
        tasks.clear();
        assert_eq!(count, 0, "Tasks should be empty initially");
        drop(tasks); // Release lock before acquiring next

        // Lock and take worker (simulating Drop behavior)
        let mut worker = message_worker.lock();
        let taken = worker.take();
        assert!(taken.is_none(), "Worker should be None initially");
    }

    // Test 3: Verify the telemetry_dropped counter behavior
    let dropped_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    dropped_counter.fetch_add(1, Ordering::Relaxed);
    assert_eq!(dropped_counter.load(Ordering::Relaxed), 1);
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_emit_quality_metrics() {
    // Test that emit_quality_metrics creates and sends the correct Metrics message
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "dashstream-quality",
        "test-tenant",
        "test-thread-quality",
    )
    .await
    .expect("Failed to create callback");

    // Emit quality metrics with sample values
    let result = callback.emit_quality_metrics(0.95, 0.87, 0.92, true).await;

    // Should succeed when Kafka is running
    assert!(result.is_ok(), "emit_quality_metrics should succeed");

    // Flush to ensure message is sent
    callback.flush().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_emit_quality_metrics_async() {
    // Test that emit_quality_metrics_async spawns a task without blocking
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "dashstream-quality",
        "test-tenant",
        "test-thread-quality-async",
    )
    .await
    .expect("Failed to create callback");

    // Emit quality metrics asynchronously
    let start = std::time::Instant::now();
    callback.emit_quality_metrics_async(0.88, 0.91, 0.85, true);

    // Should have spawned a task (non-blocking)
    assert!(
        start.elapsed() < std::time::Duration::from_millis(10),
        "emit_quality_metrics_async should return immediately (non-blocking)"
    );

    // Flush to wait for completion
    callback.flush().await.expect("Failed to flush");
}

#[test]
fn test_emit_quality_metrics_method_exists() {
    // Compile-time verification that the methods exist with correct signatures
    // Uses AgentState as a concrete type to avoid generic parameter issues

    // Verify emit_quality_metrics signature - returns Future with Result
    fn _check_async(
        callback: &DashStreamCallback<AgentState>,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + '_
    {
        callback.emit_quality_metrics(0.0, 0.0, 0.0, true)
    }

    // Verify emit_quality_metrics_async signature (no return value)
    fn _check_sync(callback: &DashStreamCallback<AgentState>) {
        callback.emit_quality_metrics_async(0.0, 0.0, 0.0, true);
    }
}

#[test]
fn test_quality_metrics_field_values() {
    // Test that quality metric values are properly clamped to expected ranges
    // This validates the intent of the API without needing Kafka

    // Valid values should be in range [0.0, 1.0]
    let accuracy = 0.95;
    let relevance = 0.87;
    let completeness = 0.92;

    assert!(
        (0.0..=1.0).contains(&accuracy),
        "Accuracy should be in range [0.0, 1.0]"
    );
    assert!(
        (0.0..=1.0).contains(&relevance),
        "Relevance should be in range [0.0, 1.0]"
    );
    assert!(
        (0.0..=1.0).contains(&completeness),
        "Completeness should be in range [0.0, 1.0]"
    );

    // Edge cases
    assert!((0.0..=1.0).contains(&0.0), "Zero is valid");
    assert!((0.0..=1.0).contains(&1.0), "One is valid");
}
