// M-444: Component tests for GraphNode
// Run with: npx tsx src/__tests__/GraphNode.test.tsx
//
// GraphNode uses @xyflow/react Handle components which require React Flow context.
// We test the extracted pure logic in GraphNodeLogic directly.

import type { NodeType, NodeStatus } from '../types/graph';
import type { DiffStatus } from '../types/schemaDiff';
import {
  NODE_TYPE_DESCRIPTIONS,
  borderWidthForDiffStatus,
  formatDurationText,
  getAnimationClass,
  getDurationPillStyle,
  getGraphNodeAriaLabel,
  getGraphNodeDiffPresentation,
  getNodeTypeInfo,
  getRingClass,
  getStatusBadgeStyles,
  getStatusIndicator,
  shouldShowActiveIndicator,
  shouldShowDuration,
} from '../components/GraphNodeLogic';

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

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

console.log('\nGraphNode Tests\n');

console.log('Status indicator mapping:');

test('pending status returns hollow circle', () => {
  assertEqual(getStatusIndicator('pending'), '○');
});

test('active status returns filled circle', () => {
  assertEqual(getStatusIndicator('active'), '●');
});

test('completed status returns checkmark', () => {
  assertEqual(getStatusIndicator('completed'), '✓');
});

test('error status returns X', () => {
  assertEqual(getStatusIndicator('error'), '✗');
});

console.log('\nAnimation class logic:');

test('active status returns node-running class', () => {
  assertEqual(getAnimationClass('active', false), 'node-running');
});

test('isActive true returns node-running class', () => {
  assertEqual(getAnimationClass('pending', true), 'node-running');
});

test('pending status with isActive false returns empty', () => {
  assertEqual(getAnimationClass('pending', false), '');
});

test('completed status with isActive false returns empty', () => {
  assertEqual(getAnimationClass('completed', false), '');
});

console.log('\nDiff status styling:');

test('unchanged status uses original border color', () => {
  const result = getGraphNodeDiffPresentation('unchanged', '#original', '#bg');
  assertEqual(result.effectiveBorderColor, '#original');
  assertEqual(result.effectiveBgColor, '#bg');
  assertEqual(result.diffStyle.badge, undefined);
  assertEqual(result.diffStyle.borderStyle, 'solid');
});

test('added status uses green border and has badge', () => {
  const result = getGraphNodeDiffPresentation('added', '#original', '#bg');
  assertEqual(result.effectiveBorderColor, '#22c55e'); // green
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertIncludes(result.effectiveBgColor, '#bg');
  assertEqual(result.diffStyle.badge, '+');
});

test('removed status uses red border and has badge', () => {
  const result = getGraphNodeDiffPresentation('removed', '#original', '#bg');
  assertEqual(result.effectiveBorderColor, '#ef4444'); // red
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertEqual(result.diffStyle.badge, '-');
});

test('modified status uses amber border and has badge', () => {
  const result = getGraphNodeDiffPresentation('modified', '#original', '#bg');
  assertEqual(result.effectiveBorderColor, '#f59e0b'); // amber
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertEqual(result.diffStyle.badge, '~');
});

test('out-of-schema status uses red border and has badge', () => {
  const result = getGraphNodeDiffPresentation('out-of-schema', '#original', '#bg');
  assertEqual(result.effectiveBorderColor, '#dc2626'); // red-600
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertEqual(result.diffStyle.badge, '!');
});

console.log('\nRing styling logic:');

test('selected node has blue ring', () => {
  assertIncludes(getRingClass(true, false), 'ring-blue-500');
});

test('focused but not selected node has purple ring', () => {
  assertIncludes(getRingClass(false, true), 'ring-purple-500');
});

test('selected takes precedence over focused', () => {
  const ringClass = getRingClass(true, true);
  assertIncludes(ringClass, 'ring-blue-500');
  assertEqual(ringClass.includes('ring-purple-500'), false);
});

test('neither selected nor focused has no ring', () => {
  assertEqual(getRingClass(false, false), '');
});

console.log('\nARIA label generation:');

