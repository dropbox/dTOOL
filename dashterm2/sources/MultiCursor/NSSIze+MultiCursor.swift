//
//  NSSIze+MultiCursor.swift
//  MultiCursor
//
//  Created by George Nachman on 3/31/22.
//

import AppKit

extension NSSize {
    func retinaRound(_ scale: CGFloat) -> NSSize {
        // BUG-509: Guard against division by zero when scale is 0
        guard scale > 0 else { return self }
        return NSSize(width: round(width * scale) / scale, height: round(height * scale) / scale)
    }
}
