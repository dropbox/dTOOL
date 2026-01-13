//! Base Factory Trait Hierarchy
//!
//! This module provides a unified trait hierarchy for factory types in DashFlow.
//! Multiple factory implementations share common patterns:
//! - `create(config)` - create an instance from configuration
//! - `type_name()` - identify the factory type
//! - `supports(config)` - check if factory can handle configuration
//!
//! # Trait Hierarchy
//!
//! - [`Factory`] - Core factory trait (create from config)
//! - [`TypedFactory`] - Factory with type introspection
//! - [`AsyncFactory`] - Async factory operations
//! - [`FactoryRegistry`] - Registry of factory instances
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::factory_trait::{Factory, TypedFactory, FactoryError};
//! use serde_json::Value;
//!
//! struct MyWidget {
//!     name: String,
//!     count: u32,
//! }
//!
//! struct MyWidgetFactory;
//!
//! impl Factory for MyWidgetFactory {
//!     type Output = MyWidget;
//!     type Error = FactoryError;
//!
//!     fn create(&self, config: &Value) -> Result<Self::Output, Self::Error> {
//!         let name = config["name"].as_str()
//!             .ok_or_else(|| FactoryError::MissingField("name".into()))?;
//!         let count = config["count"].as_u64()
//!             .ok_or_else(|| FactoryError::MissingField("count".into()))? as u32;
//!         Ok(MyWidget { name: name.to_string(), count })
//!     }
//! }
//!
//! impl TypedFactory for MyWidgetFactory {
//!     fn type_name(&self) -> &'static str {
//!         "MyWidget"
//!     }
//!
//!     fn supports(&self, config: &Value) -> bool {
//!         config.get("type").and_then(|v| v.as_str()) == Some("my_widget")
//!     }
//! }
//! ```
//!
//! # Migration Guide
//!
//! Existing factories can implement these traits while keeping their
//! domain-specific methods. The traits provide a common interface for:
//! - Generic code that works across factory types
//! - Factory registration and discovery
//! - Configuration validation
//!
//! ## Factories to Consider (found in codebase)
//!
//! - `NodeFactory<S>` - Creates graph nodes from config
//! - `ConditionFactory<S>` - Creates condition functions from config
//! - `ModelFactory` - Creates LLM model instances
//! - `DefaultModelFactory` - Default model factory implementation

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;

use crate::registry_trait::Registry;

// =============================================================================
// Type Aliases (CQ-40: Reduce repetitive error type)
// =============================================================================

/// Type alias for a boxed error that is Send + Sync.
///
/// Used across factory traits for type-erased error handling.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

// =============================================================================
// Error Types
// =============================================================================

/// Error type for factory operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum FactoryError {
    /// Required field is missing from configuration
    #[error("Missing required field: {0}")]
    MissingField(String),
    /// Field has invalid type or value
    #[error("Invalid field '{field}': expected {expected}, got {got}")]
    InvalidField {
        /// Name of the field with the invalid value.
        field: String,
        /// Expected type or format description.
        expected: String,
        /// Actual type or value received.
        got: String,
    },
    /// Configuration validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),
    /// Factory does not support this configuration
    #[error("Unsupported configuration: {0}")]
    Unsupported(String),
    /// Wrapped error from underlying creation logic
    #[error("Creation failed: {0}")]
    Creation(String),
}

// =============================================================================
// Core Factory Trait
// =============================================================================

/// Core factory trait for creating objects from configuration.
///
/// This is the minimal interface for a factory. Implementations
/// take a JSON configuration value and produce an instance of
/// the output type.
///
/// # Type Parameters
///
/// - `Output` - The type of object this factory creates
/// - `Error` - The error type returned on failure
pub trait Factory {
    /// The type of object this factory creates.
    type Output;

