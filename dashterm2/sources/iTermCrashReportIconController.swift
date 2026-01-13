//
//  iTermCrashReportIconController.swift
//  DashTerm2
//
//  Manages non-modal crash report icon in tab bar with auto-dismiss.
//

import Cocoa

@objc class iTermCrashReportIconController: NSObject {
    @objc static let shared = iTermCrashReportIconController()

    private static let pendingCrashReportKey = "DTermPendingCrashReport"
    private static let crashReportTimestampKey = "DTermCrashReportTimestamp"
    private static let autoDismissInterval: TimeInterval = 300 // 5 minutes

    private var crashReport: String?
    private var dismissTimer: Timer?
    private weak var iconButton: NSButton?
    private weak var hostingWindow: NSWindow?

    // MARK: - Public Interface

    /// Show the crash icon in the tab bar of the frontmost window.
    /// The icon auto-dismisses after 5 minutes but can be recovered from menu.
    @objc(showCrashIconWith:) func showCrashIcon(with report: String) {
        self.crashReport = report

        // Store for menu recovery
        UserDefaults.standard.set(report, forKey: Self.pendingCrashReportKey)
        UserDefaults.standard.set(Date().timeIntervalSince1970, forKey: Self.crashReportTimestampKey)

        // Setup the icon in tab bar
        DispatchQueue.main.async { [weak self] in
            self?.setupCrashIconInFrontmostWindow()
        }

        // Auto-dismiss after 5 minutes
        dismissTimer?.invalidate()
        dismissTimer = Timer.scheduledTimer(withTimeInterval: Self.autoDismissInterval, repeats: false) { [weak self] _ in
            self?.hideCrashIcon()
        }
    }

    /// Hide the crash icon from tab bar (does not clear pending report for menu recovery)
    @objc func hideCrashIcon() {
        dismissTimer?.invalidate()
        dismissTimer = nil

        DispatchQueue.main.async { [weak self] in
            self?.iconButton?.removeFromSuperview()
            self?.iconButton = nil
            self?.hostingWindow = nil
        }
    }

    /// Show the crash reporter UI with the pending report
    @objc func showCrashReporter() {
        guard let report = crashReport ?? UserDefaults.standard.string(forKey: Self.pendingCrashReportKey) else {
            return
        }

        // Show feedback window with crash report pre-filled
        DTermFeedbackReporter.shared.showFeedbackWindow(withCrashReport: report)
    }

    /// Clear the pending crash report (call after successful submission)
    @objc func clearPendingCrashReport() {
        UserDefaults.standard.removeObject(forKey: Self.pendingCrashReportKey)
        UserDefaults.standard.removeObject(forKey: Self.crashReportTimestampKey)
        crashReport = nil
        hideCrashIcon()
    }

    /// Whether there's a pending crash report (for menu validation)
    @objc var hasPendingCrashReport: Bool {
        if crashReport != nil {
            return true
        }
        return UserDefaults.standard.string(forKey: Self.pendingCrashReportKey) != nil
    }

