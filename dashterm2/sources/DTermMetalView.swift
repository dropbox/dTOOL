//
//  DTermMetalView.swift
//  DashTerm2
//
//  Metal view that uses dterm-core hybrid renderer for terminal rendering.
//  Replaces the complex iTermMetalView + iTermMetalDriver + iTermPromise stack.
//
//  Architecture:
//  - Rust (dterm-core) generates vertex data and manages glyph atlas
//  - Swift (this file) manages Metal pipeline, textures, and draw calls
//
//  Created by DashTerm2 AI Worker on 2024-12-30.
//

import AppKit
import Metal
import QuartzCore

/// Delegate protocol for DTermMetalView resize notifications.
@objc public protocol DTermMetalViewDelegate: AnyObject {
    /// Called when the drawable size changes.
    func dtermMetalView(_ view: DTermMetalView, drawableSizeWillChange size: CGSize)
}

/// Metal view that delegates vertex generation to dterm-core's hybrid renderer.
///
/// This view uses a hybrid rendering approach:
/// - **dterm-core (Rust)**: Generates vertex data and manages glyph atlas
/// - **Swift (Metal)**: Creates GPU resources and executes draw calls
///
/// This architecture eliminates the complex ObjC rendering stack while leveraging
/// Rust's memory safety for the computationally intensive parts.
///
/// ## Frame Flow
///
/// 1. Display link fires at vsync
/// 2. Swift calls `hybridRenderer.build(terminal:)` to generate vertex data
/// 3. Swift uploads any pending glyphs to the atlas texture
/// 4. Swift creates Metal buffer from vertex data
/// 5. Swift executes Metal draw calls
/// 6. Swift presents drawable
///
/// ## Thread Safety
///
/// - Display link marshals to main thread via DispatchSource
/// - All Metal operations happen on main thread
/// - dterm-core hybrid renderer is internally synchronized
@MainActor
@objc(DTermMetalView)
public class DTermMetalView: NSView {

    // MARK: - Metal Pipeline State

    /// The Metal layer for rendering.
    private var metalLayer: CAMetalLayer!

    /// Metal device.
    private var metalDevice: MTLDevice?

    /// Command queue for Metal commands.
    private var commandQueue: MTLCommandQueue?

    /// Render pipeline state for cell rendering.
    private var cellPipelineState: MTLRenderPipelineState?

    // MARK: - Multi-Pass Pipeline States

    /// Pipeline for background pass (solid color quads, no blending).
    private var backgroundPipelineState: MTLRenderPipelineState?

    /// Pipeline for glyph pass (textured quads from atlas, alpha blending).
    private var glyphPipelineState: MTLRenderPipelineState?

    /// Pipeline for decoration pass (underlines, strikethrough, alpha blending).
    private var decorationPipelineState: MTLRenderPipelineState?

    /// Atlas texture sampler.
    private var atlasSampler: MTLSamplerState?

    /// Glyph atlas texture (R8 format).
    private var atlasTexture: MTLTexture?

    /// Current atlas size (to detect when we need to recreate).
    private var currentAtlasSize: UInt32 = 0

    // MARK: - Display Link

    /// CVDisplayLink for vsync-aligned frame callbacks.
    private var displayLink: CVDisplayLink?

    /// Dispatch source for display link to main thread marshaling.
    private let displaySource: DispatchSourceUserDataAdd

    // MARK: - Renderer State

    /// The dterm-core hybrid renderer.
    private var hybridRenderer: DTermHybridRenderer?

    /// The dterm-core terminal to render.
    private weak var terminal: DTermCore?

    /// Platform-side glyph atlas manager (for fonts without file URLs).
    private var glyphAtlasManager: DTermGlyphAtlasManager?

    /// Whether we're using platform-rendered glyphs (vs fontdue).
    private var usingPlatformGlyphs: Bool = false

    /// Public getter for testing - indicates if platform glyph atlas is in use.
    @objc public var isPlatformGlyphsEnabled: Bool {
        return usingPlatformGlyphs
    }

    /// Animation time for cursor blink, etc.
    private var animationTime: Float = 0.0

    /// Last frame time for animation delta.
    private var lastFrameTime: CFTimeInterval = 0

    // MARK: - Performance Tracking

    /// Frame times for FPS calculation (rolling window).
    private var frameTimestamps: [CFTimeInterval] = []

    /// Maximum frames to track for FPS calculation.
    private let fpsWindowSize: Int = 60

    /// Total frames rendered since start.
    private var totalFramesRendered: UInt64 = 0

    /// Timestamp when rendering started.
    private var renderStartTime: CFTimeInterval = 0

    /// Last GPU frame time in milliseconds (from command buffer completion).
    private var lastGpuFrameTimeMs: Double = 0

    // MARK: - Image Rendering

    /// Render pipeline state for image rendering.
    private var imagePipelineState: MTLRenderPipelineState?

    /// Sampler for image textures.
    private var imageSampler: MTLSamplerState?

    /// Cache of Sixel image textures by cursor position (row * 10000 + col).
    private var sixelTextureCache: [Int: MTLTexture] = [:]

    /// Cache of Kitty image textures by image ID.
    private var kittyTextureCache: [UInt32: MTLTexture] = [:]

    /// Maximum number of cached image textures before eviction.
    private let maxCachedImages: Int = 64

    /// Last known Kitty dirty flag state for change detection.
    private var lastKittyDirtyState: Bool = false

    // MARK: - Public Properties

    /// Delegate for size change notifications.
    @objc public weak var delegate: DTermMetalViewDelegate?

    /// Whether rendering is paused.
    @objc public var paused: Bool = false {
        didSet {
            if paused {
                stopDisplayLink()
            } else {
                startDisplayLink()
            }
        }
    }

    /// Clear color for the view background.
    @objc public var clearColor: MTLClearColor = MTLClearColorMake(0.0, 0.0, 0.0, 1.0)

    /// Current drawable size in pixels.
    @objc public var drawableSize: CGSize {
        return metalLayer?.drawableSize ?? .zero
    }

    /// The Metal device.
    @objc public var device: MTLDevice? {
        return metalDevice
    }

    // MARK: - Performance Metrics (Public)

    /// Current frames per second (rolling average over last 60 frames).
    @objc public var currentFPS: Double {
        guard frameTimestamps.count >= 2 else { return 0 }
        let oldest = frameTimestamps.first!
        let newest = frameTimestamps.last!
        let elapsed = newest - oldest
        guard elapsed > 0 else { return 0 }
        return Double(frameTimestamps.count - 1) / elapsed
    }

