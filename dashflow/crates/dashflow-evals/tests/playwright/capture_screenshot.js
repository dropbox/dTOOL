#!/usr/bin/env node

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

async function captureScreenshot() {
  // Use librarian outputs directory (document_search was consolidated into librarian in Dec 2025)
  const htmlPath = path.resolve(__dirname, '../../../../examples/apps/librarian/outputs/eval_report.html');
  const outputPath = path.resolve(__dirname, '../../../../examples/apps/librarian/outputs/eval_report_screenshot.png');

  if (!fs.existsSync(htmlPath)) {
    console.error(`HTML file not found: ${htmlPath}`);
    process.exit(1);
  }

  console.log(`Capturing screenshot of: ${htmlPath}`);
  console.log(`Output to: ${outputPath}`);

  const browser = await chromium.launch();
  const page = await browser.newPage({
    viewport: { width: 1280, height: 1080 }
  });

  await page.goto(`file://${htmlPath}`);

  // Wait for charts to load
  await page.waitForTimeout(1000);

  // Capture full page screenshot
  await page.screenshot({
    path: outputPath,
    fullPage: true
  });

  const stats = fs.statSync(outputPath);
  console.log(`Screenshot saved: ${outputPath} (${(stats.size / 1024).toFixed(1)}KB)`);

  await browser.close();
}

captureScreenshot().catch(err => {
  console.error('Error capturing screenshot:', err);
  process.exit(1);
});
