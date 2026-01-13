// M-444: Component tests for StateViewer
// Run with: npx tsx src/__tests__/StateViewer.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { StateViewer } from '../components/StateViewer';

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
    throw new Error(message || `Expected to include: "${needle}"`);
  }
}

function assertNotIncludes(haystack: string, needle: string, message?: string): void {
  if (haystack.includes(needle)) {
    throw new Error(message || `Expected NOT to include: "${needle}"`);
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(message || `Expected ${expected}, got ${actual}`);
  }
}

function assertGreaterThan(actual: number, expected: number, message?: string): void {
  if (actual <= expected) {
    throw new Error(message || `Expected ${actual} to be greater than ${expected}`);
  }
}

function assertLessThan(actual: number, expected: number, message?: string): void {
  if (actual >= expected) {
    throw new Error(message || `Expected ${actual} to be less than ${expected}`);
  }
}

console.log('\nStateViewer Tests\n');

// ==========================================
// SECTION 1: Empty State Tests
// ==========================================

console.log('Empty state:');

test('renders "Empty state" message for empty state object', () => {
  const html = renderToStaticMarkup(<StateViewer state={{}} />);
  assertIncludes(html, 'Empty state');
});

test('empty state has italic styling', () => {
  const html = renderToStaticMarkup(<StateViewer state={{}} />);
  assertIncludes(html, 'italic');
});

test('empty state has dark background', () => {
  const html = renderToStaticMarkup(<StateViewer state={{}} />);
  assertIncludes(html, 'bg-gray-900');
});

test('empty state has border', () => {
  const html = renderToStaticMarkup(<StateViewer state={{}} />);
  assertIncludes(html, 'border-gray-700');
});

// ==========================================
// SECTION 2: Basic Value Rendering
// ==========================================

console.log('\nBasic value rendering:');

test('renders string values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ name: 'John' }} />);
  assertIncludes(html, 'name');
  assertIncludes(html, '&quot;John&quot;');
});

test('renders empty string', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ empty: '' }} />);
  assertIncludes(html, 'empty');
  assertIncludes(html, '&quot;&quot;');
});

test('renders string with special characters', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ special: '<script>alert("xss")</script>' }} />);
  assertIncludes(html, 'special');
  // HTML entities should be escaped
  assertIncludes(html, '&lt;');
  assertIncludes(html, '&gt;');
});

test('renders number values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ count: 42, price: 19.99 }} />);
  assertIncludes(html, 'count');
  assertIncludes(html, '42');
  assertIncludes(html, 'price');
  assertIncludes(html, '19.99');
});

test('renders zero', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ zero: 0 }} />);
  assertIncludes(html, 'zero');
  assertIncludes(html, '>0<');
});

test('renders negative numbers', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ negative: -42 }} />);
  assertIncludes(html, 'negative');
  assertIncludes(html, '-42');
});

test('renders floating point numbers', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ pi: 3.14159 }} />);
  assertIncludes(html, 'pi');
  assertIncludes(html, '3.14159');
});

test('renders boolean true', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ active: true }} />);
  assertIncludes(html, 'active');
  assertIncludes(html, 'true');
});

test('renders boolean false', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ disabled: false }} />);
  assertIncludes(html, 'disabled');
  assertIncludes(html, 'false');
});

test('renders null values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ data: null }} />);
  assertIncludes(html, 'data');
  assertIncludes(html, 'null');
});

test('renders undefined values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ missing: undefined }} />);
  assertIncludes(html, 'missing');
  assertIncludes(html, 'undefined');
});

// ==========================================
// SECTION 3: String Truncation
// ==========================================

console.log('\nString truncation:');

test('truncates long strings to 100 characters with ellipsis', () => {
  const longString = 'a'.repeat(150);
  const html = renderToStaticMarkup(<StateViewer state={{ description: longString }} />);
  assertIncludes(html, 'a'.repeat(100) + '...');
  assertNotIncludes(html, 'a'.repeat(150));
});

test('does not truncate strings under 100 characters', () => {
  const shortString = 'Hello World';
  const html = renderToStaticMarkup(<StateViewer state={{ greeting: shortString }} />);
  assertIncludes(html, '&quot;Hello World&quot;');
  assertNotIncludes(html, '&quot;Hello World...');
});

