# DashFlow Documentation

Documentation for converting DashFlow Python to Rust (DashFlow).

**For AI Worker Execution**

## Overview

This directory contains comprehensive planning and design documentation for converting the DashFlow Python framework (~301K lines of code) to Rust. All plans are designed for autonomous AI worker execution.

## Documents

### 📋 Implementation Status
**Status:** ✅ PRODUCTION READY - All quality targets exceeded (v1.11.3)

**Note:** Original RUST_CONVERSION_PLAN.md was consolidated at N=540. Phase documentation archived after completion. Implementation is production-ready at v1.11.3 with 100% validation success rate.

### 🏗️ [Architecture Design](./ARCHITECTURE.md)
**Key Topics:**
- Core traits (Runnable, ChatModel, Embeddings, VectorStore, Tool)
- Message system design (enums and types)
- Runnable composition (pipe operator)
- Error handling strategy
- Serialization with Serde
- Callback system
- Async patterns

### 📦 [Dependency Mapping](./DEPENDENCY_MAPPING.md)
**Contents:**
- Complete Python → Rust library mapping
- 30+ dependency conversions
- HTTP client choices
- AI/ML SDK status
- Testing frameworks
- Recommended workspace dependencies

### 🔧 [Troubleshooting Guide](./TROUBLESHOOTING.md)
**Diagnose and solve common issues:**
- Installation problems (dependencies, compilation)
- API key configuration
- Runtime errors (timeouts, rate limits, memory)
- Vector store issues (connection, search quality)
- DashFlow workflow debugging
- Integration problems (Kafka, Redis, AWS)
- Performance optimization
- Debugging tips and tools

### 🔄 [Rust Version Migration](./MIGRATION_v1.0_to_v1.6.md)
**For DashFlow users upgrading between versions:**
- Breaking changes (v1.0 → v1.6)
- Step-by-step upgrade guide
- API compatibility shims
- Deprecation policy
- Stability promise for v1.6+
- Refactoring patterns for loose coupling

### 📊 [Embedding Provider Comparison](./EMBEDDING_PROVIDERS_COMPARISON.md)
**For Choosing the Right Embedding Provider:**
- Quick comparison of 6 most common providers (12 total supported)
- Detailed provider analysis (OpenAI, Ollama, HuggingFace, Mistral, Fireworks, Nomic)
- Performance considerations (latency, throughput)
- Selection guide based on use case
- API configuration and testing
- Migration patterns between providers

## Quick Start for AI Workers

### Getting Started
1. Review [Architecture Design](./ARCHITECTURE.md) for technical approach
2. Use [Dependency Mapping](./DEPENDENCY_MAPPING.md) for library choices
3. See [AI Parts Catalog](./AI_PARTS_CATALOG.md) for component glossary
4. Check [Golden Path Guide](./GOLDEN_PATH.md) for recommended API patterns

### Before Starting
- Understand Rust: async/await, traits, lifetimes, error handling
- Understand LLM APIs: Message formats, tool calling, streaming
- Review baseline Python code at ~/dashflow before porting

## Key Decisions

### Technology Choices
- **Async Runtime:** tokio 1.38+
- **Serialization:** serde + validator + schemars
- **HTTP Client:** reqwest 0.12+
- **Template Engine:** tera or minijinja
- **Testing:** cargo test + insta (snapshots) + criterion (benchmarks)
- **Error Handling:** thiserror

### Architecture Patterns
- **Composition over Inheritance:** Traits and composition
- **Type Safety:** Leverage Rust's type system
- **Async by Default:** All I/O operations are async
- **Explicit Errors:** Result types everywhere
- **Zero-Cost Abstractions:** No runtime overhead

### Scope Decisions
- **In Scope:** dashflow, dashflow::core, text-splitters, 3+ integrations
- **Out of Scope (Initial):** legacy Python patterns, CLI tools
- **MVP Focus:** 6-month subset for time-sensitive projects

## Project Status

**Current Version:** v1.11.3 (✅ PRODUCTION READY)
**Current Phase:** Production Maintenance & Optional Enhancements

**Status:** All quality targets exceeded. 100% validation success, 0.904 quality score.

**Next AI Worker Steps:**
1. Maintain code quality and documentation
2. Optional enhancements per ROADMAP_CURRENT.md
3. Monitor for dependency updates
4. Continue test suite maintenance

## Execution Principles

When implementing, follow these principles:
1. Read the architecture docs first
2. Follow the phased approach
3. Write tests alongside code (not after)
4. Port from baseline Python source (~/dashflow)
5. Profile before optimizing
6. Use AI commits as time unit (1 commit ≈ 12 minutes)

## Additional Resources

### External Links
- [DashFlow Python Repository](https://github.com/dashflow-ai/dashflow)
- [LangChain Python Documentation](https://python.langchain.com/docs/)
- [LangChain Python API Reference](https://api.python.langchain.com/)

### Crate Documentation
- [tokio](https://docs.rs/tokio/)
- [serde](https://docs.rs/serde/)
- [reqwest](https://docs.rs/reqwest/)
- [async-trait](https://docs.rs/async-trait/)

---

**Last Updated:** 2026-01-04 (Worker #2427 - Fix stale embedding provider count)
**Status:** v1.11.3 Production-ready - All quality targets exceeded
**For:** AI Worker Execution

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
