//
//  iTermRestoreSessionIconController.swift
//  DashTerm2
//
//  Manages non-modal "restore last session" icon in tab bar.
//

import Cocoa

@objc class iTermRestoreSessionIconController: NSObject {
    @objc static let shared = iTermRestoreSessionIconController()

    private static let autoDismissInterval: TimeInterval = 300 // 5 minutes

    private var dismissTimer: Timer?
    private weak var iconButton: NSButton?
    private weak var hostingWindow: NSWindow?
    private var hasSessionToRestore = false

    // MARK: - Public Interface

    /// Show the restore session icon if there's a saved session to restore.
    @objc func showRestoreIconIfNeeded() {
        // Check if there's a saved session arrangement
        guard canRestoreSession() else {
            return
        }

        hasSessionToRestore = true

        // Setup the icon in tab bar after a short delay to let windows initialize
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            self?.setupRestoreIconInFrontmostWindow()
        }

        // Auto-dismiss after 5 minutes
        dismissTimer?.invalidate()
        dismissTimer = Timer.scheduledTimer(withTimeInterval: Self.autoDismissInterval, repeats: false) { [weak self] _ in
            self?.hideRestoreIcon()
        }
    }

    /// Hide the restore icon from tab bar
    @objc func hideRestoreIcon() {
        dismissTimer?.invalidate()
        dismissTimer = nil
        hasSessionToRestore = false

        DispatchQueue.main.async { [weak self] in
            self?.iconButton?.removeFromSuperview()
            self?.iconButton = nil
            self?.hostingWindow = nil
        }
    }

    /// Restore the last session
    @objc func restoreLastSession() {
        // Use the existing window arrangement restoration
        if let controller = NSClassFromString("iTermController") as? NSObject.Type,
           let sharedInstance = controller.perform(NSSelectorFromString("sharedInstance"))?.takeUnretainedValue() as? NSObject {
            // Try to restore saved arrangement
            if sharedInstance.responds(to: NSSelectorFromString("loadWindowArrangementWithName:")) {
                // Get the default arrangement name
                let selector = NSSelectorFromString("loadWindowArrangementWithName:")
                sharedInstance.perform(selector, with: "Default")
            }
        }

        hideRestoreIcon()
    }

    /// Whether there's a session to restore (for menu validation)
    @objc var hasRestorationAvailable: Bool {
        return hasSessionToRestore || canRestoreSession()
    }

    /// Menu item validation
    @objc func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(restoreLastSession) {
            return hasRestorationAvailable
        }
        return true
    }

    // MARK: - Private Methods

    private func canRestoreSession() -> Bool {
        // Check if there's a default window arrangement saved
        if let arrangements = UserDefaults.standard.dictionary(forKey: "NoSyncSavedArrangement") {
            return !arrangements.isEmpty
        }
        // Also check for other restoration state
        if let _ = UserDefaults.standard.string(forKey: "NoSyncLastArrangement") {
            return true
        }
        return false
    }

    // MARK: - Tab Bar Icon Setup

    private func setupRestoreIconInFrontmostWindow() {
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

        // Create the restore icon button
        let button = createRestoreIconButton()

        // Position at the right side of the tab bar (offset from crash icon position)
        let tabBarFrame = tabBarView.bounds
        let buttonSize: CGFloat = 24
        let padding: CGFloat = 40  // Offset more to avoid crash icon
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

    private func createRestoreIconButton() -> NSButton {
        let button = NSButton(frame: NSRect(x: 0, y: 0, width: 24, height: 24))
        button.bezelStyle = .regularSquare
        button.isBordered = false
        button.imagePosition = .imageOnly

        // Use SF Symbol for restore/history icon
        if #available(macOS 11.0, *) {
            let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            button.image = NSImage(systemSymbolName: "clock.arrow.circlepath", accessibilityDescription: "Restore Session")?
                .withSymbolConfiguration(config)
            button.contentTintColor = .systemBlue
        } else {
            // Fallback for older macOS
            button.title = "R"
            button.image = nil
        }

        button.target = self
        button.action = #selector(restoreIconClicked(_:))
        button.toolTip = "Restore your previous session"

        return button
    }

    @objc private func restoreIconClicked(_ sender: Any?) {
        restoreLastSession()
    }
}
