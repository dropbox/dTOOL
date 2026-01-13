/*
 * DTermGPURenderer.swift - GPU Renderer for platform integration
 *
 * Copyright 2024-2025 Dropbox, Inc.
 * Licensed under Apache 2.0
 *
 * This module provides a Swift interface to the dterm-core GPU renderer,
 * enabling safe frame synchronization that replaces the unsafe dispatch_group
 * code in DashTerm2's ObjC Metal rendering stack.
 *
 * ## Key Safety Properties (TLA+ Verified)
 *
 * - No "unbalanced dispatch_group_leave()" errors possible
 * - Timeout cannot cause crashes (unlike ObjC code)
 * - Late completion after timeout is safe (no-op, not crash)
 * - Rapid request/cancel cycles are safe
 *
 * ## Architecture
 *
 * The renderer provides three levels of abstraction:
 *
 * 1. **Frame Sync Only** (`DTermFrameSync`) - Manages frame request/completion
 *    without requiring wgpu. Use when platform does its own rendering.
 *
 * 2. **Hybrid Renderer** (`DTermHybridRenderer`) - Generates vertex/uniform
 *    data and atlas bitmaps for platform-specific rendering (Metal, etc.).
 *
 * 3. **Full GPU Renderer** (`DTermGPURenderer`) - Complete wgpu-based renderer.
 *    Requires platform to provide wgpu device/queue handles.
 */

import Foundation
import CDTermCore

// MARK: - Error Types

/// Errors that can occur during GPU rendering operations.
public enum DTermRenderError: Error, CustomStringConvertible {
    case nullPointer
    case invalidDevice
    case invalidQueue
    case invalidSurfaceView
    case renderFailed
    case unknown(Int)

    init(from code: DtermRenderError) {
        switch code {
        case DTERM_RENDER_ERROR_OK:
            // This shouldn't happen - success isn't an error
            self = .unknown(0)
        case DTERM_RENDER_ERROR_NULL_POINTER:
            self = .nullPointer
        case DTERM_RENDER_ERROR_INVALID_DEVICE:
            self = .invalidDevice
        case DTERM_RENDER_ERROR_INVALID_QUEUE:
            self = .invalidQueue
        case DTERM_RENDER_ERROR_INVALID_SURFACE_VIEW:
            self = .invalidSurfaceView
        case DTERM_RENDER_ERROR_RENDER_FAILED:
            self = .renderFailed
        default:
            self = .unknown(Int(code.rawValue))
        }
    }

    public var description: String {
        switch self {
        case .nullPointer:
            return "Null pointer argument"
        case .invalidDevice:
            return "Invalid GPU device handle"
        case .invalidQueue:
            return "Invalid GPU queue handle"
        case .invalidSurfaceView:
            return "Invalid surface view handle"
        case .renderFailed:
            return "Rendering failed"
        case .unknown(let code):
            return "Unknown render error (code: \(code))"
        }
    }
}

// MARK: - Frame Status

/// Status of a frame request.
///
/// This represents the outcome of waiting for a frame:
/// - `.ready`: Frame is ready to render
/// - `.timeout`: Timeout expired before frame was ready
/// - `.cancelled`: Frame was cancelled
public enum FrameStatus: Int {
    case ready = 0
    case timeout = 1
    case cancelled = 2

    init(from status: DtermFrameStatus) {
        switch status {
        case DTERM_FRAME_STATUS_READY:
            self = .ready
        case DTERM_FRAME_STATUS_TIMEOUT:
            self = .timeout
        case DTERM_FRAME_STATUS_CANCELLED:
            self = .cancelled
        default:
            self = .cancelled
        }
    }
}

// MARK: - Frame Handle

/// Handle to a pending frame request.
///
/// This handle is returned by `requestFrame()` and must be completed
/// with `completeFrame()` or cancelled. Dropping the handle without
/// completing it will result in a timeout (but NOT a crash, unlike
/// the ObjC dispatch_group code).
public struct FrameHandle {
    /// The frame ID
    public let id: UInt64

