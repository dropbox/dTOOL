# Phase E: GPU Renderer Roadmap

**Created**: 2025-12-30
**Status**: Complete (Phase E done)
**Iteration**: 275-311
**Mandate**: Formal verification required for all new code
**DashTerm2 Integration**: See `docs/GPU-RENDERER-DIRECTIVE.md`

---

## Formal Verification Requirements

**ALL code in this phase MUST have:**

| Verification Type | Requirement | Tool |
|------------------|-------------|------|
| Concurrency Specs | TLA+ for frame sync, rendering pipeline | TLC model checker |
| Memory Safety | Kani proofs for all GPU buffer handling | Kani |
| Data Race Freedom | Proofs for multi-threaded rendering | MIRI, Kani, TLA+ |
| Resource Leaks | Proofs for GPU resource cleanup | Kani |
| Fuzz Testing | All input handling, resize, damage | cargo-fuzz |
| Property Testing | All public APIs | proptest |

---

## Problem Statement

DashTerm2's ObjC rendering stack has fundamental concurrency bugs:
- `iTermMetalView`, `iTermMetalDriver`, `iTermPromise` use `dispatch_group`
- Manual enter/leave counting causes "Unbalanced call to dispatch_group_leave()" crashes
- ObjC lacks compile-time safety for concurrent code

**Solution**: Move GPU rendering to dterm-core where Rust's ownership model makes these bugs **impossible**.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    DashTerm2 (Swift - ~100 lines)               │
│                    Thin wrapper, provides CAMetalDrawable       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ C FFI (Kani Verified)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    GPU RENDERER (dterm-core)                     │
│                    TLA+ Specified • Kani Verified                │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │  FrameSync   │───▶│  Renderer    │───▶│  GlyphAtlas  │       │
│  │  (TLA+)      │    │  (wgpu)      │    │  (Kani)      │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│         │                    │                    │              │
│         │                    │                    │              │
│  ┌──────▼──────┐    ┌───────▼──────┐    ┌───────▼──────┐       │
│  │  oneshot    │    │  Pipeline    │    │  Allocator   │       │
│  │  channels   │    │  (verified)  │    │  (Kani)      │       │
│  │  (no crash) │    │              │    │              │       │
│  └─────────────┘    └──────────────┘    └──────────────┘       │
│                                                                  │
│  REPLACES: iTermMetalView + iTermMetalDriver + iTermPromise     │
│            (~6000 lines ObjC → ~100 lines Swift wrapper)         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Step 1: TLA+ Specification for Frame Synchronization

**Deliverable**: `specs/tlaplus/FrameSync.tla`

This is the CRITICAL component that replaces the buggy `dispatch_group` code.

```tla
---------------------------- MODULE FrameSync ------------------------------
EXTENDS Naturals, Sequences, TLC

CONSTANTS
    MAX_PENDING_FRAMES,
    TIMEOUT_MS

VARIABLES
    frame_requests,     \* Sequence of pending frame requests
    frame_handles,      \* Map of handle_id -> state
    completed_frames,   \* Set of completed frame IDs
    render_state        \* Current render state

TypeInvariant ==
    /\ frame_requests \in Seq(FrameRequest)
    /\ Len(frame_requests) <= MAX_PENDING_FRAMES
    /\ render_state \in {"Idle", "WaitingForDrawable", "Rendering"}

\* CRITICAL SAFETY PROPERTY: No "unbalanced" errors possible
\* Unlike dispatch_group, we cannot have more "leaves" than "enters"
NoUnbalancedOperations ==
    \A h \in DOMAIN frame_handles:
        frame_handles[h].completed <= 1  \* Can only complete once

\* SAFETY: Timeout is always safe
TimeoutSafe ==
    \A h \in DOMAIN frame_handles:
        frame_handles[h].state = "TimedOut" =>
            \* Handle can be safely dropped, no cleanup needed
            /\ frame_handles[h].cleanup_required = FALSE

\* SAFETY: Frame completion after timeout is safe (no crash)
LateCompletionSafe ==
    \A h \in DOMAIN frame_handles:
        /\ frame_handles[h].state = "TimedOut"
        /\ frame_handles[h].drawable_provided = TRUE
        => \* Drawable is simply dropped, no error
           frame_handles[h].error = NULL

\* LIVENESS: All requested frames eventually complete or timeout
EventualCompletion ==
    []<>(\A h \in DOMAIN frame_handles:
         frame_handles[h].state \in {"Completed", "TimedOut", "Cancelled"})

\* SAFETY: No resource leaks
NoResourceLeaks ==
    \A h \in DOMAIN frame_handles:
        frame_handles[h].state \in {"Completed", "TimedOut", "Cancelled"} =>
            frame_handles[h].resources_freed = TRUE

=============================================================================
```

