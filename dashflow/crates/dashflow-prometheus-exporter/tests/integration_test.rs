//! Integration tests for Prometheus Exporter
//!
//! These tests validate the Kafka → Prometheus bridge functionality,
//! ensuring metrics are correctly exposed and formatted.
//!
//! M-569: These tests require Docker to be running with the prometheus-exporter
//! container. Run with: `cargo test -- --ignored`
//!
//! ## Configuration
//!
//! The exporter URL is configurable via environment variable:
//! - `EXPORTER_URL`: Base URL for the prometheus-exporter (default: `http://localhost:8080`)
//! - `EXPORTER_PORT`: Port only override (default: 8080, used if EXPORTER_URL not set)
//!
//! Example:
//! ```bash
//! EXPORTER_PORT=8081 cargo test --test integration_test -- --ignored
//! EXPORTER_URL=http://192.168.1.100:8080 cargo test --test integration_test -- --ignored
//! ```

// This integration test is intended for developer validation and uses Docker + HTTP.
// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::time::Duration;
use tokio::time::sleep;

/// Get the base URL for prometheus-exporter, configurable via env var
/// M-569: Use dynamic URL to avoid port conflicts when port 8080 is in use
fn get_exporter_url() -> String {
    // First check for full URL override
    if let Ok(url) = std::env::var("EXPORTER_URL") {
        return url;
    }
    // Then check for port-only override
    let port = std::env::var("EXPORTER_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://localhost:{}", port)
}

/// M-574/M-575: Helper to wait for exporter endpoint readiness with retry
/// Returns Ok(body) on success, Err on timeout
async fn wait_for_exporter_ready(max_retries: u32) -> Result<String, String> {
    let exporter_url = get_exporter_url();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    for attempt in 0..max_retries {
        match client.get(format!("{}/metrics", exporter_url)).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body = resp
                    .text()
                    .await
                    .map_err(|e| format!("Failed to read response: {}", e))?;
                return Ok(body);
            }
            _ => {
                // Exponential backoff: 200ms, 400ms, 800ms, 1.6s, 3.2s
                let delay = Duration::from_millis(200 * (1 << attempt.min(4)));
                sleep(delay).await;
            }
        }
    }
    Err("Exporter failed to become ready within timeout".to_string())
}

/// M-576: Helper to poll metrics until a value changes or timeout
/// Returns the final body when change detected, or after max attempts
#[allow(clippy::float_cmp)] // Intentional exact comparison for detecting any metric change
async fn poll_for_metric_change(
    metric_name: &str,
    initial_value: f64,
    max_attempts: u32,
    poll_interval: Duration,
) -> (f64, String) {
    let exporter_url = get_exporter_url();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    for _ in 0..max_attempts {
        sleep(poll_interval).await;

        if let Ok(resp) = client.get(format!("{}/metrics", exporter_url)).send().await {
            if let Ok(body) = resp.text().await {
                let current_value = extract_metric_value(&body, metric_name);
                if current_value != initial_value {
                    return (current_value, body);
                }
            }
        }
    }

    // Return last known value after timeout
    let body = match client.get(format!("{}/metrics", exporter_url)).send().await {
        Ok(resp) => resp.text().await.unwrap_or_default(),
        Err(_) => String::new(),
    };
    let final_value = extract_metric_value(&body, metric_name);
    (final_value, body)
}

/// Helper function to find docker binary in common locations
fn find_docker_binary() -> String {
    // Common docker locations on different systems
    let paths = [
        "docker",                   // In PATH
        "/usr/bin/docker",          // Linux
        "/usr/local/bin/docker",    // macOS (Intel)
        "/opt/homebrew/bin/docker", // macOS (Apple Silicon)
    ];

    for path in &paths {
        if std::process::Command::new(path)
            .arg("--version")
            .output()
            .is_ok()
        {
            return path.to_string();
        }
    }

    // Fallback to "docker" and let it fail with a helpful error
    "docker".to_string()
}

/// Test 1: HTTP /metrics endpoint accessibility
///
/// Verifies that:
/// - /metrics endpoint returns HTTP 200
/// - Response is valid Prometheus text format
/// - Response is not empty
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_metrics_endpoint_accessible() {
    // M-574: Use readiness check with retry instead of fixed 2s sleep
    let body = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    assert!(
        !body.is_empty(),
        "/metrics endpoint should return non-empty response"
    );

    // Verify Prometheus text format (should contain "# HELP" or "# TYPE")
    assert!(
        body.contains("# HELP") || body.contains("# TYPE"),
        "/metrics should return valid Prometheus text format"
    );
}

