//! Key-value store abstractions for caching and persistence.
//!
//! This module provides generic key-value store interfaces and implementations.
//! The primary use cases are:
//!
//! - **LLM response caching**: Store expensive API responses
//! - **Embedding caching**: Cache computed embeddings
//! - **Session state**: Store conversation state and metadata
//! - **Configuration storage**: Persist runtime configuration
//!
//! # Core Traits
//!
//! - [`BaseStore`] - Generic key-value store trait
//!
//! # Implementations
//!
//! - [`InMemoryStore`] - In-memory HashMap-backed store for any value type
//! - [`InMemoryByteStore`] - Specialized in-memory store for byte data
//!
//! # Design Principles
//!
//! All store operations use **batch APIs** (`mget`, `mset`, `mdelete`) to encourage
//! efficient usage patterns and minimize round-trips to backing stores (databases, caches).
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust
//! use dashflow::core::stores::{BaseStore, InMemoryStore};
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut store = InMemoryStore::new();
//!
//!     // Store multiple key-value pairs
//!     store.mset(vec![
//!         ("user:123".to_string(), "Alice".to_string()),
//!         ("user:456".to_string(), "Bob".to_string()),
//!     ]).await?;
//!
//!     // Retrieve values (returns Vec<Option<V>>)
//!     let values = store.mget(vec!["user:123".to_string(), "user:999".to_string()]).await?;
//!     assert_eq!(values, vec![Some("Alice".to_string()), None]);
//!
//!     // Iterate over keys with prefix
//!     let mut keys = store.yield_keys(Some("user:")).await;
//!     while let Some(key) = keys.next().await {
//!         println!("Found key: {}", key);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Embedding Cache
//!
//! ```rust
//! use dashflow::core::stores::{BaseStore, InMemoryStore};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct EmbeddingCache {
//!     vector: Vec<f32>,
//!     model: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut store: InMemoryStore<EmbeddingCache> = InMemoryStore::new();
//!
//!     let cache = EmbeddingCache {
//!         vector: vec![0.1, 0.2, 0.3],
//!         model: "text-embedding-3-small".to_string(),
//!     };
//!
//!     store.mset(vec![("doc:123".to_string(), cache)]).await?;
//!
//!     let cached = store.mget(vec!["doc:123".to_string()]).await?;
//!     println!("Cache hit: {}", cached[0].is_some());
//!
//!     Ok(())
//! }
//! ```

use crate::core::error::Result;
use async_trait::async_trait;
use futures::stream::{self, Stream};
use std::collections::HashMap;
use std::pin::Pin;

