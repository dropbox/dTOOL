#if os(macOS)
/*
 * TerminalInputView.swift - Key and mouse capture view for macOS demo
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import SwiftUI
import AppKit

/// Mouse event data for forwarding to terminal
struct MouseEvent {
    enum Kind {
        case press(button: Int)
        case release(button: Int)
        case motion(button: Int)  // button=3 for no button
        case wheel(up: Bool)
    }
    let kind: Kind
    let col: Int
    let row: Int
    let modifiers: Int  // shift=4, meta=8, ctrl=16
}

struct TerminalInputView: NSViewRepresentable {
    let onKeyData: (Data) -> Void
    let onFocusChange: (Bool) -> Void
    let onMouseEvent: ((MouseEvent) -> Void)?
    let onCopy: (() -> Void)?
    let onPaste: (() -> Void)?
    let onScroll: ((Int) -> Void)?  // delta: positive = up, negative = down
    let charWidth: CGFloat
    let lineHeight: CGFloat

    init(
        onKeyData: @escaping (Data) -> Void,
        onFocusChange: @escaping (Bool) -> Void,
        onMouseEvent: ((MouseEvent) -> Void)? = nil,
        onCopy: (() -> Void)? = nil,
        onPaste: (() -> Void)? = nil,
        onScroll: ((Int) -> Void)? = nil,
        charWidth: CGFloat = 8.4,
        lineHeight: CGFloat = 16.8
    ) {
        self.onKeyData = onKeyData
        self.onFocusChange = onFocusChange
        self.onMouseEvent = onMouseEvent
        self.onCopy = onCopy
        self.onPaste = onPaste
        self.onScroll = onScroll
        self.charWidth = charWidth
        self.lineHeight = lineHeight
    }

    func makeNSView(context: Context) -> KeyCaptureView {
        let view = KeyCaptureView()
        view.onKeyData = onKeyData
        view.onFocusChange = onFocusChange
        view.onMouseEvent = onMouseEvent
        view.onCopy = onCopy
        view.onPaste = onPaste
        view.onScroll = onScroll
        view.charWidth = charWidth
        view.lineHeight = lineHeight
        return view
    }

    func updateNSView(_ nsView: KeyCaptureView, context: Context) {
        nsView.onKeyData = onKeyData
        nsView.onFocusChange = onFocusChange
        nsView.onMouseEvent = onMouseEvent
        nsView.onCopy = onCopy
        nsView.onPaste = onPaste
        nsView.onScroll = onScroll
        nsView.charWidth = charWidth
        nsView.lineHeight = lineHeight
    }
}

final class KeyCaptureView: NSView {
    var onKeyData: ((Data) -> Void)?
    var onFocusChange: ((Bool) -> Void)?
    var onMouseEvent: ((MouseEvent) -> Void)?
    var onCopy: (() -> Void)?
    var onPaste: (() -> Void)?
    var onScroll: ((Int) -> Void)?
    var charWidth: CGFloat = 8.4
    var lineHeight: CGFloat = 16.8

    private var pressedButton: Int = 3  // 3 = no button

    override var acceptsFirstResponder: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    // MARK: - Mouse Events

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        sendMouseEvent(.press(button: 0), event: event)
        pressedButton = 0
    }

    override func mouseUp(with event: NSEvent) {
        sendMouseEvent(.release(button: 0), event: event)
        pressedButton = 3
    }

    override func rightMouseDown(with event: NSEvent) {
        sendMouseEvent(.press(button: 2), event: event)
        pressedButton = 2
    }

    override func rightMouseUp(with event: NSEvent) {
        sendMouseEvent(.release(button: 2), event: event)
        pressedButton = 3
    }

    override func otherMouseDown(with event: NSEvent) {
        sendMouseEvent(.press(button: 1), event: event)
        pressedButton = 1
    }

    override func otherMouseUp(with event: NSEvent) {
        sendMouseEvent(.release(button: 1), event: event)
        pressedButton = 3
    }

    override func mouseDragged(with event: NSEvent) {
        sendMouseEvent(.motion(button: pressedButton), event: event)
    }

    override func rightMouseDragged(with event: NSEvent) {
        sendMouseEvent(.motion(button: pressedButton), event: event)
    }

    override func otherMouseDragged(with event: NSEvent) {
        sendMouseEvent(.motion(button: pressedButton), event: event)
    }

    override func mouseMoved(with event: NSEvent) {
        sendMouseEvent(.motion(button: 3), event: event)
    }

    override func scrollWheel(with event: NSEvent) {
        let deltaY = event.scrollingDeltaY
        if deltaY > 0 {
            sendMouseEvent(.wheel(up: true), event: event)
        } else if deltaY < 0 {
            sendMouseEvent(.wheel(up: false), event: event)
        }
    }

    private func sendMouseEvent(_ kind: MouseEvent.Kind, event: NSEvent) {
        guard let onMouseEvent else { return }

        let point = convert(event.locationInWindow, from: nil)
        let col = max(0, Int(point.x / charWidth))
        // Y is flipped in AppKit - origin at bottom
        let row = max(0, Int((bounds.height - point.y) / lineHeight))

        var modifiers = 0
        if event.modifierFlags.contains(.shift) { modifiers |= 4 }
        if event.modifierFlags.contains(.option) { modifiers |= 8 }
        if event.modifierFlags.contains(.control) { modifiers |= 16 }

        onMouseEvent(MouseEvent(kind: kind, col: col, row: row, modifiers: modifiers))
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        // Remove old tracking areas
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        // Add new tracking area for mouse moved events
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
    }

    override func becomeFirstResponder() -> Bool {
        onFocusChange?(true)
        return true
    }

    override func resignFirstResponder() -> Bool {
        onFocusChange?(false)
        return true
    }

    override func keyDown(with event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

        // Handle Cmd+C (copy) and Cmd+V (paste)
        if flags.contains(.command) {
            let chars = event.charactersIgnoringModifiers ?? ""
            if chars == "c" {
                onCopy?()
                return
            } else if chars == "v" {
                onPaste?()
                return
            }
        }

        // Handle Shift+PageUp/PageDown for scrollback navigation
        if flags.contains(.shift) {
            switch event.keyCode {
            case 116:  // Page Up
                onScroll?(24)  // Scroll up by ~1 page
                return
            case 121:  // Page Down
                onScroll?(-24)  // Scroll down by ~1 page
                return
            case 115:  // Home
                onScroll?(Int.max)  // Scroll to top
                return
            case 119:  // End
                onScroll?(Int.min)  // Scroll to bottom
                return
            default:
                break
            }
        }

        if let data = encodeKey(event) {
            onKeyData?(data)
        } else {
            super.keyDown(with: event)
        }
    }

    private func encodeKey(_ event: NSEvent) -> Data? {
        if let sequence = keyCodeSequence(event.keyCode) {
            return sequence.data(using: .utf8)
        }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let rawChars = event.charactersIgnoringModifiers ?? event.characters ?? ""

        if flags.contains(.control), let controlData = controlSequence(for: rawChars) {
            return controlData
        }

        guard !rawChars.isEmpty else { return nil }
        var output = rawChars
        if flags.contains(.option) {
            output = "\u{1b}" + output
        }
        return output.data(using: .utf8)
    }

    private func controlSequence(for chars: String) -> Data? {
        guard let scalar = chars.uppercased().unicodeScalars.first else { return nil }
        let value = scalar.value
        if value >= 0x40 && value <= 0x5F {
            return Data([UInt8(value - 0x40)])
        }
        return nil
    }

    private func keyCodeSequence(_ keyCode: UInt16) -> String? {
        switch keyCode {
        case 36, 76:
            return "\r"
        case 48:
            return "\t"
        case 51:
            return "\u{7f}"
        case 53:
            return "\u{1b}"
        case 114:
            return "\u{1b}[2~"
        case 115:
            return "\u{1b}[H"
        case 116:
            return "\u{1b}[5~"
        case 117:
            return "\u{1b}[3~"
        case 119:
            return "\u{1b}[F"
        case 121:
            return "\u{1b}[6~"
        case 123:
            return "\u{1b}[D"
        case 124:
            return "\u{1b}[C"
        case 125:
            return "\u{1b}[B"
        case 126:
            return "\u{1b}[A"
        case 122:
            return "\u{1b}OP"
        case 120:
            return "\u{1b}OQ"
        case 99:
            return "\u{1b}OR"
        case 118:
            return "\u{1b}OS"
        case 96:
            return "\u{1b}[15~"
        case 97:
            return "\u{1b}[17~"
        case 98:
            return "\u{1b}[18~"
        case 100:
            return "\u{1b}[19~"
        case 101:
            return "\u{1b}[20~"
        case 109:
            return "\u{1b}[21~"
        case 103:
            return "\u{1b}[23~"
        case 111:
            return "\u{1b}[24~"
        default:
            return nil
        }
    }
}
#endif
