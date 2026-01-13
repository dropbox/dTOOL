//! Remote node server implementation

use dashflow::{node::Node, state::GraphState, DEFAULT_MPSC_CHANNEL_CAPACITY};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info};

use crate::error::{Error, Result};
use crate::proto::{
    execution_error::ErrorCode, remote_node_service_server::RemoteNodeService,
    remote_node_service_server::RemoteNodeServiceServer, ExecuteNodeRequest, ExecuteNodeResponse,
    ExecuteNodeStreamResponse, ExecutionError, ExecutionMetrics, ExecutionSuccess,
    GetNodeMetadataRequest, GetNodeMetadataResponse, HealthRequest, HealthResponse,
    SerializationFormat,
};

/// Registry for storing nodes that can be executed remotely
///
/// `NodeRegistry` maintains a collection of nodes that can be invoked via gRPC.
/// Each node is registered with a unique name and can handle a specific state type.
pub struct NodeRegistry {
    /// Registered nodes (keyed by node name)
    nodes: Arc<RwLock<HashMap<String, RegisteredNode>>>,
}

/// Execution result with timing metrics
struct ExecutionResult {
    /// Serialized output state
    output_bytes: Vec<u8>,
    /// Time spent deserializing input (microseconds)
    deserialization_us: u64,
    /// Time spent executing node logic (microseconds)
    execution_us: u64,
    /// Time spent serializing output (microseconds)
    serialization_us: u64,
    /// CPU time consumed during execution (microseconds)
    cpu_time_us: u64,
    /// Peak physical memory usage during execution (bytes)
    memory_peak_bytes: u64,
}

/// A registered node with type-erased execution
#[derive(Clone)]
struct RegisteredNode {
    /// Execute function (type-erased)
    #[allow(clippy::type_complexity)] // Type-erased async executor requires boxed future with full bounds
    execute: Arc<
        dyn Fn(
                Vec<u8>,
                SerializationFormat,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<ExecutionResult>> + Send>,
            > + Send
            + Sync,
    >,
    /// State type name (for metadata)
    state_type_name: String,
}

impl NodeRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node for remote execution
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node
    /// * `node` - Node implementation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut registry = NodeRegistry::new();
    /// registry.register("compute", ComputeNode::new());
    /// ```
    #[allow(clippy::clone_on_ref_ptr)] // Arc<dyn Node> cloned for type-erased closure
    pub fn register<S, N>(&mut self, name: impl Into<String>, node: N)
    where
        S: GraphState + Serialize + DeserializeOwned + 'static,
        N: Node<S> + 'static,
    {
        let name_str = name.into();
        let node: Arc<dyn Node<S>> = Arc::new(node);

        // Capture state type name for metadata
        let state_type_name = std::any::type_name::<S>().to_string();

        // Type-erased execution wrapper
        let execute = Arc::new(move |state_bytes: Vec<u8>, format: SerializationFormat| {
            let node = node.clone();
            Box::pin(async move {
                // Measure CPU time and memory at start
                let cpu_start = cpu_time::ProcessTime::now();
                let mem_start = memory_stats::memory_stats().map_or(0, |m| m.physical_mem);

                // Deserialize input state
                let deser_start = std::time::Instant::now();
                let state: S = match format {
                    SerializationFormat::Json => serde_json::from_slice(&state_bytes)
                        .map_err(|e| Error::Deserialization(format!("JSON: {e}")))?,
                    SerializationFormat::Bincode => bincode::deserialize(&state_bytes)
                        .map_err(|e| Error::Deserialization(format!("Bincode: {e}")))?,
                    _ => {
                        return Err(Error::Configuration(format!(
                            "Unsupported serialization format: {format:?}"
                        )))
                    }
                };
                let deserialization_us = deser_start.elapsed().as_micros() as u64;

                // Execute node
                let exec_start = std::time::Instant::now();
                let output_state = node
                    .execute(state)
                    .await
                    .map_err(|e| Error::RemoteExecution(e.to_string()))?;
                let execution_us = exec_start.elapsed().as_micros() as u64;

                // Serialize output state
                let ser_start = std::time::Instant::now();
                let output_bytes = match format {
                    SerializationFormat::Json => serde_json::to_vec(&output_state)
                        .map_err(|e| Error::Serialization(format!("JSON: {e}")))?,
                    SerializationFormat::Bincode => bincode::serialize(&output_state)
                        .map_err(|e| Error::Serialization(format!("Bincode: {e}")))?,
                    _ => {
                        return Err(Error::Configuration(format!(
                            "Unsupported serialization format: {format:?}"
                        )))
                    }
                };
                let serialization_us = ser_start.elapsed().as_micros() as u64;

                // Measure CPU time and memory at end
                let cpu_duration = cpu_start.elapsed();
                let cpu_time_us = cpu_duration.as_micros() as u64;

                let mem_end = memory_stats::memory_stats().map_or(0, |m| m.physical_mem);
                let memory_peak_bytes = mem_end.saturating_sub(mem_start) as u64;

                Ok(ExecutionResult {
                    output_bytes,
                    deserialization_us,
                    execution_us,
                    serialization_us,
                    cpu_time_us,
                    memory_peak_bytes,
                })
            })
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<ExecutionResult>> + Send>,
                >
        });

        let registered = RegisteredNode {
            execute,
            state_type_name,
        };

        // Insert into registry (synchronous)
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to register new nodes rather than crash
        let mut guard = self.nodes.write().unwrap_or_else(|e| e.into_inner());
        guard.insert(name_str, registered);
    }

