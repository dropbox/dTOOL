# DashFlow Streaming Protocol

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Ultra-Efficient Streaming Telemetry for DashFlow/DashFlow**

**Version:** 1.0.0-draft
**Status:** Design Phase
**Author:** Andrew Yates © 2026

---

## Executive Summary

DashFlow Streaming is a binary streaming protocol designed specifically for DashFlow/DashFlow telemetry, optimized for:
- **Extreme Efficiency**: 10-100× smaller than JSON (via protobuf + compression)
- **Diff-Based Updates**: Only transmit state changes (90%+ reduction)
- **Kafka-Native**: Designed for high-throughput distributed streaming
- **AI-Optimized**: Custom encoding for LLM tokens, embeddings, and graph state
- **Full Introspection**: Rich debugging tools without sacrificing performance

---

## Design Principles

### 1. **Efficiency First**
- Protobuf base encoding
- Diff-based state updates
- Custom codecs for LLM-specific data (tokens, embeddings)
- Compression-friendly structure (zstd/lz4)

### 2. **Streaming-Native**
- Stateless message design (each message self-contained)
- Ordered sequences via message IDs
- Out-of-order tolerance
- Replayable from any checkpoint

### 3. **AI Workflow Optimized**
- Token-level streaming for LLM responses
- Embedding compression (quantization)
- Graph state diffs
- Tool call/response tracking

### 4. **Production-Ready**
- Schema evolution (protobuf)
- Backwards compatibility
- Multi-tenant isolation (tenant_id)
- Security (optional encryption)

---

## Message Architecture

### Core Message Types

```
DashStreamMessage
├── Header (metadata, routing)
├── Event (lifecycle events)
├── StateDiff (graph state changes)
├── TokenChunk (LLM streaming)
├── ToolExecution (tool calls)
├── Checkpoint (state snapshots)
├── Metrics (performance data)
└── Error (failures)
```

### Message Flow

```
[Client] → [Kafka Topic: dashstream.events.{tenant_id}]
           → [Consumer: Processor/Debugger/Storage]
           → [Kafka Topic: dashstream.processed]
           → [Dashboard/Analytics]
```

---

## Protocol Buffer Schema

### Header (Common to All Messages)

```protobuf
message Header {
  // Unique message ID (UUID)
  bytes message_id = 1;

  // Timestamp (microseconds since epoch)
  int64 timestamp_us = 2;

  // Tenant/organization ID
  string tenant_id = 3;

  // Session/thread ID
  string thread_id = 4;

  // Sequence number (ordered within thread)
  uint64 sequence = 5;

  // Message type
  MessageType type = 6;

  // Optional: Parent message (for causality)
  bytes parent_id = 7;

  // Optional: Compression (zstd, lz4, none)
  CompressionType compression = 8;

  // Optional: Schema version (for evolution)
  uint32 schema_version = 9;
}

enum MessageType {
  MESSAGE_TYPE_UNSPECIFIED = 0;
  MESSAGE_TYPE_EVENT = 1;
  MESSAGE_TYPE_STATE_DIFF = 2;
  MESSAGE_TYPE_TOKEN_CHUNK = 3;
  MESSAGE_TYPE_TOOL_EXECUTION = 4;
  MESSAGE_TYPE_CHECKPOINT = 5;
  MESSAGE_TYPE_METRICS = 6;
  MESSAGE_TYPE_ERROR = 7;
  MESSAGE_TYPE_EVENT_BATCH = 8;
}

enum CompressionType {
  COMPRESSION_NONE = 0;
  COMPRESSION_ZSTD = 1;
  COMPRESSION_LZ4 = 2;
  COMPRESSION_SNAPPY = 3;
}
```

**Transport framing note:** Kafka payloads produced by the Rust implementation include a
1‑byte compression header before the protobuf bytes:
- `0x00` = uncompressed protobuf
- `0x01` = zstd‑compressed protobuf
The protobuf `Header.compression` field mirrors the actual framing for cross‑language clients.

### Event Message