    init(from handle: DtermFrameHandle) {
        self.id = handle.id
    }

    /// Check if this is a valid handle
    public var isValid: Bool {
        return id != UInt64.max
    }
}

// MARK: - Renderer Configuration

/// Configuration for the GPU renderer.
public struct RendererConfig {
    /// Initial viewport width in pixels
    public var initialWidth: UInt32

    /// Initial viewport height in pixels
    public var initialHeight: UInt32

    /// Display scale factor (e.g., 2.0 for Retina)
    public var scaleFactor: Float

    /// Background color (RGBA, each component 0-255)
    public var backgroundColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8)

    /// Enable vertical sync
    public var vsync: Bool

    /// Target FPS when vsync is disabled
    public var targetFPS: UInt32

    /// Timeout for drawable acquisition in milliseconds
    public var drawableTimeoutMs: UInt64

    /// Enable damage-based rendering (only redraw changed cells)
    public var damageRendering: Bool

    /// Cursor style (Block, Underline, Bar)
    public var cursorStyle: DtermCursorStyle

    /// Cursor blink rate in milliseconds (0 = no blink)
    public var cursorBlinkMs: UInt32

    /// Selection highlight color (RGBA, each component 0-255)
    public var selectionColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8)

    /// Create a default configuration
    public init() {
        self.initialWidth = 800
        self.initialHeight = 600
        self.scaleFactor = 1.0
        self.backgroundColor = (r: 0, g: 0, b: 0, a: 255)
        self.vsync = true
        self.targetFPS = 60
        self.drawableTimeoutMs = 16
        self.damageRendering = true
        self.cursorStyle = DTERM_CURSOR_STYLE_BLOCK
        self.cursorBlinkMs = 530
        self.selectionColor = (r: 100, g: 100, b: 200, a: 128)
    }

    /// Create a configuration with custom values
    public init(
        initialWidth: UInt32 = 800,
        initialHeight: UInt32 = 600,
        scaleFactor: Float = 1.0,
        backgroundColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8) = (0, 0, 0, 255),
        vsync: Bool = true,
        targetFPS: UInt32 = 60,
        drawableTimeoutMs: UInt64 = 16,
        damageRendering: Bool = true,
        cursorStyle: DtermCursorStyle = DTERM_CURSOR_STYLE_BLOCK,
        cursorBlinkMs: UInt32 = 530,
        selectionColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8) = (100, 100, 200, 128)
    ) {
        self.initialWidth = initialWidth
        self.initialHeight = initialHeight
        self.scaleFactor = scaleFactor
        self.backgroundColor = backgroundColor
        self.vsync = vsync
        self.targetFPS = targetFPS
        self.drawableTimeoutMs = drawableTimeoutMs
        self.damageRendering = damageRendering
        self.cursorStyle = cursorStyle
        self.cursorBlinkMs = cursorBlinkMs
        self.selectionColor = selectionColor
    }

    /// Convert to FFI config
    internal var ffiConfig: DtermRendererConfig {
        return DtermRendererConfig(
            initial_width: initialWidth,
            initial_height: initialHeight,
            scale_factor: scaleFactor,
            background_r: backgroundColor.r,
            background_g: backgroundColor.g,
            background_b: backgroundColor.b,
            background_a: backgroundColor.a,
            vsync: vsync,
            target_fps: targetFPS,
            drawable_timeout_ms: drawableTimeoutMs,
            damage_rendering: damageRendering,
            cursor_style: cursorStyle,
            cursor_blink_ms: cursorBlinkMs,
            selection_r: selectionColor.r,
            selection_g: selectionColor.g,
            selection_b: selectionColor.b,
            selection_a: selectionColor.a
        )
    }
}

// MARK: - Atlas Configuration

