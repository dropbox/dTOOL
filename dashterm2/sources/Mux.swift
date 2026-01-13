//
//  Mux.swift
//  DashTerm2
//
//  Created by George Nachman on 10/26/21.
//

import Foundation

@objc(iTermMux)
class Mux: NSObject {
    private let group = DispatchGroup()
    private var count = 0
    private struct Task {
        let description: String
    }
    private var tasks = [Task?]()
    @objc var pendingDescriptions: [String] {
        return tasks.compactMap { $0?.description }
    }

    /// Convert evaluator results into strings that are safe to hand back to Objective-C callers.
    /// Nil and NSNull values are converted into empty strings so downstream code never sees NSNull.
    static func sanitizedStrings(from values: [AnyObject?]) -> [String] {
        return values.enumerated().map { index, value in
            sanitizedString(for: value, at: index)
        }
    }

    private static func sanitizedString(for value: AnyObject?, at index: Int) -> String {
        guard let value else {
            DLog("Mux sanitized nil expression result at index \(index)")
            return ""
        }
        if value is NSNull {
            DLog("Mux sanitized NSNull expression result at index \(index)")
            return ""
        }
        if let string = value as? String {
            return string
        }
        if let nsString = value as? NSString {
            return nsString as String
        }
        return String(describing: value)
    }

    func add(_ description: String) -> (() -> Void) {
        let i = tasks.count
        tasks.append(Task(description: description))
        group.enter()
        var completed = false
        count += 1
        let completion = {
            // BUG-f631: Use guard instead of it_assert - assertions stripped in release builds
            // Calling completion twice would cause group.leave() imbalance
            guard !completed else {
                DLog("BUG-f631: Mux completion handler called twice for task \(i) - ignoring duplicate")
                return
            }
            completed = true
            self.count -= 1
            self.tasks[i] = nil
            self.group.leave()
        }
        return completion
    }

    @objc
    func join(_ block: @escaping () -> Void) {
        if count == 0 {  // swiftlint:disable:this empty_count
            // A little optimization - avoid a spin of the runloop if everything completed synchronously.
            block()
            return
        }
        group.notify(queue: .main) {
            block()
        }
    }
}

// MARK:- Convenience methods for interpolated strings.

extension Mux {
    @objc(evaluateInterpolatedString:scope:timeout:retryTime:success:error:)
    func evaluate(_ interpolatedString: String,
                  scope: iTermVariableScope,
                  timeout: TimeInterval,
                  retryTime: TimeInterval,
                  success successHandler: @escaping (AnyObject?) -> Void,
                  error errorHandler: @escaping (Error) -> Void) {
        let completion = add(interpolatedString)
        let evaluator = iTermExpressionEvaluator(interpolatedString: interpolatedString,
                                                 scope: scope)
        if retryTime > 0 {
            evaluator.retryUntil = Date().addingTimeInterval(retryTime)
        }
        evaluator.evaluate(withTimeout: timeout, sideEffectsAllowed: true) { evaluator in
            defer {
                completion()
            }
            if let error = evaluator.error {
                errorHandler(error)
            } else {
                successHandler(evaluator.value as AnyObject?)
            }
        }
    }

    @objc(evaluateInterpolatedStrings:scope:timeout:retryTime:success:error:)
    func evaluate(_ interpolatedStrings: [String],
                  scope: iTermVariableScope,
                  timeout: TimeInterval,
                  retryTime: TimeInterval,
                  success successHandler: @escaping ([String]) -> Void,
                  error errorHandler: @escaping (Error) -> Void) {
        DLog("Mux \(self) evaluating interpolated strings \(interpolatedStrings) with timeout \(timeout) and scope \(scope)")
        enum Result {
            case pending
            case value(AnyObject?)
            case error(Error)
        }
        var lastError: Error?
        var results: [Result] = []
        for (i, string) in interpolatedStrings.enumerated() {
            results.append(.pending)
            evaluate(string, scope: scope, timeout: timeout, retryTime: retryTime) { obj in
                DLog("Mux \(self) evaluated \(string) with result \(obj?.debugDescription ?? "(nil)")")
                results[i] = .value(obj)
            } error: { error in
                DLog("Mux \(self) evaluated \(string) with error \(error)")
                lastError = error
                results[i] = .error(error)
            }
        }
        join {
            if let error = lastError {
                errorHandler(error)
            } else {
                // BUG-f632: Log unexpected states instead of it_assert - assertions stripped in release
                let rawValues: [AnyObject?] = results.enumerated().map { index, result in
                    switch result {
                    case let .value(value):
                        return value
                    case let .error(error):
                        // This shouldn't happen since we're in the else branch (lastError is nil)
                        // but handle gracefully if it does
                        DLog("BUG-f632: Mux result \(index) unexpectedly stored error: \(error)")
                        return nil
                    case .pending:
                        // This shouldn't happen - join should only fire after all completions
                        DLog("BUG-f632: Mux result \(index) was still pending when join completed")
                        return nil
                    }
                }
                successHandler(Self.sanitizedStrings(from: rawValues))
            }
        }
    }
}
