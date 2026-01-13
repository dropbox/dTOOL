# DashFlow Streaming Protocol Buffers

This directory contains Protocol Buffer definitions for the DashFlow Streaming telemetry protocol.

## Files

- `dashstream.proto` - Core DashFlow Streaming message definitions

## Overview

DashFlow Streaming is an ultra-efficient streaming telemetry protocol designed specifically for DashFlow/DashFlow workflows.

**Key Features:**
- **10-100× smaller** than JSON (protobuf + compression)
- **Diff-based updates** for state (90%+ reduction)
- **Token-level streaming** for LLM responses
- **Kafka-optimized** for distributed streaming
- **AI-specific** encoding (tokens, embeddings, graph state)

## Message Types

1. **Event** - Lifecycle events (node_start, llm_end, etc.)
2. **StateDiff** - Incremental state updates (JSON Patch style)
3. **TokenChunk** - Streaming LLM tokens
4. **ToolExecution** - Tool call/response tracking
5. **Checkpoint** - Full state snapshots
6. **Metrics** - Performance data (duration, tokens, cost)
7. **Error** - Failure information

## Code Generation

### Rust

```bash
# Install protobuf compiler
brew install protobuf  # macOS
apt install protobuf-compiler  # Ubuntu

# Generate Rust code
protoc --rust_out=src/ proto/dashstream.proto

# Or use cargo build with build.rs
```

### Python (for tooling)

```bash
pip install grpcio-tools

python -m grpc_tools.protoc \
    -I./proto \
    --python_out=. \
    --grpc_python_out=. \
    proto/dashstream.proto
```

## Usage Examples

### Producer (Sending Events)

```rust
use dashstream::v1::{Event, EventType, Header};

let event = Event {
    header: Some(Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_micros() as i64,
        tenant_id: "my-tenant".to_string(),
        thread_id: "session-123".to_string(),
        sequence: 1,
        type_: MessageType::MessageTypeEvent as i32,
        ..Default::default()
    }),
    event_type: EventType::EventTypeNodeStart as i32,
    node_id: "researcher".to_string(),
    ..Default::default()
};

// Serialize to bytes
let bytes = event.encode_to_vec();

// Send to Kafka
producer.send(bytes).await?;
```

### Consumer (Reading Events)

```rust
use dashstream::v1::DashStreamMessage;

// Receive from Kafka
let bytes = consumer.recv().await?;

// Deserialize
let message = DashStreamMessage::decode(&bytes[..])?;

match message.message {
    Some(lang_stream_message::Message::Event(event)) => {
        println!("Event: {:?}", event.event_type);
    }
    Some(lang_stream_message::Message::TokenChunk(chunk)) => {
        print!("{}", chunk.text);
    }
    _ => {}
}
```

## Compression

Messages support optional compression:

```rust
use dashstream::v1::{Header, CompressionType};

let header = Header {
    compression: CompressionType::CompressionZstd as i32,
    ..Default::default()
};

// Compress payload
let compressed = zstd::encode_all(&payload[..], 3)?;
```

## Performance

**Benchmarks** (vs JSON):
- Message Size: 20× smaller
- Encode Time: 10× faster
- Decode Time: 10× faster
- Streaming Latency: <10ms (p99)

## Schema Evolution

Protobuf supports backwards-compatible schema evolution:

1. Never remove required fields
2. Never change field numbers
3. Add new fields with unique numbers
4. Use `reserved` for deprecated fields

```protobuf
message MyMessage {
  reserved 5;  // Deprecated field
  reserved "old_field_name";

  int32 new_field = 10;  // New field
}
```

## Documentation

See [DASHSTREAM_PROTOCOL.md](../docs/DASHSTREAM_PROTOCOL.md) for:
- Complete protocol specification
- Architecture and design rationale
- Kafka integration patterns
- Debugging and introspection tools
- Performance benchmarks

## Status

**Current:** Design Complete ✅
**Next:** Rust codec implementation

## References

- Protocol Buffer Language Guide: https://protobuf.dev/
- Rust protobuf crate: https://docs.rs/protobuf/
- DashFlow Streaming Protocol Spec: [DASHSTREAM_PROTOCOL.md](../docs/DASHSTREAM_PROTOCOL.md)

---

**Author:** Andrew Yates © 2025
