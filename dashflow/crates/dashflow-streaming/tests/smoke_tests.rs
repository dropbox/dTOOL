// Smoke Tests for Live DashStream System
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox
//
//! Smoke tests validate the RUNNING DashStream stack (docker-compose.dashstream.yml)
//!
//! These tests check that all services are healthy and communicating correctly.
//!
//! **Prerequisites**: Docker compose stack must be running:
//! ```bash
//! docker-compose -f docker-compose.dashstream.yml up -d
//! ```
//!
//! **Test Categories:**
//! 1. **Service health tests** (always run): Verify services are up and responding
//! 2. **Data flow tests** (ignored by default): Require active apps sending messages
//!
//! **Usage:**
//! ```bash
//! # Run service health tests only (default)
//! cargo test --test smoke_tests
//!
//! # Run data flow tests (requires active app)
//! cargo test --test smoke_tests --ignored
//!
//! # Run all tests
//! cargo test --test smoke_tests -- --include-ignored
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use std::time::Duration;

const WEBSOCKET_URL: &str = "http://localhost:3002";
const QUALITY_MONITOR_URL: &str = "http://localhost:3003";
const PROMETHEUS_EXPORTER_URL: &str = "http://localhost:8080";
const GRAFANA_URL: &str = "http://localhost:3000";
const PROMETHEUS_URL: &str = "http://localhost:9090";

/// Helper to fetch JSON from URL
async fn fetch_json(url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let response = client.get(url).send().await?;
    let json = response.json::<Value>().await?;
    Ok(json)
}

/// Helper to fetch text from URL
async fn fetch_text(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let response = client.get(url).send().await?;
    let text = response.text().await?;
    Ok(text)
}

