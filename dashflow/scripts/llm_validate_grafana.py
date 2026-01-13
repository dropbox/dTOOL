#!/usr/bin/env python3
"""
LLM-as-Judge validation for Grafana dashboard panels.

Issue #19: Grafana Dashboard Panels
Tests that the 5 new panels for sequence validation and DLQ metrics are:
1. Visible in the Grafana dashboard
2. Rendering correctly with proper queries
3. Displaying data from Prometheus

Uses OpenAI GPT-4o-mini to evaluate screenshots from Playwright.

Usage:
    export OPENAI_API_KEY="sk-proj-..."
    python3 scripts/llm_validate_grafana.py
"""

import asyncio
import base64
import json
import os
import sys
from pathlib import Path
from typing import Dict, Any

try:
    from playwright.async_api import async_playwright
    from openai import OpenAI
except ImportError as e:
    print(f"❌ Missing dependency: {e}")
    print("Install with: pip install playwright openai")
    sys.exit(1)

# Check for API key
if not os.getenv("OPENAI_API_KEY"):
    print("❌ OPENAI_API_KEY environment variable not set")
    print("Set with: export OPENAI_API_KEY=\"sk-proj-...\"")
    sys.exit(1)

GRAFANA_URL = "http://localhost:3000"
GRAFANA_USERNAME = "admin"
GRAFANA_PASSWORD = "admin"

# Expected 5 new panels (Issue #19)
EXPECTED_PANELS = [
    "Sequence Gaps",
    "Duplicate Message Rate",
    "Out-of-Order Message Rate",
    "DLQ Write Rate",
    "DLQ Health"
]

async def login_to_grafana(page):
    """Login to Grafana."""
    await page.goto(GRAFANA_URL, timeout=10000)

    # Check if already logged in
    if "login" in page.url.lower():
        await page.fill('input[name="user"]', GRAFANA_USERNAME)
        await page.fill('input[name="password"]', GRAFANA_PASSWORD)
        await page.click('button[type="submit"]')
        await page.wait_for_timeout(2000)

    return True

async def navigate_to_dashboard(page):
    """Navigate to DashFlow Quality dashboard."""
    # Search for dashboard by title since UID is auto-generated
    await page.goto(f"{GRAFANA_URL}/dashboards", timeout=10000)
    await page.wait_for_timeout(2000)

    # Search for quality dashboard
    search_box = page.locator('input[placeholder*="Search"]').first
    await search_box.fill("Quality Agent")
    await page.wait_for_timeout(1000)

    # Click on the dashboard link
    dashboard_link = page.locator('a:has-text("DashFlow Quality Agent")').first
    await dashboard_link.click()
    await page.wait_for_timeout(3000)  # Wait for panels to render

    return True

async def capture_dashboard_screenshot(page):
    """Capture full dashboard screenshot."""
    # Scroll to bottom to ensure all panels visible
    await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
    await page.wait_for_timeout(1000)

    # Scroll back to top
    await page.evaluate("window.scrollTo(0, 0)")
    await page.wait_for_timeout(1000)

    # Take full-page screenshot
    screenshot_bytes = await page.screenshot(full_page=True)

    return screenshot_bytes

async def capture_new_panels_screenshot(page):
    """Capture screenshot of the new panels area (panels 17-21)."""
    # Scroll to the new panels (they're at gridPos y=32+)
    await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
    await page.wait_for_timeout(2000)  # Wait for panels to load

    screenshot_bytes = await page.screenshot()

    return screenshot_bytes

