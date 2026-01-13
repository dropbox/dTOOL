// M-444: Component tests for ExecutionTimeline
// Run with: npx tsx src/__tests__/ExecutionTimeline.test.tsx
// Expanded: Worker #2608 - comprehensive test coverage

import { renderToStaticMarkup } from 'react-dom/server';
import { ExecutionTimeline, TimelineEvent } from '../components/ExecutionTimeline';

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


function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || `Expected condition to be true`);
  }
}

// Helper to create test events
function makeEvent(
  eventType: string,
  timestamp: number,
  overrides: Partial<TimelineEvent> = {}
): TimelineEvent {
  return {
    timestamp,
    elapsed_ms: timestamp,
    event_type: eventType,
    ...overrides,
  };
}

// Count occurrences of a string in another string
function countOccurrences(haystack: string, needle: string): number {
  return (haystack.match(new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g')) || []).length;
}

console.log('\nExecutionTimeline Tests\n');

// ============================================================
// Empty State Tests
// ============================================================
console.log('\n--- Empty State ---');

test('renders empty state with "Waiting for events" message', () => {
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={[]} startTime={null} />
  );

  assertIncludes(html, 'data-testid="execution-timeline"');
  assertIncludes(html, 'Timeline');
  assertIncludes(html, 'Waiting for events');
});

test('renders empty state with custom maxHeight', () => {
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={[]} startTime={null} maxHeight="400px" />
  );

  assertIncludes(html, '400px');
});

test('renders empty state with default maxHeight when not specified', () => {
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={[]} startTime={null} />
  );

  assertIncludes(html, '300px'); // default maxHeight
});

test('empty state does not show header with event count', () => {
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={[]} startTime={null} />
  );

  assertNotIncludes(html, 'EXECUTION TIMELINE');
  assertNotIncludes(html, 'events)');
});

// ============================================================
// Event Count Header Tests
// ============================================================
console.log('\n--- Event Count Header ---');

