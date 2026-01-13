// Tests for dashstream.ts exported functions and constants
// Tests the protocol buffer helpers and type utilities

import {
  EXPECTED_SCHEMA_VERSION,
  MAX_DECOMPRESSED_SIZE,
  parseZstdFrameHeader,
  safeToNumber,
  safeNonNegativeSequenceString,
  coerceU64ToStr,
  EventType,
  MessageType,
  getEventTypeName,
  isNodeLifecycleEvent,
  isGraphLifecycleEvent,
} from '../proto/dashstream';

let passed = 0;
let failed = 0;

function test(name: string, fn: () => void): void {
  try {
    fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (e) {
    console.log(`  ✗ ${name}`);
    console.log(`    Error: ${e}`);
    failed++;
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(message || `Expected ${expectedStr}, got ${actualStr}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || 'Expected true but got false');
  }
}

console.log('\nDashStream Protocol Tests\n');

// ============================================================================
// Constants Tests
// ============================================================================

console.log('Constants:');

test('EXPECTED_SCHEMA_VERSION is defined and valid', () => {
  assertTrue(typeof EXPECTED_SCHEMA_VERSION === 'number', 'Should be a number');
  assertTrue(EXPECTED_SCHEMA_VERSION >= 1, 'Should be at least 1');
  assertTrue(Number.isInteger(EXPECTED_SCHEMA_VERSION), 'Should be an integer');
});

test('MAX_DECOMPRESSED_SIZE is defined and reasonable', () => {
  assertTrue(typeof MAX_DECOMPRESSED_SIZE === 'number', 'Should be a number');
  assertTrue(MAX_DECOMPRESSED_SIZE > 0, 'Should be positive');
  // Should be at least 1MB and no more than 100MB for safety
  assertTrue(MAX_DECOMPRESSED_SIZE >= 1024 * 1024, 'Should be at least 1MB');
  assertTrue(MAX_DECOMPRESSED_SIZE <= 100 * 1024 * 1024, 'Should be at most 100MB');
});

// ============================================================================
// parseZstdFrameHeader Tests
// ============================================================================

console.log('\nparseZstdFrameHeader:');

test('returns invalid for empty data', () => {
  const result = parseZstdFrameHeader(new Uint8Array([]));
  assertEqual(result.valid, false);
  assertEqual(result.contentSize, null);
});

test('returns invalid for data shorter than 5 bytes', () => {
  const result = parseZstdFrameHeader(new Uint8Array([0x28, 0xB5, 0x2F, 0xFD]));
  assertEqual(result.valid, false);
  assertEqual(result.contentSize, null);
});

test('returns invalid for wrong magic number', () => {
  // Wrong magic number (not 0xFD2FB528)
  const result = parseZstdFrameHeader(new Uint8Array([0x00, 0x00, 0x00, 0x00, 0x00]));
  assertEqual(result.valid, false);
  assertEqual(result.contentSize, null);
});

test('returns valid for correct zstd magic number', () => {
  // Zstd magic: 0xFD2FB528 in little endian = 0x28, 0xB5, 0x2F, 0xFD
  // Descriptor byte with fcsFlag=0, singleSegmentFlag=0 means fcsSize=0
  const data = new Uint8Array([0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, null); // No content size field
});

test('parses 1-byte content size with single segment flag', () => {
  // Magic + descriptor with singleSegmentFlag=1 (0x20), fcsFlag=0
  // This means fcsSize=1 (singleSegmentFlag overrides)
  const data = new Uint8Array([0x28, 0xB5, 0x2F, 0xFD, 0x20, 42]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 42);
});

test('parses 2-byte content size with fcsFlag=1', () => {
  // Magic + descriptor with fcsFlag=1 (0x40), no singleSegmentFlag
  // fcsSize=2, offset is at 5 + windowDescSize(1) + dictIdSize(0) = 6
  // Need: magic(4) + desc(1) + window(1) + fcs(2) = 8 bytes
  const data = new Uint8Array([0x28, 0xB5, 0x2F, 0xFD, 0x40, 0x00, 0x00, 0x01]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  // 2-byte content size adds 256 offset: 0x0100 = 256, plus 256 offset = 512
  assertEqual(result.contentSize, 512);
});

// ============================================================================
// safeToNumber Tests
// ============================================================================

console.log('\nsafeToNumber:');

test('returns undefined for undefined input', () => {
  const result = safeToNumber(undefined, 'test');
  assertEqual(result, undefined);
});

test('converts small bigint to number', () => {
  const result = safeToNumber(BigInt(42), 'test');
  assertEqual(result, 42);
});

test('converts zero bigint to number', () => {
  const result = safeToNumber(BigInt(0), 'test');
  assertEqual(result, 0);
});

test('converts large but safe bigint to number', () => {
  const safeValue = BigInt(Number.MAX_SAFE_INTEGER);
  const result = safeToNumber(safeValue, 'test');
  assertEqual(result, Number.MAX_SAFE_INTEGER);
});

test('converts unsafe bigint but still returns number', () => {
  // This will lose precision but should still return a number
  const unsafeValue = BigInt(Number.MAX_SAFE_INTEGER) + BigInt(100);
  const result = safeToNumber(unsafeValue, 'test');
  assertTrue(result !== undefined, 'Should return a number');
  assertTrue(typeof result === 'number', 'Should be a number type');
});

// ============================================================================
// safeNonNegativeSequenceString Tests
// ============================================================================

console.log('\nsafeNonNegativeSequenceString:');

test('returns undefined for undefined input', () => {
  const result = safeNonNegativeSequenceString(undefined);
  assertEqual(result, undefined);
});

test('returns undefined for zero (M-1068: seq==0 means missing)', () => {
  const result = safeNonNegativeSequenceString(BigInt(0));
  assertEqual(result, undefined);
});

test('returns undefined for negative value', () => {
  const result = safeNonNegativeSequenceString(BigInt(-5));
  assertEqual(result, undefined);
});

test('returns string for positive value', () => {
  const result = safeNonNegativeSequenceString(BigInt(42));
  assertEqual(result, '42');
});

test('returns string for large positive value without precision loss', () => {
  const largeValue = BigInt('9007199254740993'); // MAX_SAFE_INTEGER + 2
  const result = safeNonNegativeSequenceString(largeValue);
  assertEqual(result, '9007199254740993');
});

// ============================================================================
// coerceU64ToStr Tests
// ============================================================================

console.log('\ncoerceU64ToStr:');

test('returns undefined for undefined', () => {
  const result = coerceU64ToStr(undefined);
  assertEqual(result, undefined);
});

test('returns undefined for null', () => {
  const result = coerceU64ToStr(null);
  assertEqual(result, undefined);
});

test('handles positive bigint', () => {
  const result = coerceU64ToStr(BigInt(123));
  assertEqual(result, '123');
});

test('returns undefined for zero bigint (M-1068)', () => {
  const result = coerceU64ToStr(BigInt(0));
  assertEqual(result, undefined);
});

test('returns undefined for negative bigint', () => {
  const result = coerceU64ToStr(BigInt(-5));
  assertEqual(result, undefined);
});

test('handles positive number', () => {
  const result = coerceU64ToStr(42);
  assertEqual(result, '42');
});

test('returns undefined for zero number (M-1068)', () => {
  const result = coerceU64ToStr(0);
  assertEqual(result, undefined);
});

test('returns undefined for negative number', () => {
  const result = coerceU64ToStr(-10);
  assertEqual(result, undefined);
});

test('returns undefined for non-integer number', () => {
  const result = coerceU64ToStr(3.14);
  assertEqual(result, undefined);
});

test('returns undefined for Infinity', () => {
  const result = coerceU64ToStr(Infinity);
  assertEqual(result, undefined);
});

test('returns undefined for NaN', () => {
  const result = coerceU64ToStr(NaN);
  assertEqual(result, undefined);
});

test('handles numeric string', () => {
  const result = coerceU64ToStr('12345');
  assertEqual(result, '12345');
});

test('normalizes string with leading zeros', () => {
  const result = coerceU64ToStr('00123');
  assertEqual(result, '123');
});

test('returns undefined for zero string (M-1068)', () => {
  const result = coerceU64ToStr('0');
  assertEqual(result, undefined);
});

test('returns undefined for non-numeric string', () => {
  const result = coerceU64ToStr('abc');
  assertEqual(result, undefined);
});

test('returns undefined for empty string', () => {
  const result = coerceU64ToStr('');
  assertEqual(result, undefined);
});

test('returns undefined for string with whitespace', () => {
  const result = coerceU64ToStr(' 123 ');
  assertEqual(result, undefined);
});

test('handles Long-like object with toString', () => {
  // Simulate protobufjs Long object
  const longLike = {
    low: 42,
    high: 0,
    unsigned: true,
    toString: () => '42',
  };
  const result = coerceU64ToStr(longLike);
  assertEqual(result, '42');
});

test('returns undefined for Long-like object with zero value', () => {
  const longLikeZero = {
    low: 0,
    high: 0,
    unsigned: true,
    toString: () => '0',
  };
  const result = coerceU64ToStr(longLikeZero);
  assertEqual(result, undefined);
});

test('returns undefined for non-Long objects', () => {
  const plainObject = { value: 42 };
  const result = coerceU64ToStr(plainObject);
  assertEqual(result, undefined);
});

// ============================================================================
// getEventTypeName Tests
// ============================================================================

console.log('\ngetEventTypeName:');

test('returns "unspecified" for UNSPECIFIED', () => {
  const result = getEventTypeName(EventType.EVENT_TYPE_UNSPECIFIED);
  assertEqual(result, 'unspecified');
});

test('returns "graph_start" for GRAPH_START', () => {
  const result = getEventTypeName(EventType.EVENT_TYPE_GRAPH_START);
  assertEqual(result, 'graph_start');
});

test('returns "node_start" for NODE_START', () => {
  const result = getEventTypeName(EventType.EVENT_TYPE_NODE_START);
  assertEqual(result, 'node_start');
});

test('returns "llm_start" for LLM_START', () => {
  const result = getEventTypeName(EventType.EVENT_TYPE_LLM_START);
  assertEqual(result, 'llm_start');
});

test('returns "unknown" for invalid event type', () => {
  const result = getEventTypeName(9999 as EventType);
  assertEqual(result, 'unknown');
});

test('all defined event types have names', () => {
  const definedEvents = [
    EventType.EVENT_TYPE_UNSPECIFIED,
    EventType.EVENT_TYPE_GRAPH_START,
    EventType.EVENT_TYPE_GRAPH_END,
    EventType.EVENT_TYPE_GRAPH_ERROR,
    EventType.EVENT_TYPE_NODE_START,
    EventType.EVENT_TYPE_NODE_END,
    EventType.EVENT_TYPE_NODE_ERROR,
    EventType.EVENT_TYPE_EDGE_TRAVERSAL,
    EventType.EVENT_TYPE_CONDITIONAL_BRANCH,
    EventType.EVENT_TYPE_LLM_START,
    EventType.EVENT_TYPE_LLM_END,
    EventType.EVENT_TYPE_LLM_ERROR,
    EventType.EVENT_TYPE_TOOL_START,
    EventType.EVENT_TYPE_TOOL_END,
    EventType.EVENT_TYPE_TOOL_ERROR,
  ];
  for (const eventType of definedEvents) {
    const name = getEventTypeName(eventType);
    assertTrue(name !== 'unknown', `Event type ${eventType} should have a name`);
    assertTrue(name.length > 0, `Event type ${eventType} name should not be empty`);
  }
});

// ============================================================================
// isNodeLifecycleEvent Tests
// ============================================================================

console.log('\nisNodeLifecycleEvent:');

test('returns true for NODE_START', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_NODE_START);
  assertEqual(result, true);
});

test('returns true for NODE_END', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_NODE_END);
  assertEqual(result, true);
});

test('returns true for NODE_ERROR', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_NODE_ERROR);
  assertEqual(result, true);
});

test('returns false for GRAPH_START', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_GRAPH_START);
  assertEqual(result, false);
});

test('returns false for LLM_START', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_LLM_START);
  assertEqual(result, false);
});

test('returns false for UNSPECIFIED', () => {
  const result = isNodeLifecycleEvent(EventType.EVENT_TYPE_UNSPECIFIED);
  assertEqual(result, false);
});

// ============================================================================
// isGraphLifecycleEvent Tests
// ============================================================================

console.log('\nisGraphLifecycleEvent:');

test('returns true for GRAPH_START', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_GRAPH_START);
  assertEqual(result, true);
});

test('returns true for GRAPH_END', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_GRAPH_END);
  assertEqual(result, true);
});

test('returns true for GRAPH_ERROR', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_GRAPH_ERROR);
  assertEqual(result, true);
});

test('returns false for NODE_START', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_NODE_START);
  assertEqual(result, false);
});

test('returns false for LLM_START', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_LLM_START);
  assertEqual(result, false);
});

test('returns false for UNSPECIFIED', () => {
  const result = isGraphLifecycleEvent(EventType.EVENT_TYPE_UNSPECIFIED);
  assertEqual(result, false);
});

// ============================================================================
// EventType Enum Consistency Tests
// ============================================================================

console.log('\nEventType enum consistency:');

test('node lifecycle events have consecutive values (10-12)', () => {
  assertEqual(EventType.EVENT_TYPE_NODE_START, 10);
  assertEqual(EventType.EVENT_TYPE_NODE_END, 11);
  assertEqual(EventType.EVENT_TYPE_NODE_ERROR, 12);
});

test('graph lifecycle events have consecutive values (1-3)', () => {
  assertEqual(EventType.EVENT_TYPE_GRAPH_START, 1);
  assertEqual(EventType.EVENT_TYPE_GRAPH_END, 2);
  assertEqual(EventType.EVENT_TYPE_GRAPH_ERROR, 3);
});

test('unspecified has value 0', () => {
  assertEqual(EventType.EVENT_TYPE_UNSPECIFIED, 0);
});

// ============================================================================
// MessageType Enum Tests
// ============================================================================

console.log('\nMessageType enum:');

test('MESSAGE_TYPE_UNSPECIFIED is 0', () => {
  assertEqual(MessageType.MESSAGE_TYPE_UNSPECIFIED, 0);
});

test('MESSAGE_TYPE_EVENT is 1', () => {
  assertEqual(MessageType.MESSAGE_TYPE_EVENT, 1);
});

test('MESSAGE_TYPE_STATE_DIFF is 2', () => {
  assertEqual(MessageType.MESSAGE_TYPE_STATE_DIFF, 2);
});

test('MESSAGE_TYPE_TOKEN_CHUNK is 3', () => {
  assertEqual(MessageType.MESSAGE_TYPE_TOKEN_CHUNK, 3);
});

test('MESSAGE_TYPE_TOOL_EXECUTION is 4', () => {
  assertEqual(MessageType.MESSAGE_TYPE_TOOL_EXECUTION, 4);
});

test('MESSAGE_TYPE_CHECKPOINT is 5', () => {
  assertEqual(MessageType.MESSAGE_TYPE_CHECKPOINT, 5);
});

test('MESSAGE_TYPE_METRICS is 6', () => {
  assertEqual(MessageType.MESSAGE_TYPE_METRICS, 6);
});

test('MESSAGE_TYPE_ERROR is 7', () => {
  assertEqual(MessageType.MESSAGE_TYPE_ERROR, 7);
});

test('MESSAGE_TYPE_EVENT_BATCH is 8', () => {
  assertEqual(MessageType.MESSAGE_TYPE_EVENT_BATCH, 8);
});

test('MESSAGE_TYPE_EXECUTION_TRACE is 9', () => {
  assertEqual(MessageType.MESSAGE_TYPE_EXECUTION_TRACE, 9);
});

test('all MessageType values are unique and sequential from 0', () => {
  const values = [
    MessageType.MESSAGE_TYPE_UNSPECIFIED,
    MessageType.MESSAGE_TYPE_EVENT,
    MessageType.MESSAGE_TYPE_STATE_DIFF,
    MessageType.MESSAGE_TYPE_TOKEN_CHUNK,
    MessageType.MESSAGE_TYPE_TOOL_EXECUTION,
    MessageType.MESSAGE_TYPE_CHECKPOINT,
    MessageType.MESSAGE_TYPE_METRICS,
    MessageType.MESSAGE_TYPE_ERROR,
    MessageType.MESSAGE_TYPE_EVENT_BATCH,
    MessageType.MESSAGE_TYPE_EXECUTION_TRACE,
  ];
  for (let i = 0; i < values.length; i++) {
    assertEqual(values[i], i, `MessageType at index ${i} should equal ${i}`);
  }
});

// ============================================================================
// Additional parseZstdFrameHeader Edge Cases
// ============================================================================

console.log('\nparseZstdFrameHeader edge cases:');

test('parses 4-byte content size with fcsFlag=2', () => {
  // Magic + descriptor with fcsFlag=2 (0x80), no singleSegmentFlag
  // fcsSize=4, need window descriptor (1 byte)
  // Need: magic(4) + desc(1) + window(1) + fcs(4) = 10 bytes
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x80,                    // Descriptor: fcsFlag=2, no single segment
    0x00,                    // Window descriptor
    0x00, 0x10, 0x00, 0x00,  // Content size: 0x00001000 = 4096
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  // 4-byte fcs adds 0 offset, so 4096 raw
  assertEqual(result.contentSize, 4096);
});

test('parses 8-byte content size with fcsFlag=3', () => {
  // Magic + descriptor with fcsFlag=3 (0xC0), no singleSegmentFlag
  // fcsSize=8, need window descriptor (1 byte)
  // Need: magic(4) + desc(1) + window(1) + fcs(8) = 14 bytes
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0xC0,                    // Descriptor: fcsFlag=3, no single segment
    0x00,                    // Window descriptor
    0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Content size: 0x2000 = 8192
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 8192);
});

test('handles dictIdFlag=1 (1 byte dictId)', () => {
  // Magic + descriptor with singleSegmentFlag + dictIdFlag=1
  // Descriptor: 0x20 (singleSegment) | 0x01 (dictIdFlag=1) = 0x21
  // Need: magic(4) + desc(1) + dictId(1) + fcs(1) = 7 bytes
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x21,                    // Descriptor: singleSegment + dictIdFlag=1
    0xFF,                    // DictId (1 byte)
    100,                     // Content size
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 100);
});

test('handles dictIdFlag=2 (2 byte dictId)', () => {
  // Descriptor: 0x20 (singleSegment) | 0x02 (dictIdFlag=2) = 0x22
  // Need: magic(4) + desc(1) + dictId(2) + fcs(1) = 8 bytes
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x22,                    // Descriptor: singleSegment + dictIdFlag=2
    0xAB, 0xCD,              // DictId (2 bytes)
    50,                      // Content size
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 50);
});

test('handles dictIdFlag=3 (4 byte dictId)', () => {
  // Descriptor: 0x20 (singleSegment) | 0x03 (dictIdFlag=3) = 0x23
  // Need: magic(4) + desc(1) + dictId(4) + fcs(1) = 10 bytes
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x23,                    // Descriptor: singleSegment + dictIdFlag=3
    0x01, 0x02, 0x03, 0x04,  // DictId (4 bytes)
    75,                      // Content size
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 75);
});

test('returns invalid when frame header is truncated', () => {
  // Valid magic, but truncated before content size
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x20,                    // Descriptor: singleSegment (needs 1 byte fcs)
    // Missing content size byte
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, false);
  assertEqual(result.contentSize, null);
});

test('handles large numbers beyond MAX_SAFE_INTEGER', () => {
  // Use 8-byte fcs with a value that exceeds MAX_SAFE_INTEGER
  // MAX_SAFE_INTEGER = 9007199254740991 = 0x001FFFFFFFFFFFFF
  // Use 0x0020000000000000 = 9007199254740992 (MAX_SAFE_INTEGER + 1)
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0xC0,                    // Descriptor: fcsFlag=3, no single segment
    0x00,                    // Window descriptor
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, // 0x0020000000000000 in LE
  ]);
  const result = parseZstdFrameHeader(data);
  // Should still parse, but content size may lose precision
  assertEqual(result.valid, true);
  assertTrue(result.contentSize !== null, 'Should return a content size');
});

test('handles zero', () => {
  // 1-byte fcs with value 0
  const data = new Uint8Array([
    0x28, 0xB5, 0x2F, 0xFD, // Magic
    0x20,                    // Descriptor: singleSegment
    0x00,                    // Content size = 0
  ]);
  const result = parseZstdFrameHeader(data);
  assertEqual(result.valid, true);
  assertEqual(result.contentSize, 0);
});

test('handles negative sequences (synthetic)', () => {
  // This tests safeNonNegativeSequenceString indirectly - negative values indicate synthetic sequences
  const result = safeNonNegativeSequenceString(BigInt(-42));
  assertEqual(result, undefined);
});

// ============================================================================
// Summary
// ============================================================================

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
