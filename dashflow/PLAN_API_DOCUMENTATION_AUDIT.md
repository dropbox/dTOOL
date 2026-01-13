# Plan: API Documentation & Design Audit (Phase 231 Reimagined)

**Created:** 2025-12-16
**Purpose:** Transform 589 undocumented APIs from technical debt into strategic advantage
**Status:** APPROVED - Execute after roadmap phases 277, 278, 280

---

## Vision

This is NOT just "add rustdoc comments." This is a **systematic audit** that:

1. **Documents every public API** with examples
2. **Audits for quality** - Is this API well-designed?
3. **Audits for redundancy** - Are there duplicate APIs?
4. **Audits for intelligibility** - Can someone understand without reading source?
5. **Feeds into introspection** - Documentation becomes queryable by AI

**End state:** Any AI worker can run `dashflow introspect ask "How do I use RetryPolicy?"` and get accurate, helpful documentation.

---

## Why This Matters

### For Librarian Development
- Workers building Librarian need to understand DashFlow APIs
- Undocumented APIs force workers to read source code
- Reading source wastes time and introduces errors

### For DashFlow Quality
- Documentation forces design review
- "If I can't explain it, maybe it's poorly designed"
- Redundant APIs become obvious when documenting similar things

### For AI Self-Improvement
- Introspection can only answer questions about documented APIs
- Undocumented APIs are invisible to `dashflow introspect`
- Documentation enables AI-assisted development

---

## Current State

```bash
# Run this to see current state
./scripts/check_docs.sh
```

**589 public items missing documentation across 108 crates**

| Category | Crates | Est. Undocumented |
|----------|--------|-------------------|
| Core (dashflow) | 1 | ~150 |
| Providers | 15 | ~100 |
| Vector Stores | 22 | ~150 |
| Tools | 6 | ~30 |
| Checkpointers | 4 | ~20 |
| Integrations | 25 | ~80 |
| Infrastructure | 12 | ~40 |
| Utilities | 13 | ~20 |
| Other | 11 | ~10 |

---

## Documentation Standard

Every public item MUST have:

### 1. One-Line Summary
```rust
/// Executes a compiled graph with the given initial state.
pub async fn invoke(&self, state: S) -> Result<S>
```

### 2. Detailed Description (if non-obvious)
```rust
/// Executes a compiled graph with the given initial state.
///
/// The graph execution follows the defined edges, executing nodes in
/// topological order. Parallel nodes execute concurrently. Conditional
/// edges are evaluated after each node completes.
///
/// # Execution Flow
/// 1. Start at the START node
/// 2. Execute each reachable node
/// 3. Evaluate conditional edges
/// 4. Continue until END node or error
```

### 3. Examples
```rust
/// # Examples
///
/// ```rust
/// use dashflow::prelude::*;
///
/// let graph = StateGraph::<MyState>::new()
///     .add_node("process", process_fn)
///     .add_edge(START, "process")
///     .add_edge("process", END)
///     .compile()?;
///
/// let result = graph.invoke(initial_state).await?;
/// ```
```

### 4. Errors (if applicable)
```rust
/// # Errors
///
/// Returns `Error::NodeNotFound` if a referenced node doesn't exist.
/// Returns `Error::CycleDetected` if the graph contains cycles.
/// Returns `Error::Timeout` if execution exceeds the configured timeout.
```

### 5. See Also
```rust
/// # See Also
///
/// - [`StateGraph::new`] - Create a new graph
/// - [`CompiledGraph::stream`] - Stream execution events
/// - [`RunnableConfig`] - Configure execution options
```

---

## Audit Criteria

While documenting, evaluate each API against:

### Quality Checklist
| Criterion | Question | Action if Fails |
|-----------|----------|-----------------|
| **Naming** | Does the name clearly convey purpose? | Log IMP-XXX for rename |
| **Signature** | Are parameters intuitive? | Log IMP-XXX for redesign |
| **Defaults** | Are defaults sensible? | Log IMP-XXX for change |
| **Errors** | Are error types helpful? | Log IMP-XXX for improvement |
| **Consistency** | Does it match similar APIs? | Log IMP-XXX for alignment |

### Redundancy Checklist
| Check | Action |
|-------|--------|
| Is there another API that does the same thing? | Document relationship, consider deprecation |
| Is this a subset of another API? | Consider consolidation |
| Are there multiple ways to achieve the same result? | Document preferred approach |

### Intelligibility Checklist
| Check | Action |
|-------|--------|
| Can a new user understand this without reading source? | If no, improve docs |
| Is the type signature self-explanatory? | If no, add type aliases or newtypes |
| Are edge cases documented? | If no, add Errors/Panics sections |

---

## Audit Report Format

For each crate, generate a report:

```markdown
# API Audit Report: dashflow-openai

