# Skeptical Audit: web_research_retriever.rs

**Auditor:** Worker #1824
**Date:** 2025-12-25
**File:** `crates/dashflow/src/core/retrievers/web_research_retriever.rs`
**Lines:** 1,007
**Status:** Issues found and fixed

---

## Summary

The `WebResearchRetriever` generates search queries using an LLM, fetches web content, and retrieves relevant documents. This audit found a **critical P2 bug** that made the retriever completely non-functional.

---

## Issues Found

### M-1084 (P2 - Correctness) - FIXED

**Problem:** `WebResearchRetriever::load_url()` used `HTMLLoader` instead of `URLLoader`.

- `HTMLLoader` is a **file-based** loader that reads from disk via `std::fs::read()`
- `URLLoader` is an **HTTP-based** loader that fetches from URLs via reqwest
- The retriever passed URLs to `HTMLLoader`, which tried to read them as file paths

**Impact:** The entire web research retriever was non-functional. Any attempt to fetch web content would fail with an I/O error.

**Fix:**
- Line 18: Changed import from `HTMLLoader` to `URLLoader`
- Line 276: Changed loader instantiation from `HTMLLoader::new(url)` to `URLLoader::new(url)`

**Verification:** All 18 unit tests pass.

---

## Additional Observations (Not Fixed - Informational)

### URLLoader vs NewsLoader SSRF Protection (P3 - Security Enhancement)

`URLLoader` does NOT have SSRF protection, unlike `NewsLoader` which calls `http_client::validate_url_for_ssrf()`. The web research retriever passes user-influenced URLs from search results without validation.

**Recommendation:** Consider adding SSRF validation in a future iteration if the retriever is used with untrusted search backends.

### Silent Error Handling When verbose=false (P4 - Observability)

Lines 232-238 and 315-319: URL load failures and search failures are only logged at debug level when `verbose=true`. In production with `verbose=false`, failures are completely silent.

**Recommendation:** Consider using `tracing::warn!` or `tracing::debug!` unconditionally for operational visibility.

### Hardcoded similarity_search k=4 (P4 - Configuration)

Line 356: The number of results retrieved per query (4) is hardcoded. Should be configurable for different use cases.

---

## Files Changed

| File | Change |
|------|--------|
| `crates/dashflow/src/core/retrievers/web_research_retriever.rs` | Fixed M-1084: HTMLLoader -> URLLoader |
| `ROADMAP_CURRENT.md` | Added M-1084 to NOW/NEXT table |

---

## Test Results

```
running 18 tests
test core::retrievers::web_research_retriever::tests::test_url_database_prevents_duplicate_processing ... ok
test core::retrievers::web_research_retriever::tests::test_retriever_trait_implementation ... ok
test core::retrievers::web_research_retriever::tests::test_mock_text_splitter ... ok
test core::retrievers::web_research_retriever::tests::test_load_and_index_urls_empty_list ... ok
test core::retrievers::web_research_retriever::tests::test_clean_search_query_edge_cases ... ok
...
test result: ok. 18 passed; 0 failed; 0 ignored
```

---

## Conclusion

The `WebResearchRetriever` had a fundamental bug that prevented it from working at all. The fix is straightforward and low-risk - simply using the correct loader type. The additional observations are informational and can be addressed in future iterations if needed.
