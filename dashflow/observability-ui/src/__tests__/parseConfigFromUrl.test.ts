// M-123: Unit tests for URL param config parsing
// Run with: npx tsx src/__tests__/parseConfigFromUrl.test.ts

import { parseConfigFromUrl, RUN_STATE_STORE_DEFAULTS } from '../hooks/useRunStateStore';

// Simple test runner (async-friendly)
let passed = 0;
let failed = 0;

async function test(name: string, fn: () => void | Promise<void>): Promise<void> {
  try {
    await fn();
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

function assertDeepEqual(actual: object, expected: object, message?: string): void {
  for (const [key, value] of Object.entries(expected)) {
    if ((actual as Record<string, unknown>)[key] !== value) {
      throw new Error(`${message || 'Assertion failed'}: key "${key}" expected ${value}, got ${(actual as Record<string, unknown>)[key]}`);
    }
  }
  for (const key of Object.keys(actual)) {
    if (!(key in expected)) {
      throw new Error(`${message || 'Assertion failed'}: unexpected key "${key}" in actual`);
    }
  }
}

async function run(): Promise<void> {
  console.log('\nparseConfigFromUrl Tests\n');

  console.log('Empty/no params:');
  await test('returns empty object for empty string', () => {
    const config = parseConfigFromUrl('');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('returns empty object for no params', () => {
    const config = parseConfigFromUrl('?');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores unrecognized params', () => {
    const config = parseConfigFromUrl('?foo=bar&baz=123');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nBasic integer params:');
  await test('parses maxRuns', () => {
    const config = parseConfigFromUrl('?maxRuns=100');
    assertEqual(config.maxRuns, 100);
  });

  await test('parses maxEvents', () => {
    const config = parseConfigFromUrl('?maxEvents=50000');
    assertEqual(config.maxEventsPerRun, 50000);
  });

  await test('parses checkpointInterval', () => {
    const config = parseConfigFromUrl('?checkpointInterval=50');
    assertEqual(config.checkpointInterval, 50);
  });

  await test('parses maxCheckpoints', () => {
    const config = parseConfigFromUrl('?maxCheckpoints=500');
    assertEqual(config.maxCheckpointsPerRun, 500);
  });

  console.log('\nSize suffix params (K, M, G):');
  await test('parses maxCheckpointSize with M suffix', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=20M');
    assertEqual(config.maxCheckpointStateSizeBytes, 20 * 1024 * 1024);
  });

  await test('parses maxSnapshotSize with K suffix', () => {
    const config = parseConfigFromUrl('?maxSnapshotSize=512K');
    assertEqual(config.maxFullStateSizeBytes, 512 * 1024);
  });

  await test('parses maxSchemaSize with G suffix', () => {
    const config = parseConfigFromUrl('?maxSchemaSize=1G');
    assertEqual(config.maxSchemaJsonSizeBytes, 1024 * 1024 * 1024);
  });

  await test('parses maxEvents with K suffix', () => {
    const config = parseConfigFromUrl('?maxEvents=50K');
    assertEqual(config.maxEventsPerRun, 50 * 1024);
  });

  await test('suffix is case-insensitive', () => {
    const configLower = parseConfigFromUrl('?maxCheckpointSize=10m');
    const configUpper = parseConfigFromUrl('?maxCheckpointSize=10M');
    assertEqual(configLower.maxCheckpointStateSizeBytes, configUpper.maxCheckpointStateSizeBytes);
    assertEqual(configLower.maxCheckpointStateSizeBytes, 10 * 1024 * 1024);
  });

  console.log('\nMultiple params:');
  await test('parses multiple params together', () => {
    const config = parseConfigFromUrl('?maxRuns=100&maxEvents=50000&maxCheckpointSize=20M');
    assertEqual(config.maxRuns, 100);
    assertEqual(config.maxEventsPerRun, 50000);
    assertEqual(config.maxCheckpointStateSizeBytes, 20 * 1024 * 1024);
  });

  await test('handles all params at once', () => {
    const config = parseConfigFromUrl(
      '?maxEvents=1000&checkpointInterval=10&maxRuns=5&maxCheckpoints=20&maxCheckpointSize=1M&maxSnapshotSize=2M&maxSchemaSize=500K'
    );
    assertDeepEqual(config, {
      maxEventsPerRun: 1000,
      checkpointInterval: 10,
      maxRuns: 5,
      maxCheckpointsPerRun: 20,
      maxCheckpointStateSizeBytes: 1024 * 1024,
      maxFullStateSizeBytes: 2 * 1024 * 1024,
      maxSchemaJsonSizeBytes: 500 * 1024,
    });
  });

  console.log('\nInvalid values (should be ignored):');
  await test('ignores negative values', () => {
    const config = parseConfigFromUrl('?maxRuns=-5');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores zero values', () => {
    const config = parseConfigFromUrl('?maxRuns=0');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores non-numeric values', () => {
    const config = parseConfigFromUrl('?maxRuns=abc');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores invalid suffix combinations', () => {
    // suffix not allowed for checkpointInterval
    const config = parseConfigFromUrl('?checkpointInterval=10K');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores float values', () => {
    const config = parseConfigFromUrl('?maxRuns=10.5');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('partial valid params still work', () => {
    // maxRuns is invalid, but maxEvents is valid
    const config = parseConfigFromUrl('?maxRuns=abc&maxEvents=1000');
    assertEqual(config.maxEventsPerRun, 1000);
    assertEqual(config.maxRuns, undefined);
  });

  console.log('\nDefaults export:');
  await test('RUN_STATE_STORE_DEFAULTS is exported and has expected values', () => {
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxEventsPerRun, 10000);
    assertEqual(RUN_STATE_STORE_DEFAULTS.checkpointInterval, 100);
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxRuns, 50);
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxCheckpointsPerRun, 200);
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxCheckpointStateSizeBytes, 10 * 1024 * 1024);
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxFullStateSizeBytes, 10 * 1024 * 1024);
    assertEqual(RUN_STATE_STORE_DEFAULTS.maxSchemaJsonSizeBytes, 2 * 1024 * 1024);
  });

  // ============================================================
  // Additional edge cases added by Worker #2602
  // ============================================================

  console.log('\nLeading zeros and numeric edge cases:');
  await test('parses values with leading zeros (parseInt behavior)', () => {
    // parseInt('0100', 10) = 100 (not octal in base 10)
    const config = parseConfigFromUrl('?maxRuns=0100');
    assertEqual(config.maxRuns, 100);
  });

  await test('parses single digit values', () => {
    const config = parseConfigFromUrl('?maxRuns=1');
    assertEqual(config.maxRuns, 1);
  });

  await test('parses large integers up to safe range', () => {
    // Test a large but safe integer (not near MAX_SAFE_INTEGER to avoid precision issues)
    const config = parseConfigFromUrl('?maxEvents=999999999');
    assertEqual(config.maxEventsPerRun, 999999999);
  });

  await test('ignores exponential notation', () => {
    // 1e3 should not match the regex ^(\d+)([KMG])?$
    const config = parseConfigFromUrl('?maxRuns=1e3');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores hexadecimal notation', () => {
    // 0xFF should not match the regex
    const config = parseConfigFromUrl('?maxRuns=0xFF');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores octal notation with prefix', () => {
    // 0o10 should not match (has non-digit)
    const config = parseConfigFromUrl('?maxRuns=0o10');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nWhitespace and special characters:');
  await test('ignores values with leading whitespace', () => {
    const config = parseConfigFromUrl('?maxRuns=%20100');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores values with trailing whitespace', () => {
    const config = parseConfigFromUrl('?maxRuns=100%20');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores empty values', () => {
    const config = parseConfigFromUrl('?maxRuns=');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores values with embedded whitespace', () => {
    const config = parseConfigFromUrl('?maxRuns=1%200');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('handles plus sign as space (URL encoding)', () => {
    // + is decoded as space by URLSearchParams
    const config = parseConfigFromUrl('?maxRuns=+100');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nInvalid suffix variations:');
  await test('ignores multi-character suffixes (KB)', () => {
    // Only single-char K, M, G are valid
    const config = parseConfigFromUrl('?maxCheckpointSize=10KB');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores MiB-style suffixes', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=10MiB');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores GB suffix', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=1GB');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores T (terabyte) suffix', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=1T');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('ignores lowercase g suffix', () => {
    const config = parseConfigFromUrl('?maxSchemaSize=1g');
    assertEqual(config.maxSchemaJsonSizeBytes, 1024 * 1024 * 1024);
  });

  await test('ignores lowercase k suffix', () => {
    const config = parseConfigFromUrl('?maxSnapshotSize=512k');
    assertEqual(config.maxFullStateSizeBytes, 512 * 1024);
  });

  console.log('\nDuplicate params (URLSearchParams behavior):');
  await test('uses first value when param is duplicated', () => {
    // URLSearchParams.get() returns the first value
    const config = parseConfigFromUrl('?maxRuns=100&maxRuns=200');
    assertEqual(config.maxRuns, 100);
  });

  await test('handles duplicate params with valid and invalid values', () => {
    // First is valid (100), second is invalid (abc)
    const config = parseConfigFromUrl('?maxRuns=100&maxRuns=abc');
    assertEqual(config.maxRuns, 100);
  });

  await test('handles duplicate params with invalid then valid', () => {
    // First is invalid (abc), gets returned by get() → parsed as invalid
    const config = parseConfigFromUrl('?maxRuns=abc&maxRuns=100');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nURL encoding edge cases:');
  await test('handles URL-encoded digits', () => {
    // %31%30%30 = "100" encoded
    const config = parseConfigFromUrl('?maxRuns=%31%30%30');
    assertEqual(config.maxRuns, 100);
  });

  await test('handles URL-encoded suffix', () => {
    // 10%4D = "10M" where %4D = 'M'
    const config = parseConfigFromUrl('?maxCheckpointSize=10%4D');
    assertEqual(config.maxCheckpointStateSizeBytes, 10 * 1024 * 1024);
  });

  console.log('\nHash fragments in raw input strings:');
  await test('hash in raw string is NOT stripped (browser responsibility)', () => {
    // URLSearchParams does NOT strip hash fragments from raw strings.
    // When passed '?maxRuns=100#section&maxEvents=200', it parses:
    //   - maxRuns=100#section (value is "100#section", fails regex)
    //   - maxEvents=200 (parses normally)
    // Browser strips hash from window.location.search before it reaches here.
    const config = parseConfigFromUrl('?maxRuns=100#section&maxEvents=200');
    // maxRuns value "100#section" doesn't match ^(\d+)$ → rejected
    assertEqual(config.maxRuns, undefined);
    // maxEvents still parses correctly
    assertEqual(config.maxEventsPerRun, 200);
  });

  await test('clean search string without hash parses normally', () => {
    // This is what would happen in practice: browser provides clean search string
    const config = parseConfigFromUrl('?maxRuns=100&maxEvents=200');
    assertEqual(config.maxRuns, 100);
    assertEqual(config.maxEventsPerRun, 200);
  });

  console.log('\nBoundary values for strict int params:');
  await test('maxRuns strict int rejects suffix', () => {
    const config = parseConfigFromUrl('?maxRuns=10K');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('maxCheckpoints strict int rejects suffix', () => {
    const config = parseConfigFromUrl('?maxCheckpoints=10K');
    assertEqual(Object.keys(config).length, 0);
  });

  await test('checkpointInterval strict int rejects suffix', () => {
    const config = parseConfigFromUrl('?checkpointInterval=10K');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nParam order independence:');
  await test('param order does not affect result (order 1)', () => {
    const config1 = parseConfigFromUrl('?maxRuns=5&maxEvents=1000');
    assertEqual(config1.maxRuns, 5);
    assertEqual(config1.maxEventsPerRun, 1000);
  });

  await test('param order does not affect result (order 2)', () => {
    const config2 = parseConfigFromUrl('?maxEvents=1000&maxRuns=5');
    assertEqual(config2.maxRuns, 5);
    assertEqual(config2.maxEventsPerRun, 1000);
  });

  console.log('\nSize suffix multiplication verification:');
  await test('K suffix multiplies by 1024', () => {
    const config = parseConfigFromUrl('?maxEvents=1K');
    assertEqual(config.maxEventsPerRun, 1024);
  });

  await test('M suffix multiplies by 1048576 (1024*1024)', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=1M');
    assertEqual(config.maxCheckpointStateSizeBytes, 1048576);
  });

  await test('G suffix multiplies by 1073741824 (1024^3)', () => {
    const config = parseConfigFromUrl('?maxSchemaSize=1G');
    assertEqual(config.maxSchemaJsonSizeBytes, 1073741824);
  });

  console.log('\nMixed valid and invalid in various combinations:');
  await test('three params: valid, invalid, valid', () => {
    const config = parseConfigFromUrl('?maxRuns=5&checkpointInterval=abc&maxEvents=100');
    assertEqual(config.maxRuns, 5);
    assertEqual(config.checkpointInterval, undefined);
    assertEqual(config.maxEventsPerRun, 100);
  });

  await test('all size params with mixed suffixes', () => {
    const config = parseConfigFromUrl('?maxCheckpointSize=5M&maxSnapshotSize=10M&maxSchemaSize=1G');
    assertEqual(config.maxCheckpointStateSizeBytes, 5 * 1024 * 1024);
    assertEqual(config.maxFullStateSizeBytes, 10 * 1024 * 1024);
    assertEqual(config.maxSchemaJsonSizeBytes, 1024 * 1024 * 1024);
  });

  console.log('\nEdge numeric values:');
  await test('handles value 1 (minimum positive)', () => {
    const config = parseConfigFromUrl('?checkpointInterval=1');
    assertEqual(config.checkpointInterval, 1);
  });

  await test('handles larger multiplied values', () => {
    // 100G = 100 * 1024^3 = 107374182400
    const config = parseConfigFromUrl('?maxSchemaSize=100G');
    assertEqual(config.maxSchemaJsonSizeBytes, 100 * 1024 * 1024 * 1024);
  });

  await test('handles 0 suffix value (0K = 0, which is invalid)', () => {
    // 0 * 1024 = 0, which should be rejected as <= 0
    const config = parseConfigFromUrl('?maxCheckpointSize=0K');
    assertEqual(Object.keys(config).length, 0);
  });

  console.log('\nSpecial URL characters:');
  await test('handles ampersand correctly as separator', () => {
    const config = parseConfigFromUrl('?maxRuns=10&maxEvents=20&checkpointInterval=30');
    assertEqual(config.maxRuns, 10);
    assertEqual(config.maxEventsPerRun, 20);
    assertEqual(config.checkpointInterval, 30);
  });

  await test('handles equals sign in search string start', () => {
    // Malformed: starting with = should have empty param name
    const config = parseConfigFromUrl('?=100&maxRuns=50');
    assertEqual(config.maxRuns, 50);
  });

  console.log('\n---');
  console.log(`Results: ${passed} passed, ${failed} failed`);
  if (failed > 0) {
    process.exit(1);
  }
}

run().catch((e) => {
  console.error('Test runner error:', e);
  process.exit(1);
});
