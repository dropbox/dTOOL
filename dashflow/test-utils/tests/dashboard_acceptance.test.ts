/**
 * Dashboard Acceptance Test
 *
 * Validates that existing dashboard metrics are semantically correct.
 * This test verifies:
 * - Prometheus metrics are accessible and in valid ranges
 * - Grafana can query the same values as Prometheus
 * - Query counts are internally consistent (passed + failed = total)
 * - Rate metrics are non-negative
 *
 * This test catches:
 * - Placeholder values (e.g., always 1.0, always $0.00)
 * - Incorrect aggregations (sum vs avg confusion)
 * - Missing data that should be present
 * - Grafana-Prometheus data pipeline issues
 *
 * NOTE: This test validates EXISTING data. To generate test data first:
 *   1. Use --emit flag: npx ts-node test-utils/tests/dashboard_acceptance.test.ts --emit
 *   2. Or manually: cargo run -p dashflow-streaming --example send_test_metrics
 *   3. Or run librarian: cargo run -p librarian -- query "test query"
 *
 * Prerequisites:
 * - Docker observability stack running: docker-compose -f docker-compose.dashstream.yml up -d
 * - Wait 30 seconds for stack to stabilize
 * - Some metrics must exist (run librarian query or --emit flag)
 *
 * Usage:
 *   npx ts-node test-utils/tests/dashboard_acceptance.test.ts         # Validate only
 *   npx ts-node test-utils/tests/dashboard_acceptance.test.ts --emit  # Send test events first
 */

import fetch from 'node-fetch';
import { spawn } from 'child_process';
import * as path from 'path';

const PROMETHEUS_URL = process.env.PROMETHEUS_URL || 'http://localhost:9090';

// M-105: Emit test events before validation if --emit flag is passed
async function emitTestEvents(): Promise<boolean> {
    console.log('=== Emitting Test Events ===\n');
    console.log('Running: cargo run -p dashflow-streaming --example send_test_metrics\n');

    return new Promise((resolve) => {
        const cargoProcess = spawn('cargo', [
            'run', '-p', 'dashflow-streaming', '--example', 'send_test_metrics'
        ], {
            cwd: path.resolve(__dirname, '..', '..'),
            stdio: 'inherit',
        });

        cargoProcess.on('close', (code) => {
            if (code === 0) {
                console.log('\n✅ Test events emitted successfully');
                console.log('Waiting 5 seconds for metrics to propagate...\n');
                setTimeout(() => resolve(true), 5000);
            } else {
                console.error(`\n❌ Failed to emit test events (exit code: ${code})`);
                resolve(false);
            }
        });

        cargoProcess.on('error', (err) => {
            console.error(`\n❌ Failed to spawn cargo: ${err.message}`);
            resolve(false);
        });
    });
}
const GRAFANA_URL = process.env.GRAFANA_URL || 'http://localhost:3000';
const GRAFANA_USER = process.env.GRAFANA_USER || 'admin';
const GRAFANA_PASS = process.env.GRAFANA_PASS || 'admin';

// Prometheus datasource UID - auto-detected from Grafana API, or override via env
let prometheusDataSourceUid: string | null = process.env.PROMETHEUS_DS_UID || null;

async function getPrometheusDataSourceUid(): Promise<string> {
    if (prometheusDataSourceUid) {
        return prometheusDataSourceUid;
    }

    try {
        const response = await fetch(`${GRAFANA_URL}/api/datasources`, {
            headers: {
                'Authorization': 'Basic ' + Buffer.from(`${GRAFANA_USER}:${GRAFANA_PASS}`).toString('base64'),
            },
        });

        if (!response.ok) {
            console.warn(`Warning: Could not fetch Grafana datasources (${response.status}), falling back to 'prometheus'`);
            prometheusDataSourceUid = 'prometheus';
            return prometheusDataSourceUid;
        }

        const datasources = await response.json() as Array<{ uid: string; type: string; name: string }>;

        // Find Prometheus datasource by type
        const promDs = datasources.find(ds => ds.type === 'prometheus');
        if (promDs) {
            prometheusDataSourceUid = promDs.uid;
            console.log(`Auto-detected Prometheus datasource UID: ${prometheusDataSourceUid} (name: ${promDs.name})`);
            return prometheusDataSourceUid;
        }

        console.warn('Warning: No Prometheus datasource found in Grafana, falling back to "prometheus"');
        prometheusDataSourceUid = 'prometheus';
        return prometheusDataSourceUid;
    } catch (error) {
        console.warn(`Warning: Error fetching datasources: ${error}, falling back to 'prometheus'`);
        prometheusDataSourceUid = 'prometheus';
        return prometheusDataSourceUid;
    }
}

interface PrometheusQueryResult {
    status: string;
    data: {
        resultType: string;
        result: Array<{
            metric: Record<string, string>;
            value: [number, string];
        }>;
    };
}

