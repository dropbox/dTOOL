# MANAGER DIRECTIVE: Complete Tech Debt Paydown (N=205+)

**Date:** 2025-11-19
**Priority:** Continue maintenance until ALL tech debt paid
**For:** Worker N=205+
**Status:** Phases 1-3 COMPLETE (24/39 issues fixed), Continue with remaining issues

---

## MISSION

**Continue systematic tech debt elimination until repository is pristine.**

Work through remaining 15 issues from audit, then proactively find and fix additional tech debt. Keep working until user says "stop" or no more issues can be found.

**Approach:** Make lists → Fix systematically → Verify → Make new lists → Repeat

---

## PHASE 4: REMAINING CRITICAL ISSUES (N=205-210) - 2 Hours

### Issue #1: Fix Flaky OpenAI Test (ACTIVE)

**Test:** `dashflow-openai::chat_models::standard_tests::test_rapid_consecutive_calls_comprehensive`
- Fails in full suite
- Passes individually
- Likely: Race condition or rate limiting

**Action:**
```bash
# 1. Run test 10 times to reproduce
for i in {1..10}; do
  cargo test --package dashflow-openai --lib test_rapid_consecutive_calls_comprehensive
done

# 2. Check for:
- Shared state between tests
- Rate limiting issues
- Timing assumptions
- Non-deterministic behavior

# 3. Fix root cause
# Options:
- Add test isolation (unique API endpoint per test)
- Add retry logic with backoff
- Add sleep between rapid calls
- Mark #[ignore] with clear reason IF unfixable

# 4. Verify
cargo test --all --lib  # Must pass 100%
```

### Issue #2: Test OOM in dashflow-jina

**Problem:** Full test suite gets killed with SIGKILL

**Action:**
```bash
# 1. Run jina tests in isolation
cargo test --package dashflow-jina --lib -- --nocapture

# 2. Check memory usage
/usr/bin/time -l cargo test --package dashflow-jina --lib 2>&1 | grep "maximum resident"

# 3. Profile if needed
cargo test --package dashflow-jina --lib -- --nocapture 2>&1 | \
  grep -i "alloc\|memory"

# 4. Fix options:
- Reduce test dataset size
- Add #[cfg(not(test))] for heavy allocations
- Split large tests into smaller ones
- Add test cleanup (drop large structures)

# 5. Verify full suite completes
cargo test --workspace --lib  # Should not OOM
```

### Issue #3: Archive Cleanup (374 Markdown Files)

**Current:** 374 archived markdown files
**Target:** <50 essential files

**Action:**
```bash
# 1. List all archives
find . -path "*archive*" -name "*.md" > /tmp/archives.txt
wc -l /tmp/archives.txt

# 2. Group by category
# - Phase planning (keep representative samples)
# - Status checks (delete most, keep milestones)
# - Bug reports (keep if referenced)
# - Manager directives (keep active, archive old)

# 3. Delete obsolete archives
rm archive/phase_1_2_3_planning/SESSION_*.md  # Keep only plan summaries
rm archive/reports/*/N[0-9]*_STATUS_*.md  # Status checks
rm reports/all-to-rust2/archive/*.md  # Old branch work

# 4. Target: Keep <50 essential historical documents
find . -path "*archive*" -name "*.md" | wc -l  # Should be <50
```

### Issue #4: External Dependency Cleanup

**Problem:** `/Users/ayates/dsp_rs/` path breaks builds for others

**Action:**
```bash
# 1. Check if dashoptimize actually used
cargo tree | grep dashoptimize_

# 2. If used:
git submodule add https://github.com/<org>/dsp_rs dsp_rs
# Update paths in Cargo.toml

# 3. If not used:
# Remove dashoptimize_* from workspace members in Cargo.toml
# Verify build still works

# 4. Document in README
# "Note: DashOptimize integration requires..."
```

### Issue #5: Dead Code Audit

**Problem:** 20+ files with `#[allow(dead_code)]`

