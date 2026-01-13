// DashTermCore.swift
// DashTerm2
//
// Swift wrapper around the dashterm-core Rust FFI.
// This provides a safe, idiomatic Swift interface to the Rust terminal engine.

import Foundation

/// Terminal event types from the Rust core.
enum DashTermEvent: Codable {
    case commandStarted(id: String, text: String)
    case commandFinished(id: String, exitCode: Int32, durationMs: UInt64)
    case outputChunk(commandId: String, text: String, stream: String)
    case directoryChanged(oldPath: String, newPath: String)
    case promptShown(prompt: String)
}

/// Command information from the Rust core.
struct DashTermCommandInfo: Codable {
    let id: String
    let text: String
    let startTimeMs: UInt64
    let endTimeMs: UInt64?
    let exitCode: Int32?
    let cwd: String

    private enum CodingKeys: String, CodingKey {
        case id
        case text
        case startTimeMs = "start_time_ms"
        case endTimeMs = "end_time_ms"
        case exitCode = "exit_code"
        case cwd
    }
}

/// Terminal state snapshot from the Rust core.
struct DashTermSnapshot: Codable {
    let visibleContent: String
    let currentCommand: DashTermCommandInfo?
    let recentCommands: [DashTermCommandInfo]
    let cwd: String
    let cursorRow: Int
    let cursorCol: Int
    let rows: Int
    let cols: Int
    let title: String

    private enum CodingKeys: String, CodingKey {
        case visibleContent = "visible_content"
        case currentCommand = "current_command"
        case recentCommands = "recent_commands"
        case cwd
        case cursorRow = "cursor_row"
        case cursorCol = "cursor_col"
        case rows
        case cols
        case title
    }
}

/// Swift wrapper around the Rust terminal core.
/// Provides a safe, idiomatic interface to the dashterm-core FFI.
final class DashTermCore {
    // MARK: - Properties

    /// Opaque pointer to the Rust terminal instance.
    private var terminal: OpaquePointer?

    /// Whether the terminal has been freed.
    private var isFreed = false

    // MARK: - Initialization

    /// Create a new terminal with default configuration (24x80, 10000 line scrollback).
    init() {
        terminal = dashterm_terminal_new()
    }

    /// Create a new terminal with custom dimensions.
    /// - Parameters:
    ///   - rows: Number of rows (default 24)
    ///   - cols: Number of columns (default 80)
    ///   - scrollback: Maximum scrollback lines (default 10000)
    init(rows: Int, cols: Int, scrollback: Int = 10000) {
        terminal = dashterm_terminal_new_with_config(UInt(rows), UInt(cols), UInt(scrollback))
    }

    deinit {
        freeTerminal()
    }

    // MARK: - Memory Management

    /// Free the terminal resources.
    /// Called automatically in deinit, but can be called manually if needed.
    func freeTerminal() {
        guard !isFreed, let term = terminal else { return }
        dashterm_terminal_free(term)
        terminal = nil
        isFreed = true
    }

    // MARK: - Input

    /// Write raw bytes to the terminal for processing.
    /// - Parameter data: The data to write.
    func write(_ data: Data) {
        guard let term = terminal else { return }
        data.withUnsafeBytes { buffer in
            if let ptr = buffer.baseAddress?.assumingMemoryBound(to: UInt8.self) {
                dashterm_terminal_write(term, ptr, UInt(buffer.count))
            }
        }
    }

    /// Write a string to the terminal.
    /// - Parameter string: The string to write (will be encoded as UTF-8).
    func write(_ string: String) {
        if let data = string.data(using: .utf8) {
            write(data)
        }
    }

    // MARK: - Dimensions

    /// Number of rows in the terminal.
    var rows: Int {
        guard let term = terminal else { return 0 }
        return Int(dashterm_terminal_get_rows(term))
    }

    /// Number of columns in the terminal.
    var cols: Int {
        guard let term = terminal else { return 0 }
        return Int(dashterm_terminal_get_cols(term))
    }

