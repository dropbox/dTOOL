//
//  NSPoint+MultiCursor.swift
//  MultiCursor
//
//  Created by George Nachman on 3/31/22.
//

import AppKit

extension NSPoint {
    func retinaRound(_ scale: CGFloat) -> NSPoint {
        // BUG-510: Guard against division by zero when scale is 0
        guard scale > 0 else { return self }
        return NSPoint(x: round(x * scale) / scale, y: round(y * scale) / scale)
    }
}

