//
//  AgentEventBridge.swift
//  DashTerm
//
//  Bridges terminal output to graph visualization by detecting and processing
//  agent events from terminal output.
//

import Foundation
import Combine

/// Bridges agent events from terminal output to graph updates
@MainActor
class AgentEventBridge: ObservableObject {
    /// The terminal session to monitor
    private weak var terminalSession: TerminalSession?

    /// The graph model to update
    private weak var graphModel: GraphModel?

    /// Whether agent parsing is currently enabled
    @Published private(set) var isEnabled: Bool = false

    /// Currently active node ID (if any)
    @Published private(set) var activeNodeId: String?

    /// Currently active tool name (if any)
    @Published private(set) var activeToolName: String?

    /// Recent events for debugging/display
    @Published private(set) var recentEvents: [AgentEvent] = []

    /// Maximum number of recent events to keep
    private let maxRecentEvents = 50

    /// Cancellables for subscriptions
    private var cancellables = Set<AnyCancellable>()

    /// Timer for polling agent events
    private var pollTimer: Timer?

    /// Track consecutive tool nodes for auto-grouping
    private var pendingToolCount: Int = 0

    /// Minimum tools to trigger auto-grouping
    private let autoGroupMinTools: Int = 3

    /// Debounce timer for auto-grouping (triggers after tool sequence ends)
    private var autoGroupTimer: Timer?

    init() {}

    /// Connect the bridge to a terminal session and graph model
    func connect(terminal: TerminalSession, graph: GraphModel) {
        self.terminalSession = terminal
        self.graphModel = graph

        // Subscribe to terminal updates
        terminal.updatePublisher
            .receive(on: DispatchQueue.main)
            .sink { [weak self] in
                self?.pollAgentEvents()
            }
            .store(in: &cancellables)
    }

    /// Disconnect from current terminal and graph
    func disconnect() {
        cancellables.removeAll()
        pollTimer?.invalidate()
        pollTimer = nil
        autoGroupTimer?.invalidate()
        autoGroupTimer = nil
        terminalSession = nil
        graphModel = nil
        disable()
    }

    /// Enable agent parsing
    func enable() {
        guard let session = terminalSession else { return }

        // Enable parsing in the Rust terminal
        session.enableAgentParsing()
        isEnabled = true

        // Start polling for events
        startPolling()
    }

    /// Disable agent parsing
    func disable() {
        if let session = terminalSession {
            session.disableAgentParsing()
        }
        isEnabled = false
        stopPolling()
        activeNodeId = nil
        activeToolName = nil
    }

    /// Toggle agent parsing
    func toggle() {
        if isEnabled {
            disable()
        } else {
            enable()
        }
    }

    /// Clear all state
    func clear() {
        recentEvents.removeAll()
        activeNodeId = nil
        activeToolName = nil
        pendingToolCount = 0
        autoGroupTimer?.invalidate()
        autoGroupTimer = nil
    }

    // MARK: - Private Methods

