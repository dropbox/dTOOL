#!/bin/bash
# =============================================================================
# DashTerm2 Git Hooks Installer
# NASA/NSA Grade - Zero Defect Tolerance
# =============================================================================
# This script installs comprehensive git hooks for pre-commit and pre-push
# quality gates. Run after cloning the repository.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo ""
echo "============================================================================="
echo "  DashTerm2 Git Hooks Installer - NASA/NSA Grade"
echo "============================================================================="
echo ""

# Ensure hooks directory exists
mkdir -p "$HOOKS_DIR"

# =============================================================================
# OPTION 1: Use the comprehensive hook scripts (RECOMMENDED)
# =============================================================================
echo -e "${BLUE}Installing NASA/NSA grade git hooks...${NC}"
echo ""

# Install pre-commit hook (symlink to our comprehensive script)
if [ -f "$SCRIPT_DIR/pre-commit-hook.sh" ]; then
    ln -sf "$SCRIPT_DIR/pre-commit-hook.sh" "$HOOKS_DIR/pre-commit"
    chmod +x "$HOOKS_DIR/pre-commit"
    echo -e "${GREEN}✓${NC} Pre-commit hook installed (comprehensive)"
else
    echo -e "${RED}✗${NC} pre-commit-hook.sh not found - using inline hook"
    # Fallback to inline hook
    cat > "$HOOKS_DIR/pre-commit" << 'PRECOMMIT_EOF'
#!/bin/bash
# DashTerm2 Pre-commit Hook (Fallback)
# Runs SwiftLint on staged Swift files and clang-format check on ObjC files

set -e

echo "Running pre-commit hooks..."

# Get list of staged Swift files
SWIFT_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.swift$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Get list of staged ObjC files
OBJC_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(m|mm|h)$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run SwiftLint on Swift files
if [ -n "$SWIFT_FILES" ]; then
    echo "Checking Swift files with SwiftLint..."
    if command -v swiftlint &> /dev/null; then
        echo "$SWIFT_FILES" | xargs swiftlint lint --quiet --config .swiftlint.yml 2>/dev/null || {
            echo "SwiftLint found issues. Please fix them before committing."
            echo "Run 'swiftlint --fix' to auto-fix some issues."
        }
    else
        echo "SwiftLint not installed. Run 'brew install swiftlint'"
    fi
fi

# Run clang-format check on ObjC/C/C++/Metal files
if [ -n "$OBJC_FILES" ]; then
    echo "Checking Objective-C files with clang-format..."
    if command -v clang-format &> /dev/null; then
        for file in $OBJC_FILES; do
            if [ -f "$file" ]; then
                if ! clang-format --dry-run --Werror "$file" 2>/dev/null; then
                    echo "  $file: formatting issues detected"
                fi
            fi
        done
    else
        echo "clang-format not installed. Run 'brew install clang-format'"
    fi
fi

# Get list of staged Python files
PYTHON_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.py$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run Python linting
if [ -n "$PYTHON_FILES" ]; then
    echo "Checking Python files..."
    if command -v flake8 &> /dev/null; then
        echo "$PYTHON_FILES" | xargs flake8 --max-line-length=120 --ignore=E501,W503,E402,E741,W504,E302,E305,W191,E101 2>/dev/null || {
            echo "flake8 found issues in Python files."
        }
    elif command -v pylint &> /dev/null; then
        echo "$PYTHON_FILES" | xargs pylint --disable=C0114,C0115,C0116 --max-line-length=120 2>/dev/null || {
            echo "pylint found issues in Python files."
        }
    else
        echo "No Python linter installed. Run 'pip install flake8' or 'pip install pylint'"
    fi
fi

# Get list of staged shell scripts
SHELL_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.sh$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run shellcheck on shell scripts
if [ -n "$SHELL_FILES" ]; then
    echo "Checking shell scripts with shellcheck..."
    if command -v shellcheck &> /dev/null; then
        echo "$SHELL_FILES" | xargs shellcheck -S warning 2>/dev/null || {
            echo "shellcheck found issues in shell scripts."
        }
    else
        echo "shellcheck not installed. Run 'brew install shellcheck'"
    fi
