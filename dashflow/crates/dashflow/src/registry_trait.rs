// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Base Registry Trait Hierarchy
//!
//! This module provides a unified trait hierarchy for registry types in DashFlow.
//! Multiple registry implementations (23+) share common patterns:
//! - `register(key, value)` - add items
//! - `get(key)` - retrieve items
//! - `remove(key)` - delete items
//! - `list()` / `iter()` - enumerate items
//!
//! # Trait Hierarchy
//!
//! - [`Registry`] - Core read operations (get, contains, len, is_empty)
//! - [`RegistryMut`] - Mutable operations (register, remove)
//! - [`RegistryIter`] - Iterator support (keys, values, iter)
//! - [`ConcurrentRegistry`] - Thread-safe registry operations
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::registry_trait::{Registry, RegistryMut, RegistryIter};
//! use std::collections::HashMap;
//!
//! struct SimpleRegistry<V> {
//!     entries: HashMap<String, V>,
//! }
//!
//! impl<V> Registry<V> for SimpleRegistry<V> {
//!     fn get(&self, key: &str) -> Option<&V> {
//!         self.entries.get(key)
//!     }
//!
//!     fn contains(&self, key: &str) -> bool {
//!         self.entries.contains_key(key)
//!     }
//!
//!     fn len(&self) -> usize {
//!         self.entries.len()
//!     }
//! }
//!
//! impl<V> RegistryMut<V> for SimpleRegistry<V> {
//!     type Error = std::convert::Infallible;
//!
//!     fn register(&mut self, key: impl Into<String>, value: V) -> Result<(), Self::Error> {
//!         self.entries.insert(key.into(), value);
//!         Ok(())
//!     }
//!
//!     fn remove(&mut self, key: &str) -> Option<V> {
//!         self.entries.remove(key)
//!     }
//! }
//! ```
//!
//! # Migration Guide
//!
//! Existing registries can implement these traits while keeping their
//! domain-specific methods. The traits provide a common interface for:
//! - Generic code that works across registry types
//! - Testing utilities
//! - Introspection and monitoring
//!
//! ## Registries to Migrate (23 found)
//!
//! Core: NodeRegistry, GraphRegistry, ExecutionRegistry, StateRegistry
//! Package: ColonyPackageRegistry, LocalRegistry, RegistryClient
//! Network: ColonyResourceRegistry, PeerRegistry
//! Platform: PlatformRegistry, McpToolRegistry, TemplateRegistry
//! Self-Improvement: CircuitBreakerRegistry, AnalyzerRegistry, PlannerRegistry
//! Other: MetricsRegistry, PromptRegistry, ConditionRegistry

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};

// =============================================================================
// Core Registry Trait - Read Operations
// =============================================================================

/// Core registry trait for read-only operations.
///
/// This trait provides the minimal interface for querying a registry.
/// It uses `&str` keys (the common case) for simplicity. For registries
/// with other key types, see [`GenericRegistry`].
pub trait Registry<V> {
    /// Get a reference to a value by key.
    fn get(&self, key: &str) -> Option<&V>;

    /// Check if a key exists in the registry.
    fn contains(&self, key: &str) -> bool;

    /// Get the number of entries in the registry.
    fn len(&self) -> usize;

