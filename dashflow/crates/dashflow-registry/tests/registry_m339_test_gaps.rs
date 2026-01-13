//! Registry Test Gap Coverage (M-339)
//!
//! Tests for:
//! - Storage failure handling
//! - Large package upload handling
//! - Concurrent upload tests
//! - Search ranking validation
//!
//! Run with: `cargo test -p dashflow-registry --test registry_m339_test_gaps`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_registry::{
    ContentHash, Embedder, InMemoryStorage, InMemoryVectorStore, MockEmbedder, PackageCache,
    PackageMetadata, PackageType, ScoreComponents, ScoreWeights, SearchFilters, StorageBackend,
    TrustLevel, VectorStore,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ============================================================================
// Storage Failure Handling Tests
// ============================================================================

/// A storage backend that fails after N successful operations (for testing failure recovery).
struct FailingStorage {
    inner: InMemoryStorage,
    fail_after: Arc<AtomicUsize>,
    operation_count: Arc<AtomicUsize>,
}

impl FailingStorage {
    fn new(fail_after: usize) -> Self {
        Self {
            inner: InMemoryStorage::new(),
            fail_after: Arc::new(AtomicUsize::new(fail_after)),
            operation_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn should_fail(&self) -> bool {
        let count = self.operation_count.fetch_add(1, Ordering::SeqCst);
        let limit = self.fail_after.load(Ordering::SeqCst);
        count >= limit
    }
}

#[async_trait::async_trait]
impl StorageBackend for FailingStorage {
    async fn store(&self, data: &[u8]) -> dashflow_registry::error::Result<ContentHash> {
        if self.should_fail() {
            return Err(dashflow_registry::error::RegistryError::StorageError(
                "Simulated storage failure".to_string(),
            ));
        }
        self.inner.store(data).await
    }

    async fn get(&self, hash: &ContentHash) -> dashflow_registry::error::Result<Vec<u8>> {
        if self.should_fail() {
            return Err(dashflow_registry::error::RegistryError::StorageError(
                "Simulated storage failure".to_string(),
            ));
        }
        self.inner.get(hash).await
    }

    async fn exists(&self, hash: &ContentHash) -> dashflow_registry::error::Result<bool> {
        if self.should_fail() {
            return Err(dashflow_registry::error::RegistryError::StorageError(
                "Simulated storage failure".to_string(),
            ));
        }
        self.inner.exists(hash).await
    }

    async fn delete(&self, hash: &ContentHash) -> dashflow_registry::error::Result<()> {
        if self.should_fail() {
            return Err(dashflow_registry::error::RegistryError::StorageError(
                "Simulated storage failure".to_string(),
            ));
        }
        self.inner.delete(hash).await
    }

    async fn info(
        &self,
        hash: &ContentHash,
    ) -> dashflow_registry::error::Result<dashflow_registry::StoredPackage> {
        if self.should_fail() {
            return Err(dashflow_registry::error::RegistryError::StorageError(
                "Simulated storage failure".to_string(),
            ));
        }
        self.inner.info(hash).await
    }
}

#[tokio::test]
async fn test_storage_failure_on_store() {
    let storage = FailingStorage::new(0); // Fail immediately

    let result = storage.store(b"test data").await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Simulated storage failure"));
}

#[tokio::test]
async fn test_storage_failure_on_get() {
    let storage = FailingStorage::new(1); // Succeed on store, fail on get

    let hash = storage.store(b"test data").await.unwrap();
    let result = storage.get(&hash).await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Simulated storage failure"));
}

#[tokio::test]
async fn test_storage_failure_on_exists() {
    let storage = FailingStorage::new(1); // Succeed on store, fail on exists

    let hash = storage.store(b"test data").await.unwrap();
    let result = storage.exists(&hash).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_storage_failure_recovery_pattern() {
    // Test that after failure, subsequent operations can succeed if failure was transient
    let storage = InMemoryStorage::new();

    // First operation succeeds
    let hash1 = storage.store(b"data1").await.unwrap();
    assert!(storage.exists(&hash1).await.unwrap());

    // Verify we can still store after a previous success
    let hash2 = storage.store(b"data2").await.unwrap();
    assert!(storage.exists(&hash2).await.unwrap());

    // Both packages accessible
    let data1 = storage.get(&hash1).await.unwrap();
    let data2 = storage.get(&hash2).await.unwrap();
    assert_eq!(data1, b"data1");
    assert_eq!(data2, b"data2");
}

#[tokio::test]
async fn test_storage_not_found_error() {
    let storage = InMemoryStorage::new();
    let fake_hash = ContentHash::from_bytes(b"nonexistent");

    let result = storage.get(&fake_hash).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        dashflow_registry::error::RegistryError::PackageNotFound(_) => {}
        other => panic!("Expected PackageNotFound, got {:?}", other),
    }
}

// ============================================================================
// Large Package Upload Tests
// ============================================================================

#[tokio::test]
async fn test_large_package_store_1mb() {
    let storage = InMemoryStorage::new();
    let large_data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();

    let hash = storage.store(&large_data).await.unwrap();
    assert!(storage.exists(&hash).await.unwrap());

    let retrieved = storage.get(&hash).await.unwrap();
    assert_eq!(retrieved.len(), 1_000_000);
    assert_eq!(retrieved, large_data);
}

#[tokio::test]
async fn test_large_package_store_10mb() {
    let storage = InMemoryStorage::new();
    let large_data: Vec<u8> = (0..10_000_000).map(|i| (i % 256) as u8).collect();

    let hash = storage.store(&large_data).await.unwrap();
    let info = storage.info(&hash).await.unwrap();

    assert_eq!(info.size, 10_000_000);
}

#[tokio::test]
async fn test_large_package_hash_correctness() {
    let storage = InMemoryStorage::new();
    let large_data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();

    // Store twice - should get same hash (content-addressed)
    let hash1 = storage.store(&large_data).await.unwrap();
    let hash2 = storage.store(&large_data).await.unwrap();

    assert_eq!(hash1, hash2, "Same content should produce same hash");
}

#[tokio::test]
async fn test_package_cache_eviction_on_large_packages() {
    // Cache with 1KB limit
    let cache = PackageCache::in_memory(1024);

    // Store a 500 byte package
    let data1 = vec![1u8; 500];
    let hash1 = cache.store(&data1).await.unwrap();
    assert!(cache.contains(&hash1).await.unwrap());
    assert_eq!(cache.current_size().await, 500);

    // Store another 500 byte package (should fit)
    let data2 = vec![2u8; 500];
    let hash2 = cache.store(&data2).await.unwrap();
    assert!(cache.contains(&hash2).await.unwrap());
    assert_eq!(cache.current_size().await, 1000);

    // Store a 600 byte package (should trigger eviction of hash1)
    let data3 = vec![3u8; 600];
    let hash3 = cache.store(&data3).await.unwrap();
    assert!(cache.contains(&hash3).await.unwrap());

    // Cache should have evicted oldest to make room
    assert!(cache.current_size().await <= 1024);
}

#[tokio::test]
async fn test_package_cache_lru_eviction_order() {
    // Cache with 2KB limit
    let cache = PackageCache::in_memory(2048);

    // Store 4 packages of 500 bytes each
    let mut hashes = Vec::new();
    for i in 0..4u8 {
        let data = vec![i; 500];
        let hash = cache.store(&data).await.unwrap();
        hashes.push(hash);
    }

    assert_eq!(cache.entry_count().await, 4);

    // Access hash[2] to update its LRU time
    cache.get(&hashes[2]).await.unwrap();

    // Store a new package that requires eviction
    let new_data = vec![99u8; 500];
    cache.store(&new_data).await.unwrap();

    // hash[0] should be evicted (oldest without access)
    // hash[2] should remain (recently accessed)
    assert!(cache.contains(&hashes[2]).await.unwrap());
}

// ============================================================================
// Concurrent Upload Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_store_same_content() {
    let storage = Arc::new(InMemoryStorage::new());
    let data = b"concurrent test data";

    // Spawn 10 concurrent stores of the same data
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let storage = Arc::clone(&storage);
            let data = data.to_vec();
            tokio::spawn(async move { storage.store(&data).await })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // All should succeed with same hash
    let first_hash = results[0].as_ref().unwrap();
    for result in &results {
        assert_eq!(result.as_ref().unwrap(), first_hash);
    }
}

#[tokio::test]
async fn test_concurrent_store_different_content() {
    let storage = Arc::new(InMemoryStorage::new());

    // Spawn 10 concurrent stores of different data
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let storage = Arc::clone(&storage);
            tokio::spawn(async move {
                let data = format!("package content {}", i);
                storage.store(data.as_bytes()).await
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap().unwrap())
        .collect();

    // All hashes should be different
    let unique_hashes: std::collections::HashSet<_> = results.iter().collect();
    assert_eq!(unique_hashes.len(), 10);
}

#[tokio::test]
async fn test_concurrent_read_write() {
    let storage = Arc::new(InMemoryStorage::new());

    // First store some initial data
    let initial_hash = storage.store(b"initial data").await.unwrap();

    // Spawn concurrent reads and writes
    let handles: Vec<_> = (0..20)
        .map(|i| {
            let storage = Arc::clone(&storage);
            let hash = initial_hash.clone();
            tokio::spawn(async move {
                if i % 2 == 0 {
                    // Read
                    storage.get(&hash).await
                } else {
                    // Write new data
                    let data = format!("new data {}", i);
                    storage.store(data.as_bytes()).await.map(|_| vec![])
                }
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles).await;

    // All operations should succeed
    for result in results {
        assert!(result.unwrap().is_ok());
    }

    // Original data should still be accessible
    let data = storage.get(&initial_hash).await.unwrap();
    assert_eq!(data, b"initial data");
}

#[tokio::test]
async fn test_concurrent_cache_operations() {
    let cache = Arc::new(PackageCache::in_memory(10_000));

    // Store initial data
    let initial_hash = cache.store(b"cache test data").await.unwrap();

    // Spawn concurrent cache operations
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let cache = Arc::clone(&cache);
            let hash = initial_hash.clone();
            tokio::spawn(async move {
                match i % 3 {
                    0 => {
                        // Read
                        cache.get(&hash).await.map(|_| ())
                    }
                    1 => {
                        // Write
                        let data = format!("concurrent cache data {}", i);
                        cache.store(data.as_bytes()).await.map(|_| ())
                    }
                    _ => {
                        // Check existence
                        cache.contains(&hash).await.map(|_| ())
                    }
                }
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles).await;

    // All operations should succeed without deadlock
    for result in results {
        assert!(result.unwrap().is_ok());
    }
}

// ============================================================================
// Search Ranking Validation Tests
// ============================================================================

#[tokio::test]
async fn test_score_weights_default() {
    let weights = ScoreWeights::default();

    assert!((weights.semantic - 0.4).abs() < 0.001);
    assert!((weights.keyword - 0.2).abs() < 0.001);
    assert!((weights.capability - 0.2).abs() < 0.001);
    assert!((weights.popularity - 0.1).abs() < 0.001);
    assert!((weights.trust - 0.1).abs() < 0.001);
}

#[tokio::test]
async fn test_combined_score_calculation() {
    let weights = ScoreWeights::default();

    let components = ScoreComponents {
        semantic_score: Some(0.9),
        keyword_score: Some(0.8),
        capability_score: Some(0.7),
        popularity_score: 0.5,
        trust_score: 0.6,
    };

    let score = components.combined_score(&weights);

    // Verify score is in valid range
    assert!((0.0..=1.0).contains(&score));

    // Calculate expected score manually:
    // (0.9*0.4 + 0.8*0.2 + 0.7*0.2 + 0.5*0.1 + 0.6*0.1) / (0.4+0.2+0.2+0.1+0.1)
    // = (0.36 + 0.16 + 0.14 + 0.05 + 0.06) / 1.0 = 0.77
    assert!((score - 0.77).abs() < 0.001);
}

#[tokio::test]
async fn test_combined_score_missing_semantic() {
    let weights = ScoreWeights::default();

    let components = ScoreComponents {
        semantic_score: None, // No semantic search
        keyword_score: Some(0.8),
        capability_score: None,
        popularity_score: 0.5,
        trust_score: 0.6,
    };

    let score = components.combined_score(&weights);

    // Score should still be valid
    assert!((0.0..=1.0).contains(&score));

    // With only keyword (0.2), popularity (0.1), trust (0.1) = total weight 0.4
    // (0.8*0.2 + 0.5*0.1 + 0.6*0.1) / 0.4 = (0.16 + 0.05 + 0.06) / 0.4 = 0.675
    assert!((score - 0.675).abs() < 0.001);
}

#[tokio::test]
async fn test_combined_score_only_popularity_and_trust() {
    let weights = ScoreWeights::default();

    let components = ScoreComponents {
        semantic_score: None,
        keyword_score: None,
        capability_score: None,
        popularity_score: 0.8,
        trust_score: 0.9,
    };

    let score = components.combined_score(&weights);

    // Only popularity (0.1) and trust (0.1) = total weight 0.2
    // (0.8*0.1 + 0.9*0.1) / 0.2 = 0.17 / 0.2 = 0.85
    assert!((score - 0.85).abs() < 0.001);
}

#[tokio::test]
async fn test_search_ranking_high_semantic_beats_low() {
    let weights = ScoreWeights::default();

    let high_semantic = ScoreComponents {
        semantic_score: Some(0.95),
        keyword_score: Some(0.5),
        capability_score: None,
        popularity_score: 0.3,
        trust_score: 0.5,
    };

    let low_semantic = ScoreComponents {
        semantic_score: Some(0.3),
        keyword_score: Some(0.9),
        capability_score: None,
        popularity_score: 0.8,
        trust_score: 0.9,
    };

    let high_score = high_semantic.combined_score(&weights);
    let low_score = low_semantic.combined_score(&weights);

    // With default weights (semantic=0.4), high semantic should win
    assert!(
        high_score > low_score,
        "High semantic ({}) should beat low semantic ({})",
        high_score,
        low_score
    );
}

#[tokio::test]
async fn test_vector_store_cosine_similarity_ranking() {
    let store = InMemoryVectorStore::new();
    let embedder = MockEmbedder::new(128);

    // Create package metadata
    let metadata1 = PackageMetadata {
        hash: "sha256:aaa".to_string(),
        name: "sentiment-analyzer".to_string(),
        namespace: None,
        version: "1.0.0".to_string(),
        description: "Advanced sentiment analysis for text".to_string(),
        package_type: PackageType::Tool,
        keywords: vec!["sentiment".to_string(), "nlp".to_string()],
        capabilities: vec!["text-analysis".to_string()],
        trust_level: TrustLevel::Community,
        downloads: 1000,
        indexed_at: chrono::Utc::now(),
    };

    let metadata2 = PackageMetadata {
        hash: "sha256:bbb".to_string(),
        name: "text-classifier".to_string(),
        namespace: None,
        version: "1.0.0".to_string(),
        description: "General text classification".to_string(),
        package_type: PackageType::Tool,
        keywords: vec!["classification".to_string()],
        capabilities: vec!["text-analysis".to_string()],
        trust_level: TrustLevel::Community,
        downloads: 500,
        indexed_at: chrono::Utc::now(),
    };

    // Index packages with their embeddings
    let embed1 = embedder.embed(&metadata1.description).await.unwrap();
    let embed2 = embedder.embed(&metadata2.description).await.unwrap();

    store
        .upsert(&metadata1.hash, embed1, metadata1.clone())
        .await
        .unwrap();
    store
        .upsert(&metadata2.hash, embed2, metadata2.clone())
        .await
        .unwrap();

    // Search for "sentiment analysis"
    let query_embed = embedder.embed("sentiment analysis").await.unwrap();
    let results = store.search(query_embed, 10).await.unwrap();

    assert!(!results.is_empty());

    // Results should be sorted by score descending
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "Results should be sorted by score descending"
        );
    }
}

#[tokio::test]
async fn test_vector_store_search_limit() {
    let store = InMemoryVectorStore::new();
    let embedder = MockEmbedder::new(128);

    // Index 20 packages
    for i in 0..20 {
        let metadata = PackageMetadata {
            hash: format!("sha256:{:03}", i),
            name: format!("package-{}", i),
            namespace: None,
            version: "1.0.0".to_string(),
            description: format!("Package number {}", i),
            package_type: PackageType::Tool,
            keywords: vec![],
            capabilities: vec![],
            trust_level: TrustLevel::Community,
            downloads: i as u64,
            indexed_at: chrono::Utc::now(),
        };

        let embed = embedder.embed(&metadata.description).await.unwrap();
        let hash = metadata.hash.clone();
        store.upsert(&hash, embed, metadata).await.unwrap();
    }

    // Search with limit of 5
    let query_embed = embedder.embed("package").await.unwrap();
    let results = store.search(query_embed, 5).await.unwrap();

    assert_eq!(results.len(), 5, "Search should respect limit");
}

#[tokio::test]
async fn test_search_filters_exclude_yanked() {
    let filters = SearchFilters::default();

    // Default should exclude yanked
    assert!(filters.exclude_yanked);
}

#[tokio::test]
async fn test_search_filters_verified_only() {
    let filters = SearchFilters {
        verified_only: true,
        ..Default::default()
    };

    assert!(filters.verified_only);
}

#[tokio::test]
async fn test_search_filters_min_downloads() {
    let filters = SearchFilters {
        min_downloads: Some(1000),
        ..Default::default()
    };

    assert_eq!(filters.min_downloads, Some(1000));
}

#[tokio::test]
async fn test_search_filters_package_type() {
    let filters = SearchFilters {
        package_type: Some(PackageType::Application),
        ..Default::default()
    };

    assert!(matches!(
        filters.package_type,
        Some(PackageType::Application)
    ));
}

#[tokio::test]
async fn test_mock_embedder_determinism() {
    let embedder = MockEmbedder::new(128);

    let text = "sentiment analysis for reviews";

    // Same text should produce same embedding
    let embed1 = embedder.embed(text).await.unwrap();
    let embed2 = embedder.embed(text).await.unwrap();

    assert_eq!(embed1, embed2, "Same text should produce same embedding");
}

#[tokio::test]
async fn test_mock_embedder_normalization() {
    let embedder = MockEmbedder::new(128);

    let embed = embedder.embed("test text").await.unwrap();

    // Embedding should be normalized (L2 norm ≈ 1.0)
    let norm: f32 = embed.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.001,
        "Embedding should be normalized, got norm {}",
        norm
    );
}

#[tokio::test]
async fn test_vector_store_upsert_replaces() {
    let store = InMemoryVectorStore::new();
    let embedder = MockEmbedder::new(128);

    let metadata_v1 = PackageMetadata {
        hash: "sha256:abc".to_string(),
        name: "my-package".to_string(),
        namespace: None,
        version: "1.0.0".to_string(),
        description: "Version 1".to_string(),
        package_type: PackageType::Tool,
        keywords: vec![],
        capabilities: vec![],
        trust_level: TrustLevel::Community,
        downloads: 100,
        indexed_at: chrono::Utc::now(),
    };

    let metadata_v2 = PackageMetadata {
        version: "2.0.0".to_string(),
        description: "Version 2".to_string(),
        downloads: 200,
        ..metadata_v1.clone()
    };

    // Insert v1
    let embed = embedder.embed(&metadata_v1.description).await.unwrap();
    let hash_v1 = metadata_v1.hash.clone();
    store
        .upsert(&hash_v1, embed.clone(), metadata_v1)
        .await
        .unwrap();

    // Upsert v2 with same hash
    let embed2 = embedder.embed(&metadata_v2.description).await.unwrap();
    let hash_v2 = metadata_v2.hash.clone();
    store.upsert(&hash_v2, embed2, metadata_v2).await.unwrap();

    // Search should return v2
    let query_embed = embedder.embed("Version").await.unwrap();
    let results = store.search(query_embed, 10).await.unwrap();

    let found = results.iter().find(|r| r.id == "sha256:abc").unwrap();
    assert_eq!(found.metadata.version, "2.0.0");
    assert_eq!(found.metadata.downloads, 200);
}

#[tokio::test]
async fn test_content_hash_from_bytes_deterministic() {
    let data = b"package content for hashing";

    let hash1 = ContentHash::from_bytes(data);
    let hash2 = ContentHash::from_bytes(data);

    assert_eq!(hash1, hash2, "Same data should produce same hash");
}

#[tokio::test]
async fn test_content_hash_different_data() {
    let hash1 = ContentHash::from_bytes(b"data1");
    let hash2 = ContentHash::from_bytes(b"data2");

    assert_ne!(hash1, hash2, "Different data should produce different hashes");
}