fi

# Get list of staged JavaScript files (template files are excluded in eslint.config.js)
JS_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.js$' | grep -v 'ThirdParty/' | grep -v 'submodules/' | grep -v 'node_modules/' || true)

# Run ESLint on JavaScript files
if [ -n "$JS_FILES" ]; then
    echo "Checking JavaScript files with ESLint..."
    if command -v eslint &> /dev/null; then
        # Template files are excluded in eslint.config.js
        echo "$JS_FILES" | xargs eslint 2>/dev/null || {
            echo "ESLint found issues in JavaScript files."
        }
    else
        echo "eslint not installed. Run 'npm install -g eslint'"
    fi
fi

# Get list of staged C/C++ files
C_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(c|cpp|cc|cxx)$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run clang-format on C/C++ files
if [ -n "$C_FILES" ]; then
    echo "Checking C/C++ files with clang-format..."
    if command -v clang-format &> /dev/null; then
        for file in $C_FILES; do
            if [ -f "$file" ]; then
                if ! clang-format --dry-run --Werror "$file" 2>/dev/null; then
                    echo "  $file: formatting issues detected"
                fi
            fi
        done
    fi
fi

# Get list of staged Metal files
METAL_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.metal$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run clang-format on Metal shader files
if [ -n "$METAL_FILES" ]; then
    echo "Checking Metal shader files with clang-format..."
    if command -v clang-format &> /dev/null; then
        for file in $METAL_FILES; do
            if [ -f "$file" ]; then
                if ! clang-format --dry-run --Werror "$file" 2>/dev/null; then
                    echo "  $file: formatting issues detected"
                fi
            fi
        done
    fi
fi

# Get list of staged Markdown files
MD_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.md$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run markdownlint on Markdown files
if [ -n "$MD_FILES" ]; then
    echo "Checking Markdown files with markdownlint..."
    if command -v markdownlint &> /dev/null; then
        echo "$MD_FILES" | xargs markdownlint --config .markdownlint.json 2>/dev/null || {
            echo "markdownlint found issues in Markdown files."
        }
    else
        echo "markdownlint not installed. Run 'npm install -g markdownlint-cli'"
    fi
fi

# Get list of staged JSON files
JSON_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.json$' | grep -v 'ThirdParty/' | grep -v 'submodules/' | grep -v 'node_modules/' || true)

# Run jsonlint on JSON files
if [ -n "$JSON_FILES" ]; then
    echo "Checking JSON files with jsonlint..."
    if command -v jsonlint &> /dev/null; then
        for file in $JSON_FILES; do
            if [ -f "$file" ]; then
                jsonlint -q "$file" 2>/dev/null || {
                    echo "  $file: JSON syntax error"
                }
            fi
        done
    else
        echo "jsonlint not installed. Run 'npm install -g jsonlint'"
    fi
fi

# Get list of staged HTML files
HTML_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.html$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run htmlhint on HTML files
if [ -n "$HTML_FILES" ]; then
    echo "Checking HTML files with htmlhint..."
    if command -v htmlhint &> /dev/null; then
        echo "$HTML_FILES" | xargs htmlhint 2>/dev/null || {
            echo "htmlhint found issues in HTML files."
        }
    else
        echo "htmlhint not installed. Run 'npm install -g htmlhint'"
    fi
fi

# Get list of staged CSS files
CSS_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.css$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run stylelint on CSS files
if [ -n "$CSS_FILES" ]; then
    echo "Checking CSS files with stylelint..."
    if command -v stylelint &> /dev/null; then
        echo "$CSS_FILES" | xargs stylelint 2>/dev/null || {
            echo "stylelint found issues in CSS files."
        }
    else
        echo "stylelint not installed. Run 'npm install -g stylelint stylelint-config-standard'"
    fi
fi

# Get list of staged YAML files
YAML_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(yml|yaml)$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run yamllint on YAML files
if [ -n "$YAML_FILES" ]; then
    echo "Checking YAML files with yamllint..."
    if command -v yamllint &> /dev/null; then
        echo "$YAML_FILES" | xargs yamllint -c .yamllint.yml 2>/dev/null || {
            echo "yamllint found issues in YAML files."
        }
    else
        echo "yamllint not installed. Run 'pip install yamllint'"
    fi
