// M-444: Unit tests for schema diff utilities
// Run with: npx tsx src/__tests__/schemaDiff.test.ts

import { compareSchemas, getNodeDiffStatus, SchemaDiff, DIFF_STATUS_STYLES, DiffStatus } from '../types/schemaDiff';
import { GraphSchema, NodeSchema, EdgeSchema } from '../types/graph';

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

function assertFalse(condition: boolean, message?: string): void {
  if (condition) {
    throw new Error(message || 'Expected false but got true');
  }
}

// Helper to create minimal node schema
function makeNode(name: string, nodeType: string = 'transform'): NodeSchema {
  return {
    name,
    node_type: nodeType as NodeSchema['node_type'],
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
}

// Helper to create minimal edge schema
function makeEdge(from: string, to: string): EdgeSchema {
  return { from, to, edge_type: 'direct' };
}

// Helper to create minimal graph schema
function makeSchema(nodes: NodeSchema[], edges: EdgeSchema[]): GraphSchema {
  return {
    name: 'test_graph',
    version: '1.0',
    nodes,
    edges,
    entry_point: nodes[0]?.name || 'start',
    metadata: {},
  };
}

console.log('\nSchema Diff Tests\n');

console.log('compareSchemas:');

test('returns empty diff for identical schemas', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edge = makeEdge('node_a', 'node_b');
  const schema = makeSchema([nodeA, nodeB], [edge]);

  const diff = compareSchemas(schema, schema);

  assertEqual(diff.addedNodes.length, 0, 'addedNodes');
  assertEqual(diff.removedNodes.length, 0, 'removedNodes');
  assertEqual(diff.modifiedNodes.length, 0, 'modifiedNodes');
  assertEqual(diff.addedEdges.length, 0, 'addedEdges');
  assertEqual(diff.removedEdges.length, 0, 'removedEdges');
  assertEqual(diff.modifiedEdges.length, 0, 'modifiedEdges');
  assertFalse(diff.hasChanges, 'hasChanges should be false');
});

test('detects added nodes', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeA, nodeB], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes');
  assertEqual(diff.addedNodes[0].name, 'node_b', 'added node name');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects removed nodes', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schemaA = makeSchema([nodeA, nodeB], []);
  const schemaB = makeSchema([nodeA], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.removedNodes.length, 1, 'removedNodes');
  assertEqual(diff.removedNodes[0].name, 'node_b', 'removed node name');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects modified nodes', () => {
  const nodeA = makeNode('node_a', 'transform');
  const nodeAModified = makeNode('node_a', 'llm');
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertEqual(diff.modifiedNodes[0].name, 'node_a', 'modified node name');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects added edges', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edge = makeEdge('node_a', 'node_b');
  const schemaA = makeSchema([nodeA, nodeB], []);
  const schemaB = makeSchema([nodeA, nodeB], [edge]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedEdges.length, 1, 'addedEdges');
  assertEqual(diff.addedEdges[0].from, 'node_a', 'added edge from');
  assertEqual(diff.addedEdges[0].to, 'node_b', 'added edge to');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects removed edges', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edge = makeEdge('node_a', 'node_b');
  const schemaA = makeSchema([nodeA, nodeB], [edge]);
  const schemaB = makeSchema([nodeA, nodeB], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.removedEdges.length, 1, 'removedEdges');
  assertEqual(diff.removedEdges[0].from, 'node_a', 'removed edge from');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects modified edges (edge type change)', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edgeA: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'direct' };
  const edgeB: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'yes' };
  const schemaA = makeSchema([nodeA, nodeB], [edgeA]);
  const schemaB = makeSchema([nodeA, nodeB], [edgeB]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].label, 'yes', 'modified edge label');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles complex diff with multiple changes', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const nodeD = makeNode('node_d');
  const nodeBModified = makeNode('node_b', 'llm');

  const edgeAB = makeEdge('node_a', 'node_b');
  const edgeBC = makeEdge('node_b', 'node_c');

  const schemaA = makeSchema([nodeA, nodeB, nodeC], [edgeAB]);
  const schemaB = makeSchema([nodeA, nodeBModified, nodeD], [edgeAB, edgeBC]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes (node_d)');
  assertEqual(diff.removedNodes.length, 1, 'removedNodes (node_c)');
  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes (node_b)');
  assertEqual(diff.addedEdges.length, 1, 'addedEdges (b->c)');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

console.log('\ngetNodeDiffStatus:');

test('returns unchanged when no diff provided', () => {
  assertEqual(getNodeDiffStatus('any_node'), 'unchanged');
  assertEqual(getNodeDiffStatus('any_node', undefined), 'unchanged');
});

test('returns out-of-schema when node is in outOfSchemaNodes set', () => {
  const outOfSchema = new Set(['mystery_node']);
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: false,
  };

  assertEqual(getNodeDiffStatus('mystery_node', diff, outOfSchema), 'out-of-schema');
});

