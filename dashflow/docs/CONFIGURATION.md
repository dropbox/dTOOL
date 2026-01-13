# DashFlow Configuration Reference

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

This document lists all environment variables and configuration options for DashFlow.

## Port Allocation Reference

DashFlow components use the following default ports. When running multiple services locally for development, use the **Collision-Free Dev Ports** to avoid conflicts.

### DashFlow Services

| Service | Default Port | Env Variable | Collision-Free Dev Port | Notes |
|---------|-------------|--------------|------------------------|-------|
| **WebSocket Server** | 3002 | `WEBSOCKET_PORT` | 3002 | Fallback to 3003-3005 in dev mode |
| **Quality Monitor** | 3003 | `QUALITY_MONITOR_PORT` | 3003 | - |
| **Registry API** | 3001 | `REGISTRY_PORT` | 3001 | - |
| **Prometheus Exporter** | 8080 | `METRICS_PORT` | 8081 | **Conflicts** with Weaviate/Ollama |
| **LangServe** | 8000 | - | 8010 | **Conflicts** with Chroma/Qdrant |
| **Grafana** | 3000 | `GRAFANA_URL` | 3000 | Standard |
| **Prometheus** | 9090 | `PROMETHEUS_URL` | 9090 | Standard |

### Third-Party Dependencies

| Service | Default Port | Env Variable | Collision-Free Dev Port | Notes |
|---------|-------------|--------------|------------------------|-------|
| **ChromaDB** | 8000 | `CHROMA_URL` | 8020 | **Conflicts** with LangServe/Qdrant |
| **Weaviate** | 8080 | `WEAVIATE_URL` | 8030 | **Conflicts** with Prometheus Exporter |
| **Qdrant** | 6333/6334 | `QDRANT_URL` | 6333/6334 | No conflict |
| **Ollama** | 11434 | `OLLAMA_HOST` | 11434 | No conflict |
| **Kafka** | 9092 | `KAFKA_BROKERS` | 9092 | Standard |
| **Redis** | 6379 | `REDIS_URL` | 6379 | Standard |
| **Cassandra** | 9042 | - | 9042 | CQL port |

### ⚠️ Port Conflicts to Avoid

The following port combinations conflict by default:

1. **Port 8000**: ChromaDB, LangServe examples, Qdrant (legacy)
2. **Port 8080**: Weaviate, Prometheus Exporter, Ollama examples

**Recommended collision-free development setup:**

```bash
# DashFlow services (no changes needed)
export WEBSOCKET_PORT=3002
export QUALITY_MONITOR_PORT=3003

# Prometheus exporter - avoid 8080 conflict
export METRICS_PORT=8081

# Vector stores - unique ports
export CHROMA_URL=http://localhost:8020
export WEAVIATE_URL=http://localhost:8030

# Run with custom ports
docker run -p 8020:8000 chromadb/chroma
docker run -p 8030:8080 semitechnologies/weaviate
```

### Port Configurability Status

All DashFlow services support port configuration via environment variables:

| Component | Configurable | How to Configure |
|-----------|-------------|------------------|
| WebSocket Server | ✅ Yes | `WEBSOCKET_PORT=3002` |
| Prometheus Exporter | ✅ Yes | `METRICS_PORT=8081` |
| Registry API | ✅ Yes | `ApiConfig::default().with_port(port)` |
| LangServe | ✅ Yes | Bind address in code |
| Self-Improve Daemon | ✅ Yes | `DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL` |

---

## Environment Variables

### Core Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_TRACE` | `true` | Enable automatic trace persistence to `.dashflow/traces/` |
| `DASHFLOW_LIVE_INTROSPECTION` | `true` | Enable live introspection during graph execution |
| `DASHFLOW_API_URL` | `http://localhost:3002` | API server URL for test utilities (WebSocket server) |

### Registry Client

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_REGISTRY_URL` | `https://registry.dashflow.ai` | Package registry URL |
| `DASHFLOW_REGISTRY_API_KEY` | (none) | API key for registry authentication |

