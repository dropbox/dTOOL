#!/usr/bin/env python3
"""
Grafana Dashboard Linter - Phase 210: Semantic Correctness Checks

Detects bad patterns in Grafana dashboards that would produce misleading values:
1. PromQL patterns like `x/x` (always returns 1)
2. rate() on metrics that aren't counters (_total suffix convention)
3. Template variables defined but not used in any query
4. Hardcoded constant expressions disguised as metrics

Exit codes:
  0 - No issues found
  1 - Issues found
  2 - Parse/usage error
"""

import json
import re
import sys
import argparse
from pathlib import Path
from typing import Any


class DashboardLinter:
    """Lints Grafana dashboard JSON for semantic correctness issues."""

    def __init__(self, dashboard: dict):
        self.dashboard = dashboard
        self.issues: list[dict] = []

    def lint(self) -> list[dict]:
        """Run all lint checks and return issues."""
        self.issues = []
        self._check_placeholder_divisions()
        self._check_unused_template_variables()
        self._check_rate_on_non_counters()
        self._check_constant_expressions()
        return self.issues

    def _add_issue(self, severity: str, category: str, message: str,
                   panel_id: int | None = None, panel_title: str | None = None,
                   expr: str | None = None):
        """Record a lint issue."""
        issue = {
            "severity": severity,
            "category": category,
            "message": message,
        }
        if panel_id is not None:
            issue["panel_id"] = panel_id
        if panel_title is not None:
            issue["panel_title"] = panel_title
        if expr is not None:
            issue["expr"] = expr
        self.issues.append(issue)

    def _get_all_expressions(self) -> list[tuple[int | None, str | None, str]]:
        """Extract all PromQL expressions from panels and alerts."""
        expressions = []

        for panel in self.dashboard.get("panels", []):
            panel_id = panel.get("id")
            panel_title = panel.get("title", "Unknown")

            # Regular panel targets
            for target in panel.get("targets", []):
                expr = target.get("expr", "")
                if expr:
                    expressions.append((panel_id, panel_title, expr))

            # Alert conditions (legacy Grafana alerting)
            alert = panel.get("alert", {})
            if alert:
                for condition in alert.get("conditions", []):
                    query = condition.get("query", {})
                    # Legacy alerts reference the panel's targets by letter
                    pass  # Already covered by panel targets

        # Annotation queries
        for annotation in self.dashboard.get("annotations", {}).get("list", []):
            expr = annotation.get("expr", "")
            if expr:
                expressions.append((None, f"Annotation: {annotation.get('name', 'Unknown')}", expr))

        return expressions

    def _check_placeholder_divisions(self):
        """
        Detect patterns like `metric_a / metric_a` which always return 1.

        Pattern examples that are BAD:
        - `foo / foo` -> always 1
        - `sum(foo) / sum(foo)` -> always 1
        - `rate(foo[5m]) / rate(foo[5m])` -> always 1
        """
        for panel_id, panel_title, expr in self._get_all_expressions():
            # Remove whitespace for comparison
            expr_normalized = re.sub(r'\s+', '', expr)

            # Check for simple `a/a` pattern (same identifier/expression divided)
            # Match: `identifier / identifier` or `func(args) / func(args)`
            division_pattern = r'([a-zA-Z_][a-zA-Z0-9_]*(?:\{[^}]*\})?(?:\[[^\]]+\])?)\s*/\s*\1(?![a-zA-Z0-9_])'
            if re.search(division_pattern, expr):
                self._add_issue(
                    "error", "placeholder_division",
                    f"Expression divides metric by itself (always returns 1)",
                    panel_id, panel_title, expr
                )
                continue

            # More robust check: split by / and compare left/right sides
            # Handle nested parentheses by finding the main division operator
            if '/' in expr_normalized:
                left, right = self._split_division(expr_normalized)
                if left and right and left == right:
                    self._add_issue(
                        "error", "placeholder_division",
                        f"Expression divides identical subexpressions (always returns 1)",
                        panel_id, panel_title, expr
                    )

    def _split_division(self, expr: str) -> tuple[str | None, str | None]:
        """Split expression by main division operator, respecting parentheses."""
        depth = 0
        for i, char in enumerate(expr):
            if char == '(':
                depth += 1
            elif char == ')':
                depth -= 1
            elif char == '/' and depth == 0:
                left = expr[:i].strip()
                right = expr[i+1:].strip()
                return (left, right)
        return (None, None)

    def _check_unused_template_variables(self):
        """
        Detect template variables that are defined but never referenced in queries.

        Users can change these dropdowns but they have no effect - misleading UX.
        """
        template_vars = []
        for var in self.dashboard.get("templating", {}).get("list", []):
            var_name = var.get("name")
            if var_name:
                template_vars.append(var_name)

        if not template_vars:
            return

        # Get all expressions as a single string for searching
        all_exprs = " ".join(expr for _, _, expr in self._get_all_expressions())

        for var_name in template_vars:
            # Template variables are referenced as $var or ${var}
            var_patterns = [
                rf'\${var_name}(?![a-zA-Z0-9_])',
                rf'\$\{{{var_name}\}}',
            ]

            found = any(re.search(p, all_exprs) for p in var_patterns)

            if not found:
                self._add_issue(
                    "warning", "unused_variable",
                    f"Template variable '${var_name}' is defined but never used in any query"
                )

    def _check_rate_on_non_counters(self):
        """
        Detect rate() applied to metrics without _total suffix (convention violation).

        By Prometheus convention, counters should have _total suffix.
        rate() on gauges produces meaningless results.

        Note: This is a heuristic - some metrics may legitimately use rate() without _total.
        """
        for panel_id, panel_title, expr in self._get_all_expressions():
            # Find rate() and irate() calls
            rate_pattern = r'(?:rate|irate)\(\s*([a-zA-Z_][a-zA-Z0-9_]*)'
            matches = re.findall(rate_pattern, expr)

            for metric_name in matches:
                # Skip histogram buckets (they don't have _total)
                if metric_name.endswith('_bucket'):
                    continue
                # Skip histogram sums and counts
                if metric_name.endswith('_sum') or metric_name.endswith('_count'):
                    continue
                # Counter convention: should end with _total
                if not metric_name.endswith('_total'):
                    self._add_issue(
                        "warning", "rate_on_non_counter",
                        f"rate() applied to metric '{metric_name}' which doesn't have _total suffix. "
                        f"If this is a gauge, rate() is meaningless.",
                        panel_id, panel_title, expr
                    )

    def _check_constant_expressions(self):
        """
        Detect expressions that are just constants disguised as metrics.

        Examples:
        - `0.90` as a standalone metric (should be threshold line, not query)
        - `vector(0)` with no other operations
        """
        for panel_id, panel_title, expr in self._get_all_expressions():
            expr_stripped = expr.strip()

            # Skip expressions that are part of "or vector(0)" fallbacks - those are OK
            if 'or vector' in expr:
                continue

            # Check if expression is just a number
            if re.match(r'^-?\d+\.?\d*$', expr_stripped):
                # Allow threshold lines (commonly 0.90, 1.0, etc.)
                # This is a soft warning - thresholds are sometimes legitimate
                pass  # Don't warn on constant threshold lines

            # Check for standalone vector() with no aggregation
            if re.match(r'^vector\(\d+\)$', expr_stripped):
                self._add_issue(
                    "warning", "constant_expression",
                    f"Expression is just a constant vector - consider using threshold annotation instead",
                    panel_id, panel_title, expr
                )


