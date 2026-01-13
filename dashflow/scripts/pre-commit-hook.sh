#!/bin/bash
# scripts/pre-commit-hook.sh - DashFlow Pre-commit Hook
#
# This is the tracked version of the pre-commit hook.
# To install: ln -sf ../../scripts/pre-commit-hook.sh .git/hooks/pre-commit
#
# Checks:
#   - M-67: Block commits of target directories (build artifacts)
#   - M-87: Block commits of large files and artifact patterns
#   - M-03: Verify Last Updated dates on key documentation
#   - M-35: Lint Grafana dashboard JSON for semantic issues
#   - M-27: Require justification strings for ignored tests
#   - Type index freshness (warns if stale)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# If called from .git/hooks, adjust REPO_ROOT
if [[ "$0" == *".git/hooks"* ]]; then
    REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fi

# ==============================================================================
# M-67: Block commits of target directories
# ==============================================================================
# These patterns catch accidental staging of build artifacts.
# The artifact purge (M-61..M-65) cleaned history, but this prevents recurrence.

BLOCKED_PATTERNS=(
    "^target/"
    "^target_.*/"
    "/target/"
    "^fuzz/target/"
)

# Get list of staged files
STAGED_FILES=$(git diff --cached --name-only 2>/dev/null || echo "")

if [ -n "$STAGED_FILES" ]; then
    BLOCKED_FILES=""
    for pattern in "${BLOCKED_PATTERNS[@]}"; do
        # Use grep to find matches (|| true to avoid error if no match)
        MATCHES=$(echo "$STAGED_FILES" | grep -E "$pattern" 2>/dev/null || true)
        if [ -n "$MATCHES" ]; then
            BLOCKED_FILES="$BLOCKED_FILES$MATCHES"$'\n'
        fi
    done

    if [ -n "$BLOCKED_FILES" ]; then
        echo "ERROR: Build artifacts staged for commit (M-67 protection)"
        echo ""
        echo "The following files/directories are blocked:"
        echo "$BLOCKED_FILES" | grep -v '^$' | head -20
        TOTAL=$(echo "$BLOCKED_FILES" | grep -v '^$' | wc -l | tr -d ' ')
        if [ "$TOTAL" -gt 20 ]; then
            echo "... and $((TOTAL - 20)) more"
        fi
        echo ""
        echo "To unstage these files, run:"
        echo "  git reset HEAD -- target/ target_*/ fuzz/target/"
        echo ""
        echo "To force commit (NOT recommended), use:"
        echo "  git commit --no-verify"
        exit 1
    fi
fi

# ==============================================================================
# M-87: Block commits of large files and artifact patterns
# ==============================================================================
# Prevents accidentally committing large files that shouldn't be in git.
# Size threshold: 500KB (512000 bytes) for most files.
# Exceptions: package-lock.json, Grafana dashboards, proto schemas

LARGE_FILE_THRESHOLD=512000  # 500KB in bytes

# Patterns that are blocked regardless of size (should never be committed)
BLOCKED_ARTIFACT_PATTERNS=(
    '\.log$'
    '\.profraw$'
    '\.profdata$'
    'flamegraph\.svg$'
    'perf\.data'
    '\.pkl$'
    '\.bin$'
    'worker_logs/'
    'test-artifacts/'
    '/outputs/'
    '\.mutants$'
)

# Files/patterns exempt from size check
SIZE_EXEMPT_PATTERNS=(
    'package-lock\.json$'
    'grafana/dashboards/.*\.json$'
    'examples/.*/grafana/dashboards/.*\.json$'
    '\.schema\.json$'
    'Cargo\.lock$'
)

is_size_exempt() {
    local file="$1"
    for pattern in "${SIZE_EXEMPT_PATTERNS[@]}"; do
        if echo "$file" | grep -qE "$pattern"; then
            return 0
        fi
    done
    return 1
}