### Trace Retention

Controls automatic cleanup of execution traces in `.dashflow/traces/`.

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_TRACE_RETENTION` | `true` | Enable trace retention policy |
| `DASHFLOW_TRACE_MAX_COUNT` | `1000` | Maximum number of traces to keep |
| `DASHFLOW_TRACE_MAX_AGE_DAYS` | `30` | Maximum age of traces in days |
| `DASHFLOW_TRACE_MAX_SIZE_MB` | `500` | Maximum total size in MB |
| `DASHFLOW_TRACE_COMPRESS_AGE_DAYS` | `7` | Compress traces older than N days |

### Storage Policy

Controls cleanup of self-improvement data in `.dashflow/introspection/`.

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_STORAGE_RETENTION` | `true` | Enable storage retention policy |
| `DASHFLOW_STORAGE_MAX_REPORTS` | `100` | Maximum number of reports |
| `DASHFLOW_STORAGE_MAX_PLANS` | `200` | Maximum plans per status directory |
| `DASHFLOW_STORAGE_PLAN_AGE_DAYS` | `30` | Maximum age for implemented/failed plans |
| `DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS` | `90` | Maximum age for evaluated hypotheses |

### Storage Health Monitoring

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_STORAGE_WARNING_SIZE_MB` | `100` | Storage size warning threshold |
| `DASHFLOW_STORAGE_CRITICAL_SIZE_MB` | `500` | Storage size critical threshold |
| `DASHFLOW_STORAGE_REPORT_WARNING_COUNT` | `80` | Report count warning threshold |
| `DASHFLOW_STORAGE_PLAN_WARNING_COUNT` | `160` | Plan count warning threshold per status |

### Self-Improvement Daemon

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_SELF_IMPROVE_INTERVAL` | `60` | Analysis interval in seconds |
| `DASHFLOW_SELF_IMPROVE_TRACES_DIR` | `.dashflow/traces` | Directory to watch for traces |
| `DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL` | `http://localhost:9090` | Prometheus server URL |
| `DASHFLOW_SELF_IMPROVE_METRICS_SOURCE` | `http` | Metrics source: `http`, `inprocess`, `disabled` |
| `DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS` | `10000` | Node slowness threshold in ms |
| `DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD` | `0.05` | Error rate threshold (5%) |
| `DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD` | `3` | Repeated retry threshold |
| `DASHFLOW_SELF_IMPROVE_MIN_TRACES` | `10` | Minimum traces for analysis |
| `DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED` | `true` | Enable automatic cleanup |
| `DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL` | `10` | Cleanup every N analysis cycles |

### Health Check Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_HEALTH_STORAGE_PATH` | `.dashflow/introspection` | Storage path to check |
| `DASHFLOW_HEALTH_TRACES_PATH` | `.dashflow/traces` | Traces path to check |
| `DASHFLOW_HEALTH_CHECK_STORAGE` | `true` | Include storage in health checks |
| `DASHFLOW_HEALTH_CHECK_TRACES` | `true` | Include traces in health checks |
| `DASHFLOW_HEALTH_CHECK_CACHE` | `true` | Include cache in health checks |
| `DASHFLOW_HEALTH_MAX_STORAGE_MB` | `500` | Maximum healthy storage size |

