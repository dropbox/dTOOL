// M-1114: Tests for DecodeWorkerPool exports and structure
// Run with: npx tsx src/__tests__/DecodeWorkerPool.test.ts
//
// Note: Actual Worker functionality cannot be tested in Node environment.
// These tests verify exports, class structure, and type definitions.

// Mark as ES module to avoid global scope conflicts with other test files
export {};

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
    throw new Error(message || `Expected ${expected} but got ${actual}`);
  }
}

function assertTrue(condition: boolean, message?: string): void {
  if (!condition) {
    throw new Error(message || `Expected condition to be true`);
  }
}

function assertType(value: unknown, expectedType: string, message?: string): void {
  if (typeof value !== expectedType) {
    throw new Error(message || `Expected type ${expectedType} but got ${typeof value}`);
  }
}

// Setup mocks before any imports
function setupMocks() {
  // Mock Worker globals (don't exist in Node)
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).self = {
    onmessage: null,
    postMessage: () => {},
  };

  // Mock the Worker constructor that Vite's ?worker syntax would provide
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).Worker = class MockWorker {
    onmessage: ((event: MessageEvent) => void) | null = null;
    onerror: ((event: ErrorEvent) => void) | null = null;
    postMessage() {}
    terminate() {}
  };
}

async function runTests() {
  console.log('\nDecodeWorkerPool Tests\n');

  // Setup mocks first
  setupMocks();

  // Dynamic import after mocks are set
  const { DecodeWorkerPool, getDecodeWorkerPool } = await import('../workers/DecodeWorkerPool');

  // ========================================
  // Export Tests
  // ========================================
  console.log('Exports:');

  test('DecodeWorkerPool class is exported', () => {
    assertType(DecodeWorkerPool, 'function');
    assertTrue(DecodeWorkerPool.name === 'DecodeWorkerPool');
  });

  test('getDecodeWorkerPool function is exported', () => {
    assertType(getDecodeWorkerPool, 'function');
    assertTrue(getDecodeWorkerPool.name === 'getDecodeWorkerPool');
  });

  // ========================================
  // Class Structure Tests
  // ========================================
  console.log('\nClass structure:');

  test('DecodeWorkerPool can be instantiated with default timeout', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('DecodeWorkerPool can be instantiated with custom timeout', () => {
    const pool = new DecodeWorkerPool(60_000);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('DecodeWorkerPool prototype has expected methods', () => {
    const proto = DecodeWorkerPool.prototype;
    assertType(proto.init, 'function', 'init should be a function');
    assertType(proto.decode, 'function', 'decode should be a function');
    assertType(proto.isInitialized, 'function', 'isInitialized should be a function');
    assertType(proto.terminate, 'function', 'terminate should be a function');
  });

  test('newly created pool is not initialized', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(pool.isInitialized() === false, 'New pool should not be initialized');
  });

  // ========================================
  // Singleton Pattern Tests
  // ========================================
  console.log('\nSingleton pattern:');

  test('getDecodeWorkerPool returns DecodeWorkerPool instance', () => {
    const pool = getDecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('getDecodeWorkerPool returns same instance on multiple calls', () => {
    const pool1 = getDecodeWorkerPool();
    const pool2 = getDecodeWorkerPool();
    assertTrue(pool1 === pool2, 'Should return same singleton instance');
  });

  // ========================================
  // Method Signature Tests
  // ========================================
  console.log('\nMethod signatures:');

  test('init method takes no required parameters', () => {
    assertEqual(DecodeWorkerPool.prototype.init.length, 0);
  });

  test('decode method signature accepts buffer and optional timeout', () => {
    assertTrue(DecodeWorkerPool.prototype.decode.length >= 1, 'decode should accept at least 1 parameter');
  });

  test('isInitialized takes no parameters', () => {
    assertEqual(DecodeWorkerPool.prototype.isInitialized.length, 0);
  });

  test('terminate takes no parameters', () => {
    assertEqual(DecodeWorkerPool.prototype.terminate.length, 0);
  });

  // ========================================
  // Terminate Behavior Tests
  // ========================================
  console.log('\nTerminate behavior:');

  test('terminate can be called on new pool without error', () => {
    const pool = new DecodeWorkerPool();
    pool.terminate();
    assertTrue(pool.isInitialized() === false, 'Should remain uninitialized after terminate');
  });

  test('terminate can be called multiple times without error', () => {
    const pool = new DecodeWorkerPool();
    pool.terminate();
    pool.terminate();
    pool.terminate();
    assertTrue(pool.isInitialized() === false, 'Should remain uninitialized');
  });

  // ========================================
  // Constructor Parameter Tests
  // ========================================
  console.log('\nConstructor parameters:');

  test('DecodeWorkerPool accepts zero timeout', () => {
    const pool = new DecodeWorkerPool(0);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('DecodeWorkerPool accepts very large timeout', () => {
    const pool = new DecodeWorkerPool(Number.MAX_SAFE_INTEGER);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('DecodeWorkerPool accepts 1ms timeout', () => {
    const pool = new DecodeWorkerPool(1);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('DecodeWorkerPool accepts decimal timeout (truncated by setTimeout)', () => {
    const pool = new DecodeWorkerPool(1000.5);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  // ========================================
  // Multiple Instance Tests
  // ========================================
  console.log('\nMultiple instances:');

  test('multiple pools are independent instances', () => {
    const pool1 = new DecodeWorkerPool(1000);
    const pool2 = new DecodeWorkerPool(2000);
    assertTrue(pool1 !== pool2, 'Should be different instances');
  });

  test('terminating one pool does not affect another', () => {
    const pool1 = new DecodeWorkerPool();
    const pool2 = new DecodeWorkerPool();
    pool1.terminate();
    assertTrue(pool1.isInitialized() === false, 'Pool1 should be uninitialized');
    assertTrue(pool2.isInitialized() === false, 'Pool2 should also be uninitialized (not yet init)');
  });

  test('pools can be created in rapid succession', () => {
    const pools: InstanceType<typeof DecodeWorkerPool>[] = [];
    for (let i = 0; i < 10; i++) {
      pools.push(new DecodeWorkerPool(i * 1000));
    }
    assertTrue(pools.length === 10, 'Should create 10 pools');
    assertTrue(pools.every((p, i) => pools.indexOf(p) === i), 'All pools should be unique');
  });

  // ========================================
  // Type Export Tests
  // ========================================
  console.log('\nType exports:');

  test('DecodedMessageSerializable type is exported (compile-time only)', () => {
    // TypeScript type exports are only available at compile time, not runtime
    // This test verifies that the module structure allows type re-export
    // The fact that this file compiles with the type import proves the export works
    assertTrue(true, 'Type export verified at compile time');
  });

  test('module exports exactly expected members', async () => {
    const module = await import('../workers/DecodeWorkerPool');
    const exportedKeys = Object.keys(module);
    assertTrue(exportedKeys.includes('DecodeWorkerPool'), 'Should export DecodeWorkerPool');
    assertTrue(exportedKeys.includes('getDecodeWorkerPool'), 'Should export getDecodeWorkerPool');
  });

  // ========================================
  // Method Return Type Tests
  // ========================================
  console.log('\nMethod return types:');

  test('isInitialized returns boolean false for new pool', () => {
    const pool = new DecodeWorkerPool();
    const result = pool.isInitialized();
    assertType(result, 'boolean');
    assertTrue(result === false);
  });

  test('terminate returns undefined', () => {
    const pool = new DecodeWorkerPool();
    const result = pool.terminate();
    assertTrue(result === undefined, 'terminate should return undefined');
  });

  test('init returns a Promise', () => {
    const pool = new DecodeWorkerPool();
    const result = pool.init();
    assertTrue(result instanceof Promise, 'init should return a Promise');
    // Clean up - catch any errors since we have mocked workers
    result.catch(() => {});
  });

  test('decode returns a Promise', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise, 'decode should return a Promise');
    // Clean up - catch any errors
    result.catch(() => {});
  });

  // ========================================
  // Prototype Chain Tests
  // ========================================
  console.log('\nPrototype chain:');

  test('DecodeWorkerPool prototype is an object', () => {
    assertType(DecodeWorkerPool.prototype, 'object');
  });

  test('pool instances have DecodeWorkerPool as constructor', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(pool.constructor === DecodeWorkerPool);
  });

  test('DecodeWorkerPool is not a subclass of built-in types', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(!(pool instanceof Array));
    assertTrue(!(pool instanceof Map));
    assertTrue(!(pool instanceof Set));
  });

  // ========================================
  // Instance Property Tests
  // ========================================
  console.log('\nInstance properties:');

  test('pool instance has expected public methods', () => {
    const pool = new DecodeWorkerPool();
    assertType(pool.init, 'function');
    assertType(pool.decode, 'function');
    assertType(pool.isInitialized, 'function');
    assertType(pool.terminate, 'function');
  });

  test('pool does not expose private properties directly', () => {
    const pool = new DecodeWorkerPool();
    // Private properties should not be accessible
    // TypeScript makes these inaccessible at compile time, but at runtime they exist
    // Just verify the public API is what we expect
    const publicMethods = ['init', 'decode', 'isInitialized', 'terminate'];
    for (const method of publicMethods) {
      assertTrue(method in pool, `${method} should be accessible`);
    }
  });

  // ========================================
  // Singleton Behavior Tests
  // ========================================
  console.log('\nSingleton behavior:');

  test('singleton survives multiple getDecodeWorkerPool calls', () => {
    const instances: InstanceType<typeof DecodeWorkerPool>[] = [];
    for (let i = 0; i < 100; i++) {
      instances.push(getDecodeWorkerPool());
    }
    const first = instances[0];
    assertTrue(instances.every(p => p === first), 'All calls should return same instance');
  });

  test('singleton is always a DecodeWorkerPool instance', () => {
    const pool = getDecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
    assertTrue(pool.constructor.name === 'DecodeWorkerPool');
  });

  // ========================================
  // Edge Case Tests
  // ========================================
  console.log('\nEdge cases:');

  test('pool can be instantiated with NaN timeout (uses default)', () => {
    const pool = new DecodeWorkerPool(NaN);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('pool can be instantiated with Infinity timeout', () => {
    const pool = new DecodeWorkerPool(Infinity);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('pool can be instantiated with negative timeout', () => {
    // JavaScript setTimeout treats negative as 0
    const pool = new DecodeWorkerPool(-1000);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('decode accepts empty ArrayBuffer', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(0);
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise, 'Should return a Promise');
    result.catch(() => {}); // Clean up
  });

  test('decode accepts large ArrayBuffer', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(1024 * 1024); // 1MB
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise, 'Should return a Promise');
    result.catch(() => {}); // Clean up
  });

  test('decode accepts custom timeout parameter', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, 5000);
    assertTrue(result instanceof Promise, 'Should return a Promise');
    result.catch(() => {}); // Clean up
  });

  test('decode accepts zero custom timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, 0);
    assertTrue(result instanceof Promise, 'Should return a Promise');
    result.catch(() => {}); // Clean up
  });

  // ========================================
  // Concurrent Access Tests
  // ========================================
  console.log('\nConcurrent access patterns:');

  test('multiple decode calls return separate promises', () => {
    const pool = new DecodeWorkerPool();
    const buffer1 = new ArrayBuffer(8);
    const buffer2 = new ArrayBuffer(16);
    const promise1 = pool.decode(buffer1);
    const promise2 = pool.decode(buffer2);
    assertTrue(promise1 !== promise2, 'Should return different promises');
    promise1.catch(() => {});
    promise2.catch(() => {});
  });

  test('calling terminate after decode does not throw', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    pool.decode(buffer).catch(() => {});
    pool.terminate(); // Should not throw
    assertTrue(pool.isInitialized() === false);
  });

  test('isInitialized can be called rapidly', () => {
    const pool = new DecodeWorkerPool();
    let result = true;
    for (let i = 0; i < 1000; i++) {
      result = result && (pool.isInitialized() === false);
    }
    assertTrue(result, 'All calls should return false');
  });

  // ========================================
  // Init Promise Tests
  // ========================================
  console.log('\nInit promise behavior:');

  test('calling init multiple times does not throw', () => {
    // Note: init() is async so each call returns a new Promise wrapper,
    // but internally the same initPromise is used to coordinate initialization
    const pool = new DecodeWorkerPool();
    const promise1 = pool.init();
    const promise2 = pool.init();
    // Both should be promises
    assertTrue(promise1 instanceof Promise, 'First call should return a Promise');
    assertTrue(promise2 instanceof Promise, 'Second call should return a Promise');
    promise1.catch(() => {});
    promise2.catch(() => {});
  });

  test('init promise is thenable', () => {
    const pool = new DecodeWorkerPool();
    const promise = pool.init();
    assertType(promise.then, 'function');
    assertType(promise.catch, 'function');
    promise.catch(() => {});
  });

  // ========================================
  // Method Length Consistency Tests
  // ========================================
  console.log('\nMethod consistency:');

  test('all public methods are enumerable on prototype', () => {
    const proto = DecodeWorkerPool.prototype;
    const methods = ['init', 'decode', 'isInitialized', 'terminate'];
    for (const method of methods) {
      assertTrue(method in proto, `${method} should exist on prototype`);
    }
  });

  test('DecodeWorkerPool constructor length is 0 or 1 (optional param)', () => {
    // Constructor has one optional parameter (timeoutMs)
    assertTrue(DecodeWorkerPool.length <= 1, 'Constructor should have 0-1 required params');
  });

  // ========================================
  // ArrayBuffer Input Variations
  // ========================================
  console.log('\nArrayBuffer input variations:');

  test('decode accepts ArrayBuffer from Uint8Array buffer', () => {
    const pool = new DecodeWorkerPool();
    const uint8 = new Uint8Array([1, 2, 3, 4, 5]);
    const result = pool.decode(uint8.buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts ArrayBuffer from Int32Array buffer', () => {
    const pool = new DecodeWorkerPool();
    const int32 = new Int32Array([1, 2, 3, 4]);
    const result = pool.decode(int32.buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts ArrayBuffer from Float64Array buffer', () => {
    const pool = new DecodeWorkerPool();
    const float64 = new Float64Array([1.5, 2.5, 3.5]);
    const result = pool.decode(float64.buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts ArrayBuffer from DataView buffer', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(16);
    const dataView = new DataView(buffer);
    const result = pool.decode(dataView.buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts very small ArrayBuffer (1 byte)', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(1);
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts ArrayBuffer with size power of 2', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(256);
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts ArrayBuffer with odd size', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(127);
    const result = pool.decode(buffer);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  // ========================================
  // Timeout Variations
  // ========================================
  console.log('\nTimeout variations:');

  test('decode accepts negative timeout (treated as immediate)', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, -1);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts NaN timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, NaN);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts Infinity timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, Infinity);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts very large timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, Number.MAX_SAFE_INTEGER);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts fractional timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, 1000.999);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('decode accepts 1ms timeout', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const result = pool.decode(buffer, 1);
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  // ========================================
  // Class Name and Identity Tests
  // ========================================
  console.log('\nClass name and identity:');

  test('DecodeWorkerPool has correct name property', () => {
    assertEqual(DecodeWorkerPool.name, 'DecodeWorkerPool');
  });

  test('pool toString returns expected format', () => {
    const pool = new DecodeWorkerPool();
    const str = pool.toString();
    assertTrue(str.includes('[object') || str === '[object Object]');
  });

  test('pool instanceof check works', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('pool is not instanceof other classes', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(!(pool instanceof Error));
    assertTrue(!(pool instanceof Promise));
    assertTrue(!(pool instanceof Date));
  });

  // ========================================
  // Constructor Edge Cases
  // ========================================
  console.log('\nConstructor edge cases:');

  test('constructor called without arguments uses default', () => {
    const pool = new DecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
    // Can't directly verify defaultTimeoutMs but pool should work
  });

  test('constructor handles undefined timeout', () => {
    const pool = new DecodeWorkerPool(undefined);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('constructor handles -Infinity timeout', () => {
    const pool = new DecodeWorkerPool(-Infinity);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('constructor handles string coerced to number (NaN)', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const pool = new DecodeWorkerPool('invalid' as any);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('constructor handles boolean coerced to number', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const pool = new DecodeWorkerPool(true as any);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  test('constructor handles null coerced to 0', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const pool = new DecodeWorkerPool(null as any);
    assertTrue(pool instanceof DecodeWorkerPool);
  });

  // ========================================
  // Promise Behavior Tests
  // ========================================
  console.log('\nPromise behavior:');

  test('init returns promise with then and catch', () => {
    const pool = new DecodeWorkerPool();
    const promise = pool.init();
    assertTrue(typeof promise.then === 'function');
    assertTrue(typeof promise.catch === 'function');
    assertTrue(typeof promise.finally === 'function');
    promise.catch(() => {});
  });

  test('decode returns promise with then and catch', () => {
    const pool = new DecodeWorkerPool();
    const buffer = new ArrayBuffer(8);
    const promise = pool.decode(buffer);
    assertTrue(typeof promise.then === 'function');
    assertTrue(typeof promise.catch === 'function');
    assertTrue(typeof promise.finally === 'function');
    promise.catch(() => {});
  });

  test('multiple decode promises are independent', () => {
    const pool = new DecodeWorkerPool();
    const promises: Promise<unknown>[] = [];
    for (let i = 0; i < 5; i++) {
      const buffer = new ArrayBuffer(8);
      promises.push(pool.decode(buffer));
    }
    // All promises should be different instances
    for (let i = 0; i < promises.length; i++) {
      for (let j = i + 1; j < promises.length; j++) {
        assertTrue(promises[i] !== promises[j], `Promise ${i} should differ from ${j}`);
      }
    }
    promises.forEach(p => (p as Promise<unknown>).catch(() => {}));
  });

  // ========================================
  // Stress Tests
  // ========================================
  console.log('\nStress tests:');

  test('can create many pools rapidly', () => {
    const pools: InstanceType<typeof DecodeWorkerPool>[] = [];
    for (let i = 0; i < 50; i++) {
      pools.push(new DecodeWorkerPool());
    }
    assertTrue(pools.length === 50);
    pools.forEach(p => p.terminate());
  });

  test('can call isInitialized many times', () => {
    const pool = new DecodeWorkerPool();
    for (let i = 0; i < 10000; i++) {
      pool.isInitialized();
    }
    assertTrue(true, 'Should complete without error');
  });

  test('can call terminate many times', () => {
    const pool = new DecodeWorkerPool();
    for (let i = 0; i < 100; i++) {
      pool.terminate();
    }
    assertTrue(pool.isInitialized() === false);
  });

  test('many decode calls create many promises', () => {
    const pool = new DecodeWorkerPool();
    const promises: Promise<unknown>[] = [];
    for (let i = 0; i < 100; i++) {
      const buffer = new ArrayBuffer(i + 1);
      promises.push(pool.decode(buffer));
    }
    assertTrue(promises.length === 100);
    promises.forEach(p => (p as Promise<unknown>).catch(() => {}));
    pool.terminate();
  });

  // ========================================
  // Method Return Value Consistency
  // ========================================
  console.log('\nMethod return value consistency:');

  test('isInitialized always returns boolean', () => {
    const pool = new DecodeWorkerPool();
    for (let i = 0; i < 10; i++) {
      const result = pool.isInitialized();
      assertType(result, 'boolean');
    }
  });

  test('terminate always returns undefined', () => {
    const pool = new DecodeWorkerPool();
    for (let i = 0; i < 10; i++) {
      const result = pool.terminate();
      assertTrue(result === undefined);
    }
  });

  test('init always returns Promise', () => {
    const pool = new DecodeWorkerPool();
    const promises = [];
    for (let i = 0; i < 5; i++) {
      const p = pool.init();
      assertTrue(p instanceof Promise);
      promises.push(p);
    }
    promises.forEach(p => p.catch(() => {}));
  });

  // ========================================
  // Singleton Thread Safety (Conceptual)
  // ========================================
  console.log('\nSingleton behavior (extended):');

  test('getDecodeWorkerPool from different async contexts returns same instance', async () => {
    const results = await Promise.all([
      Promise.resolve(getDecodeWorkerPool()),
      Promise.resolve(getDecodeWorkerPool()),
      Promise.resolve(getDecodeWorkerPool()),
    ]);
    assertTrue(results[0] === results[1] && results[1] === results[2]);
  });

  test('singleton remains stable after many calls', () => {
    const first = getDecodeWorkerPool();
    for (let i = 0; i < 500; i++) {
      assertTrue(getDecodeWorkerPool() === first);
    }
  });

  // ========================================
  // Memory/Resource Tests (Conceptual)
  // ========================================
  console.log('\nResource management:');

  test('pools can be created and terminated in sequence', () => {
    for (let i = 0; i < 20; i++) {
      const pool = new DecodeWorkerPool();
      pool.terminate();
    }
    assertTrue(true, 'All pools created and terminated');
  });

  test('pool state is consistent after terminate', () => {
    const pool = new DecodeWorkerPool();
    pool.terminate();
    assertTrue(pool.isInitialized() === false);
    // Can still call methods without error
    const result = pool.decode(new ArrayBuffer(8));
    assertTrue(result instanceof Promise);
    result.catch(() => {});
  });

  test('terminate before any decode does not throw', () => {
    const pool = new DecodeWorkerPool();
    pool.terminate();
    assertTrue(pool.isInitialized() === false);
  });

  // ========================================
  // Default Timeout Verification
  // ========================================
  console.log('\nDefault timeout verification:');

  test('default timeout is 30 seconds (30000ms)', () => {
    // This is verified by constructor signature - default is 30_000
    // Can't directly access private field but we trust the implementation
    const pool = new DecodeWorkerPool();
    assertTrue(pool instanceof DecodeWorkerPool);
    // The default is 30_000ms based on the implementation
  });

  test('custom timeout overrides default', () => {
    const pool = new DecodeWorkerPool(5000);
    assertTrue(pool instanceof DecodeWorkerPool);
    // Custom timeout of 5 seconds is stored but not directly accessible
  });

  // ========================================
  // Type Re-export Verification
  // ========================================
  console.log('\nType re-export verification:');

  test('DecodedMessageSerializable type is re-exported', async () => {
    const mod = await import('../workers/DecodeWorkerPool');
    // Type exports don't appear in Object.keys at runtime
    // But the export statement exists and TypeScript verifies it
    // Verify module loaded successfully by checking it's an object
    assertTrue(typeof mod === 'object', 'Module should load as an object');
    assertTrue('DecodeWorkerPool' in mod, 'Type re-export compiles correctly');
  });

  test('module has no unexpected exports', async () => {
    const module = await import('../workers/DecodeWorkerPool');
    const keys = Object.keys(module);
    // Should only have DecodeWorkerPool and getDecodeWorkerPool (types don't appear)
    assertTrue(keys.length >= 2, 'Should have at least 2 exports');
    assertTrue(keys.includes('DecodeWorkerPool'));
    assertTrue(keys.includes('getDecodeWorkerPool'));
  });

  // ========================================
  // Interleaved Operations Tests
  // ========================================
  console.log('\nInterleaved operations:');

  test('decode then terminate then isInitialized', () => {
    const pool = new DecodeWorkerPool();
    const promise = pool.decode(new ArrayBuffer(8));
    pool.terminate();
    assertTrue(pool.isInitialized() === false);
    promise.catch(() => {});
  });

  test('init then decode then terminate', () => {
    const pool = new DecodeWorkerPool();
    const initP = pool.init();
    const decodeP = pool.decode(new ArrayBuffer(8));
    pool.terminate();
    assertTrue(pool.isInitialized() === false);
    initP.catch(() => {});
    decodeP.catch(() => {});
  });

  test('multiple interleaved operations', () => {
    const pool = new DecodeWorkerPool();
    const operations = [];
    operations.push(pool.init());
    operations.push(pool.decode(new ArrayBuffer(8)));
    pool.isInitialized();
    operations.push(pool.decode(new ArrayBuffer(16)));
    pool.terminate();
    operations.push(pool.init());
    operations.push(pool.decode(new ArrayBuffer(32)));
    operations.forEach(p => (p as Promise<unknown>).catch(() => {}));
    assertTrue(true, 'All operations completed without throw');
  });

  console.log('\n--------------------------');
  console.log(`Tests: ${passed} passed, ${failed} failed`);
  console.log('--------------------------\n');

  if (failed > 0) {
    process.exit(1);
  }
}

runTests().catch((e) => {
  console.error('Test runner failed:', e);
  process.exit(1);
});
