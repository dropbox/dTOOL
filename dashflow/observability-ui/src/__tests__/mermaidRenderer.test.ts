// Unit tests for Mermaid renderer
// Run with: npx tsx src/__tests__/mermaidRenderer.test.ts

import {
  renderSchemaToMermaid,
  renderViewModelToMermaid,
  renderSchemaStructure,
  createMermaidBlob,
} from '../utils/mermaidRenderer';
import { GraphSchema } from '../types/graph';
import { GraphViewModel, NodeState } from '../hooks/useRunStateStore';

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

function assertContains(str: string, substring: string, message?: string): void {
  if (!str.includes(substring)) {
    throw new Error(`${message || 'Assertion failed'}: expected to contain "${substring}" in:\n${str}`);
  }
}

function assertNotContains(str: string, substring: string, message?: string): void {
  if (str.includes(substring)) {
    throw new Error(`${message || 'Assertion failed'}: expected NOT to contain "${substring}" in:\n${str}`);
  }
}

// Sample schemas for testing
const simpleSchema: GraphSchema = {
  name: 'Simple Graph',
  version: '1.0.0',
  nodes: [
    { name: 'start', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    { name: 'process', node_type: 'llm', input_fields: [], output_fields: [], attributes: {} },
    { name: 'end', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
  ],
  edges: [
    { from: 'start', to: 'process', edge_type: 'direct' },
    { from: 'process', to: 'end', edge_type: 'direct' },
  ],
  entry_point: 'start',
  metadata: {},
};

const complexSchema: GraphSchema = {
  name: 'Complex Graph',
  version: '2.0.0',
  nodes: [
    { name: 'input', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    { name: 'router', node_type: 'router', input_fields: [], output_fields: [], attributes: {} },
    { name: 'llm_a', node_type: 'llm', input_fields: [], output_fields: [], attributes: {} },
    { name: 'llm_b', node_type: 'llm', input_fields: [], output_fields: [], attributes: {} },
    { name: 'aggregator', node_type: 'aggregator', input_fields: [], output_fields: [], attributes: {} },
    { name: 'tool', node_type: 'tool', input_fields: [], output_fields: [], attributes: {} },
    { name: 'output', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
  ],
  edges: [
    { from: 'input', to: 'router', edge_type: 'direct' },
    { from: 'router', to: 'llm_a', edge_type: 'conditional', label: 'path_a' },
    { from: 'router', to: 'llm_b', edge_type: 'conditional', label: 'path_b' },
    { from: 'llm_a', to: 'aggregator', edge_type: 'parallel' },
    { from: 'llm_b', to: 'aggregator', edge_type: 'parallel' },
    { from: 'aggregator', to: 'tool', edge_type: 'direct' },
    { from: 'tool', to: 'output', edge_type: 'direct' },
  ],
  entry_point: 'input',
  metadata: {},
};

// Test suites
console.log('\nMermaid Renderer Tests\n');

console.log('renderSchemaToMermaid - basic rendering:');
test('generates flowchart header', () => {
  const result = renderSchemaToMermaid(simpleSchema);
  assertContains(result, 'flowchart TD');
});

test('generates node definitions', () => {
  const result = renderSchemaToMermaid(simpleSchema);
  assertContains(result, 'start');
  assertContains(result, 'process');
  assertContains(result, 'end');
});

test('generates edge definitions', () => {
  const result = renderSchemaToMermaid(simpleSchema);
  assertContains(result, 'start --> process');
  assertContains(result, 'process --> end');
});

test('includes style definitions by default', () => {
  const result = renderSchemaToMermaid(simpleSchema);
  assertContains(result, 'classDef pending');
  assertContains(result, 'classDef active');
  assertContains(result, 'classDef completed');
  assertContains(result, 'classDef error');
});

console.log('\nrenderSchemaToMermaid - node shapes:');
test('uses stadium shape for llm nodes', () => {
  const result = renderSchemaToMermaid(simpleSchema);
  assertContains(result, 'process([');
  assertContains(result, '])');
});

test('uses diamond shape for router nodes', () => {
  const result = renderSchemaToMermaid(complexSchema);
  assertContains(result, 'router{');
});

test('uses hexagon shape for tool nodes', () => {
  const result = renderSchemaToMermaid(complexSchema);
  assertContains(result, 'tool{{');
});

console.log('\nrenderSchemaToMermaid - edge types:');
test('uses dotted arrow for conditional edges', () => {
  const result = renderSchemaToMermaid(complexSchema);
  assertContains(result, '-.->');
});

test('uses thick arrow for parallel edges', () => {
  const result = renderSchemaToMermaid(complexSchema);
  assertContains(result, '===>');
});

test('includes edge labels', () => {
  const result = renderSchemaToMermaid(complexSchema);
  assertContains(result, '|"path_a"|');
  assertContains(result, '|"path_b"|');
});

console.log('\nrenderSchemaToMermaid - with node states:');
test('applies status classes to nodes', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed' }],
    ['process', { status: 'active' }],
    ['end', { status: 'pending' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, 'class start completed');
  assertContains(result, 'class process active');
  assertContains(result, 'class end pending');
});

test('adds status indicators to labels', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed' }],
    ['process', { status: 'active' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, '✓'); // completed indicator
  assertContains(result, '⚡'); // active indicator
});

test('shows duration when available', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 150 }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, '(150ms)');
});

console.log('\nrenderSchemaToMermaid - with current node:');
test('marks current node with special class', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, 'process');
  assertContains(result, 'class process current');
});

