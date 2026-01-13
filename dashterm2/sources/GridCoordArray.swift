//
//  GridCoordArray.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 3/29/23.
//

import Foundation

// This is a performance optimization because NSValue is kinda pokey.
@objc(iTermGridCoordArray)
class GridCoordArray: NSObject, Codable {
    private var coords = [VT100GridCoord]()

    override init() {
        super.init()
    }

    init(_ coords: [VT100GridCoord]) {
        self.coords = coords
        super.init()
    }

    private enum CodingKeys: String, CodingKey {
        case coords
    }

    required convenience init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let decodedCoords = try container.decode([VT100GridCoord].self, forKey: .coords)
        self.init(decodedCoords)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(coords, forKey: .coords)
    }

    @objc override func mutableCopy() -> Any {
        return GridCoordArray(coords)
    }

    @objc var last: VT100GridCoord {
        return coords.last ?? VT100GridCoord(x: 0, y: 0)
    }

    @objc var count: Int {
        coords.count
    }

    @objc func append(coord: VT100GridCoord) {
        coords.append(coord)
    }

    @objc func append(coord: VT100GridCoord, repeating: Int) {
        for _ in 0..<repeating {
            coords.append(coord)
        }
    }

    @objc func prepend(coord: VT100GridCoord, repeating: Int) {
        for _ in 0..<repeating {
            coords.insert(coord, at: 0)
        }
    }

    @objc func removeFirst(_ n: Int) {
        // BUG-f538: Use guard to prevent crash when n > count
        guard n >= 0 && n <= coords.count else {  // swiftlint:disable:this empty_count
            DLog("removeFirst(\(n)) on array with \(coords.count) elements - clamping")
            if n > 0 {
                coords.removeAll()
            }
            return
        }
        coords.removeFirst(n)
    }

    @objc func removeLast(_ n: Int) {
        // BUG-f539: Use guard to prevent crash when n > count
        guard n >= 0 && n <= coords.count else {  // swiftlint:disable:this empty_count
            DLog("removeLast(\(n)) on array with \(coords.count) elements - clamping")
            if n > 0 {
                coords.removeAll()
            }
            return
        }
        coords.removeLast(n)
    }

    @objc func removeRange(_ range: NSRange) {
        // BUG-1582: Use guard to safely convert Range instead of force unwrap
        guard let swiftRange = Range(range) else {
            DLog("Invalid NSRange: \(range)")
            return
        }
        // BUG-f542: Clamp range to valid bounds to prevent crash
        let clampedLower = max(0, min(swiftRange.lowerBound, coords.count))  // swiftlint:disable:this empty_count
        let clampedUpper = max(clampedLower, min(swiftRange.upperBound, coords.count))  // swiftlint:disable:this empty_count
        let safeRange = clampedLower..<clampedUpper
        coords.removeSubrange(safeRange)
    }

    @objc func removeAll() {
        coords = []
    }

    @objc subscript(_ i: Int) -> VT100GridCoord {
        // BUG-f502: Use guard clause instead of it_assert to prevent crash
        // Return default (0,0) for invalid indices - same as .last property for empty arrays
        guard i >= 0 && i < coords.count else {  // swiftlint:disable:this empty_count
            DLog("subscript[\(i)] on array with \(coords.count) elements - returning default")
            return VT100GridCoord(x: 0, y: 0)
        }
        return coords[i]
    }

    @objc(coordAt:) func coord(at i: Int) -> VT100GridCoord {
        // BUG-f540: Add bounds check to prevent crash - return default for invalid index
        guard i >= 0 && i < coords.count else {  // swiftlint:disable:this empty_count
            DLog("coord(at: \(i)) on array with \(coords.count) elements - returning default")
            return VT100GridCoord(x: 0, y: 0)
        }
        return coords[i]
    }

    @objc(appendContentsOfArray:) func appendContentsOfArray(_ array: GridCoordArray) {
        coords.append(contentsOf: array.coords)
    }

    @objc(resizeRange:to:)
    func resizeRange(_ original: NSRange, to replacement: NSRange) {
        // BUG-1582: Use guard to safely convert Range instead of force unwrap
        guard let subrange = Range(original) else {
            DLog("Invalid NSRange: \(original)")
            return
        }
        // BUG-f541: Clamp subrange to valid bounds to prevent crash
        let clampedLower = max(0, min(subrange.lowerBound, coords.count))  // swiftlint:disable:this empty_count
        let clampedUpper = max(clampedLower, min(subrange.upperBound, coords.count))  // swiftlint:disable:this empty_count
        let safeSubrange = clampedLower..<clampedUpper

        var updated = Array(coords[safeSubrange])
        while updated.count > replacement.length {  // swiftlint:disable:this empty_count
            updated.removeLast()
        }
        // BUG-1530: Use guard to safely access .last instead of force unwrap
        while updated.count < replacement.length {  // swiftlint:disable:this empty_count
            guard let lastCoord = updated.last else {
                break
            }
            updated.append(lastCoord)
        }
        coords.replaceSubrange(safeSubrange, with: updated)
    }
}

