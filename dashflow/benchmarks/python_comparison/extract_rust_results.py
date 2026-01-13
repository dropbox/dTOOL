#!/usr/bin/env python3
"""
Extract Rust benchmark results from Criterion output.
"""

import json
import os
from pathlib import Path

def extract_criterion_results():
    """Extract all benchmark results from criterion output."""
    results = {}
    criterion_dir = Path("target/criterion")

    # Find all estimates.json files in base directories
    for group_dir in criterion_dir.iterdir():
        if not group_dir.is_dir() or group_dir.name == 'report':
            continue

        group = group_dir.name

        # Iterate through benchmarks in this group
        for bench_dir in group_dir.iterdir():
            if not bench_dir.is_dir() or bench_dir.name == 'report':
                continue

            bench_name = bench_dir.name
            estimates_file = bench_dir / "base" / "estimates.json"

            if not estimates_file.exists():
                continue

            # Read the estimates file
            with open(estimates_file) as f:
                data = json.load(f)

            # Extract mean estimate (in nanoseconds)
            mean_ns = data['mean']['point_estimate']

            results[bench_name] = {
                'group': group,
                'avg_ns': mean_ns,
                'avg_us': mean_ns / 1000,
                'avg_ms': mean_ns / 1_000_000,
            }

    return results

def main():
    """Extract and save Rust benchmark results."""
    results = extract_criterion_results()

    # Print results
    print("Rust DashFlow Benchmark Results")
    print("=" * 80)

    for name, result in sorted(results.items()):
        avg_ns = result['avg_ns']
        avg_us = result['avg_us']

        if avg_ns < 1000:
            print(f"{name:50s} {avg_ns:10.2f} ns")
        elif avg_us < 1000:
            print(f"{name:50s} {avg_us:10.2f} μs")
        else:
            print(f"{name:50s} {result['avg_ms']:10.2f} ms")

    print("=" * 80)
    print(f"Total benchmarks: {len(results)}")
    print("=" * 80)

    # Save to JSON
    output_file = "benchmarks/python_comparison/results_rust.json"
    with open(output_file, 'w') as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to: {output_file}")

if __name__ == "__main__":
    main()
