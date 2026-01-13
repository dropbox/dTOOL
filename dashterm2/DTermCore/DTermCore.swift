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
    private var terminal: OpaquePointer?

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

    // MARK: - Scrollback Cell Access

    /// Get cell from scrollback buffer (tiered scrollback).
    ///
    /// - Parameters:
    ///   - scrollbackRow: Row index in tiered scrollback (0 = oldest line)
    ///   - col: Column index (0-indexed)
    /// - Returns: Cell data, or nil if out of bounds
    public func scrollbackCell(at scrollbackRow: Int, col: UInt16) -> DTermScrollbackCell? {
        guard let terminal = terminal else { return nil }
        var cell = DtermScrollbackCell()
        if dterm_terminal_get_scrollback_cell(terminal, UInt(scrollbackRow), col, &cell) {
            return DTermScrollbackCell(cell)
        }
        return nil
    }

    /// Get scrollback line length (number of characters).
    ///
    /// - Parameter scrollbackRow: Row index in tiered scrollback (0 = oldest line)
    /// - Returns: Number of characters in the line, or 0 if out of bounds
    public func scrollbackLineLength(at scrollbackRow: Int) -> Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_scrollback_line_len(terminal, UInt(scrollbackRow)))
    }

    /// Check if scrollback line is wrapped (continuation of previous line).
    ///
    /// - Parameter scrollbackRow: Row index in tiered scrollback (0 = oldest line)
    /// - Returns: true if the line is wrapped, false otherwise
    public func scrollbackLineIsWrapped(at scrollbackRow: Int) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_scrollback_line_wrapped(terminal, UInt(scrollbackRow))
    }

    /// Get hyperlink URL from a scrollback cell.
    ///
    /// - Parameters:
    ///   - scrollbackRow: Row index in scrollback (0 = most recent scrollback line)
    ///   - col: Column index (0-indexed)
    /// - Returns: URL string, or nil if no hyperlink
    public func scrollbackHyperlinkAt(scrollbackRow: Int, col: UInt16) -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_scrollback_cell_hyperlink(terminal, UInt(scrollbackRow), col) else {
            return nil
        }
        return String(cString: cStr)
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

    /// Get all damaged rows with column bounds.
    public func getDamage(maxCount: Int = 100) -> [DTermRowDamage] {
        guard let terminal = terminal else { return [] }
        var damages = [DtermRowDamage](repeating: DtermRowDamage(), count: maxCount)
        let count = dterm_terminal_get_damage(terminal, &damages, UInt(maxCount))
        return (0..<Int(count)).map { DTermRowDamage(damages[$0]) }
    }

    /// Check if a specific row is damaged.
    public func rowIsDamaged(_ row: UInt16) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_row_is_damaged(terminal, row)
    }

    /// Get damage bounds for a specific row.
    public func getRowDamage(_ row: UInt16) -> (left: UInt16, right: UInt16)? {
        guard let terminal = terminal else { return nil }
        var left: UInt16 = 0
        var right: UInt16 = 0
        if dterm_terminal_get_row_damage(terminal, row, &left, &right) {
            return (left, right)
        }
        return nil
    }

    // MARK: - Line Content Extraction

    /// Total number of lines (visible + scrollback).
    public var totalLines: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_total_lines(terminal))
    }

    /// Get text content of a line by index.
    public func getLineText(lineIndex: Int) -> String {
        guard let terminal = terminal else { return "" }
        let size = dterm_terminal_get_line_text(terminal, UInt(lineIndex), nil, 0)
        guard size > 0 else { return "" }
        var buffer = [UInt8](repeating: 0, count: Int(size) + 1)
        _ = dterm_terminal_get_line_text(terminal, UInt(lineIndex), &buffer, UInt(buffer.count))
        return String(cString: buffer)
    }

    /// Get text content of a visible row.
    public func getVisibleLineText(row: UInt16) -> String {
        guard let terminal = terminal else { return "" }
        let size = dterm_terminal_get_visible_line_text(terminal, row, nil, 0)
        guard size > 0 else { return "" }
        var buffer = [UInt8](repeating: 0, count: Int(size) + 1)
        _ = dterm_terminal_get_visible_line_text(terminal, row, &buffer, UInt(buffer.count))
        return String(cString: buffer)
    }

    // MARK: - Reset

    /// Reset terminal to initial state.
    public func reset() {
        guard let terminal = terminal else { return }
        dterm_terminal_reset(terminal)
    }

    // MARK: - Screen Alignment Test

    /// Fill screen with 'E' characters for alignment test (DECALN).
    ///
    /// This is the DEC Screen Alignment Pattern (ESC # 8). It fills the entire
    /// visible screen with uppercase 'E' characters using default attributes.
    /// Used for testing screen alignment and checking character spacing.
    public func screenAlignmentTest() {
        guard let terminal = terminal else { return }
        dterm_terminal_screen_alignment_test(terminal)
    }

    // MARK: - Display Modes

    /// Whether reverse video mode is enabled (DECSCNM, mode 5).
    ///
    /// When enabled, the renderer should swap foreground and background colors
    /// for the entire screen.
    public var isReverseVideo: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_is_reverse_video(terminal)
    }

    /// Whether cursor blink is enabled (mode 12).
    public var cursorBlinkEnabled: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_cursor_blink_enabled(terminal)
    }

    /// Whether application keypad mode is enabled (DECNKM, mode 66).
    ///
    /// When enabled, the numeric keypad sends application sequences instead
    /// of numeric characters.
    public var applicationKeypadEnabled: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_application_keypad_enabled(terminal)
    }

    /// Whether 132-column mode is enabled (DECCOLM, mode 3).
    public var is132ColumnMode: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_is_132_column_mode(terminal)
    }

    /// Whether reverse wraparound mode is enabled (DECSET 45).
    ///
    /// When enabled, backspace at column 0 wraps to the end of the previous line.
    /// This is useful for line-editing applications that need to move backwards
    /// across line boundaries.
    public var isReverseWraparound: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_is_reverse_wraparound(terminal)
    }

    // MARK: - Hyperlinks (OSC 8)

    /// Get hyperlink URL at a cell position.
    ///
    /// OSC 8 hyperlinks allow terminal applications to create clickable links.
    /// - Parameters:
    ///   - row: Row index (0-indexed)
    ///   - col: Column index (0-indexed)
    /// - Returns: URL string, or nil if no hyperlink at this cell
    public func hyperlinkAt(row: UInt16, col: UInt16) -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_cell_hyperlink(terminal, row, col) else {
            return nil
        }
        return String(cString: cStr)
    }

    /// Check if a cell has a hyperlink.
    ///
    /// This is faster than `hyperlinkAt` when you only need to know if a
    /// hyperlink exists, not what the URL is.
    /// - Returns: true if cell has hyperlink
    public func hasHyperlinkAt(row: UInt16, col: UInt16) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_cell_has_hyperlink(terminal, row, col)
    }

    /// Get the current active hyperlink URL being applied to new text.
    ///
    /// When an OSC 8 hyperlink sequence is received, subsequent text will have
    /// the hyperlink applied until the hyperlink is cleared.
    /// - Returns: Active hyperlink URL, or nil if no active hyperlink
    public func currentHyperlink() -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_current_hyperlink(terminal) else {
            return nil
        }
        return String(cString: cStr)
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

