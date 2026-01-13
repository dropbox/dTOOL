// DTermCore.swift
// Swift bindings for dterm-core terminal emulation library
//
// Usage:
//   1. Copy this file to your Xcode project
//   2. Add dterm.h to your bridging header
//   3. Link against libdterm_core.a
//
// Example:
//   let terminal = DTermCore(rows: 24, cols: 80)
//   terminal.process(ptyData)
//   for row in 0..<terminal.rows {
//       for col in 0..<terminal.cols {
//           if let cell = terminal.cell(at: row, col: col) {
//               // Render cell
//           }
//       }
//   }

import Foundation

// MARK: - DTermCore

/// High-performance terminal emulator core.
///
/// Thread Safety: NOT thread-safe. Use external synchronization if needed.
public final class DTermCore {
    fileprivate var terminal: OpaquePointer?

    /// Create a new terminal with default scrollback.
    public init(rows: UInt16, cols: UInt16) {
        terminal = dterm_terminal_new(rows, cols)
    }

    /// Create a new terminal with custom scrollback configuration.
    ///
    /// - Parameters:
    ///   - rows: Number of visible rows
    ///   - cols: Number of columns
    ///   - config: Scrollback configuration
    public init(rows: UInt16, cols: UInt16, scrollback config: ScrollbackConfig) {
        terminal = dterm_terminal_new_with_scrollback(
            rows,
            cols,
            UInt(config.ringBufferSize),
            UInt(config.hotLimit),
            UInt(config.warmLimit),
            UInt(config.memoryBudget)
        )
    }

    /// Create a terminal wrapping an existing pointer (for checkpoint restore).
    fileprivate init(restoredPointer: OpaquePointer) {
        terminal = restoredPointer
    }

    deinit {
        if let terminal = terminal {
            dterm_terminal_free(terminal)
        }
    }

    // MARK: - Processing

    /// Process PTY output data.
    ///
    /// - Parameter data: Raw bytes from PTY
    public func process(_ data: Data) {
        guard let terminal = terminal else { return }
        data.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            dterm_terminal_process(
                terminal,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                UInt(ptr.count)
            )
        }
    }

    /// Process PTY output bytes.
    ///
    /// - Parameters:
    ///   - bytes: Pointer to byte data
    ///   - count: Number of bytes
    public func process(bytes: UnsafePointer<UInt8>, count: Int) {
        guard let terminal = terminal else { return }
        dterm_terminal_process(terminal, bytes, UInt(count))
    }

    // MARK: - Dimensions

    /// Number of visible rows.
    public var rows: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_rows(terminal)
    }

    /// Number of columns.
    public var cols: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cols(terminal)
    }

    /// Resize the terminal.
    public func resize(rows: UInt16, cols: UInt16) {
        guard let terminal = terminal else { return }
        dterm_terminal_resize(terminal, rows, cols)
    }

    // MARK: - Cursor

    /// Cursor row (0-indexed).
    public var cursorRow: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cursor_row(terminal)
    }

    /// Cursor column (0-indexed).
    public var cursorCol: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cursor_col(terminal)
    }

    /// Whether cursor is visible (DECTCEM).
    public var cursorVisible: Bool {
        guard let terminal = terminal else { return true }
        return dterm_terminal_cursor_visible(terminal)
    }

    // MARK: - Cells

    /// Get cell at position.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed)
    ///   - col: Column index (0-indexed)
    /// - Returns: Cell data, or nil if out of bounds
    public func cell(at row: UInt16, col: UInt16) -> DTermCell? {
        guard let terminal = terminal else { return nil }
        var cell = dterm_cell_t()
        if dterm_terminal_get_cell(terminal, row, col, &cell) {
            return DTermCell(cell)
        }
        return nil
    }

    /// Enumerate all visible cells.
    ///
    /// - Parameter handler: Called for each cell with (row, col, cell)
    public func enumerateCells(_ handler: (UInt16, UInt16, DTermCell) -> Void) {
        guard let terminal = terminal else { return }
        var cell = dterm_cell_t()
        for row in 0..<rows {
            for col in 0..<cols {
                if dterm_terminal_get_cell(terminal, row, col, &cell) {
                    handler(row, col, DTermCell(cell))
                }
            }
        }
    }

    // MARK: - Modes

    /// Terminal modes.
    public var modes: DTermModes {
        guard let terminal = terminal else { return DTermModes() }
        var modes = dterm_modes_t()
        dterm_terminal_get_modes(terminal, &modes)
        return DTermModes(modes)
    }

    /// Whether alternate screen buffer is active.
    public var isAlternateScreen: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_is_alternate_screen(terminal)
    }

    // MARK: - Title

    /// Window title set by OSC 0/2 sequences.
    public var title: String? {
        guard let terminal = terminal,
              let cStr = dterm_terminal_title(terminal) else {
            return nil
        }
        return String(cString: cStr)
    }

    // MARK: - Scrolling

    /// Total lines in scrollback history.
    public var scrollbackLines: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_scrollback_lines(terminal))
    }

    /// Current display offset (0 = at bottom).
    public var displayOffset: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_display_offset(terminal))
    }

    /// Scroll display by delta lines.
    ///
    /// - Parameter lines: Positive = scroll up, negative = scroll down
    public func scroll(lines: Int32) {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_display(terminal, lines)
    }

    /// Scroll to top of scrollback.
    public func scrollToTop() {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_to_top(terminal)
    }

    /// Scroll to bottom (live content).
    public func scrollToBottom() {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_to_bottom(terminal)
    }

    // MARK: - Damage Tracking

    /// Whether terminal content has changed since last render.
    public var needsRedraw: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_needs_redraw(terminal)
    }

    /// Clear damage tracking after rendering.
    public func clearDamage() {
        guard let terminal = terminal else { return }
        dterm_terminal_clear_damage(terminal)
    }

    // MARK: - Reset

    /// Reset terminal to initial state.
    public func reset() {
        guard let terminal = terminal else { return }
        dterm_terminal_reset(terminal)
    }
}

