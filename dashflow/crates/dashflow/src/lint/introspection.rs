//! Introspection integration for the platform usage linter.
//!
//! This module provides dynamic discovery of DashFlow platform types
//! that can be suggested as alternatives to reimplemented code.
//!
//! ## Caching
//!
//! The type index can be cached to `.dashflow/index/types.json` for faster startup.
//! Use `TypeIndex::save()` to persist and `TypeIndex::load()` to restore.

use super::semantic::SemanticIndex;
use dashflow_module_discovery::{discover_all_types, TypeInfo, TypeKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Cached type index for fast lookup
static TYPE_INDEX: OnceLock<TypeIndex> = OnceLock::new();

/// Serializable cache format for the type index
#[derive(Debug, Serialize, Deserialize)]
pub struct TypeIndexCache {
    /// Version of the cache format (for forwards compatibility)
    pub version: u32,
    /// All discovered types
    pub types: Vec<TypeInfo>,
    /// Workspace root when cache was created
    pub workspace_root: PathBuf,
    /// Timestamp when cache was created
    pub created_at: String,
    /// Semantic index for similarity search (added in v2)
    #[serde(default)]
    pub semantic_index: Option<SemanticIndex>,
}

impl TypeIndexCache {
    /// Current cache version (v2 adds semantic index)
    pub const CURRENT_VERSION: u32 = 2;

    /// Default cache path relative to workspace root
    pub const CACHE_PATH: &'static str = ".dashflow/index/types.json";

    /// Check if the cache is stale (older than any source file)
    ///
    /// Returns None if unable to determine staleness, Some(true) if stale,
    /// Some(false) if fresh.
    pub fn is_stale(&self, workspace_root: &Path) -> Option<bool> {
        // Parse cache creation time
        let cache_time = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .ok()?
            .with_timezone(&chrono::Utc);

        // Check a few key source directories for modification times
        let source_dirs = [
            "crates/dashflow/src",
            "crates/dashflow-opensearch/src",
            "crates/dashflow-openai/src",
            "crates/dashflow-module-discovery/src",
        ];

        for dir in &source_dirs {
            let dir_path = workspace_root.join(dir);
            if dir_path.exists() {
                if let Some(newest) = Self::newest_modification(&dir_path) {
                    if newest > cache_time {
                        return Some(true); // Cache is stale
                    }
                }
            }
        }

        Some(false) // Cache is fresh
    }

    /// Find the newest modification time in a directory (recursive)
    fn newest_modification(dir: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
        let mut newest: Option<std::time::SystemTime> = None;

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();

                if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                    // Get mtime if available
                    if let Some(modified) = path.metadata().ok().and_then(|m| m.modified().ok()) {
                        newest = Some(match newest {
                            Some(n) if modified > n => modified,
                            Some(n) => n,
                            None => modified,
                        });
                    }
                } else if path.is_dir()
                    && !path
                        .file_name()
                        .map_or(true, |n| n.to_string_lossy().starts_with('.'))
                {
                    if let Some(dir_newest) = Self::newest_modification(&path) {
                        let dir_sys_time = std::time::SystemTime::UNIX_EPOCH
                            + std::time::Duration::from_secs(dir_newest.timestamp() as u64);
                        newest = Some(match newest {
                            Some(n) if dir_sys_time > n => dir_sys_time,
                            Some(n) => n,
                            None => dir_sys_time,
                        });
                    }
                }
            }
        }

        newest.map(chrono::DateTime::<chrono::Utc>::from)
    }
}

/// Index of discovered types organized by capability tags
#[derive(Debug)]
pub struct TypeIndex {
    /// Types indexed by capability tag
    by_tag: HashMap<String, Vec<TypeInfo>>,

    /// Types indexed by name (lowercase)
    by_name: HashMap<String, TypeInfo>,

    /// Workspace root used for discovery (kept for debugging/introspection)
    #[allow(dead_code)] // Debug: Preserved for introspection and debugging
    workspace_root: PathBuf,

    /// Semantic index for similarity search
    semantic_index: Option<SemanticIndex>,
}

impl TypeIndex {
    /// Build the type index from workspace discovery (slow, scans all source files)
    pub fn build(workspace_root: PathBuf) -> Self {
        let all_types = discover_all_types(&workspace_root);
        Self::from_types(&all_types, workspace_root)
    }

