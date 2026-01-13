// JSON Patch (RFC 6902) operations for time-travel state reconstruction
// Works with DashStream StateDiff.operations which use JSON Pointer paths (RFC 6901)
//
// # Value Encoding Policy (M-94)
//
// Browser clients can only decode JSON and RAW encoded values. MSGPACK and PROTOBUF
// encodings require additional libraries and/or schema definitions not available
// in the browser environment.
//
// When unsupported encodings are received, this module throws UnsupportedEncodingError
// with a clear diagnostic message. Configure your DashStream producer to use JSON
// encoding for browser-facing telemetry.
//
// The backend also enforces this policy via the dashstream_unsupported_encoding_total
// metric and warning logs.
//
// See crates/dashflow-streaming/src/diff/protobuf.rs for backend handling.

import { z } from 'zod';
import { DiffOperation } from '../proto/dashstream';

// M-808: Custom error for clone failures to distinguish from other errors
export class CloneError extends Error {
  constructor(
    message: string,
    public readonly structuredCloneError?: unknown,
    public readonly jsonRoundTripError?: unknown,
  ) {
    super(message);
    this.name = 'CloneError';
  }
}

// M-680: Safe deep clone that falls back to JSON round-trip if structuredClone fails
// (e.g., on functions, DOM nodes, symbols). Prevents crashes on unexpected state values.
// M-808: Throws CloneError instead of returning original value to prevent mutation aliasing.
// Callers should catch CloneError and handle appropriately (e.g., skip operation, mark corrupted).
function safeClone<T>(value: T): T {
  let structuredCloneError: unknown;
  try {
    return structuredClone(value);
  } catch (e1) {
    structuredCloneError = e1;
    console.debug('[jsonPatch] structuredClone failed, falling back to JSON round-trip');
    try {
      return JSON.parse(JSON.stringify(value)) as T;
    } catch (e2) {
      // M-808: Throw instead of returning original to prevent silent mutation aliasing.
      // Returning original means mutations to the "clone" affect the original,
      // which can cause subtle bugs in state management (e.g., corrupted checkpoints).
      throw new CloneError(
        'Deep clone failed: structuredClone and JSON round-trip both failed. ' +
        'State may contain BigInt, circular references, or non-serializable values.',
        structuredCloneError,
        e2,
      );
    }
  }
}

// M-1113: Create bounded preview of JSON value for error messages.
// Prevents huge strings in error messages when test operation fails on large values.
const MAX_ERROR_PREVIEW_LENGTH = 200;
function boundedJsonPreview(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';

  const type = typeof value;
  if (type === 'string') {
    const str = value as string;
    if (str.length <= MAX_ERROR_PREVIEW_LENGTH) {
      return JSON.stringify(str);
    }
    return `"${str.slice(0, MAX_ERROR_PREVIEW_LENGTH)}..." (${str.length} chars)`;
  }
  if (type === 'number' || type === 'boolean') {
    return String(value);
  }
  if (type === 'object') {
    if (Array.isArray(value)) {
      return `Array(${value.length})`;
    }
    const keys = Object.keys(value as object);
    const preview = keys.slice(0, 3).join(', ');
    const more = keys.length > 3 ? `, ... +${keys.length - 3}` : '';
    return `Object{${preview}${more}}`;
  }
  return `[${type}]`;
}

// M-733: Deep equality check for RFC 6902 test operation
// JSON.stringify comparison fails when objects have keys in different order
// (e.g., {a:1,b:2} vs {b:2,a:1} are semantically equal but stringify differently)
function jsonDeepEqual(a: unknown, b: unknown): boolean {
  // Handle null/undefined
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (a === undefined || b === undefined) return false;

  // Handle primitive types
  const typeA = typeof a;
  const typeB = typeof b;
  if (typeA !== typeB) return false;
  if (typeA !== 'object') return a === b;

  // Handle arrays (order matters per RFC 6902)
  const isArrayA = Array.isArray(a);
  const isArrayB = Array.isArray(b);
  if (isArrayA !== isArrayB) return false;
  if (isArrayA && isArrayB) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!jsonDeepEqual(a[i], b[i])) return false;
    }
    return true;
  }

  // Handle objects (key order does not matter)
  const objA = a as Record<string, unknown>;
  const objB = b as Record<string, unknown>;
  const keysA = Object.keys(objA);
  const keysB = Object.keys(objB);
  if (keysA.length !== keysB.length) return false;
  for (const key of keysA) {
    if (!Object.prototype.hasOwnProperty.call(objB, key)) return false;
    if (!jsonDeepEqual(objA[key], objB[key])) return false;
  }
  return true;
}

