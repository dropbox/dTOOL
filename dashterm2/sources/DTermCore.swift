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

    /// Internal initializer for restoring from checkpoint.
    ///
    /// - Parameter restoredHandle: Handle from dterm_checkpoint_restore
    internal init(restoredHandle: OpaquePointer) {
        terminal = restoredHandle
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

    // MARK: - Mouse Tracking

    /// Whether mouse tracking is enabled.
    public var mouseTrackingEnabled: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_mouse_tracking_enabled(terminal)
    }

    /// Current mouse tracking mode.
    public var mouseMode: DTermMouseMode {
        guard let terminal = terminal else { return .none }
        return DTermMouseMode(dterm_terminal_mouse_mode(terminal))
    }

    /// Current mouse encoding format.
    public var mouseEncoding: DTermMouseEncoding {
        guard let terminal = terminal else { return .x10 }
        return DTermMouseEncoding(dterm_terminal_mouse_encoding(terminal))
    }

    // MARK: - Mouse Event Encoding

    /// Encode a mouse button press event for sending to the PTY.
    ///
    /// Returns the escape sequence to send, or nil if mouse reporting is disabled
    /// or parameters are invalid.
    ///
    /// - Parameters:
    ///   - button: Mouse button (0=left, 1=middle, 2=right)
    ///   - col: Column position (0-indexed)
    ///   - row: Row position (0-indexed)
    ///   - modifiers: Modifier keys (shift=4, meta=8, ctrl=16)
    /// - Returns: Escape sequence data to write to PTY, or nil if disabled
    public func encodeMousePress(button: UInt8, col: UInt16, row: UInt16, modifiers: UInt8 = 0) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: 32)
        let bytesWritten = dterm_terminal_encode_mouse_press(
            terminal, button, col, row, modifiers, &buffer, UInt(buffer.count)
        )
        guard bytesWritten > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesWritten)))
    }

    /// Encode a mouse button release event for sending to the PTY.
    ///
    /// Returns the escape sequence to send, or nil if mouse reporting is disabled
    /// or parameters are invalid.
    ///
    /// - Parameters:
    ///   - button: Original mouse button (0=left, 1=middle, 2=right)
    ///   - col: Column position (0-indexed)
    ///   - row: Row position (0-indexed)
    ///   - modifiers: Modifier keys (shift=4, meta=8, ctrl=16)
    /// - Returns: Escape sequence data to write to PTY, or nil if disabled
    public func encodeMouseRelease(button: UInt8, col: UInt16, row: UInt16, modifiers: UInt8 = 0) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: 32)
        let bytesWritten = dterm_terminal_encode_mouse_release(
            terminal, button, col, row, modifiers, &buffer, UInt(buffer.count)
        )
        guard bytesWritten > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesWritten)))
    }

    /// Encode a mouse motion event for sending to the PTY.
    ///
    /// Motion events are only sent in ButtonEvent (1002) or AnyEvent (1003) modes.
    /// Returns the escape sequence to send, or nil if motion tracking is not enabled.
    ///
    /// - Parameters:
    ///   - button: Button held during motion (0=left, 1=middle, 2=right, 3=none)
    ///   - col: Column position (0-indexed)
    ///   - row: Row position (0-indexed)
    ///   - modifiers: Modifier keys (shift=4, meta=8, ctrl=16)
    /// - Returns: Escape sequence data to write to PTY, or nil if disabled
    public func encodeMouseMotion(button: UInt8, col: UInt16, row: UInt16, modifiers: UInt8 = 0) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: 32)
        let bytesWritten = dterm_terminal_encode_mouse_motion(
            terminal, button, col, row, modifiers, &buffer, UInt(buffer.count)
        )
        guard bytesWritten > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesWritten)))
    }

    /// Encode a mouse wheel event for sending to the PTY.
    ///
    /// Returns the escape sequence to send, or nil if mouse reporting is disabled.
    ///
    /// - Parameters:
    ///   - up: True for wheel up, false for wheel down
    ///   - col: Column position (0-indexed)
    ///   - row: Row position (0-indexed)
    ///   - modifiers: Modifier keys (shift=4, meta=8, ctrl=16)
    /// - Returns: Escape sequence data to write to PTY, or nil if disabled
    public func encodeMouseWheel(up: Bool, col: UInt16, row: UInt16, modifiers: UInt8 = 0) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: 32)
        let bytesWritten = dterm_terminal_encode_mouse_wheel(
            terminal, up, col, row, modifiers, &buffer, UInt(buffer.count)
        )
        guard bytesWritten > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesWritten)))
    }

    /// Encode a focus event for sending to the PTY.
    ///
    /// Returns the escape sequence to send, or nil if focus reporting is disabled.
    ///
    /// - Parameter focused: True if window gained focus, false if lost focus
    /// - Returns: Escape sequence data to write to PTY, or nil if disabled
    public func encodeFocusEvent(focused: Bool) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: 8)
        let bytesWritten = dterm_terminal_encode_focus_event(
            terminal, focused, &buffer, UInt(buffer.count)
        )
        guard bytesWritten > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesWritten)))
    }

    // MARK: - Focus Reporting

    /// Whether focus reporting is enabled (mode 1004).
    public var focusReportingEnabled: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_focus_reporting_enabled(terminal)
    }

    // MARK: - Synchronized Output

    /// Whether synchronized output mode is enabled (mode 2026).
    ///
    /// When enabled, the terminal is in "batch update" mode and the renderer
    /// should defer drawing until the mode is disabled. This prevents screen
    /// tearing during rapid updates from applications like vim or tmux.
    public var synchronizedOutputEnabled: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_synchronized_output_enabled(terminal)
    }

    // MARK: - Response Buffer

    /// Whether the terminal has pending response data.
    ///
    /// Response data is generated by DSR/DA sequences and needs to be
    /// written back to the PTY.
    public var hasResponse: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_has_response(terminal)
    }

    /// Number of bytes in the pending response buffer.
    public var responseLength: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_response_len(terminal))
    }

    /// Read pending response data from the terminal.
    ///
    /// This should be called after processing input to check for any responses
    /// that need to be written back to the PTY (such as cursor position reports
    /// or device attribute responses). The data is consumed after reading.
    ///
    /// - Parameter maxBytes: Maximum number of bytes to read
    /// - Returns: Response data, or nil if no response pending
    public func readResponse(maxBytes: Int = 1024) -> Data? {
        guard let terminal = terminal else { return nil }
        var buffer = [UInt8](repeating: 0, count: maxBytes)
        let bytesRead = dterm_terminal_read_response(terminal, &buffer, UInt(maxBytes))
        guard bytesRead > 0 else { return nil }
        return Data(buffer.prefix(Int(bytesRead)))
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

    // Note: scrollbackHyperlinkAt is not yet implemented in dterm-core FFI.
    // Hyperlinks in scrollback will be added when the FFI is extended.

    // MARK: - Line Content Extraction

    /// Total number of lines (visible + scrollback).
    public var totalLines: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_total_lines(terminal))
    }

    /// Get text content of a line by absolute index.
    ///
    /// - Parameter lineIndex: Absolute line index (0 = first scrollback line)
    /// - Returns: UTF-8 text content of the line
    public func getLineText(lineIndex: Int) -> String {
        guard let terminal = terminal else { return "" }

        // First call to get required buffer size
        let requiredSize = dterm_terminal_get_line_text(terminal, UInt(lineIndex), nil, 0)
        guard requiredSize > 0 else { return "" }

        // Allocate buffer and get the text
        var buffer = [UInt8](repeating: 0, count: Int(requiredSize))
        let written = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_get_line_text(terminal, UInt(lineIndex), ptr.baseAddress, UInt(ptr.count))
        }

        guard written > 0 else { return "" }
        // Create string from UTF-8 bytes (excluding null terminator if present)
        let endIndex = written > 0 && buffer[Int(written) - 1] == 0 ? Int(written) - 1 : Int(written)
        return String(decoding: buffer.prefix(endIndex), as: UTF8.self)
    }

    /// Get text content of a visible row.
    ///
    /// - Parameter row: Row index (0 = top of visible area)
    /// - Returns: UTF-8 text content of the row
    public func getVisibleLineText(row: UInt16) -> String {
        guard let terminal = terminal else { return "" }

        // First call to get required buffer size
        let requiredSize = dterm_terminal_get_visible_line_text(terminal, row, nil, 0)
        guard requiredSize > 0 else { return "" }

        // Allocate buffer and get the text
        var buffer = [UInt8](repeating: 0, count: Int(requiredSize))
        let written = buffer.withUnsafeMutableBufferPointer { ptr in
            dterm_terminal_get_visible_line_text(terminal, row, ptr.baseAddress, UInt(ptr.count))
        }

        guard written > 0 else { return "" }
        let endIndex = written > 0 && buffer[Int(written) - 1] == 0 ? Int(written) - 1 : Int(written)
        return String(decoding: buffer.prefix(endIndex), as: UTF8.self)
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

    // MARK: - Hyperlinks (OSC 8)

    /// Get the hyperlink URL for a cell, if any.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed)
    ///   - col: Column index (0-indexed)
    /// - Returns: URL string, or nil if no hyperlink
    public func hyperlinkAt(row: UInt16, col: UInt16) -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_cell_hyperlink(terminal, row, col) else {
            return nil
        }
        return String(cString: cStr)
    }

    /// Check if a cell has a hyperlink.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed)
    ///   - col: Column index (0-indexed)
    /// - Returns: true if cell has hyperlink
    public func hasHyperlinkAt(row: UInt16, col: UInt16) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_cell_has_hyperlink(terminal, row, col)
    }

    /// Get the current active hyperlink URL being applied to new text.
    ///
    /// When an OSC 8 hyperlink sequence is received, subsequent text will have
    /// the hyperlink applied until the hyperlink is cleared.
    ///
    /// - Returns: Active hyperlink URL, or nil if no active hyperlink
    public func currentHyperlink() -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_current_hyperlink(terminal) else {
            return nil
        }
        return String(cString: cStr)
    }

    // MARK: - Reset

    /// Reset terminal to initial state.
    public func reset() {
        guard let terminal = terminal else { return }
        dterm_terminal_reset(terminal)
    }

    // MARK: - Sixel Images

    /// Check if a Sixel image is pending.
    ///
    /// When Sixel data is fully parsed, it becomes available via `getSixelImage()`.
    ///
    /// - Returns: true if a Sixel image is ready
    public var hasSixelImage: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_has_sixel_image(terminal)
    }

    /// Get the pending Sixel image.
    ///
    /// After retrieving the image, it is consumed and `hasSixelImage` returns false.
    /// The caller owns the returned image and must let it be deallocated when done.
    ///
    /// - Returns: Sixel image, or nil if none available
    public func getSixelImage() -> DTermSixelImage? {
        guard let terminal = terminal else { return nil }
        var cImage = DtermSixelImage(width: 0, height: 0, pixels: nil)
        guard dterm_terminal_get_sixel_image(terminal, &cImage) else { return nil }
        guard let pixels = cImage.pixels else { return nil }
        return DTermSixelImage(
            width: cImage.width,
            height: cImage.height,
            pixels: pixels
        )
    }

    // MARK: - Kitty Graphics

    /// Check if the terminal has any Kitty graphics images.
    public var hasKittyImages: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_kitty_has_images(terminal)
    }

    /// Check if Kitty graphics storage has changed since last render.
    ///
    /// Use this to determine if images need to be re-rendered.
    public var kittyIsDirty: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_kitty_is_dirty(terminal)
    }

    /// Clear the Kitty graphics dirty flag after rendering.
    public func clearKittyDirty() {
        guard let terminal = terminal else { return }
        dterm_terminal_kitty_clear_dirty(terminal)
    }

    /// Get the number of Kitty graphics images.
    public var kittyImageCount: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_kitty_image_count(terminal))
    }

    /// Get all Kitty image IDs.
    ///
    /// - Returns: Array of image IDs
    public func kittyImageIDs() -> [UInt32] {
        guard let terminal = terminal else { return [] }
        let count = dterm_terminal_kitty_image_count(terminal)
        guard count > 0 else { return [] }

        var ids = [UInt32](repeating: 0, count: Int(count))
        ids.withUnsafeMutableBufferPointer { buffer in
            _ = dterm_terminal_kitty_image_ids(terminal, buffer.baseAddress, count)
        }
        return ids
    }

    /// Get info about a Kitty graphics image.
    ///
    /// - Parameter imageID: The image ID
    /// - Returns: Image info, or nil if image doesn't exist
    public func kittyImageInfo(id imageID: UInt32) -> DTermKittyImageInfo? {
        guard let terminal = terminal else { return nil }
        var info = DtermKittyImageInfo(id: 0, number: 0, width: 0, height: 0, placement_count: 0)
        guard dterm_terminal_kitty_get_image_info(terminal, imageID, &info) else { return nil }
        return DTermKittyImageInfo(
            id: info.id,
            number: info.number,
            width: info.width,
            height: info.height,
            placementCount: info.placement_count
        )
    }

    /// Get pixel data for a Kitty graphics image.
    ///
    /// The pixel data is in RGBA format (4 bytes per pixel).
    ///
    /// - Parameter imageID: The image ID
    /// - Returns: Pixel data wrapper, or nil if image doesn't exist
    public func kittyImagePixels(id imageID: UInt32) -> DTermKittyImagePixels? {
        guard let terminal = terminal else { return nil }
        var pixels: UnsafeMutablePointer<UInt8>?
        var pixelCount: UInt = 0
        guard dterm_terminal_kitty_get_image_pixels(terminal, imageID, &pixels, &pixelCount) else {
            return nil
        }
        guard let pixelPtr = pixels else { return nil }
        return DTermKittyImagePixels(pixels: pixelPtr, count: Int(pixelCount))
    }

    /// Get the number of placements for a Kitty image.
    ///
    /// - Parameter imageID: The image ID
    /// - Returns: Number of placements
    public func kittyPlacementCount(imageID: UInt32) -> Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_kitty_placement_count(terminal, imageID))
    }

    /// Get placement IDs for a Kitty image.
    ///
    /// - Parameter imageID: The image ID
    /// - Returns: Array of placement IDs
    public func kittyPlacementIDs(imageID: UInt32) -> [UInt32] {
        guard let terminal = terminal else { return [] }
        let count = dterm_terminal_kitty_placement_count(terminal, imageID)
        guard count > 0 else { return [] }

        var ids = [UInt32](repeating: 0, count: Int(count))
        ids.withUnsafeMutableBufferPointer { buffer in
            _ = dterm_terminal_kitty_placement_ids(terminal, imageID, buffer.baseAddress, count)
        }
        return ids
    }

    /// Get a placement for a Kitty image.
    ///
    /// - Parameters:
    ///   - imageID: The image ID
    ///   - placementID: The placement ID
    /// - Returns: Placement info, or nil if placement doesn't exist
    public func kittyPlacement(imageID: UInt32, placementID: UInt32) -> DTermKittyPlacement? {
        guard let terminal = terminal else { return nil }
        var cPlacement = DtermKittyPlacement(
            id: 0,
            location_type: DTERM_KITTY_PLACEMENT_LOCATION_ABSOLUTE,
            row_or_parent_image: 0,
            col_or_parent_placement: 0,
            offset_x: 0,
            offset_y: 0,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            cell_x_offset: 0,
            cell_y_offset: 0,
            num_columns: 0,
            num_rows: 0,
            z_index: 0,
            is_virtual: false
        )
        guard dterm_terminal_kitty_get_placement(terminal, imageID, placementID, &cPlacement) else {
            return nil
        }
        return DTermKittyPlacement(from: cPlacement)
    }

    /// Get total bytes used by Kitty graphics storage.
    public var kittyTotalBytes: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_kitty_total_bytes(terminal))
    }

    /// Get Kitty graphics storage quota in bytes.
    public var kittyQuota: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_kitty_quota(terminal))
    }

    // MARK: - Memory Management (Directive 4)

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
    ///
    /// - Parameter budget: The memory budget configuration
    /// - Note: Only `maxBytes` is passed to dterm-core (0 = unlimited).
    ///   The `maxScrollbackLines` and `compressScrollback` fields are not yet
    ///   implemented in the FFI and are ignored.
    public func setMemoryBudget(_ budget: MemoryBudget) {
        guard let terminal = terminal else { return }
        dterm_terminal_set_memory_budget(terminal, UInt(budget.maxBytes))
    }

    // MARK: - Cell Access (Directive 4)

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
    /// - Returns: RGB color tuple (r, g, b), or (255, 255, 255) default for invalid cells
    public func cellForegroundRGB(row: UInt16, col: UInt16) -> (r: UInt8, g: UInt8, b: UInt8) {
        guard let terminal = terminal else { return (255, 255, 255) }
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        dterm_cell_fg_rgb(terminal, row, col, &r, &g, &b)
        return (r, g, b)
    }

    /// Get the background color as RGB for a cell.
    ///
    /// Handles both indexed colors (using the 256-color palette) and true color.
    /// For true color cells in the 8-byte cell format, this looks up the overflow table.
    ///
    /// - Parameters:
    ///   - row: Row index (0-based, visible area only)
    ///   - col: Column index (0-based)
    /// - Returns: RGB color tuple (r, g, b), or (0, 0, 0) default for invalid cells
    public func cellBackgroundRGB(row: UInt16, col: UInt16) -> (r: UInt8, g: UInt8, b: UInt8) {
        guard let terminal = terminal else { return (0, 0, 0) }
        var r: UInt8 = 0
        var g: UInt8 = 0
        var b: UInt8 = 0
        dterm_cell_bg_rgb(terminal, row, col, &r, &g, &b)
        return (r, g, b)
    }

    // MARK: - Palette Colors

    /// Get a color from the terminal's 256-color palette.
    ///
    /// The palette contains:
    /// - Indices 0-7: Standard ANSI colors
    /// - Indices 8-15: Bright ANSI colors
    /// - Indices 16-231: 6x6x6 color cube
    /// - Indices 232-255: Grayscale ramp
    ///
    /// - Parameter index: Palette index (0-255)
    /// - Returns: RGB color tuple, or nil if index is invalid or terminal is nil
    public func getPaletteColor(index: UInt8) -> (r: UInt8, g: UInt8, b: UInt8)? {
        guard let terminal = terminal else { return nil }
        var color = DtermRgb(r: 0, g: 0, b: 0)
        guard dterm_terminal_get_palette_color(terminal, index, &color) else {
            return nil
        }
        return (color.r, color.g, color.b)
    }

    /// Set a color in the terminal's 256-color palette.
    ///
    /// This allows customizing the terminal's color scheme at runtime.
    /// Changes affect how indexed colors are displayed.
    ///
    /// - Parameters:
    ///   - index: Palette index (0-255)
    ///   - r: Red component (0-255)
    ///   - g: Green component (0-255)
    ///   - b: Blue component (0-255)
    public func setPaletteColor(index: UInt8, r: UInt8, g: UInt8, b: UInt8) {
        guard let terminal = terminal else { return }
        dterm_terminal_set_palette_color(terminal, index, r, g, b)
    }

    /// Reset a single palette color to its default value.
    ///
    /// - Parameter index: Palette index (0-255)
    public func resetPaletteColor(index: UInt8) {
        guard let terminal = terminal else { return }
        dterm_terminal_reset_palette_color(terminal, index)
    }

    /// Reset the entire 256-color palette to default values.
    public func resetPalette() {
        guard let terminal = terminal else { return }
        dterm_terminal_reset_palette(terminal)
    }

    // MARK: - Text Selection

    /// Start a text selection.
    ///
    /// Call this when the user initiates a selection (e.g., mouse down).
    /// Use `selectionUpdate` on drag and `selectionEnd` on mouse up.
    ///
    /// - Parameters:
    ///   - col: Starting column (0-indexed)
    ///   - row: Starting row (0 = top of visible area, negative = scrollback)
    ///   - type: Type of selection (simple, block, semantic, or lines)
    public func selectionStart(col: UInt32, row: Int32, type: DTermSelectType) {
        guard let terminal = terminal else { return }
        dterm_terminal_selection_start(terminal, col, row, type.ffiValue)
    }

    /// Update selection endpoint.
    ///
    /// Call this on mouse drag to update the selection.
    /// Only works when selection is in progress (after `selectionStart`).
    ///
    /// - Parameters:
    ///   - col: Current column (0-indexed)
    ///   - row: Current row (0 = top of visible area, negative = scrollback)
    public func selectionUpdate(col: UInt32, row: Int32) {
        guard let terminal = terminal else { return }
        dterm_terminal_selection_update(terminal, col, row)
    }

    /// End the current selection.
    ///
    /// Call this when the user releases the mouse button.
    /// The selection remains visible until cleared with `selectionClear`.
    public func selectionEnd() {
        guard let terminal = terminal else { return }
        dterm_terminal_selection_end(terminal)
    }

    /// Clear the current selection.
    ///
    /// Removes any active or completed selection.
    public func selectionClear() {
        guard let terminal = terminal else { return }
        dterm_terminal_selection_clear(terminal)
    }

    /// Get the selected text as a string.
    ///
    /// - Returns: The selected text, or nil if no selection exists
    public func selectionToString() -> String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_selection_to_string(terminal) else {
            return nil
        }
        let result = String(cString: cStr)
        dterm_string_free(cStr)
        return result
    }

    // MARK: - Style Information

    /// Get the current terminal style (pen attributes).
    ///
    /// This returns the style that would be applied to newly written characters,
    /// including foreground/background colors and cell flags (bold, italic, etc.).
    ///
    /// - Returns: Current style attributes
    public func currentStyle() -> DTermStyle {
        guard let terminal = terminal else { return DTermStyle() }
        var style = dterm_style_t()
        dterm_terminal_get_style(terminal, &style)
        return DTermStyle(style)
    }

    // MARK: - Cell Complexity

    /// Check if a cell contains complex content (multi-codepoint grapheme).
    ///
    /// Complex cells contain characters that span multiple Unicode codepoints,
    /// such as emoji with ZWJ sequences, regional indicators, or combining marks.
    ///
    /// - Parameters:
    ///   - row: Row index (0-indexed)
    ///   - col: Column index (0-indexed)
    /// - Returns: true if the cell contains complex content
    public func cellIsComplex(at row: UInt16, col: UInt16) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_cell_is_complex(terminal, row, col)
    }

    // MARK: - Damage Tracking

    /// Get all damaged regions since the last render.
    ///
    /// Damaged regions indicate which parts of the screen have changed
    /// and need to be redrawn. After rendering, call `clearDamage()`.
    ///
    /// - Parameter maxCount: Maximum number of damage regions to return
    /// - Returns: Array of damage regions
    public func getDamage(maxCount: Int = 1000) -> [DTermRowDamage] {
        guard let terminal = terminal, maxCount > 0 else { return [] }
        var damages = [DtermRowDamage](repeating: DtermRowDamage(), count: maxCount)
        let count = dterm_terminal_get_damage(terminal, &damages, UInt(maxCount))
        let safeCount = min(Int(count), maxCount)
        return (0..<safeCount).map { DTermRowDamage(damages[$0]) }
    }

    /// Get the total count of damaged rows without fetching them.
    ///
    /// Useful for determining buffer size before calling `getDamage()`.
    ///
    /// - Returns: Number of damaged rows
    public func damageCount() -> Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_get_damage(terminal, nil, 0))
    }

    /// Check if a specific row is damaged.
    ///
    /// - Parameter row: Row index (0-indexed)
    /// - Returns: true if the row has changed since last clear
    public func rowIsDamaged(_ row: UInt16) -> Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_row_is_damaged(terminal, row)
    }

    /// Get damage bounds for a specific row.
    ///
    /// Returns the column range that has changed on the given row,
    /// or nil if the row is not damaged.
    ///
    /// - Parameter row: Row index (0-indexed)
    /// - Returns: Tuple of (left, right) column indices, or nil if not damaged
    public func rowDamageBounds(_ row: UInt16) -> (left: UInt16, right: UInt16)? {
        guard let terminal = terminal else { return nil }
        var left: UInt16 = 0
        var right: UInt16 = 0
        if dterm_terminal_get_row_damage(terminal, row, &left, &right) {
            return (left, right)
        }
        return nil
    }

    // MARK: - Smart Selection

    /// Get smart word boundaries at a position.
    ///
    /// Uses the smart selection engine to find word/semantic boundaries at the
    /// given position. This is useful for double-click word selection.
    ///
    /// - Parameters:
    ///   - smartSelection: Smart selection engine with rules
    ///   - row: Row index (0-indexed within visible area, or including scrollback)
    ///   - col: Column index (0-indexed)
    /// - Returns: Tuple of (start, end) column indices, or nil if no word found
    public func smartWordAt(
        selection smartSelection: DTermSmartSelection,
        row: UInt32,
        col: UInt32
    ) -> (start: UInt32, end: UInt32)? {
        guard let terminal = terminal,
              let selectionPtr = smartSelection.selection else { return nil }
        var start: UInt32 = 0
        var end: UInt32 = 0
        if dterm_terminal_smart_word_at(terminal, selectionPtr, row, col, &start, &end) {
            return (start, end)
        }
        return nil
    }

    /// Find a semantic match at a position.
    ///
    /// Returns detailed match information including the rule name and matched text.
    ///
    /// - Parameters:
    ///   - smartSelection: Smart selection engine with rules
    ///   - row: Row index
    ///   - col: Column index
    /// - Returns: Selection match, or nil if no match found
    public func smartMatchAt(
        selection smartSelection: DTermSmartSelection,
        row: UInt32,
        col: UInt32
    ) -> DTermSelectionMatch? {
        guard let terminal = terminal,
              let selectionPtr = smartSelection.selection else { return nil }
        guard let matchPtr = dterm_terminal_smart_match_at(terminal, selectionPtr, row, col) else {
            return nil
        }
        let match = DTermSelectionMatch(matchPtr.pointee)
        dterm_selection_match_free(matchPtr)
        return match
    }

    /// Count semantic matches on a row.
    ///
    /// - Parameters:
    ///   - smartSelection: Smart selection engine with rules
    ///   - row: Row index
    /// - Returns: Number of matches found
    public func smartMatchCount(
        selection smartSelection: DTermSmartSelection,
        row: UInt32
    ) -> Int {
        guard let terminal = terminal,
              let selectionPtr = smartSelection.selection else { return 0 }
        return Int(dterm_terminal_smart_match_count(terminal, selectionPtr, row))
    }

    /// Get all semantic matches on a row.
    ///
    /// - Parameters:
    ///   - smartSelection: Smart selection engine with rules
    ///   - row: Row index
    ///   - maxMatches: Maximum matches to return
    /// - Returns: Array of selection matches
    public func smartMatchesOnRow(
        selection smartSelection: DTermSmartSelection,
        row: UInt32,
        maxMatches: Int = 100
    ) -> [DTermSelectionMatch] {
        guard let terminal = terminal,
              let selectionPtr = smartSelection.selection,
              maxMatches > 0 else { return [] }

        var matchPtrs = [UnsafeMutablePointer<DtermSelectionMatch>?](
            repeating: nil,
            count: maxMatches
        )
        let count = dterm_terminal_smart_matches_on_row(
            terminal,
            selectionPtr,
            row,
            &matchPtrs,
            UInt32(maxMatches)
        )
        let safeCount = min(Int(count), maxMatches)
        var matches: [DTermSelectionMatch] = []
        for i in 0..<safeCount {
            if let ptr = matchPtrs[i] {
                matches.append(DTermSelectionMatch(ptr.pointee))
                dterm_selection_match_free(ptr)
            }
        }
        return matches
    }

    // MARK: - Terminal Callbacks

    /// Set a callback to be invoked when the terminal bell is triggered.
    ///
    /// The bell can be triggered by the BEL character (0x07) or CSI sequences.
    ///
    /// - Parameter handler: Closure to call when bell is triggered, or nil to disable.
    ///                      The closure receives no parameters.
    public func setBellHandler(_ handler: (() -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermBellCallback = { ctx in
                guard let ctx = ctx else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? () -> Void {
                    handler()
                }
            }
            dterm_terminal_set_bell_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_bell_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked when the terminal title changes.
    ///
    /// Title changes are triggered by OSC 0 or OSC 2 escape sequences.
    ///
    /// - Parameter handler: Closure to call with the new title, or nil to disable.
    public func setTitleHandler(_ handler: ((String) -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermTitleCallback = { ctx, titlePtr in
                guard let ctx = ctx, let titlePtr = titlePtr else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (String) -> Void {
                    handler(String(cString: titlePtr))
                }
            }
            dterm_terminal_set_title_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_title_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked when the terminal switches buffers.
    ///
    /// The terminal switches between main and alternate screen buffers when
    /// entering/exiting full-screen applications like vim, less, etc.
    ///
    /// - Parameter handler: Closure to call with true for alternate screen,
    ///                      false for main screen. Pass nil to disable.
    public func setBufferActivationHandler(_ handler: ((Bool) -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermBufferActivationCallback = { ctx, isAlternate in
                guard let ctx = ctx else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (Bool) -> Void {
                    handler(isAlternate)
                }
            }
            dterm_terminal_set_buffer_activation_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_buffer_activation_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked for shell integration events.
    ///
    /// Shell integration events include prompt markers, command boundaries,
    /// and working directory changes (OSC 133, OSC 7).
    ///
    /// - Parameter handler: Closure to call with the shell event, or nil to disable.
    public func setShellEventHandler(_ handler: ((DTermShellEvent) -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermShellEventCallback = { ctx, eventPtr in
                guard let ctx = ctx, let eventPtr = eventPtr else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (DTermShellEvent) -> Void {
                    handler(DTermShellEvent(eventPtr.pointee))
                }
            }
            dterm_terminal_set_shell_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_shell_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked for window manipulation commands.
    ///
    /// Window commands are triggered by CSI t (XTWINOPS) escape sequences.
    ///
    /// - Parameter handler: Closure to call with the window operation.
    ///                      Returns optional response for query operations.
    ///                      Pass nil to disable.
    public func setWindowHandler(_ handler: ((DTermWindowOp) -> DTermWindowResponse?)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermWindowCallback = { ctx, opPtr, responsePtr in
                guard let ctx = ctx, let opPtr = opPtr else { return false }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (DTermWindowOp) -> DTermWindowResponse? {
                    let op = DTermWindowOp(opPtr.pointee)
                    if let response = handler(op) {
                        responsePtr?.pointee = response.toFFI()
                        return true
                    }
                    return false
                }
                return false
            }
            dterm_terminal_set_window_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_window_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked for Kitty graphics images.
    ///
    /// This callback is invoked when a Kitty graphics image is successfully
    /// transmitted and stored by the terminal.
    ///
    /// - Parameter handler: Closure to call with image data, or nil to disable.
    public func setKittyImageHandler(_ handler: ((DTermKittyImage) -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermKittyImageCallback = { ctx, id, width, height, dataPtr, dataLen in
                guard let ctx = ctx, let dataPtr = dataPtr else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (DTermKittyImage) -> Void {
                    let data = Data(bytes: dataPtr, count: Int(dataLen))
                    let image = DTermKittyImage(id: id, width: width, height: height, rgbaData: data)
                    handler(image)
                }
            }
            dterm_terminal_set_kitty_image_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_kitty_image_callback(terminal, nil, nil)
        }
    }

    /// Set a callback to be invoked for DCS (Device Control String) sequences.
    ///
    /// DCS sequences are used for various device-specific controls.
    ///
    /// - Parameter handler: Closure to call with DCS data, or nil to disable.
    public func setDCSHandler(_ handler: ((Data, UInt8) -> Void)?) {
        guard let terminal = terminal else { return }
        if let handler = handler {
            let context = Unmanaged.passRetained(handler as AnyObject).toOpaque()
            let callback: DtermDCSCallback = { ctx, dataPtr, len, finalByte in
                guard let ctx = ctx, let dataPtr = dataPtr else { return }
                let handlerObj = Unmanaged<AnyObject>.fromOpaque(ctx).takeUnretainedValue()
                if let handler = handlerObj as? (Data, UInt8) -> Void {
                    let data = Data(bytes: dataPtr, count: Int(len))
                    handler(data, finalByte)
                }
            }
            dterm_terminal_set_dcs_callback(terminal, callback, context)
        } else {
            dterm_terminal_set_dcs_callback(terminal, nil, nil)
        }
    }

    // MARK: - Terminal Queries

    /// Get the current cursor style.
    ///
    /// - Returns: Cursor style (1-6 following DECSCUSR values), or 0 if unknown.
    public var cursorStyle: UInt8 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cursor_style(terminal)
    }

    /// Check if the terminal has an active selection.
    public var hasSelection: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_has_selection(terminal)
    }

    /// Get the icon name (set by OSC 1).
    ///
    /// - Returns: Icon name string, or nil if not set.
    public var iconName: String? {
        guard let terminal = terminal else { return nil }
        guard let cStr = dterm_terminal_icon_name(terminal) else { return nil }
        return String(cString: cStr)
    }

    /// Get the current working directory.
    ///
    /// - Returns: Directory path, or nil if not set.
    public var currentWorkingDirectory: String? {
        guard let terminal = terminal else { return nil }
        guard dterm_terminal_has_working_directory(terminal) else { return nil }
        guard let cStr = dterm_terminal_current_working_directory(terminal) else { return nil }
        return String(cString: cStr)
    }

    /// Check if a working directory is set.
    public var hasWorkingDirectory: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_has_working_directory(terminal)
    }

    /// Get text content of a row.
    ///
    /// - Parameter row: Row index (0-indexed, can include scrollback).
    /// - Returns: Row text, or nil if out of bounds.
    ///
    /// - Note: Uses existing getVisibleLineText which is more reliable.
    ///         This method is a direct FFI wrapper for dterm_terminal_row_text.
    public func rowText(at row: UInt16) -> String? {
        guard let terminal = terminal else { return nil }
        // The FFI function returns an owned pointer that must be freed
        let cStr = dterm_terminal_row_text(terminal, row)
        guard let cStr = cStr else { return nil }
        let result = String(cString: cStr)
        // Free the allocated string
        dterm_string_free(UnsafeMutablePointer(mutating: cStr))
        return result
    }

    /// Get display string for a cell (handles complex/combined characters).
    ///
    /// - Parameters:
    ///   - row: Row index
    ///   - col: Column index
    /// - Returns: Display string for the cell, or nil if empty/invalid.
    public func cellDisplayString(row: UInt16, col: UInt16) -> String? {
        guard let terminal = terminal else { return nil }
        // The FFI function returns an owned pointer that must be freed
        let cStr = dterm_terminal_cell_display_string(terminal, row, col)
        guard let cStr = cStr else { return nil }
        let result = String(cString: cStr)
        // Free the allocated string
        dterm_string_free(UnsafeMutablePointer(mutating: cStr))
        return result
    }

    // MARK: - Line Attributes

    /// Get the line size attribute for a row.
    ///
    /// Line size attributes control how characters are rendered:
    /// - Single width: Normal character width (default)
    /// - Double width: Each character is rendered double-wide (DECDWL)
    /// - Double height top: Top half of double-height line (DECDHL)
    /// - Double height bottom: Bottom half of double-height line (DECDHL)
    ///
    /// - Parameter row: Row index (0-indexed)
    /// - Returns: Line size attribute for the row
    public func rowLineSize(at row: UInt16) -> DTermLineSize {
        guard let terminal = terminal else { return .singleWidth }
        return DTermLineSize(dterm_terminal_row_line_size(terminal, row))
    }

}

// MARK: - DTermStyle

/// Current terminal style attributes (pen state).
///
/// Represents the style that would be applied to newly written characters.
public struct DTermStyle {
    /// Foreground color (packed).
    public let foreground: DTermColor

    /// Background color (packed).
    public let background: DTermColor

    /// Cell flags (bold, italic, underline, etc.).
    public let flags: CellFlags

    init() {
        self.foreground = .default
        self.background = .default
        self.flags = []
    }

    init(_ style: dterm_style_t) {
        self.foreground = DTermColor(packed: style.fg)
        self.background = DTermColor(packed: style.bg)
        self.flags = CellFlags(rawValue: UInt32(style.flags))
    }

    /// Whether bold attribute is set.
    public var isBold: Bool { flags.contains(.bold) }

    /// Whether italic attribute is set.
    public var isItalic: Bool { flags.contains(.italic) }

    /// Whether underline attribute is set.
    public var isUnderline: Bool { flags.contains(.underline) }

    /// Whether strikethrough attribute is set.
    public var isStrikethrough: Bool { flags.contains(.strikethrough) }

    /// Whether blink attribute is set.
    public var isBlink: Bool { flags.contains(.blink) }

    /// Whether inverse/reverse video is set.
    public var isInverse: Bool { flags.contains(.inverse) }

    /// Whether hidden/invisible attribute is set.
    public var isHidden: Bool { flags.contains(.invisible) }

    /// Whether dim/faint attribute is set.
    public var isDim: Bool { flags.contains(.dim) }
}

// MARK: - DTermSmartSelection

/// Smart selection engine for semantic text selection.
///
/// Provides intelligent word boundaries and pattern matching for text selection.
/// Uses built-in rules for URLs, paths, emails, and other common patterns.
///
/// Thread Safety: NOT thread-safe. Use external synchronization if needed.
public final class DTermSmartSelection {
    fileprivate var selection: OpaquePointer?

    /// Create a smart selection engine with all built-in rules.
    public init() {
        selection = dterm_smart_selection_new()
    }

    /// Create an empty smart selection engine (no rules).
    ///
    /// Use this if you want to add only custom rules.
    public init(empty: Bool) {
        if empty {
            selection = dterm_smart_selection_new_empty()
        } else {
            selection = dterm_smart_selection_new()
        }
    }

    deinit {
        if let selection = selection {
            dterm_smart_selection_free(selection)
        }
    }

    /// Enable or disable a rule by name.
    ///
    /// Built-in rule names include: "url", "email", "path", "ipv4", "ipv6", etc.
    ///
    /// - Parameters:
    ///   - name: Rule name
    ///   - enabled: Whether to enable the rule
    /// - Returns: true if rule was found, false otherwise
    @discardableResult
    public func setRuleEnabled(_ name: String, enabled: Bool) -> Bool {
        guard let selection = selection else { return false }
        return name.withCString { cStr in
            dterm_smart_selection_set_rule_enabled(selection, cStr, enabled)
        }
    }
}

// MARK: - DTermSelectionMatch

/// Result of a smart selection match.
public struct DTermSelectionMatch {
    /// Start byte offset in the text.
    public let start: UInt32

    /// End byte offset in the text (exclusive).
    public let end: UInt32

    /// Rule name that matched.
    public let ruleName: String

    /// Matched text content.
    public let matchedText: String

    /// Kind of match.
    public let kind: DTermSelectionKind

    init(_ match: DtermSelectionMatch) {
        self.start = match.start
        self.end = match.end
        if let ruleNamePtr = match.rule_name {
            self.ruleName = String(cString: ruleNamePtr)
        } else {
            self.ruleName = ""
        }
        if let textPtr = match.matched_text {
            self.matchedText = String(cString: textPtr)
        } else {
            self.matchedText = ""
        }
        self.kind = DTermSelectionKind(rawValue: match.kind) ?? .word
    }
}

/// Kind of smart selection match.
public enum DTermSelectionKind: UInt8 {
    /// Word boundary selection
    case word = 0
    /// URL pattern
    case url = 1
    /// Email pattern
    case email = 2
    /// File path pattern
    case path = 3
    /// IP address pattern
    case ipAddress = 4
    /// Custom pattern
    case custom = 255
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
        self.flags = CellFlags(rawValue: UInt32(cell.flags))
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
        // dterm FFI returns uint16_t flags, cast to UInt32 for API stability
        self.flags = CellFlags(rawValue: UInt32(cell.flags))
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
///
/// Bit layout matches dterm-core FFI DtermCell.flags (u16):
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
/// - Bit 11: SUPERSCRIPT
/// - Bit 12: SUBSCRIPT
public struct CellFlags: OptionSet, Sendable {
    public let rawValue: UInt32

    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    // Bit positions from dterm-core FFI (uint16_t flags in dterm_cell_t)
    // Must match dterm-core/src/grid/cell.rs CellFlags exactly
    public static let bold = CellFlags(rawValue: 1 << 0)
    public static let dim = CellFlags(rawValue: 1 << 1)
    public static let italic = CellFlags(rawValue: 1 << 2)
    public static let underline = CellFlags(rawValue: 1 << 3)
    public static let blink = CellFlags(rawValue: 1 << 4)
    public static let inverse = CellFlags(rawValue: 1 << 5)
    public static let invisible = CellFlags(rawValue: 1 << 6)
    public static let strikethrough = CellFlags(rawValue: 1 << 7)
    public static let doubleUnderline = CellFlags(rawValue: 1 << 8)  // DOUBLE_UNDERLINE
    public static let wide = CellFlags(rawValue: 1 << 9)             // WIDE
    public static let wideSpacer = CellFlags(rawValue: 1 << 10)      // WIDE_CONTINUATION
    public static let superscript = CellFlags(rawValue: 1 << 11)     // SUPERSCRIPT (SGR 73)
    public static let `subscript` = CellFlags(rawValue: 1 << 12)     // SUBSCRIPT (SGR 74)
    public static let curlyUnderline = CellFlags(rawValue: 1 << 13)  // CURLY_UNDERLINE
    // Bit 14: Reserved
    public static let complex = CellFlags(rawValue: 1 << 15)         // COMPLEX (overflow char)

    /// Check if double underline is active (bit 8 set)
    public var isDoubleUnderline: Bool {
        contains(.doubleUnderline)
    }
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
        // - 0x00_INDEX__: Indexed color (0-255) - type byte is 0x00 (fg/bg)
        // - 0x01_RRGGBB: True color RGB - type byte is 0x01
        // - 0x02_0000NN: Indexed underline color - type byte is 0x02
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
        case 0x02:
            // Indexed underline color (same as 0x00 but different type byte)
            self = .indexed(UInt8(packed & 0xFF))
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

// MARK: - Mouse Mode

/// Mouse tracking mode.
///
/// Controls when mouse events are reported to the application.
@objc public enum DTermMouseMode: UInt32 {
    /// No mouse tracking (default).
    case none = 0
    /// Normal tracking mode (1000) - report button press/release.
    case normal = 1
    /// Button-event tracking mode (1002) - report press/release and motion while button pressed.
    case buttonEvent = 2
    /// Any-event tracking mode (1003) - report all motion events.
    case anyEvent = 3

    init(_ ffiMode: DtermMouseMode) {
        switch ffiMode {
        case DTERM_MOUSE_MODE_NONE: self = .none
        case DTERM_MOUSE_MODE_NORMAL: self = .normal
        case DTERM_MOUSE_MODE_BUTTON_EVENT: self = .buttonEvent
        case DTERM_MOUSE_MODE_ANY_EVENT: self = .anyEvent
        default: self = .none
        }
    }
}

/// Mouse encoding format.
///
/// Controls how mouse coordinates are encoded in escape sequences.
@objc public enum DTermMouseEncoding: UInt32 {
    /// X10 compatibility mode - coordinates encoded as single bytes (limited to 223).
    case x10 = 0
    /// UTF-8 encoding (1005) - coordinates as UTF-8 characters, supports up to 2015.
    case utf8 = 1
    /// SGR encoding (1006) - coordinates as decimal parameters, supports larger values.
    case sgr = 2
    /// URXVT encoding (1015) - decimal parameters without '<' prefix.
    case urxvt = 3
    /// SGR pixel mode (1016) - like SGR but coordinates are in pixels.
    case sgrPixel = 4

    init(_ ffiEncoding: DtermMouseEncoding) {
        switch ffiEncoding {
        case DTERM_MOUSE_ENCODING_X10: self = .x10
        case DTERM_MOUSE_ENCODING_UTF8: self = .utf8
        case DTERM_MOUSE_ENCODING_SGR: self = .sgr
        case DTERM_MOUSE_ENCODING_URXVT: self = .urxvt
        case DTERM_MOUSE_ENCODING_SGR_PIXEL: self = .sgrPixel
        default: self = .x10
        }
    }
}

// MARK: - Selection Type

/// Selection type for text selection.
///
/// Controls how text is selected in the terminal.
@objc public enum DTermSelectType: UInt8 {
    /// Character-by-character selection (single click + drag).
    case simple = 0
    /// Block/rectangular selection (Option/Alt + drag).
    case block = 1
    /// Semantic selection - words, URLs, etc. (double-click).
    case semantic = 2
    /// Full line selection (triple-click).
    case lines = 3

    /// Convert to FFI value (UInt8 matching DtermSelectionType).
    var ffiValue: UInt8 {
        return rawValue
    }
}

// MARK: - Line Size

/// Line size for DEC line attributes (DECDHL/DECDWL).
///
/// Controls how characters on a line are rendered.
@objc public enum DTermLineSize: UInt32 {
    /// Normal single-width, single-height line (DECSWL).
    case singleWidth = 0
    /// Double-width line (DECDWL) - each character is rendered double-wide.
    case doubleWidth = 1
    /// Top half of double-height line (DECDHL).
    case doubleHeightTop = 2
    /// Bottom half of double-height line (DECDHL).
    case doubleHeightBottom = 3

    init(_ ffiValue: DtermLineSize) {
        switch ffiValue {
        case DTERM_LINE_SIZE_SINGLE_WIDTH: self = .singleWidth
        case DTERM_LINE_SIZE_DOUBLE_WIDTH: self = .doubleWidth
        case DTERM_LINE_SIZE_DOUBLE_HEIGHT_TOP: self = .doubleHeightTop
        case DTERM_LINE_SIZE_DOUBLE_HEIGHT_BOTTOM: self = .doubleHeightBottom
        default: self = .singleWidth
        }
    }

    /// Whether characters should be rendered at double width.
    public var isDoubleWidth: Bool {
        return self == .doubleWidth || self == .doubleHeightTop || self == .doubleHeightBottom
    }

    /// Whether this is part of a double-height line pair.
    public var isDoubleHeight: Bool {
        return self == .doubleHeightTop || self == .doubleHeightBottom
    }
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
    public var mouseMode: DTermMouseMode = .none

    /// Mouse encoding format (X10 or SGR/1006).
    public var mouseEncoding: DTermMouseEncoding = .x10

    /// Focus reporting mode (1004).
    public var focusReporting: Bool = false

    /// Synchronized output mode (2026).
    /// When enabled, rendering should be deferred to prevent tearing.
    public var synchronizedOutput: Bool = false

    /// Reverse video mode (DECSET 5).
    /// When enabled, screen colors are inverted.
    public var reverseVideo: Bool = false

    /// Cursor blink mode (DECSET 12).
    public var cursorBlink: Bool = false

    /// Application keypad mode (DECKPAM/DECKPNM).
    /// When enabled, keypad sends application sequences.
    public var applicationKeypad: Bool = false

    /// 132 column mode (DECSET 3).
    public var columnMode132: Bool = false

    /// Reverse wraparound mode (DECSET 45).
    /// When enabled, backspace at column 0 wraps to previous line.
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
        self.mouseMode = DTermMouseMode(modes.mouse_mode)
        self.mouseEncoding = DTermMouseEncoding(modes.mouse_encoding)
        self.focusReporting = modes.focus_reporting
        self.synchronizedOutput = modes.synchronized_output
        self.reverseVideo = modes.reverse_video
        self.cursorBlink = modes.cursor_blink
        self.applicationKeypad = modes.application_keypad
        self.columnMode132 = modes.column_mode_132
        self.reverseWraparound = modes.reverse_wraparound
    }
}

// MARK: - Shell Integration State

/// Shell integration state (OSC 133).
///
/// Tracks the current state of shell integration, which allows the terminal
/// to understand command boundaries and output blocks.
@objc public enum DTermShellState: UInt32 {
    /// Ground state - waiting for prompt.
    case ground = 0
    /// Receiving prompt text (after OSC 133 ; A).
    case receivingPrompt = 1
    /// User is entering command (after OSC 133 ; B).
    case enteringCommand = 2
    /// Command is executing (after OSC 133 ; C).
    case executing = 3
}

/// Output block state for block-based terminal model.
@objc public enum DTermBlockState: UInt32 {
    /// Only prompt has been received.
    case promptOnly = 0
    /// User is entering a command.
    case enteringCommand = 1
    /// Command is executing.
    case executing = 2
    /// Command has completed with exit code.
    case complete = 3
}

/// An output block representing a command and its output.
///
/// Output blocks are the fundamental unit of the block-based terminal model.
/// Each block contains a prompt, optional command, and optional output.
/// This is essential for AI agents that need to understand command boundaries.
@objc public final class DTermOutputBlock: NSObject {
    /// Unique identifier for this block.
    @objc public let id: UInt64

    /// Current state of this block.
    @objc public let state: DTermBlockState

    /// Row where the prompt started (absolute line number).
    @objc public let promptStartRow: Int

    /// Column where the prompt started.
    @objc public let promptStartCol: UInt16

    /// Row where the command text started (0 if not set).
    @objc public let commandStartRow: Int

    /// Column where the command text started (0 if not set).
    @objc public let commandStartCol: UInt16

    /// Whether command_start is valid.
    @objc public let hasCommandStart: Bool

    /// Row where command output started (0 if not set).
    @objc public let outputStartRow: Int

    /// Whether output_start_row is valid.
    @objc public let hasOutputStart: Bool

    /// Row where this block ends (exclusive).
    @objc public let endRow: Int

    /// Whether end_row is valid.
    @objc public let hasEndRow: Bool

    /// Command exit code (only valid if state is Complete).
    @objc public let exitCode: Int32

    /// Whether exit_code is valid.
    @objc public let hasExitCode: Bool

    init(_ block: DtermOutputBlock) {
        self.id = block.id
        self.state = DTermBlockState(rawValue: block.state.rawValue) ?? .promptOnly
        self.promptStartRow = Int(block.prompt_start_row)
        self.promptStartCol = block.prompt_start_col
        self.commandStartRow = Int(block.command_start_row)
        self.commandStartCol = block.command_start_col
        self.hasCommandStart = block.has_command_start
        self.outputStartRow = Int(block.output_start_row)
        self.hasOutputStart = block.has_output_start
        self.endRow = Int(block.end_row)
        self.hasEndRow = block.has_end_row
        self.exitCode = block.exit_code
        self.hasExitCode = block.has_exit_code
        super.init()
    }

    @objc public override var description: String {
        var desc = "Block(\(id)): state=\(state.rawValue)"
        desc += " prompt=(\(promptStartRow),\(promptStartCol))"
        if hasCommandStart {
            desc += " cmd=(\(commandStartRow),\(commandStartCol))"
        }
        if hasOutputStart {
            desc += " out=\(outputStartRow)"
        }
        if hasEndRow {
            desc += " end=\(endRow)"
        }
        if hasExitCode {
            desc += " exit=\(exitCode)"
        }
        return desc
    }
}

// MARK: - DTermCore Shell Integration Extension

extension DTermCore {
    /// Get the current shell integration state.
    ///
    /// Shell integration (OSC 133) allows the terminal to track:
    /// - When a prompt is displayed
    /// - When the user is entering a command
    /// - When a command is executing
    /// - When a command completes
    ///
    /// This is essential for AI agents that need to understand command boundaries.
    public var shellState: DTermShellState {
        guard let terminal = terminal else { return .ground }
        let state = dterm_terminal_shell_state(terminal)
        return DTermShellState(rawValue: state.rawValue) ?? .ground
    }

    /// Get the number of completed output blocks.
    ///
    /// Output blocks track command/output pairs for the block-based terminal model.
    public var blockCount: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_block_count(terminal))
    }

    /// Get an output block by index.
    ///
    /// - Parameter index: Block index (0 = oldest block)
    /// - Returns: OutputBlock if found, nil otherwise
    public func block(at index: Int) -> DTermOutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_block(terminal, UInt(index), &block) {
            return DTermOutputBlock(block)
        }
        return nil
    }

    /// Get the current (in-progress) output block.
    ///
    /// This returns the block that is currently receiving input/output.
    /// - Returns: Current block if one exists, nil otherwise
    public var currentBlock: DTermOutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_current_block(terminal, &block) {
            return DTermOutputBlock(block)
        }
        return nil
    }

    /// Find the output block containing a given row.
    ///
    /// - Parameter row: Absolute row number
    /// - Returns: Block index if found, nil if no block contains the row
    public func blockIndex(atRow row: Int) -> Int? {
        guard let terminal = terminal else { return nil }
        let index = dterm_terminal_block_at_row(terminal, UInt(row))
        // dterm returns usize::MAX if not found
        if index == UInt.max {
            return nil
        }
        return Int(index)
    }

    /// Get the exit code of the last completed block.
    ///
    /// - Returns: Exit code if a completed block exists, nil otherwise
    public var lastExitCode: Int32? {
        guard let terminal = terminal else { return nil }
        var exitCode: Int32 = 0
        if dterm_terminal_last_exit_code(terminal, &exitCode) {
            return exitCode
        }
        return nil
    }

    /// Get all output blocks.
    ///
    /// - Returns: Array of all output blocks in chronological order
    public var allBlocks: [DTermOutputBlock] {
        guard let terminal = terminal else { return [] }
        let count = Int(dterm_terminal_block_count(terminal))
        var blocks: [DTermOutputBlock] = []
        blocks.reserveCapacity(count)
        for i in 0..<count {
            var block = DtermOutputBlock()
            if dterm_terminal_get_block(terminal, UInt(i), &block) {
                blocks.append(DTermOutputBlock(block))
            }
        }
        return blocks
    }
}

// MARK: - DTermRowDamage

/// Damage bounds for a single row.
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

// MARK: - Version

/// Get dterm-core library version.
public func dtermVersion() -> String {
    guard let cStr = dterm_version() else { return "unknown" }
    return String(cString: cStr)
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

    /// Search for a query string with results in specified order.
    ///
    /// Unlike `find()`, this method guarantees results are sorted by the specified
    /// direction (forward = oldest to newest, backward = newest to oldest).
    ///
    /// - Parameters:
    ///   - query: Text to search for
    ///   - direction: Search direction (determines result ordering)
    ///   - maxMatches: Maximum number of matches to return
    /// - Returns: Array of search matches in specified order
    public func findOrdered(
        _ query: String,
        direction: DTermSearchDirection,
        maxMatches: Int = 100
    ) -> [DTermSearchMatch] {
        // Guard against empty query to prevent Rust FFI panic
        guard let search = search, maxMatches > 0, !query.isEmpty else { return [] }
        let queryData = query.utf8
        return Array(queryData).withUnsafeBufferPointer { buffer in
            var matches = [dterm_search_match_t](repeating: dterm_search_match_t(), count: maxMatches)
            let count = dterm_search_find_ordered(
                search,
                buffer.baseAddress,
                UInt(buffer.count),
                direction.ffiValue,
                &matches,
                UInt(maxMatches)
            )
            // Clamp count to maxMatches to prevent out-of-bounds access
            let safeCount = min(Int(count), maxMatches)
            return (0..<safeCount).map { DTermSearchMatch(matches[$0]) }
        }
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

/// Search direction for ordered search.
@objc public enum DTermSearchDirection: UInt32 {
    /// Search forward (oldest to newest).
    case forward = 0
    /// Search backward (newest to oldest).
    case backward = 1

    /// Convert to FFI value.
    var ffiValue: dterm_search_direction_t {
        switch self {
        case .forward: return DTERM_SEARCH_DIRECTION_T_FORWARD
        case .backward: return DTERM_SEARCH_DIRECTION_T_BACKWARD
        }
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
    /// Initial viewport width in pixels
    public var initialWidth: UInt32

    /// Initial viewport height in pixels
    public var initialHeight: UInt32

    /// Display scale factor (e.g., 2.0 for Retina)
    public var scaleFactor: Float

    /// Background color (r, g, b, a)
    public var backgroundColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8)

    /// Whether to enable vsync
    public var vsync: Bool

    /// Target FPS when vsync is disabled
    public var targetFPS: UInt32

    /// Maximum time to wait for a drawable (milliseconds)
    public var drawableTimeoutMs: UInt64

    /// Whether to enable damage-based rendering
    public var damageRendering: Bool

    /// Cursor style
    public var cursorStyle: DtermCursorStyle

    /// Cursor blink rate in milliseconds (0 = no blinking)
    public var cursorBlinkMs: UInt32

    /// Selection color (r, g, b, a)
    public var selectionColor: (r: UInt8, g: UInt8, b: UInt8, a: UInt8)

    /// Default configuration (black background, vsync enabled).
    public static var `default`: DTermRendererConfig {
        DTermRendererConfig(
            initialWidth: 800,
            initialHeight: 600,
            scaleFactor: 2.0,
            backgroundColor: (0, 0, 0, 255),
            vsync: true,
            targetFPS: 60,
            drawableTimeoutMs: 17,
            damageRendering: true,
            cursorStyle: DTERM_CURSOR_STYLE_BLOCK,
            cursorBlinkMs: 530,
            selectionColor: (100, 149, 237, 128)  // Cornflower blue with alpha
        )
    }

    /// Convert to FFI config.
    func toFFI() -> DtermRendererConfig {
        return DtermRendererConfig(
            initial_width: initialWidth,
            initial_height: initialHeight,
            scale_factor: scaleFactor,
            background_r: backgroundColor.r,
            background_g: backgroundColor.g,
            background_b: backgroundColor.b,
            background_a: backgroundColor.a,
            vsync: vsync,
            target_fps: targetFPS,
            drawable_timeout_ms: drawableTimeoutMs,
            damage_rendering: damageRendering,
            cursor_style: cursorStyle,
            cursor_blink_ms: cursorBlinkMs,
            selection_r: selectionColor.r,
            selection_g: selectionColor.g,
            selection_b: selectionColor.b,
            selection_a: selectionColor.a
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
/// if let handle = handle {
///     let status = renderer.waitForFrame(handle, timeoutMs: 16)
///     if status == .ready {
///         // Render...
///     }
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
        var defaultConfig = DTermRendererConfig.default.toFFI()
        handle = dterm_renderer_create(&defaultConfig)
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
    /// - Parameters:
    ///   - frameHandle: The frame handle to wait for.
    ///   - timeoutMs: Timeout in milliseconds.
    /// - Returns: Frame status (ready, timeout, or cancelled).
    public func waitForFrame(_ frameHandle: DTermFrameHandle, timeoutMs: UInt64) -> DTermFrameStatus {
        guard let handle = handle else { return .cancelled }
        let status = dterm_renderer_wait_frame(handle, frameHandle.ffiHandle, timeoutMs)
        return DTermFrameStatus(rawValue: status.rawValue) ?? .cancelled
    }
}

// MARK: - DTermGpuRenderError

/// Render error codes from dterm-core GPU renderer.
///
/// Maps to `DtermRenderError` from the FFI.
public enum DTermGpuRenderError: UInt32, Error {
    /// Success
    case ok = 0
    /// Null pointer argument
    case nullPointer = 1
    /// Invalid device handle
    case invalidDevice = 2
    /// Invalid queue handle
    case invalidQueue = 3
    /// Invalid surface view handle
    case invalidSurfaceView = 4
    /// Rendering failed
    case renderFailed = 5
}

// MARK: - DTermDamageRegion

/// Damage region for optimized rendering.
///
/// Represents a rectangular region of the terminal that needs to be redrawn.
public struct DTermDamageRegion {
    /// Starting row (0-indexed)
    public var startRow: UInt16
    /// Ending row (exclusive)
    public var endRow: UInt16
    /// Starting column (0-indexed)
    public var startCol: UInt16
    /// Ending column (exclusive)
    public var endCol: UInt16
    /// Whether this represents full damage
    public var isFull: Bool

    /// Create a damage region for full redraw.
    public static var full: DTermDamageRegion {
        DTermDamageRegion(startRow: 0, endRow: 0, startCol: 0, endCol: 0, isFull: true)
    }

    /// Create a damage region for a specific area.
    public init(startRow: UInt16, endRow: UInt16, startCol: UInt16, endCol: UInt16, isFull: Bool = false) {
        self.startRow = startRow
        self.endRow = endRow
        self.startCol = startCol
        self.endCol = endCol
        self.isFull = isFull
    }

    /// Convert to FFI struct.
    func toFFI() -> DtermDamageRegion {
        return DtermDamageRegion(
            start_row: startRow,
            end_row: endRow,
            start_col: startCol,
            end_col: endCol,
            is_full: isFull
        )
    }
}

// MARK: - DTermGpuRenderer

/// Full GPU renderer for terminal content using wgpu.
///
/// This class wraps the dterm-core GPU renderer with wgpu backend. Unlike
/// `DTermRenderer` which only handles frame synchronization, this class
/// can actually render terminal content to Metal surfaces.
///
/// ## Requirements
///
/// The GPU renderer requires:
/// - wgpu device and queue pointers (from wgpu-native)
/// - A valid surface view to render to
///
/// ## Usage
///
/// Due to wgpu's architecture, the platform (Swift) needs to:
/// 1. Create a wgpu instance
/// 2. Request a device and queue
/// 3. Pass the raw pointers to `DTermGpuRenderer.create()`
/// 4. Call `render()` with terminal and surface view
///
/// ```swift
/// // Platform creates wgpu handles (not shown)
/// let renderer = DTermGpuRenderer.create(device: devicePtr, queue: queuePtr)
///
/// // Render terminal content
/// try renderer?.render(terminal: terminal, surfaceView: surfaceViewPtr)
/// ```
///
/// ## Current Status
///
/// The full GPU renderer is implemented in dterm-core but requires
/// wgpu-native integration on the Swift side. The current workaround
/// uses `DTermRenderer` for frame sync only while the ObjC Metal stack
/// handles actual rendering.
///
/// ## Thread Safety
///
/// - Must be created on the main thread
/// - `render()` must be called from the render thread
/// - Frame sync operations can be called from any thread
public final class DTermGpuRenderer {
    /// Opaque pointer to the Rust GPU renderer
    private var handle: OpaquePointer?

    /// Private initializer - use static factory method.
    private init(handle: OpaquePointer) {
        self.handle = handle
    }

    deinit {
        if let handle = handle {
            dterm_gpu_renderer_free(handle)
        }
    }

    /// Check if the GPU renderer FFI is available.
    ///
    /// This verifies that dterm-core was compiled with the "gpu" feature.
    public static var isAvailable: Bool {
        return dterm_gpu_renderer_available()
    }

    /// Create a new GPU renderer with wgpu device and queue.
    ///
    /// - Parameters:
    ///   - device: Raw pointer to wgpu::Device
    ///   - queue: Raw pointer to wgpu::Queue
    ///   - config: Renderer configuration (optional)
    /// - Returns: A new GPU renderer, or nil if creation failed.
    ///
    /// ## Safety
    ///
    /// The device and queue pointers must remain valid for the lifetime
    /// of the renderer. These are typically obtained from wgpu-native.
    public static func create(
        device: UnsafeRawPointer,
        queue: UnsafeRawPointer,
        config: DTermRendererConfig = .default
    ) -> DTermGpuRenderer? {
        guard isAvailable else { return nil }

        var ffiConfig = config.toFFI()
        let handle = dterm_gpu_renderer_create(device, queue, &ffiConfig)
        guard let handle = handle else { return nil }
        return DTermGpuRenderer(handle: handle)
    }

    /// Create a new GPU renderer with explicit surface format.
    ///
    /// - Parameters:
    ///   - device: Raw pointer to wgpu::Device
    ///   - queue: Raw pointer to wgpu::Queue
    ///   - config: Renderer configuration (optional)
    ///   - surfaceFormat: wgpu TextureFormat value (e.g., 23 for Bgra8UnormSrgb)
    /// - Returns: A new GPU renderer, or nil if creation failed.
    public static func create(
        device: UnsafeRawPointer,
        queue: UnsafeRawPointer,
        config: DTermRendererConfig = .default,
        surfaceFormat: UInt32
    ) -> DTermGpuRenderer? {
        guard isAvailable else { return nil }

        var ffiConfig = config.toFFI()
        let handle = dterm_gpu_renderer_create_with_format(device, queue, &ffiConfig, surfaceFormat)
        guard let handle = handle else { return nil }
        return DTermGpuRenderer(handle: handle)
    }

    /// Render the terminal to the provided surface view.
    ///
    /// This performs a full render of all terminal cells.
    ///
    /// - Parameters:
    ///   - terminal: The dterm-core terminal to render
    ///   - surfaceView: Raw pointer to wgpu::TextureView
    /// - Throws: `DTermGpuRenderError` if rendering fails.
    public func render(terminal: DTermCore, surfaceView: UnsafeRawPointer) throws {
        guard let handle = handle else {
            throw DTermGpuRenderError.nullPointer
        }
        guard let terminalPtr = terminal.terminalPointer else {
            throw DTermGpuRenderError.nullPointer
        }

        let result = dterm_gpu_renderer_render(handle, terminalPtr, surfaceView)
        if result != DTERM_RENDER_ERROR_OK {
            throw DTermGpuRenderError(rawValue: result.rawValue) ?? .renderFailed
        }
    }

    /// Render the terminal with damage-based optimization.
    ///
    /// This only renders cells that have changed, significantly reducing
    /// GPU work for small updates.
    ///
    /// - Parameters:
    ///   - terminal: The dterm-core terminal to render
    ///   - surfaceView: Raw pointer to wgpu::TextureView
    ///   - damage: The damage region to render (nil = full render)
    /// - Throws: `DTermGpuRenderError` if rendering fails.
    public func render(
        terminal: DTermCore,
        surfaceView: UnsafeRawPointer,
        damage: DTermDamageRegion?
    ) throws {
        guard let handle = handle else {
            throw DTermGpuRenderError.nullPointer
        }
        guard let terminalPtr = terminal.terminalPointer else {
            throw DTermGpuRenderError.nullPointer
        }

        let result: DtermRenderError
        if let damage = damage {
            var ffiDamage = damage.toFFI()
            result = withUnsafePointer(to: &ffiDamage) { damagePtr in
                // Note: The FFI expects a Damage pointer from dterm-core, not DtermDamageRegion
                // For now, pass nil for partial damage (not yet supported)
                dterm_gpu_renderer_render_with_damage(handle, terminalPtr, surfaceView, nil)
            }
        } else {
            result = dterm_gpu_renderer_render_with_damage(handle, terminalPtr, surfaceView, nil)
        }

        if result != DTERM_RENDER_ERROR_OK {
            throw DTermGpuRenderError(rawValue: result.rawValue) ?? .renderFailed
        }
    }

    /// Request a new frame from the renderer.
    ///
    /// - Returns: A frame handle, or nil if the renderer is invalid.
    public func requestFrame() -> DTermFrameHandle? {
        guard let handle = handle else { return nil }
        let ffiHandle = dterm_gpu_renderer_request_frame(handle)
        if ffiHandle.id == UInt64.max {
            return nil
        }
        return DTermFrameHandle(ffiHandle: ffiHandle)
    }

    /// Wait for a frame to be ready.
    ///
    /// - Parameter timeoutMs: Timeout in milliseconds.
    /// - Returns: Frame status (ready, timeout, or cancelled).
    public func waitForFrame(timeoutMs: UInt64) -> DTermFrameStatus {
        guard let handle = handle else { return .cancelled }
        let status = dterm_gpu_renderer_wait_frame(handle, timeoutMs)
        return DTermFrameStatus(rawValue: status.rawValue) ?? .cancelled
    }

    // MARK: - Font Data Bridge

    /// Set the font for the GPU renderer from raw font data (TTF/OTF bytes).
    ///
    /// This creates a glyph atlas from the provided font data and attaches it
    /// to the renderer. The font data is copied internally, so the caller can
    /// release the original buffer after this call returns.
    ///
    /// - Parameters:
    ///   - fontData: Raw TTF/OTF font file data
    ///   - config: Atlas configuration (optional)
    /// - Returns: `true` on success, `false` on failure.
    @discardableResult
    public func setFont(fontData: Data, config: DTermAtlasConfig = .default) -> Bool {
        guard let handle = handle else { return false }

        return fontData.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return false }
            var ffiConfig = config.toFFI()
            return dterm_gpu_renderer_set_font(
                handle,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                UInt(ptr.count),
                &ffiConfig
            )
        }
    }

    /// Set the font for the GPU renderer from an NSFont.
    ///
    /// This extracts the font file data from the NSFont and passes it to the
    /// Rust glyph atlas. Only works for fonts that have a URL (system fonts
    /// and custom fonts loaded from files).
    ///
    /// - Parameters:
    ///   - font: The NSFont to use for rendering
    ///   - config: Atlas configuration (optional, font size is overridden)
    /// - Returns: `true` on success, `false` if font data couldn't be extracted.
    @discardableResult
    public func setFont(_ font: NSFont, config: DTermAtlasConfig? = nil) -> Bool {
        guard let fontData = Self.extractFontData(from: font) else {
            DLog("DTermGpuRenderer: Failed to extract font data from \(font.fontName)")
            return false
        }

        // Use provided config or create default with font's point size
        var atlasConfig = config ?? .default
        atlasConfig.defaultFontSize = UInt16(font.pointSize)

        return setFont(fontData: fontData, config: atlasConfig)
    }

    /// Extract raw font data (TTF/OTF bytes) from an NSFont.
    ///
    /// Delegates to DTermHybridRenderer.extractFontData which includes
    /// fallback to bundled JetBrains Mono font for system fonts.
    ///
    /// - Parameter font: The NSFont to extract data from
    /// - Returns: Font file data, or nil if extraction failed.
    private static func extractFontData(from font: NSFont) -> Data? {
        // Use shared implementation with bundled font fallback
        return DTermHybridRenderer.extractFontData(from: font)
    }

    /// Get cell dimensions from the current font.
    ///
    /// Returns the cell width and height in pixels based on the current font.
    /// These values are needed to properly size the terminal view.
    ///
    /// - Returns: Tuple of (cellWidth, cellHeight), or nil if no font is set.
    public func cellDimensions() -> (width: Float, height: Float)? {
        guard let handle = handle else { return nil }

        var width: Float = 0
        var height: Float = 0
        if dterm_gpu_renderer_get_cell_size(handle, &width, &height) {
            return (width, height)
        }
        return nil
    }

    // MARK: - Font Variants

    /// Set bold, italic, and bold-italic font variants for the GPU renderer.
    ///
    /// These variants are used when rendering text with corresponding attributes.
    /// If a variant is not provided (nil data), the regular font will be used
    /// with synthetic styling.
    ///
    /// - Parameters:
    ///   - boldData: Raw TTF/OTF font file data for bold variant (nil = use synthetic)
    ///   - italicData: Raw TTF/OTF font file data for italic variant (nil = use synthetic)
    ///   - boldItalicData: Raw TTF/OTF font file data for bold-italic variant (nil = use synthetic)
    /// - Returns: `true` on success, `false` on failure.
    @discardableResult
    public func setFontVariants(
        boldData: Data?,
        italicData: Data?,
        boldItalicData: Data?
    ) -> Bool {
        guard let handle = handle else { return false }

        // Helper to get base address and count, or nil pointers for nil data
        func withOptionalData<T>(
            _ data: Data?,
            body: (UnsafePointer<UInt8>?, UInt) -> T
        ) -> T {
            if let data = data {
                return data.withUnsafeBytes { ptr in
                    if let baseAddress = ptr.baseAddress {
                        return body(baseAddress.assumingMemoryBound(to: UInt8.self), UInt(ptr.count))
                    } else {
                        return body(nil, 0)
                    }
                }
            } else {
                return body(nil, 0)
            }
        }

        return withOptionalData(boldData) { boldPtr, boldLen in
            withOptionalData(italicData) { italicPtr, italicLen in
                withOptionalData(boldItalicData) { boldItalicPtr, boldItalicLen in
                    dterm_gpu_renderer_set_font_variants(
                        handle,
                        boldPtr,
                        boldLen,
                        italicPtr,
                        italicLen,
                        boldItalicPtr,
                        boldItalicLen
                    )
                }
            }
        }
    }

    /// Set bold, italic, and bold-italic font variants from NSFonts.
    ///
    /// Convenience method that extracts font data from NSFont objects.
    /// Pass nil for any variant to use synthetic styling for that style.
    ///
    /// - Parameters:
    ///   - boldFont: NSFont for bold variant (nil = use synthetic)
    ///   - italicFont: NSFont for italic variant (nil = use synthetic)
    ///   - boldItalicFont: NSFont for bold-italic variant (nil = use synthetic)
    /// - Returns: `true` on success, `false` on failure.
    @discardableResult
    public func setFontVariants(
        boldFont: NSFont?,
        italicFont: NSFont?,
        boldItalicFont: NSFont?
    ) -> Bool {
        let boldData = boldFont.flatMap { Self.extractFontData(from: $0) }
        let italicData = italicFont.flatMap { Self.extractFontData(from: $0) }
        let boldItalicData = boldItalicFont.flatMap { Self.extractFontData(from: $0) }

        return setFontVariants(
            boldData: boldData,
            italicData: italicData,
            boldItalicData: boldItalicData
        )
    }
}

// MARK: - DTermAtlasConfig

/// Configuration for glyph atlas.
public struct DTermAtlasConfig {
    /// Initial atlas size (width = height, must be power of 2)
    public var initialSize: UInt32

    /// Maximum atlas size (width = height)
    public var maxSize: UInt32

    /// Default font size in pixels
    public var defaultFontSize: UInt16

    /// Padding between glyphs in pixels
    public var padding: UInt32

    /// Default configuration (512px initial, 4096px max, 14px font)
    public static var `default`: DTermAtlasConfig {
        DTermAtlasConfig(
            initialSize: 512,
            maxSize: 4096,
            defaultFontSize: 14,
            padding: 1
        )
    }

    /// High-resolution configuration (1024px initial, 8192px max)
    public static var highRes: DTermAtlasConfig {
        DTermAtlasConfig(
            initialSize: 1024,
            maxSize: 8192,
            defaultFontSize: 14,
            padding: 2
        )
    }

    public init(initialSize: UInt32 = 512, maxSize: UInt32 = 4096, defaultFontSize: UInt16 = 14, padding: UInt32 = 1) {
        self.initialSize = initialSize
        self.maxSize = maxSize
        self.defaultFontSize = defaultFontSize
        self.padding = padding
    }

    /// Convert to FFI struct.
    func toFFI() -> DtermAtlasConfig {
        return DtermAtlasConfig(
            initial_size: initialSize,
            max_size: maxSize,
            default_font_size: defaultFontSize,
            padding: padding
        )
    }
}

// MARK: - DTermCore Extension for GPU Rendering

extension DTermCore {
    /// Get the underlying terminal pointer for GPU rendering.
    ///
    /// This is needed to pass the terminal to `DTermGpuRenderer.render()`.
    /// The pointer is only valid while the `DTermCore` instance is alive.
    var terminalPointer: OpaquePointer? {
        return terminal
    }
}

// MARK: - DTermHybridRenderer

/// Hybrid renderer that generates vertex data for Swift/Metal to render.
///
/// Unlike `DTermGpuRenderer` which does full GPU rendering with wgpu,
/// `DTermHybridRenderer` only generates the vertex and glyph data that
/// Swift can use with its own Metal rendering pipeline.
///
/// ## Architecture
///
/// ```
/// ┌─────────────────────────────────────────────────────────────┐
/// │  Swift (DTermMetalView)                                     │
/// │  - Owns CAMetalLayer                                        │
/// │  - Creates MTLBuffer from vertex data                       │
/// │  - Creates MTLTexture from atlas data                       │
/// │  - Executes Metal draw calls                                │
/// └───────────────────────────────┬─────────────────────────────┘
///                                 │ FFI
///                                 ▼
/// ┌─────────────────────────────────────────────────────────────┐
/// │  dterm-core (Rust)                                          │
/// │  - Generates CellVertex data from Terminal state            │
/// │  - Manages GlyphAtlas for glyph rendering                   │
/// │  - Provides raw bytes for vertices and atlas                │
/// └─────────────────────────────────────────────────────────────┘
/// ```
///
/// ## Usage
///
/// ```swift
/// // Create renderer
/// let renderer = DTermHybridRenderer()
///
/// // Set font
/// let fontData = try Data(contentsOf: fontURL)
/// renderer.setFont(fontData: fontData)
///
/// // Build vertices for terminal
/// let vertexCount = renderer.build(terminal: terminal)
///
/// // Get vertex data for Metal
/// if let vertices = renderer.vertices {
///     let buffer = device.makeBuffer(bytes: vertices.pointer,
///                                    length: vertices.count * MemoryLayout<DTermCellVertex>.stride)
/// }
///
/// // Upload pending glyphs to atlas texture
/// for glyph in renderer.pendingGlyphs {
///     texture.replace(region: glyph.region, with: glyph.data)
/// }
/// renderer.clearPendingGlyphs()
/// ```
public final class DTermHybridRenderer {
    /// Opaque pointer to the Rust hybrid renderer
    private var handle: OpaquePointer?

    /// Check if the hybrid renderer FFI is available.
    public static var isAvailable: Bool {
        return dterm_hybrid_renderer_available()
    }

    /// Create a new hybrid renderer with default configuration.
    public init?() {
        guard Self.isAvailable else { return nil }
        let handle = dterm_hybrid_renderer_create(nil)
        guard let handle = handle else { return nil }
        self.handle = handle
    }

    /// Create a new hybrid renderer with custom configuration.
    public init?(config: DTermRendererConfig) {
        guard Self.isAvailable else { return nil }
        var ffiConfig = config.toFFI()
        let handle = dterm_hybrid_renderer_create(&ffiConfig)
        guard let handle = handle else { return nil }
        self.handle = handle
    }

    deinit {
        if let handle = handle {
            dterm_hybrid_renderer_free(handle)
        }
    }

    // MARK: - Font Management

    /// Set the font from raw font data (TTF/OTF bytes).
    ///
    /// - Parameters:
    ///   - fontData: Raw font file data
    ///   - config: Optional atlas configuration
    /// - Returns: `true` on success, `false` on failure
    @discardableResult
    public func setFont(fontData: Data, config: DTermAtlasConfig? = nil) -> Bool {
        guard let handle = handle else { return false }

        return fontData.withUnsafeBytes { buffer in
            guard let ptr = buffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return false
            }

            if let config = config {
                var ffiConfig = config.toFFI()
                return dterm_hybrid_renderer_set_font(handle, ptr, UInt(fontData.count), &ffiConfig)
            } else {
                return dterm_hybrid_renderer_set_font(handle, ptr, UInt(fontData.count), nil)
            }
        }
    }

    /// Set the font from an NSFont.
    ///
    /// Extracts the raw TTF/OTF data from the font and creates an atlas.
    ///
    /// - Parameters:
    ///   - font: NSFont to use for rendering
    ///   - config: Optional atlas configuration (font size is overridden)
    /// - Returns: `true` on success, `false` if font data couldn't be extracted
    @discardableResult
    public func setFont(_ font: NSFont, config: DTermAtlasConfig? = nil) -> Bool {
        guard let fontData = Self.extractFontData(from: font) else {
            DLog("DTermHybridRenderer: Failed to extract font data from \(font.fontName)")
            return false
        }

        // Use provided config or create default with font's point size
        var atlasConfig = config ?? .default
        atlasConfig.defaultFontSize = UInt16(font.pointSize)

        return setFont(fontData: fontData, config: atlasConfig)
    }

    /// Extract raw font data from an NSFont.
    ///
    /// If the font doesn't have a file URL (common for system fonts like Monaco),
    /// this falls back to the bundled JetBrains Mono font.
    ///
    /// - Parameter font: Font to extract data from
    /// - Returns: Raw TTF/OTF bytes, or nil if extraction failed
    public static func extractFontData(from font: NSFont) -> Data? {
        let ctFont = font as CTFont

        // Try to get the font URL from CTFont descriptor
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        if let fontURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL {
            // Read the font file data
            do {
                let data = try Data(contentsOf: fontURL)
                DLog("DTermHybridRenderer: Extracted \(data.count) bytes from \(fontURL.lastPathComponent)")
                return data
            } catch {
                DLog("DTermHybridRenderer: Failed to read font data from \(fontURL): \(error)")
                // Fall through to bundled font
            }
        } else {
            DLog("DTermHybridRenderer: No font URL available for \(font.fontName), using bundled font")
        }

        // Fall back to bundled JetBrains Mono font
        // System fonts like Monaco don't have accessible file URLs
        return loadBundledFontData(bold: font.fontDescriptor.symbolicTraits.contains(.bold))
    }

    /// Load the bundled JetBrains Mono font data.
    ///
    /// - Parameter bold: Whether to load the bold variant
    /// - Returns: Font data, or nil if bundled font not found
    private static func loadBundledFontData(bold: Bool) -> Data? {
        let fontName = bold ? "JetBrainsMono-Bold" : "JetBrainsMono-Regular"

        guard let fontURL = Bundle.main.url(forResource: fontName, withExtension: "ttf", subdirectory: "Fonts") else {
            // Try without subdirectory (in case it's at root of Resources)
            guard let fontURL = Bundle.main.url(forResource: fontName, withExtension: "ttf") else {
                DLog("DTermHybridRenderer: Bundled font \(fontName).ttf not found")
                NSLog("[GPU] Bundled font \(fontName).ttf not found in app bundle")
                return nil
            }
            return loadFontData(from: fontURL, name: fontName)
        }

        return loadFontData(from: fontURL, name: fontName)
    }

    /// Load font data from a URL.
    private static func loadFontData(from url: URL, name: String) -> Data? {
        do {
            let data = try Data(contentsOf: url)
            DLog("DTermHybridRenderer: Loaded bundled font \(name) (\(data.count) bytes)")
            NSLog("[GPU] Using bundled font \(name) for GPU renderer")
            return data
        } catch {
            DLog("DTermHybridRenderer: Failed to load bundled font \(name): \(error)")
            NSLog("[GPU] Failed to load bundled font \(name): \(error)")
            return nil
        }
    }

    /// Get cell dimensions from the current font.
    ///
    /// - Returns: Tuple of (cellWidth, cellHeight), or nil if no font is set
    public func cellDimensions() -> (width: Float, height: Float)? {
        guard let handle = handle else { return nil }

        var width: Float = 0
        var height: Float = 0
        if dterm_hybrid_renderer_get_cell_size(handle, &width, &height) {
            return (width, height)
        }
        return nil
    }

    // MARK: - Building Vertex Data

    /// Build vertex data for the terminal.
    ///
    /// This generates all vertex data needed to render the terminal.
    /// After calling this, access the vertices via `vertices` property.
    ///
    /// - Parameter terminal: The terminal to render
    /// - Returns: Number of vertices generated, or 0 on failure
    @discardableResult
    public func build(terminal: DTermCore) -> UInt32 {
        guard let handle = handle, let terminalPtr = terminal.terminalPointer else {
            return 0
        }

        return dterm_hybrid_renderer_build(handle, terminalPtr)
    }

    // MARK: - Accessing Vertex Data

    /// Vertex data from the last build.
    ///
    /// Returns a pointer and count that can be used to create a Metal buffer.
    /// The pointer is valid until the next `build()` call.
    public var vertices: (pointer: UnsafeRawPointer, count: Int)? {
        guard let handle = handle else { return nil }

        var count: UInt32 = 0
        guard let ptr = dterm_hybrid_renderer_get_vertices(handle, &count) else {
            return nil
        }

        if count == 0 { return nil }

        return (UnsafeRawPointer(ptr), Int(count))
    }

    /// Background vertex data from the last build (solid color quads).
    ///
    /// Returns a pointer and count for background-only vertices.
    /// These should be rendered first with no blending.
    /// The pointer is valid until the next `build()` call.
    public var backgroundVertices: (pointer: UnsafeRawPointer, count: Int)? {
        guard let handle = handle else { return nil }

        var count: UInt32 = 0
        guard let ptr = dterm_hybrid_renderer_get_background_vertices(handle, &count) else {
            return nil
        }

        if count == 0 { return nil }

        return (UnsafeRawPointer(ptr), Int(count))
    }

    /// Glyph vertex data from the last build (textured quads from atlas).
    ///
    /// Returns a pointer and count for glyph-only vertices.
    /// These should be rendered second with alpha blending and atlas texture.
    /// The pointer is valid until the next `build()` call.
    public var glyphVertices: (pointer: UnsafeRawPointer, count: Int)? {
        guard let handle = handle else { return nil }

        var count: UInt32 = 0
        guard let ptr = dterm_hybrid_renderer_get_glyph_vertices(handle, &count) else {
            return nil
        }

        if count == 0 { return nil }

        return (UnsafeRawPointer(ptr), Int(count))
    }

    /// Decoration vertex data from the last build (underlines, strikethrough, box drawing).
    ///
    /// Returns a pointer and count for decoration-only vertices.
    /// These should be rendered last with alpha blending on top of glyphs.
    /// The pointer is valid until the next `build()` call.
    public var decorationVertices: (pointer: UnsafeRawPointer, count: Int)? {
        guard let handle = handle else { return nil }

        var count: UInt32 = 0
        guard let ptr = dterm_hybrid_renderer_get_decoration_vertices(handle, &count) else {
            return nil
        }

        if count == 0 { return nil }

        return (UnsafeRawPointer(ptr), Int(count))
    }

    /// Uniforms data from the last build.
    ///
    /// Returns a pointer to the uniform struct (64 bytes).
    /// The pointer is valid until the next `build()` call.
    public var uniforms: UnsafeRawPointer? {
        guard let handle = handle else { return nil }
        guard let ptr = dterm_hybrid_renderer_get_uniforms(handle) else {
            return nil
        }
        return UnsafeRawPointer(ptr)
    }

    // MARK: - Atlas Management

    /// Current atlas size in pixels.
    public var atlasSize: UInt32 {
        guard let handle = handle else { return 0 }
        return dterm_hybrid_renderer_get_atlas_size(handle)
    }

    // MARK: - Pending Glyph Management

    /// Number of pending glyph uploads.
    public var pendingGlyphCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_hybrid_renderer_pending_glyph_count(handle))
    }

    /// Get a pending glyph by index.
    ///
    /// - Parameter index: Glyph index (0 to pendingGlyphCount - 1)
    /// - Returns: Glyph entry and bitmap data, or nil if index is out of bounds
    public func pendingGlyph(at index: Int) -> DTermPendingGlyph? {
        guard let handle = handle else { return nil }

        var entry = DtermGlyphEntry(x: 0, y: 0, width: 0, height: 0, bearing_x: 0, bearing_y: 0, advance: 0, _padding: 0)
        var dataPtr: UnsafePointer<UInt8>?
        var dataLen: Int = 0

        guard dterm_hybrid_renderer_get_pending_glyph(
            handle,
            UInt32(index),
            &entry,
            &dataPtr,
            &dataLen
        ) else {
            return nil
        }

        guard let ptr = dataPtr else { return nil }

        return DTermPendingGlyph(
            x: entry.x,
            y: entry.y,
            width: entry.width,
            height: entry.height,
            data: UnsafeRawPointer(ptr),
            dataLength: dataLen
        )
    }

    /// Get all pending glyphs.
    ///
    /// Call this after `build()` to get any newly rasterized glyphs
    /// that need to be uploaded to the atlas texture.
    public var pendingGlyphs: [DTermPendingGlyph] {
        let count = pendingGlyphCount
        var glyphs: [DTermPendingGlyph] = []
        glyphs.reserveCapacity(count)

        for i in 0..<count {
            if let glyph = pendingGlyph(at: i) {
                glyphs.append(glyph)
            }
        }

        return glyphs
    }

    /// Clear pending glyph data.
    ///
    /// Call this after uploading all pending glyphs to the Metal texture.
    public func clearPendingGlyphs() {
        guard let handle = handle else { return }
        dterm_hybrid_renderer_clear_pending_glyphs(handle)
    }

    // MARK: - Full Atlas Data

    /// Get the full atlas bitmap data.
    ///
    /// Use this when recreating the atlas texture (e.g., after size change).
    /// Returns the complete atlas bitmap that should be uploaded to the GPU.
    ///
    /// - Returns: Full atlas data, or nil if no font is set
    public func atlasData() -> DTermAtlasData? {
        guard let handle = handle else { return nil }

        var dataPtr: UnsafePointer<UInt8>?
        var dataLen: Int = 0
        var width: UInt32 = 0
        var height: UInt32 = 0

        guard dterm_hybrid_renderer_get_atlas_data(
            handle,
            &dataPtr,
            &dataLen,
            &width,
            &height
        ) else {
            return nil
        }

        guard let ptr = dataPtr, dataLen > 0 else {
            return nil
        }

        return DTermAtlasData(
            width: width,
            height: height,
            data: UnsafeRawPointer(ptr),
            dataLength: dataLen
        )
    }

    // MARK: - Platform-Rendered Glyph Support

    /// Enable or disable platform-rendered glyph mode.
    ///
    /// When enabled, the renderer uses platform-provided glyph entries (from Core Text)
    /// instead of the internal fontdue-based atlas. This enables support for macOS
    /// system fonts (Monaco, Menlo, SF Mono) that don't have accessible file URLs.
    ///
    /// - Parameter enable: Whether to enable platform glyph mode
    /// - Returns: `true` on success, `false` if renderer is null
    @discardableResult
    public func enablePlatformGlyphs(_ enable: Bool) -> Bool {
        guard let handle = handle else { return false }
        return dterm_hybrid_renderer_enable_platform_glyphs(handle, enable)
    }

    /// Check if platform glyph mode is enabled.
    public var platformGlyphsEnabled: Bool {
        guard let handle = handle else { return false }
        return dterm_hybrid_renderer_is_platform_glyphs_enabled(handle)
    }

    /// Set cell dimensions for platform-rendered glyphs.
    ///
    /// The platform computes these from Core Text font metrics.
    /// Must be called before `build()` when using platform glyphs.
    ///
    /// - Parameters:
    ///   - width: Cell width in pixels
    ///   - height: Cell height in pixels
    /// - Returns: `true` on success
    @discardableResult
    public func setPlatformCellSize(width: Float, height: Float) -> Bool {
        guard let handle = handle else { return false }
        return dterm_hybrid_renderer_set_platform_cell_size(handle, width, height)
    }

    /// Set atlas size for platform-rendered glyphs.
    ///
    /// The platform manages its own texture atlas. This tells dterm-core the
    /// atlas dimensions for UV coordinate calculation.
    ///
    /// - Parameter size: Atlas size in pixels (square, e.g., 512, 1024, 2048)
    /// - Returns: `true` on success
    @discardableResult
    public func setPlatformAtlasSize(_ size: UInt32) -> Bool {
        guard let handle = handle else { return false }
        return dterm_hybrid_renderer_set_platform_atlas_size(handle, size)
    }

    /// Add a platform-rendered glyph entry.
    ///
    /// The platform renders glyphs using Core Text and adds them to its texture atlas.
    /// This function tells dterm-core where each glyph is located in the atlas.
    ///
    /// - Parameters:
    ///   - codepoint: Unicode codepoint
    ///   - x: X position in atlas (pixels)
    ///   - y: Y position in atlas (pixels)
    ///   - width: Glyph width (pixels)
    ///   - height: Glyph height (pixels)
    ///   - bearingX: Horizontal bearing (pixels)
    ///   - bearingY: Vertical bearing (pixels)
    ///   - advance: Horizontal advance (pixels)
    /// - Returns: `true` on success
    @discardableResult
    public func addPlatformGlyph(
        codepoint: UInt32,
        x: UInt16,
        y: UInt16,
        width: UInt16,
        height: UInt16,
        bearingX: Int16,
        bearingY: Int16,
        advance: UInt16
    ) -> Bool {
        guard let handle = handle else { return false }
        return dterm_hybrid_renderer_add_platform_glyph(
            handle,
            codepoint,
            x,
            y,
            width,
            height,
            bearingX,
            bearingY,
            advance
        )
    }

    /// Clear all platform-rendered glyph entries.
    ///
    /// Call this when changing fonts to remove stale glyph entries.
    public func clearPlatformGlyphs() {
        guard let handle = handle else { return }
        dterm_hybrid_renderer_clear_platform_glyphs(handle)
    }

    /// Number of platform glyph entries.
    public var platformGlyphCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_hybrid_renderer_platform_glyph_count(handle))
    }

    /// Synchronize glyph entries from a DTermGlyphAtlasManager.
    ///
    /// This is a convenience method that:
    /// 1. Sets the cell size from the atlas manager
    /// 2. Sets the atlas size
    /// 3. Adds all cached glyphs to the renderer
    /// 4. Enables platform glyph mode
    ///
    /// - Parameter atlasManager: The glyph atlas manager to sync from
    /// - Returns: `true` on success
    @MainActor
    @discardableResult
    public func syncWithAtlasManager(_ atlasManager: DTermGlyphAtlasManager) -> Bool {
        // Clear existing platform glyphs
        clearPlatformGlyphs()

        // Set cell dimensions
        guard setPlatformCellSize(
            width: Float(atlasManager.cellWidth),
            height: Float(atlasManager.cellHeight)
        ) else {
            return false
        }

        // Set atlas size
        guard setPlatformAtlasSize(UInt32(atlasManager.atlasSize)) else {
            return false
        }

        // Add all cached glyphs
        for entry in atlasManager.allGlyphEntries {
            addPlatformGlyph(
                codepoint: entry.codepoint,
                x: entry.x,
                y: entry.y,
                width: entry.width,
                height: entry.height,
                bearingX: entry.bearingX,
                bearingY: entry.bearingY,
                advance: entry.advance
            )
        }

        // Enable platform glyph mode
        return enablePlatformGlyphs(true)
    }
}

