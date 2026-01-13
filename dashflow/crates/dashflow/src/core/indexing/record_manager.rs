//! Record management for tracking indexed documents
//!
//! The `RecordManager` abstraction tracks which documents have been written to a vector store
//! and when they were last updated. This enables intelligent indexing that avoids redundant
//! work and detects changes.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Record storing metadata about an indexed document
#[derive(Debug, Clone)]
pub struct Record {
    /// Optional group/source identifier for the document
    pub group_id: Option<String>,
    /// Timestamp when the record was last updated (seconds since UNIX epoch)
    pub updated_at: f64,
}

/// Abstract interface for tracking indexed documents
///
/// `RecordManager` implementations keep track of which documents have been indexed,
/// storing document IDs (content hashes), timestamps, and optional group IDs.
/// This metadata enables the indexing API to:
///
/// - Skip documents that haven't changed
/// - Detect outdated documents that should be removed
/// - Group documents by source for incremental cleanup
///
/// # Important Notes
///
/// 1. **Monotonic Timestamps**: The `get_time()` method must return monotonically
///    increasing timestamps. Use server time, not client time, to avoid clock drift.
/// 2. **Distributed Systems**: `RecordManager` is separate from the vector store,
///    creating potential consistency issues. Write to vector store first, then
///    update records.
/// 3. **Namespace Isolation**: Use distinct namespaces for different indexes to
///    avoid conflicts.
#[async_trait]
pub trait RecordManager: Send + Sync {
    /// Get the namespace for this record manager
    fn namespace(&self) -> &str;

    /// Create the storage schema (e.g., database tables)
    ///
    /// For in-memory implementations, this is a no-op.
    async fn create_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get current server time as high-resolution timestamp
    ///
    /// **IMPORTANT**: Must return server time, not client time, to ensure
    /// monotonically increasing timestamps. Otherwise, clock drift can cause
    /// data loss during cleanup.
    ///
    /// Returns timestamp as seconds since UNIX epoch (float for sub-second precision).
    async fn get_time(&self) -> f64;