    /// The error type returned when creation fails.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Create an instance from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - JSON configuration for the object
    ///
    /// # Returns
    ///
    /// * `Ok(Self::Output)` - The created instance
    /// * `Err(Self::Error)` - Creation failed
    fn create(&self, config: &serde_json::Value) -> Result<Self::Output, Self::Error>;
}

/// Factory with type introspection capabilities.
///
/// Extends [`Factory`] with methods for type identification
/// and configuration validation. This enables factory discovery
/// and routing based on configuration content.
pub trait TypedFactory: Factory {
    /// Get the type name this factory produces.
    ///
    /// This should be a stable identifier used for:
    /// - Logging and debugging
    /// - Configuration routing (e.g., "type": "my_widget")
    /// - Factory registration keys
    fn type_name(&self) -> &'static str;

    /// Check if this factory can handle the given configuration.
    ///
    /// Used by factory registries to route configurations to
    /// the appropriate factory. Default implementation checks
    /// for a "type" field matching [`type_name()`](Self::type_name).
    fn supports(&self, config: &serde_json::Value) -> bool {
        config
            .get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == self.type_name())
            .unwrap_or(false)
    }

    /// Get optional schema for configuration validation.
    ///
    /// Returns a JSON Schema describing valid configurations.
    /// Default returns None (no schema available).
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }
}

// =============================================================================
// Async Factory Trait
// =============================================================================

/// Async factory trait for creating objects asynchronously.
///
/// Some factories need async operations during creation (e.g.,
/// fetching remote configuration, establishing connections).
pub trait AsyncFactory: Send + Sync {
    /// The type of object this factory creates.
    type Output: Send;

    /// The error type returned when creation fails.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Create an instance from configuration asynchronously.
    ///
    /// # Arguments
    ///
    /// * `config` - JSON configuration for the object
    ///
    /// # Returns
    ///
    /// A future that resolves to the created instance or an error.
    #[allow(clippy::type_complexity)] // Complex async return type is inherent to object-safe async traits
    fn create_async<'a>(
        &'a self,
        config: &'a serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'a>>;
}

// =============================================================================
// Factory Registry
// =============================================================================

/// Type-erased factory that can be stored in a registry.
///
/// This trait enables storing different factory types in a single
/// collection by erasing the concrete factory type.
pub trait DynFactory<T>: Send + Sync {
    /// Get the type name this factory produces.
    fn type_name(&self) -> &'static str;

    /// Check if this factory can handle the given configuration.
    fn supports(&self, config: &serde_json::Value) -> bool;

    /// Create an instance from configuration.
    fn create(&self, config: &serde_json::Value) -> Result<T, BoxError>;
}

// Blanket implementation for TypedFactory
impl<T, F> DynFactory<T> for F
where
    F: TypedFactory<Output = T> + Send + Sync,
    F::Error: 'static,
{
    fn type_name(&self) -> &'static str {
        TypedFactory::type_name(self)
    }

    fn supports(&self, config: &serde_json::Value) -> bool {
        TypedFactory::supports(self, config)
    }

    fn create(&self, config: &serde_json::Value) -> Result<T, BoxError> {
        Factory::create(self, config).map_err(|e| Box::new(e) as BoxError)
    }
}

/// Registry of factories for a given output type.
///
/// Stores multiple factories and routes configurations to the
/// appropriate factory based on the `supports()` method.
pub struct FactoryRegistry<T> {
    /// Factories indexed by type name
    factories: HashMap<String, Arc<dyn DynFactory<T>>>,
}

impl<T> Default for FactoryRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FactoryRegistry<T> {
    /// Create a new empty factory registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a factory.
    ///
    /// The factory is registered under its type name. If a factory
    /// with the same type name already exists, it is replaced.
    pub fn register<F>(&mut self, factory: F)
    where
        F: TypedFactory<Output = T> + Send + Sync + 'static,
        F::Error: 'static,
    {
        let type_name = factory.type_name().to_string();
        self.factories.insert(type_name, Arc::new(factory));
    }

