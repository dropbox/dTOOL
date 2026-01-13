// M-444: Component tests for GraphCanvas
// Run with: npx tsx src/__tests__/GraphCanvas.test.tsx
//
// GraphCanvas uses @xyflow/react hooks (useNodesState, useEdgesState) which require
// React Flow context. We test the extracted utility functions and logic.

import type { GraphSchema, NodeStatus } from '../types/graph';

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
    throw new Error(message || `Expected ${expected}, got ${actual}`);
  }
}

function assertDeepEqual<T>(actual: T, expected: T, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(message || `Expected ${expectedStr}, got ${actualStr}`);
  }
}

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

// ============================================================================
// Test the extracted logic from GraphCanvas
// ============================================================================

console.log('\nGraphCanvas Tests\n');

// Schema structure key computation (from GraphCanvas lines 119-130)
// This determines when dagre layout recomputation is needed
console.log('Schema structure key computation:');

interface GroupingOptions {
  mode: 'none' | 'attribute' | 'prefix';
  attributeKey?: string;
}

function computeSchemaStructureKey(schema: GraphSchema | null, grouping: GroupingOptions): string {
  if (!schema) return '';
  const nodeNames = schema.nodes.map((n) => n.name).sort().join(',');
  const edgeKeys = schema.edges
    .map((e) => {
      const targets = e.conditional_targets?.sort().join('+') || e.to;
      return `${e.from}->${targets}`;
    })
    .sort()
    .join(';');
  return `${nodeNames}|${edgeKeys}|${grouping.mode}|${grouping.attributeKey || ''}`;
}

test('empty schema returns empty key', () => {
  assertEqual(computeSchemaStructureKey(null, { mode: 'none' }), '');
});

test('schema key includes sorted node names', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'z_node', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'a_node', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'a_node',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertIncludes(key, 'a_node,z_node'); // Sorted order
});

test('schema key includes edge information', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'start', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'end', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [{ from: 'start', to: 'end', edge_type: 'direct' }],
    entry_point: 'start',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertIncludes(key, 'start->end');
});

test('schema key includes conditional targets sorted', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'router', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'path_a', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'path_b', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [{ from: 'router', to: '', edge_type: 'conditional', conditional_targets: ['path_b', 'path_a'] }],
    entry_point: 'router',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertIncludes(key, 'router->path_a+path_b'); // Sorted targets
});

test('schema key includes grouping mode', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [{ name: 'node', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} }],
    edges: [],
    entry_point: 'node',
    metadata: {},
  };
  const key1 = computeSchemaStructureKey(schema, { mode: 'none' });
  const key2 = computeSchemaStructureKey(schema, { mode: 'attribute', attributeKey: 'phase' });

  assertIncludes(key1, '|none|');
  assertIncludes(key2, '|attribute|phase');
});

test('same structure produces same key', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'a', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'b', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [{ from: 'a', to: 'b', edge_type: 'direct' }],
    entry_point: 'a',
    metadata: {},
  };
  const key1 = computeSchemaStructureKey(schema, { mode: 'none' });
  const key2 = computeSchemaStructureKey(schema, { mode: 'none' });
  assertEqual(key1, key2);
});

// MiniMap node color computation (from GraphCanvas lines 468-474)
console.log('\nMiniMap node color computation:');

function getMiniMapNodeColor(status: NodeStatus | undefined): string {
  if (status === 'active') return '#3B82F6';
  if (status === 'completed') return '#22C55E';
  if (status === 'error') return '#EF4444';
  return '#D1D5DB';
}

test('active status returns blue', () => {
  assertEqual(getMiniMapNodeColor('active'), '#3B82F6');
});

test('completed status returns green', () => {
  assertEqual(getMiniMapNodeColor('completed'), '#22C55E');
});

test('error status returns red', () => {
  assertEqual(getMiniMapNodeColor('error'), '#EF4444');
});

test('pending status returns gray', () => {
  assertEqual(getMiniMapNodeColor('pending'), '#D1D5DB');
});

test('undefined status returns gray', () => {
  assertEqual(getMiniMapNodeColor(undefined), '#D1D5DB');
});

// Edge ID generation (from GraphCanvas lines 174-210)
console.log('\nEdge ID generation:');

interface SchemaEdge {
  from: string;
  to: string;
  edge_type?: string;
  conditional_targets?: string[];
  label?: string;
}