// MARK: - Supporting Types

/// A pending glyph to be uploaded to the atlas texture.
public struct DTermPendingGlyph {
    /// X position in atlas (pixels)
    public let x: UInt16

    /// Y position in atlas (pixels)
    public let y: UInt16

    /// Glyph width (pixels)
    public let width: UInt16

    /// Glyph height (pixels)
    public let height: UInt16

    /// Pointer to bitmap data (R8 format)
    public let data: UnsafeRawPointer

    /// Length of bitmap data
    public let dataLength: Int

    /// Region in atlas texture for MTLTexture.replace(region:...)
    public var region: (x: Int, y: Int, width: Int, height: Int) {
        return (Int(x), Int(y), Int(width), Int(height))
    }
}

/// Full atlas bitmap data.
///
/// Returned by `DTermHybridRenderer.atlasData()` when the atlas needs
/// full re-upload (e.g., after size change).
public struct DTermAtlasData {
    /// Atlas width in pixels
    public let width: UInt32

    /// Atlas height in pixels
    public let height: UInt32

    /// Pointer to bitmap data (R8 format)
    public let data: UnsafeRawPointer

    /// Length of bitmap data
    public let dataLength: Int
}

/// Swift wrapper for DtermCellVertex (64 bytes).
///
/// This struct matches the layout of the Rust CellVertex.
public struct DTermCellVertex {
    /// Position in cell grid coordinates
    public var position: (Float, Float)

