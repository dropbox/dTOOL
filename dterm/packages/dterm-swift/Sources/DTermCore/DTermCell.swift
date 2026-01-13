/*
 * DTermCell.swift - Cell data types for DTermCore
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import Foundation
import CDTermCore

#if canImport(UIKit)
import UIKit
#endif

#if canImport(AppKit)
import AppKit
#endif

/// Represents a single cell in the terminal grid.
public struct DTermCell {
    /// Unicode codepoint (0 for empty cell).
    public let codepoint: UInt32

    /// Foreground color (packed).
    public let foreground: UInt32

    /// Background color (packed).
    public let background: UInt32

    /// Underline color (packed). 0xFFFFFFFF means use foreground.
    public let underlineColor: UInt32

    /// Cell attribute flags.
    public let flags: CellFlags

    /// Initialize from C struct.
    internal init(from cell: dterm_cell_t) {
        self.codepoint = cell.codepoint
        self.foreground = cell.fg
        self.background = cell.bg
        self.underlineColor = cell.underline_color
        self.flags = CellFlags(rawValue: cell.flags)
    }

    /// The character for this cell, or nil if empty.
    public var character: Character? {
        guard codepoint != 0, let scalar = Unicode.Scalar(codepoint) else {
            return nil
        }
        return Character(scalar)
    }

    /// Whether the cell is empty (no character).
    public var isEmpty: Bool {
        return codepoint == 0
    }
}

/// Cell attribute flags.
public struct CellFlags: OptionSet {
    public let rawValue: UInt16

    public init(rawValue: UInt16) {
        self.rawValue = rawValue
    }

    /// Bold text.
    public static let bold = CellFlags(rawValue: 1 << 0)

    /// Italic text.
    public static let italic = CellFlags(rawValue: 1 << 1)

    /// Underlined text.
    public static let underline = CellFlags(rawValue: 1 << 2)

    /// Blinking text.
    public static let blink = CellFlags(rawValue: 1 << 3)

    /// Inverse video (swap foreground/background).
    public static let inverse = CellFlags(rawValue: 1 << 4)

    /// Invisible text.
    public static let invisible = CellFlags(rawValue: 1 << 5)

    /// Strikethrough text.
    public static let strikethrough = CellFlags(rawValue: 1 << 6)

    /// Faint/dim text.
    public static let faint = CellFlags(rawValue: 1 << 7)

    /// Double underline.
    public static let doubleUnderline = CellFlags(rawValue: 1 << 8)

    /// Curly underline.
    public static let curlyUnderline = CellFlags(rawValue: 1 << 9)

    /// Dotted underline.
    public static let dottedUnderline = CellFlags(rawValue: 1 << 10)

    /// Dashed underline.
    public static let dashedUnderline = CellFlags(rawValue: 1 << 11)

    /// Superscript (SGR 73).
    public static let superscript = CellFlags(rawValue: 1 << 12)

    /// Subscript (SGR 74).
    public static let `subscript` = CellFlags(rawValue: 1 << 13)

    /// Wide character (double-width).
    public static let wide = CellFlags(rawValue: 1 << 14)
}

/// RGB color value.
public struct DTermRGB: Equatable {
    public let red: UInt8
    public let green: UInt8
    public let blue: UInt8

    public init(red: UInt8, green: UInt8, blue: UInt8) {
        self.red = red
        self.green = green
        self.blue = blue
    }

    internal init(from rgb: DtermRgb) {
        self.red = rgb.r
        self.green = rgb.g
        self.blue = rgb.b
    }

    #if canImport(UIKit)
    /// Convert to UIColor.
    public var uiColor: UIColor {
        return UIColor(
            red: CGFloat(red) / 255.0,
            green: CGFloat(green) / 255.0,
            blue: CGFloat(blue) / 255.0,
            alpha: 1.0
        )
    }
    #endif

    #if canImport(AppKit) && !targetEnvironment(macCatalyst)
    /// Convert to NSColor.
    public var nsColor: NSColor {
        return NSColor(
            red: CGFloat(red) / 255.0,
            green: CGFloat(green) / 255.0,
            blue: CGFloat(blue) / 255.0,
            alpha: 1.0
        )
    }
    #endif
}
