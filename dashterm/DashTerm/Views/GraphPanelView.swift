//
//  GraphPanelView.swift
//  DashTerm
//
//  Panel for displaying computation graph visualization
//

import SwiftUI

struct GraphPanelView: View {
    @EnvironmentObject var appState: AppState
    @State private var selectedNode: String?
    @State private var zoomLevel: CGFloat = 1.0
    @State private var panOffset: CGPoint = .zero
    @State private var triggerZoomToFit: Bool = false
    @AppStorage("showMinimap") private var showMinimap: Bool = true

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Computation Graph")
                    .font(.headline)
                    .foregroundColor(.white)

                // Agent parsing indicator
                if appState.agentParsingEnabled {
                    HStack(spacing: 4) {
                        Circle()
                            .fill(Color.green)
                            .frame(width: 8, height: 8)
                        Text("Live")
                            .font(.caption)
                            .foregroundColor(.green)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.green.opacity(0.2))
                    .cornerRadius(4)
                }

                Spacer()

                // Agent parsing toggle
                Button(action: { appState.toggleAgentParsing() }) {
                    HStack(spacing: 4) {
                        Image(systemName: appState.agentParsingEnabled ? "waveform.circle.fill" : "waveform.circle")
                        Text(appState.agentParsingEnabled ? "Parsing" : "Parse")
                            .font(.caption)
                    }
                }
                .buttonStyle(.plain)
                .foregroundColor(appState.agentParsingEnabled ? .green : .gray)
                .padding(.trailing, 8)

                // Zoom controls
                HStack(spacing: 8) {
                    Button(action: { zoomLevel = max(0.5, zoomLevel - 0.1) }) {
                        Image(systemName: "minus.magnifyingglass")
                    }
                    .buttonStyle(.plain)

                    Text("\(Int(zoomLevel * 100))%")
                        .font(.caption)
                        .foregroundColor(.gray)

                    Button(action: { zoomLevel = min(2.0, zoomLevel + 0.1) }) {
                        Image(systemName: "plus.magnifyingglass")
                    }
                    .buttonStyle(.plain)

                    Divider()
                        .frame(height: 16)

                    // Zoom to fit button
                    Button(action: { triggerZoomToFit = true }) {
                        Image(systemName: "arrow.up.left.and.arrow.down.right")
                    }
                    .buttonStyle(.plain)
                    .help("Zoom to fit all nodes")

                    // Minimap toggle button
                    Button(action: { showMinimap.toggle() }) {
                        Image(systemName: showMinimap ? "map.fill" : "map")
                    }
                    .buttonStyle(.plain)
                    .foregroundColor(showMinimap ? .white : .gray)
                    .help(showMinimap ? "Hide minimap" : "Show minimap")
                }
                .foregroundColor(.gray)
            }
            .padding()
            .background(Color(white: 0.15))

            // Graph canvas
            if let graph = appState.currentGraph {
                GraphCanvasView(
                    graph: graph,
                    selectedNode: $selectedNode,
                    zoomLevel: $zoomLevel,
                    panOffset: $panOffset,
                    triggerZoomToFit: $triggerZoomToFit
                )
            } else {
                emptyState
            }

            // Node details panel
            if let nodeId = selectedNode, let graph = appState.currentGraph {
                NodeDetailView(graph: graph, nodeId: nodeId)
                    .frame(height: 150)
            }
        }
        .background(Color(white: 0.1))
    }

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "point.3.connected.trianglepath.dotted")
                .font(.system(size: 48))
                .foregroundColor(.gray)

            Text("No Active Graph")
                .font(.headline)
                .foregroundColor(.gray)

            Text("Run an agent or load a graph to visualize")
                .font(.caption)
                .foregroundColor(.gray.opacity(0.7))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Graph canvas with Metal acceleration