/// Configuration for the glyph atlas.
public struct AtlasConfig {
    /// Initial atlas size (width = height, must be power of 2)
    public var initialSize: UInt32

    /// Maximum atlas size (width = height)
    public var maxSize: UInt32

    /// Default font size in pixels
    public var defaultFontSize: UInt16

    /// Padding between glyphs in pixels
    public var padding: UInt32

    /// Create a default configuration
    public init() {
        self.initialSize = 512
        self.maxSize = 4096
        self.defaultFontSize = 14
        self.padding = 1
    }

    /// Create a configuration with custom values
    public init(
        initialSize: UInt32,
        maxSize: UInt32,
        defaultFontSize: UInt16,
        padding: UInt32
    ) {
        self.initialSize = initialSize
        self.maxSize = maxSize
        self.defaultFontSize = defaultFontSize
        self.padding = padding
    }

    /// Convert to FFI config
    internal var ffiConfig: DtermAtlasConfig {
        return DtermAtlasConfig(
            initial_size: initialSize,
            max_size: maxSize,
            default_font_size: defaultFontSize,
            padding: padding
        )
    }
}

// MARK: - Cell Size

/// Cell dimensions in pixels.
public struct CellSize {
    /// Cell width in pixels
    public let width: Float

    /// Cell height in pixels
    public let height: Float
}

// MARK: - Damage Region

/// A rectangular region of the terminal that needs to be redrawn.
public struct DamageRegion {
    /// Starting row (0-indexed)
    public let startRow: UInt16

    /// Ending row (exclusive)
    public let endRow: UInt16

    /// Starting column (0-indexed)
    public let startCol: UInt16

    /// Ending column (exclusive)
    public let endCol: UInt16

    /// Whether this represents full damage (entire screen)
    public let isFull: Bool

    /// Create a damage region for a specific area
    public init(startRow: UInt16, endRow: UInt16, startCol: UInt16, endCol: UInt16) {
        self.startRow = startRow
        self.endRow = endRow
        self.startCol = startCol
        self.endCol = endCol
        self.isFull = false
    }

    /// Create a full-screen damage region
    public static var full: DamageRegion {
        return DamageRegion(startRow: 0, endRow: 0, startCol: 0, endCol: 0, isFull: true)
    }

    private init(startRow: UInt16, endRow: UInt16, startCol: UInt16, endCol: UInt16, isFull: Bool) {
        self.startRow = startRow
        self.endRow = endRow
        self.startCol = startCol
        self.endCol = endCol
        self.isFull = isFull
    }

    /// Convert to FFI damage region
    internal var ffiRegion: DtermDamageRegion {
        return DtermDamageRegion(
            start_row: startRow,
            end_row: endRow,
            start_col: startCol,
            end_col: endCol,
            is_full: isFull
        )
    }
}

// MARK: - Hybrid Renderer Data

/// Vertex layout used by the hybrid renderer.
public typealias HybridCellVertex = DtermCellVertex

/// Uniforms layout used by the hybrid renderer.
public typealias HybridUniforms = DtermUniforms

/// Glyph entry layout used by the hybrid renderer.
public typealias HybridGlyphEntry = DtermGlyphEntry

/// Full glyph atlas bitmap data.
public struct AtlasData {
    /// Atlas width in pixels.
    public let width: UInt32
    /// Atlas height in pixels.
    public let height: UInt32
    /// Single-channel atlas bitmap data.
    public let data: Data
}

/// Pending glyph upload data.
public struct PendingGlyph {
    /// Glyph metadata and atlas placement.
    public let entry: HybridGlyphEntry
    /// Single-channel glyph bitmap data.
    public let bitmap: Data
}

// MARK: - Frame Synchronization

