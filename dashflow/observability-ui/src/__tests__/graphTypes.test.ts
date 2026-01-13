// M-444: Unit tests for graph type definitions and constants
// Run with: npx tsx src/__tests__/graphTypes.test.ts

import {
  NODE_TYPE_STYLES,
  NODE_STATUS_STYLES,
  NodeType,
  NodeStatus,
  EdgeType,
  NodeMetadata,
  NodeSchema,
  EdgeSchema,
  GraphSchema,
  NodeExecution,
  GraphExecution,
} from '../types/graph';
import { DIFF_STATUS_STYLES, DiffStatus } from '../types/schemaDiff';

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

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || 'Expected true but got false');
  }
}

function assertDefined<T>(value: T | undefined, message?: string): asserts value is T {
  if (value === undefined) {
    throw new Error(message || 'Expected value to be defined');
  }
}

console.log('\nGraph Types Tests\n');

console.log('NODE_TYPE_STYLES:');

const nodeTypes: NodeType[] = [
  'transform',
  'llm',
  'tool',
  'router',
  'aggregator',
  'validator',
  'human_in_loop',
  'checkpoint',
  'custom',
];

test('all NodeType values have styles defined', () => {
  for (const nodeType of nodeTypes) {
    assertDefined(NODE_TYPE_STYLES[nodeType], `Style for ${nodeType}`);
  }
});

test('all node type styles have required properties', () => {
  for (const nodeType of nodeTypes) {
    const style = NODE_TYPE_STYLES[nodeType];
    assertTrue(typeof style.icon === 'string' && style.icon.length > 0, `${nodeType} icon`);
    assertTrue(typeof style.color === 'string' && style.color.startsWith('#'), `${nodeType} color`);
    assertTrue(typeof style.bgColor === 'string' && style.bgColor.startsWith('#'), `${nodeType} bgColor`);
    assertTrue(typeof style.ariaLabel === 'string' && style.ariaLabel.length > 0, `${nodeType} ariaLabel (M-472)`);
  }
});

test('node type ariaLabels are descriptive (M-472 accessibility)', () => {
  // M-472: ariaLabel should provide screen reader accessible descriptions
  for (const nodeType of nodeTypes) {
    const style = NODE_TYPE_STYLES[nodeType];
    assertTrue(style.ariaLabel.toLowerCase().includes('node'), `${nodeType} ariaLabel should mention "node"`);
  }
});

test('node type icons are unique (except for special cases)', () => {
  const icons = nodeTypes.map(t => NODE_TYPE_STYLES[t].icon);
  const uniqueIcons = new Set(icons);
  // All icons should be unique for visual distinction
  assertTrue(uniqueIcons.size === icons.length, `Expected ${icons.length} unique icons, got ${uniqueIcons.size}`);
});

console.log('\nNODE_STATUS_STYLES:');

const nodeStatuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];

test('all NodeStatus values have styles defined', () => {
  for (const status of nodeStatuses) {
    assertDefined(NODE_STATUS_STYLES[status], `Style for ${status}`);
  }
});

test('all node status styles have borderColor', () => {
  for (const status of nodeStatuses) {
    const style = NODE_STATUS_STYLES[status];
    assertTrue(typeof style.borderColor === 'string', `${status} borderColor`);
  }
});

test('active status has pulseColor for animation', () => {
  const activeStyle = NODE_STATUS_STYLES.active;
  assertDefined(activeStyle.pulseColor, 'active status should have pulseColor');
  assertTrue(activeStyle.pulseColor.startsWith('#'), 'pulseColor should be hex color');
});

test('non-active statuses do not need pulseColor', () => {
  // Only active status needs pulse animation
  for (const status of nodeStatuses.filter(s => s !== 'active')) {
    // These should not have pulseColor (undefined is fine)
    const style = NODE_STATUS_STYLES[status];
    assertTrue(style.pulseColor === undefined, `${status} should not have pulseColor`);
  }
});

console.log('\nDIFF_STATUS_STYLES:');

const diffStatuses: DiffStatus[] = ['unchanged', 'added', 'removed', 'modified', 'out-of-schema'];

test('all DiffStatus values have styles defined', () => {
  for (const status of diffStatuses) {
    assertDefined(DIFF_STATUS_STYLES[status], `Style for ${status}`);
  }
});

