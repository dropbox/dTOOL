//! Configuration types for Runnable execution
//!
//! This module provides [`RunnableConfig`] for controlling how Runnables execute.
//! Configuration includes callbacks, tracing, metadata, concurrency limits, and more.
//!
//! # Key Concepts
//!
//! - **Tags**: Strings for filtering/grouping runs in tracing systems
//! - **Metadata**: JSON-serializable key-value pairs attached to runs
//! - **Callbacks**: Event handlers notified during execution (see [`crate::core::callbacks`])
//! - **Recursion Limit**: Maximum depth for recursive calls (default: 25)
//! - **Max Concurrency**: Limit on parallel sub-calls
//!
//! # Examples
//!
//! ## Basic Configuration
//!
//! ```rust
//! use dashflow::core::config::RunnableConfig;
//!
//! let config = RunnableConfig::new()
//!     .with_tag("production")
//!     .with_tag("customer-123")
//!     .with_metadata("user_id", "alice")
//!     .unwrap()
//!     .with_max_concurrency(10);
//! ```
//!
//! ## Loading from YAML
//!
//! ```rust,ignore
//! use dashflow::core::config::RunnableConfig;
//!
//! let yaml = r#"
//! tags:
//!   - production
//!   - api-v1
//! metadata:
//!   environment: prod
//!   region: us-west-2
//! max_concurrency: 20
//! recursion_limit: 10
//! "#;
//!
//! let config: RunnableConfig = serde_yml::from_str(yaml)?;
//! ```
//!
//! ## With Callbacks
//!
//! ```rust,ignore
//! use dashflow::core::config::RunnableConfig;
//! use dashflow::core::callbacks::{CallbackManager, ConsoleCallbackHandler};
//!
//! let callbacks = CallbackManager::new()
//!     .add_handler(ConsoleCallbackHandler);
//!
//! let config = RunnableConfig::new()
//!     .with_callbacks(callbacks);
//!
//! // Pass config to Runnable methods
//! let result = runnable.invoke(input, Some(config)).await?;
//! ```
//!
//! ## Tracing with `LangSmith`
//!
//! ```rust,ignore
//! use dashflow::core::config::RunnableConfig;
//! use dashflow::core::tracers::DashFlowTracer;
//!
//! // Set up LangSmith tracing
//! let tracer = DashFlowTracer::new(
//!     "https://api.smith.dashflow.com",
//!     "your-api-key",
//!     "your-project",
//! )?;
//!
//! let config = RunnableConfig::new()
//!     .with_callbacks(CallbackManager::new().add_handler(tracer))
//!     .with_run_name("customer_query")
//!     .with_tag("production");
//!
//! let result = agent.invoke(query, Some(config)).await?;
//! ```
//!
//! # See Also
//!
//! - [`crate::core::callbacks`] - Callback system for observability
//! - [`crate::core::tracers`] - `LangSmith` tracing integration
//! - [`crate::core::runnable::Runnable`] - Core execution trait

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::core::callbacks::CallbackManager;

/// Configuration for executing a Runnable
///
/// This struct controls runtime behavior, tracing, callbacks, and metadata
/// for Runnable execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnableConfig {
    /// Tags for this call and any sub-calls
    ///
    /// You can use these to filter calls in tracing/logging.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Metadata for this call and any sub-calls
    ///
    /// Keys should be strings, values should be JSON-serializable.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Name for the tracer run for this call
    ///
    /// Defaults to the name of the Runnable class if not provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_name: Option<String>,

    /// Maximum number of parallel calls to make
    ///
    /// If not provided, uses the runtime's default concurrency limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<usize>,

    /// Maximum number of times a call can recurse
    ///
    /// Defaults to 25 if not provided.
    #[serde(default = "default_recursion_limit")]
    pub recursion_limit: usize,

    /// Runtime values for configurable attributes
    ///
    /// Used with `configurable_fields` or `configurable_alternatives`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub configurable: HashMap<String, serde_json::Value>,

    /// Unique identifier for the tracer run
    ///
    /// If not provided, a new UUID will be generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,

    /// Callback manager for this execution
    ///
    /// Contains callback handlers that will be notified of execution events.
    /// Not serialized as callbacks are runtime-only.
    #[serde(skip)]
    pub callbacks: Option<CallbackManager>,
}

const fn default_recursion_limit() -> usize {
    25
}