    private func startPolling() {
        stopPolling()
        // Poll at 60Hz to match rendering
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.0/60.0, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.pollAgentEvents()
            }
        }
    }

    private func stopPolling() {
        pollTimer?.invalidate()
        pollTimer = nil
    }

    private func pollAgentEvents() {
        guard isEnabled else { return }
        guard let session = terminalSession else { return }

        // Get agent events from terminal
        let events = session.getAgentEvents()

        for event in events {
            processEvent(event)
        }

        // Update active state
        activeNodeId = session.getActiveAgentNode()
        activeToolName = session.getActiveAgentTool()
    }

    private func processEvent(_ event: AgentEvent) {
        // Add to recent events
        recentEvents.append(event)
        if recentEvents.count > maxRecentEvents {
            recentEvents.removeFirst()
        }

        // Update graph based on event
        guard let graph = graphModel else { return }

        switch event {
        case .flowStart(let data):
            // Create a start node for the flow
            let node = GraphNode(
                id: "flow_start_\(data.name)",
                label: data.name,
                type: .start,
                status: .running
            )
            graph.addNode(node)

        case .flowComplete(let data):
            // Update the start node status
            let nodeId = "flow_start_\(data.name)"
            graph.updateNodeStatus(nodeId, status: data.success ? .success : .failed)

        case .nodeStart(let data):
            // Add or update node
            let nodeType = mapNodeType(data.nodeType)
            let node = GraphNode(
                id: data.nodeId,
                label: data.label,
                type: nodeType,
                status: .running
            )
            graph.addNode(node)

        case .nodeComplete(let data):
            let status = mapStatus(data.status)
            graph.updateNodeStatus(data.nodeId, status: status)

        case .toolUse(let data):
            // Add a tool node
            let nodeId = data.toolId ?? "tool_\(data.toolName)"
            let node = GraphNode(
                id: nodeId,
                label: data.toolName,
                type: .tool,
                status: .running
            )
            graph.addNode(node)

            // Connect to active node if any
            if let activeNode = activeNodeId {
                let edge = GraphEdge(from: activeNode, to: nodeId, type: .normal)
                graph.addEdge(edge)
            }

            // Track tool sequence for auto-grouping
            pendingToolCount += 1
            scheduleAutoGrouping()

        case .toolResult(let data):
            let nodeId = data.toolId ?? "tool_\(data.toolName)"
            graph.updateNodeStatus(nodeId, status: data.success ? .success : .failed)

            // Reschedule auto-grouping after tool completes
            scheduleAutoGrouping()

        case .thinking:
            // Update current node to show thinking state
            if let nodeId = activeNodeId {
                graph.updateNodeStatus(nodeId, status: .running)
            }

        case .output:
            // Currently not updating graph for output
            break

        case .error(let data):
            // Mark relevant node as failed
            if let nodeId = data.nodeId {
                graph.updateNodeStatus(nodeId, status: .failed)
            }
        }
    }

    private func mapNodeType(_ agentType: AgentNodeType) -> NodeType {
        switch agentType {
        case .start: return .start
        case .end: return .end
        case .model: return .model
        case .tool: return .tool
        case .condition: return .condition
        case .parallel: return .parallel
        case .join: return .join
        case .human: return .human
        case .custom: return .custom
        }
    }

    private func mapStatus(_ agentStatus: AgentStatus) -> NodeStatus {
        switch agentStatus {
        case .success: return .success
        case .failed: return .failed
        case .skipped: return .skipped
        }
    }

    /// Schedule auto-grouping after a debounce delay
    private func scheduleAutoGrouping() {
        autoGroupTimer?.invalidate()

        // Only schedule if we have enough tools
        guard pendingToolCount >= autoGroupMinTools else { return }

        // Debounce: wait 500ms after last tool event before grouping
        autoGroupTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.performAutoGrouping()
            }
        }
    }

    /// Perform auto-grouping of tool sequences
    private func performAutoGrouping() {
        guard let graph = graphModel else { return }

        let groupsCreated = graph.autoGroupTools(minTools: autoGroupMinTools)
        if groupsCreated > 0 {
            // Reset tool count after grouping
            pendingToolCount = 0
        }
    }
}

// MARK: - Agent Event Types (matching Rust types)

/// Events detected from agent output
enum AgentEvent: Codable {
    case flowStart(FlowStartData)
    case flowComplete(FlowCompleteData)
    case nodeStart(NodeStartData)
    case nodeComplete(NodeCompleteData)
    case toolUse(ToolUseData)
    case toolResult(ToolResultData)
    case thinking(ThinkingData)
    case output(OutputData)
    case error(ErrorData)

    struct FlowStartData: Codable {
        let name: String
    }

    struct FlowCompleteData: Codable {
        let name: String
        let success: Bool
    }

    struct NodeStartData: Codable {
        let nodeId: String
        let label: String
        let nodeType: AgentNodeType