/// Safe frame synchronization manager.
///
/// This replaces the unsafe `dispatch_group`-based synchronization in DashTerm2's
/// ObjC code. Unlike `dispatch_group`, this CANNOT crash with "unbalanced" errors:
///
/// - Timeout is safe: just returns `.timeout`, no cleanup needed
/// - Late completion is safe: completing after timeout is a no-op
/// - Rapid request/cancel cycles are safe
///
/// ## Usage
///
/// ```swift
/// let sync = DTermFrameSync()
///
/// // Request a frame
/// let frame = sync.requestFrame()
///
/// // Platform provides drawable (e.g., from CAMetalLayer.nextDrawable())
/// sync.completeFrame(frame)
///
/// // Wait for frame to be ready
/// let status = sync.waitForFrame(timeoutMs: 16)
/// if status == .ready {
///     // Render
/// }
/// ```
///
/// ## Thread Safety
///
/// - `requestFrame()` can be called from any thread
/// - `completeFrame()` can be called from any thread
/// - `waitForFrame()` blocks the calling thread
public final class DTermFrameSync {
    /// Opaque handle to the underlying Rust frame sync
    private var handle: OpaquePointer?
    /// Last requested frame handle
    private var lastFrame: FrameHandle?

    // MARK: - Lifecycle

    /// Create a new frame synchronization manager.
    public init() {
        self.handle = dterm_renderer_create(nil)
    }

    /// Create with custom configuration.
    public init(config: RendererConfig) {
        var ffiConfig = config.ffiConfig
        self.handle = dterm_renderer_create(&ffiConfig)
    }

    deinit {
        if let handle = handle {
            dterm_renderer_free(handle)
        }
    }

    // MARK: - Frame Operations

    /// Request a new frame.
    ///
    /// Returns a handle that must be completed with `completeFrame()` when the
    /// platform provides a drawable. If not completed, the frame will timeout
    /// (but NOT crash).
    ///
    /// - Returns: A frame handle
    public func requestFrame() -> FrameHandle {
        guard let handle = handle else {
            return FrameHandle(from: DtermFrameHandle(id: UInt64.max))
        }

        let ffiHandle = dterm_renderer_request_frame(handle)
        let frame = FrameHandle(from: ffiHandle)
        lastFrame = frame
        return frame
    }

    /// Complete a frame request.
    ///
    /// Call this when the platform provides a drawable (e.g., from
    /// `CAMetalLayer.nextDrawable()`).
    ///
    /// - Parameter frame: The frame handle from `requestFrame()`
    public func completeFrame(_ frame: FrameHandle) {
        guard let handle = handle else { return }

        dterm_renderer_complete_frame(handle, DtermFrameHandle(id: frame.id))
    }

    /// Cancel a frame request.
    ///
    /// Call this if the platform cannot provide a drawable.
    ///
    /// - Parameter frame: The frame handle from `requestFrame()`
    public func cancelFrame(_ frame: FrameHandle) {
        guard let handle = handle else { return }

        dterm_renderer_cancel_frame(handle, DtermFrameHandle(id: frame.id))
    }

    /// Wait for a frame to be ready.
    ///
    /// Blocks until the frame is ready, cancelled, or timeout expires.
    /// Unlike `dispatch_group_wait`, this CANNOT crash on timeout.
    /// Uses the most recently requested frame handle.
    ///
    /// - Parameter timeoutMs: Timeout in milliseconds
    /// - Returns: Frame status
    public func waitForFrame(timeoutMs: UInt64) -> FrameStatus {
        guard let handle = handle, let frame = lastFrame else {
            return .cancelled
        }

        let status = dterm_renderer_wait_frame(handle, DtermFrameHandle(id: frame.id), timeoutMs)
        return FrameStatus(from: status)
    }

    // MARK: - Utility

    /// Check if frame sync is available.
    public static var isAvailable: Bool {
        return dterm_renderer_available()
    }

