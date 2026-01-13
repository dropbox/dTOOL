// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Performance Module for Self-Improvement System.
//!
//! This module consolidates performance-related functionality:
//! - **Cache**: In-memory LRU cache for execution traces and metrics
//! - **Lazy Loading**: Memory-efficient iterators for large datasets
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::self_improvement::performance::{
//!     // Cache
//!     MetricsCache, DEFAULT_CACHE_CAPACITY, DEFAULT_RECENT_WINDOW,
//!     // Lazy Loading
//!     LazyPlanIterator, LazyTraceIterator, LazyStorageExt,
//! };
//!
//! // Create a metrics cache
//! let mut cache = MetricsCache::new(1000);
//! cache.put("trace_001.json", trace);
//!
//! // Use lazy iterators
//! let traces = LazyTraceIterator::default_traces();
//! for trace in traces.take(10) {
//!     // Process trace without loading all at once
//! }
//! ```

// =============================================================================
// Cache Module (from cache.rs)
// =============================================================================

pub mod cache {
    //! In-Memory Metrics Cache for Self-Improvement Daemon.
    //!
    //! Avoid repeated disk reads by caching traces and metrics.

    use crate::introspection::ExecutionTrace;
    use lru::LruCache;
    use std::collections::VecDeque;
    use std::num::NonZeroUsize;

    /// Default maximum number of traces to cache.
    pub const DEFAULT_CACHE_CAPACITY: usize = 1000;

    /// Default number of recent traces to keep for quick aggregation.
    pub const DEFAULT_RECENT_WINDOW: usize = 100;

    /// In-memory cache for execution traces and metrics.
    #[derive(Debug)]
    pub struct MetricsCache {
        traces: LruCache<String, ExecutionTrace>,
        recent_traces: VecDeque<ExecutionTrace>,
        recent_window_size: usize,
        hits: u64,
        misses: u64,
    }

    impl MetricsCache {
        /// Create a new cache with specified capacity.
        #[must_use]
        pub fn new(capacity: usize) -> Self {
            let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN);
            Self {
                traces: LruCache::new(cap),
                recent_traces: VecDeque::with_capacity(DEFAULT_RECENT_WINDOW),
                recent_window_size: DEFAULT_RECENT_WINDOW,
                hits: 0,
                misses: 0,
            }
        }

        /// Create a cache with custom capacity and recent window size.
        #[must_use]
        pub fn with_recent_window(capacity: usize, recent_window: usize) -> Self {
            let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN);
            Self {
                traces: LruCache::new(cap),
                recent_traces: VecDeque::with_capacity(recent_window),
                recent_window_size: recent_window,
                hits: 0,
                misses: 0,
            }
        }

        /// Get a trace from the cache by file path.
        ///
        /// Uses a single LRU lookup for efficiency (M-969 fix: was double-lookup).
        pub fn get(&mut self, file_path: &str) -> Option<&ExecutionTrace> {
            // Single lookup: get() returns Option and updates LRU ordering
            match self.traces.get(file_path) {
                Some(trace) => {
                    self.hits += 1;
                    Some(trace)
                }
                None => {
                    self.misses += 1;
                    None
                }
            }
        }

        /// Check if a trace is cached without updating LRU ordering.
        #[must_use]
        pub fn contains(&self, file_path: &str) -> bool {
            self.traces.contains(file_path)
        }

        /// Store a trace in the cache.
        pub fn put(&mut self, file_path: impl Into<String>, trace: ExecutionTrace) {
            let path = file_path.into();

            self.recent_traces.push_back(trace.clone());
            if self.recent_traces.len() > self.recent_window_size {
                self.recent_traces.pop_front();
            }

            self.traces.put(path, trace);
        }

        /// Get the most recent traces for aggregation.
        #[must_use]
        pub fn recent_traces(&self, limit: usize) -> Vec<&ExecutionTrace> {
            let skip = self.recent_traces.len().saturating_sub(limit);
            self.recent_traces.iter().skip(skip).collect()
        }

        /// Get all recent traces (up to window size).
        #[must_use]
        pub fn all_recent_traces(&self) -> Vec<&ExecutionTrace> {
            self.recent_traces.iter().collect()
        }

        /// Get the number of cached traces.
        #[must_use]
        pub fn len(&self) -> usize {
            self.traces.len()
        }

        /// Check if cache is empty.
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.traces.is_empty()
        }

        /// Get the number of recent traces in the window.
        #[must_use]
        pub fn recent_count(&self) -> usize {
            self.recent_traces.len()
        }

        /// Get cache hit count.
        #[must_use]
        pub fn hits(&self) -> u64 {
            self.hits
        }

        /// Get cache miss count.
        #[must_use]
        pub fn misses(&self) -> u64 {
            self.misses
        }

        /// Get cache hit rate as a percentage.
        #[must_use]
        pub fn hit_rate(&self) -> f64 {
            let total = self.hits + self.misses;
            if total == 0 {
                0.0
            } else {
                (self.hits as f64 / total as f64) * 100.0
            }
        }

        /// Clear all cached data.
        pub fn clear(&mut self) {
            self.traces.clear();
            self.recent_traces.clear();
            self.hits = 0;
            self.misses = 0;
        }

        /// Remove stale entries older than a given timestamp.
        pub fn evict_older_than(&mut self, cutoff_timestamp: &str) -> usize {
            let mut to_remove = Vec::new();

            for (path, trace) in self.traces.iter() {
                if let Some(ref ended) = trace.ended_at {
                    if ended.as_str() < cutoff_timestamp {
                        to_remove.push(path.clone());
                    }
                }
            }

            let removed_count = to_remove.len();

            for path in to_remove {
                self.traces.pop(&path);
            }

            self.recent_traces.retain(|t| {
                t.ended_at
                    .as_ref()
                    .map(|e| e.as_str() >= cutoff_timestamp)
                    .unwrap_or(true)
            });

            removed_count
        }
    }

    impl Default for MetricsCache {
        fn default() -> Self {
            Self::new(DEFAULT_CACHE_CAPACITY)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn create_test_trace(thread_id: &str) -> ExecutionTrace {
            ExecutionTrace::builder().thread_id(thread_id).build()
        }

        fn create_trace_with_timestamp(thread_id: &str, ended_at: &str) -> ExecutionTrace {
            let mut trace = ExecutionTrace::builder().thread_id(thread_id).build();
            trace.ended_at = Some(ended_at.to_string());
            trace
        }

        #[test]
        fn test_cache_put_and_get() {
            let mut cache = MetricsCache::new(10);

            let trace = create_test_trace("t1");
            cache.put("trace_001.json", trace);

            assert!(cache.contains("trace_001.json"));
            assert!(!cache.contains("trace_002.json"));

            let cached = cache.get("trace_001.json");
            assert!(cached.is_some());
            assert_eq!(cached.unwrap().thread_id, Some("t1".to_string()));
        }

        #[test]
        fn test_cache_hit_miss_stats() {
            let mut cache = MetricsCache::new(10);

            let trace = create_test_trace("t1");
            cache.put("trace_001.json", trace);

            let _ = cache.get("trace_001.json");
            assert_eq!(cache.hits(), 1);
            assert_eq!(cache.misses(), 0);

            let _ = cache.get("trace_002.json");
            assert_eq!(cache.hits(), 1);
            assert_eq!(cache.misses(), 1);

            let _ = cache.get("trace_001.json");
            assert_eq!(cache.hits(), 2);
            assert_eq!(cache.misses(), 1);

            assert!((cache.hit_rate() - 66.67).abs() < 1.0);
        }

        #[test]
        fn test_recent_traces_window() {
            let mut cache = MetricsCache::with_recent_window(100, 5);

            for i in 0..7 {
                let trace = create_test_trace(&format!("t{}", i));
                cache.put(format!("trace_{}.json", i), trace);
            }

            assert_eq!(cache.recent_count(), 5);

            let recent = cache.recent_traces(5);
            assert_eq!(recent.len(), 5);
            assert_eq!(recent[0].thread_id, Some("t2".to_string()));
            assert_eq!(recent[4].thread_id, Some("t6".to_string()));
        }

        #[test]
        fn test_lru_eviction() {
            let mut cache = MetricsCache::new(3);

            cache.put("a.json", create_test_trace("a"));
            cache.put("b.json", create_test_trace("b"));
            cache.put("c.json", create_test_trace("c"));

            assert_eq!(cache.len(), 3);

            let _ = cache.get("a.json");

            cache.put("d.json", create_test_trace("d"));

            assert_eq!(cache.len(), 3);
            assert!(cache.contains("a.json"));
            assert!(!cache.contains("b.json"));
            assert!(cache.contains("c.json"));
            assert!(cache.contains("d.json"));
        }

        #[test]
        fn test_evict_older_than() {
            let mut cache = MetricsCache::new(10);

            cache.put(
                "old1.json",
                create_trace_with_timestamp("old1", "2025-01-01T00:00:00Z"),
            );
            cache.put(
                "old2.json",
                create_trace_with_timestamp("old2", "2025-01-02T00:00:00Z"),
            );
            cache.put(
                "new1.json",
                create_trace_with_timestamp("new1", "2025-06-01T00:00:00Z"),
            );
            cache.put(
                "new2.json",
                create_trace_with_timestamp("new2", "2025-06-02T00:00:00Z"),
            );

            assert_eq!(cache.len(), 4);

            let removed = cache.evict_older_than("2025-03-01T00:00:00Z");

            assert_eq!(removed, 2);
            assert_eq!(cache.len(), 2);
            assert!(!cache.contains("old1.json"));
            assert!(!cache.contains("old2.json"));
            assert!(cache.contains("new1.json"));
            assert!(cache.contains("new2.json"));
        }

        #[test]
        fn test_clear() {
            let mut cache = MetricsCache::new(10);

            cache.put("a.json", create_test_trace("a"));
            cache.put("b.json", create_test_trace("b"));
            let _ = cache.get("a.json");
            let _ = cache.get("missing.json");

            assert_eq!(cache.len(), 2);
            assert!(cache.hits() > 0);
            assert!(cache.misses() > 0);

            cache.clear();

            assert_eq!(cache.len(), 0);
            assert_eq!(cache.recent_count(), 0);
            assert_eq!(cache.hits(), 0);
            assert_eq!(cache.misses(), 0);
        }

        #[test]
        fn test_default_capacity() {
            let cache = MetricsCache::default();
            assert_eq!(cache.len(), 0);
        }
    }
}