console.log('\nrenderSchemaToMermaid - options:');
test('changes direction with option', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, undefined, { direction: 'LR' });
  assertContains(result, 'flowchart LR');
});

test('hides durations with option', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 150 }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates, undefined, { showDurations: false });
  assertNotContains(result, '(150ms)');
});

test('hides status indicators with option', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates, undefined, { showStatusIndicators: false });
  assertNotContains(result, '✓');
});

test('omits style definitions with option', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, undefined, { includeStyleDefs: false });
  assertNotContains(result, 'classDef pending');
});

console.log('\nrenderSchemaStructure - minimal output:');
test('generates minimal diagram', () => {
  const result = renderSchemaStructure(simpleSchema);
  assertContains(result, 'flowchart TD');
  assertNotContains(result, 'classDef'); // No style defs
  assertNotContains(result, '✓'); // No status indicators
});

console.log('\nrenderViewModelToMermaid - view model rendering:');
test('renders from view model', () => {
  const viewModel: GraphViewModel = {
    schema: simpleSchema,
    schemaId: 'abc123def456',
    nodeStates: new Map([
      ['start', { status: 'completed' }],
      ['process', { status: 'active' }],
    ]),
    currentNode: 'process',
    state: { count: 1 },
    changedPaths: ['/count'],
    cursor: { threadId: 'test-thread', seq: '10' },
    isLive: true,
    observedNodes: new Set(['start', 'process']),
    outOfSchemaNodes: new Set(),
  };

  const result = renderViewModelToMermaid(viewModel);
  assertContains(result!, 'flowchart TD');
  assertContains(result!, 'start');
  assertContains(result!, 'process');
  assertContains(result!, 'class process current');
  assertContains(result!, 'class start completed');
  assertContains(result!, 'class process active');
});

test('returns null for null view model', () => {
  const result = renderViewModelToMermaid(null as unknown as GraphViewModel);
  if (result !== null) {
    throw new Error('Expected null result');
  }
});

test('returns null when schema is null', () => {
  const viewModel: GraphViewModel = {
    schema: null,
    nodeStates: new Map(),
    state: {},
    changedPaths: [],
    cursor: { threadId: 'test', seq: '1' },
    isLive: true,
    observedNodes: new Set(),
    outOfSchemaNodes: new Set(),
  };
  const result = renderViewModelToMermaid(viewModel);
  if (result !== null) {
    throw new Error('Expected null result');
  }
});

console.log('\nEscape handling:');
test('escapes special characters in node names', () => {
  const schemaWithSpecialChars: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node<with>brackets', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node<with>brackets',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schemaWithSpecialChars);
  // M-454: Now uses numeric entity codes (#60; for <, #62; for >)
  assertContains(result, '#60;'); // <
  assertContains(result, '#62;'); // >
});

test('generates safe node IDs', () => {
  const schemaWithSpaces: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node with spaces', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node with spaces',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schemaWithSpaces);
  assertContains(result, 'node_with_spaces[');
});

test('disambiguates colliding safe node IDs', () => {
  const schemaWithCollisions: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'a-b', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
      { name: 'a_b', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [
      { from: 'a-b', to: 'a_b', edge_type: 'direct' },
    ],
    entry_point: 'a-b',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schemaWithCollisions);
  assertContains(result, 'a_b[');
  assertContains(result, 'a_b_2[');
  assertContains(result, 'a_b_2 --> a_b');
});

test('renders node groups as Mermaid subgraphs', () => {
  const groupedSchema: GraphSchema = {
    name: 'Grouped',
    version: '1.0.0',
    nodes: [
      { name: 'n1', node_type: 'llm', input_fields: [], output_fields: [], attributes: { group: 'alpha' } },
      { name: 'n2', node_type: 'tool', input_fields: [], output_fields: [], attributes: { group: 'alpha' } },
      { name: 'n3', node_type: 'tool', input_fields: [], output_fields: [], attributes: { group: 'beta' } },
      { name: 'n4', node_type: 'tool', input_fields: [], output_fields: [], attributes: { group: 'beta' } },
    ],
    edges: [],
    entry_point: 'n1',
    metadata: {},
  };

  const result = renderSchemaToMermaid(groupedSchema, undefined, undefined, {
    grouping: { mode: 'attribute', attributeKey: 'group' },
  });

  assertContains(result, '%% Groups');
  assertContains(result, 'subgraph');
  assertContains(result, '["alpha"]');
  assertContains(result, '["beta"]');
});

