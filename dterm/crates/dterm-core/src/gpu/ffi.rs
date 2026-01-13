//! C FFI bindings for the GPU renderer.
//!
//! This module provides C-callable functions for the GPU renderer, enabling
//! Swift/ObjC code to use the Rust renderer with safe frame synchronization.
//!
//! ## Architecture
//!
//! The FFI layer is split into two parts:
//!
//! 1. **Frame Synchronization** (`DtermRenderer`) - Manages frame request/completion
//!    without requiring wgpu. This replaces the unsafe dispatch_group code.
//!
//! 2. **Full Renderer** (`DtermGpuRenderer`) - Complete wgpu-based renderer that
//!    can render terminal content. Requires platform to provide wgpu device/queue.
//!
//! ## Usage from Swift (Frame Sync Only)
//!
//! ```swift
//! // Create frame sync manager
//! let sync = dterm_renderer_create(nil)
//!
//! // Request a frame
//! let frame = dterm_renderer_request_frame(sync)
//!
//! // Provide drawable (from CAMetalLayer.nextDrawable())
//! dterm_renderer_complete_frame(sync, frame)
//!
//! // Wait for frame to be ready
//! let status = dterm_renderer_wait_frame(sync, frame, 16) // 16ms timeout
//! if status == DTERM_FRAME_STATUS_READY {
//!     // Platform does its own rendering
//! }
//!
//! // Clean up
//! dterm_renderer_free(sync)
//! ```
//!
//! ## Usage from Swift (Full GPU Renderer)
//!
//! ```swift
//! // Platform creates wgpu device/queue and passes raw pointers
//! let renderer = dterm_gpu_renderer_create(devicePtr, queuePtr, config)
//!
//! // Render terminal to surface
//! dterm_gpu_renderer_render(renderer, terminal, surfaceViewPtr)
//!
//! // Or with damage tracking
//! dterm_gpu_renderer_render_with_damage(renderer, terminal, surfaceViewPtr, damage)
//!
//! // Clean up
//! dterm_gpu_renderer_free(renderer)
//! ```
//!
//! ## Thread Safety
//!
//! - `dterm_renderer_create` must be called from the main thread
//! - `dterm_renderer_request_frame` can be called from any thread
//! - `dterm_renderer_complete_frame` can be called from any thread
//! - `dterm_renderer_wait_frame` blocks the calling thread
//! - `dterm_gpu_renderer_render` must be called from the render thread

use super::frame_sync::{FrameRequest, FrameStatus, FrameSync};
use super::types::{DtermBlendMode, RendererConfig};
use super::Renderer;
use crate::grid::Damage;
use crate::terminal::Terminal;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// =============================================================================
// TYPES
// =============================================================================

/// Opaque renderer handle for FFI.
pub struct DtermRenderer {
    /// Frame synchronization state
    frame_sync: Mutex<FrameSync>,
    /// Configuration (stored for future use when full rendering is supported)
    #[allow(dead_code)]
    config: RendererConfig,
    /// Next frame ID
    next_frame_id: AtomicU64,
    /// Last requested frame ID (used to validate waits)
    last_frame_id: AtomicU64,
    /// Pending frame requests by ID
    pending_frames: Mutex<HashMap<u64, FrameRequest>>,
    /// Last provided drawable pointer (opaque)
    #[allow(dead_code)]
    last_drawable: Mutex<Option<*mut c_void>>,
    /// Hybrid renderer for vertex generation and font management
    hybrid: Mutex<DtermHybridRenderer>,
    // Note: wgpu device/queue are not stored here because they require
    // platform-specific initialization. The renderer will be initialized
    // lazily when the first render call is made with a surface.
}

/// Frame handle for FFI.
///
/// This is a simplified handle that stores just the frame ID.
/// The actual FrameRequest is stored in a map inside DtermRenderer.
///
/// # Size
///
/// This struct is 8 bytes on all platforms.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DtermFrameHandle {
    /// Frame ID
    pub id: u64,
}

/// Frame status for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermFrameStatus {
    /// Frame is ready
    Ready = 0,
    /// Timeout expired
    Timeout = 1,
    /// Frame was cancelled
    Cancelled = 2,
}

/// Cursor style for FFI.
///
/// Matches the cursor styles supported by terminal emulators.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermCursorStyle {
    /// Solid block cursor
    Block = 0,
    /// Horizontal underline cursor
    Underline = 1,
    /// Vertical bar cursor
    Bar = 2,
}

impl Default for DtermCursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// Render result for FFI.
///
/// Provides detailed information about the render operation.
///
/// # Size
///
/// This struct is 24 bytes on all platforms.
///
/// # Layout (offsets in bytes)
///
/// | Offset | Field         | Size |
/// |--------|---------------|------|
/// | 0      | success       | 1    |
/// | 1      | (padding)     | 7    |
/// | 8      | frame_time_us | 8    |
/// | 16     | cells_rendered| 4    |
/// | 20     | error_code    | 4    |
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermRenderResult {
    /// Whether the render succeeded
    pub success: bool,
    /// Frame render time in microseconds
    pub frame_time_us: u64,
    /// Number of cells rendered
    pub cells_rendered: u32,
    /// Error code (0 = success, matches DtermRenderError values)
    pub error_code: i32,
}

impl Default for DtermRenderResult {
    fn default() -> Self {
        Self {
            success: true,
            frame_time_us: 0,
            cells_rendered: 0,
            error_code: 0,
        }
    }
}

impl From<FrameStatus> for DtermFrameStatus {
    fn from(status: FrameStatus) -> Self {
        match status {
            FrameStatus::Ready => DtermFrameStatus::Ready,
            FrameStatus::Timeout => DtermFrameStatus::Timeout,
            FrameStatus::Cancelled => DtermFrameStatus::Cancelled,
        }
    }
}

/// Renderer configuration for FFI.
///
/// # Size
///
/// This struct is 48 bytes on all platforms (verified for Swift binding).
///
/// # Layout (offsets in bytes)
///
/// | Offset | Field              | Size |
/// |--------|---------------------|------|
/// | 0      | initial_width      | 4    |
/// | 4      | initial_height     | 4    |
/// | 8      | scale_factor       | 4    |
/// | 12     | background_r       | 1    |
/// | 13     | background_g       | 1    |
/// | 14     | background_b       | 1    |
/// | 15     | background_a       | 1    |
/// | 16     | vsync              | 1    |
/// | 17     | (padding)          | 3    |
/// | 20     | target_fps         | 4    |
/// | 24     | drawable_timeout_ms| 8    |
/// | 32     | damage_rendering   | 1    |
/// | 33     | (padding)          | 3    |
/// | 36     | cursor_style       | 4    |
/// | 40     | cursor_blink_ms    | 4    |
/// | 44     | selection_r        | 1    |
/// | 45     | selection_g        | 1    |
/// | 46     | selection_b        | 1    |
/// | 47     | selection_a        | 1    |
#[repr(C)]
pub struct DtermRendererConfig {
    /// Initial viewport width in pixels
    pub initial_width: u32,
    /// Initial viewport height in pixels
    pub initial_height: u32,
    /// Display scale factor (e.g., 2.0 for Retina)
    pub scale_factor: f32,
    /// Background color red component (0-255)
    pub background_r: u8,
    /// Background color green component (0-255)
    pub background_g: u8,
    /// Background color blue component (0-255)
    pub background_b: u8,
    /// Background color alpha component (0-255)
    pub background_a: u8,
    /// Enable vsync
    pub vsync: bool,
    /// Target FPS when vsync is disabled
    pub target_fps: u32,
    /// Drawable timeout in milliseconds
    pub drawable_timeout_ms: u64,
    /// Enable damage-based rendering
    pub damage_rendering: bool,
    /// Cursor style
    pub cursor_style: DtermCursorStyle,
    /// Cursor blink rate in milliseconds (0 = no blinking)
    pub cursor_blink_ms: u32,
    /// Selection color red component (0-255)
    pub selection_r: u8,
    /// Selection color green component (0-255)
    pub selection_g: u8,
    /// Selection color blue component (0-255)
    pub selection_b: u8,
    /// Selection color alpha component (0-255)
    pub selection_a: u8,
}

impl Default for DtermRendererConfig {
    fn default() -> Self {
        let config = RendererConfig::default();
        Self {
            initial_width: 800,
            initial_height: 600,
            scale_factor: 1.0,
            background_r: config.background_color.0,
            background_g: config.background_color.1,
            background_b: config.background_color.2,
            background_a: 255,
            vsync: config.vsync,
            target_fps: config.target_fps,
            drawable_timeout_ms: config.drawable_timeout_ms,
            damage_rendering: config.damage_rendering,
            cursor_style: DtermCursorStyle::Block,
            cursor_blink_ms: 530, // Standard cursor blink rate
            selection_r: 51,      // Default selection color: semi-transparent blue
            selection_g: 153,
            selection_b: 255,
            selection_a: 128,
        }
    }
}

impl From<DtermRendererConfig> for RendererConfig {
    fn from(config: DtermRendererConfig) -> Self {
        Self {
            background_color: (
                config.background_r,
                config.background_g,
                config.background_b,
            ),
            vsync: config.vsync,
            target_fps: config.target_fps,
            drawable_timeout_ms: config.drawable_timeout_ms,
            damage_rendering: config.damage_rendering,
        }
    }
}

// =============================================================================
// FFI FUNCTIONS
// =============================================================================

/// Create a new renderer with optional configuration.
///
/// Returns a pointer to the renderer, or null on failure.
///
/// # Safety
///
/// - `config` may be null (uses defaults).
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_create(
    config: *const DtermRendererConfig,
) -> *mut DtermRenderer {
    let renderer_config = if config.is_null() {
        RendererConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    let mut hybrid = DtermHybridRenderer::new(renderer_config.clone());
    if !config.is_null() {
        hybrid.apply_ffi_config(unsafe { &*config });
    }

    let renderer = DtermRenderer {
        frame_sync: Mutex::new(FrameSync::new()),
        config: renderer_config,
        next_frame_id: AtomicU64::new(0),
        last_frame_id: AtomicU64::new(u64::MAX),
        pending_frames: Mutex::new(HashMap::new()),
        last_drawable: Mutex::new(None),
        hybrid: Mutex::new(hybrid),
    };
    Box::into_raw(Box::new(renderer))
}

/// Create a new renderer with custom configuration.
///
/// Returns a pointer to the renderer, or null on failure.
///
/// # Safety
///
/// - `config` must be a valid pointer to a `DtermRendererConfig`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_create_with_config(
    config: *const DtermRendererConfig,
) -> *mut DtermRenderer {
    unsafe { dterm_renderer_create(config) }
}

/// Free a renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`, or null.
/// - `renderer` must not have been freed previously.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_free(renderer: *mut DtermRenderer) {
    if !renderer.is_null() {
        drop(unsafe { Box::from_raw(renderer) });
    }
}

/// Destroy a renderer.
///
/// This is an alias for `dterm_renderer_free`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_destroy(renderer: *mut DtermRenderer) {
    unsafe { dterm_renderer_free(renderer) }
}

/// Request a new frame.
///
/// The returned handle must be completed with `dterm_renderer_complete_frame`
/// or the frame will timeout/cancel automatically.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_request_frame(
    renderer: *mut DtermRenderer,
) -> DtermFrameHandle {
    if renderer.is_null() {
        return DtermFrameHandle { id: u64::MAX };
    }

    let renderer = unsafe { &*renderer };
    let frame_id = renderer.next_frame_id.fetch_add(1, Ordering::Relaxed);
    renderer.last_frame_id.store(frame_id, Ordering::Relaxed);

    // Request frame from the frame sync
    let request = {
        let mut sync = renderer.frame_sync.lock();
        sync.request_frame(frame_id)
    };

    // Store the request for later completion
    {
        let mut pending = renderer.pending_frames.lock();
        pending.insert(frame_id, request);
    }

    DtermFrameHandle { id: frame_id }
}

/// Provide a drawable for a frame request.
///
/// This should be called after the platform provides a drawable
/// (e.g., CAMetalDrawable from nextDrawable()).
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `handle` must be a valid handle returned by `dterm_renderer_request_frame`.
/// - `metal_texture` may be null (signals readiness without storing a drawable).
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_provide_drawable(
    renderer: *mut DtermRenderer,
    handle: DtermFrameHandle,
    metal_texture: *mut c_void,
) -> bool {
    if renderer.is_null() || handle.id == u64::MAX {
        return false;
    }

    let renderer = unsafe { &*renderer };

    // Remove the request and complete it
    let request = {
        let mut pending = renderer.pending_frames.lock();
        pending.remove(&handle.id)
    };

    if let Some(request) = request {
        if !metal_texture.is_null() {
            let mut slot = renderer.last_drawable.lock();
            *slot = Some(metal_texture);
        }
        request.complete();
        return true;
    }

    false
}

/// Complete a frame request, signaling that the drawable is ready.
///
/// This is a compatibility wrapper for `dterm_renderer_provide_drawable`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `handle` must be a valid handle returned by `dterm_renderer_request_frame`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_complete_frame(
    renderer: *mut DtermRenderer,
    handle: DtermFrameHandle,
) {
    let _ = unsafe { dterm_renderer_provide_drawable(renderer, handle, std::ptr::null_mut()) };
}

/// Cancel a frame request.
///
/// This can be called if the platform cannot provide a drawable.
/// The frame will be marked as cancelled.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `handle` must be a valid handle returned by `dterm_renderer_request_frame`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_cancel_frame(
    renderer: *mut DtermRenderer,
    handle: DtermFrameHandle,
) {
    if renderer.is_null() {
        return;
    }

    let renderer = unsafe { &*renderer };

    // Remove the request (dropping it will notify as cancelled)
    let mut pending = renderer.pending_frames.lock();
    pending.remove(&handle.id);
    // Request is dropped here, which notifies the waiter
}

