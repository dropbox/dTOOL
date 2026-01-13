//
//  GraphRenderer.swift
//  DashTerm
//
//  Metal-based renderer for computation graph visualization
//

import Foundation
import Metal
import MetalKit
import simd

/// Metal renderer for the computation graph
class GraphRenderer: NSObject {
    // Metal objects
    private let device: MTLDevice
    private let commandQueue: MTLCommandQueue
    private var pipelineState: MTLRenderPipelineState?
    private var edgePipelineState: MTLRenderPipelineState?
    private var gridPipelineState: MTLRenderPipelineState?

    // Buffers
    private var uniformBuffer: MTLBuffer?
    private var nodeVertexBuffer: MTLBuffer?
    private var edgeVertexBuffer: MTLBuffer?
    private var glowVertexBuffer: MTLBuffer?

    // Text rendering
    private var textPipelineState: MTLRenderPipelineState?
    private var fontAtlas: FontAtlas?
    private var textVertexBuffer: MTLBuffer?
    private var textVertexCount: Int = 0
    private var samplerState: MTLSamplerState?

    // Uniforms
    private var uniforms = Uniforms()
    private var startTime: CFTimeInterval = CACurrentMediaTime()

    // Graph data
    private var nodes: [RenderNode] = []
    private var edges: [RenderEdge] = []
    private var nodeLabels: [String] = []

    // View transform
    private var zoom: Float = 1.0
    private var pan: simd_float2 = simd_float2(0, 0)

    // Selection
    private var selectedNodeIndex: Int? = nil
    private var selectedGroupIndex: Int? = nil
    private var glowPipelineState: MTLRenderPipelineState?

    // Running nodes animation
    private var runningPipelineState: MTLRenderPipelineState?
    private var runningNodeIndices: [Int] = []
    private var runningVertexBuffer: MTLBuffer?

    // Arrow heads
    private var arrowPipelineState: MTLRenderPipelineState?
    private var arrowVertexBuffer: MTLBuffer?
    private var arrowVertexCount: Int = 0

    // Active edges (connected to running nodes)
    private var activeEdgePipelineState: MTLRenderPipelineState?
    private var activeEdgeVertexBuffer: MTLBuffer?
    private var activeEdgeCount: Int = 0
    private var inactiveEdgeCount: Int = 0

    // Groups (collapsible node containers)
    private var groupPipelineState: MTLRenderPipelineState?
    private var groupVertexBuffer: MTLBuffer?
    private var groupLabels: [String] = []
    private var groups: [RenderGroup] = []
    private var groupTextVertexBuffer: MTLBuffer?
    private var groupTextVertexCount: Int = 0
    private let collapsedGroupRenderSize = simd_float2(100, 50)
    private let nodeArrowEndInset: Float = 40.0

    init?(device: MTLDevice) {
        self.device = device

        guard let queue = device.makeCommandQueue() else {
            return nil
        }
        self.commandQueue = queue

        super.init()

        setupPipelines()
        setupBuffers()
        setupTextRendering()
    }