```protobuf
message Event {
  Header header = 1;

  EventType event_type = 2;
  string node_id = 3;  // Graph node (if applicable)

  // Optional: Event-specific data
  map<string, bytes> attributes = 4;
}

enum EventType {
  GRAPH_START = 0;
  GRAPH_END = 1;
  NODE_START = 2;
  NODE_END = 3;
  EDGE_TRAVERSAL = 4;
  LLM_START = 5;
  LLM_END = 6;
  TOOL_START = 7;
  TOOL_END = 8;
  CHECKPOINT_SAVE = 9;
  CHECKPOINT_LOAD = 10;

  // Intra-node telemetry (v1.1.0+)
  // These events are emitted DURING node execution, between NODE_START and NODE_END
  NODE_PROGRESS = 80;      // Progress updates: attributes["message", "percent"]
  NODE_THINKING = 81;      // LLM reasoning steps: attributes["thought", "step"]
  NODE_SUBSTEP = 82;       // Internal step completion: attributes["substep_name", "status"]
  NODE_WARNING = 83;       // Non-fatal warnings during execution
}
```

### StateDiff (Diff-Based State Updates)

```protobuf
message StateDiff {
  Header header = 1;

  // Base checkpoint ID (if incremental)
  bytes base_checkpoint_id = 2;

  // Operations (JSON Patch RFC 6902 style)
  repeated Operation operations = 3;

  // Full state hash (for validation)
  bytes state_hash = 4;
}

message Operation {
  OpType op = 1;
  string path = 2;  // JSON pointer (e.g., "/messages/0")
  bytes value = 3;  // Encoded value

  enum OpType {
    ADD = 0;
    REMOVE = 1;
    REPLACE = 2;
    MOVE = 3;
    COPY = 4;
    TEST = 5;  // Validation
  }
}
```

### TokenChunk (LLM Streaming)

```protobuf
message TokenChunk {
  Header header = 1;

  // LLM request ID
  string request_id = 2;

  // Token data (UTF-8)
  string text = 3;

  // Optional: Token IDs (for analysis)
  repeated uint32 token_ids = 4;

  // Optional: Logprobs
  repeated float logprobs = 5;

  // Chunk index
  uint32 chunk_index = 6;

  // Is final chunk?
  bool is_final = 7;

  // Optional: Finish reason
  FinishReason finish_reason = 8;

  enum FinishReason {
    NONE = 0;
    STOP = 1;
    LENGTH = 2;
    CONTENT_FILTER = 3;
    TOOL_CALLS = 4;
  }
}
```

### ToolExecution

```protobuf
message ToolExecution {
  Header header = 1;

  // Tool call ID
  string call_id = 2;

  // Tool name
  string tool_name = 3;

  // Stage
  ExecutionStage stage = 4;

  // Arguments (JSON encoded)
  bytes arguments = 5;

  // Result (if stage == COMPLETED)
  bytes result = 6;

  // Error (if stage == FAILED)
  string error = 7;

  // Duration (microseconds)
  int64 duration_us = 8;

  enum ExecutionStage {
    REQUESTED = 0;
    STARTED = 1;
    COMPLETED = 2;
    FAILED = 3;
  }
}
```

### Checkpoint (State Snapshot)

```protobuf
message Checkpoint {
  Header header = 1;

  // Checkpoint ID
  bytes checkpoint_id = 2;

  // Full state (compressed)
  bytes state = 3;

  // State type hint
  string state_type = 4;

  // Checksum (for validation)
  bytes checksum = 5;

  // Storage location (if externalized)
  string storage_uri = 6;
}
```

### Metrics (Performance Data)

```protobuf
message Metrics {
  Header header = 1;

  // Scope (graph, node, llm, tool)
  string scope = 2;

  // Metrics
  map<string, MetricValue> metrics = 3;
}

message MetricValue {
  oneof value {
    int64 int_value = 1;
    double float_value = 2;
    string string_value = 3;
    bool bool_value = 4;
  }

  // Unit (seconds, bytes, count, etc.)
  string unit = 5;
}

// Common metrics:
// - duration_us: Execution time
// - tokens_used: LLM token count
// - cost_usd: Estimated cost
// - memory_bytes: Memory usage
// - cache_hit: Cache utilization
```

### Error

```protobuf
message Error {
  Header header = 1;

  // Error code
  string error_code = 2;

  // Error message
  string message = 3;

  // Stack trace (optional)
  string stack_trace = 4;

  // Error context
  map<string, string> context = 5;

  // Severity
  Severity severity = 6;

  enum Severity {
    INFO = 0;
    WARNING = 1;
    ERROR = 2;
    FATAL = 3;
  }
}
```