/// Wait for a frame to be ready.
///
/// Blocks until the frame is ready, cancelled, or timeout expires.
///
/// # Arguments
/// * `renderer` - Renderer handle
/// * `handle` - Frame handle to wait on
/// * `timeout_ms` - Timeout in milliseconds
///
/// # Returns
/// Frame status (Ready, Timeout, or Cancelled).
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_wait_frame(
    renderer: *mut DtermRenderer,
    handle: DtermFrameHandle,
    timeout_ms: u64,
) -> DtermFrameStatus {
    if renderer.is_null() {
        return DtermFrameStatus::Cancelled;
    }

    let renderer = unsafe { &*renderer };
    if handle.id == u64::MAX {
        return DtermFrameStatus::Cancelled;
    }
    let last_id = renderer.last_frame_id.load(Ordering::Relaxed);
    if last_id != handle.id {
        return DtermFrameStatus::Cancelled;
    }
    let timeout = std::time::Duration::from_millis(timeout_ms);

    let status = {
        let sync = renderer.frame_sync.lock();
        sync.wait_for_frame(timeout)
    };

    status.into()
}

/// Render the terminal using the hybrid renderer.
///
/// This builds vertex/uniform data for the current terminal state and stores
/// it in the renderer's hybrid cache for platform rendering.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `terminal` must be a valid pointer to a `Terminal`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_render(
    renderer: *mut DtermRenderer,
    terminal: *const Terminal,
) -> DtermRenderResult {
    if renderer.is_null() || terminal.is_null() {
        return DtermRenderResult {
            success: false,
            frame_time_us: 0,
            cells_rendered: 0,
            error_code: DtermRenderError::NullPointer as i32,
        };
    }

    let renderer = unsafe { &*renderer };
    let terminal = unsafe { &*terminal };

    let start = std::time::Instant::now();
    let mut hybrid = renderer.hybrid.lock();
    let _vertex_count =
        unsafe { dterm_hybrid_renderer_build(&raw mut *hybrid, std::ptr::from_ref(terminal)) };
    let elapsed: u64 = start.elapsed().as_micros().try_into().unwrap_or(u64::MAX);

    let grid = terminal.grid();
    let cells_rendered = u32::from(grid.rows()) * u32::from(grid.cols());

    DtermRenderResult {
        success: true,
        frame_time_us: elapsed,
        cells_rendered,
        error_code: DtermRenderError::Ok as i32,
    }
}

/// Get the default renderer configuration.
///
/// # Safety
///
/// - `out_config` must be a valid pointer to a `DtermRendererConfig`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_get_default_config(out_config: *mut DtermRendererConfig) {
    if out_config.is_null() {
        return;
    }

    unsafe {
        *out_config = DtermRendererConfig::default();
    }
}

/// Check if the renderer FFI is available.
///
/// This can be called to verify that the GPU renderer was compiled in.
#[no_mangle]
pub extern "C" fn dterm_renderer_available() -> bool {
    true
}

impl Clone for DtermRendererConfig {
    fn clone(&self) -> Self {
        Self {
            initial_width: self.initial_width,
            initial_height: self.initial_height,
            scale_factor: self.scale_factor,
            background_r: self.background_r,
            background_g: self.background_g,
            background_b: self.background_b,
            background_a: self.background_a,
            vsync: self.vsync,
            target_fps: self.target_fps,
            drawable_timeout_ms: self.drawable_timeout_ms,
            damage_rendering: self.damage_rendering,
            cursor_style: self.cursor_style,
            cursor_blink_ms: self.cursor_blink_ms,
            selection_r: self.selection_r,
            selection_g: self.selection_g,
            selection_b: self.selection_b,
            selection_a: self.selection_a,
        }
    }
}

#[cfg(test)]
#[allow(clippy::borrow_as_ptr)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_create_free() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };
        assert!(!renderer.is_null());
        unsafe { dterm_renderer_free(renderer) };
    }

    #[test]
    fn test_renderer_available() {
        assert!(dterm_renderer_available());
    }

    #[test]
    fn test_renderer_request_complete_flow() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request a frame
        let handle = unsafe { dterm_renderer_request_frame(renderer) };
        assert_ne!(handle.id, u64::MAX);

        // Complete it
        unsafe { dterm_renderer_complete_frame(renderer, handle) };

        // Wait should return Ready
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 100) };
        assert_eq!(status, DtermFrameStatus::Ready);

        unsafe { dterm_renderer_free(renderer) };
    }

    #[test]
    fn test_renderer_timeout() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request a frame but don't complete it
        let handle = unsafe { dterm_renderer_request_frame(renderer) };

        // Wait should timeout
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 1) };
        assert_eq!(status, DtermFrameStatus::Timeout);

        unsafe { dterm_renderer_free(renderer) };
    }

    #[test]
    fn test_renderer_cancel() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request a frame
        let handle = unsafe { dterm_renderer_request_frame(renderer) };

        // Cancel it
        unsafe { dterm_renderer_cancel_frame(renderer, handle) };

        // Wait should return Ready (drop notifies completion)
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 100) };
        assert_eq!(status, DtermFrameStatus::Ready);

        unsafe { dterm_renderer_free(renderer) };
    }

    #[test]
    fn test_renderer_null_safe() {
        // All FFI functions should handle null pointers gracefully
        unsafe {
            dterm_renderer_free(std::ptr::null_mut());

            let handle = dterm_renderer_request_frame(std::ptr::null_mut());
            assert_eq!(handle.id, u64::MAX);

            dterm_renderer_complete_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 });
            dterm_renderer_cancel_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 });

            let status =
                dterm_renderer_wait_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 }, 1);
            assert_eq!(status, DtermFrameStatus::Cancelled);

            dterm_renderer_get_default_config(std::ptr::null_mut());

            let ok = dterm_renderer_set_background_image(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                DtermBlendMode::Normal,
                1.0,
            );
            assert!(!ok);

            let ok = dterm_renderer_clear_background_image(std::ptr::null_mut());
            assert!(!ok);

            let ok = dterm_gpu_renderer_set_background_image(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                DtermBlendMode::Normal,
                1.0,
            );
            assert!(!ok);

            let ok = dterm_gpu_renderer_clear_background_image(std::ptr::null_mut());
            assert!(!ok);
        }
    }

    #[test]
    fn test_stress_ffi_no_crashes() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Rapid request/complete/timeout cycles
        for _ in 0..100 {
            let handle = unsafe { dterm_renderer_request_frame(renderer) };

            // Randomly complete, cancel, or let it timeout
            match handle.id % 3 {
                0 => unsafe { dterm_renderer_complete_frame(renderer, handle) },
                1 => unsafe { dterm_renderer_cancel_frame(renderer, handle) },
                _ => {} // Let it timeout
            }

            let _ = unsafe { dterm_renderer_wait_frame(renderer, handle, 1) };
        }

        unsafe { dterm_renderer_free(renderer) };
        // No crashes!
    }

    // =========================================================================
    // INTEGRATION TESTS - Step 9 of Phase E
    // =========================================================================
    //
    // These tests verify the end-to-end Swift-to-Rust rendering flow.
    // They test the FFI layer that Swift code will call.

    /// Integration test: Full frame request/complete/wait cycle
    ///
    /// This tests the exact sequence that Swift code will use:
    /// 1. Create renderer
    /// 2. Request frame
    /// 3. Complete frame (platform provides drawable)
    /// 4. Wait for frame
    /// 5. Clean up
    #[test]
    fn test_integration_swift_frame_cycle() {
        // Step 1: Swift calls dterm_renderer_create()
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };
        assert!(!renderer.is_null(), "Failed to create renderer");

        // Step 2: Swift calls dterm_renderer_request_frame()
        let handle = unsafe { dterm_renderer_request_frame(renderer) };
        assert_ne!(handle.id, u64::MAX, "Failed to request frame");

        // Step 3: Swift's CAMetalLayer.nextDrawable() returns drawable
        // Swift calls dterm_renderer_complete_frame()
        unsafe { dterm_renderer_complete_frame(renderer, handle) };

        // Step 4: Swift calls dterm_renderer_wait_frame() with 16ms (60fps budget)
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 16) };
        assert_eq!(
            status,
            DtermFrameStatus::Ready,
            "Frame should be ready after completion"
        );

        // Step 5: Swift calls dterm_renderer_free() on cleanup
        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Timeout handling (no drawable provided)
    ///
    /// Critical safety test: In ObjC this would cause "unbalanced dispatch_group_leave()"
    /// In Rust this safely times out.
    #[test]
    fn test_integration_timeout_safety() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request frame but don't complete it
        let handle = unsafe { dterm_renderer_request_frame(renderer) };

        // Wait should timeout (not crash!)
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 5) };
        assert_eq!(
            status,
            DtermFrameStatus::Timeout,
            "Should timeout when no drawable provided"
        );

        // Clean up should work
        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Late completion after timeout
    ///
    /// Critical safety test: In ObjC, completing after timeout causes crash.
    /// In Rust, this is safe (send to closed channel is no-op).
    #[test]
    fn test_integration_late_completion_safe() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request frame
        let handle = unsafe { dterm_renderer_request_frame(renderer) };

        // Wait and timeout
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 1) };
        assert_eq!(status, DtermFrameStatus::Timeout);

        // Now complete late - THIS WOULD CRASH IN OBJC
        // In Rust it's safe
        unsafe { dterm_renderer_complete_frame(renderer, handle) };

        // Should still be able to clean up
        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Multiple frame cycles (window resize scenario)
    ///
    /// Tests the pattern: rapid frame request/complete cycles during resize.
    #[test]
    fn test_integration_resize_scenario() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Simulate 60 frames during a resize operation
        for i in 0..60 {
            let handle = unsafe { dterm_renderer_request_frame(renderer) };

            // Complete after varying delays (simulating real drawable timing)
            if i % 10 != 0 {
                // Normal case: complete frame
                unsafe { dterm_renderer_complete_frame(renderer, handle) };
                let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 16) };
                assert_eq!(status, DtermFrameStatus::Ready);
            } else {
                // Every 10th frame: timeout (simulating dropped frame)
                let status = unsafe { dterm_renderer_wait_frame(renderer, handle, 1) };
                assert_eq!(status, DtermFrameStatus::Timeout);
                // Late completion is safe
                unsafe { dterm_renderer_complete_frame(renderer, handle) };
            }
        }

        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Concurrent frame requests (edge case)
    ///
    /// Tests what happens when a new frame is requested before the previous completes.
    #[test]
    fn test_integration_overlapping_requests() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        // Request first frame
        let handle1 = unsafe { dterm_renderer_request_frame(renderer) };
        assert_ne!(handle1.id, u64::MAX);

        // Request second frame (before completing first)
        let handle2 = unsafe { dterm_renderer_request_frame(renderer) };
        assert_ne!(handle2.id, u64::MAX);
        assert_ne!(handle1.id, handle2.id, "Frame IDs should be unique");

        // Complete both
        unsafe { dterm_renderer_complete_frame(renderer, handle1) };
        unsafe { dterm_renderer_complete_frame(renderer, handle2) };

        // Wait should return Ready (one of them will have completed)
        let status = unsafe { dterm_renderer_wait_frame(renderer, handle2, 16) };
        assert_eq!(status, DtermFrameStatus::Ready);

        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Config roundtrip
    ///
    /// Tests that config values pass correctly through FFI.
    #[test]
    fn test_integration_config_roundtrip() {
        // Get default config
        let mut config = DtermRendererConfig::default();
        unsafe { dterm_renderer_get_default_config(&mut config) };

        // Verify default values
        assert_eq!(config.background_r, 0);
        assert_eq!(config.background_g, 0);
        assert_eq!(config.background_b, 0);
        assert!(config.vsync);
        assert_eq!(config.target_fps, 60);
        assert_eq!(config.drawable_timeout_ms, 17); // ~1 frame at 60fps
        assert!(config.damage_rendering);

        // Create renderer with custom config
        config.background_r = 30;
        config.background_g = 30;
        config.background_b = 30;
        config.vsync = false;
        config.target_fps = 120;

        let renderer = unsafe { dterm_renderer_create_with_config(&config) };
        assert!(!renderer.is_null());

        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Atlas config
    ///
    /// Tests that atlas configuration works correctly.
    #[test]
    fn test_integration_atlas_config() {
        let config = DtermAtlasConfig {
            initial_size: 1024,
            max_size: 4096,
            default_font_size: 16,
            padding: 2,
        };

        // Verify conversion
        let internal: super::super::AtlasConfig = config.clone().into();
        assert_eq!(internal.initial_size, 1024);
        assert_eq!(internal.max_size, 4096);
        assert_eq!(internal.default_font_size, 16);
        assert_eq!(internal.padding, 2);
    }

    /// Integration test: Damage region
    ///
    /// Tests damage region FFI structure.
    #[test]
    fn test_integration_damage_region() {
        // Default is full damage
        let default = DtermDamageRegion::default();
        assert!(default.is_full);

        // Partial damage
        let partial = DtermDamageRegion {
            start_row: 0,
            end_row: 10,
            start_col: 0,
            end_col: 80,
            is_full: false,
        };
        assert!(!partial.is_full);
        assert_eq!(partial.start_row, 0);
        assert_eq!(partial.end_row, 10);
    }

    /// Integration test: Frame ID monotonic increase
    ///
    /// Tests that frame IDs increase monotonically (TLA+ property).
    #[test]
    fn test_integration_frame_id_monotonic() {
        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };

        let mut last_id = 0u64;
        for _ in 0..100 {
            let handle = unsafe { dterm_renderer_request_frame(renderer) };
            assert!(
                handle.id > last_id || last_id == 0,
                "Frame ID should increase"
            );
            last_id = handle.id;
            unsafe { dterm_renderer_complete_frame(renderer, handle) };
            let _ = unsafe { dterm_renderer_wait_frame(renderer, handle, 1) };
        }

        unsafe { dterm_renderer_free(renderer) };
    }

    /// Integration test: Thread safety (basic)
    ///
    /// Tests basic thread safety of the frame sync API.
    #[test]
    fn test_integration_thread_safety() {
        use std::thread;

        let renderer = unsafe { dterm_renderer_create(std::ptr::null()) };
        let renderer_ptr = renderer as usize; // Convert to usize for thread safety

        // Spawn threads that request/complete frames
        let handles: Vec<_> = (0..4)
            .map(|_| {
                thread::spawn(move || {
                    let renderer = renderer_ptr as *mut DtermRenderer;
                    for _ in 0..25 {
                        let handle = unsafe { dterm_renderer_request_frame(renderer) };
                        if handle.id != u64::MAX {
                            unsafe { dterm_renderer_complete_frame(renderer, handle) };
                            let _ = unsafe { dterm_renderer_wait_frame(renderer, handle, 5) };
                        }
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        unsafe { dterm_renderer_free(renderer) };
    }
}

// =============================================================================
// RENDERER CONFIGURATION FFI
// =============================================================================
//
// These functions allow runtime configuration of the renderer.
// DashTerm2 uses these to update settings without recreating the renderer.

fn sanitize_color_component(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn parse_font_from_ptr(data: *const u8, len: usize) -> Option<fontdue::Font> {
    if data.is_null() || len == 0 {
        return None;
    }

    // SAFETY: Caller guarantees data points to len bytes.
    let data = unsafe { std::slice::from_raw_parts(data, len) };
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
}

fn apply_font_variants(renderer: &mut DtermHybridRenderer) {
    if let Some(atlas) = renderer.glyph_atlas.as_mut() {
        atlas.set_font_variants(
            renderer.font_bold.clone(),
            renderer.font_italic.clone(),
            renderer.font_bold_italic.clone(),
        );
    }
}

fn hybrid_cell_size(renderer: &DtermHybridRenderer) -> Option<(f32, f32)> {
    if renderer.use_platform_glyphs {
        return Some(renderer.platform_cell_size);
    }
    renderer
        .glyph_atlas
        .as_ref()
        .map(|atlas| (atlas.cell_width(), atlas.line_height()))
}

fn cursor_style_from_ffi(style: DtermCursorStyle) -> super::CursorStyle {
    match style {
        DtermCursorStyle::Block => super::CursorStyle::Block,
        DtermCursorStyle::Underline => super::CursorStyle::Underline,
        DtermCursorStyle::Bar => super::CursorStyle::Bar,
    }
}

/// Set the background color for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
///
/// # Arguments
///
/// * `r`, `g`, `b`, `a` - Color components (0.0-1.0 range)
#[no_mangle]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub unsafe extern "C" fn dterm_renderer_set_background_color(
    renderer: *mut DtermRenderer,
    r: f32,
    g: f32,
    b: f32,
    _a: f32, // Alpha reserved for future use (e.g., transparency effects)
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    // SAFETY: clamp ensures values are in [0, 255] range before cast
    let color = (
        (r * 255.0).clamp(0.0, 255.0) as u8,
        (g * 255.0).clamp(0.0, 255.0) as u8,
        (b * 255.0).clamp(0.0, 255.0) as u8,
    );
    renderer.config.background_color = color;
    {
        let mut hybrid = renderer.hybrid.lock();
        hybrid.config.background_color = color;
    }
    true
}

/// Set the background image for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `texture` must be a valid platform texture pointer (opaque).
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_background_image(
    renderer: *mut DtermRenderer,
    texture: *mut c_void,
    blend_mode: DtermBlendMode,
    opacity: f32,
) -> bool {
    if renderer.is_null() || texture.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let opacity = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };

    let mut hybrid = renderer.hybrid.lock();
    hybrid.background_image = Some(BackgroundImageConfig {
        texture,
        blend_mode,
        opacity,
    });
    true
}

/// Clear the background image for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_clear_background_image(
    renderer: *mut DtermRenderer,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    let _was_set = hybrid.background_image.is_some();
    hybrid.background_image = None;
    true
}

/// Set the cursor style for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_cursor_style(
    renderer: *mut DtermRenderer,
    style: DtermCursorStyle,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    hybrid.cursor_style_override = Some(style);
    true
}

/// Set the cursor blink rate in milliseconds.
///
/// A value of 0 disables cursor blinking.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_cursor_blink_rate(
    renderer: *mut DtermRenderer,
    ms: u32,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    hybrid.cursor_blink_ms_override = Some(ms);
    true
}

