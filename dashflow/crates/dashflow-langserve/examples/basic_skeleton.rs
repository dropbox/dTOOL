//! Basic skeleton example for LangServe
//!
//! This example demonstrates All phases functionality:
//! - Creating a server with add_routes()
//! - Route configuration
//! - Core endpoints: /invoke, /batch, /stream
//! - Schema endpoints: /input_schema, /output_schema, /config_schema
//! - Interactive playground UI: /playground
//!
//! Run with: cargo run --example basic_skeleton -p dashflow-langserve

use dashflow::core::runnable::Runnable;
use dashflow_langserve::{add_routes, create_server, RouteConfig};
use serde_json::Value;

// Placeholder runnable that implements Runnable trait
struct DummyRunnable;

#[async_trait::async_trait]
impl Runnable for DummyRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<dashflow::core::config::RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        // Just echo the input back
        Ok(input)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt::init();

    // Create a basic server with CORS
    let app = create_server();

    // Add a dummy runnable with default configuration
    let runnable = DummyRunnable;
    let app = add_routes(app, runnable, RouteConfig::new("/dummy"));

    // Add another runnable with custom configuration
    let runnable2 = DummyRunnable;
    let config = RouteConfig::new("/custom")
        .with_invoke(true)
        .with_batch(false) // Disable batch endpoint
        .with_stream(true)
        .with_schema(true);
    let app = add_routes(app, runnable2, config);

    println!("LangServe All phases Complete with Observability!");
    println!();
    println!("📊 Observability Endpoints:");
    println!("  - /metrics (GET)       ← Prometheus metrics");
    println!("  - /health (GET)        ← Liveness probe");
    println!("  - /ready (GET)         ← Readiness probe");
    println!();
    println!("🚀 Runnable Endpoints:");
    println!("  - /dummy/invoke (POST)");
    println!("  - /dummy/batch (POST)");
    println!("  - /dummy/stream (POST)");
    println!("  - /dummy/input_schema (GET)");
    println!("  - /dummy/output_schema (GET)");
    println!("  - /dummy/config_schema (GET)");
    println!("  - /dummy/playground (GET)  ← Interactive UI");
    println!();
    println!("  - /custom/invoke (POST)");
    println!("  - /custom/stream (POST)");
    println!("  - /custom/input_schema (GET)");
    println!("  - /custom/output_schema (GET)");
    println!("  - /custom/config_schema (GET)");
    println!();
    println!("All phases Complete: Core endpoints, schemas, and playground functional!");
    println!("The server will echo back any input you send to it.");
    println!();
    println!("🎮 Try the interactive playground:");
    println!("   Open http://localhost:8000/dummy/playground in your browser!");
    println!();
    println!("Or use curl to invoke the runnable:");
    println!("  curl -X POST http://localhost:8000/dummy/invoke \\");
    println!("       -H 'Content-Type: application/json' \\");
    println!("       -d '{{\"input\": \"Hello LangServe!\"}}'");
    println!();
    println!("Or check the schemas:");
    println!("  curl http://localhost:8000/dummy/input_schema");
    println!("  curl http://localhost:8000/dummy/output_schema");
    println!("  curl http://localhost:8000/dummy/config_schema");
    println!();
    println!("📊 Check observability:");
    println!("  curl http://localhost:8000/metrics");
    println!("  curl http://localhost:8000/health");
    println!("  curl http://localhost:8000/ready");
    println!();
    println!("Starting server on 0.0.0.0:8000...");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;

    axum::serve(listener, app).await?;
    Ok(())
}