// MARK: - ScrollbackConfig

/// Configuration for scrollback storage.
public struct ScrollbackConfig {
    /// Size of fast ring buffer for recent lines.
    public var ringBufferSize: Int

    /// Maximum lines in hot tier (uncompressed, fast access).
    public var hotLimit: Int

    /// Maximum lines in warm tier (LZ4 compressed).
    public var warmLimit: Int

    /// Total memory budget in bytes.
    public var memoryBudget: Int

    /// Default configuration (100K lines, 100MB budget).
    public static var `default`: ScrollbackConfig {
        ScrollbackConfig(
            ringBufferSize: 10000,
            hotLimit: 1000,
            warmLimit: 10000,
            memoryBudget: 100 * 1024 * 1024
        )
    }

    /// High-capacity configuration (1M+ lines, 500MB budget).
    public static var highCapacity: ScrollbackConfig {
        ScrollbackConfig(
            ringBufferSize: 100000,
            hotLimit: 10000,
            warmLimit: 100000,
            memoryBudget: 500 * 1024 * 1024
        )
    }

    public init(
        ringBufferSize: Int,
        hotLimit: Int,
        warmLimit: Int,
        memoryBudget: Int
    ) {
        self.ringBufferSize = ringBufferSize
        self.hotLimit = hotLimit
        self.warmLimit = warmLimit
        self.memoryBudget = memoryBudget
    }
}

// MARK: - DTermCell

/// Cell data from terminal grid.
public struct DTermCell {
    /// Unicode codepoint (nil for empty cell).
    public let codepoint: UnicodeScalar?

    /// Foreground color.
    public let foreground: DTermColor

    /// Background color.
    public let background: DTermColor

    /// Cell attributes (bold, italic, etc.).
    public let flags: CellFlags

    init(_ cell: dterm_cell_t) {
        if cell.codepoint > 0 {
            self.codepoint = UnicodeScalar(cell.codepoint)
        } else {
            self.codepoint = nil
        }
        self.foreground = DTermColor(packed: cell.fg)
        self.background = DTermColor(packed: cell.bg)
        self.flags = CellFlags(rawValue: cell.flags)
    }