## Summary
- Public items: 45
- Previously documented: 12
- Newly documented: 33
- Quality issues found: 5
- Redundancy issues found: 2
- Intelligibility issues found: 3

## Quality Issues

### QUAL-001: Confusing Parameter Name
**Location:** `ChatOpenAI::new(model: &str)`
**Problem:** Parameter `model` could mean model name or Model struct
**Recommendation:** Rename to `model_name` or use newtype `ModelId`
**Severity:** Medium

### QUAL-002: ...

## Redundancy Issues

### REDU-001: Duplicate Embedding Functions
**Location:** `embed_documents()` and `embed_texts()`
**Problem:** Both do the same thing with different names
**Recommendation:** Deprecate `embed_texts()`, keep `embed_documents()`
**Severity:** Low

## Intelligibility Issues

### INTL-001: Unclear Return Type
**Location:** `ChatModel::generate() -> Result<ChatResponse>`
**Problem:** ChatResponse has 12 fields, unclear which are important
**Recommendation:** Add doc explaining common usage patterns
**Severity:** Medium

## Metrics
- Doc coverage before: 27%
- Doc coverage after: 100%
- Quality score: 8.5/10
- Estimated improvement effort: 3 commits
```

---

## Introspection Integration

### Phase 1: Doc Indexing

Add documentation to module registry:

```rust
// In module_registry.rs
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub description: String,
    pub doc_summary: Option<String>,      // NEW: One-line summary
    pub doc_full: Option<String>,         // NEW: Full rustdoc
    pub examples: Vec<String>,            // NEW: Code examples
    pub see_also: Vec<String>,            // NEW: Related items
}
```

### Phase 2: Doc Search

Enable searching documentation:

```bash
# Search docs for keyword
dashflow introspect docs search "retry policy"
> Found 5 items:
>   1. dashflow::RetryPolicy - Configure retry behavior for failed operations
>   2. dashflow::RetryConfig - Low-level retry configuration
>   3. dashflow_openai::with_retry - Add retry to OpenAI calls
>   ...

