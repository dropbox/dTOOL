/*
 * ContentView.swift - Main view for DTermDemo
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import SwiftUI
import DTermCore

struct ContentView: View {
    @StateObject private var terminalState = TerminalState()

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("DTermDemo")
                    .font(.headline)
                Spacer()
                Text("dterm-core v\(dtermVersion())")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(Color(white: 0.15))

            // Terminal view
            TerminalView(state: terminalState)

            // Controls
            ControlsView(state: terminalState)
        }
        .background(Color.black)
        .onAppear {
            terminalState.runDemo()
        }
    }
}

/// Manages terminal state and provides demo functionality.
@MainActor
class TerminalState: ObservableObject, DTermTerminalDelegate {
    @Published private(set) var terminal: DTermTerminal
    @Published private(set) var responseLog: [String] = []
    @Published private(set) var lastUpdate: Date = Date()

    init() {
        terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.delegate = self
    }

    /// Run a demo sequence to show terminal capabilities.
    func runDemo() {
        // Clear and set title
        process("\u{1b}c")  // RIS - Reset to Initial State
        process("\u{1b}]0;DTermDemo\u{07}")  // Set title

        // Welcome message with colors
        process("\u{1b}[1;34m")  // Bold blue
        process("Welcome to DTermDemo\r\n")
        process("\u{1b}[0m")  // Reset

        process("\r\n")
        process("This sample app demonstrates dterm-core integration.\r\n")
        process("\r\n")

        // Show color palette
        process("\u{1b}[1mStandard Colors:\u{1b}[0m\r\n")
        for i in 0..<8 {
            process("\u{1b}[4\(i)m  \u{1b}[0m")
        }
        process("\r\n")
        for i in 0..<8 {
            process("\u{1b}[10\(i)m  \u{1b}[0m")
        }
        process("\r\n\r\n")

        // Show 256 colors (first 16 + some cube colors)
        process("\u{1b}[1m256 Color Palette:\u{1b}[0m\r\n")
        for row in 0..<6 {
            for col in 0..<36 {
                let color = 16 + row * 36 + col
                process("\u{1b}[48;5;\(color)m \u{1b}[0m")
            }
            process("\r\n")
        }
        process("\r\n")

        // Show true color gradient
        process("\u{1b}[1mTrue Color Gradient:\u{1b}[0m\r\n")
        for i in stride(from: 0, through: 255, by: 4) {
            process("\u{1b}[48;2;\(i);0;\(255-i)m \u{1b}[0m")
        }
        process("\r\n\r\n")

        // Show text attributes
        process("\u{1b}[1mText Attributes:\u{1b}[0m\r\n")
        process("Normal ")
        process("\u{1b}[1mBold\u{1b}[0m ")
        process("\u{1b}[3mItalic\u{1b}[0m ")
        process("\u{1b}[4mUnderline\u{1b}[0m ")
        process("\u{1b}[7mReverse\u{1b}[0m ")
        process("\u{1b}[9mStrikethrough\u{1b}[0m")
        process("\r\n\r\n")

        // Show Unicode
        process("\u{1b}[1mUnicode & Emoji:\u{1b}[0m\r\n")
        process("Box Drawing: \u{2500}\u{2502}\u{250c}\u{2510}\u{2514}\u{2518}\r\n")
        process("Emoji: \u{1f600} \u{1f680} \u{2764}\u{fe0f} \u{1f3c6}\r\n")
        process("\r\n")

        // Prompt
        process("\u{1b}[32m$\u{1b}[0m ")

        objectWillChange.send()
        lastUpdate = Date()
    }

    /// Process input bytes.
    func process(_ text: String) {
        if let data = text.data(using: .utf8) {
            terminal.process(data: data)
        }
    }

    /// Add a line to scrollback (scroll up).
    func addScrollbackLine() {
        process("\r\nScrollback line \(terminal.scrollbackLines + 1)")
        objectWillChange.send()
        lastUpdate = Date()
    }

    /// Test cursor movement.
    func testCursorMovement() {
        // Save cursor position
        process("\u{1b}7")

        // Draw a box
        process("\u{1b}[5;10H")  // Move to row 5, col 10
        process("\u{1b}[1;33m")  // Yellow
        process("\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}")
        process("\u{1b}[6;10H")
        process("\u{2502}  TEST  \u{2502}")
        process("\u{1b}[7;10H")
        process("\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}")
        process("\u{1b}[0m")  // Reset

        // Restore cursor
        process("\u{1b}8")

        objectWillChange.send()
        lastUpdate = Date()
    }

    /// Clear screen.
    func clearScreen() {
        process("\u{1b}[2J\u{1b}[H")  // Clear and home
        objectWillChange.send()
        lastUpdate = Date()
    }

    /// Resize terminal.
    func resize(rows: Int, cols: Int) {
        terminal.resize(rows: rows, cols: cols)
        objectWillChange.send()
        lastUpdate = Date()
    }

    // MARK: - DTermTerminalDelegate

    nonisolated func terminalTitleDidChange(_ terminal: DTermTerminal, title: String) {
        Task { @MainActor in
            responseLog.append("Title changed: \(title)")
        }
    }

    nonisolated func terminalHasResponse(_ terminal: DTermTerminal, data: Data) {
        Task { @MainActor in
            if let text = String(data: data, encoding: .utf8) {
                responseLog.append("Response: \(text.debugDescription)")
            }
        }
    }

    nonisolated func terminalModesDidChange(_ terminal: DTermTerminal) {
        Task { @MainActor in
            responseLog.append("Modes changed")
        }
    }

    nonisolated func terminalDidReceiveSixelImage(_ terminal: DTermTerminal, width: Int, height: Int, pixels: [UInt32]) {
        Task { @MainActor in
            responseLog.append("Sixel image: \(width)x\(height)")
        }
    }

    nonisolated func terminalSetClipboard(_ terminal: DTermTerminal, content: String) {
        Task { @MainActor in
            responseLog.append("Clipboard: \(content)")
        }
    }
}

#Preview {
    ContentView()
}