    private func setupPipelines() {
        guard let library = device.makeDefaultLibrary() else {
            print("Failed to create Metal library")
            return
        }

        // Node pipeline
        let nodeDescriptor = MTLRenderPipelineDescriptor()
        nodeDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        nodeDescriptor.fragmentFunction = library.makeFunction(name: "nodeFragmentShader")
        nodeDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        nodeDescriptor.colorAttachments[0].isBlendingEnabled = true
        nodeDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        nodeDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        nodeDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        nodeDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha

        // Vertex descriptor for nodes
        let vertexDescriptor = MTLVertexDescriptor()
        vertexDescriptor.attributes[0].format = .float2
        vertexDescriptor.attributes[0].offset = 0
        vertexDescriptor.attributes[0].bufferIndex = 0
        vertexDescriptor.attributes[1].format = .float2
        vertexDescriptor.attributes[1].offset = MemoryLayout<Float>.size * 2
        vertexDescriptor.attributes[1].bufferIndex = 0
        vertexDescriptor.attributes[2].format = .float4
        vertexDescriptor.attributes[2].offset = MemoryLayout<Float>.size * 4
        vertexDescriptor.attributes[2].bufferIndex = 0
        vertexDescriptor.layouts[0].stride = MemoryLayout<NodeVertex>.stride

        nodeDescriptor.vertexDescriptor = vertexDescriptor

        do {
            pipelineState = try device.makeRenderPipelineState(descriptor: nodeDescriptor)
        } catch {
            print("Failed to create node pipeline: \(error)")
        }

        // Edge pipeline
        let edgeDescriptor = MTLRenderPipelineDescriptor()
        edgeDescriptor.vertexFunction = library.makeFunction(name: "edgeVertexShader")
        edgeDescriptor.fragmentFunction = library.makeFunction(name: "edgeFragmentShader")
        edgeDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        edgeDescriptor.colorAttachments[0].isBlendingEnabled = true
        edgeDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        edgeDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha

        do {
            edgePipelineState = try device.makeRenderPipelineState(descriptor: edgeDescriptor)
        } catch {
            print("Failed to create edge pipeline: \(error)")
        }

        // Grid pipeline
        let gridDescriptor = MTLRenderPipelineDescriptor()
        gridDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        gridDescriptor.fragmentFunction = library.makeFunction(name: "gridFragmentShader")
        gridDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        do {
            gridPipelineState = try device.makeRenderPipelineState(descriptor: gridDescriptor)
        } catch {
            print("Failed to create grid pipeline: \(error)")
        }

        // Glow pipeline (for selected node)
        let glowDescriptor = MTLRenderPipelineDescriptor()
        glowDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        glowDescriptor.fragmentFunction = library.makeFunction(name: "glowFragmentShader")
        glowDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        glowDescriptor.colorAttachments[0].isBlendingEnabled = true
        glowDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        glowDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        glowDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        glowDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        glowDescriptor.vertexDescriptor = vertexDescriptor

        do {
            glowPipelineState = try device.makeRenderPipelineState(descriptor: glowDescriptor)
        } catch {
            print("Failed to create glow pipeline: \(error)")
        }

        // Text pipeline
        let textDescriptor = MTLRenderPipelineDescriptor()
        textDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        textDescriptor.fragmentFunction = library.makeFunction(name: "textFragmentShader")
        textDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        textDescriptor.colorAttachments[0].isBlendingEnabled = true
        textDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        textDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        textDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        textDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        textDescriptor.vertexDescriptor = vertexDescriptor

        do {
            textPipelineState = try device.makeRenderPipelineState(descriptor: textDescriptor)
        } catch {
            print("Failed to create text pipeline: \(error)")
        }

        // Running node pipeline (animated spinner)
        let runningDescriptor = MTLRenderPipelineDescriptor()
        runningDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        runningDescriptor.fragmentFunction = library.makeFunction(name: "runningNodeFragmentShader")
        runningDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        runningDescriptor.colorAttachments[0].isBlendingEnabled = true
        runningDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        runningDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        runningDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        runningDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        runningDescriptor.vertexDescriptor = vertexDescriptor

        do {
            runningPipelineState = try device.makeRenderPipelineState(descriptor: runningDescriptor)
        } catch {
            print("Failed to create running node pipeline: \(error)")
        }

        // Arrow head pipeline
        let arrowDescriptor = MTLRenderPipelineDescriptor()
        arrowDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        arrowDescriptor.fragmentFunction = library.makeFunction(name: "arrowFragmentShader")
        arrowDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        arrowDescriptor.colorAttachments[0].isBlendingEnabled = true
        arrowDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        arrowDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        arrowDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        arrowDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        arrowDescriptor.vertexDescriptor = vertexDescriptor

        do {
            arrowPipelineState = try device.makeRenderPipelineState(descriptor: arrowDescriptor)
        } catch {
            print("Failed to create arrow pipeline: \(error)")
        }

        // Active edge pipeline (animated flow for edges connected to running nodes)
        let activeEdgeDescriptor = MTLRenderPipelineDescriptor()
        activeEdgeDescriptor.vertexFunction = library.makeFunction(name: "edgeVertexShader")
        activeEdgeDescriptor.fragmentFunction = library.makeFunction(name: "activeEdgeFragmentShader")
        activeEdgeDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        activeEdgeDescriptor.colorAttachments[0].isBlendingEnabled = true
        activeEdgeDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        activeEdgeDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        activeEdgeDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        activeEdgeDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha

        do {
            activeEdgePipelineState = try device.makeRenderPipelineState(descriptor: activeEdgeDescriptor)
        } catch {
            print("Failed to create active edge pipeline: \(error)")
        }

        // Group pipeline (collapsible node groups with dashed border)
        let groupDescriptor = MTLRenderPipelineDescriptor()
        groupDescriptor.vertexFunction = library.makeFunction(name: "nodeVertexShader")
        groupDescriptor.fragmentFunction = library.makeFunction(name: "groupFragmentShader")
        groupDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        groupDescriptor.colorAttachments[0].isBlendingEnabled = true
        groupDescriptor.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        groupDescriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        groupDescriptor.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        groupDescriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        groupDescriptor.vertexDescriptor = vertexDescriptor

        do {
            groupPipelineState = try device.makeRenderPipelineState(descriptor: groupDescriptor)
        } catch {
            print("Failed to create group pipeline: \(error)")
        }
    }