struct GraphCanvasView: View {
    @ObservedObject var graph: GraphModel
    @Binding var selectedNode: String?
    @Binding var zoomLevel: CGFloat
    @Binding var panOffset: CGPoint
    @Binding var triggerZoomToFit: Bool
    @AppStorage("useMetalRendering") private var useMetalRendering: Bool = true
    @AppStorage("showMinimap") private var showMinimap: Bool = true
    @State private var selectedGroup: String?

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .bottomTrailing) {
                if useMetalRendering {
                    MetalGraphView(
                        graph: graph,
                        selectedNode: $selectedNode,
                        zoomLevel: $zoomLevel,
                        panOffset: $panOffset,
                        selectedGroup: $selectedGroup,
                        onGroupToggle: { groupId in
                            graph.toggleGroupAnimated(groupId)
                        }
                    )
                } else {
                    swiftUIFallback
                }

                // Minimap overlay (only show if enabled and graph has nodes)
                if showMinimap && !graph.nodes.isEmpty {
                    MinimapView(
                        graph: graph,
                        zoomLevel: $zoomLevel,
                        panOffset: $panOffset,
                        viewportSize: geometry.size,
                        selectedNodeId: selectedNode,
                        selectedGroupId: selectedGroup,
                        onGroupSelected: { groupId in
                            selectedNode = nil
                            selectedGroup = groupId
                        }
                    )
                    .padding(8)
                    .transition(.opacity)
                }
            }
            .onChange(of: triggerZoomToFit) { _, triggered in
                if triggered {
                    performZoomToFit(viewportSize: geometry.size)
                    triggerZoomToFit = false
                }
            }
            // Keyboard shortcuts for navigation
            .onKeyPress(.leftArrow) {
                withAnimation(.easeOut(duration: 0.15)) {
                    panOffset.x += 50
                }
                return .handled
            }
            .onKeyPress(.rightArrow) {
                withAnimation(.easeOut(duration: 0.15)) {
                    panOffset.x -= 50
                }
                return .handled
            }
            .onKeyPress(.upArrow) {
                withAnimation(.easeOut(duration: 0.15)) {
                    panOffset.y += 50
                }
                return .handled
            }
            .onKeyPress(.downArrow) {
                withAnimation(.easeOut(duration: 0.15)) {
                    panOffset.y -= 50
                }
                return .handled
            }
            .onKeyPress(keys: [KeyEquivalent("=")]) { _ in
                withAnimation(.easeOut(duration: 0.15)) {
                    zoomLevel = min(2.0, zoomLevel + 0.1)
                }
                return .handled
            }
            .onKeyPress(keys: [KeyEquivalent("-")]) { _ in
                withAnimation(.easeOut(duration: 0.15)) {
                    zoomLevel = max(0.3, zoomLevel - 0.1)
                }
                return .handled
            }
            .onKeyPress(.escape) {
                selectedNode = nil
                return .handled
            }
            .focusable()
        }
    }

    private func performZoomToFit(viewportSize: CGSize) {
        let (zoom, panX, panY) = graph.computeZoomToFit(
            viewportWidth: viewportSize.width,
            viewportHeight: viewportSize.height
        )

        withAnimation(.easeInOut(duration: 0.3)) {
            zoomLevel = zoom
            panOffset = CGPoint(x: panX, y: panY)
        }
    }

    private var swiftUIFallback: some View {
        GeometryReader { geometry in
            ZStack {
                // Edges
                ForEach(graph.edges, id: \.id) { edge in
                    EdgeView(
                        edge: edge,
                        graph: graph,
                        zoomLevel: zoomLevel,
                        panOffset: panOffset,
                        viewportSize: geometry.size
                    )
                }

                // Collapsed groups
                ForEach(graph.groups.filter { $0.collapsed }) { group in
                    if let position = group.position {
                        GroupView(
                            group: group,
                            isSelected: selectedGroup == group.id,
                            onSelect: {
                                selectedNode = nil
                                selectedGroup = group.id
                            },
                            onToggle: {
                                graph.toggleGroupAnimated(group.id)
                            }
                        )
                        .position(
                            x: position.x * zoomLevel + geometry.size.width / 2 + panOffset.x,
                            y: position.y * zoomLevel + geometry.size.height / 2 + panOffset.y
                        )
                        .scaleEffect(zoomLevel)
                    }
                }

                // Nodes (hide nodes that belong to collapsed groups)
                ForEach(graph.nodes.filter { node in
                    // Show node if it's not in any group, or if its group is not collapsed
                    guard let groupId = node.groupId else { return true }
                    return !graph.isGroupCollapsed(groupId)
                }) { node in
                    NodeView(
                        node: node,
                        isSelected: selectedNode == node.id,
                        onSelect: {
                            selectedGroup = nil
                            selectedNode = node.id
                        }
                    )
                    .position(
                        x: (node.position?.x ?? 0) * zoomLevel + geometry.size.width / 2 + panOffset.x,
                        y: (node.position?.y ?? 0) * zoomLevel + geometry.size.height / 2 + panOffset.y
                    )
                    .scaleEffect(zoomLevel)
                }
            }
        }
        .clipped()
        .contentShape(Rectangle())
        .onTapGesture {
            selectedNode = nil
            selectedGroup = nil
        }
    }
}

