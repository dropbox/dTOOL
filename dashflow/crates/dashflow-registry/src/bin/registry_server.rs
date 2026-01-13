//! DashFlow Package Registry Server
//!
//! Production HTTP API server for the package registry.
//!
//! # Usage
//!
//! ```bash
//! # Build and run (in-memory storage)
//! cargo run --bin registry_server --features server -p dashflow-registry
//!
//! # With PostgreSQL (requires postgres feature)
//! DATABASE_URL=postgres://user:pass@localhost/db cargo run --bin registry_server --features "server,postgres" -p dashflow-registry
//!
//! # Using docker-compose
//! cd crates/dashflow-registry && docker-compose up -d
//! ```
//!
//! # Environment Variables
//!
//! ## Server Configuration
//! - `REGISTRY_HOST`: Host to bind to (default: 127.0.0.1)
//! - `REGISTRY_PORT`: Port to bind to (default: 3030)
//! - `RUST_LOG`: Log level filter (default: dashflow_registry=info)
//!
//! ## Database Configuration
//! - `DATABASE_URL`: PostgreSQL connection URL (optional, uses in-memory if not set)
//!   Format: postgres://user:password@host:port/database
//!
//! ## Rate Limiting
//! - `RATE_LIMIT_RPM`: Requests per minute (default: 60)
//!
//! ## CORS
//! - `CORS_ENABLED`: Enable CORS (default: true)
//! - `CORS_ORIGINS`: Comma-separated allowed origins (default: *)
//!
//! ## OpenTelemetry (requires opentelemetry feature)
//! - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP exporter endpoint (e.g., http://localhost:4317)
//! - `OTEL_SERVICE_NAME`: Service name for traces (default: dashflow-registry)
//! - `OTEL_SAMPLING_RATE`: Sampling rate 0.0-1.0 (default: 1.0)
//!
//! ## Storage
//! - `STORAGE_URL`: Storage backend URL (default: file:///tmp/dashflow-registry)
//! - `BASE_URL`: Public base URL for download links (default: http://localhost:3030)
//!
//! ## CDN Integration
//! - `CDN_ENABLED`: Enable CDN-direct downloads via presigned URLs (default: false)
//!   When enabled and using S3 storage, resolve responses include direct download URLs
//!   that bypass the API server, reducing bandwidth and improving latency.
//!
//! # API Endpoints
//!
//! ## Packages
//! - `POST /api/v1/packages` - Publish a package
//! - `GET /api/v1/packages/:hash` - Get package by content hash
//! - `POST /api/v1/packages/resolve` - Resolve name@version to hash
//! - `DELETE /api/v1/packages/:hash` - Yank a package
//!
//! ## Search
//! - `POST /api/v1/search` - Unified search (semantic + keyword)
//! - `POST /api/v1/search/semantic` - Semantic search only
//! - `GET /api/v1/search/keyword` - Keyword search only
//! - `POST /api/v1/search/capability` - Find by capability
//!
//! ## Contributions
//! - `POST /api/v1/contributions/bug` - Submit bug report
//! - `POST /api/v1/contributions/improvement` - Submit improvement
//! - `POST /api/v1/contributions/request` - Submit package request
//! - `POST /api/v1/contributions/fix` - Submit fix
//! - `POST /api/v1/contributions/:id/review` - Submit review
//!
//! ## Health
//! - `GET /` - Root handler
//! - `GET /health` - Health check
//! - `GET /ready` - Readiness check

use std::net::SocketAddr;
use std::process::ExitCode;

