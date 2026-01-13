export function getStringAttribute(
  attributes: Record<string, unknown>,
  key: string
): string | undefined {
  const value = attributes[key];
  if (value === undefined || value === null) return undefined;

  // Handle AttributeValue wrapper (from protobufjs)
  if (typeof value === 'object' && value !== null && 'stringValue' in value) {
    return (value as { stringValue: string }).stringValue;
  }

  if (typeof value === 'string') return value;
  return undefined;
}

export interface JsonAttributeOptions {
  maxBytes?: number;
}

const DEFAULT_MAX_JSON_ATTRIBUTE_BYTES = 2 * 1024 * 1024; // 2MB cap (DoS prevention)

// M-1098: Compute UTF-8 byte length without allocating the encoded buffer.
// Short-circuits when maxBytes is exceeded to avoid scanning huge strings.
// This is allocation-safe: counting 100MB of text costs O(n) iterations, not O(n) memory.
export function utf8ByteLengthCapped(str: string, maxBytes: number): number {
  let bytes = 0;

  for (let i = 0; i < str.length; i++) {
    const codeUnit = str.charCodeAt(i);

    if (codeUnit <= 0x7f) {
      bytes += 1;
    } else if (codeUnit <= 0x7ff) {
      bytes += 2;
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = i + 1 < str.length ? str.charCodeAt(i + 1) : 0;
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        i++;
      } else {
        bytes += 3;
      }
    } else {
      bytes += 3;
    }

    if (bytes > maxBytes) return bytes;
  }

  return bytes;
}

export function getJsonAttribute(
  attributes: Record<string, unknown>,
  key: string,
  options: JsonAttributeOptions = {}
): unknown {
  const value = attributes[key];
  if (value === undefined || value === null) return undefined;

  const jsonStr = getStringAttribute(attributes, key);
  if (jsonStr) {
    const maxBytes = options.maxBytes ?? DEFAULT_MAX_JSON_ATTRIBUTE_BYTES;
    const sizeBytes = utf8ByteLengthCapped(jsonStr, maxBytes);
    if (sizeBytes > maxBytes) {
      console.warn(
        `[attributes] Skipping JSON attribute '${key}' (${sizeBytes} bytes > max ${maxBytes} bytes)`
      );
      return undefined;
    }

    try {
      return JSON.parse(jsonStr);
    } catch (e) {
      console.warn(`[attributes] Failed to parse JSON attribute '${key}':`, e);
      return undefined;
    }
  }

  if (typeof value === 'object') {
    return value;
  }

  return undefined;
}

