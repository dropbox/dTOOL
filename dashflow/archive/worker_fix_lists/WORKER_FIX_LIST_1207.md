# WORKER FIX LIST #1207 - println! Migration (M-71)

**Created:** 2025-12-19 by Worker #1207
**Previous:** WORKER_FIX_LIST.md, WORKER_FIX_LIST_1200.md (P0-P1 complete)
**Priority:** Code quality - structured logging

---

## P0: println! to tracing Migration (M-71)

### Overview
The `optimize/` module has ~170 ungated `println!` calls that should use `tracing` for structured logging.

**Benefits of tracing:**
- Log level filtering (debug vs info vs warn)
- Structured fields for better queryability
- Integration with observability tools (Grafana, etc.)
- Consistent with rest of dashflow codebase

### Completed
- `graph_optimizer.rs` - 28 calls converted (commit #1206)
- `optimizers/copro.rs` - 13 calls converted (commit #1207)
- `optimizers/copro_v2.rs` - 18 calls converted (commit #1208)

**Total converted: 59 calls**

### Remaining Files (Priority Order)

| File | Count | Priority | Notes |
|------|-------|----------|-------|
| `optimizers/mipro_v2.rs` | 25 | High | Core optimizer |
| `optimizers/simba.rs` | 22 | High | Core optimizer |
| `optimizers/bootstrap_optuna.rs` | 18 | Medium | Optuna integration |
| `optimizers/autoprompt.rs` | 14 | Medium | AutoPrompt optimizer |
| `distillation/mod.rs` | 9 | Medium | Distillation module |
| `optimizers/random_search.rs` | 9 | Low | Simple optimizer |
| Other files | ~40 | Low | Various modules |

**Total remaining: ~115 calls across 23 files**

---

## Fix Pattern

```rust
// Before
println!("Optimization started with {} examples", count);

// After
tracing::info!(num_examples = count, "Optimization started");

// For verbose output
tracing::debug!(details = %value, "Detailed info");

// For warnings
tracing::warn!(error = %e, "Operation failed, continuing");
```

---

## Verification Commands

```bash
# Count remaining println! in optimize/
grep -rn 'println!' crates/dashflow/src/optimize/ | grep -v '///' | grep -v '//!' | grep -v test | wc -l

# Should decrease after each batch of fixes
# Target: 0 (excluding doc examples)

# Verify builds
cargo check -p dashflow --lib
```

---

## Worker Directive

1. **Start with high-priority files** (mipro_v2.rs, simba.rs, copro_v2.rs)
2. **Batch commits** - one commit per 2-3 files
3. **Use appropriate log levels:**
   - `tracing::info!` for user-visible progress
   - `tracing::debug!` for verbose/detailed output
   - `tracing::warn!` for recoverable errors
   - `tracing::error!` for serious failures
4. **Verify after each batch:** `cargo check -p dashflow --lib`

---

## Related Issues (from WORKER_FIX_LIST.md)

- P2 Issue 23: Large source files need refactoring
- P2 Issue 24: Excessive unwrap() usage (1,794 in codex_dashflow)
