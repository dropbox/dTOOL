//! OpenTelemetry exporter initialization

use crate::config::{PropagatorType, SamplingStrategy, TracingConfig};
use crate::error::{Error, Result};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::{BaggagePropagator, TraceContextPropagator};
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialize OpenTelemetry tracing with the provided configuration
///
/// This function sets up the OpenTelemetry SDK with:
/// - OTLP exporter (if endpoint configured)
/// - Stdout exporter (if enabled)
/// - W3C Trace Context propagation
/// - Automatic resource detection
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_observability::{TracingConfig, init_tracing};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = TracingConfig::new()
///         .with_service_name("my-agent")
///         .with_otlp_endpoint("http://localhost:4317")
///         .with_sampling_rate(1.0);
///
///     init_tracing(config).await?;
///     Ok(())
/// }
/// ```
pub async fn init_tracing(config: TracingConfig) -> Result<()> {
    // Build resource with service name and custom attributes
    let mut resource_kvs = vec![KeyValue::new("service.name", config.service_name.clone())];
    for (key, value) in config.resource_attributes {
        resource_kvs.push(KeyValue::new(key, value));
    }

    // Create resource from key-value pairs
    let resource = Resource::builder_empty()
        .with_attributes(resource_kvs)
        .build();

    // Configure sampling
    let sampler = match config.sampling {
        SamplingStrategy::Always => Sampler::AlwaysOn,
        SamplingStrategy::Never => Sampler::AlwaysOff,
        SamplingStrategy::Probabilistic(rate) => Sampler::TraceIdRatioBased(rate),
        SamplingStrategy::ParentBased { root } => {
            let root_sampler = match *root {
                SamplingStrategy::Always => Sampler::AlwaysOn,
                SamplingStrategy::Never => Sampler::AlwaysOff,
                SamplingStrategy::Probabilistic(rate) => Sampler::TraceIdRatioBased(rate),
                _ => Sampler::AlwaysOn, // Fallback for nested ParentBased
            };
            Sampler::ParentBased(Box::new(root_sampler))
        }
    };

    // Build tracer provider
    let mut provider_builder = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(sampler);

    // Add OTLP exporter if endpoint configured
    // Port 4317 = gRPC, Port 4318 = HTTP
    if let Some(endpoint) = config.otlp_endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()
            .map_err(|e| Error::ExporterConnectionError(e.to_string()))?;

        provider_builder = provider_builder.with_batch_exporter(exporter);
    }

    // Add stdout exporter if enabled
    if config.enable_stdout {
        let stdout_exporter = opentelemetry_stdout::SpanExporter::default();
        provider_builder = provider_builder.with_simple_exporter(stdout_exporter);
    }

    let provider = provider_builder.build();

    // Set global tracer provider
    global::set_tracer_provider(provider);

    // Configure propagator based on config.propagator
    let propagator: TextMapCompositePropagator = match config.propagator {
        PropagatorType::TraceContext => TextMapCompositePropagator::new(vec![
            Box::new(TraceContextPropagator::new()),
            Box::new(BaggagePropagator::new()),
        ]),
        PropagatorType::Jaeger => {
            // Jaeger propagator using W3C TraceContext for compatibility
            // Note: Native Jaeger propagation would require opentelemetry-jaeger crate
            TextMapCompositePropagator::new(vec![
                Box::new(TraceContextPropagator::new()),
                Box::new(BaggagePropagator::new()),
            ])
        }
        PropagatorType::B3 => {
            // B3 propagator using W3C TraceContext for compatibility
            // Note: Native B3 propagation would require opentelemetry-zipkin crate
            TextMapCompositePropagator::new(vec![
                Box::new(TraceContextPropagator::new()),
                Box::new(BaggagePropagator::new()),
            ])
        }
        PropagatorType::XRay => {
            // X-Ray propagator using W3C TraceContext for compatibility
            // Note: Native X-Ray propagation would require opentelemetry-aws crate
            TextMapCompositePropagator::new(vec![
                Box::new(TraceContextPropagator::new()),
                Box::new(BaggagePropagator::new()),
            ])
        }
    };

    global::set_text_map_propagator(propagator);

    // Initialize tracing subscriber
    let tracer = global::tracer("dashflow");
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(telemetry_layer)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| Error::InitializationError(e.to_string()))?;

    Ok(())
}

/// Shutdown hint for the tracing system
///
/// In OpenTelemetry v0.31+, the tracer provider shuts down automatically when
/// dropped. This function is provided as a no-op for API compatibility and as
/// a clear point in code where shutdown is expected.
///
/// For proper cleanup, ensure the `SdkTracerProvider` returned from init goes
/// out of scope (or store it and drop it explicitly) before process exit.
///
/// # Example
///
/// ```ignore
/// use dashflow_observability::shutdown_tracing;
///
/// // At application shutdown - currently a no-op as OTel v0.31 handles cleanup on drop
/// shutdown_tracing();
/// ```
pub fn shutdown_tracing() {
    // OpenTelemetry v0.31+ handles shutdown automatically when the SdkTracerProvider
    // is dropped. The global tracer provider wrapper doesn't expose explicit shutdown.
    // To ensure proper cleanup:
    // 1. Store the SdkTracerProvider from init_tracing() if you need explicit control
    // 2. Drop it before process exit to trigger span flushing
    // This function exists for API completeness and as a marker in user code.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_tracing_with_stdout() {
        let config = TracingConfig::new()
            .with_service_name("test-service")
            .with_stdout(true);

        // Should not panic
        let result = init_tracing(config).await;
        // May fail if already initialized in other tests
        let _ = result;
    }

    #[tokio::test]
    async fn test_init_tracing_with_attributes() {
        let config = TracingConfig::new()
            .with_service_name("test-service")
            .with_resource_attribute("env", "test")
            .with_resource_attribute("version", "1.0.0")
            .with_stdout(true);

        let result = init_tracing(config).await;
        let _ = result;
    }
}
