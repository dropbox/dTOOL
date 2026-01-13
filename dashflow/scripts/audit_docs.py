#!/usr/bin/env python3
"""
Identify undocumented public items in DashFlow crates.

Usage:
    python3 scripts/audit_docs.py                    # Audit all crates
    python3 scripts/audit_docs.py dashflow-openai    # Audit specific crate
    python3 scripts/audit_docs.py --summary          # Show summary only
    python3 scripts/audit_docs.py --json             # Output as JSON
"""

import subprocess
import sys
import re
import json
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Optional


@dataclass
class UndocumentedItem:
    """An undocumented public item."""
    crate: str
    item_type: str  # struct, fn, mod, etc.
    name: str
    file_path: str
    line: int

    def __str__(self) -> str:
        return f"  {self.item_type:8} {self.name} at {self.file_path}:{self.line}"


@dataclass
class CrateAudit:
    """Audit results for a single crate."""
    crate: str
    total_public: int
    documented: int
    undocumented_items: list[UndocumentedItem]

    @property
    def undocumented(self) -> int:
        return len(self.undocumented_items)

    @property
    def coverage_pct(self) -> float:
        if self.total_public == 0:
            return 100.0
        return (self.documented / self.total_public) * 100

    def to_dict(self) -> dict:
        return {
            "crate": self.crate,
            "total_public": self.total_public,
            "documented": self.documented,
            "undocumented": self.undocumented,
            "coverage_pct": round(self.coverage_pct, 1),
            "items": [asdict(item) for item in self.undocumented_items]
        }


def get_all_crates() -> list[str]:
    """Get all crates in the workspace."""
    crates_dir = Path("crates")
    if not crates_dir.exists():
        return []
    return sorted([d.name for d in crates_dir.iterdir() if d.is_dir() and (d / "Cargo.toml").exists()])


def parse_missing_doc_warning(line: str, crate: str) -> Optional[UndocumentedItem]:
    """Parse a missing doc warning from cargo doc output."""
    # Pattern: warning: missing documentation for a <type> `<name>`
    # --> crates/dashflow/src/foo.rs:123:1

    match = re.search(r"missing documentation for (?:a|an) (\w+) `([^`]+)`", line)
    if not match:
        return None

    item_type = match.group(1)
    name = match.group(2)

    return UndocumentedItem(
        crate=crate,
        item_type=item_type,
        name=name,
        file_path="",  # Will be filled from next line
        line=0
    )


def parse_location(line: str) -> tuple[str, int]:
    """Parse file location from --> line."""
    # --> crates/dashflow/src/foo.rs:123:1
    match = re.search(r"-->\s*([^:]+):(\d+):\d+", line)
    if match:
        return match.group(1), int(match.group(2))
    return "", 0


def audit_crate(crate: str, verbose: bool = False) -> CrateAudit:
    """
    Audit a single crate for undocumented public items.

    Uses `cargo doc` with -D missing_docs to find undocumented items.
    """
    items = []

    # Run cargo doc with strict missing_docs
    env = {"RUSTDOCFLAGS": "-D missing_docs"}
    result = subprocess.run(
        ["cargo", "doc", "-p", crate, "--no-deps", "--message-format=short"],
        capture_output=True,
        text=True,
        env={**subprocess.os.environ, **env},
        timeout=300  # 5 minute timeout
    )

    stderr = result.stderr
    lines = stderr.split("\n")

    current_item = None
    for i, line in enumerate(lines):
        # Check for missing doc warning
        item = parse_missing_doc_warning(line, crate)
        if item:
            current_item = item
            continue

        # Check for location (follows warning)
        if current_item and "-->" in line:
            file_path, line_num = parse_location(line)
            if file_path:
                current_item.file_path = file_path
                current_item.line = line_num
                items.append(current_item)
                current_item = None

    # Estimate total public items (rough heuristic based on source size)
    # A more accurate count would require analyzing the source
    total_estimate = len(items) + estimate_documented_items(crate)

    return CrateAudit(
        crate=crate,
        total_public=total_estimate,
        documented=total_estimate - len(items),
        undocumented_items=items
    )


