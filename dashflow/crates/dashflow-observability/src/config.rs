//! Configuration for OpenTelemetry tracing

use serde::{Deserialize, Serialize};

/// Context propagation protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PropagatorType {
    /// W3C Trace Context (recommended for cross-service tracing)
    #[default]
    TraceContext,
    /// Jaeger native propagation format
    Jaeger,
    /// B3 propagation (Zipkin)
    B3,
    /// AWS X-Ray propagation
    XRay,
}

/// Sampling strategy for span collection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SamplingStrategy {
    /// Always sample (100% of spans)
    #[default]
    Always,
    /// Never sample (0% of spans)
    Never,
    /// Sample at specified rate (0.0 to 1.0)
    Probabilistic(f64),
    /// Sample based on parent span decision
    ParentBased {
        /// Sampling strategy for root spans
        root: Box<SamplingStrategy>,
    },
}

/// Configuration for OpenTelemetry tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Service name to identify this application in traces
    pub service_name: String,

    /// OpenTelemetry Collector endpoint (OTLP/gRPC)
    /// Example: "<http://localhost:4317>" (Jaeger)
    pub otlp_endpoint: Option<String>,

    /// Sampling strategy
    pub sampling: SamplingStrategy,

    /// Context propagation protocol
    pub propagator: PropagatorType,

    /// Enable stdout exporter for debugging
    pub enable_stdout: bool,

    /// Additional resource attributes (key-value pairs)
    pub resource_attributes: Vec<(String, String)>,
}

impl TracingConfig {
    /// Create a new tracing configuration with defaults
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_observability::TracingConfig;
    ///
    /// let config = TracingConfig::new()
    ///     .with_service_name("my-agent")
    ///     .with_otlp_endpoint("http://localhost:4317")
    ///     .with_sampling_rate(1.0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            service_name: "dashflow".to_string(),
            otlp_endpoint: None,
            sampling: SamplingStrategy::Always,
            propagator: PropagatorType::TraceContext,
            enable_stdout: false,
            resource_attributes: Vec::new(),
        }
    }

    /// Set the service name
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Set the OTLP endpoint
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    /// Set the sampling rate (0.0 to 1.0)
    #[must_use]
    pub fn with_sampling_rate(mut self, rate: f64) -> Self {
        self.sampling = if rate >= 1.0 {
            SamplingStrategy::Always
        } else if rate <= 0.0 {
            SamplingStrategy::Never
        } else {
            SamplingStrategy::Probabilistic(rate)
        };
        self
    }

    /// Set the sampling strategy
    #[must_use]
    pub fn with_sampling_strategy(mut self, strategy: SamplingStrategy) -> Self {
        self.sampling = strategy;
        self
    }

    /// Set the context propagator
    #[must_use]
    pub fn with_propagator(mut self, propagator: PropagatorType) -> Self {
        self.propagator = propagator;
        self
    }

    /// Enable stdout exporter for debugging
    #[must_use]
    pub fn with_stdout(mut self, enabled: bool) -> Self {
        self.enable_stdout = enabled;
        self
    }

    /// Add a resource attribute (key-value pair)
    pub fn with_resource_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.resource_attributes.push((key.into(), value.into()));
        self
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "dashflow");
        assert_eq!(config.otlp_endpoint, None);
        assert_eq!(config.sampling, SamplingStrategy::Always);
        assert_eq!(config.propagator, PropagatorType::TraceContext);
        assert!(!config.enable_stdout);
    }

    #[test]
    fn test_builder_pattern() {
        let config = TracingConfig::new()
            .with_service_name("test-service")
            .with_otlp_endpoint("http://localhost:4317")
            .with_sampling_rate(0.5)
            .with_stdout(true)
            .with_resource_attribute("env", "production");

        assert_eq!(config.service_name, "test-service");
        assert_eq!(
            config.otlp_endpoint,
            Some("http://localhost:4317".to_string())
        );
        assert_eq!(config.sampling, SamplingStrategy::Probabilistic(0.5));
        assert!(config.enable_stdout);
        assert_eq!(config.resource_attributes.len(), 1);
    }

    #[test]
    fn test_sampling_rate_boundaries() {
        let always = TracingConfig::new().with_sampling_rate(1.5);
        assert_eq!(always.sampling, SamplingStrategy::Always);

        let never = TracingConfig::new().with_sampling_rate(-0.5);
        assert_eq!(never.sampling, SamplingStrategy::Never);

        let prob = TracingConfig::new().with_sampling_rate(0.75);
        assert_eq!(prob.sampling, SamplingStrategy::Probabilistic(0.75));
    }
}
