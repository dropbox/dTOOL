# Memory and Retriever Integration Test Guide

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Created:** 2025-11-02
**Phase:** Phase 3 - 100% Real Tests
**Scope:** Memory and retriever integration tests with real LLMs and embeddings

---

## Overview

This guide documents the comprehensive integration test suite for memory and retriever implementations created in N=532. These tests verify real execution with actual LLM calls, embeddings, and vector stores.

**Total Integration Tests:** 20 tests
- **Memory Tests:** 9 tests (LLM-based memory types)
- **Retriever Tests:** 11 tests (VectorStoreRetriever with real embeddings)

---

## Memory Integration Tests

### Test File Location
`crates/dashflow-memory/tests/memory_integration_tests.rs`

### Test Categories

#### 1. ConversationSummaryMemory Tests (3 tests)

**Purpose:** Verify LLM-based conversation summarization

**Tests:**
- `test_conversation_summary_memory_real` - Basic summarization workflow
  - Verifies progressive summary updates
  - Checks key information retention
  - Tests 2-turn conversation

- `test_conversation_summary_memory_clear_real` - Clear functionality
  - Verifies summary is cleared properly
  - Tests reset behavior

- `test_conversation_summary_memory_multi_turn_real` - Extended conversations
  - Tests 3+ turn conversations
  - Verifies summary quality across multiple turns
  - Checks key concept retention

#### 2. ConversationEntityMemory Tests (2 tests)

**Purpose:** Verify entity extraction and summarization

**Tests:**
- `test_conversation_entity_memory_real` - Entity extraction
  - Verifies entity extraction from conversation
  - Tests entity summary generation
  - Checks entity information updates

- `test_conversation_entity_memory_clear_real` - Clear functionality
  - Verifies entities are cleared properly

#### 3. ConversationTokenBufferMemory Tests (1 test)

**Purpose:** Verify token-based buffer management

**Tests:**
- `test_conversation_token_buffer_memory_real` - Token limit enforcement
  - Adds multiple messages exceeding token limit
  - Verifies old messages are pruned
  - Checks recent messages are retained

#### 4. VectorStoreRetrieverMemory Tests (3 tests)

**Purpose:** Verify semantic retrieval of conversation history

**Tests:**
- `test_vectorstore_retriever_memory_real` - Semantic retrieval
  - Stores conversations on different topics
  - Queries with Rust-related input
  - Verifies relevant memories are retrieved
  - Checks k parameter is respected

- `test_vectorstore_retriever_memory_clear_real` - Clear functionality
  - Verifies memories are cleared from vector store

- `test_vectorstore_retriever_memory_semantic_search_real` - Relevance testing
  - Stores memories on diverse topics (weather, cooking, geography)
  - Queries about cooking
  - Verifies only relevant memories retrieved
  - Checks irrelevant memories are not returned

### Running Memory Tests

**All memory integration tests:**
```bash
cargo test --test memory_integration_tests --package dashflow-memory -- --ignored
```

**Specific test:**
```bash
cargo test --test memory_integration_tests --package dashflow-memory test_conversation_summary_memory_real -- --ignored --exact
```

**Without API keys (all tests skip gracefully):**
```bash
cargo test --test memory_integration_tests --package dashflow-memory
# Output: 9 tests ignored
```

### Cost Estimates (Memory Tests)
- ConversationSummaryMemory: ~$0.01-0.02 per test (gpt-4o-mini)
- ConversationEntityMemory: ~$0.02-0.03 per test (extraction + summarization)
- ConversationTokenBufferMemory: ~$0.01 per test (tokenization only)
- VectorStoreRetrieverMemory: ~$0.01-0.02 per test (embeddings)
- **Total:** ~$0.05-0.10 per full memory test run

---

## Retriever Integration Tests

### Test File Location
`crates/dashflow-standard-tests/src/retriever_tests.rs`

> **Note**: The actual standard tests may have different test names than originally documented.
> Run `grep "pub async fn test_" crates/dashflow-standard-tests/src/retriever_tests.rs` to see current tests.

### Test Categories

#### 1. Similarity Search Tests (2 tests)

