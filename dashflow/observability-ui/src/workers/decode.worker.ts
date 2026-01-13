// M-998: Web Worker for decode/decompress to prevent main thread freezes
// This worker handles synchronous CPU-bound work (zstd decompression + protobuf decode)
// off the main thread, making it cancellable via worker termination on timeout.
//
// M-1006: This worker imports shared constants from ../proto/dashstream to avoid drift
// with the main decoder. Previously, EXPECTED_SCHEMA_VERSION and MAX_DECOMPRESSED_SIZE
// were duplicated here and could diverge from the canonical source.

import * as protobuf from 'protobufjs';
import { decompress } from 'fzstd';
import protoSchema from '../proto/dashstream.schema.json';
// M-1006: Import shared constants from canonical source to prevent drift
// M-1077: Import shared helpers to avoid duplicate implementations and semantic drift
// M-1097: Import parseZstdFrameHeader to check content size before decompression
import {
  EXPECTED_SCHEMA_VERSION,
  MAX_DECOMPRESSED_SIZE,
  safeToNumber,
  safeNonNegativeSequenceString,
  coerceU64ToStr,
  parseZstdFrameHeader,
} from '../proto/dashstream';

// Debug flag
const DEBUG_DASHSTREAM = false;

// M-1036: Error classification for decode failures
// These error types allow the UI to understand why decoding failed
// and potentially take different actions (e.g., retry vs. skip).
export const DecodeErrorType = {
  NOT_INITIALIZED: 'NOT_INITIALIZED',
  DECOMPRESSED_SIZE_EXCEEDED: 'DECOMPRESSED_SIZE_EXCEEDED',
  DECODE_FAILED: 'DECODE_FAILED',
  UNKNOWN_ERROR: 'UNKNOWN_ERROR',
} as const;
export type DecodeErrorTypeValue = typeof DecodeErrorType[keyof typeof DecodeErrorType];

// M-1077: safeToNumber, safeNonNegativeSequenceString, coerceU64ToStr are now imported
// from ../proto/dashstream to prevent drift between main-thread and worker decoders.

// Message types for worker communication
interface DecodeRequest {
  type: 'decode';
  id: number;
  buffer: ArrayBuffer;
}

interface InitRequest {
  type: 'init';
  id: number;
}

type WorkerRequest = DecodeRequest | InitRequest;

interface DecodeResponse {
  type: 'decode_result';
  id: number;
  result: DecodedMessageSerializable | null;
  /** M-1036: Human-readable error message */
  error?: string;
  /** M-1036: Machine-readable error classification for UI decision-making */
  errorType?: DecodeErrorTypeValue;
}

interface InitResponse {
  type: 'init_result';
  id: number;
  success: boolean;
  error?: string;
}

type WorkerResponse = DecodeResponse | InitResponse;

// Serializable version of DecodedMessage (no Uint8Array fields in protobuf message)
interface DecodedMessageSerializable {
  type: 'event' | 'state_diff' | 'token_chunk' | 'tool_execution' | 'checkpoint' | 'metrics' | 'error' | 'event_batch' | 'execution_trace' | 'unknown';
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  message: any; // Raw protobuf decoded message
  timestamp: number;
  threadId?: string;
  sequence?: string;
  sequencesByThread?: Record<string, string>;
  partition?: number;
  offset?: string;
  schemaVersion?: number;
  schemaVersionMismatch?: boolean;
}

// Decoder state
let root: protobuf.Root | null = null;
let DashStreamMessageType: protobuf.Type | null = null;
let initialized = false;

// Initialize the decoder
function initDecoder(): boolean {
  if (initialized) return true;

  try {
    root = protobuf.Root.fromJSON(protoSchema);
    DashStreamMessageType = root.lookupType('dashstream.v1.DashStreamMessage');
    initialized = true;
    if (DEBUG_DASHSTREAM) console.log('[DecodeWorker] Initialized successfully');
    return true;
  } catch (error) {
    console.error('[DecodeWorker] Failed to initialize:', error);
    return false;
  }
}

// Extract schema version info from header
interface Header {
  messageId?: Uint8Array;
  timestampUs?: bigint;
  tenantId?: string;
  threadId?: string;
  sequence?: bigint;
  type?: number;
  parentId?: Uint8Array;
  compression?: number;
  schemaVersion?: number;
}

