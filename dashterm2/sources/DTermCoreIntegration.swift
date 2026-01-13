// DTermCoreIntegration.swift
// Swift integration layer for dterm-core in dashterm2
//
// This provides a parallel dterm-core terminal instance for:
// 1. Comparison testing against iTerm2's parser
// 2. Gradual migration path
// 3. Performance benchmarking

import Foundation

/// Manages a parallel dterm-core terminal instance for testing and comparison.
///
/// Usage:
/// ```swift
/// let integration = DTermCoreIntegration(rows: 24, cols: 80)
/// integration.process(data)  // Feed PTY data
/// let matches = integration.compareCell(row: 0, col: 0, expected: iTerm2Cell)
/// ```
@objc public class DTermCoreIntegration: NSObject {
    private let terminal: DTermCore
    private var enabled: Bool = false
    private var byteCount: UInt64 = 0
    private var processTime: TimeInterval = 0

    /// Lock for thread-safe access to mutable state and terminal operations.
    /// All access to `enabled`, `byteCount`, `processTime`, and `terminal` methods
    /// must be protected by this lock when called from public APIs.
    private let lock = NSLock()

    // MARK: - Locking Helpers

    /// Execute a closure with the lock held. Returns the closure's result.
    private func withLock<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    /// Execute a closure with the lock held, passing the terminal instance.
    /// This variant makes the locked terminal access explicit and reduces risk of
    /// accidentally accessing terminal outside the lock.
    ///
    /// This method is internal (not private) to allow `DTermGridAdapter` to access
    /// the terminal under the same lock for efficient rendering.
    func withTerminalLock<T>(_ body: (DTermCore) -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body(terminal)
    }

    /// Direct access to the underlying DTermCore for GPU rendering.
    ///
    /// **WARNING**: This exposes the terminal without locking. The caller is responsible
    /// for ensuring thread-safe access. This is primarily for use by `DTermMetalView`
    /// which needs to build vertex buffers directly from terminal state.
    ///
    /// For most use cases, prefer `withTerminalLock(_:)` instead.
    public var terminalForRendering: DTermCore {
        return terminal
    }

    /// Create integration with given dimensions.
    @objc public init(rows: UInt16, cols: UInt16) {
        terminal = DTermCore(rows: rows, cols: cols)
        super.init()
    }

    /// Create integration with custom scrollback.
    @objc public init(rows: UInt16, cols: UInt16, scrollbackLines: Int) {
        let config = ScrollbackConfig(
            ringBufferSize: scrollbackLines,
            hotLimit: min(1000, scrollbackLines / 10),
            warmLimit: min(10000, scrollbackLines),
            memoryBudget: 100 * 1024 * 1024
        )
        terminal = DTermCore(rows: rows, cols: cols, scrollback: config)
        super.init()
    }

    /// Whether dterm-core processing is enabled.
    @objc public var isEnabled: Bool {
        get { withLock { enabled } }
        set { withLock { enabled = newValue } }
    }

    /// Process PTY data through dterm-core.
    ///
    /// - Parameter data: Raw PTY output bytes
    @objc public func process(_ data: Data) {
        withLock {
            guard enabled else { return }

            let start = CFAbsoluteTimeGetCurrent()
            terminal.process(data)
            let elapsed = CFAbsoluteTimeGetCurrent() - start

            byteCount += UInt64(data.count)
            processTime += elapsed
        }
    }

    /// Process PTY data through dterm-core (C pointer version).
    ///
    /// - Parameters:
    ///   - bytes: Pointer to byte data
    ///   - length: Number of bytes
    @objc public func process(bytes: UnsafePointer<UInt8>, length: Int) {
        withLock {
            guard enabled else { return }

            let start = CFAbsoluteTimeGetCurrent()
            terminal.process(bytes: bytes, count: length)
            let elapsed = CFAbsoluteTimeGetCurrent() - start

            byteCount += UInt64(length)
            processTime += elapsed
        }
    }

    /// Resize the dterm-core terminal.
    @objc public func resize(rows: UInt16, cols: UInt16) {
        withLock {
            guard enabled else { return }
            terminal.resize(rows: rows, cols: cols)
        }
    }

    /// Reset the dterm-core terminal.
    @objc public func reset() {
        withLock {
            terminal.reset()
            byteCount = 0
            processTime = 0
        }
    }

    // MARK: - State Accessors

    @objc public var rows: UInt16 { withTerminalLock { $0.rows } }
    @objc public var cols: UInt16 { withTerminalLock { $0.cols } }
    @objc public var cursorRow: UInt16 { withTerminalLock { $0.cursorRow } }
    @objc public var cursorCol: UInt16 { withTerminalLock { $0.cursorCol } }
    @objc public var cursorVisible: Bool { withTerminalLock { $0.cursorVisible } }

    @objc public var windowTitle: String? { withTerminalLock { $0.title } }
    @objc public var isAlternateScreen: Bool { withTerminalLock { $0.isAlternateScreen } }

    // MARK: - Scrollback

    @objc public var scrollbackLines: Int { withTerminalLock { $0.scrollbackLines } }
    @objc public var displayOffset: Int { withTerminalLock { $0.displayOffset } }

    @objc public func scroll(lines: Int32) {
        withTerminalLock { $0.scroll(lines: lines) }
    }

    @objc public func scrollToTop() {
        withTerminalLock { $0.scrollToTop() }
    }