---

## Efficiency Optimizations

### 1. Diff-Based State Updates

**Problem:** Full state can be large (MBs for complex workflows)

**Solution:** Only transmit changes

```rust
// Before: 1MB state
Checkpoint { state: <1MB binary> }

// After: ~1KB diff
StateDiff {
  base_checkpoint_id: <previous>,
  operations: [
    { op: ADD, path: "/messages/-", value: <200 bytes> }
  ]
}
```

**Savings:** 99% reduction for incremental updates

### 2. Token-Level Streaming

**Problem:** Buffering full responses increases latency

**Solution:** Stream tokens as they arrive

```rust
// OpenAI SSE → DashFlow Streaming TokenChunk
TokenChunk {
  text: "Hello",
  chunk_index: 0,
  is_final: false,
}
TokenChunk {
  text: " world",
  chunk_index: 1,
  is_final: true,
  finish_reason: STOP,
}
```

**Benefit:** Sub-100ms latency, real-time streaming

### 3. Embedding Compression

**Problem:** Embeddings are large (1536 floats = 6KB)

**Solution:** Quantization + compression

```rust
// Original: 1536 × 4 bytes = 6KB
vec![0.123456, -0.234567, ...]

// Quantized: 1536 × 1 byte = 1.5KB
vec![127, -128, ...]  // int8 quantization

// Compressed: ~500 bytes (zstd)
```

**Savings:** 90%+ reduction

### 4. Batch Encoding

**Problem:** High message overhead for small events

**Solution:** Batch multiple events

```protobuf
message EventBatch {
  Header header = 1;
  repeated Event events = 2;  // Multiple events in one message
}
```

**Benefit:** Reduced Kafka overhead, better throughput

---

## Kafka Integration

### Topic Strategy

```
dashstream.events.{tenant_id}       # Raw events
dashstream.state.{tenant_id}        # State updates
dashstream.checkpoints.{tenant_id}  # Checkpoint snapshots
dashstream.metrics.{tenant_id}      # Performance metrics
dashstream.errors.{tenant_id}       # Errors
```

### Message Key

```rust
// Key by thread_id for ordered processing
key = thread_id.as_bytes()

// Partitioning: Hash(thread_id) % partition_count
```

### Retention Policy

- Events: 7 days (configurable)
- Checkpoints: 30 days
- Metrics: 90 days (aggregated)
- Errors: 90 days

### Compaction

Enable log compaction for state topics:
- Keep only latest state per thread_id
- Automatic cleanup of old states

---

## Introspection & Debugging Tools

### 1. DashFlow Streaming Inspector (CLI)

```bash
# Tail live events
dashflow tail --topic dashstream.events.my-tenant

# Inspect specific thread
dashflow inspect --thread session-123

# Replay execution
dashflow timeline replay --thread session-123 --from-checkpoint abc123

# Diff two checkpoints
dashflow diff --thread session-123 --checkpoint1 abc123 --checkpoint2 def456

# Export to JSON (for external tools)
dashflow export --thread session-123 --format json > output.json
```

### 2. Time-Travel Debugger

```rust
// Load any checkpoint
let checkpoint = dashstream.load_checkpoint(checkpoint_id).await?;

// Replay from checkpoint
let replay = dashstream.replay(checkpoint_id).await?;

// Step through execution
while let Some(event) = replay.next().await {
    println!("Event: {:?}", event);
}
```

### 3. Flamegraph Generator

```bash
# Generate execution flamegraph
dashflow flamegraph --thread session-123 --output graph.svg
```

### 4. Cost Analysis

```bash
# Calculate costs
dashflow costs --thread session-123

# Output:
# GPT-4o: 1.2M tokens × $5/M = $6.00
# Claude: 0.5M tokens × $15/M = $7.50
# Total: $13.50
```

### 5. Performance Profiler

```bash
# Profile node execution
dashflow profile --thread session-123

# Output:
# Node           | Calls | Total Time | Avg Time
# researcher     | 3     | 45.2s      | 15.1s
# writer         | 1     | 12.3s      | 12.3s
# critic         | 2     | 8.7s       | 4.4s
```

---

## Comparison to Alternatives

### vs JSON

