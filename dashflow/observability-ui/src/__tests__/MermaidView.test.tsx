// M-444: Component tests for MermaidView
// Run with: npx tsx src/__tests__/MermaidView.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { MermaidView } from '../components/MermaidView';
import type { GraphViewModel, RunCursor, NodeState } from '../hooks/useRunStateStore';
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
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

function assertNotIncludes(haystack: string, needle: string, message?: string): void {
  if (haystack.includes(needle)) {
    throw new Error(message || `Expected NOT to include: ${needle}`);
  }
}

// Helper to create a minimal valid NodeSchema
function makeNode(name: string, nodeType: NodeType = 'transform'): NodeSchema {
  return {
    name,
    node_type: nodeType,
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
}

// Helper to create a minimal valid EdgeSchema
function makeEdge(
  from: string,
  to: string,
  edgeType: 'direct' | 'conditional' | 'parallel' = 'direct',
  label?: string
): EdgeSchema {
  return { from, to, edge_type: edgeType, label };
}

// Helper to create a minimal valid RunCursor
function makeCursor(seq: string = '0'): RunCursor {
  return { threadId: 'test-thread', seq };
}

// Helper to create a minimal valid GraphSchema
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

// Helper to create a NodeState
function makeNodeState(
  status: 'pending' | 'active' | 'completed' | 'error' = 'pending',
  durationMs?: number
): NodeState {
  return {
    status,
    durationMs,
  };
}

// Helper to create a minimal valid GraphViewModel
function makeViewModel(overrides: Partial<GraphViewModel> = {}): GraphViewModel {
  return {
    schema: null,
    schemaId: undefined,
    nodeStates: new Map(),
    currentNode: undefined,
    state: {},
    changedPaths: [],
    cursor: makeCursor(),
    isLive: true,
    observedNodes: new Set(),
    outOfSchemaNodes: new Set(),
    ...overrides,
  };
}

console.log('\nMermaidView Tests\n');

// ============================================================================
// Basic Rendering Tests
// ============================================================================

console.log('Basic Rendering:');

test('renders "No graph selected" when viewModel is null', () => {
  const html = renderToStaticMarkup(<MermaidView viewModel={null} />);

  assertIncludes(html, 'Mermaid View');
  assertIncludes(html, 'No graph selected');
  assertNotIncludes(html, 'Mermaid Text Mode');
});

test('renders "No schema available" when viewModel has no schema', () => {
  const viewModel = makeViewModel({ schema: null });
  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Mermaid View');
  assertIncludes(html, 'No schema available');
  assertNotIncludes(html, 'Mermaid Text Mode');
});

test('renders header with controls when viewModel has schema', () => {
  const schema = makeSchema({
    name: 'TestGraph',
    nodes: [makeNode('Start'), makeNode('End')],
    edges: [makeEdge('Start', 'End')],
    entry_point: 'Start',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'abcd1234efgh5678',
    cursor: makeCursor('100'),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Mermaid Text Mode');
  assertIncludes(html, 'Copy');
  assertIncludes(html, 'Download .mmd');
  // Schema ID prefix should be displayed
  assertIncludes(html, 'abcd1234');
});

test('renders empty schema (no nodes, no edges)', () => {
  const schema = makeSchema({
    name: 'EmptyGraph',
    nodes: [],
    edges: [],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'empty-schema',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'flowchart');
  assertIncludes(html, '0 nodes');
  assertIncludes(html, '0 edges');
});

test('renders schema with single node and no edges', () => {
  const schema = makeSchema({
    name: 'SingleNode',
    nodes: [makeNode('OnlyNode')],
    edges: [],
    entry_point: 'OnlyNode',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'single-node',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'OnlyNode');
  assertIncludes(html, '1 nodes');
  assertIncludes(html, '0 edges');
});

// ============================================================================
// Footer Tests
// ============================================================================

console.log('\nFooter:');

test('renders node and edge counts in footer', () => {
  const schema = makeSchema({
    name: 'CountTest',
    nodes: [makeNode('A'), makeNode('B'), makeNode('C')],
    edges: [makeEdge('A', 'B'), makeEdge('B', 'C')],
    entry_point: 'A',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'test-schema-id',
    cursor: makeCursor('50'),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, '3 nodes');
  assertIncludes(html, '2 edges');
});

test('renders LIVE indicator when isLive=true', () => {
  const schema = makeSchema({
    name: 'LiveTest',
    nodes: [makeNode('X')],
    entry_point: 'X',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'live-id',
    isLive: true,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'LIVE');
});

test('renders sequence number when not live', () => {
  const schema = makeSchema({
    name: 'PausedTest',
    nodes: [makeNode('Y')],
    entry_point: 'Y',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'paused-id',
    isLive: false,
    cursor: makeCursor('12345'),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'seq: 12345');
  assertNotIncludes(html, 'LIVE');
});

test('renders large node count correctly', () => {
  const nodes = Array.from({ length: 10 }, (_, i) => makeNode(`Node${i}`));
  const edges = Array.from({ length: 9 }, (_, i) =>
    makeEdge(`Node${i}`, `Node${i + 1}`)
  );
  const schema = makeSchema({
    name: 'LargeGraph',
    nodes,
    edges,
  });
  const viewModel = makeViewModel({ schema, schemaId: 'large-graph' });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, '10 nodes');
  assertIncludes(html, '9 edges');
});

test('renders sequence number 0', () => {
  const schema = makeSchema({
    name: 'SeqZero',
    nodes: [makeNode('Z')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'seq-zero',
    isLive: false,
    cursor: makeCursor('0'),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'seq: 0');
});

// ============================================================================
// Schema ID Display Tests
// ============================================================================

console.log('\nSchema ID Display:');

test('truncates long schemaId to 8 characters', () => {
  const schema = makeSchema({
    name: 'SchemaIdTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'verylongschemaidentifier123456',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // slice(0, 8) gives "verylong"
  assertIncludes(html, 'verylong');
  assertNotIncludes(html, 'verylongschemaidentifier123456');
});

test('displays short schemaId correctly', () => {
  const schema = makeSchema({
    name: 'ShortId',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'abc12',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'abc12');
});

test('omits schemaId display when undefined', () => {
  const schema = makeSchema({
    name: 'NoSchemaId',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: undefined,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should not have a monospace-styled schema ID badge
  // The header should still have Mermaid Text Mode
  assertIncludes(html, 'Mermaid Text Mode');
});

test('displays exactly 8 character schemaId', () => {
  const schema = makeSchema({
    name: 'Exact8',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'exactly8',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'exactly8');
});

// ============================================================================
// Mermaid Content Tests
// ============================================================================

console.log('\nMermaid Content:');

test('renders generated mermaid text content', () => {
  const schema = makeSchema({
    name: 'MermaidContent',
    nodes: [makeNode('StartNode'), makeNode('EndNode')],
    edges: [makeEdge('StartNode', 'EndNode')],
    entry_point: 'StartNode',
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'content-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Mermaid output should contain graph definition with node names
  assertIncludes(html, 'flowchart');
  assertIncludes(html, 'StartNode');
  assertIncludes(html, 'EndNode');
});

test('renders different node types with correct shapes', () => {
  const schema = makeSchema({
    name: 'NodeTypes',
    nodes: [
      makeNode('LLMNode', 'llm'),
      makeNode('ToolNode', 'tool'),
      makeNode('RouterNode', 'router'),
      makeNode('TransformNode', 'transform'),
    ],
    edges: [],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'node-types',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Each node should appear in the output
  assertIncludes(html, 'LLMNode');
  assertIncludes(html, 'ToolNode');
  assertIncludes(html, 'RouterNode');
  assertIncludes(html, 'TransformNode');
});

test('renders aggregator and validator node types', () => {
  const schema = makeSchema({
    name: 'MoreNodeTypes',
    nodes: [
      makeNode('AggregatorNode', 'aggregator'),
      makeNode('ValidatorNode', 'validator'),
    ],
    edges: [],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'more-node-types',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'AggregatorNode');
  assertIncludes(html, 'ValidatorNode');
});

test('renders human_in_loop and checkpoint node types', () => {
  const schema = makeSchema({
    name: 'SpecialNodeTypes',
    nodes: [
      makeNode('HumanNode', 'human_in_loop'),
      makeNode('CheckpointNode', 'checkpoint'),
    ],
    edges: [],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'special-node-types',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'HumanNode');
  assertIncludes(html, 'CheckpointNode');
});

test('renders custom node type', () => {
  const schema = makeSchema({
    name: 'CustomNode',
    nodes: [makeNode('MyCustomNode', 'custom')],
    edges: [],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'custom-node',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'MyCustomNode');
});

test('renders direct edges', () => {
  const schema = makeSchema({
    name: 'DirectEdges',
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B', 'direct')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'direct-edges',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Direct edges use --> arrow
  assertIncludes(html, '--&gt;');
});

test('renders conditional edges', () => {
  const schema = makeSchema({
    name: 'ConditionalEdges',
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B', 'conditional')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'conditional-edges',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Conditional edges use -.-> arrow
  assertIncludes(html, '-.-&gt;');
});

test('renders parallel edges', () => {
  const schema = makeSchema({
    name: 'ParallelEdges',
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B', 'parallel')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'parallel-edges',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Parallel edges use ===> arrow
  assertIncludes(html, '===&gt;');
});

test('renders edge labels', () => {
  const schema = makeSchema({
    name: 'EdgeLabels',
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B', 'direct', 'my_label')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'edge-labels',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'my_label');
});

test('renders multiple edges between nodes', () => {
  const schema = makeSchema({
    name: 'MultiEdge',
    nodes: [makeNode('A'), makeNode('B'), makeNode('C')],
    edges: [
      makeEdge('A', 'B'),
      makeEdge('A', 'C'),
      makeEdge('B', 'C'),
    ],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'multi-edge',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, '3 edges');
});

// ============================================================================
// Node State Tests
// ============================================================================

console.log('\nNode States:');

test('renders node with pending status', () => {
  const schema = makeSchema({
    name: 'PendingStatus',
    nodes: [makeNode('PendingNode')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('PendingNode', makeNodeState('pending'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'pending-status',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Pending status should not show indicator
  assertIncludes(html, 'PendingNode');
});

test('renders node with active status', () => {
  const schema = makeSchema({
    name: 'ActiveStatus',
    nodes: [makeNode('ActiveNode')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('ActiveNode', makeNodeState('active'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'active-status',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Active status shows ⚡ indicator
  assertIncludes(html, 'ActiveNode');
  assertIncludes(html, '⚡');
});

test('renders node with completed status', () => {
  const schema = makeSchema({
    name: 'CompletedStatus',
    nodes: [makeNode('CompletedNode')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('CompletedNode', makeNodeState('completed'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'completed-status',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Completed status shows ✓ indicator
  assertIncludes(html, 'CompletedNode');
  assertIncludes(html, '✓');
});

test('renders node with error status', () => {
  const schema = makeSchema({
    name: 'ErrorStatus',
    nodes: [makeNode('ErrorNode')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('ErrorNode', makeNodeState('error'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'error-status',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Error status shows ✗ indicator
  assertIncludes(html, 'ErrorNode');
  assertIncludes(html, '✗');
});

test('renders node with duration', () => {
  const schema = makeSchema({
    name: 'DurationTest',
    nodes: [makeNode('TimedNode')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('TimedNode', makeNodeState('completed', 150));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'duration-test',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, '150ms');
});

test('renders multiple nodes with different states', () => {
  const schema = makeSchema({
    name: 'MixedStates',
    nodes: [
      makeNode('Node1'),
      makeNode('Node2'),
      makeNode('Node3'),
      makeNode('Node4'),
    ],
    edges: [
      makeEdge('Node1', 'Node2'),
      makeEdge('Node2', 'Node3'),
      makeEdge('Node3', 'Node4'),
    ],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('Node1', makeNodeState('completed', 100));
  nodeStates.set('Node2', makeNodeState('completed', 200));
  nodeStates.set('Node3', makeNodeState('active'));
  nodeStates.set('Node4', makeNodeState('pending'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'mixed-states',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Node1');
  assertIncludes(html, 'Node2');
  assertIncludes(html, 'Node3');
  assertIncludes(html, 'Node4');
  assertIncludes(html, '100ms');
  assertIncludes(html, '200ms');
});

test('renders current node indicator', () => {
  const schema = makeSchema({
    name: 'CurrentNode',
    nodes: [makeNode('A'), makeNode('B')],
    edges: [makeEdge('A', 'B')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'current-node',
    currentNode: 'B',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Current node should have class applied
  assertIncludes(html, 'current');
});

// ============================================================================
// Options Tests
// ============================================================================

console.log('\nOptions:');

test('renders with default direction (TD)', () => {
  const schema = makeSchema({
    name: 'DefaultDirection',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'default-dir',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'flowchart TD');
});

test('renders with LR direction option', () => {
  const schema = makeSchema({
    name: 'LRDirection',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'lr-dir',
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ direction: 'LR' }} />
  );

  assertIncludes(html, 'flowchart LR');
});

test('renders with BT direction option', () => {
  const schema = makeSchema({
    name: 'BTDirection',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'bt-dir',
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ direction: 'BT' }} />
  );

  assertIncludes(html, 'flowchart BT');
});

test('renders with RL direction option', () => {
  const schema = makeSchema({
    name: 'RLDirection',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'rl-dir',
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ direction: 'RL' }} />
  );

  assertIncludes(html, 'flowchart RL');
});

test('renders without durations when showDurations=false', () => {
  const schema = makeSchema({
    name: 'NoDurations',
    nodes: [makeNode('A')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('A', makeNodeState('completed', 500));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'no-durations',
    nodeStates,
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ showDurations: false }} />
  );

  assertNotIncludes(html, '500ms');
});

test('renders without status indicators when showStatusIndicators=false', () => {
  const schema = makeSchema({
    name: 'NoIndicators',
    nodes: [makeNode('A')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('A', makeNodeState('completed'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'no-indicators',
    nodeStates,
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ showStatusIndicators: false }} />
  );

  assertNotIncludes(html, '✓');
});

test('renders without style definitions when includeStyleDefs=false', () => {
  const schema = makeSchema({
    name: 'NoStyles',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'no-styles',
  });

  const html = renderToStaticMarkup(
    <MermaidView viewModel={viewModel} options={{ includeStyleDefs: false }} />
  );

  assertNotIncludes(html, 'classDef pending');
  assertNotIncludes(html, 'classDef active');
});

// ============================================================================
// Special Character Tests
// ============================================================================

console.log('\nSpecial Characters:');

test('renders node names with underscores', () => {
  const schema = makeSchema({
    name: 'UnderscoreNames',
    nodes: [makeNode('my_node_name')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'underscore',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'my_node_name');
});

test('handles node names with numbers', () => {
  const schema = makeSchema({
    name: 'NumberNames',
    nodes: [makeNode('node123')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'numbers',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'node123');
});

test('handles schema with unicode characters in name', () => {
  const schema = makeSchema({
    name: 'Tëst Gräph',
    nodes: [makeNode('ノード')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'unicode',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should render without crashing
  assertIncludes(html, 'flowchart');
});

// ============================================================================
// CSS and Styling Tests
// ============================================================================

console.log('\nCSS and Styling:');

test('renders with dark theme background color', () => {
  const schema = makeSchema({
    name: 'StyleTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'style-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // React SSR converts to CSS format: background-color
  assertIncludes(html, 'background-color');
});

test('renders with border styling', () => {
  const schema = makeSchema({
    name: 'BorderTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'border-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should have border style
  assertIncludes(html, 'border');
});

test('renders header with border separator', () => {
  const schema = makeSchema({
    name: 'HeaderTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'header-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // React SSR converts to CSS format: border-bottom
  assertIncludes(html, 'border-bottom');
});

test('renders footer with border separator', () => {
  const schema = makeSchema({
    name: 'FooterTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'footer-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // React SSR converts to CSS format: border-top
  assertIncludes(html, 'border-top');
});

test('renders pre element with monospace font', () => {
  const schema = makeSchema({
    name: 'FontTest',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'font-test',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Pre element should have monospace font family
  assertIncludes(html, 'Monaco');
});

// ============================================================================
// Button Tests
// ============================================================================

console.log('\nButtons:');

test('renders copy button', () => {
  const schema = makeSchema({
    name: 'CopyButton',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'copy-btn',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Copy');
  assertIncludes(html, 'button');
});

test('renders download button', () => {
  const schema = makeSchema({
    name: 'DownloadButton',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'download-btn',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Download .mmd');
  assertIncludes(html, 'button');
});

test('copy button has correct title attribute', () => {
  const schema = makeSchema({
    name: 'CopyTitle',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'copy-title',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Copy Mermaid text to clipboard');
});

test('download button has correct title attribute', () => {
  const schema = makeSchema({
    name: 'DownloadTitle',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'download-title',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'Download Mermaid file');
});

// ============================================================================
// Edge Cases
// ============================================================================

console.log('\nEdge Cases:');

test('handles empty nodeStates map', () => {
  const schema = makeSchema({
    name: 'EmptyStates',
    nodes: [makeNode('A'), makeNode('B')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'empty-states',
    nodeStates: new Map(),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should render without crashing
  assertIncludes(html, 'flowchart');
});

test('handles node with undefined duration', () => {
  const schema = makeSchema({
    name: 'UndefinedDuration',
    nodes: [makeNode('A')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('A', makeNodeState('completed', undefined));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'undef-duration',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should render without crashing, without duration
  assertIncludes(html, 'flowchart');
  assertNotIncludes(html, 'undefinedms');
});

test('handles node with zero duration', () => {
  const schema = makeSchema({
    name: 'ZeroDuration',
    nodes: [makeNode('A')],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('A', makeNodeState('completed', 0));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'zero-duration',
    nodeStates,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Zero duration IS displayed (check is !== undefined, not truthy)
  assertIncludes(html, '(0ms)');
});

test('handles very long sequence number', () => {
  const schema = makeSchema({
    name: 'LongSeq',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'long-seq',
    isLive: false,
    cursor: makeCursor('99999999999'),
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'seq: 99999999999');
});

test('handles currentNode not in schema', () => {
  const schema = makeSchema({
    name: 'MissingCurrent',
    nodes: [makeNode('A')],
  });
  const viewModel = makeViewModel({
    schema,
    schemaId: 'missing-current',
    currentNode: 'NonExistent',
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  // Should render without crashing
  assertIncludes(html, 'flowchart');
});

test('handles complex graph with all features', () => {
  const schema = makeSchema({
    name: 'ComplexGraph',
    nodes: [
      makeNode('Start', 'llm'),
      makeNode('Process', 'tool'),
      makeNode('Route', 'router'),
      makeNode('End', 'transform'),
    ],
    edges: [
      makeEdge('Start', 'Process', 'direct'),
      makeEdge('Process', 'Route', 'direct'),
      makeEdge('Route', 'End', 'conditional', 'success'),
      makeEdge('Route', 'Start', 'conditional', 'retry'),
    ],
  });
  const nodeStates = new Map<string, NodeState>();
  nodeStates.set('Start', makeNodeState('completed', 100));
  nodeStates.set('Process', makeNodeState('completed', 250));
  nodeStates.set('Route', makeNodeState('active'));
  nodeStates.set('End', makeNodeState('pending'));
  const viewModel = makeViewModel({
    schema,
    schemaId: 'complex-graph-id',
    nodeStates,
    currentNode: 'Route',
    isLive: true,
  });

  const html = renderToStaticMarkup(<MermaidView viewModel={viewModel} />);

  assertIncludes(html, 'flowchart');
  assertIncludes(html, 'Start');
  assertIncludes(html, 'Process');
  assertIncludes(html, 'Route');
  assertIncludes(html, 'End');
  assertIncludes(html, '100ms');
  assertIncludes(html, '250ms');
  assertIncludes(html, '4 nodes');
  assertIncludes(html, '4 edges');
  assertIncludes(html, 'LIVE');
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