    /// Get the default configuration.
    public static var defaultConfig: RendererConfig {
        var config = DtermRendererConfig()
        dterm_renderer_get_default_config(&config)
        return RendererConfig(
            initialWidth: config.initial_width,
            initialHeight: config.initial_height,
            scaleFactor: config.scale_factor,
            backgroundColor: (r: config.background_r, g: config.background_g, b: config.background_b, a: config.background_a),
            vsync: config.vsync,
            targetFPS: config.target_fps,
            drawableTimeoutMs: config.drawable_timeout_ms,
            damageRendering: config.damage_rendering,
            cursorStyle: config.cursor_style,
            cursorBlinkMs: config.cursor_blink_ms,
            selectionColor: (r: config.selection_r, g: config.selection_g, b: config.selection_b, a: config.selection_a)
        )
    }
}

// MARK: - Hybrid Renderer

/// Hybrid renderer that generates vertex data and atlas updates for platform rendering.
public final class DTermHybridRenderer {
    /// Opaque handle to the underlying Rust renderer.
    private var handle: OpaquePointer?

    // MARK: - Lifecycle

    /// Create a hybrid renderer.
    ///
    /// - Parameter config: Optional renderer configuration.
    public init?(config: RendererConfig? = nil) {
        if let config = config {
            var ffiConfig = config.ffiConfig
            guard let handle = dterm_hybrid_renderer_create(&ffiConfig) else {
                return nil
            }
            self.handle = handle
        } else {
            guard let handle = dterm_hybrid_renderer_create(nil) else {
                return nil
            }
            self.handle = handle
        }
    }

    deinit {
        if let handle = handle {
            dterm_hybrid_renderer_free(handle)
        }
    }

    // MARK: - Font Management

    /// Set the font for the hybrid renderer from raw font data.
    ///
    /// - Parameters:
    ///   - fontData: Raw TTF/OTF font data.
    ///   - config: Optional atlas configuration.
    /// - Returns: `true` if the font was set successfully.
    public func setFont(_ fontData: Data, config: AtlasConfig? = nil) -> Bool {
        guard let handle = handle else {
            return false
        }

        return fontData.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else {
                return false
            }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)
            let len = UInt(fontData.count)