// MARK: - DTermScrollbackCell

/// Cell data from tiered scrollback storage.
///
/// This is a lightweight representation of a cell that has been evicted
/// from the ring buffer into tiered scrollback. It contains character and
/// style information but not the full cell metadata.
public struct DTermScrollbackCell {
    /// Unicode codepoint (nil for empty cell).
    public let codepoint: UnicodeScalar?

    /// Foreground color.
    public let foreground: DTermColor

    /// Background color.
    public let background: DTermColor

    /// Cell attributes (bold, italic, etc.).
    public let flags: CellFlags

    init(_ cell: DtermScrollbackCell) {
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

    /// Underline color (SGR 58/59). Nil means use foreground color.
    public let underlineColor: DTermColor?

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
        // 0xFFFFFFFF means no custom underline color (use foreground)
        if cell.underline_color == 0xFFFF_FFFF {
            self.underlineColor = nil
        } else {
            self.underlineColor = DTermColor(packed: cell.underline_color)
        }
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

// MARK: - DTermRowDamage

/// Damage information for a single row.
public struct DTermRowDamage {
    /// Row index (0 = top of visible area).
    public let row: UInt16
    /// First damaged column (inclusive).
    public let left: UInt16
    /// Last damaged column (exclusive).
    public let right: UInt16

    init(_ damage: DtermRowDamage) {
        self.row = damage.row
        self.left = damage.left
        self.right = damage.right
    }
}

// MARK: - CellFlags

/// Cell attribute flags.
///
/// Bit layout matches dterm-core grid/cell.rs CellFlags:
/// - Bit 0: BOLD
/// - Bit 1: DIM
/// - Bit 2: ITALIC
/// - Bit 3: UNDERLINE
/// - Bit 4: BLINK
/// - Bit 5: INVERSE
/// - Bit 6: HIDDEN (invisible)
/// - Bit 7: STRIKETHROUGH
/// - Bit 8: DOUBLE_UNDERLINE
/// - Bit 9: WIDE
/// - Bit 10: WIDE_CONTINUATION (spacer)
/// - Bit 11: SUPERSCRIPT (SGR 73)
/// - Bit 12: SUBSCRIPT (SGR 74)
public struct CellFlags: OptionSet {
    public let rawValue: UInt16

    public init(rawValue: UInt16) {
        self.rawValue = rawValue
    }

    // Correct bit positions from dterm-core grid/cell.rs CellFlags
    public static let bold = CellFlags(rawValue: 1 << 0)
    public static let dim = CellFlags(rawValue: 1 << 1)
    public static let italic = CellFlags(rawValue: 1 << 2)
    public static let underline = CellFlags(rawValue: 1 << 3)
    public static let blink = CellFlags(rawValue: 1 << 4)
    public static let inverse = CellFlags(rawValue: 1 << 5)
    public static let invisible = CellFlags(rawValue: 1 << 6)
    public static let strikethrough = CellFlags(rawValue: 1 << 7)
    public static let doubleUnderline = CellFlags(rawValue: 1 << 8)
    public static let wide = CellFlags(rawValue: 1 << 9)
    public static let wideSpacer = CellFlags(rawValue: 1 << 10)
    public static let superscript = CellFlags(rawValue: 1 << 11)
    public static let `subscript` = CellFlags(rawValue: 1 << 12)
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
        // Packed format from dterm-core (cell.rs):
        // - 0x00_INDEX__: Indexed color (0-255) - type byte is 0x00
        // - 0x01_RRGGBB: True color RGB - type byte is 0x01
        // - 0xFF_______: Default color - type byte is 0xFF
        let type = packed >> 24
        switch type {
        case 0xFF:
            self = .default
        case 0x00:
            self = .indexed(UInt8(packed & 0xFF))
        case 0x01:
            self = .rgb(
                r: UInt8((packed >> 16) & 0xFF),
                g: UInt8((packed >> 8) & 0xFF),
                b: UInt8(packed & 0xFF)
            )
        default:
            // Unknown type, treat as default
            self = .default
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

    /// Cursor style (DECSCUSR). Values match DECSCUSR parameters (1-6).
    public var cursorStyle: UInt8 = 1

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

    /// Mouse tracking mode (1000/1002/1003).
    public var mouseMode: MouseMode = .none

    /// Mouse encoding format (X10 or SGR/1006).
    public var mouseEncoding: MouseEncoding = .x10

    /// Focus reporting mode (1004).
    public var focusReporting: Bool = false

    /// Synchronized output mode (2026).
    /// When enabled, rendering should be deferred to prevent tearing.
    public var synchronizedOutput: Bool = false

    /// Reverse video mode (DECSCNM, mode 5).
    /// When enabled, foreground and background colors are swapped for the entire screen.
    public var reverseVideo: Bool = false

    /// Cursor blink mode (mode 12).
    /// When enabled, cursor should blink.
    public var cursorBlink: Bool = false

    /// Application keypad mode (DECNKM, mode 66).
    /// When enabled, numeric keypad sends application sequences instead of digits.
    public var applicationKeypad: Bool = false

    /// 132-column mode (DECCOLM, mode 3).
    /// When enabled, terminal is in 132-column mode; when disabled, 80-column mode.
    public var columnMode132: Bool = false

    /// Reverse wraparound mode (DECSET 45).
    /// When enabled, backspace at column 0 wraps to the end of the previous line.
    public var reverseWraparound: Bool = false

    public init() {}

    init(_ modes: dterm_modes_t) {
        self.cursorVisible = modes.cursor_visible
        self.cursorStyle = modes.cursor_style
        self.applicationCursorKeys = modes.application_cursor_keys
        self.alternateScreen = modes.alternate_screen
        self.autoWrap = modes.auto_wrap
        self.originMode = modes.origin_mode
        self.insertMode = modes.insert_mode
        self.bracketedPaste = modes.bracketed_paste
        self.mouseMode = MouseMode(ffi: modes.mouse_mode)
        self.mouseEncoding = MouseEncoding(ffi: modes.mouse_encoding)
        self.focusReporting = modes.focus_reporting
        self.synchronizedOutput = modes.synchronized_output
        self.reverseVideo = modes.reverse_video
        self.cursorBlink = modes.cursor_blink
        self.applicationKeypad = modes.application_keypad
        self.columnMode132 = modes.column_mode_132
        self.reverseWraparound = modes.reverse_wraparound
    }
}

// MARK: - Mouse Mode

/// Mouse tracking mode.
public enum MouseMode: Equatable {
    /// No mouse tracking (default).
    case none
    /// Normal tracking mode (1000) - report button press/release.
    case normal
    /// Button-event tracking mode (1002) - report press/release and motion while button pressed.
    case buttonEvent
    /// Any-event tracking mode (1003) - report all motion events.
    case anyEvent

    init(ffi: DtermMouseMode) {
        switch ffi {
        case NONE: self = .none
        case NORMAL: self = .normal
        case BUTTON_EVENT: self = .buttonEvent
        case ANY_EVENT: self = .anyEvent
        default: self = .none
        }
    }
}

/// Mouse encoding format.
public enum MouseEncoding: Equatable {
    /// X10 compatibility mode - coordinates encoded as single bytes (limited to 223).
    case x10
    /// SGR encoding (1006) - coordinates as decimal parameters, supports larger values.
    case sgr

    init(ffi: DtermMouseEncoding) {
        switch ffi {
        case X10: self = .x10
        case SGR: self = .sgr
        default: self = .x10
        }
    }
}

// MARK: - Version

/// Get dterm-core library version.
public func dtermVersion() -> String {
    guard let cStr = dterm_version() else { return "unknown" }
    return String(cString: cStr)
}

// MARK: - Clipboard Support (OSC 52)

/// Clipboard selection target for OSC 52.
///
/// OSC 52 specifies which clipboard/selection buffer to operate on:
/// - `.clipboard` ('c'): System clipboard
/// - `.primary` ('p'): X11 primary selection
/// - `.secondary` ('q'): Secondary selection
/// - `.select` ('s'): Select buffer
/// - `.cutBuffer0` through `.cutBuffer7`: Cut buffers 0-7
public enum ClipboardSelection: Equatable {
    case clipboard
    case primary
    case secondary
    case select
    case cutBuffer(UInt8)

    init?(ffi: DtermClipboardSelection) {
        switch ffi {
        case CLIPBOARD: self = .clipboard
        case PRIMARY: self = .primary
        case SECONDARY: self = .secondary
        case SELECT: self = .select
        case CUT_BUFFER0: self = .cutBuffer(0)
        case CUT_BUFFER1: self = .cutBuffer(1)
        case CUT_BUFFER2: self = .cutBuffer(2)
        case CUT_BUFFER3: self = .cutBuffer(3)
        case CUT_BUFFER4: self = .cutBuffer(4)
        case CUT_BUFFER5: self = .cutBuffer(5)
        case CUT_BUFFER6: self = .cutBuffer(6)
        case CUT_BUFFER7: self = .cutBuffer(7)
        default: return nil
        }
    }
}

/// Clipboard operation type.
public enum ClipboardOperationType {
    /// Set clipboard content
    case set
    /// Query clipboard content
    case query
    /// Clear clipboard
    case clear
}

/// Clipboard operation from OSC 52.
public struct ClipboardOperation {
    /// Operation type (set, query, or clear)
    public let type: ClipboardOperationType
    /// Target selections
    public let selections: [ClipboardSelection]
    /// Content to set (only for `.set` operations)
    public let content: String?
}

/// Protocol for handling clipboard operations.
///
/// Implement this protocol and set it via `DTermCore.clipboardHandler`
/// to receive OSC 52 clipboard requests from applications.
public protocol DTermClipboardHandler: AnyObject {
    /// Called when an application requests a clipboard operation.
    ///
    /// - Parameter operation: The clipboard operation
    /// - Returns: For `.query` operations, return the clipboard content.
    ///           Return `nil` to deny access or for `.set`/`.clear` operations.
    func handleClipboard(_ operation: ClipboardOperation) -> String?
}

// MARK: - DTermCore Clipboard Extension

extension DTermCore {
    /// Set the clipboard handler for OSC 52 operations.
    ///
    /// The handler is called when applications send OSC 52 sequences to
    /// set, query, or clear the clipboard.
    ///
    /// - Parameter handler: The handler, or nil to disable clipboard support
    public func setClipboardHandler(_ handler: DTermClipboardHandler?) {
        guard let terminal = terminal else { return }

        if let handler = handler {
            // Store handler reference to prevent deallocation
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()

            let callback: DtermClipboardCallback = { contextPtr, opPtr, responseBuffer, responseBufferLen in
                guard let contextPtr = contextPtr,
                      let opPtr = opPtr else { return 0 }

                let handler = Unmanaged<AnyObject>.fromOpaque(contextPtr).takeUnretainedValue()
                guard let clipboardHandler = handler as? DTermClipboardHandler else { return 0 }

                let op = opPtr.pointee

                // Convert operation type
                let opType: ClipboardOperationType
                switch op.op_type {
                case SET: opType = .set
                case QUERY: opType = .query
                case CLEAR: opType = .clear
                default: return 0
                }

                // Convert selections
                var selections: [ClipboardSelection] = []
                for i in 0..<Int(op.selection_count) {
                    if let sel = ClipboardSelection(ffi: op.selections.0) {
                        // Note: We need to access the tuple elements properly
                        // The selections array is a fixed-size C array accessed as a tuple
                        selections.append(sel)
                    }
                }
                // Properly access the selections tuple
                let selectionsArray = withUnsafeBytes(of: op.selections) { ptr in
                    let base = ptr.baseAddress!.assumingMemoryBound(to: DtermClipboardSelection.self)
                    return (0..<Int(op.selection_count)).compactMap { i in
                        ClipboardSelection(ffi: base[i])
                    }
                }

                // Get content for Set operations
                var content: String?
                if opType == .set, op.content_len > 0, let contentPtr = op.content {
                    content = String(
                        bytesNoCopy: UnsafeMutableRawPointer(mutating: contentPtr),
                        length: Int(op.content_len),
                        encoding: .utf8,
                        freeWhenDone: false
                    )
                }

                let operation = ClipboardOperation(
                    type: opType,
                    selections: selectionsArray,
                    content: content
                )

                // Call handler
                guard let response = clipboardHandler.handleClipboard(operation) else {
                    return 0
                }

                // For Query operations, copy response to buffer
                if opType == .query, let responseBuffer = responseBuffer {
                    let responseData = response.utf8
                    let copyLen = min(responseData.count, Int(responseBufferLen))
                    responseData.withContiguousStorageIfAvailable { bytes in
                        memcpy(responseBuffer, bytes.baseAddress, copyLen)
                    } ?? {
                        // Fallback for non-contiguous storage
                        Array(responseData).withUnsafeBytes { bytes in
                            memcpy(responseBuffer, bytes.baseAddress!, copyLen)
                        }
                    }()
                    return UInt(copyLen)
                }

                return 0
            }

            dterm_terminal_set_clipboard_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_clipboard_callback(terminal, nil, nil)
        }
    }
}

// MARK: - Search Index

/// High-performance search index for terminal content.
///
/// Uses trigram indexing with bloom filter for fast search operations.
/// Thread Safety: NOT thread-safe. Use external synchronization if needed.
public final class DTermSearch {
    private var search: OpaquePointer?

    /// Create a new search index.
    public init() {
        search = dterm_search_new()
    }

    /// Create a new search index with expected line capacity.
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

    /// Index a line of text.
    ///
    /// - Parameter line: Text content to index
    public func indexLine(_ line: String) {
        guard let search = search else { return }
        line.withCString { cStr in
            dterm_search_index_line(search, cStr, UInt(strlen(cStr)))
        }
    }

    /// Check if query might have matches (fast bloom filter check).
    ///
    /// Returns false if definitely no matches exist.
    /// Returns true if matches are possible (verify with actual search).
    ///
    /// - Parameter query: Text to search for
    /// - Returns: false if definitely not present, true if possibly present
    public func mightContain(_ query: String) -> Bool {
        // Guard against empty query to prevent Rust FFI panic
        guard let search = search, !query.isEmpty else { return false }
        return query.withCString { cStr in
            dterm_search_might_contain(search, cStr, UInt(strlen(cStr)))
        }
    }

    /// Search for a query string.
    ///
    /// - Parameters:
    ///   - query: Text to search for
    ///   - maxMatches: Maximum number of matches to return
    /// - Returns: Array of search matches
    public func find(_ query: String, maxMatches: Int = 100) -> [DTermSearchMatch] {
        // Guard against empty query to prevent Rust FFI panic
        guard let search = search, maxMatches > 0, !query.isEmpty else { return [] }
        return query.withCString { cStr in
            var matches = [dterm_search_match_t](repeating: dterm_search_match_t(), count: maxMatches)
            let count = dterm_search_find(search, cStr, UInt(strlen(cStr)), &matches, UInt(maxMatches))
            // Clamp count to maxMatches to prevent out-of-bounds access
            let safeCount = min(Int(count), maxMatches)
            return (0..<safeCount).map { DTermSearchMatch(matches[$0]) }
        }
    }

    /// Search for next match after a position.
    ///
    /// - Parameters:
    ///   - query: Text to search for
    ///   - afterLine: Line to start searching after
    ///   - afterCol: Column to start searching after
    /// - Returns: Next match, or nil if none found
    public func findNext(_ query: String, afterLine: Int, afterCol: Int) -> DTermSearchMatch? {
        // Guard against empty query to prevent Rust FFI panic
        guard let search = search, !query.isEmpty else { return nil }
        return query.withCString { cStr in
            var match = dterm_search_match_t()
            if dterm_search_find_next(search, cStr, UInt(strlen(cStr)), UInt(afterLine), UInt(afterCol), &match) {
                return DTermSearchMatch(match)
            }
            return nil
        }
    }

    /// Search for previous match before a position.
    ///
    /// - Parameters:
    ///   - query: Text to search for
    ///   - beforeLine: Line to start searching before
    ///   - beforeCol: Column to start searching before
    /// - Returns: Previous match, or nil if none found
    public func findPrev(_ query: String, beforeLine: Int, beforeCol: Int) -> DTermSearchMatch? {
        // Guard against empty query to prevent Rust FFI panic
        guard let search = search, !query.isEmpty else { return nil }
        return query.withCString { cStr in
            var match = dterm_search_match_t()
            if dterm_search_find_prev(search, cStr, UInt(strlen(cStr)), UInt(beforeLine), UInt(beforeCol), &match) {
                return DTermSearchMatch(match)
            }
            return nil
        }
    }

    /// Get the number of indexed lines.
    public var lineCount: Int {
        guard let search = search else { return 0 }
        return Int(dterm_search_line_count(search))
    }

    /// Clear the search index.
    public func clear() {
        guard let search = search else { return }
        dterm_search_clear(search)
    }
}

/// Search match result.
public struct DTermSearchMatch {
    /// Line number (0-indexed).
    public let line: Int
    /// Starting column of the match (0-indexed).
    public let startCol: Int
    /// Ending column of the match (exclusive).
    public let endCol: Int

    init(_ match: dterm_search_match_t) {
        self.line = Int(match.line)
        self.startCol = Int(match.start_col)
        self.endCol = Int(match.end_col)
    }
}

// MARK: - Shell Integration (OSC 133)

/// Shell integration state.
///
/// Tracks the current state of shell integration based on OSC 133 sequences.
public enum ShellState: Int, Equatable {
    /// Ground state - waiting for prompt.
    case ground = 0
    /// Receiving prompt text (after OSC 133 ; A).
    case receivingPrompt = 1
    /// User is entering command (after OSC 133 ; B).
    case enteringCommand = 2
    /// Command is executing (after OSC 133 ; C).
    case executing = 3

    init(ffi: DtermShellState) {
        switch ffi {
        case GROUND: self = .ground
        case RECEIVING_PROMPT: self = .receivingPrompt
        case ENTERING_COMMAND: self = .enteringCommand
        case EXECUTING: self = .executing
        default: self = .ground
        }
    }
}

/// Output block state for block-based terminal model.
///
/// Note: C enum variants are prefixed with `Block` to avoid collision with `DtermShellState`.
public enum BlockState: Int, Equatable {
    /// Only prompt has been received.
    case promptOnly = 0
    /// User is entering a command.
    case enteringCommand = 1
    /// Command is executing.
    case executing = 2
    /// Command has completed with exit code.
    case complete = 3

    init(ffi: DtermBlockState) {
        switch ffi {
        case BLOCK_PROMPT_ONLY: self = .promptOnly
        case BLOCK_ENTERING_COMMAND: self = .enteringCommand
        case BLOCK_EXECUTING: self = .executing
        case BLOCK_COMPLETE: self = .complete
        default: self = .promptOnly
        }
    }
}

/// An output block representing a command and its output.
///
/// Output blocks are the fundamental unit of the block-based terminal model.
/// Each block contains a prompt, optional command, and optional output.
public struct OutputBlock: Equatable {
    /// Unique identifier for this block.
    public let id: UInt64
    /// Current state of this block.
    public let state: BlockState
    /// Row where the prompt started (absolute line number).
    public let promptStartRow: Int
    /// Column where the prompt started.
    public let promptStartCol: UInt16
    /// Row where the command text started (nil if not set).
    public let commandStartRow: Int?
    /// Column where the command text started (nil if not set).
    public let commandStartCol: UInt16?
    /// Row where command output started (nil if not set).
    public let outputStartRow: Int?
    /// Row where this block ends (exclusive, nil if not set).
    public let endRow: Int?
    /// Command exit code (only valid if state is .complete).
    public let exitCode: Int32?

    init(_ block: DtermOutputBlock) {
        self.id = block.id
        self.state = BlockState(ffi: block.state)
        self.promptStartRow = Int(block.prompt_start_row)
        self.promptStartCol = block.prompt_start_col
        self.commandStartRow = block.has_command_start ? Int(block.command_start_row) : nil
        self.commandStartCol = block.has_command_start ? block.command_start_col : nil
        self.outputStartRow = block.has_output_start ? Int(block.output_start_row) : nil
        self.endRow = block.has_end_row ? Int(block.end_row) : nil
        self.exitCode = block.has_exit_code ? block.exit_code : nil
    }
}

// MARK: - Line Size (DEC Double Width/Height)

/// Line size mode for DEC double-width/double-height lines.
///
/// Used for rendering lines with DECDWL/DECDHL escape sequences.
public enum LineSize: Int, Equatable {
    /// Normal single-width line (default).
    case singleWidth = 0
    /// Double-width line (DECDWL - ESC # 6).
    case doubleWidth = 1
    /// Top half of double-height line (DECDHL - ESC # 3).
    case doubleHeightTop = 2
    /// Bottom half of double-height line (DECDHL - ESC # 4).
    case doubleHeightBottom = 3

    init(ffi: DtermLineSize) {
        switch ffi {
        case SINGLE_WIDTH: self = .singleWidth
        case DOUBLE_WIDTH: self = .doubleWidth
        case DOUBLE_HEIGHT_TOP: self = .doubleHeightTop
        case DOUBLE_HEIGHT_BOTTOM: self = .doubleHeightBottom
        default: self = .singleWidth
        }
    }

    /// Whether this line should be rendered at double width.
    public var isDoubleWidth: Bool {
        switch self {
        case .doubleWidth, .doubleHeightTop, .doubleHeightBottom:
            return true
        case .singleWidth:
            return false
        }
    }
}

// MARK: - DTermCore Shell Integration Extension

extension DTermCore {
    /// Get the current shell integration state.
    ///
    /// Shell integration uses OSC 133 sequences to track shell state:
    /// - `.ground`: Waiting for prompt
    /// - `.receivingPrompt`: Receiving prompt text (after OSC 133 ; A)
    /// - `.enteringCommand`: User is typing command (after OSC 133 ; B)
    /// - `.executing`: Command is running (after OSC 133 ; C)
    public var shellState: ShellState {
        guard let terminal = terminal else { return .ground }
        return ShellState(ffi: dterm_terminal_shell_state(terminal))
    }

    /// Get the number of completed output blocks.
    public var blockCount: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_block_count(terminal))
    }

    /// Get an output block by index.
    ///
    /// - Parameter index: Block index (0 = oldest block)
    /// - Returns: Output block, or nil if index is out of bounds
    public func block(at index: Int) -> OutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_block(terminal, UInt(index), &block) {
            return OutputBlock(block)
        }
        return nil
    }

