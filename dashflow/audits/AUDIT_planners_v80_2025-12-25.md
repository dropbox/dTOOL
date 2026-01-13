# Audit Report: self_improvement/planners.rs (v80)

**Date:** 2025-12-25
**Auditor:** Worker (next iteration: #1758)
**File:** `crates/dashflow/src/self_improvement/planners.rs`
**Lines:** ~1467
**Commit:** (uncommitted)

---

## Overview

The `planners.rs` module provides the plan lifecycle for the self-improvement system:

1. **Plan generation**
   - `PlanGenerator` converts analyses (capability gaps, deprecations, retrospectives) into `ExecutionPlan`s.
2. **Plan validation**
   - `PlanValidator` scores and filters plans using `ConsensusResult` reviews/critiques.
3. **Plan tracking**
   - `PlanTracker` persists plan status transitions and provides summaries/staleness detection.

---

## Issues Found and Fixed

### M-945 (P2): `insight_to_plan()` could panic on non-ASCII insights

**Location:** `planners.rs` (`insight_to_plan`)

**Problem:** The title snippet used `&insight[..N]` byte slicing. For UTF-8 strings, indexing by byte length can panic if the cutoff is not on a character boundary.

**Fix:** Build the snippet via `insight.chars().take(50).collect::<String>()`, which is UTF-8 safe.

---

### M-946 (P2): `PlanTracker` status transitions lost metadata and didn’t match storage directory semantics

**Location:** `planners.rs` (`PlanTracker::{mark_in_progress, mark_implemented, mark_failed, mark_superseded, in_progress_plans, summary, stale_plans}`)

**Problems:**
1. `mark_implemented/mark_failed/mark_superseded` updated the in-memory `plan.status` but then called `IntrospectionStorage::move_plan_to_*` which reloaded the plan from disk (old status), discarding `commit_hash`, failure reason, and `superseded_by`.
2. `mark_in_progress` updated status in-place without moving the plan to the `approved/` directory, while `IntrospectionStorage::save_plan` treats `InProgress` plans as `approved/`.
3. Summary and stale computations only consulted `pending/`, which would miss `approved/` plans if status transitions were done via storage APIs.

**Fix:**
- Use the storage lifecycle APIs for transitions:
  - `approve_plan` for in-progress
  - `complete_plan` for implemented (preserves commit hash)
  - `fail_plan` for failed (preserves reason)
  - `update_plan_status` for superseded
- Update `in_progress_plans`, `summary`, and `stale_plans` to consult `approved/` (and filter by `InProgress` defensively).
- Add tests to verify these transitions preserve metadata.

---

### M-947 (P4): Step ordering used `steps.len() as u8 + 1` (overflow risk if step list grows)

**Location:** `planners.rs` (`generate_gap_steps`, `generate_deprecation_steps`)

**Problem:** Casting `usize` → `u8` can truncate for large step lists, and `+ 1` can overflow.

**Fix:** Add `next_step_order()` using `u8::try_from(len)` with `saturating_add(1)` and `u8::MAX` fallback.

---

## Test Results

Ran:

`cargo test -p dashflow --lib planners`

All `self_improvement::planners` tests passed, including new PlanTracker lifecycle tests.

---

## Summary

| Priority | Count | Status |
|----------|-------|--------|
| P2 | 2 | ✅ FIXED (M-945, M-946) |
| P4 | 1 | ✅ FIXED (M-947) |