test('all diff status styles have borderStyle', () => {
  for (const status of diffStatuses) {
    const style = DIFF_STATUS_STYLES[status];
    assertTrue(
      style.borderStyle === 'solid' || style.borderStyle === 'dashed' || style.borderStyle === 'dotted',
      `${status} borderStyle should be valid CSS`
    );
  }
});

test('unchanged status has minimal styling', () => {
  const style = DIFF_STATUS_STYLES.unchanged;
  // Unchanged should not draw attention
  assertTrue(style.borderColor === '', 'unchanged borderColor should be empty');
  assertTrue(style.badge === undefined, 'unchanged should not have badge');
});

test('added/removed/modified have distinctive badges', () => {
  const added = DIFF_STATUS_STYLES.added;
  const removed = DIFF_STATUS_STYLES.removed;
  const modified = DIFF_STATUS_STYLES.modified;

  assertTrue(added.badge === '+', 'added badge');
  assertTrue(removed.badge === '-', 'removed badge');
  assertTrue(modified.badge === '~', 'modified badge');
});

test('out-of-schema has warning styling', () => {
  const style = DIFF_STATUS_STYLES['out-of-schema'];
  assertTrue(style.badge === '!', 'out-of-schema badge should be !');
  assertTrue(style.borderStyle === 'dotted', 'out-of-schema should use dotted border');
});

test('diff colors follow semantic meaning', () => {
  // Green for additions, red for removals, amber/yellow for modifications
  const added = DIFF_STATUS_STYLES.added;
  const removed = DIFF_STATUS_STYLES.removed;
  const modified = DIFF_STATUS_STYLES.modified;

  assertTrue(added.borderColor.includes('22c55e'), 'added should use green');
  assertTrue(removed.borderColor.includes('ef4444'), 'removed should use red');
  assertTrue(modified.borderColor.includes('f59e0b'), 'modified should use amber');
});

// ============================================
// EDGE TYPE TESTS
// ============================================

console.log('\nEDGE_TYPE:');

const edgeTypes: EdgeType[] = ['direct', 'conditional', 'parallel'];

test('EdgeType has exactly 3 values', () => {
  assertTrue(edgeTypes.length === 3, `Expected 3 edge types, got ${edgeTypes.length}`);
});

test('direct edge type is valid', () => {
  const direct: EdgeType = 'direct';
  assertTrue(edgeTypes.includes(direct), 'direct should be valid EdgeType');
});

test('conditional edge type is valid', () => {
  const conditional: EdgeType = 'conditional';
  assertTrue(edgeTypes.includes(conditional), 'conditional should be valid EdgeType');
});

test('parallel edge type is valid', () => {
  const parallel: EdgeType = 'parallel';
  assertTrue(edgeTypes.includes(parallel), 'parallel should be valid EdgeType');
});

test('edge types cover all graph flow patterns', () => {
  // Semantic validation: these types should cover common graph patterns
  assertTrue(edgeTypes.includes('direct'), 'direct for simple A->B flows');
  assertTrue(edgeTypes.includes('conditional'), 'conditional for branching');
  assertTrue(edgeTypes.includes('parallel'), 'parallel for concurrent execution');
});

// ============================================
// COLOR FORMAT VALIDATION
// ============================================

console.log('\nCOLOR_FORMAT_VALIDATION:');

function isValidHexColor(color: string): boolean {
  return /^#[0-9A-Fa-f]{6}$/.test(color);
}

test('all node type colors are valid 6-digit hex', () => {
  for (const nodeType of nodeTypes) {
    const style = NODE_TYPE_STYLES[nodeType];
    assertTrue(isValidHexColor(style.color), `${nodeType} color ${style.color} should be valid hex`);
  }
});

test('all node type bgColors are valid 6-digit hex', () => {
  for (const nodeType of nodeTypes) {
    const style = NODE_TYPE_STYLES[nodeType];
    assertTrue(isValidHexColor(style.bgColor), `${nodeType} bgColor ${style.bgColor} should be valid hex`);
  }
});

test('all node status borderColors are valid 6-digit hex', () => {
  for (const status of nodeStatuses) {
    const style = NODE_STATUS_STYLES[status];
    assertTrue(isValidHexColor(style.borderColor), `${status} borderColor ${style.borderColor} should be valid hex`);
  }
});

