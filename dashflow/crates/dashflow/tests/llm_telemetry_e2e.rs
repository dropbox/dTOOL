// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

#![allow(clippy::unwrap_used, clippy::expect_used)]

//! E2E tests validating LLM telemetry flows to WAL
//!
//! This test validates TEL-004: LLM telemetry → WAL integration
//!
//! Prior to these tests, TEL-004 was marked "VALIDATED" but no test actually
//! verified the end-to-end flow from TelemetrySink to WAL file.

use dashflow::telemetry::{TelemetryEvent, TelemetrySink};
use dashflow::wal::{WALTelemetrySink, WALWriter, WALWriterConfig};
use serde_json::Value;
use std::sync::Mutex;
use tempfile::TempDir;

/// Mutex to prevent parallel tests from interfering with each other's env vars
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Helper to read all events from WAL files in a directory
fn read_wal_events(wal_dir: &std::path::Path) -> Vec<Value> {
    let mut events = Vec::new();

    for entry in std::fs::read_dir(wal_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "wal") {
            let content = std::fs::read_to_string(&path).unwrap();
            for line in content.lines() {
                if !line.trim().is_empty() {
                    if let Ok(event) = serde_json::from_str::<Value>(line) {
                        events.push(event);
                    }
                }
            }
        }
    }

    events
}

// ============================================================================
// TEL-004: LLM Call Events → WAL
// ============================================================================

/// M-2003: Verify LlmCallCompleted events flow from TelemetrySink to WAL file
///
/// This test validates the COMPLETE pipeline:
/// 1. Create WALTelemetrySink with temp directory
/// 2. Record an LlmCallCompleted event
/// 3. Read the WAL file back
/// 4. Verify the event is present with correct fields
#[test]
fn test_llm_call_completed_flows_to_wal_m2003() {
    let _guard = ENV_MUTEX.lock().unwrap();

    // Setup: Create temp WAL directory
    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().to_path_buf();

    // Create WAL writer with test config
    let config = WALWriterConfig {
        wal_dir: wal_dir.clone(),
        max_segment_bytes: 10 * 1024 * 1024,
        fsync_on_write: true, // Ensure durability
        segment_extension: ".wal".to_string(),
    };
    let writer = WALWriter::new(config).expect("WAL writer should initialize");

    // Create WALTelemetrySink
    let sink = WALTelemetrySink::new(writer);

    // Record an LlmCallCompleted event
    sink.record_event(TelemetryEvent::LlmCallCompleted {
        model: "test-model-123".to_string(),
        provider: "test-provider".to_string(),
        messages: Some(r#"[{"role":"user","content":"test prompt"}]"#.to_string()),
        response: Some("Test response from LLM".to_string()),
        error: None,
        duration_ms: 250,
        input_tokens: Some(15),
        output_tokens: Some(25),
    });

    // Flush sink
    sink.flush();

    // Read back WAL events
    let events = read_wal_events(&wal_dir);

    // Verify: At least one event was written
    assert!(
        !events.is_empty(),
        "WAL should contain at least one event after LlmCallCompleted"
    );

    // Find the LlmCallCompleted event
    let llm_event = events.iter().find(|e| {
        e.get("event_type")
            .and_then(|v| v.as_str())
            .map_or(false, |t| t == "llm_call_completed")
    });

    assert!(
        llm_event.is_some(),
        "WAL should contain an event with event_type=llm_call_completed. Found events: {:?}",
        events.iter().map(|e| e.get("event_type")).collect::<Vec<_>>()
    );

    let event = llm_event.unwrap();

    // Verify all fields are present in the payload
    let payload = event.get("payload").expect("Event should have payload");

    assert_eq!(
        payload.get("model").and_then(|v| v.as_str()),
        Some("test-model-123"),
        "Model should match"
    );
    assert_eq!(
        payload.get("provider").and_then(|v| v.as_str()),
        Some("test-provider"),
        "Provider should match"
    );
    assert_eq!(
        payload.get("duration_ms").and_then(|v| v.as_u64()),
        Some(250),
        "Duration should match"
    );
    assert_eq!(
        payload.get("input_tokens").and_then(|v| v.as_u64()),
        Some(15),
        "Input tokens should match"
    );
    assert_eq!(
        payload.get("output_tokens").and_then(|v| v.as_u64()),
        Some(25),
        "Output tokens should match"
    );
    assert!(
        payload
            .get("response")
            .and_then(|v| v.as_str())
            .map_or(false, |r| r.contains("Test response")),
        "Response should be present"
    );
}

/// M-2003: Verify multiple LLM calls are all persisted
#[test]
fn test_multiple_llm_calls_persisted_m2003() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().to_path_buf();

    let config = WALWriterConfig {
        wal_dir: wal_dir.clone(),
        max_segment_bytes: 10 * 1024 * 1024,
        fsync_on_write: true,
        segment_extension: ".wal".to_string(),
    };
    let writer = WALWriter::new(config).unwrap();
    let sink = WALTelemetrySink::new(writer);

    // Record multiple LLM calls
    for i in 0..5 {
        sink.record_event(TelemetryEvent::LlmCallCompleted {
            model: format!("model-{}", i),
            provider: "batch-provider".to_string(),
            messages: None,
            response: Some(format!("Response {}", i)),
            error: None,
            duration_ms: 100 * (i + 1) as u64,
            input_tokens: Some(10 + i as u32),
            output_tokens: Some(20 + i as u32),
        });
    }

    sink.flush();

    // Read back and verify count
    let events = read_wal_events(&wal_dir);
    let llm_events: Vec<_> = events
        .iter()
        .filter(|e| {
            e.get("event_type")
                .and_then(|v| v.as_str())
                .map_or(false, |t| t == "llm_call_completed")
        })
        .collect();

    assert_eq!(
        llm_events.len(),
        5,
        "All 5 LLM calls should be persisted to WAL"
    );

    // Verify each model is present
    for i in 0..5 {
        let expected_model = format!("model-{}", i);
        let found = llm_events.iter().any(|e| {
            e.get("payload")
                .and_then(|p| p.get("model"))
                .and_then(|m| m.as_str())
                .map_or(false, |m| m == expected_model)
        });
        assert!(found, "Model {} should be in WAL", expected_model);
    }
}