// =============================================================================
// Lazy Loading Module (from lazy_loading.rs)
// =============================================================================

pub mod lazy_loading {
    //! Lazy Loading Utilities for Self-Improvement.
    //!
    //! Provides lazy loading iterators to avoid loading all data into memory
    //! at once.

    use crate::introspection::ExecutionTrace;
    use crate::self_improvement::storage::IntrospectionStorage;
    use crate::self_improvement::types::{ExecutionPlan, Hypothesis};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    /// Lazy iterator over execution plan files.
    pub struct LazyPlanIterator {
        paths: std::vec::IntoIter<PathBuf>,
        storage: IntrospectionStorage,
    }

    impl LazyPlanIterator {
        /// Create a new lazy iterator over plans in the given directory.
        pub fn new(dir: &Path, storage: IntrospectionStorage) -> Self {
            let paths = Self::collect_json_paths(dir);
            Self {
                paths: paths.into_iter(),
                storage,
            }
        }

        /// Create an iterator over all plan directories.
        pub fn all_plans(storage: &IntrospectionStorage) -> Self {
            let mut paths = Vec::new();
            let plans_dir = storage.plans_dir();

            for subdir in ["pending", "approved", "implemented", "failed"] {
                let dir = plans_dir.join(subdir);
                if dir.exists() {
                    paths.extend(Self::collect_json_paths(&dir));
                }
            }

            Self {
                paths: paths.into_iter(),
                storage: storage.clone(),
            }
        }

