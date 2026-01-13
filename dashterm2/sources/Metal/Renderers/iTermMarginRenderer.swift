import Metal
import simd
import AppKit

class iTermMarginRendererTransientState: iTermMetalCellRendererTransientState {
    @objc var regularVerticalColor = vector_float4(0, 0, 0, 0)
    @objc var regularHorizontalColor = vector_float4(0, 0, 0, 0)
    struct ColorExtension {
        var color: vector_float4
        var left: Bool
        var row: Int
    }
    var extensions = [ColorExtension]()
    @objc(addExtensionWithColor:left:row:)
    func addExtension(color: vector_float4, left: Bool, row: Int) {
        extensions.append(.init(color: color, left: left, row: row))
    }
}

final class iTermMarginRenderer: NSObject, iTermMetalCellRendererProtocol {
    private let blendingRenderer: iTermMetalCellRenderer
    private let nonblendingRenderer: iTermMetalCellRenderer?
    private let compositeOverRenderer: iTermMetalCellRenderer?

    private let blendingRendererForPIUs: iTermMetalCellRenderer
    private let nonblendingRendererForPIUs: iTermMetalCellRenderer?
    private let compositeOverRendererForPIUs: iTermMetalCellRenderer?

    private let colorPool: iTermMetalBufferPool
    private let fourQuadVerticesPool: iTermMetalBufferPool
    private let twoQuadVerticesPool: iTermMetalBufferPool
    private let oneQuadVerticesPool: iTermMetalBufferPool
    private let piuPool: iTermMetalMixedSizeBufferPool

    // BUG-f547: Make initializer failable to handle Metal setup failures gracefully
    @objc(initWithDevice:)
    init?(device: MTLDevice) {
        if iTermTextIsMonochrome() {
            nonblendingRenderer = iTermMetalCellRenderer(
                device: device,
                vertexFunctionName: "iTermMarginVertexShader",
                fragmentFunctionName: "iTermMarginFragmentShader",
                blending: nil,
                piuElementSize: 0,
                transientStateClass: iTermMarginRendererTransientState.self)
            compositeOverRenderer = iTermMetalCellRenderer(
                device: device,
                vertexFunctionName: "iTermMarginVertexShader",
                fragmentFunctionName: "iTermMarginFragmentShader",
                blending: iTermMetalBlending.premultipliedCompositing(),
                piuElementSize: 0,
                transientStateClass: iTermMarginRendererTransientState.self)

            nonblendingRendererForPIUs = iTermMetalCellRenderer(
                device: device,
                vertexFunctionName: "iTermMarginPIUVertexShader",
                fragmentFunctionName: "iTermMarginPIUFragmentShader",
                blending: nil,
                piuElementSize: 0,
                transientStateClass: iTermMarginRendererTransientState.self)
            compositeOverRendererForPIUs = iTermMetalCellRenderer(
                device: device,
                vertexFunctionName: "iTermMarginPIUVertexShader",
                fragmentFunctionName: "iTermMarginPIUFragmentShader",
                blending: iTermMetalBlending.premultipliedCompositing(),
                piuElementSize: 0,
                transientStateClass: iTermMarginRendererTransientState.self)
        } else {
            nonblendingRenderer = nil
            compositeOverRenderer = nil
            nonblendingRendererForPIUs = nil
            compositeOverRendererForPIUs = nil
        }

        // BUG-f547: Return nil for Metal setup failures instead of crashing
        guard let blendingCellRenderer = iTermMetalCellRenderer(
            device: device,
            vertexFunctionName: "iTermMarginVertexShader",
            fragmentFunctionName: "iTermMarginFragmentShader",
            blending: iTermMetalBlending(),
            piuElementSize: 0,
            transientStateClass: iTermMarginRendererTransientState.self) else {
            DLog("ERROR: Failed to create iTermMarginRenderer blending renderer")
            return nil
        }
        blendingRenderer = blendingCellRenderer

        // BUG-f547: Return nil for Metal setup failures instead of crashing
        guard let blendingCellRendererForPIUs = iTermMetalCellRenderer(
            device: device,
            vertexFunctionName: "iTermMarginPIUVertexShader",
            fragmentFunctionName: "iTermMarginPIUFragmentShader",
            blending: iTermMetalBlending(),
            piuElementSize: 0,
            transientStateClass: iTermMarginRendererTransientState.self) else {
            DLog("ERROR: Failed to create iTermMarginRenderer blending renderer for PIUs")
            return nil
        }
        blendingRendererForPIUs = blendingCellRendererForPIUs

        colorPool = iTermMetalBufferPool(device: device, bufferSize: MemoryLayout<vector_float4>.stride)
        fourQuadVerticesPool = iTermMetalBufferPool(device: device, bufferSize: MemoryLayout<vector_float2>.stride * 6 * 4)
        twoQuadVerticesPool = iTermMetalBufferPool(device: device, bufferSize: MemoryLayout<vector_float2>.stride * 6 * 2)
        oneQuadVerticesPool = iTermMetalBufferPool(device: device, bufferSize: MemoryLayout<vector_float2>.stride * 6 * 1)
        piuPool = iTermMetalMixedSizeBufferPool(device: device, capacity: 512, name:"Margin PIU")

        super.init()
    }

