#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Analyze tarpaulin coverage report and generate per-crate statistics.
"""

import json
import sys
from pathlib import Path
from collections import defaultdict
from typing import Dict, List, Tuple

def parse_coverage_report(report_path: str) -> Tuple[Dict, Dict]:
    """Parse tarpaulin JSON report and calculate per-crate statistics."""

    with open(report_path, 'r') as f:
        data = json.load(f)

    # Overall stats
    overall = {
        'coverage': data.get('coverage', 0),
        'lines_covered': data.get('covered', 0),
        'lines_coverable': data.get('coverable', 0)
    }

    # Per-crate stats
    crate_stats = defaultdict(lambda: {
        'covered': 0,
        'coverable': 0,
        'files': []
    })

    for file_data in data.get('files', []):
        # Join path components
        path_parts = file_data.get('path', [])
        if not path_parts:
            continue

        # Remove leading empty string from join
        full_path = '/'.join(path_parts)
        if full_path.startswith('//'):
            full_path = full_path[1:]

        # Extract crate name if in crates/ directory
        if '/crates/' in full_path:
            parts = full_path.split('/crates/')
            if len(parts) > 1:
                crate_path = parts[1]
                crate_name = crate_path.split('/')[0]
            else:
                continue
        else:
            # Not a crate file (benchmark, example, etc.)
            continue

        # Get file-level stats
        file_covered = file_data.get('covered', 0)
        file_coverable = file_data.get('coverable', 0)

        # Aggregate into crate
        crate_stats[crate_name]['covered'] += file_covered
        crate_stats[crate_name]['coverable'] += file_coverable
        crate_stats[crate_name]['files'].append(full_path)

    # Calculate coverage percentages
    for crate_name, stats in crate_stats.items():
        if stats['coverable'] > 0:
            stats['coverage'] = (stats['covered'] / stats['coverable']) * 100
        else:
            stats['coverage'] = 0.0

    return overall, dict(crate_stats)

def print_coverage_summary(overall: Dict, crate_stats: Dict):
    """Print formatted coverage summary."""

    print("=" * 80)
    print("TARPAULIN COVERAGE REPORT - v1.3.0 BASELINE")
    print("=" * 80)
    print()

    # Overall statistics
    print("OVERALL COVERAGE:")
    print(f"  Lines Covered:   {overall['lines_covered']:,}")
    print(f"  Lines Coverable: {overall['lines_coverable']:,}")
    print(f"  Coverage:        {overall['coverage']:.2f}%")
    print()

    # Sort crates by coverage (descending)
    sorted_crates = sorted(
        crate_stats.items(),
        key=lambda x: x[1]['coverage'],
        reverse=True
    )

    print("PER-CRATE COVERAGE:")
    print("-" * 80)
    print(f"{'Crate Name':<40} {'Coverage':>10} {'Covered':>10} {'Coverable':>10}")
    print("-" * 80)

    for crate_name, stats in sorted_crates:
        coverage_str = f"{stats['coverage']:.2f}%"
        print(f"{crate_name:<40} {coverage_str:>10} {stats['covered']:>10} {stats['coverable']:>10}")

    print("-" * 80)
    print()

    # Identify critical crates
    # Note: dashflow-core was merged into dashflow (2025)
    critical_crates = [
        'dashflow',
        'dashflow-streaming'
    ]

    print("CRITICAL CRATE ANALYSIS:")
    print("-" * 80)
    for crate_name in critical_crates:
        if crate_name in crate_stats:
            stats = crate_stats[crate_name]
            status = "✅ PASS" if stats['coverage'] >= 85 else "❌ NEEDS IMPROVEMENT"
            print(f"  {crate_name:<30} {stats['coverage']:>6.2f}%  Target: ≥85%  {status}")
        else:
            print(f"  {crate_name:<30} NOT FOUND")
    print("-" * 80)
    print()

    # Coverage gaps (crates below 70%)
    low_coverage_crates = [
        (name, stats) for name, stats in sorted_crates
        if stats['coverage'] < 70.0 and stats['coverable'] > 50
    ]

    if low_coverage_crates:
        print(f"COVERAGE GAPS (< 70%, > 50 coverable lines): {len(low_coverage_crates)} crates")
        print("-" * 80)
        for crate_name, stats in low_coverage_crates[:10]:  # Top 10
            print(f"  {crate_name:<40} {stats['coverage']:>6.2f}%  ({stats['covered']}/{stats['coverable']} lines)")
        if len(low_coverage_crates) > 10:
            print(f"  ... and {len(low_coverage_crates) - 10} more")
        print("-" * 80)

    print()

def main():
    if len(sys.argv) < 2:
        report_path = 'coverage/v1.3.0/tarpaulin-report.json'
    else:
        report_path = sys.argv[1]

    if not Path(report_path).exists():
        print(f"Error: Report file not found: {report_path}", file=sys.stderr)
        sys.exit(1)

    overall, crate_stats = parse_coverage_report(report_path)
    print_coverage_summary(overall, crate_stats)

    # Save detailed data for further analysis
    output_path = 'coverage/v1.3.0/coverage_analysis.json'
    with open(output_path, 'w') as f:
        json.dump({
            'overall': overall,
            'crate_stats': crate_stats
        }, f, indent=2)

    print(f"Detailed analysis saved to: {output_path}")

if __name__ == '__main__':
    main()
