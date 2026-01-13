// Protobuf decoder for DashStream messages
// Generated types and decoder for dashstream.v1 protocol

import * as protobuf from 'protobufjs';
import { decompress } from 'fzstd';
import protoSchema from './dashstream.schema.json';

// Debug flag for decoder logging
// Set to true to enable verbose logging, or check window.DEBUG_DASHSTREAM
const DEBUG_DASHSTREAM = typeof window !== 'undefined' &&
  (window as unknown as { DEBUG_DASHSTREAM?: boolean }).DEBUG_DASHSTREAM === true;

// M-976: Expected schema version for the UI. This should match CURRENT_SCHEMA_VERSION
// from crates/dashflow-streaming/src/lib.rs. When the server sends a schemaVersion
// greater than this, the UI should warn and prevent silent misinterpretation.
export const EXPECTED_SCHEMA_VERSION = 1;

// M-978: Maximum decompressed message size (bytes). Matches server's DEFAULT_MAX_PAYLOAD_SIZE.
// Protects against OOM/DoS from malformed or malicious compressed data.
// M-1006: Exported so decode.worker.ts can import from single source of truth.
export const MAX_DECOMPRESSED_SIZE = 10 * 1024 * 1024; // 10 MB

/**
 * M-1097: Parse zstd frame header to get declared content size BEFORE decompression.
 * This prevents "decompression bomb" attacks where a small compressed input expands
 * to a huge output, causing OOM/freeze. By checking the declared size in the header
 * before calling decompress(), we avoid allocating the oversized buffer.
 *
 * Returns:
 * - { valid: true, contentSize: number } if content size is declared and parseable
 * - { valid: true, contentSize: null } if content size is unknown (streaming frame)
 * - { valid: false, contentSize: null } if not a valid zstd frame
 *
 * Reference: RFC 8878 (Zstandard Compression and the 'application/zstd' Media Type)
 */
export function parseZstdFrameHeader(data: Uint8Array): { valid: boolean; contentSize: number | null } {
  // Minimum frame size: 4 (magic) + 1 (descriptor) = 5 bytes for header
  if (data.length < 5) {
    return { valid: false, contentSize: null };
  }

  // Check magic number (0xFD2FB528, little endian)
  // Use >>> 0 to convert to unsigned 32-bit (JS bitwise ops use signed 32-bit)
  const magic = (data[0] | (data[1] << 8) | (data[2] << 16) | (data[3] << 24)) >>> 0;
  if (magic !== 0xFD2FB528) {
    return { valid: false, contentSize: null };
  }

  const descriptor = data[4];

  // Extract frame header fields from Frame_Header_Descriptor
  const fcsFlag = (descriptor >> 6) & 0x03;          // Bits 7-6: Frame_Content_Size_flag
  const singleSegmentFlag = (descriptor >> 5) & 0x01; // Bit 5: Single_Segment_flag
  // Bits 4-3: reserved (should be 0)
  // Bit 2: Content_Checksum_flag
  const dictIdFlag = descriptor & 0x03;              // Bits 1-0: Dictionary_ID_flag

  // Calculate Window_Descriptor size (0 if Single_Segment_flag is set)
  const windowDescSize = singleSegmentFlag ? 0 : 1;

  // Calculate Dictionary_ID size based on flag
  const dictIdSizes = [0, 1, 2, 4];
  const dictIdSize = dictIdSizes[dictIdFlag];

  // Calculate Frame_Content_Size field size
  let fcsSize: number;
  if (fcsFlag === 0) {
    fcsSize = singleSegmentFlag ? 1 : 0;
  } else if (fcsFlag === 1) {
    fcsSize = 2;
  } else if (fcsFlag === 2) {
    fcsSize = 4;
  } else {
    fcsSize = 8;
  }

  // Offset to Frame_Content_Size field
  const fcsOffset = 5 + windowDescSize + dictIdSize;

  // Check if we have enough bytes for the header
  if (data.length < fcsOffset + fcsSize) {
    return { valid: false, contentSize: null };
  }

  // If no content size field, return unknown
  if (fcsSize === 0) {
    return { valid: true, contentSize: null };
  }

  // Read Frame_Content_Size (little endian)
  let contentSize: number;
  if (fcsSize === 1) {
    contentSize = data[fcsOffset];
  } else if (fcsSize === 2) {
    contentSize = data[fcsOffset] | (data[fcsOffset + 1] << 8);
    contentSize += 256; // FCS_Field_Size=1 adds 256 offset per RFC
  } else if (fcsSize === 4) {
    contentSize = (
      data[fcsOffset] |
      (data[fcsOffset + 1] << 8) |
      (data[fcsOffset + 2] << 16) |
      (data[fcsOffset + 3] << 24)
    ) >>> 0; // >>> 0 to treat as unsigned 32-bit
  } else {
    // 8 bytes - use BigInt for safety, check if exceeds MAX_SAFE_INTEGER
    const low = (
      data[fcsOffset] |
      (data[fcsOffset + 1] << 8) |
      (data[fcsOffset + 2] << 16) |
      (data[fcsOffset + 3] << 24)
    ) >>> 0;
    const high = (
      data[fcsOffset + 4] |
      (data[fcsOffset + 5] << 8) |
      (data[fcsOffset + 6] << 16) |
      (data[fcsOffset + 7] << 24)
    ) >>> 0;

    // If high bits are set, content size exceeds what we can safely handle
    if (high > 0) {
      // Return a value that will definitely exceed any reasonable limit
      return { valid: true, contentSize: Number.MAX_SAFE_INTEGER };
    }
    contentSize = low;
  }

  return { valid: true, contentSize };
}