    /// UV coordinates in atlas (normalized 0-1)
    public var uv: (Float, Float)

    /// Foreground color (RGBA, 0-1)
    public var fgColor: (Float, Float, Float, Float)

    /// Background color (RGBA, 0-1)
    public var bgColor: (Float, Float, Float, Float)

    /// Style flags
    public var flags: UInt32

    // Padding (internal)
    private var _padding: (UInt32, UInt32, UInt32)
}

/// Swift wrapper for DtermUniforms (64 bytes).
///
/// This struct matches the layout of the Rust Uniforms.
public struct DTermUniforms {
    /// Viewport width in pixels
    public var viewportWidth: Float

    /// Viewport height in pixels
    public var viewportHeight: Float

    /// Cell width in pixels
    public var cellWidth: Float

    /// Cell height in pixels
    public var cellHeight: Float

    /// Atlas texture size in pixels
    public var atlasSize: Float

    /// Time for animations (seconds)
    public var time: Float

    /// Cursor X position (cell coordinates, -1 if hidden)
    public var cursorX: Int32

    /// Cursor Y position (cell coordinates, -1 if hidden)
    public var cursorY: Int32

    /// Cursor color (RGBA)
    public var cursorColor: (Float, Float, Float, Float)

    // Padding (internal)
    private var _padding: (Float, Float, Float, Float)
}

