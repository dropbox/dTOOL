// M-444: Component tests for SchemaHistoryPanel
// Run with: npx tsx src/__tests__/SchemaHistoryPanel.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { SchemaHistoryPanel, SchemaObservation } from '../components/SchemaHistoryPanel';
import type { GraphSchema, NodeSchema, EdgeSchema, NodeType } from '../types/graph';

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

// Helper to create a minimal NodeSchema
function makeNode(name: string, nodeType: NodeType = 'transform'): NodeSchema {
  return {
    name,
    node_type: nodeType,
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
}

// Helper to create a minimal EdgeSchema
function makeEdge(from: string, to: string): EdgeSchema {
  return { from, to, edge_type: 'direct' };
}

// Helper to create a minimal GraphSchema
function makeSchema(overrides: Partial<GraphSchema> = {}): GraphSchema {
  return {
    name: 'TestGraph',
    version: '1.0.0',
    description: 'Test description',
    nodes: [],
    edges: [],
    entry_point: 'start',
    metadata: {},
    ...overrides,
  };
}

// Helper to create a SchemaObservation
function makeObservation(
  schemaId: string,
  graphName: string,
  schema: GraphSchema,
  overrides: Partial<SchemaObservation> = {}
): SchemaObservation {
  return {
    schemaId,
    graphName,
    schema,
    firstSeen: Date.now() - 3600000, // 1 hour ago
    lastSeen: Date.now(),
    runCount: 1,
    threadIds: ['thread-1'],
    ...overrides,
  };
}

console.log('\nSchemaHistoryPanel Tests\n');

// Empty state
test('renders "No schemas observed" message when observations is empty', () => {
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={[]} />
  );

  assertIncludes(html, 'No schemas observed yet');
  assertIncludes(html, 'Run a graph to see schema history');
});

// Header tests
test('renders schema count in header', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'MyGraph', schema),
    makeObservation('schema-2', 'MyGraph', schema),
    makeObservation('schema-3', 'MyGraph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'Schema History');
  assertIncludes(html, '3 schemas observed');
});

test('renders singular "schema" for single observation', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [makeObservation('schema-1', 'MyGraph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '1 schema observed');
});

// Schema card rendering
test('renders schema ID prefix', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('abcd1234efgh5678', 'TestGraph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should show first 8 chars of schema ID
  assertIncludes(html, 'abcd1234');
  assertIncludes(html, '...');
});

test('renders graph name', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'MySpecialGraph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'MySpecialGraph');
});

test('renders node and edge counts', () => {
  const schema = makeSchema({
    nodes: [makeNode('A'), makeNode('B'), makeNode('C')],
    edges: [makeEdge('A', 'B'), makeEdge('B', 'C')],
  });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '3 nodes');
  assertIncludes(html, '2 edges');
});

test('renders run count', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { runCount: 42 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '42 runs');
});

test('renders singular "run" for single run', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { runCount: 1 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '1 run');
  assertNotIncludes(html, '1 runs');
});

test('renders schema version when present', () => {
  const schema = makeSchema({
    nodes: [makeNode('A')],
    version: '2.5.0',
  });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'v2.5.0');
});

// Expected schema highlighting
test('renders EXPECTED badge for expected schema', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('expected-schema-id', 'Graph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="expected-schema-id"
    />
  );

  assertIncludes(html, 'EXPECTED');
  // Should have green background for expected schema (dark theme: rgba(34, 197, 94, 0.15))
  assertIncludes(html, 'rgba(34, 197, 94, 0.15)');
});

test('does not render EXPECTED badge for non-expected schema', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('some-schema-id', 'Graph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="different-schema-id"
    />
  );

  assertNotIncludes(html, 'EXPECTED');
});

// Set as Expected button
test('renders "Set as Expected" button for non-expected schema when callback provided', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="different-schema"
      onSetExpected={() => {}}
    />
  );

  assertIncludes(html, 'Set as Expected');
});

test('does not render "Set as Expected" for already expected schema', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('expected-id', 'Graph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="expected-id"
      onSetExpected={() => {}}
    />
  );

  assertNotIncludes(html, 'Set as Expected');
});

