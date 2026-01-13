#!/usr/bin/env python3
"""
Performance comparison: Python DashFlow vs Rust DashFlow (Output Parsers)

Tests CPU-bound parser operations without API calls.
"""
import sys
import time
import statistics
from typing import Callable

# Add Python DashFlow to path
sys.path.insert(0, "/Users/ayates/dashflow/libs/core")
sys.path.insert(0, "/Users/ayates/dashflow/libs/dashflow")

from dashflow_core.output_parsers.list import (  # noqa: E402
    CommaSeparatedListOutputParser,
)
from dashflow_core.output_parsers.json import (  # noqa: E402
    SimpleJsonOutputParser,
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


def main():
    """Run parser benchmarks."""
    print("Python DashFlow Parser Benchmarks")
    print("=" * 60)

    results = []

    # Test 1: CommaSeparatedListOutputParser - Simple
    parser = CommaSeparatedListOutputParser()
    simple_input = "apple, banana, cherry"
    results.append(
        benchmark(
            "CommaSeparatedListOutputParser (simple)",
            lambda: parser.parse(simple_input),
            iterations=1000,
        )
    )

    # Test 2: CommaSeparatedListOutputParser - Complex
    complex_input = (
        "apple, banana, cherry, date, elderberry, fig, grape, honeydew, kiwi, lemon"
    )
    results.append(
        benchmark(
            "CommaSeparatedListOutputParser (complex)",
            lambda: parser.parse(complex_input),
            iterations=1000,
        )
    )

    # Test 3: SimpleJsonOutputParser - Simple
    json_parser = SimpleJsonOutputParser()
    simple_json = '{"name": "Alice", "age": 30}'
    results.append(
        benchmark(
            "SimpleJsonOutputParser (simple)",
            lambda: json_parser.parse(simple_json),
            iterations=1000,
        )
    )

    # Test 4: SimpleJsonOutputParser - Complex
    complex_json = """
    {
        "person": {
            "name": "Alice",
            "age": 30,
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "country": "USA"
            },
            "hobbies": ["reading", "hiking", "coding"]
        }
    }
    """
    results.append(
        benchmark(
            "SimpleJsonOutputParser (complex)",
            lambda: json_parser.parse(complex_json),
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
        "parser_results_python.json"
    )
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2)
    print()
    print(f"Results saved to: {output_file}")


if __name__ == "__main__":
    main()