/// Generic key-value store trait.
///
/// This trait provides a uniform interface for different storage backends.
/// All operations are batch-oriented (`mget`, `mset`, `mdelete`) to encourage
/// efficient access patterns.
///
/// # Type Parameters
///
/// - `K`: Key type (must be hashable for most implementations)
/// - `V`: Value type (can be any cloneable type)
///
/// # Async Design
///
/// All methods are async to support backing stores that perform I/O:
/// - Database stores (`PostgreSQL`, Redis)
/// - Remote caches (Memcached, `DynamoDB`)
/// - File-based stores
///
/// In-memory implementations can return immediately without actual async work.
///
/// # Examples
///
/// Implement a custom store:
///
/// ```rust
/// use dashflow::core::stores::BaseStore;
/// use dashflow::core::error::Result;
/// use async_trait::async_trait;
/// use futures::stream::{self, Stream};
/// use std::pin::Pin;
/// use std::collections::HashMap;
///
/// struct MyCustomStore {
///     data: HashMap<String, i32>,
/// }
///
/// #[async_trait]
/// impl BaseStore<String, i32> for MyCustomStore {
///     async fn mget(&self, keys: Vec<String>) -> Result<Vec<Option<i32>>> {
///         Ok(keys.iter().map(|k| self.data.get(k).copied()).collect())
///     }
///
///     async fn mset(&mut self, key_value_pairs: Vec<(String, i32)>) -> Result<()> {
///         for (key, value) in key_value_pairs {
///             self.data.insert(key, value);
///         }
///         Ok(())
///     }
///
///     async fn mdelete(&mut self, keys: Vec<String>) -> Result<()> {
///         for key in keys {
///             self.data.remove(&key);
///         }
///         Ok(())
///     }
///
///     async fn yield_keys(&self, prefix: Option<&str>) -> Pin<Box<dyn Stream<Item = String> + Send + '_>> {
///         let keys: Vec<String> = match prefix {
///             Some(p) => self.data.keys().filter(|k| k.starts_with(p)).cloned().collect(),
///             None => self.data.keys().cloned().collect(),
///         };
///         Box::pin(stream::iter(keys))
///     }
/// }
/// ```
#[async_trait]
pub trait BaseStore<K, V>: Send + Sync
where
    K: Send + Sync,
    V: Send + Sync,
{
    /// Get values for multiple keys.
    ///
    /// Returns a vector with the same length as `keys`, where each position
    /// contains `Some(value)` if the key exists, or `None` if not found.
    ///
    /// # Arguments
    ///
    /// * `keys` - Vector of keys to retrieve
    ///
    /// # Returns
    ///
    /// Vector of optional values, preserving key order
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dashflow::core::stores::{BaseStore, InMemoryStore};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut store = InMemoryStore::new();
    /// store.mset(vec![("a".to_string(), 1), ("b".to_string(), 2)]).await?;
    ///
    /// let values = store.mget(vec!["a".to_string(), "c".to_string()]).await?;
    /// assert_eq!(values, vec![Some(1), None]);
    /// # Ok(())
    /// # }
    /// ```
    async fn mget(&self, keys: Vec<K>) -> Result<Vec<Option<V>>>;

    /// Set values for multiple key-value pairs.
    ///
    /// Existing keys are overwritten. Missing keys are created.
    ///
    /// # Arguments
    ///
    /// * `key_value_pairs` - Vector of (key, value) tuples to store
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dashflow::core::stores::{BaseStore, InMemoryStore};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut store = InMemoryStore::new();
    /// store.mset(vec![
    ///     ("key1".to_string(), "value1".to_string()),
    ///     ("key2".to_string(), "value2".to_string()),
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn mset(&mut self, key_value_pairs: Vec<(K, V)>) -> Result<()>;

    /// Delete multiple keys.
    ///
    /// Non-existent keys are silently ignored (no error).
    ///
    /// # Arguments
    ///
    /// * `keys` - Vector of keys to delete
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dashflow::core::stores::{BaseStore, InMemoryStore};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut store = InMemoryStore::new();
    /// store.mset(vec![("a".to_string(), 1)]).await?;
    /// store.mdelete(vec!["a".to_string(), "nonexistent".to_string()]).await?;
    ///
    /// let values = store.mget(vec!["a".to_string()]).await?;
    /// assert_eq!(values, vec![None]);
    /// # Ok(())
    /// # }
    /// ```
    async fn mdelete(&mut self, keys: Vec<K>) -> Result<()>;

    /// Get a stream of keys matching an optional prefix.
    ///
    /// If `prefix` is `None`, all keys are returned.
    /// If `prefix` is `Some(p)`, only keys starting with `p` are returned.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Optional prefix filter
    ///
    /// # Returns
    ///
    /// Async stream of keys
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dashflow::core::stores::{BaseStore, InMemoryStore};
    /// # use futures::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut store = InMemoryStore::new();
    /// store.mset(vec![
    ///     ("user:123".to_string(), 1),
    ///     ("user:456".to_string(), 2),
    ///     ("config:x".to_string(), 3),
    /// ]).await?;
    ///
    /// let mut user_keys = store.yield_keys(Some("user:")).await;
    /// let mut count = 0;
    /// while let Some(_key) = user_keys.next().await {
    ///     count += 1;
    /// }
    /// assert_eq!(count, 2);
    /// # Ok(())
    /// # }
    /// ```
    async fn yield_keys(&self, prefix: Option<&str>) -> Pin<Box<dyn Stream<Item = K> + Send + '_>>;
}

