#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Compare text outputs from Python and Rust implementations.

Uses semantic similarity to measure equivalence of responses,
focusing on content rather than exact text matching.
"""

import argparse
import os
import sys
from pathlib import Path
from difflib import SequenceMatcher


def clean_text(text: str) -> str:
    """
    Clean text for comparison by:
    - Removing debug messages
    - Removing timestamps
    - Normalizing whitespace
    - Converting to lowercase for case-insensitive comparison
    """
    lines = []
    # Use os.path.expanduser to get the current user's home directory
    home_dir = os.path.expanduser("~")
    skip_patterns = [
        "USER_AGENT environment",
        "DeprecationWarning:",
        "DashFlowDeprecatedSinceV10:",
        "Compiling",
        "Finished",
        "🤖 [ASSISTANT]",
        "---CALL AGENT---",
        "Output from node",
        "Prompt[",
        "/Library/Frameworks",
        home_dir,  # Dynamically filter current user's home path
        "site-packages",
        "frozen abc",
    ]

    for line in text.splitlines():
        # Skip debug/warning lines
        if any(pattern in line for pattern in skip_patterns):
            continue

        # Skip empty lines
        if not line.strip():
            continue

        lines.append(line.strip())

    # Join and normalize whitespace
    cleaned = " ".join(lines)
    # Remove multiple spaces
    cleaned = " ".join(cleaned.split())
    return cleaned.lower()


def calculate_similarity(text1: str, text2: str) -> float:
    """
    Calculate similarity between two texts using SequenceMatcher.

    Returns:
        Similarity score between 0.0 (no match) and 1.0 (exact match)
    """
    # Clean both texts
    cleaned1 = clean_text(text1)
    cleaned2 = clean_text(text2)

    # Use SequenceMatcher for similarity
    similarity = SequenceMatcher(None, cleaned1, cleaned2).ratio()
    return similarity


def compare_files(python_file: Path, rust_file: Path, threshold: float = 0.8) -> dict:
    """
    Compare two output files and return comparison results.

    Args:
        python_file: Path to Python baseline output
        rust_file: Path to Rust implementation output
        threshold: Minimum similarity threshold (0.0 to 1.0)

    Returns:
        dict with keys: similarity, passed, python_text, rust_text, cleaned_python, cleaned_rust
    """
    if not python_file.exists():
        return {
            "error": f"Python file not found: {python_file}",
            "passed": False,
        }

    if not rust_file.exists():
        return {
            "error": f"Rust file not found: {rust_file}",
            "passed": False,
        }

    python_text = python_file.read_text()
    rust_text = rust_file.read_text()

    similarity = calculate_similarity(python_text, rust_text)
    passed = similarity >= threshold

    return {
        "similarity": similarity,
        "passed": passed,
        "python_text": python_text,
        "rust_text": rust_text,
        "cleaned_python": clean_text(python_text),
        "cleaned_rust": clean_text(rust_text),
        "threshold": threshold,
    }


def main():
    parser = argparse.ArgumentParser(description="Compare Python and Rust outputs")
    parser.add_argument(
        "--python",
        type=Path,
        required=True,
        help="Path to Python baseline output file",
    )
    parser.add_argument(
        "--rust",
        type=Path,
        required=True,
        help="Path to Rust implementation output file",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.8,
        help="Similarity threshold (0.0-1.0, default: 0.8)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Show cleaned text comparison",
    )

    args = parser.parse_args()

    result = compare_files(args.python, args.rust, args.threshold)

    if "error" in result:
        print(f"ERROR: {result['error']}")
        sys.exit(1)

    similarity = result["similarity"]
    passed = result["passed"]

    print(f"Similarity: {similarity:.1%}")
    print(f"Threshold: {result['threshold']:.1%}")
    print(f"Status: {'✓ PASSED' if passed else '✗ FAILED'}")

    if args.verbose:
        print("\n=== Cleaned Python Text ===")
        print(result["cleaned_python"][:500])
        print("\n=== Cleaned Rust Text ===")
        print(result["cleaned_rust"][:500])

    sys.exit(0 if passed else 1)


if __name__ == "__main__":
    main()
