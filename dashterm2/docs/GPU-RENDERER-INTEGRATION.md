# DashTerm2 GPU Renderer Integration Roadmap

**Created**: 2025-12-30
**Updated**: 2025-12-31
**Status**: COMPLETE - Hybrid rendering architecture implemented
**Priority**: High - Eliminates unsafe ObjC concurrency code
**Related**: `~/dterm/docs/GPU-RENDERER-DIRECTIVE.md` (Rust side)

> **Note**: This document was the original design plan. Implementation is complete as of Worker #1687.
> See `sources/DTermMetalView.swift` and `sources/DTermCore.swift` for the actual implementation.
> **The hybrid rendering architecture is ENABLED BY DEFAULT as of Worker #1727.**
> To disable: `defaults write com.dashterm.dashterm2 dtermCoreRendererEnabled -bool NO`

---

## Background

The current GPU rendering stack in DashTerm2 has fundamental concurrency bugs:

| Component | Problem |
|-----------|---------|
| `iTermPromise` | Uses `dispatch_group` with strict enter/leave balance - crashes on edge cases |
| `iTermMetalView` | Reuses promises, causing callback accumulation |
| `iTermMetalDriver` | Complex ObjC concurrency that's hard to reason about |

We just fixed a crash ("Unbalanced call to dispatch_group_leave()") by switching to `dispatch_semaphore`, but this is a band-aid. The real fix is moving GPU rendering to dterm-core (Rust) where these bugs are impossible.

---

## Goal

Replace the ObjC rendering stack with a thin Swift wrapper around dterm-core's Rust renderer:

```
BEFORE (Current):
┌─────────────────────────────────────────────────────────┐
│                    DashTerm2 (ObjC/Swift)               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │iTermMetal   │  │iTermMetal   │  │iTermPromise     │ │
│  │View (1500+  │  │Driver       │  │(dispatch_group) │ │
│  │lines)       │  │(2000+ lines)│  │                 │ │
│  └─────────────┘  └─────────────┘  └─────────────────┘ │
│         ▲               ▲                ▲             │
│         └───────────────┴────────────────┘             │
│                 Complex, bug-prone                      │
└─────────────────────────────────────────────────────────┘

AFTER (Target):
┌─────────────────────────────────────────────────────────┐
│                    DashTerm2 (Swift)                     │
│  ┌─────────────────────────────────────────────────┐   │
│  │  DTermMetalView (~200 lines)                     │   │
│  │  - Owns CAMetalLayer                             │   │
│  │  - Provides drawables to dterm-core              │   │
│  │  - Handles resize/display link                   │   │
│  └─────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────┘
                         │ FFI
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    dterm-core (Rust)                     │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Renderer (wgpu)                                 │   │
│  │  - Frame sync (Rust channels - safe!)            │   │
│  │  - Glyph atlas                                   │   │
│  │  - Damage-based rendering                        │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Swift FFI Bindings for Renderer

Create Swift bindings to call dterm-core's renderer FFI.

#### 1.1 Add renderer bindings to DTermCore.swift

```swift
// DTermCore/DTermCore.swift (additions)

/// Opaque handle to dterm-core renderer
public class DTermRenderer {
    private let handle: OpaquePointer

    public init() {
        self.handle = dterm_renderer_create()
    }

    deinit {
        dterm_renderer_destroy(handle)
    }

    /// Request a frame. Returns handle to provide drawable.
    public func requestFrame() -> FrameHandle {
        let rawHandle = dterm_renderer_request_frame(handle)
        return FrameHandle(renderer: self, rawHandle: rawHandle)
    }

    /// Wait for frame with timeout.
    /// Returns true if ready, false on timeout.
    /// SAFE: Cannot crash with "unbalanced" errors.
    public func waitForFrame(_ frameHandle: FrameHandle, timeout: TimeInterval) -> Bool {
        let timeoutMs = UInt64(timeout * 1000)
        return dterm_renderer_wait_frame(handle, frameHandle.rawHandle, timeoutMs)
    }

    /// Render terminal to current surface.
    public func render(terminal: DTermTerminal) {
        dterm_renderer_render(handle, terminal.handle)
    }

    // Internal: provide drawable
    func provideDrawable(_ frameHandle: FrameHandle, texture: MTLTexture) {
        let texturePtr = Unmanaged.passUnretained(texture).toOpaque()
        dterm_renderer_provide_drawable(handle, frameHandle.rawHandle, texturePtr)
    }
}