if [ -n "$STAGED_FILES" ]; then
    # Check for blocked artifact patterns
    BLOCKED_ARTIFACTS=""
    for pattern in "${BLOCKED_ARTIFACT_PATTERNS[@]}"; do
        MATCHES=$(echo "$STAGED_FILES" | grep -E "$pattern" 2>/dev/null || true)
        if [ -n "$MATCHES" ]; then
            BLOCKED_ARTIFACTS="$BLOCKED_ARTIFACTS$MATCHES"$'\n'
        fi
    done

    if [ -n "$BLOCKED_ARTIFACTS" ]; then
        echo "ERROR: Artifact files staged for commit (M-87 protection)"
        echo ""
        echo "The following files should not be committed:"
        echo "$BLOCKED_ARTIFACTS" | grep -v '^$' | head -10
        echo ""
        echo "These patterns are always blocked: .log, .profraw, .pkl, .bin, etc."
        echo "Add to .gitignore or remove from staging."
        echo ""
        echo "To force commit (NOT recommended), use:"
        echo "  git commit --no-verify"
        exit 1
    fi

    # Check for large files
    LARGE_FILES=""
    while IFS= read -r file; do
        [ -z "$file" ] && continue
        # Skip deleted files
        if [ ! -f "$REPO_ROOT/$file" ]; then
            continue
        fi
        # Skip exempt files
        if is_size_exempt "$file"; then
            continue
        fi
        # Check file size
        FILE_SIZE=$(wc -c < "$REPO_ROOT/$file" 2>/dev/null | tr -d ' ' || echo "0")
        if [ "$FILE_SIZE" -gt "$LARGE_FILE_THRESHOLD" ]; then
            SIZE_KB=$((FILE_SIZE / 1024))
            LARGE_FILES="$LARGE_FILES  $file (${SIZE_KB}KB)"$'\n'
        fi
    done <<< "$STAGED_FILES"

    if [ -n "$LARGE_FILES" ]; then
        echo "ERROR: Large files staged for commit (M-87 protection)"
        echo ""
        echo "Files exceeding ${LARGE_FILE_THRESHOLD} bytes ($((LARGE_FILE_THRESHOLD / 1024))KB):"
        echo "$LARGE_FILES" | grep -v '^$' | head -10
        TOTAL=$(echo "$LARGE_FILES" | grep -v '^$' | wc -l | tr -d ' ')
        if [ "$TOTAL" -gt 10 ]; then
            echo "... and $((TOTAL - 10)) more"
        fi
        echo ""
        echo "Large files bloat git history. Consider:"
        echo "  - Adding to .gitignore for generated files"
        echo "  - Using Git LFS for legitimate large assets"
        echo "  - Compressing or splitting the file"
        echo ""
        echo "Exempt patterns: package-lock.json, Grafana dashboards, Cargo.lock"
        echo ""
        echo "To force commit (NOT recommended), use:"
        echo "  git commit --no-verify"
        exit 1
    fi
fi

# ==============================================================================
# M-03: Verify Last Updated dates on tracked documentation
# ==============================================================================
# Ensures CLAUDE.md, ROADMAP_CURRENT.md, and WORKER_DIRECTIVE.md have current
# "Last Updated" dates when modified. Prevents roadmap drift.

TRACKED_DOCS=(
    "CLAUDE.md"
    "ROADMAP_CURRENT.md"
    "WORKER_DIRECTIVE.md"
)

TODAY=$(date +%Y-%m-%d)

check_last_updated() {
    local file="$1"
    # Extract date from "**Last Updated:** YYYY-MM-DD" pattern
    local last_updated=$(grep -m1 "^\*\*Last Updated:\*\*" "$REPO_ROOT/$file" 2>/dev/null | \
        grep -oE "[0-9]{4}-[0-9]{2}-[0-9]{2}" | head -1)

    if [ -z "$last_updated" ]; then
        echo "WARNING: $file has no Last Updated date"
        return 1
    fi

    if [ "$last_updated" != "$TODAY" ]; then
        echo "WARNING: $file Last Updated date ($last_updated) is not today ($TODAY)"
        echo "  → Update the '**Last Updated:**' line before committing"
        return 1
    fi
    return 0
}

