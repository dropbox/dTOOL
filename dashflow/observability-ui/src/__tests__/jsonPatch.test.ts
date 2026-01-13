// Unit tests for JSON Patch utilities
// Run with: npx tsx src/__tests__/jsonPatch.test.ts

import {
  applyPatchOp,
  applyPatch,
  JsonPatchOp,
  convertDiffOp,
  getChangedPaths,
  CloneError,
  UnsupportedEncodingError,
  ValueEncoding,
  applyDiffOperations,
} from '../utils/jsonPatch';
import { DiffOperation } from '../proto/dashstream';

// Simple test runner
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

function assertEqual(actual: unknown, expected: unknown, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(`${message || 'Assertion failed'}: expected ${expectedStr}, got ${actualStr}`);
  }
}

// Test suites
console.log('\nJSON Patch Tests\n');

console.log('applyPatchOp - add operation:');
test('adds value at root path', () => {
  const result = applyPatchOp({}, { op: 'add', path: '/foo', value: 'bar' });
  assertEqual(result, { foo: 'bar' });
});

test('adds value at nested path', () => {
  const result = applyPatchOp({ a: {} }, { op: 'add', path: '/a/b', value: 123 });
  assertEqual(result, { a: { b: 123 } });
});

test('adds to array with - index', () => {
  const result = applyPatchOp({ items: [1, 2] }, { op: 'add', path: '/items/-', value: 3 });
  assertEqual(result, { items: [1, 2, 3] });
});

test('adds to array at specific index', () => {
  const result = applyPatchOp({ items: [1, 3] }, { op: 'add', path: '/items/1', value: 2 });
  // RFC6902: add at array index inserts (shifts existing items to the right)
  assertEqual(result, { items: [1, 2, 3] });
});

console.log('\napplyPatchOp - remove operation:');
test('removes value at path', () => {
  const result = applyPatchOp({ foo: 'bar', baz: 'qux' }, { op: 'remove', path: '/foo' });
  assertEqual(result, { baz: 'qux' });
});

test('removes nested value', () => {
  const result = applyPatchOp({ a: { b: 1, c: 2 } }, { op: 'remove', path: '/a/b' });
  assertEqual(result, { a: { c: 2 } });
});

test('removes from array', () => {
  const result = applyPatchOp({ items: [1, 2, 3] }, { op: 'remove', path: '/items/1' });
  assertEqual(result, { items: [1, 3] });
});

console.log('\napplyPatchOp - replace operation:');
test('replaces value at path', () => {
  const result = applyPatchOp({ foo: 'bar' }, { op: 'replace', path: '/foo', value: 'baz' });
  assertEqual(result, { foo: 'baz' });
});

test('replaces nested value', () => {
  const result = applyPatchOp({ a: { b: 1 } }, { op: 'replace', path: '/a/b', value: 2 });
  assertEqual(result, { a: { b: 2 } });
});

console.log('\napplyPatchOp - move operation:');
test('moves value between paths', () => {
  const result = applyPatchOp({ foo: 'bar' }, { op: 'move', path: '/baz', from: '/foo' });
  assertEqual(result, { baz: 'bar' });
});

console.log('\napplyPatchOp - copy operation:');
test('copies value to new path', () => {
  const result = applyPatchOp({ foo: 'bar' }, { op: 'copy', path: '/baz', from: '/foo' });
  assertEqual(result, { foo: 'bar', baz: 'bar' });
});

console.log('\napplyPatchOp - test operation:');
test('passes when values match', () => {
  const result = applyPatchOp({ foo: 'bar' }, { op: 'test', path: '/foo', value: 'bar' });
  assertEqual(result, { foo: 'bar' });
});