    /// Check if the registry is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Generic registry trait for non-string keys.
///
/// Most registries use String keys, but some (like StateRegistry) use
/// composite keys. This trait supports any hashable key type.
pub trait GenericRegistry<K, V>
where
    K: Eq + Hash,
{
    /// Get a reference to a value by key.
    fn get(&self, key: &K) -> Option<&V>;

    /// Check if a key exists in the registry.
    fn contains(&self, key: &K) -> bool;

    /// Get the number of entries in the registry.
    fn len(&self) -> usize;

    /// Check if the registry is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// =============================================================================
// Mutable Registry Trait
// =============================================================================

/// Registry trait for mutable operations.
///
/// Separated from [`Registry`] to allow immutable access without requiring
/// mutable borrows.
pub trait RegistryMut<V>: Registry<V> {
    /// Error type returned when registration fails.
    type Error;

    /// Register a new value with the given key.
    ///
    /// If a value already exists for this key, it is replaced.
    fn register(&mut self, key: impl Into<String>, value: V) -> Result<(), Self::Error>;

    /// Remove a value by key, returning it if it existed.
    fn remove(&mut self, key: &str) -> Option<V>;

    /// Remove all entries from the registry.
    fn clear(&mut self);
}

/// Generic mutable registry trait for non-string keys.
pub trait GenericRegistryMut<K, V>: GenericRegistry<K, V>
where
    K: Eq + Hash,
{
    /// Error type returned when registration fails.
    type Error;

    /// Register a new value with the given key.
    fn register(&mut self, key: K, value: V) -> Result<(), Self::Error>;

    /// Remove a value by key, returning it if it existed.
    fn remove(&mut self, key: &K) -> Option<V>;

    /// Remove all entries from the registry.
    fn clear(&mut self);
}

// =============================================================================
// Iterator Traits
// =============================================================================

/// Registry trait for iteration support.
///
/// Not all registries need iteration, so this is a separate trait.
pub trait RegistryIter<V>: Registry<V> {
    /// Iterator type for keys.
    type KeyIter<'a>: Iterator<Item = &'a str>
    where
        Self: 'a;

    /// Iterator type for values.
    type ValueIter<'a>: Iterator<Item = &'a V>
    where
        Self: 'a,
        V: 'a;

    /// Get an iterator over all keys.
    fn keys(&self) -> Self::KeyIter<'_>;

    /// Get an iterator over all values.
    fn values(&self) -> Self::ValueIter<'_>;
}

// =============================================================================
// Concurrent Registry Wrapper
// =============================================================================

/// Thread-safe registry wrapper using `Arc<RwLock<>>`.
///
/// Many registries in DashFlow need thread-safe access. This wrapper
/// provides a standard implementation that can wrap any registry.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::registry_trait::ConcurrentRegistry;
///
/// let registry = ConcurrentRegistry::new(SimpleRegistry::default());
///
/// // Access from multiple threads
/// let registry_clone = registry.clone();
/// std::thread::spawn(move || {
///     registry_clone.with_read(|r| {
///         println!("Entries: {}", r.len());
///     });
/// });
///
/// // Write access
/// registry.with_write(|r| {
///     r.register("key", value)?;
///     Ok(())
/// });
/// ```
#[derive(Debug)]
pub struct ConcurrentRegistry<R> {
    inner: Arc<RwLock<R>>,
}

impl<R> Clone for ConcurrentRegistry<R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<R: Default> Default for ConcurrentRegistry<R> {
    fn default() -> Self {
        Self::new(R::default())
    }
}

impl<R> ConcurrentRegistry<R> {
    /// Create a new concurrent registry wrapping the given registry.
    pub fn new(registry: R) -> Self {
        Self {
            inner: Arc::new(RwLock::new(registry)),
        }
    }

    /// Execute a read operation on the registry.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with_read<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&R) -> T,
    {
        let guard = self.inner.read().expect("Registry lock poisoned");
        f(&guard)
    }

    /// Execute a write operation on the registry.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with_write<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut R) -> T,
    {
        let mut guard = self.inner.write().expect("Registry lock poisoned");
        f(&mut guard)
    }
}

// Specialized methods for ConcurrentRegistry with string-keyed registries
impl<R> ConcurrentRegistry<R> {
    /// Get a value by key (thread-safe).
    ///
    /// Note: This returns a clone of the value, not a reference.
    pub fn get_cloned<V>(&self, key: &str) -> Option<V>
    where
        R: Registry<V>,
        V: Clone,
    {
        self.with_read(|r| r.get(key).cloned())
    }

    /// Check if a key exists (thread-safe).
    pub fn contains<V>(&self, key: &str) -> bool
    where
        R: Registry<V>,
    {
        self.with_read(|r| r.contains(key))
    }

    /// Get the number of entries (thread-safe).
    pub fn len<V>(&self) -> usize
    where
        R: Registry<V>,
    {
        self.with_read(|r| r.len())
    }

    /// Check if empty (thread-safe).
    pub fn is_empty<V>(&self) -> bool
    where
        R: Registry<V>,
    {
        self.len::<V>() == 0
    }
}

// =============================================================================
// Simple HashMap-based Registry Implementation
// =============================================================================

/// Simple registry implementation backed by a HashMap.
///
/// Useful for testing and as a reference implementation.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::registry_trait::SimpleRegistry;
///
/// let mut registry: SimpleRegistry<String> = SimpleRegistry::default();
/// registry.register("greeting", "Hello, World!".to_string());
///
/// assert_eq!(registry.get("greeting"), Some(&"Hello, World!".to_string()));
/// assert_eq!(registry.len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SimpleRegistry<V> {
    entries: HashMap<String, V>,
}

impl<V> SimpleRegistry<V> {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Create a registry with the given capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
        }
    }
}

impl<V> Registry<V> for SimpleRegistry<V> {
    fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

impl<V> RegistryMut<V> for SimpleRegistry<V> {
    type Error = std::convert::Infallible;