/// View for a group (collapsed)
struct GroupView: View {
    let group: GraphGroup
    let isSelected: Bool
    let onSelect: () -> Void
    let onToggle: () -> Void

    /// Animation scale based on collapse state and animation progress
    private var animationScale: CGFloat {
        if group.isAnimating {
            // Scale animation during expand/collapse
            let baseScale: CGFloat = group.collapsed ? 0.95 : 1.0
            let targetScale: CGFloat = group.collapsed ? 1.0 : 0.95
            return baseScale + (targetScale - baseScale) * CGFloat(group.animationProgress)
        }
        return 1.0
    }

    /// Rotation for the chevron
    private var chevronRotation: Angle {
        if group.isAnimating {
            let startAngle: Double = group.collapsed ? -90 : 0
            let endAngle: Double = group.collapsed ? 0 : -90
            return .degrees(startAngle + (endAngle - startAngle) * group.animationProgress)
        }
        return group.collapsed ? .degrees(-90) : .degrees(0)
    }

    var body: some View {
        VStack(spacing: 4) {
            // Group icon with expand/collapse chevron
            HStack(spacing: 4) {
                Image(systemName: "chevron.down")
                    .font(.system(size: 10))
                    .rotationEffect(chevronRotation)
                Image(systemName: group.collapsed ? "folder.fill" : "folder.badge.minus")
                    .font(.system(size: 14))
            }

            // Label
            Text(group.label)
                .font(.caption)
                .lineLimit(1)

            // Node count badge
            Text("\(group.nodeCount) items")
                .font(.caption2)
                .foregroundColor(.gray)
        }
        .padding(8)
        .frame(minWidth: 100)
        .background(backgroundForStatus(group.status).opacity(0.8))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(
                    isSelected ? Color.blue : Color.gray.opacity(0.5),
                    style: StrokeStyle(lineWidth: isSelected ? 2 : 1, dash: group.collapsed ? [4, 2] : [])
                )
        )
        .foregroundColor(.white)
        .scaleEffect(animationScale)
        .animation(group.isAnimating ? nil : .easeOut(duration: 0.2), value: isSelected)
        .onTapGesture(count: 2) {
            onToggle()
        }
        .onTapGesture {
            onSelect()
        }
    }

    private func backgroundForStatus(_ status: NodeStatus) -> Color {
        switch status {
        case .pending: return Color(white: 0.25)
        case .running: return Color.blue.opacity(0.6)
        case .success: return Color.green.opacity(0.5)
        case .failed: return Color.red.opacity(0.5)
        case .skipped: return Color.gray.opacity(0.4)
        case .waiting: return Color.orange.opacity(0.5)
        }
    }
}

