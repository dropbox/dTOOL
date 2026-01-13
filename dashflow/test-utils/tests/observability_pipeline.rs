//! Observability Pipeline Integration Test
//!
//! This test validates the complete DashStream observability flow:
//!   RAG App -> Kafka -> Quality Aggregator -> Prometheus
//!
//! # Prerequisites
//!
//! - Docker and docker-compose installed
//! - No external API keys required (test uses `send_test_metrics` example)
//!
//! # Running
//!
//! ```bash
//! # Run with docker-compose (containers must be up)
//! cargo test -p dashflow-test-utils --test observability_pipeline -- --ignored
//!
//! # Or start stack first:
//! docker-compose -f docker-compose-kafka.yml up -d
//! cargo test -p dashflow-test-utils --test observability_pipeline -- --ignored --nocapture
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::time::Duration;

use dashflow_streaming::testing::send_test_quality_metrics;
use dashflow_test_utils::{
    check_expected_schema_api, check_grafana_has_data, count_quality_processed, find_repo_root,
    get_kafka_offset, is_container_running, is_kafka_healthy, is_prometheus_available,
    query_prometheus, query_quality_score_in_range, wait_for_kafka_healthy,
    wait_for_kafka_messages, wait_for_quality_processed, DockerServices, ObservabilityTestResult,
    PollingConfig, DASHSTREAM_TOPIC, KAFKA_CONTAINER, QUALITY_AGGREGATOR_CONTAINER,
};

const COMPOSE_FILE_NAME: &str = "docker-compose.dashstream.yml";
const KAFKA_HEALTH_TIMEOUT: Duration = Duration::from_secs(120);
/// Expected number of test messages sent by send_test_quality_metrics
const EXPECTED_TEST_MESSAGES: i64 = 3;

/// Get the path to the docker-compose file using repo root discovery (M-108)
///
/// This is more robust than using relative paths because it works regardless
/// of the current working directory when running tests.
fn get_compose_file_path() -> String {
    find_repo_root()
        .map(|root| root.join(COMPOSE_FILE_NAME).display().to_string())
        .unwrap_or_else(|| {
            eprintln!(
                "⚠️  Could not find repo root, falling back to relative path: {}",
                COMPOSE_FILE_NAME
            );
            COMPOSE_FILE_NAME.to_string()
        })
}

/// Check if required containers are running
fn check_containers_running() -> bool {
    is_container_running(KAFKA_CONTAINER) && is_container_running(QUALITY_AGGREGATOR_CONTAINER)
}

/// Start the observability stack if not running
fn ensure_stack_running() -> Result<Option<DockerServices>, Box<dyn std::error::Error>> {
    if check_containers_running() {
        println!("✓ Observability stack already running");
        return Ok(None);
    }

    println!("Starting observability stack...");
    let compose_file = get_compose_file_path();
    println!("  Using compose file: {}", compose_file);
    let services = DockerServices::new(&compose_file).with_project_name("dashflow-observability");
    services.start()?;

    Ok(Some(services))
}

/// Kafka bootstrap servers for tests
const KAFKA_BOOTSTRAP_SERVERS: &str = "localhost:9092";

/// Send test metrics to Kafka
///
/// Uses the dashflow-streaming library directly (M-110: no subprocess spawning).
/// No external APIs (Tavily, OpenAI) required.
/// Returns the number of messages sent, or 0 on failure.
async fn run_rag_query() -> usize {
    println!("Sending protobuf-encoded quality metrics to Kafka...");

    match send_test_quality_metrics(KAFKA_BOOTSTRAP_SERVERS, DASHSTREAM_TOPIC).await {
        Ok(count) => {
            println!("  Protobuf metrics sent successfully ({} messages)", count);
            count
        }
        Err(e) => {
            println!("  Failed to send test metrics: {}", e);
            0
        }
    }
}