    /// Register a dynamic factory.
    pub fn register_dyn(&mut self, factory: Arc<dyn DynFactory<T>>) {
        let type_name = factory.type_name().to_string();
        self.factories.insert(type_name, factory);
    }

    /// Get a factory by type name.
    #[must_use]
    pub fn get(&self, type_name: &str) -> Option<&Arc<dyn DynFactory<T>>> {
        self.factories.get(type_name)
    }

    /// Find a factory that supports the given configuration.
    ///
    /// Iterates through registered factories and returns the first
    /// one where `supports(config)` returns true.
    #[must_use]
    pub fn find_supporting(&self, config: &serde_json::Value) -> Option<&Arc<dyn DynFactory<T>>> {
        self.factories.values().find(|f| f.supports(config))
    }

    /// Create an instance using the appropriate factory.
    ///
    /// Finds a factory that supports the configuration and uses it
    /// to create the instance.
    ///
    /// # Errors
    ///
    /// Returns an error if no factory supports the configuration
    /// or if creation fails.
    pub fn create(&self, config: &serde_json::Value) -> Result<T, BoxError> {
        let factory = self.find_supporting(config).ok_or_else(|| {
            Box::new(FactoryError::Unsupported(format!(
                "no factory supports config: {}",
                serde_json::to_string(config).unwrap_or_default()
            ))) as BoxError
        })?;

        factory.create(config)
    }

    /// Get all registered type names.
    pub fn type_names(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }

    /// Get the number of registered factories.
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

// =============================================================================
// Registry Trait Implementation (Phase 2.2 of REFACTORING_PLAN.md)
// =============================================================================

/// Implements the standard Registry trait for FactoryRegistry.
impl<T> Registry<Arc<dyn DynFactory<T>>> for FactoryRegistry<T> {
    fn get(&self, key: &str) -> Option<&Arc<dyn DynFactory<T>>> {
        self.factories.get(key)
    }

    fn contains(&self, key: &str) -> bool {
        self.factories.contains_key(key)
    }

    fn len(&self) -> usize {
        self.factories.len()
    }
}

// =============================================================================
// Utility: Simple Factory
// =============================================================================

/// A simple factory that uses a closure for creation.
///
/// Useful for creating factories from functions without defining
/// a new struct.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::factory_trait::{SimpleFactory, Factory, FactoryError};
///
/// let factory = SimpleFactory::new("greeter", |config| {
///     let name = config["name"].as_str()
///         .ok_or_else(|| FactoryError::MissingField("name".into()))?;
///     Ok(format!("Hello, {}!", name))
/// });
///
/// let greeting = factory.create(&serde_json::json!({"name": "World"}))?;
/// assert_eq!(greeting, "Hello, World!");
/// ```
pub struct SimpleFactory<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, FactoryError> + Send + Sync,
{
    type_name: &'static str,
    creator: F,
}

impl<T, F> SimpleFactory<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, FactoryError> + Send + Sync,
{
    /// Create a new simple factory.
    #[must_use]
    pub fn new(type_name: &'static str, creator: F) -> Self {
        Self { type_name, creator }
    }
}

impl<T, F> Factory for SimpleFactory<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, FactoryError> + Send + Sync,
{
    type Output = T;
    type Error = FactoryError;

    fn create(&self, config: &serde_json::Value) -> Result<Self::Output, Self::Error> {
        (self.creator)(config)
    }
}

impl<T, F> TypedFactory for SimpleFactory<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, FactoryError> + Send + Sync,
{
    fn type_name(&self) -> &'static str {
        self.type_name
    }
}

// =============================================================================
// Utility: FactoryInfo for introspection
// =============================================================================

/// Information about a registered factory for introspection.
#[derive(Debug, Clone)]
pub struct FactoryInfo {
    /// The type name of the factory output
    pub type_name: String,
    /// Whether the factory has a config schema
    pub has_schema: bool,
}