// MARK: - Vertex Flag Constants

/// Style flag constants for vertex data.
public struct DTermVertexFlags {
    /// Bold text
    public static let bold: UInt32 = 1

    /// Dim text
    public static let dim: UInt32 = 2

    /// Underlined text
    public static let underline: UInt32 = 4

    /// Blinking text
    public static let blink: UInt32 = 8

    /// Inverse video (swap fg/bg)
    public static let inverse: UInt32 = 16

    /// Strikethrough text
    public static let strikethrough: UInt32 = 32

    /// Cell is under cursor
    public static let isCursor: UInt32 = 64

    /// Cell is selected
    public static let isSelection: UInt32 = 128

    /// Vertex is for background quad
    public static let isBackground: UInt32 = 256
}

// MARK: - Sixel Image

/// A Sixel image with RGBA pixel data.
///
/// Sixel is a graphics protocol that encodes images as a sequence of six vertical pixels
/// per character position. This struct holds the decoded RGBA pixel data.
///
/// Memory is automatically freed when this struct is deallocated.
public final class DTermSixelImage {
    /// Image width in pixels
    public let width: UInt32

    /// Image height in pixels
    public let height: UInt32

    /// RGBA pixel data (4 bytes per pixel)
    private let pixels: UnsafeMutablePointer<UInt32>

