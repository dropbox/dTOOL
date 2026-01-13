# Load Testing Guide

Comprehensive guide for load testing DashFlow example applications.

> **Historical Note:** Previous example apps (document_search_streaming, advanced_rag, code_assistant)
> have been consolidated into the `librarian` paragon application. This guide has been updated to
> focus on load testing librarian and provides general patterns applicable to any DashFlow application.

---

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Load Testing Framework](#load-testing-framework)
4. [Load Levels](#load-levels)
5. [Usage](#usage)
6. [Interpreting Results](#interpreting-results)
7. [Best Practices](#best-practices)

---

## Overview

The load testing framework validates system behavior under sustained load and identifies performance bottlenecks.

### Key Features

- **4 Load Levels:** Sequential (1 req/s), Moderate (10 req/s), High (50 req/s), Stress (100 req/s)
- **Real LLM Calls:** Uses OpenAI API for production-realistic testing
- **Metrics Capture:** Latency (p50/p95/p99), error rate, throughput, success rate
- **Prometheus Integration:** Real-time metrics during load tests

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Load Testing Framework                        │
└─────────────────────────────────────────────────────────────────┘
                               │
                       ┌───────▼───────┐
                       │   librarian   │
                       │   (RAG app)   │
                       └───────┬───────┘
                               │
                   ┌───────────▼───────────┐
                   │   Query Corpus        │
                   │   - RAG queries       │
                   │   - Simple/complex    │
                   │   - Edge cases        │
                   └───────────┬───────────┘
                               │
                   ┌───────────▼───────────┐
                   │   Results & Metrics   │
                   │   - CSV results       │
                   │   - JSON metrics      │
                   │   - Prometheus data   │
                   └───────────────────────┘
```

---

## Quick Start

### Prerequisites

1. **Build Release Binary:**
   ```bash
   cargo build --release -p librarian
   ```

2. **Set OpenAI API Key:**
   ```bash
   export OPENAI_API_KEY="your-key-here"
   ```

3. **Optional: Start Prometheus (for metrics capture):**
   ```bash
   docker-compose up -d prometheus
   ```

### Run Basic Load Test

```bash
# Sequential queries (baseline)
for i in {1..10}; do
    time cargo run -p librarian --release -- query "What is async in Rust?"
done

# Analyze latency distribution
# Record each execution time and compute p50/p95/p99
```

---

## Load Testing Framework

### Components

1. **Application Under Test:** `librarian`
   - RAG application with vector search, LLM calls, streaming
   - Supports query mode and interactive mode
   - Exposes Prometheus metrics

2. **Test Queries:**
   - Simple queries: Direct retrieval
   - Complex queries: Multi-step reasoning
   - Edge cases: Error handling validation

### Data Flow

```
Query Corpus
    │
    ├─► Load Generator (Script/Tool)
    │       │
    │       └─► librarian binary
    │
    └─► Results CSV
            │
            ├─► Analysis Script
            │       └─► Detailed Metrics
            │
            └─► Prometheus (optional)
                    └─► Time-series metrics
```

---

## Load Levels

### 1. Sequential Load (1 req/s)

**Purpose:** Baseline performance validation with no concurrency

**Configuration:**
- Concurrent requests: 1
- Request rate: 1 req/s

**Expected Results:**
- 100% success rate
- Lowest latency (no queueing, no contention)
- Consistent performance

**Example:**
```bash
for i in {1..20}; do
    cargo run -p librarian --release -- query "Explain Rust ownership" &
    sleep 1
    wait
done
```

### 2. Moderate Load (10 req/s)

**Purpose:** Sustained production load simulation

**Expected Results:**
- 95%+ success rate
- Latency increase due to queueing (10-30% higher than sequential)

### 3. High Load (50 req/s)

**Purpose:** Burst traffic simulation

**Expected Results:**
- 80-90% success rate (OpenAI rate limits may hit)
- High latency variance

### 4. Stress Test (100 req/s)

**Purpose:** Find breaking point and failure modes

**Expected Results:**
- 50-70% success rate (many failures expected)
- OpenAI rate limit errors (429)

---

## Usage

### Environment Variables

**Required:**
- `OPENAI_API_KEY`: OpenAI API key

**Optional:**
- `RUST_LOG`: Logging level (info, debug, warn)
- `OTEL_EXPORTER_OTLP_ENDPOINT`: Jaeger endpoint for tracing

### Running Librarian

```bash
# Single query
cargo run -p librarian --release -- query "What is async in Rust?"

# Interactive mode (multi-turn)
cargo run -p librarian --release -- interactive

# With streaming
cargo run -p librarian --release --features dashstream -- query "Explain tokio"

# Run evaluation suite
cargo run -p librarian --release -- eval --suite data/eval_suite.json
```

---

## Interpreting Results

### Success Metrics

**Success Rate:**
- **>=95%**: Excellent (production-ready)
- **90-95%**: Good (acceptable for moderate load)
- **80-90%**: Fair (high load, some failures expected)
- **<80%**: Poor (stress test or system overload)

**Latency (p95):**
- **<5s**: Excellent
- **5-10s**: Good
- **10-20s**: Fair
- **>20s**: Poor (optimization needed)

### Error Analysis

**Common Errors:**

1. **Timeout (exit code 124):**
   - Query exceeded timeout
   - Solution: Optimize query, increase timeout

2. **OpenAI Rate Limit (429):**
   - Hit OpenAI API rate limit
   - Solution: Reduce request rate, upgrade API tier

3. **Runtime Error:**
   - Application panic or error
   - Solution: Check logs, fix error handling

---

## Best Practices

### 1. Start with Sequential Load

Establish baseline performance before adding concurrency.

### 2. Increment Load Gradually

Progress: sequential -> moderate -> high -> stress

### 3. Monitor System Resources

Watch CPU, memory, network during tests.

### 4. Use Prometheus for Real-Time Monitoring

```bash
# Start observability stack
docker-compose up -d

# View real-time metrics
open http://localhost:3000  # Grafana
```

### 5. Cost Management

Load testing with OpenAI API can be expensive:
- Use GPT-3.5-turbo for load testing (10x cheaper than GPT-4)
- Reduce test duration for cost savings
- Use smaller query corpus

---

## See Also

- [README_OBSERVABILITY.md](README_OBSERVABILITY.md) - Observability patterns
- [TESTING.md](TESTING.md) - General testing guide
- [apps/librarian/README.md](librarian/README.md) - Librarian documentation

---

**Last Updated:** December 19, 2025
