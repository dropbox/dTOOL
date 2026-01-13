//! `LangServe` - REST API deployment framework for `DashFlow` runnables
//!
//! `LangServe` provides a simple way to deploy `DashFlow` runnables as REST APIs using Axum.
//! It's designed to be compatible with the Python `LangServe` API format.
//!
//! # Quick Start
//!
//! ```ignore
//! use axum::Router;
//! use dashflow_langserve::{add_routes, create_server, RouteConfig};
//!
//! // Create your runnable
//! let runnable = /* your runnable */;
//!
//! // Create a server and add routes
//! let app = create_server();
//! let app = add_routes(app, runnable, RouteConfig::new("/my_runnable"));
//!
//! // Start the server
//! let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
//! axum::serve(listener, app).await.unwrap();
//! ```
//!
//! # Features
//!
//! - **Core Endpoints**: `/invoke`, `/batch`, `/stream`
//! - **Schema Endpoints**: `/input_schema`, `/output_schema`, `/config_schema`
//! - **Streaming**: Server-Sent Events (SSE) support
//! - **Client**: `RemoteRunnable` for calling remote runnables
//! - **Playground**: Interactive testing UI
//!
//! # Architecture
//!
//! `LangServe` is built on:
//! - **Axum**: Web framework
//! - **Tower**: Middleware and services
//! - **`DashFlow` Core**: Runnable trait and types
//!
//! # Implementation Status
//!
//!: Foundation ✅
//!: Core Endpoints ✅
//!: Streaming Support (basic streaming done)
//!: Schema and Metadata ✅
//!: Client Implementation ✅
//!: Playground and Polish ✅
//!: Testing and Validation ✅

pub mod client;
pub mod error;
pub mod handler;
pub mod metrics;
pub mod playground;
pub mod schema;
pub mod server;

// Re-export main API
pub use client::RemoteRunnable;
pub use error::{LangServeError, Result};
pub use schema::{
    BatchMetadata, BatchRequest, BatchResponse, InvokeMetadata, InvokeRequest, InvokeResponse,
    RunnableConfig, SchemaResponse, StreamEvent, StreamRequest,
};
pub use server::{add_routes, create_server, create_server_with_cors, RouteConfig};

// All implementation complete
// - Streaming support via SSE
// - Schema and metadata
// - Client implementation
// - Playground and polish
// - Testing and validation