    /// Average FPS since rendering started.
    @objc public var averageFPS: Double {
        guard renderStartTime > 0 && totalFramesRendered > 0 else { return 0 }
        let elapsed = CACurrentMediaTime() - renderStartTime
        guard elapsed > 0 else { return 0 }
        return Double(totalFramesRendered) / elapsed
    }

    /// Total frames rendered since start.
    @objc public var frameCount: UInt64 {
        return totalFramesRendered
    }

    /// Last GPU frame time in milliseconds.
    @objc public var gpuFrameTimeMs: Double {
        return lastGpuFrameTimeMs
    }

    /// Frame time in milliseconds (1000 / FPS).
    @objc public var frameTimeMs: Double {
        let fps = currentFPS
        guard fps > 0 else { return 0 }
        return 1000.0 / fps
    }

    /// Performance summary string for debugging.
    @objc public var performanceSummary: String {
        return String(format: "FPS: %.1f (avg: %.1f) | Frame: %.2fms | GPU: %.2fms | Frames: %llu",
                      currentFPS, averageFPS, frameTimeMs, gpuFrameTimeMs, totalFramesRendered)
    }

    // MARK: - Initialization

    /// Create a new DTermMetalView (ObjC-compatible).
    @objc public override init(frame: CGRect) {
        self.terminal = nil
        self.displaySource = DispatchSource.makeUserDataAddSource(queue: .main)

        super.init(frame: frame)

        setupMetal()
        setupHybridRenderer()
        setupDisplaySource()
        setupDisplayLink()
    }

    /// Create a new DTermMetalView with terminal.
    public init(frame: CGRect, terminal: DTermCore? = nil) {
        self.terminal = terminal
        self.displaySource = DispatchSource.makeUserDataAddSource(queue: .main)

        super.init(frame: frame)

        setupMetal()
        setupHybridRenderer()
        setupDisplaySource()
        setupDisplayLink()
    }

    required init?(coder: NSCoder) {
        self.displaySource = DispatchSource.makeUserDataAddSource(queue: .main)

        super.init(coder: coder)

        setupMetal()
        setupHybridRenderer()
        setupDisplaySource()
        setupDisplayLink()
    }

    deinit {
        MainActor.assumeIsolated {
            stopDisplayLink()
            displaySource.cancel()
        }
    }

    // MARK: - Metal Setup

    private func setupMetal() {
        // Get Metal device
        metalDevice = MTLCreateSystemDefaultDevice()
        guard let device = metalDevice else {
            DLog("DTermMetalView: No Metal device available")
            return
        }

        // Create command queue
        commandQueue = device.makeCommandQueue()

        // Setup Metal layer
        metalLayer = CAMetalLayer()
        metalLayer.device = device
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        metalLayer.contentsScale = window?.backingScaleFactor ?? 2.0
        metalLayer.displaySyncEnabled = true

        wantsLayer = true
        layer = metalLayer

        // Create pipeline
        setupPipeline(device: device)

        // Create sampler
        setupSampler(device: device)

        // Create image pipeline
        setupImagePipeline(device: device)

        updateDrawableSize()
    }

    private func setupPipeline(device: MTLDevice) {
        // Load shaders from default library
        guard let library = device.makeDefaultLibrary() else {
            DLog("DTermMetalView: Failed to create default library")
            return
        }

        guard let vertexFunction = library.makeFunction(name: "dtermCellVertex"),
              let fragmentFunction = library.makeFunction(name: "dtermCellFragment") else {
            DLog("DTermMetalView: Failed to load shader functions")
            return
        }

        // Create pipeline descriptor
        let pipelineDescriptor = MTLRenderPipelineDescriptor()
        pipelineDescriptor.vertexFunction = vertexFunction
        pipelineDescriptor.fragmentFunction = fragmentFunction
        pipelineDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        // Enable blending for text
        pipelineDescriptor.colorAttachments[0].isBlendingEnabled = true
        pipelineDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        pipelineDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        pipelineDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .one
        pipelineDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha

        // Create vertex descriptor matching DtermCellVertex (64 bytes)
        let vertexDescriptor = MTLVertexDescriptor()

        // position: float2 (offset 0)
        vertexDescriptor.attributes[0].format = .float2
        vertexDescriptor.attributes[0].offset = 0
        vertexDescriptor.attributes[0].bufferIndex = 0

        // uv: float2 (offset 8)
        vertexDescriptor.attributes[1].format = .float2
        vertexDescriptor.attributes[1].offset = 8
        vertexDescriptor.attributes[1].bufferIndex = 0

        // fg_color: float4 (offset 16)
        vertexDescriptor.attributes[2].format = .float4
        vertexDescriptor.attributes[2].offset = 16
        vertexDescriptor.attributes[2].bufferIndex = 0

        // bg_color: float4 (offset 32)
        vertexDescriptor.attributes[3].format = .float4
        vertexDescriptor.attributes[3].offset = 32
        vertexDescriptor.attributes[3].bufferIndex = 0

        // flags: uint (offset 48)
        vertexDescriptor.attributes[4].format = .uint
        vertexDescriptor.attributes[4].offset = 48
        vertexDescriptor.attributes[4].bufferIndex = 0

        // Layout: 64 bytes per vertex
        vertexDescriptor.layouts[0].stride = 64
        vertexDescriptor.layouts[0].stepFunction = .perVertex
        vertexDescriptor.layouts[0].stepRate = 1

        pipelineDescriptor.vertexDescriptor = vertexDescriptor

        do {
            cellPipelineState = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
        } catch {
            DLog("DTermMetalView: Failed to create pipeline state: \(error)")
        }

        // Also create the specialized multi-pass pipelines
        setupMultiPassPipelines(device: device, library: library)
    }

