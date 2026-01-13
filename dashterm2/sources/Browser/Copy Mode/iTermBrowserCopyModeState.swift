//
//  iTermBrowserCopyModeState.swift
//  DashTerm2
//
//  Created by George Nachman on 7/29/25.
//

import Foundation
import WebKit

@MainActor
class iTermBrowserCopyModeState: NSObject {
    // BUG-f550: Made webView optional to allow creating a no-op state when webView is unavailable
    let webView: iTermBrowserWebView?
    private let sessionSecret: String
    private var continuation: CheckedContinuation<Bool, Never>?

    var selecting: Bool = false {
        didSet {
            webView?.safelyEvaluateJavaScript(iife("DashTerm2CopyMode.selecting = \(selecting)"),
                                              in: nil,
                                              in: .defaultClient,
                                              completionHandler: nil)
        }
    }
    var mode: iTermSelectionMode = .kiTermSelectionModeCharacter {
        didSet {
            webView?.safelyEvaluateJavaScript(iife("DashTerm2CopyMode.mode = \(mode.rawValue)"),
                                              in: nil,
                                              in: .defaultClient,
                                              completionHandler: nil)
        }
    }

    init(webView: iTermBrowserWebView?, sessionSecret: String) {
        self.webView = webView
        self.sessionSecret = sessionSecret
    }

    private func callJavaScriptSync(_ script: String) -> Bool {
        // BUG-f550: Handle nil webView gracefully
        guard let webView else {
            DLog("iTermBrowserCopyModeState: webView is nil, skipping JavaScript call")
            return false
        }
        let c = continuation
        continuation = nil
        webView.safelyEvaluateJavaScript(iife("return " + script), in: nil, in: .defaultClient) { evalResult in
            switch evalResult {
            case .success(let response):
                if let boolResponse = response as? Bool {
                    c?.resume(with: .success(boolResponse))
                } else {
                    c?.resume(with: .success(true))
                }
            case .failure(let error):
                DLog("\(error) from \(script)")
                c?.resume(with: .success(false))
            }
        }
        return true
    }
}

@MainActor
extension iTermBrowserCopyModeState: @preconcurrency iTermCopyModeStateProtocol {
    func moveBackwardWord() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveBackwardWord('\(sessionSecret)')")
    }

    func moveForwardWord() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveForwardWord('\(sessionSecret)')")
    }

    func moveBackwardBigWord() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveBackwardBigWord('\(sessionSecret)')")
    }

    func moveForwardBigWord() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveForwardBigWord('\(sessionSecret)')")
    }

    func moveLeft() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveLeft('\(sessionSecret)')")
    }

    func moveRight() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveRight('\(sessionSecret)')")
    }

    func moveUp() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveUp('\(sessionSecret)')")
    }

    func moveDown() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveDown('\(sessionSecret)')")
    }

    func moveToStartOfNextLine() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToStartOfNextLine('\(sessionSecret)')")
    }

    func pageUp() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.pageUp('\(sessionSecret)')")
    }

    func pageDown() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.pageDown('\(sessionSecret)')")
    }

    func pageUpHalfScreen() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.pageUpHalfScreen('\(sessionSecret)')")
    }

    func pageDownHalfScreen() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.pageDownHalfScreen('\(sessionSecret)')")
    }

    func previousMark() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.previousMark('\(sessionSecret)')")
    }

    func nextMark() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.nextMark('\(sessionSecret)')")
    }

    func moveToStart() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToStart('\(sessionSecret)')")
    }

    func moveToEnd() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToEnd('\(sessionSecret)')")
    }

    func moveToStartOfIndentation() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToStartOfIndentation('\(sessionSecret)')")
    }

    func moveToBottomOfVisibleArea() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToBottomOfVisibleArea('\(sessionSecret)')")
    }

    func moveToMiddleOfVisibleArea() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToMiddleOfVisibleArea('\(sessionSecret)')")
    }

    func moveToTopOfVisibleArea() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToTopOfVisibleArea('\(sessionSecret)')")
    }

    func moveToStartOfLine() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToStartOfLine('\(sessionSecret)')")
    }

    func moveToEndOfLine() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.moveToEndOfLine('\(sessionSecret)')")
    }

    func swap() {
        _ = callJavaScriptSync("DashTerm2CopyMode.swap('\(sessionSecret)')")
    }

    func scrollUp() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.scrollUp('\(sessionSecret)')")
    }

    func scrollDown() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.scrollDown('\(sessionSecret)')")
    }

    func performAsynchronously(_ block: (() -> Void)!, completion: ((Bool) -> Void)!) {
        Task { @MainActor in
            let result = await withCheckedContinuation { @MainActor continuation in
                self.continuation = continuation
                block()
                if self.continuation != nil {
                    self.continuation = nil
                    continuation.resume(with: .success(false))
                }
            }
            completion(result)
        }
    }
}

// MARK: - Additional Helper Methods
extension iTermBrowserCopyModeState {
    func enableCopyMode() {
        // BUG-444: Use optional chaining for webView since it can be nil
        webView?.safelyEvaluateJavaScript("DashTerm2CopyMode.enable('\(sessionSecret)')",
                                         in: nil,
                                         in: .defaultClient,
                                         completionHandler: nil)
    }

    func disableCopyMode() {
        // BUG-445: Use optional chaining for webView since it can be nil
        webView?.safelyEvaluateJavaScript("DashTerm2CopyMode.disable('\(sessionSecret)')",
                                         in: nil,
                                         in: .defaultClient,
                                         completionHandler: nil)
    }

    func copySelection() -> Bool {
        return callJavaScriptSync("DashTerm2CopyMode.copySelection('\(sessionSecret)')")
    }

    func scrollCursorIntoView() {
        // BUG-446: Use optional chaining for webView since it can be nil
        webView?.safelyEvaluateJavaScript("DashTerm2CopyMode.scrollCursorIntoView('\(sessionSecret)')",
                                         in: nil,
                                         in: .defaultClient,
                                         completionHandler: nil)
    }
}