// M-469: Safe BigInt to Number conversion with overflow warning.
// JavaScript Numbers lose precision beyond 2^53 (MAX_SAFE_INTEGER = 9,007,199,254,740,991).
// For timestamps (~1.7e15 us) and sequence numbers, this is currently safe but we
// warn if values exceed this threshold to prevent silent data corruption.
// M-1006: Exported so decode.worker.ts can import from single source of truth.
const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;
export function safeToNumber(value: bigint | undefined, fieldName: string): number | undefined {
  if (value === undefined) return undefined;
  const numValue = Number(value);
  if (value > BigInt(MAX_SAFE_INTEGER)) {
    console.warn(
      `[DashStream] ${fieldName} value ${value} exceeds MAX_SAFE_INTEGER (${MAX_SAFE_INTEGER}). ` +
      'Precision may be lost. Consider using BigInt throughout if this becomes common.'
    );
  }
  return numValue;
}

// M-693: Sequences stored as strings to prevent precision loss for values > 2^53.
// M-1068: Real producer sequences are strictly > 0. seq==0 means "missing" (proto3 default).
// Returns string representation of positive sequences, undefined for 0/negative/undefined.
// M-1006: Exported so decode.worker.ts can import from single source of truth.
export function safeNonNegativeSequenceString(value: bigint | undefined): string | undefined {
  if (value === undefined) return undefined;
  // M-1068: Treat 0 as missing (aligns with server: sequence > 0).then_some())
  if (value <= BigInt(0)) return undefined;
  return value.toString();
}

