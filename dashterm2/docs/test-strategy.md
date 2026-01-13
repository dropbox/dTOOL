# DashTerm2 Test Strategy

## Test Gap Analysis

### Background: The 110+ Iteration Problem

From iterations #1 through #143, over 110 optimization commits were made to DashTerm2. Despite this extensive work, a critical launch crash went undetected until iteration #143. This document analyzes why this happened and defines a comprehensive test strategy to prevent similar gaps.

### Why Did We Miss a Launch Crash?

**Root Cause: No automated end-to-end launch verification**

The crash was in `iTermMetalRowDataPool`, introduced during performance optimization. It manifested as:
- Memory corruption on first frame render
- Immediate crash on app launch
- 100% reproducible on macOS 15.2

**Contributing Factors:**

1. **No CI Pipeline**: No automated builds or tests on commit/push
2. **No Smoke Test**: No script to verify basic app launch
3. **Unit Tests Don't Launch App**: XCTest tests run in isolation, not full app context
4. **No Pre-Commit Hooks**: Nothing blocked committing broken code
5. **Manual Testing Skipped**: Fast iteration pace encouraged skipping manual verification
6. **Optimization Focus**: Micro-benchmarks measured function performance, not app stability

### Existing Test Coverage

| Test Category | Coverage | Notes |
|---------------|----------|-------|
| Unit Tests (DashTerm2XCTests) | ~30 test files | Utility functions, data structures |
| Modern Tests | ~12 test files | VT100Grid, Screen, LineBuffer |
| WebExtensions Tests | ~30+ test files | Browser extension framework |
| Integration Tests | None | No full-app tests |
| UI Tests | None | No automated UI interaction |
| Smoke Tests | **NEW** | Launch stability only |

### What Tests Should Exist

#### Tier 1: Build & Launch (CRITICAL)

These must pass on every commit:

1. **Build Verification** - Does the code compile?
2. **Smoke Test** - Does the app launch and stay running for 10 seconds?
3. **Basic Window Test** - Can we create a terminal window?

#### Tier 2: Core Functionality (HIGH)

These should pass before merge to main:

4. **Terminal Operations**
   - Text input/output works
   - Scrolling works
   - Basic escape sequences work (colors, cursor movement)

5. **Session Management**
   - New tab creation works
   - New window creation works
   - Session restoration works

6. **Rendering Pipeline**
   - Metal renderer initializes
   - Text draws correctly
   - No visual corruption

#### Tier 3: Feature Tests (MEDIUM)

These should pass for releases:

7. **Preferences**
   - Preferences window opens
   - Settings persist

8. **Profile System**
   - Profile creation/deletion
   - Profile switching

9. **Integration Points**
   - Shell integration
   - tmux integration
   - SSH functionality

#### Tier 4: Performance Tests (LOW)

These validate optimization work:

10. **Benchmark Suite** (exists in `benchmarks/`)
    - Text rendering performance
    - Scrollback buffer performance
    - Memory usage

### Test Infrastructure Status

| Component | Status | Location |
|-----------|--------|----------|
| Smoke Test | **Complete** | `scripts/smoke-test.sh` |
| Pre-commit Hook | **Complete** | `scripts/install-hooks.sh` |
| Pre-push Hook | **Complete** | `scripts/install-hooks.sh` |
| CI Script | **Complete** | `scripts/ci.sh` |
| GitHub Actions | **Complete** | `.github/workflows/ci.yml` |
| UI Tests | **Skeleton Complete** | `DashTerm2UITests/` |
| Integration Tests | **TODO** | `DashTerm2IntegrationTests/` |

### Recommended CI Pipeline

```
                    +------------------+
                    |   Pull Request   |
                    +--------+---------+
                             |
                    +--------v---------+
                    |   Build Check    | <-- Must pass
                    +--------+---------+
                             |
                    +--------v---------+
                    |   Unit Tests     | <-- Must pass (DashTerm2Tests, ModernTests)
                    +--------+---------+
                             |
                    +--------v---------+
                    |   Smoke Test     | <-- Must pass
                    +--------+---------+
                             |
                    +--------v---------+
                    |   Integration    | <-- Optional (if tests exist)
                    +--------+---------+
                             |
                    +--------v---------+
                    |   Merge Ready    |
                    +------------------+
```

### Implementation Priorities

#### Phase 1: Prevention (DONE)
- [x] Smoke test script
- [x] Pre-commit hooks (lint)
- [x] Pre-push hooks (build + smoke)
- [x] CI script for local use

#### Phase 2: Automation (DONE - #150)
- [x] GitHub Actions workflow
- [x] Status checks on PRs
- [x] Test result reporting

#### Phase 3: Expansion (IN PROGRESS)
- [x] Basic UI test skeleton (DONE - #152)
- [ ] Terminal interaction tests
- [ ] Visual regression tests

### Test Writing Guidelines

When adding new features or fixing bugs:

1. **Add a regression test** if fixing a bug
2. **Add unit tests** for new utility functions
3. **Update smoke test** if changing startup behavior
4. **Run `scripts/ci.sh`** before pushing

### Monitoring Test Health

1. **Track Test Failures**: Log all CI failures with root cause
2. **Flaky Test Tracking**: Mark and investigate non-deterministic tests
3. **Coverage Metrics**: Consider adding code coverage reporting

---

## Document History

| Date | Author | Changes |
|------|--------|---------|
| 2025-12-17 | Worker #152 | Added UI test skeleton |
| 2025-12-17 | Worker #150 | Initial test gap analysis |
