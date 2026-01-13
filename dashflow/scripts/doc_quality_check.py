#!/usr/bin/env python3
"""
Score documentation quality for DashFlow crates.

Analyzes doc comments for:
- Summary presence and quality
- Example presence
- Error documentation
- See Also references
- Panic documentation

Usage:
    python3 scripts/doc_quality_check.py dashflow-openai
    python3 scripts/doc_quality_check.py --all
    python3 scripts/doc_quality_check.py --json dashflow
"""

import re
import sys
import json
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional


@dataclass
class DocQualityScore:
    """Quality scores for a single documented item."""
    name: str
    file_path: str
    line: int
    has_summary: bool = False
    has_description: bool = False  # More than just summary
    has_example: bool = False
    has_errors: bool = False
    has_panics: bool = False
    has_see_also: bool = False
    has_safety: bool = False  # For unsafe code
    summary_length: int = 0
    total_lines: int = 0

    @property
    def quality_score(self) -> float:
        """Calculate quality score 0-10."""
        score = 0.0

        # Summary (required, 2 points)
        if self.has_summary:
            score += 2.0
            # Bonus for good length (20-80 chars)
            if 20 <= self.summary_length <= 80:
                score += 0.5

        # Description beyond summary (1 point)
        if self.has_description:
            score += 1.0

        # Example (3 points - most important)
        if self.has_example:
            score += 3.0

        # Error docs (1.5 points)
        if self.has_errors:
            score += 1.5

        # See Also (1 point)
        if self.has_see_also:
            score += 1.0

        # Panics (0.5 points)
        if self.has_panics:
            score += 0.5

        # Safety (0.5 points for unsafe items)
        if self.has_safety:
            score += 0.5

        return min(score, 10.0)


@dataclass
class CrateQualityReport:
    """Quality report for an entire crate."""
    crate: str
    total_items: int = 0
    items_with_summary: int = 0
    items_with_description: int = 0
    items_with_example: int = 0
    items_with_errors: int = 0
    items_with_see_also: int = 0
    items_with_panics: int = 0
    scores: list[DocQualityScore] = field(default_factory=list)

    @property
    def summary_pct(self) -> float:
        return (self.items_with_summary / self.total_items * 100) if self.total_items else 0

    @property
    def example_pct(self) -> float:
        return (self.items_with_example / self.total_items * 100) if self.total_items else 0

    @property
    def errors_pct(self) -> float:
        return (self.items_with_errors / self.total_items * 100) if self.total_items else 0

    @property
    def see_also_pct(self) -> float:
        return (self.items_with_see_also / self.total_items * 100) if self.total_items else 0

    @property
    def average_score(self) -> float:
        if not self.scores:
            return 0.0
        return sum(s.quality_score for s in self.scores) / len(self.scores)

    def to_dict(self) -> dict:
        return {
            "crate": self.crate,
            "total_items": self.total_items,
            "average_score": round(self.average_score, 2),
            "summary_pct": round(self.summary_pct, 1),
            "example_pct": round(self.example_pct, 1),
            "errors_pct": round(self.errors_pct, 1),
            "see_also_pct": round(self.see_also_pct, 1),
            "lowest_scoring": [
                {"name": s.name, "score": round(s.quality_score, 1), "file": s.file_path, "line": s.line}
                for s in sorted(self.scores, key=lambda x: x.quality_score)[:10]
            ]
        }


def extract_doc_blocks(content: str, file_path: str) -> list[tuple[str, str, int]]:
    """
    Extract doc comment blocks and their associated items.

    Returns list of (doc_comment, item_signature, line_number).
    """
    blocks = []

    # Find all doc comment blocks followed by pub items
    # Pattern: /// comments followed by pub ...
    pattern = re.compile(
        r"((?:\s*///[^\n]*\n)+)"  # Doc comments
        r"\s*(pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static|mod|use)\s+(\w+))",
        re.MULTILINE
    )

    for match in pattern.finditer(content):
        doc = match.group(1)
        signature = match.group(2)
        name = match.group(3)
        line_num = content[:match.start()].count("\n") + 1
        blocks.append((doc, name, line_num))

    return blocks


def analyze_doc_block(doc: str, name: str, file_path: str, line: int) -> DocQualityScore:
    """Analyze a single doc block for quality metrics."""
    score = DocQualityScore(name=name, file_path=file_path, line=line)

    # Clean doc comments
    lines = [line.strip().lstrip("///").strip() for line in doc.strip().split("\n")]
    lines = [l for l in lines if l]  # Remove empty lines

    if not lines:
        return score

    score.total_lines = len(lines)

    # Check for summary (first non-empty line)
    if lines:
        summary = lines[0]
        score.has_summary = len(summary) > 5  # More than just "..."
        score.summary_length = len(summary)

    # Check for description (more than 2 lines)
    score.has_description = len(lines) > 2

    # Join for pattern matching
    full_doc = "\n".join(lines).lower()

    # Check for examples
    score.has_example = any([
        "# example" in full_doc,
        "# examples" in full_doc,
        "```rust" in full_doc,
        "```" in full_doc and ("use " in full_doc or "let " in full_doc or "fn " in full_doc)
    ])

    # Check for error documentation
    score.has_errors = any([
        "# error" in full_doc,
        "# errors" in full_doc,
        "returns `err" in full_doc,
        "returns an error" in full_doc,
        "will return an error" in full_doc,
        "result::err" in full_doc
    ])

    # Check for see also
    score.has_see_also = any([
        "# see also" in full_doc,
        "see also" in full_doc,
        "see [`" in full_doc,
        "related:" in full_doc
    ])

    # Check for panics
    score.has_panics = any([
        "# panic" in full_doc,
        "# panics" in full_doc,
        "will panic" in full_doc,
        "panics if" in full_doc
    ])

    # Check for safety (unsafe code)
    score.has_safety = any([
        "# safety" in full_doc,
        "unsafe" in full_doc
    ])

    return score