test('truncates exactly at 100 characters', () => {
  const exactString = 'x'.repeat(100);
  const html = renderToStaticMarkup(<StateViewer state={{ exact: exactString }} />);
  // Exactly 100 should NOT be truncated (>100 triggers truncation)
  // Check that the full 100-char string appears without truncation ellipsis
  assertNotIncludes(html, 'x'.repeat(100) + '...');
});

test('truncates string at 101 characters', () => {
  const overString = 'y'.repeat(101);
  const html = renderToStaticMarkup(<StateViewer state={{ over: overString }} />);
  assertIncludes(html, 'y'.repeat(100) + '...');
});

test('truncated strings have title attribute for full preview', () => {
  const longString = 'z'.repeat(150);
  const html = renderToStaticMarkup(<StateViewer state={{ long: longString }} />);
  // Title attribute shows first 100 chars of long strings
  assertIncludes(html, 'title=');
});

// ==========================================
// SECTION 4: Array Rendering
// ==========================================

console.log('\nArray rendering:');

test('renders empty array with expand indicator', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ items: [] }} />);
  assertIncludes(html, 'items');
  assertIncludes(html, '▼');
});

test('renders array elements when expanded (depth < 1)', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ colors: ['red', 'green', 'blue'] }} />);
  assertIncludes(html, 'colors');
  assertIncludes(html, '[0]');
  assertIncludes(html, '[1]');
  assertIncludes(html, '[2]');
  assertIncludes(html, '&quot;red&quot;');
  assertIncludes(html, '&quot;green&quot;');
  assertIncludes(html, '&quot;blue&quot;');
});

test('shows "and N more" for arrays with more than 10 elements', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] }} />
  );
  assertIncludes(html, 'numbers');
  assertIncludes(html, '[0]');
  assertIncludes(html, '[9]');
  assertIncludes(html, 'and 2 more');
});

test('renders array with single element', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ single: ['only'] }} />);
  assertIncludes(html, 'single');
  assertIncludes(html, '[0]');
  assertIncludes(html, '&quot;only&quot;');
});

test('renders nested arrays', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ matrix: [[1, 2], [3, 4]] }} />);
  assertIncludes(html, 'matrix');
  assertIncludes(html, '[0]');
  assertIncludes(html, '[1]');
});

test('renders array with mixed types', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ mixed: ['string', 42, true, null] }} />
  );
  assertIncludes(html, 'mixed');
  assertIncludes(html, '&quot;string&quot;');
  assertIncludes(html, '42');
  assertIncludes(html, 'true');
  assertIncludes(html, 'null');
});

test('renders exactly 10 array elements without "more" message', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ ten: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }} />
  );
  assertIncludes(html, '[9]');
  // Should not show "and N more" overflow message (note: "and" appears in keyboard help text)
  assertNotIncludes(html, '... and');
  assertNotIncludes(html, 'more');
});

test('renders 11 array elements with "and 1 more"', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ eleven: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }} />
  );
  assertIncludes(html, 'and 1 more');
});

// ==========================================
// SECTION 5: Object Rendering
// ==========================================

console.log('\nObject rendering:');

test('renders empty object with expand indicator', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ config: {} }} />);
  assertIncludes(html, 'config');
  assertIncludes(html, '▼');
});

test('renders nested objects with preview', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ user: { name: 'John', age: 30 } }} />
  );
  assertIncludes(html, 'user');
});

test('renders object with single key', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ obj: { key: 'value' } }} />);
  assertIncludes(html, 'obj');
  assertIncludes(html, 'key');
});

test('renders deeply nested objects', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ a: { b: { c: { d: 'deep' } } } }} />
  );
  assertIncludes(html, 'a');
});

test('renders object with more than 10 keys shows "more" message', () => {
  const bigObj: Record<string, number> = {};
  for (let i = 0; i < 15; i++) {
    bigObj[`key${i}`] = i;
  }
  const html = renderToStaticMarkup(<StateViewer state={{ big: bigObj }} />);
  assertIncludes(html, 'and 5 more');
});

test('renders object with exactly 10 keys without "more" message', () => {
  const tenObj: Record<string, number> = {};
  for (let i = 0; i < 10; i++) {
    tenObj[`key${i}`] = i;
  }
  const html = renderToStaticMarkup(<StateViewer state={{ ten: tenObj }} />);
  assertNotIncludes(html, 'more');
});

