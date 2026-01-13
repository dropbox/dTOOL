//
//  TerminalSession.swift
//  DashTerm
//
//  Manages a single terminal session with PTY and state
//

import Foundation
import Combine
import AppKit

/// A terminal session that manages PTY communication and terminal state
@MainActor
class TerminalSession: ObservableObject, Identifiable {
    let id = UUID()

    @Published var title: String = "Terminal"
    @Published var isRunning: Bool = false
    @Published var terminalSize: TerminalSize = TerminalSize(cols: 80, rows: 24)

    // Terminal state from Rust
    private var terminalHandle: OpaquePointer?

    // PTY
    private var ptyHandle: OpaquePointer?
    private var masterFD: Int32 = -1
    private var readSource: DispatchSourceRead?

    // Publisher for terminal updates
    let updatePublisher = PassthroughSubject<Void, Never>()

    init() {
        setupTerminal()
        spawnShell()
    }

    deinit {
        // Cancel dispatch source and close FD directly since we can't call MainActor methods
        readSource?.cancel()
        if masterFD >= 0 {
            close(masterFD)
        }
        if let handle = terminalHandle {
            dashterm_terminal_free(handle)
        }
    }

    private func setupTerminal() {
        // Create terminal via FFI
        terminalHandle = dashterm_terminal_new(UInt32(terminalSize.cols), UInt32(terminalSize.rows))
    }

    private func spawnShell() {
        // Use posix_spawn with forkpty via Process
        var masterFD: Int32 = -1
        var slaveFD: Int32 = -1

        // Open PTY
        if openpty(&masterFD, &slaveFD, nil, nil, nil) != 0 {
            print("Failed to open PTY")
            return
        }

        self.masterFD = masterFD

        // Set terminal size
        var winsize = winsize(
            ws_row: UInt16(terminalSize.rows),
            ws_col: UInt16(terminalSize.cols),
            ws_xpixel: 0,
            ws_ypixel: 0
        )
        _ = ioctl(masterFD, TIOCSWINSZ, &winsize)

        // Use posix_spawn to launch shell
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"

        var pid: pid_t = 0
        var fileActions: posix_spawn_file_actions_t?
        var attrs: posix_spawnattr_t?

        posix_spawn_file_actions_init(&fileActions)
        posix_spawnattr_init(&attrs)

        // Set up file actions: redirect stdin/stdout/stderr to slave PTY
        posix_spawn_file_actions_adddup2(&fileActions, slaveFD, STDIN_FILENO)
        posix_spawn_file_actions_adddup2(&fileActions, slaveFD, STDOUT_FILENO)
        posix_spawn_file_actions_adddup2(&fileActions, slaveFD, STDERR_FILENO)
        posix_spawn_file_actions_addclose(&fileActions, masterFD)
        if slaveFD > STDERR_FILENO {
            posix_spawn_file_actions_addclose(&fileActions, slaveFD)
        }

        // Set new session
        posix_spawnattr_setflags(&attrs, Int16(POSIX_SPAWN_SETSID))

        // Set up environment
        let env = [
            "TERM=xterm-256color",
            "COLORTERM=truecolor",
            "HOME=\(NSHomeDirectory())",
            "PATH=\(ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin")",
            "SHELL=\(shell)"
        ]

        let cEnv = env.map { strdup($0) } + [nil]
        defer { cEnv.compactMap { $0 }.forEach { free($0) } }

        let argv: [UnsafeMutablePointer<CChar>?] = [
            strdup(shell),
            strdup("-l"),
            nil
        ]
        defer { argv.compactMap { $0 }.forEach { free($0) } }

        let result = posix_spawn(&pid, shell, &fileActions, &attrs, argv, cEnv)

        posix_spawn_file_actions_destroy(&fileActions)
        posix_spawnattr_destroy(&attrs)

