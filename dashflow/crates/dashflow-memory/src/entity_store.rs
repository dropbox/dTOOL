//! Entity storage abstractions for entity memory
//!
//! Provides storage backends for entity summaries in entity-based memory systems.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Trait for entity storage backends
///
/// Entity stores maintain key-value pairs where keys are entity names
/// and values are entity summaries/descriptions.
pub trait EntityStore: Send + Sync {
    /// Get entity value from store
    fn get(&self, key: &str) -> Option<String>;

    /// Set entity value in store
    fn set(&mut self, key: &str, value: Option<String>);

    /// Delete entity value from store
    fn delete(&mut self, key: &str);

    /// Check if entity exists in store
    fn exists(&self, key: &str) -> bool;

    /// Delete all entities from store
    fn clear(&mut self);
}

/// In-memory entity store
///
/// Simple HashMap-backed entity store for testing and single-process use.
///
/// # Example
///
/// ```rust
/// use dashflow_memory::InMemoryEntityStore;
/// use dashflow_memory::EntityStore;
///
/// let mut store = InMemoryEntityStore::new();
/// store.set("Alice", Some("Software engineer from Seattle".to_string()));
/// assert_eq!(store.get("Alice"), Some("Software engineer from Seattle".to_string()));
/// assert!(store.exists("Alice"));
/// store.delete("Alice");
/// assert!(!store.exists("Alice"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct InMemoryEntityStore {
    store: Arc<RwLock<HashMap<String, String>>>,
}

impl InMemoryEntityStore {
    /// Create a new in-memory entity store
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl EntityStore for InMemoryEntityStore {
    fn get(&self, key: &str) -> Option<String> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to read rather than crash
        self.store
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    fn set(&mut self, key: &str, value: Option<String>) {
        if let Some(val) = value {
            // SAFETY: Use poison-safe pattern
            self.store
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key.to_string(), val);
        } else {
            self.delete(key);
        }
    }

    fn delete(&mut self, key: &str) {
        // SAFETY: Use poison-safe pattern
        self.store
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
    }

    fn exists(&self, key: &str) -> bool {
        // SAFETY: Use poison-safe pattern
        self.store
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(key)
    }

    fn clear(&mut self) {
        // SAFETY: Use poison-safe pattern
        self.store.write().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_entity_store() {
        let mut store = InMemoryEntityStore::new();

        // Test set and get
        store.set("Alice", Some("Engineer from Seattle".to_string()));
        assert_eq!(
            store.get("Alice"),
            Some("Engineer from Seattle".to_string())
        );

        // Test exists
        assert!(store.exists("Alice"));
        assert!(!store.exists("Bob"));

        // Test get with default
        assert_eq!(store.get("Bob"), None);

        // Test update
        store.set("Alice", Some("Senior Engineer from Seattle".to_string()));
        assert_eq!(
            store.get("Alice"),
            Some("Senior Engineer from Seattle".to_string())
        );

        // Test delete
        store.delete("Alice");
        assert!(!store.exists("Alice"));
        assert_eq!(store.get("Alice"), None);

        // Test set None (should delete)
        store.set("Charlie", Some("Data scientist".to_string()));
        assert!(store.exists("Charlie"));
        store.set("Charlie", None);
        assert!(!store.exists("Charlie"));

        // Test clear
        store.set("Alice", Some("Engineer".to_string()));
        store.set("Bob", Some("Manager".to_string()));
        assert!(store.exists("Alice"));
        assert!(store.exists("Bob"));
        store.clear();
        assert!(!store.exists("Alice"));
        assert!(!store.exists("Bob"));
    }
}