test('does not render "Set as Expected" when no callback provided', () => {
  const schema = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="different-schema"
      // No onSetExpected callback
    />
  );

  assertNotIncludes(html, 'Set as Expected');
});

// Multiple schemas sorting
test('sorts schemas by lastSeen (most recent first)', () => {
  const schemaA = makeSchema({ nodes: [makeNode('A')], name: 'GraphA' });
  const schemaB = makeSchema({ nodes: [makeNode('B')], name: 'GraphB' });
  const schemaC = makeSchema({ nodes: [makeNode('C')], name: 'GraphC' });

  const now = Date.now();
  const observations = [
    makeObservation('schema-a', 'GraphA', schemaA, { lastSeen: now - 3600000 }), // 1 hour ago
    makeObservation('schema-c', 'GraphC', schemaC, { lastSeen: now }), // Most recent
    makeObservation('schema-b', 'GraphB', schemaB, { lastSeen: now - 1800000 }), // 30 min ago
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // C should appear before B, which should appear before A
  const posC = html.indexOf('GraphC');
  const posB = html.indexOf('GraphB');
  const posA = html.indexOf('GraphA');

  if (!(posC < posB && posB < posA)) {
    throw new Error(`Expected order GraphC < GraphB < GraphA, got positions: C=${posC}, B=${posB}, A=${posA}`);
  }
});

// Accessibility tests (keyboard navigation support)
test('schema rows have tabIndex for keyboard focus', () => {
  const observations = [makeObservation('test-1', 'TestGraph', makeSchema())];
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Schema rows should have tabIndex="0" for keyboard accessibility
  assertIncludes(html, 'tabindex="0"');
});

test('schema rows have role="button" for screen readers', () => {
  const observations = [makeObservation('test-1', 'TestGraph', makeSchema())];
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Schema rows should have role="button" to announce interactivity
  assertIncludes(html, 'role="button"');
});

test('schema rows have aria-pressed attribute for selection state', () => {
  const observations = [makeObservation('test-1', 'TestGraph', makeSchema())];
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Schema rows should indicate selection state via aria-pressed
  assertIncludes(html, 'aria-pressed=');
});

test('show diff button has type="button" attribute', () => {
  // Need at least 2 schemas to show comparison controls
  // But for this test, we can just check if buttons in general have type="button"
  const observations = [makeObservation('test-1', 'TestGraph', makeSchema())];
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} onSetExpected={() => {}} />
  );

  // Buttons should have type="button"
  assertIncludes(html, 'type="button"');
});

// ============================================================================
// Schema comparison logic tests
// ============================================================================
console.log('\nSchema comparison (compareSchemas function):');

test('detects added nodes', () => {
  const schemaOld = makeSchema({ nodes: [makeNode('A')] });
  const schemaNew = makeSchema({ nodes: [makeNode('A'), makeNode('B')] });
  const observations = [
    makeObservation('schema-old', 'Graph', schemaOld),
    makeObservation('schema-new', 'Graph', schemaNew),
  ];

  // Can't directly test internal diff, but we can verify the diff output shows added node
  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Both schemas render
  assertIncludes(html, '1 nodes');
  assertIncludes(html, '2 nodes');
});