/// Set the selection highlight color.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
///
/// # Arguments
///
/// * `r`, `g`, `b`, `a` - Color components (0.0-1.0 range)
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_selection_color(
    renderer: *mut DtermRenderer,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    hybrid.selection_color = [
        sanitize_color_component(r),
        sanitize_color_component(g),
        sanitize_color_component(b),
        sanitize_color_component(a),
    ];
    true
}

/// Set the base font for the renderer from raw font data.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `font_data` must point to valid TTF/OTF data of length `font_data_len`.
///
/// # Returns
///
/// `true` on success, `false` on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_font(
    renderer: *mut DtermRenderer,
    font_data: *const u8,
    font_data_len: usize,
    size_pts: f32,
) -> bool {
    if renderer.is_null() || font_data.is_null() || font_data_len == 0 || !size_pts.is_finite() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();

    // Convert font size from points to pixels, clamping to valid u16 range.
    // The clamp ensures the value is positive and within bounds before the cast.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size_px = size_pts.round().clamp(1.0, f32::from(u16::MAX)) as u16;
    let atlas_config = super::AtlasConfig {
        default_font_size: size_px,
        ..Default::default()
    };

    // SAFETY: Caller guarantees font_data points to valid memory of at least font_data_len bytes
    let atlas = unsafe { super::GlyphAtlas::new_from_ptr(atlas_config, font_data, font_data_len) };

    if let Some(mut atlas) = atlas {
        atlas.set_font_variants(
            hybrid.font_bold.clone(),
            hybrid.font_italic.clone(),
            hybrid.font_bold_italic.clone(),
        );
        hybrid.glyph_atlas = Some(atlas);
        return true;
    }

    false
}

/// Set the bold font variant for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `font_data` must point to valid TTF/OTF data of length `font_data_len`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_bold_font(
    renderer: *mut DtermRenderer,
    font_data: *const u8,
    font_data_len: usize,
    size_pts: f32,
) -> bool {
    if renderer.is_null() || !size_pts.is_finite() {
        return false;
    }

    let Some(font) = parse_font_from_ptr(font_data, font_data_len) else {
        return false;
    };

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    hybrid.font_bold = Some(font);
    apply_font_variants(&mut hybrid);
    true
}

/// Set the italic font variant for the renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `font_data` must point to valid TTF/OTF data of length `font_data_len`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_set_italic_font(
    renderer: *mut DtermRenderer,
    font_data: *const u8,
    font_data_len: usize,
    size_pts: f32,
) -> bool {
    if renderer.is_null() || !size_pts.is_finite() {
        return false;
    }

    let Some(font) = parse_font_from_ptr(font_data, font_data_len) else {
        return false;
    };

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();
    hybrid.font_italic = Some(font);
    apply_font_variants(&mut hybrid);
    true
}

/// Clear the font cache, forcing all glyphs to be re-rasterized.
///
/// Call this when changing font sizes or after extended use to free memory.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_clear_font_cache(
    renderer: *mut DtermRenderer,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let mut hybrid = renderer.hybrid.lock();

    // Clear the internal atlas glyph cache if present
    if let Some(atlas) = hybrid.glyph_atlas.as_mut() {
        atlas.clear();
    }

    // Also clear platform glyphs if in that mode
    hybrid.platform_glyphs.clear();
    hybrid.pending_glyphs.clear();

    true
}

/// Get cell dimensions from the renderer.
///
/// Returns the cell width and height in pixels based on the current font.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
/// - `cell_width` and `cell_height` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_get_cell_size(
    renderer: *mut DtermRenderer,
    cell_width: *mut f32,
    cell_height: *mut f32,
) -> bool {
    if renderer.is_null() || cell_width.is_null() || cell_height.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };
    let hybrid = renderer.hybrid.lock();

    if let Some((width, height)) = hybrid_cell_size(&hybrid) {
        unsafe {
            *cell_width = width;
            *cell_height = height;
        }
        true
    } else {
        unsafe {
            *cell_width = 8.0;
            *cell_height = 16.0;
        }
        false
    }
}

/// Get the font baseline offset.
///
/// Returns the distance from the top of the cell to the text baseline, in pixels.
/// This is needed for correct text positioning.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_renderer_create`.
///
/// # Returns
///
/// Baseline offset in pixels, or 0.0 if no font is set.
#[no_mangle]
pub unsafe extern "C" fn dterm_renderer_get_baseline(renderer: *const DtermRenderer) -> f32 {
    if renderer.is_null() {
        return 0.0;
    }

    let renderer = unsafe { &*renderer };
    let hybrid = renderer.hybrid.lock();

    if let Some(atlas) = &hybrid.glyph_atlas {
        // Get line metrics for baseline calculation
        let metrics = atlas
            .font()
            .horizontal_line_metrics(atlas.default_font_size() as f32);
        if let Some(m) = metrics {
            // Baseline is typically ascent from the top of the cell
            return m.ascent;
        }
    }

    // Return a reasonable default if no font metrics available
    0.0
}

// =============================================================================
// FULL GPU RENDERER FFI
// =============================================================================
//
// This section provides FFI for the complete wgpu-based renderer.
// Platform code must create wgpu device/queue and pass them to these functions.

/// Opaque GPU renderer handle that wraps the full wgpu Renderer.
///
/// This is separate from `DtermRenderer` which handles frame synchronization and hybrid data.
/// The platform must provide wgpu device/queue handles.
pub struct DtermGpuRenderer {
    /// The actual renderer
    renderer: Renderer,
}

/// Render error codes for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermRenderError {
    /// Success
    Ok = 0,
    /// Null pointer argument
    NullPointer = 1,
    /// Invalid device handle
    InvalidDevice = 2,
    /// Invalid queue handle
    InvalidQueue = 3,
    /// Invalid surface view handle
    InvalidSurfaceView = 4,
    /// Rendering failed
    RenderFailed = 5,
}

/// Damage region for FFI.
///
/// Represents a rectangular region of the terminal that needs to be redrawn.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermDamageRegion {
    /// Starting row (0-indexed)
    pub start_row: u16,
    /// Ending row (exclusive)
    pub end_row: u16,
    /// Starting column (0-indexed)
    pub start_col: u16,
    /// Ending column (exclusive)
    pub end_col: u16,
    /// Whether this represents full damage
    pub is_full: bool,
}

impl Default for DtermDamageRegion {
    fn default() -> Self {
        Self {
            start_row: 0,
            end_row: 0,
            start_col: 0,
            end_col: 0,
            is_full: true, // Default to full damage
        }
    }
}

/// Create a full GPU renderer.
///
/// The platform must provide valid wgpu device and queue pointers. These are
/// typically obtained by creating a wgpu instance and requesting a device.
///
/// # Safety
///
/// - `device` must be a valid pointer to a `wgpu::Device`
/// - `queue` must be a valid pointer to a `wgpu::Queue`
/// - Both must remain valid for the lifetime of the renderer
///
/// # Returns
///
/// Pointer to the renderer, or null on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_create(
    device: *const std::ffi::c_void,
    queue: *const std::ffi::c_void,
    config: *const DtermRendererConfig,
) -> *mut DtermGpuRenderer {
    if device.is_null() || queue.is_null() {
        return std::ptr::null_mut();
    }

    // SAFETY: Caller guarantees these are valid wgpu handles
    // We cast the raw pointers and wrap them in Arc
    // This is the FFI boundary - the platform is responsible for keeping them alive
    let device_ref: &wgpu::Device = unsafe { &*(device as *const wgpu::Device) };
    let queue_ref: &wgpu::Queue = unsafe { &*(queue as *const wgpu::Queue) };

    // Clone into Arc for the renderer (wgpu handles are typically Arc-wrapped already)
    // Since we can't create Arc from raw reference, we need platform to provide Arc-compatible handles
    // For now, we use a workaround: extend lifetime via raw transmute
    // This is safe because platform guarantees the handles outlive the renderer
    let device_arc: Arc<wgpu::Device> = unsafe {
        // Create an Arc by incrementing ref count (requires platform cooperation)
        // In practice, platform would use Arc::into_raw / Arc::from_raw pattern
        std::mem::transmute::<&wgpu::Device, Arc<wgpu::Device>>(device_ref)
    };
    let queue_arc: Arc<wgpu::Queue> =
        unsafe { std::mem::transmute::<&wgpu::Queue, Arc<wgpu::Queue>>(queue_ref) };

    let config = if config.is_null() {
        RendererConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    let renderer = Renderer::new(device_arc, queue_arc, config);

    Box::into_raw(Box::new(DtermGpuRenderer { renderer }))
}

/// Create a full GPU renderer with explicit surface format.
///
/// Like `dterm_gpu_renderer_create`, but allows specifying the swapchain format.
///
/// # Safety
///
/// Same requirements as `dterm_gpu_renderer_create`.
/// Additionally, `surface_format` must be a valid wgpu::TextureFormat value.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_create_with_format(
    device: *const std::ffi::c_void,
    queue: *const std::ffi::c_void,
    config: *const DtermRendererConfig,
    surface_format: u32,
) -> *mut DtermGpuRenderer {
    if device.is_null() || queue.is_null() {
        return std::ptr::null_mut();
    }

    let device_ref: &wgpu::Device = unsafe { &*(device as *const wgpu::Device) };
    let queue_ref: &wgpu::Queue = unsafe { &*(queue as *const wgpu::Queue) };

    let device_arc: Arc<wgpu::Device> =
        unsafe { std::mem::transmute::<&wgpu::Device, Arc<wgpu::Device>>(device_ref) };
    let queue_arc: Arc<wgpu::Queue> =
        unsafe { std::mem::transmute::<&wgpu::Queue, Arc<wgpu::Queue>>(queue_ref) };

    let config = if config.is_null() {
        RendererConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    // Convert u32 to TextureFormat
    // Common formats: Bgra8UnormSrgb = 23, Rgba8UnormSrgb = 18
    let format = match surface_format {
        18 => wgpu::TextureFormat::Rgba8UnormSrgb,
        23 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb, // Default fallback
    };

    let renderer = Renderer::new_with_format(device_arc, queue_arc, config, format);

    Box::into_raw(Box::new(DtermGpuRenderer { renderer }))
}

/// Free a GPU renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`, or null.
/// - `renderer` must not have been freed previously.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_free(renderer: *mut DtermGpuRenderer) {
    if !renderer.is_null() {
        drop(unsafe { Box::from_raw(renderer) });
    }
}