STALE_DOCS=""
for doc in "${TRACKED_DOCS[@]}"; do
    # Check if this file is in staged changes
    if echo "$STAGED_FILES" | grep -qE "^${doc}$"; then
        if ! check_last_updated "$doc"; then
            STALE_DOCS="$STALE_DOCS $doc"
        fi
    fi
done

if [ -n "$STALE_DOCS" ]; then
    echo ""
    echo "M-03 WARNING: Documentation has stale Last Updated dates."
    echo "Stale files:$STALE_DOCS"
    echo ""
    echo "Expected format: **Last Updated:** $TODAY (Author - Brief description)"
    echo ""
    echo "To bypass this warning (NOT recommended), use:"
    echo "  git commit --no-verify"
    echo ""
    # Warning only - don't block the commit since this might be intentional
fi

# ==============================================================================
# M-35: Dashboard semantic linting (prevents dashboard-as-code drift)
# ==============================================================================
# Lint Grafana dashboard JSON files for semantic correctness issues.
# This catches bad patterns like x/x divisions, unused variables, etc.

DASHBOARD_FILES=$(echo "$STAGED_FILES" | grep -E '\.json$' | grep -E 'grafana.*dashboard|dashboards/' || true)

if [ -n "$DASHBOARD_FILES" ]; then
    if [ -f "$REPO_ROOT/scripts/lint_grafana_dashboard.py" ]; then
        LINT_ERRORS=""
        for dashboard in $DASHBOARD_FILES; do
            if [ -f "$REPO_ROOT/$dashboard" ]; then
                LINT_OUTPUT=$(python3 "$REPO_ROOT/scripts/lint_grafana_dashboard.py" --severity error "$REPO_ROOT/$dashboard" 2>&1 || true)
                if [ -n "$LINT_OUTPUT" ]; then
                    LINT_ERRORS="$LINT_ERRORS$LINT_OUTPUT"$'\n'
                fi
            fi
        done

        if [ -n "$LINT_ERRORS" ]; then
            echo ""
            echo "M-35 WARNING: Grafana dashboard semantic issues found:"
            echo "$LINT_ERRORS"
            echo ""
            echo "Run 'python3 scripts/lint_grafana_dashboard.py <file>' for details."
            echo ""
            # Warning only - don't block the commit
        fi
    fi
fi

# ==============================================================================
# M-27: Require justification strings for ignored tests
# ==============================================================================
# Ensures all #[ignore] attributes have reason strings like:
#   #[ignore = "requires external service"]
# This prevents silent test skips and enables test burn-down tracking.

if [ -x "$REPO_ROOT/scripts/check_ignore_reasons.sh" ]; then
    STAGED_RS=$(echo "$STAGED_FILES" | grep '\.rs$' || true)
    if [ -n "$STAGED_RS" ]; then
        if ! "$REPO_ROOT/scripts/check_ignore_reasons.sh" --staged; then
            echo ""
            echo "M-27 ERROR: Bare #[ignore] attributes found."
            echo "All ignored tests must have justification strings."
            echo ""
            echo "To bypass this check (NOT recommended), use:"
            echo "  git commit --no-verify"
            exit 1
        fi
    fi
fi

# ==============================================================================
# Type index auto-rebuild (Gap 10: ensures index stays current)
# ==============================================================================
# Set DASHFLOW_NO_INDEX_REBUILD=1 to disable auto-rebuild
if [ -x "$REPO_ROOT/scripts/pre-commit-type-index.sh" ]; then
    if [ "${DASHFLOW_NO_INDEX_REBUILD:-}" = "1" ]; then
        "$REPO_ROOT/scripts/pre-commit-type-index.sh" || true
    else
        "$REPO_ROOT/scripts/pre-commit-type-index.sh" --rebuild || true
    fi
fi

# All checks passed
exit 0