    func createTransientStateStat() -> iTermMetalFrameDataStat {
        return .pqCreateMarginTS
    }

    func renderer(for configuration: iTermCellRenderConfiguration, forPIUs: Bool) -> iTermMetalCellRenderer {
        // BUG-1782: Use nil coalescing with fallback instead of force unwrap for optional renderers
        if forPIUs {
            if iTermTextIsMonochrome() {
                if configuration.hasBackgroundImage {
                    return compositeOverRendererForPIUs ?? blendingRendererForPIUs
                } else {
                    return nonblendingRendererForPIUs ?? blendingRendererForPIUs
                }
            }
            return blendingRendererForPIUs
        } else {
            if iTermTextIsMonochrome() {
                if configuration.hasBackgroundImage {
                    return compositeOverRenderer ?? blendingRenderer
                } else {
                    return nonblendingRenderer ?? blendingRenderer
                }
            }
            return blendingRenderer
        }
    }

    func draw(with frameData: iTermMetalFrameData,
              transientState genericState: iTermMetalCellRendererTransientState) {
        // BUG-1585: Use as? to avoid crash on type mismatch
        guard let transientState = genericState as? iTermMarginRendererTransientState else {
            return
        }
        if transientState.extensions.isEmpty {
            drawNoExtensions(with: frameData, transientState: transientState, draw: .leftAndRight)
            drawNoExtensions(with: frameData, transientState: transientState, draw: .topAndBottom)
        } else {
            drawWithExtensions(with: frameData, transientState: transientState)
        }
    }

    private func drawWithExtensions(with frameData: iTermMetalFrameData,
                                    transientState: iTermMarginRendererTransientState) {
        guard let renderEncoder = frameData.renderEncoder else {
            return
        }
        drawNoExtensions(with: frameData, transientState: transientState, draw: .topAndBottom)
        if transientState.configuration.viewportSize.x > transientState.configuration.viewportSizeExcludingLegacyScrollbars.x {
            drawNoExtensions(with: frameData, transientState: transientState, draw: .underLegacyScrollbar)
        }

        let cellRenderer = renderer(for: transientState.cellConfiguration, forPIUs: true)
        let size = CGSize(
            width: CGFloat(transientState.configuration.viewportSize.x),
            height: CGFloat(transientState.configuration.viewportSize.y))
        let margins = transientState.margins
        let innerHeight = size.height - margins.bottom - margins.top

        let (leftVertexBuffer, rightVertextBuffer) = initializePIUVertexBuffers(transientState: transientState)
        // BUG-f587: Handle optional return from makePIUs - skip drawing if nil
        if let (leftPIUs, count) = makePIUs(from: CGRect(x: 0,
                                                          y: margins.top,
                                                          width: margins.left,
                                                          height: innerHeight),
                                             bottomMargin: transientState.margins.top,
                                             extensions: transientState.extensions.filter { $0.left },
                                             lineHeight: Float(transientState.cellConfiguration.cellSize.height),
                                             defaultColor: transientState.regularVerticalColor,
                                             context: transientState.poolContext) {
            transientState.pipelineState = cellRenderer.pipelineState()
            withUnsafeTemporaryAllocation(of: iTermMetalBufferBinding.self, capacity: 2) { vertexBindings in
                vertexBindings[0] = iTermMetalBufferBindingMake(
                    UInt(iTermVertexInputIndexPerInstanceUniforms.rawValue),
                    leftPIUs)
                vertexBindings[1] = iTermMetalBufferBindingMake(
                    UInt(iTermVertexInputIndexVertices.rawValue),
                    leftVertexBuffer)
                cellRenderer.draw(
                    with: transientState,
                    renderEncoder: renderEncoder,
                    numberOfVertices: 6,
                    numberOfPIUs: count,
                    vertexBindings: vertexBindings.baseAddress,
                    vertexBindingCount: UInt(vertexBindings.count),
                    fragmentBindings: nil,
                    fragmentBindingCount: 0,
                    textureBindings: nil,
                    textureBindingCount: 0)
            }
        }

        // BUG-f587: Handle optional return from makePIUs - skip drawing if nil
        let gridWidth = CGFloat(transientState.cellConfiguration.gridSize.width) * CGFloat(transientState.cellConfiguration.cellSize.width)
        let rightGutterWidth = CGFloat(transientState.configuration.viewportSize.x) - CGFloat(margins.left - margins.right - gridWidth)
        if let (rightPIUs, count) = makePIUs(from: CGRect(x: size.width - margins.right - rightGutterWidth,
                                                           y: margins.top,
                                                           width: margins.right + rightGutterWidth,
                                                           height: innerHeight),
                                              bottomMargin: transientState.margins.top,
                                              extensions: transientState.extensions.filter { !$0.left },
                                              lineHeight: Float(transientState.cellConfiguration.cellSize.height),
                                              defaultColor: transientState.regularVerticalColor,
                                              context: transientState.poolContext) {
            transientState.pipelineState = cellRenderer.pipelineState()
            withUnsafeTemporaryAllocation(of: iTermMetalBufferBinding.self, capacity: 2) { vertexBindings in
                vertexBindings[0] = iTermMetalBufferBindingMake(
                    UInt(iTermVertexInputIndexPerInstanceUniforms.rawValue),
                    rightPIUs)
                vertexBindings[1] = iTermMetalBufferBindingMake(
                    UInt(iTermVertexInputIndexVertices.rawValue),
                    rightVertextBuffer)
                cellRenderer.draw(
                    with: transientState,
                    renderEncoder: renderEncoder,
                    numberOfVertices: 6,
                    numberOfPIUs: count,
                    vertexBindings: vertexBindings.baseAddress,
                    vertexBindingCount: UInt(vertexBindings.count),
                    fragmentBindings: nil,
                    fragmentBindingCount: 0,
                    textureBindings: nil,
                    textureBindingCount: 0)
            }
        }
    }