    fn register(&mut self, key: impl Into<String>, value: V) -> Result<(), Self::Error> {
        self.entries.insert(key.into(), value);
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Option<V> {
        self.entries.remove(key)
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<V> RegistryIter<V> for SimpleRegistry<V> {
    type KeyIter<'a>
        = std::iter::Map<std::collections::hash_map::Keys<'a, String, V>, fn(&String) -> &str>
    where
        Self: 'a;

    type ValueIter<'a>
        = std::collections::hash_map::Values<'a, String, V>
    where
        Self: 'a,
        V: 'a;

    fn keys(&self) -> Self::KeyIter<'_> {
        self.entries.keys().map(String::as_str)
    }

    fn values(&self) -> Self::ValueIter<'_> {
        self.entries.values()
    }
}

// =============================================================================
// TypedFactoryRegistry - Generic Factory Registry (Phase 2.1)
// =============================================================================

/// Generic registry for storing factories by string key.
///
/// This struct consolidates the common pattern used by:
/// - `NodeRegistry<S>` - stores `Arc<dyn NodeFactory<S>>`
/// - `FactoryRegistry<T>` - stores `Arc<dyn DynFactory<T>>`
/// - `ConditionRegistry<S>` - stores `Arc<dyn ConditionFactory<S>>`
///
/// All use the same underlying pattern: `HashMap<String, Arc<dyn Trait>>`.
///
/// # Type Parameters
///
/// - `F`: The factory trait object type (e.g., `dyn NodeFactory<S>`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::registry_trait::TypedFactoryRegistry;
///
/// // Create a registry for node factories
/// let mut registry: TypedFactoryRegistry<dyn NodeFactory<MyState>> = TypedFactoryRegistry::new();
/// registry.register("chat", Arc::new(ChatNodeFactory));
/// ```
#[derive(Debug)]
pub struct TypedFactoryRegistry<F: ?Sized> {
    factories: HashMap<String, Arc<F>>,
}

impl<F: ?Sized> Default for TypedFactoryRegistry<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: ?Sized> Clone for TypedFactoryRegistry<F> {
    fn clone(&self) -> Self {
        Self {
            factories: self.factories.clone(),
        }
    }
}

impl<F: ?Sized> TypedFactoryRegistry<F> {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a factory with the given key.
    ///
    /// Returns the previous factory if one was registered with the same key.
    pub fn register(&mut self, key: impl Into<String>, factory: Arc<F>) -> Option<Arc<F>> {
        self.factories.insert(key.into(), factory)
    }

    /// Get a factory by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Arc<F>> {
        self.factories.get(key)
    }

    /// Remove a factory by key.
    pub fn remove(&mut self, key: &str) -> Option<Arc<F>> {
        self.factories.remove(key)
    }

    /// Check if a key exists.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.factories.contains_key(key)
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

    /// Get an iterator over all keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }

    /// Get an iterator over all factories.
    pub fn values(&self) -> impl Iterator<Item = &Arc<F>> {
        self.factories.values()
    }