test('active status pulseColor is valid hex', () => {
  const style = NODE_STATUS_STYLES.active;
  assertTrue(
    style.pulseColor !== undefined && isValidHexColor(style.pulseColor),
    `active pulseColor ${style.pulseColor} should be valid hex`
  );
});

// ============================================
// NODE TYPE STYLE COMPLETENESS
// ============================================

console.log('\nNODE_TYPE_STYLE_COMPLETENESS:');

test('NODE_TYPE_STYLES has entry for every NodeType', () => {
  const styleKeys = Object.keys(NODE_TYPE_STYLES) as NodeType[];
  assertTrue(styleKeys.length === nodeTypes.length, `Style count ${styleKeys.length} should match type count ${nodeTypes.length}`);
});

test('no extra keys in NODE_TYPE_STYLES', () => {
  const styleKeys = Object.keys(NODE_TYPE_STYLES);
  for (const key of styleKeys) {
    assertTrue(nodeTypes.includes(key as NodeType), `Unexpected key ${key} in NODE_TYPE_STYLES`);
  }
});

test('no extra keys in NODE_STATUS_STYLES', () => {
  const styleKeys = Object.keys(NODE_STATUS_STYLES);
  for (const key of styleKeys) {
    assertTrue(nodeStatuses.includes(key as NodeStatus), `Unexpected key ${key} in NODE_STATUS_STYLES`);
  }
});

// ============================================
// ARIA LABEL QUALITY
// ============================================

console.log('\nARIA_LABEL_QUALITY:');

test('aria labels are unique for each node type', () => {
  const labels = nodeTypes.map(t => NODE_TYPE_STYLES[t].ariaLabel);
  const uniqueLabels = new Set(labels);
  assertTrue(uniqueLabels.size === labels.length, 'All aria labels should be unique');
});

test('aria labels are lowercase-normalized unique (case-insensitive)', () => {
  const labels = nodeTypes.map(t => NODE_TYPE_STYLES[t].ariaLabel.toLowerCase());
  const uniqueLabels = new Set(labels);
  assertTrue(uniqueLabels.size === labels.length, 'Aria labels should be unique even case-insensitively');
});

test('aria labels contain the word node (for clarity)', () => {
  for (const nodeType of nodeTypes) {
    const label = NODE_TYPE_STYLES[nodeType].ariaLabel.toLowerCase();
    assertTrue(label.includes('node'), `${nodeType} ariaLabel "${label}" should contain "node"`);
  }
});

test('human_in_loop has descriptive aria label', () => {
  const label = NODE_TYPE_STYLES.human_in_loop.ariaLabel;
  assertTrue(label.toLowerCase().includes('human'), 'human_in_loop should mention human');
});

// ============================================
// NODE STATUS SEMANTICS
// ============================================

console.log('\nNODE_STATUS_SEMANTICS:');

test('pending status uses neutral color', () => {
  const borderColor = NODE_STATUS_STYLES.pending.borderColor.toLowerCase();
  // Gray tones: typically 4B-5F range for RGB components
  assertTrue(
    borderColor.includes('4b') || borderColor.includes('5') || borderColor.includes('6'),
    'pending should use gray/neutral color'
  );
});

test('active status uses attention-grabbing blue', () => {
  const borderColor = NODE_STATUS_STYLES.active.borderColor.toLowerCase();
  assertTrue(borderColor.includes('3b82f6') || borderColor.includes('60a5fa'), 'active should use blue');
});

test('completed status uses success green', () => {
  const borderColor = NODE_STATUS_STYLES.completed.borderColor.toLowerCase();
  assertTrue(borderColor.includes('22c55e'), 'completed should use green');
});

test('error status uses danger red', () => {
  const borderColor = NODE_STATUS_STYLES.error.borderColor.toLowerCase();
  assertTrue(borderColor.includes('ef4444'), 'error should use red');
});

// ============================================
// NODE TYPE CATEGORIZATION
// ============================================

console.log('\nNODE_TYPE_CATEGORIZATION:');

test('processing node types use gear/tool icons', () => {
  const processingTypes: NodeType[] = ['transform', 'tool'];
  for (const nodeType of processingTypes) {
    const icon = NODE_TYPE_STYLES[nodeType].icon;
    assertTrue(icon === '⚙️' || icon === '🔧', `${nodeType} should use processing icon`);
  }
});