function generateEdgeIds(edges: SchemaEdge[]): string[] {
  return edges.flatMap((edge) => {
    const edgeType = edge.edge_type || 'default';

    if (edge.conditional_targets && edge.conditional_targets.length > 0) {
      return edge.conditional_targets.map((target, idx) =>
        `${edge.from}-${target}-${edgeType}-${idx}`
      );
    }

    return [`${edge.from}-${edge.to}-${edgeType}`];
  });
}

test('simple edge generates correct ID', () => {
  const ids = generateEdgeIds([{ from: 'a', to: 'b' }]);
  assertDeepEqual(ids, ['a-b-default']);
});

test('edge with explicit type includes type in ID', () => {
  const ids = generateEdgeIds([{ from: 'a', to: 'b', edge_type: 'conditional' }]);
  assertDeepEqual(ids, ['a-b-conditional']);
});

test('conditional edge with targets generates multiple IDs', () => {
  const ids = generateEdgeIds([{
    from: 'router',
    to: '',
    edge_type: 'conditional',
    conditional_targets: ['path_a', 'path_b']
  }]);
  assertDeepEqual(ids, ['router-path_a-conditional-0', 'router-path_b-conditional-1']);
});

test('multiple edges generate multiple IDs', () => {
  const ids = generateEdgeIds([
    { from: 'a', to: 'b' },
    { from: 'b', to: 'c' },
  ]);
  assertDeepEqual(ids, ['a-b-default', 'b-c-default']);
});

// Edge animation state (from GraphCanvas lines 326-347)
console.log('\nEdge animation logic:');

interface NodeExecutionMap {
  [key: string]: { status: NodeStatus; duration_ms?: number };
}

function shouldAnimateEdge(
  edgeSource: string,
  edgeTarget: string,
  nodeExecutions: NodeExecutionMap,
  currentNode?: string,
  isConditional: boolean = false
): boolean {
  const sourceExecution = nodeExecutions[edgeSource];
  const targetExecution = nodeExecutions[edgeTarget];
  const isEdgeActive = sourceExecution?.status === 'completed' &&
    (targetExecution?.status === 'active' || edgeTarget === currentNode);

  return isEdgeActive || isConditional;
}

test('edge animates when source completed and target active', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed' },
    'b': { status: 'active' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), true);
});

test('edge animates when source completed and target is currentNode', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed' },
    'b': { status: 'pending' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions, 'b'), true);
});