struct NodeView: View {
    let node: GraphNode
    let isSelected: Bool
    let onSelect: () -> Void

    var body: some View {
        VStack(spacing: 4) {
            // Node icon
            Image(systemName: iconForType(node.type))
                .font(.system(size: 16))

            // Label
            Text(node.label)
                .font(.caption)
                .lineLimit(1)
        }
        .padding(8)
        .frame(minWidth: 80)
        .background(backgroundForStatus(node.status))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isSelected ? Color.blue : Color.clear, lineWidth: 2)
        )
        .foregroundColor(.white)
        .onTapGesture {
            onSelect()
        }
    }

    private func iconForType(_ type: NodeType) -> String {
        switch type {
        case .start: return "play.circle.fill"
        case .end: return "stop.circle.fill"
        case .model: return "brain"
        case .tool: return "wrench.fill"
        case .condition: return "arrow.triangle.branch"
        case .parallel: return "arrow.triangle.split"
        case .join: return "arrow.triangle.merge"
        case .human: return "person.fill"
        case .custom: return "square.fill"
        }
    }

    private func backgroundForStatus(_ status: NodeStatus) -> Color {
        switch status {
        case .pending: return Color(white: 0.3)
        case .running: return Color.blue.opacity(0.8)
        case .success: return Color.green.opacity(0.7)
        case .failed: return Color.red.opacity(0.7)
        case .skipped: return Color.gray.opacity(0.5)
        case .waiting: return Color.orange.opacity(0.7)
        }
    }
}

struct EdgeView: View {
    let edge: GraphEdge
    @ObservedObject var graph: GraphModel
    let zoomLevel: CGFloat
    let panOffset: CGPoint
    let viewportSize: CGSize

    private let collapsedGroupRenderSize = CGSize(width: 100, height: 50)

    var body: some View {
        guard let (start, end) = edgeEndpointsInViewCoordinates() else {
            return AnyView(EmptyView())
        }

        return AnyView(
            Path { path in
                path.move(to: start)
                path.addLine(to: end)
            }
            .stroke(colorForEdgeType(edge.type), lineWidth: max(1, 2 * zoomLevel))
        )
    }

    private func edgeEndpointsInViewCoordinates() -> (CGPoint, CGPoint)? {
        let nodesById = Dictionary(uniqueKeysWithValues: graph.nodes.map { ($0.id, $0) })
        let collapsedGroupIds = Set(graph.groups.filter { $0.collapsed }.map { $0.id })
        let groupPositionsById: [String: Position] = Dictionary(
            uniqueKeysWithValues: graph.groups.compactMap { group in
                guard let position = group.position else { return nil }
                return (group.id, position)
            }
        )

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

        let fromAnchor = fromCollapsedGroupId.flatMap { groupPositionsById[$0] } ?? fromNode.position
        let toAnchor = toCollapsedGroupId.flatMap { groupPositionsById[$0] } ?? toNode.position
        guard let fromAnchor, let toAnchor else { return nil }

        var startGraph = CGPoint(x: fromAnchor.x, y: fromAnchor.y)
        var endGraph = CGPoint(x: toAnchor.x, y: toAnchor.y)

        if fromCollapsedGroupId != nil {
            startGraph = clippedPointOnRectBoundary(center: startGraph, size: collapsedGroupRenderSize, towards: endGraph)
        }
        if toCollapsedGroupId != nil {
            endGraph = clippedPointOnRectBoundary(center: endGraph, size: collapsedGroupRenderSize, towards: startGraph)
        }

        return (toViewCoordinates(startGraph), toViewCoordinates(endGraph))
    }

    private func toViewCoordinates(_ point: CGPoint) -> CGPoint {
        CGPoint(
            x: point.x * zoomLevel + viewportSize.width / 2 + panOffset.x,
            y: point.y * zoomLevel + viewportSize.height / 2 + panOffset.y
        )
    }

