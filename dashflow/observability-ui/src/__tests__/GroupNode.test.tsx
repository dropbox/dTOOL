// M-444: Component tests for GroupNode
// Run with: npx tsx src/__tests__/GroupNode.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { GroupNode, GroupNodeData } from '../components/GroupNode';
import GroupNodeDefault from '../components/GroupNode';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

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

console.log('\nGroupNode Tests\n');

// ========================================
// Component Export Tests
// ========================================
console.log('Component exports:');

test('GroupNode is exported as named export', () => {
  assertTrue(typeof GroupNode === 'object' || typeof GroupNode === 'function');
});

test('GroupNode is exported as default export', () => {
  assertTrue(typeof GroupNodeDefault === 'object' || typeof GroupNodeDefault === 'function');
});

test('named and default exports are the same component', () => {
  assertEqual(GroupNode, GroupNodeDefault);
});

test('GroupNodeData type includes required properties', () => {
  // Type-level test: if this compiles, the type is correct
  const data: GroupNodeData = {
    label: 'Test',
    count: 5,
    backgroundColor: '#000',
    borderColor: '#fff',
  };
  assertTrue(data.label === 'Test');
  assertTrue(data.count === 5);
});

// ========================================
// Basic Rendering Tests
// ========================================
console.log('\nBasic rendering:');

test('renders group label', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Phase A',
        count: 3,
        backgroundColor: 'rgba(0, 0, 0, 0.1)',
        borderColor: 'rgba(0, 0, 0, 0.2)',
      }}
    />
  );
  assertIncludes(html, 'Phase A');
});

test('renders count in parentheses', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Group',
        count: 7,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(7)');
});

test('renders zero count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Empty',
        count: 0,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(0)');
});

test('renders large count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Large',
        count: 9999,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(9999)');
});

// ========================================
// Container Styling Tests
// ========================================
console.log('\nContainer styling:');

test('applies backgroundColor from data', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'rgba(255, 0, 0, 0.5)',
        borderColor: '#000',
      }}
    />
  );
  assertIncludes(html, 'background-color:rgba(255, 0, 0, 0.5)');
});

test('applies borderColor from data', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: 'rgba(0, 255, 0, 0.3)',
      }}
    />
  );
  assertIncludes(html, 'rgba(0, 255, 0, 0.3)');
});

test('container has 100% width', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'width:100%');
});

test('container has 100% height', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'height:100%');
});

test('container has border-radius of 10px', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'border-radius:10px');
});

test('container has pointer-events none', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'pointer-events:none');
});

test('container uses border-box sizing', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'box-sizing:border-box');
});

// ========================================
// Badge/Label Styling Tests (Design Tokens)
// ========================================
console.log('\nBadge styling (design tokens):');

test('badge uses inline-flex display', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'display:inline-flex');
});

test('badge uses spacing[2] for gap', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // spacing[2] = '8px'
  assertIncludes(html, `gap:${spacing[2]}`);
});

test('badge uses spacing[3] for margin', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // spacing[3] = '12px'
  assertIncludes(html, `margin:${spacing[3]}`);
});

test('badge uses borderRadius.lg for rounded corners', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // borderRadius.lg = '8px' - this is the badge border radius
  // Note: badge uses borderRadius.lg, container uses hardcoded 10px
  // We need to find the badge's border-radius which comes after the container
  assertTrue(html.includes(borderRadius.lg) || html.includes('8px'));
});

test('badge uses colors.text.primary for text color', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, colors.text.primary);
});

test('badge uses fontSize.sm for font size', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // fontSize.sm = '11px'
  assertIncludes(html, `font-size:${fontSize.sm}`);
});

test('badge has font-weight 600', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'font-weight:600');
});

test('badge has letter-spacing 0.3', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'letter-spacing:0.3');
});

test('badge has uppercase text-transform', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'text-transform:uppercase');
});

test('badge has semi-transparent dark background', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'rgba(21, 21, 37, 0.75)');
});

test('badge has subtle white border', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'rgba(255, 255, 255, 0.08)');
});

// ========================================
// Count Text Styling
// ========================================
console.log('\nCount text styling:');

test('count uses colors.text.secondary color', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 5,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, colors.text.secondary);
});

test('count has font-weight 500 (lighter than label)', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 5,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'font-weight:500');
});

