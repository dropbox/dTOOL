# WORKER: STOP - Build Integration Tests NOW

**Date:** November 11, 2025
**Status:** URGENT REDIRECT
**User asked:** "is this happening now?"

---

## Answer: NO - Integration Tests Are NOT Being Built

**You are at:** N=1299 (working on async batch iterator)

**Integration test directory:** EMPTY (tests/integration/ has no tests)

**You ignored the directive** from commit a1a139e3e2

---

## User Wants Proof NOW

**User directive (direct quote):**
> "I want you to create an integration test suite that enforce these behaviors end to end. Be skeptical and rigorous"

**Then asked:**
> "is this happening now?"

**Answer:** NO. You're still building features (N=1288-1299).

---

## STOP Building Features

**Your N=1288-1299 work:**
- trim_messages() extended features
- filter_messages() utility
- Message utility functions
- Async batch iterator
- [Other utilities]

**This is good work BUT NOT what user wants right now.**

---

## START Building Integration Tests

**Your N=1300: Create Integration Test Infrastructure**

**Commands:**
```bash
cd ~/dashflow

# Create integration test structure
mkdir -p tests/integration

# Create test infrastructure
cat > tests/integration/mod.rs << 'EOF'
//! End-to-end integration tests with real APIs
//!
//! Run with: cargo test --test integration -- --ignored

#[cfg(test)]
mod common;

#[cfg(test)]
mod test_tool_calling_e2e;

#[cfg(test)]
mod test_react_agent_e2e;

#[cfg(test)]
mod test_dashstream_e2e;

#[cfg(test)]
mod test_app_document_search_e2e;

#[cfg(test)]
mod test_failure_modes;

#[cfg(test)]
mod test_output_quality;
EOF

# Create common utilities
cat > tests/integration/common/mod.rs << 'EOF'
//! Common utilities for integration tests

use dashflow::core::messages::Message;

pub fn load_test_env() {
    dotenvy::dotenv().ok();
}

pub fn get_openai_key() -> String {
    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY required")
}

pub fn get_kafka_brokers() -> String {
    std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into())
}

pub fn verify_answer_quality(answer: &str, expected_keywords: &[&str]) -> bool {
    let answer_lower = answer.to_lowercase();
    expected_keywords.iter().any(|kw| answer_lower.contains(kw))
}
EOF

git add tests/integration/
git commit -m "# 1300: Integration test infrastructure created"
```

---

## Your N=1301: First Integration Test - Tool Calling

**Create:** `tests/integration/test_tool_calling_e2e.rs`

**Use template from DIRECTIVE_END_TO_END_INTEGRATION_TESTS.md**

**Test must:**
- Use real OpenAI API
- Bind real tool
- Verify LLM calls tool
- Verify tool executes
- Verify result is correct
- Be skeptical (check LLM didn't hallucinate)

**Run it:**
```bash
cargo test --test integration test_bind_tools -- --ignored --nocapture
```

**If fails:** Fix the issue, prove feature works

**If passes:** Commit and move to next test

---

## Your N=1302-1308: Build All Integration Tests

**One test file per commit:**
- N=1302: test_react_agent_e2e.rs
- N=1303: test_dashstream_e2e.rs
- N=1304: test_app_document_search_e2e.rs
- N=1305: test_app_advanced_rag_e2e.rs
- N=1306: test_app_code_assistant_e2e.rs
- N=1307: test_failure_modes.rs
- N=1308: test_output_quality.rs

**Each test must:**
- Run against real APIs
- Verify end-to-end behavior
- Check failure modes
- Measure performance
- Validate outputs

---

## Your N=1309: Run All Integration Tests and Report

**Command:**
```bash
cargo test --test integration -- --ignored --nocapture > integration_test_results.txt 2>&1
```

**Create report:** `reports/all-to-rust2/integration_test_report_n1309.md`

**Must document:**
- How many tests: __/20
- Pass rate: __%
- Failures: [list each failure with details]
- Performance: [measured times]
- Issues found: [bugs discovered]
- Proof features work: YES/NO with evidence

**If <100% pass rate:** Fix issues before claiming done

---

## This is NOT Optional

**User specifically requested:**
- Integration test suite
- End-to-end enforcement
- Skeptical and rigorous

**You have not done this yet.**

**Priority:**
1. Stop building more features
2. Build integration test suite (N=1300-1308)
3. Run tests and prove they pass
4. Then continue with features OR user will give next direction

---

## Files to Reference

**Read these for specifications:**
1. **DIRECTIVE_END_TO_END_INTEGRATION_TESTS.md** - Complete test specs with code examples
2. **TEST_PHILOSOPHY.md** - Testing strategy and verification approach

**Your exact next task:**
```bash
# Create tests/integration/mod.rs
# Create tests/integration/common/mod.rs
# Commit as N=1300
```

**Then build test files N=1301-1308**

**Do this NOW. User is waiting for proof features work.**