// M-473: Zod schema for JSON.parse results to avoid implicit `any` typing
// We use z.unknown() since patch values can be any valid JSON structure
const JsonValueSchema = z.unknown();

// Operation types matching proto enum
export enum OpType {
  ADD = 0,
  REMOVE = 1,
  REPLACE = 2,
  MOVE = 3,
  COPY = 4,
  TEST = 5,
}

// Value encoding types matching proto enum
export enum ValueEncoding {
  JSON = 0,
  MSGPACK = 1,
  PROTOBUF = 2,
  RAW = 3,
}

// Parsed JSON Patch operation
export interface JsonPatchOp {
  op: 'add' | 'remove' | 'replace' | 'move' | 'copy' | 'test';
  path: string;
  value?: unknown;
  from?: string;
}

// M-710: Dangerous path segments that could cause prototype pollution
const DANGEROUS_SEGMENTS = new Set(['__proto__', 'constructor', 'prototype']);

// Parse JSON Pointer path into segments
// e.g., "/messages/0/content" -> ["messages", "0", "content"]
// Per RFC 6901: "/" points to the empty string key "", "" points to the document root
// M-710: Throws on prototype pollution attempts (__proto__, constructor, prototype)
function parseJsonPointer(path: string): string[] {
  if (path === '') return []; // Empty string = document root
  if (!path.startsWith('/')) {
    throw new Error(`Invalid JSON Pointer: must start with '/' - got '${path}'`);
  }
  // "/" -> [""], "/foo" -> ["foo"], "/foo/bar" -> ["foo", "bar"]
  const segments = path.slice(1).split('/').map(segment =>
    // Unescape JSON Pointer special characters (~1 = /, ~0 = ~)
    segment.replace(/~1/g, '/').replace(/~0/g, '~')
  );

  // M-710: Block dangerous path segments to prevent prototype pollution
  for (const segment of segments) {
    if (DANGEROUS_SEGMENTS.has(segment)) {
      throw new Error(
        `[jsonPatch] Refused to apply patch: path segment '${segment}' is blocked ` +
        `to prevent prototype pollution. This may indicate a malformed or malicious patch.`
      );
    }
  }

  return segments;
}

// Get parent path and final key from segments
function splitPath(segments: string[]): { parent: string[]; key: string } {
  if (segments.length === 0) {
    return { parent: [], key: '' };
  }
  return {
    parent: segments.slice(0, -1),
    key: segments[segments.length - 1],
  };
}

// M-709: Validate and parse array index from path segment
// Returns the numeric index, or throws if invalid
// For 'add' operations: '-' is allowed (means append)
// For other operations: only digits are allowed
function parseArrayIndex(segment: string, isAdd: boolean): number {
  // Special case: '-' means "after the last element" but only for add
  if (segment === '-') {
    if (isAdd) {
      return -1; // Signal to use array.length at insertion time
    }
    throw new Error(
      `[jsonPatch] Invalid array index '-': only valid for 'add' operations. ` +
      `For remove/replace, use a numeric index.`
    );
  }

  // Validate: must be digits only (no leading zeros except "0" itself per RFC)
  // Leading zeros like "01" are technically invalid JSON Pointer array indices
  if (!/^(0|[1-9]\d*)$/.test(segment)) {
    throw new Error(
      `[jsonPatch] Invalid array index '${segment}': must be a non-negative integer. ` +
      `This may indicate a malformed patch that would corrupt state.`
    );
  }

  const index = parseInt(segment, 10);

  // Double-check: parseInt should never return NaN for valid digit strings,
  // but guard against unexpected edge cases
  if (!Number.isFinite(index) || index < 0) {
    throw new Error(
      `[jsonPatch] Array index '${segment}' parsed to invalid value ${index}. ` +
      `This should not happen - please report this bug.`
    );
  }

  return index;
}

// Navigate to a location in the state object
// M-709: Uses validated array index parsing
function getLocation(state: unknown, segments: string[]): unknown {
  let current = state;
  for (const segment of segments) {
    if (current === null || current === undefined) {
      return undefined;
    }
    if (Array.isArray(current)) {
      // For navigation, '-' means past-the-end (returns undefined)
      // Use parseArrayIndex with isAdd=false for strict validation
      try {
        const index = segment === '-' ? current.length : parseArrayIndex(segment, false);
        current = current[index];
      } catch {
        // Invalid index in path navigation - return undefined
        return undefined;
      }
    } else if (typeof current === 'object') {
      current = (current as Record<string, unknown>)[segment];
    } else {
      return undefined;
    }
  }
  return current;
}

