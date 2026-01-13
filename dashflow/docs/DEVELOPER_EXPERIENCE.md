# Developer Experience Guide - DashFlow Evals

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

This guide covers developer tools for working with the evaluation framework.

---

## Git Hooks for Quality Assurance

### Overview

Git hooks automatically run quality checks before commits and pushes, preventing quality regressions from entering the codebase.

### Installation

```bash
# Install standard hooks (formatting, linting, tests)
./scripts/setup-git-hooks.sh

# Install hooks WITH evaluation support
./scripts/setup-eval-hooks.sh
```

### Features

**Pre-commit Hook:**
1. **Critical file protection** - Prevents deletion of essential files
2. **Rust formatting** - Ensures consistent code style
3. **Clippy linting** - Catches common mistakes and anti-patterns
4. **Compilation check** - Verifies code compiles
5. **Unit tests** - Runs fast unit tests
6. **Evaluations (OPTIONAL)** - Runs full evaluation suite to prevent quality regressions

**Pre-push Hook:**
1. **Full test suite** - Runs all tests including integration tests
2. **Evaluations (OPTIONAL)** - Comprehensive quality checks before pushing
3. **Uncommitted changes warning** - Reminds you to commit everything

---

## Using Evaluation Hooks

### Default Behavior (Evals Disabled)

By default, evaluations are **disabled** for fast commits:

```bash
git commit -m "your message"
# Runs: format, lint, compile, tests (fast, ~10-30 seconds)
# Does NOT run evaluations (would add 2-3 minutes)
```

### Enable Evaluations on Commit

When working on critical changes that affect agent quality:

```bash
# Run evaluations before committing
SKIP_EVALS=false EVAL_APP=librarian git commit -m "Improved retrieval logic"

# What happens:
# 1. Standard checks (format, lint, compile, tests)
# 2. Runs full evaluation suite (50 scenarios)
# 3. Blocks commit if quality regressions detected
```

**When to use:**
- Modifying agent logic, prompts, or tools
- Changes to RAG pipeline or retrieval
- Updates to response generation
- Any change that could affect output quality

**When NOT to use:**
- Documentation changes
- Test-only changes
- Refactoring with full test coverage
- Minor code style fixes

### Enable Evaluations on Push

Run comprehensive quality checks before pushing to remote:

```bash
# Run evaluations before pushing
RUN_EVALS=true EVAL_APP=librarian git push

# What happens:
# 1. Full test suite
# 2. Full evaluation suite (50 scenarios)
# 3. Blocks push if any check fails
```

**When to use:**
- Before creating a pull request
- Before merging to main branch
- After major feature completion
- When you want extra confidence

---

## Configuration Options

### Quick Mode (Skip Expensive Checks)

For rapid iteration during development:

```bash
# Skip clippy, compilation check, tests, and evals
QUICK_MODE=true git commit -m "WIP: experimenting"

# Only runs:
# - Critical file protection
# - Formatting check
```

### Skip Specific Checks

Fine-grained control over which checks run:

```bash
# Skip tests only (useful for doc changes)
SKIP_TESTS=true git commit -m "Update README"

# Skip clippy only (if you have warnings you'll fix later)
SKIP_CLIPPY=true git commit -m "WIP: refactoring in progress"

# Skip formatting check (not recommended!)
SKIP_FMT=true git commit -m "Emergency hotfix"

# Skip pre-push checks entirely
SKIP_PREPUSH=true git push
```

### Combining Options

```bash
# Fast commit with tests but no evals
SKIP_CLIPPY=true git commit -m "Quick fix"

# Thorough commit with evals
SKIP_EVALS=false EVAL_APP=librarian git commit -m "Improved RAG quality"
```

---

## Evaluation Details

### What Gets Evaluated

When evaluations run, the system:

1. **Loads golden scenarios** (test cases from `examples/apps/librarian/data/`)
2. **Runs your app** on each scenario
3. **Scores outputs** using LLM-as-judge (6 quality dimensions)
4. **Compares to baseline** to detect regressions
5. **Blocks commit/push** if quality drops below threshold

### Performance

- **50 scenarios:** ~2-3 minutes
- **Cost:** ~$0.04-0.06 (OpenAI API calls for judging)
- **Parallelization:** Runs multiple scenarios concurrently

### API Key Required

Evaluations require an OpenAI API key:

```bash
# Set in environment
export OPENAI_API_KEY="sk-proj-..."

# Or in .env file (already configured in this repo)
# The hook will automatically use it
```

### What Gets Blocked

A commit/push is blocked if:

- **Pass rate drops:** <96% scenarios passing (was ≥96%)
- **Quality drops:** Average quality <0.90 (threshold)
- **New failures:** Previously passing scenarios now fail
- **Critical errors:** Agent crashes, timeouts, or API errors