**Action:**
```bash
# 1. List all files
grep -r "#\[allow(dead_code)\]" --include="*.rs" crates/ -l > /tmp/dead_code.txt

# 2. For each file:
# a) Remove the #[allow(dead_code)]
# b) Compile: cargo check --package <crate>
# c) For each warning:
#    - Delete if truly dead
#    - Add #[cfg(test)] if test-only
#    - Add doc comment explaining why kept if part of public API

# 3. Target: <5 files with #[allow(dead_code)]
```

### Issue #6: Duplicate Dependency Cleanup

**Action:**
```bash
# 1. Full analysis
cargo tree --workspace -d > /tmp/dupes.txt

# 2. Identify version conflicts
grep "v1\." /tmp/dupes.txt | grep "v2\."  # Same dep, different versions

# 3. Fix with version constraints
# Add to Cargo.toml:
# [patch.crates-io]
# <crate> = { version = "X.Y" }

# 4. Verify
cargo tree --workspace -d  # Should have fewer duplicates
```

---

## PHASE 5: PROACTIVE TECH DEBT HUNTING (N=211-220) - Ongoing

**Goal:** Find and fix ALL remaining tech debt until pristine.

### Hunting Checklist - Run Each Session

**1. Code Quality Scans:**
```bash
# Unused imports
cargo clippy --workspace -- -W unused-imports 2>&1 | grep "warning"

# Unused variables
cargo clippy --workspace -- -W unused-variables 2>&1 | grep "warning"

# Missing docs
cargo clippy --workspace -- -W missing-docs 2>&1 | grep "warning" | wc -l

# Complex functions
cargo clippy --workspace -- -W clippy::cognitive_complexity 2>&1 | grep "warning"

# Magic numbers
cargo clippy --workspace -- -W clippy::unreadable_literal 2>&1 | grep "warning"
```

**2. Test Quality Scans:**
```bash
# Tests with generic names
grep -r "fn test_basic\|fn test_simple\|fn test_foo" --include="*.rs" crates/*/tests/

# Tests without assertions
grep -r "#\[test\]" -A 20 --include="*.rs" crates/ | grep -v "assert"

# Flaky patterns (sleep, timing)
grep -r "sleep\|std::thread::sleep\|Duration::from_" --include="*.rs" crates/*/tests/

# Hardcoded test values
grep -r "\"test\"|\"foo\"|\"bar\"|\"example\"" --include="*.rs" crates/*/tests/ | wc -l
```

**3. Documentation Scans:**
```bash
# Broken example code
cargo test --doc 2>&1 | grep "FAILED"

# Old file references
grep -r "legacy.rs\|old_\|backup_" --include="*.md" docs/

# Dead links
find . -name "*.md" -exec grep -l "](.*)" {} \; | \
  xargs -I {} sh -c 'echo "Checking {}"; grep -o "](.*)" {} | sed "s/](\|)//g"' | \
  sort -u > /tmp/links.txt

# Outdated version references
grep -r "v1\.[0-9]\.[0-9]" --include="*.md" . | grep -v "v1.11.0\|archive"

# Inconsistent claims
grep -r "production ready\|Production Ready" --include="*.md" . | wc -l
```

**4. File Organization Scans:**
```bash
# Temp files
find . -name "*~" -o -name "*.bak" -o -name "*.tmp" | grep -v target

# Large files (>1MB)
find . -type f -size +1M | grep -v target | grep -v .git

# Empty files
find . -type f -empty | grep -v target

# Duplicate filenames
find . -name "*.rs" -o -name "*.md" | xargs basename -a | sort | uniq -d
```

**5. Dependency Scans:**
```bash
# Unused dependencies
cargo machete 2>/dev/null || cargo-udeps --workspace

# Old versions
cargo outdated --workspace

# Security advisories
cargo audit

# Dependency tree depth
cargo tree --workspace --depth 1 | wc -l
```

---

## EXECUTION PATTERN

### Each Session:

**1. Run ALL 5 Scans Above**
Document findings in `/tmp/tech_debt_scan_N<iteration>.txt`

**2. Pick Top 5-10 Issues by Impact**
Prioritize:
- Bugs/Failures (highest)
- False documentation
- Code that breaks builds
- Major complexity
- Easy wins (quick fixes with high impact)

**3. Fix Systematically**
One commit per logical group:
- "# N: Fixed 5 unused import warnings"
- "# N: Removed 3 empty test files"
- "# N: Updated 10 outdated doc references"

**4. Verify After Each Fix**
```bash
cargo test --workspace --lib
cargo clippy --workspace
./scripts/verify_documentation_claims.sh
```

**5. Create New Issue List for Next Session**
Document what's left, what's next

---

## SPECIFIC REMAINING TASKS

### Task List 1: Code Quality (N=205-208)

- [ ] Fix flaky OpenAI test
- [ ] Fix jina OOM issue
- [ ] Audit remaining 26 TODO markers (target: 0 in production code)
- [ ] Clean up #[allow(dead_code)] (20 files → 0)
- [ ] Remove unused imports (if any)
- [ ] Fix any missing error handling in examples
- [ ] Standardize collection merge naming

### Task List 2: Documentation (N=209-212)

- [ ] Run link checker on all markdown files
- [ ] Fix any broken links found
- [ ] Verify all example code compiles (`cargo test --doc`)
- [ ] Check for outdated version references
- [ ] Ensure all performance claims have context
- [ ] Verify all test count references consistent

### Task List 3: Files & Organization (N=213-216)

- [ ] Archive cleanup: 374 → <50 files
- [ ] Remove any remaining temp files
- [ ] Check for empty files
- [ ] Remove backup files (*~, *.bak)
- [ ] Optimize large files (>1MB) if any

### Task List 4: Dependencies (N=217-220)

- [ ] Fix external dsp_rs dependency
- [ ] Run cargo-udeps to find unused deps
- [ ] Check cargo audit for vulnerabilities
- [ ] Unify duplicate dependency versions
- [ ] Update outdated dependencies (if safe)

---

## SUCCESS METRICS

**Target State:**
```bash
# Verification script
./scripts/verify_documentation_claims.sh
# Output: All checks passing ✅

# Code quality
cargo clippy --workspace -- -D warnings
# Output: 0 warnings

# Tests
cargo test --workspace --lib
# Output: 100% pass rate, no OOM, no flaky tests

# TODO markers
grep -r "TODO\|FIXME\|HACK" --include="*.rs" crates/ | wc -l
# Output: 0 (or <10 with documented reasons)

# Dead code
grep -r "#\[allow(dead_code)\]" --include="*.rs" crates/ | wc -l
# Output: 0 (or <3 with documented reasons)

# Archives
find . -path "*archive*" -name "*.md" | wc -l
# Output: <50

# Documentation
cargo test --doc 2>&1 | grep "FAILED"
# Output: (no failures)

# Dependencies
cargo tree --workspace -d | wc -l
# Output: <10 duplicates

# Unused dependencies
cargo machete
# Output: No unused dependencies

# Security
cargo audit
# Output: No vulnerabilities
```

---

## REPORTING CADENCE

### Every 5 Commits (N=205, 210, 215, 220...):

Create brief status report in `/tmp/` (not reports/, avoid bloat):

```bash
cat > /tmp/tech_debt_status_N<N>.txt << EOF
# Tech Debt Status - N=<N>

## Completed This Session
- Fixed: X issues
- Cleaned: Y TODOs
- Removed: Z dead code instances

## Metrics
- TODO markers: <current count>
- Dead code allows: <current count>
- Archive files: <current count>
- Tests: <pass count> passed, <fail count> failed

## Next Session Focus
- <top 3 issues to tackle>

## Overall Progress
- Critical issues: X/4 remaining
- High issues: X/10 remaining
- Medium issues: X/9 remaining
- Low issues: X/6 remaining
EOF
```