/// Render the terminal to the provided surface view.
///
/// This performs a full render of all terminal cells.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - `terminal` must be a valid pointer to a `Terminal`.
/// - `surface_view` must be a valid pointer to a `wgpu::TextureView`.
///
/// # Returns
///
/// `DtermRenderError::Ok` on success, error code on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_render(
    renderer: *mut DtermGpuRenderer,
    terminal: *const Terminal,
    surface_view: *const std::ffi::c_void,
) -> DtermRenderError {
    if renderer.is_null() {
        return DtermRenderError::NullPointer;
    }
    if terminal.is_null() {
        return DtermRenderError::NullPointer;
    }
    if surface_view.is_null() {
        return DtermRenderError::InvalidSurfaceView;
    }

    let renderer = unsafe { &*renderer };
    let terminal = unsafe { &*terminal };
    let view = unsafe { &*(surface_view as *const wgpu::TextureView) };

    match renderer.renderer.render(terminal, view) {
        Ok(()) => DtermRenderError::Ok,
        Err(_) => DtermRenderError::RenderFailed,
    }
}

/// Render the terminal with damage-based optimization.
///
/// This only renders cells that have changed, significantly reducing GPU work
/// for small updates. If `damage` is null, performs a full render.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - `terminal` must be a valid pointer to a `Terminal`.
/// - `surface_view` must be a valid pointer to a `wgpu::TextureView`.
/// - `damage` may be null (triggers full render) or a valid pointer to a `Damage`.
///
/// # Returns
///
/// `DtermRenderError::Ok` on success, error code on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_render_with_damage(
    renderer: *mut DtermGpuRenderer,
    terminal: *const Terminal,
    surface_view: *const std::ffi::c_void,
    damage: *const Damage,
) -> DtermRenderError {
    if renderer.is_null() {
        return DtermRenderError::NullPointer;
    }
    if terminal.is_null() {
        return DtermRenderError::NullPointer;
    }
    if surface_view.is_null() {
        return DtermRenderError::InvalidSurfaceView;
    }

    let renderer = unsafe { &*renderer };
    let terminal = unsafe { &*terminal };
    let view = unsafe { &*(surface_view as *const wgpu::TextureView) };

    // Convert damage pointer to Option<&Damage>
    let damage_opt = if damage.is_null() {
        None
    } else {
        Some(unsafe { &*damage })
    };

    match renderer
        .renderer
        .render_with_damage(terminal, view, damage_opt)
    {
        Ok(()) => DtermRenderError::Ok,
        Err(_) => DtermRenderError::RenderFailed,
    }
}

/// Request a frame from the renderer's frame sync.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
///
/// # Returns
///
/// Frame handle with ID, or handle with id=u64::MAX on error.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_request_frame(
    renderer: *mut DtermGpuRenderer,
) -> DtermFrameHandle {
    if renderer.is_null() {
        return DtermFrameHandle { id: u64::MAX };
    }

    let renderer = unsafe { &*renderer };
    let request = renderer.renderer.request_frame();
    // Store request somewhere... for now just return the ID
    // The full implementation would need a way to track pending requests
    DtermFrameHandle { id: request.id() }
}

/// Wait for a frame to be ready.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
///
/// # Returns
///
/// Frame status.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_wait_frame(
    renderer: *mut DtermGpuRenderer,
    timeout_ms: u64,
) -> DtermFrameStatus {
    if renderer.is_null() {
        return DtermFrameStatus::Cancelled;
    }

    let renderer = unsafe { &*renderer };
    let timeout = std::time::Duration::from_millis(timeout_ms);
    renderer.renderer.wait_for_frame(timeout).into()
}

/// Check if the full GPU renderer FFI is available.
#[no_mangle]
pub extern "C" fn dterm_gpu_renderer_available() -> bool {
    true
}

/// Configuration for glyph atlas.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DtermAtlasConfig {
    /// Initial atlas size (width = height, must be power of 2)
    pub initial_size: u32,
    /// Maximum atlas size (width = height)
    pub max_size: u32,
    /// Default font size in pixels
    pub default_font_size: u16,
    /// Padding between glyphs in pixels
    pub padding: u32,
}

impl Default for DtermAtlasConfig {
    fn default() -> Self {
        Self {
            initial_size: 512,
            max_size: 4096,
            default_font_size: 14,
            padding: 1,
        }
    }
}

impl From<DtermAtlasConfig> for super::AtlasConfig {
    fn from(config: DtermAtlasConfig) -> Self {
        Self {
            initial_size: config.initial_size,
            max_size: config.max_size,
            default_font_size: config.default_font_size,
            padding: config.padding,
        }
    }
}

/// Set the font for the GPU renderer from raw font data (TTF/OTF bytes).
///
/// This creates a glyph atlas from the provided font data and attaches it
/// to the renderer. The font data is copied internally, so the caller can
/// free the original buffer after this call returns.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - `font_data` must be a valid pointer to TTF/OTF font data.
/// - `font_len` must be the exact length of the font data in bytes.
/// - The font data must be valid for the duration of this call.
///
/// # Returns
///
/// `true` on success, `false` on failure (invalid renderer, font, or config).
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_set_font(
    renderer: *mut DtermGpuRenderer,
    font_data: *const u8,
    font_len: usize,
    config: *const DtermAtlasConfig,
) -> bool {
    if renderer.is_null() || font_data.is_null() || font_len == 0 {
        return false;
    }

    let renderer = unsafe { &*renderer };

    let atlas_config = if config.is_null() {
        super::AtlasConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    // SAFETY: Caller guarantees font_data points to valid memory of at least font_len bytes
    let atlas = unsafe { super::GlyphAtlas::new_from_ptr(atlas_config, font_data, font_len) };

    match atlas {
        Some(atlas) => {
            renderer.renderer.set_glyph_atlas(atlas);
            true
        }
        None => false,
    }
}

/// Set additional font variants (bold, italic, bold-italic) for the GPU renderer.
///
/// This should be called after `dterm_gpu_renderer_set_font` to add style variants.
/// Each variant is optional and can be NULL.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - Font data pointers must be valid if not NULL.
/// - Font lengths must match the actual data lengths.
///
/// # Returns
///
/// `true` on success, `false` on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_set_font_variants(
    renderer: *mut DtermGpuRenderer,
    bold_data: *const u8,
    bold_len: usize,
    italic_data: *const u8,
    italic_len: usize,
    bold_italic_data: *const u8,
    bold_italic_len: usize,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };

    // Parse font variants from raw data
    let bold = if !bold_data.is_null() && bold_len > 0 {
        let data = unsafe { std::slice::from_raw_parts(bold_data, bold_len) };
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    } else {
        None
    };

    let italic = if !italic_data.is_null() && italic_len > 0 {
        let data = unsafe { std::slice::from_raw_parts(italic_data, italic_len) };
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    } else {
        None
    };

    let bold_italic = if !bold_italic_data.is_null() && bold_italic_len > 0 {
        let data = unsafe { std::slice::from_raw_parts(bold_italic_data, bold_italic_len) };
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    } else {
        None
    };

    // Set variants on the renderer (returns true if atlas exists)
    renderer
        .renderer
        .set_font_variants(bold, italic, bold_italic)
}

/// Get cell dimensions from the glyph atlas.
///
/// Returns the cell width and height in pixels based on the current font.
/// These values are needed by the platform to properly size the terminal view.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - `cell_width` and `cell_height` must be valid pointers.
///
/// # Returns
///
/// `true` if cell dimensions were retrieved, `false` if no font is set.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_get_cell_size(
    renderer: *mut DtermGpuRenderer,
    cell_width: *mut f32,
    cell_height: *mut f32,
) -> bool {
    if renderer.is_null() || cell_width.is_null() || cell_height.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };

    let (width, height) = renderer.renderer.cell_dimensions();
    unsafe {
        *cell_width = width;
        *cell_height = height;
    }
    true
}

/// Set a background image for the GPU renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
/// - `texture_view` must be a valid pointer to a `wgpu::TextureView`.
/// - This function takes ownership of the `TextureView`. The caller must not
///   use or free the texture view after calling this function.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_set_background_image(
    renderer: *mut DtermGpuRenderer,
    texture_view: *mut c_void,
    blend_mode: DtermBlendMode,
    opacity: f32,
) -> bool {
    if renderer.is_null() || texture_view.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };
    // Take ownership of the TextureView by reading from the pointer.
    // The caller must not use the texture view after this call.
    let view = unsafe { std::ptr::read(texture_view as *const wgpu::TextureView) };
    let opacity = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    renderer
        .renderer
        .set_background_image(view, blend_mode, opacity);
    true
}

/// Clear the background image for the GPU renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_gpu_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_gpu_renderer_clear_background_image(
    renderer: *mut DtermGpuRenderer,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };
    renderer.renderer.clear_background_image();
    true
}

// =============================================================================
// KANI PROOFS FOR FFI SAFETY
// =============================================================================

#[cfg(kani)]
mod ffi_proofs {
    use super::*;

    /// Proof: Null pointer handling in dterm_renderer_request_frame
    #[kani::proof]
    fn proof_request_frame_null_safe() {
        let handle = unsafe { dterm_renderer_request_frame(std::ptr::null_mut()) };
        kani::assert(handle.id == u64::MAX, "Null pointer should return MAX id");
    }