| Metric | JSON | DashFlow Streaming | Improvement |
|--------|------|------------|-------------|
| Message Size | 10 KB | 500 bytes | 20× |
| Serialization Time | 500 μs | 50 μs | 10× |
| Parse Time | 800 μs | 80 μs | 10× |
| Streaming Support | ❌ | ✅ | N/A |
| Schema Validation | ❌ | ✅ | N/A |

### vs OpenTelemetry

| Feature | OTEL | DashFlow Streaming | Winner |
|---------|------|------------|--------|
| LLM-Optimized | ❌ | ✅ | DashFlow Streaming |
| Diff-Based | ❌ | ✅ | DashFlow Streaming |
| Token Streaming | ❌ | ✅ | DashFlow Streaming |
| Ecosystem | ✅✅ | ❌ | OTEL |
| Complexity | High | Medium | DashFlow Streaming |

**Conclusion:** Use DashFlow Streaming for LLM workflows, OTEL for general observability

---

## Implementation Roadmap

### Phase 1: Core Protocol (Week 1)
- [x] Protobuf schema definition
- [ ] Rust codec implementation
- [ ] Basic Kafka producer
- [ ] Unit tests

### Phase 2: Streaming & Diff (Week 2)
- [ ] Diff-based state updates
- [ ] Token-level streaming
- [ ] Compression (zstd)
- [ ] Integration tests

### Phase 3: Tools & Debugging (Week 3)
- [ ] CLI inspector
- [ ] Time-travel debugger
- [ ] Flamegraph generator
- [ ] Cost analysis tool

### Phase 4: Production Hardening (Week 4)
- [ ] Schema evolution testing
- [ ] Multi-tenant isolation
- [ ] Security (encryption)
- [ ] Performance benchmarks

### Phase 5: Integration (Week 5)
- [ ] DashFlow integration
- [ ] DashFlow callbacks
- [ ] Kafka Streams processors
- [ ] Grafana dashboards

---

## Security Considerations

### 1. Encryption
- **At Rest:** Kafka encryption (SSL/TLS)
- **In Transit:** TLS 1.3
- **Optional:** Field-level encryption for sensitive data

### 2. Authentication
- Kafka SASL/SCRAM
- OAuth2 for API access
- mTLS for service-to-service

### 3. Authorization
- Topic-level ACLs
- Tenant isolation via topics
- RBAC for debugging tools

### 4. PII Protection
- Redaction of sensitive fields
- Opt-in PII logging
- GDPR compliance (data retention)

---

## Performance Benchmarks (Projected)

### Throughput
- **Kafka Produce:** 100K messages/sec (single producer)
- **Kafka Consume:** 500K messages/sec (consumer group)
- **Codec:** 1M encode/decode ops/sec

### Latency
- **E2E Latency:** <10ms (p99)
- **Serialization:** <100μs
- **Deserialization:** <100μs

### Size
- **Event:** ~200 bytes (vs 2KB JSON)
- **State Diff:** ~1KB (vs 100KB full state)
- **Token Chunk:** ~50 bytes (vs 500 bytes JSON)

### Compression
- **ZSTD Ratio:** 5:1 (average)
- **LZ4 Ratio:** 3:1 (faster, less compression)

---

## Example Usage

### Producer (Rust)

```rust
use dashflow_streaming::{DashStreamProducer, Event, TokenChunk};
use dashflow_streaming::kafka::KafkaConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config = KafkaConfig {
        brokers: vec!["localhost:9092"],
        topic: "dashstream.events.my-tenant",
    };

    let producer = DashStreamProducer::new(config).await?;

    // Send event
    producer.send_event(Event {
        event_type: EventType::NodeStart,
        node_id: "researcher",
        ..Default::default()
    }).await?;

    // Stream tokens
    for token in llm_stream {
        producer.send_token_chunk(TokenChunk {
            text: token,
            ..Default::default()
        }).await?;
    }

    Ok(())
}
```

### Consumer (Rust)