**Don't commit these** - they're working notes, not permanent reports.

---

## WHEN TO CREATE ACTUAL REPORTS

**Only create reports in reports/main/ for:**
1. Major milestone completion (e.g., "All 39 issues resolved")
2. Bug discoveries with analysis
3. Performance improvements with measurements
4. Architectural decisions

**NOT for:**
- Routine maintenance
- Small fixes
- Status checks
- "Everything is fine" updates

---

## CONTINUOUS IMPROVEMENT MINDSET

### Questions to Ask Every Session:

1. **"What claims in docs might be false?"**
   - Grep for numbers, percentages, "production", "complete"
   - Verify each one

2. **"What code is dead or unused?"**
   - Run with allows removed
   - Delete what compiler says is dead

3. **"What TODOs are done?"**
   - Read each TODO
   - If feature exists, remove TODO

4. **"What tests are brittle?"**
   - Look for magic numbers
   - Look for timing assumptions
   - Look for hardcoded "test" values

5. **"What documentation is outdated?"**
   - Check dates
   - Check version numbers
   - Check file references

### Proactive Scans:

```bash
# New TODOs introduced?
git diff HEAD~10 HEAD | grep "+.*TODO" | wc -l

# New dead code?
git diff HEAD~10 HEAD | grep "+.*#\[allow(dead_code)\]"

# New ignored tests?
git diff HEAD~10 HEAD | grep "+.*#\[ignore\]"

# New magic numbers?
git diff HEAD~10 HEAD | grep "+.*assert_eq.*[0-9][0-9][0-9]"

# Dependencies added?
git diff HEAD~10 HEAD -- '**/Cargo.toml' | grep "+.*="
```

---

## SPECIFIC REMAINING WORK

### Must Do (N=205-210):

**1. Fix Flaky OpenAI Test** (30 min)
```bash
# Already identified, needs fixing
# Worker is already aware (seen test failures)
```

**2. Fix Jina OOM** (45 min)
```bash
# Profile memory usage
# Reduce test dataset size or split tests
# Ensure full suite completes without kill
```

**3. Finish Dead Code Cleanup** (60 min)
```bash
# 20 files with #[allow(dead_code)] → 0
# Remove allows, let compiler guide
```

**4. External Dependency Fix** (30 min)
```bash
# Remove /Users/ayates/dsp_rs/ reference
# Or add as git submodule
# Ensure others can build
```

**5. Final Archive Cleanup** (30 min)
```bash
# 374 files → <50
# Keep only milestone summaries
# Delete status checks
```

### Should Do (N=211-215):

**6. Cargo Audit** (15 min)
```bash
cargo audit
# Fix any vulnerabilities found
```

**7. Unused Dependencies** (30 min)
```bash
cargo machete || cargo-udeps
# Remove unused deps
```

**8. Link Checking** (20 min)
```bash
# Install if needed: npm install -g markdown-link-check
find . -name "*.md" -not -path "./target/*" -exec markdown-link-check {} \;
# Fix broken links
```

**9. Doc Test Verification** (15 min)
```bash
cargo test --doc --workspace 2>&1 | grep "FAILED"
# Fix failing doc examples
```

**10. Example Error Handling** (45 min)
```bash
# Add API key validation to top 5 examples
# Add proper error messages
# Example: "Error: OPENAI_API_KEY not set. Run: export OPENAI_API_KEY=sk-..."
```

### Could Do (N=216-220):

**11. Naming Consistency** (30 min)
- Standardize collection merge strategies
- Document naming conventions

**12. Security Scan** (20 min)
```bash
cargo audit
cargo-deny check advisories
```

**13. Benchmark Verification** (30 min)
- Re-run benchmarks/python_comparison
- Verify performance claims still accurate

**14. README Optimization** (30 min)
- Remove redundant sections
- Improve scannability
- Add table of contents links

**15. CLAUDE.md Further Reduction** (20 min)
- 415 lines → 350 lines
- Move more to COMPLETED_INITIATIVES.md
- Focus on active project only

