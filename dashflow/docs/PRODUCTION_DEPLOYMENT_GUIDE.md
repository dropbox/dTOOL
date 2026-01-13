# Production Deployment Guide - Quality AI System

**Last Updated:** 2025-12-25
**Target:** Dropbox Dash - World's Greatest ChatGPT for Work

---

## Executive Summary

This guide documents how to deploy the unified quality agent with all 15 architectural innovations.

**⚠️ EVALUATION GUIDANCE:**

Before deploying to production, run a fresh evaluation to measure current system performance:

```bash
# Run the evaluation suite with real LLM calls
OPENAI_API_KEY="..." cargo test -p dashflow --test phase2b_real_e2e_tests -- --ignored

# Or use the dashflow CLI
dashflow eval run --suite quality_evaluation
```

**Target Metrics:**

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Tool use success rate | 100% | `dashflow eval run --suite tool_use` |
| Average quality score | ≥0.98 | Quality judge metrics in Grafana |
| Success rate (quality ≥0.95) | ≥90% | `quality_threshold_pass_rate` |
| Average cost per query | <$0.05 | Cost tracking in DashStream |
| P95 latency | <5s | Latency histogram in Prometheus |

> **Note:** Always run fresh evaluations before production deployment. Metrics shown here are targets, not current values.

---

## Architecture Overview

### System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                    PRODUCTION QUALITY AGENT                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Query → Confidence Prediction → Strategy Selection              │
│            ↓                           ↓                          │
│     [Low Conf: Premium]      [High Conf: Fast]                   │
│            ↓                           ↓                          │
│       Search-First            Direct Answer                      │
│            ↓                           ↓                          │
│            └──────────→ Tool Context Injection ←─────────┘       │
│                               ↓                                   │
│                      Response Validation                         │
│                               ↓                                   │
│                         Quality Gate                             │
│                               ↓                                   │
│                    [Score < 0.95?] ─→ Retry (CYCLE)             │
│                               ↓                                   │
│                    [Score ≥ 0.95?] → END                         │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
         ↓                    ↓                    ↓
    DashStream          Quality Judge          Monitoring
    Telemetry           (LLM-as-judge)         Dashboard
         ↓                    ↓                    ↓
      Kafka              Quality Events         Alerts
```

### 15 Innovations Integrated

**Phase 1: Framework Architecture**
1. ✅ Self-Correcting Graph (retry loops)
2. ✅ Confidence Scoring (self-assessment)
4. ✅ CRAG Agent (document grading)
5. ✅ Response Validator ("couldn't find" detector)
6. ✅ Tool Result Validator (pre-check)
10. ✅ Quality Gate (mandatory threshold)

**Phase 2: Advanced Patterns**
7. ✅ DashStream Quality Integration (telemetry)
8. ✅ Multi-Model Cascade (cost optimization)
9. ✅ Parallel Multi-Strategy (voting)

**Phase 3: Response Refinement**
3. ✅ Dual-Path Agent (parallel strategies)
11. ✅ QA Subgraph (modular checking)
12. ✅ Active Learning (continuous improvement)
13. ✅ Committee Judge (multi-model voting)
14. ✅ Mandatory Tool Context (re-injection)
15. ✅ Confidence Calibration (pre-emptive routing)

---

## Quick Start

### 1. Run the Unified Agent

```bash
# Build
cargo build --package dashflow --example unified_quality_agent

# Run
cargo run --package dashflow --example unified_quality_agent

# Expected output:
# - Success rate: 100%
# - Average quality: 0.97
# - Tool use: 100%
```

### 2. Review Individual Innovations

Each innovation has a standalone example:

```bash
# Self-correcting with retry loops
cargo run --package dashflow --example quality_enforced_agent

# Multi-model cascade
cargo run --package dashflow --example cascading_agent

# Confidence calibration
cargo run --package dashflow --example confidence_calibration

# Parallel strategies
cargo run --package dashflow --example dual_path_agent

# QA subgraph
cargo run --package dashflow --example qa_subgraph

# Committee judge voting
cargo run --package dashflow --example committee_judge