// M-977: Coerce u64-like values (bigint, number, string, Long) to string for precision-safe comparison.
// This handles protobufjs Long objects which have .toNumber() and can lose precision if compared directly.
// M-1068: Returns undefined for 0/negative/invalid values (seq==0 means "missing" per proto3 defaults).
// M-1006: Exported so decode.worker.ts can import from single source of truth.
export function coerceU64ToStr(value: unknown): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value === 'bigint') {
    // M-1068: Treat 0 as missing
    return value > BigInt(0) ? value.toString() : undefined;
  }
  if (typeof value === 'number') {
    // M-1068: Treat 0 as missing
    return Number.isFinite(value) && Number.isInteger(value) && value > 0
      ? value.toString()
      : undefined;
  }
  if (typeof value === 'string') {
    // Validate and normalize: only digits, convert to BigInt then back to string
    if (/^\d+$/.test(value)) {
      try {
        const bi = BigInt(value);
        // M-1068: Treat 0 as missing
        return bi > BigInt(0) ? bi.toString() : undefined;
      } catch {
        return undefined;
      }
    }
    return undefined;
  }
  // Handle protobufjs Long-like objects: { low: number, high: number, unsigned: boolean, toNumber?: () => number }
  if (typeof value === 'object' && value !== null) {
    // Check for Long.toString() which is lossless
    const maybeToString = (value as { toString?: () => string }).toString;
    if (typeof maybeToString === 'function') {
      // Longs have both low/high properties
      const obj = value as { low?: unknown; high?: unknown; unsigned?: boolean };
      if (typeof obj.low === 'number' && typeof obj.high === 'number') {
        // This looks like a Long - use toString() which is lossless
        const str = maybeToString.call(value);
        if (/^-?\d+$/.test(str)) {
          // Check positive (M-1068: 0 is "missing")
          if (str.startsWith('-') || str === '0') return undefined;
          return str;
        }
      }
    }
  }
  return undefined;
}

// Event types from dashstream.proto
export enum EventType {
  EVENT_TYPE_UNSPECIFIED = 0,
  EVENT_TYPE_GRAPH_START = 1,
  EVENT_TYPE_GRAPH_END = 2,
  EVENT_TYPE_GRAPH_ERROR = 3,
  EVENT_TYPE_NODE_START = 10,
  EVENT_TYPE_NODE_END = 11,
  EVENT_TYPE_NODE_ERROR = 12,
  EVENT_TYPE_EDGE_TRAVERSAL = 20,
  EVENT_TYPE_CONDITIONAL_BRANCH = 21,
  EVENT_TYPE_PARALLEL_START = 22,
  EVENT_TYPE_PARALLEL_END = 23,
  EVENT_TYPE_LLM_START = 30,
  EVENT_TYPE_LLM_END = 31,
  EVENT_TYPE_LLM_ERROR = 32,
  EVENT_TYPE_LLM_RETRY = 33,
  EVENT_TYPE_TOOL_START = 40,
  EVENT_TYPE_TOOL_END = 41,
  EVENT_TYPE_TOOL_ERROR = 42,
  EVENT_TYPE_CHECKPOINT_SAVE = 50,
  EVENT_TYPE_CHECKPOINT_LOAD = 51,
  EVENT_TYPE_CHECKPOINT_DELETE = 52,
  EVENT_TYPE_MEMORY_SAVE = 60,
  EVENT_TYPE_MEMORY_LOAD = 61,
  EVENT_TYPE_HUMAN_INTERRUPT = 70,
  EVENT_TYPE_HUMAN_RESUME = 71,
  EVENT_TYPE_NODE_PROGRESS = 80,
  EVENT_TYPE_NODE_THINKING = 81,
  EVENT_TYPE_NODE_SUBSTEP = 82,
  EVENT_TYPE_NODE_WARNING = 83,
  EVENT_TYPE_OPTIMIZATION_START = 90,
  EVENT_TYPE_OPTIMIZATION_END = 91,
}

// Message types
export enum MessageType {
  MESSAGE_TYPE_UNSPECIFIED = 0,
  MESSAGE_TYPE_EVENT = 1,
  MESSAGE_TYPE_STATE_DIFF = 2,
  MESSAGE_TYPE_TOKEN_CHUNK = 3,
  MESSAGE_TYPE_TOOL_EXECUTION = 4,
  MESSAGE_TYPE_CHECKPOINT = 5,
  MESSAGE_TYPE_METRICS = 6,
  MESSAGE_TYPE_ERROR = 7,
  MESSAGE_TYPE_EVENT_BATCH = 8,
  MESSAGE_TYPE_EXECUTION_TRACE = 9,
}

