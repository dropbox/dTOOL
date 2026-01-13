# Integration Test Execution Guide

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)
**Phase:** Phase 3 - 100% Real Tests

---

## Overview

This guide documents the comprehensive integration test suite created in N=529-530 and verified in N=531. These tests verify real execution of agents and chains with actual LLM providers.

**Total Integration Tests:** 28 tests
- **Agent Tests:** 15 tests (7 OpenAI + 8 Anthropic)
- **Chain Tests:** 13 tests (10 API-dependent + 3 pure transformation)

---

## Test Categories

### Agent Integration Tests

**OpenAI Agent Tests (7 tests):**
- `test_agent_simple_calculation` - Single tool use, verify correct answer
- `test_agent_multi_step_reasoning` - Multi-step problem solving
- `test_agent_no_tool_needed` - Agent decides no tool required
- `test_agent_multiple_tools` - Multiple tools, agent chooses correctly
- `test_agent_custom_function_tool` - Custom FunctionTool implementation
- `test_agent_max_iterations_limit` - AgentExecutor configuration constraints
- `test_agent_tool_with_json_output` - Tools returning structured data

**Anthropic Agent Tests (8 tests):**
- Same 7 tests as OpenAI
- `test_agent_thinking_and_reasoning` - Claude-specific complex reasoning (additional test)

**Location:**
- `crates/dashflow-openai/tests/agent_integration_tests.rs`
- `crates/dashflow-anthropic/tests/agent_integration_tests.rs`

---

### Chain Integration Tests

**Document Combining Chains (3 tests):**
- `test_stuff_documents_chain_basic` - Combine all documents into single prompt
- `test_map_reduce_documents_chain` - Process documents in parallel, then combine
- `test_refine_documents_chain` - Iteratively refine answer with each document

**Summarization Chains (3 tests):**
- `test_summarize_chain_stuff` - Convenience wrapper for Stuff pattern
- `test_summarize_chain_map_reduce` - Convenience wrapper for MapReduce pattern
- `test_summarize_chain_refine` - Convenience wrapper for Refine pattern

**HyDE - Hypothetical Document Embeddings (2 tests):**
- `test_hyde_basic` - Generate hypothetical document for query, embed it
- `test_hyde_custom_prompt` - Domain-specific prompts (fiqa for financial)

**Edge Cases (2 tests):**
- `test_stuff_documents_custom_separator` - Custom document separator
- `test_stuff_documents_empty_docs` - Handle empty document list

**Pure Transformation Tests (3 tests - NO API REQUIRED):**
- `test_transform_chain_basic` - Pure transformation (uppercase)
- `test_transform_chain_multiple_outputs` - Multiple outputs from single input
- `test_transform_chain_error_handling` - Error propagation

**Location:**
- `crates/dashflow-chains/tests/chain_integration_tests.rs`

---

## Running Tests

### Prerequisites

**API Keys Required:**
```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

**Models Used:**
- OpenAI: `gpt-4o-mini` (~$0.15/1M input, $0.60/1M output)
- OpenAI Embeddings: `text-embedding-3-small` (~$0.02/1M tokens)
- Anthropic: `claude-3-5-haiku-20241022` (~$0.25/1M input, $1.25/1M output)

**Estimated Cost:** ~$0.05-0.10 per full test run (all 28 tests)

---

### Run All Integration Tests

**All agent and chain tests (25 tests, requires API keys):**
```bash
# OpenAI agent tests (7 tests)
cargo test --test agent_integration_tests --package dashflow-openai -- --ignored

# Anthropic agent tests (8 tests)
cargo test --test agent_integration_tests --package dashflow-anthropic -- --ignored

# Chain tests with API (10 tests)
cargo test --test chain_integration_tests --package dashflow-chains -- --ignored

# Pure transformation tests (3 tests, no API keys needed)
cargo test --test chain_integration_tests --package dashflow-chains
```

**Run everything in one command:**
```bash
cargo test --test agent_integration_tests -- --ignored && \
cargo test --test chain_integration_tests --package dashflow-chains -- --ignored && \
cargo test --test chain_integration_tests --package dashflow-chains
```

---

### Run Specific Test Categories

**Single provider agents:**
```bash
# OpenAI only (7 tests)
cargo test --test agent_integration_tests --package dashflow-openai -- --ignored