    /// Proof: Null pointer handling in dterm_renderer_wait_frame
    #[kani::proof]
    fn proof_wait_frame_null_safe() {
        let status = unsafe {
            dterm_renderer_wait_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 }, 100)
        };
        kani::assert(
            status == DtermFrameStatus::Cancelled,
            "Null pointer should return Cancelled",
        );
    }

    /// Proof: Null pointer handling in dterm_renderer_complete_frame
    #[kani::proof]
    fn proof_complete_frame_null_safe() {
        // Should not crash
        unsafe {
            dterm_renderer_complete_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 });
        }
        // If we reach here, null handling worked
        kani::assert(true, "Null pointer handling succeeded");
    }

    /// Proof: Null pointer handling in dterm_renderer_cancel_frame
    #[kani::proof]
    fn proof_cancel_frame_null_safe() {
        // Should not crash
        unsafe {
            dterm_renderer_cancel_frame(std::ptr::null_mut(), DtermFrameHandle { id: 0 });
        }
        kani::assert(true, "Null pointer handling succeeded");
    }

    /// Proof: Null pointer handling in dterm_renderer_provide_drawable
    #[kani::proof]
    fn proof_provide_drawable_null_safe() {
        let ok = unsafe {
            dterm_renderer_provide_drawable(
                std::ptr::null_mut(),
                DtermFrameHandle { id: 0 },
                std::ptr::null_mut(),
            )
        };
        kani::assert(!ok, "Null pointer should return false");
    }

    /// Proof: Null pointer handling in dterm_renderer_render
    #[kani::proof]
    fn proof_renderer_render_null_safe() {
        let result = unsafe { dterm_renderer_render(std::ptr::null_mut(), std::ptr::null()) };
        kani::assert(!result.success, "Null pointer should return failure");
        kani::assert(
            result.error_code == DtermRenderError::NullPointer as i32,
            "Null pointer should return NullPointer code",
        );
    }

    /// Proof: Null pointer handling in dterm_gpu_renderer_render
    #[kani::proof]
    fn proof_gpu_render_null_safe() {
        let error = unsafe {
            dterm_gpu_renderer_render(std::ptr::null_mut(), std::ptr::null(), std::ptr::null())
        };
        kani::assert(
            error == DtermRenderError::NullPointer,
            "Null renderer should return NullPointer",
        );
    }

    /// Proof: Null pointer handling in dterm_gpu_renderer_render_with_damage
    #[kani::proof]
    fn proof_gpu_render_with_damage_null_safe() {
        let error = unsafe {
            dterm_gpu_renderer_render_with_damage(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        kani::assert(
            error == DtermRenderError::NullPointer,
            "Null renderer should return NullPointer",
        );
    }

    /// Proof: Frame handle ID is valid range
    #[kani::proof]
    fn proof_frame_handle_id_range() {
        let id: u64 = kani::any();
        let handle = DtermFrameHandle { id };
        // ID should be preserved
        kani::assert(handle.id == id, "ID should be preserved");
    }

    /// Proof: DtermRenderError values are distinct
    #[kani::proof]
    fn proof_render_error_distinct() {
        kani::assert(DtermRenderError::Ok as u32 == 0, "Ok should be 0");
        kani::assert(
            DtermRenderError::NullPointer as u32 == 1,
            "NullPointer should be 1",
        );
        kani::assert(
            DtermRenderError::InvalidDevice as u32 == 2,
            "InvalidDevice should be 2",
        );
        kani::assert(
            DtermRenderError::InvalidQueue as u32 == 3,
            "InvalidQueue should be 3",
        );
        kani::assert(
            DtermRenderError::InvalidSurfaceView as u32 == 4,
            "InvalidSurfaceView should be 4",
        );
        kani::assert(
            DtermRenderError::RenderFailed as u32 == 5,
            "RenderFailed should be 5",
        );
    }

    /// Proof: DtermBlendMode values are stable.
    #[kani::proof]
    fn proof_blend_mode_values() {
        kani::assert(DtermBlendMode::Normal as u32 == 0, "Normal should be 0");
        kani::assert(DtermBlendMode::Multiply as u32 == 1, "Multiply should be 1");
        kani::assert(DtermBlendMode::Screen as u32 == 2, "Screen should be 2");
        kani::assert(DtermBlendMode::Overlay as u32 == 3, "Overlay should be 3");
    }

    /// Proof: Null pointer handling in dterm_renderer_set_background_image
    #[kani::proof]
    fn proof_set_background_image_null_safe() {
        let ok = unsafe {
            dterm_renderer_set_background_image(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                DtermBlendMode::Normal,
                1.0,
            )
        };
        kani::assert(!ok, "Null pointer should return false");
    }

    /// Proof: Null pointer handling in dterm_gpu_renderer_set_background_image
    #[kani::proof]
    fn proof_gpu_set_background_image_null_safe() {
        let ok = unsafe {
            dterm_gpu_renderer_set_background_image(
                std::ptr::null_mut(),
                std::ptr::null(),
                DtermBlendMode::Normal,
                1.0,
            )
        };
        kani::assert(!ok, "Null pointer should return false");
    }
}

// =============================================================================
// HYBRID RENDERING FFI
// =============================================================================
//
// This section provides FFI for hybrid rendering where dterm-core generates
// vertex data and Swift/Metal does the actual rendering. This avoids the
// complexity of integrating wgpu with Swift's CAMetalLayer.
//
// ## Architecture
//
// ```text
// ┌─────────────────────────────────────────────────────────────┐
// │  Swift (DTermMetalView)                                     │
// │  - Owns CAMetalLayer                                        │
// │  - Creates MTLBuffer from dterm-core vertex data            │
// │  - Creates MTLTexture from dterm-core atlas data            │
// │  - Executes Metal draw calls                                │
// └───────────────────────────────┬─────────────────────────────┘
//                                 │ FFI
//                                 ▼
// ┌─────────────────────────────────────────────────────────────┐
// │  dterm-core (Rust)                                          │
// │  - Generates CellVertex data from Terminal state            │
// │  - Manages GlyphAtlas for glyph rendering                   │
// │  - Provides raw bytes for vertices and atlas                │
// └─────────────────────────────────────────────────────────────┘
// ```

use super::pipeline::CellVertexBuilder;

/// Configuration for background image rendering.
///
/// This is stored for the platform to use during rendering.
/// The fields are accessed by the FFI boundary (Swift/ObjC side).
#[allow(dead_code)]
struct BackgroundImageConfig {
    texture: *mut c_void,
    blend_mode: DtermBlendMode,
    opacity: f32,
}

/// Opaque hybrid renderer handle.
///
/// Unlike `DtermGpuRenderer` which does full wgpu rendering, this only
/// generates data for the platform to render with its own graphics API.
pub struct DtermHybridRenderer {
    /// Glyph atlas for text rendering
    glyph_atlas: Option<super::GlyphAtlas>,
    /// Configuration (stored for future use)
    #[allow(dead_code)]
    config: RendererConfig,
    /// Cached background vertex data (solid color quads)
    #[allow(dead_code)]
    background_vertices: Vec<super::pipeline::CellVertex>,
    /// Cached glyph vertex data (textured quads from atlas)
    #[allow(dead_code)]
    glyph_vertices: Vec<super::pipeline::CellVertex>,
    /// Cached decoration vertex data (underlines, strikethrough)
    #[allow(dead_code)]
    decoration_vertices: Vec<super::pipeline::CellVertex>,
    /// Combined vertex data for FFI (backgrounds + glyphs + decorations)
    vertex_data: Vec<super::pipeline::CellVertex>,
    /// Cached uniforms from last build
    uniforms: super::pipeline::Uniforms,
    /// Pending glyph data for incremental upload
    pending_glyphs: Vec<(super::GlyphEntry, Vec<u8>)>,
    /// Start time for animation tracking (cursor blink, etc.)
    start_time: std::time::Instant,
    // --- Platform-rendered glyph support ---
    /// Platform-provided glyph entries (codepoint -> entry)
    /// Used when platform renders glyphs with Core Text/etc. instead of fontdue
    platform_glyphs: HashMap<u32, super::GlyphEntry>,
    /// Platform-provided atlas size (pixels, square)
    platform_atlas_size: u32,
    /// Platform-provided cell dimensions (width, height in pixels)
    platform_cell_size: (f32, f32),
    /// Whether to use platform glyphs instead of internal atlas
    use_platform_glyphs: bool,
    /// Background image configuration (opaque platform texture)
    background_image: Option<BackgroundImageConfig>,
    /// Selection color override (RGBA, 0-1)
    selection_color: [f32; 4],
    /// Cursor style override
    cursor_style_override: Option<DtermCursorStyle>,
    /// Cursor blink rate override (ms)
    cursor_blink_ms_override: Option<u32>,
    /// Bold font variant (if set)
    font_bold: Option<fontdue::Font>,
    /// Italic font variant (if set)
    font_italic: Option<fontdue::Font>,
    /// Bold-italic font variant (if set)
    font_bold_italic: Option<fontdue::Font>,
}

impl DtermHybridRenderer {
    fn new(config: RendererConfig) -> Self {
        Self {
            glyph_atlas: None,
            config,
            background_vertices: Vec::new(),
            glyph_vertices: Vec::new(),
            decoration_vertices: Vec::new(),
            vertex_data: Vec::new(),
            uniforms: super::pipeline::Uniforms::default(),
            pending_glyphs: Vec::new(),
            start_time: std::time::Instant::now(),
            platform_glyphs: HashMap::new(),
            platform_atlas_size: 512,
            platform_cell_size: (8.0, 16.0),
            use_platform_glyphs: false,
            background_image: None,
            selection_color: [0.2, 0.4, 0.8, 0.5],
            cursor_style_override: None,
            cursor_blink_ms_override: None,
            font_bold: None,
            font_italic: None,
            font_bold_italic: None,
        }
    }

    fn apply_ffi_config(&mut self, config: &DtermRendererConfig) {
        self.selection_color = [
            f32::from(config.selection_r) / 255.0,
            f32::from(config.selection_g) / 255.0,
            f32::from(config.selection_b) / 255.0,
            f32::from(config.selection_a) / 255.0,
        ];
        self.cursor_style_override = Some(config.cursor_style);
        self.cursor_blink_ms_override = Some(config.cursor_blink_ms);
    }
}

/// Vertex data for FFI.
///
/// This matches the CellVertex layout in pipeline.rs:
/// - position: `[f32; 2]` (8 bytes)
/// - uv: `[f32; 2]` (8 bytes)
/// - `fg_color`: `[f32; 4]` (16 bytes)
/// - `bg_color`: `[f32; 4]` (16 bytes)
/// - flags: u32 (4 bytes)
/// - `_padding`: `[u32; 3]` (12 bytes)
///
/// Total: 64 bytes per vertex
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermCellVertex {
    /// Position in cell grid coordinates (fractional for sub-cell positioning)
    pub position: [f32; 2],
    /// UV coordinates in atlas texture (normalized 0-1)
    pub uv: [f32; 2],
    /// Foreground color (RGBA, 0-1)
    pub fg_color: [f32; 4],
    /// Background color (RGBA, 0-1)
    pub bg_color: [f32; 4],
    /// Flags (bold, dim, underline, etc.)
    pub flags: u32,
    /// Padding for alignment (required for GPU buffer compatibility)
    #[allow(clippy::pub_underscore_fields)]
    pub _padding: [u32; 3],
}

/// Uniform data for FFI.
///
/// This matches the Uniforms layout in pipeline.rs (64 bytes total).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermUniforms {
    /// Viewport width in pixels
    pub viewport_width: f32,
    /// Viewport height in pixels
    pub viewport_height: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    /// Atlas texture size in pixels
    pub atlas_size: f32,
    /// Time for animations (seconds)
    pub time: f32,
    /// Cursor X position (cell coordinates, -1 if hidden)
    pub cursor_x: i32,
    /// Cursor Y position (cell coordinates, -1 if hidden)
    pub cursor_y: i32,
    /// Cursor color (RGBA)
    pub cursor_color: [f32; 4],
    /// Padding for alignment (required for GPU uniform buffer compatibility)
    #[allow(clippy::pub_underscore_fields)]
    pub _padding: [f32; 4],
}

/// Glyph entry data for FFI.
///
/// Describes a single glyph's position and size in the atlas texture.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermGlyphEntry {
    /// X offset in atlas (pixels)
    pub x: u16,
    /// Y offset in atlas (pixels)
    pub y: u16,
    /// Glyph width (pixels)
    pub width: u16,
    /// Glyph height (pixels)
    pub height: u16,
    /// Horizontal offset from cursor position (pixels, can be negative)
    pub bearing_x: i16,
    /// Vertical offset from baseline (pixels)
    pub bearing_y: i16,
    /// Horizontal advance after rendering (pixels)
    pub advance: u16,
    /// Padding for alignment
    #[allow(clippy::pub_underscore_fields)]
    pub _padding: u16,
}

/// Create a hybrid renderer.
///
/// This creates a renderer that generates vertex data for the platform to use.
///
/// # Safety
///
/// - `config` may be null (uses defaults).
///
/// # Returns
///
/// Pointer to the renderer, or null on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_create(
    config: *const DtermRendererConfig,
) -> *mut DtermHybridRenderer {
    let renderer_config = if config.is_null() {
        RendererConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    let mut renderer = DtermHybridRenderer::new(renderer_config);
    if !config.is_null() {
        renderer.apply_ffi_config(unsafe { &*config });
    }

    Box::into_raw(Box::new(renderer))
}

/// Free a hybrid renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_free(renderer: *mut DtermHybridRenderer) {
    if !renderer.is_null() {
        drop(unsafe { Box::from_raw(renderer) });
    }
}

/// Set the font for the hybrid renderer from raw font data.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
/// - `font_data` must be a valid pointer to TTF/OTF font data.
/// - `font_len` must be the exact length of the font data in bytes.
///
/// # Returns
///
/// `true` on success, `false` on failure.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_font(
    renderer: *mut DtermHybridRenderer,
    font_data: *const u8,
    font_len: usize,
    config: *const DtermAtlasConfig,
) -> bool {
    if renderer.is_null() || font_data.is_null() || font_len == 0 {
        return false;
    }

    let renderer = unsafe { &mut *renderer };

    let atlas_config = if config.is_null() {
        super::AtlasConfig::default()
    } else {
        unsafe { (*config).clone().into() }
    };

    let atlas = unsafe { super::GlyphAtlas::new_from_ptr(atlas_config, font_data, font_len) };

    match atlas {
        Some(mut atlas) => {
            atlas.set_font_variants(
                renderer.font_bold.clone(),
                renderer.font_italic.clone(),
                renderer.font_bold_italic.clone(),
            );
            renderer.glyph_atlas = Some(atlas);
            true
        }
        None => false,
    }
}

/// Get cell dimensions from the hybrid renderer.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
/// - `cell_width` and `cell_height` must be valid pointers.
///
/// # Returns
///
/// `true` if cell dimensions were retrieved, `false` if no font is set.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_cell_size(
    renderer: *mut DtermHybridRenderer,
    cell_width: *mut f32,
    cell_height: *mut f32,
) -> bool {
    if renderer.is_null() || cell_width.is_null() || cell_height.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };

    if let Some((width, height)) = hybrid_cell_size(renderer) {
        unsafe {
            *cell_width = width;
            *cell_height = height;
        }
        true
    } else {
        // Return defaults if no font set
        unsafe {
            *cell_width = 8.0;
            *cell_height = 16.0;
        }
        false
    }
}

