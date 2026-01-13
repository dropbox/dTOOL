# v107 Skeptical Audit: consumer/mod.rs

**Auditor:** Worker #1808
**Date:** 2025-12-25
**Files Audited:**
- `crates/dashflow-streaming/src/consumer/mod.rs` (1975 lines)
- `crates/dashflow-streaming/src/consumer/tests.rs` (1191 lines)

## Executive Summary

**Result: CLEAN (No P1/P2/P3 issues)**

The consumer module is production-quality code with comprehensive validation, memory-safe pruning, atomic checkpoints, and good test coverage (~56 unit tests).

## Module Overview

`DashStreamConsumer` is a Kafka consumer for DashFlow streaming that provides:
- Async message consumption via rskafka `PartitionClient`
- Sequence validation for gap/duplicate/reorder detection
- Dead Letter Queue (DLQ) for malformed messages
- Local offset checkpointing (atomic file writes)
- TLS/SASL authentication support
- Prometheus metrics integration
- Configurable exponential backoff for fetch failures

## Positive Findings (Production Quality)

### 1. Memory Management (M-514 Pattern)
The `SequenceValidator` has proper LRU-style pruning:
- `MAX_TRACKED_THREADS` = 100,000
- `PRUNE_BATCH` = 1,000 entries per prune cycle
- `MAX_PRUNED_THREADS` = 10,000 (capped to prevent unbounded growth)
- Tracks pruned threads to avoid false gap detection on reappearance

```rust
// Line 386-431: prune_state() implementation
// - Removes oldest entries to cap memory
// - Records pruned threads for graceful reappearance handling
// - Logs warning with memory stats
```

### 2. Atomic Offset Checkpoints
`store_offset_checkpoint_atomic()` uses temp file + rename pattern (line 866-943):
- Creates temp file with unique name (PID + nonce)
- Writes + fsync
- Atomic rename to target
- Directory fsync on Unix for durability

### 3. Comprehensive Config Validation
`DashStreamConsumer::with_config()` validates (lines 976-1022):
- partition >= 0
- Non-empty bootstrap_servers, topic, group_id
- max_message_size > 0
- sasl_username and sasl_password must both be set or both unset
- ssl_certificate_location and ssl_key_location must be paired
- dlq_topic must be non-empty when enable_dlq=true

### 4. Offset Bounds Clamping
Checkpoint offsets are clamped to broker-reported `[earliest, latest]` range (line 1145):
```rust
let clamped = checkpoint.offset.clamp(earliest_offset, latest_offset);
```
This handles retention/compaction correctly.

### 5. Exponential Backoff
Fetch failures use configurable backoff (lines 1330-1347):
- Initial: `fetch_backoff_initial` (default 100ms)
- Max: `fetch_backoff_max` (default 5s)
- Doubles on each consecutive failure
- Resets on any successful fetch

### 6. Health Check
`health_check()` method (lines 1875-1883) validates broker connectivity by fetching latest offset.

### 7. Test Coverage
56+ unit tests covering:
- ConsumerConfig defaults and custom values
- Offset checkpoint round-trip (JSON and legacy integer formats)
- SequenceValidator gap/duplicate/reorder detection
- GapRecoveryPolicy variants (Continue, Halt, WarnAndContinue)
- M-514 pruned thread reappearance
- Decompression with size limits

## Issues Found

### P4 - M-1043: Misleading comment about "oldest" pruning
**Category:** Documentation/Cosmetic
**File:** `consumer/mod.rs:405`

The comment says "forget oldest pruned threads" but `HashSet` iteration order is arbitrary:
```rust
// M-514: Cap pruned_threads to prevent unbounded growth
// If we exceed the limit, forget oldest pruned threads (they'll get false gap detection,
// but this is better than OOM). HashSet iteration order is arbitrary, which is acceptable.
```

**Impact:** None - the comment acknowledges the arbitrary order is acceptable. This is purely cosmetic.

**Verdict:** No action needed. The code correctly handles memory pressure with random eviction.

### P4 - M-1044: Test coverage gap for edge case
**Category:** Testing
**File:** `consumer/tests.rs`

No explicit test for empty checkpoint file (only whitespace) - though the code handles it at line 823.

**Impact:** Very low - code path is covered, just not explicitly tested.

**Verdict:** Optional improvement.

## Security Review

1. **TLS Configuration:** Uses rustls 0.21 with proper certificate loading and mutual TLS support.

2. **SASL PLAIN:** Credentials are passed securely to rskafka and rdkafka clients.

3. **DLQ Truncation:** Large payloads are truncated to 512KB with SHA256 hash for forensics (in dlq.rs).

4. **Schema Validation:** Strict mode rejects unknown header bytes (security by default).

5. **Size Limits:** `max_message_size` is enforced before decompression to prevent bombs.

## Metrics

The consumer exports these Prometheus metrics:
- `dashstream_messages_received_total`
- `dashstream_decode_failures_total`
- `dashstream_fetch_failures_total`
- `dashstream_invalid_payloads_total`
- `dashstream_sequence_gaps_total`
- `dashstream_sequence_duplicates_total`
- `dashstream_sequence_reorders_total`
- `dashstream_sequence_gap_size` (histogram)
- `dashstream_offset_checkpoint_writes_total`
- `dashstream_offset_checkpoint_failures_total`

## Recommendations

1. **Optional:** Add explicit test for empty checkpoint file content
2. **Optional:** Clarify the "oldest" comment in pruned_threads eviction

## Conclusion

The consumer module is well-architected production code. No P1/P2/P3 issues found. The M-514 memory management pattern is correctly implemented. Test coverage is comprehensive. Security considerations are properly addressed.

**Status:** CLEAN AUDIT
