//
//  NSPoint+iTerm.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 8/27/24.
//

import Foundation

extension NSPoint {
    static func -(lhs: NSPoint, rhs: NSPoint) -> NSPoint {
        return NSPoint(x: lhs.x - rhs.x, y: lhs.y - rhs.y)
    }
    static func -(lhs: NSPoint, rhs: NSPoint) -> NSSize {
        return NSSize(width: abs(lhs.x - rhs.x), height: abs(lhs.y - rhs.y))
    }
    static func +(lhs: NSPoint, rhs: NSPoint) -> NSPoint {
        return NSPoint(x: lhs.x + rhs.x, y: lhs.y + rhs.y)
    }
    static func *(lhs: NSPoint, rhs: CGFloat) -> NSPoint {
        return NSPoint(x: lhs.x * rhs, y: lhs.y * rhs)
    }
    /// BUG-524: Guard against division by zero - return unchanged point if divisor is zero
    static func /(lhs: NSPoint, rhs: CGFloat) -> NSPoint {
        guard rhs != 0 else { return lhs }
        return NSPoint(x: lhs.x / rhs, y: lhs.y / rhs)
    }
    /// BUG-525: Guard against division by zero - return unchanged coordinate if divisor dimension is zero
    static func /(lhs: NSPoint, rhs: NSSize) -> NSPoint {
        return NSPoint(
            x: rhs.width != 0 ? lhs.x / rhs.width : lhs.x,
            y: rhs.height != 0 ? lhs.y / rhs.height : lhs.y)
    }
    func addingY(_ dy: CGFloat) -> NSPoint {
        return NSPoint(x: x, y: y + dy)
    }
    func addingX(_ dx: CGFloat) -> NSPoint {
        return NSPoint(x: x + dx, y: y)
    }
    static func -=(lhs: inout NSPoint, rhs: NSPoint) {
        lhs = lhs - rhs
    }
    static func +=(lhs: inout NSPoint, rhs: NSPoint) {
        lhs = lhs + rhs
    }
}

extension NSPoint {
    func distance(to other: NSPoint) -> CGFloat {
        return sqrt(pow(x - other.x, 2) + pow(y - other.y, 2))
    }
}
