// M-741: Track whether state contains numbers that may have lost precision.
// JavaScript numbers lose precision above 2^53-1 (MAX_SAFE_INTEGER).
// When JSON.parse() receives large integers, they get silently rounded,
// causing hash verification to fail (false corruption detection).

// M-775: Context object passed through recursion to track unsafe numbers.
// This replaces the global variable that caused race conditions in concurrent
// hash computations. Each computeStateHash call gets its own context.
interface HashContext {
  unsafeNumberDetected: boolean;
}

// Internal implementation that accepts context for thread-safe recursion.
function canonicalJsonStringInternal(value: unknown, ctx: HashContext): string {
  if (value === null) return 'null';

  switch (typeof value) {
    case 'boolean':
      return value ? 'true' : 'false';
    case 'number':
      if (!Number.isFinite(value)) return 'null';
      // M-741: Detect numbers that may have lost precision from JSON.parse.
      // If abs(value) > MAX_SAFE_INTEGER, the number likely lost precision
      // when the server sent it as JSON and JavaScript parsed it.
      if (Math.abs(value) > Number.MAX_SAFE_INTEGER) {
        ctx.unsafeNumberDetected = true;
      }
      return JSON.stringify(value);
    case 'string':
      return JSON.stringify(value);
    case 'undefined':
      return 'null';
    case 'object': {
      if (Array.isArray(value)) {
        return `[${value.map(v => canonicalJsonStringInternal(v, ctx)).join(',')}]`;
      }

      const obj = value as Record<string, unknown>;
      const keys = Object.keys(obj).sort();
      const parts: string[] = [];
      for (const key of keys) {
        const child = obj[key];
        if (child === undefined || typeof child === 'function' || typeof child === 'symbol') {
          continue;
        }
        parts.push(`${JSON.stringify(key)}:${canonicalJsonStringInternal(child, ctx)}`);
      }
      return `{${parts.join(',')}}`;
    }
    case 'bigint':
      // M-750: Serialize BigInt to its string representation for correct hashing.
      // BigInt values in graph state (e.g., large counters) must hash correctly.
      return `"${(value as bigint).toString()}"`;
    default:
      // function, symbol
      return 'null';
  }
}

// Public API: Creates a fresh context for each call (thread-safe).
// M-775: No longer uses global state - each call is isolated.
export function canonicalJsonString(value: unknown): string {
  const ctx: HashContext = { unsafeNumberDetected: false };
  return canonicalJsonStringInternal(value, ctx);
}

// M-741: Result type for hash computation that indicates reliability.
export interface StateHashResult {
  hash: Uint8Array;
  // True if the hash may be unreliable due to large numbers that lost precision in JSON.parse.
  // When true, callers should skip hash verification to avoid false corruption detection.
  hasUnsafeNumbers: boolean;
}

export async function computeStateHash(state: Record<string, unknown>): Promise<StateHashResult> {
  // M-775: Each call gets its own context - no race conditions with concurrent calls.
  const ctx: HashContext = { unsafeNumberDetected: false };

  const canonical = canonicalJsonStringInternal(state, ctx);
  const data = new TextEncoder().encode(canonical);

  // M-775: hasUnsafeNumbers is now isolated per call - no risk of overwrite by concurrent hash computations.
  const hasUnsafeNumbers = ctx.unsafeNumberDetected;

  const subtle = globalThis.crypto?.subtle;
  if (!subtle) {
    throw new Error('WebCrypto SubtleCrypto not available');
  }

  const digest = await subtle.digest('SHA-256', data);
  return {
    hash: new Uint8Array(digest),
    hasUnsafeNumbers,
  };
}

// M-741: Legacy function for backwards compatibility (returns just the hash).
// Prefer computeStateHash() in new code to handle unsafe numbers properly.
export async function computeStateHashLegacy(state: Record<string, unknown>): Promise<Uint8Array> {
  const result = await computeStateHash(state);
  if (result.hasUnsafeNumbers) {
    console.warn(
      '[stateHash] State contains numbers > MAX_SAFE_INTEGER that may have lost precision. ' +
      'Hash verification may produce false corruption flags. Consider using string encoding ' +
      'for large integers in graph state payloads.'
    );
  }
  return result.hash;
}
