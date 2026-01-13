/*
 * UIBridgeDemoView.swift - UI Bridge lifecycle demo
 *
 * Copyright 2024-2025 Dropbox, Inc.
 * Licensed under Apache 2.0
 *
 * This view demonstrates the DTermUIBridge lifecycle:
 * - Creating and destroying terminals
 * - Sending input events
 * - Resizing terminals
 * - State queries and consistency checks
 */

import SwiftUI
import DTermCore

/// Demo view for UI Bridge lifecycle testing.
struct UIBridgeDemoView: View {
    @StateObject private var viewModel = UIBridgeDemoViewModel()

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("UI Bridge Demo")
                    .font(.headline)
                Spacer()
                Text("dterm-core v\(dtermVersion())")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(Color(white: 0.15))

            // State display
            VStack(alignment: .leading, spacing: 8) {
                // Bridge state
                HStack {
                    Text("Bridge State:")
                        .fontWeight(.medium)
                    Text(viewModel.bridgeStateDescription)
                        .foregroundColor(viewModel.stateColor)
                    Spacer()
                    Circle()
                        .fill(viewModel.isConsistent ? Color.green : Color.red)
                        .frame(width: 12, height: 12)
                    Text(viewModel.isConsistent ? "Consistent" : "Inconsistent")
                        .font(.caption)
                }

                // Counters
                HStack(spacing: 16) {
                    Label("\(viewModel.pendingCount)", systemImage: "tray")
                        .help("Pending events")
                    Label("\(viewModel.callbackCount)", systemImage: "arrow.turn.up.right")
                        .help("Pending callbacks")
                    Label("\(viewModel.renderPendingCount)", systemImage: "paintbrush")
                        .help("Pending renders")
                }
                .font(.system(.body, design: .monospaced))

                Divider()

                // Terminal states
                Text("Terminals")
                    .fontWeight(.medium)

                ForEach(0..<4, id: \.self) { id in
                    HStack {
                        Text("Terminal \(id):")
                        Text(viewModel.terminalStateDescription(id: UInt32(id)))
                            .foregroundColor(viewModel.terminalStateColor(id: UInt32(id)))
                        Spacer()
                    }
                    .font(.system(.body, design: .monospaced))
                }
            }
            .padding()
            .background(Color(white: 0.05))

            // Event log
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(viewModel.eventLog.indices, id: \.self) { index in
                        Text(viewModel.eventLog[index])
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
            .background(Color.black)
            .frame(minHeight: 150)

            // Controls
            VStack(spacing: 8) {
                // Terminal lifecycle
                HStack(spacing: 12) {
                    Button("Create T0") {
                        viewModel.createTerminal(id: 0)
                    }
                    .buttonStyle(.bordered)

                    Button("Destroy T0") {
                        viewModel.destroyTerminal(id: 0)
                    }
                    .buttonStyle(.bordered)

                    Button("Create T1") {
                        viewModel.createTerminal(id: 1)
                    }
                    .buttonStyle(.bordered)

                    Button("Destroy T1") {
                        viewModel.destroyTerminal(id: 1)
                    }
                    .buttonStyle(.bordered)
                }

                // Events
                HStack(spacing: 12) {
                    Button("Input T0") {
                        viewModel.sendInput(terminalId: 0, text: "hello")
                    }
                    .buttonStyle(.bordered)

                    Button("Resize T0") {
                        viewModel.resize(terminalId: 0, rows: 30, cols: 100)
                    }
                    .buttonStyle(.bordered)

                    Button("Shutdown") {
                        viewModel.shutdown()
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                }

                // Utility
                HStack(spacing: 12) {
                    Button("Run Lifecycle Test") {
                        viewModel.runLifecycleTest()
                    }
                    .buttonStyle(.bordered)

                    Button("Clear Log") {
                        viewModel.clearLog()
                    }
                    .buttonStyle(.bordered)

                    Button("Reset Bridge") {
                        viewModel.resetBridge()
                    }
                    .buttonStyle(.bordered)
                }
            }
            .padding()
            .background(Color(white: 0.1))
        }
        .background(Color.black)
    }
}

/// View model for UI Bridge demo.
@MainActor
class UIBridgeDemoViewModel: ObservableObject {
    @Published private(set) var bridge: DTermUIBridge
    @Published private(set) var eventLog: [String] = []
    @Published private(set) var pendingCount: Int = 0
    @Published private(set) var callbackCount: Int = 0
    @Published private(set) var renderPendingCount: Int = 0
    @Published private(set) var isConsistent: Bool = true

    init() {
        bridge = DTermUIBridge()
        log("UI Bridge created")
        updateState()
    }

    // MARK: - State

    var bridgeStateDescription: String {
        switch bridge.state {
        case .idle: return "Idle"
        case .processing: return "Processing"
        case .rendering: return "Rendering"
        case .waitingForCallback: return "Waiting for Callback"
        case .shuttingDown: return "Shutting Down"
        }
    }