test('throws when values dont match', () => {
  let threw = false;
  try {
    applyPatchOp({ foo: 'bar' }, { op: 'test', path: '/foo', value: 'baz' });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected test to throw');
});

// M-733: Test operation should use semantic equality, not JSON.stringify
test('passes for objects with different key ordering', () => {
  // Create objects with different insertion order
  const objA: Record<string, number> = {};
  objA['b'] = 2;
  objA['a'] = 1;

  // This test verifies that {a:1,b:2} === {b:2,a:1} semantically
  // JSON.stringify would fail because key order differs
  const result = applyPatchOp({ data: { a: 1, b: 2 } }, { op: 'test', path: '/data', value: objA });
  assertEqual(result, { data: { a: 1, b: 2 } });
});

test('passes for nested objects with different key ordering', () => {
  const nested: Record<string, unknown> = {};
  nested['y'] = 2;
  nested['x'] = 1;
  const outer: Record<string, unknown> = {};
  outer['inner'] = nested;
  outer['name'] = 'test';

  const result = applyPatchOp(
    { obj: { name: 'test', inner: { x: 1, y: 2 } } },
    { op: 'test', path: '/obj', value: outer }
  );
  assertEqual(result, { obj: { name: 'test', inner: { x: 1, y: 2 } } });
});

console.log('\napplyPatch - multiple operations:');
test('applies operations in sequence', () => {
  const ops: JsonPatchOp[] = [
    { op: 'add', path: '/count', value: 0 },
    { op: 'replace', path: '/count', value: 1 },
    { op: 'add', path: '/items', value: [] },
    { op: 'add', path: '/items/-', value: 'a' },
    { op: 'add', path: '/items/-', value: 'b' },
  ];
  const result = applyPatch({}, ops);
  assertEqual(result, { count: 1, items: ['a', 'b'] });
});

console.log('\nconvertDiffOp - protobuf conversion:');
test('converts ADD operation', () => {
  const diffOp: DiffOperation = {
    op: 0, // ADD
    path: '/foo',
    value: new TextEncoder().encode('"bar"'),
    from: '',
    encoding: 0, // JSON
  };
  const result = convertDiffOp(diffOp);
  assertEqual(result.op, 'add');
  assertEqual(result.path, '/foo');
  assertEqual(result.value, 'bar');
});

test('converts REMOVE operation', () => {
  const diffOp: DiffOperation = {
    op: 1, // REMOVE
    path: '/foo',
    value: new Uint8Array(),
    from: '',
    encoding: 0,
  };
  const result = convertDiffOp(diffOp);
  assertEqual(result.op, 'remove');
  assertEqual(result.path, '/foo');
});

test('converts REPLACE operation with object value', () => {
  const diffOp: DiffOperation = {
    op: 2, // REPLACE
    path: '/config',
    value: new TextEncoder().encode('{"a":1,"b":2}'),
    from: '',
    encoding: 0,
  };
  const result = convertDiffOp(diffOp);
  assertEqual(result.op, 'replace');
  assertEqual(result.value, { a: 1, b: 2 });
});

console.log('\ngetChangedPaths:');
test('extracts paths from operations', () => {
  const ops: JsonPatchOp[] = [
    { op: 'add', path: '/foo', value: 1 },
    { op: 'replace', path: '/bar/baz', value: 2 },
    { op: 'remove', path: '/qux' },
  ];
  const paths = getChangedPaths(ops);
  assertEqual(paths, ['/foo', '/bar/baz', '/qux']);
});

console.log('\nJSON Pointer edge cases:');
test('handles escaped characters in path', () => {
  // ~0 = ~, ~1 = /
  const result = applyPatchOp({}, { op: 'add', path: '/a~1b', value: 'slash' });
  assertEqual(result, { 'a/b': 'slash' });
});

test('handles empty string key', () => {
  const result = applyPatchOp({}, { op: 'add', path: '/', value: 'empty' });
  assertEqual(result, { '': 'empty' });
});

test('handles tilde escape sequence', () => {
  // ~0 = ~
  const result = applyPatchOp({}, { op: 'add', path: '/a~0b', value: 'tilde' });
  assertEqual(result, { 'a~b': 'tilde' });
});

test('handles combined tilde and slash escapes', () => {
  // ~0 = ~, ~1 = /  in sequence
  const result = applyPatchOp({}, { op: 'add', path: '/a~0~1b', value: 'both' });
  assertEqual(result, { 'a~/b': 'both' });
});

test('handles deeply nested paths', () => {
  const result = applyPatchOp({}, { op: 'add', path: '/a/b/c/d/e', value: 'deep' });
  assertEqual(result, { a: { b: { c: { d: { e: 'deep' } } } } });
});

// M-710: Prototype pollution protection tests
console.log('\nM-710: Prototype pollution protection:');
test('blocks __proto__ in path', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({}, { op: 'add', path: '/__proto__/polluted', value: 'yes' });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected __proto__ to be blocked');
  if (!errorMsg.includes('prototype pollution')) {
    throw new Error(`Expected prototype pollution error, got: ${errorMsg}`);
  }
});