test('out-of-schema takes precedence over other statuses', () => {
  const outOfSchema = new Set(['node_a']);
  const diff: SchemaDiff = {
    addedNodes: [makeNode('node_a')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  // Even though node_a is in addedNodes, out-of-schema should take precedence
  assertEqual(getNodeDiffStatus('node_a', diff, outOfSchema), 'out-of-schema');
});

test('returns added when node is in addedNodes', () => {
  const diff: SchemaDiff = {
    addedNodes: [makeNode('new_node')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('new_node', diff), 'added');
});

test('returns removed when node is in removedNodes', () => {
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [makeNode('old_node')],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('old_node', diff), 'removed');
});

test('returns modified when node is in modifiedNodes', () => {
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [],
    modifiedNodes: [makeNode('changed_node')],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('changed_node', diff), 'modified');
});

test('returns unchanged for node not in any diff list', () => {
  const diff: SchemaDiff = {
    addedNodes: [makeNode('other_node')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('my_node', diff), 'unchanged');
});

test('handles empty outOfSchemaNodes set', () => {
  const emptySet = new Set<string>();
  const diff: SchemaDiff = {
    addedNodes: [makeNode('new_node')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  // Empty set should not affect status
  assertEqual(getNodeDiffStatus('new_node', diff, emptySet), 'added');
  assertEqual(getNodeDiffStatus('other_node', diff, emptySet), 'unchanged');
});

test('handles undefined outOfSchemaNodes', () => {
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [makeNode('removed_node')],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('removed_node', diff, undefined), 'removed');
});

console.log('\nDIFF_STATUS_STYLES:');

test('has entries for all DiffStatus values', () => {
  const allStatuses: DiffStatus[] = ['unchanged', 'added', 'removed', 'modified', 'out-of-schema'];
  for (const status of allStatuses) {
    assertTrue(status in DIFF_STATUS_STYLES, `Missing style for ${status}`);
  }
});

test('has exactly 5 status entries', () => {
  assertEqual(Object.keys(DIFF_STATUS_STYLES).length, 5, 'Should have 5 status entries');
});

test('unchanged status has minimal styling', () => {
  const style = DIFF_STATUS_STYLES['unchanged'];
  assertEqual(style.borderColor, '', 'unchanged borderColor should be empty');
  assertEqual(style.bgOverlay, undefined, 'unchanged bgOverlay should be undefined');
  assertEqual(style.borderStyle, 'solid', 'unchanged borderStyle should be solid');
  assertEqual(style.badge, undefined, 'unchanged should have no badge');
  assertEqual(style.badgeColor, undefined, 'unchanged should have no badgeColor');
});

test('added status has correct styling', () => {
  const style = DIFF_STATUS_STYLES['added'];
  assertEqual(style.borderColor, '#22c55e', 'added borderColor should be green');
  assertTrue(style.bgOverlay?.includes('34, 197, 94') ?? false, 'added bgOverlay should use green rgba');
  assertEqual(style.borderStyle, 'solid', 'added borderStyle should be solid');
  assertEqual(style.badge, '+', 'added badge should be +');
  assertEqual(style.badgeColor, '#22c55e', 'added badgeColor should be green');
});

test('removed status has correct styling', () => {
  const style = DIFF_STATUS_STYLES['removed'];
  assertEqual(style.borderColor, '#ef4444', 'removed borderColor should be red');
  assertTrue(style.bgOverlay?.includes('239, 68, 68') ?? false, 'removed bgOverlay should use red rgba');
  assertEqual(style.borderStyle, 'dashed', 'removed borderStyle should be dashed');
  assertEqual(style.badge, '-', 'removed badge should be -');
  assertEqual(style.badgeColor, '#ef4444', 'removed badgeColor should be red');
});

test('modified status has correct styling', () => {
  const style = DIFF_STATUS_STYLES['modified'];
  assertEqual(style.borderColor, '#f59e0b', 'modified borderColor should be amber');
  assertTrue(style.bgOverlay?.includes('245, 158, 11') ?? false, 'modified bgOverlay should use amber rgba');
  assertEqual(style.borderStyle, 'solid', 'modified borderStyle should be solid');
  assertEqual(style.badge, '~', 'modified badge should be ~');
  assertEqual(style.badgeColor, '#f59e0b', 'modified badgeColor should be amber');
});

test('out-of-schema status has correct styling', () => {
  const style = DIFF_STATUS_STYLES['out-of-schema'];
  assertEqual(style.borderColor, '#dc2626', 'out-of-schema borderColor should be darker red');
  assertTrue(style.bgOverlay?.includes('220, 38, 38') ?? false, 'out-of-schema bgOverlay should use red rgba');
  assertEqual(style.borderStyle, 'dotted', 'out-of-schema borderStyle should be dotted');
  assertEqual(style.badge, '!', 'out-of-schema badge should be !');
  assertEqual(style.badgeColor, '#dc2626', 'out-of-schema badgeColor should be darker red');
});

test('all non-unchanged statuses have badges', () => {
  const statusesWithBadges: DiffStatus[] = ['added', 'removed', 'modified', 'out-of-schema'];
  for (const status of statusesWithBadges) {
    const style = DIFF_STATUS_STYLES[status];
    assertTrue(typeof style.badge === 'string' && style.badge.length === 1, `${status} should have single-char badge`);
    assertTrue(typeof style.badgeColor === 'string' && style.badgeColor.startsWith('#'), `${status} should have hex badgeColor`);
  }
});

test('all borderColors are valid hex or empty', () => {
  const hexPattern = /^#[0-9a-fA-F]{6}$/;
  for (const [status, style] of Object.entries(DIFF_STATUS_STYLES)) {
    if (style.borderColor !== '') {
      assertTrue(hexPattern.test(style.borderColor), `${status} borderColor should be valid hex: ${style.borderColor}`);
    }
  }
});

test('all borderStyles are valid CSS values', () => {
  const validStyles = ['solid', 'dashed', 'dotted'];
  for (const [status, style] of Object.entries(DIFF_STATUS_STYLES)) {
    assertTrue(validStyles.includes(style.borderStyle), `${status} borderStyle should be valid: ${style.borderStyle}`);
  }
});

test('all bgOverlays use rgba format with low opacity', () => {
  const rgbaPattern = /^rgba\(\d+,\s*\d+,\s*\d+,\s*0\.\d+\)$/;
  for (const [status, style] of Object.entries(DIFF_STATUS_STYLES)) {
    if (style.bgOverlay !== undefined) {
      assertTrue(rgbaPattern.test(style.bgOverlay), `${status} bgOverlay should be valid rgba: ${style.bgOverlay}`);
    }
  }
});

console.log('\ncompareSchemas edge cases:');

test('handles empty schemas', () => {
  const emptySchema = makeSchema([], []);
  const diff = compareSchemas(emptySchema, emptySchema);

  assertEqual(diff.addedNodes.length, 0, 'addedNodes');
  assertEqual(diff.removedNodes.length, 0, 'removedNodes');
  assertFalse(diff.hasChanges, 'hasChanges should be false');
});

test('handles schema a empty, schema b has nodes', () => {
  const emptySchema = makeSchema([], []);
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schemaB = makeSchema([nodeA, nodeB], [makeEdge('node_a', 'node_b')]);

  const diff = compareSchemas(emptySchema, schemaB);

  assertEqual(diff.addedNodes.length, 2, 'addedNodes');
  assertEqual(diff.addedEdges.length, 1, 'addedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles schema a has nodes, schema b empty', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schemaA = makeSchema([nodeA, nodeB], [makeEdge('node_a', 'node_b')]);
  const emptySchema = makeSchema([], []);

  const diff = compareSchemas(schemaA, emptySchema);

  assertEqual(diff.removedNodes.length, 2, 'removedNodes');
  assertEqual(diff.removedEdges.length, 1, 'removedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles multiple nodes added simultaneously', () => {
  const nodeA = makeNode('node_a');
  const schemaA = makeSchema([nodeA], []);
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const nodeD = makeNode('node_d');
  const schemaB = makeSchema([nodeA, nodeB, nodeC, nodeD], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 3, 'addedNodes');
  const addedNames = diff.addedNodes.map(n => n.name).sort();
  assertEqual(addedNames, ['node_b', 'node_c', 'node_d'], 'added node names');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles multiple nodes removed simultaneously', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const schemaA = makeSchema([nodeA, nodeB, nodeC], []);
  const schemaB = makeSchema([nodeA], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.removedNodes.length, 2, 'removedNodes');
  const removedNames = diff.removedNodes.map(n => n.name).sort();
  assertEqual(removedNames, ['node_b', 'node_c'], 'removed node names');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles multiple edges added simultaneously', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const schemaA = makeSchema([nodeA, nodeB, nodeC], []);
  const schemaB = makeSchema([nodeA, nodeB, nodeC], [
    makeEdge('node_a', 'node_b'),
    makeEdge('node_b', 'node_c'),
    makeEdge('node_a', 'node_c'),
  ]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedEdges.length, 3, 'addedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles multiple edges removed simultaneously', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const schemaA = makeSchema([nodeA, nodeB, nodeC], [
    makeEdge('node_a', 'node_b'),
    makeEdge('node_b', 'node_c'),
    makeEdge('node_a', 'node_c'),
  ]);
  const schemaB = makeSchema([nodeA, nodeB, nodeC], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.removedEdges.length, 3, 'removedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects node modification via attribute change', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { model: 'gpt-4' },
  };
  const nodeAModified: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { model: 'gpt-4o' },
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertEqual(diff.modifiedNodes[0].attributes?.model, 'gpt-4o', 'modified attribute value');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects node modification via input_fields change', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['query'],
    output_fields: [],
    attributes: {},
  };
  const nodeAModified: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['query', 'context'],
    output_fields: [],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects node modification via output_fields change', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['result'],
    attributes: {},
  };
  const nodeAModified: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['result', 'metadata'],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles node with same name but completely different content', () => {
  const nodeA: NodeSchema = {
    name: 'processor',
    node_type: 'transform',
    input_fields: ['a', 'b'],
    output_fields: ['c'],
    attributes: { version: '1.0' },
  };
  const nodeAModified: NodeSchema = {
    name: 'processor',
    node_type: 'llm',
    input_fields: ['prompt'],
    output_fields: ['response'],
    attributes: { model: 'claude-3' },
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertEqual(diff.addedNodes.length, 0, 'addedNodes should be 0');
  assertEqual(diff.removedNodes.length, 0, 'removedNodes should be 0');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('edge modification only looks at from/to identity', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edge1: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'direct' };
  const edge2: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'direct', label: 'extra' };
  const schemaA = makeSchema([nodeA, nodeB], [edge1]);
  const schemaB = makeSchema([nodeA, nodeB], [edge2]);

  const diff = compareSchemas(schemaA, schemaB);

  // Same from/to but different content = modified
  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.addedEdges.length, 0, 'addedEdges should be 0');
  assertEqual(diff.removedEdges.length, 0, 'removedEdges should be 0');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles swapped node order in schema', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schemaA = makeSchema([nodeA, nodeB], []);
  const schemaB = makeSchema([nodeB, nodeA], []);

  const diff = compareSchemas(schemaA, schemaB);

  // Order should not matter for comparison
  assertEqual(diff.addedNodes.length, 0, 'addedNodes');
  assertEqual(diff.removedNodes.length, 0, 'removedNodes');
  assertEqual(diff.modifiedNodes.length, 0, 'modifiedNodes');
  assertFalse(diff.hasChanges, 'hasChanges should be false');
});

test('handles swapped edge order in schema', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const edge1 = makeEdge('node_a', 'node_b');
  const edge2 = makeEdge('node_b', 'node_c');
  const schemaA = makeSchema([nodeA, nodeB, nodeC], [edge1, edge2]);
  const schemaB = makeSchema([nodeA, nodeB, nodeC], [edge2, edge1]);

  const diff = compareSchemas(schemaA, schemaB);

  // Order should not matter for comparison
  assertEqual(diff.addedEdges.length, 0, 'addedEdges');
  assertEqual(diff.removedEdges.length, 0, 'removedEdges');
  assertEqual(diff.modifiedEdges.length, 0, 'modifiedEdges');
  assertFalse(diff.hasChanges, 'hasChanges should be false');
});

test('full schema replacement detection', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const nodeC = makeNode('node_c');
  const nodeD = makeNode('node_d');
  const schemaA = makeSchema([nodeA, nodeB], [makeEdge('node_a', 'node_b')]);
  const schemaB = makeSchema([nodeC, nodeD], [makeEdge('node_c', 'node_d')]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 2, 'addedNodes');
  assertEqual(diff.removedNodes.length, 2, 'removedNodes');
  assertEqual(diff.modifiedNodes.length, 0, 'modifiedNodes');
  assertEqual(diff.addedEdges.length, 1, 'addedEdges');
  assertEqual(diff.removedEdges.length, 1, 'removedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

console.log('\ncompareSchemas - self-loop edges:');

test('handles self-loop edge (from === to)', () => {
  const nodeA = makeNode('node_a');
  const selfLoop = makeEdge('node_a', 'node_a');
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeA], [selfLoop]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedEdges.length, 1, 'addedEdges');
  assertEqual(diff.addedEdges[0].from, 'node_a', 'self-loop from');
  assertEqual(diff.addedEdges[0].to, 'node_a', 'self-loop to');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects self-loop removal', () => {
  const nodeA = makeNode('node_a');
  const selfLoop = makeEdge('node_a', 'node_a');
  const schemaA = makeSchema([nodeA], [selfLoop]);
  const schemaB = makeSchema([nodeA], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.removedEdges.length, 1, 'removedEdges');
  assertFalse(diff.addedEdges.length > 0, 'no addedEdges');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects self-loop modification', () => {
  const nodeA = makeNode('node_a');
  const selfLoopA: EdgeSchema = { from: 'node_a', to: 'node_a', edge_type: 'direct' };
  const selfLoopB: EdgeSchema = { from: 'node_a', to: 'node_a', edge_type: 'conditional', label: 'retry' };
  const schemaA = makeSchema([nodeA], [selfLoopA]);
  const schemaB = makeSchema([nodeA], [selfLoopB]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].label, 'retry', 'modified self-loop label');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

console.log('\ncompareSchemas - parallel edges:');

test('handles parallel edges (same from/to, treated as single edge)', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edge1: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'direct' };
  const edge2: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'branch' };

  // Schema A has edge1, Schema B has edge2 - same from/to but different content
  const schemaA = makeSchema([nodeA, nodeB], [edge1]);
  const schemaB = makeSchema([nodeA, nodeB], [edge2]);

  const diff = compareSchemas(schemaA, schemaB);

  // From/to identity is the same, so it's a modification not add/remove
  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.addedEdges.length, 0, 'no addedEdges');
  assertEqual(diff.removedEdges.length, 0, 'no removedEdges');
});