### Colony (Distributed Workers)

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHFLOW_WORKER_ID` | (auto-generated) | Unique worker identifier |
| `DASHFLOW_WORKER_MODE` | `false` | Running as a colony worker |
| `DASHFLOW_PARENT_ID` | (none) | Parent worker ID |
| `DASHFLOW_TASK` | (none) | Task JSON for worker |
| `DASHFLOW_NETWORK_ENABLED` | `false` | Enable network communication |
| `DASHFLOW_NETWORK_PORT` | `0` | Network port (0 = auto) |
| `DASHFLOW_PARENT_PEER_ID` | (none) | Parent peer ID for networking |

## LLM Provider API Keys

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic (Claude) API key |
| `AZURE_OPENAI_API_KEY` | Azure OpenAI API key |
| `AZURE_OPENAI_ENDPOINT` | Azure OpenAI endpoint |
| `MISTRAL_API_KEY` | Mistral AI API key |
| `GROQ_API_KEY` | Groq API key |
| `DEEPSEEK_API_KEY` | DeepSeek API key |
| `FIREWORKS_API_KEY` | Fireworks AI API key |
| `COHERE_API_KEY` | Cohere API key |
| `XAI_API_KEY` | xAI (Grok) API key |
| `PERPLEXITY_API_KEY` | Perplexity API key |

## Vector Store Keys

| Variable | Description |
|----------|-------------|
| `PINECONE_API_KEY` | Pinecone API key |
| `PINECONE_ENVIRONMENT` | Pinecone environment |
| `QDRANT_URL` | Qdrant server URL |
| `QDRANT_API_KEY` | Qdrant API key (optional) |
| `CHROMA_URL` | ChromaDB server URL |
| `WEAVIATE_URL` | Weaviate server URL |
| `MILVUS_URL` | Milvus server URL |

## Observability

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Logging level (e.g., `debug`, `info`, `warn`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry collector endpoint |
| `LANGCHAIN_TRACING_V2` | Enable LangSmith tracing (`true`/`false`) |
| `LANGCHAIN_API_KEY` | LangSmith API key |
| `LANGCHAIN_PROJECT` | LangSmith project name |

### WebSocket Server

Configuration for the observability WebSocket server (`websocket-server` binary).

| Variable | Default | Description |
|----------|---------|-------------|
| `WEBSOCKET_HOST` | `127.0.0.1` | Bind address (use `0.0.0.0` for network - see security warning) |
| `WEBSOCKET_PORT` | `3002` | WebSocket server port (falls back to 3003-3005) |
| `WEBSOCKET_MAX_CONNECTIONS_PER_IP` | `10` | Max WebSocket connections per IP (M-488 rate limiting) |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis URL for replay buffer |
| `EXPECTED_SCHEMAS_PATH` | `.dashflow/expected_schemas.json` | Expected graph schema storage |

## Kafka/Streaming

| Variable | Default | Description |
|----------|---------|-------------|
| `KAFKA_BROKERS` | `localhost:9092` | Kafka bootstrap servers (dashflow-streaming also accepts legacy alias `KAFKA_BOOTSTRAP_SERVERS`) |
| `KAFKA_BROKER_ADDRESS_FAMILY` | *(auto)* | rdkafka address family: `any`, `v4`, `v6` (auto-detect defaults to `v4` for localhost/Docker, `any` otherwise) |
| `KAFKA_TOPIC` | *(varies)* | Kafka topic to produce/consume (WebSocket + exporter default to `dashstream-quality`; `DashStreamConfig` default is `dashstream-events`) |
| `KAFKA_DLQ_TOPIC` | `${KAFKA_TOPIC}-dlq` | WebSocket server DLQ topic for failed decodes |
| `KAFKA_GROUP_ID` | *(varies)* | Kafka consumer group ID (defaults vary by component) |
| `KAFKA_AUTO_OFFSET_RESET` | `earliest` | Auto-offset reset policy (`earliest`/`latest`) |
| `KAFKA_OLD_DATA_GRACE_SECONDS` | `30` | WebSocket decode "old data" cutoff grace window |
| `KAFKA_LAG_CHECK_INTERVAL_SECS` | `10` | WebSocket lag monitor poll interval (seconds) |
| `KAFKA_LAG_STALE_PARTITION_SECS` | `60` | WebSocket lag monitor staleness threshold for partition offset updates (seconds) |
| `METRICS_PORT` | `9090` | Prometheus exporter port |

### Kafka Security Configuration (M-413)

For secure Kafka connections (TLS/SASL), configure these environment variables.
Use `KafkaSecurityConfig::from_env()` in Rust to load automatically.

Current wiring status (verified at HEAD):
- ✅ WebSocket server (`crates/dashflow-observability/src/bin/websocket_server/main.rs`) applies `KafkaSecurityConfig` to its Kafka consumer, DLQ producer, and lag-monitor metadata consumer.
- ✅ Prometheus exporter (`crates/dashflow-prometheus-exporter/src/main.rs`) applies `KafkaSecurityConfig` to its Kafka consumer.
- ✅ `dashflow-cli` Kafka telemetry commands use `KafkaSecurityConfig::create_client_config()` for rdkafka consumers.
- ✅ `dashflow-streaming` admin/metadata helpers in `crates/dashflow-streaming/src/kafka.rs` use `KafkaSecurityConfig::create_client_config()` for rdkafka clients.
- ✅ `dashflow-streaming` `ProducerConfig::from_env()` / `ConsumerConfig::from_env()` load TLS/SASL from `KafkaSecurityConfig::from_env()` for consistency across services.

| Variable | Default | Description |
|----------|---------|-------------|
| `KAFKA_SECURITY_PROTOCOL` | `plaintext` | Security protocol: `plaintext`, `ssl`, `sasl_plaintext`, `sasl_ssl` |
| `KAFKA_SASL_MECHANISM` | *(none)* | SASL mechanism: `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512`, `GSSAPI`, `OAUTHBEARER` |
| `KAFKA_SASL_USERNAME` | *(none)* | SASL username (required with SASL mechanisms) |
| `KAFKA_SASL_PASSWORD` | *(none)* | SASL password (required with SASL mechanisms) |
| `KAFKA_SSL_CA_LOCATION` | *(none)* | Path to CA certificate file |
| `KAFKA_SSL_CERTIFICATE_LOCATION` | *(none)* | Path to client certificate (mTLS) |
| `KAFKA_SSL_KEY_LOCATION` | *(none)* | Path to client private key (mTLS) |
| `KAFKA_SSL_KEY_PASSWORD` | *(none)* | Password for encrypted private key |
| `KAFKA_SSL_ENDPOINT_ALGORITHM` | `https` | Hostname verification: `https` (verify), `none` (skip) |

**Example: Production SASL + TLS Configuration**

```bash
# Enable SASL_SSL for production
export KAFKA_SECURITY_PROTOCOL=sasl_ssl
export KAFKA_SASL_MECHANISM=SCRAM-SHA-256
export KAFKA_SASL_USERNAME=kafka-user
export KAFKA_SASL_PASSWORD=kafka-password
export KAFKA_SSL_CA_LOCATION=/etc/kafka/ca.pem

