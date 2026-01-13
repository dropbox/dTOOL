# DashFlow Script Style Guide

This document defines conventions for shell scripts in the `scripts/` directory.

## Required Patterns

### 1. Strict Mode

All scripts **must** start with a bash shebang and strict mode:

```bash
#!/bin/bash
set -euo pipefail
```

Or for more portable scripts:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

- `set -e`: Exit immediately on error
- `set -u`: Error on unset variables
- `set -o pipefail`: Fail on pipe errors

### 2. Script Header

Include a descriptive header:

```bash
#!/bin/bash
# scripts/my_script.sh - Brief description
#
# Usage:
#   ./scripts/my_script.sh [--flag]
#
# This script:
# 1. Does X
# 2. Then does Y

set -euo pipefail
```

### 3. Repo Root Discovery

Scripts should work from any directory by finding the repo root:

```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"
```

### 4. Python Usage

Use `python3` explicitly (never `python`):

```bash
# Correct
python3 scripts/my_tool.py

# Incorrect
python scripts/my_tool.py
```

## Recommended Patterns

### Argument Parsing

For scripts with options:

```bash
while [[ $# -gt 0 ]]; do
    case $1 in
        --help|-h)
            echo "Usage: $0 [options]"
            exit 0
            ;;
        --force|-f)
            FORCE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done
```

### Timeouts

Use `timeout` for operations that might hang:

```bash
timeout 120 cargo check
timeout 300 cargo test
```

### Error Messages

Write to stderr and exit with appropriate codes:

```bash
echo "ERROR: Description" >&2
exit 1
```

### Color Output

Support both color and plain output:

```bash
# Check if stdout is terminal
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    NC='\033[0m'
else
    RED=""
    GREEN=""
    NC=""
fi

echo -e "${GREEN}Success${NC}"
```

### Temp Files

Clean up temp files on exit:

```bash
TMPFILE=$(mktemp)
trap "rm -f $TMPFILE" EXIT
```

## Anti-Patterns

### Don't: Kill Process Groups

**Never** use negative PIDs to kill process groups - this can terminate your own shell:

```bash
# WRONG - can self-terminate
kill -TERM -$PID

# Correct - kill individual process
kill $PID
```

### Don't: Assume CWD

Don't assume scripts are run from repo root:

```bash
# WRONG
cat CLAUDE.md

# Correct
cat "$REPO_ROOT/CLAUDE.md"
```

### Don't: Use Unbounded Loops

Add timeouts to polling loops:

```bash
# WRONG - can run forever
while ! check_service; do
    sleep 5
done

# Correct - bounded with timeout
MAX_WAIT=60
WAITED=0
while ! check_service && [ $WAITED -lt $MAX_WAIT ]; do
    sleep 5
    WAITED=$((WAITED + 5))
done
```

## Lint Enforcement

Run the script linter to check compliance:

```bash
./scripts/lint_scripts.sh
```

This checks for:
- Missing `set -euo pipefail`
- Use of `python` instead of `python3`
- Missing shebang
- Hardcoded paths

## Examples

See these scripts as reference implementations:
- `scripts/doctor.sh` - Comprehensive diagnostics with JSON output
- `scripts/preflight.sh` - Pre-work validation
- `scripts/validate_tests.sh` - Test suite runner