    /// Create index from a list of types (used by both build and load)
    fn from_types(all_types: &[TypeInfo], workspace_root: PathBuf) -> Self {
        let mut by_tag: HashMap<String, Vec<TypeInfo>> = HashMap::new();
        let mut by_name: HashMap<String, TypeInfo> = HashMap::new();

        for ty in all_types {
            // Index by name
            by_name.insert(ty.name.to_lowercase(), ty.clone());

            // Index by capability tags
            for tag in &ty.capability_tags {
                by_tag.entry(tag.clone()).or_default().push(ty.clone());
            }
        }

        // Build semantic index from type descriptions
        let semantic_index = Some(SemanticIndex::from_descriptions(all_types.iter().map(
            |ty| {
                // Combine name, description, and documentation for richer semantics
                let text = format!("{} {} {}", ty.name, ty.description, ty.documentation);
                (ty.path.as_str(), text)
            },
        )));

        Self {
            by_tag,
            by_name,
            workspace_root,
            semantic_index,
        }
    }

    /// Create index from types with optional pre-built semantic index
    fn from_types_with_semantic(
        all_types: &[TypeInfo],
        workspace_root: PathBuf,
        semantic_index: Option<SemanticIndex>,
    ) -> Self {
        let mut by_tag: HashMap<String, Vec<TypeInfo>> = HashMap::new();
        let mut by_name: HashMap<String, TypeInfo> = HashMap::new();

        for ty in all_types {
            by_name.insert(ty.name.to_lowercase(), ty.clone());
            for tag in &ty.capability_tags {
                by_tag.entry(tag.clone()).or_default().push(ty.clone());
            }
        }

        Self {
            by_tag,
            by_name,
            workspace_root,
            semantic_index,
        }
    }

    /// Save the type index to a cache file
    pub fn save(&self, cache_path: &Path) -> std::io::Result<()> {
        // Collect all types from the index
        let mut types: Vec<TypeInfo> = self.by_name.values().cloned().collect();
        types.sort_by(|a, b| a.path.cmp(&b.path));

        let cache = TypeIndexCache {
            version: TypeIndexCache::CURRENT_VERSION,
            types,
            workspace_root: self.workspace_root.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            semantic_index: self.semantic_index.clone(),
        };

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&cache).map_err(std::io::Error::other)?;
        fs::write(cache_path, json)
    }

    /// Load the type index from a cache file (returns index and cache for staleness check)
    pub fn load(cache_path: &Path) -> Option<(Self, TypeIndexCache)> {
        let content = fs::read_to_string(cache_path).ok()?;
        let cache: TypeIndexCache = serde_json::from_str(&content).ok()?;

        // Accept both v1 and v2 caches (v1 will have semantic_index=None)
        if cache.version > TypeIndexCache::CURRENT_VERSION {
            return None;
        }

        let workspace_root = cache.workspace_root.clone();
        let types = cache.types.clone();
        let semantic_index = cache.semantic_index.clone();
        Some((
            Self::from_types_with_semantic(&types, workspace_root, semantic_index),
            cache,
        ))
    }

    /// Get or build the global type index
    ///
    /// Tries to load from cache first. If stale, auto-rebuilds the index.
    /// Use `global_no_auto_rebuild` to skip auto-rebuild.
    pub fn global(workspace_root: PathBuf) -> &'static TypeIndex {
        Self::global_with_options(workspace_root, true)
    }

    /// Get or build the global type index without auto-rebuild.
    ///
    /// Tries to load from cache first, warns if stale.
    pub fn global_no_auto_rebuild(workspace_root: PathBuf) -> &'static TypeIndex {
        Self::global_with_options(workspace_root, false)
    }

    /// Internal implementation with configurable auto-rebuild.
    fn global_with_options(workspace_root: PathBuf, auto_rebuild: bool) -> &'static TypeIndex {
        TYPE_INDEX.get_or_init(|| {
            // Try loading from cache first
            let cache_path = workspace_root.join(TypeIndexCache::CACHE_PATH);
            if let Some((index, cache)) = Self::load(&cache_path) {
                // Check if cache is stale
                if let Some(true) = cache.is_stale(&workspace_root) {
                    if auto_rebuild {
                        tracing::info!(
                            "Type index cache is stale. Auto-rebuilding..."
                        );
                        return Self::regenerate(workspace_root);
                    } else {
                        tracing::warn!(
                            "Type index cache is stale. Run `dashflow introspect index --rebuild` to update."
                        );
                    }
                }
                return index;
            }

            // Build from source and save to cache
            let index = Self::build(workspace_root);
            if let Err(e) = index.save(&cache_path) {
                tracing::warn!("Failed to save type index cache: {}", e);
            }
            index
        })
    }

    /// Regenerate the type index from source, ignoring cache
    pub fn regenerate(workspace_root: PathBuf) -> Self {
        // Compute cache_path first before moving workspace_root
        let cache_path = workspace_root.join(TypeIndexCache::CACHE_PATH);
        let index = Self::build(workspace_root);
        if let Err(e) = index.save(&cache_path) {
            tracing::warn!("Failed to save type index cache: {}", e);
        }
        index
    }

    /// Find types matching any of the given capability tags
    pub fn find_by_tags(&self, tags: &[&str]) -> Vec<&TypeInfo> {
        let mut results: Vec<&TypeInfo> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for tag in tags {
            if let Some(types) = self.by_tag.get(*tag) {
                for ty in types {
                    if seen.insert(&ty.path) {
                        results.push(ty);
                    }
                }
            }
        }

        results
    }

    /// Find types matching a name pattern
    pub fn find_by_name(&self, pattern: &str) -> Vec<&TypeInfo> {
        let pattern_lower = pattern.to_lowercase();
        self.by_name
            .iter()
            .filter(|(name, _)| name.contains(&pattern_lower))
            .map(|(_, ty)| ty)
            .collect()
    }

    /// Get the total number of indexed types
    pub fn type_count(&self) -> usize {
        self.by_name.len()
    }

    /// Get all unique capability tags in the index (Gap 18: auto-correlation)
    pub fn all_capability_tags(&self) -> Vec<&str> {
        self.by_tag.keys().map(|s| s.as_str()).collect()
    }

    /// Perform semantic search for types similar to the query.
    ///
    /// Uses TF-IDF based similarity to find types whose descriptions
    /// are semantically similar to the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Natural language query (e.g., "keyword search with BM25")
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    ///
    /// Vector of (TypeInfo, similarity_score) sorted by descending similarity.
    /// Returns empty vector if semantic index is not available.
    pub fn search_semantic(&self, query: &str, limit: usize) -> Vec<(&TypeInfo, f32)> {
        let Some(ref semantic_index) = self.semantic_index else {
            return Vec::new();
        };

        let results = semantic_index.search(query, limit);

        results
            .into_iter()
            .filter_map(|r| {
                // Look up type info by path
                // The semantic index stores full paths, but by_name uses lowercase names
                // So we need to find the type in by_name by matching the path
                self.by_name
                    .values()
                    .find(|ty| ty.path == r.type_path)
                    .map(|ty| (ty, r.score))
            })
            .collect()
    }

    /// Check if semantic search is available
    pub fn has_semantic_index(&self) -> bool {
        self.semantic_index.is_some()
    }

    /// Get semantic index statistics
    pub fn semantic_stats(&self) -> Option<(usize, usize)> {
        self.semantic_index
            .as_ref()
            .map(|idx| (idx.type_count(), idx.vocabulary_size()))
    }
}