    /// Character representation.
    public var character: Character? {
        guard let scalar = codepoint else { return nil }
        return Character(scalar)
    }

    /// Whether cell is empty (space or no content).
    public var isEmpty: Bool {
        guard let cp = codepoint else { return true }
        return cp == " " || cp == UnicodeScalar(0)
    }

    /// Whether this is a wide character.
    public var isWide: Bool {
        flags.contains(.wide)
    }

    /// Whether this is a spacer for a wide character.
    public var isWideSpacer: Bool {
        flags.contains(.wideSpacer)
    }
}

// MARK: - CellFlags

/// Cell attribute flags.
public struct CellFlags: OptionSet {
    public let rawValue: UInt16

    public init(rawValue: UInt16) {
        self.rawValue = rawValue
    }

    public static let bold = CellFlags(rawValue: 1 << 0)
    public static let dim = CellFlags(rawValue: 1 << 1)
    public static let italic = CellFlags(rawValue: 1 << 2)
    public static let underline = CellFlags(rawValue: 1 << 3)
    public static let blink = CellFlags(rawValue: 1 << 4)
    public static let inverse = CellFlags(rawValue: 1 << 5)
    public static let invisible = CellFlags(rawValue: 1 << 6)
    public static let strikethrough = CellFlags(rawValue: 1 << 7)
    public static let wide = CellFlags(rawValue: 1 << 8)
    public static let wideSpacer = CellFlags(rawValue: 1 << 9)
}

// MARK: - DTermColor

/// Terminal color representation.
public enum DTermColor: Equatable {
    /// Default foreground/background color.
    case `default`

    /// Indexed color (0-255).
    case indexed(UInt8)

    /// True color RGB.
    case rgb(r: UInt8, g: UInt8, b: UInt8)

    init(packed: UInt32) {
        if packed == 0 {
            self = .default
        } else {
            let type = packed >> 24
            if type == 0x01 {
                self = .indexed(UInt8(packed & 0xFF))
            } else {
                self = .rgb(
                    r: UInt8((packed >> 16) & 0xFF),
                    g: UInt8((packed >> 8) & 0xFF),
                    b: UInt8(packed & 0xFF)
                )
            }
        }
    }

    /// Convert to NSColor (macOS).
    #if canImport(AppKit)
    public func toNSColor(palette: [NSColor]? = nil) -> NSColor {
        switch self {
        case .default:
            return .textColor
        case .indexed(let index):
            if let palette = palette, index < palette.count {
                return palette[Int(index)]
            }
            return defaultPaletteColor(index)
        case .rgb(let r, let g, let b):
            return NSColor(
                calibratedRed: CGFloat(r) / 255,
                green: CGFloat(g) / 255,
                blue: CGFloat(b) / 255,
                alpha: 1
            )
        }
    }

    private func defaultPaletteColor(_ index: UInt8) -> NSColor {
        // Standard 16 colors
        let colors: [NSColor] = [
            .black, .red, .green, .yellow,
            .blue, .magenta, .cyan, .white,
            .darkGray, .systemRed, .systemGreen, .systemYellow,
            .systemBlue, .systemPurple, .systemTeal, .white
        ]
        if index < 16 {
            return colors[Int(index)]
        }
        // 216 color cube (16-231)
        if index < 232 {
            let i = Int(index) - 16
            let r = (i / 36) * 51
            let g = ((i / 6) % 6) * 51
            let b = (i % 6) * 51
            return NSColor(
                calibratedRed: CGFloat(r) / 255,
                green: CGFloat(g) / 255,
                blue: CGFloat(b) / 255,
                alpha: 1
            )
        }
        // Grayscale (232-255)
        let gray = (Int(index) - 232) * 10 + 8
        return NSColor(
            calibratedWhite: CGFloat(gray) / 255,
            alpha: 1
        )
    }
    #endif
}

// MARK: - DTermModes

/// Terminal mode flags.
public struct DTermModes {
    /// Cursor visible (DECTCEM).
    public var cursorVisible: Bool = true