**Tests:**
- `test_vector_store_retriever_similarity_real` - Basic similarity search
  - Queries about "Rust programming"
  - Verifies k=3 documents retrieved
  - Checks top results mention Rust

- `test_vector_store_retriever_semantic_relevance_real` - Multi-query relevance
  - Tests 3 different query types
  - Verifies semantic relevance for each
  - Checks k=2 documents per query

#### 2. MMR (Maximal Marginal Relevance) Tests (2 tests)

**Purpose:** Verify diversity in retrieval results

**Tests:**
- `test_vector_store_retriever_mmr_real` - Lambda parameter effects
  - Tests lambda=1.0 (high relevance, no diversity)
  - Tests lambda=0.5 (balanced)
  - Compares results

- `test_vector_store_retriever_mmr_diversity_real` - Maximum diversity
  - Tests lambda=0.0 (maximum diversity)
  - Verifies k=4 diverse documents retrieved

#### 3. Score Threshold Tests (2 tests)

**Purpose:** Verify score-based filtering

**Tests:**
- `test_vector_store_retriever_score_threshold_real` - Moderate threshold (0.7)
  - Queries "Rust programming language systems"
  - Verifies only high-scoring matches returned
  - Checks result count ≤ k

- `test_vector_store_retriever_high_threshold_real` - Very high threshold (0.95)
  - Queries with vague content
  - Verifies few or zero results with high threshold

#### 4. Edge Case Tests (3 tests)

**Tests:**
- `test_vector_store_retriever_empty_query_real` - Empty query handling
  - Tests with empty string query
  - Verifies graceful handling (success or error)

- `test_vector_store_retriever_long_query_real` - Very long query
  - Tests with 1000+ character query
  - Verifies relevant results still retrieved

- `test_vector_store_retriever_special_chars_real` - Special characters
  - Tests queries with ?, &, /, [], (), etc.
  - Verifies all queries handled correctly

#### 5. Performance and Stress Tests (1 test)

**Tests:**
- `test_vector_store_retriever_performance_real` - Multiple queries
  - Runs 5 queries sequentially
  - Measures total and average time
  - Asserts avg time < 2 seconds per query

#### 6. Embedding Dimension Tests (1 test)

**Tests:**
- `test_vector_store_retriever_embedding_dimensions_real` - Real embeddings
  - Verifies text-embedding-3-small produces 1536-dim vectors
  - Tests retrieval with real embeddings

### Running Retriever Tests

> **Note**: The standard retriever tests are library functions in `dashflow-standard-tests` that are
> imported and called by other crates. To run retriever conformance tests:

**Run tests that use the standard retriever tests:**
```bash
cargo test --package dashflow-standard-tests -- retriever
```

**To see available test functions:**
```bash
grep "pub async fn test_" crates/dashflow-standard-tests/src/retriever_tests.rs
```

**Without API keys (tests that require API keys will skip):**
```bash
cargo test --package dashflow-standard-tests
# Tests requiring API keys are marked #[ignore]
```

### Cost Estimates (Retriever Tests)
- Each test: ~$0.01-0.02 (text-embedding-3-small at ~$0.02/1M tokens)
- Average 8 documents × 30 tokens = 240 tokens per test
- **Total:** ~$0.05-0.10 per full retriever test run

---

## Combined Test Execution

### Run All Memory and Retriever Tests

```bash
# Memory tests
cargo test --test memory_integration_tests --package dashflow-memory -- --ignored

# Retriever standard tests
cargo test --package dashflow-standard-tests -- retriever
```

### Total Cost Estimate
- Memory tests: ~$0.05-0.10
- Retriever tests: ~$0.05-0.10
- **Combined total:** ~$0.10-0.20 per full run

---

## Prerequisites

### Required Environment Variables
```bash
export OPENAI_API_KEY="sk-..."
```

### Models Used
- **LLMs:** gpt-4o-mini (~$0.15/1M input, $0.60/1M output)
- **Embeddings:** text-embedding-3-small (~$0.02/1M tokens)
- **Tokenizer:** tiktoken cl100k_base (local, no cost)

###Test Infrastructure
All tests are marked `#[ignore]` to prevent:
- Accidental API charges during CI/CD
- Unexpected costs in development
- Rate limit issues
- Flaky tests due to network