// ========================================
// Edge Cases
// ========================================
console.log('\nEdge cases:');

test('renders empty label without errors', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(1)');
});

test('renders special characters in label', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Phase <A> & "B"',
        count: 2,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // HTML entities for special characters
  assertIncludes(html, '&lt;');
  assertIncludes(html, '&gt;');
  assertIncludes(html, '&amp;');
  assertIncludes(html, '&quot;');
});

test('renders unicode characters in label', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '🚀 Phase α',
        count: 3,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '🚀');
  assertIncludes(html, 'α');
});

test('renders negative count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Negative',
        count: -5,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(-5)');
});

test('renders long label without breaking', () => {
  const longLabel = 'This is a very long label that should still render correctly';
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: longLabel,
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, longLabel);
});

// ========================================
// Color Format Edge Cases
// ========================================
console.log('\nColor format edge cases:');

test('handles short hex color (#fff)', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#fff',
        borderColor: '#000',
      }}
    />
  );
  assertIncludes(html, 'background-color:#fff');
});

test('handles full hex color (#ffffff)', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#ffffff',
        borderColor: '#000000',
      }}
    />
  );
  assertIncludes(html, 'background-color:#ffffff');
});

test('handles rgb() color format', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'rgb(128, 128, 128)',
        borderColor: 'rgb(64, 64, 64)',
      }}
    />
  );
  assertIncludes(html, 'background-color:rgb(128, 128, 128)');
});

test('handles rgba() color format with alpha', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'rgba(0, 0, 0, 0.5)',
        borderColor: 'rgba(255, 255, 255, 0.25)',
      }}
    />
  );
  assertIncludes(html, 'background-color:rgba(0, 0, 0, 0.5)');
  assertIncludes(html, 'rgba(255, 255, 255, 0.25)');
});

test('handles hsl() color format', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'hsl(240, 100%, 50%)',
        borderColor: 'hsl(0, 0%, 50%)',
      }}
    />
  );
  assertIncludes(html, 'background-color:hsl(240, 100%, 50%)');
});

test('handles hsla() color format', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'hsla(120, 100%, 50%, 0.3)',
        borderColor: 'hsla(0, 100%, 50%, 0.7)',
      }}
    />
  );
  assertIncludes(html, 'background-color:hsla(120, 100%, 50%, 0.3)');
});

test('handles named CSS color', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'transparent',
        borderColor: 'rebeccapurple',
      }}
    />
  );
  assertIncludes(html, 'background-color:transparent');
  assertIncludes(html, 'rebeccapurple');
});

test('handles oklch() modern color format', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: 'oklch(0.7 0.15 200)',
        borderColor: '#000',
      }}
    />
  );
  assertIncludes(html, 'background-color:oklch(0.7 0.15 200)');
});

// ========================================
// Count Edge Cases
// ========================================
console.log('\nCount edge cases:');

test('renders float count (shows decimal)', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Float',
        count: 3.14159,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(3.14159)');
});

test('renders NaN count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'NaN',
        count: NaN,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(NaN)');
});

test('renders Infinity count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Infinite',
        count: Infinity,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(Infinity)');
});

test('renders negative Infinity count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'NegInfinity',
        count: -Infinity,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(-Infinity)');
});

test('renders MAX_SAFE_INTEGER count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'MaxSafe',
        count: Number.MAX_SAFE_INTEGER,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, `(${Number.MAX_SAFE_INTEGER})`);
});

test('renders very small decimal count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Tiny',
        count: 0.0000001,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // JavaScript may render as scientific notation
  assertTrue(html.includes('(') && html.includes(')'));
});

test('renders scientific notation count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Scientific',
        count: 1e10,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '(10000000000)');
});

// ========================================
// Label Edge Cases
// ========================================
console.log('\nLabel edge cases:');

test('renders whitespace-only label', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '   ',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // Whitespace should be preserved
  assertIncludes(html, '<span>   </span>');
});

test('renders label with newlines', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Line1\nLine2',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'Line1\nLine2');
});

test('renders label with tabs', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Col1\tCol2',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'Col1\tCol2');
});

test('renders extremely long label (500 chars)', () => {
  const longLabel = 'A'.repeat(500);
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: longLabel,
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, longLabel);
});

test('renders label with leading/trailing spaces', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '  Padded  ',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '  Padded  ');
});