// TypeScript interfaces matching the protobuf schema
export interface Header {
  messageId: Uint8Array;
  timestampUs: bigint;
  tenantId: string;
  threadId: string;
  sequence: bigint;
  type: MessageType;
  parentId: Uint8Array;
  compression: number;
  schemaVersion: number;
}

export interface AttributeValue {
  stringValue?: string;
  intValue?: bigint;
  floatValue?: number;
  boolValue?: boolean;
  bytesValue?: Uint8Array;
}

export interface Event {
  header?: Header;
  eventType: EventType;
  nodeId: string;
  attributes: Record<string, AttributeValue>;
  durationUs: bigint;
  llmRequestId: string;
}

export interface StateDiff {
  header?: Header;
  baseCheckpointId: Uint8Array;
  operations: DiffOperation[];
  stateHash: Uint8Array;
  fullState: Uint8Array;
}

export interface DiffOperation {
  op: number;
  path: string;
  value: Uint8Array;
  from: string;
  encoding: number;
}

export interface TokenChunk {
  header?: Header;
  requestId: string;
  text: string;
  tokenIds: number[];
  logprobs: number[];
  chunkIndex: number;
  isFinal: boolean;
  finishReason: number;
  model: string;
}

export interface ToolExecution {
  header?: Header;
  callId: string;
  toolName: string;
  stage: number;
  arguments: Uint8Array;
  result: Uint8Array;
  error: string;
  durationUs: bigint;
  retryCount: number;
}

// M-696: Checkpoint contains full state snapshot for resync
export interface Checkpoint {
  header?: Header;
  checkpointId: Uint8Array; // UUID
  state: Uint8Array;        // Full state (compressed/serialized)
  stateType: string;        // Type hint for deserialization
}

export interface Metrics {
  header?: Header;
  scope: string;
  scopeId: string;
  metrics: Record<string, unknown>;
  tags: Record<string, string>;
}

export interface ProtoError {
  header?: Header;
  errorCode: string;
  message: string;
  stackTrace: string;
  context: Record<string, string>;
  severity: number;
  exceptionType: string;
  suggestions: string[];
}

export interface EventBatch {
  header?: Header;
  events: Event[];
}

export interface NodeExecutionRecord {
  node: string;
  durationMs: bigint;
  promptTokens: bigint;
  completionTokens: bigint;
  totalTokens: bigint;
  succeeded: boolean;
  startedAt: string;
  endedAt: string;
  input: Uint8Array;
  output: Uint8Array;
  metadata: Record<string, string>;
}

export interface ErrorRecord {
  node: string;
  message: string;
  errorCode: string;
  timestamp: string;
  recovered: boolean;
  stackTrace: string;
}

export interface ExecutionTrace {
  header?: Header;
  threadId: string;
  executionId: string;
  nodesExecuted: NodeExecutionRecord[];
  totalDurationMs: bigint;
  totalTokens: bigint;
  errors: ErrorRecord[];
  completed: boolean;
  startedAt: string;
  endedAt: string;
  finalState: Uint8Array;
  metadata: Record<string, Uint8Array>;
}

export interface DashStreamMessage {
  event?: Event;
  stateDiff?: StateDiff;
  tokenChunk?: TokenChunk;
  toolExecution?: ToolExecution;
  checkpoint?: Checkpoint;
  metrics?: Metrics;
  error?: ProtoError;
  eventBatch?: EventBatch;
  executionTrace?: ExecutionTrace;
}

