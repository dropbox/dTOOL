//! Standard tests for `BaseCache` implementations
//!
//! These tests verify that cache implementations correctly handle:
//! - Cache lookup (hit and miss)
//! - Cache updates
//! - Cache clearing
//! - Multiple generations per cache entry
//! - Proper isolation between tests (empty fixture)
//!
//! # Usage
//!
//! To test your cache implementation, implement the `CacheTests` trait:
//!
//! ```rust,ignore
//! use dashflow_standard_tests::cache_tests::CacheTests;
//! use dashflow::core::caches::{BaseCache, InMemoryCache};
//! use dashflow::core::language_models::ChatGeneration;
//! use async_trait::async_trait;
//!
//! struct MyCacheTests;
//!
//! #[async_trait]
//! impl CacheTests for MyCacheTests {
//!     type Cache = InMemoryCache;
//!
//!     async fn cache(&self) -> Self::Cache {
//!         InMemoryCache::new()
//!     }
//!
//!     fn get_sample_generation(&self) -> ChatGeneration {
//!         // Create sample generation
//!     }
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::caches::BaseCache;
use dashflow::core::language_models::ChatGeneration;

/// Standard test suite for `BaseCache` implementations
///
/// This trait provides a comprehensive test suite for `BaseCache` implementations.
/// All tests follow the Python `DashFlow` standard-tests for cache components.
///
/// Python has separate `SyncCacheTestSuite` and `AsyncCacheTestSuite` (14 tests total).
/// Rust's `BaseCache` trait is always async, so we have one unified `CacheTests` trait
/// with 7 async tests that cover both Python variants' functionality.
#[async_trait]
pub trait CacheTests: Send + Sync {
    /// The cache type being tested (must implement `BaseCache`)
    type Cache: BaseCache;

    /// Create a fresh cache instance for testing
    ///
    /// The returned cache MUST be empty. Each test gets a new cache instance
    /// to ensure test isolation.
    async fn cache(&self) -> Self::Cache;

    /// Get a sample prompt for testing
    ///
    /// Override this if you need a specific prompt format
    fn get_sample_prompt(&self) -> String {
        "Sample prompt for testing.".to_string()
    }

    /// Get a sample LLM string (configuration) for testing
    ///
    /// Override this if you need a specific LLM string format
    fn get_sample_llm_string(&self) -> String {
        "Sample LLM string configuration.".to_string()
    }

