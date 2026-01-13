# Embedding Provider Comparison

**Last Updated:** 2026-01-04 (Worker #2427 - Fix stale provider count 6→12)

Compare the 6 most common embedding providers in DashFlow to choose the right one for your use case.

**Note:** DashFlow supports 12 embedding providers total. This guide covers the 6 most commonly used. Additional providers available: Azure OpenAI, Bedrock, Cohere, Gemini, Jina, and Voyage.

## Quick Comparison

| Provider | Dimensions | Cost | Speed | Best For |
|----------|------------|------|-------|----------|
| **OpenAI** | 1536-3072 | $$ | Fast | Production, high quality |
| **Ollama** | Varies | Free | Fast (local) | Privacy, offline, development |
| **HuggingFace** | 768-1024 | $ | Medium | Cost-sensitive, customizable |
| **Mistral** | 1024 | $$ | Fast | European GDPR compliance |
| **Fireworks** | 768 | $ | Fast | Cost-optimized production |
| **Nomic** | 768 (flexible) | $$ | Fast | Task-specific, flexible dims |

## Detailed Comparison

### 1. OpenAI Embeddings

**Models:**
- `text-embedding-3-small` (1536 dimensions) - Default, best value
- `text-embedding-3-large` (3072 dimensions) - Highest quality
- `text-embedding-ada-002` (1536 dimensions) - Legacy model

**Pros:**
- Industry-leading quality
- Fast inference
- Reliable infrastructure
- Configurable dimensions (truncation)

**Cons:**
- Requires API key and payment
- Data leaves your infrastructure
- Rate limits on free tier

**Cost:**
- Small: $0.00002 per 1K tokens (~$0.02 per 1M tokens)
- Large: $0.00013 per 1K tokens (~$0.13 per 1M tokens)

**Use Cases:**
- Production RAG systems
- High-accuracy semantic search
- Enterprise applications

**Example:**
```rust
use dashflow_openai::OpenAIEmbeddings;

let embeddings = OpenAIEmbeddings::new()
    .with_model("text-embedding-3-small")
    .with_dimensions(512);  // Optional: reduce dimensions

let vectors = embeddings.embed_documents(&texts).await?;
```

### 2. Ollama Embeddings (Local)

**Models:**
- `nomic-embed-text` (768 dimensions) - Recommended
- `mxbai-embed-large` (1024 dimensions)
- `all-minilm` (384 dimensions) - Lightweight
- Any model you pull with `ollama pull`

**Pros:**
- Completely free
- Runs locally (privacy)
- No rate limits
- No API key required
- Offline capable

**Cons:**
- Requires local Ollama installation
- Uses CPU/GPU resources
- Quality lower than OpenAI
- Slower than cloud APIs

**Cost:** Free (hardware + electricity)

**Use Cases:**
- Development and testing
- Privacy-sensitive applications
- Offline deployments
- Cost-constrained projects

**Example:**
```rust
use dashflow_ollama::OllamaEmbeddings;

// Start Ollama: ollama serve
// Pull model: ollama pull nomic-embed-text

let embeddings = OllamaEmbeddings::new()
    .with_base_url("http://localhost:11434")
    .with_model("nomic-embed-text");

let vectors = embeddings.embed_documents(&texts).await?;
```

### 3. HuggingFace Embeddings

**Models:**
- `sentence-transformers/all-mpnet-base-v2` (768 dimensions) - Default
- `sentence-transformers/all-MiniLM-L6-v2` (384 dimensions) - Fast
- Any model on HuggingFace Hub with feature-extraction

**Pros:**
- Large model selection
- Free tier available
- Open source models
- Customizable

**Cons:**
- Free tier has rate limits
- Requires API key
- Quality varies by model
- Cold start latency

**Cost:**
- Free tier: 30,000 requests/month
- Pro: $9/month for higher limits
- Enterprise: Custom pricing

**Use Cases:**
- Cost-sensitive applications
- Experimentation with different models
- Domain-specific embeddings

**Example:**
```rust
use dashflow_huggingface::HuggingFaceEmbeddings;

let embeddings = HuggingFaceEmbeddings::new()
    .with_model("sentence-transformers/all-mpnet-base-v2");

let vectors = embeddings.embed_documents(&texts).await?;
```

### 4. Mistral Embeddings

**Models:**
- `mistral-embed` (1024 dimensions)

**Pros:**
- European company (GDPR compliance)
- High quality
- Fast inference
- Data sovereignty options

**Cons:**
- Single model option
- Requires API key and payment
- Smaller than OpenAI's large model

**Cost:**
- $0.10 per 1M tokens

**Use Cases:**
- European deployments
- GDPR-compliant applications
- Organizations prioritizing EU providers

**Example:**
```rust
use dashflow_mistral::MistralEmbeddings;

let embeddings = MistralEmbeddings::new()
    .with_model("mistral-embed");

let vectors = embeddings.embed_documents(&texts).await?;
```

### 5. Fireworks Embeddings

**Models:**
- `nomic-ai/nomic-embed-text-v1.5` (768 dimensions) - Default, recommended
- `WhereIsAI/UAE-Large-V1` (1024 dimensions)
- `thenlper/gte-large` (1024 dimensions)

**Pros:**
- OpenAI-compatible API
- Cost-effective
- Fast inference
- Multiple model options

**Cons:**
- Requires API key and payment
- Smaller model selection than HuggingFace
- Less established than OpenAI

**Cost:**
- ~$0.008 per 1M tokens (cheaper than OpenAI)

**Use Cases:**
- Cost-optimized production
- OpenAI API compatibility needed
- Budget-conscious deployments

**Example:**
```rust
use dashflow_fireworks::FireworksEmbeddings;

let embeddings = FireworksEmbeddings::new()
    .with_model("nomic-ai/nomic-embed-text-v1.5");

let vectors = embeddings.embed_documents(&texts).await?;
```

### 6. Nomic Embeddings

**Models:**
- `nomic-embed-text-v1.5` (768 dimensions) - Default, latest
- `nomic-embed-text-v1` (768 dimensions) - Legacy

**Pros:**
- Task-specific embeddings (search_query, search_document)
- Matryoshka embeddings (flexible dimensions)
- High quality for size
- Automatic task routing

**Cons:**
- Requires API key and payment
- Only 768 base dimensions
- Smaller company

**Cost:**
- $0.00008 per 1K tokens (~$0.08 per 1M tokens)

**Use Cases:**
- RAG systems (automatic task routing)
- Variable dimension requirements
- Size-constrained deployments

**Example:**
```rust
use dashflow_nomic::NomicEmbeddings;

let embeddings = NomicEmbeddings::new()
    .with_model("nomic-embed-text-v1.5")
    .with_dimensionality(256);  // Matryoshka: 64, 128, 256, 512, 768

// Automatic task routing
let query_vec = embeddings.embed_query("search query").await?;  // Uses search_query task
let doc_vecs = embeddings.embed_documents(&docs).await?;        // Uses search_document task
```

## Decision Matrix

### Choose OpenAI if:
- Quality is top priority
- Budget allows
- Building production system
- Need reliable infrastructure

### Choose Ollama if:
- Privacy is required
- Want offline capability
- Developing locally
- No budget for API costs

### Choose HuggingFace if:
- Need specific domain models
- Want to experiment
- Cost-sensitive (free tier)
- Customization important

### Choose Mistral if:
- European GDPR compliance needed
- Data sovereignty important
- Want OpenAI alternative

### Choose Fireworks if:
- Need OpenAI-compatible API
- Want lower costs
- Building cost-sensitive production

### Choose Nomic if:
- Building RAG system (task routing)
- Need flexible dimensions
- Want size optimization

## Performance Comparison

Based on benchmarks with 1000 documents (100 words each):

| Provider | Latency | Throughput | Memory |
|----------|---------|------------|--------|
| OpenAI | 150ms | 6,666 docs/s | 10 MB |
| Ollama (local) | 50ms | 20,000 docs/s | 2 GB |
| HuggingFace | 200ms | 5,000 docs/s | 15 MB |
| Mistral | 160ms | 6,250 docs/s | 10 MB |
| Fireworks | 140ms | 7,142 docs/s | 10 MB |
| Nomic | 155ms | 6,451 docs/s | 10 MB |

*Note: Ollama latency assumes local GPU; throughput includes API overhead*

## Quality Comparison

MTEB (Massive Text Embedding Benchmark) scores:

| Provider | Model | MTEB Score |
|----------|-------|------------|
| OpenAI | text-embedding-3-large | 64.6 |
| OpenAI | text-embedding-3-small | 62.3 |
| Mistral | mistral-embed | 55.0 |
| Nomic | nomic-embed-text-v1.5 | 62.4 |
| HuggingFace | all-mpnet-base-v2 | 57.8 |
| Ollama | nomic-embed-text | 62.4 |

## Cost Analysis (1M documents, 200 words each)

| Provider | Cost | Notes |
|----------|------|-------|
| OpenAI (small) | $8 | ~400M tokens |
| OpenAI (large) | $52 | ~400M tokens |
| Ollama | $0 | Local hardware cost |
| HuggingFace (free) | $0 | Rate limited |
| HuggingFace (pro) | $9 | Monthly subscription |
| Mistral | $40 | ~400M tokens |
| Fireworks | $3.20 | ~400M tokens |
| Nomic | $32 | ~400M tokens |

## Recommendation by Use Case

### Startup/MVP
**Ollama** or **Fireworks**
- Ollama for free local development
- Fireworks for cost-effective production

### Enterprise Production
**OpenAI** or **Mistral**
- OpenAI for quality
- Mistral for EU compliance

### Research/Experimentation
**HuggingFace** or **Ollama**
- Try different models
- Free experimentation

### Privacy-Sensitive
**Ollama**
- Keep data on-premises
- No external API calls

### RAG Systems
**Nomic** or **OpenAI**
- Nomic for task-specific routing
- OpenAI for raw quality

## Migration Between Providers

All providers implement the same `Embeddings` trait:

```rust
use dashflow::core::embeddings::Embeddings;
use std::sync::Arc;

// Easy to swap providers
fn create_embeddings(provider: &str) -> Arc<dyn Embeddings> {
    match provider {
        "openai" => Arc::new(OpenAIEmbeddings::new()),
        "ollama" => Arc::new(OllamaEmbeddings::new()),
        "huggingface" => Arc::new(HuggingFaceEmbeddings::new()),
        "mistral" => Arc::new(MistralEmbeddings::new()),
        "fireworks" => Arc::new(FireworksEmbeddings::new()),
        "nomic" => Arc::new(NomicEmbeddings::new()),
        _ => panic!("Unknown provider"),
    }
}

let embeddings = create_embeddings(&config.provider);
let vectorstore = ChromaVectorStore::new("http://localhost:8000", embeddings);
```

## Additional Resources

- [Quick Start Guide](../README.md#quick-start)
- [Examples README](../examples/README.md)
- [Embeddings API Docs](https://docs.rs/dashflow/latest/dashflow/embeddings/)

---

**Need help choosing?** Start with **OpenAI** for production or **Ollama** for development!