async function queryPrometheus(query: string): Promise<number | null> {
    const response = await fetch(
        `${PROMETHEUS_URL}/api/v1/query?query=${encodeURIComponent(query)}`
    );
    const data = await response.json() as PrometheusQueryResult;

    if (data.status !== 'success' || data.data.result.length === 0) {
        return null;
    }

    return parseFloat(data.data.result[0].value[1]);
}

async function queryGrafana(expr: string): Promise<number | null> {
    const now = Date.now();
    const from = now - 5 * 60 * 1000; // 5 minutes ago
    const dsUid = await getPrometheusDataSourceUid();

    const response = await fetch(`${GRAFANA_URL}/api/ds/query`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Basic ' + Buffer.from(`${GRAFANA_USER}:${GRAFANA_PASS}`).toString('base64'),
        },
        body: JSON.stringify({
            queries: [{
                refId: 'A',
                datasource: { type: 'prometheus', uid: dsUid },
                expr: expr,
                intervalMs: 15000,
                maxDataPoints: 100,
            }],
            from: from.toString(),
            to: now.toString(),
        }),
    });

    const data = await response.json() as any;

    if (!data.results?.A?.frames?.[0]?.data?.values?.[1]) {
        return null;
    }

    const values = data.results.A.frames[0].data.values[1];
    return values.length > 0 ? values[values.length - 1] : null;
}

interface AcceptanceTestResult {
    passed: boolean;
    checks: Array<{
        name: string;
        passed: boolean;
        expected: string;
        actual: string;
        message: string;
    }>;
}

