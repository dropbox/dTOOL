//
//  Int+iTerm.swift
//  DashTerm2
//
//  Created by George Nachman on 8/27/24.
//

import Foundation

extension Int {
    init(clamping value: CGFloat) {
        if value.isNaN || value >= CGFloat(Int.max) {
            self = Int.max
        } else if value <= CGFloat(Int.min) {
            self = Int.min
        } else {
            self = Int(value)
        }
    }

    // BUG-900: Safe Double to Int conversion that handles NaN, infinity, and overflow
    init(clamping value: Double) {
        if value.isNaN || value >= Double(Int.max) {
            self = Int.max
        } else if value <= Double(Int.min) {
            self = Int.min
        } else {
            self = Int(value)
        }
    }
}

extension Int32 {
    init(clamping value: CGFloat) {
        if value.isNaN || value >= CGFloat(Int.max) {
            self = Int32.max
        } else if value <= CGFloat(Int.min) {
            self = Int32.min
        } else {
            self = Int32(value)
        }
    }
}

extension Int64 {
    init(clamping value: CGFloat) {
        if value.isNaN || value >= CGFloat(Int.max) {
            self = Int64.max
        } else if value <= CGFloat(Int.min) {
            self = Int64.min
        } else {
            self = Int64(value)
        }
    }
}

extension Int: @retroactive CustomDebugStringConvertible {
    public var debugDescription: String {
        return String(self)
    }
}
