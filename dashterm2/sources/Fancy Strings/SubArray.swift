//
//  SubArray.swift
//  DashTerm2
//
//  Created by George Nachman on 4/28/25.
//

/// This exists because ArraySlice is fucking stupid and doesn't get a start index of 0.
struct SubArray<Element>: RandomAccessCollection {
    private let base: [Element]
    private let bounds: Range<Int>

    /// - Parameters:
    ///   - base: the full array
    ///   - bounds: the subrange of `base` you wish to expose; will be clamped to valid range.
    /// BUG-f531: Clamp bounds instead of precondition crash for out-of-bounds access
    init(_ base: [Element], bounds: Range<Int>) {
        // Clamp bounds to valid range instead of crashing
        // Use Swift.min/Swift.max to avoid conflict with Collection's min()/max() methods
        let clampedLower = Swift.max(0, Swift.min(bounds.lowerBound, base.count))
        let clampedUpper = Swift.max(clampedLower, Swift.min(bounds.upperBound, base.count))
        let clampedBounds = clampedLower..<clampedUpper

        if clampedBounds != bounds {
            DLog("SubArray: clamped bounds \(bounds) to \(clampedBounds) for array of size \(base.count)")
        }

        self.base = base
        self.bounds = clampedBounds
    }

    init(_ array: [Element]) {
        base = array
        bounds = 0..<base.count
    }

    /// Returns a zero-based slice of this SubArray.
    /// BUG-f532: Clamp range instead of precondition crash for out-of-bounds access
    subscript(range: Range<Int>) -> SubArray<Element> {
        // Clamp range to valid bounds
        // Use Swift.min/Swift.max to avoid conflict with Collection's min()/max() methods
        let clampedLower = Swift.max(0, Swift.min(range.lowerBound, count))
        let clampedUpper = Swift.max(clampedLower, Swift.min(range.upperBound, count))
        if clampedLower != range.lowerBound || clampedUpper != range.upperBound {
            DLog("SubArray subscript: clamped range \(range) to \(clampedLower)..<\(clampedUpper) for count \(count)")
        }
        let newLower = bounds.lowerBound + clampedLower
        let newUpper = bounds.lowerBound + clampedUpper
        return SubArray(base, bounds: newLower..<newUpper)
    }

    // MARK: RandomAccessCollection

    /// Always starts at zero
    var startIndex: Int { 0 }

    /// Always ends at count
    var endIndex: Int {
        bounds.count
    }

    /// Access by zero-based position
    /// BUG-f623, BUG-f686: Use guard with safe fallback and log instead of crashing
    subscript(position: Int) -> Element {
        // In release builds it_assert is stripped, so we need a real guard
        guard position >= 0 && position < count else {
            // Return element at clamped position to avoid crash, log the issue
            DLog("BUG-f623: SubArray index \(position) out of bounds \(bounds) - clamping to valid range")
            // Return element at clamped position to avoid crash
            let clampedPosition = Swift.max(0, Swift.min(position, count > 0 ? count - 1 : 0))
            if count > 0 {
                DLog("SubArray: position \(position) out of bounds, using clamped \(clampedPosition)")
                return base[bounds.lowerBound + clampedPosition]
            }
            // If array is empty, we have no choice but to let it crash - but at least we logged it
            it_fatalError("SubArray: cannot access position \(position) in empty array")
        }
        return base[bounds.lowerBound + position]
    }

    /// BUG-f1396: Safe subscript that returns nil for out of bounds or empty array access
    /// instead of crashing. Use this when you're not sure if the index is valid.
    subscript(safe position: Int) -> Element? {
        guard position >= 0 && position < count else {
            return nil
        }
        return base[bounds.lowerBound + position]
    }

    /// BUG-f1396: Check if position is valid for subscript access
    func isValidPosition(_ position: Int) -> Bool {
        return position >= 0 && position < count
    }

    /// Forward index
    func index(after i: Int) -> Int {
        i + 1
    }

    /// Backward index (from RandomAccessCollection)
    func index(before i: Int) -> Int {
        i - 1
    }

    var array: [Element] {
        return Array(base[bounds])
    }
}

extension SubArray {
    /// Number of elements in the slice
    var count: Int { bounds.count }
}

extension SubArray: Equatable where Element: Equatable {
}