// M-454: XSS escape tests - ensure Mermaid syntax injection is prevented
console.log('\nM-454 XSS escape handling:');
test('escapes newlines in node names (prevents syntax injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node\nwith\nnewlines', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node\nwith\nnewlines',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  // Newlines in the label should be replaced with spaces
  assertContains(result, '"node with newlines"'); // Newlines escaped to spaces in label
  // The raw newline from the node name should NOT appear in the label
  assertNotContains(result, '"node\n');
});

test('escapes brackets in node names (prevents shape injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node]injection[test', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node]injection[test',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, '#93;'); // Escaped ]
  assertContains(result, '#91;'); // Escaped [
});

test('escapes semicolons in node names (prevents statement injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node;malicious', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node;malicious',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, '#59;'); // Escaped ;
});

test('escapes pipes in edge labels (prevents label injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'a', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
      { name: 'b', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [
      { from: 'a', to: 'b', edge_type: 'direct', label: 'label|injection' },
    ],
    entry_point: 'a',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, '#124;'); // Escaped |
});

test('escapes braces in node names (prevents shape injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node{with}braces', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node{with}braces',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, '#123;'); // Escaped {
  assertContains(result, '#125;'); // Escaped }
});

test('escapes parens in node names (prevents shape injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node(with)parens', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node(with)parens',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, '#40;'); // Escaped (
  assertContains(result, '#41;'); // Escaped )
});

test('escapes arrow syntax in node names (prevents edge injection)', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'node--edge-->injection', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'node--edge-->injection',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  // Dashes are safe inside quoted strings, but > is escaped
  assertContains(result, '#62;'); // > is escaped
  // The full label should not contain the raw > character
  assertNotContains(result, '-->'); // Arrow syntax is broken
});

// M-2624: Additional node shape tests
console.log('\nNode shapes - all types:');
test('uses parallelogram shape for aggregator nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'agg', node_type: 'aggregator', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'agg',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'agg[/');
  assertContains(result, '/]');
});

test('uses subroutine shape for validator nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'validate', node_type: 'validator', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'validate',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'validate[[');
  assertContains(result, ']]');
});

test('uses double circle shape for human_in_loop nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'human', node_type: 'human_in_loop', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'human',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'human(((');
  assertContains(result, ')))');
});

test('uses cylinder shape for checkpoint nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'save', node_type: 'checkpoint', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'save',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'save[(');
  assertContains(result, ')]');
});

test('uses rectangle shape for custom nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'custom', node_type: 'custom', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'custom',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'custom[');
  assertContains(result, ']');
  // Should not use other shapes
  assertNotContains(result, 'custom[[');
  assertNotContains(result, 'custom[(');
  assertNotContains(result, 'custom[/');
});

test('uses rectangle shape for transform nodes', () => {
  const schema: GraphSchema = {
    name: 'Test',
    version: '1.0.0',
    nodes: [
      { name: 'xform', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'xform',
    metadata: {},
  };
  const result = renderSchemaToMermaid(schema);
  assertContains(result, 'xform[');
});

// M-2624: Status indicator tests
console.log('\nStatus indicators - all statuses:');
test('shows error indicator (✗) for error status', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'error' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, '✗');
  assertContains(result, 'class start error');
});

test('shows no indicator for pending status', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'pending' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  // Pending has no indicator, check that the other indicators aren't present for start
  const startNodeLine = result.split('\n').find(l => l.includes('start[') && l.includes('"start'));
  if (!startNodeLine) throw new Error('Could not find start node line');
  assertNotContains(startNodeLine, '✓');
  assertNotContains(startNodeLine, '⚡');
  assertNotContains(startNodeLine, '✗');
});

// M-2624: Direction options tests
console.log('\nDirection options:');
test('supports TB direction (top to bottom)', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, undefined, { direction: 'TB' });
  assertContains(result, 'flowchart TB');
});

test('supports BT direction (bottom to top)', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, undefined, { direction: 'BT' });
  assertContains(result, 'flowchart BT');
});

test('supports RL direction (right to left)', () => {
  const result = renderSchemaToMermaid(simpleSchema, undefined, undefined, { direction: 'RL' });
  assertContains(result, 'flowchart RL');
});

