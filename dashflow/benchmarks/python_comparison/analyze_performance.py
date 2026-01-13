#!/usr/bin/env python3
"""
Performance Analysis Tool - Rust vs Python DashFlow
Compares benchmark results and generates detailed analysis report.
"""

import json
from pathlib import Path


def load_results(rust_path, python_path):
    """Load Rust and Python benchmark results."""
    with open(rust_path) as f:
        rust_data = json.load(f)
    with open(python_path) as f:
        python_data = json.load(f)

    # Normalize data format (handle both dict with "results" and list formats)
    rust_results = (
        rust_data.get("results", rust_data)
        if isinstance(rust_data, dict)
        else rust_data
    )
    python_results = (
        python_data if isinstance(python_data, list) else python_data.get("results", [])
    )

    return rust_results, python_results


def calculate_speedup(rust_mean, python_mean):
    """Calculate speedup factor (Python / Rust)."""
    if rust_mean == 0:
        return float("inf")
    return python_mean / rust_mean


def format_speedup(speedup):
    """Format speedup factor for display."""
    if speedup >= 1000:
        return f"{speedup:.0f}×"
    elif speedup >= 100:
        return f"{speedup:.0f}×"
    elif speedup >= 10:
        return f"{speedup:.1f}×"
    else:
        return f"{speedup:.2f}×"


def analyze_benchmarks(category_name, rust_path, python_path):
    """Analyze a category of benchmarks."""
    print("\n" + "=" * 80)
    print(f"{category_name} Performance Analysis")
    print("=" * 80 + "\n")

    rust_results, python_results = load_results(rust_path, python_path)

    # Match tests by name
    python_by_name = {r["name"]: r for r in python_results}

    print(f"{'Test Name':<55} {'Rust (μs)':<12} {'Python (μs)':<12} {'Speedup':<10}")
    print("-" * 90)

    speedups = []
    for rust_test in rust_results:
        name = rust_test["name"]
        rust_mean = rust_test["mean_us"]

        if name in python_by_name:
            python_mean = python_by_name[name]["mean_us"]
            speedup = calculate_speedup(rust_mean, python_mean)
            speedups.append((name, speedup))

            print(
                f"{name:<55} {rust_mean:>10.2f}  {python_mean:>10.2f}  {format_speedup(speedup):>8}"
            )

    if speedups:
        print("\n" + "-" * 90)
        avg_speedup = sum(s[1] for s in speedups) / len(speedups)
        min_speedup = min(speedups, key=lambda x: x[1])
        max_speedup = max(speedups, key=lambda x: x[1])

        print("\nSummary:")
        print(f"  Average Speedup: {format_speedup(avg_speedup)}")
        print(f"  Min Speedup:     {format_speedup(min_speedup[1])} ({min_speedup[0]})")
        print(f"  Max Speedup:     {format_speedup(max_speedup[1])} ({max_speedup[0]})")

        return avg_speedup, speedups

    return None, []


def main():
    """Main analysis function."""
    base_dir = Path(__file__).parent

    print("\n" + "=" * 80)
    print("DashFlow Performance Comparison: Rust vs Python")
    print("=" * 80)

    all_speedups = []

    # Analyze Output Parsers
    try:
        avg, speedups = analyze_benchmarks(
            "Output Parsers",
            base_dir / "parser_results_rust.json",
            base_dir / "parser_results_python.json",
        )
        if avg:
            all_speedups.extend(speedups)
    except FileNotFoundError as e:
        print(f"\nSkipping Output Parsers: {e}")

    # Analyze Text Splitters
    try:
        avg, speedups = analyze_benchmarks(
            "Text Splitters",
            base_dir / "text_splitter_results_rust.json",
            base_dir / "text_splitter_results_python.json",
        )
        if avg:
            all_speedups.extend(speedups)
    except FileNotFoundError as e:
        print(f"\nSkipping Text Splitters: {e}")

    # Analyze Document Loaders (if available)
    try:
        avg, speedups = analyze_benchmarks(
            "Document Loaders",
            base_dir / "document_loader_results_rust.json",
            base_dir / "document_loader_results_python.json",
        )
        if avg:
            all_speedups.extend(speedups)
    except FileNotFoundError as e:
        print(f"\nSkipping Document Loaders: {e}")

    # Overall summary
    if all_speedups:
        print("\n" + "=" * 80)
        print("Overall Performance Summary")
        print("=" * 80)

        overall_avg = sum(s[1] for s in all_speedups) / len(all_speedups)
        print(f"\nTotal Tests Compared: {len(all_speedups)}")
        print(f"Overall Average Speedup: {format_speedup(overall_avg)}")
        print(f"\nRust is {format_speedup(overall_avg)} faster than Python on average")

        # Categorize speedups
        extreme = [s for s in all_speedups if s[1] >= 10]
        high = [s for s in all_speedups if 5 <= s[1] < 10]
        moderate = [s for s in all_speedups if 2 <= s[1] < 5]
        low = [s for s in all_speedups if s[1] < 2]

        print("\nSpeedup Distribution:")
        print(
            f"  10×+ faster:   {len(extreme)} tests ({len(extreme)/len(all_speedups)*100:.0f}%)"
        )
        print(
            f"  5-10× faster:  {len(high)} tests ({len(high)/len(all_speedups)*100:.0f}%)"
        )
        print(
            f"  2-5× faster:   {len(moderate)} tests ({len(moderate)/len(all_speedups)*100:.0f}%)"
        )
        print(
            f"  <2× faster:    {len(low)} tests ({len(low)/len(all_speedups)*100:.0f}%)"
        )

        if extreme:
            print("\nTop 5 Performance Wins (10×+ faster):")
            for name, speedup in sorted(extreme, key=lambda x: x[1], reverse=True)[:5]:
                print(f"  - {name}: {format_speedup(speedup)}")

        print("\n" + "=" * 80 + "\n")


if __name__ == "__main__":
    main()
