// Run-scoped state store with time-travel support
// Implements graph view state design: sequence-indexed events with JSON Patch diffs

import { useCallback, useRef, useState } from 'react';
import { DecodedMessage, DiffOperation, EventType, isGraphLifecycleEvent, isNodeLifecycleEvent, coerceU64ToStr } from '../proto/dashstream';
import { applyDiffOperations, getChangedPaths, JsonPatchOp, convertDiffOp, applyPatch, CloneError } from '../utils/jsonPatch';
import { computeStateHash } from '../utils/stateHash';
import { getJsonAttribute, getStringAttribute, getNumberAttribute, boundAttributes, utf8ByteLengthCapped } from '../utils/attributes';
import { GraphSchema, NodeStatus } from '../types/graph';

// M-460: Use structuredClone() instead of JSON.parse(JSON.stringify())
// structuredClone is native, faster, and handles more types correctly (Date, etc.)
// M-680: Wrap in try-catch since structuredClone throws on non-serializable values
// (functions, DOM nodes, symbols). Fall back to JSON round-trip which strips them.
// M-794: JSON.stringify also throws on BigInt. Add safety net for fallback path.
// M-808: Throws CloneError instead of returning original value to prevent mutation aliasing.
// Callers should catch CloneError and handle appropriately (e.g., skip checkpoint, mark corrupted).
function deepCloneJson<T>(value: T): T {
  let structuredCloneError: unknown;
  try {
    return structuredClone(value);
  } catch (e) {
    structuredCloneError = e;
    // Fallback: JSON round-trip strips non-serializable values (functions, undefined, etc.)
    // This is lossy but prevents crashes on unexpected state values.
    console.debug('[deepCloneJson] structuredClone failed, falling back to JSON round-trip');
    try {
      return JSON.parse(JSON.stringify(value)) as T;
    } catch (jsonError) {
      // M-808: Throw instead of returning original to prevent silent mutation aliasing.
      // Returning original means mutations to the "clone" affect the original,
      // which can cause subtle bugs (e.g., checkpoint state modified by later patches,
      // hash verification on mutating state).
      throw new CloneError(
        'Deep clone failed: structuredClone and JSON round-trip both failed. ' +
        'State may contain BigInt, circular references, or non-serializable values. ' +
        'Consider using string encoding for large integers in graph state payloads.',
        structuredCloneError,
        jsonError,
      );
    }
  }
}

// M-1109: normalizeIntegerString removed - was only used by deprecated coerceU64ToString.
// coerceU64ToStr from dashstream.ts handles string normalization internally.