// M-1102: Strict numeric string parsing (rejects "123abc", accepts "123", "1.5", "-2")
// parseFloat("123abc") returns 123 which is wrong for telemetry.
function strictParseNumber(str: string): number | undefined {
  const trimmed = str.trim();
  // Empty string or whitespace-only
  if (trimmed === '') return undefined;
  // Validate numeric format: optional sign, digits, optional decimal, optional exponent
  // This regex rejects "123abc", "NaN", "Infinity" etc.
  if (!/^-?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(trimmed)) {
    return undefined;
  }
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

// M-1060: Extract numeric attribute value (handles AttributeValue wrapper)
// M-1102: Now handles floatValue wrapper and uses strict parsing (rejects junk like "123abc")
export function getNumberAttribute(
  attributes: Record<string, unknown>,
  key: string
): number | undefined {
  const value = attributes[key];
  if (value === undefined || value === null) return undefined;

  // Handle AttributeValue wrapper (from protobufjs)
  if (typeof value === 'object' && value !== null) {
    // M-1102: Check floatValue first (more specific for floating point)
    if ('floatValue' in value) {
      const floatVal = (value as { floatValue: unknown }).floatValue;
      if (typeof floatVal === 'number') return Number.isFinite(floatVal) ? floatVal : undefined;
      if (typeof floatVal === 'string') return strictParseNumber(floatVal);
    }
    // Then check intValue
    if ('intValue' in value) {
      const intVal = (value as { intValue: unknown }).intValue;
      if (typeof intVal === 'number') return Number.isFinite(intVal) ? intVal : undefined;
      if (typeof intVal === 'string') return strictParseNumber(intVal);
    }
    // Protobuf may also use doubleValue for 64-bit floats
    if ('doubleValue' in value) {
      const doubleVal = (value as { doubleValue: unknown }).doubleValue;
      if (typeof doubleVal === 'number') return Number.isFinite(doubleVal) ? doubleVal : undefined;
      if (typeof doubleVal === 'string') return strictParseNumber(doubleVal);
    }
  }

  if (typeof value === 'number') return Number.isFinite(value) ? value : undefined;
  if (typeof value === 'string') return strictParseNumber(value);
  return undefined;
}

// M-1067: Bound attribute storage to prevent memory exhaustion
// Truncates large string values and limits total attribute size
const MAX_ATTRIBUTE_STRING_LENGTH = 1000;
const MAX_TOTAL_ATTRIBUTES_BYTES = 10_000; // 10KB cap per event

// M-1088: Estimate object size WITHOUT allocating the full JSON string.
// Recursively counts approximate JSON size, stopping early when cap is exceeded.
// This is allocation-safe: even for huge nested objects, we only traverse pointers.
function estimateJsonSizeCapped(
  value: unknown,
  maxSize: number,
  stack: Set<object> = new Set()
): number {
  if (value === null) return 4; // "null"
  if (value === undefined) return 0; // undefined is skipped in JSON

  const type = typeof value;

  if (type === 'string') {
    // Account for quotes + escaping overhead (rough estimate)
    return (value as string).length + 2;
  }

  if (type === 'number') {
    // Finite numbers: up to ~20 chars; NaN/Infinity serialize as "null"
    return Number.isFinite(value as number) ? 20 : 4;
  }

  if (type === 'boolean') {
    return (value as boolean) ? 4 : 5; // "true" or "false"
  }

  if (type === 'object') {
    const obj = value as object;

    // Circular reference check to prevent infinite loops.
    // This uses a recursion stack (not a global visited set) so repeated references
    // in a DAG don't get treated as circular.
    if (stack.has(obj)) return maxSize + 1; // force "oversize" to ensure safe truncation
    stack.add(obj);

    try {
      let size = 0;

      if (Array.isArray(obj)) {
        size += 2; // []
        for (const item of obj) {
          size += estimateJsonSizeCapped(item, maxSize - size, stack) + 1; // +1 for comma
          if (size > maxSize) return size; // Early exit
        }
      } else {
        size += 2; // {}
        for (const [key, val] of Object.entries(obj)) {
          size += key.length + 3; // "key":
          size += estimateJsonSizeCapped(val, maxSize - size, stack) + 1; // +1 for comma
          if (size > maxSize) return size; // Early exit
        }
      }

      return size;
    } finally {
      stack.delete(obj);
    }
  }

  // Function, symbol, bigint, etc. - not JSON-serializable
  return 0;
}

export function boundAttributes(
  attributes: Record<string, unknown>
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  let totalSize = 0;

  for (const [key, value] of Object.entries(attributes)) {
    // Skip null/undefined
    if (value === undefined || value === null) continue;

    // Skip if we've hit the size cap
    if (totalSize >= MAX_TOTAL_ATTRIBUTES_BYTES) {
      console.debug(`[boundAttributes] Dropping attribute '${key}' - size cap reached`);
      continue;
    }

    let boundedValue: unknown = value;

    // Truncate long strings
    if (typeof value === 'string') {
      if (value.length > MAX_ATTRIBUTE_STRING_LENGTH) {
        boundedValue = value.slice(0, MAX_ATTRIBUTE_STRING_LENGTH) + '... [truncated]';
        console.debug(`[boundAttributes] Truncated string attribute '${key}' from ${value.length} chars`);
      }
      totalSize += (boundedValue as string).length;
    } else if (typeof value === 'object') {
      // M-1088 FIX: Estimate size WITHOUT JSON.stringify (allocation-safe)
      const estimatedSize = estimateJsonSizeCapped(value, MAX_ATTRIBUTE_STRING_LENGTH + 100);

      if (estimatedSize > MAX_ATTRIBUTE_STRING_LENGTH) {
        // Object is too large - create a truncated marker WITHOUT stringifying
        // Only stringify a shallow preview of the first few keys
        const preview: Record<string, unknown> = {};
        let previewKeys = 0;
        for (const [k, v] of Object.entries(value as object)) {
          if (previewKeys >= 3) break;
          if (typeof v === 'string') {
            preview[k] = v.slice(0, 50);
          } else if (typeof v === 'number' || typeof v === 'boolean' || v === null) {
            preview[k] = v;
          } else {
            preview[k] = '[object]';
          }
          previewKeys++;
        }

        boundedValue = {
          _truncated: true,
          _estimatedSize: estimatedSize,
          _preview: preview,
        };
        console.debug(`[boundAttributes] Truncated object attribute '${key}' (~${estimatedSize} bytes)`);
      }
      totalSize += Math.min(estimatedSize, MAX_ATTRIBUTE_STRING_LENGTH);
    } else {
      // Primitives (number, boolean) - small fixed size
      totalSize += 16;
    }

    result[key] = boundedValue;
  }

  return result;
}