# Anthropic only (8 tests)
cargo test --test agent_integration_tests --package dashflow-anthropic -- --ignored
```

**Chain categories:**
```bash
# Document combining chains (3 tests)
cargo test --test chain_integration_tests --package dashflow-chains test_stuff_documents_chain -- --ignored
cargo test --test chain_integration_tests --package dashflow-chains test_map_reduce_documents_chain -- --ignored --exact
cargo test --test chain_integration_tests --package dashflow-chains test_refine_documents_chain -- --ignored --exact

# Summarization chains (3 tests)
cargo test --test chain_integration_tests --package dashflow-chains test_summarize_chain -- --ignored

# HyDE tests (2 tests)
cargo test --test chain_integration_tests --package dashflow-chains test_hyde -- --ignored

# Pure transformation tests (3 tests, no API needed)
cargo test --test chain_integration_tests --package dashflow-chains test_transform_chain
```

---

### Run Individual Tests

**With output visible:**
```bash
# Agent test
cargo test --test agent_integration_tests --package dashflow-openai test_agent_simple_calculation -- --ignored --exact --nocapture

# Chain test
cargo test --test chain_integration_tests --package dashflow-chains test_stuff_documents_chain_basic -- --ignored --exact --nocapture

# Transformation test (no API)
cargo test --test chain_integration_tests --package dashflow-chains test_transform_chain_basic -- --exact --nocapture
```

---

## Test Behavior

### With API Keys Present

**Tests execute real LLM calls:**
- Make actual HTTP requests to OpenAI/Anthropic APIs
- Verify tool calling, agent reasoning, chain execution
- Assert on output quality (topics present, structure correct)
- Print outputs for manual inspection
- Tests should complete in 2-60 seconds per test (depending on complexity)

**Example output:**
```
running 1 test
Agent output: The sum of 123 + 456 is 579.

Agent used these tools:
- calculator: add(123, 456) -> 579

Agent execution took 3 iterations.
test test_agent_simple_calculation ... ok
```

---

### Without API Keys

**Tests skip gracefully:**
- Check for API key at start
- Print clear message: `"Skipping test: OPENAI_API_KEY not set"`
- Return `Ok(())` immediately
- Test marked as passing (not failed, not ignored)
- No API calls made, no charges incurred

**Example output:**
```
running 1 test
Skipping test: OPENAI_API_KEY not set
test test_agent_simple_calculation ... ok

test result: ok. 1 passed; 0 failed; 0 ignored
```

**Pure transformation tests always run:**
- No API calls, deterministic logic
- Tests pass or fail based on logic correctness
- No API keys needed

---

## Safety Features

### Protection Against Accidental API Charges

**All API-dependent tests marked `#[ignore]`:**
- Normal `cargo test` runs will NOT execute these tests
- Must explicitly use `--ignored` flag to run
- Prevents accidental API charges in CI/CD
- Prevents rate limit issues during normal development

**Graceful skipping:**
- Tests check for API keys before execution
- Missing keys → skip gracefully (not fail)
- No panics, no test failures

**Cost-efficient models:**
- Uses cheapest models that provide quality results
- OpenAI: gpt-4o-mini (not gpt-4)
- Anthropic: haiku (not opus/sonnet)
- Embeddings: small models (not large)

---

## Verification Status

**Compilation:** ✅
- All agent tests compile successfully
- All chain tests compile successfully
- Zero compilation errors or warnings

**Graceful Skipping:** ✅
- OpenAI agent tests skip gracefully without OPENAI_API_KEY
- Anthropic agent tests skip gracefully without ANTHROPIC_API_KEY
- Chain tests skip gracefully without OPENAI_API_KEY
- No errors, panics, or test failures when keys missing

**Pure Transformation Tests:** ✅
- All 3 transformation tests run without API keys
- All tests pass
- Deterministic, fast execution