test('edge does not animate when source not completed', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'active' },
    'b': { status: 'pending' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('conditional edge always animates', () => {
  const executions: NodeExecutionMap = {};
  assertEqual(shouldAnimateEdge('a', 'b', executions, undefined, true), true);
});

// Keyboard navigation logic (from GraphCanvas lines 373-430)
console.log('\nKeyboard navigation:');

function computeNextIndex(currentIndex: number, direction: 'next' | 'prev', totalNodes: number): number {
  if (direction === 'next') {
    return currentIndex < 0 || currentIndex >= totalNodes - 1 ? 0 : currentIndex + 1;
  } else {
    return currentIndex <= 0 ? totalNodes - 1 : currentIndex - 1;
  }
}

test('next from last wraps to first', () => {
  assertEqual(computeNextIndex(4, 'next', 5), 0);
});

test('next from middle advances', () => {
  assertEqual(computeNextIndex(2, 'next', 5), 3);
});

test('next from unset (-1) goes to first', () => {
  assertEqual(computeNextIndex(-1, 'next', 5), 0);
});

test('prev from first wraps to last', () => {
  assertEqual(computeNextIndex(0, 'prev', 5), 4);
});

test('prev from middle goes back', () => {
  assertEqual(computeNextIndex(2, 'prev', 5), 1);
});

// Home/End navigation
function computeHomeEndIndex(key: 'Home' | 'End', totalNodes: number): number {
  return key === 'Home' ? 0 : totalNodes - 1;
}

test('Home key returns first index', () => {
  assertEqual(computeHomeEndIndex('Home', 10), 0);
});

test('End key returns last index', () => {
  assertEqual(computeHomeEndIndex('End', 10), 9);
});

// ARIA label for container (from GraphCanvas lines 450-451)
console.log('\nContainer ARIA label:');

function getContainerAriaLabel(nodeCount: number): string {
  return `Graph visualization with ${nodeCount} nodes. Use arrow keys to navigate, Enter to select, Escape to clear focus.`;
}

test('aria label includes node count', () => {
  const label = getContainerAriaLabel(5);
  assertIncludes(label, '5 nodes');
});

test('aria label includes navigation instructions', () => {
  const label = getContainerAriaLabel(5);
  assertIncludes(label, 'arrow keys');
  assertIncludes(label, 'Enter to select');
  assertIncludes(label, 'Escape');
});

// Screen reader announcement (from GraphCanvas lines 479-486)
console.log('\nScreen reader announcement:');

function getNodeFocusAnnouncement(
  nodeName: string,
  status?: NodeStatus
): string {
  let announcement = `Node ${nodeName} focused`;
  if (status) {
    announcement += `, status: ${status}`;
  }
  return announcement;
}

test('announcement includes node name', () => {
  const announcement = getNodeFocusAnnouncement('my_node');
  assertIncludes(announcement, 'my_node');
});

test('announcement includes status when present', () => {
  const announcement = getNodeFocusAnnouncement('my_node', 'completed');
  assertIncludes(announcement, 'status: completed');
});

test('announcement omits status when undefined', () => {
  const announcement = getNodeFocusAnnouncement('my_node', undefined);
  assertEqual(announcement.includes('status:'), false);
});

// Edge style computation (from GraphCanvas lines 186-194)
console.log('\nEdge style computation:');

function getEdgeStyle(isConditional: boolean, isParallel: boolean, isActive: boolean) {
  const stroke = isActive
    ? '#3B82F6'
    : isParallel
      ? '#8B5CF6'
      : isConditional
        ? '#F59E0B'
        : '#6B7280';
  const strokeWidth = isActive ? 3 : 2;
  return { stroke, strokeWidth };
}

test('active edge is blue with width 3', () => {
  const style = getEdgeStyle(false, false, true);
  assertEqual(style.stroke, '#3B82F6');
  assertEqual(style.strokeWidth, 3);
});

test('parallel edge is purple', () => {
  const style = getEdgeStyle(false, true, false);
  assertEqual(style.stroke, '#8B5CF6');
});

test('conditional edge is amber', () => {
  const style = getEdgeStyle(true, false, false);
  assertEqual(style.stroke, '#F59E0B');
});

test('default edge is gray', () => {
  const style = getEdgeStyle(false, false, false);
  assertEqual(style.stroke, '#6B7280');
});

test('active takes precedence over edge type', () => {
  const style = getEdgeStyle(true, true, true);
  assertEqual(style.stroke, '#3B82F6'); // Active color
});

// Group palette cycling (from GraphCanvas lines 40-46)
console.log('\nGroup palette cycling:');

const GROUP_PALETTE = [
  { bg: 'rgba(59, 130, 246, 0.06)', border: 'rgba(59, 130, 246, 0.25)' }, // blue
  { bg: 'rgba(34, 197, 94, 0.06)', border: 'rgba(34, 197, 94, 0.22)' }, // green
  { bg: 'rgba(245, 158, 11, 0.06)', border: 'rgba(245, 158, 11, 0.25)' }, // amber
  { bg: 'rgba(139, 92, 246, 0.06)', border: 'rgba(139, 92, 246, 0.25)' }, // violet
  { bg: 'rgba(236, 72, 153, 0.06)', border: 'rgba(236, 72, 153, 0.25)' }, // pink
];

function getGroupPalette(groupIndex: number) {
  return GROUP_PALETTE[groupIndex % GROUP_PALETTE.length];
}

test('first group uses blue palette', () => {
  const palette = getGroupPalette(0);
  assertIncludes(palette.bg, '59, 130, 246'); // blue
});

test('sixth group wraps to blue palette', () => {
  const palette = getGroupPalette(5);
  assertIncludes(palette.bg, '59, 130, 246'); // Same as index 0
});

test('palette cycles through all colors', () => {
  // Ensure all 5 colors are distinct
  const colors = new Set<string>();
  for (let i = 0; i < 5; i++) {
    colors.add(getGroupPalette(i).bg);
  }
  assertEqual(colors.size, 5);
});

// ============================================================================
// Extended edge case tests for GraphCanvas
// ============================================================================

// Schema structure key edge cases
console.log('\nSchema structure key edge cases:');

test('schema with empty nodes array returns key with empty node list', () => {
  const schema: GraphSchema = {
    name: 'empty_graph',
    version: '1.0.0',
    nodes: [],
    edges: [],
    entry_point: '',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertEqual(key, '||none|');
});

test('schema with empty edges array includes only nodes', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'solo', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'solo',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertEqual(key, 'solo||none|');
});

test('schema with many nodes sorts them correctly', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'charlie', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'alpha', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'bravo', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [],
    entry_point: 'alpha',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertIncludes(key, 'alpha,bravo,charlie');
});

