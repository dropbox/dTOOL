# Prioritized Bug Fix List

**Generated:** December 24, 2025

## Summary

| Category | Count | Priority | Action |
|----------|-------|----------|--------|
| ✅ Truly Fixed | 85 | DONE | No action needed |
| 🟡 REAL test, no prod fix | 683 | **P1 - HIGH** | Add production fix |
| 🟠 MIXED test | 127 | **P2 - MEDIUM** | Clean up test + maybe add prod fix |
| 🔴 FAKE test | 1710 | **P3 - LOW** | Complete rewrite needed |
| ⚫ Outstanding (deleted) | 282 | **P4 - LOWEST** | Start from scratch |

---

## P1: REAL Tests Without Production Fix (683 bugs)

These are the **easiest wins** - the test is already good, just need to fix production code!

### Worker A: BUG-1 to BUG-500 (P1)
```
BUG-13, BUG-36, BUG-271, BUG-301, BUG-333, BUG-338, BUG-339, BUG-401, BUG-405, BUG-407
```

For each:
1. Read the test to understand what it's testing
2. Find the production code it calls
3. Add the fix (nil check, bounds check, etc.)
4. Run the test to verify it passes

### Worker B: BUG-501+ (P1)
```
BUG-664, BUG-673, BUG-676, BUG-677, BUG-678, BUG-679, BUG-687, BUG-808, BUG-816, BUG-818...
```

---

## P2: MIXED Tests (127 bugs)

These tests have both real and fake patterns. Need to:
1. Remove the fake patterns (NSClassFromString, instancesRespond)
2. Keep/enhance the real patterns
3. Maybe add production fix

```
BUG-2, BUG-6, BUG-7, BUG-10, BUG-12, BUG-19, BUG-20, BUG-21, BUG-22, BUG-26, BUG-27, BUG-28...
```

---

## P3: FAKE Tests (1710 bugs) - LOW PRIORITY

These need complete rewrites. Only work on these after P1 and P2 are done.

---

## P4: Outstanding/Deleted (282 bugs) - LOWEST PRIORITY

These had fake tests that were deleted. Bug IDs preserved in:
`docs/test-audit/deleted_fake_tests.txt`

Only work on these after P1, P2, P3 are done.

---

## How Workers Should Proceed

### PRIORITY ORDER:
1. **First**: Fix P1 bugs (REAL test exists, just add production fix)
2. **Second**: Clean up P2 bugs (MIXED tests)
3. **Third**: Rewrite P3 bugs (FAKE tests)
4. **Last**: Create P4 bugs from scratch

### For Each Bug:

```bash
# 1. Find the test
grep -A30 "func test_BUG_XXX" DashTerm2Tests/BugRegressionTests.swift

# 2. Understand what it tests
# Read the test body - what method does it call?

# 3. Find the production code
grep -rn "methodName" sources/

# 4. Add the fix
# Edit sources/SomeFile.swift - add nil check, bounds check, etc.

# 5. Run the test
xcodebuild test -only-testing:'DashTerm2Tests/BugRegressionTests/test_BUG_XXX'

# 6. Commit BOTH
git add sources/ DashTerm2Tests/
git commit -m "# N: Fix BUG-XXX - [description]"
```
