//! Search Routes
//!
//! Handlers for search operations: unified, semantic, keyword, capability.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::api::{
    types::{error_codes, ApiError},
    AppState,
};
use crate::cache::{cache_get_json, cache_set_json, keys as cache_keys};
use crate::{Capability, CapabilityMatch, PackageMetadata};

/// Search routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST /search - Unified search (semantic + keyword + capability)
        .route("/", post(unified_search))
        // POST /search/semantic - Semantic search only
        .route("/semantic", post(semantic_search))
        // GET /search/keyword - Keyword search (query param)
        .route("/keyword", get(keyword_search))
        // POST /search/capability - Find by capability
        .route("/capability", post(capability_search))
}

/// Search request for API
#[derive(Debug, Clone, Deserialize)]
pub struct SearchApiRequest {
    /// Natural language query (for semantic search)
    #[serde(default)]
    pub query: Option<String>,
    /// Keywords (for keyword search)
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Required capabilities
    #[serde(default)]
    pub capabilities: Option<Vec<Capability>>,
    /// Maximum results to return
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Offset for pagination
    #[serde(default)]
    pub offset: u32,
}

/// Search response for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchApiResponse {
    /// Matching packages
    pub results: Vec<SearchResultItem>,
    /// Total matches (for pagination)
    pub total: u64,
    /// Time taken in milliseconds
    pub took_ms: u64,
    /// Which search methods were used
    pub sources: SearchSources,
}

/// Single search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    /// Package metadata
    pub metadata: PackageMetadata,
    /// Relevance score (0-1)
    pub score: f64,
    /// Why this matched
    pub match_reasons: Vec<String>,
}

/// Which search sources contributed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSources {
    pub semantic: bool,
    pub keyword: bool,
    pub capability: bool,
}

fn default_limit() -> u32 {
    20
}