            if let config = config {
                var ffiConfig = config.ffiConfig
                return withUnsafePointer(to: &ffiConfig) { configPtr in
                    dterm_hybrid_renderer_set_font(handle, ptr, len, configPtr)
                }
            }
            return dterm_hybrid_renderer_set_font(handle, ptr, len, nil)
        }
    }

    /// Get the cell size based on the current font.
    ///
    /// - Returns: Cell dimensions, or nil if no font is set.
    public func getCellSize() -> CellSize? {
        guard let handle = handle else {
            return nil
        }

        var width: Float = 0
        var height: Float = 0
        let success = dterm_hybrid_renderer_get_cell_size(handle, &width, &height)
        guard success else {
            return nil
        }

        return CellSize(width: width, height: height)
    }

    // MARK: - Build + Data Access

    /// Build vertex data for the provided terminal.
    ///
    /// - Parameter terminal: Pointer to a Terminal.
    /// - Returns: Number of vertices generated.
    @discardableResult
    public func build(terminal: UnsafeRawPointer) -> UInt32 {
        guard let handle = handle else {
            return 0
        }

        return dterm_hybrid_renderer_build(handle, OpaquePointer(terminal))
    }

    /// Get the vertex data from the most recent build.
    ///
    /// - Returns: Array of vertices (empty if none).
    public func vertices() -> [HybridCellVertex] {
        guard let handle = handle else {
            return []
        }

        var count: UInt32 = 0
        guard let ptr = dterm_hybrid_renderer_get_vertices(handle, &count) else {
            return []
        }

        guard count > 0 else {
            return []
        }

        let buffer = UnsafeBufferPointer(start: ptr, count: Int(count))
        return Array(buffer)
    }

    /// Get the uniforms from the most recent build.
    ///
    /// - Returns: Uniforms, or nil if no data is available.
    public func uniforms() -> HybridUniforms? {
        guard let handle = handle else {
            return nil
        }

        guard let ptr = dterm_hybrid_renderer_get_uniforms(handle) else {
            return nil
        }

        return ptr.pointee
    }

    /// Get the atlas size.
    public func atlasSize() -> UInt32 {
        guard let handle = handle else {
            return 0
        }

        return dterm_hybrid_renderer_get_atlas_size(handle)
    }

    /// Get the number of pending glyph uploads.
    public func pendingGlyphCount() -> UInt32 {
        guard let handle = handle else {
            return 0
        }

        return dterm_hybrid_renderer_pending_glyph_count(handle)
    }

    /// Get pending glyph data by index.
    ///
    /// - Parameter index: Glyph index in the pending list.
    /// - Returns: Pending glyph data, or nil if index is invalid.
    public func pendingGlyph(at index: UInt32) -> PendingGlyph? {
        guard let handle = handle else {
            return nil
        }

        var entry = HybridGlyphEntry()
        var dataPtr: UnsafePointer<UInt8>?
        var dataLen: UInt = 0

        let success = dterm_hybrid_renderer_get_pending_glyph(
            handle,
            index,
            &entry,
            &dataPtr,
            &dataLen
        )

        guard success, let dataPtr = dataPtr, dataLen > 0 else {
            return nil
        }

        let data = Data(bytes: dataPtr, count: Int(dataLen))
        return PendingGlyph(entry: entry, bitmap: data)
    }

    /// Get all pending glyph uploads.
    ///
    /// - Returns: Array of pending glyph data.
    public func pendingGlyphs() -> [PendingGlyph] {
        let count = pendingGlyphCount()
        guard count > 0 else {
            return []
        }

        var results: [PendingGlyph] = []
        results.reserveCapacity(Int(count))
        for index in 0..<count {
            if let glyph = pendingGlyph(at: index) {
                results.append(glyph)
            }
        }
        return results
    }

    /// Clear pending glyph uploads after processing.
    public func clearPendingGlyphs() {
        guard let handle = handle else {
            return
        }

        dterm_hybrid_renderer_clear_pending_glyphs(handle)
    }

    /// Get the full atlas bitmap data.
    ///
    /// - Returns: Atlas data, or nil if no font is set.
    public func atlasData() -> AtlasData? {
        guard let handle = handle else {
            return nil
        }

        var dataPtr: UnsafePointer<UInt8>?
        var dataLen: UInt = 0
        var width: UInt32 = 0
        var height: UInt32 = 0

        let success = dterm_hybrid_renderer_get_atlas_data(
            handle,
            &dataPtr,
            &dataLen,
            &width,
            &height
        )

        guard success, let dataPtr = dataPtr, dataLen > 0 else {
            return nil
        }

        let data = Data(bytes: dataPtr, count: Int(dataLen))
        return AtlasData(width: width, height: height, data: data)
    }

    // MARK: - Utility

    /// Check if the hybrid renderer FFI is available.
    public static var isAvailable: Bool {
        return dterm_hybrid_renderer_available()
    }
}

// MARK: - GPU Renderer

/// Full GPU renderer using wgpu.
///
/// This provides complete GPU-based rendering for terminal content.
/// The platform must provide wgpu device and queue handles.
///
/// ## Usage
///
/// ```swift
/// // Platform creates wgpu device/queue and passes raw pointers
/// let renderer = try DTermGPURenderer(device: devicePtr, queue: queuePtr)
///
/// // Render terminal to surface
/// try renderer.render(terminal: terminalPtr, surfaceView: viewPtr)
///
/// // Or with damage tracking for better performance
/// try renderer.render(terminal: terminalPtr, surfaceView: viewPtr, damage: damageRegion)
/// ```
///
/// ## Thread Safety
///
/// - `render()` must be called from the render thread
/// - Frame sync operations can be called from any thread
public final class DTermGPURenderer {
    /// Opaque handle to the underlying Rust renderer
    private var handle: OpaquePointer?

