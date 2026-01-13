//! Stream event types for runnable execution monitoring
//!
//! This module provides types for streaming events during runnable execution,
//! enabling real-time visibility into execution flow.

use std::collections::HashMap;

/// Event types for `stream_events()` API
///
/// Events are emitted as runnables execute, providing real-time visibility into
/// the execution flow. Event names follow the pattern: `on_{type}_{stage}`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEventType {
    /// Chain/Runnable started
    ChainStart,
    /// Chain/Runnable streaming output
    ChainStream,
    /// Chain/Runnable ended successfully
    ChainEnd,
    /// Chat model started
    ChatModelStart,
    /// Chat model streaming tokens
    ChatModelStream,
    /// Chat model ended
    ChatModelEnd,
    /// LLM started
    LlmStart,
    /// LLM streaming tokens
    LlmStream,
    /// LLM ended
    LlmEnd,
    /// Tool started
    ToolStart,
    /// Tool streaming output
    ToolStream,
    /// Tool ended
    ToolEnd,
    /// Prompt started
    PromptStart,
    /// Prompt ended
    PromptEnd,
    /// Retriever started
    RetrieverStart,
    /// Retriever ended
    RetrieverEnd,
    /// Custom event (user-defined)
    Custom(String),
}

impl StreamEventType {
    /// Get the event name string
    #[must_use]
    pub fn event_name(&self) -> String {
        match self {
            StreamEventType::ChainStart => "on_chain_start".to_string(),
            StreamEventType::ChainStream => "on_chain_stream".to_string(),
            StreamEventType::ChainEnd => "on_chain_end".to_string(),
            StreamEventType::ChatModelStart => "on_chat_model_start".to_string(),
            StreamEventType::ChatModelStream => "on_chat_model_stream".to_string(),
            StreamEventType::ChatModelEnd => "on_chat_model_end".to_string(),
            StreamEventType::LlmStart => "on_llm_start".to_string(),
            StreamEventType::LlmStream => "on_llm_stream".to_string(),
            StreamEventType::LlmEnd => "on_llm_end".to_string(),
            StreamEventType::ToolStart => "on_tool_start".to_string(),
            StreamEventType::ToolStream => "on_tool_stream".to_string(),
            StreamEventType::ToolEnd => "on_tool_end".to_string(),
            StreamEventType::PromptStart => "on_prompt_start".to_string(),
            StreamEventType::PromptEnd => "on_prompt_end".to_string(),
            StreamEventType::RetrieverStart => "on_retriever_start".to_string(),
            StreamEventType::RetrieverEnd => "on_retriever_end".to_string(),
            StreamEventType::Custom(name) => format!("on_custom_{name}"),
        }
    }

    /// Get the type name for filtering (e.g., "chain", "llm", "tool")
    #[must_use]
    pub fn get_type_name(&self) -> String {
        match self {
            StreamEventType::ChainStart
            | StreamEventType::ChainStream
            | StreamEventType::ChainEnd => "chain".to_string(),
            StreamEventType::ChatModelStart
            | StreamEventType::ChatModelStream
            | StreamEventType::ChatModelEnd => "chat_model".to_string(),
            StreamEventType::LlmStart | StreamEventType::LlmStream | StreamEventType::LlmEnd => {
                "llm".to_string()
            }
            StreamEventType::ToolStart | StreamEventType::ToolStream | StreamEventType::ToolEnd => {
                "tool".to_string()
            }
            StreamEventType::PromptStart | StreamEventType::PromptEnd => "prompt".to_string(),
            StreamEventType::RetrieverStart | StreamEventType::RetrieverEnd => {
                "retriever".to_string()
            }
            StreamEventType::Custom(_) => "custom".to_string(),
        }
    }
}

/// Data associated with a stream event
///
/// The data varies depending on the event type:
/// - Start events: contain input data
/// - Stream events: contain chunks/deltas
/// - End events: contain final output
#[derive(Debug, Clone)]
pub enum StreamEventData {
    /// Input data (for start events)
    Input(serde_json::Value),
    /// Output data (for end events)
    Output(serde_json::Value),
    /// Chunk/delta data (for stream events)
    Chunk(serde_json::Value),
    /// Error information
    Error(String),
    /// Empty data
    Empty,
}

/// Version of the `stream_events` API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamEventsVersion {
    /// Version 1 (legacy, deprecated)
    V1,
    /// Version 2 (current, includes `parent_ids`)
    #[default]
    V2,
}

