# Audit: self_improvement/storage/mod.rs

**Auditor:** Worker v73
**Date:** 2025-12-25
**File:** `crates/dashflow/src/self_improvement/storage/mod.rs`
**Lines:** ~2,658 (was ~2,600 at audit time; +58 lines)
**Line refs updated:** 2026-01-01 by Worker #2255

## Summary

Audited the core storage module for the self-improvement system. The module handles persistence of introspection reports, execution plans, and hypotheses. Found 5 P4 issues related to inconsistencies between sync and async methods - async variants were missing versioned storage, metrics recording, and plan index updates.

## Issues Found

### P4 Issues (Fixed)

| ID | Category | Description | Location |
|----|----------|-------------|----------|
| M-923 | Consistency | Async report methods (`save_report_async`, `load_report_async`, `latest_report_async`) didn't use versioned storage or record metrics | Lines 1810-1879 (was 1780-1878) |
| M-924 | Consistency | Async plan methods (`save_plan_async`, `load_plan_async`, `list_plans_in_dir_async`) didn't use versioned storage, record metrics, or update plan index | Lines 1916-1997 (was 1881-1990) |
| M-925 | Consistency | Async hypothesis methods (`save_hypothesis_async`, `load_hypothesis_async`) didn't use versioned storage or record metrics | Lines 2028-2063 (was 1993-2053) |
| M-926 | Consistency | `update_plan` method didn't use versioned storage | Line 1416 (was 1420) |
| M-927 | Consistency | `move_plan_to_implemented` and `move_plan_to_failed` didn't use versioned storage or update plan index | Lines 1464-1508 (was 1468-1525) |

### No P0/P1/P2/P3 Issues Found

The module is well-designed with:
- Proper error handling throughout
- Comprehensive storage policy and health monitoring
- Plan index for O(1) lookups
- Schema migration support via `SchemaMigrator`
- Both sync and async APIs for all operations

## Fixes Applied

1. **M-923 (save_report_async):** Added `SchemaMigrator::save_versioned()` when versioned storage is enabled, and `record_storage_operation("save", "report")` call.

2. **M-923 (load_report_async):** Replaced direct `from_json()` with `self.parse_report()` which handles versioned parsing, and added metrics recording.

3. **M-923 (latest_report_async):** Replaced direct `from_json()` with `self.parse_report()` for versioned parsing.

4. **M-924 (save_plan_async):** Added versioned storage, plan index update, and metrics recording to match sync version.

5. **M-924 (load_plan_async):** Replaced direct `serde_json::from_str()` with `self.parse_plan()` and added metrics recording.

6. **M-924 (list_plans_in_dir_async):** Replaced direct `serde_json::from_str()` with `self.parse_plan()` for versioned parsing.

7. **M-925 (save_hypothesis_async):** Added versioned storage and metrics recording.

8. **M-925 (load_hypothesis_async):** Replaced direct `serde_json::from_str()` with `self.parse_hypothesis()` and added metrics recording.

9. **M-926 (update_plan):** Added versioned storage check to match `save_plan` behavior.

10. **M-927 (move_plan_to_implemented/failed):** Added versioned storage and plan index update to match `save_plan` behavior.

## Verification

```bash
cargo check -p dashflow  # Compiles successfully
```

## Test Coverage

The module has comprehensive tests in `storage/tests.rs` (~1,021 lines) covering:
- Report save/load operations
- Plan lifecycle (save, load, status transitions, indexing)
- Hypothesis save/load operations
- Storage policy enforcement
- Health monitoring
- Batch operations

## Architecture Notes

The storage module follows a well-designed pattern:
1. **Versioned Storage:** `SchemaMigrator` handles saving data with schema version and migrating older formats on load
2. **Plan Index:** O(1) plan lookups via `PlanIndex` stored in `plans/index.json`
3. **Metrics Integration:** All operations record metrics via `record_storage_operation()`
4. **Dual APIs:** Both sync and async variants for all operations

The fixes ensure consistency between sync and async paths, preventing potential issues where:
- Async-saved data wouldn't have schema versions (breaking migrations)
- Async operations wouldn't contribute to metrics
- Async plan saves wouldn't update the index (breaking O(1) lookups)

## Recommendations

No further action needed. All P4 issues have been fixed.