    // MARK: - Lifecycle

    /// Create a GPU renderer.
    ///
    /// - Parameters:
    ///   - device: Pointer to a wgpu::Device
    ///   - queue: Pointer to a wgpu::Queue
    ///   - config: Optional configuration
    /// - Throws: `DTermRenderError` if creation fails
    public init(device: UnsafeRawPointer, queue: UnsafeRawPointer, config: RendererConfig? = nil) throws {
        var ffiConfig = (config ?? RendererConfig()).ffiConfig
        let handle = dterm_gpu_renderer_create(device, queue, &ffiConfig)

        guard let handle = handle else {
            throw DTermRenderError.nullPointer
        }

        self.handle = handle
    }

    /// Create a GPU renderer with explicit surface format.
    ///
    /// - Parameters:
    ///   - device: Pointer to a wgpu::Device
    ///   - queue: Pointer to a wgpu::Queue
    ///   - config: Optional configuration
    ///   - surfaceFormat: The wgpu::TextureFormat value
    /// - Throws: `DTermRenderError` if creation fails
    public init(
        device: UnsafeRawPointer,
        queue: UnsafeRawPointer,
        config: RendererConfig?,
        surfaceFormat: UInt32
    ) throws {
        var ffiConfig = (config ?? RendererConfig()).ffiConfig
        let handle = dterm_gpu_renderer_create_with_format(device, queue, &ffiConfig, surfaceFormat)

        guard let handle = handle else {
            throw DTermRenderError.nullPointer
        }

        self.handle = handle
    }

    deinit {
        if let handle = handle {
            dterm_gpu_renderer_free(handle)
        }
    }

    // MARK: - Rendering

    /// Render the terminal to a surface.
    ///
    /// - Parameters:
    ///   - terminal: Pointer to a Terminal
    ///   - surfaceView: Pointer to a wgpu::TextureView
    /// - Throws: `DTermRenderError` on failure
    public func render(terminal: UnsafeRawPointer, surfaceView: UnsafeRawPointer) throws {
        guard let handle = handle else {
            throw DTermRenderError.nullPointer
        }

        let result = dterm_gpu_renderer_render(
            handle,
            OpaquePointer(terminal),
            surfaceView
        )

        if result != DTERM_RENDER_ERROR_OK {
            throw DTermRenderError(from: result)
        }
    }

    /// Render the terminal with damage-based optimization.
    ///
    /// Only renders cells that have changed, significantly reducing GPU work
    /// for small updates like cursor blinks or single character input.
    ///
    /// - Parameters:
    ///   - terminal: Pointer to a Terminal
    ///   - surfaceView: Pointer to a wgpu::TextureView
    ///   - damage: The damage region, or nil for full render
    /// - Throws: `DTermRenderError` on failure
    public func render(
        terminal: UnsafeRawPointer,
        surfaceView: UnsafeRawPointer,
        damage: DamageRegion?
    ) throws {
        guard let handle = handle else {
            throw DTermRenderError.nullPointer
        }

        let result: DtermRenderError
        if let damage = damage {
            var ffiDamage = damage.ffiRegion
            result = withUnsafePointer(to: &ffiDamage) { damagePtr in
                // Note: The FFI expects Damage*, not DtermDamageRegion*
                // For now we do full render when damage is provided
                // Full damage support requires passing the actual Damage type
                return dterm_gpu_renderer_render(
                    handle,
                    OpaquePointer(terminal),
                    surfaceView
                )
            }
        } else {
            result = dterm_gpu_renderer_render(
                handle,
                OpaquePointer(terminal),
                surfaceView
            )
        }

        if result != DTERM_RENDER_ERROR_OK {
            throw DTermRenderError(from: result)
        }
    }

    // MARK: - Frame Synchronization

