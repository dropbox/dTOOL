#!/usr/bin/env python3
"""
Performance comparison: Python DashFlow Document Loaders

Tests document loading operations without API calls.
Compares CSV, JSON, and text file loading performance.
"""

import json
import time
import statistics
import sys
import os

# Add baseline DashFlow to path
sys.path.insert(0, os.path.expanduser("~/dashflow/libs/dashflow"))
sys.path.insert(0, os.path.expanduser("~/dashflow/libs/core"))
sys.path.insert(0, os.path.expanduser("~/dashflow/libs/community"))

from dashflow_classic.document_loaders import (  # noqa: E402
    TextLoader,
    CSVLoader,
    JSONLoader,
)


def benchmark(name, func, iterations=1000):
    """Run benchmark with warmup and statistical analysis."""
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

    times.sort()
    mean = statistics.mean(times)
    median = statistics.median(times)
    stdev = statistics.stdev(times) if len(times) > 1 else 0.0
    min_time = min(times)
    max_time = max(times)

    return {
        "name": name,
        "iterations": iterations,
        "mean_us": mean,
        "median_us": median,
        "stdev_us": stdev,
        "min_us": min_time,
        "max_us": max_time,
    }


def main():
    print("Python DashFlow Document Loader Benchmarks")
    print("=" * 60)

    # Create temporary test files
    import tempfile
    import csv

    temp_dir = tempfile.mkdtemp()

    # Small text file (100 bytes)
    small_text_path = os.path.join(temp_dir, "small.txt")
    with open(small_text_path, "w") as f:
        f.write("A" * 100)

    # Medium text file (10KB)
    medium_text_path = os.path.join(temp_dir, "medium.txt")
    with open(medium_text_path, "w") as f:
        f.write("B" * 10_000)

    # Large text file (1MB)
    large_text_path = os.path.join(temp_dir, "large.txt")
    with open(large_text_path, "w") as f:
        f.write("C" * 1_000_000)

    # Small CSV file (10 rows)
    small_csv_path = os.path.join(temp_dir, "small.csv")
    with open(small_csv_path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["id", "name", "value"])
        for i in range(10):
            writer.writerow([i, f"name_{i}", f"value_{i}"])

    # Medium CSV file (100 rows)
    medium_csv_path = os.path.join(temp_dir, "medium.csv")
    with open(medium_csv_path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["id", "name", "value"])
        for i in range(100):
            writer.writerow([i, f"name_{i}", f"value_{i}"])

    # Small JSON file (10 records)
    small_json_path = os.path.join(temp_dir, "small.json")
    with open(small_json_path, "w") as f:
        json.dump(
            [{"id": i, "name": f"name_{i}", "value": f"value_{i}"} for i in range(10)],
            f,
        )

    # Medium JSON file (100 records)
    medium_json_path = os.path.join(temp_dir, "medium.json")
    with open(medium_json_path, "w") as f:
        json.dump(
            [{"id": i, "name": f"name_{i}", "value": f"value_{i}"} for i in range(100)],
            f,
        )

    results = []

    # Test 1: TextLoader - Small (100B)
    loader = TextLoader(small_text_path)
    results.append(
        benchmark("TextLoader (small, 100B)", lambda: loader.load(), iterations=1000)
    )

    # Test 2: TextLoader - Medium (10KB)
    loader = TextLoader(medium_text_path)
    results.append(
        benchmark("TextLoader (medium, 10KB)", lambda: loader.load(), iterations=1000)
    )

    # Test 3: TextLoader - Large (1MB)
    loader = TextLoader(large_text_path)
    results.append(
        benchmark("TextLoader (large, 1MB)", lambda: loader.load(), iterations=100)
    )

    # Test 4: CSVLoader - Small (10 rows)
    loader = CSVLoader(small_csv_path)
    results.append(
        benchmark("CSVLoader (small, 10 rows)", lambda: loader.load(), iterations=1000)
    )

    # Test 5: CSVLoader - Medium (100 rows)
    loader = CSVLoader(medium_csv_path)
    results.append(
        benchmark(
            "CSVLoader (medium, 100 rows)", lambda: loader.load(), iterations=1000
        )
    )

    # Test 6: JSONLoader - Small (10 records)
    loader = JSONLoader(small_json_path, jq_schema=".", text_content=False)
    results.append(
        benchmark(
            "JSONLoader (small, 10 records)", lambda: loader.load(), iterations=1000
        )
    )

    # Test 7: JSONLoader - Medium (100 records)
    loader = JSONLoader(medium_json_path, jq_schema=".", text_content=False)
    results.append(
        benchmark(
            "JSONLoader (medium, 100 records)", lambda: loader.load(), iterations=1000
        )
    )

    # Print results
    print(
        f"\n{'Test Name':<50} {'Mean (μs)':<12} {'Median (μs)':<12} {'StdDev (μs)':<12}"
    )
    print("-" * 90)

    for result in results:
        print(
            f"{result['name']:<50} {result['mean_us']:<12.2f} "
            f"{result['median_us']:<12.2f} {result['stdev_us']:<12.2f}"
        )

    # Save results to JSON
    output_file = "document_loader_results_python.json"
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2)

    print(f"\nResults saved to: {output_file}")

    # Cleanup
    import shutil

    shutil.rmtree(temp_dir)


if __name__ == "__main__":
    main()