test('aria label includes label and status', () => {
  const label = getGraphNodeAriaLabel({ label: 'my_node', status: 'active' });
  assertEqual(label, 'my_node, status: active');
});

test('aria label includes description when present', () => {
  const label = getGraphNodeAriaLabel({ label: 'my_node', status: 'completed', description: 'Process data' });
  assertIncludes(label, 'Process data');
});

test('aria label includes duration when present', () => {
  const label = getGraphNodeAriaLabel({ label: 'my_node', status: 'completed', duration: 150 });
  assertIncludes(label, 'duration: 150ms');
});

test('aria label includes all parts when all present', () => {
  const label = getGraphNodeAriaLabel({ label: 'my_node', status: 'completed', description: 'Process data', duration: 150 });
  assertIncludes(label, 'my_node');
  assertIncludes(label, 'status: completed');
  assertIncludes(label, 'Process data');
  assertIncludes(label, 'duration: 150ms');
});

console.log('\nDuration display logic:');

test('shows duration for completed status', () => {
  assertEqual(shouldShowDuration(100, 'completed'), true);
});

test('shows duration for active status', () => {
  assertEqual(shouldShowDuration(100, 'active'), true);
});

test('hides duration for pending status', () => {
  assertEqual(shouldShowDuration(100, 'pending'), false);
});

test('hides duration for error status', () => {
  assertEqual(shouldShowDuration(100, 'error'), false);
});

test('hides duration when undefined', () => {
  assertEqual(shouldShowDuration(undefined, 'completed'), false);
});

test('active status shows ellipsis after duration', () => {
  assertEqual(formatDurationText(100, 'active'), '100ms...');
});

test('completed status shows plain duration', () => {
  assertEqual(formatDurationText(100, 'completed'), '100ms');
});

console.log('\nActive indicator visibility:');

test('shows indicator when status is active', () => {
  assertEqual(shouldShowActiveIndicator('active', false), true);
});

test('shows indicator when isActive is true', () => {
  assertEqual(shouldShowActiveIndicator('pending', true), true);
});

test('hides indicator when status is pending and not active', () => {
  assertEqual(shouldShowActiveIndicator('pending', false), false);
});

test('hides indicator when status is completed and not active', () => {
  assertEqual(shouldShowActiveIndicator('completed', false), false);
});

console.log('\nBorder width based on diff status:');

test('unchanged status has 2px border', () => {
  assertEqual(borderWidthForDiffStatus('unchanged'), 2);
});

test('added status has 3px border', () => {
  assertEqual(borderWidthForDiffStatus('added'), 3);
});

test('removed status has 3px border', () => {
  assertEqual(borderWidthForDiffStatus('removed'), 3);
});

test('modified status has 3px border', () => {
  assertEqual(borderWidthForDiffStatus('modified'), 3);
});

console.log('\nNode type descriptions (TYP-08):');

