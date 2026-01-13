//! Remote node client implementation
//!
//! # Retry Support (M-197)
//!
//! The client uses `dashflow::core::retry::with_retry` for proper retry logic that
//! respects error retryability. Only transient errors (transport failures, timeouts,
//! gRPC Unavailable/DeadlineExceeded/ResourceExhausted/Aborted codes, and server
//! responses marked as retryable) are retried. Non-retryable errors (serialization,
//! configuration, server responses not marked retryable) fail immediately.
//!
//! Use `with_retry_policy()` to configure retry behavior:
//!
//! ```rust,ignore
//! use dashflow_remote_node::RemoteNode;
//! use dashflow::core::retry::RetryPolicy;
//!
//! let node = RemoteNode::new("my_node")
//!     .with_endpoint("http://compute-server:50051")
//!     .with_retry_policy(RetryPolicy::default_jitter(5)); // 5 retries with jitter
//! ```

use async_trait::async_trait;
use dashflow::constants::LONG_TIMEOUT;
use dashflow::core::error::Error as DashFlowError;
use dashflow::core::retry::{with_retry, RetryPolicy};
use dashflow::{error::Result as DashFlowResult, node::Node, state::GraphState};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::proto::{
    remote_node_service_client::RemoteNodeServiceClient, ExecuteNodeRequest, HealthRequest,
    SerializationFormat,
};

/// Configuration for a remote node
#[derive(Debug, Clone)]
pub struct RemoteNodeConfig {
    /// gRPC endpoint (e.g., "<http://compute-server:50051>")
    pub endpoint: String,
    /// Execution timeout
    pub timeout: Duration,
    /// Retry policy for transient network errors (M-197)
    ///
    /// Uses `dashflow::core::retry::with_retry` which respects error retryability:
    /// - Transport errors, timeouts, and retryable gRPC codes are retried
    /// - Serialization, configuration, and non-retryable server errors fail immediately
    pub retry_policy: RetryPolicy,
    /// Serialization format
    pub format: SerializationFormat,
    /// Enable health checks before execution
    pub health_check: bool,
    /// Request ID prefix for tracing
    pub request_id_prefix: String,
    /// Thread ID for execution context (optional)
    pub thread_id: Option<String>,
}

impl Default for RemoteNodeConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50051".to_string(),
            timeout: LONG_TIMEOUT, // 60 seconds for remote execution
            retry_policy: RetryPolicy::default(), // Uses exponential jitter (M-195)
            format: SerializationFormat::Json,
            health_check: true,
            request_id_prefix: "remote".to_string(),
            thread_id: None,
        }
    }
}

/// A node that executes on a remote server via gRPC
///
/// `RemoteNode` implements the Node trait and transparently forwards execution
/// to a remote gRPC server. It handles serialization, networking, retries,
/// and error translation.
///
/// # Type Parameters
///
/// - `S`: Graph state type (must be Serialize + `DeserializeOwned`)
///
/// # Retry Behavior (M-197)
///
/// Uses `dashflow::core::retry::with_retry` which respects error retryability:
/// - **Retryable**: Transport errors, timeouts, gRPC Unavailable/DeadlineExceeded/
///   ResourceExhausted/Aborted codes, and server responses with `retryable: true`
/// - **Not retryable**: Serialization errors, configuration errors, server responses
///   with `retryable: false`, and other gRPC status codes
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retry::RetryPolicy;
///
/// let remote_node = RemoteNode::new("heavy_computation")
///     .with_endpoint("http://compute-server:50051")
///     .with_timeout(Duration::from_secs(300))
///     .with_retry_policy(RetryPolicy::default_jitter(5)); // 5 retries
///
/// graph.add_node("compute", remote_node);
/// ```
pub struct RemoteNode<S>
where
    S: GraphState + Serialize + DeserializeOwned,
{
    /// Node name on the remote server
    node_name: String,
    /// Configuration
    config: RemoteNodeConfig,
    /// gRPC client (lazy initialized)
    client: Arc<tokio::sync::Mutex<Option<RemoteNodeServiceClient<Channel>>>>,
    /// Phantom data for state type
    _phantom: PhantomData<S>,
}