public struct FrameHandle {
    weak var renderer: DTermRenderer?
    let rawHandle: DTermFrameHandle

    /// Provide the Metal drawable for this frame.
    public func complete(with drawable: CAMetalDrawable) {
        renderer?.provideDrawable(self, texture: drawable.texture)
    }
}
```

### Phase 2: Create DTermMetalView (Thin Wrapper)

Replace `iTermMetalView` (1500+ lines) with a minimal Swift view.

#### 2.1 DTermMetalView.swift

```swift
// sources/DTermMetalView.swift

import AppKit
import Metal
import QuartzCore

/// Minimal Metal view that delegates rendering to dterm-core.
/// Replaces iTermMetalView, iTermMetalDriver, and iTermPromise.
@MainActor
public class DTermMetalView: NSView {
    private var metalLayer: CAMetalLayer!
    private var displayLink: CVDisplayLink?
    private let renderer: DTermRenderer
    private weak var terminal: DTermTerminal?

    // MARK: - Initialization

    public init(frame: NSRect, renderer: DTermRenderer, terminal: DTermTerminal) {
        self.renderer = renderer
        self.terminal = terminal
        super.init(frame: frame)
        setupMetalLayer()
        setupDisplayLink()
    }

    required init?(coder: NSCoder) {
        fatalError("Use init(frame:renderer:terminal:)")
    }

    deinit {
        stopDisplayLink()
    }

    private func setupMetalLayer() {
        metalLayer = CAMetalLayer()
        metalLayer.device = MTLCreateSystemDefaultDevice()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        metalLayer.contentsScale = window?.backingScaleFactor ?? 2.0
        layer = metalLayer
        wantsLayer = true
    }

    // MARK: - Display Link

    private func setupDisplayLink() {
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        guard let displayLink else { return }

        let callback: CVDisplayLinkOutputCallback = { _, _, _, _, _, userInfo in
            let view = Unmanaged<DTermMetalView>.fromOpaque(userInfo!).takeUnretainedValue()
            DispatchQueue.main.async {
                view.render()
            }
            return kCVReturnSuccess
        }

        CVDisplayLinkSetOutputCallback(displayLink, callback,
            Unmanaged.passUnretained(self).toOpaque())
        CVDisplayLinkStart(displayLink)
    }

    private func stopDisplayLink() {
        guard let displayLink else { return }
        CVDisplayLinkStop(displayLink)
    }

    // MARK: - Rendering

    private func render() {
        guard let terminal else { return }

        // 1. Request frame from dterm-core
        let frameHandle = renderer.requestFrame()

        // 2. Get drawable from Metal layer
        guard let drawable = metalLayer.nextDrawable() else {
            return  // No drawable available - skip frame
        }

        // 3. Provide drawable to dterm-core
        frameHandle.complete(with: drawable)

        // 4. Wait for dterm-core to signal ready (with timeout)
        //    This CANNOT crash - Rust handles timeout safely
        let ready = renderer.waitForFrame(frameHandle, timeout: 1.0 / 60.0)
        guard ready else {
            return  // Timeout - skip frame (safe, no cleanup needed)
        }

        // 5. Tell dterm-core to render
        renderer.render(terminal: terminal)

        // 6. Present
        drawable.present()
    }

    // MARK: - Resize

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        metalLayer.drawableSize = CGSize(
            width: newSize.width * (window?.backingScaleFactor ?? 2.0),
            height: newSize.height * (window?.backingScaleFactor ?? 2.0)
        )
    }
}
```

**That's it.** ~100 lines replaces ~3500 lines of `iTermMetalView` + `iTermMetalDriver` + `iTermPromise`.

### Phase 3: Integration and Migration

#### 3.1 Feature flag for gradual rollout

```swift
// sources/iTermAdvancedSettingsModel.h (add)
+ (BOOL)useDTermCoreRenderer;