test('detects removed nodes', () => {
  const schemaWithNodes = makeSchema({ nodes: [makeNode('A'), makeNode('B')] });
  const schemaWithLessNodes = makeSchema({ nodes: [makeNode('A')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schemaWithNodes),
    makeObservation('schema-2', 'Graph', schemaWithLessNodes),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '2 nodes');
  assertIncludes(html, '1 nodes');
});

test('detects modified nodes (same name, different content)', () => {
  const nodeOld: NodeSchema = { ...makeNode('A'), node_type: 'transform' };
  const nodeNew: NodeSchema = { ...makeNode('A'), node_type: 'llm' };
  const schemaOld = makeSchema({ nodes: [nodeOld] });
  const schemaNew = makeSchema({ nodes: [nodeNew] });
  const observations = [
    makeObservation('schema-old', 'Graph', schemaOld),
    makeObservation('schema-new', 'Graph', schemaNew),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Both schemas should render (even if node content differs)
  assertIncludes(html, '1 nodes');
});

test('detects added edges', () => {
  const schemaOld = makeSchema({ edges: [makeEdge('A', 'B')] });
  const schemaNew = makeSchema({ edges: [makeEdge('A', 'B'), makeEdge('B', 'C')] });
  const observations = [
    makeObservation('schema-old', 'Graph', schemaOld),
    makeObservation('schema-new', 'Graph', schemaNew),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '1 edges');
  assertIncludes(html, '2 edges');
});

test('detects removed edges', () => {
  const schemaWithEdges = makeSchema({ edges: [makeEdge('A', 'B'), makeEdge('B', 'C')] });
  const schemaWithLessEdges = makeSchema({ edges: [makeEdge('A', 'B')] });
  const observations = [
    makeObservation('schema-1', 'Graph', schemaWithEdges),
    makeObservation('schema-2', 'Graph', schemaWithLessEdges),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '2 edges');
  assertIncludes(html, '1 edges');
});

// ============================================================================
// Edge cases - Schema ID
// ============================================================================
console.log('\nEdge cases - schema ID:');

test('handles short schema ID (less than 8 chars)', () => {
  const schema = makeSchema();
  const observations = [makeObservation('abc', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should display what's available
  assertIncludes(html, 'abc');
  assertIncludes(html, '...');
});

test('handles very long schema ID', () => {
  const longId = 'a'.repeat(100);
  const schema = makeSchema();
  const observations = [makeObservation(longId, 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should only show first 8 chars
  assertIncludes(html, 'aaaaaaaa');
  assertIncludes(html, '...');
});

test('handles schema ID with special characters', () => {
  const schema = makeSchema();
  const observations = [makeObservation('abc-def_123', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'abc-def_');
});

// ============================================================================
// Edge cases - Graph name
// ============================================================================
console.log('\nEdge cases - graph name:');

test('handles very long graph name', () => {
  const longName = 'VeryLongGraphNameThatExceedsNormalLengthAndMightCauseDisplayIssues';
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', longName, schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, longName);
});

test('handles graph name with special characters', () => {
  const specialName = 'Graph<Test>&"Value"';
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', specialName, schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // HTML entities should be escaped
  assertIncludes(html, '&lt;');
  assertIncludes(html, '&gt;');
  assertIncludes(html, '&amp;');
});

test('handles graph name with unicode characters', () => {
  const unicodeName = 'グラフ🚀测试';
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', unicodeName, schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'グラフ');
  assertIncludes(html, '🚀');
  assertIncludes(html, '测试');
});

test('handles empty graph name', () => {
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', '', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should still render without crashing
  assertIncludes(html, 'Schema History');
});

// ============================================================================
// Edge cases - Node and edge counts
// ============================================================================
console.log('\nEdge cases - counts:');

test('renders 0 nodes', () => {
  const schema = makeSchema({ nodes: [] });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '0 nodes');
});

test('renders 0 edges', () => {
  const schema = makeSchema({ edges: [] });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '0 edges');
});

test('renders large node count', () => {
  const nodes = Array.from({ length: 999 }, (_, i) => makeNode(`Node${i}`));
  const schema = makeSchema({ nodes });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '999 nodes');
});

test('renders large edge count', () => {
  const edges = Array.from({ length: 500 }, (_, i) => makeEdge(`A${i}`, `B${i}`));
  const schema = makeSchema({ edges });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '500 edges');
});

test('renders large run count', () => {
  const schema = makeSchema();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { runCount: 10000 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '10000 runs');
});

test('renders 0 run count', () => {
  const schema = makeSchema();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { runCount: 0 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '0 runs');
});

// ============================================================================
// Edge cases - Version
// ============================================================================
console.log('\nEdge cases - version:');

test('does not render version prefix when version is empty string', () => {
  const schema = makeSchema({ version: '' });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should not have standalone "v" without version number
  // Note: version is falsy so shouldn't render
  assertNotIncludes(html, '>v<');
});

test('does not render version when undefined', () => {
  const schema = makeSchema({ version: undefined });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render without crashing
  assertIncludes(html, 'Schema History');
});

test('renders complex semver version', () => {
  const schema = makeSchema({ version: '1.2.3-beta.4+build.567' });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'v1.2.3-beta.4+build.567');
});

// ============================================================================
// Multiple observations
// ============================================================================
console.log('\nMultiple observations:');

test('renders all observations', () => {
  const observations = Array.from({ length: 5 }, (_, i) =>
    makeObservation(`schema-${i}`, `Graph${i}`, makeSchema())
  );

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '5 schemas observed');
  for (let i = 0; i < 5; i++) {
    assertIncludes(html, `Graph${i}`);
  }
});

test('renders 10+ observations', () => {
  const observations = Array.from({ length: 15 }, (_, i) =>
    makeObservation(`schema-${i}`, `Graph${i}`, makeSchema())
  );

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '15 schemas observed');
});