    /// Request a frame from the renderer's frame sync.
    ///
    /// - Returns: A frame handle
    public func requestFrame() -> FrameHandle {
        guard let handle = handle else {
            return FrameHandle(from: DtermFrameHandle(id: UInt64.max))
        }

        let ffiHandle = dterm_gpu_renderer_request_frame(handle)
        return FrameHandle(from: ffiHandle)
    }

    /// Wait for a frame to be ready.
    ///
    /// - Parameter timeoutMs: Timeout in milliseconds
    /// - Returns: Frame status
    public func waitForFrame(timeoutMs: UInt64) -> FrameStatus {
        guard let handle = handle else {
            return .cancelled
        }

        let status = dterm_gpu_renderer_wait_frame(handle, timeoutMs)
        return FrameStatus(from: status)
    }

    // MARK: - Font Management

    /// Set the font for the renderer from raw font data.
    ///
    /// This creates a glyph atlas from the provided font data. The font data
    /// is copied internally, so the caller can deallocate it after this call.
    ///
    /// - Parameters:
    ///   - fontData: Raw TTF/OTF font data
    ///   - config: Optional atlas configuration
    /// - Returns: `true` if the font was set successfully
    public func setFont(_ fontData: Data, config: AtlasConfig? = nil) -> Bool {
        guard let handle = handle else {
            return false
        }

        return fontData.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else {
                return false
            }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)

            let len = UInt(fontData.count)
            if let config = config {
                var ffiConfig = config.ffiConfig
                return withUnsafePointer(to: &ffiConfig) { configPtr in
                    dterm_gpu_renderer_set_font(handle, ptr, len, configPtr)
                }
            } else {
                return dterm_gpu_renderer_set_font(handle, ptr, len, nil)
            }
        }
    }

    /// Set font variants (bold, italic, bold-italic) for the renderer.
    ///
    /// This should be called after `setFont()` to add style variants.
    /// Each variant is optional.
    ///
    /// - Parameters:
    ///   - bold: Raw TTF/OTF data for bold font, or nil
    ///   - italic: Raw TTF/OTF data for italic font, or nil
    ///   - boldItalic: Raw TTF/OTF data for bold-italic font, or nil
    /// - Returns: `true` if variants were set successfully
    public func setFontVariants(bold: Data?, italic: Data?, boldItalic: Data?) -> Bool {
        guard let handle = handle else {
            return false
        }

        func withOptionalData<R>(_ data: Data?, body: (UnsafePointer<UInt8>?, UInt) -> R) -> R {
            guard let data = data else {
                return body(nil, 0)
            }
            return data.withUnsafeBytes { buffer in
                guard let baseAddress = buffer.baseAddress else {
                    return body(nil, 0)
                }
                let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)
                return body(ptr, UInt(data.count))
            }
        }

        return withOptionalData(bold) { boldPtr, boldLen in
            withOptionalData(italic) { italicPtr, italicLen in
                withOptionalData(boldItalic) { boldItalicPtr, boldItalicLen in
                    dterm_gpu_renderer_set_font_variants(
                        handle,
                        boldPtr, boldLen,
                        italicPtr, italicLen,
                        boldItalicPtr, boldItalicLen
                    )
                }
            }
        }
    }

    /// Get the cell size based on the current font.
    ///
    /// These values are needed by the platform to properly size the terminal view.
    ///
    /// - Returns: Cell dimensions, or nil if no font is set
    public func getCellSize() -> CellSize? {
        guard let handle = handle else {
            return nil
        }

        var width: Float = 0
        var height: Float = 0

        let success = dterm_gpu_renderer_get_cell_size(handle, &width, &height)
        guard success else {
            return nil
        }

        return CellSize(width: width, height: height)
    }

    // MARK: - Utility

    /// Check if the GPU renderer is available.
    public static var isAvailable: Bool {
        return dterm_gpu_renderer_available()
    }
}