        fn collect_json_paths(dir: &Path) -> Vec<PathBuf> {
            if !dir.exists() {
                return Vec::new();
            }

            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .map(|e| e.path())
                .collect()
        }
    }

    impl Iterator for LazyPlanIterator {
        type Item = io::Result<ExecutionPlan>;

        fn next(&mut self) -> Option<Self::Item> {
            self.paths.next().map(|path| {
                let contents = fs::read_to_string(&path)?;
                self.storage
                    .parse_plan_from_json(&contents)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            })
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.paths.size_hint()
        }
    }

    /// Lazy iterator over hypothesis files.
    pub struct LazyHypothesisIterator {
        paths: std::vec::IntoIter<PathBuf>,
        storage: IntrospectionStorage,
    }

    impl LazyHypothesisIterator {
        /// Create a new lazy iterator over hypotheses in the given directory.
        pub fn new(dir: &Path, storage: IntrospectionStorage) -> Self {
            let paths = Self::collect_json_paths(dir);
            Self {
                paths: paths.into_iter(),
                storage,
            }
        }

        /// Create an iterator over all hypothesis directories.
        pub fn all_hypotheses(storage: &IntrospectionStorage) -> Self {
            let mut paths = Vec::new();
            let hypotheses_dir = storage.hypotheses_dir();

            for subdir in ["active", "evaluated"] {
                let dir = hypotheses_dir.join(subdir);
                if dir.exists() {
                    paths.extend(Self::collect_json_paths(&dir));
                }
            }

            Self {
                paths: paths.into_iter(),
                storage: storage.clone(),
            }
        }