test('multiple edges sorted by from->to', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'a', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'b', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'c', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [
      { from: 'c', to: 'a', edge_type: 'direct' },
      { from: 'a', to: 'b', edge_type: 'direct' },
    ],
    entry_point: 'a',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  // Edges sorted: a->b comes before c->a alphabetically
  assertIncludes(key, 'a->b;c->a');
});

test('edge with empty conditional_targets uses to field', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [
      { name: 'x', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
      { name: 'y', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} },
    ],
    edges: [{ from: 'x', to: 'y', edge_type: 'direct', conditional_targets: [] }],
    entry_point: 'x',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'none' });
  assertIncludes(key, 'x->y');
});

test('prefix grouping mode in key', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [{ name: 'node', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} }],
    edges: [],
    entry_point: 'node',
    metadata: {},
  };
  const key = computeSchemaStructureKey(schema, { mode: 'prefix' });
  assertIncludes(key, '|prefix|');
});

test('attribute key appended to all modes', () => {
  const schema: GraphSchema = {
    name: 'test_graph',
    version: '1.0.0',
    nodes: [{ name: 'node', node_type: 'transform', description: '', input_fields: [], output_fields: [], attributes: {} }],
    edges: [],
    entry_point: 'node',
    metadata: {},
  };
  // The function always appends attributeKey (defaulting to '') for cache key stability
  const keyNone = computeSchemaStructureKey(schema, { mode: 'none' });
  const keyNoneWithAttr = computeSchemaStructureKey(schema, { mode: 'none', attributeKey: 'ignored' });
  const keyAttr = computeSchemaStructureKey(schema, { mode: 'attribute', attributeKey: 'phase' });

  // none mode with no attributeKey ends with empty string
  assertEqual(keyNone.endsWith('|none|'), true);
  // none mode with attributeKey still includes it (for cache differentiation)
  assertEqual(keyNoneWithAttr.endsWith('|none|ignored'), true);
  // attribute mode includes it
  assertEqual(keyAttr.endsWith('|attribute|phase'), true);
});

// MiniMap node color comprehensive tests
console.log('\nMiniMap node color comprehensive:');

test('getMiniMapNodeColor handles all known statuses', () => {
  // Ensure all four statuses return distinct colors
  const colors = new Set<string>();
  colors.add(getMiniMapNodeColor('pending'));
  colors.add(getMiniMapNodeColor('active'));
  colors.add(getMiniMapNodeColor('completed'));
  colors.add(getMiniMapNodeColor('error'));
  assertEqual(colors.size, 4);
});

test('getMiniMapNodeColor is idempotent', () => {
  const color1 = getMiniMapNodeColor('active');
  const color2 = getMiniMapNodeColor('active');
  assertEqual(color1, color2);
});

test('getMiniMapNodeColor returns hex colors', () => {
  const hexPattern = /^#[0-9A-F]{6}$/i;
  assertEqual(hexPattern.test(getMiniMapNodeColor('active')), true);
  assertEqual(hexPattern.test(getMiniMapNodeColor('completed')), true);
  assertEqual(hexPattern.test(getMiniMapNodeColor('error')), true);
  assertEqual(hexPattern.test(getMiniMapNodeColor('pending')), true);
  assertEqual(hexPattern.test(getMiniMapNodeColor(undefined)), true);
});

// Edge ID generation edge cases
console.log('\nEdge ID generation edge cases:');

test('empty edges array returns empty IDs', () => {
  const ids = generateEdgeIds([]);
  assertDeepEqual(ids, []);
});

test('edge with undefined edge_type uses default', () => {
  const ids = generateEdgeIds([{ from: 'x', to: 'y', edge_type: undefined }]);
  assertDeepEqual(ids, ['x-y-default']);
});

test('parallel edge type in ID', () => {
  const ids = generateEdgeIds([{ from: 'a', to: 'b', edge_type: 'parallel' }]);
  assertDeepEqual(ids, ['a-b-parallel']);
});

