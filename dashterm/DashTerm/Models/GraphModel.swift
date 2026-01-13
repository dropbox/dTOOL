//
//  GraphModel.swift
//  DashTerm
//
//  Data models for computation graph visualization
//

import Foundation
import QuartzCore

/// A computation graph for visualization
class GraphModel: ObservableObject {
    @Published var name: String
    @Published var nodes: [GraphNode]
    @Published var edges: [GraphEdge]
    @Published var groups: [GraphGroup]

    private var graphHandle: OpaquePointer?

    init(name: String = "Unnamed Graph") {
        self.name = name
        self.nodes = []
        self.edges = []
        self.groups = []

        // Create graph via FFI
        name.withCString { namePtr in
            graphHandle = dashterm_graph_new(namePtr)
        }
    }

    deinit {
        if let handle = graphHandle {
            dashterm_graph_free(handle)
        }
    }

    /// Add a node to the graph
    func addNode(_ node: GraphNode) {
        guard let handle = graphHandle else { return }

        let typeValue: UInt32 = {
            switch node.type {
            case .start: return 0
            case .end: return 1
            case .model: return 2
            case .tool: return 3
            case .condition: return 4
            case .parallel: return 5
            case .join: return 6
            case .human: return 7
            case .custom: return 8
            }
        }()

        node.id.withCString { idPtr in
            node.label.withCString { labelPtr in
                _ = dashterm_graph_add_node(handle, idPtr, labelPtr, typeValue)
            }
        }

        nodes.append(node)

        // Auto-compute layout after adding node
        computeLayout()
    }

    /// Add an edge to the graph
    func addEdge(_ edge: GraphEdge) {
        guard let handle = graphHandle else { return }

        let typeValue: UInt32 = {
            switch edge.type {
            case .normal: return 0
            case .conditionalTrue: return 1
            case .conditionalFalse: return 2
            case .error: return 3
            case .fork: return 4
            case .join: return 5
            case .loop: return 6
            }
        }()

        edge.from.withCString { fromPtr in
            edge.to.withCString { toPtr in
                _ = dashterm_graph_add_edge(handle, fromPtr, toPtr, typeValue)
            }
        }

        edges.append(edge)

        // Re-compute layout after adding edge (edges affect topological ordering)
        computeLayout()
    }

    /// Compute layout positions for all nodes using a hierarchical algorithm
    func computeLayout() {
        guard let handle = graphHandle else { return }

        // Call Rust layout algorithm
        dashterm_graph_compute_layout(handle)

        // Refresh node positions from Rust
        refreshLayout()
    }

    /// Load layout data from the Rust graph
    func refreshLayout() {
        guard let handle = graphHandle else { return }
        guard let jsonPtr = dashterm_graph_get_layout_json(handle) else { return }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)