    /// Resize the terminal.
    /// - Parameters:
    ///   - rows: New number of rows.
    ///   - cols: New number of columns.
    func resize(rows: Int, cols: Int) {
        guard let term = terminal else { return }
        dashterm_terminal_resize(term, UInt(rows), UInt(cols))
    }

    // MARK: - Cursor

    /// Current cursor position as (row, col), both 0-indexed.
    var cursorPosition: (row: Int, col: Int) {
        guard let term = terminal else { return (0, 0) }
        var row: UInt = 0
        var col: UInt = 0
        dashterm_terminal_get_cursor(term, &row, &col)
        return (Int(row), Int(col))
    }

    // MARK: - Content

    /// Get a single line of text from the terminal.
    /// - Parameter row: The row index (0-indexed).
    /// - Returns: The text content of that row, or nil if out of bounds.
    func line(at row: Int) -> String? {
        guard let term = terminal else { return nil }
        guard let ptr = dashterm_terminal_get_line(term, UInt(row)) else { return nil }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Get all visible text content.
    var visibleText: String {
        guard let term = terminal else { return "" }
        guard let ptr = dashterm_terminal_get_visible_text(term) else { return "" }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Get full text including scrollback.
    var fullText: String {
        guard let term = terminal else { return "" }
        guard let ptr = dashterm_terminal_get_full_text(term) else { return "" }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Get the window title.
    var title: String {
        guard let term = terminal else { return "" }
        guard let ptr = dashterm_terminal_get_title(term) else { return "" }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    // MARK: - AI Agent API

    /// Current working directory.
    var cwd: String {
        guard let term = terminal else { return "" }
        guard let ptr = dashterm_terminal_get_cwd(term) else { return "" }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }

    /// Last exit code, or nil if no command has completed.
    var lastExitCode: Int32? {
        guard let term = terminal else { return nil }
        let code = dashterm_terminal_get_last_exit_code(term)
        return code == -1 ? nil : code
    }

    /// Current running command, if any.
    var currentCommand: DashTermCommandInfo? {
        guard let term = terminal else { return nil }
        guard let ptr = dashterm_terminal_get_current_command(term) else { return nil }
        defer { dashterm_string_free(ptr) }
        let json = String(cString: ptr)
        return try? JSONDecoder().decode(DashTermCommandInfo.self, from: Data(json.utf8))
    }

    /// Get recent command history.
    /// - Parameter limit: Maximum number of commands to return (0 = all).
    /// - Returns: Array of command info objects.
    func commandHistory(limit: Int = 10) -> [DashTermCommandInfo] {
        guard let term = terminal else { return [] }
        guard let ptr = dashterm_terminal_get_command_history(term, UInt(limit)) else { return [] }
        defer { dashterm_string_free(ptr) }
        let json = String(cString: ptr)
        return (try? JSONDecoder().decode([DashTermCommandInfo].self, from: Data(json.utf8))) ?? []
    }

    /// Get a complete snapshot of terminal state.
    /// Useful for AI agents that need all terminal information at once.
    var snapshot: DashTermSnapshot? {
        guard let term = terminal else { return nil }
        guard let ptr = dashterm_terminal_get_snapshot(term) else { return nil }
        defer { dashterm_string_free(ptr) }
        let json = String(cString: ptr)
        return try? JSONDecoder().decode(DashTermSnapshot.self, from: Data(json.utf8))
    }

    // MARK: - Version

    /// Library version string.
    static var version: String {
        guard let ptr = dashterm_version() else { return "unknown" }
        defer { dashterm_string_free(ptr) }
        return String(cString: ptr)
    }
}

// MARK: - CustomStringConvertible

extension DashTermCore: CustomStringConvertible {
    var description: String {
        "DashTermCore(\(rows)x\(cols), cursor: \(cursorPosition))"
    }
}