    /// Get the current (in-progress) output block.
    ///
    /// Returns the block currently being built, if any.
    public var currentBlock: OutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_current_block(terminal, &block) {
            return OutputBlock(block)
        }
        return nil
    }

    /// Find the output block containing a given row.
    ///
    /// - Parameter row: Absolute row number
    /// - Returns: Block index, or nil if no block contains the row
    public func blockIndex(containingRow row: Int) -> Int? {
        guard let terminal = terminal else { return nil }
        let index = dterm_terminal_block_at_row(terminal, UInt(row))
        // usize::MAX indicates not found
        if index == UInt.max {
            return nil
        }
        return Int(index)
    }

    /// Get the line size for a specific row.
    ///
    /// Returns the DEC line size mode (single-width, double-width, or double-height).
    /// Double-width/height lines display characters at 2x width.
    ///
    /// - Parameter row: Row index (0-indexed)
    /// - Returns: Line size mode
    public func lineSize(forRow row: UInt16) -> LineSize {
        guard let terminal = terminal else { return .singleWidth }
        return LineSize(ffi: dterm_terminal_row_line_size(terminal, row))
    }

    // MARK: - Memory Management (Directive 4 FFI)

    /// Get the total memory usage of the terminal in bytes.
    ///
    /// This includes the screen buffer, scrollback, and any overflow tables
    /// for complex characters and true colors.
    ///
    /// - Returns: Memory usage in bytes, or 0 if terminal is invalid
    public var memoryUsage: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_memory_usage(terminal))
    }

    /// Set a memory budget for the terminal.
    ///
    /// When a memory budget is set, the terminal will automatically:
    /// - Trim scrollback history when it exceeds the budget
    /// - Evict least-recently-used overflow entries
    /// - Compress scrollback if enabled
    ///
    /// - Parameter budget: The memory budget configuration
    /// - Returns: true if the budget was set successfully
    @discardableResult
    public func setMemoryBudget(_ budget: MemoryBudget) -> Bool {
        guard let terminal = terminal else { return false }
        var cBudget = dterm_memory_budget_t(
            max_bytes: budget.maxBytes,
            max_scrollback_lines: budget.maxScrollbackLines,
            compress_scrollback: budget.compressScrollback,
            _reserved: (0, 0, 0, 0, 0, 0, 0)
        )
        return dterm_terminal_set_memory_budget(terminal, &cBudget)
    }

    // MARK: - Cell Access (Directive 4 FFI)

    /// Get the Unicode codepoint for a cell at the given position.
    ///
    /// This function handles complex characters (emoji, non-BMP) by looking up
    /// the overflow table when the COMPLEX flag is set in the 8-byte cell.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based, visible area only)
    ///   - col: Column index (0-based)
    /// - Returns: Unicode codepoint, or 0 for empty/invalid cells
    public func cellCodepoint(row: UInt16, col: UInt16) -> UInt32 {
        guard let terminal = terminal else { return 0 }
        return dterm_cell_codepoint(terminal, row, col)
    }

    /// Get the foreground color as RGB for a cell.
    ///
    /// Handles both indexed colors (using the 256-color palette) and true color.
    /// For true color cells in the 8-byte cell format, this looks up the overflow table.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based, visible area only)
    ///   - col: Column index (0-based)
    /// - Returns: RGB color tuple (r, g, b), or nil on error
    public func cellForegroundRGB(row: UInt16, col: UInt16) -> (r: UInt8, g: UInt8, b: UInt8)? {
        guard let terminal = terminal else { return nil }
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        if dterm_cell_fg_rgb(terminal, row, col, &r, &g, &b) {
            return (r, g, b)
        }
        return nil
    }

    /// Get the background color as RGB for a cell.
    ///
    /// Handles both indexed colors (using the 256-color palette) and true color.
    /// For true color cells in the 8-byte cell format, this looks up the overflow table.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based, visible area only)
    ///   - col: Column index (0-based)
    /// - Returns: RGB color tuple (r, g, b), or nil on error
    public func cellBackgroundRGB(row: UInt16, col: UInt16) -> (r: UInt8, g: UInt8, b: UInt8)? {
        guard let terminal = terminal else { return nil }
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        if dterm_cell_bg_rgb(terminal, row, col, &r, &g, &b) {
            return (r, g, b)
        }
        return nil
    }
}