use dashflow::core::config_loader::env_vars::{
    env_string, BASE_URL, CDN_ENABLED, CORS_ENABLED, CORS_ORIGINS, DATABASE_URL, RATE_LIMIT_RPM,
    REGISTRY_HOST, REGISTRY_PORT, STORAGE_URL,
};
#[cfg(feature = "opentelemetry")]
use dashflow::core::config_loader::env_vars::{
    OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_SAMPLING_RATE, OTEL_SERVICE_NAME,
};
use dashflow_registry::api::state::ServerConfig;
use dashflow_registry::{ApiConfig, ApiServer, AppState};
#[cfg(not(feature = "postgres"))]
use tracing::warn;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Server configuration loaded from environment
struct EnvConfig {
    host: String,
    port: u16,
    database_url: Option<String>,
    rate_limit_rpm: u32,
    cors_enabled: bool,
    cors_origins: Vec<String>,
    storage_url: String,
    base_url: String,
    /// Enable CDN-direct downloads (redirect to S3/R2 instead of proxying)
    cdn_enabled: bool,
    // OpenTelemetry configuration
    #[cfg(feature = "opentelemetry")]
    otel_endpoint: Option<String>,
    #[cfg(feature = "opentelemetry")]
    otel_service_name: String,
    #[cfg(feature = "opentelemetry")]
    otel_sampling_rate: f64,
}

impl EnvConfig {
    fn from_env() -> Self {
        Self {
            host: env_string(REGISTRY_HOST).unwrap_or_else(|| "127.0.0.1".to_string()),
            port: env_string(REGISTRY_PORT)
                .and_then(|p| p.parse().ok())
                .unwrap_or(3030),
            database_url: env_string(DATABASE_URL),
            rate_limit_rpm: env_string(RATE_LIMIT_RPM)
                .and_then(|r| r.parse().ok())
                .unwrap_or(60),
            cors_enabled: env_string(CORS_ENABLED)
                .map(|v| v.to_lowercase() != "false" && v != "0")
                .unwrap_or(true),
            cors_origins: env_string(CORS_ORIGINS)
                .map(|o| o.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["*".to_string()]),
            storage_url: env_string(STORAGE_URL)
                .unwrap_or_else(|| "file:///tmp/dashflow-registry".to_string()),
            base_url: env_string(BASE_URL)
                .unwrap_or_else(|| "http://localhost:3030".to_string()),
            cdn_enabled: env_string(CDN_ENABLED)
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),
            // OpenTelemetry configuration
            #[cfg(feature = "opentelemetry")]
            otel_endpoint: env_string(OTEL_EXPORTER_OTLP_ENDPOINT),
            #[cfg(feature = "opentelemetry")]
            otel_service_name: env_string(OTEL_SERVICE_NAME)
                .unwrap_or_else(|| "dashflow-registry".to_string()),
            #[cfg(feature = "opentelemetry")]
            otel_sampling_rate: env_string(OTEL_SAMPLING_RATE)
                .and_then(|r| r.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

fn init_tracing(_config: &EnvConfig) {
    // Set up tracing with environment filter
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("dashflow_registry=info,tower_http=info"));

    // Build base subscriber
    let subscriber = tracing_subscriber::registry().with(filter).with(
        fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false),
    );

    // Add OpenTelemetry layer if feature enabled and endpoint configured
    #[cfg(feature = "opentelemetry")]
    let subscriber = {
        use opentelemetry::global;
        use opentelemetry::trace::TracerProvider;
        use opentelemetry::KeyValue;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
        use opentelemetry_sdk::Resource;
        use tracing_opentelemetry::OpenTelemetryLayer;

        if let Some(ref endpoint) = _config.otel_endpoint {
            // Build resource with service name
            let resource = Resource::builder_empty()
                .with_attributes(vec![
                    KeyValue::new("service.name", _config.otel_service_name.clone()),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])
                .build();

            // Configure sampling
            let sampler = if _config.otel_sampling_rate >= 1.0 {
                Sampler::AlwaysOn
            } else if _config.otel_sampling_rate <= 0.0 {
                Sampler::AlwaysOff
            } else {
                Sampler::TraceIdRatioBased(_config.otel_sampling_rate)
            };

            match opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()
            {
                Ok(exporter) => {
                    // Build tracer provider
                    let provider = SdkTracerProvider::builder()
                        .with_resource(resource)
                        .with_sampler(sampler)
                        .with_batch_exporter(exporter)
                        .build();

                    // Set global tracer provider
                    global::set_tracer_provider(provider.clone());

                    let tracer = provider.tracer("dashflow-registry");
                    let otel_layer = OpenTelemetryLayer::new(tracer);

                    info!(
                        endpoint = %endpoint,
                        service_name = %_config.otel_service_name,
                        sampling_rate = _config.otel_sampling_rate,
                        "OpenTelemetry tracing enabled"
                    );

                    subscriber.with(Some(otel_layer))
                }
                Err(err) => {
                    error!(
                        error = %err,
                        "Failed to create OTLP exporter; OpenTelemetry disabled"
                    );
                    subscriber.with(None::<OpenTelemetryLayer<_, opentelemetry_sdk::trace::Tracer>>)
                }
            }
        } else {
            // No OpenTelemetry endpoint configured
            subscriber.with(None::<OpenTelemetryLayer<_, opentelemetry_sdk::trace::Tracer>>)
        }
    };

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Failed to set tracing subscriber: {err}");
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Load configuration from environment
    let config = EnvConfig::from_env();

    // Initialize tracing (must be done after loading config for OpenTelemetry)
    init_tracing(&config);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "DashFlow Package Registry Server starting"
    );

