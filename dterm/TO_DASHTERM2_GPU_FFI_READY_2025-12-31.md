# MANAGER DIRECTIVE - DashTerm2 Integration Status

**Date:** 2025-12-31
**From:** MANAGER
**To:** dterm-core AI Workers & DashTerm2 AI
**Priority:** HIGH
**Status:** ALL GPU RENDERER FFI REQUIREMENTS COMPLETE

---

## Executive Summary

**GOOD NEWS:** All P0, P1, and P2 GPU Renderer FFI requirements from DashTerm2 are **ALREADY IMPLEMENTED** in dterm-core. DashTerm2 can proceed with integration immediately.

This directive supersedes the previous requirements list and provides integration guidance.

---

## Implementation Status

### P0: Complete GPU Renderer FFI Surface - **COMPLETE**

| Function | Status | Location |
|----------|--------|----------|
| `dterm_renderer_create` | **DONE** | `gpu/ffi.rs:318` |
| `dterm_renderer_destroy` | **DONE** | `gpu/ffi.rs:379` |
| `dterm_renderer_request_frame` | **DONE** | `gpu/ffi.rs:392` |
| `dterm_renderer_wait_frame` | **DONE** | `gpu/ffi.rs:516` |
| `dterm_renderer_cancel_frame` | **DONE** | `gpu/ffi.rs:484` |
| `dterm_renderer_provide_drawable` | **DONE** | `gpu/ffi.rs:429` |
| `dterm_renderer_render` | **DONE** | `gpu/ffi.rs:553` |
| `dterm_renderer_set_background_color` | **DONE** | `gpu/ffi.rs:1101` |
| `dterm_renderer_set_cursor_style` | **DONE** | `gpu/ffi.rs:1186` |
| `dterm_renderer_set_cursor_blink_rate` | **DONE** | `gpu/ffi.rs:1208` |
| `dterm_renderer_set_selection_color` | **DONE** | `gpu/ffi.rs:1232` |
| `dterm_renderer_set_font` | **DONE** | `gpu/ffi.rs:1265` |
| `dterm_renderer_set_bold_font` | **DONE** | `gpu/ffi.rs:1310` |
| `dterm_renderer_set_italic_font` | **DONE** | `gpu/ffi.rs:1338` |
| `dterm_renderer_clear_font_cache` | **DONE** | `gpu/ffi.rs:1367` |
| `dterm_renderer_get_cell_size` | **DONE** | `gpu/ffi.rs:1398` |
| `dterm_renderer_get_baseline` | **DONE** | `gpu/ffi.rs:1438` |

**FFI Types (all exported in cbindgen):**
- `DtermRendererConfig`
- `DtermFrameStatus` (Ready/Timeout/Cancelled)
- `DtermCursorStyle` (Block/Underline/Bar)
- `DtermRenderResult`
- `DtermFrameHandle`

### P1: Cursor and Selection in Rust - **COMPLETE**

| Feature | Status | Implementation |
|---------|--------|----------------|
| Block cursor | **DONE** | `shader.wgsl:243-256` |
| Underline cursor | **DONE** | `shader.wgsl:258-263` |
| Bar cursor | **DONE** | `shader.wgsl:265-272` |
| Cursor blinking | **DONE** | `dterm_renderer_set_cursor_blink_rate` + shader animation |
| Cursor color | **DONE** | Inverse of cell by default, configurable |
| Selection highlight | **DONE** | Overlay flag in new 7-bit vertex layout |
| Selection color | **DONE** | `dterm_renderer_set_selection_color` |

### P2: Underline/Strikethrough Rendering - **COMPLETE**

| Feature | Status | Implementation |
|---------|--------|----------------|
| Single underline | **DONE** | `pipeline.rs:787-854` |
| Double underline | **DONE** | `pipeline.rs:855-973` |
| Curly underline | **DONE** | `pipeline.rs:975-1060` |
| Strikethrough | **DONE** | `pipeline.rs:1062-1125` |
| Underline color | **DONE** | SGR 58/59 support via `underline_color` |

### P2: Background Image Support - **COMPLETE**

| Function | Status | Location |
|----------|--------|----------|
| `dterm_renderer_set_background_image` | **DONE** | `gpu/ffi.rs:1134` |
| `dterm_renderer_clear_background_image` | **DONE** | `gpu/ffi.rs:1166` |
| `DtermBlendMode` enum | **DONE** | `gpu/types.rs:21-32` |

---

## Swift Bindings - COMPLETE

All FFI functions are wrapped in Swift at:
`packages/dterm-swift/Sources/DTermCore/DTermGPURenderer.swift`

Three abstraction levels available:

1. **`DTermFrameSync`** - Frame synchronization only (993 lines)
   - Safe replacement for `dispatch_group`
   - Cannot crash with "unbalanced" errors
   - Timeout handling is safe

2. **`DTermHybridRenderer`** - Vertex/uniform generation
   - Generates vertex data for platform-specific rendering
   - Platform glyphs mode for CoreText fonts
   - Atlas management with incremental updates

3. **`DTermGPURenderer`** - Full wgpu rendering
   - Complete GPU pipeline
   - Requires wgpu device/queue handles

---

## DashTerm2 Integration Guide

### Recommended Approach: Hybrid Renderer

DashTerm2 should use `DTermHybridRenderer` for incremental migration:

```swift
import DTermCore

// 1. Create hybrid renderer
let renderer = DTermHybridRenderer()!

// 2. Enable platform glyphs (use CoreText for font rendering)
// This lets DashTerm2 keep using its existing font stack
dterm_hybrid_renderer_enable_platform_glyphs(renderer.handle, true)

// 3. Configure cell size from your existing font metrics
dterm_hybrid_renderer_set_platform_cell_size(renderer.handle, cellWidth, cellHeight)

// 4. Build vertex data from terminal state
let vertexCount = renderer.build(terminal: terminalPtr)

// 5. Get vertices and upload to Metal buffer
let vertices = renderer.vertices()
// Upload to MTLBuffer...

// 6. Process pending glyphs (add to your atlas)
for glyph in renderer.pendingGlyphs() {
    // Upload glyph.bitmap to your texture atlas
}
renderer.clearPendingGlyphs()

// 7. Render using your Metal pipeline
// (vertices use same format as iTerm2 expectations)
```

### Metal Shader Flag Layout (Action Required)

DashTerm2's Metal shader must use the **new 7-bit vertex flag layout**.
See `docs/METAL_SHADER_MIGRATION.md` for the exact constants, shader template,
and migration checklist.

### Frame Synchronization (Safe Replacement for dispatch_group)

```swift
let sync = DTermFrameSync()

// Request frame
let frame = sync.requestFrame()

// When CAMetalLayer provides drawable:
sync.completeFrame(frame)

// Wait with safe timeout (cannot crash!)
let status = sync.waitForFrame(timeoutMs: 16)
switch status {
case .ready:
    // Render
case .timeout:
    // Skip frame (no cleanup needed, no crash possible)
case .cancelled:
    // Frame was cancelled
}
```

---

## What DashTerm2 Can Delete After Integration

Once integration is validated, DashTerm2 can delete:

- `iTermMetalView.m` (~2000 lines)
- `iTermMetalDriver.m` (~2500 lines)
- `iTermPromise.m` (~500 lines)
- `dispatch_group`-based frame sync code
- ObjC vertex buffer generation

Replace with ~200 lines of Swift calling dterm-core.

---

## Next Steps for DashTerm2

1. **Test Frame Sync** - Replace `dispatch_group` with `DTermFrameSync`
   - This alone fixes the crash bugs
   - No rendering changes needed initially

2. **Test Hybrid Renderer** - Validate vertex output
   - Compare vertices to current iTerm2 output
   - Use existing Metal shaders initially

3. **Migrate Rendering** - Switch to dterm-core vertex data
   - Keep your Metal pipeline
   - Use dterm-core for vertex generation

4. **Delete ObjC** - Remove legacy code
   - After validation passes

---

## Questions Answered

### Q: Metal texture interop?
A: Use `dterm_renderer_provide_drawable` - pass `CAMetalDrawable.texture` as opaque pointer.

### Q: Frame ownership?
A: Swift owns the drawable. Get from `CAMetalLayer.nextDrawable()`, pass to Rust via `provide_drawable`, call `present()` after Rust render completes.

### Q: Font fallback?
A: Use platform glyphs mode. Swift handles font fallback via CoreText, registers glyphs with `dterm_hybrid_renderer_add_platform_glyph`.

---

## CI Blocker (Separate Issue)

GitHub Actions hosted runners are disabled for this repository. This blocks Windows/Linux CI but does NOT block DashTerm2 macOS integration.

**DashTerm2 can proceed with integration testing on macOS.**

See `docs/CI_ALTERNATIVES.md` for CI resolution options.

---

## Summary

**All GPU Renderer FFI requirements are COMPLETE.**

DashTerm2 integration can proceed immediately:

1. Frame sync is safe and tested
2. Hybrid renderer generates compatible vertex data
3. All cursor/selection/decoration features implemented
4. Swift bindings are complete and documented

The terminal for AI agents is ready for integration.

---

*End of MANAGER_DIRECTIVE.md*