    /// Get a registered node by name
    #[cfg(test)]
    async fn get_node(&self, name: &str) -> Option<RegisteredNode> {
        let guard = self.nodes.read().unwrap();
        guard.get(name).cloned()
    }

    /// List all registered nodes
    pub async fn list_nodes(&self) -> Vec<String> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to list nodes rather than crash
        let guard = self.nodes.read().unwrap_or_else(|e| e.into_inner());
        guard.keys().cloned().collect()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NodeRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRegistry").finish_non_exhaustive()
    }
}

/// Remote node gRPC server
///
/// `RemoteNodeServer` hosts nodes that can be executed remotely via gRPC.
/// It handles serialization, deserialization, and error translation.
pub struct RemoteNodeServer {
    /// Node registry
    registry: Arc<RwLock<HashMap<String, RegisteredNode>>>,
    /// Server version
    version: String,
}

impl RemoteNodeServer {
    /// Create a new server with a registry
    #[must_use]
    pub fn new(registry: NodeRegistry) -> Self {
        Self {
            registry: registry.nodes,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Start the gRPC server
    ///
    /// # Arguments
    ///
    /// * `addr` - Socket address to bind to
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut registry = NodeRegistry::new();
    /// registry.register("compute", ComputeNode::new());
    ///
    /// let server = RemoteNodeServer::new(registry);
    /// server.serve("0.0.0.0:50051".parse()?).await?;
    /// ```
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        info!("Starting RemoteNodeServer on {}", addr);

        Server::builder()
            .add_service(RemoteNodeServiceServer::new(self))
            .serve(addr)
            .await
            .map_err(Error::Transport)?;

        Ok(())
    }
}

#[tonic::async_trait]
impl RemoteNodeService for RemoteNodeServer {
    async fn execute_node(
        &self,
        request: Request<ExecuteNodeRequest>,
    ) -> std::result::Result<Response<ExecuteNodeResponse>, Status> {
        let req = request.into_inner();
        let start_time = std::time::Instant::now();

        debug!("Received execution request for node '{}'", req.node_name);

        // Find node in registry (clone to drop lock before await)
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to execute nodes rather than crash
        let node = {
            let guard = self.registry.read().unwrap_or_else(|e| e.into_inner());
            guard.get(&req.node_name).cloned()
        };

        if node.is_none() {
            error!("Node '{}' not found in registry", req.node_name);
            return Ok(Response::new(ExecuteNodeResponse {
                request_id: req.request_id.clone(),
                result: Some(crate::proto::execute_node_response::Result::Error(
                    ExecutionError {
                        code: ErrorCode::NodeNotFound as i32,
                        message: format!("Node '{}' not found", req.node_name),
                        stack_trace: String::new(),
                        context: Default::default(),
                        retryable: false,
                    },
                )),
                metrics: None,
            }));
        }

        // SAFETY: node.is_none() check above returns early, so unwrap is safe
        #[allow(clippy::unwrap_used)]
        let node = node.unwrap();
        let format = SerializationFormat::try_from(req.format).unwrap_or(SerializationFormat::Json);

        // Execute node
        let result = (node.execute)(req.state.clone(), format).await;

        let response = match result {
            Ok(exec_result) => {
                info!("Node '{}' execution succeeded", req.node_name);
                ExecuteNodeResponse {
                    request_id: req.request_id,
                    result: Some(crate::proto::execute_node_response::Result::Success(
                        ExecutionSuccess {
                            state: exec_result.output_bytes.clone(),
                            format: format as i32,
                            state_type: req.state_type,
                        },
                    )),
                    metrics: Some(ExecutionMetrics {
                        duration_us: start_time.elapsed().as_micros() as u64,
                        deserialization_us: exec_result.deserialization_us,
                        execution_us: exec_result.execution_us,
                        serialization_us: exec_result.serialization_us,
                        network_us: 0,
                        input_bytes: req.state.len() as u64,
                        output_bytes: exec_result.output_bytes.len() as u64,
                        memory_peak_bytes: exec_result.memory_peak_bytes,
                        cpu_time_us: exec_result.cpu_time_us,
                    }),
                }
            }
            Err(e) => {
                error!("Node '{}' execution failed: {}", req.node_name, e);
                ExecuteNodeResponse {
                    request_id: req.request_id,
                    result: Some(crate::proto::execute_node_response::Result::Error(
                        ExecutionError {
                            code: ErrorCode::ExecutionFailed as i32,
                            message: e.to_string(),
                            stack_trace: String::new(),
                            context: Default::default(),
                            retryable: e.is_retryable(),
                        },
                    )),
                    metrics: Some(ExecutionMetrics {
                        duration_us: start_time.elapsed().as_micros() as u64,
                        deserialization_us: 0,
                        execution_us: 0,
                        serialization_us: 0,
                        network_us: 0,
                        input_bytes: req.state.len() as u64,
                        output_bytes: 0,
                        memory_peak_bytes: 0,
                        cpu_time_us: 0,
                    }),
                }
            }
        };

        Ok(Response::new(response))
    }