    private func clippedPointOnRectBoundary(center: CGPoint, size: CGSize, towards: CGPoint) -> CGPoint {
        let dx = towards.x - center.x
        let dy = towards.y - center.y

        let lengthSquared = dx * dx + dy * dy
        let length = lengthSquared > 1e-6 ? sqrt(lengthSquared) : 1.0
        let dirX = dx / length
        let dirY = dy / length

        let halfWidth = size.width / 2
        let halfHeight = size.height / 2

        let tx = abs(dirX) > 1e-6 ? (halfWidth / abs(dirX)) : CGFloat.greatestFiniteMagnitude
        let ty = abs(dirY) > 1e-6 ? (halfHeight / abs(dirY)) : CGFloat.greatestFiniteMagnitude
        let t = min(tx, ty)

        return CGPoint(x: center.x + dirX * t, y: center.y + dirY * t)
    }

    private func colorForEdgeType(_ type: EdgeType) -> Color {
        switch type {
        case .normal: return Color.gray
        case .conditionalTrue: return Color.green
        case .conditionalFalse: return Color.red
        case .error: return Color.red.opacity(0.8)
        case .fork, .join: return Color.blue
        case .loop: return Color.orange
        }
    }
}

struct NodeDetailView: View {
    let graph: GraphModel
    let nodeId: String

    var node: GraphNode? {
        graph.nodes.first(where: { $0.id == nodeId })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let node = node {
                HStack {
                    Text(node.label)
                        .font(.headline)

                    Spacer()

                    StatusBadge(status: node.status)
                }

                if let description = node.description {
                    Text(description)
                        .font(.caption)
                        .foregroundColor(.gray)
                }

                if let timing = node.timing {
                    HStack {
                        Text("Duration:")
                        Text("\(timing.durationMs ?? 0)ms")
                            .foregroundColor(.gray)
                    }
                    .font(.caption)
                }
            }
        }
        .padding()
        .background(Color(white: 0.15))
        .foregroundColor(.white)
    }
}

struct StatusBadge: View {
    let status: NodeStatus

    var body: some View {
        Text(status.rawValue)
            .font(.caption2)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(colorForStatus(status))
            .cornerRadius(4)
    }

    private func colorForStatus(_ status: NodeStatus) -> Color {
        switch status {
        case .pending: return Color.gray
        case .running: return Color.blue
        case .success: return Color.green
        case .failed: return Color.red
        case .skipped: return Color.gray.opacity(0.5)
        case .waiting: return Color.orange
        }
    }
}

/// Minimap navigation for large graphs
struct MinimapView: View {
    @ObservedObject var graph: GraphModel
    @Binding var zoomLevel: CGFloat
    @Binding var panOffset: CGPoint
    let viewportSize: CGSize
    var selectedNodeId: String?
    var selectedGroupId: String?
    var onGroupSelected: ((String) -> Void)?

    /// Size of the minimap
    private let minimapSize = CGSize(width: 120, height: 80)

    /// Compute the bounding box of all nodes and groups
    private var graphBounds: CGRect {
        guard !graph.nodes.isEmpty || !graph.groups.isEmpty else {
            return CGRect(x: -100, y: -100, width: 200, height: 200)
        }

        var minX: CGFloat = .infinity
        var maxX: CGFloat = -.infinity
        var minY: CGFloat = .infinity
        var maxY: CGFloat = -.infinity

        for node in graph.nodes {
            guard let pos = node.position else { continue }
            minX = min(minX, pos.x)
            maxX = max(maxX, pos.x)
            minY = min(minY, pos.y)
            maxY = max(maxY, pos.y)
        }

        // Include collapsed group positions
        for group in graph.groups.filter({ $0.collapsed }) {
            guard let pos = group.position else { continue }
            minX = min(minX, pos.x)
            maxX = max(maxX, pos.x)
            minY = min(minY, pos.y)
            maxY = max(maxY, pos.y)
        }

        // Add padding around nodes
        let padding: CGFloat = 60
        return CGRect(
            x: minX - padding,
            y: minY - padding,
            width: max(200, maxX - minX + padding * 2),
            height: max(150, maxY - minY + padding * 2)
        )
    }