        enum CodingKeys: String, CodingKey {
            case nodeId = "node_id"
            case label
            case nodeType = "node_type"
        }
    }

    struct NodeCompleteData: Codable {
        let nodeId: String
        let status: AgentStatus
        let durationMs: UInt64?

        enum CodingKeys: String, CodingKey {
            case nodeId = "node_id"
            case status
            case durationMs = "duration_ms"
        }
    }

    struct ToolUseData: Codable {
        let toolName: String
        let toolId: String?
        let inputPreview: String?

        enum CodingKeys: String, CodingKey {
            case toolName = "tool_name"
            case toolId = "tool_id"
            case inputPreview = "input_preview"
        }
    }

    struct ToolResultData: Codable {
        let toolName: String
        let toolId: String?
        let success: Bool

        enum CodingKeys: String, CodingKey {
            case toolName = "tool_name"
            case toolId = "tool_id"
            case success
        }
    }

    struct ThinkingData: Codable {
        let preview: String?
    }

    struct OutputData: Codable {
        let content: String
    }

    struct ErrorData: Codable {
        let message: String
        let nodeId: String?

        enum CodingKeys: String, CodingKey {
            case message
            case nodeId = "node_id"
        }
    }

    // Custom decoding for tagged enum
    enum CodingKeys: String, CodingKey {
        case type, data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "FlowStart":
            let data = try container.decode(FlowStartData.self, forKey: .data)
            self = .flowStart(data)
        case "FlowComplete":
            let data = try container.decode(FlowCompleteData.self, forKey: .data)
            self = .flowComplete(data)
        case "NodeStart":
            let data = try container.decode(NodeStartData.self, forKey: .data)
            self = .nodeStart(data)
        case "NodeComplete":
            let data = try container.decode(NodeCompleteData.self, forKey: .data)
            self = .nodeComplete(data)
        case "ToolUse":
            let data = try container.decode(ToolUseData.self, forKey: .data)
            self = .toolUse(data)
        case "ToolResult":
            let data = try container.decode(ToolResultData.self, forKey: .data)
            self = .toolResult(data)
        case "Thinking":
            let data = try container.decode(ThinkingData.self, forKey: .data)
            self = .thinking(data)
        case "Output":
            let data = try container.decode(OutputData.self, forKey: .data)
            self = .output(data)
        case "Error":
            let data = try container.decode(ErrorData.self, forKey: .data)
            self = .error(data)
        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown event type: \(type)")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        switch self {
        case .flowStart(let data):
            try container.encode("FlowStart", forKey: .type)
            try container.encode(data, forKey: .data)
        case .flowComplete(let data):
            try container.encode("FlowComplete", forKey: .type)
            try container.encode(data, forKey: .data)
        case .nodeStart(let data):
            try container.encode("NodeStart", forKey: .type)
            try container.encode(data, forKey: .data)
        case .nodeComplete(let data):
            try container.encode("NodeComplete", forKey: .type)
            try container.encode(data, forKey: .data)
        case .toolUse(let data):
            try container.encode("ToolUse", forKey: .type)
            try container.encode(data, forKey: .data)
        case .toolResult(let data):
            try container.encode("ToolResult", forKey: .type)
            try container.encode(data, forKey: .data)
        case .thinking(let data):
            try container.encode("Thinking", forKey: .type)
            try container.encode(data, forKey: .data)
        case .output(let data):
            try container.encode("Output", forKey: .type)
            try container.encode(data, forKey: .data)
        case .error(let data):
            try container.encode("Error", forKey: .type)
            try container.encode(data, forKey: .data)
        }
    }
}

/// Agent node types (matching Rust AgentNodeType)
enum AgentNodeType: String, Codable {
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

/// Agent execution status (matching Rust AgentStatus)
enum AgentStatus: String, Codable {
    case success = "Success"
    case failed = "Failed"
    case skipped = "Skipped"
}