/// Build vertex data for the terminal.
///
/// This generates all vertex data needed to render the terminal. The data
/// is cached internally and can be accessed via `dterm_hybrid_renderer_get_vertices`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
/// - `terminal` must be a valid pointer to a Terminal.
///
/// # Returns
///
/// The number of vertices generated, or 0 on failure.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_build(
    renderer: *mut DtermHybridRenderer,
    terminal: *const Terminal,
) -> u32 {
    if renderer.is_null() || terminal.is_null() {
        return 0;
    }

    let renderer = unsafe { &mut *renderer };
    let terminal = unsafe { &*terminal };

    let grid = terminal.grid();
    let rows = grid.rows();
    let cols = grid.cols();
    let cursor = grid.cursor();
    let cursor_visible = terminal.cursor_visible();
    let selection = terminal.text_selection();

    // Get cell dimensions and atlas size based on mode
    let (cell_width, cell_height, atlas_size) = if renderer.use_platform_glyphs {
        // Use platform-provided values
        (
            renderer.platform_cell_size.0,
            renderer.platform_cell_size.1,
            renderer.platform_atlas_size,
        )
    } else if let Some(atlas) = &renderer.glyph_atlas {
        // Use internal atlas values
        (atlas.cell_width(), atlas.line_height(), atlas.size())
    } else {
        // Defaults
        (8.0, 16.0, 512)
    };

    // Update uniforms
    let cursor_color = terminal
        .cursor_color()
        .unwrap_or_else(|| terminal.default_foreground());

    // Get cursor style info from terminal
    let term_cursor_style = terminal.cursor_style();
    let cursor_style = renderer
        .cursor_style_override
        .map(cursor_style_from_ffi)
        .unwrap_or_else(|| super::terminal_cursor_style_to_gpu(term_cursor_style));
    let blink_ms = renderer
        .cursor_blink_ms_override
        .unwrap_or_else(|| if super::cursor_should_blink(term_cursor_style) { 530 } else { 0 });

    renderer.uniforms = super::pipeline::Uniforms {
        viewport_width: cell_width * f32::from(cols),
        viewport_height: cell_height * f32::from(rows),
        cell_width,
        cell_height,
        atlas_size: atlas_size as f32,
        time: renderer.start_time.elapsed().as_secs_f32(),
        cursor_x: if cursor_visible {
            i32::from(cursor.col)
        } else {
            -1
        },
        cursor_y: if cursor_visible {
            i32::from(cursor.row)
        } else {
            -1
        },
        cursor_color: super::rgb_to_f32(cursor_color),
        selection_color: renderer.selection_color,
        cursor_style: cursor_style as u32,
        cursor_blink_ms: blink_ms,
        _padding: [0; 2],
    };

    // Build vertices
    let mut builder = CellVertexBuilder::new(cell_width, cell_height);

    for row in 0..rows {
        let Some(row_data) = grid.row(row) else {
            continue;
        };
        for col in 0..cols {
            let Some(&cell) = row_data.get(col) else {
                continue;
            };

            let mut resolved = super::resolve_cell_style(terminal, grid, cell, row, col);
            if cursor_visible && cursor.row == row && cursor.col == col {
                resolved.flags |= super::pipeline::FLAG_IS_CURSOR;
            }
            if selection.contains(i32::from(row), col) {
                resolved.flags |= super::pipeline::FLAG_IS_SELECTION;
            }

            builder.add_background(u32::from(col), u32::from(row), resolved.bg, resolved.flags);

            if resolved.draw_glyph && !cell.is_wide_continuation() {
                // CRITICAL: Check for box drawing characters FIRST
                // These must be rendered with geometric primitives, not font glyphs
                // Bug fix: This was missing, causing invisible box drawing in hybrid renderer
                // See: docs/RETROSPECTIVE_INVISIBLE_CHARS_2025-12-31.md
                if super::box_drawing::is_box_drawing(resolved.glyph) {
                    let box_verts = super::box_drawing::generate_box_drawing_vertices(
                        resolved.glyph,
                        u32::from(col),
                        u32::from(row),
                        resolved.fg,
                    );
                    for v in box_verts {
                        builder.add_raw_vertex(v);
                    }
                } else if renderer.use_platform_glyphs {
                    // Use platform-provided glyph entries
                    let codepoint = resolved.glyph as u32;
                    if let Some(entry) = renderer.platform_glyphs.get(&codepoint) {
                        if entry.width > 0 && entry.height > 0 {
                            let (u_min, v_min, u_max, v_max) =
                                entry.uv_coords(renderer.platform_atlas_size);
                            builder.add_glyph(
                                u32::from(col),
                                u32::from(row),
                                [u_min, v_min],
                                [u_max, v_max],
                                resolved.fg,
                                resolved.bg,
                                resolved.flags,
                            );
                        }
                    }
                } else if let Some(atlas) = renderer.glyph_atlas.as_mut() {
                    // Use internal fontdue-based atlas
                    let key = super::GlyphKey::new(
                        resolved.glyph,
                        atlas.default_font_size(),
                        resolved.is_bold,
                        resolved.is_italic,
                    );
                    if let Some(entry) = atlas.ensure(key).copied() {
                        if entry.width > 0 && entry.height > 0 {
                            let (u_min, v_min, u_max, v_max) = entry.uv_coords(atlas.size());
                            builder.add_glyph(
                                u32::from(col),
                                u32::from(row),
                                [u_min, v_min],
                                [u_max, v_max],
                                resolved.fg,
                                resolved.bg,
                                resolved.flags,
                            );
                        }
                    }
                }
            }

            // Add decorations (underline/strikethrough)
            // Decoration color defaults to fg, but can be overridden by underline_color
            let decoration_color = resolved.underline_color.unwrap_or(resolved.fg);

            match resolved.underline_style {
                super::UnderlineStyle::Single => {
                    builder.add_single_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                super::UnderlineStyle::Double => {
                    builder.add_double_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                super::UnderlineStyle::Curly => {
                    builder.add_curly_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                super::UnderlineStyle::Dotted => {
                    builder.add_dotted_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                super::UnderlineStyle::Dashed => {
                    builder.add_dashed_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                super::UnderlineStyle::None => {}
            }

            if resolved.has_strikethrough {
                builder.add_strikethrough(
                    u32::from(col),
                    u32::from(row),
                    resolved.fg, // Strikethrough uses fg color
                    resolved.flags,
                );
            }
        }
    }

    // Collect pending glyphs for the platform to upload
    if let Some(atlas) = renderer.glyph_atlas.as_mut() {
        if atlas.has_pending() {
            renderer.pending_glyphs.clear();
            for (_, entry, bitmap) in atlas.take_pending() {
                renderer.pending_glyphs.push((entry, bitmap));
            }
        }
    }

    let (backgrounds, glyphs, decorations) = builder.into_separated();
    let total = backgrounds.len() + glyphs.len() + decorations.len();

    // Build combined vertex_data for get_vertices() FFI
    let mut combined = Vec::with_capacity(total);
    combined.extend_from_slice(&backgrounds);
    combined.extend_from_slice(&glyphs);
    combined.extend_from_slice(&decorations);
    renderer.vertex_data = combined;

    renderer.background_vertices = backgrounds;
    renderer.glyph_vertices = glyphs;
    renderer.decoration_vertices = decorations;
    total as u32
}

/// Get the background vertex data from the last build.
///
/// Returns solid-color background quads (rendered first, no texture needed).
///
/// # Safety
/// - `renderer` must be a valid pointer.
/// - `out_count` must be a valid pointer.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_background_vertices(
    renderer: *const DtermHybridRenderer,
    out_count: *mut u32,
) -> *const DtermCellVertex {
    if renderer.is_null() || out_count.is_null() {
        return std::ptr::null();
    }
    let renderer = unsafe { &*renderer };
    unsafe { *out_count = renderer.background_vertices.len() as u32 };
    if renderer.background_vertices.is_empty() {
        return std::ptr::null();
    }
    renderer.background_vertices.as_ptr() as *const DtermCellVertex
}

/// Get the glyph vertex data from the last build.
///
/// Returns textured glyph quads (rendered second, uses atlas texture).
///
/// # Safety
/// - `renderer` must be a valid pointer.
/// - `out_count` must be a valid pointer.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_glyph_vertices(
    renderer: *const DtermHybridRenderer,
    out_count: *mut u32,
) -> *const DtermCellVertex {
    if renderer.is_null() || out_count.is_null() {
        return std::ptr::null();
    }
    let renderer = unsafe { &*renderer };
    unsafe { *out_count = renderer.glyph_vertices.len() as u32 };
    if renderer.glyph_vertices.is_empty() {
        return std::ptr::null();
    }
    renderer.glyph_vertices.as_ptr() as *const DtermCellVertex
}

/// Get the decoration vertex data from the last build.
///
/// Returns underline/strikethrough/box-drawing quads (rendered last, solid color).
///
/// # Safety
/// - `renderer` must be a valid pointer.
/// - `out_count` must be a valid pointer.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_decoration_vertices(
    renderer: *const DtermHybridRenderer,
    out_count: *mut u32,
) -> *const DtermCellVertex {
    if renderer.is_null() || out_count.is_null() {
        return std::ptr::null();
    }
    let renderer = unsafe { &*renderer };
    unsafe { *out_count = renderer.decoration_vertices.len() as u32 };
    if renderer.decoration_vertices.is_empty() {
        return std::ptr::null();
    }
    renderer.decoration_vertices.as_ptr() as *const DtermCellVertex
}

/// Get all vertex data combined from the last build.
///
/// Returns a contiguous array of all vertices (backgrounds + glyphs + decorations).
/// For multi-pass rendering, prefer the separate getters.
///
/// # Safety
/// - `renderer` must be a valid pointer.
/// - `out_count` must be a valid pointer.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_vertices(
    renderer: *const DtermHybridRenderer,
    out_count: *mut u32,
) -> *const DtermCellVertex {
    if renderer.is_null() || out_count.is_null() {
        return std::ptr::null();
    }
    let renderer = unsafe { &*renderer };
    unsafe { *out_count = renderer.vertex_data.len() as u32 };
    if renderer.vertex_data.is_empty() {
        return std::ptr::null();
    }
    renderer.vertex_data.as_ptr() as *const DtermCellVertex
}

/// Get the uniforms from the last build.
///
/// The returned pointer is valid until the next call to `dterm_hybrid_renderer_build`
/// or `dterm_hybrid_renderer_free`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
///
/// # Returns
///
/// Pointer to the uniforms, or null if no data is available.
#[allow(clippy::borrow_as_ptr)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_uniforms(
    renderer: *const DtermHybridRenderer,
) -> *const DtermUniforms {
    if renderer.is_null() {
        return std::ptr::null();
    }

    let renderer = unsafe { &*renderer };

    // Uniforms and DtermUniforms have the same layout
    std::ptr::addr_of!(renderer.uniforms) as *const DtermUniforms
}

/// Get the atlas size.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
///
/// # Returns
///
/// Atlas size in pixels, or 0 if no font is set.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_atlas_size(
    renderer: *const DtermHybridRenderer,
) -> u32 {
    if renderer.is_null() {
        return 0;
    }

    let renderer = unsafe { &*renderer };

    renderer.glyph_atlas.as_ref().map(|a| a.size()).unwrap_or(0)
}

/// Get the number of pending glyph uploads.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
///
/// # Returns
///
/// The number of pending glyphs, or 0 if none.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_pending_glyph_count(
    renderer: *const DtermHybridRenderer,
) -> u32 {
    if renderer.is_null() {
        return 0;
    }

    let renderer = unsafe { &*renderer };
    renderer.pending_glyphs.len() as u32
}

/// Get pending glyph data by index.
///
/// After `dterm_hybrid_renderer_build` is called, new glyphs may have been
/// rasterized. This function returns the pending glyph bitmap data that needs
/// to be uploaded to the platform's texture.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
/// - `index` must be less than the count returned by `dterm_hybrid_renderer_pending_glyph_count`.
/// - `out_entry` must be a valid pointer.
/// - `out_data` must be a valid pointer.
/// - `out_data_len` must be a valid pointer.
///
/// # Returns
///
/// `true` if the glyph data was retrieved, `false` if index is out of bounds.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_pending_glyph(
    renderer: *const DtermHybridRenderer,
    index: u32,
    out_entry: *mut DtermGlyphEntry,
    out_data: *mut *const u8,
    out_data_len: *mut usize,
) -> bool {
    if renderer.is_null() || out_entry.is_null() || out_data.is_null() || out_data_len.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };

    let idx = index as usize;
    if idx >= renderer.pending_glyphs.len() {
        return false;
    }

    let (entry, bitmap) = &renderer.pending_glyphs[idx];

    unsafe {
        *out_entry = DtermGlyphEntry {
            x: entry.x,
            y: entry.y,
            width: entry.width,
            height: entry.height,
            bearing_x: entry.offset_x,
            bearing_y: entry.offset_y,
            advance: entry.advance,
            _padding: 0,
        };
        *out_data = bitmap.as_ptr();
        *out_data_len = bitmap.len();
    }

    true
}

/// Clear pending glyph data.
///
/// Call this after uploading all pending glyphs to the platform texture.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_clear_pending_glyphs(
    renderer: *mut DtermHybridRenderer,
) {
    if renderer.is_null() {
        return;
    }

    let renderer = unsafe { &mut *renderer };
    renderer.pending_glyphs.clear();
}

/// Get the full atlas bitmap data.
///
/// This returns the entire atlas texture data as a single-channel (grayscale) bitmap.
/// The platform can use this to create or update its texture.
///
/// The returned pointer is valid until the next call to `dterm_hybrid_renderer_build`,
/// `dterm_hybrid_renderer_set_font`, or `dterm_hybrid_renderer_free`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
/// - `out_data` must be a valid pointer to receive the texture data pointer.
/// - `out_len` must be a valid pointer to receive the texture data length in bytes.
/// - `out_width` must be a valid pointer to receive the atlas width in pixels.
/// - `out_height` must be a valid pointer to receive the atlas height in pixels.
///
/// # Returns
///
/// `true` if atlas data is available, `false` if no font is set or any pointer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_get_atlas_data(
    renderer: *const DtermHybridRenderer,
    out_data: *mut *const u8,
    out_len: *mut usize,
    out_width: *mut u32,
    out_height: *mut u32,
) -> bool {
    if renderer.is_null()
        || out_data.is_null()
        || out_len.is_null()
        || out_width.is_null()
        || out_height.is_null()
    {
        return false;
    }

    let renderer = unsafe { &*renderer };

    if let Some(atlas) = &renderer.glyph_atlas {
        let texture_data = atlas.texture_data();
        let size = atlas.size();

        unsafe {
            *out_data = texture_data.as_ptr();
            *out_len = texture_data.len();
            *out_width = size;
            *out_height = size;
        }

        true
    } else {
        false
    }
}

/// Check if the hybrid renderer FFI is available.
#[no_mangle]
pub extern "C" fn dterm_hybrid_renderer_available() -> bool {
    true
}

// =============================================================================
// PLATFORM-RENDERED GLYPH SUPPORT
// =============================================================================
//
// These functions allow the platform (Swift) to render glyphs using Core Text
// and provide the glyph atlas data to dterm-core. This enables support for
// macOS system fonts (Monaco, Menlo, SF Mono) that don't have accessible file URLs.
//
// ## Workflow
//
// 1. Platform creates glyph atlas using Core Text
// 2. Platform calls `dterm_hybrid_renderer_set_platform_cell_size` with font metrics
// 3. Platform calls `dterm_hybrid_renderer_set_platform_atlas_size` with texture size
// 4. Platform calls `dterm_hybrid_renderer_add_platform_glyph` for each glyph
// 5. Platform calls `dterm_hybrid_renderer_enable_platform_glyphs(true)`
// 6. During `dterm_hybrid_renderer_build`, Rust uses platform glyphs for UV coords
// 7. Platform renders with its own Metal texture containing the glyphs

/// Enable or disable platform-rendered glyph mode.
///
/// When enabled, the renderer uses platform-provided glyph entries instead of
/// the internal fontdue-based atlas. The platform must provide glyph entries
/// via `dterm_hybrid_renderer_add_platform_glyph`.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
///
/// # Returns
///
/// `true` on success, `false` if renderer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_enable_platform_glyphs(
    renderer: *mut DtermHybridRenderer,
    enable: bool,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    renderer.use_platform_glyphs = enable;
    true
}

/// Set the cell size for platform-rendered glyphs.
///
/// The platform computes cell dimensions from font metrics using Core Text.
/// This must be called before `dterm_hybrid_renderer_build` when using
/// platform glyphs.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
///
/// # Returns
///
/// `true` on success, `false` if renderer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_platform_cell_size(
    renderer: *mut DtermHybridRenderer,
    width: f32,
    height: f32,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    renderer.platform_cell_size = (width, height);
    true
}

/// Set the atlas size for platform-rendered glyphs.
///
/// The platform manages its own texture atlas. This tells dterm-core the
/// atlas dimensions for UV coordinate calculation.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
///
/// # Returns
///
/// `true` on success, `false` if renderer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_platform_atlas_size(
    renderer: *mut DtermHybridRenderer,
    size: u32,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    renderer.platform_atlas_size = size;
    true
}

