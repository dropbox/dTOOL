//
//  PasteboardReporter.swift
//  DashTerm2
//
//  Created by George Nachman on 3/6/22.
//

import Foundation

@objc(iTermPasteboardReporterDelegate)
protocol PasteboardReporterDelegate: AnyObject {
    @objc func pasteboardReporterRequestPermission(_ sender: PasteboardReporter,
                                                   completion: @escaping (_ allowed: Bool, _ permanently: Bool) -> Void)
    @objc func pasteboardReporter(_ sender: PasteboardReporter, reportPasteboard: String)
}

// A helper to manage sharing clipboard contents, mostly to do with ensuring the sharing is authorized.
@objc(iTermPasteboardReporter)
class PasteboardReporter: NSObject {
    private static let userDefaultsKey = "NoSyncNeverAllowPaste"
    @objc weak var delegate: PasteboardReporterDelegate?

    // The int values match tags in prefs.
    @objc(iTermPasteboardReporterConfiguration) enum Configuration: Int {
        case never = 0
        case always = 1
        case askEachTime = 2
    }

    @objc
    static func configuration() -> Configuration {
        if UserDefaults.standard.bool(forKey: Self.userDefaultsKey) {
            return .never
        }
        if SecureUserDefaults.instance.allowPaste.value {
            return .always
        }
        return .askEachTime
    }

    @objc
    static func setConfiguration(_ value: Int) {
        // BUG-2889: Don't force unwrap - invalid value from ObjC could crash
        guard let config = Configuration(rawValue: value) else {
            DLog("Invalid configuration value: \(value)")
            return
        }
        set(configuration: config)
    }

    static func set(configuration: Configuration) {
        switch configuration {
        case .never:
            if Self.removeAuth() {
                UserDefaults.standard.set(true, forKey: Self.userDefaultsKey)
            }

        case .always:
            guard doubleCheck() else {
                return
            }
            do {
                DLog("Set secure user default to true")
                try SecureUserDefaults.instance.allowPaste.set(true)
                DLog("Set user default to false")
                UserDefaults.standard.set(false, forKey: Self.userDefaultsKey)
            } catch {
                DLog("Failed to enable allowPaste: \(error.localizedDescription)")
            }

        case .askEachTime:
            guard Self.removeAuth() else {
                return
            }
            UserDefaults.standard.set(false, forKey: Self.userDefaultsKey)
        }
    }

    private static func removeAuth() -> Bool {
        do {
            try SecureUserDefaults.instance.allowPaste.set(nil)
            return true
        } catch {
            if !SecureUserDefaults.instance.allowPaste.value {
                return true
            }
            failedToDeleteSecureSetting(error)
            return false
        }
    }

    private static func failedToDeleteSecureSetting(_ error: Error) {
        guard let url = SecureUserDefaults.instance.allowPaste.url else {
            // App support doesn't exist, so no problem.
            return
        }
        let alert = NSAlert()
        alert.messageText = "Error Updating Settings"
        alert.informativeText = "An error occurred while removing the file that authorizes clipboard reporting: \(error.localizedDescription).\nAs long as this file exists, clipboard reporting could be enabled by programs running on this computer."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Reveal in Finder")
        alert.runModal()
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    private static func doubleCheck() -> Bool {
        let alert = NSAlert()
        alert.messageText = "Really Enable Clipboard Reporting?"
        alert.informativeText = "Reporting the content of the clipboard to apps running inside DashTerm2 may expose sensitive information such as passwords. Think carefully before enabling this."
        alert.alertStyle = .warning
        let button = alert.addButton(withTitle: "OK")
        if #available(macOS 11.0, *) {
            button.hasDestructiveAction = true
        }
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    @objc
    func handleRequest(pasteboard: String, completion: @escaping () -> ()) {
        // BUG-1180: Validate pasteboard content before reporting.
        // Skip reporting if the pasteboard is empty - nothing useful to report.
        guard !pasteboard.isEmpty else {
            DLog("Pasteboard is empty, skipping report")
            completion()
            return
        }
        switch Self.configuration() {
        case .never:
            DLog("Pasteboard reporting permanently disallowed")
            completion()
            return
        case .always:
            DLog("Pasteboard reporting permanently allowed")
            delegate?.pasteboardReporter(self, reportPasteboard: pasteboard)
            completion()
            return
        case .askEachTime:
            DLog("Requesting permission for pasteboard reporting")
            ask(pasteboard: pasteboard, completion: completion)
        }
    }

    private func ask(pasteboard: String, completion: @escaping () -> ()) {
        delegate?.pasteboardReporterRequestPermission(self) { [weak self] allowed, permanently in
            DLog("allowed=\(allowed) permanently=\(permanently)")
            if !allowed {
                if permanently {
                    Self.set(configuration: .never)
                }
                completion()
                return
            }

            // allowed
            if permanently {
                Self.set(configuration: .always)
            }
            if let self = self {
                DLog("Requesting pasteboard report be sent")
                self.delegate?.pasteboardReporter(self, reportPasteboard: pasteboard)
            }
            completion()
        }
    }
}

