// Unit tests for GraphNodeLogic pure functions
// Run with: npx tsx src/__tests__/GraphNodeLogic.test.ts

import {
  NODE_TYPE_DESCRIPTIONS,
  getNodeTypeInfo,
  getStatusIndicator,
  getStatusBadgeStyles,
  getAnimationClass,
  getRingClass,
  getGraphNodeAriaLabel,
  shouldShowDuration,
  formatDurationText,
  shouldShowActiveIndicator,
  borderWidthForDiffStatus,
  getDurationPillStyle,
  getGraphNodeDiffPresentation,
} from '../components/GraphNodeLogic';
import type { NodeStatus, NodeType } from '../types/graph';
import type { DiffStatus } from '../types/schemaDiff';
import { DIFF_STATUS_STYLES } from '../types/schemaDiff';

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

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(`${message || 'Assertion failed'}: "${haystack}" does not include "${needle}"`);
  }
}

// ============================================================
// NODE_TYPE_DESCRIPTIONS export
// ============================================================
console.log('\nNODE_TYPE_DESCRIPTIONS:');

test('contains all 9 node types', () => {
  const types: NodeType[] = [
    'transform', 'llm', 'tool', 'router', 'aggregator',
    'validator', 'human_in_loop', 'checkpoint', 'custom'
  ];
  for (const t of types) {
    assertTrue(t in NODE_TYPE_DESCRIPTIONS, `Missing ${t}`);
  }
});

test('each entry has badge and description', () => {
  for (const [key, val] of Object.entries(NODE_TYPE_DESCRIPTIONS)) {
    assertTrue(typeof val.badge === 'string' && val.badge.length > 0, `${key} badge empty`);
    assertTrue(typeof val.description === 'string' && val.description.length > 0, `${key} description empty`);
  }
});

test('transform has correct badge', () => {
  assertEqual(NODE_TYPE_DESCRIPTIONS.transform.badge, 'Transform');
});

test('llm has correct badge', () => {
  assertEqual(NODE_TYPE_DESCRIPTIONS.llm.badge, 'LLM');
});

test('human_in_loop has Human badge', () => {
  assertEqual(NODE_TYPE_DESCRIPTIONS.human_in_loop.badge, 'Human');
});

// ============================================================
// getNodeTypeInfo
// ============================================================
console.log('\ngetNodeTypeInfo:');

test('returns correct info for transform', () => {
  const info = getNodeTypeInfo('transform');
  assertEqual(info.badge, 'Transform');
  assertIncludes(info.description, 'Pure function');
});

test('returns correct info for llm', () => {
  const info = getNodeTypeInfo('llm');
  assertEqual(info.badge, 'LLM');
  assertIncludes(info.description, 'Language model');
});

test('returns correct info for tool', () => {
  const info = getNodeTypeInfo('tool');
  assertEqual(info.badge, 'Tool');
  assertIncludes(info.description, 'External tool');
});

test('returns correct info for router', () => {
  const info = getNodeTypeInfo('router');
  assertEqual(info.badge, 'Router');
  assertIncludes(info.description, 'Conditional routing');
});

test('returns correct info for aggregator', () => {
  const info = getNodeTypeInfo('aggregator');
  assertEqual(info.badge, 'Aggregator');
  assertIncludes(info.description, 'Collects');
});

test('returns correct info for validator', () => {
  const info = getNodeTypeInfo('validator');
  assertEqual(info.badge, 'Validator');
  assertIncludes(info.description, 'Validates');
});

test('returns correct info for human_in_loop', () => {
  const info = getNodeTypeInfo('human_in_loop');
  assertEqual(info.badge, 'Human');
  assertIncludes(info.description, 'human review');
});

test('returns correct info for checkpoint', () => {
  const info = getNodeTypeInfo('checkpoint');
  assertEqual(info.badge, 'Checkpoint');
  assertIncludes(info.description, 'Persists state');
});

test('returns correct info for custom', () => {
  const info = getNodeTypeInfo('custom');
  assertEqual(info.badge, 'Custom');
  assertIncludes(info.description, 'user-defined');
});

test('falls back to custom for unknown type', () => {
  const info = getNodeTypeInfo('unknown_type' as NodeType);
  assertEqual(info.badge, 'Custom');
  assertIncludes(info.description, 'user-defined');
});

// ============================================================
// getStatusIndicator
// ============================================================
console.log('\ngetStatusIndicator:');

test('pending returns circle outline', () => {
  assertEqual(getStatusIndicator('pending'), '○');
});

test('active returns filled circle', () => {
  assertEqual(getStatusIndicator('active'), '●');
});

test('completed returns checkmark', () => {
  assertEqual(getStatusIndicator('completed'), '✓');
});

test('error returns X', () => {
  assertEqual(getStatusIndicator('error'), '✗');
});

