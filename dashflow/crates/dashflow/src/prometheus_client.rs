//! Prometheus Client for querying metrics
//!
//! This module provides a client for querying Prometheus metrics,
//! enabling the self-improvement daemon to access the same data
//! that humans see in Grafana dashboards.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::prometheus_client::PrometheusClient;
//!
//! let client = PrometheusClient::new("http://localhost:9090");
//!
//! // Instant query
//! let results = client.query("up").await?;
//!
//! // Range query
//! let series = client.query_range(
//!     "rate(http_requests_total[5m])",
//!     Utc::now() - Duration::hours(1),
//!     Utc::now(),
//!     "15s",
//! ).await?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

use crate::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, SHORT_TIMEOUT};

/// Default request timeout (10 seconds)
///
/// **Deprecated since v1.11.0**: Use `crate::constants::DEFAULT_HTTP_CONNECT_TIMEOUT` instead.
/// **Scheduled for removal in v2.0**.
#[deprecated(
    since = "1.11.0",
    note = "Use crate::constants::DEFAULT_HTTP_CONNECT_TIMEOUT instead. Will be removed in v2.0."
)]
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Default connect timeout (10 seconds)
///
/// **Deprecated since v1.11.0**: Use `crate::constants::DEFAULT_HTTP_CONNECT_TIMEOUT` instead.
/// **Scheduled for removal in v2.0**.
#[deprecated(
    since = "1.11.0",
    note = "Use crate::constants::DEFAULT_HTTP_CONNECT_TIMEOUT instead. Will be removed in v2.0."
)]
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default health check timeout (5 seconds)
///
/// **Deprecated since v1.11.0**: Use `crate::constants::SHORT_TIMEOUT` instead.
/// **Scheduled for removal in v2.0**.
#[deprecated(since = "1.11.0", note = "Use crate::constants::SHORT_TIMEOUT instead. Will be removed in v2.0.")]
pub const DEFAULT_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for Prometheus client timeouts.
///
/// # Example
///
/// ```rust
/// use dashflow::prometheus_client::{PrometheusClientConfig, PrometheusClient};
/// use std::time::Duration;
///
/// let config = PrometheusClientConfig::default()
///     .with_connect_timeout(Duration::from_secs(5))
///     .with_health_check_timeout(Duration::from_secs(2));
///
/// let client = PrometheusClient::with_config("http://localhost:9090", config);
/// ```
#[derive(Clone, Debug)]
pub struct PrometheusClientConfig {
    /// Overall request timeout (default: 10s)
    pub request_timeout: Duration,
    /// Connection establishment timeout (default: 10s)
    pub connect_timeout: Duration,
    /// Health check request timeout (default: 5s)
    pub health_check_timeout: Duration,
}

impl Default for PrometheusClientConfig {
    fn default() -> Self {
        Self {
            // Use centralized constants for consistency across the codebase
            request_timeout: DEFAULT_HTTP_CONNECT_TIMEOUT,
            connect_timeout: DEFAULT_HTTP_CONNECT_TIMEOUT,
            health_check_timeout: SHORT_TIMEOUT,
        }
    }
}

impl PrometheusClientConfig {
    /// Create config with custom request timeout.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Create config with custom connect timeout.
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Create config with custom health check timeout.
    #[must_use]
    pub fn with_health_check_timeout(mut self, timeout: Duration) -> Self {
        self.health_check_timeout = timeout;
        self
    }
}

/// Prometheus client for querying metrics.
#[derive(Clone)]
pub struct PrometheusClient {
    /// Base URL of the Prometheus server (e.g., "http://localhost:9090")
    endpoint: String,
    /// HTTP client for making requests
    client: reqwest::Client,
    /// Client configuration
    config: PrometheusClientConfig,
}

/// Result of an instant query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantQueryResult {
    /// Status of the query ("success" or "error")
    pub status: String,
    /// Query results
    pub data: QueryData,
    /// Error type if status is "error"
    #[serde(default)]
    pub error_type: Option<String>,
    /// Error message if status is "error"
    #[serde(default)]
    pub error: Option<String>,
}

/// Data from a Prometheus query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryData {
    /// Result type: "vector", "matrix", "scalar", or "string"
    #[serde(rename = "resultType")]
    pub result_type: String,
    /// Query results
    pub result: Vec<QueryResult>,
}