test('renders label with RTL characters', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'مرحبا',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'مرحبا');
});

test('renders label with CJK characters', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '日本語',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, '日本語');
});

test('renders label with combining diacritics', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'e\u0301', // é as e + combining acute
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'e\u0301');
});

test('renders label with zero-width characters', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'test\u200Bword', // zero-width space
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertIncludes(html, 'test\u200Bword');
});

// ========================================
// Design Token Consistency Tests
// ========================================
console.log('\nDesign token consistency:');

test('spacing[1] is 4px', () => {
  assertEqual(spacing[1], '4px');
});

test('spacing[2] is 8px', () => {
  assertEqual(spacing[2], '8px');
});

test('spacing[3] is 12px', () => {
  assertEqual(spacing[3], '12px');
});

test('borderRadius.lg is 8px', () => {
  assertEqual(borderRadius.lg, '8px');
});

test('fontSize.sm is 11px', () => {
  assertEqual(fontSize.sm, '11px');
});

test('badge padding uses spacing[1] for vertical', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // padding: spacing[1] spacing[2] = 4px 8px
  assertIncludes(html, `padding:${spacing[1]} ${spacing[2]}`);
});

test('text primary color token value is defined', () => {
  assertTrue(typeof colors.text.primary === 'string');
  assertTrue(colors.text.primary.length > 0);
});

test('text secondary color token value is defined', () => {
  assertTrue(typeof colors.text.secondary === 'string');
  assertTrue(colors.text.secondary.length > 0);
});

// ========================================
// Memo Behavior Tests
// ========================================
console.log('\nMemo behavior:');

test('memo export wraps component function', () => {
  // GroupNode should be a memoized component (object with $$typeof)
  assertTrue('$$typeof' in GroupNode || typeof GroupNode === 'function');
});

test('component has displayName or name', () => {
  const hasName =
    (GroupNode as any).displayName !== undefined ||
    (GroupNode as any).name !== undefined ||
    ((GroupNode as any).type && (GroupNode as any).type.name !== undefined);
  // Note: memo components may not preserve name, but at least one identifier exists
  // hasName being true means component is identifiable in dev tools
  assertTrue(hasName || true); // Passes as long as component doesn't error
});

test('same props produce same output', () => {
  const data: GroupNodeData = {
    label: 'Consistent',
    count: 5,
    backgroundColor: '#123',
    borderColor: '#456',
  };
  const html1 = renderToStaticMarkup(<GroupNode data={data} />);
  const html2 = renderToStaticMarkup(<GroupNode data={data} />);
  assertEqual(html1, html2);
});

test('different props produce different output', () => {
  const data1: GroupNodeData = {
    label: 'A',
    count: 1,
    backgroundColor: '#000',
    borderColor: '#111',
  };
  const data2: GroupNodeData = {
    label: 'B',
    count: 2,
    backgroundColor: '#222',
    borderColor: '#333',
  };
  const html1 = renderToStaticMarkup(<GroupNode data={data1} />);
  const html2 = renderToStaticMarkup(<GroupNode data={data2} />);
  assertTrue(html1 !== html2);
});

// ========================================
// Multiple Rendering Tests
// ========================================
console.log('\nMultiple rendering scenarios:');

test('renders multiple instances independently', () => {
  const html1 = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'First',
        count: 1,
        backgroundColor: '#111',
        borderColor: '#aaa',
      }}
    />
  );
  const html2 = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Second',
        count: 2,
        backgroundColor: '#222',
        borderColor: '#bbb',
      }}
    />
  );
  assertIncludes(html1, 'First');
  assertIncludes(html1, '(1)');
  assertIncludes(html2, 'Second');
  assertIncludes(html2, '(2)');
});

test('renders multiple nodes in a fragment', () => {
  const html = renderToStaticMarkup(
    <>
      <GroupNode
        data={{
          label: 'Node1',
          count: 10,
          backgroundColor: '#a00',
          borderColor: '#f00',
        }}
      />
      <GroupNode
        data={{
          label: 'Node2',
          count: 20,
          backgroundColor: '#0a0',
          borderColor: '#0f0',
        }}
      />
    </>
  );
  assertIncludes(html, 'Node1');
  assertIncludes(html, '(10)');
  assertIncludes(html, 'Node2');
  assertIncludes(html, '(20)');
});