/// Enrich a lint warning with dynamically discovered alternatives
#[derive(Clone)]
pub struct IntrospectionEnricher {
    index: &'static TypeIndex,
}

impl IntrospectionEnricher {
    /// Create a new enricher using the global type index
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            index: TypeIndex::global(workspace_root),
        }
    }

    /// Find alternative DashFlow types for a given pattern
    ///
    /// Uses a hybrid approach: first semantic search with pattern name + category,
    /// then falls back to tag-based lookup. This provides better relevance.
    pub fn find_alternatives(&self, pattern_name: &str, category: &str) -> Vec<AlternativeType> {
        // Build semantic query from pattern name and category
        let query = format!(
            "{} {}",
            pattern_name.replace('_', " "),
            category.replace('_', " ")
        );

        // Try semantic search first (if index has semantic capability)
        let semantic_results = self.index.search_semantic(&query, 5);
        if !semantic_results.is_empty() {
            // Filter results with minimum relevance score (0.2)
            let relevant: Vec<_> = semantic_results
                .into_iter()
                .filter(|(_, score)| *score >= 0.2)
                .take(5)
                .map(|(ty, _)| ty)
                .collect();

            if !relevant.is_empty() {
                return relevant
                    .into_iter()
                    .map(|ty| self.type_to_alternative(ty))
                    .collect();
            }
        }

        // Fall back to tag-based lookup if semantic search yields nothing
        let tags = self.infer_tags_from_pattern(pattern_name, category);
        let types = self.index.find_by_tags(&tags);

        types
            .into_iter()
            .take(5)
            .map(|ty| self.type_to_alternative(ty))
            .collect()
    }

    /// Convert a TypeInfo to an AlternativeType
    fn type_to_alternative(&self, ty: &TypeInfo) -> AlternativeType {
        AlternativeType {
            name: ty.name.clone(),
            path: ty.path.clone(),
            crate_name: ty.crate_name.clone(),
            kind: match ty.kind {
                TypeKind::Struct => "struct",
                TypeKind::Trait => "trait",
                TypeKind::Function => "fn",
                TypeKind::Enum => "enum",
                TypeKind::TypeAlias => "type",
                TypeKind::Const => "const",
            },
            description: ty.description.clone(),
        }
    }

    /// Infer capability tags from a pattern name and category (Gap 18: auto-correlation)
    ///
    /// Uses fuzzy matching against actual TypeIndex capability tags to automatically
    /// correlate patterns with types, eliminating hardcoded mappings.
    fn infer_tags_from_pattern(&self, pattern_name: &str, category: &str) -> Vec<&str> {
        let mut tags = Vec::new();

        // Get all available tags from the TypeIndex
        let available_tags = self.index.all_capability_tags();

        // Split pattern name into keywords (e.g., "bm25_search" -> ["bm25", "search"])
        let pattern_keywords: Vec<&str> = pattern_name.split('_').collect();

        // Auto-correlate: find tags that match pattern keywords
        for tag in &available_tags {
            let tag_lower = tag.to_lowercase();

            // Exact match with any pattern keyword
            if pattern_keywords
                .iter()
                .any(|kw| kw.to_lowercase() == tag_lower)
            {
                tags.push(*tag);
                continue;
            }

            // Prefix/substring match (e.g., "retriev" matches "retriever")
            if pattern_keywords.iter().any(|kw| {
                let kw_lower = kw.to_lowercase();
                tag_lower.starts_with(&kw_lower) || kw_lower.starts_with(&tag_lower)
            }) {
                tags.push(*tag);
                continue;
            }

            // Pattern name contains tag (e.g., "cost_tracking" contains "cost")
            if pattern_name.to_lowercase().contains(&tag_lower) {
                tags.push(*tag);
            }
        }

        // Category-based fallback correlation
        let category_tags = match category {
            "retrievers" => vec!["retriever", "search"],
            "observability" => vec!["metrics", "tracing", "cost_tracking"],
            "evaluation" => vec!["evaluation"],
            "models" => vec!["llm", "chat", "embeddings"],
            "loaders" => vec!["document_loader", "chunking"],
            "memory" => vec!["caching"],
            _ => vec![],
        };

        for cat_tag in category_tags {
            if available_tags.contains(&cat_tag) && !tags.contains(&cat_tag) {
                tags.push(cat_tag);
            }
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        tags.retain(|tag| seen.insert(*tag));

        tags
    }

    /// Format discovered alternatives for warning message
    pub fn format_alternatives(&self, alternatives: &[AlternativeType]) -> String {
        if alternatives.is_empty() {
            return String::new();
        }

        let mut output = String::from("\n   Discovered alternatives:\n");

        for alt in alternatives {
            output.push_str(&format!(
                "     - {} {} from {}\n",
                alt.kind, alt.name, alt.crate_name
            ));
            if !alt.description.is_empty() {
                output.push_str(&format!("       {}\n", alt.description));
            }
        }

        output
    }
}

