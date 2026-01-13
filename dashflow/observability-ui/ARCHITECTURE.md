# Observability UI Architecture

This document explains the state management architecture in the DashFlow observability UI.

## State Pipeline Overview

The UI uses a **single unified state management system** (`useRunStateStore`) that serves all purposes:

```
WebSocket Messages
       │
       ▼
┌─────────────────────────┐
│    useRunStateStore     │
│   (Single Source of     │
│        Truth)           │
└─────────────────────────┘
       │
       ▼
┌─────────────────────────┐
│   RunStateStore         │
│   - Live State          │
│   - Time-Travel State   │
│   - Schema Tracking     │
└─────────────────────────┘
```

## State Store: `useRunStateStore`

**Location:** `src/hooks/useRunStateStore.ts`

**Purpose:** Unified state management for both live execution and time-travel debugging.

**Key Characteristics:**
- Stores every event with sequence numbers (sorted, deduplicated)
- Can reconstruct state at any point in time via `RunCursor`
- Applies JSON Patch operations (RFC 6902) for precise state diffs
- Supports rewinding and fast-forwarding through execution history
- Handles zstd-compressed messages (Phase 741)
- Verifies state hashes after applying diffs (Phase 744)
- Quarantines messages missing `thread_id` (Phase 743)
- Marks runs as corrupted on hash mismatch

**State Structure:**
```typescript
interface RunState {
  events: DashStreamEvent[];      // All events, sorted by seq
  checkpoints: StateCheckpoint[]; // Periodic snapshots
  schema: GraphSchema | null;     // Current schema
  schemaId: string | null;        // Schema content hash
  quarantined: QuarantinedMessage[]; // Unbound telemetry
  corrupted: boolean;             // Hash verification failed
  startTime: number;              // First event timestamp
}

interface RunCursor {
  threadId: string;
  seq: number;
}
```

**Key Methods:**
- `processMessage(decoded)` - Process incoming WebSocket message
- `getStateAtCursor(cursor)` - Reconstruct state at any point
- `getViewModel()` - Get unified graph rendering data
- `getRunsSorted()` - Get runs sorted by recency
- `getQuarantinedMessages()` - Get unbound telemetry

## State Diff Protocol

The store processes WebSocket messages as follows:

1. **Event messages** (`event`, `eventBatch`):
   - Appended to event log with sequence number
   - Sorted by `seq` to handle out-of-order delivery
   - Deduplicated by `seq` to handle retransmissions
   - Node lifecycle events (start/end/error) update node status

2. **State diff messages** (`stateDiff`):
   - If `fullState` present: stored as checkpoint
   - If `operations` present: JSON Patch applied to reconstruct state
   - State hash verified against `state_hash` field
   - Run marked corrupted if hash mismatch

3. **Schema events** (`GraphStart` with schema):
   - Schema extracted from `graph_schema_json` or `graph_manifest`
   - Schema ID tracked for change detection
   - Timeline markers added for schema changes

## Value Encoding

The store supports multiple value encodings in state diffs (Phase 745):

- **JSON** (default): Values parsed as JSON strings
- **MSGPACK**: Not supported - hard-fails with `UnsupportedEncodingError`
- **PROTOBUF**: Not supported - hard-fails with `UnsupportedEncodingError`

The UI enforces JSON-only encoding. Non-JSON encodings cause immediate failure rather than silent corruption.

## Compression Support

The store supports zstd-compressed WebSocket messages (Phase 741):

- Messages with `0x01` header byte are zstd-compressed
- Uses `fzstd` library for browser-compatible decompression
- Transparent to the rest of the pipeline

## Time-Travel Cursor

The cursor `(thread_id, seq)` determines what state is displayed:

- **Live mode**: Cursor follows latest event (`seq = Infinity`)
- **Paused mode**: Cursor fixed at specific sequence number
- **Time-travel**: Cursor moved via timeline slider

State reconstruction uses checkpoints + JSON Patch for efficiency:
1. Find nearest checkpoint before cursor
2. Apply patches from checkpoint to cursor
3. Cache result for repeated access

## Common Pitfalls

### State Not Updating

**Problem:** UI doesn't update when state changes.

**Cause:** Store doesn't trigger re-render.

**Fix:** Components should use store selectors that return new references on change.

### Showing Wrong State in Node Details

**Problem:** Node details show global state instead of node-attributed state.

**Fix:** Use cursor at node's end event:
```typescript
const stateAtNode = store.getStateAtCursor({ threadId, seq: nodeEndSeq });
```

### Hash Mismatch Corruption

**Problem:** Run marked as corrupted, state may be incorrect.

**Cause:** State hash verification failed after applying diffs.

**Fix:** Check if producer is sending correct hashes. Use `getQuarantinedMessages()` to see unbound telemetry.

## Historical Note

Prior to Phase 490, the UI had two separate pipelines:
- `useGraphEvents` - Live state updates
- `useRunStateStore` - Time-travel state

These were consolidated into `useRunStateStore` as the single source of truth. The `useGraphEvents` hook was removed in Phase 755.

## See Also

- [README.md](./README.md) - UI setup and usage