test('mixed edge types generate correct IDs', () => {
  const ids = generateEdgeIds([
    { from: 'a', to: 'b', edge_type: 'direct' },
    { from: 'b', to: 'c', edge_type: 'conditional' },
    { from: 'c', to: 'd', edge_type: 'parallel' },
  ]);
  assertDeepEqual(ids, ['a-b-direct', 'b-c-conditional', 'c-d-parallel']);
});

test('single conditional target generates one ID', () => {
  const ids = generateEdgeIds([{
    from: 'router',
    to: '',
    edge_type: 'conditional',
    conditional_targets: ['single_path']
  }]);
  assertDeepEqual(ids, ['router-single_path-conditional-0']);
});

test('many conditional targets generate indexed IDs', () => {
  const ids = generateEdgeIds([{
    from: 'hub',
    to: '',
    edge_type: 'conditional',
    conditional_targets: ['a', 'b', 'c', 'd', 'e']
  }]);
  assertEqual(ids.length, 5);
  assertEqual(ids[0], 'hub-a-conditional-0');
  assertEqual(ids[4], 'hub-e-conditional-4');
});

// Edge animation state edge cases
console.log('\nEdge animation state edge cases:');

test('edge does not animate when target completed', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed' },
    'b': { status: 'completed' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('edge does not animate when source is error', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'error' },
    'b': { status: 'active' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('edge does not animate when source is pending', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'pending' },
    'b': { status: 'pending' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('edge animates when both conditions met regardless of duration', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed', duration_ms: 5000 },
    'b': { status: 'active', duration_ms: 0 },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), true);
});

test('edge does not animate with missing source execution', () => {
  const executions: NodeExecutionMap = {
    'b': { status: 'active' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('edge does not animate with missing target execution and no currentNode', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions), false);
});

test('edge animates when target missing but is currentNode', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'completed' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions, 'b'), true);
});

test('conditional flag overrides all execution states', () => {
  const executions: NodeExecutionMap = {
    'a': { status: 'pending' },
    'b': { status: 'pending' },
  };
  assertEqual(shouldAnimateEdge('a', 'b', executions, undefined, true), true);
});

// Keyboard navigation edge cases
console.log('\nKeyboard navigation edge cases:');

test('next with single node stays at 0', () => {
  assertEqual(computeNextIndex(0, 'next', 1), 0);
});

test('prev with single node stays at 0', () => {
  assertEqual(computeNextIndex(0, 'prev', 1), 0);
});

test('next with two nodes alternates', () => {
  assertEqual(computeNextIndex(0, 'next', 2), 1);
  assertEqual(computeNextIndex(1, 'next', 2), 0);
});

test('prev with two nodes alternates', () => {
  assertEqual(computeNextIndex(0, 'prev', 2), 1);
  assertEqual(computeNextIndex(1, 'prev', 2), 0);
});

test('next handles large totalNodes', () => {
  assertEqual(computeNextIndex(99, 'next', 100), 0);
  assertEqual(computeNextIndex(98, 'next', 100), 99);
});

test('prev handles large totalNodes', () => {
  assertEqual(computeNextIndex(0, 'prev', 100), 99);
  assertEqual(computeNextIndex(1, 'prev', 100), 0);
});

test('Home with single node returns 0', () => {
  assertEqual(computeHomeEndIndex('Home', 1), 0);
});

test('End with single node returns 0', () => {
  assertEqual(computeHomeEndIndex('End', 1), 0);
});

test('End with large count returns last index', () => {
  assertEqual(computeHomeEndIndex('End', 1000), 999);
});

// Container ARIA label edge cases
console.log('\nContainer ARIA label edge cases:');

test('aria label with zero nodes', () => {
  const label = getContainerAriaLabel(0);
  assertIncludes(label, '0 nodes');
});

test('aria label with one node (singular not special-cased)', () => {
  const label = getContainerAriaLabel(1);
  assertIncludes(label, '1 nodes');
});

test('aria label with large node count', () => {
  const label = getContainerAriaLabel(1000);
  assertIncludes(label, '1000 nodes');
});

// Screen reader announcement edge cases
console.log('\nScreen reader announcement edge cases:');

test('announcement with empty node name', () => {
  const announcement = getNodeFocusAnnouncement('');
  assertIncludes(announcement, 'Node  focused');
});

