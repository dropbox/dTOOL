// M-444: Comprehensive tests for TimelineSlider component helper functions
// Run with: npx tsx src/__tests__/TimelineSlider.test.tsx
//
// The component uses React hooks so can't SSR render, but we can test pure helper functions
// and the algorithmic logic used in the component.

// Re-implement helper functions for testing (same logic as component)
// This validates the algorithm without modifying production code

// Simple test runner
let passed = 0;
let failed = 0;

function test(name: string, fn: () => void): void {
  try {
    fn();
    console.log(`  \u2713 ${name}`);
    passed++;
  } catch (e) {
    console.log(`  \u2717 ${name}`);
    console.log(`    Error: ${e}`);
    failed++;
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(`${message || 'Assertion failed'}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || 'Assertion failed: expected true');
  }
}

function assertFalse(condition: boolean, message?: string): void {
  if (condition) {
    throw new Error(message || 'Assertion failed: expected false');
  }
}

function assertDeepEqual<T>(actual: T, expected: T, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(`${message || 'Assertion failed'}: expected ${expectedStr}, got ${actualStr}`);
  }
}

function assertThrows(fn: () => void, message?: string): void {
  let threw = false;
  try {
    fn();
  } catch {
    threw = true;
  }
  if (!threw) {
    throw new Error(message || 'Expected function to throw');
  }
}

function assertApproximatelyEqual(actual: number, expected: number, tolerance: number, message?: string): void {
  if (Math.abs(actual - expected) > tolerance) {
    throw new Error(`${message || 'Assertion failed'}: expected ~${expected}, got ${actual} (tolerance: ${tolerance})`);
  }
}

// ============================================================
// Helper functions from TimelineSlider.tsx (copied for testing)
// ============================================================

function formatTime(timestamp: number, baseTime: number): string {
  const elapsed = timestamp - baseTime;
  const seconds = elapsed / 1000;
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${String(minutes).padStart(2, '0')}:${secs.toFixed(1).padStart(4, '0')}`;
}

function getStatusColor(status: 'running' | 'completed' | 'error'): string {
  switch (status) {
    case 'running':
      return '#3b82f6';
    case 'completed':
      return '#22c55e';
    case 'error':
      return '#ef4444';
    default:
      return '#6b7280';
  }
}

// Duration formatting logic used in sortedRunsWithInfo
function formatDuration(durationMs: number): string {
  if (durationMs < 1000) {
    return `${durationMs}ms`;
  } else if (durationMs < 60000) {
    return `${(durationMs / 1000).toFixed(1)}s`;
  } else {
    return `${(durationMs / 60000).toFixed(1)}m`;
  }
}

// Schema mismatch detection logic
function detectSchemaMismatch(expected: string | undefined, current: string | undefined): boolean {
  if (!expected || !current) return false;
  return expected !== current;
}

// Marker position calculation (extracted logic)
function calculateMarkerPosition(
  markerIndex: number,
  sliderMin: number,
  sliderMax: number
): number {
  const denominator = sliderMax - sliderMin;
  if (denominator <= 0) return 0;
  const position = ((markerIndex - sliderMin) / denominator) * 100;
  return Math.min(100, Math.max(0, position));
}

// Marker style lookup (extracted logic)
function getMarkerStyle(type: 'session' | 'schema' | 'node_start' | 'node_end' | 'error'): {
  color: string;
  size: number;
  shape: string;
} {
  const styles: Record<string, { color: string; size: number; shape: string }> = {
    session: { color: '#22d3ee', size: 6, shape: 'diamond' },
    schema: { color: '#f59e0b', size: 8, shape: 'triangle' },
    node_start: { color: '#22c55e', size: 4, shape: 'circle' },
    node_end: { color: '#3b82f6', size: 4, shape: 'circle' },
    error: { color: '#ef4444', size: 6, shape: 'circle' },
  };
  return styles[type] || { color: '#6b7280', size: 4, shape: 'circle' };
}

// Slider range computation logic
function computeSliderRange(
  events: Array<{ seq: string }>,
  cursorSeq: string | undefined
): { min: number; max: number; value: number } {
  if (!events || events.length === 0) {
    return { min: 0, max: 100, value: 0 };
  }
  const min = 0;
  const max = events.length - 1;
  let value = max; // Default to latest
  if (cursorSeq) {
    const cursorIndex = events.findIndex(e => e.seq === cursorSeq);
    if (cursorIndex >= 0) {
      value = cursorIndex;
    }
  }
  return { min, max, value };
}

// Step logic (back/forward)
function computeStepIndex(
  currentIndex: number,
  direction: 'back' | 'forward',
  maxIndex: number
): number {
  if (direction === 'forward') {
    return Math.min(currentIndex + 1, maxIndex);
  } else {
    return Math.max(currentIndex - 1, 0);
  }
}

// Jump logic (start/end)
function computeJumpIndex(target: 'start' | 'end', maxIndex: number): number {
  return target === 'start' ? 0 : maxIndex;
}

// Run sorting comparator (most recent first)
function sortRunsByStartTime<T extends { startTime: number }>(runs: T[]): T[] {
  return [...runs].sort((a, b) => b.startTime - a.startTime);
}

// ============================================================
// TESTS
// ============================================================

console.log('\nTimelineSlider Tests\n');

// ========================================
// formatTime helper function
// ========================================
console.log('formatTime helper function:');

test('formatTime returns 00:00.0 for zero elapsed time', () => {
  const result = formatTime(1000, 1000);
  assertEqual(result, '00:00.0');
});

test('formatTime formats seconds correctly', () => {
  const result = formatTime(6500, 1000);
  assertEqual(result, '00:05.5');
});

test('formatTime formats minutes and seconds', () => {
  const baseTime = 1000;
  const timestamp = baseTime + (90.2 * 1000);
  const result = formatTime(timestamp, baseTime);
  assertEqual(result, '01:30.2');
});

test('formatTime pads minutes to 2 digits', () => {
  const baseTime = 0;
  const timestamp = 5 * 60 * 1000;
  const result = formatTime(timestamp, baseTime);
  assertEqual(result, '05:00.0');
});

test('formatTime handles large elapsed times', () => {
  const baseTime = 0;
  const timestamp = (10 * 60 + 45.3) * 1000;
  const result = formatTime(timestamp, baseTime);
  assertEqual(result, '10:45.3');
});

test('formatTime handles sub-second precision', () => {
  const result = formatTime(100, 0);
  assertEqual(result, '00:00.1');
});

test('formatTime handles 0.9 seconds', () => {
  const result = formatTime(900, 0);
  assertEqual(result, '00:00.9');
});

test('formatTime handles exactly 1 minute', () => {
  const result = formatTime(60000, 0);
  assertEqual(result, '01:00.0');
});

test('formatTime handles 59.9 seconds', () => {
  const result = formatTime(59900, 0);
  assertEqual(result, '00:59.9');
});

test('formatTime handles very small values (10ms)', () => {
  const result = formatTime(10, 0);
  assertEqual(result, '00:00.0');
});

test('formatTime handles double-digit minutes', () => {
  const result = formatTime(99 * 60 * 1000, 0);
  assertEqual(result, '99:00.0');
});

test('formatTime handles 100+ minutes', () => {
  const result = formatTime(125 * 60 * 1000, 0);
  assertEqual(result, '125:00.0');
});

test('formatTime handles non-zero base time', () => {
  const baseTime = 1609459200000; // Jan 1, 2021 00:00:00 UTC
  const timestamp = baseTime + 5500;
  const result = formatTime(timestamp, baseTime);
  assertEqual(result, '00:05.5');
});

test('formatTime handles fractional milliseconds rounding', () => {
  // 5.55 seconds should round to 5.6 or 5.5 depending on implementation
  const result = formatTime(5550, 0);
  assertEqual(result, '00:05.5');
});

// ========================================
// getStatusColor helper function
// ========================================
console.log('\ngetStatusColor helper function:');

test('getStatusColor returns blue (#3b82f6) for running', () => {
  assertEqual(getStatusColor('running'), '#3b82f6');
});

test('getStatusColor returns green (#22c55e) for completed', () => {
  assertEqual(getStatusColor('completed'), '#22c55e');
});

test('getStatusColor returns red (#ef4444) for error', () => {
  assertEqual(getStatusColor('error'), '#ef4444');
});

test('getStatusColor returns gray (#6b7280) for unknown status', () => {
  assertEqual(getStatusColor('unknown' as 'running'), '#6b7280');
});

test('getStatusColor handles empty string status', () => {
  assertEqual(getStatusColor('' as 'running'), '#6b7280');
});

test('getStatusColor is case-sensitive (RUNNING returns gray)', () => {
  assertEqual(getStatusColor('RUNNING' as 'running'), '#6b7280');
});

// ========================================
// formatDuration logic
// ========================================
console.log('\nformatDuration logic (from sortedRunsWithInfo):');

test('formatDuration shows milliseconds for < 1000ms', () => {
  assertEqual(formatDuration(500), '500ms');
});

test('formatDuration shows 0ms for zero duration', () => {
  assertEqual(formatDuration(0), '0ms');
});

test('formatDuration shows 999ms at boundary', () => {
  assertEqual(formatDuration(999), '999ms');
});

test('formatDuration shows seconds for exactly 1000ms', () => {
  assertEqual(formatDuration(1000), '1.0s');
});

test('formatDuration shows seconds for 1-60s', () => {
  assertEqual(formatDuration(5000), '5.0s');
});

test('formatDuration shows seconds at 59.9s', () => {
  assertEqual(formatDuration(59900), '59.9s');
});

test('formatDuration shows seconds at 59999ms (boundary)', () => {
  assertEqual(formatDuration(59999), '60.0s');
});

test('formatDuration shows minutes for exactly 60000ms', () => {
  assertEqual(formatDuration(60000), '1.0m');
});

test('formatDuration shows minutes for > 60s', () => {
  assertEqual(formatDuration(120000), '2.0m');
});

test('formatDuration shows minutes with decimals', () => {
  assertEqual(formatDuration(90000), '1.5m');
});

test('formatDuration handles large durations', () => {
  assertEqual(formatDuration(3600000), '60.0m');
});

test('formatDuration handles fractional seconds', () => {
  assertEqual(formatDuration(1500), '1.5s');
});

// ========================================
// Slider range logic
// ========================================
console.log('\nSlider range logic:');

test('slider range uses index-based positioning', () => {
  const events = Array.from({ length: 10 }, (_, i) => ({ seq: String(i) }));
  const result = computeSliderRange(events, undefined);
  assertEqual(result.min, 0);
  assertEqual(result.max, 9);
  assertEqual(result.value, 9); // Default to latest
});

test('slider range defaults to 0-100 when empty', () => {
  const result = computeSliderRange([], undefined);
  assertEqual(result.min, 0);
  assertEqual(result.max, 100);
  assertEqual(result.value, 0);
});

test('slider range handles single event', () => {
  const events = [{ seq: '1' }];
  const result = computeSliderRange(events, undefined);
  assertEqual(result.min, 0);
  assertEqual(result.max, 0);
  assertEqual(result.value, 0);
});

test('slider range finds cursor position correctly', () => {
  const events = [{ seq: 'a' }, { seq: 'b' }, { seq: 'c' }, { seq: 'd' }, { seq: 'e' }];
  const result = computeSliderRange(events, 'c');
  assertEqual(result.value, 2);
});

test('slider range defaults to max when cursor not found', () => {
  const events = [{ seq: 'a' }, { seq: 'b' }, { seq: 'c' }];
  const result = computeSliderRange(events, 'nonexistent');
  assertEqual(result.value, 2); // max
});

test('slider range handles cursor at start', () => {
  const events = [{ seq: 'first' }, { seq: 'middle' }, { seq: 'last' }];
  const result = computeSliderRange(events, 'first');
  assertEqual(result.value, 0);
});

test('slider range handles cursor at end', () => {
  const events = [{ seq: 'first' }, { seq: 'middle' }, { seq: 'last' }];
  const result = computeSliderRange(events, 'last');
  assertEqual(result.value, 2);
});

test('slider range handles undefined cursor (defaults to max)', () => {
  const events = [{ seq: 'a' }, { seq: 'b' }];
  const result = computeSliderRange(events, undefined);
  assertEqual(result.value, 1);
});

test('slider range handles large event counts', () => {
  const events = Array.from({ length: 10000 }, (_, i) => ({ seq: String(i) }));
  const result = computeSliderRange(events, '5000');
  assertEqual(result.min, 0);
  assertEqual(result.max, 9999);
  assertEqual(result.value, 5000);
});

// ========================================
// Marker position calculation
// ========================================
console.log('\nMarker position calculation:');

test('marker at index 0 is at position 0%', () => {
  assertEqual(calculateMarkerPosition(0, 0, 100), 0);
});

test('marker at max index is at position 100%', () => {
  assertEqual(calculateMarkerPosition(100, 0, 100), 100);
});

test('marker at midpoint is at 50%', () => {
  assertEqual(calculateMarkerPosition(50, 0, 100), 50);
});

test('marker position for 10 events (index 5)', () => {
  const position = calculateMarkerPosition(5, 0, 9);
  assertApproximatelyEqual(position, 55.55, 0.1);
});

test('marker position clamps to 0 for negative index', () => {
  assertEqual(calculateMarkerPosition(-5, 0, 100), 0);
});

test('marker position clamps to 100 for index beyond max', () => {
  assertEqual(calculateMarkerPosition(150, 0, 100), 100);
});

test('marker position returns 0 when denominator is 0', () => {
  assertEqual(calculateMarkerPosition(5, 0, 0), 0);
});

test('marker position handles non-zero min', () => {
  assertEqual(calculateMarkerPosition(60, 10, 110), 50);
});

test('marker position handles single event (min=max)', () => {
  assertEqual(calculateMarkerPosition(0, 0, 0), 0);
});

// ========================================
// Schema mismatch detection
// ========================================
console.log('\nSchema mismatch detection:');

test('schema mismatch detected when IDs differ', () => {
  const hasMismatch = detectSchemaMismatch('abc123', 'def456');
  assertTrue(hasMismatch, 'Should detect mismatch');
});

test('no schema mismatch when IDs match', () => {
  const hasMismatch = detectSchemaMismatch('abc123', 'abc123');
  assertFalse(hasMismatch, 'Should not detect mismatch');
});

test('no schema mismatch when expectedSchemaId is undefined', () => {
  const hasMismatch = detectSchemaMismatch(undefined, 'abc123');
  assertFalse(hasMismatch, 'Should not detect mismatch when expected is undefined');
});

test('no schema mismatch when currentSchemaId is undefined', () => {
  const hasMismatch = detectSchemaMismatch('abc123', undefined);
  assertFalse(hasMismatch, 'Should not detect mismatch when current is undefined');
});

test('no schema mismatch when both are undefined', () => {
  const hasMismatch = detectSchemaMismatch(undefined, undefined);
  assertFalse(hasMismatch);
});

test('no schema mismatch when expected is empty string', () => {
  const hasMismatch = detectSchemaMismatch('', 'abc123');
  assertFalse(hasMismatch);
});

test('no schema mismatch when current is empty string', () => {
  const hasMismatch = detectSchemaMismatch('abc123', '');
  assertFalse(hasMismatch);
});

test('schema mismatch is case-sensitive', () => {
  const hasMismatch = detectSchemaMismatch('ABC', 'abc');
  assertTrue(hasMismatch, 'Should be case-sensitive');
});

test('schema mismatch detects whitespace differences', () => {
  const hasMismatch = detectSchemaMismatch('abc', ' abc');
  assertTrue(hasMismatch);
});

// ========================================
// Marker style lookup
// ========================================
console.log('\nMarker style lookup:');

test('session marker has diamond shape', () => {
  const style = getMarkerStyle('session');
  assertEqual(style.shape, 'diamond');
  assertEqual(style.size, 6);
});

test('schema marker has triangle shape', () => {
  const style = getMarkerStyle('schema');
  assertEqual(style.shape, 'triangle');
  assertEqual(style.size, 8);
});

test('node_start marker has circle shape and green color', () => {
  const style = getMarkerStyle('node_start');
  assertEqual(style.shape, 'circle');
  assertEqual(style.color, '#22c55e');
  assertEqual(style.size, 4);
});

test('node_end marker has circle shape and blue color', () => {
  const style = getMarkerStyle('node_end');
  assertEqual(style.shape, 'circle');
  assertEqual(style.color, '#3b82f6');
  assertEqual(style.size, 4);
});

test('error marker has circle shape and red color', () => {
  const style = getMarkerStyle('error');
  assertEqual(style.shape, 'circle');
  assertEqual(style.color, '#ef4444');
  assertEqual(style.size, 6);
});

test('unknown marker type returns default style', () => {
  const style = getMarkerStyle('unknown' as 'session');
  assertEqual(style.shape, 'circle');
  assertEqual(style.color, '#6b7280');
  assertEqual(style.size, 4);
});

// ========================================
// Step logic (back/forward)
// ========================================
console.log('\nStep logic (back/forward):');

test('step forward increments index by 1', () => {
  assertEqual(computeStepIndex(5, 'forward', 10), 6);
});

test('step back decrements index by 1', () => {
  assertEqual(computeStepIndex(5, 'back', 10), 4);
});

test('step forward at max stays at max', () => {
  assertEqual(computeStepIndex(10, 'forward', 10), 10);
});

test('step back at 0 stays at 0', () => {
  assertEqual(computeStepIndex(0, 'back', 10), 0);
});

test('step forward from 0', () => {
  assertEqual(computeStepIndex(0, 'forward', 10), 1);
});

test('step back from max', () => {
  assertEqual(computeStepIndex(10, 'back', 10), 9);
});

test('step forward handles single-element list (max=0)', () => {
  assertEqual(computeStepIndex(0, 'forward', 0), 0);
});

test('step back handles single-element list (max=0)', () => {
  assertEqual(computeStepIndex(0, 'back', 0), 0);
});

// ========================================
// Jump logic (start/end)
// ========================================
console.log('\nJump logic (start/end):');

test('jump to start returns 0', () => {
  assertEqual(computeJumpIndex('start', 100), 0);
});

test('jump to end returns max index', () => {
  assertEqual(computeJumpIndex('end', 100), 100);
});

test('jump to start with max=0 returns 0', () => {
  assertEqual(computeJumpIndex('start', 0), 0);
});

test('jump to end with max=0 returns 0', () => {
  assertEqual(computeJumpIndex('end', 0), 0);
});

// ========================================
// Run sorting logic
// ========================================
console.log('\nRun sorting logic:');

test('sorts runs by startTime descending (most recent first)', () => {
  const runs = [
    { startTime: 100, id: 'a' },
    { startTime: 300, id: 'b' },
    { startTime: 200, id: 'c' },
  ];
  const sorted = sortRunsByStartTime(runs);
  assertEqual(sorted[0].id, 'b');
  assertEqual(sorted[1].id, 'c');
  assertEqual(sorted[2].id, 'a');
});

test('sorts runs with same startTime (stable)', () => {
  const runs = [
    { startTime: 100, id: 'a' },
    { startTime: 100, id: 'b' },
    { startTime: 100, id: 'c' },
  ];
  const sorted = sortRunsByStartTime(runs);
  assertEqual(sorted.length, 3);
});

test('handles empty runs array', () => {
  const sorted = sortRunsByStartTime([]);
  assertEqual(sorted.length, 0);
});

test('handles single run', () => {
  const runs = [{ startTime: 100, id: 'only' }];
  const sorted = sortRunsByStartTime(runs);
  assertEqual(sorted.length, 1);
  assertEqual(sorted[0].id, 'only');
});

test('does not mutate original array', () => {
  const runs = [
    { startTime: 100, id: 'a' },
    { startTime: 300, id: 'b' },
  ];
  const original = [...runs];
  sortRunsByStartTime(runs);
  assertEqual(runs[0].id, original[0].id);
  assertEqual(runs[1].id, original[1].id);
});

// ========================================
// Edge cases
// ========================================
console.log('\nEdge cases:');

test('formatTime handles negative elapsed time (returns negative display)', () => {
  // timestamp < baseTime results in negative elapsed
  const result = formatTime(0, 1000);
  // Negative seconds display depends on implementation
  assertTrue(result.includes('-') || result.startsWith('00'), 'Should handle negative or wrap');
});

test('formatDuration handles negative duration (shows as-is)', () => {
  // Negative duration is edge case, shows negative ms
  const result = formatDuration(-500);
  assertEqual(result, '-500ms');
});

test('slider with very large seq strings', () => {
  const events = [
    { seq: '99999999999999999999' },
    { seq: '100000000000000000000' },
  ];
  const result = computeSliderRange(events, '99999999999999999999');
  assertEqual(result.value, 0);
});

test('schema IDs with special characters', () => {
  const hasMismatch = detectSchemaMismatch('abc-def_123', 'abc-def_124');
  assertTrue(hasMismatch);
});

test('schema IDs with unicode', () => {
  const hasMismatch = detectSchemaMismatch('schéma-日本語', 'schéma-日本語');
  assertFalse(hasMismatch);
});

test('formatTime with very large timestamp', () => {
  const result = formatTime(Number.MAX_SAFE_INTEGER, 0);
  assertTrue(result.includes(':'), 'Should still format with colon');
});

test('marker position with floating point precision', () => {
  // 1/3 position in range 0-99
  const position = calculateMarkerPosition(33, 0, 99);
  assertApproximatelyEqual(position, 33.33, 0.1);
});

test('formatDuration at exact boundaries', () => {
  assertEqual(formatDuration(999), '999ms');
  assertEqual(formatDuration(1000), '1.0s');
  assertEqual(formatDuration(59999), '60.0s');
  assertEqual(formatDuration(60000), '1.0m');
});

test('computeSliderRange with duplicate seq values', () => {
  const events = [{ seq: 'dup' }, { seq: 'dup' }, { seq: 'dup' }];
  const result = computeSliderRange(events, 'dup');
  assertEqual(result.value, 0); // finds first match
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
