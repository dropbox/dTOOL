/*
 * TerminalView.swift - SwiftUI terminal display component
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 *
 * This view renders the terminal content using SwiftUI. It demonstrates:
 * - Cell-by-cell rendering with colors
 * - Cursor display
 * - Text attributes (bold, italic, underline, etc.)
 * - Scrolling support
 */

import SwiftUI
import DTermCore

/// Main terminal display view.
struct TerminalView: View {
    @ObservedObject var state: TerminalState

    /// Font size for terminal text.
    let fontSize: CGFloat = 14

    var body: some View {
        GeometryReader { geometry in
            ScrollView([.horizontal, .vertical]) {
                TerminalContentView(
                    terminal: state.terminal,
                    fontSize: fontSize,
                    lastUpdate: state.lastUpdate
                )
            }
            .background(Color.black)
        }
    }
}

/// Selection range in terminal coordinates (duplicated for iOS compatibility)
#if os(macOS)
// Use SelectionRange from MacContentView.swift
#else
struct SelectionRange: Equatable {
    var startRow: Int
    var startCol: Int
    var endRow: Int
    var endCol: Int

    var normalized: SelectionRange {
        if startRow < endRow || (startRow == endRow && startCol <= endCol) {
            return self
        }
        return SelectionRange(startRow: endRow, startCol: endCol, endRow: startRow, endCol: startCol)
    }

    func contains(row: Int, col: Int) -> Bool {
        let norm = normalized
        if row < norm.startRow || row > norm.endRow { return false }
        if row == norm.startRow && row == norm.endRow {
            return col >= norm.startCol && col <= norm.endCol
        }
        if row == norm.startRow { return col >= norm.startCol }
        if row == norm.endRow { return col <= norm.endCol }
        return true
    }
}
#endif

/// Renders terminal content as a grid of cells.
struct TerminalContentView: View {
    let terminal: DTermTerminal
    let fontSize: CGFloat
    let lastUpdate: Date  // Forces redraw
    var cursorBlink: Bool = true  // Cursor blink state
    var selection: SelectionRange? = nil  // Current selection

    /// Character width based on font.
    var charWidth: CGFloat {
        // Approximate width for monospace font
        fontSize * 0.6
    }

    /// Line height.
    var lineHeight: CGFloat {
        fontSize * 1.2
    }