        if result == 0 {
            // Parent process
            close(slaveFD)
            isRunning = true

            // Set non-blocking
            let flags = fcntl(masterFD, F_GETFL)
            _ = fcntl(masterFD, F_SETFL, flags | O_NONBLOCK)

            // Start reading
            startReading()
        } else {
            print("posix_spawn failed: \(result)")
            close(masterFD)
            close(slaveFD)
        }
    }

    private func startReading() {
        let source = DispatchSource.makeReadSource(fileDescriptor: masterFD, queue: .global(qos: .userInteractive))

        source.setEventHandler { [weak self] in
            self?.readFromPTY()
        }

        source.setCancelHandler { [weak self] in
            if let fd = self?.masterFD, fd >= 0 {
                close(fd)
            }
        }

        readSource = source
        source.resume()
    }

    private func readFromPTY() {
        var buffer = [UInt8](repeating: 0, count: 4096)
        let bytesRead = read(masterFD, &buffer, buffer.count)

        if bytesRead > 0 {
            // Check if we're at the bottom before processing new output
            // If so, we'll auto-scroll to stay at the bottom
            let wasAtBottom: Bool
            if let handle = terminalHandle {
                wasAtBottom = dashterm_terminal_get_display_offset(handle) == 0
            } else {
                wasAtBottom = true
            }

            // Process through terminal emulator
            if let handle = terminalHandle {
                buffer.withUnsafeBufferPointer { ptr in
                    dashterm_terminal_process(handle, ptr.baseAddress, UInt(bytesRead))
                }

                // Auto-scroll to bottom if user was at bottom before new output
                if wasAtBottom {
                    dashterm_terminal_scroll_to_bottom(handle)
                }
            }

            // Notify UI to update and process events
            DispatchQueue.main.async { [weak self] in
                self?.processEvents()
                self?.updatePublisher.send()
            }
        } else if bytesRead == 0 || (bytesRead < 0 && errno != EAGAIN && errno != EWOULDBLOCK) {
            // EOF or error
            DispatchQueue.main.async { [weak self] in
                self?.isRunning = false
            }
        }
    }

    /// Write data to the PTY
    func write(_ string: String) {
        guard isRunning, masterFD >= 0 else { return }

        if let data = string.data(using: .utf8) {
            data.withUnsafeBytes { ptr in
                _ = Darwin.write(masterFD, ptr.baseAddress, data.count)
            }
        }
    }

    /// Resize the terminal
    func resize(cols: Int, rows: Int) {
        terminalSize = TerminalSize(cols: cols, rows: rows)

        // Update Rust terminal
        if let handle = terminalHandle {
            dashterm_terminal_resize(handle, UInt32(cols), UInt32(rows))
        }

        // Update PTY
        if masterFD >= 0 {
            var winsize = winsize(
                ws_row: UInt16(rows),
                ws_col: UInt16(cols),
                ws_xpixel: 0,
                ws_ypixel: 0
            )
            _ = ioctl(masterFD, TIOCSWINSZ, &winsize)
        }
    }

    /// Get the terminal grid for rendering
    func getGrid() -> [[TerminalCell]] {
        guard let handle = terminalHandle else {
            return []
        }

        guard let jsonPtr = dashterm_terminal_get_grid_json(handle) else {
            return []
        }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)

        do {
            let cells = try JSONDecoder().decode([[TerminalCell]].self, from: jsonString.data(using: .utf8)!)
            return cells
        } catch {
            print("Failed to decode grid: \(error)")
            return []
        }
    }

    /// Get cursor position
    func getCursor() -> CursorPosition {
        guard let handle = terminalHandle else {
            return CursorPosition(row: 0, col: 0, visible: true)
        }

        let cursor = dashterm_terminal_get_cursor(handle)
        return CursorPosition(row: Int(cursor.row), col: Int(cursor.col), visible: cursor.visible)
    }

    /// Get and process pending terminal events
    func processEvents() {
        guard let handle = terminalHandle else { return }

        guard let jsonPtr = dashterm_terminal_get_events_json(handle) else { return }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)

        do {
            let events = try JSONDecoder().decode([TerminalEvent].self, from: jsonString.data(using: .utf8)!)
            for event in events {
                handleEvent(event)
            }
        } catch {
            // Ignore decode errors - events are best-effort
        }
    }

    private func handleEvent(_ event: TerminalEvent) {
        switch event {
        case .bell:
            NSSound.beep()
        case .titleChanged(let newTitle):
            title = newTitle
        case .exit(let code):
            isRunning = false
            print("Terminal exited with code: \(code)")
        case .redraw:
            // Handled by damage tracking
            break
        }
    }

    /// Get damaged regions for efficient partial updates
    func getDamage() -> [DamageRegion] {
        guard let handle = terminalHandle else { return [] }

        guard let jsonPtr = dashterm_terminal_get_damage_json(handle) else { return [] }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)

        do {
            // Damage is array of [line, left, right] tuples
            let tuples = try JSONDecoder().decode([[Int]].self, from: jsonString.data(using: .utf8)!)
            return tuples.compactMap { tuple in
                guard tuple.count == 3 else { return nil }
                return DamageRegion(line: tuple[0], left: tuple[1], right: tuple[2])
            }
        } catch {
            // Fall back to full damage
            return [DamageRegion(line: 0, left: 0, right: terminalSize.cols)]
        }
    }

    /// Reset damage tracking after rendering
    func resetDamage() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_reset_damage(handle)
    }

    /// Get a single row of the terminal grid (for partial updates)
    func getGridRow(_ row: Int) -> [TerminalCell] {
        let grid = getGrid()
        guard row >= 0 && row < grid.count else { return [] }
        return grid[row]
    }

    // MARK: - Scrollback

    /// Get the current scroll display offset (0 = bottom, positive = scrolled up)
    func getDisplayOffset() -> Int {
        guard let handle = terminalHandle else { return 0 }
        return Int(dashterm_terminal_get_display_offset(handle))
    }

    /// Get the total number of history lines available for scrolling
    func getHistorySize() -> Int {
        guard let handle = terminalHandle else { return 0 }
        return Int(dashterm_terminal_get_history_size(handle))
    }

    /// Scroll the display up by the given number of lines
    func scrollUp(_ lines: Int) {
        guard let handle = terminalHandle, lines > 0 else { return }
        dashterm_terminal_scroll_up(handle, UInt32(lines))
        updatePublisher.send()
    }

    /// Scroll the display down by the given number of lines
    func scrollDown(_ lines: Int) {
        guard let handle = terminalHandle, lines > 0 else { return }
        dashterm_terminal_scroll_down(handle, UInt32(lines))
        updatePublisher.send()
    }

    /// Scroll to the top of history
    func scrollToTop() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_scroll_to_top(handle)
        updatePublisher.send()
    }

    /// Scroll to the bottom (most recent output)
    func scrollToBottom() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_scroll_to_bottom(handle)
        updatePublisher.send()
    }

    /// Check if terminal is currently scrolled (not at bottom)
    var isScrolled: Bool {
        getDisplayOffset() > 0
    }

    // MARK: - Agent Parsing

    /// Whether agent parsing is currently enabled
    @Published private(set) var isAgentParsingEnabled: Bool = false

    /// Enable agent output parsing
    func enableAgentParsing() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_enable_agent_parsing(handle)
        isAgentParsingEnabled = true
    }

    /// Disable agent output parsing
    func disableAgentParsing() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_disable_agent_parsing(handle)
        isAgentParsingEnabled = false
    }

    /// Toggle agent parsing
    func toggleAgentParsing() {
        if isAgentParsingEnabled {
            disableAgentParsing()
        } else {
            enableAgentParsing()
        }
    }

    /// Get pending agent events
    func getAgentEvents() -> [AgentEvent] {
        guard let handle = terminalHandle else { return [] }
        guard let jsonPtr = dashterm_terminal_get_agent_events_json(handle) else { return [] }
        defer { dashterm_string_free(jsonPtr) }

        let jsonString = String(cString: jsonPtr)
        guard !jsonString.isEmpty, jsonString != "[]" else { return [] }

        do {
            let events = try JSONDecoder().decode([AgentEvent].self, from: jsonString.data(using: .utf8)!)
            return events
        } catch {
            print("Failed to decode agent events: \(error)")
            return []
        }
    }

    /// Get currently active agent node ID
    func getActiveAgentNode() -> String? {
        guard let handle = terminalHandle else { return nil }
        guard let ptr = dashterm_terminal_get_active_agent_node(handle) else { return nil }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Get currently active agent tool name
    func getActiveAgentTool() -> String? {
        guard let handle = terminalHandle else { return nil }
        guard let ptr = dashterm_terminal_get_active_agent_tool(handle) else { return nil }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Clear agent parser state
    func clearAgentState() {
        guard let handle = terminalHandle else { return }
        dashterm_terminal_clear_agent_state(handle)
    }

    private func cleanup() {
        readSource?.cancel()

        if let handle = terminalHandle {
            dashterm_terminal_free(handle)
        }
    }
}

// MARK: - Supporting Types

struct TerminalSize: Equatable {
    let cols: Int
    let rows: Int
}

struct CursorPosition {
    let row: Int
    let col: Int
    let visible: Bool
}

/// A damaged region that needs redrawing
struct DamageRegion {
    let line: Int
    let left: Int
    let right: Int
}

/// Terminal events from the Rust backend
enum TerminalEvent: Codable {
    case redraw
    case bell
    case titleChanged(String)
    case exit(Int32)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        // Try simple string values first
        if let stringValue = try? container.decode(String.self) {
            switch stringValue {
            case "Redraw": self = .redraw
            case "Bell": self = .bell
            default: self = .redraw
            }
            return
        }

        // Try object format
        let keyedContainer = try decoder.container(keyedBy: CodingKeys.self)

        if let title = try? keyedContainer.decode(String.self, forKey: .TitleChanged) {
            self = .titleChanged(title)
        } else if let code = try? keyedContainer.decode(Int32.self, forKey: .Exit) {
            self = .exit(code)
        } else {
            self = .redraw
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .redraw:
            try container.encode("Redraw")
        case .bell:
            try container.encode("Bell")
        case .titleChanged(let title):
            var keyed = encoder.container(keyedBy: CodingKeys.self)
            try keyed.encode(title, forKey: .TitleChanged)
        case .exit(let code):
            var keyed = encoder.container(keyedBy: CodingKeys.self)
            try keyed.encode(code, forKey: .Exit)
        }
    }

    enum CodingKeys: String, CodingKey {
        case TitleChanged, Exit
    }
}

