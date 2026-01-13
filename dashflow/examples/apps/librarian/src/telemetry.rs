//! Telemetry setup for Book Search

use anyhow::Result;
use metrics_exporter_prometheus::PrometheusBuilder;
use opentelemetry::trace::TracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Telemetry handle that must be kept alive and shut down properly
pub struct TelemetryHandle {
    provider: Option<SdkTracerProvider>,
}

impl TelemetryHandle {
    /// Shutdown telemetry and flush any pending traces
    pub fn shutdown(self) {
        if let Some(provider) = self.provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("Warning: Failed to shutdown telemetry provider: {:?}", e);
            }
        }
    }
}

/// Initialize telemetry (metrics, tracing, logging)
/// Returns a handle that must be kept alive and shut down properly
pub fn init_telemetry(
    metrics_port: u16,
    otlp_endpoint: Option<&str>,
    service_name: &str,
) -> Result<TelemetryHandle> {
    // Initialize Prometheus metrics exporter
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], metrics_port))
        .install()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus exporter: {}", e))?;

    // Record startup metric
    metrics::counter!("librarian_startup_total").increment(1);

    // Build logging/tracing layers
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true);

    // Initialize tracing with or without OTLP
    let provider = if let Some(endpoint) = otlp_endpoint {
        // Create OTLP exporter
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create OTLP exporter: {}", e))?;

        // Create resource with service name for Jaeger identification
        let resource = Resource::builder()
            .with_attribute(KeyValue::new("service.name", service_name.to_string()))
            .build();

        let provider = SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build();

        let tracer = provider.tracer(service_name.to_string());

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();

        Some(provider)
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        None
    };

    Ok(TelemetryHandle { provider })
}

/// Record index size metric
pub fn record_index_size(doc_count: u64) {
    metrics::gauge!("librarian_index_size_docs").set(doc_count as f64);
}

/// Record quality score
pub fn record_quality_score(score: f64) {
    metrics::gauge!("librarian_quality_score").set(score);
}

/// Record error
pub fn record_error(error_type: &str) {
    metrics::counter!("librarian_errors_total", "type" => error_type.to_string()).increment(1);
}

/// Record search operation
pub fn record_search(duration_ms: f64, result_count: usize) {
    metrics::counter!("librarian_searches_total").increment(1);
    metrics::histogram!("librarian_search_duration_ms").record(duration_ms);
    metrics::histogram!("librarian_search_results").record(result_count as f64);
}
