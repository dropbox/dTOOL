// DTermAIInterface.swift
// Direct memory access to terminal state for in-process AI
//
// This class provides the AI running inside DashTerm2's process with
// direct access to all terminal windows and their state.
//
// Performance target: <0.01ms for all read operations (no IPC, no serialization)
//
// See docs/DTERM-AI-DIRECTIVE-V3.md PART 4 for design details.

import Foundation

/// A command block tracked by shell integration (OSC 133)
@objc public class DTermCommandBlock: NSObject {
    /// The command text that was entered
    @objc public let command: String

    /// The output produced by the command
    @objc public let output: String

    /// Exit code (if available, -1 if not)
    @objc public let exitCode: Int32

    /// When the command started executing
    @objc public let startDate: Date?

    /// When the command finished
    @objc public let endDate: Date?

    /// Whether the command is still running
    @objc public let isRunning: Bool

    /// Working directory when command was executed
    @objc public let workingDirectory: String?

    @objc public init(command: String, output: String, exitCode: Int32,
                      startDate: Date?, endDate: Date?, isRunning: Bool,
                      workingDirectory: String?) {
        self.command = command
        self.output = output
        self.exitCode = exitCode
        self.startDate = startDate
        self.endDate = endDate
        self.isRunning = isRunning
        self.workingDirectory = workingDirectory
        super.init()
    }
}

/// Information about a terminal session
@objc public class DTermSessionInfo: NSObject {
    /// Unique identifier for this session
    @objc public let sessionID: String

    /// Human-readable name (tab title)
    @objc public let name: String

    /// Terminal dimensions
    @objc public let rows: Int
    @objc public let cols: Int

    /// Whether this terminal is locked from AI access
    @objc public let isAILocked: Bool

    /// Whether shell integration is installed
    @objc public let hasShellIntegration: Bool

    /// Current working directory (if known)
    @objc public let workingDirectory: String?

    /// Whether the terminal is at a shell prompt
    @objc public let isAtPrompt: Bool

    @objc public init(sessionID: String, name: String, rows: Int, cols: Int,
                      isAILocked: Bool, hasShellIntegration: Bool,
                      workingDirectory: String?, isAtPrompt: Bool) {
        self.sessionID = sessionID
        self.name = name
        self.rows = rows
        self.cols = cols
        self.isAILocked = isAILocked
        self.hasShellIntegration = hasShellIntegration
        self.workingDirectory = workingDirectory
        self.isAtPrompt = isAtPrompt
        super.init()
    }
}

/// Direct access to terminal state - runs in-process, <0.01ms latency
///
/// This class provides the primary interface for AI features to read
/// terminal state and send input. All operations are synchronous and
/// fast because they access memory directly (no IPC).
///
/// Thread Safety: All methods must be called on the main thread.
@objc public class DTermAIInterface: NSObject {

    // MARK: - AI Lock Key (stored in session's user info)
    private static let aiLockedKey = "DTermAILocked"

    // MARK: - Get All Terminals

    /// Get information about all open terminal sessions
    /// - Returns: Array of session info objects (excludes AI-locked sessions' content)
    @objc public static func getAllTerminals() -> [DTermSessionInfo] {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        guard let controller = iTermController.sharedInstance() else {
            return []
        }

        return controller.allSessions().compactMap { session -> DTermSessionInfo? in
            let isLocked = isAILocked(session: session)
            let screen = session.screen

            return DTermSessionInfo(
                sessionID: session.guid ?? UUID().uuidString,
                name: session.name ?? "Terminal",
                rows: Int(session.rows),
                cols: Int(session.columns),
                isAILocked: isLocked,
                hasShellIntegration: screen?.shellIntegrationInstalled ?? false,
                workingDirectory: session.currentLocalWorkingDirectory,
                isAtPrompt: session.isAtShellPrompt
            )
        }
    }

