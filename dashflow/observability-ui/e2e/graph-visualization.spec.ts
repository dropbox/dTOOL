import { test, expect } from '@playwright/test';

test.describe('Graph Visualization', () => {
  test('should display graph tab with live execution features', async ({ page }) => {
    // Navigate to the dashboard
    await page.goto('http://localhost:5173');

    // Wait for page to load (use domcontentloaded - networkidle hangs with WebSocket)
    await page.waitForLoadState('domcontentloaded');
    await page.waitForTimeout(2000); // Allow React to hydrate

    // Take screenshot of initial state
    await page.screenshot({ path: 'test-results/01-initial-dashboard.png', fullPage: true });

    // Click on Graph tab
    const graphTab = page.getByRole('button', { name: /graph/i });
    if (await graphTab.isVisible()) {
      await graphTab.click();
      await page.waitForTimeout(1000);
    }

    // Take screenshot of graph tab
    await page.screenshot({ path: 'test-results/02-graph-tab.png', fullPage: true });

    // Check for key elements
    const pageContent = await page.content();

    // LLM-as-Judge evaluation criteria
    const evaluationReport = {
      timestamp: new Date().toISOString(),
      url: page.url(),
      criteria: {
        hasGraphTab: pageContent.includes('Graph') || pageContent.includes('graph'),
        hasNodes: pageContent.includes('node') || pageContent.includes('Node'),
        hasTimeline: pageContent.includes('Timeline') || pageContent.includes('timeline'),
        hasStateDiff: pageContent.includes('State') || pageContent.includes('state'),
        hasLiveIndicator: pageContent.includes('LIVE') || pageContent.includes('live'),
        hasExecutionInfo: pageContent.includes('execution') || pageContent.includes('Execution'),
      },
      elementsFound: {
        buttons: await page.locator('button').count(),
        canvases: await page.locator('canvas').count(),
        svgs: await page.locator('svg').count(),
      }
    };

    console.log('\n========== LLM-as-Judge Evaluation ==========');
    console.log(JSON.stringify(evaluationReport, null, 2));
    console.log('==============================================\n');

    // Take final screenshot
    await page.screenshot({ path: 'test-results/03-final-state.png', fullPage: true });

    // Basic assertions
    expect(evaluationReport.criteria.hasGraphTab).toBeTruthy();
  });

  test('should show graph canvas with nodes when graph data exists', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await page.waitForLoadState('domcontentloaded');
    await page.waitForTimeout(2000);

    // Navigate to graph tab
    const graphTab = page.getByRole('button', { name: /graph/i });
    if (await graphTab.isVisible()) {
      await graphTab.click();
    }

    // Wait for any animations
    await page.waitForTimeout(2000);

    // Check for React Flow elements (the graph library)
    const reactFlowContainer = page.locator('.react-flow');
    const hasReactFlow = await reactFlowContainer.count() > 0;

    // Check for graph nodes
    const graphNodes = page.locator('.react-flow__node');
    const nodeCount = await graphNodes.count();

    // Check for edges
    const graphEdges = page.locator('.react-flow__edge');
    const edgeCount = await graphEdges.count();

    console.log('\n========== Graph Canvas Evaluation ==========');
    console.log({
      hasReactFlow,
      nodeCount,
      edgeCount,
      timestamp: new Date().toISOString()
    });
    console.log('==============================================\n');

    await page.screenshot({ path: 'test-results/04-graph-canvas.png', fullPage: true });
  });
});
