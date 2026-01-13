//
//  iTermSessionSelector.swift
//  DashTerm2
//
//  Created by George Nachman on 2/16/25.
//

@objc(iTermSessionSelector)
class SessionSelector: NSObject {
    @objc static let statusDidChange = NSNotification.Name("SessionSelectorStatusDidChange")
    private struct Entry {
        var terminal: Bool
        var reason: String
        var promise: iTermPromise<PTYSession>
        var seal: iTermPromiseSeal

        init?(terminal: Bool, reason: String) {
            self.terminal = terminal
            self.reason = reason
            var promiseSeal: iTermPromiseSeal?
            self.promise = iTermPromise { promiseSeal = $0 }
            guard let seal = promiseSeal else {
                // BUG-f567, BUG-f688: Log error and return nil instead of crashing if promiseSeal is not set
                DLog("BUG-f567: promiseSeal should be set by iTermPromise initializer - returning nil")
                return nil
            }
            self.seal = seal
        }
    }
    // nonisolated(unsafe) because access is protected by entriesLock
    nonisolated(unsafe) private static var entries = [Entry]()
    private static let entriesLock = NSLock()
    @objc static var isActive: Bool {
        entriesLock.withLock { !entries.isEmpty }
    }

    @objc static var currentReason: String? {
        return entriesLock.withLock { entries.last?.reason }
    }
    @objc static var wantsTerminal: Bool {
        return entriesLock.withLock { entries.last?.terminal == true }
    }

    static func select(terminal: Bool, reason: String) -> iTermPromise<PTYSession> {
        guard let entry = Entry(terminal: terminal, reason: reason) else {
            // BUG-f567: Return a rejected promise if Entry creation fails
            let rejectPromise = iTermPromise<PTYSession> { seal in
                seal.reject(NSError(domain: "SessionSelector", code: -1, userInfo: [NSLocalizedDescriptionKey: "Failed to create entry"]))
            }
            return rejectPromise
        }
        entriesLock.withLock {
            entries.append(entry)
        }
        NotificationCenter.default.post(name: statusDidChange, object: reason)
        return entry.promise
    }

    @objc static func didSelect(_ session: PTYSession) {
        let entry = entriesLock.withLock { entries.popLast() }
        if let entry {
            NotificationCenter.default.post(name: statusDidChange, object: nil)
            entry.seal.fulfill(session)
        }
    }

    @objc static func cancel(_ promise: iTermPromise<PTYSession>) {
        var entryToReject: Entry?
        var shouldNotify = false
        entriesLock.withLock {
            let wasEmpty = entries.isEmpty
            let i = entries.firstIndex { $0.promise === promise}
            guard let i else {
                return
            }
            entryToReject = entries[i]
            entries.remove(at: IndexSet(integer: i))
            shouldNotify = entries.isEmpty && !wasEmpty
        }
        entryToReject?.seal.reject(NSError(domain: "com.dashterm.dashterm2.session-selector", code: 0))
        if shouldNotify {
            NotificationCenter.default.post(name: statusDidChange, object: nil)
        }
    }
}