// Compare two Uint8Array for equality
function hashesEqual(a: Uint8Array | undefined, b: Uint8Array): boolean {
  if (!a || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

// M-1109: DEPRECATED - Use coerceU64ToStr from dashstream.ts instead.
// The original function used toNumber() which loses precision for values > 2^53.
// coerceU64ToStr uses toString() which is lossless.
// Keeping this comment block to explain the change for code archaeology.

// M-693: Compare sequence strings as BigInts for correct ordering
function compareSeqs(a: string, b: string): number {
  const aBig = BigInt(a);
  const bBig = BigInt(b);
  if (aBig < bBig) return -1;
  if (aBig > bBig) return 1;
  return 0;
}

// M-786: Check if sequence is a real producer sequence (> 0).
// M-1068: seq==0 means "missing" (proto3 default). Real sequences are strictly > 0.
// Synthetic sequences are negative BigInt values generated locally for messages
// that lack a producer-assigned sequence number.
// Server treats sequence==0 as None: `(header.sequence > 0).then_some(header.sequence)`
function isRealSeq(seq: string): boolean {
  return BigInt(seq) > BigInt(0);
}

// M-696: Convert Uint8Array to hex string for checkpoint ID keys
// M-1110: Bounded bytesToHex to prevent huge string allocations from untrusted bytes.
// messageId/checkpointId should be UUID-like (16-32 bytes), but malicious producers
// could send huge bytes that cause OOM when converted to hex strings.
const MAX_BYTES_FOR_HEX = 64; // 64 bytes = 128 hex chars, enough for any UUID + padding

function bytesToHex(bytes: Uint8Array | undefined, maxBytes = MAX_BYTES_FOR_HEX): string {
  if (!bytes || bytes.length === 0) return '';

  // M-1110: Cap the number of bytes we convert to prevent DoS
  if (bytes.length <= maxBytes) {
    // Fast path for normal-sized IDs
    return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  // M-1110: For oversized bytes, return bounded representation
  // Format: first 16 bytes as hex + marker showing total size
  const prefixBytes = bytes.slice(0, 16);
  const prefixHex = Array.from(prefixBytes).map(b => b.toString(16).padStart(2, '0')).join('');
  return `${prefixHex}...(${bytes.length}b)`;
}

// M-1112: Chunk event_batch apply to avoid long main-thread stalls.
const EVENT_BATCH_CHUNK_SIZE = 250;
const MESSAGE_DRAIN_TIME_BUDGET_MS = 8;

type PendingMessageItem = { kind: 'message'; decoded: DecodedMessage };
type PendingEventBatchItem = {
  kind: 'event_batch';
  batch: DecodedMessage;
  events: unknown[];
  index: number;
};
type PendingItem = PendingMessageItem | PendingEventBatchItem;

function nowMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now();
  }
  return Date.now();
}

// M-696: Check if checkpoint ID bytes are non-empty (valid)
function hasValidCheckpointId(bytes: Uint8Array | undefined): boolean {
  return bytes !== undefined && bytes.length > 0 && bytes.some(b => b !== 0);
}

// Cursor for time travel: identifies a point in time within a run
// M-693: seq is string to prevent precision loss for values > 2^53 (MAX_SAFE_INTEGER)
export interface RunCursor {
  threadId: string;
  seq: string;
}

// A stored event with sequence and state diff info
// M-693: seq is string to prevent precision loss for values > 2^53
export interface StoredEvent {
  seq: string;
  timestamp: number;
  // Message kind for non-event telemetry (e.g., token_chunk, metrics) and dedupe fallback (M-799/M-800)
  kind?: string;
  // Hex-encoded header.messageId when available (preferred dedupe key) (M-800)
  messageId?: string;
  eventType: EventType;
  nodeId?: string;
  attributes: Record<string, unknown>;
  // State changes at this event (if any)
  changedPaths?: string[];
  // For StateDiff messages
  operations?: JsonPatchOp[];
}

// Node execution state at a point in time
// M-693: startSeq/endSeq are strings to prevent precision loss for values > 2^53
export interface NodeState {
  status: NodeStatus;
  startTime?: number;
  endTime?: number;
  durationMs?: number;
  error?: string;
  // Sequence numbers for time-travel state reconstruction (string for precision)
  startSeq?: string;
  endSeq?: string;
}

// Run state store for a single thread_id
export interface RunStateStore {
  threadId: string;
  graphName: string;
  schema: GraphSchema | null;
  schemaId?: string;
  status: 'running' | 'completed' | 'error';
  startTime: number; // Producer timestamp (may have clock skew)
  // M-1086: Arrival time when UI first saw this run (monotonic, no clock skew).
  // Used for eviction decisions to avoid clock-skewed producers causing incorrect eviction.
  arrivalTime: number;
  endTime?: number;

  // Event log (bounded)
  events: StoredEvent[];
  maxEvents: number;
  // M-1059: O(1) deduplication via Set instead of O(n) findIndex
  dedupeKeys: Set<string>;

  // State reconstruction
  latestState: Record<string, unknown>;
  checkpoints: Map<string, Record<string, unknown>>; // seq (string) -> state snapshot
  checkpointInterval: number; // Create checkpoint every N events

  // M-696: Checkpoints by ID for base_checkpoint_id verification
  // Maps checkpoint ID (hex string) to { seq, state, stateValid } for resync
  // M-771: stateValid tracks whether checkpoint state was successfully parsed;
  // unparseable/oversize checkpoints should be treated as "missing base" for resync detection
  checkpointsById: Map<string, { seq: string; state: Record<string, unknown>; stateValid: boolean }>;
  // Set when StateDiff references a base_checkpoint_id we don't have
  needsResync: boolean;
  // Last known checkpoint ID for chaining verification
  lastCheckpointId: string | null;

  // Node states (derived from events)
  nodeStates: Map<string, NodeState>;
  currentNode?: string;

  // State hash verification for data integrity
  corrupted: boolean; // True if state_hash mismatch detected OR patch apply failed
  hashMismatchCount: number; // Number of hash mismatches
  hashVerificationSkipWarned: boolean; // M-683: Warn once per run when state_hash missing
  hashVerificationUnsafeNumberWarned: boolean; // M-741: Warn once when unsafe numbers detected
  // M-784: Disable hash verification per-run after first compute failure (e.g., no WebCrypto).
  // Prevents log spam by only warning once and skipping future attempts.
  hashVerificationErrorWarned: boolean;
  // M-719: Serialize hash verification updates to avoid async races on store state.
  hashVerificationChain: Promise<void>;

  // M-704: Track patch apply failures for corruption flagging
  // When true, skip subsequent patches until a full state snapshot arrives for recovery
  patchApplyFailed: boolean;
  patchApplyError?: string; // Error message from failed patch apply
  // M-730: Track which sequence caused the patch apply failure for debugging
  patchApplyFailedSeq?: string;

  // M-787: Track highest seq where state was successfully applied (snapshot or patch).
  // Used to detect out-of-order state mutations which would corrupt state.
  // When a state-mutating message arrives with seq < lastAppliedSeq, we skip the
  // mutation and flag needsResync=true to wait for recovery via full snapshot.
  lastAppliedSeq: string | null;

  // M-770: Track full state snapshot parse failures
  snapshotParseError?: string;

  // M-116: Debug details for corruption diagnostics
  // M-693: firstMismatchSeq is string for precision
  corruptionDetails?: {
    firstMismatchSeq: string;
    firstMismatchTime: number;
    expectedHash: string;
    computedHash: string;
  };

  // M-39: Track observed nodes for expected vs observed comparison
  observedNodes: Set<string>;
}

// View model for renderers (Canvas, Mermaid, etc.)
export interface GraphViewModel {
  schema: GraphSchema | null;
  schemaId?: string;
  nodeStates: Map<string, NodeState>;
  currentNode?: string;
  state: Record<string, unknown>;
  changedPaths: string[];
  cursor: RunCursor;
  isLive: boolean;
  // M-39: Observed nodes for expected vs observed comparison
  observedNodes: Set<string>;
  // M-39: Nodes referenced in events but not in declared schema
  outOfSchemaNodes: Set<string>;
}

/**
 * Configuration for the run state store.
 *
 * M-123: All settings can be overridden via URL parameters for debugging or
 * production tuning. Use `parseConfigFromUrl()` to read URL overrides.
 *
 * **Memory Tradeoffs:**
 * - Higher limits allow more data but increase browser memory usage
 * - Lower limits prevent OOM but may drop events or runs
 * - For debugging large graphs, temporarily increase via URL params
 * - Production deployments should tune based on expected graph sizes
 */
export interface RunStateStoreConfig {
  /**
   * Maximum events stored per run before oldest events are dropped.
   * URL param: `maxEvents`
   *
   * **Memory impact:** ~1-10KB per event depending on state diff size.
   * 10,000 events ≈ 10-100MB per run. Increase for long-running graphs,
   * decrease if browser becomes sluggish.
   */
  maxEventsPerRun: number;
  /**
   * Events between automatic state checkpoints for time-travel seeking.
   * URL param: `checkpointInterval`
   *
   * **Memory impact:** Each checkpoint stores full state snapshot (~10-100KB).
   * Lower values = faster seeking but more memory. Higher values = slower
   * seeking but less memory. 100 is a good balance for most graphs.
   */
  checkpointInterval: number;
  /**
   * Maximum concurrent runs tracked. Oldest runs are evicted when exceeded.
   * URL param: `maxRuns`
   *
   * **Memory impact:** Each run stores events + checkpoints. 50 runs with
   * default settings can use 500MB-5GB depending on graph complexity.
   * Reduce if tracking many simultaneous runs.
   */
  maxRuns: number;
  /**
   * Maximum checkpoints stored per run (prevents unbounded memory growth).
   * URL param: `maxCheckpoints`
   *
   * **Memory impact:** Each checkpoint is a full state clone (~10-100KB).
   * 200 checkpoints ≈ 2-20MB per run. With maxRuns=50, total checkpoint
   * memory can reach 1GB. M-715.
   */
  maxCheckpointsPerRun: number;
  /**
   * Maximum checkpoint state size in bytes (DoS prevention).
   * URL param: `maxCheckpointSize`
   *
   * **Memory impact:** Prevents single large checkpoint from OOMing browser.
   * 10MB default allows most graph states. Increase only for graphs with
   * very large state objects. M-738.
   */
  maxCheckpointStateSizeBytes: number;
  /**
   * Maximum fullState snapshot size in bytes (DoS prevention).
   * URL param: `maxSnapshotSize`
   *
   * **Memory impact:** Large snapshots can freeze browser during JSON.parse.
   * 10MB default is safe for most browsers. Increase if graphs have large
   * state that needs to be parsed. M-783.
   */
  maxFullStateSizeBytes: number;
  /**
   * Maximum schema/manifest JSON size in bytes (DoS prevention).
   * URL param: `maxSchemaSize`
   *
   * **Memory impact:** Untrusted schema parsing can freeze browser.
   * 2MB default handles complex graph schemas. Increase only if your
   * graph manifest is legitimately larger. M-1087.
   */
  maxSchemaJsonSizeBytes: number;
}

const DEFAULT_CONFIG: RunStateStoreConfig = {
  maxEventsPerRun: 10000,
  checkpointInterval: 100, // Checkpoint every 100 events for fast seeking
  maxRuns: 50,
  maxCheckpointsPerRun: 200, // M-715: Limit checkpoint storage per run
  maxCheckpointStateSizeBytes: 10 * 1024 * 1024, // M-738: 10MB limit prevents browser OOM
  maxFullStateSizeBytes: 10 * 1024 * 1024, // M-783: 10MB limit prevents browser OOM on large snapshots
  maxSchemaJsonSizeBytes: 2 * 1024 * 1024, // M-1087: 2MB cap prevents schema/manifest parse DoS
};

// M-123: Export default config for reference in tests and documentation
export { DEFAULT_CONFIG as RUN_STATE_STORE_DEFAULTS };

/**
 * M-123: Parse RunStateStoreConfig overrides from URL parameters.
 *
 * This allows runtime configuration without code changes, useful for:
 * - Debugging large graphs (increase limits temporarily)
 * - Production tuning based on traffic patterns
 * - Testing memory limits in different browsers
 *
 * **URL Parameter Mapping:**
 * - `maxEvents` → maxEventsPerRun
 * - `checkpointInterval` → checkpointInterval
 * - `maxRuns` → maxRuns
 * - `maxCheckpoints` → maxCheckpointsPerRun
 * - `maxCheckpointSize` → maxCheckpointStateSizeBytes
 * - `maxSnapshotSize` → maxFullStateSizeBytes
 * - `maxSchemaSize` → maxSchemaJsonSizeBytes
 *
 * **Size suffixes supported:** K (1024), M (1024*1024), G (1024*1024*1024)
 * Examples: `maxCheckpointSize=20M`, `maxEvents=50K`
 *
 * @param search - URL search string (defaults to window.location.search)
 * @returns Partial config with only the overridden values
 *
 * @example
 * // URL: ?maxRuns=100&maxEvents=50000&maxCheckpointSize=20M
 * const config = parseConfigFromUrl();
 * // Returns: { maxRuns: 100, maxEventsPerRun: 50000, maxCheckpointStateSizeBytes: 20971520 }
 */
export function parseConfigFromUrl(
  search: string = typeof window !== 'undefined' ? window.location.search : ''
): Partial<RunStateStoreConfig> {
  const params = new URLSearchParams(search);
  const config: Partial<RunStateStoreConfig> = {};

  // Helper to parse integer with optional size suffix (K, M, G)
  const parseIntWithSuffix = (value: string): number | null => {
    const match = value.match(/^(\d+)([KMG])?$/i);
    if (!match) return null;
    let num = parseInt(match[1], 10);
    if (isNaN(num) || num < 0) return null;
    const suffix = match[2]?.toUpperCase();
    if (suffix === 'K') num *= 1024;
    else if (suffix === 'M') num *= 1024 * 1024;
    else if (suffix === 'G') num *= 1024 * 1024 * 1024;
    return num;
  };

  // Helper to parse strict integer (no suffix, no decimals, no trailing chars)
  const parseStrictInt = (value: string): number | null => {
    // Must be only digits
    if (!/^\d+$/.test(value)) return null;
    const num = parseInt(value, 10);
    if (isNaN(num) || num < 0) return null;
    return num;
  };

  // Map URL param names to config keys
  const paramMap: { param: string; key: keyof RunStateStoreConfig; allowSuffix: boolean }[] = [
    { param: 'maxEvents', key: 'maxEventsPerRun', allowSuffix: true },
    { param: 'checkpointInterval', key: 'checkpointInterval', allowSuffix: false },
    { param: 'maxRuns', key: 'maxRuns', allowSuffix: false },
    { param: 'maxCheckpoints', key: 'maxCheckpointsPerRun', allowSuffix: false },
    { param: 'maxCheckpointSize', key: 'maxCheckpointStateSizeBytes', allowSuffix: true },
    { param: 'maxSnapshotSize', key: 'maxFullStateSizeBytes', allowSuffix: true },
    { param: 'maxSchemaSize', key: 'maxSchemaJsonSizeBytes', allowSuffix: true },
  ];

  for (const { param, key, allowSuffix } of paramMap) {
    const value = params.get(param);
    if (value !== null) {
      const parsed = allowSuffix ? parseIntWithSuffix(value) : parseStrictInt(value);
      if (parsed !== null && parsed > 0) {
        config[key] = parsed;
      } else {
        console.warn(`[parseConfigFromUrl] Invalid value for ${param}: "${value}". Using default.`);
      }
    }
  }

  // Log applied overrides for debugging
  if (Object.keys(config).length > 0) {
    console.info('[parseConfigFromUrl] URL overrides applied:', config);
  }

  return config;
}

// Run info with display-friendly metadata for UI rendering
export interface RunInfo {
  threadId: string;
  graphName: string;
  status: 'running' | 'completed' | 'error';
  startTime: number;
  endTime?: number;
  eventCount: number;
  label: string; // Human-readable label like "my-graph (2m ago)"
  corrupted: boolean; // True if state hash verification failed OR patch apply failed
  needsResync: boolean; // M-696: True if StateDiff referenced missing checkpoint
  patchApplyFailed: boolean; // M-704: True if patch apply failed
  // M-730: Sequence that caused patch apply failure (for debugging)
  patchApplyFailedSeq?: string;
  // M-116: Debug details for corruption diagnostics
  // M-693: firstMismatchSeq is string for precision
  corruptionDetails?: {
    firstMismatchSeq: string;
    firstMismatchTime: number;
    expectedHash: string;
    computedHash: string;
  };
}

function formatRelativeDelta(deltaMs: number): string {
  if (deltaMs < 1000) return `${Math.floor(deltaMs)}ms`;
  if (deltaMs < 60000) return `${Math.floor(deltaMs / 1000)}s`;
  if (deltaMs < 3600000) return `${Math.floor(deltaMs / 60000)}m`;
  if (deltaMs < 86400000) return `${Math.floor(deltaMs / 3600000)}h`;
  return `${Math.floor(deltaMs / 86400000)}d`;
}

// Format timestamp as relative time for display
export function formatRelativeTime(timestamp: number, now: number = Date.now()): string {
  const diff = now - timestamp;

  if (diff < 0) {
    const aheadMs = -diff;
    let isoTimestamp = '';
    try {
      isoTimestamp = new Date(timestamp).toISOString();
    } catch {
      isoTimestamp = String(timestamp);
    }
    return `in the future (${formatRelativeDelta(aheadMs)} ahead; ${isoTimestamp})`;
  }
  if (diff < 1000) return 'just now';
  if (diff < 60000) return `${Math.floor(diff / 1000)}s ago`;
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
  return `${Math.floor(diff / 86400000)}d ago`;
}

// Format duration for display
function formatDuration(durationMs: number): string {
  if (durationMs < 1000) return `${durationMs}ms`;
  if (durationMs < 60000) return `${(durationMs / 1000).toFixed(1)}s`;
  return `${(durationMs / 60000).toFixed(1)}m`;
}

// Generate display-friendly label for a run
function generateRunLabel(store: RunStateStore): string {
  const status = store.status === 'running' ? '\u25b6' :
                 store.status === 'error' ? '\u2717' :
                 store.status === 'completed' ? '\u2713' : '';

  const relativeTime = formatRelativeTime(store.startTime);

  const duration = store.endTime
    ? ` (${formatDuration(store.endTime - store.startTime)})`
    : store.status === 'running' ? ' (running)' : '';

  const corrupted = store.corrupted ? ' [corrupted]' : '';
  // M-696: Show resync needed status
  const resync = store.needsResync ? ' [resync needed]' : '';
  // M-704: Show patch apply failure status (more specific than just 'corrupted')
  const patchFailed = store.patchApplyFailed ? ' [patch failed]' : '';

  return `${status} ${store.graphName}${duration} - ${relativeTime}${corrupted}${resync}${patchFailed}`;
}

// M-1108: Quarantine stores bounded summary instead of full DecodedMessage
// This prevents OOM from large malformed messages and avoids retaining sensitive payloads.
// Quarantined message for malformed telemetry
// Messages without thread_id are quarantined instead of merged into "default" run
export interface QuarantinedMessage {
  seq: number;
  timestamp: number;
  type: string;
  reason: string;
  // Kafka cursor info for debugging
  partition?: number;
  offset?: string;
  schemaVersion?: number;
  // Bounded size info
  estimatedSizeBytes: number;
  // Bounded messageId prefix (first 16 bytes as hex)
  messageIdPrefix?: string;
  // Bounded attribute keys only (not values), up to 20 keys
  attributeKeys?: string[];
}

// M-1108: Maximum quarantine memory budget (5MB)
const MAX_QUARANTINE_BYTES = 5 * 1024 * 1024;

// M-1108: Estimate size of a decoded message without allocating
function estimateDecodedMessageSize(decoded: DecodedMessage): number {
  let size = 100; // Base overhead for structure

  // Message type specific size estimation
  if (decoded.message.event) {
    const event = decoded.message.event;
    size += 50; // Event base
    if (event.attributes) {
      // Attributes are Record<string, AttributeValue>
      for (const [key, attrValue] of Object.entries(event.attributes)) {
        size += key.length + 4; // key + quotes + colon + comma
        // AttributeValue has stringValue, intValue, floatValue, boolValue, bytesValue
        if (attrValue.stringValue !== undefined) {
          size += attrValue.stringValue.length + 2;
        } else if (attrValue.bytesValue !== undefined) {
          size += attrValue.bytesValue.length * 2; // Hex encoding
        } else {
          size += 20; // Numbers, booleans
        }
        if (size > MAX_QUARANTINE_BYTES) return size; // Early exit
      }
    }
    if (event.header?.messageId) {
      size += (event.header.messageId as Uint8Array).length * 2; // Hex encoding
    }
  } else if (decoded.message.stateDiff) {
    const diff = decoded.message.stateDiff;
    size += 50;
    if (diff.operations) {
      size += diff.operations.length * 100; // Rough estimate per op
    }
  } else if (decoded.message.checkpoint) {
    const cp = decoded.message.checkpoint;
    size += 50;
    if (cp.state) {
      // state is Uint8Array, use length directly
      size += (cp.state as Uint8Array).length;
    }
  } else if (decoded.message.eventBatch) {
    const batch = decoded.message.eventBatch;
    size += 50;
    if (batch.events) {
      size += batch.events.length * 200; // Rough per-event estimate
    }
  }

  return Math.min(size, MAX_QUARANTINE_BYTES * 2); // Cap estimate
}

// M-1108: Create bounded quarantine summary from DecodedMessage
function createQuarantineSummary(
  decoded: DecodedMessage,
  syntheticSeq: number,
  reason: string
): QuarantinedMessage {
  const summary: QuarantinedMessage = {
    seq: syntheticSeq,
    timestamp: decoded.timestamp,
    type: decoded.type,
    reason,
    partition: decoded.partition,
    offset: decoded.offset,
    schemaVersion: decoded.schemaVersion,
    estimatedSizeBytes: estimateDecodedMessageSize(decoded),
  };

  // Extract bounded messageId prefix (first 16 bytes)
  if (decoded.message.event?.header?.messageId) {
    const bytes = decoded.message.event.header.messageId;
    const prefix = bytes.slice(0, 16);
    summary.messageIdPrefix = bytesToHex(prefix);
    if (bytes.length > 16) {
      summary.messageIdPrefix += `...(${bytes.length} bytes)`;
    }
  }

  // Extract bounded attribute keys (up to 20)
  if (decoded.message.event?.attributes) {
    const keys = Object.keys(decoded.message.event.attributes);
    summary.attributeKeys = keys.slice(0, 20);
    if (keys.length > 20) {
      summary.attributeKeys.push(`...(${keys.length} total)`);
    }
  }

  return summary;
}

// M-121: Valid seek range for a run (oldest..latest seq bounds)
// M-693: seq fields are strings to prevent precision loss for values > 2^53
export interface SeekRange {
  oldestSeq: string;
  latestSeq: string;
  oldestCheckpointSeq: string; // Oldest checkpoint that can be used for reconstruction
}

export interface UseRunStateStoreResult {
  // Process incoming messages
  processMessage: (decoded: DecodedMessage) => void;

  // Get available runs
  getRuns: () => string[];
  // Get runs sorted by recency with display-friendly labels
  getRunsSorted: () => RunInfo[];
  getRunStore: (threadId: string) => RunStateStore | undefined;

  // Access quarantined messages (unbound telemetry missing thread_id)
  getQuarantined: () => QuarantinedMessage[];
  clearQuarantine: () => void;

  // Cursor control
  cursor: RunCursor | null;
  setCursor: (cursor: RunCursor) => void;
  setLiveMode: (live: boolean) => void;
  isLive: boolean;

  // Get view model at current cursor
  getViewModel: () => GraphViewModel | null;

  // Get state at any point in time
  // M-693: seq is string for precision
  getStateAt: (threadId: string, seq: string) => Record<string, unknown>;
  getNodeStatesAt: (threadId: string, seq: string) => Map<string, NodeState>;

  // M-121: Time-travel validity checks
  // M-693: targetSeq is string for precision
  getSeekRange: (threadId: string) => SeekRange | null;
  isSeekValid: (threadId: string, targetSeq: string) => boolean;
  clampSeq: (threadId: string, targetSeq: string) => string;

  // M-450: Hash verification version - increments when async hash check completes
  // Use as dependency to trigger re-render when corruption status changes
  hashVerificationVersion: number;

  // M-711: Mark runs as needing resync due to gap/stale cursor signals
  // reason: Human-readable description of why resync is needed
  markActiveRunsNeedResync: (reason: string) => void;

  // M-744: Clear all run state (for cursor_reset recovery)
  // This completely resets the UI state to a clean slate
  clearAllRuns: () => void;
}

export function useRunStateStore(
  config: Partial<RunStateStoreConfig> = {},
): UseRunStateStoreResult {
  const mergedConfig = { ...DEFAULT_CONFIG, ...config };

  // Store runs by thread_id
  const runsRef = useRef<Map<string, RunStateStore>>(new Map());

  // Quarantine for messages missing thread_id (unbound telemetry)
  const quarantineRef = useRef<QuarantinedMessage[]>([]);
  // M-1108: Track quarantine byte budget (summaries are bounded, but still track total)
  const quarantineBytesRef = useRef(0);
  const MAX_QUARANTINE = 1000; // Limit quarantine count
  const MAX_QUARANTINE_SIZE_BYTES = MAX_QUARANTINE_BYTES; // 5MB byte budget

  // Current cursor position
  const [cursor, setCursorState] = useState<RunCursor | null>(null);
  const [isLive, setIsLive] = useState(true);
  const isLiveRef = useRef(isLive);
  isLiveRef.current = isLive;

  // M-450: Hash verification version - increments when any hash verification completes
  // This allows consumers to trigger re-renders when async corruption detection completes
  const [hashVerificationVersion, setHashVerificationVersion] = useState(0);

  // M-693, M-735: Synthetic sequence counter for messages without sequence numbers
  //
  // ## Why Synthetic Sequences?
  // Some messages arrive without DashStream sequence numbers:
  // - Quarantined messages (missing thread_id, stored temporarily)
  // - Events decoded without a sequence field (e.g., legacy formats)
  //
  // ## Why Negative?
  // Real DashStream sequences are u64 (0 to 2^64-1), always non-negative.
  // Synthetic sequences use negative BigInt values (-1, -2, -3, ...) to:
  // 1. Guarantee ZERO collision with real sequences
  // 2. Sort BEFORE real sequences when using BigInt comparison (compareSeqs)
  //
  // ## Ordering Behavior
  // With BigInt: -3 < -2 < -1 < 0 < 1 < 2 < ...
  // So synthetic events appear before real events in sorted views.
  // This is intentional: unsequenced events are "best effort" ordering.
  //
  // ## Reset Behavior
  // Reset to -1 on cursor_reset (clearAllRuns) to start fresh after recovery.
  const nextSyntheticSeqRef = useRef(BigInt(-1));

  // M-1112: Queue + chunked draining to keep UI responsive under large event_batch.
  const pendingQueueRef = useRef<PendingItem[]>([]);
  const drainScheduledRef = useRef(false);
  const drainingRef = useRef(false);

  // Helper to extract state from event attributes
  // Checks initial_state_json (for GraphStart) and state_json/state (for others)
  // M-1038: Now enforces size limit before JSON.parse to prevent browser OOM/freeze
  const extractState = useCallback((attributes: Record<string, unknown>): Record<string, unknown> => {
    const stateJson =
      getStringAttribute(attributes, 'initial_state_json') ||
      getStringAttribute(attributes, 'state_json') ||
      getStringAttribute(attributes, 'state');
    if (!stateJson) return {};

    // M-1038: Enforce size limit before parsing to prevent DoS via browser OOM/freeze
    // Uses same limit as fullState snapshots for consistency (default 10MB)
    // M-1098: Use utf8ByteLengthCapped instead of TextEncoder().encode() to avoid
    // allocating the full UTF-8 buffer just to check size (allocation-safe size check)
    const sizeBytes = utf8ByteLengthCapped(stateJson, mergedConfig.maxFullStateSizeBytes + 1);
    if (sizeBytes > mergedConfig.maxFullStateSizeBytes) {
      console.error(
        `[useRunStateStore] Event state JSON too large (${sizeBytes} bytes, ` +
        `max ${mergedConfig.maxFullStateSizeBytes}). Skipping parse to prevent browser freeze.`
      );
      return {};
    }

    try {
      return JSON.parse(stateJson);
    } catch (e) {
      console.warn('[useRunStateStore] Failed to parse state JSON from event attributes:', e);
      return {};
    }
  }, [mergedConfig.maxFullStateSizeBytes]);

  // Helper to extract schema from GraphStart attributes
  const extractSchema = useCallback((attributes: Record<string, unknown>): GraphSchema | null => {
    // Prefer graph_schema_json (current format from dashstream_callback.rs)
    const schemaJson = getJsonAttribute(attributes, 'graph_schema_json', {
      maxBytes: mergedConfig.maxSchemaJsonSizeBytes,
    });
    if (schemaJson && typeof schemaJson === 'object') {
      // Backend emits GraphSchema JSON (arrays, snake_case enums) via dashstream_callback.rs
      return schemaJson as GraphSchema;
    }

    // Fallback to graph_manifest
    const manifest =
      getJsonAttribute(attributes, 'graph_manifest', { maxBytes: mergedConfig.maxSchemaJsonSizeBytes }) ||
      getJsonAttribute(attributes, 'manifest', { maxBytes: mergedConfig.maxSchemaJsonSizeBytes });
    if (manifest && typeof manifest === 'object') {
      const m = manifest as Record<string, unknown>;
      return {
        name: (m['name'] as string) || (m['graph_name'] as string) || 'unknown',
        version: (m['version'] as string) || '1.0.0',
        description: m['description'] as string | undefined,
        nodes: [],
        edges: [],
        entry_point: (m['entry_point'] as string) || '__start__',
        metadata: {},
      };
    }

    return null;
  }, [mergedConfig.maxSchemaJsonSizeBytes]);

  // Create or get run store
  const getOrCreateRunStore = useCallback((threadId: string, timestamp: number): RunStateStore => {
    let store = runsRef.current.get(threadId);
    if (!store) {
      store = {
        threadId,
        graphName: 'unknown',
        schema: null,
        status: 'running',
        startTime: timestamp,
        // M-1086: Use monotonic arrival time for eviction (avoids producer clock skew)
        arrivalTime: Date.now(),
        events: [],
        maxEvents: mergedConfig.maxEventsPerRun,
        dedupeKeys: new Set(), // M-1059: O(1) deduplication
        latestState: {},
        checkpoints: new Map(),
        checkpointInterval: mergedConfig.checkpointInterval,
        // M-696: Checkpoint-by-ID storage for resync
        checkpointsById: new Map(),
        needsResync: false,
        lastCheckpointId: null,
        nodeStates: new Map(),
        corrupted: false,
        hashMismatchCount: 0,
        hashVerificationSkipWarned: false, // M-683
        hashVerificationUnsafeNumberWarned: false, // M-741
        hashVerificationErrorWarned: false, // M-784
        hashVerificationChain: Promise.resolve(), // M-719
        patchApplyFailed: false, // M-704: Track patch apply failures
        patchApplyFailedSeq: undefined, // M-730: Track which seq caused failure
        lastAppliedSeq: null, // M-787: Track highest applied state seq for OOO detection
        observedNodes: new Set(), // M-39: Track observed nodes
      };
      runsRef.current.set(threadId, store);

      // Trim old runs if needed
      // M-1086: Use arrivalTime for eviction instead of startTime.
      // Producer timestamp (startTime) can have clock skew causing:
      // - Runs with timestamp=0 being evicted prematurely
      // - Runs with future timestamps never being evicted
      // Arrival time is monotonic and reflects when the UI actually saw the run.
      if (runsRef.current.size > mergedConfig.maxRuns) {
        const oldest = Array.from(runsRef.current.entries())
          .sort(([, a], [, b]) => a.arrivalTime - b.arrivalTime)[0];
        if (oldest) {
          runsRef.current.delete(oldest[0]);
        }
      }
    }
    return store;
  }, [mergedConfig]);

  // Add event to store with ordering and deduplication
  // Maintains per-run event ordering (sorted by seq) and deduplicates by a stable key
  // (messageId preferred; fallback: kind+seq) (M-800)
  // M-693: Uses string sequences with BigInt comparisons for precision
  const addEvent = useCallback((store: RunStateStore, event: StoredEvent) => {
    const getDedupeKey = (e: StoredEvent): string => {
      const msgId = e.messageId && e.messageId.length > 0 ? e.messageId : undefined;
      if (msgId) return `id:${msgId}`;
      return `${e.kind ?? 'unknown'}:${e.seq}`;
    };

    // Deduplicate by stable key (messageId preferred). This prevents dropping distinct messages
    // that share a sequence number (M-800).
    // M-1059: Use Set for O(1) lookup instead of O(n) findIndex
    const dedupeKey = getDedupeKey(event);
    if (store.dedupeKeys.has(dedupeKey)) {
      console.debug(`[RunStateStore] Duplicate event ${dedupeKey}, skipping`);
      return;
    }

    // Find insertion point to maintain sorted order by seq
    // Binary search for efficiency on large event arrays
    // M-693: Use BigInt for comparison to handle values > 2^53
    let insertIndex = store.events.length;
    if (store.events.length > 0) {
      const lastSeq = store.events[store.events.length - 1].seq;
      if (compareSeqs(event.seq, lastSeq) >= 0) {
        // Most common case: new event is latest (or ties), append at end
        insertIndex = store.events.length;
      } else {
        // Out-of-order event: find correct position
        // Binary search for insertion point (after all <= seq) to keep stable ordering for ties
        let low = 0;
        let high = store.events.length;
        while (low < high) {
          const mid = Math.floor((low + high) / 2);
          if (compareSeqs(store.events[mid].seq, event.seq) <= 0) {
            low = mid + 1;
          } else {
            high = mid;
          }
        }
        insertIndex = low;
        console.debug(`[RunStateStore] Out-of-order event seq=${event.seq}, inserting at index ${insertIndex}`);
      }
    }

    // Insert at correct position
    store.events.splice(insertIndex, 0, event);
    // M-1059: Track dedupe key for O(1) duplicate detection
    store.dedupeKeys.add(dedupeKey);

    // Trim events if exceeding max (remove oldest from start)
    // Also trim checkpoints coherently to avoid memory leaks
    if (store.events.length > store.maxEvents) {
      const trimCount = store.events.length - store.maxEvents;
      const trimmedEvents = store.events.splice(0, trimCount);
      // M-1059: Remove dedupe keys for trimmed events
      for (const trimmed of trimmedEvents) {
        store.dedupeKeys.delete(getDedupeKey(trimmed));
      }

      // Get the seq of the oldest remaining event
      const oldestRemainingSeq = store.events.length > 0 ? store.events[0].seq : null;

      // Remove checkpoints for seqs that no longer have corresponding events
      // Keep one checkpoint <= oldestRemainingSeq for time-travel reconstruction
      // M-693: Use BigInt comparisons for checkpoint seq ordering
      let keepCheckpointSeq: string | null = null;
      const checkpointSeqs = Array.from(store.checkpoints.keys()).sort(compareSeqs);

      for (const seq of checkpointSeqs) {
        if (oldestRemainingSeq !== null && compareSeqs(seq, oldestRemainingSeq) < 0) {
          // This checkpoint is before our oldest event
          // Keep the most recent one (for state reconstruction)
          if (keepCheckpointSeq === null || compareSeqs(seq, keepCheckpointSeq) > 0) {
            if (keepCheckpointSeq !== null) {
              store.checkpoints.delete(keepCheckpointSeq);
            }
            keepCheckpointSeq = seq;
          }
        }
      }

      // M-725: Trim nodeStates to only keep nodes that appear in remaining events
      // M-726: Trim observedNodes similarly to prevent unbounded growth
      const nodesInRemainingEvents = new Set<string>();
      for (const event of store.events) {
        if (event.nodeId) {
          nodesInRemainingEvents.add(event.nodeId);
        }
      }

      // Remove node states for nodes not in remaining events
      const nodeStatesToRemove: string[] = [];
      for (const nodeId of store.nodeStates.keys()) {
        if (!nodesInRemainingEvents.has(nodeId)) {
          nodeStatesToRemove.push(nodeId);
        }
      }
      for (const nodeId of nodeStatesToRemove) {
        store.nodeStates.delete(nodeId);
      }

      // Remove observed nodes not in remaining events
      const observedNodesToRemove: string[] = [];
      for (const nodeId of store.observedNodes) {
        if (!nodesInRemainingEvents.has(nodeId)) {
          observedNodesToRemove.push(nodeId);
        }
      }
      for (const nodeId of observedNodesToRemove) {
        store.observedNodes.delete(nodeId);
      }

      console.debug(
        `[RunStateStore] Trimmed ${trimCount} events (seqs ${trimmedEvents[0]?.seq}-${trimmedEvents[trimmedEvents.length - 1]?.seq}), ` +
        `${store.checkpoints.size} checkpoints remaining, ${nodeStatesToRemove.length} nodeStates trimmed, ` +
        `${observedNodesToRemove.length} observedNodes trimmed, oldest event seq=${oldestRemainingSeq}`
      );
    }

    // Create checkpoint if needed
    // M-693: Use BigInt for modulo operation
    const seqBigInt = BigInt(event.seq);
    if (seqBigInt > BigInt(0) && seqBigInt % BigInt(store.checkpointInterval) === BigInt(0)) {
      // M-808: Handle clone failure gracefully - skip checkpoint but continue processing
      try {
        store.checkpoints.set(event.seq, deepCloneJson(store.latestState));
      } catch (e) {
        if (e instanceof CloneError) {
          console.warn(
            `[RunStateStore] Failed to create checkpoint at seq=${event.seq}: ${e.message}. ` +
            'Time-travel granularity may be reduced.'
          );
        } else {
          throw e; // Re-throw unexpected errors
        }
      }

      // M-715: Evict oldest checkpoints if over limit (keep newest for time-travel)
      if (store.checkpoints.size > mergedConfig.maxCheckpointsPerRun) {
        const sortedCheckpointSeqs = Array.from(store.checkpoints.keys()).sort(compareSeqs);
        const toEvict = sortedCheckpointSeqs.slice(0, store.checkpoints.size - mergedConfig.maxCheckpointsPerRun);
        for (const evictSeq of toEvict) {
          store.checkpoints.delete(evictSeq);
          // Keep checkpointsById coherent with seq-indexed eviction (M-721)
          const idsToRemove: string[] = [];
          for (const [id, info] of store.checkpointsById.entries()) {
            if (info.seq === evictSeq) idsToRemove.push(id);
          }
          for (const id of idsToRemove) store.checkpointsById.delete(id);
        }
        console.debug(
          `[RunStateStore] Evicted ${toEvict.length} oldest checkpoints (M-715); ${store.checkpoints.size} remaining`
        );
      }
    }
  }, [mergedConfig.maxCheckpointsPerRun]);

  const buildInnerEventMessageFromBatch = (batch: DecodedMessage, rawEvent: unknown): DecodedMessage => {
    const event = rawEvent as { header?: unknown };
    const innerHeader = event.header as unknown as Record<string, unknown> | undefined;
    const batchThreadId = batch.threadId!;
    const innerThreadId =
      (innerHeader?.['threadId'] as string | undefined) || batchThreadId;
    // M-1109: Use coerceU64ToStr from dashstream.ts which uses toString() (lossless)
    // instead of the old coerceU64ToString which used toNumber() (loses precision > 2^53)
    const innerSeqStr = coerceU64ToStr(innerHeader?.['sequence']);
    const innerSeq = innerSeqStr && isRealSeq(innerSeqStr) ? innerSeqStr : undefined;

    // M-807: Inner events may have their own timestampUs in their header.
    // If present, use it; otherwise fall back to the batch's timestamp.
    // M-817: Use explicit checks to preserve timestamp=0 (Unix epoch) instead of || fallback.
    // M-973: Convert timestampUs (microseconds) to milliseconds. Handle bigint, number, and Long-like.
    const innerTimestampUs = innerHeader?.['timestampUs'];
    let innerTimestamp = batch.timestamp; // default to batch timestamp
    let usValue: number | undefined;
    if (typeof innerTimestampUs === 'bigint') {
      // For very large bigints, Number() conversion may lose precision, but that's acceptable
      // for timestamps (we only need ~ms precision for display purposes)
      usValue = Number(innerTimestampUs);
    } else if (typeof innerTimestampUs === 'number') {
      usValue = innerTimestampUs;
    } else if (typeof innerTimestampUs === 'object' && innerTimestampUs !== null) {
      // Handle protobufjs Long-like objects
      const maybeToNumber = (innerTimestampUs as { toNumber?: () => unknown }).toNumber;
      if (typeof maybeToNumber === 'function') {
        try {
          const parsed = maybeToNumber.call(innerTimestampUs);
          if (typeof parsed === 'number') {
            usValue = parsed;
          }
        } catch {
          // Long.toNumber() threw (overflow), fall back to batch timestamp
        }
      }
    }
    // Convert microseconds to milliseconds and validate
    if (usValue !== undefined && Number.isFinite(usValue)) {
      const msValue = Math.floor(usValue / 1000);
      // Sanity check: timestamp should be reasonable (not negative, not too far in the future)
      // Allow timestamps up to 1 year in the future to handle clock skew
      const maxReasonableMs = Date.now() + 365 * 24 * 60 * 60 * 1000;
      if (msValue >= 0 && msValue <= maxReasonableMs) {
        innerTimestamp = msValue;
      }
      // If out of range, fall back to batch timestamp (already set as default)
    }

    return {
      type: 'event',
      message: { event: rawEvent as never },
      timestamp: innerTimestamp,
      threadId: innerThreadId,
      sequence: innerSeq,
      // M-685: Inner events inherit the batch's Kafka cursor
      partition: batch.partition,
      offset: batch.offset,
    };
  };

  const processMessageImpl = useCallback((decoded: DecodedMessage) => {
    // Quarantine messages missing thread_id instead of merging into "default"
    // M-693: QuarantinedMessage.seq is number for compatibility, use synthetic counter
    // M-1108: Store bounded summary instead of full message to prevent OOM
    const threadId = decoded.threadId!;
    // Use monotonic counter instead of Date.now() to avoid collisions
    // when multiple messages arrive in the same millisecond
    // M-693: seq is string; use decoded.sequence (string) or generate synthetic
    let seq: string;
    if (decoded.sequence && isRealSeq(decoded.sequence)) {
      seq = decoded.sequence;
    } else {
      seq = nextSyntheticSeqRef.current.toString();
      nextSyntheticSeqRef.current = nextSyntheticSeqRef.current - BigInt(1);
    }
    const timestamp = decoded.timestamp;

    if (decoded.type === 'event' && decoded.message.event) {
      const event = decoded.message.event;
      const eventType = event.eventType as EventType;
      const nodeId = event.nodeId;
      const attributes = event.attributes || {};

      const store = getOrCreateRunStore(threadId, timestamp);
      const messageId = bytesToHex(event.header?.messageId) || undefined;

      // Create stored event
      // M-1067: Bound attributes to prevent memory exhaustion from large payloads
      const storedEvent: StoredEvent = {
        seq,
        timestamp,
        kind: 'event',
        messageId,
        eventType,
        nodeId,
        attributes: boundAttributes(attributes as Record<string, unknown>),
      };

      // Handle graph lifecycle events
      if (isGraphLifecycleEvent(eventType)) {
        if (eventType === EventType.EVENT_TYPE_GRAPH_START) {
          // M-1111: Use bounded attributes for GraphStart metadata reads to prevent
          // oversized strings from bloating memory/UI. storedEvent.attributes is already
          // bounded (line 915), but extractSchema needs full attributes for JSON parsing.
          const boundAttrs = storedEvent.attributes as Record<string, unknown>;
          const schema = extractSchema(attributes as Record<string, unknown>);
          const schemaId = getStringAttribute(boundAttrs, 'schema_id');
          const graphName = getStringAttribute(boundAttrs, 'graph_name');

          store.schema = schema;
          store.schemaId = schemaId;
          store.graphName = schema?.name || graphName || 'unknown';
          store.nodeStates.clear();

          // M-798: Never mutate authoritative state with synthetic (unordered) seq.
          // M-1037: Also check for out-of-order state mutations to prevent corruption.
          if (!isRealSeq(seq)) {
            console.warn(
              `[useRunStateStore] GraphStart missing real sequence for thread=${threadId} (seq=${seq}). ` +
              `Skipping state initialization; run flagged for resync.`
            );
            store.needsResync = true;
            store.corrupted = true;
          } else {
            // M-1037: Detect out-of-order state mutations (same pattern as StateDiff).
            const isOutOfOrder = store.lastAppliedSeq !== null &&
              isRealSeq(store.lastAppliedSeq) &&
              compareSeqs(seq, store.lastAppliedSeq) < 0;

            if (isOutOfOrder) {
              console.warn(
                `[useRunStateStore] OUT-OF-ORDER GraphStart detected for thread=${threadId}. ` +
                `Received seq=${seq} but lastAppliedSeq=${store.lastAppliedSeq}. ` +
                `Skipping state initialization to prevent corruption. Run flagged for resync recovery.`
              );
              store.needsResync = true;
              store.corrupted = true;
            } else {
              store.latestState = extractState(attributes as Record<string, unknown>);
              // M-1037: Update lastAppliedSeq after successful state mutation
              store.lastAppliedSeq = seq;
              // Initial checkpoint
              // M-808: Handle clone failure gracefully - skip checkpoint but continue
              try {
                store.checkpoints.set(seq, deepCloneJson(store.latestState));
              } catch (e) {
                if (e instanceof CloneError) {
                  console.warn(`[RunStateStore] Failed to create initial checkpoint at seq=${seq}: ${e.message}`);
                } else {
                  throw e;
                }
              }
            }
          }
        } else if (eventType === EventType.EVENT_TYPE_GRAPH_END) {
          store.status = 'completed';
          store.endTime = timestamp;
          store.currentNode = undefined;
        } else if (eventType === EventType.EVENT_TYPE_GRAPH_ERROR) {
          store.status = 'error';
          store.endTime = timestamp;
          // M-1092 FIX: Clear currentNode on graph error (consistent with GRAPH_END)
          // UI should not show stale "active" node after graph failure
          store.currentNode = undefined;
        }
      }

      // Handle node lifecycle events
      if (isNodeLifecycleEvent(eventType) && nodeId) {
        // M-39: Track observed nodes for expected vs observed comparison
        store.observedNodes.add(nodeId);

        if (eventType === EventType.EVENT_TYPE_NODE_START) {
          store.currentNode = nodeId;
          store.nodeStates.set(nodeId, {
            status: 'active',
            startTime: timestamp,
            startSeq: seq,
          });

          // Update state from event
          // M-1037: Apply the same out-of-order guard as StateDiff/checkpoints
          const newState = extractState(attributes as Record<string, unknown>);
          if (Object.keys(newState).length > 0) {
            if (!isRealSeq(seq)) {
              console.warn(
                `[useRunStateStore] NodeStart state mutation missing real sequence for thread=${threadId} node=${nodeId} (seq=${seq}). ` +
                `Skipping state mutation; run flagged for resync.`
              );
              store.needsResync = true;
              store.corrupted = true;
            } else {
              // M-1037: Detect out-of-order state mutations
              const isOutOfOrder = store.lastAppliedSeq !== null &&
                isRealSeq(store.lastAppliedSeq) &&
                compareSeqs(seq, store.lastAppliedSeq) < 0;

              if (isOutOfOrder) {
                console.warn(
                  `[useRunStateStore] OUT-OF-ORDER NodeStart state mutation detected for thread=${threadId} node=${nodeId}. ` +
                  `Received seq=${seq} but lastAppliedSeq=${store.lastAppliedSeq}. ` +
                  `Skipping state mutation to prevent corruption. Run flagged for resync recovery.`
                );
                store.needsResync = true;
                store.corrupted = true;
              } else {
                store.latestState = { ...store.latestState, ...newState };
                storedEvent.changedPaths = Object.keys(newState).map(k => `/${k}`);
                // M-1037: Update lastAppliedSeq after successful state mutation
                store.lastAppliedSeq = seq;
              }
            }
          }
        } else if (eventType === EventType.EVENT_TYPE_NODE_END) {
          const nodeState = store.nodeStates.get(nodeId);
          if (nodeState) {
            nodeState.status = 'completed';
            nodeState.endTime = timestamp;
            nodeState.endSeq = seq;
            // M-1060: Prefer producer's duration_us if available (more accurate than wall-clock diff)
            // Fall back to timestamp diff, but clamp negative durations to 0 (indicates clock skew)
            const producerDurationUs = getNumberAttribute(attributes as Record<string, unknown>, 'duration_us');
            if (producerDurationUs !== undefined && producerDurationUs >= 0) {
              nodeState.durationMs = producerDurationUs / 1000;
            } else {
              const computedMs = nodeState.startTime
                ? timestamp - nodeState.startTime
                : undefined;
              // Clamp negative durations to 0 (clock skew)
              nodeState.durationMs = computedMs !== undefined ? Math.max(0, computedMs) : undefined;
            }
          }

          if (store.currentNode === nodeId) {
            store.currentNode = undefined;
          }

          // Update state from event
          // M-1037: Apply the same out-of-order guard as StateDiff/checkpoints
          const newState = extractState(attributes as Record<string, unknown>);
          if (Object.keys(newState).length > 0) {
            if (!isRealSeq(seq)) {
              console.warn(
                `[useRunStateStore] NodeEnd state mutation missing real sequence for thread=${threadId} node=${nodeId} (seq=${seq}). ` +
                `Skipping state mutation; run flagged for resync.`
              );
              store.needsResync = true;
              store.corrupted = true;
            } else {
              // M-1037: Detect out-of-order state mutations
              const isOutOfOrder = store.lastAppliedSeq !== null &&
                isRealSeq(store.lastAppliedSeq) &&
                compareSeqs(seq, store.lastAppliedSeq) < 0;

              if (isOutOfOrder) {
                console.warn(
                  `[useRunStateStore] OUT-OF-ORDER NodeEnd state mutation detected for thread=${threadId} node=${nodeId}. ` +
                  `Received seq=${seq} but lastAppliedSeq=${store.lastAppliedSeq}. ` +
                  `Skipping state mutation to prevent corruption. Run flagged for resync recovery.`
                );
                store.needsResync = true;
                store.corrupted = true;
              } else {
                store.latestState = { ...store.latestState, ...newState };
                storedEvent.changedPaths = Object.keys(newState).map(k => `/${k}`);
                // M-1037: Update lastAppliedSeq after successful state mutation
                store.lastAppliedSeq = seq;
              }
            }
          }
        } else if (eventType === EventType.EVENT_TYPE_NODE_ERROR) {
          const nodeState = store.nodeStates.get(nodeId);
          if (nodeState) {
            nodeState.status = 'error';
            nodeState.endTime = timestamp;
            nodeState.endSeq = seq;
            // M-1091 FIX: Use getStringAttribute for wrapper safety + truncate for UI safety
            const rawError = getStringAttribute(attributes as Record<string, unknown>, 'error');
            nodeState.error = rawError && rawError.length > 2000 ? rawError.slice(0, 2000) + '... [truncated]' : rawError;
          }
        }
      }

      addEvent(store, storedEvent);

      // Update cursor in live mode
      // M-693/M-786: Use isRealSeq for string seq comparison (accepts seq >= 0)
      // M-1090 FIX: Only advance cursor, never move backwards on out-of-order events
      // This prevents cursor jumping back on delayed messages (network reorder, replays)
      if (isLiveRef.current && isRealSeq(seq)) {
        setCursorState((currentCursor) => {
          // First event or different thread: set cursor
          if (!currentCursor || currentCursor.threadId !== threadId) {
            return { threadId, seq };
          }
          // Same thread: only advance if new seq > current seq (monotonic)
          if (compareSeqs(seq, currentCursor.seq) > 0) {
            return { threadId, seq };
          }
          // Out-of-order event: keep current cursor position
          return currentCursor;
        });
      }
    } else if (decoded.type === 'state_diff' && decoded.message.stateDiff) {
      const stateDiff = decoded.message.stateDiff;
      const store = getOrCreateRunStore(threadId, timestamp);
      const messageId = bytesToHex(stateDiff.header?.messageId) || undefined;

      // M-798: Never apply state mutations without a real producer sequence.
      // If we can't order a state update, it must not touch authoritative state.
      if (!isRealSeq(seq)) {
        console.warn(
          `[useRunStateStore] StateDiff missing real sequence for thread=${threadId} (seq=${seq}). ` +
          `Skipping state mutation; run flagged for resync.`
        );
        store.needsResync = true;
        store.corrupted = true;
        addEvent(store, {
          seq,
          timestamp,
          kind: 'state_diff',
          messageId,
          eventType: EventType.EVENT_TYPE_UNSPECIFIED,
          attributes: { messageType: 'state_diff', skipped: 'missing_real_sequence' },
        });
        return;
      }

      // M-696: Verify base_checkpoint_id if present (StateDiff chain verification)
      // If we don't have the referenced checkpoint, we may have missed messages
      // M-771: Also check stateValid - unparseable checkpoints should trigger resync
      if (hasValidCheckpointId(stateDiff.baseCheckpointId)) {
        const baseId = bytesToHex(stateDiff.baseCheckpointId);
        const baseCheckpoint = store.checkpointsById.get(baseId);
        if (!baseCheckpoint) {
          // We don't have the base checkpoint - flag for resync
          if (!store.needsResync) {
            console.warn(
              `[useRunStateStore] StateDiff references unknown base_checkpoint_id=${baseId.slice(0, 16)}... ` +
              `for thread=${threadId} seq=${seq}. Run may need resync.`
            );
            store.needsResync = true;
          }
        } else if (!baseCheckpoint.stateValid) {
          // M-771: We have the checkpoint ID but state is invalid (parse failed or oversize)
          // Treat as missing base for resync detection
          if (!store.needsResync) {
            console.warn(
              `[useRunStateStore] StateDiff references base_checkpoint_id=${baseId.slice(0, 16)}... ` +
              `but checkpoint state is invalid (parse failed or oversize) for thread=${threadId} seq=${seq}. ` +
              `Run may need resync.`
            );
            store.needsResync = true;
          }
        } else {
          // We have the base checkpoint with valid state - this diff chain is valid
          console.debug(
            `[useRunStateStore] StateDiff base_checkpoint_id=${baseId.slice(0, 16)}... verified for seq=${seq}`
          );
        }
      }

      const storedEvent: StoredEvent = {
        seq,
        timestamp,
        kind: 'state_diff',
        messageId,
        eventType: EventType.EVENT_TYPE_UNSPECIFIED,
        attributes: {},
      };

      // M-777: Track whether state was actually applied (snapshot or patch succeeded)
      // If false, skip hash verification to avoid false mismatches
      let stateApplied = true;

      // M-787: Detect out-of-order state mutations.
      // If we've already applied state at a higher seq, applying an older seq would corrupt state.
      // This can happen due to multi-producer scenarios, network reordering, or protocol bugs.
      // Defense: skip the mutation and flag for resync recovery via full snapshot.
      // Note: isRealSeq check ensures synthetic negative seqs don't trigger false OOO detection.
      const isOutOfOrder = store.lastAppliedSeq !== null &&
        isRealSeq(seq) &&
        isRealSeq(store.lastAppliedSeq) &&
        compareSeqs(seq, store.lastAppliedSeq) < 0;

      if (isOutOfOrder) {
        console.warn(
          `[useRunStateStore] OUT-OF-ORDER state mutation detected for thread=${threadId}. ` +
          `Received seq=${seq} but lastAppliedSeq=${store.lastAppliedSeq}. ` +
          `Skipping state mutation to prevent corruption. Run flagged for resync recovery.`
        );
        store.needsResync = true;
        store.corrupted = true;
        stateApplied = false;
        // Still record the event for timeline (addEvent handles out-of-order insertion),
        // but we won't mutate state. Fall through to addEvent below.
      } else if (stateDiff.fullState && stateDiff.fullState.length > 0) {
        // Full state snapshot
        // M-783: Reject oversized fullState to prevent DoS via browser OOM/freeze.
        // Large JSON.parse operations can lock the main thread for seconds and exhaust memory.
        if (stateDiff.fullState.length > mergedConfig.maxFullStateSizeBytes) {
          console.error(
            `[RunStateStore] FullState snapshot too large (${stateDiff.fullState.length} bytes, ` +
            `max ${mergedConfig.maxFullStateSizeBytes}) for thread=${threadId} seq=${seq}. ` +
            `Run marked as corrupted/needsResync. Consider increasing maxFullStateSizeBytes if legitimate.`
          );
          store.corrupted = true;
          store.needsResync = true;
          store.snapshotParseError = `Snapshot size ${stateDiff.fullState.length} exceeds limit ${mergedConfig.maxFullStateSizeBytes}`;
          stateApplied = false;
        } else {
          try {
            // M-772: Use fatal: true to throw on invalid UTF-8 instead of silently replacing
            // with U+FFFD replacement characters. Invalid UTF-8 indicates data corruption.
            const stateStr = new TextDecoder('utf-8', { fatal: true }).decode(stateDiff.fullState);
            store.latestState = JSON.parse(stateStr);
            storedEvent.changedPaths = Object.keys(store.latestState).map(k => `/${k}`);
            // Snapshots are natural checkpoints for fast/accurate seeking.
            // M-808: Handle clone failure gracefully - skip checkpoint but continue
            try {
              store.checkpoints.set(seq, deepCloneJson(store.latestState));
            } catch (e) {
              if (e instanceof CloneError) {
                console.warn(
                  `[RunStateStore] Failed to create snapshot checkpoint at seq=${seq}: ${e.message}. ` +
                  'Time-travel granularity may be reduced.'
                );
              } else {
                throw e;
              }
            }

            // M-715: Evict oldest checkpoints if over limit
            if (store.checkpoints.size > mergedConfig.maxCheckpointsPerRun) {
              const sortedCheckpointSeqs = Array.from(store.checkpoints.keys()).sort(compareSeqs);
              const toEvict = sortedCheckpointSeqs.slice(0, store.checkpoints.size - mergedConfig.maxCheckpointsPerRun);
              for (const evictSeq of toEvict) {
                store.checkpoints.delete(evictSeq);
                // Keep checkpointsById coherent with seq-indexed eviction (M-721)
                const idsToRemove: string[] = [];
                for (const [id, info] of store.checkpointsById.entries()) {
                  if (info.seq === evictSeq) idsToRemove.push(id);
                }
                for (const id of idsToRemove) store.checkpointsById.delete(id);
              }
            }

            // M-704: Full state snapshot provides recovery from patch apply failures
            if (store.patchApplyFailed) {
              console.info(
                `[useRunStateStore] Full state snapshot received for thread=${threadId} seq=${seq}. ` +
                `Recovering from patch apply failure. State is now consistent.`
              );
              store.patchApplyFailed = false;
              store.patchApplyError = undefined;
            }
            // M-696: Full snapshot also recovers from missing checkpoint references
            if (store.needsResync) {
              console.info(
                `[useRunStateStore] Full state snapshot received for thread=${threadId} seq=${seq}. ` +
                `Recovering from checkpoint gap. State is now consistent.`
              );
              store.needsResync = false;
            }
            // M-770: Full state snapshot recovers from prior snapshot parse failures
            if (store.snapshotParseError) {
              console.info(
                `[useRunStateStore] Full state snapshot received for thread=${threadId} seq=${seq}. ` +
                `Recovering from prior snapshot parse error. State is now consistent.`
              );
              store.snapshotParseError = undefined;
            }
            // M-806: Clear corrupted flag on successful snapshot recovery.
            // A successful full state snapshot means state is now consistent, regardless
            // of what caused the previous corruption (missing seq, out-of-order, hash mismatch, etc.).
            if (store.corrupted) {
              console.info(
                `[useRunStateStore] Full state snapshot for thread=${threadId} seq=${seq} ` +
                `clearing corrupted flag. State integrity restored.`
              );
              store.corrupted = false;
              store.corruptionDetails = undefined;
            }
            // M-787: Update lastAppliedSeq after successful snapshot application.
            // Only update for real sequences (>= 0), not synthetic negative seqs.
            if (isRealSeq(seq)) {
              store.lastAppliedSeq = seq;
            }
          } catch (e) {
            // M-770: Mark run as corrupted/needsResync on fullState parse failure.
            // Without this, UI can continue showing stale/partial state as if it's healthy.
            const errorMsg = e instanceof Error ? e.message : String(e);
            store.corrupted = true;
            store.needsResync = true;
            store.snapshotParseError = errorMsg;
            // M-777: Snapshot parse failed, don't run hash verification
            stateApplied = false;
            console.error(
              `[useRunStateStore] Failed to parse full state for thread=${threadId} seq=${seq}: ${errorMsg}. ` +
              `Run marked as corrupted/needsResync. State may be stale.`
            );
          }
        }
      } else if (stateDiff.operations && stateDiff.operations.length > 0) {
        // M-704: Skip patch application if previous patch failed - wait for full state recovery
        // M-776: Also skip if needsResync=true - we're applying patches to a known-wrong base
        if (store.patchApplyFailed || store.needsResync) {
          if (store.patchApplyFailed) {
            console.warn(
              `[useRunStateStore] Skipping patch for thread=${threadId} seq=${seq} due to prior patch failure. ` +
              `Waiting for full state snapshot for recovery.`
            );
          } else {
            // M-776: needsResync is true - base state is unknown/wrong
            console.warn(
              `[useRunStateStore] Skipping patch for thread=${threadId} seq=${seq} - run needs resync. ` +
              `State base is unknown; waiting for full state snapshot or checkpoint for recovery.`
            );
          }
          // Still record the event for timeline purposes, but state won't be updated
          // M-777: Mark that state wasn't applied so hash verification is skipped
          stateApplied = false;
        } else {
          // Apply JSON Patch operations
          const ops = stateDiff.operations.map(convertDiffOp);
          storedEvent.operations = ops;
          storedEvent.changedPaths = getChangedPaths(ops);

          try {
            store.latestState = applyDiffOperations(
              store.latestState,
              stateDiff.operations as DiffOperation[],
            ) as Record<string, unknown>;
            // M-787: Update lastAppliedSeq after successful patch application.
            // Only update for real sequences (>= 0), not synthetic negative seqs.
            if (isRealSeq(seq)) {
              store.lastAppliedSeq = seq;
            }
          } catch (e) {
            // M-704: Mark run as corrupted on patch apply failure
            // M-730: Track which seq caused the failure for debugging
            const errorMsg = e instanceof Error ? e.message : String(e);
            store.patchApplyFailed = true;
            store.patchApplyError = errorMsg;
            store.patchApplyFailedSeq = seq;
            store.corrupted = true;
            // M-777: Patch failed, don't run hash verification
            stateApplied = false;
            console.error(
              `[useRunStateStore] Patch apply FAILED for thread=${threadId} seq=${seq}: ${errorMsg}. ` +
              `Run marked as corrupted. Subsequent patches will be skipped until full state snapshot arrives.`
            );
          }
        }
      }

      // Verify state_hash after applying snapshot/patches to detect corruption
      // M-777: Only verify hash when state was actually applied; skip if patch was skipped or failed.
      // When stateApplied=false, latestState doesn't match the expected hash target.
      // M-450: Hash verification triggers UI update via hashVerificationVersion
      // M-670: Clone state BEFORE scheduling async hash to avoid race with subsequent diffs.
      // Without this, store.latestState can be mutated by incoming messages while the hash
      // is being computed, producing false mismatches (hashing "future state" vs expected).
      // M-784: Skip if hash verification previously failed (e.g., no WebCrypto) to avoid log spam.
      if (stateApplied && stateDiff.stateHash && stateDiff.stateHash.length > 0 && !store.hashVerificationErrorWarned) {
        const expectedHashBytes = new Uint8Array(stateDiff.stateHash);
        // M-808: Handle clone failure gracefully - skip hash verification if clone fails
        let stateSnapshotForHash: Record<string, unknown>;
        try {
          stateSnapshotForHash = deepCloneJson(store.latestState);
        } catch (e) {
          if (e instanceof CloneError) {
            console.warn(
              `[RunStateStore] Failed to clone state for hash verification at seq=${seq}: ${e.message}. ` +
              'Hash verification skipped.'
            );
            setHashVerificationVersion(v => v + 1);
            return; // Skip hash verification for this diff
          }
          throw e;
        }
        store.hashVerificationChain = store.hashVerificationChain
          .then(async () => {
            const hashResult = await computeStateHash(stateSnapshotForHash);

            // M-741: Skip verification if state contains unsafe numbers (> MAX_SAFE_INTEGER).
            // JSON.parse rounds large integers, causing false hash mismatches.
            if (hashResult.hasUnsafeNumbers) {
              if (!store.hashVerificationUnsafeNumberWarned) {
                store.hashVerificationUnsafeNumberWarned = true;
                console.warn(
                  `[useRunStateStore] State contains numbers > MAX_SAFE_INTEGER that may have lost precision. ` +
                  `Hash verification skipped to avoid false corruption flags. ` +
                  `Consider using string encoding for large integers in graph state payloads.`
                );
              }
              setHashVerificationVersion(v => v + 1);
              return;
            }

            if (!hashesEqual(expectedHashBytes, hashResult.hash)) {
              store.hashMismatchCount++;
              // M-116: Track corruption details on first mismatch
              const expectedHashStr = Array.from(expectedHashBytes).map(b => b.toString(16).padStart(2, '0')).join('');
              const computedHashStr = Array.from(hashResult.hash).map(b => b.toString(16).padStart(2, '0')).join('');
              if (!store.corrupted) {
                // First mismatch - record details
                store.corruptionDetails = {
                  firstMismatchSeq: seq,
                  firstMismatchTime: timestamp,
                  expectedHash: expectedHashStr,
                  computedHash: computedHashStr,
                };
              }
              store.corrupted = true;
              console.warn(
                `[useRunStateStore] State hash mismatch for thread=${threadId} seq=${seq}. ` +
                `Expected ${expectedHashStr}, got ${computedHashStr}. ` +
                `Run marked as corrupted (${store.hashMismatchCount} mismatches total).`
              );
            }
            // M-450: Trigger UI update after verification completes (success or mismatch)
            setHashVerificationVersion(v => v + 1);
          })
          .catch(e => {
            // M-784: Only warn once per run and disable future attempts to avoid log spam.
            // Common causes: WebCrypto unavailable in non-HTTPS environments.
            if (!store.hashVerificationErrorWarned) {
              store.hashVerificationErrorWarned = true;
              console.warn(
                `[useRunStateStore] Failed to compute state hash for thread=${threadId} seq=${seq}: ${e}. ` +
                `Hash verification disabled for this run. To re-enable, reload the page.`
              );
            }
            // M-450: Also trigger update on error so UI doesn't get stuck
            setHashVerificationVersion(v => v + 1);
          });
      } else {
        // M-683: Warn when producer omits state_hash - state integrity cannot be verified.
        // Only warn once per run to avoid log spam.
        if (!store.hashVerificationSkipWarned) {
          store.hashVerificationSkipWarned = true;
          console.warn(
            `[useRunStateStore] StateDiff for thread=${threadId} seq=${seq} has no state_hash. ` +
            `State integrity verification skipped. Consider enabling state_hash in the producer ` +
            `(DashStreamCallback::enable_state_hash) for corruption detection.`
          );
        }
      }

      addEvent(store, storedEvent);

      // M-693/M-786: Use isRealSeq for string seq comparison (accepts seq >= 0)
      // M-1090 FIX: Only advance cursor, never move backwards on out-of-order events
      if (isLiveRef.current && isRealSeq(seq)) {
        setCursorState((currentCursor) => {
          if (!currentCursor || currentCursor.threadId !== threadId) {
            return { threadId, seq };
          }
          if (compareSeqs(seq, currentCursor.seq) > 0) {
            return { threadId, seq };
          }
          return currentCursor;
        });
      }
    } else if (decoded.type === 'checkpoint' && decoded.message.checkpoint) {
      // M-696: Process checkpoint messages for resync support
      const checkpoint = decoded.message.checkpoint;
      const store = getOrCreateRunStore(threadId, timestamp);

      // Extract checkpoint ID from the message (use checkpointId field or header.messageId)
      const checkpointId = bytesToHex(
        (checkpoint as unknown as { checkpointId?: Uint8Array }).checkpointId ||
        checkpoint.header?.messageId
      );
      const messageId = bytesToHex(checkpoint.header?.messageId) || undefined;

      // Record checkpoint message in the run timeline (M-799)
      addEvent(store, {
        seq,
        timestamp,
        kind: 'checkpoint',
        messageId,
        eventType: EventType.EVENT_TYPE_UNSPECIFIED,
        attributes: {
          messageType: 'checkpoint',
          checkpointId: checkpointId || undefined,
          stateBytes: (checkpoint as unknown as { state?: Uint8Array }).state?.length ?? 0,
        },
      });

      // M-798: Never apply checkpoint state without a real producer sequence.
      if (!isRealSeq(seq)) {
        console.warn(
          `[useRunStateStore] Checkpoint missing real sequence for thread=${threadId} (seq=${seq}). ` +
          `Skipping checkpoint state; run flagged for resync.`
        );
        store.needsResync = true;
        store.corrupted = true;
        return;
      }

      if (checkpointId) {
        // Try to parse state from checkpoint
        const stateBytes = (checkpoint as unknown as { state?: Uint8Array }).state;
        if (stateBytes && stateBytes.length > 0) {
          // M-738: Reject oversized checkpoint state to prevent DoS via browser OOM
          if (stateBytes.length > mergedConfig.maxCheckpointStateSizeBytes) {
            console.warn(
              `[RunStateStore] Checkpoint state too large (${stateBytes.length} bytes, ` +
              `max ${mergedConfig.maxCheckpointStateSizeBytes}). Skipping parse for id=${checkpointId.slice(0, 16)}...`
            );
            // M-771: Track checkpoint ID but mark stateValid=false so base_checkpoint_id
            // verification will treat it as an unusable base and trigger resync
            store.lastCheckpointId = checkpointId;
            store.checkpointsById.set(checkpointId, { seq, state: {}, stateValid: false });
            return;
          }
          try {
            // M-778: Use fatal: true to throw on invalid UTF-8 (consistent with fullState decode at line 857).
            // Without fatal: true, invalid UTF-8 sequences are silently replaced with U+FFFD, which
            // corrupts the state hash and can cause downstream parse failures or incorrect state.
            const stateStr = new TextDecoder('utf-8', { fatal: true }).decode(stateBytes);
            const state = JSON.parse(stateStr) as Record<string, unknown>;

            // Store checkpoint by ID for base_checkpoint_id verification
            // M-771: Mark stateValid=true for successfully parsed checkpoints
            // M-808: Handle clone failure gracefully - store with stateValid=false if clone fails
            const existing = store.checkpointsById.get(checkpointId);
            if (existing && existing.seq !== seq) {
              store.checkpoints.delete(existing.seq);
            }
            let clonedStateForById: Record<string, unknown>;
            try {
              clonedStateForById = deepCloneJson(state);
              store.checkpointsById.set(checkpointId, { seq, state: clonedStateForById, stateValid: true });
            } catch (e) {
              if (e instanceof CloneError) {
                console.warn(
                  `[RunStateStore] Failed to clone checkpoint state for id=${checkpointId.slice(0, 16)}... seq=${seq}: ${e.message}. ` +
                  'Storing with stateValid=false.'
                );
                store.checkpointsById.set(checkpointId, { seq, state: {}, stateValid: false });
              } else {
                throw e;
              }
            }
            store.lastCheckpointId = checkpointId;

            // M-787: Check for out-of-order checkpoint before updating latestState.
            // We still store the checkpoint by ID (above) for potential recovery use,
            // but we don't overwrite latestState with an older checkpoint's state.
            const isCheckpointOutOfOrder = store.lastAppliedSeq !== null &&
              isRealSeq(seq) &&
              isRealSeq(store.lastAppliedSeq) &&
              compareSeqs(seq, store.lastAppliedSeq) < 0;

            if (isCheckpointOutOfOrder) {
              console.warn(
                `[useRunStateStore] OUT-OF-ORDER checkpoint for thread=${threadId}. ` +
                `Checkpoint seq=${seq} but lastAppliedSeq=${store.lastAppliedSeq}. ` +
                `Stored checkpoint by ID for recovery but NOT updating latestState. ` +
                `Run flagged for resync.`
              );
              store.needsResync = true;
              store.corrupted = true;
              // Don't update latestState or lastAppliedSeq - skip to event storage
            } else {
              // Also update latestState and create a seq-indexed checkpoint
              store.latestState = state;
              // M-808: Handle clone failure gracefully - skip checkpoint but latestState is already set
              try {
                store.checkpoints.set(seq, deepCloneJson(state));
              } catch (e) {
                if (e instanceof CloneError) {
                  console.warn(
                    `[RunStateStore] Failed to create checkpoint at seq=${seq}: ${e.message}. ` +
                    'Time-travel granularity may be reduced.'
                  );
                } else {
                  throw e;
                }
              }
              // M-787: Update lastAppliedSeq after successful checkpoint state application
              if (isRealSeq(seq)) {
                store.lastAppliedSeq = seq;
              }
              // M-806: Clear corrupted/needsResync on successful checkpoint application.
              // A valid checkpoint with parseable state provides full recovery.
              if (store.needsResync) {
                console.info(
                  `[useRunStateStore] Checkpoint id=${checkpointId.slice(0, 16)}... seq=${seq} ` +
                  `clearing needsResync for thread=${threadId}. State recovered from checkpoint.`
                );
                store.needsResync = false;
              }
              if (store.corrupted) {
                console.info(
                  `[useRunStateStore] Checkpoint id=${checkpointId.slice(0, 16)}... seq=${seq} ` +
                  `clearing corrupted flag for thread=${threadId}. State integrity restored.`
                );
                store.corrupted = false;
                store.corruptionDetails = undefined;
              }
              if (store.patchApplyFailed) {
                console.info(
                  `[useRunStateStore] Checkpoint id=${checkpointId.slice(0, 16)}... seq=${seq} ` +
                  `clearing patchApplyFailed for thread=${threadId}. State recovered from checkpoint.`
                );
                store.patchApplyFailed = false;
                store.patchApplyError = undefined;
              }
            }

            // M-715: Evict oldest checkpoints if over limit
            if (store.checkpoints.size > mergedConfig.maxCheckpointsPerRun) {
              const sortedCheckpointSeqs = Array.from(store.checkpoints.keys()).sort(compareSeqs);
              const toEvict = sortedCheckpointSeqs.slice(0, store.checkpoints.size - mergedConfig.maxCheckpointsPerRun);
              for (const evictSeq of toEvict) {
                store.checkpoints.delete(evictSeq);
                // Keep checkpointsById coherent with seq-indexed eviction (M-721)
                const idsToRemove: string[] = [];
                for (const [id, info] of store.checkpointsById.entries()) {
                  if (info.seq === evictSeq) idsToRemove.push(id);
                }
                for (const id of idsToRemove) store.checkpointsById.delete(id);
              }
            }
            // M-755: Removed redundant checkpointsById eviction block.
            // The coherent eviction logic at lines 989-1001 already keeps checkpoints and
            // checkpointsById synchronized - it evicts from checkpoints first, then removes
            // corresponding entries from checkpointsById. A second eviction pass here would
            // desynchronize the stores by removing checkpointsById entries whose seqs
            // still exist in checkpoints.

            console.debug(
              `[RunStateStore] Stored checkpoint id=${checkpointId.slice(0, 16)}... seq=${seq} for thread=${threadId}`
            );
          } catch (e) {
            // M-729: Still update lastCheckpointId even when state parsing fails.
            // M-771: Mark stateValid=false so base_checkpoint_id verification will
            // treat this as an unusable base and trigger resync.
            console.warn(
              `[RunStateStore] Failed to parse checkpoint state for id=${checkpointId.slice(0, 16)}...:`,
              e
            );
            store.lastCheckpointId = checkpointId;
            // Store a placeholder indicating checkpoint exists but state is invalid
            store.checkpointsById.set(checkpointId, { seq, state: {}, stateValid: false });
          }
        }
      }

      // Update cursor in live mode
      // M-1090 FIX: Only advance cursor, never move backwards on out-of-order events
      if (isLiveRef.current && isRealSeq(seq)) {
        setCursorState((currentCursor) => {
          if (!currentCursor || currentCursor.threadId !== threadId) {
            return { threadId, seq };
          }
          if (compareSeqs(seq, currentCursor.seq) > 0) {
            return { threadId, seq };
          }
          return currentCursor;
        });
      }
    } else {
      // M-799: Record non-core message types in the run timeline so operators can see them.
      const store = getOrCreateRunStore(threadId, timestamp);

      // M-1067: Bound attributes to prevent memory exhaustion from large payloads
      const mkStoredEvent = (messageIdHex: string | undefined, attributes: Record<string, unknown>): StoredEvent => ({
        seq,
        timestamp,
        kind: decoded.type,
        messageId: messageIdHex,
        eventType: EventType.EVENT_TYPE_UNSPECIFIED,
        attributes: boundAttributes(attributes),
      });

      if (decoded.type === 'token_chunk' && decoded.message.tokenChunk) {
        const tokenChunk = decoded.message.tokenChunk;
        const msgId = bytesToHex(tokenChunk.header?.messageId) || undefined;
        addEvent(store, mkStoredEvent(msgId, {
          messageType: 'token_chunk',
          requestId: tokenChunk.requestId || undefined,
          model: tokenChunk.model || undefined,
          chunkIndex: tokenChunk.chunkIndex,
          isFinal: tokenChunk.isFinal,
          textLen: tokenChunk.text?.length ?? 0,
          finishReason: tokenChunk.finishReason,
        }));
      } else if (decoded.type === 'tool_execution' && decoded.message.toolExecution) {
        const tool = decoded.message.toolExecution;
        const msgId = bytesToHex(tool.header?.messageId) || undefined;
        addEvent(store, mkStoredEvent(msgId, {
          messageType: 'tool_execution',
          callId: tool.callId || undefined,
          toolName: tool.toolName || undefined,
          stage: tool.stage,
          retryCount: tool.retryCount,
          durationUs: tool.durationUs !== undefined ? tool.durationUs.toString() : undefined,
          argsBytes: tool.arguments?.length ?? 0,
          resultBytes: tool.result?.length ?? 0,
          error: tool.error || undefined,
        }));
      } else if (decoded.type === 'metrics' && decoded.message.metrics) {
        const metrics = decoded.message.metrics;
        const msgId = bytesToHex(metrics.header?.messageId) || undefined;
        // M-1039 FIX: Store bounded metadata instead of full tags (unbounded, potentially sensitive)
        // Extract tag keys and count rather than storing arbitrary tag values
        const rawTags = metrics.tags || {};
        const tagKeys = Object.keys(rawTags);
        // Allowlist of safe tag keys for observability (no PII/secrets)
        const SAFE_TAG_KEYS = ['tenant_id', 'thread_id', 'model', 'graph_name', 'node_id', 'scope', 'scope_id', 'stage', 'env'];
        const safeTags: Record<string, string> = {};
        for (const key of SAFE_TAG_KEYS) {
          if (key in rawTags && typeof rawTags[key] === 'string') {
            // Cap individual tag value length to prevent memory abuse
            const value = rawTags[key];
            safeTags[key] = value.length > 256 ? value.slice(0, 256) + '...' : value;
          }
        }
        addEvent(store, mkStoredEvent(msgId, {
          messageType: 'metrics',
          scope: metrics.scope || undefined,
          scopeId: metrics.scopeId || undefined,
          // M-1039: Store bounded tag info instead of full tags
          tagCount: tagKeys.length,
          tagKeys: tagKeys.slice(0, 20), // Cap to 20 keys max for display
          safeTags, // Only allowlisted keys with capped values
          metricKeys: metrics.metrics ? Object.keys(metrics.metrics).length : 0,
        }));
      } else if (decoded.type === 'error' && decoded.message.error) {
        const err = decoded.message.error;
        const msgId = bytesToHex(err.header?.messageId) || undefined;
        addEvent(store, mkStoredEvent(msgId, {
          messageType: 'error',
          errorCode: err.errorCode || undefined,
          message: err.message || undefined,
          severity: err.severity,
          exceptionType: err.exceptionType || undefined,
        }));
      } else if (decoded.type === 'execution_trace' && decoded.message.executionTrace) {
        const trace = decoded.message.executionTrace;
        const msgId = bytesToHex(trace.header?.messageId) || undefined;
        addEvent(store, mkStoredEvent(msgId, {
          messageType: 'execution_trace',
          executionId: trace.executionId || undefined,
          nodesExecuted: trace.nodesExecuted?.length ?? 0,
          totalDurationMs: trace.totalDurationMs !== undefined ? trace.totalDurationMs.toString() : undefined,
          totalTokens: trace.totalTokens !== undefined ? trace.totalTokens.toString() : undefined,
          errors: trace.errors?.length ?? 0,
        }));
      } else {
        addEvent(store, mkStoredEvent(undefined, { messageType: decoded.type }));
      }

      // M-1090 FIX: Only advance cursor, never move backwards on out-of-order events
      if (isLiveRef.current && isRealSeq(seq)) {
        setCursorState((currentCursor) => {
          if (!currentCursor || currentCursor.threadId !== threadId) {
            return { threadId, seq };
          }
          if (compareSeqs(seq, currentCursor.seq) > 0) {
            return { threadId, seq };
          }
          return currentCursor;
        });
      }
    }
  }, [getOrCreateRunStore, extractSchema, extractState, addEvent]);

  const processMessageImplRef = useRef(processMessageImpl);
  processMessageImplRef.current = processMessageImpl;
  const drainQueueRef = useRef<(() => void) | null>(null);

  const scheduleDrain = useCallback(() => {
    if (drainScheduledRef.current) return;
    drainScheduledRef.current = true;
    setTimeout(() => {
      drainScheduledRef.current = false;
      drainQueueRef.current?.();
    }, 0);
  }, []);

  const drainQueue = useCallback(() => {
    if (drainingRef.current) return;
    drainingRef.current = true;
    try {
      const start = nowMs();
      while (pendingQueueRef.current.length > 0) {
        const item = pendingQueueRef.current[0];
        if (item.kind === 'message') {
          const decoded = item.decoded;
          if (decoded.type === 'event_batch' && decoded.message.eventBatch?.events) {
            pendingQueueRef.current.shift();
            pendingQueueRef.current.unshift({
              kind: 'event_batch',
              batch: decoded,
              events: decoded.message.eventBatch.events as unknown[],
              index: 0,
            });
          } else {
            pendingQueueRef.current.shift();
            processMessageImplRef.current(decoded);
          }
        } else {
          const batchItem = item;
          const end = Math.min(batchItem.events.length, batchItem.index + EVENT_BATCH_CHUNK_SIZE);
          for (; batchItem.index < end; batchItem.index++) {
            processMessageImplRef.current(buildInnerEventMessageFromBatch(batchItem.batch, batchItem.events[batchItem.index]));
            if (nowMs() - start > MESSAGE_DRAIN_TIME_BUDGET_MS) {
              scheduleDrain();
              return;
            }
          }
          if (batchItem.index >= batchItem.events.length) {
            pendingQueueRef.current.shift();
          }
        }

        if (nowMs() - start > MESSAGE_DRAIN_TIME_BUDGET_MS) {
          scheduleDrain();
          return;
        }
      }
    } finally {
      drainingRef.current = false;
    }
  }, [scheduleDrain]);

  drainQueueRef.current = drainQueue;

  // Process incoming message
  // M-693: Uses string sequences for precision
  const processMessage = useCallback((decoded: DecodedMessage) => {
    // Quarantine messages missing thread_id instead of merging into "default"
    // M-693: QuarantinedMessage.seq is number for compatibility, use synthetic counter
    // M-1108: Store bounded summary instead of full message to prevent OOM
    if (!decoded.threadId) {
      const syntheticSeq = Number(nextSyntheticSeqRef.current);
      nextSyntheticSeqRef.current = nextSyntheticSeqRef.current - BigInt(1);

      // M-1108: Create bounded summary instead of storing full message
      const quarantined = createQuarantineSummary(
        decoded,
        syntheticSeq,
        'Missing thread_id - unbound telemetry'
      );

      // M-1108: Estimate summary size (much smaller than original, but still track)
      const summarySize =
        100 + // Base overhead
        (quarantined.messageIdPrefix?.length || 0) +
        (quarantined.attributeKeys?.reduce((acc, k) => acc + k.length, 0) || 0) +
        quarantined.reason.length;

      // M-1108: Evict oldest if exceeding byte budget OR count limit
      while (
        (quarantineBytesRef.current + summarySize > MAX_QUARANTINE_SIZE_BYTES ||
          quarantineRef.current.length >= MAX_QUARANTINE) &&
        quarantineRef.current.length > 0
      ) {
        const evicted = quarantineRef.current.shift();
        if (evicted) {
          // Estimate removed size
          const evictedSize =
            100 +
            (evicted.messageIdPrefix?.length || 0) +
            (evicted.attributeKeys?.reduce((acc, k) => acc + k.length, 0) || 0) +
            evicted.reason.length;
          quarantineBytesRef.current = Math.max(0, quarantineBytesRef.current - evictedSize);
        }
      }

      quarantineRef.current.push(quarantined);
      quarantineBytesRef.current += summarySize;

      console.warn('[RunStateStore] Quarantined message without thread_id:', decoded.type);
      return;
    }

    pendingQueueRef.current.push({ kind: 'message', decoded });
    drainQueue();
  }, [drainQueue]);

  // Get state at a specific sequence number
  // M-121: Handles invalid seeks by clamping to valid range and warning
  // M-693: Uses string sequences for precision
  const getStateAt = useCallback((threadId: string, targetSeq: string): Record<string, unknown> => {
    const store = runsRef.current.get(threadId);
    if (!store) return {};
    if (store.events.length === 0) return store.latestState;

    // M-121: Validate targetSeq is within reconstructable range
    const oldestEventSeq = store.events[0].seq;
    const latestEventSeq = store.events[store.events.length - 1].seq;

    // Clamp targetSeq to valid range
    // M-693: Use BigInt comparison
    let effectiveTargetSeq = targetSeq;
    if (compareSeqs(targetSeq, oldestEventSeq) < 0) {
      console.warn(
        `[RunStateStore] Seek to seq=${targetSeq} is before oldest event seq=${oldestEventSeq}. ` +
        `Clamping to oldest available.`
      );
      effectiveTargetSeq = oldestEventSeq;
    } else if (compareSeqs(targetSeq, latestEventSeq) > 0) {
      console.warn(
        `[RunStateStore] Seek to seq=${targetSeq} is after latest event seq=${latestEventSeq}. ` +
        `Clamping to latest available.`
      );
      effectiveTargetSeq = latestEventSeq;
    }

    // Find nearest checkpoint at or before effectiveTargetSeq
    // M-693: checkpointSeq is string; use null for "not found"
    let checkpointSeq: string | null = null;
    let state: Record<string, unknown> = {};

    // Sorted checkpoint search (from highest to lowest that's <= target)
    // M-693: Use compareSeqs for sorting
    const sortedCheckpoints = Array.from(store.checkpoints.entries())
      .filter(([seq]) => compareSeqs(seq, effectiveTargetSeq) <= 0)
      .sort(([a], [b]) => compareSeqs(b, a)); // Descending - want highest valid checkpoint

    if (sortedCheckpoints.length > 0) {
      const [nearestSeq, checkpoint] = sortedCheckpoints[0];
      checkpointSeq = nearestSeq;
      // M-808: Handle clone failure gracefully - fall back to empty state
      try {
        state = deepCloneJson(checkpoint);
      } catch (e) {
        if (e instanceof CloneError) {
          console.warn(
            `[RunStateStore] Failed to clone checkpoint at seq=${nearestSeq} for time-travel: ${e.message}. ` +
            'Using empty state as base.'
          );
          checkpointSeq = null; // Reset to allow full event replay
        } else {
          throw e;
        }
      }
    } else {
      // M-121: No valid checkpoint - this shouldn't happen if events exist
      // Fall back to oldest event's starting state or empty
      console.warn(
        `[RunStateStore] No checkpoint found at or before seq=${effectiveTargetSeq}. ` +
        `State reconstruction may be incomplete.`
      );
      // Try to use the store's current state as base if checkpoint is at "0" (initial)
      if (store.checkpoints.has('0')) {
        // M-808: Handle clone failure gracefully - fall back to empty state
        try {
          state = deepCloneJson(store.checkpoints.get('0')!);
          checkpointSeq = '0';
        } catch (e) {
          if (e instanceof CloneError) {
            console.warn(`[RunStateStore] Failed to clone initial checkpoint for time-travel: ${e.message}`);
          } else {
            throw e;
          }
        }
      }
    }

    // Apply events from checkpoint to target
    for (const event of store.events) {
      // M-693: Use string comparison
      if (checkpointSeq !== null && compareSeqs(event.seq, checkpointSeq) <= 0) continue;
      if (compareSeqs(event.seq, effectiveTargetSeq) > 0) break;

      if (event.operations) {
        try {
          state = applyPatch(state, event.operations) as Record<string, unknown>;
        } catch (e) {
          console.error(
            `[RunStateStore] Failed to apply patch at seq=${event.seq}:`,
            e
          );
          // Continue with best-effort state reconstruction
        }
      }
    }

    return state;
  }, []);

  // Get node states at a specific sequence number
  // M-121: Clamps targetSeq to valid range similar to getStateAt
  // M-693: Uses string sequences for precision
  const getNodeStatesAt = useCallback((threadId: string, targetSeq: string): Map<string, NodeState> => {
    const store = runsRef.current.get(threadId);
    if (!store) return new Map();
    if (store.events.length === 0) return new Map(store.nodeStates);

    const nodeStates = new Map<string, NodeState>();

    // M-121: Clamp targetSeq to valid event range
    // M-693: Use string comparisons
    const oldestEventSeq = store.events[0].seq;
    const latestEventSeq = store.events[store.events.length - 1].seq;
    let effectiveTargetSeq = targetSeq;
    if (compareSeqs(targetSeq, oldestEventSeq) < 0) {
      effectiveTargetSeq = oldestEventSeq;
    } else if (compareSeqs(targetSeq, latestEventSeq) > 0) {
      effectiveTargetSeq = latestEventSeq;
    }

    // Replay events to build node states at target
    for (const event of store.events) {
      if (compareSeqs(event.seq, effectiveTargetSeq) > 0) break;

      if (isNodeLifecycleEvent(event.eventType) && event.nodeId) {
        if (event.eventType === EventType.EVENT_TYPE_NODE_START) {
          nodeStates.set(event.nodeId, {
            status: 'active',
            startTime: event.timestamp,
            startSeq: event.seq,
          });
        } else if (event.eventType === EventType.EVENT_TYPE_NODE_END) {
          const existing = nodeStates.get(event.nodeId);
          if (existing) {
            existing.status = 'completed';
            existing.endTime = event.timestamp;
            existing.endSeq = event.seq;
            // M-1060: Prefer producer's duration_us if available
            const producerDurationUs = getNumberAttribute(event.attributes, 'duration_us');
            if (producerDurationUs !== undefined && producerDurationUs >= 0) {
              existing.durationMs = producerDurationUs / 1000;
            } else {
              const computedMs = existing.startTime
                ? event.timestamp - existing.startTime
                : undefined;
              existing.durationMs = computedMs !== undefined ? Math.max(0, computedMs) : undefined;
            }
          }
        } else if (event.eventType === EventType.EVENT_TYPE_NODE_ERROR) {
          const existing = nodeStates.get(event.nodeId);
          if (existing) {
            existing.status = 'error';
            existing.endTime = event.timestamp;
            existing.endSeq = event.seq;
            // M-1091 FIX: Use getStringAttribute for wrapper safety + truncate for UI safety
            const rawError = getStringAttribute(event.attributes as Record<string, unknown>, 'error');
            existing.error = rawError && rawError.length > 2000 ? rawError.slice(0, 2000) + '... [truncated]' : rawError;
          }
        }
      }
    }

    return nodeStates;
  }, []);

  // M-121: Get valid seek range for a run
  // M-693: Returns string sequences for precision
  const getSeekRange = useCallback((threadId: string): SeekRange | null => {
    const store = runsRef.current.get(threadId);
    if (!store || store.events.length === 0) return null;

    const oldestSeq = store.events[0].seq;
    const latestSeq = store.events[store.events.length - 1].seq;

    // Find oldest checkpoint that can be used for reconstruction
    // Must be <= oldestSeq (otherwise we can't replay events between checkpoint and oldest)
    // M-693: Use string sequences with BigInt comparison
    let oldestCheckpointSeq = oldestSeq; // Default: can only start from oldest event
    const checkpointSeqs = Array.from(store.checkpoints.keys()).sort(compareSeqs);
    for (const seq of checkpointSeqs) {
      if (compareSeqs(seq, oldestSeq) <= 0) {
        oldestCheckpointSeq = seq;
        break; // Found the oldest valid checkpoint
      }
    }

    return {
      oldestSeq,
      latestSeq,
      oldestCheckpointSeq,
    };
  }, []);

  // M-121: Check if a seek target is valid (within reconstructable range)
  // M-693: Uses string sequences for precision
  const isSeekValid = useCallback((threadId: string, targetSeq: string): boolean => {
    const range = getSeekRange(threadId);
    if (!range) return false;

    // Valid if targetSeq is within the range where we have events
    // AND we have a checkpoint that covers it (checkpoint <= targetSeq)
    // M-693: Use BigInt comparison
    const hasValidCheckpoint = compareSeqs(range.oldestCheckpointSeq, targetSeq) <= 0;
    const hasEventsCoverage = compareSeqs(targetSeq, range.oldestSeq) >= 0 &&
                              compareSeqs(targetSeq, range.latestSeq) <= 0;

    // Special case: if checkpoint is at targetSeq, we can reconstruct even without events
    const store = runsRef.current.get(threadId);
    if (store?.checkpoints.has(targetSeq)) {
      return compareSeqs(targetSeq, range.latestSeq) <= 0;
    }

    return hasValidCheckpoint && hasEventsCoverage;
  }, [getSeekRange]);

  // M-121: Clamp a sequence number to valid seek range
  // M-693: Uses string sequences for precision
  const clampSeq = useCallback((threadId: string, targetSeq: string): string => {
    const range = getSeekRange(threadId);
    if (!range) return targetSeq; // No data, return as-is

    // Clamp to valid range
    // M-693: Use BigInt comparison
    if (compareSeqs(targetSeq, range.oldestSeq) < 0) {
      console.debug(`[RunStateStore] Clamping seq ${targetSeq} to oldest valid ${range.oldestSeq}`);
      return range.oldestSeq;
    }
    if (compareSeqs(targetSeq, range.latestSeq) > 0) {
      console.debug(`[RunStateStore] Clamping seq ${targetSeq} to latest ${range.latestSeq}`);
      return range.latestSeq;
    }
    return targetSeq;
  }, [getSeekRange]);

  // Get view model at current cursor
  const getViewModel = useCallback((): GraphViewModel | null => {
    if (!cursor) return null;

    const store = runsRef.current.get(cursor.threadId);
    if (!store) return null;

    // Get state and node states at cursor
    const state = isLive
      ? store.latestState
      : getStateAt(cursor.threadId, cursor.seq);

    const nodeStates = isLive
      ? store.nodeStates
      : getNodeStatesAt(cursor.threadId, cursor.seq);

    // Find changed paths at cursor
    const cursorEvent = store.events.find(e => e.seq === cursor.seq);
    const changedPaths = cursorEvent?.changedPaths || [];

    // Find current node at cursor
    let currentNode: string | undefined;
    if (isLive) {
      currentNode = store.currentNode;
    } else {
      // Find last active node at cursor
      // M-693: Use string comparison
      for (const event of store.events) {
        if (compareSeqs(event.seq, cursor.seq) > 0) break;
        if (event.eventType === EventType.EVENT_TYPE_NODE_START) {
          currentNode = event.nodeId;
        } else if (event.eventType === EventType.EVENT_TYPE_NODE_END && event.nodeId === currentNode) {
          currentNode = undefined;
        }
      }
    }

    // M-39: Compute out-of-schema nodes (observed but not in schema)
    const schemaNodeNames = new Set(store.schema?.nodes.map(n => n.name) || []);
    const outOfSchemaNodes = new Set<string>();
    for (const nodeName of store.observedNodes) {
      if (!schemaNodeNames.has(nodeName)) {
        outOfSchemaNodes.add(nodeName);
      }
    }

    return {
      schema: store.schema,
      schemaId: store.schemaId,
      nodeStates,
      currentNode,
      state,
      changedPaths,
      cursor,
      isLive,
      observedNodes: store.observedNodes,
      outOfSchemaNodes,
    };
  }, [cursor, isLive, getStateAt, getNodeStatesAt]);

  // Public cursor setter
  // M-121: Validates cursor and warns if out of valid range (but still sets it)
  // M-693: Uses string sequences for precision
  const setCursor = useCallback((newCursor: RunCursor) => {
    const store = runsRef.current.get(newCursor.threadId);
    if (store && store.events.length > 0) {
      const oldestSeq = store.events[0].seq;
      const latestSeq = store.events[store.events.length - 1].seq;

      // M-693: Use string comparison
      if (compareSeqs(newCursor.seq, oldestSeq) < 0) {
        console.warn(
          `[RunStateStore] setCursor: seq=${newCursor.seq} is before oldest event seq=${oldestSeq}. ` +
          `State reconstruction will use oldest available state.`
        );
      } else if (compareSeqs(newCursor.seq, latestSeq) > 0) {
        console.warn(
          `[RunStateStore] setCursor: seq=${newCursor.seq} is after latest event seq=${latestSeq}. ` +
          `State reconstruction will use latest available state.`
        );
      }
    }
    setCursorState(newCursor);
    setIsLive(false);
  }, []);

  // Set live mode (cursor follows latest event)
  const setLiveMode = useCallback((live: boolean) => {
    setIsLive(live);
    if (live && cursor) {
      // Jump to latest event in current run
      const store = runsRef.current.get(cursor.threadId);
      if (store && store.events.length > 0) {
        const latestEvent = store.events[store.events.length - 1];
        setCursorState({ threadId: cursor.threadId, seq: latestEvent.seq });
      }
    }
  }, [cursor]);

  // Get list of runs
  const getRuns = useCallback((): string[] => {
    return Array.from(runsRef.current.keys());
  }, []);

  // Get runs sorted by recency with display-friendly labels
  const getRunsSorted = useCallback((): RunInfo[] => {
    const runs: RunInfo[] = [];

    for (const store of runsRef.current.values()) {
      runs.push({
        threadId: store.threadId,
        graphName: store.graphName,
        status: store.status,
        startTime: store.startTime,
        endTime: store.endTime,
        eventCount: store.events.length,
        label: generateRunLabel(store),
        corrupted: store.corrupted,
        needsResync: store.needsResync, // M-696: Checkpoint chain verification
        patchApplyFailed: store.patchApplyFailed, // M-704: Patch apply failure
        patchApplyFailedSeq: store.patchApplyFailedSeq, // M-730: Which seq caused failure
        // M-116: Include corruption details for diagnostics
        corruptionDetails: store.corruptionDetails,
      });
    }

    // Sort by startTime descending (most recent first)
    runs.sort((a, b) => b.startTime - a.startTime);

    return runs;
  }, []);

  // Get run store
  const getRunStore = useCallback((threadId: string): RunStateStore | undefined => {
    return runsRef.current.get(threadId);
  }, []);

  // Access quarantined messages (unbound telemetry)
  const getQuarantined = useCallback(() => {
    return [...quarantineRef.current];
  }, []);

  const clearQuarantine = useCallback(() => {
    quarantineRef.current = [];
    // M-1108: Reset byte counter when clearing quarantine
    quarantineBytesRef.current = 0;
  }, []);

  // M-711: Mark runs as needing resync when gap or cursor_stale signals are received.
  // M-788: Mark ALL runs (not just running ones) since completed/error runs may have
  // received corrupted state before they finished. Gap/stale signals are connection-level,
  // so we can't know which specific runs were affected - must mark all as potentially corrupted.
  const markActiveRunsNeedResync = useCallback((reason: string) => {
    let markedCount = 0;
    for (const store of runsRef.current.values()) {
      // M-788: Mark all runs regardless of status. Even completed/error runs might have
      // received corrupted data before finishing. The only runs to skip are those already
      // marked needsResync (to avoid duplicate logging).
      if (!store.needsResync) {
        store.needsResync = true;
        // Also mark as corrupted since state cannot be trusted
        store.corrupted = true;
        markedCount++;
        console.warn(
          `[useRunStateStore] Marked run thread=${store.threadId} (${store.graphName}) status=${store.status} as needing resync. ` +
          `Reason: ${reason}. State is now untrusted until a full snapshot/checkpoint arrives.`
        );
      }
    }
    if (markedCount > 0) {
      // Trigger UI update so corruption banners appear
      setHashVerificationVersion(v => v + 1);
    }
  }, []);

  // M-744: Clear all run state for cursor_reset recovery
  // This completely resets the UI state to start fresh after a cursor reset
  const clearAllRuns = useCallback(() => {
    const runCount = runsRef.current.size;
    runsRef.current.clear();
    quarantineRef.current = [];
    // Reset cursor to null (will go back to live mode)
    setCursorState(null);
    setIsLive(true);
    // Reset synthetic sequence counter
    nextSyntheticSeqRef.current = BigInt(-1);
    // Trigger UI update
    setHashVerificationVersion(v => v + 1);
    console.info(
      `[useRunStateStore] Cleared all run state (${runCount} runs removed). ` +
      `UI reset for clean recovery after cursor_reset.`
    );
  }, []);

  return {
    processMessage,
    getRuns,
    getRunsSorted,
    getRunStore,
    getQuarantined,
    clearQuarantine,
    cursor,
    setCursor,
    setLiveMode,
    isLive,
    getViewModel,
    getStateAt,
    getNodeStatesAt,
    // M-121: Time-travel validity checks
    getSeekRange,
    isSeekValid,
    clampSeq,
    // M-450: Hash verification version for UI re-renders
    hashVerificationVersion,
    // M-711: Mark runs needing resync due to gap/stale cursor
    markActiveRunsNeedResync,
    // M-744: Clear all runs for cursor_reset recovery
    clearAllRuns,
  };
}

// M-2591: Export internal utility functions for unit testing.
// These are pure functions with complex logic that benefit from direct testing.
// Exported separately from the main hook to allow testing without React rendering.
export {
  deepCloneJson as _deepCloneJson,
  hashesEqual as _hashesEqual,
  compareSeqs as _compareSeqs,
  isRealSeq as _isRealSeq,
  bytesToHex as _bytesToHex,
  hasValidCheckpointId as _hasValidCheckpointId,
  MAX_BYTES_FOR_HEX as _MAX_BYTES_FOR_HEX,
};