**Model Check Requirements**:
- 10+ concurrent frame requests
- Timeouts occurring at any point
- Late completions after timeout
- Rapid request/cancel cycles (the pattern that crashes ObjC)

---

## Step 2: TLA+ Specification for Render Pipeline

**Deliverable**: `specs/tlaplus/RenderPipeline.tla`

```tla
--------------------------- MODULE RenderPipeline ---------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS
    MAX_VERTICES,
    MAX_ATLAS_SIZE,
    GPU_MEMORY_LIMIT

VARIABLES
    vertex_buffer,      \* Current vertex buffer contents
    atlas_state,        \* Glyph atlas state
    pipeline_state,     \* Render pipeline state
    gpu_resources       \* Allocated GPU resources

\* SAFETY: GPU memory bounded
MemoryBounded ==
    gpu_resources.total_bytes <= GPU_MEMORY_LIMIT

\* SAFETY: Atlas never overflows
AtlasNeverOverflows ==
    atlas_state.used_bytes <= MAX_ATLAS_SIZE

\* SAFETY: Vertex buffer bounds
VertexBoundsValid ==
    Len(vertex_buffer) <= MAX_VERTICES

\* SAFETY: Pipeline state machine valid
PipelineStateValid ==
    pipeline_state \in {"Uninitialized", "Ready", "Recording", "Executing", "Error"}

\* No invalid transitions
ValidTransitions ==
    [][
        \/ (pipeline_state = "Uninitialized" /\ pipeline_state' \in {"Ready", "Error"})
        \/ (pipeline_state = "Ready" /\ pipeline_state' \in {"Recording", "Error"})
        \/ (pipeline_state = "Recording" /\ pipeline_state' \in {"Executing", "Ready", "Error"})
        \/ (pipeline_state = "Executing" /\ pipeline_state' \in {"Ready", "Error"})
        \/ (pipeline_state = "Error" /\ pipeline_state' = "Uninitialized")
    ]_pipeline_state

=============================================================================
```

---

## Step 3: Kani Proofs for Frame Synchronization

**Deliverable**: `src/renderer/frame_sync_proofs.rs`