// M-708: Operation types for correct array semantics
type SetLocationOp = 'add' | 'replace';

// Set a value at a location, creating intermediate objects/arrays as needed
// M-708: RFC6902-correct array semantics:
//   - 'add' on arrays: INSERT at index (splice), '-' appends
//   - 'replace' on arrays: REPLACE at index (assignment)
// M-709: Validates array indices and throws on invalid paths
function setLocation(state: unknown, segments: string[], value: unknown, op: SetLocationOp = 'replace'): unknown {
  if (segments.length === 0) {
    return value;
  }

  const isAdd = op === 'add';

  // Ensure state is an object or array
  let root = state;
  if (root === null || root === undefined || typeof root !== 'object') {
    // Determine if we should create an array or object based on first segment
    root = /^\d+$/.test(segments[0]) || segments[0] === '-' ? [] : {};
  }

  const { parent, key } = splitPath(segments);

  // Navigate to parent, creating intermediate containers
  // M-709: Use strict index validation for intermediate navigation
  let current: unknown = root;
  for (let i = 0; i < parent.length; i++) {
    const segment = parent[i];
    const nextSegment = i < parent.length - 1 ? parent[i + 1] : key;
    const nextIsArray = /^\d+$/.test(nextSegment) || nextSegment === '-';

    if (Array.isArray(current)) {
      // M-709: Validate array index (isAdd=false for intermediate navigation)
      const index = parseArrayIndex(segment, false);
      if (current[index] === undefined || current[index] === null) {
        current[index] = nextIsArray ? [] : {};
      }
      current = current[index];
    } else if (typeof current === 'object' && current !== null) {
      const obj = current as Record<string, unknown>;
      if (obj[segment] === undefined || obj[segment] === null) {
        obj[segment] = nextIsArray ? [] : {};
      }
      current = obj[segment];
    }
  }

  // Set the final value
  // M-708: RFC6902 array semantics differ for add vs replace
  if (Array.isArray(current)) {
    // M-709: Validate the final array index
    const index = parseArrayIndex(key, isAdd);

    if (isAdd) {
      // M-708: RFC6902 'add' on arrays: INSERT at index (splice)
      // '-' (returned as -1) means append at end
      if (index === -1) {
        current.push(value);
      } else {
        // Validate index is within bounds for insertion (0 to length inclusive)
        if (index > current.length) {
          throw new Error(
            `[jsonPatch] Array 'add' index ${index} out of bounds for array of length ${current.length}. ` +
            `Index must be <= array length for insertion.`
          );
        }
        current.splice(index, 0, value);
      }
    } else {
      // 'replace' on arrays: direct assignment at index
      // Validate index exists for replacement
      if (index >= current.length) {
        throw new Error(
          `[jsonPatch] Array 'replace' index ${index} out of bounds for array of length ${current.length}. ` +
          `Index must be < array length for replacement.`
        );
      }
      current[index] = value;
    }
  } else if (typeof current === 'object' && current !== null) {
    (current as Record<string, unknown>)[key] = value;
  }

  return root;
}

// Remove a value at a location
// M-709: Validates array indices and throws on invalid paths
function removeLocation(state: unknown, segments: string[]): unknown {
  if (segments.length === 0) {
    return undefined;
  }

  const { parent, key } = splitPath(segments);
  const parentObj = getLocation(state, parent);

  if (parentObj === undefined || parentObj === null) {
    return state;
  }

  if (Array.isArray(parentObj)) {
    // M-709: Validate array index (isAdd=false for remove)
    const index = parseArrayIndex(key, false);
    // Validate index is within bounds for removal
    if (index >= parentObj.length) {
      throw new Error(
        `[jsonPatch] Array 'remove' index ${index} out of bounds for array of length ${parentObj.length}. ` +
        `Index must be < array length for removal.`
      );
    }
    parentObj.splice(index, 1);
  } else if (typeof parentObj === 'object') {
    delete (parentObj as Record<string, unknown>)[key];
  }

  return state;
}

// Error for unsupported encodings
export class UnsupportedEncodingError extends Error {
  constructor(encoding: ValueEncoding) {
    const encodingName = ValueEncoding[encoding] || `unknown(${encoding})`;
    super(
      `[jsonPatch] Unsupported value encoding: ${encodingName}. ` +
      `The observability UI only supports JSON and RAW encodings. ` +
      `Configure your DashStream producer to use JSON encoding for browser clients.`
    );
    this.name = 'UnsupportedEncodingError';
  }
}

