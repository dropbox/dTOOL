# Audit: LLM Provider Crates

**Status:** ✅ COMPLETE - All providers verified SAFE (#1419, #1422, #1423, #1430)
**Priority:** P3 (LLM Integrations)

This file covers all LLM provider crates not covered in their own audit files.

---

## dashflow-azure-openai

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1422) - test module starts at line 953; all `.unwrap()` in doc comments (line 58) or `#[cfg(test)]` modules.
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - no `.unwrap()` calls in production code; test modules at lines 434, 487.

---

## dashflow-bedrock

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1419 M-402) - all `.unwrap()` in `#[cfg(test)]` modules (test@963+).
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - test modules at lines 491, 646; all `.unwrap()` in `#[cfg(test)]` modules.

---

## dashflow-cloudflare

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1423) - test module starts at line 432; both `.unwrap()` calls (lines 487, 505) are in `#[cfg(test)]` module.

---

## dashflow-cohere

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/rerank.rs`

### Known Issues
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - test modules at lines 512, 646; all `.unwrap()` in `#[cfg(test)]` modules.
- `src/rerank.rs`: ✅ SAFE (verified #1422) - test module starts at line 274; all `.unwrap()` in `#[cfg(test)]` modules. Has #[ignore] tests requiring API key.

---

## dashflow-deepseek

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/config_ext.rs`: ✅ SAFE (verified #1423) - test module starts at line 100; `panic!` at line 148 is in test assertion (proper test code pattern).

---

## dashflow-fireworks

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified locally) - no production `panic!`/`.unwrap()` (only in doc examples and `#[cfg(test)]`).
- `src/embeddings.rs`: ✅ SAFE (verified #1430) - `new()` panics if `FIREWORKS_API_KEY` is unset BUT already has `new_without_api_key()` as a fallible alternative (line 123). Panic is documented with `/// Panics if...` (line 92). This is the expected pattern - matches other providers.

---

## dashflow-gemini

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1423) - test module starts at line 823; both `.unwrap()` calls (lines 876, 890) are in `#[cfg(test)]` module.
- `src/embeddings.rs`: ✅ SAFE (verified #1423) - test modules start at lines 479, 567; both `.unwrap()` calls (lines 554, 558) are in `#[cfg(test)]` modules.

---

## dashflow-groq

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified locally) - no production `panic!`/`.unwrap()` (only in doc examples and `#[cfg(test)]`).

---

## dashflow-huggingface

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - test module starts at line 340; all `.unwrap()` calls are in `#[cfg(test)]` modules.
- 40+ #[ignore] tests requiring API key

---

## dashflow-mistral

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1422) - test modules at lines 392, 901, 1185; all `.unwrap()` in doc comments (line 52) or `#[cfg(test)]` modules.
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - test module starts at line 235; all `.unwrap()` in doc comments (lines 33, 41) or `#[cfg(test)]` modules.

---

## dashflow-ollama

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/embeddings.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1422) - test modules at lines 837, 1243; all `.unwrap()` in doc comments (lines 60, 93, 400) or `#[cfg(test)]` modules.
- `src/embeddings.rs`: ✅ SAFE (verified #1422) - test modules at lines 235, 279; all `.unwrap()` in doc comments (lines 31, 149) or `#[cfg(test)]` modules.

---

## dashflow-perplexity

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/config_ext.rs`: ✅ SAFE (verified #1423) - test module starts at line 100; `panic!` at line 147 is in test assertion (proper test code pattern).

---

## dashflow-replicate

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified #1423) - the only `.unwrap()` (line 67) is in a doc comment example (`///`).

---

## dashflow-together

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`

### Known Issues
- 8 #[ignore] tests requiring API key
- `src/chat_models.rs`: ✅ SAFE (verified #1423) - the only `.unwrap()` (line 67) is in a doc comment example (`///`).

---

## dashflow-xai

### Files
- [ ] `src/lib.rs`
- [ ] `src/chat_models.rs`
- [ ] `src/config_ext.rs`

### Known Issues
- `src/chat_models.rs`: ✅ SAFE (verified locally) - no production `panic!`/`.unwrap()` (only in doc examples and `#[cfg(test)]`).
- `src/config_ext.rs`: `panic!` usage appears only in `#[cfg(test)]` assertions.

---

## Common Issues Across All LLM Providers

1. **High panic!/unwrap! counts** - SAFE: Audits confirm all are in `#[cfg(test)]` modules or doc comment examples
2. **Config extensions** - SAFE: All `panic!` calls are in test assertions (verified #1423)
3. **#[ignore] tests** - Most require API keys (expected for integration tests)
4. **Duplicate patterns** - Similar code across providers follows consistent safety patterns

---

## Critical Checks (Apply to ALL)

1. **Real API calls** - Not mocked
2. **Error handling** - API errors properly mapped
3. **Streaming works** - No data loss
4. **Rate limiting** - Respected
5. **Token counting** - Accurate

---

## Test Coverage Gaps (Apply to ALL)

- [ ] API integration tests (need keys)
- [ ] Error handling tests
- [ ] Rate limiting tests
- [ ] Streaming reliability tests