    /// Menu item validation - enables "Review Crash Report..." only when there's a pending report
    @objc func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(showCrashReporter) {
            return hasPendingCrashReport
        }
        return true
    }

    // MARK: - Tab Bar Icon Setup

    private func setupCrashIconInFrontmostWindow() {
        // Remove existing icon if any
        iconButton?.removeFromSuperview()
        iconButton = nil

        // Get the frontmost terminal window
        guard let frontWindow = NSApp.mainWindow ?? NSApp.keyWindow else {
            return
        }

        // Try to find the tab bar view
        guard let tabBarView = findTabBarView(in: frontWindow.contentView) else {
            return
        }

        // Create the crash icon button
        let button = createCrashIconButton()

        // Position at the right side of the tab bar
        let tabBarFrame = tabBarView.bounds
        let buttonSize: CGFloat = 24
        let padding: CGFloat = 8
        button.frame = NSRect(
            x: tabBarFrame.width - buttonSize - padding,
            y: (tabBarFrame.height - buttonSize) / 2,
            width: buttonSize,
            height: buttonSize
        )
        button.autoresizingMask = [.minXMargin]

        tabBarView.addSubview(button)
        self.iconButton = button
        self.hostingWindow = frontWindow
    }

    private func findTabBarView(in view: NSView?) -> NSView? {
        guard let view = view else { return nil }

        // Look for PSMTabBarControl or iTermTabBarControlView
        let className = NSStringFromClass(type(of: view))
        if className.contains("TabBar") {
            return view
        }

        // Recursively search subviews
        for subview in view.subviews {
            if let tabBar = findTabBarView(in: subview) {
                return tabBar
            }
        }

        return nil
    }

    private func createCrashIconButton() -> NSButton {
        let button = NSButton(frame: NSRect(x: 0, y: 0, width: 24, height: 24))
        button.bezelStyle = .regularSquare
        button.isBordered = false
        button.imagePosition = .imageOnly

        // Use SF Symbol for crash/warning icon if available, otherwise use text
        if #available(macOS 11.0, *) {
            let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            button.image = NSImage(systemSymbolName: "exclamationmark.triangle.fill", accessibilityDescription: "Crash Report")?
                .withSymbolConfiguration(config)
            button.contentTintColor = .systemOrange
        } else {
            // Fallback for older macOS
            button.title = "⚠️"
            button.image = nil
        }

        button.target = self
        button.action = #selector(crashIconClicked(_:))
        button.toolTip = "A crash was detected. Click to review and submit a report."

        // Add subtle pulse animation to draw attention
        addPulseAnimation(to: button)

        return button
    }

    private func addPulseAnimation(to button: NSButton) {
        let pulse = CABasicAnimation(keyPath: "opacity")
        pulse.fromValue = 1.0
        pulse.toValue = 0.6
        pulse.duration = 1.0
        pulse.autoreverses = true
        pulse.repeatCount = 3 // Pulse 3 times then stop
        pulse.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        button.layer?.add(pulse, forKey: "pulseAnimation")
    }

    @objc private func crashIconClicked(_ sender: Any?) {
        showCrashReporter()
    }
}

// MARK: - Silent Crash Detection Extension

extension iTermCrashReportIconController {
    /// Silently check for crash reports without showing any UI.
    /// Returns the crash report text if a new crash was found.
    @objc static func detectLatestCrashReportSilently() -> String? {
        let appName = Bundle.main.object(forInfoDictionaryKey: kCFBundleExecutableKey as String) as? String ?? "DashTerm2"

        // Search paths for crash logs
        let homeDir = NSHomeDirectory()
        let folders = [
            "\(homeDir)/Library/Logs/DiagnosticReports/",
            "\(homeDir)/Library/Logs/CrashReporter/"
        ]

        // Determine file extension based on macOS version
        let crashLogPrefix: String
        let crashLogSuffix: String
        if #available(macOS 12.0, *) {
            crashLogPrefix = "\(appName)-"
            crashLogSuffix = ".ips"
        } else {
            crashLogPrefix = "\(appName)_"
            crashLogSuffix = ".crash"
        }

        var newestPath: String?
        var newestDate: Date?

        for folder in folders {
            guard let contents = try? FileManager.default.contentsOfDirectory(atPath: folder) else {
                continue
            }

            for filename in contents {
                if filename.hasPrefix(crashLogPrefix) && filename.hasSuffix(crashLogSuffix) {
                    let fullPath = (folder as NSString).appendingPathComponent(filename)
                    guard let attrs = try? FileManager.default.attributesOfItem(atPath: fullPath),
                          let modDate = attrs[.modificationDate] as? Date else {
                        continue
                    }

                    if newestDate == nil || modDate > newestDate! {
                        newestPath = fullPath
                        newestDate = modDate
                    }
                }
            }
        }

        guard let crashPath = newestPath, let crashDate = newestDate else {
            return nil // No crash log found
        }

        // Check if we've already reported this crash
        let lastReportInterval = UserDefaults.standard.double(forKey: "UKCrashReporterLastCrashReportDate")
        let lastReportDate = Date(timeIntervalSince1970: lastReportInterval)

        if crashDate <= lastReportDate {
            return nil // Already reported
        }

        // Read the crash log
        guard let crashLog = try? String(contentsOfFile: crashPath, encoding: .utf8) else {
            return nil
        }

        // Add system info header
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "unknown"

        let enrichedReport = """
        Version: \(version) (\(build))
        Crash detected at: \(crashDate)

        \(crashLog)
        """

        return enrichedReport
    }
}
