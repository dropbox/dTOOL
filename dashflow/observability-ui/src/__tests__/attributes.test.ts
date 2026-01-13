// Unit tests for protobuf AttributeValue helpers
// Run with: npx tsx src/__tests__/attributes.test.ts

import { getJsonAttribute, getStringAttribute, getNumberAttribute, utf8ByteLengthCapped, boundAttributes } from '../utils/attributes';

let passed = 0;
let failed = 0;

async function test(name: string, fn: () => void | Promise<void>): Promise<void> {
  try {
    await fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (e) {
    console.log(`  ✗ ${name}`);
    console.log(`    Error: ${e}`);
    failed++;
  }
}

function assertEqual(actual: unknown, expected: unknown, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(`${message || 'Assertion failed'}: expected ${expectedStr}, got ${actualStr}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || 'Expected condition to be true');
  }
}

async function run(): Promise<void> {
  console.log('\nAttributes Tests\n');

  // ============================================================
  // getStringAttribute tests
  // ============================================================
  console.log('--- getStringAttribute ---');

  await test('returns direct strings', () => {
    assertEqual(getStringAttribute({ schema_id: 'abc' }, 'schema_id'), 'abc');
  });

  await test('returns AttributeValue wrapper strings', () => {
    assertEqual(getStringAttribute({ schema_id: { stringValue: 'abc' } }, 'schema_id'), 'abc');
  });

  await test('returns undefined for missing keys', () => {
    assertEqual(getStringAttribute({}, 'schema_id'), undefined);
  });

  await test('returns undefined for null values', () => {
    assertEqual(getStringAttribute({ key: null }, 'key'), undefined);
  });

  await test('returns undefined for undefined values', () => {
    assertEqual(getStringAttribute({ key: undefined }, 'key'), undefined);
  });

  await test('returns undefined for number values', () => {
    assertEqual(getStringAttribute({ key: 42 }, 'key'), undefined);
  });

  await test('returns undefined for boolean values', () => {
    assertEqual(getStringAttribute({ key: true }, 'key'), undefined);
    assertEqual(getStringAttribute({ key: false }, 'key'), undefined);
  });

  await test('returns empty string for empty string values', () => {
    assertEqual(getStringAttribute({ key: '' }, 'key'), '');
  });

  await test('returns empty string from AttributeValue wrapper', () => {
    assertEqual(getStringAttribute({ key: { stringValue: '' } }, 'key'), '');
  });

  await test('returns undefined for objects without stringValue', () => {
    assertEqual(getStringAttribute({ key: { intValue: 42 } }, 'key'), undefined);
    assertEqual(getStringAttribute({ key: { someOtherKey: 'value' } }, 'key'), undefined);
  });

  await test('returns undefined for arrays', () => {
    assertEqual(getStringAttribute({ key: ['a', 'b'] }, 'key'), undefined);
  });

  await test('handles special string characters', () => {
    assertEqual(getStringAttribute({ key: 'hello\nworld' }, 'key'), 'hello\nworld');
    assertEqual(getStringAttribute({ key: 'tab\there' }, 'key'), 'tab\there');
    assertEqual(getStringAttribute({ key: '🎉' }, 'key'), '🎉');
  });

  await test('handles Unicode strings', () => {
    assertEqual(getStringAttribute({ key: '日本語' }, 'key'), '日本語');
    assertEqual(getStringAttribute({ key: { stringValue: 'Привет' } }, 'key'), 'Привет');
  });

  // ============================================================
  // getJsonAttribute tests
  // ============================================================
  console.log('\n--- getJsonAttribute ---');

  await test('parses JSON from AttributeValue wrapper', () => {
    assertEqual(getJsonAttribute({ payload: { stringValue: '{"a":1}' } }, 'payload'), { a: 1 });
  });

  await test('skips oversized JSON strings', () => {
    const bigJson = `{"a":"${'x'.repeat(1000)}"}`;
    assertEqual(getJsonAttribute({ payload: { stringValue: bigJson } }, 'payload', { maxBytes: 100 }), undefined);
  });

  await test('returns undefined for missing key', () => {
    assertEqual(getJsonAttribute({}, 'missing'), undefined);
  });

  await test('returns undefined for null value', () => {
    assertEqual(getJsonAttribute({ key: null }, 'key'), undefined);
  });

  await test('returns undefined for undefined value', () => {
    assertEqual(getJsonAttribute({ key: undefined }, 'key'), undefined);
  });

  await test('parses nested JSON objects', () => {
    const nested = { stringValue: '{"a":{"b":{"c":1}}}' };
    const result = getJsonAttribute({ payload: nested }, 'payload');
    assertEqual(result, { a: { b: { c: 1 } } });
  });

  await test('parses JSON arrays', () => {
    assertEqual(getJsonAttribute({ arr: { stringValue: '[1,2,3]' } }, 'arr'), [1, 2, 3]);
  });

  await test('parses JSON with mixed types', () => {
    const json = '{"num":1,"str":"hello","bool":true,"null":null,"arr":[1,"a"]}';
    const expected = { num: 1, str: 'hello', bool: true, null: null, arr: [1, 'a'] };
    assertEqual(getJsonAttribute({ data: { stringValue: json } }, 'data'), expected);
  });

  await test('returns undefined for invalid JSON', () => {
    assertEqual(getJsonAttribute({ bad: { stringValue: '{invalid json}' } }, 'bad'), undefined);
    assertEqual(getJsonAttribute({ bad: { stringValue: '{"a":}' } }, 'bad'), undefined);
    assertEqual(getJsonAttribute({ bad: { stringValue: 'not json at all' } }, 'bad'), undefined);
  });

  await test('returns raw objects directly', () => {
    const obj = { a: 1, b: 2 };
    assertEqual(getJsonAttribute({ payload: obj }, 'payload'), obj);
  });

  await test('returns raw arrays directly', () => {
    const arr = [1, 2, 3];
    assertEqual(getJsonAttribute({ payload: arr }, 'payload'), arr);
  });

  await test('returns undefined for number values (not string)', () => {
    assertEqual(getJsonAttribute({ num: 42 }, 'num'), undefined);
  });

  await test('returns undefined for boolean values', () => {
    assertEqual(getJsonAttribute({ bool: true }, 'bool'), undefined);
  });

  await test('uses default max bytes when not specified', () => {
    // Default is 2MB - this should pass
    const mediumJson = `{"data":"${'x'.repeat(1000)}"}`;
    const result = getJsonAttribute({ payload: { stringValue: mediumJson } }, 'payload');
    assertTrue(result !== undefined, 'Should parse medium JSON with default limit');
  });

  await test('respects custom maxBytes option', () => {
    const json = '{"a":"test"}';
    assertEqual(getJsonAttribute({ p: { stringValue: json } }, 'p', { maxBytes: 5 }), undefined);
    assertEqual(getJsonAttribute({ p: { stringValue: json } }, 'p', { maxBytes: 50 }), { a: 'test' });
  });

  // ============================================================
  // getNumberAttribute tests
  // ============================================================
  console.log('\n--- getNumberAttribute ---');

  await test('returns direct numbers', () => {
    assertEqual(getNumberAttribute({ count: 42 }, 'count'), 42);
    assertEqual(getNumberAttribute({ rate: 3.14 }, 'rate'), 3.14);
  });

  await test('handles intValue wrapper', () => {
    assertEqual(getNumberAttribute({ count: { intValue: 42 } }, 'count'), 42);
    assertEqual(getNumberAttribute({ count: { intValue: '123' } }, 'count'), 123);
  });

  await test('handles floatValue wrapper', () => {
    assertEqual(getNumberAttribute({ rate: { floatValue: 3.14 } }, 'rate'), 3.14);
    assertEqual(getNumberAttribute({ rate: { floatValue: '2.5' } }, 'rate'), 2.5);
  });

  await test('handles doubleValue wrapper', () => {
    assertEqual(getNumberAttribute({ value: { doubleValue: 1.5e10 } }, 'value'), 1.5e10);
  });

  await test('uses strict parsing (rejects junk)', () => {
    // parseFloat("123abc") returns 123, but strict parsing rejects it
    assertEqual(getNumberAttribute({ bad: '123abc' }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: 'abc123' }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: '12.34.56' }, 'bad'), undefined);
  });

  await test('rejects NaN/Infinity', () => {
    assertEqual(getNumberAttribute({ bad: NaN }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: Infinity }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: -Infinity }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: 'NaN' }, 'bad'), undefined);
    assertEqual(getNumberAttribute({ bad: 'Infinity' }, 'bad'), undefined);
  });

  await test('parses valid numeric strings', () => {
    assertEqual(getNumberAttribute({ val: '42' }, 'val'), 42);
    assertEqual(getNumberAttribute({ val: '-5.5' }, 'val'), -5.5);
    assertEqual(getNumberAttribute({ val: '1e10' }, 'val'), 1e10);
    assertEqual(getNumberAttribute({ val: '.5' }, 'val'), 0.5);
  });

  await test('returns undefined for missing keys', () => {
    assertEqual(getNumberAttribute({}, 'missing'), undefined);
  });

  await test('returns undefined for null values', () => {
    assertEqual(getNumberAttribute({ key: null }, 'key'), undefined);
  });

  await test('returns undefined for undefined values', () => {
    assertEqual(getNumberAttribute({ key: undefined }, 'key'), undefined);
  });

  await test('handles zero correctly', () => {
    assertEqual(getNumberAttribute({ val: 0 }, 'val'), 0);
    assertEqual(getNumberAttribute({ val: '0' }, 'val'), 0);
    assertEqual(getNumberAttribute({ val: { intValue: 0 } }, 'val'), 0);
  });

  await test('handles negative zero', () => {
    const result = getNumberAttribute({ val: -0 }, 'val');
    assertEqual(result, 0); // -0 equals 0 in JS
    assertEqual(Object.is(result, -0), true);
  });

  await test('handles very large numbers', () => {
    assertEqual(getNumberAttribute({ val: Number.MAX_SAFE_INTEGER }, 'val'), Number.MAX_SAFE_INTEGER);
    assertEqual(getNumberAttribute({ val: Number.MIN_SAFE_INTEGER }, 'val'), Number.MIN_SAFE_INTEGER);
  });

  await test('handles exponential notation', () => {
    assertEqual(getNumberAttribute({ val: '1e5' }, 'val'), 100000);
    assertEqual(getNumberAttribute({ val: '1E5' }, 'val'), 100000);
    assertEqual(getNumberAttribute({ val: '1.5e-3' }, 'val'), 0.0015);
    assertEqual(getNumberAttribute({ val: '-2.5E+4' }, 'val'), -25000);
  });

  await test('handles strings with whitespace', () => {
    assertEqual(getNumberAttribute({ val: '  42  ' }, 'val'), 42);
    assertEqual(getNumberAttribute({ val: '\t3.14\n' }, 'val'), 3.14);
  });

  await test('rejects empty strings', () => {
    assertEqual(getNumberAttribute({ val: '' }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: '   ' }, 'val'), undefined);
  });

  await test('rejects boolean values', () => {
    assertEqual(getNumberAttribute({ val: true }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: false }, 'val'), undefined);
  });

  await test('rejects objects without number wrappers', () => {
    assertEqual(getNumberAttribute({ val: { someKey: 42 } }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: [1, 2, 3] }, 'val'), undefined);
  });

  await test('handles NaN/Infinity in wrapper values', () => {
    assertEqual(getNumberAttribute({ val: { intValue: NaN } }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: { floatValue: Infinity } }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: { doubleValue: -Infinity } }, 'val'), undefined);
  });

  await test('rejects hex strings', () => {
    assertEqual(getNumberAttribute({ val: '0x10' }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: '0xFF' }, 'val'), undefined);
  });

  await test('rejects octal strings', () => {
    assertEqual(getNumberAttribute({ val: '0o10' }, 'val'), undefined);
    assertEqual(getNumberAttribute({ val: '010' }, 'val'), 10); // Not interpreted as octal
  });

  await test('rejects binary strings', () => {
    assertEqual(getNumberAttribute({ val: '0b10' }, 'val'), undefined);
  });

  // ============================================================
  // utf8ByteLengthCapped tests
  // ============================================================
  console.log('\n--- utf8ByteLengthCapped ---');

  await test('counts ASCII as 1 byte each', () => {
    assertEqual(utf8ByteLengthCapped('abc', 100), 3);
    assertEqual(utf8ByteLengthCapped('Hello, World!', 100), 13);
  });

  await test('counts empty string as 0 bytes', () => {
    assertEqual(utf8ByteLengthCapped('', 100), 0);
  });

  await test('counts 2-byte characters (Latin extended, Greek)', () => {
    // 'é' (U+00E9) = 2 bytes, 'ñ' (U+00F1) = 2 bytes
    assertEqual(utf8ByteLengthCapped('é', 100), 2);
    assertEqual(utf8ByteLengthCapped('éñ', 100), 4);
    // Greek 'α' (U+03B1) = 2 bytes
    assertEqual(utf8ByteLengthCapped('α', 100), 2);
  });

  await test('counts 3-byte characters (Chinese, Japanese, Korean)', () => {
    // Chinese '中' (U+4E2D) = 3 bytes
    assertEqual(utf8ByteLengthCapped('中', 100), 3);
    // Japanese 'あ' (U+3042) = 3 bytes
    assertEqual(utf8ByteLengthCapped('あ', 100), 3);
    assertEqual(utf8ByteLengthCapped('中文', 100), 6);
  });

  await test('counts 4-byte characters (emoji with surrogate pairs)', () => {
    // '😀' (U+1F600) = 4 bytes (represented as surrogate pair in JS)
    assertEqual(utf8ByteLengthCapped('😀', 100), 4);
    // '🎉' (U+1F389) = 4 bytes
    assertEqual(utf8ByteLengthCapped('🎉', 100), 4);
    assertEqual(utf8ByteLengthCapped('😀🎉', 100), 8);
  });

  await test('handles mixed content correctly', () => {
    // 'Hi😀' = 'H'(1) + 'i'(1) + '😀'(4) = 6 bytes
    assertEqual(utf8ByteLengthCapped('Hi😀', 100), 6);
    // 'café' = 'c'(1) + 'a'(1) + 'f'(1) + 'é'(2) = 5 bytes
    assertEqual(utf8ByteLengthCapped('café', 100), 5);
  });

  await test('handles orphaned high surrogate as 3-byte BMP char', () => {
    // Create orphaned high surrogate (not followed by valid low surrogate)
    const orphanedHigh = String.fromCharCode(0xD800) + 'a';
    // Orphaned surrogate counts as 3 bytes, 'a' as 1
    assertEqual(utf8ByteLengthCapped(orphanedHigh, 100), 4);
  });

  await test('handles orphaned low surrogate as 3-byte BMP char', () => {
    // Create orphaned low surrogate (not preceded by valid high surrogate)
    const orphanedLow = 'a' + String.fromCharCode(0xDC00);
    // 'a' as 1, orphaned surrogate counts as 3 bytes
    assertEqual(utf8ByteLengthCapped(orphanedLow, 100), 4);
  });

  await test('short-circuits when maxBytes is exceeded', () => {
    // Long string but with low maxBytes - should stop early
    const longStr = 'a'.repeat(1000);
    const result = utf8ByteLengthCapped(longStr, 10);
    // Should return a value > 10 (indicating limit exceeded)
    assertTrue(result > 10, `Expected > 10, got ${result}`);
  });

  await test('returns exact count when under limit', () => {
    assertEqual(utf8ByteLengthCapped('abc', 10), 3);
    assertEqual(utf8ByteLengthCapped('中文', 10), 6);
  });

  await test('handles exactly at boundary', () => {
    assertEqual(utf8ByteLengthCapped('aaaa', 4), 4); // Exactly at limit
    assertEqual(utf8ByteLengthCapped('aaaaa', 4), 5); // Just over limit
  });

  await test('handles high surrogate at end of string', () => {
    // High surrogate at end with no following low surrogate
    const str = 'abc' + String.fromCharCode(0xD800);
    // 'a'(1) + 'b'(1) + 'c'(1) + orphaned(3) = 6
    assertEqual(utf8ByteLengthCapped(str, 100), 6);
  });

  await test('handles consecutive surrogates forming multiple emoji', () => {
    // Two emoji back to back
    const str = '😀😀';
    assertEqual(utf8ByteLengthCapped(str, 100), 8);
  });

  await test('counts currency symbols correctly', () => {
    // € (U+20AC) = 3 bytes
    assertEqual(utf8ByteLengthCapped('€', 100), 3);
    // ¥ (U+00A5) = 2 bytes
    assertEqual(utf8ByteLengthCapped('¥', 100), 2);
    // $ (U+0024) = 1 byte
    assertEqual(utf8ByteLengthCapped('$', 100), 1);
  });

  await test('handles mathematical symbols', () => {
    // π (U+03C0) = 2 bytes
    assertEqual(utf8ByteLengthCapped('π', 100), 2);
    // ∞ (U+221E) = 3 bytes
    assertEqual(utf8ByteLengthCapped('∞', 100), 3);
  });

  await test('maxBytes of 0 returns immediately', () => {
    const result = utf8ByteLengthCapped('test', 0);
    assertTrue(result > 0, 'Should return count > 0 indicating limit exceeded');
  });

  // ============================================================
  // boundAttributes tests
  // ============================================================
  console.log('\n--- boundAttributes ---');

  await test('passes through small attributes unchanged', () => {
    const input = { a: 1, b: 'hello', c: true };
    const result = boundAttributes(input);
    assertEqual(result, { a: 1, b: 'hello', c: true });
  });

  await test('skips null and undefined values', () => {
    const input = { a: 1, b: null, c: undefined, d: 'ok' };
    const result = boundAttributes(input);
    assertEqual(result, { a: 1, d: 'ok' });
  });

  await test('truncates long strings', () => {
    const longValue = 'x'.repeat(2000);
    const input = { long: longValue };
    const result = boundAttributes(input);
    const resultStr = result.long as string;
    // Should be truncated to 1000 chars + " ... [truncated]"
    assertTrue(resultStr.endsWith('... [truncated]'), 'Expected truncation marker');
    assertTrue(resultStr.length < 1100, `Expected truncated length < 1100, got ${resultStr.length}`);
  });

  await test('truncates large objects with preview', () => {
    // Create an object larger than MAX_ATTRIBUTE_STRING_LENGTH (1000 chars)
    const largeObj: Record<string, string> = {};
    for (let i = 0; i < 50; i++) {
      largeObj[`key${i}`] = 'x'.repeat(100);
    }
    const input = { big: largeObj };
    const result = boundAttributes(input);
    const resultObj = result.big as { _truncated?: boolean; _preview?: object };
    assertEqual(resultObj._truncated, true);
    assertTrue(resultObj._preview !== undefined, 'Expected _preview field');
  });

  await test('truncates circular objects to prevent UI crashes', () => {
    const cyclic: Record<string, unknown> = { a: 1 };
    cyclic.self = cyclic;

    const result = boundAttributes({ cyclic });
    const bounded = result.cyclic as { _truncated?: boolean; _preview?: object };
    assertEqual(bounded._truncated, true);
    assertTrue(bounded._preview !== undefined, 'Expected _preview field');
  });

  await test('drops attributes after total size cap is reached', () => {
    const input: Record<string, unknown> = {};
    for (let i = 0; i < 50; i++) {
      input[`k${i}`] = 'x'.repeat(2000);
    }

    const result = boundAttributes(input);
    const keys = Object.keys(result);
    assertTrue(keys.length < 50, `Expected some attributes dropped, got ${keys.length}`);
    assertTrue('k0' in result, 'Expected early keys to be retained');
    assertTrue(!('k49' in result), 'Expected late keys to be dropped after size cap');
  });

  await test('handles nested objects', () => {
    const input = { nested: { a: 1, b: 'hello' } };
    const result = boundAttributes(input);
    assertEqual(result, { nested: { a: 1, b: 'hello' } });
  });

  await test('handles arrays in attributes', () => {
    const input = { arr: [1, 2, 3] };
    const result = boundAttributes(input);
    assertEqual(result, { arr: [1, 2, 3] });
  });

  await test('preserves boolean and number values', () => {
    const input = { bool: false, num: 42, float: 3.14 };
    const result = boundAttributes(input);
    assertEqual(result, { bool: false, num: 42, float: 3.14 });
  });

  await test('handles empty object', () => {
    assertEqual(boundAttributes({}), {});
  });

  await test('handles deeply nested small objects', () => {
    const input = { a: { b: { c: { d: { e: 1 } } } } };
    const result = boundAttributes(input);
    assertEqual(result, { a: { b: { c: { d: { e: 1 } } } } });
  });

  await test('preview contains at most 3 keys', () => {
    const largeObj: Record<string, string> = {};
    for (let i = 0; i < 10; i++) {
      largeObj[`key${i}`] = 'x'.repeat(200);
    }
    const result = boundAttributes({ big: largeObj });
    const bounded = result.big as { _preview?: Record<string, unknown> };
    if (bounded._preview) {
      const previewKeys = Object.keys(bounded._preview);
      assertTrue(previewKeys.length <= 3, `Expected <= 3 preview keys, got ${previewKeys.length}`);
    }
  });

  await test('preview truncates long string values to 50 chars', () => {
    const largeObj = { longKey: 'x'.repeat(200), otherLong: 'y'.repeat(5000) };
    const result = boundAttributes({ big: largeObj });
    const bounded = result.big as { _preview?: Record<string, string> };
    if (bounded._preview && bounded._preview.longKey) {
      assertTrue(bounded._preview.longKey.length <= 50, 'Preview should truncate long strings');
    }
  });

  await test('preview shows [object] for nested objects', () => {
    const largeObj: Record<string, unknown> = {};
    for (let i = 0; i < 50; i++) {
      largeObj[`key${i}`] = { nested: 'x'.repeat(100) };
    }
    const result = boundAttributes({ big: largeObj });
    const bounded = result.big as { _preview?: Record<string, string> };
    if (bounded._preview) {
      const firstKey = Object.keys(bounded._preview)[0];
      assertEqual(bounded._preview[firstKey], '[object]');
    }
  });

  await test('handles string exactly at truncation boundary', () => {
    const str1000 = 'x'.repeat(1000);
    const str1001 = 'x'.repeat(1001);
    const result1000 = boundAttributes({ val: str1000 });
    const result1001 = boundAttributes({ val: str1001 });

    assertEqual(result1000.val, str1000); // Exactly at limit, no truncation
    assertTrue((result1001.val as string).endsWith('... [truncated]'), 'Should truncate 1001 char string');
  });

  await test('handles multiple large attributes - some dropped', () => {
    const input = {
      first: 'a'.repeat(5000),
      second: 'b'.repeat(5000),
      third: 'c'.repeat(5000),
    };
    const result = boundAttributes(input);
    assertTrue('first' in result, 'First attribute should be retained');
    // Due to size cap of 10KB, not all will fit
    const keys = Object.keys(result);
    assertTrue(keys.length >= 1, 'At least first attribute should be retained');
  });

  await test('preserves empty strings', () => {
    const result = boundAttributes({ empty: '' });
    assertEqual(result, { empty: '' });
  });

  await test('preserves zero values', () => {
    const result = boundAttributes({ zero: 0 });
    assertEqual(result, { zero: 0 });
  });

  await test('preserves false boolean', () => {
    const result = boundAttributes({ flag: false });
    assertEqual(result, { flag: false });
  });

  await test('handles mixed null and valid values', () => {
    const input = { a: null, b: 1, c: undefined, d: 'ok', e: null };
    const result = boundAttributes(input);
    assertEqual(result, { b: 1, d: 'ok' });
  });

  await test('handles object with only null values', () => {
    const result = boundAttributes({ a: null, b: undefined });
    assertEqual(result, {});
  });

  await test('handles arrays with mixed types', () => {
    const input = { arr: [1, 'two', { three: 3 }, null] };
    const result = boundAttributes(input);
    assertEqual(result, { arr: [1, 'two', { three: 3 }, null] });
  });

  await test('estimates size includes _estimatedSize field', () => {
    const largeObj: Record<string, string> = {};
    for (let i = 0; i < 50; i++) {
      largeObj[`key${i}`] = 'x'.repeat(100);
    }
    const result = boundAttributes({ big: largeObj });
    const bounded = result.big as { _estimatedSize?: number };
    assertTrue(bounded._estimatedSize !== undefined, 'Expected _estimatedSize field');
    assertTrue(typeof bounded._estimatedSize === 'number', '_estimatedSize should be a number');
  });

  // Summary
  console.log('\n--------------------------');
  console.log(`Tests: ${passed} passed, ${failed} failed`);
  console.log('--------------------------\n');

  if (failed > 0) {
    process.exit(1);
  }
}

void run();