impl<S> RemoteNode<S>
where
    S: GraphState + Serialize + DeserializeOwned,
{
    /// Create a new remote node
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the node on the remote server
    pub fn new(node_name: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            config: RemoteNodeConfig::default(),
            client: Arc::new(tokio::sync::Mutex::new(None)),
            _phantom: PhantomData,
        }
    }

    /// Set the gRPC endpoint
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    /// Set the execution timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the retry policy for transient network errors (M-197)
    ///
    /// By default, the client uses exponential backoff with jitter (3 retries).
    /// Use this method to customize retry behavior.
    ///
    /// # Retry Behavior
    ///
    /// The retry logic respects error retryability:
    /// - **Retryable**: Transport errors, timeouts, gRPC Unavailable/DeadlineExceeded/
    ///   ResourceExhausted/Aborted codes, and server responses with `retryable: true`
    /// - **Not retryable**: Serialization errors, configuration errors, server responses
    ///   with `retryable: false`, and other gRPC status codes
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_remote_node::RemoteNode;
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// let node = RemoteNode::new("my_node")
    ///     .with_retry_policy(RetryPolicy::exponential(5)); // 5 retries
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }

    /// Set the serialization format
    #[must_use]
    pub fn with_format(mut self, format: SerializationFormat) -> Self {
        self.config.format = format;
        self
    }

    /// Enable or disable health checks
    #[must_use]
    pub fn with_health_check(mut self, enabled: bool) -> Self {
        self.config.health_check = enabled;
        self
    }

    /// Set thread ID for execution context
    ///
    /// The thread ID is passed to the remote server for tracing and checkpoint isolation.
    /// This is useful when integrating with `DashFlow`'s checkpoint system.
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.config.thread_id = Some(thread_id.into());
        self
    }

    /// Get or create gRPC client
    async fn get_client(&self) -> Result<RemoteNodeServiceClient<Channel>> {
        let mut guard = self.client.lock().await;

        if let Some(client) = &*guard {
            return Ok(client.clone());
        }

        // Connect to remote server
        debug!("Connecting to remote node at {}", self.config.endpoint);
        let channel = Channel::from_shared(self.config.endpoint.clone())
            .map_err(|e| Error::Configuration(e.to_string()))?
            .connect()
            .await?;

        let client = RemoteNodeServiceClient::new(channel);

        *guard = Some(client.clone());
        Ok(client)
    }

    /// Perform health check (respects `config.health_check` setting)
    async fn health_check(&self) -> Result<()> {
        if !self.config.health_check {
            return Ok(());
        }

        self.check_health().await
    }

    /// Check remote node health
    ///
    /// This performs a health check regardless of the `health_check` config setting.
    /// Use this when you need to explicitly verify the remote node's availability.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the service is healthy and serving
    /// - `Err(Error::HealthCheckFailed)` if the service is not serving
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_remote_node::RemoteNode;
    ///
    /// let client: RemoteNode<MyState> = RemoteNode::new("my_node", "http://localhost:50051");
    ///
    /// // Explicit health check
    /// client.check_health().await?;
    /// println!("Remote node is healthy");
    /// ```
    pub async fn check_health(&self) -> Result<()> {
        let mut client = self.get_client().await?;

        let request = tonic::Request::new(HealthRequest {
            service: "RemoteNodeService".to_string(),
        });

        let response = client.health(request).await?;
        let health = response.into_inner();

        if health.status() == crate::proto::health_response::HealthStatus::Serving {
            debug!("Health check passed for {}", self.config.endpoint);
            Ok(())
        } else {
            let msg = format!("Service not serving: {}", health.message);
            Err(Error::HealthCheckFailed(msg))
        }
    }

    /// Serialize state
    fn serialize_state(&self, state: &S) -> Result<Vec<u8>> {
        match self.config.format {
            SerializationFormat::Json => {
                serde_json::to_vec(state).map_err(|e| Error::Serialization(format!("JSON: {e}")))
            }
            SerializationFormat::Bincode => {
                bincode::serialize(state).map_err(|e| Error::Serialization(format!("Bincode: {e}")))
            }
            _ => Err(Error::Configuration(format!(
                "Unsupported serialization format: {:?}",
                self.config.format
            ))),
        }
    }

    /// Deserialize state
    ///
    /// Note: Not used directly in execute_with_retry (deserialization happens inline
    /// within the retry closure to avoid borrow issues), but kept for tests and
    /// potential future use.
    #[allow(clippy::result_large_err)] // Error enum contains detailed context needed for debugging
    #[allow(dead_code)] // Test infrastructure: Used in tests; kept for symmetry with serialize_state
    fn deserialize_state(&self, bytes: &[u8]) -> Result<S> {
        match self.config.format {
            SerializationFormat::Json => serde_json::from_slice(bytes)
                .map_err(|e| Error::Deserialization(format!("JSON: {e}"))),
            SerializationFormat::Bincode => bincode::deserialize(bytes)
                .map_err(|e| Error::Deserialization(format!("Bincode: {e}"))),
            _ => Err(Error::Configuration(format!(
                "Unsupported serialization format: {:?}",
                self.config.format
            ))),
        }
    }

    /// Execute node with retry logic (M-197)
    ///
    /// Uses `dashflow::core::retry::with_retry` which respects error retryability:
    /// - Transport errors and retryable gRPC codes trigger retry
    /// - Server responses with `retryable: true` trigger retry
    /// - Non-retryable errors fail immediately without retry
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Mutex> cloned for retry closure
    async fn execute_with_retry(&self, state: S) -> Result<S> {
        // Health check (if enabled)
        self.health_check().await?;

        // Serialize state (non-retryable on failure)
        let state_bytes = self.serialize_state(&state)?;
        let state_type = std::any::type_name::<S>().to_string();

        // Generate request ID
        let request_id = format!("{}-{}", self.config.request_id_prefix, uuid::Uuid::new_v4());

        debug!(
            "Executing remote node '{}' (request_id: {})",
            self.node_name, request_id
        );

        // Execute with retry using DashFlow's retry logic (respects is_retryable)
        let result = with_retry(&self.config.retry_policy, || {
            let node_name = self.node_name.clone();
            let state_bytes = state_bytes.clone();
            let state_type = state_type.clone();
            let request_id = request_id.clone();
            let thread_id = self.config.thread_id.clone().unwrap_or_default();
            let format = self.config.format;
            let timeout = self.config.timeout;
            let client_arc = self.client.clone();
            let endpoint = self.config.endpoint.clone();

            async move {
                // Get or create gRPC client
                let mut guard = client_arc.lock().await;
                let client = if let Some(client) = &mut *guard {
                    client.clone()
                } else {
                    // Connect to remote server
                    debug!("Connecting to remote node at {}", endpoint);
                    let channel = Channel::from_shared(endpoint.clone())
                        .map_err(|e| DashFlowError::api(format!("Invalid endpoint: {e}")))?
                        .connect()
                        .await
                        .map_err(|e| {
                            // Transport errors are retryable
                            DashFlowError::network(format!("Connection failed: {e}"))
                        })?;
                    let new_client = RemoteNodeServiceClient::new(channel);
                    *guard = Some(new_client.clone());
                    new_client
                };
                drop(guard);

                let mut client = client;
                let request = tonic::Request::new(ExecuteNodeRequest {
                    node_name: node_name.clone(),
                    state: state_bytes,
                    format: format as i32,
                    state_type,
                    timeout_ms: timeout.as_millis() as u64,
                    request_id: request_id.clone(),
                    thread_id,
                    context: Default::default(),
                });

                // Execute with timeout
                let mut request_with_timeout = tonic::Request::new(request.into_inner());
                request_with_timeout.set_timeout(timeout);

                let response = client
                    .execute_node(request_with_timeout)
                    .await
                    .map_err(|status| {
                        // Map gRPC status codes to retryable/non-retryable errors
                        match status.code() {
                            tonic::Code::Unavailable
                            | tonic::Code::DeadlineExceeded
                            | tonic::Code::ResourceExhausted
                            | tonic::Code::Aborted => {
                                // These are transient errors, retryable
                                DashFlowError::network(format!("gRPC error: {status}"))
                            }
                            _ => {
                                // Other gRPC errors are not retryable
                                DashFlowError::api(format!("gRPC error: {status}"))
                            }
                        }
                    })?;

                let response = response.into_inner();

                // Handle response
                match response.result {
                    Some(crate::proto::execute_node_response::Result::Success(success)) => {
                        // Deserialize output state
                        match format {
                            SerializationFormat::Json => serde_json::from_slice(&success.state)
                                .map_err(|e| {
                                    // Deserialization errors are not retryable
                                    DashFlowError::api(format!("JSON deserialization failed: {e}"))
                                }),
                            SerializationFormat::Bincode => bincode::deserialize(&success.state)
                                .map_err(|e| {
                                    DashFlowError::api(format!(
                                        "Bincode deserialization failed: {e}"
                                    ))
                                }),
                            _ => Err(DashFlowError::api(format!(
                                "Unsupported serialization format: {format:?}"
                            ))),
                        }
                    }
                    Some(crate::proto::execute_node_response::Result::Error(error)) => {
                        let msg = format!(
                            "Remote execution failed (code: {:?}): {}",
                            error.code(),
                            error.message
                        );

                        // Respect the server's retryable flag (M-197)
                        if error.retryable {
                            warn!("{} (retryable)", msg);
                            Err(DashFlowError::network(msg))
                        } else {
                            error!("{} (not retryable)", msg);
                            Err(DashFlowError::api(msg))
                        }
                    }
                    None => Err(DashFlowError::api("Missing result field in response")),
                }
            }
        })
        .await;

        match result {
            Ok(state) => {
                info!("Remote node '{}' execution succeeded", self.node_name);
                Ok(state)
            }
            Err(e) => {
                error!("Remote node '{}' execution failed: {}", self.node_name, e);
                // Wrap core error in the dashflow::error::Error type
                Err(Error::DashFlow(dashflow::error::Error::Core(e)))
            }
        }
    }
}