    /// Get a specific session by its ID
    /// - Parameter sessionID: The session's GUID
    /// - Returns: The PTYSession, or nil if not found
    @objc public static func getSession(byID sessionID: String) -> PTYSession? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        return iTermController.sharedInstance()?.session(withGUID: sessionID)
    }

    // MARK: - Read Screen Content

    /// Read the current visible screen content
    /// - Parameter session: The terminal session
    /// - Returns: Screen content as a string, or nil if AI-locked
    @objc public static func readScreen(session: PTYSession) -> String? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return nil
        }

        // Try dterm-core first (faster, direct memory access)
        if let dtermCore = session.dtermCoreIntegration,
           iTermAdvancedSettingsModel.dtermCoreEnabled() {
            let lines = dtermCore.extractVisibleLines()
            return lines.joined(separator: "\n")
        }

        // Fallback to VT100Screen
        guard let screen = session.screen else {
            return nil
        }

        return screen.compactLineDump()
    }

    /// Read scrollback buffer content
    /// - Parameters:
    ///   - session: The terminal session
    ///   - lines: Maximum number of lines to return (0 = all)
    /// - Returns: Scrollback content as a string, or nil if AI-locked
    @objc public static func readScrollback(session: PTYSession, lines: Int) -> String? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return nil
        }

        guard let screen = session.screen else {
            return nil
        }

        // Get full content including history
        guard let fullContent = screen.compactLineDumpWithHistory() else {
            return nil
        }

        if lines <= 0 {
            return fullContent
        }

        // Limit to requested number of lines
        let allLines = fullContent.components(separatedBy: "\n")
        let limitedLines = allLines.suffix(lines)
        return limitedLines.joined(separator: "\n")
    }

    // MARK: - Command History (Shell Integration)

    /// Get command history from OSC 133 shell integration
    /// - Parameters:
    ///   - session: The terminal session
    ///   - count: Maximum number of commands to return (0 = all)
    /// - Returns: Array of command blocks, or empty if AI-locked or no shell integration
    @objc public static func getCommandHistory(session: PTYSession, count: Int) -> [DTermCommandBlock] {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return []
        }

        guard let screen = session.screen,
              screen.shellIntegrationInstalled else {
            return []
        }

        var blocks: [DTermCommandBlock] = []

        // Try dterm-core first
        if let dtermCore = session.dtermCoreIntegration,
           iTermAdvancedSettingsModel.dtermCoreEnabled() {
            let allBlocks = dtermCore.allBlocks
            let limitedBlocks = count > 0 ? Array(allBlocks.suffix(count)) : allBlocks

            for block in limitedBlocks {
                let commandText = dtermCore.extractCommandText(from: block)
                let outputText = dtermCore.extractOutputText(from: block)

                // Block is "running" if it's in executing state
                let isRunning = block.state == .executing

                blocks.append(DTermCommandBlock(
                    command: commandText,
                    output: outputText,
                    exitCode: block.hasExitCode ? block.exitCode : -1,
                    startDate: nil,  // dterm-core doesn't track dates yet
                    endDate: nil,
                    isRunning: isRunning,
                    workingDirectory: nil
                ))
            }

            return blocks
        }

        // Fallback to VT100Screen marks
        var marks: [VT100ScreenMarkReading] = []
        screen.enumeratePrompts(from: nil, to: nil) { mark in
            if let mark = mark {
                marks.append(mark)
            }
        }

        // Limit and reverse to get most recent first
        let limitedMarks = count > 0 ? Array(marks.suffix(count)) : marks

        for mark in limitedMarks {
            guard let command = mark.command, !command.isEmpty else { continue }

            // Get output range and extract text using commandInRange
            // (Note: rangeOfOutputForCommandMark returns the output region)
            let outputRange = screen.rangeOfOutput(forCommandMark: mark)
            var outputText = ""
            if outputRange.start.x >= 0 && outputRange.end.y >= outputRange.start.y {
                // Use commandInRange which works for any range of text
                outputText = screen.command(in: outputRange) ?? ""
            }

            blocks.append(DTermCommandBlock(
                command: command,
                output: outputText,
                exitCode: mark.hasCode ? mark.code : -1,
                startDate: mark.startDate,
                endDate: mark.endDate,
                isRunning: mark.isRunning,
                workingDirectory: nil  // Would need to look up from screen state
            ))
        }

        return blocks
    }

    // MARK: - Send Input

    /// Send text input to the terminal PTY
    /// - Parameters:
    ///   - session: The terminal session
    ///   - text: Text to send (can include escape sequences)
    /// - Returns: true if sent successfully, false if blocked
    @objc public static func sendInput(session: PTYSession, text: String) -> Bool {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        // Note: We allow sending input to AI-locked terminals if explicitly requested
        // The lock is primarily for READING, not writing
        // However, the UI can provide a separate "block AI input" option if needed

        session.writeTask(text)
        return true
    }

    /// Send a command with newline
    /// - Parameters:
    ///   - session: The terminal session
    ///   - command: Command to execute (newline will be appended)
    /// - Returns: true if sent successfully
    @objc public static func sendCommand(session: PTYSession, command: String) -> Bool {
        return sendInput(session: session, text: command + "\n")
    }

    // MARK: - AI Lock Status

    /// Check if a terminal is locked from AI access
    /// - Parameter session: The terminal session
    /// - Returns: true if the terminal is locked from AI reading
    @objc public static func isAILocked(session: PTYSession) -> Bool {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        // Check session-level lock via genericScope (Swift-compatible version of variablesScope)
        if let scope = session.genericScope,
           let locked = scope.value(forVariableName: aiLockedKey) as? Bool {
            return locked
        }

        return false
    }

    /// Set the AI lock status for a terminal
    /// - Parameters:
    ///   - session: The terminal session
    ///   - locked: Whether to lock the terminal from AI access
    @objc public static func setAILocked(session: PTYSession, locked: Bool) {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if let scope = session.genericScope {
            scope.setValue(locked, forVariableNamed: aiLockedKey)

            // Post notification for UI update
            NotificationCenter.default.post(
                name: NSNotification.Name("DTermAILockStatusChanged"),
                object: session,
                userInfo: ["locked": locked]
            )
        }
    }

    // MARK: - Cursor Position

    /// Get the current cursor position
    /// - Parameter session: The terminal session
    /// - Returns: Tuple of (row, col) with 0-based indices, or nil if AI-locked
    @objc public static func getCursorPosition(session: PTYSession) -> NSValue? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return nil
        }

        // Try dterm-core first
        if let dtermCore = session.dtermCoreIntegration,
           iTermAdvancedSettingsModel.dtermCoreEnabled() {
            let row = Int(dtermCore.cursorRow)
            let col = Int(dtermCore.cursorCol)
            return NSValue(point: NSPoint(x: CGFloat(col), y: CGFloat(row)))
        }

        // Fallback to VT100Screen - get cursor from currentGrid
        guard let screen = session.screen,
              let grid = screen.currentGrid() else {
            return nil
        }

        let cursorX = grid.cursorX
        let cursorY = grid.cursorY
        return NSValue(point: NSPoint(x: CGFloat(cursorX), y: CGFloat(cursorY)))
    }

    // MARK: - Working Directory

    /// Get the current working directory
    /// - Parameter session: The terminal session
    /// - Returns: Working directory path, or nil if unknown or AI-locked
    @objc public static func getWorkingDirectory(session: PTYSession) -> String? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return nil
        }

        return session.currentLocalWorkingDirectory
    }

    // MARK: - Search

    /// Search for text in the visible screen
    /// - Parameters:
    ///   - session: The terminal session
    ///   - query: Text to search for
    /// - Returns: Array of matches as (row, startCol, endCol) tuples, or empty if AI-locked
    @objc public static func searchScreen(session: PTYSession, query: String) -> [[NSNumber]] {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return []
        }

        // Use dterm-core's efficient search
        if let dtermCore = session.dtermCoreIntegration,
           iTermAdvancedSettingsModel.dtermCoreEnabled() {
            let results = dtermCore.searchVisible(query: query)
            return results.map { result in
                result.map { NSNumber(value: $0) }
            }
        }

        // Fallback: simple text search on screen content
        guard let content = readScreen(session: session) else {
            return []
        }

        var matches: [[NSNumber]] = []
        let lines = content.components(separatedBy: "\n")

        for (row, line) in lines.enumerated() {
            var searchRange = line.startIndex..<line.endIndex
            while let range = line.range(of: query, range: searchRange) {
                let startCol = line.distance(from: line.startIndex, to: range.lowerBound)
                let endCol = line.distance(from: line.startIndex, to: range.upperBound)
                matches.append([
                    NSNumber(value: row),
                    NSNumber(value: startCol),
                    NSNumber(value: endCol)
                ])
                searchRange = range.upperBound..<line.endIndex
            }
        }

        return matches
    }

    // MARK: - Current Command

    /// Get the command currently being entered (requires shell integration)
    /// - Parameter session: The terminal session
    /// - Returns: Current command text, or nil if not at prompt or AI-locked
    @objc public static func getCurrentCommand(session: PTYSession) -> String? {
        guard Thread.isMainThread else {
            it_fatalError("DTermAIInterface must be called on main thread")
        }

        if isAILocked(session: session) {
            return nil
        }

        return session.currentCommand
    }
}