test('AI node type uses robot icon', () => {
  assertTrue(NODE_TYPE_STYLES.llm.icon === '🤖', 'llm should use robot icon');
});

test('routing node type uses branching icon', () => {
  assertTrue(NODE_TYPE_STYLES.router.icon === '🔀', 'router should use branching icon');
});

test('aggregator uses chart/data icon', () => {
  assertTrue(NODE_TYPE_STYLES.aggregator.icon === '📊', 'aggregator should use chart icon');
});

test('validator uses checkmark icon', () => {
  assertTrue(NODE_TYPE_STYLES.validator.icon === '✓', 'validator should use checkmark');
});

test('human_in_loop uses person icon', () => {
  assertTrue(NODE_TYPE_STYLES.human_in_loop.icon === '👤', 'human_in_loop should use person icon');
});

test('checkpoint uses save/disk icon', () => {
  assertTrue(NODE_TYPE_STYLES.checkpoint.icon === '💾', 'checkpoint should use disk icon');
});

test('custom uses generic package icon', () => {
  assertTrue(NODE_TYPE_STYLES.custom.icon === '📦', 'custom should use package icon');
});

// ============================================
// DARK THEME COLOR CONTRAST (GN-01)
// ============================================

console.log('\nDARK_THEME_CONTRAST:');

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const result = /^#([0-9A-Fa-f]{2})([0-9A-Fa-f]{2})([0-9A-Fa-f]{2})$/.exec(hex);
  if (!result) throw new Error(`Invalid hex color: ${hex}`);
  return {
    r: parseInt(result[1], 16),
    g: parseInt(result[2], 16),
    b: parseInt(result[3], 16),
  };
}

function getLuminance(rgb: { r: number; g: number; b: number }): number {
  // Relative luminance formula
  const [rs, gs, bs] = [rgb.r / 255, rgb.g / 255, rgb.b / 255].map(c =>
    c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  );
  return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
}

test('all bgColors are dark (luminance < 0.2)', () => {
  for (const nodeType of nodeTypes) {
    const bgColor = NODE_TYPE_STYLES[nodeType].bgColor;
    const rgb = hexToRgb(bgColor);
    const luminance = getLuminance(rgb);
    assertTrue(luminance < 0.2, `${nodeType} bgColor ${bgColor} luminance ${luminance.toFixed(3)} should be < 0.2`);
  }
});

test('all colors are bright enough for dark backgrounds (luminance > 0.1)', () => {
  for (const nodeType of nodeTypes) {
    const color = NODE_TYPE_STYLES[nodeType].color;
    const rgb = hexToRgb(color);
    const luminance = getLuminance(rgb);
    assertTrue(luminance > 0.1, `${nodeType} color ${color} luminance ${luminance.toFixed(3)} should be > 0.1`);
  }
});

test('color has higher luminance than bgColor for each node type', () => {
  for (const nodeType of nodeTypes) {
    const style = NODE_TYPE_STYLES[nodeType];
    const colorLum = getLuminance(hexToRgb(style.color));
    const bgLum = getLuminance(hexToRgb(style.bgColor));
    assertTrue(colorLum > bgLum, `${nodeType} color should be brighter than bgColor`);
  }
});

// ============================================
// INTERFACE STRUCTURE VALIDATION
// ============================================

console.log('\nINTERFACE_STRUCTURE:');

// Create valid interface instances to test type compatibility
test('NodeMetadata interface has expected fields', () => {
  const metadata: NodeMetadata = {
    node_type: 'llm',
    input_fields: ['query'],
    output_fields: ['response'],
    attributes: { model: 'gpt-4' },
  };
  assertTrue(metadata.node_type === 'llm', 'node_type should be settable');
  assertTrue(Array.isArray(metadata.input_fields), 'input_fields should be array');
  assertTrue(Array.isArray(metadata.output_fields), 'output_fields should be array');
  assertTrue(typeof metadata.attributes === 'object', 'attributes should be object');
});

test('NodeMetadata optional fields work correctly', () => {
  const metadata: NodeMetadata = {
    description: 'Test description',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    position: [100, 200],
    attributes: {},
  };
  assertTrue(metadata.description === 'Test description', 'description should be optional');
  assertTrue(Array.isArray(metadata.position) && metadata.position.length === 2, 'position should be tuple');
});