// Decoded message with type information
// M-693: sequence is string to prevent precision loss for values > 2^53 (MAX_SAFE_INTEGER)
// M-685: partition/offset added to track Kafka cursor for EventBatch inner events
// M-976: schemaVersion added for protocol compatibility validation
export interface DecodedMessage {
  type: 'event' | 'state_diff' | 'token_chunk' | 'tool_execution' | 'checkpoint' | 'metrics' | 'error' | 'event_batch' | 'execution_trace' | 'unknown';
  message: DashStreamMessage;
  timestamp: number;
  threadId?: string;
  sequence?: string; // M-693: string to handle sequences > MAX_SAFE_INTEGER
  // M-982: For event_batch, track max inner sequence per thread to persist accurate per-thread cursors.
  // Without this, a single batch containing multiple threadIds would only advance one thread cursor.
  sequencesByThread?: Record<string, string>;
  // M-685: Kafka cursor for resume/replay - allows EventBatch inner events to inherit cursor
  partition?: number;
  offset?: string;
  // M-976: Schema version from message header for protocol compatibility validation.
  // If schemaVersion > EXPECTED_SCHEMA_VERSION, the UI should warn about potential misinterpretation.
  schemaVersion?: number;
  // M-976: True if schemaVersion > EXPECTED_SCHEMA_VERSION (UI out of date).
  // When set, the caller should: (1) show a warning, (2) avoid committing cursors.
  schemaVersionMismatch?: boolean;
}

// Proto schema is loaded from generated JSON file
// To regenerate: npm run proto:gen
// To validate: npm run proto:check

// Singleton decoder instance
let decoder: DashStreamDecoder | null = null;

export class DashStreamDecoder {
  private root: protobuf.Root | null = null;
  private DashStreamMessage: protobuf.Type | null = null;
  private initialized = false;
  private initPromise: Promise<void> | null = null;

  async init(): Promise<void> {
    if (this.initialized) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = (async () => {
      try {
        // Load proto schema from generated JSON (source of truth: proto/dashstream.proto)
        this.root = protobuf.Root.fromJSON(protoSchema);
        this.DashStreamMessage = this.root.lookupType('dashstream.v1.DashStreamMessage');
        this.initialized = true;
        if (DEBUG_DASHSTREAM) console.log('[DashStreamDecoder] Initialized successfully from generated schema');
      } catch (error) {
        console.error('[DashStreamDecoder] Failed to initialize:', error);
        throw error;
      }
    })();

    return this.initPromise;
  }

