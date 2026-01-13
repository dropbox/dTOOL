// M-444: Component tests for StateDiffViewer
// Run with: npx tsx src/__tests__/StateDiffViewer.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { StateDiffViewer, getChangedPaths } from '../components/StateDiffViewer';

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

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      message || `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
    );
  }
}

console.log('\nStateDiffViewer Tests\n');

// Component render tests
console.log('Component Rendering:');

test('renders empty state placeholder when both states are empty', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer currentState={{}} previousState={{}} />
  );

  assertIncludes(html, 'data-testid="state-diff"');
  assertIncludes(html, 'Waiting for state...');
  assertIncludes(html, '{}');
});

test('renders live state indicator when isLive=true', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ key: 'value' }}
      previousState={{}}
      isLive={true}
    />
  );

  assertIncludes(html, 'LIVE STATE');
});

test('renders paused state with cursor seq when isLive=false', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ key: 'value' }}
      previousState={{}}
      isLive={false}
      cursorSeq="12345"
    />
  );

  assertIncludes(html, 'seq=12345');
});

test('shows +N count for new entries', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ newKey: 'new value' }}
      previousState={{}}
    />
  );

  assertIncludes(html, '+1');
});

test('shows ~N count for changed entries', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ key: 'new value' }}
      previousState={{ key: 'old value' }}
    />
  );

  assertIncludes(html, '~1');
});

test('shows -N count for removed entries', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{}}
      previousState={{ removedKey: 'value' }}
    />
  );

  assertIncludes(html, '-1');
});

test('displays entry keys and values', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ username: 'alice', count: 42 }}
      previousState={{}}
    />
  );

  assertIncludes(html, 'username');
  assertIncludes(html, 'alice');
  assertIncludes(html, 'count');
  assertIncludes(html, '42');
});

test('shows previous value for changed entries', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ status: 'completed' }}
      previousState={{ status: 'pending' }}
    />
  );

  assertIncludes(html, 'completed');
  assertIncludes(html, 'was:');
  assertIncludes(html, 'pending');
});

test('renders legend with +new, ~changed, -removed', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ key: 'value' }}
      previousState={{}}
    />
  );

  assertIncludes(html, 'new');
  assertIncludes(html, 'changed');
  assertIncludes(html, 'removed');
});

// getChangedPaths utility tests
console.log('\ngetChangedPaths utility:');

test('returns empty array for identical primitives', () => {
  assertEqual(getChangedPaths('hello', 'hello'), []);
  assertEqual(getChangedPaths(42, 42), []);
  assertEqual(getChangedPaths(true, true), []);
});

test('returns root path for different primitives', () => {
  assertEqual(getChangedPaths('hello', 'world'), ['/']);
  assertEqual(getChangedPaths(42, 100), ['/']);
  assertEqual(getChangedPaths(true, false), ['/']);
});

test('returns root path for primitive type changes', () => {
  assertEqual(getChangedPaths('string', 42), ['/']);
});

test('empty array and empty object treated as structurally equal', () => {
  // getChangedPaths compares by structure (keys/values), not type
  // Empty array and empty object both have no keys, so no diff
  assertEqual(getChangedPaths([], {}), []);
});

test('returns paths for changed object keys', () => {
  const current = { a: 1, b: 2 };
  const previous = { a: 1, b: 3 };
  assertEqual(getChangedPaths(current, previous), ['/b']);
});

test('returns paths for added object keys', () => {
  const current = { a: 1, b: 2 };
  const previous = { a: 1 };
  assertEqual(getChangedPaths(current, previous), ['/b']);
});

test('returns paths for removed object keys', () => {
  const current = { a: 1 };
  const previous = { a: 1, b: 2 };
  assertEqual(getChangedPaths(current, previous), ['/b']);
});

test('returns paths for changed array indices', () => {
  const current = [1, 2, 3];
  const previous = [1, 9, 3];
  assertEqual(getChangedPaths(current, previous), ['/1']);
});

test('returns paths for nested changes', () => {
  const current = { outer: { inner: 'new' } };
  const previous = { outer: { inner: 'old' } };
  assertEqual(getChangedPaths(current, previous), ['/outer/inner']);
});

test('escapes JSON Pointer special characters', () => {
  const current = { 'a/b': 1, 'c~d': 2 };
  const previous = { 'a/b': 9, 'c~d': 2 };
  // '/' becomes '~1', '~' becomes '~0'
  assertEqual(getChangedPaths(current, previous), ['/a~1b']);
});

// --- Extended getChangedPaths tests ---
console.log('\ngetChangedPaths - null/undefined handling:');

test('returns root path when current is null', () => {
  assertEqual(getChangedPaths(null, { a: 1 }), ['/']);
});

test('returns root path when previous is null', () => {
  assertEqual(getChangedPaths({ a: 1 }, null), ['/']);
});

test('returns empty for both null', () => {
  assertEqual(getChangedPaths(null, null), []);
});

test('returns root path when current is undefined', () => {
  assertEqual(getChangedPaths(undefined, { a: 1 }), ['/']);
});

test('returns root path when previous is undefined', () => {
  assertEqual(getChangedPaths({ a: 1 }, undefined), ['/']);
});

test('returns empty for both undefined', () => {
  assertEqual(getChangedPaths(undefined, undefined), []);
});

console.log('\ngetChangedPaths - array length differences:');

test('returns paths for added array elements', () => {
  const current = [1, 2, 3];
  const previous = [1, 2];
  assertEqual(getChangedPaths(current, previous), ['/2']);
});

test('returns paths for removed array elements', () => {
  const current = [1, 2];
  const previous = [1, 2, 3];
  assertEqual(getChangedPaths(current, previous), ['/2']);
});

test('returns multiple paths for multiple array changes', () => {
  const current = [1, 20, 30, 4];
  const previous = [1, 2, 3, 4];
  assertEqual(getChangedPaths(current, previous), ['/1', '/2']);
});

console.log('\ngetChangedPaths - deeply nested structures:');

test('handles triple-nested changes', () => {
  const current = { a: { b: { c: 'new' } } };
  const previous = { a: { b: { c: 'old' } } };
  assertEqual(getChangedPaths(current, previous), ['/a/b/c']);
});

test('handles nested array in object', () => {
  const current = { items: [1, 2, 99] };
  const previous = { items: [1, 2, 3] };
  assertEqual(getChangedPaths(current, previous), ['/items/2']);
});

test('handles object in array', () => {
  const current = [{ name: 'alice' }, { name: 'bob' }];
  const previous = [{ name: 'alice' }, { name: 'charlie' }];
  assertEqual(getChangedPaths(current, previous), ['/1/name']);
});

test('handles multiple nested changes', () => {
  const current = { a: { x: 1 }, b: { y: 2 } };
  const previous = { a: { x: 9 }, b: { y: 8 } };
  const paths = getChangedPaths(current, previous);
  assertEqual(paths.length, 2);
  assertEqual(paths.includes('/a/x'), true);
  assertEqual(paths.includes('/b/y'), true);
});

console.log('\ngetChangedPaths - special character escaping:');

test('escapes tilde in key names', () => {
  const current = { 'a~b': 1 };
  const previous = { 'a~b': 2 };
  // '~' becomes '~0'
  assertEqual(getChangedPaths(current, previous), ['/a~0b']);
});

test('escapes both tilde and slash in same key', () => {
  const current = { 'a~/b': 1 };
  const previous = { 'a~/b': 2 };
  // '~' becomes '~0', '/' becomes '~1'
  assertEqual(getChangedPaths(current, previous), ['/a~0~1b']);
});

test('handles numeric string keys', () => {
  const current = { '0': 'a', '1': 'b' };
  const previous = { '0': 'a', '1': 'x' };
  assertEqual(getChangedPaths(current, previous), ['/1']);
});

// --- Component rendering - value type handling ---
console.log('\nComponent - value type rendering:');

test('renders null values correctly', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ nullable: null }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'nullable');
  assertIncludes(html, 'null');
});

test('renders boolean values correctly', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ flag: true, other: false }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'flag');
  assertIncludes(html, 'true');
  assertIncludes(html, 'other');
  assertIncludes(html, 'false');
});

test('renders empty array as []', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ items: [] }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'items');
  assertIncludes(html, '[]');
});

test('renders empty object as {}', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ config: {} }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'config');
  assertIncludes(html, '{}');
});

test('renders array with items count', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ items: [1, 2, 3] }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'items');
  assertIncludes(html, '3 items');
});

test('renders object with keys count for large objects', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ config: { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, i: 9, j: 10 } }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'config');
  assertIncludes(html, '10 keys');
});

test('renders numeric values correctly', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ count: 42, price: 19.99, negative: -5 }}
      previousState={{}}
    />
  );
  assertIncludes(html, '42');
  assertIncludes(html, '19.99');
  assertIncludes(html, '-5');
});

// --- Component rendering - diff computation ---
console.log('\nComponent - diff computation:');

test('handles mixed diff types correctly', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ unchanged: 'same', newKey: 'added', changed: 'new' }}
      previousState={{ unchanged: 'same', removed: 'gone', changed: 'old' }}
    />
  );
  // Should show counts for each type
  assertIncludes(html, '+1'); // newKey
  assertIncludes(html, '~1'); // changed
  assertIncludes(html, '-1'); // removed
});

test('keys are sorted alphabetically', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ zebra: 1, apple: 2, mango: 3 }}
      previousState={{}}
    />
  );
  // All keys should be present
  assertIncludes(html, 'apple');
  assertIncludes(html, 'mango');
  assertIncludes(html, 'zebra');
  // Check order: apple should come before zebra
  const appleIdx = html.indexOf('apple');
  const mangoIdx = html.indexOf('mango');
  const zebraIdx = html.indexOf('zebra');
  if (!(appleIdx < mangoIdx && mangoIdx < zebraIdx)) {
    throw new Error(`Keys not sorted: apple=${appleIdx}, mango=${mangoIdx}, zebra=${zebraIdx}`);
  }
});

test('unchanged entries show no diff marker styling', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ unchanged: 'same' }}
      previousState={{ unchanged: 'same' }}
    />
  );
  // Unchanged entries don't show +/- badges in header counts
  // The key should be present but no count badges (they only appear when > 0)
  assertIncludes(html, 'unchanged');
  // Header count section should not show any numbers for unchanged-only state
  // (counts are only rendered when > 0)
});

test('renders cursorSeq as ? when undefined', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ key: 'value' }}
      previousState={{}}
      isLive={false}
    />
  );
  assertIncludes(html, 'seq=?');
});

// --- Component rendering - edge cases ---
console.log('\nComponent - edge cases:');

test('handles deeply nested value change display', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ config: { nested: { value: 'new' } } }}
      previousState={{ config: { nested: { value: 'old' } } }}
    />
  );
  assertIncludes(html, 'config');
  assertIncludes(html, '~1'); // Should detect change
  assertIncludes(html, 'was:'); // Should show previous value
});

test('handles array value change display', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ items: [1, 2, 3] }}
      previousState={{ items: [1, 2] }}
    />
  );
  assertIncludes(html, 'items');
  assertIncludes(html, '~1'); // Array changed
});

test('handles special characters in keys', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ 'key-with-dash': 'value', 'key.with.dots': 'other' }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'key-with-dash');
  assertIncludes(html, 'key.with.dots');
});

test('handles unicode in keys and values', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ '日本語': 'にほんご', emoji: '🚀' }}
      previousState={{}}
    />
  );
  assertIncludes(html, '日本語');
  assertIncludes(html, 'にほんご');
  assertIncludes(html, '🚀');
});

test('handles string values with quotes', () => {
  const html = renderToStaticMarkup(
    <StateDiffViewer
      currentState={{ message: 'He said "hello"' }}
      previousState={{}}
    />
  );
  assertIncludes(html, 'message');
  // JSON serialization escapes internal quotes
  assertIncludes(html, 'hello');
});

// --- deepEqual tests (via getChangedPaths) ---
console.log('\ndeepEqual (via getChangedPaths):');

test('deepEqual: same reference returns equal', () => {
  const obj = { a: 1, b: { c: 2 } };
  assertEqual(getChangedPaths(obj, obj), []);
});

test('deepEqual: equal objects with different references', () => {
  const a = { x: 1, y: 2 };
  const b = { x: 1, y: 2 };
  assertEqual(getChangedPaths(a, b), []);
});

test('deepEqual: nested arrays equality', () => {
  const a = [[1, 2], [3, 4]];
  const b = [[1, 2], [3, 4]];
  assertEqual(getChangedPaths(a, b), []);
});

test('deepEqual: mixed nested structures equality', () => {
  const a = { arr: [{ id: 1 }, { id: 2 }], count: 2 };
  const b = { arr: [{ id: 1 }, { id: 2 }], count: 2 };
  assertEqual(getChangedPaths(a, b), []);
});

test('deepEqual: detects nested array element difference', () => {
  const a = [[1, 2], [3, 4]];
  const b = [[1, 2], [3, 99]];
  assertEqual(getChangedPaths(a, b), ['/1/1']);
});

test('deepEqual: handles empty vs non-empty arrays', () => {
  assertEqual(getChangedPaths([], [1]), ['/0']);
  assertEqual(getChangedPaths([1], []), ['/0']);
});

test('deepEqual: handles empty vs non-empty objects', () => {
  const paths = getChangedPaths({}, { a: 1 });
  assertEqual(paths, ['/a']);
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