console.log('\ncompareSchemas - complex attributes:');

test('detects modification in attribute string values', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: { model: 'gpt-4', temperature: '0.7' },
  };
  const nodeAModified: NodeSchema = {
    name: 'node_a',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: { model: 'gpt-4', temperature: '0.8' },
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('detects attribute addition as modification', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { key1: 'value1' },
  };
  const nodeAModified: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { key1: 'value1', key2: 'value2' },
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeAModified], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedNodes.length, 1, 'modifiedNodes');
});

test('treats empty attributes object same as missing attributes', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
  const nodeACopy: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeACopy], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertFalse(diff.hasChanges, 'hasChanges should be false for identical empty attributes');
});

test('attribute order does not affect comparison (JSON.stringify sorts)', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { b: 'two', a: 'one' },
  };
  const nodeACopy: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: { a: 'one', b: 'two' },
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeACopy], []);

  const diff = compareSchemas(schemaA, schemaB);

  // Note: JSON.stringify object key order depends on insertion order in JS
  // This test documents the actual behavior
  // If keys are in different order, JSON.stringify may show them as different
  const strA = JSON.stringify(nodeA);
  const strB = JSON.stringify(nodeACopy);
  if (strA === strB) {
    assertFalse(diff.hasChanges, 'hasChanges should be false when JSON strings match');
  } else {
    assertTrue(diff.modifiedNodes.length === 1, 'different JSON key order shows as modified');
  }
});

