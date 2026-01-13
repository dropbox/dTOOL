//
//  AsyncSemaphore.swift
//  DashTerm2
//
//  Created by George Nachman on 6/9/25.
//

actor AsyncSemaphore {
    private var value: Int
    private var waiters: [CheckedContinuation<Void, Never>] = []

    init(value: Int) {
        self.value = value
    }

    func wait() async {
        await withCheckedContinuation { continuation in
            if value > 0 {
                value -= 1
                DLog("Semaphore decremented to \(value)")
                continuation.resume()
            } else {
                DLog("Semaphore adding waiter")
                waiters.append(continuation)
            }
        }
    }

    func signal() {
        // BUG-f985: Use guard with isEmpty check and safer pattern for removeFirst()
        // Even with actor isolation, using explicit guard is more defensive
        guard !waiters.isEmpty else {
            value += 1
            DLog("Semaphore incremented to \(value)")
            return
        }
        let continuation = waiters.removeFirst()
        DLog("Semaphore resuming waiter")
        continuation.resume()
    }
}

actor AsyncMutex {
    private let sema = AsyncSemaphore(value: 1)

    func sync<T>(_ block: () async throws -> T) async rethrows -> T{
        await sema.wait()
        do {
            let result = try await block()
            await sema.signal()
            return result
        } catch {
            await sema.signal()
            throw error
        }
    }
}