#[async_trait]
impl<S> Node<S> for RemoteNode<S>
where
    S: GraphState + Serialize + DeserializeOwned,
{
    async fn execute(&self, state: S) -> DashFlowResult<S> {
        self.execute_with_retry(state)
            .await
            .map_err(|e| dashflow::error::Error::NodeExecution {
                node: self.node_name.clone(),
                source: Box::new(e),
            })
    }

    fn name(&self) -> String {
        format!("RemoteNode({}@{})", self.node_name, self.config.endpoint)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S> std::fmt::Debug for RemoteNode<S>
where
    S: GraphState + Serialize + DeserializeOwned,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteNode")
            .field("node_name", &self.node_name)
            .field("endpoint", &self.config.endpoint)
            .field("timeout", &self.config.timeout)
            .field("retry_policy", &self.config.retry_policy)
            .finish()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    // GraphState is already implemented via blanket impl

    #[test]
    fn test_remote_node_creation() {
        let node = RemoteNode::<TestState>::new("test_node")
            .with_endpoint("http://localhost:50051")
            .with_timeout(Duration::from_secs(30))
            .with_retry_policy(RetryPolicy::exponential(5));

        assert_eq!(node.node_name, "test_node");
        assert_eq!(node.config.endpoint, "http://localhost:50051");
        assert_eq!(node.config.timeout, Duration::from_secs(30));
        assert_eq!(node.config.retry_policy.max_retries, 5);
    }

    #[test]
    fn test_serialize_deserialize_json() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);

        let state = TestState { value: 42 };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_bincode() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Bincode);

        let state = TestState { value: 42 };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_thread_id_configuration() {
        // Test with thread_id
        let node_with_thread =
            RemoteNode::<TestState>::new("test_node").with_thread_id("thread-123");

        assert_eq!(
            node_with_thread.config.thread_id,
            Some("thread-123".to_string())
        );

        // Test without thread_id (default)
        let node_without_thread = RemoteNode::<TestState>::new("test_node");
        assert_eq!(node_without_thread.config.thread_id, None);

        // Test chaining with other builders
        let node_chained = RemoteNode::<TestState>::new("test_node")
            .with_endpoint("http://server:50051")
            .with_thread_id("session-456")
            .with_timeout(Duration::from_secs(120));

        assert_eq!(
            node_chained.config.thread_id,
            Some("session-456".to_string())
        );
        assert_eq!(node_chained.config.endpoint, "http://server:50051");
        assert_eq!(node_chained.config.timeout, Duration::from_secs(120));
    }

    // ============================================
    // RemoteNodeConfig tests
    // ============================================

    #[test]
    fn test_remote_node_config_default() {
        let config = RemoteNodeConfig::default();
        assert_eq!(config.endpoint, "http://localhost:50051");
        assert!(config.health_check);
        assert_eq!(config.request_id_prefix, "remote");
        assert!(config.thread_id.is_none());
    }

    #[test]
    fn test_remote_node_config_debug() {
        let config = RemoteNodeConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("localhost:50051"));
        assert!(debug.contains("health_check"));
    }

    #[test]
    fn test_remote_node_config_clone() {
        let config = RemoteNodeConfig {
            endpoint: "http://custom:9999".to_string(),
            timeout: Duration::from_secs(120),
            retry_policy: RetryPolicy::exponential(5),
            format: SerializationFormat::Bincode,
            health_check: false,
            request_id_prefix: "custom".to_string(),
            thread_id: Some("thread-123".to_string()),
        };
        let cloned = config.clone();
        assert_eq!(cloned.endpoint, config.endpoint);
        assert_eq!(cloned.timeout, config.timeout);
        assert_eq!(cloned.health_check, config.health_check);
        assert_eq!(cloned.thread_id, config.thread_id);
    }

    // ============================================
    // RemoteNode builder tests
    // ============================================

    #[test]
    fn test_with_format_json() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        assert!(matches!(node.config.format, SerializationFormat::Json));
    }

    #[test]
    fn test_with_format_bincode() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Bincode);
        assert!(matches!(node.config.format, SerializationFormat::Bincode));
    }

    #[test]
    fn test_with_health_check_enabled() {
        let node = RemoteNode::<TestState>::new("test").with_health_check(true);
        assert!(node.config.health_check);
    }

    #[test]
    fn test_with_health_check_disabled() {
        let node = RemoteNode::<TestState>::new("test").with_health_check(false);
        assert!(!node.config.health_check);
    }

    #[test]
    fn test_with_endpoint_various_formats() {
        // Standard HTTP
        let node = RemoteNode::<TestState>::new("test").with_endpoint("http://localhost:50051");
        assert_eq!(node.config.endpoint, "http://localhost:50051");

        // HTTPS
        let node = RemoteNode::<TestState>::new("test").with_endpoint("https://secure.example.com:443");
        assert_eq!(node.config.endpoint, "https://secure.example.com:443");

        // IPv4
        let node = RemoteNode::<TestState>::new("test").with_endpoint("http://192.168.1.100:9000");
        assert_eq!(node.config.endpoint, "http://192.168.1.100:9000");

        // IPv6
        let node = RemoteNode::<TestState>::new("test").with_endpoint("http://[::1]:50051");
        assert_eq!(node.config.endpoint, "http://[::1]:50051");
    }

    #[test]
    fn test_with_timeout_various_durations() {
        // Short timeout
        let node = RemoteNode::<TestState>::new("test").with_timeout(Duration::from_millis(100));
        assert_eq!(node.config.timeout, Duration::from_millis(100));

        // Long timeout
        let node = RemoteNode::<TestState>::new("test").with_timeout(Duration::from_secs(3600));
        assert_eq!(node.config.timeout, Duration::from_secs(3600));

        // Zero timeout
        let node = RemoteNode::<TestState>::new("test").with_timeout(Duration::ZERO);
        assert_eq!(node.config.timeout, Duration::ZERO);
    }

    #[test]
    fn test_with_retry_policy_various_configs() {
        // Zero retries
        let node = RemoteNode::<TestState>::new("test").with_retry_policy(RetryPolicy::exponential(0));
        assert_eq!(node.config.retry_policy.max_retries, 0);

        // Many retries
        let node = RemoteNode::<TestState>::new("test").with_retry_policy(RetryPolicy::exponential(10));
        assert_eq!(node.config.retry_policy.max_retries, 10);
    }

    #[test]
    fn test_builder_chain_all_methods() {
        let node = RemoteNode::<TestState>::new("complex_node")
            .with_endpoint("http://compute:8080")
            .with_timeout(Duration::from_secs(300))
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_format(SerializationFormat::Bincode)
            .with_health_check(false)
            .with_thread_id("thread-xyz");

        assert_eq!(node.node_name, "complex_node");
        assert_eq!(node.config.endpoint, "http://compute:8080");
        assert_eq!(node.config.timeout, Duration::from_secs(300));
        assert_eq!(node.config.retry_policy.max_retries, 5);
        assert!(matches!(node.config.format, SerializationFormat::Bincode));
        assert!(!node.config.health_check);
        assert_eq!(node.config.thread_id, Some("thread-xyz".to_string()));
    }

    // ============================================
    // Serialization edge case tests
    // ============================================

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct ComplexState {
        id: String,
        values: Vec<i32>,
        nested: Option<Box<NestedState>>,
    }

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct NestedState {
        name: String,
        count: u64,
    }

    #[test]
    fn test_serialize_empty_state() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let state = TestState { value: 0 };
        let serialized = node.serialize_state(&state).unwrap();
        assert!(!serialized.is_empty());
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_negative_values() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let state = TestState { value: -9999 };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_max_i32() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let state = TestState { value: i32::MAX };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_min_i32() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let state = TestState { value: i32::MIN };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_complex_nested_state_json() {
        let node = RemoteNode::<ComplexState>::new("test").with_format(SerializationFormat::Json);
        let state = ComplexState {
            id: "test-123".to_string(),
            values: vec![1, 2, 3, -4, 5],
            nested: Some(Box::new(NestedState {
                name: "inner".to_string(),
                count: 42,
            })),
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_complex_nested_state_bincode() {
        let node = RemoteNode::<ComplexState>::new("test").with_format(SerializationFormat::Bincode);
        let state = ComplexState {
            id: "test-456".to_string(),
            values: vec![100, 200, 300],
            nested: None,
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct UnicodeState {
        text: String,
    }

    #[test]
    fn test_serialize_unicode_content_json() {
        let node = RemoteNode::<UnicodeState>::new("test").with_format(SerializationFormat::Json);
        let state = UnicodeState {
            text: "日本語 中文 한국어 العربية 🦀🔧".to_string(),
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_unicode_content_bincode() {
        let node = RemoteNode::<UnicodeState>::new("test").with_format(SerializationFormat::Bincode);
        let state = UnicodeState {
            text: "émojis: 👨‍👩‍👧‍👦 family".to_string(),
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_empty_string() {
        let node = RemoteNode::<UnicodeState>::new("test").with_format(SerializationFormat::Json);
        let state = UnicodeState {
            text: String::new(),
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct LargeState {
        data: Vec<u8>,
    }

    #[test]
    fn test_serialize_large_data_json() {
        let node = RemoteNode::<LargeState>::new("test").with_format(SerializationFormat::Json);
        let state = LargeState {
            data: vec![0u8; 10000], // 10KB
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_serialize_large_data_bincode() {
        let node = RemoteNode::<LargeState>::new("test").with_format(SerializationFormat::Bincode);
        let state = LargeState {
            data: vec![255u8; 10000], // 10KB
        };
        let serialized = node.serialize_state(&state).unwrap();
        let deserialized = node.deserialize_state(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    // ============================================
    // Deserialization error tests
    // ============================================

    #[test]
    fn test_deserialize_invalid_json() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let result = node.deserialize_state(b"not valid json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn test_deserialize_truncated_bincode() {
        let node = RemoteNode::<ComplexState>::new("test").with_format(SerializationFormat::Bincode);
        // Truncated bincode (valid start but incomplete)
        let result = node.deserialize_state(&[1, 0, 0, 0]); // Just length prefix, no data
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Bincode"));
    }

    #[test]
    fn test_deserialize_empty_bytes_json() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        let result = node.deserialize_state(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_empty_bytes_bincode() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Bincode);
        let result = node.deserialize_state(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_wrong_type_json() {
        let node = RemoteNode::<TestState>::new("test").with_format(SerializationFormat::Json);
        // Valid JSON but wrong structure
        let result = node.deserialize_state(b"{\"wrong_field\": \"value\"}");
        assert!(result.is_err());
    }

    // ============================================
    // Node name tests
    // ============================================

    #[test]
    fn test_node_name_simple() {
        let node = RemoteNode::<TestState>::new("simple");
        assert_eq!(node.node_name, "simple");
    }

    #[test]
    fn test_node_name_with_underscores() {
        let node = RemoteNode::<TestState>::new("compute_heavy_node");
        assert_eq!(node.node_name, "compute_heavy_node");
    }

    #[test]
    fn test_node_name_with_dashes() {
        let node = RemoteNode::<TestState>::new("compute-heavy-node");
        assert_eq!(node.node_name, "compute-heavy-node");
    }

    #[test]
    fn test_node_name_with_numbers() {
        let node = RemoteNode::<TestState>::new("node_v2_0");
        assert_eq!(node.node_name, "node_v2_0");
    }

    #[test]
    fn test_node_name_unicode() {
        let node = RemoteNode::<TestState>::new("節點_测试");
        assert_eq!(node.node_name, "節點_测试");
    }

    #[test]
    fn test_node_name_from_string() {
        let name = String::from("dynamic_name");
        let node = RemoteNode::<TestState>::new(name);
        assert_eq!(node.node_name, "dynamic_name");
    }

    // ============================================
    // Bincode vs JSON comparison tests
    // ============================================

    #[test]
    fn test_bincode_smaller_than_json_for_numeric_data() {
        let json_node = RemoteNode::<LargeState>::new("test").with_format(SerializationFormat::Json);
        let bincode_node = RemoteNode::<LargeState>::new("test").with_format(SerializationFormat::Bincode);

        let state = LargeState {
            data: vec![42u8; 1000],
        };

        let json_bytes = json_node.serialize_state(&state).unwrap();
        let bincode_bytes = bincode_node.serialize_state(&state).unwrap();

        // Bincode should be more compact for binary data
        assert!(bincode_bytes.len() < json_bytes.len());
    }

    // ============================================
    // Default values verification tests
    // ============================================

    #[test]
    fn test_default_node_values() {
        let node = RemoteNode::<TestState>::new("test");

        // Verify all defaults match RemoteNodeConfig::default()
        assert_eq!(node.config.endpoint, "http://localhost:50051");
        assert!(node.config.health_check);
        assert_eq!(node.config.request_id_prefix, "remote");
        assert!(node.config.thread_id.is_none());
        assert!(matches!(node.config.format, SerializationFormat::Json));
    }
}
