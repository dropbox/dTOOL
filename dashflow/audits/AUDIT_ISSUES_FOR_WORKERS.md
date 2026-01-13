# Issues for Workers to Fix

**Created:** 2025-12-16
**Updated:** 2025-12-16 (Worker #873 completed all remaining phases)
**Status:** COMPLETE (21/21 phases complete)
**Roadmap:** See [ROADMAP_CURRENT.md Part 17](../ROADMAP_CURRENT.md#part-17-codebase-audit-fixes-phases-381-401) (Phases 381-401)

---

## Priority 1: Actual Bugs/Issues - ✅ COMPLETE

### 1.1 Missing Timeout in OpenAI Assistant Wait Loop - ✅ DONE (#870)
**File:** `dashflow-openai/src/assistant.rs:441` (`wait_for_run()` function)
**Phase:** 381
**Status:** COMPLETE

### 1.2 Mutex `.lock().unwrap()` in FAISS Store - ✅ DONE (#870)
**File:** `dashflow-faiss/src/faiss_store.rs`
**Phase:** 382
**Status:** COMPLETE (crate excluded due to upstream Send/Sync issues)

---

## Priority 2: Test Coverage Gaps

### 2.1 Database Tests - Testcontainers (Phases 383-388)

| Service | Test Count | Phase | Status |
|---------|------------|-------|--------|
| PostgreSQL | 6 | 383 | ✅ DONE (#871) |
| Redis Stack | 12 | 384 | ✅ DONE (#871) |
| Cassandra | 8 | 385 | ✅ DONE (#871) |
| ChromaDB | 28 | 386 | ✅ DONE (#875) |
| DynamoDB | 9 | 387 | ✅ DONE (#875) |
| S3 | 5 | 388 | ✅ DONE (#875) |

### 2.2 API Tests - Mock Servers (Phases 389-391)

| Service | Test Count | Phase | Status |
|---------|------------|-------|--------|
| OpenAI API | 30 | 389 | ✅ DONE (#875) |
| HuggingFace API | 32 | 390 | ✅ DONE (#875) |
| Together AI | 8 | 391 | ✅ DONE (#875) |

### 2.3 Infrastructure (Phases 392-393)

| Task | Phase | Status |
|------|-------|--------|
| Create `.env.test` template | 392 | ✅ DONE (#871, was 386) |
| Add CI job for integration tests | 393 | MOOT (no GitHub Actions CI) |

---

## Priority 3: Code Quality Issues

### 3.1 Doc Examples with `unimplemented!()`
**Files affected:**
- `dashflow-redis/src/lib.rs` (3 occurrences)
- `dashflow-supabase/src/lib.rs`
- `dashflow-clickhouse/src/lib.rs`
- `dashflow-qdrant/src/lib.rs`, `src/qdrant.rs` (9 occurrences)
- `dashflow-weaviate/src/lib.rs`, `src/weaviate.rs`
- `dashflow-annoy/src/lib.rs` (2 occurrences)
- `dashflow-usearch/src/lib.rs` (2 occurrences)
- `dashflow-sqlitevss/src/lib.rs` (2 occurrences)

**Fix:** Replace with working stub implementations or mark with `#[doc(hidden)]`

### 3.2 ConsistentFakeEmbeddings in Examples
**Files:**
- `dashflow-chroma/examples/chroma_validation.rs`
- `dashflow-qdrant/examples/qdrant_validation.rs`

**Issue:** Examples use fake embeddings, may confuse users
**Fix:** Add clear documentation that these are for demonstration only

### 3.3 MockEmbeddings in Documentation
**Files:**
- `dashflow-lancedb/src/lib.rs:24-34,62`
- `dashflow-neo4j/src/lib.rs:19,25`

**Fix:** Use real embeddings or clearly mark as example-only

---

## Priority 4: Security Considerations (Already Safe)

These have been verified as properly implemented:

### 4.1 SQL Injection Prevention
**Status:** SAFE
- `dashflow-postgres-checkpointer`: Uses `validate_identifier()` for table names
- `dashflow-sqlitevss`: Uses parameterized queries

### 4.2 Path Traversal Prevention
**Status:** SAFE
- `dashflow-file-tool`: Uses `canonicalize()` properly
- `dashflow-shell-tool`: Uses allowlists and sandboxing

### 4.3 Command Injection Prevention
**Status:** SAFE
- `dashflow-shell-tool`: Command allowlist, prefix restrictions, sandbox mode

---

## Verified Safe: No Action Needed

These were flagged initially but verified as safe:

| File | Issue | Resolution |
|------|-------|------------|
| `executor.rs` | 221 .unwrap() | All in test/doc code |
| `qdrant.rs` | 184 .unwrap() | Uses safe patterns (unwrap_or*) |
| `runnable.rs` | 123 .unwrap() | All in test/doc code |
| `platform_registry.rs` | 71 .unwrap() | All in test/doc code |
| `token_buffer.rs` | Guarded unwrap | Safe: guarded by len() check |
| `conversation_entity.rs:539` | unimplemented! | In #[cfg(test)] module |
| `unsafe` blocks | Pin, LMDB | Proper SAFETY comments |

---

## Action Items Summary

### ✅ COMPLETE - Bug Fixes (Phases 381-382):
1. [x] **Phase 381**: Add timeout to `wait_for_run()` - DONE (#870)
2. [x] **Phase 382**: FAISS mutex handling - DONE (#870, crate excluded)

### ✅ COMPLETE - Database Testcontainers (Phases 383-385):
3. [x] **Phase 383**: PostgreSQL testcontainers - DONE (#871)
4. [x] **Phase 384**: Redis Stack testcontainers - DONE (#871)
5. [x] **Phase 385**: Cassandra testcontainers - DONE (#871)

### ✅ COMPLETE - Additional Database Testcontainers (Phases 386-388):
6. [x] **Phase 386**: ChromaDB testcontainers (28 tests) - DONE (#875)
7. [x] **Phase 387**: DynamoDB LocalStack (9 tests) - DONE (#875)
8. [x] **Phase 388**: S3 LocalStack (5 tests) - DONE (#875)

### ✅ COMPLETE - API Mock Servers (Phases 389-391):
9. [x] **Phase 389**: OpenAI API mock server (30 tests) - DONE (#875)
10. [x] **Phase 390**: HuggingFace API mock server (32 tests) - DONE (#875)
11. [x] **Phase 391**: Together AI mock server (8 tests) - DONE (#875)

### ✅ COMPLETE/MOOT - Infrastructure (Phases 392-393):
12. [x] **Phase 392**: `.env.test` template - DONE (#871)
13. [x] **Phase 393**: CI job - MOOT (no GitHub Actions)

### ✅ COMPLETE - Doc Example Cleanup (Phases 394-399):
14. [x] **Phase 394**: `dashflow-redis/src/lib.rs` (3 occurrences) - DONE (#872)
15. [x] **Phase 395**: `dashflow-supabase/src/lib.rs` - DONE (#872)
16. [x] **Phase 396**: `dashflow-clickhouse/src/lib.rs` - DONE (#872)
17. [x] **Phase 397**: `dashflow-qdrant/src/lib.rs`, `src/qdrant.rs` (9 occurrences) - DONE (#872)
18. [x] **Phase 398**: `dashflow-weaviate/src/lib.rs`, `src/weaviate.rs` - DONE (#872)
19. [x] **Phase 399**: `dashflow-annoy`, `dashflow-usearch`, `dashflow-sqlitevss` - DONE (#872)

### ✅ COMPLETE - Example Documentation (Phases 400-401):
20. [x] **Phase 400**: Document ConsistentFakeEmbeddings in examples - DONE (#872)
21. [x] **Phase 401**: Document MockEmbeddings in lib.rs examples - DONE (#872)

---

## Progress Summary

| Category | Phases | Done | Remaining |
|----------|--------|------|-----------|
| Bug Fixes | 381-382 | 2 | 0 |
| Database Testcontainers | 383-388 | 6 | 0 |
| API Mock Servers | 389-391 | 3 | 0 |
| Infrastructure | 392-393 | 2 | 0 |
| Doc Example Cleanup | 394-399 | 6 | 0 |
| Example Documentation | 400-401 | 2 | 0 |
| **Total** | **381-401** | **21** | **0** |

**Status:** ALL PHASES COMPLETE ✅