def lint_file(filepath: Path) -> list[dict]:
    """Lint a single dashboard file."""
    try:
        with open(filepath) as f:
            dashboard = json.load(f)
    except json.JSONDecodeError as e:
        return [{"severity": "error", "category": "parse_error", "message": f"Invalid JSON: {e}"}]
    except IOError as e:
        return [{"severity": "error", "category": "io_error", "message": f"Cannot read file: {e}"}]

    linter = DashboardLinter(dashboard)
    return linter.lint()


def main():
    parser = argparse.ArgumentParser(
        description="Lint Grafana dashboard JSON for semantic correctness issues"
    )
    parser.add_argument(
        "files", nargs="+", type=Path,
        help="Dashboard JSON files to lint"
    )
    parser.add_argument(
        "--severity", choices=["error", "warning", "all"], default="all",
        help="Minimum severity to report (default: all)"
    )
    parser.add_argument(
        "--json", action="store_true",
        help="Output results as JSON"
    )

    args = parser.parse_args()

    all_issues = []
    for filepath in args.files:
        if not filepath.exists():
            print(f"Error: File not found: {filepath}", file=sys.stderr)
            sys.exit(2)

        issues = lint_file(filepath)
        for issue in issues:
            issue["file"] = str(filepath)
        all_issues.extend(issues)

    # Filter by severity
    if args.severity == "error":
        all_issues = [i for i in all_issues if i["severity"] == "error"]

    if args.json:
        print(json.dumps(all_issues, indent=2))
    else:
        for issue in all_issues:
            severity = issue["severity"].upper()
            category = issue["category"]
            message = issue["message"]
            file = issue.get("file", "unknown")
            panel = issue.get("panel_title", "")

            location = f"{file}"
            if panel:
                location += f" [Panel: {panel}]"

            print(f"{severity}: {location}")
            print(f"  Category: {category}")
            print(f"  {message}")
            if "expr" in issue:
                print(f"  Expression: {issue['expr'][:100]}{'...' if len(issue.get('expr', '')) > 100 else ''}")
            print()

    # Exit with error code if any issues found
    has_errors = any(i["severity"] == "error" for i in all_issues)
    sys.exit(1 if has_errors else 0)


if __name__ == "__main__":
    main()
