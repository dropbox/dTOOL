// Note: Audited - this crate has zero unwrap calls in production code.
// clone_on_ref_ptr: Moved to function-level allows where Arc::clone() pattern needed

//! Remote Node Execution for Distributed `DashFlow` Workflows
//!
//! This crate enables distributing graph node execution across multiple machines
//! via gRPC. Compute-intensive nodes can run on dedicated infrastructure while
//! maintaining the same Node interface.
//!
//! # Features
//!
//! - **Remote execution**: Execute nodes on different machines via gRPC
//! - **Transparent interface**: `RemoteNode` implements the same Node trait
//! - **Fault tolerance**: Automatic retry with exponential backoff
//! - **Flexible serialization**: JSON or bincode for state transfer
//! - **Health checking**: Monitor remote node availability
//! - **Load balancing**: Distribute work across multiple endpoints (future)
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::StateGraph;
//! use dashflow_remote_node::RemoteNode;
//!
//! let mut graph = StateGraph::new();
//!
//! // Add local nodes
//! graph.add_node("preprocess", preprocess_node);
//!
//! // Add remote node for heavy computation
//! let remote_node = RemoteNode::new("compute")
//!     .with_endpoint("http://compute-server:50051")
//!     .with_timeout(Duration::from_secs(300))
//!     .with_retry_count(3);
//!
//! graph.add_node("heavy_compute", remote_node);
//!
//! // Add more local nodes
//! graph.add_node("postprocess", postprocess_node);
//!
//! graph.add_edge("preprocess", "heavy_compute");
//! graph.add_edge("heavy_compute", "postprocess");
//! graph.set_entry_point("preprocess");
//!
//! let app = graph.compile()?;
//! let result = app.invoke(state).await?;
//! ```

pub mod client;
pub mod error;
pub mod server;

// Re-export common types
pub use client::{RemoteNode, RemoteNodeConfig};
pub use error::{Error, Result};
pub use server::{NodeRegistry, RemoteNodeServer};

// Re-export generated protobuf types
#[allow(clippy::clone_on_ref_ptr)] // Generated tonic code uses Arc::clone() patterns
pub mod proto {
    tonic::include_proto!("dashflow.remote_node.v1");
}
