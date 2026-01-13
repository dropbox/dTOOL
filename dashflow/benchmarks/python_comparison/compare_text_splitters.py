#!/usr/bin/env python3
"""
Performance comparison: Python DashFlow vs Rust DashFlow (Text Splitters)

Tests CPU-bound text splitting operations without API calls.
"""
import sys
import time
import statistics
from typing import Callable

# Add Python DashFlow to path
sys.path.insert(0, "/Users/ayates/dashflow/libs/text-splitters")

from dashflow_text_splitters import (  # noqa: E402
    CharacterTextSplitter,
    RecursiveCharacterTextSplitter,
)


def benchmark(name: str, func: Callable, iterations: int = 1000) -> dict:
    """Run benchmark and return timing statistics."""
    times = []

    # Warmup
    for _ in range(10):
        func()

    # Measure
    for _ in range(iterations):
        start = time.perf_counter()
        func()
        end = time.perf_counter()
        times.append((end - start) * 1_000_000)  # Convert to microseconds

    return {
        "name": name,
        "iterations": iterations,
        "mean_us": statistics.mean(times),
        "median_us": statistics.median(times),
        "stdev_us": statistics.stdev(times) if len(times) > 1 else 0,
        "min_us": min(times),
        "max_us": max(times),
    }


# Test data
SHORT_TEXT = "Hello world. This is a test. Another sentence here."

MEDIUM_TEXT = """
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu
fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in
culpa qui officia deserunt mollit anim id est laborum.

Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque
laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi
architecto beatae vitae dicta sunt explicabo.
""".strip()

LONG_TEXT = MEDIUM_TEXT * 10  # ~5KB


def main():
    """Run text splitter benchmarks."""
    print("Python DashFlow Text Splitter Benchmarks")
    print("=" * 60)

    results = []

    # Test 1: CharacterTextSplitter - Short text
    splitter = CharacterTextSplitter(chunk_size=50, chunk_overlap=10, separator=". ")
    results.append(
        benchmark(
            "CharacterTextSplitter (short, 50 chars)",
            lambda: splitter.split_text(SHORT_TEXT),
            iterations=1000,
        )
    )

    # Test 2: CharacterTextSplitter - Medium text
    splitter = CharacterTextSplitter(chunk_size=100, chunk_overlap=20, separator="\n\n")
    results.append(
        benchmark(
            "CharacterTextSplitter (medium, 100 chars)",
            lambda: splitter.split_text(MEDIUM_TEXT),
            iterations=1000,
        )
    )

    # Test 3: CharacterTextSplitter - Long text
    splitter = CharacterTextSplitter(chunk_size=200, chunk_overlap=50, separator="\n\n")
    results.append(
        benchmark(
            "CharacterTextSplitter (long, 200 chars)",
            lambda: splitter.split_text(LONG_TEXT),
            iterations=1000,
        )
    )

    # Test 4: RecursiveCharacterTextSplitter - Short text
    splitter = RecursiveCharacterTextSplitter(chunk_size=50, chunk_overlap=10)
    results.append(
        benchmark(
            "RecursiveCharacterTextSplitter (short, 50 chars)",
            lambda: splitter.split_text(SHORT_TEXT),
            iterations=1000,
        )
    )

    # Test 5: RecursiveCharacterTextSplitter - Medium text
    splitter = RecursiveCharacterTextSplitter(chunk_size=100, chunk_overlap=20)
    results.append(
        benchmark(
            "RecursiveCharacterTextSplitter (medium, 100 chars)",
            lambda: splitter.split_text(MEDIUM_TEXT),
            iterations=1000,
        )
    )

    # Test 6: RecursiveCharacterTextSplitter - Long text
    splitter = RecursiveCharacterTextSplitter(chunk_size=200, chunk_overlap=50)
    results.append(
        benchmark(
            "RecursiveCharacterTextSplitter (long, 200 chars)",
            lambda: splitter.split_text(LONG_TEXT),
            iterations=1000,
        )
    )

    # Print results
    print()
    header = f"{'Test Name':<50} {'Mean (μs)':<12} "
    header += f"{'Median (μs)':<12} {'StdDev (μs)':<12}"
    print(header)
    print("-" * 90)
    for result in results:
        line = f"{result['name']:<50} {result['mean_us']:<12.2f} "
        line += f"{result['median_us']:<12.2f} "
        line += f"{result['stdev_us']:<12.2f}"
        print(line)

    # Save results to JSON for Rust comparison
    import json

    output_file = (
        "/Users/ayates/dashflow_rs/benchmarks/python_comparison/"
        "text_splitter_results_python.json"
    )
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2)
    print()
    print(f"Results saved to: {output_file}")


if __name__ == "__main__":
    main()
