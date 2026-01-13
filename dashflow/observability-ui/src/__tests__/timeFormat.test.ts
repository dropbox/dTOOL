// Unit tests for relative time formatting
// Run with: npx tsx src/__tests__/timeFormat.test.ts

import { formatRelativeTime } from '../hooks/useRunStateStore';
import { formatUptime, formatTimestamp, formatKafkaStatus } from '../utils/timeFormat';

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

function assertEqual(actual: unknown, expected: unknown, message?: string): void {
  const actualStr = JSON.stringify(actual);
  const expectedStr = JSON.stringify(expected);
  if (actualStr !== expectedStr) {
    throw new Error(`${message || 'Assertion failed'}: expected ${expectedStr}, got ${actualStr}`);
  }
}

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(`${message || 'Assertion failed'}: expected "${haystack}" to include "${needle}"`);
  }
}

async function run(): Promise<void> {
  console.log('\nTime Format Tests\n');

  await test('returns "just now" for current timestamp', () => {
    assertEqual(formatRelativeTime(1000, 1000), 'just now');
  });

  await test('formats seconds ago', () => {
    assertEqual(formatRelativeTime(0, 1500), '1s ago');
  });

  await test('formats future timestamps with delta and ISO time', () => {
    const now = 1700000000000;
    const future = now + 5000;
    const iso = new Date(future).toISOString();
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (5s ahead;');
    assertIncludes(formatted, iso);
  });

  // Additional formatRelativeTime tests for full coverage
  await test('formats minutes ago (1 minute)', () => {
    assertEqual(formatRelativeTime(0, 60000), '1m ago');
  });

  await test('formats minutes ago (59 minutes)', () => {
    assertEqual(formatRelativeTime(0, 59 * 60000), '59m ago');
  });

  await test('formats hours ago (1 hour)', () => {
    assertEqual(formatRelativeTime(0, 3600000), '1h ago');
  });

  await test('formats hours ago (23 hours)', () => {
    assertEqual(formatRelativeTime(0, 23 * 3600000), '23h ago');
  });

  await test('formats days ago (1 day)', () => {
    assertEqual(formatRelativeTime(0, 86400000), '1d ago');
  });

  await test('formats days ago (7 days)', () => {
    assertEqual(formatRelativeTime(0, 7 * 86400000), '7d ago');
  });

  await test('formats future with minutes delta', () => {
    const now = 1700000000000;
    const future = now + 120000; // 2 minutes ahead
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (2m ahead;');
  });

  await test('formats future with hours delta', () => {
    const now = 1700000000000;
    const future = now + 7200000; // 2 hours ahead
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (2h ahead;');
  });

  await test('formats future with days delta', () => {
    const now = 1700000000000;
    const future = now + 172800000; // 2 days ahead
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (2d ahead;');
  });

  await test('formatUptime clamps negative values to 0s', () => {
    assertEqual(formatUptime(-5), '0s');
  });

  await test('formatUptime returns 0s for NaN', () => {
    assertEqual(formatUptime(NaN), '0s');
  });

  await test('formatUptime returns 0s for Infinity', () => {
    assertEqual(formatUptime(Infinity), '0s');
    assertEqual(formatUptime(-Infinity), '0s');
  });

  await test('formatUptime returns 0s for zero', () => {
    assertEqual(formatUptime(0), '0s');
  });

  await test('formatUptime formats seconds', () => {
    assertEqual(formatUptime(59), '59s');
  });

  await test('formatUptime formats single second', () => {
    assertEqual(formatUptime(1), '1s');
    assertEqual(formatUptime(1.9), '1s');
  });

  await test('formatUptime formats minutes + seconds', () => {
    assertEqual(formatUptime(61.9), '1m 1s');
  });

  await test('formatUptime formats exactly 60 seconds as 1m 0s', () => {
    assertEqual(formatUptime(60), '1m 0s');
  });

  await test('formatUptime formats hours + minutes', () => {
    assertEqual(formatUptime(3600), '1h 0m');
  });

  await test('formatUptime formats hours with minutes (non-zero)', () => {
    assertEqual(formatUptime(3660), '1h 1m');
    assertEqual(formatUptime(7199), '1h 59m');
  });

  await test('formatUptime formats days for > 24h durations', () => {
    assertEqual(formatUptime(86400), '1d 0h 0m');
    assertEqual(formatUptime(90061), '1d 1h 1m');
  });

  await test('formatUptime handles multi-day durations', () => {
    assertEqual(formatUptime(172800), '2d 0h 0m');
    assertEqual(formatUptime(259200 + 7200 + 1800), '3d 2h 30m');
  });

  await test('formatUptime handles 23h 59m (just under a day)', () => {
    assertEqual(formatUptime(86340), '23h 59m');
  });

  // T-06: formatTimestamp tests
  await test('formatTimestamp returns consistent 24h format', () => {
    const date = new Date('2026-01-05T14:30:45');
    const result = formatTimestamp(date);
    // Should be 24h format like "14:30:45"
    assertIncludes(result, '14:30:45');
  });

  await test('formatTimestamp accepts number (epoch ms)', () => {
    const epoch = new Date('2026-01-05T09:15:30').getTime();
    const result = formatTimestamp(epoch);
    assertIncludes(result, '09:15:30');
  });

  // T-08: formatKafkaStatus tests
  await test('formatKafkaStatus maps waiting_for_messages', () => {
    assertEqual(formatKafkaStatus('waiting_for_messages'), 'Waiting for data...');
  });

  await test('formatKafkaStatus maps waiting (alias)', () => {
    assertEqual(formatKafkaStatus('waiting'), 'Waiting for data...');
  });

  await test('formatKafkaStatus maps connected', () => {
    assertEqual(formatKafkaStatus('connected'), 'Connected');
  });

  await test('formatKafkaStatus maps healthy to Connected', () => {
    assertEqual(formatKafkaStatus('healthy'), 'Connected');
  });

  await test('formatKafkaStatus maps reconnecting', () => {
    assertEqual(formatKafkaStatus('reconnecting'), 'Reconnecting...');
  });

  await test('formatKafkaStatus maps disconnected', () => {
    assertEqual(formatKafkaStatus('disconnected'), 'Disconnected');
  });

  await test('formatKafkaStatus maps error', () => {
    assertEqual(formatKafkaStatus('error'), 'Error');
  });

  await test('formatKafkaStatus maps degraded', () => {
    assertEqual(formatKafkaStatus('degraded'), 'Degraded');
  });

  await test('formatKafkaStatus handles unknown status gracefully', () => {
    // Should capitalize first letter and replace underscores with spaces
    assertEqual(formatKafkaStatus('some_unknown_state'), 'Some unknown state');
  });

  await test('formatKafkaStatus handles empty string', () => {
    // Empty string capitalizes first letter of empty = empty string
    assertEqual(formatKafkaStatus(''), '');
  });

  await test('formatKafkaStatus handles single character', () => {
    assertEqual(formatKafkaStatus('x'), 'X');
  });

  await test('formatKafkaStatus is case-insensitive', () => {
    assertEqual(formatKafkaStatus('CONNECTED'), 'Connected');
    assertEqual(formatKafkaStatus('Waiting_For_Messages'), 'Waiting for data...');
    assertEqual(formatKafkaStatus('HEALTHY'), 'Connected');
    assertEqual(formatKafkaStatus('Reconnecting'), 'Reconnecting...');
  });

  // ============================================================
  // Additional edge case tests for comprehensive coverage
  // ============================================================

  // --- formatRelativeTime boundary tests ---

  await test('formatRelativeTime boundary: 999ms = "just now"', () => {
    assertEqual(formatRelativeTime(0, 999), 'just now');
  });

  await test('formatRelativeTime boundary: 1000ms = "1s ago"', () => {
    assertEqual(formatRelativeTime(0, 1000), '1s ago');
  });

  await test('formatRelativeTime boundary: 1999ms = "1s ago" (floors)', () => {
    assertEqual(formatRelativeTime(0, 1999), '1s ago');
  });

  await test('formatRelativeTime boundary: 59999ms = "59s ago"', () => {
    assertEqual(formatRelativeTime(0, 59999), '59s ago');
  });

  await test('formatRelativeTime boundary: 60000ms = "1m ago"', () => {
    assertEqual(formatRelativeTime(0, 60000), '1m ago');
  });

  await test('formatRelativeTime boundary: 3599999ms = "59m ago"', () => {
    assertEqual(formatRelativeTime(0, 3599999), '59m ago');
  });

  await test('formatRelativeTime boundary: 3600000ms = "1h ago"', () => {
    assertEqual(formatRelativeTime(0, 3600000), '1h ago');
  });

  await test('formatRelativeTime boundary: 86399999ms = "23h ago"', () => {
    assertEqual(formatRelativeTime(0, 86399999), '23h ago');
  });

  await test('formatRelativeTime boundary: 86400000ms = "1d ago"', () => {
    assertEqual(formatRelativeTime(0, 86400000), '1d ago');
  });

  await test('formatRelativeTime very large: 365 days', () => {
    const ms365days = 365 * 86400000;
    assertEqual(formatRelativeTime(0, ms365days), '365d ago');
  });

  await test('formatRelativeTime very large: 1000 days', () => {
    const ms1000days = 1000 * 86400000;
    assertEqual(formatRelativeTime(0, ms1000days), '1000d ago');
  });

  await test('formatRelativeTime future with sub-second delta (ms format)', () => {
    const now = 1700000000000;
    const future = now + 500; // 500ms ahead
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (500ms ahead;');
  });

  await test('formatRelativeTime future exact second boundary', () => {
    const now = 1700000000000;
    const future = now + 1000; // 1 second ahead
    const formatted = formatRelativeTime(future, now);
    assertIncludes(formatted, 'in the future (1s ahead;');
  });

  await test('formatRelativeTime zero diff = "just now"', () => {
    assertEqual(formatRelativeTime(5000, 5000), 'just now');
  });

  await test('formatRelativeTime with very small diff (1ms) = "just now"', () => {
    assertEqual(formatRelativeTime(0, 1), 'just now');
  });

  // --- formatUptime boundary tests ---

  await test('formatUptime boundary: 59.999s floors to 59s', () => {
    assertEqual(formatUptime(59.999), '59s');
  });

  await test('formatUptime boundary: 60.0s = "1m 0s"', () => {
    assertEqual(formatUptime(60.0), '1m 0s');
  });

  await test('formatUptime boundary: 3599s = "59m 59s"', () => {
    assertEqual(formatUptime(3599), '59m 59s');
  });

  await test('formatUptime boundary: 3600s = "1h 0m"', () => {
    assertEqual(formatUptime(3600), '1h 0m');
  });

  await test('formatUptime boundary: 86399s = "23h 59m"', () => {
    assertEqual(formatUptime(86399), '23h 59m');
  });

  await test('formatUptime boundary: 86400s = "1d 0h 0m"', () => {
    assertEqual(formatUptime(86400), '1d 0h 0m');
  });

  await test('formatUptime very large: 7 days (1 week)', () => {
    assertEqual(formatUptime(7 * 86400), '7d 0h 0m');
  });

  await test('formatUptime very large: 30 days (1 month)', () => {
    assertEqual(formatUptime(30 * 86400), '30d 0h 0m');
  });

  await test('formatUptime very large: 365 days (1 year)', () => {
    assertEqual(formatUptime(365 * 86400), '365d 0h 0m');
  });

  await test('formatUptime very large: 1000 days', () => {
    assertEqual(formatUptime(1000 * 86400), '1000d 0h 0m');
  });

  await test('formatUptime complex multi-day with all components', () => {
    // 5 days, 12 hours, 30 minutes, 45 seconds
    const seconds = 5 * 86400 + 12 * 3600 + 30 * 60 + 45;
    assertEqual(formatUptime(seconds), '5d 12h 30m');
  });

  await test('formatUptime sub-second value floors to 0s', () => {
    assertEqual(formatUptime(0.5), '0s');
  });

  await test('formatUptime sub-second value 0.001 floors to 0s', () => {
    assertEqual(formatUptime(0.001), '0s');
  });

  await test('formatUptime exactly 1 second', () => {
    assertEqual(formatUptime(1), '1s');
  });

  // --- formatTimestamp edge cases ---

  await test('formatTimestamp midnight boundary (00:00:00)', () => {
    const midnight = new Date('2026-01-05T00:00:00');
    const result = formatTimestamp(midnight);
    assertIncludes(result, '00:00:00');
  });

  await test('formatTimestamp end of day (23:59:59)', () => {
    const endOfDay = new Date('2026-01-05T23:59:59');
    const result = formatTimestamp(endOfDay);
    assertIncludes(result, '23:59:59');
  });

  await test('formatTimestamp epoch start (1970-01-01)', () => {
    const epoch = new Date(0);
    const result = formatTimestamp(epoch);
    // Should be 00:00:00 in UTC (may vary by timezone)
    assertIncludes(result, ':00:00');
  });

  await test('formatTimestamp with epoch ms = 0', () => {
    const result = formatTimestamp(0);
    assertIncludes(result, ':00:00');
  });

  await test('formatTimestamp far future date (year 3000)', () => {
    const farFuture = new Date('3000-06-15T12:30:45');
    const result = formatTimestamp(farFuture);
    assertIncludes(result, '12:30:45');
  });

  await test('formatTimestamp early morning single digit hour', () => {
    const earlyMorning = new Date('2026-01-05T05:03:07');
    const result = formatTimestamp(earlyMorning);
    assertIncludes(result, '05:03:07');
  });

  await test('formatTimestamp noon (12:00:00)', () => {
    const noon = new Date('2026-01-05T12:00:00');
    const result = formatTimestamp(noon);
    assertIncludes(result, '12:00:00');
  });

  await test('formatTimestamp 1 second before midnight (23:59:59)', () => {
    const beforeMidnight = new Date('2026-01-05T23:59:59');
    const result = formatTimestamp(beforeMidnight);
    assertIncludes(result, '23:59:59');
  });

  // --- formatKafkaStatus edge cases ---

  await test('formatKafkaStatus with multiple underscores', () => {
    assertEqual(formatKafkaStatus('some___status'), 'Some   status');
  });

  await test('formatKafkaStatus with leading underscore', () => {
    // First char is underscore, toUpperCase('_') = '_', so underscore preserved
    assertEqual(formatKafkaStatus('_status'), '_status');
  });

  await test('formatKafkaStatus with trailing underscore', () => {
    assertEqual(formatKafkaStatus('status_'), 'Status ');
  });

  await test('formatKafkaStatus with numbers', () => {
    assertEqual(formatKafkaStatus('status123'), 'Status123');
  });

  await test('formatKafkaStatus with mixed case underscores (complex)', () => {
    // Fallback only capitalizes first char, preserves rest of case, replaces underscores
    assertEqual(formatKafkaStatus('SOME_Mixed_STATUS'), 'SOME Mixed STATUS');
  });

  await test('formatKafkaStatus all lowercase no underscores (unmapped)', () => {
    assertEqual(formatKafkaStatus('pending'), 'Pending');
  });

  await test('formatKafkaStatus already capitalized no underscores', () => {
    assertEqual(formatKafkaStatus('Starting'), 'Starting');
  });

  await test('formatKafkaStatus with single underscore at end', () => {
    assertEqual(formatKafkaStatus('running_'), 'Running ');
  });

  await test('formatKafkaStatus numeric only', () => {
    assertEqual(formatKafkaStatus('123'), '123');
  });

  await test('formatKafkaStatus long unknown status', () => {
    assertEqual(formatKafkaStatus('very_long_unknown_status_name'), 'Very long unknown status name');
  });

  // Summary
  console.log('\n--------------------------');
  console.log(`Tests: ${passed} passed, ${failed} failed`);
  console.log('--------------------------\n');

  if (failed > 0) {
    process.exit(1);
  }
}

void run();
