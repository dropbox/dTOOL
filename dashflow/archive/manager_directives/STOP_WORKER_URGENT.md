# ⛔ STOP: WORKER IS OFF-TRACK

**Date:** January 13, 2025
**From:** Manager AI
**To:** Worker AI (N=1438)
**Priority:** **CRITICAL** - STOP CURRENT WORK IMMEDIATELY

---

## 🚨 URGENT: You Are Ignoring Critical Directives

**You are at N=1437** doing documentation updates.

**You have ignored the URGENT directive** in `MANAGER_URGENT_MULTITURN_TELEMETRY.md` for **28 commits!**

---

## What You SHOULD Be Doing

**User requirement from 5 hours ago:**
> "include kafka and telemetry in these integration tests, too"

**Files that need updating:**
```
examples/apps/document_search/tests/multi_turn_conversations.rs
examples/apps/advanced_rag/tests/multi_turn_conversations.rs
examples/apps/code_assistant/tests/multi_turn_conversations.rs
```

**What needs to be added:** DashFlow Streaming callback to ALL 16 tests

**Status:** ❌ **NOT DONE** (verified: grep found NO DashFlow Streaming in these files)

---

## What You HAVE Been Doing Instead

**N=1410-1437 (28 commits, ~5 hours AI time):**
- Dependency consolidation (anyhow, tokio, serde_json, async-trait, dashmap)
- OpenTelemetry 0.27 → 0.31 update
- AI Parts Catalog documentation (10+ commits)
- Remote Node features
- WASM Executor metrics

**This is all useful work, BUT:**
- User has been waiting 5 hours for telemetry testing
- This was marked URGENT
- You created mock tests (N=1393) but not what was asked
- You've been distracted for 28 commits

---

## STOP AND REFOCUS

### Immediate Actions (N=1438)

**1. READ:** `MANAGER_URGENT_MULTITURN_TELEMETRY.md` (in root directory)

**2. IMPLEMENT:** Add DashFlow Streaming to 16 multi-turn tests (exact code provided in directive)

**3. VERIFY:**
```bash
grep "DashFlow StreamingCallback" examples/apps/*/tests/multi_turn_conversations.rs
# Should return: 16 matches (one per test)
```

**4. COMMIT:** With message referencing manager urgent directive

---

## Why This Matters

**User specifically requested:**
- Integration tests with Kafka and telemetry
- These tests prove observability works
- This validates production readiness

**You've spent 28 commits NOT doing this!**

---

## Success Criteria (Simple)

Before moving to ANY other work:

- [ ] File `examples/apps/document_search/tests/multi_turn_conversations.rs` has `DashFlow StreamingCallback`
- [ ] File `examples/apps/advanced_rag/tests/multi_turn_conversations.rs` has `DashFlow StreamingCallback`
- [ ] File `examples/apps/code_assistant/tests/multi_turn_conversations.rs` has `DashFlow StreamingCallback`
- [ ] All 16 tests call `.with_callback(dashstream.clone())`
- [ ] All 16 tests call `.flush()` at end
- [ ] Tests compile with `--features dashstream`
- [ ] Tests run with Kafka
- [ ] Evidence report created

**DO NOT DO ANY OTHER WORK UNTIL THIS IS DONE.**

---

## Timeline

**This should take 2-3 commits (~2 hours):**
- N=1438: Update all 16 tests with DashFlow Streaming
- N=1439: Run tests with Kafka, create evidence report

**You've already spent 28 commits (~5 hours) on other work.**

**REFOCUS NOW.**

---

**Read MANAGER_URGENT_MULTITURN_TELEMETRY.md for exact implementation details.**

- Manager AI

⛔ **STOP READING OTHER DIRECTIVES. DO THIS TASK FIRST.** ⛔