// Usage:
if (iTermAdvancedSettingsModel.useDTermCoreRenderer) {
    return DTermMetalView(frame: frame, renderer: renderer, terminal: terminal)
} else {
    return iTermMetalView(frame: frame)  // Legacy
}
```

#### 3.2 Migration checklist

| Component | Action | Status |
|-----------|--------|--------|
| `DTermMetalView.swift` | Created as new hybrid renderer | ✅ DONE (#1668) |
| `DTermHybrid.metal` | New Metal shaders for hybrid renderer | ✅ DONE (#1685) |
| `SessionView` integration | Selects DTermMetalView when enabled | ✅ DONE (#1669) |
| Feature flag | `dtermCoreRendererEnabled` advanced setting | ✅ DONE |
| FPS overlay | Development FPS counter | ✅ DONE (#1672-1673) |
| Image rendering (Sixel/Kitty) | Via DTermMetalView | ✅ DONE (#1684-1687) |
| Legacy ObjC files | Keep for fallback until stable | ⏳ PENDING DELETION |

#### 3.3 Files to delete after migration complete

```
sources/iTermMetalView.swift          (1500+ lines)
sources/iTermMetalDriver.m            (2000+ lines)
sources/iTermMetalDriver.h
sources/iTermMetalFrameData.m
sources/iTermMetalFrameData.h
sources/iTermMetalRowData.m
sources/iTermMetalRowData.h
sources/iTermPromise.m                (600+ lines)
sources/iTermPromise.h
sources/Metal/*.metal                 (shaders)
```

**Total: ~6000+ lines of complex ObjC deleted, replaced with ~100 lines of Swift.**

### Phase 4: Testing

#### 4.1 Visual regression tests

```swift
// DashTerm2Tests/DTermMetalViewTests.swift

class DTermMetalViewTests: XCTestCase {
    func testRenderBasicText() async {
        let renderer = DTermRenderer()
        let terminal = DTermTerminal()
        terminal.feed("Hello, World!")

        let view = DTermMetalView(frame: .init(x: 0, y: 0, width: 800, height: 600),
                                   renderer: renderer, terminal: terminal)

        // Capture screenshot
        let image = view.snapshot()

        // Compare to golden image
        XCTAssertTrue(image.matches(golden: "basic_text.png", tolerance: 0.01))
    }

    func testRenderWithTimeout() async {
        // Simulate slow drawable acquisition
        let renderer = DTermRenderer()
        let terminal = DTermTerminal()

        // This should NOT crash (the bug we fixed)
        for _ in 0..<1000 {
            let frameHandle = renderer.requestFrame()
            // Don't provide drawable - timeout
            let ready = renderer.waitForFrame(frameHandle, timeout: 0.001)
            XCTAssertFalse(ready)  // Should timeout, not crash
        }
    }
}
```

#### 4.2 Performance benchmarks

```swift
func testRenderPerformance() {
    let renderer = DTermRenderer()
    let terminal = DTermTerminal()

    // Fill terminal with text
    for _ in 0..<10000 {
        terminal.feed("Lorem ipsum dolor sit amet\n")
    }

    measure {
        for _ in 0..<60 {  // 1 second of frames
            renderer.render(terminal: terminal)
        }
    }
    // Target: < 16ms per frame average
}
```

---

## Acceptance Criteria

1. **No ObjC concurrency primitives in rendering path**
   - No `dispatch_group`
   - No `dispatch_semaphore` (removed from iTermPromise)
   - No manual lock/unlock in hot path

2. **Performance parity or better**
   - 60 FPS sustained
   - No regression in input latency
   - Memory usage similar or lower

3. **Code reduction**
   - Delete 6000+ lines of ObjC
   - New Swift code < 200 lines

4. **All existing tests pass**
   - Visual appearance unchanged
   - No rendering regressions

---

## Timeline Dependencies

This work depends on dterm-core implementing:
1. `dterm_renderer_create()` - Renderer initialization
2. `dterm_renderer_request_frame()` - Frame request
3. `dterm_renderer_provide_drawable()` - Drawable provision
4. `dterm_renderer_wait_frame()` - Safe timeout handling
5. `dterm_renderer_render()` - Actual rendering

See `~/dterm/docs/GPU-RENDERER-DIRECTIVE.md` for the Rust implementation plan.

---

## Rollback Plan

If issues are found after migration:
1. Feature flag `useDTermCoreRenderer` defaults to `false`
2. Legacy code kept in codebase until migration proven stable
3. Can revert to legacy renderer instantly via preference

---

## Related Bugs Fixed by This Migration

| Bug | Description | Root Cause |
|-----|-------------|------------|
| BUG-4043 | dispatch_group_leave crash | Manual concurrency in ObjC |
| BUG-1620 | Promise race condition | Reusing promises |
| BUG-f1639 | Unbalanced dispatch_group_leave | Same as above |

All of these become **impossible** with Rust-based frame sync.