# Optional: Mutual TLS (mTLS) for additional security
export KAFKA_SSL_CERTIFICATE_LOCATION=/etc/kafka/client.pem
export KAFKA_SSL_KEY_LOCATION=/etc/kafka/client-key.pem
```

**Example: TLS Only (no SASL authentication)**

```bash
export KAFKA_SECURITY_PROTOCOL=ssl
export KAFKA_SSL_CA_LOCATION=/etc/kafka/ca.pem
```

### Delivery Semantics (M-411)

DashFlow Streaming provides **at-least-once** delivery semantics. Messages are guaranteed to be delivered but may be delivered more than once in failure scenarios.

**How it works:**
- Producer enables Kafka idempotence (`enable.idempotence=true`) to prevent duplicates from broker-side retries within the same produce session
- Application-level retry on network errors may cause duplicates if the broker received the message but the acknowledgment was lost
- Each message has a unique `message_id` (UUID) in the Header for consumer-side deduplication

**Duplicate scenarios:**
1. Network timeout after broker receives message, before ACK → application retries → duplicate
2. Producer restart during send → new produce session → no idempotence protection

**Handling duplicates:**
- **Consumer-side deduplication**: Use `header.message_id` to track processed messages (recommended for critical workloads)
- **Idempotent processing**: Design consumers so processing the same message twice has no adverse effects
- **Accept at-least-once**: Many streaming use cases (metrics, logs) naturally tolerate duplicates

**For exactly-once semantics**, Kafka transactions are required (not currently supported by `DashStreamProducer`). For most telemetry/observability use cases, at-least-once with idempotent processing is sufficient.

**Topic Provisioning (M-410):**

Production deployments should pre-create topics with proper configuration rather than relying on Kafka auto-create:

```rust
use dashflow_streaming::{ensure_topics_with_dlq, recommended_config, dlq_config};

