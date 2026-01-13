# Categorized Issue Backlog (Section-by-Section)
Date: 2025-12-05
Author: Codex audit (granular list)

## Resilience / Recovery
- Telemetry send is single-shot: no retry/backoff on broker errors (`crates/dashflow-streaming/src/producer.rs:320-379`).
- DLQ send is also single-shot and fire-and-forget; any failure is permanent (`crates/dashflow-streaming/src/dlq.rs:166-196`).
- Producer shutdown lacks flush/await of in-flight sends; graceful shutdown can lose telemetry (all producer send paths).
- Resume uses only in-memory index; if index is stale/corrupt, newer checkpoint files are ignored on resume (`crates/dashflow/src/checkpoint.rs:474-489`).

## Security / Config Hardening
- Kafka ProducerConfig has no TLS/SASL fields; plaintext is default (`crates/dashflow-streaming/src/producer.rs:126-175`).
- Decoder accepts messages with missing schema headers after only a warning (`crates/dashflow-streaming/src/codec.rs:269-283`).
- Unknown/invalid header byte falls through to legacy decode without validation (`crates/dashflow-streaming/src/codec.rs:158-169`).
- Checkpoint temp filenames predictable (pid+timestamp) and not hardened against races (`crates/dashflow/src/checkpoint.rs:61-74`).

## Resource Safety / Backpressure
- Telemetry callbacks spawn unbounded tasks; no concurrency cap/queue (`crates/dashflow/src/dashstream_callback.rs:248-305,376-385`; `crates/dashflow/src/node.rs:355-584`).
- Custom stream channel is fixed-size; drops data on full buffer with only a warn, no backpressure/metrics (`crates/dashflow/src/stream.rs:12-35`).
- State diff path duplicates full states into JSON and retains previous state; large/high-frequency states can OOM (`crates/dashflow/src/dashstream_callback.rs:238-309`).
- Telemetry sequence uses blocking std::sync::Mutex in async hot path (`crates/dashflow-streaming/src/producer.rs:511-518`).

## Data Integrity / Validation
- Default decoder path omits schema validation; batches only warn on missing headers (`crates/dashflow-streaming/src/codec.rs:134-183,220-283`).
- Checkpoint blobs lack versioning/checksum; corruption/struct drift yield opaque failures (`crates/dashflow/src/checkpoint.rs:494-509`).
- Checkpoint index deserialization failure resets to empty silently, losing latest pointers (`crates/dashflow/src/checkpoint.rs:344-349`).
- State diff serialization uses `unwrap_or_default`, emitting empty diffs on error without alert (`crates/dashflow/src/dashstream_callback.rs:398-409`).
- RephraseQueryRetriever mocks panic on `.stream()`; GEPA optimizer stub panics (`crates/dashflow/src/core/retrievers/rephrase_query_retriever.rs:214,322,554,802`; `crates/dashflow/src/optimize/optimizers/gepa.rs:628`).

## Observability / Logging
- Telemetry failures only warn; application never notified of dropped observability (`crates/dashflow/src/dashstream_callback.rs:248-305,376-385`; `crates/dashflow/src/node.rs:355-584`).
- Codec header warnings use `eprintln!`, bypassing structured tracing/metrics (`crates/dashflow-streaming/src/codec.rs:269-283`).
- DLQ serialization/send errors lack source context (topic/offset) in logs (`crates/dashflow-streaming/src/dlq.rs:166-196`).
- Dropped custom stream events have no metric/counter, only a warn (`crates/dashflow/src/stream.rs:12-35`).

## Performance / Concurrency
- GRPO trace collection is sequential per thread_id; should gather concurrently (`crates/dashflow/src/optimize/optimizers/grpo.rs:324-336`).
- Telemetry sends are per-task spawns without batching; high-volume loads add scheduler overhead (`crates/dashflow/src/dashstream_callback.rs:248-385`).
- Compression threshold/level fixed (512 bytes, zstd level 3) with no tuning hooks for workload differences (`crates/dashflow-streaming/src/codec.rs:86-112`).
- Async checkpointer constructor uses blocking `std::fs::create_dir_all`, stalling runtime on slow disks (`crates/dashflow/src/checkpoint.rs:337-341`).