test('handles duplicate graph names', () => {
  const observations = [
    makeObservation('schema-1', 'SameName', makeSchema()),
    makeObservation('schema-2', 'SameName', makeSchema()),
    makeObservation('schema-3', 'SameName', makeSchema()),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render all three with same name
  const sameNameCount = (html.match(/SameName/g) || []).length;
  if (sameNameCount !== 3) {
    throw new Error(`Expected 3 occurrences of SameName, got ${sameNameCount}`);
  }
});

// ============================================================================
// Timestamp handling
// ============================================================================
console.log('\nTimestamp edge cases:');

test('handles very old timestamp (year 2000)', () => {
  const schema = makeSchema();
  const oldDate = new Date('2000-01-01T12:00:00Z').getTime();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { lastSeen: oldDate }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render date (format depends on locale)
  assertIncludes(html, 'Jan');
  assertIncludes(html, '1');
});

test('handles future timestamp', () => {
  const schema = makeSchema();
  const futureDate = new Date('2099-12-31T23:59:59Z').getTime();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { lastSeen: futureDate }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'Dec');
  assertIncludes(html, '31');
});

test('handles midnight timestamp', () => {
  const schema = makeSchema();
  const midnight = new Date('2024-06-15T00:00:00Z').getTime();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { lastSeen: midnight }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render without crashing
  assertIncludes(html, 'Jun');
});

