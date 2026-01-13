//
//  NotifyingArray.swift
//  DashTerm2
//
//  Created by George Nachman on 2/24/25.
//

class NotifyingArray<Element> {
    private var storage = [Element]()

    var didInsert: ((Int) -> ())?
    var didRemove: ((Range<Int>) -> ())?
    var didModify: ((Int) -> ())?

    func append(_ element: Element) {
        storage.append(element)
        DLog("Insert \(element)")
        didInsert?(storage.count - 1)
    }

    @discardableResult
    func removeLast(_ n: Int = 1) -> [Element] {
        DLog("Remove \(String(describing: storage.last))")
        // BUG-f988: Guard against invalid count to prevent crash
        // If n is negative or greater than storage.count, this would crash
        guard n > 0 && n <= storage.count else {
            if n <= 0 {
                DLog("WARNING: removeLast called with non-positive count: \(n)")
                return []
            } else {
                DLog("WARNING: removeLast(\(n)) called but storage only has \(storage.count) elements")
                // Remove all elements instead of crashing
                let allElements = Array(storage)
                let originalCount = storage.count
                storage.removeAll()
                if originalCount > 0 {
                    didRemove?(0..<originalCount)
                }
                return allElements
            }
        }
        let count = storage.count
        let removed = Array(storage[(storage.count - n)...])
        storage.removeLast(n)
        didRemove?((count - n)..<count)
        return removed
    }

    var last: Element? {
        storage.last
    }

    func firstIndex(where test: (Element) -> Bool) -> Int? {
        return storage.firstIndex(where: test)
    }

    func last(where closure: (Element) throws -> Bool) rethrows -> Element? {
        return try storage.last(where: closure)
    }

    subscript(_ index: Int) -> Element {
        get {
            // BUG-f1009: Replace precondition with guard + fatalError logging
            // The precondition would crash without any logging, making debugging difficult
            // This logs the issue and then crashes with a clear error message
            guard index >= 0 && index < storage.count else {
                DLog("BUG-f1009 FATAL: NotifyingArray subscript get: index \(index) out of bounds (count=\(storage.count))")
                it_fatalError("BUG-f1009: NotifyingArray index \(index) out of bounds (count=\(storage.count))")
            }
            return storage[index]
        }
        set {
            // BUG-f991: Guard against out of bounds access on set
            guard index >= 0 && index < storage.count else {
                DLog("WARNING: NotifyingArray subscript set: index \(index) out of bounds (count=\(storage.count))")
                return
            }
            storage[index] = newValue
            DLog("didModify \(newValue)")
            didModify?(index)
        }
    }

    // BUG-f991: Safe subscript that returns nil for out of bounds index
    subscript(safe index: Int) -> Element? {
        guard index >= 0 && index < storage.count else {
            return nil
        }
        return storage[index]
    }

    // BUG-f991: Check if index is valid
    func isValidIndex(_ index: Int) -> Bool {
        return index >= 0 && index < storage.count
    }

    var count: Int {
        storage.count
    }
}

