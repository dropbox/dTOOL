#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""Compare two evaluation runs to identify changes."""

import json
import sys
from pathlib import Path

def load_report(path):
    """Load JSON report."""
    with open(path) as f:
        return json.load(f)

def compare_reports(report1, report2, name1="N=85", name2="N=87"):
    """Compare two evaluation reports."""

    print(f"\n{'='*80}")
    print(f"EVALUATION COMPARISON: {name1} vs {name2}")
    print(f"{'='*80}\n")

    # Summary comparison
    s1 = report1['summary']
    s2 = report2['summary']

    print("SUMMARY METRICS:")
    print(f"  Pass Rate:      {s1['pass_rate']*100:.1f}% → {s2['pass_rate']*100:.1f}% ({s2['passed']-s1['passed']:+d} scenarios)")
    print(f"  Avg Quality:    {s1['avg_quality']:.3f} → {s2['avg_quality']:.3f} ({s2['avg_quality']-s1['avg_quality']:+.3f})")
    print(f"  Avg Latency:    {s1['avg_latency_ms']}ms → {s2['avg_latency_ms']}ms ({s2['avg_latency_ms']-s1['avg_latency_ms']:+d}ms)")
    print(f"  Duration:       {s1['total_duration_secs']:.1f}s → {s2['total_duration_secs']:.1f}s ({s2['total_duration_secs']-s1['total_duration_secs']:+.1f}s)")

    # Build scenario dicts
    scenarios1 = {s['id']: s for s in report1['scenarios']}
    scenarios2 = {s['id']: s for s in report2['scenarios']}

    # Find differences
    improved = []  # fail → pass
    regressed = []  # pass → fail
    still_passing = []
    still_failing = []

    for sid in sorted(scenarios1.keys()):
        s1 = scenarios1[sid]
        s2 = scenarios2[sid]

        if not s1['passed'] and s2['passed']:
            improved.append((sid, s1, s2))
        elif s1['passed'] and not s2['passed']:
            regressed.append((sid, s1, s2))
        elif s1['passed'] and s2['passed']:
            still_passing.append((sid, s1, s2))
        else:
            still_failing.append((sid, s1, s2))

    print(f"\nSCENARIO CHANGES:")
    print(f"  Improved (fail → pass):  {len(improved)}")
    print(f"  Regressed (pass → fail): {len(regressed)}")
    print(f"  Still passing:           {len(still_passing)}")
    print(f"  Still failing:           {len(still_failing)}")

    if improved:
        print(f"\n✅ IMPROVED SCENARIOS ({len(improved)}):")
        for sid, s1, s2 in improved:
            print(f"  {sid}")
            print(f"    Quality: {s1['quality']:.3f} → {s2['quality']:.3f} ({s2['quality']-s1['quality']:+.3f})")
            print(f"    Latency: {s1['latency_ms']}ms → {s2['latency_ms']}ms ({s2['latency_ms']-s1['latency_ms']:+d}ms)")
            if 'error' in s1:
                print(f"    Previous error: {s1['error']}")

    if regressed:
        print(f"\n❌ REGRESSED SCENARIOS ({len(regressed)}):")
        for sid, s1, s2 in regressed:
            print(f"  {sid}")
            print(f"    Quality: {s1['quality']:.3f} → {s2['quality']:.3f} ({s2['quality']-s1['quality']:+.3f})")
            print(f"    Latency: {s1['latency_ms']}ms → {s2['latency_ms']}ms ({s2['latency_ms']-s1['latency_ms']:+d}ms)")
            if 'error' in s2:
                print(f"    New error: {s2['error']}")

    # Quality changes in still-passing scenarios
    quality_changes = []
    for sid, s1, s2 in still_passing:
        delta = s2['quality'] - s1['quality']
        if abs(delta) >= 0.05:  # Significant change
            quality_changes.append((sid, delta, s1['quality'], s2['quality']))

    if quality_changes:
        print(f"\n📊 SIGNIFICANT QUALITY CHANGES (still passing, Δ ≥ 0.05):")
        for sid, delta, q1, q2 in sorted(quality_changes, key=lambda x: abs(x[1]), reverse=True):
            print(f"  {sid}: {q1:.3f} → {q2:.3f} ({delta:+.3f})")

    # Latency outliers in still-failing scenarios
    latency_outliers = []
    for sid, s1, s2 in still_failing:
        delta = s2['latency_ms'] - s1['latency_ms']
        if abs(delta) >= 2000:  # 2 second change
            latency_outliers.append((sid, delta, s1['latency_ms'], s2['latency_ms']))

    if latency_outliers:
        print(f"\n⏱️  LARGE LATENCY CHANGES (still failing, Δ ≥ 2000ms):")
        for sid, delta, l1, l2 in sorted(latency_outliers, key=lambda x: abs(x[1]), reverse=True):
            print(f"  {sid}: {l1}ms → {l2}ms ({delta:+d}ms, {delta/l1*100:+.1f}%)")

    print(f"\n{'='*80}\n")

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print("Usage: python compare_eval_runs.py <report1.json> <report2.json> [name1] [name2]")
        sys.exit(1)

    report1 = load_report(sys.argv[1])
    report2 = load_report(sys.argv[2])

    name1 = sys.argv[3] if len(sys.argv) > 3 else "Run 1"
    name2 = sys.argv[4] if len(sys.argv) > 4 else "Run 2"

    compare_reports(report1, report2, name1, name2)