    /// Whether we own the pixels memory (and should free it)
    private let ownsMemory: Bool

    internal init(width: UInt32, height: UInt32, pixels: UnsafeMutablePointer<UInt32>) {
        self.width = width
        self.height = height
        self.pixels = pixels
        self.ownsMemory = true
    }

    deinit {
        if ownsMemory {
            dterm_sixel_image_free(pixels)
        }
    }

    /// Total pixel count (width * height)
    public var pixelCount: Int {
        Int(width) * Int(height)
    }

    /// Bytes per row (width * 4)
    public var bytesPerRow: Int {
        Int(width) * 4
    }

    /// Access the raw RGBA pixel data.
    ///
    /// Each pixel is 4 bytes: R, G, B, A (in UInt32 format).
    /// The data is row-major, top-to-bottom, left-to-right.
    public func withPixelData<T>(_ body: (UnsafePointer<UInt32>) throws -> T) rethrows -> T {
        try body(UnsafePointer(pixels))
    }

    /// Create a CGImage from this Sixel image.
    ///
    /// - Returns: CGImage, or nil if creation failed
    public func toCGImage() -> CGImage? {
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
            .union(.byteOrder32Little)

        guard let context = CGContext(
            data: UnsafeMutableRawPointer(pixels),
            width: Int(width),
            height: Int(height),
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: bitmapInfo.rawValue
        ) else {
            return nil
        }

        return context.makeImage()
    }