// ============================================================
// getStatusBadgeStyles
// ============================================================
console.log('\ngetStatusBadgeStyles:');

test('pending returns gray styles', () => {
  const styles = getStatusBadgeStyles('pending');
  assertIncludes(styles, 'bg-gray-500/20');
  assertIncludes(styles, 'border-gray-500/50');
  assertIncludes(styles, 'text-gray-400');
});

test('active returns blue styles', () => {
  const styles = getStatusBadgeStyles('active');
  assertIncludes(styles, 'bg-blue-500/20');
  assertIncludes(styles, 'border-blue-500/50');
  assertIncludes(styles, 'text-blue-400');
});

test('completed returns green styles', () => {
  const styles = getStatusBadgeStyles('completed');
  assertIncludes(styles, 'bg-green-500/20');
  assertIncludes(styles, 'border-green-500/50');
  assertIncludes(styles, 'text-green-400');
});

test('error returns red styles', () => {
  const styles = getStatusBadgeStyles('error');
  assertIncludes(styles, 'bg-red-500/20');
  assertIncludes(styles, 'border-red-500/50');
  assertIncludes(styles, 'text-red-400');
});

// ============================================================
// getAnimationClass
// ============================================================
console.log('\ngetAnimationClass:');

test('returns node-running when status is active', () => {
  assertEqual(getAnimationClass('active', false), 'node-running');
});

test('returns node-running when isActive is true', () => {
  assertEqual(getAnimationClass('pending', true), 'node-running');
});

test('returns node-running when both active status and isActive', () => {
  assertEqual(getAnimationClass('active', true), 'node-running');
});

test('returns empty string when completed and not isActive', () => {
  assertEqual(getAnimationClass('completed', false), '');
});

test('returns empty string when pending and not isActive', () => {
  assertEqual(getAnimationClass('pending', false), '');
});

test('returns empty string when error and not isActive', () => {
  assertEqual(getAnimationClass('error', false), '');
});

// ============================================================
// getRingClass
// ============================================================
console.log('\ngetRingClass:');

test('returns blue ring when selected', () => {
  const cls = getRingClass(true, false);
  assertIncludes(cls, 'ring-2');
  assertIncludes(cls, 'ring-blue-500');
  assertIncludes(cls, 'ring-offset-2');
  assertIncludes(cls, 'ring-offset-slate-900');
});

test('returns purple ring when focused but not selected', () => {
  const cls = getRingClass(false, true);
  assertIncludes(cls, 'ring-2');
  assertIncludes(cls, 'ring-purple-500');
  assertIncludes(cls, 'ring-offset-2');
  assertIncludes(cls, 'ring-offset-slate-900');
});

test('selected takes precedence over focused', () => {
  const cls = getRingClass(true, true);
  assertIncludes(cls, 'ring-blue-500');
  assertFalse(cls.includes('ring-purple-500'), 'Should not include purple');
});

test('returns empty string when neither selected nor focused', () => {
  assertEqual(getRingClass(false, false), '');
});

// ============================================================
// getGraphNodeAriaLabel
// ============================================================
console.log('\ngetGraphNodeAriaLabel:');

test('includes label and status', () => {
  const label = getGraphNodeAriaLabel({ label: 'node1', status: 'pending' });
  assertIncludes(label, 'node1');
  assertIncludes(label, 'status: pending');
});

test('includes description when provided', () => {
  const label = getGraphNodeAriaLabel({
    label: 'node1',
    status: 'active',
    description: 'A test node'
  });
  assertIncludes(label, 'A test node');
});

test('includes duration when provided', () => {
  const label = getGraphNodeAriaLabel({
    label: 'node1',
    status: 'completed',
    duration: 150
  });
  assertIncludes(label, 'duration: 150ms');
});

test('includes all fields when provided', () => {
  const label = getGraphNodeAriaLabel({
    label: 'analyzer',
    status: 'completed',
    description: 'Analyzes data',
    duration: 250
  });
  assertIncludes(label, 'analyzer');
  assertIncludes(label, 'status: completed');
  assertIncludes(label, 'Analyzes data');
  assertIncludes(label, 'duration: 250ms');
});

test('handles zero duration', () => {
  const label = getGraphNodeAriaLabel({
    label: 'fast',
    status: 'completed',
    duration: 0
  });
  assertIncludes(label, 'duration: 0ms');
});

test('omits duration when undefined', () => {
  const label = getGraphNodeAriaLabel({ label: 'node', status: 'pending' });
  assertFalse(label.includes('duration:'), 'Should not include duration');
});

test('handles empty description', () => {
  const label = getGraphNodeAriaLabel({
    label: 'node',
    status: 'pending',
    description: ''
  });
  // Empty description should still be part of the conditional
  assertTrue(label.includes('node'), 'Should include label');
});

// ============================================================
// shouldShowDuration
// ============================================================
console.log('\nshouldShowDuration:');

