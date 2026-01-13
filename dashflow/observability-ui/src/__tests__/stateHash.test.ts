// Unit tests for state hash canonicalization
// Run with: npx tsx src/__tests__/stateHash.test.ts
//
// M-2594: Expanded test coverage for edge cases:
// - Empty objects/arrays
// - null/undefined handling
// - Non-finite numbers (NaN, Infinity)
// - BigInt serialization
// - Function/symbol properties (should be omitted)
// - Deeply nested structures
// - Concurrent hash computations (M-775 race condition fix)
// - Negative unsafe numbers
// - MAX_SAFE_INTEGER boundary values
//
// M-2610: Added 26 additional edge case and hash uniqueness tests (45 -> 71 tests)

import { canonicalJsonString, computeStateHash, computeStateHashLegacy } from '../utils/stateHash';

// Simple test runner (async-friendly)
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
    throw new Error(message || 'Expected true but got false');
  }
}

function assertFalse(condition: boolean, message?: string): void {
  if (condition) {
    throw new Error(message || 'Expected false but got true');
  }
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

async function run(): Promise<void> {
  console.log('\nState Hash Tests\n');

  // ==========================================================================
  // canonicalJsonString - stable ordering
  // ==========================================================================
  console.log('canonicalJsonString - stable ordering:');

  await test('sorts object keys recursively', () => {
    const state = { b: 2, a: 1, nested: { z: 'x', y: [true, null] } };
    const canonical = canonicalJsonString(state);
    assertEqual(canonical, '{"a":1,"b":2,"nested":{"y":[true,null],"z":"x"}}');
  });

  await test('produces same output regardless of key insertion order', () => {
    const state1 = { alpha: 1, beta: 2, gamma: 3 };
    const state2 = { gamma: 3, alpha: 1, beta: 2 };
    const state3 = { beta: 2, gamma: 3, alpha: 1 };
    const c1 = canonicalJsonString(state1);
    const c2 = canonicalJsonString(state2);
    const c3 = canonicalJsonString(state3);
    assertEqual(c1, c2, 'state1 vs state2');
    assertEqual(c2, c3, 'state2 vs state3');
    assertEqual(c1, '{"alpha":1,"beta":2,"gamma":3}');
  });

  // ==========================================================================
  // canonicalJsonString - primitives
  // ==========================================================================
  console.log('\ncanonicalJsonString - primitives:');

  await test('handles null', () => {
    assertEqual(canonicalJsonString(null), 'null');
  });

  await test('handles undefined as null', () => {
    assertEqual(canonicalJsonString(undefined), 'null');
  });

  await test('handles true', () => {
    assertEqual(canonicalJsonString(true), 'true');
  });

  await test('handles false', () => {
    assertEqual(canonicalJsonString(false), 'false');
  });

  await test('handles positive integers', () => {
    assertEqual(canonicalJsonString(42), '42');
  });

  await test('handles negative integers', () => {
    assertEqual(canonicalJsonString(-123), '-123');
  });

  await test('handles floating point numbers', () => {
    assertEqual(canonicalJsonString(3.14159), '3.14159');
  });

  await test('handles zero', () => {
    assertEqual(canonicalJsonString(0), '0');
  });

  await test('handles negative zero as zero', () => {
    // JSON.stringify treats -0 as 0
    assertEqual(canonicalJsonString(-0), '0');
  });

  await test('handles strings', () => {
    assertEqual(canonicalJsonString('hello world'), '"hello world"');
  });

  await test('escapes special characters in strings', () => {
    assertEqual(canonicalJsonString('line1\nline2'), '"line1\\nline2"');
    assertEqual(canonicalJsonString('tab\there'), '"tab\\there"');
    assertEqual(canonicalJsonString('quote"here'), '"quote\\"here"');
  });

  await test('handles empty string', () => {
    assertEqual(canonicalJsonString(''), '""');
  });

  await test('handles string with unicode characters', () => {
    assertEqual(canonicalJsonString('hello 世界'), '"hello 世界"');
  });

  await test('handles string with emoji', () => {
    assertEqual(canonicalJsonString('test 🚀'), '"test 🚀"');
  });

  // ==========================================================================
  // canonicalJsonString - non-finite numbers
  // ==========================================================================
  console.log('\ncanonicalJsonString - non-finite numbers:');

  await test('handles NaN as null', () => {
    assertEqual(canonicalJsonString(NaN), 'null');
  });

  await test('handles Infinity as null', () => {
    assertEqual(canonicalJsonString(Infinity), 'null');
  });

  await test('handles -Infinity as null', () => {
    assertEqual(canonicalJsonString(-Infinity), 'null');
  });

  // ==========================================================================
  // canonicalJsonString - BigInt (M-750)
  // ==========================================================================
  console.log('\ncanonicalJsonString - BigInt (M-750):');

  await test('serializes BigInt as quoted string', () => {
    assertEqual(canonicalJsonString(BigInt(12345)), '"12345"');
  });

  await test('handles large BigInt values', () => {
    const bigValue = BigInt('999999999999999999999999999999');
    assertEqual(canonicalJsonString(bigValue), '"999999999999999999999999999999"');
  });

  await test('handles negative BigInt', () => {
    assertEqual(canonicalJsonString(BigInt(-42)), '"-42"');
  });

  await test('handles BigInt zero', () => {
    assertEqual(canonicalJsonString(BigInt(0)), '"0"');
  });

  // ==========================================================================
  // canonicalJsonString - empty structures
  // ==========================================================================
  console.log('\ncanonicalJsonString - empty structures:');

  await test('handles empty object', () => {
    assertEqual(canonicalJsonString({}), '{}');
  });

  await test('handles empty array', () => {
    assertEqual(canonicalJsonString([]), '[]');
  });

  await test('handles object with only undefined values', () => {
    const obj = { a: undefined, b: undefined };
    assertEqual(canonicalJsonString(obj), '{}');
  });

  // ==========================================================================
  // canonicalJsonString - arrays
  // ==========================================================================
  console.log('\ncanonicalJsonString - arrays:');

  await test('handles simple arrays', () => {
    assertEqual(canonicalJsonString([1, 2, 3]), '[1,2,3]');
  });

  await test('handles mixed type arrays', () => {
    assertEqual(canonicalJsonString([1, 'two', true, null]), '[1,"two",true,null]');
  });

  await test('handles nested arrays', () => {
    assertEqual(canonicalJsonString([[1, 2], [3, 4]]), '[[1,2],[3,4]]');
  });

  await test('handles arrays with objects', () => {
    const arr = [{ b: 2, a: 1 }, { d: 4, c: 3 }];
    assertEqual(canonicalJsonString(arr), '[{"a":1,"b":2},{"c":3,"d":4}]');
  });

  await test('handles sparse arrays with undefined as null', () => {
    const arr = [1, undefined, 3];
    assertEqual(canonicalJsonString(arr), '[1,null,3]');
  });

  await test('handles arrays with null elements', () => {
    assertEqual(canonicalJsonString([null, null, null]), '[null,null,null]');
  });

  // ==========================================================================
  // canonicalJsonString - special properties
  // ==========================================================================
  console.log('\ncanonicalJsonString - special properties:');

  await test('omits undefined properties', () => {
    const obj = { a: 1, b: undefined, c: 3 };
    assertEqual(canonicalJsonString(obj), '{"a":1,"c":3}');
  });

  await test('omits function properties', () => {
    const obj = { a: 1, fn: () => {}, c: 3 };
    assertEqual(canonicalJsonString(obj), '{"a":1,"c":3}');
  });

  await test('omits symbol properties', () => {
    const sym = Symbol('test');
    const obj = { a: 1, [sym]: 'hidden', c: 3 };
    // Symbol keys won't appear in Object.keys()
    assertEqual(canonicalJsonString(obj), '{"a":1,"c":3}');
  });

  await test('handles symbol values by omitting property', () => {
    const obj = { a: 1, b: Symbol('test') };
    // Symbol values cause property to be omitted
    assertEqual(canonicalJsonString(obj), '{"a":1}');
  });

  await test('handles object with numeric string keys', () => {
    const obj = { '2': 'b', '1': 'a', '10': 'c' };
    // Numeric strings are sorted lexicographically
    assertEqual(canonicalJsonString(obj), '{"1":"a","10":"c","2":"b"}');
  });

  await test('handles object with special characters in keys', () => {
    const obj = { 'key with spaces': 1, 'key-with-dashes': 2 };
    assertEqual(canonicalJsonString(obj), '{"key with spaces":1,"key-with-dashes":2}');
  });

  // ==========================================================================
  // canonicalJsonString - deeply nested structures
  // ==========================================================================
  console.log('\ncanonicalJsonString - deeply nested structures:');

  await test('handles deeply nested objects', () => {
    const deep = { level1: { level2: { level3: { level4: { value: 'deep' } } } } };
    assertEqual(
      canonicalJsonString(deep),
      '{"level1":{"level2":{"level3":{"level4":{"value":"deep"}}}}}'
    );
  });

  await test('handles complex nested structure with mixed types', () => {
    const complex = {
      users: [
        { name: 'Alice', scores: [95, 87, 92] },
        { name: 'Bob', scores: [78, 85, 90] },
      ],
      metadata: { version: 1, active: true },
    };
    const expected =
      '{"metadata":{"active":true,"version":1},"users":[{"name":"Alice","scores":[95,87,92]},{"name":"Bob","scores":[78,85,90]}]}';
    assertEqual(canonicalJsonString(complex), expected);
  });

  await test('handles arrays inside nested objects', () => {
    const obj = { outer: { inner: { data: [1, 2, 3] } } };
    assertEqual(canonicalJsonString(obj), '{"outer":{"inner":{"data":[1,2,3]}}}');
  });

  await test('handles objects inside nested arrays', () => {
    const arr = [[[{ a: 1 }]]];
    assertEqual(canonicalJsonString(arr), '[[[{"a":1}]]]');
  });

  // ==========================================================================
  // computeStateHash - golden vector
  // ==========================================================================
  console.log('\ncomputeStateHash - golden vector:');

  await test('matches Rust canonical SHA-256', async () => {
    const state = { b: 2, a: 1, nested: { z: 'x', y: [true, null] } };
    const result = await computeStateHash(state);
    // M-741: computeStateHash now returns StateHashResult with hash and hasUnsafeNumbers
    assertEqual(
      toHex(result.hash),
      'f35279c8aa6b00bc82d43a191596cc3b41b7de7899ee16e36a08efe3afc45103'
    );
    assertEqual(result.hasUnsafeNumbers, false, 'small numbers should not be flagged as unsafe');
  });

  await test('produces consistent hash for same input', async () => {
    const state = { test: 'data', count: 42 };
    const result1 = await computeStateHash(state);
    const result2 = await computeStateHash(state);
    assertEqual(toHex(result1.hash), toHex(result2.hash), 'same input should produce same hash');
  });

  // ==========================================================================
  // computeStateHash - empty state
  // ==========================================================================
  console.log('\ncomputeStateHash - empty state:');

  await test('hashes empty object consistently', async () => {
    const result1 = await computeStateHash({});
    const result2 = await computeStateHash({});
    assertEqual(toHex(result1.hash), toHex(result2.hash), 'empty objects should hash identically');
    assertFalse(result1.hasUnsafeNumbers, 'empty object has no unsafe numbers');
  });

  await test('returns 32-byte hash for empty object', async () => {
    const result = await computeStateHash({});
    assertEqual(result.hash.length, 32, 'SHA-256 should be 32 bytes');
  });

  // ==========================================================================
  // computeStateHash - unsafe numbers (M-741)
  // ==========================================================================
  console.log('\ncomputeStateHash - unsafe numbers (M-741):');

  await test('detects unsafe numbers > MAX_SAFE_INTEGER', async () => {
    // M-741: Numbers larger than MAX_SAFE_INTEGER (2^53-1) lose precision in JSON.parse
    const state = { bigNumber: 9007199254740993 }; // MAX_SAFE_INTEGER + 2
    const result = await computeStateHash(state);
    assertEqual(result.hasUnsafeNumbers, true, 'large number should be flagged as unsafe');
  });

  await test('detects unsafe negative numbers < -MAX_SAFE_INTEGER', async () => {
    const state = { bigNegative: -9007199254740993 }; // -(MAX_SAFE_INTEGER + 2)
    const result = await computeStateHash(state);
    assertEqual(result.hasUnsafeNumbers, true, 'large negative number should be flagged as unsafe');
  });

  await test('MAX_SAFE_INTEGER itself is safe', async () => {
    const state = { exact: Number.MAX_SAFE_INTEGER }; // 2^53-1 = 9007199254740991
    const result = await computeStateHash(state);
    assertFalse(result.hasUnsafeNumbers, 'MAX_SAFE_INTEGER should be safe');
  });

  await test('MAX_SAFE_INTEGER + 1 is unsafe', async () => {
    const state = { onePast: Number.MAX_SAFE_INTEGER + 1 }; // 9007199254740992
    const result = await computeStateHash(state);
    assertTrue(result.hasUnsafeNumbers, 'MAX_SAFE_INTEGER + 1 should be unsafe');
  });

  await test('-MAX_SAFE_INTEGER is safe', async () => {
    const state = { negativeMax: -Number.MAX_SAFE_INTEGER };
    const result = await computeStateHash(state);
    assertFalse(result.hasUnsafeNumbers, '-MAX_SAFE_INTEGER should be safe');
  });

  await test('detects unsafe numbers in nested structures', async () => {
    const state = {
      outer: {
        inner: {
          values: [1, 2, 9007199254740993],
        },
      },
    };
    const result = await computeStateHash(state);
    assertTrue(result.hasUnsafeNumbers, 'nested unsafe number should be detected');
  });

  await test('detects unsafe numbers in arrays', async () => {
    const state = { data: [1, 2, 9007199254740993] };
    const result = await computeStateHash(state);
    assertTrue(result.hasUnsafeNumbers, 'unsafe number in array should be detected');
  });

  await test('safe when all numbers are small', async () => {
    const state = { a: 1, b: 2, c: -100, d: 0.5, e: 1000000 };
    const result = await computeStateHash(state);
    assertFalse(result.hasUnsafeNumbers, 'small numbers should all be safe');
  });

  // ==========================================================================
  // computeStateHash - concurrent computations (M-775)
  // ==========================================================================
  console.log('\ncomputeStateHash - concurrent computations (M-775):');

  await test('concurrent hash computations do not interfere', async () => {
    // M-775: Each computeStateHash call should have isolated context
    // Previously, a global variable caused race conditions

    // State with unsafe number
    const unsafeState = { bigNumber: 9007199254740993 };
    // State with safe numbers
    const safeState = { smallNumber: 42 };

    // Run both computations concurrently
    const [unsafeResult, safeResult] = await Promise.all([
      computeStateHash(unsafeState),
      computeStateHash(safeState),
    ]);

    // Each should have correct hasUnsafeNumbers flag
    assertTrue(unsafeResult.hasUnsafeNumbers, 'unsafe state should be flagged');
    assertFalse(safeResult.hasUnsafeNumbers, 'safe state should not be flagged');

    // Verify hashes are different (sanity check)
    assertTrue(
      toHex(unsafeResult.hash) !== toHex(safeResult.hash),
      'different states should have different hashes'
    );
  });

  await test('many concurrent computations all return correct results', async () => {
    // Run many concurrent hashes to stress-test isolation
    const promises: Promise<{ index: number; result: Awaited<ReturnType<typeof computeStateHash>> }>[] = [];

    for (let i = 0; i < 10; i++) {
      const state =
        i % 2 === 0
          ? { value: i, safe: true } // safe state
          : { value: 9007199254740993 + i }; // unsafe state
      promises.push(
        computeStateHash(state).then(result => ({ index: i, result }))
      );
    }

    const results = await Promise.all(promises);

    for (const { index, result } of results) {
      if (index % 2 === 0) {
        assertFalse(result.hasUnsafeNumbers, `index ${index} should be safe`);
      } else {
        assertTrue(result.hasUnsafeNumbers, `index ${index} should be unsafe`);
      }
    }
  });

  await test('interleaved safe and unsafe computations maintain isolation', async () => {
    const results = await Promise.all([
      computeStateHash({ safe: 1 }),
      computeStateHash({ unsafe: 9007199254740993 }),
      computeStateHash({ safe: 2 }),
      computeStateHash({ unsafe: 9007199254740994 }),
      computeStateHash({ safe: 3 }),
    ]);

    assertFalse(results[0].hasUnsafeNumbers, 'result 0 should be safe');
    assertTrue(results[1].hasUnsafeNumbers, 'result 1 should be unsafe');
    assertFalse(results[2].hasUnsafeNumbers, 'result 2 should be safe');
    assertTrue(results[3].hasUnsafeNumbers, 'result 3 should be unsafe');
    assertFalse(results[4].hasUnsafeNumbers, 'result 4 should be safe');
  });

  // ==========================================================================
  // computeStateHashLegacy
  // ==========================================================================
  console.log('\ncomputeStateHashLegacy:');

  await test('returns Uint8Array hash directly', async () => {
    const state = { a: 1, b: 2 };
    const hash = await computeStateHashLegacy(state);
    assertTrue(hash instanceof Uint8Array, 'should return Uint8Array');
    assertEqual(hash.length, 32, 'SHA-256 should be 32 bytes');
  });

  await test('matches computeStateHash result', async () => {
    const state = { test: 'data' };
    const legacyHash = await computeStateHashLegacy(state);
    const newResult = await computeStateHash(state);
    assertEqual(toHex(legacyHash), toHex(newResult.hash), 'legacy and new should match');
  });

  await test('returns consistent hash for same input', async () => {
    const state = { key: 'value' };
    const hash1 = await computeStateHashLegacy(state);
    const hash2 = await computeStateHashLegacy(state);
    assertEqual(toHex(hash1), toHex(hash2), 'same input should produce same hash');
  });

  // ==========================================================================
  // Edge cases and boundary conditions
  // ==========================================================================
  console.log('\nedge cases:');

  await test('handles object with null prototype', () => {
    const obj = Object.create(null);
    obj.a = 1;
    obj.b = 2;
    assertEqual(canonicalJsonString(obj), '{"a":1,"b":2}');
  });

  await test('handles very long strings', () => {
    const longString = 'a'.repeat(10000);
    const result = canonicalJsonString(longString);
    assertEqual(result, `"${longString}"`);
  });

  await test('handles object with many keys', () => {
    const obj: Record<string, number> = {};
    for (let i = 0; i < 100; i++) {
      obj[`key${i.toString().padStart(3, '0')}`] = i;
    }
    const result = canonicalJsonString(obj);
    // Keys should be sorted lexicographically
    assertTrue(result.indexOf('"key000"') < result.indexOf('"key001"'), 'key000 before key001');
    assertTrue(result.indexOf('"key099"') > result.indexOf('"key098"'), 'key099 after key098');
  });

  await test('handles Date objects as empty object', () => {
    // Date objects serialize to {} with JSON.stringify when you call toJSON
    // but our function treats them as objects with sorted keys
    const date = new Date('2024-01-01');
    const result = canonicalJsonString(date);
    // Date objects have no enumerable own properties, so they become {}
    assertEqual(result, '{}');
  });

  await test('handles array with holes', () => {
    // eslint-disable-next-line no-sparse-arrays
    const arr = [1, , 3];
    const result = canonicalJsonString(arr);
    // Note: Unlike JSON.stringify which converts holes to null,
    // our implementation preserves holes (map() preserves sparse array structure)
    // This is consistent behavior for state hashing
    assertEqual(result, '[1,,3]');
  });

  await test('handles exponential notation numbers', () => {
    assertEqual(canonicalJsonString(1e10), '10000000000');
    assertEqual(canonicalJsonString(1.5e-5), '0.000015');
  });

  await test('handles very small numbers', () => {
    assertEqual(canonicalJsonString(Number.MIN_VALUE), '5e-324');
  });

  await test('handles Number.EPSILON', () => {
    const result = canonicalJsonString(Number.EPSILON);
    assertEqual(result, '2.220446049250313e-16');
  });

  // ==========================================================================
  // Hash uniqueness tests
  // ==========================================================================
  console.log('\nhash uniqueness:');

  await test('different objects produce different hashes', async () => {
    const states = [
      { a: 1 },
      { a: 2 },
      { b: 1 },
      { a: 1, b: 2 },
      { a: '1' },
    ];

    const hashes = await Promise.all(states.map(s => computeStateHash(s)));
    const hexHashes = hashes.map(r => toHex(r.hash));

    // All hashes should be unique
    const uniqueHashes = new Set(hexHashes);
    assertEqual(uniqueHashes.size, states.length, 'all hashes should be unique');
  });

  await test('order-different objects produce same hash', async () => {
    const state1 = { z: 1, y: 2, x: 3 };
    const state2 = { x: 3, y: 2, z: 1 };
    const state3 = { y: 2, z: 1, x: 3 };

    const [hash1, hash2, hash3] = await Promise.all([
      computeStateHash(state1),
      computeStateHash(state2),
      computeStateHash(state3),
    ]);

    assertEqual(toHex(hash1.hash), toHex(hash2.hash), 'hash1 should equal hash2');
    assertEqual(toHex(hash2.hash), toHex(hash3.hash), 'hash2 should equal hash3');
  });

  await test('whitespace in strings produces different hash', async () => {
    const state1 = { text: 'hello' };
    const state2 = { text: 'hello ' };
    const state3 = { text: ' hello' };

    const [hash1, hash2, hash3] = await Promise.all([
      computeStateHash(state1),
      computeStateHash(state2),
      computeStateHash(state3),
    ]);

    assertTrue(toHex(hash1.hash) !== toHex(hash2.hash), 'trailing space should change hash');
    assertTrue(toHex(hash2.hash) !== toHex(hash3.hash), 'leading vs trailing space should differ');
    assertTrue(toHex(hash1.hash) !== toHex(hash3.hash), 'no space vs leading space should differ');
  });

  // ==========================================================================
  // Summary
  // ==========================================================================
  console.log('\n--------------------------');
  console.log(`Tests: ${passed} passed, ${failed} failed`);
  console.log('--------------------------\n');

  if (failed > 0) {
    process.exit(1);
  }
}

void run();
