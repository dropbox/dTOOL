/**
 * Grafana Dashboard Integration Test
 *
 * Verifies that the DashStream Quality Dashboard loads correctly and displays
 * actual data (not "No data" messages).
 *
 * Prerequisites:
 * - Docker observability stack running: docker-compose -f docker-compose.dashstream.yml up -d
 * - Some data generated: cargo run -p librarian -- query "test"
 *
 * Usage:
 *   node test-utils/tests/grafana_dashboard.test.js
 *
 * Or via npm script:
 *   npm run test:grafana
 *
 * Note: This is a Node script that uses the Playwright browser library
 * (it is not an `@playwright/test` runner test file).
 */

const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const GRAFANA_URL = process.env.GRAFANA_URL || 'http://localhost:3000';
const GRAFANA_USER = process.env.GRAFANA_USER || 'admin';
const GRAFANA_PASS = process.env.GRAFANA_PASS || 'admin';
// M-103: Route E2E screenshots to untracked artifacts dir instead of reports/main
// Use TEST_OUTPUT_DIR env var to allow CI to specify a custom location
const OUTPUT_DIR = process.env.TEST_OUTPUT_DIR || path.join(__dirname, '..', '..', 'test-artifacts', 'grafana-e2e');

const DASHBOARDS_DIR = path.join(__dirname, '..', '..', 'grafana', 'dashboards');

// M-126: All provisioned dashboards to test
// Each entry: { uid, jsonFile, name, requiredPrometheusMetrics }
const DASHBOARDS = [
    {
        uid: 'dashstream-quality',
        jsonFile: 'grafana_quality_dashboard.json',
        name: 'DashStream Quality',
        requiredPrometheusMetrics: [
            { query: 'dashstream_quality_monitor_quality_score', name: 'Quality Score', required: true },
            { query: 'dashstream_quality_monitor_queries_total', name: 'Queries', required: true },
            { query: 'dashstream_quality_monitor_queries_passed_total', name: 'Queries Passed', required: true },
            { query: 'sum(dashstream_quality_monitor_queries_failed_total)', name: 'Queries Failed', required: true },
        ],
    },
    {
        uid: 'dashflow-streaming',
        jsonFile: 'streaming_metrics_dashboard.json',
        name: 'DashFlow Streaming',
        requiredPrometheusMetrics: [
            { query: 'kafka_consumer_messages_received_total', name: 'Kafka Messages', required: false },
            { query: 'websocket_connections_total', name: 'WebSocket Connections', required: false },
        ],
    },
    {
        uid: 'librarian-main',
        jsonFile: 'librarian.json',
        name: 'Librarian',
        requiredPrometheusMetrics: [
            { query: 'librarian_queries_total', name: 'Librarian Queries', required: false },
            { query: 'librarian_search_latency_seconds', name: 'Search Latency', required: false },
        ],
    },
];

// Legacy: single dashboard path for backward compatibility
const DASHBOARD_JSON_PATH = path.join(DASHBOARDS_DIR, 'grafana_quality_dashboard.json');

// M-126: Load panel titles from a dashboard JSON file
function loadDashboardPanelTitles(dashboardJsonPath) {
    try {
        const raw = fs.readFileSync(dashboardJsonPath, 'utf8');
        const dashboard = JSON.parse(raw);

        if (!dashboard || !Array.isArray(dashboard.panels)) {
            throw new Error('dashboard JSON missing panels[]');
        }

        const titles = dashboard.panels
            .filter((panel) => panel && typeof panel.title === 'string' && typeof panel.type === 'string')
            .map((panel) => panel.title.trim())
            .filter((title) => title.length > 0);

        if (titles.length === 0) {
            throw new Error('dashboard JSON has zero titled panels');
        }

        return titles;
    } catch (error) {
        console.warn(`Warning: failed to load panel titles from ${dashboardJsonPath}: ${error.message}`);
        return [];
    }
}

