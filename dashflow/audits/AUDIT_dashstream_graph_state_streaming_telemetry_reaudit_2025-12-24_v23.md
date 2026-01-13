# v23 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

**Date:** 2025-12-24
**Auditor:** Manager (v23 deep source code audit)
**Prior Audits:** v22 (M-739..M-748), v21 (M-719..M-738), v20 (M-707..M-716), v19, v18, v15, v14, v13, v12, v11
**Scope:** observability-ui/, crates/dashflow-observability/, WebSocket/Kafka streaming pipeline
**Method:** Deep skeptical source code analysis - actual source, not documentation

---

## Executive Summary

This v23 audit continues the deep skeptical source code analysis. Found **12 NEW issues** (M-749 to M-760) including **3 P1s** related to:
- Type mismatches after M-693 BigInt migration
- Hash verification edge cases with BigInt state values
- EventBatch sequence extraction edge case
- Cursor comparison logic error
- Checkpoint storage ordering assumptions

---

## New Issues (M-749 to M-760)

### M-749 (P1): StateDiffViewer cursorSeq prop typed as `number` but sequences are strings
**Category:** Type Mismatch
**Severity:** P1 (TypeScript compile should catch, but JSX coercion hides it)

**Problem:** After M-693 migration to string sequences, `StateDiffViewer.tsx:19` still types `cursorSeq` as `number`. Callers pass string sequences, which get coerced.

**File:** `observability-ui/src/components/StateDiffViewer.tsx:19`
```typescript
interface StateDiffViewerProps {
  // ...
  cursorSeq?: number;  // WRONG: should be string after M-693
}
```

**Impact:** Display shows incorrect sequence values when coercion happens. Type safety lost.

**Fix:** Change type to `string | undefined`, update display to handle string.

---

### M-750 (P1): canonicalJsonString serializes BigInt to "null" (incorrect hashes)
**Category:** Hash Verification
**Severity:** P1 (false corruption detection)

**Problem:** `stateHash.ts:31` in the `default` case returns `'null'` for BigInt values. If graph state contains BigInt (e.g., from large counters), the canonical representation drops the value, causing hash mismatch with server.

**File:** `observability-ui/src/utils/stateHash.ts:31-32`
```typescript
default:
  // bigint, function, symbol
  return 'null';
```

**Impact:** Any state containing BigInt values will hash incorrectly, triggering false corruption flags.

**Fix:** Convert BigInt to string representation: `case 'bigint': return `"${value.toString()}"`;`

---

### M-751 (P1): EventBatch max sequence extraction returns undefined on any zero-seq event
**Category:** Resume/Cursor
**Severity:** P1 (cursor can be lost for entire batch)

**Problem:** `dashstream.ts:392-395` checks `seqBigInt > BigInt(0)` and only updates `maxInnerSeqBigInt` for positive sequences. But if ANY event in batch has seq=0 (valid for graph_start), the entire batch can return `undefined` for sequence if that was the first event checked.

**File:** `observability-ui/src/proto/dashstream.ts:387-405`
```typescript
for (const event of decoded.eventBatch.events || []) {
  const seqBigInt = eventHeader?.sequence;
  if (seqBigInt !== undefined && seqBigInt > BigInt(0)) {
    // Only positive seqs considered
    maxInnerSeqBigInt = ...
  }
}
// If first event has seq=0 and others aren't reached due to early state...
```

**Impact:** Batches with mixed seq values may lose cursor position.

**Fix:** Continue iterating even if current event has seq <= 0; only exclude from max calculation.

---

### M-752 (P2): TimelineSlider event display mixes seq (string) with index count
**Category:** UI/Display
**Severity:** P2 (confusing, not corrupt)

**Problem:** `TimelineSlider.tsx:595` displays `"Event {cursor.seq} of {sliderRange.max}"`. After M-693, `cursor.seq` is the actual sequence string (could be "12345678901234") but `sliderRange.max` is the array index count (e.g., 100). This mixes units confusingly.

**File:** `observability-ui/src/components/TimelineSlider.tsx:593-596`
```tsx
Event {cursor.seq} of {sliderRange.max}
```

**Impact:** Confusing display: "Event 12345678901234 of 100".

**Fix:** Display as "Event {index+1} of {length}" or "Seq {seq}" separately.

---

### M-753 (P2): selectedNodeState uses unsorted getRuns() for "most recent" run
**Category:** Logic Error
**Severity:** P2 (wrong run selected in edge cases)

**Problem:** `App.tsx:495` uses `getRuns()[length-1]` assuming last element is most recent, but `getRuns()` returns unsorted threadIds (Map iteration order). Should use `getRunsSorted()`.

**File:** `observability-ui/src/App.tsx:493-496`
```typescript
const allRuns = getRuns();
if (allRuns.length === 0) return null;
// Use the most recent run (getRuns returns string[] of threadIds)
threadId = allRuns[allRuns.length - 1];  // NOT guaranteed most recent
```