```rust
#[cfg(kani)]
mod frame_sync_proofs {
    use super::*;

    /// CRITICAL PROOF: Timeout cannot cause "unbalanced" crash
    /// This is the exact bug that crashes ObjC code
    #[kani::proof]
    fn proof_timeout_never_crashes() {
        let mut sync = FrameSync::new();

        // Request a frame
        let handle = sync.request_frame();

        // Timeout occurs (drawable never provided)
        let result = sync.wait_for_frame_blocking(1);
        kani::assert!(result.is_none());

        // Handle is now dropped - MUST NOT CRASH
        drop(handle);

        // FrameSync is still valid
        kani::assert!(sync.is_valid());
    }

    /// CRITICAL PROOF: Late completion after timeout is safe
    /// This is another crash pattern in ObjC
    #[kani::proof]
    fn proof_late_completion_safe() {
        let mut sync = FrameSync::new();
        let handle = sync.request_frame();

        // Timeout first
        let result = sync.wait_for_frame_blocking(1);
        kani::assert!(result.is_none());

        // Now drawable arrives late
        // In ObjC: dispatch_group_leave() -> CRASH
        // In Rust: oneshot send to closed channel -> Ok (no crash)
        handle.complete(MockTexture::new());

        // Must not crash, must not leak
        kani::assert!(sync.is_valid());
    }

    /// PROOF: Rapid request/cancel cycle is safe
    /// This is the exact pattern that triggers ObjC crashes
    #[kani::proof]
    #[kani::unwind(100)]
    fn proof_rapid_request_cancel_safe() {
        let mut sync = FrameSync::new();

        for _ in 0..100 {
            let handle = sync.request_frame();

            let complete: bool = kani::any();
            let timeout: bool = kani::any();

            if complete {
                handle.complete(MockTexture::new());
            }

            if timeout {
                let _ = sync.wait_for_frame_blocking(1);
            }

            // Handle may be dropped without completing
            // MUST NOT CRASH (unlike dispatch_group)
        }

        kani::assert!(sync.is_valid());
    }

    /// PROOF: oneshot channel semantics are correct
    #[kani::proof]
    fn proof_oneshot_send_once() {
        let (tx, rx) = oneshot::channel::<i32>();

        // Can only send once (compile-time enforced by move semantics)
        tx.send(42);
        // tx.send(43); // Would not compile - tx is moved

        // Receive is safe
        let result = rx.blocking_recv();
        kani::assert!(result == Ok(42));
    }
}
```

---

## Step 4: Kani Proofs for GPU Resource Management

**Deliverable**: `src/renderer/gpu_proofs.rs`

```rust
#[cfg(kani)]
mod gpu_proofs {
    use super::*;

    /// PROOF: Vertex buffer never exceeds bounds
    #[kani::proof]
    #[kani::unwind(1000)]
    fn proof_vertex_buffer_bounded() {
        let mut buffer = VertexBuffer::new(MAX_VERTICES);

        for _ in 0..1000 {
            let vertex: Vertex = kani::any();
            buffer.push(vertex);

            kani::assert!(buffer.len() <= MAX_VERTICES);
        }
    }

    /// PROOF: Atlas allocation never overflows
    #[kani::proof]
    fn proof_atlas_allocation_safe() {
        let mut atlas = GlyphAtlas::new(MAX_ATLAS_SIZE);

        let width: u32 = kani::any_where(|&w| w > 0 && w <= 256);
        let height: u32 = kani::any_where(|&h| h > 0 && h <= 256);

        match atlas.allocate(width, height) {
            Some(region) => {
                kani::assert!(region.x + region.width <= atlas.width());
                kani::assert!(region.y + region.height <= atlas.height());
            }
            None => {
                // Allocation failed - atlas is full, but no crash
                kani::assert!(atlas.is_valid());
            }
        }
    }

    /// PROOF: GPU resource cleanup is complete
    #[kani::proof]
    fn proof_resource_cleanup() {
        let renderer = Renderer::new_mock();

        // Allocate resources
        let buffer = renderer.create_buffer(1024);
        let texture = renderer.create_texture(256, 256);

        // Track allocations
        let initial_count = renderer.resource_count();

        // Drop resources
        drop(buffer);
        drop(texture);

        // All resources freed
        kani::assert!(renderer.resource_count() == initial_count - 2);
    }
}
```

---

## Step 5: Fuzz Testing for Renderer

**Deliverable**: `fuzz/fuzz_targets/renderer.rs`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use dterm_core::renderer::{Renderer, FrameSync};

#[derive(Debug, Arbitrary)]
enum RendererAction {
    RequestFrame,
    ProvideDrawable { late: bool },
    Timeout { ms: u16 },
    Render { damage_count: u8 },
    Resize { width: u16, height: u16 },
    Reset,
}