    // BUG-f587: Return optional to handle empty arrays and overflow gracefully instead of crashing
    private func makePIUs(from container: NSRect,
                          bottomMargin: CGFloat,
                          extensions: [iTermMarginRendererTransientState.ColorExtension],
                          lineHeight: Float,
                          defaultColor: vector_float4,
                          context: iTermMetalBufferPoolContext) -> (MTLBuffer, Int)? {
        let sorted = extensions.sorted { $0.row < $1.row }
        var result = Array<iTermMarginExtensionPIU>()
        for e in sorted {
            var color = e.color
            if iTermTextIsMonochrome() {
                color.x *= color.w
                color.y *= color.w
                color.z *= color.w
            }
            result.append(.init(
                color: color,
                defaultBackgroundColor: defaultColor,
                yOffset: Float(container.height) - Float(e.row + 1) * lineHeight + Float(bottomMargin)))
        }
        // BUG-f587: Handle empty array case - withUnsafeBytes returns nil baseAddress for empty arrays
        guard !result.isEmpty else {
            DLog("makePIUs: empty extensions array, returning nil")
            return nil
        }
        let buffer = result.withUnsafeBytes { urbp in
            guard let pius = urbp.baseAddress else {
                // BUG-f587: Should not happen for non-empty arrays, but log and return nil instead of crashing
                DLog("ERROR: makePIUs: withUnsafeBytes returned nil baseAddress for non-empty array")
                return nil as MTLBuffer?
            }
            // BUG-f588: Check for overflow in buffer size calculation
            let stride = MemoryLayout<iTermMarginExtensionPIU>.stride
            let (bufferSize, overflow) = result.count.multipliedReportingOverflow(by: stride)
            if overflow {
                // BUG-f588: Return nil instead of crashing on overflow
                DLog("ERROR: makePIUs: Buffer size overflow count=\(result.count) stride=\(stride)")
                return nil as MTLBuffer?
            }
            return piuPool.requestBuffer(from: context,
                                         size: bufferSize,
                                         bytes: pius)
        }
        guard let buffer else {
            return nil
        }
        return (buffer, result.count)
    }