    /// Upsert (insert or update) records
    ///
    /// Creates new records or updates existing ones with current timestamp.
    ///
    /// # Arguments
    ///
    /// * `keys` - Document IDs (usually content hashes) to upsert
    /// * `group_ids` - Optional source/group IDs for each document
    /// * `time_at_least` - Optional minimum timestamp validation (for clock drift detection)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - `keys` and `group_ids` lengths don't match (when `group_ids` provided)
    /// - `time_at_least` is in the future (indicates clock drift)
    async fn update(
        &self,
        keys: &[String],
        group_ids: Option<&[Option<String>]>,
        time_at_least: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Check if document IDs exist in the record store
    ///
    /// Returns a boolean for each key indicating existence.
    async fn exists(
        &self,
        keys: &[String],
    ) -> Result<Vec<bool>, Box<dyn std::error::Error + Send + Sync>>;

    /// List document IDs matching filter criteria
    ///
    /// # Arguments
    ///
    /// * `before` - Only include records updated before this timestamp
    /// * `after` - Only include records updated after this timestamp
    /// * `group_ids` - Only include records with these group IDs
    /// * `limit` - Maximum number of records to return
    ///
    /// # Returns
    ///
    /// List of document IDs matching all specified filters
    async fn list_keys(
        &self,
        before: Option<f64>,
        after: Option<f64>,
        group_ids: Option<&[String]>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete records by document ID
    ///
    /// Removes the specified records from the store.
    async fn delete_keys(
        &self,
        keys: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// In-memory implementation of `RecordManager` for testing and simple use cases
///
/// Stores all records in a `HashMap` in memory. Suitable for:
/// - Unit testing
/// - Small datasets (< 100K documents)
/// - Single-process applications
///
/// For production use with large datasets or distributed systems, implement
/// `RecordManager` with a database backend (`PostgreSQL`, Redis, etc.).
#[derive(Debug, Clone)]
pub struct InMemoryRecordManager {
    namespace: String,
    records: Arc<RwLock<HashMap<String, Record>>>,
}

impl InMemoryRecordManager {
    /// Create a new in-memory record manager
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace to isolate this index from others
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of records stored
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Check if the record store is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
}

#[async_trait]
impl RecordManager for InMemoryRecordManager {
    fn namespace(&self) -> &str {
        &self.namespace
    }

    async fn create_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // In-memory implementation doesn't need schema creation
        Ok(())
    }

    async fn get_time(&self) -> f64 {
        // Use system time for consistency
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    async fn update(
        &self,
        keys: &[String],
        group_ids: Option<&[Option<String>]>,
        time_at_least: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Validate inputs
        if let Some(gids) = group_ids {
            if keys.len() != gids.len() {
                return Err("Length of keys must match length of group_ids".into());
            }
        }

        let current_time = self.get_time().await;

        if let Some(time_at_least) = time_at_least {
            if time_at_least > current_time {
                return Err("time_at_least must be in the past".into());
            }
        }

        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());

        for (i, key) in keys.iter().enumerate() {
            let group_id = group_ids
                .and_then(|gids| gids.get(i))
                .and_then(std::clone::Clone::clone);

            records.insert(
                key.clone(),
                Record {
                    group_id,
                    updated_at: current_time,
                },
            );
        }

        Ok(())
    }

    async fn exists(
        &self,
        keys: &[String],
    ) -> Result<Vec<bool>, Box<dyn std::error::Error + Send + Sync>> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        Ok(keys.iter().map(|key| records.contains_key(key)).collect())
    }

    async fn list_keys(
        &self,
        before: Option<f64>,
        after: Option<f64>,
        group_ids: Option<&[String]>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());

        let mut result: Vec<String> = records
            .iter()
            .filter(|(_, record)| {
                // Filter by timestamp (before)
                if let Some(before_time) = before {
                    if record.updated_at >= before_time {
                        return false;
                    }
                }

                // Filter by timestamp (after)
                if let Some(after_time) = after {
                    if record.updated_at <= after_time {
                        return false;
                    }
                }

                // Filter by group_ids
                if let Some(gids) = group_ids {
                    if let Some(ref record_gid) = record.group_id {
                        if !gids.contains(record_gid) {
                            return false;
                        }
                    } else {
                        // Record has no group_id, doesn't match filter
                        return false;
                    }
                }

                true
            })
            .map(|(key, _)| key.clone())
            .collect();

        // Apply limit
        if let Some(limit_val) = limit {
            result.truncate(limit_val);
        }

        Ok(result)
    }

    async fn delete_keys(
        &self,
        keys: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        for key in keys {
            records.remove(key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_in_memory_record_manager_basic() {
        let manager = InMemoryRecordManager::new("test_in_memory_record_manager_basic");
        assert_eq!(manager.namespace(), "test_in_memory_record_manager_basic");
        assert!(manager.is_empty());

        // Create schema (no-op for in-memory)
        manager.create_schema().await.unwrap();

        // Add records
        let keys = vec!["doc1".to_string(), "doc2".to_string()];
        let group_ids = vec![Some("source1".to_string()), Some("source2".to_string())];
        manager.update(&keys, Some(&group_ids), None).await.unwrap();

        assert_eq!(manager.len(), 2);

        // Check existence
        let exists = manager.exists(&keys).await.unwrap();
        assert_eq!(exists, vec![true, true]);

        let non_existent = manager.exists(&["doc3".to_string()]).await.unwrap();
        assert_eq!(non_existent, vec![false]);
    }

    #[tokio::test]
    async fn test_record_manager_update_validation() {
        let manager = InMemoryRecordManager::new("test_record_manager_update_validation");

        // Mismatched lengths should fail
        let keys = vec!["doc1".to_string()];
        let group_ids = vec![Some("s1".to_string()), Some("s2".to_string())];
        let result = manager.update(&keys, Some(&group_ids), None).await;
        assert!(result.is_err());

        // time_at_least in future should fail
        let current = manager.get_time().await;
        let result = manager.update(&keys, None, Some(current + 1000.0)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_record_manager_list_keys_filtering() {
        let manager = InMemoryRecordManager::new("test_record_manager_list_keys_filtering");

        // Add records with different group_ids
        manager
            .update(
                &["doc1".to_string()],
                Some(&[Some("source1".to_string())]),
                None,
            )
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let time_between = manager.get_time().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        manager
            .update(
                &["doc2".to_string()],
                Some(&[Some("source2".to_string())]),
                None,
            )
            .await
            .unwrap();

        // List all
        let all_keys = manager.list_keys(None, None, None, None).await.unwrap();
        assert_eq!(all_keys.len(), 2);

        // Filter by group_id
        let source1_keys = manager
            .list_keys(None, None, Some(&["source1".to_string()]), None)
            .await
            .unwrap();
        assert_eq!(source1_keys, vec!["doc1".to_string()]);

        // Filter by time (before)
        let before_keys = manager
            .list_keys(Some(time_between), None, None, None)
            .await
            .unwrap();
        assert_eq!(before_keys.len(), 1);

        // Filter by time (after)
        let after_keys = manager
            .list_keys(None, Some(time_between), None, None)
            .await
            .unwrap();
        assert_eq!(after_keys.len(), 1);

        // Test limit
        let limited = manager.list_keys(None, None, None, Some(1)).await.unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[tokio::test]
    async fn test_record_manager_delete() {
        let manager = InMemoryRecordManager::new("test_record_manager_delete");

        let keys = vec!["doc1".to_string(), "doc2".to_string(), "doc3".to_string()];
        manager.update(&keys, None, None).await.unwrap();
        assert_eq!(manager.len(), 3);

        // Delete some keys
        manager
            .delete_keys(&["doc1".to_string(), "doc3".to_string()])
            .await
            .unwrap();
        assert_eq!(manager.len(), 1);

        let exists = manager.exists(&keys).await.unwrap();
        assert_eq!(exists, vec![false, true, false]);
    }

    #[tokio::test]
    async fn test_record_manager_upsert_behavior() {
        let manager = InMemoryRecordManager::new("test_record_manager_upsert_behavior");

        // Insert
        manager
            .update(
                &["doc1".to_string()],
                Some(&[Some("source1".to_string())]),
                None,
            )
            .await
            .unwrap();

        let time1 = {
            let records = manager.records.read().unwrap();
            records.get("doc1").unwrap().updated_at
        };

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update (should change timestamp and group_id)
        manager
            .update(
                &["doc1".to_string()],
                Some(&[Some("source2".to_string())]),
                None,
            )
            .await
            .unwrap();

        let records = manager.records.read().unwrap();
        let record = records.get("doc1").unwrap();
        assert!(record.updated_at > time1);
        assert_eq!(record.group_id, Some("source2".to_string()));
    }

    // --- Edge Case Tests ---

    #[tokio::test]
    async fn test_empty_keys_array() {
        let manager = InMemoryRecordManager::new("test_empty_keys_array");

        // Update with empty keys should succeed
        manager.update(&[], None, None).await.unwrap();
        assert!(manager.is_empty());

        // Exists with empty keys should return empty
        let exists = manager.exists(&[]).await.unwrap();
        assert_eq!(exists, Vec::<bool>::new());

        // Delete with empty keys should succeed
        manager.delete_keys(&[]).await.unwrap();
    }

    #[tokio::test]
    async fn test_single_record_operations() {
        let manager = InMemoryRecordManager::new("test_single_record_operations");

        // Single record with no group_id
        manager
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();
        assert_eq!(manager.len(), 1);

        let exists = manager.exists(&["doc1".to_string()]).await.unwrap();
        assert_eq!(exists, vec![true]);

        // List should return the single record
        let keys = manager.list_keys(None, None, None, None).await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "doc1");

        // Single record with group_id None
        manager
            .update(&["doc2".to_string()], Some(&[None]), None)
            .await
            .unwrap();
        assert_eq!(manager.len(), 2);

        let records = manager.records.read().unwrap();
        let record = records.get("doc2").unwrap();
        assert_eq!(record.group_id, None);
    }

    #[tokio::test]
    async fn test_many_records_stress() {
        let manager = InMemoryRecordManager::new("test_many_records_stress");

        // Add 1000 records
        let keys: Vec<String> = (0..1000).map(|i| format!("doc{}", i)).collect();
        manager.update(&keys, None, None).await.unwrap();
        assert_eq!(manager.len(), 1000);

        // Check all exist
        let exists = manager.exists(&keys).await.unwrap();
        assert!(exists.iter().all(|&e| e));

        // List all
        let all_keys = manager.list_keys(None, None, None, None).await.unwrap();
        assert_eq!(all_keys.len(), 1000);

        // Delete half
        let to_delete: Vec<String> = (0..500).map(|i| format!("doc{}", i)).collect();
        manager.delete_keys(&to_delete).await.unwrap();
        assert_eq!(manager.len(), 500);

        // Check deleted ones don't exist
        let exists_after = manager.exists(&to_delete).await.unwrap();
        assert!(exists_after.iter().all(|&e| !e));
    }

    #[tokio::test]
    async fn test_group_id_none_filtering() {
        let manager = InMemoryRecordManager::new("test_group_id_none_filtering");

        // Add records with and without group_ids
        manager
            .update(&["doc1".to_string()], Some(&[Some("g1".to_string())]), None)
            .await
            .unwrap();
        manager
            .update(&["doc2".to_string()], Some(&[None]), None)
            .await
            .unwrap();
        manager
            .update(&["doc3".to_string()], None, None)
            .await
            .unwrap();

        // Filter by group_id - should only return doc1
        let filtered = manager
            .list_keys(None, None, Some(&["g1".to_string()]), None)
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "doc1");

        // Filter by non-existent group should return empty
        let empty = manager
            .list_keys(None, None, Some(&["nonexistent".to_string()]), None)
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_list_keys_multiple_group_ids() {
        let manager = InMemoryRecordManager::new("test_list_keys_multiple_group_ids");

        manager
            .update(&["doc1".to_string()], Some(&[Some("g1".to_string())]), None)
            .await
            .unwrap();
        manager
            .update(&["doc2".to_string()], Some(&[Some("g2".to_string())]), None)
            .await
            .unwrap();
        manager
            .update(&["doc3".to_string()], Some(&[Some("g3".to_string())]), None)
            .await
            .unwrap();

        // Filter by multiple group_ids
        let filtered = manager
            .list_keys(
                None,
                None,
                Some(&["g1".to_string(), "g3".to_string()]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"doc1".to_string()));
        assert!(filtered.contains(&"doc3".to_string()));
    }

    #[tokio::test]
    async fn test_list_keys_before_and_after_combined() {
        let manager = InMemoryRecordManager::new("test_list_keys_before_and_after_combined");

        manager
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let time1 = manager.get_time().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        manager
            .update(&["doc2".to_string()], None, None)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let time2 = manager.get_time().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        manager
            .update(&["doc3".to_string()], None, None)
            .await
            .unwrap();

        // Query window between time1 and time2 should return doc2
        let windowed = manager
            .list_keys(Some(time2), Some(time1), None, None)
            .await
            .unwrap();
        assert_eq!(windowed.len(), 1);
        assert_eq!(windowed[0], "doc2");

        // Query with before < after should return empty
        let empty = manager
            .list_keys(Some(time1), Some(time2), None, None)
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_exists_non_existent_keys() {
        let manager = InMemoryRecordManager::new("test_exists_non_existent_keys");

        manager
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();

        // Mix of existent and non-existent
        let keys = vec![
            "doc1".to_string(),
            "nonexistent".to_string(),
            "doc2".to_string(),
        ];
        let exists = manager.exists(&keys).await.unwrap();
        assert_eq!(exists, vec![true, false, false]);
    }

    #[tokio::test]
    async fn test_delete_non_existent_keys() {
        let manager = InMemoryRecordManager::new("test_delete_non_existent_keys");

        manager
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();
        assert_eq!(manager.len(), 1);

        // Delete non-existent keys should succeed (no-op)
        manager
            .delete_keys(&["nonexistent".to_string(), "doc2".to_string()])
            .await
            .unwrap();
        assert_eq!(manager.len(), 1);

        // Delete mix of existent and non-existent
        manager
            .delete_keys(&["doc1".to_string(), "nonexistent".to_string()])
            .await
            .unwrap();
        assert!(manager.is_empty());
    }

    #[tokio::test]
    async fn test_list_keys_with_zero_limit() {
        let manager = InMemoryRecordManager::new("test_list_keys_with_zero_limit");

        let keys = vec!["doc1".to_string(), "doc2".to_string(), "doc3".to_string()];
        manager.update(&keys, None, None).await.unwrap();

        // Limit of 0 should return empty
        let result = manager.list_keys(None, None, None, Some(0)).await.unwrap();
        assert!(result.is_empty());

        // Limit larger than result set should return all
        let result = manager
            .list_keys(None, None, None, Some(100))
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_update_with_time_at_least_validation() {
        let manager = InMemoryRecordManager::new("test_update_with_time_at_least_validation");

        // Get current time
        let now = manager.get_time().await;

        // Update with time_at_least in the past should succeed
        manager
            .update(&["doc1".to_string()], None, Some(now - 1.0))
            .await
            .unwrap();

        // time_at_least equal to now might succeed (timing dependent)
        let _result = manager.update(&["doc2".to_string()], None, Some(now)).await;
        // Don't assert - timing dependent

        // time_at_least far in future should fail
        let result = manager
            .update(&["doc3".to_string()], None, Some(now + 10000.0))
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("time_at_least must be in the past"));
    }

    #[tokio::test]
    async fn test_get_time_monotonicity() {
        let manager = InMemoryRecordManager::new("test_get_time_monotonicity");

        let mut times = Vec::new();
        for _ in 0..10 {
            let time = manager.get_time().await;
            times.push(time);
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        // Times should be monotonically increasing (or at least non-decreasing)
        for i in 1..times.len() {
            assert!(
                times[i] >= times[i - 1],
                "Time went backwards: {} < {}",
                times[i],
                times[i - 1]
            );
        }
    }

    #[tokio::test]
    async fn test_namespace_isolation() {
        let manager1 = InMemoryRecordManager::new("namespace1");
        let manager2 = InMemoryRecordManager::new("namespace2");

        assert_eq!(manager1.namespace(), "namespace1");
        assert_eq!(manager2.namespace(), "namespace2");

        // Add record to manager1
        manager1
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();

        // manager2 should be independent
        assert_eq!(manager1.len(), 1);
        assert_eq!(manager2.len(), 0);
    }

    #[tokio::test]
    async fn test_clone_independence() {
        let manager1 = InMemoryRecordManager::new("test_clone_independence");

        manager1
            .update(&["doc1".to_string()], None, None)
            .await
            .unwrap();

        // Clone shares the same underlying storage (Arc)
        let manager2 = manager1.clone();
        assert_eq!(manager2.len(), 1);

        // Updates through clone should be visible in original
        manager2
            .update(&["doc2".to_string()], None, None)
            .await
            .unwrap();
        assert_eq!(manager1.len(), 2);
        assert_eq!(manager2.len(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_updates() {
        use std::sync::Arc as StdArc;

        let manager = StdArc::new(InMemoryRecordManager::new("test_concurrent_updates"));

        // Spawn multiple concurrent tasks updating different keys
        let mut handles = vec![];
        for i in 0..10 {
            let manager_clone = StdArc::clone(&manager);
            let handle = tokio::spawn(async move {
                let key = format!("doc{}", i);
                manager_clone.update(&[key], None, None).await.unwrap();
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Should have 10 records
        assert_eq!(manager.len(), 10);
    }

    #[tokio::test]
    async fn test_update_overwrites_group_id() {
        let manager = InMemoryRecordManager::new("test_update_overwrites_group_id");

        // Initial insert with group_id
        manager
            .update(&["doc1".to_string()], Some(&[Some("g1".to_string())]), None)
            .await
            .unwrap();

        {
            let records = manager.records.read().unwrap();
            assert_eq!(
                records.get("doc1").unwrap().group_id,
                Some("g1".to_string())
            );
        }

        // Update to None group_id
        manager
            .update(&["doc1".to_string()], Some(&[None]), None)
            .await
            .unwrap();

        {
            let records = manager.records.read().unwrap();
            assert_eq!(records.get("doc1").unwrap().group_id, None);
        }

        // Update to different group_id
        manager
            .update(&["doc1".to_string()], Some(&[Some("g2".to_string())]), None)
            .await
            .unwrap();

        let records = manager.records.read().unwrap();
        assert_eq!(
            records.get("doc1").unwrap().group_id,
            Some("g2".to_string())
        );
    }

    #[tokio::test]
    async fn test_empty_string_group_id() {
        let manager = InMemoryRecordManager::new("test_empty_string_group_id");

        // Empty string as group_id should be valid
        manager
            .update(&["doc1".to_string()], Some(&[Some("".to_string())]), None)
            .await
            .unwrap();

        {
            let records = manager.records.read().unwrap();
            assert_eq!(records.get("doc1").unwrap().group_id, Some("".to_string()));
        }

        // Should be filterable
        let filtered = manager
            .list_keys(None, None, Some(&["".to_string()]), None)
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "doc1");
    }
}
