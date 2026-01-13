/*
 * DTermTerminal.swift - Main terminal class for DTermCore
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import Foundation
import CDTermCore

// MARK: - Delegate Protocol

/// Protocol for receiving terminal events.
///
/// This protocol mirrors SwiftTerm's `TerminalDelegate` for compatibility.
/// When integrating with SwiftTerm, implement a bridge that forwards these
/// callbacks to the SwiftTerm delegate.
public protocol DTermTerminalDelegate: AnyObject {
    // MARK: - Title/Window

    /// Called when the terminal title changes (OSC 0, OSC 2).
    /// Maps to SwiftTerm's `setTerminalTitle`.
    func terminalTitleDidChange(_ terminal: DTermTerminal, title: String)

    /// Called when the terminal icon name changes (OSC 1).
    /// Maps to SwiftTerm's `setTerminalIconTitle`.
    func terminalIconNameDidChange(_ terminal: DTermTerminal, iconName: String)

    /// Called when a window manipulation command is received (CSI t).
    /// Maps to SwiftTerm's `windowCommand`.
    ///
    /// - Parameters:
    ///   - terminal: The terminal.
    ///   - command: The window command type.
    /// - Returns: Response bytes to send back, or nil.
    func terminalWindowCommand(_ terminal: DTermTerminal, command: WindowCommand) -> Data?

    // MARK: - I/O

    /// Called when the terminal needs to send response data (e.g., DSR, DA).
    /// Maps to SwiftTerm's `send`.
    func terminalHasResponse(_ terminal: DTermTerminal, data: Data)

    // MARK: - Cursor

    /// Called when cursor visibility changes.
    /// Maps to SwiftTerm's `showCursor`/`hideCursor`.
    func terminalCursorVisibilityDidChange(_ terminal: DTermTerminal, visible: Bool)

    /// Called when cursor style changes.
    /// Maps to SwiftTerm's `cursorStyleChanged`.
    func terminalCursorStyleDidChange(_ terminal: DTermTerminal, style: CursorStyle)

    // MARK: - Display

    /// Called when terminal modes change.
    func terminalModesDidChange(_ terminal: DTermTerminal)

    /// Called when the terminal size changes (after resize).
    /// Maps to SwiftTerm's `sizeChanged`.
    func terminalSizeDidChange(_ terminal: DTermTerminal)

    /// Called when the display scroll position changes.
    /// Maps to SwiftTerm's `scrolled`.
    ///
    /// - Parameters:
    ///   - terminal: The terminal.
    ///   - displayOffset: Current display offset (scroll position).
    func terminalScrolled(_ terminal: DTermTerminal, displayOffset: Int)

    /// Called on every linefeed.
    /// Maps to SwiftTerm's `linefeed`.
    func terminalLinefeed(_ terminal: DTermTerminal)

    /// Called when the terminal bell is triggered.
    /// Maps to SwiftTerm's `bell`.
    func terminalBell(_ terminal: DTermTerminal)

    // MARK: - Buffer/Screen

    /// Called when the terminal switches between main and alternate screen buffers.
    ///
    /// This is triggered when applications like vim, less, or tmux enter or exit
    /// the alternate screen buffer. Maps to SwiftTerm's `bufferActivated`.
    ///
    /// - Parameters:
    ///   - terminal: The terminal that changed buffers.
    ///   - isAlternate: `true` if switched to alternate screen, `false` if switched to main.
    func terminalBufferActivated(_ terminal: DTermTerminal, isAlternate: Bool)

    // MARK: - Selection

    /// Called when text selection changes.
    /// Maps to SwiftTerm's `selectionChanged`.
    func terminalSelectionDidChange(_ terminal: DTermTerminal)

    // MARK: - Mouse

    /// Called when mouse mode changes.
    /// Maps to SwiftTerm's `mouseModeChanged`.
    func terminalMouseModeDidChange(_ terminal: DTermTerminal)

    // MARK: - Shell Integration

    /// Called when current working directory changes (OSC 7).
    /// Maps to SwiftTerm's `hostCurrentDirectoryUpdated`.
    func terminalCurrentDirectoryDidChange(_ terminal: DTermTerminal, path: String?)

    /// Called when current document changes (OSC 6).
    /// Maps to SwiftTerm's `hostCurrentDocumentUpdated`.
    func terminalCurrentDocumentDidChange(_ terminal: DTermTerminal, url: String?)

    // MARK: - Colors

    /// Called when a palette color changes (OSC 4, OSC 104).
    /// Maps to SwiftTerm's `colorChanged`.
    ///
    /// - Parameters:
    ///   - terminal: The terminal that updated its palette.
    ///   - index: Palette index that changed (0-255), or nil for reset.
    func terminalColorDidChange(_ terminal: DTermTerminal, index: Int?)

    /// Called when foreground color changes (OSC 10).
    /// Maps to SwiftTerm's `setForegroundColor`.
    func terminalForegroundColorDidChange(_ terminal: DTermTerminal, color: DTermRGB)

    /// Called when background color changes (OSC 11).
    /// Maps to SwiftTerm's `setBackgroundColor`.
    func terminalBackgroundColorDidChange(_ terminal: DTermTerminal, color: DTermRGB)

    /// Called when cursor color changes (OSC 12).
    /// Maps to SwiftTerm's `setCursorColor`.
    func terminalCursorColorDidChange(_ terminal: DTermTerminal, color: DTermRGB?)

    /// Query current foreground and background colors.
    /// Maps to SwiftTerm's `getColors`.
    ///
    /// - Returns: Tuple of (foreground, background) colors.
    func terminalGetColors(_ terminal: DTermTerminal) -> (foreground: DTermRGB, background: DTermRGB)

    // MARK: - Clipboard

    /// Called when clipboard content should be set (OSC 52).
    /// Maps to SwiftTerm's `clipboardCopy`.
    func terminalSetClipboard(_ terminal: DTermTerminal, content: String)

    // MARK: - Notifications

    /// Called when a notification should be shown (OSC 9, OSC 777).
    /// Maps to SwiftTerm's `notify`.
    func terminalNotify(_ terminal: DTermTerminal, title: String, body: String)

    // MARK: - Images

    /// Called when a Sixel image is received.
    /// Maps to SwiftTerm's `createImageFromBitmap`.
    func terminalDidReceiveSixelImage(_ terminal: DTermTerminal, width: Int, height: Int, pixels: [UInt32])

    /// Called when a Kitty graphics image is received.
    /// Maps to SwiftTerm's `createImage`.
    ///
    /// - Parameters:
    ///   - terminal: The terminal that received the image.
    ///   - id: Image ID assigned by the terminal.
    ///   - width: Image width in pixels.
    ///   - height: Image height in pixels.
    ///   - data: RGBA pixel data (4 bytes per pixel).
    func terminalDidReceiveKittyImage(_ terminal: DTermTerminal, id: UInt32, width: UInt32, height: UInt32, data: Data)

    /// Called when an iTerm2 inline image is received (OSC 1337).
    /// Maps to SwiftTerm's `iTermContent`.
    func terminalDidReceiveITermImage(_ terminal: DTermTerminal, content: Data)

    // MARK: - Security

    /// Query whether the process is trusted for secure input.
    /// Maps to SwiftTerm's `isProcessTrusted`.
    func terminalIsProcessTrusted(_ terminal: DTermTerminal) -> Bool
}

/// Optional delegate methods with default implementations.
public extension DTermTerminalDelegate {
    // Title/Window
    func terminalTitleDidChange(_ terminal: DTermTerminal, title: String) {}
    func terminalIconNameDidChange(_ terminal: DTermTerminal, iconName: String) {}
    func terminalWindowCommand(_ terminal: DTermTerminal, command: WindowCommand) -> Data? { nil }

    // I/O
    func terminalHasResponse(_ terminal: DTermTerminal, data: Data) {}

    // Cursor
    func terminalCursorVisibilityDidChange(_ terminal: DTermTerminal, visible: Bool) {}
    func terminalCursorStyleDidChange(_ terminal: DTermTerminal, style: CursorStyle) {}

    // Display
    func terminalModesDidChange(_ terminal: DTermTerminal) {}
    func terminalSizeDidChange(_ terminal: DTermTerminal) {}
    func terminalScrolled(_ terminal: DTermTerminal, displayOffset: Int) {}
    func terminalLinefeed(_ terminal: DTermTerminal) {}
    func terminalBell(_ terminal: DTermTerminal) {}

    // Buffer/Screen
    func terminalBufferActivated(_ terminal: DTermTerminal, isAlternate: Bool) {}

    // Selection
    func terminalSelectionDidChange(_ terminal: DTermTerminal) {}

    // Mouse
    func terminalMouseModeDidChange(_ terminal: DTermTerminal) {}

    // Shell Integration
    func terminalCurrentDirectoryDidChange(_ terminal: DTermTerminal, path: String?) {}
    func terminalCurrentDocumentDidChange(_ terminal: DTermTerminal, url: String?) {}

    // Colors
    func terminalColorDidChange(_ terminal: DTermTerminal, index: Int?) {}
    func terminalForegroundColorDidChange(_ terminal: DTermTerminal, color: DTermRGB) {}
    func terminalBackgroundColorDidChange(_ terminal: DTermTerminal, color: DTermRGB) {}
    func terminalCursorColorDidChange(_ terminal: DTermTerminal, color: DTermRGB?) {}
    func terminalGetColors(_ terminal: DTermTerminal) -> (foreground: DTermRGB, background: DTermRGB) {
        // Default xterm colors
        return (
            foreground: DTermRGB(red: 229, green: 229, blue: 229),
            background: DTermRGB(red: 0, green: 0, blue: 0)
        )
    }

    // Clipboard
    func terminalSetClipboard(_ terminal: DTermTerminal, content: String) {}

    // Notifications
    func terminalNotify(_ terminal: DTermTerminal, title: String, body: String) {}

    // Images
    func terminalDidReceiveSixelImage(_ terminal: DTermTerminal, width: Int, height: Int, pixels: [UInt32]) {}
    func terminalDidReceiveKittyImage(_ terminal: DTermTerminal, id: UInt32, width: UInt32, height: UInt32, data: Data) {}
    func terminalDidReceiveITermImage(_ terminal: DTermTerminal, content: Data) {}

    // Security
    func terminalIsProcessTrusted(_ terminal: DTermTerminal) -> Bool { true }
}

/// A high-performance terminal emulator powered by dterm-core.
///
/// DTermTerminal provides a Swift-native interface to the dterm-core Rust library,
/// offering VT100/VT220/xterm terminal emulation with support for:
/// - Unicode and emoji
/// - 256 colors and 24-bit true color
/// - Mouse tracking
/// - Bracketed paste mode
/// - OSC 7 (current working directory)
/// - OSC 8 (hyperlinks)
/// - OSC 52 (clipboard)
/// - OSC 133 (shell integration)
/// - Sixel and Kitty graphics
///
/// ## Usage
///
/// ```swift
/// let terminal = DTermTerminal(rows: 24, cols: 80)
/// terminal.delegate = self
///
/// // Process input from PTY
/// terminal.process(data: inputData)
///
/// // Get cell data for rendering
/// for row in 0..<terminal.rows {
///     for col in 0..<terminal.cols {
///         if let cell = terminal.getCell(row: row, col: col) {
///             // Render cell
///         }
///     }
/// }
/// ```
public final class DTermTerminal {
    /// Opaque handle to the underlying dterm-core terminal.
    private var handle: OpaquePointer

    /// Terminal delegate for receiving events.
    public weak var delegate: DTermTerminalDelegate?

    /// Buffer for title string (to extend lifetime across FFI boundary).
    private var titleBuffer: String?

    /// Track which Kitty image IDs we've already notified about.
    private var notifiedKittyImageIds = Set<UInt32>()

    /// Snapshot of the 256-color palette for change detection.
    private var paletteSnapshot: [DTermRGB] = []

    // MARK: - Lifecycle

    /// Create a new terminal with the specified dimensions.
    ///
    /// - Parameters:
    ///   - rows: Number of visible rows.
    ///   - cols: Number of columns.
    public init(rows: Int, cols: Int) {
        self.handle = dterm_terminal_new(UInt16(rows), UInt16(cols))
        self.paletteSnapshot = capturePalette()
        setupCallbacks()
    }

    /// Create a new terminal with custom scrollback settings.
    ///
    /// - Parameters:
    ///   - rows: Number of visible rows.
    ///   - cols: Number of columns.
    ///   - ringBufferSize: Size of fast ring buffer.
    ///   - hotLimit: Max lines in hot tier.
    ///   - warmLimit: Max lines in warm tier.
    ///   - memoryBudget: Memory budget in bytes.
    public init(rows: Int, cols: Int, ringBufferSize: Int, hotLimit: Int, warmLimit: Int, memoryBudget: Int) {
        self.handle = dterm_terminal_new_with_scrollback(
            UInt16(rows),
            UInt16(cols),
            UInt(ringBufferSize),
            UInt(hotLimit),
            UInt(warmLimit),
            UInt(memoryBudget)
        )
        self.paletteSnapshot = capturePalette()
        setupCallbacks()
    }

    deinit {
        dterm_terminal_free(handle)
    }

    // MARK: - Callback Setup

    /// Set up FFI callbacks to forward events to the delegate.
    private func setupCallbacks() {
        let context = Unmanaged.passUnretained(self).toOpaque()

        // Bell callback
        dterm_terminal_set_bell_callback(handle, { ctx in
            guard let ctx = ctx else { return }
            let terminal = Unmanaged<DTermTerminal>.fromOpaque(ctx).takeUnretainedValue()
            terminal.delegate?.terminalBell(terminal)
        }, context)

        // Buffer activation callback (alternate screen)
        dterm_terminal_set_buffer_activation_callback(handle, { ctx, isAlternate in
            guard let ctx = ctx else { return }
            let terminal = Unmanaged<DTermTerminal>.fromOpaque(ctx).takeUnretainedValue()
            terminal.delegate?.terminalBufferActivated(terminal, isAlternate: isAlternate)
        }, context)

        // Title callback
        dterm_terminal_set_title_callback(handle, { ctx, titlePtr in
            guard let ctx = ctx, let titlePtr = titlePtr else { return }
            let terminal = Unmanaged<DTermTerminal>.fromOpaque(ctx).takeUnretainedValue()
            let title = String(cString: titlePtr)
            terminal.delegate?.terminalTitleDidChange(terminal, title: title)
        }, context)

        // Window command callback
        dterm_terminal_set_window_callback(handle, { ctx, opPtr, responsePtr in
            guard let ctx = ctx, let opPtr = opPtr, let responsePtr = responsePtr else {
                return false
            }
            let terminal = Unmanaged<DTermTerminal>.fromOpaque(ctx).takeUnretainedValue()
            let op = opPtr.pointee

            // Convert FFI WindowOp to Swift WindowCommand
            let command: WindowCommand? = {
                switch op.op_type {
                case DTERM_WINDOW_OP_TYPE_DE_ICONIFY: return .deIconify
                case DTERM_WINDOW_OP_TYPE_ICONIFY: return .iconify
                case DTERM_WINDOW_OP_TYPE_MOVE_WINDOW:
                    return .moveTo(x: Int(op.param1), y: Int(op.param2))
                case DTERM_WINDOW_OP_TYPE_RESIZE_WINDOW_PIXELS:
                    return .resizePixels(width: Int(op.param2), height: Int(op.param1))
                case DTERM_WINDOW_OP_TYPE_RAISE_WINDOW: return .raise
                case DTERM_WINDOW_OP_TYPE_LOWER_WINDOW: return .lower
                case DTERM_WINDOW_OP_TYPE_REFRESH_WINDOW: return .refresh
                case DTERM_WINDOW_OP_TYPE_RESIZE_WINDOW_CELLS:
                    return .resizeChars(cols: Int(op.param2), rows: Int(op.param1))
                case DTERM_WINDOW_OP_TYPE_REPORT_WINDOW_STATE: return .reportState
                case DTERM_WINDOW_OP_TYPE_REPORT_WINDOW_POSITION: return .reportPosition
                case DTERM_WINDOW_OP_TYPE_REPORT_WINDOW_SIZE_PIXELS: return .reportSizePixels
                case DTERM_WINDOW_OP_TYPE_REPORT_TEXT_AREA_CELLS: return .reportTextAreaChars
                case DTERM_WINDOW_OP_TYPE_REPORT_SCREEN_SIZE_CELLS: return .reportScreenSizeChars
                case DTERM_WINDOW_OP_TYPE_REPORT_ICON_LABEL: return .reportIconLabel
                case DTERM_WINDOW_OP_TYPE_REPORT_WINDOW_TITLE: return .reportTitle
                case DTERM_WINDOW_OP_TYPE_PUSH_TITLE:
                    let mode = op.param1
                    return .pushTitle(icon: mode & 1 != 0, window: mode & 2 != 0)
                case DTERM_WINDOW_OP_TYPE_POP_TITLE:
                    let mode = op.param1
                    return .popTitle(icon: mode & 1 != 0, window: mode & 2 != 0)
                case DTERM_WINDOW_OP_TYPE_MAXIMIZE_WINDOW:
                    return .maximize(horizontal: true, vertical: true)
                case DTERM_WINDOW_OP_TYPE_ENTER_FULLSCREEN: return .fullscreen(on: true)
                case DTERM_WINDOW_OP_TYPE_EXIT_FULLSCREEN: return .fullscreen(on: false)
                case DTERM_WINDOW_OP_TYPE_TOGGLE_FULLSCREEN:
                    // Toggle not directly supported, treat as fullscreen on
                    return .fullscreen(on: true)
                default: return nil
                }
            }()

            guard let cmd = command else { return false }

            // Call delegate and handle response
            if terminal.delegate?.terminalWindowCommand(terminal, command: cmd) != nil {
                // If delegate returns data, we handled it
                responsePtr.pointee.has_response = true
                // Response data would be parsed here if needed
                return true
            }
            return false
        }, context)

        // Shell event callback for OSC 133 shell integration
        // Note: Directory changes (OSC 7) are polled via currentWorkingDirectory property
        dterm_terminal_set_shell_callback(handle, { ctx, eventPtr in
            guard let ctx = ctx, let eventPtr = eventPtr else { return }
            let _ = Unmanaged<DTermTerminal>.fromOpaque(ctx).takeUnretainedValue()
            let event = eventPtr.pointee

            // Shell integration events (OSC 133) can be used to track command execution
            // Prompt start, command input, output start, command finished
            // Future: Add delegate methods for shell integration if needed
            switch event.event_type {
            case DTERM_SHELL_EVENT_TYPE_PROMPT_START:
                // Prompt line started (OSC 133 ; A)
                break
            case DTERM_SHELL_EVENT_TYPE_COMMAND_START:
                // User started typing command (OSC 133 ; B)
                break
            case DTERM_SHELL_EVENT_TYPE_OUTPUT_START:
                // Command execution started (OSC 133 ; C)
                break
            case DTERM_SHELL_EVENT_TYPE_COMMAND_FINISHED:
                // Command completed with exit code (OSC 133 ; D)
                // event.exit_code contains the exit status
                break
            default:
                break
            }
        }, context)
    }

    // MARK: - Dimensions

    /// Number of visible rows.
    public var rows: Int {
        Int(dterm_terminal_rows(handle))
    }

    /// Number of columns.
    public var cols: Int {
        Int(dterm_terminal_cols(handle))
    }

    /// Current memory usage in bytes.
    public var memoryUsage: Int {
        Int(dterm_terminal_memory_usage(handle))
    }

    /// Set the scrollback memory budget in bytes.
    ///
    /// The terminal will compress scrollback and evict overflow entries
    /// to stay under this budget.
    ///
    /// - Parameter bytes: Memory budget in bytes.
    public func setMemoryBudget(_ bytes: Int) {
        dterm_terminal_set_memory_budget(handle, UInt(bytes))
    }

    /// Resize the terminal.
    ///
    /// - Parameters:
    ///   - rows: New number of rows.
    ///   - cols: New number of columns.
    public func resize(rows: Int, cols: Int) {
        dterm_terminal_resize(handle, UInt16(rows), UInt16(cols))
    }

    // MARK: - Cursor

    /// Cursor row position (0-indexed).
    public var cursorRow: Int {
        Int(dterm_terminal_cursor_row(handle))
    }

    /// Cursor column position (0-indexed).
    public var cursorCol: Int {
        Int(dterm_terminal_cursor_col(handle))
    }

    /// Whether the cursor is visible.
    public var cursorVisible: Bool {
        dterm_terminal_cursor_visible(handle)
    }

    // MARK: - Input Processing

    /// Process input bytes from the PTY.
    ///
    /// This is the main method for feeding data to the terminal. Call this
    /// whenever data is received from the shell process.
    ///
    /// - Parameter data: Raw bytes from the PTY.
    public func process(data: Data) {
        data.withUnsafeBytes { buffer in
            guard let ptr = buffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return
            }
            dterm_terminal_process(handle, ptr, UInt(buffer.count))
        }

        // Check for response data
        checkResponse()

        // Check for Sixel images
        checkSixelImage()

        // Check for Kitty images
        checkKittyImages()

        // Check for palette changes
        checkPaletteChanges()
    }

    /// Process input bytes from a byte array.
    ///
    /// - Parameter bytes: Bytes to process.
    public func process(bytes: [UInt8]) {
        bytes.withUnsafeBufferPointer { buffer in
            guard let ptr = buffer.baseAddress else { return }
            dterm_terminal_process(handle, ptr, UInt(buffer.count))
        }

        checkResponse()
        checkSixelImage()
        checkKittyImages()
        checkPaletteChanges()
    }

    /// Reset the terminal to initial state.
    public func reset() {
        dterm_terminal_reset(handle)
    }

    // MARK: - Cells

    /// Get cell at the specified position.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: The cell at that position, or nil if out of bounds.
    public func getCell(row: Int, col: Int) -> DTermCell? {
        var cell = dterm_cell_t()
        guard dterm_terminal_get_cell(handle, UInt16(row), UInt16(col), &cell) else {
            return nil
        }
        return DTermCell(from: cell)
    }

    /// Get the Unicode codepoint for a cell.
    ///
    /// For complex characters (non-BMP, grapheme clusters), this returns the
    /// first codepoint. Use `cellDisplayString()` for the full character.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: The Unicode codepoint (0 for empty cell).
    public func cellCodepoint(row: Int, col: Int) -> UInt32 {
        dterm_cell_codepoint(handle, UInt16(row), UInt16(col))
    }

    /// Get foreground color as RGB for a cell.
    ///
    /// Resolves indexed colors via the palette and true color via overflow tables.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: The RGB color.
    public func cellFgRgb(row: Int, col: Int) -> DTermRGB {
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        dterm_cell_fg_rgb(handle, UInt16(row), UInt16(col), &r, &g, &b)
        return DTermRGB(red: r, green: g, blue: b)
    }

    /// Get background color as RGB for a cell.
    ///
    /// Resolves indexed colors via the palette and true color via overflow tables.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: The RGB color.
    public func cellBgRgb(row: Int, col: Int) -> DTermRGB {
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        dterm_cell_bg_rgb(handle, UInt16(row), UInt16(col), &r, &g, &b)
        return DTermRGB(red: r, green: g, blue: b)
    }

    /// Get the hyperlink URL for a cell, if any.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: The hyperlink URL, or nil if none.
    public func cellHyperlink(row: Int, col: Int) -> String? {
        // Note: We need to use dterm_terminal_cell_hyperlink but it takes a mutable pointer
        // This is a design issue in the FFI that should be fixed
        guard let ptr = dterm_terminal_cell_hyperlink(handle, UInt16(row), UInt16(col)) else {
            return nil
        }
        return String(cString: ptr)
    }

    /// Check if a cell has a hyperlink.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed).
    ///   - col: Column index (0-indexed).
    /// - Returns: True if the cell has a hyperlink.
    public func cellHasHyperlink(row: Int, col: Int) -> Bool {
        dterm_terminal_cell_has_hyperlink(handle, UInt16(row), UInt16(col))
    }

    /// Get line size attribute for a row.
    ///
    /// - Parameter row: Row index (0-indexed).
    /// - Returns: The line size (single width, double width, etc.).
    public func lineSize(row: Int) -> LineSize {
        LineSize(from: dterm_terminal_row_line_size(handle, UInt16(row)))
    }

    // MARK: - Text Content

    /// Get the text content of a visible row.
    ///
    /// - Parameter row: Row index (0-indexed).
    /// - Returns: The text content of the row.
    public func getLineText(row: Int) -> String {
        // First get required size
        let requiredSize = dterm_terminal_get_visible_line_text(handle, UInt16(row), nil, 0)
        guard requiredSize > 0 else { return "" }

        var buffer = [UInt8](repeating: 0, count: Int(requiredSize))
        let written = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_get_visible_line_text(handle, UInt16(row), ptr.baseAddress, UInt(requiredSize))
        }

        guard written > 0 else { return "" }
        return String(bytes: buffer.prefix(Int(written)), encoding: .utf8) ?? ""
    }

    // MARK: - Scrollback

    /// Total number of scrollback lines.
    public var scrollbackLines: Int {
        Int(dterm_terminal_scrollback_lines(handle))
    }

    /// Total number of lines (visible + scrollback).
    public var totalLines: Int {
        Int(dterm_terminal_total_lines(handle))
    }

    /// Current display offset (scroll position).
    public var displayOffset: Int {
        Int(dterm_terminal_display_offset(handle))
    }

    /// Scroll display by the specified number of lines.
    ///
    /// - Parameter delta: Lines to scroll (positive = up, negative = down).
    public func scroll(delta: Int) {
        dterm_terminal_scroll_display(handle, Int32(delta))
    }

    /// Scroll to the top of scrollback.
    public func scrollToTop() {
        dterm_terminal_scroll_to_top(handle)
    }

    /// Scroll to the bottom (live content).
    public func scrollToBottom() {
        dterm_terminal_scroll_to_bottom(handle)
    }

    // MARK: - Text Selection

    /// Selection type for mouse-based text selection.
    ///
    /// See the TLA+ spec at `tla/Selection.tla` for formal semantics.
    public enum SelectionType: UInt8 {
        /// Character-by-character selection (single click + drag).
        case simple = 0
        /// Rectangular block selection (Alt + click + drag).
        case block = 1
        /// Semantic selection - words, URLs, etc. (double-click).
        case semantic = 2
        /// Full line selection (triple-click).
        case lines = 3
    }

    /// Start a new text selection.
    ///
    /// Call this on mouse down to begin a new selection.
    /// This clears any existing selection.
    ///
    /// - Parameters:
    ///   - col: Starting column (0-indexed).
    ///   - row: Starting row (0 = top of visible area, negative = scrollback).
    ///   - type: Type of selection (simple, block, semantic, lines).
    public func startSelection(col: Int, row: Int, type: SelectionType = .simple) {
        dterm_terminal_selection_start(handle,
                                       UInt32(col),
                                       Int32(row),
                                       type.rawValue)
    }

    /// Update the selection endpoint.
    ///
    /// Call this on mouse drag to update the selection.
    /// Only works when selection is in progress.
    ///
    /// - Parameters:
    ///   - col: Current column (0-indexed).
    ///   - row: Current row (0 = top of visible area, negative = scrollback).
    public func updateSelection(col: Int, row: Int) {
        dterm_terminal_selection_update(handle, UInt32(col), Int32(row))
    }

    /// Complete the selection.
    ///
    /// Call this on mouse up to finish the selection.
    public func endSelection() {
        dterm_terminal_selection_end(handle)
    }

    /// Clear any active selection.
    public func clearSelection() {
        dterm_terminal_selection_clear(handle)
    }

    /// Whether there is an active selection.
    public var hasSelection: Bool {
        dterm_terminal_has_selection(handle)
    }

    /// Get the selected text as a string.
    ///
    /// Returns nil if there is no selection.
    /// For block selections, rows are separated by newlines.
    public var selectedText: String? {
        guard let ptr = dterm_terminal_selection_to_string(handle) else {
            return nil
        }
        defer { dterm_string_free(ptr) }
        return String(cString: ptr)
    }

    // MARK: - State

    /// Window title.
    public var title: String? {
        guard let ptr = dterm_terminal_title(handle) else {
            return nil
        }
        titleBuffer = String(cString: ptr)
        return titleBuffer
    }

    /// Window icon name (set by OSC 1).
    public var iconName: String? {
        guard let ptr = dterm_terminal_icon_name(handle) else {
            return nil
        }
        return String(cString: ptr)
    }

    /// Cursor style (DECSCUSR values 1-6).
    public var cursorStyle: CursorStyle {
        CursorStyle(rawValue: dterm_terminal_cursor_style(handle)) ?? .blinkingBlock
    }

    /// Whether alternate screen buffer is active.
    public var isAlternateScreen: Bool {
        dterm_terminal_is_alternate_screen(handle)
    }

    /// Get current terminal modes.
    public var modes: DTermModes {
        var modes = dterm_modes_t()
        dterm_terminal_get_modes(handle, &modes)
        return DTermModes(from: modes)
    }

    /// Current shell integration state.
    public var shellState: ShellState {
        ShellState(from: dterm_terminal_shell_state(handle))
    }

    /// Current working directory (from OSC 7).
    public var currentWorkingDirectory: String? {
        guard dterm_terminal_has_working_directory(handle) else {
            return nil
        }
        guard let ptr = dterm_terminal_current_working_directory(handle) else {
            return nil
        }
        return String(cString: ptr)
    }

    /// Current hyperlink being applied to new text.
    public var currentHyperlink: String? {
        guard let ptr = dterm_terminal_current_hyperlink(handle) else {
            return nil
        }
        return String(cString: ptr)
    }

    /// Secure keyboard entry mode.
    ///
    /// When enabled, the UI layer should activate platform-specific secure input
    /// mechanisms to prevent keylogging:
    ///
    /// - **macOS**: Call `EnableSecureEventInput()` / `DisableSecureEventInput()`
    /// - **iOS**: Not applicable (sandboxed by default)
    /// - **Windows**: Limited protection available (document to users)
    /// - **Linux/X11**: Not possible (X11 is inherently insecure)
    /// - **Linux/Wayland**: Secure by default (no action needed)
    ///
    /// This is a read/write property. Set it to enable secure mode when the user
    /// activates "Secure Keyboard Entry" from the menu, and query it to check
    /// the current state.
    public var isSecureKeyboardEntry: Bool {
        get { dterm_terminal_is_secure_keyboard_entry(handle) }
        set { dterm_terminal_set_secure_keyboard_entry(handle, newValue) }
    }

    // MARK: - Damage Tracking

    /// Whether the terminal needs a full redraw.
    public var needsRedraw: Bool {
        dterm_terminal_needs_redraw(handle)
    }

    /// Clear damage after rendering.
    public func clearDamage() {
        dterm_terminal_clear_damage(handle)
    }

    /// Check if a specific row is damaged.
    ///
    /// - Parameter row: Row index (0-indexed).
    /// - Returns: True if the row needs redrawing.
    public func rowIsDamaged(_ row: Int) -> Bool {
        dterm_terminal_row_is_damaged(handle, UInt16(row))
    }

    // MARK: - Mouse Encoding

    /// Whether mouse tracking is enabled.
    public var mouseTrackingEnabled: Bool {
        dterm_terminal_mouse_tracking_enabled(handle)
    }

    /// Encode a mouse button press event.
    ///
    /// - Parameters:
    ///   - button: Mouse button (0=left, 1=middle, 2=right).
    ///   - col: Column (0-indexed).
    ///   - row: Row (0-indexed).
    ///   - modifiers: Modifier keys (shift=4, meta=8, ctrl=16).
    /// - Returns: Escape sequence to send to PTY, or nil if mouse reporting disabled.
    public func encodeMousePress(button: Int, col: Int, row: Int, modifiers: Int = 0) -> Data? {
        var buffer = [UInt8](repeating: 0, count: 32)
        let len = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_encode_mouse_press(
                handle,
                UInt8(button),
                UInt16(col),
                UInt16(row),
                UInt8(modifiers),
                ptr.baseAddress,
                UInt(32)
            )
        }
        guard len > 0 else { return nil }
        return Data(buffer.prefix(Int(len)))
    }

    /// Encode a mouse button release event.
    ///
    /// - Parameters:
    ///   - button: Original mouse button.
    ///   - col: Column (0-indexed).
    ///   - row: Row (0-indexed).
    ///   - modifiers: Modifier keys.
    /// - Returns: Escape sequence to send to PTY, or nil if mouse reporting disabled.
    public func encodeMouseRelease(button: Int, col: Int, row: Int, modifiers: Int = 0) -> Data? {
        var buffer = [UInt8](repeating: 0, count: 32)
        let len = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_encode_mouse_release(
                handle,
                UInt8(button),
                UInt16(col),
                UInt16(row),
                UInt8(modifiers),
                ptr.baseAddress,
                UInt(32)
            )
        }
        guard len > 0 else { return nil }
        return Data(buffer.prefix(Int(len)))
    }

    /// Encode a mouse motion event.
    ///
    /// - Parameters:
    ///   - button: Button held during motion (3=none).
    ///   - col: Column (0-indexed).
    ///   - row: Row (0-indexed).
    ///   - modifiers: Modifier keys.
    /// - Returns: Escape sequence to send to PTY, or nil if motion tracking disabled.
    public func encodeMouseMotion(button: Int, col: Int, row: Int, modifiers: Int = 0) -> Data? {
        var buffer = [UInt8](repeating: 0, count: 32)
        let len = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_encode_mouse_motion(
                handle,
                UInt8(button),
                UInt16(col),
                UInt16(row),
                UInt8(modifiers),
                ptr.baseAddress,
                UInt(32)
            )
        }
        guard len > 0 else { return nil }
        return Data(buffer.prefix(Int(len)))
    }

    /// Encode a mouse wheel event.
    ///
    /// - Parameters:
    ///   - up: True for wheel up, false for wheel down.
    ///   - col: Column (0-indexed).
    ///   - row: Row (0-indexed).
    ///   - modifiers: Modifier keys.
    /// - Returns: Escape sequence to send to PTY, or nil if mouse reporting disabled.
    public func encodeMouseWheel(up: Bool, col: Int, row: Int, modifiers: Int = 0) -> Data? {
        var buffer = [UInt8](repeating: 0, count: 32)
        let len = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_encode_mouse_wheel(
                handle,
                up,
                UInt16(col),
                UInt16(row),
                UInt8(modifiers),
                ptr.baseAddress,
                UInt(32)
            )
        }
        guard len > 0 else { return nil }
        return Data(buffer.prefix(Int(len)))
    }

    /// Encode a focus event.
    ///
    /// - Parameter focused: True if window gained focus.
    /// - Returns: Escape sequence to send to PTY, or nil if focus reporting disabled.
    public func encodeFocusEvent(focused: Bool) -> Data? {
        var buffer = [UInt8](repeating: 0, count: 8)
        let len = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_encode_focus_event(handle, focused, ptr.baseAddress, UInt(8))
        }
        guard len > 0 else { return nil }
        return Data(buffer.prefix(Int(len)))
    }

    // MARK: - Palette

    /// Get a color from the 256-color palette.
    ///
    /// - Parameter index: Color index (0-255).
    /// - Returns: The RGB color, or nil if index is invalid.
    public func getPaletteColor(index: Int) -> DTermRGB? {
        var rgb = DtermRgb()
        guard dterm_terminal_get_palette_color(handle, UInt8(index), &rgb) else {
            return nil
        }
        return DTermRGB(from: rgb)
    }

    /// Set a color in the 256-color palette.
    ///
    /// - Parameters:
    ///   - index: Color index (0-255).
    ///   - color: The new RGB color.
    public func setPaletteColor(index: Int, color: DTermRGB) {
        dterm_terminal_set_palette_color(handle, UInt8(index), color.red, color.green, color.blue)
        checkPaletteChanges()
    }

    /// Reset the entire palette to defaults.
    public func resetPalette() {
        dterm_terminal_reset_palette(handle)
        checkPaletteChanges()
    }

    // MARK: - Private Helpers

    private func checkResponse() {
        guard dterm_terminal_has_response(handle) else { return }

        let len = dterm_terminal_response_len(handle)
        guard len > 0 else { return }

        var buffer = [UInt8](repeating: 0, count: Int(len))
        let read = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_read_response(handle, ptr.baseAddress, UInt(len))
        }

        if read > 0 {
            let data = Data(buffer.prefix(Int(read)))
            delegate?.terminalHasResponse(self, data: data)
        }
    }

    private func checkSixelImage() {
        guard dterm_terminal_has_sixel_image(handle) else { return }

        var image = DtermSixelImage()
        guard dterm_terminal_get_sixel_image(handle, &image) else { return }

        defer { dterm_sixel_image_free(image.pixels) }

        let pixelCount = Int(image.width) * Int(image.height)
        let pixels = Array(UnsafeBufferPointer(start: image.pixels, count: pixelCount))

        delegate?.terminalDidReceiveSixelImage(
            self,
            width: Int(image.width),
            height: Int(image.height),
            pixels: pixels
        )
    }

    private func checkKittyImages() {
        // Only check if dirty flag is set (optimization)
        guard dterm_terminal_kitty_is_dirty(handle) else { return }

        // Get all image IDs
        let count = dterm_terminal_kitty_image_count(handle)
        guard count > 0 else { return }

        var ids = [UInt32](repeating: 0, count: Int(count))
        let actual = dterm_terminal_kitty_image_ids(handle, &ids, count)

        // Check each image
        for i in 0..<Int(actual) {
            let imageId = ids[i]

            // Skip if we've already notified about this image
            if notifiedKittyImageIds.contains(imageId) {
                continue
            }

            // Get image info
            var info = DtermKittyImageInfo()
            guard dterm_terminal_kitty_get_image_info(handle, imageId, &info) else { continue }

            // Get pixel data
            var pixels: UnsafeMutablePointer<UInt8>?
            var pixelCount: UInt = 0
            guard dterm_terminal_kitty_get_image_pixels(handle, imageId, &pixels, &pixelCount),
                  let pixelPtr = pixels else { continue }

            defer { dterm_kitty_image_free(pixelPtr) }

            // Copy pixel data to Data object
            let data = Data(bytes: pixelPtr, count: Int(pixelCount))

            // Mark as notified
            notifiedKittyImageIds.insert(imageId)

            // Notify delegate
            delegate?.terminalDidReceiveKittyImage(
                self,
                id: info.id,
                width: info.width,
                height: info.height,
                data: data
            )
        }
    }

    private func checkPaletteChanges() {
        if paletteSnapshot.count != 256 {
            paletteSnapshot = capturePalette()
            return
        }

        var changedIndices: [Int] = []
        changedIndices.reserveCapacity(8)

        for index in 0..<256 {
            guard let color = getPaletteColor(index: index) else {
                continue
            }

            if paletteSnapshot[index] != color {
                paletteSnapshot[index] = color
                changedIndices.append(index)
            }
        }

        guard let delegate = delegate, !changedIndices.isEmpty else { return }
        for index in changedIndices {
            delegate.terminalColorDidChange(self, index: index)
        }
    }

    private func capturePalette() -> [DTermRGB] {
        var snapshot: [DTermRGB] = []
        snapshot.reserveCapacity(256)

        for index in 0..<256 {
            if let color = getPaletteColor(index: index) {
                snapshot.append(color)
            } else {
                snapshot.append(DTermRGB(red: 0, green: 0, blue: 0))
            }
        }

        return snapshot
    }
}

// MARK: - Library Version

/// Get the dterm-core library version.
public func dtermVersion() -> String {
    guard let ptr = dterm_version() else {
        return "unknown"
    }
    return String(cString: ptr)
}