/// A single query result (vector or matrix element).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Metric labels
    pub metric: HashMap<String, String>,
    /// For vector results: single [timestamp, value] pair
    #[serde(default)]
    pub value: Option<(f64, String)>,
    /// For matrix results: array of [timestamp, value] pairs
    #[serde(default)]
    pub values: Option<Vec<(f64, String)>>,
}

/// A time series with multiple data points.
#[derive(Debug, Clone)]
pub struct TimeSeries {
    /// Metric labels
    pub metric: HashMap<String, String>,
    /// Data points as (timestamp, value) pairs
    pub values: Vec<(DateTime<Utc>, f64)>,
}

/// A single metric value from an instant query.
#[derive(Debug, Clone)]
pub struct MetricValue {
    /// Metric labels
    pub labels: HashMap<String, String>,
    /// The metric value
    pub value: f64,
    /// Timestamp of the value
    pub timestamp: DateTime<Utc>,
}

/// Error type for Prometheus client operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum PrometheusError {
    /// HTTP request failed
    #[error("Request failed: {0}")]
    RequestFailed(String),
    /// Failed to parse response
    #[error("Parse error: {0}")]
    ParseError(String),
    /// Prometheus returned an error.
    #[error("Prometheus error ({error_type}): {message}")]
    QueryError {
        /// Type of error from Prometheus (e.g., "bad_data", "timeout").
        error_type: String,
        /// Human-readable error message.
        message: String,
    },
    /// Connection timeout
    #[error("Request timeout")]
    Timeout,
}

// ============================================================================
// Error Helpers (CQ-35: Reduce repetitive error mapping)
// ============================================================================

/// Convert any displayable error to ParseError
#[inline]
fn parse_err(e: impl std::fmt::Display) -> PrometheusError {
    PrometheusError::ParseError(e.to_string())
}

impl PrometheusClient {
    /// Create a new Prometheus client with default configuration.
    ///
    /// # Arguments
    /// * `endpoint` - Base URL of the Prometheus server (e.g., "http://localhost:9090")
    pub fn new(endpoint: &str) -> Self {
        Self::with_config(endpoint, PrometheusClientConfig::default())
    }

    /// Create a new Prometheus client with custom request timeout.
    ///
    /// Note: For full control over all timeouts (connect, health check),
    /// use `with_config()` instead.
    #[must_use]
    pub fn with_timeout(endpoint: &str, timeout: Duration) -> Self {
        let config = PrometheusClientConfig::default().with_request_timeout(timeout);
        Self::with_config(endpoint, config)
    }

    /// Create a new Prometheus client with custom configuration.
    ///
    /// # Arguments
    /// * `endpoint` - Base URL of the Prometheus server
    /// * `config` - Client configuration with all timeout settings
    #[must_use]
    pub fn with_config(endpoint: &str, config: PrometheusClientConfig) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(config.request_timeout)
                .connect_timeout(config.connect_timeout)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            config,
        }
    }

    /// Execute an instant query.
    ///
    /// # Arguments
    /// * `promql` - PromQL query string
    ///
    /// # Returns
    /// Vector of metric values at the current time
    pub async fn query(&self, promql: &str) -> Result<Vec<MetricValue>, PrometheusError> {
        let url = format!("{}/api/v1/query", self.endpoint);

        let response = self
            .client
            .get(&url)
            .query(&[("query", promql)])
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    PrometheusError::Timeout
                } else {
                    PrometheusError::RequestFailed(e.to_string())
                }
            })?;

        let result: InstantQueryResult = response.json().await.map_err(parse_err)?;

        if result.status != "success" {
            return Err(PrometheusError::QueryError {
                error_type: result.error_type.unwrap_or_default(),
                message: result.error.unwrap_or_default(),
            });
        }

        // Convert to MetricValue
        let mut values = Vec::with_capacity(result.data.result.len());
        for r in result.data.result {
            if let Some((ts, val)) = r.value {
                let timestamp = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(Utc::now);
                let value = val.parse::<f64>().unwrap_or(0.0);
                values.push(MetricValue {
                    labels: r.metric,
                    value,
                    timestamp,
                });
            }
        }

        Ok(values)
    }

    /// Execute a range query.
    ///
    /// # Arguments
    /// * `promql` - PromQL query string
    /// * `start` - Start time
    /// * `end` - End time
    /// * `step` - Query resolution step (e.g., "15s", "1m")
    ///
    /// # Returns
    /// Vector of time series with multiple data points
    pub async fn query_range(
        &self,
        promql: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        step: &str,
    ) -> Result<Vec<TimeSeries>, PrometheusError> {
        let url = format!("{}/api/v1/query_range", self.endpoint);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("query", promql),
                ("start", &start.timestamp().to_string()),
                ("end", &end.timestamp().to_string()),
                ("step", step),
            ])
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    PrometheusError::Timeout
                } else {
                    PrometheusError::RequestFailed(e.to_string())
                }
            })?;

        let result: InstantQueryResult = response.json().await.map_err(parse_err)?;

        if result.status != "success" {
            return Err(PrometheusError::QueryError {
                error_type: result.error_type.unwrap_or_default(),
                message: result.error.unwrap_or_default(),
            });
        }

        // Convert to TimeSeries
        let mut series_list = Vec::with_capacity(result.data.result.len());
        for r in result.data.result {
            let data_points = if let Some(values) = r.values {
                let mut points = Vec::with_capacity(values.len());
                for (ts, val) in values {
                    let timestamp = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(Utc::now);
                    let value = val.parse::<f64>().unwrap_or(0.0);
                    points.push((timestamp, value));
                }
                points
            } else {
                Vec::new()
            };
            series_list.push(TimeSeries {
                metric: r.metric,
                values: data_points,
            });
        }

        Ok(series_list)
    }

    /// Check if Prometheus is healthy.
    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}/-/healthy", self.endpoint);
        self.client
            .get(&url)
            .timeout(self.config.health_check_timeout)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

