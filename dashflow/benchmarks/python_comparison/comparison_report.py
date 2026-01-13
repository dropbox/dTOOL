#!/usr/bin/env python3
"""
Generate performance comparison report between Python and Rust DashFlow implementations.
"""
import json


def load_results(python_file, rust_file):
    """Load benchmark results from JSON files."""
    with open(python_file) as f:
        python_results = json.load(f)

    with open(rust_file) as f:
        rust_data = json.load(f)
        rust_results = rust_data["results"]

    return python_results, rust_results


def calculate_speedup(python_time, rust_time):
    """Calculate speedup factor (positive = Rust faster, negative = Python faster)."""
    if rust_time == 0:
        return float("inf")
    return python_time / rust_time


def generate_report(python_results, rust_results):
    """Generate comparison report."""
    print("=" * 100)
    print("PERFORMANCE COMPARISON: Python DashFlow vs Rust DashFlow")
    print("=" * 100)
    print()

    header = f"{'Test Name':<50} {'Python (μs)':<15} "
    header += f"{'Rust (μs)':<15} {'Speedup':<15} {'Winner':<10}"
    print(header)
    print("-" * 100)

    speedups = []

    for py_result, rust_result in zip(python_results, rust_results):
        assert (
            py_result["name"] == rust_result["name"]
        ), f"Mismatch: {py_result['name']} vs {rust_result['name']}"

        py_time = py_result["mean_us"]
        rust_time = rust_result["mean_us"]
        speedup = calculate_speedup(py_time, rust_time)
        speedups.append((py_result["name"], speedup))

        if speedup > 1:
            speedup_str = f"{speedup:.2f}×"
            winner = "Rust 🦀"
        elif speedup < 1:
            speedup_str = f"{1/speedup:.2f}× slower"
            winner = "Python 🐍"
        else:
            speedup_str = "1.00×"
            winner = "Tie"

        line = f"{py_result['name']:<50} {py_time:<15.2f} "
        line += f"{rust_time:<15.2f} {speedup_str:<15} {winner:<10}"
        print(line)

    print()
    print("=" * 100)
    print("SUMMARY")
    print("=" * 100)

    rust_wins = sum(1 for _, s in speedups if s > 1)
    python_wins = sum(1 for _, s in speedups if s < 1)

    print(f"Rust wins: {rust_wins}/{len(speedups)}")
    print(f"Python wins: {python_wins}/{len(speedups)}")
    print()

    if rust_wins > 0:
        avg_rust_speedup = sum(s for _, s in speedups if s > 1) / rust_wins
        print(f"Average Rust speedup (when faster): {avg_rust_speedup:.2f}×")

    if python_wins > 0:
        avg_python_speedup = sum(1 / s for _, s in speedups if s < 1) / python_wins
        print(f"Average Python speedup (when faster): {avg_python_speedup:.2f}×")

    print()
    print("=" * 100)
    print("OBSERVATIONS")
    print("=" * 100)
    print()

    # Identify unexpected results
    print("Unexpected findings:")
    for name, speedup in speedups:
        if "CommaSeparated" in name and speedup < 1:
            msg = f"  - {name}: Python is {1/speedup:.2f}× faster "
            msg += "(unexpected for simple string operations)"
            print(msg)
        elif "Json" in name and speedup > 1:
            msg = f"  - {name}: Rust is {speedup:.2f}× faster "
            msg += "(expected for serde_json)"
            print(msg)

    print()
    print("Potential optimizations:")
    for name, speedup in speedups:
        if speedup < 1:
            msg = f"  - {name}: Investigate Rust implementation for "
            msg += "performance bottlenecks"
            print(msg)


if __name__ == "__main__":
    python_file = "benchmarks/python_comparison/parser_results_python.json"
    rust_file = "benchmarks/python_comparison/parser_results_rust.json"

    python_results, rust_results = load_results(python_file, rust_file)
    generate_report(python_results, rust_results)