    /// Scale factor to fit graph in minimap
    private var minimapScale: CGFloat {
        let scaleX = minimapSize.width / graphBounds.width
        let scaleY = minimapSize.height / graphBounds.height
        return min(scaleX, scaleY) * 0.9  // Leave some margin
    }

    /// Convert graph coordinates to minimap coordinates
    private func toMinimapCoords(_ pos: Position) -> CGPoint {
        let scale = minimapScale
        let centerX = minimapSize.width / 2
        let centerY = minimapSize.height / 2
        let graphCenterX = graphBounds.midX
        let graphCenterY = graphBounds.midY

        return CGPoint(
            x: centerX + (pos.x - graphCenterX) * scale,
            y: centerY + (pos.y - graphCenterY) * scale
        )
    }

    /// The viewport rectangle in minimap coordinates
    private var viewportRect: CGRect {
        let scale = minimapScale
        let centerX = minimapSize.width / 2
        let centerY = minimapSize.height / 2
        let graphCenterX = graphBounds.midX
        let graphCenterY = graphBounds.midY

        // Size of viewport in graph coordinates
        let viewportGraphWidth = viewportSize.width / zoomLevel
        let viewportGraphHeight = viewportSize.height / zoomLevel

        // Center of viewport in graph coordinates
        let viewportCenterX = -panOffset.x / zoomLevel
        let viewportCenterY = -panOffset.y / zoomLevel

        // Convert to minimap coordinates
        let minimapX = centerX + (viewportCenterX - graphCenterX) * scale - (viewportGraphWidth * scale) / 2
        let minimapY = centerY + (viewportCenterY - graphCenterY) * scale - (viewportGraphHeight * scale) / 2

        return CGRect(
            x: minimapX,
            y: minimapY,
            width: viewportGraphWidth * scale,
            height: viewportGraphHeight * scale
        )
    }