Tests gracefully skip when API keys are not set, printing:
```
Skipping test: OPENAI_API_KEY not set
```

---

## Test Data

### Memory Test Conversations
- Introduction scenarios (names, professions)
- Technical discussions (Rust, Arc, Box, ownership)
- Entity-rich conversations (people, places, companies)

### Retriever Test Documents
8 documents covering:
- Programming languages (Rust, Python)
- Geography (Paris, France)
- AI/ML concepts (machine learning, deep learning)
- Technology (vector databases, semantic search, DashFlow)

---

## Verification Results

### Compilation
- ✅ Memory tests: 9 tests compile successfully
- ✅ Retriever tests: 11 tests compile successfully
- ✅ Zero compilation errors or warnings

### Runtime (without API keys)
- ✅ Memory tests: All 9 tests gracefully ignored
- ✅ Retriever tests: All 11 tests gracefully ignored
- ✅ No panics, errors, or failures

### Test Quality
- ✅ All tests have clear documentation
- ✅ All tests have descriptive names
- ✅ All tests verify real functionality
- ✅ All tests check edge cases
- ✅ All tests assert on meaningful outputs

---

## Comparison to Agent/Chain Tests (N=529-531)

### Similarities
- All tests marked #[ignore] for safety
- All tests check for API keys and skip gracefully
- All tests use cost-efficient models
- All tests verify real execution
- All tests include comprehensive assertions

### Differences
- **Memory tests** verify stateful behavior (save/load/clear)
- **Retriever tests** focus on search quality (relevance, diversity, thresholds)
- **Agent/Chain tests** (N=529-530) verify multi-step reasoning and document processing

### Coverage
| Test Type | Count | Focus |
|-----------|-------|-------|
| Agent tests | 16 | Tool calling, reasoning |
| Chain tests | 13 | Document processing |
| Memory tests | 9 | Stateful conversations |
| Retriever tests | 11 | Search quality |
| **Total** | **49** | **Comprehensive** |

---

## Next Steps (N=533-534)

Per EXECUTION_PLAN_FINAL.md, N=532-534 covers "Convert memory/retriever tests (15-20h)":

### Remaining Work
1. **Persistent storage backends** (Optional, if time permits):
   - Redis chat message history integration tests
   - MongoDB chat message history integration tests
   - PostgreSQL chat message history integration tests

2. **Advanced retriever types** (Optional, if time permits):
   - MultiQueryRetriever with real LLM query generation
   - ContextualCompressionRetriever with real compression
   - EnsembleRetriever with multiple retrievers

3. **Documentation:**
   - ✅ Test execution guide (this document)
   - Integration with existing test infrastructure docs

### Priority
N=532 focused on:
1. **Core memory types with LLM integration** ✅
2. **VectorStoreRetriever with real embeddings** ✅

These are the most commonly used memory/retriever implementations and provide the highest value for integration testing.

---

## Maintenance Notes

### Updating Tests
- Models may change (gpt-4o-mini → newer models)
- Embedding dimensions may change (text-embedding-3-small: 1536 dims)
- API costs may change (update cost estimates in comments)

### Adding New Tests
Follow the pattern established:
1. Mark test `#[tokio::test]` and `#[ignore]`
2. Call `skip_if_no_api_key()` at start
3. Use cost-efficient models
4. Add comprehensive assertions
5. Include println! statements for visibility
6. Document in this guide

### Debugging Failures
When tests fail with API keys set:
1. Check API key is valid
2. Check rate limits
3. Check model availability
4. Check assertion logic
5. Run with `--nocapture` to see println! output

---

## Summary

N=532 successfully created 20 comprehensive integration tests:
- **9 memory tests** covering LLM-based memory types
- **11 retriever tests** covering search quality and edge cases

All tests:
- ✅ Compile successfully
- ✅ Skip gracefully without API keys
- ✅ Use cost-efficient models
- ✅ Verify real functionality
- ✅ Include comprehensive assertions
- ✅ Follow N=529-530 patterns

**Estimated total cost:** ~$0.10-0.20 per full run (20 tests)
**Value:** Comprehensive verification of memory and retriever behavior with real LLMs
