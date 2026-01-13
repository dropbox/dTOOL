# Superhuman Librarian

The ultimate RAG paragon with hybrid search, memory, and analysis over Project Gutenberg books.

## Requirements

- **Embedding API Key**: One of the following (auto-detected):
  - **OpenAI** (recommended): `export OPENAI_API_KEY="sk-..."`
  - **HuggingFace**: `export HF_TOKEN="hf_..."` (free at https://huggingface.co/settings/tokens)

  The librarian auto-detects which API key is available.

## Features

- **Hybrid Search**: Combines BM25 keyword and kNN semantic search via OpenSearch
- **Flexible Embeddings**: OpenAI `text-embedding-3-small` or HuggingFace `all-MiniLM-L6-v2`
- **Full Telemetry**: Prometheus metrics, Grafana dashboards, Jaeger tracing
- **Evaluation Framework**: Golden Q&A dataset with automated scoring
- **Memory**: Persistent conversation history, bookmarks, and reading progress
- **Fan Out**: Parallel multi-strategy search demonstrating DashFlow's parallel execution
- **Rich CLI**: Query, chat, bookmark, and memory management commands
- **Analysis**: Character extraction, relationship mapping, and theme identification

## Quick Start

```bash
# 1. Start infrastructure
cd examples/apps/librarian
docker-compose up -d

# 2. Wait for OpenSearch to be ready
curl http://localhost:9200/_cluster/health?wait_for_status=yellow

# 3. Index 10 classic books
cargo run -p librarian --bin indexer -- --preset quick

# 4. Search!
cargo run -p librarian -- query "Who is Elizabeth Bennet's love interest?"

# 5. View dashboards
open http://localhost:3000  # Grafana (admin/admin)
open http://localhost:5601  # OpenSearch Dashboards
open http://localhost:16686 # Jaeger

# 6. Run evaluation
cargo run -p librarian --bin librarian_eval
```

## Architecture

```
Gutenberg Books -> Chunker -> Embeddings -> OpenSearch (Hybrid Index)
                                                ↓
Query -> Intent -> Fan Out Search -> Rerank -> Generate -> Quality Check
                       ↓
                   ┌───┴───┐
             Semantic  Keyword  Filtered
                   └───┬───┘
                   Merge Results
```

## CLI Commands

### Indexer

```bash
# Index with preset (6 presets available)
cargo run -p librarian --bin indexer -- --preset quick         # 10 books (~5K chunks)
cargo run -p librarian --bin indexer -- --preset classics      # 50 books (~25K chunks)
cargo run -p librarian --bin indexer -- --preset full          # 135 books (~70K chunks)
cargo run -p librarian --bin indexer -- --preset massive       # 1000+ books (~500K chunks)
cargo run -p librarian --bin indexer -- --preset multilingual  # 80+ books in fr/de/es/it/pt/la
cargo run -p librarian --bin indexer -- --preset gutenberg     # ALL ~70K English books

# Index specific books by Gutenberg ID
cargo run -p librarian --bin indexer -- --book-ids 1342,2701,84

# List available presets with descriptions
cargo run -p librarian --bin indexer -- --list-presets

# Cross-language search setup
cargo run -p librarian --bin indexer -- --preset multilingual --detect-language --multilingual
```

### Search

```bash
# Hybrid search (default)
cargo run -p librarian -- query "Who is Elizabeth Bennet?"

# Filter by author
cargo run -p librarian -- query "monster" --author "Mary Shelley"

# Keyword-only search
cargo run -p librarian -- query "Captain Ahab" --mode keyword

# Semantic-only search
cargo run -p librarian -- query "obsession with revenge" --mode semantic

# Show index statistics
cargo run -p librarian -- stats
```

### Fan Out Search (Parallel Execution)

Demonstrates DashFlow's ability to execute multiple search strategies in parallel:

```bash
# Run semantic + keyword + hybrid searches in parallel
cargo run -p librarian -- fan-out "revenge and obsession"

# With timing breakdown (shows parallelism speedup)
cargo run -p librarian -- fan-out "love and marriage" --show-timing

# Specify strategies
cargo run -p librarian -- fan-out "white whale" --strategies semantic,keyword
```

The fan-out search:
1. Executes multiple search strategies **simultaneously**
2. Merges and deduplicates results
3. Reports timing metrics showing parallelism benefit
4. Visible in Jaeger traces as parallel spans

### Interactive Chat (with Memory)

```bash
# Start chat session (memory persisted to data/memory/)
cargo run -p librarian -- chat --user alice

# Example session:
# You: What happens in Pride and Prejudice?
# Librarian: [searches and responds based on indexed content]
# You: Tell me more about Darcy
# Librarian: [uses conversation context for better search]
# You: quit
```

### Bookmarks

```bash
# Add a bookmark
cargo run -p librarian -- bookmark add --book 1342 --chunk 50 --note "Darcy's letter"

# List bookmarks
cargo run -p librarian -- bookmark list --user alice

# Filter by book
cargo run -p librarian -- bookmark list --book 1342
```

### Memory Management

```bash
# View memory summary
cargo run -p librarian -- memory show --user alice

# Clear memory
cargo run -p librarian -- memory clear --user alice --confirm
```

### Character Analysis

```bash
# List characters in a book (by Gutenberg ID)
cargo run -p librarian -- characters 1342                    # Pride and Prejudice
cargo run -p librarian -- characters 2701                    # Moby Dick

# With relationship mapping
cargo run -p librarian -- characters 1342 --relationships

# JSON output
cargo run -p librarian -- characters 1342 --format json
```

### Theme Analysis

```bash
# Extract themes from a book
cargo run -p librarian -- themes 1342

# With evidence passages
cargo run -p librarian -- themes 1342 --with-evidence

# Limit number of themes
cargo run -p librarian -- themes 1342 -n 5 --with-evidence
```

### Full Book Analysis

```bash
# Complete analysis (characters + relationships + themes)
cargo run -p librarian -- analyze 1342

# JSON output for programmatic use
cargo run -p librarian -- analyze 1342 --format json
```

Supported book IDs (Gutenberg):
- `1342` - Pride and Prejudice (Jane Austen)
- `2701` - Moby Dick (Herman Melville)
- `84` - Frankenstein (Mary Shelley)
- `1524` - Hamlet (William Shakespeare)
- `98` - A Tale of Two Cities (Charles Dickens)

### Evaluation

```bash
# Run built-in evaluation questions
cargo run -p librarian --bin librarian_eval

# JSON output
cargo run -p librarian --bin librarian_eval -- --format json
```

### Introspection & Self-Improvement

The librarian can analyze its own performance and suggest improvements:

```bash
# Introspect last search - why did it succeed or fail?
cargo run -p librarian -- introspect
# Output: Detailed analysis of query, strategy, timing, and diagnosis

# Show execution trace for last search
cargo run -p librarian -- trace
cargo run -p librarian -- trace --last 5    # Show last 5 traces

# Show performance summary analysis
cargo run -p librarian -- trace --summary
# Output: Success rate, latency percentiles, strategy comparison, bottlenecks

# View improvement suggestions
cargo run -p librarian -- improve suggestions

# Generate new suggestions based on recent traces
cargo run -p librarian -- improve generate

# List all improvements (including applied)
cargo run -p librarian -- improve list

# Apply an improvement
cargo run -p librarian -- improve apply 1
```

The introspection system:
- Records execution traces for all searches
- Analyzes patterns to find failures and bottlenecks
- Generates actionable improvement suggestions
- Tracks improvement status (suggested, applied, verified)

Data is stored in `data/introspection/` as JSON files.

## Infrastructure Services

| Service | Port | Purpose |
|---------|------|---------|
| OpenSearch | 9200 | Vector store with hybrid search |
| OpenSearch Dashboards | 5601 | Index visualization |
| Prometheus | 9090 | Metrics collection |
| Grafana | 3000 | Dashboards (admin/admin) |
| Jaeger | 16686 | Distributed tracing |

## Metrics Exported

- `librarian_queries_total{type}` - Query count by type (hybrid/keyword/semantic)
- `librarian_search_latency_ms` - Search latency histogram
- `librarian_embedding_latency_ms` - Embedding generation latency
- `librarian_fan_out_searches_total` - Fan out search count
- `librarian_fan_out_latency_ms` - Fan out search latency
- `librarian_fan_out_speedup` - Parallelism speedup factor
- `librarian_analysis_total{type}` - Analysis count by type (characters/relationships/themes)
- `librarian_index_size_docs` - Number of indexed documents
- `librarian_quality_score` - Average evaluation quality score
- `librarian_errors_total{type}` - Error count by type

## Memory System

The librarian maintains persistent memory across sessions:

- **Conversation History**: Last 50 turns saved per user
- **Reading Progress**: Track where you left off in each book
- **Bookmarks**: Save interesting passages with notes
- **Search History**: Recent 100 searches for context

Memory is stored in `data/memory/` as JSON files, one per user.

## See Also

- [PLAN_BOOK_SEARCH_PARAGON.md](../../../PLAN_BOOK_SEARCH_PARAGON.md) - Full implementation plan
- [DashFlow](../../../crates/dashflow/README.md) - Core framework
- [dashflow-opensearch](../../../crates/dashflow-opensearch/README.md) - OpenSearch integration
- [dashflow-huggingface](../../../crates/dashflow-huggingface/README.md) - HuggingFace integration
- [docs/EXAMPLE_APPS.md](../../../docs/EXAMPLE_APPS.md) - All example applications