// MARK: - MemoryBudget

/// Configuration for controlling terminal memory usage.
///
/// When a memory budget is set, the terminal will automatically:
/// - Trim scrollback history when it exceeds the budget
/// - Evict least-recently-used overflow entries
/// - Compress scrollback if supported
public struct MemoryBudget {
    /// Maximum total memory in bytes (0 = unlimited).
    public var maxBytes: Int

    /// Maximum scrollback lines (0 = use default from config).
    public var maxScrollbackLines: Int

    /// Whether to automatically compress old scrollback.
    public var compressScrollback: Bool

    /// Default configuration (100MB budget, 100K lines).
    public static var `default`: MemoryBudget {
        MemoryBudget(
            maxBytes: 100 * 1024 * 1024,
            maxScrollbackLines: 100_000,
            compressScrollback: false
        )
    }

    /// Unlimited memory (no restrictions).
    public static var unlimited: MemoryBudget {
        MemoryBudget(
            maxBytes: 0,
            maxScrollbackLines: 0,
            compressScrollback: false
        )
    }

    /// Low memory configuration (16MB budget, 10K lines, compression enabled).
    ///
    /// Suitable for resource-constrained environments or when running many terminals.
    public static var lowMemory: MemoryBudget {
        MemoryBudget(
            maxBytes: 16 * 1024 * 1024,
            maxScrollbackLines: 10_000,
            compressScrollback: true
        )
    }

