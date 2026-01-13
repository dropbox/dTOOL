# Installation

**Last Updated:** 2026-01-04 (Worker #2494 - Remove non-existent dashflow-document-loaders crate)

This guide will help you set up DashFlow in your project.

## Prerequisites

- **Rust**: 1.80 or later (MSRV - Minimum Supported Rust Version)
- **Cargo**: Comes with Rust installation
- **Optional**: Docker (for testing vector stores locally)

### Installing Rust

If you don't have Rust installed, use [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify installation:

```bash
rustc --version
cargo --version
```

## Adding DashFlow to Your Project

### Option 1: Using Cargo

Add dependencies to your `Cargo.toml`:

```toml
[dependencies]
# Core functionality
dashflow = "1.11"

# LLM providers (choose what you need)
dashflow-openai = "1.11"
dashflow-anthropic = "1.11"
dashflow-ollama = "1.11"
dashflow-cohere = "1.11"

# Vector stores (optional)
dashflow-qdrant = "1.11"
dashflow-weaviate = "1.11"
dashflow-chroma = "1.11"

# Additional functionality
dashflow-text-splitters = "1.11"
# Document loaders are built into the core dashflow crate

# Async runtime
tokio = { version = "1", features = ["full"] }
```

### Option 2: Adding Individual Crates

For minimal installations, add only what you need:

```bash
# Core functionality
cargo add dashflow

# Add specific providers
cargo add dashflow-openai
cargo add dashflow-anthropic

# Add async runtime
cargo add tokio --features full
```

## Feature Flags

Many crates support optional features:

### dashflow Features

```toml
dashflow = { version = "1.11", features = [
    "embeddings",      # Embedding models
    "vector-stores",   # Vector store traits
    "document-loaders", # Document loading
    "text-splitters",  # Text splitting
    "agents",          # Agent framework
    "tools",           # Tool execution
] }
```

### Document Loader Support

Document loaders are part of the core `dashflow` crate. For PDF support,
additional system dependencies are required (see System Dependencies below).

## System Dependencies

Some features require system libraries:

### PDF Support (Optional)

**Linux (Ubuntu/Debian)**:
```bash
sudo apt-get install libpoppler-dev
```

**macOS**:
```bash
brew install poppler
```

**Windows**:
Download poppler binaries from [poppler-windows](https://github.com/oschwartz10612/poppler-windows/releases/)

### Vector Stores (Optional)

If you're using vector stores, you may need to run them as services:

**Qdrant**:
```bash
docker run -p 6333:6333 qdrant/qdrant
```

**Weaviate**:
```bash
docker run -p 8080:8080 semitechnologies/weaviate:latest
```

**Chroma**:
```bash
docker run -p 8000:8000 chromadb/chroma
```

## Environment Variables

Set up API keys for LLM providers:

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Cohere
export COHERE_API_KEY="..."

# Ollama (if not running locally)
export OLLAMA_BASE_URL="http://localhost:11434"
```

### Using .env Files

Create a `.env` file in your project root:

```env
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
COHERE_API_KEY=...
```

Add `dotenv` to load environment variables:

```bash
cargo add dotenv
```

```rust
use dotenv::dotenv;

fn main() {
    dotenv().ok();
    // Your code here
}
```

## Verifying Installation

Create a simple test program to verify installation:

```bash
cargo new dashflow-test
cd dashflow-test
```

Edit `Cargo.toml`:

```toml
[dependencies]
dashflow = "1.11"
dashflow-openai = "1.11"
tokio = { version = "1", features = ["full"] }
```

Edit `src/main.rs`:

```rust
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();
    let response = llm.invoke("Hello!").await?;
    println!("Response: {}", response.content);
    Ok(())
}
```

Run:

```bash
export OPENAI_API_KEY="sk-..."
cargo run
```

Expected output:

```
Response: Hello! How can I assist you today?
```

## Troubleshooting

### Compilation Errors

**Problem**: "cannot find crate `dashflow_core`"

**Solution**: Run `cargo update` to fetch dependencies.

---

**Problem**: "linker `cc` not found"

**Solution**: Install build tools:
- Linux: `sudo apt-get install build-essential`
- macOS: `xcode-select --install`
- Windows: Install Visual Studio with C++ tools

---

**Problem**: PDF loading fails

**Solution**: Install poppler libraries (see System Dependencies above)

### Runtime Errors

**Problem**: "API key not found"

**Solution**: Set environment variables correctly:
```bash
export OPENAI_API_KEY="sk-..."
```

---

**Problem**: "Connection refused" for vector stores

**Solution**: Ensure vector store service is running:
```bash
docker ps  # Check running containers
```

## Next Steps

Now that you have DashFlow installed, continue to:

- **[Quick Start](./quick-start.md)**: Build your first application
- **[Core Concepts](./core-concepts.md)**: Learn the fundamentals
- **[Examples](../examples/rag.md)**: Explore practical examples

## Getting Help

- Review [Troubleshooting Guide](../../../TROUBLESHOOTING.md)
- Open an issue on [GitHub](https://github.com/dropbox/dTOOL/dashflow/issues)