# Tool context management
cargo run --package dashflow --example mandatory_tool_context

# Active learning
cargo run --package dashflow --example active_learning
```

---

## Production Configuration

### Environment Setup

**Required:**
```bash
# OpenAI API key (for LLM calls and judge)
export OPENAI_API_KEY=your_key_here

# Kafka configuration (for telemetry)
export KAFKA_BROKERS=localhost:9092
export KAFKA_TOPIC=dashstream-quality  # default topic name
```

**Optional (for advanced features):**
```bash
# Anthropic API (for Claude judge in committee)
export ANTHROPIC_API_KEY=your_key_here

# Vector store (for semantic search)
export CHROMA_URL=http://localhost:8000

# Web search (for external retrieval)
export TAVILY_API_KEY=your_key_here
```

**Kafka Security (for secured Kafka clusters):**
```bash
# Security protocol: plaintext (default), ssl, sasl_plaintext, sasl_ssl
export KAFKA_SECURITY_PROTOCOL=sasl_ssl

# SASL authentication
export KAFKA_SASL_MECHANISM=SCRAM-SHA-256
export KAFKA_SASL_USERNAME=kafka-user
export KAFKA_SASL_PASSWORD=kafka-password

# TLS/SSL settings
export KAFKA_SSL_CA_LOCATION=/etc/kafka/ca.pem
# Optional: mTLS client certificates
# export KAFKA_SSL_CERTIFICATE_LOCATION=/etc/kafka/client.pem
# export KAFKA_SSL_KEY_LOCATION=/etc/kafka/client-key.pem
```

### Quality Gate Configuration

```rust
use dashflow::quality::{QualityGate, QualityGateConfig, RetryStrategy};
use dashflow::core::rate_limiters::InMemoryRateLimiter;
use std::sync::Arc;
use std::time::Duration;

// Configure quality gate with retry and rate limiting
let config = QualityGateConfig {
    threshold: 0.95,              // Minimum acceptable quality score
    max_retries: 3,               // Maximum retry attempts
    retry_strategy: RetryStrategy::StrongerPrompt,
    emit_telemetry: true,         // Enable DashStream telemetry
    rate_limiter: Some(Arc::new(
        InMemoryRateLimiter::new(5.0, Duration::from_millis(100), 10.0)
    )),
};

// Create quality gate with generate and judge functions
let quality_gate = QualityGate::new(
    config,
    |input| async move {
        // Your LLM generation logic here
        Ok("Generated response".to_string())
    },
    |response, _input| async move {
        // Your quality judgment logic here (returns 0.0-1.0)
        Ok(0.95)
    },
);

// Use the quality gate
let result = quality_gate.run("user query").await?;
```

---

## Monitoring & Observability

### DashStream Integration

All agent interactions can be logged via the DashStream protobuf protocol:

```rust
use dashflow_streaming::{DashStreamMessage, DashStreamProducer, Event, EventType};
use dashflow_streaming::kafka::KafkaConfig;

// Create a Kafka producer for streaming events
let config = KafkaConfig::new("localhost:9092", "dashstream.events");
let producer = DashStreamProducer::new(config).await?;

// Send events during execution
producer.send_event(Event {
    event_type: EventType::GraphStart as i32,
    node_id: "agent".to_string(),
    ..Default::default()
}).await?;

// Events are automatically:
// 1. Encoded as Protocol Buffers
// 2. Compressed with Zstd
// 3. Sent to Kafka
```

### Quality Events

**Event Schema:**

```protobuf
message QualityEvaluation {
  Header header = 1;
  string response_id = 2;
  float accuracy = 3;
  float relevance = 4;
  float completeness = 5;
  float overall_score = 6;
  string reasoning = 7;
  repeated string issues = 8;  // ["tool_results_ignored", etc.]
}
```

### Dashboard Metrics

**Real-time monitoring:**

```bash
# View quality scores in real-time
cargo run --bin quality_monitor -- --thread-id <thread_id>

# Output:
Turn 1: Score 0.96 ✅ (Acc:0.95, Rel:0.98, Comp:0.95)
Turn 2: Score 0.92 ✅ (Acc:0.90, Rel:0.95, Comp:0.90)
Turn 3: Score 0.98 ✅ (Acc:0.98, Rel:0.99, Comp:0.97)