        fn collect_json_paths(dir: &Path) -> Vec<PathBuf> {
            if !dir.exists() {
                return Vec::new();
            }

            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .map(|e| e.path())
                .collect()
        }
    }

    impl Iterator for LazyHypothesisIterator {
        type Item = io::Result<Hypothesis>;

        fn next(&mut self) -> Option<Self::Item> {
            self.paths.next().map(|path| {
                let contents = fs::read_to_string(&path)?;
                self.storage
                    .parse_hypothesis_from_json(&contents)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            })
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.paths.size_hint()
        }
    }

    /// Lazy iterator over execution trace files.
    pub struct LazyTraceIterator {
        paths: std::vec::IntoIter<PathBuf>,
    }

    impl LazyTraceIterator {
        /// Create a new lazy iterator over traces in the given directory.
        pub fn new(dir: &Path) -> Self {
            let paths = Self::collect_trace_paths(dir);
            Self {
                paths: paths.into_iter(),
            }
        }

        /// Create from the default traces directory.
        pub fn default_traces() -> Self {
            let traces_dir = PathBuf::from(".dashflow/traces");
            Self::new(&traces_dir)
        }

        fn collect_trace_paths(dir: &Path) -> Vec<PathBuf> {
            if !dir.exists() {
                return Vec::new();
            }

            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| {
                    let path = e.path();
                    path.extension()
                        .is_some_and(|ext| ext == "json" || ext == "gz")
                })
                .map(|e| e.path())
                .collect()
        }