/// Add a platform-rendered glyph entry.
///
/// The platform renders glyphs using Core Text and adds them to its texture atlas.
/// This function tells dterm-core where each glyph is located in the atlas.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
///
/// # Returns
///
/// `true` on success, `false` if renderer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_add_platform_glyph(
    renderer: *mut DtermHybridRenderer,
    codepoint: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bearing_x: i16,
    bearing_y: i16,
    advance: u16,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };

    let entry = super::GlyphEntry {
        x,
        y,
        width,
        height,
        offset_x: bearing_x,
        offset_y: bearing_y,
        advance,
    };

    renderer.platform_glyphs.insert(codepoint, entry);
    true
}

/// Clear all platform-rendered glyph entries.
///
/// Call this when changing fonts to remove stale glyph entries.
///
/// # Safety
///
/// - `renderer` must be a valid pointer returned by `dterm_hybrid_renderer_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_clear_platform_glyphs(
    renderer: *mut DtermHybridRenderer,
) {
    if renderer.is_null() {
        return;
    }

    let renderer = unsafe { &mut *renderer };
    renderer.platform_glyphs.clear();
}

/// Get the number of platform glyph entries.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
///
/// # Returns
///
/// The number of platform glyphs registered, or 0 if renderer is null.
#[allow(clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_platform_glyph_count(
    renderer: *const DtermHybridRenderer,
) -> u32 {
    if renderer.is_null() {
        return 0;
    }

    let renderer = unsafe { &*renderer };
    renderer.platform_glyphs.len() as u32
}

/// Check if platform glyph mode is enabled.
///
/// # Safety
///
/// - `renderer` must be a valid pointer.
///
/// # Returns
///
/// `true` if platform glyph mode is enabled, `false` otherwise.
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_is_platform_glyphs_enabled(
    renderer: *const DtermHybridRenderer,
) -> bool {
    if renderer.is_null() {
        return false;
    }

    let renderer = unsafe { &*renderer };
    renderer.use_platform_glyphs
}

// =============================================================================
// IMAGE TEXTURE CACHE FFI
// =============================================================================

/// Opaque image texture cache handle for FFI.
pub struct DtermImageCache {
    cache: super::ImageTextureCache,
}

/// Create a new image texture cache.
///
/// # Safety
///
/// The returned pointer must be freed with `dterm_image_cache_free`.
///
/// # Arguments
///
/// * `memory_budget` - Maximum GPU memory budget in bytes (0 = use default 64MB).
///
/// # Returns
///
/// A new image cache handle, or null on failure.
#[no_mangle]
pub extern "C" fn dterm_image_cache_create(memory_budget: usize) -> *mut DtermImageCache {
    let budget = if memory_budget == 0 {
        super::DEFAULT_IMAGE_BUDGET
    } else {
        memory_budget
    };

    Box::into_raw(Box::new(DtermImageCache {
        cache: super::ImageTextureCache::new(budget),
    }))
}

/// Free an image texture cache.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`,
///   or null (which is a no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_free(cache: *mut DtermImageCache) {
    if !cache.is_null() {
        drop(unsafe { Box::from_raw(cache) });
    }
}

/// Upload an image to the cache.
///
/// This allocates a handle and prepares the image for GPU upload.
/// The actual GPU texture creation is the caller's responsibility.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
/// - `data` must be a valid pointer to image pixel data.
/// - `data_len` must be the exact length of the data in bytes.
/// - The data must remain valid for the duration of this call.
///
/// # Arguments
///
/// * `cache` - Image cache handle.
/// * `data` - Pointer to image pixel data.
/// * `data_len` - Length of data in bytes.
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
/// * `format` - Image format (0=RGBA, 1=RGB, 2=ARGB).
/// * `out_rgba` - Output pointer for converted RGBA data (caller must free with `dterm_image_free_rgba`).
/// * `out_rgba_len` - Output length of RGBA data.
///
/// # Returns
///
/// Image handle (non-zero on success, 0 on failure).
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_upload(
    cache: *mut DtermImageCache,
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    format: u8,
    out_rgba: *mut *mut u8,
    out_rgba_len: *mut usize,
) -> u64 {
    if cache.is_null() || data.is_null() || data_len == 0 {
        return 0;
    }

    let Some(img_format) = super::ImageFormat::from_u8(format) else {
        return 0;
    };

    let cache = unsafe { &mut *cache };
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };

    // Convert to RGBA
    let Some(rgba_data) = super::ImageTextureCache::convert_to_rgba(data_slice, width, height, img_format) else {
        return 0;
    };

    // Allocate handle
    let handle = cache.cache.allocate_handle(width, height, img_format);
    if handle.is_null() {
        return 0;
    }

    // Return RGBA data to caller if requested
    if !out_rgba.is_null() && !out_rgba_len.is_null() {
        let len = rgba_data.len();
        let boxed = rgba_data.into_boxed_slice();
        let ptr = Box::into_raw(boxed) as *mut u8;
        unsafe {
            *out_rgba = ptr;
            *out_rgba_len = len;
        }
    }

    handle.raw()
}

/// Free RGBA data returned by `dterm_image_cache_upload`.
///
/// # Safety
///
/// - `data` must be a pointer returned by `dterm_image_cache_upload` via `out_rgba`,
///   or null (which is a no-op).
/// - `len` must be the exact length returned via `out_rgba_len`.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_free_rgba(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        drop(unsafe { Box::from_raw(std::slice::from_raw_parts_mut(data, len)) });
    }
}

/// Place an image at a terminal position.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Arguments
///
/// * `cache` - Image cache handle.
/// * `handle` - Image handle from `dterm_image_cache_upload`.
/// * `row` - Row position (negative for scrollback).
/// * `col` - Column position.
/// * `width_cells` - Width in terminal cells.
/// * `height_cells` - Height in terminal cells.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_place(
    cache: *mut DtermImageCache,
    handle: u64,
    row: i64,
    col: u16,
    width_cells: u16,
    height_cells: u16,
) {
    if cache.is_null() || handle == 0 {
        return;
    }

    let cache = unsafe { &mut *cache };
    let img_handle = super::ImageHandle::from_raw(handle);

    cache.cache.place(super::ImagePlacement::new(
        img_handle,
        row,
        col,
        width_cells,
        height_cells,
    ));
}

/// Remove an image from the cache.
///
/// This removes the image and all its placements.
/// The caller is responsible for freeing the GPU texture.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Arguments
///
/// * `cache` - Image cache handle.
/// * `handle` - Image handle from `dterm_image_cache_upload`.
///
/// # Returns
///
/// `true` if the image was found and removed, `false` otherwise.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_remove(cache: *mut DtermImageCache, handle: u64) -> bool {
    if cache.is_null() || handle == 0 {
        return false;
    }

    let cache = unsafe { &mut *cache };
    let img_handle = super::ImageHandle::from_raw(handle);
    cache.cache.remove(img_handle)
}

/// Get the number of images in the cache.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Returns
///
/// Number of images in the cache.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_image_count(cache: *const DtermImageCache) -> usize {
    if cache.is_null() {
        return 0;
    }

    let cache = unsafe { &*cache };
    cache.cache.image_count()
}

/// Get the number of active placements.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Returns
///
/// Number of active placements.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_placement_count(cache: *const DtermImageCache) -> usize {
    if cache.is_null() {
        return 0;
    }

    let cache = unsafe { &*cache };
    cache.cache.placement_count()
}

/// Set the image memory budget.
///
/// If the new budget is lower than current usage, images will be evicted.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Arguments
///
/// * `cache` - Image cache handle.
/// * `bytes` - New memory budget in bytes.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_set_budget(cache: *mut DtermImageCache, bytes: usize) {
    if cache.is_null() {
        return;
    }

    let cache = unsafe { &mut *cache };
    cache.cache.set_memory_budget(bytes);
}

/// Get the current memory usage.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
///
/// # Returns
///
/// Current memory usage in bytes.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_memory_used(cache: *const DtermImageCache) -> usize {
    if cache.is_null() {
        return 0;
    }

    let cache = unsafe { &*cache };
    cache.cache.memory_used()
}

/// Clear all images and placements.
///
/// # Safety
///
/// - `cache` must be a valid pointer returned by `dterm_image_cache_create`.
#[no_mangle]
pub unsafe extern "C" fn dterm_image_cache_clear(cache: *mut DtermImageCache) {
    if cache.is_null() {
        return;
    }

    let cache = unsafe { &mut *cache };
    cache.cache.clear();
}

/// Check if the image cache FFI is available.
#[no_mangle]
pub extern "C" fn dterm_image_cache_available() -> bool {
    true
}

// =============================================================================
// INLINE IMAGE FFI (OSC 1337 FILE)
// =============================================================================

/// Inline image info for FFI.
///
/// Contains metadata about an iTerm2 inline image stored via OSC 1337 File.
///
/// # Size
///
/// This struct is 40 bytes on all platforms.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermInlineImageInfo {
    /// Unique image ID.
    pub id: u64,
    /// Row where image was placed.
    pub row: u16,
    /// Column where image was placed.
    pub col: u16,
    /// Width specification type: 0=Auto, 1=Cells, 2=Pixels, 3=Percent.
    pub width_spec_type: u8,
    /// Width specification value (cells/pixels/percent, 0 for auto).
    pub width_spec_value: u32,
    /// Height specification type: 0=Auto, 1=Cells, 2=Pixels, 3=Percent.
    pub height_spec_type: u8,
    /// Height specification value (cells/pixels/percent, 0 for auto).
    pub height_spec_value: u32,
    /// Whether to preserve aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// Size of image data in bytes.
    pub data_size: usize,
}

impl DtermInlineImageInfo {
    fn from_image(img: &crate::iterm_image::InlineImage) -> Self {
        let (width_spec_type, width_spec_value) = dim_spec_to_ffi(img.width());
        let (height_spec_type, height_spec_value) = dim_spec_to_ffi(img.height());
        Self {
            id: img.id(),
            row: img.cursor_row(),
            col: img.cursor_col(),
            width_spec_type,
            width_spec_value,
            height_spec_type,
            height_spec_value,
            preserve_aspect_ratio: img.preserve_aspect_ratio(),
            data_size: img.data().len(),
        }
    }
}

/// Convert DimensionSpec to FFI format.
fn dim_spec_to_ffi(spec: crate::iterm_image::DimensionSpec) -> (u8, u32) {
    match spec {
        crate::iterm_image::DimensionSpec::Auto => (0, 0),
        crate::iterm_image::DimensionSpec::Cells(n) => (1, n),
        crate::iterm_image::DimensionSpec::Pixels(n) => (2, n),
        crate::iterm_image::DimensionSpec::Percent(n) => (3, u32::from(n)),
    }
}

/// Get the number of inline images in a terminal.
///
/// # Safety
///
/// - `terminal` must be a valid pointer to a `Terminal`.
///
/// # Returns
///
/// Number of stored inline images.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_inline_image_count(
    terminal: *const crate::terminal::Terminal,
) -> usize {
    if terminal.is_null() {
        return 0;
    }
    let terminal = unsafe { &*terminal };
    terminal.inline_images().len()
}

/// Get info about an inline image by index.
///
/// # Safety
///
/// - `terminal` must be a valid pointer to a `Terminal`.
/// - `out_info` must be a valid pointer to a `DtermInlineImageInfo`.
///
/// # Arguments
///
/// * `terminal` - Terminal handle.
/// * `index` - Image index (0-based).
/// * `out_info` - Output pointer for image info.
///
/// # Returns
///
/// `true` if the image exists and info was written, `false` otherwise.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_inline_image_info(
    terminal: *const crate::terminal::Terminal,
    index: usize,
    out_info: *mut DtermInlineImageInfo,
) -> bool {
    if terminal.is_null() || out_info.is_null() {
        return false;
    }
    let terminal = unsafe { &*terminal };
    let images = terminal.inline_images().images();
    if index >= images.len() {
        return false;
    }
    unsafe {
        *out_info = DtermInlineImageInfo::from_image(&images[index]);
    }
    true
}

/// Get raw data for an inline image.
///
/// # Safety
///
/// - `terminal` must be a valid pointer to a `Terminal`.
/// - `out_data` must be a valid pointer to receive a pointer to the data.
/// - `out_len` must be a valid pointer to receive the data length.
///
/// The returned data pointer is valid as long as the terminal exists and
/// the image has not been evicted. Do NOT free the returned pointer.
///
/// # Arguments
///
/// * `terminal` - Terminal handle.
/// * `index` - Image index (0-based).
/// * `out_data` - Output pointer for image data.
/// * `out_len` - Output pointer for data length.
///
/// # Returns
///
/// `true` if the image exists and data pointer was written, `false` otherwise.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_inline_image_data(
    terminal: *const crate::terminal::Terminal,
    index: usize,
    out_data: *mut *const u8,
    out_len: *mut usize,
) -> bool {
    if terminal.is_null() || out_data.is_null() || out_len.is_null() {
        return false;
    }
    let terminal = unsafe { &*terminal };
    let images = terminal.inline_images().images();
    if index >= images.len() {
        return false;
    }
    let data = images[index].data();
    unsafe {
        *out_data = data.as_ptr();
        *out_len = data.len();
    }
    true
}

/// Clear all inline images from a terminal.
///
/// # Safety
///
/// - `terminal` must be a valid pointer to a `Terminal`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_inline_image_clear(
    terminal: *mut crate::terminal::Terminal,
) {
    if terminal.is_null() {
        return;
    }
    let terminal = unsafe { &mut *terminal };
    terminal.inline_images_mut().clear();
}

/// Check if inline image FFI is available.
#[no_mangle]
pub extern "C" fn dterm_inline_image_available() -> bool {
    true
}

#[cfg(test)]
#[allow(clippy::borrow_as_ptr)]
mod hybrid_tests {
    use super::*;
    use crate::selection::{SelectionSide, SelectionType};

    #[test]
    fn test_hybrid_renderer_create_free() {
        let renderer = unsafe { dterm_hybrid_renderer_create(std::ptr::null()) };
        assert!(!renderer.is_null());
        unsafe { dterm_hybrid_renderer_free(renderer) };
    }