test('returns true for completed with duration', () => {
  assertTrue(shouldShowDuration(100, 'completed'));
});

test('returns true for active with duration', () => {
  assertTrue(shouldShowDuration(50, 'active'));
});

test('returns false for pending with duration', () => {
  assertFalse(shouldShowDuration(100, 'pending'));
});

test('returns false for error with duration', () => {
  assertFalse(shouldShowDuration(100, 'error'));
});

test('returns false when duration is undefined for completed', () => {
  assertFalse(shouldShowDuration(undefined, 'completed'));
});

test('returns false when duration is undefined for active', () => {
  assertFalse(shouldShowDuration(undefined, 'active'));
});

test('returns true for zero duration on completed', () => {
  assertTrue(shouldShowDuration(0, 'completed'));
});

test('returns true for zero duration on active', () => {
  assertTrue(shouldShowDuration(0, 'active'));
});

// ============================================================
// formatDurationText
// ============================================================
console.log('\nformatDurationText:');

test('adds ellipsis for active status', () => {
  assertEqual(formatDurationText(100, 'active'), '100ms...');
});

test('no ellipsis for completed status', () => {
  assertEqual(formatDurationText(100, 'completed'), '100ms');
});

test('no ellipsis for pending status', () => {
  assertEqual(formatDurationText(100, 'pending'), '100ms');
});

test('no ellipsis for error status', () => {
  assertEqual(formatDurationText(100, 'error'), '100ms');
});

test('handles zero duration', () => {
  assertEqual(formatDurationText(0, 'completed'), '0ms');
});

test('handles zero duration for active', () => {
  assertEqual(formatDurationText(0, 'active'), '0ms...');
});

test('handles large duration values', () => {
  assertEqual(formatDurationText(10000, 'completed'), '10000ms');
});

// ============================================================
// shouldShowActiveIndicator
// ============================================================
console.log('\nshouldShowActiveIndicator:');

test('returns true when status is active', () => {
  assertTrue(shouldShowActiveIndicator('active', false));
});

test('returns true when isActive is true', () => {
  assertTrue(shouldShowActiveIndicator('pending', true));
});

test('returns true when both are active', () => {
  assertTrue(shouldShowActiveIndicator('active', true));
});

test('returns false when pending and not isActive', () => {
  assertFalse(shouldShowActiveIndicator('pending', false));
});

test('returns false when completed and not isActive', () => {
  assertFalse(shouldShowActiveIndicator('completed', false));
});

test('returns false when error and not isActive', () => {
  assertFalse(shouldShowActiveIndicator('error', false));
});

// ============================================================
// borderWidthForDiffStatus
// ============================================================
console.log('\nborderWidthForDiffStatus:');

test('returns 2 for unchanged', () => {
  assertEqual(borderWidthForDiffStatus('unchanged'), 2);
});

test('returns 3 for added', () => {
  assertEqual(borderWidthForDiffStatus('added'), 3);
});

test('returns 3 for removed', () => {
  assertEqual(borderWidthForDiffStatus('removed'), 3);
});

test('returns 3 for modified', () => {
  assertEqual(borderWidthForDiffStatus('modified'), 3);
});

test('returns 3 for out-of-schema', () => {
  assertEqual(borderWidthForDiffStatus('out-of-schema'), 3);
});

// ============================================================
// getDurationPillStyle
// ============================================================
console.log('\ngetDurationPillStyle:');

test('returns blue style for active status', () => {
  const style = getDurationPillStyle('active');
  assertIncludes(style, 'bg-blue-500/20');
  assertIncludes(style, 'text-blue-300');
  assertIncludes(style, 'border');
  assertIncludes(style, 'border-blue-500/30');
});

test('returns dark style for completed status', () => {
  const style = getDurationPillStyle('completed');
  assertIncludes(style, 'bg-black/30');
  assertIncludes(style, 'text-gray-300');
  assertIncludes(style, 'border');
  assertIncludes(style, 'border-gray-600/30');
});

test('returns dark style for pending status', () => {
  const style = getDurationPillStyle('pending');
  assertIncludes(style, 'bg-black/30');
  assertIncludes(style, 'text-gray-300');
});

test('returns dark style for error status', () => {
  const style = getDurationPillStyle('error');
  assertIncludes(style, 'bg-black/30');
  assertIncludes(style, 'text-gray-300');
});

// ============================================================
// getGraphNodeDiffPresentation
// ============================================================
console.log('\ngetGraphNodeDiffPresentation:');

test('unchanged uses original border and bg colors', () => {
  const result = getGraphNodeDiffPresentation('unchanged', '#123456', '#abcdef');
  assertEqual(result.effectiveBorderColor, '#123456');
  assertEqual(result.effectiveBgColor, '#abcdef');
  assertEqual(result.diffStyle, DIFF_STATUS_STYLES['unchanged']);
});