function extractSchemaVersionInfo(header: Header | undefined): { schemaVersion?: number; schemaVersionMismatch?: boolean } {
  const sv = header?.schemaVersion;
  if (sv === undefined || sv === null) {
    return {};
  }
  const numSv = typeof sv === 'number' ? sv : Number(sv);
  if (!Number.isFinite(numSv) || numSv < 0) {
    return {};
  }
  const mismatch = numSv > EXPECTED_SCHEMA_VERSION;
  return { schemaVersion: numSv, schemaVersionMismatch: mismatch || undefined };
}

// M-1036: Result type for decode function with structured error info
interface DecodeResult {
  result: DecodedMessageSerializable | null;
  error?: string;
  errorType?: DecodeErrorTypeValue;
}

// Decode a binary message
// M-1036: Returns structured error info when decoding fails
function decode(buffer: ArrayBuffer): DecodeResult {
  if (!initialized || !DashStreamMessageType) {
    return {
      result: null,
      error: 'Decoder not initialized',
      errorType: DecodeErrorType.NOT_INITIALIZED,
    };
  }

  try {
    const bytes = new Uint8Array(buffer);
    let messageBuffer = bytes;
    let usedHeaderInterpretation = false;

    if (bytes.length > 1) {
      const headerByte = bytes[0];
      if (headerByte === 0x00) {
        // Uncompressed with header
        messageBuffer = bytes.subarray(1);
        usedHeaderInterpretation = true;
        if (DEBUG_DASHSTREAM) console.debug('[DecodeWorker] Detected uncompressed header (0x00)');
      } else if (headerByte === 0x01) {
        // Zstd compressed
        try {
          const compressedData = bytes.subarray(1);

          // M-1097: Check declared content size BEFORE decompression to prevent decompression bombs.
          // The previous code called decompress() first, then checked size - but the allocation
          // had already occurred, allowing OOM/freeze from malicious inputs.
          const frameHeader = parseZstdFrameHeader(compressedData);
          if (frameHeader.valid) {
            if (frameHeader.contentSize !== null) {
              // Content size is declared - check against limit before allocating
              if (frameHeader.contentSize > MAX_DECOMPRESSED_SIZE) {
                const errMsg = `Zstd frame declares content size ${frameHeader.contentSize} which exceeds limit ${MAX_DECOMPRESSED_SIZE}. Rejected without decompression (M-1097).`;
                console.error(`[DecodeWorker] ${errMsg}`);
                return {
                  result: null,
                  error: errMsg,
                  errorType: DecodeErrorType.DECOMPRESSED_SIZE_EXCEEDED,
                };
              }
            } else {
              // Content size is unknown (streaming frame) - reject to be safe.
              // DashStream protocol always uses frames with declared content size.
              const errMsg = 'Zstd frame has unknown content size (streaming frame). DashStream requires frames with declared size. Rejected (M-1097).';
              console.error(`[DecodeWorker] ${errMsg}`);
              return {
                result: null,
                error: errMsg,
                errorType: DecodeErrorType.DECOMPRESSED_SIZE_EXCEEDED,
              };
            }
          }
          // Note: If frameHeader.valid is false, we still try decompression as fallback
          // (may be legacy format or non-standard zstd). Post-decompression check remains as safety net.

          // Type cast needed: fzstd returns Uint8Array<ArrayBufferLike>, protobufjs expects Uint8Array<ArrayBuffer>
          messageBuffer = new Uint8Array(decompress(compressedData));
          // M-978: Post-decompression size check as safety net (e.g., if header parsing failed
          // and we fell through to decompression). This is defense-in-depth.
          if (messageBuffer.length > MAX_DECOMPRESSED_SIZE) {
            const errMsg = `Decompressed size ${messageBuffer.length} exceeds limit ${MAX_DECOMPRESSED_SIZE}`;
            console.error(`[DecodeWorker] ${errMsg}`);
            return {
              result: null,
              error: errMsg,
              errorType: DecodeErrorType.DECOMPRESSED_SIZE_EXCEEDED,
            };
          }
          usedHeaderInterpretation = true;
          if (DEBUG_DASHSTREAM) console.debug('[DecodeWorker] Decompressed zstd message, size:', messageBuffer.length);
        } catch (decompressError) {
          if (DEBUG_DASHSTREAM) {
            console.debug('[DecodeWorker] Zstd decompression failed, trying legacy format:', decompressError);
          }
          messageBuffer = bytes;
          usedHeaderInterpretation = false;
        }
      }
    }

    // Decode protobuf
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let decoded: any;
    try {
      decoded = DashStreamMessageType.decode(messageBuffer);
    } catch (decodeError) {
      if (usedHeaderInterpretation && bytes.length > 1) {
        if (DEBUG_DASHSTREAM) {
          console.debug('[DecodeWorker] Header-based decode failed, trying legacy format:', decodeError);
        }
        try {
          decoded = DashStreamMessageType.decode(bytes);
        } catch (legacyError) {
          const errMsg = `All decode attempts failed (headerError: ${decodeError}, legacyError: ${legacyError})`;
          console.error('[DecodeWorker] All decode attempts failed:', {
            headerError: decodeError,
            legacyError: legacyError,
            bufferLength: bytes.length,
            firstByte: bytes[0]?.toString(16),
          });
          return {
            result: null,
            error: errMsg,
            errorType: DecodeErrorType.DECODE_FAILED,
          };
        }
      } else {
        throw decodeError;
      }
    }
    const now = Date.now();

    // Determine message type and extract fields
    if (decoded.event) {
      const header = decoded.event.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'event',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.eventBatch) {
      const header = decoded.eventBatch.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const headerThreadId = header?.threadId;
      const headerSeq = safeNonNegativeSequenceString(header?.sequence);

      let maxInnerSeqStr: string | undefined;
      let maxInnerSeqBigInt: bigint | undefined;
      let innerThreadId: string | undefined;
      const maxInnerSeqByThread: Record<string, string> = {};
      const maxInnerSeqByThreadBigInt: Record<string, bigint> = {};

      for (const event of decoded.eventBatch.events || []) {
        const eventHeader = event.header;
        const seqStr = coerceU64ToStr(eventHeader?.sequence);
        const eventThreadId = eventHeader?.threadId;
        if (seqStr !== undefined) {
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
            // Skip
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
      return { result: {
        type: 'event_batch',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: effectiveThreadId,
        sequence,
        sequencesByThread,
        ...svInfo,
      } };
    }

    if (decoded.stateDiff) {
      const header = decoded.stateDiff.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'state_diff',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.tokenChunk) {
      const header = decoded.tokenChunk.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'token_chunk',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.toolExecution) {
      const header = decoded.toolExecution.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'tool_execution',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.metrics) {
      const header = decoded.metrics.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'metrics',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.error) {
      const header = decoded.error.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'error',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.checkpoint) {
      const header = decoded.checkpoint.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'checkpoint',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    if (decoded.executionTrace) {
      const header = decoded.executionTrace.header;
      const tsUs = safeToNumber(header?.timestampUs, 'timestampUs');
      const svInfo = extractSchemaVersionInfo(header);
      return { result: {
        type: 'execution_trace',
        message: decoded,
        timestamp: tsUs !== undefined ? tsUs / 1000 : now,
        threadId: header?.threadId || decoded.executionTrace.threadId,
        sequence: safeNonNegativeSequenceString(header?.sequence),
        ...svInfo,
      } };
    }

    return { result: {
      type: 'unknown',
      message: decoded,
      timestamp: now,
    } };
  } catch (error) {
    const errMsg = error instanceof Error ? error.message : String(error);
    console.error('[DecodeWorker] Failed to decode message:', error);
    return {
      result: null,
      error: errMsg,
      errorType: DecodeErrorType.UNKNOWN_ERROR,
    };
  }
}

// Handle messages from main thread
self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;

  if (request.type === 'init') {
    const success = initDecoder();
    const response: InitResponse = {
      type: 'init_result',
      id: request.id,
      success,
      error: success ? undefined : 'Failed to initialize decoder',
    };
    self.postMessage(response);
  } else if (request.type === 'decode') {
    // M-1036: decode() now returns structured error info alongside result
    const decodeResult = decode(request.buffer);
    const response: DecodeResponse = {
      type: 'decode_result',
      id: request.id,
      result: decodeResult.result,
      error: decodeResult.error,
      errorType: decodeResult.errorType,
    };
    self.postMessage(response);
  }
};

// Export types for the main thread
export type { WorkerRequest, WorkerResponse, DecodeRequest, InitRequest, DecodeResponse, InitResponse, DecodedMessageSerializable };