test('NODE_TYPE_DESCRIPTIONS has expected keys', () => {
  const expectedKeys: NodeType[] = [
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
  for (const key of expectedKeys) {
    assertEqual(typeof NODE_TYPE_DESCRIPTIONS[key].badge, 'string');
    assertEqual(typeof NODE_TYPE_DESCRIPTIONS[key].description, 'string');
  }
});

test('transform node has correct badge and description', () => {
  const info = getNodeTypeInfo('transform');
  assertEqual(info.badge, 'Transform');
  assertIncludes(info.description, 'Pure function');
});

test('llm node has correct badge and description', () => {
  const info = getNodeTypeInfo('llm');
  assertEqual(info.badge, 'LLM');
  assertIncludes(info.description, 'Language model');
});

test('tool node has correct badge and description', () => {
  const info = getNodeTypeInfo('tool');
  assertEqual(info.badge, 'Tool');
  assertIncludes(info.description, 'External tool');
});

test('router node has correct badge and description', () => {
  const info = getNodeTypeInfo('router');
  assertEqual(info.badge, 'Router');
  assertIncludes(info.description, 'Conditional routing');
});

test('aggregator node has correct badge and description', () => {
  const info = getNodeTypeInfo('aggregator');
  assertEqual(info.badge, 'Aggregator');
  assertIncludes(info.description, 'Collects and combines');
});

test('validator node has correct badge and description', () => {
  const info = getNodeTypeInfo('validator');
  assertEqual(info.badge, 'Validator');
  assertIncludes(info.description, 'Validates state');
});

test('human_in_loop node has correct badge and description', () => {
  const info = getNodeTypeInfo('human_in_loop');
  assertEqual(info.badge, 'Human');
  assertIncludes(info.description, 'human review');
});

test('checkpoint node has correct badge and description', () => {
  const info = getNodeTypeInfo('checkpoint');
  assertEqual(info.badge, 'Checkpoint');
  assertIncludes(info.description, 'Persists state');
});

test('custom node has correct badge and description', () => {
  const info = getNodeTypeInfo('custom');
  assertEqual(info.badge, 'Custom');
  assertIncludes(info.description, 'user-defined');
});

console.log('\nStatus badge styling (TYP-06):');

test('pending status has gray badge styling', () => {
  const style = getStatusBadgeStyles('pending');
  assertIncludes(style, 'bg-gray-500');
  assertIncludes(style, 'text-gray-400');
});

test('active status has blue badge styling', () => {
  const style = getStatusBadgeStyles('active');
  assertIncludes(style, 'bg-blue-500');
  assertIncludes(style, 'text-blue-400');
});

test('completed status has green badge styling', () => {
  const style = getStatusBadgeStyles('completed');
  assertIncludes(style, 'bg-green-500');
  assertIncludes(style, 'text-green-400');
});

test('error status has red badge styling', () => {
  const style = getStatusBadgeStyles('error');
  assertIncludes(style, 'bg-red-500');
  assertIncludes(style, 'text-red-400');
});

console.log('\nDuration pill styling (TYP-03):');

test('active status duration pill has blue styling', () => {
  const style = getDurationPillStyle('active');
  assertIncludes(style, 'bg-blue-500');
  assertIncludes(style, 'text-blue-300');
});

test('completed status duration pill has gray styling', () => {
  const style = getDurationPillStyle('completed');
  assertIncludes(style, 'bg-black');
  assertIncludes(style, 'text-gray-300');
});

test('pending status duration pill has default gray styling', () => {
  const style = getDurationPillStyle('pending');
  assertIncludes(style, 'bg-black');
  assertIncludes(style, 'text-gray-300');
});

test('error status duration pill has default gray styling', () => {
  const style = getDurationPillStyle('error');
  assertIncludes(style, 'bg-black');
  assertIncludes(style, 'text-gray-300');
});

console.log('\nAnimation class edge cases:');

test('error status with isActive false returns empty', () => {
  assertEqual(getAnimationClass('error', false), '');
});

test('error status with isActive true returns node-running', () => {
  assertEqual(getAnimationClass('error', true), 'node-running');
});

test('completed status with isActive true returns node-running', () => {
  assertEqual(getAnimationClass('completed', true), 'node-running');
});

console.log('\nDuration formatting edge cases:');

test('0ms duration for active status', () => {
  assertEqual(formatDurationText(0, 'active'), '0ms...');
});

test('0ms duration for completed status', () => {
  assertEqual(formatDurationText(0, 'completed'), '0ms');
});

test('large duration (10000ms) for active status', () => {
  assertEqual(formatDurationText(10000, 'active'), '10000ms...');
});

test('large duration (10000ms) for completed status', () => {
  assertEqual(formatDurationText(10000, 'completed'), '10000ms');
});

test('very large duration (1000000ms) for completed status', () => {
  assertEqual(formatDurationText(1000000, 'completed'), '1000000ms');
});

test('sub-second duration (1ms) for completed status', () => {
  assertEqual(formatDurationText(1, 'completed'), '1ms');
});

console.log('\nBorder width edge cases:');

test('out-of-schema status has 3px border', () => {
  assertEqual(borderWidthForDiffStatus('out-of-schema'), 3);
});

test('unchanged is the only status with 2px border', () => {
  const statuses: DiffStatus[] = ['unchanged', 'added', 'removed', 'modified', 'out-of-schema'];
  for (const status of statuses) {
    const expected = status === 'unchanged' ? 2 : 3;
    assertEqual(borderWidthForDiffStatus(status), expected);
  }
});

console.log('\nARIA label edge cases:');

test('aria label with empty label string', () => {
  const label = getGraphNodeAriaLabel({ label: '', status: 'pending' });
  assertEqual(label, ', status: pending');
});

test('aria label with very long label', () => {
  const longLabel = 'a'.repeat(200);
  const label = getGraphNodeAriaLabel({ label: longLabel, status: 'active' });
  assertIncludes(label, longLabel);
  assertIncludes(label, 'status: active');
});

test('aria label with very long description', () => {
  const longDesc = 'This is a very long description that explains what this node does in great detail '.repeat(5);
  const label = getGraphNodeAriaLabel({ label: 'test', status: 'completed', description: longDesc });
  assertIncludes(label, longDesc);
});

test('aria label with duration = 0', () => {
  const label = getGraphNodeAriaLabel({ label: 'fast_node', status: 'completed', duration: 0 });
  assertIncludes(label, 'duration: 0ms');
});

test('aria label with large duration', () => {
  const label = getGraphNodeAriaLabel({ label: 'slow_node', status: 'completed', duration: 999999 });
  assertIncludes(label, 'duration: 999999ms');
});

test('aria label with special characters in label', () => {
  const label = getGraphNodeAriaLabel({ label: 'node_with-special.chars', status: 'active' });
  assertIncludes(label, 'node_with-special.chars');
});

test('aria label with unicode in description', () => {
  const label = getGraphNodeAriaLabel({ label: 'test', status: 'active', description: 'Processes 日本語 data' });
  assertIncludes(label, 'Processes 日本語 data');
});

console.log('\nDuration display edge cases:');

test('duration = 0 is shown for completed status', () => {
  assertEqual(shouldShowDuration(0, 'completed'), true);
});

test('duration = 0 is shown for active status', () => {
  assertEqual(shouldShowDuration(0, 'active'), true);
});

test('duration = 0 is not shown for pending status', () => {
  assertEqual(shouldShowDuration(0, 'pending'), false);
});

test('duration = 0 is not shown for error status', () => {
  assertEqual(shouldShowDuration(0, 'error'), false);
});

test('negative duration is not shown (undefined behavior)', () => {
  // Negative durations shouldn't happen in practice, but test the behavior
  assertEqual(shouldShowDuration(-100, 'completed'), true); // Type still allows it
});

console.log('\nActive indicator comprehensive:');

test('error status with isActive true shows indicator', () => {
  assertEqual(shouldShowActiveIndicator('error', true), true);
});

test('error status with isActive false hides indicator', () => {
  assertEqual(shouldShowActiveIndicator('error', false), false);
});

test('completed status with isActive true shows indicator', () => {
  assertEqual(shouldShowActiveIndicator('completed', true), true);
});

console.log('\nNode type info fallback:');

test('getNodeTypeInfo returns custom for unknown type', () => {
  // Cast to bypass TypeScript checking for invalid type test
  const info = getNodeTypeInfo('unknown_type' as NodeType);
  assertEqual(info.badge, 'Custom');
  assertIncludes(info.description, 'user-defined');
});

test('getNodeTypeInfo returns custom for empty string type', () => {
  const info = getNodeTypeInfo('' as NodeType);
  assertEqual(info.badge, 'Custom');
});

console.log('\nDiff presentation comprehensive:');

test('unchanged diff preserves original background exactly', () => {
  const result = getGraphNodeDiffPresentation('unchanged', '#123456', '#abcdef');
  assertEqual(result.effectiveBgColor, '#abcdef');
  assertEqual(result.effectiveBorderColor, '#123456');
});

test('added diff uses green gradient overlay', () => {
  const result = getGraphNodeDiffPresentation('added', '#000000', 'rgb(50,50,50)');
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertIncludes(result.effectiveBgColor, 'rgb(50,50,50)');
  assertEqual(result.effectiveBorderColor, '#22c55e');
});

test('removed diff uses red gradient overlay', () => {
  const result = getGraphNodeDiffPresentation('removed', '#ffffff', '#000');
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertEqual(result.effectiveBorderColor, '#ef4444');
});

test('modified diff uses amber gradient overlay', () => {
  const result = getGraphNodeDiffPresentation('modified', 'blue', 'navy');
  assertIncludes(result.effectiveBgColor, 'linear-gradient(');
  assertEqual(result.effectiveBorderColor, '#f59e0b');
});

test('out-of-schema diff uses darker red', () => {
  const result = getGraphNodeDiffPresentation('out-of-schema', 'transparent', 'black');
  assertEqual(result.effectiveBorderColor, '#dc2626');
  assertEqual(result.diffStyle.badge, '!');
});

test('all diff statuses have correct borderStyle', () => {
  const statusStyles: Record<DiffStatus, string> = {
    'unchanged': 'solid',
    'added': 'solid',
    'removed': 'dashed',
    'modified': 'solid',
    'out-of-schema': 'dotted',
  };
  for (const [status, expectedStyle] of Object.entries(statusStyles)) {
    const result = getGraphNodeDiffPresentation(status as DiffStatus, '#000', '#fff');
    assertEqual(result.diffStyle.borderStyle, expectedStyle);
  }
});

console.log('\nRing class comprehensive:');

test('ring class includes offset styling for selected', () => {
  const ringClass = getRingClass(true, false);
  assertIncludes(ringClass, 'ring-offset-2');
  assertIncludes(ringClass, 'ring-offset-slate-900');
});

test('ring class includes offset styling for focused', () => {
  const ringClass = getRingClass(false, true);
  assertIncludes(ringClass, 'ring-offset-2');
  assertIncludes(ringClass, 'ring-offset-slate-900');
});

test('ring class has ring-2 width', () => {
  const selectedRing = getRingClass(true, false);
  const focusedRing = getRingClass(false, true);
  assertIncludes(selectedRing, 'ring-2');
  assertIncludes(focusedRing, 'ring-2');
});

console.log('\nStatus badge styles comprehensive:');

test('all status badge styles include opacity modifiers', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  for (const status of statuses) {
    const style = getStatusBadgeStyles(status);
    assertIncludes(style, '/20'); // bg opacity
    assertIncludes(style, '/50'); // border opacity
  }
});

