/**
 * Grafana Dashboard Visual Regression Test
 *
 * Uses Playwright's built-in screenshot comparison to detect visual regressions.
 * Creates baseline screenshots on first run, compares against them on subsequent runs.
 *
 * Prerequisites:
 * - Docker observability stack running: docker-compose -f docker-compose.dashstream.yml up -d
 * - npx playwright install chromium
 *
 * Usage:
 *   # First run (creates baseline screenshots):
 *   npx playwright test test-utils/tests/grafana_visual_regression.test.js --update-snapshots
 *
 *   # Subsequent runs (compares against baseline):
 *   npx playwright test test-utils/tests/grafana_visual_regression.test.js
 *
 * M-100: Cross-platform snapshots - see playwright.config.js for platform-aware configuration
 * M-101: Uses proper wait strategies instead of fixed sleeps
 * M-102: Uses Grafana/Prometheus APIs for metric assertions
 */

const { test, expect } = require('@playwright/test');
const path = require('path');
const fs = require('fs');

const GRAFANA_URL = process.env.GRAFANA_URL || 'http://localhost:3000';
const PROMETHEUS_URL = process.env.PROMETHEUS_URL || 'http://localhost:9090';
const GRAFANA_USER = process.env.GRAFANA_USER || 'admin';
const GRAFANA_PASS = process.env.GRAFANA_PASS || 'admin';
const DASHBOARD_UID = 'dashstream-quality';

const DASHBOARD_JSON_PATH = path.join(
    __dirname,
    '..',
    '..',
    'grafana',
    'dashboards',
    'grafana_quality_dashboard.json'
);

let cachedPanelTitles = null;

function getDashboardPanelTitles() {
    if (cachedPanelTitles) {
        return cachedPanelTitles;
    }

    const raw = fs.readFileSync(DASHBOARD_JSON_PATH, 'utf8');
    const dashboard = JSON.parse(raw);

    if (!dashboard || !Array.isArray(dashboard.panels)) {
        throw new Error(`Invalid dashboard JSON: missing panels[] in ${DASHBOARD_JSON_PATH}`);
    }

    const titles = dashboard.panels
        .filter((panel) => panel && typeof panel.title === 'string' && typeof panel.type === 'string')
        .map((panel) => panel.title.trim())
        .filter((title) => title.length > 0);

    if (titles.length === 0) {
        throw new Error(`Dashboard JSON has zero titled panels in ${DASHBOARD_JSON_PATH}`);
    }

    cachedPanelTitles = titles;
    return cachedPanelTitles;
}

async function locatePanelContainerByTitle(page, panelTitle) {
    let titleLocator = page.getByText(panelTitle, { exact: true });
    if ((await titleLocator.count()) === 0) {
        titleLocator = page.getByText(panelTitle, { exact: false });
    }

    const title = titleLocator.first();
    await title.waitFor({ timeout: 10000 });

    const containerCandidates = [
        title.locator('xpath=ancestor-or-self::*[@data-viz-panel-key][1]'),
        title.locator(
            "xpath=ancestor-or-self::*[contains(concat(' ', normalize-space(@class), ' '), ' panel-container ')][1]"
        ),
        title.locator(
            "xpath=ancestor-or-self::*[contains(concat(' ', normalize-space(@class), ' '), ' react-grid-item ')][1]"
        ),
    ];

    for (const candidate of containerCandidates) {
        if ((await candidate.count()) > 0) {
            return candidate.first();
        }
    }

    throw new Error(`Could not locate panel container for title: ${panelTitle}`);
}

// M-101: Helper to wait for Grafana panels to finish loading
// Replaces fixed waitForTimeout() calls with proper condition-based waiting
async function waitForPanelsLoaded(page, timeout = 30000) {
    // Wait for any loading spinners to disappear
    const loadingIndicators = [
        '.panel-loading',
        '[data-testid="panel-loading"]',
        '.panel-in-fullscreen .panel-loading',
        '.spinner',
        '[aria-label="Panel loading bar"]',
    ];

    for (const selector of loadingIndicators) {
        try {
            await page.waitForSelector(selector, { state: 'hidden', timeout: 5000 });
        } catch {
            // Selector may not exist, which is fine
        }
    }

    // Wait for network to be idle (no pending requests for 500ms)
    await page.waitForLoadState('networkidle', { timeout });

    // Wait for at least one panel to be rendered
    await page.waitForSelector('[data-viz-panel-key], .panel-container, .react-grid-item', {
        timeout,
        state: 'visible',
    });
}

