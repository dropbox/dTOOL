// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Observability pipeline test utilities
//!
//! Helpers for testing the DashStream observability pipeline:
//! - Kafka message counting
//! - Quality aggregator log parsing
//! - Prometheus metric queries

use std::process::{Command, Stdio};
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::{Result, TestError};

/// Kafka topic configuration for observability testing
pub const DASHSTREAM_TOPIC: &str = "dashstream-quality";
pub const KAFKA_CONTAINER: &str = "dashstream-kafka";
pub const QUALITY_AGGREGATOR_CONTAINER: &str = "dashstream-quality-monitor";

/// Get the current Kafka offset for a topic
///
/// Returns the sum of all partition offsets (total message count)
pub fn get_kafka_offset(topic: &str) -> Result<i64> {
    let output = Command::new("docker")
        .args([
            "exec",
            KAFKA_CONTAINER,
            "kafka-run-class",
            "kafka.tools.GetOffsetShell",
            "--broker-list",
            "localhost:9092",
            "--topic",
            topic,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Topic may not exist yet - return 0
        if stderr.contains("UnknownTopicOrPartition") {
            return Ok(0);
        }
        return Err(TestError::DockerError(format!(
            "Failed to get Kafka offset: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse output like: "topic:0:42" - sum all partition offsets
    let total: i64 = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                parts[2].parse::<i64>().ok()
            } else {
                None
            }
        })
        .sum();

    Ok(total)
}

/// Check if Kafka container is healthy
pub fn is_kafka_healthy() -> bool {
    Command::new("docker")
        .args([
            "exec",
            KAFKA_CONTAINER,
            "kafka-broker-api-versions",
            "--bootstrap-server",
            "localhost:9092",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Wait for Kafka to become healthy
pub async fn wait_for_kafka_healthy(timeout: Duration) -> Result<()> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if is_kafka_healthy() {
            tracing::info!("Kafka is healthy");
            return Ok(());
        }
        tracing::debug!(
            "Waiting for Kafka... ({:.1}s elapsed)",
            start.elapsed().as_secs_f64()
        );
        sleep(Duration::from_secs(5)).await;
    }

    Err(TestError::ServiceUnhealthy(format!(
        "Kafka not healthy after {:?}",
        timeout
    )))
}

/// Get logs from a docker container
pub fn get_container_logs(container: &str, lines: Option<usize>) -> Result<String> {
    let mut cmd = Command::new("docker");
    cmd.arg("logs");

    if let Some(n) = lines {
        cmd.args(["--tail", &n.to_string()]);
    }

    cmd.arg(container);

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    // Docker logs go to stderr for some containers
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    Ok(format!("{stdout}{stderr}"))
}

/// Count occurrences of "Quality:" in quality aggregator logs
///
/// Returns the number of quality metrics that were successfully processed
pub fn count_quality_processed() -> Result<usize> {
    let logs = get_container_logs(QUALITY_AGGREGATOR_CONTAINER, Some(1000))?;
    Ok(logs.matches("Quality:").count())
}

/// Check if quality aggregator is processing metrics
///
/// Looks for "Quality:" in logs which indicates successful metric processing
pub fn is_quality_aggregator_processing() -> Result<bool> {
    Ok(count_quality_processed()? > 0)
}

/// Prometheus query response structures
#[derive(Debug, Deserialize)]
pub struct PrometheusResponse {
    pub status: String,
    pub data: PrometheusData,
}

#[derive(Debug, Deserialize)]
pub struct PrometheusData {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
pub struct PrometheusResult {
    pub metric: serde_json::Value,
    pub value: (f64, String),
}

/// Query Prometheus for a metric value
pub async fn query_prometheus(query: &str) -> Result<Option<f64>> {
    let prometheus_url =
        std::env::var("PROMETHEUS_URL").unwrap_or_else(|_| "http://localhost:9090".to_string());

    let client = Client::new();
    let response = client
        .get(format!("{prometheus_url}/api/v1/query"))
        .query(&[("query", query)])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(TestError::ServiceUnhealthy(format!(
            "Prometheus returned status {}",
            response.status()
        )));
    }

    let data: PrometheusResponse = response.json().await?;

    if data.status != "success" {
        return Ok(None);
    }

    // Get first result value
    if let Some(result) = data.data.result.first() {
        if let Ok(value) = result.value.1.parse::<f64>() {
            return Ok(Some(value));
        }
    }

    Ok(None)
}

/// Check if Prometheus is available
pub async fn is_prometheus_available() -> bool {
    let prometheus_url =
        std::env::var("PROMETHEUS_URL").unwrap_or_else(|_| "http://localhost:9090".to_string());

    let client = Client::new();
    client
        .get(format!("{prometheus_url}/-/healthy"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Check if a docker container is running
pub fn is_container_running(name: &str) -> bool {
    Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .stdout(Stdio::piped())
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().any(|line| line.contains(name))
        })
        .unwrap_or(false)
}

/// Check if expected-schema API is accessible (returns 200)
/// Note: Expected-schema API is on the websocket server (default port 3002)
///
/// LIMITATION: This only checks HTTP 200 status, not schema content validation.
/// For full validation, use `verify_expected_schema_content()` which checks:
/// - Schema is returned (not empty)
/// - Schema can be parsed as valid JSON
/// - Schema contains expected graph_name field
pub async fn check_expected_schema_api() -> Result<bool> {
    let api_url =
        std::env::var("DASHFLOW_API_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let response = client
        .get(format!("{api_url}/api/expected-schema"))
        .send()
        .await;

    match response {
        Ok(r) => Ok(r.status().is_success()),
        Err(e) => {
            tracing::warn!("Expected-schema API check failed: {}", e);
            Ok(false)
        }
    }
}

/// Expected schema entry from the API (M-261)
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedSchemaEntry {
    /// Content-addressed schema ID (hash)
    pub schema_id: String,
    /// Graph name this schema is for
    pub graph_name: String,
    /// Environment (e.g., "production", "staging", "development")
    pub environment: Option<String>,
    /// When this expectation was set (Unix timestamp ms)
    pub pinned_at: i64,
    /// Who set this expectation (optional, for audit)
    pub pinned_by: Option<String>,
    /// Note/description for this schema pin
    pub note: Option<String>,
}

/// Request body for setting expected schema (M-261)
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetExpectedSchemaRequest {
    /// Content-addressed schema ID (hash)
    pub schema_id: String,
    /// Environment (e.g., "production", "staging", "development")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Who set this expectation (optional, for audit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_by: Option<String>,
    /// Note/description for this schema pin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Result of expected schema content verification (M-261)
#[derive(Debug)]
pub struct ExpectedSchemaVerification {
    /// Whether the API returned valid JSON
    pub is_valid_json: bool,
    /// Whether the response is an array (list endpoint returns array)
    pub is_array: bool,
    /// Number of schemas in the response
    pub schema_count: usize,
    /// List of validation errors for individual schemas
    pub validation_errors: Vec<String>,
    /// The parsed schemas (empty if parsing failed)
    pub schemas: Vec<ExpectedSchemaEntry>,
}

impl ExpectedSchemaVerification {
    /// Check if the verification passed (valid JSON array with all schemas valid)
    pub fn is_valid(&self) -> bool {
        self.is_valid_json && self.is_array && self.validation_errors.is_empty()
    }
}

/// Verify expected-schema API returns valid, well-formed content (M-261: OBS-19)
///
/// This function strengthens the E2E check beyond just "200 OK" by verifying:
/// - Response is valid JSON
/// - Response is an array (list endpoint)
/// - Each schema entry has required fields (schema_id, graph_name, pinned_at)
/// - pinned_at is a valid positive timestamp
///
/// Note: Expected-schema API is on the websocket server (default port 3002)
pub async fn verify_expected_schema_content() -> Result<ExpectedSchemaVerification> {
    let api_url =
        std::env::var("DASHFLOW_API_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let response = client
        .get(format!("{api_url}/api/expected-schema"))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(TestError::ServiceUnhealthy(format!(
            "Expected-schema API returned status {}",
            response.status()
        )));
    }

    let body = response.text().await?;

    // Check if response is valid JSON
    let json_value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Expected-schema API returned invalid JSON: {}", e);
            return Ok(ExpectedSchemaVerification {
                is_valid_json: false,
                is_array: false,
                schema_count: 0,
                validation_errors: vec![format!("Invalid JSON: {e}")],
                schemas: vec![],
            });
        }
    };

    // Check if response is an array
    let array = match json_value.as_array() {
        Some(arr) => arr,
        None => {
            return Ok(ExpectedSchemaVerification {
                is_valid_json: true,
                is_array: false,
                schema_count: 0,
                validation_errors: vec!["Response is not a JSON array".to_string()],
                schemas: vec![],
            });
        }
    };

    // Parse and validate each schema entry
    let mut schemas = Vec::new();
    let mut validation_errors = Vec::new();

    for (i, item) in array.iter().enumerate() {
        // Try to parse as ExpectedSchemaEntry
        match serde_json::from_value::<ExpectedSchemaEntry>(item.clone()) {
            Ok(entry) => {
                // Additional validation: pinned_at should be a positive timestamp
                if entry.pinned_at <= 0 {
                    validation_errors.push(format!(
                        "Schema {i}: pinned_at ({}) is not a valid positive timestamp",
                        entry.pinned_at
                    ));
                }
                // schema_id should not be empty
                if entry.schema_id.is_empty() {
                    validation_errors.push(format!("Schema {i}: schema_id is empty"));
                }
                // graph_name should not be empty
                if entry.graph_name.is_empty() {
                    validation_errors.push(format!("Schema {i}: graph_name is empty"));
                }
                schemas.push(entry);
            }
            Err(e) => {
                validation_errors.push(format!("Schema {i}: failed to parse: {e}"));
            }
        }
    }

    Ok(ExpectedSchemaVerification {
        is_valid_json: true,
        is_array: true,
        schema_count: array.len(),
        validation_errors,
        schemas,
    })
}

/// Result of schema roundtrip verification (M-261)
#[derive(Debug)]
pub struct SchemaRoundtripResult {
    /// Whether the PUT succeeded
    pub put_succeeded: bool,
    /// Whether the GET succeeded
    pub get_succeeded: bool,
    /// Whether the DELETE succeeded
    pub delete_succeeded: bool,
    /// Whether the retrieved schema matched what was sent
    pub schema_matched: bool,
    /// Error message if any step failed
    pub error: Option<String>,
    /// The retrieved schema (if GET succeeded)
    pub retrieved_schema: Option<ExpectedSchemaEntry>,
}

impl SchemaRoundtripResult {
    /// Check if the full roundtrip passed
    pub fn is_success(&self) -> bool {
        self.put_succeeded && self.get_succeeded && self.delete_succeeded && self.schema_matched
    }
}

/// Verify expected-schema API roundtrip: PUT, GET, validate content, DELETE (M-261: OBS-19)
///
/// This function performs a complete roundtrip test:
/// 1. PUT a test schema
/// 2. GET it back and verify content matches
/// 3. DELETE the test schema
///
/// Uses a unique graph name to avoid conflicts with production data.
///
/// Note: Expected-schema API is on the websocket server (default port 3002)
pub async fn verify_schema_roundtrip() -> Result<SchemaRoundtripResult> {
    let api_url =
        std::env::var("DASHFLOW_API_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    // Use a unique graph name to avoid conflicts
    let test_graph_name = format!("__test_roundtrip_{}", uuid::Uuid::new_v4());
    let test_schema_id = format!("sha256:test_{}", uuid::Uuid::new_v4());
    let test_environment = "test".to_string();
    let test_pinned_by = "test-utils-verify".to_string();
    let test_note = "Automated roundtrip test".to_string();

    let request = SetExpectedSchemaRequest {
        schema_id: test_schema_id.clone(),
        environment: Some(test_environment.clone()),
        pinned_by: Some(test_pinned_by.clone()),
        note: Some(test_note.clone()),
    };

    // 1. PUT the test schema
    let put_response = client
        .put(format!("{api_url}/api/expected-schema/{test_graph_name}"))
        .json(&request)
        .send()
        .await;

    let put_response = match put_response {
        Ok(r) => r,
        Err(e) => {
            return Ok(SchemaRoundtripResult {
                put_succeeded: false,
                get_succeeded: false,
                delete_succeeded: false,
                schema_matched: false,
                error: Some(format!("PUT request failed: {e}")),
                retrieved_schema: None,
            });
        }
    };

    if !put_response.status().is_success() {
        return Ok(SchemaRoundtripResult {
            put_succeeded: false,
            get_succeeded: false,
            delete_succeeded: false,
            schema_matched: false,
            error: Some(format!("PUT returned status {}", put_response.status())),
            retrieved_schema: None,
        });
    }

    // 2. GET the schema back
    let get_response = client
        .get(format!("{api_url}/api/expected-schema/{test_graph_name}"))
        .send()
        .await;

    let get_response = match get_response {
        Ok(r) => r,
        Err(e) => {
            // Clean up: try to delete even if GET failed
            let _ = client
                .delete(format!("{api_url}/api/expected-schema/{test_graph_name}"))
                .send()
                .await;
            return Ok(SchemaRoundtripResult {
                put_succeeded: true,
                get_succeeded: false,
                delete_succeeded: false,
                schema_matched: false,
                error: Some(format!("GET request failed: {e}")),
                retrieved_schema: None,
            });
        }
    };

    if !get_response.status().is_success() {
        // Clean up
        let _ = client
            .delete(format!("{api_url}/api/expected-schema/{test_graph_name}"))
            .send()
            .await;
        return Ok(SchemaRoundtripResult {
            put_succeeded: true,
            get_succeeded: false,
            delete_succeeded: false,
            schema_matched: false,
            error: Some(format!("GET returned status {}", get_response.status())),
            retrieved_schema: None,
        });
    }

    // Parse the retrieved schema
    let retrieved: ExpectedSchemaEntry = match get_response.json().await {
        Ok(schema) => schema,
        Err(e) => {
            // Clean up
            let _ = client
                .delete(format!("{api_url}/api/expected-schema/{test_graph_name}"))
                .send()
                .await;
            return Ok(SchemaRoundtripResult {
                put_succeeded: true,
                get_succeeded: true,
                delete_succeeded: false,
                schema_matched: false,
                error: Some(format!("Failed to parse GET response: {e}")),
                retrieved_schema: None,
            });
        }
    };

    // 3. Verify the content matches
    let mut mismatches = Vec::new();
    if retrieved.schema_id != test_schema_id {
        mismatches.push(format!(
            "schema_id mismatch: expected '{}', got '{}'",
            test_schema_id, retrieved.schema_id
        ));
    }
    if retrieved.graph_name != test_graph_name {
        mismatches.push(format!(
            "graph_name mismatch: expected '{}', got '{}'",
            test_graph_name, retrieved.graph_name
        ));
    }
    if retrieved.environment != Some(test_environment.clone()) {
        mismatches.push(format!(
            "environment mismatch: expected '{:?}', got '{:?}'",
            Some(&test_environment),
            retrieved.environment
        ));
    }
    if retrieved.pinned_by != Some(test_pinned_by.clone()) {
        mismatches.push(format!(
            "pinned_by mismatch: expected '{:?}', got '{:?}'",
            Some(&test_pinned_by),
            retrieved.pinned_by
        ));
    }
    if retrieved.pinned_at <= 0 {
        mismatches.push(format!(
            "pinned_at invalid: expected positive timestamp, got {}",
            retrieved.pinned_at
        ));
    }

    let schema_matched = mismatches.is_empty();

    // 4. DELETE the test schema
    let delete_response = client
        .delete(format!("{api_url}/api/expected-schema/{test_graph_name}"))
        .send()
        .await;

    let delete_succeeded = match delete_response {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    };

    let error = if !schema_matched {
        Some(mismatches.join("; "))
    } else {
        None
    };

    Ok(SchemaRoundtripResult {
        put_succeeded: true,
        get_succeeded: true,
        delete_succeeded,
        schema_matched,
        error,
        retrieved_schema: Some(retrieved),
    })
}

/// Grafana query response structures
#[derive(Debug, Deserialize)]
pub struct GrafanaQueryResponse {
    pub results: serde_json::Value,
}

/// Grafana datasource query request body
#[derive(Debug, serde::Serialize)]
struct GrafanaQueryRequest {
    queries: Vec<GrafanaQuery>,
    from: String,
    to: String,
}

#[derive(Debug, serde::Serialize)]
struct GrafanaQuery {
    #[serde(rename = "refId")]
    ref_id: String,
    datasource: GrafanaDatasource,
    expr: String,
    #[serde(rename = "intervalMs")]
    interval_ms: u64,
    #[serde(rename = "maxDataPoints")]
    max_data_points: u64,
}

#[derive(Debug, serde::Serialize)]
struct GrafanaDatasource {
    #[serde(rename = "type")]
    ds_type: String,
    uid: String,
}

/// Response from Grafana's /api/datasources endpoint
#[derive(Debug, Deserialize)]
struct GrafanaDatasourceInfo {
    uid: String,
    #[serde(rename = "type")]
    ds_type: String,
    name: String,
}

// =============================================================================
// Typed Grafana Frame Structures (M-262: OBS-20)
// =============================================================================
//
// These types properly parse Grafana's dataframe response format instead of
// relying on manual JSON path traversal. The structure is:
//
// {
//   "results": {
//     "A": {
//       "frames": [{
//         "schema": { "name": "...", "fields": [...] },
//         "data": { "values": [[timestamps], [values]] }
//       }],
//       "status": 200
//     }
//   }
// }

/// Complete Grafana query response with typed results
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaTypedResponse {
    /// Results keyed by refId (e.g., "A", "B")
    pub results: std::collections::HashMap<String, GrafanaRefResult>,
}

/// Result for a single query reference (refId)
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaRefResult {
    /// Data frames containing the query results
    #[serde(default)]
    pub frames: Vec<GrafanaFrame>,
    /// HTTP status code for this query
    #[serde(default)]
    pub status: Option<u16>,
    /// Error message if query failed
    #[serde(default)]
    pub error: Option<String>,
}

/// A single Grafana data frame
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaFrame {
    /// Schema describing the frame's fields
    #[serde(default)]
    pub schema: Option<GrafanaFrameSchema>,
    /// The actual data values
    #[serde(default)]
    pub data: Option<GrafanaFrameData>,
}

/// Schema for a Grafana data frame
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaFrameSchema {
    /// Name of the frame (usually the metric name)
    #[serde(default)]
    pub name: Option<String>,
    /// Reference ID that produced this frame
    #[serde(rename = "refId", default)]
    pub ref_id: Option<String>,
    /// Field definitions
    #[serde(default)]
    pub fields: Vec<GrafanaFrameField>,
}

/// Field definition within a frame schema
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaFrameField {
    /// Field name (e.g., "Time", "Value", or label names)
    pub name: String,
    /// Field type (e.g., "time", "number", "string")
    #[serde(rename = "type", default)]
    pub field_type: Option<String>,
    /// Labels associated with this field (for metrics)
    #[serde(default)]
    pub labels: Option<std::collections::HashMap<String, String>>,
}

/// Data section of a Grafana frame
#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaFrameData {
    /// Values as column arrays: `[[timestamps], [values], ...]`
    /// Each inner array corresponds to a field in the schema
    #[serde(default)]
    pub values: Vec<serde_json::Value>,
}

impl GrafanaFrame {
    /// Check if this frame contains any data points
    pub fn has_data(&self) -> bool {
        if let Some(data) = &self.data {
            // Check that values array is not empty and contains non-empty arrays
            !data.values.is_empty()
                && data.values.iter().any(|col| {
                    col.as_array().map(|a| !a.is_empty()).unwrap_or(false)
                })
        } else {
            false
        }
    }

    /// Get the number of data points in this frame
    pub fn data_point_count(&self) -> usize {
        self.data
            .as_ref()
            .and_then(|d| d.values.first())
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    }

    /// Extract numeric values from the specified column index
    /// Returns None if the column doesn't exist or contains non-numeric data
    pub fn get_numeric_values(&self, column_index: usize) -> Option<Vec<f64>> {
        self.data
            .as_ref()
            .and_then(|d| d.values.get(column_index))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            })
    }

    /// Get the metric name from the schema
    pub fn metric_name(&self) -> Option<&str> {
        self.schema.as_ref().and_then(|s| s.name.as_deref())
    }

    /// Get labels for the value field (typically the last field)
    pub fn labels(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.schema
            .as_ref()
            .and_then(|s| s.fields.last())
            .and_then(|f| f.labels.as_ref())
    }
}

impl GrafanaTypedResponse {
    /// Check if any refId has data
    pub fn has_any_data(&self) -> bool {
        self.results.values().any(|r| r.has_data())
    }

    /// Get frames for a specific refId
    pub fn frames_for(&self, ref_id: &str) -> &[GrafanaFrame] {
        self.results
            .get(ref_id)
            .map(|r| r.frames.as_slice())
            .unwrap_or(&[])
    }

    /// Get all frames across all refIds
    pub fn all_frames(&self) -> impl Iterator<Item = &GrafanaFrame> {
        self.results.values().flat_map(|r| r.frames.iter())
    }
}

impl GrafanaRefResult {
    /// Check if this result has any frames with data
    pub fn has_data(&self) -> bool {
        self.frames.iter().any(|f| f.has_data())
    }

    /// Check if this result has an error
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Value assertion for Grafana frame data (M-262: OBS-20)
#[derive(Debug, Clone)]
pub enum GrafanaValueAssertion {
    /// At least one data point exists
    HasData,
    /// All values are within range [min, max]
    InRange { min: f64, max: f64 },
    /// At least one value is greater than threshold
    AnyGreaterThan(f64),
    /// At least one value is less than threshold
    AnyLessThan(f64),
    /// All values are greater than threshold
    AllGreaterThan(f64),
    /// All values are non-zero
    AllNonZero,
    /// The most recent value (last in array) meets a condition
    LatestGreaterThan(f64),
    /// Minimum number of data points required
    MinDataPoints(usize),
}

/// Result of a Grafana value assertion
#[derive(Debug, Clone)]
pub struct GrafanaAssertionResult {
    /// Whether the assertion passed
    pub passed: bool,
    /// Human-readable explanation
    pub message: String,
    /// The values that were checked (if numeric)
    pub checked_values: Option<Vec<f64>>,
}

impl GrafanaAssertionResult {
    fn pass(message: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            checked_values: None,
        }
    }

    fn fail(message: impl Into<String>) -> Self {
        Self {
            passed: false,
            message: message.into(),
            checked_values: None,
        }
    }

    fn with_values(mut self, values: Vec<f64>) -> Self {
        self.checked_values = Some(values);
        self
    }
}

impl GrafanaValueAssertion {
    /// Check this assertion against a Grafana frame
    pub fn check(&self, frame: &GrafanaFrame) -> GrafanaAssertionResult {
        match self {
            GrafanaValueAssertion::HasData => {
                if frame.has_data() {
                    GrafanaAssertionResult::pass(format!(
                        "frame has {} data points",
                        frame.data_point_count()
                    ))
                } else {
                    GrafanaAssertionResult::fail("frame has no data points")
                }
            }

            GrafanaValueAssertion::InRange { min, max } => {
                // Value column is typically index 1 (after timestamp column)
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        let all_in_range = vals.iter().all(|v| *v >= *min && *v <= *max);
                        if all_in_range {
                            GrafanaAssertionResult::pass(format!(
                                "all {} values in range [{}, {}]",
                                vals.len(),
                                min,
                                max
                            ))
                            .with_values(vals)
                        } else {
                            let out_of_range: Vec<_> = vals
                                .iter()
                                .filter(|v| **v < *min || **v > *max)
                                .copied()
                                .collect();
                            GrafanaAssertionResult::fail(format!(
                                "{} values out of range [{}, {}]: {:?}",
                                out_of_range.len(),
                                min,
                                max,
                                out_of_range.iter().take(5).collect::<Vec<_>>()
                            ))
                            .with_values(vals)
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::AnyGreaterThan(threshold) => {
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        if let Some(max_val) = vals.iter().copied().reduce(f64::max) {
                            if max_val > *threshold {
                                GrafanaAssertionResult::pass(format!(
                                    "max value {} > threshold {}",
                                    max_val, threshold
                                ))
                                .with_values(vals)
                            } else {
                                GrafanaAssertionResult::fail(format!(
                                    "max value {} <= threshold {}",
                                    max_val, threshold
                                ))
                                .with_values(vals)
                            }
                        } else {
                            GrafanaAssertionResult::fail("no values to compare")
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::AnyLessThan(threshold) => {
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        if let Some(min_val) = vals.iter().copied().reduce(f64::min) {
                            if min_val < *threshold {
                                GrafanaAssertionResult::pass(format!(
                                    "min value {} < threshold {}",
                                    min_val, threshold
                                ))
                                .with_values(vals)
                            } else {
                                GrafanaAssertionResult::fail(format!(
                                    "min value {} >= threshold {}",
                                    min_val, threshold
                                ))
                                .with_values(vals)
                            }
                        } else {
                            GrafanaAssertionResult::fail("no values to compare")
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::AllGreaterThan(threshold) => {
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        let failing: Vec<_> =
                            vals.iter().filter(|v| **v <= *threshold).copied().collect();
                        if failing.is_empty() {
                            GrafanaAssertionResult::pass(format!(
                                "all {} values > {}",
                                vals.len(),
                                threshold
                            ))
                            .with_values(vals)
                        } else {
                            GrafanaAssertionResult::fail(format!(
                                "{} values <= {}: {:?}",
                                failing.len(),
                                threshold,
                                failing.iter().take(5).collect::<Vec<_>>()
                            ))
                            .with_values(vals)
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::AllNonZero => {
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        let zeros = vals.iter().filter(|v| **v == 0.0).count();
                        if zeros == 0 {
                            GrafanaAssertionResult::pass(format!(
                                "all {} values are non-zero",
                                vals.len()
                            ))
                            .with_values(vals)
                        } else {
                            GrafanaAssertionResult::fail(format!(
                                "{} of {} values are zero",
                                zeros,
                                vals.len()
                            ))
                            .with_values(vals)
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::LatestGreaterThan(threshold) => {
                let values = frame.get_numeric_values(1);
                match values {
                    Some(vals) if vals.is_empty() => {
                        GrafanaAssertionResult::fail("no numeric values found")
                    }
                    Some(vals) => {
                        if let Some(latest) = vals.last() {
                            if *latest > *threshold {
                                GrafanaAssertionResult::pass(format!(
                                    "latest value {} > threshold {}",
                                    latest, threshold
                                ))
                                .with_values(vals)
                            } else {
                                GrafanaAssertionResult::fail(format!(
                                    "latest value {} <= threshold {}",
                                    latest, threshold
                                ))
                                .with_values(vals)
                            }
                        } else {
                            GrafanaAssertionResult::fail("no values found")
                        }
                    }
                    None => GrafanaAssertionResult::fail("no numeric values found"),
                }
            }

            GrafanaValueAssertion::MinDataPoints(min_count) => {
                let count = frame.data_point_count();
                if count >= *min_count {
                    GrafanaAssertionResult::pass(format!(
                        "{} data points >= minimum {}",
                        count, min_count
                    ))
                } else {
                    GrafanaAssertionResult::fail(format!(
                        "{} data points < minimum {}",
                        count, min_count
                    ))
                }
            }
        }
    }
}

/// Discover the UID of the Prometheus datasource from Grafana
///
/// Queries /api/datasources and returns the UID of the first Prometheus datasource found.
/// Falls back to "prometheus" if discovery fails.
async fn discover_prometheus_datasource_uid(
    client: &Client,
    grafana_url: &str,
    grafana_user: &str,
    grafana_pass: &str,
) -> String {
    let response = client
        .get(format!("{grafana_url}/api/datasources"))
        .basic_auth(grafana_user, Some(grafana_pass))
        .send()
        .await;

    match response {
        Ok(r) if r.status().is_success() => {
            if let Ok(datasources) = r.json::<Vec<GrafanaDatasourceInfo>>().await {
                // Find the first Prometheus datasource
                if let Some(prom) = datasources.iter().find(|ds| ds.ds_type == "prometheus") {
                    tracing::debug!(
                        "Discovered Prometheus datasource '{}' with UID: {}",
                        prom.name,
                        prom.uid
                    );
                    return prom.uid.clone();
                }
            }
        }
        Ok(r) => {
            tracing::warn!("Failed to fetch datasources: status {}", r.status());
        }
        Err(e) => {
            tracing::warn!("Failed to fetch datasources: {}", e);
        }
    }

    // Fallback to default UID
    tracing::debug!("Using fallback datasource UID: prometheus");
    "prometheus".to_string()
}

/// Check if Grafana returns data for a PromQL query via proper /api/ds/query POST API
///
/// This uses the Grafana datasource query API which requires:
/// - POST request with JSON body
/// - Basic auth (default: admin:admin)
/// - Time range in Unix milliseconds
///
/// Datasource UID is discovered dynamically from Grafana's /api/datasources endpoint.
pub async fn check_grafana_has_data(panel_query: &str) -> Result<bool> {
    let grafana_url =
        std::env::var("GRAFANA_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let grafana_user = std::env::var("GRAFANA_USER").unwrap_or_else(|_| "admin".to_string());
    let grafana_pass = std::env::var("GRAFANA_PASSWORD").unwrap_or_else(|_| "admin".to_string());

    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    // Discover the Prometheus datasource UID dynamically
    let datasource_uid =
        discover_prometheus_datasource_uid(&client, &grafana_url, &grafana_user, &grafana_pass)
            .await;

    // Time range: last 5 minutes
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let from_ms = now_ms - (5 * 60 * 1000); // 5 minutes ago

    // Build the query request body
    let request_body = GrafanaQueryRequest {
        queries: vec![GrafanaQuery {
            ref_id: "A".to_string(),
            datasource: GrafanaDatasource {
                ds_type: "prometheus".to_string(),
                uid: datasource_uid,
            },
            expr: panel_query.to_string(),
            interval_ms: 15000,
            max_data_points: 100,
        }],
        from: from_ms.to_string(),
        to: now_ms.to_string(),
    };

    // Query Grafana API with POST and proper authentication
    let response = client
        .post(format!("{grafana_url}/api/ds/query"))
        .basic_auth(&grafana_user, Some(&grafana_pass))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await;

    match response {
        Ok(r) if r.status().is_success() => {
            let body = r.text().await.unwrap_or_default();
            tracing::debug!("Grafana query response: {}", body);

            // Parse response using typed structures (M-262: OBS-20)
            let has_data = match serde_json::from_str::<GrafanaTypedResponse>(&body) {
                Ok(typed_response) => typed_response.has_any_data(),
                Err(e) => {
                    tracing::warn!("Failed to parse Grafana response as typed frame: {}", e);
                    false
                }
            };

            if !has_data {
                tracing::debug!("Grafana query returned no data for: {}", panel_query);
            }
            Ok(has_data)
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            tracing::warn!(
                "Grafana query returned status {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
            Ok(false)
        }
        Err(e) => {
            tracing::warn!("Grafana query failed: {}", e);
            Ok(false)
        }
    }
}

/// Query Grafana and return typed frame data for detailed analysis (M-262: OBS-20)
///
/// Unlike `check_grafana_has_data` which returns a boolean, this returns the full
/// typed response structure allowing callers to inspect frame schemas, values,
/// and run assertions on the data.
///
/// # Example
/// ```ignore
/// let response = query_grafana_frames("up{job='prometheus'}").await?;
/// for frame in response.all_frames() {
///     if let Some(values) = frame.get_numeric_values(1) {
///         println!("Values: {:?}", values);
///     }
/// }
/// ```
pub async fn query_grafana_frames(panel_query: &str) -> Result<GrafanaTypedResponse> {
    let grafana_url =
        std::env::var("GRAFANA_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let grafana_user = std::env::var("GRAFANA_USER").unwrap_or_else(|_| "admin".to_string());
    let grafana_pass = std::env::var("GRAFANA_PASSWORD").unwrap_or_else(|_| "admin".to_string());

    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    // Discover the Prometheus datasource UID dynamically
    let datasource_uid =
        discover_prometheus_datasource_uid(&client, &grafana_url, &grafana_user, &grafana_pass)
            .await;

    // Time range: last 5 minutes
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let from_ms = now_ms - (5 * 60 * 1000); // 5 minutes ago

    // Build the query request body
    let request_body = GrafanaQueryRequest {
        queries: vec![GrafanaQuery {
            ref_id: "A".to_string(),
            datasource: GrafanaDatasource {
                ds_type: "prometheus".to_string(),
                uid: datasource_uid,
            },
            expr: panel_query.to_string(),
            interval_ms: 15000,
            max_data_points: 100,
        }],
        from: from_ms.to_string(),
        to: now_ms.to_string(),
    };

    // Query Grafana API with POST and proper authentication
    let response = client
        .post(format!("{grafana_url}/api/ds/query"))
        .basic_auth(&grafana_user, Some(&grafana_pass))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await;

    match response {
        Ok(r) if r.status().is_success() => {
            let body = r.text().await.unwrap_or_default();
            tracing::debug!("Grafana query response: {}", body);

            serde_json::from_str::<GrafanaTypedResponse>(&body).map_err(|e| {
                TestError::Other(format!("failed to parse Grafana response: {}", e))
            })
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            Err(TestError::Other(format!(
                "Grafana query returned status {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )))
        }
        Err(e) => Err(TestError::Other(format!("Grafana query failed: {}", e))),
    }
}

/// Verify Grafana data with assertions (M-262: OBS-20)
///
/// Queries Grafana for the given PromQL expression and runs the specified
/// assertions against all frames in the response.
///
/// # Arguments
/// * `panel_query` - The PromQL expression to query
/// * `assertions` - List of assertions to check against each frame
///
/// # Returns
/// * `Ok(GrafanaVerificationResult)` - Contains pass/fail status and detailed results
///
/// # Example
/// ```ignore
/// use test_utils::observability::{verify_grafana_data, GrafanaValueAssertion};
///
/// let result = verify_grafana_data(
///     "dashstream_quality_monitor_quality_score",
///     &[
///         GrafanaValueAssertion::HasData,
///         GrafanaValueAssertion::InRange { min: 0.0, max: 1.0 },
///     ],
/// ).await?;
///
/// assert!(result.passed, "Verification failed: {:?}", result.failures);
/// ```
pub async fn verify_grafana_data(
    panel_query: &str,
    assertions: &[GrafanaValueAssertion],
) -> Result<GrafanaVerificationResult> {
    let response = query_grafana_frames(panel_query).await?;

    let mut results = Vec::new();
    let mut all_passed = true;

    // Check assertions against all frames
    for (frame_idx, frame) in response.all_frames().enumerate() {
        for assertion in assertions {
            let result = assertion.check(frame);
            if !result.passed {
                all_passed = false;
            }
            results.push(GrafanaFrameAssertionResult {
                frame_index: frame_idx,
                metric_name: frame.metric_name().map(String::from),
                assertion: format!("{:?}", assertion),
                result,
            });
        }
    }

    // If no frames, assertions should fail
    let frame_count = response.all_frames().count();
    if frame_count == 0 {
        all_passed = false;
        results.push(GrafanaFrameAssertionResult {
            frame_index: 0,
            metric_name: None,
            assertion: "HasData".to_string(),
            result: GrafanaAssertionResult::fail("no frames returned from query"),
        });
    }

    // Collect failures before moving results
    let failures: Vec<_> = results
        .iter()
        .filter(|r| !r.result.passed)
        .cloned()
        .collect();

    Ok(GrafanaVerificationResult {
        passed: all_passed,
        query: panel_query.to_string(),
        frame_count,
        assertion_results: results,
        failures,
    })
}

/// Result of verifying Grafana data with assertions
#[derive(Debug, Clone)]
pub struct GrafanaVerificationResult {
    /// Whether all assertions passed
    pub passed: bool,
    /// The PromQL query that was executed
    pub query: String,
    /// Number of frames returned
    pub frame_count: usize,
    /// Results for each assertion on each frame
    pub assertion_results: Vec<GrafanaFrameAssertionResult>,
    /// Only the failed assertions
    pub failures: Vec<GrafanaFrameAssertionResult>,
}

/// Result of an assertion on a specific frame
#[derive(Debug, Clone)]
pub struct GrafanaFrameAssertionResult {
    /// Index of the frame this result applies to
    pub frame_index: usize,
    /// Metric name from the frame schema (if available)
    pub metric_name: Option<String>,
    /// String representation of the assertion
    pub assertion: String,
    /// The assertion result
    pub result: GrafanaAssertionResult,
}

/// Query Prometheus for quality_score and verify it's in valid range (0.0-1.0)
/// Note: The actual metric name is dashstream_quality_monitor_quality_score
pub async fn query_quality_score_in_range() -> Result<Option<(f64, bool)>> {
    let value = query_prometheus("dashstream_quality_monitor_quality_score").await?;
    match value {
        Some(v) => {
            let in_range = (0.0..=1.0).contains(&v);
            Ok(Some((v, in_range)))
        }
        None => Ok(None),
    }
}

/// Configuration for polling operations (M-109)
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// Maximum time to wait for the condition
    pub timeout: Duration,
    /// Interval between checks
    pub poll_interval: Duration,
    /// Whether to log progress
    pub verbose: bool,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            poll_interval: Duration::from_secs(2),
            verbose: true,
        }
    }
}

impl PollingConfig {
    /// Create a fast polling config for quick operations
    #[must_use]
    pub fn fast() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            poll_interval: Duration::from_millis(500),
            verbose: false,
        }
    }

    /// Create a config for longer operations
    #[must_use]
    pub fn slow() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            poll_interval: Duration::from_secs(5),
            verbose: true,
        }
    }
}

/// Wait for Kafka offset to increase (M-109: polling instead of fixed sleep)
///
/// Polls the Kafka offset until it increases from the baseline by at least
/// `expected_increase`, or until timeout.
///
/// # Arguments
/// * `topic` - Kafka topic to monitor
/// * `baseline_offset` - Starting offset to compare against
/// * `expected_increase` - Minimum increase expected (e.g., 3 for 3 messages)
/// * `config` - Polling configuration
///
/// # Returns
/// * `Ok(new_offset)` - The new offset after increase detected
/// * `Err` - If timeout or error
pub async fn wait_for_kafka_messages(
    topic: &str,
    baseline_offset: i64,
    expected_increase: i64,
    config: &PollingConfig,
) -> Result<i64> {
    let start = std::time::Instant::now();
    let target_offset = baseline_offset + expected_increase;

    while start.elapsed() < config.timeout {
        match get_kafka_offset(topic) {
            Ok(current_offset) => {
                let increase = current_offset - baseline_offset;
                if current_offset >= target_offset {
                    if config.verbose {
                        tracing::info!(
                            "Kafka offset reached target: {} (+{} messages in {:.1}s)",
                            current_offset,
                            increase,
                            start.elapsed().as_secs_f64()
                        );
                    }
                    return Ok(current_offset);
                }

                if config.verbose {
                    tracing::debug!(
                        "Kafka offset: {} (+{}), waiting for +{} ({:.1}s elapsed)",
                        current_offset,
                        increase,
                        expected_increase,
                        start.elapsed().as_secs_f64()
                    );
                }
            }
            Err(e) => {
                if config.verbose {
                    tracing::warn!("Error checking Kafka offset: {}", e);
                }
            }
        }

        sleep(config.poll_interval).await;
    }

    // Final check after timeout
    let final_offset = get_kafka_offset(topic)?;
    Err(TestError::ServiceUnhealthy(format!(
        "Kafka offset did not reach target {} (current: {}, baseline: {}) after {:?}",
        target_offset, final_offset, baseline_offset, config.timeout
    )))
}

/// Wait for quality aggregator to process metrics (M-109: polling instead of fixed sleep)
///
/// Polls the quality aggregator logs until it shows at least `expected_count`
/// processed metrics, or until timeout.
///
/// # Arguments
/// * `baseline_count` - Starting processed count to compare against
/// * `expected_increase` - Minimum increase expected
/// * `config` - Polling configuration
///
/// # Returns
/// * `Ok(new_count)` - The new processed count
/// * `Err` - If timeout or error
pub async fn wait_for_quality_processed(
    baseline_count: usize,
    expected_increase: usize,
    config: &PollingConfig,
) -> Result<usize> {
    let start = std::time::Instant::now();
    let target_count = baseline_count + expected_increase;

    while start.elapsed() < config.timeout {
        match count_quality_processed() {
            Ok(current_count) => {
                let increase = current_count.saturating_sub(baseline_count);
                if current_count >= target_count {
                    if config.verbose {
                        tracing::info!(
                            "Quality processed reached target: {} (+{} in {:.1}s)",
                            current_count,
                            increase,
                            start.elapsed().as_secs_f64()
                        );
                    }
                    return Ok(current_count);
                }

                if config.verbose {
                    tracing::debug!(
                        "Quality processed: {} (+{}), waiting for +{} ({:.1}s elapsed)",
                        current_count,
                        increase,
                        expected_increase,
                        start.elapsed().as_secs_f64()
                    );
                }
            }
            Err(e) => {
                if config.verbose {
                    tracing::warn!("Error checking quality processed: {}", e);
                }
            }
        }

        sleep(config.poll_interval).await;
    }

    // Final check after timeout
    let final_count = count_quality_processed()?;
    Err(TestError::ServiceUnhealthy(format!(
        "Quality processed did not reach target {} (current: {}, baseline: {}) after {:?}",
        target_count, final_count, baseline_count, config.timeout
    )))
}

/// Wait for Prometheus metric to appear (M-109: polling instead of fixed sleep)
///
/// Polls Prometheus until the specified metric returns a value, or until timeout.
///
/// # Arguments
/// * `metric_name` - The Prometheus metric to query
/// * `config` - Polling configuration
///
/// # Returns
/// * `Ok(Some(value))` - The metric value when found
/// * `Ok(None)` - If metric never appeared (and timeout not exceeded)
/// * `Err` - If error
pub async fn wait_for_prometheus_metric(
    metric_name: &str,
    config: &PollingConfig,
) -> Result<Option<f64>> {
    let start = std::time::Instant::now();

    while start.elapsed() < config.timeout {
        match query_prometheus(metric_name).await {
            Ok(Some(value)) => {
                if config.verbose {
                    tracing::info!(
                        "Prometheus metric '{}' = {} (found in {:.1}s)",
                        metric_name,
                        value,
                        start.elapsed().as_secs_f64()
                    );
                }
                return Ok(Some(value));
            }
            Ok(None) => {
                if config.verbose {
                    tracing::debug!(
                        "Prometheus metric '{}' not yet available ({:.1}s elapsed)",
                        metric_name,
                        start.elapsed().as_secs_f64()
                    );
                }
            }
            Err(e) => {
                if config.verbose {
                    tracing::warn!("Error querying Prometheus metric '{}': {}", metric_name, e);
                }
            }
        }

        sleep(config.poll_interval).await;
    }

    if config.verbose {
        tracing::warn!(
            "Prometheus metric '{}' not found after {:?}",
            metric_name,
            config.timeout
        );
    }
    Ok(None)
}

/// Results from an observability pipeline test
#[derive(Debug, Default)]
pub struct ObservabilityTestResult {
    /// Number of new Kafka messages
    pub kafka_messages: i64,
    /// Number of quality metrics processed by aggregator
    pub quality_processed: usize,
    /// Prometheus metric value (if available)
    pub prometheus_value: Option<f64>,
}

impl ObservabilityTestResult {
    /// Check if the pipeline is fully working
    pub fn is_pipeline_working(&self) -> bool {
        self.kafka_messages > 0 && self.quality_processed > 0
    }

    /// Check if Kafka is receiving messages
    pub fn is_kafka_receiving(&self) -> bool {
        self.kafka_messages > 0
    }

    /// Check if quality aggregator is processing
    pub fn is_aggregator_processing(&self) -> bool {
        self.quality_processed > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_result_is_working() {
        let result = ObservabilityTestResult {
            kafka_messages: 10,
            quality_processed: 5,
            prometheus_value: Some(0.85),
        };

        assert!(result.is_pipeline_working());
        assert!(result.is_kafka_receiving());
        assert!(result.is_aggregator_processing());
    }

    #[test]
    fn test_observability_result_partial() {
        let result = ObservabilityTestResult {
            kafka_messages: 10,
            quality_processed: 0,
            prometheus_value: None,
        };

        assert!(!result.is_pipeline_working());
        assert!(result.is_kafka_receiving());
        assert!(!result.is_aggregator_processing());
    }

    // ==========================================================================
    // Grafana Frame Parsing Tests (M-262: OBS-20)
    // ==========================================================================

    /// Sample Grafana response for testing frame parsing
    fn sample_grafana_response() -> &'static str {
        r#"{
            "results": {
                "A": {
                    "frames": [{
                        "schema": {
                            "name": "up{instance=\"localhost:9090\",job=\"prometheus\"}",
                            "refId": "A",
                            "fields": [
                                {"name": "Time", "type": "time"},
                                {"name": "Value", "type": "number", "labels": {"instance": "localhost:9090", "job": "prometheus"}}
                            ]
                        },
                        "data": {
                            "values": [
                                [1703862000000, 1703862015000, 1703862030000],
                                [1.0, 1.0, 1.0]
                            ]
                        }
                    }],
                    "status": 200
                }
            }
        }"#
    }

    #[test]
    fn test_parse_grafana_typed_response() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        assert!(response.has_any_data());
        assert_eq!(response.results.len(), 1);
        assert!(response.results.contains_key("A"));

        let ref_a = &response.results["A"];
        assert_eq!(ref_a.frames.len(), 1);
        assert!(ref_a.has_data());
        assert!(!ref_a.has_error());
    }

    #[test]
    fn test_grafana_frame_has_data() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        let frame = &response.results["A"].frames[0];
        assert!(frame.has_data());
        assert_eq!(frame.data_point_count(), 3);
    }

    #[test]
    fn test_grafana_frame_empty_data() {
        let json = r#"{
            "results": {
                "A": {
                    "frames": [{
                        "schema": {"name": "empty"},
                        "data": {"values": []}
                    }]
                }
            }
        }"#;

        let response: GrafanaTypedResponse = serde_json::from_str(json).expect("should parse");
        let frame = &response.results["A"].frames[0];
        assert!(!frame.has_data());
        assert_eq!(frame.data_point_count(), 0);
    }

    #[test]
    fn test_grafana_frame_get_numeric_values() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        let frame = &response.results["A"].frames[0];

        // Column 0 is timestamps
        let timestamps = frame.get_numeric_values(0).expect("should have timestamps");
        assert_eq!(timestamps.len(), 3);
        assert!((timestamps[0] - 1_703_862_000_000.0).abs() < 0.5);

        // Column 1 is values
        let values = frame.get_numeric_values(1).expect("should have values");
        assert_eq!(values.len(), 3);
        assert!(values.iter().all(|&v| (v - 1.0).abs() < f64::EPSILON));

        // Column 2 doesn't exist
        assert!(frame.get_numeric_values(2).is_none());
    }

    #[test]
    fn test_grafana_frame_metric_name() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        let frame = &response.results["A"].frames[0];
        assert_eq!(
            frame.metric_name(),
            Some("up{instance=\"localhost:9090\",job=\"prometheus\"}")
        );
    }

    #[test]
    fn test_grafana_frame_labels() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        let frame = &response.results["A"].frames[0];
        let labels = frame.labels().expect("should have labels");
        assert_eq!(labels.get("instance"), Some(&"localhost:9090".to_string()));
        assert_eq!(labels.get("job"), Some(&"prometheus".to_string()));
    }

    // ==========================================================================
    // Value Assertion Tests (M-262: OBS-20)
    // ==========================================================================

    fn frame_with_values(values: Vec<f64>) -> GrafanaFrame {
        let timestamps: Vec<f64> = (0..values.len())
            .map(|i| 1_700_000_000_000.0 + (i as f64 * 15000.0))
            .collect();

        GrafanaFrame {
            schema: Some(GrafanaFrameSchema {
                name: Some("test_metric".to_string()),
                ref_id: Some("A".to_string()),
                fields: vec![
                    GrafanaFrameField {
                        name: "Time".to_string(),
                        field_type: Some("time".to_string()),
                        labels: None,
                    },
                    GrafanaFrameField {
                        name: "Value".to_string(),
                        field_type: Some("number".to_string()),
                        labels: None,
                    },
                ],
            }),
            data: Some(GrafanaFrameData {
                values: vec![
                    serde_json::json!(timestamps),
                    serde_json::json!(values),
                ],
            }),
        }
    }

    #[test]
    fn test_assertion_has_data_pass() {
        let frame = frame_with_values(vec![1.0, 2.0, 3.0]);
        let result = GrafanaValueAssertion::HasData.check(&frame);
        assert!(result.passed);
        assert!(result.message.contains("3 data points"));
    }

    #[test]
    fn test_assertion_has_data_fail() {
        let frame = frame_with_values(vec![]);
        let result = GrafanaValueAssertion::HasData.check(&frame);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_in_range_pass() {
        let frame = frame_with_values(vec![0.5, 0.7, 0.9]);
        let result = GrafanaValueAssertion::InRange { min: 0.0, max: 1.0 }.check(&frame);
        assert!(result.passed);
        assert!(result.message.contains("all 3 values in range"));
    }

    #[test]
    fn test_assertion_in_range_fail() {
        let frame = frame_with_values(vec![0.5, 1.5, 0.9]);
        let result = GrafanaValueAssertion::InRange { min: 0.0, max: 1.0 }.check(&frame);
        assert!(!result.passed);
        assert!(result.message.contains("out of range"));
    }

    #[test]
    fn test_assertion_any_greater_than_pass() {
        let frame = frame_with_values(vec![0.1, 0.2, 0.5]);
        let result = GrafanaValueAssertion::AnyGreaterThan(0.3).check(&frame);
        assert!(result.passed);
        assert!(result.message.contains("max value 0.5"));
    }

    #[test]
    fn test_assertion_any_greater_than_fail() {
        let frame = frame_with_values(vec![0.1, 0.2, 0.25]);
        let result = GrafanaValueAssertion::AnyGreaterThan(0.3).check(&frame);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_any_less_than_pass() {
        let frame = frame_with_values(vec![0.5, 0.3, 0.2]);
        let result = GrafanaValueAssertion::AnyLessThan(0.25).check(&frame);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_any_less_than_fail() {
        let frame = frame_with_values(vec![0.5, 0.3, 0.4]);
        let result = GrafanaValueAssertion::AnyLessThan(0.25).check(&frame);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_all_greater_than_pass() {
        let frame = frame_with_values(vec![0.5, 0.6, 0.7]);
        let result = GrafanaValueAssertion::AllGreaterThan(0.4).check(&frame);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_all_greater_than_fail() {
        let frame = frame_with_values(vec![0.5, 0.3, 0.7]);
        let result = GrafanaValueAssertion::AllGreaterThan(0.4).check(&frame);
        assert!(!result.passed);
        assert!(result.message.contains("1 values <= 0.4"));
    }

    #[test]
    fn test_assertion_all_non_zero_pass() {
        let frame = frame_with_values(vec![0.1, 0.2, 0.3]);
        let result = GrafanaValueAssertion::AllNonZero.check(&frame);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_all_non_zero_fail() {
        let frame = frame_with_values(vec![0.1, 0.0, 0.3]);
        let result = GrafanaValueAssertion::AllNonZero.check(&frame);
        assert!(!result.passed);
        assert!(result.message.contains("1 of 3 values are zero"));
    }

    #[test]
    fn test_assertion_latest_greater_than_pass() {
        let frame = frame_with_values(vec![0.1, 0.2, 0.5]);
        let result = GrafanaValueAssertion::LatestGreaterThan(0.4).check(&frame);
        assert!(result.passed);
        assert!(result.message.contains("latest value 0.5"));
    }

    #[test]
    fn test_assertion_latest_greater_than_fail() {
        let frame = frame_with_values(vec![0.5, 0.4, 0.3]);
        let result = GrafanaValueAssertion::LatestGreaterThan(0.4).check(&frame);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_min_data_points_pass() {
        let frame = frame_with_values(vec![0.1, 0.2, 0.3]);
        let result = GrafanaValueAssertion::MinDataPoints(3).check(&frame);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_min_data_points_fail() {
        let frame = frame_with_values(vec![0.1, 0.2]);
        let result = GrafanaValueAssertion::MinDataPoints(3).check(&frame);
        assert!(!result.passed);
        assert!(result.message.contains("2 data points < minimum 3"));
    }

    #[test]
    fn test_grafana_typed_response_helpers() {
        let response: GrafanaTypedResponse =
            serde_json::from_str(sample_grafana_response()).expect("should parse");

        // Test frames_for
        let frames = response.frames_for("A");
        assert_eq!(frames.len(), 1);

        let frames_missing = response.frames_for("B");
        assert!(frames_missing.is_empty());

        // Test all_frames
        let all_frames: Vec<_> = response.all_frames().collect();
        assert_eq!(all_frames.len(), 1);
    }

    #[test]
    fn test_assertion_result_with_values() {
        let result = GrafanaAssertionResult::pass("test")
            .with_values(vec![1.0, 2.0, 3.0]);
        assert!(result.passed);
        assert_eq!(result.checked_values, Some(vec![1.0, 2.0, 3.0]));
    }
}