        /// Get the total number of trace files (without loading them).
        pub fn count_available(&self) -> usize {
            self.paths.len()
        }
    }

    /// Maximum allowed decompressed trace size: 100 MB (protection against gzip bombs)
    const MAX_DECOMPRESSED_SIZE: u64 = 100 * 1024 * 1024;

    impl Iterator for LazyTraceIterator {
        type Item = io::Result<ExecutionTrace>;

        fn next(&mut self) -> Option<Self::Item> {
            self.paths.next().map(|path| {
                let contents = if path.extension().is_some_and(|e| e == "gz") {
                    use flate2::read::GzDecoder;
                    use std::io::Read;

                    let file = fs::File::open(&path)?;
                    let decoder = GzDecoder::new(file);
                    // Limit decompressed size to prevent gzip bombs
                    let mut limited = decoder.take(MAX_DECOMPRESSED_SIZE + 1);
                    let mut contents = String::new();
                    limited.read_to_string(&mut contents)?;

                    if contents.len() as u64 > MAX_DECOMPRESSED_SIZE {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Decompressed trace size exceeds limit of {} bytes",
                                MAX_DECOMPRESSED_SIZE
                            ),
                        ));
                    }
                    contents
                } else {
                    fs::read_to_string(&path)?
                };

                serde_json::from_str(&contents)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            })
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.paths.size_hint()
        }
    }

    /// Lazy file reference that only loads content on demand.
    pub struct LazyFile<T> {
        path: PathBuf,
        _phantom: std::marker::PhantomData<T>,
    }

    impl<T> LazyFile<T> {
        /// Create a new lazy file reference.
        pub fn new(path: PathBuf) -> Self {
            Self {
                path,
                _phantom: std::marker::PhantomData,
            }
        }

        /// Get the path to the file.
        pub fn path(&self) -> &Path {
            &self.path
        }

        /// Check if the file exists.
        pub fn exists(&self) -> bool {
            self.path.exists()
        }

        /// Get file metadata without loading content.
        pub fn metadata(&self) -> io::Result<fs::Metadata> {
            fs::metadata(&self.path)
        }
    }

    impl<T: serde::de::DeserializeOwned> LazyFile<T> {
        /// Load and parse the file content.
        pub fn load(&self) -> io::Result<T> {
            let contents = fs::read_to_string(&self.path)?;
            serde_json::from_str(&contents)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
    }

    /// Configuration for lazy loading behavior.
    #[derive(Debug, Clone)]
    pub struct LazyLoadConfig {
        /// Number of items to buffer for batch processing.
        pub buffer_size: usize,
        /// Whether to skip items that fail to parse instead of failing.
        pub skip_errors: bool,
    }

    impl Default for LazyLoadConfig {
        fn default() -> Self {
            Self {
                buffer_size: 100,
                skip_errors: true,
            }
        }
    }

    /// Extension trait for lazy loading from IntrospectionStorage.
    pub trait LazyStorageExt {
        /// Create a lazy iterator over all execution plans.
        fn lazy_plans(&self) -> LazyPlanIterator;
        /// Create a lazy iterator over all hypotheses.
        fn lazy_hypotheses(&self) -> LazyHypothesisIterator;
        /// Create a lazy iterator over pending plans only.
        fn lazy_pending_plans(&self) -> LazyPlanIterator;
    }

    impl LazyStorageExt for IntrospectionStorage {
        fn lazy_plans(&self) -> LazyPlanIterator {
            LazyPlanIterator::all_plans(self)
        }

        fn lazy_hypotheses(&self) -> LazyHypothesisIterator {
            LazyHypothesisIterator::all_hypotheses(self)
        }

        fn lazy_pending_plans(&self) -> LazyPlanIterator {
            let dir = self.plans_dir().join("pending");
            LazyPlanIterator::new(&dir, self.clone())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        #[test]
        fn test_lazy_trace_iterator_empty_dir() {
            let dir = tempdir().unwrap();
            let iter = LazyTraceIterator::new(dir.path());
            assert_eq!(iter.count(), 0);
        }

        #[test]
        fn test_lazy_trace_iterator_nonexistent_dir() {
            let iter = LazyTraceIterator::new(Path::new("/nonexistent/path"));
            assert_eq!(iter.count(), 0);
        }

        #[test]
        fn test_lazy_file_exists() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("test.json");
            fs::write(&path, "{}").unwrap();

            let lazy: LazyFile<serde_json::Value> = LazyFile::new(path.clone());
            assert!(lazy.exists());
            assert_eq!(lazy.path(), path);
        }

        #[test]
        fn test_lazy_file_load() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("test.json");
            fs::write(&path, r#"{"key": "value"}"#).unwrap();

            let lazy: LazyFile<serde_json::Value> = LazyFile::new(path);
            let value = lazy.load().unwrap();
            assert_eq!(value["key"], "value");
        }

        #[test]
        fn test_lazy_load_config_default() {
            let config = LazyLoadConfig::default();
            assert_eq!(config.buffer_size, 100);
            assert!(config.skip_errors);
        }
    }
}

// =============================================================================
// Re-exports for convenience
// =============================================================================

pub use cache::*;
pub use lazy_loading::*;
