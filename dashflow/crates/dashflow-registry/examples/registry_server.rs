//! Registry Server Example
//!
//! Starts the DashFlow package registry HTTP API server.
//!
//! Usage:
//!   # In-memory mode (default):
//!   cargo run --example registry_server --features server -p dashflow-registry
//!
//!   # PostgreSQL mode:
//!   cargo run --example registry_server --features "server,postgres" -p dashflow-registry
//!
//! Environment Variables:
//!   REGISTRY_HOST     - Host to bind to (default: 127.0.0.1)
//!   REGISTRY_PORT     - Port to bind to (default: 3030)
//!   DATABASE_URL      - PostgreSQL connection string (enables postgres mode)
//!                       Example: postgres://user:pass@localhost/dashflow_registry

use dashflow_registry::{ApiConfig, ApiServer, AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse environment
    let host = std::env::var("REGISTRY_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("REGISTRY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3030);

    println!("DashFlow Package Registry Server");
    println!("=================================");
    println!();

    // Create application state
    #[cfg(feature = "postgres")]
    let state = {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            println!("Connecting to PostgreSQL...");
            let metadata_store =
                dashflow_registry::PostgresMetadataStore::connect(&database_url).await?;
            println!("Running migrations...");
            metadata_store.migrate().await?;
            println!("Using PostgreSQL metadata store");
            AppState::new().await?.with_metadata_store(metadata_store)
        } else {
            println!("DATABASE_URL not set, using in-memory metadata store");
            AppState::new().await?
        }
    };

    #[cfg(not(feature = "postgres"))]
    let state = {
        println!("Using in-memory metadata store");
        println!("(Enable 'postgres' feature and set DATABASE_URL for PostgreSQL)");
        AppState::new().await?
    };

    // Create API server configuration
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let config = ApiConfig::default().with_addr(addr);

    // Create server with state
    let server = ApiServer::with_state(state, config);

    println!();
    println!("Starting server at http://{}:{}", host, port);
    println!();
    println!("API Endpoints:");
    println!("  POST   /api/v1/packages          - Publish a package");
    println!("  GET    /api/v1/packages/:hash    - Get package by hash");
    println!("  POST   /api/v1/packages/resolve  - Resolve name@version to hash");
    println!("  DELETE /api/v1/packages/:hash    - Yank a package");
    println!();
    println!("  POST   /api/v1/search            - Unified search");
    println!("  POST   /api/v1/search/semantic   - Semantic search");
    println!("  GET    /api/v1/search/keyword    - Keyword search");
    println!("  POST   /api/v1/search/capability - Capability search");
    println!();
    println!("  GET    /health                   - Health check");
    println!("  GET    /ready                    - Readiness check");
    println!();

    server.run().await?;

    Ok(())
}
