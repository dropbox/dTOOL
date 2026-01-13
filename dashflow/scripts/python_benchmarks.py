#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Python DashFlow Benchmarks for Comparison with Rust Implementation

This script benchmarks the same operations as the Rust benchmarks in
crates/dashflow-benchmarks/benches/core_benchmarks.rs to allow direct
performance comparison.

Run with:
    python3 scripts/python_benchmarks.py

Requirements:
    pip install dashflow-text-splitters timeit
"""

import time
import statistics
from typing import List, Callable
import sys

try:
    from dashflow_text_splitters import (
        CharacterTextSplitter,
        RecursiveCharacterTextSplitter,
    )
except ImportError:
    print("Error: dashflow-text-splitters not installed")
    print("Install with: pip install dashflow-text-splitters")
    sys.exit(1)


def benchmark(name: str, fn: Callable, iterations: int = 100) -> dict:
    """Run a benchmark and return timing statistics"""
    times = []

    # Warmup
    for _ in range(5):
        fn()

    # Actual benchmark
    for _ in range(iterations):
        start = time.perf_counter()
        fn()
        end = time.perf_counter()
        times.append((end - start) * 1_000_000)  # Convert to microseconds

    return {
        "name": name,
        "mean_us": statistics.mean(times),
        "median_us": statistics.median(times),
        "stdev_us": statistics.stdev(times) if len(times) > 1 else 0,
        "min_us": min(times),
        "max_us": max(times),
        "iterations": iterations,
    }


def bench_text_splitters():
    """Benchmark text splitters (matching Rust benchmarks)"""
    print("=" * 80)
    print("Text Splitter Benchmarks")
    print("=" * 80)

    # Generate test texts matching Rust benchmarks
    small_text = "This is a short paragraph. " * 10  # ~260 chars
    medium_text = "This is a medium paragraph with more content. " * 100  # ~4700 chars
    large_text = "This is a large document with lots of text. " * 1000  # ~47000 chars

    print(f"\nTest text sizes:")
    print(f"  Small:  {len(small_text):,} chars")
    print(f"  Medium: {len(medium_text):,} chars")
    print(f"  Large:  {len(large_text):,} chars")
    print()

    results = []

    # CharacterTextSplitter - small text
    splitter_small = CharacterTextSplitter(
        chunk_size=100,
        chunk_overlap=20,
        separator="\n\n",
    )
    results.append(
        benchmark(
            "character_splitter_small",
            lambda: splitter_small.split_text(small_text),
            iterations=100,
        )
    )

    # CharacterTextSplitter - medium text
    splitter_medium = CharacterTextSplitter(
        chunk_size=500,
        chunk_overlap=50,
        separator="\n\n",
    )
    results.append(
        benchmark(
            "character_splitter_medium",
            lambda: splitter_medium.split_text(medium_text),
            iterations=100,
        )
    )

    # CharacterTextSplitter - large text
    splitter_large = CharacterTextSplitter(
        chunk_size=1000,
        chunk_overlap=100,
        separator="\n\n",
    )
    results.append(
        benchmark(
            "character_splitter_large",
            lambda: splitter_large.split_text(large_text),
            iterations=100,
        )
    )

    # RecursiveCharacterTextSplitter - medium text
    recursive_medium = RecursiveCharacterTextSplitter(
        chunk_size=500,
        chunk_overlap=50,
    )
    results.append(
        benchmark(
            "recursive_splitter_medium",
            lambda: recursive_medium.split_text(medium_text),
            iterations=100,
        )
    )

    # RecursiveCharacterTextSplitter - large text
    recursive_large = RecursiveCharacterTextSplitter(
        chunk_size=1000,
        chunk_overlap=100,
    )
    results.append(
        benchmark(
            "recursive_splitter_large",
            lambda: recursive_large.split_text(large_text),
            iterations=100,
        )
    )

    # Print results
    print(f"{'Benchmark':<40} {'Mean (µs)':<15} {'Median (µs)':<15} {'Throughput':<20}")
    print("-" * 90)

    for result in results:
        name = result["name"]
        mean = result["mean_us"]
        median = result["median_us"]

        # Calculate throughput (chars/sec)
        if "small" in name:
            text_len = len(small_text)
        elif "medium" in name:
            text_len = len(medium_text)
        elif "large" in name:
            text_len = len(large_text)
        else:
            text_len = 0

        throughput_chars_per_sec = (text_len / (mean / 1_000_000)) if mean > 0 else 0
        throughput_str = f"{throughput_chars_per_sec/1000:.1f}K chars/sec"

        print(f"{name:<40} {mean:>12.2f}    {median:>12.2f}    {throughput_str:<20}")

    return results


def main():
    print("Python DashFlow Benchmarks")
    print("Comparing to Rust implementation\n")

    # Check Python version
    print(f"Python version: {sys.version}")

    # Check dashflow-text-splitters version
    try:
        import dashflow_text_splitters
        print(f"dashflow-text-splitters version: {dashflow_text_splitters.__version__}")
    except AttributeError:
        print("dashflow-text-splitters version: unknown")

    print()

    # Run benchmarks
    text_splitter_results = bench_text_splitters()

    # Summary
    print("\n" + "=" * 80)
    print("Summary")
    print("=" * 80)
    print(f"\nTotal benchmarks run: {len(text_splitter_results)}")
    print("\nResults saved for comparison with Rust benchmarks.")
    print("\nTo compare with Rust:")
    print("  1. Run Rust benchmarks: cargo bench -p dashflow-benchmarks text_splitters")
    print("  2. Compare the mean times above with Rust criterion output")
    print("  3. Calculate speedup: Python_time / Rust_time")

    # Save results to file for automated comparison
    import json
    output_file = "benchmark_results_python.json"
    with open(output_file, "w") as f:
        json.dump(text_splitter_results, f, indent=2)
    print(f"\nResults saved to: {output_file}")


if __name__ == "__main__":
    main()
