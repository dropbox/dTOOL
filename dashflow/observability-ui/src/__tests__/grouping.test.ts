// Comprehensive tests for node grouping utilities
// Run with: npx tsx src/__tests__/grouping.test.ts

import { computeNodeGroups, GroupingOptions, GroupingMode } from '../utils/grouping';
import { GraphSchema, NodeSchema, NodeType } from '../types/graph';

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

function assertDefined<T>(value: T | undefined | null, message?: string): asserts value is T {
  if (value === undefined || value === null) {
    throw new Error(message || 'Expected value to be defined');
  }
}

// Helper to create minimal node schema
function makeNode(
  name: string,
  nodeType: NodeType = 'transform',
  attributes: Record<string, string> = {}
): NodeSchema {
  return {
    name,
    node_type: nodeType,
    input_fields: [],
    output_fields: [],
    attributes,
  };
}

// Helper to create minimal graph schema
function makeSchema(nodes: NodeSchema[]): GraphSchema {
  return {
    name: 'test_graph',
    version: '1.0',
    nodes,
    edges: [],
    entry_point: nodes[0]?.name || 'start',
    metadata: {},
  };
}

console.log('\nNode Grouping Tests\n');

// ============================================================================
// Mode: none
// ============================================================================
console.log('mode: none:');

test('returns empty array when mode is none', () => {
  const schema = makeSchema([
    makeNode('node_a', 'llm'),
    makeNode('node_b', 'tool'),
  ]);
  const options: GroupingOptions = { mode: 'none' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 0, 'groups');
});

test('returns empty array for empty schema with mode none', () => {
  const schema = makeSchema([]);
  const options: GroupingOptions = { mode: 'none' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 0, 'groups');
});

test('returns empty array for large schema with mode none', () => {
  const nodes = Array.from({ length: 100 }, (_, i) => makeNode(`node_${i}`, 'llm'));
  const schema = makeSchema(nodes);
  const options: GroupingOptions = { mode: 'none' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 0, 'groups');
});

// ============================================================================
// Mode: node_type
// ============================================================================
console.log('\nmode: node_type:');

