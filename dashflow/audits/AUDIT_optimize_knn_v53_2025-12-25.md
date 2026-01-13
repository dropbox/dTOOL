# v53 Skeptical Audit: optimize/knn.rs

**Auditor:** Worker #1725
**Date:** 2025-12-25
**File:** `crates/dashflow/src/optimize/knn.rs`
**Lines:** 417

## Summary

K-nearest neighbors retrieval for finding similar examples based on embedding similarity.
Used for retrieval-augmented few-shot learning in DashOptimize.

**Verdict:** No significant issues (P0/P1/P2/P3). 2 P4 items found.

## Architecture

```
KNN<E: Embeddings>
├── k: usize                          (number of neighbors)
├── trainset: Vec<Example>            (examples to search)
├── embedder: Arc<E>                  (embedding client)
├── trainset_embeddings: Vec<Vec<f32>> (pre-computed)
│
├── new(k, trainset, embedder) -> Result<Self, Error>
│   └── Pre-computes embeddings for all training examples
├── retrieve(query) -> Result<Vec<Example>, Error>
│   ├── Embeds query
│   ├── Computes cosine similarity with all trainset embeddings
│   └── Returns top-k most similar examples
├── k() -> usize                      (getter)
└── trainset_size() -> usize          (getter)

cosine_similarity(a, b) -> f32        (internal helper)
└── Handles zero vectors gracefully (returns 0.0)
```

## Code Breakdown

| Section | Lines | % | Description |
|---------|-------|---|-------------|
| Module docs | 1-43 | 10% | Good documentation with example |
| KNN struct | 44-71 | 7% | Clean struct definition |
| Constructor | 73-130 | 14% | Pre-computes embeddings |
| retrieve() | 131-185 | 13% | Main retrieval logic |
| cosine_similarity | 198-218 | 5% | Vector math helper |
| **Tests** | 220-417 | **47%** | Comprehensive test suite |

## Analysis

### Strengths

1. **Pre-computed embeddings**: Training set embedded once at initialization
2. **Good NaN handling**: Sort uses `unwrap_or(Equal)` for NaN scores
3. **Zero vector handling**: cosine_similarity returns 0.0 for zero vectors
4. **Input text normalization**: Consistent "key: value | key: value" format

### P4 Issues Found

#### M-859: No validation of k parameter
**File:** `knn.rs:88`
**Category:** Defensive

```rust
pub async fn new(k: usize, trainset: Vec<Example>, embedder: Arc<E>) -> Result<Self, Error>
```

No validation that `k > 0`. With `k = 0`:
- Constructor succeeds
- `retrieve()` returns empty vec

Not an error per se, but semantically useless. Also no warning if `k > trainset.len()`.

**Impact:** P4 - Edge case. Reasonable behavior but could warn.

---

#### M-860: cosine_similarity panics on dimension mismatch
**File:** `knn.rs:207`
**Category:** Error handling

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must have same length");
```

Uses `assert_eq!` which panics if vectors have different lengths. Could happen if:
- Embedding model changed between KNN creation and retrieval
- Buggy embedder returns inconsistent dimensions

**Impact:** P4 - Internal function, assumption is that embedder is consistent.
In practice, any sane embedder returns same dimensions for all calls.
Panic is acceptable as this indicates a programming error, not runtime error.

---

### Test Mock Analysis

Tests use `MockEmbedder` (lines 270-313). This is a **legitimate test double**:
- Tests verify KNN retrieval logic, not Embeddings behavior
- Mock provides deterministic embeddings for predictable test outcomes
- No violation of mock prohibition in CLAUDE.md

## Edge Cases Handled Well

1. **Zero vector**: Returns similarity 0.0 (not NaN or panic)
2. **NaN scores**: Sort treats NaN as Equal (stable ordering)
3. **Empty trainset**: Constructor succeeds, retrieve returns empty
4. **k > trainset.len()**: Returns all available (min(k, n))

## Verification

No changes made - audit only.

## Recommendations

1. **M-859 (Optional):** Add `tracing::debug!` when k=0 or k > trainset.len()
2. **M-860 (Optional):** Replace assert with Result error for defensive coding