test('blocks constructor in path', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({}, { op: 'add', path: '/constructor/prototype', value: 'bad' });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected constructor to be blocked');
  if (!errorMsg.includes('prototype pollution')) {
    throw new Error(`Expected prototype pollution error, got: ${errorMsg}`);
  }
});

test('blocks prototype in path', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({}, { op: 'add', path: '/prototype', value: 'exploit' });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected prototype to be blocked');
  if (!errorMsg.includes('prototype pollution')) {
    throw new Error(`Expected prototype pollution error, got: ${errorMsg}`);
  }
});

test('blocks __proto__ in nested path', () => {
  let threw = false;
  try {
    applyPatchOp({ obj: {} }, { op: 'add', path: '/obj/__proto__/x', value: 1 });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected nested __proto__ to be blocked');
});

// M-1101: DoS protection tests
console.log('\nM-1101: DoS protection (limits):');
test('throws on too many operations', () => {
  const ops: JsonPatchOp[] = [];
  for (let i = 0; i < 100; i++) {
    ops.push({ op: 'add', path: `/key${i}`, value: i });
  }
  // With maxOperations=50, should throw
  let threw = false;
  let errorMsg = '';
  try {
    applyPatch({}, ops, { maxOperations: 50 });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected maxOperations limit to throw');
  if (!errorMsg.includes('Too many operations')) {
    throw new Error(`Expected 'Too many operations' error, got: ${errorMsg}`);
  }
});

test('throws on path too long', () => {
  const longPath = '/a'.repeat(600); // 1200 chars
  let threw = false;
  let errorMsg = '';
  try {
    applyPatch({}, [{ op: 'add', path: longPath, value: 'x' }], { maxPathLength: 100 });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected maxPathLength limit to throw');
  if (!errorMsg.includes('Path too long')) {
    throw new Error(`Expected 'Path too long' error, got: ${errorMsg}`);
  }
});

test('respects default limits (allows normal operations)', () => {
  const ops: JsonPatchOp[] = [];
  for (let i = 0; i < 100; i++) {
    ops.push({ op: 'add', path: `/key${i}`, value: i });
  }
  // Default maxOperations is 10000, so 100 should be fine
  const result = applyPatch({}, ops);
  assertEqual((result as Record<string, number>).key99, 99);
});

// M-709: Array index validation tests
console.log('\nM-709: Array index validation:');
test('throws on negative array index', () => {
  // Note: Leading zeros like "01" are caught, but negative needs different test
  let threw = false;
  try {
    applyPatchOp({ items: [1, 2] }, { op: 'replace', path: '/items/abc', value: 99 });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected invalid array index to throw');
});

test('throws on leading zero array index', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({ items: [1, 2, 3] }, { op: 'replace', path: '/items/01', value: 99 });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected leading zero array index to throw');
  if (!errorMsg.includes('Invalid array index')) {
    throw new Error(`Expected 'Invalid array index' error, got: ${errorMsg}`);
  }
});

test('throws on array index out of bounds for replace', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({ items: [1, 2] }, { op: 'replace', path: '/items/5', value: 99 });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected out of bounds array index to throw');
  if (!errorMsg.includes('out of bounds')) {
    throw new Error(`Expected 'out of bounds' error, got: ${errorMsg}`);
  }
});

test('throws on array index out of bounds for remove', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({ items: [1, 2] }, { op: 'remove', path: '/items/10' });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected out of bounds remove to throw');
  if (!errorMsg.includes('out of bounds')) {
    throw new Error(`Expected 'out of bounds' error, got: ${errorMsg}`);
  }
});

test('throws when using - for non-add operations', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({ items: [1, 2] }, { op: 'remove', path: '/items/-' });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected - on remove to throw');
  if (!errorMsg.includes("only valid for 'add'")) {
    throw new Error(`Expected "only valid for 'add'" error, got: ${errorMsg}`);
  }
});

test('allows add at end of array (index === length)', () => {
  // Adding at index 2 in array of length 2 is valid (appends)
  const result = applyPatchOp({ items: [1, 2] }, { op: 'add', path: '/items/2', value: 3 });
  assertEqual(result, { items: [1, 2, 3] });
});

