# Fake Test Audit Results

**Date:** 2025-12-23
**Audited by:** MANAGER AI

## Executive Summary

**1566 fake tests** in `BugRegressionTests.swift` use `loadSourceFile()` to grep source code instead of testing production code. These tests prove nothing and must be rewritten.

## Trap Mechanism

The `loadSourceFile()` function now contains `XCTFail()` which causes ALL fake tests to fail immediately with this message:

```
⛔️⛔️⛔️ FAKE TEST DETECTED ⛔️⛔️⛔️

This test uses loadSourceFile() to check if strings exist in source code.
THAT PROVES NOTHING ABOUT WHETHER THE BUG IS ACTUALLY FIXED!
```

## WORKER DIRECTIVE: Converting Fake Tests to Real Tests

### Priority Order

Focus on P0 root cause bugs first, then work through by bug number.

### Conversion Pattern

**FAKE (Before):**
```swift
func test_BUG_1450_toolSnippetsUsesItAssertNotAssert() {
    guard let content = loadSourceFile(relativePath: "sources/iTermToolSnippets.m") else {
        XCTFail("Could not load file")
        return
    }
    XCTAssertTrue(content.contains("it_assert"))  // PROVES NOTHING!
}
```

**REAL (After):**
```swift
func test_BUG_1450_toolSnippetsHandlesNilInput() {
    // Import the actual module
    // @testable import DashTerm2SharedARC

    // Create REAL production object
    let snippets = iTermToolSnippets()

    // Exercise the code path that had the bug
    let result = snippets.processInput(nil)

    // Assert correct behavior
    XCTAssertNotNil(result, "BUG-1450: Should handle nil input without crash")
}
```

### Files With REAL Tests (Reference Examples)

| File | Bug Coverage | Pattern |
|------|--------------|---------|
| `iTermLineBlockArrayRaceTests.m` | RC-001 | Concurrent thread stress test |
| `RootCauseP0Tests.m` | RC-004, RC-005, RC-006 | Mock delegates, edge case inputs |
| `TmuxBugRegressionTests.m` | BUG-1079 to BUG-1103 | Parser edge cases |
| `SearchEngineTests.swift` | Search functionality | Full integration tests |

### What Makes a Test REAL

1. **Instantiates production class** - `let foo = ActualClass()`
2. **Calls production method** - `foo.methodThatHadBug()`
3. **Uses edge-case inputs** - nil, empty, boundary values
4. **Asserts behavior** - not crash, correct return value

### What Makes a Test FAKE

1. Reads source files as strings
2. Searches for patterns in code
3. Uses `loadSourceFile()`, `containsRegex()`, `sourceContains()`
4. Creates mock classes inside test methods (`class SafeFoo {}`)

## Metrics

| Category | Count |
|----------|-------|
| Total tests in BugRegressionTests.swift | 2,909 |
| Fake tests (using loadSourceFile) | 1,566 |
| Percentage fake | 54% |

## Next Steps for Worker

1. Run `xcodebuild test` - see which tests fail with "FAKE TEST DETECTED"
2. Pick a failing test
3. Understand what bug it's supposed to test (check upstream issues)
4. Rewrite to instantiate actual production code
5. Verify test passes
6. Repeat

**Goal:** Zero fake tests. Every test must exercise real production code.
