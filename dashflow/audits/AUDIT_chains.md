# Audit: dashflow-chains

**Status:** NOT STARTED
**Files:** 30 src + tests + examples
**Priority:** P0 (Chain Orchestration)

---

## File Checklist

### Source Files
- [ ] `src/lib.rs` - Module exports
- [ ] `src/api.rs` - API chain
- [ ] `src/constitutional_ai.rs` - Constitutional AI
- [ ] `src/conversation.rs` - Conversation chain
- [ ] `src/conversational_retrieval.rs` - Conversational retrieval
- [ ] `src/cypher_utils.rs` - Cypher utilities
- [ ] `src/flare.rs` - FLARE chain
- [ ] `src/graph_cypher_qa.rs` - Graph Cypher QA
- [ ] `src/graph_qa.rs` - Graph QA
- [ ] `src/hyde.rs` - HyDE chain
- [ ] `src/llm.rs` - LLM chain (CRITICAL)
- [ ] `src/llm_checker.rs` - LLM checker
- [ ] `src/llm_math.rs` - LLM math chain
- [ ] `src/llm_requests.rs` - LLM requests
- [ ] `src/moderation.rs` - Moderation chain
- [ ] `src/qa_generation.rs` - QA generation
- [ ] `src/qa_with_sources.rs` - QA with sources
- [ ] `src/retrieval.rs` - Retrieval chain
- [ ] `src/retrieval_qa.rs` - Retrieval QA
- [ ] `src/router.rs` - Router chain
- [ ] `src/sequential.rs` - Sequential chain
- [ ] `src/sql_database_chain.rs` - SQL database chain
- [ ] `src/sql_database_prompts.rs` - SQL prompts
- [ ] `src/summarize.rs` - Summarization
- [ ] `src/transform.rs` - Transform chain

### src/combine_documents/
- [ ] `mod.rs` - Module
- [ ] `map_reduce.rs` - Map-reduce
- [ ] `refine.rs` - Refine
- [ ] `stuff.rs` - Stuff documents

### src/natbot/
- [ ] `mod.rs` - Natbot module
- [ ] `chain.rs` - Natbot chain
- [ ] `crawler.rs` - Web crawler
- [ ] `prompt.rs` - Natbot prompts

### Test Files
- [ ] `tests/chain_integration_tests.rs`
- [ ] `tests/natbot_tests.rs`

### Example Files
- [ ] `examples/01_basic_llm_chain.rs`
- [ ] `examples/02_sequential_chain.rs`
- [ ] `examples/03_stuff_documents.rs`
- [ ] `examples/04_hyde_retrieval.rs`

---

## Known Issues Found

### FakeLLM Usage in Tests
**Files using FakeLLM:** (Verified 2025-12-30)
- `src/graph_qa.rs` (lines 514, 709, 721, 747, 763, 798)
- `src/graph_cypher_qa.rs` (lines 427, 475, 511, 534, 559, 590)
- `src/llm_checker.rs` (lines 274, 385, 398, 411, 420, 437)
- `src/qa_with_sources.rs` (line 525)

**Action:** Verify all FakeLLM usage is within `#[cfg(test)]` blocks

### MockLLM in Test Code ✅
**`src/llm.rs:208-209`:**
```rust
// Mock LLM for testing
struct MockLLM;
```

**RESOLVED (2025-12-30):** MockLLM is defined inside `mod tests {}` block (line 200). It is test code, not production code. No action needed.

### Panic Patterns
High .unwrap() counts across files - need review

---

## Critical Checks

1. **All chains work with real LLMs** - Not just mocks
2. **Sequential chains handle errors** - Proper error propagation
3. **Memory integration works** - Conversation chains
4. **Retrieval chains use real retrievers** - Not mocked
5. **SQL injection prevention** - In sql_database_chain

---

## Test Coverage Gaps

- [ ] End-to-end chain tests with real LLMs
- [ ] Error propagation through chain steps
- [ ] Memory persistence tests
- [ ] Large document handling
- [ ] Timeout handling