---

## COMMIT STRATEGY

**Small, Focused Commits:**
- One logical change per commit
- Clear "what and why" in message
- Easy to review and revert if needed

**Batch Similar Fixes:**
```
# 205: Removed 50 completed TODO markers from dashflow-core
# 206: Removed 30 completed TODO markers from dashflow
# 207: Cleaned dead code from 5 integration crates
# 208: Cleaned dead code from 5 tool crates
```

**Not:**
```
# 205: Fixed lots of stuff
```

---

## MAINTAINING MOMENTUM

### If You Get Stuck:

**On a hard bug:**
- Document the issue clearly
- Add #[ignore] with extensive TODO explaining blocker
- Move to next issue
- Come back later

**On a big task:**
- Break into smaller pieces
- Do first 25%, commit
- Do next 25%, commit
- Incremental progress

**On ambiguous decision:**
- Document both options in commit message
- Pick the simpler/safer one
- Note decision for future reference

### If You Run Out of Issues:

**1. Run all 5 scans again**
**2. Check git log for patterns:**
```bash
# What keeps breaking?
git log --oneline --all | grep -i "fix.*test\|flaky" | wc -l

# What keeps getting updated?
git log --oneline --all --since="1 week ago" -- README.md | wc -l
```

**3. Read CLAUDE.md carefully:**
- Any contradictions?
- Any outdated instructions?
- Any missing processes?

**4. Check for emerging patterns:**
```bash
# New files being created frequently?
git log --oneline HEAD~50..HEAD --name-only | sort | uniq -c | sort -rn | head -20

# Certain packages with lots of churn?
git log --oneline HEAD~50..HEAD --name-only | grep "crates/" | cut -d/ -f2 | sort | uniq -c | sort -rn
```

---

## STOP CONDITIONS

**Stop working when:**
1. ✅ All 5 verification scans return 0 issues
2. ✅ Verification script passes all checks
3. ✅ `cargo test --workspace --lib` = 100% pass, no OOM
4. ✅ TODO markers = 0 (or <5 with reasons documented)
5. ✅ Dead code allows = 0 (or <3 with reasons documented)
6. ✅ Archive files <50
7. ✅ No broken links
8. ✅ No doc test failures
9. ✅ Working tree clean
10. ✅ User says "stop" or "ready to merge"

**OR:**
- Context reaches 60%
- Blocking decision needed from user

---

## CURRENT STATUS (N=204)

### ✅ Completed (24/39 Issues):
- Version management fixed
- All critical factual claims corrected
- 907 TODO markers cleaned (933 → 26)
- legacy.rs deleted
- Temp files deleted
- CLAUDE.md reduced
- Status reports archived
- Verification script created and passing
- Release notes created
- outputs/ in gitignore

### ⚠️ In Progress (2/39):
- Flaky OpenAI test
- Test OOM issue

### 🔜 Remaining (13/39):
- Archive cleanup (374 files)
- External dependency (dsp_rs)
- Dead code audit (20 files)
- Duplicate dependencies
- Link checking
- Doc tests
- Example error handling
- Naming consistency
- Security audit
- Unused dependencies
- Benchmark re-verification
- README optimization
- Final polish

**Estimated:** 2-3 hours to complete ALL remaining

---

## EXCELLENCE STANDARDS

**Every commit should:**
- ✅ Build successfully
- ✅ Pass all tests
- ✅ Pass clippy
- ✅ Have clear commit message
- ✅ Leave working tree clean

**Every session should:**
- ✅ Make measurable progress
- ✅ Document what was done
- ✅ Identify next steps
- ✅ Verify quality maintained

**Every fix should:**
- ✅ Solve root cause (not symptom)
- ✅ Be verifiable
- ✅ Not add new tech debt
- ✅ Improve code quality

---

**For Worker N=205+:** Continue systematic tech debt elimination. Run scans, make lists, fix issues, verify, repeat. Work until pristine or user says stop.