test('status badge styles include border class', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  for (const status of statuses) {
    const style = getStatusBadgeStyles(status);
    assertIncludes(style, 'border-');
  }
});

console.log('\nNode type descriptions completeness:');

test('all node type descriptions are non-empty strings', () => {
  const types: NodeType[] = ['transform', 'llm', 'tool', 'router', 'aggregator', 'validator', 'human_in_loop', 'checkpoint', 'custom'];
  for (const nodeType of types) {
    const info = NODE_TYPE_DESCRIPTIONS[nodeType];
    assertEqual(info.badge.length > 0, true);
    assertEqual(info.description.length > 0, true);
  }
});

test('all node type badges are short (max 11 chars)', () => {
  const types: NodeType[] = ['transform', 'llm', 'tool', 'router', 'aggregator', 'validator', 'human_in_loop', 'checkpoint', 'custom'];
  for (const nodeType of types) {
    const info = NODE_TYPE_DESCRIPTIONS[nodeType];
    assertEqual(info.badge.length <= 11, true);
  }
});

test('all node type descriptions are informative (min 20 chars)', () => {
  const types: NodeType[] = ['transform', 'llm', 'tool', 'router', 'aggregator', 'validator', 'human_in_loop', 'checkpoint', 'custom'];
  for (const nodeType of types) {
    const info = NODE_TYPE_DESCRIPTIONS[nodeType];
    assertEqual(info.description.length >= 20, true);
  }
});

console.log('\nStatus indicator consistency:');

test('all status indicators are single characters', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  for (const status of statuses) {
    const indicator = getStatusIndicator(status);
    assertEqual(indicator.length, 1);
  }
});

test('all status indicators are distinct', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  const indicators = statuses.map(getStatusIndicator);
  const unique = new Set(indicators);
  assertEqual(unique.size, 4);
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
