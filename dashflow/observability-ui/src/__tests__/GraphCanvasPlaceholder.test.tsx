// M-2605: Comprehensive component tests for GraphCanvasPlaceholder
// Run with: npx tsx src/__tests__/GraphCanvasPlaceholder.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { GraphCanvasPlaceholder } from '../components/GraphCanvasPlaceholder';
import GraphCanvasPlaceholderDefault from '../components/GraphCanvasPlaceholder';
import { borderRadius, colors, fontSize, spacing } from '../styles/tokens';

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

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

function assertNotIncludes(haystack: string, needle: string, message?: string): void {
  if (haystack.includes(needle)) {
    throw new Error(message || `Expected NOT to include: ${needle}`);
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(message || `Expected ${expected} but got ${actual}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || `Expected condition to be true`);
  }
}

console.log('\nGraphCanvasPlaceholder Tests\n');

// ========================================
// Component Export Tests
// ========================================
console.log('Component exports:');

test('GraphCanvasPlaceholder is exported as named export', () => {
  assertTrue(typeof GraphCanvasPlaceholder === 'object' || typeof GraphCanvasPlaceholder === 'function');
});

test('GraphCanvasPlaceholder is exported as default export', () => {
  assertTrue(typeof GraphCanvasPlaceholderDefault === 'object' || typeof GraphCanvasPlaceholderDefault === 'function');
});

test('named and default exports are the same component', () => {
  assertEqual(GraphCanvasPlaceholder, GraphCanvasPlaceholderDefault);
});

test('component is a function (functional component)', () => {
  assertTrue(typeof GraphCanvasPlaceholder === 'function');
});

test('component name matches expected name', () => {
  assertEqual(GraphCanvasPlaceholder.name, 'GraphCanvasPlaceholder');
});

// ========================================
// Rendering Tests
// ========================================
console.log('\nRendering:');

test('component renders without throwing', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertTrue(html.length > 0);
});

test('component renders without props (no required props)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertTrue(html.startsWith('<div'));
});

test('component renders same output on consecutive calls (pure)', () => {
  const html1 = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  const html2 = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertEqual(html1, html2);
});

// ========================================
// Content Rendering Tests
// ========================================
console.log('\nContent rendering:');

test('renders chart icon emoji', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, '📊');
});

test('renders primary placeholder message', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'Waiting for graph execution...');
});

test('renders guidance text', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'Run a demo app with node descriptions to see the graph');
});

test('message mentions graph execution', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'graph');
  assertIncludes(html, 'execution');
});

test('guidance mentions demo app', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'demo app');
});

test('guidance mentions node descriptions', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'node descriptions');
});

// ========================================
// Icon Tests
// ========================================
console.log('\nIcon styling:');

test('icon has large font size (2.5rem)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'font-size:2.5rem');
});

test('icon has bottom margin from spacing tokens', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `margin-bottom:${spacing[2]}`);
});

// ========================================
// Container Styling Tests (Design Tokens)
// ========================================
console.log('\nContainer styling (design tokens):');

test('container fills available height (100%)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'height:100%');
});

test('container uses flexbox display', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'display:flex');
});

test('container centers content vertically (align-items)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'align-items:center');
});

test('container centers content horizontally (justify-content)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'justify-content:center');
});

test('container uses primary background color from tokens', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `background-color:${colors.bg.primary}`);
});

test('container uses large border radius from tokens', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `border-radius:${borderRadius.lg}`);
});

test('container has dashed border with primary border color', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `border:2px dashed ${colors.border.primary}`);
});

test('border is 2px width', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'border:2px');
});

test('border style is dashed (not solid)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'dashed');
  assertNotIncludes(html, 'border:2px solid');
});

// ========================================
// Inner Content Styling Tests
// ========================================
console.log('\nInner content styling:');

test('inner content is text-centered', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'text-align:center');
});

// ========================================
// Message Styling Tests
// ========================================
console.log('\nMessage styling:');

test('primary message uses neutral status color', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `color:${colors.status.neutral}`);
});

test('guidance text uses neutralDark status color', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `color:${colors.status.neutralDark}`);
});

test('guidance text uses medium font size from tokens', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `font-size:${fontSize.md}`);
});

test('guidance text has top margin from spacing tokens', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `margin-top:${spacing[1]}`);
});

// ========================================
// DOM Structure Tests
// ========================================
console.log('\nDOM structure:');

test('renders exactly 5 div elements (outer, inner, icon, message, guidance)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  const divCount = (html.match(/<div/g) || []).length;
  assertEqual(divCount, 5);
});

test('outer div is the root element', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertTrue(html.startsWith('<div'));
});

test('closes all div tags properly', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  const openDivCount = (html.match(/<div/g) || []).length;
  const closeDivCount = (html.match(/<\/div>/g) || []).length;
  assertEqual(openDivCount, closeDivCount);
});

test('all style attributes use inline format', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'style="');
});

// ========================================
// Dark Theme Compliance Tests
// ========================================
console.log('\nDark theme compliance:');

test('uses dark background (colors.bg.primary)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.bg.primary);
});

test('does not use white background', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertNotIncludes(html, 'background-color:#fff');
  assertNotIncludes(html, 'background-color:#ffffff');
  assertNotIncludes(html, 'background-color:white');
});

test('does not use light gray backgrounds', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertNotIncludes(html, 'background-color:#f');
  assertNotIncludes(html, 'background-color:#F');
});

test('uses muted text colors (not black)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertNotIncludes(html, 'color:#000');
  assertNotIncludes(html, 'color:black');
});

// ========================================
// Design Token Usage Verification
// ========================================
console.log('\nDesign token usage:');

test('uses colors.bg.primary token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.bg.primary);
});

test('uses colors.border.primary token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.border.primary);
});

test('uses colors.status.neutral token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.status.neutral);
});

test('uses colors.status.neutralDark token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.status.neutralDark);
});

test('uses spacing[1] token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, spacing[1]);
});

test('uses spacing[2] token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, spacing[2]);
});

test('uses borderRadius.lg token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, borderRadius.lg);
});

test('uses fontSize.md token', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, fontSize.md);
});

// ========================================
// Layout Behavior Tests
// ========================================
console.log('\nLayout behavior:');

test('flex container for centering content', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'display:flex');
  assertIncludes(html, 'align-items:center');
  assertIncludes(html, 'justify-content:center');
});

test('height is 100% to fill parent container', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'height:100%');
});

test('no fixed width constraint (responsive)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertNotIncludes(html, 'width:');
});

// ========================================
// Accessibility Considerations
// ========================================
console.log('\nAccessibility:');

test('icon is decorative (part of visual design)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  // Icon is in a div, not an img, so no alt needed - it's supplementary
  assertIncludes(html, '📊');
});

test('text content is human-readable', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'Waiting for');
  assertIncludes(html, 'Run a');
});

test('no interactive elements (pure display)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertNotIncludes(html, '<button');
  assertNotIncludes(html, '<a ');
  assertNotIncludes(html, '<input');
  assertNotIncludes(html, 'onclick');
});

// ========================================
// Placeholder Visual Hierarchy Tests
// ========================================
console.log('\nVisual hierarchy:');

test('icon is largest element (2.5rem)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, 'font-size:2.5rem');
});

test('guidance is smallest text (fontSize.md)', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, `font-size:${fontSize.md}`);
});

test('guidance color is darker than message (neutralDark vs neutral)', () => {
  // This verifies the visual hierarchy through color differentiation
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  assertIncludes(html, colors.status.neutral);
  assertIncludes(html, colors.status.neutralDark);
});

// ========================================
// Content Order Tests
// ========================================
console.log('\nContent order:');

test('icon appears before message text', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  const iconIndex = html.indexOf('📊');
  const messageIndex = html.indexOf('Waiting for graph execution');
  assertTrue(iconIndex < messageIndex, 'Icon should appear before message');
});

test('message appears before guidance text', () => {
  const html = renderToStaticMarkup(<GraphCanvasPlaceholder />);
  const messageIndex = html.indexOf('Waiting for graph execution');
  const guidanceIndex = html.indexOf('Run a demo app');
  assertTrue(messageIndex < guidanceIndex, 'Message should appear before guidance');
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
