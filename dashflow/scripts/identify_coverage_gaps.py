#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Identify specific coverage gaps in dashflow by analyzing tarpaulin JSON output.
This helps prioritize which functions/modules need additional tests.
Note: dashflow-core was merged into dashflow (2025).
"""

import json
import sys
from pathlib import Path
from collections import defaultdict

def analyze_file_coverage(file_data):
    """Analyze coverage for a single file."""
    total_lines = 0
    covered_lines = 0
    uncovered_ranges = []

    current_uncovered = []

    for trace in file_data.get("traces", []):
        line_num = trace.get("line")
        stats = trace.get("stats", {})
        hits = stats.get("Line", 0)

        total_lines += 1

        if hits > 0:
            covered_lines += 1
            # End uncovered range if we were tracking one
            if current_uncovered:
                uncovered_ranges.append((current_uncovered[0], current_uncovered[-1]))
                current_uncovered = []
        else:
            current_uncovered.append(line_num)

    # Add final uncovered range
    if current_uncovered:
        uncovered_ranges.append((current_uncovered[0], current_uncovered[-1]))

    coverage_pct = (covered_lines / total_lines * 100) if total_lines > 0 else 0

    return {
        "total_lines": total_lines,
        "covered_lines": covered_lines,
        "uncovered_lines": total_lines - covered_lines,
        "coverage_pct": coverage_pct,
        "uncovered_ranges": uncovered_ranges
    }

def main():
    # Load tarpaulin JSON
    tarpaulin_file = Path("coverage/v1.3.0/tarpaulin-report.json")
    if not tarpaulin_file.exists():
        print(f"Error: {tarpaulin_file} not found")
        sys.exit(1)

    with open(tarpaulin_file) as f:
        data = json.load(f)

    # Analyze dashflow crate files only (previously dashflow-core)
    core_files = []
    for file_data in data.get("files", []):
        path_parts = file_data.get("path", [])
        path = "/".join(path_parts)

        # Filter for dashflow source files (not tests, not examples)
        # Match paths like /crates/dashflow/src/ but not /crates/dashflow-streaming/src/
        if ("/crates/dashflow/src/" in path or "/dashflow-core/src/" in path) and path.endswith(".rs"):
            analysis = analyze_file_coverage(file_data)
            analysis["path"] = path
            analysis["short_path"] = "/".join(path_parts[path_parts.index("src"):])
            core_files.append(analysis)

    # Sort by uncovered lines (most gaps first)
    core_files.sort(key=lambda x: x["uncovered_lines"], reverse=True)

    # Print report
    print("=" * 80)
    print("DashFlow Core Coverage Gap Analysis")
    print("=" * 80)
    print()

    print(f"Total files analyzed: {len(core_files)}")
    print()

    # Top 20 files with most uncovered lines
    print("Top 20 Files with Most Uncovered Lines:")
    print("-" * 80)
    print(f"{'File':<50} {'Coverage':>10} {'Uncovered':>12}")
    print("-" * 80)

    for file_info in core_files[:20]:
        short_path = file_info["short_path"]
        if len(short_path) > 47:
            short_path = "..." + short_path[-44:]

        print(f"{short_path:<50} {file_info['coverage_pct']:>9.1f}% {file_info['uncovered_lines']:>12}")

    print()
    print("=" * 80)

    # Identify files with < 50% coverage
    low_coverage = [f for f in core_files if f["coverage_pct"] < 50]
    print(f"\nFiles with < 50% coverage: {len(low_coverage)}")

    if low_coverage:
        print("\nLow Coverage Files (Priority for Testing):")
        print("-" * 80)
        for file_info in low_coverage[:10]:
            print(f"\n{file_info['short_path']}")
            print(f"  Coverage: {file_info['coverage_pct']:.1f}%")
            print(f"  Uncovered lines: {file_info['uncovered_lines']}")

            # Show first few uncovered ranges
            ranges = file_info['uncovered_ranges'][:5]
            if ranges:
                print(f"  Uncovered ranges:")
                for start, end in ranges:
                    if start == end:
                        print(f"    Line {start}")
                    else:
                        print(f"    Lines {start}-{end}")

    # Summary statistics
    total_lines = sum(f["total_lines"] for f in core_files)
    total_covered = sum(f["covered_lines"] for f in core_files)
    total_uncovered = sum(f["uncovered_lines"] for f in core_files)
    overall_coverage = (total_covered / total_lines * 100) if total_lines > 0 else 0

    print()
    print("=" * 80)
    print("Summary Statistics")
    print("=" * 80)
    print(f"Total coverable lines: {total_lines}")
    print(f"Covered lines: {total_covered}")
    print(f"Uncovered lines: {total_uncovered}")
    print(f"Overall coverage: {overall_coverage:.2f}%")
    print()

if __name__ == "__main__":
    main()