def analyze_crate(crate: str) -> CrateQualityReport:
    """Analyze documentation quality for a crate."""
    crate_path = Path("crates") / crate / "src"
    report = CrateQualityReport(crate=crate)

    if not crate_path.exists():
        return report

    for rs_file in crate_path.rglob("*.rs"):
        try:
            content = rs_file.read_text()
            rel_path = str(rs_file.relative_to(Path(".")))

            blocks = extract_doc_blocks(content, rel_path)
            for doc, name, line in blocks:
                score = analyze_doc_block(doc, name, rel_path, line)
                report.scores.append(score)
                report.total_items += 1

                if score.has_summary:
                    report.items_with_summary += 1
                if score.has_description:
                    report.items_with_description += 1
                if score.has_example:
                    report.items_with_example += 1
                if score.has_errors:
                    report.items_with_errors += 1
                if score.has_see_also:
                    report.items_with_see_also += 1
                if score.has_panics:
                    report.items_with_panics += 1

        except Exception as e:
            print(f"Error reading {rs_file}: {e}", file=sys.stderr)

    return report


def get_all_crates() -> list[str]:
    """Get all crates in the workspace."""
    crates_dir = Path("crates")
    if not crates_dir.exists():
        return []
    return sorted([d.name for d in crates_dir.iterdir() if d.is_dir() and (d / "Cargo.toml").exists()])


def print_report(report: CrateQualityReport) -> None:
    """Print a quality report."""
    print(f"\n=== Documentation Quality: {report.crate} ===\n")
    print(f"Total documented items: {report.total_items}")
    print(f"Average quality score: {report.average_score:.1f}/10")
    print()
    print("Coverage by category:")
    print(f"  Has summary:    {report.summary_pct:5.1f}%")
    print(f"  Has examples:   {report.example_pct:5.1f}%")
    print(f"  Has errors:     {report.errors_pct:5.1f}%")
    print(f"  Has see also:   {report.see_also_pct:5.1f}%")

    if report.scores:
        print("\nLowest scoring items:")
        for score in sorted(report.scores, key=lambda x: x.quality_score)[:10]:
            print(f"  {score.quality_score:.1f}/10  {score.name} at {score.file_path}:{score.line}")

        print("\nHighest scoring items:")
        for score in sorted(report.scores, key=lambda x: x.quality_score, reverse=True)[:5]:
            print(f"  {score.quality_score:.1f}/10  {score.name} at {score.file_path}:{score.line}")


def print_summary(reports: list[CrateQualityReport]) -> None:
    """Print summary of all crates."""
    print("\n=== Documentation Quality Summary ===\n")

    total_items = sum(r.total_items for r in reports)
    if total_items == 0:
        print("No documented items found.")
        return

    # Calculate overall stats
    all_scores = [s for r in reports for s in r.scores]
    avg_score = sum(s.quality_score for s in all_scores) / len(all_scores) if all_scores else 0

    print(f"Crates analyzed: {len(reports)}")
    print(f"Total documented items: {total_items}")
    print(f"Overall average score: {avg_score:.1f}/10")
    print()

    # Print crates by score
    print("Crates by average score:")
    sorted_reports = sorted(reports, key=lambda r: r.average_score, reverse=True)
    for report in sorted_reports:
        if report.total_items > 0:
            bar = "#" * int(report.average_score)
            print(f"  {report.crate:40} {report.average_score:4.1f}/10 {bar}")


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Check documentation quality")
    parser.add_argument("crate", nargs="?", help="Specific crate to analyze")
    parser.add_argument("--all", action="store_true", help="Analyze all crates")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    args = parser.parse_args()

    if args.all or not args.crate:
        crates = get_all_crates()
        reports = []
        for i, crate in enumerate(crates):
            if args.verbose:
                print(f"[{i+1}/{len(crates)}] Analyzing {crate}...", file=sys.stderr)
            report = analyze_crate(crate)
            if report.total_items > 0:
                reports.append(report)

        if args.json:
            print(json.dumps([r.to_dict() for r in reports], indent=2))
        else:
            print_summary(reports)
    else:
        report = analyze_crate(args.crate)
        if args.json:
            print(json.dumps(report.to_dict(), indent=2))
        else:
            print_report(report)


if __name__ == "__main__":
    main()