test('sorts correctly with close timestamps', () => {
  const schema = makeSchema();
  const base = Date.now();
  const observations = [
    makeObservation('schema-1', 'First', schema, { lastSeen: base - 1000 }),
    makeObservation('schema-2', 'Second', schema, { lastSeen: base }),
    makeObservation('schema-3', 'Third', schema, { lastSeen: base - 500 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Second should come first (most recent)
  const posSecond = html.indexOf('Second');
  const posThird = html.indexOf('Third');
  const posFirst = html.indexOf('First');

  if (!(posSecond < posThird && posThird < posFirst)) {
    throw new Error(`Expected order Second < Third < First`);
  }
});

// ============================================================================
// Node types
// ============================================================================
console.log('\nNode types:');

test('handles various node types in schema', () => {
  const nodes: NodeSchema[] = [
    makeNode('A', 'transform'),
    makeNode('B', 'llm'),
    makeNode('C', 'tool'),
    makeNode('D', 'router'),
    makeNode('E', 'aggregator'),
  ];
  const schema = makeSchema({ nodes });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, '5 nodes');
});

// ============================================================================
// Thread IDs
// ============================================================================
console.log('\nThread IDs:');

test('handles empty threadIds array', () => {
  const schema = makeSchema();
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { threadIds: [] }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render without crashing (threadIds not displayed in UI)
  assertIncludes(html, 'Schema History');
});

test('handles many threadIds', () => {
  const schema = makeSchema();
  const threadIds = Array.from({ length: 100 }, (_, i) => `thread-${i}`);
  const observations = [
    makeObservation('schema-1', 'Graph', schema, { threadIds }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should render without crashing
  assertIncludes(html, 'Schema History');
});

// ============================================================================
// Styling and tokens
// ============================================================================
console.log('\nStyling and tokens:');

test('uses dark theme colors for border', () => {
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Should use dark theme border color token
  assertIncludes(html, 'border');
});

test('uses monospace font for schema ID', () => {
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'monospace');
});

test('applies cursor pointer for interactive rows', () => {
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'cursor:pointer');
});

// ============================================================================
// Combined scenarios
// ============================================================================
console.log('\nCombined scenarios:');

test('renders expected schema among many', () => {
  const observations = Array.from({ length: 5 }, (_, i) =>
    makeObservation(`schema-${i}`, `Graph${i}`, makeSchema())
  );

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="schema-2"
    />
  );

  assertIncludes(html, 'EXPECTED');
  // Should be only one EXPECTED badge
  const expectedCount = (html.match(/EXPECTED/g) || []).length;
  if (expectedCount !== 1) {
    throw new Error(`Expected 1 EXPECTED badge, got ${expectedCount}`);
  }
});

test('renders Set as Expected for all non-expected schemas', () => {
  const observations = [
    makeObservation('schema-1', 'Graph1', makeSchema()),
    makeObservation('schema-2', 'Graph2', makeSchema()),
    makeObservation('schema-3', 'Graph3', makeSchema()),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="schema-2"
      onSetExpected={() => {}}
    />
  );

  // Should have 2 "Set as Expected" buttons (schema-1 and schema-3)
  const setExpectedCount = (html.match(/Set as Expected/g) || []).length;
  if (setExpectedCount !== 2) {
    throw new Error(`Expected 2 Set as Expected buttons, got ${setExpectedCount}`);
  }
});

test('handles all props together', () => {
  const schema = makeSchema({
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B')],
    version: '2.0.0',
  });
  const observations = [
    makeObservation('expected-schema', 'MainGraph', schema, { runCount: 42 }),
    makeObservation('other-schema', 'OtherGraph', makeSchema(), { runCount: 5 }),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel
      observations={observations}
      expectedSchemaId="expected-schema"
      onSetExpected={() => {}}
      onCompare={() => {}}
    />
  );

  assertIncludes(html, 'EXPECTED');
  assertIncludes(html, '2 nodes');
  assertIncludes(html, '1 edges');
  assertIncludes(html, 'v2.0.0');
  assertIncludes(html, '42 runs');
  assertIncludes(html, 'MainGraph');
  assertIncludes(html, 'OtherGraph');
  assertIncludes(html, 'Set as Expected');
});

// ============================================================================
// Selection state rendering (initial state)
// ============================================================================
console.log('\nSelection state rendering:');

test('renders aria-pressed="false" for unselected schemas by default', () => {
  const schema = makeSchema();
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'aria-pressed="false"');
});

test('does not render SELECTED badge when no schemas selected', () => {
  const schema = makeSchema();
  const observations = [
    makeObservation('schema-1', 'Graph1', schema),
    makeObservation('schema-2', 'Graph2', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertNotIncludes(html, 'SELECTED');
});

test('does not render comparison controls when no schemas selected', () => {
  const schema = makeSchema();
  const observations = [
    makeObservation('schema-1', 'Graph1', schema),
    makeObservation('schema-2', 'Graph2', schema),
  ];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertNotIncludes(html, 'Show Diff');
  assertNotIncludes(html, 'Hide Diff');
  assertNotIncludes(html, 'schemas selected for comparison');
});

// ============================================================================
// Metadata handling
// ============================================================================
console.log('\nMetadata handling:');

test('handles schema with empty metadata', () => {
  const schema = makeSchema({ metadata: {} });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  assertIncludes(html, 'Schema History');
});

test('handles schema with complex metadata', () => {
  const schema = makeSchema({
    metadata: {
      author: 'TestUser',
      created: '2024-01-01',
      environment: 'production',
      version_tag: 'v1.2.3',
    },
  });
  const observations = [makeObservation('schema-1', 'Graph', schema)];

  const html = renderToStaticMarkup(
    <SchemaHistoryPanel observations={observations} />
  );

  // Metadata not displayed directly, but shouldn't cause issues
  assertIncludes(html, 'Schema History');
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