test('NodeSchema interface has expected fields', () => {
  const schema: NodeSchema = {
    name: 'test_node',
    node_type: 'tool',
    input_fields: ['input'],
    output_fields: ['output'],
    attributes: {},
  };
  assertTrue(schema.name === 'test_node', 'name should be required');
  assertTrue(schema.node_type === 'tool', 'node_type should be required');
});

test('EdgeSchema interface has expected fields', () => {
  const edge: EdgeSchema = {
    from: 'node_a',
    to: 'node_b',
    edge_type: 'direct',
  };
  assertTrue(edge.from === 'node_a', 'from should be required');
  assertTrue(edge.to === 'node_b', 'to should be required');
  assertTrue(edge.edge_type === 'direct', 'edge_type should be required');
});

test('EdgeSchema conditional_targets for conditional edges', () => {
  const edge: EdgeSchema = {
    from: 'router',
    to: 'default',
    edge_type: 'conditional',
    label: 'success',
    conditional_targets: ['success_node', 'failure_node'],
  };
  assertTrue(edge.edge_type === 'conditional', 'edge_type should be conditional');
  assertTrue(Array.isArray(edge.conditional_targets), 'conditional_targets should be array');
  assertTrue(edge.conditional_targets!.length === 2, 'should have 2 targets');
});

test('GraphSchema interface has expected fields', () => {
  const graph: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [],
    edges: [],
    entry_point: 'start',
    metadata: {},
  };
  assertTrue(graph.name === 'test_graph', 'name should be required');
  assertTrue(graph.version === '1.0.0', 'version should be required');
  assertTrue(graph.entry_point === 'start', 'entry_point should be required');
  assertTrue(Array.isArray(graph.nodes), 'nodes should be array');
  assertTrue(Array.isArray(graph.edges), 'edges should be array');
});

test('GraphSchema optional fields work correctly', () => {
  const graph: GraphSchema = {
    name: 'full_graph',
    version: '2.0.0',
    description: 'A comprehensive graph',
    nodes: [],
    edges: [],
    entry_point: 'init',
    state_type: 'CustomState',
    exported_at: '2026-01-05T12:00:00Z',
    metadata: { author: 'test' },
  };
  assertTrue(graph.description === 'A comprehensive graph', 'description optional');
  assertTrue(graph.state_type === 'CustomState', 'state_type optional');
  assertTrue(graph.exported_at === '2026-01-05T12:00:00Z', 'exported_at optional');
});

test('NodeExecution interface has expected fields', () => {
  const execution: NodeExecution = {
    node_name: 'test_node',
    status: 'completed',
  };
  assertTrue(execution.node_name === 'test_node', 'node_name required');
  assertTrue(execution.status === 'completed', 'status required');
});

test('NodeExecution optional timing fields', () => {
  const execution: NodeExecution = {
    node_name: 'timed_node',
    status: 'completed',
    start_time: 1000,
    end_time: 2000,
    duration_ms: 1000,
  };
  assertTrue(execution.start_time === 1000, 'start_time optional');
  assertTrue(execution.end_time === 2000, 'end_time optional');
  assertTrue(execution.duration_ms === 1000, 'duration_ms optional');
});

test('NodeExecution optional state fields', () => {
  const execution: NodeExecution = {
    node_name: 'stateful_node',
    status: 'completed',
    input_state: { query: 'hello' },
    output_state: { response: 'world' },
  };
  assertTrue(typeof execution.input_state === 'object', 'input_state optional');
  assertTrue(typeof execution.output_state === 'object', 'output_state optional');
});

test('NodeExecution error field for error status', () => {
  const execution: NodeExecution = {
    node_name: 'failed_node',
    status: 'error',
    error: 'Something went wrong',
  };
  assertTrue(execution.status === 'error', 'status should be error');
  assertTrue(execution.error === 'Something went wrong', 'error field optional');
});

test('GraphExecution interface has expected fields', () => {
  const execution: GraphExecution = {
    graph_id: 'graph-123',
    graph_name: 'test_graph',
    thread_id: 'thread-456',
    schema: {
      name: 'test_graph',
      version: '1.0.0',
      nodes: [],
      edges: [],
      entry_point: 'start',
      metadata: {},
    },
    node_executions: {},
    state: {},
    status: 'running',
    start_time: Date.now(),
  };
  assertTrue(execution.graph_id === 'graph-123', 'graph_id required');
  assertTrue(execution.thread_id === 'thread-456', 'thread_id required');
  assertTrue(execution.status === 'running', 'status required');
});