    /// Create a sample Generation object for testing
    ///
    /// This must be implemented by the test suite to provide a valid
    /// `ChatGeneration` for the specific cache implementation.
    fn get_sample_generation(&self) -> ChatGeneration;

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_cache_is_empty` (sync line 49, async line 142)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that the cache is empty.
    ///
    /// Verifies that looking up a non-existent entry returns None.
    async fn test_cache_is_empty(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();

        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(result.is_none(), "Expected cache to be empty");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_update_cache` (sync line 55, async line 149)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test updating the cache.
    ///
    /// Verifies that after updating the cache with a generation,
    /// the same generation can be retrieved.
    async fn test_update_cache(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();
        let generation = self.get_sample_generation();

        cache
            .update(&prompt, &llm_string, vec![generation.clone()])
            .await;

        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(result.is_some(), "Expected cache hit after update");

        let cached = result.unwrap();
        assert_eq!(cached.len(), 1, "Expected one generation in cache");
        assert_eq!(
            cached[0].message.as_text(),
            generation.message.as_text(),
            "Cached generation text should match"
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_cache_still_empty` (sync line 63, async line 157)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that the cache is still empty after previous tests.
    ///
    /// This verifies that the `cache()` fixture provides a fresh,
    /// empty cache for each test (proper test isolation).
    async fn test_cache_still_empty(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();

        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(
            result.is_none(),
            "Expected cache to still be empty (fixture isolation)"
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_clear_cache` (sync line 75, async line 170)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test clearing the cache.
    ///
    /// Verifies that after updating and then clearing the cache,
    /// lookups return None.
    async fn test_clear_cache(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();
        let generation = self.get_sample_generation();

        // Add to cache
        cache.update(&prompt, &llm_string, vec![generation]).await;

        // Verify it's there
        assert!(cache.lookup(&prompt, &llm_string).await.is_some());

        // Clear cache
        cache.clear().await.expect("Cache clear should succeed");

        // Verify it's gone
        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(result.is_none(), "Expected cache to be empty after clear");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_cache_miss` (sync line 84, async line 179)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test cache miss with non-existent prompt.
    ///
    /// Verifies that looking up a prompt that was never cached returns None.
    async fn test_cache_miss(&self) {
        let cache = self.cache().await;
        let llm_string = self.get_sample_llm_string();

        let result = cache.lookup("Nonexistent prompt", &llm_string).await;
        assert!(
            result.is_none(),
            "Expected cache miss for non-existent prompt"
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_cache_hit` (sync line 88, async line 186)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test cache hit after update.
    ///
    /// Verifies that after updating the cache, the exact same
    /// generation can be retrieved.
    async fn test_cache_hit(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();
        let generation = self.get_sample_generation();

        cache
            .update(&prompt, &llm_string, vec![generation.clone()])
            .await;

        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(result.is_some(), "Expected cache hit");

        let cached = result.unwrap();
        assert_eq!(cached.len(), 1, "Expected one generation");
        assert_eq!(
            cached[0].message.as_text(),
            generation.message.as_text(),
            "Cached generation should match original"
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/cache.py
    /// Python function: `test_update_cache_with_multiple_generations` (sync line 96, async line 194)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test updating the cache with multiple Generation objects.
    ///
    /// Verifies that the cache can store and retrieve multiple generations
    /// for the same (prompt, `llm_string`) key.
    async fn test_update_cache_with_multiple_generations(&self) {
        let cache = self.cache().await;
        let prompt = self.get_sample_prompt();
        let llm_string = self.get_sample_llm_string();

        let generation1 = self.get_sample_generation();
        let generation2 = {
            let mut gen = self.get_sample_generation();
            // Modify to make it different (simple approach - change text)
            if let dashflow::core::messages::Message::AI { content, .. } = &mut gen.message {
                *content = "Another generated text.".into();
            }
            gen
        };

        let generations = vec![generation1.clone(), generation2.clone()];
        cache
            .update(&prompt, &llm_string, generations.clone())
            .await;

        let result = cache.lookup(&prompt, &llm_string).await;
        assert!(result.is_some(), "Expected cache hit");

        let cached = result.unwrap();
        assert_eq!(cached.len(), 2, "Expected two generations in cache");
        assert_eq!(
            cached[0].message.as_text(),
            generation1.message.as_text(),
            "First generation should match"
        );
        assert_eq!(
            cached[1].message.as_text(),
            generation2.message.as_text(),
            "Second generation should match"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::caches::InMemoryCache;
    use dashflow::core::messages::Message;

    /// Test implementation for InMemoryCache
    struct InMemoryCacheTests;

    #[async_trait]
    impl CacheTests for InMemoryCacheTests {
        type Cache = InMemoryCache;

        async fn cache(&self) -> Self::Cache {
            InMemoryCache::new()
        }

        fn get_sample_generation(&self) -> ChatGeneration {
            ChatGeneration {
                message: Message::AI {
                    content: "Sample generated text.".into(),
                    tool_calls: vec![],
                    invalid_tool_calls: vec![],
                    usage_metadata: None,
                    fields: Default::default(),
                },
                generation_info: None,
            }
        }
    }

    #[tokio::test]
    async fn test_in_memory_cache_is_empty() {
        let suite = InMemoryCacheTests;
        suite.test_cache_is_empty().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_update() {
        let suite = InMemoryCacheTests;
        suite.test_update_cache().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_still_empty() {
        let suite = InMemoryCacheTests;
        suite.test_cache_still_empty().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_clear() {
        let suite = InMemoryCacheTests;
        suite.test_clear_cache().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_miss() {
        let suite = InMemoryCacheTests;
        suite.test_cache_miss().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_hit() {
        let suite = InMemoryCacheTests;
        suite.test_cache_hit().await;
    }

    #[tokio::test]
    async fn test_in_memory_cache_multiple_generations() {
        let suite = InMemoryCacheTests;
        suite.test_update_cache_with_multiple_generations().await;
    }
}
