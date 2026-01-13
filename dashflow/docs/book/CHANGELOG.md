# Changelog

All notable changes to DashFlow will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.0] - 2025-11-08

### Added

**Integration & Tooling Release - Expanded Provider Ecosystem and Documentation**

This release expands the LLM provider ecosystem with Together AI and Replicate support, adds the Weaviate vector store, and delivers a comprehensive API documentation website. The codebase now includes 100+ document loaders, extensive tool support, and advanced memory systems.

#### Phase 4: LLM Provider Expansion

- **Together AI Provider:** Access to 100+ open-source models
  - New crate: \`dashflow-together\` (~850 lines)
  - OpenAI-compatible API integration (base URL: api.together.xyz/v1)
  - Authentication: TOGETHER_API_KEY environment variable
  - Default model: meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo
  - Streaming support with async-openai client
  - Function/tool calling support
  - Builder pattern: \`with_model()\`, \`with_temperature()\`, \`with_max_tokens()\`, etc.
  - Retry policies and rate limiting
  - 10 integration tests (requires API token)
  - Example: \`crates/dashflow-together/examples/basic.rs\`
  - Implementation: N=1014-1016 (3 commits)

- **Replicate Provider:** Run models on Replicate platform
  - New crate: \`dashflow-replicate\` (~845 lines)
  - OpenAI-compatible proxy (base URL: openai-proxy.replicate.com/v1)
  - Authentication: REPLICATE_API_TOKEN environment variable
  - Model format: owner/model-name (e.g., meta/meta-llama-3-8b-instruct)
  - Streaming support and tool calling
  - Configurable retry and rate limiting
  - 10 integration tests
  - Example: \`crates/dashflow-replicate/examples/basic.rs\`
  - Implementation: N=1017 (1 commit)

- **Cohere Provider:** Already implemented
  - Crate: \`dashflow-cohere\`
  - ChatCohere, CohereEmbeddings, CohereRerank
  - Command R and Command R+ models
  - Embed-english-v3.0 and embed-multilingual-v3.0 embeddings
  - Document reranking API integration

#### Phase 6: Documentation & Examples

- **API Documentation Website:** Comprehensive mdBook site
  - New: \`docs/book/\` directory
  - 54 pages of documentation
  - Sections: Getting Started, Core Concepts, Architecture, Agents, Chains, Tools, Advanced Topics, API Reference, Examples, Migration Guides, Contributing, Resources
  - mdBook v0.4.52 configuration
  - Runnable code examples in playground
  - Search functionality with boost configuration
  - Rust theme with navy dark mode
  - Build: \`cd docs/book && mdbook build\`
  - Serve: \`cd docs/book && mdbook serve\`

#### Phases 1-3, 5: Pre-existing Features (Verified Complete)

- **Phase 1 - Vector Stores:**
  - Weaviate fully functional (~490 lines, 20+ tests)
  - Milvus and Faiss implemented but deferred (SDK/trait issues)

- **Phase 2 - Document Loaders:**
  - 100+ loaders implemented (PDF, CSV, JSON, HTML, Markdown, Directory, and many more)

- **Phase 3 - Tools:**
  - SQL Database, HTTP, File System, Shell, Calculator tools
  - Additional: JSON, Playwright, GitHub, Slack, Jira, GraphQL, OpenAPI, WASM

- **Phase 5 - Memory Systems:**
  - All memory types: Buffer, BufferWindow, Summary, Entity, KG, Token Buffer, VectorStore
  - Chat history backends: File, Redis, MongoDB, Postgres, DynamoDB, Upstash, Cassandra

### Changed

- Workspace version: 1.4.0 → 1.5.0
- Total crates: 87 → 89 (+2 new LLM provider crates)
- LLM providers: 13 → 15 (added Together AI, Replicate)

### Quality

- Workspace compiles cleanly
- Zero compiler warnings
- Zero clippy warnings
- All tests passing

### Statistics

- **Commits:** 8 (N=1010 to N=1017)
  - N=1010: v1.5.0 planning
  - N=1011: Status assessment
  - N=1012: Documentation website
  - N=1013: Documentation build verification
  - N=1014-1016: Together AI provider
  - N=1017: Replicate provider
- **Efficiency:** Plan estimated 60-80 commits, actual: 8 (most features pre-existed)

### Known Issues

- **Milvus:** Excluded (SDK API incompatibility)
- **Faiss:** Excluded (Send trait requirements)
- Both deferred to v1.6.0

### Migration Guide

No breaking changes. All additions are backward compatible.

#### Using Together AI

\`\`\`rust
use dashflow_together::ChatTogether;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;

let llm = ChatTogether::new()
    .with_model("meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo")
    .with_temperature(0.7);

let messages = vec![Message::human("Hello!")];
let response = llm.generate(&messages, None, None, None, None).await?;
\`\`\`

#### Using Replicate

\`\`\`rust
use dashflow_replicate::ChatReplicate;

let llm = ChatReplicate::new()
    .with_model("meta/meta-llama-3-70b-instruct")
    .with_temperature(0.7);

let response = llm.generate(&messages, None, None, None, None).await?;
\`\`\`