    /// Creates specialized pipeline states for multi-pass rendering.
    /// This separates backgrounds, glyphs, and decorations for cleaner rendering
    /// without GPU branch divergence.
    private func setupMultiPassPipelines(device: MTLDevice, library: MTLLibrary) {
        // Shared vertex descriptor (all use DtermCellVertex - 64 bytes)
        let vertexDescriptor = MTLVertexDescriptor()

        // position: float2 (offset 0)
        vertexDescriptor.attributes[0].format = .float2
        vertexDescriptor.attributes[0].offset = 0
        vertexDescriptor.attributes[0].bufferIndex = 0

        // uv: float2 (offset 8)
        vertexDescriptor.attributes[1].format = .float2
        vertexDescriptor.attributes[1].offset = 8
        vertexDescriptor.attributes[1].bufferIndex = 0

        // fg_color: float4 (offset 16)
        vertexDescriptor.attributes[2].format = .float4
        vertexDescriptor.attributes[2].offset = 16
        vertexDescriptor.attributes[2].bufferIndex = 0

        // bg_color: float4 (offset 32)
        vertexDescriptor.attributes[3].format = .float4
        vertexDescriptor.attributes[3].offset = 32
        vertexDescriptor.attributes[3].bufferIndex = 0

        // flags: uint (offset 48)
        vertexDescriptor.attributes[4].format = .uint
        vertexDescriptor.attributes[4].offset = 48
        vertexDescriptor.attributes[4].bufferIndex = 0

        // Layout: 64 bytes per vertex
        vertexDescriptor.layouts[0].stride = 64
        vertexDescriptor.layouts[0].stepFunction = .perVertex
        vertexDescriptor.layouts[0].stepRate = 1

        // Common vertex shader for all passes
        guard let vertexFunction = library.makeFunction(name: "dtermCellVertex") else {
            DLog("DTermMetalView: Failed to load dtermCellVertex shader")
            return
        }

        // MARK: Background Pipeline (no blending)
        if let bgFragment = library.makeFunction(name: "dtermBackgroundFragment") {
            let bgDescriptor = MTLRenderPipelineDescriptor()
            bgDescriptor.vertexFunction = vertexFunction
            bgDescriptor.fragmentFunction = bgFragment
            bgDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
            // No blending - backgrounds are opaque
            bgDescriptor.colorAttachments[0].isBlendingEnabled = false
            bgDescriptor.vertexDescriptor = vertexDescriptor

            do {
                backgroundPipelineState = try device.makeRenderPipelineState(descriptor: bgDescriptor)
            } catch {
                DLog("DTermMetalView: Failed to create background pipeline: \(error)")
            }
        }

        // MARK: Glyph Pipeline (alpha blending with atlas texture)
        if let glyphFragment = library.makeFunction(name: "dtermGlyphFragment") {
            let glyphDescriptor = MTLRenderPipelineDescriptor()
            glyphDescriptor.vertexFunction = vertexFunction
            glyphDescriptor.fragmentFunction = glyphFragment
            glyphDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
            // Alpha blending for text
            glyphDescriptor.colorAttachments[0].isBlendingEnabled = true
            glyphDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
            glyphDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
            glyphDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .one
            glyphDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
            glyphDescriptor.vertexDescriptor = vertexDescriptor

            do {
                glyphPipelineState = try device.makeRenderPipelineState(descriptor: glyphDescriptor)
            } catch {
                DLog("DTermMetalView: Failed to create glyph pipeline: \(error)")
            }
        }

        // MARK: Decoration Pipeline (alpha blending, solid color)
        if let decoFragment = library.makeFunction(name: "dtermDecorationFragment") {
            let decoDescriptor = MTLRenderPipelineDescriptor()
            decoDescriptor.vertexFunction = vertexFunction
            decoDescriptor.fragmentFunction = decoFragment
            decoDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
            // Alpha blending for decorations on top of glyphs
            decoDescriptor.colorAttachments[0].isBlendingEnabled = true
            decoDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
            decoDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
            decoDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .one
            decoDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
            decoDescriptor.vertexDescriptor = vertexDescriptor

            do {
                decorationPipelineState = try device.makeRenderPipelineState(descriptor: decoDescriptor)
            } catch {
                DLog("DTermMetalView: Failed to create decoration pipeline: \(error)")
            }
        }

        DLog("DTermMetalView: Multi-pass pipelines created (bg: \(backgroundPipelineState != nil), glyph: \(glyphPipelineState != nil), deco: \(decorationPipelineState != nil))")
    }

    private func setupSampler(device: MTLDevice) {
        let samplerDescriptor = MTLSamplerDescriptor()
        // Use nearest-neighbor filtering for crisp, pixel-perfect text rendering.
        // Linear filtering causes blurry text because it interpolates between texels.
        samplerDescriptor.minFilter = .nearest
        samplerDescriptor.magFilter = .nearest
        samplerDescriptor.mipFilter = .notMipmapped
        samplerDescriptor.sAddressMode = .clampToEdge
        samplerDescriptor.tAddressMode = .clampToEdge

        atlasSampler = device.makeSamplerState(descriptor: samplerDescriptor)
    }

    private func setupImagePipeline(device: MTLDevice) {
        guard let library = device.makeDefaultLibrary() else {
            DLog("DTermMetalView: Failed to create default library for image pipeline")
            return
        }

        guard let vertexFunction = library.makeFunction(name: "dtermImageVertex"),
              let fragmentFunction = library.makeFunction(name: "dtermImageFragment") else {
            DLog("DTermMetalView: Failed to load image shader functions")
            return
        }

        let pipelineDescriptor = MTLRenderPipelineDescriptor()
        pipelineDescriptor.vertexFunction = vertexFunction
        pipelineDescriptor.fragmentFunction = fragmentFunction
        pipelineDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        // Enable alpha blending for image transparency
        pipelineDescriptor.colorAttachments[0].isBlendingEnabled = true
        pipelineDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        pipelineDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        pipelineDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .one
        pipelineDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha

        // Image vertex descriptor matching DTermImageVertex (32 bytes)
        let vertexDescriptor = MTLVertexDescriptor()

        // position: float2 (offset 0)
        vertexDescriptor.attributes[0].format = .float2
        vertexDescriptor.attributes[0].offset = 0
        vertexDescriptor.attributes[0].bufferIndex = 0

        // uv: float2 (offset 8)
        vertexDescriptor.attributes[1].format = .float2
        vertexDescriptor.attributes[1].offset = 8
        vertexDescriptor.attributes[1].bufferIndex = 0

        // tint: float4 (offset 16)
        vertexDescriptor.attributes[2].format = .float4
        vertexDescriptor.attributes[2].offset = 16
        vertexDescriptor.attributes[2].bufferIndex = 0

        // Layout: 32 bytes per vertex
        vertexDescriptor.layouts[0].stride = 32
        vertexDescriptor.layouts[0].stepFunction = .perVertex
        vertexDescriptor.layouts[0].stepRate = 1

        pipelineDescriptor.vertexDescriptor = vertexDescriptor

        do {
            imagePipelineState = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
        } catch {
            DLog("DTermMetalView: Failed to create image pipeline state: \(error)")
        }

        // Create sampler for images (same settings as atlas)
        let samplerDescriptor = MTLSamplerDescriptor()
        samplerDescriptor.minFilter = .linear
        samplerDescriptor.magFilter = .linear
        samplerDescriptor.mipFilter = .notMipmapped
        samplerDescriptor.sAddressMode = .clampToEdge
        samplerDescriptor.tAddressMode = .clampToEdge
        imageSampler = device.makeSamplerState(descriptor: samplerDescriptor)
    }