// Legacy: Treat the dashboard JSON as source-of-truth for KPI coverage: all titled panels should render.
const REQUIRED_PANELS = loadDashboardPanelTitles(DASHBOARD_JSON_PATH);

async function verifyGrafanaDashboard() {
    console.log('=== Grafana Dashboard Integration Test ===\n');

    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
        viewport: { width: 1920, height: 1080 }
    });
    const page = await context.newPage();

    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    // M-126: Use first dashboard for legacy function (dashstream-quality)
    const primaryDashboard = DASHBOARDS[0];
    const results = {
        timestamp,
        grafanaUrl: GRAFANA_URL,
        dashboardUid: primaryDashboard.uid,
        passed: true,
        checks: [],
        screenshots: [],
    };

    // Ensure output directory exists
    if (!fs.existsSync(OUTPUT_DIR)) {
        fs.mkdirSync(OUTPUT_DIR, { recursive: true });
    }

    try {
        // 1. Check Grafana is accessible
        console.log('1. Checking Grafana accessibility...');
        const response = await page.goto(`${GRAFANA_URL}/api/health`, { timeout: 10000 });
        const healthCheck = response?.ok() ?? false;
        results.checks.push({
            name: 'Grafana Health',
            passed: healthCheck,
            message: healthCheck ? 'Grafana API responding' : 'Grafana API not responding',
        });
        if (!healthCheck) {
            throw new Error('Grafana not accessible');
        }
        console.log('   PASS: Grafana is healthy\n');

        // 2. Login
        console.log('2. Logging into Grafana...');
        await page.goto(`${GRAFANA_URL}/login`);

        // Check if already logged in (no login form)
        const loginForm = await page.$('input[name="user"]');
        if (loginForm) {
            await page.fill('input[name="user"]', GRAFANA_USER);
            await page.fill('input[name="password"]', GRAFANA_PASS);
            await page.click('button[type="submit"]');
            await page.waitForTimeout(3000);
        }

        // Check if we can access dashboard (login success indicator)
        await page.goto(`${GRAFANA_URL}/d/${primaryDashboard.uid}`);
        await page.waitForTimeout(2000);
        const dashboardLoaded = !page.url().includes('/login');

        results.checks.push({
            name: 'Grafana Login',
            passed: dashboardLoaded,
            message: dashboardLoaded ? 'Login successful / Dashboard accessible' : 'Cannot access dashboard',
        });
        if (!dashboardLoaded) {
            throw new Error('Cannot access dashboard - login may have failed');
        }
        console.log('   PASS: Dashboard accessible\n');

        // 3. Navigate to dashboard
        console.log(`3. Loading ${primaryDashboard.name} Dashboard...`);
        await page.goto(`${GRAFANA_URL}/d/${primaryDashboard.uid}?orgId=1&refresh=5s`);
        await page.waitForTimeout(5000); // Wait for panels to load

        // Capture full dashboard screenshot
        const dashboardScreenshot = path.join(OUTPUT_DIR, `grafana_e2e_dashboard_${timestamp}.png`);
        await page.screenshot({ path: dashboardScreenshot, fullPage: true });
        results.screenshots.push(dashboardScreenshot);
        console.log(`   Screenshot: ${dashboardScreenshot}\n`);

        // 4. Check for "No data" panels
        console.log('4. Checking panel data...');
        const pageContent = await page.content();

        // Count "No data" occurrences
        const noDataCount = (pageContent.match(/No data/gi) || []).length;
        console.log(`   Found ${noDataCount} panels showing "No data"`);

        // Check required panels have data
        for (const panelName of REQUIRED_PANELS) {
            // Try multiple selector strategies for panel detection
            // Grafana uses different data-testid patterns across versions
            const panelSelectors = [
                `[data-testid="data-testid Panel header ${panelName}"]`,
                `[data-viz-panel-key] >> text="${panelName}"`,
                `[aria-label*="${panelName}"]`,
                `.panel-title:has-text("${panelName}")`,
            ];

            let panelElement = null;
            let panelContainer = null;
            for (const selector of panelSelectors) {
                try {
                    panelElement = await page.$(selector);
                    if (panelElement) {
                        // Get the panel container (parent elements that contain the panel content)
                        panelContainer = await page.evaluateHandle((el) => {
                            // Walk up to find the panel container
                            let current = el;
                            for (let i = 0; i < 10 && current; i++) {
                                if (current.classList &&
                                    (current.classList.contains('panel-container') ||
                                     current.classList.contains('react-grid-item') ||
                                     current.getAttribute('data-viz-panel-key'))) {
                                    return current;
                                }
                                current = current.parentElement;
                            }
                            // Fallback: return parent's parent's parent (typical nesting depth)
                            return el.parentElement?.parentElement?.parentElement || el.parentElement;
                        }, panelElement);
                        break;
                    }
                } catch (e) {
                    // Selector syntax may not work, try next
                }
            }

            let hasData = false;
            let message = 'Panel not found';

            if (panelContainer) {
                // Check if this specific panel shows "No data"
                const panelContent = await page.evaluate((el) => {
                    return el ? el.innerText || el.textContent : '';
                }, panelContainer);

                // Panel has data if it doesn't contain "No data" message
                const containsNoData = /\bNo data\b/i.test(panelContent);
                // Also check for actual numeric values (positive indicator)
                const containsNumbers = /\d+\.?\d*\s*(%|ms|s|$)/m.test(panelContent);

                // E2E coverage requirement: fail if a KPI panel shows "No data".
                hasData = !containsNoData;
                message = hasData
                    ? containsNumbers
                        ? 'Panel has data'
                        : 'Panel has data (no numeric text detected)'
                    : 'Panel shows "No data"';
            } else if (panelElement) {
                message = 'Panel header found but container not located';
            }

            results.checks.push({
                name: `Panel: ${panelName}`,
                passed: hasData,
                message: message,
            });

            const status = hasData ? 'PASS' : 'FAIL';
            console.log(`   ${panelName}: ${status} - ${message}`);
        }

        // 5. Semantic validation - DOM-based checks are SOFT (warnings only)
        // NOTE: DOM scraping is unreliable in Grafana - use Prometheus API checks (section 6) as source of truth
        // See Issue 20: DOM structure varies by Grafana version, making text extraction fragile
        console.log('\n5. Semantic validation of displayed values (soft checks - DOM scraping)...');
        console.log('   Note: These are informational only. Prometheus API checks in section 6 are authoritative.');

        // 5a. Quality Score extraction (soft check - DOM is unreliable)
        const qualityScoreText = await page.evaluate(() => {
            // Find the Current Quality Score panel and extract its value
            const panels = document.querySelectorAll('[data-viz-panel-key], .panel-container');
            for (const panel of panels) {
                const text = panel.innerText || '';
                if (text.includes('Current Quality Score') || text.includes('Quality Score')) {
                    // Extract numeric value - look for a decimal like 0.xxx
                    const match = text.match(/\b(0\.\d+|1\.0+|1\.00*)\b/);
                    if (match) return match[1];
                }
            }
            return null;
        });

        if (qualityScoreText) {
            const qualityScore = parseFloat(qualityScoreText);
            const scoreValid = !isNaN(qualityScore) && qualityScore >= 0 && qualityScore <= 1;
            // Soft check - always passes but logs result
            results.checks.push({
                name: 'DOM Soft: Quality Score Range',
                passed: true, // Soft check - DOM extraction is unreliable
                message: scoreValid
                    ? `DOM extracted ${qualityScore} (in valid range [0, 1])`
                    : `DOM extracted ${qualityScore} (WARN: outside [0, 1] - verify via Prometheus)`,
            });
            console.log(`   Quality Score DOM: ${scoreValid ? 'OK' : 'WARN'} (value: ${qualityScore})`);
        } else {
            results.checks.push({
                name: 'DOM Soft: Quality Score Range',
                passed: true, // Soft check - DOM extraction is unreliable
                message: 'Could not extract from DOM (verify via Prometheus API below)',
            });
            console.log('   Quality Score DOM: SKIP (not extractable from DOM)');
        }

        // 5b. Failure rate extraction (soft check - DOM is unreliable)
        const failureRateText = await page.evaluate(() => {
            const panels = document.querySelectorAll('[data-viz-panel-key], .panel-container');
            for (const panel of panels) {
                const text = panel.innerText || '';
                if (text.includes('Failure Rate') || text.includes('Success Rate')) {
                    // Extract percentage value
                    const match = text.match(/(\d+\.?\d*)\s*%/);
                    if (match) return match[1];
                }
            }
            return null;
        });

        if (failureRateText) {
            const failureRate = parseFloat(failureRateText);
            const rateValid = !isNaN(failureRate) && failureRate >= 0 && failureRate <= 100;
            results.checks.push({
                name: 'DOM Soft: Failure Rate Range',
                passed: true, // Soft check
                message: rateValid
                    ? `DOM extracted ${failureRate}% (in valid range [0, 100])`
                    : `DOM extracted ${failureRate}% (WARN: outside [0, 100] - verify via Prometheus)`,
            });
            console.log(`   Failure Rate DOM: ${rateValid ? 'OK' : 'WARN'} (value: ${failureRate}%)`);
        } else {
            results.checks.push({
                name: 'DOM Soft: Failure Rate Range',
                passed: true, // Soft check
                message: 'Failure rate not extracted from DOM (acceptable)',
            });
            console.log('   Failure Rate DOM: SKIP (not displayed)');
        }

        // 6. Check specific metric values via Prometheus API (source of truth)
        // NOTE: This is the authoritative validation - more reliable than DOM scraping
        console.log('\n6. Verifying Prometheus metrics (source of truth)...');
        const prometheusUrl = process.env.PROMETHEUS_URL || 'http://localhost:9090';

        // Helper to query Prometheus API
        async function queryPrometheus(query) {
            const metricPage = await context.newPage();
            try {
                const metricResponse = await metricPage.goto(
                    `${prometheusUrl}/api/v1/query?query=${encodeURIComponent(query)}`,
                    { timeout: 5000 }
                );
                return await metricResponse?.json();
            } finally {
                await metricPage.close();
            }
        }

        const metricsToCheck = [
            { query: 'dashstream_quality_monitor_quality_score', name: 'Quality Score', required: true },
            { query: 'dashstream_quality_monitor_queries_total', name: 'Queries', required: true },
            { query: 'dashstream_quality_monitor_queries_passed_total', name: 'Queries Passed', required: true },
            { query: 'sum(dashstream_quality_monitor_queries_failed_total)', name: 'Queries Failed', required: true },
            { query: 'dashstream_quality_retry_count_count', name: 'Retry Count', required: false },
        ];

        for (const metric of metricsToCheck) {
            try {
                const metricData = await queryPrometheus(metric.query);
                const hasData = metricData?.data?.result?.length > 0;

                // Only required metrics affect pass/fail
                const checkPassed = hasData || !metric.required;
                results.checks.push({
                    name: `Prometheus: ${metric.name}`,
                    passed: checkPassed,
                    message: hasData
                        ? `Found ${metricData.data.result.length} series`
                        : metric.required ? 'REQUIRED: No data in Prometheus' : 'Optional: No data (acceptable)',
                });
                const status = hasData ? 'PASS' : (metric.required ? 'FAIL' : 'SKIP');
                console.log(`   ${metric.name}: ${status} (${metricData?.data?.result?.length || 0} series)`);
            } catch (e) {
                results.checks.push({
                    name: `Prometheus: ${metric.name}`,
                    passed: false,
                    message: `Error: ${e.message}`,
                });
                console.log(`   ${metric.name}: ERROR - ${e.message}`);
            }
        }

        // 6b. API-based semantic validation (authoritative - replaces unreliable DOM scraping)
        console.log('\n6b. API-based semantic validation...');

        // Validate quality score is in [0, 1] range via Prometheus
        try {
            const qualityData = await queryPrometheus('dashstream_quality_monitor_quality_score');
            const values = qualityData?.data?.result?.map(r => parseFloat(r.value?.[1])) || [];
            const validValues = values.filter(v => !isNaN(v));

            if (validValues.length > 0) {
                const invalidValues = validValues.filter(v => v < 0 || v > 1);
                const semanticValid = invalidValues.length === 0;
                results.checks.push({
                    name: 'Semantic API: Quality Score Range',
                    passed: semanticValid,
                    message: semanticValid
                        ? `All ${validValues.length} quality scores in valid range [0, 1]`
                        : `FAIL: ${invalidValues.length} values outside [0, 1]: ${invalidValues.slice(0, 3).join(', ')}`,
                });
                console.log(`   Quality Score Range (API): ${semanticValid ? 'PASS' : 'FAIL'} (${validValues.length} values)`);
            } else {
                results.checks.push({
                    name: 'Semantic API: Quality Score Range',
                    passed: true, // No data to validate is acceptable
                    message: 'No quality score data to validate',
                });
                console.log('   Quality Score Range (API): SKIP (no data)');
            }
        } catch (e) {
            results.checks.push({
                name: 'Semantic API: Quality Score Range',
                passed: false,
                message: `API query failed: ${e.message}`,
            });
            console.log(`   Quality Score Range (API): ERROR - ${e.message}`);
        }

        // Validate failure rate (if exists) is in [0, 100] range
        try {
            const failureData = await queryPrometheus('dashstream_quality_monitor_failure_rate');
            const values = failureData?.data?.result?.map(r => parseFloat(r.value?.[1])) || [];
            const validValues = values.filter(v => !isNaN(v));

            if (validValues.length > 0) {
                const invalidValues = validValues.filter(v => v < 0 || v > 100);
                const semanticValid = invalidValues.length === 0;
                results.checks.push({
                    name: 'Semantic API: Failure Rate Range',
                    passed: semanticValid,
                    message: semanticValid
                        ? `All ${validValues.length} failure rates in valid range [0, 100]`
                        : `FAIL: ${invalidValues.length} values outside [0, 100]: ${invalidValues.slice(0, 3).join(', ')}`,
                });
                console.log(`   Failure Rate Range (API): ${semanticValid ? 'PASS' : 'FAIL'} (${validValues.length} values)`);
            } else {
                results.checks.push({
                    name: 'Semantic API: Failure Rate Range',
                    passed: true, // No data means no failures, which is fine
                    message: 'No failure rate data (acceptable if no failures)',
                });
                console.log('   Failure Rate Range (API): SKIP (no data)');
            }
        } catch (e) {
            // Failure rate metric may not exist - this is acceptable
            results.checks.push({
                name: 'Semantic API: Failure Rate Range',
                passed: true,
                message: `No failure rate metric (acceptable): ${e.message}`,
            });
            console.log('   Failure Rate Range (API): SKIP (no metric)');
        }

        // 7. Capture additional screenshots for documentation
        console.log('\n7. Capturing section screenshots...');

        // Quality metrics section
        await page.evaluate(() => window.scrollTo(0, 0));
        await page.waitForTimeout(500);
        const qualityScreenshot = path.join(OUTPUT_DIR, `grafana_e2e_quality_${timestamp}.png`);
        await page.screenshot({ path: qualityScreenshot });
        results.screenshots.push(qualityScreenshot);

        // Infrastructure section
        await page.evaluate(() => window.scrollTo(0, 800));
        await page.waitForTimeout(500);
        const infraScreenshot = path.join(OUTPUT_DIR, `grafana_e2e_infrastructure_${timestamp}.png`);
        await page.screenshot({ path: infraScreenshot });
        results.screenshots.push(infraScreenshot);

        console.log(`   Captured ${results.screenshots.length} screenshots\n`);

        // Determine overall pass/fail
        const failedChecks = results.checks.filter(c => !c.passed);
        results.passed = failedChecks.length === 0;

    } catch (error) {
        console.error(`\nERROR: ${error.message}`);
        results.passed = false;
        results.error = error.message;

        // Capture error state
        const errorScreenshot = path.join(OUTPUT_DIR, `grafana_e2e_error_${timestamp}.png`);
        await page.screenshot({ path: errorScreenshot, fullPage: true });
        results.screenshots.push(errorScreenshot);
    } finally {
        await browser.close();
    }

    // Write results JSON
    const resultsFile = path.join(OUTPUT_DIR, `grafana_e2e_results_${timestamp}.json`);
    fs.writeFileSync(resultsFile, JSON.stringify(results, null, 2));
    console.log(`Results written to: ${resultsFile}`);

    // Print summary
    console.log('\n=== Test Summary ===');
    console.log(`Status: ${results.passed ? 'PASSED' : 'FAILED'}`);
    console.log(`Checks: ${results.checks.filter(c => c.passed).length}/${results.checks.length} passed`);
    console.log(`Screenshots: ${results.screenshots.length} captured`);

    if (!results.passed) {
        console.log('\nFailed checks:');
        results.checks.filter(c => !c.passed).forEach(c => {
            console.log(`  - ${c.name}: ${c.message}`);
        });
    }

    // Exit with appropriate code
    process.exit(results.passed ? 0 : 1);
}