    private func drawNoExtensions(with frameData: iTermMetalFrameData,
                                  transientState: iTermMarginRendererTransientState,
                                  draw: Draw) {
        guard let renderEncoder = frameData.renderEncoder else {
            return
        }
        initializeRegularVertexBuffer(tState: transientState, draw: draw)
        var color = transientState.regularHorizontalColor
        if iTermTextIsMonochrome() {
            color.x *= color.w
            color.y *= color.w
            color.z *= color.w
        }
        let colorBuffer = colorPool.requestBuffer(
            from: transientState.poolContext,
            withBytes: &color,
            checkIfChanged: true)
        let cellRenderer = renderer(for: transientState.cellConfiguration, forPIUs: false)
        transientState.pipelineState = cellRenderer.pipelineState()
        withUnsafeTemporaryAllocation(of: iTermMetalBufferBinding.self, capacity: 1) { vertexBindings in
            vertexBindings[0] = iTermMetalBufferBindingMake(
                UInt(iTermVertexInputIndexVertices.rawValue),
                transientState.vertexBuffer)
            withUnsafeTemporaryAllocation(of: iTermMetalBufferBinding.self, capacity: 1) { fragmentBindings in
                fragmentBindings[0] = iTermMetalBufferBindingMake(
                    UInt(iTermFragmentBufferIndexMarginColor.rawValue),
                    colorBuffer)
                cellRenderer.draw(
                    with: transientState,
                    renderEncoder: renderEncoder,
                    numberOfVertices: 6 * 4,
                    numberOfPIUs: 0,
                    vertexBindings: vertexBindings.baseAddress,
                    vertexBindingCount: UInt(vertexBindings.count),
                    fragmentBindings: fragmentBindings.baseAddress,
                    fragmentBindingCount: UInt(fragmentBindings.count),
                    textureBindings: nil,
                    textureBindingCount: 0)
            }
        }
    }

    var rendererDisabled: Bool {
        return false
    }

    func createTransientState(forCellConfiguration configuration: iTermCellRenderConfiguration,
                              commandBuffer: MTLCommandBuffer) -> iTermMetalRendererTransientState? {
        // BUG-1783: Remove force unwrap - return nil if transient state creation fails
        let renderer = renderer(for: configuration, forPIUs: false)
        return renderer.createTransientState(
            forCellConfiguration: configuration,
            commandBuffer: commandBuffer)
    }

    @discardableResult
    private func appendVerticesForQuad(
        _ quad: CGRect,
        to v: UnsafeMutablePointer<vector_float2>) -> UnsafeMutablePointer<vector_float2> {
        v.pointee = simd_make_float2(Float(quad.maxX), Float(quad.minY))
        let v1 = v.advanced(by: 1)
        v1.pointee = simd_make_float2(Float(quad.minX), Float(quad.minY))
        let v2 = v.advanced(by: 2)
        v2.pointee = simd_make_float2(Float(quad.minX), Float(quad.maxY))
        let v3 = v.advanced(by: 3)
        v3.pointee = simd_make_float2(Float(quad.maxX), Float(quad.minY))
        let v4 = v.advanced(by: 4)
        v4.pointee = simd_make_float2(Float(quad.minX), Float(quad.maxY))
        let v5 = v.advanced(by: 5)
        v5.pointee = simd_make_float2(Float(quad.maxX), Float(quad.maxY))
        return v.advanced(by: 6)
    }

    private func initializePIUVertexBuffers(transientState tState: iTermMarginRendererTransientState) -> (MTLBuffer, MTLBuffer) {
        let margins = tState.margins
        var leftVertices = [vector_float2](repeating: .zero, count: 6)
        leftVertices.withUnsafeMutableBufferPointer { buf in
            guard var v = buf.baseAddress else { return }
            v = appendVerticesForQuad(
                CGRect(x: 0,
                       y: 0,
                       width: margins.left,
                       height: tState.cellConfiguration.cellSize.height),
                to: v)
        }
        var rightVertices = [vector_float2](repeating: .zero, count: 6)
        rightVertices.withUnsafeMutableBufferPointer { buf in
            guard var v = buf.baseAddress else { return }
            let gridWidth = CGFloat(tState.cellConfiguration.gridSize.width) * CGFloat(tState.cellConfiguration.cellSize.width)
            let rightGutterWidth = CGFloat(tState.configuration.viewportSizeExcludingLegacyScrollbars.x) - margins.left - margins.right - gridWidth
            let width = margins.right + rightGutterWidth
            v = appendVerticesForQuad(
                CGRect(x: CGFloat(tState.configuration.viewportSizeExcludingLegacyScrollbars.x) - width,
                       y: 0,
                       width: width,
                       height: tState.cellConfiguration.cellSize.height),
                to: v)
        }
        return (
            oneQuadVerticesPool.requestBuffer(
                from: tState.poolContext,
                withBytes: leftVertices,
                checkIfChanged: true),
            oneQuadVerticesPool.requestBuffer(
                from: tState.poolContext,
                withBytes: rightVertices,
                checkIfChanged: true)
        )
    }

