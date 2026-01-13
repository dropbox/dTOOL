// M-2570: Tests for Tooltip component
// Run with: npx tsx src/__tests__/Tooltip.test.tsx

import {
  Tooltip,
  computeTooltipCoords,
  computeTooltipCoordsUnclamped,
  clampTooltipCoordsToViewport,
} from '../components/Tooltip';

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

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(`${message || 'Assertion failed'}: expected ${expected}, got ${actual}`);
  }
}

function assertTruthy(value: unknown, message?: string): void {
  if (!value) {
    throw new Error(`${message || 'Assertion failed'}: expected truthy value, got ${value}`);
  }
}

console.log('\nTooltip Tests\n');

console.log('Component export:');
test('Tooltip is exported as named export', () => {
  assertEqual(typeof Tooltip, 'function', 'Tooltip should be a function');
});

test('Tooltip is a React functional component', () => {
  // React functional components have a name property
  assertTruthy(Tooltip.name, 'Should have a name');
});

// Test position calculation logic
// The logic from Tooltip.tsx's updatePosition() switch statement
console.log('\nPosition calculation logic:');

interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
  right: number;
  bottom: number;
}

const triggerRect: Rect = {
  left: 100,
  top: 100,
  width: 80,
  height: 30,
  right: 180,
  bottom: 130,
};

const tooltipRect: Rect = {
  left: 0,
  top: 0,
  width: 100,
  height: 40,
  right: 100,
  bottom: 40,
};

test('top position centers tooltip horizontally above trigger', () => {
  const { x, y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'top');
  // x = 100 + (80 - 100) / 2 = 100 - 10 = 90
  assertEqual(x, 90, 'x should be centered');
  // y = 100 - 40 - 8 = 52
  assertEqual(y, 52, 'y should be above trigger with gap');
});

test('bottom position centers tooltip horizontally below trigger', () => {
  const { x, y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'bottom');
  // x = 100 + (80 - 100) / 2 = 90
  assertEqual(x, 90, 'x should be centered');
  // y = 130 + 8 = 138
  assertEqual(y, 138, 'y should be below trigger with gap');
});

test('left position centers tooltip vertically to left of trigger', () => {
  const { x, y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'left');
  // x = 100 - 100 - 8 = -8
  assertEqual(x, -8, 'x should be left of trigger with gap');
  // y = 100 + (30 - 40) / 2 = 100 - 5 = 95
  assertEqual(y, 95, 'y should be centered');
});

test('right position centers tooltip vertically to right of trigger', () => {
  const { x, y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'right');
  // x = 180 + 8 = 188
  assertEqual(x, 188, 'x should be right of trigger with gap');
  // y = 100 + (30 - 40) / 2 = 95
  assertEqual(y, 95, 'y should be centered');
});

// Test viewport clamping logic
console.log('\nViewport clamping logic:');