    var stateColor: Color {
        switch bridge.state {
        case .idle: return .green
        case .processing: return .yellow
        case .rendering: return .blue
        case .waitingForCallback: return .orange
        case .shuttingDown: return .red
        }
    }

    func terminalStateDescription(id: UInt32) -> String {
        switch bridge.terminalState(terminalId: id) {
        case .inactive: return "Inactive"
        case .active: return "Active"
        case .disposed: return "Disposed"
        }
    }

    func terminalStateColor(id: UInt32) -> Color {
        switch bridge.terminalState(terminalId: id) {
        case .inactive: return .gray
        case .active: return .green
        case .disposed: return .red
        }
    }

    // MARK: - Actions

    func createTerminal(id: UInt32) {
        do {
            try bridge.handleCreateTerminal(terminalId: id)
            log("Created terminal \(id)")
        } catch {
            log("ERROR: Failed to create terminal \(id): \(error)")
        }
        updateState()
    }

    func destroyTerminal(id: UInt32) {
        do {
            try bridge.handleDestroyTerminal(terminalId: id)
            log("Destroyed terminal \(id)")
        } catch {
            log("ERROR: Failed to destroy terminal \(id): \(error)")
        }
        updateState()
    }

    func sendInput(terminalId: UInt32, text: String) {
        guard let data = text.data(using: .utf8) else {
            log("ERROR: Failed to encode input")
            return
        }

        do {
            try bridge.handleInput(terminalId: terminalId, data: data)
            log("Sent input to terminal \(terminalId): \"\(text)\"")
        } catch {
            log("ERROR: Failed to send input: \(error)")
        }
        updateState()
    }

    func resize(terminalId: UInt32, rows: UInt16, cols: UInt16) {
        do {
            try bridge.handleResize(terminalId: terminalId, rows: rows, cols: cols)
            log("Resized terminal \(terminalId) to \(rows)x\(cols)")
        } catch {
            log("ERROR: Failed to resize: \(error)")
        }
        updateState()
    }

    func shutdown() {
        do {
            try bridge.handleShutdown()
            log("Shutdown complete")
        } catch {
            log("ERROR: Failed to shutdown: \(error)")
        }
        updateState()
    }

    func runLifecycleTest() {
        log("--- Starting lifecycle test ---")

        // Test 1: Create terminal
        createTerminal(id: 0)
        guard bridge.terminalState(terminalId: 0) == .active else {
            log("FAIL: Terminal 0 should be active")
            return
        }
        log("PASS: Terminal 0 is active")

        // Test 2: Send input
        sendInput(terminalId: 0, text: "test input")
        log("PASS: Input sent successfully")

        // Test 3: Resize
        resize(terminalId: 0, rows: 24, cols: 80)
        log("PASS: Resize successful")

        // Test 4: Create second terminal
        createTerminal(id: 1)
        guard bridge.terminalState(terminalId: 1) == .active else {
            log("FAIL: Terminal 1 should be active")
            return
        }
        log("PASS: Terminal 1 is active")

        // Test 5: Destroy first terminal
        destroyTerminal(id: 0)
        guard bridge.terminalState(terminalId: 0) == .disposed else {
            log("FAIL: Terminal 0 should be disposed")
            return
        }
        log("PASS: Terminal 0 is disposed")

        // Test 6: Verify consistency
        guard bridge.isConsistent else {
            log("FAIL: Bridge is inconsistent")
            return
        }
        log("PASS: Bridge is consistent")

        // Test 7: Destroy second terminal
        destroyTerminal(id: 1)
        guard bridge.terminalState(terminalId: 1) == .disposed else {
            log("FAIL: Terminal 1 should be disposed")
            return
        }
        log("PASS: Terminal 1 is disposed")

        // Test 8: Final consistency check
        guard bridge.isConsistent else {
            log("FAIL: Bridge is inconsistent after cleanup")
            return
        }
        log("PASS: Final consistency check passed")

        log("--- Lifecycle test PASSED ---")
    }

    func clearLog() {
        eventLog.removeAll()
    }

    func resetBridge() {
        bridge = DTermUIBridge()
        log("Bridge reset")
        updateState()
    }

    // MARK: - Private

    private func log(_ message: String) {
        let timestamp = DateFormatter.localizedString(
            from: Date(),
            dateStyle: .none,
            timeStyle: .medium
        )
        eventLog.append("[\(timestamp)] \(message)")

        // Keep log size reasonable
        if eventLog.count > 100 {
            eventLog.removeFirst(eventLog.count - 100)
        }
    }

    private func updateState() {
        pendingCount = bridge.pendingCount
        callbackCount = bridge.callbackCount
        renderPendingCount = bridge.renderPendingCount
        isConsistent = bridge.isConsistent
    }
}

#Preview {
    UIBridgeDemoView()
}