impl RunnableConfig {
    /// Create a new empty configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tag to this configuration
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags to this configuration
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags
            .extend(tags.into_iter().map(std::convert::Into::into));
        self
    }

    /// Add metadata to this configuration
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> crate::core::Result<Self> {
        let json_value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), json_value);
        Ok(self)
    }

    /// Set the run name
    #[must_use]
    pub fn with_run_name(mut self, name: impl Into<String>) -> Self {
        self.run_name = Some(name.into());
        self
    }

    /// Set the max concurrency
    #[must_use]
    pub const fn with_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = Some(max);
        self
    }

    /// Set the recursion limit
    #[must_use]
    pub const fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    /// Set a configurable value
    pub fn with_configurable(
        mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> crate::core::Result<Self> {
        let json_value = serde_json::to_value(value)?;
        self.configurable.insert(key.into(), json_value);
        Ok(self)
    }

    /// Set the run ID
    #[must_use]
    pub const fn with_run_id(mut self, id: Uuid) -> Self {
        self.run_id = Some(id);
        self
    }

    /// Generate a new run ID if one doesn't exist
    pub fn ensure_run_id(&mut self) -> Uuid {
        *self.run_id.get_or_insert_with(Uuid::new_v4)
    }

    /// Set the callback manager
    #[must_use]
    pub fn with_callbacks(mut self, callbacks: CallbackManager) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    /// Get or create a callback manager
    ///
    /// Returns the existing callback manager, or creates an empty one if none exists.
    #[must_use]
    pub fn get_callback_manager(&self) -> CallbackManager {
        self.callbacks.clone().unwrap_or_default()
    }

    /// Merge another configuration into this one
    ///
    /// Values from `other` take precedence over values in `self`.
    #[must_use]
    pub fn merge(mut self, other: RunnableConfig) -> Self {
        // Merge tags (append)
        self.tags.extend(other.tags);

        // Merge metadata (other overwrites)
        self.metadata.extend(other.metadata);

        // Take other's values if present
        if other.run_name.is_some() {
            self.run_name = other.run_name;
        }
        if other.max_concurrency.is_some() {
            self.max_concurrency = other.max_concurrency;
        }
        if other.recursion_limit != default_recursion_limit() {
            self.recursion_limit = other.recursion_limit;
        }
        self.configurable.extend(other.configurable);
        if other.run_id.is_some() {
            self.run_id = other.run_id;
        }
        if other.callbacks.is_some() {
            self.callbacks = other.callbacks;
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_config_builder() {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_tags(vec!["foo", "bar"])
            .with_run_name("my_run")
            .with_max_concurrency(10)
            .with_recursion_limit(50);

        assert_eq!(config.tags, vec!["test", "foo", "bar"]);
        assert_eq!(config.run_name, Some("my_run".to_string()));
        assert_eq!(config.max_concurrency, Some(10));
        assert_eq!(config.recursion_limit, 50);
    }

    #[test]
    fn test_config_metadata() {
        let config = RunnableConfig::new()
            .with_metadata("key1", "value1")
            .unwrap()
            .with_metadata("key2", 42)
            .unwrap();

        assert_eq!(
            config.metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(config.metadata.get("key2"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_config_merge() {
        let config1 = RunnableConfig::new().with_tag("tag1").with_run_name("run1");

        let config2 = RunnableConfig::new().with_tag("tag2").with_run_name("run2");

        let merged = config1.merge(config2);

        assert_eq!(merged.tags, vec!["tag1", "tag2"]);
        assert_eq!(merged.run_name, Some("run2".to_string())); // config2 takes precedence
    }

    #[test]
    fn test_ensure_run_id() {
        let mut config = RunnableConfig::new();
        assert!(config.run_id.is_none());

        let id1 = config.ensure_run_id();
        assert!(config.run_id.is_some());

        let id2 = config.ensure_run_id();
        assert_eq!(id1, id2); // Should return same ID
    }

    #[test]
    fn test_serialization() {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_run_name("my_run")
            .with_max_concurrency(5);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RunnableConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.tags, deserialized.tags);
        assert_eq!(config.run_name, deserialized.run_name);
        assert_eq!(config.max_concurrency, deserialized.max_concurrency);
    }

    #[test]
    fn test_default_config() {
        let config = RunnableConfig::new();

        assert!(config.tags.is_empty());
        assert!(config.metadata.is_empty());
        assert!(config.run_name.is_none());
        assert!(config.max_concurrency.is_none());
        assert_eq!(config.recursion_limit, 0); // Default value from derive(Default)
        assert!(config.configurable.is_empty());
        assert!(config.run_id.is_none());
        assert!(config.callbacks.is_none());
    }

    #[test]
    fn test_default_trait() {
        let config = RunnableConfig::default();
        assert_eq!(config.recursion_limit, 0); // Default from derive(Default)
        assert!(config.tags.is_empty());
    }

    #[test]
    fn test_clone() {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_run_name("my_run")
            .with_max_concurrency(10);

        let cloned = config.clone();

        assert_eq!(config.tags, cloned.tags);
        assert_eq!(config.run_name, cloned.run_name);
        assert_eq!(config.max_concurrency, cloned.max_concurrency);
    }

    #[test]
    fn test_with_configurable() {
        let config = RunnableConfig::new()
            .with_configurable("model", "gpt-4")
            .unwrap()
            .with_configurable("temperature", 0.7)
            .unwrap();

        assert_eq!(
            config.configurable.get("model"),
            Some(&serde_json::json!("gpt-4"))
        );
        assert_eq!(
            config.configurable.get("temperature"),
            Some(&serde_json::json!(0.7))
        );
    }

    #[test]
    fn test_with_run_id() {
        let id = Uuid::new_v4();
        let config = RunnableConfig::new().with_run_id(id);

        assert_eq!(config.run_id, Some(id));
    }

    #[test]
    fn test_with_callbacks() {
        let callbacks = CallbackManager::new();
        let config = RunnableConfig::new().with_callbacks(callbacks);

        assert!(config.callbacks.is_some());
    }

    #[test]
    fn test_get_callback_manager_none() {
        let config = RunnableConfig::new();
        let manager = config.get_callback_manager();

        // Should return a default manager
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_get_callback_manager_some() {
        let callbacks = CallbackManager::new();
        let config = RunnableConfig::new().with_callbacks(callbacks);
        let manager = config.get_callback_manager();

        assert_eq!(manager.len(), 0); // Empty manager
    }

    #[test]
    fn test_merge_metadata() {
        let config1 = RunnableConfig::new()
            .with_metadata("key1", "value1")
            .unwrap()
            .with_metadata("key2", "value2")
            .unwrap();

        let config2 = RunnableConfig::new()
            .with_metadata("key2", "overwritten")
            .unwrap()
            .with_metadata("key3", "value3")
            .unwrap();

        let merged = config1.merge(config2);

        assert_eq!(
            merged.metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(
            merged.metadata.get("key2"),
            Some(&serde_json::json!("overwritten"))
        );
        assert_eq!(
            merged.metadata.get("key3"),
            Some(&serde_json::json!("value3"))
        );
    }

    #[test]
    fn test_merge_configurable() {
        let config1 = RunnableConfig::new()
            .with_configurable("model", "gpt-3.5")
            .unwrap();

        let config2 = RunnableConfig::new()
            .with_configurable("model", "gpt-4")
            .unwrap()
            .with_configurable("temperature", 0.9)
            .unwrap();

        let merged = config1.merge(config2);

        assert_eq!(
            merged.configurable.get("model"),
            Some(&serde_json::json!("gpt-4"))
        );
        assert_eq!(
            merged.configurable.get("temperature"),
            Some(&serde_json::json!(0.9))
        );
    }

    #[test]
    fn test_merge_recursion_limit_default() {
        let config1 = RunnableConfig::new(); // Default recursion_limit = 0
        let config2 = RunnableConfig::new(); // Default recursion_limit = 0

        let merged = config1.merge(config2);

        // When both are default (0), should remain default
        assert_eq!(merged.recursion_limit, 0);
    }

    #[test]
    fn test_merge_recursion_limit_custom() {
        let config1 = RunnableConfig::new().with_recursion_limit(10);
        let config2 = RunnableConfig::new().with_recursion_limit(50);

        let merged = config1.merge(config2);

        // config2's custom value takes precedence
        assert_eq!(merged.recursion_limit, 50);
    }

    #[test]
    fn test_merge_max_concurrency() {
        let config1 = RunnableConfig::new().with_max_concurrency(5);
        let config2 = RunnableConfig::new().with_max_concurrency(10);

        let merged = config1.merge(config2);

        assert_eq!(merged.max_concurrency, Some(10));
    }

    #[test]
    fn test_merge_run_id() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let config1 = RunnableConfig::new().with_run_id(id1);
        let config2 = RunnableConfig::new().with_run_id(id2);

        let merged = config1.merge(config2);

        assert_eq!(merged.run_id, Some(id2)); // config2 takes precedence
    }

    #[test]
    fn test_merge_callbacks() {
        let callbacks1 = CallbackManager::new();
        let callbacks2 = CallbackManager::new();

        let config1 = RunnableConfig::new().with_callbacks(callbacks1);
        let config2 = RunnableConfig::new().with_callbacks(callbacks2);

        let merged = config1.merge(config2);

        assert!(merged.callbacks.is_some()); // config2's callbacks
    }

    #[test]
    fn test_merge_partial() {
        let config1 = RunnableConfig::new()
            .with_tag("tag1")
            .with_run_name("run1")
            .with_max_concurrency(5);

        let config2 = RunnableConfig::new().with_tag("tag2");

        let merged = config1.merge(config2);

        assert_eq!(merged.tags, vec!["tag1", "tag2"]);
        assert_eq!(merged.run_name, Some("run1".to_string())); // config1 preserved
        assert_eq!(merged.max_concurrency, Some(5)); // config1 preserved
    }

    #[test]
    fn test_serialization_skip_empty_fields() {
        let config = RunnableConfig::new();

        let json = serde_json::to_value(&config).unwrap();

        // Empty fields should be skipped
        assert!(json.get("tags").is_none() || json["tags"].as_array().unwrap().is_empty());
        assert!(json.get("metadata").is_none() || json["metadata"].as_object().unwrap().is_empty());
        assert!(json.get("run_name").is_none());
        assert!(json.get("max_concurrency").is_none());
        assert!(json.get("run_id").is_none());
        assert!(json.get("callbacks").is_none());
    }

    #[test]
    fn test_serialization_with_all_fields() {
        let id = Uuid::new_v4();
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_metadata("key", "value")
            .unwrap()
            .with_run_name("my_run")
            .with_max_concurrency(10)
            .with_recursion_limit(50)
            .with_configurable("model", "gpt-4")
            .unwrap()
            .with_run_id(id);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RunnableConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.tags, deserialized.tags);
        assert_eq!(config.metadata, deserialized.metadata);
        assert_eq!(config.run_name, deserialized.run_name);
        assert_eq!(config.max_concurrency, deserialized.max_concurrency);
        assert_eq!(config.recursion_limit, deserialized.recursion_limit);
        assert_eq!(config.configurable, deserialized.configurable);
        assert_eq!(config.run_id, deserialized.run_id);
        // callbacks not serialized
        assert!(deserialized.callbacks.is_none());
    }

    #[test]
    fn test_with_tags_multiple() {
        let tags = vec!["tag1", "tag2", "tag3"];
        let config = RunnableConfig::new().with_tags(tags.clone());

        assert_eq!(config.tags, tags);
    }

    #[test]
    fn test_with_tags_empty() {
        let config = RunnableConfig::new().with_tags(Vec::<String>::new());

        assert!(config.tags.is_empty());
    }

    #[test]
    fn test_debug_trait() {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_run_name("my_run");

        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("RunnableConfig"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("my_run"));
    }

    #[test]
    fn test_metadata_different_types() {
        let config = RunnableConfig::new()
            .with_metadata("string", "text")
            .unwrap()
            .with_metadata("number", 42)
            .unwrap()
            .with_metadata("float", 3.5)
            .unwrap()
            .with_metadata("bool", true)
            .unwrap();

        assert_eq!(
            config.metadata.get("string"),
            Some(&serde_json::json!("text"))
        );
        assert_eq!(config.metadata.get("number"), Some(&serde_json::json!(42)));
        assert_eq!(config.metadata.get("float"), Some(&serde_json::json!(3.5)));
        assert_eq!(config.metadata.get("bool"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_configurable_different_types() {
        let config = RunnableConfig::new()
            .with_configurable("string_val", "text")
            .unwrap()
            .with_configurable("int_val", 100)
            .unwrap()
            .with_configurable("bool_val", false)
            .unwrap();

        assert_eq!(
            config.configurable.get("string_val"),
            Some(&serde_json::json!("text"))
        );
        assert_eq!(
            config.configurable.get("int_val"),
            Some(&serde_json::json!(100))
        );
        assert_eq!(
            config.configurable.get("bool_val"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn test_ensure_run_id_generates_valid_uuid() {
        let mut config = RunnableConfig::new();
        let id = config.ensure_run_id();

        // Should be a valid UUID
        assert_ne!(id, Uuid::nil());

        // Second call should return same ID
        let id2 = config.ensure_run_id();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_with_recursion_limit_zero() {
        let config = RunnableConfig::new().with_recursion_limit(0);
        assert_eq!(config.recursion_limit, 0);
    }

    #[test]
    fn test_with_recursion_limit_large() {
        let config = RunnableConfig::new().with_recursion_limit(1000);
        assert_eq!(config.recursion_limit, 1000);
    }

    #[test]
    fn test_merge_empty_configs() {
        let config1 = RunnableConfig::new();
        let config2 = RunnableConfig::new();

        let merged = config1.merge(config2);

        assert!(merged.tags.is_empty());
        assert!(merged.metadata.is_empty());
        assert!(merged.run_name.is_none());
        assert_eq!(merged.recursion_limit, 0); // Default from derive(Default)
    }
}
