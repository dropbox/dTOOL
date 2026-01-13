# DashFlow Introspection System

**Version:** 1.11.3
**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

DashFlow provides a comprehensive introspection system that enables self-aware AI agents, platform usage linting, and capability discovery across the entire workspace.

## Overview

The introspection system operates at multiple levels:

| Level | Scope | Commands | Use Case |
|-------|-------|----------|----------|
| **Platform** | DashFlow framework | `introspect list/search/show` | "What modules exist?" |
| **Type** | Individual types | `introspect types/find-capability` | "What retrievers are available?" |
| **Capability** | Semantic tags | `introspect capabilities/find-capability` | "What provides BM25 search?" |
| **Runtime** | Execution traces | `introspect ask/health` | "Why did X happen?" |

## Quick Start

```bash
# List all modules in DashFlow
dashflow introspect list

# Search for modules by keyword
dashflow introspect search distillation

# Search types (structs, traits, functions)
dashflow introspect search --types retriever

# Find types by capability tag
dashflow introspect find-capability bm25

# List all capability tags
dashflow introspect capabilities --with-counts

# Check platform health
dashflow introspect health
```

## Module Discovery

### Listing Modules

```bash
# List all modules
dashflow introspect list

# Filter by category
dashflow introspect list --category optimize

# JSON output for tooling
dashflow introspect list --format json
```

### Searching Modules

```bash
# Substring search
dashflow introspect search distill

# Semantic (TF-IDF) search
dashflow introspect search --semantic "keyword search BM25"

# Include individual types in results
dashflow introspect search --types retriever
```

### Module Details

```bash
# Show details for a module
dashflow introspect show distillation

# JSON output
dashflow introspect show distillation --format json
```

## Type Discovery (Phase 952-964)

The type index discovers all public types (structs, traits, enums, functions) across the workspace.

### Listing Types

```bash
# List types in a specific crate
dashflow introspect types dashflow-opensearch

# Filter by kind
dashflow introspect types --kind struct

# Search with filter
dashflow introspect types --filter retriever
```

### Type Index Management

```bash
# Check index status
dashflow introspect index

# Rebuild the index
dashflow introspect index --rebuild
```

The index is stored at `.dashflow/index/types.json` and includes:
- Type name, path, crate
- Kind (struct, trait, enum, function)
- Doc comments
- Capability tags (inferred and explicit)
- Semantic embeddings for similarity search

## Capability Discovery (Phase 965-966)

Types are automatically tagged with capabilities based on:

1. **Type names**: `BM25Retriever` → `bm25`, `retriever`
2. **Doc comments**: "provides keyword search" → `bm25`, `search`
3. **Method signatures**: `get_relevant_documents()` → `retriever`
4. **Explicit attributes**: `#[dashflow::capability("search", "bm25")]`

### Finding by Capability

```bash
# Find all types with a capability
dashflow introspect find-capability retriever

# Show capability tags for each result
dashflow introspect find-capability bm25 --show-tags

# Limit results
dashflow introspect find-capability search -n 10
```

### Listing All Capabilities

```bash
# List all available capability tags
dashflow introspect capabilities

# With type counts
dashflow introspect capabilities --with-counts
```

### Available Capabilities

Common capability tags discovered in DashFlow:

| Category | Tags |
|----------|------|
| **Retrieval** | `retriever`, `bm25`, `search`, `vector_store`, `hybrid_search`, `similarity_search` |
| **Embeddings** | `embeddings`, `dense_retrieval`, `sparse_retrieval` |
| **Language Models** | `llm`, `chat`, `completion`, `text_generation` |
| **Processing** | `chunking`, `splitting`, `document_loader`, `parsing`, `transformation` |
| **Observability** | `cost_tracking`, `metrics`, `tracing`, `telemetry` |
| **Optimization** | `optimization`, `distillation`, `finetuning`, `prompt_optimization` |
| **Execution** | `chain`, `graph`, `runnable`, `streaming`, `batching` |

## Semantic Search (Phase 959-961)

The introspection system includes TF-IDF based semantic search for finding similar types.