/// Unified search combining semantic, keyword, and capability matching
async fn unified_search(
    State(state): State<AppState>,
    Json(request): Json<SearchApiRequest>,
) -> Result<Json<SearchApiResponse>, (StatusCode, Json<ApiError>)> {
    use std::collections::HashMap;

    let start = Instant::now();

    // Generate cache key based on request parameters
    let cache_key_input = format!(
        "unified:{}:{}:{}:{}",
        request.query.as_deref().unwrap_or(""),
        request
            .keywords
            .as_ref()
            .map(|k| k.join(","))
            .unwrap_or_default(),
        request.limit,
        request.offset
    );
    let cache_key = cache_keys::search(&cache_key_input);

    // Check cache first
    if let Ok(Some(mut cached)) =
        cache_get_json::<SearchApiResponse>(state.data_cache.as_ref(), &cache_key).await
    {
        cached.took_ms = start.elapsed().as_millis() as u64;
        return Ok(Json(cached));
    }

    // Determine which search methods to use
    let use_semantic = request.query.is_some();
    let use_keyword = request.keywords.is_some() || request.query.is_some();
    let use_capability = request.capabilities.is_some();

    // Collect all results with their scores (hash -> (metadata, score, reasons))
    let mut merged_results: HashMap<String, (PackageMetadata, f64, Vec<String>)> = HashMap::new();

    // Execute semantic search if query provided
    if let Some(ref query) = request.query {
        if let Ok(semantic_results) = state.search.search(query, request.limit as usize).await {
            for r in semantic_results {
                let hash = r.metadata.hash.clone();
                merged_results.insert(
                    hash,
                    (r.metadata, r.score, vec!["semantic_match".to_string()]),
                );
            }
        }

        // Also do keyword search on the query
        if let Ok(keyword_results) = state
            .metadata
            .search_keyword(query, request.limit as usize, 0)
            .await
        {
            for pkg in keyword_results {
                let hash = pkg.hash.to_string();
                merged_results
                    .entry(hash.clone())
                    .and_modify(|(_, score, reasons)| {
                        *score += 0.5; // Boost score if found by both methods
                        reasons.push("keyword_match".to_string());
                    })
                    .or_insert_with(|| {
                        (
                            PackageMetadata::from(&pkg),
                            0.5,
                            vec!["keyword_match".to_string()],
                        )
                    });
            }
        }
    }

    // Execute keyword search on explicit keywords
    if let Some(ref keywords) = request.keywords {
        let query = keywords.join(" ");
        if let Ok(keyword_results) = state
            .metadata
            .search_keyword(&query, request.limit as usize, 0)
            .await
        {
            for pkg in keyword_results {
                let hash = pkg.hash.to_string();
                merged_results
                    .entry(hash.clone())
                    .and_modify(|(_, score, reasons)| {
                        *score += 0.5;
                        if !reasons.contains(&"keyword_match".to_string()) {
                            reasons.push("keyword_match".to_string());
                        }
                    })
                    .or_insert_with(|| {
                        (
                            PackageMetadata::from(&pkg),
                            0.5,
                            vec!["keyword_match".to_string()],
                        )
                    });
            }
        }
    }

    // Sort by score descending and collect results
    let mut results: Vec<SearchResultItem> = merged_results
        .into_iter()
        .map(|(_, (metadata, score, reasons))| SearchResultItem {
            metadata,
            score,
            match_reasons: reasons,
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply pagination
    let results: Vec<_> = results
        .into_iter()
        .skip(request.offset as usize)
        .take(request.limit as usize)
        .collect();

    let total = results.len() as u64;
    let took_ms = start.elapsed().as_millis() as u64;

    let response = SearchApiResponse {
        results,
        total,
        took_ms,
        sources: SearchSources {
            semantic: use_semantic,
            keyword: use_keyword,
            capability: use_capability,
        },
    };

    // Cache the response
    let _ = cache_set_json(
        state.data_cache.as_ref(),
        &cache_key,
        &response,
        Some(state.cache_config.search_ttl),
    )
    .await;

    Ok(Json(response))
}

/// Semantic search request
#[derive(Debug, Deserialize)]
struct SemanticSearchRequest {
    query: Option<String>,
    limit: Option<u32>,
}

/// Semantic-only search using embeddings
async fn semantic_search(
    State(state): State<AppState>,
    Json(request): Json<SemanticSearchRequest>,
) -> Result<Json<SearchApiResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();

    let query = request.query.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                "Query is required for semantic search",
            )),
        )
    })?;

    let limit = request.limit.unwrap_or(20);

    // Generate cache key for this search
    let cache_key = cache_keys::search(&format!("semantic:{}:{}", query, limit));

    // Check cache first
    if let Ok(Some(mut cached)) =
        cache_get_json::<SearchApiResponse>(state.data_cache.as_ref(), &cache_key).await
    {
        cached.took_ms = start.elapsed().as_millis() as u64;
        return Ok(Json(cached));
    }

    // Cache miss - perform semantic search
    let results = state
        .search
        .search(&query, limit as usize)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Search failed: {}", e),
                )),
            )
        })?;

    let search_results: Vec<_> = results
        .into_iter()
        .map(|r| SearchResultItem {
            metadata: r.metadata,
            score: r.score,
            match_reasons: vec!["semantic_match".to_string()],
        })
        .collect();

    let total = search_results.len() as u64;
    let took_ms = start.elapsed().as_millis() as u64;

    let response = SearchApiResponse {
        results: search_results,
        total,
        took_ms,
        sources: SearchSources {
            semantic: true,
            keyword: false,
            capability: false,
        },
    };

    // Cache the response
    let _ = cache_set_json(
        state.data_cache.as_ref(),
        &cache_key,
        &response,
        Some(state.cache_config.search_ttl),
    )
    .await;

    Ok(Json(response))
}

/// Keyword search parameters
#[derive(Debug, Deserialize)]
struct KeywordSearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