    var body: some View {
        Canvas { context, size in
            let rows = terminal.rows
            let cols = terminal.cols
            let cursorRow = terminal.cursorRow
            let cursorCol = terminal.cursorCol
            let cursorVisible = terminal.cursorVisible

            for row in 0..<rows {
                for col in 0..<cols {
                    let x = CGFloat(col) * charWidth
                    let y = CGFloat(row) * lineHeight

                    if let cell = terminal.getCell(row: row, col: col) {
                        // Draw background
                        let bgColor = colorFromPacked(cell.background, defaultColor: .black)
                        if bgColor != .black {
                            context.fill(
                                Path(CGRect(x: x, y: y, width: charWidth, height: lineHeight)),
                                with: .color(bgColor)
                            )
                        }

                        // Draw selection highlight
                        if let sel = selection, sel.contains(row: row, col: col) {
                            context.fill(
                                Path(CGRect(x: x, y: y, width: charWidth, height: lineHeight)),
                                with: .color(Color.blue.opacity(0.4))
                            )
                        }

                        // Draw cursor (with blink support, only when not scrolled back)
                        let isAtBottom = terminal.displayOffset == 0
                        if cursorVisible && isAtBottom && row == cursorRow && col == cursorCol && cursorBlink {
                            context.fill(
                                Path(CGRect(x: x, y: y, width: charWidth, height: lineHeight)),
                                with: .color(Color.white.opacity(0.5))
                            )
                        }

                        // Draw character
                        if let char = cell.character {
                            let charStr = String(char)
                            if !charStr.isEmpty && charStr != " " {
                                let fgColor = colorFromPacked(cell.foreground, defaultColor: .white)

                                var text = Text(charStr)
                                    .font(.system(size: fontSize, design: .monospaced))
                                    .foregroundColor(fgColor)

                                if cell.flags.contains(.bold) {
                                    text = text.bold()
                                }
                                if cell.flags.contains(.italic) {
                                    text = text.italic()
                                }

                                context.draw(
                                    text,
                                    at: CGPoint(x: x + charWidth / 2, y: y + lineHeight / 2),
                                    anchor: .center
                                )

                                // Draw underline
                                if cell.flags.contains(.underline) {
                                    let underlineY = y + lineHeight - 2
                                    context.stroke(
                                        Path { path in
                                            path.move(to: CGPoint(x: x, y: underlineY))
                                            path.addLine(to: CGPoint(x: x + charWidth, y: underlineY))
                                        },
                                        with: .color(fgColor),
                                        lineWidth: 1
                                    )
                                }

                                // Draw strikethrough
                                if cell.flags.contains(.strikethrough) {
                                    let strikeY = y + lineHeight / 2
                                    context.stroke(
                                        Path { path in
                                            path.move(to: CGPoint(x: x, y: strikeY))
                                            path.addLine(to: CGPoint(x: x + charWidth, y: strikeY))
                                        },
                                        with: .color(fgColor),
                                        lineWidth: 1
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        .frame(
            width: CGFloat(terminal.cols) * charWidth,
            height: CGFloat(terminal.rows) * lineHeight
        )
    }

    /// Convert packed UInt32 color to SwiftUI Color.
    ///
    /// The packed format depends on the color type:
    /// - Default color: 0x00000000 (use default)
    /// - Indexed color: 0x01XXXXII where II is the palette index
    /// - True color: 0x02RRGGBB
    func colorFromPacked(_ packed: UInt32, defaultColor: Color) -> Color {
        // Check color type (high byte)
        let colorType = (packed >> 24) & 0xFF

        switch colorType {
        case 0:
            // Default color
            return defaultColor
        case 1:
            // Indexed color (256-color palette)
            let index = UInt8(packed & 0xFF)
            return colorFromIndex(index)
        case 2:
            // True color (24-bit RGB)
            let r = UInt8((packed >> 16) & 0xFF)
            let g = UInt8((packed >> 8) & 0xFF)
            let b = UInt8(packed & 0xFF)
            return Color(red: Double(r) / 255, green: Double(g) / 255, blue: Double(b) / 255)
        default:
            return defaultColor
        }
    }

    /// Convert 256-color palette index to Color.
    func colorFromIndex(_ index: UInt8) -> Color {
        // Standard 16 colors
        let standard: [Color] = [
            Color(red: 0, green: 0, blue: 0),           // 0: Black
            Color(red: 0.8, green: 0, blue: 0),         // 1: Red
            Color(red: 0, green: 0.8, blue: 0),         // 2: Green
            Color(red: 0.8, green: 0.8, blue: 0),       // 3: Yellow
            Color(red: 0, green: 0, blue: 0.8),         // 4: Blue
            Color(red: 0.8, green: 0, blue: 0.8),       // 5: Magenta
            Color(red: 0, green: 0.8, blue: 0.8),       // 6: Cyan
            Color(red: 0.75, green: 0.75, blue: 0.75),  // 7: White
            Color(red: 0.5, green: 0.5, blue: 0.5),     // 8: Bright Black
            Color(red: 1, green: 0, blue: 0),           // 9: Bright Red
            Color(red: 0, green: 1, blue: 0),           // 10: Bright Green
            Color(red: 1, green: 1, blue: 0),           // 11: Bright Yellow
            Color(red: 0, green: 0, blue: 1),           // 12: Bright Blue
            Color(red: 1, green: 0, blue: 1),           // 13: Bright Magenta
            Color(red: 0, green: 1, blue: 1),           // 14: Bright Cyan
            Color(red: 1, green: 1, blue: 1),           // 15: Bright White
        ]

        if index < 16 {
            return standard[Int(index)]
        } else if index < 232 {
            // 6x6x6 color cube (indices 16-231)
            let cubeIndex = Int(index) - 16
            let r = cubeIndex / 36
            let g = (cubeIndex / 6) % 6
            let b = cubeIndex % 6

            let toValue: (Int) -> Double = { v in
                v == 0 ? 0 : (Double(v) * 40 + 55) / 255
            }

            return Color(red: toValue(r), green: toValue(g), blue: toValue(b))
        } else {
            // Grayscale ramp (indices 232-255)
            let gray = Double(Int(index) - 232) * 10 + 8
            let value = gray / 255
            return Color(red: value, green: value, blue: value)
        }
    }
}