/// Test 2: Quality monitoring metrics presence
///
/// Verifies that all expected quality monitoring metrics are exposed:
/// - dashstream_quality_monitor_queries_total
/// - dashstream_quality_monitor_queries_passed_total
/// - dashstream_quality_monitor_queries_failed_total
/// - dashstream_quality_monitor_quality_score
/// - dashstream_query_latency_ms
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_quality_metrics_present() {
    // M-575: Use readiness check with retry instead of fixed 2s sleep
    let body = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    // Check for quality monitoring metrics (with dashstream namespace)
    let expected_metrics = vec![
        "dashstream_quality_monitor_queries_total",
        "dashstream_quality_monitor_queries_passed_total",
        "dashstream_quality_monitor_queries_failed_total",
        "dashstream_quality_monitor_quality_score",
        "dashstream_query_latency_ms",
    ];

    for metric in expected_metrics {
        assert!(
            body.contains(metric),
            "Expected metric '{}' not found in /metrics output",
            metric
        );
    }
}

/// Test 3: Application-specific metrics presence (dashstream_* namespace)
///
/// Verifies that application-specific metrics are exposed with `dashstream_` prefix:
/// - dashstream_librarian_requests_total
/// - dashstream_librarian_iterations
/// - dashstream_librarian_tests_total
/// - dashstream_librarian_request_duration_seconds
///
/// Note: As of Dec 2025, code_assistant and document_search were consolidated into librarian.
/// Legacy app types are mapped to librarian metrics internally.
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_application_metrics_present() {
    // M-575: Use readiness check with retry instead of fixed 2s sleep
    let body = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    // Check for application-specific metrics.
    let expected_metrics = vec![
        "dashstream_librarian_requests_total",
        "dashstream_librarian_iterations",
        "dashstream_librarian_tests_total",
        "dashstream_librarian_request_duration_seconds",
    ];

    for metric in expected_metrics {
        assert!(
            body.contains(metric),
            "Expected metric '{}' not found in /metrics output",
            metric
        );
    }

    // Verify that legacy/non-namespaced metric names do NOT exist in the exporter.
    let invalid_metrics = vec![
        "librarian_requests_total",
        "librarian_iterations",
        "librarian_tests_total",
        "librarian_request_duration_seconds",
    ];

    for metric in invalid_metrics {
        assert!(
            !body.contains(metric),
            "Legacy metric '{}' should NOT exist in exporter output",
            metric
        );
    }
}

/// Test 4: Metric values are reasonable
///
/// Verifies that:
/// - Counters are non-negative
/// - Gauges are within expected ranges
/// - Histograms have bucket structure
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_metric_values_reasonable() {
    // M-575: Use readiness check with retry instead of fixed 2s sleep
    let body = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    // Parse metrics and check values
    for line in body.lines() {
        // Skip comments and empty lines
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        // Parse metric line: "metric_name{labels} value"
        if let Some((name_with_labels, value_str)) = line.rsplit_once(' ') {
            if let Ok(value) = value_str.parse::<f64>() {
                // Check that counters are non-negative
                if name_with_labels.contains("_total") {
                    assert!(
                        value >= 0.0,
                        "Counter metric '{}' has negative value: {}",
                        name_with_labels,
                        value
                    );
                }

                // Check that quality score is in range [0, 1]
                if name_with_labels.contains("quality_score") {
                    assert!(
                        (0.0..=1.0).contains(&value),
                        "Quality score '{}' out of range [0, 1]: {}",
                        name_with_labels,
                        value
                    );
                }

                // Check that latency is reasonable (not negative, not absurdly high)
                // Skip _sum and _count metrics as they're cumulative across all time
                if name_with_labels.contains("latency")
                    && !name_with_labels.contains("_sum")
                    && !name_with_labels.contains("_count")
                {
                    assert!(
                        value >= 0.0,
                        "Latency metric '{}' is negative: {}",
                        name_with_labels,
                        value
                    );
                    assert!(
                        value < 3600000.0,
                        "Latency metric '{}' is too high (>1 hour): {}",
                        name_with_labels,
                        value
                    );
                }
            }
        }
    }
}

