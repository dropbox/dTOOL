#!/bin/bash
# Setup script for git hooks
# Installs enhanced pre-commit hooks for quality enforcement
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GIT_HOOKS_DIR="$REPO_ROOT/.git/hooks"

# ANSI color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo ""
echo -e "${BLUE}${BOLD}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}${BOLD}║           Git Hooks Setup - DashFlow Rust                   ║${NC}"
echo -e "${BLUE}${BOLD}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check if git repo
if [ ! -d "$REPO_ROOT/.git" ]; then
    echo "❌ Not a git repository: $REPO_ROOT"
    exit 1
fi

echo "📁 Repository: $REPO_ROOT"
echo ""

# Backup existing hooks
if [ -f "$GIT_HOOKS_DIR/pre-commit" ]; then
    BACKUP_FILE="$GIT_HOOKS_DIR/pre-commit.backup.$(date +%Y%m%d_%H%M%S)"
    echo "💾 Backing up existing pre-commit hook to:"
    echo "   $BACKUP_FILE"
    cp "$GIT_HOOKS_DIR/pre-commit" "$BACKUP_FILE"
    echo ""
fi

# Install enhanced pre-commit hook
echo "📝 Installing enhanced pre-commit hook..."
cat > "$GIT_HOOKS_DIR/pre-commit" << 'HOOK_EOF'
#!/bin/bash
# Enhanced pre-commit hook for DashFlow Rust
# Enforces code quality standards before commit

set -e

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration - Set these via environment or modify here
SKIP_TESTS="${SKIP_TESTS:-false}"          # Set to "true" to skip tests
SKIP_CLIPPY="${SKIP_CLIPPY:-false}"        # Set to "true" to skip clippy
SKIP_FMT="${SKIP_FMT:-false}"              # Set to "true" to skip formatting check
QUICK_MODE="${QUICK_MODE:-false}"          # Set to "true" for quick checks only

# Critical files that MUST NOT be deleted
CRITICAL_FILES=(
    "scripts/python/json_to_text.py"
)

echo ""
echo -e "${BLUE}${BOLD}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}${BOLD}║                 Pre-commit Quality Checks                    ║${NC}"
echo -e "${BLUE}${BOLD}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ============================================================================
# Check 0: M-67 - Block target directory commits (build artifacts)
# ============================================================================
echo -e "${BOLD}[0/6]${NC} Checking for build artifacts (M-67 protection)..."

BLOCKED_PATTERNS=(
    "^target/"
    "^target_.*/"
    "/target/"
    "^fuzz/target/"
)

STAGED_FILES=$(git diff --cached --name-only 2>/dev/null || echo "")
BLOCKED_FILES=""

if [ -n "$STAGED_FILES" ]; then
    for pattern in "${BLOCKED_PATTERNS[@]}"; do
        MATCHES=$(echo "$STAGED_FILES" | grep -E "$pattern" 2>/dev/null || true)
        if [ -n "$MATCHES" ]; then
            BLOCKED_FILES="$BLOCKED_FILES$MATCHES"$'\n'
        fi
    done
fi

if [ -n "$BLOCKED_FILES" ]; then
    echo -e "${RED}✗ BLOCKED: Build artifacts staged for commit${NC}"
    echo ""
    echo "The following files/directories are blocked:"
    echo "$BLOCKED_FILES" | grep -v '^$' | head -20
    TOTAL=$(echo "$BLOCKED_FILES" | grep -v '^$' | wc -l | tr -d ' ')
    if [ "$TOTAL" -gt 20 ]; then
        echo "... and $((TOTAL - 20)) more"
    fi
    echo ""
    echo "To unstage: git reset HEAD -- target/ target_*/ fuzz/target/"
    echo "To force (NOT recommended): git commit --no-verify"
    exit 1
fi
echo -e "${GREEN}✓ No build artifacts staged${NC}"
echo ""

# ============================================================================
# Check 1: Critical File Protection
# ============================================================================
echo -e "${BOLD}[1/6]${NC} Checking for critical file deletions..."

