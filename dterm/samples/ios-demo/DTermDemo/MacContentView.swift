#if os(macOS)
/*
 * MacContentView.swift - macOS terminal demo with PTY
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import SwiftUI
import AppKit
import DTermCore

struct MacContentView: View {
    @StateObject private var state = MacTerminalState()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(state.title)
                    .font(.headline)
                // Show scroll indicator when viewing scrollback
                if state.terminal.displayOffset > 0 {
                    Text("[scrollback: \(state.terminal.displayOffset) lines]")
                        .font(.caption)
                        .foregroundColor(.gray)
                }
                Spacer()
                Button("Run vttest") {
                    state.sendText("vttest\n")
                }
                Button("Restart Shell") {
                    state.restartShell()
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(Color(white: 0.15))

            MacTerminalView(state: state)
        }
        .background(Color.black)
        .onAppear {
            state.startShellIfNeeded()
        }
    }
}

struct MacTerminalView: View {
    @ObservedObject var state: MacTerminalState

    let fontSize: CGFloat = 14

    private var charWidth: CGFloat {
        fontSize * 0.6
    }

    private var lineHeight: CGFloat {
        fontSize * 1.2
    }

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                TerminalContentView(
                    terminal: state.terminal,
                    fontSize: fontSize,
                    lastUpdate: state.lastUpdate,
                    cursorBlink: state.cursorBlink,
                    selection: state.selection
                )
                TerminalInputView(
                    onKeyData: { data in
                        state.sendInput(data)
                    },
                    onFocusChange: { focused in
                        state.sendFocus(focused)
                    },
                    onMouseEvent: { event in
                        state.sendMouseEvent(event)
                    },
                    onCopy: {
                        state.copySelection()
                    },
                    onPaste: {
                        state.pasteFromClipboard()
                    },
                    onScroll: { delta in
                        state.handleScroll(delta: delta)
                    },
                    charWidth: charWidth,
                    lineHeight: lineHeight
                )
            }
            .background(Color.black)
            .onAppear {
                state.updateSize(
                    width: geometry.size.width,
                    height: geometry.size.height,
                    charWidth: charWidth,
                    lineHeight: lineHeight
                )
            }
            .onChange(of: geometry.size) { newSize in
                state.updateSize(
                    width: newSize.width,
                    height: newSize.height,
                    charWidth: charWidth,
                    lineHeight: lineHeight
                )
            }
        }
    }
}

/// Selection range in terminal coordinates
struct SelectionRange: Equatable {
    var startRow: Int
    var startCol: Int
    var endRow: Int
    var endCol: Int

    /// Normalize so start is before end
    var normalized: SelectionRange {
        if startRow < endRow || (startRow == endRow && startCol <= endCol) {
            return self
        }
        return SelectionRange(startRow: endRow, startCol: endCol, endRow: startRow, endCol: startCol)
    }

    /// Check if a cell is within the selection
    func contains(row: Int, col: Int) -> Bool {
        let norm = normalized
        if row < norm.startRow || row > norm.endRow { return false }
        if row == norm.startRow && row == norm.endRow {
            return col >= norm.startCol && col <= norm.endCol
        }
        if row == norm.startRow { return col >= norm.startCol }
        if row == norm.endRow { return col <= norm.endCol }
        return true
    }
}

@MainActor
final class MacTerminalState: ObservableObject, DTermTerminalDelegate {
    @Published private(set) var terminal: DTermTerminal
    @Published private(set) var lastUpdate: Date = Date()
    @Published private(set) var title: String = "dterm"
    @Published private(set) var cursorBlink: Bool = false
    @Published var selection: SelectionRange?
    @Published private(set) var isSelecting: Bool = false

    private var pty: PTYSession?
    private var currentRows: Int
    private var currentCols: Int
    private var blinkTimer: Timer?

    init() {
        let rows = 24
        let cols = 80
        currentRows = rows
        currentCols = cols
        terminal = DTermTerminal(rows: rows, cols: cols)
        terminal.delegate = self
        startCursorBlink()
    }

    private func startCursorBlink() {
        blinkTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.cursorBlink.toggle()
            }
        }
    }

    deinit {
        blinkTimer?.invalidate()
    }

    func startShellIfNeeded() {
        if pty == nil {
            startShell()
        }
    }

    func restartShell() {
        pty?.terminate()
        pty = nil
        startShell()
    }

    func sendInput(_ data: Data) {
        pty?.write(data)
    }

    func sendText(_ text: String) {
        guard let data = text.data(using: .utf8) else { return }
        sendInput(data)
    }

    func sendFocus(_ focused: Bool) {
        if let data = terminal.encodeFocusEvent(focused: focused) {
            sendInput(data)
        }
    }

    /// Handle scrollback navigation
    func handleScroll(delta: Int) {
        if delta == Int.max {
            terminal.scrollToTop()
        } else if delta == Int.min {
            terminal.scrollToBottom()
        } else {
            terminal.scroll(delta: delta)
        }
        lastUpdate = Date()
    }

    func sendMouseEvent(_ event: MouseEvent) {
        // If terminal is tracking mouse, forward to PTY
        if terminal.mouseTrackingEnabled {
            var data: Data?
            switch event.kind {
            case .press(let button):
                data = terminal.encodeMousePress(
                    button: button,
                    col: event.col,
                    row: event.row,
                    modifiers: event.modifiers
                )
            case .release(let button):
                data = terminal.encodeMouseRelease(
                    button: button,
                    col: event.col,
                    row: event.row,
                    modifiers: event.modifiers
                )
            case .motion(let button):
                data = terminal.encodeMouseMotion(
                    button: button,
                    col: event.col,
                    row: event.row,
                    modifiers: event.modifiers
                )
            case .wheel(let up):
                data = terminal.encodeMouseWheel(
                    up: up,
                    col: event.col,
                    row: event.row,
                    modifiers: event.modifiers
                )
            }

            if let data {
                sendInput(data)
            }
            return
        }

        // Handle local events when terminal isn't tracking mouse
        switch event.kind {
        case .press(let button) where button == 0:
            startSelection(row: event.row, col: event.col)
        case .motion where isSelecting:
            updateSelection(row: event.row, col: event.col)
        case .release(let button) where button == 0:
            endSelection(row: event.row, col: event.col)
        case .wheel(let up):
            // Handle scroll wheel for scrollback navigation
            let delta = up ? 3 : -3
            terminal.scroll(delta: delta)
            lastUpdate = Date()
        default:
            break
        }
    }

    // MARK: - Selection

    func startSelection(row: Int, col: Int) {
        isSelecting = true
        selection = SelectionRange(startRow: row, startCol: col, endRow: row, endCol: col)
    }

    func updateSelection(row: Int, col: Int) {
        guard isSelecting, var sel = selection else { return }
        sel.endRow = row
        sel.endCol = col
        selection = sel
    }

    func endSelection(row: Int, col: Int) {
        updateSelection(row: row, col: col)
        isSelecting = false
    }

    func clearSelection() {
        selection = nil
        isSelecting = false
    }

    /// Get selected text from terminal
    func getSelectedText() -> String? {
        guard let sel = selection?.normalized else { return nil }
        var text = ""
        for row in sel.startRow...sel.endRow {
            var rowText = ""
            let startCol = row == sel.startRow ? sel.startCol : 0
            let endCol = row == sel.endRow ? sel.endCol : terminal.cols - 1
            for col in startCol...endCol {
                if let cell = terminal.getCell(row: row, col: col),
                   let char = cell.character {
                    rowText.append(char)
                } else {
                    rowText.append(" ")
                }
            }
            // Trim trailing spaces for each line
            text += rowText.trimmingCharacters(in: .whitespaces)
            if row < sel.endRow {
                text += "\n"
            }
        }
        return text.isEmpty ? nil : text
    }

    /// Copy selection to clipboard
    func copySelection() {
        guard let text = getSelectedText() else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    /// Paste from clipboard with bracketed paste support
    func pasteFromClipboard() {
        let pasteboard = NSPasteboard.general
        guard let text = pasteboard.string(forType: .string) else { return }

        // Check if bracketed paste mode is enabled
        if terminal.modes.bracketedPaste {
            // Wrap text with bracketed paste escape sequences
            // \x1b[200~ = start bracketed paste
            // \x1b[201~ = end bracketed paste
            sendText("\u{1b}[200~\(text)\u{1b}[201~")
        } else {
            sendText(text)
        }
    }

    func updateSize(width: CGFloat, height: CGFloat, charWidth: CGFloat, lineHeight: CGFloat) {
        let cols = max(1, Int(width / charWidth))
        let rows = max(1, Int(height / lineHeight))
        resize(rows: rows, cols: cols)
    }

    private func resize(rows: Int, cols: Int) {
        guard rows != currentRows || cols != currentCols else { return }
        currentRows = rows
        currentCols = cols
        terminal.resize(rows: rows, cols: cols)
        pty?.resize(rows: rows, cols: cols)
    }

    private func startShell() {
        do {
            let env = ProcessInfo.processInfo.environment
            let (command, exitOnComplete) = startCommand(for: env)
            let session = try PTYSession(
                rows: currentRows,
                cols: currentCols,
                command: command
            )
            session.onData = { [weak self] data in
                Task { @MainActor in
                    guard let self else { return }
                    self.terminal.process(data: data)
                    self.lastUpdate = Date()
                }
            }
            session.onExit = { [weak self] in
                Task { @MainActor in
                    self?.title = "dterm (session ended)"
                    if exitOnComplete {
                        NSApp.terminate(nil)
                    }
                }
            }
            pty = session
        } catch {
            title = "dterm (pty error)"
        }
    }

    private func startCommand(for env: [String: String]) -> ([String], Bool) {
        if let commandLog = env["DTERM_VTTEST_COMMAND_LOG"], !commandLog.isEmpty {
            let outputLog = env["DTERM_VTTEST_LOG"] ?? defaultVttestLogPath()
            let exitOnComplete = env["DTERM_VTTEST_EXIT_ON_COMPLETE"] == "1"
            return (["vttest", "-c", commandLog, "-l", outputLog], exitOnComplete)
        }
        return (["/bin/zsh", "-l"], false)
    }

    private func defaultVttestLogPath() -> String {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let logsDir = home.appendingPathComponent("dterm/worker_logs", isDirectory: true)
        try? FileManager.default.createDirectory(at: logsDir, withIntermediateDirectories: true)
        return logsDir.appendingPathComponent("vttest.log").path
    }

    // MARK: - DTermTerminalDelegate

    nonisolated func terminalTitleDidChange(_ terminal: DTermTerminal, title: String) {
        Task { @MainActor in
            self.title = title.isEmpty ? "dterm" : title
        }
    }

    nonisolated func terminalHasResponse(_ terminal: DTermTerminal, data: Data) {
        Task { @MainActor in
            self.pty?.write(data)
        }
    }

    nonisolated func terminalModesDidChange(_ terminal: DTermTerminal) {}

    nonisolated func terminalDidReceiveSixelImage(_ terminal: DTermTerminal, width: Int, height: Int, pixels: [UInt32]) {}

    nonisolated func terminalSetClipboard(_ terminal: DTermTerminal, content: String) {
        Task { @MainActor in
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(content, forType: .string)
        }
    }
}
#endif