    public init(maxBytes: Int, maxScrollbackLines: Int, compressScrollback: Bool) {
        self.maxBytes = maxBytes
        self.maxScrollbackLines = maxScrollbackLines
        self.compressScrollback = compressScrollback
    }
}

// MARK: - GPU Renderer

/// Status of a frame request.
///
/// This enum maps to `DtermFrameStatus` from dterm-core FFI.
public enum DTermFrameStatus: UInt32 {
    /// Frame is ready - drawable was provided
    case ready = 0
    /// Timeout expired before drawable was provided
    case timeout = 1
    /// Request was cancelled (sender dropped)
    case cancelled = 2
}

/// Frame handle for tracking frame requests.
///
/// This is a value type that wraps the FFI `DtermFrameHandle`.
public struct DTermFrameHandle {
    /// The underlying frame ID
    public let id: UInt64

    /// Create a frame handle from an FFI handle.
    init(ffiHandle: DtermFrameHandle) {
        self.id = ffiHandle.id
    }

    /// Convert to FFI handle.
    var ffiHandle: DtermFrameHandle {
        return DtermFrameHandle(id: id)
    }
}

/// GPU renderer configuration.
public struct DTermRendererConfig {
    /// Background color
    public var backgroundColor: (r: UInt8, g: UInt8, b: UInt8)

