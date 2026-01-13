#!/usr/bin/env python3
"""Emit fixed-length lines for benchmark inputs.

Modes:
  - Single: Emit lines with fixed dimensions (default)
  - Sweep: Test multiple width/line combinations and report timing

Author: DashTerm2 Project
Updated: Iteration #147
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from typing import TextIO


def build_line(pattern: str, columns: int) -> str:
    if not pattern:
        pattern = "X"
    # Repeat pattern until we have enough characters, then trim.
    repeated = (pattern * ((columns + len(pattern) - 1) // len(pattern)))[:columns]
    return repeated + "\n"


def emit_lines(writer: TextIO, line: str, count: int) -> None:
    """Emit lines to the writer."""
    for _ in range(count):
        writer.write(line)
    writer.flush()


def run_single(args: argparse.Namespace) -> int:
    """Standard mode: emit lines to stdout."""
    if args.lines < 0 or args.columns < 0:
        raise SystemExit("lines and columns must be non-negative")

    line = build_line(args.pattern, args.columns)
    emit_lines(sys.stdout, line, args.lines)
    return 0


def run_sweep(args: argparse.Namespace) -> int:
    """Sweep mode: test multiple configurations and report generator timing.

    This measures the generator's own overhead, NOT terminal rendering time.
    To measure terminal rendering, redirect output to the terminal under test.
    """
    # Parse sweep ranges
    widths = parse_range(args.widths)
    line_counts = parse_range(args.line_counts)

    results = []

    print(f"Long-line generator sweep benchmark", file=sys.stderr)
    print(f"Widths: {widths}", file=sys.stderr)
    print(f"Line counts: {line_counts}", file=sys.stderr)
    print(f"Iterations per config: {args.iterations}", file=sys.stderr)
    print("", file=sys.stderr)

    # Header
    print(f"{'Width':>8} {'Lines':>8} {'Mean (ms)':>12} {'Stddev':>10} {'MB/s':>10}", file=sys.stderr)
    print("-" * 52, file=sys.stderr)

    for width in widths:
        line = build_line(args.pattern, width)
        for line_count in line_counts:
            times_ms = []
            total_bytes = width * line_count

            for _ in range(args.iterations):
                # Write to /dev/null to measure pure generator speed
                with open("/dev/null", "w") as devnull:
                    start = time.perf_counter()
                    emit_lines(devnull, line, line_count)
                    end = time.perf_counter()
                    times_ms.append((end - start) * 1000)

            mean_ms = sum(times_ms) / len(times_ms)
            variance = sum((t - mean_ms) ** 2 for t in times_ms) / len(times_ms)
            stddev_ms = variance ** 0.5
            throughput_mbs = (total_bytes / 1e6) / (mean_ms / 1000) if mean_ms > 0 else 0

            print(f"{width:>8} {line_count:>8} {mean_ms:>12.3f} {stddev_ms:>10.3f} {throughput_mbs:>10.1f}", file=sys.stderr)

            results.append({
                "width": width,
                "lines": line_count,
                "mean_ms": round(mean_ms, 3),
                "stddev_ms": round(stddev_ms, 3),
                "throughput_mbs": round(throughput_mbs, 1),
                "total_bytes": total_bytes,
            })

    print("", file=sys.stderr)

    # Output JSON to stdout for programmatic use
    if args.json:
        output = {
            "benchmark_type": "generator_sweep",
            "timestamp": time.strftime("%Y%m%dT%H%M%S"),
            "pattern": args.pattern,
            "iterations": args.iterations,
            "results": results,
        }
        print(json.dumps(output, indent=2))

    return 0


def parse_range(spec: str) -> list[int]:
    """Parse a range specification like '100,500,1000' or '100:500:100'."""
    if ":" in spec:
        # Colon format: start:end:step
        parts = spec.split(":")
        if len(parts) == 2:
            start, end = int(parts[0]), int(parts[1])
            step = (end - start) // 4 or 1
        elif len(parts) == 3:
            start, end, step = int(parts[0]), int(parts[1]), int(parts[2])
        else:
            raise ValueError(f"Invalid range spec: {spec}")
        return list(range(start, end + 1, step))
    else:
        # Comma-separated list
        return [int(x.strip()) for x in spec.split(",")]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Emit repeated fixed-length lines for terminal benchmarking.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Single mode (default): emit 1000 lines of 500 chars each
  %(prog)s --lines 1000 --columns 500

  # Sweep mode: test multiple configurations
  %(prog)s --sweep --widths 100,500,1000,2000 --line-counts 100,500,1000

  # Sweep with range syntax (start:end:step)
  %(prog)s --sweep --widths 100:1000:100 --line-counts 500:2000:500

  # Output JSON results
  %(prog)s --sweep --json --widths 500,1000 --line-counts 1000
""",
    )

    # Single mode options
    parser.add_argument("--lines", type=int, default=1000, help="Number of lines to emit (single mode)")
    parser.add_argument("--columns", type=int, default=500, help="Characters per line (single mode)")
    parser.add_argument(
        "--pattern",
        default="X",
        help="Characters to repeat to build each line (default: 'X')",
    )

    # Sweep mode options
    parser.add_argument("--sweep", action="store_true", help="Enable sweep mode to test multiple configurations")
    parser.add_argument("--widths", default="100,500,1000,2000,5000",
                        help="Column widths to test (comma-separated or start:end:step)")
    parser.add_argument("--line-counts", dest="line_counts", default="100,500,1000,5000",
                        help="Line counts to test (comma-separated or start:end:step)")
    parser.add_argument("--iterations", type=int, default=5,
                        help="Iterations per configuration in sweep mode (default: 5)")
    parser.add_argument("--json", action="store_true", help="Output JSON results to stdout (sweep mode)")

    args = parser.parse_args()

    if args.sweep:
        return run_sweep(args)
    else:
        return run_single(args)


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
