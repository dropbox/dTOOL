/*
 * DTermModes.swift - Terminal mode types for DTermCore
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import Foundation
import CDTermCore

/// Mouse tracking modes.
public enum MouseMode: UInt32 {
    /// No mouse tracking.
    case none = 0

    /// Normal tracking (1000) - report button press/release.
    case normal = 1

    /// Button-event tracking (1002) - report press/release and motion while button pressed.
    case buttonEvent = 2

    /// Any-event tracking (1003) - report all motion events.
    case anyEvent = 3

    internal init(from mode: DtermMouseMode) {
        self = MouseMode(rawValue: mode.rawValue) ?? .none
    }
}

/// Mouse encoding formats.
public enum MouseEncoding: UInt32 {
    /// X10 compatibility mode - coordinates encoded as single bytes (limited to 223).
    case x10 = 0

    /// SGR encoding (1006) - coordinates as decimal parameters, supports larger values.
    case sgr = 1

    internal init(from encoding: DtermMouseEncoding) {
        self = MouseEncoding(rawValue: encoding.rawValue) ?? .x10
    }
}

/// Cursor styles (DECSCUSR).
public enum CursorStyle: UInt8 {
    /// Default (usually blinking block).
    case `default` = 0

    /// Blinking block.
    case blinkingBlock = 1

    /// Steady block.
    case steadyBlock = 2

    /// Blinking underline.
    case blinkingUnderline = 3

    /// Steady underline.
    case steadyUnderline = 4

    /// Blinking bar.
    case blinkingBar = 5

    /// Steady bar.
    case steadyBar = 6

    /// Whether this cursor style blinks.
    public var blinks: Bool {
        switch self {
        case .default, .blinkingBlock, .blinkingUnderline, .blinkingBar:
            return true
        case .steadyBlock, .steadyUnderline, .steadyBar:
            return false
        }
    }
}

/// Terminal mode flags.
public struct DTermModes {
    /// Cursor visible (DECTCEM).
    public let cursorVisible: Bool

    /// Cursor style (DECSCUSR).
    public let cursorStyle: CursorStyle

    /// Application cursor keys (DECCKM).
    public let applicationCursorKeys: Bool

    /// Alternate screen buffer active.
    public let alternateScreen: Bool

    /// Auto-wrap mode (DECAWM).
    public let autoWrap: Bool

    /// Origin mode (DECOM).
    public let originMode: Bool

    /// Insert mode (IRM).
    public let insertMode: Bool

    /// Bracketed paste mode.
    public let bracketedPaste: Bool

    /// Mouse tracking mode.
    public let mouseMode: MouseMode

    /// Mouse encoding format.
    public let mouseEncoding: MouseEncoding

    /// Focus reporting mode (1004).
    public let focusReporting: Bool

    /// Synchronized output mode (2026).
    public let synchronizedOutput: Bool

    /// Reverse video mode (DECSET 5).
    public let reverseVideo: Bool

    /// Cursor blink mode (DECSET 12).
    public let cursorBlink: Bool

    /// Application keypad mode (DECKPAM/DECKPNM).
    public let applicationKeypad: Bool

    /// 132 column mode (DECSET 3).
    public let columnMode132: Bool

    /// Reverse wraparound mode (DECSET 45).
    public let reverseWraparound: Bool

    /// Initialize from C struct.
    internal init(from modes: dterm_modes_t) {
        self.cursorVisible = modes.cursor_visible
        self.cursorStyle = CursorStyle(rawValue: modes.cursor_style) ?? .default
        self.applicationCursorKeys = modes.application_cursor_keys
        self.alternateScreen = modes.alternate_screen
        self.autoWrap = modes.auto_wrap
        self.originMode = modes.origin_mode
        self.insertMode = modes.insert_mode
        self.bracketedPaste = modes.bracketed_paste
        self.mouseMode = MouseMode(from: modes.mouse_mode)
        self.mouseEncoding = MouseEncoding(from: modes.mouse_encoding)
        self.focusReporting = modes.focus_reporting
        self.synchronizedOutput = modes.synchronized_output
        self.reverseVideo = modes.reverse_video
        self.cursorBlink = modes.cursor_blink
        self.applicationKeypad = modes.application_keypad
        self.columnMode132 = modes.column_mode_132
        self.reverseWraparound = modes.reverse_wraparound
    }
}

/// Shell integration state (OSC 133).
public enum ShellState: UInt32 {
    /// Ground state - waiting for prompt.
    case ground = 0

    /// Receiving prompt text (after OSC 133 ; A).
    case receivingPrompt = 1

    /// User is entering command (after OSC 133 ; B).
    case enteringCommand = 2

    /// Command is executing (after OSC 133 ; C).
    case executing = 3

    internal init(from state: DtermShellState) {
        self = ShellState(rawValue: state.rawValue) ?? .ground
    }
}

/// Line size for DEC line attributes (DECDHL/DECDWL).
public enum LineSize: UInt32 {
    /// Normal single-width, single-height line.
    case singleWidth = 0

    /// Double-width line (DECDWL).
    case doubleWidth = 1

    /// Top half of double-height line (DECDHL).
    case doubleHeightTop = 2

    /// Bottom half of double-height line (DECDHL).
    case doubleHeightBottom = 3

    internal init(from size: DtermLineSize) {
        self = LineSize(rawValue: size.rawValue) ?? .singleWidth
    }
}

/// Window manipulation commands (CSI t / XTWINOPS).
///
/// Maps to SwiftTerm's `WindowManipulationCommand`.
public enum WindowCommand {
    /// De-iconify window.
    case deIconify

    /// Iconify window.
    case iconify

    /// Move window to (x, y).
    case moveTo(x: Int, y: Int)

    /// Resize window to (width, height) in pixels.
    case resizePixels(width: Int, height: Int)

    /// Raise window.
    case raise

    /// Lower window.
    case lower

    /// Refresh window.
    case refresh

    /// Resize window to (cols, rows) in characters.
    case resizeChars(cols: Int, rows: Int)

    /// Maximize or restore window.
    case maximize(horizontal: Bool, vertical: Bool)

    /// Fullscreen or restore.
    case fullscreen(on: Bool)

    /// Report window state (iconified/normal).
    case reportState

    /// Report window position.
    case reportPosition

    /// Report text area size in pixels.
    case reportSizePixels

    /// Report window size in characters.
    case reportSizeChars

    /// Report text area size in characters.
    case reportTextAreaChars

    /// Report screen size in characters.
    case reportScreenSizeChars

    /// Report icon label.
    case reportIconLabel

    /// Report window title.
    case reportTitle

    /// Push icon and window title to stack.
    case pushTitle(icon: Bool, window: Bool)

    /// Pop icon and window title from stack.
    case popTitle(icon: Bool, window: Bool)
}
