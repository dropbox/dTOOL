# DashTerm2 Validation & CI Infrastructure Plan

## Problem Statement

110+ optimization iterations completed with:
- ❌ No verified builds
- ❌ No test runs
- ❌ No linting
- ❌ No performance benchmarks

**This is technical debt that must be addressed before further optimization work.**

---

## Priority 1: Fix Build (BLOCKING)

### Issue
SwiftyMarkdown framework built with SDK 'macosx26.0', incompatible with current SDK 'macosx15.2'.

### Actions
1. Rebuild SwiftyMarkdown from source with current SDK
2. OR update to latest Xcode/SDK that supports the framework
3. Verify clean build passes

---

## Priority 2: Git Commit Hooks (Linting)

### Languages to Lint
- **Objective-C**: clang-format, oclint
- **Swift**: SwiftLint, swift-format
- **Metal Shaders**: clang-format (C-like)

### Implementation
```bash
# .git/hooks/pre-commit
#!/bin/bash
set -e

# Swift linting
if command -v swiftlint &> /dev/null; then
    swiftlint lint --strict --quiet
fi

# Objective-C formatting check
if command -v clang-format &> /dev/null; then
    find sources -name "*.m" -o -name "*.h" | head -50 | xargs clang-format --dry-run --Werror
fi

echo "✓ Lint checks passed"
```

**Repository hook location:** `tools/git-hooks/pre-commit` mirrors the above logic while only touching staged files. Install it with `ln -sf ../../tools/git-hooks/pre-commit .git/hooks/pre-commit` to keep local hooks in sync with version control. The script quietly skips checks when SwiftLint or clang-format are missing, matching the installation guidance below.

### Install
```bash
brew install swiftlint clang-format
```

---

## Priority 3: Build & Test Automation

### Pre-commit Build Check
```bash
# Quick compile check (not full build)
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
    -configuration Debug \
    CODE_SIGN_IDENTITY="" CODE_SIGNING_REQUIRED=NO \
    build-for-testing 2>&1 | tail -5
```

### Test Schemes
- `DashTerm2Tests` - Core unit tests
- `ModernTests` - Modern Swift tests

### CI Script

Run `./scripts/ci.sh` to build DashTerm2 (Debug, codesign disabled) and execute the
`DashTerm2Tests` and `ModernTests` suites. Logs land in `reports/ci/` so failures can
be inspected after the fact. Environment variables allow customization:

- `CI_DESTINATION` (default `platform=macOS`) to target a specific simulator/device
- `CI_CONFIGURATION` (default `Debug`)
- `CI_LOG_DIR` if logs should go elsewhere

The script exits on the first xcodebuild error thanks to `set -euo pipefail` and
the use of `tee` with pipefail enabled.

---

## Priority 4: Performance Benchmarking

### Benchmark Suite

DashTerm2 now includes the following repeatable scripts under `benchmarks/`:

1. **Throughput (`throughput.sh`)** – wraps `hyperfine` and saves the JSON
   artifact under `benchmarks/results/throughput_latest.json`. Override the
   command/runs/warmups or refresh the baseline via `--update-baseline`.
2. **Memory (`memory.sh`)** – launches or attaches to DashTerm2, samples RSS and
   VSZ multiple times, and writes `benchmarks/results/memory_latest.json`.
3. **Frame Time (`frametime.sh`)** – captures a Metal System Trace via
   `xcrun xctrace` and stores the `.trace` bundle under
   `benchmarks/results/traces/` for manual inspection.

### Baseline Recording
- Run the throughput/memory scripts with `--update-baseline` to refresh
  `benchmarks/results/*_baseline.json` whenever hardware baselines change.
- Capture `*_latest.json` (and trace bundles) before/after optimization batches
  so regressions are obvious in git history.
- Keep `benchmarks/results/traces/` in source control for reproducibility.

### Automated Performance Regression
Use `./benchmarks/compare_against_baseline.sh --auto-run` to execute the latest
throughput benchmark and compare it to `throughput_baseline.json`. The helper
fails the shell when the mean runtime is more than 5% slower than the saved
baseline, printing both numbers for context.

## Implementation Order

1. **IMMEDIATE**: Fix SwiftyMarkdown build issue
2. **TODAY**: Install SwiftLint + clang-format
3. **TODAY**: Create pre-commit hook
4. **THIS WEEK**: Set up CI script
5. **THIS WEEK**: Create benchmark suite
6. **ONGOING**: Run benchmarks before/after optimization batches

---

## Worker Directive

**STOP** further optimization work until:
1. Build passes
2. Tests pass
3. Linting configured
4. Baseline benchmarks recorded

Then resume optimization with proper validation.
