# v26 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

**Date:** 2025-12-24
**Auditor:** Manager (v26 deep skeptical audit)
**Prior Audits:** v25 (M-774), v24 (M-762..M-773), v23 (M-749..M-760), v22 (M-739..M-748), v21 (M-719..M-738)
**Scope:** `observability-ui/`, state hash verification, race conditions
**Method:** Deep skeptical source code analysis — actual source, not documentation

---

## Executive Summary

This v26 audit continues the deep skeptical source code analysis. Found **1 NEW P2 issue** (M-775) - a race condition in state hash verification that can cause false negatives (missing corruption detection).

Also verified that **M-774 was already fixed** in commit #1663.

---

## New Issue (M-775)

### M-775 (P2): stateHash.ts `unsafeNumberDetected` global variable race condition
**Category:** Race Condition / Hash Verification
**Severity:** P2 (can cause false negatives in corruption detection)

**Problem:** The `unsafeNumberDetected` variable in `stateHash.ts:5` is a **module-level mutable global** that gets modified during `canonicalJsonString()` (line 19) and read in `computeStateHash()` (line 69). When multiple runs compute hashes concurrently, this shared state creates a race condition.

**File:** `observability-ui/src/utils/stateHash.ts:5,19,63,69`

```typescript
// Line 5 - Global mutable state (BAD)
let unsafeNumberDetected = false;

// Line 19 - Modified during serialization
if (Math.abs(value) > Number.MAX_SAFE_INTEGER) {
  unsafeNumberDetected = true;
}

// Lines 63,69 - Reset and read in computeStateHash
unsafeNumberDetected = false;           // Line 63
const canonical = canonicalJsonString(state);
const hasUnsafeNumbers = unsafeNumberDetected;  // Line 69
```

**Race Scenario:**
1. Thread A (run-1): `unsafeNumberDetected = false` (line 63)
2. Thread A: starts `canonicalJsonString(stateA)`, sets `unsafeNumberDetected = true` (has large number)
3. Thread B (run-2): `unsafeNumberDetected = false` (line 63) — **OVERWRITES Thread A's flag!**
4. Thread A: captures `hasUnsafeNumbers = unsafeNumberDetected` — now `false` (WRONG!)
5. Thread A: Computes hash, returns `hasUnsafeNumbers: false` — **false negative**
6. Result: Run-1's state hash is compared despite having unsafe numbers → **false corruption detection**

**Impact:**
- With concurrent state updates across multiple runs, `hasUnsafeNumbers` can incorrectly report `false` when the state actually contained large numbers.
- This causes hash verification to proceed when it should be skipped, triggering false corruption flags.
- The existing comment at line 68 acknowledges "concurrent calls" but the fix only addresses the async portion, not the synchronous race during `canonicalJsonString`.

**Note:** M-719 serializes hash verification PER RUN via `hashVerificationChain`, but different runs can still compute hashes concurrently, triggering this race.

**Fix direction:**
1. **Option A (Preferred):** Pass a context object through `canonicalJsonString` to track unsafe numbers:
   ```typescript
   interface SerializationContext {
     unsafeNumberDetected: boolean;
   }

   function canonicalJsonString(value: unknown, ctx: SerializationContext): string {
     // ... modify ctx.unsafeNumberDetected instead of global
   }

   export async function computeStateHash(state: Record<string, unknown>): Promise<StateHashResult> {
     const ctx: SerializationContext = { unsafeNumberDetected: false };
     const canonical = canonicalJsonString(state, ctx);
     // ...
     return { hash, hasUnsafeNumbers: ctx.unsafeNumberDetected };
   }
   ```

2. **Option B:** Use a closure to capture the flag per-call.

**Acceptance criteria:**
- Add a test that calls `computeStateHash` concurrently on two states where one has unsafe numbers and one doesn't.
- Verify each result independently reports the correct `hasUnsafeNumbers` value.

---

## Verification: M-774 FIXED

**M-774** (binary processing chain timeout) was fixed in commit #1663:

- **Lines 161-180:** Added `withTimeout<T>()` helper function
- **Lines 1257-1261:** Wrapped `ensureDecoderReady()` with 30s timeout
- **Lines 1268-1272:** Wrapped `blob.arrayBuffer()` with 30s timeout
- **Constant:** `BINARY_PROCESSING_STEP_TIMEOUT_MS = 30_000` (line 163)

The fix is correct and complete.

---

## Summary Table

| ID | Priority | Category | Description | Status |
|----|----------|----------|-------------|--------|
| **M-775** | P2 | Race | stateHash.ts global `unsafeNumberDetected` race condition | **NEW** |
| ~~M-774~~ | ~~P3~~ | ~~Perf~~ | ~~Binary processing timeout~~ | ✅ FIXED #1663 |

---

## Current State

**Stats after v26:** 0 P0 | 0 P1 | 1 P2 (M-775) | ~84 backlog P3/P4

All streaming/graph-state P1s from v21-v24 remain fixed. The codebase has one remaining P2 correctness issue (M-775) that should be fixed for true production readiness.