# Show full docs for an item
dashflow introspect docs show dashflow::RetryPolicy
> # RetryPolicy
>
> Configure retry behavior for failed operations.
>
> ## Example
> ```rust
> let policy = RetryPolicy::exponential()
>     .with_max_retries(3)
>     .with_jitter(true);
> ```
>
> ## See Also
> - RetryConfig
> - CircuitBreaker
```

### Phase 3: AI Query Integration

Enable natural language queries:

```bash
dashflow introspect ask "How do I add retry to my graph?"
> Based on the documentation:
>
> Use `RetryPolicy` to configure retry behavior:
>
> ```rust
> let policy = RetryPolicy::exponential()
>     .with_max_retries(3)
>     .with_base_delay(Duration::from_millis(100));
>
> graph.add_node_with_config("api_call", my_fn, NodeConfig {
>     retry_policy: Some(policy),
>     ..Default::default()
> });
> ```
>
> See also: CircuitBreaker for failure thresholds
```

### Phase 4: Doc Quality in Health Checks

```bash
dashflow introspect health
> ...
> Documentation Coverage:
>   ✅ Core crate: 98% documented (147/150 items)
>   ⚠️  dashflow-openai: 85% documented (38/45 items)
>   ❌ dashflow-qdrant: 45% documented (22/49 items)
>
> Recommendation: Run `dashflow introspect docs audit dashflow-qdrant`
```

---

## Implementation Phases

### Phase 231A: Documentation Infrastructure (1 commit)

**Create tooling:**

1. `scripts/audit_docs.py` - Identify undocumented items by crate
   ```bash
   python scripts/audit_docs.py --crate dashflow-openai
   > Undocumented public items in dashflow-openai:
   >   - ChatOpenAI::new (function)
   >   - ChatOpenAI::with_config (function)
   >   - ChatResponse (struct)
   >   - ChatResponse::content (field)
   >   ...
   ```

2. `scripts/doc_quality_check.py` - Score documentation quality
   ```bash
   python scripts/doc_quality_check.py --crate dashflow-openai
   > Documentation Quality Score: 6.5/10
   >   - Has summary: 85%
   >   - Has examples: 30%
   >   - Has errors section: 15%
   >   - Has see also: 10%
   ```

3. Update `dashflow introspect health` to include doc coverage

**Deliverables:**
- [ ] `scripts/audit_docs.py`
- [ ] `scripts/doc_quality_check.py`
- [ ] Doc coverage in health checks

---

### Phase 231B: Core Crate Documentation (5-8 commits)

**Document the main `dashflow` crate - most important APIs**

Priority order:
1. **Graph types:** StateGraph, CompiledGraph, Node, Edge
2. **Execution:** invoke, stream, RunnableConfig
3. **State:** MergeableState, Checkpoint, CheckpointMetadata
4. **Resilience:** RetryPolicy, CircuitBreaker, RateLimiter
5. **Quality:** QualityGate, QualityScore
6. **Introspection:** All introspection module APIs
7. **Self-improvement:** All self_improvement module APIs

**Per-commit scope:** ~20 items documented + audit report

**Deliverables:**
- [ ] 100% doc coverage for dashflow crate
- [ ] `reports/audit_dashflow_core.md`
- [ ] IMP-XXX issues for quality/redundancy/intelligibility problems

---

### Phase 231C: Provider Crate Documentation (3-4 commits)

**Document all 15 LLM provider crates**

Apply consistent patterns:
- `ChatModel` implementations
- `EmbeddingModel` implementations
- Configuration structs
- Error types

**Crates:**
openai, anthropic, azure-openai, bedrock, cohere, deepseek, fireworks, gemini, groq, mistral, ollama, perplexity, replicate, together, voyage

**Per-commit scope:** 3-4 providers documented

**Deliverables:**
- [ ] 100% doc coverage for all provider crates
- [ ] `reports/audit_providers.md`
- [ ] Consistency report (do all providers follow same patterns?)

---

### Phase 231D: Vector Store Documentation (3-4 commits)

**Document all 22 vector store crates**

Apply consistent patterns:
- `VectorStore` trait implementation
- `similarity_search` methods
- Filter types
- Connection configuration

**Crates:**
chroma, elasticsearch, faiss, lancedb, milvus, mongodb, opensearch, pgvector, pinecone, qdrant, redis, sqlite-vec, sqlitevss, supabase, surrealdb, tigris, typesense, vald, weaviate, ...

**Per-commit scope:** 5-6 vector stores documented

**Deliverables:**
- [ ] 100% doc coverage for all vector store crates
- [ ] `reports/audit_vector_stores.md`
- [ ] Consistency report

---

### Phase 231E: Tools & Integration Documentation (2-3 commits)

**Document remaining crates:**
- Tools: shell, file, json, human, calculator, webscrape
- Checkpointers: postgres, redis, s3, dynamodb
- Integrations: Various integration crates

**Per-commit scope:** ~30 items documented

**Deliverables:**
- [ ] 100% doc coverage for remaining crates
- [ ] `reports/audit_tools_integrations.md`

---

### Phase 231F: Design Review Report (1 commit)

**Compile all findings into comprehensive report:**

```markdown
# DashFlow API Design Review

## Executive Summary
- Total public APIs: 589
- Quality issues found: X
- Redundancy issues found: Y
- Intelligibility issues found: Z

## Critical Issues (must fix)
...

## High Priority Issues (should fix)
...

## Medium Priority Issues (nice to fix)
...

## Recommended Consolidations
...