```rust
use dashflow_streaming::{DashStreamConsumer, Message};

#[tokio::main]
async fn main() -> Result<()> {
    let consumer = DashStreamConsumer::new(config).await?;

    while let Some(message) = consumer.next().await {
        match message {
            Message::Event(event) => {
                println!("Event: {:?}", event);
            }
            Message::TokenChunk(chunk) => {
                print!("{}", chunk.text);
            }
            Message::StateDiff(diff) => {
                // Apply diff to local state
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## Intra-Node Streaming Telemetry

**Added in:** v1.1.0 (November 2025)
**Status:** Production-ready

### Overview

Traditional graph execution provides visibility at the node boundaries (NodeStart/NodeEnd) but offers no insight into what happens during node execution. For long-running operations, LLM reasoning, or complex multi-step processes, this creates a "black box" problem.

Intra-node streaming telemetry solves this by allowing nodes to emit telemetry **during execution**, providing real-time visibility into:

- Progress updates for long-running operations
- LLM chain-of-thought reasoning steps
- Internal substep completion
- Token-by-token LLM response generation
- Tool call stages (requested → started → completed)
- Custom metrics during execution
- Non-fatal warnings

### Node Execution Timeline

**Traditional (Black Box):**
```
NodeStart (T0)
    ↓
    [NO VISIBILITY - 2 seconds]
    ↓
NodeEnd (T0+2000ms)
```

**With Intra-Node Telemetry:**
```
NodeStart (T0)
    ↓
    NodeProgress("Analyzing query...", 10%) (T0+100ms)
    NodeThinking("User wants X", step=1) (T0+200ms)
    NodeProgress("Calling LLM...", 50%) (T0+500ms)
    TokenChunk("The", idx=0) (T0+1000ms)
    TokenChunk(" answer", idx=1) (T0+1100ms)
    NodeProgress("Validating...", 90%) (T0+1800ms)
    ↓
NodeEnd (T0+2000ms)
```

### Event Types

#### NODE_PROGRESS (80)
**Purpose:** Track completion percentage during long-running operations

**Attributes:**
- `message` (string): Human-readable progress description
- `percent` (float): Completion percentage (0.0 to 1.0)

**Example:**
```rust
ctx.send_progress("Processing 500/1000 documents", 0.5).await?;
```

**Use cases:**
- Document processing pipelines
- Large-scale data transformations
- Multi-step search operations
- Batch LLM inference

#### NODE_THINKING (81)
**Purpose:** Capture LLM chain-of-thought reasoning steps

**Attributes:**
- `thought` (string): The reasoning step or thought
- `step` (uint32): Step number in reasoning chain

**Example:**
```rust
ctx.send_thinking("User query requires search before answering", 1).await?;
ctx.send_thinking("Identified 3 relevant search sources", 2).await?;
```

**Use cases:**
- LLM agents with explicit reasoning
- Multi-step planning
- Decision-making processes
- ReAct-style agents

#### NODE_SUBSTEP (82)
**Purpose:** Track completion of internal operations within a node

**Attributes:**
- `substep_name` (string): Name of the substep
- `status` (string): Status (e.g., "started", "complete", "failed")

**Example:**
```rust
ctx.send_substep("validate_input", "complete").await?;
ctx.send_substep("call_external_api", "started").await?;
```

**Use cases:**
- Complex nodes with multiple internal stages
- API call tracking
- Validation steps
- Data transformation pipelines

#### NODE_WARNING (83)
**Purpose:** Emit non-fatal warnings during execution

**Attributes:**
- Custom attributes per warning type

**Use cases:**
- Degraded mode operation
- Fallback usage
- Recoverable errors
- Configuration issues

### Integration with Existing Messages

Intra-node telemetry works seamlessly with existing message types:

**TokenChunk**: Already supported, now accessible from nodes via `ctx.send_token()`
**ToolExecution**: Already supported, now accessible via `ctx.send_tool_event()`
**Metrics**: Already supported, now accessible via `ctx.send_metric()`
**Error**: Already supported for non-fatal errors via `ctx.send_error()`

### NodeContext API

Nodes emit telemetry through `NodeContext`, which provides:

**High-Level API** (Simple to use):
```rust
ctx.send_progress(message: &str, percent: f64) -> Result<()>
ctx.send_thinking(thought: &str, step: u32) -> Result<()>
ctx.send_substep(name: &str, status: &str) -> Result<()>
```

**Low-Level API** (Full protocol access):
```rust
ctx.send_token(text: &str, idx: u32, is_final: bool, request_id: &str) -> Result<()>
ctx.send_tool_event(call_id: &str, tool: &str, stage: i32, duration: i64) -> Result<()>
ctx.send_metric(name: &str, value: f64, unit: &str) -> Result<()>
ctx.send_error(code: &str, msg: &str, severity: i32) -> Result<()>
```

**Performance:**
- All sends are fire-and-forget (tokio::spawn)
- No blocking on Kafka producer
- < 0.01% overhead per message
- No-op mode when producer unavailable

### Backward Compatibility

The feature is **fully backward compatible**:

1. **Opt-In**: Nodes must implement `supports_streaming() -> bool` to enable
2. **Default Behavior**: Nodes without streaming continue to work unchanged
3. **No Producer**: If no DashStreamCallback attached, all sends are no-ops
4. **Feature Gated**: Entire system works without `dashstream` feature

### Example: Streaming Search Agent

```rust
struct SearchAgentNode;