    /// Get an iterator over all (key, factory) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<F>)> {
        self.factories.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Clear all registered factories.
    pub fn clear(&mut self) {
        self.factories.clear();
    }
}

impl<F: ?Sized> Registry<Arc<F>> for TypedFactoryRegistry<F> {
    fn get(&self, key: &str) -> Option<&Arc<F>> {
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
// Registry Stats - Introspection Helper
// =============================================================================

/// Statistics about a registry for introspection.
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Number of entries
    pub count: usize,
    /// Registry name/identifier
    pub name: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl RegistryStats {
    /// Create stats from a registry.
    pub fn from_registry<V>(name: &str, registry: &impl Registry<V>) -> Self {
        Self {
            count: registry.len(),
            name: name.to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the stats.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_registry_basic() {
        let mut registry: SimpleRegistry<i32> = SimpleRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register("one", 1).unwrap();
        registry.register("two", 2).unwrap();

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.get("one"), Some(&1));
        assert_eq!(registry.get("two"), Some(&2));
        assert_eq!(registry.get("three"), None);
        assert!(registry.contains("one"));
        assert!(!registry.contains("three"));
    }

    #[test]
    fn test_simple_registry_remove() {
        let mut registry: SimpleRegistry<String> = SimpleRegistry::new();

        registry.register("key", "value".to_string()).unwrap();
        assert_eq!(registry.len(), 1);

        let removed = registry.remove("key");
        assert_eq!(removed, Some("value".to_string()));
        assert_eq!(registry.len(), 0);
        assert!(!registry.contains("key"));
    }

    #[test]
    fn test_simple_registry_clear() {
        let mut registry: SimpleRegistry<i32> = SimpleRegistry::new();

        registry.register("a", 1).unwrap();
        registry.register("b", 2).unwrap();
        assert_eq!(registry.len(), 2);

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_simple_registry_iter() {
        let mut registry: SimpleRegistry<i32> = SimpleRegistry::new();

        registry.register("a", 1).unwrap();
        registry.register("b", 2).unwrap();
        registry.register("c", 3).unwrap();

        let keys: Vec<_> = registry.keys().collect();
        assert_eq!(keys.len(), 3);

        let values: Vec<_> = registry.values().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&&1));
        assert!(values.contains(&&2));
        assert!(values.contains(&&3));
    }

    #[test]
    fn test_concurrent_registry_basic() {
        let registry: ConcurrentRegistry<SimpleRegistry<i32>> = ConcurrentRegistry::default();

        registry.with_write(|r| {
            r.register("one", 1).unwrap();
            r.register("two", 2).unwrap();
        });

        assert_eq!(registry.len::<i32>(), 2);
        assert!(registry.contains::<i32>("one"));
        assert_eq!(registry.get_cloned::<i32>("one"), Some(1));
    }

    #[test]
    fn test_concurrent_registry_thread_safe() {
        use std::thread;

        let registry: ConcurrentRegistry<SimpleRegistry<i32>> = ConcurrentRegistry::default();

        // Write from one thread
        let r1 = registry.clone();
        let h1 = thread::spawn(move || {
            r1.with_write(|r| {
                for i in 0..100 {
                    let _ = r.register(format!("a_{i}"), i);
                }
            });
        });

        // Write from another thread
        let r2 = registry.clone();
        let h2 = thread::spawn(move || {
            r2.with_write(|r| {
                for i in 0..100 {
                    let _ = r.register(format!("b_{i}"), i + 100);
                }
            });
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Both threads' writes should be visible
        assert_eq!(registry.len::<i32>(), 200);
    }

    #[test]
    fn test_registry_stats() {
        let mut registry: SimpleRegistry<i32> = SimpleRegistry::new();
        registry.register("a", 1).unwrap();
        registry.register("b", 2).unwrap();

        let stats =
            RegistryStats::from_registry("test", &registry).with_metadata("type", "SimpleRegistry");

        assert_eq!(stats.count, 2);
        assert_eq!(stats.name, "test");
        assert_eq!(
            stats.metadata.get("type"),
            Some(&"SimpleRegistry".to_string())
        );
    }

    // TypedFactoryRegistry tests
    trait TestFactory: Send + Sync {
        fn name(&self) -> &str;
    }

    struct MockFactory {
        name: String,
    }

    impl TestFactory for MockFactory {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_typed_factory_registry_basic() {
        let mut registry: TypedFactoryRegistry<dyn TestFactory> = TypedFactoryRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        let factory1: Arc<dyn TestFactory> = Arc::new(MockFactory {
            name: "factory1".to_string(),
        });
        let factory2: Arc<dyn TestFactory> = Arc::new(MockFactory {
            name: "factory2".to_string(),
        });

        registry.register("one", factory1);
        registry.register("two", factory2);

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 2);
        assert!(registry.contains("one"));
        assert!(registry.contains("two"));
        assert!(!registry.contains("three"));

        let f = registry.get("one").unwrap();
        assert_eq!(f.name(), "factory1");
    }

    #[test]
    fn test_typed_factory_registry_remove() {
        let mut registry: TypedFactoryRegistry<dyn TestFactory> = TypedFactoryRegistry::new();

        let factory: Arc<dyn TestFactory> = Arc::new(MockFactory {
            name: "test".to_string(),
        });
        registry.register("key", factory);

        assert_eq!(registry.len(), 1);
        let removed = registry.remove("key");
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_typed_factory_registry_iter() {
        let mut registry: TypedFactoryRegistry<dyn TestFactory> = TypedFactoryRegistry::new();

        for i in 0..3 {
            let factory: Arc<dyn TestFactory> = Arc::new(MockFactory {
                name: format!("factory{}", i),
            });
            registry.register(format!("key{}", i), factory);
        }

        let keys: Vec<_> = registry.keys().collect();
        assert_eq!(keys.len(), 3);

        let values: Vec<_> = registry.values().collect();
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_typed_factory_registry_clone() {
        let mut registry: TypedFactoryRegistry<dyn TestFactory> = TypedFactoryRegistry::new();

        let factory: Arc<dyn TestFactory> = Arc::new(MockFactory {
            name: "test".to_string(),
        });
        registry.register("key", factory);

        let cloned = registry.clone();
        assert_eq!(cloned.len(), 1);
        assert!(cloned.contains("key"));
    }
}