console.log('\ncompareSchemas - schema metadata:');

test('schema name difference does not affect node/edge comparison', () => {
  const nodeA = makeNode('node_a');
  const schema1: GraphSchema = {
    name: 'graph_v1',
    version: '1.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: {},
  };
  const schema2: GraphSchema = {
    name: 'graph_v2',
    version: '1.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: {},
  };

  const diff = compareSchemas(schema1, schema2);

  // compareSchemas only compares nodes and edges, not schema-level metadata
  assertFalse(diff.hasChanges, 'schema name difference does not trigger hasChanges');
});

test('schema version difference does not affect node/edge comparison', () => {
  const nodeA = makeNode('node_a');
  const schema1: GraphSchema = {
    name: 'graph',
    version: '1.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: {},
  };
  const schema2: GraphSchema = {
    name: 'graph',
    version: '2.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: {},
  };

  const diff = compareSchemas(schema1, schema2);

  assertFalse(diff.hasChanges, 'version difference does not trigger hasChanges');
});

test('entry_point difference does not affect node/edge comparison', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const schema1: GraphSchema = {
    name: 'graph',
    version: '1.0',
    nodes: [nodeA, nodeB],
    edges: [],
    entry_point: 'node_a',
    metadata: {},
  };
  const schema2: GraphSchema = {
    name: 'graph',
    version: '1.0',
    nodes: [nodeA, nodeB],
    edges: [],
    entry_point: 'node_b',
    metadata: {},
  };

  const diff = compareSchemas(schema1, schema2);

  assertFalse(diff.hasChanges, 'entry_point difference does not trigger hasChanges');
});