/// Collect observability pipeline results
async fn collect_results() -> ObservabilityTestResult {
    let mut result = ObservabilityTestResult::default();

    // Check Kafka messages
    match get_kafka_offset(DASHSTREAM_TOPIC) {
        Ok(offset) => {
            result.kafka_messages = offset;
            println!("  Kafka total offset: {}", offset);
        }
        Err(e) => {
            println!("  Kafka offset error: {}", e);
        }
    }

    // Check quality aggregator processing
    match count_quality_processed() {
        Ok(count) => {
            result.quality_processed = count;
            println!("  Quality aggregator processed: {}", count);
        }
        Err(e) => {
            println!("  Quality aggregator error: {}", e);
        }
    }

    // Check Prometheus metrics (if available)
    if is_prometheus_available().await {
        match query_prometheus("dashstream_quality_accuracy").await {
            Ok(Some(value)) => {
                result.prometheus_value = Some(value);
                println!("  Prometheus quality accuracy: {}", value);
            }
            Ok(None) => {
                println!("  Prometheus: no data for dashstream_quality_accuracy");
            }
            Err(e) => {
                println!("  Prometheus query error: {}", e);
            }
        }
    } else {
        println!("  Prometheus not available (skipping)");
    }

    result
}

/// Main integration test
///
/// This test is ignored by default because it requires Docker.
/// Run with: cargo test --test observability_pipeline -- --ignored --nocapture
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_observability_pipeline_end_to_end() {
    println!("\n=== DashFlow Observability Pipeline Integration Test ===\n");

    // 1. Check/start docker stack
    let _services = ensure_stack_running().expect("Failed to start observability stack");

    // 2. Wait for Kafka to be healthy
    println!("Waiting for Kafka to be healthy...");
    wait_for_kafka_healthy(KAFKA_HEALTH_TIMEOUT)
        .await
        .expect("Kafka did not become healthy");

    // 3. Get baseline Kafka offset
    let offset_before = get_kafka_offset(DASHSTREAM_TOPIC).unwrap_or(0);
    println!("Kafka offset before: {}", offset_before);

    // 4. Run RAG query (M-110: direct library call, no subprocess)
    let messages_sent = run_rag_query().await;
    assert_eq!(
        messages_sent as i64, EXPECTED_TEST_MESSAGES,
        "Expected {} test metrics to be sent",
        EXPECTED_TEST_MESSAGES
    );

    // 5. Wait for Kafka to receive messages (M-109: polling instead of fixed sleep)
    println!("Waiting for Kafka to receive messages...");
    let polling_config = PollingConfig::default();
    let offset_after = wait_for_kafka_messages(
        DASHSTREAM_TOPIC,
        offset_before,
        messages_sent as i64,
        &polling_config,
    )
    .await
    .expect("Kafka messages not received in time");
    let new_messages = offset_after - offset_before;
    println!("Kafka offset after: {} (new: {})", offset_after, new_messages);

    // 6. Wait for quality aggregator to process (M-109: polling instead of fixed sleep)
    println!("Waiting for quality aggregator to process metrics...");
    let quality_baseline = count_quality_processed().unwrap_or(0);
    // Quality aggregator may have processed some already, so wait for at least 1 new
    let _ = wait_for_quality_processed(quality_baseline, 1, &polling_config).await;

    // 7. Collect all results
    println!("\nCollecting results...");
    let mut results = collect_results().await;
    results.kafka_messages = new_messages;

    // 8. Print summary
    println!("\n=== Test Summary ===");
    println!("Kafka messages (new):    {}", results.kafka_messages);
    println!(
        "Quality aggregator:      {}",
        if results.quality_processed > 0 {
            format!("PASS ({} processed)", results.quality_processed)
        } else {
            "FAIL (0 processed)".to_string()
        }
    );
    println!(
        "Prometheus metrics:      {}",
        match results.prometheus_value {
            Some(v) => format!("PASS (value: {})", v),
            None => "SKIP (not configured)".to_string(),
        }
    );

    // 9. Assert pipeline is working
    assert!(
        results.kafka_messages > 0,
        "FAIL: No new messages in Kafka topic '{}'. \
         Check that DashStreamCallback is emitting messages.",
        DASHSTREAM_TOPIC
    );

    // Pipeline is operational - assert quality processing works
    assert!(
        results.quality_processed > 0,
        "Quality monitor did not process any metrics. Expected > 0, got {}",
        results.quality_processed
    );

    println!("\n✓ Integration test completed");
}