Average: 0.95
Issues: 0 low-quality responses
Tool use: 100%
```

**Aggregate analytics:**

```bash
# Quality distribution over time
cargo run --bin quality_analytics -- --time-range 24h

# Output:
=== Quality Analytics ===
Time period: Last 24 hours
Total responses: 1,247

Quality Distribution:
0.95-1.00: 876 (70.2%) ✅
0.90-0.95: 298 (23.9%) ⚠️
0.80-0.90:  51 (4.1%) ❌
<0.80:      22 (1.8%) 🚨

Common Issues:
- tool_results_ignored: 73 instances (5.9%)
- incomplete_coverage: 31 instances (2.5%)
- factual_errors: 8 instances (0.6%)

Model Usage:
- gpt-4o-mini: 1,121 (90.0%) - $0.56 total
- gpt-4: 126 (10.0%) - $3.78 total
- Total cost: $4.34
- Cost per query: $0.0035
```

### Alerting

**Configure alerts via Prometheus alerting rules:**

```yaml
# monitoring/alert_rules.yml
groups:
  - name: quality-alerts
    rules:
      - alert: QualityDegradation
        expr: dashstream_quality_monitor_quality_score < 0.95
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Quality score below threshold"

      - alert: HighFailureRate
        # Calculate failure rate from counters
        expr: |
          sum(rate(dashstream_quality_monitor_queries_failed_total[5m])) /
          sum(rate(dashstream_quality_monitor_queries_total[5m])) > 0.10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Query failure rate exceeds 10%"
```

> **Available Metrics:** See `crates/dashflow-prometheus-exporter/README.md` for the complete list of exported metrics.

**Alert Examples:**

```
🚨 ALERT: Low Quality Response
Thread: abc123
Score: 0.82 (< 0.95 threshold)
Issues: tool_results_ignored
Time: 2025-11-14 10:23:45 UTC

🚨 ALERT: High Tool Ignore Rate
Window: Last 5 minutes
Rate: 12.3% (> 5% threshold)
Affected queries: 7 of 57
Action: Investigate system prompt or retry logic
```

---

## Quality Validation

### Test Suite

**Run comprehensive quality tests:**

```bash
# Full test suite
cargo test --package dashflow --lib quality

# Specific innovation tests
cargo test --package dashflow --test self_correcting
cargo test --package dashflow --test cascade
cargo test --package dashflow --test confidence_calibration
```

### Evaluation Scenarios

**Production evaluation using MultiDimensionalJudge:**

```rust
use dashflow_evals::quality_judge::{MultiDimensionalJudge, QualityScore};
use std::sync::Arc;

let judge = MultiDimensionalJudge::new(judge_model);

// Prepare test scenarios
let scenarios = vec![
    ("What is the capital of France?", "Paris is the capital.", "Paris"),
    ("Explain machine learning", "ML is a subset of AI...", ""),
    // ... more scenarios
];

// Batch evaluation with concurrency control
let scores: Vec<QualityScore> = judge.score_batch_with_concurrency(
    &scenarios,
    5,  // max concurrent evaluations
).await?;

// Analyze results
let avg_quality: f64 = scores.iter().map(|s| s.overall).sum::<f64>() / scores.len() as f64;
let success_count = scores.iter().filter(|s| s.meets_threshold(0.95)).count();

println!("Average quality: {:.3}", avg_quality);
println!("Success rate: {:.1}%", (success_count as f64 / scores.len() as f64) * 100.0);
```

**Production Targets:**

| Metric | Target | How to Verify |
|--------|--------|---------------|
| Tool use success rate | 100% | `dashflow eval run --suite tool_use` |
| Average quality score | ≥0.98 | Quality metrics in Grafana dashboard |
| Success rate (quality ≥0.95) | ≥90% | `quality_threshold_pass_rate` metric |
| Responses below 0.95 | <2% | Monitor `quality_failures_total` |
| Average cost per query | <$0.05 | Cost tracking via DashStream |
| P95 latency | <5s | `request_duration_seconds` histogram |

> **Note:** Run `dashflow eval run --suite full` before each production deployment to verify these targets are met.

---

## Deployment Steps

> **Pre-deployment Checklist:**
> 1. Run `dashflow eval run --suite full` and verify all targets are met
> 2. Review Grafana dashboards for any anomalies
> 3. Ensure observability infrastructure is running (Prometheus, Grafana)

### Step 1: Infrastructure Setup

```bash
# 1. Start Kafka (for DashStream)
docker run -d \
  --name kafka \
  -p 9092:9092 \
  apache/kafka:latest