/// In-memory store implementation using `HashMap`.
///
/// This store is backed by a standard `HashMap` and suitable for:
/// - Development and testing
/// - Short-lived caches
/// - Single-process applications
///
/// For production distributed systems, consider:
/// - Redis-backed stores
/// - PostgreSQL-backed stores
/// - DynamoDB-backed stores
///
/// # Type Parameters
///
/// - `V`: Value type (must be Clone)
///
/// Keys are always `String` for this implementation.
///
/// # Examples
///
/// ```rust
/// use dashflow::core::stores::{BaseStore, InMemoryStore};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Store any cloneable type
///     let mut store: InMemoryStore<Vec<f32>> = InMemoryStore::new();
///
///     store.mset(vec![
///         ("embedding:1".to_string(), vec![0.1, 0.2, 0.3]),
///         ("embedding:2".to_string(), vec![0.4, 0.5, 0.6]),
///     ]).await?;
///
///     let embeddings = store.mget(vec!["embedding:1".to_string()]).await?;
///     assert_eq!(embeddings[0].as_ref().unwrap().len(), 3);
///
///     Ok(())
/// }
/// ```
pub struct InMemoryStore<V>
where
    V: Clone + Send + Sync,
{
    store: HashMap<String, V>,
}

impl<V> InMemoryStore<V>
where
    V: Clone + Send + Sync,
{
    /// Create a new empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    /// Create a new in-memory store with a specified capacity.
    ///
    /// Pre-allocating capacity can improve performance when the number
    /// of entries is known in advance.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            store: HashMap::with_capacity(capacity),
        }
    }

    /// Get the number of entries in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Check if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Clear all entries from the store.
    pub fn clear(&mut self) {
        self.store.clear();
    }
}

impl<V> Default for InMemoryStore<V>
where
    V: Clone + Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<V> BaseStore<String, V> for InMemoryStore<V>
where
    V: Clone + Send + Sync,
{
    async fn mget(&self, keys: Vec<String>) -> Result<Vec<Option<V>>> {
        Ok(keys
            .iter()
            .map(|key| self.store.get(key).cloned())
            .collect())
    }

    async fn mset(&mut self, key_value_pairs: Vec<(String, V)>) -> Result<()> {
        for (key, value) in key_value_pairs {
            self.store.insert(key, value);
        }
        Ok(())
    }

    async fn mdelete(&mut self, keys: Vec<String>) -> Result<()> {
        for key in keys {
            self.store.remove(&key);
        }
        Ok(())
    }

    async fn yield_keys(
        &self,
        prefix: Option<&str>,
    ) -> Pin<Box<dyn Stream<Item = String> + Send + '_>> {
        let keys: Vec<String> = match prefix {
            Some(p) => self
                .store
                .keys()
                .filter(|k| k.starts_with(p))
                .cloned()
                .collect(),
            None => self.store.keys().cloned().collect(),
        };
        Box::pin(stream::iter(keys))
    }
}