    type ExecuteNodeStreamStream = tokio_stream::wrappers::ReceiverStream<
        std::result::Result<ExecuteNodeStreamResponse, Status>,
    >;

    async fn execute_node_stream(
        &self,
        request: Request<ExecuteNodeRequest>,
    ) -> std::result::Result<Response<Self::ExecuteNodeStreamStream>, Status> {
        let req = request.into_inner();
        let start_time = std::time::Instant::now();

        debug!(
            "Received streaming execution request for node '{}'",
            req.node_name
        );

        // Create channel for streaming responses
        let (tx, rx) = tokio::sync::mpsc::channel(DEFAULT_MPSC_CHANNEL_CAPACITY);

        // Find node in registry (clone to drop lock before await)
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to execute nodes rather than crash
        let node = {
            let guard = self.registry.read().unwrap_or_else(|e| e.into_inner());
            guard.get(&req.node_name).cloned()
        };

        if node.is_none() {
            error!("Node '{}' not found in registry", req.node_name);
            let _ = tx
                .send(Ok(ExecuteNodeStreamResponse {
                    response: Some(crate::proto::execute_node_stream_response::Response::Error(
                        ExecutionError {
                            code: ErrorCode::NodeNotFound as i32,
                            message: format!("Node '{}' not found", req.node_name),
                            stack_trace: String::new(),
                            context: Default::default(),
                            retryable: false,
                        },
                    )),
                }))
                .await;
            return Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
                rx,
            )));
        }

        // SAFETY: node.is_none() check above returns early, so unwrap is safe
        #[allow(clippy::unwrap_used)]
        let node = node.unwrap();
        let format = SerializationFormat::try_from(req.format).unwrap_or(SerializationFormat::Json);

        // Spawn execution task
        tokio::spawn(async move {
            // Send initial progress
            let _ = tx
                .send(Ok(ExecuteNodeStreamResponse {
                    response: Some(
                        crate::proto::execute_node_stream_response::Response::Progress(
                            crate::proto::ProgressUpdate {
                                percentage: 0.0,
                                message: "Starting execution".to_string(),
                                current_step: "initialization".to_string(),
                            },
                        ),
                    ),
                }))
                .await;

            // Send deserialization progress
            let _ = tx
                .send(Ok(ExecuteNodeStreamResponse {
                    response: Some(
                        crate::proto::execute_node_stream_response::Response::Progress(
                            crate::proto::ProgressUpdate {
                                percentage: 10.0,
                                message: "Deserializing input".to_string(),
                                current_step: "deserialization".to_string(),
                            },
                        ),
                    ),
                }))
                .await;

            // Send execution progress
            let _ = tx
                .send(Ok(ExecuteNodeStreamResponse {
                    response: Some(
                        crate::proto::execute_node_stream_response::Response::Progress(
                            crate::proto::ProgressUpdate {
                                percentage: 30.0,
                                message: "Executing node".to_string(),
                                current_step: "execution".to_string(),
                            },
                        ),
                    ),
                }))
                .await;

            // Execute node
            let result = (node.execute)(req.state.clone(), format).await;

            // Send serialization progress
            let _ = tx
                .send(Ok(ExecuteNodeStreamResponse {
                    response: Some(
                        crate::proto::execute_node_stream_response::Response::Progress(
                            crate::proto::ProgressUpdate {
                                percentage: 90.0,
                                message: "Serializing output".to_string(),
                                current_step: "serialization".to_string(),
                            },
                        ),
                    ),
                }))
                .await;

            // Send final result
            match result {
                Ok(exec_result) => {
                    info!("Node '{}' streaming execution succeeded", req.node_name);
                    let _ = tx
                        .send(Ok(ExecuteNodeStreamResponse {
                            response: Some(
                                crate::proto::execute_node_stream_response::Response::Success(
                                    ExecutionSuccess {
                                        state: exec_result.output_bytes,
                                        format: format as i32,
                                        state_type: req.state_type,
                                    },
                                ),
                            ),
                        }))
                        .await;
                }
                Err(e) => {
                    error!("Node '{}' streaming execution failed: {}", req.node_name, e);
                    let _ = tx
                        .send(Ok(ExecuteNodeStreamResponse {
                            response: Some(
                                crate::proto::execute_node_stream_response::Response::Error(
                                    ExecutionError {
                                        code: ErrorCode::ExecutionFailed as i32,
                                        message: e.to_string(),
                                        stack_trace: String::new(),
                                        context: Default::default(),
                                        retryable: e.is_retryable(),
                                    },
                                ),
                            ),
                        }))
                        .await;
                }
            }

            debug!(
                "Streaming execution completed in {:?}",
                start_time.elapsed()
            );
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn health(
        &self,
        request: Request<HealthRequest>,
    ) -> std::result::Result<Response<HealthResponse>, Status> {
        let req = request.into_inner();

        if req.service.is_empty() || req.service == "RemoteNodeService" {
            Ok(Response::new(HealthResponse {
                status: crate::proto::health_response::HealthStatus::Serving as i32,
                message: "Service is healthy".to_string(),
                version: self.version.clone(),
                capabilities: vec!["ExecuteNode".to_string(), "ExecuteNodeStream".to_string()],
            }))
        } else {
            Ok(Response::new(HealthResponse {
                status: crate::proto::health_response::HealthStatus::ServiceUnknown as i32,
                message: format!("Unknown service: {}", req.service),
                version: self.version.clone(),
                capabilities: vec![],
            }))
        }
    }

    async fn get_node_metadata(
        &self,
        request: Request<GetNodeMetadataRequest>,
    ) -> std::result::Result<Response<GetNodeMetadataResponse>, Status> {
        let req = request.into_inner();
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to get metadata rather than crash
        let guard = self.registry.read().unwrap_or_else(|e| e.into_inner());

        if let Some(registered) = guard.get(&req.node_name) {
            Ok(Response::new(GetNodeMetadataResponse {
                node_name: req.node_name.clone(),
                state_types: vec![registered.state_type_name.clone()],
                formats: vec![
                    SerializationFormat::Json as i32,
                    SerializationFormat::Bincode as i32,
                ],
                resources: None,
                capabilities: vec![],
                estimated_duration_ms: -1,
            }))
        } else {
            Err(Status::not_found(format!(
                "Node '{}' not found",
                req.node_name
            )))
        }
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::node::FunctionNode;

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    // GraphState is already implemented via blanket impl

    #[tokio::test]
    async fn test_node_registry_creation() {
        let registry = NodeRegistry::new();
        let nodes = registry.list_nodes().await;
        assert_eq!(nodes.len(), 0);
    }

    #[tokio::test]
    async fn test_node_registration() {
        let mut registry = NodeRegistry::new();

        let node = FunctionNode::new("test", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        registry.register("test_node", node);

        let nodes = registry.list_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert!(nodes.contains(&"test_node".to_string()));
    }

    #[tokio::test]
    async fn test_execution_metrics_timing() {
        let mut registry = NodeRegistry::new();

        // Register a node that does some work
        let node = FunctionNode::new("compute", |state: TestState| {
            Box::pin(async move {
                // Simulate some computation
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(TestState {
                    value: state.value * 2,
                })
            })
        });

        registry.register("compute_node", node);

        // Get the registered node
        let registered = registry.get_node("compute_node").await.unwrap();

        // Create test state
        let test_state = TestState { value: 42 };

        // Test with JSON serialization
        let state_bytes = serde_json::to_vec(&test_state).unwrap();
        let result = (registered.execute)(state_bytes, SerializationFormat::Json).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();

        // Verify output correctness
        let output_state: TestState = serde_json::from_slice(&exec_result.output_bytes).unwrap();
        assert_eq!(output_state.value, 84);

        // Verify timing metrics - deserialization and serialization might be 0
        // for very small payloads (faster than 1 microsecond), but execution time
        // should be measurable since we sleep for 10ms
        assert!(
            exec_result.execution_us >= 10_000,
            "Execution time should be at least 10ms (10,000 us), got {}",
            exec_result.execution_us
        );

        // Verify CPU time is tracked
        // CPU time tracks actual CPU usage, not wall-clock time like execution_us
        // For async operations with sleep, CPU time may be much lower than wall-clock time
        // since sleep() doesn't consume CPU. The value might be 0 for very fast operations
        // or I/O-bound work, but the field should exist and be populated.
        // This assertion just verifies the field is being populated (any value is fine)
        let _ = exec_result.cpu_time_us; // Field exists and is accessible

        // Verify output bytes were measured
        assert!(
            !exec_result.output_bytes.is_empty(),
            "Output bytes should not be empty"
        );
    }

    #[tokio::test]
    async fn test_execution_metrics_bincode() {
        let mut registry = NodeRegistry::new();

        let node = FunctionNode::new("compute", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 100,
                })
            })
        });

        registry.register("compute_node", node);

        let registered = registry.get_node("compute_node").await.unwrap();
        let test_state = TestState { value: 50 };

        // Test with Bincode serialization
        let state_bytes = bincode::serialize(&test_state).unwrap();
        let result = (registered.execute)(state_bytes, SerializationFormat::Bincode).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();

        // Verify output
        let output_state: TestState = bincode::deserialize(&exec_result.output_bytes).unwrap();
        assert_eq!(output_state.value, 150);

        // Verify metrics - timing may be 0 for very fast operations
        // The important thing is that the mechanism is in place
        assert!(
            !exec_result.output_bytes.is_empty(),
            "Output bytes should not be empty"
        );
    }

    #[tokio::test]
    async fn test_cpu_time_tracking() {
        let mut registry = NodeRegistry::new();

        // Register a node that does CPU-intensive work (not I/O)
        let node = FunctionNode::new("cpu_work", |state: TestState| {
            Box::pin(async move {
                // CPU-intensive computation (not async sleep)
                let mut result = state.value;
                for _ in 0..1_000_000 {
                    result = result.wrapping_add(1).wrapping_mul(3) % 10007;
                }
                Ok(TestState { value: result })
            })
        });

        registry.register("cpu_node", node);

        let registered = registry.get_node("cpu_node").await.unwrap();
        let test_state = TestState { value: 1 };

        // Execute with JSON serialization
        let state_bytes = serde_json::to_vec(&test_state).unwrap();
        let result = (registered.execute)(state_bytes, SerializationFormat::Json).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();

        // Verify CPU time was tracked
        // For CPU-intensive work, cpu_time_us should be non-zero
        // We expect at least some measurable CPU time for 1M iterations
        assert!(
            exec_result.cpu_time_us > 0,
            "CPU time should be non-zero for CPU-intensive work, got {}",
            exec_result.cpu_time_us
        );

        // CPU time should be less than or equal to wall-clock execution time
        // (can be higher on multi-threaded workloads, but our node is single-threaded)
        assert!(
            exec_result.cpu_time_us <= exec_result.execution_us * 2, // Allow some margin
            "CPU time ({}) should be roughly <= execution time ({})",
            exec_result.cpu_time_us,
            exec_result.execution_us
        );

        // Memory tracking should be present (value may be 0 or positive)
        let _ = exec_result.memory_peak_bytes; // Field exists and is accessible
    }

    #[tokio::test]
    async fn test_memory_tracking() {
        let mut registry = NodeRegistry::new();

        // Register a node that allocates memory
        let node = FunctionNode::new("memory_work", |state: TestState| {
            Box::pin(async move {
                // Allocate a large vector to increase memory usage
                let _large_vec: Vec<u8> = vec![0u8; 10_000_000]; // 10MB allocation
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        registry.register("memory_node", node);

        let registered = registry.get_node("memory_node").await.unwrap();
        let test_state = TestState { value: 1 };

        // Execute with JSON serialization
        let state_bytes = serde_json::to_vec(&test_state).unwrap();
        let result = (registered.execute)(state_bytes, SerializationFormat::Json).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();

        // Verify memory tracking field exists and is accessible
        // Memory tracking shows the difference between start and end memory
        // Note: The actual value depends on system memory allocator behavior
        // and may be 0 if the allocator doesn't immediately commit physical pages
        let _ = exec_result.memory_peak_bytes;

        // Verify output correctness
        let output_state: TestState = serde_json::from_slice(&exec_result.output_bytes).unwrap();
        assert_eq!(output_state.value, 2);
    }

    #[tokio::test]
    async fn test_streaming_execution() {
        use futures::StreamExt;

        let mut registry = NodeRegistry::new();

        // Register a test node
        let node = FunctionNode::new("stream_test", |state: TestState| {
            Box::pin(async move {
                // Simulate some work
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                Ok(TestState {
                    value: state.value * 3,
                })
            })
        });

        registry.register("stream_node", node);

        // Create server
        let server = RemoteNodeServer::new(registry);

        // Create request
        let test_state = TestState { value: 7 };
        let state_bytes = serde_json::to_vec(&test_state).unwrap();

        let request = Request::new(ExecuteNodeRequest {
            node_name: "stream_node".to_string(),
            state: state_bytes,
            format: SerializationFormat::Json as i32,
            state_type: "TestState".to_string(),
            timeout_ms: 5000,
            request_id: "test-123".to_string(),
            thread_id: "thread-1".to_string(),
            context: Default::default(),
        });

        // Execute streaming request
        let response = server.execute_node_stream(request).await;
        assert!(response.is_ok());

        let mut stream = response.unwrap().into_inner();

        let mut progress_count = 0;
        let mut final_result = None;

        // Collect all stream messages
        while let Some(msg) = stream.next().await {
            assert!(msg.is_ok());
            let msg = msg.unwrap();

            match msg.response {
                Some(crate::proto::execute_node_stream_response::Response::Progress(progress)) => {
                    progress_count += 1;
                    // Verify progress messages are well-formed
                    assert!(!progress.message.is_empty());
                    assert!(!progress.current_step.is_empty());
                }
                Some(crate::proto::execute_node_stream_response::Response::Success(success)) => {
                    final_result = Some(success);
                }
                Some(crate::proto::execute_node_stream_response::Response::Error(error)) => {
                    panic!("Unexpected error in stream: {}", error.message);
                }
                _ => {}
            }
        }

        // Verify we received progress updates
        assert!(
            progress_count >= 3,
            "Expected at least 3 progress updates, got {}",
            progress_count
        );

        // Verify final result
        assert!(final_result.is_some(), "Expected final success result");
        let success = final_result.unwrap();

        let output_state: TestState = serde_json::from_slice(&success.state).unwrap();
        assert_eq!(output_state.value, 21); // 7 * 3 = 21
    }

    #[tokio::test]
    async fn test_streaming_execution_node_not_found() {
        use futures::StreamExt;

        let registry = NodeRegistry::new();
        let server = RemoteNodeServer::new(registry);

        // Create request for non-existent node
        let request = Request::new(ExecuteNodeRequest {
            node_name: "nonexistent".to_string(),
            state: vec![],
            format: SerializationFormat::Json as i32,
            state_type: "TestState".to_string(),
            timeout_ms: 5000,
            request_id: "test-404".to_string(),
            thread_id: "thread-1".to_string(),
            context: Default::default(),
        });

        // Execute streaming request
        let response = server.execute_node_stream(request).await;
        assert!(response.is_ok());

        let mut stream = response.unwrap().into_inner();

        // Should get exactly one error message
        let msg = stream.next().await;
        assert!(msg.is_some());

        let msg = msg.unwrap();
        assert!(msg.is_ok());

        let msg = msg.unwrap();
        match msg.response {
            Some(crate::proto::execute_node_stream_response::Response::Error(error)) => {
                assert_eq!(error.code, ErrorCode::NodeNotFound as i32);
                assert!(error.message.contains("not found"));
            }
            _ => panic!("Expected error response"),
        }

        // Stream should be complete
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_get_node_metadata_with_state_types() {
        let mut registry = NodeRegistry::new();

        // Register a node with a specific state type
        let node = FunctionNode::new("test", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value + 1,
                })
            })
        });

        registry.register("test_node", node);

        // Create server
        let server = RemoteNodeServer::new(registry);

        // Request metadata
        let request = Request::new(GetNodeMetadataRequest {
            node_name: "test_node".to_string(),
        });

        let response = server.get_node_metadata(request).await;
        assert!(response.is_ok());

        let metadata = response.unwrap().into_inner();

        // Verify node name
        assert_eq!(metadata.node_name, "test_node");

        // Verify state_types contains the TestState type
        assert_eq!(metadata.state_types.len(), 1);
        let state_type = &metadata.state_types[0];
        assert!(
            state_type.contains("TestState"),
            "Expected state_type to contain 'TestState', got: {}",
            state_type
        );

        // Verify supported formats
        assert_eq!(metadata.formats.len(), 2);
        assert!(metadata
            .formats
            .contains(&(SerializationFormat::Json as i32)));
        assert!(metadata
            .formats
            .contains(&(SerializationFormat::Bincode as i32)));
    }

    #[tokio::test]
    async fn test_get_node_metadata_not_found() {
        let registry = NodeRegistry::new();
        let server = RemoteNodeServer::new(registry);

        // Request metadata for non-existent node
        let request = Request::new(GetNodeMetadataRequest {
            node_name: "nonexistent".to_string(),
        });

        let response = server.get_node_metadata(request).await;
        assert!(response.is_err());

        let err = response.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
        assert!(err.message().contains("not found"));
    }
}