**Test Counts Verified:** ✅
- OpenAI agents: 7 tests
- Anthropic agents: 8 tests
- Chain tests (API): 10 tests
- Chain tests (pure): 3 tests
- Total: 28 tests

---

## Expected Behavior with Real API Keys

**Not verified in N=531** (API keys not available in environment)

### Expected Results

When API keys are available, tests should:

1. **Execute successfully:** All tests should pass
2. **Make real API calls:** Verify with actual LLM responses
3. **Verify tool calling:** Agents should correctly use tools
4. **Verify chain execution:** Chains should process documents correctly
5. **Complete in reasonable time:** 2-60 seconds per test
6. **Cost ~$0.05-0.10:** For full test run (28 tests)

### Potential Issues to Watch For

**Agent tests:**
- Tool calling variations between providers
- Non-deterministic reasoning paths (may need multiple runs)
- Rate limiting (run with delays if needed)
- Model refusals (if prompts trigger safety filters)

**Chain tests:**
- Output quality variations (LLMs not 100% deterministic)
- Document separator handling
- Empty document edge cases
- HyDE embedding quality

**Mitigation:**
- Tests use `temperature=0.0` for determinism
- Assertions check for key topics, not exact strings
- Multiple assertion patterns for robustness
- Clear error messages when assertions fail

---

## Next Steps for Comprehensive Execution

### When API Keys Available

**Phase 1: Smoke Test (5-10 minutes)**
1. Run 1-2 agent tests from each provider
2. Run 1-2 chain tests
3. Verify basic functionality works
4. Check API charges are reasonable

**Phase 2: Full Test Run (15-30 minutes)**
1. Run all OpenAI agent tests (7 tests)
2. Run all Anthropic agent tests (8 tests)
3. Run all chain tests (10 API + 3 pure)
4. Document any failures

**Phase 3: Failure Analysis (variable)**
1. If failures occur, investigate each
2. Determine if test needs adjustment or code has bug
3. Re-run failed tests after fixes
4. Document lessons learned

---

## Test Maintenance

### Adding New Tests

**Follow N=529-530 patterns:**
1. Mark API tests with `#[tokio::test]` and `#[ignore]`
2. Check for API keys at start, skip if missing
3. Use cost-efficient models
4. Assert on output quality, not exact matches
5. Print outputs for manual inspection
6. Document test purpose clearly

**Example template:**
```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_my_new_feature() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Skipping test: OPENAI_API_KEY not set");
            return Ok(());
        }
    };

    // Create client with cost-efficient model
    let client = ChatOpenAI::new()
        .with_api_key(&api_key)
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    // Test logic here
    // ...

    // Assert on quality, not exact matches
    assert!(output.contains("expected topic"));

    Ok(())
}
```

---

## Related Documentation

- **N=529 Report:** reports/make_everything_rust/agent_integration_tests_N529_2025-11-02-04-12.md
- **N=530 Report:** reports/make_everything_rust/chain_integration_tests_N530_2025-11-02-05-22.md
- **N=531 Report:** (this session) Integration test verification and execution guide
- **Execution Plan:** EXECUTION_PLAN_FINAL.md (Phase 3, N=529-531)

---

## Summary

**Integration test suite status:**
- ✅ 28 comprehensive integration tests created
- ✅ All tests compile successfully
- ✅ All tests skip gracefully without API keys
- ✅ Pure transformation tests run without API keys
- ✅ Cost-efficient model selection
- ✅ Safety features in place (#[ignore], API key checks)
- ⏳ Real API execution pending (requires API keys in environment)

**Ready for execution when:**
- OPENAI_API_KEY available in environment
- ANTHROPIC_API_KEY available in environment
- Budget approved for API charges (~$0.05-0.10 per run)

**Commands to run when ready:**
```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Run all integration tests
cargo test --test agent_integration_tests -- --ignored && \
cargo test --test chain_integration_tests --package dashflow-chains -- --ignored && \
cargo test --test chain_integration_tests --package dashflow-chains
```
