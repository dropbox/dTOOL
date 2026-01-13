# v85 Skeptical Audit: trace_retention.rs

**Date:** 2025-12-25
**Scope:** `crates/dashflow/src/self_improvement/trace_retention.rs` (801 lines; was 781 at audit time)
**Auditor:** Worker #1761
**Line refs updated:** 2026-01-01 by Worker #2255

## Summary

Trace retention policy for `.dashflow/traces/`. Provides configurable limits for count, age, size,
and automatic compression of old traces. No P0/P1/P2/P3 issues found. Four P4 issues identified and fixed.

## Issues Found and Fixed

### M-955 (P4 → FIXED): Silent skip of files with unreadable metadata

**Problem:** In `list_traces()`, files with unreadable metadata or modification time were silently skipped using `if let Ok()` pattern.

**Fix:** Changed to explicit match with `debug!` logging:
```rust
match entry.metadata() {
    Ok(metadata) => match metadata.modified() {
        Ok(modified) => { /* add to list */ }
        Err(e) => {
            debug!(path = %path.display(), error = %e, "Skipping trace file: cannot read modification time");
        }
    },
    Err(e) => {
        debug!(path = %path.display(), error = %e, "Skipping trace file: cannot read metadata");
    }
}
```

### M-956 (P4 → FIXED): Silent fallback to default on env var parse failure

**Problem:** `from_env()` used `.parse().ok()` which silently swallowed parse errors and used defaults without any indication to the user.

**Fix:** Added explicit error handling with `warn!` logging:
```rust
let max_traces = match std::env::var("DASHFLOW_TRACE_MAX_COUNT") {
    Ok(v) => match v.parse() {
        Ok(n) => Some(n),
        Err(e) => {
            warn!(var = "DASHFLOW_TRACE_MAX_COUNT", value = %v, error = %e,
                  default = DEFAULT_MAX_TRACES, "Failed to parse env var, using default");
            Some(DEFAULT_MAX_TRACES)
        }
    },
    Err(_) => Some(DEFAULT_MAX_TRACES),
};
```

### M-957 (P4 → FIXED): `to_string_lossy()` path extension check

**Problem:** Used `path.to_string_lossy().ends_with(".json.gz")` which could behave unexpectedly with non-UTF8 paths.

**Fix:** Changed to proper `OsStr` extension checking:
```rust
let is_compressed = path.extension() == Some(OsStr::new("gz"))
    && path
        .file_stem()
        .and_then(|s| Path::new(s).extension())
        == Some(OsStr::new("json"));
```

### M-958 (P4 → FIXED): No tracing/logging during cleanup operations

**Problem:** Cleanup operations (delete, compress) had no logging, making it hard to diagnose issues or monitor what the retention system is doing.

**Fix:** Added summary logging at end of cleanup:
```rust
if stats.deleted_count > 0 || stats.compressed_count > 0 {
    info!(
        deleted = stats.deleted_count,
        freed_bytes = stats.freed_bytes,
        compressed = stats.compressed_count,
        compression_saved_bytes = stats.compression_saved_bytes,
        // ... other fields
        "Trace retention cleanup completed"
    );
}
```

## Positive Observations

1. **Gzip bomb protection**: `decompress_trace()` limits output to 100MB using `.take()` - excellent security practice
2. **Environment configuration**: Follows `DASHFLOW_*` convention per DESIGN_INVARIANTS.md
3. **Non-blocking cleanup**: Individual file deletion errors are recorded in stats but don't fail the operation
4. **Builder pattern**: Clean fluent API for policy configuration
5. **Multiple limit types**: Count, age, size limits all supported and work together
6. **Compression support**: Automatic compression of old traces before deletion
7. **Good defaults**: 1000 traces, 30 days max age, 500MB size limit

## Test Results

```
running 10 tests
test self_improvement::trace_retention::tests::test_default_policy ... ok
test self_improvement::trace_retention::tests::test_unlimited_policy ... ok
test self_improvement::trace_retention::tests::test_policy_builders ... ok
test self_improvement::trace_retention::tests::test_empty_directory_stats ... ok
test self_improvement::trace_retention::tests::test_directory_stats ... ok
test self_improvement::trace_retention::tests::test_cleanup_by_count ... ok
test self_improvement::trace_retention::tests::test_cleanup_by_size ... ok
test self_improvement::trace_retention::tests::test_cleanup_disabled ... ok
test self_improvement::trace_retention::tests::test_needs_cleanup ... ok
test self_improvement::trace_retention::tests::test_nonexistent_directory ... ok

test result: ok. 10 passed; 0 failed
```

## Files Modified

- `crates/dashflow/src/self_improvement/trace_retention.rs`
