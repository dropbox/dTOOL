# DashFlow Example Applications

**Last Updated:** 2026-01-05 (Worker #2516 - Fix stale codex-dashflow test count: 80→75)

This document catalogs the example applications in `examples/apps/`.

---

## Current Example Applications

As of v1.11.3, the example applications have been consolidated to focus on the highest-quality, most maintainable examples.

### Librarian (Superhuman Librarian - Ultimate RAG Paragon)

**Location:** `examples/apps/librarian/`

**Pattern:** Production-ready RAG agent demonstrating DashFlow's full capabilities

**What It Demonstrates:**
- StateGraph-based agent architecture
- Vector store integration for document retrieval
- Multi-turn conversations with context management
- Quality evaluation with LLM-as-judge
- Cost tracking and observability
- Parallel tool execution (DashFlow pattern)

**Run:**
```bash
# Requires OpenAI API key
export OPENAI_API_KEY="sk-..."

# Run the librarian
cargo run -p librarian -- query "What is async programming in Rust?"

# With evaluation
cargo run -p librarian -- eval
```

**Features:**
- Full DashFlow StateGraph implementation
- Demonstrates best practices for agent design
- Comprehensive test suite (17 E2E/integration tests + 54 unit tests = 71 total)
- Production-ready error handling

---

### Codex DashFlow (AI Code Assistant Paragon)

**Location:** `examples/apps/codex-dashflow/`

**Pattern:** AI-powered code generation, explanation, and refactoring

**What It Demonstrates:**
- Using `dashflow::generate()` for LLM calls with automatic telemetry
- ReAct agent pattern via `create_react_agent` → `CompiledGraph<AgentState>`
- Tracing/observability integration with info_span
- Multiple specialized modules (generator, explainer, refactorer, test_generator, docs_generator)
- Chat mode with conversation memory

**Run:**
```bash
# Requires OpenAI API key
export OPENAI_API_KEY="sk-..."

# Generate code
cargo run -p codex-dashflow -- generate "write a function that parses CSV"

# Explain existing code
cargo run -p codex-dashflow -- explain --file src/lib.rs

# Suggest refactors
cargo run -p codex-dashflow -- refactor --file src/lib.rs

# Generate tests
cargo run -p codex-dashflow -- test --file src/lib.rs

# Chat mode
cargo run -p codex-dashflow -- chat
```

**Features:**
- Full DashFlow platform integration (generate, StateGraph, observability)
- Comprehensive test suite (42 E2E/integration tests + 33 unit tests = 75 total)
- Multiple operation modes (generate, explain, refactor, test, docs, chat)

**See Also:** `docs/CODEX_DASHFLOW_ARCHIVE_NOTICE.md` for migration from standalone repo.

---

### Common (Shared Utilities)

**Location:** `examples/apps/common/`

**Purpose:** Shared utilities for example applications and testing

**Contains:**
- `create_llm()`: Unified LLM factory for examples
- `LLMRequirements`: Configuration types
- `QualityJudge`: LLM-as-judge evaluation utilities

**Usage:**
```rust
use examples_common::{create_llm, QualityJudge};

let llm = create_llm()?;
let judge = QualityJudge::new(llm);
let score = judge.evaluate(&response, &expected).await?;
```

---

## Historical Note

Previous versions of DashFlow included additional example applications:
- `document_search` - Basic RAG demonstration
- `advanced_rag` - Corrective RAG patterns
- `code_assistant` - Code generation
- `research_team` - Multi-agent patterns

These have been consolidated into the `librarian` paragon application, which demonstrates all the same patterns in a single, well-maintained example.

For historical reference, see git history prior to v1.11.3.

---

## Application Status

| Application | Status | Description |
|-------------|--------|-------------|
| librarian | Active | Production-ready RAG paragon |
| codex-dashflow | Active | AI code assistant paragon |
| common | Active | Shared test utilities |

---

## Quick Start

```bash
# Clone and build
git clone https://github.com/dropbox/dTOOL/dashflow
cd dashflow

# Set up API key
export OPENAI_API_KEY="sk-..."

# Run the librarian example
cargo run -p librarian -- query "What is DashFlow?"
```

---

## See Also

- [QUICKSTART.md](../QUICKSTART.md) - Getting started guide
- [COOKBOOK.md](COOKBOOK.md) - Common patterns and recipes
- [ARCHITECTURE.md](ARCHITECTURE.md) - System design