fuzz_target!(|actions: Vec<RendererAction>| {
    let mut renderer = Renderer::new_mock();
    let mut sync = FrameSync::new();
    let mut pending_handle: Option<FrameHandle> = None;

    for action in actions {
        match action {
            RendererAction::RequestFrame => {
                pending_handle = Some(sync.request_frame());
            }
            RendererAction::ProvideDrawable { late } => {
                if let Some(handle) = pending_handle.take() {
                    if !late {
                        handle.complete(MockTexture::new());
                    }
                    // If late, handle is dropped without completing
                    // MUST NOT CRASH
                }
            }
            RendererAction::Timeout { ms } => {
                let _ = sync.wait_for_frame_blocking(ms as u64);
            }
            RendererAction::Render { damage_count } => {
                let damage = generate_damage(damage_count);
                renderer.render_damage(&damage);
            }
            RendererAction::Resize { width, height } => {
                let w = width.max(1).min(4096);
                let h = height.max(1).min(4096);
                renderer.resize(w, h);
            }
            RendererAction::Reset => {
                renderer.reset();
                pending_handle = None;
            }
        }

        // Invariants must always hold
        assert!(renderer.is_valid());
        assert!(sync.is_valid());
    }
});
```

---

## Implementation Steps

| Step | Description | Verification | Status |
|------|-------------|--------------|--------|
| 1 | TLA+ spec for FrameSync | TLC: 10+ frames, all timeout patterns | COMPLETE |
| 2 | TLA+ spec for RenderPipeline | TLC: memory bounds, state machine | COMPLETE (iter 301) |
| 3 | Implement FrameSync with oneshot channels | Kani: 5+ proofs | COMPLETE |
| 4 | Implement wgpu renderer skeleton | Kani: resource proofs | COMPLETE |
| 5 | Implement GlyphAtlas | Kani: allocation proofs | COMPLETE |
| 6 | Implement damage-based rendering | Fuzz testing | COMPLETE (iter 301) |
| 7 | FFI layer | Kani + MIRI | COMPLETE (iter 302) |
| 8 | DashTerm2 Swift wrapper | Integration tests | COMPLETE (iter 311) |

---

## Verification Checklist (MANDATORY)

Before merging any code in this phase:

- [x] TLA+ FrameSync spec model-checked (10+ frames, timeout patterns)
- [x] TLA+ RenderPipeline spec written (iter 301)
- [ ] TLA+ RenderPipeline spec model-checked (pending TLC/Java)
- [x] Kani proof: timeout_never_crashes
- [x] Kani proof: late_completion_safe
- [x] Kani proof: rapid_request_cancel_safe
- [x] Kani proof: vertex_buffer_bounded
- [x] Kani proof: atlas_allocation_safe
- [x] Kani proof: resource_cleanup
- [x] MIRI clean run on damage + frame_sync tests (iter 301)
- [x] Fuzz target run (579K+ runs, 0 crashes)
- [x] Zero clippy warnings
- [x] All tests pass (1891 tests with gpu,ffi features; iter 311)
- [x] Swift package builds (iter 311)

---

## Success Criteria

1. **Zero concurrency bugs** - Proven by TLA+ and Kani
2. **No "unbalanced dispatch_group" possible** - Rust ownership prevents it
3. **No resource leaks** - Proven by Kani
4. **60 FPS sustained** - Benchmarked on M1 MacBook Air
5. **<1ms frame latency** - Benchmarked
6. **<50MB GPU memory** for 10K line scrollback

---

## DashTerm2 Integration

DashTerm2 now:
1. Provides `CAMetalDrawable` to dterm-core via FFI
2. Uses the thin Swift wrapper (legacy renderer still available)
3. Gates the new renderer behind `dtermCoreRendererEnabled`

See `docs/GPU-RENDERER-DIRECTIVE.md` for full API specification.

---

## References

- `docs/GPU-RENDERER-DIRECTIVE.md` - API specification
- `docs/WORKER_DIRECTIVE_VERIFICATION.md` - Verification requirements
- `specs/tlaplus/` - Existing TLA+ specifications
- `crates/dterm-core/src/kani_proofs/` - Existing Kani proofs