    /// Whether to enable vsync
    public var vsync: Bool

    /// Target FPS when vsync is disabled
    public var targetFPS: UInt32

    /// Maximum time to wait for a drawable (milliseconds)
    public var drawableTimeoutMs: UInt64

    /// Whether to enable damage-based rendering
    public var damageRendering: Bool

    /// Default configuration (black background, vsync enabled).
    public static var `default`: DTermRendererConfig {
        DTermRendererConfig(
            backgroundColor: (0, 0, 0),
            vsync: true,
            targetFPS: 60,
            drawableTimeoutMs: 17,
            damageRendering: true
        )
    }

    /// Convert to FFI config.
    func toFFI() -> DtermRendererConfig {
        return DtermRendererConfig(
            background_r: backgroundColor.r,
            background_g: backgroundColor.g,
            background_b: backgroundColor.b,
            vsync: vsync,
            target_fps: targetFPS,
            drawable_timeout_ms: drawableTimeoutMs,
            damage_rendering: damageRendering
        )
    }
}

/// GPU renderer for terminal content.
///
/// This class wraps the dterm-core GPU renderer, providing safe frame
/// synchronization using Rust channels. Unlike the ObjC renderer stack,
/// this implementation CANNOT crash with "unbalanced dispatch_group" errors.
///
/// ## Usage
///
/// ```swift
/// let renderer = DTermRenderer()
///
/// // Request a frame
/// let handle = renderer.requestFrame()
///
/// // Platform provides drawable (from CAMetalLayer.nextDrawable())
/// renderer.completeFrame(handle)
///
/// // Wait for frame to be ready
/// let status = renderer.waitForFrame(timeoutMs: 16)
/// if status == .ready {
///     // Render...
/// }
/// ```
///
/// ## Thread Safety
///
/// - Frame requests and completions can be called from any thread
/// - `waitForFrame` blocks the calling thread
/// - The renderer is internally synchronized with Rust mutexes
public final class DTermRenderer {
    /// Opaque pointer to the Rust renderer
    private var handle: OpaquePointer?