// M-473: Type-safe JSON parsing with Zod validation
// Returns unknown type instead of any, preventing implicit any propagation
function parseJsonSafe(jsonStr: string): unknown {
  // Parse JSON and validate with Zod schema
  // This ensures we get `unknown` type instead of `any` from JSON.parse
  const parsed = JSON.parse(jsonStr);
  const result = JsonValueSchema.safeParse(parsed);
  if (result.success) {
    return result.data;
  }
  // safeParse should always succeed for z.unknown(), but handle edge cases
  throw new Error(`JSON validation failed: ${result.error.message}`);
}

// Decode value bytes based on encoding
// Hard-fail on MSGPACK/PROTOBUF encodings (not supported in browser)
// Producers must use JSON encoding for browser-facing telemetry
function decodeValue(valueBytes: Uint8Array, encoding: number): unknown {
  if (!valueBytes || valueBytes.length === 0) {
    return undefined;
  }

  // M-772: Use fatal: true to throw on invalid UTF-8 instead of silently replacing
  // with U+FFFD replacement characters. Invalid UTF-8 indicates data corruption
  // or encoding mismatch, which should trigger patch-failure handling.
  const textDecoder = new TextDecoder('utf-8', { fatal: true });

  switch (encoding) {
    case ValueEncoding.JSON: {
      // M-740: Do NOT fall back to string on parse failure - that silently corrupts state.
      // Let the error propagate so patch-failure handling marks the run as corrupted/needsResync.
      const jsonStr = textDecoder.decode(valueBytes);
      try {
        // M-473: Use type-safe JSON parsing
        return parseJsonSafe(jsonStr);
      } catch (e) {
        // M-756: Only wrap SyntaxError (JSON.parse failure) with context.
        // Let other errors (Zod validation, unexpected errors) propagate unchanged
        // to preserve their original stack trace and type for debugging.
        if (e instanceof SyntaxError) {
          // SyntaxError from JSON.parse - add context about the raw bytes
          throw new Error(
            `[jsonPatch] Failed to parse JSON value: ${e.message}. ` +
            `Raw bytes decoded to: "${jsonStr.slice(0, 100)}${jsonStr.length > 100 ? '...' : ''}". ` +
            `This indicates a producer encoding issue or data corruption.`
          );
        }
        // Other errors (Zod validation, etc.) - propagate unchanged
        throw e;
      }
    }

    case ValueEncoding.RAW:
      return textDecoder.decode(valueBytes);

    case ValueEncoding.MSGPACK:
    case ValueEncoding.PROTOBUF:
      // Hard-fail on unsupported encodings
      // These require additional libraries and/or schema definitions.
      // Producers should use JSON encoding for browser clients.
      throw new UnsupportedEncodingError(encoding as ValueEncoding);

    default:
      // M-452: Unknown encoding is an error, not a silent fallback
      // Silent fallback masks producer misconfiguration and data integrity issues.
      // If you need a new encoding, add explicit support for it above.
      throw new UnsupportedEncodingError(encoding as ValueEncoding);
  }
}

// Convert protobuf DiffOperation to JsonPatchOp
export function convertDiffOp(diffOp: DiffOperation): JsonPatchOp {
  const opMap: Record<number, JsonPatchOp['op']> = {
    [OpType.ADD]: 'add',
    [OpType.REMOVE]: 'remove',
    [OpType.REPLACE]: 'replace',
    [OpType.MOVE]: 'move',
    [OpType.COPY]: 'copy',
    [OpType.TEST]: 'test',
  };

  return {
    op: opMap[diffOp.op] || 'replace',
    path: diffOp.path,
    value: decodeValue(diffOp.value, diffOp.encoding),
    from: diffOp.from || undefined,
  };
}

// M-1101: Configurable limits to prevent DoS via huge patch arrays or long paths
export interface PatchLimits {
  maxOperations?: number; // Default: 10000 - prevents freeze from huge patch arrays
  maxPathLength?: number; // Default: 1000 - prevents DoS from very long paths
}

const DEFAULT_PATCH_LIMITS: Required<PatchLimits> = {
  maxOperations: 10000,
  maxPathLength: 1000,
};

// M-1101: Validate path length to prevent DoS
function validatePathLength(path: string, maxLength: number): void {
  if (path.length > maxLength) {
    throw new Error(
      `[jsonPatch] Path too long: ${path.length} chars > max ${maxLength}. ` +
      `This may indicate a malformed or malicious patch.`
    );
  }
}