  decode(buffer: Uint8Array): DecodedMessage | null {
    if (!this.initialized || !this.DashStreamMessage) {
      if (DEBUG_DASHSTREAM) console.warn('[DashStreamDecoder] Not initialized, returning null');
      return null;
    }

    try {
      // M-453: Robust header detection for DashStream protocol
      //
      // Header format: First byte indicates compression (0x00=none, 0x01=zstd)
      // Legacy format: No header, raw protobuf
      //
      // Disambiguation strategy:
      // - Valid protobuf field tags start at 0x08 (field 1, varint) since field 0 is reserved
      // - 0x00 and 0x01 are NEVER valid protobuf first bytes for any real message
      // - Therefore we can safely interpret 0x00/0x01 as headers
      //
      // However, to handle malformed data gracefully:
      // 1. If header byte detected, try header interpretation first
      // 2. If decode fails after header stripping, fallback to legacy (whole buffer)
      // 3. This catches edge cases like empty payloads or corrupt headers
      //
      let messageBuffer = buffer;
      let usedHeaderInterpretation = false;

      if (buffer.length > 1) {
        const headerByte = buffer[0];
        if (headerByte === 0x00) {
          // Uncompressed with header - strip header byte
          // M-978: Use subarray instead of slice to avoid needless copy
          messageBuffer = buffer.subarray(1);
          usedHeaderInterpretation = true;
          if (DEBUG_DASHSTREAM) console.debug('[DashStreamDecoder] Detected uncompressed header (0x00)');
        } else if (headerByte === 0x01) {
          // Zstd compressed - decompress using fzstd
          try {
            // M-978: Use subarray to avoid needless copy
            const compressedData = buffer.subarray(1);

            // M-1097: Check declared content size BEFORE decompression to prevent decompression bombs.
            // The previous code called decompress() first, then checked size - but the allocation
            // had already occurred, allowing OOM/freeze from malicious inputs.
            const frameHeader = parseZstdFrameHeader(compressedData);
            if (frameHeader.valid) {
              if (frameHeader.contentSize !== null) {
                // Content size is declared - check against limit before allocating
                if (frameHeader.contentSize > MAX_DECOMPRESSED_SIZE) {
                  console.error(
                    `[DashStreamDecoder] Zstd frame declares content size ${frameHeader.contentSize} which exceeds limit ${MAX_DECOMPRESSED_SIZE}. ` +
                    'Rejecting without decompression to prevent OOM (M-1097).'
                  );
                  return null;
                }
              } else {
                // Content size is unknown (streaming frame) - reject to be safe.
                // DashStream protocol always uses frames with declared content size.
                console.error(
                  '[DashStreamDecoder] Zstd frame has unknown content size (streaming frame). ' +
                  'DashStream requires frames with declared size. Rejecting to prevent decompression bomb (M-1097).'
                );
                return null;
              }
            }
            // Note: If frameHeader.valid is false, we still try decompression as fallback
            // (may be legacy format or non-standard zstd). Post-decompression check remains as safety net.

            messageBuffer = decompress(compressedData);
            // M-978: Post-decompression size check as safety net (e.g., if header parsing failed
            // and we fell through to decompression). This is defense-in-depth.
            if (messageBuffer.length > MAX_DECOMPRESSED_SIZE) {
              console.error(
                `[DashStreamDecoder] Decompressed size ${messageBuffer.length} exceeds limit ${MAX_DECOMPRESSED_SIZE}. ` +
                'Treating as protocol error to prevent OOM.'
              );
              return null;
            }
            usedHeaderInterpretation = true;
            if (DEBUG_DASHSTREAM) console.debug('[DashStreamDecoder] Decompressed zstd message, size:', messageBuffer.length);
          } catch (decompressError) {
            // M-453: Decompression failed - might be legacy format that happens to start with 0x01
            // Try decoding the original buffer as legacy fallback
            if (DEBUG_DASHSTREAM) {
              console.debug('[DashStreamDecoder] Zstd decompression failed, trying legacy format:', decompressError);
            }
            messageBuffer = buffer;
            usedHeaderInterpretation = false;
          }
        }
        // headerByte >= 0x08: Valid protobuf field tag, no header (legacy format)
      }

      // M-453: Try decode with fallback to original buffer if header interpretation fails
      let decoded: DashStreamMessage;
      try {
        decoded = this.DashStreamMessage.decode(messageBuffer) as unknown as DashStreamMessage;
      } catch (decodeError) {
        if (usedHeaderInterpretation && buffer.length > 1) {
          // Header interpretation failed - try legacy format (whole buffer)
          if (DEBUG_DASHSTREAM) {
            console.debug('[DashStreamDecoder] Header-based decode failed, trying legacy format:', decodeError);
          }
          try {
            decoded = this.DashStreamMessage.decode(buffer) as unknown as DashStreamMessage;
          } catch (legacyError) {
            // Both interpretations failed
            console.error('[DashStreamDecoder] All decode attempts failed:', {
              headerError: decodeError,
              legacyError: legacyError,
              bufferLength: buffer.length,
              firstByte: buffer[0]?.toString(16),
            });
            return null;
          }
        } else {
          // No fallback available
          throw decodeError;
        }
      }
      const now = Date.now();

      // M-976: Extract schema version from header and check for mismatch.
      // Helper to avoid duplicating logic across all message types.
      const extractSchemaVersionInfo = (header: Header | undefined): { schemaVersion?: number; schemaVersionMismatch?: boolean } => {
        const sv = header?.schemaVersion;
        if (sv === undefined || sv === null) {
          return {}; // No schema version in message (backwards compatible)
        }
        const numSv = typeof sv === 'number' ? sv : Number(sv);
        if (!Number.isFinite(numSv) || numSv < 0) {
          return {}; // Invalid schema version, ignore
        }
        const mismatch = numSv > EXPECTED_SCHEMA_VERSION;
        return { schemaVersion: numSv, schemaVersionMismatch: mismatch || undefined };
      };

      // Determine message type
      // M-469: Use safeToNumber() for BigInt conversions to detect overflow
      // M-693: Use safeNonNegativeSequenceString() for sequences to prevent precision loss
      if (decoded.event) {
        const header = decoded.event.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'event',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.eventBatch) {
        const header = decoded.eventBatch.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const headerThreadId = header?.threadId;
        const headerSeq = safeNonNegativeSequenceString(header?.sequence);

        // EventBatch header sequence is intentionally 0 (batch envelope); use the max inner event sequence
        // as a best-effort cursor for resume/replay correctness.
        // M-693: Use BigInt for comparison, return string for storage
        // M-1068: seq==0 is "missing" (proto3 default). Real sequences are > 0.
        // M-977: Use coerceU64ToStr to safely handle protobufjs Long objects without precision loss.
        //        Direct comparisons on Long via > or >= can coerce to number and lose precision > 2^53.
        let maxInnerSeqStr: string | undefined;
        let maxInnerSeqBigInt: bigint | undefined;
        let innerThreadId: string | undefined;
        // M-982: Track per-thread max sequence within the batch.
        const maxInnerSeqByThread: Record<string, string> = {};
        const maxInnerSeqByThreadBigInt: Record<string, bigint> = {};
        for (const event of decoded.eventBatch.events || []) {
          const eventHeader = event.header;
          // M-977: Convert sequence to string first, then to BigInt for comparison
          const seqStr = coerceU64ToStr(eventHeader?.sequence);
          const eventThreadId = eventHeader?.threadId;
          if (seqStr !== undefined) {
            // M-1068: coerceU64ToStr now filters 0 (missing) and negatives; only real sequences (> 0) counted
            try {
              const seqBigInt = BigInt(seqStr);
              if (maxInnerSeqBigInt === undefined || seqBigInt > maxInnerSeqBigInt) {
                maxInnerSeqBigInt = seqBigInt;
                maxInnerSeqStr = seqStr;
              }
              if (eventThreadId) {
                const prevBig = maxInnerSeqByThreadBigInt[eventThreadId];
                if (prevBig === undefined || seqBigInt > prevBig) {
                  maxInnerSeqByThreadBigInt[eventThreadId] = seqBigInt;
                  maxInnerSeqByThread[eventThreadId] = seqStr;
                }
              }
            } catch {
              // BigInt() failed (shouldn't happen after coerceU64ToStr validation), skip this event
            }
          }
          if (!innerThreadId && eventThreadId) {
            innerThreadId = eventThreadId;
          }
        }

        const effectiveThreadId = headerThreadId || innerThreadId;
        const sequence = maxInnerSeqStr !== undefined ? maxInnerSeqStr : headerSeq;
        const sequencesByThread =
          Object.keys(maxInnerSeqByThread).length > 0 ? maxInnerSeqByThread : undefined;
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'event_batch',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: effectiveThreadId,
          sequence,
          sequencesByThread,
          ...svInfo,
        };
      }

      if (decoded.stateDiff) {
        const header = decoded.stateDiff.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'state_diff',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.tokenChunk) {
        const header = decoded.tokenChunk.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'token_chunk',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.toolExecution) {
        const header = decoded.toolExecution.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'tool_execution',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.metrics) {
        const header = decoded.metrics.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'metrics',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.error) {
        const header = decoded.error.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'error',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.checkpoint) {
        const header = decoded.checkpoint.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'checkpoint',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      if (decoded.executionTrace) {
        const header = decoded.executionTrace.header;
        const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
        const svInfo = extractSchemaVersionInfo(header);
        return {
          type: 'execution_trace',
          message: decoded,
          timestamp: tsUs !== undefined ? tsUs / 1000 : now,
          threadId: header?.threadId || decoded.executionTrace.threadId,
          sequence: safeNonNegativeSequenceString(header?.sequence),
          ...svInfo,
        };
      }

      return {
        type: 'unknown',
        message: decoded,
        timestamp: now,
      };
    } catch (error) {
      console.error('[DashStreamDecoder] Failed to decode message:', error);
      return null;
    }
  }