async function runAcceptanceTests(): Promise<AcceptanceTestResult> {
    console.log('=== Dashboard Acceptance Test ===\n');
    console.log('This test verifies the dashboard shows TRUTH, not just data.\n');

    const result: AcceptanceTestResult = {
        passed: true,
        checks: [],
    };

    // Check 1: Quality Score is in valid range [0, 1]
    console.log('Check 1: Quality score range validation...');
    const qualityScore = await queryPrometheus('dashstream_quality_monitor_quality_score');
    if (qualityScore !== null) {
        const isValid = qualityScore >= 0 && qualityScore <= 1;
        result.checks.push({
            name: 'Quality Score Range',
            passed: isValid,
            expected: '[0.0, 1.0]',
            actual: qualityScore.toString(),
            message: isValid
                ? `Quality score ${qualityScore} is valid`
                : `FAIL: Quality score ${qualityScore} is outside valid range`,
        });
        if (!isValid) result.passed = false;
        console.log(`  ${isValid ? 'PASS' : 'FAIL'}: Quality score = ${qualityScore}`);
    } else {
        result.checks.push({
            name: 'Quality Score Range',
            passed: false,
            expected: '[0.0, 1.0]',
            actual: 'null',
            message: 'No quality score metric found',
        });
        result.passed = false;
        console.log('  FAIL: No quality score metric found');
    }

    // Check 2: Quality score is NOT a constant placeholder
    console.log('\nCheck 2: Quality score is not a placeholder...');
    const queryCount = await queryPrometheus('dashstream_quality_monitor_queries_total');
    if (qualityScore !== null && queryCount !== null && queryCount > 0) {
        // If we have multiple queries, the quality score shouldn't be exactly 1.0 or 0.0
        // (statistically unlikely for real data)
        const isPlaceholder = qualityScore === 1.0 || qualityScore === 0.0;
        const passed = !isPlaceholder || queryCount < 3;
        result.checks.push({
            name: 'Quality Score Not Placeholder',
            passed: passed,
            expected: 'Varying value (not constant 1.0 or 0.0)',
            actual: qualityScore.toString(),
            message: passed
                ? `Quality score ${qualityScore} appears to be real data`
                : `WARNING: Quality score is exactly ${qualityScore} - may be placeholder`,
        });
        console.log(`  ${passed ? 'PASS' : 'WARN'}: Quality score = ${qualityScore} with ${queryCount} queries`);
    } else {
        console.log('  SKIP: Insufficient data for placeholder check');
    }

    // Check 3: Query counts are consistent
    console.log('\nCheck 3: Query count consistency...');
    const queriesPassed = await queryPrometheus('dashstream_quality_monitor_queries_passed_total');
    // queries_failed_total is a CounterVec labeled by category; sum across all categories.
    const queriesFailed = await queryPrometheus('sum(dashstream_quality_monitor_queries_failed_total)');

    if (queryCount === null) {
        result.checks.push({
            name: 'Query Count Consistency',
            passed: false,
            expected: 'dashstream_quality_monitor_queries_total present',
            actual: 'null',
            message: 'Missing dashstream_quality_monitor_queries_total in Prometheus',
        });
        result.passed = false;
        console.log('  FAIL: Missing dashstream_quality_monitor_queries_total');
    } else if (queriesPassed === null) {
        result.checks.push({
            name: 'Query Count Consistency',
            passed: false,
            expected: 'dashstream_quality_monitor_queries_passed_total present',
            actual: 'null',
            message: 'Missing dashstream_quality_monitor_queries_passed_total in Prometheus',
        });
        result.passed = false;
        console.log('  FAIL: Missing dashstream_quality_monitor_queries_passed_total');
    } else if (queriesFailed === null) {
        result.checks.push({
            name: 'Query Count Consistency',
            passed: false,
            expected: 'dashstream_quality_monitor_queries_failed_total present (sum across category)',
            actual: 'null',
            message: 'Missing dashstream_quality_monitor_queries_failed_total in Prometheus',
        });
        result.passed = false;
        console.log('  FAIL: Missing dashstream_quality_monitor_queries_failed_total');
    } else {
        const expectedTotal = (queriesPassed || 0) + (queriesFailed || 0);
        // Allow for some timing differences
        const isConsistent = Math.abs(queryCount - expectedTotal) <= 2;
        result.checks.push({
            name: 'Query Count Consistency',
            passed: isConsistent,
            expected: `passed(${queriesPassed}) + failed(${queriesFailed || 0}) ≈ total`,
            actual: `total = ${queryCount}`,
            message: isConsistent
                ? `Query counts are consistent`
                : `FAIL: Total queries (${queryCount}) != passed (${queriesPassed}) + failed (${queriesFailed || 0})`,
        });
        if (!isConsistent) result.passed = false;
        console.log(`  ${isConsistent ? 'PASS' : 'FAIL'}: ${queriesPassed} passed + ${queriesFailed || 0} failed = ${queryCount} total`);
    }

    // Check 4: Grafana can query the same values as Prometheus
    console.log('\nCheck 4: Grafana-Prometheus consistency...');
    const grafanaQualityScore = await queryGrafana('dashstream_quality_monitor_quality_score');
    if (grafanaQualityScore !== null && qualityScore !== null) {
        // Allow small floating point differences
        const isConsistent = Math.abs(grafanaQualityScore - qualityScore) < 0.01;
        result.checks.push({
            name: 'Grafana-Prometheus Consistency',
            passed: isConsistent,
            expected: `Prometheus: ${qualityScore}`,
            actual: `Grafana: ${grafanaQualityScore}`,
            message: isConsistent
                ? `Grafana and Prometheus show same value`
                : `FAIL: Values differ - possible datasource issue`,
        });
        if (!isConsistent) result.passed = false;
        console.log(`  ${isConsistent ? 'PASS' : 'FAIL'}: Prometheus=${qualityScore}, Grafana=${grafanaQualityScore}`);
    } else {
        result.checks.push({
            name: 'Grafana-Prometheus Consistency',
            passed: false,
            expected: 'Grafana and Prometheus both return a value',
            actual: `Prometheus: ${qualityScore ?? 'null'}, Grafana: ${grafanaQualityScore ?? 'null'}`,
            message: 'Could not query Grafana and Prometheus consistently',
        });
        result.passed = false;
        console.log('  FAIL: Could not query Grafana and Prometheus consistently');
    }

    // Check 5: Verify rate metrics are non-negative
    console.log('\nCheck 5: Rate metrics are non-negative...');
    const rate = await queryPrometheus('rate(dashstream_quality_monitor_queries_total[5m])');
    if (rate !== null) {
        const isValid = rate >= 0;
        result.checks.push({
            name: 'Rate Non-Negative',
            passed: isValid,
            expected: '>= 0',
            actual: rate.toString(),
            message: isValid
                ? `Query rate ${rate.toFixed(4)}/s is valid`
                : `FAIL: Negative rate ${rate} indicates counter reset or bug`,
        });
        if (!isValid) result.passed = false;
        console.log(`  ${isValid ? 'PASS' : 'FAIL'}: Query rate = ${rate.toFixed(4)}/s`);
    } else {
        console.log('  SKIP: Rate metric not available');
    }

    // Summary
    console.log('\n=== Acceptance Test Summary ===');
    console.log(`Overall: ${result.passed ? 'PASSED' : 'FAILED'}`);
    console.log(`Checks: ${result.checks.filter(c => c.passed).length}/${result.checks.length} passed`);

    if (!result.passed) {
        console.log('\nFailed checks:');
        result.checks.filter(c => !c.passed).forEach(c => {
            console.log(`  - ${c.name}: ${c.message}`);
        });
    }

    return result;
}

// M-105: Main entry point with --emit support
async function main(): Promise<void> {
    const shouldEmit = process.argv.includes('--emit');

    if (shouldEmit) {
        const emitSuccess = await emitTestEvents();
        if (!emitSuccess) {
            console.error('Failed to emit test events. Continuing with validation...');
        }
    }

    const result = await runAcceptanceTests();
    process.exit(result.passed ? 0 : 1);
}

// Run if called directly
main().catch(err => {
    console.error('Error running acceptance tests:', err);
    process.exit(2);
});
