// Playwright configuration for DashFlow observability tests
// M-100: Cross-platform snapshot baseline support
const { defineConfig, devices } = require('@playwright/test');
const os = require('os');

// Determine platform for snapshot naming
// This allows separate baselines per OS while sharing test logic
const platform = os.platform(); // 'darwin', 'linux', 'win32'

module.exports = defineConfig({
  testDir: './test-utils/tests',
  testMatch: '**/*.test.js',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: 'html',
  use: {
    baseURL: process.env.GRAFANA_URL || 'http://localhost:3000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  // Expect test timeout to be longer for Grafana dashboard loading
  timeout: 60000,
  expect: {
    timeout: 10000,
    // M-100: Configure snapshot comparison for cross-platform robustness
    toHaveScreenshot: {
      // Higher threshold to account for font rendering differences across platforms
      // Linux/macOS/Windows render fonts slightly differently
      maxDiffPixelRatio: 0.08,
      // Allow anti-aliasing differences (common cross-platform issue)
      threshold: 0.3,
      // Animations can cause flakiness
      animations: 'disabled',
    },
    toMatchSnapshot: {
      maxDiffPixelRatio: 0.08,
      threshold: 0.3,
    },
  },
  // M-100: Platform-aware snapshot directory
  // Baselines stored per-platform to avoid cross-platform failures
  snapshotPathTemplate: '{testDir}/{testFileDir}/{testFileName}-snapshots/{arg}-{projectName}-{platform}{ext}',
});