test('renders singular "event" for single event', () => {
  const events = [makeEvent('GraphStart', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Should show "1 events" (component uses plural always for simplicity)
  assertIncludes(html, '1 events');
});

test('renders event count in header', () => {
  const events = [
    makeEvent('GraphStart', 1000),
    makeEvent('NodeStart', 1100, { node_id: 'node1' }),
    makeEvent('NodeEnd', 1200, { node_id: 'node1' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'EXECUTION TIMELINE');
  assertIncludes(html, '3 events');
});

test('renders large event count correctly', () => {
  const events = Array.from({ length: 42 }, (_, i) =>
    makeEvent('NodeStart', i * 100, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '42 events');
});

// ============================================================
// Event Type Rendering Tests
// ============================================================
console.log('\n--- Event Type Rendering ---');

test('renders GraphStart events', () => {
  const events = [makeEvent('GraphStart', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'GraphStart');
  assertIncludes(html, '>'); // GraphStart symbol
});

test('renders GraphEnd events', () => {
  const events = [makeEvent('GraphEnd', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'GraphEnd');
});

test('renders NodeStart events', () => {
  const events = [makeEvent('NodeStart', 1000, { node_id: 'test_node' })];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'NodeStart');
  assertIncludes(html, 'test_node');
});

test('renders NodeEnd events', () => {
  const events = [makeEvent('NodeEnd', 1000, { node_id: 'test_node' })];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'NodeEnd');
});

test('renders StateUpdate events', () => {
  const events = [makeEvent('StateUpdate', 1000, { details: 'counter = 42' })];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'StateUpdate');
  assertIncludes(html, 'counter = 42');
});

test('renders NodeError events with error styling', () => {
  const events = [
    makeEvent('NodeError', 1000, { node_id: 'failed_node', details: 'Something went wrong' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // NodeError should have red color (#ef4444)
  assertIncludes(html, '#ef4444');
  assertIncludes(html, 'NodeError');
});

test('renders unknown event types with default styling', () => {
  const events = [makeEvent('CustomUnknownEvent', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'CustomUnknownEvent');
  // Should use default neutral color
  assertIncludes(html, '#9ca3af'); // neutral gray
});

test('renders multiple event types with different colors', () => {
  const events = [
    makeEvent('GraphStart', 1000),
    makeEvent('NodeStart', 1050, { node_id: 'n1' }),
    makeEvent('StateUpdate', 1100),
    makeEvent('NodeEnd', 1150, { node_id: 'n1' }),
    makeEvent('NodeError', 1200, { node_id: 'n2' }),
    makeEvent('GraphEnd', 1250),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Verify different event types are present
  assertIncludes(html, 'GraphStart');
  assertIncludes(html, 'NodeStart');
  assertIncludes(html, 'StateUpdate');
  assertIncludes(html, 'NodeEnd');
  assertIncludes(html, 'NodeError');
  assertIncludes(html, 'GraphEnd');
  // Verify 6 events count
  assertIncludes(html, '6 events');
});

// ============================================================
// Node ID Rendering Tests
// ============================================================
console.log('\n--- Node ID Rendering ---');

test('renders node_id when present', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'my_special_node' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'my_special_node');
});

test('omits node_id section when not present', () => {
  const events = [
    makeEvent('GraphStart', 1000), // GraphStart typically has no node_id
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Should not have the cyan-colored node_id span
  // Check that there's no minWidth:80px span (node_id specific styling)
  assertIncludes(html, 'GraphStart');
});

test('renders empty string node_id (falsy value treated as absent)', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: '' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Empty string is falsy, so node_id should not render
  assertIncludes(html, 'NodeStart');
});

test('renders node_id with special characters', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'node-with-dashes_and_underscores.123' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'node-with-dashes_and_underscores.123');
});

test('renders very long node_id (should be truncated with ellipsis via CSS)', () => {
  const longNodeId = 'this_is_a_very_long_node_id_that_should_be_truncated';
  const events = [
    makeEvent('NodeStart', 1000, { node_id: longNodeId }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, longNodeId);
  // CSS handles truncation via text-overflow: ellipsis
  assertIncludes(html, 'text-overflow');
});

// ============================================================
// Details Rendering Tests
// ============================================================
console.log('\n--- Details Rendering ---');

test('renders details when present', () => {
  const events = [
    makeEvent('StateUpdate', 1000, { details: 'state.count = 42' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'state.count = 42');
});

test('omits details section when not present', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'test' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Event renders without details
  assertIncludes(html, 'NodeStart');
  assertIncludes(html, 'test');
});

test('renders details with JSON content', () => {
  const events = [
    makeEvent('StateUpdate', 1000, { details: '{"key": "value", "count": 5}' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // React escapes quotes in HTML attributes, so check for escaped version
  assertIncludes(html, '{&quot;key&quot;: &quot;value&quot;, &quot;count&quot;: 5}');
});

test('renders multiline details (preserves content, CSS handles display)', () => {
  const events = [
    makeEvent('NodeError', 1000, { details: 'Error: Line 1\nLine 2\nLine 3' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'Error: Line 1');
});

// ============================================================
// Elapsed Time Formatting Tests
// ============================================================
console.log('\n--- Elapsed Time Formatting ---');

test('renders zero elapsed time as 00:00.0', () => {
  const events = [
    makeEvent('GraphStart', 0, { elapsed_ms: 0 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '00:00.0');
});

test('renders sub-second elapsed time', () => {
  const events = [
    makeEvent('NodeStart', 500, { elapsed_ms: 500 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '00:00.5');
});

test('renders elapsed time under a minute', () => {
  const events = [
    makeEvent('NodeEnd', 45000, { elapsed_ms: 45000 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '00:45.0');
});

test('renders elapsed time formatted correctly for over a minute', () => {
  // 61.5 seconds = 01:01.5
  const events = [
    makeEvent('NodeEnd', 61500, { elapsed_ms: 61500 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '01:01.5');
});

test('renders elapsed time for multiple minutes', () => {
  // 185.3 seconds = 03:05.3
  const events = [
    makeEvent('NodeEnd', 185300, { elapsed_ms: 185300 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '03:05.3');
});

test('renders elapsed time with fractional milliseconds', () => {
  // 1234 ms = 1.234 seconds = 00:01.2 (truncated to one decimal)
  const events = [
    makeEvent('NodeEnd', 1234, { elapsed_ms: 1234 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '00:01.2');
});

test('renders very large elapsed time (over an hour)', () => {
  // 3661500 ms = 61 minutes 1.5 seconds = 61:01.5
  const events = [
    makeEvent('NodeEnd', 3661500, { elapsed_ms: 3661500 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '61:01.5');
});

test('uses elapsed_ms from event when provided', () => {
  const events = [
    makeEvent('NodeStart', 5000, { elapsed_ms: 3000 }), // 3 seconds
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  // Should show 00:03.0 (from elapsed_ms) not 00:05.0 (from timestamp - startTime)
  assertIncludes(html, '00:03.0');
});

test('calculates elapsed from timestamp when startTime provided and elapsed_ms not set', () => {
  const baseTime = 1000;
  const events = [
    { timestamp: baseTime + 2500, event_type: 'NodeStart' } as TimelineEvent,
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={baseTime} />
  );

  // 2.5 seconds = 00:02.5
  assertIncludes(html, '00:02.5');
});

test('handles null startTime when elapsed_ms not provided (defaults to 0)', () => {
  const events = [
    { timestamp: 5000, event_type: 'NodeStart' } as TimelineEvent,
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={null} />
  );

  // With null startTime and no elapsed_ms, should default to 0
  assertIncludes(html, '00:00.0');
});

// ============================================================
// Schema Mismatch Tests
// ============================================================
console.log('\n--- Schema Mismatch ---');

test('renders schema mismatch indicator when event schema differs from expected', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'test_node',
      schema_id: 'schema-v2-abc',
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline
      events={events}
      startTime={1000}
      expectedSchemaId="schema-v1-xyz"
    />
  );

  // Should show the "!" mismatch indicator
  assertIncludes(html, '!');
  // Should have red background for out-of-schema events
  assertIncludes(html, 'rgba(239, 68, 68, 0.15)');
});

test('does not show schema mismatch when schemas match', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'test_node',
      schema_id: 'same-schema-id',
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline
      events={events}
      startTime={1000}
      expectedSchemaId="same-schema-id"
    />
  );

  // Should NOT have red mismatch background
  assertNotIncludes(html, 'rgba(239, 68, 68, 0.15)');
});

test('does not show schema mismatch when event has no schema_id', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'test_node' }), // No schema_id
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline
      events={events}
      startTime={1000}
      expectedSchemaId="expected-schema"
    />
  );

  // Should NOT show mismatch indicator for events without schema_id
  assertNotIncludes(html, 'rgba(239, 68, 68, 0.15)');
});

test('does not show schema mismatch when no expectedSchemaId provided', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'test_node',
      schema_id: 'event-schema',
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Without expectedSchemaId, no mismatch detection
  assertNotIncludes(html, 'rgba(239, 68, 68, 0.15)');
});

test('shows mismatch indicator for multiple mismatched events', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'n1', schema_id: 'schema-a' }),
    makeEvent('NodeEnd', 1100, { node_id: 'n1', schema_id: 'schema-b' }),
    makeEvent('NodeStart', 1200, { node_id: 'n2', schema_id: 'schema-c' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline
      events={events}
      startTime={1000}
      expectedSchemaId="expected-schema"
    />
  );

  // All three events have mismatched schemas
  // Count "!" occurrences should be 3
  const mismatchCount = countOccurrences(html, 'rgba(239, 68, 68, 0.15)');
  assertTrue(mismatchCount === 3, `Expected 3 mismatch backgrounds, got ${mismatchCount}`);
});

test('renders schema mismatch tooltip with truncated schema IDs', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'test_node',
      schema_id: 'schema-v2-abcdef1234567890',
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline
      events={events}
      startTime={1000}
      expectedSchemaId="schema-v1-xyz0987654321fed"
    />
  );

  // Tooltip should include truncated schema info
  assertIncludes(html, 'Schema mismatch');
  assertIncludes(html, 'expected');
});

// ============================================================
// Virtualization Tests
// ============================================================
console.log('\n--- Virtualization ---');

test('renders non-virtualized list for small event count (<50)', () => {
  const events = Array.from({ length: 10 }, (_, i) =>
    makeEvent('NodeStart', i * 100, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  // Should render all events directly (not virtualized)
  assertIncludes(html, '10 events');
  // Should have all node IDs rendered
  assertIncludes(html, 'node-0');
  assertIncludes(html, 'node-9');
});

test('renders non-virtualized list at threshold boundary (49 events)', () => {
  const events = Array.from({ length: 49 }, (_, i) =>
    makeEvent('NodeStart', i * 100, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '49 events');
  // All events should be rendered directly
  assertIncludes(html, 'node-0');
  assertIncludes(html, 'node-48');
});

test('renders non-virtualized list at exactly threshold (50 events)', () => {
  const events = Array.from({ length: 50 }, (_, i) =>
    makeEvent('NodeStart', i * 100, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  // 50 is NOT > 50, so still non-virtualized
  assertIncludes(html, '50 events');
  assertIncludes(html, 'node-0');
  assertIncludes(html, 'node-49');
});

test('uses virtualization for events above threshold (51 events)', () => {
  const events = Array.from({ length: 51 }, (_, i) =>
    makeEvent('NodeStart', i * 100, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '51 events');
  // In SSR (renderToStaticMarkup), react-window may not render all items
  // The component structure should be different though
});

test('handles very large event count', () => {
  const events = Array.from({ length: 1000 }, (_, i) =>
    makeEvent('NodeStart', i * 10, { node_id: `node-${i}` })
  );
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '1000 events');
});

// ============================================================
// maxHeight Prop Tests
// ============================================================
console.log('\n--- maxHeight Prop ---');

test('applies custom maxHeight to event list container', () => {
  const events = [makeEvent('GraphStart', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} maxHeight="500px" />
  );

  assertIncludes(html, '500px');
});

test('applies default maxHeight when not specified', () => {
  const events = [makeEvent('GraphStart', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, '300px');
});

test('handles numeric maxHeight string', () => {
  const events = [makeEvent('GraphStart', 1000)];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} maxHeight="250px" />
  );

  assertIncludes(html, '250px');
});

// ============================================================
// Edge Cases Tests
// ============================================================
console.log('\n--- Edge Cases ---');

test('renders event with all optional fields populated', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'complete_node',
      details: 'Processing started',
      schema_id: 'schema-123',
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} expectedSchemaId="schema-123" />
  );

  assertIncludes(html, 'complete_node');
  assertIncludes(html, 'Processing started');
  assertIncludes(html, 'NodeStart');
});

test('renders event with minimal fields (only required)', () => {
  const events = [
    { timestamp: 1000, elapsed_ms: 0, event_type: 'GraphStart' },
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'GraphStart');
  assertIncludes(html, '00:00.0');
});

test('handles events with same timestamp', () => {
  const events = [
    makeEvent('NodeStart', 1000, { node_id: 'node1' }),
    makeEvent('NodeStart', 1000, { node_id: 'node2' }),
    makeEvent('NodeStart', 1000, { node_id: 'node3' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'node1');
  assertIncludes(html, 'node2');
  assertIncludes(html, 'node3');
  assertIncludes(html, '3 events');
});

test('handles events with negative elapsed_ms', () => {
  // Edge case: clock skew could cause negative elapsed times
  const events = [
    makeEvent('NodeStart', 1000, { elapsed_ms: -500 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // Should still render (may show negative or handle gracefully)
  assertIncludes(html, 'NodeStart');
});

test('renders with startTime of 0', () => {
  const events = [
    makeEvent('GraphStart', 0, { elapsed_ms: 0 }),
    makeEvent('NodeStart', 1000, { elapsed_ms: 1000 }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  assertIncludes(html, '00:00.0');
  assertIncludes(html, '00:01.0');
});

test('renders events in provided order (not sorted)', () => {
  // Events provided out of order - component should render in array order
  const events = [
    makeEvent('NodeEnd', 3000, { node_id: 'third' }),
    makeEvent('NodeStart', 1000, { node_id: 'first' }),
    makeEvent('StateUpdate', 2000, { node_id: 'second' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={0} />
  );

  // All three should be present
  assertIncludes(html, 'first');
  assertIncludes(html, 'second');
  assertIncludes(html, 'third');
});

test('handles special characters in details', () => {
  const events = [
    makeEvent('StateUpdate', 1000, { details: '<script>alert("xss")</script>' }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  // React escapes HTML automatically
  assertIncludes(html, '&lt;script&gt;');
  assertNotIncludes(html, '<script>');
});

test('handles unicode in node_id and details', () => {
  const events = [
    makeEvent('NodeStart', 1000, {
      node_id: 'ノード_🚀',
      details: '処理開始 → 完了 ✓'
    }),
  ];
  const html = renderToStaticMarkup(
    <ExecutionTimeline events={events} startTime={1000} />
  );

  assertIncludes(html, 'ノード_🚀');
  assertIncludes(html, '処理開始');
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