## Recommended Deprecations
...
```

**Deliverables:**
- [ ] `reports/API_DESIGN_REVIEW.md`
- [ ] Prioritized list of improvements
- [ ] Estimated effort for each fix

---

### Phase 231G: Introspection Integration (2-3 commits)

**Make documentation queryable:**

1. Index rustdoc into module registry
2. Add `dashflow introspect docs search <query>`
3. Add `dashflow introspect docs show <item>`
4. Enhance `dashflow introspect ask` to use docs
5. Add doc coverage to health checks

**Deliverables:**
- [ ] `dashflow introspect docs` subcommand
- [ ] Doc-aware `introspect ask`
- [ ] Doc coverage in `introspect health`

---

## Execution Order

| Phase | Commits | Depends On | Deliverable |
|-------|---------|------------|-------------|
| 231A | 1 | None | Tooling |
| 231B | 5-8 | 231A | Core docs |
| 231C | 3-4 | 231A | Provider docs |
| 231D | 3-4 | 231A | Vector store docs |
| 231E | 2-3 | 231A | Tool/integration docs |
| 231F | 1 | 231B-E | Design review |
| 231G | 2-3 | 231F | Introspection |
| **Total** | **17-26** | | |

---

## Success Criteria

### Documentation Coverage
- [ ] 100% of public items have doc comments
- [ ] 90%+ have examples
- [ ] 80%+ have See Also references

### Audit Quality
- [ ] Every crate has audit report
- [ ] All quality issues logged as IMP-XXX
- [ ] Critical issues have fix plans

### Introspection Integration
- [ ] `dashflow introspect docs search` works
- [ ] `dashflow introspect ask` uses documentation
- [ ] `dashflow introspect health` shows doc coverage

### Design Improvements
- [ ] 10+ IMP-XXX issues identified
- [ ] 5+ redundant APIs identified for consolidation
- [ ] Comprehensive design review report

---

## Synergy with Librarian

This work directly enables Librarian development:

| Librarian Need | How Docs Help |
|----------------|---------------|
| Use StateGraph | Documented with examples |
| Add memory | Checkpointer docs with patterns |
| Fan out search | Parallel execution docs |
| Self-improvement | Full self_improvement module docs |
| Graph viewer | Visualization API docs |

**Workers building Librarian can query:**
```bash
dashflow introspect ask "How do I add checkpointing to my graph?"
> Based on documentation:
>
> Use a Checkpointer implementation:
> ```rust
> let checkpointer = PostgresCheckpointer::new(db_url).await?;
> let graph = graph.with_checkpointer(checkpointer);
> ```
> ...
```

---

## Timeline

| Week | Phases | Commits |
|------|--------|---------|
| 1 | 231A (tooling) + 231B (core start) | 5 |
| 2 | 231B (core complete) + 231C (providers) | 8 |
| 3 | 231D (vector stores) + 231E (tools) | 6 |
| 4 | 231F (review) + 231G (introspection) | 4 |

**Total: ~23 commits over 4 weeks of worker time**

---

## Appendix: Scripts

### audit_docs.py (skeleton)

```python
#!/usr/bin/env python3
"""Identify undocumented public items in a crate."""

import subprocess
import json
import sys

def get_undocumented(crate: str) -> list[dict]:
    """Run cargo doc and parse warnings for missing docs."""
    result = subprocess.run(
        ["cargo", "doc", "-p", crate, "--no-deps"],
        capture_output=True,
        text=True,
        env={"RUSTDOCFLAGS": "-D missing_docs"}
    )
    # Parse stderr for missing doc warnings
    # Return list of {name, type, location}
    ...

if __name__ == "__main__":
    crate = sys.argv[1] if len(sys.argv) > 1 else "dashflow"
    items = get_undocumented(crate)
    for item in items:
        print(f"  - {item['name']} ({item['type']}) at {item['location']}")
```

### doc_quality_check.py (skeleton)

```python
#!/usr/bin/env python3
"""Score documentation quality for a crate."""

import re
from pathlib import Path

def score_doc(doc: str) -> dict:
    """Score a doc comment."""
    return {
        "has_summary": len(doc.split("\n")[0]) > 10,
        "has_example": "# Example" in doc or "```" in doc,
        "has_errors": "# Error" in doc,
        "has_see_also": "See Also" in doc or "See also" in doc,
    }

def audit_crate(crate_path: Path) -> dict:
    """Audit all doc comments in a crate."""
    ...
```