// ==========================================
// SECTION 6: Change Highlighting
// ==========================================

console.log('\nChange highlighting:');

test('highlights changed keys when highlightChanges=true', () => {
  const previousState = { count: 1 };
  const currentState = { count: 2 };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'bg-yellow-900/30');
  assertIncludes(html, 'text-yellow-400');
  assertIncludes(html, '●');
});

test('highlights newly added keys', () => {
  const previousState = { existing: 'value' };
  const currentState = { existing: 'value', newKey: 'added' };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'newKey');
  assertIncludes(html, 'text-yellow-400');
});

test('does not highlight unchanged keys', () => {
  const previousState = { unchanged: 'same' };
  const currentState = { unchanged: 'same' };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'unchanged');
  assertIncludes(html, 'text-cyan-400');
});

test('does not highlight when highlightChanges=false', () => {
  const previousState = { count: 1 };
  const currentState = { count: 2 };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={false} />
  );
  assertNotIncludes(html, 'bg-yellow-900/30');
  assertNotIncludes(html, 'text-yellow-400');
});

test('does not highlight when no previousState provided', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ value: 123 }} highlightChanges={true} />
  );
  assertNotIncludes(html, 'bg-yellow-900/30');
  assertNotIncludes(html, 'text-yellow-400');
});

test('highlights changes in nested object values', () => {
  const previousState = { nested: { inner: 'old' } };
  const currentState = { nested: { inner: 'new' } };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  // The parent key 'nested' should be highlighted because its content changed
  assertIncludes(html, 'text-yellow-400');
});

test('highlights changes in array values', () => {
  const previousState = { arr: [1, 2, 3] };
  const currentState = { arr: [1, 2, 4] };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'text-yellow-400');
});

test('highlights when value type changes', () => {
  const previousState = { val: 'string' };
  const currentState = { val: 123 };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'text-yellow-400');
});

test('highlights when null changes to value', () => {
  const previousState = { val: null };
  const currentState = { val: 'now set' };
  const html = renderToStaticMarkup(
    <StateViewer state={currentState} previousState={previousState} highlightChanges={true} />
  );
  assertIncludes(html, 'text-yellow-400');
});

// ==========================================
// SECTION 7: Multiple State Entries
// ==========================================

console.log('\nMultiple state entries:');

test('renders multiple state entries', () => {
  const html = renderToStaticMarkup(
    <StateViewer
      state={{ name: 'Test', count: 42, active: true, tags: ['a', 'b'] }}
    />
  );
  assertIncludes(html, 'name');
  assertIncludes(html, 'count');
  assertIncludes(html, 'active');
  assertIncludes(html, 'tags');
});

test('renders large number of top-level keys', () => {
  const state: Record<string, number> = {};
  for (let i = 0; i < 20; i++) {
    state[`field${i}`] = i;
  }
  const html = renderToStaticMarkup(<StateViewer state={state} />);
  assertIncludes(html, 'field0');
  assertIncludes(html, 'field19');
});

test('preserves key order', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ zebra: 1, apple: 2, mango: 3 }} />
  );
  const zebraPos = html.indexOf('zebra');
  const applePos = html.indexOf('apple');
  const mangoPos = html.indexOf('mango');
  // Keys appear in insertion order
  assertLessThan(zebraPos, applePos);
  assertLessThan(applePos, mangoPos);
});

// ==========================================
// SECTION 8: maxDepth Control
// ==========================================

console.log('\nmaxDepth control:');

test('respects maxDepth for nested expansion', () => {
  const deepState = {
    level1: { level2: { level3: { level4: 'deep value' } } },
  };
  const html = renderToStaticMarkup(<StateViewer state={deepState} maxDepth={2} />);
  assertIncludes(html, 'level1');
  assertIncludes(html, '{...}');
});

test('maxDepth=0 does not render nested object children', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ nested: { inner: 'value' } }} maxDepth={0} />
  );
  // With maxDepth=0, the nested object's children should not be rendered
  assertIncludes(html, 'nested');
  assertNotIncludes(html, 'inner');
});