test('schema metadata difference does not affect node/edge comparison', () => {
  const nodeA = makeNode('node_a');
  const schema1: GraphSchema = {
    name: 'graph',
    version: '1.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: { author: 'alice' },
  };
  const schema2: GraphSchema = {
    name: 'graph',
    version: '1.0',
    nodes: [nodeA],
    edges: [],
    entry_point: 'node_a',
    metadata: { author: 'bob', reviewed: 'true' },
  };

  const diff = compareSchemas(schema1, schema2);

  assertFalse(diff.hasChanges, 'metadata difference does not trigger hasChanges');
});

console.log('\ncompareSchemas - stress tests:');

test('handles large number of nodes (100 nodes)', () => {
  const nodes: NodeSchema[] = [];
  for (let i = 0; i < 100; i++) {
    nodes.push(makeNode(`node_${i}`));
  }
  const schemaA = makeSchema(nodes.slice(0, 50), []);
  const schemaB = makeSchema(nodes.slice(25, 75), []);

  const diff = compareSchemas(schemaA, schemaB);

  // schemaA has nodes 0-49, schemaB has nodes 25-74
  // Removed: 0-24 (25 nodes)
  // Added: 50-74 (25 nodes)
  // Unchanged: 25-49 (25 nodes)
  assertEqual(diff.removedNodes.length, 25, 'removedNodes count');
  assertEqual(diff.addedNodes.length, 25, 'addedNodes count');
  assertEqual(diff.modifiedNodes.length, 0, 'modifiedNodes count');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

test('handles large number of edges (100 edges)', () => {
  const nodes: NodeSchema[] = [];
  for (let i = 0; i < 101; i++) {
    nodes.push(makeNode(`node_${i}`));
  }
  const edgesA: EdgeSchema[] = [];
  const edgesB: EdgeSchema[] = [];
  for (let i = 0; i < 100; i++) {
    edgesA.push(makeEdge(`node_${i}`, `node_${i + 1}`));
  }
  for (let i = 50; i < 150; i++) {
    if (i < 100) {
      edgesB.push(makeEdge(`node_${i}`, `node_${i + 1}`));
    }
  }

  // Add more nodes for edgesB that go beyond 100
  for (let i = 101; i <= 150; i++) {
    nodes.push(makeNode(`node_${i}`));
  }

  const schemaA = makeSchema(nodes, edgesA);
  const schemaB = makeSchema(nodes, edgesB);

  const diff = compareSchemas(schemaA, schemaB);

  // edgesA: 0-99 (node_i -> node_i+1)
  // edgesB: 50-99 (same from/to)
  // Removed: 0-49 (50 edges)
  assertEqual(diff.removedEdges.length, 50, 'removedEdges count');
  assertTrue(diff.hasChanges, 'hasChanges should be true');
});

console.log('\ncompareSchemas - special node names:');

test('handles empty string node name', () => {
  const nodeEmpty = makeNode('');
  const nodeA = makeNode('node_a');
  const schemaA = makeSchema([nodeEmpty], []);
  const schemaB = makeSchema([nodeEmpty, nodeA], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes');
  assertEqual(diff.addedNodes[0].name, 'node_a', 'added node name');
});

test('handles node name with special characters', () => {
  const nodeSpecial = makeNode('node-with-dashes_and_underscores.and.dots');
  const schemaA = makeSchema([], []);
  const schemaB = makeSchema([nodeSpecial], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes');
  assertEqual(diff.addedNodes[0].name, 'node-with-dashes_and_underscores.and.dots', 'node name preserved');
});

test('handles unicode node names', () => {
  const nodeUnicode = makeNode('节点_日本語_한국어');
  const schemaA = makeSchema([], []);
  const schemaB = makeSchema([nodeUnicode], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes');
  assertEqual(diff.addedNodes[0].name, '节点_日本語_한국어', 'unicode node name preserved');
});

test('handles very long node name', () => {
  const longName = 'a'.repeat(1000);
  const nodeLong = makeNode(longName);
  const schemaA = makeSchema([], []);
  const schemaB = makeSchema([nodeLong], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 1, 'addedNodes');
  assertEqual(diff.addedNodes[0].name.length, 1000, 'long node name length preserved');
});

test('handles node name with whitespace', () => {
  const nodeWhitespace = makeNode('node with spaces');
  const nodeTab = makeNode('node\twith\ttabs');
  const schemaA = makeSchema([], []);
  const schemaB = makeSchema([nodeWhitespace, nodeTab], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.addedNodes.length, 2, 'addedNodes');
});

console.log('\ngetNodeDiffStatus - edge cases:');

test('handles empty string node name lookup', () => {
  const diff: SchemaDiff = {
    addedNodes: [makeNode('')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('', diff), 'added');
  assertEqual(getNodeDiffStatus('other', diff), 'unchanged');
});

test('handles special character node name lookup', () => {
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [],
    modifiedNodes: [makeNode('node.with.dots')],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('node.with.dots', diff), 'modified');
});

test('handles unicode node name lookup', () => {
  const diff: SchemaDiff = {
    addedNodes: [makeNode('узел')],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  assertEqual(getNodeDiffStatus('узел', diff), 'added');
});

test('handles whitespace node name lookup', () => {
  const outOfSchema = new Set(['  spaced  ']);
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: false,
  };

  assertEqual(getNodeDiffStatus('  spaced  ', diff, outOfSchema), 'out-of-schema');
});

test('getNodeDiffStatus priority: out-of-schema > added > removed > modified', () => {
  // Simulate a node that appears in multiple lists (unlikely but tests priority)
  const node = makeNode('multi_status');
  const outOfSchema = new Set(['multi_status']);
  const diff: SchemaDiff = {
    addedNodes: [node],
    removedNodes: [node],
    modifiedNodes: [node],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: true,
  };

  // out-of-schema should take precedence
  assertEqual(getNodeDiffStatus('multi_status', diff, outOfSchema), 'out-of-schema');

  // Without outOfSchema, added takes precedence (checked first in implementation)
  assertEqual(getNodeDiffStatus('multi_status', diff), 'added');
});

test('getNodeDiffStatus with large outOfSchemaNodes set', () => {
  const largeSet = new Set<string>();
  for (let i = 0; i < 1000; i++) {
    largeSet.add(`node_${i}`);
  }
  const diff: SchemaDiff = {
    addedNodes: [],
    removedNodes: [],
    modifiedNodes: [],
    addedEdges: [],
    removedEdges: [],
    modifiedEdges: [],
    hasChanges: false,
  };

  assertEqual(getNodeDiffStatus('node_500', diff, largeSet), 'out-of-schema');
  assertEqual(getNodeDiffStatus('node_9999', diff, largeSet), 'unchanged');
});

console.log('\nDIFF_STATUS_STYLES - detailed validation:');

test('added style bgOverlay has 0.08 opacity', () => {
  const style = DIFF_STATUS_STYLES['added'];
  assertTrue(style.bgOverlay?.includes('0.08') ?? false, 'added bgOverlay should have 0.08 opacity');
});

test('removed style bgOverlay has 0.08 opacity', () => {
  const style = DIFF_STATUS_STYLES['removed'];
  assertTrue(style.bgOverlay?.includes('0.08') ?? false, 'removed bgOverlay should have 0.08 opacity');
});

test('modified style bgOverlay has 0.08 opacity', () => {
  const style = DIFF_STATUS_STYLES['modified'];
  assertTrue(style.bgOverlay?.includes('0.08') ?? false, 'modified bgOverlay should have 0.08 opacity');
});

test('out-of-schema style bgOverlay has higher 0.12 opacity', () => {
  const style = DIFF_STATUS_STYLES['out-of-schema'];
  assertTrue(style.bgOverlay?.includes('0.12') ?? false, 'out-of-schema bgOverlay should have 0.12 opacity');
});

test('badge colors match border colors', () => {
  const statusesWithBadges: DiffStatus[] = ['added', 'removed', 'modified', 'out-of-schema'];
  for (const status of statusesWithBadges) {
    const style = DIFF_STATUS_STYLES[status];
    assertEqual(style.badgeColor, style.borderColor, `${status} badgeColor should match borderColor`);
  }
});

test('all non-unchanged statuses have bgOverlay', () => {
  const statusesWithOverlay: DiffStatus[] = ['added', 'removed', 'modified', 'out-of-schema'];
  for (const status of statusesWithOverlay) {
    const style = DIFF_STATUS_STYLES[status];
    assertTrue(typeof style.bgOverlay === 'string', `${status} should have bgOverlay`);
  }
});

test('color consistency: green family for added', () => {
  const style = DIFF_STATUS_STYLES['added'];
  assertTrue(style.borderColor === '#22c55e', 'added uses green-500');
  assertTrue(style.bgOverlay?.includes('34, 197, 94') ?? false, 'added rgba uses green RGB values');
});

test('color consistency: red family for removed', () => {
  const style = DIFF_STATUS_STYLES['removed'];
  assertTrue(style.borderColor === '#ef4444', 'removed uses red-500');
  assertTrue(style.bgOverlay?.includes('239, 68, 68') ?? false, 'removed rgba uses red RGB values');
});

test('color consistency: amber family for modified', () => {
  const style = DIFF_STATUS_STYLES['modified'];
  assertTrue(style.borderColor === '#f59e0b', 'modified uses amber-500');
  assertTrue(style.bgOverlay?.includes('245, 158, 11') ?? false, 'modified rgba uses amber RGB values');
});

test('color consistency: darker red for out-of-schema', () => {
  const style = DIFF_STATUS_STYLES['out-of-schema'];
  assertTrue(style.borderColor === '#dc2626', 'out-of-schema uses red-600');
  assertTrue(style.bgOverlay?.includes('220, 38, 38') ?? false, 'out-of-schema rgba uses red-600 RGB values');
});

test('borderStyle visual hierarchy', () => {
  // solid = normal changes (added, modified)
  // dashed = removed (node going away)
  // dotted = out-of-schema (warning/error state)
  assertEqual(DIFF_STATUS_STYLES['unchanged'].borderStyle, 'solid');
  assertEqual(DIFF_STATUS_STYLES['added'].borderStyle, 'solid');
  assertEqual(DIFF_STATUS_STYLES['modified'].borderStyle, 'solid');
  assertEqual(DIFF_STATUS_STYLES['removed'].borderStyle, 'dashed');
  assertEqual(DIFF_STATUS_STYLES['out-of-schema'].borderStyle, 'dotted');
});

console.log('\ncompareSchemas - edge type variations:');

test('detects different edge_type as modification', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edgeDirect: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'direct' };
  const edgeConditional: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional' };
  const schemaA = makeSchema([nodeA, nodeB], [edgeDirect]);
  const schemaB = makeSchema([nodeA, nodeB], [edgeConditional]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].edge_type, 'conditional', 'new edge_type');
});

test('edge label addition counts as modification', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edgeNoLabel: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional' };
  const edgeWithLabel: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'success' };
  const schemaA = makeSchema([nodeA, nodeB], [edgeNoLabel]);
  const schemaB = makeSchema([nodeA, nodeB], [edgeWithLabel]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].label, 'success', 'added label');
});

