// M-2592: Unit tests for design tokens
// Run with: npx tsx src/__tests__/tokens.test.ts

import {
  colors,
  spacing,
  fontSize,
  borderRadius,
  shadows,
  durations,
} from '../styles/tokens';

let passed = 0;
let failed = 0;

async function test(name: string, fn: () => void | Promise<void>): Promise<void> {
  try {
    await fn();
    console.log(`  \u2713 ${name}`);
    passed++;
  } catch (e) {
    console.log(`  \u2717 ${name}`);
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

function assertTrue(actual: boolean, message?: string): void {
  if (!actual) {
    throw new Error(`${message || 'Expected true but got false'}`);
  }
}

// Helper to validate hex color format
function isValidHexColor(value: string): boolean {
  return /^#[0-9A-Fa-f]{3}([0-9A-Fa-f]{3})?$/.test(value);
}

// Helper to validate rgba format
function isValidRgba(value: string): boolean {
  return /^rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*(,\s*[\d.]+\s*)?\)$/.test(value);
}

// Helper to validate CSS color (hex, rgb, rgba)
function isValidCssColor(value: string): boolean {
  return isValidHexColor(value) || isValidRgba(value);
}

// Helper to validate CSS pixel unit
function isValidPixelUnit(value: string): boolean {
  return /^\d+(\.\d+)?px$/.test(value);
}

// Helper to validate CSS time unit
function isValidTimeUnit(value: string): boolean {
  return /^\d+(\.\d+)?(ms|s)$/.test(value);
}

async function run(): Promise<void> {
  console.log('\nDesign Tokens Tests\n');

  // === colors.bg tests ===
  console.log('  colors.bg:');

  await test('colors.bg exports all background color keys', () => {
    const expectedKeys = ['primary', 'secondary', 'tertiary', 'surface', 'surfaceHover',
      'overlay', 'emptyState', 'slider', 'card', 'elevated', 'dropdown'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.bg, `colors.bg.${key} should exist`);
    }
  });

  await test('colors.bg all values are valid CSS colors', () => {
    for (const [key, value] of Object.entries(colors.bg)) {
      assertTrue(isValidCssColor(value), `colors.bg.${key} should be valid CSS color: ${value}`);
    }
  });

  // === colors.border tests ===
  console.log('  colors.border:');

  await test('colors.border exports all border color keys', () => {
    const expectedKeys = ['primary', 'secondary', 'hover', 'muted', 'dashed', 'separator'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.border, `colors.border.${key} should exist`);
    }
  });

  await test('colors.border all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.border)) {
      assertTrue(isValidHexColor(value), `colors.border.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.text tests ===
  console.log('  colors.text:');

  await test('colors.text exports all text color keys', () => {
    const expectedKeys = ['primary', 'secondary', 'tertiary', 'muted', 'faint', 'disabled',
      'light', 'lighter', 'white', 'black', 'link', 'code'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.text, `colors.text.${key} should exist`);
    }
  });

  await test('colors.text all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.text)) {
      assertTrue(isValidHexColor(value), `colors.text.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.status tests ===
  console.log('  colors.status:');

  await test('colors.status exports all status color keys', () => {
    const expectedKeys = ['success', 'successDark', 'successLime', 'emerald', 'error',
      'errorDark', 'warning', 'warningDark', 'info', 'infoHover', 'neutral', 'neutralDark'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.status, `colors.status.${key} should exist`);
    }
  });

  await test('colors.status all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.status)) {
      assertTrue(isValidHexColor(value), `colors.status.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.statusBg tests ===
  console.log('  colors.statusBg:');

  await test('colors.statusBg exports all status background color keys', () => {
    const expectedKeys = ['success', 'successMaterial', 'emerald', 'emeraldBorder', 'error',
      'errorStrong', 'errorSolid', 'errorSolidStrong', 'errorBorder', 'errorBorderSubtle',
      'errorMaterial', 'warningBorder', 'warning', 'warningAmber', 'warningMaterial',
      'info', 'infoLight', 'infoBorder', 'neutral', 'neutralBorder', 'purple'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.statusBg, `colors.statusBg.${key} should exist`);
    }
  });

  await test('colors.statusBg all values are valid CSS colors', () => {
    for (const [key, value] of Object.entries(colors.statusBg)) {
      assertTrue(isValidCssColor(value), `colors.statusBg.${key} should be valid CSS color: ${value}`);
    }
  });

  // === colors.alpha tests ===
  console.log('  colors.alpha:');

  await test('colors.alpha exports all alpha transparency keys', () => {
    const expectedKeys = ['white05', 'white08', 'white10', 'white15',
      'black10', 'black15', 'black20', 'gray08', 'gray25'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.alpha, `colors.alpha.${key} should exist`);
    }
  });

  await test('colors.alpha all values are valid rgba colors', () => {
    for (const [key, value] of Object.entries(colors.alpha)) {
      assertTrue(isValidRgba(value), `colors.alpha.${key} should be valid rgba color: ${value}`);
    }
  });

  // === colors.accent tests ===
  console.log('  colors.accent:');

  await test('colors.accent exports all accent color keys', () => {
    const expectedKeys = ['cyan', 'purple', 'amber', 'lightRed', 'mediumRed'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.accent, `colors.accent.${key} should exist`);
    }
  });

  await test('colors.accent all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.accent)) {
      assertTrue(isValidHexColor(value), `colors.accent.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.graph tests ===
  console.log('  colors.graph:');

  await test('colors.graph exports all graph color keys', () => {
    const expectedKeys = ['pending', 'active', 'completed', 'error',
      'pendingStroke', 'activeStroke', 'completedStroke', 'errorStroke',
      'conditional', 'parallel'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.graph, `colors.graph.${key} should exist`);
    }
  });

  await test('colors.graph all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.graph)) {
      assertTrue(isValidHexColor(value), `colors.graph.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.chart tests ===
  console.log('  colors.chart:');

  await test('colors.chart exports all chart color keys', () => {
    const expectedKeys = ['purple', 'green', 'yellow', 'orange', 'teal'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.chart, `colors.chart.${key} should exist`);
    }
  });

  await test('colors.chart all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.chart)) {
      assertTrue(isValidHexColor(value), `colors.chart.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.connection tests ===
  console.log('  colors.connection:');

  await test('colors.connection exports all connection color keys', () => {
    const expectedKeys = ['healthy', 'degraded', 'reconnecting', 'waiting', 'unavailable'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.connection, `colors.connection.${key} should exist`);
    }
  });

  await test('colors.connection all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.connection)) {
      assertTrue(isValidHexColor(value), `colors.connection.${key} should be valid hex color: ${value}`);
    }
  });

  // === colors.ui tests ===
  console.log('  colors.ui:');

  await test('colors.ui exports all UI color keys', () => {
    const expectedKeys = ['tabActive', 'bannerWarning', 'bannerWarningDark', 'bannerError'];
    for (const key of expectedKeys) {
      assertTrue(key in colors.ui, `colors.ui.${key} should exist`);
    }
  });

  await test('colors.ui all values are valid hex colors', () => {
    for (const [key, value] of Object.entries(colors.ui)) {
      assertTrue(isValidHexColor(value), `colors.ui.${key} should be valid hex color: ${value}`);
    }
  });

  // === spacing tests ===
  console.log('  spacing:');

  await test('spacing exports all spacing keys', () => {
    const expectedKeys = ['0', '1', '2', '3', '4', '5', '6', '8', '10', '12'];
    for (const key of expectedKeys) {
      assertTrue(key in spacing, `spacing.${key} should exist`);
    }
  });

  await test('spacing all values are valid pixel units', () => {
    for (const [key, value] of Object.entries(spacing)) {
      assertTrue(isValidPixelUnit(value), `spacing.${key} should be valid pixel unit: ${value}`);
    }
  });

  await test('spacing follows 4px base unit', () => {
    assertEqual(spacing['0'], '0px');
    assertEqual(spacing['1'], '4px');
    assertEqual(spacing['2'], '8px');
    assertEqual(spacing['3'], '12px');
    assertEqual(spacing['4'], '16px');
    assertEqual(spacing['5'], '20px');
    assertEqual(spacing['6'], '24px');
    assertEqual(spacing['8'], '32px');
    assertEqual(spacing['10'], '40px');
    assertEqual(spacing['12'], '48px');
  });

  // === fontSize tests ===
  console.log('  fontSize:');

  await test('fontSize exports all fontSize keys', () => {
    const expectedKeys = ['xs', 'sm', 'base', 'md', 'lg', 'xl'];
    for (const key of expectedKeys) {
      assertTrue(key in fontSize, `fontSize.${key} should exist`);
    }
  });

  await test('fontSize all values are valid pixel units', () => {
    for (const [key, value] of Object.entries(fontSize)) {
      assertTrue(isValidPixelUnit(value), `fontSize.${key} should be valid pixel unit: ${value}`);
    }
  });

  await test('fontSize values are in ascending order', () => {
    const xsValue = parseInt(fontSize.xs, 10);
    const smValue = parseInt(fontSize.sm, 10);
    const baseValue = parseInt(fontSize.base, 10);
    const mdValue = parseInt(fontSize.md, 10);
    const lgValue = parseInt(fontSize.lg, 10);
    const xlValue = parseInt(fontSize.xl, 10);

    assertTrue(xsValue < smValue, `xs (${xsValue}) should be < sm (${smValue})`);
    assertTrue(smValue < baseValue, `sm (${smValue}) should be < base (${baseValue})`);
    assertTrue(baseValue < mdValue, `base (${baseValue}) should be < md (${mdValue})`);
    assertTrue(mdValue < lgValue, `md (${mdValue}) should be < lg (${lgValue})`);
    assertTrue(lgValue < xlValue, `lg (${lgValue}) should be < xl (${xlValue})`);
  });

  // === borderRadius tests ===
  console.log('  borderRadius:');

  await test('borderRadius exports all borderRadius keys', () => {
    const expectedKeys = ['sm', 'md', 'lg', 'full'];
    for (const key of expectedKeys) {
      assertTrue(key in borderRadius, `borderRadius.${key} should exist`);
    }
  });

  await test('borderRadius all values are valid pixel units', () => {
    for (const [key, value] of Object.entries(borderRadius)) {
      assertTrue(isValidPixelUnit(value), `borderRadius.${key} should be valid pixel unit: ${value}`);
    }
  });

  await test('borderRadius.full is large enough for pill shapes', () => {
    const fullValue = parseInt(borderRadius.full, 10);
    assertTrue(fullValue > 1000, `borderRadius.full (${fullValue}) should be > 1000 for pill shapes`);
  });

  // === shadows tests ===
  console.log('  shadows:');

  await test('shadows exports all shadow keys', () => {
    const expectedKeys = ['focus', 'focusLarge', 'thumbGlow', 'error', 'dropdown'];
    for (const key of expectedKeys) {
      assertTrue(key in shadows, `shadows.${key} should exist`);
    }
  });

  await test('shadows all values are non-empty strings', () => {
    for (const [key, value] of Object.entries(shadows)) {
      assertTrue(typeof value === 'string', `shadows.${key} should be string`);
      assertTrue(value.length > 0, `shadows.${key} should be non-empty`);
    }
  });

  await test('shadows.focus uses rgba format', () => {
    assertTrue(shadows.focus.includes('rgba'), `shadows.focus should contain rgba: ${shadows.focus}`);
  });

  await test('shadows.error uses inset', () => {
    assertTrue(shadows.error.includes('inset'), `shadows.error should contain inset: ${shadows.error}`);
  });

  // === durations tests ===
  console.log('  durations:');

  await test('durations exports all duration keys', () => {
    const expectedKeys = ['fast', 'normal', 'slow'];
    for (const key of expectedKeys) {
      assertTrue(key in durations, `durations.${key} should exist`);
    }
  });

  await test('durations all values are valid time units', () => {
    for (const [key, value] of Object.entries(durations)) {
      assertTrue(isValidTimeUnit(value), `durations.${key} should be valid time unit: ${value}`);
    }
  });

  await test('durations values are in ascending order', () => {
    const fastValue = parseInt(durations.fast, 10);
    const normalValue = parseInt(durations.normal, 10);
    const slowValue = parseInt(durations.slow, 10);

    assertTrue(fastValue < normalValue, `fast (${fastValue}) should be < normal (${normalValue})`);
    assertTrue(normalValue < slowValue, `normal (${normalValue}) should be < slow (${slowValue})`);
  });

  // === colors structure tests ===
  console.log('  colors structure:');

  await test('colors has expected number of categories', () => {
    const expectedCategories = ['bg', 'border', 'text', 'status', 'statusBg',
      'alpha', 'accent', 'graph', 'chart', 'connection', 'ui'];
    const actualKeys = Object.keys(colors).sort();
    const expectedKeys = expectedCategories.sort();
    assertEqual(actualKeys, expectedKeys);
  });

  // === Dark theme consistency tests ===
  console.log('  dark theme consistency:');

  await test('primary background is lighter than secondary (visual hierarchy)', () => {
    // In hex, higher values = lighter color
    // #1a1a2e vs #151525
    const primaryValue = parseInt(colors.bg.primary.slice(1, 3), 16);
    const secondaryValue = parseInt(colors.bg.secondary.slice(1, 3), 16);
    assertTrue(primaryValue > secondaryValue,
      `bg.primary (${colors.bg.primary}) should be lighter than bg.secondary (${colors.bg.secondary})`);
  });

  await test('text.primary is lighter than text.secondary', () => {
    const primaryValue = parseInt(colors.text.primary.slice(1, 3), 16);
    const secondaryValue = parseInt(colors.text.secondary.slice(1, 3), 16);
    assertTrue(primaryValue > secondaryValue,
      `text.primary (${colors.text.primary}) should be lighter than text.secondary (${colors.text.secondary})`);
  });

  await test('status colors follow semantic conventions', () => {
    assertEqual(colors.status.success, '#22c55e', 'success should be green');
    assertEqual(colors.status.error, '#ef4444', 'error should be red');
    assertEqual(colors.status.warning, '#f59e0b', 'warning should be amber');
    assertEqual(colors.status.info, '#3b82f6', 'info should be blue');
  });

  // Print summary
  console.log(`\n  ${passed} passed, ${failed} failed\n`);

  if (failed > 0) {
    process.exit(1);
  }
}

run().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
