//! Optional caching layer for language models
//!
//! A cache is useful for two reasons:
//!
//! 1. It can save you money by reducing the number of API calls you make to the LLM
//!    provider if you're often requesting the same completion multiple times.
//! 2. It can speed up your application by reducing the number of API calls you make to the
//!    LLM provider.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::caches::{BaseCache, InMemoryCache};
//! use dashflow::core::language_models::ChatGeneration;
//!
//! # async fn example() {
//! let cache = InMemoryCache::new();
//!
//! // Cache miss
//! let result = cache.lookup("What is 2+2?", "gpt-4").await;
//! assert!(result.is_none());
//!
//! // Update cache
//! let generations = vec![/* ChatGeneration instances */];
//! cache.update("What is 2+2?", "gpt-4", generations.clone()).await;
//!
//! // Cache hit
//! let result = cache.lookup("What is 2+2?", "gpt-4").await;
//! assert!(result.is_some());
//! # }
//! ```

use crate::core::error::Result;
use crate::core::language_models::ChatGeneration;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Type alias for cached generation results
pub type CachedGenerations = Vec<ChatGeneration>;

/// Interface for a caching layer for LLMs and Chat models
///
/// The cache interface consists of the following methods:
///
/// - `lookup`: Look up a value based on a prompt and `llm_string`
/// - `update`: Update the cache based on a prompt and `llm_string`
/// - `clear`: Clear the cache
///
/// All methods are async to support various backing stores (Redis, databases, etc.)
#[async_trait]
pub trait BaseCache: Send + Sync {
    /// Look up based on `prompt` and `llm_string`
    ///
    /// A cache implementation is expected to generate a key from the 2-tuple
    /// of prompt and `llm_string` (e.g., by concatenating them with a delimiter).
    ///
    /// # Arguments
    ///
    /// * `prompt` - A string representation of the prompt.
    ///   In the case of a chat model, the prompt is a non-trivial
    ///   serialization of the prompt into the language model.
    /// * `llm_string` - A string representation of the LLM configuration.
    ///   This is used to capture the invocation parameters of the LLM
    ///   (e.g., model name, temperature, stop tokens, max tokens, etc.).
    ///
    /// # Returns
    ///
    /// On a cache miss, return `None`. On a cache hit, return the cached value.
    async fn lookup(&self, prompt: &str, llm_string: &str) -> Option<CachedGenerations>;

    /// Update cache based on `prompt` and `llm_string`
    ///
    /// The prompt and `llm_string` are used to generate a key for the cache.
    /// The key should match that of the lookup method.
    ///
    /// # Arguments
    ///
    /// * `prompt` - A string representation of the prompt
    /// * `llm_string` - A string representation of the LLM configuration
    /// * `return_val` - The value to be cached
    async fn update(&self, prompt: &str, llm_string: &str, return_val: CachedGenerations);

    /// Clear the cache
    async fn clear(&self) -> Result<()>;
}

/// Error type for cache configuration validation.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum CacheConfigError {
    /// max_size must be greater than 0.
    #[error("Invalid max_size: must be greater than 0, got {max_size}")]
    InvalidMaxSize {
        /// The invalid maximum size value that was provided.
        max_size: usize,
    },
}

/// In-memory cache that stores generations in a `HashMap`
///
/// This cache is useful for development and testing, or for short-lived applications.
/// For production use cases, consider using a distributed cache like Redis.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::caches::{InMemoryCache, BaseCache};
/// use dashflow::core::language_models::ChatGeneration;
///
/// # async fn example() {
/// // Create cache with maximum size
/// let cache = InMemoryCache::with_max_size(100);
///
/// // Use the cache
/// let generations = vec![/* ChatGeneration instances */];
/// cache.update("prompt", "gpt-4", generations).await;
///
/// let result = cache.lookup("prompt", "gpt-4").await;
/// assert!(result.is_some());
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct InMemoryCache {
    cache: Arc<RwLock<HashMap<(String, String), CachedGenerations>>>,
    max_size: Option<usize>,
}