impl FactoryInfo {
    /// Create factory info from a dynamic factory.
    #[must_use]
    pub fn from_dyn<T>(factory: &dyn DynFactory<T>) -> Self {
        Self {
            type_name: factory.type_name().to_string(),
            has_schema: false, // DynFactory doesn't expose schema
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Test struct
    #[derive(Debug, PartialEq)]
    struct Widget {
        name: String,
        value: i32,
    }

    // Test factory
    struct WidgetFactory;

    impl Factory for WidgetFactory {
        type Output = Widget;
        type Error = FactoryError;

        fn create(&self, config: &serde_json::Value) -> Result<Self::Output, Self::Error> {
            let name = config["name"]
                .as_str()
                .ok_or_else(|| FactoryError::MissingField("name".into()))?
                .to_string();
            let value = config["value"]
                .as_i64()
                .ok_or_else(|| FactoryError::MissingField("value".into()))?
                as i32;
            Ok(Widget { name, value })
        }
    }

    impl TypedFactory for WidgetFactory {
        fn type_name(&self) -> &'static str {
            "widget"
        }
    }

    #[test]
    fn test_factory_create() {
        let factory = WidgetFactory;
        let config = json!({
            "type": "widget",
            "name": "test",
            "value": 42
        });

        let widget = Factory::create(&factory, &config).unwrap();
        assert_eq!(widget.name, "test");
        assert_eq!(widget.value, 42);
    }

    #[test]
    fn test_factory_missing_field() {
        let factory = WidgetFactory;
        let config = json!({
            "type": "widget",
            "name": "test"
            // missing "value"
        });

        let result = Factory::create(&factory, &config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FactoryError::MissingField(_)));
    }

    #[test]
    fn test_typed_factory_supports() {
        let factory = WidgetFactory;

        assert!(TypedFactory::supports(&factory, &json!({"type": "widget"})));
        assert!(!TypedFactory::supports(&factory, &json!({"type": "other"})));
        assert!(!TypedFactory::supports(
            &factory,
            &json!({"name": "no type"})
        ));
    }

    #[test]
    fn test_factory_registry() {
        let mut registry: FactoryRegistry<Widget> = FactoryRegistry::new();
        registry.register(WidgetFactory);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("widget").is_some());
        assert!(registry.get("other").is_none());
    }

    #[test]
    fn test_factory_registry_create() {
        let mut registry: FactoryRegistry<Widget> = FactoryRegistry::new();
        registry.register(WidgetFactory);

        let config = json!({
            "type": "widget",
            "name": "from_registry",
            "value": 100
        });

        let widget = registry.create(&config).unwrap();
        assert_eq!(widget.name, "from_registry");
        assert_eq!(widget.value, 100);
    }

    #[test]
    fn test_factory_registry_no_match() {
        let registry: FactoryRegistry<Widget> = FactoryRegistry::new();

        let config = json!({"type": "unknown"});
        let result = registry.create(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_factory() {
        let factory = SimpleFactory::new("greeting", |config| {
            let name = config["name"]
                .as_str()
                .ok_or_else(|| FactoryError::MissingField("name".into()))?;
            Ok(format!("Hello, {}!", name))
        });

        let config = json!({"type": "greeting", "name": "World"});
        let greeting = Factory::create(&factory, &config).unwrap();
        assert_eq!(greeting, "Hello, World!");
    }

    #[test]
    fn test_simple_factory_typed() {
        let factory = SimpleFactory::new("greeting", |config| {
            let name = config["name"]
                .as_str()
                .ok_or_else(|| FactoryError::MissingField("name".into()))?;
            Ok(format!("Hello, {}!", name))
        });

        assert_eq!(TypedFactory::type_name(&factory), "greeting");
        assert!(TypedFactory::supports(
            &factory,
            &json!({"type": "greeting"})
        ));
    }
}