test('negative x is clamped to padding', () => {
  const { x } = clampTooltipCoordsToViewport(
    { x: -50, y: 100 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 8, 'x should be clamped to padding');
});

test('x exceeding viewport is clamped', () => {
  const { x } = clampTooltipCoordsToViewport(
    { x: 950, y: 100 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  // Max x = 1000 - 100 - 8 = 892
  assertEqual(x, 892, 'x should be clamped to viewport boundary');
});

test('negative y is clamped to padding', () => {
  const { y } = clampTooltipCoordsToViewport(
    { x: 100, y: -20 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(y, 8, 'y should be clamped to padding');
});

test('y exceeding viewport is clamped', () => {
  const { y } = clampTooltipCoordsToViewport(
    { x: 100, y: 780 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  // Max y = 800 - 40 - 8 = 752
  assertEqual(y, 752, 'y should be clamped to viewport boundary');
});

test('coordinates within viewport are unchanged', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 200, y: 200 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 200, 'x should be unchanged');
  assertEqual(y, 200, 'y should be unchanged');
});

test('tooltip wider than viewport clamps to padding', () => {
  const { x } = clampTooltipCoordsToViewport(
    { x: 10, y: 10 },
    { width: 500, height: 40 },
    { width: 200, height: 800 }
  );
  assertEqual(x, 8, 'x should clamp to padding when tooltip cannot fit');
});

// Test edge cases
console.log('\nEdge cases:');

test('same-sized trigger and tooltip centers correctly', () => {
  const sameSize: Rect = { left: 50, top: 50, width: 100, height: 40, right: 150, bottom: 90 };
  const { x, y } = computeTooltipCoordsUnclamped(
    sameSize,
    sameSize,
    'top'
  );
  // x = 50 + (100 - 100) / 2 = 50
  assertEqual(x, 50, 'x should be at trigger left');
  // y = 50 - 40 - 8 = 2
  assertEqual(y, 2, 'y should be above trigger with gap');
});

test('larger tooltip than trigger still centers', () => {
  const smallTrigger: Rect = { left: 100, top: 100, width: 20, height: 20, right: 120, bottom: 120 };
  const largeTooltip: Rect = { left: 0, top: 0, width: 200, height: 80, right: 200, bottom: 80 };
  const { x } = computeTooltipCoordsUnclamped(smallTrigger, largeTooltip, 'top');
  // x = 100 + (20 - 200) / 2 = 100 - 90 = 10
  assertEqual(x, 10, 'x should be centered (may go negative)');
});

test('combined position + clamp matches component behavior', () => {
  const coords = computeTooltipCoords(
    triggerRect,
    tooltipRect,
    'left',
    { width: 1000, height: 800 }
  );

  // unclamped x is -8, then clamped to padding 8
  assertEqual(coords.x, 8);
  assertEqual(coords.y, 95);
});

// Custom gap parameter tests
console.log('\nCustom gap parameter:');

test('top position with custom gap (0)', () => {
  const { y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'top', 0);
  // y = 100 - 40 - 0 = 60
  assertEqual(y, 60, 'y should have no gap');
});

test('top position with custom gap (20)', () => {
  const { y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'top', 20);
  // y = 100 - 40 - 20 = 40
  assertEqual(y, 40, 'y should have 20px gap');
});

test('bottom position with custom gap (0)', () => {
  const { y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'bottom', 0);
  // y = 130 + 0 = 130
  assertEqual(y, 130, 'y should have no gap');
});

test('bottom position with custom gap (15)', () => {
  const { y } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'bottom', 15);
  // y = 130 + 15 = 145
  assertEqual(y, 145, 'y should have 15px gap');
});

test('left position with custom gap (0)', () => {
  const { x } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'left', 0);
  // x = 100 - 100 - 0 = 0
  assertEqual(x, 0, 'x should have no gap');
});

test('left position with custom gap (12)', () => {
  const { x } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'left', 12);
  // x = 100 - 100 - 12 = -12
  assertEqual(x, -12, 'x should have 12px gap');
});

test('right position with custom gap (0)', () => {
  const { x } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'right', 0);
  // x = 180 + 0 = 180
  assertEqual(x, 180, 'x should have no gap');
});

test('right position with custom gap (25)', () => {
  const { x } = computeTooltipCoordsUnclamped(triggerRect, tooltipRect, 'right', 25);
  // x = 180 + 25 = 205
  assertEqual(x, 205, 'x should have 25px gap');
});

// Custom padding parameter tests
console.log('\nCustom padding parameter:');

test('clamp with custom padding (0)', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: -50, y: -30 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 },
    0
  );
  assertEqual(x, 0, 'x should clamp to 0 with no padding');
  assertEqual(y, 0, 'y should clamp to 0 with no padding');
});

test('clamp with custom padding (20)', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: -50, y: -30 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 },
    20
  );
  assertEqual(x, 20, 'x should clamp to 20 with custom padding');
  assertEqual(y, 20, 'y should clamp to 20 with custom padding');
});

test('clamp respects custom padding on max boundary', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 950, y: 780 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 },
    20
  );
  // Max x = 1000 - 100 - 20 = 880
  // Max y = 800 - 40 - 20 = 740
  assertEqual(x, 880, 'x should clamp to viewport - width - padding');
  assertEqual(y, 740, 'y should clamp to viewport - height - padding');
});

// computeTooltipCoords with custom parameters
console.log('\ncomputeTooltipCoords with custom parameters:');

test('computeTooltipCoords with custom gap only', () => {
  const coords = computeTooltipCoords(
    triggerRect,
    tooltipRect,
    'bottom',
    { width: 1000, height: 800 },
    20  // custom gap
  );
  // x = 100 + (80 - 100) / 2 = 90 (within bounds)
  // y = 130 + 20 = 150 (within bounds)
  assertEqual(coords.x, 90, 'x should be centered');
  assertEqual(coords.y, 150, 'y should use custom gap');
});