/// Options for filtering events in `stream_events()`
///
/// Matches Python `DashFlow`'s `astream_events()` filtering parameters.
/// All filters are `ANDed` together (event must pass all specified filters).
#[derive(Debug, Clone, Default)]
pub struct StreamEventsOptions {
    /// API version (v1 or v2, default: v2)
    pub version: StreamEventsVersion,
    /// Include only events from runnables with these names
    pub include_names: Option<Vec<String>>,
    /// Include only events of these types (e.g., "chain", "llm", "tool")
    pub include_types: Option<Vec<String>>,
    /// Include only events with these tags
    pub include_tags: Option<Vec<String>>,
    /// Exclude events from runnables with these names
    pub exclude_names: Option<Vec<String>>,
    /// Exclude events of these types
    pub exclude_types: Option<Vec<String>>,
    /// Exclude events with these tags
    pub exclude_tags: Option<Vec<String>>,
}

impl StreamEventsOptions {
    /// Create default options (v2, no filters)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create options with specified version
    #[must_use]
    pub fn with_version(version: StreamEventsVersion) -> Self {
        Self {
            version,
            ..Default::default()
        }
    }

    /// Add name filter (include)
    pub fn include_name(mut self, name: impl Into<String>) -> Self {
        self.include_names
            .get_or_insert_with(Vec::new)
            .push(name.into());
        self
    }

    /// Add type filter (include)
    pub fn include_type(mut self, type_name: impl Into<String>) -> Self {
        self.include_types
            .get_or_insert_with(Vec::new)
            .push(type_name.into());
        self
    }

    /// Add tag filter (include)
    pub fn include_tag(mut self, tag: impl Into<String>) -> Self {
        self.include_tags
            .get_or_insert_with(Vec::new)
            .push(tag.into());
        self
    }

    /// Add name filter (exclude)
    pub fn exclude_name(mut self, name: impl Into<String>) -> Self {
        self.exclude_names
            .get_or_insert_with(Vec::new)
            .push(name.into());
        self
    }

    /// Add type filter (exclude)
    pub fn exclude_type(mut self, type_name: impl Into<String>) -> Self {
        self.exclude_types
            .get_or_insert_with(Vec::new)
            .push(type_name.into());
        self
    }

    /// Add tag filter (exclude)
    pub fn exclude_tag(mut self, tag: impl Into<String>) -> Self {
        self.exclude_tags
            .get_or_insert_with(Vec::new)
            .push(tag.into());
        self
    }

    /// Check if an event should be included based on filters
    #[must_use]
    pub fn should_include(&self, event: &StreamEvent) -> bool {
        // Check include_names
        if let Some(ref names) = self.include_names {
            if !names.contains(&event.name) {
                return false;
            }
        }

        // Check include_types
        if let Some(ref types) = self.include_types {
            let event_type = event.event_type.get_type_name();
            if !types.contains(&event_type) {
                return false;
            }
        }

        // Check include_tags
        if let Some(ref tags) = self.include_tags {
            if !event.tags.iter().any(|t| tags.contains(t)) {
                return false;
            }
        }

        // Check exclude_names
        if let Some(ref names) = self.exclude_names {
            if names.contains(&event.name) {
                return false;
            }
        }

        // Check exclude_types
        if let Some(ref types) = self.exclude_types {
            let event_type = event.event_type.get_type_name();
            if types.contains(&event_type) {
                return false;
            }
        }

        // Check exclude_tags
        if let Some(ref tags) = self.exclude_tags {
            if event.tags.iter().any(|t| tags.contains(t)) {
                return false;
            }
        }

        true
    }
}

/// A streaming event emitted during runnable execution
///
/// Follows Python `DashFlow`'s `StreamEvent` schema, providing real-time visibility
/// into execution flow.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::{Runnable, StreamEvent};
/// use futures::StreamExt;
///
/// let mut stream = runnable.stream_events(input, None, None).await?;
/// while let Some(event) = stream.next().await {
///     match event.event_type {
///         StreamEventType::ChainStart => {
///             println!("Started: {}", event.name);
///         }
///         StreamEventType::ChainStream => {
///             println!("Chunk: {:?}", event.data);
///         }
///         StreamEventType::ChainEnd => {
///             println!("Completed: {}", event.name);
///         }
///         _ => {}
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StreamEvent {
    /// Event type (determines event name)
    pub event_type: StreamEventType,
    /// Name of the runnable that generated this event
    pub name: String,
    /// Unique ID for this execution run
    pub run_id: uuid::Uuid,
    /// IDs of parent runnables (empty for root)
    pub parent_ids: Vec<uuid::Uuid>,
    /// Tags associated with this runnable
    pub tags: Vec<String>,
    /// Metadata associated with this runnable
    pub metadata: HashMap<String, serde_json::Value>,
    /// Event data (varies by event type)
    pub data: StreamEventData,
}