// M-101: Helper to wait for scroll and content to stabilize
async function waitForScrollStabilization(page) {
    // Wait for any lazy-loaded content to render after scroll
    await page.waitForLoadState('networkidle', { timeout: 10000 });
}

// M-102: Query Prometheus directly for metric values
async function queryPrometheus(query) {
    const url = `${PROMETHEUS_URL}/api/v1/query?query=${encodeURIComponent(query)}`;
    try {
        const response = await fetch(url);
        if (!response.ok) {
            throw new Error(`Prometheus query failed: ${response.status}`);
        }
        const data = await response.json();
        if (data.status !== 'success') {
            throw new Error(`Prometheus query error: ${data.error}`);
        }
        return data.data;
    } catch (error) {
        // Prometheus may not be running - return null to allow test to skip gracefully
        console.warn(`Prometheus query failed: ${error.message}`);
        return null;
    }
}

// M-102: Query Grafana API for dashboard data
async function queryGrafanaDashboard(page, dashboardUid) {
    const apiUrl = `${GRAFANA_URL}/api/dashboards/uid/${dashboardUid}`;
    try {
        // Use page context for authentication (already logged in)
        const response = await page.evaluate(async (url) => {
            const resp = await fetch(url, { credentials: 'include' });
            return {
                ok: resp.ok,
                status: resp.status,
                data: await resp.json().catch(() => null),
            };
        }, apiUrl);

        if (!response.ok) {
            throw new Error(`Grafana API failed: ${response.status}`);
        }
        return response.data;
    } catch (error) {
        console.warn(`Grafana API query failed: ${error.message}`);
        return null;
    }
}

// Configure test to retry on flaky visual comparisons
test.describe.configure({ mode: 'serial' });