        do {
            let layout = try JSONDecoder().decode(GraphLayoutData.self, from: jsonString.data(using: .utf8)!)

            // Update node positions and group assignments from layout data
            for layoutNode in layout.nodes {
                if let index = nodes.firstIndex(where: { $0.id == layoutNode.id }) {
                    if let position = layoutNode.position, position.count >= 2 {
                        nodes[index].position = Position(x: position[0], y: position[1])
                    }
                    nodes[index].groupId = layoutNode.groupId
                }
            }

            // Update groups from layout data
            if let layoutGroups = layout.groups {
                groups = layoutGroups.map { layoutGroup in
                    let status: NodeStatus = {
                        switch layoutGroup.status {
                        case "Running": return .running
                        case "Success": return .success
                        case "Failed": return .failed
                        case "Skipped": return .skipped
                        case "Waiting": return .waiting
                        default: return .pending
                        }
                    }()
                    var position: Position? = nil
                    if let pos = layoutGroup.position, pos.count >= 2 {
                        position = Position(x: pos[0], y: pos[1])
                    }
                    return GraphGroup(
                        id: layoutGroup.id,
                        label: layoutGroup.label,
                        collapsed: layoutGroup.collapsed,
                        nodeCount: layoutGroup.nodeCount,
                        status: status,
                        position: position
                    )
                }
            }
        } catch {
            print("Failed to decode graph layout: \(error)")
        }
    }

    /// Update node status
    func updateNodeStatus(_ nodeId: String, status: NodeStatus) {
        if let index = nodes.firstIndex(where: { $0.id == nodeId }) {
            nodes[index].status = status
        }

        // Update any group containing this node
        if let node = nodes.first(where: { $0.id == nodeId }),
           let groupId = node.groupId {
            updateGroupStatus(groupId)
        }
    }

    // MARK: - Group Management

    /// Create a new group
    func createGroup(id: String, label: String) {
        guard let handle = graphHandle else { return }

        id.withCString { idPtr in
            label.withCString { labelPtr in
                let resultPtr = dashterm_graph_create_group(handle, idPtr, labelPtr)
                if resultPtr != nil {
                    dashterm_string_free(resultPtr)
                }
            }
        }

        let group = GraphGroup(id: id, label: label, collapsed: false, nodeCount: 0, status: .pending)
        groups.append(group)
    }

    /// Add a node to a group
    func addNodeToGroup(nodeId: String, groupId: String) {
        guard let handle = graphHandle else { return }

        nodeId.withCString { nodeIdPtr in
            groupId.withCString { groupIdPtr in
                _ = dashterm_graph_add_node_to_group(handle, nodeIdPtr, groupIdPtr)
            }
        }

        // Update local node reference
        if let index = nodes.firstIndex(where: { $0.id == nodeId }) {
            nodes[index].groupId = groupId
        }

        updateGroupStatus(groupId)
    }

    /// Toggle group collapsed state
    func toggleGroup(_ groupId: String) {
        guard let handle = graphHandle else { return }

        var newCollapsed = false
        groupId.withCString { groupIdPtr in
            newCollapsed = dashterm_graph_toggle_group(handle, groupIdPtr)
        }

        if let index = groups.firstIndex(where: { $0.id == groupId }) {
            groups[index].collapsed = newCollapsed
        }
    }

    /// Toggle group with animation
    func toggleGroupAnimated(_ groupId: String, duration: TimeInterval = 0.3) {
        guard let index = groups.firstIndex(where: { $0.id == groupId }) else { return }

        // Start animation
        groups[index].isAnimating = true
        groups[index].animationProgress = 0.0

        // Toggle the collapsed state
        toggleGroup(groupId)

        // Animate progress
        let startTime = CACurrentMediaTime()
        let timer = Timer.scheduledTimer(withTimeInterval: 1.0/60.0, repeats: true) { [weak self] timer in
            guard let self = self else {
                timer.invalidate()
                return
            }

            let elapsed = CACurrentMediaTime() - startTime
            let progress = min(elapsed / duration, 1.0)

            // Update on main thread
            DispatchQueue.main.async { [weak self] in
                guard let self = self,
                      let currentIndex = self.groups.firstIndex(where: { $0.id == groupId }) else {
                    timer.invalidate()
                    return
                }

                // Ease-out cubic
                let easedProgress = 1.0 - pow(1.0 - progress, 3.0)
                self.groups[currentIndex].animationProgress = easedProgress

                if progress >= 1.0 {
                    self.groups[currentIndex].isAnimating = false
                    timer.invalidate()
                }
            }
        }
        RunLoop.main.add(timer, forMode: .common)
    }

    /// Update group status from child nodes
    func updateGroupStatus(_ groupId: String) {
        guard let handle = graphHandle else { return }

        groupId.withCString { groupIdPtr in
            dashterm_graph_update_group_status(handle, groupIdPtr)
        }

        // Refresh group status from layout
        refreshLayout()
    }

    /// Automatically group tool sequences
    func autoGroupTools(minTools: Int = 2) -> Int {
        guard let handle = graphHandle else { return 0 }

        let count = dashterm_graph_auto_group_tools(handle, UInt32(minTools))
        refreshLayout()
        return Int(count)
    }

    /// Get nodes in a specific group
    func nodesInGroup(_ groupId: String) -> [GraphNode] {
        return nodes.filter { $0.groupId == groupId }
    }

    /// Check if a group is collapsed
    func isGroupCollapsed(_ groupId: String) -> Bool {
        return groups.first(where: { $0.id == groupId })?.collapsed ?? false
    }

    /// Update group position (for drag-to-reposition)
    func setGroupPosition(_ groupId: String, position: Position) {
        if let index = groups.firstIndex(where: { $0.id == groupId }) {
            groups[index].position = position
        }
    }

    /// Compute zoom and pan values to fit all nodes within the given viewport size.
    /// Returns (zoom, panX, panY) values that can be applied to the view.
    func computeZoomToFit(viewportWidth: CGFloat, viewportHeight: CGFloat) -> (zoom: CGFloat, panX: CGFloat, panY: CGFloat) {
        guard let handle = graphHandle else { return (1.0, 0.0, 0.0) }

        let result = dashterm_graph_compute_zoom_to_fit(handle, Float(viewportWidth), Float(viewportHeight))
        return (CGFloat(result.zoom), CGFloat(result.pan_x), CGFloat(result.pan_y))
    }

    /// Animate node position changes smoothly.
    /// Call this method when you want to smoothly transition nodes to new positions.
    func animateToNewPositions(duration: TimeInterval = 0.3) {
        guard let handle = graphHandle else { return }
        guard let jsonPtr = dashterm_graph_get_layout_json(handle) else { return }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)

        do {
            let layout = try JSONDecoder().decode(GraphLayoutData.self, from: jsonString.data(using: .utf8)!)

            // Store target positions
            var targetPositions: [String: Position] = [:]
            for layoutNode in layout.nodes {
                if let position = layoutNode.position, position.count >= 2 {
                    targetPositions[layoutNode.id] = Position(x: position[0], y: position[1])
                }
            }

            // Apply animations on main thread
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                for (index, node) in self.nodes.enumerated() {
                    if let targetPos = targetPositions[node.id] {
                        self.nodes[index].targetPosition = targetPos
                        self.nodes[index].animationProgress = 0.0
                    }
                }
            }
        } catch {
            print("Failed to decode graph layout for animation: \(error)")
        }
    }
}