deleted_critical_files=()
for file in "${CRITICAL_FILES[@]}"; do
    if git diff --cached --name-status | grep -q "^D.*$file"; then
        deleted_critical_files+=("$file")
    fi
done

if [ ${#deleted_critical_files[@]} -gt 0 ]; then
    echo -e "${RED}✗ BLOCKED: Critical files cannot be deleted${NC}"
    for file in "${deleted_critical_files[@]}"; do
        echo -e "  ${RED}✗${NC} $file"
    done
    echo ""
    echo "To unstage: git restore --staged ${deleted_critical_files[0]}"
    exit 1
fi
echo -e "${GREEN}✓ No critical files deleted${NC}"
echo ""

# ============================================================================
# Check 2: Rust Formatting
# ============================================================================
if [ "$SKIP_FMT" = "false" ]; then
    echo -e "${BOLD}[2/6]${NC} Checking Rust formatting..."
    if ! cargo fmt --all -- --check > /dev/null 2>&1; then
        echo -e "${RED}✗ Formatting check failed${NC}"
        echo ""
        echo "Run: cargo fmt --all"
        echo ""
        exit 1
    fi
    echo -e "${GREEN}✓ Formatting OK${NC}"
else
    echo -e "${YELLOW}[2/6] Skipped: Rust formatting (SKIP_FMT=true)${NC}"
fi
echo ""

# ============================================================================
# Check 3: Clippy Linting
# ============================================================================
if [ "$SKIP_CLIPPY" = "false" ] && [ "$QUICK_MODE" = "false" ]; then
    echo -e "${BOLD}[3/6]${NC} Running clippy lints (prod targets, strict)..."
    CLIPPY_OUTPUT=$(cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used 2>&1) || {
        echo "$CLIPPY_OUTPUT" | grep -v "^$" || true
        echo -e "${RED}✗ Clippy found issues${NC}"
        echo ""
        echo "Note: unwrap()/expect() are forbidden in production targets."
        echo "If intentional, add #[allow(clippy::unwrap_used|expect_used)] with a SAFETY justification comment."
        echo ""
        echo "Fix issues or run: SKIP_CLIPPY=true git commit"
        echo ""
        exit 1
    }
    echo "$CLIPPY_OUTPUT" | grep -v "^$" || true
    echo -e "${GREEN}✓ Clippy passed${NC}"
elif [ "$QUICK_MODE" = "true" ]; then
    echo -e "${YELLOW}[3/6] Skipped: Clippy (QUICK_MODE=true)${NC}"
else
    echo -e "${YELLOW}[3/6] Skipped: Clippy (SKIP_CLIPPY=true)${NC}"
fi
echo ""

# ============================================================================
# Check 4: Compilation Check
# ============================================================================
if [ "$QUICK_MODE" = "false" ]; then
    echo -e "${BOLD}[4/6]${NC} Checking compilation..."
    if ! cargo check --workspace --all-targets > /dev/null 2>&1; then
        echo -e "${RED}✗ Compilation failed${NC}"
        echo ""
        echo "Fix compilation errors before committing"
        echo ""
        exit 1
    fi
    echo -e "${GREEN}✓ Compilation OK${NC}"
else
    echo -e "${YELLOW}[4/6] Skipped: Compilation (QUICK_MODE=true)${NC}"
fi
echo ""

# ============================================================================
# Check 5: Tests (Optional)
# ============================================================================
if [ "$SKIP_TESTS" = "false" ] && [ "$QUICK_MODE" = "false" ]; then
    echo -e "${BOLD}[5/6]${NC} Running tests..."

    # Get changed Rust files
    CHANGED_FILES=$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$' || true)

    if [ -n "$CHANGED_FILES" ]; then
        # Run quick tests only (exclude integration tests)
        if ! cargo test --workspace --lib --bins 2>&1 | tail -5; then
            echo -e "${RED}✗ Tests failed${NC}"
            echo ""
            echo "Fix failing tests or run: SKIP_TESTS=true git commit"
            echo ""
            exit 1
        fi
        echo -e "${GREEN}✓ Tests passed${NC}"
    else
        echo -e "${YELLOW}✓ No Rust files changed, skipping tests${NC}"
    fi
elif [ "$QUICK_MODE" = "true" ]; then
    echo -e "${YELLOW}[5/6] Skipped: Tests (QUICK_MODE=true)${NC}"
else
    echo -e "${YELLOW}[5/6] Skipped: Tests (SKIP_TESTS=true)${NC}"
fi
echo ""

# ============================================================================
# Success
# ============================================================================
echo -e "${GREEN}${BOLD}✓ All pre-commit checks passed!${NC}"
echo ""

# Tips for faster commits
if [ "$QUICK_MODE" = "false" ]; then
    echo -e "${BLUE}💡 Tip: For faster commits, use:${NC}"
    echo "   QUICK_MODE=true git commit     (skip clippy, check, tests)"
    echo "   SKIP_TESTS=true git commit     (skip tests only)"
    echo ""
fi

exit 0
HOOK_EOF

chmod +x "$GIT_HOOKS_DIR/pre-commit"
echo -e "${GREEN}✓ Pre-commit hook installed${NC}"
echo ""

# Install pre-push hook
echo "📝 Installing pre-push hook..."
cat > "$GIT_HOOKS_DIR/pre-push" << 'HOOK_EOF'
#!/bin/bash
# Pre-push hook for DashFlow Rust
# Runs comprehensive checks before pushing

set -e

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

SKIP_PREPUSH="${SKIP_PREPUSH:-false}"

if [ "$SKIP_PREPUSH" = "true" ]; then
    echo ""
    echo -e "${YELLOW}⚠️  Pre-push checks skipped (SKIP_PREPUSH=true)${NC}"
    echo ""
    exit 0
fi

echo ""
echo -e "${BLUE}${BOLD}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}${BOLD}║                   Pre-push Quality Checks                    ║${NC}"
echo -e "${BLUE}${BOLD}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check 1: Run full test suite
echo -e "${BOLD}[1/2]${NC} Running full test suite..."
if ! cargo test --workspace; then
    echo -e "${RED}✗ Tests failed${NC}"
    echo ""
    echo "Fix failing tests or run: SKIP_PREPUSH=true git push"
    echo ""
    exit 1
fi
echo -e "${GREEN}✓ All tests passed${NC}"
echo ""

# Check 2: Check for uncommitted changes
echo -e "${BOLD}[2/2]${NC} Checking for uncommitted changes..."
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo -e "${YELLOW}⚠️  Warning: You have uncommitted changes${NC}"
    echo ""
    echo "Consider committing all changes before pushing"
    echo ""
    read -p "Continue push anyway? (y/N): " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Push cancelled"
        exit 1
    fi
fi
echo -e "${GREEN}✓ No uncommitted changes${NC}"
echo ""

echo -e "${GREEN}${BOLD}✓ All pre-push checks passed!${NC}"
echo ""

exit 0
HOOK_EOF

chmod +x "$GIT_HOOKS_DIR/pre-push"
echo -e "${GREEN}✓ Pre-push hook installed${NC}"
echo ""

# Summary
echo -e "${GREEN}${BOLD}════════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}${BOLD}✓ Git hooks installation complete!${NC}"
echo -e "${GREEN}${BOLD}════════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Installed hooks:"
echo "  ✓ pre-commit  - M-67 artifact protection, format, lint, compile, test"
echo "  ✓ pre-push    - Full test suite before push"
echo ""
echo "Configuration options:"
echo "  QUICK_MODE=true git commit       - Skip clippy, check, tests"
echo "  SKIP_TESTS=true git commit       - Skip tests only"
echo "  SKIP_CLIPPY=true git commit      - Skip clippy only"
echo "  SKIP_FMT=true git commit         - Skip formatting check"
echo "  SKIP_PREPUSH=true git push       - Skip pre-push checks"
echo ""
echo "Hook locations:"
echo "  pre-commit: $GIT_HOOKS_DIR/pre-commit"
echo "  pre-push:   $GIT_HOOKS_DIR/pre-push"
echo ""
echo "To uninstall:"
echo "  rm $GIT_HOOKS_DIR/pre-commit"
echo "  rm $GIT_HOOKS_DIR/pre-push"
echo ""