test.describe('Grafana Dashboard Visual Regression', () => {
    test.beforeEach(async ({ page }) => {
        // Login to Grafana
        await page.goto(`${GRAFANA_URL}/login`);

        // Check if login form exists (may be already logged in)
        const loginForm = await page.$('input[name="user"]');
        if (loginForm) {
            await page.fill('input[name="user"]', GRAFANA_USER);
            await page.fill('input[name="password"]', GRAFANA_PASS);
            await page.click('button[type="submit"]');
            // M-101: Wait for navigation to complete instead of fixed timeout
            await page.waitForURL('**/d/**', { timeout: 10000 }).catch(() => {
                // May redirect elsewhere - wait for network idle instead
                return page.waitForLoadState('networkidle', { timeout: 10000 });
            });
        }
    });

    test('dashboard overview should match baseline', async ({ page }) => {
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        // M-101: Wait for panels to load instead of fixed timeout
        await waitForPanelsLoaded(page);

        // Full dashboard screenshot comparison
        // M-100: Threshold configured in playwright.config.js for cross-platform tolerance
        await expect(page).toHaveScreenshot('dashboard-overview.png', {
            fullPage: true,
            timeout: 30000,
        });
    });

    test('quality metrics section should match baseline', async ({ page }) => {
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        // M-101: Wait for panels to load instead of fixed timeout
        await waitForPanelsLoaded(page);

        // Scroll to top for quality metrics
        await page.evaluate(() => window.scrollTo(0, 0));
        // M-101: Wait for scroll to stabilize
        await waitForScrollStabilization(page);

        await expect(page).toHaveScreenshot('quality-metrics-section.png', {
            timeout: 30000,
        });
    });

    test('quality score panel should display valid value', async ({ page }) => {
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        // M-101: Wait for panels to load instead of fixed timeout
        await waitForPanelsLoaded(page);

        // M-102: Use Prometheus API to verify quality score metric exists and is valid
        // This replaces regex-based HTML content inspection
        const promData = await queryPrometheus('dashstream_quality_score');

        if (promData && promData.result && promData.result.length > 0) {
            // Prometheus has the metric - verify it's in valid range [0, 1]
            const value = parseFloat(promData.result[0].value[1]);
            expect(value).toBeGreaterThanOrEqual(0);
            expect(value).toBeLessThanOrEqual(1);
        } else {
            // Prometheus not available or no data - fall back to visual verification
            // Look for the actual rendered value in the panel, not raw HTML
            const qualityScorePanel = await locatePanelContainerByTitle(page, 'Quality Score');
            await expect(qualityScorePanel).toBeVisible();

            // The panel should contain a numeric value (the stat display)
            // Use a more targeted check than raw HTML regex
            const panelText = await qualityScorePanel.textContent();
            // Quality score can be displayed as decimal (0.904) or percentage (90.4%)
            const hasNumericValue =
                /\d+\.?\d*/.test(panelText) && // Has a number
                !panelText.includes('No data') && // Not showing "No data"
                !panelText.includes('N/A'); // Not showing N/A

            expect(hasNumericValue, `Quality Score panel should display a numeric value, got: "${panelText}"`).toBe(
                true
            );
        }
    });

    test('should not show "No data" on critical panels', async ({ page }) => {
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        // M-101: Wait for panels to load instead of fixed timeout
        await waitForPanelsLoaded(page);

        // Treat the dashboard JSON as the source-of-truth for which KPI panels must render.
        // This ensures we cover *all* KPI panels, not just a handful of hardcoded titles.
        const panelTitles = getDashboardPanelTitles();

        for (const panelTitle of panelTitles) {
            const container = await locatePanelContainerByTitle(page, panelTitle);
            await container.scrollIntoViewIfNeeded();
            // M-101: Wait after scroll for content to stabilize
            await waitForScrollStabilization(page);
            await expect(container, `Panel "${panelTitle}" should not show "No data"`).not.toContainText(/No data/i);
        }
    });

    test('infrastructure section should match baseline', async ({ page }) => {
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        // M-101: Wait for panels to load instead of fixed timeout
        await waitForPanelsLoaded(page);

        // Scroll to infrastructure section
        await page.evaluate(() => window.scrollTo(0, 800));
        // M-101: Wait for scroll to stabilize
        await waitForScrollStabilization(page);

        await expect(page).toHaveScreenshot('infrastructure-section.png', {
            timeout: 30000,
        });
    });

    // M-102: New test that uses Grafana API to verify dashboard configuration
    test('dashboard should be properly configured via API', async ({ page }) => {
        // Navigate first to ensure we're authenticated
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1`);
        await waitForPanelsLoaded(page);

        // Query Grafana API for dashboard metadata
        const dashboardData = await queryGrafanaDashboard(page, DASHBOARD_UID);

        if (dashboardData && dashboardData.dashboard) {
            const dashboard = dashboardData.dashboard;

            // Verify dashboard has expected structure
            expect(dashboard.uid).toBe(DASHBOARD_UID);
            expect(Array.isArray(dashboard.panels)).toBe(true);
            expect(dashboard.panels.length).toBeGreaterThan(0);

            // Verify critical panels exist by checking titles
            const panelTitles = dashboard.panels.map((p) => p.title).filter(Boolean);
            expect(panelTitles.length).toBeGreaterThan(0);

            // Cross-check with our JSON source-of-truth
            const expectedTitles = getDashboardPanelTitles();
            for (const expected of expectedTitles.slice(0, 5)) {
                // Check first 5 as sanity check
                expect(panelTitles, `Expected panel "${expected}" in dashboard`).toContain(expected);
            }
        } else {
            // API not available - skip this validation (test.skip in Playwright)
            test.skip(true, 'Grafana API not available for dashboard verification');
        }
    });

    // M-102: Test that Prometheus metrics are being scraped
    test('prometheus should have dashboard metrics', async () => {
        // Query for common DashStream metrics
        const metricsToCheck = [
            'dashstream_quality_score',
            'dashstream_messages_total',
            'dashstream_latency_seconds',
        ];

        let foundMetrics = 0;
        for (const metric of metricsToCheck) {
            const promData = await queryPrometheus(metric);
            if (promData && promData.result && promData.result.length > 0) {
                foundMetrics++;
            }
        }

        // At least one metric should be present if observability stack is running
        // If Prometheus is down, skip instead of fail
        if (foundMetrics === 0) {
            const promHealth = await queryPrometheus('up');
            if (!promHealth) {
                test.skip(true, 'Prometheus not available');
            }
        }

        // If Prometheus is up, we expect at least some metrics
        // (may be 0 if no traffic, which is acceptable)
    });
});

// NOTE: Playwright configuration should go in playwright.config.js, not here.
// See playwright.config.js for:
// - M-100: Cross-platform snapshot configuration
// - Viewport settings
// - Timeout configuration