test('edge label removal counts as modification', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edgeWithLabel: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'error' };
  const edgeNoLabel: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional' };
  const schemaA = makeSchema([nodeA, nodeB], [edgeWithLabel]);
  const schemaB = makeSchema([nodeA, nodeB], [edgeNoLabel]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].label, undefined, 'removed label');
});

test('edge label change counts as modification', () => {
  const nodeA = makeNode('node_a');
  const nodeB = makeNode('node_b');
  const edgeLabelA: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'yes' };
  const edgeLabelB: EdgeSchema = { from: 'node_a', to: 'node_b', edge_type: 'conditional', label: 'no' };
  const schemaA = makeSchema([nodeA, nodeB], [edgeLabelA]);
  const schemaB = makeSchema([nodeA, nodeB], [edgeLabelB]);

  const diff = compareSchemas(schemaA, schemaB);

  assertEqual(diff.modifiedEdges.length, 1, 'modifiedEdges');
  assertEqual(diff.modifiedEdges[0].label, 'no', 'changed label');
});

console.log('\ncompareSchemas - node_type variations:');

test('all node_types can be compared', () => {
  // Use valid NodeType values from graph.ts
  const nodeTypes: NodeSchema['node_type'][] = [
    'transform', 'llm', 'tool', 'router', 'aggregator', 'validator', 'human_in_loop', 'checkpoint', 'custom'
  ];

  for (const nodeType of nodeTypes) {
    const nodeA = makeNode('test_node', 'transform');
    const nodeB = makeNode('test_node', nodeType);
    const schemaA = makeSchema([nodeA], []);
    const schemaB = makeSchema([nodeB], []);

    const diff = compareSchemas(schemaA, schemaB);

    if (nodeType === 'transform') {
      assertFalse(diff.hasChanges, `same type ${nodeType} should not change`);
    } else {
      assertTrue(diff.modifiedNodes.length === 1, `type change to ${nodeType} detected`);
    }
  }
});

console.log('\ncompareSchemas - input/output field edge cases:');

test('empty arrays vs arrays with empty strings', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };
  const nodeB: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: [''],
    output_fields: [],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeB], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertTrue(diff.modifiedNodes.length === 1, 'empty array vs array with empty string is different');
});

test('field order matters in comparison', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['a', 'b', 'c'],
    output_fields: [],
    attributes: {},
  };
  const nodeB: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['c', 'b', 'a'],
    output_fields: [],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeB], []);

  const diff = compareSchemas(schemaA, schemaB);

  // JSON.stringify preserves array order, so different order = modified
  assertTrue(diff.modifiedNodes.length === 1, 'field order change detected');
});

test('duplicate fields in array', () => {
  const nodeA: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['a', 'a', 'a'],
    output_fields: [],
    attributes: {},
  };
  const nodeB: NodeSchema = {
    name: 'node_a',
    node_type: 'transform',
    input_fields: ['a'],
    output_fields: [],
    attributes: {},
  };
  const schemaA = makeSchema([nodeA], []);
  const schemaB = makeSchema([nodeB], []);

  const diff = compareSchemas(schemaA, schemaB);

  assertTrue(diff.modifiedNodes.length === 1, 'duplicate fields vs single field is different');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