    private func setupBuffers() {
        // Uniform buffer
        uniformBuffer = device.makeBuffer(
            length: MemoryLayout<Uniforms>.size,
            options: .storageModeShared
        )
    }

    private func setupTextRendering() {
        // Create font atlas
        fontAtlas = FontAtlas(device: device, fontName: "SF Mono", fontSize: 12.0)

        // Create sampler state
        let samplerDescriptor = MTLSamplerDescriptor()
        samplerDescriptor.minFilter = .linear
        samplerDescriptor.magFilter = .linear
        samplerDescriptor.mipFilter = .notMipmapped
        samplerDescriptor.sAddressMode = .clampToEdge
        samplerDescriptor.tAddressMode = .clampToEdge
        samplerState = device.makeSamplerState(descriptor: samplerDescriptor)
    }

    /// Set zoom level
    func setZoom(_ zoom: Float) {
        self.zoom = zoom
    }

    /// Set pan offset
    func setPan(_ pan: simd_float2) {
        self.pan = pan
    }

    /// Set selected node by ID
    func setSelectedNode(_ nodeId: String?, in graph: GraphModel) {
        if let nodeId = nodeId,
           let index = graph.nodes.firstIndex(where: { $0.id == nodeId }) {
            selectedNodeIndex = index
        } else {
            selectedNodeIndex = nil
        }
    }

    /// Set selected group by ID
    func setSelectedGroup(_ groupId: String?, in graph: GraphModel) {
        if let groupId = groupId {
            // Find index among collapsed groups only
            let collapsedGroups = graph.groups.filter { $0.collapsed }
            if let index = collapsedGroups.firstIndex(where: { $0.id == groupId }) {
                selectedGroupIndex = index
            } else {
                selectedGroupIndex = nil
            }
        } else {
            selectedGroupIndex = nil
        }
    }

    /// Update the graph data for rendering
    func updateGraph(_ graph: GraphModel) {
        let nodesById = Dictionary(uniqueKeysWithValues: graph.nodes.map { ($0.id, $0) })
        let groupPositionsById: [String: simd_float2] = Dictionary(
            uniqueKeysWithValues: graph.groups.compactMap { group in
                guard let position = group.position else { return nil }
                return (group.id, simd_float2(Float(position.x), Float(position.y)))
            }
        )

        // Build set of collapsed group IDs
        let collapsedGroupIds = Set(graph.groups.filter { $0.collapsed }.map { $0.id })

        // Track running node IDs
        let runningNodeIds = Set(graph.nodes.filter { $0.status == .running }.map { $0.id })

        // Convert graph nodes to render nodes and track running indices
        // Skip nodes that belong to collapsed groups
        runningNodeIndices = []
        var visibleNodeIndex = 0
        nodes = graph.nodes.compactMap { node -> RenderNode? in
            // Skip nodes in collapsed groups
            if let groupId = node.groupId, collapsedGroupIds.contains(groupId) {
                return nil
            }

            if node.status == .running {
                runningNodeIndices.append(visibleNodeIndex)
            }
            visibleNodeIndex += 1

            return RenderNode(
                position: simd_float2(
                    Float(node.position?.x ?? 0),
                    Float(node.position?.y ?? 0)
                ),
                size: simd_float2(80, 40),
                color: colorForStatus(node.status),
                cornerRadius: 8
            )
        }

        // Store node labels (only for visible nodes)
        nodeLabels = graph.nodes.compactMap { node -> String? in
            if let groupId = node.groupId, collapsedGroupIds.contains(groupId) {
                return nil
            }
            return node.label
        }

        // Convert collapsed groups to render groups
        let collapsedGroupsWithPosition = graph.groups.filter { $0.collapsed && $0.position != nil }
        groups = collapsedGroupsWithPosition.compactMap { group -> RenderGroup? in
            guard let position = group.position else { return nil }
            return RenderGroup(
                position: simd_float2(Float(position.x), Float(position.y)),
                size: collapsedGroupRenderSize,
                color: colorForStatus(group.status),
                collapsed: group.collapsed
            )
        }
        groupLabels = collapsedGroupsWithPosition.map { $0.label }

        // Convert edges to render edges, marking active ones
        edges = graph.edges.compactMap { edge -> RenderEdge? in
            guard let fromNode = nodesById[edge.from],
                  let toNode = nodesById[edge.to] else {
                return nil
            }

            let fromCollapsedGroupId = fromNode.groupId.flatMap { collapsedGroupIds.contains($0) ? $0 : nil }
            let toCollapsedGroupId = toNode.groupId.flatMap { collapsedGroupIds.contains($0) ? $0 : nil }

            // Skip edges entirely within the same collapsed group
            if let fromGroupId = fromCollapsedGroupId,
               let toGroupId = toCollapsedGroupId,
               fromGroupId == toGroupId {
                return nil
            }

            guard let fromAnchor = edgeAnchorPosition(
                node: fromNode,
                collapsedGroupId: fromCollapsedGroupId,
                groupPositionsById: groupPositionsById
            ),
            let toAnchor = edgeAnchorPosition(
                node: toNode,
                collapsedGroupId: toCollapsedGroupId,
                groupPositionsById: groupPositionsById
            ) else {
                return nil
            }

            var start = fromAnchor
            var end = toAnchor

            if fromCollapsedGroupId != nil {
                start = clippedPointOnRectBoundary(center: fromAnchor, size: collapsedGroupRenderSize, towards: toAnchor)
            }
            if toCollapsedGroupId != nil {
                end = clippedPointOnRectBoundary(center: toAnchor, size: collapsedGroupRenderSize, towards: fromAnchor)
            }

            // Edge is active if either endpoint is a running node (including nodes in collapsed groups)
            let isActive = runningNodeIds.contains(edge.from) || runningNodeIds.contains(edge.to)

            return RenderEdge(
                start: start,
                end: end,
                color: colorForEdgeType(edge.type),
                thickness: 2.0,
                isActive: isActive,
                endInset: toCollapsedGroupId == nil ? nodeArrowEndInset : 0.0
            )
        }

        // Update vertex buffers
        updateVertexBuffers()
    }

