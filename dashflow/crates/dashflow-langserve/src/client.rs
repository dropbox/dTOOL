//! `RemoteRunnable` client for calling `LangServe` servers over HTTP
//!
//! # Retry Support (M-196)
//!
//! The client supports automatic retries for transient network errors using
//! exponential backoff with jitter. Use `with_retry_policy()` to configure:
//!
//! ```rust,ignore
//! use dashflow_langserve::client::RemoteRunnable;
//! use dashflow::core::retry::RetryPolicy;
//!
//! let remote = RemoteRunnable::new("http://localhost:8000/my_runnable/")?
//!     .with_retry_policy(RetryPolicy::default_jitter(3)); // 3 retries
//! ```

use crate::error::{LangServeError, Result};
use crate::schema::{
    BatchRequest, BatchResponse, InvokeRequest, InvokeResponse, RunnableConfig, StreamRequest,
};
use async_stream::stream;
use dashflow::core::error::Error as DashFlowError;
use dashflow::core::http_client;
use dashflow::core::retry::{with_retry, RetryPolicy};
use eventsource_stream::Eventsource;
use futures::stream::{Stream, StreamExt};
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use url::Url;

/// A `RemoteRunnable` is a runnable that is executed on a remote server via HTTP.
///
/// This client implements the `DashFlow` Runnable interface for calling remote
/// `LangServe` servers. It provides methods for invoke, batch, and stream operations.
///
/// # Example
///
/// ```no_run
/// use dashflow_langserve::client::RemoteRunnable;
/// use serde_json::json;
///
/// #[tokio::main]
/// async fn main() {
///     let remote = RemoteRunnable::new("http://localhost:8000/my_runnable/")
///         .expect("Failed to create RemoteRunnable");
///
///     let result = remote.invoke(json!({"text": "Hello"}), None).await
///         .expect("Failed to invoke");
///
///     println!("Result: {:?}", result);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RemoteRunnable {
    /// Base URL for the remote runnable endpoint
    base_url: Url,

    /// HTTP client for making requests
    client: Client,

    /// Request timeout in seconds
    timeout: Option<u64>,

    /// Retry policy for transient errors (M-196)
    retry_policy: RetryPolicy,
}

impl RemoteRunnable {
    /// Create a new `RemoteRunnable` client
    ///
    /// # Arguments
    ///
    /// * `url` - The base URL of the remote runnable (e.g., "<http://localhost:8000/my_runnable>/")
    ///
    /// # Returns
    ///
    /// A Result containing the `RemoteRunnable` or an error if the URL is invalid
    pub fn new(url: &str) -> Result<Self> {
        // Ensure URL ends with trailing slash
        let url_with_slash = if url.ends_with('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let base_url = Url::parse(&url_with_slash).map_err(|e| {
            LangServeError::InvalidRequest(format!("Invalid URL '{url_with_slash}': {e}"))
        })?;

        let client = http_client::create_llm_client().map_err(|e| {
            LangServeError::InternalError(format!("Failed to create HTTP client: {e}"))
        })?;

