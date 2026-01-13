//
//  CPUGovernor.swift
//  DashTerm2
//
//  Created by George Nachman on 8/4/21.
//

import Foundation
import os.log

func DLog(_ messageBlock: @autoclosure () -> String, file: String = #file, line: Int = #line, function: String = #function) {
    let message = messageBlock()
    os_log("%{public}@", log: .default, type: .debug, message)
}

private class AtomicFlag {
    private var _value: Bool
    private let queue = DispatchQueue(label: "com.dashterm.dashterm2.atomic-flag")

    var value: Bool {
        get {
            return queue.sync { return _value }
        }
        set {
            queue.sync { _value = newValue }
        }
    }

    init(_ value: Bool) {
        self._value = value
    }
}

@objc(iTermCPUGovernor) public class CPUGovernor: NSObject {
    @objc public var pid: pid_t {
        willSet {
            if newValue != pid {
                DLog("pid will change")
                invalidate()
            }
        }
        didSet {
            DLog("pid=\(pid)")
        }
    }

    // Time running / time suspended
    private let dutyCycle: Double

    private let queue = DispatchQueue(label: "com.dashterm.dashterm2.cpu-governor")
    private var running = AtomicFlag(false)

    // Amount of time in a suspend-wait-resume-wait cycle.
    private let cycleTime = 0.1

    // Outstanding tokens.
    private var tokens = Set<Int>()
    private var nextToken = 0
    private var _gracePeriodEndTime: DispatchTime?

    @objc(initWithPID:dutyCycle:) public init(_ pid: pid_t, dutyCycle: Double) {
        self.pid = pid
        self.dutyCycle = dutyCycle
    }

    @objc(setGracePeriodDuration:) public func setGracePeriodDuration(_ value: TimeInterval) {
        if value <= 0 {
            return
        }
        // BUG-2658: Check for overflow in nanoseconds calculation
        // When 10.14 support is dropped we can use DispatchTime.advanced(by:)
        let currentNanoseconds = DispatchTime.now().uptimeNanoseconds
        // Cap value to avoid overflow (max ~292 years in nanoseconds fits in UInt64)
        let cappedValue = min(value, TimeInterval(UInt64.max / NSEC_PER_SEC))
        let (durationNanoseconds, mulOverflow) = UInt64(cappedValue).multipliedReportingOverflow(by: NSEC_PER_SEC)
        if mulOverflow {
            // Use maximum possible time if overflow
            _gracePeriodEndTime = DispatchTime(uptimeNanoseconds: UInt64.max)
            return
        }
        let (endNanoseconds, addOverflow) = currentNanoseconds.addingReportingOverflow(durationNanoseconds)
        if addOverflow {
            // Use maximum possible time if overflow
            _gracePeriodEndTime = DispatchTime(uptimeNanoseconds: UInt64.max)
            return
        }
        _gracePeriodEndTime = DispatchTime(uptimeNanoseconds: endNanoseconds)
    }

    @objc public func incr() -> Int {
        let token = nextToken
        nextToken += 1
        // BUG-1618: Use guard instead of precondition - tokens should be unique but if not, just skip
        guard !tokens.contains(token) else {
            DLog("Warning: token \(token) already exists in CPUGovernor tokens set")
            return token
        }
        tokens.insert(token)
        update()
        DLog("Allocate token \(token) giving \(tokens)")
        return token
    }

    @objc(decr:) public func decr(_ token: Int) {
        guard tokens.remove(token) != nil else {
            DLog("Deallocate already-removed token \(token)")
            return
        }
        DLog("Deallocate token \(token) giving \(tokens)")
        update()
    }

    @objc public func invalidate() {
        DLog("Invalidate")
        tokens.removeAll()
        guard running.value else {
            return
        }
        update()
    }

    private func update() {
        if tokens.isEmpty && running.value {
            stop()
        } else if !tokens.isEmpty && !running.value {
            start()
        }
    }

    private func start() {
        guard !running.value else {
            return
        }
        DLog("running=true")
        running.value = true
        queue.async { [weak self] in
            self?.mainloop()
        }
    }

    private func stop() {
        guard running.value else {
            return
        }
        DLog("running=false")
        running.value = false
    }

    private var processTerminated: Bool {
        return kill(pid, 0) != 0
    }

    private func mainloop() {
        dispatchPrecondition(condition: .onQueue(queue))

        DLog("Start mainloop")
        while running.value && !processTerminated {
            cycle()
        }
        DLog("Return from mainloop")
    }

    private func cycle() {
        DLog("Cycle")
        suspend()
        sleepWhileSuspended()

        if processTerminated {
            return
        }

        resume()
        sleepWhileRunning()
    }

    private var inGracePeriod: Bool {
        guard let gracePeriodEndTime = _gracePeriodEndTime else {
            return false
        }
        return DispatchTime.now().uptimeNanoseconds < gracePeriodEndTime.uptimeNanoseconds
    }

    private func suspend() {
        if inGracePeriod {
            DLog("Grace period - not suspending \(pid)")
            return
        }
        DLog("Suspend \(pid) now=\(DispatchTime.now().uptimeNanoseconds) grace=\(_gracePeriodEndTime?.uptimeNanoseconds ?? 0)")
        kill(pid, SIGSTOP)
    }

    private func resume() {
        DLog("Resume \(pid)")
        kill(pid, SIGCONT)
    }

    private func sleepWhileSuspended() {
        sleep(1)
    }

    private func sleepWhileRunning() {
        sleep(dutyCycle)
    }

    private func sleep(_ multiplier: TimeInterval) {
        // BUG-f1399: Guard against division by zero when dutyCycle is -1
        // This could happen if an invalid dutyCycle is passed to init
        let divisor = dutyCycle + 1
        guard divisor > 0 else {
            DLog("BUG-f1399: Invalid dutyCycle \(dutyCycle) would cause division by zero - skipping sleep")
            return
        }
        let coeff = cycleTime / divisor
        DLog("Sleep for \(multiplier) units of \(coeff) sec = \(coeff * multiplier)")
        Thread.sleep(forTimeInterval: coeff * multiplier)
    }
}