    // MARK: - Hybrid Renderer Setup

    private func setupHybridRenderer() {
        guard DTermHybridRenderer.isAvailable else {
            DLog("DTermMetalView: Hybrid renderer not available")
            return
        }

        hybridRenderer = DTermHybridRenderer()
        if hybridRenderer == nil {
            DLog("DTermMetalView: Failed to create hybrid renderer")
        }
    }

    // MARK: - Display Link Setup

    /// Counter for display link event logging
    private static var eventCounter: Int = 0

    private func setupDisplaySource() {
        NSLog("[DTermMetalView] setupDisplaySource: configuring event handler")
        displaySource.setEventHandler { [weak self] in
            DTermMetalView.eventCounter += 1
            if DTermMetalView.eventCounter % 60 == 1 {
                NSLog("[DTermMetalView] displaySource event handler fired (event #%d)", DTermMetalView.eventCounter)
            }
            Task { @MainActor in
                self?.render()
            }
        }
        displaySource.resume()
        NSLog("[DTermMetalView] setupDisplaySource: display source resumed")
    }

    private func setupDisplayLink() {
        var displayLink: CVDisplayLink?
        let createStatus = CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        NSLog("[DTermMetalView] setupDisplayLink: CVDisplayLinkCreateWithActiveCGDisplays returned %d", createStatus)

        guard let displayLink else {
            NSLog("[DTermMetalView] Failed to create display link")
            DLog("DTermMetalView: Failed to create display link")
            return
        }

        self.displayLink = displayLink

        let displaySourcePtr = Unmanaged.passUnretained(displaySource).toOpaque()
        let callbackStatus = CVDisplayLinkSetOutputCallback(displayLink, { _, _, _, _, _, userInfo -> CVReturn in
            guard let userInfo else { return kCVReturnSuccess }
            let source = Unmanaged<DispatchSourceUserDataAdd>.fromOpaque(userInfo).takeUnretainedValue()
            source.add(data: 1)
            return kCVReturnSuccess
        }, displaySourcePtr)
        NSLog("[DTermMetalView] setupDisplayLink: CVDisplayLinkSetOutputCallback returned %d", callbackStatus)

        // Start display link (respects paused state)
        if !paused {
            let startStatus = CVDisplayLinkStart(displayLink)
            NSLog("[DTermMetalView] setupDisplayLink: CVDisplayLinkStart returned %d, isRunning=%d", startStatus, CVDisplayLinkIsRunning(displayLink) ? 1 : 0)
        } else {
            NSLog("[DTermMetalView] setupDisplayLink: paused=true, not starting display link")
        }
    }

    private func startDisplayLink() {
        guard let displayLink, !CVDisplayLinkIsRunning(displayLink) else { return }
        CVDisplayLinkStart(displayLink)
    }

    private func stopDisplayLink() {
        guard let displayLink, CVDisplayLinkIsRunning(displayLink) else { return }
        CVDisplayLinkStop(displayLink)
    }

    // MARK: - Rendering

    /// Frame counter for debug logging (only logs every N frames)
    private var debugFrameCounter = 0

    private func render() {
        debugFrameCounter += 1

        // DEBUG: Log first 5 frames then every 60 to debug decoration vertices
        let shouldLog = debugFrameCounter <= 5 || debugFrameCounter % 60 == 1
        if shouldLog {
            NSLog("[DTermMetalView] render: frame %d starting", debugFrameCounter)
        }

        guard !paused else {
            if shouldLog { NSLog("[DTermMetalView] render: paused, skipping") }
            return
        }

        guard let device = metalDevice,
              let commandQueue = commandQueue,
              let pipelineState = cellPipelineState,
              let sampler = atlasSampler else {
            if shouldLog {
                NSLog("[DTermMetalView] render: Missing pipeline state (device=%d, queue=%d, pipeline=%d, sampler=%d)",
                      metalDevice != nil ? 1 : 0, self.commandQueue != nil ? 1 : 0,
                      cellPipelineState != nil ? 1 : 0, atlasSampler != nil ? 1 : 0)
            }
            renderFallback()
            return
        }

        guard let drawable = metalLayer.nextDrawable() else {
            return
        }

        // Track frame timing for FPS calculation
        let currentTime = CACurrentMediaTime()

        // Initialize render start time on first frame
        if renderStartTime == 0 {
            renderStartTime = currentTime
        }

        // Update rolling frame timestamp window
        frameTimestamps.append(currentTime)
        if frameTimestamps.count > fpsWindowSize {
            frameTimestamps.removeFirst()
        }

        // Increment total frame counter
        totalFramesRendered += 1

        // Update animation time
        if lastFrameTime > 0 {
            let delta = Float(currentTime - lastFrameTime)
            animationTime += delta
        }
        lastFrameTime = currentTime

        // If no hybrid renderer or terminal, render blank
        guard let hybridRenderer = hybridRenderer,
              let terminal = terminal else {
            if shouldLog {
                NSLog("[DTermMetalView] render: No terminal (hybridRenderer=%d, terminal=%d)",
                      self.hybridRenderer != nil ? 1 : 0, self.terminal != nil ? 1 : 0)
            }
            renderBlank(drawable: drawable, commandQueue: commandQueue)
            return
        }

        if shouldLog {
            NSLog("[DTermMetalView] render: hybridRenderer and terminal OK, building vertices")
        }

        // Build vertex data from terminal
        let vertexCount = hybridRenderer.build(terminal: terminal)

        if vertexCount == 0 {
            if shouldLog {
                NSLog("[DTermMetalView] render: vertexCount=0, rendering blank")
            }
            renderBlank(drawable: drawable, commandQueue: commandQueue)
            return
        }

        if shouldLog {
            NSLog("[DTermMetalView] render: vertexCount=%d, continuing with render", vertexCount)
        }

        // Update atlas texture if needed
        updateAtlasTexture(device: device, hybridRenderer: hybridRenderer)

        // Get uniforms
        guard let uniformsPtr = hybridRenderer.uniforms else {
            renderBlank(drawable: drawable, commandQueue: commandQueue)
            return
        }

        // Create uniforms buffer
        guard let uniformsBuffer = device.makeBuffer(
            bytes: uniformsPtr,
            length: 64,  // DtermUniforms is 64 bytes
            options: .storageModeShared
        ) else {
            renderBlank(drawable: drawable, commandQueue: commandQueue)
            return
        }

        // Create command buffer and encoder
        guard let commandBuffer = commandQueue.makeCommandBuffer() else {
            return
        }

        let renderPassDescriptor = MTLRenderPassDescriptor()
        renderPassDescriptor.colorAttachments[0].texture = drawable.texture
        renderPassDescriptor.colorAttachments[0].loadAction = .clear
        renderPassDescriptor.colorAttachments[0].storeAction = .store
        renderPassDescriptor.colorAttachments[0].clearColor = clearColor

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderPassDescriptor) else {
            return
        }