    /// Create an NSImage from this Sixel image.
    ///
    /// - Returns: NSImage, or nil if creation failed
    public func toNSImage() -> NSImage? {
        guard let cgImage = toCGImage() else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: Int(width), height: Int(height)))
    }
}

// MARK: - Kitty Graphics Types

/// Location type for Kitty image placements.
public enum DTermKittyPlacementLocation {
    /// Absolute position in the terminal grid
    case absolute(row: UInt32, col: UInt32)

    /// Virtual placement (not tied to specific cells)
    case virtual

    /// Relative to another placement
    case relative(parentImageID: UInt32, parentPlacementID: UInt32, offsetX: Int32, offsetY: Int32)
}

/// Info about a Kitty graphics image.
public struct DTermKittyImageInfo {
    /// Image ID
    public let id: UInt32

    /// Image number (0 if not assigned)
    public let number: UInt32

    /// Width in pixels
    public let width: UInt32

    /// Height in pixels
    public let height: UInt32

    /// Number of placements for this image
    public let placementCount: UInt32
}

/// Pixel data for a Kitty graphics image.
///
/// Memory is automatically freed when this struct is deallocated.
public final class DTermKittyImagePixels {
    /// RGBA pixel data (4 bytes per pixel)
    private let pixels: UnsafeMutablePointer<UInt8>

