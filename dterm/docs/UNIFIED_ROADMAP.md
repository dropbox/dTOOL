# Unified Roadmap - dterm-core

**Date:** 2025-12-31
**Status:** 23/23 Victory Conditions Complete
**Next:** Hardening & Optimization

---

## Current State

| Metric | Value | Status |
|--------|-------|--------|
| Victory Conditions | 23/23 | COMPLETE |
| Tests | 1914+ | PASSING |
| Performance | 3.6 GiB/s (ASCII) | EXCEEDS |
| Latency | ~11 ns | 175,000x better |

---

## Active Directive Files

| File | Purpose | Priority |
|------|---------|----------|
| [GPU-RENDERER-DIRECTIVE.md](GPU-RENDERER-DIRECTIVE.md) | wgpu GPU renderer for DashTerm2 | **HIGH** |
| [WORKER_DIRECTIVE_HARDENING.md](WORKER_DIRECTIVE_HARDENING.md) | Kernel-level security hardening | **CRITICAL** |
| [WORKER_DIRECTIVE_VERIFICATION.md](WORKER_DIRECTIVE_VERIFICATION.md) | Formal proofs & testing | **HIGH** |
| [WORKER_DIRECTIVE_263.md](WORKER_DIRECTIVE_263.md) | 33 improvements | MEDIUM |

---

## Phase A: KERNEL HARDENING (CRITICAL)

**Source:** `docs/WORKER_DIRECTIVE_HARDENING.md`

### A1. Adversarial Fuzzing
| Target | Status | Priority |
|--------|--------|----------|
| `adversarial_vt.rs` - CVE patterns | **DONE** | CRITICAL |
| `resource_exhaustion.rs` - Billion laughs | **DONE** | CRITICAL |
| `terminal.rs` - Full integration | **DONE** | HIGH |

### A2. Sanitizer Suite
| Sanitizer | Status | Priority |
|-----------|--------|----------|
| AddressSanitizer (ASan) | READY | CRITICAL |
| MemorySanitizer (MSan) | READY | CRITICAL |
| ThreadSanitizer (TSan) | READY | CRITICAL |
| UndefinedBehaviorSanitizer | READY | HIGH |
| `scripts/sanitizer-check.sh` | **DONE** | HIGH |

### A3. Stress Tests
| Test | Duration | Status |
|------|----------|--------|
| 24-hour stability | 24h | TODO |
| Memory pressure | ~1h | TODO |
| Concurrent stress | ~30m | TODO |

### A4. CVE Regression Tests
| CVE Pattern | Terminal | Status |
|-------------|----------|--------|
| Integer overflow cursor | xterm | **DONE** |
| SGR overflow | rxvt | **DONE** |
| DECRQSS leak | xterm | **DONE** |
| Title injection | xterm | **DONE** |
| Nested escape DoS | generic | **DONE** |
| UTF-8 overlong encoding | generic | **DONE** |
| Resource exhaustion | generic | **DONE** |

**Note:** CVE regression tests added in `src/tests/cve_regression.rs`

---

## Phase B: FORMAL VERIFICATION (HIGH)

**Source:** `docs/WORKER_DIRECTIVE_VERIFICATION.md`

### B1. Memory Safety Proofs (Kani)
| Module | Proofs Needed | Status |
|--------|---------------|--------|
| `grid/row.rs` | 4 (unchecked access) | **DONE** |
| `grid/page.rs` | 3 (pointer arithmetic) | **DONE** |
| `scrollback/disk.rs` | 3 (mmap bounds) | **DONE** |

### B2. New Fuzz Targets
| Target | Gap Filled | Status |
|--------|-----------|--------|
| `terminal.rs` | Full integration | **DONE** |
| `ffi.rs` | FFI boundary | **DONE** |
| `kitty_graphics.rs` | Graphics state | **DONE** |
| `checkpoint.rs` | Serialization | **DONE** |
| `selection.rs` | Wide char bugs | **DONE** |

### B3. Property Tests (Proptest)
| Test | Status |
|------|--------|
| Origin mode + scroll region | **DONE** |
| Wide char selection | **DONE** |
| Scrollback tier ordering | **DONE** |
| Alternate screen round-trip | **DONE** |

### B4. Concurrency Tests (Loom)
| Test | Status |
|------|--------|
| FairMutex no data race | TODO |
| FairRwLock no data race | TODO |
| Lease prevents starvation | TODO |

---

## Phase C: IMPROVEMENTS (MEDIUM)

**Source:** `docs/WORKER_DIRECTIVE_263.md`

### C1. Optional Features (3 items)
| Feature | Effort | Status |
|---------|--------|--------|
| Agent-Terminal Integration | HIGH | Step 1 DONE |
| WASM Plugin System | HIGH | Phase 5 hardening COMPLETE |
| Memory Pooling | MEDIUM | TODO |

### C2. Memory Optimizations (10 items)
| ID | Target | Savings |
|----|--------|---------|
| M1 | KittyPlacement field sizes | ~30 bytes |
| M2 | Arc<[u8]> for images | 24 bytes + cache |
| M3 | SmallString for titles | Heap alloc |
| M4 | CommandMark Options | ~56 bytes |
| M5 | LoadingImage pre-alloc | Reallocs |
| M6 | SmallVec for placements | ~100 bytes |
| M7 | Bounded command_marks | Unbounded growth |
| M8 | Sparse ColorPalette | ~700 bytes |
| M9 | CellExtra packing | ~40 bytes |
| M10 | title_stack interning | 48+ bytes |

