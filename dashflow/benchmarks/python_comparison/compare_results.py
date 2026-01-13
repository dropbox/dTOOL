#!/usr/bin/env python3
"""
Compare Rust and Python benchmark results.
"""

import json
from pathlib import Path

def load_results():
    """Load both Rust and Python results."""
    with open("benchmarks/python_comparison/results_rust.json") as f:
        rust_results = json.load(f)

    with open("benchmarks/python_comparison/results_python.json") as f:
        python_results = json.load(f)

    return rust_results, python_results

def compare_benchmarks(rust_results, python_results):
    """Compare matching benchmarks and calculate speedup."""
    comparisons = []

    # Find matching benchmarks
    for name in sorted(set(rust_results.keys()) & set(python_results.keys())):
        rust = rust_results[name]
        python = python_results[name]

        rust_ns = rust['avg_ns']
        python_ns = python['avg_ns']

        speedup = python_ns / rust_ns

        comparisons.append({
            'name': name,
            'rust_ns': rust_ns,
            'python_ns': python_ns,
            'speedup': speedup,
        })

    return comparisons

def format_time(ns):
    """Format nanoseconds in appropriate unit."""
    if ns < 1000:
        return f"{ns:.2f} ns"
    elif ns < 1_000_000:
        return f"{ns/1000:.2f} μs"
    else:
        return f"{ns/1_000_000:.2f} ms"

def print_comparison_table(comparisons):
    """Print comparison table."""
    print("\n" + "=" * 100)
    print("Rust vs Python DashFlow Performance Comparison")
    print("=" * 100)
    print(f"{'Benchmark':<40} {'Rust':>12} {'Python':>12} {'Speedup':>10}")
    print("-" * 100)

    for comp in comparisons:
        rust_time = format_time(comp['rust_ns'])
        python_time = format_time(comp['python_ns'])
        speedup = comp['speedup']

        print(f"{comp['name']:<40} {rust_time:>12} {python_time:>12} {speedup:>9.1f}×")

    print("=" * 100)

def print_summary(comparisons):
    """Print summary statistics."""
    speedups = [c['speedup'] for c in comparisons]

    avg_speedup = sum(speedups) / len(speedups)
    min_speedup = min(speedups)
    max_speedup = max(speedups)
    median_speedup = sorted(speedups)[len(speedups) // 2]

    print("\nSummary Statistics:")
    print("-" * 100)
    print(f"Total benchmarks compared: {len(comparisons)}")
    print(f"Average speedup: {avg_speedup:.1f}×")
    print(f"Median speedup: {median_speedup:.1f}×")
    print(f"Minimum speedup: {min_speedup:.1f}×")
    print(f"Maximum speedup: {max_speedup:.1f}×")
    print("-" * 100)

    # Categorize speedups
    categories = {
        '> 100×': len([s for s in speedups if s > 100]),
        '10-100×': len([s for s in speedups if 10 <= s <= 100]),
        '2-10×': len([s for s in speedups if 2 <= s < 10]),
        '< 2×': len([s for s in speedups if s < 2]),
    }

    print("\nSpeedup Distribution:")
    print("-" * 100)
    for category, count in categories.items():
        percentage = (count / len(speedups)) * 100
        print(f"{category:>10}: {count:3d} benchmarks ({percentage:5.1f}%)")
    print("-" * 100)

def print_top_speedups(comparisons, n=5):
    """Print top N speedups."""
    sorted_comps = sorted(comparisons, key=lambda c: c['speedup'], reverse=True)

    print(f"\nTop {n} Performance Improvements:")
    print("-" * 100)
    for i, comp in enumerate(sorted_comps[:n], 1):
        rust_time = format_time(comp['rust_ns'])
        python_time = format_time(comp['python_ns'])
        print(f"{i}. {comp['name']:<40} {comp['speedup']:>6.1f}× faster ({python_time} → {rust_time})")
    print("-" * 100)

def generate_markdown_report(comparisons):
    """Generate markdown report."""
    report = []
    report.append("# Rust vs Python DashFlow Performance Comparison")
    report.append("")
    report.append("## Summary")
    report.append("")

    speedups = [c['speedup'] for c in comparisons]
    avg_speedup = sum(speedups) / len(speedups)
    min_speedup = min(speedups)
    max_speedup = max(speedups)
    median_speedup = sorted(speedups)[len(speedups) // 2]

    report.append(f"- **Total benchmarks compared**: {len(comparisons)}")
    report.append(f"- **Average speedup**: {avg_speedup:.1f}×")
    report.append(f"- **Median speedup**: {median_speedup:.1f}×")
    report.append(f"- **Minimum speedup**: {min_speedup:.1f}×")
    report.append(f"- **Maximum speedup**: {max_speedup:.1f}×")
    report.append("")

    # Speedup distribution
    report.append("## Speedup Distribution")
    report.append("")
    categories = {
        '> 100×': len([s for s in speedups if s > 100]),
        '10-100×': len([s for s in speedups if 10 <= s <= 100]),
        '2-10×': len([s for s in speedups if 2 <= s < 10]),
        '< 2×': len([s for s in speedups if s < 2]),
    }

    for category, count in categories.items():
        percentage = (count / len(speedups)) * 100
        report.append(f"- **{category}**: {count} benchmarks ({percentage:.1f}%)")
    report.append("")

    # Top speedups
    sorted_comps = sorted(comparisons, key=lambda c: c['speedup'], reverse=True)
    report.append("## Top 10 Performance Improvements")
    report.append("")
    report.append("| Rank | Benchmark | Speedup | Python | Rust |")
    report.append("|------|-----------|---------|--------|------|")

    for i, comp in enumerate(sorted_comps[:10], 1):
        rust_time = format_time(comp['rust_ns'])
        python_time = format_time(comp['python_ns'])
        report.append(f"| {i} | {comp['name']} | {comp['speedup']:.1f}× | {python_time} | {rust_time} |")
    report.append("")

    # Full comparison table
    report.append("## Full Comparison Table")
    report.append("")
    report.append("| Benchmark | Rust | Python | Speedup |")
    report.append("|-----------|------|--------|---------|")

    for comp in comparisons:
        rust_time = format_time(comp['rust_ns'])
        python_time = format_time(comp['python_ns'])
        report.append(f"| {comp['name']} | {rust_time} | {python_time} | {comp['speedup']:.1f}× |")
    report.append("")

    return "\n".join(report)

def main():
    """Main comparison function."""
    rust_results, python_results = load_results()
    comparisons = compare_benchmarks(rust_results, python_results)

    # Print comparison table
    print_comparison_table(comparisons)

    # Print summary statistics
    print_summary(comparisons)

    # Print top speedups
    print_top_speedups(comparisons, n=10)

    # Generate markdown report
    markdown_report = generate_markdown_report(comparisons)

    # Save markdown report
    report_file = "benchmarks/python_comparison/COMPARISON_REPORT.md"
    with open(report_file, 'w') as f:
        f.write(markdown_report)
    print(f"\nMarkdown report saved to: {report_file}")

    # Save JSON comparison
    comparison_file = "benchmarks/python_comparison/comparison_results.json"
    with open(comparison_file, 'w') as f:
        json.dump(comparisons, f, indent=2)
    print(f"JSON comparison saved to: {comparison_file}")

if __name__ == "__main__":
    main()