    /// Total byte count
    public let count: Int

    internal init(pixels: UnsafeMutablePointer<UInt8>, count: Int) {
        self.pixels = pixels
        self.count = count
    }

    deinit {
        dterm_kitty_image_free(pixels)
    }

    /// Access the raw RGBA pixel data.
    ///
    /// The data is in RGBA format, 4 bytes per pixel.
    public func withPixelData<T>(_ body: (UnsafePointer<UInt8>) throws -> T) rethrows -> T {
        try body(UnsafePointer(pixels))
    }

    /// Create a CGImage from this pixel data.
    ///
    /// - Parameters:
    ///   - width: Image width in pixels
    ///   - height: Image height in pixels
    /// - Returns: CGImage, or nil if creation failed
    public func toCGImage(width: Int, height: Int) -> CGImage? {
        guard count >= width * height * 4 else { return nil }

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
            .union(.byteOrder32Big)

        guard let context = CGContext(
            data: UnsafeMutableRawPointer(pixels),
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: colorSpace,
            bitmapInfo: bitmapInfo.rawValue
        ) else {
            return nil
        }

        return context.makeImage()
    }
}

/// A placement of a Kitty graphics image.
public struct DTermKittyPlacement {
    /// Placement ID
    public let id: UInt32

    /// Location type and coordinates
    public let location: DTermKittyPlacementLocation

    /// Source rectangle x offset in pixels
    public let sourceX: UInt32

    /// Source rectangle y offset in pixels
    public let sourceY: UInt32

    /// Source rectangle width (0 = full image width)
    public let sourceWidth: UInt32

    /// Source rectangle height (0 = full image height)
    public let sourceHeight: UInt32

    /// Pixel offset within starting cell, x
    public let cellXOffset: UInt32

    /// Pixel offset within starting cell, y
    public let cellYOffset: UInt32

    /// Number of columns to display (0 = auto)
    public let numColumns: UInt32

    /// Number of rows to display (0 = auto)
    public let numRows: UInt32

    /// Z-index for stacking (negative = below text)
    public let zIndex: Int32

    /// Whether this is a virtual placement
    public let isVirtual: Bool

    internal init(from cPlacement: DtermKittyPlacement) {
        self.id = cPlacement.id

        switch cPlacement.location_type {
        case DTERM_KITTY_PLACEMENT_LOCATION_ABSOLUTE:
            self.location = .absolute(
                row: cPlacement.row_or_parent_image,
                col: cPlacement.col_or_parent_placement
            )
        case DTERM_KITTY_PLACEMENT_LOCATION_VIRTUAL:
            self.location = .virtual
        case DTERM_KITTY_PLACEMENT_LOCATION_RELATIVE:
            self.location = .relative(
                parentImageID: cPlacement.row_or_parent_image,
                parentPlacementID: cPlacement.col_or_parent_placement,
                offsetX: cPlacement.offset_x,
                offsetY: cPlacement.offset_y
            )
        default:
            self.location = .absolute(
                row: cPlacement.row_or_parent_image,
                col: cPlacement.col_or_parent_placement
            )
        }

        self.sourceX = cPlacement.source_x
        self.sourceY = cPlacement.source_y
        self.sourceWidth = cPlacement.source_width
        self.sourceHeight = cPlacement.source_height
        self.cellXOffset = cPlacement.cell_x_offset
        self.cellYOffset = cPlacement.cell_y_offset
        self.numColumns = cPlacement.num_columns
        self.numRows = cPlacement.num_rows
        self.zIndex = cPlacement.z_index
        self.isVirtual = cPlacement.is_virtual
    }
}

// MARK: - DTermCheckpoint

/// Checkpoint manager for crash recovery and session persistence.
///
/// `DTermCheckpoint` manages automatic persistence of terminal state to disk.
/// It tracks when saves should occur based on time and line count thresholds,
/// and provides restore functionality for crash recovery.
///
/// ## Usage
///
/// ```swift
/// // Create checkpoint manager for session
/// guard let checkpoint = DTermCheckpoint(path: "~/.local/share/dashterm/sessions/abc123") else {
///     return
/// }
///
/// // Check if we should save based on thresholds
/// if checkpoint.shouldSave {
///     checkpoint.save(terminal: terminal)
/// }
///
/// // Notify about new lines for threshold tracking
/// checkpoint.notifyLines(count: 100)
///
/// // Restore from checkpoint on startup
/// if checkpoint.exists {
///     if let terminal = checkpoint.restore() {
///         // Use restored terminal
///     }
/// }
/// ```
public final class DTermCheckpoint {
    /// Opaque pointer to the Rust checkpoint manager
    private var handle: OpaquePointer?