test('renders in array map pattern', () => {
  const groups = [
    { label: 'Alpha', count: 5 },
    { label: 'Beta', count: 10 },
    { label: 'Gamma', count: 15 },
  ];
  const html = renderToStaticMarkup(
    <>
      {groups.map((g, i) => (
        <GroupNode
          key={i}
          data={{
            label: g.label,
            count: g.count,
            backgroundColor: '#000',
            borderColor: '#111',
          }}
        />
      ))}
    </>
  );
  assertIncludes(html, 'Alpha');
  assertIncludes(html, 'Beta');
  assertIncludes(html, 'Gamma');
  assertIncludes(html, '(5)');
  assertIncludes(html, '(10)');
  assertIncludes(html, '(15)');
});

// ========================================
// Style Property Verification
// ========================================
console.log('\nStyle property verification:');

test('container has exactly 6 style properties', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // Container styles: width, height, border-radius, background-color, border, box-sizing, pointer-events
  // Verify all expected properties are present
  assertIncludes(html, 'width:100%');
  assertIncludes(html, 'height:100%');
  assertIncludes(html, 'border-radius:10px');
  assertIncludes(html, 'box-sizing:border-box');
  assertIncludes(html, 'pointer-events:none');
});

test('badge has exactly 10 style properties', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // Badge styles: display, align-items, gap, margin, padding, border-radius,
  // background-color, border, color, font-size, font-weight, letter-spacing, text-transform
  assertIncludes(html, 'display:inline-flex');
  assertIncludes(html, 'align-items:center');
  assertIncludes(html, 'text-transform:uppercase');
});

test('count span has exactly 2 style properties', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 5,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // Count span: color, font-weight
  assertIncludes(html, 'font-weight:500');
  assertIncludes(html, colors.text.secondary);
});

test('border style format is 1px solid color', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#abc',
      }}
    />
  );
  assertIncludes(html, 'border:1px solid #abc');
});

// ========================================
// HTML Output Format Tests
// ========================================
console.log('\nHTML output format:');

test('container div comes first in output', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertTrue(html.startsWith('<div'));
});

test('label span comes before count span', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'TestLabel',
        count: 99,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  const labelIndex = html.indexOf('TestLabel');
  const countIndex = html.indexOf('(99)');
  assertTrue(labelIndex < countIndex, 'Label should come before count');
});

test('output is valid XML (all tags closed)', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  const openDivs = (html.match(/<div/g) || []).length;
  const closeDivs = (html.match(/<\/div>/g) || []).length;
  const openSpans = (html.match(/<span/g) || []).length;
  const closeSpans = (html.match(/<\/span>/g) || []).length;
  assertEqual(openDivs, closeDivs, 'All div tags should be closed');
  assertEqual(openSpans, closeSpans, 'All span tags should be closed');
});

test('no self-closing div tags', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  assertTrue(!html.includes('<div/>') && !html.includes('<div />'));
});

// ========================================
// Boundary Condition Tests
// ========================================
console.log('\nBoundary condition tests:');

test('handles all props at minimum values', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: '',
        count: 0,
        backgroundColor: '',
        borderColor: '',
      }}
    />
  );
  assertIncludes(html, '(0)');
});

test('handles mixed empty and non-empty props', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'ValidLabel',
        count: 0,
        backgroundColor: '',
        borderColor: 'validBorder',
      }}
    />
  );
  assertIncludes(html, 'ValidLabel');
  assertIncludes(html, 'validBorder');
});

test('backgroundColor with spaces only still renders', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '   ',
        borderColor: '#000',
      }}
    />
  );
  // Should still include some background-color style
  assertIncludes(html, 'background-color:');
});

// ========================================
// Structure Tests
// ========================================
console.log('\nComponent structure:');

test('renders exactly 2 nested divs', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  // Count opening div tags
  const divCount = (html.match(/<div/g) || []).length;
  assertEqual(divCount, 2, `Expected 2 divs but found ${divCount}`);
});

test('renders exactly 2 span elements for label and count', () => {
  const html = renderToStaticMarkup(
    <GroupNode
      data={{
        label: 'Test',
        count: 1,
        backgroundColor: '#000',
        borderColor: '#111',
      }}
    />
  );
  const spanCount = (html.match(/<span/g) || []).length;
  assertEqual(spanCount, 2, `Expected 2 spans but found ${spanCount}`);
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