    var body: some View {
        let nodesById = Dictionary(uniqueKeysWithValues: graph.nodes.map { ($0.id, $0) })
        let collapsedGroupIds = Set(graph.groups.filter { $0.collapsed }.map { $0.id })
        let groupPositionsById: [String: Position] = Dictionary(
            uniqueKeysWithValues: graph.groups.compactMap { group in
                guard let position = group.position else { return nil }
                return (group.id, position)
            }
        )

        ZStack {
            // Background
            RoundedRectangle(cornerRadius: 4)
                .fill(Color(white: 0.12))
                .frame(width: minimapSize.width, height: minimapSize.height)

            // Graph bounds outline
            RoundedRectangle(cornerRadius: 2)
                .stroke(Color.gray.opacity(0.3), lineWidth: 0.5)
                .frame(
                    width: graphBounds.width * minimapScale,
                    height: graphBounds.height * minimapScale
                )

            // Edges
            Canvas { context, size in
                for edge in graph.edges {
                    guard let fromNode = nodesById[edge.from],
                          let toNode = nodesById[edge.to] else { continue }

                    let fromCollapsedGroupId = fromNode.groupId.flatMap { collapsedGroupIds.contains($0) ? $0 : nil }
                    let toCollapsedGroupId = toNode.groupId.flatMap { collapsedGroupIds.contains($0) ? $0 : nil }

                    // Skip edges entirely within the same collapsed group
                    if let fromGroupId = fromCollapsedGroupId,
                       let toGroupId = toCollapsedGroupId,
                       fromGroupId == toGroupId {
                        continue
                    }

                    let fromPos = fromCollapsedGroupId.flatMap { groupPositionsById[$0] } ?? fromNode.position
                    let toPos = toCollapsedGroupId.flatMap { groupPositionsById[$0] } ?? toNode.position

                    guard let fromPos, let toPos else { continue }

                    let from = toMinimapCoords(fromPos)
                    let to = toMinimapCoords(toPos)

                    var path = Path()
                    path.move(to: from)
                    path.addLine(to: to)

                    context.stroke(path, with: .color(Color.gray.opacity(0.4)), lineWidth: 0.5)
                }
            }
            .frame(width: minimapSize.width, height: minimapSize.height)

            // Collapsed groups (render as rounded rectangles)
            ForEach(graph.groups.filter { $0.collapsed }) { group in
                if let pos = group.position {
                    let minimapPos = toMinimapCoords(pos)
                    let isSelected = group.id == selectedGroupId
                    RoundedRectangle(cornerRadius: 2)
                        .fill(colorForStatus(group.status).opacity(0.7))
                        .frame(width: isSelected ? 10 : 8, height: isSelected ? 6 : 5)
                        .overlay(
                            RoundedRectangle(cornerRadius: 2)
                                .stroke(
                                    isSelected ? Color.white : Color.white.opacity(0.5),
                                    style: StrokeStyle(lineWidth: isSelected ? 1.5 : 0.5, dash: [2, 1])
                                )
                        )
                        .position(minimapPos)
                        .onTapGesture {
                            onGroupSelected?(group.id)
                        }
                }
            }

            // Nodes (skip nodes in collapsed groups)
            ForEach(graph.nodes.filter { node in
                guard let groupId = node.groupId else { return true }
                return !graph.isGroupCollapsed(groupId)
            }) { node in
                if let pos = node.position {
                    let minimapPos = toMinimapCoords(pos)
                    let isSelected = node.id == selectedNodeId
                    Circle()
                        .fill(colorForStatus(node.status))
                        .frame(width: isSelected ? 6 : 4, height: isSelected ? 6 : 4)
                        .overlay(
                            Circle()
                                .stroke(Color.white, lineWidth: isSelected ? 1.5 : 0)
                        )
                        .position(minimapPos)
                }
            }

            // Viewport indicator
            Rectangle()
                .stroke(Color.white.opacity(0.7), lineWidth: 1)
                .background(Color.white.opacity(0.1))
                .frame(width: max(8, viewportRect.width), height: max(6, viewportRect.height))
                .position(
                    x: viewportRect.midX,
                    y: viewportRect.midY
                )
        }
        .frame(width: minimapSize.width, height: minimapSize.height)
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .overlay(
            RoundedRectangle(cornerRadius: 4)
                .stroke(Color.gray.opacity(0.4), lineWidth: 1)
        )
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    navigateToMinimapLocation(value.location)
                }
        )
    }

    private func navigateToMinimapLocation(_ location: CGPoint) {
        let scale = minimapScale
        let centerX = minimapSize.width / 2
        let centerY = minimapSize.height / 2
        let graphCenterX = graphBounds.midX
        let graphCenterY = graphBounds.midY

        // Convert minimap location to graph coordinates
        let graphX = (location.x - centerX) / scale + graphCenterX
        let graphY = (location.y - centerY) / scale + graphCenterY

        // Update pan offset to center on this location
        panOffset = CGPoint(
            x: -graphX * zoomLevel,
            y: -graphY * zoomLevel
        )
    }

    private func colorForStatus(_ status: NodeStatus) -> Color {
        switch status {
        case .pending: return Color.gray
        case .running: return Color.blue
        case .success: return Color.green
        case .failed: return Color.red
        case .skipped: return Color.gray.opacity(0.5)
        case .waiting: return Color.orange
        }
    }
}

#Preview {
    GraphPanelView()
        .environmentObject(AppState())
        .frame(width: 400, height: 600)
}