impl StreamEvent {
    /// Create a new `StreamEvent`
    pub fn new(
        event_type: StreamEventType,
        name: impl Into<String>,
        run_id: uuid::Uuid,
        data: StreamEventData,
    ) -> Self {
        Self {
            event_type,
            name: name.into(),
            run_id,
            parent_ids: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
            data,
        }
    }

    /// Add parent run ID
    #[must_use]
    pub fn with_parent(mut self, parent_id: uuid::Uuid) -> Self {
        self.parent_ids.push(parent_id);
        self
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Get the event name string (e.g., "`on_chain_start`")
    #[must_use]
    pub fn event_name(&self) -> String {
        self.event_type.event_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // StreamEventType Tests
    // ============================================================================

    #[test]
    fn test_stream_event_type_chain_start_event_name() {
        assert_eq!(StreamEventType::ChainStart.event_name(), "on_chain_start");
    }

    #[test]
    fn test_stream_event_type_chain_stream_event_name() {
        assert_eq!(StreamEventType::ChainStream.event_name(), "on_chain_stream");
    }

    #[test]
    fn test_stream_event_type_chain_end_event_name() {
        assert_eq!(StreamEventType::ChainEnd.event_name(), "on_chain_end");
    }

    #[test]
    fn test_stream_event_type_chat_model_start_event_name() {
        assert_eq!(
            StreamEventType::ChatModelStart.event_name(),
            "on_chat_model_start"
        );
    }

    #[test]
    fn test_stream_event_type_chat_model_stream_event_name() {
        assert_eq!(
            StreamEventType::ChatModelStream.event_name(),
            "on_chat_model_stream"
        );
    }

    #[test]
    fn test_stream_event_type_chat_model_end_event_name() {
        assert_eq!(
            StreamEventType::ChatModelEnd.event_name(),
            "on_chat_model_end"
        );
    }

    #[test]
    fn test_stream_event_type_llm_start_event_name() {
        assert_eq!(StreamEventType::LlmStart.event_name(), "on_llm_start");
    }

    #[test]
    fn test_stream_event_type_llm_stream_event_name() {
        assert_eq!(StreamEventType::LlmStream.event_name(), "on_llm_stream");
    }

    #[test]
    fn test_stream_event_type_llm_end_event_name() {
        assert_eq!(StreamEventType::LlmEnd.event_name(), "on_llm_end");
    }

    #[test]
    fn test_stream_event_type_tool_start_event_name() {
        assert_eq!(StreamEventType::ToolStart.event_name(), "on_tool_start");
    }

    #[test]
    fn test_stream_event_type_tool_stream_event_name() {
        assert_eq!(StreamEventType::ToolStream.event_name(), "on_tool_stream");
    }

    #[test]
    fn test_stream_event_type_tool_end_event_name() {
        assert_eq!(StreamEventType::ToolEnd.event_name(), "on_tool_end");
    }

    #[test]
    fn test_stream_event_type_prompt_start_event_name() {
        assert_eq!(StreamEventType::PromptStart.event_name(), "on_prompt_start");
    }

    #[test]
    fn test_stream_event_type_prompt_end_event_name() {
        assert_eq!(StreamEventType::PromptEnd.event_name(), "on_prompt_end");
    }

    #[test]
    fn test_stream_event_type_retriever_start_event_name() {
        assert_eq!(
            StreamEventType::RetrieverStart.event_name(),
            "on_retriever_start"
        );
    }

    #[test]
    fn test_stream_event_type_retriever_end_event_name() {
        assert_eq!(
            StreamEventType::RetrieverEnd.event_name(),
            "on_retriever_end"
        );
    }

    #[test]
    fn test_stream_event_type_custom_event_name() {
        let custom = StreamEventType::Custom("my_event".to_string());
        assert_eq!(custom.event_name(), "on_custom_my_event");
    }

    #[test]
    fn test_stream_event_type_custom_empty_name() {
        let custom = StreamEventType::Custom(String::new());
        assert_eq!(custom.event_name(), "on_custom_");
    }

    #[test]
    fn test_stream_event_type_chain_get_type_name() {
        assert_eq!(StreamEventType::ChainStart.get_type_name(), "chain");
        assert_eq!(StreamEventType::ChainStream.get_type_name(), "chain");
        assert_eq!(StreamEventType::ChainEnd.get_type_name(), "chain");
    }

    #[test]
    fn test_stream_event_type_chat_model_get_type_name() {
        assert_eq!(StreamEventType::ChatModelStart.get_type_name(), "chat_model");
        assert_eq!(
            StreamEventType::ChatModelStream.get_type_name(),
            "chat_model"
        );
        assert_eq!(StreamEventType::ChatModelEnd.get_type_name(), "chat_model");
    }

    #[test]
    fn test_stream_event_type_llm_get_type_name() {
        assert_eq!(StreamEventType::LlmStart.get_type_name(), "llm");
        assert_eq!(StreamEventType::LlmStream.get_type_name(), "llm");
        assert_eq!(StreamEventType::LlmEnd.get_type_name(), "llm");
    }

    #[test]
    fn test_stream_event_type_tool_get_type_name() {
        assert_eq!(StreamEventType::ToolStart.get_type_name(), "tool");
        assert_eq!(StreamEventType::ToolStream.get_type_name(), "tool");
        assert_eq!(StreamEventType::ToolEnd.get_type_name(), "tool");
    }

    #[test]
    fn test_stream_event_type_prompt_get_type_name() {
        assert_eq!(StreamEventType::PromptStart.get_type_name(), "prompt");
        assert_eq!(StreamEventType::PromptEnd.get_type_name(), "prompt");
    }

    #[test]
    fn test_stream_event_type_retriever_get_type_name() {
        assert_eq!(StreamEventType::RetrieverStart.get_type_name(), "retriever");
        assert_eq!(StreamEventType::RetrieverEnd.get_type_name(), "retriever");
    }

    #[test]
    fn test_stream_event_type_custom_get_type_name() {
        let custom = StreamEventType::Custom("anything".to_string());
        assert_eq!(custom.get_type_name(), "custom");
    }

    #[test]
    fn test_stream_event_type_equality() {
        assert_eq!(StreamEventType::ChainStart, StreamEventType::ChainStart);
        assert_ne!(StreamEventType::ChainStart, StreamEventType::ChainEnd);
    }

    #[test]
    fn test_stream_event_type_custom_equality() {
        let custom1 = StreamEventType::Custom("test".to_string());
        let custom2 = StreamEventType::Custom("test".to_string());
        let custom3 = StreamEventType::Custom("other".to_string());
        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
    }

    #[test]
    fn test_stream_event_type_clone() {
        let original = StreamEventType::ChatModelStart;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_stream_event_type_custom_clone() {
        let original = StreamEventType::Custom("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_stream_event_type_debug() {
        let event_type = StreamEventType::LlmStart;
        let debug = format!("{:?}", event_type);
        assert!(debug.contains("LlmStart"));
    }

    // ============================================================================
    // StreamEventData Tests
    // ============================================================================

    #[test]
    fn test_stream_event_data_input() {
        let data = StreamEventData::Input(serde_json::json!({"key": "value"}));
        match data {
            StreamEventData::Input(v) => assert_eq!(v["key"], "value"),
            _ => panic!("Expected Input variant"),
        }
    }

    #[test]
    fn test_stream_event_data_output() {
        let data = StreamEventData::Output(serde_json::json!({"result": 42}));
        match data {
            StreamEventData::Output(v) => assert_eq!(v["result"], 42),
            _ => panic!("Expected Output variant"),
        }
    }

    #[test]
    fn test_stream_event_data_chunk() {
        let data = StreamEventData::Chunk(serde_json::json!({"token": "hello"}));
        match data {
            StreamEventData::Chunk(v) => assert_eq!(v["token"], "hello"),
            _ => panic!("Expected Chunk variant"),
        }
    }

    #[test]
    fn test_stream_event_data_error() {
        let data = StreamEventData::Error("Something went wrong".to_string());
        match data {
            StreamEventData::Error(msg) => assert_eq!(msg, "Something went wrong"),
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn test_stream_event_data_empty() {
        let data = StreamEventData::Empty;
        match data {
            StreamEventData::Empty => {}
            _ => panic!("Expected Empty variant"),
        }
    }

    #[test]
    fn test_stream_event_data_clone() {
        let original = StreamEventData::Input(serde_json::json!({"test": true}));
        let cloned = original.clone();
        match (original, cloned) {
            (StreamEventData::Input(v1), StreamEventData::Input(v2)) => {
                assert_eq!(v1, v2);
            }
            _ => panic!("Clone should preserve variant"),
        }
    }

    #[test]
    fn test_stream_event_data_debug() {
        let data = StreamEventData::Error("test error".to_string());
        let debug = format!("{:?}", data);
        assert!(debug.contains("Error"));
        assert!(debug.contains("test error"));
    }

    // ============================================================================
    // StreamEventsVersion Tests
    // ============================================================================

    #[test]
    fn test_stream_events_version_default_is_v2() {
        let version = StreamEventsVersion::default();
        assert_eq!(version, StreamEventsVersion::V2);
    }

    #[test]
    fn test_stream_events_version_v1() {
        let version = StreamEventsVersion::V1;
        assert_ne!(version, StreamEventsVersion::V2);
    }

    #[test]
    fn test_stream_events_version_equality() {
        assert_eq!(StreamEventsVersion::V1, StreamEventsVersion::V1);
        assert_eq!(StreamEventsVersion::V2, StreamEventsVersion::V2);
        assert_ne!(StreamEventsVersion::V1, StreamEventsVersion::V2);
    }

    #[test]
    fn test_stream_events_version_clone() {
        let original = StreamEventsVersion::V2;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_stream_events_version_debug() {
        let version = StreamEventsVersion::V1;
        let debug = format!("{:?}", version);
        assert!(debug.contains("V1"));
    }

    // ============================================================================
    // StreamEventsOptions Tests
    // ============================================================================

    #[test]
    fn test_stream_events_options_new() {
        let options = StreamEventsOptions::new();
        assert_eq!(options.version, StreamEventsVersion::V2);
        assert!(options.include_names.is_none());
        assert!(options.include_types.is_none());
        assert!(options.include_tags.is_none());
        assert!(options.exclude_names.is_none());
        assert!(options.exclude_types.is_none());
        assert!(options.exclude_tags.is_none());
    }

    #[test]
    fn test_stream_events_options_default() {
        let options = StreamEventsOptions::default();
        assert_eq!(options.version, StreamEventsVersion::V2);
    }

    #[test]
    fn test_stream_events_options_with_version() {
        let options = StreamEventsOptions::with_version(StreamEventsVersion::V1);
        assert_eq!(options.version, StreamEventsVersion::V1);
    }

    #[test]
    fn test_stream_events_options_include_name() {
        let options = StreamEventsOptions::new().include_name("test_runnable");
        assert!(options.include_names.is_some());
        assert!(options.include_names.unwrap().contains(&"test_runnable".to_string()));
    }

    #[test]
    fn test_stream_events_options_include_name_multiple() {
        let options = StreamEventsOptions::new()
            .include_name("runnable1")
            .include_name("runnable2");
        let names = options.include_names.unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"runnable1".to_string()));
        assert!(names.contains(&"runnable2".to_string()));
    }

    #[test]
    fn test_stream_events_options_include_type() {
        let options = StreamEventsOptions::new().include_type("chain");
        assert!(options.include_types.is_some());
        assert!(options.include_types.unwrap().contains(&"chain".to_string()));
    }

    #[test]
    fn test_stream_events_options_include_type_multiple() {
        let options = StreamEventsOptions::new()
            .include_type("chain")
            .include_type("llm");
        let types = options.include_types.unwrap();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"chain".to_string()));
        assert!(types.contains(&"llm".to_string()));
    }

    #[test]
    fn test_stream_events_options_include_tag() {
        let options = StreamEventsOptions::new().include_tag("important");
        assert!(options.include_tags.is_some());
        assert!(options.include_tags.unwrap().contains(&"important".to_string()));
    }

    #[test]
    fn test_stream_events_options_exclude_name() {
        let options = StreamEventsOptions::new().exclude_name("internal");
        assert!(options.exclude_names.is_some());
        assert!(options.exclude_names.unwrap().contains(&"internal".to_string()));
    }

    #[test]
    fn test_stream_events_options_exclude_type() {
        let options = StreamEventsOptions::new().exclude_type("tool");
        assert!(options.exclude_types.is_some());
        assert!(options.exclude_types.unwrap().contains(&"tool".to_string()));
    }

    #[test]
    fn test_stream_events_options_exclude_tag() {
        let options = StreamEventsOptions::new().exclude_tag("debug");
        assert!(options.exclude_tags.is_some());
        assert!(options.exclude_tags.unwrap().contains(&"debug".to_string()));
    }

    #[test]
    fn test_stream_events_options_builder_chain() {
        let options = StreamEventsOptions::with_version(StreamEventsVersion::V1)
            .include_name("runnable1")
            .include_type("chain")
            .include_tag("important")
            .exclude_name("internal")
            .exclude_type("tool")
            .exclude_tag("debug");

        assert_eq!(options.version, StreamEventsVersion::V1);
        assert!(options.include_names.is_some());
        assert!(options.include_types.is_some());
        assert!(options.include_tags.is_some());
        assert!(options.exclude_names.is_some());
        assert!(options.exclude_types.is_some());
        assert!(options.exclude_tags.is_some());
    }

    #[test]
    fn test_stream_events_options_should_include_no_filters() {
        let options = StreamEventsOptions::new();
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_name_match() {
        let options = StreamEventsOptions::new().include_name("my_runnable");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "my_runnable",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_name_no_match() {
        let options = StreamEventsOptions::new().include_name("my_runnable");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "other_runnable",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_type_match() {
        let options = StreamEventsOptions::new().include_type("chain");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_type_no_match() {
        let options = StreamEventsOptions::new().include_type("llm");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_tag_match() {
        let options = StreamEventsOptions::new().include_tag("important");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_tags(vec!["important".to_string(), "other".to_string()]);
        assert!(options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_include_tag_no_match() {
        let options = StreamEventsOptions::new().include_tag("important");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_tags(vec!["other".to_string()]);
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_exclude_name() {
        let options = StreamEventsOptions::new().exclude_name("internal");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "internal",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_exclude_type() {
        let options = StreamEventsOptions::new().exclude_type("tool");
        let event = StreamEvent::new(
            StreamEventType::ToolStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_should_exclude_tag() {
        let options = StreamEventsOptions::new().exclude_tag("debug");
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_tags(vec!["debug".to_string()]);
        assert!(!options.should_include(&event));
    }

    #[test]
    fn test_stream_events_options_and_logic() {
        // Include name AND type must both match
        let options = StreamEventsOptions::new()
            .include_name("my_chain")
            .include_type("chain");

        // Both match
        let event1 = StreamEvent::new(
            StreamEventType::ChainStart,
            "my_chain",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(options.should_include(&event1));

        // Name matches, type doesn't
        let event2 = StreamEvent::new(
            StreamEventType::LlmStart,
            "my_chain",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event2));

        // Type matches, name doesn't
        let event3 = StreamEvent::new(
            StreamEventType::ChainStart,
            "other_chain",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert!(!options.should_include(&event3));
    }

    #[test]
    fn test_stream_events_options_clone() {
        let original = StreamEventsOptions::new()
            .include_name("test")
            .exclude_type("tool");
        let cloned = original.clone();
        assert_eq!(cloned.include_names, Some(vec!["test".to_string()]));
        assert_eq!(cloned.exclude_types, Some(vec!["tool".to_string()]));
    }

    // ============================================================================
    // StreamEvent Tests
    // ============================================================================

    #[test]
    fn test_stream_event_new() {
        let run_id = uuid::Uuid::new_v4();
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "my_chain",
            run_id,
            StreamEventData::Empty,
        );
        assert_eq!(event.event_type, StreamEventType::ChainStart);
        assert_eq!(event.name, "my_chain");
        assert_eq!(event.run_id, run_id);
        assert!(event.parent_ids.is_empty());
        assert!(event.tags.is_empty());
        assert!(event.metadata.is_empty());
    }

    #[test]
    fn test_stream_event_with_parent() {
        let run_id = uuid::Uuid::new_v4();
        let parent_id = uuid::Uuid::new_v4();
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "child",
            run_id,
            StreamEventData::Empty,
        )
        .with_parent(parent_id);

        assert_eq!(event.parent_ids.len(), 1);
        assert_eq!(event.parent_ids[0], parent_id);
    }

    #[test]
    fn test_stream_event_with_multiple_parents() {
        let run_id = uuid::Uuid::new_v4();
        let parent1 = uuid::Uuid::new_v4();
        let parent2 = uuid::Uuid::new_v4();
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "grandchild",
            run_id,
            StreamEventData::Empty,
        )
        .with_parent(parent1)
        .with_parent(parent2);

        assert_eq!(event.parent_ids.len(), 2);
        assert!(event.parent_ids.contains(&parent1));
        assert!(event.parent_ids.contains(&parent2));
    }

    #[test]
    fn test_stream_event_with_tags() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

        assert_eq!(event.tags.len(), 2);
        assert!(event.tags.contains(&"tag1".to_string()));
        assert!(event.tags.contains(&"tag2".to_string()));
    }

    #[test]
    fn test_stream_event_with_empty_tags() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_tags(vec![]);

        assert!(event.tags.is_empty());
    }

    #[test]
    fn test_stream_event_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), serde_json::json!("value1"));
        metadata.insert("key2".to_string(), serde_json::json!(42));

        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        )
        .with_metadata(metadata);

        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata["key1"], "value1");
        assert_eq!(event.metadata["key2"], 42);
    }

    #[test]
    fn test_stream_event_event_name() {
        let event = StreamEvent::new(
            StreamEventType::LlmEnd,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert_eq!(event.event_name(), "on_llm_end");
    }

    #[test]
    fn test_stream_event_with_input_data() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Input(serde_json::json!({"input": "test data"})),
        );

        match event.data {
            StreamEventData::Input(v) => assert_eq!(v["input"], "test data"),
            _ => panic!("Expected Input data"),
        }
    }

    #[test]
    fn test_stream_event_with_output_data() {
        let event = StreamEvent::new(
            StreamEventType::ChainEnd,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Output(serde_json::json!({"output": "result"})),
        );

        match event.data {
            StreamEventData::Output(v) => assert_eq!(v["output"], "result"),
            _ => panic!("Expected Output data"),
        }
    }

    #[test]
    fn test_stream_event_with_chunk_data() {
        let event = StreamEvent::new(
            StreamEventType::ChainStream,
            "test",
            uuid::Uuid::new_v4(),
            StreamEventData::Chunk(serde_json::json!({"token": "hello"})),
        );

        match event.data {
            StreamEventData::Chunk(v) => assert_eq!(v["token"], "hello"),
            _ => panic!("Expected Chunk data"),
        }
    }

    #[test]
    fn test_stream_event_clone() {
        let run_id = uuid::Uuid::new_v4();
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "test",
            run_id,
            StreamEventData::Empty,
        )
        .with_tags(vec!["tag".to_string()]);

        let cloned = event.clone();
        assert_eq!(event.name, cloned.name);
        assert_eq!(event.run_id, cloned.run_id);
        assert_eq!(event.tags, cloned.tags);
    }

    #[test]
    fn test_stream_event_debug() {
        let event = StreamEvent::new(
            StreamEventType::ToolStart,
            "my_tool",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        let debug = format!("{:?}", event);
        assert!(debug.contains("StreamEvent"));
        assert!(debug.contains("my_tool"));
    }

    #[test]
    fn test_stream_event_builder_chain() {
        let run_id = uuid::Uuid::new_v4();
        let parent_id = uuid::Uuid::new_v4();
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), serde_json::json!("1.0"));

        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "complex_chain",
            run_id,
            StreamEventData::Input(serde_json::json!({"query": "test"})),
        )
        .with_parent(parent_id)
        .with_tags(vec!["production".to_string(), "v2".to_string()])
        .with_metadata(metadata);

        assert_eq!(event.name, "complex_chain");
        assert_eq!(event.parent_ids.len(), 1);
        assert_eq!(event.tags.len(), 2);
        assert_eq!(event.metadata.len(), 1);
        match event.data {
            StreamEventData::Input(v) => assert_eq!(v["query"], "test"),
            _ => panic!("Expected Input data"),
        }
    }

    #[test]
    fn test_stream_event_unicode_name() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "链式处理器", // Chinese: Chain processor
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert_eq!(event.name, "链式处理器");
    }

    #[test]
    fn test_stream_event_empty_name() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert_eq!(event.name, "");
    }

    #[test]
    fn test_stream_event_special_chars_in_name() {
        let event = StreamEvent::new(
            StreamEventType::ChainStart,
            "my-chain_v2.0+beta",
            uuid::Uuid::new_v4(),
            StreamEventData::Empty,
        );
        assert_eq!(event.name, "my-chain_v2.0+beta");
    }
}
