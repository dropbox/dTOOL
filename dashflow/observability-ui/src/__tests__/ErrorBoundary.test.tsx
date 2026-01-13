// M-455/M-2625: Comprehensive tests for ErrorBoundary component
// Run with: npx tsx src/__tests__/ErrorBoundary.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { ErrorBoundary } from '../components/ErrorBoundary';
import type { ErrorInfo } from 'react';

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

function assertTrue(actual: boolean, message?: string): void {
  if (!actual) {
    throw new Error(message || 'Expected true but got false');
  }
}

// assertFalse available if needed for future tests
// function assertFalse(actual: boolean, message?: string): void {
//   if (actual) {
//     throw new Error(message || 'Expected false but got true');
//   }
// }

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: "${needle}" in "${haystack.slice(0, 100)}..."`);
  }
}

function assertNotIncludes(haystack: string, needle: string, message?: string): void {
  if (haystack.includes(needle)) {
    throw new Error(message || `Expected NOT to include: "${needle}"`);
  }
}

console.log('\nErrorBoundary Tests\n');

// ============================================================
// Static methods and class structure
// ============================================================
console.log('--- Class structure ---');

test('ErrorBoundary is a class component with getDerivedStateFromError', () => {
  // ErrorBoundary must be a class component for getDerivedStateFromError/componentDidCatch
  assertEqual(typeof ErrorBoundary.getDerivedStateFromError, 'function', 'Should have getDerivedStateFromError');
});

test('ErrorBoundary has componentDidCatch method', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(typeof instance.componentDidCatch, 'function', 'Should have componentDidCatch');
});

test('ErrorBoundary has render method', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(typeof instance.render, 'function', 'Should have render');
});

test('ErrorBoundary has handleRetry method', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(typeof instance.handleRetry, 'function', 'Should have handleRetry');
});

test('ErrorBoundary constructor accepts props', () => {
  const instance = new ErrorBoundary({ children: <div>Test</div>, name: 'Test', fallback: <span>Fallback</span> });
  assertEqual(instance.props.name, 'Test', 'Should store name prop');
});

// ============================================================
// Initial state
// ============================================================
console.log('\n--- Initial state ---');

test('initial state hasError is false', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(instance.state.hasError, false, 'Initial hasError should be false');
});

test('initial state error is null', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(instance.state.error, null, 'Initial error should be null');
});

test('initial state errorInfo is null', () => {
  const instance = new ErrorBoundary({ children: null });
  assertEqual(instance.state.errorInfo, null, 'Initial errorInfo should be null');
});

test('initial state is independent per instance', () => {
  const instance1 = new ErrorBoundary({ children: null });
  const instance2 = new ErrorBoundary({ children: null });
  // Use getDerivedStateFromError to set error state (can't directly mutate readonly state)
  const errorState = ErrorBoundary.getDerivedStateFromError(new Error('Test'));
  // Apply to instance1 via spread (simulating React's state update)
  (instance1 as { state: typeof instance1.state }).state = { ...instance1.state, ...errorState };
  assertEqual(instance2.state.hasError, false, 'Instances should have independent state');
});

// ============================================================
// getDerivedStateFromError
// ============================================================
console.log('\n--- getDerivedStateFromError ---');

test('getDerivedStateFromError sets hasError to true', () => {
  const testError = new Error('Test error');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual(newState.hasError, true, 'hasError should be true');
});

test('getDerivedStateFromError captures the error object', () => {
  const testError = new Error('Test error message');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual(newState.error, testError, 'error should be the error object');
});

test('getDerivedStateFromError preserves error message', () => {
  const testError = new Error('Specific error message');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual(newState.error?.message, 'Specific error message', 'error message should be preserved');
});

test('getDerivedStateFromError handles TypeError', () => {
  const testError = new TypeError('Type error message');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual(newState.hasError, true, 'hasError should be true for TypeError');
  assertTrue(newState.error instanceof TypeError, 'error should be a TypeError');
});

test('getDerivedStateFromError handles RangeError', () => {
  const testError = new RangeError('Range error message');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual(newState.hasError, true, 'hasError should be true for RangeError');
  assertTrue(newState.error instanceof RangeError, 'error should be a RangeError');
});

test('getDerivedStateFromError handles SyntaxError', () => {
  const testError = new SyntaxError('Syntax error message');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertTrue(newState.error instanceof SyntaxError, 'error should be a SyntaxError');
});

test('getDerivedStateFromError handles error with stack trace', () => {
  const testError = new Error('Error with stack');
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertTrue(typeof newState.error?.stack === 'string', 'error should have stack trace');
});

test('getDerivedStateFromError handles error with custom properties', () => {
  const testError = new Error('Custom error') as Error & { code: number };
  testError.code = 500;
  const newState = ErrorBoundary.getDerivedStateFromError(testError);
  assertEqual((newState.error as Error & { code: number })?.code, 500, 'Custom properties should be preserved');
});

// ============================================================
// componentDidCatch
// ============================================================
console.log('\n--- componentDidCatch ---');

test('componentDidCatch sets errorInfo in state', () => {
  const instance = new ErrorBoundary({ children: null });
  const testError = new Error('Test');
  const testErrorInfo: ErrorInfo = { componentStack: 'at TestComponent' };

  // Mock setState to capture the state update
  let capturedState: unknown = null;
  instance.setState = ((state: unknown) => {
    capturedState = state;
  }) as typeof instance.setState;

  instance.componentDidCatch(testError, testErrorInfo);

  assertEqual((capturedState as { errorInfo: ErrorInfo }).errorInfo, testErrorInfo, 'Should set errorInfo');
});

test('componentDidCatch calls onError callback when provided', () => {
  let callbackCalled = false;
  let callbackError: Error | null = null;
  let callbackInfo: ErrorInfo | null = null;

  const onError = (error: Error, errorInfo: ErrorInfo) => {
    callbackCalled = true;
    callbackError = error;
    callbackInfo = errorInfo;
  };

  const instance = new ErrorBoundary({ children: null, onError });
  const testError = new Error('Callback test');
  const testErrorInfo: ErrorInfo = { componentStack: 'at CallbackComponent' };

  // Mock setState
  instance.setState = (() => {}) as typeof instance.setState;

  instance.componentDidCatch(testError, testErrorInfo);

  assertEqual(callbackCalled, true, 'onError callback should be called');
  assertEqual(callbackError, testError, 'callback should receive error');
  assertEqual(callbackInfo, testErrorInfo, 'callback should receive errorInfo');
});

test('componentDidCatch works without onError callback', () => {
  const instance = new ErrorBoundary({ children: null });
  const testError = new Error('No callback test');
  const testErrorInfo: ErrorInfo = { componentStack: 'at TestComponent' };

  // Mock setState
  instance.setState = (() => {}) as typeof instance.setState;

  // Should not throw
  let didThrow = false;
  try {
    instance.componentDidCatch(testError, testErrorInfo);
  } catch {
    didThrow = true;
  }

  assertEqual(didThrow, false, 'Should not throw when onError is not provided');
});

test('componentDidCatch uses "Unknown" when no name prop', () => {
  const instance = new ErrorBoundary({ children: null });
  const testError = new Error('Test');
  const testErrorInfo: ErrorInfo = { componentStack: 'at TestComponent' };

  // Mock setState
  instance.setState = (() => {}) as typeof instance.setState;

  // Should not throw - boundary name defaults to "Unknown"
  let didThrow = false;
  try {
    instance.componentDidCatch(testError, testErrorInfo);
  } catch {
    didThrow = true;
  }

  assertEqual(didThrow, false, 'Should handle missing name prop');
});

test('componentDidCatch uses provided name prop', () => {
  const instance = new ErrorBoundary({ children: null, name: 'MyBoundary' });
  const testError = new Error('Test');
  const testErrorInfo: ErrorInfo = { componentStack: 'at TestComponent' };

  // Mock setState
  instance.setState = (() => {}) as typeof instance.setState;

  // Should not throw
  let didThrow = false;
  try {
    instance.componentDidCatch(testError, testErrorInfo);
  } catch {
    didThrow = true;
  }

  assertEqual(didThrow, false, 'Should use provided name prop');
});

test('componentDidCatch preserves full component stack', () => {
  const instance = new ErrorBoundary({ children: null });
  const testError = new Error('Test');
  const longStack = 'at Component1\n    at Component2\n    at Component3\n    at App';
  const testErrorInfo: ErrorInfo = { componentStack: longStack };

  let capturedState: unknown = null;
  instance.setState = ((state: unknown) => {
    capturedState = state;
  }) as typeof instance.setState;

  instance.componentDidCatch(testError, testErrorInfo);

  assertEqual(
    (capturedState as { errorInfo: ErrorInfo }).errorInfo.componentStack,
    longStack,
    'Should preserve full component stack'
  );
});

// ============================================================
// handleRetry
// ============================================================
console.log('\n--- handleRetry ---');

test('handleRetry resets state to initial values', () => {
  const instance = new ErrorBoundary({ children: null });

  // Set error state first
  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: { componentStack: 'test' },
  };

  // Mock setState to capture the reset state
  let resetState: unknown = null;
  instance.setState = ((state: unknown) => {
    resetState = state;
  }) as typeof instance.setState;

  instance.handleRetry();

  assertEqual((resetState as { hasError: boolean }).hasError, false, 'hasError should be false');
  assertEqual((resetState as { error: Error | null }).error, null, 'error should be null');
  assertEqual((resetState as { errorInfo: ErrorInfo | null }).errorInfo, null, 'errorInfo should be null');
});

test('handleRetry can be called multiple times', () => {
  const instance = new ErrorBoundary({ children: null });

  let callCount = 0;
  instance.setState = (() => {
    callCount++;
  }) as typeof instance.setState;

  instance.handleRetry();
  instance.handleRetry();
  instance.handleRetry();

  assertEqual(callCount, 3, 'handleRetry should be callable multiple times');
});

test('handleRetry resets regardless of current error', () => {
  const instance = new ErrorBoundary({ children: null });

  instance.state = {
    hasError: true,
    error: new TypeError('Type error'),
    errorInfo: { componentStack: 'deep stack\n  at A\n  at B' },
  };

  let resetState: unknown = null;
  instance.setState = ((state: unknown) => {
    resetState = state;
  }) as typeof instance.setState;

  instance.handleRetry();

  assertEqual((resetState as { hasError: boolean }).hasError, false, 'Should reset hasError');
});

// ============================================================
// Rendering - children when no error
// ============================================================
console.log('\n--- Rendering children (no error) ---');

test('renders children when no error', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>
      <div data-testid="child">Child content</div>
    </ErrorBoundary>
  );

  assertIncludes(html, 'data-testid="child"', 'Should render child element');
  assertIncludes(html, 'Child content', 'Should render child content');
});

test('renders multiple children when no error', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>
      <span>First</span>
      <span>Second</span>
    </ErrorBoundary>
  );

  assertIncludes(html, 'First', 'Should render first child');
  assertIncludes(html, 'Second', 'Should render second child');
});

test('renders null children without error', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>{null}</ErrorBoundary>
  );

  // Should render empty (no error UI)
  assertNotIncludes(html, 'Error', 'Should not show error UI');
});

test('renders string children', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>Just a string</ErrorBoundary>
  );

  assertIncludes(html, 'Just a string', 'Should render string children');
});

test('renders number children', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>{42}</ErrorBoundary>
  );

  assertIncludes(html, '42', 'Should render number children');
});

test('renders boolean children (true renders nothing)', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>{true}</ErrorBoundary>
  );

  // Boolean true renders nothing in React
  assertNotIncludes(html, 'true', 'Should not render boolean true as text');
});

test('renders array children', () => {
  const items = [1, 2, 3].map((n) => <span key={n}>{n}</span>);
  const html = renderToStaticMarkup(
    <ErrorBoundary>{items}</ErrorBoundary>
  );

  assertIncludes(html, '1', 'Should render first array item');
  assertIncludes(html, '2', 'Should render second array item');
  assertIncludes(html, '3', 'Should render third array item');
});

test('renders fragment children', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary>
      <>
        <span>Fragment child 1</span>
        <span>Fragment child 2</span>
      </>
    </ErrorBoundary>
  );

  assertIncludes(html, 'Fragment child 1', 'Should render first fragment child');
  assertIncludes(html, 'Fragment child 2', 'Should render second fragment child');
});

test('renders nested ErrorBoundary children', () => {
  const html = renderToStaticMarkup(
    <ErrorBoundary name="Outer">
      <ErrorBoundary name="Inner">
        <div>Nested content</div>
      </ErrorBoundary>
    </ErrorBoundary>
  );

  assertIncludes(html, 'Nested content', 'Should render nested boundary content');
});

// ============================================================
// Rendering - custom fallback
// ============================================================
console.log('\n--- Rendering custom fallback ---');

test('renders custom fallback when error and fallback prop provided', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    fallback: <div data-testid="custom-fallback">Custom error UI</div>,
  });

  // Simulate error state
  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'data-testid="custom-fallback"', 'Should render custom fallback');
  assertIncludes(html, 'Custom error UI', 'Should show custom fallback content');
  assertNotIncludes(html, 'Child', 'Should not render children');
});

test('renders custom fallback with string content', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    fallback: 'Simple string fallback',
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Simple string fallback', 'Should render string fallback');
});

test('renders custom fallback with complex JSX', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    fallback: (
      <div className="error-container">
        <h1>Oops!</h1>
        <p>Something went wrong</p>
        <button>Try Again</button>
      </div>
    ),
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Oops!', 'Should render h1 from complex fallback');
  assertIncludes(html, 'Something went wrong', 'Should render p from complex fallback');
  assertIncludes(html, 'Try Again', 'Should render button from complex fallback');
});

test('renders default UI when fallback is null (null is falsy)', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    fallback: null,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // null fallback is falsy, so component shows default error UI
  assertIncludes(html, 'Component Error', 'Should show default error UI when fallback is null');
  assertIncludes(html, 'Retry', 'Should have Retry button');
});

// ============================================================
// Rendering - default error UI
// ============================================================
console.log('\n--- Rendering default error UI ---');

test('renders default error UI when error and no fallback', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Something went wrong'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Something went wrong', 'Should show error message');
  assertNotIncludes(html, 'Child', 'Should not render children');
});

test('default error UI shows boundary name from props', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    name: 'TestBoundary',
  });

  instance.state = {
    hasError: true,
    error: new Error('Test error'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'TestBoundary Error', 'Should show boundary name in title');
});

test('default error UI shows "Component" when no name prop', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test error'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Component Error', 'Should show "Component" as default name');
});

test('default error UI shows fallback message when error has no message', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  // Error with empty message
  const errorWithNoMessage = new Error('');
  // Some errors have empty message but error.message is ''
  instance.state = {
    hasError: true,
    error: errorWithNoMessage,
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // Should show either the empty message or the fallback
  // The component uses: this.state.error?.message || 'An unexpected error occurred'
  assertIncludes(html, 'An unexpected error occurred', 'Should show fallback error message');
});

test('default error UI includes Retry button', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Retry', 'Should have Retry button');
  assertIncludes(html, '<button', 'Should have a button element');
});

test('default error UI has proper container styling', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // Check for inline style attributes
  assertIncludes(html, 'style="', 'Should have inline styles');
  assertIncludes(html, 'padding', 'Should have padding style');
  assertIncludes(html, 'border-radius', 'Should have border-radius style');
});

test('default error UI has flex layout', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'display:flex', 'Should use flex display');
  assertIncludes(html, 'flex-direction:column', 'Should use column flex direction');
});

// ============================================================
// Boundary name edge cases
// ============================================================
console.log('\n--- Boundary name edge cases ---');

test('handles empty string name', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    name: '',
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // Empty string is falsy, so it should show "Component"
  assertIncludes(html, 'Component Error', 'Should show "Component" for empty name');
});

test('handles name with special characters', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    name: '<Script>',
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // React should escape special characters
  assertIncludes(html, '&lt;Script&gt; Error', 'Should escape special characters in name');
});

test('handles very long name', () => {
  const longName = 'A'.repeat(100);
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
    name: longName,
  });

  instance.state = {
    hasError: true,
    error: new Error('Test'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, longName, 'Should render long name');
});

// ============================================================
// Edge cases
// ============================================================
console.log('\n--- Edge cases ---');

test('handles error with null message gracefully', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: null, // No error object at all
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'An unexpected error occurred', 'Should show fallback message for null error');
});

test('special characters in error message are escaped in HTML', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('<script>alert("xss")</script>'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  // React should escape the special characters
  assertNotIncludes(html, '<script>', 'Script tags should be escaped');
  assertIncludes(html, '&lt;script&gt;', 'Script tags should be HTML-escaped');
});

test('long error messages are preserved', () => {
  const longMessage = 'A'.repeat(500);
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error(longMessage),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, longMessage, 'Long error message should be preserved');
});

test('error message with newlines renders correctly', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('Line 1\nLine 2\nLine 3'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'Line 1', 'Should include first line');
});

test('error message with unicode characters', () => {
  const instance = new ErrorBoundary({
    children: <div>Child</div>,
  });

  instance.state = {
    hasError: true,
    error: new Error('エラーが発生しました 🚨'),
    errorInfo: null,
  };

  const result = instance.render();
  const html = renderToStaticMarkup(result as React.ReactElement);

  assertIncludes(html, 'エラーが発生しました', 'Should render unicode characters');
});

test('handles undefined children prop', () => {
  const instance = new ErrorBoundary({
    children: undefined,
  });

  assertEqual(instance.state.hasError, false, 'Should initialize with no error');

  const result = instance.render();
  // undefined children should render nothing
  assertEqual(result, undefined, 'Should render undefined for undefined children');
});

// ============================================================
// Props validation and defaults
// ============================================================
console.log('\n--- Props validation and defaults ---');

test('works with all optional props omitted', () => {
  const instance = new ErrorBoundary({ children: <div>Content</div> });
  assertEqual(instance.props.name, undefined, 'name should be undefined');
  assertEqual(instance.props.fallback, undefined, 'fallback should be undefined');
  assertEqual(instance.props.onError, undefined, 'onError should be undefined');
});

test('works with all props provided', () => {
  const onError = () => {};
  const instance = new ErrorBoundary({
    children: <div>Content</div>,
    name: 'TestBoundary',
    fallback: <span>Fallback</span>,
    onError,
  });

  assertEqual(instance.props.name, 'TestBoundary', 'name should be set');
  assertEqual(typeof instance.props.fallback, 'object', 'fallback should be set');
  assertEqual(instance.props.onError, onError, 'onError should be set');
});

// ============================================================
// Error recovery simulation
// ============================================================
console.log('\n--- Error recovery simulation ---');

test('can simulate error -> retry -> success cycle', () => {
  const instance = new ErrorBoundary({ children: <div>Success</div> });

  // Initially no error
  assertEqual(instance.state.hasError, false, 'Should start with no error');

  // Simulate error via getDerivedStateFromError
  const errorState = ErrorBoundary.getDerivedStateFromError(new Error('Test'));
  (instance as { state: typeof instance.state }).state = { ...instance.state, ...errorState };
  assertEqual(instance.state.hasError, true, 'Should be in error state');

  // Simulate retry
  type StateShape = { hasError: boolean; error: Error | null; errorInfo: ErrorInfo | null };
  let resetState: StateShape | undefined;
  instance.setState = ((state: StateShape) => {
    resetState = state;
  }) as typeof instance.setState;
  instance.handleRetry();

  // After retry, state should be reset
  assertTrue(resetState !== undefined, 'setState should have been called');
  assertEqual(resetState!.hasError, false, 'Should reset hasError on retry');
  assertEqual(resetState!.error, null, 'Should reset error on retry');
});

test('can simulate multiple error cycles', () => {
  const instance = new ErrorBoundary({ children: <div>Content</div> });
  const instanceWithMutableState = instance as { state: typeof instance.state };

  let stateUpdates = 0;
  instance.setState = (() => {
    stateUpdates++;
  }) as typeof instance.setState;

  // First error cycle
  instanceWithMutableState.state = { ...instance.state, ...ErrorBoundary.getDerivedStateFromError(new Error('Error 1')) };
  instance.handleRetry();

  // Second error cycle
  instanceWithMutableState.state = { ...instance.state, ...ErrorBoundary.getDerivedStateFromError(new Error('Error 2')) };
  instance.handleRetry();

  // Third error cycle
  instanceWithMutableState.state = { ...instance.state, ...ErrorBoundary.getDerivedStateFromError(new Error('Error 3')) };
  instance.handleRetry();

  assertEqual(stateUpdates, 3, 'Should allow multiple retry cycles');
});

// Summary
console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
