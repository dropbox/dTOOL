# v34 Skeptical Audit: useRunStateStore.ts

**Date:** 2025-12-25 (updated 2026-01-01 by #2254)
**Scope:** `observability-ui/src/hooks/useRunStateStore.ts` (2007 → 2558 lines)
**Prior Audits:** v21-v33 covered related files; useRunStateStore last thorough audit v29

## Summary

**ALL ISSUES FIXED.** The code is extensively hardened after 50+ fixes across v18-v33 audits. Both P4 defensive coding suggestions from this audit have been addressed.

## Audit Methodology

1. Read entire file line-by-line
2. Analyzed error handling paths and edge cases
3. Verified all M-* fixes from prior audits are intact
4. Checked for race conditions, type mismatches, and logic errors
5. Reviewed BigInt/sequence handling for precision issues

## Findings

### ~~M-816 (P4): coerceU64ToString doesn't wrap toNumber() in try-catch~~ ✅ FIXED

**Original Location:** `useRunStateStore.ts:111-117`
**Category:** Defensive Coding

**Status:** ✅ FIXED
- M-1109 removed `coerceU64ToString` entirely (replaced by `coerceU64ToStr` from dashstream.ts)
- New code at lines 1037-1049 handles Long-like objects with proper try-catch:
```typescript
try {
  const parsed = maybeToNumber.call(innerTimestampUs);
  if (typeof parsed === 'number') {
    usValue = parsed;
  }
} catch {
  // Long.toNumber() threw (overflow), fall back to batch timestamp
}
```

**Verified by:** #2254 - Lines 45-46 document removal, lines 1041-1048 show try-catch.

---

### ~~M-817 (P4): innerTimestamp fallback treats 0 as falsy~~ ✅ FIXED

**Original Location:** `useRunStateStore.ts:1273-1276`
**Current Location:** `useRunStateStore.ts:1024-1053`
**Category:** Edge Case

**Status:** ✅ FIXED - Comment at lines 1026-1027 documents fix:
```typescript
// M-817: Use explicit checks to preserve timestamp=0 (Unix epoch) instead of || fallback.
```

Now uses explicit undefined check (line 1052):
```typescript
if (usValue !== undefined && Number.isFinite(usValue)) {
  const msValue = Math.floor(usValue / 1000);
```

**Verified by:** #2254 - Lines 1024-1053 show refactored timestamp handling.

---

## Verified Hardening

The following fixes from prior audits are correctly implemented:

| M-ID | Description | Status |
|------|-------------|--------|
| M-693 | String sequences for BigInt precision | |
| M-704 | patchApplyFailed tracking + recovery | |
| M-715 | Checkpoint eviction limits | |
| M-719 | hashVerificationChain serialization | |
| M-721 | checkpointsById coherent eviction | |
| M-725/726 | nodeStates/observedNodes trimming | |
| M-730 | patchApplyFailedSeq tracking | |
| M-770/771 | Snapshot/checkpoint parse error handling | |
| M-776/777 | Skip patches when needsResync + stateApplied guard | |
| M-783/738 | Size limits for fullState/checkpoint | |
| M-786 | isRealSeq accepts seq >= 0 | |
| M-787 | lastAppliedSeq out-of-order detection | |
| M-788 | markActiveRunsNeedResync marks all runs | |
| M-798 | Reject state mutations with synthetic seq | |
| M-799/800 | Record all message types + messageId dedupe | |
| M-806 | Clear corrupted on snapshot/checkpoint recovery | |
| M-808 | CloneError handling throughout | |

## Code Quality Observations

1. **Extensive error handling** - Every major operation has appropriate try-catch with CloneError handling
2. **BigInt consistency** - All sequence comparisons use `compareSeqs()` with BigInt
3. **State recovery paths** - Full state snapshots and checkpoints properly clear corruption flags
4. **Memory bounds** - Event trimming includes nodeStates, observedNodes, and checkpoint coherence
5. **Async safety** - Hash verification serialized via promise chain (M-719)

## Conclusion

This file has been hardened through 50+ fixes across 15 audit rounds. Both P4 items from this audit (M-816, M-817) have been fixed. **All issues resolved.**

---

**Auditor:** Worker #1714
**Time Spent:** ~15 minutes