    private func edgeAnchorPosition(
        node: GraphNode,
        collapsedGroupId: String?,
        groupPositionsById: [String: simd_float2]
    ) -> simd_float2? {
        if let groupId = collapsedGroupId, let groupPosition = groupPositionsById[groupId] {
            return groupPosition
        }
        guard let position = node.position else { return nil }
        return simd_float2(Float(position.x), Float(position.y))
    }

    private func safeNormalize(_ vector: simd_float2) -> simd_float2 {
        let lengthSquared = simd_length_squared(vector)
        guard lengthSquared > 1e-6 else { return simd_float2(1, 0) }
        return vector / sqrt(lengthSquared)
    }

    private func clippedPointOnRectBoundary(center: simd_float2, size: simd_float2, towards: simd_float2) -> simd_float2 {
        let direction = safeNormalize(towards - center)
        let halfSize = size / 2

        let tx = abs(direction.x) > 1e-6 ? (halfSize.x / abs(direction.x)) : Float.greatestFiniteMagnitude
        let ty = abs(direction.y) > 1e-6 ? (halfSize.y / abs(direction.y)) : Float.greatestFiniteMagnitude
        let t = min(tx, ty)

        return center + direction * t
    }

    private func updateVertexBuffers() {
        // Create node vertices (excluding running nodes which get separate buffer)
        var nodeVertices: [NodeVertex] = []
        var runningVertices: [NodeVertex] = []

        for (index, node) in nodes.enumerated() {
            let vertices = createQuadVertices(
                position: node.position,
                size: node.size,
                color: node.color
            )
            if runningNodeIndices.contains(index) {
                runningVertices.append(contentsOf: vertices)
            } else {
                nodeVertices.append(contentsOf: vertices)
            }
        }

        if !nodeVertices.isEmpty {
            nodeVertexBuffer = device.makeBuffer(
                bytes: nodeVertices,
                length: nodeVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            nodeVertexBuffer = nil
        }

        if !runningVertices.isEmpty {
            runningVertexBuffer = device.makeBuffer(
                bytes: runningVertices,
                length: runningVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            runningVertexBuffer = nil
        }

        // Create edge vertices (line segments) - separate active and inactive
        var inactiveEdgeVertices: [EdgeVertex] = []
        var activeEdgeVertices: [EdgeVertex] = []

        for edge in edges {
            let direction = safeNormalize(edge.end - edge.start)
            let vertices = [
                EdgeVertex(position: edge.start, direction: direction, progress: 0, color: edge.color),
                EdgeVertex(position: edge.start, direction: direction, progress: 0, color: edge.color),
                EdgeVertex(position: edge.end, direction: direction, progress: 1, color: edge.color),
                EdgeVertex(position: edge.end, direction: direction, progress: 1, color: edge.color)
            ]

            if edge.isActive {
                activeEdgeVertices.append(contentsOf: vertices)
            } else {
                inactiveEdgeVertices.append(contentsOf: vertices)
            }
        }

        inactiveEdgeCount = inactiveEdgeVertices.count / 4
        activeEdgeCount = activeEdgeVertices.count / 4

        if !inactiveEdgeVertices.isEmpty {
            edgeVertexBuffer = device.makeBuffer(
                bytes: inactiveEdgeVertices,
                length: inactiveEdgeVertices.count * MemoryLayout<EdgeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            edgeVertexBuffer = nil
        }

        if !activeEdgeVertices.isEmpty {
            activeEdgeVertexBuffer = device.makeBuffer(
                bytes: activeEdgeVertices,
                length: activeEdgeVertices.count * MemoryLayout<EdgeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            activeEdgeVertexBuffer = nil
        }

        // Create arrow head vertices at edge endpoints
        updateArrowVertices()

        // Create text vertices for node labels
        updateTextVertices()

        // Create group vertices
        updateGroupVertices()
    }

    private func updateGroupVertices() {
        var groupVertices: [NodeVertex] = []

        for group in groups {
            let vertices = createQuadVertices(
                position: group.position,
                size: group.size,
                color: group.color
            )
            groupVertices.append(contentsOf: vertices)
        }

        if !groupVertices.isEmpty {
            groupVertexBuffer = device.makeBuffer(
                bytes: groupVertices,
                length: groupVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            groupVertexBuffer = nil
        }

        // Also update group text vertices
        updateGroupTextVertices()
    }

    private func updateGroupTextVertices() {
        guard let atlas = fontAtlas else { return }

        var textVertices: [NodeVertex] = []
        let textColor = simd_float4(1.0, 1.0, 1.0, 1.0) // White text

        for (index, group) in groups.enumerated() {
            guard index < groupLabels.count else { continue }
            let label = groupLabels[index]

            // Measure text to center it
            let textWidth = Float(atlas.measureString(label))
            let lineHeight = Float(atlas.lineHeight)

            // Start position (centered on group, offset down slightly for icon)
            var x = group.position.x - textWidth / 2
            let y = group.position.y + 4 // Below the folder icon

            // Generate vertices for each character
            for char in label {
                guard let metrics = atlas.metrics(for: char) else { continue }

                let glyphWidth = Float(metrics.size.width)
                let glyphHeight = Float(metrics.size.height)

                // Glyph quad position
                let glyphX = x
                let glyphY = y

                // UV coordinates from atlas
                let uvRect = metrics.uvRect
                let u0 = Float(uvRect.minX)
                let v0 = Float(uvRect.minY)
                let u1 = Float(uvRect.maxX)
                let v1 = Float(uvRect.maxY)

                // Create quad (2 triangles, 6 vertices)
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY),
                    texCoord: simd_float2(u0, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY),
                    texCoord: simd_float2(u1, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY + glyphHeight),
                    texCoord: simd_float2(u0, v1),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY),
                    texCoord: simd_float2(u1, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY + glyphHeight),
                    texCoord: simd_float2(u1, v1),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY + glyphHeight),
                    texCoord: simd_float2(u0, v1),
                    color: textColor
                ))

                x += Float(metrics.advance)
            }
        }

        groupTextVertexCount = textVertices.count

        if !textVertices.isEmpty {
            groupTextVertexBuffer = device.makeBuffer(
                bytes: textVertices,
                length: textVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            groupTextVertexBuffer = nil
        }
    }

    private func updateArrowVertices() {
        var arrowVertices: [NodeVertex] = []
        let arrowSize: Float = 12.0  // Arrow head size in pixels

        for edge in edges {
            let direction = safeNormalize(edge.end - edge.start)
            let perpendicular = simd_float2(-direction.y, direction.x)

            // Arrow head positioned at the end of the edge, pulled back slightly
            let arrowTip = edge.end - direction * edge.endInset
            let arrowBase = arrowTip - direction * arrowSize
            let arrowLeft = arrowBase + perpendicular * (arrowSize * 0.5)
            let arrowRight = arrowBase - perpendicular * (arrowSize * 0.5)

            // Triangle vertices with UV coords for SDF
            arrowVertices.append(NodeVertex(position: arrowTip, texCoord: simd_float2(0.5, 0.0), color: edge.color))
            arrowVertices.append(NodeVertex(position: arrowLeft, texCoord: simd_float2(0.0, 1.0), color: edge.color))
            arrowVertices.append(NodeVertex(position: arrowRight, texCoord: simd_float2(1.0, 1.0), color: edge.color))
        }

        arrowVertexCount = arrowVertices.count

        if !arrowVertices.isEmpty {
            arrowVertexBuffer = device.makeBuffer(
                bytes: arrowVertices,
                length: arrowVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        } else {
            arrowVertexBuffer = nil
        }
    }

    private func updateTextVertices() {
        guard let atlas = fontAtlas else { return }

        var textVertices: [NodeVertex] = []
        let textColor = simd_float4(1.0, 1.0, 1.0, 1.0) // White text

        for (index, node) in nodes.enumerated() {
            guard index < nodeLabels.count else { continue }
            let label = nodeLabels[index]

            // Measure text to center it
            let textWidth = Float(atlas.measureString(label))
            let lineHeight = Float(atlas.lineHeight)

            // Start position (centered on node)
            var x = node.position.x - textWidth / 2
            let y = node.position.y - lineHeight / 2

            // Generate vertices for each character
            for char in label {
                guard let metrics = atlas.metrics(for: char) else { continue }

                let glyphWidth = Float(metrics.size.width)
                let glyphHeight = Float(metrics.size.height)

                // Glyph quad position
                let glyphX = x
                let glyphY = y

                // UV coordinates from atlas
                let uvRect = metrics.uvRect
                let u0 = Float(uvRect.minX)
                let v0 = Float(uvRect.minY)
                let u1 = Float(uvRect.maxX)
                let v1 = Float(uvRect.maxY)

                // Create quad (2 triangles, 6 vertices)
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY),
                    texCoord: simd_float2(u0, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY),
                    texCoord: simd_float2(u1, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY + glyphHeight),
                    texCoord: simd_float2(u0, v1),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY),
                    texCoord: simd_float2(u1, v0),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX + glyphWidth, glyphY + glyphHeight),
                    texCoord: simd_float2(u1, v1),
                    color: textColor
                ))
                textVertices.append(NodeVertex(
                    position: simd_float2(glyphX, glyphY + glyphHeight),
                    texCoord: simd_float2(u0, v1),
                    color: textColor
                ))

