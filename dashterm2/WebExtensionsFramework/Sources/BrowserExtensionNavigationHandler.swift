// BrowserExtensionNavigationHandler.swift
// Navigation handler for extension content script injection

import Foundation
import WebKit

/// Navigation handler that can be used by the browser's WKNavigationDelegate implementation
/// This is a simple pass-through handler since injection scripts handle content script injection
@MainActor
public class BrowserExtensionNavigationHandler: NSObject, BrowserExtensionWKNavigationDelegate {
    
    /// Logger for debugging and error reporting
    private let logger: BrowserExtensionLogger
    
    /// Initialize the navigation handler
    /// - Parameter logger: Logger for debugging and error reporting
    public init(logger: BrowserExtensionLogger) {
        self.logger = logger
        super.init()
    }
    
    // MARK: - BrowserExtensionWKNavigationDelegate Methods
    
    public func webView(_ webView: BrowserExtensionWKWebView, decidePolicyFor navigationAction: BrowserExtensionWKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        logger.debug("Navigation action requested for URL: \(navigationAction.be_request.url?.absoluteString ?? "nil")")
        // Injection script should already be installed by the active manager
        // No need to add it here on every navigation
        decisionHandler(.allow)
    }
    
    public func webView(_ webView: BrowserExtensionWKWebView, decidePolicyFor navigationResponse: BrowserExtensionWKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        // Response policy is typically used for download handling or blocking certain content types
        // Extensions don't currently need special response policy handling, so allow all responses
        logger.debug("Navigation response received, allowing")
        decisionHandler(.allow)
    }

    public func webView(_ webView: BrowserExtensionWKWebView, didStartProvisionalNavigation navigation: BrowserExtensionWKNavigation?) {
        // Provisional navigation started - the URL is being loaded but content not yet received
        // Extensions are notified via webNavigation API events (handled by BrowserExtensionActiveManager)
        logger.debug("Provisional navigation started for URL: \(webView.be_url?.absoluteString ?? "nil")")
    }

    public func webView(_ webView: BrowserExtensionWKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: BrowserExtensionWKNavigation?) {
        // Server redirect during provisional navigation
        // Extensions are notified via webNavigation.onBeforeRedirect event (if implemented)
        logger.debug("Server redirect received for URL: \(webView.be_url?.absoluteString ?? "nil")")
    }

    public func webView(_ webView: BrowserExtensionWKWebView, didFailProvisionalNavigation navigation: BrowserExtensionWKNavigation?, withError error: Error) {
        // Provisional navigation failed before content was received
        // This could be due to network errors, invalid URLs, etc.
        logger.error("Provisional navigation failed for URL: \(webView.be_url?.absoluteString ?? "nil"), error: \(error)")
    }
    
    public func webView(_ webView: BrowserExtensionWKWebView, didCommit navigation: BrowserExtensionWKNavigation?) {
        logger.debug("Navigation committed for URL: \(webView.be_url?.absoluteString ?? "nil")")
        // Injection script is already injected and will handle timing automatically
        // No additional action needed here
    }
    
    public func webView(_ webView: BrowserExtensionWKWebView, didFinish navigation: BrowserExtensionWKNavigation?) {
        logger.debug("Navigation finished for URL: \(webView.be_url?.absoluteString ?? "nil")")
        // Injection script is already injected and will handle timing automatically
        // No additional action needed here
    }
    
    public func webView(_ webView: BrowserExtensionWKWebView, didFail navigation: BrowserExtensionWKNavigation?, withError error: Error) {
        // Navigation failed after content started loading
        // Extensions are notified via webNavigation.onErrorOccurred event (if implemented)
        logger.error("Navigation failed for URL: \(webView.be_url?.absoluteString ?? "nil"), error: \(error)")
    }

    public func webView(_ webView: BrowserExtensionWKWebView, didReceive challenge: URLAuthenticationChallenge, completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        // Authentication challenges - use default handling
        // Extensions don't typically handle authentication directly; the browser handles it
        // Server trust, basic auth, client certificates are all handled by the system
        logger.debug("Authentication challenge received for protection space: \(challenge.protectionSpace.host)")
        completionHandler(.performDefaultHandling, nil)
    }

    public func webView(_ webView: BrowserExtensionWKWebView, webContentProcessDidTerminate navigation: BrowserExtensionWKNavigation?) {
        // Web content process crashed or was terminated by the system (e.g., due to memory pressure)
        // Extensions should be notified so they can clean up any state related to this webview
        logger.error("Web content process terminated for URL: \(webView.be_url?.absoluteString ?? "nil")")
    }
    
}