test('added uses diffStyle border color', () => {
  const result = getGraphNodeDiffPresentation('added', '#123456', '#abcdef');
  assertEqual(result.effectiveBorderColor, DIFF_STATUS_STYLES['added'].borderColor);
  assertIncludes(result.effectiveBgColor, 'linear-gradient');
});

test('removed uses diffStyle border color', () => {
  const result = getGraphNodeDiffPresentation('removed', '#123456', '#abcdef');
  assertEqual(result.effectiveBorderColor, DIFF_STATUS_STYLES['removed'].borderColor);
  assertIncludes(result.effectiveBgColor, 'linear-gradient');
});

test('modified uses diffStyle border color', () => {
  const result = getGraphNodeDiffPresentation('modified', '#123456', '#abcdef');
  assertEqual(result.effectiveBorderColor, DIFF_STATUS_STYLES['modified'].borderColor);
  assertIncludes(result.effectiveBgColor, 'linear-gradient');
});

test('out-of-schema uses diffStyle border color', () => {
  const result = getGraphNodeDiffPresentation('out-of-schema', '#123456', '#abcdef');
  assertEqual(result.effectiveBorderColor, DIFF_STATUS_STYLES['out-of-schema'].borderColor);
  assertIncludes(result.effectiveBgColor, 'linear-gradient');
});

test('diffStyle is correctly mapped for each status', () => {
  const statuses: DiffStatus[] = ['unchanged', 'added', 'removed', 'modified', 'out-of-schema'];
  for (const status of statuses) {
    const result = getGraphNodeDiffPresentation(status, '#000', '#fff');
    assertEqual(result.diffStyle, DIFF_STATUS_STYLES[status], `diffStyle mismatch for ${status}`);
  }
});

test('effectiveBgColor includes gradient overlay when diffStyle has bgOverlay', () => {
  const result = getGraphNodeDiffPresentation('added', '#000', '#fff');
  // Added has bgOverlay, so effectiveBgColor should be a gradient
  const bgOverlay = DIFF_STATUS_STYLES['added'].bgOverlay;
  if (bgOverlay) {
    assertIncludes(result.effectiveBgColor, 'linear-gradient');
    assertIncludes(result.effectiveBgColor, bgOverlay);
    assertIncludes(result.effectiveBgColor, '#fff');
  }
});

test('effectiveBorderColor falls back to original when diffStyle.borderColor is empty', () => {
  // unchanged has no borderColor override
  const result = getGraphNodeDiffPresentation('unchanged', '#custom', 'bg');
  // Since DIFF_STATUS_STYLES.unchanged may have borderColor='' or undefined
  const styleColor = DIFF_STATUS_STYLES['unchanged'].borderColor;
  if (!styleColor) {
    assertEqual(result.effectiveBorderColor, '#custom');
  } else {
    assertEqual(result.effectiveBorderColor, styleColor);
  }
});

// ============================================================
// Edge cases and integration
// ============================================================
console.log('\nEdge cases and integration:');

test('all NodeStatus values have getStatusIndicator mapping', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  for (const s of statuses) {
    const indicator = getStatusIndicator(s);
    assertTrue(typeof indicator === 'string' && indicator.length > 0, `No indicator for ${s}`);
  }
});

test('all NodeStatus values have getStatusBadgeStyles mapping', () => {
  const statuses: NodeStatus[] = ['pending', 'active', 'completed', 'error'];
  for (const s of statuses) {
    const styles = getStatusBadgeStyles(s);
    assertTrue(typeof styles === 'string' && styles.length > 0, `No styles for ${s}`);
  }
});

test('all DiffStatus values have borderWidthForDiffStatus handling', () => {
  const statuses: DiffStatus[] = ['unchanged', 'added', 'removed', 'modified', 'out-of-schema'];
  for (const s of statuses) {
    const width = borderWidthForDiffStatus(s);
    assertTrue(width === 2 || width === 3, `Invalid width for ${s}`);
  }
});

test('animation and activeIndicator have same logic', () => {
  // These two functions should produce consistent results
  const cases: Array<[NodeStatus, boolean]> = [
    ['active', false],
    ['active', true],
    ['pending', false],
    ['pending', true],
    ['completed', false],
    ['error', false],
  ];
  for (const [status, isActive] of cases) {
    const anim = getAnimationClass(status, isActive);
    const indicator = shouldShowActiveIndicator(status, isActive);
    // Both should agree on "is this node active?"
    assertEqual(anim !== '', indicator, `Mismatch for ${status}/${isActive}`);
  }
});

// ============================================================
// Results
// ============================================================
console.log('\n========================================');
console.log(`GraphNodeLogic tests: ${passed} passed, ${failed} failed`);
console.log('========================================');

if (failed > 0) {
  process.exit(1);
}
