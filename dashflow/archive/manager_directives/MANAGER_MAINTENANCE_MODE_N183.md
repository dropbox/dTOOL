# MANAGER DIRECTIVE: Continuous Maintenance Mode (N=183+)

**Date:** 2025-11-19
**Status:** Active
**For:** Worker N=183 and beyond
**Context:** Cleanup phases A, B, C complete (N=173-182). Repository in excellent state.

---

## MISSION

Continue maintenance mode with focus on:
1. Test stability and quality
2. Code quality improvements
3. Documentation accuracy
4. Bug fixes

**Work until user returns to merge.** Stay in maintenance mode - no major new features.

---

## CONTINUOUS MAINTENANCE TASKS

### 1. Test Quality & Stability

**Every session:**
```bash
# Run full test suite
cargo test --all --lib

# Check for flaky tests
cargo test --all --lib -- --test-threads=1

# Verify no new ignored tests added
git diff HEAD -- '**/*.rs' | grep "#\[ignore\]"
```

**If tests fail:**
- Investigate immediately
- Fix the bug (per CLAUDE.md: "FAILING TESTS ARE GOOD - THEY REVEAL BUGS")
- Never ignore failing tests without thorough investigation
- Document fix in commit message

**If tests are flaky:**
- Add retries with proper logging
- Investigate timing issues
- Fix race conditions
- Stabilize before continuing

### 2. Code Quality Monitoring

**Every N mod 5 (N=185, 190, 195, etc.):**
```bash
# Check for new TODOs/FIXMEs
grep -r "TODO\|FIXME" --include="*.rs" crates/ | wc -l
# Target: <60 (currently 57)

# Run clippy
cargo clippy --all-targets

# Check for unused dependencies
cargo machete || cargo-udeps

# Verify documentation builds
cargo doc --no-deps --document-private-items
```

**Address issues found:**
- Resolve TODOs if simple (<30 min)
- Fix clippy warnings immediately
- Remove unused dependencies
- Fix broken doc links

### 3. Documentation Accuracy

**Check for factual errors:**
- README test counts match actual counts
- Version numbers current
- Examples run without errors
- Links not broken

**If found:**
- Update immediately
- Verify with actual measurements
- Commit: "Documentation accuracy - Fixed [specific issue]"

### 4. Small Bug Fixes

**Acceptable maintenance work:**
- Fix compiler warnings
- Fix clippy lints
- Resolve simple TODOs
- Fix documentation errors
- Improve error messages
- Add missing tests for existing code

**NOT acceptable (wait for user):**
- New features
- Major refactorings
- API changes
- New dependencies
- Architecture changes

### 5. Status Verification

**Every session, check:**
```bash
# Build status
cargo build --all

# Test status
cargo test --all --lib

# Working tree
git status

# Ahead of origin
git status | grep "ahead"
```

**Report if:**
- Build breaks
- Tests fail
- Working tree has uncommitted changes at session end
- Any blocking issues found

---

## ITERATION PROTOCOL

### Each Session (N++)

1. **Start:** Read last 3 commit messages + CLAUDE.md
2. **Check:** Run test suite
3. **Work:** Fix any issues found, or improve code quality
4. **Verify:** Tests still pass
5. **Commit:** Clear message describing work
6. **Report:** Status in commit message

### Commit Message Format

```
# N: [Brief description of maintenance work]
**Current Plan**: Continuous maintenance (no formal plan)
**Checklist**: [What was done]

## Changes
[Specific changes made]

## Verification
- Build: [passing/failing]
- Tests: [X passed, Y failed, Z ignored]
- Clippy: [clean/warnings]

## Next AI: Continue maintenance mode
[Any specific notes for next worker]
```

### When to STOP

**Stop work if:**
1. User says "stop" or "ready to merge"
2. Context window reaches 60%
3. Major blocker found that needs user decision
4. Working tree has uncommitted changes and unclear how to proceed

**Leave clear note in uncommitted file if stopped mid-work.**

---

## MAINTENANCE CHECKLIST

Track these goals across sessions:

- [ ] **Tests:** All passing (currently ✅)
- [ ] **Build:** Clean (currently ✅)
- [ ] **Clippy:** No warnings (currently ✅)
- [ ] **TODOs:** <50 in production code (currently 57 - very close)
- [ ] **Ignored tests:** All documented with reasons (currently ✅)
- [ ] **Working tree:** Clean at session end (currently ✅)
- [ ] **Documentation:** Accurate and current (currently ✅)

---

## WHAT SUCCESS LOOKS LIKE

**Good maintenance session:**
- Found and fixed 1-2 small issues
- All tests still passing
- Working tree clean
- Clear commit message
- Ready for next worker or user

**Bad maintenance session:**
- Started new feature work
- Left uncommitted changes
- Broke tests
- Added complexity
- Unclear what was done

---

## CURRENT STATUS (N=182)

**Achieved:**
- ✅ legacy.rs deleted (41K lines)
- ✅ Temp files deleted
- ✅ CLAUDE.md reduced (521 → 415 lines)
- ✅ Old reports archived
- ✅ kg.rs bugs fixed (2 tests)
- ✅ Code quality verified
- ✅ 5,174 tests passing

**Metrics:**
- Tests: 5,174 passed, 0 failed, 404 ignored
- TODOs/PLACEHOLDERs: 57 (target <50)
- Ignored tests: All legitimate (API keys, Docker, cloud)
- CLAUDE.md: 415 lines (from 521)

**Ready for merge:** YES ✅

---

## EXAMPLES OF GOOD MAINTENANCE WORK

### Example 1: Found Compiler Warning
```
# 185: Fixed unused import warning in chains module

## Changes
- Removed unused `use std::time::Duration;` from chains/retrieval.rs
- Compiler warned: "unused import"
- Simple fix, no functional change

## Verification
- Build: passing
- Tests: 5,174 passed, 0 failed
- Clippy: clean
```

### Example 2: Found Documentation Error
```
# 187: Fixed README test count (was 2,619, now 5,174)

## Changes
- Updated dashflow-core/README.md test count
- Previous: "2,619 tests"
- Actual (measured): 5,174 tests
- Ran: cargo test --all --lib | grep "test result"

## Verification
Measurement accurate as of N=187.
```

### Example 3: Fixed Simple TODO
```
# 190: Implemented TODO in prompts/base.rs - Added validation

## Changes
- Resolved TODO(validation): Added input parameter validation
- Added check for empty template string
- Added test: test_prompt_template_validation_empty
- No API changes, fully backward compatible

## Verification
- Build: passing
- Tests: 5,175 passed (new test added)
- Clippy: clean
```

---

## WHAT TO AVOID

**Don't:**
- Start major refactorings
- Add new features
- Change APIs
- Install new dependencies
- Merge branches
- Push to origin (wait for user)
- Spend >2 hours on single issue
- Leave uncommitted work

**Do:**
- Small improvements
- Bug fixes
- Documentation updates
- Test stability
- Code quality
- Keep commits small and focused

---

## DURATION

**Continue until:** User says ready to merge

**Expected:** Could be 1 day, could be 1 week. Stay in maintenance mode.

**Check-in:** Every N mod 5, create brief status report showing:
- Current N
- Maintenance work done since last check-in
- Repository health (build, tests, quality)
- Any issues found

---

**For Next Worker (N=183+):** Read this directive, execute maintenance mode, stay alert for user's return.