// Apply a single JSON Patch operation to state (MUTABLY - caller must clone first)
// M-708: Uses RFC6902-correct array semantics for add vs replace
// M-709: Validates array indices and throws on invalid paths
// M-710: Throws on prototype pollution attempts
// M-1101: Operates mutably to avoid O(N²) cloning. Caller handles the single clone.
function applyPatchOpMutable(state: unknown, op: JsonPatchOp, limits: Required<PatchLimits>): unknown {
  // M-1101: Validate path length
  validatePathLength(op.path, limits.maxPathLength);
  if (op.from) {
    validatePathLength(op.from, limits.maxPathLength);
  }

  const segments = parseJsonPointer(op.path);

  switch (op.op) {
    case 'add':
      // M-708: RFC6902 'add' on arrays uses INSERT semantics
      return setLocation(state, segments, op.value, 'add');

    case 'replace':
      // M-708: RFC6902 'replace' on arrays uses REPLACE semantics
      return setLocation(state, segments, op.value, 'replace');

    case 'remove':
      return removeLocation(state, segments);

    case 'move': {
      if (!op.from) {
        throw new Error('move operation requires "from" path');
      }
      const fromSegments = parseJsonPointer(op.from);
      const value = getLocation(state, fromSegments);
      removeLocation(state, fromSegments);
      // M-708: 'move' is semantically a remove + add, so target uses 'add' semantics
      return setLocation(state, segments, value, 'add');
    }

    case 'copy': {
      if (!op.from) {
        throw new Error('copy operation requires "from" path');
      }
      const fromSegments = parseJsonPointer(op.from);
      // M-460: Use structuredClone() for better performance
      // M-680: Use safeClone() to handle non-serializable values
      // Note: copy still needs to clone the value, but not the entire state
      const value = safeClone(getLocation(state, fromSegments));
      // M-708: 'copy' targets a new location, so uses 'add' semantics
      return setLocation(state, segments, value, 'add');
    }

    case 'test': {
      const actual = getLocation(state, segments);
      const expected = op.value;
      // M-733: Use deep equality instead of JSON.stringify to handle key ordering
      if (!jsonDeepEqual(actual, expected)) {
        // M-1113: Use bounded previews to prevent huge error strings
        throw new Error(`test failed: expected ${boundedJsonPreview(expected)} at ${op.path}, got ${boundedJsonPreview(actual)}`);
      }
      return state;
    }

    default:
      console.warn(`[jsonPatch] Unknown operation: ${op.op}`);
      return state;
  }
}

// Apply a single JSON Patch operation to state (immutably - clones for each op)
// DEPRECATED: Use applyPatch() instead which clones once for O(N) performance.
// This function is kept for backward compatibility but has O(N²) performance.
// M-708: Uses RFC6902-correct array semantics for add vs replace
// M-709: Validates array indices and throws on invalid paths
// M-710: Throws on prototype pollution attempts
export function applyPatchOp(state: unknown, op: JsonPatchOp): unknown {
  // M-460: Use structuredClone() for better performance
  // M-680: Use safeClone() to handle non-serializable values
  const newState = safeClone(state ?? {});
  return applyPatchOpMutable(newState, op, DEFAULT_PATCH_LIMITS);
}

// Apply multiple JSON Patch operations to state
// M-1101: Clone state ONCE at the start, then apply all operations mutably.
// Previous O(N²) version cloned for each operation, causing browser freeze on large patches.
export function applyPatch(
  state: unknown,
  operations: JsonPatchOp[],
  limits: PatchLimits = {}
): unknown {
  const mergedLimits = { ...DEFAULT_PATCH_LIMITS, ...limits };

  // M-1101: Cap operation count to prevent freeze from huge patch arrays
  if (operations.length > mergedLimits.maxOperations) {
    throw new Error(
      `[jsonPatch] Too many operations: ${operations.length} > max ${mergedLimits.maxOperations}. ` +
      `This may indicate a malformed or malicious StateDiff.`
    );
  }

  // M-1101: Clone state ONCE at the start (O(N) instead of O(N²))
  // M-680: Use safeClone() to handle non-serializable values
  let result: unknown = safeClone(state ?? {});

  // Apply operations mutably to the clone
  for (const op of operations) {
    result = applyPatchOpMutable(result, op, mergedLimits);
  }

  return result;
}

// Apply DiffOperations from protobuf directly
export function applyDiffOperations(state: unknown, diffOps: DiffOperation[]): unknown {
  const jsonPatchOps = diffOps.map(convertDiffOp);
  return applyPatch(state, jsonPatchOps);
}

// Compute changed paths from operations
export function getChangedPaths(operations: JsonPatchOp[]): string[] {
  return operations.map(op => op.path);
}