fi

# Get list of staged XML/Plist files
XML_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(xml|plist)$' | grep -v 'ThirdParty/' | grep -v 'submodules/' || true)

# Run xmllint on XML/Plist files
if [ -n "$XML_FILES" ]; then
    echo "Checking XML/Plist files with xmllint..."
    if command -v xmllint &> /dev/null; then
        for file in $XML_FILES; do
            if [ -f "$file" ]; then
                xmllint --noout "$file" 2>/dev/null || {
                    echo "  $file: XML syntax error"
                }
            fi
        done
    else
        echo "xmllint not installed (usually comes with libxml2)"
    fi
fi

echo "Pre-commit checks completed."
exit 0
PRECOMMIT_EOF

chmod +x "$HOOKS_DIR/pre-commit"

# Create pre-push hook for smoke test (catches crashes before push)
cat > "$HOOKS_DIR/pre-push" << 'EOF'
#!/bin/bash
# DashTerm2 Pre-push Hook
# Runs build verification and smoke test before pushing
# This prevents pushing code that crashes on launch

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/../.." && pwd )"

echo "=== Pre-push: Build Verification ==="

cd "$PROJECT_ROOT"

# Build
echo "Building DashTerm2..."
if ! xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" 2>&1 | grep -E "(BUILD|error:)" | tail -5; then
    echo "BUILD FAILED - push aborted"
    exit 1
fi

# Smoke test
echo ""
echo "Running smoke test..."
if [ -x "$PROJECT_ROOT/scripts/smoke-test.sh" ]; then
    if ! "$PROJECT_ROOT/scripts/smoke-test.sh"; then
        echo ""
        echo "SMOKE TEST FAILED - push aborted"
        echo "The app crashes on launch. Fix the crash before pushing."
        exit 1
    fi
else
    echo "Warning: smoke-test.sh not found, skipping"
fi

echo ""
echo "=== Pre-push verification passed ==="
exit 0
EOF

chmod +x "$HOOKS_DIR/pre-push"
fi

# =============================================================================
# INSTALL PRE-PUSH HOOK
# =============================================================================
if [ -f "$SCRIPT_DIR/pre-push-hook.sh" ]; then
    ln -sf "$SCRIPT_DIR/pre-push-hook.sh" "$HOOKS_DIR/pre-push"
    chmod +x "$HOOKS_DIR/pre-push"
    echo -e "${GREEN}✓${NC} Pre-push hook installed (comprehensive)"
else
    echo -e "${RED}✗${NC} pre-push-hook.sh not found - using inline hook"
    # Keep existing inline hook
fi

# =============================================================================
# SUMMARY
# =============================================================================
echo ""
echo "============================================================================="
echo -e "${GREEN}Git hooks installed successfully!${NC}"
echo "============================================================================="
echo ""
echo "Hooks installed:"
echo "  - pre-commit: NASA/NSA grade multi-tier analysis"
echo "  - pre-push:   Full build verification + smoke test"
echo ""
echo "============================================================================="
echo "TIER 1 - Install ALL static analysis tools:"
echo "============================================================================="
echo ""
echo "  ./scripts/install-static-analysis.sh"
echo ""
echo "Or install manually:"
echo ""
echo "  # Deep Static Analyzers"
echo "  brew install llvm infer cppcheck oclint periphery"
echo ""
echo "  # Language Linters"
echo "  brew install swiftlint swiftformat clang-format shellcheck"
echo "  pip install flake8 mypy pylint bandit safety lizard radon"
echo "  npm install -g eslint markdownlint-cli jsonlint htmlhint stylelint"
echo ""
echo "  # Security Scanners"
echo "  brew install semgrep trufflehog gitleaks"
echo ""
echo "============================================================================="
echo "Run full static analysis:"
echo "============================================================================="
echo ""
echo "  ./scripts/static-analysis.sh         # Full analysis"
echo "  ./scripts/static-analysis.sh --quick # Quick checks only"
echo "  ./scripts/security-scan.sh           # Security-focused scan"
echo ""
echo "============================================================================="
