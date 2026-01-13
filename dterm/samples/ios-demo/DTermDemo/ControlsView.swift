/*
 * ControlsView.swift - Demo control panel
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 *
 * Provides demo controls for testing terminal functionality.
 */

import SwiftUI
import DTermCore

/// Control panel for terminal demo.
struct ControlsView: View {
    @ObservedObject var state: TerminalState
    @State private var inputText: String = ""

    var body: some View {
        VStack(spacing: 8) {
            // Input field
            HStack {
                TextField("Input", text: $inputText)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(.body, design: .monospaced))

                Button("Send") {
                    if !inputText.isEmpty {
                        state.process(inputText)
                        inputText = ""
                    }
                }
                .buttonStyle(.bordered)
            }

            // Demo buttons
            HStack(spacing: 12) {
                Button("Demo") {
                    state.runDemo()
                }
                .buttonStyle(.bordered)

                Button("Clear") {
                    state.clearScreen()
                }
                .buttonStyle(.bordered)

                Button("Box") {
                    state.testCursorMovement()
                }
                .buttonStyle(.bordered)

                Button("+Line") {
                    state.addScrollbackLine()
                }
                .buttonStyle(.bordered)
            }

            // Status bar
            HStack {
                Text("Size: \(state.terminal.rows)x\(state.terminal.cols)")
                Spacer()
                Text("Cursor: (\(state.terminal.cursorRow), \(state.terminal.cursorCol))")
                Spacer()
                Text("Scrollback: \(state.terminal.scrollbackLines)")
            }
            .font(.caption)
            .foregroundColor(.secondary)
        }
        .padding()
        .background(Color(white: 0.1))
    }
}

#Preview {
    ControlsView(state: TerminalState())
}