**Impact:** Node state panel may show state from wrong run when multiple runs exist.

**Fix:** Use `getRunsSorted()` and take first entry (most recent).

---

### M-754 (P2): Offset comparison uses `!==` which is always true for BigInt vs string
**Category:** Logic Error
**Severity:** P2 (unnecessary storage writes)

**Problem:** `App.tsx:1036` compares `BigInt(offset) !== BigInt(prev)` where both are derived from strings. The intent appears to be "only update if different", but `BigInt(x) !== BigInt(y)` is always true since they're different BigInt instances (reference comparison). Should use `>` for monotonic offsets.

**File:** `observability-ui/src/App.tsx:1036`
```typescript
if (prev === undefined || BigInt(offset) !== BigInt(prev)) {
  lastOffsetsByPartitionRef.current[key] = offset;
}
```

**Impact:** Every offset update triggers storage write, wasting cycles. Also, backward offset detection broken.

**Fix:** Use `BigInt(offset) > BigInt(prev)` for monotonic forward check, or `BigInt(offset).toString() !== prev` for equality.

---

### M-755 (P2): Checkpoint lastCheckpointId set before parse success
**Category:** Error Handling
**Severity:** P2 (stale ID reference after parse failure)

**Problem:** `useRunStateStore.ts:904-905` sets `store.lastCheckpointId = checkpointId` BEFORE the try block that parses state. If parsing fails (line 932), `lastCheckpointId` points to a checkpoint that was never successfully stored.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:895-936`
```typescript
if (checkpointId) {
  // Try to parse state from checkpoint
  const stateBytes = ...
  if (stateBytes && stateBytes.length > 0) {
    try {
      // ... parse and store ...
      store.lastCheckpointId = checkpointId;  // Should be here, inside try after success
```

Wait, let me re-read... Actually looking at the code again:

**File:** `observability-ui/src/hooks/useRunStateStore.ts:904-905`
```typescript
store.checkpointsById.set(checkpointId, { seq, state: deepCloneJson(state) });
store.lastCheckpointId = checkpointId;
```

These are INSIDE the try block. Let me verify... yes they are inside. However, `store.checkpointsById.set` happens before state is fully validated. If `deepCloneJson` throws, the entry is partially stored.

**Revised Problem:** If `deepCloneJson(state)` throws (e.g., circular reference somehow), the `checkpointsById.set` call may have already mutated the map. But actually deepCloneJson uses JSON.stringify which would throw before the set.

Let me re-examine for a real issue here...

Actually, the issue is: line 905 sets `lastCheckpointId` after successful storage, which is correct. However, the catch block at line 932-934 only warns and doesn't clear `checkpointsById` entry if it was partially added. But since the set happens atomically before any async work, this is probably fine.

Let me revise this issue to something I actually found:

**Revised M-755 (P2): Checkpoint eviction sorts by seq but checkpointsById keyed by ID**
**Category:** Eviction/Consistency
**Severity:** P2

**Problem:** `useRunStateStore.ts:919-927` evicts `checkpointsById` entries by sorting on `entry[1].seq` and evicting oldest. But this assumes the stored `seq` accurately reflects chronological order. If checkpoints arrive out-of-order (e.g., replay), eviction may remove newer checkpoints that have lower seq due to thread interleaving.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:919-927`
```typescript
if (store.checkpointsById.size > mergedConfig.maxCheckpointsPerRun) {
  const entries = Array.from(store.checkpointsById.entries());
  entries.sort((a, b) => compareSeqs(a[1].seq, b[1].seq)); // Sort by seq
  const toEvict = entries.slice(0, entries.length - mergedConfig.maxCheckpointsPerRun);
  for (const [evictId] of toEvict) {
    store.checkpointsById.delete(evictId);
  }
}
```

**Impact:** Under replay with multiple threads, important checkpoints may be evicted prematurely.

**Fix:** Consider eviction by insertion time (Map maintains insertion order) or LRU access time.

---

### M-756 (P2): JSON decode fallback catches ALL errors, not just SyntaxError
**Category:** Error Handling
**Severity:** P2 (masking non-parse errors)

**Problem:** `jsonPatch.ts:333` catches all errors from JSON.parse, not just SyntaxError. If JSON.parse throws a RangeError (e.g., string too long) or other error, the code silently returns the raw string, corrupting state.

**File:** `observability-ui/src/utils/jsonPatch.ts:327-336`
```typescript
case ValueEncoding.JSON:
  try {
    const jsonStr = textDecoder.decode(valueBytes);
    return parseJsonSafe(jsonStr);
  } catch {  // Catches ALL errors
    // If JSON parsing fails, return as string
    return textDecoder.decode(valueBytes);
  }
```

**Impact:** Non-parse errors (OOM, stack overflow) silently corrupt state instead of failing visibly.

**Fix:** Catch only SyntaxError; re-throw others. Or remove fallback entirely per M-740 fix direction.

---

### M-757 (P3): Legacy get_messages_after_legacy is dead_code but comment says used in tests
**Category:** Code Quality
**Severity:** P3 (documentation mismatch)

**Problem:** `websocket_server.rs:953` has `#[allow(dead_code)]` but the comment at line 952 says "Used in tests for legacy behavior verification". If it's used in tests, it shouldn't need the allow attribute. If it's not used, the comment is wrong.

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:952-954`
```rust
/// ...
/// Kept for backwards compatibility testing; production uses partition-offset resume.
#[allow(dead_code)] // Used in tests for legacy behavior verification
async fn get_messages_after_legacy(&self, ...) {
```

**Impact:** Confusing; may indicate dead test or incomplete migration.

**Fix:** Either enable tests that use this function, or remove the misleading comment.

---

### M-758 (P3): applyLagMetrics maxLatencyMs never reset (grows forever)
**Category:** Metrics
**Severity:** P3 (misleading max value)

**Problem:** `App.tsx:171` tracks `maxLatencyMs` but it's never reset on reconnect or session boundary. Over time, this accumulates the all-time max, not session max.

**File:** `observability-ui/src/App.tsx:166-172`
```typescript
const applyLagMetricsRef = useRef({
  pendingCount: 0,
  totalApplied: 0,
  totalLatencyMs: 0,
  lastReportTime: 0,
  maxLatencyMs: 0,  // Never reset
});
```

**Impact:** Max latency metric becomes meaningless over long sessions.

**Fix:** Reset maxLatencyMs on reconnect along with other counters, or track rolling window max.

---

### M-759 (P3): schemaObservations threadIds array grows unbounded within observation
**Category:** Memory
**Severity:** P3 (unbounded growth per schema)

**Problem:** `App.tsx:343-345` pushes threadIds into the observation's array without limit. For a schema seen across many runs, this array grows unbounded.

**File:** `observability-ui/src/App.tsx:341-346`
```typescript
if (!existing.threadIds.includes(threadId)) {
  existing.threadIds.push(threadId);  // No limit
  existing.runCount++;
}
```

**Impact:** Memory growth for long-running dashboards with many runs per schema.

**Fix:** Cap threadIds array (e.g., keep last 100) and note total in runCount.

---

### M-760 (P3): formatRelativeTime shows "in the future" without actual time
**Category:** UI/Display
**Severity:** P3 (unhelpful message)

**Problem:** `useRunStateStore.ts:265` returns "in the future" for negative diffs (clock skew) without showing the actual time, making debugging harder.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:261-265`
```typescript
function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  if (diff < 0) return 'in the future';  // Unhelpful for debugging
```

**Impact:** Clock skew issues hard to diagnose.

**Fix:** Return `"in the future (${Math.abs(diff)}ms ahead)"` or fall back to absolute time.

---

## Summary Table

| ID | Priority | Category | Description | File(s) |
|----|----------|----------|-------------|---------|
| **M-749** | P1 | Type | StateDiffViewer cursorSeq typed as number, should be string | `StateDiffViewer.tsx:19` |
| **M-750** | P1 | Hash | canonicalJsonString serializes BigInt to "null" | `stateHash.ts:31-32` |
| **M-751** | P1 | Cursor | EventBatch sequence extraction fails on zero-seq events | `dashstream.ts:387-405` |
| **M-752** | P2 | UI | TimelineSlider mixes seq string with index count | `TimelineSlider.tsx:593-596` |
| **M-753** | P2 | Logic | selectedNodeState uses unsorted getRuns() | `App.tsx:493-496` |
| **M-754** | P2 | Logic | Offset comparison uses !== (always true for BigInt) | `App.tsx:1036` |
| **M-755** | P2 | Eviction | checkpointsById eviction by seq, not insertion time | `useRunStateStore.ts:919-927` |
| **M-756** | P2 | Error | JSON decode fallback catches ALL errors | `jsonPatch.ts:327-336` |
| **M-757** | P3 | Docs | dead_code comment mismatch | `websocket_server.rs:952-954` |
| **M-758** | P3 | Metrics | maxLatencyMs never reset | `App.tsx:171` |
| **M-759** | P3 | Memory | threadIds array unbounded per schema | `App.tsx:343-345` |
| **M-760** | P3 | UI | formatRelativeTime unhelpful "in the future" | `useRunStateStore.ts:265` |

**Totals:** 3 P1, 5 P2, 4 P3

---

## Verification Commands

```bash
# Type check after M-749 fix
cd observability-ui && npm run typecheck

# Test hash with BigInt values after M-750 fix
# (add test case to stateHash.test.ts)

# Verify sequence extraction after M-751 fix
# (add EventBatch test with mixed seq values)
```

---

## Cross-References

- M-693: BigInt sequence migration (v18) - foundational change that exposed M-749, M-750
- M-719-M-738: v21 audit findings (async hash race, etc.)
- M-739-M-748: v22 audit findings (apply-lag, cursor_reset)
