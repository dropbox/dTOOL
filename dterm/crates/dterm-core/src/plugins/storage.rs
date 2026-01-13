//! Plugin storage API.
//!
//! Provides a sandboxed key-value storage system for plugins. Each plugin
//! gets its own isolated namespace with configurable size limits.
//!
//! ## Security Model
//!
//! - Each plugin has isolated storage (no cross-plugin access)
//! - Total storage per plugin is capped (default 1 MiB)
//! - Individual key/value sizes are limited
//! - Storage requires the `Storage` permission
//!
//! ## Persistence
//!
//! Storage is currently in-memory only. Future versions may support
//! optional persistence to disk with encryption.

use std::collections::HashMap;

use super::types::PluginId;

/// Maximum key length in bytes for plugin storage.
pub const PLUGIN_STORAGE_MAX_KEY_LENGTH: usize = 256;

/// Maximum value length in bytes for plugin storage.
pub const PLUGIN_STORAGE_MAX_VALUE_LENGTH: usize = 64 * 1024; // 64 KiB

/// Default storage quota per plugin in bytes.
pub const PLUGIN_STORAGE_DEFAULT_QUOTA: usize = 1024 * 1024; // 1 MiB

/// Configuration for plugin storage.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Maximum total storage per plugin (bytes).
    pub quota: usize,
    /// Maximum key length (bytes).
    pub max_key_length: usize,
    /// Maximum value length (bytes).
    pub max_value_length: usize,
    /// Maximum number of keys per plugin.
    pub max_keys: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            quota: PLUGIN_STORAGE_DEFAULT_QUOTA,
            max_key_length: PLUGIN_STORAGE_MAX_KEY_LENGTH,
            max_value_length: PLUGIN_STORAGE_MAX_VALUE_LENGTH,
            max_keys: 1000,
        }
    }
}

