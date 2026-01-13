//
//  DashTermApp.swift
//  DashTerm
//
//  A native macOS terminal with visualization capabilities
//

import SwiftUI

@main
struct DashTermApp: App {
    @StateObject private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Terminal Tab") {
                    appState.createNewTerminal()
                }
                .keyboardShortcut("t", modifiers: .command)

                Button("New Window") {
                    // TODO: Implement new window
                }
                .keyboardShortcut("n", modifiers: .command)
            }

            CommandMenu("View") {
                Button("Toggle Graph Panel") {
                    appState.showGraphPanel.toggle()
                }
                .keyboardShortcut("g", modifiers: [.command, .shift])

                Button("Toggle Split View") {
                    appState.splitViewMode.toggle()
                }
                .keyboardShortcut("d", modifiers: [.command, .shift])

                Divider()

                Button(appState.agentParsingEnabled ? "Disable Agent Parsing" : "Enable Agent Parsing") {
                    appState.toggleAgentParsing()
                }
                .keyboardShortcut("a", modifiers: [.command, .shift])

                Button("Clear Agent Graph") {
                    appState.clearAgentGraph()
                }
                .keyboardShortcut("k", modifiers: [.command, .shift])

                Divider()

                Button("Load Demo Graph") {
                    appState.loadDemoGraph()
                }
                .keyboardShortcut("l", modifiers: [.command, .shift])
            }
        }

        Settings {
            SettingsView()
                .environmentObject(appState)
        }
    }
}

/// Global application state
@MainActor
class AppState: ObservableObject {
    @Published var terminals: [TerminalSession] = []
    @Published var activeTerminalId: UUID?
    @Published var showGraphPanel: Bool = false
    @Published var splitViewMode: Bool = false
    @Published var currentGraph: GraphModel?
    @Published var agentParsingEnabled: Bool = false

    /// Bridge for connecting terminal output to graph visualization
    let agentEventBridge = AgentEventBridge()

    init() {
        // Create initial terminal
        createNewTerminal()
    }

    func createNewTerminal() {
        let session = TerminalSession()
        terminals.append(session)
        activeTerminalId = session.id

        // If this is the first terminal and we have a graph, connect the bridge
        if let graph = currentGraph {
            agentEventBridge.connect(terminal: session, graph: graph)
        }
    }

    /// Enable agent parsing for the active terminal
    func enableAgentParsing() {
        guard let activeId = activeTerminalId,
              let session = terminals.first(where: { $0.id == activeId }) else { return }

        // Create a new graph for live updates if we don't have one
        if currentGraph == nil {
            currentGraph = GraphModel(name: "Live Agent Flow")
        }

        // Connect the bridge
        agentEventBridge.connect(terminal: session, graph: currentGraph!)
        agentEventBridge.enable()
        agentParsingEnabled = true
        showGraphPanel = true
    }

    /// Disable agent parsing
    func disableAgentParsing() {
        agentEventBridge.disable()
        agentParsingEnabled = false
    }

    /// Toggle agent parsing
    func toggleAgentParsing() {
        if agentParsingEnabled {
            disableAgentParsing()
        } else {
            enableAgentParsing()
        }
    }

    /// Clear the current agent graph
    func clearAgentGraph() {
        currentGraph = GraphModel(name: "Live Agent Flow")
        if agentParsingEnabled, let activeId = activeTerminalId,
           let session = terminals.first(where: { $0.id == activeId }) {
            agentEventBridge.connect(terminal: session, graph: currentGraph!)
        }
    }

    func closeTerminal(_ id: UUID) {
        terminals.removeAll { $0.id == id }
        if activeTerminalId == id {
            activeTerminalId = terminals.first?.id
        }
    }

    /// Load a demo graph for testing visualization
    func loadDemoGraph() {
        let graph = GraphModel(name: "Demo Agent Flow")

        // Create sample nodes with positions
        let nodes = [
            GraphNode(id: "start", label: "Start", type: .start, status: .success,
                     position: Position(x: 0, y: -150)),
            GraphNode(id: "analyze", label: "Analyze Input", type: .model, status: .success,
                     position: Position(x: 0, y: -75)),
            GraphNode(id: "condition", label: "Has Tools?", type: .condition, status: .success,
                     position: Position(x: 0, y: 0)),
            GraphNode(id: "tool_call", label: "Call Tool", type: .tool, status: .running,
                     position: Position(x: -100, y: 75)),
            GraphNode(id: "generate", label: "Generate Response", type: .model, status: .pending,
                     position: Position(x: 100, y: 75)),
            GraphNode(id: "end", label: "End", type: .end, status: .pending,
                     position: Position(x: 0, y: 150))
        ]

        for node in nodes {
            graph.addNode(node)
        }

        // Create edges
        graph.addEdge(GraphEdge(from: "start", to: "analyze", type: .normal))
        graph.addEdge(GraphEdge(from: "analyze", to: "condition", type: .normal))
        graph.addEdge(GraphEdge(from: "condition", to: "tool_call", type: .conditionalTrue, label: "Yes"))
        graph.addEdge(GraphEdge(from: "condition", to: "generate", type: .conditionalFalse, label: "No"))
        graph.addEdge(GraphEdge(from: "tool_call", to: "generate", type: .normal))
        graph.addEdge(GraphEdge(from: "generate", to: "end", type: .normal))

        currentGraph = graph
        showGraphPanel = true
    }
}
