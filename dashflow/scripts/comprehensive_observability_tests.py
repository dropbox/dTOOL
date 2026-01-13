#!/usr/bin/env python3
"""
Comprehensive Observability Test Suite

Runs all LLM-as-judge validation tests:
- Issue #12: Infrastructure metrics (Prometheus + Grafana)
- Issue #14: Distributed tracing (Jaeger)
- Issue #15: Observability UI (React + WebSocket)

Usage:
    python3 scripts/comprehensive_observability_tests.py

Environment:
    OPENAI_API_KEY - Required for LLM-as-judge validation

Exit codes:
    0 - All tests PASS
    1 - One or more tests FAIL
"""

import asyncio
import sys
import os
import json
import subprocess
from pathlib import Path
from datetime import datetime


async def run_test(script_name):
    """Run a validation test script and return results."""
    script_path = Path(__file__).parent / script_name

    if not script_path.exists():
        return {
            "test": script_name,
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": f"Test script not found: {script_path}",
            "errors": ["Script not found"]
        }

    print(f"\n{'='*70}")
    print(f"Running: {script_name}")
    print(f"{'='*70}\n")

    try:
        process = await asyncio.create_subprocess_exec(
            sys.executable, str(script_path),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        stdout, stderr = await asyncio.wait_for(
            process.communicate(),
            timeout=120  # 2 minute timeout per test
        )

        stdout_text = stdout.decode()
        stderr_text = stderr.decode()

        # Try to extract JSON result from output
        result = None
        for line in stdout_text.split('\n'):
            if line.strip().startswith('{'):
                try:
                    result = json.loads(line)
                    break
                except:
                    pass

        if not result:
            result = {
                "test": script_name,
                "verdict": "FAIL" if process.returncode != 0 else "UNKNOWN",
                "confidence": 50,
                "reasoning": "Could not parse test output",
                "exit_code": process.returncode,
                "stdout_sample": stdout_text[:500],
                "stderr_sample": stderr_text[:500]
            }

        result["test"] = script_name
        result["exit_code"] = process.returncode

        return result

    except asyncio.TimeoutError:
        return {
            "test": script_name,
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": "Test timed out after 2 minutes",
            "errors": ["Timeout"]
        }
    except Exception as e:
        return {
            "test": script_name,
            "verdict": "FAIL",
            "confidence": 100,
            "reasoning": f"Test execution failed: {str(e)}",
            "errors": [str(e)]
        }


async def main():
    """Main entry point."""
    print("=" * 70)
    print("COMPREHENSIVE OBSERVABILITY TEST SUITE")
    print("=" * 70)
    print(f"Started: {datetime.now().isoformat()}")
    print(f"OPENAI_API_KEY: {'set' if os.getenv('OPENAI_API_KEY') else 'NOT SET'}")
    print("=" * 70)

    # Define tests
    tests = [
        ("llm_validate_grafana.py", "Issue #12: Infrastructure Metrics (Grafana)"),
        ("llm_validate_jaeger_traces.py", "Issue #14: Distributed Tracing (Jaeger)"),
        ("llm_validate_observability_ui.py", "Issue #15: Observability UI"),
    ]

    results = []

    # Run tests sequentially (to avoid resource conflicts)
    for script_name, description in tests:
        print(f"\n\n{'#'*70}")
        print(f"# {description}")
        print(f"# Script: {script_name}")
        print(f"{'#'*70}")

        result = await run_test(script_name)
        results.append(result)

        # Print immediate result
        verdict = result.get("verdict", "UNKNOWN")
        confidence = result.get("confidence", 0)

        if verdict == "PASS":
            print(f"\n✅ {description}: PASS (confidence: {confidence}%)")
        else:
            print(f"\n❌ {description}: FAIL (confidence: {confidence}%)")
            print(f"   Reason: {result.get('reasoning', 'Unknown')}")

    # Summary
    print("\n\n" + "=" * 70)
    print("TEST SUMMARY")
    print("=" * 70)

    passed = sum(1 for r in results if r.get("verdict") == "PASS")
    failed = sum(1 for r in results if r.get("verdict") != "PASS")

    for i, result in enumerate(results, 1):
        test_name = result.get("test", "Unknown")
        verdict = result.get("verdict", "UNKNOWN")
        confidence = result.get("confidence", 0)

        status = "✅ PASS" if verdict == "PASS" else "❌ FAIL"
        print(f"{i}. {test_name}: {status} ({confidence}% confidence)")
        if verdict != "PASS":
            print(f"   {result.get('reasoning', '')}")

    print(f"\nTotal: {passed}/{len(results)} passed")
    print("=" * 70)

    # Save detailed results
    results_file = Path(__file__).parent.parent / "test_results_observability.json"
    with open(results_file, 'w') as f:
        json.dump({
            "timestamp": datetime.now().isoformat(),
            "summary": {
                "total": len(results),
                "passed": passed,
                "failed": failed
            },
            "results": results
        }, f, indent=2)

    print(f"\nDetailed results saved to: {results_file}")

    # Exit code
    if failed == 0:
        print("\n🎉 All tests passed!")
        return 0
    else:
        print(f"\n⚠️  {failed} test(s) failed")
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