                x += Float(metrics.advance)
            }
        }

        textVertexCount = textVertices.count

        if !textVertices.isEmpty {
            textVertexBuffer = device.makeBuffer(
                bytes: textVertices,
                length: textVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )
        }
    }

    private func createQuadVertices(position: simd_float2, size: simd_float2, color: simd_float4) -> [NodeVertex] {
        let halfSize = size / 2
        return [
            NodeVertex(position: position + simd_float2(-halfSize.x, -halfSize.y), texCoord: simd_float2(0, 0), color: color),
            NodeVertex(position: position + simd_float2(halfSize.x, -halfSize.y), texCoord: simd_float2(1, 0), color: color),
            NodeVertex(position: position + simd_float2(-halfSize.x, halfSize.y), texCoord: simd_float2(0, 1), color: color),
            NodeVertex(position: position + simd_float2(halfSize.x, -halfSize.y), texCoord: simd_float2(1, 0), color: color),
            NodeVertex(position: position + simd_float2(halfSize.x, halfSize.y), texCoord: simd_float2(1, 1), color: color),
            NodeVertex(position: position + simd_float2(-halfSize.x, halfSize.y), texCoord: simd_float2(0, 1), color: color),
        ]
    }

    /// Render the graph
    func render(in view: MTKView) {
        guard let drawable = view.currentDrawable,
              let descriptor = view.currentRenderPassDescriptor,
              let commandBuffer = commandQueue.makeCommandBuffer(),
              let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: descriptor) else {
            return
        }

        // Update uniforms
        let currentTime = CACurrentMediaTime()
        uniforms.time = Float(currentTime - startTime)
        uniforms.viewportSize = simd_float2(Float(view.drawableSize.width), Float(view.drawableSize.height))
        uniforms.projectionMatrix = makeOrthographicMatrix(
            left: -Float(view.drawableSize.width) / 2,
            right: Float(view.drawableSize.width) / 2,
            bottom: -Float(view.drawableSize.height) / 2,
            top: Float(view.drawableSize.height) / 2,
            near: -1,
            far: 1
        )
        uniforms.viewMatrix = makeViewMatrix(pan: pan, zoom: zoom)

        memcpy(uniformBuffer?.contents(), &uniforms, MemoryLayout<Uniforms>.size)

        // Draw grid background
        if let gridPipeline = gridPipelineState {
            encoder.setRenderPipelineState(gridPipeline)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            // Draw full screen quad for grid
        }

        // Draw inactive edges (standard shader)
        if let edgePipeline = edgePipelineState, let edgeBuffer = edgeVertexBuffer, inactiveEdgeCount > 0 {
            encoder.setRenderPipelineState(edgePipeline)
            encoder.setVertexBuffer(edgeBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: inactiveEdgeCount * 4)
        }

        // Draw active edges (animated shader for edges connected to running nodes)
        if let activeEdgePipeline = activeEdgePipelineState,
           let activeBuffer = activeEdgeVertexBuffer,
           activeEdgeCount > 0 {
            encoder.setRenderPipelineState(activeEdgePipeline)
            encoder.setVertexBuffer(activeBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: activeEdgeCount * 4)
        }

        // Draw arrow heads at edge endpoints
        if let arrowPipeline = arrowPipelineState,
           let arrowBuffer = arrowVertexBuffer,
           arrowVertexCount > 0 {
            encoder.setRenderPipelineState(arrowPipeline)
            encoder.setVertexBuffer(arrowBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: arrowVertexCount)
        }

        // Draw collapsed groups
        if let groupPipeline = groupPipelineState,
           let groupBuffer = groupVertexBuffer,
           !groups.isEmpty {
            encoder.setRenderPipelineState(groupPipeline)
            encoder.setVertexBuffer(groupBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: groups.count * 6)
        }

        // Draw group labels
        if let textPipeline = textPipelineState,
           let groupTextBuffer = groupTextVertexBuffer,
           let atlas = fontAtlas,
           let sampler = samplerState,
           groupTextVertexCount > 0 {
            encoder.setRenderPipelineState(textPipeline)
            encoder.setVertexBuffer(groupTextBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentTexture(atlas.texture, index: 0)
            encoder.setFragmentSamplerState(sampler, index: 0)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: groupTextVertexCount)
        }

        // Draw selection glow behind selected group
        if let glowPipeline = glowPipelineState,
           let selectedIndex = selectedGroupIndex,
           selectedIndex < groups.count {
            let group = groups[selectedIndex]
            // Create larger glow quad
            let glowSize = group.size * 2.0
            let glowColor = simd_float4(0.4, 0.7, 1.0, 0.8)  // Blue glow
            let glowVertices = createQuadVertices(
                position: group.position,
                size: glowSize,
                color: glowColor
            )

            let groupGlowBuffer = device.makeBuffer(
                bytes: glowVertices,
                length: glowVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )

            if let glowBuffer = groupGlowBuffer {
                encoder.setRenderPipelineState(glowPipeline)
                encoder.setVertexBuffer(glowBuffer, offset: 0, index: 0)
                encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
                encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
            }
        }

        // Draw selection glow behind selected node
        if let glowPipeline = glowPipelineState,
           let selectedIndex = selectedNodeIndex,
           selectedIndex < nodes.count {
            let node = nodes[selectedIndex]
            // Create larger glow quad
            let glowSize = node.size * 2.0
            let glowColor = simd_float4(0.4, 0.7, 1.0, 0.8)  // Blue glow
            let glowVertices = createQuadVertices(
                position: node.position,
                size: glowSize,
                color: glowColor
            )

            glowVertexBuffer = device.makeBuffer(
                bytes: glowVertices,
                length: glowVertices.count * MemoryLayout<NodeVertex>.stride,
                options: .storageModeShared
            )

            if let glowBuffer = glowVertexBuffer {
                encoder.setRenderPipelineState(glowPipeline)
                encoder.setVertexBuffer(glowBuffer, offset: 0, index: 0)
                encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
                encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
            }
        }

        // Draw non-running nodes (static shader)
        let nonRunningCount = nodes.count - runningNodeIndices.count
        if let nodePipeline = pipelineState, let nodeBuffer = nodeVertexBuffer, nonRunningCount > 0 {
            encoder.setRenderPipelineState(nodePipeline)
            encoder.setVertexBuffer(nodeBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: nonRunningCount * 6)
        }

        // Draw running nodes (animated spinner shader)
        if let runningPipeline = runningPipelineState,
           let runningBuffer = runningVertexBuffer,
           !runningNodeIndices.isEmpty {
            encoder.setRenderPipelineState(runningPipeline)
            encoder.setVertexBuffer(runningBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: runningNodeIndices.count * 6)
        }

        // Draw node labels
        if let textPipeline = textPipelineState,
           let textBuffer = textVertexBuffer,
           let atlas = fontAtlas,
           let sampler = samplerState,
           textVertexCount > 0 {
            encoder.setRenderPipelineState(textPipeline)
            encoder.setVertexBuffer(textBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentTexture(atlas.texture, index: 0)
            encoder.setFragmentSamplerState(sampler, index: 0)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: textVertexCount)
        }

        encoder.endEncoding()
        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    private func makeOrthographicMatrix(left: Float, right: Float, bottom: Float, top: Float, near: Float, far: Float) -> simd_float4x4 {
        let sx = 2 / (right - left)
        let sy = 2 / (top - bottom)
        let sz = 1 / (far - near)
        let tx = -(right + left) / (right - left)
        let ty = -(top + bottom) / (top - bottom)
        let tz = -near / (far - near)

        return simd_float4x4(columns: (
            simd_float4(sx, 0, 0, 0),
            simd_float4(0, sy, 0, 0),
            simd_float4(0, 0, sz, 0),
            simd_float4(tx, ty, tz, 1)
        ))
    }

    private func makeViewMatrix(pan: simd_float2, zoom: Float) -> simd_float4x4 {
        // Scale then translate: V = T * S
        // Scale matrix
        let scaleMatrix = simd_float4x4(columns: (
            simd_float4(zoom, 0, 0, 0),
            simd_float4(0, zoom, 0, 0),
            simd_float4(0, 0, 1, 0),
            simd_float4(0, 0, 0, 1)
        ))

        // Translation matrix (pan offset)
        let translateMatrix = simd_float4x4(columns: (
            simd_float4(1, 0, 0, 0),
            simd_float4(0, 1, 0, 0),
            simd_float4(0, 0, 1, 0),
            simd_float4(pan.x, pan.y, 0, 1)
        ))

        // Combined: first scale, then translate
        return translateMatrix * scaleMatrix
    }

    private func colorForStatus(_ status: NodeStatus) -> simd_float4 {
        switch status {
        case .pending: return simd_float4(0.3, 0.3, 0.3, 1.0)
        case .running: return simd_float4(0.2, 0.5, 1.0, 1.0)
        case .success: return simd_float4(0.2, 0.8, 0.3, 1.0)
        case .failed: return simd_float4(0.9, 0.2, 0.2, 1.0)
        case .skipped: return simd_float4(0.5, 0.5, 0.5, 0.7)
        case .waiting: return simd_float4(1.0, 0.7, 0.2, 1.0)
        }
    }

    private func colorForEdgeType(_ type: EdgeType) -> simd_float4 {
        switch type {
        case .normal: return simd_float4(0.5, 0.5, 0.5, 1.0)
        case .conditionalTrue: return simd_float4(0.2, 0.8, 0.3, 1.0)
        case .conditionalFalse: return simd_float4(0.9, 0.2, 0.2, 1.0)
        case .error: return simd_float4(0.9, 0.2, 0.2, 0.8)
        case .fork, .join: return simd_float4(0.2, 0.5, 1.0, 1.0)
        case .loop: return simd_float4(1.0, 0.7, 0.2, 1.0)
        }
    }
}

// MARK: - Supporting Types

struct Uniforms {
    var projectionMatrix: simd_float4x4 = matrix_identity_float4x4
    var viewMatrix: simd_float4x4 = matrix_identity_float4x4
    var viewportSize: simd_float2 = simd_float2(1, 1)
    var time: Float = 0
    var padding: Float = 0
}

struct NodeVertex {
    var position: simd_float2
    var texCoord: simd_float2
    var color: simd_float4
}

struct EdgeVertex {
    var position: simd_float2
    var direction: simd_float2
    var progress: Float
    var color: simd_float4
}

struct RenderNode {
    var position: simd_float2
    var size: simd_float2
    var color: simd_float4
    var cornerRadius: Float
}

struct RenderEdge {
    var start: simd_float2
    var end: simd_float2
    var color: simd_float4
    var thickness: Float
    var isActive: Bool = false
    var endInset: Float = 40.0
}

struct RenderGroup {
    var position: simd_float2
    var size: simd_float2
    var color: simd_float4
    var collapsed: Bool
}

// MARK: - MTKViewDelegate

extension GraphRenderer: MTKViewDelegate {
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        // Handle resize
    }

    func draw(in view: MTKView) {
        render(in: view)
    }
}