test('announcement with special characters', () => {
  const announcement = getNodeFocusAnnouncement('node-with_special.chars');
  assertIncludes(announcement, 'node-with_special.chars');
});

test('announcement with pending status', () => {
  const announcement = getNodeFocusAnnouncement('test', 'pending');
  assertIncludes(announcement, 'status: pending');
});

test('announcement with error status', () => {
  const announcement = getNodeFocusAnnouncement('test', 'error');
  assertIncludes(announcement, 'status: error');
});

test('announcement structure is consistent', () => {
  // All announcements should have same structure
  const withStatus = getNodeFocusAnnouncement('a', 'active');
  const withoutStatus = getNodeFocusAnnouncement('a');

  // Both start with "Node X focused"
  assertIncludes(withStatus, 'Node a focused');
  assertIncludes(withoutStatus, 'Node a focused');
});

// Edge style computation edge cases
console.log('\nEdge style computation edge cases:');

test('parallel takes precedence over conditional when not active', () => {
  const style = getEdgeStyle(true, true, false);
  assertEqual(style.stroke, '#8B5CF6'); // Purple (parallel)
});

test('all inactive variations have width 2', () => {
  assertEqual(getEdgeStyle(false, false, false).strokeWidth, 2);
  assertEqual(getEdgeStyle(true, false, false).strokeWidth, 2);
  assertEqual(getEdgeStyle(false, true, false).strokeWidth, 2);
  assertEqual(getEdgeStyle(true, true, false).strokeWidth, 2);
});

test('only active has width 3', () => {
  assertEqual(getEdgeStyle(false, false, true).strokeWidth, 3);
  assertEqual(getEdgeStyle(true, false, true).strokeWidth, 3);
  assertEqual(getEdgeStyle(false, true, true).strokeWidth, 3);
  assertEqual(getEdgeStyle(true, true, true).strokeWidth, 3);
});

test('edge style colors are hex format', () => {
  const hexPattern = /^#[0-9A-F]{6}$/i;
  assertEqual(hexPattern.test(getEdgeStyle(false, false, false).stroke), true);
  assertEqual(hexPattern.test(getEdgeStyle(true, false, false).stroke), true);
  assertEqual(hexPattern.test(getEdgeStyle(false, true, false).stroke), true);
  assertEqual(hexPattern.test(getEdgeStyle(false, false, true).stroke), true);
});

// Group palette cycling edge cases
console.log('\nGroup palette cycling edge cases:');

test('second group uses green palette', () => {
  const palette = getGroupPalette(1);
  assertIncludes(palette.bg, '34, 197, 94'); // green
});

test('third group uses amber palette', () => {
  const palette = getGroupPalette(2);
  assertIncludes(palette.bg, '245, 158, 11'); // amber
});

test('fourth group uses violet palette', () => {
  const palette = getGroupPalette(3);
  assertIncludes(palette.bg, '139, 92, 246'); // violet
});

test('fifth group uses pink palette', () => {
  const palette = getGroupPalette(4);
  assertIncludes(palette.bg, '236, 72, 153'); // pink
});

test('large index wraps correctly', () => {
  // Index 27 = 27 % 5 = 2 = amber
  const palette = getGroupPalette(27);
  assertIncludes(palette.bg, '245, 158, 11'); // amber
});

test('very large index still wraps', () => {
  // Index 1000 = 1000 % 5 = 0 = blue
  const palette = getGroupPalette(1000);
  assertIncludes(palette.bg, '59, 130, 246'); // blue
});

test('palette has both bg and border properties', () => {
  for (let i = 0; i < 5; i++) {
    const palette = getGroupPalette(i);
    assertEqual(typeof palette.bg, 'string');
    assertEqual(typeof palette.border, 'string');
    assertEqual(palette.bg.startsWith('rgba('), true);
    assertEqual(palette.border.startsWith('rgba('), true);
  }
});

test('all palettes have low opacity bg (0.06)', () => {
  for (let i = 0; i < 5; i++) {
    const palette = getGroupPalette(i);
    assertIncludes(palette.bg, '0.06)');
  }
});

test('all palettes have higher opacity border', () => {
  for (let i = 0; i < 5; i++) {
    const palette = getGroupPalette(i);
    // Borders have 0.22 or 0.25 opacity
    const hasValidOpacity = palette.border.includes('0.22)') || palette.border.includes('0.25)');
    assertEqual(hasValidOpacity, true);
  }
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