    /// Create a new GPU renderer with default configuration.
    public init?() {
        guard dterm_renderer_available() else {
            return nil
        }
        handle = dterm_renderer_create()
        if handle == nil {
            return nil
        }
    }

    /// Create a new GPU renderer with custom configuration.
    public init?(config: DTermRendererConfig) {
        guard dterm_renderer_available() else {
            return nil
        }
        var ffiConfig = config.toFFI()
        handle = dterm_renderer_create_with_config(&ffiConfig)
        if handle == nil {
            return nil
        }
    }

    deinit {
        if let handle = handle {
            dterm_renderer_free(handle)
        }
    }

    /// Check if the GPU renderer FFI is available.
    public static var isAvailable: Bool {
        return dterm_renderer_available()
    }

    /// Request a new frame.
    ///
    /// The returned handle must be completed with `completeFrame()` or the
    /// frame will timeout/cancel automatically.
    ///
    /// - Returns: A frame handle, or nil if the renderer is invalid.
    public func requestFrame() -> DTermFrameHandle? {
        guard let handle = handle else { return nil }
        let ffiHandle = dterm_renderer_request_frame(handle)
        if ffiHandle.id == UInt64.max {
            return nil
        }
        return DTermFrameHandle(ffiHandle: ffiHandle)
    }

