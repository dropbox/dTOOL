//! Error types for `DashFlow` Observability

use thiserror::Error;

/// Error type for observability operations
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// OpenTelemetry SDK initialization failed
    #[error("Failed to initialize OpenTelemetry: {0}")]
    InitializationError(String),

    /// Tracing configuration error
    #[error("Invalid tracing configuration: {0}")]
    ConfigurationError(String),

    /// Exporter connection error
    #[error("Failed to connect to exporter endpoint: {0}")]
    ExporterConnectionError(String),

    /// Span creation error
    #[error("Failed to create span: {0}")]
    SpanCreationError(String),

    /// Metrics error
    #[error("Metrics operation failed: {0}")]
    Metrics(String),

    /// Generic error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type for observability operations
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_error_display() {
        let err = Error::InitializationError("tracer failed".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to initialize OpenTelemetry: tracer failed"
        );
    }

    #[test]
    fn test_configuration_error_display() {
        let err = Error::ConfigurationError("invalid sampling rate".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid tracing configuration: invalid sampling rate"
        );
    }

    #[test]
    fn test_exporter_connection_error_display() {
        let err = Error::ExporterConnectionError("connection refused".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to connect to exporter endpoint: connection refused"
        );
    }

    #[test]
    fn test_span_creation_error_display() {
        let err = Error::SpanCreationError("invalid name".to_string());
        assert_eq!(err.to_string(), "Failed to create span: invalid name");
    }

    #[test]
    fn test_metrics_error_display() {
        let err = Error::Metrics("counter overflow".to_string());
        assert_eq!(err.to_string(), "Metrics operation failed: counter overflow");
    }

    #[test]
    fn test_other_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("generic failure");
        let err = Error::from(anyhow_err);
        assert!(matches!(err, Error::Other(_)));
        assert!(err.to_string().contains("generic failure"));
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Metrics("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Metrics"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_error_variants_exhaustive() {
        // Ensure all variants are constructible
        let _ = Error::InitializationError("init".to_string());
        let _ = Error::ConfigurationError("config".to_string());
        let _ = Error::ExporterConnectionError("connect".to_string());
        let _ = Error::SpanCreationError("span".to_string());
        let _ = Error::Metrics("metrics".to_string());
        let _ = Error::Other(anyhow::anyhow!("other"));
    }
}
