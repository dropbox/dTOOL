#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Test Rigor Analysis Tool
Counts assertions in Rust test functions and classifies test quality.
"""
import re
import sys

def count_assertions(test_code):
    """Count assertions in test code."""
    patterns = [
        r'\bassert!',
        r'\bassert_eq!',
        r'\bassert_ne!',
        r'\bassert_matches!',
    ]
    count = 0
    for pattern in patterns:
        count += len(re.findall(pattern, test_code))
    return count

def classify_test(assertion_count):
    """Classify test as TRIVIAL, ADEQUATE, or RIGOROUS."""
    if assertion_count <= 2:
        return "TRIVIAL"
    elif assertion_count <= 7:
        return "ADEQUATE"
    else:
        return "RIGOROUS"

def analyze_file(filepath):
    """Analyze test file and report metrics."""
    with open(filepath, 'r') as f:
        content = f.read()

    # Find all test functions
    test_pattern = r'#\[test\]\s+fn\s+(\w+)\s*\(\s*\)\s*\{([^}]*(?:\{[^}]*\}[^}]*)*)\}'
    tests = re.findall(test_pattern, content, re.DOTALL)

    results = []
    for test_name, test_body in tests:
        assertion_count = count_assertions(test_body)
        classification = classify_test(assertion_count)
        results.append((test_name, assertion_count, classification))

    # Sort by line number (roughly by name for now)
    results.sort(key=lambda x: x[0])

    # Print results
    print(f"Test Rigor Analysis: {filepath}")
    print("=" * 80)
    print(f"{'Test Name':<50} {'Assertions':>10}  {'Classification':>15}")
    print("-" * 80)

    trivial = 0
    adequate = 0
    rigorous = 0

    for test_name, count, classification in results:
        print(f"{test_name:<50} {count:>10}  {classification:>15}")
        if classification == "TRIVIAL":
            trivial += 1
        elif classification == "ADEQUATE":
            adequate += 1
        else:
            rigorous += 1

    total = len(results)
    print("=" * 80)
    print()
    print("Summary:")
    print(f"  Total tests: {total}")
    print(f"  TRIVIAL: {trivial} ({100*trivial/total if total else 0:.1f}%)")
    print(f"  ADEQUATE: {adequate} ({100*adequate/total if total else 0:.1f}%)")
    print(f"  RIGOROUS: {rigorous} ({100*rigorous/total if total else 0:.1f}%)")
    print(f"  Rigor: {100*rigorous/total if total else 0:.1f}%")

    if adequate > 0:
        print()
        print("ADEQUATE tests to upgrade:")
        for i, (test_name, count, classification) in enumerate(results, 1):
            if classification == "ADEQUATE":
                print(f"  {i}. {test_name}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: count_test_assertions.py <rust_file>")
        sys.exit(1)

    analyze_file(sys.argv[1])