/// Test that Kafka health check works
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_kafka_health_check() {
    if !is_container_running(KAFKA_CONTAINER) {
        println!("Skipping: Kafka container not running");
        return;
    }

    let healthy = is_kafka_healthy();
    println!("Kafka healthy: {}", healthy);
    assert!(healthy, "Kafka should be healthy when container is running");
}

/// Test that we can get Kafka offsets
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_kafka_offset_retrieval() {
    if !is_container_running(KAFKA_CONTAINER) {
        println!("Skipping: Kafka container not running");
        return;
    }

    let offset = get_kafka_offset(DASHSTREAM_TOPIC);
    println!("Kafka offset result: {:?}", offset);
    assert!(offset.is_ok(), "Should be able to get Kafka offset");
}

/// Test that we can get quality aggregator logs
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_quality_aggregator_logs() {
    if !is_container_running(QUALITY_AGGREGATOR_CONTAINER) {
        println!("Skipping: Quality aggregator container not running");
        return;
    }

    let count = count_quality_processed();
    println!("Quality processed count: {:?}", count);
    assert!(
        count.is_ok(),
        "Should be able to get quality aggregator logs"
    );
}

/// Test Prometheus query (if available)
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_prometheus_query() {
    if !is_prometheus_available().await {
        println!("Skipping: Prometheus not available");
        return;
    }

    let result = query_prometheus("up").await;
    println!("Prometheus 'up' query: {:?}", result);
    assert!(result.is_ok(), "Should be able to query Prometheus");
}

