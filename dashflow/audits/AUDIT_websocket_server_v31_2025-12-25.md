# DashFlow v31 Skeptical Audit: WebSocket Server - Replay Timeout Logic
**Auditor:** Worker #1707
**Date:** 2025-12-25 (updated 2026-01-01)
**Scope:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs` (replay timeout logic)
**Previous Audit:** v30 (DashStream graph state streaming telemetry)

> **⚠️ STALE LINE REFERENCES:** This audit's line numbers were updated 2026-01-01 to match current code (1579 lines vs original 1156). Both M-809 and M-810 have been FIXED.

---

## Executive Summary

This audit examined the WebSocket server's replay timeout protection mechanism (M-743). A **P2 bug** was discovered where the timeout has **no effect** due to incorrect async placement - the future is `.await`ed BEFORE being wrapped in the timeout.

| ID | Priority | Status | Description |
|----|----------|--------|-------------|
| M-809 | **P2** | ✅ **FIXED** | Replay timeout broken - `.await` executes BEFORE timeout wrapper (fixed: lines 558-574) |
| M-810 | P3 | ✅ **FIXED** | Thread-mode resume doesn't send `replay_complete` message (fixed: lines 1512-1528) |

---

## Findings

### M-809 (P2): Replay Timeout is Completely Non-Functional - ✅ FIXED

**Original Location:** `handlers.rs:427-430` (now at lines 558-574)

**Original Problematic Code:**
```rust
let replay_future = handle_resume_message(&msg, &state, &mut socket, replay_start).await;
// ^^^^^^^^^^^^^^ BUG: The .await HERE means the replay is ALREADY DONE

match tokio::time::timeout(Duration::from_secs(replay_timeout_secs), async { replay_future }).await {
    // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ This timeout wraps an IMMEDIATE value, not a future!
```

**Issue:** The `handle_resume_message` function was fully `.await`ed on line 427, completing the entire replay operation BEFORE the timeout was applied.

**Current Fixed Code (lines 558-574):**
```rust
// Previously: `handle_resume_message(...).await` was completed BEFORE timeout,
// which meant timeout had no effect. Fixed: now timeout wraps the actual future.
match tokio::time::timeout(
    Duration::from_secs(replay_timeout_secs),
    handle_resume_message(&msg, &state, &mut socket, replay_start)
).await {
    // ... properly handles timeout, success, and error cases
}
```

**Status:** ✅ FIXED - Timeout now correctly wraps the async operation.

---

### M-810 (P3): Thread-Mode Resume Missing `replay_complete` Message - ✅ FIXED

**Original Location:** `handlers.rs:1007-1155` vs `handlers.rs:976-997` (now at lines 1307-1528)

**Issue:** The `handle_thread_mode_resume` function originally returned `Ok(())` directly without sending a `replay_complete` JSON message to the client.

**Current Fixed Code (lines 1512-1528):**
```rust
// M-810: Send replay_complete for thread-mode resume (consistent with partition mode).
// This allows UI to know when replay is finished and switch to live processing.
let replay_complete = serde_json::json!({
    "type": "replay_complete",
    "totalReplayed": total_replayed,
    "mode": "thread",
});
// M-1009: Use timed send
if let Err(reason) = send_with_timeout(
    socket,
    Message::Text(replay_complete.to_string()),
    &state.metrics,
)
.await
{
    eprintln!("   Failed to send thread-mode replay_complete: {}", reason);
    return Err("replay_complete_failed");
}
```

**Status:** ✅ FIXED - Thread mode now sends `replay_complete` consistent with partition mode.

---

## Test Cases

**M-809 Validation:** To confirm the timeout is broken, add the following test or debugging:
1. Set `REPLAY_TIMEOUT_SECS=1`
2. Trigger a resume request with a large replay that takes >1 second
3. Observe that the replay completes without timeout error

The fix can be validated by the same test - after fix, large replays should timeout after 1 second.

---

## Files Audited

| File | Lines (original → current) | Issues Found |
|------|---------------------------|--------------|
| `handlers.rs` | 1156 → 1579 | M-809 (P2) ✅, M-810 (P3) ✅ |
| `main.rs` | 2900+ | (reviewed, no new issues) |
| `replay_buffer.rs` | 400+ | (reviewed, no new issues) |

---

## Audit Methodology

1. Focused on M-743 replay timeout since it's a recent feature meant to protect against slow clients
2. Read the timeout implementation in handlers.rs
3. Identified that `.await` placement breaks the timeout completely
4. Cross-referenced with partition-mode replay to find thread-mode omission

---

## Recommendations

1. ~~**Fix M-809 immediately (P2)** - Remove the `.await` so the timeout wraps the actual async operation~~ ✅ DONE
2. ~~**Fix M-810 (P3)** - Add `replay_complete` message to thread-mode resume for protocol consistency~~ ✅ DONE
3. **Add integration test** - Test that replay timeout actually triggers for slow operations (optional)

---

## Change Log

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-25 | Worker #1707 | Initial v31 audit |
| 1.1 | 2026-01-01 | Worker #2247 | Updated line refs (1156→1579 lines); marked M-809, M-810 as FIXED |
