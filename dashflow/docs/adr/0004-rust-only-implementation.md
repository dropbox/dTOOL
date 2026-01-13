# ADR-0004: Rust-Only Implementation

**Status:** accepted
**Date:** 2025-12-22
**Author:** DashFlow Architecture Team
**Last Updated:** 2025-12-22 (Worker #1414 - Initial ADR creation)

## Context

DashFlow started as a port of LangChain (Python) to Rust. The question arose: should we maintain Python interop, use pyo3 for bindings, or go pure Rust?

Key considerations:
- LangChain's Python ecosystem is mature but has performance limitations
- Python's GIL limits concurrent execution
- Deployment complexity with mixed Python/Rust binaries
- Target users are AI agent developers who may prefer either language

## Decision

**DashFlow is implemented entirely in Rust with NO Python runtime dependency in production.**

Specifically:
- No Python subprocess calls in production code
- No pyo3 bindings in production code
- No Python imports or runtime dependency
- The final `cargo` binary runs without Python installed

### Allowed Exceptions

Python IS allowed for:
- Development helper scripts (`scripts/*.py`)
- Model export tools (ONNX, tokenizer generation)
- Validation and testing scripts
- Documentation generation

### Correct Patterns

```rust
// RIGHT: Pure Rust implementation
use tiktoken_rs::CoreBPE;  // Rust tokenizer
use rust_bert::pipelines;   // Rust ML

// RIGHT: C++ dependencies when needed (allowed)
use candle_core::Tensor;    // C++ backend OK
```

### Anti-Patterns

```rust
// WRONG: Python subprocess
std::process::Command::new("python")
    .arg("tokenize.py")
    .spawn()?;

// WRONG: pyo3 in production
#[pymodule]
fn my_module(m: &Bound<'_, PyModule>) -> PyResult<()> { ... }
```

## Consequences

### Positive
- Single binary deployment (no Python version conflicts)
- Predictable performance (no GIL, no startup overhead)
- Memory safety at compile time
- Easier to reason about resource usage

### Negative
- Can't directly use Python ML ecosystem (transformers, etc.)
- Must port or find Rust equivalents for Python libraries
- Some features may lag behind Python LangChain

### Neutral
- C++ dependencies are acceptable where needed
- Development tooling can still use Python

## Alternatives Considered

### Alternative 1: pyo3 Bindings
- Wrap Python LangChain for compatibility
- Rejected: Defeats performance goals, deployment complexity

### Alternative 2: Python for ML, Rust for Core
- Split: Rust core + Python ML inference
- Rejected: Complexity at boundary, deployment issues

### Alternative 3: WASM for Portability
- Compile to WebAssembly for universal runtime
- Rejected: Performance constraints, incomplete ecosystem

## Related Documents

- `CLAUDE.md` section "PYTHON ABSOLUTELY FORBIDDEN IN IMPLEMENTATION"
- Root `Cargo.toml` workspace configuration