// ============================================================================
// Blocking Client (for synchronous code like the daemon)
// ============================================================================

/// Blocking Prometheus client for synchronous code paths.
///
/// This client uses `reqwest::blocking` and is suitable for use in
/// synchronous contexts like the self-improvement daemon.
#[derive(Clone)]
pub struct BlockingPrometheusClient {
    endpoint: String,
    client: reqwest::blocking::Client,
    config: PrometheusClientConfig,
}

impl BlockingPrometheusClient {
    /// Create a new blocking Prometheus client with default configuration.
    pub fn new(endpoint: &str) -> Self {
        Self::with_config(endpoint, PrometheusClientConfig::default())
    }

    /// Create a new blocking client with custom request timeout.
    ///
    /// Note: For full control over all timeouts (connect, health check),
    /// use `with_config()` instead.
    #[must_use]
    pub fn with_timeout(endpoint: &str, timeout: Duration) -> Self {
        let config = PrometheusClientConfig::default().with_request_timeout(timeout);
        Self::with_config(endpoint, config)
    }

    /// Create a new blocking client with custom configuration.
    ///
    /// # Arguments
    /// * `endpoint` - Base URL of the Prometheus server
    /// * `config` - Client configuration with all timeout settings
    #[must_use]
    pub fn with_config(endpoint: &str, config: PrometheusClientConfig) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            client: reqwest::blocking::Client::builder()
                .timeout(config.request_timeout)
                .connect_timeout(config.connect_timeout)
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
            config,
        }
    }

    /// Execute an instant query (blocking).
    pub fn query(&self, promql: &str) -> Result<Vec<MetricValue>, PrometheusError> {
        let url = format!("{}/api/v1/query", self.endpoint);

        let response = self
            .client
            .get(&url)
            .query(&[("query", promql)])
            .timeout(self.config.request_timeout)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    PrometheusError::Timeout
                } else {
                    PrometheusError::RequestFailed(e.to_string())
                }
            })?;

        let result: InstantQueryResult = response.json().map_err(parse_err)?;

        if result.status != "success" {
            return Err(PrometheusError::QueryError {
                error_type: result.error_type.unwrap_or_default(),
                message: result.error.unwrap_or_default(),
            });
        }

        let mut values = Vec::with_capacity(result.data.result.len());
        for r in result.data.result {
            if let Some((ts, val)) = r.value {
                let timestamp = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(Utc::now);
                let value = val.parse::<f64>().unwrap_or(0.0);
                values.push(MetricValue {
                    labels: r.metric,
                    value,
                    timestamp,
                });
            }
        }

        Ok(values)
    }

    /// Execute a range query (blocking).
    pub fn query_range(
        &self,
        promql: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        step: &str,
    ) -> Result<Vec<TimeSeries>, PrometheusError> {
        let url = format!("{}/api/v1/query_range", self.endpoint);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("query", promql),
                ("start", &start.timestamp().to_string()),
                ("end", &end.timestamp().to_string()),
                ("step", step),
            ])
            .timeout(self.config.request_timeout)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    PrometheusError::Timeout
                } else {
                    PrometheusError::RequestFailed(e.to_string())
                }
            })?;

        let result: InstantQueryResult = response.json().map_err(parse_err)?;

        if result.status != "success" {
            return Err(PrometheusError::QueryError {
                error_type: result.error_type.unwrap_or_default(),
                message: result.error.unwrap_or_default(),
            });
        }

        let mut series_list = Vec::with_capacity(result.data.result.len());
        for r in result.data.result {
            let data_points = if let Some(values) = r.values {
                let mut points = Vec::with_capacity(values.len());
                for (ts, val) in values {
                    let timestamp = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(Utc::now);
                    let value = val.parse::<f64>().unwrap_or(0.0);
                    points.push((timestamp, value));
                }
                points
            } else {
                Vec::new()
            };
            series_list.push(TimeSeries {
                metric: r.metric,
                values: data_points,
            });
        }

        Ok(series_list)
    }

    /// Check if Prometheus is healthy (blocking).
    pub fn is_healthy(&self) -> bool {
        let url = format!("{}/-/healthy", self.endpoint);
        self.client
            .get(&url)
            .timeout(self.config.health_check_timeout)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

/// Common PromQL queries for DashFlow metrics.
pub mod queries {
    /// Error rate over the last 5 minutes.
    pub const ERROR_RATE_5M: &str =
        "rate(dashflow_execution_errors_total[5m]) / rate(dashflow_executions_total[5m])";

    /// Node duration p99 latency.
    pub const NODE_DURATION_P99: &str =
        "histogram_quantile(0.99, rate(dashflow_node_duration_seconds_bucket[5m]))";

    /// Node duration p95 latency.
    pub const NODE_DURATION_P95: &str =
        "histogram_quantile(0.95, rate(dashflow_node_duration_seconds_bucket[5m]))";

    /// Total retries.
    pub const RETRIES_TOTAL: &str = "increase(dashflow_retries_total[1h])";

    /// Quality score (DashStream).
    pub const QUALITY_SCORE: &str = "dashstream_quality_monitor_quality_score";

    /// Success rate (DashStream).
    pub const SUCCESS_RATE: &str = "dashstream_quality_monitor_success_rate";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_client_new() {
        let client = PrometheusClient::new("http://localhost:9090");
        assert_eq!(client.endpoint(), "http://localhost:9090");

        // Should strip trailing slash
        let client = PrometheusClient::new("http://localhost:9090/");
        assert_eq!(client.endpoint(), "http://localhost:9090");
    }

    #[test]
    fn test_prometheus_error_display() {
        let err = PrometheusError::RequestFailed("connection refused".to_string());
        assert!(err.to_string().contains("connection refused"));

        let err = PrometheusError::QueryError {
            error_type: "bad_data".to_string(),
            message: "invalid query".to_string(),
        };
        assert!(err.to_string().contains("bad_data"));
        assert!(err.to_string().contains("invalid query"));
    }

    #[test]
    fn test_parse_instant_query_result() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {
                        "metric": {"__name__": "up", "instance": "localhost:9090"},
                        "value": [1702500000.0, "1"]
                    }
                ]
            }
        }"#;

        let result: InstantQueryResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data.result_type, "vector");
        assert_eq!(result.data.result.len(), 1);
        assert_eq!(
            result.data.result[0].metric.get("__name__"),
            Some(&"up".to_string())
        );
    }

    #[test]
    fn test_parse_range_query_result() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"__name__": "up", "instance": "localhost:9090"},
                        "values": [
                            [1702500000.0, "1"],
                            [1702500015.0, "1"],
                            [1702500030.0, "1"]
                        ]
                    }
                ]
            }
        }"#;

        let result: InstantQueryResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data.result_type, "matrix");
        assert_eq!(result.data.result.len(), 1);
        assert_eq!(result.data.result[0].values.as_ref().unwrap().len(), 3);
    }
}
