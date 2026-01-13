#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""Count .clone() usage by crate."""

import subprocess
import sys
from pathlib import Path

def count_clones_in_crate(crate_path):
    """Count clone calls in a crate."""
    try:
        result = subprocess.run(
            ['rg', r'\.clone\(\)', '--type', 'rust', str(crate_path)],
            capture_output=True,
            text=True,
            check=False
        )
        if result.stdout.strip():
            lines = result.stdout.strip().split('\n')
            return len(lines)
        return 0
    except Exception as e:
        print(f"Error processing {crate_path}: {e}", file=sys.stderr)
        return 0

def main():
    # Use repo-relative path (script is in scripts/)
    script_dir = Path(__file__).parent.resolve()
    crates_dir = script_dir.parent / 'crates'
    results = []

    for crate_dir in sorted(crates_dir.iterdir()):
        if crate_dir.is_dir() and ('dashflow' in crate_dir.name.lower()):
            count = count_clones_in_crate(crate_dir)
            results.append((count, crate_dir.name))

    # Sort by count descending
    results.sort(reverse=True)

    print("Clone Usage by Crate:")
    print("=" * 60)
    for count, name in results[:30]:
        print(f"{count:>5} {name}")

    total = sum(c for c, _ in results)
    print("=" * 60)
    print(f"{total:>5} TOTAL")

if __name__ == '__main__':
    main()
