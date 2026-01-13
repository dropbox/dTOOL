//! Request and response schemas for `LangServe` API

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Request for /invoke endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    /// The input to pass to the runnable
    pub input: Value,

    /// Optional configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RunnableConfig>,

    /// Additional keyword arguments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kwargs: Option<Value>,
}

/// Response from /invoke endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    /// The output from the runnable
    pub output: Value,

    /// Metadata about the invocation
    pub metadata: InvokeMetadata,
}

/// Request for /batch endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    /// The inputs to pass to the runnable
    pub inputs: Vec<Value>,

    /// Optional configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RunnableConfig>,

    /// Optional per-input configurations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs: Option<Vec<RunnableConfig>>,

    /// Additional keyword arguments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kwargs: Option<Value>,
}

/// Response from /batch endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    /// The outputs from the runnable
    pub output: Vec<Value>,

    /// Metadata about the batch invocation
    pub metadata: BatchMetadata,
}

/// Request for /stream endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRequest {
    /// The input to pass to the runnable
    pub input: Value,

    /// Optional configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RunnableConfig>,

    /// Additional keyword arguments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kwargs: Option<Value>,
}

/// Runnable configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunnableConfig {
    /// Tags for this run
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Metadata for this run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,

    /// Run name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_name: Option<String>,

    /// Maximum concurrency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<usize>,

    /// Recursion limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursion_limit: Option<usize>,

    /// Configurable fields (extensible)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configurable: Option<Value>,
}

/// Metadata for a single invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeMetadata {
    /// Unique run ID
    pub run_id: Uuid,

    /// Feedback tokens (for tracing systems)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback_tokens: Vec<String>,
}

/// Metadata for a batch invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// Run IDs for each input
    pub run_ids: Vec<Uuid>,
}

/// Schema endpoint response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaResponse {
    /// JSON schema
    pub schema: Value,
}

/// Server-sent event for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Event type (data, metadata, error, end)
    pub event: String,

    /// Event data
    pub data: Value,
}

impl InvokeMetadata {
    /// Create new metadata with a generated run ID
    #[must_use]
    pub fn new() -> Self {
        Self {
            run_id: Uuid::new_v4(),
            feedback_tokens: Vec::new(),
        }
    }

    /// Create metadata with a specific run ID
    #[must_use]
    pub fn with_run_id(run_id: Uuid) -> Self {
        Self {
            run_id,
            feedback_tokens: Vec::new(),
        }
    }
}