    info!(
        host = %config.host,
        port = config.port,
        database = config.database_url.as_ref().map(|_| "PostgreSQL").unwrap_or("In-memory"),
        rate_limit_rpm = config.rate_limit_rpm,
        cors_enabled = config.cors_enabled,
        "Configuration loaded"
    );

    // Build server config
    let server_config = ServerConfig {
        max_body_size: 50 * 1024 * 1024, // 50MB
        rate_limit_rpm: config.rate_limit_rpm,
        cors_enabled: config.cors_enabled,
        cors_origins: config.cors_origins.clone(),
        base_url: config.base_url.clone(),
        storage_url: config.storage_url.clone(),
        cdn_enabled: config.cdn_enabled,
    };

    // Create application state
    let state = match create_app_state(&config, server_config).await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to initialize application state");
            return ExitCode::FAILURE;
        }
    };

    // Parse address
    let addr: SocketAddr = match format!("{}:{}", config.host, config.port).parse() {
        Ok(a) => a,
        Err(e) => {
            error!(host = %config.host, port = config.port, error = %e, "Invalid address");
            return ExitCode::FAILURE;
        }
    };

    // Create server configuration
    let api_config = ApiConfig::default().with_addr(addr);

    // Create and run server
    let server = ApiServer::with_state(state, api_config);

    info!(
        addr = %addr,
        health = format!("http://{}/health", addr),
        api = format!("http://{}/api/v1", addr),
        "Server starting"
    );

    // Handle graceful shutdown
    let shutdown_signal = shutdown_signal();

    if let Err(e) = server.run_until(shutdown_signal).await {
        error!(error = %e, "Server error");
        return ExitCode::FAILURE;
    }

    info!("Server shutdown complete");
    ExitCode::SUCCESS
}

async fn create_app_state(
    config: &EnvConfig,
    server_config: ServerConfig,
) -> dashflow_registry::Result<AppState> {
    let state = AppState::with_config(server_config).await?;

    // If DATABASE_URL is set, connect to PostgreSQL
    #[cfg(feature = "postgres")]
    if let Some(ref database_url) = config.database_url {
        info!("Connecting to PostgreSQL database...");

        let pg_store =
            dashflow_registry::metadata::postgres::PostgresMetadataStore::connect(database_url)
                .await?;

        info!("Running database migrations...");
        pg_store.migrate().await?;

        info!("PostgreSQL connection established");

        // Replace in-memory stores with PostgreSQL
        return Ok(state.with_store(pg_store));
    }

    #[cfg(not(feature = "postgres"))]
    if config.database_url.is_some() {
        warn!("DATABASE_URL is set but postgres feature is not enabled. Using in-memory storage.");
    }

    info!("Using in-memory storage (data will not persist across restarts)");
    Ok(state)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            error!(error = %err, "Failed to install Ctrl+C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                error!(error = %err, "Failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown...");
        }
        _ = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        }
    }
}