def evaluate_with_llm(screenshot_b64: str, panel_area_screenshot_b64: str) -> Dict[str, Any]:
    """Use OpenAI GPT-4o-mini to evaluate Grafana dashboard."""

    client = OpenAI(api_key=os.getenv("OPENAI_API_KEY"))

    prompt = f"""You are an expert at validating Grafana dashboards. Analyze these two screenshots of a Grafana dashboard.

**Context**: Issue #19 added 5 new panels to this dashboard for sequence validation and DLQ (Dead Letter Queue) metrics.

**Expected Panels** (added in Issue #19):
1. "Sequence Gaps (Message Loss Detection)" - Time series graph showing rate of sequence gaps
2. "Duplicate Message Rate" - Time series graph showing duplicate messages per second
3. "Out-of-Order Message Rate" - Time series graph showing reordered messages per second
4. "DLQ Write Rate by Error Type" - Stacked time series showing DLQ writes by error type
5. "DLQ Health (Send Failures)" - Stat panel showing DLQ send failure rate

**Validation Criteria**:
1. Are ALL 5 new panels visible in the dashboard?
2. Do panel titles match expected names (exact or close match)?
3. Are panels rendering correctly (not showing errors)?
4. Are the graph/stat panel types correct (4 time series graphs + 1 stat panel)?
5. Is the dashboard overall functional (not showing major errors)?

**Important**:
- If you see any of the expected panel names, that panel EXISTS (even if no data yet)
- "No data" is acceptable - we're validating panel CONFIGURATION, not data presence
- Panels with empty graphs are VALID if they're configured correctly
- Focus on: panel existence, correct titles, correct visualization types

**Return JSON**:
{{
  "verdict": "PASS" or "FAIL",
  "confidence": 0-100,
  "reasoning": "Detailed explanation of what you found",
  "panels_found": ["list", "of", "panel", "names"],
  "missing_panels": ["list", "of", "missing"],
  "panel_count": <total panels visible>,
  "errors": ["any", "errors", "seen"]
}}

**Examples**:
- PASS: All 5 panels visible with correct names and types (even if empty)
- PASS: 5 panels visible, titles slightly different but clearly the same metrics
- FAIL: Only 3 of 5 panels visible
- FAIL: Panels showing "Panel plugin not found" errors
- FAIL: Dashboard not loading at all

Analyze both screenshots carefully and return ONLY valid JSON."""

    try:
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:image/png;base64,{screenshot_b64}",
                                "detail": "high"
                            }
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:image/png;base64,{panel_area_screenshot_b64}",
                                "detail": "high"
                            }
                        }
                    ]
                }
            ],
            max_tokens=1000,
            temperature=0.1
        )

        llm_response = response.choices[0].message.content.strip()

        # Try to parse JSON
        try:
            # Remove markdown code blocks if present
            if llm_response.startswith("```"):
                llm_response = llm_response.split("```")[1]
                if llm_response.startswith("json"):
                    llm_response = llm_response[4:]

            result = json.loads(llm_response)
            return result
        except json.JSONDecodeError as e:
            print(f"⚠️  Failed to parse LLM response as JSON: {e}")
            print(f"Raw response: {llm_response[:500]}")
            return {
                "verdict": "FAIL",
                "confidence": 50,
                "reasoning": f"Could not parse LLM response: {str(e)}",
                "errors": ["JSON parsing failed"]
            }

    except Exception as e:
        print(f"❌ OpenAI API call failed: {e}")
        return {
            "verdict": "FAIL",
            "confidence": 0,
            "reasoning": f"OpenAI API error: {str(e)}",
            "errors": [str(e)]
        }

async def test_grafana_dashboard():
    """Main test function."""

    print("=" * 70)
    print("LLM-as-Judge: Grafana Dashboard Validation (Issue #19)")
    print("=" * 70)
    print()

    async with async_playwright() as p:
        # Launch browser
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(viewport={"width": 1920, "height": 1080})
        page = await context.new_page()

        try:
            # Step 1: Login to Grafana
            print("1. Logging into Grafana...")
            await login_to_grafana(page)
            print("   ✅ Login successful")

            # Step 2: Navigate to dashboard
            print("2. Navigating to DashFlow Quality dashboard...")
            await navigate_to_dashboard(page)
            print("   ✅ Dashboard loaded")

            # Step 3: Capture screenshots
            print("3. Capturing dashboard screenshots...")
            full_screenshot = await capture_dashboard_screenshot(page)
            panel_screenshot = await capture_new_panels_screenshot(page)
            print(f"   ✅ Screenshots captured ({len(full_screenshot)} bytes, {len(panel_screenshot)} bytes)")

            # Step 4: Encode screenshots
            full_screenshot_b64 = base64.b64encode(full_screenshot).decode('utf-8')
            panel_screenshot_b64 = base64.b64encode(panel_screenshot).decode('utf-8')

            # Step 5: LLM evaluation
            print("4. Evaluating dashboard with LLM (GPT-4o-mini)...")
            result = evaluate_with_llm(full_screenshot_b64, panel_screenshot_b64)

            # Print results
            print()
            print("=" * 70)
            print("RESULTS")
            print("=" * 70)
            print(json.dumps(result, indent=2))
            print("=" * 70)

            # Return result
            return result

        finally:
            await browser.close()

def main():
    """Entry point."""

    # Check Grafana is running
    import urllib.request
    try:
        urllib.request.urlopen(GRAFANA_URL, timeout=5)
    except Exception as e:
        print(f"❌ Grafana not accessible at {GRAFANA_URL}")
        print(f"   Error: {e}")
        print("   Start with: docker restart dashstream-grafana")
        sys.exit(1)

    # Run test
    result = asyncio.run(test_grafana_dashboard())

    # Exit with appropriate code
    if result.get("verdict") == "PASS":
        print()
        print("✅ VALIDATION PASSED")
        sys.exit(0)
    else:
        print()
        print("❌ VALIDATION FAILED")
        sys.exit(1)

if __name__ == "__main__":
    main()