    /// Complete a frame request, signaling that the drawable is ready.
    ///
    /// This should be called after the platform provides a drawable
    /// (e.g., CAMetalDrawable from nextDrawable()).
    ///
    /// - Parameter frameHandle: The frame handle to complete.
    public func completeFrame(_ frameHandle: DTermFrameHandle) {
        guard let handle = handle else { return }
        dterm_renderer_complete_frame(handle, frameHandle.ffiHandle)
    }

    /// Cancel a frame request.
    ///
    /// This can be called if the platform cannot provide a drawable.
    ///
    /// - Parameter frameHandle: The frame handle to cancel.
    public func cancelFrame(_ frameHandle: DTermFrameHandle) {
        guard let handle = handle else { return }
        dterm_renderer_cancel_frame(handle, frameHandle.ffiHandle)
    }

    /// Wait for a frame to be ready.
    ///
    /// Blocks until the frame is ready, cancelled, or timeout expires.
    ///
    /// **Safe**: This cannot crash with "unbalanced" errors like dispatch_group.
    /// Timeout just returns `.timeout` and cleans up automatically.
    ///
    /// - Parameter timeoutMs: Timeout in milliseconds.
    /// - Returns: Frame status (ready, timeout, or cancelled).
    public func waitForFrame(timeoutMs: UInt64) -> DTermFrameStatus {
        guard let handle = handle else { return .cancelled }
        let status = dterm_renderer_wait_frame(handle, timeoutMs)
        return DTermFrameStatus(rawValue: status.rawValue) ?? .cancelled
    }
}
