//
//  NSSize+iTerm.swift
//  DashTerm2
//
//  Created by George Nachman on 8/27/24.
//

import Foundation

extension NSSize {
    static func *(lhs: NSSize, rhs: CGFloat) -> NSSize {
        return NSSize(width: lhs.width * rhs, height: lhs.height * rhs)
    }
    /// Divide size by scalar. Returns .zero if divisor is zero to prevent NaN/Inf.
    static func /(lhs: NSSize, rhs: CGFloat) -> NSSize {
        // BUG-12002: Guard against division by zero - return .zero for safety
        guard rhs != 0 else { return .zero }
        return NSSize(width: lhs.width / rhs, height: lhs.height / rhs)
    }
    /// Element-wise division of sizes. Returns .zero for dimensions where divisor is zero.
    static func /(lhs: NSSize, rhs: NSSize) -> NSSize {
        // BUG-12002: Guard against division by zero in each dimension
        let newWidth = rhs.width != 0 ? lhs.width / rhs.width : 0
        let newHeight = rhs.height != 0 ? lhs.height / rhs.height : 0
        return NSSize(width: newWidth, height: newHeight)
    }
}

extension NSSize: @retroactive Hashable {
    public func hash(into hasher: inout Hasher) {
        hasher.combine(width)
        hasher.combine(height)
    }
}

extension NSSize {
    func multiplied(by other: NSSize) -> NSSize {
        return NSSize(width: width * other.width, height: height * other.height)
    }
    /// Returns the multiplicative inverse (1/width, 1/height).
    /// Returns .zero for dimensions that are zero to prevent NaN/Inf.
    var inverted: NSSize {
        // BUG-12002: Guard against division by zero in each dimension
        let newWidth = width != 0 ? 1.0 / width : 0
        let newHeight = height != 0 ? 1.0 / height : 0
        return NSSize(width: newWidth, height: newHeight)
    }
}

extension NSSize {
    var area: CGFloat {
        return width * height
    }
}

func abs(_ size: NSSize) -> NSSize {
    return NSSize(width: abs(size.width), height: abs(size.height))
}
