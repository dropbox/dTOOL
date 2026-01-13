//
//  BuildInfoWindowController.swift
//  DashTerm2
//
//  Created by Andrew Yates
//

import Cocoa

@objc class BuildInfoWindowController: NSWindowController {
    private static var sharedController: BuildInfoWindowController?

    private let textView: NSTextView
    private let scrollView: NSScrollView

    @objc static func showBuildInfo() {
        if sharedController == nil {
            sharedController = BuildInfoWindowController()
        }
        sharedController?.showWindow(nil)
        sharedController?.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    init() {
        // Create window
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 200),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Build Info"
        window.center()

        // Create scroll view with text view
        scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 200))
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder

        textView = NSTextView(frame: scrollView.bounds)
        textView.isEditable = false
        textView.isSelectable = true
        textView.backgroundColor = NSColor.textBackgroundColor
        textView.textContainerInset = NSSize(width: 16, height: 16)
        textView.autoresizingMask = [.width, .height]

        // Set monospace font
        textView.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)

        scrollView.documentView = textView
        window.contentView = scrollView

        super.init(window: window)

        updateBuildInfo()
    }

    required init?(coder: NSCoder) {
        it_fatalError("init(coder:) has not been implemented")
    }

    private func updateBuildInfo() {
        let info = """
        DashTerm2 Build Information
        ═══════════════════════════════════════

        Version:    \(BuildInfo.appVersion)
        Build:      \(BuildInfo.buildNumber)
        Git Branch: \(BuildInfo.gitBranch)
        Git Commit: \(BuildInfo.gitCommit)
        Built:      \(BuildInfo.buildTimestamp)

        ═══════════════════════════════════════
        Full Version: \(BuildInfo.fullVersionString)
        """

        textView.string = info
    }
}