    private enum Draw {
        case topAndBottom
        case leftAndRight
        case underLegacyScrollbar
    }
    private func initializeRegularVertexBuffer(tState: iTermMarginRendererTransientState,
                                               draw: Draw) {
        let size = CGSize(
            width: CGFloat(tState.configuration.viewportSize.x),
            height: CGFloat(tState.configuration.viewportSize.y))
        let margins = tState.margins
        var vertices = [vector_float2](repeating: .zero, count: 6 * 2)
        vertices.withUnsafeMutableBufferPointer { buf in
            guard var v = buf.baseAddress else { return }
            switch draw {
            case .topAndBottom:
                // Top
                v = appendVerticesForQuad(
                    CGRect(x: 0,
                           y: 0,
                           width: size.width,
                           height: margins.top),
                    to: v)
                // Bottom
                v = appendVerticesForQuad(
                    CGRect(x: 0,
                           y: size.height - margins.bottom,
                           width: size.width,
                           height: margins.bottom),
                    to: v)
            case .leftAndRight:
                let innerHeight = size.height - margins.bottom - margins.top
                // Left
                v = appendVerticesForQuad(
                    CGRect(x: 0,
                           y: margins.top,
                           width: margins.left,
                           height: innerHeight),
                    to: v)
                // Right
                let gridWidth = CGFloat(tState.cellConfiguration.gridSize.width) * CGFloat(tState.cellConfiguration.cellSize.width)
                let rightGutterWidth = CGFloat(tState.configuration.viewportSize.x) - margins.left - margins.right - gridWidth
                v = appendVerticesForQuad(
                    CGRect(x: size.width - margins.right - rightGutterWidth,
                           y: margins.top,
                           width: margins.right + rightGutterWidth,
                           height: innerHeight),
                    to: v)
            case .underLegacyScrollbar:
                let innerHeight = size.height - margins.bottom - margins.top
                v = appendVerticesForQuad(
                    CGRect(x: CGFloat(tState.configuration.viewportSizeExcludingLegacyScrollbars.x),
                           y: margins.top,
                           width: CGFloat(tState.configuration.viewportSize.x - tState.configuration.viewportSizeExcludingLegacyScrollbars.x),
                           height: innerHeight),
                    to: v)
            }
        }
        tState.vertexBuffer = twoQuadVerticesPool.requestBuffer(
            from: tState.poolContext,
            withBytes: vertices,
            checkIfChanged: true)
    }

    @discardableResult
    func initializeVertexBuffer(
        _ tState: iTermMarginRendererTransientState) -> Int {
        let size = CGSize(
            width: CGFloat(tState.configuration.viewportSize.x),
            height: CGFloat(tState.configuration.viewportSize.y))
        let margins = tState.margins
        var vertices = [vector_float2](repeating: .zero, count: 6 * 8)
        var count = 0
        vertices.withUnsafeMutableBufferPointer { buf in
            guard var v = buf.baseAddress else { return }
            // Top
            v = appendVerticesForQuad(
                CGRect(x: 0,
                       y: size.height - margins.bottom,
                       width: size.width,
                       height: margins.bottom),
                to: v)
            count += 1
            // Bottom
            v = appendVerticesForQuad(
                CGRect(x: 0,
                       y: 0,
                       width: size.width,
                       height: margins.top),
                to: v)
            count += 1
            let gridWidth = CGFloat(tState.cellConfiguration.gridSize.width) * CGFloat(tState.cellConfiguration.cellSize.width)
            let rightGutterWidth = CGFloat(tState.configuration.viewportSize.x) - margins.left - margins.right - gridWidth
            let y = margins.top
            let h = CGFloat(VT100GridRangeMake(0, max(0, tState.cellConfiguration.gridSize.height)).length)
                * tState.cellConfiguration.cellSize.height
            v = appendVerticesForQuad(
                CGRect(x: 0,
                       y: y,
                       width: margins.left,
                       height: h),
                to: v)
            v = appendVerticesForQuad(
                CGRect(x: size.width - margins.right - rightGutterWidth,
                       y: y,
                       width: margins.right + rightGutterWidth,
                       height: h),
                to: v)
            count += 2
        }
        tState.vertexBuffer = fourQuadVerticesPool.requestBuffer(
            from: tState.poolContext,
            withBytes: vertices,
            checkIfChanged: true)
        return count
    }
}