#[async_trait]
impl Node<State> for SearchAgentNode {
    fn supports_streaming(&self) -> bool { true }

    async fn execute_with_context(&self, state: State, ctx: &NodeContext) -> Result<State> {
        ctx.send_progress("Starting search", 0.1).await?;

        // Search Wikipedia
        ctx.send_substep("search_wikipedia", "started").await?;
        let wiki_results = search_wikipedia(&state.query).await?;
        ctx.send_tool_event("search_1", "wikipedia", STAGE_COMPLETE, 500_000).await?;
        ctx.send_progress("Wikipedia search complete", 0.5).await?;

        // Search ArXiv
        ctx.send_substep("search_arxiv", "started").await?;
        let arxiv_results = search_arxiv(&state.query).await?;
        ctx.send_tool_event("search_2", "arxiv", STAGE_COMPLETE, 800_000).await?;
        ctx.send_progress("ArXiv search complete", 1.0).await?;

        Ok(state.with_results(wiki_results, arxiv_results))
    }
}
```

### Observability Benefits

1. **Real-Time Monitoring**: See exactly what nodes are doing
2. **Debugging**: Identify where nodes get stuck or slow
3. **User Experience**: Show progress indicators in UIs
4. **Performance Analysis**: Track substep durations
5. **Error Diagnosis**: Understand failure context
6. **Cost Tracking**: Monitor LLM token usage in real-time

### Example: Running the Demo

```bash
# Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# Run streaming example
cargo run --example streaming_node --features dashstream

# Monitor telemetry
docker-compose -f docker-compose-kafka.yml exec kafka \
    kafka-console-consumer --bootstrap-server localhost:9092 \
    --topic dashstream-streaming-demo --from-beginning
```

**Output shows:**
- 4 NodeStart events
- 4 NodeEnd events
- ~15 NodeProgress events
- 3 NodeThinking events
- 6 NodeSubstep events
- 6 ToolExecution events
- ~60 TokenChunk events
- 5 Metrics events
- Multiple StateDiff events

### Implementation Details

**Location:** `crates/dashflow/src/node.rs`
**Protocol:** `proto/dashstream.proto` (EventType 80-83)
**Example:** `crates/dashflow/examples/streaming_node.rs`

**Key Components:**
1. `NodeContext` struct - Execution context with telemetry methods
2. `Node::execute_with_context()` - New trait method for streaming nodes
3. `Node::supports_streaming()` - Opt-in flag
4. `EventCallback::get_producer()` - Access to DashFlow Streaming producer
5. Executor integration - Context creation and node invocation

**Tests:** 19 tests covering all functionality (all passing)

---

## Future Enhancements

1. **Binary Diff Algorithm**
   - Custom diff for state (better than JSON Patch)
   - Efficient for large states

2. **Auto-Compression Detection**
   - Compress only if beneficial
   - Skip compression for small messages

3. **Schema Registry Integration**
   - Confluent Schema Registry
   - Automatic schema evolution

4. **GraphQL API**
   - Query interface for debugging
   - Real-time subscriptions

5. **Machine Learning Integration**
   - Anomaly detection
   - Cost prediction
   - Performance regression detection

---

## References

- Protobuf: https://protobuf.dev/
- Kafka: https://kafka.apache.org/
- JSON Patch (RFC 6902): https://tools.ietf.org/html/rfc6902
- ZSTD Compression: https://facebook.github.io/zstd/

---

**Status:** Design Complete - Ready for Implementation

**Next Steps:**
1. Review design with team
2. Create protobuf schema files
3. Implement Rust codec
4. Build CLI inspector
5. Integrate with DashFlow

**Author:** Andrew Yates © 2026
**License:** Proprietary - Internal Use Only