/// An alternative DashFlow type that could be used instead
#[derive(Debug, Clone)]
pub struct AlternativeType {
    /// Type name
    pub name: String,
    /// Full path (e.g., "dashflow_opensearch::OpenSearchBM25Retriever")
    pub path: String,
    /// Crate containing this type
    pub crate_name: String,
    /// Type kind (struct, trait, fn, etc.)
    pub kind: &'static str,
    /// Description from doc comment
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_tags_from_pattern() {
        // Build a TypeIndex with expected capability tags for testing
        let mut by_tag: HashMap<String, Vec<TypeInfo>> = HashMap::new();

        // Add tags that should be matched by the patterns
        by_tag.insert("bm25".to_string(), Vec::new());
        by_tag.insert("retriever".to_string(), Vec::new());
        by_tag.insert("search".to_string(), Vec::new());
        by_tag.insert("cost_tracking".to_string(), Vec::new());
        by_tag.insert("metrics".to_string(), Vec::new());
        by_tag.insert("tracing".to_string(), Vec::new());

        // Note: Box::leak is acceptable here - test process exits after tests,
        // and IntrospectionEnricher requires &'static TypeIndex by design
        let enricher = IntrospectionEnricher {
            index: Box::leak(Box::new(TypeIndex {
                by_tag,
                by_name: HashMap::new(),
                workspace_root: PathBuf::from("."),
                semantic_index: None,
            })),
        };

        // Gap 18: Auto-correlation should match pattern keywords to available tags
        let tags = enricher.infer_tags_from_pattern("bm25_search", "retrievers");
        assert!(
            tags.contains(&"bm25"),
            "Should match 'bm25' from pattern keyword"
        );
        assert!(
            tags.contains(&"retriever"),
            "Should include 'retriever' from category"
        );
        assert!(
            tags.contains(&"search"),
            "Should match 'search' from pattern keyword"
        );

        let tags = enricher.infer_tags_from_pattern("cost_tracking", "observability");
        assert!(
            tags.contains(&"cost_tracking"),
            "Should match 'cost_tracking' from pattern"
        );
        assert!(
            tags.contains(&"metrics"),
            "Should include 'metrics' from category"
        );
    }
}
