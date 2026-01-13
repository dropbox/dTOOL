//! Trait for automatic instrumentation

use async_trait::async_trait;
use std::future::Future;

/// Trait for automatic span instrumentation
///
/// This trait provides a convenient way to wrap async operations in OpenTelemetry spans.
/// Implementors can use the `with_span` method to automatically create spans with
/// appropriate attributes.
///
/// # Example
///
/// ```ignore
/// use dashflow_observability::Traceable;
/// use async_trait::async_trait;
/// use std::future::Future;
/// use tracing::Instrument;
///
/// struct MyService {
///     name: String,
/// }
///
/// #[async_trait]
/// impl Traceable for MyService {
///     async fn execute_traced<F, T>(&self, operation: &str, f: F) -> T
///     where
///         F: Future<Output = T> + Send,
///         T: Send,
///     {
///         let span = tracing::info_span!("my_service.execute", operation = operation);
///         f.instrument(span).await
///     }
/// }
/// ```
#[async_trait]
pub trait Traceable {
    /// Execute an async operation within a tracing span
    ///
    /// # Arguments
    ///
    /// * `operation` - Name of the operation being performed
    /// * `f` - Async function to execute within the span
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dashflow_observability::Traceable;
    ///
    /// let result = service.execute_traced("fetch_data", async {
    ///     // Your async operation here
    ///     42
    /// }).await;
    /// ```
    async fn execute_traced<F, T>(&self, operation: &str, f: F) -> T
    where
        F: Future<Output = T> + Send,
        T: Send;
}

/// Helper macro to create a traced span with automatic attributes
///
/// # Example
///
/// ```rust
/// use dashflow_observability::traced_span;
///
/// async fn my_function() {
///     let result = traced_span!("my_operation", {
///         // Your async code here
///         42
///     });
/// }
/// ```
#[macro_export]
macro_rules! traced_span {
    ($name:expr, $body:expr) => {{
        use tracing::Instrument;
        let span = tracing::info_span!($name);
        async move { $body }.instrument(span).await
    }};
    ($name:expr, $($key:ident = $value:expr),+ , $body:expr) => {{
        use tracing::Instrument;
        let span = tracing::info_span!($name, $($key = $value),+);
        async move { $body }.instrument(span).await
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{info, Instrument};

    struct TestService {
        name: String,
    }

    #[async_trait]
    impl Traceable for TestService {
        async fn execute_traced<F, T>(&self, operation: &str, f: F) -> T
        where
            F: Future<Output = T> + Send,
            T: Send,
        {
            let span = tracing::info_span!(
                "test_service.execute",
                service.name = %self.name,
                operation = operation
            );
            f.instrument(span).await
        }
    }

    #[tokio::test]
    async fn test_traceable_trait() {
        // Initialize tracing for test
        let _ = tracing_subscriber::fmt::try_init();

        let service = TestService {
            name: "test".to_string(),
        };

        let result = service
            .execute_traced("test_operation", async {
                info!("Inside traced operation");
                42
            })
            .await;

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_traced_span_macro() {
        let _ = tracing_subscriber::fmt::try_init();

        let result = traced_span!("test_span", 100);
        assert_eq!(result, 100);
    }
}