// M-2624: Edge cases
console.log('\nEdge cases:');
test('handles schema with no edges', () => {
  const noEdgeSchema: GraphSchema = {
    name: 'No Edges',
    version: '1.0.0',
    nodes: [
      { name: 'lonely', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'lonely',
    metadata: {},
  };
  const result = renderSchemaToMermaid(noEdgeSchema);
  assertContains(result, 'flowchart TD');
  assertContains(result, 'lonely[');
  assertContains(result, '%% Edges');
});

test('handles schema with single node', () => {
  const singleNodeSchema: GraphSchema = {
    name: 'Single',
    version: '1.0.0',
    nodes: [
      { name: 'only', node_type: 'llm', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'only',
    metadata: {},
  };
  const result = renderSchemaToMermaid(singleNodeSchema);
  assertContains(result, 'only([');
});

test('handles empty node name gracefully', () => {
  const emptyNameSchema: GraphSchema = {
    name: 'Empty Name',
    version: '1.0.0',
    nodes: [
      { name: '', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: '',
    metadata: {},
  };
  const result = renderSchemaToMermaid(emptyNameSchema);
  // Empty name should become 'node' as the safe ID
  assertContains(result, 'node[');
});

test('handles nodes with identical safe IDs', () => {
  const collisionSchema: GraphSchema = {
    name: 'Collision',
    version: '1.0.0',
    nodes: [
      { name: 'test@node', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
      { name: 'test#node', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
      { name: 'test!node', node_type: 'transform', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'test@node',
    metadata: {},
  };
  const result = renderSchemaToMermaid(collisionSchema);
  // All three should have unique IDs
  assertContains(result, 'test_node[');
  assertContains(result, 'test_node_2[');
  assertContains(result, 'test_node_3[');
});

test('handles zero duration correctly', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 0 }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, '(0ms)');
});

test('handles large duration values', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 123456 }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates);
  assertContains(result, '(123456ms)');
});

// M-2624: createMermaidBlob tests
console.log('\ncreateMermaidBlob:');
test('creates blob with correct content', () => {
  const blob = createMermaidBlob('flowchart TD\n  A --> B');
  // Blob exists and has size
  if (blob.size !== 'flowchart TD\n  A --> B'.length) {
    throw new Error(`Expected size ${('flowchart TD\n  A --> B').length}, got ${blob.size}`);
  }
});

test('creates blob with correct type', () => {
  const blob = createMermaidBlob('test');
  if (blob.type !== 'text/plain') {
    throw new Error(`Expected type 'text/plain', got '${blob.type}'`);
  }
});

test('creates blob for empty string', () => {
  const blob = createMermaidBlob('');
  if (blob.size !== 0) {
    throw new Error(`Expected size 0, got ${blob.size}`);
  }
});

test('creates blob with unicode content', () => {
  const content = 'flowchart TD\n  A["Node ✓"] --> B["Node ⚡"]';
  const blob = createMermaidBlob(content);
  if (blob.size === 0) {
    throw new Error('Blob should not be empty');
  }
});

// M-2624: Combined options tests
console.log('\nCombined options:');
test('all options disabled produces minimal output', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 100 }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates, undefined, {
    showDurations: false,
    showStatusIndicators: false,
    includeStyleDefs: false,
  });
  assertNotContains(result, '✓');
  assertNotContains(result, '(100ms)');
  assertNotContains(result, 'classDef');
});

test('all options enabled produces full output', () => {
  const nodeStates = new Map<string, NodeState>([
    ['start', { status: 'completed', durationMs: 100 }],
    ['process', { status: 'active' }],
  ]);
  const result = renderSchemaToMermaid(simpleSchema, nodeStates, 'process', {
    direction: 'LR',
    showDurations: true,
    showStatusIndicators: true,
    includeStyleDefs: true,
  });
  assertContains(result, 'flowchart LR');
  assertContains(result, '✓');
  assertContains(result, '⚡');
  assertContains(result, '(100ms)');
  assertContains(result, 'classDef');
  assertContains(result, 'class process current');
});

// M-2624: renderViewModelToMermaid edge cases
console.log('\nrenderViewModelToMermaid edge cases:');
test('returns null for undefined view model', () => {
  const result = renderViewModelToMermaid(undefined);
  if (result !== null) {
    throw new Error('Expected null result for undefined');
  }
});

test('passes options through to renderSchemaToMermaid', () => {
  const viewModel: GraphViewModel = {
    schema: simpleSchema,
    schemaId: 'test123',
    nodeStates: new Map([['start', { status: 'completed' }]]),
    currentNode: undefined,
    state: {},
    changedPaths: [],
    cursor: { threadId: 'test', seq: '1' },
    isLive: true,
    observedNodes: new Set(),
    outOfSchemaNodes: new Set(),
  };
  const result = renderViewModelToMermaid(viewModel, { direction: 'LR', showDurations: false });
  assertContains(result!, 'flowchart LR');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