# 2. Start Chroma (for vector search)
docker run -d \
  --name chroma \
  -p 8000:8000 \
  chromadb/chroma:latest

# 3. Verify services
curl http://localhost:8000/api/v1/heartbeat  # Chroma
# Should return: {"status": "ok"}
```

### Step 2: Build Production Binary

```bash
# Build with optimizations
cargo build --release --package dashflow

# Binary location:
# target/release/dashflow
```

### Step 3: Configuration

Create `config/production.toml`:

```toml
[agent]
fast_model = "gpt-4o-mini"
premium_model = "gpt-4"
judge_model = "gpt-4o-mini"

[quality]
threshold = 0.95
max_retries = 3
enable_auto_retry = true

[telemetry]
enable_dashstream = true
kafka_brokers = ["localhost:9092"]
topic_prefix = "dashflow_prod"

[cost]
enable_cascade = true
confidence_threshold = 0.75

[monitoring]
enable_quality_monitoring = true
alert_threshold = 0.95
alert_window_minutes = 5
```

### Step 4: Deploy

```bash
# Run production service
./target/release/dashflow \
  --config config/production.toml \
  --port 8080

# Or with Docker:
docker run -d \
  --name dashflow-agent \
  -p 8080:8080 \
  -v $(pwd)/config:/config \
  -e OPENAI_API_KEY=$OPENAI_API_KEY \
  dashflow:latest
```

### Step 5: Verify Deployment

```bash
# Health check
curl http://localhost:8080/health
# Expected: {"status": "healthy", "quality_system": "operational"}

# Test query
curl -X POST http://localhost:8080/query \
  -H "Content-Type: application/json" \
  -d '{
    "query": "What is Rust?",
    "thread_id": "test_thread"
  }'

# Check metrics
curl http://localhost:8080/metrics
# Expected: quality scores, retry rates, model usage stats
```

---

## Performance Tuning

### Optimization Strategies

**1. Confidence Calibration**

Train the confidence model on production data:

```rust
// Collect training data from production
let training_data = load_production_logs()?;

// Train classifier
let classifier = ToolDecisionClassifier::train(training_data)?;

// Deploy updated model
agent.update_confidence_classifier(classifier);
```

**2. Model Selection**

Adjust model selection based on cost/quality tradeoffs:

```rust
// Cost-optimized (90% savings)
.fast_model("gpt-4o-mini")
.premium_model("gpt-4")
.confidence_threshold(0.70)  // Aggressive

// Quality-optimized (best quality)
.fast_model("gpt-4")
.premium_model("gpt-4o")
.confidence_threshold(0.90)  // Conservative
```

**3. Retry Optimization**

Tune retry behavior:

```rust
// Aggressive retries (best quality)
.max_retries(5)
.retry_on_score_below(0.95)

// Conservative retries (faster)
.max_retries(2)
.retry_on_score_below(0.85)
```

**4. Parallel Strategies**

Enable parallel execution for complex queries:

```rust
.enable_dual_path(true)  // Run fast + premium in parallel
.enable_committee_judge(true)  // Use 3 judges for voting
```

---

## Active Learning

### Continuous Improvement Loop

```
Production Queries → Quality Scores → Low-Quality Detection → Training Data
                                          ↓
                                    Retrain Classifier
                                          ↓
                                    Deploy Updated Model
                                          ↓
                                    Improved Predictions
```

**Implementation:**

```bash
# 1. Collect low-quality conversations
cargo run --bin collect_training_data -- \
  --quality-threshold 0.90 \
  --output training_data.jsonl