        // Select atlas texture
        let atlasTextureToUse: MTLTexture?
        if usingPlatformGlyphs, let platformAtlas = glyphAtlasManager?.atlasTexture {
            atlasTextureToUse = platformAtlas
        } else {
            atlasTextureToUse = atlasTexture
        }

        // Multi-pass rendering: backgrounds → glyphs → decorations
        // This eliminates GPU branch divergence and allows proper blend modes per pass
        let useMultiPass = backgroundPipelineState != nil &&
                          glyphPipelineState != nil &&
                          decorationPipelineState != nil

        if useMultiPass {
            // PASS 1: Backgrounds (no blending, no texture)
            if let bgVertices = hybridRenderer.backgroundVertices, bgVertices.count > 0,
               let bgPipeline = backgroundPipelineState,
               let bgBuffer = device.makeBuffer(bytes: bgVertices.pointer,
                                                length: bgVertices.count * 64,
                                                options: .storageModeShared) {
                encoder.setRenderPipelineState(bgPipeline)
                encoder.setVertexBuffer(bgBuffer, offset: 0, index: 0)
                encoder.setVertexBuffer(uniformsBuffer, offset: 0, index: 1)
                encoder.setFragmentBuffer(uniformsBuffer, offset: 0, index: 1)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: bgVertices.count)
            }