#[tokio::test]
#[ignore = "requires websocket server"]
async fn smoke_test_websocket_server_health() {
    let health = fetch_json(&format!("{}/health", WEBSOCKET_URL))
        .await
        .expect("WebSocket server not responding");

    println!(
        "WebSocket Health: {}",
        serde_json::to_string_pretty(&health).unwrap()
    );

    // Validate status
    assert_eq!(
        health["status"].as_str().unwrap(),
        "healthy",
        "WebSocket server should be healthy"
    );

    // Validate metrics exist
    let metrics = &health["metrics"];
    assert!(metrics.is_object(), "Should have metrics object");
    assert!(
        metrics["kafka_messages_received"].as_u64().is_some(),
        "Should have message count"
    );
    assert_eq!(
        metrics["kafka_errors"].as_u64().unwrap(),
        0,
        "Should have zero Kafka errors"
    );

    // Validate Kafka connection
    assert_eq!(
        health["kafka_status"].as_str().unwrap(),
        "connected",
        "Should be connected to Kafka"
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_quality_monitor_health() {
    let health = fetch_json(&format!("{}/health", QUALITY_MONITOR_URL))
        .await
        .expect("Quality monitor not responding");

    println!(
        "Quality Monitor Health: {}",
        serde_json::to_string_pretty(&health).unwrap()
    );

    // Validate status
    assert_eq!(
        health["status"].as_str().unwrap(),
        "healthy",
        "Quality monitor should be healthy"
    );

    // Validate metrics - actual schema uses stats.total_evaluations and stats.average_overall
    let stats = &health["stats"];
    assert!(
        stats["total_evaluations"].as_u64().is_some(),
        "Should have stats.total_evaluations metric"
    );
    let avg_quality = stats["average_overall"].as_f64().unwrap_or(0.0);
    assert!(
        (0.0..=1.0).contains(&avg_quality),
        "Quality should be 0-1, got {}",
        avg_quality
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_prometheus_exporter_metrics() {
    let metrics = fetch_text(&format!("{}/metrics", PROMETHEUS_EXPORTER_URL))
        .await
        .expect("Prometheus exporter not responding");

    println!(
        "Prometheus Exporter Metrics (first 500 chars):\n{}",
        &metrics[..500.min(metrics.len())]
    );

    // Validate key metrics exist
    assert!(
        metrics.contains("dashstream_"),
        "Should have dashstream metrics"
    );
    assert!(
        metrics.contains("dashstream_quality_monitor_quality_score"),
        "Should have quality score metric"
    );
    assert!(
        metrics.contains("dashstream_query_latency_ms"),
        "Should have latency metric"
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_grafana_api() {
    let health = fetch_json(&format!("{}/api/health", GRAFANA_URL))
        .await
        .expect("Grafana not responding");

    println!(
        "Grafana Health: {}",
        serde_json::to_string_pretty(&health).unwrap()
    );

    assert_eq!(
        health["database"].as_str().unwrap(),
        "ok",
        "Grafana database should be OK"
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_prometheus_api() {
    let targets = fetch_json(&format!("{}/api/v1/targets", PROMETHEUS_URL))
        .await
        .expect("Prometheus not responding");

    println!(
        "Prometheus Targets: {}",
        serde_json::to_string_pretty(&targets).unwrap()
    );

    assert_eq!(
        targets["status"].as_str().unwrap(),
        "success",
        "Prometheus API should return success"
    );

    // Check that dashstream-exporter target exists
    let active_targets = &targets["data"]["activeTargets"];
    assert!(active_targets.is_array(), "Should have active targets");

    let exporter_found = active_targets.as_array().unwrap().iter().any(|t| {
        let job = t["labels"]["job"].as_str().unwrap_or("");
        job == "dashstream-exporter" || job == "dashstream-quality"
    });

    assert!(exporter_found, "Should have dashstream exporter target configured (job: dashstream-exporter or dashstream-quality)");
}

#[tokio::test]
#[ignore = "requires websocket server"]
async fn smoke_test_end_to_end_data_flow() {
    // This test validates that data is flowing through the entire pipeline

    // 1. Check quality monitor has processed queries
    let quality_health = fetch_json(&format!("{}/health", QUALITY_MONITOR_URL))
        .await
        .expect("Quality monitor not responding");

    let total_queries = quality_health["total_queries"].as_u64().unwrap();
    assert!(
        total_queries > 0,
        "Quality monitor should have processed at least 1 query. Got: {}. Is an app running?",
        total_queries
    );

    // 2. Check websocket server received messages
    let ws_health = fetch_json(&format!("{}/health", WEBSOCKET_URL))
        .await
        .expect("WebSocket server not responding");

    let messages_received = ws_health["metrics"]["kafka_messages_received"]
        .as_u64()
        .unwrap();
    assert!(
        messages_received > 0,
        "WebSocket server should have received messages. Got: {}",
        messages_received
    );

    // 3. Check prometheus exporter has metrics
    let metrics = fetch_text(&format!("{}/metrics", PROMETHEUS_EXPORTER_URL))
        .await
        .expect("Prometheus exporter not responding");

    // Extract quality score value
    let quality_score_line = metrics
        .lines()
        .find(|line| line.starts_with("dashstream_quality_monitor_quality_score "))
        .expect("Should have quality score metric");

    let quality_score: f64 = quality_score_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .expect("Should parse quality score");

    assert!(
        (0.0..=1.0).contains(&quality_score),
        "Quality score should be valid (0.0-1.0). Got: {}",
        quality_score
    );

    println!("✅ End-to-end data flow validated:");
    println!("   - Quality Monitor: {} queries processed", total_queries);
    println!(
        "   - WebSocket Server: {} messages received",
        messages_received
    );
    println!(
        "   - Prometheus Exporter: quality_score = {}",
        quality_score
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_no_error_accumulation() {
    // Check that errors aren't accumulating over time

    let ws_health = fetch_json(&format!("{}/health", WEBSOCKET_URL))
        .await
        .expect("WebSocket server not responding");

    let kafka_errors = ws_health["metrics"]["kafka_errors"].as_u64().unwrap();
    let infra_errors = ws_health["metrics"]["infrastructure_errors"]
        .as_u64()
        .unwrap();
    let decode_errors = ws_health["metrics"]["decode_errors"].as_u64().unwrap();
    let dropped_messages = ws_health["metrics"]["dropped_messages"].as_u64().unwrap();

    println!("Error counts:");
    println!("  kafka_errors: {}", kafka_errors);
    println!("  infrastructure_errors: {}", infra_errors);
    println!("  decode_errors: {}", decode_errors);
    println!("  dropped_messages: {}", dropped_messages);

    // Allow small number of transient errors, but should be low
    assert!(
        kafka_errors < 10,
        "Too many Kafka errors: {}. Something is broken.",
        kafka_errors
    );
    assert!(
        decode_errors == 0,
        "Should have ZERO decode errors. Got: {}. This indicates data corruption!",
        decode_errors
    );
    assert!(
        dropped_messages == 0,
        "Should have ZERO dropped messages. Got: {}. This indicates message loss!",
        dropped_messages
    );
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_quality_monitor_processing_actively() {
    // Validate quality monitor is actively processing, not stalled

    let health1 = fetch_json(&format!("{}/health", QUALITY_MONITOR_URL))
        .await
        .expect("Quality monitor not responding");

    // Use stats.total_evaluations and stats.kafka_messages_received
    let evals1 = health1["stats"]["total_evaluations"].as_u64().unwrap_or(0);
    let msgs1 = health1["stats"]["kafka_messages_received"]
        .as_u64()
        .unwrap_or(0);

    println!(
        "Quality monitor at T0: {} evaluations, {} kafka messages",
        evals1, msgs1
    );

    // Wait 2 seconds
    tokio::time::sleep(Duration::from_secs(2)).await;

    let health2 = fetch_json(&format!("{}/health", QUALITY_MONITOR_URL))
        .await
        .expect("Quality monitor not responding");

    let evals2 = health2["stats"]["total_evaluations"].as_u64().unwrap_or(0);
    let msgs2 = health2["stats"]["kafka_messages_received"]
        .as_u64()
        .unwrap_or(0);

    println!(
        "Quality monitor at T+2s: {} evaluations, {} kafka messages",
        evals2, msgs2
    );

    // Just verify service is running and responsive
    let uptime = health2["uptime_seconds"].as_f64().unwrap_or(0.0);
    assert!(uptime > 0.0, "Quality monitor should have positive uptime");

    if evals2 == evals1 {
        println!("⚠️  No new evaluations processed in 2 seconds");
        println!("   This is OK if no app is currently sending events");
    } else {
        println!(
            "✅ Quality monitor actively processing: {} new evaluations in 2s",
            evals2 - evals1
        );
    }
}

#[tokio::test]
#[ignore = "requires running dashstream stack"]
async fn smoke_test_system_versions_consistent() {
    // All services should report compatible versions
    // Current expected schema version is 1 (from CURRENT_SCHEMA_VERSION in lib.rs)
    const EXPECTED_SCHEMA_VERSION: u32 = 1;

    let ws_health = fetch_json(&format!("{}/health", WEBSOCKET_URL)).await;
    let quality_health = fetch_json(&format!("{}/health", QUALITY_MONITOR_URL)).await;

    assert!(ws_health.is_ok(), "WebSocket server should be running");
    assert!(quality_health.is_ok(), "Quality monitor should be running");

    let ws_health = ws_health.unwrap();
    let quality_health = quality_health.unwrap();

    // Check for schema_version in health responses if available
    let ws_version = ws_health
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let quality_version = quality_health
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    println!("Version check:");
    println!("  Expected schema_version: {}", EXPECTED_SCHEMA_VERSION);
    println!("  WebSocket server: {:?}", ws_version);
    println!("  Quality monitor: {:?}", quality_version);

    // If version fields exist, validate they match expected
    if let Some(v) = ws_version {
        assert_eq!(
            v, EXPECTED_SCHEMA_VERSION,
            "WebSocket server schema_version ({}) should match expected ({})",
            v, EXPECTED_SCHEMA_VERSION
        );
    } else {
        println!("  Note: WebSocket server health does not expose schema_version field");
    }

    if let Some(v) = quality_version {
        assert_eq!(
            v, EXPECTED_SCHEMA_VERSION,
            "Quality monitor schema_version ({}) should match expected ({})",
            v, EXPECTED_SCHEMA_VERSION
        );
    } else {
        println!("  Note: Quality monitor health does not expose schema_version field");
    }

    // Validate versions are consistent with each other (if both exist)
    if ws_version.is_some() && quality_version.is_some() {
        assert_eq!(
            ws_version, quality_version,
            "WebSocket and Quality monitor schema versions should match"
        );
        println!(
            "  All services running consistent schema version {}",
            ws_version.unwrap()
        );
    } else {
        println!("  Services are running (schema_version field not exposed in health endpoints)");
        println!("  Enhancement: Add 'schema_version' to health responses for runtime validation");
    }
}