test('GraphExecution status values are valid', () => {
  const statuses: GraphExecution['status'][] = ['running', 'completed', 'error'];
  for (const status of statuses) {
    const execution: GraphExecution = {
      graph_id: 'test',
      graph_name: 'test',
      thread_id: 'test',
      schema: { name: 'test', version: '1', nodes: [], edges: [], entry_point: 'start', metadata: {} },
      node_executions: {},
      state: {},
      status,
      start_time: 0,
    };
    assertTrue(execution.status === status, `status ${status} should be valid`);
  }
});

test('GraphExecution optional fields', () => {
  const execution: GraphExecution = {
    graph_id: 'graph-789',
    graph_name: 'full_graph',
    thread_id: 'thread-abc',
    schema: { name: 'full', version: '1.0', nodes: [], edges: [], entry_point: 'start', metadata: {} },
    schema_id: 'sha256:abc123',
    current_node: 'processing',
    node_executions: { processing: { node_name: 'processing', status: 'active' } },
    state: { data: 'value' },
    status: 'running',
    start_time: 1000,
    end_time: 2000,
  };
  assertTrue(execution.schema_id === 'sha256:abc123', 'schema_id optional');
  assertTrue(execution.current_node === 'processing', 'current_node optional');
  assertTrue(execution.end_time === 2000, 'end_time optional');
});

// ============================================
// TYPE EXHAUSTIVENESS
// ============================================

console.log('\nTYPE_EXHAUSTIVENESS:');

test('NodeType array covers all defined types', () => {
  // This test ensures our test array matches the actual type definition
  const allTypes: NodeType[] = [
    'transform', 'llm', 'tool', 'router', 'aggregator',
    'validator', 'human_in_loop', 'checkpoint', 'custom',
  ];
  assertTrue(allTypes.length === 9, 'Should have 9 node types');
  for (const t of allTypes) {
    assertTrue(nodeTypes.includes(t), `nodeTypes array should include ${t}`);
  }
});

test('NodeStatus array covers all defined statuses', () => {
  const allStatuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  assertTrue(allStatuses.length === 4, 'Should have 4 node statuses');
  for (const s of allStatuses) {
    assertTrue(nodeStatuses.includes(s), `nodeStatuses array should include ${s}`);
  }
});

test('EdgeType array covers all defined types', () => {
  const allEdgeTypes: EdgeType[] = ['direct', 'conditional', 'parallel'];
  assertTrue(allEdgeTypes.length === 3, 'Should have 3 edge types');
  for (const e of allEdgeTypes) {
    assertTrue(edgeTypes.includes(e), `edgeTypes array should include ${e}`);
  }
});

// ============================================
// CROSS-INTERFACE CONSISTENCY
// ============================================

console.log('\nCROSS_INTERFACE_CONSISTENCY:');

test('NodeMetadata and NodeSchema share common fields', () => {
  // Both interfaces should have node_type, input_fields, output_fields, attributes
  const metadata: NodeMetadata = {
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
  const schema: NodeSchema = {
    name: 'test',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
  assertTrue(metadata.node_type === schema.node_type, 'node_type should be compatible');
  assertTrue(
    Array.isArray(metadata.input_fields) && Array.isArray(schema.input_fields),
    'input_fields should be compatible'
  );
});

test('NodeExecution status values match NodeStatus type', () => {
  for (const status of nodeStatuses) {
    const execution: NodeExecution = {
      node_name: 'test',
      status,
    };
    assertTrue(nodeStatuses.includes(execution.status), `${status} should be valid NodeStatus`);
  }
});

test('GraphExecution node_executions values are valid NodeExecution', () => {
  const nodeExec: NodeExecution = {
    node_name: 'inner',
    status: 'completed',
    duration_ms: 100,
  };
  const graphExec: GraphExecution = {
    graph_id: 'test',
    graph_name: 'test',
    thread_id: 'test',
    schema: { name: 'test', version: '1', nodes: [], edges: [], entry_point: 'start', metadata: {} },
    node_executions: { inner: nodeExec },
    state: {},
    status: 'completed',
    start_time: 0,
  };
  assertTrue(graphExec.node_executions.inner.status === 'completed', 'nested NodeExecution should be valid');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
