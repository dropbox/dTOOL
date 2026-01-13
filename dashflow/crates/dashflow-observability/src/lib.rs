//! `DashFlow` Observability
//!
//! Enterprise-grade distributed tracing, metrics, and cost tracking for `DashFlow` Rust applications.
//!
//! This crate provides OpenTelemetry integration with automatic instrumentation for:
//! - Graph execution workflows
//! - Node invocations
//! - LLM generation calls
//! - Tool invocations
//! - Checkpointer operations
//! - Cost tracking and reporting
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_observability::{TracingConfig, init_tracing};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure tracing
//!     let config = TracingConfig::new()
//!         .with_service_name("my-agent")
//!         .with_otlp_endpoint("http://localhost:4317")
//!         .with_sampling_rate(1.0);
//!
//!     // Initialize tracing
//!     init_tracing(config).await?;
//!
//!     // Your application code here
//!     // All tracing::span! and tracing::event! calls will be exported
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod cost;
pub mod error;
pub mod exporter;
pub mod metrics;
pub mod tracer;

#[cfg(feature = "metrics-server")]
pub mod metrics_server;

pub use config::{PropagatorType, SamplingStrategy, TracingConfig};
pub use cost::{
    AlertLevel, BudgetConfig, BudgetEnforcer, CostRecord, CostReport, CostTracker, ModelPrice,
    ModelPricing, Pricing, TokenUsage,
};
pub use error::{Error, Result};
pub use exporter::init_tracing;
pub use metrics::{
    default_slo_definitions, export_metrics, init_default_recorder, metrics_registry,
    register_default_metrics, MetricsRecorder, MetricsRegistry, SloDefinition, SloType,
};
pub use tracer::Traceable;