        Ok(Self {
            base_url,
            client,
            timeout: None,
            retry_policy: RetryPolicy::default(), // Default uses jitter (M-195)
        })
    }

    /// Set the retry policy for transient network errors (M-196)
    ///
    /// By default, the client uses exponential backoff with jitter (3 retries).
    /// Use this method to customize retry behavior.
    ///
    /// # Arguments
    ///
    /// * `policy` - The retry policy to use
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_langserve::client::RemoteRunnable;
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// let remote = RemoteRunnable::new("http://localhost:8000/my_runnable/")?
    ///     .with_retry_policy(RetryPolicy::exponential(5)); // 5 retries
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Create a new `RemoteRunnable` with a custom timeout
    ///
    /// # Arguments
    ///
    /// * `url` - The base URL of the remote runnable
    /// * `timeout_secs` - Request timeout in seconds
    pub fn with_timeout(url: &str, timeout_secs: u64) -> Result<Self> {
        let mut remote = Self::new(url)?;
        remote.timeout = Some(timeout_secs);

        // Rebuild client with timeout (using optimized client settings)
        let timeout = std::time::Duration::from_secs(timeout_secs);
        remote.client = http_client::HttpClientBuilder::new()
            .with_llm_defaults()
            .request_timeout(timeout)
            .build()
            .map_err(|e| {
                LangServeError::InternalError(format!("Failed to create HTTP client: {e}"))
            })?;

        Ok(remote)
    }

    /// Create a new `RemoteRunnable` with custom client configuration
    ///
    /// # Arguments
    ///
    /// * `url` - The base URL of the remote runnable
    /// * `client` - A custom `reqwest::Client`
    pub fn with_client(url: &str, client: Client) -> Result<Self> {
        let url_with_slash = if url.ends_with('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let base_url = Url::parse(&url_with_slash).map_err(|e| {
            LangServeError::InvalidRequest(format!("Invalid URL '{url_with_slash}': {e}"))
        })?;

        Ok(Self {
            base_url,
            client,
            timeout: None,
            retry_policy: RetryPolicy::default(),
        })
    }

    /// Invoke the remote runnable with the given input
    ///
    /// # Arguments
    ///
    /// * `input` - The input value to pass to the runnable
    /// * `config` - Optional runnable configuration
    ///
    /// # Returns
    ///
    /// The output from the runnable
    ///
    /// # Retry Behavior (M-196)
    ///
    /// This method automatically retries on transient network errors using
    /// the configured retry policy. Non-retryable errors (4xx client errors,
    /// authentication failures) are returned immediately.
    pub async fn invoke(&self, input: Value, config: Option<RunnableConfig>) -> Result<Value> {
        let url = self
            .base_url
            .join("invoke")
            .map_err(|e| LangServeError::InternalError(format!("Failed to construct URL: {e}")))?;

        let client = self.client.clone();
        let url_clone = url.clone();

        let invoke_response = with_retry(&self.retry_policy, || {
            let client = client.clone();
            let url = url_clone.clone();
            let request = InvokeRequest {
                input: input.clone(),
                config: config.clone(),
                kwargs: None,
            };
            async move {
                let response = client
                    .post(url)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_connect() || e.is_timeout() {
                            DashFlowError::network(format!("Request failed: {e}"))
                        } else {
                            DashFlowError::api(format!("Request failed: {e}"))
                        }
                    })?;

                // Check status code - 5xx errors are retryable, 4xx are not
                let status = response.status();
                if !status.is_success() {
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return if status.is_server_error() || status.as_u16() == 429 {
                        // Server errors and rate limits are retryable
                        if status.as_u16() == 429 {
                            Err(DashFlowError::rate_limit(format!(
                                "Rate limited: {error_text}"
                            )))
                        } else {
                            Err(DashFlowError::network(format!(
                                "Server error {status}: {error_text}"
                            )))
                        }
                    } else {
                        // Client errors are not retryable
                        Err(DashFlowError::api(format!(
                            "Server returned error {status}: {error_text}"
                        )))
                    };
                }

                response.json::<InvokeResponse>().await.map_err(|e| {
                    DashFlowError::api(format!("Failed to deserialize response: {e}"))
                })
            }
        })
        .await
        .map_err(|e| LangServeError::ExecutionError(e.to_string()))?;

        Ok(invoke_response.output)
    }

    /// Batch invoke the remote runnable with multiple inputs
    ///
    /// # Arguments
    ///
    /// * `inputs` - Vector of input values to pass to the runnable
    /// * `config` - Optional runnable configuration (applied to all inputs)
    ///
    /// # Returns
    ///
    /// Vector of outputs from the runnable
    ///
    /// # Retry Behavior (M-196)
    ///
    /// This method automatically retries on transient network errors using
    /// the configured retry policy. Non-retryable errors (4xx client errors,
    /// authentication failures) are returned immediately.
    pub async fn batch(
        &self,
        inputs: Vec<Value>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Value>> {
        let url = self
            .base_url
            .join("batch")
            .map_err(|e| LangServeError::InternalError(format!("Failed to construct URL: {e}")))?;

        let client = self.client.clone();
        let url_clone = url.clone();

        let batch_response = with_retry(&self.retry_policy, || {
            let client = client.clone();
            let url = url_clone.clone();
            let request = BatchRequest {
                inputs: inputs.clone(),
                config: config.clone(),
                configs: None,
                kwargs: None,
            };
            async move {
                let response = client
                    .post(url)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_connect() || e.is_timeout() {
                            DashFlowError::network(format!("Request failed: {e}"))
                        } else {
                            DashFlowError::api(format!("Request failed: {e}"))
                        }
                    })?;

                // Check status code - 5xx errors are retryable, 4xx are not
                let status = response.status();
                if !status.is_success() {
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return if status.is_server_error() || status.as_u16() == 429 {
                        if status.as_u16() == 429 {
                            Err(DashFlowError::rate_limit(format!(
                                "Rate limited: {error_text}"
                            )))
                        } else {
                            Err(DashFlowError::network(format!(
                                "Server error {status}: {error_text}"
                            )))
                        }
                    } else {
                        Err(DashFlowError::api(format!(
                            "Server returned error {status}: {error_text}"
                        )))
                    };
                }

                response.json::<BatchResponse>().await.map_err(|e| {
                    DashFlowError::api(format!("Failed to deserialize response: {e}"))
                })
            }
        })
        .await
        .map_err(|e| LangServeError::ExecutionError(e.to_string()))?;

        Ok(batch_response.output)
    }

    /// Stream outputs from the remote runnable
    ///
    /// # Arguments
    ///
    /// * `input` - The input value to pass to the runnable
    /// * `config` - Optional runnable configuration
    ///
    /// # Returns
    ///
    /// A stream of output values
    pub async fn stream(
        &self,
        input: Value,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        let request = StreamRequest {
            input,
            config,
            kwargs: None,
        };

        let url = self
            .base_url
            .join("stream")
            .map_err(|e| LangServeError::InternalError(format!("Failed to construct URL: {e}")))?;

        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LangServeError::ExecutionError(format!("Request failed: {e}")))?;

        // Check status code
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LangServeError::ExecutionError(format!(
                "Server returned error {status}: {error_text}"
            )));
        }

        // Parse SSE stream
        let mut event_stream = response.bytes_stream().eventsource();

        let output_stream = stream! {
            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        // Parse the event type and data
                        let event_type = event.event.as_str();

                        match event_type {
                            "" | "data" => {
                                // Default event type or explicit "data" event
                                // Parse the data as JSON
                                match serde_json::from_str::<Value>(&event.data) {
                                    Ok(data) => yield Ok(data),
                                    Err(e) => {
                                        yield Err(LangServeError::SerializationError(e));
                                        return;
                                    }
                                }
                            }
                            "error" => {
                                // Server sent an error event
                                yield Err(LangServeError::StreamingError(format!(
                                    "Server error: {}",
                                    event.data
                                )));
                                return;
                            }
                            "end" => {
                                // Stream complete
                                return;
                            }
                            "metadata" => {
                                // Metadata event, skip for now
                                continue;
                            }
                            _ => {
                                // Unknown event type, skip
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(LangServeError::StreamingError(format!(
                            "Stream error: {e}"
                        )));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    /// Get the input schema from the remote runnable
    pub async fn input_schema(&self) -> Result<Value> {
        self.get_schema("input_schema").await
    }

    /// Get the output schema from the remote runnable
    pub async fn output_schema(&self) -> Result<Value> {
        self.get_schema("output_schema").await
    }

    /// Get the config schema from the remote runnable
    pub async fn config_schema(&self) -> Result<Value> {
        self.get_schema("config_schema").await
    }

    /// Helper method to fetch a schema endpoint
    async fn get_schema(&self, endpoint: &str) -> Result<Value> {
        let url = self
            .base_url
            .join(endpoint)
            .map_err(|e| LangServeError::InternalError(format!("Failed to construct URL: {e}")))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| LangServeError::ExecutionError(format!("Request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LangServeError::SchemaError(format!(
                "Failed to fetch schema: {status} - {error_text}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| LangServeError::SchemaError(format!("Failed to deserialize schema: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_runnable_new() {
        let remote = RemoteRunnable::new("http://localhost:8000/my_runnable");
        assert!(remote.is_ok());

        let remote = remote.unwrap();
        assert_eq!(
            remote.base_url.as_str(),
            "http://localhost:8000/my_runnable/"
        );
    }

    #[test]
    fn test_remote_runnable_trailing_slash() {
        let remote = RemoteRunnable::new("http://localhost:8000/my_runnable/");
        assert!(remote.is_ok());

        let remote = remote.unwrap();
        assert_eq!(
            remote.base_url.as_str(),
            "http://localhost:8000/my_runnable/"
        );
    }

    #[test]
    fn test_remote_runnable_invalid_url() {
        let remote = RemoteRunnable::new("not a valid url");
        assert!(remote.is_err());
    }

    #[test]
    fn test_remote_runnable_with_timeout() {
        let remote = RemoteRunnable::with_timeout("http://localhost:8000/my_runnable", 30);
        assert!(remote.is_ok());

        let remote = remote.unwrap();
        assert_eq!(remote.timeout, Some(30));
    }

    // ==================== URL Handling Tests ====================

    #[test]
    fn test_remote_runnable_various_urls() {
        let urls = vec![
            "http://localhost:8000/runnable",
            "http://localhost:8000/runnable/",
            "https://api.example.com/v1/runnable",
            "http://127.0.0.1:3000/api",
        ];
        for url in urls {
            let remote = RemoteRunnable::new(url);
            assert!(remote.is_ok(), "Failed for URL: {}", url);
            assert!(remote.unwrap().base_url.as_str().ends_with('/'));
        }
    }

    #[test]
    fn test_remote_runnable_url_with_path() {
        let remote = RemoteRunnable::new("http://localhost:8000/api/v1/my_runnable").unwrap();
        assert_eq!(
            remote.base_url.as_str(),
            "http://localhost:8000/api/v1/my_runnable/"
        );
    }

    #[test]
    fn test_remote_runnable_url_with_port() {
        let remote = RemoteRunnable::new("http://localhost:3000/runnable").unwrap();
        assert!(remote.base_url.port() == Some(3000));
    }

    #[test]
    fn test_remote_runnable_https_url() {
        let remote = RemoteRunnable::new("https://secure.example.com/runnable").unwrap();
        assert_eq!(remote.base_url.scheme(), "https");
    }

    #[test]
    fn test_remote_runnable_invalid_urls() {
        let invalid_urls = vec![
            "",
            "not-a-url",
            "://missing-scheme",
            "http://",
            "ftp://unsupported-protocol.com/run",
        ];
        for url in invalid_urls {
            let result = RemoteRunnable::new(url);
            // Some may succeed (like ftp), but empty and invalid should fail
            if url.is_empty() || url == "not-a-url" || url == "://missing-scheme" || url == "http://"
            {
                assert!(result.is_err(), "Should fail for URL: {}", url);
            }
        }
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_remote_runnable_with_retry_policy() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable")
            .unwrap()
            .with_retry_policy(RetryPolicy::no_retry());
        // Just verify it compiles and runs
        assert!(remote.base_url.as_str().contains("localhost"));
    }

    #[test]
    fn test_remote_runnable_with_client() {
        let client = Client::new();
        let remote = RemoteRunnable::with_client("http://localhost:8000/runnable", client);
        assert!(remote.is_ok());
    }

    #[test]
    fn test_remote_runnable_with_client_invalid_url() {
        let client = Client::new();
        let remote = RemoteRunnable::with_client("not-a-url", client);
        assert!(remote.is_err());
    }

    #[test]
    fn test_remote_runnable_with_timeout_invalid_url() {
        let remote = RemoteRunnable::with_timeout("not-a-url", 30);
        assert!(remote.is_err());
    }

    #[test]
    fn test_remote_runnable_with_timeout_zero() {
        let remote = RemoteRunnable::with_timeout("http://localhost:8000/runnable", 0);
        assert!(remote.is_ok());
        assert_eq!(remote.unwrap().timeout, Some(0));
    }

    #[test]
    fn test_remote_runnable_with_timeout_large() {
        let remote = RemoteRunnable::with_timeout("http://localhost:8000/runnable", 3600);
        assert!(remote.is_ok());
        assert_eq!(remote.unwrap().timeout, Some(3600));
    }

    // ==================== Debug and Clone Tests ====================

    #[test]
    fn test_remote_runnable_debug() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let debug = format!("{:?}", remote);
        assert!(debug.contains("RemoteRunnable"));
        assert!(debug.contains("localhost"));
    }

    #[test]
    fn test_remote_runnable_clone() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let cloned = remote.clone();
        assert_eq!(remote.base_url, cloned.base_url);
        assert_eq!(remote.timeout, cloned.timeout);
    }

    #[test]
    fn test_remote_runnable_clone_with_timeout() {
        let remote = RemoteRunnable::with_timeout("http://localhost:8000/runnable", 60).unwrap();
        let cloned = remote.clone();
        assert_eq!(cloned.timeout, Some(60));
    }

    // ==================== URL Construction Tests ====================

    #[test]
    fn test_url_join_invoke() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let invoke_url = remote.base_url.join("invoke").unwrap();
        assert_eq!(invoke_url.as_str(), "http://localhost:8000/runnable/invoke");
    }

    #[test]
    fn test_url_join_batch() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let batch_url = remote.base_url.join("batch").unwrap();
        assert_eq!(batch_url.as_str(), "http://localhost:8000/runnable/batch");
    }

    #[test]
    fn test_url_join_stream() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let stream_url = remote.base_url.join("stream").unwrap();
        assert_eq!(stream_url.as_str(), "http://localhost:8000/runnable/stream");
    }

    #[test]
    fn test_url_join_input_schema() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let schema_url = remote.base_url.join("input_schema").unwrap();
        assert_eq!(
            schema_url.as_str(),
            "http://localhost:8000/runnable/input_schema"
        );
    }

    #[test]
    fn test_url_join_output_schema() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let schema_url = remote.base_url.join("output_schema").unwrap();
        assert_eq!(
            schema_url.as_str(),
            "http://localhost:8000/runnable/output_schema"
        );
    }

    #[test]
    fn test_url_join_config_schema() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        let schema_url = remote.base_url.join("config_schema").unwrap();
        assert_eq!(
            schema_url.as_str(),
            "http://localhost:8000/runnable/config_schema"
        );
    }

    // ==================== Default State Tests ====================

    #[test]
    fn test_remote_runnable_default_timeout() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        assert!(remote.timeout.is_none());
    }

    #[test]
    fn test_remote_runnable_has_client() {
        let remote = RemoteRunnable::new("http://localhost:8000/runnable").unwrap();
        // Client exists (we can't easily inspect it, but the struct has it)
        let debug = format!("{:?}", remote);
        assert!(debug.contains("client"));
    }
}