  isInitialized(): boolean {
    return this.initialized;
  }
}

// Get or create singleton decoder
export async function getDecoder(): Promise<DashStreamDecoder> {
  if (!decoder) {
    decoder = new DashStreamDecoder();
    await decoder.init();
  }
  return decoder;
}

// Helper to get event type name
export function getEventTypeName(eventType: EventType): string {
  const names: Record<EventType, string> = {
    [EventType.EVENT_TYPE_UNSPECIFIED]: 'unspecified',
    [EventType.EVENT_TYPE_GRAPH_START]: 'graph_start',
    [EventType.EVENT_TYPE_GRAPH_END]: 'graph_end',
    [EventType.EVENT_TYPE_GRAPH_ERROR]: 'graph_error',
    [EventType.EVENT_TYPE_NODE_START]: 'node_start',
    [EventType.EVENT_TYPE_NODE_END]: 'node_end',
    [EventType.EVENT_TYPE_NODE_ERROR]: 'node_error',
    [EventType.EVENT_TYPE_EDGE_TRAVERSAL]: 'edge_traversal',
    [EventType.EVENT_TYPE_CONDITIONAL_BRANCH]: 'conditional_branch',
    [EventType.EVENT_TYPE_PARALLEL_START]: 'parallel_start',
    [EventType.EVENT_TYPE_PARALLEL_END]: 'parallel_end',
    [EventType.EVENT_TYPE_LLM_START]: 'llm_start',
    [EventType.EVENT_TYPE_LLM_END]: 'llm_end',
    [EventType.EVENT_TYPE_LLM_ERROR]: 'llm_error',
    [EventType.EVENT_TYPE_LLM_RETRY]: 'llm_retry',
    [EventType.EVENT_TYPE_TOOL_START]: 'tool_start',
    [EventType.EVENT_TYPE_TOOL_END]: 'tool_end',
    [EventType.EVENT_TYPE_TOOL_ERROR]: 'tool_error',
    [EventType.EVENT_TYPE_CHECKPOINT_SAVE]: 'checkpoint_save',
    [EventType.EVENT_TYPE_CHECKPOINT_LOAD]: 'checkpoint_load',
    [EventType.EVENT_TYPE_CHECKPOINT_DELETE]: 'checkpoint_delete',
    [EventType.EVENT_TYPE_MEMORY_SAVE]: 'memory_save',
    [EventType.EVENT_TYPE_MEMORY_LOAD]: 'memory_load',
    [EventType.EVENT_TYPE_HUMAN_INTERRUPT]: 'human_interrupt',
    [EventType.EVENT_TYPE_HUMAN_RESUME]: 'human_resume',
    [EventType.EVENT_TYPE_NODE_PROGRESS]: 'node_progress',
    [EventType.EVENT_TYPE_NODE_THINKING]: 'node_thinking',
    [EventType.EVENT_TYPE_NODE_SUBSTEP]: 'node_substep',
    [EventType.EVENT_TYPE_NODE_WARNING]: 'node_warning',
    [EventType.EVENT_TYPE_OPTIMIZATION_START]: 'optimization_start',
    [EventType.EVENT_TYPE_OPTIMIZATION_END]: 'optimization_end',
  };
  return names[eventType] || 'unknown';
}

// Helper to check if event is a node lifecycle event
export function isNodeLifecycleEvent(eventType: EventType): boolean {
  return eventType >= EventType.EVENT_TYPE_NODE_START && eventType <= EventType.EVENT_TYPE_NODE_ERROR;
}

// Helper to check if event is a graph lifecycle event
export function isGraphLifecycleEvent(eventType: EventType): boolean {
  return eventType >= EventType.EVENT_TYPE_GRAPH_START && eventType <= EventType.EVENT_TYPE_GRAPH_ERROR;
}