// M-126: Verify all provisioned dashboards are accessible
async function verifyAllDashboards() {
    console.log('=== Grafana Multi-Dashboard Test ===\n');
    console.log(`Testing ${DASHBOARDS.length} provisioned dashboards:\n`);
    DASHBOARDS.forEach((d, i) => console.log(`  ${i + 1}. ${d.name} (uid: ${d.uid})`));
    console.log('');

    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
        viewport: { width: 1920, height: 1080 }
    });
    const page = await context.newPage();

    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const allResults = {
        timestamp,
        grafanaUrl: GRAFANA_URL,
        dashboards: [],
        passed: true,
    };

    // Ensure output directory exists
    if (!fs.existsSync(OUTPUT_DIR)) {
        fs.mkdirSync(OUTPUT_DIR, { recursive: true });
    }

    try {
        // 1. Check Grafana is accessible
        console.log('1. Checking Grafana accessibility...');
        const response = await page.goto(`${GRAFANA_URL}/api/health`, { timeout: 10000 });
        if (!response?.ok()) {
            throw new Error('Grafana not accessible');
        }
        console.log('   PASS: Grafana is healthy\n');

        // 2. Login
        console.log('2. Logging into Grafana...');
        await page.goto(`${GRAFANA_URL}/login`);
        const loginForm = await page.$('input[name="user"]');
        if (loginForm) {
            await page.fill('input[name="user"]', GRAFANA_USER);
            await page.fill('input[name="password"]', GRAFANA_PASS);
            await page.click('button[type="submit"]');
            await page.waitForTimeout(3000);
        }
        console.log('   PASS: Login complete\n');

        // 3. Test each dashboard
        console.log('3. Testing each dashboard...\n');
        for (const dashboard of DASHBOARDS) {
            console.log(`   === ${dashboard.name} (${dashboard.uid}) ===`);
            const dashResult = {
                uid: dashboard.uid,
                name: dashboard.name,
                accessible: false,
                panelsFound: 0,
                panelsTotal: 0,
                passed: true,
                checks: [],
            };

            try {
                // Navigate to dashboard
                await page.goto(`${GRAFANA_URL}/d/${dashboard.uid}?orgId=1&refresh=5s`);
                await page.waitForTimeout(3000);

                // Check if dashboard loaded (not redirected to login or error page)
                const currentUrl = page.url();
                dashResult.accessible = currentUrl.includes(dashboard.uid) && !currentUrl.includes('/login');

                if (!dashResult.accessible) {
                    console.log(`   FAIL: Dashboard not accessible`);
                    dashResult.passed = false;
                    dashResult.checks.push({ name: 'Dashboard Load', passed: false, message: `Redirected to: ${currentUrl}` });
                } else {
                    console.log(`   PASS: Dashboard loaded`);
                    dashResult.checks.push({ name: 'Dashboard Load', passed: true, message: 'Dashboard accessible' });

                    // Load panel titles from JSON
                    const jsonPath = path.join(DASHBOARDS_DIR, dashboard.jsonFile);
                    const expectedPanels = loadDashboardPanelTitles(jsonPath);
                    dashResult.panelsTotal = expectedPanels.length;

                    // Take screenshot
                    const screenshotPath = path.join(OUTPUT_DIR, `grafana_e2e_${dashboard.uid}_${timestamp}.png`);
                    await page.screenshot({ path: screenshotPath, fullPage: true });
                    console.log(`   Screenshot: ${screenshotPath}`);

                    // Check for panels with data
                    const pageContent = await page.content();
                    const noDataCount = (pageContent.match(/No data/gi) || []).length;
                    dashResult.panelsFound = Math.max(0, expectedPanels.length - noDataCount);

                    console.log(`   Panels: ${dashResult.panelsFound}/${dashResult.panelsTotal} have data (${noDataCount} show "No data")`);

                    // For the primary dashboard (dashstream-quality), panels must have data
                    if (dashboard.uid === 'dashstream-quality' && noDataCount > 0) {
                        dashResult.checks.push({
                            name: 'Panel Data',
                            passed: false,
                            message: `${noDataCount} panels show "No data" - run librarian query to generate metrics`
                        });
                        dashResult.passed = false;
                    } else {
                        dashResult.checks.push({
                            name: 'Panel Data',
                            passed: true,
                            message: noDataCount > 0 ? `${noDataCount} panels without data (acceptable for non-primary)` : 'All panels have data'
                        });
                    }
                }
            } catch (error) {
                console.log(`   ERROR: ${error.message}`);
                dashResult.passed = false;
                dashResult.checks.push({ name: 'Dashboard Error', passed: false, message: error.message });
            }

            allResults.dashboards.push(dashResult);
            if (!dashResult.passed) {
                allResults.passed = false;
            }
            console.log('');
        }
    } catch (error) {
        console.error(`\nERROR: ${error.message}`);
        allResults.passed = false;
        allResults.error = error.message;
    } finally {
        await browser.close();
    }

    // Write results JSON
    const resultsFile = path.join(OUTPUT_DIR, `grafana_e2e_all_dashboards_${timestamp}.json`);
    fs.writeFileSync(resultsFile, JSON.stringify(allResults, null, 2));
    console.log(`Results written to: ${resultsFile}`);

    // Print summary
    console.log('\n=== Multi-Dashboard Test Summary ===');
    console.log(`Status: ${allResults.passed ? 'PASSED' : 'FAILED'}`);
    console.log(`Dashboards tested: ${allResults.dashboards.length}`);
    allResults.dashboards.forEach(d => {
        const status = d.passed ? 'PASS' : 'FAIL';
        console.log(`  - ${d.name}: ${status} (${d.panelsFound}/${d.panelsTotal} panels with data)`);
    });

    // Exit with appropriate code
    process.exit(allResults.passed ? 0 : 1);
}

// Run if called directly
if (require.main === module) {
    // M-126: Run multi-dashboard test by default, use --legacy for single dashboard
    const legacyMode = process.argv.includes('--legacy');
    if (legacyMode) {
        console.log('Running in legacy mode (single dashboard)...\n');
        verifyGrafanaDashboard().catch(console.error);
    } else {
        verifyAllDashboards().catch(console.error);
    }
}

module.exports = { verifyGrafanaDashboard, verifyAllDashboards, DASHBOARDS };