test('computeTooltipCoords with custom gap and padding', () => {
  const coords = computeTooltipCoords(
    { left: 0, top: 0, width: 50, height: 30, right: 50, bottom: 30 },
    { width: 100, height: 40 },
    'top',
    { width: 1000, height: 800 },
    10,  // custom gap
    15   // custom viewport padding
  );
  // unclamped: x = 0 + (50 - 100) / 2 = -25, y = 0 - 40 - 10 = -50
  // clamped: x = max(15, -25) = 15, y = max(15, -50) = 15
  assertEqual(coords.x, 15, 'x should clamp to custom padding');
  assertEqual(coords.y, 15, 'y should clamp to custom padding');
});

test('computeTooltipCoords passes through valid coordinates', () => {
  const coords = computeTooltipCoords(
    { left: 400, top: 300, width: 100, height: 50, right: 500, bottom: 350 },
    { width: 150, height: 60 },
    'bottom',
    { width: 1000, height: 800 },
    8,
    8
  );
  // x = 400 + (100 - 150) / 2 = 375 (within bounds)
  // y = 350 + 8 = 358 (within bounds)
  assertEqual(coords.x, 375, 'x should be unchanged');
  assertEqual(coords.y, 358, 'y should be unchanged');
});

// Zero-dimension edge cases
console.log('\nZero-dimension edge cases:');

test('zero width trigger centers tooltip correctly', () => {
  const zeroWidthTrigger: Rect = { left: 100, top: 100, width: 0, height: 30, right: 100, bottom: 130 };
  const { x } = computeTooltipCoordsUnclamped(zeroWidthTrigger, tooltipRect, 'top');
  // x = 100 + (0 - 100) / 2 = 50
  assertEqual(x, 50, 'x should center on zero-width trigger');
});

test('zero height trigger centers tooltip correctly', () => {
  const zeroHeightTrigger: Rect = { left: 100, top: 100, width: 80, height: 0, right: 180, bottom: 100 };
  const { y } = computeTooltipCoordsUnclamped(zeroHeightTrigger, tooltipRect, 'left');
  // y = 100 + (0 - 40) / 2 = 80
  assertEqual(y, 80, 'y should center on zero-height trigger');
});

test('zero width tooltip positions correctly', () => {
  const zeroWidthTooltip = { width: 0, height: 40 };
  const { x } = computeTooltipCoordsUnclamped(triggerRect, zeroWidthTooltip, 'top');
  // x = 100 + (80 - 0) / 2 = 140
  assertEqual(x, 140, 'x should center zero-width tooltip');
});

test('zero height tooltip positions correctly', () => {
  const zeroHeightTooltip = { width: 100, height: 0 };
  const { y } = computeTooltipCoordsUnclamped(triggerRect, zeroHeightTooltip, 'top');
  // y = 100 - 0 - 8 = 92
  assertEqual(y, 92, 'y should position zero-height tooltip');
});

test('both zero dimensions', () => {
  const zeroTrigger: Rect = { left: 50, top: 50, width: 0, height: 0, right: 50, bottom: 50 };
  const zeroTooltip = { width: 0, height: 0 };
  const { x, y } = computeTooltipCoordsUnclamped(zeroTrigger, zeroTooltip, 'top');
  // x = 50 + (0 - 0) / 2 = 50
  // y = 50 - 0 - 8 = 42
  assertEqual(x, 50, 'x should be at trigger position');
  assertEqual(y, 42, 'y should be above with gap');
});

// Exact boundary positions
console.log('\nExact boundary positions:');

test('coordinates exactly at minimum boundary stay unchanged', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 8, y: 8 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 8, 'x at exact min boundary unchanged');
  assertEqual(y, 8, 'y at exact min boundary unchanged');
});

test('coordinates exactly at maximum boundary stay unchanged', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 892, y: 752 },  // 1000 - 100 - 8 = 892, 800 - 40 - 8 = 752
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 892, 'x at exact max boundary unchanged');
  assertEqual(y, 752, 'y at exact max boundary unchanged');
});

test('coordinates one pixel past min boundary are clamped', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 7, y: 7 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 8, 'x past min boundary clamped');
  assertEqual(y, 8, 'y past min boundary clamped');
});

test('coordinates one pixel past max boundary are clamped', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 893, y: 753 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 892, 'x past max boundary clamped');
  assertEqual(y, 752, 'y past max boundary clamped');
});