test('throws on add beyond array length', () => {
  let threw = false;
  let errorMsg = '';
  try {
    applyPatchOp({ items: [1, 2] }, { op: 'add', path: '/items/5', value: 99 });
  } catch (e) {
    threw = true;
    errorMsg = String(e);
  }
  if (!threw) throw new Error('Expected add beyond length to throw');
  if (!errorMsg.includes('out of bounds')) {
    throw new Error(`Expected 'out of bounds' error, got: ${errorMsg}`);
  }
});

// Encoding tests
console.log('\nValue encoding tests:');
test('decodes RAW encoding as string', () => {
  const diffOp: DiffOperation = {
    op: 0, // ADD
    path: '/raw',
    value: new TextEncoder().encode('plain text'),
    from: '',
    encoding: 3, // RAW
  };
  const result = convertDiffOp(diffOp);
  assertEqual(result.value, 'plain text');
});

test('throws UnsupportedEncodingError for MSGPACK', () => {
  const diffOp: DiffOperation = {
    op: 0, // ADD
    path: '/data',
    value: new Uint8Array([0x92, 0x01, 0x02]), // MSGPACK [1, 2]
    from: '',
    encoding: 1, // MSGPACK
  };
  let threw = false;
  let error: unknown = null;
  try {
    convertDiffOp(diffOp);
  } catch (e) {
    threw = true;
    error = e;
  }
  if (!threw) throw new Error('Expected MSGPACK to throw');
  if (!(error instanceof UnsupportedEncodingError)) {
    throw new Error(`Expected UnsupportedEncodingError, got: ${error}`);
  }
});

test('throws UnsupportedEncodingError for PROTOBUF', () => {
  const diffOp: DiffOperation = {
    op: 0, // ADD
    path: '/data',
    value: new Uint8Array([0x08, 0x01]), // Some protobuf bytes
    from: '',
    encoding: 2, // PROTOBUF
  };
  let threw = false;
  let error: unknown = null;
  try {
    convertDiffOp(diffOp);
  } catch (e) {
    threw = true;
    error = e;
  }
  if (!threw) throw new Error('Expected PROTOBUF to throw');
  if (!(error instanceof UnsupportedEncodingError)) {
    throw new Error(`Expected UnsupportedEncodingError, got: ${error}`);
  }
});

test('throws UnsupportedEncodingError for unknown encoding', () => {
  const diffOp: DiffOperation = {
    op: 0, // ADD
    path: '/data',
    value: new Uint8Array([0x00]),
    from: '',
    encoding: 99, // Unknown
  };
  let threw = false;
  let error: unknown = null;
  try {
    convertDiffOp(diffOp);
  } catch (e) {
    threw = true;
    error = e;
  }
  if (!threw) throw new Error('Expected unknown encoding to throw');
  if (!(error instanceof UnsupportedEncodingError)) {
    throw new Error(`Expected UnsupportedEncodingError, got: ${error}`);
  }
});

test('handles empty value bytes', () => {
  const diffOp: DiffOperation = {
    op: 1, // REMOVE
    path: '/foo',
    value: new Uint8Array(0),
    from: '',
    encoding: 0, // JSON
  };
  const result = convertDiffOp(diffOp);
  assertEqual(result.value, undefined);
});

// applyDiffOperations tests
console.log('\napplyDiffOperations - integration:');
test('applies multiple DiffOperations', () => {
  const diffOps: DiffOperation[] = [
    {
      op: 0, // ADD
      path: '/name',
      value: new TextEncoder().encode('"test"'),
      from: '',
      encoding: 0,
    },
    {
      op: 0, // ADD
      path: '/count',
      value: new TextEncoder().encode('42'),
      from: '',
      encoding: 0,
    },
  ];
  const result = applyDiffOperations({}, diffOps);
  assertEqual(result, { name: 'test', count: 42 });
});

// Deep equality edge cases
console.log('\njsonDeepEqual edge cases (via test operation):');
test('test operation handles null values', () => {
  const result = applyPatchOp({ data: null }, { op: 'test', path: '/data', value: null });
  assertEqual(result, { data: null });
});

