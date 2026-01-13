# Next 20 Critical/High Issues (Additional Set)
Date: 2025-12-04
Author: Codex audit (additional pass)

1) No batching/flow control on telemetry senders
   - `crates/dashflow/src/dashstream_callback.rs:248-305,376-385` and `crates/dashflow/src/node.rs:355-584` spawn per-event sends with no queue or bounded concurrency; spikes can starve the runtime and drop messages.

2) Producer send path lacks idempotence on restart
   - `crates/dashflow-streaming/src/producer.rs:320-379` doesn’t set Kafka idempotence configs beyond `enable.idempotence`; sequence state is volatile, so restart may duplicate messages without consumer-side dedup.

3) Unchecked fallback to legacy decode
   - `crates/dashflow-streaming/src/codec.rs:158-169` treats unknown first byte as legacy uncompressed, risking deserialization panics on crafted payloads; should reject/validate magic byte.

4) Missing TLS/SASL config in ProducerConfig
   - `crates/dashflow-streaming/src/producer.rs:126-175` exposes only basic fields; secure clusters require code changes, leading to accidental plaintext deployments.

5) No telemetry for dropped custom stream events
   - `crates/dashflow/src/stream.rs:12-35` drops data on full channel with only a warn; lacks counter/metric so operators can’t see loss rates.

6) Inconsistent schema validation on batches vs single messages
   - `crates/dashflow-streaming/src/codec.rs:220-283` batch decode only warns on missing header; single-message validation optional. Mixed-version/invalid batches can infiltrate consumers.

7) Panic-prone mocks in rephrase_query_retriever
   - `crates/dashflow/src/core/retrievers/rephrase_query_retriever.rs:214,322,554,802` `unimplemented!()` in `.stream()` mocks; any inadvertent use in async scenarios causes test/bench panics.

8) Panic stub in GEPA optimizer API
   - `crates/dashflow/src/optimize/optimizers/gepa.rs:628` explicit `unimplemented!()` in public-facing function; misuse crashes caller.

9) No checksum/version on serialized checkpoints
   - `crates/dashflow/src/checkpoint.rs:494-509` bincode blobs lack versioning/checksum; silent corruption or struct drift will surface as opaque errors or undefined state.

10) Stale index risk after crash despite atomic write
    - `crates/dashflow/src/checkpoint.rs:378-382` atomic write helps, but in-memory index remains single-source; crash between checkpoint write and index write loses newest checkpoint pointer with no repair scan.

11) Graph resume ignores on-disk newer checkpoints
    - `crates/dashflow/src/checkpoint.rs:474-489` uses index only; if index corrupt/outdated, resume won’t scan files to recover latest state.

12) Telemetry error paths don’t include node/task context in DLQ
    - `crates/dashflow-streaming/src/dlq.rs:166-196` logs failures without source context, complicating triage and correlation.

13) Producer timeout config defaults to 30s; no per-call override
    - `crates/dashflow-streaming/src/producer.rs:126-175` timeout fixed in config; callers can’t set per-message timeouts, leading to head-of-line blocking under broker slowdown.

14) State diff serialization uses unwrap defaults (data loss)
    - `crates/dashflow/src/dashstream_callback.rs:398-409` `unwrap_or_default` on `patch_to_proto`/`to_vec`; emits empty diffs without raising, losing auditability.

15) Async constructor for checkpointer uses blocking fs in hot path
    - `crates/dashflow/src/checkpoint.rs:337-341` `std::fs::create_dir_all` runs in constructors used in async contexts, blocking executor threads on slow disks.

16) No backoff/retry for DLQ sends
    - `crates/dashflow-streaming/src/dlq.rs:166-196` single attempt; transient broker issues drop DLQ messages permanently.

17) Thread-local stream writer not cleared on panic/unwind
    - `crates/dashflow/src/stream.rs:12-90` set/unset with no guard; panic can leak writer into later executions, misrouting events.

18) Missing structured logging in codec warnings
    - `crates/dashflow-streaming/src/codec.rs:269-283` uses `eprintln!`, bypassing tracing/metrics; malformed traffic yields unstructured stderr noise.

19) Telemetry send lacks spanning context propagation for DLQ
    - `crates/dashflow-streaming/src/dlq.rs:166-196` does not propagate trace headers; DLQ analysis loses trace linkage to the failing execution.

20) No configurable compression level/threshold for streaming
    - `crates/dashflow-streaming/src/codec.rs:86-112` compress threshold fixed at 512 bytes and level fixed; cannot tune for throughput vs CPU, leading to suboptimal performance in different workloads.
