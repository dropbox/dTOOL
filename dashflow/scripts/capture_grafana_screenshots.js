/**
 * Playwright script to capture Grafana dashboard screenshots
 *
 * USAGE:
 *   node scripts/capture_grafana_screenshots.js
 *
 * PREREQUISITES:
 *   1. Start the observability stack: docker-compose up -d
 *   2. Install Playwright: npm install playwright (in repo root or globally)
 *   3. Grafana should be accessible at http://localhost:3000
 *
 * ENVIRONMENT VARIABLES (optional):
 *   GRAFANA_URL      - Grafana base URL (default: http://localhost:3000)
 *   GRAFANA_USER     - Grafana username (default: admin)
 *   GRAFANA_PASS     - Grafana password (default: admin)
 *   DASHBOARD_UID    - Dashboard UID to capture (default: dashstream-quality)
 *   OUTPUT_DIR       - Output directory for screenshots (default: test-artifacts/grafana-screenshots)
 *
 * OUTPUT:
 *   Screenshots are saved to test-artifacts/grafana-screenshots/ with timestamps:
 *   - grafana_home_YYYY-MM-DD.png        - Grafana home page
 *   - grafana_dashboard_YYYY-MM-DD.png   - Full dashboard (scrolled)
 *   - grafana_quality_metrics_*.png      - Quality metrics section
 *   - grafana_infrastructure_*.png       - Infrastructure section
 *   - grafana_alerting_*.png             - Alerting section
 *
 * NOTE: This script writes to test-artifacts/ which is gitignored.
 *       For versioned proof images, manually copy to reports/evidence/.
 */

const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

// Configuration with environment variable overrides
const GRAFANA_URL = process.env.GRAFANA_URL || 'http://localhost:3000';
const GRAFANA_USER = process.env.GRAFANA_USER || 'admin';
const GRAFANA_PASS = process.env.GRAFANA_PASS || 'admin';
const DASHBOARD_UID = process.env.DASHBOARD_UID || 'dashstream-quality'; // From grafana_quality_dashboard.json

async function captureScreenshots() {
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
        viewport: { width: 1920, height: 1080 }
    });
    const page = await context.newPage();

    // Create output directory (default: test-artifacts/grafana-screenshots)
    const outputDir = process.env.OUTPUT_DIR || path.join(__dirname, '..', 'test-artifacts', 'grafana-screenshots');
    if (!fs.existsSync(outputDir)) {
        fs.mkdirSync(outputDir, { recursive: true });
    }
    console.log(`Output directory: ${outputDir}`);

    const timestamp = new Date().toISOString().split('T')[0];

    try {
        // Login to Grafana
        console.log('Logging into Grafana...');
        await page.goto(`${GRAFANA_URL}/login`);
        await page.fill('input[name="user"]', GRAFANA_USER);
        await page.fill('input[name="password"]', GRAFANA_PASS);
        await page.click('button[type="submit"]');
        await page.waitForTimeout(2000);

        // Capture home page
        console.log('Capturing Grafana home...');
        await page.goto(`${GRAFANA_URL}/`);
        await page.waitForTimeout(2000);
        await page.screenshot({
            path: path.join(outputDir, `grafana_home_${timestamp}.png`),
            fullPage: false
        });
        console.log(`Saved: grafana_home_${timestamp}.png`);

        // Navigate to dashboard
        console.log('Navigating to DashStream Quality Dashboard...');
        await page.goto(`${GRAFANA_URL}/d/${DASHBOARD_UID}/dashstream-quality-monitoring?orgId=1&refresh=5s`);
        await page.waitForTimeout(5000); // Wait for panels to load

        // Capture full dashboard
        console.log('Capturing dashboard overview...');
        await page.screenshot({
            path: path.join(outputDir, `grafana_dashboard_${timestamp}.png`),
            fullPage: true
        });
        console.log(`Saved: grafana_dashboard_${timestamp}.png`);

        // Scroll to different sections and capture
        // Quality Metrics section
        console.log('Capturing Quality Metrics section...');
        await page.evaluate(() => window.scrollTo(0, 0));
        await page.waitForTimeout(1000);
        await page.screenshot({
            path: path.join(outputDir, `grafana_quality_metrics_${timestamp}.png`),
            fullPage: false
        });
        console.log(`Saved: grafana_quality_metrics_${timestamp}.png`);

        // Infrastructure section (scroll down)
        console.log('Capturing Infrastructure section...');
        await page.evaluate(() => window.scrollTo(0, 800));
        await page.waitForTimeout(1000);
        await page.screenshot({
            path: path.join(outputDir, `grafana_infrastructure_${timestamp}.png`),
            fullPage: false
        });
        console.log(`Saved: grafana_infrastructure_${timestamp}.png`);

        // Alerting section (scroll more)
        console.log('Capturing Alerting section...');
        await page.evaluate(() => window.scrollTo(0, 1600));
        await page.waitForTimeout(1000);
        await page.screenshot({
            path: path.join(outputDir, `grafana_alerting_${timestamp}.png`),
            fullPage: false
        });
        console.log(`Saved: grafana_alerting_${timestamp}.png`);

        console.log('\nAll screenshots captured successfully!');

    } catch (error) {
        console.error('Error capturing screenshots:', error);
        // Capture error state
        await page.screenshot({
            path: path.join(outputDir, `grafana_error_${timestamp}.png`),
            fullPage: true
        });
    } finally {
        await browser.close();
    }
}

captureScreenshots();