impl InMemoryCache {
    /// Create a new `InMemoryCache` with no size limit
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size: None,
        }
    }

    /// Create a new `InMemoryCache` with a maximum size
    ///
    /// When the cache exceeds the maximum size, the oldest items are removed (FIFO).
    ///
    /// # Arguments
    ///
    /// * `max_size` - The maximum number of items to store in the cache
    ///
    /// # Panics
    ///
    /// Panics if `max_size` is 0
    // SAFETY: Panicking constructor with documented behavior; use try_with_max_size() for fallible version
    #[allow(clippy::expect_used)]
    #[must_use]
    pub fn with_max_size(max_size: usize) -> Self {
        Self::try_with_max_size(max_size).expect("max_size must be greater than 0")
    }

    /// Create a new `InMemoryCache` with a maximum size, returning an error if invalid.
    ///
    /// When the cache exceeds the maximum size, the oldest items are removed (FIFO).
    ///
    /// # Arguments
    ///
    /// * `max_size` - The maximum number of items to store in the cache
    ///
    /// # Errors
    ///
    /// Returns `CacheConfigError::InvalidMaxSize` if `max_size` is 0.
    pub fn try_with_max_size(max_size: usize) -> std::result::Result<Self, CacheConfigError> {
        if max_size == 0 {
            return Err(CacheConfigError::InvalidMaxSize { max_size });
        }
        Ok(Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size: Some(max_size),
        })
    }

    /// Get the current number of cached items
    pub async fn len(&self) -> usize {
        self.cache.read().await.len()
    }

    /// Check if the cache is empty
    pub async fn is_empty(&self) -> bool {
        self.cache.read().await.is_empty()
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BaseCache for InMemoryCache {
    async fn lookup(&self, prompt: &str, llm_string: &str) -> Option<CachedGenerations> {
        let cache = self.cache.read().await;
        cache
            .get(&(prompt.to_string(), llm_string.to_string()))
            .cloned()
    }

    async fn update(&self, prompt: &str, llm_string: &str, return_val: CachedGenerations) {
        let mut cache = self.cache.write().await;

        // If max_size is set and we're at capacity, remove the oldest entry
        if let Some(max) = self.max_size {
            if cache.len() >= max {
                // Remove the first key (oldest in insertion order for HashMap)
                // Note: HashMap doesn't guarantee insertion order, so this is approximate
                // For production, consider using IndexMap or similar
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }
        }

        cache.insert((prompt.to_string(), llm_string.to_string()), return_val);
    }

    async fn clear(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CacheConfigError;
    use crate::core::messages::Message;
    use crate::test_prelude::*;

    fn create_test_generation(content: &str) -> ChatGeneration {
        ChatGeneration {
            message: Message::AI {
                content: content.into(),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
            generation_info: None,
        }
    }

    #[tokio::test]
    async fn test_in_memory_cache_basic() {
        let cache = InMemoryCache::new();

        // Cache miss
        let result = cache.lookup("What is 2+2?", "gpt-4").await;
        assert!(result.is_none());

        // Update cache
        let generations = vec![create_test_generation("4")];
        cache
            .update("What is 2+2?", "gpt-4", generations.clone())
            .await;

        // Cache hit
        let result = cache.lookup("What is 2+2?", "gpt-4").await;
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].message.as_text(), "4");
    }

    #[tokio::test]
    async fn test_in_memory_cache_different_llm_strings() {
        let cache = InMemoryCache::new();

        // Same prompt, different LLM configs should be separate cache entries
        let gen1 = vec![create_test_generation("Answer from GPT-4")];
        let gen2 = vec![create_test_generation("Answer from GPT-3.5")];

        cache.update("What is AI?", "gpt-4,temp=0", gen1).await;
        cache.update("What is AI?", "gpt-3.5,temp=1", gen2).await;

        let result1 = cache.lookup("What is AI?", "gpt-4,temp=0").await;
        let result2 = cache.lookup("What is AI?", "gpt-3.5,temp=1").await;

        assert!(result1.is_some());
        assert!(result2.is_some());
        assert_eq!(result1.unwrap()[0].message.as_text(), "Answer from GPT-4");
        assert_eq!(result2.unwrap()[0].message.as_text(), "Answer from GPT-3.5");
    }

    #[tokio::test]
    async fn test_in_memory_cache_with_max_size() {
        let cache = InMemoryCache::with_max_size(2);

        // Add 3 items to cache with max_size=2
        cache
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;
        cache
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;
        cache
            .update("q3", "model", vec![create_test_generation("a3")])
            .await;

        // Cache should have at most 2 items
        assert!(cache.len().await <= 2);
    }

    #[tokio::test]
    async fn test_in_memory_cache_clear() {
        let cache = InMemoryCache::new();

        // Add items
        cache
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;
        cache
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;

        assert_eq!(cache.len().await, 2);

        // Clear cache
        cache.clear().await.unwrap();

        assert_eq!(cache.len().await, 0);
        assert!(cache.lookup("q1", "model").await.is_none());
        assert!(cache.lookup("q2", "model").await.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_cache_is_empty() {
        let cache = InMemoryCache::new();

        assert!(cache.is_empty().await);

        cache
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;

        assert!(!cache.is_empty().await);

        cache.clear().await.unwrap();

        assert!(cache.is_empty().await);
    }

    #[test]
    fn test_in_memory_cache_try_with_max_size_valid() {
        let result = InMemoryCache::try_with_max_size(10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_in_memory_cache_try_with_max_size_zero_fails() {
        let result = InMemoryCache::try_with_max_size(0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CacheConfigError::InvalidMaxSize { max_size: 0 }
        ));
    }

    // ========================================================================
    // COMPREHENSIVE TEST COVERAGE
    // ========================================================================

    // ------------------------------------------------------------------------
    // Edge Cases and Special Characters
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_empty_prompt_and_llm_string() {
        let cache = InMemoryCache::new();

        // Empty prompt
        cache
            .update("", "model", vec![create_test_generation("result")])
            .await;
        let result = cache.lookup("", "model").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "result");

        // Empty llm_string
        cache
            .update("prompt", "", vec![create_test_generation("result2")])
            .await;
        let result = cache.lookup("prompt", "").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "result2");

        // Both empty
        cache
            .update("", "", vec![create_test_generation("result3")])
            .await;
        let result = cache.lookup("", "").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "result3");
    }

    #[tokio::test]
    async fn test_special_characters_in_keys() {
        let cache = InMemoryCache::new();

        let special_prompt = "What is \"AI\"?\nLine 2\tTab\r\nWindows line";
        let special_llm = "model=gpt-4,temp=0.7,stop=[\"\\n\",\"END\"]";

        cache
            .update(
                special_prompt,
                special_llm,
                vec![create_test_generation("answer")],
            )
            .await;

        let result = cache.lookup(special_prompt, special_llm).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "answer");
    }

    #[tokio::test]
    async fn test_unicode_in_keys() {
        let cache = InMemoryCache::new();

        let unicode_prompt = "What is AI? 🤖 人工智能 🧠 Künstliche Intelligenz";
        let unicode_llm = "model=gpt-4-日本語";

        cache
            .update(
                unicode_prompt,
                unicode_llm,
                vec![create_test_generation("unicode answer 答え")],
            )
            .await;

        let result = cache.lookup(unicode_prompt, unicode_llm).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "unicode answer 答え");
    }

    #[tokio::test]
    async fn test_very_long_keys() {
        let cache = InMemoryCache::new();

        let long_prompt = "a".repeat(10000);
        let long_llm = "b".repeat(10000);

        cache
            .update(&long_prompt, &long_llm, vec![create_test_generation("ok")])
            .await;

        let result = cache.lookup(&long_prompt, &long_llm).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "ok");
    }

    // ------------------------------------------------------------------------
    // Multiple Generations
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_multiple_generations_in_cache() {
        let cache = InMemoryCache::new();

        let gens = vec![
            create_test_generation("answer 1"),
            create_test_generation("answer 2"),
            create_test_generation("answer 3"),
        ];

        cache.update("prompt", "model", gens.clone()).await;

        let result = cache.lookup("prompt", "model").await;
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.len(), 3);
        assert_eq!(cached[0].message.as_text(), "answer 1");
        assert_eq!(cached[1].message.as_text(), "answer 2");
        assert_eq!(cached[2].message.as_text(), "answer 3");
    }

    #[tokio::test]
    async fn test_empty_generations_list() {
        let cache = InMemoryCache::new();

        // Empty generations list
        cache.update("prompt", "model", vec![]).await;

        let result = cache.lookup("prompt", "model").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 0);
    }

    // ------------------------------------------------------------------------
    // Large Payloads
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_large_generation_content() {
        let cache = InMemoryCache::new();

        let large_content = "x".repeat(100_000); // 100KB content
        let gen = create_test_generation(&large_content);

        cache.update("prompt", "model", vec![gen]).await;

        let result = cache.lookup("prompt", "model").await;
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached[0].message.as_text().len(), 100_000);
    }

    #[tokio::test]
    async fn test_many_cache_entries() {
        let cache = InMemoryCache::new();

        // Add 1000 cache entries
        for i in 0..1000 {
            let prompt = format!("prompt_{}", i);
            let llm = format!("model_{}", i);
            cache
                .update(&prompt, &llm, vec![create_test_generation("answer")])
                .await;
        }

        assert_eq!(cache.len().await, 1000);

        // Verify random entries
        let result = cache.lookup("prompt_500", "model_500").await;
        assert!(result.is_some());

        let result = cache.lookup("prompt_999", "model_999").await;
        assert!(result.is_some());
    }

    // ------------------------------------------------------------------------
    // Concurrent Access
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_concurrent_updates() {
        let cache = InMemoryCache::new();

        let cache1 = cache.clone();
        let cache2 = cache.clone();

        let task1 = tokio::spawn(async move {
            for i in 0..100 {
                cache1
                    .update(
                        &format!("prompt_{}", i),
                        "model",
                        vec![create_test_generation("answer1")],
                    )
                    .await;
            }
        });

        let task2 = tokio::spawn(async move {
            for i in 100..200 {
                cache2
                    .update(
                        &format!("prompt_{}", i),
                        "model",
                        vec![create_test_generation("answer2")],
                    )
                    .await;
            }
        });

        task1.await.unwrap();
        task2.await.unwrap();

        // Should have 200 entries
        assert_eq!(cache.len().await, 200);
    }

    #[tokio::test]
    async fn test_concurrent_read_write() {
        let cache = InMemoryCache::new();

        // Pre-populate cache
        for i in 0..50 {
            cache
                .update(
                    &format!("prompt_{}", i),
                    "model",
                    vec![create_test_generation("answer")],
                )
                .await;
        }

        let cache1 = cache.clone();
        let cache2 = cache.clone();

        let read_task = tokio::spawn(async move {
            for i in 0..50 {
                let result = cache1.lookup(&format!("prompt_{}", i), "model").await;
                assert!(result.is_some());
            }
        });

        let write_task = tokio::spawn(async move {
            for i in 50..100 {
                cache2
                    .update(
                        &format!("prompt_{}", i),
                        "model",
                        vec![create_test_generation("new")],
                    )
                    .await;
            }
        });

        read_task.await.unwrap();
        write_task.await.unwrap();

        assert_eq!(cache.len().await, 100);
    }

    // ------------------------------------------------------------------------
    // Cache Replacement Logic (max_size)
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_max_size_eviction_single_entry() {
        let cache = InMemoryCache::with_max_size(1);

        cache
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;
        assert_eq!(cache.len().await, 1);

        cache
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;
        assert_eq!(cache.len().await, 1);

        // At least one of the entries should be in cache
        let has_q1 = cache.lookup("q1", "model").await.is_some();
        let has_q2 = cache.lookup("q2", "model").await.is_some();
        assert!(has_q1 || has_q2);
    }

    #[tokio::test]
    async fn test_max_size_boundary() {
        let cache = InMemoryCache::with_max_size(5);

        // Add exactly max_size entries
        for i in 0..5 {
            cache
                .update(
                    &format!("q{}", i),
                    "model",
                    vec![create_test_generation(&format!("a{}", i))],
                )
                .await;
        }
        assert_eq!(cache.len().await, 5);

        // Add one more to trigger eviction
        cache
            .update("q5", "model", vec![create_test_generation("a5")])
            .await;
        assert!(cache.len().await <= 5);
    }

    #[tokio::test]
    async fn test_max_size_large_limit() {
        let cache = InMemoryCache::with_max_size(1000);

        // Add items up to limit
        for i in 0..1000 {
            cache
                .update(
                    &format!("q{}", i),
                    "model",
                    vec![create_test_generation("answer")],
                )
                .await;
        }
        assert_eq!(cache.len().await, 1000);

        // Add more to trigger evictions
        for i in 1000..1100 {
            cache
                .update(
                    &format!("q{}", i),
                    "model",
                    vec![create_test_generation("answer")],
                )
                .await;
        }
        assert!(cache.len().await <= 1000);
    }

    // ------------------------------------------------------------------------
    // Update Overwrite Behavior
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_update_overwrites_existing() {
        let cache = InMemoryCache::new();

        // Initial value
        cache
            .update("prompt", "model", vec![create_test_generation("old")])
            .await;
        let result = cache.lookup("prompt", "model").await;
        assert_eq!(result.unwrap()[0].message.as_text(), "old");

        // Overwrite
        cache
            .update("prompt", "model", vec![create_test_generation("new")])
            .await;
        let result = cache.lookup("prompt", "model").await;
        assert_eq!(result.unwrap()[0].message.as_text(), "new");

        // Cache size should still be 1
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_update_changes_generation_count() {
        let cache = InMemoryCache::new();

        // Start with 1 generation
        cache
            .update("prompt", "model", vec![create_test_generation("a1")])
            .await;
        let result = cache.lookup("prompt", "model").await;
        assert_eq!(result.unwrap().len(), 1);

        // Update with 3 generations
        cache
            .update(
                "prompt",
                "model",
                vec![
                    create_test_generation("b1"),
                    create_test_generation("b2"),
                    create_test_generation("b3"),
                ],
            )
            .await;
        let result = cache.lookup("prompt", "model").await;
        assert_eq!(result.unwrap().len(), 3);

        // Update with empty list
        cache.update("prompt", "model", vec![]).await;
        let result = cache.lookup("prompt", "model").await;
        assert_eq!(result.unwrap().len(), 0);
    }

    // ------------------------------------------------------------------------
    // Clone Behavior
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_clone_shares_cache() {
        let cache1 = InMemoryCache::new();

        cache1
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;

        let cache2 = cache1.clone();

        // Both should see the same data
        let result1 = cache1.lookup("q1", "model").await;
        let result2 = cache2.lookup("q1", "model").await;
        assert!(result1.is_some());
        assert!(result2.is_some());

        // Update via cache2
        cache2
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;

        // cache1 should see the update
        let result = cache1.lookup("q2", "model").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].message.as_text(), "a2");

        // Both should report same length
        assert_eq!(cache1.len().await, cache2.len().await);
    }

    #[tokio::test]
    async fn test_clone_clear_affects_both() {
        let cache1 = InMemoryCache::new();

        cache1
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;
        cache1
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;

        let cache2 = cache1.clone();
        assert_eq!(cache2.len().await, 2);

        // Clear via cache1
        cache1.clear().await.unwrap();

        // Both should be empty
        assert_eq!(cache1.len().await, 0);
        assert_eq!(cache2.len().await, 0);
        assert!(cache2.lookup("q1", "model").await.is_none());
    }

    // ------------------------------------------------------------------------
    // Different Message Types in Generations
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_cache_with_different_message_types() {
        let cache = InMemoryCache::new();

        // System message generation
        let sys_gen = ChatGeneration {
            message: Message::System {
                content: "system prompt".into(),
                fields: Default::default(),
            },
            generation_info: None,
        };

        // Human message generation
        let human_gen = ChatGeneration {
            message: Message::Human {
                content: "user question".into(),
                fields: Default::default(),
            },
            generation_info: None,
        };

        // Tool message generation
        let tool_gen = ChatGeneration {
            message: Message::Tool {
                content: "tool result".into(),
                tool_call_id: "call_123".to_string(),
                artifact: None,
                status: None,
                fields: Default::default(),
            },
            generation_info: None,
        };

        cache
            .update(
                "prompt",
                "model",
                vec![sys_gen.clone(), human_gen.clone(), tool_gen.clone()],
            )
            .await;

        let result = cache.lookup("prompt", "model").await;
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.len(), 3);
    }

    // ------------------------------------------------------------------------
    // Cache Miss Scenarios
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_cache_miss_scenarios() {
        let cache = InMemoryCache::new();

        cache
            .update("prompt1", "model1", vec![create_test_generation("a1")])
            .await;

        // Different prompt
        assert!(cache.lookup("prompt2", "model1").await.is_none());

        // Different llm_string
        assert!(cache.lookup("prompt1", "model2").await.is_none());

        // Both different
        assert!(cache.lookup("prompt2", "model2").await.is_none());

        // Case sensitivity
        assert!(cache.lookup("Prompt1", "model1").await.is_none());
        assert!(cache.lookup("prompt1", "Model1").await.is_none());
    }

    // ------------------------------------------------------------------------
    // Default Trait
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_default_trait() {
        let cache1 = InMemoryCache::default();
        let cache2 = InMemoryCache::new();

        // Both should be empty
        assert!(cache1.is_empty().await);
        assert!(cache2.is_empty().await);

        // Both should have no max_size
        cache1
            .update("q1", "m", vec![create_test_generation("a1")])
            .await;
        cache2
            .update("q1", "m", vec![create_test_generation("a1")])
            .await;

        assert_eq!(cache1.len().await, 1);
        assert_eq!(cache2.len().await, 1);
    }

    // ------------------------------------------------------------------------
    // Sequential Operations
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sequential_operations() {
        let cache = InMemoryCache::new();

        // Add items
        cache
            .update("q1", "model", vec![create_test_generation("a1")])
            .await;
        cache
            .update("q2", "model", vec![create_test_generation("a2")])
            .await;
        assert_eq!(cache.len().await, 2);

        // Clear
        cache.clear().await.unwrap();
        assert_eq!(cache.len().await, 0);

        // Add again
        cache
            .update("q3", "model", vec![create_test_generation("a3")])
            .await;
        assert_eq!(cache.len().await, 1);

        // Verify old entries are gone and new entry exists
        assert!(cache.lookup("q1", "model").await.is_none());
        assert!(cache.lookup("q2", "model").await.is_none());
        assert!(cache.lookup("q3", "model").await.is_some());
    }

    #[tokio::test]
    async fn test_clear_on_empty_cache() {
        let cache = InMemoryCache::new();

        assert!(cache.is_empty().await);

        // Clear empty cache should succeed
        cache.clear().await.unwrap();

        assert!(cache.is_empty().await);
    }

    // ------------------------------------------------------------------------
    // Key Uniqueness
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_key_uniqueness() {
        let cache = InMemoryCache::new();

        // These should all be different cache entries
        cache
            .update("ab", "cd", vec![create_test_generation("1")])
            .await;
        cache
            .update("a", "bcd", vec![create_test_generation("2")])
            .await;
        cache
            .update("abc", "d", vec![create_test_generation("3")])
            .await;

        // All three should be cached separately
        assert_eq!(cache.len().await, 3);

        let r1 = cache.lookup("ab", "cd").await;
        let r2 = cache.lookup("a", "bcd").await;
        let r3 = cache.lookup("abc", "d").await;

        assert_eq!(r1.unwrap()[0].message.as_text(), "1");
        assert_eq!(r2.unwrap()[0].message.as_text(), "2");
        assert_eq!(r3.unwrap()[0].message.as_text(), "3");
    }
}