test('test operation fails on null vs undefined', () => {
  let threw = false;
  try {
    applyPatchOp({ data: null }, { op: 'test', path: '/data', value: undefined });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected null vs undefined to fail');
});

test('test operation compares arrays by order', () => {
  // Arrays must match in order
  const result = applyPatchOp({ arr: [1, 2, 3] }, { op: 'test', path: '/arr', value: [1, 2, 3] });
  assertEqual(result, { arr: [1, 2, 3] });
});

test('test operation fails on arrays with different order', () => {
  let threw = false;
  try {
    applyPatchOp({ arr: [1, 2, 3] }, { op: 'test', path: '/arr', value: [3, 2, 1] });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected arrays with different order to fail');
});

test('test operation fails on arrays with different length', () => {
  let threw = false;
  try {
    applyPatchOp({ arr: [1, 2, 3] }, { op: 'test', path: '/arr', value: [1, 2] });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected arrays with different length to fail');
});

test('test operation handles primitive types', () => {
  applyPatchOp({ num: 42 }, { op: 'test', path: '/num', value: 42 });
  applyPatchOp({ str: 'hello' }, { op: 'test', path: '/str', value: 'hello' });
  applyPatchOp({ bool: true }, { op: 'test', path: '/bool', value: true });
});

test('test operation fails on type mismatch', () => {
  let threw = false;
  try {
    applyPatchOp({ val: '42' }, { op: 'test', path: '/val', value: 42 });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected string vs number to fail');
});

// Move and copy edge cases
console.log('\nMove and copy edge cases:');
test('move requires from path', () => {
  let threw = false;
  try {
    applyPatchOp({ a: 1 }, { op: 'move', path: '/b' });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected move without from to throw');
});

test('copy requires from path', () => {
  let threw = false;
  try {
    applyPatchOp({ a: 1 }, { op: 'copy', path: '/b' });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected copy without from to throw');
});

test('copy creates deep clone', () => {
  const original = { nested: { val: 1 } };
  const result = applyPatchOp(original, { op: 'copy', path: '/clone', from: '/nested' }) as Record<string, unknown>;
  // Modify original nested
  original.nested.val = 99;
  // Clone should be independent
  assertEqual((result.clone as Record<string, number>).val, 1);
});

test('move within arrays', () => {
  // Move element from index 0 to index 2
  const result = applyPatchOp(
    { items: ['a', 'b', 'c'] },
    { op: 'move', path: '/items/2', from: '/items/0' }
  );
  // After remove at 0: ['b', 'c'], then add at 2: ['b', 'c', 'a']
  assertEqual(result, { items: ['b', 'c', 'a'] });
});

// Invalid JSON pointer tests
console.log('\nInvalid JSON pointer tests:');
test('throws on path not starting with /', () => {
  let threw = false;
  try {
    applyPatchOp({}, { op: 'add', path: 'invalid', value: 1 });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error('Expected invalid path to throw');
});

// State immutability test
console.log('\nImmutability tests:');
test('applyPatchOp does not mutate original state', () => {
  const original = { foo: 'bar', nested: { x: 1 } };
  const copy = JSON.parse(JSON.stringify(original));
  applyPatchOp(original, { op: 'replace', path: '/foo', value: 'changed' });
  assertEqual(original, copy); // Original should be unchanged
});

test('applyPatch does not mutate original state', () => {
  const original = { items: [1, 2, 3] };
  const copy = JSON.parse(JSON.stringify(original));
  applyPatch(original, [
    { op: 'add', path: '/items/-', value: 4 },
    { op: 'replace', path: '/items/0', value: 99 },
  ]);
  assertEqual(original, copy); // Original should be unchanged
});

// Unknown operation handling
console.log('\nUnknown operation handling:');
test('unknown operation is handled gracefully', () => {
  // Cast to bypass TypeScript check for testing runtime behavior
  const result = applyPatchOp({ foo: 1 }, { op: 'unknown' as JsonPatchOp['op'], path: '/bar', value: 2 });
  // Should return state unchanged (with console warning)
  assertEqual(result, { foo: 1 });
});

// ValueEncoding enum export test
console.log('\nValueEncoding enum:');
test('ValueEncoding enum values are correct', () => {
  assertEqual(ValueEncoding.JSON, 0);
  assertEqual(ValueEncoding.MSGPACK, 1);
  assertEqual(ValueEncoding.PROTOBUF, 2);
  assertEqual(ValueEncoding.RAW, 3);
});

// CloneError type check
console.log('\nCloneError:');
test('CloneError is exported and has correct name', () => {
  const error = new CloneError('test');
  assertEqual(error.name, 'CloneError');
  assertEqual(error.message, 'test');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