/// Terminal cell representation matching Rust Cell struct
struct TerminalCell: Codable {
    let content: String
    let width: UInt8
    let attrs: CellAttributes

    var foreground: ColorAttribute { attrs.foreground }
    var background: ColorAttribute { attrs.background }
    var bold: Bool { attrs.bold }
    var italic: Bool { attrs.italic }
    var underline: Bool { attrs.underline }
    var strikethrough: Bool { attrs.strikethrough }
    var inverse: Bool { attrs.inverse }
}

struct CellAttributes: Codable {
    let foreground: ColorAttribute
    let background: ColorAttribute
    let bold: Bool
    let italic: Bool
    let underline: Bool
    let strikethrough: Bool
    let inverse: Bool
    let hidden: Bool
    let dim: Bool
    let blink: Bool
}

enum ColorAttribute: Codable {
    case `default`
    case named(UInt8)
    case indexed(UInt8)
    case rgb(UInt8, UInt8, UInt8)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if let stringValue = try? container.decode(String.self), stringValue == "Default" {
            self = .default
            return
        }

        // Handle object format
        let keyedContainer = try decoder.container(keyedBy: CodingKeys.self)

        if let value = try? keyedContainer.decode(UInt8.self, forKey: .Named) {
            self = .named(value)
        } else if let value = try? keyedContainer.decode(UInt8.self, forKey: .Indexed) {
            self = .indexed(value)
        } else if let values = try? keyedContainer.decode([UInt8].self, forKey: .Rgb), values.count == 3 {
            self = .rgb(values[0], values[1], values[2])
        } else {
            self = .default
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .default:
            try container.encode("Default")
        case .named(let v):
            var keyed = encoder.container(keyedBy: CodingKeys.self)
            try keyed.encode(v, forKey: .Named)
        case .indexed(let v):
            var keyed = encoder.container(keyedBy: CodingKeys.self)
            try keyed.encode(v, forKey: .Indexed)
        case .rgb(let r, let g, let b):
            var keyed = encoder.container(keyedBy: CodingKeys.self)
            try keyed.encode([r, g, b], forKey: .Rgb)
        }
    }

    enum CodingKeys: String, CodingKey {
        case Named, Indexed, Rgb
    }
}