    #[test]
    fn test_hybrid_renderer_null_safe() {
        // All functions should handle null pointers gracefully
        unsafe {
            dterm_hybrid_renderer_free(std::ptr::null_mut());

            let result = dterm_hybrid_renderer_set_font(
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                std::ptr::null(),
            );
            assert!(!result);

            let mut w = 0.0f32;
            let mut h = 0.0f32;
            let result = dterm_hybrid_renderer_get_cell_size(std::ptr::null_mut(), &mut w, &mut h);
            assert!(!result);

            let count = dterm_hybrid_renderer_build(std::ptr::null_mut(), std::ptr::null());
            assert_eq!(count, 0);

            let mut count = 0u32;
            let vertices = dterm_hybrid_renderer_get_vertices(std::ptr::null(), &mut count);
            assert!(vertices.is_null());
            assert_eq!(count, 0);

            let uniforms = dterm_hybrid_renderer_get_uniforms(std::ptr::null());
            assert!(uniforms.is_null());

            let size = dterm_hybrid_renderer_get_atlas_size(std::ptr::null());
            assert_eq!(size, 0);

            assert!(dterm_hybrid_renderer_available());
        }
    }

    #[test]
    fn test_cell_vertex_size() {
        // Ensure DtermCellVertex has the expected size (64 bytes)
        assert_eq!(std::mem::size_of::<DtermCellVertex>(), 64);
    }

    #[test]
    fn test_uniforms_size() {
        // Ensure DtermUniforms has the expected size (64 bytes)
        assert_eq!(std::mem::size_of::<DtermUniforms>(), 64);
    }

    #[test]
    fn test_hybrid_renderer_selection_overlay() {
        let mut terminal = Terminal::new(1, 1);
        {
            let selection = terminal.text_selection_mut();
            selection.start_selection(0, 0, SelectionSide::Left, SelectionType::Simple);
            selection.complete_selection();
        }

        let renderer = unsafe { dterm_hybrid_renderer_create(std::ptr::null()) };
        assert!(!renderer.is_null());

        let count = unsafe { dterm_hybrid_renderer_build(renderer, std::ptr::from_ref(&terminal)) };
        assert!(count > 0);

        let mut background_count = 0u32;
        let background_ptr =
            unsafe { dterm_hybrid_renderer_get_background_vertices(renderer, &mut background_count) };
        assert!(!background_ptr.is_null());
        assert!(background_count >= 6);

        let backgrounds =
            unsafe { std::slice::from_raw_parts(background_ptr, background_count as usize) };
        let flags = backgrounds[0].flags;
        assert_ne!(flags & super::super::OVERLAY_SELECTION, 0);

        unsafe { dterm_hybrid_renderer_free(renderer) };
    }

    #[test]
    fn test_hybrid_renderer_get_atlas_data_null_safe() {
        // All null pointer combinations should return false
        unsafe {
            let mut data: *const u8 = std::ptr::null();
            let mut len: usize = 0;
            let mut width: u32 = 0;
            let mut height: u32 = 0;

            // Null renderer
            let result = dterm_hybrid_renderer_get_atlas_data(
                std::ptr::null(),
                &mut data,
                &mut len,
                &mut width,
                &mut height,
            );
            assert!(!result);

            // Create a renderer but don't set a font
            let renderer = dterm_hybrid_renderer_create(std::ptr::null());
            assert!(!renderer.is_null());

            // Should return false because no font is set
            let result = dterm_hybrid_renderer_get_atlas_data(
                renderer,
                &mut data,
                &mut len,
                &mut width,
                &mut height,
            );
            assert!(!result, "Should return false when no font is set");

            dterm_hybrid_renderer_free(renderer);
        }
    }
}

#[cfg(test)]
mod image_cache_tests {
    use super::*;

    #[test]
    fn test_image_cache_create_free() {
        let cache = dterm_image_cache_create(0);
        assert!(!cache.is_null());
        unsafe { dterm_image_cache_free(cache) };
    }

    #[test]
    fn test_image_cache_null_safe() {
        // All functions should handle null pointers gracefully
        unsafe {
            dterm_image_cache_free(std::ptr::null_mut());

            let handle = dterm_image_cache_upload(
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                100,
                100,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(handle, 0);

            dterm_image_cache_place(std::ptr::null_mut(), 1, 0, 0, 4, 4);

            let removed = dterm_image_cache_remove(std::ptr::null_mut(), 1);
            assert!(!removed);

            let count = dterm_image_cache_image_count(std::ptr::null());
            assert_eq!(count, 0);

            let count = dterm_image_cache_placement_count(std::ptr::null());
            assert_eq!(count, 0);

            let used = dterm_image_cache_memory_used(std::ptr::null());
            assert_eq!(used, 0);

            dterm_image_cache_set_budget(std::ptr::null_mut(), 1024);
            dterm_image_cache_clear(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_image_cache_upload_and_place() {
        unsafe {
            let cache = dterm_image_cache_create(1024 * 1024);
            assert!(!cache.is_null());

            // Create a 10x10 RGBA image (400 bytes)
            let width = 10u32;
            let height = 10u32;
            let data: Vec<u8> = vec![255; (width * height * 4) as usize];

            let mut out_rgba: *mut u8 = std::ptr::null_mut();
            let mut out_len: usize = 0;

            let handle = dterm_image_cache_upload(
                cache,
                data.as_ptr(),
                data.len(),
                width,
                height,
                0, // RGBA
                std::ptr::from_mut(&mut out_rgba),
                std::ptr::from_mut(&mut out_len),
            );

            assert_ne!(handle, 0);
            assert!(!out_rgba.is_null());
            assert_eq!(out_len, (width * height * 4) as usize);

            // Free the RGBA data
            dterm_image_free_rgba(out_rgba, out_len);

            // Place the image
            dterm_image_cache_place(cache, handle, 5, 10, 4, 3);

            assert_eq!(dterm_image_cache_image_count(cache), 1);
            assert_eq!(dterm_image_cache_placement_count(cache), 1);

            // Remove the image
            let removed = dterm_image_cache_remove(cache, handle);
            assert!(removed);

            assert_eq!(dterm_image_cache_image_count(cache), 0);
            assert_eq!(dterm_image_cache_placement_count(cache), 0);

            dterm_image_cache_free(cache);
        }
    }

    #[test]
    fn test_image_cache_rgb_to_rgba() {
        unsafe {
            let cache = dterm_image_cache_create(1024 * 1024);

            // Create a 2x2 RGB image (12 bytes)
            let width = 2u32;
            let height = 2u32;
            let data: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128];

            let mut out_rgba: *mut u8 = std::ptr::null_mut();
            let mut out_len: usize = 0;

            let handle = dterm_image_cache_upload(
                cache,
                data.as_ptr(),
                data.len(),
                width,
                height,
                1, // RGB
                std::ptr::from_mut(&mut out_rgba),
                std::ptr::from_mut(&mut out_len),
            );

            assert_ne!(handle, 0);
            assert_eq!(out_len, 16); // 2x2x4 = 16 bytes RGBA

            // Verify conversion (R-G-B-A order, alpha should be 255)
            let rgba_slice = std::slice::from_raw_parts(out_rgba, out_len);
            assert_eq!(rgba_slice[3], 255); // First pixel alpha
            assert_eq!(rgba_slice[7], 255); // Second pixel alpha

            dterm_image_free_rgba(out_rgba, out_len);
            dterm_image_cache_free(cache);
        }
    }

    #[test]
    fn test_image_cache_memory_tracking() {
        unsafe {
            let cache = dterm_image_cache_create(1024 * 1024);

            // Upload a 10x10 image (400 bytes GPU memory)
            let data: Vec<u8> = vec![255; 400];
            let handle = dterm_image_cache_upload(
                cache,
                data.as_ptr(),
                data.len(),
                10,
                10,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_ne!(handle, 0);

            let used = dterm_image_cache_memory_used(cache);
            assert_eq!(used, 400); // 10x10x4 bytes

            dterm_image_cache_remove(cache, handle);
            assert_eq!(dterm_image_cache_memory_used(cache), 0);

            dterm_image_cache_free(cache);
        }
    }

    #[test]
    fn test_image_cache_clear() {
        unsafe {
            let cache = dterm_image_cache_create(1024 * 1024);

            let data: Vec<u8> = vec![255; 400];
            let h1 = dterm_image_cache_upload(
                cache,
                data.as_ptr(),
                data.len(),
                10,
                10,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let h2 = dterm_image_cache_upload(
                cache,
                data.as_ptr(),
                data.len(),
                10,
                10,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );

            dterm_image_cache_place(cache, h1, 0, 0, 4, 4);
            dterm_image_cache_place(cache, h2, 5, 0, 4, 4);

            assert_eq!(dterm_image_cache_image_count(cache), 2);
            assert_eq!(dterm_image_cache_placement_count(cache), 2);

            dterm_image_cache_clear(cache);

            assert_eq!(dterm_image_cache_image_count(cache), 0);
            assert_eq!(dterm_image_cache_placement_count(cache), 0);
            assert_eq!(dterm_image_cache_memory_used(cache), 0);

            dterm_image_cache_free(cache);
        }
    }

    #[test]
    fn test_image_cache_available() {
        assert!(dterm_image_cache_available());
    }
}

#[cfg(test)]
mod inline_image_tests {
    use super::*;

    #[test]
    fn test_inline_image_null_safe() {
        unsafe {
            // All functions should handle null pointers gracefully
            let count = dterm_terminal_inline_image_count(std::ptr::null());
            assert_eq!(count, 0);

            let mut info = DtermInlineImageInfo {
                id: 0,
                row: 0,
                col: 0,
                width_spec_type: 0,
                width_spec_value: 0,
                height_spec_type: 0,
                height_spec_value: 0,
                preserve_aspect_ratio: false,
                data_size: 0,
            };
            let result =
                dterm_terminal_inline_image_info(std::ptr::null(), 0, std::ptr::from_mut(&mut info));
            assert!(!result);

            let mut data: *const u8 = std::ptr::null();
            let mut len: usize = 0;
            let result = dterm_terminal_inline_image_data(
                std::ptr::null(),
                0,
                std::ptr::from_mut(&mut data),
                std::ptr::from_mut(&mut len),
            );
            assert!(!result);

            dterm_terminal_inline_image_clear(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_inline_image_from_terminal() {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(24, 80);

        // Initially empty
        assert_eq!(
            unsafe { dterm_terminal_inline_image_count(std::ptr::from_ref(&term)) },
            0
        );

        // Store an image via OSC 1337
        term.process(b"\x1b]1337;File=inline=1:SGVsbG8=\x07"); // "Hello"

        // Should now have 1 image
        assert_eq!(
            unsafe { dterm_terminal_inline_image_count(std::ptr::from_ref(&term)) },
            1
        );

        // Get image info
        let mut info = DtermInlineImageInfo {
            id: 0,
            row: 0,
            col: 0,
            width_spec_type: 0,
            width_spec_value: 0,
            height_spec_type: 0,
            height_spec_value: 0,
            preserve_aspect_ratio: false,
            data_size: 0,
        };
        unsafe {
            let result = dterm_terminal_inline_image_info(
                std::ptr::from_ref(&term),
                0,
                std::ptr::from_mut(&mut info),
            );
            assert!(result);
        }
        assert_eq!(info.id, 0);
        assert_eq!(info.row, 0);
        assert_eq!(info.col, 0);
        assert_eq!(info.data_size, 5); // "Hello" is 5 bytes
        assert!(info.preserve_aspect_ratio); // Default

        // Get image data
        let mut data: *const u8 = std::ptr::null();
        let mut len: usize = 0;
        unsafe {
            let result = dterm_terminal_inline_image_data(
                std::ptr::from_ref(&term),
                0,
                std::ptr::from_mut(&mut data),
                std::ptr::from_mut(&mut len),
            );
            assert!(result);
            assert!(!data.is_null());
            assert_eq!(len, 5);

            // Verify data contents
            let slice = std::slice::from_raw_parts(data, len);
            assert_eq!(slice, b"Hello");
        }

        // Clear images
        unsafe {
            dterm_terminal_inline_image_clear(std::ptr::from_mut(&mut term));
        }
        assert_eq!(
            unsafe { dterm_terminal_inline_image_count(std::ptr::from_ref(&term)) },
            0
        );
    }

    #[test]
    fn test_inline_image_with_dimensions() {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(24, 80);

        // Store image with dimensions
        term.process(b"\x1b]1337;File=width=100px;height=50;inline=1:SGVsbG8=\x07");

        let mut info = DtermInlineImageInfo {
            id: 0,
            row: 0,
            col: 0,
            width_spec_type: 0,
            width_spec_value: 0,
            height_spec_type: 0,
            height_spec_value: 0,
            preserve_aspect_ratio: false,
            data_size: 0,
        };
        unsafe {
            let result = dterm_terminal_inline_image_info(
                std::ptr::from_ref(&term),
                0,
                std::ptr::from_mut(&mut info),
            );
            assert!(result);
        }

        // width=100px -> type 2 (Pixels), value 100
        assert_eq!(info.width_spec_type, 2);
        assert_eq!(info.width_spec_value, 100);

        // height=50 -> type 1 (Cells), value 50
        assert_eq!(info.height_spec_type, 1);
        assert_eq!(info.height_spec_value, 50);
    }

    #[test]
    fn test_inline_image_out_of_bounds() {
        use crate::terminal::Terminal;

        let term = Terminal::new(24, 80);

        // Try to get image at invalid index
        let mut info = DtermInlineImageInfo {
            id: 0,
            row: 0,
            col: 0,
            width_spec_type: 0,
            width_spec_value: 0,
            height_spec_type: 0,
            height_spec_value: 0,
            preserve_aspect_ratio: false,
            data_size: 0,
        };
        unsafe {
            let result = dterm_terminal_inline_image_info(
                std::ptr::from_ref(&term),
                100,
                std::ptr::from_mut(&mut info),
            );
            assert!(!result);

            let mut data: *const u8 = std::ptr::null();
            let mut len: usize = 0;
            let result = dterm_terminal_inline_image_data(
                std::ptr::from_ref(&term),
                100,
                std::ptr::from_mut(&mut data),
                std::ptr::from_mut(&mut len),
            );
            assert!(!result);
        }
    }

    #[test]
    fn test_inline_image_available() {
        assert!(dterm_inline_image_available());
    }
}