            // PASS 2: Glyphs (alpha blending, atlas texture)
            if let glyphVertices = hybridRenderer.glyphVertices, glyphVertices.count > 0,
               let glyphPipeline = glyphPipelineState,
               let glyphBuffer = device.makeBuffer(bytes: glyphVertices.pointer,
                                                   length: glyphVertices.count * 64,
                                                   options: .storageModeShared) {
                encoder.setRenderPipelineState(glyphPipeline)
                encoder.setVertexBuffer(glyphBuffer, offset: 0, index: 0)
                encoder.setVertexBuffer(uniformsBuffer, offset: 0, index: 1)
                encoder.setFragmentBuffer(uniformsBuffer, offset: 0, index: 1)
                if let atlas = atlasTextureToUse {
                    encoder.setFragmentTexture(atlas, index: 0)
                }
                encoder.setFragmentSamplerState(sampler, index: 0)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: glyphVertices.count)
            }

            // PASS 3: Decorations (alpha blending, solid color - on top of glyphs)
            let decoVerticesOpt = hybridRenderer.decorationVertices
            if shouldLog {
                NSLog("[DTermMetalView] PASS3: decorationVertices=%@, count=%d, decoPipeline=%d",
                      decoVerticesOpt != nil ? "non-nil" : "nil",
                      decoVerticesOpt?.count ?? 0,
                      decorationPipelineState != nil ? 1 : 0)
            }
            if let decoVertices = decoVerticesOpt, decoVertices.count > 0,
               let decoPipeline = decorationPipelineState,
               let decoBuffer = device.makeBuffer(bytes: decoVertices.pointer,
                                                  length: decoVertices.count * 64,
                                                  options: .storageModeShared) {
                if shouldLog {
                    NSLog("[DTermMetalView] PASS3: Drawing %d decoration vertices", decoVertices.count)
                }
                encoder.setRenderPipelineState(decoPipeline)
                encoder.setVertexBuffer(decoBuffer, offset: 0, index: 0)
                encoder.setVertexBuffer(uniformsBuffer, offset: 0, index: 1)
                encoder.setFragmentBuffer(uniformsBuffer, offset: 0, index: 1)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: decoVertices.count)
            }
        } else {
            // Fallback: Combined single-pass rendering (legacy)
            guard let vertexData = hybridRenderer.vertices,
                  let vertexBuffer = device.makeBuffer(bytes: vertexData.pointer,
                                                       length: vertexData.count * 64,
                                                       options: .storageModeShared) else {
                encoder.endEncoding()
                commandBuffer.present(drawable)
                commandBuffer.commit()
                return
            }

            encoder.setRenderPipelineState(pipelineState)
            encoder.setVertexBuffer(vertexBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformsBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformsBuffer, offset: 0, index: 1)
            if let atlas = atlasTextureToUse {
                encoder.setFragmentTexture(atlas, index: 0)
            }
            encoder.setFragmentSamplerState(sampler, index: 0)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: vertexData.count)
        }

        // Render images (Sixel, Kitty Graphics) after text
        renderImages(encoder: encoder, device: device, terminal: terminal, uniformsBuffer: uniformsBuffer)

        encoder.endEncoding()

        // Track GPU timing via command buffer completion
        let gpuStartTime = CACurrentMediaTime()
        commandBuffer.addCompletedHandler { [weak self] _ in
            let gpuEndTime = CACurrentMediaTime()
            let gpuTimeMs = (gpuEndTime - gpuStartTime) * 1000.0
            Task { @MainActor in
                self?.lastGpuFrameTimeMs = gpuTimeMs
            }
        }

        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    private func renderBlank(drawable: CAMetalDrawable, commandQueue: MTLCommandQueue) {
        guard let commandBuffer = commandQueue.makeCommandBuffer() else {
            return
        }

        let renderPassDescriptor = MTLRenderPassDescriptor()
        renderPassDescriptor.colorAttachments[0].texture = drawable.texture
        renderPassDescriptor.colorAttachments[0].loadAction = .clear
        renderPassDescriptor.colorAttachments[0].storeAction = .store
        renderPassDescriptor.colorAttachments[0].clearColor = clearColor

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderPassDescriptor) else {
            return
        }

        encoder.endEncoding()

        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    private func renderFallback() {
        guard let drawable = metalLayer?.nextDrawable() else { return }
        drawable.present()
    }

    // MARK: - Image Rendering

    /// Image vertex structure (32 bytes, matches Metal shader)
    private struct ImageVertex {
        var position: SIMD2<Float>  // 8 bytes
        var uv: SIMD2<Float>        // 8 bytes
        var tint: SIMD4<Float>      // 16 bytes
    }

    /// Render all terminal images (Sixel, Kitty Graphics).
    private func renderImages(
        encoder: MTLRenderCommandEncoder,
        device: MTLDevice,
        terminal: DTermCore,
        uniformsBuffer: MTLBuffer
    ) {
        guard let imagePipeline = imagePipelineState,
              let sampler = imageSampler else {
            return
        }

        // Process pending Sixel images
        processSixelImages(device: device, terminal: terminal)

        // Process Kitty graphics images
        processKittyImages(device: device, terminal: terminal)

        // Render all cached Kitty images with placements
        renderKittyImages(encoder: encoder, device: device, terminal: terminal, uniformsBuffer: uniformsBuffer, pipelineState: imagePipeline, sampler: sampler)
    }

    /// Process any pending Sixel images and upload to GPU.
    private func processSixelImages(device: MTLDevice, terminal: DTermCore) {
        guard terminal.hasSixelImage else { return }

        guard let sixelImage = terminal.getSixelImage() else { return }

        // Create texture from Sixel image
        let textureDescriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba8Unorm,
            width: Int(sixelImage.width),
            height: Int(sixelImage.height),
            mipmapped: false
        )
        textureDescriptor.usage = [.shaderRead]
        textureDescriptor.storageMode = .shared

        guard let texture = device.makeTexture(descriptor: textureDescriptor) else {
            DLog("DTermMetalView: Failed to create Sixel texture")
            return
        }

        // Upload pixel data
        sixelImage.withPixelData { pixels in
            let region = MTLRegion(
                origin: MTLOrigin(x: 0, y: 0, z: 0),
                size: MTLSize(width: Int(sixelImage.width), height: Int(sixelImage.height), depth: 1)
            )
            texture.replace(
                region: region,
                mipmapLevel: 0,
                withBytes: pixels,
                bytesPerRow: sixelImage.bytesPerRow
            )
        }

        // Store in cache at current cursor position
        // Note: Sixel images are placed at the cursor position when parsed
        // For now we use a simple position key; production would track image positions
        let cursorRow = terminal.cursorRow
        let cursorCol = terminal.cursorCol
        let positionKey = Int(cursorRow) * 10000 + Int(cursorCol)

        sixelTextureCache[positionKey] = texture

        // Evict old textures if cache is full
        evictOldTextures()

        DLog("DTermMetalView: Cached Sixel image \(sixelImage.width)x\(sixelImage.height) at (\(cursorRow), \(cursorCol))")
    }

    /// Process Kitty graphics images and upload to GPU.
    private func processKittyImages(device: MTLDevice, terminal: DTermCore) {
        guard terminal.hasKittyImages else { return }

        // Only update if dirty flag changed
        let isDirty = terminal.kittyIsDirty
        guard isDirty else { return }

        // Clear dirty flag
        terminal.clearKittyDirty()

        // Get all image IDs
        let imageIDs = terminal.kittyImageIDs()

        // Remove textures for deleted images
        let validIDs = Set(imageIDs)
        kittyTextureCache = kittyTextureCache.filter { validIDs.contains($0.key) }

        // Upload new or updated images
        for imageID in imageIDs {
            // Skip if already cached
            if kittyTextureCache[imageID] != nil {
                continue
            }

            guard let info = terminal.kittyImageInfo(id: imageID),
                  let pixels = terminal.kittyImagePixels(id: imageID) else {
                continue
            }

            // Create texture
            let textureDescriptor = MTLTextureDescriptor.texture2DDescriptor(
                pixelFormat: .rgba8Unorm,
                width: Int(info.width),
                height: Int(info.height),
                mipmapped: false
            )
            textureDescriptor.usage = [.shaderRead]
            textureDescriptor.storageMode = .shared

            guard let texture = device.makeTexture(descriptor: textureDescriptor) else {
                DLog("DTermMetalView: Failed to create Kitty texture for image \(imageID)")
                continue
            }

            // Upload pixel data
            pixels.withPixelData { pixelPtr in
                let region = MTLRegion(
                    origin: MTLOrigin(x: 0, y: 0, z: 0),
                    size: MTLSize(width: Int(info.width), height: Int(info.height), depth: 1)
                )
                texture.replace(
                    region: region,
                    mipmapLevel: 0,
                    withBytes: pixelPtr,
                    bytesPerRow: Int(info.width) * 4
                )
            }

            kittyTextureCache[imageID] = texture
            DLog("DTermMetalView: Cached Kitty image \(imageID) (\(info.width)x\(info.height))")
        }

        // Evict old textures if cache is full
        evictOldTextures()
    }

    /// Render all Kitty images with their placements.
    private func renderKittyImages(
        encoder: MTLRenderCommandEncoder,
        device: MTLDevice,
        terminal: DTermCore,
        uniformsBuffer: MTLBuffer,
        pipelineState: MTLRenderPipelineState,
        sampler: MTLSamplerState
    ) {
        guard terminal.hasKittyImages else { return }

        // Get cell dimensions for placement calculations
        let cellW = Float(cellSize.width)
        let cellH = Float(cellSize.height)
        guard cellW > 0 && cellH > 0 else { return }

        // Set image pipeline
        encoder.setRenderPipelineState(pipelineState)
        encoder.setVertexBuffer(uniformsBuffer, offset: 0, index: 1)
        encoder.setFragmentSamplerState(sampler, index: 0)

        // Render each image with its placements
        for imageID in terminal.kittyImageIDs() {
            guard let texture = kittyTextureCache[imageID],
                  let info = terminal.kittyImageInfo(id: imageID) else {
                continue
            }

            encoder.setFragmentTexture(texture, index: 0)

            // Get placements for this image
            let placementIDs = terminal.kittyPlacementIDs(imageID: imageID)

            for placementID in placementIDs {
                guard let placement = terminal.kittyPlacement(imageID: imageID, placementID: placementID) else {
                    continue
                }

                // Skip virtual placements for now (they require scrollback tracking)
                if placement.isVirtual {
                    continue
                }

                // Calculate position based on location type
                var row: Float = 0
                var col: Float = 0

                switch placement.location {
                case .absolute(let r, let c):
                    row = Float(r)
                    col = Float(c)
                case .virtual:
                    continue  // Skip virtual placements
                case .relative:
                    continue  // Skip relative placements for now
                }

                // Calculate pixel position
                let pixelX = col * cellW + Float(placement.cellXOffset)
                let pixelY = row * cellH + Float(placement.cellYOffset)

                // Calculate display dimensions
                let displayWidth: Float
                let displayHeight: Float

                if placement.numColumns > 0 && placement.numRows > 0 {
                    displayWidth = Float(placement.numColumns) * cellW
                    displayHeight = Float(placement.numRows) * cellH
                } else {
                    // Auto-size to image dimensions
                    displayWidth = Float(info.width)
                    displayHeight = Float(info.height)
                }

                // Calculate source UV coordinates
                let srcX = Float(placement.sourceX) / Float(info.width)
                let srcY = Float(placement.sourceY) / Float(info.height)
                let srcW = placement.sourceWidth > 0
                    ? Float(placement.sourceWidth) / Float(info.width)
                    : 1.0 - srcX
                let srcH = placement.sourceHeight > 0
                    ? Float(placement.sourceHeight) / Float(info.height)
                    : 1.0 - srcY

                // Build quad vertices (2 triangles, 6 vertices)
                let vertices = buildImageQuad(
                    x: pixelX,
                    y: pixelY,
                    width: displayWidth,
                    height: displayHeight,
                    uvX: srcX,
                    uvY: srcY,
                    uvW: srcW,
                    uvH: srcH
                )

                // Create vertex buffer
                guard let vertexBuffer = device.makeBuffer(
                    bytes: vertices,
                    length: vertices.count * MemoryLayout<ImageVertex>.stride,
                    options: .storageModeShared
                ) else {
                    continue
                }

                encoder.setVertexBuffer(vertexBuffer, offset: 0, index: 0)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
            }
        }
    }

    /// Build a quad (2 triangles, 6 vertices) for image rendering.
    private func buildImageQuad(
        x: Float,
        y: Float,
        width: Float,
        height: Float,
        uvX: Float,
        uvY: Float,
        uvW: Float,
        uvH: Float
    ) -> [ImageVertex] {
        let tint = SIMD4<Float>(1.0, 1.0, 1.0, 1.0)  // No tint

        // Triangle 1: top-left, top-right, bottom-left
        // Triangle 2: top-right, bottom-right, bottom-left
        return [
            // Triangle 1
            ImageVertex(position: SIMD2(x, y), uv: SIMD2(uvX, uvY), tint: tint),
            ImageVertex(position: SIMD2(x + width, y), uv: SIMD2(uvX + uvW, uvY), tint: tint),
            ImageVertex(position: SIMD2(x, y + height), uv: SIMD2(uvX, uvY + uvH), tint: tint),
            // Triangle 2
            ImageVertex(position: SIMD2(x + width, y), uv: SIMD2(uvX + uvW, uvY), tint: tint),
            ImageVertex(position: SIMD2(x + width, y + height), uv: SIMD2(uvX + uvW, uvY + uvH), tint: tint),
            ImageVertex(position: SIMD2(x, y + height), uv: SIMD2(uvX, uvY + uvH), tint: tint),
        ]
    }

    /// Evict old textures if cache exceeds maximum size.
    private func evictOldTextures() {
        let totalCached = sixelTextureCache.count + kittyTextureCache.count

        if totalCached > maxCachedImages {
            // Evict oldest Sixel textures first (they're position-based, less useful)
            let toEvict = totalCached - maxCachedImages

            if sixelTextureCache.count > 0 {
                let evictCount = min(toEvict, sixelTextureCache.count)
                let keysToRemove = Array(sixelTextureCache.keys.prefix(evictCount))
                for key in keysToRemove {
                    sixelTextureCache.removeValue(forKey: key)
                }
                DLog("DTermMetalView: Evicted \(evictCount) Sixel textures")
            }
        }
    }

    /// Clear all cached image textures.
    @objc public func clearImageCache() {
        sixelTextureCache.removeAll()
        kittyTextureCache.removeAll()
        DLog("DTermMetalView: Cleared image cache")
    }

    // MARK: - Atlas Texture Management

    private func updateAtlasTexture(device: MTLDevice, hybridRenderer: DTermHybridRenderer) {
        // When using platform glyphs, the atlas is managed by DTermGlyphAtlasManager
        if usingPlatformGlyphs {
            // Platform glyphs don't use the fontdue atlas
            // The DTermGlyphAtlasManager handles texture updates internally
            return
        }

        let atlasSize = hybridRenderer.atlasSize

        // Recreate texture if size changed
        let needsFullUpload = atlasSize != currentAtlasSize || atlasTexture == nil
        if needsFullUpload {
            createAtlasTexture(device: device, size: atlasSize)
            currentAtlasSize = atlasSize

            // Upload full atlas when recreating texture
            if let texture = atlasTexture, let atlasData = hybridRenderer.atlasData() {
                let region = MTLRegion(
                    origin: MTLOrigin(x: 0, y: 0, z: 0),
                    size: MTLSize(width: Int(atlasData.width), height: Int(atlasData.height), depth: 1)
                )
                texture.replace(
                    region: region,
                    mipmapLevel: 0,
                    withBytes: atlasData.data,
                    bytesPerRow: Int(atlasData.width)  // R8 format = 1 byte per pixel
                )
            }

            // Clear pending glyphs since we uploaded everything
            hybridRenderer.clearPendingGlyphs()
            return
        }

        // Upload only pending glyphs (incremental update)
        guard let texture = atlasTexture else { return }

        for glyph in hybridRenderer.pendingGlyphs {
            let region = MTLRegion(
                origin: MTLOrigin(x: Int(glyph.x), y: Int(glyph.y), z: 0),
                size: MTLSize(width: Int(glyph.width), height: Int(glyph.height), depth: 1)
            )

            texture.replace(
                region: region,
                mipmapLevel: 0,
                withBytes: glyph.data,
                bytesPerRow: Int(glyph.width)  // R8 format = 1 byte per pixel
            )
        }

        hybridRenderer.clearPendingGlyphs()
    }

    private func createAtlasTexture(device: MTLDevice, size: UInt32) {
        guard size > 0 else { return }

        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .r8Unorm,  // Single channel for glyph alpha
            width: Int(size),
            height: Int(size),
            mipmapped: false
        )
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .shared

        atlasTexture = device.makeTexture(descriptor: descriptor)

        if atlasTexture == nil {
            DLog("DTermMetalView: Failed to create atlas texture (\(size)x\(size))")
        }
    }

    // MARK: - Size Management

    private func updateDrawableSize() {
        guard let metalLayer else { return }

        let scale = window?.backingScaleFactor ?? 2.0
        let newSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )

        if newSize != metalLayer.drawableSize {
            metalLayer.drawableSize = newSize
            delegate?.dtermMetalView(self, drawableSizeWillChange: newSize)
        }
    }

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateDrawableSize()
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        metalLayer?.contentsScale = window?.backingScaleFactor ?? 2.0
        updateDrawableSize()
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        metalLayer?.contentsScale = window?.backingScaleFactor ?? 2.0
        updateDrawableSize()
    }

    // MARK: - Terminal Management

    /// Set the terminal to render.
    public func setTerminal(_ terminal: DTermCore?) {
        self.terminal = terminal
    }

    /// Set the DTermCoreIntegration to render.
    /// This is the preferred method for ObjC callers.
    @objc public func setIntegration(_ integration: DTermCoreIntegration?) {
        if let integration = integration {
            self.terminal = integration.terminalForRendering
            DLog("DTermMetalView.setIntegration: connected to terminal \(integration.rows)x\(integration.cols)")
        } else {
            self.terminal = nil
            DLog("DTermMetalView.setIntegration: cleared terminal")
        }
    }

    /// Set the font for the hybrid renderer.
    ///
    /// Uses the platform glyph atlas (Core Text) for fonts without accessible file URLs.
    /// This enables support for system fonts like Monaco, Menlo, and SF Mono.
    ///
    /// - Parameter font: NSFont to use for glyph rendering
    /// - Returns: `true` on success
    @objc @discardableResult
    public func setFont(_ font: NSFont) -> Bool {
        guard let renderer = hybridRenderer else { return false }

        // Check if font has an accessible file URL (not a system font)
        let ctFont = font as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let hasFileURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL != nil

        // Use fontdue for fonts with file URLs, platform glyphs for system fonts
        if hasFileURL {
            if let fontData = DTermHybridRenderer.extractFontData(from: font),
               renderer.setFont(fontData: fontData) {
                DLog("DTermMetalView: Using fontdue atlas for \(font.fontName)")
                usingPlatformGlyphs = false
                return true
            }
            // fontdue failed even with file URL - try platform glyphs
            DLog("DTermMetalView: fontdue failed for \(font.fontName), trying platform glyphs")
        }

        // System font or fontdue failed - use platform glyph atlas (Core Text)
        return usePlatformGlyphAtlas(font: font)
    }

    /// Set up the platform glyph atlas for fonts without file URLs.
    ///
    /// This uses Core Text to render glyphs, which works with any macOS font.
    private func usePlatformGlyphAtlas(font: NSFont) -> Bool {
        guard let device = metalDevice, let renderer = hybridRenderer else {
            DLog("DTermMetalView: Cannot create glyph atlas - no Metal device or renderer")
            return false
        }

        // Create or reset the glyph atlas manager
        if glyphAtlasManager == nil {
            glyphAtlasManager = DTermGlyphAtlasManager(device: device)
        }

        guard let atlas = glyphAtlasManager else {
            DLog("DTermMetalView: Failed to create glyph atlas manager")
            return false
        }

        // Configure the atlas with the font
        guard atlas.setFont(font) else {
            DLog("DTermMetalView: Failed to set font on glyph atlas manager")
            return false
        }

        // Sync glyph entries to Rust
        guard renderer.syncWithAtlasManager(atlas) else {
            DLog("DTermMetalView: Failed to sync glyph atlas with renderer")
            return false
        }

        usingPlatformGlyphs = true
        DLog("DTermMetalView: Using platform glyph atlas (Core Text) for \(font.fontName)")
        DLog("  Cell size: \(atlas.cellWidth) x \(atlas.cellHeight)")
        DLog("  Atlas size: \(atlas.atlasSize)")
        DLog("  Pre-rendered glyphs: \(renderer.platformGlyphCount)")

        return true
    }

    /// Get the cell dimensions from the font.
    ///
    /// - Returns: Cell size from the hybrid renderer's font, or .zero if not configured
    @objc public var cellSize: CGSize {
        // Prefer platform glyph atlas dimensions when using platform glyphs
        if usingPlatformGlyphs, let atlas = glyphAtlasManager {
            return CGSize(width: atlas.cellWidth, height: atlas.cellHeight)
        }
        if let dims = hybridRenderer?.cellDimensions() {
            return CGSize(width: CGFloat(dims.width), height: CGFloat(dims.height))
        }
        return .zero
    }

    // MARK: - Layer Configuration

    public override var wantsUpdateLayer: Bool {
        return true
    }

    public override func makeBackingLayer() -> CALayer {
        return metalLayer ?? super.makeBackingLayer()
    }

    // MARK: - Performance Methods

    /// Reset all performance counters.
    @objc public func resetPerformanceCounters() {
        frameTimestamps.removeAll()
        totalFramesRendered = 0
        renderStartTime = 0
        lastGpuFrameTimeMs = 0
    }

    /// Log current performance metrics (for debugging).
    @objc public func logPerformance() {
        DLog("DTermMetalView: \(performanceSummary)")
    }
}

// MARK: - Availability Check

extension DTermMetalView {
    /// Check if DTermMetalView is available (requires hybrid renderer).
    @objc public static var isAvailable: Bool {
        return DTermHybridRenderer.isAvailable
    }
}