```bash
# Semantic search
dashflow introspect search --semantic "keyword search with BM25"

# Control result count
dashflow introspect search --semantic "vector similarity" --limit 10
```

The semantic index:
- Tokenizes type names, descriptions, and doc comments
- Computes TF-IDF vectors for each type
- Supports fuzzy matching and synonym detection
- Stored in `.dashflow/index/types.json` alongside type data

## Explicit Capability Annotations

For types where automatic inference isn't sufficient, use explicit annotations:

```rust
#[dashflow::capability("bm25", "keyword_search", "retriever")]
pub struct MyKeywordRetriever {
    // ...
}
```

Or using the shorter form:

```rust
#[capability("embeddings", "vector_store")]
pub struct MyEmbeddingStore {
    // ...
}
```

Explicit annotations are merged with inferred capabilities.

## JSON Schema

The type index follows a formal JSON schema for tooling integration:

**Location:** `schemas/introspection-index.schema.json`

```bash
# Validate your index against the schema
jq --slurpfile schema schemas/introspection-index.schema.json \
   'if $schema[0] then "Valid" else "Invalid" end' \
   .dashflow/index/types.json
```

See the schema for full structure documentation.

## Platform Usage Linting

The introspection system powers the platform linter which detects when apps reimplement platform features:

```bash
# Lint an app directory
dashflow lint examples/apps/librarian

# With detailed explanations
dashflow lint --explain examples/apps/librarian
```

The linter:
1. Scans source files for reimplementation patterns
2. Queries introspection for matching platform types
3. Suggests existing platform features
4. Collects feedback on why platform wasn't used

See `dashflow lint --help` for all options.

## Pre-commit Hook Integration

The type index is automatically regenerated when Rust source files change:

```bash
# .git/hooks/pre-commit (installed by dashflow)
if git diff --cached --name-only | grep -q '\.rs$'; then
    dashflow introspect index --rebuild
    git add .dashflow/index/
fi
```

## API Reference

### Rust API

```rust
use dashflow_module_discovery::{
    discover_all_types,
    find_types_by_capability,
    get_all_capability_tags,
    TypeInfo,
};

// Discover all types in workspace
let types = discover_all_types("/path/to/workspace");

// Find types by capability
let retrievers = find_types_by_capability("/path/to/workspace", "retriever");

// Get all capability tags
let tags = get_all_capability_tags("/path/to/workspace");
```

### TypeIndex API

```rust
use dashflow::lint::{TypeIndex, TypeIndexCache};

// Build index from workspace
let index = TypeIndex::build(PathBuf::from("."));

// Or load from cache
let cache_path = PathBuf::from(".dashflow/index/types.json");
if let Some((index, cache)) = TypeIndex::load(&cache_path) {
    // Use cached index
    let results = index.search_semantic("keyword search", 20);
}
```

## Troubleshooting

### Index is Stale

```bash
# Check index status
dashflow introspect index

# Rebuild if stale
dashflow introspect index --rebuild
```

### Type Not Found

1. Check if the type is public (`pub`)
2. Check if it's in a scanned crate (see `discover_all_workspace_crates()`)
3. Rebuild the index

### Capability Not Detected

1. Add explicit annotation: `#[dashflow::capability("tag")]`
2. Improve doc comments with relevant keywords
3. Ensure method names follow conventions (e.g., `get_relevant_documents`)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    CLI Commands                              │
│  introspect list/search/types/find-capability/capabilities  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    dashflow-cli                              │
│              commands/introspect.rs                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 dashflow::lint::TypeIndex                    │
│           In-memory index with semantic search              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              dashflow-module-discovery                       │
│  syn parsing, capability inference, type discovery          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              .dashflow/index/types.json                      │
│                 Cached index on disk                         │
└─────────────────────────────────────────────────────────────┘
```

## Related Documentation

- [CLAUDE.md](../CLAUDE.md) - Project overview and AI worker instructions
- [DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) - Architectural constraints
- [ROADMAP_CURRENT.md](../ROADMAP_CURRENT.md) - Part 35 introspection roadmap
