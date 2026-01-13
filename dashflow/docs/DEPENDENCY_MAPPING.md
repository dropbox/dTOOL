# Python → Rust Dependency Mapping

**Last Updated:** 2026-01-03 (Worker #2414 - Update stale SDK status; all integrations now complete)

Complete mapping of Python dependencies to Rust equivalents for DashFlow conversion.

## Core Dependencies

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **pydantic** | 2.x | `serde` | 1.0 | Serialization |
| | | `validator` | 0.19 | Field validation |
| | | `derive_builder` | 0.20 | Builder patterns |
| | | `schemars` | 0.8 | JSON Schema generation |
| **langsmith** | >=0.3.45 | `dashflow-langsmith` | 1.11 | ✅ DashFlow native integration |
| **tenacity** | >=8.1.0 | `tokio-retry` | 0.3 | Retry logic |
| | | `backoff` | 0.4 | Exponential backoff |
| **jsonpatch** | >=1.33.0 | `json-patch` | 2.0 | JSON patching |
| **PyYAML** | >=5.3.0 | `serde_yaml` | 0.9 | YAML parsing |
| **typing-extensions** | >=4.7.0 | Native | - | Rust type system |
| **packaging** | >=23.2.0 | `semver` | 1.0 | Version comparison |

## Async Runtime

| Python | Rust | Notes |
|--------|------|-------|
| **asyncio** | `tokio` 1.38+ | Full-featured async runtime |
| **concurrent.futures** | `tokio::task` | Async tasks |
| | `rayon` 1.10+ | Data parallelism |
| **threading** | `std::thread` | Native threads |
| | `crossbeam` | Advanced concurrency |

## HTTP & Networking

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **httpx** | >=0.25.2 | `reqwest` | 0.12+ | HTTP client (async/sync) |
| **httpx-sse** | >=0.3.1 | `eventsource-stream` | 0.2+ | Server-sent events |
| | | `async-stream` | 0.3+ | Custom streams |
| **requests** | >=2.0.0 | `reqwest` (blocking) | 0.12+ | Sync HTTP |
| **aiohttp** | >=3.9.1 | `reqwest` | 0.12+ | Async HTTP |
| **websockets** | - | `tokio-tungstenite` | 0.24+ | WebSocket support |

## AI/ML Integration SDKs

| Python SDK | Version | Rust Crate | Version | Status |
|------------|---------|------------|---------|--------|
| **anthropic** | >=0.69.0 | `dashflow-anthropic` | 1.11 | ✅ DashFlow native integration |
| **openai** | >=1.109.1 | `dashflow-openai` | 1.11 | ✅ DashFlow native integration |
| **ollama** | >=0.6.0 | `dashflow-ollama` | 1.11 | ✅ DashFlow native integration |
| **groq** | >=0.30.0 | `dashflow-groq` | 1.11 | ✅ DashFlow native integration |
| **mistralai** | - | `dashflow-mistral` | 1.11 | ✅ DashFlow native integration |
| **fireworks-ai** | >=0.13.0 | `dashflow-fireworks` | 1.11 | ✅ DashFlow native integration |
| **huggingface-hub** | >=0.33.4 | `dashflow-huggingface` | 1.11 | ✅ DashFlow native integration |
| **chromadb** | >=1.0.20 | `dashflow-chroma` | 1.11 | ✅ DashFlow native integration |
| **qdrant-client** | >=1.15.1 | `dashflow-qdrant` | 1.11 | ✅ DashFlow native integration |
| **exa-py** | >=1.0.8 | `dashflow-exa` | 1.11 | ✅ DashFlow native integration |
| **nomic** | >=3.5.3 | `dashflow-nomic` | 1.11 | ✅ DashFlow native integration |

## Testing Dependencies

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **pytest** | >=8.0.0 | `cargo test` | - | Built-in test framework |
| **pytest-asyncio** | >=0.21.1 | `tokio::test` | - | Async test macro |
| **pytest-mock** | >=3.10.0 | `mockall` | 0.13+ | Mocking framework |
| **syrupy** | >=4.0.2 | `insta` | 1.39+ | Snapshot testing |
| **pytest-benchmark** | - | `criterion` | 0.5+ | Benchmarking |
| **pytest-codspeed** | - | `criterion` | 0.5+ | Performance testing |
| **vcrpy** | >=7.0.0 | `wiremock` | 0.6+ | HTTP mocking |
| **pytest-socket** | >=0.7.0 | Test isolation | - | Manual test isolation |
| **pytest-xdist** | >=3.6.1 | `cargo nextest` | - | Parallel test execution |

## Text Processing

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **tiktoken** | >=0.7.0 | `tiktoken-rs` | 0.6+ | ✅ OpenAI tokenizer |
| **beautifulsoup4** | - | `scraper` | 0.20+ | HTML parsing |
| | | `select` | 0.6+ | CSS selectors |
| **lxml** | - | `roxmltree` | 0.20+ | XML parsing |
| **markdown** | - | `pulldown-cmark` | 0.12+ | Markdown parsing |
| **jinja2** | - | `tera` | 1.19+ | Template engine |
| | | `minijinja` | 2.0+ | Lightweight Jinja2 |

## Data Processing

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **numpy** | >=1.26.4 | `ndarray` | 0.16+ | N-dimensional arrays |
| **pandas** | >=2.0.0 | `polars` | 0.44+ | DataFrames |
| **transformers** | >=4.39.0 | `rust-bert` | 0.26+ | Transformer models |
| **sentence-transformers** | >=2.6.0 | `rust-bert` | 0.26+ | Sentence embeddings |
| **tokenizers** | >=0.15.1 | `tokenizers` | 0.20+ | HuggingFace tokenizers |
| **pillow** | >=10.3.0 | `image` | 0.25+ | Image processing |

## Database & Storage

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **SQLAlchemy** | >=1.4.0 | `sqlx` | 0.8+ | Async SQL toolkit |
| | | `diesel` | 2.2+ | ORM alternative |
| **redis** | - | `redis` | 0.27+ | Redis client |
| **cassio** | >=0.1.0 | Custom | - | Cassandra integration |

## Parsing & Code Analysis

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **lark** | >=1.1.5 | `pest` | 2.7+ | Parser generator |
| **tree-sitter** | - | `tree-sitter` | 0.24+ | Code parsing |

## Utilities

| Python Package | Version | Rust Crate | Version | Notes |
|----------------|---------|------------|---------|-------|
| **toml** | >=0.10.2 | `toml` | 0.8+ | TOML parsing |
| **python-dotenv** | >=1.0.0 | `dotenvy` | 0.15+ | .env file loading |
| **freezegun** | >=1.2.2 | `mock_instant` | 0.5+ | Time mocking |
| **responses** | >=0.25.0 | `wiremock` | 0.6+ | HTTP mocking |
| **uuid** | - | `uuid` | 1.11+ | UUID generation |
| **chrono** | - | `chrono` | 0.4+ | Date/time handling |

## Development Tools

| Python Tool | Rust Tool | Notes |
|-------------|-----------|-------|
| **ruff** | `clippy` | Linter |
| | `rustfmt` | Formatter |
| **mypy** | `cargo check` | Type checking (built-in) |
| **black** | `rustfmt` | Code formatting |
| **isort** | `rustfmt` | Import sorting |
| **poetry** / **uv** | `cargo` | Package manager |
| **hatchling** | `cargo` | Build system |

## Key Observations

### ✅ Strong Ecosystem Support
- HTTP clients (reqwest)
- Async runtime (tokio)
- Serialization (serde)
- Testing (criterion, insta, mockall, nextest)
- AI SDKs (DashFlow native: dashflow-openai, dashflow-anthropic, dashflow-ollama, etc.)

### ✅ All Major Integrations Complete
- **Anthropic SDK** - `dashflow-anthropic` - Complete native integration
- **LangSmith client** - `dashflow-langsmith` - Full tracing/observability
- **All LLM providers** - Native integrations for Groq, Mistral, Fireworks, Gemini, DeepSeek, xAI, etc.

### 🔧 Different Approaches
- **Pydantic → Serde**: More manual validation, less automatic
- **Jinja2 → Tera/MiniJinja**: Similar but not identical syntax
- **Pytest → Cargo test**: Different test organization patterns
- **Dynamic typing → Static types**: Requires more upfront design

## Recommended Workspace Dependencies

```toml
[workspace.dependencies]
# Core
tokio = { version = "1.38", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
futures = "0.3"
async-stream = "0.3"
thiserror = "2.0"
anyhow = "1.0"

# HTTP
reqwest = { version = "0.12", features = ["json", "stream"] }
eventsource-stream = "0.2"

# Validation & Schema
validator = "0.19"
schemars = "0.8"

# Utilities
uuid = { version = "1.11", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
dashmap = "6.0"

# Template engines
tera = "1.19"
minijinja = "2.0"

# Testing
mockall = "0.13"
wiremock = "0.6"
insta = "1.39"
criterion = "0.5"

# Integration SDKs
async-openai = "0.27"
ollama-rs = "0.2"
qdrant-client = "1.11"
```

## Migration Status: ✅ COMPLETE

All Python-to-Rust migration is complete. DashFlow provides native Rust implementations for:

1. **Core Infrastructure** - ✅ Complete
   - tokio, serde, reqwest, async-trait
   - Full testing setup (cargo test, nextest, insta, criterion)

2. **Validation & Schemas** - ✅ Complete
   - validator, schemars (tool schemas)
   - tera/minijinja (prompt templates)
   - mockall, wiremock (testing)

3. **LLM Integrations** - ✅ Complete (100+ crates)
   - All major providers: OpenAI, Anthropic, Ollama, Groq, Mistral, Fireworks, etc.
   - Native implementations, not wrappers

4. **Vector Stores & Tools** - ✅ Complete
   - Qdrant, Chroma, Pinecone, Weaviate, pgvector, etc.
   - rust-bert available for local embeddings
   - 50+ tool integrations (GitHub, Jira, Slack, etc.)

---