### Interpreting Results

**Success:**
```
✓ Evaluations passed (no quality regressions)
  Pass Rate: 48/50 (96%)
  Avg Quality: 0.924
  Avg Latency: 2.3s
```

**Failure:**
```
✗ Evaluations failed
  Pass Rate: 45/50 (90%) ← Below 96% threshold
  Failed scenarios:
    - 12_medium_when_to_use_query: Quality 0.82 (threshold: 0.90)
    - 18_complex_streaming_server_query: Quality 0.78 (threshold: 0.90)
    - 33_complex_metrics_query: Timeout after 30s

Quality regression detected. Fix issues or skip: SKIP_EVALS=true git commit
```

---

## Troubleshooting

### Hook Not Running

**Problem:** Hook doesn't execute

**Solution:**
```bash
# Verify hook is installed
ls -la .git/hooks/pre-commit

# Re-install if missing
./scripts/setup-eval-hooks.sh

# Verify executable permission
chmod +x .git/hooks/pre-commit
```

### Evaluations Failing

**Problem:** Evals fail but you think output is fine

**Solution:**
```bash
# Run eval manually to see detailed output
cargo run -p librarian -- eval

# Check specific failing scenario
cargo run -p librarian -- eval --scenario 12

# Review scenario expectations in librarian data directory
ls examples/apps/librarian/data/
```

### API Key Not Found

**Problem:** `OPENAI_API_KEY not set`

**Solution:**
```bash
# Check .env file exists
cat .env | grep OPENAI_API_KEY

# Export it
export OPENAI_API_KEY=$(grep OPENAI_API_KEY .env | cut -d '=' -f2 | tr -d '"')

# Or load all env vars
source .env

# Verify it's set
echo $OPENAI_API_KEY
```

### Hook Takes Too Long

**Problem:** Pre-commit hook is too slow

**Solutions:**
```bash
# Option 1: Use QUICK_MODE for fast commits
QUICK_MODE=true git commit -m "Quick fix"

# Option 2: Disable evals (default)
git commit -m "Normal commit"  # Evals are disabled by default

# Option 3: Skip tests during rapid iteration
SKIP_TESTS=true git commit -m "WIP"
```

### Want to Commit Despite Failures

**Problem:** Need to commit but checks are failing

**Solution:**
```bash
# Skip all checks (NOT RECOMMENDED for production)
git commit --no-verify -m "Emergency commit"

# Skip only evals
SKIP_EVALS=true git commit -m "Commit without eval checks"

# Skip only tests
SKIP_TESTS=true git commit -m "Commit without tests"
```

---

## Best Practices

### Development Workflow

**During Active Development:**
```bash
# Fast commits without evals
git commit -m "WIP: implementing feature X"
git commit -m "WIP: debugging issue Y"
```

**Before Creating PR:**
```bash
# Run full checks with evaluations
SKIP_EVALS=false EVAL_APP=librarian git commit -m "Implement feature X"

# Or run evals on push
git commit -m "Implement feature X"
RUN_EVALS=true EVAL_APP=librarian git push
```

**Before Merging to Main:**
```bash
# Always run evals before merging
SKIP_EVALS=false EVAL_APP=librarian git commit -m "Final: feature X complete"
RUN_EVALS=true EVAL_APP=librarian git push
```

### CI/CD Integration

Run evaluations locally before pushing:

```bash
# Run evaluations with OPENAI_API_KEY from .env
source .env
cargo run -p librarian -- eval
```

> **Note:** This repo does not have GitHub Actions CI. Run evaluations locally.

The eval binary exits with:
- **Exit code 0:** All checks passed
- **Exit code 1:** Quality regression detected

---

## Additional Developer Tools (Planned)

The following tools are planned but not yet implemented. Contributions welcome!

### Watch Mode

Automatically re-run evaluations when files change. Use cargo-watch for now:

```bash
# Manual workaround using cargo-watch
cargo install cargo-watch
cargo watch -x "run -p librarian"
```

### Interactive REPL

Debug failed scenarios interactively. Not yet implemented.

### VS Code Extension

Run evaluations from VS Code. Not yet implemented.

---

## Summary

**Default behavior:** Fast commits without evaluations (~10-30 seconds)

**Enable evals when needed:**
- `SKIP_EVALS=false EVAL_APP=librarian git commit` for pre-commit checks
- `RUN_EVALS=true EVAL_APP=librarian git push` for pre-push checks

**Skip checks when appropriate:**
- `QUICK_MODE=true git commit` for rapid iteration
- `SKIP_TESTS=true git commit` for doc-only changes
- `git commit --no-verify` for emergencies (not recommended)

**Best practice:** Always run evaluations before creating PRs or merging to main.
