# Audit: core/retrievers.rs v111

**Date:** 2025-12-25
**Worker:** #1819
**File:** `crates/dashflow/src/core/retrievers.rs` (3090 lines)
**Scope:** Main retrievers module including VectorStoreRetriever, EnsembleRetriever, MultiQueryRetriever, ContextualCompressionRetriever, plus submodule declarations

## Summary

Audited the core retrievers module for bugs, safety issues, and correctness problems. Found and fixed one P3 issue with RunnableConfig propagation in EnsembleRetriever.

## Issues Found

### P3 - Fixed

| ID | Description | Location | Fix |
|----|-------------|----------|-----|
| **M-1079** | EnsembleRetriever ignores RunnableConfig - parameter was unused (`_config`) and hardcoded to `None` when calling child retrievers | Lines 2081, 2086 | Config is now cloned and passed to each spawned retriever task |

### P4 - Not Fixed (Low Priority)

| Issue | Description | Location | Impact |
|-------|-------------|----------|--------|
| O(n^2) dedup | `unique_documents()` uses `seen.contains(&doc)` for full Document comparison in a loop, while `unique_by_key()` correctly uses HashSet | Lines 1443-1455 | Low - typical document sets are small |
| Code duplication | doc_key extraction logic repeated 3 times in `weighted_reciprocal_rank()` | Lines 1999-2014, 2026-2036, 2044-2054 | Maintainability only |

## Code Quality Observations

### Positive
- Comprehensive test coverage (~1200 lines of tests, ~42% of file)
- Good validation in `validate_config()` for search parameters
- `try_new()` pattern for fallible construction with proper error types
- Proper use of async_trait for async implementations
- Well-documented with examples in doc comments
- Correct parallel retrieval in EnsembleRetriever using tokio::spawn

### Comparison to MergerRetriever
MergerRetriever (in `merger_retriever.rs`) was already correct - it properly clones and passes config:
```rust
let config = config.cloned();
handles.push(tokio::spawn(async move {
    retriever
        .get_relevant_documents(&query, config.as_ref())
        .await
}));
```

EnsembleRetriever now matches this pattern after the fix.

## Files Changed

- `crates/dashflow/src/core/retrievers.rs` - Fixed EnsembleRetriever config propagation

## Test Results

All 8 EnsembleRetriever tests pass after fix:
- test_ensemble_basic_merging
- test_ensemble_no_duplicates
- test_ensemble_with_id_key
- test_ensemble_equal_weights_constructor
- test_ensemble_weighted_preference
- test_ensemble_empty_retriever
- test_ensemble_try_new_valid
- test_ensemble_try_new_mismatched_weights

## Submodules Not Audited

The following submodules (10,336 lines total) were not deeply audited in this pass:
- `bm25_retriever.rs` (929 lines)
- `tfidf_retriever.rs` (926 lines)
- `knn_retriever.rs` (868 lines)
- `rephrase_query_retriever.rs` (871 lines)
- `parent_document_retriever.rs` (1692 lines)
- `time_weighted_retriever.rs` (1292 lines)
- `web_research_retriever.rs` (1006 lines)
- `self_query.rs` (1055 lines)
- `merger_retriever.rs` (760 lines) - Quick review showed correct config handling
- Stub retrievers (elasticsearch, pinecone, weaviate)

These should be audited in a future pass.

## Verification

```bash
# All tests pass
cargo test -p dashflow --lib -- retrievers::tests::test_ensemble
# running 8 tests ... ok. 8 passed
```
