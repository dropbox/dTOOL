//
//  NSRange+MultiCursor.swift
//  MultiCursor
//
//  Created by George Nachman on 3/31/22.
//

import Foundation

extension NSRange {
    func shifted(by delta: Int) -> NSRange {
        return NSRange(location: location + delta, length: length)
    }

    var droppingFirst: NSRange? {
        if length == 0 {
            return nil
        }
        // BUG-2661: Check for overflow when adding 1 to location
        let (newLocation, overflow) = location.addingReportingOverflow(1)
        if overflow {
            return nil
        }
        return NSRange(location: newLocation, length: length - 1)
    }

    var droppingLast: NSRange? {
        if length == 0 {
            return nil
        }
        return NSRange(location: location, length: length - 1)
    }

    // Not inclusive of `to`
    init(from: Int, to: Int) {
        self.init(location: min(from, to), length: max(from, to) -  min(from, to))
    }
}

