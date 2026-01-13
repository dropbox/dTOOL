# Superhuman Librarian Walkthrough

This walkthrough demonstrates all features of the Superhuman Librarian, the ultimate RAG paragon for DashFlow.

## Prerequisites

- Docker and Docker Compose installed
- Rust toolchain (1.75+)
- ~2GB disk space for book data and OpenSearch index
- **Embedding API key** - one of the following:
  - **OpenAI API key** (recommended, auto-detected from OPENAI_API_KEY)
  - HuggingFace API token (get one free at https://huggingface.co/settings/tokens)

Set your API key:
```bash
# Option 1: OpenAI (recommended)
export OPENAI_API_KEY="sk-your_key_here"

# Option 2: HuggingFace
export HF_TOKEN="hf_your_token_here"

# Or add to .env file and source it:
source .env
```

**Note:** The librarian auto-detects which API key is available. OpenAI embeddings use `text-embedding-3-small` with 1024 dimensions (OpenSearch limit).

## 1. Start Infrastructure

```bash
cd examples/apps/librarian
docker-compose up -d
```

Wait for all services to be healthy:

```bash
# Check service health
docker-compose ps

# Expected output:
# opensearch          healthy
# opensearch-dashboards healthy
# prometheus          healthy
# grafana             healthy
# jaeger              healthy
```

Verify OpenSearch is ready:

```bash
curl http://localhost:9200/_cluster/health?wait_for_status=yellow
```

## 2. Index Books

Index the quick preset (10 classic books):

```bash
cargo run -p librarian --bin indexer -- --preset quick
```

Expected output:
```
╔══════════════════════════════════════════════════════════════╗
║              Book Search Indexer                             ║
╠══════════════════════════════════════════════════════════════╣
║  Books to index:     10                                      ║
║  Chunk size:        1000 chars                               ║
║  Chunk overlap:      200 chars                               ║
║  Index:                           books                      ║
╚══════════════════════════════════════════════════════════════╝

INFO indexer: Auto-detected OPENAI_API_KEY, using OpenAI embeddings...
INFO indexer: Creating OpenSearch index with embedding dimension 1024...
INFO indexer: Processing: Pride and Prejudice by Jane Austen
  [1342] 929 chunks indexed
INFO indexer: Processing: Moby Dick by Herman Melville
  [2701] 1551 chunks indexed
INFO indexer: Processing: Frankenstein by Mary Shelley
  [84] 534 chunks indexed
...

╔══════════════════════════════════════════════════════════════╗
║              Indexing Complete                               ║
╠══════════════════════════════════════════════════════════════╣
║  Books indexed:      10 /   10                               ║
║  Total chunks:        6823                                   ║
╚══════════════════════════════════════════════════════════════╝
```

Check index statistics:
```bash
cargo run -p librarian -- stats
```

## 3. Basic Search

### Hybrid Search (default)

```bash
cargo run -p librarian -- query "Who is Elizabeth Bennet?"
```

### Keyword Search

```bash
cargo run -p librarian -- query "white whale" --mode keyword
```

### Semantic Search

```bash
cargo run -p librarian -- query "obsession with revenge" --mode semantic
```

### Filtered Search

```bash
cargo run -p librarian -- query "love" --author "Austen"
```

## 4. Fan Out Search (Parallel Execution)

Demonstrate DashFlow's parallel node execution:

```bash
cargo run -p librarian -- fan-out "revenge and obsession" --show-timing
```

Expected output:
```
Fan Out Search Results:
========================

Timing breakdown:
  semantic - 120ms (8 results)
  keyword - 85ms (6 results)
  hybrid - 145ms (10 results)

Total time: 150ms
Sequential time: 350ms
Speedup: 2.33x

Found 15 unique results:
...
```

## 5. Character Analysis

### List Characters

```bash
cargo run -p librarian -- characters 1342
```

Expected output:
```
CHARACTERS in Pride and Prejudice (ID: 1342)
--------------------------------------------------

1. Elizabeth Bennet (47 mentions)
   Also known as: Lizzy, Eliza, Miss Elizabeth
   Sample: "Elizabeth, having rather expected to affront him..."

2. Fitzwilliam Darcy (38 mentions)
   Also known as: Darcy, Mr. Darcy
   Sample: "Mr. Darcy soon drew the attention of the room..."
...
```

### With Relationships

```bash
cargo run -p librarian -- characters 1342 --relationships
```

Expected output includes:
```
RELATIONSHIPS
--------------------------------------------------

Elizabeth Bennet <-> Fitzwilliam Darcy (Romantic)
   Central romantic relationship; initially antagonistic, evolving to love and marriage
   Evidence: "In vain I have struggled..."

Jane Bennet <-> Mr. Bingley (Romantic)
   Secondary romantic relationship; separated by misunderstanding, reunited
...
```

## 6. Theme Analysis

```bash
cargo run -p librarian -- themes 1342 --with-evidence
```

Expected output:
```
THEMES in Pride and Prejudice (ID: 1342)
--------------------------------------------------

1. Pride (relevance: 0.89)
   The excessive belief in one's own worth or abilities
   Keywords: pride, proud, arrogant, arrogance, haughty
   Evidence:
     1. (chunk 23) "His pride, his abominable pride..."
     2. (chunk 156) "...pride and vanity had been the means..."

2. Prejudice (relevance: 0.85)
   Preconceived opinion not based on reason or experience
...
```

## 7. Full Book Analysis

```bash
cargo run -p librarian -- analyze 1342
```

## 8. Interactive Chat with Memory

```bash
cargo run -p librarian -- chat --user alice
```

Example session:
```
Superhuman Librarian Chat
=========================
Type your questions, or 'quit' to exit.

You: What happens in Pride and Prejudice?
Librarian: Based on my search, I found 3 relevant passages...

You: Tell me more about Darcy
Librarian: [uses conversation context for better search]...

You: quit
Goodbye! Your conversation has been saved.
```

## 9. Bookmarks and Memory

### Add Bookmark

```bash
cargo run -p librarian -- bookmark add --book 1342 --chunk 156 --note "Darcy's letter is the turning point"
```

### View Memory

```bash
cargo run -p librarian -- memory show --user alice
```

## 10. Run Evaluation

```bash
cargo run -p librarian --bin eval
```

Expected output:
```
Evaluation Results
==================
Total questions: 30
Retrieval accuracy: 87%
Average score: 0.82

Detailed Results:
1. "What is Mr. Darcy's first name?" - ✓ (score: 1.00)
2. "Why does Elizabeth initially dislike Mr. Darcy?" - ✓ (score: 0.85)
...
```

## 11. View Telemetry

### Grafana Dashboard

Open http://localhost:3000 (admin/admin)

Navigate to: Dashboards → Librarian → Overview

Panels show:
- Query rate by type (hybrid/keyword/semantic)
- Search latency (P50, P95, P99)
- Fan out speedup metrics
- Analysis counts
- Error rates

### Jaeger Traces

Open http://localhost:16686

Search for service: `librarian`

View trace for a search query:
- `parse_query` span
- `hybrid_search` span (parallel children for kNN and BM25)
- `generate` span
- Total request time

### OpenSearch Dashboards

Open http://localhost:5601

View:
- Index pattern: `books`
- Document count
- Index mappings

## 12. JSON Output for Automation

All commands support `--format json`:

```bash
# Characters as JSON
cargo run -p librarian -- characters 1342 --format json

# Themes as JSON
cargo run -p librarian -- themes 1342 --format json

# Full analysis as JSON
cargo run -p librarian -- analyze 1342 --format json
```

## Feature Summary

| Feature | Status | Command |
|---------|--------|---------|
| Hybrid Search | ✅ | `query` |
| Keyword Search | ✅ | `query --mode keyword` |
| Semantic Search | ✅ | `query --mode semantic` |
| Fan Out | ✅ | `fan-out` |
| Memory | ✅ | `chat`, `memory` |
| Bookmarks | ✅ | `bookmark` |
| Character Analysis | ✅ | `characters` |
| Theme Analysis | ✅ | `themes` |
| Full Analysis | ✅ | `analyze` |
| Evaluation | ✅ | `eval` binary |
| Telemetry | ✅ | Grafana, Jaeger |

## Troubleshooting

### OpenSearch not starting

```bash
# Check logs
docker-compose logs opensearch

# Increase vm.max_map_count if needed
sudo sysctl -w vm.max_map_count=262144
```

### OpenSearch index creation blocked

If you see "cluster create-index blocked" error:
```bash
# Clear the index creation block
curl -X PUT "http://localhost:9200/_cluster/settings" \
  -H 'Content-Type: application/json' \
  -d'{"persistent":{"cluster.blocks.create_index":null}}'
```

### Embedding API limits

**OpenAI:** Uses `text-embedding-3-small` with 1024 dimensions (OpenSearch max). Monitor usage at https://platform.openai.com/usage

**HuggingFace:** The Inference API has rate limits for free tier. Set `HF_TOKEN` for higher limits.

### Large books failing to index

Very large books (like War and Peace) may exceed OpenSearch's bulk request limit. The indexer will continue with other books. You can:
- Index large books individually with smaller chunk sizes
- Increase OpenSearch memory limits in docker-compose.yml

### Evaluation failures

If eval shows low scores:
1. Ensure books are indexed: `cargo run -p librarian -- stats`
2. Check OpenSearch is running: `curl http://localhost:9200`
3. Verify embeddings work: Run a simple query first
4. Ensure the embedding dimension matches index (1024 for OpenAI, 384 for HuggingFace)
