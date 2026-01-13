# Audit: Other Vector Store Crates

**Status:** ✅ COMPLETE - ALL 14 vector stores verified SAFE (#1426, #1430, re-verified #2190)
**Priority:** P3 (Vector Stores)

This file covers vector store crates not covered in their own audit files.

---

## dashflow-annoy

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/store.rs` - ✅ SAFE (verified #1426 M-384)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/store.rs`: Uses poison-safe `.unwrap_or_else(|e| e.into_inner())` pattern for ALL mutex locks (starts line 206). All `.unwrap()` exclusively in test module. No `panic!` or `.expect()` in production code.

---

## dashflow-cassandra

### Files
- [x] `src/lib.rs` - ✅ SAFE (re-exports only)
- [x] `src/cassandra_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- `src/cassandra_store.rs:38`: Single `.unwrap()` with SAFETY comment "Safe: we checked non-empty above". Guards check `name.is_empty()` and return early, guaranteeing `.next()` returns Some. No test module - crate has no tests currently.

---

## dashflow-clickhouse

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/clickhouse_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/clickhouse_store.rs:27`: Single `.unwrap()` with SAFETY comment + `#[allow(clippy::unwrap_used)]`. Guards check `name.is_empty()` and return early, guaranteeing `.next()` returns Some.

---

## dashflow-elasticsearch

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/database_chain.rs` - ✅ SAFE (verified #1430)
- [x] `src/elasticsearch.rs` - ✅ SAFE (verified #1430)
- [x] `src/bm25_retriever.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/database_chain.rs`: Test module at line 552. Production `.expect()` calls use HARDCODED template strings (always valid). All other `.unwrap()` in `#[cfg(test)]` module.
- `src/elasticsearch.rs`: Test module. All `.unwrap()` in test code only.
- `src/bm25_retriever.rs`: Test module. All `.unwrap()` in `#[tokio::test]` functions.

---

## dashflow-hnsw

### Files
- [x] `src/lib.rs` - ✅ SAFE (re-exports only)
- [x] `src/hnsw_store.rs` - ✅ SAFE (verified #1426 M-385)

### Known Issues - ALL SAFE
- `src/hnsw_store.rs`: Uses poison-safe `.unwrap_or_else(|e| e.into_inner())` pattern for ALL mutex locks (12 occurrences, lines 107-369). All `.unwrap()` exclusively in test module (line 391+). No `panic!` or `.expect()` in production code.

---

## dashflow-lancedb

### Files
- [x] `src/lib.rs` - ✅ SAFE (verified #1430) - MockEmbeddings in docs only
- [x] `src/lancedb_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- `src/lib.rs:24-34,62`: MockEmbeddings in documentation (doc-comment examples, not production code)
- `src/lancedb_store.rs:42-52`: MockEmbeddings struct is for testing/examples only
- **NO `.unwrap()`/`.expect()`/`panic!` found in crate** - completely clean!

---

## dashflow-milvus

### Files
- [x] `src/lib.rs` - ✅ SAFE (re-exports only)
- [x] `src/milvus_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- Test module at line 632. All `.unwrap()`/`.expect()` in test code only (lines 677, 690 - MockEmbeddings impl and test helper).

---

## dashflow-mongodb

### Files
- [x] `src/lib.rs` - ✅ SAFE (verified #1430)
- [x] `src/mongodb_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- Example uses "fake embeddings" - this is documentation only, not production code
- **NO `.unwrap()`/`.expect()`/`panic!` found in crate** - completely clean!

---

## dashflow-opensearch

### Files
- [x] `src/lib.rs` - ✅ SAFE (re-exports only)
- [x] `src/opensearch_store.rs` - ✅ SAFE (verified #1430)
- [x] `src/bm25_retriever.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- Test module at line 585 in bm25_retriever.rs. All `.unwrap()`/`.expect()` in test code only (lines 599, 615, 622, 644, 649, 667, 683).
- opensearch_store.rs has no `.unwrap()`/`.expect()`/`panic!` - completely clean!

---

## dashflow-sqlitevss

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/sqlitevss_store.rs` - ✅ SAFE (verified #1430, re-verified #2190)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/sqlitevss_store.rs`: No production `.unwrap()` calls - code was refactored to use `let Some(ids) = ids else { ... }` pattern. Test module at line 447. All `.unwrap()` in test code only.

---

## dashflow-supabase

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/supabase_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- **NO `.unwrap()`/`.expect()`/`panic!` found in crate** - completely clean!

---

## dashflow-timescale

### Files
- [x] `src/lib.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- **NO `.unwrap()`/`.expect()`/`panic!` found in crate** - completely clean!

---

## dashflow-typesense

### Files
- [x] `src/lib.rs` - ✅ SAFE (re-exports only)
- [x] `src/typesense_store.rs` - ✅ SAFE (verified #1430)

### Known Issues - ALL SAFE
- File has `#![allow(clippy::unwrap_used)]` at line 3 with documented rationale: "JSON parsing of known valid structures"
- `src/typesense_store.rs:44`: `.unwrap_or_else()` fallback pattern (SAFE)
- `src/typesense_store.rs:257`: `serde_json::to_string(doc).unwrap()` on TypesenseDocument - SAFE because TypesenseDocument struct (lines 78-84) contains only: `id: String`, `vec: Vec<f32>`, `text_and_metadata: HashMap<String, JsonValue>`. All fields are always JSON-serializable.
- Test module at line 416. Lines 467, 486 `.unwrap()` are in test functions.

---

## dashflow-usearch

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/usearch_store.rs` - ✅ SAFE (verified #1430, re-verified #2190)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/usearch_store.rs`: No production `.unwrap()` calls - code was refactored. Test module at line 708. All `.unwrap()` in test code only.

---

## dashflow-weaviate

### Files
- [x] `src/lib.rs` - ✅ SAFE: No unimplemented! or unsafe patterns (re-verified #2190)
- [x] `src/weaviate.rs` - ✅ SAFE (verified #1430, re-verified #2190)

### Known Issues - ALL SAFE
- `src/lib.rs`: No `unimplemented!` macros (doc examples were refactored)
- `src/weaviate.rs`: No `unimplemented!` macros (doc examples were refactored)
- **NO `.unwrap()`/`.expect()`/`panic!` found in crate** - completely clean!

---

## Common Issues - ALL RESOLVED

1. **unimplemented! in documentation** - ✅ REMOVED: All `unimplemented!` macros have been removed from doc-comment examples in vector store crates (re-verified #2190)
2. **High .unwrap() counts** - ✅ SAFE: Original counts were misleading:
   - annoy: Uses poison-safe `.unwrap_or_else()` for mutexes; all `.unwrap()` in test code
   - hnsw: Uses poison-safe `.unwrap_or_else()` for mutexes; all `.unwrap()` in test code
   - usearch: No production `.unwrap()` - code was refactored (re-verified #2190)
   - sqlitevss: No production `.unwrap()` - uses `let Some(ids) = ids else { ... }` pattern (re-verified #2190)
3. **MockEmbeddings in lancedb** - ✅ SAFE: In doc examples and test helpers only, not production code

---

## Critical Checks (Apply to ALL)

1. **Real database connections** - Not mocked
2. **Vector operations correct** - Distance calculations
3. **Metadata filtering** - All filter types work
4. **Persistence** - Data survives restarts

---

## Test Coverage Gaps

- [ ] Integration tests with real databases
- [ ] Large dataset tests
- [ ] Concurrent access tests
