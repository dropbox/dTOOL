//! Request handlers for `LangServe` endpoints

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html,
    },
    Json,
};
use dashflow::core::runnable::Runnable;
use futures::stream::{Stream, StreamExt};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    error::{LangServeError, Result},
    playground,
    schema::{
        BatchMetadata, BatchRequest, BatchResponse, InvokeMetadata, InvokeRequest, InvokeResponse,
        StreamRequest,
    },
};

/// State shared across handlers
///
/// The runnable uses `serde_json::Value` for both input and output to enable
/// dynamic typing at the API boundary (similar to Python's dynamic typing).
#[derive(Clone)]
pub struct AppState {
    /// The runnable to serve
    pub runnable: Arc<dyn Runnable<Input = Value, Output = Value>>,

    /// Base URL path for the runnable (e.g., "/`my_runnable`")
    pub base_path: String,
}

/// Handler for /invoke endpoint
///
/// Invokes the runnable with a single input and returns the output.
#[instrument(skip(state, request), fields(base_path = %state.base_path))]
pub async fn invoke_handler(
    State(state): State<AppState>,
    Json(request): Json<InvokeRequest>,
) -> Result<Json<InvokeResponse>> {
    info!("Processing invoke request");
    let start = std::time::Instant::now();

    // Convert RunnableConfig if provided
    let config = request.config.map(|c| {
        let metadata = if let Some(Value::Object(map)) = c.metadata {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        let configurable = if let Some(Value::Object(map)) = c.configurable {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        dashflow::core::config::RunnableConfig {
            tags: c.tags,
            metadata,
            run_name: c.run_name,
            max_concurrency: c.max_concurrency,
            recursion_limit: c.recursion_limit.unwrap_or(25),
            configurable,
            run_id: None,
            callbacks: None, // Callbacks can be added via RunnableConfig if needed
        }
    });

    // Invoke the runnable
    let result = state
        .runnable
        .invoke(request.input, config)
        .await
        .map_err(|e| LangServeError::ExecutionError(e.to_string()));

    let duration = start.elapsed().as_secs_f64();

    match result {
        Ok(output) => {
            crate::metrics::record_request("invoke", duration);
            info!("Invoke request completed successfully in {:.3}s", duration);
            let metadata = InvokeMetadata::new();
            Ok(Json(InvokeResponse { output, metadata }))
        }
        Err(e) => {
            crate::metrics::record_error("invoke", "execution_error");
            error!("Invoke request failed: {}", e);
            Err(e)
        }
    }
}

/// Handler for /batch endpoint
///
/// Invokes the runnable with multiple inputs and returns all outputs.
#[allow(clippy::clone_on_ref_ptr)] // Arc clone needed for concurrent task spawning
#[instrument(skip(state, request), fields(base_path = %state.base_path, batch_size = request.inputs.len()))]
pub async fn batch_handler(
    State(state): State<AppState>,
    Json(request): Json<BatchRequest>,
) -> Result<Json<BatchResponse>> {
    let start = std::time::Instant::now();
    let batch_size = request.inputs.len();

    info!("Processing batch request with {} inputs", batch_size);

    // Record batch size metric
    crate::metrics::record_batch_size(batch_size);

    // Convert RunnableConfig if provided
    let config = request.config.map(|c| {
        let metadata = if let Some(Value::Object(map)) = c.metadata {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        let configurable = if let Some(Value::Object(map)) = c.configurable {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        dashflow::core::config::RunnableConfig {
            tags: c.tags,
            metadata,
            run_name: c.run_name,
            max_concurrency: c.max_concurrency,
            recursion_limit: c.recursion_limit.unwrap_or(25),
            configurable,
            run_id: None,
            callbacks: None, // Callbacks can be added via RunnableConfig if needed
        }
    });

    // Batch invoke the runnable
    // Note: We can't use the default batch() method on trait objects (requires Sized)
    // So we implement concurrent invocation manually
    let tasks: Vec<_> = request
        .inputs
        .into_iter()
        .map(|input| {
            let runnable = state.runnable.clone();
            let config = config.clone();
            async move { runnable.invoke(input, config).await }
        })
        .collect();

    let results = futures::future::join_all(tasks).await;

    // Collect results, returning error if any failed
    let outputs: std::result::Result<Vec<_>, _> = results.into_iter().collect();

    let duration = start.elapsed().as_secs_f64();

    match outputs {
        Ok(outputs) => {
            crate::metrics::record_request("batch", duration);
            info!(
                "Batch request completed successfully in {:.3}s, {} outputs",
                duration,
                outputs.len()
            );
            let metadata = BatchMetadata::new(outputs.len());
            Ok(Json(BatchResponse {
                output: outputs,
                metadata,
            }))
        }
        Err(e) => {
            crate::metrics::record_error("batch", "execution_error");
            error!("Batch request failed: {}", e);
            Err(LangServeError::ExecutionError(e.to_string()))
        }
    }
}

/// Handler for /stream endpoint
///
/// Streams output from the runnable as Server-Sent Events (SSE).
#[instrument(skip(state, request), fields(base_path = %state.base_path))]
pub async fn stream_handler(
    State(state): State<AppState>,
    Json(request): Json<StreamRequest>,
) -> std::result::Result<
    Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>,
    LangServeError,
> {
    info!("Processing stream request");
    let start = std::time::Instant::now();

    // Convert RunnableConfig if provided
    let config = request.config.map(|c| {
        let metadata = if let Some(Value::Object(map)) = c.metadata {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        let configurable = if let Some(Value::Object(map)) = c.configurable {
            map.into_iter().collect()
        } else {
            Default::default()
        };

        dashflow::core::config::RunnableConfig {
            tags: c.tags,
            metadata,
            run_name: c.run_name,
            max_concurrency: c.max_concurrency,
            recursion_limit: c.recursion_limit.unwrap_or(25),
            configurable,
            run_id: None,
            callbacks: None, // Callbacks can be added via RunnableConfig if needed
        }
    });

    // Get the stream from the runnable
    let mut stream = state
        .runnable
        .stream(request.input, config)
        .await
        .map_err(|e| {
            crate::metrics::record_error("stream", "execution_error");
            LangServeError::ExecutionError(e.to_string())
        })?;

    // Convert the runnable stream into an SSE stream
    let metadata = InvokeMetadata::new();
    let run_id = metadata.run_id;

    let sse_stream = async_stream::stream! {
        let mut chunk_count = 0usize;
        let mut had_error = false;

        // Send data events
        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    chunk_count += 1;
                    let data = json!({
                        "output": output,
                    });
                    if let Ok(event) = Event::default()
                        .event("data")
                        .json_data(data)
                    {
                        yield Ok(event);
                    }
                }
                Err(e) => {
                    had_error = true;
                    crate::metrics::record_error("stream", "stream_error");
                    error!("Stream error: {}", e);

                    // Send error event and stop
                    let error_data = json!({
                        "error": e.to_string(),
                    });
                    if let Ok(event) = Event::default()
                        .event("error")
                        .json_data(error_data)
                    {
                        yield Ok(event);
                    }
                    break;
                }
            }
        }

        // Record metrics at stream end
        let duration = start.elapsed().as_secs_f64();
        if !had_error {
            crate::metrics::record_request("stream", duration);
            crate::metrics::record_stream_chunks(chunk_count);
            info!("Stream request completed successfully in {:.3}s, {} chunks", duration, chunk_count);
        }

        // Send metadata at the end
        let metadata_data = json!({
            "run_id": run_id,
        });
        if let Ok(event) = Event::default()
            .event("metadata")
            .json_data(metadata_data)
        {
            yield Ok(event);
        }

        // Send end event
        let event = Event::default()
            .event("end")
            .data("{}");
        yield Ok(event);
    };

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

/// Handler for /`input_schema` endpoint
///
/// Returns a JSON Schema describing the expected input format.
/// Since our runnables use `serde_json::Value` for dynamic typing,
/// we return a schema that accepts any valid JSON.
pub async fn input_schema_handler(State(_state): State<AppState>) -> Result<Json<Value>> {
    // Since we use serde_json::Value for input (dynamic typing like Python),
    // we return a generic schema that accepts any JSON value.
    // Specific runnables could override this in the future.
    let schema = json!({
        "title": "Input",
        "type": "object",
        "properties": {},
        "description": "Input to the runnable. Accepts any JSON value."
    });

    Ok(Json(schema))
}

/// Handler for /`output_schema` endpoint
///
/// Returns a JSON Schema describing the expected output format.
/// Since our runnables use `serde_json::Value` for dynamic typing,
/// we return a schema that accepts any valid JSON.
pub async fn output_schema_handler(State(_state): State<AppState>) -> Result<Json<Value>> {
    // Since we use serde_json::Value for output (dynamic typing like Python),
    // we return a generic schema that accepts any JSON value.
    // Specific runnables could override this in the future.
    let schema = json!({
        "title": "Output",
        "type": "object",
        "properties": {},
        "description": "Output from the runnable. Can be any JSON value."
    });

    Ok(Json(schema))
}

/// Handler for /`config_schema` endpoint
///
/// Returns a JSON Schema describing the configuration options.
pub async fn config_schema_handler(State(_state): State<AppState>) -> Result<Json<Value>> {
    // Generate schema for RunnableConfig based on our schema definition
    let schema = json!({
        "title": "RunnableConfig",
        "type": "object",
        "properties": {
            "tags": {
                "title": "Tags",
                "description": "Tags for this run",
                "default": [],
                "type": "array",
                "items": {
                    "type": "string"
                }
            },
            "metadata": {
                "title": "Metadata",
                "description": "Metadata for this run",
                "type": "object"
            },
            "run_name": {
                "title": "Run Name",
                "description": "Name for this run",
                "type": "string"
            },
            "max_concurrency": {
                "title": "Max Concurrency",
                "description": "Maximum concurrency for this run",
                "type": "integer"
            },
            "recursion_limit": {
                "title": "Recursion Limit",
                "description": "Maximum recursion depth",
                "type": "integer",
                "default": 25
            },
            "configurable": {
                "title": "Configurable",
                "description": "Configurable fields (extensible)",
                "type": "object"
            }
        }
    });

    Ok(Json(schema))
}

/// Handler for /playground endpoint
///
/// Returns an interactive HTML playground UI for testing the runnable.
pub async fn playground_handler(State(state): State<AppState>) -> Html<String> {
    let html = playground::get_playground_html(&state.base_path);
    Html(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::Error as CoreError;
    use futures::stream;
    use std::pin::Pin;

    // Simple test runnable that echoes input
    struct TestRunnable;

    #[async_trait::async_trait]
    impl Runnable for TestRunnable {
        type Input = Value;
        type Output = Value;

        async fn invoke(
            &self,
            input: Self::Input,
            _config: Option<dashflow::core::config::RunnableConfig>,
        ) -> std::result::Result<Self::Output, CoreError> {
            Ok(input)
        }

        async fn batch(
            &self,
            inputs: Vec<Self::Input>,
            _config: Option<dashflow::core::config::RunnableConfig>,
        ) -> std::result::Result<Vec<Self::Output>, CoreError> {
            Ok(inputs)
        }

        async fn stream(
            &self,
            input: Self::Input,
            _config: Option<dashflow::core::config::RunnableConfig>,
        ) -> std::result::Result<
            Pin<Box<dyn Stream<Item = std::result::Result<Self::Output, CoreError>> + Send>>,
            CoreError,
        > {
            let stream = stream::once(async move { Ok(input) });
            Ok(Box::pin(stream))
        }
    }

    fn create_test_state() -> AppState {
        AppState {
            runnable: Arc::new(TestRunnable),
            base_path: "/test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_input_schema_handler() {
        let state = create_test_state();
        let result = input_schema_handler(State(state)).await;

        assert!(result.is_ok());
        let Json(schema) = result.unwrap();

        // Verify schema structure
        assert_eq!(schema["title"], "Input");
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    #[tokio::test]
    async fn test_output_schema_handler() {
        let state = create_test_state();
        let result = output_schema_handler(State(state)).await;

        assert!(result.is_ok());
        let Json(schema) = result.unwrap();

        // Verify schema structure
        assert_eq!(schema["title"], "Output");
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    #[tokio::test]
    async fn test_config_schema_handler() {
        let state = create_test_state();
        let result = config_schema_handler(State(state)).await;

        assert!(result.is_ok());
        let Json(schema) = result.unwrap();

        // Verify schema structure
        assert_eq!(schema["title"], "RunnableConfig");
        assert_eq!(schema["type"], "object");

        // Check all expected properties exist
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("tags"));
        assert!(properties.contains_key("metadata"));
        assert!(properties.contains_key("run_name"));
        assert!(properties.contains_key("max_concurrency"));
        assert!(properties.contains_key("recursion_limit"));
        assert!(properties.contains_key("configurable"));

        // Verify specific property details
        assert_eq!(properties["tags"]["type"], "array");
        assert_eq!(properties["tags"]["items"]["type"], "string");
        assert_eq!(properties["metadata"]["type"], "object");
        assert_eq!(properties["recursion_limit"]["default"], 25);
    }
}