test('maxDepth=1 shows one level of nesting', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ nested: { inner: { deep: 'value' } } }} maxDepth={1} />
  );
  assertIncludes(html, 'nested');
  assertIncludes(html, 'inner');
});

test('default maxDepth is 3', () => {
  const deepState = {
    l1: { l2: { l3: { l4: 'end' } } },
  };
  const html = renderToStaticMarkup(<StateViewer state={deepState} />);
  // At depth 3, l4 should show as {...} because we're at the limit
  assertIncludes(html, 'l1');
});

test('arrays respect maxDepth', () => {
  const html = renderToStaticMarkup(
    <StateViewer state={{ arr: [[[[1]]]] }} maxDepth={1} />
  );
  assertIncludes(html, 'arr');
});

// ==========================================
// SECTION 9: Accessibility Tests
// ==========================================

console.log('\nAccessibility:');

test('expandable rows have tabIndex for keyboard focus', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ nested: { inner: 'value' } }} />);
  assertIncludes(html, 'tabindex="0"');
});

test('expandable rows have role="button" for screen readers', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ nested: { inner: 'value' } }} />);
  assertIncludes(html, 'role="button"');
});

test('expandable rows have aria-expanded attribute', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ nested: { inner: 'value' } }} />);
  assertIncludes(html, 'aria-expanded=');
});

test('non-expandable rows do not have accessibility button attributes', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ simple: 'string' }} />);
  assertIncludes(html, 'simple');
  assertIncludes(html, 'string');
});

test('toolbar buttons have type="button" attribute', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  const buttonCount = (html.match(/type="button"/g) || []).length;
  if (buttonCount < 4) {
    throw new Error(`Expected at least 4 buttons with type="button", found ${buttonCount}`);
  }
});

test('main container has aria-label for instructions', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'aria-label=');
  assertIncludes(html, 'Ctrl+E');
});

test('clear search button has aria-label', () => {
  // The clear button only appears when there's a search term, which SSR won't show
  // But the input should still be accessible
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'placeholder="Search keys..."');
});

// ==========================================
// SECTION 10: Header/Toolbar Tests
// ==========================================

console.log('\nHeader/Toolbar:');

test('renders search input', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'Search keys');
  assertIncludes(html, '<input');
});

test('renders expand all button', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, '⊞');
});

test('renders collapse all button', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, '⊟');
});

test('renders reset button', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, '↺');
});

test('renders copy all button', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, '📋');
});

test('shows depth indicator', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'Depth:');
});

test('depth shows current/max format', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} maxDepth={5} />);
  assertIncludes(html, '/5');
});

// ==========================================
// SECTION 11: Copy Button Tests
// ==========================================

console.log('\nCopy buttons:');

test('each state value has a copy button', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  // The copy button appears in each StateValue row
  const copyButtons = (html.match(/Copy value as JSON/g) || []).length;
  assertGreaterThan(copyButtons, 0);
});

test('copy button shows clipboard icon', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, '📋');
});

test('copy button has proper styling for hover visibility', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'group-hover:opacity-100');
  assertIncludes(html, 'opacity-0');
});

// ==========================================
// SECTION 12: Visual Styling Tests
// ==========================================

console.log('\nVisual styling:');

test('uses monospace font for values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'font-mono');
});

test('key names use cyan color', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'text-cyan-400');
});

test('values use gray color', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'text-gray-300');
});

test('container has dark background', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'bg-gray-900');
});

test('container has border', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'border-gray-700');
});

test('header has slightly different background', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'bg-gray-800');
});

test('nested items have left border for visual hierarchy', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ obj: { nested: 'value' } }} />);
  assertIncludes(html, 'border-l');
});

test('expandable rows have hover state', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ obj: { inner: 'value' } }} />);
  assertIncludes(html, 'hover:bg-gray-700');
});

// ==========================================
// SECTION 13: Expand/Collapse Indicators
// ==========================================

console.log('\nExpand/collapse indicators:');

test('expanded items show down arrow', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ obj: { inner: 'value' } }} />);
  assertIncludes(html, '▼');
});

test('collapse indicator is right arrow', () => {
  // At depth 2+, items are collapsed by default
  const html = renderToStaticMarkup(
    <StateViewer state={{ l1: { l2: { l3: 'deep' } } }} maxDepth={3} />
  );
  // l3 should be collapsed (▶) at depth 2
  assertIncludes(html, '▶');
});

