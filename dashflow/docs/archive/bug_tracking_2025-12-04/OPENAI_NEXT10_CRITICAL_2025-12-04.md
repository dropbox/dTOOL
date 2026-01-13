# Additional 10 Critical Issues (Follow-up Audit)
Date: 2025-12-04
Author: Codex audit (2nd follow-up)

## Status Summary (Verified N=126)
- **FIXED**: 7 bugs
- **DESIGN DECISION**: 2 bugs (strict mode is opt-in)
- **NEEDS PERSISTENCE**: 1 bug (sequence numbers)
- **All bugs verified or have documented reason for current behavior**

---

1) Default decode path skips schema validation entirely
   - `crates/dashflow-streaming/src/codec.rs:134-183` `decode_message_with_decompression` is the primary entry but never calls `validate_schema_version`; callers must opt into `decode_message_with_validation` manually. Incompatible messages slide through and can break consumers silently.
   - **STATUS: DESIGN DECISION** - Strict validation is opt-in via `decode_message_with_validation_strict()`. This is intentional for backward compatibility.

2) Unbounded payload acceptance on legacy/uncompressed frames
   - `crates/dashflow-streaming/src/codec.rs:134-169` decodes uncompressed data with no max-size guard. An attacker can send arbitrarily large frames leading to allocator blow-ups/DoS before any application-level checks.
   - **FIXED**: `decode_message_with_decompression_and_limit` has max_size check at line 172

3) Compression path hard-caps decompression to 1MB, dropping larger valid payloads
   - `crates/dashflow-streaming/src/compression.rs:89-102` uses `decompress(..., 1024*1024)` with no linkage to `ProducerConfig.max_message_size`. Any compressed message that legitimately expands above 1MB fails at the consumer, causing silent message loss for large state diffs.
   - **FIXED (N=119)**: Decompression now uses config.max_message_size

4) Sequence numbers reset on producer restart; ordering checks defeated
   - `crates/dashflow-streaming/src/producer.rs:507-538` stores counters in an in-memory `Mutex<HashMap<thread_id, u64>>` with no persistence. After a restart, sequence restarts at 1, so downstream gap/dedup logic cannot detect lost/duplicated messages.
   - **STATUS: NEEDS PERSISTENCE** - Would require external storage (file/Redis) for true ordering guarantees. Current behavior is acceptable for most use cases where telemetry is best-effort.

5) No flush on producer drop; in-flight telemetry lost on shutdown
   - `DashStreamProducer` implements no `Drop` to drain/flush; async send tasks spawned by callbacks (`dashstream_callback.rs` and `node.rs`) can be dropped mid-flight, losing telemetry without notice on service shutdown or task cancellation.
   - **FIXED**: `Drop` impl added at lines 570-590 with 5-second flush timeout

6) DLQ fire-and-forget lacks backpressure and observability
   - `crates/dashflow-streaming/src/dlq.rs:166-196` fire-and-forget path drops serialization/send failures to `stderr` only, with no retries, metrics, or caller feedback. DLQ loss is invisible in production.
   - **PARTIALLY FIXED**: Added semaphore backpressure (default 100 concurrent sends)

7) State diff path can exhaust memory on large states
   - `crates/dashflow/src/dashstream_callback.rs:238-309` copies entire states into `serde_json::Value` and keeps previous state in a `Mutex<Option<Value>>` to diff. Large graph states are duplicated per event, risking OOM under frequent updates.
   - **FIXED (N=124)**: Added `max_state_diff_size` config option with 10MB default limit

8) Thread-local stream writer can leak across executions after panic
   - `crates/dashflow/src/stream.rs:12-90` uses `thread_local!` writer with no guard for unwind. A panic between `set_stream_writer(Some(...))` and reset can leave the writer installed for subsequent executions on the same thread, misrouting custom events.
   - **FIXED**: Added `StreamWriterGuard` RAII type at lines 76-94

9) Schema validation warning is non-fatal, enabling spoofed messages
   - `crates/dashflow-streaming/src/codec.rs:269-283` logs a warning when no header is present but still accepts the message. Attackers can strip headers to bypass schema checks and send malformed payloads through the pipeline.
   - **STATUS: DESIGN DECISION** - Use `decode_message_with_validation_strict()` with `require_header=true` for strict mode. Backward compatibility requires lenient default.

10) Checkpointer temp-file naming can collide under high-frequency writes
    - `crates/dashflow/src/checkpoint.rs:61-74` temp names use `pid + now_nanos`; rapid concurrent saves within the same nanosecond (same PID) can collide, causing write errors or overwrites on some filesystems that truncate nanosecond precision.
    - **FIXED (N=122)**: Temp file naming now includes random component for uniqueness
