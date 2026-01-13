#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Analyze validation failures from N=84 evaluation results.

Categorizes failures by missing keywords and identifies over-specific requirements.
"""

import json
import re
from collections import defaultdict
from typing import Dict, List, Tuple

def parse_error_message(error: str) -> Tuple[List[str], List[str]]:
    """Extract missing and forbidden strings from error message."""
    missing = []
    forbidden = []

    # Extract missing strings
    missing_match = re.search(r'Missing required strings: \[(.*?)\]', error)
    if missing_match:
        missing_str = missing_match.group(1)
        # Parse the list of quoted strings
        missing = re.findall(r'"([^"]*)"', missing_str)

    # Extract forbidden strings
    forbidden_match = re.search(r'Found forbidden strings: \[(.*?)\]', error)
    if forbidden_match:
        forbidden_str = forbidden_match.group(1)
        forbidden = re.findall(r'"([^"]*)"', forbidden_str)

    return missing, forbidden

def categorize_validation_failures(results_file: str):
    """Analyze validation failures and categorize by missing keyword."""

    with open(results_file, 'r') as f:
        data = json.load(f)

    # Collect validation failures
    validation_failures = []
    latency_only_failures = []
    quality_only_failures = []

    for scenario in data['scenarios']:
        if not scenario['passed']:
            error = scenario.get('error', '')

            # Check if validation failed
            has_validation_fail = 'Validation failed' in error
            has_latency_fail = 'Latency exceeded' in error
            has_quality_fail = 'Quality below threshold' in error

            if has_validation_fail:
                missing, forbidden = parse_error_message(error)
                validation_failures.append({
                    'id': scenario['id'],
                    'quality': scenario['quality'],
                    'latency_ms': scenario['latency_ms'],
                    'missing': missing,
                    'forbidden': forbidden,
                    'error': error,
                    'has_latency_fail': has_latency_fail,
                    'has_quality_fail': has_quality_fail
                })
            elif has_latency_fail and not has_quality_fail:
                latency_only_failures.append({
                    'id': scenario['id'],
                    'quality': scenario['quality'],
                    'latency_ms': scenario['latency_ms'],
                })
            elif has_quality_fail and not has_latency_fail:
                quality_only_failures.append({
                    'id': scenario['id'],
                    'quality': scenario['quality'],
                    'latency_ms': scenario['latency_ms'],
                })

    # Categorize by missing keyword
    by_missing_keyword = defaultdict(list)
    by_forbidden_keyword = defaultdict(list)

    for failure in validation_failures:
        for keyword in failure['missing']:
            by_missing_keyword[keyword].append(failure)
        for keyword in failure['forbidden']:
            by_forbidden_keyword[keyword].append(failure)

    # Print analysis
    print("=" * 80)
    print("VALIDATION FAILURES ANALYSIS (N=84)")
    print("=" * 80)
    print()

    print(f"Total validation failures: {len(validation_failures)}")
    print(f"  - Validation only: {len([f for f in validation_failures if not f['has_latency_fail'] and not f['has_quality_fail']])}")
    print(f"  - Validation + Latency: {len([f for f in validation_failures if f['has_latency_fail']])}")
    print(f"  - Validation + Quality: {len([f for f in validation_failures if f['has_quality_fail']])}")
    print()
    print(f"Latency-only failures: {len(latency_only_failures)}")
    print(f"Quality-only failures: {len(quality_only_failures)}")
    print()

    # Analyze missing keywords
    print("=" * 80)
    print("MISSING KEYWORDS ANALYSIS")
    print("=" * 80)
    print()

    # Sort by frequency
    keyword_freq = sorted(by_missing_keyword.items(), key=lambda x: len(x[1]), reverse=True)

    for keyword, failures in keyword_freq:
        print(f"\nKeyword: \"{keyword}\" - {len(failures)} scenarios")
        print("-" * 80)

        # Calculate average quality for these failures
        avg_quality = sum(f['quality'] for f in failures) / len(failures)
        high_quality_count = len([f for f in failures if f['quality'] >= 0.90])

        print(f"  Average quality: {avg_quality:.3f}")
        print(f"  High quality (≥0.90): {high_quality_count}/{len(failures)} ({high_quality_count/len(failures)*100:.0f}%)")
        print()
        print("  Affected scenarios:")

        for failure in failures:
            quality_marker = "🟢" if failure['quality'] >= 0.90 else "🟡" if failure['quality'] >= 0.80 else "🔴"
            latency_marker = "⏱️" if failure['has_latency_fail'] else ""
            qual_marker = "📉" if failure['has_quality_fail'] else ""

            print(f"    {quality_marker} {failure['id']}: quality={failure['quality']:.3f}, latency={failure['latency_ms']}ms {latency_marker}{qual_marker}")
            if len(failure['missing']) > 1:
                print(f"       Also missing: {[k for k in failure['missing'] if k != keyword]}")

    # Analyze forbidden keywords
    if by_forbidden_keyword:
        print()
        print("=" * 80)
        print("FORBIDDEN KEYWORDS FOUND")
        print("=" * 80)
        print()

        for keyword, failures in sorted(by_forbidden_keyword.items(), key=lambda x: len(x[1]), reverse=True):
            print(f"\nForbidden keyword: \"{keyword}\" - {len(failures)} scenarios")
            print("-" * 80)

            for failure in failures:
                print(f"  - {failure['id']}: quality={failure['quality']:.3f}")

    # Recommendations
    print()
    print("=" * 80)
    print("RECOMMENDATIONS")
    print("=" * 80)
    print()

    print("## High Priority: Keywords to Remove (High Quality + Frequent)")
    print()

    for keyword, failures in keyword_freq:
        avg_quality = sum(f['quality'] for f in failures) / len(failures)
        high_quality_count = len([f for f in failures if f['quality'] >= 0.90])
        high_quality_pct = high_quality_count / len(failures)

        # Recommend removal if high average quality and frequent
        if avg_quality >= 0.90 and len(failures) >= 3:
            print(f"  ❌ REMOVE \"{keyword}\"")
            print(f"     Reason: {len(failures)} failures, avg quality {avg_quality:.3f}, {high_quality_pct*100:.0f}% high quality")
            print(f"     Expected impact: +{len(failures)} passes")
            print()
        elif avg_quality >= 0.85 and high_quality_pct >= 0.50:
            print(f"  ⚠️  REVIEW \"{keyword}\"")
            print(f"     Reason: {len(failures)} failures, avg quality {avg_quality:.3f}, {high_quality_pct*100:.0f}% high quality")
            print(f"     Recommend: Make optional or use case-insensitive matching")
            print()

    print()
    print("## Low Priority: Keywords to Keep (Essential for validation)")
    print()

    for keyword, failures in keyword_freq:
        avg_quality = sum(f['quality'] for f in failures) / len(failures)

        # Keep if low quality suggests genuine failures
        if avg_quality < 0.85:
            print(f"  ✅ KEEP \"{keyword}\"")
            print(f"     Reason: {len(failures)} failures, avg quality {avg_quality:.3f}")
            print(f"     Indicates genuine quality issues, not over-specific validation")
            print()

    # Summary metrics
    print()
    print("=" * 80)
    print("EXPECTED IMPACT")
    print("=" * 80)
    print()

    # Count removable keywords
    removable = []
    for keyword, failures in keyword_freq:
        avg_quality = sum(f['quality'] for f in failures) / len(failures)
        if avg_quality >= 0.90 and len(failures) >= 3:
            removable.extend([f['id'] for f in failures])

    # Remove duplicates (scenarios with multiple removable keywords)
    unique_removable = len(set(removable))

    print(f"Scenarios with removable keywords: {unique_removable}")
    print(f"Current pass rate: {data['summary']['pass_rate']*100:.1f}%")
    print(f"Expected pass rate after keyword removal: {(data['summary']['pass_rate'] + unique_removable/50)*100:.1f}%")
    print(f"Expected improvement: +{unique_removable/50*100:.1f} percentage points")
    print()

if __name__ == '__main__':
    # Updated from document_search to librarian (app consolidation Dec 2025)
    results_file = 'examples/apps/librarian/outputs/eval_report.json'
    categorize_validation_failures(results_file)