    /// Application cursor keys mode (DECCKM).
    public var applicationCursorKeys: Bool = false

    /// Alternate screen buffer active.
    public var alternateScreen: Bool = false

    /// Auto-wrap mode (DECAWM).
    public var autoWrap: Bool = true

    /// Origin mode (DECOM).
    public var originMode: Bool = false

    /// Insert mode (IRM).
    public var insertMode: Bool = false

    /// Bracketed paste mode.
    public var bracketedPaste: Bool = false

    public init() {}

    init(_ modes: dterm_modes_t) {
        self.cursorVisible = modes.cursor_visible
        self.applicationCursorKeys = modes.application_cursor_keys
        self.alternateScreen = modes.alternate_screen
        self.autoWrap = modes.auto_wrap
        self.originMode = modes.origin_mode
        self.insertMode = modes.insert_mode
        self.bracketedPaste = modes.bracketed_paste
    }
}

// MARK: - Version

/// Get dterm-core library version.
public func dtermVersion() -> String {
    guard let cStr = dterm_version() else { return "unknown" }
    return String(cString: cStr)
}

// MARK: - DTermSearch

/// High-performance trigram-indexed search with bloom filter.
///
/// Thread Safety: NOT thread-safe. Use external synchronization if needed.
public final class DTermSearch {
    private var search: OpaquePointer?

    /// Create a new search index.
    public init() {
        search = dterm_search_new()
    }

    /// Create a new search index with expected capacity.
    ///
    /// - Parameter expectedLines: Expected number of lines to index
    public init(expectedLines: Int) {
        search = dterm_search_with_capacity(UInt(expectedLines))
    }

    deinit {
        if let search = search {
            dterm_search_free(search)
        }
    }

    // MARK: - Indexing

    /// Index a line of text for searching.
    ///
    /// - Parameter line: The line text to index
    public func indexLine(_ line: String) {
        guard let search = search else { return }
        line.withCString { ptr in
            dterm_search_index_line(search, UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self), UInt(line.utf8.count))
        }
    }

    /// Number of indexed lines.
    public var lineCount: Int {
        guard let search = search else { return 0 }
        return Int(dterm_search_line_count(search))
    }

    /// Clear the search index.
    public func clear() {
        guard let search = search else { return }
        dterm_search_clear(search)
    }

    // MARK: - Searching

    /// Fast check if a query might have matches (bloom filter).
    ///
    /// Returns false if definitely no matches exist.
    /// Returns true if matches are possible (verify with actual search).
    ///
    /// - Parameter query: Search query string
    /// - Returns: Whether matches might exist
    public func mightContain(_ query: String) -> Bool {
        guard let search = search else { return false }
        return query.withCString { ptr in
            dterm_search_might_contain(
                search,
                UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self),
                UInt(query.utf8.count)
            )
        }
    }

    /// Find all matches for a query.
    ///
    /// - Parameters:
    ///   - query: Search query string
    ///   - maxResults: Maximum number of results to return
    /// - Returns: Array of search matches
    public func find(_ query: String, maxResults: Int = 1000) -> [DTermSearchMatch] {
        guard let search = search else { return [] }

        return query.withCString { ptr -> [DTermSearchMatch] in
            var matches = [dterm_search_match_t](repeating: dterm_search_match_t(), count: maxResults)
            let count = dterm_search_find(
                search,
                UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self),
                UInt(query.utf8.count),
                &matches,
                UInt(maxResults)
            )
            return matches.prefix(Int(count)).map { DTermSearchMatch($0) }
        }
    }

    /// Find the next match after a given position.
    ///
    /// - Parameters:
    ///   - query: Search query string
    ///   - afterLine: Line number to search after
    ///   - afterCol: Column to search after
    /// - Returns: The next match, or nil if none found
    public func findNext(_ query: String, afterLine: Int, afterCol: Int = 0) -> DTermSearchMatch? {
        guard let search = search else { return nil }

        return query.withCString { ptr -> DTermSearchMatch? in
            var match = dterm_search_match_t()
            let found = dterm_search_find_next(
                search,
                UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self),
                UInt(query.utf8.count),
                UInt(afterLine),
                UInt(afterCol),
                &match
            )
            return found ? DTermSearchMatch(match) : nil
        }
    }

    /// Find the previous match before a given position.
    ///
    /// - Parameters:
    ///   - query: Search query string
    ///   - beforeLine: Line number to search before
    ///   - beforeCol: Column to search before
    /// - Returns: The previous match, or nil if none found
    public func findPrev(_ query: String, beforeLine: Int, beforeCol: Int = Int.max) -> DTermSearchMatch? {
        guard let search = search else { return nil }

        return query.withCString { ptr -> DTermSearchMatch? in
            var match = dterm_search_match_t()
            let found = dterm_search_find_prev(
                search,
                UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self),
                UInt(query.utf8.count),
                UInt(beforeLine),
                UInt(beforeCol),
                &match
            )
            return found ? DTermSearchMatch(match) : nil
        }
    }
}