def estimate_documented_items(crate: str) -> int:
    """
    Estimate number of documented public items.

    This is a rough heuristic - counts /// doc comments.
    For accurate counts, would need to parse AST.
    """
    crate_path = Path("crates") / crate / "src"
    if not crate_path.exists():
        return 0

    count = 0
    for rs_file in crate_path.rglob("*.rs"):
        try:
            content = rs_file.read_text()
            # Count doc comments followed by pub
            # This is imperfect but gives a ballpark
            count += len(re.findall(r"///[^\n]*\n(?:\s*///[^\n]*\n)*\s*pub\s", content))
        except Exception:
            pass

    return count


def audit_all_crates(verbose: bool = False) -> list[CrateAudit]:
    """Audit all crates in the workspace."""
    crates = get_all_crates()
    results = []

    for i, crate in enumerate(crates):
        if verbose:
            print(f"[{i+1}/{len(crates)}] Auditing {crate}...", file=sys.stderr)

        try:
            audit = audit_crate(crate, verbose)
            results.append(audit)
        except subprocess.TimeoutExpired:
            print(f"  TIMEOUT: {crate}", file=sys.stderr)
        except Exception as e:
            print(f"  ERROR: {crate}: {e}", file=sys.stderr)

    return results


def print_summary(audits: list[CrateAudit]) -> None:
    """Print summary of all audits."""
    total_public = sum(a.total_public for a in audits)
    total_documented = sum(a.documented for a in audits)
    total_undocumented = sum(a.undocumented for a in audits)

    print("\n=== Documentation Audit Summary ===\n")
    print(f"Crates analyzed: {len(audits)}")
    print(f"Total public items: {total_public}")
    print(f"Documented: {total_documented}")
    print(f"Undocumented: {total_undocumented}")
    if total_public > 0:
        print(f"Coverage: {(total_documented / total_public) * 100:.1f}%")

    # Print crates with most undocumented items
    print("\n=== Crates with Most Undocumented Items ===\n")
    sorted_audits = sorted(audits, key=lambda a: a.undocumented, reverse=True)
    for audit in sorted_audits[:20]:
        if audit.undocumented > 0:
            print(f"  {audit.crate:40} {audit.undocumented:4} undocumented ({audit.coverage_pct:.0f}% coverage)")


def print_crate_details(audit: CrateAudit) -> None:
    """Print detailed results for a single crate."""
    print(f"\n=== {audit.crate} ===\n")
    print(f"Total public items: {audit.total_public}")
    print(f"Documented: {audit.documented}")
    print(f"Undocumented: {audit.undocumented}")
    print(f"Coverage: {audit.coverage_pct:.1f}%")

    if audit.undocumented_items:
        print("\nUndocumented items:")
        for item in audit.undocumented_items:
            print(item)


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Audit documentation coverage")
    parser.add_argument("crate", nargs="?", help="Specific crate to audit")
    parser.add_argument("--summary", action="store_true", help="Show summary only")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    args = parser.parse_args()

    if args.crate:
        # Audit specific crate
        audit = audit_crate(args.crate, args.verbose)
        if args.json:
            print(json.dumps(audit.to_dict(), indent=2))
        else:
            print_crate_details(audit)
    else:
        # Audit all crates
        audits = audit_all_crates(args.verbose)

        if args.json:
            print(json.dumps([a.to_dict() for a in audits], indent=2))
        elif args.summary:
            print_summary(audits)
        else:
            print_summary(audits)
            print("\n=== Per-Crate Details ===")
            for audit in sorted(audits, key=lambda a: a.undocumented, reverse=True):
                if audit.undocumented > 0:
                    print_crate_details(audit)


if __name__ == "__main__":
    main()
