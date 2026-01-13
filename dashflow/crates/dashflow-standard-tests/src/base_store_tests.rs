//! Standard tests for `BaseStore` implementations
//!
//! These tests ensure that `BaseStore` implementations conform to the expected behavior
//! defined in the Python `DashFlow` standard tests.
//!
//! All tests are marked with STANDARD TEST labels to indicate they are ports
//! from Python `DashFlow` and should not be removed without careful consideration.

use dashflow::core::stores::BaseStore;
use futures::StreamExt;

/// Base trait for `BaseStore` standard tests (synchronous variant)
///
/// Test suites should implement this trait to run all standard tests against
/// their `BaseStore` implementation. This ensures API compatibility with Python `DashFlow`.
///
/// Note: "Synchronous" here refers to the Python test structure. In Rust, all
/// `BaseStore` methods are async, but the tests themselves follow the same logic
/// as Python's synchronous `BaseStore` tests.
#[async_trait::async_trait]
pub trait BaseStoreTests<V>
where
    V: Clone + Send + Sync + PartialEq + std::fmt::Debug + 'static,
{
    /// Returns a fresh, empty store instance for testing
    ///
    /// The store must be completely empty before each test.
    async fn store(&self) -> Box<dyn BaseStore<String, V>>;

    /// Returns three distinct example values for testing
    ///
    /// These values will be used to test set/get/delete operations.
    fn three_values(&self) -> (V, V, V);

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_kv_store_is_empty` (line 47)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that the key-value store is empty.
    async fn test_kv_store_is_empty(&self) {
        let store = self.store().await;
        let keys = vec!["foo".to_string(), "bar".to_string(), "buzz".to_string()];
        let values = store.mget(keys).await.expect("mget should succeed");
        assert_eq!(values, vec![None, None, None]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_set_and_get_values` (line 52)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test setting and getting values in the key-value store.
    async fn test_set_and_get_values(&self) {
        let mut store = self.store().await;
        let (value1, value2, _) = self.three_values();

        let key_value_pairs = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
        ];
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        let values = store
            .mget(vec!["foo".to_string(), "bar".to_string()])
            .await
            .expect("mget should succeed");
        assert_eq!(values, vec![Some(value1), Some(value2)]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_store_still_empty` (line 64)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that the store is still empty.
    ///
    /// This test should follow a test that sets values.
    /// This just verifies that the fixture is set up properly to be empty after each test.
    async fn test_store_still_empty(&self) {
        let store = self.store().await;
        let keys = vec!["foo".to_string()];
        let values = store.mget(keys).await.expect("mget should succeed");
        assert_eq!(values, vec![None]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_delete_values` (line 75)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test deleting values from the key-value store.
    async fn test_delete_values(&self) {
        let mut store = self.store().await;
        let (value1, value2, _) = self.three_values();

        let key_value_pairs = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
        ];
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        store
            .mdelete(vec!["foo".to_string()])
            .await
            .expect("mdelete should succeed");

        let values = store
            .mget(vec!["foo".to_string(), "bar".to_string()])
            .await
            .expect("mget should succeed");
        assert_eq!(values, vec![None, Some(value2)]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_delete_bulk_values` (line 88)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that we can delete several values at once.
    async fn test_delete_bulk_values(&self) {
        let mut store = self.store().await;
        let (value1, value2, value3) = self.three_values();

        let key_values = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
            ("buz".to_string(), value3.clone()),
        ];
        store.mset(key_values).await.expect("mset should succeed");

        store
            .mdelete(vec!["foo".to_string(), "buz".to_string()])
            .await
            .expect("mdelete should succeed");

        let values = store
            .mget(vec![
                "foo".to_string(),
                "bar".to_string(),
                "buz".to_string(),
            ])
            .await
            .expect("mget should succeed");
        assert_eq!(values, vec![None, Some(value2), None]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_delete_missing_keys` (line 100)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Deleting missing keys should not raise an exception.
    async fn test_delete_missing_keys(&self) {
        let mut store = self.store().await;

        store
            .mdelete(vec!["foo".to_string()])
            .await
            .expect("mdelete should succeed");

        store
            .mdelete(vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string(),
            ])
            .await
            .expect("mdelete should succeed");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_set_values_is_idempotent` (line 105)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Setting values by key should be idempotent.
    async fn test_set_values_is_idempotent(&self) {
        let mut store = self.store().await;
        let (value1, value2, _) = self.three_values();

        let key_value_pairs = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
        ];
        store
            .mset(key_value_pairs.clone())
            .await
            .expect("mset should succeed");
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        let values = store
            .mget(vec!["foo".to_string(), "bar".to_string()])
            .await
            .expect("mget should succeed");
        assert_eq!(values, vec![Some(value1), Some(value2)]);

        let mut keys: Vec<String> = store.yield_keys(None).await.collect().await;
        keys.sort();
        assert_eq!(keys, vec!["bar".to_string(), "foo".to_string()]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_get_can_get_same_value` (line 118)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that the same value can be retrieved multiple times.
    async fn test_get_can_get_same_value(&self) {
        let mut store = self.store().await;
        let (value1, value2, _) = self.three_values();

        let key_value_pairs = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
        ];
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        // This test assumes kv_store does not handle duplicates by default
        let values = store
            .mget(vec![
                "foo".to_string(),
                "bar".to_string(),
                "foo".to_string(),
                "bar".to_string(),
            ])
            .await
            .expect("mget should succeed");
        assert_eq!(
            values,
            vec![
                Some(value1.clone()),
                Some(value2.clone()),
                Some(value1),
                Some(value2)
            ]
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_overwrite_values_by_key` (line 130)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that we can overwrite values by key using mset.
    async fn test_overwrite_values_by_key(&self) {
        let mut store = self.store().await;
        let (value1, value2, value3) = self.three_values();

        let key_value_pairs = vec![
            ("foo".to_string(), value1.clone()),
            ("bar".to_string(), value2.clone()),
        ];
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        // Now overwrite value of key "foo"
        let new_key_value_pairs = vec![("foo".to_string(), value3.clone())];
        store
            .mset(new_key_value_pairs)
            .await
            .expect("mset should succeed");

        // Check that the value has been updated
        let values = store
            .mget(vec!["foo".to_string(), "bar".to_string()])
            .await
            .expect("mget should succeed");
        assert_eq!(values, vec![Some(value3), Some(value2)]);
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/base_store.py
    /// Python function: `test_yield_keys` (line 147)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that we can yield keys from the store.
    async fn test_yield_keys(&self) {
        let mut store = self.store().await;
        let (value1, value2, _) = self.three_values();

        let key_value_pairs = vec![("foo".to_string(), value1), ("bar".to_string(), value2)];
        store
            .mset(key_value_pairs)
            .await
            .expect("mset should succeed");

        // Collect all keys
        let mut all_keys: Vec<String> = store.yield_keys(None).await.collect().await;
        all_keys.sort();
        assert_eq!(all_keys, vec!["bar".to_string(), "foo".to_string()]);

        // Collect keys with prefix
        let mut prefix_keys: Vec<String> = store.yield_keys(Some("foo")).await.collect().await;
        prefix_keys.sort();
        assert_eq!(prefix_keys, vec!["foo".to_string()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::stores::InMemoryStore;

    /// Test implementation for InMemoryStore
    struct TestInMemoryStore;

    #[async_trait::async_trait]
    impl BaseStoreTests<String> for TestInMemoryStore {
        async fn store(&self) -> Box<dyn BaseStore<String, String>> {
            Box::new(InMemoryStore::<String>::new())
        }

        fn three_values(&self) -> (String, String, String) {
            (
                "value1".to_string(),
                "value2".to_string(),
                "value3".to_string(),
            )
        }
    }

    #[tokio::test]
    async fn test_in_memory_kv_store_is_empty() {
        let test = TestInMemoryStore;
        test.test_kv_store_is_empty().await;
    }

    #[tokio::test]
    async fn test_in_memory_set_and_get_values() {
        let test = TestInMemoryStore;
        test.test_set_and_get_values().await;
    }

    #[tokio::test]
    async fn test_in_memory_store_still_empty() {
        let test = TestInMemoryStore;
        test.test_store_still_empty().await;
    }

    #[tokio::test]
    async fn test_in_memory_delete_values() {
        let test = TestInMemoryStore;
        test.test_delete_values().await;
    }

    #[tokio::test]
    async fn test_in_memory_delete_bulk_values() {
        let test = TestInMemoryStore;
        test.test_delete_bulk_values().await;
    }

    #[tokio::test]
    async fn test_in_memory_delete_missing_keys() {
        let test = TestInMemoryStore;
        test.test_delete_missing_keys().await;
    }

    #[tokio::test]
    async fn test_in_memory_set_values_is_idempotent() {
        let test = TestInMemoryStore;
        test.test_set_values_is_idempotent().await;
    }

    #[tokio::test]
    async fn test_in_memory_get_can_get_same_value() {
        let test = TestInMemoryStore;
        test.test_get_can_get_same_value().await;
    }

    #[tokio::test]
    async fn test_in_memory_overwrite_values_by_key() {
        let test = TestInMemoryStore;
        test.test_overwrite_values_by_key().await;
    }

    #[tokio::test]
    async fn test_in_memory_yield_keys() {
        let test = TestInMemoryStore;
        test.test_yield_keys().await;
    }
}