/// Errors from storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Key not found.
    NotFound,
    /// Key is too long.
    KeyTooLong {
        /// Actual length.
        length: usize,
        /// Maximum allowed.
        max: usize,
    },
    /// Value is too large.
    ValueTooLarge {
        /// Actual length.
        length: usize,
        /// Maximum allowed.
        max: usize,
    },
    /// Storage quota exceeded.
    QuotaExceeded {
        /// Current usage.
        current: usize,
        /// Requested additional bytes.
        requested: usize,
        /// Maximum quota.
        quota: usize,
    },
    /// Too many keys.
    TooManyKeys {
        /// Current count.
        current: usize,
        /// Maximum allowed.
        max: usize,
    },
    /// Permission denied (Storage permission not granted).
    PermissionDenied,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "key not found"),
            Self::KeyTooLong { length, max } => {
                write!(f, "key too long: {length} bytes (max {max})")
            }
            Self::ValueTooLarge { length, max } => {
                write!(f, "value too large: {length} bytes (max {max})")
            }
            Self::QuotaExceeded {
                current,
                requested,
                quota,
            } => {
                write!(
                    f,
                    "storage quota exceeded: {current} + {requested} > {quota}"
                )
            }
            Self::TooManyKeys { current, max } => {
                write!(f, "too many keys: {current} (max {max})")
            }
            Self::PermissionDenied => write!(f, "storage permission denied"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Storage for a single plugin.
#[derive(Debug)]
pub struct PluginStorage {
    /// The owning plugin.
    plugin_id: PluginId,
    /// Configuration.
    config: StorageConfig,
    /// Key-value data.
    data: HashMap<String, Vec<u8>>,
    /// Current total size (keys + values).
    current_size: usize,
}

impl PluginStorage {
    /// Create new storage for a plugin.
    pub fn new(plugin_id: PluginId) -> Self {
        Self::with_config(plugin_id, StorageConfig::default())
    }

    /// Create storage with custom configuration.
    pub fn with_config(plugin_id: PluginId, config: StorageConfig) -> Self {
        Self {
            plugin_id,
            config,
            data: HashMap::new(),
            current_size: 0,
        }
    }

    /// Get the plugin ID.
    pub fn plugin_id(&self) -> PluginId {
        self.plugin_id
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> StorageResult<&[u8]> {
        self.data.get(key).map(Vec::as_slice).ok_or(StorageError::NotFound)
    }

    /// Set a key-value pair.
    pub fn set(&mut self, key: &str, value: &[u8]) -> StorageResult<()> {
        // Validate key length
        if key.len() > self.config.max_key_length {
            return Err(StorageError::KeyTooLong {
                length: key.len(),
                max: self.config.max_key_length,
            });
        }

        // Validate value length
        if value.len() > self.config.max_value_length {
            return Err(StorageError::ValueTooLarge {
                length: value.len(),
                max: self.config.max_value_length,
            });
        }

        // Calculate size change
        let new_entry_size = key.len() + value.len();
        let old_entry_size = self
            .data
            .get(key)
            .map(|v| key.len() + v.len())
            .unwrap_or(0);

        // Check if this is a new key
        if old_entry_size == 0 && self.data.len() >= self.config.max_keys {
            return Err(StorageError::TooManyKeys {
                current: self.data.len(),
                max: self.config.max_keys,
            });
        }

        // Check quota
        let new_total = self.current_size - old_entry_size + new_entry_size;
        if new_total > self.config.quota {
            return Err(StorageError::QuotaExceeded {
                current: self.current_size - old_entry_size,
                requested: new_entry_size,
                quota: self.config.quota,
            });
        }

        // Update storage
        self.data.insert(key.to_string(), value.to_vec());
        self.current_size = new_total;

        Ok(())
    }

    /// Delete a key.
    pub fn delete(&mut self, key: &str) -> StorageResult<()> {
        if let Some(value) = self.data.remove(key) {
            self.current_size -= key.len() + value.len();
            Ok(())
        } else {
            Err(StorageError::NotFound)
        }
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Get all keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.data.keys().map(String::as_str)
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if storage is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get current storage usage in bytes.
    pub fn usage(&self) -> usize {
        self.current_size
    }

    /// Get storage quota in bytes.
    pub fn quota(&self) -> usize {
        self.config.quota
    }

    /// Get remaining storage in bytes.
    pub fn remaining(&self) -> usize {
        self.config.quota.saturating_sub(self.current_size)
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.data.clear();
        self.current_size = 0;
    }
}

/// Manager for multiple plugin storages.
#[derive(Debug)]
pub struct StorageManager {
    /// Per-plugin storage.
    storages: HashMap<PluginId, PluginStorage>,
    /// Default configuration for new storages.
    default_config: StorageConfig,
}

impl StorageManager {
    /// Create a new storage manager.
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
            default_config: StorageConfig::default(),
        }
    }

    /// Create with custom default configuration.
    pub fn with_config(config: StorageConfig) -> Self {
        Self {
            storages: HashMap::new(),
            default_config: config,
        }
    }

    /// Get or create storage for a plugin.
    pub fn get_or_create(&mut self, plugin_id: PluginId) -> &mut PluginStorage {
        self.storages
            .entry(plugin_id)
            .or_insert_with(|| PluginStorage::with_config(plugin_id, self.default_config.clone()))
    }

    /// Get storage for a plugin (if exists).
    pub fn get(&self, plugin_id: PluginId) -> Option<&PluginStorage> {
        self.storages.get(&plugin_id)
    }

    /// Get mutable storage for a plugin (if exists).
    pub fn get_mut(&mut self, plugin_id: PluginId) -> Option<&mut PluginStorage> {
        self.storages.get_mut(&plugin_id)
    }

    /// Remove storage for a plugin.
    pub fn remove(&mut self, plugin_id: PluginId) -> Option<PluginStorage> {
        self.storages.remove(&plugin_id)
    }

    /// Get total usage across all plugins.
    pub fn total_usage(&self) -> usize {
        self.storages.values().map(PluginStorage::usage).sum()
    }

    /// Get number of plugins with storage.
    pub fn plugin_count(&self) -> usize {
        self.storages.len()
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_basic_operations() {
        let mut storage = PluginStorage::new(PluginId(1));

        // Set and get
        storage.set("key1", b"value1").unwrap();
        assert_eq!(storage.get("key1").unwrap(), b"value1");

        // Update
        storage.set("key1", b"new_value").unwrap();
        assert_eq!(storage.get("key1").unwrap(), b"new_value");

        // Delete
        storage.delete("key1").unwrap();
        assert!(storage.get("key1").is_err());
    }

    #[test]
    fn test_storage_not_found() {
        let storage = PluginStorage::new(PluginId(1));
        let err = storage.get("missing").unwrap_err();
        assert_eq!(err, StorageError::NotFound);
    }

    #[test]
    fn test_storage_key_too_long() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                max_key_length: 10,
                ..Default::default()
            },
        );

        let err = storage.set("this_key_is_too_long", b"value").unwrap_err();
        assert!(matches!(err, StorageError::KeyTooLong { .. }));
    }

    #[test]
    fn test_storage_value_too_large() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                max_value_length: 10,
                ..Default::default()
            },
        );

        let large_value = [0u8; 100];
        let err = storage.set("key", &large_value).unwrap_err();
        assert!(matches!(err, StorageError::ValueTooLarge { .. }));
    }

    #[test]
    fn test_storage_quota_exceeded() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                quota: 100,
                ..Default::default()
            },
        );

        // This should fit
        storage.set("key1", &[0u8; 50]).unwrap();

        // This should exceed quota
        let err = storage.set("key2", &[0u8; 60]).unwrap_err();
        assert!(matches!(err, StorageError::QuotaExceeded { .. }));
    }

    #[test]
    fn test_storage_too_many_keys() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                max_keys: 3,
                quota: 1_000_000, // Large quota
                ..Default::default()
            },
        );

        storage.set("key1", b"v").unwrap();
        storage.set("key2", b"v").unwrap();
        storage.set("key3", b"v").unwrap();

        let err = storage.set("key4", b"v").unwrap_err();
        assert!(matches!(err, StorageError::TooManyKeys { .. }));
    }

    #[test]
    fn test_storage_update_within_quota() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                quota: 100,
                ..Default::default()
            },
        );

        // Fill most of quota
        storage.set("key", &[0u8; 90]).unwrap();

        // Update with same-size value should work
        storage.set("key", &[1u8; 90]).unwrap();

        // Update with smaller value should work
        storage.set("key", &[2u8; 10]).unwrap();

        // Now we have space for more
        storage.set("key2", &[3u8; 80]).unwrap();
    }

    #[test]
    fn test_storage_usage_tracking() {
        let mut storage = PluginStorage::new(PluginId(1));

        assert_eq!(storage.usage(), 0);
        assert!(storage.is_empty());

        storage.set("abc", b"12345").unwrap();
        assert_eq!(storage.usage(), 8); // 3 + 5
        assert_eq!(storage.len(), 1);
        assert!(!storage.is_empty());

        storage.set("xyz", b"1234567890").unwrap();
        assert_eq!(storage.usage(), 21); // 8 + 3 + 10

        storage.delete("abc").unwrap();
        assert_eq!(storage.usage(), 13); // 3 + 10
    }

    #[test]
    fn test_storage_clear() {
        let mut storage = PluginStorage::new(PluginId(1));

        storage.set("key1", b"value1").unwrap();
        storage.set("key2", b"value2").unwrap();
        assert!(!storage.is_empty());

        storage.clear();
        assert!(storage.is_empty());
        assert_eq!(storage.usage(), 0);
    }

    #[test]
    fn test_storage_contains() {
        let mut storage = PluginStorage::new(PluginId(1));

        assert!(!storage.contains("key"));
        storage.set("key", b"value").unwrap();
        assert!(storage.contains("key"));
    }

    #[test]
    fn test_storage_keys_iterator() {
        let mut storage = PluginStorage::new(PluginId(1));

        storage.set("alpha", b"1").unwrap();
        storage.set("beta", b"2").unwrap();
        storage.set("gamma", b"3").unwrap();

        let mut keys: Vec<_> = storage.keys().collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_storage_manager() {
        let mut manager = StorageManager::new();

        // Get or create storage for plugin 1
        {
            let storage = manager.get_or_create(PluginId(1));
            storage.set("key", b"value").unwrap();
        }

        // Get storage for plugin 1 (should exist)
        assert!(manager.get(PluginId(1)).is_some());
        assert_eq!(manager.get(PluginId(1)).unwrap().get("key").unwrap(), b"value");

        // Plugin 2 doesn't have storage yet
        assert!(manager.get(PluginId(2)).is_none());

        // Create storage for plugin 2
        manager.get_or_create(PluginId(2)).set("foo", b"bar").unwrap();

        assert_eq!(manager.plugin_count(), 2);

        // Remove plugin 1's storage
        let removed = manager.remove(PluginId(1));
        assert!(removed.is_some());
        assert_eq!(manager.plugin_count(), 1);
    }

    #[test]
    fn test_storage_manager_total_usage() {
        let mut manager = StorageManager::new();

        manager.get_or_create(PluginId(1)).set("a", b"12345").unwrap();
        manager.get_or_create(PluginId(2)).set("bb", b"1234567890").unwrap();

        // Plugin 1: 1 + 5 = 6
        // Plugin 2: 2 + 10 = 12
        assert_eq!(manager.total_usage(), 18);
    }

    #[test]
    fn test_storage_error_display() {
        assert_eq!(format!("{}", StorageError::NotFound), "key not found");
        assert_eq!(
            format!("{}", StorageError::KeyTooLong { length: 300, max: 256 }),
            "key too long: 300 bytes (max 256)"
        );
        assert_eq!(
            format!("{}", StorageError::PermissionDenied),
            "storage permission denied"
        );
    }

    #[test]
    fn test_storage_remaining() {
        let mut storage = PluginStorage::with_config(
            PluginId(1),
            StorageConfig {
                quota: 100,
                ..Default::default()
            },
        );

        assert_eq!(storage.remaining(), 100);
        storage.set("key", &[0u8; 30]).unwrap();
        assert_eq!(storage.remaining(), 100 - 3 - 30); // 67
    }
}