# 2. Train classifier
cargo run --bin train_classifier -- \
  --input training_data.jsonl \
  --output classifier_v2.model

# 3. Deploy updated classifier
cargo run --bin deploy_classifier -- \
  --model classifier_v2.model \
  --target production
```

**Expected Improvement:**

- Week 1: 65% accuracy on confidence prediction
- Week 4: 80% accuracy
- Week 12: 95% accuracy

---

## Troubleshooting

### Common Issues

**Issue: Low quality scores (<0.95)**

**Solution:**
1. Check if tool results are being returned
2. Verify response validator is catching "couldn't find"
3. Increase max_retries
4. Route more queries to premium model (lower confidence_threshold)

**Issue: High costs**

**Solution:**
1. Increase confidence_threshold (use fast model more)
2. Enable confidence calibration (better routing)
3. Review query complexity (pre-filter simple queries)

**Issue: Slow response times**

**Solution:**
1. Reduce max_retries
2. Disable parallel strategies for simple queries
3. Use streaming for progressive feedback
4. Cache frequent queries

**Issue: Tool results ignored**

**Solution:**
1. Verify ResponseValidator is enabled
2. Check tool context injection count
3. Add stronger system prompts
4. Use mandatory tool context (INNOVATION 14)

---

## Production Checklist

**Before Launch:**

- [ ] All 15 innovations tested individually
- [ ] Unified agent tested on 100+ scenarios
- [ ] Quality metrics meet targets (100% tool use, 98%+ quality)
- [ ] DashFlow Streaming telemetry configured
- [ ] Kafka cluster operational
- [ ] Monitoring dashboard deployed
- [ ] Alerts configured
- [ ] Cost tracking enabled
- [ ] Active learning pipeline ready
- [ ] Rollback plan documented

**Day 1 Monitoring:**

- [ ] Check quality distribution (>95% should be ≥0.95)
- [ ] Monitor tool use rate (should be 100%)
- [ ] Track costs (should be <$0.05/query)
- [ ] Review retry rates (should be <20%)
- [ ] Investigate any low-quality responses

**Week 1 Optimization:**

- [ ] Collect 1,000+ production queries
- [ ] Retrain confidence classifier
- [ ] Tune thresholds based on actual performance
- [ ] Update model selection strategy
- [ ] Document any edge cases

---

## Success Criteria

### Production Readiness

✅ **Tool Use:** 100% success rate
✅ **Quality:** 98%+ average score
✅ **Cost:** 90% savings through intelligent routing
✅ **Reliability:** <2% responses below threshold
✅ **Self-Healing:** Automatic retries on failure
✅ **Observability:** Real-time quality monitoring
✅ **Continuous Improvement:** Active learning enabled

### Dropbox Dash Deployment

**Status:** Run evaluation to determine readiness

Before deploying to Dropbox Dash, verify:

1. **Run Fresh Evaluation:**
   ```bash
   dashflow eval run --suite quality_evaluation --output results.json
   ```

2. **Check Target Metrics:**
   - Tool use success rate: ≥100%
   - Average quality score: ≥0.98
   - Success rate (quality ≥0.95): ≥90%

3. **Review Observability:**
   - Verify Grafana dashboards are accessible
   - Check Prometheus metrics are being scraped
   - Confirm DashStream telemetry is flowing

4. **Deploy:**
   Once all targets are met, proceed with deployment following the steps above.

---

## References

- **docs/OBSERVABILITY_INFRASTRUCTURE.md** - Observability stack setup (Prometheus, Grafana, DashStream)
- **docs/COOKBOOK.md** - Common usage patterns and examples
- **docs/CONFIGURATION.md** - Environment variables and configuration options
- **crates/dashflow/tests/phase2b_real_e2e_tests.rs** - Real LLM E2E tests (run with `--ignored`)

### Architecture Documentation

- **15 Architectural Innovations** - See `COMPLETED_INITIATIVES.md`
- **Quality Agent Implementation** - `crates/dashflow/src/quality/`
- **Evaluation Framework** - `crates/dashflow-evals/`

---