// MARK: - DTermSearchMatch

/// A search match result.
public struct DTermSearchMatch {
    /// Line number (0-indexed).
    public let line: Int

    /// Starting column of the match (0-indexed).
    public let startColumn: Int

    /// Ending column of the match (exclusive).
    public let endColumn: Int

    init(_ match: dterm_search_match_t) {
        self.line = Int(match.line)
        self.startColumn = Int(match.start_col)
        self.endColumn = Int(match.end_col)
    }

    /// Range of columns containing the match.
    public var columnRange: Range<Int> {
        startColumn..<endColumn
    }
}

// MARK: - DTermCheckpoint

/// Session checkpoint manager for crash recovery.
///
/// Thread Safety: NOT thread-safe. Use external synchronization if needed.
public final class DTermCheckpoint {
    private var checkpoint: OpaquePointer?

    /// Create a new checkpoint manager.
    ///
    /// - Parameter directory: Directory to store checkpoint files
    public init(directory: String) {
        checkpoint = directory.withCString { ptr in
            dterm_checkpoint_new(
                UnsafeRawPointer(ptr).assumingMemoryBound(to: UInt8.self),
                UInt(directory.utf8.count)
            )
        }
    }

    /// Create a new checkpoint manager with URL.
    ///
    /// - Parameter directory: Directory URL to store checkpoint files
    public convenience init(directory: URL) {
        self.init(directory: directory.path)
    }

    deinit {
        if let checkpoint = checkpoint {
            dterm_checkpoint_free(checkpoint)
        }
    }

    // MARK: - Status

    /// Check if a checkpoint should be performed based on time and line thresholds.
    public var shouldSave: Bool {
        guard let checkpoint = checkpoint else { return false }
        return dterm_checkpoint_should_save(checkpoint)
    }

    /// Check if a valid checkpoint exists.
    public var exists: Bool {
        guard let checkpoint = checkpoint else { return false }
        return dterm_checkpoint_exists(checkpoint)
    }

    /// Notify the checkpoint manager that lines were added.
    ///
    /// - Parameter count: Number of lines added
    public func notifyLines(_ count: Int) {
        guard let checkpoint = checkpoint else { return }
        dterm_checkpoint_notify_lines(checkpoint, UInt(count))
    }

    // MARK: - Save/Restore

    /// Save a checkpoint of the terminal state.
    ///
    /// - Parameter terminal: The terminal to checkpoint
    /// - Returns: Whether the save succeeded
    @discardableResult
    public func save(terminal: DTermCore) -> Bool {
        guard let checkpoint = checkpoint,
              let termPtr = terminal.terminal else { return false }
        return dterm_checkpoint_save(checkpoint, termPtr)
    }

    /// Restore terminal state from the latest checkpoint.
    ///
    /// - Returns: A restored terminal, or nil if restore failed
    public func restore() -> DTermCore? {
        guard let checkpoint = checkpoint,
              let termPtr = dterm_checkpoint_restore(checkpoint) else { return nil }
        return DTermCore(restoredPointer: termPtr)
    }
}