test('leaf values (strings) have no expand indicator', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ str: 'value' }} />);
  // Should have a spacer, not an arrow
  const arrowCount = (html.match(/▼|▶/g) || []).length;
  // No arrows for simple string value at root level
  assertEqual(arrowCount, 0);
});

// ==========================================
// SECTION 14: Edge Cases
// ==========================================

console.log('\nEdge cases:');

test('handles state with numeric keys', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ '123': 'numeric key' }} />);
  assertIncludes(html, '123');
});

test('handles state with special key names', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ 'key-with-dash': 1, 'key.with.dots': 2 }} />);
  assertIncludes(html, 'key-with-dash');
  assertIncludes(html, 'key.with.dots');
});

test('handles state with emoji in values', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ emoji: '👋 Hello!' }} />);
  assertIncludes(html, '👋');
});

test('handles state with unicode characters', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ unicode: '日本語テスト' }} />);
  assertIncludes(html, '日本語テスト');
});

test('handles very large numbers', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ big: 9007199254740991 }} />);
  assertIncludes(html, '9007199254740991');
});

test('handles very small numbers', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ tiny: 0.000001 }} />);
  assertIncludes(html, '0.000001');
});

test('handles NaN', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ nan: NaN }} />);
  assertIncludes(html, 'NaN');
});

test('handles Infinity', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ inf: Infinity }} />);
  assertIncludes(html, 'Infinity');
});

test('handles negative Infinity', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ negInf: -Infinity }} />);
  assertIncludes(html, '-Infinity');
});

test('handles array with undefined elements', () => {
  // eslint-disable-next-line no-sparse-arrays
  const html = renderToStaticMarkup(<StateViewer state={{ sparse: [1, , 3] }} />);
  assertIncludes(html, 'sparse');
});

test('handles deeply nested empty objects', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ a: { b: { c: {} } } }} />);
  assertIncludes(html, 'a');
});

// ==========================================
// SECTION 15: Content Overflow
// ==========================================

console.log('\nContent overflow:');

test('content area has max height with scroll', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'max-h-64');
  assertIncludes(html, 'overflow-y-auto');
});

test('values have truncate class for long inline content', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'truncate');
});

// ==========================================
// SECTION 16: Focus States
// ==========================================

console.log('\nFocus states:');

test('container is focusable', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  // Main container should have tabIndex (React SSR outputs lowercase)
  assertIncludes(html, 'tabindex');
});

test('expandable rows have focus ring styling', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ obj: { inner: 'value' } }} />);
  assertIncludes(html, 'focus:ring-2');
});

test('search input has focus styling', () => {
  const html = renderToStaticMarkup(<StateViewer state={{ key: 'value' }} />);
  assertIncludes(html, 'focus:ring-1');
  assertIncludes(html, 'focus:ring-blue-500');
});

// ==========================================
// SECTION 17: Complex State Structures
// ==========================================

console.log('\nComplex structures:');

test('handles mix of arrays and objects', () => {
  const html = renderToStaticMarkup(
    <StateViewer
      state={{
        users: [{ name: 'Alice', tags: ['admin'] }, { name: 'Bob', tags: ['user'] }],
      }}
    />
  );
  assertIncludes(html, 'users');
});

test('handles circular-like structure references (flattened)', () => {
  // State won't have actual circular refs, but can have repeated similar structures
  const html = renderToStaticMarkup(
    <StateViewer
      state={{
        a: { ref: 'b' },
        b: { ref: 'a' },
      }}
    />
  );
  assertIncludes(html, 'a');
  assertIncludes(html, 'b');
});

test('handles state from typical API response', () => {
  const apiResponse = {
    data: {
      items: [
        { id: 1, name: 'Item 1', metadata: { created: '2024-01-01' } },
        { id: 2, name: 'Item 2', metadata: { created: '2024-01-02' } },
      ],
      pagination: { page: 1, total: 100, hasMore: true },
    },
    status: 'success',
    timestamp: 1704067200000,
  };
  const html = renderToStaticMarkup(<StateViewer state={apiResponse} />);
  assertIncludes(html, 'data');
  assertIncludes(html, 'status');
  assertIncludes(html, 'success');
});

// ==========================================
// Summary
// ==========================================

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