/// Keyword-only search using metadata store
async fn keyword_search(
    State(state): State<AppState>,
    Query(params): Query<KeywordSearchParams>,
) -> Result<Json<SearchApiResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();

    // Generate cache key for this search
    let cache_key = cache_keys::search(&format!("keyword:{}:{}", params.q, params.limit));

    // Check cache first
    if let Ok(Some(mut cached)) =
        cache_get_json::<SearchApiResponse>(state.data_cache.as_ref(), &cache_key).await
    {
        cached.took_ms = start.elapsed().as_millis() as u64;
        return Ok(Json(cached));
    }

    // Cache miss - search using metadata store's keyword index
    let packages = state
        .metadata
        .search_keyword(&params.q, params.limit as usize, 0)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Keyword search failed: {}", e),
                )),
            )
        })?;

    let results: Vec<SearchResultItem> = packages
        .into_iter()
        .map(|pkg| SearchResultItem {
            metadata: PackageMetadata::from(&pkg),
            score: 1.0, // Keyword search doesn't have a normalized score
            match_reasons: vec!["keyword_match".to_string()],
        })
        .collect();

    let total = results.len() as u64;
    let took_ms = start.elapsed().as_millis() as u64;

    let response = SearchApiResponse {
        results,
        total,
        took_ms,
        sources: SearchSources {
            semantic: false,
            keyword: true,
            capability: false,
        },
    };

    // Cache the response
    let _ = cache_set_json(
        state.data_cache.as_ref(),
        &cache_key,
        &response,
        Some(state.cache_config.search_ttl),
    )
    .await;

    Ok(Json(response))
}

/// Capability search request
#[derive(Debug, Deserialize)]
struct CapabilitySearchRequest {
    capabilities: Vec<Capability>,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    require_all: bool,
}

/// Find packages by capability
async fn capability_search(
    State(state): State<AppState>,
    Json(request): Json<CapabilitySearchRequest>,
) -> Result<Json<SearchApiResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();

    if request.capabilities.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                "At least one capability required".to_string(),
            )),
        ));
    }

    // Generate cache key for this search
    let cap_names: Vec<_> = request
        .capabilities
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let cache_key = cache_keys::search(&format!(
        "capability:{}:{}:{}",
        cap_names.join(","),
        request.limit,
        request.require_all
    ));

    // Check cache first
    if let Ok(Some(mut cached)) =
        cache_get_json::<SearchApiResponse>(state.data_cache.as_ref(), &cache_key).await
    {
        cached.took_ms = start.elapsed().as_millis() as u64;
        return Ok(Json(cached));
    }

    // Search for packages using capability names as keywords
    // Use a larger limit since we'll filter by capability
    let search_limit = (request.limit as usize) * 10;
    let mut all_packages = Vec::new();

    for capability in &request.capabilities {
        let packages = state
            .metadata
            .search_keyword(&capability.name, search_limit, 0)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        error_codes::INTERNAL_ERROR,
                        format!("Capability search failed: {}", e),
                    )),
                )
            })?;
        all_packages.extend(packages);
    }

    // De-duplicate by hash
    let mut seen = std::collections::HashSet::new();
    all_packages.retain(|pkg| seen.insert(pkg.hash.clone()));

    // Filter packages that match the required capabilities
    let mut results: Vec<SearchResultItem> = Vec::new();

    for pkg in all_packages {
        let (matches, reasons) = CapabilityMatch::matches(&pkg, &request.capabilities);

        // If require_all is true, all capabilities must match
        // If require_all is false, at least one capability must match
        let should_include = if request.require_all {
            matches
        } else {
            !reasons.is_empty()
        };

        if should_include {
            // Calculate score based on how many capabilities matched
            let matched_count = reasons
                .iter()
                .filter(|r| matches!(r, crate::MatchReason::CapabilityMatch { .. }))
                .count();
            let score = matched_count as f64 / request.capabilities.len() as f64;

            let match_reasons: Vec<String> = reasons.iter().map(|r| format!("{:?}", r)).collect();

            results.push(SearchResultItem {
                metadata: PackageMetadata::from(&pkg),
                score,
                match_reasons,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply limit
    results.truncate(request.limit as usize);

    let total = results.len() as u64;
    let took_ms = start.elapsed().as_millis() as u64;

    let response = SearchApiResponse {
        results,
        total,
        took_ms,
        sources: SearchSources {
            semantic: false,
            keyword: false,
            capability: true,
        },
    };

    // Cache the response
    let _ = cache_set_json(
        state.data_cache.as_ref(),
        &cache_key,
        &response,
        Some(state.cache_config.search_ttl),
    )
    .await;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 20);
    }
}