impl Default for InvokeMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchMetadata {
    /// Create new batch metadata with the given number of runs
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            run_ids: (0..count).map(|_| Uuid::new_v4()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoke_request_serialization() {
        let req = InvokeRequest {
            input: Value::String("test".to_string()),
            config: None,
            kwargs: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: InvokeRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(req.input, deserialized.input);
    }

    #[test]
    fn test_invoke_metadata_generation() {
        let metadata = InvokeMetadata::new();
        assert!(!metadata.run_id.is_nil());
    }

    #[test]
    fn test_batch_metadata_generation() {
        let metadata = BatchMetadata::new(5);
        assert_eq!(metadata.run_ids.len(), 5);

        // All UUIDs should be unique
        let unique_ids: std::collections::HashSet<_> = metadata.run_ids.iter().collect();
        assert_eq!(unique_ids.len(), 5);
    }

    // ==================== InvokeRequest Tests ====================

    #[test]
    fn test_invoke_request_with_config() {
        let req = InvokeRequest {
            input: Value::String("test".to_string()),
            config: Some(RunnableConfig::default()),
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: InvokeRequest = serde_json::from_str(&json).unwrap();
        assert!(deser.config.is_some());
    }

    #[test]
    fn test_invoke_request_with_kwargs() {
        let req = InvokeRequest {
            input: Value::Null,
            config: None,
            kwargs: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("kwargs"));
    }

    #[test]
    fn test_invoke_request_minimal_json() {
        let json = r#"{"input": 42}"#;
        let req: InvokeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.input, Value::Number(42.into()));
        assert!(req.config.is_none());
        assert!(req.kwargs.is_none());
    }

    #[test]
    fn test_invoke_request_complex_input() {
        let req = InvokeRequest {
            input: serde_json::json!({
                "text": "hello",
                "numbers": [1, 2, 3],
                "nested": {"a": "b"}
            }),
            config: None,
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: InvokeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.input, deser.input);
    }

    #[test]
    fn test_invoke_request_debug() {
        let req = InvokeRequest {
            input: Value::String("debug test".to_string()),
            config: None,
            kwargs: None,
        };
        let debug = format!("{:?}", req);
        assert!(debug.contains("InvokeRequest"));
        assert!(debug.contains("debug test"));
    }

    #[test]
    fn test_invoke_request_clone() {
        let req = InvokeRequest {
            input: Value::String("clone test".to_string()),
            config: Some(RunnableConfig::default()),
            kwargs: Some(serde_json::json!({})),
        };
        let cloned = req.clone();
        assert_eq!(req.input, cloned.input);
    }

    // ==================== InvokeResponse Tests ====================

    #[test]
    fn test_invoke_response_serialization() {
        let resp = InvokeResponse {
            output: Value::String("result".to_string()),
            metadata: InvokeMetadata::new(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("result"));
        assert!(json.contains("run_id"));
    }

    #[test]
    fn test_invoke_response_debug() {
        let resp = InvokeResponse {
            output: Value::Null,
            metadata: InvokeMetadata::new(),
        };
        let debug = format!("{:?}", resp);
        assert!(debug.contains("InvokeResponse"));
    }

    #[test]
    fn test_invoke_response_clone() {
        let resp = InvokeResponse {
            output: Value::Number(42.into()),
            metadata: InvokeMetadata::new(),
        };
        let cloned = resp.clone();
        assert_eq!(resp.output, cloned.output);
    }

    // ==================== BatchRequest Tests ====================

    #[test]
    fn test_batch_request_empty_inputs() {
        let req = BatchRequest {
            inputs: vec![],
            config: None,
            configs: None,
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: BatchRequest = serde_json::from_str(&json).unwrap();
        assert!(deser.inputs.is_empty());
    }

    #[test]
    fn test_batch_request_multiple_inputs() {
        let req = BatchRequest {
            inputs: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
            config: None,
            configs: None,
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: BatchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.inputs.len(), 3);
    }

    #[test]
    fn test_batch_request_with_configs() {
        let req = BatchRequest {
            inputs: vec![Value::Null, Value::Null],
            config: None,
            configs: Some(vec![RunnableConfig::default(), RunnableConfig::default()]),
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("configs"));
    }

    #[test]
    fn test_batch_request_minimal_json() {
        let json = r#"{"inputs": [1, 2, 3]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.inputs.len(), 3);
    }

    #[test]
    fn test_batch_request_debug() {
        let req = BatchRequest {
            inputs: vec![Value::Null],
            config: None,
            configs: None,
            kwargs: None,
        };
        let debug = format!("{:?}", req);
        assert!(debug.contains("BatchRequest"));
    }

    // ==================== BatchResponse Tests ====================

    #[test]
    fn test_batch_response_serialization() {
        let resp = BatchResponse {
            output: vec![Value::String("a".to_string()), Value::String("b".to_string())],
            metadata: BatchMetadata::new(2),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser: BatchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.output.len(), 2);
        assert_eq!(deser.metadata.run_ids.len(), 2);
    }

    #[test]
    fn test_batch_response_empty() {
        let resp = BatchResponse {
            output: vec![],
            metadata: BatchMetadata::new(0),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser: BatchResponse = serde_json::from_str(&json).unwrap();
        assert!(deser.output.is_empty());
        assert!(deser.metadata.run_ids.is_empty());
    }

    #[test]
    fn test_batch_response_debug() {
        let resp = BatchResponse {
            output: vec![],
            metadata: BatchMetadata::new(0),
        };
        let debug = format!("{:?}", resp);
        assert!(debug.contains("BatchResponse"));
    }

    // ==================== StreamRequest Tests ====================

    #[test]
    fn test_stream_request_serialization() {
        let req = StreamRequest {
            input: Value::String("stream me".to_string()),
            config: None,
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: StreamRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.input, deser.input);
    }

    #[test]
    fn test_stream_request_minimal_json() {
        let json = r#"{"input": "hello"}"#;
        let req: StreamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.input, Value::String("hello".to_string()));
    }

    #[test]
    fn test_stream_request_with_config() {
        let req = StreamRequest {
            input: Value::Null,
            config: Some(RunnableConfig {
                tags: vec!["streaming".to_string()],
                ..Default::default()
            }),
            kwargs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("streaming"));
    }

    #[test]
    fn test_stream_request_debug() {
        let req = StreamRequest {
            input: Value::Null,
            config: None,
            kwargs: None,
        };
        let debug = format!("{:?}", req);
        assert!(debug.contains("StreamRequest"));
    }

    // ==================== RunnableConfig Tests ====================

    #[test]
    fn test_runnable_config_default() {
        let config = RunnableConfig::default();
        assert!(config.tags.is_empty());
        assert!(config.metadata.is_none());
        assert!(config.run_name.is_none());
        assert!(config.max_concurrency.is_none());
        assert!(config.recursion_limit.is_none());
        assert!(config.configurable.is_none());
    }

    #[test]
    fn test_runnable_config_full() {
        let config = RunnableConfig {
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            metadata: Some(serde_json::json!({"key": "value"})),
            run_name: Some("test_run".to_string()),
            max_concurrency: Some(10),
            recursion_limit: Some(50),
            configurable: Some(serde_json::json!({"custom": true})),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deser: RunnableConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.tags.len(), 2);
        assert_eq!(deser.run_name, Some("test_run".to_string()));
        assert_eq!(deser.max_concurrency, Some(10));
        assert_eq!(deser.recursion_limit, Some(50));
    }

    #[test]
    fn test_runnable_config_skip_serializing_if_empty() {
        let config = RunnableConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("tags"));
        assert!(!json.contains("metadata"));
    }

    #[test]
    fn test_runnable_config_debug() {
        let config = RunnableConfig {
            run_name: Some("debug_test".to_string()),
            ..Default::default()
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("RunnableConfig"));
        assert!(debug.contains("debug_test"));
    }

    #[test]
    fn test_runnable_config_clone() {
        let config = RunnableConfig {
            tags: vec!["clone".to_string()],
            max_concurrency: Some(5),
            ..Default::default()
        };
        let cloned = config.clone();
        assert_eq!(config.tags, cloned.tags);
        assert_eq!(config.max_concurrency, cloned.max_concurrency);
    }

    // ==================== InvokeMetadata Tests ====================

    #[test]
    fn test_invoke_metadata_with_run_id() {
        let uuid = Uuid::new_v4();
        let metadata = InvokeMetadata::with_run_id(uuid);
        assert_eq!(metadata.run_id, uuid);
        assert!(metadata.feedback_tokens.is_empty());
    }

    #[test]
    fn test_invoke_metadata_default() {
        let metadata = InvokeMetadata::default();
        assert!(!metadata.run_id.is_nil());
    }

    #[test]
    fn test_invoke_metadata_uniqueness() {
        let m1 = InvokeMetadata::new();
        let m2 = InvokeMetadata::new();
        assert_ne!(m1.run_id, m2.run_id);
    }

    #[test]
    fn test_invoke_metadata_serialization() {
        let metadata = InvokeMetadata {
            run_id: Uuid::nil(),
            feedback_tokens: vec!["token1".to_string(), "token2".to_string()],
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("token1"));
        assert!(json.contains("token2"));
    }

    #[test]
    fn test_invoke_metadata_empty_tokens_skip() {
        let metadata = InvokeMetadata::new();
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(!json.contains("feedback_tokens"));
    }

    #[test]
    fn test_invoke_metadata_debug() {
        let metadata = InvokeMetadata::new();
        let debug = format!("{:?}", metadata);
        assert!(debug.contains("InvokeMetadata"));
        assert!(debug.contains("run_id"));
    }

    #[test]
    fn test_invoke_metadata_clone() {
        let metadata = InvokeMetadata {
            run_id: Uuid::new_v4(),
            feedback_tokens: vec!["a".to_string()],
        };
        let cloned = metadata.clone();
        assert_eq!(metadata.run_id, cloned.run_id);
        assert_eq!(metadata.feedback_tokens, cloned.feedback_tokens);
    }

    // ==================== BatchMetadata Tests ====================

    #[test]
    fn test_batch_metadata_empty() {
        let metadata = BatchMetadata::new(0);
        assert!(metadata.run_ids.is_empty());
    }

    #[test]
    fn test_batch_metadata_large() {
        let metadata = BatchMetadata::new(100);
        assert_eq!(metadata.run_ids.len(), 100);
        // All unique
        let unique: std::collections::HashSet<_> = metadata.run_ids.iter().collect();
        assert_eq!(unique.len(), 100);
    }

    #[test]
    fn test_batch_metadata_serialization() {
        let metadata = BatchMetadata::new(3);
        let json = serde_json::to_string(&metadata).unwrap();
        let deser: BatchMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.run_ids.len(), 3);
    }

    #[test]
    fn test_batch_metadata_debug() {
        let metadata = BatchMetadata::new(2);
        let debug = format!("{:?}", metadata);
        assert!(debug.contains("BatchMetadata"));
        assert!(debug.contains("run_ids"));
    }

    // ==================== SchemaResponse Tests ====================

    #[test]
    fn test_schema_response_serialization() {
        let resp = SchemaResponse {
            schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("type"));
        assert!(json.contains("object"));
    }

    #[test]
    fn test_schema_response_debug() {
        let resp = SchemaResponse {
            schema: Value::Null,
        };
        let debug = format!("{:?}", resp);
        assert!(debug.contains("SchemaResponse"));
    }

    #[test]
    fn test_schema_response_clone() {
        let resp = SchemaResponse {
            schema: serde_json::json!({"key": "value"}),
        };
        let cloned = resp.clone();
        assert_eq!(resp.schema, cloned.schema);
    }

    // ==================== StreamEvent Tests ====================

    #[test]
    fn test_stream_event_data() {
        let event = StreamEvent {
            event: "data".to_string(),
            data: Value::String("chunk".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("data"));
        assert!(json.contains("chunk"));
    }

    #[test]
    fn test_stream_event_error() {
        let event = StreamEvent {
            event: "error".to_string(),
            data: serde_json::json!({"message": "something went wrong"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("something went wrong"));
    }

    #[test]
    fn test_stream_event_end() {
        let event = StreamEvent {
            event: "end".to_string(),
            data: serde_json::json!({}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("end"));
    }

    #[test]
    fn test_stream_event_metadata() {
        let event = StreamEvent {
            event: "metadata".to_string(),
            data: serde_json::json!({"run_id": "abc123"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("metadata"));
        assert!(json.contains("run_id"));
    }

    #[test]
    fn test_stream_event_debug() {
        let event = StreamEvent {
            event: "debug".to_string(),
            data: Value::Null,
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("StreamEvent"));
    }

    #[test]
    fn test_stream_event_clone() {
        let event = StreamEvent {
            event: "data".to_string(),
            data: Value::Number(42.into()),
        };
        let cloned = event.clone();
        assert_eq!(event.event, cloned.event);
        assert_eq!(event.data, cloned.data);
    }

    #[test]
    fn test_stream_event_roundtrip() {
        let original = StreamEvent {
            event: "custom".to_string(),
            data: serde_json::json!({"nested": {"value": [1, 2, 3]}}),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deser: StreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(original.event, deser.event);
        assert_eq!(original.data, deser.data);
    }
}
