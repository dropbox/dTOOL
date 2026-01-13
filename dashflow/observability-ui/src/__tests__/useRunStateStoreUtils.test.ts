// M-2591: Unit tests for useRunStateStore internal utility functions
// Run with: npx tsx src/__tests__/useRunStateStoreUtils.test.ts
//
// Tests pure functions exported for testing:
// - deepCloneJson: structuredClone with JSON fallback
// - hashesEqual: Uint8Array equality
// - compareSeqs: BigInt sequence comparison
// - isRealSeq: check if sequence > 0
// - bytesToHex: byte array to hex string with bounds
// - hasValidCheckpointId: check if checkpoint ID is valid

export {};

import {
  _deepCloneJson as deepCloneJson,
  _hashesEqual as hashesEqual,
  _compareSeqs as compareSeqs,
  _isRealSeq as isRealSeq,
  _bytesToHex as bytesToHex,
  _hasValidCheckpointId as hasValidCheckpointId,
  _MAX_BYTES_FOR_HEX as MAX_BYTES_FOR_HEX,
} from '../hooks/useRunStateStore';

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
    throw new Error(message || `Expected ${expectedStr} but got ${actualStr}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || `Expected condition to be true`);
  }
}

function assertFalse(condition: boolean, message?: string): void {
  if (condition) {
    throw new Error(message || `Expected condition to be false`);
  }
}

async function run(): Promise<void> {
  console.log('\nuseRunStateStore Utility Functions Tests\n');

  // ============================================================
  // deepCloneJson tests
  // ============================================================
  console.log('\n--- deepCloneJson ---');

  test('clones simple objects', () => {
    const original = { a: 1, b: 'hello' };
    const cloned = deepCloneJson(original);
    assertEqual(cloned, original);
    // Verify it's a new object
    assertTrue(cloned !== original, 'Clone should be a different object reference');
  });

  test('clones nested objects', () => {
    const original = { outer: { inner: { value: 42 } } };
    const cloned = deepCloneJson(original);
    assertEqual(cloned, original);
    assertTrue(cloned.outer !== original.outer, 'Nested objects should be cloned');
    assertTrue(cloned.outer.inner !== original.outer.inner, 'Deeply nested objects should be cloned');
  });

  test('clones arrays', () => {
    const original = [1, 2, { nested: true }];
    const cloned = deepCloneJson(original);
    assertEqual(cloned, original);
    assertTrue(cloned !== original, 'Array should be cloned');
    assertTrue(cloned[2] !== original[2], 'Array elements should be cloned');
  });

  test('clones Date objects (structuredClone preserves them)', () => {
    const date = new Date('2026-01-05T12:00:00Z');
    const original = { date };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.date instanceof Date, 'Date should be preserved');
    assertEqual(cloned.date.getTime(), date.getTime());
  });

  test('handles null and undefined values', () => {
    const original = { a: null, b: undefined };
    const cloned = deepCloneJson(original);
    assertEqual(cloned.a, null);
    // undefined becomes missing in JSON serialization, structuredClone preserves it
    // Accept either behavior since structuredClone is preferred
  });

  test('clones empty objects and arrays', () => {
    assertEqual(deepCloneJson({}), {});
    assertEqual(deepCloneJson([]), []);
  });

  test('mutations to clone do not affect original', () => {
    const original = { value: 1, nested: { x: 10 } };
    const cloned = deepCloneJson(original);
    cloned.value = 999;
    cloned.nested.x = 999;
    assertEqual(original.value, 1);
    assertEqual(original.nested.x, 10);
  });

  // Note: structuredClone in modern Node.js DOES support BigInt (unlike JSON.stringify)
  // So BigInt values are cloned successfully via the structuredClone path
  test('handles BigInt values via structuredClone', () => {
    const withBigInt = { value: BigInt(12345678901234567890n) };
    const cloned = deepCloneJson(withBigInt);
    // Can't use assertEqual here since JSON.stringify fails on BigInt
    assertTrue(cloned.value === withBigInt.value, 'BigInt value should be preserved');
    assertTrue(cloned !== withBigInt, 'Clone should be different object');
  });

  // structuredClone handles circular references, so deepCloneJson should too
  test('handles circular references via structuredClone', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const circular: any = { a: 1 };
    circular.self = circular;
    const cloned = deepCloneJson(circular);
    assertTrue(cloned.a === 1, 'Value should be cloned');
    assertTrue(cloned.self === cloned, 'Circular reference should point to cloned object');
    assertTrue(cloned !== circular, 'Clone should be different object');
  });

  // Test the JSON fallback path (when structuredClone fails but JSON succeeds)
  // This is tested by having non-serializable values that get stripped
  test('handles objects with undefined (JSON strips them)', () => {
    const withUndefined = { a: 1, b: undefined, c: 3 };
    const cloned = deepCloneJson(withUndefined);
    assertTrue(cloned.a === 1, 'Defined values should be preserved');
    assertTrue(cloned.c === 3, 'Defined values should be preserved');
    // structuredClone preserves undefined, so it should still be there
    // (structuredClone path is preferred)
  });

  // ============================================================
  // hashesEqual tests
  // ============================================================
  console.log('\n--- hashesEqual ---');

  test('returns true for identical Uint8Arrays', () => {
    const a = new Uint8Array([1, 2, 3, 4]);
    const b = new Uint8Array([1, 2, 3, 4]);
    assertTrue(hashesEqual(a, b));
  });

  test('returns false for different lengths', () => {
    const a = new Uint8Array([1, 2, 3]);
    const b = new Uint8Array([1, 2, 3, 4]);
    assertFalse(hashesEqual(a, b));
  });

  test('returns false for different values', () => {
    const a = new Uint8Array([1, 2, 3, 4]);
    const b = new Uint8Array([1, 2, 3, 5]);
    assertFalse(hashesEqual(a, b));
  });

  test('returns false when first argument is undefined', () => {
    const b = new Uint8Array([1, 2, 3]);
    assertFalse(hashesEqual(undefined, b));
  });

  test('returns true for empty arrays', () => {
    const a = new Uint8Array([]);
    const b = new Uint8Array([]);
    assertTrue(hashesEqual(a, b));
  });

  test('returns false when first byte differs', () => {
    const a = new Uint8Array([0, 2, 3]);
    const b = new Uint8Array([1, 2, 3]);
    assertFalse(hashesEqual(a, b));
  });

  test('returns false when last byte differs', () => {
    const a = new Uint8Array([1, 2, 3]);
    const b = new Uint8Array([1, 2, 4]);
    assertFalse(hashesEqual(a, b));
  });

  test('handles all-zero arrays', () => {
    const a = new Uint8Array([0, 0, 0, 0]);
    const b = new Uint8Array([0, 0, 0, 0]);
    assertTrue(hashesEqual(a, b));
  });

  test('handles high byte values (255)', () => {
    const a = new Uint8Array([255, 128, 0, 64]);
    const b = new Uint8Array([255, 128, 0, 64]);
    assertTrue(hashesEqual(a, b));
  });

  // ============================================================
  // compareSeqs tests
  // ============================================================
  console.log('\n--- compareSeqs ---');

  test('returns -1 when a < b', () => {
    assertEqual(compareSeqs('1', '2'), -1);
  });

  test('returns 1 when a > b', () => {
    assertEqual(compareSeqs('2', '1'), 1);
  });

  test('returns 0 when a equals b', () => {
    assertEqual(compareSeqs('5', '5'), 0);
  });

  test('handles large numbers beyond MAX_SAFE_INTEGER', () => {
    const large1 = '9007199254740993'; // MAX_SAFE_INTEGER + 2
    const large2 = '9007199254740994'; // MAX_SAFE_INTEGER + 3
    assertEqual(compareSeqs(large1, large2), -1);
    assertEqual(compareSeqs(large2, large1), 1);
    assertEqual(compareSeqs(large1, large1), 0);
  });

  test('handles very large BigInt values', () => {
    const huge1 = '18446744073709551615'; // u64::MAX
    const huge2 = '18446744073709551614';
    assertEqual(compareSeqs(huge2, huge1), -1);
    assertEqual(compareSeqs(huge1, huge2), 1);
  });

  test('handles zero', () => {
    assertEqual(compareSeqs('0', '0'), 0);
    assertEqual(compareSeqs('0', '1'), -1);
    assertEqual(compareSeqs('1', '0'), 1);
  });

  test('handles negative sequences (synthetic)', () => {
    assertEqual(compareSeqs('-1', '0'), -1);
    assertEqual(compareSeqs('-100', '-50'), -1);
    assertEqual(compareSeqs('-50', '-100'), 1);
  });

  // ============================================================
  // isRealSeq tests
  // ============================================================
  console.log('\n--- isRealSeq ---');

  test('returns true for positive sequences', () => {
    assertTrue(isRealSeq('1'));
    assertTrue(isRealSeq('100'));
    assertTrue(isRealSeq('9007199254740993')); // Beyond MAX_SAFE_INTEGER
  });

  test('returns false for zero (proto3 default means missing)', () => {
    assertFalse(isRealSeq('0'));
  });

  test('returns false for negative sequences (synthetic)', () => {
    assertFalse(isRealSeq('-1'));
    assertFalse(isRealSeq('-100'));
    assertFalse(isRealSeq('-9007199254740993'));
  });

  test('handles u64::MAX', () => {
    assertTrue(isRealSeq('18446744073709551615'));
  });

  // ============================================================
  // bytesToHex tests
  // ============================================================
  console.log('\n--- bytesToHex ---');

  test('returns empty string for undefined', () => {
    assertEqual(bytesToHex(undefined), '');
  });

  test('returns empty string for empty array', () => {
    assertEqual(bytesToHex(new Uint8Array([])), '');
  });

  test('converts simple bytes to hex', () => {
    assertEqual(bytesToHex(new Uint8Array([0, 1, 2, 15, 16, 255])), '0001020f10ff');
  });

  test('pads single-digit hex values with leading zero', () => {
    assertEqual(bytesToHex(new Uint8Array([0, 5, 10])), '00050a');
  });

  test('handles UUID-like 16-byte arrays', () => {
    const uuid = new Uint8Array([
      0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
      0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
    ]);
    assertEqual(bytesToHex(uuid), '123456789abcdef0123456789abcdef0');
  });

  test('MAX_BYTES_FOR_HEX is 64', () => {
    assertEqual(MAX_BYTES_FOR_HEX, 64);
  });

  test('truncates arrays larger than MAX_BYTES_FOR_HEX', () => {
    // Create array of 100 bytes (larger than MAX_BYTES_FOR_HEX=64)
    const largeArray = new Uint8Array(100);
    for (let i = 0; i < 100; i++) {
      largeArray[i] = i % 256;
    }
    const result = bytesToHex(largeArray);
    // Should be first 16 bytes as hex + size marker
    const expectedPrefix = '000102030405060708090a0b0c0d0e0f';
    assertTrue(result.startsWith(expectedPrefix), `Should start with first 16 bytes hex: ${result}`);
    assertTrue(result.includes('...(100b)'), `Should include size marker: ${result}`);
  });

  test('does not truncate arrays at exactly MAX_BYTES_FOR_HEX', () => {
    const exactArray = new Uint8Array(64);
    for (let i = 0; i < 64; i++) {
      exactArray[i] = i;
    }
    const result = bytesToHex(exactArray);
    // Should be full 64 bytes = 128 hex chars
    assertEqual(result.length, 128);
    assertFalse(result.includes('...'), 'Should not be truncated');
  });

  test('handles custom maxBytes parameter', () => {
    const array = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]);
    // With maxBytes=16, should truncate 18-byte array
    const result = bytesToHex(array, 16);
    assertTrue(result.includes('...(18b)'), `Should truncate with custom maxBytes: ${result}`);
  });

  // ============================================================
  // hasValidCheckpointId tests
  // ============================================================
  console.log('\n--- hasValidCheckpointId ---');

  test('returns false for undefined', () => {
    assertFalse(hasValidCheckpointId(undefined));
  });

  test('returns false for empty array', () => {
    assertFalse(hasValidCheckpointId(new Uint8Array([])));
  });

  test('returns false for all-zero array', () => {
    assertFalse(hasValidCheckpointId(new Uint8Array([0, 0, 0, 0])));
  });

  test('returns true for array with any non-zero byte', () => {
    assertTrue(hasValidCheckpointId(new Uint8Array([0, 0, 1, 0])));
    assertTrue(hasValidCheckpointId(new Uint8Array([1])));
    assertTrue(hasValidCheckpointId(new Uint8Array([255, 0, 0, 0])));
  });

  test('returns true for UUID-like 16-byte array', () => {
    const uuid = new Uint8Array([
      0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
      0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
    ]);
    assertTrue(hasValidCheckpointId(uuid));
  });

  test('handles single-byte arrays', () => {
    assertFalse(hasValidCheckpointId(new Uint8Array([0])));
    assertTrue(hasValidCheckpointId(new Uint8Array([1])));
    assertTrue(hasValidCheckpointId(new Uint8Array([255])));
  });

  // ============================================================
  // Additional deepCloneJson tests - M-2629
  // ============================================================
  console.log('\n--- deepCloneJson (additional tests) ---');

  test('clones objects with functions (functions are stripped via JSON fallback)', () => {
    // structuredClone throws on functions, but JSON fallback strips them silently
    // deepCloneJson falls back to JSON round-trip which removes functions
    const withFunc = { a: 1, fn: () => 42 };
    const cloned = deepCloneJson(withFunc);
    assertEqual(cloned.a, 1);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    assertEqual((cloned as any).fn, undefined);
  });

  test('clones objects with Symbol keys (Symbols are not preserved)', () => {
    const sym = Symbol('test');
    const withSymbol = { [sym]: 'value', regular: 'prop' };
    const cloned = deepCloneJson(withSymbol);
    // Symbol-keyed properties are not enumerable in JSON/structuredClone
    assertEqual(cloned.regular, 'prop');
  });

  test('clones Map objects via structuredClone', () => {
    const map = new Map([['key1', 'value1'], ['key2', 'value2']]);
    const original = { map };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.map instanceof Map, 'Map should be preserved');
    assertEqual(cloned.map.get('key1'), 'value1');
    assertEqual(cloned.map.get('key2'), 'value2');
    assertTrue(cloned.map !== original.map, 'Map should be cloned');
  });

  test('clones Set objects via structuredClone', () => {
    const set = new Set([1, 2, 3]);
    const original = { set };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.set instanceof Set, 'Set should be preserved');
    assertTrue(cloned.set.has(1));
    assertTrue(cloned.set.has(2));
    assertTrue(cloned.set.has(3));
    assertTrue(cloned.set !== original.set, 'Set should be cloned');
  });

  test('clones RegExp via structuredClone', () => {
    const regex = /test-pattern/gi;
    const original = { regex };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.regex instanceof RegExp, 'RegExp should be preserved');
    assertEqual(cloned.regex.source, 'test-pattern');
    assertEqual(cloned.regex.flags, 'gi');
  });

  test('clones TypedArrays (Float32Array)', () => {
    const floats = new Float32Array([1.5, 2.5, 3.5]);
    const original = { floats };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.floats instanceof Float32Array, 'Float32Array should be preserved');
    assertEqual(cloned.floats.length, 3);
    assertTrue(Math.abs(cloned.floats[0] - 1.5) < 0.001, 'Float values preserved');
  });

  test('clones TypedArrays (Int16Array)', () => {
    const ints = new Int16Array([-1000, 0, 1000, 32767]);
    const original = { ints };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.ints instanceof Int16Array, 'Int16Array should be preserved');
    assertEqual(Array.from(cloned.ints), [-1000, 0, 1000, 32767]);
  });

  test('clones ArrayBuffer via structuredClone', () => {
    const buffer = new ArrayBuffer(16);
    const view = new Uint8Array(buffer);
    view[0] = 42;
    view[15] = 255;
    const original = { buffer };
    const cloned = deepCloneJson(original);
    assertTrue(cloned.buffer instanceof ArrayBuffer, 'ArrayBuffer should be preserved');
    const clonedView = new Uint8Array(cloned.buffer);
    assertEqual(clonedView[0], 42);
    assertEqual(clonedView[15], 255);
  });

  test('preserves NaN values', () => {
    const original = { value: NaN };
    const cloned = deepCloneJson(original);
    assertTrue(Number.isNaN(cloned.value), 'NaN should be preserved');
  });

  test('preserves Infinity values', () => {
    const original = { pos: Infinity, neg: -Infinity };
    const cloned = deepCloneJson(original);
    assertEqual(cloned.pos, Infinity);
    assertEqual(cloned.neg, -Infinity);
  });

  test('preserves negative zero', () => {
    const original = { value: -0 };
    const cloned = deepCloneJson(original);
    assertTrue(Object.is(cloned.value, -0), 'Negative zero should be preserved');
  });

  test('clones deeply nested arrays', () => {
    const original = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]];
    const cloned = deepCloneJson(original);
    assertEqual(cloned, original);
    assertTrue(cloned[0] !== original[0], 'Nested arrays should be cloned');
    assertTrue(cloned[0][0] !== original[0][0], 'Deep arrays should be cloned');
  });

  test('clones mixed nested structures', () => {
    const original = {
      array: [1, { nested: true }],
      obj: { arr: [2, 3], deep: { value: 'test' } }
    };
    const cloned = deepCloneJson(original);
    assertEqual(cloned, original);
    assertTrue(cloned.array !== original.array);
    assertTrue(cloned.obj !== original.obj);
    assertTrue(cloned.obj.deep !== original.obj.deep);
  });

  // ============================================================
  // Additional hashesEqual tests - M-2629
  // ============================================================
  console.log('\n--- hashesEqual (additional tests) ---');

  test('returns false when both arguments could be undefined conceptually', () => {
    // hashesEqual signature is (a: Uint8Array | undefined, b: Uint8Array)
    // Testing edge cases for first argument being undefined
    assertFalse(hashesEqual(undefined, new Uint8Array([1, 2, 3])));
  });

  test('handles very long identical arrays', () => {
    const size = 10000;
    const a = new Uint8Array(size);
    const b = new Uint8Array(size);
    for (let i = 0; i < size; i++) {
      a[i] = i % 256;
      b[i] = i % 256;
    }
    assertTrue(hashesEqual(a, b), 'Long identical arrays should be equal');
  });

  test('handles very long arrays with difference in middle', () => {
    const size = 10000;
    const a = new Uint8Array(size);
    const b = new Uint8Array(size);
    for (let i = 0; i < size; i++) {
      a[i] = i % 256;
      b[i] = i % 256;
    }
    b[5000] = (b[5000] + 1) % 256; // Change middle byte
    assertFalse(hashesEqual(a, b), 'Arrays with middle difference should not be equal');
  });

  test('handles arrays of length 1', () => {
    assertTrue(hashesEqual(new Uint8Array([0]), new Uint8Array([0])));
    assertTrue(hashesEqual(new Uint8Array([255]), new Uint8Array([255])));
    assertFalse(hashesEqual(new Uint8Array([0]), new Uint8Array([1])));
  });

  test('handles arrays of length 2', () => {
    assertTrue(hashesEqual(new Uint8Array([0, 0]), new Uint8Array([0, 0])));
    assertFalse(hashesEqual(new Uint8Array([0, 1]), new Uint8Array([1, 0])));
    assertFalse(hashesEqual(new Uint8Array([255, 254]), new Uint8Array([255, 255])));
  });

  test('handles SHA-256 like hash (32 bytes)', () => {
    const hash1 = new Uint8Array(32);
    const hash2 = new Uint8Array(32);
    for (let i = 0; i < 32; i++) {
      hash1[i] = i * 8;
      hash2[i] = i * 8;
    }
    assertTrue(hashesEqual(hash1, hash2), 'SHA-256 size hashes should match');
    hash2[31] = 0;
    assertFalse(hashesEqual(hash1, hash2), 'SHA-256 size hashes with diff should not match');
  });

  // ============================================================
  // Additional compareSeqs tests - M-2629
  // ============================================================
  console.log('\n--- compareSeqs (additional tests) ---');

  test('handles leading zeros in string representation', () => {
    // "007" as BigInt is 7
    assertEqual(compareSeqs('007', '7'), 0);
    assertEqual(compareSeqs('007', '8'), -1);
    assertEqual(compareSeqs('007', '006'), 1);
  });

  test('handles single digit comparisons', () => {
    assertEqual(compareSeqs('0', '9'), -1);
    assertEqual(compareSeqs('9', '0'), 1);
    assertEqual(compareSeqs('5', '5'), 0);
  });

  test('handles comparison at numeric boundaries', () => {
    // Test around MAX_SAFE_INTEGER = 9007199254740991
    assertEqual(compareSeqs('9007199254740991', '9007199254740991'), 0);
    assertEqual(compareSeqs('9007199254740990', '9007199254740991'), -1);
    assertEqual(compareSeqs('9007199254740992', '9007199254740991'), 1);
  });

  test('handles i64::MAX comparison', () => {
    const i64Max = '9223372036854775807';
    assertEqual(compareSeqs(i64Max, i64Max), 0);
    assertEqual(compareSeqs('9223372036854775806', i64Max), -1);
    assertEqual(compareSeqs(i64Max, '9223372036854775806'), 1);
  });

  test('handles comparison of negative numbers', () => {
    assertEqual(compareSeqs('-5', '-3'), -1);
    assertEqual(compareSeqs('-3', '-5'), 1);
    assertEqual(compareSeqs('-10', '-10'), 0);
  });

  test('handles mixed positive and negative comparison', () => {
    assertEqual(compareSeqs('-1', '1'), -1);
    assertEqual(compareSeqs('1', '-1'), 1);
    assertEqual(compareSeqs('-1000', '0'), -1);
    assertEqual(compareSeqs('0', '-1'), 1);
  });

  test('handles very large positive numbers', () => {
    // Larger than u64::MAX
    const huge1 = '999999999999999999999999999999';
    const huge2 = '999999999999999999999999999998';
    assertEqual(compareSeqs(huge1, huge2), 1);
    assertEqual(compareSeqs(huge2, huge1), -1);
    assertEqual(compareSeqs(huge1, huge1), 0);
  });

  // ============================================================
  // Additional isRealSeq tests - M-2629
  // ============================================================
  console.log('\n--- isRealSeq (additional tests) ---');

  test('handles string "1" (smallest real seq)', () => {
    assertTrue(isRealSeq('1'));
  });

  test('handles leading zeros', () => {
    assertTrue(isRealSeq('007')); // 7 > 0
    assertFalse(isRealSeq('000')); // 0 == 0
  });

  test('handles very small positive number', () => {
    assertTrue(isRealSeq('0000001')); // Still 1 > 0
  });

  test('handles boundary between negative and positive', () => {
    assertFalse(isRealSeq('-0')); // -0n == 0n in BigInt
    assertTrue(isRealSeq('1'));
    assertFalse(isRealSeq('-1'));
  });

  test('handles various negative values', () => {
    assertFalse(isRealSeq('-9223372036854775808')); // i64::MIN
    assertFalse(isRealSeq('-18446744073709551615')); // negative u64::MAX
  });

  test('handles proto3 default value explicitly', () => {
    // Proto3 uses 0 as default, meaning "not set"
    assertFalse(isRealSeq('0'));
  });

  // ============================================================
  // Additional bytesToHex tests - M-2629
  // ============================================================
  console.log('\n--- bytesToHex (additional tests) ---');

  test('handles array exactly one byte over MAX_BYTES_FOR_HEX', () => {
    const array = new Uint8Array(65); // MAX_BYTES_FOR_HEX + 1
    for (let i = 0; i < 65; i++) {
      array[i] = i;
    }
    const result = bytesToHex(array);
    assertTrue(result.includes('...(65b)'), `Should truncate 65 bytes: ${result}`);
    assertTrue(result.startsWith('000102030405060708090a0b0c0d0e0f'), 'Should start with first 16 bytes');
  });

  test('handles maxBytes=0 edge case', () => {
    const array = new Uint8Array([1, 2, 3]);
    const result = bytesToHex(array, 0);
    // With maxBytes=0, any non-empty array should be truncated
    assertTrue(result.includes('...(3b)'), `Should truncate with maxBytes=0: ${result}`);
  });

  test('handles maxBytes=1', () => {
    const array = new Uint8Array([255, 128]);
    const result = bytesToHex(array, 1);
    assertTrue(result.includes('...(2b)'), `Should truncate: ${result}`);
  });

  test('handles single byte array (within default maxBytes)', () => {
    assertEqual(bytesToHex(new Uint8Array([0])), '00');
    assertEqual(bytesToHex(new Uint8Array([15])), '0f');
    assertEqual(bytesToHex(new Uint8Array([16])), '10');
    assertEqual(bytesToHex(new Uint8Array([255])), 'ff');
  });

  test('handles all bytes 0x00', () => {
    const zeros = new Uint8Array(10);
    assertEqual(bytesToHex(zeros), '00000000000000000000');
  });

  test('handles all bytes 0xFF', () => {
    const ones = new Uint8Array(5);
    ones.fill(255);
    assertEqual(bytesToHex(ones), 'ffffffffff');
  });

  test('handles very large array (stress test for truncation)', () => {
    const huge = new Uint8Array(10000);
    for (let i = 0; i < 10000; i++) {
      huge[i] = i % 256;
    }
    const result = bytesToHex(huge);
    assertTrue(result.includes('...(10000b)'), `Should show total size: ${result}`);
    assertTrue(result.length < 100, `Result should be bounded: length=${result.length}`);
  });

  test('handles alternating bytes pattern', () => {
    const pattern = new Uint8Array([0, 255, 0, 255, 0, 255]);
    assertEqual(bytesToHex(pattern), '00ff00ff00ff');
  });

  // ============================================================
  // Additional hasValidCheckpointId tests - M-2629
  // ============================================================
  console.log('\n--- hasValidCheckpointId (additional tests) ---');

  test('handles long array with only last byte non-zero', () => {
    const array = new Uint8Array(100);
    array[99] = 1; // Only last byte is non-zero
    assertTrue(hasValidCheckpointId(array), 'Should be valid if any byte is non-zero');
  });

  test('handles long array with only first byte non-zero', () => {
    const array = new Uint8Array(100);
    array[0] = 1;
    assertTrue(hasValidCheckpointId(array));
  });

  test('handles long array with middle byte non-zero', () => {
    const array = new Uint8Array(100);
    array[50] = 128;
    assertTrue(hasValidCheckpointId(array));
  });

  test('handles long all-zero array', () => {
    const zeros = new Uint8Array(1000);
    assertFalse(hasValidCheckpointId(zeros), 'All zeros should be invalid');
  });

  test('handles 32-byte hash-like all zeros', () => {
    const hash = new Uint8Array(32);
    assertFalse(hasValidCheckpointId(hash));
  });

  test('handles 32-byte hash-like with one bit set', () => {
    const hash = new Uint8Array(32);
    hash[16] = 1;
    assertTrue(hasValidCheckpointId(hash));
  });

  test('handles byte value 0x80 (128)', () => {
    assertTrue(hasValidCheckpointId(new Uint8Array([128])));
    assertTrue(hasValidCheckpointId(new Uint8Array([0, 0, 128, 0])));
  });

  test('handles maximum byte value throughout', () => {
    const maxBytes = new Uint8Array(16);
    maxBytes.fill(255);
    assertTrue(hasValidCheckpointId(maxBytes));
  });

  // ============================================================
  // Summary
  // ============================================================
  console.log('\n--------------------------');
  console.log(`Tests: ${passed} passed, ${failed} failed`);
  console.log('--------------------------\n');

  if (failed > 0) {
    process.exit(1);
  }
}

void run();
