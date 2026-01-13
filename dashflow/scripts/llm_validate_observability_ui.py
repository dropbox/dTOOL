#!/usr/bin/env python3
"""
LLM-as-Judge Validation: Observability UI

Tests that the React observability UI:
1. Loads without errors
2. Connects to WebSocket server (localhost:3002)
3. Displays real-time LangStream events
4. Shows connection status indicator
5. Renders event stream with timestamps, types, thread IDs

Uses Playwright + OpenAI GPT-4o-mini for visual validation.

Usage:
    python3 scripts/llm_validate_observability_ui.py

Environment:
    OPENAI_API_KEY - Required for LLM-as-judge validation

Exit codes:
    0 - PASS (UI functional, events visible)
    1 - FAIL (UI broken, no events, or validation error)
"""

import asyncio
import sys
import os
import json
import base64
import subprocess
import time
from pathlib import Path

try:
    from playwright.async_api import async_playwright
    import openai
except ImportError:
    print("ERROR: Missing dependencies. Install with:")
    print("  pip install playwright openai")
    print("  playwright install chromium")
    sys.exit(1)


async def start_ui_dev_server():
    """Start the observability UI dev server in background."""
    ui_dir = Path(__file__).parent.parent / "observability-ui"

    if not ui_dir.exists():
        return None, "observability-ui directory not found"

    # Start vite dev server
    process = subprocess.Popen(
        ["npm", "run", "dev"],
        cwd=ui_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )

    # Wait for server to be ready
    for _ in range(30):  # 30 second timeout
        try:
            import urllib.request
            urllib.request.urlopen("http://localhost:5173", timeout=1)
            return process, None
        except:
            time.sleep(1)

    process.kill()
    return None, "UI dev server failed to start within 30 seconds"


async def test_observability_ui():
    """
    LLM-as-judge validation of Observability UI functionality.

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

    # Start UI dev server
    print("🚀 Starting observability UI dev server...")
    ui_process, error = await start_ui_dev_server()
    if error:
        return {
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": f"Failed to start UI: {error}",
            "errors": [error]
        }

    try:
        async with async_playwright() as p:
            print("🌐 Launching browser...")
            browser = await p.chromium.launch(headless=True)
            page = await browser.new_page()

            # Collect console logs and errors
            console_logs = []
            errors = []

            page.on("console", lambda msg: console_logs.append(f"{msg.type}: {msg.text}"))
            page.on("pageerror", lambda exc: errors.append(str(exc)))

            try:
                print("📡 Navigating to UI (http://localhost:5173)...")
                await page.goto("http://localhost:5173", timeout=10000)
                await page.wait_for_load_state("networkidle", timeout=10000)

                print("⏳ Waiting for WebSocket connection and events (5 seconds)...")
                await asyncio.sleep(5)

                # Take screenshots
                print("📸 Capturing screenshots...")
                screenshot1 = await page.screenshot(full_page=True)
                screenshot1_b64 = base64.b64encode(screenshot1).decode()

                await asyncio.sleep(3)

                screenshot2 = await page.screenshot(full_page=True)
                screenshot2_b64 = base64.b64encode(screenshot2).decode()

                await browser.close()

                # Call OpenAI
                print("🤖 Sending to OpenAI for evaluation...")
                client = openai.OpenAI(api_key=api_key)

                response = client.chat.completions.create(
                    model="gpt-4o-mini",
                    messages=[{
                        "role": "user",
                        "content": [
                            {"type": "text", "text": """Analyze these screenshots of a real-time observability UI.

Check for:
1. Connection status indicator (🟢 Connected or 🔴 Disconnected)
2. Event stream with timestamps and data
3. No React errors or blank screens

Respond in JSON:
{"verdict": "PASS"|"FAIL", "confidence": 0-100, "reasoning": "...", "connection_status": "connected"|"disconnected", "events_visible": <count>, "ui_quality": "excellent"|"good"|"poor"|"broken", "critical_issues": []}

PASS if: connected, >= 3 events visible, good UI quality"""},
                            {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{screenshot1_b64}", "detail": "high"}},
                            {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{screenshot2_b64}", "detail": "high"}}
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
                result["errors"] = errors
                result["console_logs"] = console_logs[-20:]

                return result

            except Exception as e:
                await browser.close()
                return {
                    "verdict": "FAIL",
                    "confidence": 100,
                    "reasoning": f"Test failed: {str(e)}",
                    "errors": [str(e)]
                }

    finally:
        if ui_process:
            print("🛑 Stopping UI dev server...")
            ui_process.terminate()
            try:
                ui_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                ui_process.kill()


async def main():
    """Main entry point."""
    print("=" * 70)
    print("LLM-as-Judge: Observability UI Validation")
    print("=" * 70)

    result = await test_observability_ui()

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