/// A node in the graph
struct GraphNode: Identifiable {
    let id: String
    var label: String
    var type: NodeType
    var status: NodeStatus
    var position: Position?
    var description: String?
    var timing: NodeTiming?
    var groupId: String?

    // Animation properties
    var targetPosition: Position?
    var animationProgress: Double = 1.0  // 0.0 = at start, 1.0 = at target

    /// Get the interpolated position for smooth animation
    var animatedPosition: Position? {
        guard let current = position else { return nil }
        guard let target = targetPosition, animationProgress < 1.0 else { return current }

        let t = animationProgress
        let eased = t * t * (3 - 2 * t)  // Smoothstep easing

        return Position(
            x: current.x + (target.x - current.x) * eased,
            y: current.y + (target.y - current.y) * eased
        )
    }
}

/// A directed edge between nodes
struct GraphEdge: Identifiable {
    var id: String { "\(from)->\(to)" }
    let from: String
    let to: String
    var type: EdgeType
    var label: String?
}

/// Position for layout
struct Position: Codable {
    var x: CGFloat
    var y: CGFloat
}

/// Node types matching Rust NodeType
enum NodeType: String, Codable {
    case start = "Start"
    case end = "End"
    case model = "Model"
    case tool = "Tool"
    case condition = "Condition"
    case parallel = "Parallel"
    case join = "Join"
    case human = "Human"
    case custom = "Custom"
}

/// Node execution status
enum NodeStatus: String, Codable {
    case pending = "Pending"
    case running = "Running"
    case success = "Success"
    case failed = "Failed"
    case skipped = "Skipped"
    case waiting = "Waiting"
}

/// Edge types matching Rust EdgeType
enum EdgeType: String, Codable {
    case normal = "Normal"
    case conditionalTrue = "ConditionalTrue"
    case conditionalFalse = "ConditionalFalse"
    case error = "Error"
    case fork = "Fork"
    case join = "Join"
    case loop = "Loop"
}

/// Node timing information
struct NodeTiming: Codable {
    let startedAt: UInt64
    let completedAt: UInt64?
    let durationMs: UInt64?

    enum CodingKeys: String, CodingKey {
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case durationMs = "duration_ms"
    }
}

/// A collapsible group of nodes
struct GraphGroup: Identifiable {
    let id: String
    var label: String
    var collapsed: Bool
    var nodeCount: Int
    var status: NodeStatus
    var position: Position?

    // Animation properties
    var animationProgress: Double = 1.0  // 0.0 = animation start, 1.0 = animation complete
    var isAnimating: Bool = false
}

/// Layout data from Rust
struct GraphLayoutData: Codable {
    let nodes: [NodeLayoutData]
    let edges: [EdgeLayoutData]
    let groups: [GroupLayoutData]?
}

struct NodeLayoutData: Codable {
    let id: String
    let label: String
    let nodeType: String
    let status: String
    let position: [CGFloat]?
    let groupId: String?

    enum CodingKeys: String, CodingKey {
        case id, label
        case nodeType = "node_type"
        case status, position
        case groupId = "group_id"
    }
}

struct EdgeLayoutData: Codable {
    let from: String
    let to: String
    let edgeType: String
    let label: String?

    enum CodingKeys: String, CodingKey {
        case from, to
        case edgeType = "edge_type"
        case label
    }
}

struct GroupLayoutData: Codable {
    let id: String
    let label: String
    let collapsed: Bool
    let nodeCount: Int
    let status: String
    let position: [CGFloat]?

    enum CodingKeys: String, CodingKey {
        case id, label, collapsed
        case nodeCount = "node_count"
        case status, position
    }
}
