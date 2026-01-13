# Audit: Miscellaneous Crates

**Status:** IN PROGRESS - Multiple crates verified SAFE (#1423)
**Priority:** P3 (Various)

This file covers remaining miscellaneous crates.

---

## dashflow-compression

### Files
- [ ] `src/lib.rs`

### Known Issues
- `src/lib.rs`: ✅ SAFE (verified #1423) - test module starts at line 255; doc example at line 88; all other .unwrap()/panic! (lines 267-375) are in `#[cfg(test)]` module.

---

## dashflow-context

### Files
- [ ] `src/lib.rs`

### Known Issues
- No specific issues found

---

## dashflow-derive

### Files
- [ ] `src/lib.rs`

### Known Issues
- `src/lib.rs`: ✅ SAFE (verified #1423) - proc-macro crate (compile-time code). Lines 4-5 document SAFETY: `field.ident.unwrap()` (named fields always have idents), `to_lowercase().next().unwrap()` (always produces char). Failures occur at compile-time, not runtime.

---

## dashflow-factories

### Files
- [ ] `src/lib.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/llm.rs`
- [ ] `src/tools.rs`

### Known Issues
- `src/tools.rs`: ✅ SAFE (verified #1423) - test module starts at line 203; `.unwrap()` at line 230 is in `#[cfg(test)]` module.

---

## dashflow-file-management

### Files
- [x] `src/lib.rs`
- [x] `src/toolkit.rs`
- [x] `src/tools.rs`
- [x] `src/utils.rs`

### Known Issues
- ✅ SAFE (verified #1425) - All production code is safe:
  - `src/lib.rs`: Crate-level `#![allow(clippy::unwrap_used)]` but NO actual unguarded unwraps in production code.
  - `src/toolkit.rs`: NO `.unwrap()` or `.expect()` calls in production code. Only safe patterns (`.filter_map()`, `.cloned()`).
  - `src/tools.rs`: Lines 165, 480, 583 use safe `.unwrap_or()` patterns. All other `.unwrap()` (lines 651+) in `#[cfg(test)]` module.
  - `src/utils.rs`: Lines 57, 69 use safe `.unwrap_or_else()` patterns. All other `.unwrap()` (lines 116+) in `#[cfg(test)]` module.
- Tests: 26 unit tests + 6 doc tests all pass.

---

## dashflow-macros

### Files
- [ ] `src/lib.rs`

### Known Issues
- `src/lib.rs`: ✅ SAFE (verified #1423) - proc-macro crate (compile-time code). Lines 4-5 document SAFETY: `field.ident.unwrap()` (named fields always have idents), `inputs.first().unwrap()` (validated input count). Failures occur at compile-time, not runtime.

---

## dashflow-module-discovery

### Files
- [ ] `src/lib.rs`

### Known Issues
- `src/lib.rs`: ✅ SAFE (verified #1423) - test module starts at line 1735; all `.unwrap()` and `panic!` calls in test module are in `#[cfg(test)]` module.
- Contains TODO detection code (lines 318-319, 484-485) - production code, no panics.

---

## dashflow-neo4j

### Files
- [ ] `src/lib.rs` (docs reference MockEmbeddings!)
- [ ] `src/graph_store.rs`
- [ ] `src/neo4j_graph.rs`
- [ ] `src/neo4j_vector.rs`

### Known Issues
- `src/lib.rs:19,25`: MockEmbeddings in documentation
- `src/neo4j_graph.rs:247`: Note about #[ignore] tests

---

## dashflow-openapi

### Files
- [ ] `src/lib.rs`

### Known Issues
- No specific issues found

---

## dashflow-project

### Files
- [ ] `src/lib.rs`
- [ ] `src/discovery.rs`
- [ ] `src/documentation.rs`
- [ ] `src/languages.rs`

### Known Issues
- `src/discovery.rs`: ✅ SAFE (verified #1423) - test module starts at line 706; all `.unwrap()` in test module are in `#[cfg(test)]` module.
- `src/documentation.rs`: ✅ SAFE (verified #1423) - test module starts at line 185. Line 133 uses safe pattern: loads `self.content = Some(...)` before unwrap, guaranteed Some after is_none() check.

---

## dashflow-prometheus-exporter

### Files
- [ ] `src/main.rs`

### Known Issues
- No specific issues found

---

## dashflow-prompts

### Files
- [ ] `src/lib.rs`

### Known Issues
- `src/lib.rs`: ✅ SAFE (verified #1423) - test module starts at line 979. Line 844 uses safe pattern: `is_none() || x.unwrap()` (short-circuit evaluation guards unwrap). All other `.unwrap()` in test module are in `#[cfg(test)]` module.

---

## dashflow-remote-node

### Files
- [ ] `src/client.rs`
- [ ] `src/server.rs`

### Known Issues
- `src/server.rs`: ✅ FIXED #1424 - RwLock poison risks fixed with `.unwrap_or_else(|e| e.into_inner())` pattern.
  - **FIXED:** All RwLock reads/writes now use poison-safe pattern.
  - **SAFE patterns:** `node.unwrap()` guarded by preceding `is_none()` check with early return.
  - **Test code (lines 573+):** All remaining `.unwrap()/panic!` are in `#[cfg(test)]` module.
- `src/client.rs`: ✅ SAFE (verified #1423) - test module starts at line 502; all `.unwrap()` in test module are in `#[cfg(test)]` module.

---

## dashflow-voyage

### Files
- [ ] `src/lib.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/rerank.rs`

### Known Issues
- `src/rerank.rs`: ✅ SAFE (verified #1423) - test module starts at line 368; all `.unwrap()` in test module are in `#[cfg(test)]` module.
- `src/embeddings.rs`: ✅ SAFE (verified #1423) - test modules start at lines 420, 511; all `.unwrap()` in test modules are in `#[cfg(test)]` modules.

---

## dashflow-jina

### Files
- [ ] `src/lib.rs`
- [ ] `src/config_ext.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/rerank.rs`

### Known Issues
- `src/embeddings.rs`: ✅ SAFE (verified #1423) - test modules start at lines 466, 593; all `.unwrap()` in test modules are in `#[cfg(test)]` modules.
- `src/rerank.rs`: ✅ SAFE (verified #1423) - test module starts at line 278; all `.unwrap()` in test module are in `#[cfg(test)]` module.

---

## dashflow-nomic

### Files
- [ ] `src/lib.rs`
- [ ] `src/embeddings.rs`

### Known Issues
- `src/embeddings.rs`: ✅ SAFE (verified #1423) - test module starts at line 390; all `.unwrap()` in test module are in `#[cfg(test)]` module.

---

## Critical Checks (Apply to ALL)

1. **No mocks in production** - Verify test-only
2. **Error handling** - Complete
3. **Resource cleanup** - No leaks

---

## Test Coverage Gaps

- [ ] Error handling tests
- [ ] Resource cleanup tests
