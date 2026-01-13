import { test, expect } from '@playwright/test';

test.describe('Live Demo Recording', () => {
  test.setTimeout(180_000); // 3 minutes for full demo

  test('record live telemetry demo', async ({ page }) => {
    // Navigate to UI
    await page.goto('http://localhost:5173');

    // Wait for page to load and show connection status
    // The connection badge in header shows "Connected" when WebSocket is established
    await page.waitForTimeout(5_000);

    // Check if we're getting data (message count > 0)
    await expect(page.locator('text=Messages Received')).toBeVisible({ timeout: 30_000 });

    // Dashboard view - wait for messages to flow
    // Try to find the messages counter
    const messagesCounter = page.locator('[data-testid="messages-total"]');
    if (await messagesCounter.count() > 0) {
      await expect(messagesCounter).not.toHaveText('0', { timeout: 60_000 });
    }
    await page.waitForTimeout(10_000); // Record dashboard for 10s

    // Switch to Graph tab
    const graphTab = page.locator('text=Graph');
    if (await graphTab.count() > 0) {
      await graphTab.click();
      await page.waitForTimeout(30_000); // Record graph for 30s while agent runs
    }

    // Switch back to Dashboard
    const dashboardTab = page.locator('text=Dashboard');
    if (await dashboardTab.count() > 0) {
      await dashboardTab.click();
    }
    await page.waitForTimeout(20_000); // Record metrics for 20s

    // Final overview
    await page.waitForTimeout(10_000);

    // Take final screenshot
    await page.screenshot({ path: 'screenshots/demo-final.png', fullPage: true });
  });
});