// Create main topic + DLQ with proper retention and partitioning
let (main_created, dlq_created) = ensure_topics_with_dlq(
    "kafka:9092",
    "dashstream-quality",
    recommended_config(),  // 10 partitions, 7-day retention
    dlq_config(),          // 3 partitions, 30-day retention
).await?;
```

### DLQ Durability Semantics (M-429)

When a Kafka message fails to decode/process, the WebSocket server sends it to the Dead Letter Queue (DLQ) for forensic analysis. The DLQ behavior is **fail-open** (best-effort):

**Current Behavior:**
- **DLQ send is asynchronous**: Background task sends to DLQ without blocking message processing
- **Offset commit proceeds regardless**: Kafka offset is committed even if DLQ send fails
- **Backpressure handling**: If DLQ concurrency limit is reached, messages are dropped (not queued)

**Implications:**
1. **Data loss under DLQ failure**: If Kafka DLQ is down, failed messages are lost (metrics incremented)
2. **No re-processing**: Once offset is committed, the main message won't be retried
3. **Best-effort forensics**: DLQ is for debugging, not guaranteed delivery

**Fail-Open Rationale:**
- Blocking on DLQ would halt all message processing (availability over forensics)
- Main message stream is prioritized over failed message diagnostics
- Metrics (`websocket_dlq_send_failures_total`) track DLQ issues for alerting

**Configuration:**

| Variable | Default | Description |
|----------|---------|-------------|
| `MAX_CONCURRENT_DLQ_SENDS` | 100 | Max concurrent background DLQ sends (semaphore limit) |
| `DLQ_SEND_TIMEOUT_SECS` | 5 | DLQ producer delivery timeout (maps to Kafka `message.timeout.ms`) |
| `KAFKA_DLQ_TOPIC` | `${KAFKA_TOPIC}-dlq` | DLQ topic name |

**Monitoring:**
- Alert on `websocket_dlq_send_failures_total` rate to detect DLQ issues
- Alert on `websocket_dlq_sends_total` rate for decode error trends
- Use `WebSocketDlqBroken` alert (see `monitoring/alert_rules.yml`)

**Future Options** (not currently implemented):
- **Fail-closed mode**: Block offset commit until DLQ succeeds (config knob)
- **Local buffering**: Buffer failed messages to disk when DLQ is unavailable
- **Retry with backoff**: Exponential backoff retry for transient DLQ failures

For most observability use cases, fail-open is appropriate since DLQ is primarily for debugging, not data recovery.

## Grafana Integration

| Variable | Default | Description |
|----------|---------|-------------|
| `GRAFANA_URL` | `http://localhost:3000` | Grafana server URL |
| `GRAFANA_USER` | `admin` | Grafana username |
| `GRAFANA_PASSWORD` | `admin` | Grafana password |

## Example Configuration

```bash
# ~/.bashrc or ~/.zshrc

# Core
export OPENAI_API_KEY="sk-..."
export DASHFLOW_TRACE=true
export DASHFLOW_LIVE_INTROSPECTION=true

# Self-improvement (optional)
export DASHFLOW_SELF_IMPROVE_INTERVAL=120
export DASHFLOW_TRACE_MAX_COUNT=500

# Observability (optional)
export RUST_LOG=info
export LANGCHAIN_TRACING_V2=true
export LANGCHAIN_API_KEY="ls__..."
```

## Configuration Precedence

1. Environment variables (highest priority)
2. `.env` file in project root
3. Built-in defaults (lowest priority)

---

For more information:
- [CLI Reference](CLI_REFERENCE.md) - Command-line options
- [Architecture Guide](ARCHITECTURE.md) - System design
- [Golden Path Guide](GOLDEN_PATH.md) - Recommended API patterns