    /// Create a new checkpoint manager for the given directory path.
    ///
    /// The checkpoint manager will create checkpoint files in this directory.
    /// The directory should be unique per session to avoid conflicts.
    ///
    /// - Parameter path: Directory path for checkpoint files (will be created if needed)
    public init?(path: String) {
        let pathData = path.utf8CString
        guard let handle = pathData.withUnsafeBufferPointer({ buffer -> OpaquePointer? in
            guard let baseAddress = buffer.baseAddress else { return nil }
            let len = buffer.count - 1 // Exclude null terminator
            return baseAddress.withMemoryRebound(to: UInt8.self, capacity: len) { ptr in
                dterm_checkpoint_new(ptr, UInt(len))
            }
        }) else {
            return nil
        }
        self.handle = handle
    }

    deinit {
        if let handle = handle {
            dterm_checkpoint_free(handle)
        }
    }

    // MARK: - Threshold Checks

    /// Check if a checkpoint should be performed based on time and line thresholds.
    ///
    /// This returns `true` when enough time has passed since the last save OR
    /// enough new lines have been added since the last save.
    ///
    /// Default thresholds:
    /// - Time: 60 seconds
    /// - Lines: 1000 lines
    public var shouldSave: Bool {
        guard let handle = handle else { return false }
        return dterm_checkpoint_should_save(handle)
    }

    /// Notify the checkpoint manager that lines were added.
    ///
    /// Call this when new output is received to track against the line threshold.
    ///
    /// - Parameter count: Number of lines added
    public func notifyLines(count: Int) {
        guard let handle = handle else { return }
        dterm_checkpoint_notify_lines(handle, UInt(count))
    }

    // MARK: - Persistence

    /// Check if a valid checkpoint exists at the configured path.
    public var exists: Bool {
        guard let handle = handle else { return false }
        return dterm_checkpoint_exists(handle)
    }

    /// Save a checkpoint of the terminal state.
    ///
    /// This serializes the terminal's visible grid, scrollback, cursor position,
    /// modes, and other state to disk. The checkpoint can later be restored
    /// with `restore()`.
    ///
    /// - Parameter terminal: Terminal to save
    /// - Returns: `true` on success, `false` on failure
    @discardableResult
    public func save(terminal: DTermCore) -> Bool {
        guard let handle = handle, let termPtr = terminal.terminalPointer else {
            return false
        }
        return dterm_checkpoint_save(handle, termPtr)
    }

    /// Restore a terminal from the checkpoint.
    ///
    /// Creates a new terminal initialized with the saved state from the last
    /// successful checkpoint. Returns `nil` if no checkpoint exists or if
    /// restoration fails (e.g., corrupted checkpoint).
    ///
    /// - Returns: Restored terminal, or `nil` on failure
    public func restore() -> DTermCore? {
        guard let handle = handle else { return nil }
        guard let termPtr = dterm_checkpoint_restore(handle) else { return nil }
        return DTermCore(restoredHandle: termPtr)
    }
}

// MARK: - Shell Event Types

/// Shell integration event types (OSC 133, OSC 7).
public enum DTermShellEventType: Int {
    /// Prompt started (OSC 133 ; A).
    case promptStart = 0
    /// Command input started (OSC 133 ; B).
    case commandStart = 1
    /// Command execution/output started (OSC 133 ; C).
    case outputStart = 2
    /// Command finished with exit code (OSC 133 ; D).
    case commandFinished = 3
    /// Working directory changed (OSC 7).
    case directoryChanged = 4
}

/// Shell integration event from dterm-core.
public struct DTermShellEvent {
    /// Type of shell event.
    public let eventType: DTermShellEventType
    /// Row where event occurred (for position events).
    public let row: UInt32
    /// Column where event occurred (for position events).
    public let col: UInt16
    /// Exit code for CommandFinished events (-1 if unknown).
    public let exitCode: Int32
    /// Path or URL for DirectoryChanged events.
    public let path: String?

    init(_ ffi: DtermShellEvent) {
        self.eventType = DTermShellEventType(rawValue: Int(ffi.event_type.rawValue)) ?? .promptStart
        self.row = ffi.row
        self.col = ffi.col
        self.exitCode = ffi.exit_code
        if let pathPtr = ffi.path {
            self.path = String(cString: pathPtr)
        } else {
            self.path = nil
        }
    }
}

// MARK: - Window Operation Types

/// Window operation types (CSI t / XTWINOPS).
public enum DTermWindowOpType: Int {
    /// De-iconify (restore from minimized) window.
    case deIconify = 1
    /// Iconify (minimize) window.
    case iconify = 2
    /// Move window to pixel position.
    case moveWindow = 3
    /// Resize window to pixel dimensions.
    case resizeWindowPixels = 4
    /// Raise window to front.
    case raiseWindow = 5
    /// Lower window to back.
    case lowerWindow = 6
    /// Refresh/redraw window.
    case refreshWindow = 7
    /// Resize text area to cell dimensions.
    case resizeWindowCells = 8
    /// Report window state (iconified or not).
    case reportWindowState = 11
    /// Report window position.
    case reportWindowPosition = 13
    /// Report window size in pixels.
    case reportWindowSizePixels = 14
    /// Report text area size in cells.
    case reportTextAreaCells = 18
    /// Report screen size in cells.
    case reportScreenSizeCells = 19
    /// Report icon label.
    case reportIconLabel = 20
    /// Report window title.
    case reportWindowTitle = 21
    /// Pop title from stack.
    case popTitle = 23
    /// Push title to stack.
    case pushTitle = 22
}

/// Window manipulation operation from dterm-core.
public struct DTermWindowOp {
    /// Operation type.
    public let opType: DTermWindowOpType
    /// First parameter (x, height, or mode depending on operation).
    public let param1: UInt16
    /// Second parameter (y, width, or 0 depending on operation).
    public let param2: UInt16

    init(_ ffi: DtermWindowOp) {
        self.opType = DTermWindowOpType(rawValue: Int(ffi.op_type.rawValue)) ?? .deIconify
        self.param1 = ffi.param1
        self.param2 = ffi.param2
    }
}

/// Response to a window operation query.
public struct DTermWindowResponse {
    /// Whether a response should be sent to the terminal.
    public let hasResponse: Bool
    /// For state reports: the state value.
    public let state: UInt8
    /// For position/size reports: x or width value.
    public let xOrWidth: UInt16
    /// For position/size reports: y or height value.
    public let yOrHeight: UInt16

    public init(hasResponse: Bool = false, state: UInt8 = 0, xOrWidth: UInt16 = 0, yOrHeight: UInt16 = 0) {
        self.hasResponse = hasResponse
        self.state = state
        self.xOrWidth = xOrWidth
        self.yOrHeight = yOrHeight
    }

    /// Create a state response (e.g., for window state query).
    public static func stateResponse(_ state: UInt8) -> DTermWindowResponse {
        return DTermWindowResponse(hasResponse: true, state: state)
    }

    /// Create a position/size response.
    public static func sizeResponse(width: UInt16, height: UInt16) -> DTermWindowResponse {
        return DTermWindowResponse(hasResponse: true, xOrWidth: width, yOrHeight: height)
    }

    func toFFI() -> DtermWindowResponse {
        return DtermWindowResponse(
            has_response: hasResponse,
            state: state,
            x_or_width: xOrWidth,
            y_or_height: yOrHeight
        )
    }
}

// MARK: - Kitty Image Callback Type

/// Kitty graphics image from callback.
public struct DTermKittyImage {
    /// Image ID assigned by the terminal.
    public let id: UInt32
    /// Width in pixels.
    public let width: UInt32
    /// Height in pixels.
    public let height: UInt32
    /// RGBA pixel data (4 bytes per pixel).
    public let rgbaData: Data
}

// MARK: - UI Bridge

/// State of the UI Bridge event processing system.
public enum DTermUIState: UInt32 {
    /// No work in progress, ready to process events.
    case idle = 0
    /// Currently processing an event.
    case processing = 1
    /// Waiting for render completion.
    case rendering = 2
    /// Waiting for callback completion.
    case waitingForCallback = 3
    /// System is shutting down.
    case shuttingDown = 4
}

/// State of a terminal slot in the UI Bridge.
public enum DTermUITerminalState: UInt32 {
    /// Terminal slot is available.
    case inactive = 0
    /// Terminal is active and usable.
    case active = 1
    /// Terminal has been disposed (cannot be reactivated).
    case disposed = 2
}

/// Error codes for UI Bridge operations.
public enum DTermUIErrorCode: UInt32, Error {
    /// Operation succeeded.
    case ok = 0
    /// Event queue is full.
    case queueFull = 1
    /// System is shutting down.
    case shuttingDown = 2
    /// Terminal ID is invalid or out of range.
    case invalidTerminalId = 3
    /// Terminal is not in the expected state.
    case invalidTerminalState = 4
    /// Bridge state is invalid for this operation.
    case invalidBridgeState = 5
    /// Consistency check failed.
    case consistencyError = 6
    /// Unknown error.
    case unknown = 255
}

/// UI Bridge for running terminal on a separate thread with event queuing.
///
/// The UI Bridge provides safe communication between UI and terminal threads:
/// - UI thread enqueues events (input, resize, render requests)
/// - Terminal thread processes events and sends callbacks
/// - All operations are lock-free and safe
///
/// ## Usage Pattern
///
/// ```swift
/// // UI Thread
/// let bridge = DTermUIBridge()
/// bridge.enqueueCreateTerminal(id: 0, rows: 24, cols: 80)
/// bridge.enqueueInput(terminalId: 0, data: "ls\n".data(using: .utf8)!)
///
/// // Terminal Thread
/// while true {
///     if let info = bridge.startProcessing() {
///         switch info.kind {
///         case .createTerminal:
///             // Create terminal
///             bridge.handleCreateTerminal(terminalId: info.terminalId, terminal: terminal)
///         case .input:
///             // Process input
///             bridge.handleInput(terminalId: info.terminalId, data: info.inputData!)
///         // ... other events
///         }
///         bridge.completeProcessing()
///     }
/// }
/// ```
public final class DTermUIBridge {
    private var bridge: OpaquePointer?

    /// Create a new UI Bridge.
    public init?() {
        bridge = dterm_ui_create()
        guard bridge != nil else { return nil }
    }

    deinit {
        if let bridge = bridge {
            dterm_ui_free(bridge)
        }
    }

    // MARK: - State Queries

    /// Get the current state of the bridge.
    public var state: DTermUIState {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_state(bridge).rawValue
        return DTermUIState(rawValue: rawValue) ?? .idle
    }

    /// Get the number of pending events in the queue.
    public var pendingCount: Int {
        guard let bridge = bridge else { return 0 }
        return Int(dterm_ui_pending_count(bridge))
    }

    /// Get the number of pending callbacks.
    public var callbackCount: Int {
        guard let bridge = bridge else { return 0 }
        return Int(dterm_ui_callback_count(bridge))
    }

    /// Get the number of pending render requests.
    public var renderPendingCount: Int {
        guard let bridge = bridge else { return 0 }
        return Int(dterm_ui_render_pending_count(bridge))
    }

    /// Check if the bridge is in a consistent state.
    public var isConsistent: Bool {
        guard let bridge = bridge else { return false }
        return dterm_ui_is_consistent(bridge)
    }

    /// Get the state of a terminal slot.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Terminal state
    public func terminalState(id: UInt32) -> DTermUITerminalState {
        guard let bridge = bridge else { return .inactive }
        let rawValue = dterm_ui_terminal_state(bridge, id).rawValue
        return DTermUITerminalState(rawValue: rawValue) ?? .inactive
    }

    // MARK: - Enqueue Events (UI Thread)

    /// Enqueue input data for a terminal.
    ///
    /// - Parameters:
    ///   - terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    ///   - data: Input data to send
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueInput(terminalId: UInt32, data: Data) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        return data.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return DTermUIErrorCode.unknown }
            let rawValue = dterm_ui_enqueue_input(
                bridge,
                terminalId,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                UInt(ptr.count)
            ).rawValue
            return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
        }
    }

    /// Enqueue a resize event for a terminal.
    ///
    /// - Parameters:
    ///   - terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    ///   - rows: New row count
    ///   - cols: New column count
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueResize(terminalId: UInt32, rows: UInt16, cols: UInt16) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_resize(bridge, terminalId, rows, cols).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Enqueue a render request for a terminal.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueRender(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_render(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Enqueue a create terminal event.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueCreateTerminal(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_create_terminal(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Enqueue a destroy terminal event.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueDestroyTerminal(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_destroy_terminal(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Enqueue a callback event.
    ///
    /// - Parameters:
    ///   - terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    ///   - callbackId: Callback identifier
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueCallback(terminalId: UInt32, callbackId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_callback(bridge, terminalId, callbackId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Enqueue a shutdown event.
    ///
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func enqueueShutdown() -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_enqueue_shutdown(bridge).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    // MARK: - Process Events (Terminal Thread)

    /// Start processing the next event.
    ///
    /// - Parameter info: Receives event info if an event is available
    /// - Returns: Error code (ok if event available, invalidBridgeState if no event)
    @discardableResult
    public func startProcessing(info: inout DtermUIEventInfo) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_start_processing(bridge, &info).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Complete processing of the current event.
    ///
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func completeProcessing() -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_complete_processing(bridge).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Complete a render request.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func completeRender(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_complete_render(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Complete a callback.
    ///
    /// - Parameter callbackId: Callback identifier
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func completeCallback(callbackId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_complete_callback(bridge, callbackId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    // MARK: - Handle Events (Terminal Thread)

    /// Handle a create terminal event by registering a terminal.
    ///
    /// Call this after startProcessing returns a createTerminal event.
    /// The terminal should already be created and ready for use.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func handleCreateTerminal(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_handle_create_terminal(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Handle a destroy terminal event.
    ///
    /// - Parameter terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func handleDestroyTerminal(terminalId: UInt32) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_handle_destroy_terminal(bridge, terminalId).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Handle an input event.
    ///
    /// - Parameters:
    ///   - terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    ///   - data: Input data pointer from event info
    ///   - length: Input data length from event info
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func handleInput(terminalId: UInt32, data: UnsafePointer<UInt8>, length: Int) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_handle_input(bridge, terminalId, data, UInt(length)).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Handle a resize event.
    ///
    /// - Parameters:
    ///   - terminalId: Terminal ID (0 to MAX_TERMINALS-1)
    ///   - rows: New row count
    ///   - cols: New column count
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func handleResize(terminalId: UInt32, rows: UInt16, cols: UInt16) -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_handle_resize(bridge, terminalId, rows, cols).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }

    /// Handle a shutdown event.
    ///
    /// - Returns: Error code (ok on success)
    @discardableResult
    public func handleShutdown() -> DTermUIErrorCode {
        guard let bridge = bridge else { return .shuttingDown }
        let rawValue = dterm_ui_handle_shutdown(bridge).rawValue
        return DTermUIErrorCode(rawValue: rawValue) ?? .unknown
    }
}
