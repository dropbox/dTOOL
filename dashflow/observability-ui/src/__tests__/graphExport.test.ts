// Unit tests for graph export utilities (DOT and Mermaid escaping)
// Run with: npx tsx src/__tests__/graphExport.test.ts

// IIFE to isolate scope from other test files
(function runGraphExportTests() {

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

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(`${message || 'Assertion failed'}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertContains(str: string, substring: string, message?: string): void {
  if (!str.includes(substring)) {
    throw new Error(`${message || 'Assertion failed'}: expected to contain "${substring}" in:\n${str}`);
  }
}

// assertNotContains available if needed
// function assertNotContains(str: string, substring: string, message?: string): void {
//   if (str.includes(substring)) {
//     throw new Error(`${message || 'Assertion failed'}: expected NOT to contain "${substring}" in:\n${str}`);
//   }
// }

// Helper functions that mirror the implementation in App.tsx
const escapeDot = (s: string): string => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
const sanitizeMermaidId = (s: string): string => s.replace(/[^a-zA-Z0-9_]/g, '_');
const escapeMermaidLabel = (s: string): string => s.replace(/"/g, '#quot;');

// DOT escaping tests
console.log('\nDOT format escaping:');

test('escapeDot handles plain text', () => {
  assertEqual(escapeDot('hello'), 'hello');
});

test('escapeDot escapes double quotes', () => {
  assertEqual(escapeDot('say "hello"'), 'say \\"hello\\"');
});

test('escapeDot escapes backslashes', () => {
  assertEqual(escapeDot('path\\to\\file'), 'path\\\\to\\\\file');
});

test('escapeDot handles mixed special characters', () => {
  assertEqual(escapeDot('file "test\\data"'), 'file \\"test\\\\data\\"');
});

test('escapeDot preserves newlines in DOT labels', () => {
  const input = 'line1\\nline2';
  const escaped = escapeDot(input);
  // Backslash-n should become double-backslash-n
  assertEqual(escaped, 'line1\\\\nline2');
});

// Mermaid ID sanitization tests
console.log('\nMermaid ID sanitization:');

test('sanitizeMermaidId allows alphanumeric', () => {
  assertEqual(sanitizeMermaidId('hello123'), 'hello123');
});

test('sanitizeMermaidId allows underscores', () => {
  assertEqual(sanitizeMermaidId('hello_world'), 'hello_world');
});

test('sanitizeMermaidId replaces spaces with underscores', () => {
  assertEqual(sanitizeMermaidId('hello world'), 'hello_world');
});

test('sanitizeMermaidId replaces special characters', () => {
  assertEqual(sanitizeMermaidId('node-1 (test)'), 'node_1__test_');
});

test('sanitizeMermaidId handles quotes', () => {
  assertEqual(sanitizeMermaidId('say "hi"'), 'say__hi_');
});

// Mermaid label escaping tests
console.log('\nMermaid label escaping:');

test('escapeMermaidLabel handles plain text', () => {
  assertEqual(escapeMermaidLabel('hello'), 'hello');
});

test('escapeMermaidLabel escapes quotes with HTML entity', () => {
  assertEqual(escapeMermaidLabel('say "hello"'), 'say #quot;hello#quot;');
});

test('escapeMermaidLabel preserves other characters', () => {
  assertEqual(escapeMermaidLabel('a < b > c & d'), 'a < b > c & d');
});

// Integration tests - simulating full export output
console.log('\nIntegration tests:');

test('DOT export with special characters produces valid syntax', () => {
  const nodeName = 'node "test"';
  const escaped = escapeDot(nodeName);
  const dotLine = `"${escaped}" [label="${escaped}"];`;
  // Should have escaped quotes (\" in the output)
  assertContains(dotLine, '\\"');
  // The escaped name should appear correctly
  assertEqual(dotLine, '"node \\"test\\"" [label="node \\"test\\""];');
});

test('Mermaid export with special characters produces valid syntax', () => {
  const nodeName = 'node-1 (test)';
  const nodeId = sanitizeMermaidId(nodeName);
  const label = escapeMermaidLabel(nodeName);
  const mermaidLine = `${nodeId}["${label}"]`;
  assertContains(mermaidLine, 'node_1__test_["node-1 (test)"]');
});

test('DOT handles node names with backslashes', () => {
  const nodeName = 'path\\to\\node';
  const escaped = escapeDot(nodeName);
  assertEqual(escaped, 'path\\\\to\\\\node');
});

test('Mermaid handles edge labels with quotes', () => {
  const label = 'condition "true"';
  const escaped = escapeMermaidLabel(label);
  assertEqual(escaped, 'condition #quot;true#quot;');
});

// Summary
console.log('\n---');
console.log(`${passed} passed, ${failed} failed\n`);

if (failed > 0) {
  process.exit(1);
}

})(); // End IIFE