### C3. Efficiency Optimizations (10 items)
| ID | Target | Impact |
|----|--------|--------|
| E1 | Bloom filter SIMD | 2-3x search |
| E2 | Unchecked row clear | Faster clear |
| E3 | inline(always) cells | Every render |
| E4 | O(log n) search | Large scrollback |
| E5 | Lock-free style table | Parallel render |
| E6 | In-place bitmap ops | Fewer allocs |
| E7 | Page zeroing skip | 64KB/reuse |
| E8 | Branch hints | Micro-opt |
| E9 | Lazy row length | Row operations |
| E10 | Explicit parser SIMD | Consistent perf |

**Notes (Iteration 449):**
- M4 (CommandMark Options): DEFERRED - ~32KB savings not worth API churn
- M9 (CellExtra packing): COMPLETE - see `grid/extra.rs` docs
- E1 (Bloom filter SIMD): DEFERRED - FNV-1a optimal for trigrams
- E5 (Lock-free style table): N/A - single-threaded design
- E10 (Explicit parser SIMD): COMPLETE - LLVM auto-vectorizes

### C4. Usability Improvements (10 items)
| ID | Target | Impact |
|----|--------|--------|
| U1 | Scrollback defaults | Easier API |
| U2 | Display for IDs | Better debugging |
| U3 | Config builders | Fluent config |
| U4 | TerminalSize struct | Clearer API |
| U5 | Error conversions | Error handling |
| U6 | SearchMatch methods | Ergonomic search |
| U7 | Error context | Better messages |
| U8 | Orchestrator Clone | Testing |
| U9 | Duration types | Type safety |
| U10 | TriggerBuilder | Clean builder |

---

## Phase D: ORIGINAL ROADMAP ITEMS

**Source:** `docs/ROADMAP.md`

### D1. Phase 12 - Agent-Terminal Integration
| Step | Description | Status |
|------|-------------|--------|
| 1 | TerminalSlot resources | COMPLETE |
| 2 | ExecutionIoDriver trait | TODO |
| 3 | Wire execution to I/O | TODO |
| 4 | Completion detection | TODO |
| 5 | Higher-level runtime | TODO |

### D2. Remaining TLA+ Verification
- [ ] Run TLC model checker on all 15 specs
- [ ] Document any counterexamples found

### D3. Interactive vttest
- [ ] Run via macOS demo
- [ ] Document results in CONFORMANCE.md

---

## Phase E: GPU RENDERER (HIGH)

**Source:** `docs/GPU-RENDERER-DIRECTIVE.md`

**Impact:** Replaces ~5000 lines ObjC → ~1500 lines Rust

### E1. Frame Synchronization (Critical)
| Component | Purpose | Status |
|-----------|---------|--------|
| `FrameHandle` | Ownership-based sync (cannot crash) | **COMPLETE** |
| `oneshot::channel` | Replaces dispatch_group | **COMPLETE** |
| FFI bindings | C API for Swift | **COMPLETE** |

### E2. wgpu Renderer
| Component | Purpose | Status |
|-----------|---------|--------|
| `wgpu_backend.rs` | Cross-platform GPU | TODO |
| `glyph_atlas.rs` | Font texture atlas | TODO |
| `damage.rs` | Damage-based rendering | TODO |
| `cell.wgsl` | Cell rendering shader | TODO |

### E3. FFI Layer
| Function | Purpose | Status |
|----------|---------|--------|
| `dterm_renderer_create()` | Create renderer | **COMPLETE** |
| `dterm_renderer_request_frame()` | Request frame handle | **COMPLETE** |
| `dterm_renderer_provide_drawable()` | Provide Metal texture | **COMPLETE** |
| `dterm_renderer_wait_frame()` | Safe wait with timeout | **COMPLETE** |
| `dterm_renderer_render()` | Render terminal state | **COMPLETE** |

**Key Insight:** Rust ownership eliminates crash class:
```rust
// Rust: ownership prevents double-signal
let (tx, rx) = oneshot::channel();
tx.send(value);  // tx is CONSUMED here
// tx.send(again);  // Compiler error: use of moved value
```

---

## Execution Priority

### Week 1: Security Hardening
1. Create adversarial fuzz targets
2. Run sanitizer suite
3. Add CVE regression tests
4. Create `scripts/sanitizer-check.sh`

### Week 2: Formal Verification
1. Add Row/Page/Mmap Kani proofs
2. Create Terminal/FFI fuzz targets
3. Add Loom concurrency tests
4. Run extended proptest suite

### Week 3: Performance & Usability
1. Quick wins (E3, U2, M5, E6, U6)
2. Memory optimizations (M1-M10)
3. Efficiency optimizations (E1-E10)

### Week 4: Integration
1. Complete Phase 12 steps 2-5
2. Run 24-hour stress test
3. Full verification suite
4. Documentation update

---

## Verification Gates

### Before ANY Commit
```bash
cargo build -p dterm-core --features ffi
cargo test -p dterm-core --features ffi
cargo clippy -p dterm-core --features ffi -- -D warnings
```

### Before Feature Merge
```bash
cargo +nightly miri test grid::page::
PROPTEST_CASES=10000 cargo test proptest
cargo +nightly fuzz run parser -- -max_total_time=300
```

### Before Release
```bash
./scripts/nightly-verification.sh  # Full suite
./scripts/sanitizer-check.sh       # All sanitizers
```

---

## Quick Reference

| Task Type | Go To |
|-----------|-------|
| GPU renderer | `docs/GPU-RENDERER-DIRECTIVE.md` |
| Security hardening | `docs/WORKER_DIRECTIVE_HARDENING.md` |
| Formal proofs | `docs/WORKER_DIRECTIVE_VERIFICATION.md` |
| Optimizations | `docs/WORKER_DIRECTIVE_263.md` |
| Full roadmap | `docs/ROADMAP.md` |
| Gap analysis | `docs/GAP_ANALYSIS.md` |
| Feature union | `docs/FEATURE_UNION.md` |