    @objc public func scrollToBottom() {
        withTerminalLock { $0.scrollToBottom() }
    }

    // MARK: - Performance Stats

    /// Total bytes processed.
    @objc public var totalBytesProcessed: UInt64 { withLock { byteCount } }

    /// Total time spent processing (seconds).
    @objc public var totalProcessingTime: TimeInterval { withLock { processTime } }

    /// Average throughput in MB/s.
    @objc public var throughputMBps: Double {
        withLock {
            guard processTime > 0 else { return 0 }
            return Double(byteCount) / processTime / (1024 * 1024)
        }
    }

    /// Performance summary string.
    @objc public var performanceSummary: String {
        withLock {
            let mbProcessed = Double(byteCount) / (1024 * 1024)
            let throughput = processTime > 0 ? Double(byteCount) / processTime / (1024 * 1024) : 0
            return String(format: "dterm-core: %.2f MB processed at %.1f MB/s",
                          mbProcessed,
                          throughput)
        }
    }

    // MARK: - Cell Access

    /// Get character at position.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based)
    ///   - col: Column index (0-based)
    /// - Returns: Character, or space if empty/out of bounds
    @objc public func characterAt(row: UInt16, col: UInt16) -> unichar {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col),
                  let scalar = cell.codepoint else {
                return unichar(0x20) // space
            }
            return unichar(scalar.value)
        }
    }

    /// Get foreground color at position (packed ARGB).
    @objc public func foregroundColorAt(row: UInt16, col: UInt16) -> UInt32 {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return 0
            }
            return colorToPacked(cell.foreground)
        }
    }

    /// Get background color at position (packed ARGB).
    @objc public func backgroundColorAt(row: UInt16, col: UInt16) -> UInt32 {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return 0
            }
            return colorToPacked(cell.background)
        }
    }

    /// Check if cell is bold.
    @objc public func isBoldAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.bold)
        }
    }

    /// Check if cell is italic.
    @objc public func isItalicAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.italic)
        }
    }

    /// Check if cell is underlined.
    @objc public func isUnderlineAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.underline)
        }
    }

    /// Check if cell is a wide character.
    @objc public func isWideAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.wide)
        }
    }

    /// Get raw cell flags for debugging.
    @objc public func rawFlagsAt(row: UInt16, col: UInt16) -> UInt32 {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return 0
            }
            // cell.flags.rawValue is already UInt32 after casting from FFI uint16_t
            return cell.flags.rawValue
        }
    }

    /// Check if cell is dim.
    @objc public func isDimAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.dim)
        }
    }

    /// Check if cell has blink attribute.
    @objc public func isBlinkAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.blink)
        }
    }

    /// Check if cell is inverse (reverse video).
    @objc public func isInverseAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.inverse)
        }
    }

    /// Check if cell is invisible.
    @objc public func isInvisibleAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.invisible)
        }
    }

    /// Check if cell has strikethrough.
    @objc public func isStrikethroughAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.flags.contains(.strikethrough)
        }
    }

    /// Get underline color at position (packed, or 0xFFFFFFFF for default).
    ///
    /// SGR 58 (set underline color) and SGR 59 (reset underline color).
    /// - Returns: Packed color value, or 0xFFFFFFFF if using default foreground
    @objc public func underlineColorAt(row: UInt16, col: UInt16) -> UInt32 {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col),
                  let underlineColor = cell.underlineColor else {
                return 0xFFFF_FFFF  // Use default foreground
            }
            return colorToPacked(underlineColor)
        }
    }

    /// Check if cell has a custom underline color (SGR 58).
    @objc public func hasUnderlineColorAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            guard let cell = terminal.cell(at: row, col: col) else {
                return false
            }
            return cell.underlineColor != nil
        }
    }

    // MARK: - Hyperlinks (OSC 8)

    /// Get hyperlink URL at position, if any.
    ///
    /// OSC 8 hyperlinks allow terminal applications to create clickable links.
    /// - Parameters:
    ///   - row: Row index (0-based)
    ///   - col: Column index (0-based)
    /// - Returns: URL string, or nil if no hyperlink
    @objc public func hyperlinkAt(row: UInt16, col: UInt16) -> String? {
        return withTerminalLock { terminal in
            terminal.hyperlinkAt(row: row, col: col)
        }
    }

    /// Check if cell has a hyperlink.
    ///
    /// This is a faster check than `hyperlinkAt` when you only need to know
    /// if a hyperlink exists, not what the URL is.
    /// - Returns: true if cell has hyperlink
    @objc public func hasHyperlinkAt(row: UInt16, col: UInt16) -> Bool {
        return withTerminalLock { terminal in
            terminal.hasHyperlinkAt(row: row, col: col)
        }
    }

    /// Get the current active hyperlink URL being applied to new text.
    ///
    /// When an OSC 8 hyperlink sequence is received, subsequent text will have
    /// the hyperlink applied until the hyperlink is cleared.
    /// - Returns: Active hyperlink URL, or nil if no active hyperlink
    @objc public func currentHyperlink() -> String? {
        return withTerminalLock { terminal in
            terminal.currentHyperlink()
        }
    }

    // MARK: - Terminal Modes

    /// Terminal modes (Swift-only access).
    public var modes: DTermModes {
        return withTerminalLock { $0.modes }
    }

    /// Whether origin mode is enabled (DECOM).
    @objc public var originMode: Bool {
        return withTerminalLock { $0.modes.originMode }
    }

    /// Whether insert mode is enabled (IRM).
    @objc public var insertMode: Bool {
        return withTerminalLock { $0.modes.insertMode }
    }

    /// Whether bracketed paste mode is enabled.
    @objc public var bracketedPasteMode: Bool {
        return withTerminalLock { $0.modes.bracketedPaste }
    }

    /// Whether application cursor keys mode is enabled (DECCKM).
    @objc public var applicationCursorKeys: Bool {
        return withTerminalLock { $0.modes.applicationCursorKeys }
    }

    /// Whether autowrap mode is enabled (DECAWM).
    @objc public var autoWrapMode: Bool {
        return withTerminalLock { $0.modes.autoWrap }
    }

    // MARK: - Line Content Extraction (Priority 3)

    /// Extract visible screen content as text lines by iterating cells.
    ///
    /// Uses cell-by-cell enumeration since FFI line text APIs aren't available yet.
    /// - Returns: Array of line strings from visible area
    @objc public func extractVisibleLines() -> [String] {
        return withLock {
            guard enabled else { return [] }
            let rowCount = terminal.rows
            let colCount = terminal.cols
            var lines: [String] = []
            lines.reserveCapacity(Int(rowCount))

            for row in 0..<rowCount {
                var lineChars: [Character] = []
                lineChars.reserveCapacity(Int(colCount))

                for col in 0..<colCount {
                    if let cell = terminal.cell(at: row, col: col) {
                        if let char = cell.character {
                            lineChars.append(char)
                        } else {
                            lineChars.append(" ")
                        }
                    } else {
                        lineChars.append(" ")
                    }
                }

                // Trim trailing whitespace
                while !lineChars.isEmpty && lineChars.last == " " {
                    lineChars.removeLast()
                }
                lines.append(String(lineChars))
            }
            return lines
        }
    }

    /// Get text content of a visible row by iterating cells.
    ///
    /// - Parameter row: Row index (0 = top of visible area)
    /// - Returns: Row text content (trailing whitespace trimmed)
    @objc public func getVisibleLineText(row: UInt16) -> String {
        return withLock {
            guard enabled else { return "" }
            guard row < terminal.rows else { return "" }
            let colCount = terminal.cols
            var lineChars: [Character] = []
            lineChars.reserveCapacity(Int(colCount))

            for col in 0..<colCount {
                if let cell = terminal.cell(at: row, col: col) {
                    if let char = cell.character {
                        lineChars.append(char)
                    } else {
                        lineChars.append(" ")
                    }
                } else {
                    lineChars.append(" ")
                }
            }

            // Trim trailing whitespace
            while !lineChars.isEmpty && lineChars.last == " " {
                lineChars.removeLast()
            }
            return String(lineChars)
        }
    }

    // MARK: - Damage Tracking (Priority 3)

    /// Whether the terminal needs a redraw.
    @objc public var needsRedraw: Bool {
        return withLock {
            guard enabled else { return false }
            return terminal.needsRedraw
        }
    }

    /// Clear damage tracking after rendering.
    @objc public func clearDamage() {
        withLock {
            guard enabled else { return }
            terminal.clearDamage()
        }
    }

    // MARK: - Search Integration (Priority 3)

    /// Search visible screen content for text.
    ///
    /// - Parameters:
    ///   - query: Text to search for
    /// - Returns: Array of search matches [row, startCol, endCol]
    @objc public func searchVisible(query: String) -> [[Int]] {
        // extractVisibleLines already uses lock internally
        guard !query.isEmpty else { return [] }
        let visibleLines = extractVisibleLines()
        guard !visibleLines.isEmpty else { return [] }

        var results: [[Int]] = []

        for (row, lineText) in visibleLines.enumerated() {
            var searchStart = lineText.startIndex

            while searchStart < lineText.endIndex {
                guard let range = lineText.range(of: query, range: searchStart..<lineText.endIndex) else {
                    break
                }
                let startCol = lineText.distance(from: lineText.startIndex, to: range.lowerBound)
                let endCol = lineText.distance(from: lineText.startIndex, to: range.upperBound)
                results.append([row, startCol, endCol])
                searchStart = range.upperBound
            }
        }
        return results
    }

    /// Check if visible terminal content contains the search query.
    ///
    /// - Parameter query: Text to search for
    /// - Returns: true if query is found in visible area
    @objc public func containsText(_ query: String) -> Bool {
        guard !query.isEmpty else { return false }

        // Use extractVisibleLines for efficiency - single lock acquisition
        let visibleLines = extractVisibleLines()
        for lineText in visibleLines {
            if lineText.contains(query) { return true }
        }

        return false
    }

    // MARK: - Comparison Testing

    /// Compare cursor position with iTerm2.
    ///
    /// - Parameters:
    ///   - iTermRow: iTerm2's cursor row
    ///   - iTermCol: iTerm2's cursor column
    /// - Returns: true if positions match
    @objc public func compareCursor(iTermRow: Int, iTermCol: Int) -> Bool {
        guard enabled else { return true }
        return Int(cursorRow) == iTermRow && Int(cursorCol) == iTermCol
    }

    /// Generate comparison report for current state.
    @objc public func generateComparisonReport() -> String {
        guard enabled else { return "dterm-core integration disabled" }

        var report = "=== dterm-core State ===\n"
        report += "Dimensions: \(rows)x\(cols)\n"
        report += "Cursor: (\(cursorRow), \(cursorCol))\n"
        report += "Cursor visible: \(cursorVisible)\n"
        report += "Alternate screen: \(isAlternateScreen)\n"
        report += "Scrollback: \(scrollbackLines) lines\n"
        report += performanceSummary + "\n"
        return report
    }

    // MARK: - Side-by-Side Validation

    /// Result of a validation comparison.
    @objc public class ValidationResult: NSObject {
        @objc public let passed: Bool
        @objc public let discrepancies: [String]
        @objc public let checkedCells: Int
        @objc public let mismatchedCells: Int

        init(passed: Bool, discrepancies: [String], checkedCells: Int, mismatchedCells: Int) {
            self.passed = passed
            self.discrepancies = discrepancies
            self.checkedCells = checkedCells
            self.mismatchedCells = mismatchedCells
            super.init()
        }

        @objc public override var description: String {
            if passed {
                return "[PASS] dterm-core validation: \(checkedCells) cells checked, all match"
            } else {
                return "[FAIL] dterm-core validation: \(mismatchedCells)/\(checkedCells) cells mismatched\n" +
                       discrepancies.prefix(10).joined(separator: "\n")
            }
        }
    }

    /// Validate cursor position against iTerm2.
    ///
    /// - Parameters:
    ///   - iTermCursorX: iTerm2's cursor X position (1-based)
    ///   - iTermCursorY: iTerm2's cursor Y position (1-based)
    /// - Returns: Array of discrepancy descriptions, empty if match
    @objc public func validateCursor(iTermCursorX: Int, iTermCursorY: Int) -> [String] {
        return withLock {
            guard enabled else { return [] }
            var discrepancies: [String] = []

            // iTerm2 uses 1-based coordinates, dterm-core uses 0-based
            let expectedRow = iTermCursorY - 1
            let expectedCol = iTermCursorX - 1

            let dtermRow = Int(terminal.cursorRow)
            let dtermCol = Int(terminal.cursorCol)

            if dtermRow != expectedRow {
                discrepancies.append("Cursor row mismatch: dterm=\(dtermRow) vs iTerm=\(expectedRow) (1-based: \(iTermCursorY))")
            }
            if dtermCol != expectedCol {
                discrepancies.append("Cursor col mismatch: dterm=\(dtermCol) vs iTerm=\(expectedCol) (1-based: \(iTermCursorX))")
            }

            return discrepancies
        }
    }

    /// Validate a single cell against iTerm2's screen_char_t data.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based)
    ///   - col: Column index (0-based)
    ///   - iTermCode: iTerm2's character code (unichar)
    ///   - iTermBold: iTerm2's bold flag
    ///   - iTermItalic: iTerm2's italic flag
    ///   - iTermUnderline: iTerm2's underline flag
    ///   - iTermInverse: iTerm2's inverse flag
    /// - Returns: Array of discrepancy descriptions, empty if match
    @objc public func validateCell(
        row: UInt16,
        col: UInt16,
        iTermCode: unichar,
        iTermBold: Bool,
        iTermItalic: Bool,
        iTermUnderline: Bool,
        iTermInverse: Bool
    ) -> [String] {
        return withLock {
            guard enabled else { return [] }
            var discrepancies: [String] = []

            guard let cell = terminal.cell(at: row, col: col) else {
                discrepancies.append("[\(row),\(col)] dterm-core cell not available")
                return discrepancies
            }

            // Compare character code
            let dtermCode: unichar
            if let scalar = cell.codepoint {
                dtermCode = unichar(scalar.value)
            } else {
                dtermCode = 0x20  // space for empty
            }

            // Skip DWC_RIGHT cells (value 4) - these are placeholders for wide chars
            let DWC_RIGHT: unichar = 4
            if iTermCode == DWC_RIGHT {
                // For wide char continuation, dterm-core uses a spacer flag instead
                if !cell.flags.contains(.wideSpacer) && dtermCode != 0x20 {
                    discrepancies.append("[\(row),\(col)] Wide char spacer mismatch: dterm has code=\(dtermCode), wideSpacer=\(cell.flags.contains(.wideSpacer))")
                }
                return discrepancies
            }

            // For regular cells, compare the character code
            if dtermCode != iTermCode && iTermCode != 0 {  // 0 means never-written cell
                // Check if it's a space (0x20) vs empty (0)
                if !(dtermCode == 0x20 && iTermCode == 0) {
                    discrepancies.append("[\(row),\(col)] Char mismatch: dterm=U+\(String(format: "%04X", dtermCode)) ('\(Character(UnicodeScalar(dtermCode)!))') vs iTerm=U+\(String(format: "%04X", iTermCode))")
                }
            }

            // Compare attributes
            if cell.flags.contains(.bold) != iTermBold {
                discrepancies.append("[\(row),\(col)] Bold mismatch: dterm=\(cell.flags.contains(.bold)) vs iTerm=\(iTermBold)")
            }
            if cell.flags.contains(.italic) != iTermItalic {
                discrepancies.append("[\(row),\(col)] Italic mismatch: dterm=\(cell.flags.contains(.italic)) vs iTerm=\(iTermItalic)")
            }
            if cell.flags.contains(.underline) != iTermUnderline {
                discrepancies.append("[\(row),\(col)] Underline mismatch: dterm=\(cell.flags.contains(.underline)) vs iTerm=\(iTermUnderline)")
            }
            if cell.flags.contains(.inverse) != iTermInverse {
                discrepancies.append("[\(row),\(col)] Inverse mismatch: dterm=\(cell.flags.contains(.inverse)) vs iTerm=\(iTermInverse)")
            }

            return discrepancies
        }
    }

    /// Perform full screen validation against iTerm2's VT100Screen.
    ///
    /// This method is called from ObjC with the screen state extracted from VT100Screen.
    /// Each row is provided as an array of dictionaries with cell data.
    ///
    /// - Parameters:
    ///   - screenRows: Array of row data, each containing cell information
    ///   - iTermCursorX: iTerm2 cursor X (1-based)
    ///   - iTermCursorY: iTerm2 cursor Y (1-based)
    /// - Returns: ValidationResult with pass/fail and discrepancies
    @objc public func validateScreen(
        screenRows: [[String: Any]],
        iTermCursorX: Int,
        iTermCursorY: Int
    ) -> ValidationResult {
        // Check enabled under lock
        guard withLock({ enabled }) else {
            return ValidationResult(passed: true, discrepancies: [], checkedCells: 0, mismatchedCells: 0)
        }

        var allDiscrepancies: [String] = []
        var checkedCells = 0
        var mismatchedCells = 0

        // Validate cursor first (acquires its own lock)
        let cursorDiscrepancies = validateCursor(iTermCursorX: iTermCursorX, iTermCursorY: iTermCursorY)
        allDiscrepancies.append(contentsOf: cursorDiscrepancies)

        // Validate each cell (each validateCell acquires its own lock)
        for (rowIndex, rowData) in screenRows.enumerated() {
            guard let cells = rowData["cells"] as? [[String: Any]] else { continue }

            for (colIndex, cellData) in cells.enumerated() {
                guard let code = cellData["code"] as? Int,
                      let bold = cellData["bold"] as? Bool,
                      let italic = cellData["italic"] as? Bool,
                      let underline = cellData["underline"] as? Bool,
                      let inverse = cellData["inverse"] as? Bool else {
                    continue
                }

                checkedCells += 1
                let cellDiscrepancies = validateCell(
                    row: UInt16(rowIndex),
                    col: UInt16(colIndex),
                    iTermCode: unichar(code),
                    iTermBold: bold,
                    iTermItalic: italic,
                    iTermUnderline: underline,
                    iTermInverse: inverse
                )

                if !cellDiscrepancies.isEmpty {
                    mismatchedCells += 1
                    allDiscrepancies.append(contentsOf: cellDiscrepancies)
                }
            }
        }

        let passed = allDiscrepancies.isEmpty
        return ValidationResult(
            passed: passed,
            discrepancies: allDiscrepancies,
            checkedCells: checkedCells,
            mismatchedCells: mismatchedCells
        )
    }

    /// Quick validation that only checks cursor and a sample of cells.
    ///
    /// - Parameters:
    ///   - iTermCursorX: iTerm2 cursor X (1-based)
    ///   - iTermCursorY: iTerm2 cursor Y (1-based)
    ///   - sampleCells: Array of [row, col, code, bold, italic, underline, inverse]
    /// - Returns: Array of discrepancy descriptions
    @objc public func quickValidate(
        iTermCursorX: Int,
        iTermCursorY: Int,
        sampleCells: [[Int]]
    ) -> [String] {
        // Check enabled under lock
        guard withLock({ enabled }) else { return [] }

        var discrepancies = validateCursor(iTermCursorX: iTermCursorX, iTermCursorY: iTermCursorY)

        for sample in sampleCells {
            guard sample.count >= 7 else { continue }  // swiftlint:disable:this empty_count
            let row = UInt16(sample[0])
            let col = UInt16(sample[1])
            let code = unichar(sample[2])
            let bold = sample[3] != 0
            let italic = sample[4] != 0
            let underline = sample[5] != 0
            let inverse = sample[6] != 0

            let cellDisc = validateCell(
                row: row, col: col,
                iTermCode: code,
                iTermBold: bold,
                iTermItalic: italic,
                iTermUnderline: underline,
                iTermInverse: inverse
            )
            discrepancies.append(contentsOf: cellDisc)
        }

        return discrepancies
    }

    // MARK: - Shell Integration State

    /// Get the current shell integration state.
    ///
    /// Shell integration (OSC 133) allows the terminal to track command boundaries.
    /// This is essential for AI agents that need to understand:
    /// - When a prompt is displayed
    /// - When the user is entering a command
    /// - When a command is executing
    /// - When a command completes
    @objc public var shellState: DTermShellState {
        return withTerminalLock { $0.shellState }
    }

    /// Get the number of completed output blocks.
    @objc public var blockCount: Int {
        return withTerminalLock { $0.blockCount }
    }

    /// Get an output block by index.
    ///
    /// - Parameter index: Block index (0 = oldest block)
    /// - Returns: OutputBlock if found, nil otherwise
    @objc public func block(at index: Int) -> DTermOutputBlock? {
        return withTerminalLock { $0.block(at: index) }
    }

    /// Get the current (in-progress) output block.
    @objc public var currentBlock: DTermOutputBlock? {
        return withTerminalLock { $0.currentBlock }
    }

    /// Find the output block containing a given row.
    ///
    /// - Parameter row: Absolute row number
    /// - Returns: Block index if found, -1 if no block contains the row
    @objc public func blockIndex(atRow row: Int) -> Int {
        return withTerminalLock { $0.blockIndex(atRow: row) ?? -1 }
    }

    /// Get the exit code of the last completed block.
    ///
    /// - Returns: Exit code if a completed block exists. Check hasLastExitCode first.
    @objc public var lastExitCode: Int32 {
        return withTerminalLock { $0.lastExitCode ?? 0 }
    }

    /// Whether there is a valid last exit code.
    @objc public var hasLastExitCode: Bool {
        return withTerminalLock { $0.lastExitCode != nil }
    }

    /// Get all output blocks.
    @objc public var allBlocks: [DTermOutputBlock] {
        return withTerminalLock { $0.allBlocks }
    }

    /// Extract command text from an output block.
    ///
    /// This extracts the command text (between command start and output start or end).
    /// - Parameter block: Output block to extract from
    /// - Returns: Command text, or empty string if not available
    @objc public func extractCommandText(from block: DTermOutputBlock) -> String {
        guard block.hasCommandStart else { return "" }

        return withTerminalLock { terminal -> String in
            // Get lines from command start to output start (or end)
            let startRow = block.commandStartRow
            let endRow: Int
            if block.hasOutputStart {
                endRow = block.outputStartRow
            } else if block.hasEndRow {
                endRow = block.endRow
            } else {
                // Still executing - use current cursor row
                endRow = Int(terminal.cursorRow)
            }

            var text = ""
            for lineIdx in startRow..<endRow {
                let lineText = terminal.getLineText(lineIndex: lineIdx)
                if !text.isEmpty {
                    text += "\n"
                }
                text += lineText
            }

            // Trim the first line to start from command start column
            if !text.isEmpty {
                let lines = text.components(separatedBy: "\n")
                if let firstLine = lines.first, !firstLine.isEmpty {
                    let startCol = Int(block.commandStartCol)
                    if startCol < firstLine.count {  // swiftlint:disable:this empty_count
                        let startIndex = firstLine.index(firstLine.startIndex, offsetBy: startCol)
                        let trimmedFirst = String(firstLine[startIndex...])
                        var result = [trimmedFirst]
                        result.append(contentsOf: lines.dropFirst())
                        text = result.joined(separator: "\n")
                    }
                }
            }

            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }

    /// Extract output text from an output block.
    ///
    /// This extracts the output (between output start and end).
    /// - Parameter block: Output block to extract from
    /// - Returns: Output text, or empty string if not available
    @objc public func extractOutputText(from block: DTermOutputBlock) -> String {
        guard block.hasOutputStart else { return "" }

        return withTerminalLock { terminal -> String in
            let startRow = block.outputStartRow
            let endRow: Int
            if block.hasEndRow {
                endRow = block.endRow
            } else {
                // Still executing - use current cursor row
                endRow = Int(terminal.cursorRow)
            }

            var lines: [String] = []
            for lineIdx in startRow..<endRow {
                lines.append(terminal.getLineText(lineIndex: lineIdx))
            }

            return lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }

    // MARK: - Private Helpers

    private func colorToPacked(_ color: DTermColor) -> UInt32 {
        // Packed format from dterm-core (cell.rs):
        // - 0x00_INDEX__: Indexed color (0-255) - type byte is 0x00
        // - 0x01_RRGGBB: True color RGB - type byte is 0x01
        // - 0xFF_______: Default color - type byte is 0xFF
        switch color {
        case .default:
            return 0xFF000000
        case .indexed(let index):
            return UInt32(index)  // Type byte 0x00
        case .rgb(let r, let g, let b):
            return 0x01000000 | (UInt32(r) << 16) | (UInt32(g) << 8) | UInt32(b)
        }
    }
}

// MARK: - Version Info

@objc public class DTermCoreInfo: NSObject {
    /// Get dterm-core library version.
    @objc public static var libraryVersion: String {
        dtermVersion()
    }

    /// Check if dterm-core is available.
    @objc public static var isAvailable: Bool {
        // Try to create a minimal terminal to verify library loads
        let terminal = DTermCore(rows: 1, cols: 1)
        return terminal.rows == 1
    }
}

// MARK: - Grid Adapter

/// Adapter that converts dterm-core cells into screen_char_t lines for rendering.
@objc public final class DTermGridAdapter: NSObject {
    private let integration: DTermCoreIntegration

    @objc public init(integration: DTermCoreIntegration) {
        self.integration = integration
        super.init()
    }

    /// Build a ScreenCharArray for the given absolute line index.
    ///
    /// - Parameters:
    ///   - line: Absolute line index (0 = first scrollback line).
    ///   - width: Visible grid width.
    /// - Returns: ScreenCharArray if the line is visible in dterm-core, else nil.
    @objc public func screenCharArray(forLine line: Int32, width: Int32) -> ScreenCharArray? {
        guard integration.isEnabled else { return nil }
        let width = Int(width)
        guard width > 0 else { return nil }

        let buffer = UnsafeMutablePointer<screen_char_t>.allocate(capacity: width + 1)
        buffer.initialize(repeating: screen_char_t(), count: width + 1)

        let populated = populateLine(buffer: buffer, eaIndex: nil, line: Int(line), width: width)
        if !populated {
            buffer.deinitialize(count: width + 1)
            buffer.deallocate()
            return nil
        }

        let continuation = buffer[width]
        return ScreenCharArray(line: buffer,
                               length: Int32(width),
                               metadata: iTermImmutableMetadataDefault(),
                               continuation: continuation,
                               freeOnRelease: true)
    }

    /// Build a ScreenCharArray with external attributes for the given absolute line index.
    ///
    /// This method returns both the screen character array and an external attribute index
    /// containing extended attributes like underline color (SGR 58/59) and hyperlinks.
    ///
    /// - Parameters:
    ///   - line: Absolute line index (0 = first scrollback line).
    ///   - width: Visible grid width.
    /// - Returns: Tuple of (ScreenCharArray, iTermExternalAttributeIndex?) if line is visible, else nil.
    public func screenCharArrayWithExternalAttributes(
        forLine line: Int32,
        width: Int32
    ) -> (ScreenCharArray, iTermExternalAttributeIndex?)? {
        guard integration.isEnabled else { return nil }
        let width = Int(width)
        guard width > 0 else { return nil }

        let buffer = UnsafeMutablePointer<screen_char_t>.allocate(capacity: width + 1)
        buffer.initialize(repeating: screen_char_t(), count: width + 1)

        let eaIndex = iTermExternalAttributeIndex()

        let populated = populateLine(buffer: buffer, eaIndex: eaIndex, line: Int(line), width: width)
        if !populated {
            buffer.deinitialize(count: width + 1)
            buffer.deallocate()
            return nil
        }

        let continuation = buffer[width]
        let screenCharArray = ScreenCharArray(line: buffer,
                                              length: Int32(width),
                                              metadata: iTermImmutableMetadataDefault(),
                                              continuation: continuation,
                                              freeOnRelease: true)

        // Return nil for eaIndex if it's empty (no external attributes)
        let returnedEaIndex: iTermExternalAttributeIndex? = eaIndex.isEmpty ? nil : eaIndex
        return (screenCharArray, returnedEaIndex)
    }

    private func populateLine(buffer: UnsafeMutablePointer<screen_char_t>,
                              eaIndex: iTermExternalAttributeIndex?,
                              line: Int,
                              width: Int) -> Bool {
        var hasLine = false

        integration.withTerminalLock { terminal in
            let rowCount = Int(terminal.rows)
            let colCount = Int(terminal.cols)
            let scrollbackLineCount = terminal.scrollbackLines

            // Line index model:
            // - Lines 0..<scrollbackLineCount are in scrollback (tiered storage)
            // - Lines scrollbackLineCount..<(scrollbackLineCount + rowCount) are visible grid
            let visibleGridStart = scrollbackLineCount
            let visibleGridEnd = scrollbackLineCount + rowCount

            let maxCols = min(width, colCount)

            if line < scrollbackLineCount {
                // Scrollback line - use scrollback cell FFI
                let scrollbackRow = line
                if maxCols > 0 {
                    for col in 0..<maxCols {
                        let cell = terminal.scrollbackCell(at: scrollbackRow, col: UInt16(col))
                        // Note: scrollback hyperlinks not yet implemented in dterm-core FFI
                        _ = eaIndex  // Silence unused warning

                        if let cell = cell {
                            buffer[col] = screenChar(fromScrollback: cell)
                        }
                    }
                }

                // Set continuation based on whether line is wrapped
                var continuation = screen_char_t()
                if terminal.scrollbackLineIsWrapped(at: scrollbackRow) {
                    continuation.code = unichar(EOL_SOFT)
                } else {
                    continuation.code = unichar(EOL_HARD)
                }
                buffer[width] = continuation
                hasLine = true

            } else if line >= visibleGridStart && line < visibleGridEnd {
                // Visible grid line - use regular cell FFI
                let gridRow = line - visibleGridStart
                if maxCols > 0 {
                    for col in 0..<maxCols {
                        let cell = terminal.cell(at: UInt16(gridRow), col: UInt16(col))
                        let hyperlinkURL = terminal.hyperlinkAt(row: UInt16(gridRow), col: UInt16(col))

                        if let cell = cell {
                            buffer[col] = screenChar(from: cell)

                            // Build external attributes if underline color or hyperlink is present
                            if let eaIndex = eaIndex {
                                let ulColor = cell.underlineColor

                                // Only create external attribute if there's something to store
                                if ulColor != nil || hyperlinkURL != nil {
                                    let colorValue: VT100TerminalColorValue
                                    let hasUnderlineColor: Bool
                                    if let ulColor = ulColor {
                                        colorValue = self.termColorToVT100ColorValue(ulColor)
                                        hasUnderlineColor = true
                                    } else {
                                        colorValue = VT100TerminalColorValue(red: 0, green: 0, blue: 0, mode: ColorModeAlternate)
                                        hasUnderlineColor = false
                                    }

                                    // Create iTermURL from hyperlink URL string
                                    let url: iTermURL?
                                    if let hyperlinkURL = hyperlinkURL, let nsurl = URL(string: hyperlinkURL) {
                                        url = iTermURL(url: nsurl, identifier: nil, target: nil)
                                    } else {
                                        url = nil
                                    }

                                    let ea = iTermExternalAttribute(
                                        havingUnderlineColor: hasUnderlineColor,
                                        underlineColor: colorValue,
                                        url: url,
                                        blockIDList: nil,
                                        controlCode: nil
                                    )
                                    eaIndex.setObject(ea, atIndexedSubscript: UInt(col))
                                }
                            }
                        }
                    }
                }

                var continuation = screen_char_t()
                continuation.code = unichar(EOL_HARD)
                buffer[width] = continuation
                hasLine = true
            }
            // Lines beyond visible grid end are invalid - hasLine remains false
        }

        return hasLine
    }

    /// Convert a DTermColor to VT100TerminalColorValue for external attributes.
    private func termColorToVT100ColorValue(_ color: DTermColor) -> VT100TerminalColorValue {
        switch color {
        case .default:
            return VT100TerminalColorValue(red: 0, green: 0, blue: 0, mode: ColorModeAlternate)
        case .indexed(let index):
            return VT100TerminalColorValue(red: Int32(index), green: 0, blue: 0, mode: ColorModeNormal)
        case .rgb(let r, let g, let b):
            return VT100TerminalColorValue(red: Int32(r), green: Int32(g), blue: Int32(b), mode: ColorMode24bit)
        }
    }

    private func screenChar(from cell: DTermCell) -> screen_char_t {
        var result = screen_char_t()

        if let scalar = cell.codepoint {
            if scalar.value <= UInt32(UInt16.max) {
                result.code = unichar(scalar.value)
            } else {
                let string = String(scalar) as NSString
                // Use raw value 0 for iTermUnicodeNormalizationNone (defined in ITAddressBookMgr.h)
                ComplexCharRegistry.instance.setComplexChar(in: &result,
                                                            string: string,
                                                            normalization: iTermUnicodeNormalization(rawValue: 0)!,
                                                            isSpacingCombiningMark: false)
            }
        }

        apply(color: cell.foreground, isForeground: true, to: &result)
        apply(color: cell.background, isForeground: false, to: &result)

        result.bold = cell.flags.contains(.bold) ? 1 : 0
        result.faint = cell.flags.contains(.dim) ? 1 : 0
        result.italic = cell.flags.contains(.italic) ? 1 : 0
        result.blink = cell.flags.contains(.blink) ? 1 : 0
        result.invisible = cell.flags.contains(.invisible) ? 1 : 0
        result.strikethrough = cell.flags.contains(.strikethrough) ? 1 : 0

        if cell.flags.isDoubleUnderline {
            result.underline = 1
            ScreenCharSetUnderlineStyle(&result, .double)
        } else if cell.flags.contains(.underline) {
            result.underline = 1
            ScreenCharSetUnderlineStyle(&result, .single)
        } else {
            result.underline = 0
        }

        if cell.flags.contains(.inverse) {
            ScreenCharInvert(&result)
        }

        if cell.flags.contains(.wideSpacer) {
            ScreenCharSetDWC_RIGHT(&result)
        }

        return result
    }

    private func screenChar(fromScrollback cell: DTermScrollbackCell) -> screen_char_t {
        var result = screen_char_t()

        if let scalar = cell.codepoint {
            if scalar.value <= UInt32(UInt16.max) {
                result.code = unichar(scalar.value)
            } else {
                let string = String(scalar) as NSString
                ComplexCharRegistry.instance.setComplexChar(in: &result,
                                                            string: string,
                                                            normalization: iTermUnicodeNormalization(rawValue: 0)!,
                                                            isSpacingCombiningMark: false)
            }
        }

        apply(color: cell.foreground, isForeground: true, to: &result)
        apply(color: cell.background, isForeground: false, to: &result)

        result.bold = cell.flags.contains(.bold) ? 1 : 0
        result.faint = cell.flags.contains(.dim) ? 1 : 0
        result.italic = cell.flags.contains(.italic) ? 1 : 0
        result.blink = cell.flags.contains(.blink) ? 1 : 0
        result.invisible = cell.flags.contains(.invisible) ? 1 : 0
        result.strikethrough = cell.flags.contains(.strikethrough) ? 1 : 0

        // Note: Scrollback cells don't have underlineColor - use basic underline
        if cell.flags.isDoubleUnderline {
            result.underline = 1
            ScreenCharSetUnderlineStyle(&result, .double)
        } else if cell.flags.contains(.underline) {
            result.underline = 1
            ScreenCharSetUnderlineStyle(&result, .single)
        } else {
            result.underline = 0
        }

        if cell.flags.contains(.inverse) {
            ScreenCharInvert(&result)
        }

        if cell.flags.contains(.wideSpacer) {
            ScreenCharSetDWC_RIGHT(&result)
        }

        return result
    }

    private func apply(color: DTermColor, isForeground: Bool, to cell: inout screen_char_t) {
        switch color {
        case .default:
            if isForeground {
                cell.foregroundColorMode = ColorModeAlternate.rawValue
                cell.foregroundColor = UInt32(ALTSEM_DEFAULT)
            } else {
                cell.backgroundColorMode = ColorModeAlternate.rawValue
                cell.backgroundColor = UInt32(ALTSEM_DEFAULT)
            }
        case .indexed(let index):
            if isForeground {
                cell.foregroundColorMode = ColorModeNormal.rawValue
                cell.foregroundColor = UInt32(index)
            } else {
                cell.backgroundColorMode = ColorModeNormal.rawValue
                cell.backgroundColor = UInt32(index)
            }
        case .rgb(let r, let g, let b):
            if isForeground {
                cell.foregroundColorMode = ColorMode24bit.rawValue
                cell.foregroundColor = UInt32(r)
                cell.fgGreen = UInt32(g)
                cell.fgBlue = UInt32(b)
            } else {
                cell.backgroundColorMode = ColorMode24bit.rawValue
                cell.backgroundColor = UInt32(r)
                cell.bgGreen = UInt32(g)
                cell.bgBlue = UInt32(b)
            }
        }
    }
}