/// In-memory byte store for binary data.
///
/// Specialized store for `Vec<u8>` (byte arrays). Useful for:
/// - Serialized data caching
/// - Binary embeddings
/// - Compressed data storage
/// - File content caching
///
/// # Examples
///
/// ```rust
/// use dashflow::core::stores::{BaseStore, InMemoryByteStore};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut store = InMemoryByteStore::new();
///
///     // Store binary data
///     store.mset(vec![
///         ("image:1".to_string(), b"PNG\x89...".to_vec()),
///         ("config".to_string(), br#"{"key": "value"}"#.to_vec()),
///     ]).await?;
///
///     // Retrieve and use
///     let data = store.mget(vec!["config".to_string()]).await?;
///     if let Some(bytes) = &data[0] {
///         let json = String::from_utf8_lossy(bytes);
///         println!("Config: {}", json);
///     }
///
///     Ok(())
/// }
/// ```
pub type InMemoryByteStore = InMemoryStore<Vec<u8>>;

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_in_memory_store_basic() {
        let mut store = InMemoryStore::new();

        // Test mset
        store
            .mset(vec![
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ])
            .await
            .unwrap();

        // Test mget - existing keys
        let values = store
            .mget(vec!["key1".to_string(), "key2".to_string()])
            .await
            .unwrap();
        assert_eq!(
            values,
            vec![Some("value1".to_string()), Some("value2".to_string())]
        );

        // Test mget - non-existent key
        let values = store.mget(vec!["nonexistent".to_string()]).await.unwrap();
        assert_eq!(values, vec![None]);

        // Test mget - mixed
        let values = store
            .mget(vec!["key1".to_string(), "nonexistent".to_string()])
            .await
            .unwrap();
        assert_eq!(values, vec![Some("value1".to_string()), None]);
    }

    #[tokio::test]
    async fn test_in_memory_store_overwrite() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![("key".to_string(), "value1".to_string())])
            .await
            .unwrap();

        let values = store.mget(vec!["key".to_string()]).await.unwrap();
        assert_eq!(values, vec![Some("value1".to_string())]);

        // Overwrite
        store
            .mset(vec![("key".to_string(), "value2".to_string())])
            .await
            .unwrap();

        let values = store.mget(vec!["key".to_string()]).await.unwrap();
        assert_eq!(values, vec![Some("value2".to_string())]);
    }

    #[tokio::test]
    async fn test_in_memory_store_delete() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("key1".to_string(), 1),
                ("key2".to_string(), 2),
                ("key3".to_string(), 3),
            ])
            .await
            .unwrap();

        // Delete existing keys
        store
            .mdelete(vec!["key1".to_string(), "key2".to_string()])
            .await
            .unwrap();

        let values = store
            .mget(vec![
                "key1".to_string(),
                "key2".to_string(),
                "key3".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(values, vec![None, None, Some(3)]);

        // Delete non-existent key (should not error)
        store
            .mdelete(vec!["nonexistent".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_yield_keys_no_prefix() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("key1".to_string(), 1),
                ("key2".to_string(), 2),
                ("other".to_string(), 3),
            ])
            .await
            .unwrap();

        let mut keys_stream = store.yield_keys(None).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort(); // HashMap iteration order is not guaranteed
        assert_eq!(keys, vec!["key1", "key2", "other"]);
    }

    #[tokio::test]
    async fn test_yield_keys_with_prefix() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("user:123".to_string(), 1),
                ("user:456".to_string(), 2),
                ("config:x".to_string(), 3),
                ("config:y".to_string(), 4),
            ])
            .await
            .unwrap();

        // Get keys with "user:" prefix
        let mut keys_stream = store.yield_keys(Some("user:")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["user:123", "user:456"]);

        // Get keys with "config:" prefix
        let mut keys_stream = store.yield_keys(Some("config:")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["config:x", "config:y"]);

        // Non-matching prefix
        let mut keys_stream = store.yield_keys(Some("nonexistent:")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        assert_eq!(keys.len(), 0);
    }

    #[tokio::test]
    async fn test_byte_store() {
        let mut store = InMemoryByteStore::new();

        store
            .mset(vec![
                ("data1".to_string(), b"hello".to_vec()),
                ("data2".to_string(), vec![0x01, 0x02, 0x03]),
            ])
            .await
            .unwrap();

        let values = store
            .mget(vec!["data1".to_string(), "data2".to_string()])
            .await
            .unwrap();

        assert_eq!(values[0].as_ref().unwrap(), b"hello");
        assert_eq!(values[1].as_ref().unwrap(), &[0x01, 0x02, 0x03]);
    }

    #[tokio::test]
    async fn test_store_helper_methods() {
        let mut store: InMemoryStore<i32> = InMemoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store
            .mset(vec![("a".to_string(), 1), ("b".to_string(), 2)])
            .await
            .unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 2);

        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[tokio::test]
    async fn test_with_capacity() {
        let store: InMemoryStore<i32> = InMemoryStore::with_capacity(100);
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    #[tokio::test]
    async fn test_complex_value_types() {
        #[derive(Clone, Debug, PartialEq)]
        struct ComplexValue {
            id: u64,
            data: Vec<String>,
        }

        let mut store: InMemoryStore<ComplexValue> = InMemoryStore::new();

        let value = ComplexValue {
            id: 42,
            data: vec!["hello".to_string(), "world".to_string()],
        };

        store
            .mset(vec![("complex".to_string(), value.clone())])
            .await
            .unwrap();

        let values = store.mget(vec!["complex".to_string()]).await.unwrap();
        assert_eq!(values[0].as_ref().unwrap(), &value);
    }

    #[tokio::test]
    async fn test_empty_operations() {
        let mut store = InMemoryStore::new();

        // Empty mset
        store.mset(vec![]).await.unwrap();
        assert!(store.is_empty());

        // Empty mget
        let values: Vec<Option<String>> = store.mget(vec![]).await.unwrap();
        assert_eq!(values.len(), 0);

        // Empty mdelete
        store.mdelete(vec![]).await.unwrap();
        assert!(store.is_empty());
    }

    #[tokio::test]
    async fn test_large_batch_operations() {
        let mut store = InMemoryStore::new();

        // Insert 1000 key-value pairs
        let pairs: Vec<_> = (0..1000).map(|i| (format!("key{}", i), i)).collect();
        store.mset(pairs).await.unwrap();

        assert_eq!(store.len(), 1000);

        // Retrieve subset
        let keys: Vec<_> = (0..100).map(|i| format!("key{}", i)).collect();
        let values = store.mget(keys).await.unwrap();
        assert_eq!(values.len(), 100);
        assert_eq!(values[0], Some(0));
        assert_eq!(values[99], Some(99));

        // Delete subset
        let delete_keys: Vec<_> = (0..500).map(|i| format!("key{}", i)).collect();
        store.mdelete(delete_keys).await.unwrap();
        assert_eq!(store.len(), 500);
    }

    #[tokio::test]
    async fn test_duplicate_keys_in_mset() {
        let mut store = InMemoryStore::new();

        // Set same key multiple times in one batch
        store
            .mset(vec![
                ("key".to_string(), "value1".to_string()),
                ("key".to_string(), "value2".to_string()),
                ("key".to_string(), "value3".to_string()),
            ])
            .await
            .unwrap();

        // Last value should win
        let values = store.mget(vec!["key".to_string()]).await.unwrap();
        assert_eq!(values, vec![Some("value3".to_string())]);
        assert_eq!(store.len(), 1);
    }

    #[tokio::test]
    async fn test_duplicate_keys_in_mget() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![("key".to_string(), "value".to_string())])
            .await
            .unwrap();

        // Request same key multiple times
        let values = store
            .mget(vec![
                "key".to_string(),
                "key".to_string(),
                "key".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(values.len(), 3);
        assert_eq!(
            values,
            vec![
                Some("value".to_string()),
                Some("value".to_string()),
                Some("value".to_string())
            ]
        );
    }

    #[tokio::test]
    async fn test_duplicate_keys_in_mdelete() {
        let mut store = InMemoryStore::new();

        store.mset(vec![("key".to_string(), 1)]).await.unwrap();

        // Delete same key multiple times
        store
            .mdelete(vec![
                "key".to_string(),
                "key".to_string(),
                "key".to_string(),
            ])
            .await
            .unwrap();

        let values = store.mget(vec!["key".to_string()]).await.unwrap();
        assert_eq!(values, vec![None]);
    }

    #[tokio::test]
    async fn test_special_characters_in_keys() {
        let mut store = InMemoryStore::new();

        let special_keys = vec![
            "key with spaces".to_string(),
            "key:with:colons".to_string(),
            "key/with/slashes".to_string(),
            "key.with.dots".to_string(),
            "key-with-dashes".to_string(),
            "key_with_underscores".to_string(),
            "key@with@symbols".to_string(),
            "key#with#hashes".to_string(),
            "".to_string(), // empty key
        ];

        let pairs: Vec<_> = special_keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect();

        store.mset(pairs).await.unwrap();

        let values = store.mget(special_keys.clone()).await.unwrap();
        for (i, value) in values.iter().enumerate() {
            assert_eq!(*value, Some(i));
        }
    }

    #[tokio::test]
    async fn test_unicode_keys() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("键".to_string(), "Chinese".to_string()),
                ("キー".to_string(), "Japanese".to_string()),
                ("🔑".to_string(), "Emoji".to_string()),
                ("مفتاح".to_string(), "Arabic".to_string()),
            ])
            .await
            .unwrap();

        let values = store
            .mget(vec![
                "键".to_string(),
                "キー".to_string(),
                "🔑".to_string(),
                "مفتاح".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(values[0], Some("Chinese".to_string()));
        assert_eq!(values[1], Some("Japanese".to_string()));
        assert_eq!(values[2], Some("Emoji".to_string()));
        assert_eq!(values[3], Some("Arabic".to_string()));
    }

    #[tokio::test]
    async fn test_very_long_keys() {
        let mut store = InMemoryStore::new();

        // 10KB key
        let long_key = "a".repeat(10_000);
        store.mset(vec![(long_key.clone(), 42)]).await.unwrap();

        let values = store.mget(vec![long_key]).await.unwrap();
        assert_eq!(values, vec![Some(42)]);
    }

    #[tokio::test]
    async fn test_very_long_values() {
        let mut store = InMemoryStore::new();

        // 1MB value
        let long_value = "x".repeat(1_000_000);
        store
            .mset(vec![("key".to_string(), long_value.clone())])
            .await
            .unwrap();

        let values = store.mget(vec!["key".to_string()]).await.unwrap();
        assert_eq!(values[0].as_ref().unwrap().len(), 1_000_000);
    }

    #[tokio::test]
    async fn test_yield_keys_empty_store() {
        let store: InMemoryStore<i32> = InMemoryStore::new();

        let mut keys_stream = store.yield_keys(None).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        assert_eq!(keys.len(), 0);
    }

    #[tokio::test]
    async fn test_yield_keys_empty_prefix() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![("a".to_string(), 1), ("b".to_string(), 2)])
            .await
            .unwrap();

        // Empty string prefix should match keys that start with empty string (all keys)
        let mut keys_stream = store.yield_keys(Some("")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_yield_keys_prefix_ordering() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("user:001".to_string(), 1),
                ("user:100".to_string(), 2),
                ("user:010".to_string(), 3),
                ("user:002".to_string(), 4),
            ])
            .await
            .unwrap();

        let mut keys_stream = store.yield_keys(Some("user:")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        // Lexicographic ordering
        assert_eq!(keys, vec!["user:001", "user:002", "user:010", "user:100"]);
    }

    #[tokio::test]
    async fn test_default_trait() {
        let store: InMemoryStore<i32> = Default::default();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[tokio::test]
    async fn test_store_after_clear() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![("a".to_string(), 1), ("b".to_string(), 2)])
            .await
            .unwrap();
        assert_eq!(store.len(), 2);

        store.clear();
        assert_eq!(store.len(), 0);

        // Can still use after clear
        store.mset(vec![("c".to_string(), 3)]).await.unwrap();
        assert_eq!(store.len(), 1);

        let values = store.mget(vec!["c".to_string()]).await.unwrap();
        assert_eq!(values, vec![Some(3)]);
    }

    #[tokio::test]
    async fn test_overwrite_multiple_times() {
        let mut store = InMemoryStore::new();

        for i in 0..10 {
            store.mset(vec![("key".to_string(), i)]).await.unwrap();

            let values = store.mget(vec!["key".to_string()]).await.unwrap();
            assert_eq!(values, vec![Some(i)]);
        }

        assert_eq!(store.len(), 1);
    }

    #[tokio::test]
    async fn test_mixed_operations_sequence() {
        let mut store = InMemoryStore::new();

        // Set
        store
            .mset(vec![("a".to_string(), 1), ("b".to_string(), 2)])
            .await
            .unwrap();

        // Get
        let values = store
            .mget(vec!["a".to_string(), "b".to_string()])
            .await
            .unwrap();
        assert_eq!(values, vec![Some(1), Some(2)]);

        // Update
        store.mset(vec![("a".to_string(), 10)]).await.unwrap();

        // Delete
        store.mdelete(vec!["b".to_string()]).await.unwrap();

        // Get again
        let values = store
            .mget(vec!["a".to_string(), "b".to_string()])
            .await
            .unwrap();
        assert_eq!(values, vec![Some(10), None]);

        // Add new
        store.mset(vec![("c".to_string(), 3)]).await.unwrap();

        // Check keys
        let mut keys_stream = store.yield_keys(None).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["a", "c"]);
    }

    #[tokio::test]
    async fn test_byte_store_with_empty_bytes() {
        let mut store = InMemoryByteStore::new();

        store
            .mset(vec![
                ("empty".to_string(), vec![]),
                ("nonempty".to_string(), vec![1, 2, 3]),
            ])
            .await
            .unwrap();

        let values = store
            .mget(vec!["empty".to_string(), "nonempty".to_string()])
            .await
            .unwrap();

        assert_eq!(values[0].as_ref().unwrap().len(), 0);
        assert_eq!(values[1].as_ref().unwrap(), &[1, 2, 3]);
    }

    #[tokio::test]
    async fn test_byte_store_with_binary_data() {
        let mut store = InMemoryByteStore::new();

        // Binary data with null bytes and high bytes
        let binary_data = vec![0x00, 0xFF, 0x80, 0x7F, 0x01, 0xFE];

        store
            .mset(vec![("binary".to_string(), binary_data.clone())])
            .await
            .unwrap();

        let values = store.mget(vec!["binary".to_string()]).await.unwrap();
        assert_eq!(values[0].as_ref().unwrap(), &binary_data);
    }

    #[tokio::test]
    async fn test_store_with_option_values() {
        let mut store: InMemoryStore<Option<i32>> = InMemoryStore::new();

        store
            .mset(vec![
                ("some".to_string(), Some(42)),
                ("none".to_string(), None),
            ])
            .await
            .unwrap();

        let values = store
            .mget(vec!["some".to_string(), "none".to_string()])
            .await
            .unwrap();

        assert_eq!(values[0], Some(Some(42)));
        assert_eq!(values[1], Some(None));
    }

    #[tokio::test]
    async fn test_store_with_result_values() {
        #[derive(Clone, Debug, PartialEq)]
        struct MyError(String);

        let mut store: InMemoryStore<std::result::Result<i32, MyError>> = InMemoryStore::new();

        store
            .mset(vec![
                ("ok".to_string(), Ok(42)),
                ("err".to_string(), Err(MyError("error".to_string()))),
            ])
            .await
            .unwrap();

        let values = store
            .mget(vec!["ok".to_string(), "err".to_string()])
            .await
            .unwrap();

        assert_eq!(values[0], Some(Ok(42)));
        assert_eq!(values[1], Some(Err(MyError("error".to_string()))));
    }

    #[tokio::test]
    async fn test_mget_preserves_order() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("a".to_string(), 1),
                ("b".to_string(), 2),
                ("c".to_string(), 3),
            ])
            .await
            .unwrap();

        // Request in specific order
        let values = store
            .mget(vec![
                "c".to_string(),
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
            ])
            .await
            .unwrap();

        // Should preserve request order
        assert_eq!(values, vec![Some(3), Some(1), Some(2), Some(3)]);
    }

    #[tokio::test]
    async fn test_prefix_matching_edge_cases() {
        let mut store = InMemoryStore::new();

        store
            .mset(vec![
                ("pre".to_string(), 1),
                ("prefix".to_string(), 2),
                ("prefixed".to_string(), 3),
                ("pre:fix".to_string(), 4),
                ("other".to_string(), 5),
            ])
            .await
            .unwrap();

        // Prefix "pre" should match all keys starting with "pre"
        let mut keys_stream = store.yield_keys(Some("pre")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["pre", "pre:fix", "prefix", "prefixed"]);

        // Prefix "prefix" should only match "prefix" and "prefixed"
        let mut keys_stream = store.yield_keys(Some("prefix")).await;
        let mut keys = Vec::new();
        while let Some(key) = keys_stream.next().await {
            keys.push(key);
        }

        keys.sort();
        assert_eq!(keys, vec!["prefix", "prefixed"]);
    }

    #[tokio::test]
    async fn test_capacity_hint() {
        let store: InMemoryStore<i32> = InMemoryStore::with_capacity(1000);
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
        // Capacity hint doesn't affect length, just pre-allocation
    }
}
