# v84 Skeptical Audit: export_import.rs

**Date:** 2025-12-25
**Scope:** `crates/dashflow/src/self_improvement/export_import.rs` (933 lines; was 895 at audit time)
**Auditor:** Worker #1761

## Summary

Export/Import API for introspection data (reports, plans, hypotheses) as JSON archives.
No P0/P1/P2 issues found. Four P4 issues identified and fixed.

## Issues Found and Fixed

### M-951 (P3 → FIXED): Silent export failures

**Problem:** During export, `load_report()` errors were silently ignored using `if let Ok()` pattern. Users had no indication their export might be incomplete due to corrupted files.

**Fix:** Added `warn!` logging with report ID and error when load fails:
```rust
match storage.load_report(id) {
    Ok(report) => archive.reports.push(report),
    Err(e) => {
        warn!(report_id = %id, error = %e, "Failed to load report during export - skipping");
    }
}
```

Also fixed plan loading to log warnings when `pending_plans()`, `approved_plans()`, etc. fail.

### M-952 (P4 → FIXED): Clippy allows too broad

**Problem:** Module-level `#![allow(clippy::expect_used, clippy::unwrap_used, ...)]` applied to entire module but only test code uses `unwrap()`.

**Fix:** Removed module-level allows; added `#[allow(clippy::expect_used, clippy::unwrap_used)]` to `#[cfg(test)]` module only.

### M-953 (P4 → FIXED): `original_size_bytes` never populated

**Problem:** `ArchiveMetadata::original_size_bytes` field defaulted to 0 and was never set in `update_metadata()`.

**Fix:** Added size estimation in `update_metadata()` using JSON serialization of each item:
```rust
let mut size_estimate: u64 = 0;
for report in &self.reports {
    size_estimate += serde_json::to_string(report)
        .map(|s| s.len() as u64)
        .unwrap_or(0);
}
// ... same for plans and hypotheses
self.metadata.original_size_bytes = size_estimate;
```

### M-954 (P4 → FIXED): Validation behavior undocumented

**Problem:** Validation errors only abort import when `conflict_resolution = Fail`. With Skip/Overwrite, errors are recorded but import continues. This could surprise users.

**Fix:** Added comprehensive documentation to `ImportConfig` explaining the behavior:
```rust
/// # Validation Behavior
///
/// When `validate = true`, the archive is validated for structural issues...
/// **Important:** Validation errors only abort the import when `conflict_resolution = Fail`.
/// With `Skip` or `Overwrite` conflict resolution, validation errors are recorded but the
/// import continues...
```

## Positive Observations

1. **Clean architecture**: Good separation of concerns (Archive, ExportConfig, ImportConfig, ImportResult)
2. **Builder pattern**: Fluent API for configuration (`ExportConfig::reports_only().with_filter(...)`)
3. **Conflict handling**: Three modes (Skip, Overwrite, Fail) cover common use cases
4. **Dry run support**: Can preview import without actually modifying storage
5. **Version checking**: Rejects archives from newer versions with clear error
6. **Good test coverage**: 11 tests covering roundtrip, conflicts, dry run, version check

## Test Results

```
running 11 tests
test self_improvement::export_import::tests::test_archive_creation ... ok
test self_improvement::export_import::tests::test_archive_serialization ... ok
test self_improvement::export_import::tests::test_export_config_builders ... ok
test self_improvement::export_import::tests::test_export_with_filter ... ok
test self_improvement::export_import::tests::test_import_result_totals ... ok
test self_improvement::export_import::tests::test_export_import_roundtrip ... ok
test self_improvement::export_import::tests::test_import_skip_existing ... ok
test self_improvement::export_import::tests::test_import_overwrite_existing ... ok
test self_improvement::export_import::tests::test_import_dry_run ... ok
test self_improvement::export_import::tests::test_conflict_resolution_fail ... ok
test self_improvement::export_import::tests::test_archive_version_check ... ok

test result: ok. 11 passed; 0 failed
```

## Files Modified

- `crates/dashflow/src/self_improvement/export_import.rs`