test('groups nodes by node_type', () => {
  const schema = makeSchema([
    makeNode('node_a', 'llm'),
    makeNode('node_b', 'llm'),
    makeNode('node_c', 'tool'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  const llmGroup = groups.find((g) => g.label === 'LLM');
  const toolGroup = groups.find((g) => g.label === 'Tool');
  assertDefined(llmGroup, 'LLM group exists');
  assertDefined(toolGroup, 'Tool group exists');
  assertEqual(llmGroup.nodes.length, 2, 'LLM group nodes');
  assertEqual(toolGroup.nodes.length, 1, 'Tool group nodes');
});

test('sorts nodes within groups alphabetically', () => {
  const schema = makeSchema([
    makeNode('charlie', 'llm'),
    makeNode('alpha', 'llm'),
    makeNode('bravo', 'llm'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'number of groups');
  assertEqual(groups[0].nodes[0].name, 'alpha', 'first node');
  assertEqual(groups[0].nodes[1].name, 'bravo', 'second node');
  assertEqual(groups[0].nodes[2].name, 'charlie', 'third node');
});

test('groups are sorted by label alphabetically', () => {
  const schema = makeSchema([
    makeNode('node_a', 'router'),
    makeNode('node_b', 'llm'),
    makeNode('node_c', 'aggregator'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].label, 'Aggregator', 'first group');
  assertEqual(groups[1].label, 'LLM', 'second group');
  assertEqual(groups[2].label, 'Router', 'third group');
});

test('group key includes node_type prefix', () => {
  const schema = makeSchema([makeNode('node_a', 'llm')]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].key, 'node_type:llm', 'key');
});

// ============================================================================
// Label formatting for all node types
// ============================================================================
console.log('\nlabel formatting for all node types:');

test('formats transform as Transform', () => {
  const schema = makeSchema([makeNode('n', 'transform')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Transform', 'label');
});

test('formats llm as LLM (special case)', () => {
  const schema = makeSchema([makeNode('n', 'llm')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'LLM', 'label');
});

test('formats tool as Tool', () => {
  const schema = makeSchema([makeNode('n', 'tool')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Tool', 'label');
});

test('formats router as Router', () => {
  const schema = makeSchema([makeNode('n', 'router')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Router', 'label');
});

test('formats aggregator as Aggregator', () => {
  const schema = makeSchema([makeNode('n', 'aggregator')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Aggregator', 'label');
});

test('formats validator as Validator', () => {
  const schema = makeSchema([makeNode('n', 'validator')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Validator', 'label');
});

test('formats human_in_loop as Human-in-Loop (special case)', () => {
  const schema = makeSchema([makeNode('n', 'human_in_loop')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Human-in-Loop', 'label');
});

test('formats checkpoint as Checkpoint', () => {
  const schema = makeSchema([makeNode('n', 'checkpoint')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Checkpoint', 'label');
});

test('formats custom as Custom', () => {
  const schema = makeSchema([makeNode('n', 'custom')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Custom', 'label');
});

test('creates all 9 node type groups when all types present', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform'),
    makeNode('n2', 'llm'),
    makeNode('n3', 'tool'),
    makeNode('n4', 'router'),
    makeNode('n5', 'aggregator'),
    makeNode('n6', 'validator'),
    makeNode('n7', 'human_in_loop'),
    makeNode('n8', 'checkpoint'),
    makeNode('n9', 'custom'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 9, 'number of groups');
  const labels = groups.map((g) => g.label).sort();
  assertEqual(labels, [
    'Aggregator',
    'Checkpoint',
    'Custom',
    'Human-in-Loop',
    'LLM',
    'Router',
    'Tool',
    'Transform',
    'Validator',
  ], 'all labels');
});

// ============================================================================
// Mode: attribute
// ============================================================================
console.log('\nmode: attribute:');

test('groups nodes by custom attribute', () => {
  const schema = makeSchema([
    makeNode('node_a', 'transform', { team: 'frontend' }),
    makeNode('node_b', 'transform', { team: 'backend' }),
    makeNode('node_c', 'transform', { team: 'frontend' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  const frontendGroup = groups.find((g) => g.label === 'frontend');
  const backendGroup = groups.find((g) => g.label === 'backend');
  assertDefined(frontendGroup, 'frontend group exists');
  assertDefined(backendGroup, 'backend group exists');
  assertEqual(frontendGroup.nodes.length, 2, 'frontend group nodes');
  assertEqual(backendGroup.nodes.length, 1, 'backend group nodes');
});

test('uses (missing) label for nodes without attribute', () => {
  const schema = makeSchema([
    makeNode('node_a', 'transform', { team: 'frontend' }),
    makeNode('node_b', 'transform', {}),
    makeNode('node_c', 'transform', { team: '' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  const missingGroup = groups.find((g) => g.label === '(missing)');
  assertDefined(missingGroup, '(missing) group exists');
  assertEqual(missingGroup.nodes.length, 2, 'nodes without team attribute');
});

test('uses default "group" key when attributeKey is undefined', () => {
  const schema = makeSchema([
    makeNode('node_a', 'transform', { group: 'groupA' }),
    makeNode('node_b', 'transform', { group: 'groupB' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  assertTrue(groups.some((g) => g.label === 'groupA'), 'groupA exists');
  assertTrue(groups.some((g) => g.label === 'groupB'), 'groupB exists');
});

test('trims whitespace from attribute values', () => {
  const schema = makeSchema([
    makeNode('node_a', 'transform', { team: '  frontend  ' }),
    makeNode('node_b', 'transform', { team: 'frontend' }),
    makeNode('node_c', 'transform', { team: '\tfrontend\n' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'all in same group');
  assertEqual(groups[0].label, 'frontend', 'label is trimmed');
  assertEqual(groups[0].nodes.length, 3, 'all 3 nodes');
});

test('group key includes attr prefix and attribute key', () => {
  const schema = makeSchema([makeNode('node_a', 'llm', { team: 'backend' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].key, 'attr:team:backend', 'key');
});

test('handles attribute key with whitespace', () => {
  const schema = makeSchema([makeNode('node_a', 'llm', { group: 'test' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: '  group  ' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'groups');
  assertEqual(groups[0].label, 'test', 'label');
});

test('handles empty attributeKey by using default "group"', () => {
  const schema = makeSchema([makeNode('node_a', 'llm', { group: 'mygroup' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: '' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'groups');
  assertEqual(groups[0].label, 'mygroup', 'label');
});

test('sorts attribute groups alphabetically by label', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { category: 'zebra' }),
    makeNode('n2', 'transform', { category: 'alpha' }),
    makeNode('n3', 'transform', { category: 'middle' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'category' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].label, 'alpha', 'first');
  assertEqual(groups[1].label, 'middle', 'second');
  assertEqual(groups[2].label, 'zebra', 'third');
});

test('handles Unicode attribute values', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { region: '日本' }),
    makeNode('n2', 'transform', { region: '中国' }),
    makeNode('n3', 'transform', { region: '日本' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'region' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  assertTrue(groups.some((g) => g.label === '日本'), 'Japan group');
  assertTrue(groups.some((g) => g.label === '中国'), 'China group');
  const japanGroup = groups.find((g) => g.label === '日本');
  assertDefined(japanGroup, 'Japan group exists');
  assertEqual(japanGroup.nodes.length, 2, 'Japan nodes');
});

test('handles emoji attribute values', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { priority: '🔥' }),
    makeNode('n2', 'transform', { priority: '⭐' }),
    makeNode('n3', 'transform', { priority: '🔥' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'priority' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  const fireGroup = groups.find((g) => g.label === '🔥');
  assertDefined(fireGroup, 'fire group exists');
  assertEqual(fireGroup.nodes.length, 2, 'fire nodes');
});

test('treats attribute values as case-sensitive', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { team: 'Frontend' }),
    makeNode('n2', 'transform', { team: 'frontend' }),
    makeNode('n3', 'transform', { team: 'FRONTEND' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 3, 'case-sensitive creates 3 groups');
});

test('handles special characters in attribute values', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { path: '/api/v1' }),
    makeNode('n2', 'transform', { path: '/api/v2' }),
    makeNode('n3', 'transform', { path: '/api/v1' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'path' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  const v1Group = groups.find((g) => g.label === '/api/v1');
  assertDefined(v1Group, 'v1 group exists');
  assertEqual(v1Group.nodes.length, 2, 'v1 nodes');
});

test('handles numeric-like attribute values as strings', () => {
  const schema = makeSchema([
    makeNode('n1', 'transform', { version: '1' }),
    makeNode('n2', 'transform', { version: '2' }),
    makeNode('n3', 'transform', { version: '10' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'version' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 3, 'number of groups');
  // String sorting: "1", "10", "2"
  assertEqual(groups[0].label, '1', 'first');
  assertEqual(groups[1].label, '10', 'second');
  assertEqual(groups[2].label, '2', 'third');
});

// ============================================================================
// Edge cases
// ============================================================================
console.log('\nedge cases:');

test('handles empty schema', () => {
  const schema = makeSchema([]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 0, 'empty schema');
});

test('handles single node', () => {
  const schema = makeSchema([makeNode('only_node', 'llm')]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'number of groups');
  assertEqual(groups[0].nodes.length, 1, 'nodes in group');
});

test('handles nodes with same name in different groups by node_type', () => {
  const schema = makeSchema([
    makeNode('processor', 'llm'),
    makeNode('processor', 'tool'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 2, 'number of groups');
  assertEqual(groups[0].nodes.length, 1, 'first group');
  assertEqual(groups[1].nodes.length, 1, 'second group');
});

test('handles very long attribute values', () => {
  const longValue = 'a'.repeat(10000);
  const schema = makeSchema([makeNode('n1', 'transform', { desc: longValue })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'desc' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'number of groups');
  assertEqual(groups[0].label, longValue, 'long label preserved');
});

test('handles nodes with undefined attributes object', () => {
  const node: NodeSchema = {
    name: 'no_attrs',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: undefined as unknown as Record<string, string>,
  };
  const schema = makeSchema([node]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'number of groups');
  assertEqual(groups[0].label, '(missing)', 'missing label');
});

test('handles mixed node types and attributes consistently', () => {
  const schema = makeSchema([
    makeNode('a', 'llm', { team: 'ai' }),
    makeNode('b', 'tool', { team: 'ai' }),
    makeNode('c', 'llm', { team: 'data' }),
    makeNode('d', 'router'),
  ]);
  const byType = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(byType.length, 3, 'by type: llm, tool, router');
  const byAttr = computeNodeGroups(schema, { mode: 'attribute', attributeKey: 'team' });
  assertEqual(byAttr.length, 3, 'by attr: ai, data, (missing)');
});

// ============================================================================
// Performance and scale
// ============================================================================
console.log('\nperformance and scale:');

test('handles large number of nodes efficiently', () => {
  const nodeTypes: NodeType[] = ['llm', 'tool', 'transform'];
  const nodes = Array.from({ length: 1000 }, (_, i) =>
    makeNode(`node_${i}`, nodeTypes[i % 3])
  );
  const schema = makeSchema(nodes);
  const options: GroupingOptions = { mode: 'node_type' };
  const start = performance.now();
  const groups = computeNodeGroups(schema, options);
  const duration = performance.now() - start;
  assertEqual(groups.length, 3, 'number of groups');
  const totalNodes = groups.reduce((sum, g) => sum + g.nodes.length, 0);
  assertEqual(totalNodes, 1000, 'total nodes');
  assertTrue(duration < 100, `completed in ${duration.toFixed(2)}ms (< 100ms)`);
});

test('handles many unique attribute values', () => {
  const nodes = Array.from({ length: 500 }, (_, i) =>
    makeNode(`node_${i}`, 'transform', { uniqueId: `id_${i}` })
  );
  const schema = makeSchema(nodes);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'uniqueId' };
  const start = performance.now();
  const groups = computeNodeGroups(schema, options);
  const duration = performance.now() - start;
  assertEqual(groups.length, 500, 'number of groups');
  assertTrue(duration < 100, `completed in ${duration.toFixed(2)}ms (< 100ms)`);
});

// ============================================================================
// NodeGroup structure
// ============================================================================
console.log('\nNodeGroup structure:');

test('returns groups with correct structure', () => {
  const schema = makeSchema([makeNode('test_node', 'llm')]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertTrue('key' in groups[0], 'has key');
  assertTrue('label' in groups[0], 'has label');
  assertTrue('nodes' in groups[0], 'has nodes');
  assertTrue(Array.isArray(groups[0].nodes), 'nodes is array');
});

test('nodes in groups preserve original properties', () => {
  const originalNode = makeNode('my_node', 'tool', { custom: 'value' });
  originalNode.input_fields = ['input1', 'input2'];
  originalNode.output_fields = ['output1'];
  const schema = makeSchema([originalNode]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  const groupedNode = groups[0].nodes[0];
  assertEqual(groupedNode.name, 'my_node', 'name');
  assertEqual(groupedNode.node_type, 'tool', 'node_type');
  assertEqual(groupedNode.attributes, { custom: 'value' }, 'attributes');
  assertEqual(groupedNode.input_fields, ['input1', 'input2'], 'input_fields');
  assertEqual(groupedNode.output_fields, ['output1'], 'output_fields');
});

// ============================================================================
// GroupingMode type safety
// ============================================================================
console.log('\nGroupingMode type safety:');

test('accepts all valid GroupingMode values', () => {
  const schema = makeSchema([makeNode('n', 'llm')]);
  const modes: GroupingMode[] = ['none', 'node_type', 'attribute'];
  for (const mode of modes) {
    const groups = computeNodeGroups(schema, { mode });
    assertTrue(Array.isArray(groups), `mode ${mode} returns array`);
  }
});

// ============================================================================
// Deterministic output
// ============================================================================
console.log('\ndeterministic output:');

test('produces same output for same input (deterministic)', () => {
  const schema = makeSchema([
    makeNode('z_node', 'tool'),
    makeNode('a_node', 'llm'),
    makeNode('m_node', 'tool'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups1 = computeNodeGroups(schema, options);
  const groups2 = computeNodeGroups(schema, options);
  assertEqual(JSON.stringify(groups1), JSON.stringify(groups2), 'same output');
});

test('node order in input does not affect output order', () => {
  const schema1 = makeSchema([
    makeNode('z_node', 'llm'),
    makeNode('a_node', 'llm'),
  ]);
  const schema2 = makeSchema([
    makeNode('a_node', 'llm'),
    makeNode('z_node', 'llm'),
  ]);
  const options: GroupingOptions = { mode: 'node_type' };
  const groups1 = computeNodeGroups(schema1, options);
  const groups2 = computeNodeGroups(schema2, options);
  assertEqual(groups1[0].nodes[0].name, 'a_node', 'schema1 first node');
  assertEqual(groups2[0].nodes[0].name, 'a_node', 'schema2 first node');
});

// ============================================================================
// Additional nodeTypeLabel formatting tests
// ============================================================================
console.log('\nadditional nodeTypeLabel formatting:');

test('formats node types with multiple underscores', () => {
  // Using type assertion since these are hypothetical types
  const schema = makeSchema([makeNode('n', 'data_processing_node' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Data Processing Node', 'multiple underscores');
});

test('formats node types with leading underscore', () => {
  const schema = makeSchema([makeNode('n', '_internal' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, ' Internal', 'leading underscore becomes space');
});

test('formats node types with trailing underscore', () => {
  const schema = makeSchema([makeNode('n', 'legacy_' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'Legacy ', 'trailing underscore becomes space');
});

test('formats single character node type', () => {
  const schema = makeSchema([makeNode('n', 'x' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'X', 'single char capitalized');
});

test('formats already capitalized node type', () => {
  const schema = makeSchema([makeNode('n', 'ALLCAPS' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  // Word boundary \b\w/g only capitalizes first letter of each word
  assertEqual(groups[0].label, 'ALLCAPS', 'preserves existing caps');
});

test('formats mixed case node type', () => {
  const schema = makeSchema([makeNode('n', 'camelCase' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'CamelCase', 'capitalizes first letter');
});

test('formats node type with numbers', () => {
  const schema = makeSchema([makeNode('n', 'v2_processor' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, 'V2 Processor', 'numbers preserved');
});

test('formats empty node type', () => {
  const schema = makeSchema([makeNode('n', '' as NodeType)]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].label, '', 'empty stays empty');
});

// ============================================================================
// Additional attribute edge cases
// ============================================================================
console.log('\nadditional attribute edge cases:');

test('handles attribute key with special characters', () => {
  const schema = makeSchema([makeNode('n', 'llm', { 'my-attr': 'value' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'my-attr' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'groups');
  assertEqual(groups[0].label, 'value', 'label');
  assertEqual(groups[0].key, 'attr:my-attr:value', 'key');
});

test('handles attribute key with dots', () => {
  const schema = makeSchema([makeNode('n', 'llm', { 'config.env': 'prod' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'config.env' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 1, 'groups');
  assertEqual(groups[0].label, 'prod', 'label');
});

test('handles attribute value with colons (key delimiter)', () => {
  const schema = makeSchema([makeNode('n', 'llm', { url: 'http://example.com' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'url' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].key, 'attr:url:http://example.com', 'key preserves colons');
});

test('handles attribute value with newlines', () => {
  const schema = makeSchema([makeNode('n', 'llm', { desc: 'line1\nline2' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'desc' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].label, 'line1\nline2', 'newlines preserved');
});

test('handles whitespace-only attribute value as missing', () => {
  const schema = makeSchema([makeNode('n', 'llm', { team: '   ' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].label, '(missing)', 'whitespace-only is missing');
});

test('handles tab-only attribute value as missing', () => {
  const schema = makeSchema([makeNode('n', 'llm', { team: '\t\t' })]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'team' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups[0].label, '(missing)', 'tabs-only is missing');
});

test('handles null-like string attribute values', () => {
  const schema = makeSchema([
    makeNode('n1', 'llm', { value: 'null' }),
    makeNode('n2', 'llm', { value: 'undefined' }),
    makeNode('n3', 'llm', { value: 'false' }),
  ]);
  const options: GroupingOptions = { mode: 'attribute', attributeKey: 'value' };
  const groups = computeNodeGroups(schema, options);
  assertEqual(groups.length, 3, 'three distinct groups');
  assertTrue(groups.some((g) => g.label === 'null'), 'null string');
  assertTrue(groups.some((g) => g.label === 'undefined'), 'undefined string');
  assertTrue(groups.some((g) => g.label === 'false'), 'false string');
});

// ============================================================================
// Node name edge cases
// ============================================================================
console.log('\nnode name edge cases:');

test('handles nodes with empty string names', () => {
  const schema = makeSchema([
    makeNode('', 'llm'),
    makeNode('valid', 'llm'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].nodes.length, 2, 'both nodes in group');
  assertEqual(groups[0].nodes[0].name, '', 'empty name sorts first');
});

test('handles nodes with whitespace names', () => {
  const schema = makeSchema([
    makeNode('  ', 'llm'),
    makeNode('normal', 'llm'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].nodes.length, 2, 'both nodes in group');
});

test('handles nodes with Unicode names', () => {
  const schema = makeSchema([
    makeNode('αβγ', 'llm'),
    makeNode('δεζ', 'llm'),
    makeNode('abc', 'llm'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  // localeCompare sorts: abc, αβγ, δεζ (locale-dependent)
  assertEqual(groups[0].nodes.length, 3, 'all nodes in group');
});

test('handles nodes with numbers in names for sorting', () => {
  const schema = makeSchema([
    makeNode('node_10', 'llm'),
    makeNode('node_2', 'llm'),
    makeNode('node_1', 'llm'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  // String sorting: node_1, node_10, node_2
  assertEqual(groups[0].nodes[0].name, 'node_1', 'first');
  assertEqual(groups[0].nodes[1].name, 'node_10', 'second (string sort)');
  assertEqual(groups[0].nodes[2].name, 'node_2', 'third');
});

test('handles duplicate node names within same group', () => {
  const schema = makeSchema([
    makeNode('dup', 'llm'),
    makeNode('dup', 'llm'),
    makeNode('dup', 'llm'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].nodes.length, 3, 'all duplicates preserved');
  assertEqual(groups[0].nodes.every((n) => n.name === 'dup'), true, 'all named dup');
});

// ============================================================================
// Group key uniqueness and tie-breaking
// ============================================================================
console.log('\ngroup key uniqueness:');

test('groups with same label but different keys remain separate', () => {
  // This scenario would be unusual but tests the key uniqueness
  const node1 = makeNode('n1', 'transform', { labelA: 'shared' });
  const node2 = makeNode('n2', 'transform', { labelB: 'shared' });

  // Group by labelA - n2 will have (missing)
  const groupsA = computeNodeGroups(makeSchema([node1, node2]), { mode: 'attribute', attributeKey: 'labelA' });
  assertEqual(groupsA.length, 2, 'two groups by labelA');

  // Group by labelB - n1 will have (missing)
  const groupsB = computeNodeGroups(makeSchema([node1, node2]), { mode: 'attribute', attributeKey: 'labelB' });
  assertEqual(groupsB.length, 2, 'two groups by labelB');
});

test('group keys are unique even with same label value', () => {
  const schema = makeSchema([
    makeNode('n1', 'llm', { team: 'alpha' }),
    makeNode('n2', 'tool', { team: 'alpha' }),
  ]);
  // By node_type, both will have different keys
  const byType = computeNodeGroups(schema, { mode: 'node_type' });
  const keys = byType.map((g) => g.key);
  assertEqual(keys.length, new Set(keys).size, 'all keys unique');

  // By attribute, both in same group
  const byAttr = computeNodeGroups(schema, { mode: 'attribute', attributeKey: 'team' });
  assertEqual(byAttr.length, 1, 'same attribute value = one group');
});

test('sort stability when labels match', () => {
  // Create scenario where two groups could have same label in different contexts
  // Since labels come from different sources (node_type labels are formatted),
  // this tests the secondary sort by key
  const schema = makeSchema([
    makeNode('n1', 'llm'),
    makeNode('n2', 'tool'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  // Both have unique labels, but verify sort is deterministic
  const groups2 = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups[0].key, groups2[0].key, 'deterministic first group');
  assertEqual(groups[1].key, groups2[1].key, 'deterministic second group');
});

// ============================================================================
// Input mutation safety
// ============================================================================
console.log('\ninput mutation safety:');

test('does not mutate original schema', () => {
  const nodes = [makeNode('z', 'llm'), makeNode('a', 'llm')];
  const schema = makeSchema(nodes);
  const originalOrder = [...schema.nodes.map((n) => n.name)];
  computeNodeGroups(schema, { mode: 'node_type' });
  const afterOrder = schema.nodes.map((n) => n.name);
  assertEqual(afterOrder, originalOrder, 'original schema unchanged');
});

test('does not mutate original nodes', () => {
  const node = makeNode('test', 'llm', { key: 'value' });
  const originalAttrs = { ...node.attributes };
  const schema = makeSchema([node]);
  computeNodeGroups(schema, { mode: 'attribute', attributeKey: 'key' });
  assertEqual(node.attributes, originalAttrs, 'original attributes unchanged');
});

// ============================================================================
// Multiple groups interaction
// ============================================================================
console.log('\nmultiple groups interaction:');

test('handles schema where all nodes go to (missing) group', () => {
  const schema = makeSchema([
    makeNode('n1', 'llm'),
    makeNode('n2', 'tool'),
    makeNode('n3', 'transform'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'attribute', attributeKey: 'nonexistent' });
  assertEqual(groups.length, 1, 'all in one group');
  assertEqual(groups[0].label, '(missing)', 'label is missing');
  assertEqual(groups[0].nodes.length, 3, 'all three nodes');
});

test('handles schema with mix of same-type and different-type nodes', () => {
  const schema = makeSchema([
    makeNode('llm1', 'llm'),
    makeNode('llm2', 'llm'),
    makeNode('llm3', 'llm'),
    makeNode('tool1', 'tool'),
    makeNode('transform1', 'transform'),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'node_type' });
  assertEqual(groups.length, 3, 'three groups');
  const llmGroup = groups.find((g) => g.label === 'LLM');
  assertDefined(llmGroup, 'LLM group exists');
  assertEqual(llmGroup.nodes.length, 3, 'LLM has 3 nodes');
});

test('attribute grouping respects natural attribute distribution', () => {
  const schema = makeSchema([
    makeNode('n1', 'llm', { stage: 'dev' }),
    makeNode('n2', 'llm', { stage: 'dev' }),
    makeNode('n3', 'llm', { stage: 'staging' }),
    makeNode('n4', 'llm', { stage: 'staging' }),
    makeNode('n5', 'llm', { stage: 'prod' }),
  ]);
  const groups = computeNodeGroups(schema, { mode: 'attribute', attributeKey: 'stage' });
  assertEqual(groups.length, 3, 'three stages');
  const devGroup = groups.find((g) => g.label === 'dev');
  const stagingGroup = groups.find((g) => g.label === 'staging');
  const prodGroup = groups.find((g) => g.label === 'prod');
  assertDefined(devGroup, 'dev exists');
  assertDefined(stagingGroup, 'staging exists');
  assertDefined(prodGroup, 'prod exists');
  assertEqual(devGroup.nodes.length, 2, 'dev count');
  assertEqual(stagingGroup.nodes.length, 2, 'staging count');
  assertEqual(prodGroup.nodes.length, 1, 'prod count');
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