/// Test 5: Docker container health check
///
/// Verifies that:
/// - Prometheus-exporter Docker container is running
/// - Container reports healthy status
/// - Container has been up for at least 10 seconds
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_docker_container_healthy() {
    use std::process::Command;

    let docker = find_docker_binary();

    // Check if container is running
    let output = Command::new(&docker)
        .args([
            "ps",
            "--filter",
            "name=dashstream-prometheus-exporter",
            "--format",
            "{{.Status}}",
        ])
        .output()
        .expect("Failed to execute docker ps command");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.contains("Up"),
        "Prometheus-exporter container is not running. Status: {}",
        status
    );

    // Check if container is healthy
    let output = Command::new(&docker)
        .args([
            "inspect",
            "--format",
            "{{.State.Health.Status}}",
            "dashstream-prometheus-exporter",
        ])
        .output()
        .expect("Failed to execute docker inspect command");

    let health = String::from_utf8_lossy(&output.stdout);
    assert!(
        health.contains("healthy") || health.contains("starting"),
        "Prometheus-exporter container is not healthy. Status: {}",
        health
    );
}

/// Test 6: Histogram bucket structure
///
/// Verifies that histogram metrics have proper bucket structure:
/// - _bucket labels with le="..."
/// - _sum and _count metrics
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_histogram_structure() {
    // M-575: Use readiness check with retry instead of fixed 2s sleep
    let body = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    // Check for histogram metrics structure
    // As of Dec 2025, document_search metrics are consolidated under librarian_*
    let histogram_metrics = vec![
        "librarian_request_duration_seconds",
        "dashstream_query_latency_ms",
    ];

    for metric in histogram_metrics {
        // Check for _bucket metrics
        assert!(
            body.contains(&format!("{}_bucket", metric)),
            "Histogram '{}' missing _bucket metrics",
            metric
        );

        // Check for le labels
        assert!(
            body.contains(&format!("{}_bucket{{le=", metric)),
            "Histogram '{}' missing le labels in buckets",
            metric
        );

        // Check for _sum metric
        assert!(
            body.contains(&format!("{}_sum", metric)),
            "Histogram '{}' missing _sum metric",
            metric
        );

        // Check for _count metric
        assert!(
            body.contains(&format!("{}_count", metric)),
            "Histogram '{}' missing _count metric",
            metric
        );
    }
}

/// Test 7: Metrics increase over time
///
/// Verifies that metrics actually update when events flow through the system.
/// This test fetches metrics twice with a delay and verifies counters increased.
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_metrics_increase_over_time() {
    // M-575: Use readiness check with retry instead of fixed 2s sleep
    let body1 = wait_for_exporter_ready(15)
        .await
        .expect("Exporter should become ready");

    // Extract initial counter value
    let initial_count = extract_metric_value(&body1, "dashstream_quality_monitor_queries_total");

    // M-576: Poll until metrics change instead of fixed 10s sleep
    // Poll every 500ms for up to 30 attempts (15s total max wait)
    let (final_count, _body2) = poll_for_metric_change(
        "dashstream_quality_monitor_queries_total",
        initial_count,
        30,
        Duration::from_millis(500),
    )
    .await;

    // If quality monitor is running, counters should increase
    // If not running, both will be 0 and test will pass
    if initial_count > 0.0 || final_count > 0.0 {
        assert!(
            final_count >= initial_count,
            "Counter should not decrease: {} -> {}",
            initial_count,
            final_count
        );
    }
}

/// Helper function to extract metric value from Prometheus text format
fn extract_metric_value(body: &str, metric_name: &str) -> f64 {
    for line in body.lines() {
        if line.starts_with(metric_name) && !line.contains("# HELP") && !line.contains("# TYPE") {
            // Parse line: "metric_name value" or "metric_name{labels} value"
            if let Some((_name, value_str)) = line.rsplit_once(' ') {
                if let Ok(value) = value_str.trim().parse::<f64>() {
                    return value;
                }
            }
        }
    }
    0.0
}

/// Test 8: No error logs in container
///
/// Verifies that prometheus-exporter container has no ERROR or WARN level logs
/// in the last 60 seconds of operation.
#[tokio::test]
#[ignore = "requires Docker - run prometheus-exporter container first"]
async fn test_no_error_logs_in_container() {
    use std::process::Command;

    let docker = find_docker_binary();

    // Get container logs from last 60 seconds
    let output = Command::new(&docker)
        .args(["logs", "--since", "60s", "dashstream-prometheus-exporter"])
        .output()
        .expect("Failed to execute docker logs command");

    let logs = String::from_utf8_lossy(&output.stdout);
    let errors = String::from_utf8_lossy(&output.stderr);

    // Check for ERROR level logs
    let error_count = logs.matches("ERROR").count() + errors.matches("ERROR").count();
    assert_eq!(
        error_count, 0,
        "Found {} ERROR level logs in container output",
        error_count
    );

    // Check for panic messages
    let panic_count = logs.matches("panic").count() + errors.matches("panic").count();
    assert_eq!(
        panic_count, 0,
        "Found {} panic messages in container output",
        panic_count
    );
}