/// M-2003: Verify LLM error events are persisted with error field
#[test]
fn test_llm_call_error_persisted_m2003() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().to_path_buf();

    let config = WALWriterConfig {
        wal_dir: wal_dir.clone(),
        max_segment_bytes: 10 * 1024 * 1024,
        fsync_on_write: true,
        segment_extension: ".wal".to_string(),
    };
    let writer = WALWriter::new(config).unwrap();
    let sink = WALTelemetrySink::new(writer);

    // Record an error event
    sink.record_event(TelemetryEvent::LlmCallCompleted {
        model: "error-model".to_string(),
        provider: "error-provider".to_string(),
        messages: Some(r#"[{"role":"user","content":"bad prompt"}]"#.to_string()),
        response: None,
        error: Some("RateLimitError: Too many requests".to_string()),
        duration_ms: 50,
        input_tokens: Some(5),
        output_tokens: None,
    });

    sink.flush();

    let events = read_wal_events(&wal_dir);
    let error_event = events.iter().find(|e| {
        e.get("event_type")
            .and_then(|v| v.as_str())
            .map_or(false, |t| t == "llm_call_completed")
    });

    assert!(error_event.is_some(), "Error event should be persisted");

    let event = error_event.unwrap();
    let payload = event.get("payload").unwrap();

    assert_eq!(
        payload.get("model").and_then(|v| v.as_str()),
        Some("error-model")
    );
    assert!(
        payload
            .get("error")
            .and_then(|v| v.as_str())
            .map_or(false, |e| e.contains("RateLimitError")),
        "Error field should contain error message"
    );
    assert!(
        payload.get("response").is_none() || payload.get("response").unwrap().is_null(),
        "Response should be null/absent on error"
    );
}

// ============================================================================
// WAL File Format Verification
// ============================================================================

/// Verify WAL events have correct structure and timestamp
#[test]
fn test_wal_event_structure() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let wal_dir = temp_dir.path().to_path_buf();

    let config = WALWriterConfig {
        wal_dir: wal_dir.clone(),
        max_segment_bytes: 10 * 1024 * 1024,
        fsync_on_write: true,
        segment_extension: ".wal".to_string(),
    };
    let writer = WALWriter::new(config).unwrap();
    let sink = WALTelemetrySink::new(writer);

    let before_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    sink.record_event(TelemetryEvent::LlmCallCompleted {
        model: "timestamp-test".to_string(),
        provider: "test".to_string(),
        messages: None,
        response: None,
        error: None,
        duration_ms: 1,
        input_tokens: None,
        output_tokens: None,
    });

    let after_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 1000; // Add 1s buffer

    sink.flush();

    let events = read_wal_events(&wal_dir);
    assert!(!events.is_empty());

    let event = &events[0];

    // Verify required fields
    assert!(event.get("timestamp_ms").is_some(), "Should have timestamp_ms");
    assert!(event.get("event_type").is_some(), "Should have event_type");
    assert!(event.get("payload").is_some(), "Should have payload");

    // Verify timestamp is reasonable
    let ts = event.get("timestamp_ms").unwrap().as_u64().unwrap();
    assert!(
        ts >= before_ms && ts <= after_ms,
        "Timestamp should be between {} and {}, got {}",
        before_ms,
        after_ms,
        ts
    );
}
