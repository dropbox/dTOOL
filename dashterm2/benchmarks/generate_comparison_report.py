#!/usr/bin/env python3
"""Generate terminal comparison report from benchmark JSON files.

Usage:
    python3 generate_comparison_report.py [results_dir] [--output report.md]

Author: DashTerm2 Project
Created: Iteration #147
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime
from pathlib import Path


def load_results(results_dir: Path) -> dict[str, dict]:
    """Load all JSON benchmark results from directory."""
    results = {}

    for json_file in results_dir.glob("*.json"):
        try:
            with open(json_file) as f:
                data = json.load(f)

            terminal_name = data.get("terminal", {}).get("name", "unknown")
            timestamp = data.get("timestamp", "unknown")

            # Use terminal name as key, keep most recent
            key = terminal_name
            if key not in results or timestamp > results[key].get("timestamp", ""):
                results[key] = data
                results[key]["_file"] = str(json_file)
        except (json.JSONDecodeError, KeyError) as e:
            print(f"Warning: Could not parse {json_file}: {e}", file=sys.stderr)

    return results


def format_ms(value: float | None) -> str:
    """Format milliseconds with 1 decimal place."""
    if value is None:
        return "N/A"
    return f"{value:.1f}"


def generate_report(results: dict[str, dict], output_path: Path | None = None) -> str:
    """Generate markdown comparison report."""
    lines = []

    # Header
    lines.append("# Terminal Emulator Performance Comparison Report")
    lines.append("")
    lines.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")

    if not results:
        lines.append("*No benchmark results found.*")
        return "\n".join(lines)

    # System info (from first result)
    first = next(iter(results.values()))
    system = first.get("system", {})
    lines.append("## System Information")
    lines.append("")
    lines.append(f"- **CPU:** {system.get('chip', 'Unknown')}")
    lines.append(f"- **Cores:** {system.get('cores', 'Unknown')}")
    lines.append(f"- **Memory:** {system.get('memory', 'Unknown')}")
    lines.append("")

    # Terminals tested
    lines.append("## Terminals Tested")
    lines.append("")
    for name, data in sorted(results.items()):
        term = data.get("terminal", {})
        lines.append(f"- **{name}** v{term.get('version', 'unknown')}")
    lines.append("")

    # Collect all benchmark names
    all_benchmarks = set()
    for data in results.values():
        for r in data.get("results", []):
            all_benchmarks.add(r.get("name", ""))

    # Performance table
    lines.append("## Performance Comparison (mean time in ms)")
    lines.append("")

    # Create table header
    terminals = sorted(results.keys())
    header = "| Benchmark |"
    separator = "|-----------|"
    for t in terminals:
        header += f" {t} |"
        separator += "--------:|"

    lines.append(header)
    lines.append(separator)

    # Create table rows
    for bench_name in sorted(all_benchmarks):
        if not bench_name:
            continue
        row = f"| {bench_name} |"
        for terminal in terminals:
            data = results[terminal]
            value = None
            for r in data.get("results", []):
                if r.get("name") == bench_name:
                    value = r.get("mean_ms")
                    break
            row += f" {format_ms(value)} |"
        lines.append(row)

    lines.append("")

    # Analysis section
    lines.append("## Analysis")
    lines.append("")

    if len(terminals) >= 2:
        # Find best terminal for each benchmark
        lines.append("### Best Performer by Benchmark")
        lines.append("")
        for bench_name in sorted(all_benchmarks):
            if not bench_name:
                continue
            best_terminal = None
            best_time = float('inf')
            for terminal in terminals:
                for r in results[terminal].get("results", []):
                    if r.get("name") == bench_name:
                        t = r.get("mean_ms", float('inf'))
                        if t < best_time:
                            best_time = t
                            best_terminal = terminal
                        break
            if best_terminal:
                lines.append(f"- **{bench_name}:** {best_terminal} ({format_ms(best_time)}ms)")
        lines.append("")

    # Raw data section
    lines.append("## Raw Data")
    lines.append("")
    for name, data in sorted(results.items()):
        lines.append(f"### {name}")
        lines.append("")
        lines.append("```json")
        lines.append(json.dumps(data.get("results", []), indent=2))
        lines.append("```")
        lines.append("")

    report = "\n".join(lines)

    if output_path:
        with open(output_path, "w") as f:
            f.write(report)
        print(f"Report written to: {output_path}", file=sys.stderr)

    return report


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate terminal comparison report from benchmark results."
    )
    parser.add_argument(
        "results_dir",
        nargs="?",
        default="benchmarks/results/comparison",
        help="Directory containing benchmark JSON files"
    )
    parser.add_argument(
        "--output", "-o",
        help="Output path for markdown report"
    )

    args = parser.parse_args()

    results_dir = Path(args.results_dir)
    if not results_dir.exists():
        print(f"Error: Results directory not found: {results_dir}", file=sys.stderr)
        return 1

    results = load_results(results_dir)

    output_path = Path(args.output) if args.output else None
    report = generate_report(results, output_path)

    if not output_path:
        print(report)

    return 0


if __name__ == "__main__":
    sys.exit(main())
