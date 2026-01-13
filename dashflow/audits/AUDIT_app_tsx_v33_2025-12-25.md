# DashFlow v33 Skeptical Audit: App.tsx WebSocket Client
**Auditor:** Worker #1712
**Date:** 2025-12-25 (updated 2026-01-01)
**Scope:** `observability-ui/src/App.tsx`, `observability-ui/src/proto/dashstream.ts`, `observability-ui/src/utils/jsonPatch.ts`, `observability-ui/src/utils/timeFormat.ts`
**Previous Audit:** v32 (DashStream callback producer)

> **⚠️ LINE REFERENCES UPDATED:** This audit's line numbers were updated 2026-01-01 to match current code. Both M-814 and M-815 have been FIXED.

---

## Executive Summary

This audit examined the observability UI's WebSocket client (App.tsx), the protocol decoder (dashstream.ts), and the JSON patch utilities (jsonPatch.ts). After exhaustive review, **no P0, P1, or significant P2 issues were found**. The code has been comprehensively hardened through 32 prior skeptical audits.

| ID | Priority | Status | Description |
|----|----------|--------|-------------|
| M-814 | P4 | ✅ **FIXED** | formatUptime now validates negative/invalid input (moved to utils/timeFormat.ts:1-16) |
| M-815 | P4 | ✅ **FIXED** | evictOldestEntries type refined to `T extends string` (App.tsx:170-206) |

**Conclusion:** v33 audit COMPLETE - all issues now resolved.

---

## Audit Methodology

1. Examined WebSocket connection handling, cursor management, and reconnection logic
2. Reviewed binary message processing chain and timeout handling
3. Verified BigInt usage for sequences/offsets (M-693, M-707, M-763)
4. Checked error handling patterns and cleanup in useEffect hooks
5. Reviewed protocol decoder for edge cases in header detection
6. Verified JSON patch security (M-710 prototype pollution prevention)

---

## Prior Fixes Verified

The following fixes from previous audits were confirmed to be correctly implemented:

### App.tsx (50+ prior fixes)
- **M-451/M-459**: Reconnect timeout cleanup and exponential backoff
- **M-674/M-675/M-676**: Kafka partition/offset resume with proper commit-after-apply
- **M-693/M-707/M-763**: BigInt sequences/offsets with validation
- **M-720/M-723**: Cursor-binary pairing protocol error detection
- **M-774**: Binary processing timeout with reconnect recovery
- **M-678/M-727**: LRU eviction for localStorage with partition 0 protection
- **M-789**: Messages/sec rate calculation with counter reset detection
- **M-792**: Health polling timeout and in-flight guard
- **M-711**: Gap/stale cursor triggers resync marking
- **M-744/M-745**: Cursor reset protocol for recovery

### dashstream.ts
- **M-453**: Robust header detection with fallback to legacy format
- **M-469**: Safe BigInt to Number conversion with overflow warning
- **M-693/M-751**: String sequences with seq=0 support
- **M-685**: Kafka cursor passed to EventBatch inner events

### jsonPatch.ts
- **M-708**: RFC6902-correct array semantics (add=splice, replace=assignment)
- **M-709**: Array index validation with bounds checking
- **M-710**: Prototype pollution prevention
- **M-733**: Deep equality for test operations
- **M-740**: No fallback to string on JSON parse failure
- **M-808**: CloneError thrown instead of returning original

---

## Findings

### M-814 (P4): formatUptime doesn't validate negative input - ✅ FIXED

**File:** `utils/timeFormat.ts:1-16` (was `App.tsx:2833-2839`)

**Fix Applied:**
```typescript
export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s';  // M-814 FIX
  // ...
}
```

**Resolution:** formatUptime was extracted to a dedicated module and now validates both negative and non-finite inputs. Tests added in `__tests__/timeFormat.test.ts:56-57`.

---

### M-815 (P4): evictOldestEntries type parameter imprecise - ✅ FIXED

**File:** `App.tsx:170-206` (was 130-166)

**Fix Applied:**
```typescript
function evictOldestEntries<T extends string>(  // M-815 FIX: was string | number
  entries: Record<string, T>,
  maxEntries: number,
  _compareFn: (a: T, b: T) => number,  // Legacy param for compat
  protectedKeys: string[] = [],
  updatedAt?: Record<string, number>   // M-1058: timestamp for true LRU
): Record<string, T> {
```

**Resolution:** Type parameter refined to `T extends string` as recommended. Additionally, the function now supports timestamp-based LRU eviction (M-1058).

---

## Files Audited

| File | Lines (original → current) | Issues Found |
|------|---------------------------|--------------|
| `App.tsx` | 2842 → 3329 | M-814 (P4) ✅, M-815 (P4) ✅ |
| `dashstream.ts` | 578 → 850 | None |
| `jsonPatch.ts` | 540 → 630 | None |
| `timeFormat.ts` | (new) 18 | M-814 fix location |

---

## Security Checklist

| Category | Status | Notes |
|----------|--------|-------|
| XSS Prevention | ✅ | React JSX escaping, no dangerouslySetInnerHTML |
| Prototype Pollution | ✅ | M-710 blocks __proto__/constructor/prototype paths |
| BigInt Overflow | ✅ | Sequences/offsets stored as strings with BigInt comparison |
| localStorage Limits | ✅ | M-678 LRU eviction prevents unbounded growth |
| WebSocket Security | ✅ | wss:// used for https:// origins |
| API Rate Limiting | ✅ | M-792 in-flight guard prevents overlapping requests |

---

## Recommendations

Given the maturity of this code (32 prior audits, 50+ fixes), consider:

1. **Freeze UI Code**: App.tsx, dashstream.ts, and jsonPatch.ts are production-ready
2. **Future Audits**: Focus on server-side code and new features
3. **Documentation**: Consider adding CHANGELOG.md for UI changes

---

## Change Log

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-25 | Worker #1712 | Initial v33 audit - no critical issues |
| 1.1 | 2026-01-01 | Worker #2250 | M-814, M-815 marked FIXED; line refs updated (files grew significantly) |
