# Audit: rate_limiter.rs v109 (2025-12-25)

**File:** `crates/dashflow-streaming/src/rate_limiter.rs`
**Lines:** 628
**Auditor:** Worker #1810

## Summary

Per-tenant rate limiting using token bucket algorithm with both in-memory and Redis-backed distributed modes. Overall code quality is **GOOD** with proper defensive coding, fallback handling, and metric cardinality protection.

**Status:** 0 P0 | 0 P1 | 0 P2 | 0 P3 | 2 P4 noted (no fixes required)

---

## Issues Found

### M-1053 (P4): available_tokens uses arbitrary pruning, not LRU - NOTED

**Category:** Consistency

**Location:** Lines 483-492

**Observation:** `available_tokens()` uses arbitrary `take(PRUNE_BATCH)` for eviction, while `check_rate_limit_local()` (lines 356-366) uses proper LRU eviction sorted by `last_access`. This inconsistency is acceptable because:
- `available_tokens()` is a monitoring method called infrequently
- The hot path (`check_rate_limit_local`) correctly uses LRU

**Action:** None required - design tradeoff documented.

---

### M-1054 (P4): Redis key not sanitized for special characters - NOTED

**Category:** Hygiene

**Location:** Line 389

**Observation:** `format!("rate_limit:{}:bucket", tenant_id)` uses tenant_id directly in Redis key without sanitization. While Redis keys can contain any binary data safely (no command injection risk), applying the same SHA256 sanitization used for metric labels (lines 88-99) would be more consistent.

**Action:** None required - no security impact, but could be improved for consistency in a future cleanup.

---

## Positive Findings

1. **Tenant label sanitization** (lines 72-99): Unsafe tenant labels are hashed with SHA256 to create safe `tenant_XXXXXXXXXXXX` labels, preventing metric cardinality explosion and invalid characters.

2. **LRU eviction for token buckets** (S-16, lines 137-139, 171, 355-366): Token buckets track `last_access` time for proper LRU eviction when exceeding `MAX_TENANT_BUCKETS` (10,000).

3. **Metric cardinality cap** (lines 189-191, 279-291): Limits distinct tenant labels to `MAX_TENANT_METRIC_LABELS` (1,000), with overflow tenants aggregated under "overflow" label.

4. **Redis fallback with rate-limited logging** (lines 303-328): On Redis failure, falls back to local rate limiting with warning logged only on first occurrence and every 100th after.

5. **Redis timeout handling** (line 386): 2-second timeout prevents Redis latency from blocking the hot path.

6. **Defensive rate limit normalization** (lines 101-109, 143-147): Handles NaN, negative, and infinite values safely. Ensures `burst_capacity >= 1` when rate > 0.

7. **Atomic Redis token bucket** (lines 394-430): Lua script ensures atomic check-and-consume across distributed servers with 1-hour TTL.

8. **Good test coverage** (lines 513-628): Tests token bucket mechanics, refill behavior, quota enforcement, tenant isolation, and custom limits.

---

## Verification

```bash
cargo check -p dashflow-streaming  # Zero warnings
```

---

## Conclusion

rate_limiter.rs is well-designed with proper defensive coding, distributed rate limiting support, and cardinality protection. No P0-P3 issues found. Two P4 observations noted for future consideration but no action required.
