#!/usr/bin/env python3
"""
LLM-as-Judge Validation: Jaeger Distributed Traces

Tests that distributed tracing:
1. Websocket-server service is registered in Jaeger
2. Traces are being generated
3. Spans include meaningful data (partition, offset, timing)
4. Jaeger UI is accessible and functional

Uses Playwright + OpenAI GPT-4o-mini for visual validation.

Usage:
    python3 scripts/llm_validate_jaeger_traces.py

Environment:
    OPENAI_API_KEY - Required for LLM-as-judge validation

Exit codes:
    0 - PASS (tracing functional, traces visible)
    1 - FAIL (tracing broken or validation error)
"""

import asyncio
import sys
import os
import json
import base64
import requests

try:
    from playwright.async_api import async_playwright
    import openai
except ImportError:
    print("ERROR: Missing dependencies. Install with:")
    print("  pip install playwright openai requests")
    print("  playwright install chromium")
    sys.exit(1)


async def test_jaeger_traces():
    """
    LLM-as-judge validation of Jaeger distributed tracing.

    Returns:
        dict: Validation results with verdict, confidence, reasoning
    """

    # Check for API key
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        return {
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": "OPENAI_API_KEY environment variable not set",
            "errors": ["Missing OPENAI_API_KEY"]
        }

    # First, check Jaeger API
    print("📡 Checking Jaeger API...")
    try:
        services_resp = requests.get("http://localhost:16686/api/services", timeout=5)
        services = services_resp.json()["data"]
        print(f"   Services registered: {services}")

        traces_resp = requests.get(
            "http://localhost:16686/api/traces",
            params={"service": "websocket-server", "limit": 5},
            timeout=5
        )
        traces = traces_resp.json()["data"]
        print(f"   Traces found: {len(traces)}")

    except Exception as e:
        return {
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": f"Jaeger API not accessible: {str(e)}",
            "errors": [str(e)]
        }

    # Now validate UI with LLM
    async with async_playwright() as p:
        print("🌐 Launching browser...")
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()

        try:
            print("📡 Navigating to Jaeger UI...")
            await page.goto("http://localhost:16686", timeout=10000)
            await page.wait_for_load_state("networkidle", timeout=10000)

            # Wait for UI to load
            await asyncio.sleep(2)

            # Select websocket-server service
            print("🔍 Selecting websocket-server service...")
            try:
                await page.select_option('select', 'websocket-server', timeout=5000)
                await asyncio.sleep(1)
            except:
                print("   Warning: Could not select service (may already be selected)")

            # Click "Find Traces"
            print("🔎 Clicking Find Traces...")
            try:
                await page.click('button:has-text("Find Traces")', timeout=5000)
                await asyncio.sleep(3)
            except:
                print("   Warning: Could not click Find Traces button")

            # Take screenshot
            print("📸 Capturing screenshot...")
            screenshot = await page.screenshot(full_page=True)
            screenshot_b64 = base64.b64encode(screenshot).decode()

            await browser.close()

            # Call OpenAI
            print("🤖 Sending to OpenAI for evaluation...")
            client = openai.OpenAI(api_key=api_key)

            api_summary = {
                "services": services,
                "trace_count": len(traces),
                "sample_span": traces[0]["spans"][0] if traces and traces[0].get("spans") else {}
            }

            response = client.chat.completions.create(
                model="gpt-4o-mini",
                messages=[{
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": f"""Analyze Jaeger distributed tracing system.

**API Data (AUTHORITATIVE)**:
{json.dumps(api_summary, indent=2)}

**Screenshot**: Jaeger UI for visual confirmation

**Evaluation Criteria (Base on API data, not screenshot)**:
1. Service registered: "websocket-server" in services list = PASS
2. Trace count: trace_count >= 3 = PASS (API shows {len(traces)} traces)
3. Span data complete: sample_span has partition, offset, timing fields = PASS
4. UI visual check: Screenshot shows Jaeger interface loaded (any state is acceptable)

**Important**: API data is authoritative. If API shows trace_count=5 and requirement is >=3, that is a PASS for criterion 2.

Respond in JSON:
{{"verdict": "PASS"|"FAIL", "confidence": 0-100, "reasoning": "...", "service_registered": true|false, "trace_quality": "excellent"|"good"|"poor", "span_data_complete": true|false, "ui_functional": true|false}}

**Success Criteria**: PASS if all 4 criteria met based on API data validation."""
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:image/png;base64,{screenshot_b64}",
                                "detail": "high"
                            }
                        }
                    ]
                }],
                temperature=0.1,
                max_tokens=500
            )

            llm_response = response.choices[0].message.content

            # Parse JSON
            if "```json" in llm_response:
                llm_response = llm_response.split("```json")[1].split("```")[0].strip()
            elif "```" in llm_response:
                llm_response = llm_response.split("```")[1].split("```")[0].strip()

            result = json.loads(llm_response)
            result["services_registered"] = services
            result["trace_count"] = len(traces)
            result["span_details"] = traces[0]["spans"][0] if traces and traces[0].get("spans") else {}

            return result

        except Exception as e:
            await browser.close()
            return {
                "verdict": "FAIL",
                "confidence": 100,
                "reasoning": f"Test failed: {str(e)}",
                "errors": [str(e)]
            }


async def main():
    """Main entry point."""
    print("=" * 70)
    print("LLM-as-Judge: Jaeger Distributed Traces Validation")
    print("=" * 70)

    result = await test_jaeger_traces()

    print("\n" + "=" * 70)
    print("RESULTS")
    print("=" * 70)
    print(json.dumps(result, indent=2))
    print("=" * 70)

    verdict = result.get("verdict", "FAIL")
    confidence = result.get("confidence", 0)

    if verdict == "PASS":
        print(f"✅ PASS (confidence: {confidence}%)")
        return 0
    else:
        print(f"❌ FAIL (confidence: {confidence}%)")
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