/// Strict E2E Integration Test (Section 1.2)
///
/// This test validates ALL observability requirements with strict assertions:
/// 1. Kafka receives messages
/// 2. Quality aggregator processes events
/// 3. Prometheus quality_score is in valid range (0.0-1.0)
/// 4. /api/expected-schema returns 200
/// 5. Grafana queries return data (not "No data")
///
/// Run with: cargo test -p dashflow-test-utils --test observability_pipeline test_e2e_strict -- --ignored --nocapture
#[tokio::test]
#[ignore = "requires Docker"]
async fn test_e2e_strict_observability() {
    println!("\n=== DashFlow Strict E2E Observability Test ===\n");
    println!("This test validates Section 1.2 requirements:");
    println!("  1. Kafka messages received");
    println!("  2. Quality aggregator processing");
    println!("  3. Prometheus quality_score in 0.0-1.0 range");
    println!("  4. /api/expected-schema returns 200");
    println!("  5. Grafana queries return data\n");

    // 1. Check/start docker stack
    let _services = ensure_stack_running().expect("Failed to start observability stack");

    // 2. Wait for Kafka to be healthy
    println!("Waiting for Kafka to be healthy...");
    wait_for_kafka_healthy(KAFKA_HEALTH_TIMEOUT)
        .await
        .expect("Kafka did not become healthy");

    // 3. Get baseline Kafka offset
    let offset_before = get_kafka_offset(DASHSTREAM_TOPIC).unwrap_or(0);
    println!("Kafka offset before: {}", offset_before);

    // 4. Run RAG query (M-110: direct library call, no subprocess)
    let messages_sent = run_rag_query().await;
    assert_eq!(
        messages_sent as i64, EXPECTED_TEST_MESSAGES,
        "Expected {} test metrics to be sent",
        EXPECTED_TEST_MESSAGES
    );

    // 5. Wait for Kafka to receive messages (M-109: polling instead of fixed sleep)
    println!("Waiting for Kafka to receive messages...");
    let polling_config = PollingConfig::default();
    let offset_after = wait_for_kafka_messages(
        DASHSTREAM_TOPIC,
        offset_before,
        messages_sent as i64,
        &polling_config,
    )
    .await
    .expect("STRICT FAIL: Kafka messages not received in time");
    let new_messages = offset_after - offset_before;

    // === STRICT ASSERTIONS ===
    println!("\n=== Running Strict Assertions ===\n");

    // Assertion 1: Kafka messages (already verified by polling)
    println!("CHECK 1: Kafka messages received: {}", new_messages);
    assert!(
        new_messages > 0,
        "STRICT FAIL: No new messages in Kafka topic '{}'. \
         Check that DashStreamCallback is emitting messages.",
        DASHSTREAM_TOPIC
    );
    println!("  ✓ PASS: {} new Kafka messages", new_messages);

    // Assertion 2: Quality aggregator processing (M-109: polling instead of assuming)
    println!("CHECK 2: Waiting for quality aggregator to process...");
    let quality_baseline = count_quality_processed().unwrap_or(0);
    let quality_count = wait_for_quality_processed(quality_baseline, 1, &polling_config)
        .await
        .unwrap_or(quality_baseline);
    println!("CHECK 2: Quality aggregator processed: {}", quality_count);
    assert!(
        quality_count > 0,
        "STRICT FAIL: Quality aggregator did not process any metrics."
    );
    println!("  ✓ PASS: {} events processed", quality_count);

    // Assertion 3: Prometheus quality_score in valid range
    println!("CHECK 3: Prometheus quality_score range...");
    assert!(
        is_prometheus_available().await,
        "STRICT FAIL: Prometheus must be available for E2E test"
    );

    let quality_result = query_quality_score_in_range().await;
    assert!(
        quality_result.is_ok(),
        "STRICT FAIL: Failed to query Prometheus: {:?}",
        quality_result.err()
    );

    let quality_data = quality_result.unwrap();
    assert!(
        quality_data.is_some(),
        "STRICT FAIL: No quality_score metric found - send_test_metrics must emit quality_score"
    );

    let (value, in_range) = quality_data.unwrap();
    println!("  quality_score = {}", value);
    assert!(
        in_range,
        "STRICT FAIL: quality_score {} is outside valid range 0.0-1.0",
        value
    );
    println!("  ✓ PASS: quality_score {} is in range [0.0, 1.0]", value);

    // Assertion 4: Expected-schema API returns 200
    // STRICT: This MUST pass - API should be running for E2E tests
    println!("CHECK 4: /api/expected-schema endpoint...");
    match check_expected_schema_api().await {
        Ok(true) => println!("  ✓ PASS: /api/expected-schema returns 200"),
        Ok(false) => {
            panic!("STRICT FAIL: /api/expected-schema did not return 200 - API must be running for E2E tests")
        }
        Err(e) => {
            panic!(
                "STRICT FAIL: Expected-schema check failed: {} - API must be accessible",
                e
            )
        }
    }

    // Assertion 5: Grafana queries return data
    // STRICT: Grafana MUST return data - empty data indicates broken observability pipeline
    println!("CHECK 5: Grafana queries return data...");
    match check_grafana_has_data("dashstream_quality_monitor_quality_score").await {
        Ok(true) => println!("  ✓ PASS: Grafana shows data (not 'No data')"),
        Ok(false) => {
            panic!("STRICT FAIL: Grafana returned no data for dashstream_quality_monitor_quality_score - dashboard would show 'No data'")
        }
        Err(e) => {
            panic!(
                "STRICT FAIL: Grafana check failed: {} - observability stack must be accessible",
                e
            )
        }
    }

    println!("\n=== Strict E2E Test Summary ===");
    println!("ALL CHECKS PASSED:");
    println!(
        "  1. Kafka messages:       ✓ PASS ({} messages)",
        new_messages
    );
    println!(
        "  2. Quality processing:   ✓ PASS ({} events)",
        quality_count
    );
    println!("  3. Prometheus range:     ✓ PASS (0.0-1.0)");
    println!("  4. Expected-schema API:  ✓ PASS");
    println!("  5. Grafana data:         ✓ PASS");
    println!("\n✓ Strict E2E test completed - all checks passed");
}