// Very small viewport
console.log('\nVery small viewport:');

test('viewport smaller than tooltip + 2*padding clamps to padding', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 50, y: 50 },
    { width: 100, height: 40 },
    { width: 50, height: 30 },  // smaller than tooltip
    8
  );
  // Max x = 50 - 100 - 8 = -58, min = 8, so x = 8
  // Max y = 30 - 40 - 8 = -18, min = 8, so y = 8
  assertEqual(x, 8, 'x clamps to min padding in tiny viewport');
  assertEqual(y, 8, 'y clamps to min padding in tiny viewport');
});

test('viewport exactly fits tooltip + padding', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 100, y: 100 },
    { width: 84, height: 34 },
    { width: 100, height: 50 },  // 84 + 8 + 8 = 100, 34 + 8 + 8 = 50
    8
  );
  // Only valid position is x = 8, y = 8
  assertEqual(x, 8, 'x clamps to only valid position');
  assertEqual(y, 8, 'y clamps to only valid position');
});

// All positions with varying size ratios
console.log('\nVarying size ratios:');

test('tooltip much wider than trigger - top position', () => {
  const narrowTrigger: Rect = { left: 500, top: 200, width: 10, height: 20, right: 510, bottom: 220 };
  const wideTooltip = { width: 300, height: 50 };
  const { x } = computeTooltipCoordsUnclamped(narrowTrigger, wideTooltip, 'top');
  // x = 500 + (10 - 300) / 2 = 500 - 145 = 355
  assertEqual(x, 355, 'wide tooltip centers on narrow trigger');
});

test('tooltip much taller than trigger - left position', () => {
  const shortTrigger: Rect = { left: 300, top: 400, width: 80, height: 10, right: 380, bottom: 410 };
  const tallTooltip = { width: 100, height: 200 };
  const { y } = computeTooltipCoordsUnclamped(shortTrigger, tallTooltip, 'left');
  // y = 400 + (10 - 200) / 2 = 400 - 95 = 305
  assertEqual(y, 305, 'tall tooltip centers on short trigger');
});

test('tooltip much smaller than trigger - bottom position', () => {
  const largeTrigger: Rect = { left: 100, top: 100, width: 400, height: 200, right: 500, bottom: 300 };
  const smallTooltip = { width: 50, height: 20 };
  const { x, y } = computeTooltipCoordsUnclamped(largeTrigger, smallTooltip, 'bottom');
  // x = 100 + (400 - 50) / 2 = 100 + 175 = 275
  // y = 300 + 8 = 308
  assertEqual(x, 275, 'small tooltip centers on large trigger');
  assertEqual(y, 308, 'y positioned below large trigger');
});

test('tooltip same size as trigger - right position', () => {
  const sameSizeRect: Rect = { left: 200, top: 200, width: 100, height: 50, right: 300, bottom: 250 };
  const sameSizeTooltip = { width: 100, height: 50 };
  const { x, y } = computeTooltipCoordsUnclamped(sameSizeRect, sameSizeTooltip, 'right');
  // x = 300 + 8 = 308
  // y = 200 + (50 - 50) / 2 = 200
  assertEqual(x, 308, 'same-size tooltip to right of trigger');
  assertEqual(y, 200, 'same-size tooltip vertically aligned');
});

// Fractional coordinate handling
console.log('\nFractional coordinates:');

test('handles fractional trigger dimensions', () => {
  const fractionalTrigger: Rect = { left: 100.5, top: 200.25, width: 80.5, height: 30.75, right: 181, bottom: 231 };
  const { x, y } = computeTooltipCoordsUnclamped(fractionalTrigger, tooltipRect, 'top');
  // x = 100.5 + (80.5 - 100) / 2 = 100.5 - 9.75 = 90.75
  // y = 200.25 - 40 - 8 = 152.25
  assertEqual(x, 90.75, 'handles fractional x');
  assertEqual(y, 152.25, 'handles fractional y');
});

test('clamping preserves fractional values within bounds', () => {
  const { x, y } = clampTooltipCoordsToViewport(
    { x: 100.5, y: 200.75 },
    { width: 100, height: 40 },
    { width: 1000, height: 800 }
  );
  assertEqual(x, 100.5, 'fractional x preserved when in bounds');
  assertEqual(y, 200.75, 'fractional y preserved when in bounds');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
